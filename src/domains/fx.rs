// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! 汇率换算域：fx/fx_rate 函数 + frankfurter.dev 在线 API。
//!
//! 设计依据：design.md D6（Provider trait 注入 + 三级缓存 + stale 策略）
//! Feature 门控：`fx = ["dep:ureq", "dep:dirs"]`

use crate::core::CalculationDomain;
use crate::core::{AstNode, CalcError, EvalContext, EvalResult};

/// 汇率换算域：支持 `fx(value, "FROM", "TO")` 和 `fx_rate("FROM", "TO")`。
pub struct FxDomain;

impl CalculationDomain for FxDomain {
    fn domain_name(&self) -> &str {
        "fx"
    }

    fn supports(&self, ast: &AstNode) -> bool {
        contains_fx_function(ast)
    }

    fn evaluate(&self, ast: &AstNode, ctx: &EvalContext) -> Result<EvalResult, CalcError> {
        evaluate_fx(ast, ctx)
    }

    fn priority(&self) -> u8 {
        30
    }

    fn nondeterministic_functions(&self) -> &'static [&'static str] {
        &["fx", "fx_rate"]
    }
}

/// 递归检查 AST 是否包含 fx/fx_rate 函数调用。
fn contains_fx_function(ast: &AstNode) -> bool {
    match ast {
        AstNode::FunctionCall(name, args) => {
            (name == "fx" || name == "fx_rate") || args.iter().any(contains_fx_function)
        }
        AstNode::BinaryOp(_, l, r) => contains_fx_function(l) || contains_fx_function(r),
        AstNode::UnaryOp(_, e) => contains_fx_function(e),
        _ => false,
    }
}

/// 汇率域求值入口（stub — 待 Phase 4 实现）。
fn evaluate_fx(_ast: &AstNode, _ctx: &EvalContext) -> Result<EvalResult, CalcError> {
    Err(CalcError::domain("fx domain not yet implemented"))
}
