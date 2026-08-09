// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! 算术核心函数：四则运算、幂、阶乘、取模、绝对值。
//!
//! 从 `domains/arithmetic.rs` 提取的纯数学逻辑，
//! 供 `domains/`（AST 求值路径）和 `api/`（直接 API 路径）共用。

use crate::core::{CalcError, MAX_FACTORIAL_INPUT};

/// 加法：`a + b`。溢出（`!is_finite()`）返回 `NaNOrInf`。
pub fn add(a: f64, b: f64) -> Result<f64, CalcError> {
    let r = a + b;
    if !r.is_finite() {
        return Err(CalcError::nan_or_inf());
    }
    Ok(r)
}

/// 减法：`a - b`。溢出返回 `NaNOrInf`。
pub fn sub(a: f64, b: f64) -> Result<f64, CalcError> {
    let r = a - b;
    if !r.is_finite() {
        return Err(CalcError::nan_or_inf());
    }
    Ok(r)
}

/// 乘法：`a * b`。溢出返回 `NaNOrInf`。
pub fn mul(a: f64, b: f64) -> Result<f64, CalcError> {
    let r = a * b;
    if !r.is_finite() {
        return Err(CalcError::nan_or_inf());
    }
    Ok(r)
}

/// 除法：`a / b`。
///
/// - `b == 0.0 && a == 0.0` → `NaNOrInf`（0/0 = NaN）
/// - `b == 0.0 && a != 0.0` → `DivisionByZero`
/// - 结果溢出 → `NaNOrInf`
pub fn div(a: f64, b: f64) -> Result<f64, CalcError> {
    if b == 0.0 {
        if a == 0.0 {
            return Err(CalcError::nan_or_inf());
        }
        return Err(CalcError::division_by_zero());
    }
    let r = a / b;
    if !r.is_finite() {
        return Err(CalcError::nan_or_inf());
    }
    Ok(r)
}

/// 幂运算：`a ^ b`。
///
/// - `0.0^0.0 = 1.0`（组合数学约定）
/// - 结果溢出 → `NaNOrInf`
pub fn pow(a: f64, b: f64) -> Result<f64, CalcError> {
    // 0^0 = 1 (spec Req 2 Scen 3，组合数学约定)
    if a == 0.0 && b == 0.0 {
        return Ok(1.0);
    }
    let r = a.powf(b);
    if !r.is_finite() {
        return Err(CalcError::nan_or_inf());
    }
    Ok(r)
}

/// 取模：`a % b`。`b == 0.0` 返回 `DivisionByZero`。
pub fn rem(a: f64, b: f64) -> Result<f64, CalcError> {
    if b == 0.0 {
        return Err(CalcError::division_by_zero());
    }
    let r = a % b;
    if !r.is_finite() {
        return Err(CalcError::nan_or_inf());
    }
    Ok(r)
}

/// 阶乘：`n!`。
///
/// - `n < 0` 或 `n` 非整数 → `Domain`
/// - `n > MAX_FACTORIAL_INPUT` → `Overflow`
/// - 超过 `f64::MAX` → `Overflow`
pub fn factorial(n: f64) -> Result<f64, CalcError> {
    if n < 0.0 || n.fract() != 0.0 {
        return Err(CalcError::domain(format!(
            "factorial requires non-negative integer, got {}",
            n
        ))
        .with_i18n(
            "msg.core.factorial_negative",
            vec![("value".to_string(), n.to_string())],
        ));
    }
    let n = n as u64;
    if n > MAX_FACTORIAL_INPUT {
        return Err(CalcError::overflow());
    }
    let mut result: f64 = 1.0;
    for i in 2..=n {
        result *= i as f64;
        if result.is_infinite() {
            return Err(CalcError::overflow());
        }
    }
    Ok(result)
}

