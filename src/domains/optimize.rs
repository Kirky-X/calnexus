// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! Optimize 计算域：一维极小化 `minimize` 与线性规划 `lp` 的表达式入口。
//!
//! 算法层在 `math::optimize`；本域负责参数提取、变量绑定与结果封装。

use super::common::{ensure_math_constants, resolve_variable, unsupported_function_error};
use crate::core::CalculationDomain;
use crate::core::{AstNode, CalcError, EvalContext, EvalResult, UnaryOp};
use crate::math::optimize::{LpProblem, LpSolution, LpStatus, minimize_1d, solve_lp};
use crate::math::symbolic::{ast_to_symbolic, eval_symbolic};
use std::collections::HashMap;

/// optimize 域函数白名单（目录 / REPL 补全 / MCP list_functions 单一事实源）。
pub(crate) const OPTIMIZE_FUNCTIONS: &[&str] = &["minimize", "lp"];

/// Optimize 计算域：priority=25，与 number_theory / solve 同级。
pub struct OptimizeDomain;

impl CalculationDomain for OptimizeDomain {
    fn domain_name(&self) -> &str {
        "optimize"
    }

    fn priority(&self) -> u8 {
        25
    }

    fn supports(&self, ast: &AstNode) -> bool {
        contains_optimize_function(ast)
    }

    fn evaluate(&self, ast: &AstNode, ctx: &EvalContext) -> Result<EvalResult, CalcError> {
        let ctx = ensure_math_constants(ctx);
        match ast {
            AstNode::FunctionCall(name, args) => self.eval_function(name, args, &ctx),
            _ => Err(CalcError::domain(
                "optimize domain expects a function call".to_string(),
            )),
        }
    }
}

impl Default for OptimizeDomain {
    fn default() -> Self {
        Self
    }
}

impl OptimizeDomain {
    fn eval_function(
        &self,
        name: &str,
        args: &[AstNode],
        ctx: &EvalContext,
    ) -> Result<EvalResult, CalcError> {
        if !OPTIMIZE_FUNCTIONS.contains(&name) {
            return Err(unsupported_function_error("optimize", name));
        }
        match name {
            "minimize" => self.eval_minimize(args, ctx),
            "lp" => self.eval_lp(args, ctx),
            _ => Err(unsupported_function_error("optimize", name)),
        }
    }

    /// `minimize(f, var, [a, b])` → `[x*, f(x*)]`。
    fn eval_minimize(&self, args: &[AstNode], ctx: &EvalContext) -> Result<EvalResult, CalcError> {
        if args.len() != 3 {
            return Err(
                CalcError::domain("minimize expects 3 arguments".to_string())
                    .with_i18n("msg.optimize.bad_arity", vec![]),
            );
        }
        let AstNode::Variable(var) = &args[1] else {
            return Err(
                CalcError::domain("minimize variable must be a variable name".to_string())
                    .with_i18n("msg.optimize.bad_variable", vec![]),
            );
        };
        let AstNode::List(bounds) = &args[2] else {
            return Err(
                CalcError::domain("minimize bounds must be a list [a, b]".to_string())
                    .with_i18n("msg.optimize.invalid_interval", vec![]),
            );
        };
        if bounds.len() != 2 {
            return Err(
                CalcError::domain("minimize bounds must contain two values".to_string())
                    .with_i18n("msg.optimize.invalid_interval", vec![]),
            );
        }
        let a = scalar_arg(&bounds[0], ctx)?;
        let b = scalar_arg(&bounds[1], ctx)?;
        let expr = ast_to_symbolic(&args[0])?;
        let f = |x: f64| {
            let mut env = HashMap::new();
            env.insert(var.clone(), x);
            eval_symbolic(&expr, &env).unwrap_or(f64::NAN)
        };
        let (x, fx) = minimize_1d(f, a, b, 1e-12, 300)?;
        Ok(EvalResult::Vector(vec![x, fx]))
    }

