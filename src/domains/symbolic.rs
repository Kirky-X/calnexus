// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! Symbolic 计算域：符号微分、积分、化简、极限、泰勒级数。
//!
//! 设计依据：
//! - design.md D2（SymbolicExpr 枚举 + AST 变换 + 字符串输出）
//! - v1.0 symbolic-domain spec
//!
//! 路由策略：AST 含 diff/integrate/simplify/limit/taylor 函数调用时路由至本域。
//! priority=30，与 Complex/Matrix/Vector 同级。
//!
//! 核心数据结构 [`SymbolicExpr`] 与 [`AstNode`] 双向转换，符号变换后格式化为
//! 字符串返回 [`EvalResult::Symbolic`]。
//!
//! T021 重构：纯数学逻辑委托给 `math::symbolic`，本模块仅保留 AST 转换和域路由。

use crate::core::CalculationDomain;
use crate::core::{AstNode, BinaryOp, CalcError, EvalContext, EvalResult, UnaryOp};
use crate::math::symbolic as math_sym;
use std::collections::HashMap;

// Re-export SymbolicExpr and ast_to_symbolic for tests and other modules.
pub use crate::math::symbolic::SymbolicExpr;
pub use crate::math::symbolic::ast_to_symbolic;

/// 符号函数白名单。
const SYMBOLIC_FUNCTIONS: &[&str] = &["diff", "integrate", "simplify", "limit", "taylor"];

// ============================ AstNode ↔ SymbolicExpr 转换 ============================
// ast_to_symbolic 已迁移至 math::symbolic，此处保留 re-export 供向后兼容。

// ============================ SymbolicDomain (TG3.7) ============================

/// Symbolic 计算域（TG3.7）。
///
/// priority=30，路由触发词：diff/integrate/simplify/limit/taylor。
pub struct SymbolicDomain;

impl CalculationDomain for SymbolicDomain {
    fn domain_name(&self) -> &str {
        "symbolic"
    }

    fn priority(&self) -> u8 {
        30
    }

    fn supports(&self, ast: &AstNode) -> bool {
        contains_symbolic_function(ast)
    }

    fn evaluate(&self, ast: &AstNode, _ctx: &EvalContext) -> Result<EvalResult, CalcError> {
        self.eval_node(ast)
    }
}

impl Default for SymbolicDomain {
    fn default() -> Self {
        Self
    }
}

impl SymbolicDomain {
    /// 递归求值 AST 节点。
    fn eval_node(&self, ast: &AstNode) -> Result<EvalResult, CalcError> {
        match ast {
            AstNode::FunctionCall(name, args) => self.eval_function(name, args),
            _ => Err(CalcError::domain(format!(
                "symbolic domain expects function call, got: {:?}",
                ast
            ))
            .with_i18n(
                "msg.symbolic.expects_function_call",
                vec![("node".to_string(), format!("{:?}", ast))],
            )),
        }
    }

    /// 求值符号函数调用：按函数名分发到对应的处理方法。
    fn eval_function(&self, name: &str, args: &[AstNode]) -> Result<EvalResult, CalcError> {
        if !SYMBOLIC_FUNCTIONS.contains(&name) {
            return Err(CalcError::domain(format!(
                "unsupported function in symbolic domain: {}",
                name
            ))
            .with_i18n(
                "msg.symbolic.unsupported_function",
                vec![("name".to_string(), name.to_string())],
            ));
        }
        match name {
            "diff" => self.eval_diff(args),
            "integrate" => self.eval_integrate(args),
            "simplify" => self.eval_simplify(args),
            "limit" => self.eval_limit(args),
            "taylor" => self.eval_taylor(args),
            _ => unreachable!("checked above"),
        }
    }

    /// diff(expr, var)：符号求导。
    fn eval_diff(&self, args: &[AstNode]) -> Result<EvalResult, CalcError> {
        if args.len() != 2 {
            return Err(CalcError::domain(format!(
                "diff() requires exactly 2 arguments, got {}",
                args.len()
            ))
            .with_i18n(
                "msg.symbolic.diff_arg_count",
                vec![("actual".to_string(), args.len().to_string())],
            ));
        }
        let expr = ast_to_symbolic(&args[0])?;
        let var = extract_var_name(&args[1])?;
        let result = math_sym::simplify(&math_sym::diff(&expr, &var));
        Ok(EvalResult::Symbolic(math_sym::symbolic_to_string(&result)))
    }

    /// integrate(expr, var)：符号不定积分。
    fn eval_integrate(&self, args: &[AstNode]) -> Result<EvalResult, CalcError> {
        if args.len() != 2 {
            return Err(CalcError::domain(format!(
                "integrate() requires exactly 2 arguments, got {}",
                args.len()
            ))
            .with_i18n(
                "msg.symbolic.integrate_arg_count",
                vec![("actual".to_string(), args.len().to_string())],
            ));
        }
        let expr = ast_to_symbolic(&args[0])?;
        let var = extract_var_name(&args[1])?;
        let result = math_sym::simplify(&math_sym::integrate(&expr, &var)?);
        Ok(EvalResult::Symbolic(math_sym::symbolic_to_string(&result)))
    }

    /// simplify(expr)：符号化简。
    fn eval_simplify(&self, args: &[AstNode]) -> Result<EvalResult, CalcError> {
        if args.len() != 1 {
            return Err(CalcError::domain(format!(
                "simplify() requires exactly 1 argument, got {}",
                args.len()
            ))
            .with_i18n(
                "msg.symbolic.simplify_arg_count",
                vec![("actual".to_string(), args.len().to_string())],
            ));
        }
        let expr = ast_to_symbolic(&args[0])?;
        let result = math_sym::simplify(&expr);
        Ok(EvalResult::Symbolic(math_sym::symbolic_to_string(&result)))
    }

    /// limit(expr, var, point)：极限计算。
    fn eval_limit(&self, args: &[AstNode]) -> Result<EvalResult, CalcError> {
        if args.len() != 3 {
            return Err(CalcError::domain(format!(
                "limit() requires exactly 3 arguments, got {}",
                args.len()
            ))
            .with_i18n(
                "msg.symbolic.limit_arg_count",
                vec![("actual".to_string(), args.len().to_string())],
            ));
        }
        let expr = ast_to_symbolic(&args[0])?;
        let var = extract_var_name(&args[1])?;
        let point = extract_number(&args[2])?;
        math_sym::limit(&expr, &var, point)
    }

    /// taylor(expr, var, order)：泰勒级数展开。
    fn eval_taylor(&self, args: &[AstNode]) -> Result<EvalResult, CalcError> {
        if args.len() != 3 {
            return Err(CalcError::domain(format!(
                "taylor() requires exactly 3 arguments, got {}",
                args.len()
            ))
            .with_i18n(
                "msg.symbolic.taylor_arg_count",
                vec![("actual".to_string(), args.len().to_string())],
            ));
        }
        let expr = ast_to_symbolic(&args[0])?;
        let var = extract_var_name(&args[1])?;
        let order = extract_number(&args[2])? as u32;
        math_sym::taylor(&expr, &var, order)
    }
}

/// 递归检查 AST 是否含符号函数调用。
fn contains_symbolic_function(ast: &AstNode) -> bool {
    match ast {
        AstNode::FunctionCall(name, args) => {
            SYMBOLIC_FUNCTIONS.contains(&name.as_str())
                || args.iter().any(contains_symbolic_function)
        }
        AstNode::BinaryOp(_, l, r) => {
            contains_symbolic_function(l) || contains_symbolic_function(r)
        }
        AstNode::UnaryOp(_, e) => contains_symbolic_function(e),
        _ => false,
    }
}

/// 从 AST 提取变量名（Variable 节点）。
fn extract_var_name(ast: &AstNode) -> Result<String, CalcError> {
    match ast {
        AstNode::Variable(name) => Ok(name.clone()),
        _ => Err(
            CalcError::domain(format!("expected variable name, got: {:?}", ast)).with_i18n(
                "msg.symbolic.expected_variable_name",
                vec![("node".to_string(), format!("{:?}", ast))],
            ),
        ),
    }
}

