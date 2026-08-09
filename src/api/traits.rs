// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! API trait 定义：5 个分组 trait。

use crate::core::{CalcError, EvalResult};

/// 标量数学 trait：算术 + 科学函数 + 数论 + 组合。
pub trait ScalarMath {
    /// 加法。
    fn add(&self, a: f64, b: f64) -> Result<EvalResult, CalcError>;
    /// 减法。
    fn sub(&self, a: f64, b: f64) -> Result<EvalResult, CalcError>;
    /// 乘法。
    fn mul(&self, a: f64, b: f64) -> Result<EvalResult, CalcError>;
    /// 除法。
    fn div(&self, a: f64, b: f64) -> Result<EvalResult, CalcError>;
    /// 正弦。
    fn sin(&self, x: f64) -> Result<EvalResult, CalcError>;
    /// 余弦。
    fn cos(&self, x: f64) -> Result<EvalResult, CalcError>;
}

/// 线性代数 trait：矩阵/向量运算。
pub trait LinearAlgebra {
    /// 矩阵行列式。
    fn det(&self, m: &super::types::Matrix) -> Result<EvalResult, CalcError>;
    /// 向量点积。
    fn dot(&self, a: &super::types::Vector, b: &super::types::Vector) -> Result<EvalResult, CalcError>;
}

/// 数据分析 trait：统计函数。
pub trait DataAnalysis {
    /// 均值。
    fn mean(&self, data: &[f64]) -> Result<EvalResult, CalcError>;
    /// 标准差。
    fn std(&self, data: &[f64]) -> Result<EvalResult, CalcError>;
}

/// 符号数学 trait：符号微分/多项式/复数。
pub trait SymbolicMath {
    /// 符号微分。
    fn differentiate(&self, expr: &str, var: &str) -> Result<EvalResult, CalcError>;
}

/// 应用数学 trait：时间/单位/汇率（feature-gated）。
pub trait AppliedMath {
    /// 单位换算。
    fn convert(&self, value: f64, from: &str, to: &str) -> Result<EvalResult, CalcError>;
}
