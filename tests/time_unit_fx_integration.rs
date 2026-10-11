// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
// CalcError 144 字节，函数签名触发 result_large_err——与 lib.rs 同论证
#![allow(clippy::result_large_err)]

//! 端到端集成测试：经公共 `calnexus::evaluate()` 验证 TimeDomain / UnitDomain / FxDomain 全链路。
//! 覆盖 date_diff、convert、fx 网络容错、now() 缓存旁路与跨域 Domain 错误。

#![allow(clippy::approx_constant)]

use calnexus::{CacheManager, CalcError, ErrorKind, EvalContext, EvalResult, evaluate};
use std::time::Duration;

/// 辅助：通过公共 evaluate 入口求值，返回 (result, domain, cache_hit, fmt_prec)。
fn eval_full(
    expr: &str,
    ctx: &EvalContext,
    cache: &CacheManager,
) -> Result<(EvalResult, String, bool, Option<usize>), CalcError> {
    evaluate(expr, ctx, None, cache)
}

/// 辅助：通过公共 evaluate 入口求值并断言成功，提取标量值。
fn eval_scalar(expr: &str) -> f64 {
    let ctx = EvalContext::new();
    let cache = CacheManager::new();
    let (result, _, _, _) = eval_full(expr, &ctx, &cache).expect("expected successful evaluation");
    result
        .as_scalar()
        .unwrap_or_else(|| panic!("expected Scalar result, got {:?}", result))
}

// ============================================================================
// TimeDomain 端到端
// ============================================================================

