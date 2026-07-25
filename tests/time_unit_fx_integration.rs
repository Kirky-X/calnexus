// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! time-unit-fx-domains Phase 5 端到端集成测试。
//!
//! 经公共 `calnexus::evaluate()` 入口验证 TimeDomain / UnitDomain / FxDomain 的全链路
//! 行为（parse → canonicalize → cache → route → evaluate）。覆盖：
//! - TimeDomain：date_diff / date 多格式识别 / reformat_date
//! - UnitDomain：convert 线性路径 / 温度仿射 / 算术包围 / 跨域函数拒绝
//! - FxDomain：网络可达性容错（成功或 DomainError 均可接受）
//! - 非确定性函数 now() 缓存旁路（两次求值 cache_hit 均 false）
//! - Str 参与 BinaryOp 的显式 Domain 错误（`1+"a"`）

#![allow(clippy::approx_constant)]

use calnexus::{evaluate, CacheManager, CalcError, ErrorKind, EvalContext, EvalResult};
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

/// T029 用例 1：`date_diff("2026-01-01","2026-07-25")` = 205（day 单位缺省）。
#[test]
fn test_time_date_diff_days_end_to_end() {
    let value = eval_scalar(r#"date_diff("2026-01-01","2026-07-25")"#);
    assert_eq!(value, 205.0);
}

/// T029 用例 2：`date("25 Jul 2026")` 与 `date("2026-07-25")` 结果相等（多格式识别）。
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

/// T029 用例 3：`reformat_date("25/07/2026","%d/%m/%Y","%Y-%m-%d")` = Symbolic("2026-07-25")。
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

/// T029 用例 4：`convert(100,"cm","m")` = 1。
#[test]
fn test_unit_convert_length_end_to_end() {
    let value = eval_scalar(r#"convert(100,"cm","m")"#);
    assert!((value - 1.0).abs() < 1e-9, "expected 1.0, got {}", value);
}

/// T029 用例 5：`convert(0,"C","F")` = 32（温度仿射路径）。
#[test]
fn test_unit_convert_temperature_end_to_end() {
    let value = eval_scalar(r#"convert(0,"C","F")"#);
    assert!((value - 32.0).abs() < 1e-9, "expected 32.0, got {}", value);
}

/// T029 用例 6：`convert(100,"cm","m")*2` = 2（算术包围）。
#[test]
fn test_unit_arithmetic_wrapping_end_to_end() {
    let value = eval_scalar(r#"convert(100,"cm","m")*2"#);
    assert!((value - 2.0).abs() < 1e-9, "expected 2.0, got {}", value);
}

/// T029 用例 9：`sin(1)+convert(1,"km","m")` 返回 DomainError（跨域函数混入）。
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

/// T029 用例 7：fx 表达式经公共 evaluate 入口求值。
///
/// 公共 API 无法注入 mock provider，FrankfurterProvider 会尝试访问 frankfurter.dev。
/// 测试环境可能无网络，故接受两种结果：
/// - Ok：网络可达，fx 路由至 fx 域并返回 Scalar
/// - Err(DomainError)：网络不可达或缓存过期且不允许 stale
///
/// 关键验收点：表达式不会 panic，且不会路由到错误域（domain 名应为 "fx"）。
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

/// T029 用例 8：`now()` 两次求值 cache_hit 均为 false（R-ncb-003 缓存旁路）。
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

/// T029 用例 10：`1+"a"` 返回 kind=Domain 错误（Str 在 BinaryOp 中无域匹配，R-esl-005）。
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
