// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! API trait 定义：5 个分组 trait。

use crate::api::types::{BigNumber, Complex, Matrix, Polynomial, Vector};
use crate::core::{CalcError, EvalResult};

/// 标量数学 trait：算术 + 科学函数 + 精度 + 数论 + 组合。
pub trait ScalarMath {
    // ── 算术 ──
    /// 加法。
    fn add(&self, a: f64, b: f64) -> Result<EvalResult, CalcError>;
    /// 减法。
    fn sub(&self, a: f64, b: f64) -> Result<EvalResult, CalcError>;
    /// 乘法。
    fn mul(&self, a: f64, b: f64) -> Result<EvalResult, CalcError>;
    /// 除法（除零返回 DivisionByZero）。
    fn div(&self, a: f64, b: f64) -> Result<EvalResult, CalcError>;
    /// 幂运算（复合上限防护）。
    fn pow(&self, a: f64, b: f64) -> Result<EvalResult, CalcError>;
    /// 取模。
    fn rem(&self, a: f64, b: f64) -> Result<EvalResult, CalcError>;
    /// 阶乘（输入上限 10000）。
    fn factorial(&self, n: u64) -> Result<EvalResult, CalcError>;
    /// 绝对值。
    fn abs(&self, x: f64) -> Result<EvalResult, CalcError>;

    // ── 科学函数 ──
    /// 正弦（弧度）。
    fn sin(&self, x: f64) -> Result<EvalResult, CalcError>;
    /// 余弦（弧度）。
    fn cos(&self, x: f64) -> Result<EvalResult, CalcError>;
    /// 正切（弧度）。
    fn tan(&self, x: f64) -> Result<EvalResult, CalcError>;
    /// 反正弦，定义域 [-1, 1]。
    fn asin(&self, x: f64) -> Result<EvalResult, CalcError>;
    /// 反余弦，定义域 [-1, 1]。
    fn acos(&self, x: f64) -> Result<EvalResult, CalcError>;
    /// 反正切。
    fn atan(&self, x: f64) -> Result<EvalResult, CalcError>;
    /// 自然对数，定义域 (0, ∞)。
    fn ln(&self, x: f64) -> Result<EvalResult, CalcError>;
    /// 以 base 为底的对数。
    fn log(&self, x: f64, base: f64) -> Result<EvalResult, CalcError>;
    /// 自然指数 e^x。
    fn exp(&self, x: f64) -> Result<EvalResult, CalcError>;
    /// 双曲正弦。
    fn sinh(&self, x: f64) -> Result<EvalResult, CalcError>;
    /// 双曲余弦。
    fn cosh(&self, x: f64) -> Result<EvalResult, CalcError>;
    /// 双曲正切。
    fn tanh(&self, x: f64) -> Result<EvalResult, CalcError>;
    /// Gamma 函数（定义域校验）。
    fn gamma(&self, x: f64) -> Result<EvalResult, CalcError>;
    /// 误差函数。
    fn erf(&self, x: f64) -> Result<EvalResult, CalcError>;

    // ── 精度 ──
    /// 任意精度求值：BigRational 计算后格式化为 digits 位小数。
    fn precision_eval(&self, digits: usize, expr: &str) -> Result<EvalResult, CalcError>;

