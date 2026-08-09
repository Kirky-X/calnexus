// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! DataAnalysis trait 实现。

use crate::api::CalNexus;
use crate::core::{CalcError, EvalResult};

/// DataAnalysis API 访问器。
pub struct DataAnalysisImpl<'a> {
    pub(crate) cn: &'a CalNexus,
}

impl<'a> DataAnalysisImpl<'a> {
    /// 均值。
    pub fn mean(&self, data: &[f64]) -> Result<EvalResult, CalcError> {
        Ok(EvalResult::Scalar(crate::math::statistics::mean(data)))
    }

    /// 标准差。
    pub fn std(&self, data: &[f64]) -> Result<EvalResult, CalcError> {
        Ok(EvalResult::Scalar(crate::math::statistics::std(data)))
    }
}
