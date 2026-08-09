// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! API trait 定义：5 个分组 trait。

use crate::api::types::{BigNumber, Complex, Matrix, Polynomial, Vector};
use crate::core::{CalcError, EvalResult};

/// 标量数学 trait：算术 + 科学函数 + 精度 + 数论 + 组合。
pub trait ScalarMath {
    // ── 算术 ──
    fn add(&self, a: f64, b: f64) -> Result<EvalResult, CalcError>;
    fn sub(&self, a: f64, b: f64) -> Result<EvalResult, CalcError>;
    fn mul(&self, a: f64, b: f64) -> Result<EvalResult, CalcError>;
    fn div(&self, a: f64, b: f64) -> Result<EvalResult, CalcError>;
    fn pow(&self, a: f64, b: f64) -> Result<EvalResult, CalcError>;
    fn rem(&self, a: f64, b: f64) -> Result<EvalResult, CalcError>;
    fn factorial(&self, n: u64) -> Result<EvalResult, CalcError>;
    fn abs(&self, x: f64) -> Result<EvalResult, CalcError>;

    // ── 科学函数 ──
    fn sin(&self, x: f64) -> Result<EvalResult, CalcError>;
    fn cos(&self, x: f64) -> Result<EvalResult, CalcError>;
    fn tan(&self, x: f64) -> Result<EvalResult, CalcError>;
    fn asin(&self, x: f64) -> Result<EvalResult, CalcError>;
    fn acos(&self, x: f64) -> Result<EvalResult, CalcError>;
    fn atan(&self, x: f64) -> Result<EvalResult, CalcError>;
    fn ln(&self, x: f64) -> Result<EvalResult, CalcError>;
    fn log(&self, x: f64, base: f64) -> Result<EvalResult, CalcError>;
    fn exp(&self, x: f64) -> Result<EvalResult, CalcError>;
    fn sinh(&self, x: f64) -> Result<EvalResult, CalcError>;
    fn cosh(&self, x: f64) -> Result<EvalResult, CalcError>;
    fn tanh(&self, x: f64) -> Result<EvalResult, CalcError>;
    fn gamma(&self, x: f64) -> Result<EvalResult, CalcError>;
    fn erf(&self, x: f64) -> Result<EvalResult, CalcError>;

    // ── 精度 ──
    fn precision_eval(&self, digits: usize, expr: &str) -> Result<EvalResult, CalcError>;

    // ── 数论 ──
    fn gcd(&self, a: &BigNumber, b: &BigNumber) -> Result<EvalResult, CalcError>;
    fn lcm(&self, a: &BigNumber, b: &BigNumber) -> Result<EvalResult, CalcError>;
    fn is_prime(&self, n: &BigNumber) -> Result<EvalResult, CalcError>;
    fn prime_sieve(&self, n: u64) -> Result<EvalResult, CalcError>;
    fn mod_pow(&self, base: &BigNumber, exp: &BigNumber, m: &BigNumber) -> Result<EvalResult, CalcError>;
    fn mod_inverse(&self, a: &BigNumber, m: &BigNumber) -> Result<EvalResult, CalcError>;
    fn euler_phi(&self, n: &BigNumber) -> Result<EvalResult, CalcError>;

    // ── 组合 ──
    fn perm(&self, n: u64, k: u64) -> Result<EvalResult, CalcError>;
    fn comb(&self, n: u64, k: u64) -> Result<EvalResult, CalcError>;
    fn catalan(&self, n: u64) -> Result<EvalResult, CalcError>;
    fn stirling_first(&self, n: u64, k: u64) -> Result<EvalResult, CalcError>;
    fn stirling_second(&self, n: u64, k: u64) -> Result<EvalResult, CalcError>;
}

/// 线性代数 trait：矩阵 + 向量。
pub trait LinearAlgebra {
    // ── 矩阵 ──
    fn det(&self, m: &Matrix) -> Result<EvalResult, CalcError>;
    fn inverse(&self, m: &Matrix) -> Result<EvalResult, CalcError>;
    fn transpose(&self, m: &Matrix) -> Result<EvalResult, CalcError>;
    fn identity(&self, n: usize) -> Result<EvalResult, CalcError>;
    fn mat_add(&self, a: &Matrix, b: &Matrix) -> Result<EvalResult, CalcError>;
    fn mat_sub(&self, a: &Matrix, b: &Matrix) -> Result<EvalResult, CalcError>;
    fn mat_mul(&self, a: &Matrix, b: &Matrix) -> Result<EvalResult, CalcError>;
    fn scalar_mul(&self, s: f64, m: &Matrix) -> Result<EvalResult, CalcError>;

