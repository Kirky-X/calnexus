//! CalNexus 计算引擎：表达式解析、AST 规范化、L1 缓存、域路由。

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
