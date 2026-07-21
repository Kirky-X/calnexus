// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! Server evaluate 接口层：`#[forge]` 声明式封装 + `CalcError` → `ApiError` 映射。
//!
//! spec.md R-sdforge-007：单个 `#[forge]` async fn 同时生成 HTTP 路由与 MCP tool，
//! 取代 p1 阶段的 `inventory::submit!` + `SdForgeTool` 手写封装 + `preserve_*` 链接器 hack。
//! 链接器 inventory 保留由 `sdforge::init_all_plugins()` 托管（http.rs/mcp.rs 调用）。

use super::shared_cache;
use super::{EvaluateRequest, EvaluateResponse};
use crate::core::{CalcError, ErrorKind};
use crate::evaluate as eval_expr;
use sdforge::error::ApiError;
use sdforge::forge;
use std::time::Duration;

/// 请求级超时（秒）：防止慢攻击（slowloris）与无限循环表达式耗尽连接资源。
///
/// spec.md R-sdforge-002 安全约束：HTTP/MCP 请求必须在合理时间内返回（成功或 503）。
/// 与 `CalcError::Timeout`（计算层超时，由 evaluate 内部 Alarm 控制）互补：
/// - 计算层超时：精确中断 evaluate 内部的循环
/// - 请求级超时：兜底保护，覆盖 spawn_blocking 启动 / 缓存写入等任何意外延迟
pub const REQUEST_TIMEOUT_SECS: u64 = 30;

/// `POST /api/v1/evaluate` + MCP `evaluate` tool（单函数双协议）。
///
/// `#[forge]` 宏自动生成 axum handler（消费 `EvaluateRequest` body + `ApiError::into_response`
/// 错误路径）与 MCP tool struct/schema（input_schema 从 `EvaluateRequest` 字段推导），
/// 并 `inventory::submit!` 注册。`name="evaluate"` + `version="v1"` 决定路径前缀 `/api/v1`，
/// 叠加 `path="/evaluate"` → 最终路由 `/api/v1/evaluate`（spec.md R-sdforge-002）。
///
/// 函数体内保留 `spawn_blocking` 隔离 oxcache 同步 `block_on`（与 p1 手写 handler 等价）：
/// `#[forge]` 只生成外层路由/tool 外壳，函数体仍是 CalNexus 代码，隔离策略不变。
///
/// BUG-S-M-001: 用 `tokio::time::timeout` 包裹 `spawn_blocking`，超时返回 503
/// ServiceUnavailable（`retry_after=REQUEST_TIMEOUT_SECS`），防止慢攻击。
#[forge(
    name = "evaluate",
    version = "v1",
    path = "/evaluate",
    method = "POST",
    tool_name = "evaluate",
    description = "Evaluate a math expression. Returns {result, domain, cache}. Supports vars and precision."
)]
pub(crate) async fn evaluate(req: EvaluateRequest) -> Result<EvaluateResponse, ApiError> {
    evaluate_with_timeout(req, Duration::from_secs(REQUEST_TIMEOUT_SECS)).await
}

/// 带超时的 evaluate 实现（可测试入口）。
///
/// 提取为独立函数以便单元测试注入短超时验证 503 路径，避免依赖真实慢表达式。
/// `evaluate` 公开入口固定使用 `REQUEST_TIMEOUT_SECS`。
///
/// 超时映射：`Err(Elapsed)` → `ApiError::service_unavailable("evaluate", Some(secs))`
/// （HTTP 503 / MCP SERVICE_UNAVAILABLE，与 `CalcError::Timeout` 路径一致）。
/// spawn_blocking panic：保留原 `internal_with_source` 500 路径（不脱敏，因为
/// `ApiError::Internal` 序列化时 `source` 字段 `#[serde(skip)]`，客户端仅见通用消息）。
pub(crate) async fn evaluate_with_timeout(
    req: EvaluateRequest,
    timeout: Duration,
) -> Result<EvaluateResponse, ApiError> {
    req.validate()?;
    let ctx = req.to_eval_context();
    let precision = req.precision;
    let expr = req.expr.clone();
    // spawn_blocking 把同步 evaluate（内部 CacheManager 用 block_on 调 oxcache async API）
    // 移到无 runtime context 的阻塞线程池，避免 "Cannot start a runtime from within a runtime"。
    let join_handle = tokio::task::spawn_blocking(move || {
        let cache = shared_cache();
        eval_expr(&expr, &ctx, precision, cache)
    });
    // BUG-S-M-001: 请求级超时兜底，覆盖 spawn_blocking 启动 / evaluate 内部任何意外延迟。
    // 计算层 Alarm 已在 evaluate 内部精确中断循环，此处仅作慢攻击防御。
    let (result, domain, cache_hit, fmt_prec) = match tokio::time::timeout(timeout, join_handle).await {
        Ok(Ok(r)) => r,
        Ok(Err(join_err)) => {
            return Err(ApiError::internal_with_source(
                "evaluate task failed",
                "spawn_blocking",
                join_err,
            ))
        }
        Err(_) => {
            return Err(ApiError::service_unavailable(
                "evaluate",
                Some(timeout.as_secs()),
            ))
        }
    }
    .map_err(calc_error_to_api_error)?;
    Ok(EvaluateResponse::from_eval(
        result, domain, cache_hit, fmt_prec,
    ))
}

