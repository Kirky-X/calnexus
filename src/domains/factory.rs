// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! 计算域工厂函数：构建默认路由器与 precision 域实例。
//!
//! 设计依据：规则 25（mod.rs 只放 trait/struct/re-export，实现拆到独立文件）。
//! 将工厂函数从 mod.rs 移到本文件，避免 mod.rs 包含实现逻辑。
//!
//! 注册职责归 domains 层（知道自己有哪些实现），避免 src/core 反向依赖
//! src/domains 的具体域类型（DIP 依赖倒置原则）。

use std::sync::OnceLock;

use crate::core::{CalculationDomain, DomainRouter};

use super::{
    ArithmeticDomain, CombinatoricsDomain, ComplexDomain, MatrixDomain, NumberTheoryDomain,
    PolynomialDomain, PrecisionDomain, ScientificDomain, StatisticsDomain, SymbolicDomain,
    VectorDomain,
};

/// 构建默认路由器：注册全部 11 个计算域（含 SymbolicDomain）。
///
/// 供 `crate::core::evaluate` 与 REPL 共用。进程级缓存（OnceLock），只构建一次，
/// 避免每次请求重复分配 11 个 Box。
pub(crate) fn build_default_router() -> &'static DomainRouter {
    static DEFAULT_ROUTER: OnceLock<DomainRouter> = OnceLock::new();
    DEFAULT_ROUTER.get_or_init(|| {
        let mut router = DomainRouter::new();
        router.register(Box::new(PrecisionDomain));
        router.register(Box::new(ComplexDomain));
        router.register(Box::new(MatrixDomain));
        router.register(Box::new(VectorDomain));
        router.register(Box::new(SymbolicDomain));
        router.register(Box::new(PolynomialDomain));
        router.register(Box::new(NumberTheoryDomain));
        router.register(Box::new(CombinatoricsDomain));
        router.register(Box::new(ScientificDomain));
        router.register(Box::new(StatisticsDomain));
        router.register(Box::new(ArithmeticDomain));
        router
    })
}

/// 构建 PrecisionDomain 实例（供 precision 模式直接求值，绕过路由器）。
///
/// 通过 trait object 工厂函数，使 src/core/evaluator.rs 无需导入具体域类型，
/// 彻底消除 src/core → src/domains 的类型依赖。
pub(crate) fn build_precision_domain() -> Box<dyn CalculationDomain> {
    Box::new(PrecisionDomain)
}
