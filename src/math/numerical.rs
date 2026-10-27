// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! 数值线性代数核心：特征分解/SVD/LU/QR/solve/矩阵指数，供 domains 与 api 共用。
//!
//! 所有函数入口先 `require_finite` 拦截 NaN/Inf。

#![cfg(feature = "numerical")]

use crate::core::CalcError;
use nalgebra::{DMatrix, DVector, LU, QR, SVD, SymmetricEigen};
use num_complex::Complex;

/// 实对称矩阵特征分解 →（特征值升序, 特征向量列对应）；非方阵或非对称返回 DomainError。
pub fn eig(matrix: &DMatrix<f64>) -> Result<(Vec<f64>, DMatrix<f64>), CalcError> {
    require_finite(matrix.iter().copied())?;
    if !matrix.is_square() {
        return Err(CalcError::domain(format!(
            "eig() requires a square matrix, got {}x{}",
            matrix.nrows(),
            matrix.ncols()
        ))
        .with_i18n(
            "msg.numerical.eig_requires_square",
            vec![
                ("rows".to_string(), matrix.nrows().to_string()),
                ("cols".to_string(), matrix.ncols().to_string()),
            ],
        ));
    }
    const SYMMETRY_TOL: f64 = 1e-10;
    let n = matrix.nrows();
    for i in 0..n {
        for j in (i + 1)..n {
            let scale = matrix[(i, j)].abs().max(matrix[(j, i)].abs()).max(1.0);
            if (matrix[(i, j)] - matrix[(j, i)]).abs() > SYMMETRY_TOL * scale {
                return Err(CalcError::domain(
                    "eig() requires a real symmetric matrix".to_string(),
                )
                .with_hint("use eig_general for non-symmetric matrices")
                .with_i18n("msg.numerical.eig_requires_symmetric", vec![]));
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

/// 实矩阵特征分解结果：特征值与对应特征向量（列序一致）。
pub struct EigGeneral {
    /// 特征值排序分路径：对称路径（委托 [`eig`]）按实部升序；
    /// 一般路径按字典序 `(re, im)`（`total_cmp`）。
    pub values: Vec<Complex<f64>>,
    /// 特征向量：`vectors[i]` 对应 `values[i]`，长度 n。
    pub vectors: Vec<Vec<Complex<f64>>>,
}

/// 实矩阵特征值与特征向量：对称委托 [`eig`]，非对称走实 Schur + 零空间回代。
pub fn eig_general(matrix: &DMatrix<f64>) -> Result<EigGeneral, CalcError> {
    require_finite(matrix.iter().copied())?;
    if !matrix.is_square() {
        return Err(CalcError::domain(format!(
            "eig_general() requires a square matrix, got {}x{}",
            matrix.nrows(),
            matrix.ncols()
        ))
        .with_i18n(
            "msg.numerical.eig_requires_square",
            vec![
                ("rows".to_string(), matrix.nrows().to_string()),
                ("cols".to_string(), matrix.ncols().to_string()),
            ],
        ));
    }
    let n = matrix.nrows();
    if is_symmetric_within(matrix) {
        let (values, vecs) = eig(matrix)?;
        return Ok(EigGeneral {
            values: values
                .into_iter()
                .map(|v| Complex::new(normalize_zero(v), 0.0))
                .collect(),
            vectors: (0..n)
                .map(|col| {
                    (0..n)
                        .map(|row| Complex::new(normalize_zero(vecs[(row, col)]), 0.0))
                        .collect()
                })
                .collect(),
        });
    }

    let schur = nalgebra::linalg::Schur::new(matrix.clone());
    let (q, t) = schur.unpack();
    let raw_values = schur_block_eigenvalues(&t);
    let t_c = DMatrix::<Complex<f64>>::from_fn(n, n, |i, j| Complex::new(t[(i, j)], 0.0));
    let q_c = DMatrix::<Complex<f64>>::from_fn(n, n, |i, j| Complex::new(q[(i, j)], 0.0));

    let mut pairs: Vec<(Complex<f64>, Vec<Complex<f64>>)> = raw_values
        .iter()
        .map(|lambda| {
            let mut shifted = t_c.clone();
            for i in 0..n {
                shifted[(i, i)] -= lambda;
            }
            let y = null_vector_complex(&shifted);
            let y_vec = nalgebra::DVector::from_vec(y);
            let x = &q_c * y_vec;
            (
                *lambda,
                normalize_complex_vector(x.iter().copied().collect()),
            )
        })
        .collect();

    pairs.sort_by(|(a, _), (b, _)| a.re.total_cmp(&b.re).then(a.im.total_cmp(&b.im)));
    let (values, vectors) = pairs.into_iter().unzip();
    Ok(EigGeneral { values, vectors })
}

/// 高斯消元求复矩阵的零空间向量（自由变量取第一个非主元列）。
fn null_vector_complex(matrix: &DMatrix<Complex<f64>>) -> Vec<Complex<f64>> {
    let n = matrix.nrows();
    let mut a = matrix.clone();
    let mut pivot_cols: Vec<usize> = Vec::new();
    let mut rank = 0usize;
    for col in 0..n {
        let mut pivot = None;
        let mut best = 0.0;
        for r in rank..n {
            let v = a[(r, col)].norm();
            if v > best {
                best = v;
                pivot = Some(r);
            }
        }
        let Some(p) = pivot else { continue };
        if best < 1e-12 {
            continue;
        }
        if p != rank {
            a.swap_rows(p, rank);
        }
        let d = a[(rank, col)];
        for r in (rank + 1)..n {
            let factor = a[(r, col)] / d;
            if factor.norm() > 0.0 {
                for c in col..n {
                    let sub = factor * a[(rank, c)];
                    a[(r, c)] -= sub;
                }
            }
        }
        pivot_cols.push(col);
        rank += 1;
        if rank == n {
            break;
        }
    }
    // 满秩即无零空间方向：此时自由变量回退到 n-1 会产出一个与矩阵无关的
    // 伪向量（实测 A·v 明显非零），归一化后仍是错的「特征向量」。
    // 返回零向量作为哨兵，由调用方按残差判定是否可用。
    if pivot_cols.len() == n {
        return vec![Complex::new(0.0, 0.0); n];
    }
    let mut y = vec![Complex::new(0.0, 0.0); n];
    let free = (0..n).find(|c| !pivot_cols.contains(c)).unwrap_or(n - 1);
    y[free] = Complex::new(1.0, 0.0);
    for (row, &col) in pivot_cols.iter().enumerate().rev() {
        let mut sum = Complex::new(0.0, 0.0);
        for j in (col + 1)..n {
            sum += a[(row, j)] * y[j];
        }
        y[col] = -sum / a[(row, col)];
    }
    y
}

/// 从实 Schur 形式 T 的 1×1/2×2 对角块提取特征值（与块顺序一致）。
fn schur_block_eigenvalues(t: &DMatrix<f64>) -> Vec<Complex<f64>> {
    let n = t.nrows();
    let mut values = Vec::with_capacity(n);
    let mut i = 0;
    while i < n {
        if i + 1 < n && t[(i + 1, i)].abs() > 1e-12 {
            let a = t[(i, i)];
            let b = t[(i, i + 1)];
            let c = t[(i + 1, i)];
            let d = t[(i + 1, i + 1)];
            let tr = a + d;
            let det = a * d - b * c;
            let disc = tr * tr - 4.0 * det;
            if disc < 0.0 {
                let re = tr / 2.0;
                let im = (-disc).sqrt() / 2.0;
                values.push(Complex::new(re, im));
                values.push(Complex::new(re, -im));
            } else {
                let root = disc.sqrt() / 2.0;
                values.push(Complex::new(tr / 2.0 + root, 0.0));
                values.push(Complex::new(tr / 2.0 - root, 0.0));
            }
            i += 2;
        } else {
            values.push(Complex::new(t[(i, i)], 0.0));
            i += 1;
        }
    }
    values
}

/// 复向量归一化：除以最大模分量；实特征值向量的微小虚部置 0。
fn normalize_complex_vector(mut x: Vec<Complex<f64>>) -> Vec<Complex<f64>> {
    let max_re = x.iter().map(|c| c.re.abs()).fold(0.0, f64::max);
    let max_im = x.iter().map(|c| c.im.abs()).fold(0.0, f64::max);
    if max_im <= 1e-12 * max_re.max(1.0) {
        for c in &mut x {
            c.im = 0.0;
        }
    }
    let scale = x.iter().map(|c| c.norm()).fold(0.0, f64::max).max(1e-300);
    for c in &mut x {
        *c /= scale;
    }
    x
}

/// 对称性判定（相对尺度 1e-10）：方阵对角线两侧元素近似相等。
fn is_symmetric_within(matrix: &DMatrix<f64>) -> bool {
    const SYMMETRY_TOL: f64 = 1e-10;
    let n = matrix.nrows();
    for i in 0..n {
        for j in (i + 1)..n {
            let scale = matrix[(i, j)].abs().max(matrix[(j, i)].abs()).max(1.0);
            if (matrix[(i, j)] - matrix[(j, i)]).abs() > SYMMETRY_TOL * scale {
                return false;
            }
        }
    }
    true
}

/// 将 `-0.0` 规范为 `0.0`，保证 JSON 与快照输出稳定。
fn normalize_zero(x: f64) -> f64 {
    if x == 0.0 { 0.0 } else { x }
}

/// SVD 三元组：(U, 奇异值降序, Vt)，满足 A = U·diag(S)·Vt。
pub type SvdTriple = (DMatrix<f64>, Vec<f64>, DMatrix<f64>);
/// LU 三元组：(L, U, P)。
pub type LuTriple = (DMatrix<f64>, DMatrix<f64>, DMatrix<f64>);

pub fn svd(matrix: &DMatrix<f64>) -> Result<SvdTriple, CalcError> {
    require_finite(matrix.iter().copied())?;
    let decomp = SVD::new(matrix.clone(), true, true);
    let u = decomp.u.ok_or_else(|| {
        CalcError::domain("SVD decomposition failed: U not available").with_i18n(
            "msg.numerical.svd_failed",
            vec![("component".to_string(), "U".to_string())],
        )
    })?;
    let vt = decomp.v_t.ok_or_else(|| {
        CalcError::domain("SVD decomposition failed: Vt not available").with_i18n(
            "msg.numerical.svd_failed",
            vec![("component".to_string(), "Vt".to_string())],
        )
    })?;
    let s: Vec<f64> = decomp.singular_values.iter().copied().collect();
    Ok((u, s, vt))
}

/// LU 分解 → (L, U, P)，满足 P·A = L·U；要求方阵。
pub fn lu(matrix: &DMatrix<f64>) -> Result<LuTriple, CalcError> {
    require_finite(matrix.iter().copied())?;
    if !matrix.is_square() {
        return Err(CalcError::domain(format!(
            "lu() requires a square matrix, got {}x{}",
            matrix.nrows(),
            matrix.ncols()
        ))
        .with_i18n(
            "msg.numerical.lu_requires_square",
            vec![
                ("rows".to_string(), matrix.nrows().to_string()),
                ("cols".to_string(), matrix.ncols().to_string()),
            ],
        ));
    }
    let n = matrix.nrows();
    let decomp = LU::new(matrix.clone());
    let l = decomp.l();
    let u = decomp.u();
    let mut p_mat = DMatrix::<f64>::identity(n, n);
    decomp.p().permute_rows(&mut p_mat);
    Ok((l, u, p_mat))
}

/// QR 分解（Householder 瘦 QR）→ (Q, R)，满足 A = Q·R。
pub fn qr(matrix: &DMatrix<f64>) -> Result<(DMatrix<f64>, DMatrix<f64>), CalcError> {
    require_finite(matrix.iter().copied())?;
    let decomp = QR::new(matrix.clone());
    let q = decomp.q();
    let r = decomp.r();
    Ok((q, r))
}

/// 最小二乘 lstsq(A, b)：QR 主路径回代；R 对角近零（秩亏）时回退 SVD 伪逆返回最小范数解。
pub fn lstsq(a: &DMatrix<f64>, b: &DVector<f64>) -> Result<DVector<f64>, CalcError> {
    if a.nrows() != b.nrows() {
        return Err(
            CalcError::domain("lstsq: row count of A must equal length of b".to_string())
                .with_i18n(
                    "msg.numerical.lstsq_row_mismatch",
                    vec![
                        ("dim".to_string(), a.nrows().to_string()),
                        ("value".to_string(), b.len().to_string()),
                    ],
                ),
        );
    }
    if a.nrows() == 0 || a.ncols() == 0 {
        return Err(
            CalcError::domain("lstsq: A must be non-empty".to_string()).with_i18n(
                "msg.numerical.lstsq_empty_matrix",
                vec![
                    ("dim".to_string(), a.nrows().to_string()),
                    ("value".to_string(), a.ncols().to_string()),
                ],
            ),
        );
    }
    // QR 回代要求 R 是指向解空间的上三角：仅当 m >= n 成立（R 为 m×n，
    // 对角线元素 i<n 全部落在前 n 行内）。欠定系统 m < n 时 R 只有 m 行，
    // r[(i,i)] 与 qtb[i] 在 i>=m 越界 panic，且最小范数解本就需 SVD，
    // 故直接跳过 QR 走下方 SVD 伪逆。
    // tol 在两条分支共用：秩判定的尺度取 QR 路径的可视对角，欠定时退化为
    // 全矩阵最大绝对值，避免依赖不可用的 R 对角线。
    let m = a.nrows();
    let n = a.ncols();
    let tol = if m >= n {
        let (q, r) = qr(a)?;
        let diag_max = (0..n).map(|i| r[(i, i)].abs()).fold(0.0_f64, f64::max);
        let tol = 1e-12 * diag_max.max(1.0);
        let rank_deficient = (0..n).any(|i| r[(i, i)].abs() <= tol);
        if !rank_deficient {
            let qtb = q.transpose() * b;
            let mut x = DVector::<f64>::zeros(n);
            for i in (0..n).rev() {
                let mut sum = qtb[i];
                for j in (i + 1)..n {
                    sum -= r[(i, j)] * x[j];
                }
                x[i] = sum / r[(i, i)];
            }
            require_finite(x.iter().copied())?;
            return Ok(x);
        }
        tol
    } else {
        let scale = a.iter().fold(0.0_f64, |acc, v| acc.max(v.abs()));
        1e-12 * scale.max(1.0)
    };
    let (u, s, vt) = svd(a)?;
    let mut x = DVector::<f64>::zeros(a.ncols());
    for i in 0..s.len() {
        if s[i] > tol {
            let coef = u.column(i).dot(b) / s[i];
            for j in 0..a.ncols() {
                x[j] += vt[(i, j)] * coef;
            }
        }
    }
    require_finite(x.iter().copied())?;
    Ok(x)
}

/// 解线性方程组 A·x = b；A 须方阵且行数与 b 匹配，奇异返回 DomainError。
pub fn solve(matrix: &DMatrix<f64>, b: &DVector<f64>) -> Result<DVector<f64>, CalcError> {
    require_finite(matrix.iter().copied())?;
    require_finite(b.iter().copied())?;
    if !matrix.is_square() {
        return Err(CalcError::domain(format!(
            "solve() requires a square coefficient matrix, got {}x{}",
            matrix.nrows(),
            matrix.ncols()
        ))
        .with_i18n(
            "msg.numerical.solve_requires_square",
            vec![
                ("rows".to_string(), matrix.nrows().to_string()),
                ("cols".to_string(), matrix.ncols().to_string()),
            ],
        ));
    }
    if b.len() != matrix.nrows() {
        return Err(CalcError::domain(format!(
            "solve() dimension mismatch: A is {}x{} but b has {} entries",
            matrix.nrows(),
            matrix.ncols(),
            b.len()
        ))
        .with_i18n(
            "msg.numerical.solve_dim_mismatch",
            vec![
                ("rows".to_string(), matrix.nrows().to_string()),
                ("cols".to_string(), matrix.ncols().to_string()),
                ("len".to_string(), b.len().to_string()),
            ],
        ));
    }
    let lu_decomp = LU::new(matrix.clone());
    lu_decomp.solve(b).ok_or_else(|| {
        CalcError::domain("solve(): coefficient matrix is singular".to_string())
            .with_i18n("msg.numerical.solve_singular", vec![])
    })
}

/// 矩阵指数 exp(A)：实对称走特征分解，非对称方阵走 Pade 缩放-平方法。
pub fn matrix_exp(matrix: &DMatrix<f64>) -> Result<DMatrix<f64>, CalcError> {
    require_finite(matrix.iter().copied())?;
    if !matrix.is_square() {
        return Err(CalcError::domain(format!(
            "matrix_exp() requires a square matrix, got {}x{}",
            matrix.nrows(),
            matrix.ncols()
        ))
        .with_i18n(
            "msg.numerical.matrix_exp_requires_square",
            vec![
                ("rows".to_string(), matrix.nrows().to_string()),
                ("cols".to_string(), matrix.ncols().to_string()),
            ],
        ));
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

/// Pade 近似矩阵指数：选缩放因子 s 使 ||A/2^s|| < 0.5，6 阶近似后平方回 s 次。
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
    // 6 阶 Pade 近似：(D + N) / (D - N)
    let i = DMatrix::<f64>::identity(n, n);
    let a2 = &scaled * &scaled;
    let a4 = &a2 * &a2;
    let a6 = &a4 * &a2;
    // Pade[6/6] 系数
    let b = [
        1.0,             // c0
        0.5,             // c1
        1.0 / 9.0,       // c2 = 1/9
        1.0 / 72.0,      // c3
        1.0 / 1008.0,    // c4
        1.0 / 30240.0,   // c5
        1.0 / 1209600.0, // c6
    ];
    let n_mat = &i * b[0]
        + &scaled * b[1]
        + &a2 * b[2]
        + &a4 * b[3]
        + &a6 * b[4]
        + &(&a6 * &scaled) * b[5]
        + &(&a6 * &a2) * b[6];
    let d_mat = &i * b[0] - &scaled * b[1] + &a2 * b[2] - &a4 * b[3] + &a6 * b[4]
        - &(&a6 * &scaled) * b[5]
        + &(&a6 * &a2) * b[6];
    // 求解 D_mat · result = N_mat → result = D_mat⁻¹ · N_mat
    let lu = LU::new(d_mat);
    let mut result = lu.solve(&n_mat).ok_or_else(|| {
        CalcError::domain("matrix_exp(): Pade denominator is singular".to_string())
            .with_i18n("msg.numerical.matrix_exp_pade_singular", vec![])
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

    // ===== eig_general（非对称） =====

    /// 断言 `A·x = λ·x`（逐分量残差 < 1e-9）。
    fn assert_eigenpair(m: &DMatrix<f64>, lambda: Complex<f64>, x: &[Complex<f64>]) {
        let n = m.nrows();
        assert_eq!(x.len(), n);
        for i in 0..n {
            let ax = (0..n).fold(Complex::new(0.0, 0.0), |acc, j| {
                acc + Complex::new(m[(i, j)], 0.0) * x[j]
            });
            let lx = lambda * x[i];
            assert!(
                (ax - lx).norm() < 1e-9,
                "row {i}: A·x = {ax:?}, λ·x = {lx:?}"
            );
        }
    }

    #[test]
    fn eig_general_rotation_returns_conjugate_pair() {
        let m = DMatrix::from_row_slice(2, 2, &[0.0, -1.0, 1.0, 0.0]);
        let result = eig_general(&m).unwrap();
        let values = &result.values;
        assert_eq!(values.len(), 2);
        assert!(values[0].re.abs() < 1e-9);
        assert!((values[0].im + 1.0).abs() < 1e-9);
        assert!(values[1].re.abs() < 1e-9);
        assert!((values[1].im - 1.0).abs() < 1e-9);
        assert_eq!(result.vectors.len(), 2);
        for (lambda, x) in values.iter().zip(result.vectors.iter()) {
            assert_eigenpair(&m, *lambda, x);
        }
    }

    #[test]
    fn eig_general_upper_triangular_returns_diagonal() {
        let m = DMatrix::from_row_slice(3, 3, &[3.0, 1.0, 2.0, 0.0, -1.0, 4.0, 0.0, 0.0, 2.0]);
        let result = eig_general(&m).unwrap();
        let values = &result.values;
        let mut re: Vec<f64> = values.iter().map(|c| c.re).collect();
        re.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert!((re[0] + 1.0).abs() < 1e-9);
        assert!((re[1] - 2.0).abs() < 1e-9);
        assert!((re[2] - 3.0).abs() < 1e-9);
        assert!(values.iter().all(|c| c.im.abs() < 1e-9));
        for (lambda, x) in values.iter().zip(result.vectors.iter()) {
            assert_eigenpair(&m, *lambda, x);
        }
    }

    #[test]
    fn eig_general_symmetric_matches_eig() {
        let m = DMatrix::from_row_slice(2, 2, &[2.0, 1.0, 1.0, 2.0]);
        let (expected, _) = eig(&m).unwrap();
        let result = eig_general(&m).unwrap();
        assert_eq!(result.values.len(), expected.len());
        for (c, e) in result.values.iter().zip(expected.iter()) {
            assert!((c.re - e).abs() < 1e-9);
            assert!(c.im.abs() < 1e-9);
        }
        for (lambda, x) in result.values.iter().zip(result.vectors.iter()) {
            assert_eigenpair(&m, *lambda, x);
        }
    }

    #[test]
    fn eig_general_diagonal_vectors_are_basis() {
        let m = DMatrix::from_row_slice(3, 3, &[2.0, 0.0, 0.0, 0.0, 5.0, 0.0, 0.0, 0.0, -1.0]);
        let result = eig_general(&m).unwrap();
        for (lambda, x) in result.values.iter().zip(result.vectors.iter()) {
            assert_eigenpair(&m, *lambda, x);
            let max_norm = x.iter().map(|c| c.norm()).fold(0.0, f64::max);
            assert!((max_norm - 1.0).abs() < 1e-9);
        }
    }

    #[test]
    fn eig_general_non_square_errors() {
        let m = DMatrix::from_row_slice(2, 3, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        assert!(matches!(eig_general(&m), Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn eig_general_rejects_nan() {
        let m = DMatrix::from_row_slice(2, 2, &[1.0, f64::NAN, 2.0, 4.0]);
        assert!(matches!(eig_general(&m), Err(e) if e.kind == ErrorKind::NaNOrInf));
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
        let expected = DMatrix::from_row_slice(2, 2, &[1.0_f64.exp(), 0.0, 0.0, 2.0_f64.exp()]);
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

    #[test]
    fn lstsq_overdetermined_line() {
        let a = DMatrix::from_row_slice(3, 2, &[1.0, 1.0, 1.0, 2.0, 1.0, 3.0]);
        let b = DVector::from_vec(vec![6.0, 9.0, 12.0]);
        let x = lstsq(&a, &b).unwrap();
        assert!(
            (x[0] - 3.0).abs() < 1e-9 && (x[1] - 3.0).abs() < 1e-9,
            "x={x:?}"
        );
    }

    #[test]
    fn lstsq_rank_deficient_returns_min_norm() {
        let a = DMatrix::from_row_slice(2, 2, &[1.0, 2.0, 2.0, 4.0]);
        let b = DVector::from_vec(vec![3.0, 6.0]);
        let x = lstsq(&a, &b).unwrap();
        assert!(x.iter().all(|v| v.is_finite()), "x={x:?}");
    }

    #[test]
    fn lstsq_dimension_mismatch_errors() {
        let a = DMatrix::from_row_slice(2, 2, &[1.0, 0.0, 0.0, 1.0]);
        let b = DVector::from_vec(vec![1.0]);
        assert!(matches!(lstsq(&a, &b), Err(e) if e.kind == ErrorKind::Domain));
    }

    /// 欠定系统（m < n）：QR 的 R 只有 m 行，回代索引 i>=m 会越界 panic。
    /// 该路径必须绕过 QR 走 SVD，返回长度 = n 的有限最小范数解。
    #[test]
    fn lstsq_underdetermined_does_not_panic() {
        let a = DMatrix::from_row_slice(2, 3, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let b = DVector::from_vec(vec![1.0, 2.0]);
        let x = lstsq(&a, &b).expect("欠定系统应返回最小范数解");
        assert_eq!(x.len(), 3, "解长度必须等于未知数个数 n");
        assert!(x.iter().all(|v| v.is_finite()), "x={x:?}");
        let residual = (a * x.clone() - b).norm();
        assert!(residual < 1e-9, "residual={residual}");
    }
}
