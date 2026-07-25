// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! 单位换算表：静态系数量 + 温度仿射函数。
//!
//! 设计依据：design.md D5（8 量纲 SI 基准系数表 + 温度仿射特例）
//! Feature 门控：`unit = []`

/// 量纲枚举：8 个物理量纲。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dimension {
    Length,
    Mass,
    Temperature,
    Volume,
    Area,
    Speed,
    Data,
    Time,
}

// stub — 待 Phase 3 实现完整换算表与温度仿射函数