    // ── 数论 ──
    /// 最大公约数（BigInt）。
    fn gcd(&self, a: &BigNumber, b: &BigNumber) -> Result<EvalResult, CalcError>;
    /// 最小公倍数（BigInt）。
    fn lcm(&self, a: &BigNumber, b: &BigNumber) -> Result<EvalResult, CalcError>;
    /// 素性判定（Miller-Rabin）。
    fn is_prime(&self, n: &BigNumber) -> Result<EvalResult, CalcError>;
    /// 埃氏筛：返回 n 以内全部素数（上限 10^7）。
    fn prime_sieve(&self, n: u64) -> Result<EvalResult, CalcError>;
    /// 模幂（快速幂）。
    fn mod_pow(
        &self,
        base: &BigNumber,
        exp: &BigNumber,
        m: &BigNumber,
    ) -> Result<EvalResult, CalcError>;
    /// 模逆元（存在性校验）。
    fn mod_inverse(&self, a: &BigNumber, m: &BigNumber) -> Result<EvalResult, CalcError>;
    /// 欧拉函数 φ(n)。
    fn euler_phi(&self, n: &BigNumber) -> Result<EvalResult, CalcError>;
    /// 中国剩余定理。
    fn crt(&self, remainders: &[BigNumber], moduli: &[BigNumber]) -> Result<EvalResult, CalcError>;
    /// 离散对数（BSGS）。
    fn discrete_log(
        &self,
        g: &BigNumber,
        h: &BigNumber,
        p: &BigNumber,
    ) -> Result<EvalResult, CalcError>;

    // ── 组合 ──
    /// 排列数 P(n,k)。
    fn perm(&self, n: u64, k: u64) -> Result<EvalResult, CalcError>;
    /// 组合数 C(n,k)。
    fn comb(&self, n: u64, k: u64) -> Result<EvalResult, CalcError>;
    /// 卡特兰数第 n 项。
    fn catalan(&self, n: u64) -> Result<EvalResult, CalcError>;
    /// 第一类 Stirling 数。
    fn stirling_first(&self, n: u64, k: u64) -> Result<EvalResult, CalcError>;
    /// 第二类 Stirling 数。
    fn stirling_second(&self, n: u64, k: u64) -> Result<EvalResult, CalcError>;
}

/// 线性代数 trait：矩阵 + 向量。
pub trait LinearAlgebra {
    // ── 矩阵 ──
    /// 方阵行列式。
    fn det(&self, m: &Matrix) -> Result<EvalResult, CalcError>;
    /// 方阵逆（奇异矩阵报错）。
    fn inverse(&self, m: &Matrix) -> Result<EvalResult, CalcError>;
    /// 矩阵转置。
    fn transpose(&self, m: &Matrix) -> Result<EvalResult, CalcError>;
    /// 单位矩阵。
    fn identity(&self, n: usize) -> Result<EvalResult, CalcError>;
    /// 计算 `mat_add`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn mat_add(&self, a: &Matrix, b: &Matrix) -> Result<EvalResult, CalcError>;
    /// 计算 `mat_sub`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn mat_sub(&self, a: &Matrix, b: &Matrix) -> Result<EvalResult, CalcError>;
    /// 计算 `mat_mul`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn mat_mul(&self, a: &Matrix, b: &Matrix) -> Result<EvalResult, CalcError>;
    /// 计算 `scalar_mul`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn scalar_mul(&self, s: f64, m: &Matrix) -> Result<EvalResult, CalcError>;

    // ── 向量 ──
    /// 计算 `dot`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn dot(&self, a: &Vector, b: &Vector) -> Result<EvalResult, CalcError>;
    /// 计算 `cross`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn cross(&self, a: &Vector, b: &Vector) -> Result<EvalResult, CalcError>;
    /// 计算 `normalize`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn normalize(&self, a: &Vector) -> Result<EvalResult, CalcError>;
    /// 计算 `magnitude`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn magnitude(&self, a: &Vector) -> Result<EvalResult, CalcError>;
    /// 计算 `vector_add`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn vector_add(&self, a: &Vector, b: &Vector) -> Result<EvalResult, CalcError>;
    /// 计算 `vector_sub`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn vector_sub(&self, a: &Vector, b: &Vector) -> Result<EvalResult, CalcError>;

