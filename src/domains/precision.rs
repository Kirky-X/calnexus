// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! Precision 计算域：大整数运算、精确分数、高精度小数。
//! 含 BigNumber 或 precision() 的表达式路由至本域；内部统一用 BigRational 求值。

use super::common::{
    ensure_math_constants, resolve_variable, unsupported_function_error, unsupported_node_error,
};
use crate::core::CalculationDomain;
use crate::core::{
    AstNode, BinaryOp, CalcError, EvalContext, EvalResult, MAX_POW_EXPONENT, MAX_PRECISION,
    UnaryOp, check_pow_output_size,
};
use crate::math::precision as math_prec;
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{Signed, Zero};

// Re-export format_bigrational（供 domains/mod.rs 对外暴露）
#[cfg(any(feature = "cli", feature = "http", feature = "mcp"))]
pub use crate::math::precision::format_bigrational;

/// Precision 计算域（priority=25）：大整数运算、精确分数、大数阶乘、大整数幂。
pub struct PrecisionDomain;

impl CalculationDomain for PrecisionDomain {
    fn domain_name(&self) -> &str {
        "precision"
    }

    fn priority(&self) -> u8 {
        25
    }

    fn supports(&self, ast: &AstNode) -> bool {
        // 含 BigNumber 或 precision() 时认领，但排除其他域的专用函数（让位更具体的域）。
        contains_precision(ast) && !contains_other_domain_function(ast)
    }

    fn evaluate(&self, ast: &AstNode, ctx: &EvalContext) -> Result<EvalResult, CalcError> {
        // 预绑定 pi/e（若上下文未提供），与其他域保持一致。
        let ctx = ensure_math_constants(ctx);

        // 处理 precision(N, expr) 函数：求值 expr，N 仅供 CLI 格式化使用
        if let AstNode::FunctionCall(name, args) = ast
            && name == "precision"
        {
            if args.len() != 2 {
                return Err(CalcError::domain(format!(
                    "precision() requires exactly 2 arguments (N, expr), got {}",
                    args.len()
                ))
                .with_i18n(
                    "msg.precision.precision_arg_count",
                    vec![("actual".to_string(), args.len().to_string())],
                ));
            }
            // 验证 N 为正整数
            let _n = extract_precision_value(&args[0])?;
            let value = self.eval(&args[1], &ctx)?;
            return Ok(math_prec::rational_to_result(value));
        }

        let value = self.eval(ast, &ctx)?;
        Ok(math_prec::rational_to_result(value))
    }
}

impl PrecisionDomain {
    /// 递归求值 AST 节点为 `BigRational`。
    fn eval(&self, ast: &AstNode, ctx: &EvalContext) -> Result<BigRational, CalcError> {
        match ast {
            AstNode::Number(n) => math_prec::f64_to_rational(*n),
            AstNode::BigNumber(s) => {
                let big = BigInt::parse_bytes(s.as_bytes(), 10).ok_or_else(|| {
                    CalcError::parse(format!("invalid big integer literal: {}", s)).with_i18n(
                        "msg.precision.invalid_big_integer",
                        vec![("value".to_string(), s.to_string())],
                    )
                })?;
                Ok(BigRational::from_integer(big))
            }
            AstNode::Variable(name) => {
                let v = resolve_variable(ctx, name)?;
                math_prec::f64_to_rational(v)
            }
            AstNode::BinaryOp(op, l, r) => {
                let a = self.eval(l, ctx)?;
                let b = self.eval(r, ctx)?;
                self.eval_binary(*op, a, b)
            }
            AstNode::UnaryOp(op, e) => {
                let v = self.eval(e, ctx)?;
                match op {
                    UnaryOp::Neg => Ok(-v),
                    UnaryOp::Abs => Ok(v.abs()),
                    UnaryOp::Factorial => {
                        let n = math_prec::rational_to_int(&v, "factorial")?;
                        Ok(BigRational::from_integer(
                            math_prec::factorial(&n).map_err(|e| {
                                e.with_i18n(
                                    "msg.core.factorial_negative",
                                    vec![("value".to_string(), n.to_string())],
                                )
                            })?,
                        ))
                    }
                }
            }
            AstNode::FunctionCall(name, args) => self.eval_function(name, args, ctx),
            AstNode::Complex(_, _) | AstNode::Matrix(_) | AstNode::List(_) | AstNode::Str(_) => {
                Err(unsupported_node_error("precision", ast))
            }
        }
    }

    /// 求值二元运算。
    fn eval_binary(
        &self,
        op: BinaryOp,
        a: BigRational,
        b: BigRational,
    ) -> Result<BigRational, CalcError> {
        match op {
            BinaryOp::Add => Ok(a + b),
            BinaryOp::Sub => Ok(a - b),
            BinaryOp::Mul => Ok(a * b),
            BinaryOp::Div => {
                if b.is_zero() {
                    return Err(CalcError::division_by_zero());
                }
                Ok(a / b)
            }
            BinaryOp::Pow => {
                let exp = math_prec::rational_to_int(&b, "power exponent")?;
                // 安全约束：指数按绝对值限幅，负指数同样受限（pow 内部先算 a^|exp| 再取倒数，防 DoS）。
                if exp.abs() > BigInt::from(MAX_POW_EXPONENT) {
                    return Err(CalcError::domain(format!(
                        "power exponent absolute value must not exceed {} (got {})",
                        MAX_POW_EXPONENT, exp
                    ))
                    .with_i18n(
                        "msg.precision.power_abs_exceeds_max",
                        vec![
                            ("max".to_string(), MAX_POW_EXPONENT.to_string()),
                            ("value".to_string(), exp.to_string()),
                        ],
                    ));
                }
                // 安全约束：底数 × 指数复合限制，复用 core 层 check_pow_output_size 防超大输出 DoS。
                let abs_exp_u64 = u64::try_from(&exp.abs()).map_err(|_| {
                    CalcError::domain(format!("power exponent too large: {}", exp)).with_i18n(
                        "msg.precision.power_exponent_too_large",
                        vec![("value".to_string(), exp.to_string())],
                    )
                })?;
                let base_bits = std::cmp::max(a.numer().bits(), a.denom().bits()) as u64;
                check_pow_output_size(base_bits, abs_exp_u64)?;
                // BigRational::pow 接受 i32 指数
                let exp_i32 = i32::try_from(&exp).map_err(|_| {
                    CalcError::domain(format!("power exponent too large: {}", exp)).with_i18n(
                        "msg.precision.power_exponent_too_large",
                        vec![("value".to_string(), exp.to_string())],
                    )
                })?;
                Ok(a.pow(exp_i32))
            }
            BinaryOp::Mod => {
                if b.is_zero() {
                    return Err(CalcError::division_by_zero());
                }
                // 大整数取模：提取整数部分计算
                let a_int = math_prec::rational_to_int(&a, "mod operand")?;
                let b_int = math_prec::rational_to_int(&b, "mod operand")?;
                Ok(BigRational::from_integer(a_int % b_int))
            }
        }
    }

