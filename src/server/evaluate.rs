// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! Server evaluate 接口层：`#[forge]` 声明式封装 + `CalcError` → `ApiError` 映射。
//!
//! 单个 `#[forge]` async fn 同时生成 HTTP 路由与 MCP tool，
//! 取代手写的 `inventory::submit!` + `SdForgeTool` 封装 + `preserve_*` 链接器 hack。
//! 链接器 inventory 保留由 `sdforge::init_all_plugins()` 托管（http.rs/mcp.rs 调用）。
use super::lang::resolve_i18n;
use super::shared_cache;
use super::{EvaluateRequest, EvaluateResponse};
use crate::core::{CalcError, ErrorKind};
use crate::evaluate as eval_expr;
use crate::i18n::I18n;
use sdforge::error::ApiError;
use sdforge::forge;
use std::time::Duration;

/// 请求级超时（秒）：防止慢攻击（slowloris）与无限循环表达式耗尽连接资源。
///
/// 安全约束：HTTP/MCP 请求必须在合理时间内返回（成功或 503）。
/// 与 `CalcError::Timeout`（计算层超时，由 evaluate 内部 Alarm 控制）互补：
/// - 计算层超时：精确中断 evaluate 内部的循环
/// - 请求级超时：兜底保护，覆盖 spawn_blocking 启动 / 缓存写入等任何意外延迟
pub const REQUEST_TIMEOUT_SECS: u64 = 30;

