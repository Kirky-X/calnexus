// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! S3 Error Paths —— 11 种 `ErrorKind` 全覆盖、错误 i18n 渲染、退出码契约。
//!
//! 既有套件仅深覆盖 7 种 kind；本模块补齐 `DependencyUnavailable`
//! （经 fx mock 恒失败源确定性复现，零网络）、`Usage`/`UndefinedSymbol`
//! （公开构造器契约 + 表达式层映射现状钉住）与 `Timeout` 的 kind 级断言。

use calnexus::{evaluate, CalcError, CacheManager, ErrorKind, EvalContext, EvalResult, I18n, Lang};
#[cfg(feature = "fx")]
use calnexus::DomainRouter;

use crate::common::{approx_eq, eval_err, eval_ok};

fn kind_of(expr: &str) -> ErrorKind {
    eval_err(expr).kind
}

fn scalar_of(r: &EvalResult) -> f64 {
    match r {
        EvalResult::Scalar(v) => *v,
        other => panic!("expected Scalar, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// 11 种 ErrorKind 逐一断言（meta 清单：新增 kind 时本表必须扩展）
// ---------------------------------------------------------------------------

/// kind → 触发表达式/构造途径的完备性清单。
#[test]
fn error_all_eleven_kinds_covered() {
    // 1. Parse —— 词法/语法错误
    assert_eq!(kind_of("(2+3"), ErrorKind::Parse);
    assert_eq!(kind_of("2 +* 3"), ErrorKind::Parse);
    // 2. Eval —— 域内求值错误（参数个数、未绑定变量映射为 Eval）
    assert_eq!(kind_of("factorial(1,2)"), ErrorKind::Eval);
    let e = eval_err("foo+1");
    assert_eq!(e.kind, ErrorKind::Eval);
    assert!(e.message.contains("unbound variable"), "got {e}");
    // 3. Overflow —— factorial 超出 MAX_FACTORIAL_INPUT(10_000)
    assert_eq!(kind_of("factorial(10001)"), ErrorKind::Overflow);
    // 4. DivisionByZero
    assert_eq!(kind_of("1/0"), ErrorKind::DivisionByZero);
    // 5. Domain —— 定义域/未知函数/未知单位等语义错误
    assert_eq!(kind_of("asin(2)"), ErrorKind::Domain);
    // 6. Depth —— 嵌套深度超 256
    assert_eq!(
        kind_of(&format!("{}1{}", "(".repeat(300), ")".repeat(300))),
        ErrorKind::Depth
    );
    // 7. NaNOrInf —— 浮点结果非有限
    assert_eq!(kind_of("1e308 * 10"), ErrorKind::NaNOrInf);
    // 8. Timeout —— timeout=0 立即超时（构造器契约 + 管线触发）
    assert_eq!(CalcError::timeout().kind, ErrorKind::Timeout);
    let ctx = EvalContext {
        timeout: std::time::Duration::ZERO,
        ..EvalContext::new()
    };
    let err = evaluate("1+1", &ctx, None, &CacheManager::new()).unwrap_err();
    assert_eq!(err.kind, ErrorKind::Timeout);
    // 9. Usage —— CLI 参数用法错误（构造器契约；子进程断言见 cli_e2e）
    let usage = CalcError::usage("invalid --precision value");
    assert_eq!(usage.kind, ErrorKind::Usage);
    assert!(usage.message.contains("invalid --precision"));
    // 10. UndefinedSymbol —— 公开构造器契约。现状钉住：evaluate() 管线
    //     将未绑定变量映射为 Eval（见 #2），UndefinedSymbol kind 当前仅
    //     经构造器与 CLI hint 上下文化逻辑（cli.rs 按 kind 匹配）消费
    let sym = CalcError::undefined_symbol("foo");
    assert_eq!(sym.kind, ErrorKind::UndefinedSymbol);
    assert_eq!(sym.message, "undefined symbol: foo");
    assert_eq!(sym.kind.exit_code(), 1);
    // 11. DependencyUnavailable —— 经 fx mock 恒失败源确定性复现（fx 门控，
    //     见下方独立测试；此处构造器契约）
    assert_eq!(
        CalcError::dependency_unavailable("x").kind,
        ErrorKind::DependencyUnavailable
    );
}

#[test]
fn error_exit_code_contract() {
    // Timeout | DependencyUnavailable → 3；Usage → 2；其余 → 1
    assert_eq!(CalcError::timeout().kind.exit_code(), 3);
    assert_eq!(CalcError::dependency_unavailable("x").kind.exit_code(), 3);
    assert_eq!(CalcError::usage("x").kind.exit_code(), 2);
    assert_eq!(CalcError::parse("x").kind.exit_code(), 1);
    assert_eq!(CalcError::eval("x").kind.exit_code(), 1);
    assert_eq!(CalcError::overflow().kind.exit_code(), 1);
    assert_eq!(CalcError::nan_or_inf().kind.exit_code(), 1);
    assert_eq!(CalcError::domain("x").kind.exit_code(), 1);
    assert_eq!(CalcError::depth_exceeded().kind.exit_code(), 1);
    assert_eq!(CalcError::division_by_zero().kind.exit_code(), 1);
    assert_eq!(CalcError::undefined_symbol("x").kind.exit_code(), 1);
}

// ---------------------------------------------------------------------------
// fx 恒失败源 → DependencyUnavailable 确定性覆盖（零网络）
// ---------------------------------------------------------------------------

#[cfg(feature = "fx")]
#[test]
fn error_fx_dependency_unavailable_deterministic() {
    let router = crate::common::fx_mock::MockFxDomain::failing_router();
    let err = crate::common::eval_via_router("fx_rate(\"USD\", \"EUR\")", &router).unwrap_err();
    assert_eq!(err.kind, ErrorKind::DependencyUnavailable);
    // 算术包装同样传播
    let err = crate::common::eval_via_router("fx(100, \"USD\", \"EUR\") * 2", &router).unwrap_err();
    assert_eq!(err.kind, ErrorKind::DependencyUnavailable);
}

#[cfg(feature = "fx")]
#[test]
fn error_fx_real_domain_offline_tolerant() {
    // 真实 FxDomain::default() 冒烟：断网 → DependencyUnavailable（缓存
    // 过期且未开 ALLOW_STALE），有网/有缓存 → 成功。两侧均视为通过，
    // 保证 CI 断网全绿的同时钉住「不 panic、错误带 kind」契约。
    let mut router = DomainRouter::new();
    router.register(Box::new(calnexus::FxDomain::default()));
    match crate::common::eval_via_router("fx_rate(\"USD\", \"EUR\")", &router) {
        Ok((r, domain, _, _)) => {
            assert_eq!(domain, "fx");
            assert!(matches!(r, EvalResult::Scalar(v) if v > 0.0), "got {r:?}");
        }
        Err(e) => assert!(
            matches!(e.kind, ErrorKind::DependencyUnavailable | ErrorKind::Domain),
            "tolerated kinds are DependencyUnavailable/Domain, got {e:?}"
        ),
    }
}

// ---------------------------------------------------------------------------
// 错误诊断 rich 信息：hint / source / span
// ---------------------------------------------------------------------------

#[test]
fn error_hints_and_source() {
    // 除零带 hint
    let e = eval_err("1/0");
    assert_eq!(e.kind, ErrorKind::DivisionByZero);
    assert!(e.hint.is_some(), "division by zero should carry hint");
    // parse 错误带 span
    let e = eval_err("(2+3");
    assert!(e.span.is_some(), "parse error should carry span");
}

// ---------------------------------------------------------------------------
// i18n 错误渲染
// ---------------------------------------------------------------------------

#[test]
fn error_i18n_friendly_bilingual() {
    let e = CalcError::division_by_zero();
    let en = e.friendly(&I18n::new(Lang::En));
    let zh = e.friendly(&I18n::new(Lang::Zh));
    assert!(!en.is_empty() && !zh.is_empty());
    assert!(
        zh.chars().any(|c| c >= '\u{4e00}'),
        "zh render should contain CJK, got {zh}"
    );
    assert!(
        !en.chars().any(|c| c >= '\u{4e00}'),
        "en render must not contain CJK, got {en}"
    );
}

#[test]
fn error_i18n_key_mapping() {
    // kind → i18n key 一一对应
    assert_eq!(ErrorKind::Parse.i18n_key(), "error.parse");
    assert_eq!(
        ErrorKind::DivisionByZero.i18n_key(),
        "error.division_by_zero"
    );
    assert_eq!(
        ErrorKind::UndefinedSymbol.i18n_key(),
        "error.undefined_symbol"
    );
    assert_eq!(
        ErrorKind::DependencyUnavailable.i18n_key(),
        "error.dependency_unavailable"
    );
}

#[test]
fn error_to_json_contract() {
    let e = CalcError::division_by_zero();
    let json = e.to_json();
    assert!(
        json.starts_with('{') && json.contains("\"error\""),
        "got {json}"
    );
    // to_explain 教育模式输出非空
    assert!(!e.to_explain(&I18n::new(Lang::En)).is_empty());
}

#[test]
fn i18n_lang_resolution() {
    assert_eq!(I18n::default().lang(), Lang::En);
    assert_eq!(I18n::new(Lang::Zh).lang(), Lang::Zh);
    assert_eq!(I18n::from_str("zh-CN").lang(), Lang::Zh);
    assert_eq!(I18n::from_str("zh-Hans").lang(), Lang::Zh);
    assert_eq!(I18n::from_str("en-US").lang(), Lang::En);
    // 未知标签回落 En（永不失败）
    assert_eq!(I18n::from_str("xx-FAKE").lang(), Lang::En);
}

#[test]
fn i18n_message_lookup() {
    let i18n = I18n::new(Lang::En);
    // 已知键取译文；未知键回落键名本身
    assert_eq!(i18n.t("error.parse"), "Parse error");
    assert_eq!(i18n.t("no.such.key"), "no.such.key");
    // tf 占位符替换
    let s = i18n.tf("msg.unbound_variable", &[("name", "foo")]);
    assert!(s.contains("foo"), "placeholder substitution, got {s}");
}

// ---------------------------------------------------------------------------
// 表达式层错误路径补充（非法实参类型与跨域混用）
// ---------------------------------------------------------------------------

#[test]
fn error_string_operand_rejected_outside_function_args() {
    // 字符串字面量仅可作为函数实参；出现在操作数上下文被各域拒绝
    let e = eval_err("1 + \"a\"");
    assert_eq!(e.kind, ErrorKind::Domain);
}

#[cfg(feature = "unit")]
#[test]
fn error_cross_domain_function_rejected() {
    // sin 混入 unit 域表达式 → Domain 且点名函数
    let e = eval_err("sin(1) + convert(1, \"km\", \"m\")");
    assert_eq!(e.kind, ErrorKind::Domain);
    assert!(
        e.message.contains("sin"),
        "should name offending function, got {e}"
    );
}

#[test]
fn error_valid_after_error_recovery() {
    // 同一进程内错误后继续求值不受污染（状态隔离）
    let _ = eval_err("1/0");
    let _ = eval_err("(2+3");
    assert!(approx_eq(scalar_of(&eval_ok("2+3")), 5.0));
}