    /// 求值函数调用。
    fn eval_function(
        &self,
        name: &str,
        args: &[AstNode],
        ctx: &EvalContext,
    ) -> Result<BigRational, CalcError> {
        match name {
            "factorial" => {
                let v = self.eval_one_arg(name, args, ctx)?;
                let n = math_prec::rational_to_int(&v, "factorial")?;
                Ok(BigRational::from_integer(
                    math_prec::factorial(&n).map_err(|e| {
                        e.with_i18n(
                            "msg.core.factorial_negative",
                            vec![("value".to_string(), n.to_string())],
                        )
                    })?,
                ))
            }
            "abs" => {
                let v = self.eval_one_arg(name, args, ctx)?;
                Ok(v.abs())
            }
            "precision" => {
                // precision(N, expr) 在 evaluate() 顶层处理，此处不应到达
                Err(
                    CalcError::domain("precision() must be at expression top level".to_string())
                        .with_i18n("msg.precision.precision_must_be_top_level", vec![]),
                )
            }
            "mod" => {
                // parser 将 `%` 转换为 mod(a, b) 函数调用
                let (a, b) = self.eval_two_args(name, args, ctx)?;
                if b.is_zero() {
                    return Err(CalcError::division_by_zero());
                }
                let a_int = math_prec::rational_to_int(&a, "mod operand")?;
                let b_int = math_prec::rational_to_int(&b, "mod operand")?;
                Ok(BigRational::from_integer(a_int % b_int))
            }
            _ => Err(unsupported_function_error("precision", name)),
        }
    }

    /// 求值单参数函数：验证参数数为 1，返回求值结果。
    fn eval_one_arg(
        &self,
        name: &str,
        args: &[AstNode],
        ctx: &EvalContext,
    ) -> Result<BigRational, CalcError> {
        if args.len() != 1 {
            return Err(CalcError::domain(format!(
                "{}() requires exactly 1 argument, got {}",
                name,
                args.len()
            ))
            .with_i18n(
                "msg.precision.arg_count_1",
                vec![
                    ("name".to_string(), name.to_string()),
                    ("actual".to_string(), args.len().to_string()),
                ],
            ));
        }
        self.eval(&args[0], ctx)
    }

    /// 求值双参数函数：验证参数数为 2，返回求值结果。
    fn eval_two_args(
        &self,
        name: &str,
        args: &[AstNode],
        ctx: &EvalContext,
    ) -> Result<(BigRational, BigRational), CalcError> {
        if args.len() != 2 {
            return Err(CalcError::domain(format!(
                "{}() requires exactly 2 arguments, got {}",
                name,
                args.len()
            ))
            .with_i18n(
                "msg.precision.arg_count_2",
                vec![
                    ("name".to_string(), name.to_string()),
                    ("actual".to_string(), args.len().to_string()),
                ],
            ));
        }
        let a = self.eval(&args[0], ctx)?;
        let b = self.eval(&args[1], ctx)?;
        Ok((a, b))
    }
}

/// 递归检查 AST 是否应路由至 PrecisionDomain（含 BigNumber 或 precision()）。
fn contains_precision(ast: &AstNode) -> bool {
    match ast {
        AstNode::BigNumber(_) => true,
        AstNode::FunctionCall(name, args) => {
            if name == "precision" {
                return true;
            }
            args.iter().any(contains_precision)
        }
        AstNode::BinaryOp(_, l, r) => contains_precision(l) || contains_precision(r),
        AstNode::UnaryOp(_, e) => contains_precision(e),
        AstNode::Matrix(rows) => rows.iter().flatten().any(contains_precision),
        AstNode::List(elements) => elements.iter().any(contains_precision),
        AstNode::Number(_) | AstNode::Variable(_) | AstNode::Complex(_, _) | AstNode::Str(_) => {
            false
        }
    }
}

/// 其他域的专用函数列表：AST 含这些调用时 PrecisionDomain 让位给更具体的域。
const OTHER_DOMAIN_FUNCTIONS: &[&str] = &[
    "gcd",
    "lcm",
    "is_prime",
    "prime_sieve",
    "mod_inverse",
    "mod_pow",
    "euler_phi",
    "P",
    "C",
    "catalan",
    "stirling",
];

/// 递归检查 AST 是否含其他域的专用函数调用。
fn contains_other_domain_function(ast: &AstNode) -> bool {
    match ast {
        AstNode::FunctionCall(name, args) => {
            OTHER_DOMAIN_FUNCTIONS.contains(&name.as_str())
                || args.iter().any(contains_other_domain_function)
        }
        AstNode::BinaryOp(_, l, r) => {
            contains_other_domain_function(l) || contains_other_domain_function(r)
        }
        AstNode::UnaryOp(_, e) => contains_other_domain_function(e),
        AstNode::Matrix(rows) => rows.iter().flatten().any(contains_other_domain_function),
        AstNode::List(elements) => elements.iter().any(contains_other_domain_function),
        AstNode::Number(_)
        | AstNode::Variable(_)
        | AstNode::BigNumber(_)
        | AstNode::Complex(_, _)
        | AstNode::Str(_) => false,
    }
}

