// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! CalNexus 计算引擎：表达式解析、AST 规范化、L1 缓存、域路由。

// 测试代码中使用 3.14 / 6.283... 等值作为测试输入，并非数学常量的误用。
// 测试函数名保留 P/C 大写以对应排列 (Permutation) / 组合 (Combination) 数学记号。
#![cfg_attr(test, allow(clippy::approx_constant, non_snake_case))]

#[cfg(feature = "cli")]
pub mod batch;
#[cfg(feature = "cli")]
pub mod cli;
pub mod core;
pub mod domains;
pub mod output;
#[cfg(feature = "cli")]
pub mod repl;
pub mod symbolic;

pub use core::cache::{CacheKeyGen, CacheManager};
pub use core::canonicalizer::AstCanonicalizer;
pub use core::domain::{CalculationDomain, DomainRouter};
pub use core::parser::parse;
pub use core::types::{
    AstNode, BinaryOp, CalcError, CanonicalForm, EvalContext, EvalResult, UnaryOp,
};
pub use domains::arithmetic::ArithmeticDomain;
pub use domains::combinatorics::CombinatoricsDomain;
pub use domains::complex::ComplexDomain;
pub use domains::matrix::MatrixDomain;
pub use domains::number_theory::NumberTheoryDomain;
pub use domains::polynomial::PolynomialDomain;
pub use domains::precision::PrecisionDomain;
pub use domains::scientific::ScientificDomain;
pub use domains::statistics::StatisticsDomain;
pub use domains::vector::VectorDomain;
pub use symbolic::SymbolicDomain;
