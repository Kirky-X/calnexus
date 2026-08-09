// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! API 层类型包装器。
//!
//! 为 `EvalResult` 的各变体提供类型安全的包装，
//! 用于直接 API（`api/`）的方法参数和返回值。

use num_bigint::BigInt;
use num_complex::Complex64;

use crate::core::EvalResult;

/// 矩阵包装器：封装 `Vec<Vec<f64>>`（与 `EvalResult::Matrix` 一致）。
///
/// 提供行优先构造器 + 到/从 `EvalResult` 的双向转换。
#[derive(Debug, Clone, PartialEq)]
pub struct Matrix {
    rows: Vec<Vec<f64>>,
}

impl Matrix {
    /// 从行优先二维向量构造。
    pub fn from_rows(rows: &[&[f64]]) -> Self {
        Self {
            rows: rows.iter().map(|r| r.to_vec()).collect(),
        }
    }

    /// 获取行数。
    pub fn nrows(&self) -> usize {
        self.rows.len()
    }

    /// 获取列数（假设矩形）。
    pub fn ncols(&self) -> usize {
        self.rows.first().map_or(0, |r| r.len())
    }

    /// 获取内部行优先数据。
    pub fn as_rows(&self) -> &[Vec<f64>] {
        &self.rows
    }
}

impl From<Matrix> for EvalResult {
    fn from(m: Matrix) -> Self {
        EvalResult::Matrix(m.rows)
    }
}

impl TryFrom<EvalResult> for Matrix {
    type Error = &'static str;

    fn try_from(result: EvalResult) -> Result<Self, Self::Error> {
        match result {
            EvalResult::Matrix(rows) => Ok(Matrix { rows }),
            _ => Err("expected Matrix result"),
        }
    }
}

/// 向量包装器：封装 `Vec<f64>`。
#[derive(Debug, Clone, PartialEq)]
pub struct Vector {
    data: Vec<f64>,
}

impl Vector {
    /// 从切片构造。
    pub fn new(data: &[f64]) -> Self {
        Self {
            data: data.to_vec(),
        }
    }

    /// 获取长度。
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// 获取内部数据。
    pub fn as_slice(&self) -> &[f64] {
        &self.data
    }
}

impl From<Vector> for EvalResult {
    fn from(v: Vector) -> Self {
        EvalResult::Vector(v.data)
    }
}

impl TryFrom<EvalResult> for Vector {
    type Error = &'static str;

    fn try_from(result: EvalResult) -> Result<Self, Self::Error> {
        match result {
            EvalResult::Vector(data) => Ok(Vector { data }),
            _ => Err("expected Vector result"),
        }
    }
}

/// 复数包装器：封装 `Complex64`。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Complex {
    value: Complex64,
}

impl Complex {
    /// 从实部/虚部构造。
    pub fn new(re: f64, im: f64) -> Self {
        Self {
            value: Complex64::new(re, im),
        }
    }

    /// 获取实部。
    pub fn re(&self) -> f64 {
        self.value.re
    }

    /// 获取虚部。
    pub fn im(&self) -> f64 {
        self.value.im
    }

    /// 获取内部 `Complex64` 值。
    pub fn value(&self) -> Complex64 {
        self.value
    }
}

impl From<Complex> for EvalResult {
    fn from(c: Complex) -> Self {
        EvalResult::Complex(c.re(), c.im())
    }
}

impl TryFrom<EvalResult> for Complex {
    type Error = &'static str;

    fn try_from(result: EvalResult) -> Result<Self, Self::Error> {
        match result {
            EvalResult::Complex(re, im) => Ok(Complex::new(re, im)),
            _ => Err("expected Complex result"),
        }
    }
}

/// 多项式包装器：封装 `Vec<f64>` 升幂系数（coef[i] 为 x^i 的系数）。
#[derive(Debug, Clone, PartialEq)]
pub struct Polynomial {
    coeffs: Vec<f64>,
}

impl Polynomial {
    /// 从升幂系数向量构造。
    pub fn new(coeffs: &[f64]) -> Self {
        Self {
            coeffs: coeffs.to_vec(),
        }
    }

    /// 获取系数向量。
    pub fn coeffs(&self) -> &[f64] {
        &self.coeffs
    }

    /// 获取多项式次数（系数向量长度 - 1）。
    pub fn degree(&self) -> usize {
        self.coeffs.len().saturating_sub(1)
    }
}

