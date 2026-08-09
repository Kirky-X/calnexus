// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! 复数核心函数：四则运算/模/幅角/共轭/指数/对数。

use num_complex::Complex64;

use crate::core::CalcError;

/// 复数加法。
#[inline]
pub fn add(a: Complex64, b: Complex64) -> Complex64 {
    a + b
}

/// 复数减法。
#[inline]
pub fn sub(a: Complex64, b: Complex64) -> Complex64 {
    a - b
}

/// 复数乘法。
#[inline]
pub fn mul(a: Complex64, b: Complex64) -> Complex64 {
    a * b
}

/// 复数除法。除数为零时返回 `DivisionByZero` 错误。
pub fn div(a: Complex64, b: Complex64) -> Result<Complex64, CalcError> {
    if b.norm() == 0.0 {
        return Err(CalcError::division_by_zero());
    }
    Ok(a / b)
}

/// 复数幂运算。
///
/// - `0^0 = 1`（与其他域约定一致）
/// - 结果含 NaN/Inf 时返回 `NaNOrInf` 错误
pub fn pow(a: Complex64, b: Complex64) -> Result<Complex64, CalcError> {
    let zero = Complex64::new(0.0, 0.0);
    if a == zero && b == zero {
        return Ok(Complex64::new(1.0, 0.0));
    }
    let result = a.powc(b);
    if !result.is_finite() {
        return Err(CalcError::nan_or_inf());
    }
    Ok(result)
}

/// 复数模（绝对值）。
#[inline]
pub fn norm(c: Complex64) -> f64 {
    c.norm()
}

/// 复数幅角（弧度）。`arg(0+0i)` 返回 `Domain` 错误。
pub fn arg(c: Complex64) -> Result<f64, CalcError> {
    if c.re == 0.0 && c.im == 0.0 {
        return Err(CalcError::domain(
            "arg(0+0i) is undefined (atan2(0,0) is indeterminate)".to_string(),
        ));
    }
    Ok(c.arg())
}

/// 复数共轭。
#[inline]
pub fn conj(c: Complex64) -> Complex64 {
    c.conj()
}

/// 复指数。
#[inline]
pub fn exp(c: Complex64) -> Complex64 {
    c.exp()
}

/// 复对数。
#[inline]
pub fn ln(c: Complex64) -> Complex64 {
    c.ln()
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f64 = 1e-10;

    fn assert_c64_approx(actual: Complex64, expected_re: f64, expected_im: f64) {
        assert!(
            (actual.re - expected_re).abs() < EPS && (actual.im - expected_im).abs() < EPS,
            "expected ({}, {}), got ({}, {})",
            expected_re,
            expected_im,
            actual.re,
            actual.im,
        );
    }

    // ===== 四则运算 =====

    #[test]
    fn test_add() {
        let a = Complex64::new(1.0, 2.0);
        let b = Complex64::new(3.0, 4.0);
        assert_c64_approx(add(a, b), 4.0, 6.0);
    }

    #[test]
    fn test_sub() {
        let a = Complex64::new(1.0, 2.0);
        let b = Complex64::new(3.0, 4.0);
        assert_c64_approx(sub(a, b), -2.0, -2.0);
    }

    #[test]
    fn test_mul() {
        let a = Complex64::new(1.0, 2.0);
        let b = Complex64::new(3.0, 4.0);
        // (1+2i)(3+4i) = 3+4i+6i+8i² = 3+10i-8 = -5+10i
        assert_c64_approx(mul(a, b), -5.0, 10.0);
    }

    #[test]
    fn test_div_normal() {
        let a = Complex64::new(1.0, 2.0);
        let b = Complex64::new(3.0, 4.0);
        let result = div(a, b).unwrap();
        // (1+2i)/(3+4i) = (1+2i)(3-4i)/25 = (3-4i+6i-8i²)/25 = (11+2i)/25
        assert_c64_approx(result, 11.0 / 25.0, 2.0 / 25.0);
    }

    #[test]
    fn test_div_by_zero() {
        let a = Complex64::new(1.0, 2.0);
        let b = Complex64::new(0.0, 0.0);
        assert!(matches!(div(a, b), Err(ref e) if e.kind == crate::core::ErrorKind::DivisionByZero));
    }

    // ===== 幂运算 =====

    #[test]
    fn test_pow_squared() {
        let a = Complex64::new(1.0, 1.0);
        let b = Complex64::new(2.0, 0.0);
        // (1+i)^2 = 1+2i+i² = 2i
        assert_c64_approx(pow(a, b).unwrap(), 0.0, 2.0);
    }

    #[test]
    fn test_pow_zero_zero() {
        let zero = Complex64::new(0.0, 0.0);
        assert_c64_approx(pow(zero, zero).unwrap(), 1.0, 0.0);
    }

    #[test]
    fn test_pow_overflow() {
        let a = Complex64::new(1e308, 1e308);
        let b = Complex64::new(1e308, 1e308);
        assert!(matches!(pow(a, b), Err(ref e) if e.kind == crate::core::ErrorKind::NaNOrInf));
    }

    // ===== 模/幅角/共轭/指数/对数 =====

    #[test]
    fn test_norm() {
        let c = Complex64::new(3.0, 4.0);
        assert!((norm(c) - 5.0).abs() < EPS);
    }

    #[test]
    fn test_norm_pure_imaginary() {
        let c = Complex64::new(0.0, 3.0);
        assert!((norm(c) - 3.0).abs() < EPS);
    }

    #[test]
    fn test_arg_first_quadrant() {
        let c = Complex64::new(1.0, 1.0);
        assert!((arg(c).unwrap() - std::f64::consts::FRAC_PI_4).abs() < EPS);
    }

    #[test]
    fn test_arg_positive_real() {
        let c = Complex64::new(2.0, 0.0);
        assert!((arg(c).unwrap() - 0.0).abs() < EPS);
    }

    #[test]
    fn test_arg_zero_is_error() {
        let c = Complex64::new(0.0, 0.0);
        assert!(matches!(arg(c), Err(ref e) if e.kind == crate::core::ErrorKind::Domain));
    }

    #[test]
    fn test_conj() {
        let c = Complex64::new(3.0, 4.0);
        assert_c64_approx(conj(c), 3.0, -4.0);
    }

    #[test]
    fn test_conj_pure_real() {
        let c = Complex64::new(5.0, 0.0);
        assert_c64_approx(conj(c), 5.0, 0.0);
    }

    #[test]
    fn test_exp_euler() {
        // exp(i*pi) = -1+0i
        let c = Complex64::new(0.0, std::f64::consts::PI);
        assert_c64_approx(exp(c), -1.0, 0.0);
    }

    #[test]
    fn test_exp_general() {
        let c = Complex64::new(1.0, 1.0);
        let result = exp(c);
        let expected_re = std::f64::consts::E * 1.0_f64.cos();
        let expected_im = std::f64::consts::E * 1.0_f64.sin();
        assert_c64_approx(result, expected_re, expected_im);
    }

    #[test]
    fn test_ln_one_plus_i() {
        let c = Complex64::new(1.0, 1.0);
        let expected = c.ln();
        assert_c64_approx(ln(c), expected.re, expected.im);
    }

    #[test]
    fn test_ln_positive_real() {
        let c = Complex64::new(2.0, 0.0);
        assert_c64_approx(ln(c), std::f64::consts::LN_2, 0.0);
    }
}