/// 缺省 day 单位：date_diff 跨半年应为 205 天。
#[test]
fn test_time_date_diff_days_end_to_end() {
    let value = eval_scalar(r#"date_diff("2026-01-01","2026-07-25")"#);
    assert_eq!(value, 205.0);
}

/// date 多格式识别：两种写法解析结果相等。
#[test]
fn test_time_date_multi_format_equality() {
    let ctx = EvalContext::new();
    let cache = CacheManager::new();
    let (r1, _, _, _) = eval_full(r#"date("25 Jul 2026")"#, &ctx, &cache).unwrap();
    let (r2, _, _, _) = eval_full(r#"date("2026-07-25")"#, &ctx, &cache).unwrap();
    assert_eq!(
        r1, r2,
        "multi-format date parse should yield equal DateTime"
    );
    // 确认是 DateTime 变体且内容正确
    match r1 {
        EvalResult::DateTime(s) => {
            assert!(
                s.starts_with("2026-07-25T00:00:00"),
                "expected 2026-07-25T00:00:00 prefix, got {}",
                s
            );
        }
        other => panic!("expected DateTime variant, got {:?}", other),
    }
}

/// reformat_date 按格式串转换，返回 Symbolic("2026-07-25")。
#[test]
fn test_time_reformat_date_end_to_end() {
    let ctx = EvalContext::new();
    let cache = CacheManager::new();
    let (result, domain, _, _) = eval_full(
        r#"reformat_date("25/07/2026","%d/%m/%Y","%Y-%m-%d")"#,
        &ctx,
        &cache,
    )
    .unwrap();
    assert_eq!(domain, "time");
    match result {
        EvalResult::Symbolic(s) => assert_eq!(s, "2026-07-25"),
        other => panic!("expected Symbolic(\"2026-07-25\"), got {:?}", other),
    }
}

// ============================================================================
// UnitDomain 端到端
// ============================================================================

/// 长度线性换算：convert(100,"cm","m") = 1。
#[test]
fn test_unit_convert_length_end_to_end() {
    let value = eval_scalar(r#"convert(100,"cm","m")"#);
    assert!((value - 1.0).abs() < 1e-9, "expected 1.0, got {}", value);
}

/// 温度仿射路径：convert(0,"C","F") = 32。
#[test]
fn test_unit_convert_temperature_end_to_end() {
    let value = eval_scalar(r#"convert(0,"C","F")"#);
    assert!((value - 32.0).abs() < 1e-9, "expected 32.0, got {}", value);
}

/// 算术包围：convert 结果可参与四则运算（*2 = 2）。
#[test]
fn test_unit_arithmetic_wrapping_end_to_end() {
    let value = eval_scalar(r#"convert(100,"cm","m")*2"#);
    assert!((value - 2.0).abs() < 1e-9, "expected 2.0, got {}", value);
}

/// 跨域函数混入（sin + convert）须返回 DomainError。
#[test]
fn test_unit_cross_domain_function_rejected_end_to_end() {
    let ctx = EvalContext::new();
    let cache = CacheManager::new();
    let result = eval_full(r#"sin(1)+convert(1,"km","m")"#, &ctx, &cache);
    let err = result.expect_err("expected DomainError for cross-domain function mixing");
    assert_eq!(
        err.kind,
        ErrorKind::Domain,
        "expected Domain error kind, got {:?}",
        err.kind
    );
    // 消息应含 "sin" 以指明被拒绝的跨域函数名
    assert!(
        err.message.contains("sin"),
        "error message should mention 'sin', got: {}",
        err.message
    );
}

// ============================================================================
// FxDomain 端到端（网络容错）
// ============================================================================

/// fx 网络容错：接受 Ok 或 DomainError；成功时 domain="fx" 且旁路缓存。
#[test]
fn test_fx_expression_end_to_end_network_tolerant() {
    let ctx = EvalContext::new();
    // 给 fx 较宽松的超时，避免 CI 网络抖动误判
    let mut ctx = ctx;
    ctx.timeout = Duration::from_secs(15);
    let cache = CacheManager::new();
    let result = eval_full(r#"fx(1,"USD","EUR")"#, &ctx, &cache);

    match result {
        Ok((EvalResult::Scalar(_), domain, cache_hit, _)) => {
            // 网络可达：必须路由到 fx 域；fx 是非确定性函数，cache_hit 必为 false
            assert_eq!(domain, "fx", "fx expression must route to fx domain");
            assert!(!cache_hit, "fx is nondeterministic, cache must be bypassed");
        }
        Ok((other, domain, _, _)) => {
            panic!(
                "expected Scalar or DomainError, got {:?} from domain {}",
                other, domain
            );
        }
        Err(e) => {
            // 网络不可达：应为 Domain 错误（provider 报 network_unreachable / stale_hint 等）
            assert_eq!(
                e.kind,
                ErrorKind::Domain,
                "network failure should yield Domain error, got kind={:?} msg={}",
                e.kind,
                e.message
            );
        }
    }
}

// ============================================================================
// 非确定性函数缓存旁路
// ============================================================================

/// now() 非确定性：两次求值 cache_hit 均为 false。
#[test]
fn test_now_cache_bypass_end_to_end() {
    let ctx = EvalContext::new();
    let cache = CacheManager::new();

    // 第一次求值：now() 是非确定性函数，应跳过 cache.get 与 cache.insert
    let (r1, domain1, cache_hit1, _) = eval_full("now()", &ctx, &cache).unwrap();
    assert_eq!(domain1, "time", "now() must route to time domain");
    assert!(
        !cache_hit1,
        "nondeterministic function must bypass cache read (first call)"
    );
    // 确认返回 DateTime
    assert!(
        matches!(r1, EvalResult::DateTime(_)),
        "now() must return DateTime"
    );

    // 第二次求值：依然不应命中缓存（即使 cache 中已有其他条目）
    let (r2, _, cache_hit2, _) = eval_full("now()", &ctx, &cache).unwrap();
    assert!(
        !cache_hit2,
        "nondeterministic function must bypass cache read (second call)"
    );
    assert!(
        matches!(r2, EvalResult::DateTime(_)),
        "now() must return DateTime"
    );
}

// ============================================================================
// Str 参与 BinaryOp 的显式 Domain 错误
// ============================================================================

/// Str 参与 BinaryOp 无域匹配，返回 kind=Domain 错误。
#[test]
fn test_str_in_binary_op_returns_domain_error() {
    let ctx = EvalContext::new();
    let cache = CacheManager::new();
    let result = eval_full(r#"1+"a""#, &ctx, &cache);
    let err = result.expect_err("expected DomainError for Str in BinaryOp");
    assert_eq!(
        err.kind,
        ErrorKind::Domain,
        "expected Domain error kind for Str in BinaryOp, got {:?}",
        err.kind
    );
}
