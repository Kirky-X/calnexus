// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! Numerical 线性代数分解（feature-gated，`numerical` feature 启用）。
//!
//! 提供 eig / SVD / LU / QR / solve(Ax=b) 五个分解函数，基于 nalgebra 分解 API。
//! 由 MatrixDomain 委托调用（design D2）；路由入口仍是 MatrixDomain（priority=30）。
//!
//! design.md：
//! - D1：复用 nalgebra（不引入新依赖）
//! - D2：实现拆到本文件（控制 matrix.rs 行数），MatrixDomain 委托
//! - D3：复合返回走 `EvalResult::Json`；各函数接收**已求值的 `DMatrix<f64>`**（MatrixDomain
//!   eval_node 已从 AST 提取），返回 `Result<EvalResult, CalcError>`。
//!
//! nalgebra 0.35 分解 API 真相（T002 闭环）：
//! - `LU::new(m)`：partial row pivoting，满足 P·A = L·U；`p().permute_rows(&mut id)` 得 P 矩阵
//! - `SVD::new(m, compute_u, compute_v)`：三参数
//!
//! 输入净化（规则 12）：所有函数入口先 `require_finite` 拦截 NaN/Inf——NaN 会绕过 eig 对称校验
//! （`(NaN-NaN).abs() > tol` 恒 false）并触发 nalgebra 分解内部 panic。

#![cfg(feature = "numerical")]

use crate::core::{CalcError, EvalResult};
use nalgebra::{DMatrix, DVector, SymmetricEigen, LU, QR, SVD};
use serde_json::{json, Value};

/// T003: LU 分解 → `{"L":[[..]], "U":[[..]], "P":[[..]]}`，满足 P·A = L·U。
///
/// 方阵要求；非方阵返回 DomainError。L 为单位下三角（对角线 1），U 为上三角，
/// P 为置换矩阵（由 nalgebra `PermutationSequence::permute_rows` 应用到单位矩阵构造）。
pub fn lu(matrix: DMatrix<f64>) -> Result<EvalResult, CalcError> {
    require_finite(matrix.iter().copied())?;
    if !matrix.is_square() {
        return Err(CalcError::domain(format!(
            "lu() requires a square matrix, got {}x{}",
            matrix.nrows(),
            matrix.ncols()
        )));
    }
    let n = matrix.nrows();
    let decomp = LU::new(matrix);
    let l = decomp.l();
    let u = decomp.u();
    let mut p_mat = DMatrix::<f64>::identity(n, n);
    decomp.p().permute_rows(&mut p_mat);
    Ok(EvalResult::Json(json!({
        "L": dmatrix_to_json(&l),
        "U": dmatrix_to_json(&u),
        "P": dmatrix_to_json(&p_mat),
    })))
}

/// T004: QR 分解 → `{"Q":[[..]], "R":[[..]]}`，满足 A = Q·R。
///
/// nalgebra 瘦 QR（Householder），对任意形状成立（无 m≥n 要求）。Q 列正交（Q^T·Q=I），
/// R 上三角。Q 形状 m×min(m,n)，R 形状 min(m,n)×n。
pub fn qr(matrix: DMatrix<f64>) -> Result<EvalResult, CalcError> {
    require_finite(matrix.iter().copied())?;
    let decomp = QR::new(matrix);
    let q = decomp.q();
    let r = decomp.r();
    Ok(EvalResult::Json(json!({
        "Q": dmatrix_to_json(&q),
        "R": dmatrix_to_json(&r),
    })))
}

