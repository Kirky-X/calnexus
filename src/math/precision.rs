// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! 精度核心函数：BigRational 求值 + 格式化。

use crate::core::{CalcError, EvalResult, MAX_FACTORIAL_INPUT};
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, Signed, Zero};

/// 计算大整数阶乘；超过 MAX_FACTORIAL_INPUT 或负数返回错误（防 DoS）。
pub fn factorial(n: &BigInt) -> Result<BigInt, CalcError> {
    if n < &BigInt::zero() {
        return Err(CalcError::domain(format!(
            "factorial requires non-negative integer, got {}",
            n
        ))
        .with_i18n(
            "msg.core.factorial_negative",
            vec![("value".to_string(), n.to_string())],
        ));
    }
    if n > &BigInt::from(MAX_FACTORIAL_INPUT) {
        return Err(CalcError::domain(format!(
            "factorial input must not exceed {} (got {})",
            MAX_FACTORIAL_INPUT, n
        ))
        .with_i18n(
            "msg.precision.factorial_exceeds_max",
            vec![
                ("max".to_string(), MAX_FACTORIAL_INPUT.to_string()),
                ("value".to_string(), n.to_string()),
            ],
        ));
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

/// 格式化 BigRational：None 输出分数（整数约简），Some(N) 输出 N 位小数。
pub fn format_bigrational(value: &BigRational, precision: Option<usize>) -> String {
    if let Some(n) = precision {
        format_decimal(value, n)
    } else if value.is_integer() {
        value.numer().to_string()
    } else {
        format!("{}/{}", value.numer(), value.denom())
    }
}

/// 格式化为指定精度的小数，四舍五入。
fn format_decimal(value: &BigRational, precision: usize) -> String {
    let ten = BigInt::from(10);
    let neg = value.is_negative();
    let abs = value.abs();
    let numer = abs.numer();
    let denom = abs.denom();

    let mut scale = BigInt::one();
    for _ in 0..precision {
        scale *= &ten;
    }
    // 加 0.5 后向下取整，实现四舍五入
    let rounded = (&BigInt::from(2) * numer * &scale + denom) / (&BigInt::from(2) * denom);

    if precision == 0 {
        let sign = if neg { "-" } else { "" };
        format!("{}{}", sign, rounded)
    } else {
        let int_part = &rounded / &scale;
        let frac_part = &rounded % &scale;
        let int_str = int_part.to_string();
        let frac_str = format!("{:0>width$}", frac_part, width = precision);
        let sign = if neg { "-" } else { "" };
        format!("{}{}.{}", sign, int_str, frac_str)
    }
}

/// f64 转 BigRational：i64 内整数精确转换，否则用 from_float，失败报错。
pub fn f64_to_rational(n: f64) -> Result<BigRational, CalcError> {
    if n.fract() == 0.0 && n.abs() <= i64::MAX as f64 {
        Ok(BigRational::from_integer(BigInt::from(n as i64)))
    } else {
        BigRational::from_float(n).ok_or_else(|| {
            CalcError::eval(format!("cannot convert {} to BigRational", n)).with_i18n(
                "msg.precision.cannot_convert_rational",
                vec![("value".to_string(), n.to_string())],
            )
        })
    }
}

/// 从 BigRational 提取 BigInt；要求整数，可为负，范围检查由调用方负责。
pub fn rational_to_int(r: &BigRational, ctx: &str) -> Result<BigInt, CalcError> {
    if !r.is_integer() {
        return Err(
            CalcError::domain(format!("{} requires integer operand, got {}", ctx, r)).with_i18n(
                "msg.precision.requires_integer_operand",
                vec![
                    ("ctx".to_string(), ctx.to_string()),
                    ("value".to_string(), r.to_string()),
                ],
            ),
        );
    }
    Ok(r.numer().clone())
}

/// BigRational 转 EvalResult：整数→BigInt，否则→BigRational。
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
        assert_eq!(factorial(&BigInt::from(5)).unwrap(), BigInt::from(120));
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
        // 四舍五入 3.5 → 4
        assert_eq!(format_bigrational(&r, Some(0)), "4");
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
