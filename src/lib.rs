// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! CalNexus 计算引擎：表达式解析、AST 规范化、L1 缓存、域路由。

// 测试代码中使用 3.14 / 6.283... 等值作为测试输入，并非数学常量的误用。
// 测试函数名保留 P/C 大写以对应排列 (Permutation) / 组合 (Combination) 数学记号。
#![cfg_attr(test, allow(clippy::approx_constant, non_snake_case))]
// 无 CLI feature 时，output/symbolic 中仅 CLI 调用的函数不构成 dead code
#![cfg_attr(not(feature = "cli"), allow(dead_code))]

#[cfg(feature = "cli")]
mod batch;
#[cfg(feature = "cli")]
mod cli;
mod core;
pub mod domains;
mod i18n;
mod output;
#[cfg(feature = "cli")]
mod repl;
#[cfg(any(feature = "http", feature = "mcp"))]
mod server;

pub use core::evaluate;
pub use core::{
    parse, AstCanonicalizer, AstNode, BinaryOp, CacheKeyGen, CacheManager, CalcError,
    CalculationDomain, CanonicalForm, DomainRouter, ErrorKind, EvalContext, EvalResult, Span,
    UnaryOp,
};
pub use domains::{
    ArithmeticDomain, CombinatoricsDomain, ComplexDomain, MatrixDomain, NumberTheoryDomain,
    PolynomialDomain, PrecisionDomain, ScientificDomain, StatisticsDomain, SymbolicDomain,
    VectorDomain,
};
pub use i18n::{I18n, Lang};

#[cfg(feature = "cli")]
pub use cli::run;
#[cfg(any(feature = "cli", feature = "http", feature = "mcp"))]
pub use domains::format_bigrational;
#[cfg(any(feature = "http", feature = "mcp"))]
pub use server::*;
