// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! 数值线性代数核心函数：特征分解/SVD/LU/QR/线性方程组求解/矩阵指数。
//!
//! 从 `domains/numerical.rs` 提取的纯数学逻辑，返回纯 Rust 类型（非 `EvalResult`），
//! 供 `domains/`（AST 求值路径）和 `api/`（直接 API 路径）共用。
//!
//! nalgebra 0.35 分解 API：
//! - `LU::new(m)`：partial row pivoting，P·A = L·U
//! - `SVD::new(m, compute_u, compute_v)`：三参数
//! - `SymmetricEigen::new(m)`：实对称矩阵特征分解
//! - `QR::new(m)`：Householder 瘦 QR
//!
//! 输入净化：所有函数入口先 `require_finite` 拦截 NaN/Inf。

#![cfg(feature = "numerical")]

use crate::core::CalcError;
use nalgebra::{DMatrix, DVector, SymmetricEigen, LU, QR, SVD};

/// 实对称矩阵特征分解 → (特征值升序, 特征向量矩阵列对应)。
///
/// 要求实对称方阵。非方阵 → DomainError；非对称 → DomainError；NaN/Inf → NaNOrInf。
pub fn eig(matrix: &DMatrix<f64>) -> Result<(Vec<f64>, DMatrix<f64>), CalcError> {
    require_finite(matrix.iter().copied())?;
    if !matrix.is_square() {
        return Err(CalcError::domain(format!(
            "eig() requires a square matrix, got {}x{}",
            matrix.nrows(),
            matrix.ncols()
        )));
    }
    const SYMMETRY_TOL: f64 = 1e-10;
    let n = matrix.nrows();
    for i in 0..n {
        for j in (i + 1)..n {
            let scale = matrix[(i, j)].abs().max(matrix[(j, i)].abs()).max(1.0);
            if (matrix[(i, j)] - matrix[(j, i)]).abs() > SYMMETRY_TOL * scale {
                return Err(CalcError::domain(
                    "eig() requires a real symmetric matrix".to_string(),
                ));
            }
        }
    }
    let decomp = SymmetricEigen::new(matrix.clone());
    let mut indexed: Vec<(usize, f64)> = decomp.eigenvalues.iter().copied().enumerate().collect();
    indexed.sort_by(|a, b| {
        a.1.partial_cmp(&b.1).unwrap_or_else(|| {
            // NaN 无法比较，按 f64 总序定义处理：NaN 排最后
            match (a.1.is_nan(), b.1.is_nan()) {
                (true, true) => std::cmp::Ordering::Equal,
                (true, false) => std::cmp::Ordering::Greater,
                (false, true) => std::cmp::Ordering::Less,
                _ => std::cmp::Ordering::Equal,
            }
        })
    });
    let values: Vec<f64> = indexed.iter().map(|&(_, v)| v).collect();
    let mut sorted_vecs = DMatrix::<f64>::zeros(n, n);
    for (col, &(src, _)) in indexed.iter().enumerate() {
        for row in 0..n {
            sorted_vecs[(row, col)] = decomp.eigenvectors[(row, src)];
        }
    }
    Ok((values, sorted_vecs))
}

/// 奇异值分解 → (U, S降序, Vt)，满足 A = U·diag(S)·Vt。
///
/// NaN/Inf → NaNOrInf。
pub fn svd(matrix: &DMatrix<f64>) -> Result<(DMatrix<f64>, Vec<f64>, DMatrix<f64>), CalcError> {
    require_finite(matrix.iter().copied())?;
    let decomp = SVD::new(matrix.clone(), true, true);
    let u = decomp.u.ok_or_else(|| CalcError::domain("SVD decomposition failed: U not available"))?;
    let vt = decomp.v_t.ok_or_else(|| CalcError::domain("SVD decomposition failed: Vt not available"))?;
    let s: Vec<f64> = decomp.singular_values.iter().copied().collect();
    Ok((u, s, vt))
}

