// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! 跨域公共辅助函数。
//!
//! 消除各计算域重复的 pi/e 注入、变量解析与不支持错误构造。

use std::borrow::Cow;

use crate::core::{AstNode, CalcError, EvalContext};

/// 确保上下文包含 pi/e 常量；仅在缺失时 clone 上下文，返回 Cow。
pub fn ensure_math_constants(ctx: &EvalContext) -> Cow<'_, EvalContext> {
    let needs_pi = ctx.get_var("pi").is_none();
    let needs_e = ctx.get_var("e").is_none();
    if !needs_pi && !needs_e {
        return Cow::Borrowed(ctx);
    }
    let mut ctx = ctx.clone();
    if needs_pi {
        ctx = ctx.with_var("pi", std::f64::consts::PI);
    }
    if needs_e {
        ctx = ctx.with_var("e", std::f64::consts::E);
    }
    Cow::Owned(ctx)
}

/// 解析变量名到 f64 值，未绑定时返回统一错误。
pub fn resolve_variable(ctx: &EvalContext, name: &str) -> Result<f64, CalcError> {
    ctx.get_var(name).ok_or_else(|| {
        CalcError::eval(format!("unbound variable: {}", name)).with_i18n(
            "msg.unbound_variable",
            vec![("name".to_string(), name.to_string())],
        )
    })
}

/// 构造"域不支持此节点类型"错误。
pub fn unsupported_node_error(domain: &str, ast: &AstNode) -> CalcError {
    CalcError::domain(format!(
        "{} domain does not support this node type: {:?}",
        domain, ast
    ))
    .with_i18n(
        "msg.domain.unsupported_node",
        vec![
            ("domain".to_string(), domain.to_string()),
            ("node".to_string(), format!("{:?}", ast)),
        ],
    )
}

/// 构造"域不支持此函数"错误。
pub fn unsupported_function_error(domain: &str, name: &str) -> CalcError {
    CalcError::domain(format!(
        "unsupported function in {} domain: {}",
        domain, name
    ))
    .with_i18n(
        "msg.domain.unsupported_function",
        vec![
            ("domain".to_string(), domain.to_string()),
            ("name".to_string(), name.to_string()),
        ],
    )
}
