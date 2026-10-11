// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! CalNexus 输出格式化模块。
//!
//! 提供 LaTeX / 步骤 / 规范形式三种输出格式化器。

mod canonical;
mod latex;
mod steps;

#[cfg(feature = "cli")]
pub(crate) use canonical::format_canonical;
#[cfg(feature = "cli")]
pub(crate) use latex::format_latex;
#[cfg(feature = "cli")]
pub(crate) use steps::generate_steps;
