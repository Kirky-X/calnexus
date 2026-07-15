// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! Complex 计算域：复数四则运算、模、幅角、共轭、复指数、复对数。
//!
//! 设计依据：
//! - complex-domain spec：10 个 requirements / 24 个 scenarios
//! - design.md D4：基于 `num_complex::Complex64` 实现，priority=30
//!
//! 路由策略：AST 含 `Complex` 节点或 `complex()`/`conj()`/`arg()` 函数调用时路由至本域。
//! `abs()`/`exp()`/`ln()` 仅当参数含 `Complex` 节点时路由至本域。

use crate::core::CalculationDomain;
use crate::core::{AstNode, BinaryOp, CalcError, EvalContext, EvalResult, UnaryOp};
use num_complex::Complex64;

/// Complex 计算域。
///
/// priority=30，高于 Scientific（20）与 Arithmetic（10）。
/// 支持复数四则运算、模、幅角、共轭、复指数、复对数。
pub struct ComplexDomain;

impl CalculationDomain for ComplexDomain {
    fn domain_name(&self) -> &str {
        "complex"
    }

    fn priority(&self) -> u8 {
        30
    }

    fn supports(&self, ast: &AstNode) -> bool {
        contains_complex(ast)
    }

    fn evaluate(&self, ast: &AstNode, ctx: &EvalContext) -> Result<EvalResult, CalcError> {
        // 预绑定 pi/e（若上下文未提供）
        let mut ctx = ctx.clone();
        if ctx.get_var("pi").is_none() {
            ctx = ctx.with_var("pi", std::f64::consts::PI);
        }
        if ctx.get_var("e").is_none() {
            ctx = ctx.with_var("e", std::f64::consts::E);
        }

        let value = self.eval(ast, &ctx)?;
        match value {
            ComplexValue::Scalar(v) => {
                if !v.is_finite() {
                    return Err(CalcError::NaNOrInf);
                }
                Ok(EvalResult::Scalar(v))
            }
            ComplexValue::Complex(c) => {
                if !c.re.is_finite() || !c.im.is_finite() {
                    return Err(CalcError::NaNOrInf);
                }
                Ok(EvalResult::Complex(c.re, c.im))
            }
        }
    }
}

impl ComplexDomain {
    /// 递归求值 AST 节点，返回标量或复数。
    ///
    /// 标量与复数的区分规则（spec Req 9：混合运算）：
    /// - `abs()`/`arg()` 返回标量
    /// - `conj()`/`exp()`/`ln()`/`complex()` 返回复数
    /// - `Complex` 节点返回复数，`Number` 节点返回标量
    /// - 二元运算：两个标量运算结果为标量；任一为复数则结果为复数
    fn eval(&self, ast: &AstNode, ctx: &EvalContext) -> Result<ComplexValue, CalcError> {
        match ast {
            AstNode::Number(n) => Ok(ComplexValue::Scalar(*n)),
            AstNode::Complex(re, im) => Ok(ComplexValue::Complex(Complex64::new(*re, *im))),
            AstNode::Variable(name) => {
                if name == "i" {
                    return Ok(ComplexValue::Complex(Complex64::new(0.0, 1.0)));
                }
                ctx.get_var(name)
                    .map(ComplexValue::Scalar)
                    .ok_or_else(|| CalcError::EvalError(format!("unbound variable: {}", name)))
            }
            AstNode::BinaryOp(op, l, r) => {
                let a = self.eval(l, ctx)?;
                let b = self.eval(r, ctx)?;
                self.eval_binary(*op, a, b)
            }
            AstNode::UnaryOp(op, e) => {
                let v = self.eval(e, ctx)?;
                match op {
                    UnaryOp::Neg => match v {
                        ComplexValue::Scalar(s) => Ok(ComplexValue::Scalar(-s)),
                        ComplexValue::Complex(c) => Ok(ComplexValue::Complex(-c)),
                    },
                    UnaryOp::Abs => Ok(ComplexValue::Scalar(v.to_complex().norm())),
                    UnaryOp::Factorial => Err(CalcError::DomainError(
                        "factorial not supported in complex domain".to_string(),
                    )),
                }
            }
            AstNode::FunctionCall(name, args) => self.eval_function(name, args, ctx),
            AstNode::Matrix(_) | AstNode::List(_) | AstNode::BigNumber(_) => {
                Err(CalcError::DomainError(format!(
                    "complex domain does not support this node type: {:?}",
                    ast
                )))
            }
        }
    }

