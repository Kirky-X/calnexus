// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! CalNexus 计算域集合：仅含模块声明与 re-export，无实现函数。

mod arithmetic;
mod combinatorics;
mod common;
mod complex;
mod factory;
#[cfg(feature = "fx")]
mod fx;
#[cfg(feature = "fx")]
pub mod fx_provider;
mod matrix;
mod number_theory;
#[cfg(feature = "numerical")]
mod numerical;
mod polynomial;
mod precision;
mod scientific;
mod statistics;
mod symbolic;
#[cfg(feature = "time")]
mod time;
#[cfg(feature = "unit")]
mod unit;

mod vector;

pub(crate) use factory::{build_default_router, build_precision_domain};

// 函数白名单常量供 function_catalog 派生目录（单一事实源，防目录漂移）
pub(crate) use arithmetic::ARITHMETIC_FUNCTIONS;
pub(crate) use combinatorics::COMBINATORICS_FUNCTIONS;
pub(crate) use complex::COMPLEX_FUNCTIONS;
#[cfg(feature = "fx")]
pub(crate) use fx::FX_FUNCTIONS;
pub(crate) use matrix::MATRIX_FUNCTIONS;
#[cfg(feature = "numerical")]
pub(crate) use matrix::NUMERICAL_FUNCTIONS;
pub(crate) use number_theory::NUMBER_THEORY_FUNCTIONS;
pub(crate) use polynomial::POLYNOMIAL_FUNCTIONS;
pub(crate) use scientific::SCIENTIFIC_FUNCTIONS;
pub(crate) use statistics::STATISTICS_FUNCTIONS;
pub(crate) use symbolic::SYMBOLIC_FUNCTIONS;

// 数值微分实现（域层）：经 api 层 ScalarMathImpl::derivative 公开暴露
#[cfg(feature = "numerical")]
pub(crate) use numerical::derivative;
#[cfg(feature = "time")]
pub(crate) use time::TIME_FUNCTIONS;
#[cfg(feature = "unit")]
pub(crate) use unit::UNIT_FUNCTIONS;
pub(crate) use vector::VECTOR_FUNCTIONS;

pub use arithmetic::ArithmeticDomain;
pub use combinatorics::CombinatoricsDomain;
pub use complex::ComplexDomain;
#[cfg(feature = "fx")]
pub use fx::FxDomain;
#[cfg(feature = "fx")]
pub use fx_provider::RateProvider;
pub use matrix::MatrixDomain;
pub use number_theory::NumberTheoryDomain;
pub use polynomial::PolynomialDomain;
pub use precision::PrecisionDomain;
#[cfg(any(feature = "cli", feature = "http", feature = "mcp"))]
pub use precision::format_bigrational;
pub use scientific::ScientificDomain;
pub use statistics::StatisticsDomain;
pub use symbolic::SymbolicDomain;
#[cfg(feature = "time")]
pub use time::TimeDomain;
#[cfg(feature = "unit")]
pub use unit::UnitDomain;
pub use vector::VectorDomain;