    // ── 数值分解（feature-gated）──
    #[cfg(feature = "numerical")]
    /// 实对称矩阵特征分解（numerical feature）。
    fn eig(&self, m: &Matrix) -> Result<EvalResult, CalcError>;
    #[cfg(feature = "numerical")]
    /// 奇异值分解（numerical feature）。
    fn svd(&self, m: &Matrix) -> Result<EvalResult, CalcError>;
    #[cfg(feature = "numerical")]
    /// LU 分解（numerical feature，f64 近似）。
    fn lu(&self, m: &Matrix) -> Result<EvalResult, CalcError>;
    #[cfg(feature = "numerical")]
    /// QR 分解（numerical feature）。
    fn qr(&self, m: &Matrix) -> Result<EvalResult, CalcError>;
    #[cfg(feature = "numerical")]
    /// 线性方程组 Ax=b（numerical feature）。
    fn solve(&self, a: &Matrix, b: &Vector) -> Result<EvalResult, CalcError>;
    #[cfg(feature = "numerical")]
    /// 矩阵指数（Pade 近似，numerical feature）。
    fn matrix_exp(&self, m: &Matrix) -> Result<EvalResult, CalcError>;
}

/// 数据分析 trait：统计。
pub trait DataAnalysis {
    // ── 基础统计 ──
    /// 算术平均。
    fn mean(&self, data: &[f64]) -> Result<EvalResult, CalcError>;
    /// 方差（样本，n-1）。
    fn variance(&self, data: &[f64]) -> Result<EvalResult, CalcError>;
    /// 计算 `std`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn std(&self, data: &[f64]) -> Result<EvalResult, CalcError>;
    /// 中位数。
    fn median(&self, data: &[f64]) -> Result<EvalResult, CalcError>;
    /// 计算 `min`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn min(&self, data: &[f64]) -> Result<EvalResult, CalcError>;
    /// 计算 `max`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn max(&self, data: &[f64]) -> Result<EvalResult, CalcError>;
    /// 计算 `sum`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn sum(&self, data: &[f64]) -> Result<EvalResult, CalcError>;
    /// 计算 `count`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn count(&self, data: &[f64]) -> Result<EvalResult, CalcError>;

    // ── 分布函数 ──
    /// 计算 `norm_pdf`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn norm_pdf(&self, x: f64, mu: f64, sigma: f64) -> Result<EvalResult, CalcError>;
    /// 计算 `norm_cdf`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn norm_cdf(&self, x: f64, mu: f64, sigma: f64) -> Result<EvalResult, CalcError>;
    /// 计算 `norm_inv`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn norm_inv(&self, p: f64, mu: f64, sigma: f64) -> Result<EvalResult, CalcError>;
    /// 计算 `t_pdf`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn t_pdf(&self, x: f64, df: f64) -> Result<EvalResult, CalcError>;
    /// 计算 `t_cdf`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn t_cdf(&self, x: f64, df: f64) -> Result<EvalResult, CalcError>;
    /// 计算 `t_inv`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn t_inv(&self, p: f64, df: f64) -> Result<EvalResult, CalcError>;
    /// 计算 `chi2_pdf`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn chi2_pdf(&self, x: f64, k: f64) -> Result<EvalResult, CalcError>;
    /// 计算 `chi2_cdf`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn chi2_cdf(&self, x: f64, k: f64) -> Result<EvalResult, CalcError>;
    /// 计算 `chi2_inv`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn chi2_inv(&self, p: f64, k: f64) -> Result<EvalResult, CalcError>;
    /// 计算 `f_pdf`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn f_pdf(&self, x: f64, d1: f64, d2: f64) -> Result<EvalResult, CalcError>;
    /// 计算 `f_cdf`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn f_cdf(&self, x: f64, d1: f64, d2: f64) -> Result<EvalResult, CalcError>;
    /// 计算 `f_inv`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn f_inv(&self, p: f64, d1: f64, d2: f64) -> Result<EvalResult, CalcError>;
    /// 计算 `poisson_pmf`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn poisson_pmf(&self, k: f64, lambda: f64) -> Result<EvalResult, CalcError>;
    /// 计算 `poisson_cdf`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn poisson_cdf(&self, k: f64, lambda: f64) -> Result<EvalResult, CalcError>;
    /// 计算 `binom_pmf`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn binom_pmf(&self, k: f64, n: f64, p: f64) -> Result<EvalResult, CalcError>;
    /// 计算 `binom_cdf`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn binom_cdf(&self, k: f64, n: f64, p: f64) -> Result<EvalResult, CalcError>;

