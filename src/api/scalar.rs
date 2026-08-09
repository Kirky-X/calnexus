// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! ScalarMath trait 实现。

use crate::api::CalNexus;
use crate::core::{CalcError, EvalResult};

/// ScalarMath API 访问器。
pub struct ScalarMathImpl<'a> {
    pub(crate) cn: &'a CalNexus,
}

impl<'a> ScalarMathImpl<'a> {
    /// 加法。
    pub fn add(&self, a: f64, b: f64) -> Result<EvalResult, CalcError> {
        crate::math::arithmetic::add(a, b).map(EvalResult::Scalar)
    }

    /// 减法。
    pub fn sub(&self, a: f64, b: f64) -> Result<EvalResult, CalcError> {
        crate::math::arithmetic::sub(a, b).map(EvalResult::Scalar)
    }

    /// 乘法。
    pub fn mul(&self, a: f64, b: f64) -> Result<EvalResult, CalcError> {
        crate::math::arithmetic::mul(a, b).map(EvalResult::Scalar)
    }

    /// 除法。
    pub fn div(&self, a: f64, b: f64) -> Result<EvalResult, CalcError> {
        crate::math::arithmetic::div(a, b).map(EvalResult::Scalar)
    }

    /// 正弦。
    pub fn sin(&self, x: f64) -> Result<EvalResult, CalcError> {
        crate::math::scientific::sin(x).map(EvalResult::Scalar)
    }

    /// 余弦。
    pub fn cos(&self, x: f64) -> Result<EvalResult, CalcError> {
        crate::math::scientific::cos(x).map(EvalResult::Scalar)
    }
}
