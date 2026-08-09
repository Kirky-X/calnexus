// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! Scientific 计算域：三角函数、反三角函数、对数、指数、双曲函数、特殊函数。
//!
//! 设计依据：
//! - scientific-domain spec：10 个 requirements / 31 个 scenarios
//! - design.md D4：实现 `CalculationDomain` trait
//!
//! priority = 20（高于 Arithmetic 的 10），含科学函数或 pi/e 常量的表达式
//! 路由至本域。本域内含完整算术求值能力，以处理混合表达式如 `sin(x)+2*3`。

use crate::core::CalculationDomain;
use crate::core::{AstNode, BinaryOp, CalcError, EvalContext, EvalResult, UnaryOp};
use super::common::{ensure_math_constants, resolve_variable, unsupported_node_error};

/// 科学函数白名单。
const SCIENTIFIC_FUNCTIONS: &[&str] = &[
    "sin", "cos", "tan", "asin", "acos", "atan", "ln", "log10", "log2", "log", "exp", "sinh",
    "cosh", "tanh", "gamma", "erf",
];

// Lanczos 系数和 erf 逼近系数已迁移到 `math/scientific.rs`。

/// Scientific 计算域。
///
/// 支持三角/反三角/对数/指数/双曲/特殊函数，内置 pi/e 常量。
/// priority=20，高于 ArithmeticDomain，含科学函数的表达式路由至本域。
pub struct ScientificDomain;

impl CalculationDomain for ScientificDomain {
    fn domain_name(&self) -> &str {
        "scientific"
    }

    fn priority(&self) -> u8 {
        20
    }

    fn supports(&self, ast: &AstNode) -> bool {
        contains_scientific(ast)
    }

    fn evaluate(&self, ast: &AstNode, ctx: &EvalContext) -> Result<EvalResult, CalcError> {
        let ctx = ensure_math_constants(ctx);
        let value = self.eval_node(ast, &ctx)?;
        Ok(EvalResult::Scalar(value))
    }
}

impl ScientificDomain {
    /// 递归求值 AST 节点。
    fn eval_node(&self, ast: &AstNode, ctx: &EvalContext) -> Result<f64, CalcError> {
        match ast {
            AstNode::Number(n) => Ok(*n),
            AstNode::Variable(name) => resolve_variable(ctx, name),
            AstNode::BinaryOp(op, l, r) => {
                let a = self.eval_node(l, ctx)?;
                let b = self.eval_node(r, ctx)?;
                self.eval_binary(*op, a, b)
            }
            AstNode::UnaryOp(op, e) => {
                let v = self.eval_node(e, ctx)?;
                match op {
                    UnaryOp::Neg => Ok(-v),
                    UnaryOp::Factorial => self.eval_factorial(v),
                    UnaryOp::Abs => Ok(crate::math::arithmetic::abs(v)),
                }
            }
            AstNode::FunctionCall(name, args) => self.eval_function(name, args, ctx),
            AstNode::Complex(_, _)
            | AstNode::Matrix(_)
            | AstNode::List(_)
            | AstNode::BigNumber(_)
            | AstNode::Str(_) => Err(unsupported_node_error("scientific", ast)),
        }
    }

    /// 求值二元运算（委托给 `math::arithmetic::*`）。
    fn eval_binary(&self, op: BinaryOp, a: f64, b: f64) -> Result<f64, CalcError> {
        match op {
            BinaryOp::Add => crate::math::arithmetic::add(a, b),
            BinaryOp::Sub => crate::math::arithmetic::sub(a, b),
            BinaryOp::Mul => crate::math::arithmetic::mul(a, b),
            BinaryOp::Div => crate::math::arithmetic::div(a, b),
            BinaryOp::Pow => crate::math::arithmetic::pow(a, b),
            BinaryOp::Mod => crate::math::arithmetic::rem(a, b),
        }
    }

    /// 求值阶乘（委托给 `math::arithmetic::factorial`）。
    fn eval_factorial(&self, n: f64) -> Result<f64, CalcError> {
        crate::math::arithmetic::factorial(n)
    }

