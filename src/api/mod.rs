// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! 直接 API 层（L4b）。
//!
//! 提供 `CalNexus` 门面结构体 + 5 个分组 trait，绕过表达式解析直接调用 math 层函数。
//!
//! 依赖方向：`api/` → `math/` → `core/`，不依赖 `domains/`。

pub mod cache;
pub mod traits;
pub mod types;

mod applied;
mod linalg;
mod scalar;
mod stats;
mod symbolic_api;

pub use self::scalar::ScalarMathImpl;
pub use self::linalg::LinearAlgebraImpl;
pub use self::stats::DataAnalysisImpl;
pub use self::symbolic_api::SymbolicMathImpl;
pub use self::applied::AppliedMathImpl;

// Re-export traits for external use.
pub use self::traits::{AppliedMath, DataAnalysis, LinearAlgebra, ScalarMath, SymbolicMath};

use crate::core::{CalcError, EvalContext};
use crate::api::types::{BigNumber, Complex, Matrix, Polynomial, Vector};
use std::sync::RwLock;

/// CalNexus 门面结构体：直接 API 入口。
///
/// 持有缓存管理器 + 变量上下文，提供 5 个分组 trait 访问器。
pub struct CalNexus {
    ctx: RwLock<EvalContext>,
}

impl CalNexus {
    /// 创建默认实例（空上下文）。
    pub fn new() -> Self {
        Self {
            ctx: RwLock::new(EvalContext::new()),
        }
    }

    /// 设置变量。
    pub fn set_var(&self, name: &str, value: f64) {
        let mut ctx = self.ctx.write().unwrap();
        *ctx = ctx.clone().with_var(name, value);
    }

    /// 获取变量。
    pub fn get_var(&self, name: &str) -> Option<f64> {
        let ctx = self.ctx.read().unwrap();
        ctx.get_var(name)
    }

    /// 清空所有变量。
    pub fn clear_vars(&self) {
        let mut ctx = self.ctx.write().unwrap();
        *ctx = EvalContext::new();
    }

    /// 获取标量数学 API。
    pub fn scalar(&self) -> ScalarMathImpl<'_> {
        ScalarMathImpl { cn: self }
    }

    /// 获取线性代数 API。
    pub fn linalg(&self) -> LinearAlgebraImpl<'_> {
        LinearAlgebraImpl { cn: self }
    }

    /// 获取数据分析 API。
    pub fn stats(&self) -> DataAnalysisImpl<'_> {
        DataAnalysisImpl { cn: self }
    }

    /// 获取符号数学 API。
    pub fn symbolic(&self) -> SymbolicMathImpl<'_> {
        SymbolicMathImpl { cn: self }
    }

    /// 获取应用数学 API（时间/单位/汇率）。
    pub fn applied(&self) -> AppliedMathImpl<'_> {
        AppliedMathImpl { cn: self }
    }
}

impl Default for CalNexus {
    fn default() -> Self {
        Self::new()
    }
}

// ── Trait 实现委托 ──

