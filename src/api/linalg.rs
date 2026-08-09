// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! LinearAlgebra trait 实现。

use nalgebra::DMatrix;

use crate::api::types::{Matrix, Vector};
use crate::api::CalNexus;
use crate::core::{CalcError, EvalResult};
use crate::math;

/// LinearAlgebra API 访问器。
pub struct LinearAlgebraImpl<'a> {
    #[allow(dead_code)]
    pub(crate) cn: &'a CalNexus,
}

// ── 辅助 ──

fn to_dmatrix(m: &Matrix) -> DMatrix<f64> {
    let rows = m.nrows();
    let cols = m.ncols();
    let flat: Vec<f64> = m.as_rows().iter().flatten().copied().collect();
    DMatrix::from_row_slice(rows, cols, &flat)
}

fn dmatrix_to_result(dm: DMatrix<f64>) -> EvalResult {
    let rows: Vec<Vec<f64>> = (0..dm.nrows())
        .map(|r| (0..dm.ncols()).map(|c| dm[(r, c)]).collect())
        .collect();
    EvalResult::Matrix(rows)
}

impl<'a> LinearAlgebraImpl<'a> {
    // ── 矩阵 ──

    pub fn det(&self, m: &Matrix) -> Result<EvalResult, CalcError> {
        math::matrix::det(&to_dmatrix(m)).map(EvalResult::Scalar)
    }

    pub fn inverse(&self, m: &Matrix) -> Result<EvalResult, CalcError> {
        math::matrix::inverse(&to_dmatrix(m)).map(dmatrix_to_result)
    }

    pub fn transpose(&self, m: &Matrix) -> Result<EvalResult, CalcError> {
        Ok(dmatrix_to_result(math::matrix::transpose(&to_dmatrix(m))))
    }

    pub fn identity(&self, n: usize) -> Result<EvalResult, CalcError> {
        math::matrix::identity(n).map(dmatrix_to_result)
    }

    pub fn mat_add(&self, a: &Matrix, b: &Matrix) -> Result<EvalResult, CalcError> {
        math::matrix::mat_add(&to_dmatrix(a), &to_dmatrix(b)).map(dmatrix_to_result)
    }

    pub fn mat_sub(&self, a: &Matrix, b: &Matrix) -> Result<EvalResult, CalcError> {
        math::matrix::mat_sub(&to_dmatrix(a), &to_dmatrix(b)).map(dmatrix_to_result)
    }

    pub fn mat_mul(&self, a: &Matrix, b: &Matrix) -> Result<EvalResult, CalcError> {
        math::matrix::mat_mul(&to_dmatrix(a), &to_dmatrix(b)).map(dmatrix_to_result)
    }

    pub fn scalar_mul(&self, s: f64, m: &Matrix) -> Result<EvalResult, CalcError> {
        Ok(dmatrix_to_result(math::matrix::scalar_mul(
            &to_dmatrix(m),
            s,
        )))
    }

    // ── 向量 ──

    pub fn dot(&self, a: &Vector, b: &Vector) -> Result<EvalResult, CalcError> {
        math::vector::dot(a.as_slice(), b.as_slice()).map(EvalResult::Scalar)
    }

    pub fn cross(&self, a: &Vector, b: &Vector) -> Result<EvalResult, CalcError> {
        math::vector::cross(a.as_slice(), b.as_slice()).map(EvalResult::Vector)
    }

    pub fn normalize(&self, a: &Vector) -> Result<EvalResult, CalcError> {
        math::vector::normalize(a.as_slice()).map(EvalResult::Vector)
    }

    pub fn magnitude(&self, a: &Vector) -> Result<EvalResult, CalcError> {
        Ok(EvalResult::Scalar(math::vector::magnitude(a.as_slice())))
    }

    pub fn vector_add(&self, a: &Vector, b: &Vector) -> Result<EvalResult, CalcError> {
        math::vector::vector_add(a.as_slice(), b.as_slice()).map(EvalResult::Vector)
    }

    pub fn vector_sub(&self, a: &Vector, b: &Vector) -> Result<EvalResult, CalcError> {
        math::vector::vector_sub(a.as_slice(), b.as_slice()).map(EvalResult::Vector)
    }

    // ── 数值分解（feature-gated）──

    #[cfg(feature = "numerical")]
    pub fn eig(&self, m: &Matrix) -> Result<EvalResult, CalcError> {
        let dm = to_dmatrix(m);
        let (values, vectors) = math::numerical::eig(&dm)?;
        let rows: Vec<Vec<f64>> = (0..vectors.nrows())
            .map(|r| (0..vectors.ncols()).map(|c| vectors[(r, c)]).collect())
            .collect();
        Ok(EvalResult::Json(serde_json::json!({
            "values": values,
            "vectors": rows,
        })))
    }

    #[cfg(feature = "numerical")]
    pub fn svd(&self, m: &Matrix) -> Result<EvalResult, CalcError> {
        let dm = to_dmatrix(m);
        let (u, s, vt) = math::numerical::svd(&dm)?;
        let u_rows: Vec<Vec<f64>> = (0..u.nrows())
            .map(|r| (0..u.ncols()).map(|c| u[(r, c)]).collect())
            .collect();
        let vt_rows: Vec<Vec<f64>> = (0..vt.nrows())
            .map(|r| (0..vt.ncols()).map(|c| vt[(r, c)]).collect())
            .collect();
        Ok(EvalResult::Json(serde_json::json!({
            "U": u_rows,
            "S": s,
            "Vt": vt_rows,
        })))
    }

    #[cfg(feature = "numerical")]
    pub fn lu(&self, m: &Matrix) -> Result<EvalResult, CalcError> {
        let dm = to_dmatrix(m);
        let (l, u, p) = math::numerical::lu(&dm)?;
        let to_rows = |m: &nalgebra::DMatrix<f64>| -> Vec<Vec<f64>> {
            (0..m.nrows())
                .map(|r| (0..m.ncols()).map(|c| m[(r, c)]).collect())
                .collect()
        };
        Ok(EvalResult::Json(serde_json::json!({
            "L": to_rows(&l),
            "U": to_rows(&u),
            "P": to_rows(&p),
        })))
    }

    #[cfg(feature = "numerical")]
    pub fn qr(&self, m: &Matrix) -> Result<EvalResult, CalcError> {
        let dm = to_dmatrix(m);
        let (q, r) = math::numerical::qr(&dm)?;
        let to_rows = |m: &nalgebra::DMatrix<f64>| -> Vec<Vec<f64>> {
            (0..m.nrows())
                .map(|r| (0..m.ncols()).map(|c| m[(r, c)]).collect())
                .collect()
        };
        Ok(EvalResult::Json(serde_json::json!({
            "Q": to_rows(&q),
            "R": to_rows(&r),
        })))
    }

    #[cfg(feature = "numerical")]
    pub fn solve(&self, a: &Matrix, b: &Vector) -> Result<EvalResult, CalcError> {
        let da = to_dmatrix(a);
        let db = nalgebra::DVector::from_row_slice(b.as_slice());
        let x = math::numerical::solve(&da, &db)?;
        Ok(EvalResult::Vector(x.iter().copied().collect()))
    }

    #[cfg(feature = "numerical")]
    pub fn matrix_exp(&self, m: &Matrix) -> Result<EvalResult, CalcError> {
        let dm = to_dmatrix(m);
        math::numerical::matrix_exp(&dm).map(dmatrix_to_result)
    }
}
