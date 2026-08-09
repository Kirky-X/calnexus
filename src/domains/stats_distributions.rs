// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! 统计分布函数模块：正态/t/χ²/F/泊松/二项分布的 PDF/PMF/CDF/逆函数。
//!
//! 依赖 `stats_special` 模块的特殊函数（ln_gamma, regularized_gamma_inc, regularized_beta_inc 等）。

use super::stats_special::{ln_binomial, ln_factorial, ln_gamma, regularized_beta_inc, regularized_gamma_inc};

// ===== 正态分布 =====

/// 正态分布 PDF：f(x) = exp(-z²/2) / (σ√(2π))，z = (x-μ)/σ。
pub fn norm_pdf(x: f64, mu: f64, sigma: f64) -> f64 {
    let z = (x - mu) / sigma;
    (-0.5 * z * z).exp() / (sigma * (2.0 * std::f64::consts::PI).sqrt())
}

/// 正态分布 CDF：F(x) = 0.5 * (1 + erf(z/√2))，z = (x-μ)/σ。
///
/// 使用 `regularized_gamma_inc(0.5, z²)` 计算 erf（erf(x) = P(0.5, x²)），
/// 避免依赖不稳定的 `f64::erf()`。
pub fn norm_cdf(x: f64, mu: f64, sigma: f64) -> f64 {
    let z = (x - mu) / sigma;
    let az = z.abs();
    // erf(az/√2) = P(0.5, az²/2)
    let p = regularized_gamma_inc(0.5, 0.5 * az * az);
    if z >= 0.0 {
        0.5 + 0.5 * p
    } else {
        0.5 - 0.5 * p
    }
}

/// 正态分布逆函数（分位数）：Beasley-Springer-Moro 有理近似。
///
/// 精度 ~1e-9。p 必须在 (0, 1) 内。
pub fn norm_inv(p: f64, mu: f64, sigma: f64) -> f64 {
    if p <= 0.0 || p >= 1.0 {
        return f64::NAN;
    }
    // 标准正态分位数（μ=0, σ=1）
    let z = norm_inv_standard(p);
    mu + sigma * z
}

