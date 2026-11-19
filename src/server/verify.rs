// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! Server verify 工具：表达式恒等判定的 HTTP / MCP 双协议入口。

use super::evaluate::REQUEST_TIMEOUT_SECS;
use super::lang::resolve_i18n;
use super::types::MAX_EXPR_LEN;
use super::{calc_error_to_api_error_i18n, shared_cache};
use crate::math::verify::{VerifyVerdict, verify_equality};
use sdforge::error::ApiError;
use sdforge::forge;
use std::time::Duration;

/// `POST /api/v1/verify` + MCP `verify` 请求。
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct VerifyRequest {
    pub lhs: String,
    pub rhs: String,
    #[serde(default)]
    pub var: Option<String>,
    #[serde(default)]
    pub sample_points: Option<Vec<f64>>,
    #[serde(default)]
    pub lang: Option<String>,
}

/// 数值反例。
#[derive(Debug, Clone, serde::Serialize)]
pub struct Counterexample {
    pub x: f64,
    pub lhs: f64,
    pub rhs: f64,
}

/// `verify` 响应。
#[derive(Debug, Clone, serde::Serialize)]
pub struct VerifyResponse {
    pub v: u32,
    pub equal: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub counterexample: Option<Counterexample>,
}

impl VerifyRequest {
    pub fn validate(&self) -> Result<(), ApiError> {
        let i18n = resolve_i18n(self.lang.as_deref());
        for (field, value) in [("lhs", &self.lhs), ("rhs", &self.rhs)] {
            if value.is_empty() {
                return Err(ApiError::validation(
                    field,
                    i18n.t("server.validation.expr_empty"),
                ));
            }
            if value.len() > MAX_EXPR_LEN {
                return Err(ApiError::validation(
                    field,
                    i18n.tf(
                        "server.validation.expr_too_long",
                        &[
                            ("len", &value.len().to_string()),
                            ("max", &MAX_EXPR_LEN.to_string()),
                        ],
                    ),
                ));
            }
            if value.bytes().any(|b| b == 0) {
                return Err(ApiError::validation(
                    field,
                    i18n.t("server.validation.expr_null_bytes"),
                ));
            }
        }
        Ok(())
    }
}

/// `POST /api/v1/verify` + MCP `verify`：判断两侧表达式是否恒等。
#[forge(
    name = "verify",
    version = 1,
    path = "/verify",
    method = "POST",
    tool_name = "verify",
    description = "Verify whether two math expressions are identical. Args MUST be wrapped in a req object: req.lhs and req.rhs are expression strings, req.var is the variable name (optional, auto-detected), req.sample_points is an optional numeric grid, req.lang is optional response language. Returns equal plus an optional numeric counterexample {x,lhs,rhs}. Errors: 400 InvalidInput, 422 ValidationError, 503 unavailable on timeout.",
    i18n_key = "endpoint.verify.description"
)]
pub(crate) async fn verify(req: VerifyRequest) -> Result<VerifyResponse, ApiError> {
    verify_with_timeout(req, Duration::from_secs(REQUEST_TIMEOUT_SECS)).await
}

/// 带超时的实现（可测试入口）。
pub(crate) async fn verify_with_timeout(
    req: VerifyRequest,
    timeout: Duration,
) -> Result<VerifyResponse, ApiError> {
    let i18n = resolve_i18n(req.lang.as_deref());
    req.validate()?;
    let lhs = req.lhs.clone();
    let rhs = req.rhs.clone();
    let var = req.var.clone();
    let points = req.sample_points.clone();
    let _cache = shared_cache();
    let handle = tokio::task::spawn_blocking(move || {
        verify_equality(&lhs, &rhs, var.as_deref(), points.as_deref())
    });
    let verdict = match tokio::time::timeout(timeout, handle).await {
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
                "verify",
                Some(timeout.as_secs()),
            ));
        }
    }
    .map_err(|e| calc_error_to_api_error_i18n(e, &i18n))?;

    let (equal, counterexample) = match verdict {
        VerifyVerdict::Identical => (true, None),
        VerifyVerdict::Counterexample { x, lhs, rhs } => {
            (false, Some(Counterexample { x, lhs, rhs }))
        }
    };
    Ok(VerifyResponse {
        v: 1,
        equal,
        counterexample,
    })
}

#[cfg(all(test, feature = "server"))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_verify_identity_and_counterexample() {
        let identical = verify_with_timeout(
            VerifyRequest {
                lhs: "(x+1)^2".to_string(),
                rhs: "x^2+2*x+1".to_string(),
                var: Some("x".to_string()),
                sample_points: None,
                lang: Some("en".to_string()),
            },
            Duration::from_secs(5),
        )
        .await
        .unwrap();
        assert!(identical.equal);
        assert!(identical.counterexample.is_none());

        let different = verify_with_timeout(
            VerifyRequest {
                lhs: "x^2".to_string(),
                rhs: "x^3".to_string(),
                var: Some("x".to_string()),
                sample_points: None,
                lang: Some("en".to_string()),
            },
            Duration::from_secs(5),
        )
        .await
        .unwrap();
        assert!(!different.equal);
        assert_eq!(different.counterexample.unwrap().x, 2.0);
    }
}