/// `POST /api/v1/evaluate` + MCP `evaluate` tool（单函数双协议）。
///
/// `#[forge]` 宏自动生成 axum handler（消费 `EvaluateRequest` body + `ApiError::into_response`
/// 错误路径）与 MCP tool struct/schema（input_schema 从 `EvaluateRequest` 字段推导），
/// 并 `inventory::submit!` 注册。`name="evaluate"` + `version=1` 决定路径前缀 `/api/v1`，
/// 叠加 `path="/evaluate"` → 最终路由 `/api/v1/evaluate`。
///
/// 函数体内保留 `spawn_blocking` 隔离同步求值（避免阻塞 tokio 异步运行时）：
/// `#[forge]` 只生成外层路由/tool 外壳，函数体仍是 CalNexus 代码，隔离策略不变。
///
/// 用 `tokio::time::timeout` 包裹 `spawn_blocking`，超时返回 503
/// ServiceUnavailable（`retry_after=REQUEST_TIMEOUT_SECS`），防止慢攻击。
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
    // 语言协商：请求级 lang 字段 → I18n（缺省/未知值回退英文，见 lang::resolve_i18n）
    let i18n = resolve_i18n(req.lang.as_deref());
    let ctx = req.to_eval_context();
    let precision = req.precision;
    let expr = req.expr.clone();
    // spawn_blocking 把同步 evaluate（内部 CacheManager 为 oxcache 同步字节权重实现）
    // 移到无 runtime context 的阻塞线程池，避免 "Cannot start a runtime from within a runtime"。
    let join_handle = tokio::task::spawn_blocking(move || {
        let cache = shared_cache();
        eval_expr(&expr, &ctx, precision, cache)
    });
    // 请求级超时兜底，覆盖 spawn_blocking 启动 / evaluate 内部任何意外延迟。
    // 计算层 Alarm 已在 evaluate 内部精确中断循环，此处仅作慢攻击防御。
    let (result, domain, cache_hit, fmt_prec) =
        match tokio::time::timeout(timeout, join_handle).await {
            Ok(Ok(r)) => r,
            Ok(Err(join_err)) => {
                // 500 文案迁入 FTL 目录经请求级 i18n 输出
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

/// 将 `CalcError` 映射为 sdforge 标准 `ApiError`（错误契约）。
///
/// 映射表：
/// - `Timeout` → `ApiError::service_unavailable("evaluate", Some(30))`（HTTP 503 / MCP SERVICE_UNAVAILABLE）
/// - 其余 9 种 `ErrorKind`（Parse/Eval/Overflow/DivisionByZero/Domain/Depth/NaNOrInf/
///   UndefinedSymbol/Usage）→ `ApiError::invalid_input("{Kind}: {message}", None, None)`
///   （HTTP 400 / MCP INVALID_INPUT）
///
/// `kind` 名称作为 `message` 前缀保留可诊断性（要求 message 含 kind 前缀，
/// 如 `"DivisionByZero: ..."`）；原始 message 追加其后（失败显性化）。
pub fn calc_error_to_api_error(e: CalcError) -> ApiError {
    calc_error_to_api_error_i18n(e, &I18n::default())
}

/// 语言感知版 `CalcError` → `ApiError` 映射（语言协商核心）。
///
/// - `i18n` 为英文（缺省/显式 `lang=en`）：输出与 [`calc_error_to_api_error`] 逐字节一致
///   （`"{Kind}: {message}"`，机器契约不变）。
/// - `i18n` 为中文（请求 `lang=zh`）：message 变为 `"{本地化 kind 标签}: {本地化 detail}"`，
///   detail 优先用 `i18n_key`+`i18n_args` 目录渲染，无键时回退原始英文 message
///   （与 CLI `friendly()` 的双重模式一致）。
/// - 503 路径（Timeout/DependencyUnavailable）不受语言影响（`service`/`retry_after` 为协议字段）。
pub fn calc_error_to_api_error_i18n(e: CalcError, i18n: &I18n) -> ApiError {
    match e.kind {
        ErrorKind::Timeout => ApiError::service_unavailable("evaluate", Some(REQUEST_TIMEOUT_SECS)),
        // 上游依赖故障 → 503（不再误标为客户端 400）
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

/// 按协商语言渲染 `CalcError` 的 detail 部分（不含 kind 前缀）。
///
/// 英文（缺省）路径返回原始 `message`（逐字节契约）；中文路径优先用
/// `i18n_key`+`i18n_args` 目录渲染，无键时回退原始英文 message。
/// 供 fx 工具等无 kind 前缀的错误映射点复用。
pub(crate) fn localized_error_detail(e: &CalcError, i18n: &I18n) -> String {
    if i18n.lang() == crate::i18n::Lang::Zh {
        match e.i18n_key {
            Some(key) => {
                let args_ref: Vec<(&str, &str)> = e
                    .i18n_args
                    .iter()
                    .map(|(k, v)| (k.as_str(), v.as_str()))
                    .collect();
                i18n.tf(key, &args_ref)
            }
            None => e.message.clone(),
        }
    } else {
        e.message.clone()
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
    // evaluate_with_timeout 是 evaluate 的可测试入口，注入短超时验证 503 路径。

    /// 简单表达式 + 充足超时 → 成功返回 EvaluateResponse（基线）。
    /// 不验证 cache 状态（可能被前序测试缓存命中），仅验证成功返回 + 域名正确。
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

    /// 真实慢表达式 + 短超时 → ServiceUnavailable（503）。
    /// `factorial(10000)` 在 precision domain 内触发 CPU 密集计算（大整数阶乘 + 高精度格式化）。
    /// timeout=1ms 确保稳定性（不依赖机器绝对性能，只需 spawn_blocking 启动 + 计算超过 1ms）。
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
            let via_i18n = calc_error_to_api_error_i18n(e, &I18n::default());
            match (legacy, via_i18n) {
                (
                    ApiError::InvalidInput { message: m1, .. },
                    ApiError::InvalidInput { message: m2, .. },
                ) => assert_eq!(m1, m2),
                (a, b) => panic!("变体不一致: {a:?} vs {b:?}"),
            }
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

    /// 请求 lang=None 端到端：错误 message 语言跟随系统语言检测链
    /// （「缺省即英文」契约变更为「缺省即检测」；显式 lang=en 的
    /// 英文机器契约由 calc_error_to_api_error_i18n_en_matches_legacy 覆盖）。
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