/// 从 AST 提取数值（Number 节点）。
fn extract_number(ast: &AstNode) -> Result<f64, CalcError> {
    match ast {
        AstNode::Number(n) => Ok(*n),
        AstNode::BigNumber(s) => s.parse::<f64>().map_err(|_| {
            CalcError::domain(format!("invalid big number: {}", s)).with_i18n(
                "msg.invalid_bignumber",
                vec![("value".to_string(), s.to_string())],
            )
        }),
        AstNode::UnaryOp(UnaryOp::Neg, e) => Ok(-extract_number(e)?),
        _ => Err(
            CalcError::domain(format!("expected number, got: {:?}", ast)).with_i18n(
                "msg.symbolic.expected_number",
                vec![("node".to_string(), format!("{:?}", ast))],
            ),
        ),
    }
}

// ============================ 单元测试 (TG3.9) ============================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::parse;
    use crate::core::ErrorKind;
    use crate::math::symbolic as math_sym;

    // ----- TG3.1 转换测试 -----

    #[test]
    fn test_ast_to_symbolic_number() {
        let ast = parse("42").unwrap();
        let sym = ast_to_symbolic(&ast).unwrap();
        assert_eq!(sym, SymbolicExpr::Const(42.0));
    }

    #[test]
    fn test_ast_to_symbolic_variable() {
        let ast = parse("x").unwrap();
        let sym = ast_to_symbolic(&ast).unwrap();
        assert_eq!(sym, SymbolicExpr::Var("x".to_string()));
    }

    #[test]
    fn test_ast_to_symbolic_arithmetic() {
        let ast = parse("2*x+3").unwrap();
        let sym = ast_to_symbolic(&ast).unwrap();
        let expected = SymbolicExpr::Add(
            Box::new(SymbolicExpr::Mul(
                Box::new(SymbolicExpr::Const(2.0)),
                Box::new(SymbolicExpr::Var("x".to_string())),
            )),
            Box::new(SymbolicExpr::Const(3.0)),
        );
        assert_eq!(sym, expected);
    }

    #[test]
    fn test_ast_to_symbolic_function() {
        let ast = parse("sin(x)").unwrap();
        let sym = ast_to_symbolic(&ast).unwrap();
        assert_eq!(
            sym,
            SymbolicExpr::Sin(Box::new(SymbolicExpr::Var("x".to_string())))
        );
    }

    #[test]
    fn test_symbolic_to_string_basic() {
        let sym = SymbolicExpr::Add(
            Box::new(SymbolicExpr::Const(2.0)),
            Box::new(SymbolicExpr::Var("x".to_string())),
        );
        assert_eq!(math_sym::symbolic_to_string(&sym), "2+x");
    }

    // ----- TG3.2 求导测试 -----

    #[test]
    fn test_diff_power_rule() {
        let expr = SymbolicExpr::Pow(
            Box::new(SymbolicExpr::Var("x".to_string())),
            Box::new(SymbolicExpr::Const(3.0)),
        );
        let result = math_sym::simplify(&math_sym::diff(&expr, "x"));
        assert_eq!(math_sym::symbolic_to_string(&result), "3*x^2");
    }

    #[test]
    fn test_diff_trig_sin() {
        let expr = SymbolicExpr::Sin(Box::new(SymbolicExpr::Var("x".to_string())));
        let result = math_sym::simplify(&math_sym::diff(&expr, "x"));
        assert_eq!(math_sym::symbolic_to_string(&result), "cos(x)");
    }

    #[test]
    fn test_diff_trig_cos() {
        let expr = SymbolicExpr::Cos(Box::new(SymbolicExpr::Var("x".to_string())));
        let result = math_sym::simplify(&math_sym::diff(&expr, "x"));
        assert_eq!(math_sym::symbolic_to_string(&result), "-sin(x)");
    }

    #[test]
    fn test_diff_exp() {
        let expr = SymbolicExpr::Exp(Box::new(SymbolicExpr::Var("x".to_string())));
        let result = math_sym::simplify(&math_sym::diff(&expr, "x"));
        assert_eq!(math_sym::symbolic_to_string(&result), "exp(x)");
    }

    #[test]
    fn test_diff_ln() {
        let expr = SymbolicExpr::Ln(Box::new(SymbolicExpr::Var("x".to_string())));
        let result = math_sym::simplify(&math_sym::diff(&expr, "x"));
        assert_eq!(math_sym::symbolic_to_string(&result), "1/x");
    }

    #[test]
    fn test_diff_chain_rule() {
        let expr = SymbolicExpr::Sin(Box::new(SymbolicExpr::Pow(
            Box::new(SymbolicExpr::Var("x".to_string())),
            Box::new(SymbolicExpr::Const(2.0)),
        )));
        let result = math_sym::simplify(&math_sym::diff(&expr, "x"));
        let s = math_sym::symbolic_to_string(&result);
        assert!(s.contains("cos(x^2)"), "expected cos(x^2) in: {}", s);
        assert!(s.contains("2*x"), "expected 2*x in: {}", s);
    }

    #[test]
    fn test_diff_constant() {
        let expr = SymbolicExpr::Const(5.0);
        let result = math_sym::diff(&expr, "x");
        assert_eq!(result, SymbolicExpr::Const(0.0));
    }

    #[test]
    fn test_diff_product_rule() {
        let expr = SymbolicExpr::Mul(
            Box::new(SymbolicExpr::Var("x".to_string())),
            Box::new(SymbolicExpr::Sin(Box::new(SymbolicExpr::Var(
                "x".to_string(),
            )))),
        );
        let result = math_sym::simplify(&math_sym::diff(&expr, "x"));
        let s = math_sym::symbolic_to_string(&result);
        assert!(s.contains("sin(x)"), "expected sin(x) in: {}", s);
        assert!(s.contains("cos(x)"), "expected cos(x) in: {}", s);
    }

    // ----- TG3.3 积分测试 -----

    #[test]
    fn test_integrate_power() {
        let expr = SymbolicExpr::Pow(
            Box::new(SymbolicExpr::Var("x".to_string())),
            Box::new(SymbolicExpr::Const(2.0)),
        );
        let result = math_sym::simplify(&math_sym::integrate(&expr, "x").unwrap());
        assert_eq!(math_sym::symbolic_to_string(&result), "x^3/3");
    }

    #[test]
    fn test_integrate_sin() {
        let expr = SymbolicExpr::Sin(Box::new(SymbolicExpr::Var("x".to_string())));
        let result = math_sym::simplify(&math_sym::integrate(&expr, "x").unwrap());
        assert_eq!(math_sym::symbolic_to_string(&result), "-cos(x)");
    }

    #[test]
    fn test_integrate_cos() {
        let expr = SymbolicExpr::Cos(Box::new(SymbolicExpr::Var("x".to_string())));
        let result = math_sym::simplify(&math_sym::integrate(&expr, "x").unwrap());
        assert_eq!(math_sym::symbolic_to_string(&result), "sin(x)");
    }

    #[test]
    fn test_integrate_exp() {
        let expr = SymbolicExpr::Exp(Box::new(SymbolicExpr::Var("x".to_string())));
        let result = math_sym::simplify(&math_sym::integrate(&expr, "x").unwrap());
        assert_eq!(math_sym::symbolic_to_string(&result), "exp(x)");
    }

    #[test]
    fn test_integrate_one_over_x() {
        let expr = SymbolicExpr::Div(
            Box::new(SymbolicExpr::Const(1.0)),
            Box::new(SymbolicExpr::Var("x".to_string())),
        );
        let result = math_sym::simplify(&math_sym::integrate(&expr, "x").unwrap());
        assert_eq!(math_sym::symbolic_to_string(&result), "ln(x)");
    }

    #[test]
    fn test_integrate_unsupported_returns_error() {
        let expr = SymbolicExpr::Mul(
            Box::new(SymbolicExpr::Sin(Box::new(SymbolicExpr::Var(
                "x".to_string(),
            )))),
            Box::new(SymbolicExpr::Cos(Box::new(SymbolicExpr::Var(
                "x".to_string(),
            )))),
        );
        let result = math_sym::integrate(&expr, "x");
        assert!(result.is_err());
    }

    // ----- TG3.4 化简测试 -----

    #[test]
    fn test_simplify_add_zero() {
        let expr = SymbolicExpr::Add(
            Box::new(SymbolicExpr::Const(0.0)),
            Box::new(SymbolicExpr::Var("x".to_string())),
        );
        let result = math_sym::simplify(&expr);
        assert_eq!(result, SymbolicExpr::Var("x".to_string()));
    }

    #[test]
    fn test_simplify_mul_one() {
        let expr = SymbolicExpr::Mul(
            Box::new(SymbolicExpr::Const(1.0)),
            Box::new(SymbolicExpr::Var("x".to_string())),
        );
        let result = math_sym::simplify(&expr);
        assert_eq!(result, SymbolicExpr::Var("x".to_string()));
    }

    #[test]
    fn test_simplify_mul_zero() {
        let expr = SymbolicExpr::Mul(
            Box::new(SymbolicExpr::Var("x".to_string())),
            Box::new(SymbolicExpr::Const(0.0)),
        );
        let result = math_sym::simplify(&expr);
        assert_eq!(result, SymbolicExpr::Const(0.0));
    }

    #[test]
    fn test_simplify_pow_zero() {
        let expr = SymbolicExpr::Pow(
            Box::new(SymbolicExpr::Var("x".to_string())),
            Box::new(SymbolicExpr::Const(0.0)),
        );
        let result = math_sym::simplify(&expr);
        assert_eq!(result, SymbolicExpr::Const(1.0));
    }

    #[test]
    fn test_simplify_pow_one() {
        let expr = SymbolicExpr::Pow(
            Box::new(SymbolicExpr::Var("x".to_string())),
            Box::new(SymbolicExpr::Const(1.0)),
        );
        let result = math_sym::simplify(&expr);
        assert_eq!(result, SymbolicExpr::Var("x".to_string()));
    }

    #[test]
    fn test_simplify_constant_folding() {
        let expr = SymbolicExpr::Add(
            Box::new(SymbolicExpr::Const(2.0)),
            Box::new(SymbolicExpr::Const(3.0)),
        );
        let result = math_sym::simplify(&expr);
        assert_eq!(result, SymbolicExpr::Const(5.0));
    }

    #[test]
    fn test_simplify_nested() {
        let expr = SymbolicExpr::Mul(
            Box::new(SymbolicExpr::Add(
                Box::new(SymbolicExpr::Const(2.0)),
                Box::new(SymbolicExpr::Const(3.0)),
            )),
            Box::new(SymbolicExpr::Const(1.0)),
        );
        let result = math_sym::simplify(&expr);
        assert_eq!(result, SymbolicExpr::Const(5.0));
    }

    // ----- TG3.5 极限测试 -----

    #[test]
    fn test_limit_direct_substitution() {
        let expr = SymbolicExpr::Pow(
            Box::new(SymbolicExpr::Var("x".to_string())),
            Box::new(SymbolicExpr::Const(2.0)),
        );
        let result = math_sym::limit(&expr, "x", 3.0).unwrap();
        assert_eq!(result, EvalResult::Scalar(9.0));
    }

    #[test]
    fn test_limit_lhopital_zero_over_zero() {
        let expr = SymbolicExpr::Div(
            Box::new(SymbolicExpr::Sin(Box::new(SymbolicExpr::Var(
                "x".to_string(),
            )))),
            Box::new(SymbolicExpr::Var("x".to_string())),
        );
        let result = math_sym::limit(&expr, "x", 0.0).unwrap();
        if let EvalResult::Scalar(v) = result {
            assert!((v - 1.0).abs() < 1e-9, "expected 1.0, got {}", v);
        } else {
            panic!("expected Scalar");
        }
    }

    #[test]
    fn test_limit_polynomial() {
        let expr = SymbolicExpr::Div(
            Box::new(SymbolicExpr::Sub(
                Box::new(SymbolicExpr::Pow(
                    Box::new(SymbolicExpr::Var("x".to_string())),
                    Box::new(SymbolicExpr::Const(2.0)),
                )),
                Box::new(SymbolicExpr::Const(1.0)),
            )),
            Box::new(SymbolicExpr::Sub(
                Box::new(SymbolicExpr::Var("x".to_string())),
                Box::new(SymbolicExpr::Const(1.0)),
            )),
        );
        let result = math_sym::limit(&expr, "x", 1.0).unwrap();
        if let EvalResult::Scalar(v) = result {
            assert!((v - 2.0).abs() < 1e-9, "expected 2.0, got {}", v);
        } else {
            panic!("expected Scalar");
        }
    }

    // ----- TG3.6 泰勒级数测试 -----

    #[test]
    fn test_taylor_exp() {
        let expr = SymbolicExpr::Exp(Box::new(SymbolicExpr::Var("x".to_string())));
        let result = math_sym::taylor(&expr, "x", 3).unwrap();
        if let EvalResult::Symbolic(s) = result {
            assert!(s.contains("1"), "expected 1 in: {}", s);
            assert!(s.contains("0.5*x^2"), "expected 0.5*x^2 in: {}", s);
            assert!(s.contains("x^3"), "expected x^3 term in: {}", s);
        } else {
            panic!("expected Symbolic");
        }
    }

    #[test]
    fn test_taylor_sin() {
        let expr = SymbolicExpr::Sin(Box::new(SymbolicExpr::Var("x".to_string())));
        let result = math_sym::taylor(&expr, "x", 5).unwrap();
        if let EvalResult::Symbolic(s) = result {
            assert!(s.contains("x^3"), "expected x^3 term in: {}", s);
            assert!(s.contains("x^5"), "expected x^5 term in: {}", s);
        } else {
            panic!("expected Symbolic");
        }
    }

    #[test]
    fn test_taylor_order_exceeds_max() {
        let expr = SymbolicExpr::Exp(Box::new(SymbolicExpr::Var("x".to_string())));
        let result = math_sym::taylor(&expr, "x", 21);
        assert!(result.is_err());
    }

    // ----- TG3.7 路由测试 -----

    #[test]
    fn test_domain_name_and_priority() {
        let domain = SymbolicDomain;
        assert_eq!(domain.domain_name(), "symbolic");
        assert_eq!(domain.priority(), 30);
    }

    #[test]
    fn test_supports_diff() {
        let domain = SymbolicDomain;
        let ast = parse("diff(x^2, x)").unwrap();
        assert!(domain.supports(&ast));
    }

    #[test]
    fn test_supports_not_arithmetic() {
        let domain = SymbolicDomain;
        let ast = parse("2+3").unwrap();
        assert!(!domain.supports(&ast));
    }

    #[test]
    fn test_supports_nested() {
        let domain = SymbolicDomain;
        let ast = parse("2+diff(x,x)").unwrap();
        assert!(domain.supports(&ast));
    }

    // ----- TG3.7 端到端 evaluate 测试 -----

    #[test]
    fn test_evaluate_diff_power() {
        let domain = SymbolicDomain;
        let ast = parse("diff(x^2, x)").unwrap();
        let result = domain.evaluate(&ast, &EvalContext::default()).unwrap();
        if let EvalResult::Symbolic(s) = result {
            assert_eq!(s, "2*x");
        } else {
            panic!("expected Symbolic");
        }
    }

    #[test]
    fn test_evaluate_simplify() {
        let domain = SymbolicDomain;
        let ast = parse("simplify(x^2+2*x^2)").unwrap();
        let result = domain.evaluate(&ast, &EvalContext::default()).unwrap();
        if let EvalResult::Symbolic(s) = result {
            assert!(s.contains("3*x^2"), "expected 3*x^2 in: {}", s);
        } else {
            panic!("expected Symbolic");
        }
    }

    #[test]
    fn test_evaluate_limit() {
        let domain = SymbolicDomain;
        let ast = parse("limit(sin(x)/x, x, 0)").unwrap();
        let result = domain.evaluate(&ast, &EvalContext::default()).unwrap();
        if let EvalResult::Scalar(v) = result {
            assert!((v - 1.0).abs() < 1e-9, "expected 1.0, got {}", v);
        } else {
            panic!("expected Scalar");
        }
    }

    #[test]
    fn test_evaluate_taylor() {
        let domain = SymbolicDomain;
        let ast = parse("taylor(exp(x), x, 2)").unwrap();
        let result = domain.evaluate(&ast, &EvalContext::default()).unwrap();
        if let EvalResult::Symbolic(s) = result {
            assert!(s.contains("1"), "expected 1 in: {}", s);
            assert!(s.contains("0.5*x^2"), "expected 0.5*x^2 in: {}", s);
        } else {
            panic!("expected Symbolic");
        }
    }

    #[test]
    fn test_evaluate_integrate() {
        let domain = SymbolicDomain;
        let ast = parse("integrate(x^2, x)").unwrap();
        let result = domain.evaluate(&ast, &EvalContext::default()).unwrap();
        if let EvalResult::Symbolic(s) = result {
            assert_eq!(s, "x^3/3");
        } else {
            panic!("expected Symbolic");
        }
    }

    #[test]
    fn test_evaluate_unsupported_function() {
        let domain = SymbolicDomain;
        let ast = AstNode::FunctionCall("foo".to_string(), vec![]);
        let result = domain.evaluate(&ast, &EvalContext::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_evaluate_diff_wrong_arg_count() {
        let domain = SymbolicDomain;
        let ast = AstNode::FunctionCall("diff".to_string(), vec![AstNode::Number(1.0)]);
        let result = domain.evaluate(&ast, &EvalContext::default());
        assert!(result.is_err());
    }

    // ----- 辅助函数测试 (via math_sym) -----

    #[test]
    fn test_eval_symbolic_basic() {
        let mut env = HashMap::new();
        env.insert("x".to_string(), 3.0);
        let expr = SymbolicExpr::Add(
            Box::new(SymbolicExpr::Var("x".to_string())),
            Box::new(SymbolicExpr::Const(2.0)),
        );
        assert_eq!(math_sym::eval_symbolic(&expr, &env).unwrap(), 5.0);
    }

    // ----- TG3.10 proptest 属性测试 -----

    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig { cases: 128, ..ProptestConfig::default() })]

        #[test]
        fn prop_diff_linearity(a in -10.0f64..10.0, b in -10.0f64..10.0) {
            let f = SymbolicExpr::Mul(
                Box::new(SymbolicExpr::Const(a)),
                Box::new(SymbolicExpr::Var("x".to_string())),
            );
            let g = SymbolicExpr::Mul(
                Box::new(SymbolicExpr::Const(b)),
                Box::new(SymbolicExpr::Var("x".to_string())),
            );
            let sum = SymbolicExpr::Add(Box::new(f.clone()), Box::new(g.clone()));
            let d_sum = math_sym::simplify(&math_sym::diff(&sum, "x"));
            let d_f = math_sym::simplify(&math_sym::diff(&f, "x"));
            let d_g = math_sym::simplify(&math_sym::diff(&g, "x"));
            let expected = math_sym::simplify(&SymbolicExpr::Add(Box::new(d_f), Box::new(d_g)));
            prop_assert_eq!(d_sum, expected);
        }

        #[test]
        fn prop_simplify_idempotent(c in -5.0f64..5.0) {
            let expr = SymbolicExpr::Add(
                Box::new(SymbolicExpr::Const(c)),
                Box::new(SymbolicExpr::Mul(
                    Box::new(SymbolicExpr::Const(1.0)),
                    Box::new(SymbolicExpr::Var("x".to_string())),
                )),
            );
            let once = math_sym::simplify(&expr);
            let twice = math_sym::simplify(&once);
            prop_assert_eq!(once, twice);
        }

        #[test]
        fn prop_diff_constant_is_zero(c in -100.0f64..100.0) {
            let expr = SymbolicExpr::Const(c);
            let result = math_sym::diff(&expr, "x");
            prop_assert_eq!(result, SymbolicExpr::Const(0.0));
        }
    }

    // ===== 补充覆盖测试：ast_to_symbolic 错误/边界路径 =====

    #[test]
    fn test_ast_to_symbolic_big_number_valid() {
        let ast = AstNode::BigNumber("12345".to_string());
        let sym = ast_to_symbolic(&ast).unwrap();
        assert_eq!(sym, SymbolicExpr::Const(12345.0));
    }

    #[test]
    fn test_ast_to_symbolic_big_number_invalid() {
        let ast = AstNode::BigNumber("not_a_number".to_string());
        assert!(ast_to_symbolic(&ast).is_err());
    }

    #[test]
    fn test_ast_to_symbolic_mod_error() {
        let ast = AstNode::BinaryOp(
            BinaryOp::Mod,
            Box::new(AstNode::Number(5.0)),
            Box::new(AstNode::Number(3.0)),
        );
        assert!(ast_to_symbolic(&ast).is_err());
    }

    #[test]
    fn test_ast_to_symbolic_abs_error() {
        let ast = AstNode::UnaryOp(UnaryOp::Abs, Box::new(AstNode::Number(5.0)));
        assert!(ast_to_symbolic(&ast).is_err());
    }

    #[test]
    fn test_ast_to_symbolic_factorial_error() {
        let ast = AstNode::UnaryOp(UnaryOp::Factorial, Box::new(AstNode::Number(5.0)));
        assert!(ast_to_symbolic(&ast).is_err());
    }

    #[test]
    fn test_ast_to_symbolic_unknown_function() {
        let ast = AstNode::FunctionCall("unknown".to_string(), vec![AstNode::Number(1.0)]);
        assert!(ast_to_symbolic(&ast).is_err());
    }

    #[test]
    fn test_ast_to_symbolic_complex_error() {
        let ast = AstNode::Complex(1.0, 2.0);
        assert!(ast_to_symbolic(&ast).is_err());
    }

    #[test]
    fn test_ast_to_symbolic_matrix_error() {
        let ast = AstNode::Matrix(vec![vec![AstNode::Number(1.0)]]);
        assert!(ast_to_symbolic(&ast).is_err());
    }

    #[test]
    fn test_ast_to_symbolic_list_error() {
        let ast = AstNode::List(vec![AstNode::Number(1.0)]);
        assert!(ast_to_symbolic(&ast).is_err());
    }

    #[test]
    fn test_ast_to_symbolic_wrong_arg_count() {
        let ast = AstNode::FunctionCall(
            "sin".to_string(),
            vec![AstNode::Number(1.0), AstNode::Number(2.0)],
        );
        assert!(ast_to_symbolic(&ast).is_err());
        let ast = AstNode::FunctionCall(
            "cos".to_string(),
            vec![AstNode::Number(1.0), AstNode::Number(2.0)],
        );
        assert!(ast_to_symbolic(&ast).is_err());
        let ast = AstNode::FunctionCall(
            "ln".to_string(),
            vec![AstNode::Number(1.0), AstNode::Number(2.0)],
        );
        assert!(ast_to_symbolic(&ast).is_err());
        let ast = AstNode::FunctionCall(
            "tan".to_string(),
            vec![AstNode::Number(1.0), AstNode::Number(2.0)],
        );
        assert!(ast_to_symbolic(&ast).is_err());
        let ast = AstNode::FunctionCall(
            "exp".to_string(),
            vec![AstNode::Number(1.0), AstNode::Number(2.0)],
        );
        assert!(ast_to_symbolic(&ast).is_err());
    }

    #[test]
    fn test_ast_to_symbolic_sub_div_pow_neg() {
        let ast = AstNode::BinaryOp(
            BinaryOp::Sub,
            Box::new(AstNode::Number(5.0)),
            Box::new(AstNode::Number(3.0)),
        );
        assert_eq!(
            ast_to_symbolic(&ast).unwrap(),
            SymbolicExpr::Sub(
                Box::new(SymbolicExpr::Const(5.0)),
                Box::new(SymbolicExpr::Const(3.0)),
            )
        );
        let ast = AstNode::BinaryOp(
            BinaryOp::Div,
            Box::new(AstNode::Number(6.0)),
            Box::new(AstNode::Number(2.0)),
        );
        assert_eq!(
            ast_to_symbolic(&ast).unwrap(),
            SymbolicExpr::Div(
                Box::new(SymbolicExpr::Const(6.0)),
                Box::new(SymbolicExpr::Const(2.0)),
            )
        );
        let ast = AstNode::BinaryOp(
            BinaryOp::Pow,
            Box::new(AstNode::Number(2.0)),
            Box::new(AstNode::Number(3.0)),
        );
        assert_eq!(
            ast_to_symbolic(&ast).unwrap(),
            SymbolicExpr::Pow(
                Box::new(SymbolicExpr::Const(2.0)),
                Box::new(SymbolicExpr::Const(3.0)),
            )
        );
        let ast = AstNode::UnaryOp(UnaryOp::Neg, Box::new(AstNode::Number(5.0)));
        assert_eq!(
            ast_to_symbolic(&ast).unwrap(),
            SymbolicExpr::Neg(Box::new(SymbolicExpr::Const(5.0)))
        );
    }

    #[test]
    fn test_ast_to_symbolic_tan_ln_exp_log() {
        let v = SymbolicExpr::Var("x".to_string());
        let ast_v = AstNode::Variable("x".to_string());
        assert_eq!(
            ast_to_symbolic(&AstNode::FunctionCall("tan".to_string(), vec![ast_v.clone()])).unwrap(),
            SymbolicExpr::Tan(Box::new(v.clone()))
        );
        assert_eq!(
            ast_to_symbolic(&AstNode::FunctionCall("ln".to_string(), vec![ast_v.clone()])).unwrap(),
            SymbolicExpr::Ln(Box::new(v.clone()))
        );
        assert_eq!(
            ast_to_symbolic(&AstNode::FunctionCall("exp".to_string(), vec![ast_v.clone()])).unwrap(),
            SymbolicExpr::Exp(Box::new(v.clone()))
        );
        assert_eq!(
            ast_to_symbolic(&AstNode::FunctionCall("log".to_string(), vec![ast_v])).unwrap(),
            SymbolicExpr::Ln(Box::new(v))
        );
    }

    #[test]
    fn test_ast_to_symbolic_pi_e_constants() {
        let ast = AstNode::Variable("pi".to_string());
        assert_eq!(
            ast_to_symbolic(&ast).unwrap(),
            SymbolicExpr::Const(std::f64::consts::PI)
        );
        let ast = AstNode::Variable("e".to_string());
        assert_eq!(
            ast_to_symbolic(&ast).unwrap(),
            SymbolicExpr::Const(std::f64::consts::E)
        );
    }

    // ===== 补充覆盖测试：symbolic_to_string 格式化分支 (via math_sym) =====

    #[test]
    fn test_symbolic_to_string_sub_with_add_parens() {
        let expr = SymbolicExpr::Sub(
            Box::new(SymbolicExpr::Var("a".to_string())),
            Box::new(SymbolicExpr::Add(
                Box::new(SymbolicExpr::Var("b".to_string())),
                Box::new(SymbolicExpr::Var("c".to_string())),
            )),
        );
        assert_eq!(math_sym::symbolic_to_string(&expr), "a-(b+c)");
    }

    #[test]
    fn test_symbolic_to_string_sub_with_sub_parens() {
        let expr = SymbolicExpr::Sub(
            Box::new(SymbolicExpr::Var("a".to_string())),
            Box::new(SymbolicExpr::Sub(
                Box::new(SymbolicExpr::Var("b".to_string())),
                Box::new(SymbolicExpr::Var("c".to_string())),
            )),
        );
        assert_eq!(math_sym::symbolic_to_string(&expr), "a-(b-c)");
    }

    #[test]
    fn test_symbolic_to_string_sub_plain() {
        let expr = SymbolicExpr::Sub(
            Box::new(SymbolicExpr::Var("a".to_string())),
            Box::new(SymbolicExpr::Var("b".to_string())),
        );
        assert_eq!(math_sym::symbolic_to_string(&expr), "a-b");
    }

    #[test]
    fn test_symbolic_to_string_pow_complex_operands() {
        let expr = SymbolicExpr::Pow(
            Box::new(SymbolicExpr::Add(
                Box::new(SymbolicExpr::Var("x".to_string())),
                Box::new(SymbolicExpr::Const(1.0)),
            )),
            Box::new(SymbolicExpr::Sub(
                Box::new(SymbolicExpr::Var("x".to_string())),
                Box::new(SymbolicExpr::Const(1.0)),
            )),
        );
        assert_eq!(math_sym::symbolic_to_string(&expr), "(x+1)^(x-1)");
    }

    #[test]
    fn test_symbolic_to_string_pow_simple_operands() {
        let expr = SymbolicExpr::Pow(
            Box::new(SymbolicExpr::Var("x".to_string())),
            Box::new(SymbolicExpr::Const(2.0)),
        );
        assert_eq!(math_sym::symbolic_to_string(&expr), "x^2");
    }

    #[test]
    fn test_symbolic_to_string_neg_with_add() {
        let expr = SymbolicExpr::Neg(Box::new(SymbolicExpr::Add(
            Box::new(SymbolicExpr::Var("x".to_string())),
            Box::new(SymbolicExpr::Const(1.0)),
        )));
        assert_eq!(math_sym::symbolic_to_string(&expr), "-(x+1)");
    }

    #[test]
    fn test_symbolic_to_string_neg_with_sub() {
        let expr = SymbolicExpr::Neg(Box::new(SymbolicExpr::Sub(
            Box::new(SymbolicExpr::Var("x".to_string())),
            Box::new(SymbolicExpr::Const(1.0)),
        )));
        assert_eq!(math_sym::symbolic_to_string(&expr), "-(x-1)");
    }

    #[test]
    fn test_symbolic_to_string_neg_simple() {
        let expr = SymbolicExpr::Neg(Box::new(SymbolicExpr::Var("x".to_string())));
        assert_eq!(math_sym::symbolic_to_string(&expr), "-x");
    }

    #[test]
    fn test_symbolic_to_string_div_and_mul_with_parens() {
        let expr = SymbolicExpr::Div(
            Box::new(SymbolicExpr::Add(
                Box::new(SymbolicExpr::Var("x".to_string())),
                Box::new(SymbolicExpr::Const(1.0)),
            )),
            Box::new(SymbolicExpr::Sub(
                Box::new(SymbolicExpr::Var("x".to_string())),
                Box::new(SymbolicExpr::Const(1.0)),
            )),
        );
        assert_eq!(math_sym::symbolic_to_string(&expr), "(x+1)/(x-1)");
        let expr = SymbolicExpr::Mul(
            Box::new(SymbolicExpr::Add(
                Box::new(SymbolicExpr::Var("x".to_string())),
                Box::new(SymbolicExpr::Const(1.0)),
            )),
            Box::new(SymbolicExpr::Var("x".to_string())),
        );
        assert_eq!(math_sym::symbolic_to_string(&expr), "(x+1)*x");
    }

    #[test]
    fn test_symbolic_to_string_tan_ln_exp() {
        assert_eq!(
            math_sym::symbolic_to_string(&SymbolicExpr::Tan(Box::new(SymbolicExpr::Var(
                "x".to_string()
            )))),
            "tan(x)"
        );
        assert_eq!(
            math_sym::symbolic_to_string(&SymbolicExpr::Ln(Box::new(SymbolicExpr::Var(
                "x".to_string()
            )))),
            "ln(x)"
        );
        assert_eq!(
            math_sym::symbolic_to_string(&SymbolicExpr::Exp(Box::new(SymbolicExpr::Var(
                "x".to_string()
            )))),
            "exp(x)"
        );
    }

    // ===== 补充覆盖测试：diff 边界路径 (via math_sym) =====

    #[test]
    fn test_diff_sub() {
        let expr = SymbolicExpr::Sub(
            Box::new(SymbolicExpr::Var("x".to_string())),
            Box::new(SymbolicExpr::Const(3.0)),
        );
        let result = math_sym::simplify(&math_sym::diff(&expr, "x"));
        assert_eq!(math_sym::symbolic_to_string(&result), "1");
    }

    #[test]
    fn test_diff_neg() {
        let expr = SymbolicExpr::Neg(Box::new(SymbolicExpr::Var("x".to_string())));
        let result = math_sym::simplify(&math_sym::diff(&expr, "x"));
        assert_eq!(math_sym::symbolic_to_string(&result), "-1");
    }

    #[test]
    fn test_diff_div_quotient_rule() {
        let expr = SymbolicExpr::Div(
            Box::new(SymbolicExpr::Var("x".to_string())),
            Box::new(SymbolicExpr::Add(
                Box::new(SymbolicExpr::Var("x".to_string())),
                Box::new(SymbolicExpr::Const(1.0)),
            )),
        );
        let result = math_sym::diff(&expr, "x");
        assert!(matches!(result, SymbolicExpr::Div(_, _)));
        let s = math_sym::symbolic_to_string(&math_sym::simplify(&result));
        assert!(s.contains("(x+1)^2"), "expected (x+1)^2 in: {}", s);
    }

    #[test]
    fn test_diff_pow_non_constant_exponent() {
        let expr = SymbolicExpr::Pow(
            Box::new(SymbolicExpr::Var("x".to_string())),
            Box::new(SymbolicExpr::Var("x".to_string())),
        );
        let result = math_sym::diff(&expr, "x");
        assert!(matches!(result, SymbolicExpr::Mul(_, _)));
        let s = math_sym::symbolic_to_string(&result);
        assert!(s.contains("ln(x)"), "expected ln(x) in: {}", s);
    }

    #[test]
    fn test_diff_tan() {
        let expr = SymbolicExpr::Tan(Box::new(SymbolicExpr::Var("x".to_string())));
        let result = math_sym::diff(&expr, "x");
        let s = math_sym::symbolic_to_string(&result);
        assert!(s.contains("cos(x)"), "expected cos(x) in: {}", s);
        assert!(s.contains("^2"), "expected ^2 in: {}", s);
    }

    #[test]
    fn test_diff_ln_chain_rule() {
        let expr = SymbolicExpr::Ln(Box::new(SymbolicExpr::Pow(
            Box::new(SymbolicExpr::Var("x".to_string())),
            Box::new(SymbolicExpr::Const(2.0)),
        )));
        let result = math_sym::simplify(&math_sym::diff(&expr, "x"));
        let s = math_sym::symbolic_to_string(&result);
        assert!(s.contains("1/x^2"), "expected 1/x^2 in: {}", s);
        assert!(s.contains("2*x"), "expected 2*x in: {}", s);
    }

    #[test]
    fn test_diff_variable_not_matching() {
        let expr = SymbolicExpr::Var("y".to_string());
        let result = math_sym::diff(&expr, "x");
        assert_eq!(result, SymbolicExpr::Const(0.0));
    }

    // ===== 补充覆盖测试：integrate 边界路径 (via math_sym) =====

    #[test]
    fn test_integrate_constant() {
        let expr = SymbolicExpr::Const(5.0);
        let result = math_sym::simplify(&math_sym::integrate(&expr, "x").unwrap());
        assert_eq!(math_sym::symbolic_to_string(&result), "5*x");
    }

    #[test]
    fn test_integrate_other_variable() {
        let expr = SymbolicExpr::Var("y".to_string());
        let result = math_sym::simplify(&math_sym::integrate(&expr, "x").unwrap());
        assert_eq!(math_sym::symbolic_to_string(&result), "y*x");
    }

    #[test]
    fn test_integrate_sub() {
        let expr = SymbolicExpr::Sub(
            Box::new(SymbolicExpr::Var("x".to_string())),
            Box::new(SymbolicExpr::Const(3.0)),
        );
        let result = math_sym::simplify(&math_sym::integrate(&expr, "x").unwrap());
        let s = math_sym::symbolic_to_string(&result);
        assert!(s.contains("x^2/2"), "expected x^2/2 in: {}", s);
        assert!(s.contains("3*x"), "expected 3*x in: {}", s);
    }

    #[test]
    fn test_integrate_tan_unsupported() {
        let expr = SymbolicExpr::Tan(Box::new(SymbolicExpr::Var("x".to_string())));
        assert!(math_sym::integrate(&expr, "x").is_err());
    }

    #[test]
    fn test_integrate_ln_unsupported() {
        let expr = SymbolicExpr::Ln(Box::new(SymbolicExpr::Var("x".to_string())));
        assert!(math_sym::integrate(&expr, "x").is_err());
    }

    #[test]
    fn test_integrate_pow_non_variable_base() {
        let expr = SymbolicExpr::Pow(
            Box::new(SymbolicExpr::Add(
                Box::new(SymbolicExpr::Var("x".to_string())),
                Box::new(SymbolicExpr::Const(1.0)),
            )),
            Box::new(SymbolicExpr::Const(2.0)),
        );
        assert!(math_sym::integrate(&expr, "x").is_err());
    }

    #[test]
    fn test_integrate_pow_different_var_base() {
        let expr = SymbolicExpr::Pow(
            Box::new(SymbolicExpr::Var("y".to_string())),
            Box::new(SymbolicExpr::Const(2.0)),
        );
        assert!(math_sym::integrate(&expr, "x").is_err());
    }

    #[test]
    fn test_integrate_sin_non_var_error() {
        let expr = SymbolicExpr::Sin(Box::new(SymbolicExpr::Pow(
            Box::new(SymbolicExpr::Var("x".to_string())),
            Box::new(SymbolicExpr::Const(2.0)),
        )));
        assert!(math_sym::integrate(&expr, "x").is_err());
    }

    #[test]
    fn test_integrate_cos_non_var_error() {
        let expr = SymbolicExpr::Cos(Box::new(SymbolicExpr::Var("y".to_string())));
        assert!(math_sym::integrate(&expr, "x").is_err());
    }

    #[test]
    fn test_integrate_exp_non_var_error() {
        let expr = SymbolicExpr::Exp(Box::new(SymbolicExpr::Var("y".to_string())));
        assert!(math_sym::integrate(&expr, "x").is_err());
    }

    #[test]
    fn test_integrate_mul_with_const_right() {
        let expr = SymbolicExpr::Mul(
            Box::new(SymbolicExpr::Var("x".to_string())),
            Box::new(SymbolicExpr::Const(3.0)),
        );
        let result = math_sym::integrate(&expr, "x");
        assert!(result.is_ok());
        let s = math_sym::symbolic_to_string(&math_sym::simplify(&result.unwrap()));
        assert!(s.contains("3"), "expected 3 in: {}", s);
    }

    #[test]
    fn test_integrate_div_non_one_over_var_error() {
        let expr = SymbolicExpr::Div(
            Box::new(SymbolicExpr::Var("x".to_string())),
            Box::new(SymbolicExpr::Const(2.0)),
        );
        assert!(math_sym::integrate(&expr, "x").is_err());
    }

    #[test]
    fn test_integrate_neg() {
        let expr = SymbolicExpr::Neg(Box::new(SymbolicExpr::Sin(Box::new(SymbolicExpr::Var(
            "x".to_string(),
        )))));
        let result = math_sym::simplify(&math_sym::integrate(&expr, "x").unwrap());
        assert_eq!(math_sym::symbolic_to_string(&result), "cos(x)");
    }

    // ===== 补充覆盖测试：simplify 边界路径 (via math_sym) =====

    #[test]
    fn test_simplify_sub_zero_right() {
        let expr = SymbolicExpr::Sub(
            Box::new(SymbolicExpr::Var("x".to_string())),
            Box::new(SymbolicExpr::Const(0.0)),
        );
        assert_eq!(math_sym::simplify(&expr), SymbolicExpr::Var("x".to_string()));
    }

    #[test]
    fn test_simplify_sub_zero_left() {
        let expr = SymbolicExpr::Sub(
            Box::new(SymbolicExpr::Const(0.0)),
            Box::new(SymbolicExpr::Var("x".to_string())),
        );
        assert_eq!(
            math_sym::simplify(&expr),
            SymbolicExpr::Neg(Box::new(SymbolicExpr::Var("x".to_string())))
        );
    }

    #[test]
    fn test_simplify_sub_constant_folding() {
        let expr = SymbolicExpr::Sub(
            Box::new(SymbolicExpr::Const(5.0)),
            Box::new(SymbolicExpr::Const(3.0)),
        );
        assert_eq!(math_sym::simplify(&expr), SymbolicExpr::Const(2.0));
    }

    #[test]
    fn test_simplify_sub_combine_like_terms() {
        let expr = SymbolicExpr::Sub(
            Box::new(SymbolicExpr::Mul(
                Box::new(SymbolicExpr::Const(2.0)),
                Box::new(SymbolicExpr::Var("x".to_string())),
            )),
            Box::new(SymbolicExpr::Mul(
                Box::new(SymbolicExpr::Const(3.0)),
                Box::new(SymbolicExpr::Var("x".to_string())),
            )),
        );
        assert_eq!(
            math_sym::simplify(&expr),
            SymbolicExpr::Mul(
                Box::new(SymbolicExpr::Const(-1.0)),
                Box::new(SymbolicExpr::Var("x".to_string())),
            )
        );
    }

    #[test]
    fn test_simplify_div_one() {
        let expr = SymbolicExpr::Div(
            Box::new(SymbolicExpr::Var("x".to_string())),
            Box::new(SymbolicExpr::Const(1.0)),
        );
        assert_eq!(math_sym::simplify(&expr), SymbolicExpr::Var("x".to_string()));
    }

    #[test]
    fn test_simplify_div_zero_numerator() {
        let expr = SymbolicExpr::Div(
            Box::new(SymbolicExpr::Const(0.0)),
            Box::new(SymbolicExpr::Var("x".to_string())),
        );
        assert_eq!(math_sym::simplify(&expr), SymbolicExpr::Const(0.0));
    }

    #[test]
    fn test_simplify_div_constant_folding() {
        let expr = SymbolicExpr::Div(
            Box::new(SymbolicExpr::Const(6.0)),
            Box::new(SymbolicExpr::Const(2.0)),
        );
        assert_eq!(math_sym::simplify(&expr), SymbolicExpr::Const(3.0));
    }

    #[test]
    fn test_simplify_div_by_zero_kept() {
        let expr = SymbolicExpr::Div(
            Box::new(SymbolicExpr::Const(6.0)),
            Box::new(SymbolicExpr::Const(0.0)),
        );
        assert_eq!(
            math_sym::simplify(&expr),
            SymbolicExpr::Div(
                Box::new(SymbolicExpr::Const(6.0)),
                Box::new(SymbolicExpr::Const(0.0)),
            )
        );
    }

    #[test]
    fn test_simplify_neg_double_negation() {
        let expr = SymbolicExpr::Neg(Box::new(SymbolicExpr::Neg(Box::new(SymbolicExpr::Var(
            "x".to_string(),
        )))));
        assert_eq!(math_sym::simplify(&expr), SymbolicExpr::Var("x".to_string()));
    }

    #[test]
    fn test_simplify_neg_constant() {
        let expr = SymbolicExpr::Neg(Box::new(SymbolicExpr::Const(5.0)));
        assert_eq!(math_sym::simplify(&expr), SymbolicExpr::Const(-5.0));
    }

    #[test]
    fn test_simplify_pow_one_base() {
        let expr = SymbolicExpr::Pow(
            Box::new(SymbolicExpr::Const(1.0)),
            Box::new(SymbolicExpr::Var("x".to_string())),
        );
        assert_eq!(math_sym::simplify(&expr), SymbolicExpr::Const(1.0));
    }

    #[test]
    fn test_simplify_pow_constant_folding() {
        let expr = SymbolicExpr::Pow(
            Box::new(SymbolicExpr::Const(2.0)),
            Box::new(SymbolicExpr::Const(3.0)),
        );
        assert_eq!(math_sym::simplify(&expr), SymbolicExpr::Const(8.0));
    }

    #[test]
    fn test_simplify_add_combine_like_terms() {
        let expr = SymbolicExpr::Add(
            Box::new(SymbolicExpr::Mul(
                Box::new(SymbolicExpr::Const(2.0)),
                Box::new(SymbolicExpr::Var("x".to_string())),
            )),
            Box::new(SymbolicExpr::Mul(
                Box::new(SymbolicExpr::Const(3.0)),
                Box::new(SymbolicExpr::Var("x".to_string())),
            )),
        );
        assert_eq!(
            math_sym::simplify(&expr),
            SymbolicExpr::Mul(
                Box::new(SymbolicExpr::Const(5.0)),
                Box::new(SymbolicExpr::Var("x".to_string())),
            )
        );
    }

    #[test]
    fn test_simplify_nested_sub_div() {
        let expr = SymbolicExpr::Div(
            Box::new(SymbolicExpr::Sub(
                Box::new(SymbolicExpr::Mul(
                    Box::new(SymbolicExpr::Const(2.0)),
                    Box::new(SymbolicExpr::Var("x".to_string())),
                )),
                Box::new(SymbolicExpr::Const(0.0)),
            )),
            Box::new(SymbolicExpr::Const(1.0)),
        );
        assert_eq!(
            math_sym::simplify(&expr),
            SymbolicExpr::Mul(
                Box::new(SymbolicExpr::Const(2.0)),
                Box::new(SymbolicExpr::Var("x".to_string())),
            )
        );
    }

    #[test]
    fn test_simplify_neg_preserved() {
        let expr = SymbolicExpr::Neg(Box::new(SymbolicExpr::Var("x".to_string())));
        assert_eq!(
            math_sym::simplify(&expr),
            SymbolicExpr::Neg(Box::new(SymbolicExpr::Var("x".to_string())))
        );
    }

    // ===== TG9.2 补充覆盖：integrate/simplify/limit/taylor 未覆盖路径 =====

    #[test]
    fn test_integrate_add_linearity() {
        let expr = SymbolicExpr::Add(
            Box::new(SymbolicExpr::Var("x".to_string())),
            Box::new(SymbolicExpr::Const(3.0)),
        );
        let result = math_sym::simplify(&math_sym::integrate(&expr, "x").unwrap());
        let s = math_sym::symbolic_to_string(&result);
        assert!(s.contains("x^2/2"), "expected x^2/2 in: {}", s);
        assert!(s.contains("3*x"), "expected 3*x in: {}", s);
    }

    #[test]
    fn test_integrate_mul_const_left() {
        let expr = SymbolicExpr::Mul(
            Box::new(SymbolicExpr::Const(3.0)),
            Box::new(SymbolicExpr::Var("x".to_string())),
        );
        let result = math_sym::simplify(&math_sym::integrate(&expr, "x").unwrap());
        let s = math_sym::symbolic_to_string(&result);
        assert!(s.contains("3"), "expected 3 in: {}", s);
        assert!(s.contains("x^2/2"), "expected x^2/2 in: {}", s);
    }

    #[test]
    fn test_integrate_pow_neg_one() {
        let expr = SymbolicExpr::Pow(
            Box::new(SymbolicExpr::Var("x".to_string())),
            Box::new(SymbolicExpr::Const(-1.0)),
        );
        let result = math_sym::integrate(&expr, "x").unwrap();
        assert_eq!(
            result,
            SymbolicExpr::Ln(Box::new(SymbolicExpr::Var("x".to_string())))
        );
    }

    #[test]
    fn test_integrate_div_one_over_var() {
        let expr = SymbolicExpr::Div(
            Box::new(SymbolicExpr::Const(1.0)),
            Box::new(SymbolicExpr::Var("x".to_string())),
        );
        let result = math_sym::integrate(&expr, "x").unwrap();
        assert_eq!(
            result,
            SymbolicExpr::Ln(Box::new(SymbolicExpr::Var("x".to_string())))
        );
    }

    #[test]
    fn test_simplify_tan() {
        let expr = SymbolicExpr::Tan(Box::new(SymbolicExpr::Var("x".to_string())));
        let result = math_sym::simplify(&expr);
        assert_eq!(result, expr);
    }

    #[test]
    fn test_limit_denominator_derivative_zero() {
        let expr = SymbolicExpr::Div(
            Box::new(SymbolicExpr::Var("x".to_string())),
            Box::new(SymbolicExpr::Const(0.0)),
        );
        let result = math_sym::limit(&expr, "x", 0.0);
        assert!(result.is_err());
    }

    #[test]
    fn test_eval_symbolic_tan_and_ln() {
        let mut env = HashMap::new();
        env.insert("x".to_string(), 0.0);
        let tan_expr = SymbolicExpr::Tan(Box::new(SymbolicExpr::Var("x".to_string())));
        assert_eq!(math_sym::eval_symbolic(&tan_expr, &env).unwrap(), 0.0);

        env.insert("x".to_string(), -1.0);
        let ln_expr = SymbolicExpr::Ln(Box::new(SymbolicExpr::Var("x".to_string())));
        let ln_result = math_sym::eval_symbolic(&ln_expr, &env);
        assert!(matches!(ln_result, Err(e) if e.kind == ErrorKind::Domain));

        env.insert("x".to_string(), 0.0);
        let ln_zero_result = math_sym::eval_symbolic(&ln_expr, &env);
        assert!(matches!(ln_zero_result, Err(e) if e.kind == ErrorKind::Domain));

        env.insert("x".to_string(), 1.0);
        assert_eq!(math_sym::eval_symbolic(&ln_expr, &env).unwrap(), 0.0);
    }

    #[test]
    fn test_taylor_empty_terms() {
        let expr = SymbolicExpr::Const(0.0);
        let result = math_sym::taylor(&expr, "x", 3).unwrap();
        if let EvalResult::Symbolic(s) = result {
            assert_eq!(s, "0");
        } else {
            panic!("expected Symbolic");
        }
    }

    #[test]
    fn test_taylor_format_term_coeff_one() {
        let expr = SymbolicExpr::Var("x".to_string());
        let result = math_sym::taylor(&expr, "x", 1).unwrap();
        if let EvalResult::Symbolic(s) = result {
            assert_eq!(s, "x");
        } else {
            panic!("expected Symbolic");
        }

        let expr2 = SymbolicExpr::Pow(
            Box::new(SymbolicExpr::Var("x".to_string())),
            Box::new(SymbolicExpr::Const(2.0)),
        );
        let result2 = math_sym::taylor(&expr2, "x", 2).unwrap();
        if let EvalResult::Symbolic(s) = result2 {
            assert!(s.contains("x^2"), "expected x^2 in: {}", s);
            assert!(!s.contains("1*x"), "should not contain 1* prefix: {}", s);
        } else {
            panic!("expected Symbolic");
        }
    }

    #[test]
    fn test_symbolic_domain_default() {
        let domain = SymbolicDomain;
        assert_eq!(domain.domain_name(), "symbolic");
        assert_eq!(domain.priority(), 30);
    }

    #[test]
    fn test_eval_node_non_function_call() {
        let domain = SymbolicDomain;
        let ast = AstNode::Number(42.0);
        let result = domain.evaluate(&ast, &EvalContext::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_eval_integrate_wrong_arg_count() {
        let domain = SymbolicDomain;
        let ast = AstNode::FunctionCall("integrate".to_string(), vec![AstNode::Number(1.0)]);
        let result = domain.evaluate(&ast, &EvalContext::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_eval_simplify_wrong_arg_count() {
        let domain = SymbolicDomain;
        let ast = AstNode::FunctionCall(
            "simplify".to_string(),
            vec![AstNode::Number(1.0), AstNode::Number(2.0)],
        );
        let result = domain.evaluate(&ast, &EvalContext::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_eval_limit_wrong_arg_count() {
        let domain = SymbolicDomain;
        let ast = AstNode::FunctionCall(
            "limit".to_string(),
            vec![AstNode::Number(1.0), AstNode::Number(2.0)],
        );
        let result = domain.evaluate(&ast, &EvalContext::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_eval_taylor_wrong_arg_count() {
        let domain = SymbolicDomain;
        let ast = AstNode::FunctionCall(
            "taylor".to_string(),
            vec![AstNode::Number(1.0), AstNode::Number(2.0)],
        );
        let result = domain.evaluate(&ast, &EvalContext::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_contains_symbolic_function_unary_op() {
        let ast = AstNode::UnaryOp(
            UnaryOp::Neg,
            Box::new(AstNode::FunctionCall(
                "diff".to_string(),
                vec![
                    AstNode::Variable("x".to_string()),
                    AstNode::Variable("x".to_string()),
                ],
            )),
        );
        assert!(contains_symbolic_function(&ast));
        let ast2 = AstNode::UnaryOp(UnaryOp::Neg, Box::new(AstNode::Number(5.0)));
        assert!(!contains_symbolic_function(&ast2));
    }

    #[test]
    fn test_extract_var_name_error() {
        let ast = AstNode::Number(5.0);
        assert!(extract_var_name(&ast).is_err());
    }

    #[test]
    fn test_extract_number_all_paths() {
        assert_eq!(extract_number(&AstNode::Number(3.14)).unwrap(), 3.14);
        assert_eq!(
            extract_number(&AstNode::BigNumber("42".to_string())).unwrap(),
            42.0
        );
        assert!(extract_number(&AstNode::BigNumber("abc".to_string())).is_err());
        assert_eq!(
            extract_number(&AstNode::UnaryOp(
                UnaryOp::Neg,
                Box::new(AstNode::Number(5.0))
            ))
            .unwrap(),
            -5.0
        );
        assert!(extract_number(&AstNode::Variable("x".to_string())).is_err());
    }

    #[test]
    fn test_eval_symbolic_dispatch() {
        let mut env = HashMap::new();
        env.insert("x".to_string(), 2.0);
        env.insert("y".to_string(), 3.0);

        // Leaf
        assert_eq!(
            math_sym::eval_symbolic(&SymbolicExpr::Const(42.0), &env).unwrap(),
            42.0
        );
        assert_eq!(
            math_sym::eval_symbolic(&SymbolicExpr::Var("x".to_string()), &env).unwrap(),
            2.0
        );
        assert!(math_sym::eval_symbolic(&SymbolicExpr::Var("z".to_string()), &env).is_err());

        // Binary ops
        let x = SymbolicExpr::Var("x".to_string());
        let y = SymbolicExpr::Var("y".to_string());
        assert_eq!(
            math_sym::eval_symbolic(
                &SymbolicExpr::Add(Box::new(x.clone()), Box::new(y.clone())),
                &env
            ).unwrap(),
            5.0
        );
        assert_eq!(
            math_sym::eval_symbolic(
                &SymbolicExpr::Sub(Box::new(x.clone()), Box::new(y.clone())),
                &env
            ).unwrap(),
            -1.0
        );
        assert_eq!(
            math_sym::eval_symbolic(
                &SymbolicExpr::Mul(Box::new(x.clone()), Box::new(y.clone())),
                &env
            ).unwrap(),
            6.0
        );
        assert!(
            (math_sym::eval_symbolic(
                &SymbolicExpr::Div(Box::new(x.clone()), Box::new(y.clone())),
                &env
            ).unwrap() - 2.0 / 3.0).abs() < 1e-10
        );
        assert_eq!(
            math_sym::eval_symbolic(
                &SymbolicExpr::Pow(Box::new(x.clone()), Box::new(y.clone())),
                &env
            ).unwrap(),
            8.0
        );

        // Div by zero
        let zero = SymbolicExpr::Const(0.0);
        let div_pos = math_sym::eval_symbolic(
            &SymbolicExpr::Div(Box::new(SymbolicExpr::Const(5.0)), Box::new(zero.clone())),
            &env,
        );
        assert!(matches!(div_pos, Err(ref e) if e.kind == ErrorKind::DivisionByZero));

        // Unary ops
        assert_eq!(
            math_sym::eval_symbolic(&SymbolicExpr::Neg(Box::new(x.clone())), &env).unwrap(),
            -2.0
        );
        assert!(
            (math_sym::eval_symbolic(&SymbolicExpr::Sin(Box::new(x.clone())), &env).unwrap() - 2.0_f64.sin()).abs() < 1e-10
        );
        assert!(
            (math_sym::eval_symbolic(&SymbolicExpr::Cos(Box::new(x.clone())), &env).unwrap() - 2.0_f64.cos()).abs() < 1e-10
        );
        assert!(
            (math_sym::eval_symbolic(&SymbolicExpr::Tan(Box::new(x.clone())), &env).unwrap() - 2.0_f64.tan()).abs() < 1e-10
        );
        assert!(
            (math_sym::eval_symbolic(&SymbolicExpr::Exp(Box::new(x.clone())), &env).unwrap() - 2.0_f64.exp()).abs() < 1e-10
        );

        // Ln boundaries
        assert!(
            (math_sym::eval_symbolic(&SymbolicExpr::Ln(Box::new(SymbolicExpr::Const(1.0))), &env).unwrap() - 0.0).abs() < 1e-10
        );
        assert!(matches!(
            math_sym::eval_symbolic(&SymbolicExpr::Ln(Box::new(SymbolicExpr::Const(0.0))), &env),
            Err(ref e) if e.kind == ErrorKind::Domain
        ));
        assert!(matches!(
            math_sym::eval_symbolic(&SymbolicExpr::Ln(Box::new(SymbolicExpr::Const(-1.0))), &env),
            Err(ref e) if e.kind == ErrorKind::Domain
        ));

        // Nested
        let add_xy = SymbolicExpr::Add(Box::new(x.clone()), Box::new(y.clone()));
        let mul = SymbolicExpr::Mul(Box::new(add_xy), Box::new(x.clone()));
        assert_eq!(math_sym::eval_symbolic(&mul, &env).unwrap(), 10.0);

        let sin_x = SymbolicExpr::Sin(Box::new(x.clone()));
        let cos_x = SymbolicExpr::Cos(Box::new(x.clone()));
        let sin_sq = SymbolicExpr::Pow(Box::new(sin_x), Box::new(SymbolicExpr::Const(2.0)));
        let cos_sq = SymbolicExpr::Pow(Box::new(cos_x), Box::new(SymbolicExpr::Const(2.0)));
        let pythag = SymbolicExpr::Add(Box::new(sin_sq), Box::new(cos_sq));
        assert!((math_sym::eval_symbolic(&pythag, &env).unwrap() - 1.0).abs() < 1e-10);
    }
}
