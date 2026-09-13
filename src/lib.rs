// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! CalNexus 计算引擎：表达式解析、AST 规范化、L1 缓存、域路由。
//!
// v015：CalcError 携带 source_detail 后为 144 字节，超过 clippy::result_large_err
// 默认阈值（128）。错误类型按值传递（Clone 语义被缓存/测试广泛依赖）是既定 API
// 设计，Box 化属于破坏性变更——crate 级豁免并在此记录论证（规则：豁免必须显性化）。
#![allow(clippy::result_large_err)]
// 测试代码中使用 3.14 / 6.283... 等值作为测试输入，并非数学常量的误用。
// 测试函数名保留 P/C 大写以对应排列 (Permutation) / 组合 (Combination) 数学记号。
#![cfg_attr(test, allow(clippy::approx_constant, non_snake_case))]
// 无 CLI feature 时，output/symbolic 中仅 CLI 调用的函数不构成 dead code
#![cfg_attr(not(feature = "cli"), allow(dead_code))]

mod api;
#[cfg(feature = "cli")]
mod batch;
#[cfg(feature = "cli")]
mod cli;
mod core;
/// 数学实现层。
///
/// # 内部 API
///
/// 本模块为**内部 API，非 Semver 承诺**（v015 R-api-003）：嵌入式调用方应优先使用
/// crate 根的门面（`api::CalNexus`）与五个分组 trait（ScalarMath/LinearAlgebra/…）。
/// 本模块的函数签名可能在 minor 版本间调整而不另行公告。
pub mod domains;
mod function_catalog;
mod i18n;
/// 数学纯函数层。
///
/// # 内部 API
///
/// 本模块为**内部 API，非 Semver 承诺**（v015 R-api-003）：领域逻辑经
/// `crate::domains` 委托暴露；直接依赖本模块的下游在升级时可能需要适配。
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
/// Server 接口层具名导出（v015 T041，R-api-002：取消通配导出，冻结确定面）。
#[cfg(any(feature = "http", feature = "mcp"))]
pub use server::{
    EvaluateRequest, EvaluateResponse, ListFunctionsRequest, ListFunctionsResponse, ServerError,
    calc_error_to_api_error,
};
#[cfg(feature = "http")]
pub use server::{HttpServer, build_router};
#[cfg(feature = "mcp")]
pub use server::{McpServer, build_mcp_server};