/// 将 `CalcError` 映射为 sdforge 标准 `ApiError`（spec.md R-sdforge-002/003 错误契约）。
///
/// 映射表（design.md D2）：
/// - `Timeout` → `ApiError::service_unavailable("evaluate", Some(30))`（HTTP 503 / MCP SERVICE_UNAVAILABLE）
/// - 其余 9 种 `ErrorKind`（Parse/Eval/Overflow/DivisionByZero/Domain/Depth/NaNOrInf/
///   UndefinedSymbol/Usage）→ `ApiError::invalid_input("{Kind}: {message}", None, None)`
///   （HTTP 400 / MCP INVALID_INPUT）
///
/// `kind` 名称作为 `message` 前缀保留可诊断性（spec.md R-sdforge-002 要求 message 含 kind 前缀，
/// 如 `"DivisionByZero: ..."`）；原始 message 追加其后（规则12 失败显性化）。
pub fn calc_error_to_api_error(e: CalcError) -> ApiError {
    match e.kind {
        ErrorKind::Timeout => ApiError::service_unavailable("evaluate", Some(30)),
        kind => ApiError::invalid_input(
            format!("{}: {}", error_kind_prefix(kind), e.message),
            None,
            None,
        ),
    }
}

/// `ErrorKind` → 协议层诊断前缀（变体名字面量，作为 `ApiError::InvalidInput` message 前缀保留可诊断性）。
fn error_kind_prefix(kind: ErrorKind) -> &'static str {
    match kind {
        ErrorKind::Parse => "Parse",
        ErrorKind::Eval => "Eval",
        ErrorKind::Overflow => "Overflow",
        ErrorKind::DivisionByZero => "DivisionByZero",
        ErrorKind::Domain => "Domain",
        ErrorKind::Depth => "Depth",
        ErrorKind::NaNOrInf => "NaNOrInf",
        ErrorKind::UndefinedSymbol => "UndefinedSymbol",
        ErrorKind::Timeout => "Timeout",
        ErrorKind::Usage => "Usage",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 8 种计算错误变体 → InvalidInput(400)，message 含 `"{Kind}:"` 前缀，field/value 为 None。
    #[test]
    fn calc_error_to_api_error_compute_kinds_map_to_invalid_input() {
        let cases: [(&str, CalcError); 8] = [
            ("Parse", CalcError::parse("unexpected token '@'")),
            ("Eval", CalcError::eval("domain violation")),
            ("Overflow", CalcError::overflow()),
            ("DivisionByZero", CalcError::division_by_zero()),
            ("Domain", CalcError::domain("sqrt of negative")),
            ("Depth", CalcError::depth_exceeded()),
            ("NaNOrInf", CalcError::nan_or_inf()),
            ("UndefinedSymbol", CalcError::undefined_symbol("foo")),
        ];
        for (prefix, err) in cases {
            let api = calc_error_to_api_error(err);
            match api {
                ApiError::InvalidInput {
                    message,
                    field,
                    value,
                } => {
                    let expected = format!("{}:", prefix);
                    assert!(
                        message.starts_with(&expected),
                        "kind {prefix:?}: message {message:?} 应以前缀 {expected:?} 开头"
                    );
                    assert!(field.is_none(), "kind {prefix:?}: field 应为 None");
                    assert!(value.is_none(), "kind {prefix:?}: value 应为 None");
                }
                other => panic!("kind {prefix:?}: 期望 InvalidInput，得到 {other:?}"),
            }
        }
    }

    /// `Usage` → InvalidInput(400)，message 前缀 `"Usage:"` 且保留原始 message。
    #[test]
    fn calc_error_to_api_error_usage_maps_to_invalid_input() {
        let api = calc_error_to_api_error(CalcError::usage("invalid --var"));
        match api {
            ApiError::InvalidInput { message, .. } => {
                assert!(message.starts_with("Usage:"));
                assert!(message.contains("invalid --var"));
            }
            other => panic!("期望 InvalidInput，得到 {other:?}"),
        }
    }

    /// `Timeout` → ServiceUnavailable(503)，service="evaluate"，retry_after=30。
    #[test]
    fn calc_error_to_api_error_timeout_maps_to_service_unavailable() {
        let api = calc_error_to_api_error(CalcError::timeout());
        match api {
            ApiError::ServiceUnavailable {
                service,
                retry_after,
                ..
            } => {
                assert_eq!(service, "evaluate");
                assert_eq!(retry_after, Some(30));
            }
            other => panic!("期望 ServiceUnavailable，得到 {other:?}"),
        }
    }

    /// 原始 message 必须完整保留在 ApiError.message（规则12 失败显性化回归）。
    #[test]
    fn calc_error_to_api_error_preserves_original_message() {
        let api = calc_error_to_api_error(CalcError::parse("the @ token is bad"));
        match api {
            ApiError::InvalidInput { message, .. } => {
                assert!(message.starts_with("Parse:"));
                assert!(message.contains("the @ token is bad"));
            }
            _ => panic!("期望 InvalidInput"),
        }
    }

    // === BUG-S-M-001: 请求级超时（防止慢攻击 / slowloris）===
    // evaluate_with_timeout 是 evaluate 的可测试入口，注入短超时验证 503 路径。

    /// 简单表达式 + 充足超时 → 成功返回 EvaluateResponse（基线）。
    /// 不验证 cache 状态（可能被前序测试缓存命中），仅验证成功返回 + 域名正确。
    #[tokio::test]
    async fn test_evaluate_with_timeout_succeeds_on_simple_expr() {
        let req = EvaluateRequest {
            expr: "2+3".into(),
            vars: std::collections::HashMap::new(),
            precision: None,
        };
        let result = evaluate_with_timeout(req, Duration::from_secs(5)).await;
        assert!(result.is_ok(), "simple expr should succeed within 5s");
        let resp = result.unwrap();
        assert_eq!(resp.domain, "arithmetic");
        // cache 字段为 "hit" 或 "miss"（取决于前序测试是否已缓存该表达式）
        assert!(resp.cache == "hit" || resp.cache == "miss");
    }

    /// 零超时（1ns）→ 立即返回 ServiceUnavailable（503）。
    /// 使用 `precision(10000, factorial(10000))`（CPU 密集计算）确保 1ns 内不可能完成，
    /// 避免简单表达式在缓存命中时 1ns 内完成导致测试不稳定。
    #[tokio::test]
    async fn test_evaluate_with_timeout_returns_service_unavailable_on_zero_timeout() {
        let req = EvaluateRequest {
            expr: "precision(10000, factorial(10000))".into(),
            vars: std::collections::HashMap::new(),
            precision: None,
        };
        let result = evaluate_with_timeout(req, Duration::from_nanos(1)).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            ApiError::ServiceUnavailable { service, .. } => {
                assert_eq!(service, "evaluate");
            }
            other => panic!("期望 ServiceUnavailable，得到 {other:?}"),
        }
    }

    /// 真实慢表达式 + 短超时 → ServiceUnavailable（503）。
    /// `factorial(10000)` 在 precision domain 内触发 CPU 密集计算（大整数阶乘 + 高精度格式化）。
    /// timeout=1ms 确保稳定性（不依赖机器绝对性能，只需 spawn_blocking 启动 + 计算超过 1ms）。
    #[tokio::test]
    async fn test_evaluate_with_timeout_returns_service_unavailable_on_slow_expr() {
        let req = EvaluateRequest {
            expr: "precision(10000, factorial(10000))".into(),
            vars: std::collections::HashMap::new(),
            precision: None,
        };
        let result = evaluate_with_timeout(req, Duration::from_millis(1)).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            ApiError::ServiceUnavailable { service, .. } => {
                assert_eq!(service, "evaluate");
            }
            other => panic!("期望 ServiceUnavailable（慢表达式超时），得到 {other:?}"),
        }
    }

    /// validate() 失败 → 立即返回 ValidationError，不进入 spawn_blocking / timeout 路径。
    #[tokio::test]
    async fn test_evaluate_with_timeout_validates_before_timeout() {
        let req = EvaluateRequest {
            expr: String::new(),
            vars: std::collections::HashMap::new(),
            precision: None,
        };
        let result = evaluate_with_timeout(req, Duration::from_secs(5)).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ApiError::ValidationError { .. }
        ));
    }

    /// REQUEST_TIMEOUT_SECS 常量必须为 30（spec.md R-sdforge-002 契约）。
    #[test]
    fn test_request_timeout_secs_is_30() {
        assert_eq!(REQUEST_TIMEOUT_SECS, 30);
    }
}
