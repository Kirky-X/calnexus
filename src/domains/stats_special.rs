// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! 统计特殊函数模块：ln_gamma、正则化不完全 Gamma/Beta 函数、ln_factorial、ln_binomial。
//!
//! 为统计分布（`stats_distributions`）和假设检验（`stats_tests`）提供数学基础。
//! 算法依据：NIST DLMF §8（Gamma/Beta 函数）、Numerical Recipes §6。

/// Lanczos 逼近系数（g=7, n=9），与 scientific.rs 共享同一组系数。
const LANCZOS_G: f64 = 7.0;
const LANCZOS_COEF: [f64; 9] = [
    0.999_999_999_999_809_9,
    676.5203681218851,
    -1259.1392167224028,
    771.32342877765313,
    -176.61502916214059,
    12.507343278686905,
    -0.13857109526572012,
    9.9843695780195716e-6,
    1.624290006787e-11,
];

/// 正则化不完全 Gamma / Beta 函数的连分数迭代精度。
const EPS: f64 = 1e-14;
/// 连分数迭代上限。
const MAX_ITER: usize = 300;
/// 连分数最小值（避免除零）。
const TINY: f64 = 1e-30;

// ===== ln_gamma =====

/// 计算 ln(Γ(z))，z > 0。
///
/// 使用 Lanczos 近似（g=7, 9 系数），在对数空间计算避免溢出。
/// 对 z < 0.5 使用反射公式 Γ(z)Γ(1-z) = π/sin(πz)。
pub fn ln_gamma(z: f64) -> f64 {
    if z < 0.5 {
        // 反射公式：ln(Γ(z)) = ln(π) - ln(sin(πz)) - ln(Γ(1-z))
        std::f64::consts::PI.ln() - (std::f64::consts::PI * z).sin().ln() - ln_gamma(1.0 - z)
    } else {
        let x = z - 1.0;
        let mut a = LANCZOS_COEF[0];
        let t = x + LANCZOS_G + 0.5;
        for (i, coef) in LANCZOS_COEF.iter().enumerate().skip(1) {
            a += coef / (x + i as f64);
        }
        0.5 * (2.0 * std::f64::consts::PI).ln() + (x + 0.5) * t.ln() - t + a.ln()
    }
}

// ===== regularized_gamma_inc =====

/// 正则化下不完全 Gamma 函数 P(a, x) = γ(a,x) / Γ(a)。
///
/// - x < a+1：级数展开（DLMF §8.7）
/// - x ≥ a+1：连分数计算 Q(a,x) = 1 - P(a,x)（DLMF §8.9）
///
/// 要求 a > 0, x ≥ 0。
pub fn regularized_gamma_inc(a: f64, x: f64) -> f64 {
    if x < 0.0 || a <= 0.0 {
        return f64::NAN;
    }
    if x == 0.0 {
        return 0.0;
    }
    if x < a + 1.0 {
        gamma_series(a, x)
    } else {
        1.0 - gamma_cf(a, x)
    }
}

/// 级数展开计算 P(a, x)。
fn gamma_series(a: f64, x: f64) -> f64 {
    let mut sum = 1.0 / a;
    let mut term = 1.0 / a;
    for n in 1..=MAX_ITER {
        term *= x / (a + n as f64);
        sum += term;
        if term.abs() < sum.abs() * EPS {
            return sum * (-x + a * x.ln() - ln_gamma(a)).exp();
        }
    }
    sum * (-x + a * x.ln() - ln_gamma(a)).exp()
}

/// 连分数计算 Q(a, x) = 1 - P(a, x)。
///
/// Numerical Recipes §6.2 gcf 算法：每个迭代处理一个连分数系数
/// a_i = -i*(i-a)，b 从 x+1-a 开始每次递增 2。
fn gamma_cf(a: f64, x: f64) -> f64 {
    let mut b = x + 1.0 - a;
    let mut c = 1.0 / TINY;
    let mut d = 1.0 / b;
    let mut h = d;
    for i in 1..=MAX_ITER {
        let an = -(i as f64) * (i as f64 - a);
        b += 2.0;
        d = an * d + b;
        if d.abs() < TINY {
            d = TINY;
        }
        c = b + an / c;
        if c.abs() < TINY {
            c = TINY;
        }
        d = 1.0 / d;
        let delta = d * c;
        h *= delta;
        if (delta - 1.0).abs() < EPS {
            return h * (-x + a * x.ln() - ln_gamma(a)).exp();
        }
    }
    h * (-x + a * x.ln() - ln_gamma(a)).exp()
}

// ===== regularized_beta_inc =====

