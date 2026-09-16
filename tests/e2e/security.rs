// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! S7 Security & Robustness —— 注入、DoS 向量、深度嵌套、资源限制。
//!
//! 与既有 security_tests.rs（CLI 子进程视角）互补，本模块从库 API 面
//! 钉住：解析/求值对恶意输入永不 panic、深度守卫 256 精确边界、
//! 输入长度上界、NaN/Inf 传播拦截。嵌套用例在 16MB 栈线程运行
//! （守卫必须在栈溢出前拒绝输入——sec_011 同款前置条件）。

use calnexus::{CacheManager, CalNexus, ErrorKind, EvalContext, EvalResult, evaluate};

use crate::common::{approx_eq, eval_err, eval_ok};

fn eval_raw(expr: &str) -> Result<EvalResult, calnexus::CalcError> {
    evaluate(expr, &EvalContext::new(), None, &CacheManager::new()).map(|(v, _, _, _)| v)
}

/// 在 16MB 栈线程运行 f 并要求不 panic（深嵌套场景前置条件）。
fn on_big_stack<F>(f: F)
where
    F: FnOnce() + Send + 'static,
{
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap();
}

// ---------------------------------------------------------------------------
// 注入向量（库层面无副作用、无 panic）
// ---------------------------------------------------------------------------

#[test]
fn sec_shell_metacharacters_inert() {
    // 分号/反引号/$() 等元字符只能是解析错误或尾部输入——无执行面
    for expr in [
        "1; rm -rf /",
        "1 && echo pwned",
        "`id`",
        "$(whoami)",
        "1 | cat /etc/passwd",
        "../../etc/passwd",
    ] {
        let err = eval_raw(expr).unwrap_err();
        assert_eq!(err.kind, ErrorKind::Parse, "{expr}: {err}");
    }
    // 进程存活且后续求值正常（状态未污染）
    assert!(approx_eq(
        match eval_ok("2+2") {
            EvalResult::Scalar(v) => v,
            _ => panic!(),
        },
        4.0
    ));
}

#[test]
fn sec_control_and_unicode_chars_rejected_gracefully() {
    // 控制字符与 emoji：拒绝但消息合法（span 按字符偏移，不 panic）
    for expr in ["\u{0}+1", "🎉+1", "\u{7}1"] {
        let err = eval_raw(expr).unwrap_err();
        assert_eq!(err.kind, ErrorKind::Parse, "{expr:?}");
    }
    // 多字节字符出现在错误消息中（不触发重切 UTF-8 边界 panic）
    let err = eval_err("🎉+1");
    assert!(
        err.message.contains("🎉") || err.span.is_some(),
        "got {err}"
    );
}

// ---------------------------------------------------------------------------
// 资源限制：表达式长度 / 嵌套深度
// ---------------------------------------------------------------------------

#[test]
fn sec_expression_length_limit() {
    // MAX_EXPR_LEN = 4096：超长一律 Parse 错误（含超长标识符）
    let long = "a".repeat(10_000) + " + 1";
    let err = eval_raw(&long).unwrap_err();
    assert_eq!(err.kind, ErrorKind::Parse);
    assert!(err.message.contains("4096"), "got {err}");
    // 扁平长表达式（10001 字符）同样受限
    let flat = "1+".repeat(5000) + "1";
    let err = eval_raw(&flat).unwrap_err();
    assert_eq!(err.kind, ErrorKind::Parse);
}

#[test]
fn sec_depth_guard_exact_boundary() {
    // MAX_DEPTH = 256：合法边界内求值成功，超界 Depth 错误。
    // 256 层合法输入需 mathexpr 递归解析 256 深度，libtest 默认 2MB
    // 线程栈不足（CLI 子进程走 8MB 主线程不受影响），故大栈线程运行。
    on_big_stack(|| {
        let ok256 = "(".repeat(256) + "1" + &")".repeat(256);
        assert!(approx_eq(
            match eval_raw(&ok256).unwrap() {
                EvalResult::Scalar(v) => v,
                other => panic!("Scalar expected, got {other:?}"),
            },
            1.0
        ));
        let bad257 = "(".repeat(257) + "1" + &")".repeat(257);
        let err = eval_raw(&bad257).unwrap_err();
        assert_eq!(err.kind, ErrorKind::Depth);
        // hint 指明上限（安全引导而非裸错误）
        let hint_text = format!("{}{}", err.hint.as_deref().unwrap_or(""), err.message);
        assert!(
            hint_text.contains("256"),
            "depth hint should mention 256: {hint_text}"
        );
        // prescan 极端：2041 层同样 Depth（迭代预检在递归解析前拒绝）
        let p2041 = "(".repeat(2041) + "1" + &")".repeat(2041);
        assert_eq!(eval_raw(&p2041).unwrap_err().kind, ErrorKind::Depth);
    });
}

