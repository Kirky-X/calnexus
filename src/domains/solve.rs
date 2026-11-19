// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! Solve 计算域：方程求解表达式入口 `solve` / `solve_system` / `solve_ineq`。
//!
//! 本文件仅负责路由分发与结果转换；求解算法在 `math::solve`，本域不重复实现。

use super::common::{ensure_math_constants, resolve_variable, unsupported_function_error};
use crate::core::CalculationDomain;
use crate::core::{AstNode, CalcError, CompareOp, EvalContext, EvalResult};
use crate::math::solve::{
    InequalityOp, SolutionSet, solve_equation, solve_equation_in_interval, solve_inequality,
    solve_system,
};
use crate::math::symbolic::{SymbolicExpr, ast_to_symbolic, symbolic_to_string};

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
            "solve_system" => self.eval_solve_system(args, ctx),
            "solve_ineq" => self.eval_solve_ineq(args),
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

    /// `solve_system([eq1, eq2], [x, y], [x0, y0])`：多变量 Newton，输出 JSON。
    fn eval_solve_system(
        &self,
        args: &[AstNode],
        ctx: &EvalContext,
    ) -> Result<EvalResult, CalcError> {
        if args.len() != 3 {
            return Err(CalcError::domain(format!(
                "solve_system expects 3 arguments, got {}",
                args.len()
            ))
            .with_i18n("msg.solve.bad_arity", vec![]));
        }
        let AstNode::List(eq_nodes) = &args[0] else {
            return Err(
                CalcError::domain("solve_system expects an equation list".to_string())
                    .with_i18n("msg.solve.first_arg_not_equation", vec![]),
            );
        };
        let AstNode::List(var_nodes) = &args[1] else {
            return Err(
                CalcError::domain("solve_system expects a variable list".to_string())
                    .with_i18n("msg.solve.system_dimension_mismatch", vec![]),
            );
        };
        let AstNode::List(init_nodes) = &args[2] else {
            return Err(CalcError::domain(
                "solve_system expects an initial value list".to_string(),
            )
            .with_i18n("msg.solve.system_dimension_mismatch", vec![]));
        };

        let mut equations: Vec<String> = Vec::with_capacity(eq_nodes.len());
        for node in eq_nodes {
            match node {
                AstNode::Str(text) => equations.push(text.clone()),
                AstNode::FunctionCall(name, parts) if name == "eq" && parts.len() == 2 => {
                    let lhs = ast_to_symbolic(&parts[0])?;
                    let rhs = ast_to_symbolic(&parts[1])?;
                    let diff = SymbolicExpr::Sub(Box::new(lhs), Box::new(rhs));
                    equations.push(symbolic_to_string(&diff));
                }
                _ => {
                    return Err(CalcError::domain(
                        "solve_system equations must be equality expressions".to_string(),
                    )
                    .with_i18n("msg.solve.first_arg_not_equation", vec![]));
                }
            }
        }
        let vars: Vec<String> = var_nodes
            .iter()
            .map(equation_variable)
            .collect::<Result<_, _>>()?;
        let x0: Vec<f64> = init_nodes
            .iter()
            .map(|n| scalar_arg(n, ctx))
            .collect::<Result<_, _>>()?;

        let var_refs: Vec<&str> = vars.iter().map(String::as_str).collect();
        let eq_refs: Vec<&str> = equations.iter().map(String::as_str).collect();
        let (solution, residual) = solve_system(&eq_refs, &var_refs, &x0, 1e-12, 200)?;
        Ok(EvalResult::Json(serde_json::json!({
            "solution": solution,
            "residual": residual,
        })))
    }

    /// `solve_ineq(ineq, var)`：首参须为 Compare 节点，输出区间 JSON。
    fn eval_solve_ineq(&self, args: &[AstNode]) -> Result<EvalResult, CalcError> {
        if args.len() != 2 {
            return Err(
                CalcError::domain("solve_ineq expects 2 arguments".to_string())
                    .with_i18n("msg.solve.bad_arity", vec![]),
            );
        }
        let AstNode::Compare(cmp, lhs, rhs) = &args[0] else {
            return Err(unsupported_function_error(
                "solve_ineq",
                "non-comparison first argument",
            ));
        };
        let op = match cmp {
            CompareOp::Lt => InequalityOp::Lt,
            CompareOp::Le => InequalityOp::Le,
            CompareOp::Gt => InequalityOp::Gt,
            CompareOp::Ge => InequalityOp::Ge,
        };
        let var = equation_variable(&args[1])?;
        let lhs = ast_to_symbolic(lhs)?;
        let rhs = ast_to_symbolic(rhs)?;
        Ok(EvalResult::Json(solve_inequality(&lhs, &rhs, &var, op)?))
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
    #[test]
    fn test_solve_system_domain_json() {
        let got = eval_solve_domain("solve_system([x+y=3, x*y=2], [x,y], [1,1])").unwrap();
        let EvalResult::Json(value) = got else {
            panic!("expected JSON result");
        };
        let solution = value["solution"].as_array().unwrap();
        assert_eq!(solution.len(), 2);
        let x = solution[0].as_f64().unwrap();
        let y = solution[1].as_f64().unwrap();
        assert!((x + y - 3.0).abs() < 1e-9);
        assert!((x * y - 2.0).abs() < 1e-9);
        assert!(value["residual"].as_f64().unwrap() < 1e-9);
    }

    #[test]
    fn test_solve_system_domain_bad_arity() {
        let err = eval_solve_domain("solve_system([x=1], [x])").unwrap_err();
        assert_eq!(err.kind, crate::core::ErrorKind::Domain);
    }
    #[test]
    fn test_solve_ineq_domain_strict_intervals() {
        let got = eval_solve_domain("solve_ineq(x^2-1 > 0, x)").unwrap();
        let EvalResult::Json(value) = got else {
            panic!("expected JSON result");
        };
        let intervals = value["intervals"].as_array().unwrap();
        assert_eq!(intervals.len(), 2);
        assert!(intervals[0]["lower"].is_null());
        assert_eq!(intervals[0]["upper"].as_f64().unwrap(), -1.0);
        assert!(intervals[1]["lower"].as_f64().unwrap() == 1.0);
        assert!(intervals[1]["upper"].is_null());
    }

    #[test]
    fn test_solve_ineq_domain_closed_interval() {
        let got = eval_solve_domain("solve_ineq(x^2-1 <= 0, x)").unwrap();
        let EvalResult::Json(value) = got else {
            panic!("expected JSON result");
        };
        let intervals = value["intervals"].as_array().unwrap();
        assert_eq!(intervals.len(), 1);
        assert_eq!(intervals[0]["lower"].as_f64().unwrap(), -1.0);
        assert_eq!(intervals[0]["upper"].as_f64().unwrap(), 1.0);
        assert!(!intervals[0]["lower_open"].as_bool().unwrap());
    }

    #[test]
    fn test_solve_ineq_domain_rejects_non_comparison() {
        let err = eval_solve_domain("solve_ineq(x^2-1, x)").unwrap_err();
        assert_eq!(err.kind, crate::core::ErrorKind::Domain);
    }
}