impl ScalarMath for ScalarMathImpl<'_> {
    fn add(&self, a: f64, b: f64) -> Result<crate::core::EvalResult, CalcError> { ScalarMathImpl::add(self, a, b) }
    fn sub(&self, a: f64, b: f64) -> Result<crate::core::EvalResult, CalcError> { ScalarMathImpl::sub(self, a, b) }
    fn mul(&self, a: f64, b: f64) -> Result<crate::core::EvalResult, CalcError> { ScalarMathImpl::mul(self, a, b) }
    fn div(&self, a: f64, b: f64) -> Result<crate::core::EvalResult, CalcError> { ScalarMathImpl::div(self, a, b) }
    fn pow(&self, a: f64, b: f64) -> Result<crate::core::EvalResult, CalcError> { ScalarMathImpl::pow(self, a, b) }
    fn rem(&self, a: f64, b: f64) -> Result<crate::core::EvalResult, CalcError> { ScalarMathImpl::rem(self, a, b) }
    fn factorial(&self, n: u64) -> Result<crate::core::EvalResult, CalcError> { ScalarMathImpl::factorial(self, n) }
    fn abs(&self, x: f64) -> Result<crate::core::EvalResult, CalcError> { ScalarMathImpl::abs(self, x) }
    fn sin(&self, x: f64) -> Result<crate::core::EvalResult, CalcError> { ScalarMathImpl::sin(self, x) }
    fn cos(&self, x: f64) -> Result<crate::core::EvalResult, CalcError> { ScalarMathImpl::cos(self, x) }
    fn tan(&self, x: f64) -> Result<crate::core::EvalResult, CalcError> { ScalarMathImpl::tan(self, x) }
    fn asin(&self, x: f64) -> Result<crate::core::EvalResult, CalcError> { ScalarMathImpl::asin(self, x) }
    fn acos(&self, x: f64) -> Result<crate::core::EvalResult, CalcError> { ScalarMathImpl::acos(self, x) }
    fn atan(&self, x: f64) -> Result<crate::core::EvalResult, CalcError> { ScalarMathImpl::atan(self, x) }
    fn ln(&self, x: f64) -> Result<crate::core::EvalResult, CalcError> { ScalarMathImpl::ln(self, x) }
    fn log(&self, x: f64, base: f64) -> Result<crate::core::EvalResult, CalcError> { ScalarMathImpl::log(self, x, base) }
    fn exp(&self, x: f64) -> Result<crate::core::EvalResult, CalcError> { ScalarMathImpl::exp(self, x) }
    fn sinh(&self, x: f64) -> Result<crate::core::EvalResult, CalcError> { ScalarMathImpl::sinh(self, x) }
    fn cosh(&self, x: f64) -> Result<crate::core::EvalResult, CalcError> { ScalarMathImpl::cosh(self, x) }
    fn tanh(&self, x: f64) -> Result<crate::core::EvalResult, CalcError> { ScalarMathImpl::tanh(self, x) }
    fn gamma(&self, x: f64) -> Result<crate::core::EvalResult, CalcError> { ScalarMathImpl::gamma(self, x) }
    fn erf(&self, x: f64) -> Result<crate::core::EvalResult, CalcError> { ScalarMathImpl::erf(self, x) }
    fn precision_eval(&self, digits: usize, expr: &str) -> Result<crate::core::EvalResult, CalcError> { ScalarMathImpl::precision_eval(self, digits, expr) }
    fn gcd(&self, a: &BigNumber, b: &BigNumber) -> Result<crate::core::EvalResult, CalcError> { ScalarMathImpl::gcd(self, a, b) }
    fn lcm(&self, a: &BigNumber, b: &BigNumber) -> Result<crate::core::EvalResult, CalcError> { ScalarMathImpl::lcm(self, a, b) }
    fn is_prime(&self, n: &BigNumber) -> Result<crate::core::EvalResult, CalcError> { ScalarMathImpl::is_prime(self, n) }
    fn prime_sieve(&self, n: u64) -> Result<crate::core::EvalResult, CalcError> { ScalarMathImpl::prime_sieve(self, n) }
    fn mod_pow(&self, base: &BigNumber, exp: &BigNumber, m: &BigNumber) -> Result<crate::core::EvalResult, CalcError> { ScalarMathImpl::mod_pow(self, base, exp, m) }
    fn mod_inverse(&self, a: &BigNumber, m: &BigNumber) -> Result<crate::core::EvalResult, CalcError> { ScalarMathImpl::mod_inverse(self, a, m) }
    fn euler_phi(&self, n: &BigNumber) -> Result<crate::core::EvalResult, CalcError> { ScalarMathImpl::euler_phi(self, n) }
    fn perm(&self, n: u64, k: u64) -> Result<crate::core::EvalResult, CalcError> { ScalarMathImpl::perm(self, n, k) }
    fn comb(&self, n: u64, k: u64) -> Result<crate::core::EvalResult, CalcError> { ScalarMathImpl::comb(self, n, k) }
    fn catalan(&self, n: u64) -> Result<crate::core::EvalResult, CalcError> { ScalarMathImpl::catalan(self, n) }
    fn stirling_first(&self, n: u64, k: u64) -> Result<crate::core::EvalResult, CalcError> { ScalarMathImpl::stirling_first(self, n, k) }
    fn stirling_second(&self, n: u64, k: u64) -> Result<crate::core::EvalResult, CalcError> { ScalarMathImpl::stirling_second(self, n, k) }
}