/// LU 分解 → (L, U, P)，满足 P·A = L·U。
///
/// 方阵要求；L 单位下三角，U 上三角，P 置换矩阵。
pub fn lu(matrix: &DMatrix<f64>) -> Result<(DMatrix<f64>, DMatrix<f64>, DMatrix<f64>), CalcError> {
    require_finite(matrix.iter().copied())?;
    if !matrix.is_square() {
        return Err(CalcError::domain(format!(
            "lu() requires a square matrix, got {}x{}",
            matrix.nrows(),
            matrix.ncols()
        )));
    }
    let n = matrix.nrows();
    let decomp = LU::new(matrix.clone());
    let l = decomp.l();
    let u = decomp.u();
    let mut p_mat = DMatrix::<f64>::identity(n, n);
    decomp.p().permute_rows(&mut p_mat);
    Ok((l, u, p_mat))
}

/// QR 分解 → (Q, R)，满足 A = Q·R。
///
/// 瘦 QR（Householder），Q 列正交，R 上三角。
pub fn qr(matrix: &DMatrix<f64>) -> Result<(DMatrix<f64>, DMatrix<f64>), CalcError> {
    require_finite(matrix.iter().copied())?;
    let decomp = QR::new(matrix.clone());
    let q = decomp.q();
    let r = decomp.r();
    Ok((q, r))
}

/// 解线性方程组 A·x = b → x。
///
/// A 须方阵且与 b 行数匹配；A 奇异 → DomainError。
pub fn solve(matrix: &DMatrix<f64>, b: &DVector<f64>) -> Result<DVector<f64>, CalcError> {
    require_finite(matrix.iter().copied())?;
    require_finite(b.iter().copied())?;
    if !matrix.is_square() {
        return Err(CalcError::domain(format!(
            "solve() requires a square coefficient matrix, got {}x{}",
            matrix.nrows(),
            matrix.ncols()
        )));
    }
    if b.len() != matrix.nrows() {
        return Err(CalcError::domain(format!(
            "solve() dimension mismatch: A is {}x{} but b has {} entries",
            matrix.nrows(),
            matrix.ncols(),
            b.len()
        )));
    }
    let lu_decomp = LU::new(matrix.clone());
    lu_decomp.solve(b).ok_or_else(|| {
        CalcError::domain("solve(): coefficient matrix is singular".to_string())
    })
}

/// 矩阵指数 exp(A)。
///
/// 实对称矩阵走特征值分解快速路径；非对称方阵走 Pade 近似（缩放-平方法）；
/// 非方阵 → DomainError；NaN/Inf → NaNOrInf。
pub fn matrix_exp(matrix: &DMatrix<f64>) -> Result<DMatrix<f64>, CalcError> {
    require_finite(matrix.iter().copied())?;
    if !matrix.is_square() {
        return Err(CalcError::domain(format!(
            "matrix_exp() requires a square matrix, got {}x{}",
            matrix.nrows(),
            matrix.ncols()
        )));
    }
    let n = matrix.nrows();
    // 快速路径：实对称矩阵 → 特征值分解
    if is_symmetric(matrix) {
        let (values, vectors) = eig(matrix)?;
        // exp(A) = V · diag(e^λ_i) · V^T
        let mut diag = DMatrix::<f64>::zeros(n, n);
        for i in 0..n {
            diag[(i, i)] = values[i].exp();
        }
        return Ok(&vectors * diag * vectors.transpose());
    }
    // 一般路径：Pade 近似 + 缩放-平方法
    pade_exp(matrix)
}

/// 校验矩阵是否对称（容差 1e-10）。
fn is_symmetric(m: &DMatrix<f64>) -> bool {
    if !m.is_square() {
        return false;
    }
    let n = m.nrows();
    const TOL: f64 = 1e-10;
    for i in 0..n {
        for j in (i + 1)..n {
            let scale = m[(i, j)].abs().max(m[(j, i)].abs()).max(1.0);
            if (m[(i, j)] - m[(j, i)]).abs() > TOL * scale {
                return false;
            }
        }
    }
    true
}

