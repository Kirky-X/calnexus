// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! CalNexus 输出格式化模块（v1.1 新增）。
//!
//! 提供 LaTeX / 步骤 / 规范形式三种输出格式化器：
//! - `latex::format_latex`：将 `EvalResult` + `AstNode` 渲染为 LaTeX 字符串
//! - `steps::generate_steps`：遍历 AST 生成逐步求值字符串列表
//! - `canonical::format_canonical`：暴露 `CanonicalForm` 的字符串形式
//!
//! 设计依据：design.md D1 — 独立模块避免 `src/cli.rs` 进一步膨胀。

pub mod canonical;
pub mod latex;
pub mod steps;