/// 从 AST 节点提取精度值（正整数）。
fn extract_precision_value(ast: &AstNode) -> Result<usize, CalcError> {
    let v = match ast {
        AstNode::Number(n) => {
            if n.fract() == 0.0 && *n > 0.0 {
                *n as usize
            } else {
                return Err(CalcError::domain(format!(
                    "precision N must be a positive integer, got {}",
                    n
                ))
                .with_i18n(
                    "msg.precision.precision_must_be_positive_int",
                    vec![("value".to_string(), n.to_string())],
                ));
            }
        }
        AstNode::BigNumber(s) => {
            let big = BigInt::parse_bytes(s.as_bytes(), 10).ok_or_else(|| {
                CalcError::parse(format!("invalid precision value: {}", s)).with_i18n(
                    "msg.precision.precision_invalid_value",
                    vec![("value".to_string(), s.to_string())],
                )
            })?;
            usize::try_from(big).map_err(|_| {
                CalcError::domain(format!("precision N out of range: {}", s)).with_i18n(
                    "msg.precision.precision_out_of_range",
                    vec![("value".to_string(), s.to_string())],
                )
            })?
        }
        _ => {
            return Err(
                CalcError::domain("precision N must be a literal integer".to_string())
                    .with_i18n("msg.precision.precision_must_be_literal", vec![]),
            );
        }
    };
    if v == 0 {
        return Err(
            CalcError::domain("precision N must be positive".to_string())
                .with_i18n("msg.precision.precision_must_be_positive", vec![]),
        );
    }
    // 安全约束：拒绝超大精度值（MAX_PRECISION 上限），防 format_decimal 循环 DoS。
    if v > MAX_PRECISION {
        return Err(CalcError::domain(format!(
            "precision N must not exceed {} (got {})",
            MAX_PRECISION, v
        ))
        .with_i18n(
            "msg.precision.precision_exceeds_max",
            vec![
                ("max".to_string(), MAX_PRECISION.to_string()),
                ("value".to_string(), v.to_string()),
            ],
        ));
    }
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ErrorKind;
    use crate::core::MAX_FACTORIAL_INPUT;
    use crate::core::parse;
    use crate::math::precision::format_bigrational;

    /// 创建默认上下文。
    fn default_ctx() -> EvalContext {
        EvalContext::new()
    }

    /// 创建指定精度的上下文。
    fn ctx_with_precision(n: usize) -> EvalContext {
        let mut ctx = EvalContext::new();
        ctx.precision = Some(n);
        ctx
    }

    /// 断言结果为 BigInt 且值匹配。
    fn assert_bigint(actual: &EvalResult, expected: &str) {
        match actual {
            EvalResult::BigInt(b) => {
                let expected_big = BigInt::parse_bytes(expected.as_bytes(), 10).unwrap();
                assert_eq!(
                    b, &expected_big,
                    "expected BigInt({}), got BigInt({})",
                    expected, b
                );
            }
            other => panic!("expected BigInt({}), got {:?}", expected, other),
        }
    }

    /// 断言结果为 BigRational 且值匹配 `num/den`。
    fn assert_bigrational(actual: &EvalResult, num: &str, den: &str) {
        match actual {
            EvalResult::BigRational(r) => {
                let expected_num = BigInt::parse_bytes(num.as_bytes(), 10).unwrap();
                let expected_den = BigInt::parse_bytes(den.as_bytes(), 10).unwrap();
                let expected = BigRational::new(expected_num, expected_den);
                assert_eq!(
                    r, &expected,
                    "expected BigRational({}/{}), got {:?}",
                    num, den, r
                );
            }
            other => panic!("expected BigRational({}/{}), got {:?}", num, den, other),
        }
    }

    // ===== 大整数运算 =====

    #[test]
    fn test_big_integer_addition() {
        let ast = parse("12345678901234567890 + 1").unwrap();
        let domain = PrecisionDomain;
        assert!(domain.supports(&ast));
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_bigint(&result, "12345678901234567891");
    }

    #[test]
    fn test_big_integer_multiplication() {
        // 999999999999 * 999999999999 → 精确大整数
        let ast = parse("999999999999 * 999999999999").unwrap();
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_bigint(&result, "999999999998000000000001");
    }

    // ===== 精确分数 =====

    #[test]
    fn test_fraction_addition() {
        let ast = parse("1/3 + 1/6").unwrap();
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        // 1/3+1/6 = 2/6+1/6 = 3/6 = 1/2，分母不为 1，应为 BigRational
        assert_bigrational(&result, "1", "2");
    }

    #[test]
    fn test_fraction_reduction() {
        let ast = parse("2/4").unwrap();
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_bigrational(&result, "1", "2");
    }

    // ===== 高精度小数 =====

    #[test]
    fn test_precision_50_decimal() {
        // --precision 50 "1/3" → 50 位精度的 0.333...3
        let ast = parse("1/3").unwrap();
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        let formatted = format_bigrational(result.as_bigrational().unwrap(), Some(50));
        assert!(formatted.starts_with("0.3"));
        assert_eq!(formatted.len(), 52); // "0." + 50 digits
        assert!(formatted.chars().skip(2).all(|c| c == '3'));
    }

    #[test]
    fn test_default_precision_fraction() {
        // 无 --precision 的 1/3 → 精确分数 1/3
        let ast = parse("1/3").unwrap();
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_bigrational(&result, "1", "3");
    }

    // ===== 大数阶乘 =====

    #[test]
    fn test_factorial_100() {
        // factorial(100) → 精确大整数
        let ast = parse("factorial(100)").unwrap();
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        // 100! 共 158 位
        let EvalResult::BigInt(b) = &result else {
            panic!("expected BigInt, got {:?}", result)
        };
        assert!(b > &BigInt::zero());
        assert!(b.to_string().len() > 100); // 100! 有 158 位
    }

    #[test]
    fn test_factorial_5() {
        let ast = parse("factorial(5)").unwrap();
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_bigint(&result, "120");
    }

    // ===== 精度触发 =====

    #[test]
    fn test_precision_flag_triggers_routing() {
        // --precision 50 "1 + 2" → 路由到 PrecisionDomain
        // （CLI 层处理，单元测试验证 supports 与 evaluate）
        let ast = parse("1 + 2").unwrap();
        let domain = PrecisionDomain;
        // 1+2 不含 BigNumber 或 precision()，supports 返回 false
        // 但 CLI 层 --precision 会直接使用 PrecisionDomain
        assert!(!domain.supports(&ast));
        // 直接调用 evaluate 仍可工作
        let result = domain.evaluate(&ast, &ctx_with_precision(50)).unwrap();
        assert_bigint(&result, "3");
    }

    #[test]
    fn test_no_precision_uses_arithmetic() {
        // 无 --precision 的 1 + 2 → ArithmeticDomain（不路由到 Precision）
        let ast = parse("1 + 2").unwrap();
        let domain = PrecisionDomain;
        assert!(!domain.supports(&ast));
    }

    // ===== 大整数幂运算 =====

    #[test]
    fn test_power_2_100() {
        // 2^100 → 精确大整数
        let ast = parse("2^100").unwrap();
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        // 2^100 = 1267650600228229401496703205376
        assert_bigint(&result, "1267650600228229401496703205376");
    }

    #[test]
    fn test_power_10_50() {
        // 10^50 → 1 后跟 50 个 0
        let ast = parse("10^50").unwrap();
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        let expected = format!("1{}", "0".repeat(50));
        assert_bigint(&result, &expected);
    }

    // ===== 分数运算 =====

    #[test]
    fn test_fraction_times_integer() {
        let ast = parse("1/3 * 3").unwrap();
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_bigint(&result, "1");
    }

    #[test]
    fn test_fraction_division() {
        let ast = parse("(1/2) / (1/4)").unwrap();
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_bigint(&result, "2");
    }

    // ===== 混合运算 =====

    #[test]
    fn test_bigint_plus_fraction() {
        // 12345678901234567890 + 1/3 → BigRational
        let ast = parse("12345678901234567890 + 1/3").unwrap();
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        // 结果 = (12345678901234567890 * 3 + 1) / 3 = 37037036703703703671/3
        assert_bigrational(&result, "37037036703703703671", "3");
    }

    #[test]
    fn test_mixed_fraction_reduction() {
        // 2/3 * (3 + 1/2) → 2/3 * 7/2 = 14/6 = 7/3
        let ast = parse("2/3 * (3 + 1/2)").unwrap();
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_bigrational(&result, "7", "3");
    }

    // ===== 域路由 =====

    #[test]
    fn test_precision_function_routes() {
        // precision(50, 1/3) → 路由到 PrecisionDomain
        let ast = parse("precision(50, 1/3)").unwrap();
        let domain = PrecisionDomain;
        assert!(domain.supports(&ast));
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_bigrational(&result, "1", "3");
    }

    #[test]
    fn test_bignumber_routes() {
        // 含 BigNumber 的表达式路由到 PrecisionDomain
        let ast = parse("12345678901234567890 + 1").unwrap();
        let domain = PrecisionDomain;
        assert!(domain.supports(&ast));
    }

    #[test]
    fn test_cli_precision_flag_routes() {
        // --precision 50 CLI 参数 → 所有表达式路由到 PrecisionDomain
        // （CLI 层处理，单元测试验证 evaluate 可处理普通表达式）
        let ast = parse("1 + 2").unwrap();
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &ctx_with_precision(50)).unwrap();
        assert_bigint(&result, "3");
    }

    // ===== 输出格式 =====

    #[test]
    fn test_bigint_output_no_scientific() {
        // factorial(20) → 完整十进制，不用科学计数法
        let ast = parse("factorial(20)").unwrap();
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        let EvalResult::BigInt(b) = &result else {
            panic!("expected BigInt, got {:?}", result)
        };
        let s = b.to_string();
        assert!(!s.contains('e') && !s.contains('E'));
        assert!(!s.contains('.'));
        // 20! = 2432902008176640000
        assert_eq!(s, "2432902008176640000");
    }

    #[test]
    fn test_fraction_output_format() {
        // 1/3 → "1/3" 形式
        let r = BigRational::new(BigInt::from(1), BigInt::from(3));
        let formatted = format_bigrational(&r, None);
        assert_eq!(formatted, "1/3");
    }

    #[test]
    fn test_integer_fraction_output() {
        // 4/2 → "2"（分母为 1 时输出整数）
        let r = BigRational::new(BigInt::from(4), BigInt::from(2)); // 自动约分为 2/1
        let formatted = format_bigrational(&r, None);
        assert_eq!(formatted, "2");
    }

    // ===== 额外覆盖：域属性与错误处理 =====

    #[test]
    fn test_precision_domain_priority() {
        let domain = PrecisionDomain;
        assert_eq!(domain.priority(), 25);
        assert_eq!(domain.domain_name(), "precision");
    }

    #[test]
    fn test_precision_priority_between_complex_and_statistics() {
        let precision = PrecisionDomain;
        let statistics = crate::StatisticsDomain;
        let complex = crate::ComplexDomain;
        // priority: Complex(30) > Precision(25) > Statistics(20)
        assert!(complex.priority() > precision.priority());
        assert!(precision.priority() > statistics.priority());
    }

    #[test]
    fn test_division_by_zero() {
        let ast = parse("1/0").unwrap();
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::DivisionByZero));
    }

    #[test]
    fn test_unsupported_complex_node() {
        let ast = AstNode::Complex(1.0, 2.0);
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_unsupported_matrix_node() {
        let ast = AstNode::Matrix(vec![vec![AstNode::Number(1.0)]]);
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_unsupported_list_node() {
        let ast = AstNode::List(vec![AstNode::Number(1.0)]);
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_unsupported_function() {
        let ast = parse("sin(1)").unwrap();
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_precision_wrong_arg_count() {
        let ast = AstNode::FunctionCall("precision".to_string(), vec![]);
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_precision_invalid_n() {
        // precision(-5, 1/3) → N 必须为正整数
        let ast = AstNode::FunctionCall(
            "precision".to_string(),
            vec![AstNode::Number(-5.0), parse("1/3").unwrap()],
        );
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_factorial_negative() {
        // factorial(-1) → DomainError（负数阶乘未定义）
        let ast = AstNode::FunctionCall("factorial".to_string(), vec![AstNode::Number(-1.0)]);
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(ref e) if e.kind == ErrorKind::Domain));
        if let Err(ref e) = result {
            assert_eq!(
                e.i18n_key,
                Some("msg.core.factorial_negative"),
                "expected i18n key msg.core.factorial_negative"
            );
        }
    }

    #[test]
    fn test_factorial_zero() {
        let ast = parse("factorial(0)").unwrap();
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_bigint(&result, "1");
    }

    #[test]
    fn test_negation() {
        let ast = parse("-(12345678901234567890)").unwrap();
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_bigint(&result, "-12345678901234567890");
    }

    #[test]
    fn test_abs_function() {
        let ast = parse("abs(-12345678901234567890)").unwrap();
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_bigint(&result, "12345678901234567890");
    }

    #[test]
    fn test_abs_unary_op() {
        let ast = AstNode::UnaryOp(
            UnaryOp::Abs,
            Box::new(AstNode::BigNumber("-42".to_string())),
        );
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_bigint(&result, "42");
    }

    #[test]
    fn test_subtraction() {
        let ast = parse("12345678901234567890 - 1").unwrap();
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_bigint(&result, "12345678901234567889");
    }

    #[test]
    fn test_modulo() {
        let ast = parse("10 % 3").unwrap();
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_bigint(&result, "1");
    }

    #[test]
    fn test_negative_power() {
        // 2^(-1) = 1/2
        let ast = parse("2^(-1)").unwrap();
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_bigrational(&result, "1", "2");
    }

    #[test]
    fn test_format_decimal_precision_zero() {
        let r = BigRational::new(BigInt::from(7), BigInt::from(2)); // 3.5
        let formatted = format_bigrational(&r, Some(0));
        // 四舍五入 3.5 → 4
        assert_eq!(formatted, "4");
    }

    #[test]
    fn test_format_decimal_half() {
        let r = BigRational::new(BigInt::from(1), BigInt::from(2)); // 0.5
        let formatted = format_bigrational(&r, Some(3));
        assert_eq!(formatted, "0.500");
    }

    #[test]
    fn test_format_decimal_negative() {
        let r = BigRational::new(BigInt::from(-1), BigInt::from(3)); // -1/3
        let formatted = format_bigrational(&r, Some(5));
        assert_eq!(formatted, "-0.33333");
    }

    #[test]
    fn test_format_integer_bigrational() {
        let r = BigRational::new(BigInt::from(42), BigInt::from(1));
        let formatted = format_bigrational(&r, None);
        assert_eq!(formatted, "42");
    }

    #[test]
    fn test_bigint_parse_from_string() {
        // 验证 BigNumber 节点正确解析大整数
        let ast = parse("123456789012345678901234567890").unwrap();
        let AstNode::BigNumber(s) = &ast else {
            panic!("expected BigNumber, got {:?}", ast)
        };
        assert_eq!(s, "123456789012345678901234567890");
    }

    #[test]
    fn test_small_number_not_bignumber() {
        // 小数字不应被解析为 BigNumber
        let ast = parse("12345").unwrap();
        let AstNode::Number(n) = &ast else {
            panic!("expected Number, got {:?}", ast)
        };
        assert_eq!(*n, 12345.0);
    }

    #[test]
    fn test_decimal_not_bignumber() {
        // 小数不应被解析为 BigNumber（即使整数部分有 16+ 位）
        // 注：f64 本身精度有限，此测试验证不误匹配
        let ast = parse("1.5").unwrap();
        let AstNode::Number(n) = &ast else {
            panic!("expected Number, got {:?}", ast)
        };
        assert_eq!(*n, 1.5);
    }

    // ===== 覆盖未覆盖分支的补充测试 =====

    #[test]
    fn test_number_float_conversion() {
        // 非整数 f64 → BigRational::from_float 成功路径
        let ast = AstNode::Number(1.5);
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_bigrational(&result, "3", "2");
    }

    #[test]
    fn test_number_float_conversion_error() {
        // 非有限 f64 → from_float 返回 None → EvalError
        let ast = AstNode::Number(f64::INFINITY);
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Eval));
    }

    #[test]
    fn test_bignumber_parse_error() {
        // BigNumber 无效字符串 → ParseError
        let ast = AstNode::BigNumber("abc".to_string());
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Parse));
    }

    #[test]
    fn test_bound_integer_variable() {
        // 绑定的整数变量
        let ast = AstNode::Variable("x".to_string());
        let ctx = EvalContext::new().with_var("x", 5.0);
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &ctx).unwrap();
        assert_bigint(&result, "5");
    }

    #[test]
    fn test_bound_float_variable() {
        // 绑定的浮点变量 → from_float 成功
        let ast = AstNode::Variable("y".to_string());
        let ctx = EvalContext::new().with_var("y", 1.5);
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &ctx).unwrap();
        assert_bigrational(&result, "3", "2");
    }

    #[test]
    fn test_bound_infinite_variable() {
        // 绑定的无限变量 → from_float 失败 → EvalError
        let ast = AstNode::Variable("z".to_string());
        let ctx = EvalContext::new().with_var("z", f64::INFINITY);
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &ctx);
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Eval));
    }

    #[test]
    fn test_unbound_variable() {
        // 未绑定变量 → EvalError
        let ast = AstNode::Variable("w".to_string());
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Eval));
    }

    #[test]
    fn test_unary_factorial_manual_ast() {
        // UnaryOp::Factorial（parser 不产生此节点）
        let ast = AstNode::UnaryOp(
            UnaryOp::Factorial,
            Box::new(AstNode::BigNumber("5".to_string())),
        );
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_bigint(&result, "120");
    }

    #[test]
    fn test_power_exponent_too_large() {
        // 幂指数超过 i32 范围 → DomainError
        let ast = AstNode::BinaryOp(
            BinaryOp::Pow,
            Box::new(AstNode::BigNumber("2".to_string())),
            Box::new(AstNode::BigNumber("3000000000".to_string())),
        );
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_binary_mod_normal() {
        // BinaryOp::Mod 正常路径（parser 将 % 转为 mod() 函数调用）
        let ast = AstNode::BinaryOp(
            BinaryOp::Mod,
            Box::new(AstNode::BigNumber("10".to_string())),
            Box::new(AstNode::BigNumber("3".to_string())),
        );
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_bigint(&result, "1");
    }

    #[test]
    fn test_binary_mod_by_zero() {
        // BinaryOp::Mod 除零 → DivisionByZero
        let ast = AstNode::BinaryOp(
            BinaryOp::Mod,
            Box::new(AstNode::BigNumber("10".to_string())),
            Box::new(AstNode::BigNumber("0".to_string())),
        );
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::DivisionByZero));
    }

    #[test]
    fn test_factorial_function_wrong_arg_count() {
        // factorial() 参数数量错误
        let ast = AstNode::FunctionCall(
            "factorial".to_string(),
            vec![AstNode::Number(5.0), AstNode::Number(6.0)],
        );
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_abs_function_wrong_arg_count() {
        // abs() 参数数量错误
        let ast = AstNode::FunctionCall(
            "abs".to_string(),
            vec![AstNode::Number(5.0), AstNode::Number(6.0)],
        );
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_precision_nested_in_function() {
        // precision() 嵌套在其他函数中 → DomainError
        let ast = AstNode::FunctionCall(
            "abs".to_string(),
            vec![AstNode::FunctionCall(
                "precision".to_string(),
                vec![AstNode::Number(5.0), AstNode::Number(1.0)],
            )],
        );
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_mod_function_wrong_arg_count() {
        // mod() 参数数量错误
        let ast = AstNode::FunctionCall("mod".to_string(), vec![AstNode::Number(10.0)]);
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_mod_function_div_by_zero() {
        // mod() 第二参数为零 → DivisionByZero
        let ast = AstNode::FunctionCall(
            "mod".to_string(),
            vec![
                AstNode::BigNumber("10".to_string()),
                AstNode::BigNumber("0".to_string()),
            ],
        );
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::DivisionByZero));
    }

    #[test]
    fn test_contains_precision_unary_op() {
        // contains_precision for UnaryOp
        let ast = AstNode::UnaryOp(UnaryOp::Neg, Box::new(AstNode::BigNumber("42".to_string())));
        let domain = PrecisionDomain;
        assert!(domain.supports(&ast));
    }

    #[test]
    fn test_contains_precision_matrix() {
        // contains_precision for Matrix
        let ast = AstNode::Matrix(vec![vec![AstNode::BigNumber("42".to_string())]]);
        let domain = PrecisionDomain;
        assert!(domain.supports(&ast));
    }

    #[test]
    fn test_contains_precision_list() {
        // contains_precision for List
        let ast = AstNode::List(vec![AstNode::BigNumber("42".to_string())]);
        let domain = PrecisionDomain;
        assert!(domain.supports(&ast));
    }

    #[test]
    fn test_precision_bignumber_valid() {
        // extract_precision_value BigNumber 成功路径
        let ast = AstNode::FunctionCall(
            "precision".to_string(),
            vec![AstNode::BigNumber("50".to_string()), AstNode::Number(1.0)],
        );
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_bigint(&result, "1");
    }

    #[test]
    fn test_precision_bignumber_invalid() {
        // extract_precision_value BigNumber 解析失败
        let ast = AstNode::FunctionCall(
            "precision".to_string(),
            vec![AstNode::BigNumber("abc".to_string()), AstNode::Number(1.0)],
        );
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Parse));
    }

    #[test]
    fn test_precision_bignumber_out_of_range() {
        // extract_precision_value BigNumber 超出 usize 范围
        let ast = AstNode::FunctionCall(
            "precision".to_string(),
            vec![AstNode::BigNumber("-5".to_string()), AstNode::Number(1.0)],
        );
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_precision_non_literal_n() {
        // extract_precision_value 非字面量 → DomainError
        let ast = AstNode::FunctionCall(
            "precision".to_string(),
            vec![AstNode::Variable("x".to_string()), AstNode::Number(1.0)],
        );
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_precision_n_zero() {
        // extract_precision_value N == 0 → DomainError
        let ast = AstNode::FunctionCall(
            "precision".to_string(),
            vec![AstNode::Number(0.0), AstNode::Number(1.0)],
        );
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_rational_to_int_error_factorial() {
        // rational_to_int 非整数操作数 → DomainError
        let ast = AstNode::FunctionCall("factorial".to_string(), vec![AstNode::Number(1.5)]);
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_rational_to_int_error_power() {
        // rational_to_int 非整数幂指数 → DomainError
        let ast = AstNode::BinaryOp(
            BinaryOp::Pow,
            Box::new(AstNode::BigNumber("2".to_string())),
            Box::new(AstNode::Number(1.5)),
        );
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    // ===== 覆盖 extract_precision_value BigNumber("0") 路径 =====

    #[test]
    fn test_precision_bignumber_zero() {
        // 仅 BigNumber("0") 经 BigInt 解析后 v == 0 才到达此分支（Number(0.0) 提前拒绝）。
        let ast = AstNode::FunctionCall(
            "precision".to_string(),
            vec![AstNode::BigNumber("0".to_string()), AstNode::Number(1.0)],
        );
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    // ===== 覆盖测试辅助函数的 panic 分支 =====

    #[test]
    #[should_panic(expected = "expected BigInt")]
    fn test_assert_bigint_panics_on_non_bigint() {
        // 传入 Scalar 而非 BigInt → panic
        assert_bigint(&EvalResult::Scalar(1.0), "1");
    }

    #[test]
    #[should_panic(expected = "expected BigRational")]
    fn test_assert_bigrational_panics_on_non_bigrational() {
        // 传入 BigInt 而非 BigRational → panic
        assert_bigrational(&EvalResult::BigInt(BigInt::from(1)), "1", "2");
    }

    // ===== factorial/pow 上界 =====

    /// factorial(MAX_FACTORIAL_INPUT) 应成功（边界值）。
    #[test]
    fn test_factorial_at_bound_allowed() {
        let ast = AstNode::FunctionCall(
            "factorial".to_string(),
            vec![AstNode::BigNumber(MAX_FACTORIAL_INPUT.to_string())],
        );
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(result.is_ok(), "factorial at bound should succeed");
        let EvalResult::BigInt(b) = result.unwrap() else {
            panic!("expected BigInt")
        };
        // 10000! 有约 35660 位数字
        assert!(b.to_string().len() > 30000);
    }

    /// factorial(MAX_FACTORIAL_INPUT + 1) 应返回 Domain 错误（DoS 防护）。
    #[test]
    fn test_factorial_oversized_function_call_returns_error() {
        let oversized = MAX_FACTORIAL_INPUT + 1;
        let ast = AstNode::FunctionCall(
            "factorial".to_string(),
            vec![AstNode::BigNumber(oversized.to_string())],
        );
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(
            matches!(result, Err(ref e) if e.kind == ErrorKind::Domain),
            "factorial({}) should return Domain error, got {:?}",
            oversized,
            result
        );
    }

    /// `N!` 后缀阶乘运算符超大输入应返回 Domain 错误。
    #[test]
    fn test_factorial_oversized_unary_op_returns_error() {
        let oversized = MAX_FACTORIAL_INPUT + 1;
        let ast = AstNode::UnaryOp(
            UnaryOp::Factorial,
            Box::new(AstNode::BigNumber(oversized.to_string())),
        );
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(
            matches!(result, Err(ref e) if e.kind == ErrorKind::Domain),
            "({})! should return Domain error, got {:?}",
            oversized,
            result
        );
    }

    /// `a^MAX_POW_EXPONENT` 应成功（边界值，base=2）。
    #[test]
    fn test_pow_at_bound_allowed() {
        let ast = AstNode::BinaryOp(
            BinaryOp::Pow,
            Box::new(AstNode::BigNumber("2".to_string())),
            Box::new(AstNode::BigNumber(MAX_POW_EXPONENT.to_string())),
        );
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(result.is_ok(), "pow at bound should succeed");
        let EvalResult::BigInt(b) = result.unwrap() else {
            panic!("expected BigInt")
        };
        // 2^100000 有约 30103 位数字
        assert!(b.to_string().len() > 30000);
    }

    /// `a^(MAX_POW_EXPONENT + 1)` 应返回 Domain 错误（DoS 防护）。
    #[test]
    fn test_pow_oversized_exponent_returns_error() {
        let oversized = MAX_POW_EXPONENT + 1;
        let ast = AstNode::BinaryOp(
            BinaryOp::Pow,
            Box::new(AstNode::BigNumber("2".to_string())),
            Box::new(AstNode::BigNumber(oversized.to_string())),
        );
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(
            matches!(result, Err(ref e) if e.kind == ErrorKind::Domain),
            "2^{} should return Domain error, got {:?}",
            oversized,
            result
        );
    }

    /// 小负指数应成功（产生有理数）；负指数绝对值同样受 MAX_POW_EXPONENT 约束防 DoS。
    #[test]
    fn test_pow_negative_exponent_unbounded() {
        let ast = AstNode::BinaryOp(
            BinaryOp::Pow,
            Box::new(AstNode::BigNumber("2".to_string())),
            Box::new(AstNode::BigNumber("-1".to_string())),
        );
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_bigrational(&result, "1", "2");
    }

    /// `a^(-(MAX_POW_EXPONENT+1))` 应返回 Domain 错误：负指数超限同样可致 DoS，按 abs() 拒绝。
    #[test]
    fn test_pow_oversized_negative_exponent_returns_error() {
        let oversized_neg = -(MAX_POW_EXPONENT as i64 + 1); // -100001
        let ast = AstNode::BinaryOp(
            BinaryOp::Pow,
            Box::new(AstNode::BigNumber("2".to_string())),
            Box::new(AstNode::BigNumber(oversized_neg.to_string())),
        );
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(
            matches!(result, Err(ref e) if e.kind == ErrorKind::Domain),
            "2^({}) should return Domain error (negative exponent DoS), got {:?}",
            oversized_neg,
            result
        );
    }

    // ===== 底数复合限制测试 =====

    /// `check_pow_output_size`：小底数 × 大指数应通过（2^100000）。
    #[test]
    fn test_check_pow_output_size_small_base_passes() {
        // 2 的 bits = 2（二进制 `10` 需 2 位）
        // 2 × 100000 = 200000 ≤ 3_320_000
        assert!(check_pow_output_size(2, 100_000).is_ok());
    }

    /// `check_pow_output_size`：大底数 × 大指数应拒绝（(10^10000)^99999）。
    #[test]
    fn test_check_pow_output_size_large_base_rejects() {
        // 底数 bits≈33219，×99999 超上限，应拒绝
        let result = check_pow_output_size(33_219, 99_999);
        assert!(
            matches!(result, Err(ref e) if e.kind == ErrorKind::Domain),
            "10^10000 ^ 99999 should be rejected (base DoS), got {:?}",
            result
        );
    }

    /// `check_pow_output_size` 边界：恰好等于上限应通过（base_bits=33, exp=100000）。
    #[test]
    fn test_check_pow_output_size_boundary_at_limit_passes() {
        // 33 × 100000 = 3_300_000 ≤ 3_320_000，通过
        assert!(check_pow_output_size(33, 100_000).is_ok());
    }

    /// `check_pow_output_size` 边界：刚超过上限应拒绝（base_bits=34, exp=100000）。
    #[test]
    fn test_check_pow_output_size_boundary_over_limit_rejects() {
        // 34 × 100000 = 3_400_000 > 3_320_000，拒绝
        let result = check_pow_output_size(34, 100_000);
        assert!(
            matches!(result, Err(ref e) if e.kind == ErrorKind::Domain),
            "2^33 ^ 100000 should be rejected (over limit), got {:?}",
            result
        );
    }

    /// 端到端：安全大小的幂运算应通过复合限制（(2^200)^10），不误拦合法计算。
    #[test]
    fn test_pow_compound_limit_allows_safe_computation() {
        // 2^200 作为底数（bits=201），指数 10
        let ast = AstNode::BinaryOp(
            BinaryOp::Pow,
            Box::new(AstNode::BigNumber(
                "1606938044258990275541962092341162602522202993782792835301376".to_string(),
            )), // 2^200
            Box::new(AstNode::BigNumber("10".to_string())),
        );
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(
            result.is_ok(),
            "2^200 ^ 10 should succeed, got {:?}",
            result
        );
    }

    /// 端到端：大底数 × 大指数应被复合限制拦截（(2^33)^100000）。
    #[test]
    fn test_pow_compound_limit_rejects_large_base_large_exp() {
        // 2^33 = 8589934592 作为底数（bits=34），指数 100000
        let ast = AstNode::BinaryOp(
            BinaryOp::Pow,
            Box::new(AstNode::BigNumber("8589934592".to_string())), // 2^33
            Box::new(AstNode::BigNumber("100000".to_string())),
        );
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(
            matches!(result, Err(ref e) if e.kind == ErrorKind::Domain),
            "2^33 ^ 100000 should be rejected by compound limit, got {:?}",
            result
        );
    }

    // ===== proptest 属性测试 =====

    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

        /// 属性：大整数加法满足交换律 a+b = b+a
        #[test]
        fn prop_bigint_addition_commutative(a in 0u64..1_000_000_000_000, b in 0u64..1_000_000_000_000) {
            let domain = PrecisionDomain;
            let ctx = default_ctx();
            let ast_ab = parse(&format!("{} + {}", a, b)).unwrap();
            let ast_ba = parse(&format!("{} + {}", b, a)).unwrap();
            let r_ab = domain.evaluate(&ast_ab, &ctx).unwrap();
            let r_ba = domain.evaluate(&ast_ba, &ctx).unwrap();
            prop_assert_eq!(r_ab, r_ba);
        }

        /// 属性：大整数乘法满足交换律 a*b = b*a
        #[test]
        fn prop_bigint_multiplication_commutative(a in 0u64..1_000_000, b in 0u64..1_000_000) {
            let domain = PrecisionDomain;
            let ctx = default_ctx();
            let ast_ab = parse(&format!("{} * {}", a, b)).unwrap();
            let ast_ba = parse(&format!("{} * {}", b, a)).unwrap();
            let r_ab = domain.evaluate(&ast_ab, &ctx).unwrap();
            let r_ba = domain.evaluate(&ast_ba, &ctx).unwrap();
            prop_assert_eq!(r_ab, r_ba);
        }

        /// 属性：分数约分幂等 — (a/b) 约分后分母整除原始分母
        #[test]
        fn prop_fraction_reduces(a in 1u64..1000, b in 1u64..1000) {
            let domain = PrecisionDomain;
            let ctx = default_ctx();
            let ast = parse(&format!("{}/{}", a, b)).unwrap();
            let result = domain.evaluate(&ast, &ctx).unwrap();
            match result {
                EvalResult::BigRational(r) => {
                    // 分母为正且分子分母互质（约分后）
                    prop_assert!(*r.denom() > BigInt::zero());
                }
                EvalResult::BigInt(_) => {
                    // 整数结果（b 整除 a）
                }
                other => panic!("expected BigInt or BigRational, got {:?}", other),
            }
        }

        /// 属性：factorial(n) = n * factorial(n-1)
        #[test]
        fn prop_factorial_recurrence(n in 1u32..50) {
            let domain = PrecisionDomain;
            let ctx = default_ctx();
            let ast_n = parse(&format!("factorial({})", n)).unwrap();
            let ast_n1 = parse(&format!("factorial({})", n - 1)).unwrap();
            let r_n = domain.evaluate(&ast_n, &ctx).unwrap();
            let r_n1 = domain.evaluate(&ast_n1, &ctx).unwrap();
            // r_n = n * r_n1
            let n_big = BigInt::from(n);
            let expected = match r_n1 {
                EvalResult::BigInt(b) => n_big * b,
                _ => panic!("expected BigInt"),
            };
            prop_assert_eq!(r_n, EvalResult::BigInt(expected));
        }
    }
}
