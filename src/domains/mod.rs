// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! CalNexus 计算域集合。
mod arithmetic;
mod combinatorics;
mod complex;
mod matrix;
mod number_theory;
mod polynomial;
mod precision;
mod scientific;
mod statistics;
mod vector;
pub use arithmetic::ArithmeticDomain;
pub use combinatorics::CombinatoricsDomain;
pub use complex::ComplexDomain;
pub use matrix::MatrixDomain;
pub use number_theory::NumberTheoryDomain;
pub use polynomial::PolynomialDomain;
#[cfg(any(feature = "cli", feature = "http", feature = "mcp"))]
pub use precision::format_bigrational;
pub use precision::PrecisionDomain;
pub use scientific::ScientificDomain;
pub use statistics::StatisticsDomain;
pub use vector::VectorDomain;