    /// 求值二元运算。
    ///
    /// 两个标量做标量运算；任一为复数则提升为复数运算。
    fn eval_binary(
        &self,
        op: BinaryOp,
        a: ComplexValue,
        b: ComplexValue,
    ) -> Result<ComplexValue, CalcError> {
        // 两个标量 → 标量运算
        if let (ComplexValue::Scalar(av), ComplexValue::Scalar(bv)) = (&a, &b) {
            let result = match op {
                BinaryOp::Add => av + bv,
                BinaryOp::Sub => av - bv,
                BinaryOp::Mul => av * bv,
                BinaryOp::Div => {
                    if *bv == 0.0 {
                        return Err(CalcError::DivisionByZero);
                    }
                    av / bv
                }
                BinaryOp::Pow => av.powf(*bv),
                BinaryOp::Mod => {
                    return Err(CalcError::DomainError(
                        "mod not supported in complex domain".to_string(),
                    ))
                }
            };
            return Ok(ComplexValue::Scalar(result));
        }

        // 任一为复数 → 复数运算
        let ac = a.to_complex();
        let bc = b.to_complex();
        let result = match op {
            BinaryOp::Add => ac + bc,
            BinaryOp::Sub => ac - bc,
            BinaryOp::Mul => ac * bc,
            BinaryOp::Div => {
                if bc.norm() == 0.0 {
                    return Err(CalcError::DivisionByZero);
                }
                ac / bc
            }
            BinaryOp::Pow => ac.powc(bc),
            BinaryOp::Mod => {
                return Err(CalcError::DomainError(
                    "mod not supported for complex numbers".to_string(),
                ))
            }
        };
        Ok(ComplexValue::Complex(result))
    }

    /// 求值函数调用。
    fn eval_function(
        &self,
        name: &str,
        args: &[AstNode],
        ctx: &EvalContext,
    ) -> Result<ComplexValue, CalcError> {
        match name {
            "complex" => {
                if args.len() != 2 {
                    return Err(CalcError::DomainError(format!(
                        "complex() requires exactly 2 arguments, got {}",
                        args.len()
                    )));
                }
                let re = self.eval(&args[0], ctx)?.to_complex().re;
                let im = self.eval(&args[1], ctx)?.to_complex().re;
                Ok(ComplexValue::Complex(Complex64::new(re, im)))
            }
            "conj" => {
                let c = self.expect_one_arg(name, args, ctx)?;
                Ok(ComplexValue::Complex(c.conj()))
            }
            "arg" => {
                let c = self.expect_one_arg(name, args, ctx)?;
                Ok(ComplexValue::Scalar(c.arg()))
            }
            "abs" => {
                let c = self.expect_one_arg(name, args, ctx)?;
                Ok(ComplexValue::Scalar(c.norm()))
            }
            "exp" => {
                let c = self.expect_one_arg(name, args, ctx)?;
                Ok(ComplexValue::Complex(c.exp()))
            }
            "ln" => {
                let c = self.expect_one_arg(name, args, ctx)?;
                Ok(ComplexValue::Complex(c.ln()))
            }
            _ => Err(CalcError::DomainError(format!(
                "unsupported function in complex domain: {}",
                name
            ))),
        }
    }

    /// 辅助：要求单参数函数并返回复数值。
    fn expect_one_arg(
        &self,
        name: &str,
        args: &[AstNode],
        ctx: &EvalContext,
    ) -> Result<Complex64, CalcError> {
        if args.len() != 1 {
            return Err(CalcError::DomainError(format!(
                "{}() requires exactly 1 argument, got {}",
                name,
                args.len()
            )));
        }
        Ok(self.eval(&args[0], ctx)?.to_complex())
    }
}

/// 内部求值结果：标量或复数。
///
/// 用于区分 `abs()`/`arg()` 返回标量与其他运算返回复数（spec Req 9）。
enum ComplexValue {
    Scalar(f64),
    Complex(Complex64),
}

impl ComplexValue {
    /// 提升为复数。
    fn to_complex(&self) -> Complex64 {
        match self {
            ComplexValue::Scalar(v) => Complex64::new(*v, 0.0),
            ComplexValue::Complex(c) => *c,
        }
    }
}

