// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! 矩阵核心函数：行列式/逆/转置/单位矩阵/加减乘/标量运算。
//!
//! 将 `domains/matrix.rs` 中的矩阵数学逻辑提取为独立 `pub fn`，
//! 接收 `&DMatrix<f64>` 参数，供域层和 API 层共用。

use nalgebra::DMatrix;

use crate::core::CalcError;

/// 矩阵维度上限（防 DoS）。
pub const MAX_MATRIX_DIM: usize = 1000;

/// 方阵行列式。要求方阵。
pub fn det(m: &DMatrix<f64>) -> Result<f64, CalcError> {
    if !m.is_square() {
        return Err(CalcError::domain(format!(
            "det() requires a square matrix, got {}x{}",
            m.nrows(),
            m.ncols()
        )));
    }
    Ok(m.determinant())
}

/// 矩阵转置。
pub fn transpose(m: &DMatrix<f64>) -> DMatrix<f64> {
    m.transpose()
}

/// 方阵逆矩阵。奇异矩阵返回错误。
pub fn inverse(m: &DMatrix<f64>) -> Result<DMatrix<f64>, CalcError> {
    if !m.is_square() {
        return Err(CalcError::domain(format!(
            "inverse() requires a square matrix, got {}x{}",
            m.nrows(),
            m.ncols()
        )));
    }
    m.clone().try_inverse().ok_or_else(|| {
        CalcError::domain("matrix is singular (not invertible)".to_string())
    })
}

/// n×n 单位矩阵。n 须 ≤ MAX_MATRIX_DIM。
pub fn identity(n: usize) -> Result<DMatrix<f64>, CalcError> {
    if n > MAX_MATRIX_DIM {
        return Err(CalcError::domain(format!(
            "identity() dimension {} exceeds maximum of {}",
            n, MAX_MATRIX_DIM
        )));
    }
    Ok(DMatrix::identity(n, n))
}

/// 矩阵加法。要求形状一致。
pub fn mat_add(a: &DMatrix<f64>, b: &DMatrix<f64>) -> Result<DMatrix<f64>, CalcError> {
    if a.shape() != b.shape() {
        return Err(CalcError::domain(format!(
            "matrix dimension mismatch for add: {}x{} vs {}x{}",
            a.nrows(), a.ncols(), b.nrows(), b.ncols()
        )));
    }
    Ok(a + b)
}

/// 矩阵减法。要求形状一致。
pub fn mat_sub(a: &DMatrix<f64>, b: &DMatrix<f64>) -> Result<DMatrix<f64>, CalcError> {
    if a.shape() != b.shape() {
        return Err(CalcError::domain(format!(
            "matrix dimension mismatch for sub: {}x{} vs {}x{}",
            a.nrows(), a.ncols(), b.nrows(), b.ncols()
        )));
    }
    Ok(a - b)
}

/// 矩阵乘法。支持 矩阵×矩阵（a.ncols == b.nrows）和 矩阵×标量/标量×矩阵。
pub fn mat_mul(a: &DMatrix<f64>, b: &DMatrix<f64>) -> Result<DMatrix<f64>, CalcError> {
    if a.ncols() != b.nrows() {
        return Err(CalcError::domain(format!(
            "matrix multiplication dimension mismatch: {}x{} * {}x{}",
            a.nrows(), a.ncols(), b.nrows(), b.ncols()
        )));
    }
    Ok(a * b)
}

/// 标量乘法。
pub fn scalar_mul(m: &DMatrix<f64>, s: f64) -> DMatrix<f64> {
    m * s
}

/// 矩阵除以标量。
pub fn scalar_div(m: &DMatrix<f64>, s: f64) -> Result<DMatrix<f64>, CalcError> {
    if s == 0.0 {
        return Err(CalcError::division_by_zero());
    }
    Ok(m / s)
}

// ===== 测试 =====

#[cfg(test)]
mod tests {
    use super::*;

    fn mat2x2(data: [f64; 4]) -> DMatrix<f64> {
        DMatrix::from_row_slice(2, 2, &data)
    }

    // --- det ---

    #[test]
    fn test_det_2x2() {
        let m = mat2x2([1.0, 2.0, 3.0, 4.0]);
        assert!((det(&m).unwrap() - (-2.0)).abs() < 1e-10);
    }