impl LinearAlgebra for LinearAlgebraImpl<'_> {
    fn det(&self, m: &Matrix) -> Result<crate::core::EvalResult, CalcError> { LinearAlgebraImpl::det(self, m) }
    fn inverse(&self, m: &Matrix) -> Result<crate::core::EvalResult, CalcError> { LinearAlgebraImpl::inverse(self, m) }
    fn transpose(&self, m: &Matrix) -> Result<crate::core::EvalResult, CalcError> { LinearAlgebraImpl::transpose(self, m) }
    fn identity(&self, n: usize) -> Result<crate::core::EvalResult, CalcError> { LinearAlgebraImpl::identity(self, n) }
    fn mat_add(&self, a: &Matrix, b: &Matrix) -> Result<crate::core::EvalResult, CalcError> { LinearAlgebraImpl::mat_add(self, a, b) }
    fn mat_sub(&self, a: &Matrix, b: &Matrix) -> Result<crate::core::EvalResult, CalcError> { LinearAlgebraImpl::mat_sub(self, a, b) }
    fn mat_mul(&self, a: &Matrix, b: &Matrix) -> Result<crate::core::EvalResult, CalcError> { LinearAlgebraImpl::mat_mul(self, a, b) }
    fn scalar_mul(&self, s: f64, m: &Matrix) -> Result<crate::core::EvalResult, CalcError> { LinearAlgebraImpl::scalar_mul(self, s, m) }
    fn dot(&self, a: &Vector, b: &Vector) -> Result<crate::core::EvalResult, CalcError> { LinearAlgebraImpl::dot(self, a, b) }
    fn cross(&self, a: &Vector, b: &Vector) -> Result<crate::core::EvalResult, CalcError> { LinearAlgebraImpl::cross(self, a, b) }
    fn normalize(&self, a: &Vector) -> Result<crate::core::EvalResult, CalcError> { LinearAlgebraImpl::normalize(self, a) }
    fn magnitude(&self, a: &Vector) -> Result<crate::core::EvalResult, CalcError> { LinearAlgebraImpl::magnitude(self, a) }
    fn vector_add(&self, a: &Vector, b: &Vector) -> Result<crate::core::EvalResult, CalcError> { LinearAlgebraImpl::vector_add(self, a, b) }
    fn vector_sub(&self, a: &Vector, b: &Vector) -> Result<crate::core::EvalResult, CalcError> { LinearAlgebraImpl::vector_sub(self, a, b) }
    #[cfg(feature = "numerical")]
    fn eig(&self, m: &Matrix) -> Result<crate::core::EvalResult, CalcError> { LinearAlgebraImpl::eig(self, m) }
    #[cfg(feature = "numerical")]
    fn svd(&self, m: &Matrix) -> Result<crate::core::EvalResult, CalcError> { LinearAlgebraImpl::svd(self, m) }
    #[cfg(feature = "numerical")]
    fn lu(&self, m: &Matrix) -> Result<crate::core::EvalResult, CalcError> { LinearAlgebraImpl::lu(self, m) }
    #[cfg(feature = "numerical")]
    fn qr(&self, m: &Matrix) -> Result<crate::core::EvalResult, CalcError> { LinearAlgebraImpl::qr(self, m) }
    #[cfg(feature = "numerical")]
    fn solve(&self, a: &Matrix, b: &Vector) -> Result<crate::core::EvalResult, CalcError> { LinearAlgebraImpl::solve(self, a, b) }
    #[cfg(feature = "numerical")]
    fn matrix_exp(&self, m: &Matrix) -> Result<crate::core::EvalResult, CalcError> { LinearAlgebraImpl::matrix_exp(self, m) }
}