    /// 求值科学函数调用：按函数名分发到对应的处理方法。
    fn eval_function(
        &self,
        name: &str,
        args: &[AstNode],
        ctx: &EvalContext,
    ) -> Result<f64, CalcError> {
        if !SCIENTIFIC_FUNCTIONS.contains(&name) {
            return Err(
                CalcError::eval(format!("unknown function: {}", name)).with_i18n(
                    "msg.unknown_function",
                    vec![("name".to_string(), name.to_string())],
                ),
            );
        }
        match name {
            // ===== 三角函数 =====
            "sin" => self.eval_unary_math(name, args, ctx, crate::math::scientific::sin),
            "cos" => self.eval_unary_math(name, args, ctx, crate::math::scientific::cos),
            "tan" => self.eval_unary_math(name, args, ctx, crate::math::scientific::tan),
            // ===== 反三角函数 =====
            "asin" => self.eval_unary_math(name, args, ctx, crate::math::scientific::asin),
            "acos" => self.eval_unary_math(name, args, ctx, crate::math::scientific::acos),
            "atan" => self.eval_unary_math(name, args, ctx, crate::math::scientific::atan),
            // ===== 对数函数 =====
            "ln" => self.eval_unary_math(name, args, ctx, crate::math::scientific::ln),
            "log10" => self.eval_unary_math(name, args, ctx, crate::math::scientific::log10),
            "log2" => self.eval_unary_math(name, args, ctx, crate::math::scientific::log2),
            "log" => self.eval_log(name, args, ctx),
            // ===== 指数函数 =====
            "exp" => self.eval_unary_math(name, args, ctx, crate::math::scientific::exp),
            // ===== 双曲函数 =====
            "sinh" => self.eval_unary_math(name, args, ctx, crate::math::scientific::sinh),
            "cosh" => self.eval_unary_math(name, args, ctx, crate::math::scientific::cosh),
            "tanh" => self.eval_unary_math(name, args, ctx, crate::math::scientific::tanh),
            // ===== 特殊函数 =====
            "gamma" => self.eval_unary_math(name, args, ctx, crate::math::scientific::gamma),
            "erf" => self.eval_unary_math(name, args, ctx, crate::math::scientific::erf),
            _ => Err(
                CalcError::eval(format!("unknown function: {}", name)).with_i18n(
                    "msg.unknown_function",
                    vec![("name".to_string(), name.to_string())],
                ),
            ),
        }
    }

    /// 单参数 math 函数通用模板：求值参数 → 调用 math 函数。
    fn eval_unary_math(
        &self,
        name: &str,
        args: &[AstNode],
        ctx: &EvalContext,
        f: impl Fn(f64) -> Result<f64, CalcError>,
    ) -> Result<f64, CalcError> {
        let x = self.eval_one_arg(name, args, ctx)?;
        f(x)
    }

    /// log(value, base)：委托给 `math::scientific::log`。
    fn eval_log(&self, _name: &str, args: &[AstNode], ctx: &EvalContext) -> Result<f64, CalcError> {
        if args.len() != 2 {
            return Err(CalcError::eval(format!(
                "log expects 2 arguments (value, base), got {}",
                args.len()
            ))
            .with_i18n(
                "msg.scientific.log_arg_count",
                vec![("actual".to_string(), args.len().to_string())],
            ));
        }
        let value = self.eval_node(&args[0], ctx)?;
        let base = self.eval_node(&args[1], ctx)?;
        crate::math::scientific::log(value, base)
    }

    /// 求值单参数函数的参数。
    fn eval_one_arg(
        &self,
        name: &str,
        args: &[AstNode],
        ctx: &EvalContext,
    ) -> Result<f64, CalcError> {
        if args.len() != 1 {
            return Err(CalcError::eval(format!(
                "{} expects 1 argument, got {}",
                name,
                args.len()
            ))
            .with_i18n(
                "msg.scientific.arg_count",
                vec![
                    ("name".to_string(), name.to_string()),
                    ("actual".to_string(), args.len().to_string()),
                ],
            ));
        }
        self.eval_node(&args[0], ctx)
    }

