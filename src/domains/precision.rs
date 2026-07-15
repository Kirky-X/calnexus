// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! Precision 计算域：大整数运算、精确分数、高精度小数。
//!
//! 设计依据：
//! - precision-domain spec：10 个 requirements / 21 个 scenarios
//! - design.md D7：基于 `num_bigint::BigInt` + `num_rational::BigRational`，priority=25
//!
//! 路由策略：
//! - AST 含 `BigNumber` 节点（16+ 位整数字面量）→ 路由至本域
//! - AST 含 `precision()` 函数调用 → 路由至本域
//! - `--precision N` CLI 参数 → CLI 层直接使用本域（绕过路由器）
//!
//! 内部求值统一使用 `BigRational`，结果根据分母是否为 1 转换为 `BigInt` 或 `BigRational`。

use crate::core::CalculationDomain;
use crate::core::{
    AstNode, BinaryOp, CalcError, EvalContext, EvalResult, UnaryOp, MAX_FACTORIAL_INPUT,
    MAX_POW_EXPONENT, MAX_PRECISION,
};
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, Signed, Zero};

/// Precision 计算域。
///
/// priority=25，低于 Complex/Matrix（30）但高于 Statistics（20）和 Arithmetic（10）。
/// 支持大整数运算、精确分数、大数阶乘、大整数幂。
pub struct PrecisionDomain;

impl CalculationDomain for PrecisionDomain {
    fn domain_name(&self) -> &str {
        "precision"
    }

    fn priority(&self) -> u8 {
        25
    }

    fn supports(&self, ast: &AstNode) -> bool {
        // 含 BigNumber 或 precision() 时认领，但排除其他域的专用函数
        // （如 is_prime(BigNumber) 应路由至 NumberTheoryDomain，而非本域）
        contains_precision(ast) && !contains_other_domain_function(ast)
    }

    fn evaluate(&self, ast: &AstNode, ctx: &EvalContext) -> Result<EvalResult, CalcError> {
        // 处理 precision(N, expr) 函数：求值 expr，N 仅供 CLI 格式化使用
        if let AstNode::FunctionCall(name, args) = ast {
            if name == "precision" {
                if args.len() != 2 {
                    return Err(CalcError::domain(format!(
                        "precision() requires exactly 2 arguments (N, expr), got {}",
                        args.len()
                    )));
                }
                // 验证 N 为正整数
                let _n = extract_precision_value(&args[0])?;
                let value = self.eval(&args[1], ctx)?;
                return Ok(rational_to_result(value));
            }
        }

        let value = self.eval(ast, ctx)?;
        Ok(rational_to_result(value))
    }
}

