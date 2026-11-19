// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! 表达式静态检查纯函数：常量除零、非正对数、负平方根与深度预警。
//!
//! 供 server `lint` tool 使用；只做结构/常量分析，不求值自由变量。

use std::collections::HashMap;

use crate::core::MAX_AST_DEPTH;
use crate::core::{AstNode, BinaryOp, UnaryOp};
use crate::math::symbolic::{ast_to_symbolic, eval_symbolic, simplify};

/// 单条 lint 发现：稳定 code 供 AI/工具消费，message 为可读说明。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LintFinding {
    pub code: &'static str,
    pub message: String,
}

/// 提取字面量数值（含一元负号包裹的负数）。
fn as_literal_number(ast: &AstNode) -> Option<f64> {
    match ast {
        AstNode::Number(v) => Some(*v),
        AstNode::UnaryOp(UnaryOp::Neg, inner) => as_literal_number(inner).map(|v| -v),
        _ => None,
    }
}

/// 判断子表达式是否为常量零（自由变量存在时不判定）。
fn is_constant_zero(ast: &AstNode) -> bool {
    let Ok(symbolic) = ast_to_symbolic(ast) else {
        return false;
    };
    let simplified = simplify(&symbolic);
    match eval_symbolic(&simplified, &HashMap::new()) {
        Ok(value) => value.abs() < 1e-15,
        Err(_) => false,
    }
}

/// 计算 AST 深度（叶子为 1）。
fn ast_depth(ast: &AstNode) -> usize {
    match ast {
        AstNode::Number(_)
        | AstNode::BigNumber(_)
        | AstNode::Complex(_, _)
        | AstNode::Variable(_)
        | AstNode::Str(_) => 1,
        AstNode::BinaryOp(_, l, r) | AstNode::Compare(_, l, r) => {
            1 + ast_depth(l).max(ast_depth(r))
        }
        AstNode::UnaryOp(_, e) => 1 + ast_depth(e),
        AstNode::FunctionCall(_, args) | AstNode::List(args) => {
            1 + args.iter().map(ast_depth).max().unwrap_or(0)
        }
        AstNode::Matrix(rows) => 1 + rows.iter().flatten().map(ast_depth).max().unwrap_or(0),
    }
}

/// 静态检查表达式，返回全部发现（无问题返回空向量）。
pub fn lint_expr(ast: &AstNode) -> Vec<LintFinding> {
    let mut findings = Vec::new();
    let threshold = MAX_AST_DEPTH / 2;
    walk(ast, &mut findings, threshold);
    findings
}

fn walk(ast: &AstNode, findings: &mut Vec<LintFinding>, depth_threshold: usize) {
    if ast_depth(ast) > depth_threshold {
        findings.push(LintFinding {
            code: "DEEP_NESTING",
            message: format!(
                "expression nesting exceeds the warning threshold of {depth_threshold}"
            ),
        });
        // 深度预警只报一次，避免每递归一层重复出现。
        return;
    }
    match ast {
        AstNode::BinaryOp(BinaryOp::Div, _, rhs) => {
            if is_constant_zero(rhs) {
                findings.push(LintFinding {
                    code: "DIV_ZERO_LITERAL",
                    message: "division by a constant-zero expression".to_string(),
                });
            }
        }
        AstNode::FunctionCall(name, args) if matches!(name.as_str(), "ln" | "log") => {
            if let Some(value) = args.first().and_then(as_literal_number)
                && value <= 0.0
            {
                findings.push(LintFinding {
                    code: "LOG_NONPOSITIVE_LITERAL",
                    message: format!("{name} argument is a non-positive literal: {value}"),
                });
            }
        }
        AstNode::FunctionCall(name, args) if name == "sqrt" => {
            if let Some(value) = args.first().and_then(as_literal_number)
                && value < 0.0
            {
                findings.push(LintFinding {
                    code: "SQRT_NEGATIVE_LITERAL",
                    message: format!("sqrt argument is a negative literal: {value}"),
                });
            }
        }
        _ => {}
    }
    match ast {
        AstNode::BinaryOp(_, l, r) | AstNode::Compare(_, l, r) => {
            walk(l, findings, depth_threshold);
            walk(r, findings, depth_threshold);
        }
        AstNode::UnaryOp(UnaryOp::Neg | UnaryOp::Abs | UnaryOp::Factorial, e) => {
            walk(e, findings, depth_threshold);
        }
        AstNode::FunctionCall(_, args) | AstNode::List(args) => {
            for arg in args {
                walk(arg, findings, depth_threshold);
            }
        }
        AstNode::Matrix(rows) => {
            for row in rows {
                for cell in row {
                    walk(cell, findings, depth_threshold);
                }
            }
        }
        AstNode::Number(_)
        | AstNode::BigNumber(_)
        | AstNode::Complex(_, _)
        | AstNode::Variable(_)
        | AstNode::Str(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::parse;

    fn codes(expr: &str) -> Vec<&'static str> {
        lint_expr(&parse(expr).unwrap())
            .into_iter()
            .map(|f| f.code)
            .collect()
    }

    #[test]
    fn test_div_zero_literal() {
        assert!(codes("1/0").contains(&"DIV_ZERO_LITERAL"));
        assert!(codes("1/(2-2)").contains(&"DIV_ZERO_LITERAL"));
    }

    #[test]
    fn test_log_and_sqrt_literals() {
        assert!(codes("ln(0)").contains(&"LOG_NONPOSITIVE_LITERAL"));
        assert!(codes("log(-1)").contains(&"LOG_NONPOSITIVE_LITERAL"));
        assert!(codes("sqrt(-1)").contains(&"SQRT_NEGATIVE_LITERAL"));
    }

    #[test]
    fn test_clean_expression_has_no_findings() {
        assert!(lint_expr(&parse("sin(1)+2*x").unwrap()).is_empty());
    }

    #[test]
    fn test_deep_nesting_warning() {
        let mut expr = "1".to_string();
        for _ in 0..130 {
            expr = format!("({expr}+1)");
        }
        assert!(codes(&expr).contains(&"DEEP_NESTING"));
    }
}
