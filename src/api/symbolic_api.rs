// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! SymbolicMath trait 实现。

use crate::api::CalNexus;
use crate::core::{CalcError, EvalResult};

/// SymbolicMath API 访问器。
pub struct SymbolicMathImpl<'a> {
    pub(crate) cn: &'a CalNexus,
}

impl<'a> SymbolicMathImpl<'a> {
    /// 符号微分（占位）。
    pub fn differentiate(&self, _expr: &str, _var: &str) -> Result<EvalResult, CalcError> {
        Err(CalcError::domain("symbolic differentiate not yet implemented via direct API".to_string()))
    }
}
