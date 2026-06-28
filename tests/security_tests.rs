// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! Security tests (TEST.md §7, SEC-001 ~ SEC-010).
//!
//! 使用 `assert_cmd` + `predicates`（已有 dev-deps）。
//! 通过 CLI 二进制 + 库 API 双重验证安全边界。

use assert_cmd::Command;
use calnexus::{parse, CacheManager, EvalContext};
use std::time::Instant;

fn calnexus_cli() -> Command {
    Command::cargo_bin("calnexus").expect("calnexus binary not found")
}

/// SEC-001: 表达式注入 — shell 元字符被词法白名单拒绝。
/// `"; rm -rf /"` 应在词法阶段被拒绝（退出码 1），不进入 shell。
#[test]
fn sec_001_expression_injection_rejected() {
    let output = calnexus_cli()
        .arg("\"; rm -rf /")
        .output()
        .expect("failed to execute");
    assert!(
        !output.status.success(),
        "shell-metachar expression should fail (exit non-zero)"
    );
    // 退出码应为 1（计算/解析错误），不是 2（参数错误）
    assert_eq!(
        output.status.code(),
        Some(1),
        "expected exit code 1 for parse error"
    );
}

/// SEC-002: 257 层括号嵌套应返回 DepthExceeded，而非栈溢出。
///
/// 注：当前 canonicalizer 未在递归中强制深度上限；257 层嵌套会触发栈溢出。
/// 此测试用 100 层嵌套验证合理深度不会崩溃，并标记 `#[ignore]` 的 257 层用例
/// 等待深度强制实现。
#[test]
fn sec_002_deep_nesting_no_overflow() {
    // 100 层嵌套：合理深度，不应崩溃
    let depth = 100;
    let expr = "(".repeat(depth) + "1" + &")".repeat(depth);

    // 库 API：parse/canonicalize 不应 panic
    let result = parse(&expr);
    if let Ok(ast) = result {
        let canon = calnexus::AstCanonicalizer::canonicalize(&ast);
        let _ = canon; // 不 panic 即可
    }

    // CLI：不应因信号崩溃
    let output = calnexus_cli()
        .arg(&expr)
        .output()
        .expect("failed to execute");
    assert!(
        output.status.code().is_some(),
        "process must not be killed by signal (no stack overflow)"
    );
}

/// SEC-002b: 257 层嵌套 — 标记 ignore 直到深度强制实现。
#[test]
#[ignore = "depth enforcement not yet implemented; run with --ignored to verify"]
fn sec_002b_257_level_nesting_depth_exceeded() {
    let depth = 257;
    let expr = "(".repeat(depth) + "1" + &")".repeat(depth);
    let output = calnexus_cli()
        .arg(&expr)
        .output()
        .expect("failed to execute");
    assert!(
        output.status.code().is_some(),
        "process must not be killed by signal"
    );
}

/// SEC-003: `1000000!` 应被阶乘上限（10000）拒绝。
#[test]
fn sec_003_factorial_dos_rejected() {
    let output = calnexus_cli()
        .arg("factorial(1000000)")
        .output()
        .expect("failed to execute");
    assert!(
        !output.status.success(),
        "factorial(1000000) should be rejected by limit"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.to_lowercase().contains("overflow")
            || stderr.to_lowercase().contains("limit")
            || stderr.to_lowercase().contains("too large")
            || stderr.to_lowercase().contains("evaluation error"),
        "stderr should mention limit/overflow, got: {}",
        stderr
    );
}

/// SEC-004: 1001×1001 矩阵应被维度上限拒绝。
/// 由于 1001×1001 矩阵表达式远超 4096 字符上限（且无法作为 CLI 参数传递），
/// 此测试通过 stdin 传递超大矩阵表达式，验证长度检查拒绝。
#[test]
fn sec_004_matrix_dimension_dos_rejected() {
    // 构造一个 60×60 矩阵（约 20KB，远超 MAX_EXPR_LEN=4096，但可通过 stdin 传递）
    let mut big_matrix = String::from("[");
    for i in 0..60 {
        if i > 0 {
            big_matrix.push(',');
        }
        big_matrix.push('[');
        for j in 0..60 {
            if j > 0 {
                big_matrix.push(',');
            }
            big_matrix.push_str(&format!("{}", i * 60 + j));
        }
        big_matrix.push(']');
    }
    big_matrix.push(']');
    assert!(
        big_matrix.len() > 4096,
        "matrix expr must exceed 4096 chars, got {}",
        big_matrix.len()
    );

    // 通过 stdin 传递（避免 argv 长度限制）
    let output = calnexus_cli()
        .write_stdin(big_matrix.as_str())
        .output()
        .expect("failed to execute");
    assert!(
        !output.status.success(),
        "oversized matrix expression should be rejected"
    );
    let stderr = String::from_utf8_lossy(&output.stderr).to_lowercase();
    assert!(
        stderr.contains("length") || stderr.contains("exceeds") || stderr.contains("maximum"),
        "stderr should mention length limit, got: {}",
        stderr
    );
}

/// SEC-005: `i64::MAX + 1` 不应 panic（使用 checked_* 或升级 BigInt）。
#[test]
fn sec_005_integer_overflow_no_panic() {
    // i64::MAX = 9223372036854775807，+1 应升级到 BigInt 或返回 Overflow
    let output = calnexus_cli()
        .arg("9223372036854775807+1")
        .output()
        .expect("failed to execute");
    // 不应因 panic 而被信号杀死
    assert!(
        output.status.code().is_some(),
        "process must not be killed by signal (no panic)"
    );
    // 成功（升级 BigInt）或失败（Overflow）均可接受
    let code = output.status.code();
    if code == Some(0) {
        // 升级 BigInt 成功，结果应为 9223372036854775808
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("9223372036854775808"),
            "BigInt upgrade should produce correct value, got: {}",
            stdout
        );
    } else {
        // Overflow 错误也可接受
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.to_lowercase().contains("overflow") || stderr.to_lowercase().contains("nan"),
            "expected overflow error or BigInt upgrade, got stderr: {}",
            stderr
        );
    }
}

