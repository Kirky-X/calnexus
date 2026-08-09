// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! AppliedMath trait 实现。

use crate::api::CalNexus;
use crate::core::{CalcError, EvalResult};

/// AppliedMath API 访问器。
pub struct AppliedMathImpl<'a> {
    pub(crate) cn: &'a CalNexus,
}

impl<'a> AppliedMathImpl<'a> {
    /// 单位换算。
    #[cfg(feature = "unit")]
    pub fn convert(&self, value: f64, from: &str, to: &str) -> Result<EvalResult, CalcError> {
        crate::math::unit::convert_value(value, from, to).map(EvalResult::Scalar)
    }
}