    // check_finite 已迁移到 math/scientific.rs 内部使用。
}

impl Default for ScientificDomain {
    fn default() -> Self {
        Self
    }
}

/// 检查 AST 是否包含科学函数或 pi/e 常量。
fn contains_scientific(ast: &AstNode) -> bool {
    match ast {
        AstNode::Number(_) | AstNode::Complex(_, _) | AstNode::BigNumber(_) | AstNode::Str(_) => {
            false
        }
        AstNode::Variable(name) => name == "pi" || name == "e",
        AstNode::BinaryOp(_, l, r) => contains_scientific(l) || contains_scientific(r),
        AstNode::UnaryOp(_, e) => contains_scientific(e),
        AstNode::FunctionCall(name, args) => {
            SCIENTIFIC_FUNCTIONS.contains(&name.as_str()) || args.iter().any(contains_scientific)
        }
        AstNode::Matrix(rows) => rows
            .iter()
            .flat_map(|row| row.iter())
            .any(contains_scientific),
        AstNode::List(elements) => elements.iter().any(contains_scientific),
    }
}

// lanczos_gamma 和 erf 已迁移到 `math/scientific.rs`。

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::parse;
    use crate::core::ErrorKind;

    /// 辅助函数：解析 + 求值，返回 f64
    fn eval(input: &str) -> Result<f64, CalcError> {
        let ast = parse(input).unwrap();
        let domain = ScientificDomain;
        let ctx = EvalContext::new();
        domain
            .evaluate(&ast, &ctx)
            .map(|r| r.as_scalar().expect("expected scalar result"))
    }

    /// 辅助函数：近似比较
    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-10
    }

    // ===== Requirement 1: Trigonometric Functions in Radians =====

    #[test]
    fn test_sin_zero() {
        // sin(0) → 0.0 (Req 1 Scen 1)
        assert!(approx(eval("sin(0)").unwrap(), 0.0));
    }

    #[test]
    fn test_cos_zero() {
        // cos(0) → 1.0 (Req 1 Scen 2)
        assert!(approx(eval("cos(0)").unwrap(), 1.0));
    }

    #[test]
    fn test_sin_pi_over_two() {
        // sin(pi/2) → 1.0 (Req 1 Scen 3)
        assert!(approx(eval("sin(pi/2)").unwrap(), 1.0));
    }

    #[test]
    fn test_tan_pi_over_four() {
        // tan(pi/4) → ≈1.0 (Req 1 Scen 4)
        assert!(approx(eval("tan(pi/4)").unwrap(), 1.0));
    }

    // ===== Requirement 2: Inverse Trigonometric Functions =====

    #[test]
    fn test_asin_one() {
        // asin(1) → ≈pi/2 (Req 2 Scen 1)
        assert!(approx(
            eval("asin(1)").unwrap(),
            std::f64::consts::FRAC_PI_2
        ));
    }

    #[test]
    fn test_acos_zero() {
        // acos(0) → ≈pi/2 (Req 2 Scen 2)
        assert!(approx(
            eval("acos(0)").unwrap(),
            std::f64::consts::FRAC_PI_2
        ));
    }

    #[test]
    fn test_atan_one() {
        // atan(1) → ≈pi/4 (Req 2 Scen 3)
        assert!(approx(
            eval("atan(1)").unwrap(),
            std::f64::consts::FRAC_PI_4
        ));
    }

    // ===== Requirement 3: Inverse Trigonometric Domain Validation =====

    #[test]
    fn test_asin_out_of_range() {
        // asin(2) → DomainError (Req 3 Scen 1)
        let result = eval("asin(2)");
        assert!(result.is_err());
        let err = result.as_ref().unwrap_err();
        assert_eq!(err.kind, ErrorKind::Domain);
        assert_eq!(err.hint.as_deref(), Some("asin domain is [-1, 1]"));
    }

    #[test]
    fn test_acos_out_of_range_negative() {
        // acos(-1.5) → DomainError (Req 3 Scen 2)
        let result = eval("acos(-1.5)");
        assert!(result.is_err());
        let err = result.as_ref().unwrap_err();
        assert_eq!(err.kind, ErrorKind::Domain);
        assert_eq!(err.hint.as_deref(), Some("acos domain is [-1, 1]"));
    }

    // ===== Requirement 4: Logarithmic Functions =====

    #[test]
    fn test_ln_e() {
        // ln(e) → 1.0 (Req 4 Scen 1)
        assert!(approx(eval("ln(e)").unwrap(), 1.0));
    }

    #[test]
    fn test_ln_one() {
        // ln(1) → 0.0 (Req 4 Scen 2)
        assert!(approx(eval("ln(1)").unwrap(), 0.0));
    }

    #[test]
    fn test_log10_100() {
        // log10(100) → 2.0 (Req 4 Scen 3)
        assert!(approx(eval("log10(100)").unwrap(), 2.0));
    }

    #[test]
    fn test_log2_8() {
        // log2(8) → 3.0 (Req 4 Scen 4)
        assert!(approx(eval("log2(8)").unwrap(), 3.0));
    }

    #[test]
    fn test_log_arbitrary_base() {
        // log(100, 10) → 2.0 (Req 4 Scen 5)
        assert!(approx(eval("log(100, 10)").unwrap(), 2.0));
    }

    // ===== Requirement 5: Logarithmic Domain Validation =====

    #[test]
    fn test_ln_negative() {
        // ln(-1) → DomainError (Req 5 Scen 1)
        let result = eval("ln(-1)");
        assert!(result.is_err());
        assert!(
            matches!(&result, Err(e) if e.kind == ErrorKind::Domain),
            "expected DomainError, got {:?}",
            result
        );
    }

    #[test]
    fn test_ln_zero() {
        // ln(0) → DomainError (Req 5 Scen 2)
        let result = eval("ln(0)");
        assert!(result.is_err());
        assert!(
            matches!(&result, Err(e) if e.kind == ErrorKind::Domain),
            "expected DomainError, got {:?}",
            result
        );
    }

    #[test]
    fn test_log10_negative() {
        // log10(-5) → DomainError (Req 5 Scen 3)
        let result = eval("log10(-5)");
        assert!(result.is_err());
        assert!(
            matches!(&result, Err(e) if e.kind == ErrorKind::Domain),
            "expected DomainError, got {:?}",
            result
        );
    }

    // ===== Requirement 6: Exponential Function =====

    #[test]
    fn test_exp_zero() {
        // exp(0) → 1.0 (Req 6 Scen 1)
        assert!(approx(eval("exp(0)").unwrap(), 1.0));
    }

    #[test]
    fn test_exp_one() {
        // exp(1) → ≈e (Req 6 Scen 2)
        assert!(approx(eval("exp(1)").unwrap(), std::f64::consts::E));
    }

    #[test]
    fn test_exp_ten() {
        // exp(10) → ≈22026.465794806718 (Req 6 Scen 3)
        assert!(approx(eval("exp(10)").unwrap(), 22026.465794806718));
    }

    // ===== Requirement 7: Hyperbolic Functions =====

    #[test]
    fn test_sinh_zero() {
        // sinh(0) → 0.0 (Req 7 Scen 1)
        assert!(approx(eval("sinh(0)").unwrap(), 0.0));
    }

    #[test]
    fn test_cosh_zero() {
        // cosh(0) → 1.0 (Req 7 Scen 2)
        assert!(approx(eval("cosh(0)").unwrap(), 1.0));
    }

    #[test]
    fn test_tanh_zero() {
        // tanh(0) → 0.0 (Req 7 Scen 3)
        assert!(approx(eval("tanh(0)").unwrap(), 0.0));
    }

    // ===== Requirement 8: Special Mathematical Functions =====

    #[test]
    fn test_gamma_five() {
        // gamma(5) → 24.0 (Req 8 Scen 1, Γ(5) = 4! = 24)
        assert!(approx(eval("gamma(5)").unwrap(), 24.0));
    }

    #[test]
    fn test_gamma_one() {
        // gamma(1) → 1.0 (Req 8 Scen 2)
        assert!(approx(eval("gamma(1)").unwrap(), 1.0));
    }

    #[test]
    fn test_erf_zero() {
        // erf(0) → 0.0 (Req 8 Scen 3)
        assert!(approx(eval("erf(0)").unwrap(), 0.0));
    }

    #[test]
    fn test_erf_large() {
        // erf(100) → ≈1.0 (Req 8 Scen 4)
        assert!(approx(eval("erf(100)").unwrap(), 1.0));
    }

    // ===== Requirement 9: Mathematical Constants pi and e =====

    #[test]
    fn test_constant_pi() {
        // pi → ≈3.141592653589793 (Req 9 Scen 1)
        assert!(approx(eval("pi").unwrap(), std::f64::consts::PI));
    }

    #[test]
    fn test_constant_e() {
        // e → ≈2.718281828459045 (Req 9 Scen 2)
        assert!(approx(eval("e").unwrap(), std::f64::consts::E));
    }

    // ===== Requirement 10: Mixed Scientific Computation =====

    #[test]
    fn test_sin_pi_over_two_plus_cos_zero() {
        // sin(pi/2) + cos(0) → 2.0 (Req 10 Scen 1)
        assert!(approx(eval("sin(pi/2) + cos(0)").unwrap(), 2.0));
    }

    #[test]
    fn test_exp_one_plus_ln_e() {
        // exp(1) + ln(e) → ≈e + 1 (Req 10 Scen 2)
        assert!(approx(
            eval("exp(1) + ln(e)").unwrap(),
            std::f64::consts::E + 1.0
        ));
    }

    // ===== supports() 方法测试 =====

    #[test]
    fn test_supports_scientific() {
        let domain = ScientificDomain;
        assert!(domain.supports(&parse("sin(x)").unwrap()));
        assert!(domain.supports(&parse("log(100,10)").unwrap()));
        assert!(domain.supports(&parse("gamma(5) + erf(1)").unwrap()));
        assert!(domain.supports(&parse("sinh(1) + cosh(1)").unwrap()));
        assert!(domain.supports(&parse("sin(x) + 2*3").unwrap()));
        assert!(domain.supports(&parse("pi").unwrap()));
        assert!(domain.supports(&parse("e").unwrap()));
    }

    #[test]
    fn test_does_not_support_pure_arithmetic() {
        let domain = ScientificDomain;
        assert!(!domain.supports(&parse("2+3*4").unwrap()));
        assert!(!domain.supports(&parse("42").unwrap()));
        assert!(!domain.supports(&parse("factorial(5)").unwrap()));
        assert!(!domain.supports(&parse("mod(10,3) + abs(-2)").unwrap()));
    }

    #[test]
    fn test_priority_higher_than_arithmetic() {
        let scientific = ScientificDomain;
        let arithmetic = crate::ArithmeticDomain;
        assert!(scientific.priority() > arithmetic.priority());
    }

    // ===== 覆盖未覆盖分支的补充测试 =====

    #[test]
    fn test_pi_e_auto_binding() {
        // lines 63-68: pi/e 自动绑定（EvalContext::new() 无 pi/e）
        let ast = parse("sin(pi/2) + ln(e)").unwrap();
        let domain = ScientificDomain;
        let ctx = EvalContext::new();
        let result = domain.evaluate(&ast, &ctx).unwrap();
        assert!(approx(result.as_scalar().unwrap(), 2.0));
    }

    #[test]
    fn test_unary_factorial_manual_ast() {
        // line 91: UnaryOp::Factorial（parser 不产生此节点，需手动构造）
        let ast = AstNode::UnaryOp(UnaryOp::Factorial, Box::new(AstNode::Number(5.0)));
        let domain = ScientificDomain;
        let result = domain.evaluate(&ast, &EvalContext::new()).unwrap();
        assert!(approx(result.as_scalar().unwrap(), 120.0));
    }

    #[test]
    fn test_unary_abs_manual_ast() {
        // line 92: UnaryOp::Abs（parser 不产生此节点，需手动构造）
        let ast = AstNode::UnaryOp(UnaryOp::Abs, Box::new(AstNode::Number(-3.5)));
        let domain = ScientificDomain;
        let result = domain.evaluate(&ast, &EvalContext::new()).unwrap();
        assert!(approx(result.as_scalar().unwrap(), 3.5));
    }

    #[test]
    fn test_domain_error_complex_node() {
        // lines 97-100: Complex 节点 DomainError
        let ast = AstNode::Complex(1.0, 2.0);
        let domain = ScientificDomain;
        let result = domain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(&result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_domain_error_matrix_node() {
        // lines 97-100: Matrix 节点 DomainError
        let ast = AstNode::Matrix(vec![vec![AstNode::Number(1.0)]]);
        let domain = ScientificDomain;
        let result = domain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(&result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_domain_error_list_node() {
        // lines 97-100: List 节点 DomainError
        let ast = AstNode::List(vec![AstNode::Number(1.0)]);
        let domain = ScientificDomain;
        let result = domain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(&result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_domain_error_bignumber_node() {
        // lines 97-100: BigNumber 节点 DomainError
        let ast = AstNode::BigNumber("123".to_string());
        let domain = ScientificDomain;
        let result = domain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(&result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_scalar_mul_finite() {
        // line 110: BinaryOp::Mul 正常路径
        let ast = AstNode::BinaryOp(
            BinaryOp::Mul,
            Box::new(AstNode::Number(3.0)),
            Box::new(AstNode::Number(4.0)),
        );
        let domain = ScientificDomain;
        let result = domain.evaluate(&ast, &EvalContext::new()).unwrap();
        assert!(approx(result.as_scalar().unwrap(), 12.0));
    }

    #[test]
    fn test_scalar_zero_div_zero() {
        // lines 113-115: 0/0 → NaNOrInf
        let ast = AstNode::BinaryOp(
            BinaryOp::Div,
            Box::new(AstNode::Number(0.0)),
            Box::new(AstNode::Number(0.0)),
        );
        let domain = ScientificDomain;
        let result = domain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(&result, Err(e) if e.kind == ErrorKind::NaNOrInf));
    }

    #[test]
    fn test_scalar_div_by_zero() {
        // line 116: x/0 (x≠0) → DivisionByZero
        let ast = AstNode::BinaryOp(
            BinaryOp::Div,
            Box::new(AstNode::Number(5.0)),
            Box::new(AstNode::Number(0.0)),
        );
        let domain = ScientificDomain;
        let result = domain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(&result, Err(e) if e.kind == ErrorKind::DivisionByZero));
    }

    #[test]
    fn test_scalar_zero_pow_zero() {
        // line 122: 0^0 → 1.0
        let ast = AstNode::BinaryOp(
            BinaryOp::Pow,
            Box::new(AstNode::Number(0.0)),
            Box::new(AstNode::Number(0.0)),
        );
        let domain = ScientificDomain;
        let result = domain.evaluate(&ast, &EvalContext::new()).unwrap();
        assert!(approx(result.as_scalar().unwrap(), 1.0));
    }

    #[test]
    fn test_scalar_mod_by_zero() {
        // lines 127-130: mod by zero → DivisionByZero
        let ast = AstNode::BinaryOp(
            BinaryOp::Mod,
            Box::new(AstNode::Number(10.0)),
            Box::new(AstNode::Number(0.0)),
        );
        let domain = ScientificDomain;
        let result = domain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(&result, Err(e) if e.kind == ErrorKind::DivisionByZero));
    }

    #[test]
    fn test_scalar_mod_normal() {
        // line 130: mod 正常路径
        let ast = AstNode::BinaryOp(
            BinaryOp::Mod,
            Box::new(AstNode::Number(10.0)),
            Box::new(AstNode::Number(3.0)),
        );
        let domain = ScientificDomain;
        let result = domain.evaluate(&ast, &EvalContext::new()).unwrap();
        assert!(approx(result.as_scalar().unwrap(), 1.0));
    }

    #[test]
    fn test_scalar_result_not_finite() {
        // line 134: 标量运算结果非有限 → NaNOrInf
        let ast = AstNode::BinaryOp(
            BinaryOp::Add,
            Box::new(AstNode::Number(1e308)),
            Box::new(AstNode::Number(1e308)),
        );
        let domain = ScientificDomain;
        let result = domain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(&result, Err(e) if e.kind == ErrorKind::NaNOrInf));
    }

    #[test]
    fn test_factorial_negative_input() {
        // lines 141-146: factorial 负数输入 → DomainError
        let ast = AstNode::UnaryOp(UnaryOp::Factorial, Box::new(AstNode::Number(-1.0)));
        let domain = ScientificDomain;
        let result = domain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(&result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_factorial_fractional_input() {
        // lines 141-146: factorial 小数输入 → DomainError
        let ast = AstNode::UnaryOp(UnaryOp::Factorial, Box::new(AstNode::Number(2.5)));
        let domain = ScientificDomain;
        let result = domain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(&result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_factorial_overflow_input_too_large() {
        // lines 148-149: factorial 输入超过 10000 → Overflow
        let ast = AstNode::UnaryOp(UnaryOp::Factorial, Box::new(AstNode::Number(10001.0)));
        let domain = ScientificDomain;
        let result = domain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(&result, Err(e) if e.kind == ErrorKind::Overflow));
    }

    #[test]
    fn test_factorial_overflow_during_computation() {
        // lines 154-156: factorial 计算过程中溢出 → Overflow
        let ast = AstNode::UnaryOp(UnaryOp::Factorial, Box::new(AstNode::Number(171.0)));
        let domain = ScientificDomain;
        let result = domain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(&result, Err(e) if e.kind == ErrorKind::Overflow));
    }

    #[test]
    fn test_factorial_normal_computation() {
        // lines 151-158: factorial 正常计算路径
        let ast = AstNode::UnaryOp(UnaryOp::Factorial, Box::new(AstNode::Number(6.0)));
        let domain = ScientificDomain;
        let result = domain.evaluate(&ast, &EvalContext::new()).unwrap();
        assert!(approx(result.as_scalar().unwrap(), 720.0));
    }

    #[test]
    fn test_log2_non_positive() {
        // lines 230-234: log2(0) → DomainError
        let result = eval("log2(0)");
        assert!(matches!(&result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_log_wrong_arg_count() {
        // lines 240-244: log() 参数数量错误
        let ast = AstNode::FunctionCall("log".to_string(), vec![AstNode::Number(100.0)]);
        let domain = ScientificDomain;
        let result = domain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(&result, Err(e) if e.kind == ErrorKind::Eval));
    }

    #[test]
    fn test_log_non_positive_value() {
        // lines 248-252: log(-1, 10) → DomainError
        let ast = AstNode::FunctionCall(
            "log".to_string(),
            vec![AstNode::Number(-1.0), AstNode::Number(10.0)],
        );
        let domain = ScientificDomain;
        let result = domain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(&result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_log_non_positive_base() {
        // lines 254-258: log(100, -1) → DomainError
        let ast = AstNode::FunctionCall(
            "log".to_string(),
            vec![AstNode::Number(100.0), AstNode::Number(-1.0)],
        );
        let domain = ScientificDomain;
        let result = domain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(&result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_log_base_equal_one() {
        // lines 254-258: log(100, 1) → DomainError
        let ast = AstNode::FunctionCall(
            "log".to_string(),
            vec![AstNode::Number(100.0), AstNode::Number(1.0)],
        );
        let domain = ScientificDomain;
        let result = domain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(&result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_unknown_function() {
        // line 291: unknown function
        let ast = AstNode::FunctionCall("unknown_func".to_string(), vec![AstNode::Number(1.0)]);
        let domain = ScientificDomain;
        let result = domain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(&result, Err(e) if e.kind == ErrorKind::Eval));
    }

    #[test]
    fn test_eval_one_arg_wrong_count() {
        // lines 302-307: eval_one_arg 参数数量错误
        let ast = AstNode::FunctionCall(
            "sin".to_string(),
            vec![AstNode::Number(1.0), AstNode::Number(2.0)],
        );
        let domain = ScientificDomain;
        let result = domain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(&result, Err(e) if e.kind == ErrorKind::Eval));
    }

    #[test]
    fn test_check_finite_nan_or_inf() {
        // line 315: check_finite 返回 NaNOrInf
        let result = eval("exp(1000)");
        assert!(matches!(&result, Err(e) if e.kind == ErrorKind::NaNOrInf));
    }

    #[test]
    fn test_default_impl() {
        // lines 322-324: Default impl
        let domain = ScientificDomain;
        assert_eq!(domain.domain_name(), "scientific");
        assert_eq!(domain.priority(), 20);
    }

    #[test]
    fn test_contains_scientific_matrix() {
        // lines 337-341: contains_scientific for Matrix
        let ast = AstNode::Matrix(vec![vec![AstNode::FunctionCall(
            "sin".to_string(),
            vec![AstNode::Number(1.0)],
        )]]);
        let domain = ScientificDomain;
        assert!(domain.supports(&ast));
    }

    #[test]
    fn test_contains_scientific_list() {
        // line 341: contains_scientific for List
        let ast = AstNode::List(vec![AstNode::FunctionCall(
            "cos".to_string(),
            vec![AstNode::Number(1.0)],
        )]);
        let domain = ScientificDomain;
        assert!(domain.supports(&ast));
    }

    #[test]
    fn test_lanczos_gamma_reflection() {
        // line 350: lanczos_gamma 反射公式（x < 0.5）
        // gamma(-0.5) = -2*sqrt(pi) ≈ -3.5449077018
        let result = eval("gamma(-0.5)").unwrap();
        assert!(approx(result, -2.0 * std::f64::consts::PI.sqrt()));
    }

    // ===== proptest 属性测试 =====

    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

        // 三角恒等式：sin(x)^2 + cos(x)^2 == 1
        #[test]
        fn prop_sin_cos_identity(x in -100.0f64..100.0) {
            let r = eval(&format!("sin({})^2 + cos({})^2", x, x)).unwrap();
            prop_assert!((r - 1.0).abs() < 1e-9, "sin(x)^2+cos(x)^2 should be 1, got {}", r);
        }

        // 对数恒等式：exp(ln(x)) ≈ x for x > 0
        #[test]
        fn prop_exp_ln_identity(x in 0.001f64..1000.0) {
            let r = eval(&format!("exp(ln({}))", x)).unwrap();
            prop_assert!((r - x).abs() < 1e-9 * x.abs().max(1.0), "exp(ln(x)) should be {}, got {}", x, r);
        }

        // 双曲恒等式：cosh(x)^2 - sinh(x)^2 == 1
        // 范围限制在 [-5, 5]：cosh(5)^2 ≈ 5500，f64 精度足以恢复差值 1。
        // 更大范围时 cosh²/sinh² 超出 f64 有效位数，差值退化为 0。
        #[test]
        fn prop_cosh_sinh_identity(x in -5.0f64..5.0) {
            let r = eval(&format!("cosh({})^2 - sinh({})^2", x, x)).unwrap();
            prop_assert!((r - 1.0).abs() < 1e-9, "cosh(x)^2-sinh(x)^2 should be 1, got {}", r);
        }
    }
}