/// T005: 实对称矩阵特征分解 → `{"values":[..], "vectors":[[..]]}`（特征值升序，vectors 列对应）。
///
/// 仅支持实对称矩阵（nalgebra `SymmetricEigen`）。非方阵或非对称返回 DomainError——
/// `SymmetricEigen::new` 内部 `try_new().unwrap()` 对收敛失败会 panic 且**不校验对称性**，
/// 必须调用前显式校验（规则 12：失败显性化，禁止用户输入触发 panic 或默默返回错误结果）。
/// `eigenvalues` 原始未排序，本函数按升序重排（连同 `eigenvectors` 对应列）。
pub fn eig(matrix: DMatrix<f64>) -> Result<EvalResult, CalcError> {
    require_finite(matrix.iter().copied())?;
    if !matrix.is_square() {
        return Err(CalcError::domain(format!(
            "eig() requires a square matrix, got {}x{}",
            matrix.nrows(),
            matrix.ncols()
        )));
    }
    const SYMMETRY_TOL: f64 = 1e-10; // 相对容差系数（避免大数值对称矩阵被绝对容差误判）
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
    let decomp = SymmetricEigen::new(matrix);
    // eigenvalues 未排序 → 按升序重排（连带 eigenvectors 列）
    let mut indexed: Vec<(usize, f64)> = decomp.eigenvalues.iter().copied().enumerate().collect();
    indexed.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    let values: Vec<f64> = indexed.iter().map(|&(_, v)| v).collect();
    let mut sorted_vecs = DMatrix::<f64>::zeros(n, n);
    for (col, &(src, _)) in indexed.iter().enumerate() {
        for row in 0..n {
            sorted_vecs[(row, col)] = decomp.eigenvectors[(row, src)];
        }
    }
    Ok(EvalResult::Json(json!({
        "values": values,
        "vectors": dmatrix_to_json(&sorted_vecs),
    })))
}

/// T006: 奇异值分解 → `{"U":[[..]], "S":[..], "Vt":[[..]]}`，满足 A = U·diag(S)·Vt（S 降序）。
///
/// `SVD::new(m, true, true)` 内部已 `sort_by_singular_values` → 字段降序；`compute_u/v=true`
/// 保证 `u`/`v_t` 为 `Some`（nalgebra 不变式，`expect` 文档化而非吞错）。
pub fn svd(matrix: DMatrix<f64>) -> Result<EvalResult, CalcError> {
    require_finite(matrix.iter().copied())?;
    let decomp = SVD::new(matrix, true, true);
    let u = decomp.u.expect("compute_u=true guarantees U");
    let vt = decomp.v_t.expect("compute_v=true guarantees Vt");
    let s: Vec<f64> = decomp.singular_values.iter().copied().collect();
    Ok(EvalResult::Json(json!({
        "U": dmatrix_to_json(&u),
        "S": s,
        "Vt": dmatrix_to_json(&vt),
    })))
}

/// T007: 解线性方程组 A·x = b → `Vector([..])`（nalgebra LU solve）。
///
/// A 须方阵且与 b 行数匹配，否则 DomainError；A 奇异（`LU::solve` 返回 `None`）→ DomainError
/// （规则 12：失败显性化，禁止静默返回伪解）。
pub fn solve(matrix: DMatrix<f64>, b: DVector<f64>) -> Result<EvalResult, CalcError> {
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
    let lu = LU::new(matrix);
    let x = lu
        .solve(&b)
        .ok_or_else(|| CalcError::domain("solve(): coefficient matrix is singular".to_string()))?;
    Ok(EvalResult::Vector(x.iter().copied().collect()))
}

/// DMatrix → JSON 二维数组（行优先）。复用 matrix.rs evaluate 的 rows 收集模式（规则 8）。
fn dmatrix_to_json(m: &DMatrix<f64>) -> Value {
    let rows: Vec<Vec<f64>> = (0..m.nrows())
        .map(|i| (0..m.ncols()).map(|j| m[(i, j)]).collect())
        .collect();
    json!(rows)
}