    // ── 向量 ──
    fn dot(&self, a: &Vector, b: &Vector) -> Result<EvalResult, CalcError>;
    fn cross(&self, a: &Vector, b: &Vector) -> Result<EvalResult, CalcError>;
    fn normalize(&self, a: &Vector) -> Result<EvalResult, CalcError>;
    fn magnitude(&self, a: &Vector) -> Result<EvalResult, CalcError>;
    fn vector_add(&self, a: &Vector, b: &Vector) -> Result<EvalResult, CalcError>;
    fn vector_sub(&self, a: &Vector, b: &Vector) -> Result<EvalResult, CalcError>;
}

/// 数据分析 trait：统计。
pub trait DataAnalysis {
    // ── 基础统计 ──
    fn mean(&self, data: &[f64]) -> Result<EvalResult, CalcError>;
    fn variance(&self, data: &[f64]) -> Result<EvalResult, CalcError>;
    fn std(&self, data: &[f64]) -> Result<EvalResult, CalcError>;
    fn median(&self, data: &[f64]) -> Result<EvalResult, CalcError>;
    fn min(&self, data: &[f64]) -> Result<EvalResult, CalcError>;
    fn max(&self, data: &[f64]) -> Result<EvalResult, CalcError>;
    fn sum(&self, data: &[f64]) -> Result<EvalResult, CalcError>;
    fn count(&self, data: &[f64]) -> Result<EvalResult, CalcError>;

    // ── 分布函数 ──
    fn norm_pdf(&self, x: f64, mu: f64, sigma: f64) -> Result<EvalResult, CalcError>;
    fn norm_cdf(&self, x: f64, mu: f64, sigma: f64) -> Result<EvalResult, CalcError>;
    fn norm_inv(&self, p: f64, mu: f64, sigma: f64) -> Result<EvalResult, CalcError>;
    fn t_pdf(&self, x: f64, df: f64) -> Result<EvalResult, CalcError>;
    fn t_cdf(&self, x: f64, df: f64) -> Result<EvalResult, CalcError>;
    fn t_inv(&self, p: f64, df: f64) -> Result<EvalResult, CalcError>;
    fn chi2_pdf(&self, x: f64, k: f64) -> Result<EvalResult, CalcError>;
    fn chi2_cdf(&self, x: f64, k: f64) -> Result<EvalResult, CalcError>;
    fn chi2_inv(&self, p: f64, k: f64) -> Result<EvalResult, CalcError>;
    fn f_pdf(&self, x: f64, d1: f64, d2: f64) -> Result<EvalResult, CalcError>;
    fn f_cdf(&self, x: f64, d1: f64, d2: f64) -> Result<EvalResult, CalcError>;
    fn f_inv(&self, p: f64, d1: f64, d2: f64) -> Result<EvalResult, CalcError>;
    fn poisson_pmf(&self, k: f64, lambda: f64) -> Result<EvalResult, CalcError>;
    fn poisson_cdf(&self, k: f64, lambda: f64) -> Result<EvalResult, CalcError>;
    fn binom_pmf(&self, k: f64, n: f64, p: f64) -> Result<EvalResult, CalcError>;
    fn binom_cdf(&self, k: f64, n: f64, p: f64) -> Result<EvalResult, CalcError>;

    // ── 假设检验 ──
    fn t_test_one(&self, data: &[f64], mu: f64) -> Result<EvalResult, CalcError>;
    fn t_test_two(&self, a: &[f64], b: &[f64]) -> Result<EvalResult, CalcError>;
    fn chi2_test(&self, observed: &[f64], expected: &[f64]) -> Result<EvalResult, CalcError>;

    // ── 相关 ──
    fn pearson(&self, x: &[f64], y: &[f64]) -> Result<EvalResult, CalcError>;
    fn spearman(&self, x: &[f64], y: &[f64]) -> Result<EvalResult, CalcError>;
}

/// 符号数学 trait：符号演算 + 多项式 + 复数。
pub trait SymbolicMath {
    // ── 符号演算 ──
    fn differentiate(&self, expr: &str, var: &str) -> Result<EvalResult, CalcError>;
    fn integrate(&self, expr: &str, var: &str) -> Result<EvalResult, CalcError>;
    fn simplify(&self, expr: &str) -> Result<EvalResult, CalcError>;
    fn limit(&self, expr: &str, var: &str, target: f64) -> Result<EvalResult, CalcError>;
    fn taylor_expand(&self, expr: &str, var: &str, center: f64, order: usize) -> Result<EvalResult, CalcError>;

