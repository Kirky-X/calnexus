// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! Solve 计算域：方程求解表达式入口 `solve` / `solve_system` / `solve_ineq`。
//!
//! 本文件仅负责路由分发与结果转换；求解算法在 `math::solve`，本域不重复实现。

use super::common::{ensure_math_constants, resolve_variable, unsupported_function_error};
use crate::core::CalculationDomain;
use crate::core::{AstNode, CalcError, EvalContext, EvalResult};
use crate::math::solve::{SolutionSet, solve_equation, solve_equation_in_interval};
use crate::math::symbolic::{SymbolicExpr, ast_to_symbolic};

/// solve 域函数白名单（目录 / REPL 补全 / MCP list_functions 的单一事实源）。
pub(crate) const SOLVE_FUNCTIONS: &[&str] = &["solve", "solve_system", "solve_ineq"];

/// Solve 计算域：priority=25，与 number_theory / combinatorics / polynomial 同级。
pub struct SolveDomain;

impl CalculationDomain for SolveDomain {
    fn domain_name(&self) -> &str {
        "solve"
    }

    fn priority(&self) -> u8 {
        25
    }

    fn supports(&self, ast: &AstNode) -> bool {
        contains_solve_function(ast)
    }

    fn evaluate(&self, ast: &AstNode, ctx: &EvalContext) -> Result<EvalResult, CalcError> {
        let ctx = ensure_math_constants(ctx);
        match ast {
            AstNode::FunctionCall(name, args) => self.eval_function(name, args, &ctx),
            _ => Err(CalcError::domain(format!(
                "solve domain expects a function call, got {:?}",
                ast
            ))),
        }
    }
}

impl Default for SolveDomain {
    fn default() -> Self {
        Self
    }
}

impl SolveDomain {
    fn eval_function(
        &self,
        name: &str,
        args: &[AstNode],
        ctx: &EvalContext,
    ) -> Result<EvalResult, CalcError> {
        if !SOLVE_FUNCTIONS.contains(&name) {
            return Err(unsupported_function_error("solve", name));
        }
        match name {
            "solve" => self.eval_solve(args, ctx),
            // solve_system / solve_ineq 分别在后续任务 T007 / T009 接入。
            _ => Err(unsupported_function_error("solve", name)),
        }
    }

    /// `solve(eq, var)` / `solve(eq, var, [a,b])` / `solve(eq, var, a, b)`。
    fn eval_solve(&self, args: &[AstNode], ctx: &EvalContext) -> Result<EvalResult, CalcError> {
        if args.len() != 2 && args.len() != 3 && args.len() != 4 {
            return Err(CalcError::domain(format!(
                "solve expects 2, 3 or 4 arguments, got {}",
                args.len()
            ))
            .with_i18n("msg.solve.bad_arity", vec![]));
        }
        let (lhs, rhs) = equation_sides(&args[0])?;
        let var = equation_variable(&args[1])?;

        let set = match args.len() {
            2 => solve_equation(&lhs, &rhs, &var)?,
            3 => {
                let AstNode::List(bounds) = &args[2] else {
                    return Err(CalcError::domain(
                        "solve interval must be a list [a, b]".to_string(),
                    )
                    .with_i18n("msg.solve.bad_interval_arg", vec![]));
                };
                if bounds.len() != 2 {
                    return Err(CalcError::domain(
                        "solve interval list must contain exactly two bounds".to_string(),
                    )
                    .with_i18n("msg.solve.bad_interval_arg", vec![]));
                }
                let a = scalar_arg(&bounds[0], ctx)?;
                let b = scalar_arg(&bounds[1], ctx)?;
                solve_equation_in_interval(&lhs, &rhs, &var, a, b)?
            }
            _ => {
                let a = scalar_arg(&args[2], ctx)?;
                let b = scalar_arg(&args[3], ctx)?;
                solve_equation_in_interval(&lhs, &rhs, &var, a, b)?
            }
        };
        Ok(solution_to_result(set))
    }
}

/// 内部 `eq(lhs, rhs)` 节点 → 两侧符号表达式。
fn equation_sides(ast: &AstNode) -> Result<(SymbolicExpr, SymbolicExpr), CalcError> {
    match ast {
        AstNode::FunctionCall(name, args) if name == "eq" && args.len() == 2 => {
            Ok((ast_to_symbolic(&args[0])?, ast_to_symbolic(&args[1])?))
        }
        _ => Err(CalcError::domain(
            "solve first argument must be an equation (lhs = rhs)".to_string(),
        )
        .with_i18n("msg.solve.first_arg_not_equation", vec![])),
    }
}