    /// `lp(c, A_ub, b_ub)`：求解 min c·x s.t. A_ub·x ≤ b_ub, x ≥ 0。
    fn eval_lp(&self, args: &[AstNode], ctx: &EvalContext) -> Result<EvalResult, CalcError> {
        if args.len() != 3 {
            return Err(CalcError::domain("lp expects 3 arguments".to_string())
                .with_i18n("msg.optimize.bad_arity", vec![]));
        }
        let AstNode::List(c_nodes) = &args[0] else {
            return Err(CalcError::domain("lp objective must be a list".to_string())
                .with_i18n("msg.optimize.lp_dimension_mismatch", vec![]));
        };
        let matrix_rows: Vec<&Vec<AstNode>> = match &args[1] {
            AstNode::Matrix(rows) => rows.iter().collect(),
            AstNode::List(rows) => rows
                .iter()
                .map(|row| match row {
                    AstNode::List(cells) => Ok(cells),
                    _ => Err(
                        CalcError::domain("lp matrix rows must be lists".to_string())
                            .with_i18n("msg.optimize.lp_dimension_mismatch", vec![]),
                    ),
                })
                .collect::<Result<_, _>>()?,
            _ => {
                return Err(
                    CalcError::domain("lp matrix must be a matrix literal".to_string())
                        .with_i18n("msg.optimize.lp_dimension_mismatch", vec![]),
                );
            }
        };
        let AstNode::List(b_nodes) = &args[2] else {
            return Err(CalcError::domain("lp rhs must be a list".to_string())
                .with_i18n("msg.optimize.lp_dimension_mismatch", vec![]));
        };

        let c: Vec<f64> = c_nodes
            .iter()
            .map(|n| scalar_arg(n, ctx))
            .collect::<Result<_, _>>()?;
        let a_ub: Vec<Vec<f64>> = matrix_rows
            .iter()
            .map(|row| row.iter().map(|n| scalar_arg(n, ctx)).collect())
            .collect::<Result<_, _>>()?;
        let b_ub: Vec<f64> = b_nodes
            .iter()
            .map(|n| scalar_arg(n, ctx))
            .collect::<Result<_, _>>()?;

        let solution: LpSolution = solve_lp(&LpProblem {
            c: &c,
            a_ub: &a_ub,
            b_ub: &b_ub,
        })?;
        let status = match solution.status {
            LpStatus::Optimal => "optimal",
            LpStatus::Infeasible => "infeasible",
            LpStatus::Unbounded => "unbounded",
        };
        Ok(EvalResult::Json(serde_json::json!({
            "status": status,
            "x": solution.x,
            "objective": solution.objective,
        })))
    }
}

fn scalar_arg(ast: &AstNode, ctx: &EvalContext) -> Result<f64, CalcError> {
    match ast {
        AstNode::Number(v) => Ok(*v),
        AstNode::Variable(name) => resolve_variable(ctx, name),
        AstNode::UnaryOp(UnaryOp::Neg, inner) => Ok(-scalar_arg(inner, ctx)?),
        _ => Err(
            CalcError::domain("unsupported numeric argument".to_string())
                .with_i18n("msg.optimize.invalid_interval", vec![]),
        ),
    }
}

fn contains_optimize_function(ast: &AstNode) -> bool {
    match ast {
        AstNode::FunctionCall(name, args) => {
            OPTIMIZE_FUNCTIONS.contains(&name.as_str())
                || args.iter().any(contains_optimize_function)
        }
        AstNode::BinaryOp(_, l, r) | AstNode::Compare(_, l, r) => {
            contains_optimize_function(l) || contains_optimize_function(r)
        }
        AstNode::UnaryOp(_, e) => contains_optimize_function(e),
        AstNode::Matrix(rows) => rows.iter().flatten().any(contains_optimize_function),
        AstNode::List(elements) => elements.iter().any(contains_optimize_function),
        AstNode::Number(_)
        | AstNode::BigNumber(_)
        | AstNode::Complex(_, _)
        | AstNode::Variable(_)
        | AstNode::Str(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::parse;

    fn eval_optimize(expr: &str) -> Result<EvalResult, CalcError> {
        let ast = parse(expr)?;
        assert!(OptimizeDomain.supports(&ast));
        OptimizeDomain.evaluate(&ast, &EvalContext::new())
    }

    #[test]
    fn test_minimize_quadratic_through_domain() {
        let got = eval_optimize("minimize((x-3)^2, x, [0, 10])").unwrap();
        let EvalResult::Vector(v) = got else {
            panic!("expected vector result");
        };
        assert!((v[0] - 3.0).abs() < 1e-6, "x*={}", v[0]);
        assert!(v[1].abs() < 1e-10, "f(x*)={}", v[1]);
    }

    #[test]
    fn test_optimize_domain_metadata() {
        assert_eq!(OptimizeDomain.priority(), 25);
        assert_eq!(OptimizeDomain.domain_name(), "optimize");
        assert!(contains_optimize_function(&parse("lp([1],[1])").unwrap()));
        assert!(!contains_optimize_function(&parse("2+3").unwrap()));
    }

    #[test]
    fn test_minimize_bad_arity() {
        assert!(eval_optimize("minimize(x^2, x)").is_err());
    }
    #[test]
    fn test_lp_domain_optimal() {
        let got = eval_optimize("lp([-3,-2], [[2,1],[1,2]], [20,20])").unwrap();
        let EvalResult::Json(v) = got else {
            panic!("expected JSON result");
        };
        assert_eq!(v["status"].as_str().unwrap(), "optimal");
        let x = v["x"].as_array().unwrap();
        assert!((x[0].as_f64().unwrap() - 20.0 / 3.0).abs() < 1e-9);
        assert!((v["objective"].as_f64().unwrap() + 100.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn test_lp_domain_infeasible_status() {
        let got = eval_optimize("lp([1], [[1],[-1]], [1,-2])").unwrap();
        let EvalResult::Json(v) = got else {
            panic!("expected JSON result");
        };
        assert_eq!(v["status"].as_str().unwrap(), "infeasible");
    }

    #[test]
    fn test_lp_domain_dimension_mismatch_error() {
        let err = eval_optimize("lp([1,2], [[1]], [1])").unwrap_err();
        assert_eq!(err.kind, crate::core::ErrorKind::Domain);
    }
}
