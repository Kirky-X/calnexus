// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! wasm-bindgen 导出层：`evaluate` 与 `list_functions`（仅 wasm32 + `wasm-api`）。
//!
//! 契约：成功 `{"v":1,"result":...}`；失败 `Err(JsValue)`，payload 为
//! `{"v":1,"error":{"code","kind","message"}}`，`code` 沿用 CLI 退出码语义。

#[cfg(not(target_arch = "wasm32"))]
compile_error!("wasm-api is only supported on the wasm32 target");

use wasm_bindgen::prelude::*;

use crate::core::{CacheManager, CalcError, ErrorKind, EvalContext};

/// 错误类别的 snake_case 名称（与 `ErrorKind` 变体一一对应）。
fn kind_name(kind: ErrorKind) -> &'static str {
    match kind {
        ErrorKind::Parse => "parse",
        ErrorKind::Eval => "eval",
        ErrorKind::Overflow => "overflow",
        ErrorKind::DivisionByZero => "division_by_zero",
        ErrorKind::Domain => "domain",
        ErrorKind::Depth => "depth",
        ErrorKind::NaNOrInf => "nan_or_inf",
        ErrorKind::UndefinedSymbol => "undefined_symbol",
        ErrorKind::Timeout => "timeout",
        ErrorKind::Usage => "usage",
        ErrorKind::DependencyUnavailable => "dependency_unavailable",
    }
}

/// 构造错误 payload（JSON 字符串）。
fn error_payload(code: u8, kind: &str, message: &str) -> String {
    crate::core::sort_json_keys(serde_json::json!({
        "v": 1,
        "error": {
            "code": code,
            "kind": kind,
            "message": message,
        }
    }))
    .to_string()
}

/// 将 `CalcError` 转为 JS 侧可解析的 JSON 错误。
fn to_js_error(err: &CalcError) -> JsValue {
    let code = err.kind.exit_code() as u8;
    JsValue::from_str(&error_payload(code, kind_name(err.kind), &err.to_string()))
}

/// 进程级 L1 缓存（wasm 单线程，`OnceLock` 共享）。
fn shared_cache() -> &'static CacheManager {
    static CACHE: std::sync::OnceLock<CacheManager> = std::sync::OnceLock::new();
    CACHE.get_or_init(CacheManager::new)
}

/// wasm 求值入口：返回 `{"v":1,"result":...}`。
#[wasm_bindgen]
pub fn evaluate(expr: &str) -> Result<String, JsValue> {
    if expr.trim().is_empty() {
        return Err(JsValue::from_str(&error_payload(
            2,
            "usage",
            "expression must not be empty",
        )));
    }
    match crate::core::evaluate(expr, &EvalContext::new(), None, shared_cache()) {
        Ok((result, _, _, _)) => {
            let payload = crate::core::sort_json_keys(serde_json::json!({
                "v": 1,
                "result": result
            }));
            Ok(payload.to_string())
        }
        Err(err) => Err(to_js_error(&err)),
    }
}

/// 运行时函数目录：`[{"domain":"...","functions":[...]}]`。
#[wasm_bindgen]
pub fn list_functions() -> String {
    let entries: Vec<serde_json::Value> = crate::function_catalog::DOMAIN_FUNCTIONS
        .iter()
        .map(|(domain, functions)| {
            serde_json::json!({
                "domain": domain,
                "functions": functions,
            })
        })
        .collect();
    serde_json::to_string(&entries).unwrap_or_else(|_| "[]".to_string())
}