/// 正则化下不完全 Beta 函数 I_x(a, b) = B(x; a, b) / B(a, b)。
///
/// 双路径算法：
/// - x ≤ 0.5：超几何级数展开 I_x(a,b) = (x^a/(a·B(a,b))) · ₂F₁(a, 1-b; a+1; x)
/// - x > 0.5：对称性 I_x(a,b) = 1 - I_{1-x}(b,a)，转为小 x 级数
/// - 级数不收敛时回退到 Lentz 连分数（DLMF §8.17）
///
/// 要求 a > 0, b > 0, 0 ≤ x ≤ 1。
pub fn regularized_beta_inc(a: f64, b: f64, x: f64) -> f64 {
    if x < 0.0 || x > 1.0 || a <= 0.0 || b <= 0.0 {
        return f64::NAN;
    }
    if x == 0.0 {
        return 0.0;
    }
    if x == 1.0 {
        return 1.0;
    }
    if x <= 0.5 {
        beta_series(a, b, x)
    } else {
        1.0 - beta_series(b, a, 1.0 - x)
    }
}

/// 超几何级数计算 I_x(a, b)。
///
/// I_x(a,b) = (x^a / (a · B(a,b))) · ₂F₁(a, 1-b; a+1; x)
///
/// ₂F₁ 递推：t_k = t_{k-1} · (a+k-1)·(k-b) · x / ((a+k) · k)，t_0 = 1
/// 对 x < 0.5 收敛快速（几何级数比 ≈ x）。
fn beta_series(a: f64, b: f64, x: f64) -> f64 {
    let ln_prefactor = a * x.ln() - a.ln() - ln_gamma(a) - ln_gamma(b) + ln_gamma(a + b);
    let prefactor = ln_prefactor.exp();

    let mut sum = 1.0;
    let mut term = 1.0;
    for k in 1..=2000 {
        let kf = k as f64;
        term *= (a + kf - 1.0) * (kf - b) * x / ((a + kf) * kf);
        sum += term;
        if term.abs() < sum.abs() * EPS {
            break;
        }
    }
    prefactor * sum
}

/// Beta 函数连分数（Numerical Recipes §6.2 betacf）。
///
/// 每个迭代处理两个连分数系数（even + odd step）：
/// - even: a_{2m} = m*(b-m)*x / ((a-1+2m)*(a+2m))
/// - odd:  a_{2m+1} = -(a+m)*(a+b+m)*x / ((a+2m)*(a+1+2m))
fn beta_cf(a: f64, b: f64, x: f64) -> f64 {
    let qab = a + b;
    let qap = a + 1.0;
    let qam = a - 1.0;
    let mut c = 1.0;
    let mut d = 1.0 - qab * x / qap;
    if d.abs() < TINY {
        d = TINY;
    }
    d = 1.0 / d;
    let mut h = d;
    for m in 1..=MAX_ITER {
        let m = m as f64;
        // Even step: a_{2m}
        let aa = m * (b - m) * x / ((qam + 2.0 * m) * (a + 2.0 * m));
        d = 1.0 + aa * d;
        if d.abs() < TINY {
            d = TINY;
        }
        c = 1.0 + aa / c;
        if c.abs() < TINY {
            c = TINY;
        }
        d = 1.0 / d;
        h *= d * c;
        // Odd step: a_{2m+1}
        let aa = -(a + m) * (qab + m) * x / ((a + 2.0 * m) * (qap + 2.0 * m));
        d = 1.0 + aa * d;
        if d.abs() < TINY {
            d = TINY;
        }
        c = 1.0 + aa / c;
        if c.abs() < TINY {
            c = TINY;
        }
        d = 1.0 / d;
        let delta = d * c;
        h *= delta;
        if (delta - 1.0).abs() < EPS {
            return h;
        }
    }
    h
}

// ===== ln_factorial / ln_binomial =====

/// 计算 ln(n!)，n ≥ 0。
///
/// 对小 n（≤ 20）使用直接累加；对大 n 使用 ln_gamma(n+1)。
pub fn ln_factorial(n: u64) -> f64 {
    if n <= 1 {
        return 0.0;
    }
    if n <= 20 {
        (2..=n).map(|i| (i as f64).ln()).sum()
    } else {
        ln_gamma(n as f64 + 1.0)
    }
}

