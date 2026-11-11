// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! 外部框架集成层：把核心能力封装为第三方装配框架的模块。
//!
//! 当前仅含 trait-kit `AsyncKit` 集成（feature `kit` 门控）。

#[cfg(feature = "kit")]
pub mod kit;
