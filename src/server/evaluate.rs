// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! Server evaluate 接口层：`#[forge]` 声明式封装 + `CalcError` → `ApiError` 映射。
//!
//! 单个 `#[forge]` async fn 同时生成 HTTP 路由与 MCP tool（inventory 保留由 `init_all_plugins()` 托管）。
use super::lang::resolve_i18n;
use super::shared_cache;
use super::{EvaluateRequest, EvaluateResponse};
use crate::core::{CalcError, ErrorKind};
use crate::evaluate as eval_expr;
use crate::i18n::I18n;
use sdforge::error::ApiError;
use sdforge::forge;
use std::time::Duration;

/// 请求级超时秒数：慢攻击兜底（计算层超时由 evaluate 内部 Alarm 控制）。
pub const REQUEST_TIMEOUT_SECS: u64 = 30;

/// `POST /api/v1/evaluate` + MCP `evaluate` tool（`#[forge]` 生成路由与 schema）。
///
/// 超时返回 503 ServiceUnavailable（`retry_after=REQUEST_TIMEOUT_SECS`）。
#[forge(
    name = "evaluate",
    version = 1,
    path = "/evaluate",
    method = "POST",
    tool_name = "evaluate",
    description = "Evaluate a math expression. Args MUST be wrapped in a req object: req.expr is the expression string, req.vars holds optional variable bindings, req.precision is optional, req.lang is optional response language (BCP-47 tag, 'en' default, 'zh' supported). Limits: expr up to 4096 chars, vars up to 1024 keys, precision up to 10000. Returns result, domain, cache. Errors: 400 InvalidInput (syntax/eval), 422 ValidationError (limit exceeded), 503 ServiceUnavailable (timeout/upstream, includes Retry-After)."
)]
pub(crate) async fn evaluate(req: EvaluateRequest) -> Result<EvaluateResponse, ApiError> {
    evaluate_with_timeout(req, Duration::from_secs(REQUEST_TIMEOUT_SECS)).await
}

/// 带超时的 evaluate 实现（可测试入口）：超时→503，spawn_blocking panic→500 internal。
pub(crate) async fn evaluate_with_timeout(
    req: EvaluateRequest,
    timeout: Duration,
) -> Result<EvaluateResponse, ApiError> {
    req.validate()?;
    // 语言协商：req.lang → I18n（缺省/未知值回退英文）
    let i18n = resolve_i18n(req.lang.as_deref());
    let ctx = req.to_eval_context();
    let precision = req.precision;
    let expr = req.expr.clone();
    // spawn_blocking 隔离同步求值，避免 "Cannot start a runtime from within a runtime"
    let join_handle = tokio::task::spawn_blocking(move || {
        let cache = shared_cache();
        eval_expr(&expr, &ctx, precision, cache)
    });
    // 请求级超时兜底：覆盖 spawn_blocking 启动等意外延迟（慢攻击防御）
    let (result, domain, cache_hit, fmt_prec) =
        match tokio::time::timeout(timeout, join_handle).await {
            Ok(Ok(r)) => r,
            Ok(Err(join_err)) => {
                // 500 文案经请求级 i18n 输出
                return Err(ApiError::internal_with_source(
                    i18n.t("server.evaluate_task_failed"),
                    "spawn_blocking",
                    join_err,
                ));
            }
            Err(_) => {
                return Err(ApiError::service_unavailable(
                    "evaluate",
                    Some(timeout.as_secs()),
                ));
            }
        }
        .map_err(|e| calc_error_to_api_error_i18n(e, &i18n))?;
    Ok(EvaluateResponse::from_eval(
        result, domain, cache_hit, fmt_prec,
    ))
}

/// `CalcError` → `ApiError` 标准映射（恒英文）：Timeout/DependencyUnavailable→503，其余→400。
impl From<CalcError> for ApiError {
    fn from(e: CalcError) -> Self {
        match e.kind {
            ErrorKind::Timeout | ErrorKind::DependencyUnavailable => {
                ApiError::service_unavailable("evaluate", Some(REQUEST_TIMEOUT_SECS))
            }
            kind => ApiError::invalid_input(
                format!("{}: {}", error_kind_prefix(kind), e.message),
                None,
                None,
            ),
        }
    }
}

/// `CalcError` → `ApiError` 薄别名（经 `From` 转发），与标准映射逐字节一致。
pub fn calc_error_to_api_error(e: CalcError) -> ApiError {
    ApiError::from(e)
}

/// 语言感知映射：en 与标准映射逐字节一致，zh 本地化 message，503 路径不受语言影响。
pub fn calc_error_to_api_error_i18n(e: CalcError, i18n: &I18n) -> ApiError {
    match e.kind {
        ErrorKind::Timeout => ApiError::service_unavailable("evaluate", Some(REQUEST_TIMEOUT_SECS)),
        // 上游依赖故障 → 503
        ErrorKind::DependencyUnavailable => {
            ApiError::service_unavailable("evaluate", Some(REQUEST_TIMEOUT_SECS))
        }
        kind => {
            let message = if i18n.lang() == crate::i18n::Lang::Zh {
                format!(
                    "{}: {}",
                    i18n.t(kind.i18n_key()),
                    localized_error_detail(&e, i18n)
                )
            } else {
                format!("{}: {}", error_kind_prefix(kind), e.message)
            };
            ApiError::invalid_input(message, None, None)
        }
    }
}