/// Pade 近似矩阵指数（缩放-平方法）。
///
/// 选择缩放因子 s 使 ||A/2^s|| < 0.5，然后计算 6 阶 Pade 近似，
/// 最后平方回 s 次。
fn pade_exp(matrix: &DMatrix<f64>) -> Result<DMatrix<f64>, CalcError> {
    let n = matrix.nrows();
    let norm = matrix.norm();
    // 选择缩放次数
    let s = if norm <= 0.5 {
        0
    } else {
        (norm / 0.5).log2().ceil() as usize
    };
    let scaled = matrix / (1i64 << s) as f64;
    // 6 阶 Pade 近似：(D + N) / (D - N)，其中
    // Pade 系数 c_k = (2p-k)! p! / ((2p)! k! (p-k)!)，p=6
    let i = DMatrix::<f64>::identity(n, n);
    let a2 = &scaled * &scaled;
    let a4 = &a2 * &a2;
    let a6 = &a4 * &a2;
    // Pade[6/6] 系数
    let b = [
        1.0,                          // c0
        0.5,                          // c1
        1.0 / 9.0,                    // c2 = 1/9
        1.0 / 72.0,                   // c3
        1.0 / 1008.0,                 // c4
        1.0 / 30240.0,                // c5
        1.0 / 1209600.0,              // c6
    ];
    let n_mat = &i * b[0] + &scaled * b[1] + &a2 * b[2] + &a4 * b[3] + &a6 * b[4]
        + &(&a6 * &scaled) * b[5]
        + &(&a6 * &a2) * b[6];
    let d_mat = &i * b[0] - &scaled * b[1] + &a2 * b[2] - &a4 * b[3] + &a6 * b[4]
        - &(&a6 * &scaled) * b[5]
        + &(&a6 * &a2) * b[6];
    // 求解 D_mat · result = N_mat → result = D_mat⁻¹ · N_mat
    let lu = LU::new(d_mat);
    let mut result = lu.solve(&n_mat).ok_or_else(|| {
        CalcError::domain("matrix_exp(): Pade denominator is singular".to_string())
    })?;
    // 平方回
    for _ in 0..s {
        result = &result * &result;
    }
    Ok(result)
}

