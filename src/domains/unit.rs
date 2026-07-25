// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! 物理单位换算域：8 量纲线性/仿射换算。
//!
//! 设计依据：design.md D5（自建换算表 + 温度仿射特例）
//! Feature 门控：`unit = []`

use crate::core::CalculationDomain;
use crate::core::{AstNode, CalcError, EvalContext, EvalResult};

/// 单位换算域：支持 `convert(value, "from", "to")` 覆盖 8 个量纲。
pub struct UnitDomain;

impl CalculationDomain for UnitDomain {
    fn domain_name(&self) -> &str {
        "unit"
    }

    fn supports(&self, ast: &AstNode) -> bool {
        contains_unit_function(ast)
    }

    fn evaluate(&self, ast: &AstNode, ctx: &EvalContext) -> Result<EvalResult, CalcError> {
        evaluate_unit(ast, ctx)
    }

    fn priority(&self) -> u8 {
        30
    }
}

/// 递归检查 AST 是否包含 convert 函数调用。
fn contains_unit_function(ast: &AstNode) -> bool {
    match ast {
        AstNode::FunctionCall(name, args) => {
            name == "convert" || args.iter().any(contains_unit_function)
        }
        AstNode::BinaryOp(_, l, r) => contains_unit_function(l) || contains_unit_function(r),
        AstNode::UnaryOp(_, e) => contains_unit_function(e),
        _ => false,
    }
}

/// 单位域求值入口（stub — 待 Phase 3 实现）。
fn evaluate_unit(_ast: &AstNode, _ctx: &EvalContext) -> Result<EvalResult, CalcError> {
    Err(CalcError::domain("unit domain not yet implemented"))
}
