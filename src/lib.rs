//! CalNexus 计算引擎：表达式解析、AST 规范化、L1 缓存、域路由。

pub mod core;
pub mod domains;
#[cfg(feature = "cli")]
pub mod cli;

pub use core::cache::{CacheKeyGen, CacheManager};
pub use core::canonicalizer::AstCanonicalizer;
pub use core::domain::{CalculationDomain, DomainRouter};
pub use core::parser::parse;
pub use core::types::{
    AstNode, BinaryOp, CalcError, CanonicalForm, EvalContext, EvalResult, UnaryOp,
};
pub use domains::arithmetic::ArithmeticDomain;
pub use domains::scientific::ScientificDomain;