/// 校验元素全部有限（非 NaN/Inf）。
fn require_finite(values: impl IntoIterator<Item = f64>) -> Result<(), CalcError> {
    if values.into_iter().all(|x| x.is_finite()) {
        Ok(())
    } else {
        Err(CalcError::nan_or_inf())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ErrorKind;

    /// 测试辅助：两矩阵逐元素近似相等。
    fn assert_matrices_approx(a: &DMatrix<f64>, b: &DMatrix<f64>, tol: f64) {
        assert_eq!(a.shape(), b.shape(), "shape mismatch");
        for i in 0..a.nrows() {
            for j in 0..a.ncols() {
                assert!(
                    (a[(i, j)] - b[(i, j)]).abs() < tol,
                    "mismatch at ({},{}): {} vs {}",
                    i,
                    j,
                    a[(i, j)],
                    b[(i, j)]
                );
            }
        }
    }

    // ===== eig =====

    #[test]
    fn eig_symmetric_2x2() {
        let m = DMatrix::from_row_slice(2, 2, &[2.0, 1.0, 1.0, 2.0]);
        let (values, vectors) = eig(&m).unwrap();
        // 升序 [1, 3]
        assert!((values[0] - 1.0).abs() < 1e-9);
        assert!((values[1] - 3.0).abs() < 1e-9);
        // 特征关系 M·v = λ·v
        for (i, &val) in values.iter().enumerate() {
            let v: DVector<f64> = vectors.column(i).into_owned();
            let mv = &m * &v;
            let lv = val * &v;
            assert!((mv - lv).norm() < 1e-9);
        }
    }

    #[test]
    fn eig_non_square_errors() {
        let m = DMatrix::from_row_slice(2, 3, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        assert!(matches!(eig(&m), Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn eig_non_symmetric_errors() {
        let m = DMatrix::from_row_slice(2, 2, &[1.0, 2.0, 3.0, 4.0]);
        assert!(matches!(eig(&m), Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn eig_rejects_nan() {
        let m = DMatrix::from_row_slice(2, 2, &[1.0, f64::NAN, 2.0, 4.0]);
        assert!(matches!(eig(&m), Err(e) if e.kind == ErrorKind::NaNOrInf));
    }

    // ===== svd =====

    #[test]
    fn svd_2x2_reconstructs() {
        let m = DMatrix::from_row_slice(2, 2, &[1.0, 2.0, 3.0, 4.0]);
        let (u, s, vt) = svd(&m).unwrap();
        assert!(s[0] >= s[1]); // 降序
        let k = s.len();
        let mut diag = DMatrix::<f64>::zeros(k, k);
        for i in 0..k {
            diag[(i, i)] = s[i];
        }
        let recon = &u * &diag * &vt;
        assert_matrices_approx(&recon, &m, 1e-9);
    }

    #[test]
    fn svd_rejects_inf() {
        let m = DMatrix::from_row_slice(2, 2, &[1.0, f64::INFINITY, 2.0, 4.0]);
        assert!(matches!(svd(&m), Err(e) if e.kind == ErrorKind::NaNOrInf));
    }

    // ===== lu =====

    #[test]
    fn lu_2x2_pa_equals_lu() {
        let m = DMatrix::from_row_slice(2, 2, &[1.0, 2.0, 3.0, 4.0]);
        let (l, u, p) = lu(&m).unwrap();
        assert_matrices_approx(&(&p * &m), &(&l * &u), 1e-9);
    }

    #[test]
    fn lu_non_square_errors() {
        let m = DMatrix::from_row_slice(2, 3, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        assert!(matches!(lu(&m), Err(e) if e.kind == ErrorKind::Domain));
    }

    // ===== qr =====

    #[test]
    fn qr_2x2_reconstructs() {
        let m = DMatrix::from_row_slice(2, 2, &[1.0, 2.0, 3.0, 4.0]);
        let (q, r) = qr(&m).unwrap();
        assert_matrices_approx(&(&q * &r), &m, 1e-9);
        // Q 正交
        let qtq = q.transpose() * &q;
        assert_matrices_approx(&qtq, &DMatrix::<f64>::identity(q.ncols(), q.ncols()), 1e-9);
    }

    // ===== solve =====

    #[test]
    fn solve_2x2() {
        let a = DMatrix::from_row_slice(2, 2, &[2.0, 1.0, 1.0, 3.0]);
        let b = DVector::from_row_slice(&[1.0, 2.0]);
        let x = solve(&a, &b).unwrap();
        let diff = &a * &x - &b;
        assert!(diff.norm() < 1e-9);
    }

    #[test]
    fn solve_singular_errors() {
        let a = DMatrix::from_row_slice(2, 2, &[1.0, 2.0, 2.0, 4.0]);
        let b = DVector::from_row_slice(&[1.0, 2.0]);
        assert!(matches!(solve(&a, &b), Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn solve_rejects_nan_b() {
        let a = DMatrix::from_row_slice(2, 2, &[1.0, 2.0, 3.0, 4.0]);
        let b = DVector::from_row_slice(&[1.0, f64::NAN]);
        assert!(matches!(solve(&a, &b), Err(e) if e.kind == ErrorKind::NaNOrInf));
    }

    // ===== matrix_exp =====

    #[test]
    fn matrix_exp_zero_is_identity() {
        let z = DMatrix::<f64>::zeros(2, 2);
        let result = matrix_exp(&z).unwrap();
        assert_matrices_approx(&result, &DMatrix::<f64>::identity(2, 2), 1e-9);
    }

    #[test]
    fn matrix_exp_identity_is_e_times_i() {
        let i = DMatrix::<f64>::identity(2, 2);
        let result = matrix_exp(&i).unwrap();
        let expected = std::f64::consts::E * DMatrix::<f64>::identity(2, 2);
        assert_matrices_approx(&result, &expected, 1e-9);
    }

    #[test]
    fn matrix_exp_diagonal() {
        let d = DMatrix::from_row_slice(2, 2, &[1.0, 0.0, 0.0, 2.0]);
        let result = matrix_exp(&d).unwrap();
        let expected = DMatrix::from_row_slice(2, 2, &[
            1.0_f64.exp(),
            0.0,
            0.0,
            2.0_f64.exp(),
        ]);
        assert_matrices_approx(&result, &expected, 1e-9);
    }

    #[test]
    fn matrix_exp_non_square_errors() {
        let m = DMatrix::from_row_slice(2, 3, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        assert!(matches!(matrix_exp(&m), Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn matrix_exp_rejects_nan() {
        let m = DMatrix::from_row_slice(2, 2, &[f64::NAN, 0.0, 0.0, 1.0]);
        assert!(matches!(matrix_exp(&m), Err(e) if e.kind == ErrorKind::NaNOrInf));
    }
}