    // ── 多项式 ──
    fn poly_add(&self, a: &Polynomial, b: &Polynomial) -> Result<EvalResult, CalcError>;
    fn poly_sub(&self, a: &Polynomial, b: &Polynomial) -> Result<EvalResult, CalcError>;
    fn poly_mul(&self, a: &Polynomial, b: &Polynomial) -> Result<EvalResult, CalcError>;
    fn poly_div(&self, a: &Polynomial, b: &Polynomial) -> Result<EvalResult, CalcError>;
    fn poly_roots(&self, p: &Polynomial) -> Result<EvalResult, CalcError>;
    fn poly_eval(&self, p: &Polynomial, x: f64) -> Result<EvalResult, CalcError>;

    // ── 复数 ──
    fn complex_add(&self, a: &Complex, b: &Complex) -> Result<EvalResult, CalcError>;
    fn complex_sub(&self, a: &Complex, b: &Complex) -> Result<EvalResult, CalcError>;
    fn complex_mul(&self, a: &Complex, b: &Complex) -> Result<EvalResult, CalcError>;
    fn complex_div(&self, a: &Complex, b: &Complex) -> Result<EvalResult, CalcError>;
    fn complex_abs(&self, z: &Complex) -> Result<EvalResult, CalcError>;
    fn complex_arg(&self, z: &Complex) -> Result<EvalResult, CalcError>;
    fn complex_conj(&self, z: &Complex) -> Result<EvalResult, CalcError>;
    fn complex_exp(&self, z: &Complex) -> Result<EvalResult, CalcError>;
    fn complex_ln(&self, z: &Complex) -> Result<EvalResult, CalcError>;
}

/// 应用数学 trait：时间 + 单位 + 汇率（feature-gated）。
pub trait AppliedMath {
    // ── 时间：构造 ──
    #[cfg(feature = "time")]
    fn date(&self, date_str: &str) -> Result<EvalResult, CalcError>;
    #[cfg(feature = "time")]
    fn datetime(&self, datetime_str: &str, tz: Option<&str>) -> Result<EvalResult, CalcError>;
    #[cfg(feature = "time")]
    fn timestamp(&self, datetime_str: &str) -> Result<EvalResult, CalcError>;
    #[cfg(feature = "time")]
    fn from_timestamp(&self, secs: i64, tz: Option<&str>) -> Result<EvalResult, CalcError>;

    // ── 时间：算术 ──
    #[cfg(feature = "time")]
    fn now(&self, tz: Option<&str>) -> Result<EvalResult, CalcError>;
    #[cfg(feature = "time")]
    fn today(&self, tz: Option<&str>) -> Result<EvalResult, CalcError>;
    #[cfg(feature = "time")]
    fn date_add(&self, date: &str, n: i64, unit: &str) -> Result<EvalResult, CalcError>;
    #[cfg(feature = "time")]
    fn date_diff(&self, a: &str, b: &str, unit: Option<&str>) -> Result<EvalResult, CalcError>;

    // ── 时间：格式与日历 ──
    #[cfg(feature = "time")]
    fn format_date(&self, date: &str, fmt: &str, tz: Option<&str>) -> Result<EvalResult, CalcError>;
    #[cfg(feature = "time")]
    fn reformat_date(&self, input: &str, from_fmt: &str, to_fmt: &str) -> Result<EvalResult, CalcError>;
    #[cfg(feature = "time")]
    fn weekday(&self, date: &str) -> Result<EvalResult, CalcError>;
    #[cfg(feature = "time")]
    fn day_of_year(&self, date: &str) -> Result<EvalResult, CalcError>;
    #[cfg(feature = "time")]
    fn is_leap_year(&self, year: i64) -> Result<EvalResult, CalcError>;

    // ── 单位 ──
    #[cfg(feature = "unit")]
    fn convert(&self, value: f64, from: &str, to: &str) -> Result<EvalResult, CalcError>;

    // ── 汇率 ──
    #[cfg(feature = "fx")]
    fn fx(&self, amount: f64, from: &str, to: &str) -> Result<EvalResult, CalcError>;
    #[cfg(feature = "fx")]
    fn fx_rate(&self, from: &str, to: &str) -> Result<EvalResult, CalcError>;
}
