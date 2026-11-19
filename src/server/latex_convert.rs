// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! Server to_latex 工具：表达式 AST → LaTeX 的 HTTP / MCP 双协议入口。

use super::evaluate::REQUEST_TIMEOUT_SECS;
use super::lang::resolve_i18n;
use super::types::MAX_EXPR_LEN;
use super::{calc_error_to_api_error_i18n, shared_cache};
use crate::core::parse;
use crate::output::format_latex_ast;
use sdforge::error::ApiError;
use sdforge::forge;
use std::time::Duration;

/// `POST /api/v1/to_latex` + MCP `to_latex` 请求。
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ToLatexRequest {
    pub expr: String,
    #[serde(default)]
    pub lang: Option<String>,
}

/// `to_latex` 响应。
#[derive(Debug, Clone, serde::Serialize)]
pub struct ToLatexResponse {
    pub v: u32,
    pub latex: String,
}

impl ToLatexRequest {
    pub fn validate(&self) -> Result<(), ApiError> {
        let i18n = resolve_i18n(self.lang.as_deref());
        if self.expr.is_empty() {
            return Err(ApiError::validation(
                "expr",
                i18n.t("server.validation.expr_empty"),
            ));
        }
        if self.expr.len() > MAX_EXPR_LEN {
            return Err(ApiError::validation(
                "expr",
                i18n.tf(
                    "server.validation.expr_too_long",
                    &[
                        ("len", &self.expr.len().to_string()),
                        ("max", &MAX_EXPR_LEN.to_string()),
                    ],
                ),
            ));
        }
        if self.expr.bytes().any(|b| b == 0) {
            return Err(ApiError::validation(
                "expr",
                i18n.t("server.validation.expr_null_bytes"),
            ));
        }
        Ok(())
    }
}

/// `POST /api/v1/to_latex` + MCP `to_latex`：返回表达式的 LaTeX 表示。
#[forge(
    name = "to_latex",
    version = 1,
    path = "/to_latex",
    method = "POST",
    tool_name = "to_latex",
    description = "Convert a math expression to LaTeX. Args MUST be wrapped in a req object: req.expr is the expression string, req.lang is optional response language. Returns the LaTeX string. Errors: 400 InvalidInput, 422 ValidationError, 503 unavailable on timeout.",
    i18n_key = "endpoint.to_latex.description"
)]
pub(crate) async fn to_latex(req: ToLatexRequest) -> Result<ToLatexResponse, ApiError> {
    to_latex_with_timeout(req, Duration::from_secs(REQUEST_TIMEOUT_SECS)).await
}

/// 带超时的实现（可测试入口）。
pub(crate) async fn to_latex_with_timeout(
    req: ToLatexRequest,
    timeout: Duration,
) -> Result<ToLatexResponse, ApiError> {
    let i18n = resolve_i18n(req.lang.as_deref());
    req.validate()?;
    let expr = req.expr.clone();
    let _cache = shared_cache();
    let handle = tokio::task::spawn_blocking(move || {
        let ast = parse(&expr)?;
        format_latex_ast(&ast)
    });
    let latex = match tokio::time::timeout(timeout, handle).await {
        Ok(Ok(result)) => result,
        Ok(Err(join_err)) => {
            return Err(ApiError::internal_with_source(
                i18n.t("server.evaluate_task_failed"),
                "spawn_blocking",
                join_err,
            ));
        }
        Err(_) => {
            return Err(ApiError::service_unavailable(
                "to_latex",
                Some(timeout.as_secs()),
            ));
        }
    }
    .map_err(|e| calc_error_to_api_error_i18n(e, &i18n))?;
    Ok(ToLatexResponse { v: 1, latex })
}

#[cfg(all(test, feature = "server"))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_to_latex_converts_polynomial() {
        let resp = to_latex_with_timeout(
            ToLatexRequest {
                expr: "x^2+1".to_string(),
                lang: Some("en".to_string()),
            },
            Duration::from_secs(5),
        )
        .await
        .unwrap();
        assert!(resp.latex.contains("^{2}"), "{}", resp.latex);
    }

    #[tokio::test]
    async fn test_to_latex_rejects_empty() {
        assert!(
            to_latex_with_timeout(
                ToLatexRequest {
                    expr: String::new(),
                    lang: None,
                },
                Duration::from_secs(5),
            )
            .await
            .is_err()
        );
    }
}
