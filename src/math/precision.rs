// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! 精度核心函数：BigRational 求值 + 格式化。
//!
//! 从 `domains/precision.rs` 提取的纯数学逻辑，
//! 不含 AST 转换和域路由（留在域层）。

use crate::core::{CalcError, EvalResult, MAX_FACTORIAL_INPUT};
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, Signed, Zero};

/// 计算大整数阶乘。
///
/// 安全约束：拒绝超过 `MAX_FACTORIAL_INPUT` 的输入，防止循环 DoS。
/// 负数输入返回 DomainError（阶乘定义域为非负整数）。
pub fn factorial(n: &BigInt) -> Result<BigInt, CalcError> {
    if n < &BigInt::zero() {
        return Err(CalcError::domain(format!(
            "factorial requires non-negative integer, got {}",
            n
        )));
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

/// 将 f64 转换为 BigRational。
///
/// 整数且范围在 i64 内：精确转换为整数 BigRational；
/// 否则：尝试 `BigRational::from_float`，失败时返回错误。
pub fn f64_to_rational(n: f64) -> Result<BigRational, CalcError> {
    if n.fract() == 0.0 && n.abs() < 9e15 {
        Ok(BigRational::from_integer(BigInt::from(n as i64)))
    } else {
        BigRational::from_float(n).ok_or_else(|| {
            CalcError::eval(format!("cannot convert {} to BigRational", n))
        })
    }
}

/// 从 BigRational 提取 BigInt（要求为整数）。
///
/// 返回 BigInt 形式的操作数（可为负数，由调用方负责范围检查）。
pub fn rational_to_int(r: &BigRational, ctx: &str) -> Result<BigInt, CalcError> {
    if !r.is_integer() {
        return Err(CalcError::domain(format!(
            "{} requires integer operand, got {}",
            ctx, r
        )));
    }
    Ok(r.numer().clone())
}

/// 将 BigRational 转换为 EvalResult。
///
/// 分母为 1 → BigInt；否则 → BigRational。
pub fn rational_to_result(value: BigRational) -> EvalResult {
    if value.is_integer() {
        EvalResult::BigInt(value.numer().clone())
    } else {
        EvalResult::BigRational(value)
    }
}

// ============================ 单元测试 ============================

#[cfg(test)]
mod tests {
    use super::*;

    // --- factorial ---

    #[test]
    fn test_factorial_zero() {
        assert_eq!(factorial(&BigInt::zero()).unwrap(), BigInt::one());
    }

    #[test]
    fn test_factorial_five() {
        assert_eq!(
            factorial(&BigInt::from(5)).unwrap(),
            BigInt::from(120)
        );
    }

    #[test]
    fn test_factorial_negative() {
        assert!(factorial(&BigInt::from(-1)).is_err());
    }

    #[test]
    fn test_factorial_exceeds_max() {
        let oversized = BigInt::from(MAX_FACTORIAL_INPUT + 1);
        assert!(factorial(&oversized).is_err());
    }

    #[test]
    fn test_factorial_at_bound() {
        let at_bound = BigInt::from(MAX_FACTORIAL_INPUT);
        let result = factorial(&at_bound);
        assert!(result.is_ok());
    }

    // --- format_bigrational ---

    #[test]
    fn test_format_fraction() {
        let r = BigRational::new(BigInt::from(1), BigInt::from(3));
        assert_eq!(format_bigrational(&r, None), "1/3");
    }

    #[test]
    fn test_format_integer_fraction() {
        let r = BigRational::new(BigInt::from(4), BigInt::from(2));
        assert_eq!(format_bigrational(&r, None), "2");
    }

    #[test]
    fn test_format_decimal_precision_zero() {
        let r = BigRational::new(BigInt::from(7), BigInt::from(2));
        assert_eq!(format_bigrational(&r, Some(0)), "3");
    }

    #[test]
    fn test_format_decimal_half() {
        let r = BigRational::new(BigInt::from(1), BigInt::from(2));
        assert_eq!(format_bigrational(&r, Some(3)), "0.500");
    }

    #[test]
    fn test_format_decimal_negative() {
        let r = BigRational::new(BigInt::from(-1), BigInt::from(3));
        assert_eq!(format_bigrational(&r, Some(5)), "-0.33333");
    }

    #[test]
    fn test_format_decimal_50_digits() {
        let r = BigRational::new(BigInt::from(1), BigInt::from(3));
        let formatted = format_bigrational(&r, Some(50));
        assert!(formatted.starts_with("0.3"));
        assert_eq!(formatted.len(), 52); // "0." + 50 digits
        assert!(formatted.chars().skip(2).all(|c| c == '3'));
    }

    // --- f64_to_rational ---

    #[test]
    fn test_f64_integer() {
        let r = f64_to_rational(5.0).unwrap();
        assert_eq!(r, BigRational::from_integer(BigInt::from(5)));
    }

    #[test]
    fn test_f64_fraction() {
        let r = f64_to_rational(1.5).unwrap();
        assert_eq!(r, BigRational::new(BigInt::from(3), BigInt::from(2)));
    }

    #[test]
    fn test_f64_infinity_error() {
        assert!(f64_to_rational(f64::INFINITY).is_err());
    }

    // --- rational_to_int ---

    #[test]
    fn test_rational_to_int_integer() {
        let r = BigRational::from_integer(BigInt::from(42));
        assert_eq!(rational_to_int(&r, "test").unwrap(), BigInt::from(42));
    }

    #[test]
    fn test_rational_to_int_non_integer_error() {
        let r = BigRational::new(BigInt::from(1), BigInt::from(3));
        assert!(rational_to_int(&r, "test").is_err());
    }

    // --- rational_to_result ---

    #[test]
    fn test_rational_to_result_integer() {
        let r = BigRational::from_integer(BigInt::from(42));
        let result = rational_to_result(r);
        assert!(matches!(result, EvalResult::BigInt(_)));
    }

    #[test]
    fn test_rational_to_result_fraction() {
        let r = BigRational::new(BigInt::from(1), BigInt::from(3));
        let result = rational_to_result(r);
        assert!(matches!(result, EvalResult::BigRational(_)));
    }
}