/// 求解变量必须是纯变量名（允许 pi/e 之外的一切标识符）。
fn equation_variable(ast: &AstNode) -> Result<String, CalcError> {
    match ast {
        AstNode::Variable(name) => Ok(name.clone()),
        _ => Err(
            CalcError::domain("solve variable must be a variable name".to_string())
                .with_i18n("msg.solve.bad_variable", vec![]),
        ),
    }
}

/// 区间端点：数字字面量、变量（含 pi/e）或一元负号。
fn scalar_arg(ast: &AstNode, ctx: &EvalContext) -> Result<f64, CalcError> {
    match ast {
        AstNode::Number(v) => Ok(*v),
        AstNode::BigNumber(s) => s.parse::<f64>().map_err(|_| {
            CalcError::domain(format!("invalid numeric bound: {s}"))
                .with_i18n("msg.solve.bad_interval_arg", vec![])
        }),
        AstNode::Variable(name) => resolve_variable(ctx, name),
        AstNode::UnaryOp(op, inner) => {
            let v = scalar_arg(inner, ctx)?;
            match op {
                crate::core::UnaryOp::Neg => Ok(-v),
                _ => Err(
                    CalcError::domain("unsupported interval bound expression".to_string())
                        .with_i18n("msg.solve.bad_interval_arg", vec![]),
                ),
            }
        }
        _ => Err(
            CalcError::domain("unsupported interval bound expression".to_string())
                .with_i18n("msg.solve.bad_interval_arg", vec![]),
        ),
    }
}

/// 求解结果转 `EvalResult`：复根走 ComplexList，纯实根走 Vector。
fn solution_to_result(set: SolutionSet) -> EvalResult {
    match set {
        SolutionSet::Exact(roots) => roots_to_result(roots),
        SolutionSet::Numeric(roots) => EvalResult::Vector(roots),
        SolutionSet::NumericComplex(roots) => roots_to_result(roots),
    }
}

fn roots_to_result(roots: Vec<(f64, f64)>) -> EvalResult {
    if roots.iter().any(|(_, im)| *im != 0.0) {
        EvalResult::ComplexList(roots)
    } else {
        EvalResult::Vector(roots.into_iter().map(|(re, _)| re).collect())
    }
}

/// 递归检查 AST 是否含 solve 域函数调用（路由判定）。
fn contains_solve_function(ast: &AstNode) -> bool {
    match ast {
        AstNode::FunctionCall(name, args) => {
            SOLVE_FUNCTIONS.contains(&name.as_str()) || args.iter().any(contains_solve_function)
        }
        AstNode::BinaryOp(_, l, r) | AstNode::Compare(_, l, r) => {
            contains_solve_function(l) || contains_solve_function(r)
        }
        AstNode::UnaryOp(_, e) => contains_solve_function(e),
        AstNode::Matrix(rows) => rows.iter().flatten().any(contains_solve_function),
        AstNode::List(elements) => elements.iter().any(contains_solve_function),
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
    use crate::core::{CalculationDomain, parse};

    fn eval_solve_domain(expr: &str) -> Result<EvalResult, CalcError> {
        let ast = parse(expr)?;
        assert!(SolveDomain.supports(&ast), "supports should claim {expr}");
        SolveDomain.evaluate(&ast, &EvalContext::new())
    }

    #[test]
    fn test_solve_quadratic_through_domain() {
        let got = eval_solve_domain("solve(x^2-4=0, x)").unwrap();
        let EvalResult::Vector(mut v) = got else {
            panic!("expected vector roots");
        };
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(v, vec![-2.0, 2.0]);
    }

    #[test]
    fn test_solve_interval_form() {
        let got = eval_solve_domain("solve(cos(x)=0, x, [0, pi])").unwrap();
        let EvalResult::Vector(v) = got else {
            panic!("expected vector roots");
        };
        assert_eq!(v.len(), 1);
        assert!((v[0] - std::f64::consts::FRAC_PI_2).abs() < 1e-8);
    }

    #[test]
    fn test_solve_domain_priority_and_whitelist() {
        assert_eq!(SolveDomain.priority(), 25);
        assert_eq!(SolveDomain.domain_name(), "solve");
        assert!(contains_solve_function(
            &parse("solve_ineq(x>0, x)").unwrap()
        ));
        assert!(!contains_solve_function(&parse("2+3").unwrap()));
    }

    #[test]
    fn test_solve_bad_arity() {
        let err = eval_solve_domain("solve(x=0)").unwrap_err();
        assert_eq!(err.kind, crate::core::ErrorKind::Domain);
    }
}