/// 递归检查 AST 是否应路由至 ComplexDomain。
///
/// 路由条件（spec Req 8）：
/// - 含 `Complex` 节点
/// - 含 `complex()`/`conj()`/`arg()` 函数调用（复数专用函数）
/// - `abs()`/`exp()`/`ln()` 的参数含 `Complex` 节点（共享函数，仅复数参数时路由）
fn contains_complex(ast: &AstNode) -> bool {
    match ast {
        AstNode::Complex(_, _) => true,
        AstNode::FunctionCall(name, args) => {
            // 复数专用函数：总是路由至 ComplexDomain
            if matches!(name.as_str(), "complex" | "conj" | "arg") {
                return true;
            }
            // 共享函数（abs/exp/ln）：仅当参数含 Complex 节点时路由
            if matches!(name.as_str(), "abs" | "exp" | "ln") {
                return args.iter().any(contains_complex);
            }
            // 其他函数：检查参数
            args.iter().any(contains_complex)
        }
        AstNode::BinaryOp(_, l, r) => contains_complex(l) || contains_complex(r),
        AstNode::UnaryOp(_, e) => contains_complex(e),
        AstNode::Matrix(rows) => rows.iter().flatten().any(contains_complex),
        AstNode::List(elements) => elements.iter().any(contains_complex),
        AstNode::Number(_) | AstNode::Variable(_) | AstNode::BigNumber(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::parse;

    /// 测试浮点近似相等（默认容差 1e-10）。
    fn assert_approx(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 1e-10,
            "expected {} but got {}",
            expected,
            actual
        );
    }

    /// 测试复数近似相等。
    fn assert_complex_approx(actual: &EvalResult, expected_re: f64, expected_im: f64) {
        match actual {
            EvalResult::Complex(re, im) => {
                assert_approx(*re, expected_re);
                assert_approx(*im, expected_im);
            }
            other => panic!("expected Complex result, got {:?}", other),
        }
    }

    /// 测试标量结果。
    fn assert_scalar(actual: &EvalResult, expected: f64) {
        match actual {
            EvalResult::Scalar(v) => assert_approx(*v, expected),
            other => panic!("expected Scalar({}), got {:?}", expected, other),
        }
    }

    /// 创建预绑定 pi/e 的上下文。
    fn default_ctx() -> EvalContext {
        EvalContext::new()
            .with_var("pi", std::f64::consts::PI)
            .with_var("e", std::f64::consts::E)
    }

    // ===== Requirement 1: 复数字面量解析 =====

    #[test]
    fn test_complex_literal_standard() {
        // 3+4i → Complex(3, 4)（Req 1 Scen 1）
        let ast = parse("3+4i").unwrap();
        assert_eq!(ast, AstNode::Complex(3.0, 4.0));
    }

    #[test]
    fn test_complex_literal_pure_imaginary() {
        // 2i → Complex(0, 2)（Req 1 Scen 2）
        let ast = parse("2i").unwrap();
        assert_eq!(ast, AstNode::Complex(0.0, 2.0));
    }

    #[test]
    fn test_real_number_not_route_to_complex() {
        // 5 → Number(5)，不路由到 ComplexDomain（Req 1 Scen 3）
        let ast = parse("5").unwrap();
        assert_eq!(ast, AstNode::Number(5.0));
        let domain = ComplexDomain;
        assert!(!domain.supports(&ast));
    }

    // ===== Requirement 2: 复数四则运算 =====

    #[test]
    fn test_complex_addition() {
        // (1+2i) + (3+4i) → 4+6i（Req 2 Scen 1）
        let ast = parse("(1+2i) + (3+4i)").unwrap();
        let domain = ComplexDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_complex_approx(&result, 4.0, 6.0);
    }

    #[test]
    fn test_complex_subtraction() {
        // (1+2i) - (3+4i) → -2-2i（Req 2 Scen 2）
        let ast = parse("(1+2i) - (3+4i)").unwrap();
        let domain = ComplexDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_complex_approx(&result, -2.0, -2.0);
    }

    #[test]
    fn test_complex_multiplication() {
        // (1+2i) * (3+4i) → -5+10i（Req 2 Scen 3）
        let ast = parse("(1+2i) * (3+4i)").unwrap();
        let domain = ComplexDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_complex_approx(&result, -5.0, 10.0);
    }

    #[test]
    fn test_complex_division() {
        // (1+2i) / (3+4i) → 0.44+0.08i（Req 2 Scen 4）
        let ast = parse("(1+2i) / (3+4i)").unwrap();
        let domain = ComplexDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_complex_approx(&result, 0.44, 0.08);
    }

    // ===== Requirement 3: 复数模 =====

    #[test]
    fn test_complex_abs_standard() {
        // abs(3+4i) → 5.0（Req 3 Scen 1）
        let ast = parse("abs(3+4i)").unwrap();
        let domain = ComplexDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_scalar(&result, 5.0);
    }

    #[test]
    fn test_complex_abs_pure_imaginary() {
        // abs(0+3i) → 3.0（Req 3 Scen 2）
        let ast = parse("abs(0+3i)").unwrap();
        let domain = ComplexDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_scalar(&result, 3.0);
    }

    // ===== Requirement 4: 复数幅角 =====

    #[test]
    fn test_complex_arg_first_quadrant() {
        // arg(1+1i) → pi/4（Req 4 Scen 1）
        let ast = parse("arg(1+1i)").unwrap();
        let domain = ComplexDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_scalar(&result, std::f64::consts::FRAC_PI_4);
    }

    #[test]
    fn test_complex_arg_positive_real() {
        // arg(2+0i) → 0.0（Req 4 Scen 2）
        let ast = parse("arg(2+0i)").unwrap();
        let domain = ComplexDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_scalar(&result, 0.0);
    }

    // ===== Requirement 5: 复数共轭 =====

    #[test]
    fn test_complex_conjugate_standard() {
        // conj(3+4i) → 3-4i（Req 5 Scen 1）
        let ast = parse("conj(3+4i)").unwrap();
        let domain = ComplexDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_complex_approx(&result, 3.0, -4.0);
    }

    #[test]
    fn test_complex_conjugate_pure_real() {
        // conj(5+0i) → 5+0i（Req 5 Scen 2）
        let ast = parse("conj(5+0i)").unwrap();
        let domain = ComplexDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_complex_approx(&result, 5.0, 0.0);
    }

    // ===== Requirement 6: 复指数 =====

    #[test]
    fn test_complex_exp_euler_identity() {
        // exp(complex(0,1)*pi) → -1+0i（Req 6 Scen 1，欧拉公式恒等式）
        // 等价于数学上的 exp(i*pi) = -1
        let ast = parse("exp(complex(0,1)*pi)").unwrap();
        let domain = ComplexDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_complex_approx(&result, -1.0, 0.0);
    }

    #[test]
    fn test_complex_exp_general() {
        // exp(1+1i) → 约 2.718+2.718i（Req 6 Scen 2）
        let ast = parse("exp(1+1i)").unwrap();
        let domain = ComplexDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        let expected_re = std::f64::consts::E * 1.0_f64.cos();
        let expected_im = std::f64::consts::E * 1.0_f64.sin();
        assert_complex_approx(&result, expected_re, expected_im);
    }

    // ===== Requirement 7: 复对数 =====

    #[test]
    fn test_complex_ln_one_plus_i() {
        // ln(1+1i) → 约 0.347+0.785i（Req 7 Scen 1）
        let ast = parse("ln(1+1i)").unwrap();
        let domain = ComplexDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        let c = Complex64::new(1.0, 1.0);
        let expected = c.ln();
        assert_complex_approx(&result, expected.re, expected.im);
    }

    #[test]
    fn test_complex_ln_positive_real() {
        // ln(2+0i) → 约 0.693+0i（Req 7 Scen 2）
        let ast = parse("ln(2+0i)").unwrap();
        let domain = ComplexDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_complex_approx(&result, std::f64::consts::LN_2, 0.0);
    }

    // ===== Requirement 8: 域路由 =====

    #[test]
    fn test_route_complex_node() {
        // 3+4i + 1 → 含 Complex 节点，路由到 ComplexDomain（Req 8 Scen 1）
        let ast = parse("3+4i + 1").unwrap();
        let domain = ComplexDomain;
        assert!(domain.supports(&ast));
    }

    #[test]
    fn test_route_complex_function() {
        // conj(2+3i) → 含 conj() 函数，路由到 ComplexDomain（Req 8 Scen 2）
        let ast = parse("conj(2+3i)").unwrap();
        let domain = ComplexDomain;
        assert!(domain.supports(&ast));
    }

    // ===== Requirement 9: 混合运算 =====

    #[test]
    fn test_mixed_abs_complex_plus_real() {
        // abs(3+4i) + 2 → 标量 7.0（Req 9 Scen 1）
        let ast = parse("abs(3+4i) + 2").unwrap();
        let domain = ComplexDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_scalar(&result, 7.0);
    }

    #[test]
    fn test_mixed_complex_plus_real() {
        // (1+2i) + 3 → 4+2i（Req 9 Scen 2）
        let ast = parse("(1+2i) + 3").unwrap();
        let domain = ComplexDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_complex_approx(&result, 4.0, 2.0);
    }

    // ===== Requirement 10: 错误处理 =====

    #[test]
    fn test_unsupported_matrix_node() {
        // 矩阵节点 → DomainError（Req 10 Scen 1）
        let ast = AstNode::Matrix(vec![vec![AstNode::Number(1.0)]]);
        let domain = ComplexDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        let e = result.unwrap_err();
        assert!(
            matches!(e, CalcError::DomainError(_)),
            "expected DomainError, got {:?}",
            e
        );
    }

    #[test]
    fn test_invalid_complex_literal_parse_error() {
        // 3+*4i → 语法错误（Req 10 Scen 2，非法复数字面量）
        let result = parse("3+*4i");
        let e = result.unwrap_err();
        assert!(
            matches!(e, CalcError::ParseError(_)),
            "expected ParseError, got {:?}",
            e
        );
    }

    // ===== 额外覆盖：域优先级与名称 =====

    #[test]
    fn test_complex_domain_priority() {
        let domain = ComplexDomain;
        assert_eq!(domain.priority(), 30);
        assert_eq!(domain.domain_name(), "complex");
    }

    #[test]
    fn test_complex_priority_higher_than_scientific() {
        let complex = ComplexDomain;
        let scientific = crate::ScientificDomain;
        assert!(complex.priority() > scientific.priority());
    }

    // ===== 额外覆盖：除零与不支持的操作 =====

    #[test]
    fn test_complex_division_by_zero() {
        // (1+2i) / 0 → DivisionByZero
        let ast = parse("(1+2i) / 0").unwrap();
        let domain = ComplexDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(CalcError::DivisionByZero)));
    }

    #[test]
    fn test_complex_unsupported_function() {
        // unknown_func(1+2i) → DomainError
        let ast = parse("unknown_func(1+2i)").unwrap();
        let domain = ComplexDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_complex_factorial_unsupported() {
        // (1+2i)! → DomainError（阶乘不支持复数）
        let ast = AstNode::UnaryOp(UnaryOp::Factorial, Box::new(AstNode::Complex(1.0, 2.0)));
        let domain = ComplexDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_complex_list_unsupported() {
        // List 节点 → DomainError
        let ast = AstNode::List(vec![AstNode::Number(1.0)]);
        let domain = ComplexDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_complex_wrong_arg_count() {
        // conj() 无参数 → DomainError
        let ast = AstNode::FunctionCall("conj".to_string(), vec![]);
        let domain = ComplexDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_complex_pow() {
        // (1+1i)^2 → 0+2i
        let ast = parse("(1+1i)^2").unwrap();
        let domain = ComplexDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_complex_approx(&result, 0.0, 2.0);
    }

    #[test]
    fn test_complex_neg() {
        // -(1+2i) → -1-2i
        let ast = parse("-(1+2i)").unwrap();
        let domain = ComplexDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_complex_approx(&result, -1.0, -2.0);
    }

    #[test]
    fn test_complex_abs_unary_op() {
        // abs 作为 UnaryOp.Abs
        let ast = AstNode::UnaryOp(UnaryOp::Abs, Box::new(AstNode::Complex(3.0, 4.0)));
        let domain = ComplexDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_scalar(&result, 5.0);
    }

    #[test]
    fn test_complex_function_call_abs_with_real_arg() {
        // abs(5) → 不路由到 complex（参数无 Complex 节点）
        let ast = parse("abs(5)").unwrap();
        let domain = ComplexDomain;
        assert!(!domain.supports(&ast));
    }

    #[test]
    fn test_complex_i_variable() {
        // i 变量 → Complex(0, 1)
        let ast = AstNode::Variable("i".to_string());
        let domain = ComplexDomain;
        // Variable("i") alone doesn't route to complex, but if evaluated:
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_complex_approx(&result, 0.0, 1.0);
    }

    #[test]
    fn test_complex_nan_detection() {
        // 0+0i / 0 → DivisionByZero (not NaN)
        let ast = parse("(0+0i) / 0").unwrap();
        let domain = ComplexDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(CalcError::DivisionByZero)));
    }

    // ===== 额外覆盖：pi/e 自动绑定（ctx 无 pi/e 时触发）=====

    #[test]
    fn test_pi_e_auto_binding() {
        // 使用无 pi/e 的上下文，触发 evaluate 中的自动绑定（lines 37, 40）
        let ast = parse("conj(3+4i)").unwrap();
        let domain = ComplexDomain;
        let ctx = EvalContext::new();
        let result = domain.evaluate(&ast, &ctx).unwrap();
        assert_complex_approx(&result, 3.0, -4.0);
    }

    // ===== 额外覆盖：标量结果 NaNOrInf =====

    #[test]
    fn test_scalar_nan_or_inf() {
        // abs(3+4i)^1000 → 标量结果为 infinity → NaNOrInf（line 47）
        let ast = AstNode::BinaryOp(
            BinaryOp::Pow,
            Box::new(AstNode::FunctionCall(
                "abs".to_string(),
                vec![AstNode::Complex(3.0, 4.0)],
            )),
            Box::new(AstNode::Number(1000.0)),
        );
        let domain = ComplexDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(CalcError::NaNOrInf)));
    }

    // ===== 额外覆盖：复数结果 NaNOrInf =====

    #[test]
    fn test_complex_nan_or_inf() {
        // exp(1000+0i) → 复数结果 re 为 infinity → NaNOrInf（line 53）
        let ast = AstNode::FunctionCall("exp".to_string(), vec![AstNode::Complex(1000.0, 0.0)]);
        let domain = ComplexDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(CalcError::NaNOrInf)));
    }

    // ===== 额外覆盖：标量取负（Neg on Scalar）=====

    #[test]
    fn test_neg_on_scalar_in_complex() {
        // (1+2i) + (-3) → Neg 作用于标量 -3（line 90）
        let ast = AstNode::BinaryOp(
            BinaryOp::Add,
            Box::new(AstNode::Complex(1.0, 2.0)),
            Box::new(AstNode::UnaryOp(
                UnaryOp::Neg,
                Box::new(AstNode::Number(3.0)),
            )),
        );
        let domain = ComplexDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_complex_approx(&result, -2.0, 2.0);
    }

    // ===== 额外覆盖：标量二元运算（Sub, Mul, Div, Pow, Mod）=====

    #[test]
    fn test_scalar_sub_in_complex() {
        // abs(3+4i) - 1 → 5 - 1 = 4（line 119/120 Sub）
        let ast = AstNode::BinaryOp(
            BinaryOp::Sub,
            Box::new(AstNode::FunctionCall(
                "abs".to_string(),
                vec![AstNode::Complex(3.0, 4.0)],
            )),
            Box::new(AstNode::Number(1.0)),
        );
        let domain = ComplexDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_scalar(&result, 4.0);
    }

    #[test]
    fn test_scalar_mul_in_complex() {
        // abs(3+4i) * 2 → 5 * 2 = 10（line 120/121 Mul）
        let ast = AstNode::BinaryOp(
            BinaryOp::Mul,
            Box::new(AstNode::FunctionCall(
                "abs".to_string(),
                vec![AstNode::Complex(3.0, 4.0)],
            )),
            Box::new(AstNode::Number(2.0)),
        );
        let domain = ComplexDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_scalar(&result, 10.0);
    }

    #[test]
    fn test_scalar_div_in_complex() {
        // abs(3+4i) / 2 → 5 / 2 = 2.5（line 125/126 Div normal）
        let ast = AstNode::BinaryOp(
            BinaryOp::Div,
            Box::new(AstNode::FunctionCall(
                "abs".to_string(),
                vec![AstNode::Complex(3.0, 4.0)],
            )),
            Box::new(AstNode::Number(2.0)),
        );
        let domain = ComplexDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_scalar(&result, 2.5);
    }

    #[test]
    fn test_scalar_div_by_zero_in_complex() {
        // abs(3+4i) / 0 → DivisionByZero（line 123/124 Div by zero）
        let ast = AstNode::BinaryOp(
            BinaryOp::Div,
            Box::new(AstNode::FunctionCall(
                "abs".to_string(),
                vec![AstNode::Complex(3.0, 4.0)],
            )),
            Box::new(AstNode::Number(0.0)),
        );
        let domain = ComplexDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(CalcError::DivisionByZero)));
    }

    #[test]
    fn test_scalar_pow_in_complex() {
        // abs(3+4i) ^ 2 → 5 ^ 2 = 25（line 127/128 Pow）
        let ast = AstNode::BinaryOp(
            BinaryOp::Pow,
            Box::new(AstNode::FunctionCall(
                "abs".to_string(),
                vec![AstNode::Complex(3.0, 4.0)],
            )),
            Box::new(AstNode::Number(2.0)),
        );
        let domain = ComplexDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_scalar(&result, 25.0);
    }

    #[test]
    fn test_scalar_mod_in_complex() {
        // 两个标量的 Mod → DomainError（lines 129-132 Mod not supported）
        let ast = AstNode::BinaryOp(
            BinaryOp::Mod,
            Box::new(AstNode::FunctionCall(
                "abs".to_string(),
                vec![AstNode::Complex(3.0, 4.0)],
            )),
            Box::new(AstNode::Number(2.0)),
        );
        let domain = ComplexDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    // ===== 额外覆盖：复数 Mod 不支持 =====

    #[test]
    fn test_complex_mod_unsupported() {
        // Complex % Complex → DomainError（lines 152-155）
        let ast = AstNode::BinaryOp(
            BinaryOp::Mod,
            Box::new(AstNode::Complex(1.0, 2.0)),
            Box::new(AstNode::Complex(3.0, 4.0)),
        );
        let domain = ComplexDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    // ===== 额外覆盖：complex() 函数调用 =====

    #[test]
    fn test_complex_function_normal() {
        // complex(1, 2) 作为函数调用 → Complex(1, 2)（lines 176-178）
        let ast = AstNode::FunctionCall(
            "complex".to_string(),
            vec![AstNode::Number(1.0), AstNode::Number(2.0)],
        );
        let domain = ComplexDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_complex_approx(&result, 1.0, 2.0);
    }

    #[test]
    fn test_complex_function_wrong_arg_count() {
        // complex(1) → DomainError（lines 171-175 参数数量错误）
        let ast = AstNode::FunctionCall("complex".to_string(), vec![AstNode::Number(1.0)]);
        let domain = ComplexDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    // ===== 额外覆盖：contains_complex 对 UnaryOp/Matrix/List 的路由 =====

    #[test]
    fn test_supports_unary_op_with_complex() {
        // -(3+4i) → UnaryOp 包含 Complex → supports 返回 true（line 265）
        let ast = AstNode::UnaryOp(UnaryOp::Neg, Box::new(AstNode::Complex(3.0, 4.0)));
        let domain = ComplexDomain;
        assert!(domain.supports(&ast));
    }

    #[test]
    fn test_supports_matrix_with_complex() {
        // Matrix 包含 Complex → supports 返回 true（line 266）
        let ast = AstNode::Matrix(vec![vec![AstNode::Complex(1.0, 2.0)]]);
        let domain = ComplexDomain;
        assert!(domain.supports(&ast));
    }

    #[test]
    fn test_supports_list_with_complex() {
        // List 包含 Complex → supports 返回 true（line 267）
        let ast = AstNode::List(vec![AstNode::Complex(1.0, 2.0)]);
        let domain = ComplexDomain;
        assert!(domain.supports(&ast));
    }

    // ===== 覆盖测试辅助函数的 panic 分支（lines 294, 302）=====

    #[test]
    #[should_panic(expected = "expected Complex result")]
    fn test_assert_complex_approx_panics_on_non_complex() {
        // 传入 Scalar 而非 Complex → panic（line 294）
        assert_complex_approx(&EvalResult::Scalar(1.0), 1.0, 0.0);
    }

    #[test]
    #[should_panic(expected = "expected Scalar")]
    fn test_assert_scalar_panics_on_non_scalar() {
        // 传入 Complex 而非 Scalar → panic（line 302）
        assert_scalar(&EvalResult::Complex(1.0, 2.0), 1.0);
    }

    // ===== proptest 属性测试（task 12.7）=====

    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

        /// 属性：共轭的共轭 = 原复数（spec Req 5 数学性质）
        #[test]
        fn prop_double_conjugate_identity(re in -1e6f64..1e6, im in -1e6f64..1e6) {
            let ast = AstNode::FunctionCall(
                "conj".to_string(),
                vec![AstNode::FunctionCall(
                    "conj".to_string(),
                    vec![AstNode::Complex(re, im)],
                )],
            );
            let domain = ComplexDomain;
            let result = domain.evaluate(&ast, &default_ctx()).unwrap();
            match result {
                EvalResult::Complex(r, i) => {
                    prop_assert!((r - re).abs() < 1e-6);
                    prop_assert!((i - im).abs() < 1e-6);
                }
                _ => panic!("expected Complex result"),
            }
        }

        /// 属性：模非负（spec Req 3 数学性质）
        #[test]
        fn prop_abs_non_negative(re in -1e6f64..1e6, im in -1e6f64..1e6) {
            let ast = AstNode::FunctionCall(
                "abs".to_string(),
                vec![AstNode::Complex(re, im)],
            );
            let domain = ComplexDomain;
            let result = domain.evaluate(&ast, &default_ctx()).unwrap();
            match result {
                EvalResult::Scalar(v) => prop_assert!(v >= 0.0),
                _ => panic!("expected Scalar result"),
            }
        }

        /// 属性：欧拉公式 exp(i*theta) 模为 1
        /// exp(complex(0, theta)) 的模 = 1（对任意实数 theta）
        #[test]
        fn prop_euler_formula_unit_modulus(theta in -10.0f64..10.0) {
            let ast = AstNode::FunctionCall(
                "exp".to_string(),
                vec![AstNode::Complex(0.0, theta)],
            );
            let domain = ComplexDomain;
            let result = domain.evaluate(&ast, &default_ctx()).unwrap();
            match result {
                EvalResult::Complex(re, im) => {
                    let modulus = (re * re + im * im).sqrt();
                    prop_assert!((modulus - 1.0).abs() < 1e-9);
                }
                _ => panic!("expected Complex result"),
            }
        }

        /// 属性：复数加法交换律 a+b = b+a
        #[test]
        fn prop_addition_commutative(
            ar in -1e3f64..1e3, ai in -1e3f64..1e3,
            br in -1e3f64..1e3, bi in -1e3f64..1e3
        ) {
            let domain = ComplexDomain;
            let ctx = default_ctx();
            let ast_ab = AstNode::BinaryOp(
                BinaryOp::Add,
                Box::new(AstNode::Complex(ar, ai)),
                Box::new(AstNode::Complex(br, bi)),
            );
            let ast_ba = AstNode::BinaryOp(
                BinaryOp::Add,
                Box::new(AstNode::Complex(br, bi)),
                Box::new(AstNode::Complex(ar, ai)),
            );
            let r_ab = domain.evaluate(&ast_ab, &ctx).unwrap();
            let r_ba = domain.evaluate(&ast_ba, &ctx).unwrap();
            match (&r_ab, &r_ba) {
                (EvalResult::Complex(r1, i1), EvalResult::Complex(r2, i2)) => {
                    prop_assert!((r1 - r2).abs() < 1e-9);
                    prop_assert!((i1 - i2).abs() < 1e-9);
                }
                _ => panic!("expected Complex results"),
            }
        }

        /// 属性：复数与其共轭之和为实数（2*re）
        #[test]
        fn prop_conjugate_sum_is_real(re in -1e6f64..1e6, im in -1e6f64..1e6) {
            let ast = AstNode::BinaryOp(
                BinaryOp::Add,
                Box::new(AstNode::Complex(re, im)),
                Box::new(AstNode::FunctionCall(
                    "conj".to_string(),
                    vec![AstNode::Complex(re, im)],
                )),
            );
            let domain = ComplexDomain;
            let result = domain.evaluate(&ast, &default_ctx()).unwrap();
            match result {
                EvalResult::Complex(r, i) => {
                    prop_assert!((r - 2.0 * re).abs() < 1e-6);
                    prop_assert!(i.abs() < 1e-6);
                }
                _ => panic!("expected Complex result"),
            }
        }
    }
}