impl DataAnalysis for DataAnalysisImpl<'_> {
    fn mean(&self, data: &[f64]) -> Result<crate::core::EvalResult, CalcError> { DataAnalysisImpl::mean(self, data) }
    fn variance(&self, data: &[f64]) -> Result<crate::core::EvalResult, CalcError> { DataAnalysisImpl::variance(self, data) }
    fn std(&self, data: &[f64]) -> Result<crate::core::EvalResult, CalcError> { DataAnalysisImpl::std(self, data) }
    fn median(&self, data: &[f64]) -> Result<crate::core::EvalResult, CalcError> { DataAnalysisImpl::median(self, data) }
    fn min(&self, data: &[f64]) -> Result<crate::core::EvalResult, CalcError> { DataAnalysisImpl::min(self, data) }
    fn max(&self, data: &[f64]) -> Result<crate::core::EvalResult, CalcError> { DataAnalysisImpl::max(self, data) }
    fn sum(&self, data: &[f64]) -> Result<crate::core::EvalResult, CalcError> { DataAnalysisImpl::sum(self, data) }
    fn count(&self, data: &[f64]) -> Result<crate::core::EvalResult, CalcError> { DataAnalysisImpl::count(self, data) }
    fn norm_pdf(&self, x: f64, mu: f64, sigma: f64) -> Result<crate::core::EvalResult, CalcError> { DataAnalysisImpl::norm_pdf(self, x, mu, sigma) }
    fn norm_cdf(&self, x: f64, mu: f64, sigma: f64) -> Result<crate::core::EvalResult, CalcError> { DataAnalysisImpl::norm_cdf(self, x, mu, sigma) }
    fn norm_inv(&self, p: f64, mu: f64, sigma: f64) -> Result<crate::core::EvalResult, CalcError> { DataAnalysisImpl::norm_inv(self, p, mu, sigma) }
    fn t_pdf(&self, x: f64, df: f64) -> Result<crate::core::EvalResult, CalcError> { DataAnalysisImpl::t_pdf(self, x, df) }
    fn t_cdf(&self, x: f64, df: f64) -> Result<crate::core::EvalResult, CalcError> { DataAnalysisImpl::t_cdf(self, x, df) }
    fn t_inv(&self, p: f64, df: f64) -> Result<crate::core::EvalResult, CalcError> { DataAnalysisImpl::t_inv(self, p, df) }
    fn chi2_pdf(&self, x: f64, k: f64) -> Result<crate::core::EvalResult, CalcError> { DataAnalysisImpl::chi2_pdf(self, x, k) }
    fn chi2_cdf(&self, x: f64, k: f64) -> Result<crate::core::EvalResult, CalcError> { DataAnalysisImpl::chi2_cdf(self, x, k) }
    fn chi2_inv(&self, p: f64, k: f64) -> Result<crate::core::EvalResult, CalcError> { DataAnalysisImpl::chi2_inv(self, p, k) }
    fn f_pdf(&self, x: f64, d1: f64, d2: f64) -> Result<crate::core::EvalResult, CalcError> { DataAnalysisImpl::f_pdf(self, x, d1, d2) }
    fn f_cdf(&self, x: f64, d1: f64, d2: f64) -> Result<crate::core::EvalResult, CalcError> { DataAnalysisImpl::f_cdf(self, x, d1, d2) }
    fn f_inv(&self, p: f64, d1: f64, d2: f64) -> Result<crate::core::EvalResult, CalcError> { DataAnalysisImpl::f_inv(self, p, d1, d2) }
    fn poisson_pmf(&self, k: f64, lambda: f64) -> Result<crate::core::EvalResult, CalcError> { DataAnalysisImpl::poisson_pmf(self, k, lambda) }
    fn poisson_cdf(&self, k: f64, lambda: f64) -> Result<crate::core::EvalResult, CalcError> { DataAnalysisImpl::poisson_cdf(self, k, lambda) }
    fn binom_pmf(&self, k: f64, n: f64, p: f64) -> Result<crate::core::EvalResult, CalcError> { DataAnalysisImpl::binom_pmf(self, k, n, p) }
    fn binom_cdf(&self, k: f64, n: f64, p: f64) -> Result<crate::core::EvalResult, CalcError> { DataAnalysisImpl::binom_cdf(self, k, n, p) }
    fn t_test_one(&self, data: &[f64], mu: f64) -> Result<crate::core::EvalResult, CalcError> { DataAnalysisImpl::t_test_one(self, data, mu) }
    fn t_test_two(&self, a: &[f64], b: &[f64]) -> Result<crate::core::EvalResult, CalcError> { DataAnalysisImpl::t_test_two(self, a, b) }
    fn chi2_test(&self, observed: &[f64], expected: &[f64]) -> Result<crate::core::EvalResult, CalcError> { DataAnalysisImpl::chi2_test(self, observed, expected) }
    fn pearson(&self, x: &[f64], y: &[f64]) -> Result<crate::core::EvalResult, CalcError> { DataAnalysisImpl::pearson(self, x, y) }
    fn spearman(&self, x: &[f64], y: &[f64]) -> Result<crate::core::EvalResult, CalcError> { DataAnalysisImpl::spearman(self, x, y) }
    fn lin_reg(&self, x: &[f64], y: &[f64]) -> Result<crate::core::EvalResult, CalcError> { DataAnalysisImpl::lin_reg(self, x, y) }
    fn poly_reg(&self, x: &[f64], y: &[f64], degree: usize) -> Result<crate::core::EvalResult, CalcError> { DataAnalysisImpl::poly_reg(self, x, y, degree) }
    fn multi_reg(&self, x: &[Vec<f64>], y: &[f64]) -> Result<crate::core::EvalResult, CalcError> { DataAnalysisImpl::multi_reg(self, x, y) }
}

