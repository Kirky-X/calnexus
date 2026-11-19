// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! Server explain 工具：表达式求值步骤的 HTTP / MCP 双协议入口。

use super::lang::resolve_i18n;
use super::types::MAX_EXPR_LEN;
use super::{calc_error_to_api_error_i18n, shared_cache};
use crate::core::{EvalContext, parse};
use crate::output::generate_steps;
use sdforge::error::ApiError;
use sdforge::forge;
use std::collections::HashMap;
use std::time::Duration;

use super::evaluate::REQUEST_TIMEOUT_SECS;

/// `POST /api/v1/explain` + MCP `explain` 请求。
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ExplainRequest {
    pub expr: String,
    #[serde(default)]
    pub vars: Option<HashMap<String, f64>>,
    #[serde(default)]
    pub lang: Option<String>,
}

/// `explain` 响应：逐步推导文本。
#[derive(Debug, Clone, serde::Serialize)]
pub struct ExplainResponse {
    pub v: u32,
    pub steps: Vec<String>,
}

impl ExplainRequest {
    /// 复用 evaluate 的表达式限长 / 空值 / NUL 字节校验。
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

/// `POST /api/v1/explain` + MCP `explain`：返回表达式的分步推导。
#[forge(
    name = "explain",
    version = 1,
    path = "/explain",
    method = "POST",
    tool_name = "explain",
    description = "Generate step-by-step derivation for a math expression. Args MUST be wrapped in a req object: req.expr is the expression string, req.vars holds optional variable bindings, req.lang is optional response language. Returns ordered derivation steps. Errors: 400 InvalidInput, 422 ValidationError, 503 unavailable on timeout.",
    i18n_key = "endpoint.explain.description"
)]
pub(crate) async fn explain(req: ExplainRequest) -> Result<ExplainResponse, ApiError> {
    explain_with_timeout(req, Duration::from_secs(REQUEST_TIMEOUT_SECS)).await
}

/// 带超时的实现（可测试入口）。
pub(crate) async fn explain_with_timeout(
    req: ExplainRequest,
    timeout: Duration,
) -> Result<ExplainResponse, ApiError> {
    let i18n = resolve_i18n(req.lang.as_deref());
    req.validate()?;
    let expr = req.expr.clone();
    let vars = req.vars.clone().unwrap_or_default();
    // 预热共享缓存，避免首次请求在 spawn_blocking 内初始化造成抖动。
    let _cache = shared_cache();
    let handle = tokio::task::spawn_blocking(move || {
        let ast = parse(&expr)?;
        // 步骤生成为符号推导，变量取值不影响步骤；未绑定的自由变量以 1.0 占位，
        // 使 diff(x^2,x) 一类调用无需显式 --var 也能产出步骤。
        let mut ctx = EvalContext {
            vars,
            ..Default::default()
        };
        let mut names = std::collections::HashSet::new();
        collect_variables(&ast, &mut names);
        for name in names {
            if name != "pi" && name != "e" {
                ctx.vars.entry(name).or_insert(1.0);
            }
        }
        generate_steps(&ast, &ctx)
    });
    let steps = match tokio::time::timeout(timeout, handle).await {
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
                "explain",
                Some(timeout.as_secs()),
            ));
        }
    }
    .map_err(|e| calc_error_to_api_error_i18n(e, &i18n))?;
    Ok(ExplainResponse { v: 1, steps })
}

/// 收集 AST 中出现的变量名（供未绑定变量占位）。
fn collect_variables(ast: &crate::core::AstNode, out: &mut std::collections::HashSet<String>) {
    use crate::core::AstNode;
    match ast {
        AstNode::Variable(name) => {
            out.insert(name.clone());
        }
        AstNode::BinaryOp(_, l, r) | AstNode::Compare(_, l, r) => {
            collect_variables(l, out);
            collect_variables(r, out);
        }
        AstNode::UnaryOp(_, e) => collect_variables(e, out),
        AstNode::FunctionCall(_, args) | AstNode::List(args) => {
            for arg in args {
                collect_variables(arg, out);
            }
        }
        AstNode::Matrix(rows) => {
            for row in rows {
                for cell in row {
                    collect_variables(cell, out);
                }
            }
        }
        AstNode::Number(_) | AstNode::BigNumber(_) | AstNode::Complex(_, _) | AstNode::Str(_) => {}
    }
}

#[cfg(all(test, feature = "server"))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_explain_returns_steps() {
        let resp = explain_with_timeout(
            ExplainRequest {
                expr: "diff(x^2,x)".to_string(),
                vars: None,
                lang: Some("en".to_string()),
            },
            Duration::from_secs(5),
        )
        .await
        .unwrap();
        assert!(!resp.steps.is_empty());
    }

    #[tokio::test]
    async fn test_explain_syntax_error_is_client_error() {
        let err = explain_with_timeout(
            ExplainRequest {
                expr: "1 +".to_string(),
                vars: None,
                lang: Some("en".to_string()),
            },
            Duration::from_secs(5),
        )
        .await
        .unwrap_err();
        assert_eq!(err.http_status(), 400);
    }
}
