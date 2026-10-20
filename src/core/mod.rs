// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! CalNexus 核心引擎：解析、规范化、L1 缓存、域路由与求值。

mod cache;
mod canonicalizer;
mod domain;
mod evaluator;
mod parser;
mod types;

#[cfg(feature = "cli")]
pub(crate) use cache::DEFAULT_MAX_WEIGHT_BYTES;
pub use cache::{CacheKeyGen, CacheManager};
pub use canonicalizer::AstCanonicalizer;
pub use domain::{CalculationDomain, DomainRouter};
pub use evaluator::{evaluate, evaluate_with_router};
#[cfg(feature = "cli")]
pub(crate) use parser::MAX_EXPR_LEN;
pub use parser::parse;
pub use types::{
    AstNode, BinaryOp, CalcError, CanonicalForm, ErrorKind, EvalContext, EvalResult,
    MAX_FACTORIAL_INPUT, MAX_POW_EXPONENT, MAX_PRECISION, Span, UnaryOp, check_pow_output_size,
};
// `--json` 键序重排出口：仅 CLI 与 wasm-api 两条序列化链消费它，
// server / lib-only 构建下再导出即为未使用。
#[cfg(any(feature = "cli", feature = "wasm-api"))]
pub use types::sort_json_keys;
