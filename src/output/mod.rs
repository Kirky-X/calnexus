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
#[cfg(any(feature = "cli", feature = "server"))]
#[allow(unused_imports)] // 仅 cli 构建中由输出层消费
pub(crate) use latex::format_latex;
#[cfg(feature = "server")]
pub(crate) use latex::format_latex_ast;
#[cfg(any(feature = "cli", feature = "server"))]
pub(crate) use steps::generate_steps;