/// 标准正态逆函数 Beasley-Springer-Moro 算法。
fn norm_inv_standard(p: f64) -> f64 {
    // 有理近似系数（Abramowitz & Stegun 26.2.23 改进版）
    const A: [f64; 6] = [
        -3.969683028665376e+01,
        2.209460984245205e+02,
        -2.759285104469687e+02,
        1.383577518672690e+02,
        -3.066479806614716e+01,
        2.506628277459239e+00,
    ];
    const B: [f64; 5] = [
        -5.447609879822406e+01,
        1.615858368580409e+02,
        -1.556989798598866e+02,
        6.680131188771972e+01,
        -1.328068155288572e+01,
    ];
    const C: [f64; 6] = [
        -7.784894002430293e-03,
        -3.223964580411365e-01,
        -2.400758277161838e+00,
        -2.549732539343734e+00,
        4.374664141464968e+00,
        2.938163982698783e+00,
    ];
    const D: [f64; 4] = [
        7.784695709041462e-03,
        3.224671290700398e-01,
        2.445134137142996e+00,
        3.754408661907416e+00,
    ];

    let p_low = 0.02425;
    let p_high = 1.0 - p_low;

    if p < p_low {
        // 有理近似下尾
        let q = (-2.0 * p.ln()).sqrt();
        (((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    } else if p <= p_high {
        // 有理近似中心区域
        let q = p - 0.5;
        let r = q * q;
        (((((A[0] * r + A[1]) * r + A[2]) * r + A[3]) * r + A[4]) * r + A[5]) * q
            / (((((B[0] * r + B[1]) * r + B[2]) * r + B[3]) * r + B[4]) * r + 1.0)
    } else {
        // 有理近似上尾
        let q = (-2.0 * (1.0 - p).ln()).sqrt();
        -(((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    }
}

// ===== t 分布 =====

/// t 分布 PDF。
pub fn t_pdf(x: f64, df: f64) -> f64 {
    let coef = (ln_gamma((df + 1.0) / 2.0) - ln_gamma(df / 2.0) - 0.5 * (df * std::f64::consts::PI).ln()).exp();
    coef * (1.0 + x * x / df).powf(-(df + 1.0) / 2.0)
}

/// t 分布 CDF（基于 regularized_beta_inc）。
pub fn t_cdf(x: f64, df: f64) -> f64 {
    let t2 = x * x;
    let v = df / (df + t2);
    let p = 0.5 * regularized_beta_inc(df / 2.0, 0.5, v);
    if x >= 0.0 {
        1.0 - p
    } else {
        p
    }
}

/// t 分布逆函数（Newton-Raphson 迭代）。
pub fn t_inv(p: f64, df: f64) -> f64 {
    if p <= 0.0 || p >= 1.0 {
        return f64::NAN;
    }
    // 初始值用正态近似
    let mut x = norm_inv_standard(p);
    for _ in 0..100 {
        let cdf = t_cdf(x, df);
        let pdf = t_pdf(x, df);
        if pdf < 1e-300 {
            break;
        }
        let dx = (cdf - p) / pdf;
        x -= dx;
        if dx.abs() < 1e-10 {
            break;
        }
    }
    x
}

// ===== χ² 分布 =====

/// χ² 分布 PDF。
pub fn chi2_pdf(x: f64, k: f64) -> f64 {
    if x < 0.0 {
        return 0.0;
    }
    if x == 0.0 {
        if k < 2.0 { return f64::INFINITY; }
        if k == 2.0 { return 0.5; }
        return 0.0;
    }
    ((k / 2.0 - 1.0) * x.ln() - x / 2.0 - (k / 2.0) * 2.0_f64.ln() - ln_gamma(k / 2.0)).exp()
}

/// χ² 分布 CDF（基于 regularized_gamma_inc）。
pub fn chi2_cdf(x: f64, k: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    regularized_gamma_inc(k / 2.0, x / 2.0)
}

/// χ² 分布逆函数（Newton-Raphson 迭代）。
pub fn chi2_inv(p: f64, k: f64) -> f64 {
    if p <= 0.0 || p >= 1.0 {
        return f64::NAN;
    }
    // 初始值：用 Wilson-Hilferty 近似
    let z = norm_inv_standard(p);
    let y = 1.0 - 2.0 / (9.0 * k) + z * (2.0 / (9.0 * k)).sqrt();
    let mut x = k * y * y * y;
    if x <= 0.0 {
        x = k; // fallback
    }
    for _ in 0..100 {
        let cdf = chi2_cdf(x, k);
        let pdf = chi2_pdf(x, k);
        if pdf < 1e-300 {
            break;
        }
        let dx = (cdf - p) / pdf;
        x -= dx;
        if x <= 0.0 {
            x = 1e-10;
        }
        if dx.abs() < 1e-10 {
            break;
        }
    }
    x
}

// ===== F 分布 =====

/// F 分布 PDF。
pub fn f_pdf(x: f64, d1: f64, d2: f64) -> f64 {
    if x < 0.0 {
        return 0.0;
    }
    if x == 0.0 {
        if d1 < 2.0 { return f64::INFINITY; }
        if d1 == 2.0 { return 1.0; }
        return 0.0;
    }
    let log_coef = (d1 / 2.0) * d1.ln() + (d2 / 2.0) * d2.ln()
        + ln_gamma((d1 + d2) / 2.0) - ln_gamma(d1 / 2.0) - ln_gamma(d2 / 2.0);
    let log_body = ((d1 / 2.0 - 1.0) * x.ln()) - ((d1 + d2) / 2.0) * (d1 * x + d2).ln();
    (log_coef + log_body).exp()
}

/// F 分布 CDF（基于 regularized_beta_inc）。
pub fn f_cdf(x: f64, d1: f64, d2: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    let u = d1 * x / (d1 * x + d2);
    regularized_beta_inc(d1 / 2.0, d2 / 2.0, u)
}

/// F 分布逆函数（Newton-Raphson 迭代）。
pub fn f_inv(p: f64, d1: f64, d2: f64) -> f64 {
    if p <= 0.0 || p >= 1.0 {
        return f64::NAN;
    }
    // 初始值
    let mut x = 1.0;
    for _ in 0..100 {
        let cdf = f_cdf(x, d1, d2);
        let pdf = f_pdf(x, d1, d2);
        if pdf < 1e-300 {
            x *= 1.5;
            continue;
        }
        let dx = (cdf - p) / pdf;
        x -= dx;
        if x <= 0.0 {
            x = 1e-10;
        }
        if dx.abs() < 1e-10 {
            break;
        }
    }
    x
}

// ===== 泊松分布 =====

/// 泊松分布 PMF：P(X=k) = exp(k*ln(λ) - λ - ln(k!))。
pub fn poisson_pmf(k: f64, lambda: f64) -> f64 {
    if k < 0.0 || lambda <= 0.0 {
        return 0.0;
    }
    let ki = k as u64;
    if (ki as f64 - k).abs() > 1e-10 {
        return 0.0; // k 必须为整数
    }
    (ki as f64 * lambda.ln() - lambda - ln_factorial(ki)).exp()
}

/// 泊松分布 CDF：P(X ≤ k) = sum_{i=0}^{k} PMF(i)。
pub fn poisson_cdf(k: f64, lambda: f64) -> f64 {
    if k < 0.0 || lambda <= 0.0 {
        return 0.0;
    }
    let ki = k.floor() as u64;
    // 使用 regularized_gamma_inc: CDF = Q(k+1, λ) = 1 - P(k+1, λ)
    1.0 - regularized_gamma_inc(ki as f64 + 1.0, lambda)
}

// ===== 二项分布 =====

/// 二项分布 PMF：P(X=k) = C(n,k) * p^k * (1-p)^(n-k)。
pub fn binom_pmf(k: f64, n: f64, p: f64) -> f64 {
    if k < 0.0 || n < 0.0 || p < 0.0 || p > 1.0 {
        return 0.0;
    }
    let ki = k as u64;
    let ni = n as u64;
    if ki > ni {
        return 0.0;
    }
    (ln_binomial(ni, ki) + ki as f64 * p.ln() + (ni - ki) as f64 * (1.0 - p).ln()).exp()
}

/// 二项分布 CDF：P(X ≤ k) = sum_{i=0}^{k} PMF(i)。
pub fn binom_cdf(k: f64, n: f64, p: f64) -> f64 {
    if k < 0.0 || n < 0.0 || p < 0.0 || p > 1.0 {
        return 0.0;
    }
    let ki = k.floor() as u64;
    let ni = n as u64;
    if ki >= ni {
        return 1.0;
    }
    // 使用 regularized_beta_inc: CDF = 1 - I_p(k+1, n-k)
    1.0 - regularized_beta_inc(ki as f64 + 1.0, ni as f64 - ki as f64, p)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_approx(actual: f64, expected: f64, tol: f64, label: &str) {
        assert!(
            (actual - expected).abs() < tol,
            "{}: expected {} but got {} (diff={})",
            label, expected, actual, (actual - expected).abs()
        );
    }

    // ===== 正态分布 =====

    #[test]
    fn test_norm_pdf_standard() {
        assert_approx(norm_pdf(0.0, 0.0, 1.0), 0.3989422804014327, 1e-10, "norm_pdf(0,0,1)");
    }

    #[test]
    fn test_norm_cdf_standard() {
        assert_approx(norm_cdf(0.0, 0.0, 1.0), 0.5, 1e-10, "norm_cdf(0,0,1)");
    }

    #[test]
    fn test_norm_cdf_196() {
        assert_approx(norm_cdf(1.96, 0.0, 1.0), 0.975, 1e-3, "norm_cdf(1.96,0,1)");
    }

    #[test]
    fn test_norm_inv_half() {
        assert_approx(norm_inv(0.5, 0.0, 1.0), 0.0, 1e-6, "norm_inv(0.5,0,1)");
    }

    #[test]
    fn test_norm_inv_975() {
        assert_approx(norm_inv(0.975, 0.0, 1.0), 1.96, 1e-2, "norm_inv(0.975,0,1)");
    }

    #[test]
    fn test_norm_inv_025() {
        assert_approx(norm_inv(0.025, 0.0, 1.0), -1.96, 1e-2, "norm_inv(0.025,0,1)");
    }

    // ===== t 分布 =====

    #[test]
    fn test_t_pdf_zero() {
        assert_approx(t_pdf(0.0, 10.0), 0.3891, 1e-3, "t_pdf(0,10)");
    }

    #[test]
    fn test_t_cdf_zero() {
        assert_approx(t_cdf(0.0, 10.0), 0.5, 1e-10, "t_cdf(0,10)");
    }

    #[test]
    fn test_t_cdf_2228() {
        assert_approx(t_cdf(2.228, 10.0), 0.975, 1e-3, "t_cdf(2.228,10)");
    }

    #[test]
    fn test_t_inv_half() {
        assert_approx(t_inv(0.5, 10.0), 0.0, 1e-6, "t_inv(0.5,10)");
    }

    #[test]
    fn test_t_inv_975() {
        assert_approx(t_inv(0.975, 10.0), 2.228, 1e-2, "t_inv(0.975,10)");
    }

    // ===== χ² 分布 =====

    #[test]
    fn test_chi2_pdf_k2() {
        assert_approx(chi2_pdf(1.0, 2.0), 0.3033, 1e-3, "chi2_pdf(1,2)");
    }

    #[test]
    fn test_chi2_cdf_5991() {
        // 5.991 是 df=2 的 χ² 临界值（α=0.05），对 df=5 CDF ≈ 0.6929
        assert_approx(chi2_cdf(5.991, 5.0), 0.6929, 1e-3, "chi2_cdf(5.991,5)");
    }

    #[test]
    fn test_chi2_inv_95_5() {
        assert_approx(chi2_inv(0.95, 5.0), 11.070, 1e-1, "chi2_inv(0.95,5)");
    }

    // ===== F 分布 =====

    #[test]
    fn test_f_cdf_424() {
        // F(5,10) 在 x=4.24 的 CDF：4.24 > F_{0.05}=3.326，所以 CDF > 0.95
        assert_approx(f_cdf(4.24, 5.0, 10.0), 0.9751, 1e-3, "f_cdf(4.24,5,10)");
    }

    #[test]
    fn test_f_inv_95() {
        assert_approx(f_inv(0.95, 5.0, 10.0), 3.326, 1e-1, "f_inv(0.95,5,10)");
    }

    // ===== 泊松分布 =====

    #[test]
    fn test_poisson_pmf() {
        assert_approx(poisson_pmf(3.0, 2.0), 0.1804, 1e-3, "poisson_pmf(3,2)");
    }

    #[test]
    fn test_poisson_cdf() {
        assert_approx(poisson_cdf(3.0, 2.0), 0.8571, 1e-3, "poisson_cdf(3,2)");
    }

    // ===== 二项分布 =====

    #[test]
    fn test_binom_pmf() {
        assert_approx(binom_pmf(3.0, 10.0, 0.5), 0.1172, 1e-3, "binom_pmf(3,10,0.5)");
    }

    #[test]
    fn test_binom_cdf() {
        assert_approx(binom_cdf(5.0, 10.0, 0.5), 0.6230, 1e-3, "binom_cdf(5,10,0.5)");
    }
}