    // ── 假设检验 ──
    /// 计算 `t_test_one`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn t_test_one(&self, data: &[f64], mu: f64) -> Result<EvalResult, CalcError>;
    /// 计算 `t_test_two`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn t_test_two(&self, a: &[f64], b: &[f64]) -> Result<EvalResult, CalcError>;
    /// 计算 `chi2_test`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn chi2_test(&self, observed: &[f64], expected: &[f64]) -> Result<EvalResult, CalcError>;

    // ── 相关 ──
    /// 计算 `pearson`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn pearson(&self, x: &[f64], y: &[f64]) -> Result<EvalResult, CalcError>;
    /// 计算 `spearman`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn spearman(&self, x: &[f64], y: &[f64]) -> Result<EvalResult, CalcError>;

    // ── 回归 ──
    /// 计算 `lin_reg`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn lin_reg(&self, x: &[f64], y: &[f64]) -> Result<EvalResult, CalcError>;
    /// 计算 `poly_reg`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn poly_reg(&self, x: &[f64], y: &[f64], degree: usize) -> Result<EvalResult, CalcError>;
    /// 计算 `multi_reg`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn multi_reg(&self, x: &[Vec<f64>], y: &[f64]) -> Result<EvalResult, CalcError>;
}

/// 符号数学 trait：符号演算 + 多项式 + 复数。
pub trait SymbolicMath {
    // ── 符号演算 ──
    /// 符号微分。
    fn differentiate(&self, expr: &str, var: &str) -> Result<EvalResult, CalcError>;
    /// 计算 `integrate`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn integrate(&self, expr: &str, var: &str) -> Result<EvalResult, CalcError>;
    /// 计算 `simplify`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn simplify(&self, expr: &str) -> Result<EvalResult, CalcError>;
    /// 计算 `limit`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn limit(&self, expr: &str, var: &str, target: f64) -> Result<EvalResult, CalcError>;
    /// 泰勒展开。
    fn taylor_expand(
        &self,
        expr: &str,
        var: &str,
        center: f64,
        order: usize,
    ) -> Result<EvalResult, CalcError>;

    // ── 多项式 ──
    /// 计算 `poly_add`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn poly_add(&self, a: &Polynomial, b: &Polynomial) -> Result<EvalResult, CalcError>;
    /// 计算 `poly_sub`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn poly_sub(&self, a: &Polynomial, b: &Polynomial) -> Result<EvalResult, CalcError>;
    /// 计算 `poly_mul`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn poly_mul(&self, a: &Polynomial, b: &Polynomial) -> Result<EvalResult, CalcError>;
    /// 计算 `poly_div`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn poly_div(&self, a: &Polynomial, b: &Polynomial) -> Result<EvalResult, CalcError>;
    /// 计算 `poly_roots`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn poly_roots(&self, p: &Polynomial) -> Result<EvalResult, CalcError>;
    /// 计算 `poly_eval`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn poly_eval(&self, p: &Polynomial, x: f64) -> Result<EvalResult, CalcError>;

    // ── 复数 ──
    /// 计算 `complex_add`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn complex_add(&self, a: &Complex, b: &Complex) -> Result<EvalResult, CalcError>;
    /// 计算 `complex_sub`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn complex_sub(&self, a: &Complex, b: &Complex) -> Result<EvalResult, CalcError>;
    /// 计算 `complex_mul`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn complex_mul(&self, a: &Complex, b: &Complex) -> Result<EvalResult, CalcError>;
    /// 计算 `complex_div`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn complex_div(&self, a: &Complex, b: &Complex) -> Result<EvalResult, CalcError>;
    /// 计算 `complex_abs`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn complex_abs(&self, z: &Complex) -> Result<EvalResult, CalcError>;
    /// 计算 `complex_arg`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn complex_arg(&self, z: &Complex) -> Result<EvalResult, CalcError>;
    /// 计算 `complex_conj`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn complex_conj(&self, z: &Complex) -> Result<EvalResult, CalcError>;
    /// 计算 `complex_exp`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn complex_exp(&self, z: &Complex) -> Result<EvalResult, CalcError>;
    /// 计算 `complex_ln`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn complex_ln(&self, z: &Complex) -> Result<EvalResult, CalcError>;