impl SymbolicMath for SymbolicMathImpl<'_> {
    fn differentiate(&self, expr: &str, var: &str) -> Result<crate::core::EvalResult, CalcError> { SymbolicMathImpl::differentiate(self, expr, var) }
    fn integrate(&self, expr: &str, var: &str) -> Result<crate::core::EvalResult, CalcError> { SymbolicMathImpl::integrate(self, expr, var) }
    fn simplify(&self, expr: &str) -> Result<crate::core::EvalResult, CalcError> { SymbolicMathImpl::simplify(self, expr) }
    fn limit(&self, expr: &str, var: &str, target: f64) -> Result<crate::core::EvalResult, CalcError> { SymbolicMathImpl::limit(self, expr, var, target) }
    fn taylor_expand(&self, expr: &str, var: &str, center: f64, order: usize) -> Result<crate::core::EvalResult, CalcError> { SymbolicMathImpl::taylor_expand(self, expr, var, center, order) }
    fn poly_add(&self, a: &Polynomial, b: &Polynomial) -> Result<crate::core::EvalResult, CalcError> { SymbolicMathImpl::poly_add(self, a, b) }
    fn poly_sub(&self, a: &Polynomial, b: &Polynomial) -> Result<crate::core::EvalResult, CalcError> { SymbolicMathImpl::poly_sub(self, a, b) }
    fn poly_mul(&self, a: &Polynomial, b: &Polynomial) -> Result<crate::core::EvalResult, CalcError> { SymbolicMathImpl::poly_mul(self, a, b) }
    fn poly_div(&self, a: &Polynomial, b: &Polynomial) -> Result<crate::core::EvalResult, CalcError> { SymbolicMathImpl::poly_div(self, a, b) }
    fn poly_roots(&self, p: &Polynomial) -> Result<crate::core::EvalResult, CalcError> { SymbolicMathImpl::poly_roots(self, p) }
    fn poly_eval(&self, p: &Polynomial, x: f64) -> Result<crate::core::EvalResult, CalcError> { SymbolicMathImpl::poly_eval(self, p, x) }
    fn complex_add(&self, a: &Complex, b: &Complex) -> Result<crate::core::EvalResult, CalcError> { SymbolicMathImpl::complex_add(self, a, b) }
    fn complex_sub(&self, a: &Complex, b: &Complex) -> Result<crate::core::EvalResult, CalcError> { SymbolicMathImpl::complex_sub(self, a, b) }
    fn complex_mul(&self, a: &Complex, b: &Complex) -> Result<crate::core::EvalResult, CalcError> { SymbolicMathImpl::complex_mul(self, a, b) }
    fn complex_div(&self, a: &Complex, b: &Complex) -> Result<crate::core::EvalResult, CalcError> { SymbolicMathImpl::complex_div(self, a, b) }
    fn complex_abs(&self, z: &Complex) -> Result<crate::core::EvalResult, CalcError> { SymbolicMathImpl::complex_abs(self, z) }
    fn complex_arg(&self, z: &Complex) -> Result<crate::core::EvalResult, CalcError> { SymbolicMathImpl::complex_arg(self, z) }
    fn complex_conj(&self, z: &Complex) -> Result<crate::core::EvalResult, CalcError> { SymbolicMathImpl::complex_conj(self, z) }
    fn complex_exp(&self, z: &Complex) -> Result<crate::core::EvalResult, CalcError> { SymbolicMathImpl::complex_exp(self, z) }
    fn complex_ln(&self, z: &Complex) -> Result<crate::core::EvalResult, CalcError> { SymbolicMathImpl::complex_ln(self, z) }
}

