// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! 时间计算域：日期/时间构造、时间戳互转、间隔计算、格式解析。
//!
//! 设计依据：design.md D4（jiff 0.2 + IANA tzdb）
//! Feature 门控：`time = ["dep:jiff"]`

use crate::core::CalculationDomain;
use crate::core::{AstNode, CalcError, EvalContext, EvalResult};

/// 时间计算域：支持 date/datetime/timestamp/from_timestamp/date_diff/date_add/
/// parse_date/format_date/reformat_date/weekday/day_of_year/is_leap_year/now/today。
pub struct TimeDomain;

impl CalculationDomain for TimeDomain {
    fn domain_name(&self) -> &str {
        "time"
    }

    fn supports(&self, ast: &AstNode) -> bool {
        contains_time_function(ast)
    }

    fn evaluate(&self, ast: &AstNode, ctx: &EvalContext) -> Result<EvalResult, CalcError> {
        evaluate_time(ast, ctx)
    }

    fn priority(&self) -> u8 {
        30
    }

    fn nondeterministic_functions(&self) -> &'static [&'static str] {
        &["now", "today"]
    }
}

/// 时间域支持的函数名集合。
const TIME_FUNCTIONS: &[&str] = &[
    "date",
    "datetime",
    "timestamp",
    "from_timestamp",
    "date_diff",
    "date_add",
    "parse_date",
    "format_date",
    "reformat_date",
    "weekday",
    "day_of_year",
    "is_leap_year",
    "now",
    "today",
];

/// 递归检查 AST 是否包含时间域函数调用。
fn contains_time_function(ast: &AstNode) -> bool {
    match ast {
        AstNode::FunctionCall(name, args) => {
            TIME_FUNCTIONS.contains(&name.as_str()) || args.iter().any(contains_time_function)
        }
        AstNode::BinaryOp(_, l, r) => contains_time_function(l) || contains_time_function(r),
        AstNode::UnaryOp(_, e) => contains_time_function(e),
        _ => false,
    }
}

/// 时间域求值入口（stub — 待 Phase 2 实现）。
fn evaluate_time(_ast: &AstNode, _ctx: &EvalContext) -> Result<EvalResult, CalcError> {
    Err(CalcError::domain("time domain not yet implemented"))
}
