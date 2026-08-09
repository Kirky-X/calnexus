// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! 科学函数核心：三角/对数/指数/双曲/gamma/erf。
//!
//! 从 `domains/scientific.rs` 提取的纯数学逻辑。

use crate::core::CalcError;

/// Lanczos 逼近系数（g=7, n=9），用于 gamma 函数。
const LANCZOS_G: f64 = 7.0;
const LANCZOS_COEF: [f64; 9] = [
    0.999_999_999_999_809_9,
    676.5203681218851,
    -1259.1392167224028,
    771.323_428_777_653_1,
    -176.615_029_162_140_6,
    12.507343278686905,
    -0.13857109526572012,
    9.984_369_578_019_572e-6,
    1.5056327351493116e-7,
];

/// Abramowitz & Stegun 7.1.26 逼近系数，用于 erf 函数（最大误差 ~1.5e-7）。
const ERF_A1: f64 = 0.254829592;
const ERF_A2: f64 = -0.284496736;
const ERF_A3: f64 = 1.421413741;
const ERF_A4: f64 = -1.453152027;
const ERF_A5: f64 = 1.061405429;
const ERF_P: f64 = 0.3275911;

// ===== 辅助函数 =====

/// 检查结果是否有限，非有限返回 NaNOrInf。
fn check_finite(value: f64) -> Result<f64, CalcError> {
    if !value.is_finite() {
        return Err(CalcError::nan_or_inf());
    }
    Ok(value)
}

// ===== 三角函数 =====

/// 正弦函数。NaN/Inf 输入 → NaNOrInf。
pub fn sin(x: f64) -> Result<f64, CalcError> {
    check_finite(x.sin())
}

/// 余弦函数。
pub fn cos(x: f64) -> Result<f64, CalcError> {
    check_finite(x.cos())
}

/// 正切函数。
pub fn tan(x: f64) -> Result<f64, CalcError> {
    check_finite(x.tan())
}

// ===== 反三角函数 =====

/// 反正弦函数。输入须在 [-1, 1]，否则返回 Domain。
pub fn asin(x: f64) -> Result<f64, CalcError> {
    if !(-1.0..=1.0).contains(&x) {
        return Err(
            CalcError::domain(format!("asin requires argument in [-1, 1], got {}", x))
                .with_hint("asin domain is [-1, 1]")
                .with_i18n(
                    "msg.scientific.asin_domain",
                    vec![("value".to_string(), x.to_string())],
                ),
        );
    }
    check_finite(x.asin())
}

/// 反余弦函数。输入须在 [-1, 1]，否则返回 Domain。
pub fn acos(x: f64) -> Result<f64, CalcError> {
    if !(-1.0..=1.0).contains(&x) {
        return Err(
            CalcError::domain(format!("acos requires argument in [-1, 1], got {}", x))
                .with_hint("acos domain is [-1, 1]")
                .with_i18n(
                    "msg.scientific.acos_domain",
                    vec![("value".to_string(), x.to_string())],
                ),
        );
    }
    check_finite(x.acos())
}

/// 反正切函数。
pub fn atan(x: f64) -> Result<f64, CalcError> {
    check_finite(x.atan())
}

// ===== 对数函数 =====

/// 自然对数。`x <= 0.0` 返回 Domain。
pub fn ln(x: f64) -> Result<f64, CalcError> {
    if x <= 0.0 {
        return Err(
            CalcError::domain(format!("ln requires positive argument, got {}", x)).with_i18n(
                "msg.scientific.ln_positive",
                vec![("value".to_string(), x.to_string())],
            ),
        );
    }
    check_finite(x.ln())
}

/// 以 10 为底的对数。`x <= 0.0` 返回 Domain。
pub fn log10(x: f64) -> Result<f64, CalcError> {
    if x <= 0.0 {
        return Err(
            CalcError::domain(format!("log10 requires positive argument, got {}", x))
                .with_i18n(
                    "msg.scientific.log10_positive",
                    vec![("value".to_string(), x.to_string())],
                ),
        );
    }
    check_finite(x.log10())
}

