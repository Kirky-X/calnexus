// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! Scientific 计算域：三角函数、反三角函数、对数、指数、双曲函数、特殊函数。
//! priority=20（高于 Arithmetic），含科学函数或 pi/e 常量的表达式路由至本域。

use super::common::{ensure_math_constants, resolve_variable, unsupported_node_error};
use crate::core::CalculationDomain;
use crate::core::{AstNode, BinaryOp, CalcError, EvalContext, EvalResult, UnaryOp};

/// 科学函数白名单。
pub(crate) const SCIENTIFIC_FUNCTIONS: &[&str] = &[
    "sin", "cos", "tan", "asin", "acos", "atan", "ln", "log10", "log2", "log", "exp", "sinh",
    "cosh", "tanh", "gamma", "erf", "erfc", "lgamma", "polylog", "besselj", "bessely", "besseli",
    "besselk", "beta", "zeta", "airy_ai", "airy_bi", "root", "integral",
];

/// Scientific 计算域，priority=20，内置 pi/e 常量。
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
            "erfc" => self.eval_unary_math(name, args, ctx, crate::math::scientific::erfc),
            "lgamma" => self.eval_unary_math(name, args, ctx, crate::math::scientific::lgamma),
            // polylog(s, z)：阶数必须是 2/3 的整数，与 bessel 同 (阶数, 自变量) 形态
            "polylog" => self.eval_polylog(args, ctx),
            "zeta" => self.eval_unary_math(name, args, ctx, crate::math::scientific::zeta),
            "airy_ai" => self.eval_unary_math(name, args, ctx, crate::math::scientific::airy_ai),
            "airy_bi" => self.eval_unary_math(name, args, ctx, crate::math::scientific::airy_bi),
            "beta" => self.eval_binary_math(name, args, ctx, crate::math::scientific::beta),
            "besselj" => self.eval_bessel(name, args, ctx, crate::math::scientific::bessel_j),
            "bessely" => self.eval_bessel(name, args, ctx, crate::math::scientific::bessel_y),
            "besseli" => self.eval_bessel(name, args, ctx, crate::math::scientific::bessel_i),
            "besselk" => self.eval_bessel(name, args, ctx, crate::math::scientific::bessel_k),
            // ===== 数值求根 / 积分 =====
            "root" => self.eval_root(args, ctx),
            "integral" => self.eval_integral(args, ctx),
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

    /// 双参数 math 函数通用模板：求值两参数 → 调用 math 函数。
    fn eval_binary_math(
        &self,
        name: &str,
        args: &[AstNode],
        ctx: &EvalContext,
        f: impl Fn(f64, f64) -> Result<f64, CalcError>,
    ) -> Result<f64, CalcError> {
        if args.len() != 2 {
            return Err(CalcError::eval(format!(
                "{} expects 2 arguments, got {}",
                name,
                args.len()
            ))
            .with_i18n(
                "msg.scientific.arg_count_2",
                vec![
                    ("name".to_string(), name.to_string()),
                    ("actual".to_string(), args.len().to_string()),
                ],
            ));
        }
        let a = self.eval_node(&args[0], ctx)?;
        let b = self.eval_node(&args[1], ctx)?;
        f(a, b)
    }

    /// Bessel 族模板：阶数（0..=100 整数）+ 参数 x。
    fn eval_bessel(
        &self,
        name: &str,
        args: &[AstNode],
        ctx: &EvalContext,
        f: impl Fn(u32, f64) -> Result<f64, CalcError>,
    ) -> Result<f64, CalcError> {
        if args.len() != 2 {
            return Err(CalcError::eval(format!(
                "{} expects 2 arguments (order, x), got {}",
                name,
                args.len()
            ))
            .with_i18n(
                "msg.scientific.arg_count_2",
                vec![
                    ("name".to_string(), name.to_string()),
                    ("actual".to_string(), args.len().to_string()),
                ],
            ));
        }
        let order = self.eval_integer_arg(name, &args[0], ctx)?;
        let x = self.eval_node(&args[1], ctx)?;
        f(order, x)
    }

    /// `polylog(s, z)`：s 取整数阶（越界由实现层返回 Domain），z 为实自变量。
    fn eval_polylog(&self, args: &[AstNode], ctx: &EvalContext) -> Result<f64, CalcError> {
        if args.len() != 2 {
            return Err(CalcError::eval(format!(
                "polylog expects 2 arguments (order, z), got {}",
                args.len()
            ))
            .with_i18n(
                "msg.scientific.arg_count_2",
                vec![
                    ("name".to_string(), "polylog".to_string()),
                    ("actual".to_string(), args.len().to_string()),
                ],
            ));
        }
        let order = self.eval_integer_arg("polylog", &args[0], ctx)?;
        let z = self.eval_node(&args[1], ctx)?;
        crate::math::scientific::polylog(order, z)
    }

    /// 在指定变量取值下对子表达式求值（`root`/`integral` 用）。
    fn eval_expr_at(
        &self,
        expr: &AstNode,
        var: &str,
        x: f64,
        ctx: &EvalContext,
    ) -> Result<f64, CalcError> {
        let mut local = ctx.clone();
        local.vars.insert(var.to_string(), x);
        self.eval_node(expr, &local)
    }

    /// `root(expr, var, a, b)`：Brent 法求根；区间未包围根返回 Domain。
    fn eval_root(&self, args: &[AstNode], ctx: &EvalContext) -> Result<f64, CalcError> {
        let (var, a, b) = self.eval_quad_args("root", args, ctx)?;
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        let f = |x: f64| {
            self.eval_expr_at(&args[0], &var, x, ctx)
                .unwrap_or(f64::NAN)
        };
        let fa = f(lo);
        let fb = f(hi);
        if !fa.is_finite() || !fb.is_finite() {
            return Err(CalcError::nan_or_inf());
        }
        if fa * fb > 0.0 {
            return Err(CalcError::domain(format!(
                "root: interval [{}, {}] does not bracket a root",
                lo, hi
            ))
            .with_i18n(
                "msg.scientific.root_not_bracketed",
                vec![
                    ("a".to_string(), lo.to_string()),
                    ("b".to_string(), hi.to_string()),
                ],
            ));
        }
        crate::math::solvers::brent(f, lo, hi, 1e-12, 200)
    }

    /// `integral(expr, var, a, b)`：GK15 定积分，误差超阈值退回自适应 Simpson。
    fn eval_integral(&self, args: &[AstNode], ctx: &EvalContext) -> Result<f64, CalcError> {
        let (var, a, b) = self.eval_quad_args("integral", args, ctx)?;
        if a == b {
            return Ok(0.0);
        }
        let f = |x: f64| {
            self.eval_expr_at(&args[0], &var, x, ctx)
                .unwrap_or(f64::NAN)
        };
        for x in [a, 0.5 * (a + b), b] {
            if !f(x).is_finite() {
                return Err(CalcError::nan_or_inf());
            }
        }
        let (value, err) = crate::math::solvers::gauss_kronrod_15(f, a, b)?;
        if err > 1e-10 {
            crate::math::solvers::adaptive_simpson(f, a, b, 1e-10, 50)
        } else {
            Ok(value)
        }
    }

    /// `root`/`integral` 公共参数解析：4 个参数（expr, var, a, b）。
    fn eval_quad_args(
        &self,
        name: &str,
        args: &[AstNode],
        ctx: &EvalContext,
    ) -> Result<(String, f64, f64), CalcError> {
        if args.len() != 4 {
            return Err(CalcError::eval(format!(
                "{} expects 4 arguments (expr, var, a, b), got {}",
                name,
                args.len()
            ))
            .with_i18n(
                "msg.scientific.arg_count_4",
                vec![
                    ("name".to_string(), name.to_string()),
                    ("actual".to_string(), args.len().to_string()),
                ],
            ));
        }
        let var = crate::domains::common::extract_variable_name(&args[1])?;
        let a = self.eval_node(&args[2], ctx)?;
        let b = self.eval_node(&args[3], ctx)?;
        if !a.is_finite() || !b.is_finite() {
            return Err(CalcError::nan_or_inf());
        }
        Ok((var, a, b))
    }

    /// 求值 Bessel 阶数参数：非负整数且 <= 100。
    fn eval_integer_arg(
        &self,
        name: &str,
        ast: &AstNode,
        ctx: &EvalContext,
    ) -> Result<u32, CalcError> {
        let value = self.eval_node(ast, ctx)?;
        if !value.is_finite() || value < 0.0 || value.fract() != 0.0 || value > 100.0 {
            return Err(CalcError::domain(format!(
                "{}() order must be an integer in 0..=100, got {}",
                name, value
            ))
            .with_i18n(
                "msg.scientific.bessel_order_invalid",
                vec![
                    ("name".to_string(), name.to_string()),
                    ("value".to_string(), value.to_string()),
                ],
            ));
        }
        Ok(value as u32)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ErrorKind;
    use crate::core::parse;

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

    // ===== Trigonometric Functions in Radians =====

    #[test]
    fn test_sin_zero() {
        assert!(approx(eval("sin(0)").unwrap(), 0.0));
    }

    #[test]
    fn test_cos_zero() {
        assert!(approx(eval("cos(0)").unwrap(), 1.0));
    }

    #[test]
    fn test_sin_pi_over_two() {
        assert!(approx(eval("sin(pi/2)").unwrap(), 1.0));
    }

    #[test]
    fn test_tan_pi_over_four() {
        assert!(approx(eval("tan(pi/4)").unwrap(), 1.0));
    }

    // ===== Inverse Trigonometric Functions =====

    #[test]
    fn test_asin_one() {
        assert!(approx(
            eval("asin(1)").unwrap(),
            std::f64::consts::FRAC_PI_2
        ));
    }

    #[test]
    fn test_acos_zero() {
        assert!(approx(
            eval("acos(0)").unwrap(),
            std::f64::consts::FRAC_PI_2
        ));
    }

    #[test]
    fn test_atan_one() {
        assert!(approx(
            eval("atan(1)").unwrap(),
            std::f64::consts::FRAC_PI_4
        ));
    }

    // ===== Inverse Trigonometric Domain Validation =====

    #[test]
    fn test_asin_out_of_range() {
        let result = eval("asin(2)");
        assert!(result.is_err());
        let err = result.as_ref().unwrap_err();
        assert_eq!(err.kind, ErrorKind::Domain);
        assert_eq!(err.hint.as_deref(), Some("asin domain is [-1, 1]"));
    }

    #[test]
    fn test_acos_out_of_range_negative() {
        let result = eval("acos(-1.5)");
        assert!(result.is_err());
        let err = result.as_ref().unwrap_err();
        assert_eq!(err.kind, ErrorKind::Domain);
        assert_eq!(err.hint.as_deref(), Some("acos domain is [-1, 1]"));
    }

    // ===== Logarithmic Functions =====

    #[test]
    fn test_ln_e() {
        assert!(approx(eval("ln(e)").unwrap(), 1.0));
    }

    #[test]
    fn test_ln_one() {
        assert!(approx(eval("ln(1)").unwrap(), 0.0));
    }

    #[test]
    fn test_log10_100() {
        assert!(approx(eval("log10(100)").unwrap(), 2.0));
    }

    #[test]
    fn test_log2_8() {
        assert!(approx(eval("log2(8)").unwrap(), 3.0));
    }

    #[test]
    fn test_log_arbitrary_base() {
        assert!(approx(eval("log(100, 10)").unwrap(), 2.0));
    }

    // ===== Logarithmic Domain Validation =====

    #[test]
    fn test_ln_negative() {
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
        let result = eval("log10(-5)");
        assert!(result.is_err());
        assert!(
            matches!(&result, Err(e) if e.kind == ErrorKind::Domain),
            "expected DomainError, got {:?}",
            result
        );
    }

    // ===== Exponential Function =====

    #[test]
    fn test_exp_zero() {
        assert!(approx(eval("exp(0)").unwrap(), 1.0));
    }

    #[test]
    fn test_exp_one() {
        assert!(approx(eval("exp(1)").unwrap(), std::f64::consts::E));
    }

    #[test]
    fn test_exp_ten() {
        assert!(approx(eval("exp(10)").unwrap(), 22026.465794806718));
    }

    // ===== Hyperbolic Functions =====

    #[test]
    fn test_sinh_zero() {
        assert!(approx(eval("sinh(0)").unwrap(), 0.0));
    }

    #[test]
    fn test_cosh_zero() {
        assert!(approx(eval("cosh(0)").unwrap(), 1.0));
    }

    #[test]
    fn test_tanh_zero() {
        assert!(approx(eval("tanh(0)").unwrap(), 0.0));
    }

    // ===== Special Mathematical Functions =====

    #[test]
    fn test_gamma_five() {
        assert!(approx(eval("gamma(5)").unwrap(), 24.0));
    }

    #[test]
    fn test_gamma_one() {
        assert!(approx(eval("gamma(1)").unwrap(), 1.0));
    }

    #[test]
    fn test_erf_zero() {
        assert!(approx(eval("erf(0)").unwrap(), 0.0));
    }

    #[test]
    fn test_erf_large() {
        assert!(approx(eval("erf(100)").unwrap(), 1.0));
    }

    // ===== Mathematical Constants pi and e =====

    #[test]
    fn test_constant_pi() {
        assert!(approx(eval("pi").unwrap(), std::f64::consts::PI));
    }

    #[test]
    fn test_constant_e() {
        assert!(approx(eval("e").unwrap(), std::f64::consts::E));
    }

    // ===== Mixed Scientific Computation =====

    #[test]
    fn test_sin_pi_over_two_plus_cos_zero() {
        assert!(approx(eval("sin(pi/2) + cos(0)").unwrap(), 2.0));
    }

    #[test]
    fn test_exp_one_plus_ln_e() {
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
        // pi/e 自动绑定（EvalContext::new() 无 pi/e）
        let ast = parse("sin(pi/2) + ln(e)").unwrap();
        let domain = ScientificDomain;
        let ctx = EvalContext::new();
        let result = domain.evaluate(&ast, &ctx).unwrap();
        assert!(approx(result.as_scalar().unwrap(), 2.0));
    }

    #[test]
    fn test_unary_factorial_manual_ast() {
        // UnaryOp::Factorial（parser 不产生此节点，需手动构造）
        let ast = AstNode::UnaryOp(UnaryOp::Factorial, Box::new(AstNode::Number(5.0)));
        let domain = ScientificDomain;
        let result = domain.evaluate(&ast, &EvalContext::new()).unwrap();
        assert!(approx(result.as_scalar().unwrap(), 120.0));
    }

    #[test]
    fn test_unary_abs_manual_ast() {
        // UnaryOp::Abs（parser 不产生此节点，需手动构造）
        let ast = AstNode::UnaryOp(UnaryOp::Abs, Box::new(AstNode::Number(-3.5)));
        let domain = ScientificDomain;
        let result = domain.evaluate(&ast, &EvalContext::new()).unwrap();
        assert!(approx(result.as_scalar().unwrap(), 3.5));
    }

    #[test]
    fn test_domain_error_complex_node() {
        // Complex 节点 DomainError
        let ast = AstNode::Complex(1.0, 2.0);
        let domain = ScientificDomain;
        let result = domain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(&result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_domain_error_matrix_node() {
        // Matrix 节点 DomainError
        let ast = AstNode::Matrix(vec![vec![AstNode::Number(1.0)]]);
        let domain = ScientificDomain;
        let result = domain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(&result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_domain_error_list_node() {
        // List 节点 DomainError
        let ast = AstNode::List(vec![AstNode::Number(1.0)]);
        let domain = ScientificDomain;
        let result = domain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(&result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_domain_error_bignumber_node() {
        // BigNumber 节点 DomainError
        let ast = AstNode::BigNumber("123".to_string());
        let domain = ScientificDomain;
        let result = domain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(&result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_scalar_mul_finite() {
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
        // 0/0 → NaNOrInf
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
        // x/0 (x≠0) → DivisionByZero
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
        // 0^0 → 1.0
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
        // mod by zero → DivisionByZero
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
        // 标量运算结果非有限 → NaNOrInf
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
        // factorial 负数输入 → DomainError
        let ast = AstNode::UnaryOp(UnaryOp::Factorial, Box::new(AstNode::Number(-1.0)));
        let domain = ScientificDomain;
        let result = domain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(&result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_factorial_fractional_input() {
        // factorial 小数输入 → DomainError
        let ast = AstNode::UnaryOp(UnaryOp::Factorial, Box::new(AstNode::Number(2.5)));
        let domain = ScientificDomain;
        let result = domain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(&result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_factorial_overflow_input_too_large() {
        // factorial 输入超过 10000 → Overflow
        let ast = AstNode::UnaryOp(UnaryOp::Factorial, Box::new(AstNode::Number(10001.0)));
        let domain = ScientificDomain;
        let result = domain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(&result, Err(e) if e.kind == ErrorKind::Overflow));
    }

    #[test]
    fn test_factorial_overflow_during_computation() {
        // factorial 计算过程中溢出 → Overflow
        let ast = AstNode::UnaryOp(UnaryOp::Factorial, Box::new(AstNode::Number(171.0)));
        let domain = ScientificDomain;
        let result = domain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(&result, Err(e) if e.kind == ErrorKind::Overflow));
    }

    #[test]
    fn test_factorial_normal_computation() {
        let ast = AstNode::UnaryOp(UnaryOp::Factorial, Box::new(AstNode::Number(6.0)));
        let domain = ScientificDomain;
        let result = domain.evaluate(&ast, &EvalContext::new()).unwrap();
        assert!(approx(result.as_scalar().unwrap(), 720.0));
    }

    #[test]
    fn test_log2_non_positive() {
        // log2(0) → DomainError
        let result = eval("log2(0)");
        assert!(matches!(&result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_log_wrong_arg_count() {
        // log() 参数数量错误
        let ast = AstNode::FunctionCall("log".to_string(), vec![AstNode::Number(100.0)]);
        let domain = ScientificDomain;
        let result = domain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(&result, Err(e) if e.kind == ErrorKind::Eval));
    }

    #[test]
    fn test_log_non_positive_value() {
        // log(-1, 10) → DomainError
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
        // log(100, -1) → DomainError
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
        // log(100, 1) → DomainError
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
        // unknown function
        let ast = AstNode::FunctionCall("unknown_func".to_string(), vec![AstNode::Number(1.0)]);
        let domain = ScientificDomain;
        let result = domain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(&result, Err(e) if e.kind == ErrorKind::Eval));
    }

    #[test]
    fn test_eval_one_arg_wrong_count() {
        // eval_one_arg 参数数量错误
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
        // check_finite 返回 NaNOrInf
        let result = eval("exp(1000)");
        assert!(matches!(&result, Err(e) if e.kind == ErrorKind::NaNOrInf));
    }

    #[test]
    fn test_default_impl() {
        let domain = ScientificDomain;
        assert_eq!(domain.domain_name(), "scientific");
        assert_eq!(domain.priority(), 20);
    }

    #[test]
    fn test_contains_scientific_matrix() {
        // contains_scientific for Matrix
        let ast = AstNode::Matrix(vec![vec![AstNode::FunctionCall(
            "sin".to_string(),
            vec![AstNode::Number(1.0)],
        )]]);
        let domain = ScientificDomain;
        assert!(domain.supports(&ast));
    }

    #[test]
    fn test_contains_scientific_list() {
        // contains_scientific for List
        let ast = AstNode::List(vec![AstNode::FunctionCall(
            "cos".to_string(),
            vec![AstNode::Number(1.0)],
        )]);
        let domain = ScientificDomain;
        assert!(domain.supports(&ast));
    }

    #[test]
    fn test_lanczos_gamma_reflection() {
        // lanczos_gamma 反射公式（x < 0.5）。
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

        // 双曲恒等式：cosh(x)^2 - sinh(x)^2 == 1（限 [-5,5]，大范围超出 f64 精度）。
        #[test]
        fn prop_cosh_sinh_identity(x in -5.0f64..5.0) {
            let r = eval(&format!("cosh({})^2 - sinh({})^2", x, x)).unwrap();
            prop_assert!((r - 1.0).abs() < 1e-9, "cosh(x)^2-sinh(x)^2 should be 1, got {}", r);
        }
    }
}