/// 渲染 `CalcError` detail（不含 kind 前缀）：zh 优先 `i18n_key` 目录，无键回退英文 message。
pub(crate) fn localized_error_detail(e: &CalcError, i18n: &I18n) -> String {
    if i18n.lang() == crate::i18n::Lang::Zh {
        e.localized_detail(i18n)
    } else {
        e.message.clone()
    }
}

/// `ErrorKind` → 协议层诊断前缀（作为 `ApiError::InvalidInput` message 前缀）。
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
        ErrorKind::DependencyUnavailable => "DependencyUnavailable",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// DependencyUnavailable 映射 503 ServiceUnavailable
    #[test]
    fn calc_error_to_api_error_dependency_unavailable_maps_to_503() {
        let e = CalcError::dependency_unavailable("FX upstream unreachable");
        let api_err = calc_error_to_api_error(e);
        assert!(
            matches!(api_err, sdforge::error::ApiError::ServiceUnavailable { .. }),
            "DependencyUnavailable 应映射 ServiceUnavailable/503，实际 {:?}",
            api_err
        );
    }

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

    /// 原始 message 必须完整保留在 ApiError.message（失败显性化回归）。
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

    // === 请求级超时（防止慢攻击 / slowloris）===
    // 注入短超时验证 503 路径。

    /// 简单表达式 + 充足超时 → 成功返回 EvaluateResponse（不验证 cache 状态）。
    #[tokio::test]
    async fn test_evaluate_with_timeout_succeeds_on_simple_expr() {
        let req = EvaluateRequest {
            expr: "2+3".into(),
            vars: std::collections::HashMap::new(),
            precision: None,
            lang: None,
        };
        let result = evaluate_with_timeout(req, Duration::from_secs(5)).await;
        assert!(result.is_ok(), "simple expr should succeed within 5s");
        let resp = result.unwrap();
        assert_eq!(resp.domain, "arithmetic");
        // cache 状态取决于前序测试是否已缓存该表达式
        assert!(resp.cache == "hit" || resp.cache == "miss");
    }

    /// 零超时（1ns）→ 503；用 CPU 密集表达式避免缓存命中致测试不稳定。
    #[tokio::test]
    async fn test_evaluate_with_timeout_returns_service_unavailable_on_zero_timeout() {
        let req = EvaluateRequest {
            expr: "precision(10000, factorial(10000))".into(),
            vars: std::collections::HashMap::new(),
            precision: None,
            lang: None,
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

    /// 慢表达式 + 超时 1ms → 503（不依赖机器绝对性能）。
    #[tokio::test]
    async fn test_evaluate_with_timeout_returns_service_unavailable_on_slow_expr() {
        let req = EvaluateRequest {
            expr: "precision(10000, factorial(10000))".into(),
            vars: std::collections::HashMap::new(),
            precision: None,
            lang: None,
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
            lang: None,
        };
        let result = evaluate_with_timeout(req, Duration::from_secs(5)).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ApiError::ValidationError { .. }
        ));
    }

    /// REQUEST_TIMEOUT_SECS 常量必须为 30（契约）。
    #[test]
    fn test_request_timeout_secs_is_30() {
        assert_eq!(REQUEST_TIMEOUT_SECS, 30);
    }

    // === 语言协商（lang 请求字段）===

    /// 英文（缺省 I18n）路径与旧 `calc_error_to_api_error` 逐字节一致（机器契约）。
    #[test]
    fn calc_error_to_api_error_i18n_en_matches_legacy() {
        let cases = [
            CalcError::parse("unexpected token '@'"),
            CalcError::undefined_symbol("foo"),
            CalcError::domain("sqrt of negative"),
            CalcError::division_by_zero(),
        ];
        for e in cases {
            let legacy = calc_error_to_api_error(e.clone());
            // 显式英文：I18n::default() 走 locale 检测，zh 系统下该测试会假失败
            let via_i18n = calc_error_to_api_error_i18n(e, &I18n::new(crate::i18n::Lang::En));
            match (legacy, via_i18n) {
                (
                    ApiError::InvalidInput { message: m1, .. },
                    ApiError::InvalidInput { message: m2, .. },
                ) => assert_eq!(m1, m2),
                (a, b) => panic!("变体不一致: {a:?} vs {b:?}"),
            }
        }
    }

    /// `From<CalcError> for ApiError` 覆盖全部 11 个 ErrorKind；薄别名与 From 输出一致。
    #[test]
    fn from_calc_error_covers_all_kinds() {
        // 503 契约
        let service_cases = [
            CalcError::timeout(),
            CalcError::dependency_unavailable("fx source down"),
        ];
        for e in service_cases {
            match ApiError::from(e) {
                ApiError::ServiceUnavailable { .. } => {}
                other => panic!("Timeout/DependencyUnavailable 应映射 503，实际 {other:?}"),
            }
        }
        // 400 契约（其余 9 个 kind，message 形如 "{Kind}: {message}"）
        let client_cases = [
            CalcError::parse("unexpected token"),
            CalcError::eval("bad expression"),
            CalcError::overflow(),
            CalcError::division_by_zero(),
            CalcError::domain("negative sqrt"),
            CalcError::depth_exceeded(),
            CalcError::nan_or_inf(),
            CalcError::undefined_symbol("foo"),
            CalcError::usage("bad flag"),
        ];
        for e in client_cases {
            match ApiError::from(e) {
                ApiError::InvalidInput { message, .. } => {
                    assert!(
                        message.contains(": "),
                        "message 应含 kind 前缀（Kind: message），实际 {:?}",
                        message
                    );
                }
                other => panic!("客户端错误 kind 应映射 400，实际 {other:?}"),
            }
        }
        // 薄别名与 From 逐字节一致（Debug 表示比对）
        for e in [CalcError::domain("x"), CalcError::timeout()] {
            assert_eq!(
                format!("{:?}", calc_error_to_api_error(e.clone())),
                format!("{:?}", ApiError::from(e)),
            );
        }
    }

    /// lang=zh：message 切换为「本地化 kind 标签: 本地化 detail」（undefined_symbol 走参数化目录键）。
    #[test]
    fn calc_error_to_api_error_i18n_zh_localizes_message() {
        let i18n = resolve_i18n(Some("zh-CN"));
        let api = calc_error_to_api_error_i18n(CalcError::undefined_symbol("foo"), &i18n);
        match api {
            ApiError::InvalidInput { message, .. } => {
                assert!(
                    message.contains("未定义符号") && message.contains("foo"),
                    "zh message 应为本地化标签 + 参数化 detail，实际 {message:?}"
                );
                assert!(
                    !message.contains("Undefined"),
                    "zh 路径不应保留英文 kind 前缀"
                );
            }
            other => panic!("期望 InvalidInput，得到 {other:?}"),
        }
    }

    /// lang=zh + 无 i18n_key 的错误：detail 回退原始英文 message（fail-loud 契约）。
    #[test]
    fn calc_error_to_api_error_i18n_zh_falls_back_to_raw_message_without_key() {
        let i18n = resolve_i18n(Some("zh"));
        let api = calc_error_to_api_error_i18n(CalcError::parse("raw english detail"), &i18n);
        match api {
            ApiError::InvalidInput { message, .. } => {
                assert!(
                    message.contains("解析错误") && message.contains("raw english detail"),
                    "zh kind 标签 + 英文 detail 回退，实际 {message:?}"
                );
            }
            other => panic!("期望 InvalidInput，得到 {other:?}"),
        }
    }

    /// 503 路径不受语言协商影响（service/retry_after 为协议字段）。
    #[test]
    fn calc_error_to_api_error_i18n_zh_keeps_timeout_contract() {
        let i18n = resolve_i18n(Some("zh"));
        let api = calc_error_to_api_error_i18n(CalcError::timeout(), &i18n);
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

    /// 请求 lang=zh 端到端：未定义变量的错误 message 为中文。
    #[tokio::test]
    async fn test_evaluate_with_timeout_zh_lang_localizes_error() {
        let req = EvaluateRequest {
            expr: "foo + 1".into(),
            vars: std::collections::HashMap::new(),
            precision: None,
            lang: Some("zh".into()),
        };
        let err = evaluate_with_timeout(req, Duration::from_secs(5))
            .await
            .expect_err("undefined symbol must error");
        match err {
            ApiError::InvalidInput { message, .. } => {
                assert!(
                    message.contains("求值错误")
                        && message.contains("未绑定变量")
                        && message.contains("foo"),
                    "zh message 应为本地化标签 + 参数化 detail，实际 {message:?}"
                );
            }
            other => panic!("期望 InvalidInput，得到 {other:?}"),
        }
    }

    /// 请求 lang=None 端到端：错误文案语言跟随系统检测（缺省即检测，非固定英文）。
    #[tokio::test]
    async fn test_evaluate_with_timeout_default_lang_follows_detection() {
        let expect_zh = crate::i18n::detect_locale() == crate::i18n::Lang::Zh;
        let req = EvaluateRequest {
            expr: "foo + 1".into(),
            vars: std::collections::HashMap::new(),
            precision: None,
            lang: None,
        };
        let err = evaluate_with_timeout(req, Duration::from_secs(5))
            .await
            .expect_err("undefined symbol must error");
        match err {
            ApiError::InvalidInput { message, .. } => {
                if expect_zh {
                    assert!(
                        message.contains("求值错误") && message.contains("未绑定变量"),
                        "检测为 zh 时缺省 lang 应输出中文，实际 {message:?}"
                    );
                } else {
                    assert!(
                        message.starts_with("Eval:") && message.contains("unbound variable: foo"),
                        "检测为 en 时缺省 lang 应保持英文契约，实际 {message:?}"
                    );
                }
            }
            other => panic!("期望 InvalidInput，得到 {other:?}"),
        }
    }
}