/// 以 2 为底的对数。`x <= 0.0` 返回 Domain。
pub fn log2(x: f64) -> Result<f64, CalcError> {
    if x <= 0.0 {
        return Err(
            CalcError::domain(format!("log2 requires positive argument, got {}", x)).with_i18n(
                "msg.scientific.log2_positive",
                vec![("value".to_string(), x.to_string())],
            ),
        );
    }
    check_finite(x.log2())
}

/// 任意底对数：`log(value, base)`。
///
/// - `value <= 0.0` → Domain
/// - `base <= 0.0 || base == 1.0` → Domain
pub fn log(value: f64, base: f64) -> Result<f64, CalcError> {
    if value <= 0.0 {
        return Err(
            CalcError::domain(format!("log requires positive value, got {}", value)).with_i18n(
                "msg.scientific.log_positive_value",
                vec![("value".to_string(), value.to_string())],
            ),
        );
    }
    if base <= 0.0 || (base - 1.0).abs() < f64::EPSILON {
        return Err(CalcError::domain(format!(
            "log requires positive base != 1, got {}",
            base
        ))
        .with_i18n(
            "msg.scientific.log_positive_base",
            vec![("value".to_string(), base.to_string())],
        ));
    }
    check_finite(value.log(base))
}

// ===== 指数函数 =====

/// 指数函数 `e^x`。溢出返回 NaNOrInf。
pub fn exp(x: f64) -> Result<f64, CalcError> {
    check_finite(x.exp())
}

// ===== 双曲函数 =====

/// 双曲正弦。
pub fn sinh(x: f64) -> Result<f64, CalcError> {
    check_finite(x.sinh())
}

/// 双曲余弦。
pub fn cosh(x: f64) -> Result<f64, CalcError> {
    check_finite(x.cosh())
}

/// 双曲正切。
pub fn tanh(x: f64) -> Result<f64, CalcError> {
    check_finite(x.tanh())
}

// ===== 特殊函数 =====

/// Lanczos 逼近计算 gamma 函数。
///
/// 对 x > 0.5 使用 Lanczos 逼近；对 x < 0.5 使用反射公式 Γ(z)Γ(1-z) = π/sin(πz)。
/// 非正整数是 gamma 函数的极点，显式拒绝为 Domain 错误。
pub fn gamma(x: f64) -> Result<f64, CalcError> {
    // 非正整数（0, -1, -2, …）是 gamma 函数的极点
    if x <= 0.0 && x == x.floor() && x.is_finite() {
        return Err(
            CalcError::domain(format!("gamma({}) is undefined: pole at non-positive integer", x))
                .with_hint("gamma is defined for positive reals and non-integer negatives")
                .with_i18n(
                    "msg.scientific.gamma_pole",
                    vec![("value".to_string(), x.to_string())],
                ),
        );
    }
    check_finite(lanczos_gamma(x))
}

/// Abramowitz & Stegun 7.1.26 逼近计算 erf 函数（最大误差 ~1.5e-7）。
pub fn erf(x: f64) -> Result<f64, CalcError> {
    check_finite(erf_raw(x))
}

/// Lanczos 逼近计算 gamma 函数（内部，返回原始 f64）。
fn lanczos_gamma(x: f64) -> f64 {
    if x < 0.5 {
        std::f64::consts::PI / ((std::f64::consts::PI * x).sin() * lanczos_gamma(1.0 - x))
    } else {
        let x = x - 1.0;
        let mut a = LANCZOS_COEF[0];
        let t = x + LANCZOS_G + 0.5;
        for (i, coef) in LANCZOS_COEF.iter().enumerate().skip(1) {
            a += coef / (x + i as f64);
        }
        (2.0 * std::f64::consts::PI).sqrt() * t.powf(x + 0.5) * (-t).exp() * a
    }
}

