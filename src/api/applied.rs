// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! AppliedMath trait 实现。

use crate::api::CalNexus;
use crate::core::{CalcError, EvalResult};

/// AppliedMath API 访问器。
pub struct AppliedMathImpl<'a> {
    pub(crate) cn: &'a CalNexus,
}

impl<'a> AppliedMathImpl<'a> {
    // ── 时间 ──

    #[cfg(feature = "time")]
    pub fn now(&self) -> Result<EvalResult, CalcError> {
        let zoned = jiff::Zoned::now();
        Ok(EvalResult::Scalar(
            zoned.timestamp().as_second() as f64,
        ))
    }

    #[cfg(feature = "time")]
    pub fn today(&self) -> Result<EvalResult, CalcError> {
        let date = jiff::Zoned::now().date();
        // 返回 YYYY-MM-DD 字符串的序号表示
        let s = date.to_string();
        Ok(EvalResult::Symbolic(s))
    }

    #[cfg(feature = "time")]
    pub fn date_add(&self, _date: &str, _expr: &str) -> Result<EvalResult, CalcError> {
        Err(CalcError::domain(
            "date_add not yet available via direct API; use evaluate()".to_string(),
        ))
    }

    #[cfg(feature = "time")]
    pub fn date_diff(&self, _a: &str, _b: &str) -> Result<EvalResult, CalcError> {
        Err(CalcError::domain(
            "date_diff not yet available via direct API; use evaluate()".to_string(),
        ))
    }

    // ── 单位 ──

    #[cfg(feature = "unit")]
    pub fn convert(&self, value: f64, from: &str, to: &str) -> Result<EvalResult, CalcError> {
        crate::math::unit::convert_value(value, from, to).map(EvalResult::Scalar)
    }

    // ── 汇率 ──

    #[cfg(feature = "fx")]
    pub fn fx(&self, _amount: f64, _from: &str, _to: &str) -> Result<EvalResult, CalcError> {
        Err(CalcError::domain(
            "fx not yet available via direct API; use evaluate()".to_string(),
        ))
    }

    #[cfg(feature = "fx")]
    pub fn fx_rate(&self, _from: &str, _to: &str) -> Result<EvalResult, CalcError> {
        Err(CalcError::domain(
            "fx_rate not yet available via direct API; use evaluate()".to_string(),
        ))
    }
}