/// 校验元素全部有限（非 NaN/Inf）。
///
/// NaN 会绕过 eig 对称校验（`(NaN-NaN).abs() > tol` 恒 false）并可能触发 nalgebra 分解内部
/// `unwrap()` panic；Inf 污染 LU/SVD 迭代产生伪解。须在每函数入口拦截（规则 12）。
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
    use nalgebra::{SymmetricEigen, QR, SVD};

    /// 测试辅助：JSON 二维数组 → DMatrix。
    fn matrix_from_json(v: &Value) -> DMatrix<f64> {
        let rows = v.as_array().expect("expected JSON array");
        let ncols = rows[0].as_array().expect("row not array").len();
        let data: Vec<f64> = rows
            .iter()
            .flat_map(|row| {
                row.as_array()
                    .expect("row not array")
                    .iter()
                    .map(|x| x.as_f64().expect("element not f64"))
            })
            .collect();
        DMatrix::from_row_slice(rows.len(), ncols, &data)
    }

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

    /// T002: nalgebra 0.35 + `features=["std"]` 下 LU/QR/SVD/SymmetricEigen 可用。
    /// 闭环 design R1：无需补 nalgebra feature。
    #[test]
    fn nalgebra_decompose_symbols_available() {
        let m: DMatrix<f64> = DMatrix::from_row_slice(2, 2, &[1.0, 2.0, 3.0, 4.0]);
        let _lu = LU::new(m.clone());
        let _qr = QR::new(m.clone());
        // SVD::new 三参数：(matrix, compute_u, compute_v)
        let _svd = SVD::new(m.clone(), true, true);
        // SymmetricEigen 要求实对称矩阵
        let sym: DMatrix<f64> = DMatrix::from_row_slice(2, 2, &[2.0, 1.0, 1.0, 2.0]);
        let _eig = SymmetricEigen::new(sym);
    }

    /// T003: lu(2x2) 返回 {L,U,P}，P·A = L·U（双重验证：permute_rows + 返回的 P 矩阵）。
    #[test]
    fn lu_2x2_reconstructs_pa_equals_lu() {
        let m = DMatrix::from_row_slice(2, 2, &[1.0, 2.0, 3.0, 4.0]);
        let result = lu(m.clone()).unwrap();
        let json = match result {
            EvalResult::Json(v) => v,
            other => panic!("expected Json, got {:?}", other),
        };
        let l = matrix_from_json(&json["L"]);
        let u = matrix_from_json(&json["U"]);
        let p = matrix_from_json(&json["P"]);

        // 验证 1：nalgebra 原生 permute_rows(A) == L*U
        let mut pa_native = m.clone();
        LU::new(m.clone()).p().permute_rows(&mut pa_native);
        assert_matrices_approx(&pa_native, &(&l * &u), 1e-9);

        // 验证 2：返回的 P 矩阵满足 P*A == L*U
        assert_matrices_approx(&(&p * &m), &(&l * &u), 1e-9);
    }

    /// T003: lu(3x3) 同理（含行置换的非平凡情形）。
    #[test]
    fn lu_3x3_reconstructs() {
        let m = DMatrix::from_row_slice(3, 3, &[2.0, 1.0, 1.0, 1.0, 3.0, 2.0, 1.0, 0.0, 0.0]);
        let result = lu(m.clone()).unwrap();
        let json = match result {
            EvalResult::Json(v) => v,
            other => panic!("expected Json, got {:?}", other),
        };
        let l = matrix_from_json(&json["L"]);
        let u = matrix_from_json(&json["U"]);
        let p = matrix_from_json(&json["P"]);
        // L 单位下三角：对角线全 1
        assert!((l[(0, 0)] - 1.0).abs() < 1e-9);
        assert!((l[(1, 1)] - 1.0).abs() < 1e-9);
        assert!((l[(2, 2)] - 1.0).abs() < 1e-9);
        // U 上三角：下三角为 0
        assert!(u[(1, 0)].abs() < 1e-9);
        assert!(u[(2, 0)].abs() < 1e-9);
        assert!(u[(2, 1)].abs() < 1e-9);
        // P*A == L*U
        assert_matrices_approx(&(&p * &m), &(&l * &u), 1e-9);
    }

    /// T003: lu 返回 JSON 含 L/U/P 三 key。
    #[test]
    fn lu_returns_json_with_l_u_p_keys() {
        let m = DMatrix::from_row_slice(2, 2, &[4.0, 3.0, 6.0, 3.0]);
        let json = match lu(m).unwrap() {
            EvalResult::Json(v) => v,
            other => panic!("expected Json, got {:?}", other),
        };
        let obj = json.as_object().expect("expected JSON object");
        assert!(obj.contains_key("L"));
        assert!(obj.contains_key("U"));
        assert!(obj.contains_key("P"));
        assert_eq!(obj.len(), 3);
    }

    /// T003: lu(非方阵) → DomainError。
    #[test]
    fn lu_non_square_errors() {
        let m = DMatrix::from_row_slice(2, 3, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let result = lu(m);
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    /// T004: qr(2x2) 返回 {Q,R}，Q·R = A + Q^T·Q = I（正交）+ R 上三角。
    #[test]
    fn qr_2x2_reconstructs_and_q_orthogonal() {
        let m = DMatrix::from_row_slice(2, 2, &[1.0, 2.0, 3.0, 4.0]);
        let json = match qr(m.clone()).unwrap() {
            EvalResult::Json(v) => v,
            other => panic!("expected Json, got {:?}", other),
        };
        let q = matrix_from_json(&json["Q"]);
        let r = matrix_from_json(&json["R"]);
        // A = Q·R
        assert_matrices_approx(&(&q * &r), &m, 1e-9);
        // Q^T·Q = I（列正交）
        let k = q.ncols();
        let qtq = q.transpose() * &q;
        assert_matrices_approx(&qtq, &DMatrix::<f64>::identity(k, k), 1e-9);
        // R 上三角（下三角为 0）
        assert!(r[(1, 0)].abs() < 1e-9);
    }

    /// T004: qr(3x3) 经典维基例子（Householder）。
    #[test]
    fn qr_3x3_reconstructs() {
        // Wikipedia QR 经典例子
        let m = DMatrix::from_row_slice(
            3,
            3,
            &[12.0, -51.0, 4.0, 6.0, 167.0, -68.0, -4.0, 24.0, -41.0],
        );
        let json = match qr(m.clone()).unwrap() {
            EvalResult::Json(v) => v,
            other => panic!("expected Json, got {:?}", other),
        };
        let q = matrix_from_json(&json["Q"]);
        let r = matrix_from_json(&json["R"]);
        // A = Q·R
        assert_matrices_approx(&(&q * &r), &m, 1e-7);
        // R 上三角：严格下三角为 0
        assert!(r[(1, 0)].abs() < 1e-7);
        assert!(r[(2, 0)].abs() < 1e-7);
        assert!(r[(2, 1)].abs() < 1e-7);
    }

    /// T004: qr(非方阵 2x3) 瘦分解也成立（nalgebra 无 m≥n 要求）。
    #[test]
    fn qr_non_square_2x3_reconstructs() {
        let m = DMatrix::from_row_slice(2, 3, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let json = match qr(m.clone()).unwrap() {
            EvalResult::Json(v) => v,
            other => panic!("expected Json, got {:?}", other),
        };
        let q = matrix_from_json(&json["Q"]);
        let r = matrix_from_json(&json["R"]);
        // 瘦 QR：Q 是 2x2，R 是 2x3
        assert_eq!(q.shape(), (2, 2));
        assert_eq!(r.shape(), (2, 3));
        // A = Q·R
        assert_matrices_approx(&(&q * &r), &m, 1e-9);
    }

    /// T004: qr 返回 JSON 含 Q/R 两 key。
    #[test]
    fn qr_returns_json_with_q_r_keys() {
        let m = DMatrix::from_row_slice(2, 2, &[1.0, 0.0, 0.0, 1.0]);
        let json = match qr(m).unwrap() {
            EvalResult::Json(v) => v,
            other => panic!("expected Json, got {:?}", other),
        };
        let obj = json.as_object().expect("expected JSON object");
        assert!(obj.contains_key("Q"));
        assert!(obj.contains_key("R"));
        assert_eq!(obj.len(), 2);
    }

    /// T005: eig([[2,1],[1,2]]) → 特征值 {1,3} 升序 + M·v=λ·v 还原。
    #[test]
    fn eig_symmetric_2x2_eigenrelation_and_sorted() {
        let m = DMatrix::from_row_slice(2, 2, &[2.0, 1.0, 1.0, 2.0]);
        let json = match eig(m.clone()).unwrap() {
            EvalResult::Json(v) => v,
            other => panic!("expected Json, got {:?}", other),
        };
        let values: Vec<f64> = json["values"]
            .as_array()
            .expect("values not array")
            .iter()
            .map(|x| x.as_f64().expect("value not f64"))
            .collect();
        let vectors = matrix_from_json(&json["vectors"]);
        // 升序：[1, 3]
        assert!((values[0] - 1.0).abs() < 1e-9);
        assert!((values[1] - 3.0).abs() < 1e-9);
        // M·v_i = λ_i·v_i
        for i in 0..2 {
            let v: nalgebra::DVector<f64> = vectors.column(i).into_owned();
            let mv = &m * &v;
            let lv = values[i] * &v;
            assert!(
                (mv - lv).norm() < 1e-9,
                "eigenrelation failed for col {}",
                i
            );
        }
    }

    /// T005: eig(3x3 对称) 特征值升序 + M·v=λ·v。
    #[test]
    fn eig_symmetric_3x3_eigenrelation() {
        let m = DMatrix::from_row_slice(3, 3, &[4.0, 1.0, 2.0, 1.0, 5.0, 3.0, 2.0, 3.0, 6.0]);
        let json = match eig(m.clone()).unwrap() {
            EvalResult::Json(v) => v,
            other => panic!("expected Json, got {:?}", other),
        };
        let values: Vec<f64> = json["values"]
            .as_array()
            .expect("values not array")
            .iter()
            .map(|x| x.as_f64().expect("value not f64"))
            .collect();
        let vectors = matrix_from_json(&json["vectors"]);
        // 升序
        assert!(values[0] <= values[1] + 1e-9);
        assert!(values[1] <= values[2] + 1e-9);
        // M·v_i = λ_i·v_i
        for i in 0..3 {
            let v: nalgebra::DVector<f64> = vectors.column(i).into_owned();
            let mv = &m * &v;
            let lv = values[i] * &v;
            assert!(
                (mv - lv).norm() < 1e-8,
                "eigenrelation failed for col {}",
                i
            );
        }
    }

    /// T005: eig(非对称) → DomainError（显式校验，避免 SymmetricEigen panic）。
    #[test]
    fn eig_non_symmetric_errors() {
        let m = DMatrix::from_row_slice(2, 2, &[1.0, 2.0, 3.0, 4.0]);
        let result = eig(m);
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    /// T005: eig(非方阵) → DomainError。
    #[test]
    fn eig_non_square_errors() {
        let m = DMatrix::from_row_slice(2, 3, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let result = eig(m);
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    /// T006: svd(2x2) 还原 A = U·diag(S)·Vt + S 降序 + U/Vt 正交。
    #[test]
    fn svd_2x2_reconstructs_and_sorted() {
        let m = DMatrix::from_row_slice(2, 2, &[1.0, 2.0, 3.0, 4.0]);
        let json = match svd(m.clone()).unwrap() {
            EvalResult::Json(v) => v,
            other => panic!("expected Json, got {:?}", other),
        };
        let u = matrix_from_json(&json["U"]);
        let s: Vec<f64> = json["S"]
            .as_array()
            .expect("S not array")
            .iter()
            .map(|x| x.as_f64().expect("sv not f64"))
            .collect();
        let vt = matrix_from_json(&json["Vt"]);
        // S 降序
        assert!(s[0] >= s[1], "singular values not descending: {:?}", s);
        // A = U·diag(S)·Vt
        let k = s.len();
        let mut diag = DMatrix::<f64>::zeros(k, k);
        for i in 0..k {
            diag[(i, i)] = s[i];
        }
        let recon = &u * &diag * &vt;
        assert_matrices_approx(&recon, &m, 1e-9);
        // U 列正交：U^T·U = I
        let utu = u.transpose() * &u;
        assert_matrices_approx(&utu, &DMatrix::<f64>::identity(k, k), 1e-9);
        // Vt 行正交：Vt·Vt^T = I
        let vtvt = &vt * vt.transpose();
        assert_matrices_approx(&vtvt, &DMatrix::<f64>::identity(k, k), 1e-9);
    }

    /// T006: svd(3x2) 瘦分解（m>n）形状 + 还原。
    #[test]
    fn svd_3x2_thin_decomposition() {
        // m=3, n=2 → k=2; U 3×2, S len 2, Vt 2×2
        let m = DMatrix::from_row_slice(3, 2, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let json = match svd(m.clone()).unwrap() {
            EvalResult::Json(v) => v,
            other => panic!("expected Json, got {:?}", other),
        };
        let u = matrix_from_json(&json["U"]);
        let s: Vec<f64> = json["S"]
            .as_array()
            .expect("S not array")
            .iter()
            .map(|x| x.as_f64().expect("sv not f64"))
            .collect();
        let vt = matrix_from_json(&json["Vt"]);
        assert_eq!(u.shape(), (3, 2));
        assert_eq!(s.len(), 2);
        assert_eq!(vt.shape(), (2, 2));
        assert!(s[0] >= s[1]);
        // A = U·diag(S)·Vt
        let mut diag = DMatrix::<f64>::zeros(2, 2);
        for i in 0..2 {
            diag[(i, i)] = s[i];
        }
        let recon = &u * &diag * &vt;
        assert_matrices_approx(&recon, &m, 1e-9);
    }

    /// T006: svd 返回 JSON 含 U/S/Vt 三 key。
    #[test]
    fn svd_returns_json_with_u_s_vt_keys() {
        let m = DMatrix::from_row_slice(2, 2, &[1.0, 0.0, 0.0, 1.0]);
        let json = match svd(m).unwrap() {
            EvalResult::Json(v) => v,
            other => panic!("expected Json, got {:?}", other),
        };
        let obj = json.as_object().expect("expected JSON object");
        assert!(obj.contains_key("U"));
        assert!(obj.contains_key("S"));
        assert!(obj.contains_key("Vt"));
        assert_eq!(obj.len(), 3);
    }

    /// T007: solve(2x2, b) 还原 A·x = b。
    #[test]
    fn solve_2x2_reconstructs() {
        let a = DMatrix::from_row_slice(2, 2, &[2.0, 1.0, 1.0, 3.0]);
        let b = DVector::from_row_slice(&[1.0, 2.0]);
        let result = solve(a.clone(), b.clone()).unwrap();
        let x = match result {
            EvalResult::Vector(v) => v,
            other => panic!("expected Vector, got {:?}", other),
        };
        // A·x = b
        let xv = DVector::from_row_slice(&x);
        let diff = &a * &xv - &b;
        assert!(diff.norm() < 1e-9, "A·x != b: {:?}", x);
    }

    /// T007: solve(3x3, b) 还原 A·x = b。
    #[test]
    fn solve_3x3_reconstructs() {
        let a = DMatrix::from_row_slice(3, 3, &[3.0, 2.0, -1.0, 2.0, -2.0, 0.5, -1.0, 0.5, -1.0]);
        let b = DVector::from_row_slice(&[1.0, -2.0, 0.0]);
        let result = solve(a.clone(), b.clone()).unwrap();
        let x = match result {
            EvalResult::Vector(v) => v,
            other => panic!("expected Vector, got {:?}", other),
        };
        let xv = DVector::from_row_slice(&x);
        let diff = &a * &xv - &b;
        assert!(diff.norm() < 1e-9, "A·x != b: {:?}", x);
    }

    /// T007: solve(奇异矩阵) → DomainError。
    #[test]
    fn solve_singular_errors() {
        // [[1,2],[2,4]] 行成比例 → 秩 1 < 2，奇异
        let a = DMatrix::from_row_slice(2, 2, &[1.0, 2.0, 2.0, 4.0]);
        let b = DVector::from_row_slice(&[1.0, 2.0]);
        let result = solve(a, b);
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    /// T007: solve(维度不匹配) → DomainError。
    #[test]
    fn solve_dim_mismatch_errors() {
        let a = DMatrix::from_row_slice(2, 2, &[1.0, 2.0, 3.0, 4.0]);
        let b = DVector::from_row_slice(&[1.0, 2.0, 3.0]); // len 3 != 2
        let result = solve(a, b);
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    /// T007: solve(非方阵 A) → DomainError。
    #[test]
    fn solve_non_square_a_errors() {
        let a = DMatrix::from_row_slice(2, 3, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let b = DVector::from_row_slice(&[1.0, 2.0]);
        let result = solve(a, b);
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    /// HIGH-1 修复：lu 拒绝 NaN 矩阵（NaN 会绕过对称/奇异校验触发 panic）。
    #[test]
    fn lu_rejects_nan_matrix() {
        let m = DMatrix::from_row_slice(2, 2, &[1.0, f64::NAN, 2.0, 4.0]);
        assert!(matches!(lu(m), Err(e) if e.kind == ErrorKind::NaNOrInf));
    }

    /// HIGH-1 修复：eig 拒绝 Inf 矩阵（Inf 污染 SymmetricEigen 迭代）。
    #[test]
    fn eig_rejects_inf_matrix() {
        let m = DMatrix::from_row_slice(2, 2, &[1.0, f64::INFINITY, f64::INFINITY, 4.0]);
        assert!(matches!(eig(m), Err(e) if e.kind == ErrorKind::NaNOrInf));
    }

    /// HIGH-1 修复：solve 拒绝 NaN 右端向量 b。
    #[test]
    fn solve_rejects_nan_b() {
        let a = DMatrix::from_row_slice(2, 2, &[1.0, 2.0, 3.0, 4.0]);
        let b = DVector::from_row_slice(&[1.0, f64::NAN]);
        assert!(matches!(solve(a, b), Err(e) if e.kind == ErrorKind::NaNOrInf));
    }
}
