// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! LinearAlgebra trait 实现。

use crate::api::types::{Matrix, Vector};
use crate::api::CalNexus;
use crate::core::{CalcError, EvalResult};

/// LinearAlgebra API 访问器。
pub struct LinearAlgebraImpl<'a> {
    pub(crate) cn: &'a CalNexus,
}

impl<'a> LinearAlgebraImpl<'a> {
    /// 矩阵行列式。
    pub fn det(&self, m: &Matrix) -> Result<EvalResult, CalcError> {
        let rows = m.nrows();
        let cols = m.ncols();
        let flat: Vec<f64> = m.as_rows().iter().flatten().copied().collect();
        let dm = nalgebra::DMatrix::from_row_slice(rows, cols, &flat);
        crate::math::matrix::det(&dm).map(EvalResult::Scalar)
    }

    /// 向量点积。
    pub fn dot(&self, a: &Vector, b: &Vector) -> Result<EvalResult, CalcError> {
        crate::math::vector::dot(a.as_slice(), b.as_slice()).map(EvalResult::Scalar)
    }
}