    // ── 方程求解 ──
    /// 计算 `solve_equation`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn solve_equation(
        &self,
        expr: &str,
        var: &str,
        method: &str,
        options: Option<&[f64]>,
    ) -> Result<EvalResult, CalcError>;
}

/// 应用数学 trait：时间 + 单位 + 汇率（feature-gated）。
pub trait AppliedMath {
    // ── 时间：构造 ──
    #[cfg(feature = "time")]
    /// 计算 `date`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn date(&self, date_str: &str) -> Result<EvalResult, CalcError>;
    #[cfg(feature = "time")]
    /// 计算 `datetime`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn datetime(&self, datetime_str: &str, tz: Option<&str>) -> Result<EvalResult, CalcError>;
    #[cfg(feature = "time")]
    /// 计算 `timestamp`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn timestamp(&self, datetime_str: &str) -> Result<EvalResult, CalcError>;
    #[cfg(feature = "time")]
    /// 计算 `from_timestamp`（语义详见 docs/API_GUIDE.md 对应章节）。
    /// 命名说明：`from_*` 惯例无 self，但本 trait 全方法统一 `&self` 求值门面签名
    /// （命名权衡，与 timestamp/now 等一致），故豁免 clippy::wrong_self_convention。
    #[allow(clippy::wrong_self_convention)]
    fn from_timestamp(&self, secs: i64, tz: Option<&str>) -> Result<EvalResult, CalcError>;

    // ── 时间：算术 ──
    #[cfg(feature = "time")]
    /// 计算 `now`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn now(&self, tz: Option<&str>) -> Result<EvalResult, CalcError>;
    #[cfg(feature = "time")]
    /// 计算 `today`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn today(&self, tz: Option<&str>) -> Result<EvalResult, CalcError>;
    #[cfg(feature = "time")]
    /// 计算 `date_add`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn date_add(&self, date: &str, n: i64, unit: &str) -> Result<EvalResult, CalcError>;
    #[cfg(feature = "time")]
    /// 计算 `date_diff`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn date_diff(&self, a: &str, b: &str, unit: Option<&str>) -> Result<EvalResult, CalcError>;

    // ── 时间：格式与日历 ──
    #[cfg(feature = "time")]
    /// 计算 `format_date`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn format_date(&self, date: &str, fmt: &str, tz: Option<&str>)
    -> Result<EvalResult, CalcError>;
    #[cfg(feature = "time")]
    /// 计算 `reformat_date`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn reformat_date(
        &self,
        input: &str,
        from_fmt: &str,
        to_fmt: &str,
    ) -> Result<EvalResult, CalcError>;
    #[cfg(feature = "time")]
    /// 计算 `weekday`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn weekday(&self, date: &str) -> Result<EvalResult, CalcError>;
    #[cfg(feature = "time")]
    /// 计算 `day_of_year`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn day_of_year(&self, date: &str) -> Result<EvalResult, CalcError>;
    #[cfg(feature = "time")]
    /// 计算 `is_leap_year`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn is_leap_year(&self, year: i64) -> Result<EvalResult, CalcError>;

    // ── 单位 ──
    #[cfg(feature = "unit")]
    /// 计算 `convert`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn convert(&self, value: f64, from: &str, to: &str) -> Result<EvalResult, CalcError>;

    // ── 汇率 ──
    #[cfg(feature = "fx")]
    /// 计算 `fx`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn fx(&self, amount: f64, from: &str, to: &str) -> Result<EvalResult, CalcError>;
    #[cfg(feature = "fx")]
    /// 计算 `fx_rate`（语义详见 docs/API_GUIDE.md 对应章节）。
    fn fx_rate(&self, from: &str, to: &str) -> Result<EvalResult, CalcError>;
}
