// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! 统计核心函数：基础统计 + 特殊函数 + 分布 + 检验 + 相关。
//!
//! 将 `domains/statistics.rs`、`domains/stats_special.rs`、
//! `domains/stats_distributions.rs`、`domains/stats_tests.rs` 中的
//! 纯数学逻辑统一提取到此模块，供域层和 API 层共用。

// ===== 常量 =====

/// Lanczos 逼近系数（g=7, n=9）。
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
    1.624290006787e-11,
];

/// 连分数迭代精度 / 上限 / 最小值。
const EPS: f64 = 1e-14;
const MAX_ITER: usize = 300;
const TINY: f64 = 1e-30;

use std::collections::HashMap;
use nalgebra::DMatrix;
use crate::core::CalcError;

// ===== 基础统计函数 =====

pub fn mean(values: &[f64]) -> f64 {
    if values.is_empty() { return f64::NAN; }
    values.iter().sum::<f64>() / values.len() as f64
}

pub fn variance(values: &[f64]) -> f64 {
    if values.is_empty() { return f64::NAN; }
    let m = mean(values);
    values.iter().map(|x| (x - m).powi(2)).sum::<f64>() / values.len() as f64
}

pub fn std(values: &[f64]) -> f64 {
    variance(values).sqrt()
}

pub fn median(values: &[f64]) -> f64 {
    if values.is_empty() { return f64::NAN; }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = sorted.len();
    if n % 2 == 1 {
        sorted[n / 2]
    } else {
        (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0
    }
}

pub fn min(values: &[f64]) -> f64 {
    if values.is_empty() { return f64::NAN; }
    values.iter().cloned().fold(f64::INFINITY, f64::min)
}

pub fn max(values: &[f64]) -> f64 {
    if values.is_empty() { return f64::NAN; }
    values.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
}

pub fn sum(values: &[f64]) -> f64 {
    values.iter().sum()
}

pub fn count(values: &[f64]) -> f64 {
    values.len() as f64
}

// ===== 特殊函数（原 stats_special.rs）=====

/// 计算 ln(Γ(z))，z > 0。Lanczos 近似（g=7, 9 系数）。
/// MEDIUM #51 修复：z <= 0 时返回 f64::NAN 而非产生误导性结果。
pub fn ln_gamma(z: f64) -> f64 {
    if z <= 0.0 {
        return f64::NAN;
    }
    if z < 0.5 {
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

/// 正则化下不完全 Gamma 函数 P(a, x) = γ(a,x) / Γ(a)。
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

fn gamma_series(a: f64, x: f64) -> f64 {
    let mut sum_val = 1.0 / a;
    let mut term = 1.0 / a;
    for n in 1..=MAX_ITER {
        term *= x / (a + n as f64);
        sum_val += term;
        if term.abs() < sum_val.abs() * EPS {
            return sum_val * (-x + a * x.ln() - ln_gamma(a)).exp();
        }
    }
    sum_val * (-x + a * x.ln() - ln_gamma(a)).exp()
}

fn gamma_cf(a: f64, x: f64) -> f64 {
    let mut b = x + 1.0 - a;
    let mut c = 1.0 / TINY;
    let mut d = 1.0 / b;
    let mut h = d;
    for i in 1..=MAX_ITER {
        let an = -(i as f64) * (i as f64 - a);
        b += 2.0;
        d = an * d + b;
        if d.abs() < TINY { d = TINY; }
        c = b + an / c;
        if c.abs() < TINY { c = TINY; }
        d = 1.0 / d;
        let delta = d * c;
        h *= delta;
        if (delta - 1.0).abs() < EPS {
            return h * (-x + a * x.ln() - ln_gamma(a)).exp();
        }
    }
    h * (-x + a * x.ln() - ln_gamma(a)).exp()
}

/// 正则化下不完全 Beta 函数 I_x(a, b)。
pub fn regularized_beta_inc(a: f64, b: f64, x: f64) -> f64 {
    if !(0.0..=1.0).contains(&x) || a <= 0.0 || b <= 0.0 {
        return f64::NAN;
    }
    if x == 0.0 { return 0.0; }
    if x == 1.0 { return 1.0; }
    if x <= 0.5 {
        beta_series(a, b, x)
    } else {
        1.0 - beta_series(b, a, 1.0 - x)
    }
}

fn beta_series(a: f64, b: f64, x: f64) -> f64 {
    let ln_prefactor = a * x.ln() - a.ln() - ln_gamma(a) - ln_gamma(b) + ln_gamma(a + b);
    let prefactor = ln_prefactor.exp();
    let mut sum_val = 1.0;
    let mut term = 1.0;
    for k in 1..=2000 {
        let kf = k as f64;
        term *= (a + kf - 1.0) * (kf - b) * x / ((a + kf) * kf);
        sum_val += term;
        if term.abs() < sum_val.abs() * EPS { break; }
    }
    prefactor * sum_val
}

#[allow(dead_code)]
fn beta_cf(a: f64, b: f64, x: f64) -> f64 {
    let qab = a + b;
    let qap = a + 1.0;
    let qam = a - 1.0;
    let mut c = 1.0;
    let mut d = 1.0 - qab * x / qap;
    if d.abs() < TINY { d = TINY; }
    d = 1.0 / d;
    let mut h = d;
    for m in 1..=MAX_ITER {
        let m = m as f64;
        let aa = m * (b - m) * x / ((qam + 2.0 * m) * (a + 2.0 * m));
        d = 1.0 + aa * d;
        if d.abs() < TINY { d = TINY; }
        c = 1.0 + aa / c;
        if c.abs() < TINY { c = TINY; }
        d = 1.0 / d;
        h *= d * c;
        let aa = -(a + m) * (qab + m) * x / ((a + 2.0 * m) * (qap + 2.0 * m));
        d = 1.0 + aa * d;
        if d.abs() < TINY { d = TINY; }
        c = 1.0 + aa / c;
        if c.abs() < TINY { c = TINY; }
        d = 1.0 / d;
        let delta = d * c;
        h *= delta;
        if (delta - 1.0).abs() < EPS { return h; }
    }
    h
}

/// 计算 ln(n!)。小 n（≤20）直接累加；大 n 用 ln_gamma(n+1)。
pub fn ln_factorial(n: u64) -> f64 {
    if n <= 1 { return 0.0; }
    if n <= 20 {
        (2..=n).map(|i| (i as f64).ln()).sum()
    } else {
        ln_gamma(n as f64 + 1.0)
    }
}

/// 计算 ln(C(n, k)) = ln(n!) - ln(k!) - ln((n-k)!)。
pub fn ln_binomial(n: u64, k: u64) -> f64 {
    if k > n { return f64::NAN; }
    if k == 0 || k == n { return 0.0; }
    ln_factorial(n) - ln_factorial(k) - ln_factorial(n - k)
}

// ===== 分布函数（原 stats_distributions.rs）=====

/// 正态分布 PDF。
pub fn norm_pdf(x: f64, mu: f64, sigma: f64) -> f64 {
    let z = (x - mu) / sigma;
    (-0.5 * z * z).exp() / (sigma * (2.0 * std::f64::consts::PI).sqrt())
}

/// 正态分布 CDF。
pub fn norm_cdf(x: f64, mu: f64, sigma: f64) -> f64 {
    let z = (x - mu) / sigma;
    let az = z.abs();
    let p = regularized_gamma_inc(0.5, 0.5 * az * az);
    if z >= 0.0 { 0.5 + 0.5 * p } else { 0.5 - 0.5 * p }
}

/// 正态分布逆函数（Beasley-Springer-Moro）。
pub fn norm_inv(p: f64, mu: f64, sigma: f64) -> f64 {
    if p <= 0.0 || p >= 1.0 { return f64::NAN; }
    mu + sigma * norm_inv_standard(p)
}

fn norm_inv_standard(p: f64) -> f64 {
    const A: [f64; 6] = [-3.969683028665376e+01, 2.209460984245205e+02, -2.759285104469687e+02,
        1.383_577_518_672_69e2, -3.066479806614716e+01, 2.506628277459239e+00];
    const B: [f64; 5] = [-5.447609879822406e+01, 1.615858368580409e+02, -1.556989798598866e+02,
        6.680131188771972e+01, -1.328068155288572e+01];
    const C: [f64; 6] = [-7.784894002430293e-03, -3.223964580411365e-01, -2.400758277161838e+00,
        -2.549732539343734e+00, 4.374664141464968e+00, 2.938163982698783e+00];
    const D: [f64; 4] = [7.784695709041462e-03, 3.224671290700398e-01,
        2.445134137142996e+00, 3.754408661907416e+00];
    let p_low = 0.02425;
    let p_high = 1.0 - p_low;
    if p < p_low {
        let q = (-2.0 * p.ln()).sqrt();
        (((((C[0]*q+C[1])*q+C[2])*q+C[3])*q+C[4])*q+C[5])
            / ((((D[0]*q+D[1])*q+D[2])*q+D[3])*q+1.0)
    } else if p <= p_high {
        let q = p - 0.5;
        let r = q * q;
        (((((A[0]*r+A[1])*r+A[2])*r+A[3])*r+A[4])*r+A[5])*q
            / (((((B[0]*r+B[1])*r+B[2])*r+B[3])*r+B[4])*r+1.0)
    } else {
        let q = (-2.0 * (1.0 - p).ln()).sqrt();
        -(((((C[0]*q+C[1])*q+C[2])*q+C[3])*q+C[4])*q+C[5])
            / ((((D[0]*q+D[1])*q+D[2])*q+D[3])*q+1.0)
    }
}

/// t 分布 PDF。
pub fn t_pdf(x: f64, df: f64) -> f64 {
    let coef = (ln_gamma((df+1.0)/2.0) - ln_gamma(df/2.0) - 0.5*(df*std::f64::consts::PI).ln()).exp();
    coef * (1.0 + x*x/df).powf(-(df+1.0)/2.0)
}

/// t 分布 CDF。
pub fn t_cdf(x: f64, df: f64) -> f64 {
    let t2 = x * x;
    let v = df / (df + t2);
    let p = 0.5 * regularized_beta_inc(df/2.0, 0.5, v);
    if x >= 0.0 { 1.0 - p } else { p }
}

/// t 分布逆函数（Newton-Raphson）。
pub fn t_inv(p: f64, df: f64) -> f64 {
    if p <= 0.0 || p >= 1.0 { return f64::NAN; }
    let mut x = norm_inv_standard(p);
    for _ in 0..100 {
        let cdf = t_cdf(x, df);
        let pdf = t_pdf(x, df);
        if pdf < 1e-300 { break; }
        let dx = (cdf - p) / pdf;
        x -= dx;
        if dx.abs() < 1e-10 { break; }
    }
    x
}

/// χ² 分布 PDF。
pub fn chi2_pdf(x: f64, k: f64) -> f64 {
    if x < 0.0 { return 0.0; }
    if x == 0.0 {
        if k < 2.0 { return f64::INFINITY; }
        if k == 2.0 { return 0.5; }
        return 0.0;
    }
    ((k/2.0-1.0)*x.ln() - x/2.0 - (k/2.0)*2.0_f64.ln() - ln_gamma(k/2.0)).exp()
}

/// χ² 分布 CDF。
pub fn chi2_cdf(x: f64, k: f64) -> f64 {
    if x <= 0.0 { return 0.0; }
    regularized_gamma_inc(k/2.0, x/2.0)
}

/// χ² 分布逆函数（Newton-Raphson）。
pub fn chi2_inv(p: f64, k: f64) -> f64 {
    if p <= 0.0 || p >= 1.0 { return f64::NAN; }
    let z = norm_inv_standard(p);
    let y = 1.0 - 2.0/(9.0*k) + z*(2.0/(9.0*k)).sqrt();
    let mut x = k * y * y * y;
    if x <= 0.0 { x = k; }
    for _ in 0..100 {
        let cdf = chi2_cdf(x, k);
        let pdf = chi2_pdf(x, k);
        if pdf < 1e-300 { break; }
        let dx = (cdf - p) / pdf;
        x -= dx;
        if x <= 0.0 { x = 1e-10; }
        if dx.abs() < 1e-10 { break; }
    }
    x
}

/// F 分布 PDF。
pub fn f_pdf(x: f64, d1: f64, d2: f64) -> f64 {
    if x < 0.0 { return 0.0; }
    if x == 0.0 {
        if d1 < 2.0 { return f64::INFINITY; }
        if d1 == 2.0 { return 1.0; }
        return 0.0;
    }
    let log_coef = (d1/2.0)*d1.ln() + (d2/2.0)*d2.ln()
        + ln_gamma((d1+d2)/2.0) - ln_gamma(d1/2.0) - ln_gamma(d2/2.0);
    let log_body = ((d1/2.0-1.0)*x.ln()) - ((d1+d2)/2.0)*(d1*x+d2).ln();
    (log_coef + log_body).exp()
}

/// F 分布 CDF。
pub fn f_cdf(x: f64, d1: f64, d2: f64) -> f64 {
    if x <= 0.0 { return 0.0; }
    let u = d1 * x / (d1 * x + d2);
    regularized_beta_inc(d1/2.0, d2/2.0, u)
}

/// F 分布逆函数（Newton-Raphson）。
pub fn f_inv(p: f64, d1: f64, d2: f64) -> f64 {
    if p <= 0.0 || p >= 1.0 { return f64::NAN; }
    let mut x = 1.0;
    for _ in 0..100 {
        let cdf = f_cdf(x, d1, d2);
        let pdf = f_pdf(x, d1, d2);
        if pdf < 1e-300 { x *= 1.5; continue; }
        let dx = (cdf - p) / pdf;
        x -= dx;
        if x <= 0.0 { x = 1e-10; }
        if dx.abs() < 1e-10 { break; }
    }
    x
}

/// 泊松分布 PMF。
pub fn poisson_pmf(k: f64, lambda: f64) -> f64 {
    if k < 0.0 || lambda <= 0.0 { return 0.0; }
    let ki = k as u64;
    if (ki as f64 - k).abs() > 1e-10 { return 0.0; }
    (ki as f64 * lambda.ln() - lambda - ln_factorial(ki)).exp()
}

/// 泊松分布 CDF。
pub fn poisson_cdf(k: f64, lambda: f64) -> f64 {
    if k < 0.0 || lambda <= 0.0 { return 0.0; }
    let ki = k.floor() as u64;
    1.0 - regularized_gamma_inc(ki as f64 + 1.0, lambda)
}

/// 二项分布 PMF。
pub fn binom_pmf(k: f64, n: f64, p: f64) -> f64 {
    if k < 0.0 || n < 0.0 || !(0.0..=1.0).contains(&p) { return 0.0; }
    let ki = k as u64;
    let ni = n as u64;
    if ki > ni { return 0.0; }
    (ln_binomial(ni, ki) + ki as f64 * p.ln() + (ni - ki) as f64 * (1.0 - p).ln()).exp()
}

/// 二项分布 CDF。
pub fn binom_cdf(k: f64, n: f64, p: f64) -> f64 {
    if k < 0.0 || n < 0.0 || !(0.0..=1.0).contains(&p) { return 0.0; }
    let ki = k.floor() as u64;
    let ni = n as u64;
    if ki >= ni { return 1.0; }
    1.0 - regularized_beta_inc(ki as f64 + 1.0, ni as f64 - ki as f64, p)
}

// ===== 检验与相关函数（原 stats_tests.rs）=====

/// 单样本 t 检验（双尾）。返回 {"t", "df", "p", "mean"}。
pub fn t_test_one(data: &[f64], mu: f64) -> HashMap<String, f64> {
    // HIGH #49 修复：n <= 1 时方差除以零，返回 NaN 结果
    if data.len() <= 1 {
        let mut result = HashMap::new();
        result.insert("t".into(), f64::NAN);
        result.insert("df".into(), f64::NAN);
        result.insert("p".into(), f64::NAN);
        result.insert("mean".into(), if data.is_empty() { f64::NAN } else { data[0] });
        return result;
    }
    let n = data.len() as f64;
    let mean = data.iter().sum::<f64>() / n;
    let var = data.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0);
    let se = (var / n).sqrt();
    let t = if se > 0.0 { (mean - mu) / se } else { 0.0 };
    let df = n - 1.0;
    let p = 2.0 * (1.0 - t_cdf(t.abs(), df));
    let mut result = HashMap::new();
    result.insert("t".into(), t);
    result.insert("df".into(), df);
    result.insert("p".into(), p);
    result.insert("mean".into(), mean);
    result
}

/// 双样本 Welch t 检验（双尾）。返回 {"t", "df", "p", "mean1", "mean2"}。
pub fn t_test_two(a: &[f64], b: &[f64]) -> HashMap<String, f64> {
    // HIGH #50 修复：n <= 1 时方差除以零
    if a.len() <= 1 || b.len() <= 1 {
        let mut result = HashMap::new();
        result.insert("t".into(), f64::NAN);
        result.insert("df".into(), f64::NAN);
        result.insert("p".into(), f64::NAN);
        result.insert("mean1".into(), mean(a));
        result.insert("mean2".into(), mean(b));
        return result;
    }
    let n1 = a.len() as f64;
    let n2 = b.len() as f64;
    let mean1 = a.iter().sum::<f64>() / n1;
    let mean2 = b.iter().sum::<f64>() / n2;
    let var1 = a.iter().map(|&x| (x - mean1).powi(2)).sum::<f64>() / (n1 - 1.0);
    let var2 = b.iter().map(|&x| (x - mean2).powi(2)).sum::<f64>() / (n2 - 1.0);
    let se = (var1/n1 + var2/n2).sqrt();
    let t = if se > 0.0 { (mean1 - mean2) / se } else { 0.0 };
    let s1_n1 = var1 / n1;
    let s2_n2 = var2 / n2;
    let df = (s1_n1 + s2_n2).powi(2) / (s1_n1.powi(2)/(n1-1.0) + s2_n2.powi(2)/(n2-1.0));
    let p = 2.0 * (1.0 - t_cdf(t.abs(), df));
    let mut result = HashMap::new();
    result.insert("t".into(), t);
    result.insert("df".into(), df);
    result.insert("p".into(), p);
    result.insert("mean1".into(), mean1);
    result.insert("mean2".into(), mean2);
    result
}

/// χ² 拟合优度检验。返回 {"chi2", "df", "p"}。
pub fn chi2_test(observed: &[f64], expected: &[f64]) -> HashMap<String, f64> {
    assert_eq!(observed.len(), expected.len(), "observed and expected must have same length");
    assert!(expected.iter().all(|&e| e > 0.0), "all expected values must be positive");
    let chi2 = observed.iter().zip(expected.iter())
        .map(|(o, e)| (o - e).powi(2) / e).sum::<f64>();
    let df = (observed.len() - 1) as f64;
    let p = 1.0 - chi2_cdf(chi2, df);
    let mut result = HashMap::new();
    result.insert("chi2".into(), chi2);
    result.insert("df".into(), df);
    result.insert("p".into(), p);
    result
}

/// Pearson 相关系数。
pub fn pearson(x: &[f64], y: &[f64]) -> f64 {
    assert_eq!(x.len(), y.len(), "x and y must have same length");
    let n = x.len() as f64;
    let mean_x = x.iter().sum::<f64>() / n;
    let mean_y = y.iter().sum::<f64>() / n;
    let mut cov = 0.0;
    let mut var_x = 0.0;
    let mut var_y = 0.0;
    for i in 0..x.len() {
        let dx = x[i] - mean_x;
        let dy = y[i] - mean_y;
        cov += dx * dy;
        var_x += dx * dx;
        var_y += dy * dy;
    }
    let denom = (var_x * var_y).sqrt();
    if denom == 0.0 { 0.0 } else { cov / denom }
}

/// Spearman 秩相关系数。
pub fn spearman(x: &[f64], y: &[f64]) -> f64 {
    assert_eq!(x.len(), y.len(), "x and y must have same length");
    pearson(&rank(x), &rank(y))
}

fn rank(data: &[f64]) -> Vec<f64> {
    let n = data.len();
    let mut indexed: Vec<(usize, f64)> = data.iter().copied().enumerate().collect();
    indexed.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    let mut ranks = vec![0.0; n];
    let mut i = 0;
    while i < n {
        let mut j = i;
        while j < n && (indexed[j].1 - indexed[i].1).abs() < 1e-15 * (indexed[i].1.abs() + indexed[j].1.abs()).max(1.0) { j += 1; }
        let avg_rank = (i + 1 + j) as f64 / 2.0;
        for k in i..j { ranks[indexed[k].0] = avg_rank; }
        i = j;
    }
    ranks
}

// ===== 回归分析 =====

/// 简单线性回归 y = slope*x + intercept，返回 (slope, intercept, r_squared)。
///
/// x.len() < 2 时返回 (0.0, 0.0, 0.0)。
pub fn linear_regression(x: &[f64], y: &[f64]) -> (f64, f64, f64) {
    let n = x.len();
    if n < 2 || n != y.len() {
        return (0.0, 0.0, 0.0);
    }
    let x_mean = mean(x);
    let y_mean = mean(y);
    let mut ss_xy = 0.0;
    let mut ss_xx = 0.0;
    let mut ss_yy = 0.0;
    for i in 0..n {
        let dx = x[i] - x_mean;
        let dy = y[i] - y_mean;
        ss_xy += dx * dy;
        ss_xx += dx * dx;
        ss_yy += dy * dy;
    }
    if ss_xx.abs() < 1e-30 {
        return (0.0, y_mean, 0.0);
    }
    let slope = ss_xy / ss_xx;
    let intercept = y_mean - slope * x_mean;
    let r_squared = if ss_yy.abs() < 1e-30 {
        1.0
    } else {
        (ss_xy * ss_xy) / (ss_xx * ss_yy)
    };
    (slope, intercept, r_squared)
}

/// 多项式回归：拟合 y = c0 + c1*x + c2*x^2 + ... + cd*x^d。
///
/// 返回 (coefficients升幂, r_squared)。构造 Vandermonde 矩阵 → (XᵀX)⁻¹Xᵀy。
pub fn polynomial_regression(
    x: &[f64],
    y: &[f64],
    degree: usize,
) -> Result<(Vec<f64>, f64), CalcError> {
    let n = x.len();
    if degree == 0 || degree >= n {
        return Err(CalcError::domain(format!(
            "polynomial_regression(): degree {} invalid for {} data points",
            degree, n
        )));
    }
    if n != y.len() {
        return Err(CalcError::domain(
            "polynomial_regression(): x and y length mismatch".to_string(),
        ));
    }
    // 构造 Vandermonde 矩阵 X (n × (degree+1))
    let cols = degree + 1;
    let data: Vec<f64> = (0..n)
        .flat_map(|i| (0..cols).map(move |j| x[i].powi(j as i32)))
        .collect();
    let x_mat = DMatrix::from_row_slice(n, cols, &data);
    let y_vec = nalgebra::DVector::from_row_slice(y);
    // β = (XᵀX)⁻¹Xᵀy
    let xtx = x_mat.transpose() * &x_mat;
    let xty = x_mat.transpose() * &y_vec;
    let xtx_inv = xtx.try_inverse().ok_or_else(|| {
        CalcError::domain("polynomial_regression(): singular matrix (XᵀX not invertible)".to_string())
    })?;
    let beta = xtx_inv * xty;
    let coeffs: Vec<f64> = beta.iter().copied().collect();
    // R²
    let y_pred = &x_mat * &beta;
    let y_mean = mean(y);
    let ss_res: f64 = y.iter().zip(y_pred.iter()).map(|(yi, pi)| (yi - pi).powi(2)).sum();
    let ss_tot: f64 = y.iter().map(|yi| (yi - y_mean).powi(2)).sum();
    let r_squared = if ss_tot.abs() < 1e-30 { 1.0 } else { 1.0 - ss_res / ss_tot };
    Ok((coeffs, r_squared))
}

/// 多元回归：y = c0 + c1*x1 + c2*x2 + ...。x 为 &[Vec<f64>]，每个内向量是一个特征。
///
/// 返回 (coefficients含截距, r_squared)。
pub fn multiple_regression(
    x: &[Vec<f64>],
    y: &[f64],
) -> Result<(Vec<f64>, f64), CalcError> {
    if x.is_empty() || y.is_empty() {
        return Err(CalcError::domain(
            "multiple_regression(): empty input".to_string(),
        ));
    }
    let n = y.len();
    let p = x.len(); // 特征数
    if x.iter().any(|xi| xi.len() != n) {
        return Err(CalcError::domain(
            "multiple_regression(): feature vector length mismatch".to_string(),
        ));
    }
    if p >= n {
        return Err(CalcError::domain(
            "multiple_regression(): underdetermined system (features >= samples)".to_string(),
        ));
    }
    // 构造设计矩阵 X (n × (p+1))，第一列为 1（截距）
    let cols = p + 1;
    let data: Vec<f64> = (0..n)
        .flat_map(|i| {
            std::iter::once(1.0).chain((0..p).map(move |j| x[j][i]))
        })
        .collect();
    let x_mat = DMatrix::from_row_slice(n, cols, &data);
    let y_vec = nalgebra::DVector::from_row_slice(y);
    // β = (XᵀX)⁻¹Xᵀy
    let xtx = x_mat.transpose() * &x_mat;
    let xty = x_mat.transpose() * &y_vec;
    let xtx_inv = xtx.try_inverse().ok_or_else(|| {
        CalcError::domain("multiple_regression(): singular matrix".to_string())
    })?;
    let beta = xtx_inv * xty;
    let coeffs: Vec<f64> = beta.iter().copied().collect();
    // R²
    let y_pred = &x_mat * &beta;
    let y_mean = mean(y);
    let ss_res: f64 = y.iter().zip(y_pred.iter()).map(|(yi, pi)| (yi - pi).powi(2)).sum();
    let ss_tot: f64 = y.iter().map(|yi| (yi - y_mean).powi(2)).sum();
    let r_squared = if ss_tot.abs() < 1e-30 { 1.0 } else { 1.0 - ss_res / ss_tot };
    Ok((coeffs, r_squared))
}

// ===== 测试 =====

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

    // --- 基础统计 ---

    #[test]
    fn test_mean_basic() {
        assert_approx(mean(&[1.0, 2.0, 3.0, 4.0, 5.0]), 3.0, 1e-15, "mean");
    }

    #[test]
    fn test_mean_single() {
        assert_approx(mean(&[42.0]), 42.0, 1e-15, "mean single");
    }

    #[test]
    fn test_variance_basic() {
        assert_approx(variance(&[1.0, 2.0, 3.0, 4.0, 5.0]), 2.0, 1e-15, "variance");
    }

    #[test]
    fn test_variance_identical() {
        assert_approx(variance(&[3.0, 3.0, 3.0]), 0.0, 1e-15, "variance identical");
    }

    #[test]
    fn test_std_basic() {
        assert_approx(std(&[1.0, 2.0, 3.0, 4.0, 5.0]), 2.0_f64.sqrt(), 1e-10, "std");
    }

    #[test]
    fn test_median_odd() {
        assert_approx(median(&[1.0, 2.0, 3.0, 4.0, 5.0]), 3.0, 1e-15, "median odd");
    }

    #[test]
    fn test_median_even() {
        assert_approx(median(&[1.0, 2.0, 3.0, 4.0]), 2.5, 1e-15, "median even");
    }

    #[test]
    fn test_min_max() {
        assert_approx(min(&[3.0, 1.0, 4.0, 1.0, 5.0]), 1.0, 1e-15, "min");
        assert_approx(max(&[3.0, 1.0, 4.0, 1.0, 5.0]), 5.0, 1e-15, "max");
    }

    #[test]
    fn test_sum_count() {
        assert_approx(sum(&[1.0, 2.0, 3.0, 4.0, 5.0]), 15.0, 1e-15, "sum");
        assert_approx(count(&[1.0, 2.0, 3.0, 4.0, 5.0]), 5.0, 1e-15, "count");
    }

    // --- 特殊函数 ---

    #[test]
    fn test_ln_gamma_1() {
        assert_approx(ln_gamma(1.0), 0.0, 1e-10, "ln_gamma(1)");
    }

    #[test]
    fn test_ln_gamma_10() {
        assert_approx(ln_gamma(10.0), 12.801827480, 1e-6, "ln_gamma(10)");
    }

    #[test]
    fn test_ln_gamma_half() {
        assert_approx(ln_gamma(0.5), 0.5723649429, 1e-6, "ln_gamma(0.5)");
    }

    #[test]
    fn test_gamma_inc_1_1() {
        assert_approx(regularized_gamma_inc(1.0, 1.0), 0.6321205588, 1e-6, "P(1,1)");
    }

    #[test]
    fn test_gamma_inc_zero_x() {
        assert_approx(regularized_gamma_inc(2.0, 0.0), 0.0, 1e-15, "P(2,0)");
    }

    #[test]
    fn test_beta_inc_half() {
        assert_approx(regularized_beta_inc(1.0, 1.0, 0.5), 0.5, 1e-10, "I_0.5(1,1)");
    }

    #[test]
    fn test_beta_inc_boundary() {
        assert_approx(regularized_beta_inc(2.0, 3.0, 0.0), 0.0, 1e-15, "I_0(2,3)");
        assert_approx(regularized_beta_inc(2.0, 3.0, 1.0), 1.0, 1e-15, "I_1(2,3)");
    }

    #[test]
    fn test_ln_factorial() {
        assert_approx(ln_factorial(0), 0.0, 1e-15, "ln_factorial(0)");
        assert_approx(ln_factorial(5), 120.0_f64.ln(), 1e-10, "ln_factorial(5)");
    }

    #[test]
    fn test_ln_binomial() {
        assert_approx(ln_binomial(10, 3), 120.0_f64.ln(), 1e-10, "ln_binomial(10,3)");
        assert!(ln_binomial(3, 10).is_nan());
    }

    // --- 分布函数 ---

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
        assert_approx(norm_cdf(1.96, 0.0, 1.0), 0.975, 1e-3, "norm_cdf(1.96)");
    }

    #[test]
    fn test_norm_inv_half() {
        assert_approx(norm_inv(0.5, 0.0, 1.0), 0.0, 1e-6, "norm_inv(0.5)");
    }

    #[test]
    fn test_norm_inv_975() {
        assert_approx(norm_inv(0.975, 0.0, 1.0), 1.96, 1e-2, "norm_inv(0.975)");
    }

    #[test]
    fn test_t_pdf_zero() {
        assert_approx(t_pdf(0.0, 10.0), 0.3891, 1e-3, "t_pdf(0,10)");
    }

    #[test]
    fn test_t_cdf_zero() {
        assert_approx(t_cdf(0.0, 10.0), 0.5, 1e-10, "t_cdf(0,10)");
    }

    #[test]
    fn test_t_inv_half() {
        assert_approx(t_inv(0.5, 10.0), 0.0, 1e-6, "t_inv(0.5,10)");
    }

    #[test]
    fn test_chi2_cdf() {
        assert_approx(chi2_cdf(5.991, 5.0), 0.6929, 1e-3, "chi2_cdf(5.991,5)");
    }

    #[test]
    fn test_chi2_inv() {
        assert_approx(chi2_inv(0.95, 5.0), 11.070, 1e-1, "chi2_inv(0.95,5)");
    }

    #[test]
    fn test_f_cdf() {
        assert_approx(f_cdf(4.24, 5.0, 10.0), 0.9751, 1e-3, "f_cdf(4.24,5,10)");
    }

    #[test]
    fn test_poisson() {
        assert_approx(poisson_pmf(3.0, 2.0), 0.1804, 1e-3, "poisson_pmf(3,2)");
        assert_approx(poisson_cdf(3.0, 2.0), 0.8571, 1e-3, "poisson_cdf(3,2)");
    }

    #[test]
    fn test_binom() {
        assert_approx(binom_pmf(3.0, 10.0, 0.5), 0.1172, 1e-3, "binom_pmf(3,10,0.5)");
        assert_approx(binom_cdf(5.0, 10.0, 0.5), 0.6230, 1e-3, "binom_cdf(5,10,0.5)");
    }

    // --- 检验与相关 ---

    #[test]
    fn test_t_test_one_at_mean() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let r = t_test_one(&data, 3.0);
        assert_approx(*r.get("t").unwrap(), 0.0, 1e-10, "t_test_one t");
        assert_approx(*r.get("p").unwrap(), 1.0, 1e-10, "t_test_one p");
    }

    #[test]
    fn test_t_test_two_identical() {
        let a = vec![1.0, 2.0, 3.0];
        let r = t_test_two(&a, &a);
        assert_approx(*r.get("t").unwrap(), 0.0, 1e-10, "t_test_two t");
        assert_approx(*r.get("p").unwrap(), 1.0, 1e-10, "t_test_two p");
    }

    #[test]
    fn test_chi2_test_perfect() {
        let o = vec![10.0, 20.0, 30.0];
        let e = vec![10.0, 20.0, 30.0];
        let r = chi2_test(&o, &e);
        assert_approx(*r.get("chi2").unwrap(), 0.0, 1e-15, "chi2 perfect");
        assert_approx(*r.get("p").unwrap(), 1.0, 1e-10, "chi2 perfect p");
    }

    #[test]
    fn test_pearson_perfect() {
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let y = vec![2.0, 4.0, 6.0, 8.0, 10.0];
        assert_approx(pearson(&x, &y), 1.0, 1e-10, "pearson");
    }

    #[test]
    fn test_spearman_perfect() {
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let y = vec![5.0, 4.0, 3.0, 2.0, 1.0];
        assert_approx(spearman(&x, &y), -1.0, 1e-10, "spearman");
    }

    #[test]
    fn test_rank_with_ties() {
        let data = vec![10.0, 20.0, 20.0, 40.0];
        let r = rank(&data);
        assert_approx(r[0], 1.0, 1e-15, "rank[0]");
        assert_approx(r[1], 2.5, 1e-15, "rank[1]");
        assert_approx(r[2], 2.5, 1e-15, "rank[2]");
        assert_approx(r[3], 4.0, 1e-15, "rank[3]");
    }

    // ===== 回归分析 =====

    #[test]
    fn test_linear_regression_perfect() {
        let x = vec![1.0, 2.0, 3.0];
        let y = vec![2.0, 4.0, 6.0];
        let (slope, intercept, r_sq) = linear_regression(&x, &y);
        assert_approx(slope, 2.0, 1e-12, "slope");
        assert_approx(intercept, 0.0, 1e-12, "intercept");
        assert_approx(r_sq, 1.0, 1e-12, "r_squared");
    }

    #[test]
    fn test_linear_regression_short() {
        let (s, i, r) = linear_regression(&[1.0], &[2.0]);
        assert_eq!((s, i, r), (0.0, 0.0, 0.0));
    }

    #[test]
    fn test_polynomial_regression_quadratic() {
        let x = vec![0.0, 1.0, 2.0, 3.0, 4.0];
        let y = vec![1.0, 0.0, 1.0, 4.0, 9.0]; // (x-1)² = x²-2x+1
        let (coeffs, r_sq) = polynomial_regression(&x, &y, 2).unwrap();
        assert_approx(coeffs[0], 1.0, 1e-9, "c0");
        assert_approx(coeffs[1], -2.0, 1e-9, "c1");
        assert_approx(coeffs[2], 1.0, 1e-9, "c2");
        assert!(r_sq > 0.99, "r_squared too low: {}", r_sq);
    }

    #[test]
    fn test_polynomial_regression_invalid_degree() {
        assert!(polynomial_regression(&[1.0, 2.0], &[1.0, 2.0], 0).is_err());
        assert!(polynomial_regression(&[1.0, 2.0], &[1.0, 2.0], 2).is_err());
    }

    #[test]
    fn test_multiple_regression_basic() {
        // y = 1 + 2*x1 + 3*x2
        let x1 = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let x2 = vec![2.0, 1.0, 3.0, 2.0, 4.0];
        let y: Vec<f64> = x1.iter().zip(x2.iter())
            .map(|(a, b)| 1.0 + 2.0 * a + 3.0 * b).collect();
        let (coeffs, r_sq) = multiple_regression(&[x1, x2], &y).unwrap();
        assert_approx(coeffs[0], 1.0, 1e-9, "intercept");
        assert_approx(coeffs[1], 2.0, 1e-9, "c1");
        assert_approx(coeffs[2], 3.0, 1e-9, "c2");
        assert_approx(r_sq, 1.0, 1e-9, "r_squared");
    }

    #[test]
    fn test_multiple_regression_empty() {
        assert!(multiple_regression(&[], &[]).is_err());
    }
}