/// A&S 7.1.26 erf 逼近（内部，返回原始 f64）。
fn erf_raw(x: f64) -> f64 {
    // erf(0) = 0 数学上精确，A&S 逼近在此点有 ~1e-7 误差，特判以保证 spec 精确匹配。
    if x == 0.0 {
        return 0.0;
    }
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();
    let t = 1.0 / (1.0 + ERF_P * x);
    let y = 1.0
        - (((((ERF_A5 * t + ERF_A4) * t) + ERF_A3) * t + ERF_A2) * t + ERF_A1) * t * (-x * x).exp();
    sign * y
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ErrorKind;

    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-10
    }

    // ===== 三角函数 =====

    #[test]
    fn test_sin_zero() {
        assert!(approx(sin(0.0).unwrap(), 0.0));
    }

    #[test]
    fn test_cos_zero() {
        assert!(approx(cos(0.0).unwrap(), 1.0));
    }

    #[test]
    fn test_sin_pi_over_two() {
        assert!(approx(sin(std::f64::consts::FRAC_PI_2).unwrap(), 1.0));
    }

    #[test]
    fn test_tan_pi_over_four() {
        assert!(approx(tan(std::f64::consts::FRAC_PI_4).unwrap(), 1.0));
    }

    // ===== 反三角函数 =====

    #[test]
    fn test_asin_one() {
        assert!(approx(asin(1.0).unwrap(), std::f64::consts::FRAC_PI_2));
    }

    #[test]
    fn test_asin_out_of_range() {
        let r = asin(2.0);
        assert!(matches!(&r, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_acos_zero() {
        assert!(approx(acos(0.0).unwrap(), std::f64::consts::FRAC_PI_2));
    }

    #[test]
    fn test_acos_out_of_range() {
        let r = acos(-1.5);
        assert!(matches!(&r, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_atan_one() {
        assert!(approx(atan(1.0).unwrap(), std::f64::consts::FRAC_PI_4));
    }

    // ===== 对数函数 =====

    #[test]
    fn test_ln_e() {
        assert!(approx(ln(std::f64::consts::E).unwrap(), 1.0));
    }

    #[test]
    fn test_ln_one() {
        assert!(approx(ln(1.0).unwrap(), 0.0));
    }

    #[test]
    fn test_ln_negative() {
        let r = ln(-1.0);
        assert!(matches!(&r, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_ln_zero() {
        let r = ln(0.0);
        assert!(matches!(&r, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_log10_100() {
        assert!(approx(log10(100.0).unwrap(), 2.0));
    }

    #[test]
    fn test_log2_8() {
        assert!(approx(log2(8.0).unwrap(), 3.0));
    }

    #[test]
    fn test_log_arbitrary_base() {
        assert!(approx(log(100.0, 10.0).unwrap(), 2.0));
    }

    #[test]
    fn test_log_bad_base() {
        let r = log(100.0, 1.0);
        assert!(matches!(&r, Err(e) if e.kind == ErrorKind::Domain));
    }

    // ===== 指数函数 =====

    #[test]
    fn test_exp_zero() {
        assert!(approx(exp(0.0).unwrap(), 1.0));
    }

    #[test]
    fn test_exp_one() {
        assert!(approx(exp(1.0).unwrap(), std::f64::consts::E));
    }

    #[test]
    fn test_exp_overflow() {
        let r = exp(1000.0);
        assert!(matches!(&r, Err(e) if e.kind == ErrorKind::NaNOrInf));
    }

    // ===== 双曲函数 =====

    #[test]
    fn test_sinh_zero() {
        assert!(approx(sinh(0.0).unwrap(), 0.0));
    }

    #[test]
    fn test_cosh_zero() {
        assert!(approx(cosh(0.0).unwrap(), 1.0));
    }

    #[test]
    fn test_tanh_zero() {
        assert!(approx(tanh(0.0).unwrap(), 0.0));
    }

    // ===== 特殊函数 =====

    #[test]
    fn test_gamma_five() {
        assert!(approx(gamma(5.0).unwrap(), 24.0));
    }

    #[test]
    fn test_gamma_one() {
        assert!(approx(gamma(1.0).unwrap(), 1.0));
    }

    #[test]
    fn test_gamma_reflection() {
        // gamma(-0.5) = -2*sqrt(pi)
        assert!(approx(gamma(-0.5).unwrap(), -2.0 * std::f64::consts::PI.sqrt()));
    }

    #[test]
    fn test_erf_zero() {
        assert!(approx(erf(0.0).unwrap(), 0.0));
    }

    #[test]
    fn test_erf_large() {
        assert!(approx(erf(100.0).unwrap(), 1.0));
    }
}