/// 绝对值：`|x|`。无错误路径。
pub fn abs(x: f64) -> f64 {
    x.abs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ErrorKind;

    // ===== add =====

    #[test]
    fn test_add_normal() {
        assert_eq!(add(2.0, 3.0).unwrap(), 5.0);
    }

    #[test]
    fn test_add_negative() {
        assert_eq!(add(-1.0, 1.0).unwrap(), 0.0);
    }

    #[test]
    fn test_add_overflow() {
        let r = add(f64::MAX, f64::MAX);
        assert!(matches!(&r, Err(e) if e.kind == ErrorKind::NaNOrInf));
    }

    // ===== sub =====

    #[test]
    fn test_sub_normal() {
        assert_eq!(sub(10.0, 4.0).unwrap(), 6.0);
    }

    #[test]
    fn test_sub_overflow() {
        let r = sub(f64::MIN, f64::MAX);
        assert!(matches!(&r, Err(e) if e.kind == ErrorKind::NaNOrInf));
    }

    // ===== mul =====

    #[test]
    fn test_mul_normal() {
        assert_eq!(mul(6.0, 7.0).unwrap(), 42.0);
    }

    #[test]
    fn test_mul_zero() {
        assert_eq!(mul(0.0, 1e308).unwrap(), 0.0);
    }

    #[test]
    fn test_mul_overflow() {
        let r = mul(f64::MAX, 2.0);
        assert!(matches!(&r, Err(e) if e.kind == ErrorKind::NaNOrInf));
    }

    // ===== div =====

    #[test]
    fn test_div_normal() {
        assert_eq!(div(20.0, 4.0).unwrap(), 5.0);
    }

    #[test]
    fn test_div_zero_zero() {
        let r = div(0.0, 0.0);
        assert!(matches!(&r, Err(e) if e.kind == ErrorKind::NaNOrInf));
    }

    #[test]
    fn test_div_nonzero_zero() {
        let r = div(5.0, 0.0);
        assert!(matches!(&r, Err(e) if e.kind == ErrorKind::DivisionByZero));
    }

    // ===== pow =====

    #[test]
    fn test_pow_integer() {
        assert_eq!(pow(2.0, 10.0).unwrap(), 1024.0);
    }

    #[test]
    fn test_pow_zero_zero() {
        assert_eq!(pow(0.0, 0.0).unwrap(), 1.0);
    }

    #[test]
    fn test_pow_fractional() {
        let r = pow(2.0, 0.5).unwrap();
        assert!((r - 1.4142135623730951).abs() < 1e-10);
    }

    #[test]
    fn test_pow_overflow() {
        let r = pow(2.0, 10000.0);
        assert!(matches!(&r, Err(e) if e.kind == ErrorKind::NaNOrInf));
    }

    // ===== rem =====

    #[test]
    fn test_rem_normal() {
        assert_eq!(rem(10.0, 3.0).unwrap(), 1.0);
    }

    #[test]
    fn test_rem_negative() {
        assert_eq!(rem(-7.0, 3.0).unwrap(), -1.0);
    }

    #[test]
    fn test_rem_zero_divisor() {
        let r = rem(10.0, 0.0);
        assert!(matches!(&r, Err(e) if e.kind == ErrorKind::DivisionByZero));
    }

    // ===== factorial =====

    #[test]
    fn test_factorial_zero() {
        assert_eq!(factorial(0.0).unwrap(), 1.0);
    }

    #[test]
    fn test_factorial_five() {
        assert_eq!(factorial(5.0).unwrap(), 120.0);
    }

    #[test]
    fn test_factorial_ten() {
        assert_eq!(factorial(10.0).unwrap(), 3628800.0);
    }

    #[test]
    fn test_factorial_negative() {
        let r = factorial(-1.0);
        assert!(matches!(&r, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_factorial_fractional() {
        let r = factorial(2.5);
        assert!(matches!(&r, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_factorial_exceeds_bound() {
        let r = factorial(10001.0);
        assert!(matches!(&r, Err(e) if e.kind == ErrorKind::Overflow));
    }

    #[test]
    fn test_factorial_overflow_f64() {
        // 171! > f64::MAX
        let r = factorial(171.0);
        assert!(matches!(&r, Err(e) if e.kind == ErrorKind::Overflow));
    }

    // ===== abs =====

    #[test]
    fn test_abs_negative() {
        assert_eq!(abs(-5.0), 5.0);
    }

    #[test]
    fn test_abs_positive() {
        assert_eq!(abs(3.14), 3.14);
    }

    #[test]
    fn test_abs_zero() {
        assert_eq!(abs(0.0), 0.0);
    }
}
