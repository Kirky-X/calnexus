// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! CalNexus 计算引擎：表达式解析、AST 规范化、L1 缓存、域路由。
// 豁免 clippy::result_large_err：CalcError 按值传递是既定 API，Box 化为破坏性变更
#![allow(clippy::result_large_err)]
// 测试用 3.14 等值输入，且函数名 P/C 对应排列/组合记号
#![cfg_attr(test, allow(clippy::approx_constant, non_snake_case))]
// 无 cli feature 时仅 CLI 调用的函数不算 dead code
#![cfg_attr(not(feature = "cli"), allow(dead_code))]

mod api;
#[cfg(feature = "cli")]
mod batch;
#[cfg(feature = "cli")]
mod cli;
mod core;
/// 数学实现层（内部 API，非 Semver 承诺；嵌入调用请走 crate 根门面）。
pub mod domains;
mod function_catalog;
mod i18n;
/// 数学纯函数层（内部 API，非 Semver 承诺；领域逻辑经 `crate::domains` 暴露）。
pub mod math;
mod output;
#[cfg(feature = "cli")]
mod repl;
#[cfg(any(feature = "http", feature = "mcp"))]
mod server;

pub use api::traits::{AppliedMath, DataAnalysis, LinearAlgebra, ScalarMath, SymbolicMath};
pub use api::types::{BigNumber, Complex, Matrix, Polynomial, Vector};
pub use api::{
    AppliedMathImpl, CalNexus, DataAnalysisImpl, LinearAlgebraImpl, ScalarMathImpl,
    SymbolicMathImpl,
};
pub use core::{
    AstCanonicalizer, AstNode, BinaryOp, CacheKeyGen, CacheManager, CalcError, CalculationDomain,
    CanonicalForm, DomainRouter, ErrorKind, EvalContext, EvalResult, Span, UnaryOp, parse,
};
pub use core::{evaluate, evaluate_with_router};
#[cfg(feature = "fx")]
pub use domains::FxDomain;
#[cfg(feature = "time")]
pub use domains::TimeDomain;
#[cfg(feature = "unit")]
pub use domains::UnitDomain;
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
/// Server 接口层具名导出（冻结确定面）。
#[cfg(any(feature = "http", feature = "mcp"))]
pub use server::{
    EvaluateRequest, EvaluateResponse, ListFunctionsRequest, ListFunctionsResponse, ServerError,
    calc_error_to_api_error, calc_error_to_api_error_i18n,
};
#[cfg(feature = "http")]
pub use server::{HttpServer, build_router};
#[cfg(feature = "mcp")]
pub use server::{McpServer, build_mcp_server};