#[test]
fn sec_deep_nested_containers_rejected_gracefully() {
    // 300 层嵌套列表/矩阵：parse 产出后由域求值拒绝——全程无 panic
    on_big_stack(|| {
        let nested = "[".repeat(300) + "1" + &"]".repeat(300);
        let err = eval_raw(&nested).unwrap_err();
        assert_eq!(err.kind, ErrorKind::Domain);
        let nested_m = "[[".repeat(150) + "1" + &"]]".repeat(150);
        let err = eval_raw(&nested_m).unwrap_err();
        assert_eq!(err.kind, ErrorKind::Domain);
        assert!(err.message.contains("scalars"), "got {err}");
    });
}

// ---------------------------------------------------------------------------
// 数值健壮性：溢出 / NaN / Inf
// ---------------------------------------------------------------------------

#[test]
fn sec_integer_boundary_no_panic() {
    // i64 边界字面量经 f64/BigInt 通道不 panic
    for expr in [
        "9223372036854775807",
        "9223372036854775808",
        "-9223372036854775809",
    ] {
        let r = eval_raw(expr);
        assert!(r.is_ok(), "{expr} should evaluate, got {r:?}");
    }
    // 巨大阶乘实参 → Overflow（受 MAX_FACTORIAL_INPUT 约束）
    assert_eq!(
        eval_raw("factorial(99999999999)").unwrap_err().kind,
        ErrorKind::Overflow
    );
}

#[test]
fn sec_nonfinite_inputs_rejected() {
    // 超大字面量折叠为 Inf → NaNOrInf 拦截（不落库、不返回）
    assert_eq!(eval_raw("1e309").unwrap_err().kind, ErrorKind::NaNOrInf);
    assert_eq!(eval_raw("-1e309").unwrap_err().kind, ErrorKind::NaNOrInf);
    // 门面直接注入 NaN/Inf → NaNOrInf
    let cn = CalNexus::new();
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let err = cn.scalar().add(bad, 1.0).unwrap_err();
        assert_eq!(err.kind, ErrorKind::NaNOrInf);
    }
}

#[test]
fn sec_timeout_bounds_evaluation() {
    // timeout=0 立即拦截（脆弱时序不可测，故用零上界钉住机制存在性）
    let ctx = EvalContext {
        timeout: std::time::Duration::ZERO,
        ..EvalContext::new()
    };
    let err = evaluate("1+1", &ctx, None, &CacheManager::new()).unwrap_err();
    assert_eq!(err.kind, ErrorKind::Timeout);
}

// ---------------------------------------------------------------------------
// 组合 DoS 向量
// ---------------------------------------------------------------------------

#[test]
fn sec_combined_vectors_survive() {
    // 多向量连击后进程仍健康（每向量一个真实 DoS 载荷形态）
    on_big_stack(|| {
        let vectors = [
            "1; rm -rf /".to_string(),
            "(".repeat(257) + "1" + &")".repeat(257),
            "[".repeat(300) + "1" + &"]".repeat(300),
            "factorial(99999999999)".to_string(),
            "1e308 * 1e308".to_string(),
            "a".repeat(5000),
        ];
        for v in vectors {
            let _ = eval_raw(&v);
        }
        // 存活 + 正确性未受影响
        assert!(approx_eq(
            match eval_ok("6*7") {
                EvalResult::Scalar(x) => x,
                _ => panic!(),
            },
            42.0
        ));
    });
}