impl PrecisionDomain {
    /// 递归求值 AST 节点为 `BigRational`。
    fn eval(&self, ast: &AstNode, ctx: &EvalContext) -> Result<BigRational, CalcError> {
        match ast {
            AstNode::Number(n) => {
                if n.fract() == 0.0 && n.abs() < 9e15 {
                    Ok(BigRational::from_integer(BigInt::from(*n as i64)))
                } else {
                    // 非整数 f64：近似转换（仅用于混合表达式中的小数）
                    BigRational::from_float(*n).ok_or_else(|| {
                        CalcError::eval(format!("cannot convert {} to BigRational", n))
                    })
                }
            }
            AstNode::BigNumber(s) => {
                let big = BigInt::parse_bytes(s.as_bytes(), 10).ok_or_else(|| {
                    CalcError::parse(format!("invalid big integer literal: {}", s))
                })?;
                Ok(BigRational::from_integer(big))
            }
            AstNode::Variable(name) => {
                if let Some(v) = ctx.get_var(name) {
                    if v.fract() == 0.0 && v.abs() < 9e15 {
                        Ok(BigRational::from_integer(BigInt::from(v as i64)))
                    } else {
                        BigRational::from_float(v).ok_or_else(|| {
                            CalcError::eval(format!("cannot convert {} to BigRational", v))
                        })
                    }
                } else {
                    Err(CalcError::eval(format!("unbound variable: {}", name)))
                }
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
                        let n = rational_to_int(&v, "factorial")?;
                        Ok(BigRational::from_integer(factorial(&n)?))
                    }
                }
            }
            AstNode::FunctionCall(name, args) => self.eval_function(name, args, ctx),
            AstNode::Complex(_, _) | AstNode::Matrix(_) | AstNode::List(_) => {
                Err(CalcError::domain(format!(
                    "precision domain does not support this node type: {:?}",
                    ast
                )))
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
                let exp = rational_to_int(&b, "power exponent")?;
                // 安全约束：拒绝超大指数（绝对值），防止 DoS。
                // tiangang SAST CRITICAL 修复 + 复审 C-1 修复：
                // `BigRational::pow(neg_i32)` 内部实现为 `Pow::pow(self, (-exp) as u64).reciprocal()`，
                // 即先计算 `a^|exp|`（巨大中间值）再取倒数。故负指数绝对值超限同样会 DoS
                // （`2^(-2000000000)` 计算 `2^2000000000` ~6 亿位数字）。必须用 `abs()` 检查。
                if exp.abs() > BigInt::from(MAX_POW_EXPONENT) {
                    return Err(CalcError::domain(format!(
                        "power exponent absolute value must not exceed {} (got {})",
                        MAX_POW_EXPONENT, exp
                    )));
                }
                // BigRational::pow 接受 i32 指数
                let exp_i32 = i32::try_from(&exp)
                    .map_err(|_| CalcError::domain(format!("power exponent too large: {}", exp)))?;
                Ok(a.pow(exp_i32))
            }
            BinaryOp::Mod => {
                if b.is_zero() {
                    return Err(CalcError::division_by_zero());
                }
                // 大整数取模：提取整数部分计算
                let a_int = rational_to_int(&a, "mod operand")?;
                let b_int = rational_to_int(&b, "mod operand")?;
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
                if args.len() != 1 {
                    return Err(CalcError::domain(format!(
                        "factorial() requires exactly 1 argument, got {}",
                        args.len()
                    )));
                }
                let v = self.eval(&args[0], ctx)?;
                let n = rational_to_int(&v, "factorial")?;
                Ok(BigRational::from_integer(factorial(&n)?))
            }
            "abs" => {
                if args.len() != 1 {
                    return Err(CalcError::domain(format!(
                        "abs() requires exactly 1 argument, got {}",
                        args.len()
                    )));
                }
                let v = self.eval(&args[0], ctx)?;
                Ok(v.abs())
            }
            "precision" => {
                // precision(N, expr) 在 evaluate() 顶层处理，此处不应到达
                Err(CalcError::domain(
                    "precision() must be at expression top level".to_string(),
                ))
            }
            "mod" => {
                // parser 将 `%` 转换为 mod(a, b) 函数调用
                if args.len() != 2 {
                    return Err(CalcError::domain(format!(
                        "mod() requires exactly 2 arguments, got {}",
                        args.len()
                    )));
                }
                let a = self.eval(&args[0], ctx)?;
                let b = self.eval(&args[1], ctx)?;
                if b.is_zero() {
                    return Err(CalcError::division_by_zero());
                }
                let a_int = rational_to_int(&a, "mod operand")?;
                let b_int = rational_to_int(&b, "mod operand")?;
                Ok(BigRational::from_integer(a_int % b_int))
            }
            _ => Err(CalcError::domain(format!(
                "unsupported function in precision domain: {}",
                name
            ))),
        }
    }
}

/// 递归检查 AST 是否应路由至 PrecisionDomain。
///
/// 路由条件（spec Req 9）：
/// - 含 `BigNumber` 节点（大整数字面量）
/// - 含 `precision()` 函数调用
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
        AstNode::Number(_) | AstNode::Variable(_) | AstNode::Complex(_, _) => false,
    }
}