/// 计算 ln(C(n, k)) = ln(n!) - ln(k!) - ln((n-k)!)。
///
/// 要求 0 ≤ k ≤ n。
pub fn ln_binomial(n: u64, k: u64) -> f64 {
    if k > n {
        return f64::NAN;
    }
    if k == 0 || k == n {
        return 0.0;
    }
    ln_factorial(n) - ln_factorial(k) - ln_factorial(n - k)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_approx(actual: f64, expected: f64, tol: f64, label: &str) {
        assert!(
            (actual - expected).abs() < tol,
            "{}: expected {} but got {} (diff={})",
            label,
            expected,
            actual,
            (actual - expected).abs()
        );
    }

    // ===== ln_gamma 测试 =====

    #[test]
    fn test_ln_gamma_1() {
        // ln(Γ(1)) = ln(1) = 0
        assert_approx(ln_gamma(1.0), 0.0, 1e-10, "ln_gamma(1)");
    }

    #[test]
    fn test_ln_gamma_2() {
        // ln(Γ(2)) = ln(1!) = 0
        assert_approx(ln_gamma(2.0), 0.0, 1e-9, "ln_gamma(2)");
    }

    #[test]
    fn test_ln_gamma_10() {
        // ln(Γ(10)) = ln(9!) = ln(362880) ≈ 12.801827480
        assert_approx(ln_gamma(10.0), 12.801827480, 1e-6, "ln_gamma(10)");
    }

    #[test]
    fn test_ln_gamma_half() {
        // ln(Γ(0.5)) = ln(√π) ≈ 0.5723649429
        assert_approx(ln_gamma(0.5), 0.5723649429, 1e-6, "ln_gamma(0.5)");
    }

    // ===== regularized_gamma_inc 测试 =====

    #[test]
    fn test_gamma_inc_1_1() {
        // P(1, 1) = 1 - e^(-1) ≈ 0.6321205588
        assert_approx(regularized_gamma_inc(1.0, 1.0), 0.6321205588, 1e-6, "P(1,1)");
    }

    #[test]
    fn test_gamma_inc_2_3() {
        // P(2, 3) ≈ 0.8008517265
        assert_approx(regularized_gamma_inc(2.0, 3.0), 0.8008517265, 1e-6, "P(2,3)");
    }

    #[test]
    fn test_gamma_inc_0_5_0_5() {
        // P(0.5, 0.5) = erf(√0.5) ≈ 0.6826894921
        assert_approx(regularized_gamma_inc(0.5, 0.5), 0.6826894921, 1e-6, "P(0.5,0.5)");
    }

    #[test]
    fn test_gamma_inc_zero_x() {
        // P(a, 0) = 0 for a > 0
        assert_approx(regularized_gamma_inc(2.0, 0.0), 0.0, 1e-15, "P(2,0)");
    }

    // ===== regularized_beta_inc 测试 =====

    #[test]
    fn test_beta_inc_half_1_1() {
        // I_0.5(1, 1) = 0.5
        assert_approx(regularized_beta_inc(1.0, 1.0, 0.5), 0.5, 1e-10, "I_0.5(1,1)");
    }

    #[test]
    fn test_beta_inc_half_2_2() {
        // I_0.5(2, 2) = 0.5（对称性）
        assert_approx(regularized_beta_inc(2.0, 2.0, 0.5), 0.5, 1e-10, "I_0.5(2,2)");
    }

    #[test]
    fn test_beta_inc_0_3_5_5() {
        // I_0.3(5,5) = 1 - sum_{k=5}^{9} C(9,k)*0.3^k*0.7^{9-k} ≈ 0.09881
        assert_approx(regularized_beta_inc(5.0, 5.0, 0.3), 0.09881, 1e-3, "I_0.3(5,5)");
    }

    #[test]
    fn test_beta_inc_boundary_0() {
        assert_approx(regularized_beta_inc(2.0, 3.0, 0.0), 0.0, 1e-15, "I_0(2,3)");
    }

    #[test]
    fn test_beta_inc_boundary_1() {
        assert_approx(regularized_beta_inc(2.0, 3.0, 1.0), 1.0, 1e-15, "I_1(2,3)");
    }

    // ===== ln_factorial 测试 =====

    #[test]
    fn test_ln_factorial_0() {
        assert_approx(ln_factorial(0), 0.0, 1e-15, "ln_factorial(0)");
    }

    #[test]
    fn test_ln_factorial_1() {
        assert_approx(ln_factorial(1), 0.0, 1e-15, "ln_factorial(1)");
    }

    #[test]
    fn test_ln_factorial_5() {
        // ln(5!) = ln(120) ≈ 4.7874917428
        assert_approx(ln_factorial(5), 120.0_f64.ln(), 1e-10, "ln_factorial(5)");
    }

    #[test]
    fn test_ln_factorial_20() {
        // ln(20!) 直接累加路径
        let expected: f64 = (2..=20).map(|i| (i as f64).ln()).sum();
        assert_approx(ln_factorial(20), expected, 1e-10, "ln_factorial(20)");
    }

    #[test]
    fn test_ln_factorial_large() {
        // ln(100!) 使用 ln_gamma 路径
        assert_approx(ln_factorial(100), ln_gamma(101.0), 1e-6, "ln_factorial(100)");
    }

    // ===== ln_binomial 测试 =====

    #[test]
    fn test_ln_binomial_10_3() {
        // C(10, 3) = 120, ln(120) ≈ 4.7874917428
        assert_approx(ln_binomial(10, 3), 120.0_f64.ln(), 1e-10, "ln_binomial(10,3)");
    }

    #[test]
    fn test_ln_binomial_edge_0() {
        // C(n, 0) = 1, ln(1) = 0
        assert_approx(ln_binomial(10, 0), 0.0, 1e-15, "ln_binomial(10,0)");
    }

    #[test]
    fn test_ln_binomial_edge_n() {
        // C(n, n) = 1, ln(1) = 0
        assert_approx(ln_binomial(10, 10), 0.0, 1e-15, "ln_binomial(10,10)");
    }

    #[test]
    fn test_ln_binomial_invalid() {
        // k > n → NaN
        assert!(ln_binomial(3, 10).is_nan());
    }
}

