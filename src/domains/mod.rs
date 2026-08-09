// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! CalNexus 计算域集合。
//!
//! 规则 25 合规：本 mod.rs 仅包含模块声明与 re-export，零实现函数。
//! 工厂函数（build_default_router / build_precision_domain）位于 `factory.rs`。

mod arithmetic;
mod combinatorics;
mod complex;
mod factory;
#[cfg(feature = "fx")]
mod fx;
#[cfg(feature = "fx")]
mod fx_provider;
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

pub use arithmetic::ArithmeticDomain;
pub use combinatorics::CombinatoricsDomain;
pub use complex::ComplexDomain;
#[cfg(feature = "fx")]
pub use fx::FxDomain;
pub use matrix::MatrixDomain;
pub use number_theory::NumberTheoryDomain;
pub use polynomial::PolynomialDomain;
#[cfg(any(feature = "cli", feature = "http", feature = "mcp"))]
pub use precision::format_bigrational;
pub use precision::PrecisionDomain;
pub use scientific::ScientificDomain;
pub use statistics::StatisticsDomain;
pub use symbolic::SymbolicDomain;
#[cfg(feature = "time")]
pub use time::TimeDomain;
#[cfg(feature = "unit")]
pub use unit::UnitDomain;
pub use vector::VectorDomain;