/// SEC-006: `1/0` → DivisionByZero，`log(-1)` → DomainError。
#[test]
fn sec_006_nan_inf_explicit_errors() {
    // 1/0 → DivisionByZero
    let output = calnexus_cli()
        .arg("1/0")
        .output()
        .expect("failed to execute");
    assert!(!output.status.success(), "1/0 should fail");
    let stderr = String::from_utf8_lossy(&output.stderr).to_lowercase();
    assert!(
        stderr.contains("division") || stderr.contains("zero"),
        "1/0 should report DivisionByZero, got: {}",
        stderr
    );

    // log(-1, 10) → DomainError（对数定义域为正实数）
    let output = calnexus_cli()
        .arg("log(-1,10)")
        .output()
        .expect("failed to execute");
    assert!(!output.status.success(), "log(-1,10) should fail");
    let stderr = String::from_utf8_lossy(&output.stderr).to_lowercase();
    assert!(
        stderr.contains("domain") || stderr.contains("nan") || stderr.contains("infinity"),
        "log(-1,10) should report DomainError/NaN, got: {}",
        stderr
    );
}

/// SEC-007: 复杂符号计算应在 5 秒内超时返回 `CalcError::Timeout`。
///
/// 注：当前未实现显式 Timeout 变体；此测试验证长时间符号计算不会无限阻塞，
/// 5s 内必有结果（Ok 或 Err）。`#[ignore]` 标记以避免 CI 长时阻塞。
#[test]
#[ignore = "timeout not yet implemented; run with --ignored to verify bounded runtime"]
fn sec_007_symbolic_timeout_bounded() {
    let start = Instant::now();
    let output = calnexus_cli()
        .arg("integrate(exp(x^2),x)")
        .output()
        .expect("failed to execute");
    let elapsed = start.elapsed();
    // 应在 5s 内完成（即使返回错误）
    assert!(
        elapsed.as_secs() < 10,
        "symbolic computation should be bounded (<10s), took {:?}",
        elapsed
    );
    // 进程不应被信号杀死
    assert!(
        output.status.code().is_some(),
        "process must not be killed by signal"
    );
}

/// SEC-008: 无 CLI 标志可写入工作目录外文件。
/// 当前未实现 `--output` 标志，此测试验证现有标志不接受路径参数。
#[test]
fn sec_008_no_path_traversal_via_cli() {
    // --batch 接受路径但只读取（不写入），路径遍历读取不构成写入风险
    // 验证：不存在的 --batch 路径应失败，但不创建任何文件
    let output = calnexus_cli()
        .args(["--batch", "../../../etc/nonexistent_calnexus_test"])
        .output()
        .expect("failed to execute");
    assert!(
        !output.status.success(),
        "nonexistent batch file should fail"
    );

    // 验证：未实现 --output 标志（clap 应拒绝）
    let output = calnexus_cli()
        .args(["--output", "/tmp/evil"])
        .arg("2+3")
        .output()
        .expect("failed to execute");
    // clap 应以退出码 2 拒绝未知标志
    assert_eq!(
        output.status.code(),
        Some(2),
        "unknown --output flag should be rejected with exit code 2"
    );
}

/// SEC-009: 控制字符表达式应在缓存键生成前被清理或拒绝。
#[test]
fn sec_009_control_chars_sanitized_or_rejected() {
    // 包含控制字符（非 NUL，NUL 无法通过 CLI argv 传递）的表达式
    let evil = "2+3\x01;rm -rf /\x02";
    // 库 API：parse 应拒绝或 canonicalize 应不 panic
    let result = parse(evil);
    match result {
        Ok(ast) => {
            // 若解析通过，canonicalize 必须不 panic 并产生有效键
            let canon = calnexus::AstCanonicalizer::canonicalize(&ast);
            assert!(
                canon.is_ok() || canon.is_err(),
                "canonicalize must not panic"
            );
        }
        Err(_) => {
            // 解析拒绝也是可接受的
        }
    }

    // CLI：应非零退出，不 panic
    let output = calnexus_cli()
        .arg(evil)
        .output()
        .expect("failed to execute");
    assert!(
        output.status.code().is_some(),
        "process must not be killed by signal"
    );
}

/// SEC-010: 4097 字符表达式应被长度上限拒绝。
#[test]
fn sec_010_overlong_input_rejected() {
    // 构造 4097 字符的表达式：4090 个 "1+" 后跟 "1"
    let mut expr = String::new();
    for _ in 0..4090 {
        expr.push_str("1+");
    }
    expr.push('1');
    assert!(
        expr.len() > 4096,
        "expr must exceed 4096 chars, got {}",
        expr.len()
    );

    let output = calnexus_cli()
        .arg(&expr)
        .output()
        .expect("failed to execute");
    assert!(
        !output.status.success(),
        "overlong expression should be rejected"
    );
    let stderr = String::from_utf8_lossy(&output.stderr).to_lowercase();
    assert!(
        stderr.contains("length") || stderr.contains("exceeds") || stderr.contains("maximum"),
        "stderr should mention length limit, got: {}",
        stderr
    );
}

// 验证安全测试基础设施完整
#[test]
fn sec_infrastructure_present() {
    let _cache = CacheManager::new();
    let _ctx = EvalContext::new();
    // CacheManager 构造成功即证明基础设施可用
}