impl From<Polynomial> for EvalResult {
    fn from(p: Polynomial) -> Self {
        EvalResult::Polynomial(p.coeffs)
    }
}

impl TryFrom<EvalResult> for Polynomial {
    type Error = &'static str;

    fn try_from(result: EvalResult) -> Result<Self, Self::Error> {
        match result {
            EvalResult::Polynomial(coeffs) => Ok(Polynomial { coeffs }),
            _ => Err("expected Polynomial result"),
        }
    }
}

/// 大整数包装器：封装 `BigInt`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BigNumber {
    value: BigInt,
}

impl BigNumber {
    /// 从 `BigInt` 构造。
    pub fn new(value: BigInt) -> Self {
        Self { value }
    }

    /// 从 `i64` 构造。
    pub fn from_i64(v: i64) -> Self {
        Self {
            value: BigInt::from(v),
        }
    }

    /// 获取内部 `BigInt` 值。
    pub fn value(&self) -> &BigInt {
        &self.value
    }
}

impl From<BigNumber> for EvalResult {
    fn from(b: BigNumber) -> Self {
        EvalResult::BigInt(b.value)
    }
}

impl TryFrom<EvalResult> for BigNumber {
    type Error = &'static str;

    fn try_from(result: EvalResult) -> Result<Self, Self::Error> {
        match result {
            EvalResult::BigInt(value) => Ok(BigNumber { value }),
            _ => Err("expected BigInt result"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== Matrix =====

    #[test]
    fn test_matrix_from_rows() {
        let m = Matrix::from_rows(&[&[1.0, 2.0], &[3.0, 4.0]]);
        assert_eq!(m.nrows(), 2);
        assert_eq!(m.ncols(), 2);
    }

    #[test]
    fn test_matrix_roundtrip() {
        let m = Matrix::from_rows(&[&[1.0, 2.0], &[3.0, 4.0]]);
        let result: EvalResult = m.clone().into();
        let m2 = Matrix::try_from(result).unwrap();
        assert_eq!(m, m2);
    }

    #[test]
    fn test_matrix_try_from_wrong_variant() {
        let result = EvalResult::Scalar(1.0);
        assert!(Matrix::try_from(result).is_err());
    }

    // ===== Vector =====

    #[test]
    fn test_vector_new() {
        let v = Vector::new(&[1.0, 2.0, 3.0]);
        assert_eq!(v.len(), 3);
        assert!(!v.is_empty());
    }

    #[test]
    fn test_vector_roundtrip() {
        let v = Vector::new(&[1.0, 2.0, 3.0]);
        let result: EvalResult = v.clone().into();
        let v2 = Vector::try_from(result).unwrap();
        assert_eq!(v, v2);
    }

    // ===== Complex =====

    #[test]
    fn test_complex_new() {
        let c = Complex::new(3.0, 4.0);
        assert_eq!(c.re(), 3.0);
        assert_eq!(c.im(), 4.0);
    }

    #[test]
    fn test_complex_roundtrip() {
        let c = Complex::new(3.0, 4.0);
        let result: EvalResult = c.into();
        let c2 = Complex::try_from(result).unwrap();
        assert_eq!(c, c2);
    }

    // ===== Polynomial =====

    #[test]
    fn test_polynomial_new() {
        let p = Polynomial::new(&[1.0, 2.0, 3.0]);
        assert_eq!(p.degree(), 2);
        assert_eq!(p.coeffs(), &[1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_polynomial_roundtrip() {
        let p = Polynomial::new(&[1.0, 2.0, 3.0]);
        let result: EvalResult = p.clone().into();
        let p2 = Polynomial::try_from(result).unwrap();
        assert_eq!(p, p2);
    }

    // ===== BigNumber =====

    #[test]
    fn test_bignumber_from_i64() {
        let b = BigNumber::from_i64(42);
        assert_eq!(b.value(), &BigInt::from(42));
    }

    #[test]
    fn test_bignumber_roundtrip() {
        let b = BigNumber::from_i64(42);
        let result: EvalResult = b.clone().into();
        let b2 = BigNumber::try_from(result).unwrap();
        assert_eq!(b, b2);
    }

    #[test]
    fn test_bignumber_try_from_wrong_variant() {
        let result = EvalResult::Scalar(1.0);
        assert!(BigNumber::try_from(result).is_err());
    }
}