impl AppliedMath for AppliedMathImpl<'_> {
    // ── 时间：构造 ──
    #[cfg(feature = "time")]
    fn date(&self, date_str: &str) -> Result<crate::core::EvalResult, CalcError> { AppliedMathImpl::date(self, date_str) }
    #[cfg(feature = "time")]
    fn datetime(&self, datetime_str: &str, tz: Option<&str>) -> Result<crate::core::EvalResult, CalcError> { AppliedMathImpl::datetime(self, datetime_str, tz) }
    #[cfg(feature = "time")]
    fn timestamp(&self, datetime_str: &str) -> Result<crate::core::EvalResult, CalcError> { AppliedMathImpl::timestamp(self, datetime_str) }
    #[cfg(feature = "time")]
    fn from_timestamp(&self, secs: i64, tz: Option<&str>) -> Result<crate::core::EvalResult, CalcError> { AppliedMathImpl::from_timestamp(self, secs, tz) }

    // ── 时间：算术 ──
    #[cfg(feature = "time")]
    fn now(&self, tz: Option<&str>) -> Result<crate::core::EvalResult, CalcError> { AppliedMathImpl::now(self, tz) }
    #[cfg(feature = "time")]
    fn today(&self, tz: Option<&str>) -> Result<crate::core::EvalResult, CalcError> { AppliedMathImpl::today(self, tz) }
    #[cfg(feature = "time")]
    fn date_add(&self, date: &str, n: i64, unit: &str) -> Result<crate::core::EvalResult, CalcError> { AppliedMathImpl::date_add(self, date, n, unit) }
    #[cfg(feature = "time")]
    fn date_diff(&self, a: &str, b: &str, unit: Option<&str>) -> Result<crate::core::EvalResult, CalcError> { AppliedMathImpl::date_diff(self, a, b, unit) }

    // ── 时间：格式与日历 ──
    #[cfg(feature = "time")]
    fn format_date(&self, date: &str, fmt: &str, tz: Option<&str>) -> Result<crate::core::EvalResult, CalcError> { AppliedMathImpl::format_date(self, date, fmt, tz) }
    #[cfg(feature = "time")]
    fn reformat_date(&self, input: &str, from_fmt: &str, to_fmt: &str) -> Result<crate::core::EvalResult, CalcError> { AppliedMathImpl::reformat_date(self, input, from_fmt, to_fmt) }
    #[cfg(feature = "time")]
    fn weekday(&self, date: &str) -> Result<crate::core::EvalResult, CalcError> { AppliedMathImpl::weekday(self, date) }
    #[cfg(feature = "time")]
    fn day_of_year(&self, date: &str) -> Result<crate::core::EvalResult, CalcError> { AppliedMathImpl::day_of_year(self, date) }
    #[cfg(feature = "time")]
    fn is_leap_year(&self, year: i64) -> Result<crate::core::EvalResult, CalcError> { AppliedMathImpl::is_leap_year(self, year) }

    // ── 单位 ──
    #[cfg(feature = "unit")]
    fn convert(&self, value: f64, from: &str, to: &str) -> Result<crate::core::EvalResult, CalcError> { AppliedMathImpl::convert(self, value, from, to) }

    // ── 汇率 ──
    #[cfg(feature = "fx")]
    fn fx(&self, amount: f64, from: &str, to: &str) -> Result<crate::core::EvalResult, CalcError> { AppliedMathImpl::fx(self, amount, from, to) }
    #[cfg(feature = "fx")]
    fn fx_rate(&self, from: &str, to: &str) -> Result<crate::core::EvalResult, CalcError> { AppliedMathImpl::fx_rate(self, from, to) }
}