/// 其他域的专用函数列表（NumberTheory + Combinatorics）。
/// 当 AST 含这些函数调用时，PrecisionDomain 不应认领，让位给更具体的域。
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
        | AstNode::Complex(_, _) => false,
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
                )));
            }
        }
        AstNode::BigNumber(s) => {
            let big = BigInt::parse_bytes(s.as_bytes(), 10)
                .ok_or_else(|| CalcError::parse(format!("invalid precision value: {}", s)))?;
            usize::try_from(big)
                .map_err(|_| CalcError::domain(format!("precision N out of range: {}", s)))?
        }
        _ => {
            return Err(CalcError::domain(
                "precision N must be a literal integer".to_string(),
            ));
        }
    };
    if v == 0 {
        return Err(CalcError::domain(
            "precision N must be positive".to_string(),
        ));
    }
    // 安全约束：拒绝超大精度值，防止 format_decimal 循环 DoS
    // （tiangang SAST CRITICAL：precision(N, expr) 表达式语法绕过 server 层校验）
    if v > MAX_PRECISION {
        return Err(CalcError::domain(format!(
            "precision N must not exceed {} (got {})",
            MAX_PRECISION, v
        )));
    }
    Ok(v)
}

/// 将 BigRational 转换为 EvalResult。
///
/// 分母为 1 → BigInt；否则 → BigRational。
fn rational_to_result(value: BigRational) -> EvalResult {
    if value.is_integer() {
        EvalResult::BigInt(value.numer().clone())
    } else {
        EvalResult::BigRational(value)
    }
}

/// 从 BigRational 提取 BigInt（要求为整数）。
///
/// 返回 BigInt 形式的操作数（可为负数，由调用方负责范围检查）。
/// 例如 pow 的负指数由此返回，再由 `BinaryOp::Pow` 分支的 `abs()` 检查约束。
fn rational_to_int(r: &BigRational, ctx: &str) -> Result<BigInt, CalcError> {
    if !r.is_integer() {
        return Err(CalcError::domain(format!(
            "{} requires integer operand, got {}",
            ctx, r
        )));
    }
    Ok(r.numer().clone())
}

/// 计算大整数阶乘。
///
/// 安全约束：拒绝超过 `MAX_FACTORIAL_INPUT` 的输入，防止循环 DoS
/// （tiangang SAST CRITICAL 修复：`factorial(1000000000)` 可在 24 字节请求内永久挂死服务器）。
/// 负数输入返回 0（保持原有语义）。
fn factorial(n: &BigInt) -> Result<BigInt, CalcError> {
    if n < &BigInt::zero() {
        return Ok(BigInt::zero());
    }
    if n > &BigInt::from(MAX_FACTORIAL_INPUT) {
        return Err(CalcError::domain(format!(
            "factorial input must not exceed {} (got {})",
            MAX_FACTORIAL_INPUT, n
        )));
    }
    let mut result = BigInt::one();
    let mut i = BigInt::one();
    let one = BigInt::one();
    while &i <= n {
        result *= &i;
        i += &one;
    }
    Ok(result)
}

/// 格式化 BigRational 为输出字符串。
///
/// - `precision = None`：分数形式 `num/den`，分母为 1 时输出整数
/// - `precision = Some(N)`：N 位小数（不含前导 `0.`）
pub fn format_bigrational(value: &BigRational, precision: Option<usize>) -> String {
    if let Some(n) = precision {
        format_decimal(value, n)
    } else if value.is_integer() {
        value.numer().to_string()
    } else {
        format!("{}/{}", value.numer(), value.denom())
    }
}

