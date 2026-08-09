// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! 核心数学函数层（L3）。
//!
//! 将各计算域的实际数学运算逻辑提取为独立 `pub fn`，
//! 供 `domains/`（AST 求值路径）和 `api/`（直接 API 路径）共用。
//!
//! 依赖方向：`math/` → `core/`（仅依赖 types/error），不依赖 `domains/` 或 `api/`。

pub mod arithmetic;
pub mod combinatorics;
pub mod complex;
pub mod matrix;
pub mod number_theory;
pub mod polynomial;
pub mod precision;
pub mod scientific;
pub mod statistics;
pub mod symbolic;
pub mod vector;

// feature-gated 可选域
#[cfg(feature = "fx")]
pub mod fx;
#[cfg(feature = "numerical")]
pub mod numerical;
#[cfg(feature = "time")]
pub mod time;
#[cfg(feature = "unit")]
pub mod unit;
#[cfg(feature = "unit")]
pub mod unit_table;
