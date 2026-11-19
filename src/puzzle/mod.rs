// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! 数学谜题生成器（`puzzle` feature 门控，仅 CLI 编排既有域函数，零新增依赖）。
//!
//! 三类题型共用确定性 RNG 与统一 `Puzzle` DTO，答案均可经既有纯函数复验。

mod arithmetic;
mod calculus;
mod generate;
mod number_theory;
mod rng;

pub use generate::generate;

/// 谜题类型与题目内容。
#[derive(Debug, Clone)]
pub struct Puzzle {
    pub kind: String,
    pub question: String,
    pub answer: String,
    pub steps: Vec<String>,
}
