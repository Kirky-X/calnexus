// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! Server lint 工具：表达式静态检查的 HTTP / MCP 双协议入口。

use super::evaluate::REQUEST_TIMEOUT_SECS;
use super::lang::resolve_i18n;
use super::types::MAX_EXPR_LEN;
use super::{calc_error_to_api_error_i18n, shared_cache};
use crate::core::parse;
use crate::math::lint::lint_expr;
use sdforge::error::ApiError;
use sdforge::forge;
use std::time::Duration;

/// `POST /api/v1/lint` + MCP `lint` 请求。
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct LintRequest {
    pub expr: String,
    #[serde(default)]
    pub lang: Option<String>,
}

/// 单条 lint 发现（协议 DTO，与纯函数层 code 对齐）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct LintFinding {
    pub code: String,
    pub message: String,
}

/// `lint` 响应。
#[derive(Debug, Clone, serde::Serialize)]
pub struct LintResponse {
    pub v: u32,
    pub findings: Vec<LintFinding>,
}

impl LintRequest {
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

/// `POST /api/v1/lint` + MCP `lint`：返回表达式静态检查发现。
#[forge(
    name = "lint",
    version = 1,
    path = "/lint",
    method = "POST",
    tool_name = "lint",
    description = "Lint a math expression for suspicious constructs. Args MUST be wrapped in a req object: req.expr is the expression string, req.lang is optional response language. Returns findings with stable codes (DIV_ZERO_LITERAL, LOG_NONPOSITIVE_LITERAL, SQRT_NEGATIVE_LITERAL, DEEP_NESTING). Errors: 400 InvalidInput, 422 ValidationError, 503 unavailable on timeout.",
    i18n_key = "endpoint.lint.description"
)]
pub(crate) async fn lint(req: LintRequest) -> Result<LintResponse, ApiError> {
    lint_with_timeout(req, Duration::from_secs(REQUEST_TIMEOUT_SECS)).await
}

/// 带超时的实现（可测试入口）。
pub(crate) async fn lint_with_timeout(
    req: LintRequest,
    timeout: Duration,
) -> Result<LintResponse, ApiError> {
    let i18n = resolve_i18n(req.lang.as_deref());
    req.validate()?;
    let expr = req.expr.clone();
    let _cache = shared_cache();
    let handle = tokio::task::spawn_blocking(move || {
        let ast = parse(&expr)?;
        Ok::<Vec<crate::math::lint::LintFinding>, crate::core::CalcError>(lint_expr(&ast))
    });
    let findings = match tokio::time::timeout(timeout, handle).await {
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
                "lint",
                Some(timeout.as_secs()),
            ));
        }
    }
    .map_err(|e| calc_error_to_api_error_i18n(e, &i18n))?;

    Ok(LintResponse {
        v: 1,
        findings: findings
            .into_iter()
            .map(|f| LintFinding {
                code: f.code.to_string(),
                message: f.message,
            })
            .collect(),
    })
}

#[cfg(all(test, feature = "server"))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_lint_div_zero_and_clean() {
        let bad = lint_with_timeout(
            LintRequest {
                expr: "1/0".to_string(),
                lang: Some("en".to_string()),
            },
            Duration::from_secs(5),
        )
        .await
        .unwrap();
        assert!(bad.findings.iter().any(|f| f.code == "DIV_ZERO_LITERAL"));

        let clean = lint_with_timeout(
            LintRequest {
                expr: "sin(1)".to_string(),
                lang: Some("en".to_string()),
            },
            Duration::from_secs(5),
        )
        .await
        .unwrap();
        assert!(clean.findings.is_empty());
    }
}
