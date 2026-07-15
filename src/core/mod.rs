// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! CalNexus 核心引擎：表达式解析、AST 规范化、L1 缓存、域路由。
//!
//! 共享类型定义在 [`types`] 模块中，所有计算域和 CLI 共用这些类型。
//! 表达式解析器在 [`parser`] 模块中，将字符串解析为 [`types::AstNode`]。
//! AST 规范化器在 [`canonicalizer`] 模块中，生成 [`types::CanonicalForm`]。
//! L1 缓存管理器在 [`cache`] 模块中，基于 Moka + BLAKE3。
//! 计算域接口与路由器在 [`domain`] 模块中。

mod cache;
mod canonicalizer;
mod domain;
mod parser;
mod types;

pub use cache::{CacheKeyGen, CacheManager};
pub use canonicalizer::AstCanonicalizer;
pub use domain::{CalculationDomain, DomainRouter};
pub use parser::parse;
#[cfg(feature = "cli")]
pub(crate) use parser::MAX_EXPR_LEN;
pub use types::{
    AstNode, BinaryOp, CalcError, CanonicalForm, ErrorKind, EvalContext, EvalResult, Span, UnaryOp,
    MAX_FACTORIAL_INPUT, MAX_POW_EXPONENT, MAX_PRECISION,
};
// escape_json_string 仅 batch.rs（cli feature）使用，条件导出避免非 cli 下的 unused import
#[cfg(feature = "cli")]
pub use types::escape_json_string;