    #[test]
    fn test_det_identity() {
        let m = DMatrix::identity(3, 3);
        assert!((det(&m).unwrap() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_det_non_square() {
        let m = DMatrix::from_row_slice(2, 3, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        assert!(det(&m).is_err());
    }

    // --- transpose ---

    #[test]
    fn test_transpose() {
        let m = mat2x2([1.0, 2.0, 3.0, 4.0]);
        let t = transpose(&m);
        assert!((t[(0, 1)] - 3.0).abs() < 1e-10);
        assert!((t[(1, 0)] - 2.0).abs() < 1e-10);
    }

    // --- inverse ---

    #[test]
    fn test_inverse_2x2() {
        let m = mat2x2([1.0, 2.0, 3.0, 4.0]);
        let inv = inverse(&m).unwrap();
        let product = &m * &inv;
        for i in 0..2 {
            for j in 0..2 {
                let expected = if i == j { 1.0 } else { 0.0 };
                assert!((product[(i, j)] - expected).abs() < 1e-10);
            }
        }
    }

    #[test]
    fn test_inverse_singular() {
        let m = mat2x2([1.0, 2.0, 2.0, 4.0]);
        assert!(inverse(&m).is_err());
    }

    #[test]
    fn test_inverse_non_square() {
        let m = DMatrix::from_row_slice(2, 3, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        assert!(inverse(&m).is_err());
    }

    // --- identity ---

    #[test]
    fn test_identity_3() {
        let m = identity(3).unwrap();
        assert_eq!(m.nrows(), 3);
        assert_eq!(m.ncols(), 3);
        assert!((m[(0, 0)] - 1.0).abs() < 1e-15);
        assert!((m[(0, 1)] - 0.0).abs() < 1e-15);
    }

    #[test]
    fn test_identity_exceeds_limit() {
        assert!(identity(MAX_MATRIX_DIM + 1).is_err());
    }

    // --- mat_add / mat_sub ---

    #[test]
    fn test_mat_add() {
        let a = mat2x2([1.0, 2.0, 3.0, 4.0]);
        let b = mat2x2([5.0, 6.0, 7.0, 8.0]);
        let c = mat_add(&a, &b).unwrap();
        assert!((c[(0, 0)] - 6.0).abs() < 1e-10);
        assert!((c[(1, 1)] - 12.0).abs() < 1e-10);
    }

    #[test]
    fn test_mat_add_dim_mismatch() {
        let a = DMatrix::from_row_slice(2, 2, &[1.0, 2.0, 3.0, 4.0]);
        let b = DMatrix::from_row_slice(2, 3, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        assert!(mat_add(&a, &b).is_err());
    }

    #[test]
    fn test_mat_sub() {
        let a = mat2x2([5.0, 6.0, 7.0, 8.0]);
        let b = mat2x2([1.0, 2.0, 3.0, 4.0]);
        let c = mat_sub(&a, &b).unwrap();
        assert!((c[(0, 0)] - 4.0).abs() < 1e-10);
    }

    // --- mat_mul ---

    #[test]
    fn test_mat_mul() {
        let a = mat2x2([1.0, 2.0, 3.0, 4.0]);
        let b = mat2x2([5.0, 6.0, 7.0, 8.0]);
        let c = mat_mul(&a, &b).unwrap();
        // [1*5+2*7, 1*6+2*8] = [19, 22]
        assert!((c[(0, 0)] - 19.0).abs() < 1e-10);
        assert!((c[(0, 1)] - 22.0).abs() < 1e-10);
    }

    #[test]
    fn test_mat_mul_dim_mismatch() {
        let a = DMatrix::from_row_slice(2, 3, &[1.0; 6]);
        let b = DMatrix::from_row_slice(2, 2, &[1.0; 4]);
        assert!(mat_mul(&a, &b).is_err());
    }

    // --- scalar_mul / scalar_div ---

    #[test]
    fn test_scalar_mul() {
        let m = mat2x2([1.0, 2.0, 3.0, 4.0]);
        let r = scalar_mul(&m, 3.0);
        assert!((r[(0, 0)] - 3.0).abs() < 1e-10);
        assert!((r[(1, 1)] - 12.0).abs() < 1e-10);
    }

    #[test]
    fn test_scalar_div() {
        let m = mat2x2([2.0, 4.0, 6.0, 8.0]);
        let r = scalar_div(&m, 2.0).unwrap();
        assert!((r[(0, 0)] - 1.0).abs() < 1e-10);
        assert!((r[(1, 1)] - 4.0).abs() < 1e-10);
    }

    #[test]
    fn test_scalar_div_zero() {
        let m = mat2x2([1.0, 2.0, 3.0, 4.0]);
        assert!(scalar_div(&m, 0.0).is_err());
    }
}
