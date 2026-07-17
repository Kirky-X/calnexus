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

/// `POST /api/v1/evaluate` + MCP `evaluate` tool（单函数双协议）。
///
/// `#[forge]` 宏自动生成 axum handler（消费 `EvaluateRequest` body + `ApiError::into_response`
/// 错误路径）与 MCP tool struct/schema（input_schema 从 `EvaluateRequest` 字段推导），
/// 并 `inventory::submit!` 注册。`name="evaluate"` + `version="v1"` 决定路径前缀 `/api/v1`，
/// 叠加 `path="/evaluate"` → 最终路由 `/api/v1/evaluate`（spec.md R-sdforge-002）。
///
/// 函数体内保留 `spawn_blocking` 隔离 oxcache 同步 `block_on`（与 p1 手写 handler 等价）：
/// `#[forge]` 只生成外层路由/tool 外壳，函数体仍是 CalNexus 代码，隔离策略不变。
#[forge(
    name = "evaluate",
    version = "v1",
    path = "/evaluate",
    method = "POST",
    tool_name = "evaluate",
    description = "Evaluate a math expression. Returns {result, domain, cache}. Supports vars and precision."
)]
pub(crate) async fn evaluate(req: EvaluateRequest) -> Result<EvaluateResponse, ApiError> {
    req.validate()?;
    let ctx = req.to_eval_context();
    let precision = req.precision;
    let expr = req.expr.clone();
    // spawn_blocking 把同步 evaluate（内部 CacheManager 用 block_on 调 oxcache async API）
    // 移到无 runtime context 的阻塞线程池，避免 "Cannot start a runtime from within a runtime"。
    let (result, domain, cache_hit, fmt_prec) = tokio::task::spawn_blocking(move || {
        let cache = shared_cache();
        eval_expr(&expr, &ctx, precision, cache)
    })
    .await
    .map_err(|e| ApiError::internal_with_source("evaluate task failed", "spawn_blocking", e))?
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

/// `ErrorKind` → 协议层诊断前缀（变体名字面量，与旧 `ErrorResponse.kind` 的 `{:?}` 格式一致）。
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
}