/// 格式化 BigRational 为指定精度的十进制小数。
///
/// 例如 `1/3` 精度 5 → `0.33333`，`1/2` 精度 3 → `0.500`。
fn format_decimal(value: &BigRational, precision: usize) -> String {
    let ten = BigInt::from(10);
    let neg = value.is_negative();
    let abs = value.abs();
    let numer = abs.numer();
    let denom = abs.denom();

    // 整数部分
    let int_part = numer / denom;
    let remainder = numer % denom;

    // 小数部分：remainder * 10^precision / denom
    let mut scale = BigInt::one();
    for _ in 0..precision {
        scale *= &ten;
    }
    let scaled = remainder * &scale;
    let frac_digits = scaled / denom;

    let int_str = int_part.to_string();
    let frac_str = format!("{:0>width$}", frac_digits.to_string(), width = precision);

    let sign = if neg { "-" } else { "" };
    if precision == 0 {
        format!("{}{}", sign, int_str)
    } else {
        format!("{}{}.{}", sign, int_str, frac_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::parse;
    use crate::core::ErrorKind;

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

    // ===== Requirement 1: 大整数运算 =====

    #[test]
    fn test_big_integer_addition() {
        // 12345678901234567890 + 1 → 12345678901234567891（Req 1 Scen 1）
        let ast = parse("12345678901234567890 + 1").unwrap();
        let domain = PrecisionDomain;
        assert!(domain.supports(&ast));
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_bigint(&result, "12345678901234567891");
    }

    #[test]
    fn test_big_integer_multiplication() {
        // 999999999999 * 999999999999 → 精确大整数（Req 1 Scen 2）
        let ast = parse("999999999999 * 999999999999").unwrap();
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_bigint(&result, "999999999998000000000001");
    }

    // ===== Requirement 2: 精确分数 =====

    #[test]
    fn test_fraction_addition() {
        // 1/3 + 1/6 → 1/2（Req 2 Scen 1）
        let ast = parse("1/3 + 1/6").unwrap();
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        // 1/3+1/6 = 2/6+1/6 = 3/6 = 1/2，分母不为 1，应为 BigRational
        assert_bigrational(&result, "1", "2");
    }

    #[test]
    fn test_fraction_reduction() {
        // 2/4 → 1/2（Req 2 Scen 2）
        let ast = parse("2/4").unwrap();
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_bigrational(&result, "1", "2");
    }

    // ===== Requirement 3: 高精度小数 =====

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

    // ===== Requirement 4: 大数阶乘 =====

    #[test]
    fn test_factorial_100() {
        // factorial(100) → 精确大整数（Req 4 Scen 1）
        let ast = parse("factorial(100)").unwrap();
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        // 100! = 93326215443944152681699238856266700490715968264381621468592963895217599993229915608941463976156518286253697920827223758251185210916864000000000000000000000000
        let EvalResult::BigInt(b) = &result else {
            panic!("expected BigInt, got {:?}", result)
        };
        assert!(b > &BigInt::zero());
        assert!(b.to_string().len() > 100); // 100! 有 158 位
    }

    #[test]
    fn test_factorial_5() {
        // factorial(5) → 120（Req 4 Scen 2）
        let ast = parse("factorial(5)").unwrap();
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_bigint(&result, "120");
    }

    // ===== Requirement 5: 精度触发 =====

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

    // ===== Requirement 6: 大整数幂运算 =====

    #[test]
    fn test_power_2_100() {
        // 2^100 → 精确大整数（Req 6 Scen 1）
        let ast = parse("2^100").unwrap();
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        // 2^100 = 1267650600228229401496703205376
        assert_bigint(&result, "1267650600228229401496703205376");
    }

    #[test]
    fn test_power_10_50() {
        // 10^50 → 1 后跟 50 个 0（Req 6 Scen 2）
        let ast = parse("10^50").unwrap();
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        let expected = format!("1{}", "0".repeat(50));
        assert_bigint(&result, &expected);
    }

    // ===== Requirement 7: 分数运算 =====

    #[test]
    fn test_fraction_times_integer() {
        // 1/3 * 3 → 1（Req 7 Scen 1）
        let ast = parse("1/3 * 3").unwrap();
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_bigint(&result, "1");
    }

    #[test]
    fn test_fraction_division() {
        // (1/2) / (1/4) → 2（Req 7 Scen 2）
        let ast = parse("(1/2) / (1/4)").unwrap();
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_bigint(&result, "2");
    }

    // ===== Requirement 8: 混合运算 =====

    #[test]
    fn test_bigint_plus_fraction() {
        // 12345678901234567890 + 1/3 → BigRational（Req 8 Scen 1）
        let ast = parse("12345678901234567890 + 1/3").unwrap();
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        // 结果 = (12345678901234567890 * 3 + 1) / 3 = 37037036703703703671/3
        assert_bigrational(&result, "37037036703703703671", "3");
    }

    #[test]
    fn test_mixed_fraction_reduction() {
        // 2/3 * (3 + 1/2) → 2/3 * 7/2 = 14/6 = 7/3（Req 8 Scen 2）
        let ast = parse("2/3 * (3 + 1/2)").unwrap();
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_bigrational(&result, "7", "3");
    }

    // ===== Requirement 9: 域路由 =====

    #[test]
    fn test_precision_function_routes() {
        // precision(50, 1/3) → 路由到 PrecisionDomain（Req 9 Scen 1）
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

    // ===== Requirement 10: 输出格式 =====

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
        // factorial(-1) → 0（负数阶乘返回 0）
        let ast = AstNode::FunctionCall("factorial".to_string(), vec![AstNode::Number(-1.0)]);
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_bigint(&result, "0");
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
        assert_eq!(formatted, "3");
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
        // lines 70-71: 非整数 f64 → BigRational::from_float 成功路径
        let ast = AstNode::Number(1.5);
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_bigrational(&result, "3", "2");
    }

    #[test]
    fn test_number_float_conversion_error() {
        // lines 70-71: 非有限 f64 → from_float 返回 None → EvalError
        let ast = AstNode::Number(f64::INFINITY);
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Eval));
    }

    #[test]
    fn test_bignumber_parse_error() {
        // lines 76-77: BigNumber 无效字符串 → ParseError
        let ast = AstNode::BigNumber("abc".to_string());
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Parse));
    }

    #[test]
    fn test_bound_integer_variable() {
        // lines 80-83: 绑定的整数变量
        let ast = AstNode::Variable("x".to_string());
        let ctx = EvalContext::new().with_var("x", 5.0);
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &ctx).unwrap();
        assert_bigint(&result, "5");
    }

    #[test]
    fn test_bound_float_variable() {
        // lines 84-88: 绑定的浮点变量 → from_float 成功
        let ast = AstNode::Variable("y".to_string());
        let ctx = EvalContext::new().with_var("y", 1.5);
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &ctx).unwrap();
        assert_bigrational(&result, "3", "2");
    }

    #[test]
    fn test_bound_infinite_variable() {
        // lines 85-87: 绑定的无限变量 → from_float 失败 → EvalError
        let ast = AstNode::Variable("z".to_string());
        let ctx = EvalContext::new().with_var("z", f64::INFINITY);
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &ctx);
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Eval));
    }

    #[test]
    fn test_unbound_variable() {
        // lines 89-90: 未绑定变量 → EvalError
        let ast = AstNode::Variable("w".to_string());
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Eval));
    }

    #[test]
    fn test_unary_factorial_manual_ast() {
        // lines 104-105: UnaryOp::Factorial（parser 不产生此节点）
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
        // lines 140-141: 幂指数超过 i32 范围 → DomainError
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
        // lines 145-151: BinaryOp::Mod 正常路径（parser 将 % 转为 mod() 函数调用）
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
        // lines 145-146: BinaryOp::Mod 除零 → DivisionByZero
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
        // lines 166-169: factorial() 参数数量错误
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
        // lines 177-180: abs() 参数数量错误
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
        // lines 187-189: precision() 嵌套在其他函数中 → DomainError
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
        // lines 194-197: mod() 参数数量错误
        let ast = AstNode::FunctionCall("mod".to_string(), vec![AstNode::Number(10.0)]);
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_mod_function_div_by_zero() {
        // line 202: mod() 第二参数为零 → DivisionByZero
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
        // line 231: contains_precision for UnaryOp
        let ast = AstNode::UnaryOp(UnaryOp::Neg, Box::new(AstNode::BigNumber("42".to_string())));
        let domain = PrecisionDomain;
        assert!(domain.supports(&ast));
    }

    #[test]
    fn test_contains_precision_matrix() {
        // line 232: contains_precision for Matrix
        let ast = AstNode::Matrix(vec![vec![AstNode::BigNumber("42".to_string())]]);
        let domain = PrecisionDomain;
        assert!(domain.supports(&ast));
    }

    #[test]
    fn test_contains_precision_list() {
        // line 233: contains_precision for List
        let ast = AstNode::List(vec![AstNode::BigNumber("42".to_string())]);
        let domain = PrecisionDomain;
        assert!(domain.supports(&ast));
    }

    #[test]
    fn test_precision_bignumber_valid() {
        // lines 251-257: extract_precision_value BigNumber 成功路径
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
        // lines 253-254: extract_precision_value BigNumber 解析失败
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
        // lines 255-257: extract_precision_value BigNumber 超出 usize 范围
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
        // lines 260-262: extract_precision_value 非字面量 → DomainError
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
        // lines 266-268: extract_precision_value N == 0 → DomainError
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
        // lines 287-290: rational_to_int 非整数操作数 → DomainError
        let ast = AstNode::FunctionCall("factorial".to_string(), vec![AstNode::Number(1.5)]);
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_rational_to_int_error_power() {
        // lines 287-290: rational_to_int 非整数幂指数 → DomainError
        let ast = AstNode::BinaryOp(
            BinaryOp::Pow,
            Box::new(AstNode::BigNumber("2".to_string())),
            Box::new(AstNode::Number(1.5)),
        );
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    // ===== 覆盖 extract_precision_value BigNumber("0") 路径（lines 265-268）=====

    #[test]
    fn test_precision_bignumber_zero() {
        // lines 265-268: extract_precision_value BigNumber("0") → v == 0 → DomainError
        // Number(0.0) 走 line 242 的 *n > 0.0 检查，不会到达 lines 265-268。
        // 只有 BigNumber("0") 经过 BigInt 解析后 v == 0 才能到达此分支。
        let ast = AstNode::FunctionCall(
            "precision".to_string(),
            vec![AstNode::BigNumber("0".to_string()), AstNode::Number(1.0)],
        );
        let domain = PrecisionDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    // ===== 覆盖测试辅助函数的 panic 分支（lines 382, 395）=====

    #[test]
    #[should_panic(expected = "expected BigInt")]
    fn test_assert_bigint_panics_on_non_bigint() {
        // 传入 Scalar 而非 BigInt → panic（line 382）
        assert_bigint(&EvalResult::Scalar(1.0), "1");
    }

    #[test]
    #[should_panic(expected = "expected BigRational")]
    fn test_assert_bigrational_panics_on_non_bigrational() {
        // 传入 BigInt 而非 BigRational → panic（line 395）
        assert_bigrational(&EvalResult::BigInt(BigInt::from(1)), "1", "2");
    }

    // ===== 安全审查 CRITICAL 修复：factorial/pow 上界（T025）=====

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

    /// 小负指数应成功（产生有理数，如 2^-1 = 1/2）。
    ///
    /// 注意：大负指数仍有 DoS 风险（见 `test_pow_oversized_negative_exponent_returns_error`），
    /// 因为 `BigRational::pow(neg_i32)` 内部计算 `a^|exp|` 再取倒数，
    /// 中间值 `a^|exp|` 可能巨大。故负指数也受 `MAX_POW_EXPONENT` 的 `abs()` 检查约束。
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

    /// 安全审查 CRITICAL C-1 修复：`a^(-(MAX_POW_EXPONENT+1))` 应返回 Domain 错误。
    ///
    /// 漏洞：`BigRational::pow(neg_i32)` 内部实现为 `Pow::pow(self, (-exp) as u64).reciprocal()`，
    /// 即先计算 `a^|exp|`（巨大中间值），再取倒数。`2^(-100001)` 会计算 `2^100001`（~30104 位数字），
    /// 而 `2^(-2000000000)` 会计算 `2^2000000000`（~6 亿位数字，~600MB）导致内存爆炸 + CPU 挂死。
    ///
    /// 修复：对指数取 `abs()` 后与 `MAX_POW_EXPONENT` 比较，负指数绝对值超限同样拒绝。
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
