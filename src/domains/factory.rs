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

#[cfg(test)]
mod tests {
    use super::*;

    // ===== 域优先级测试（从 core/domain.rs 迁移，消除 core → domains 类型依赖）=====
    //
    // priority 是 domains 层的属性，测试应在 domains 层进行（规则 25 + DIP）。
    // 原测试位于 core/domain.rs，直接构造具体域类型，违反 ARCHITECTURE.md §2.3
    // "core → domains 类型依赖 = 0" 声明。

    #[test]
    fn test_priority_number_theory_equals_combinatorics() {
        // NumberTheory(25) 与 Combinatorics(25) 同级
        let nt = NumberTheoryDomain;
        let cb = CombinatoricsDomain;
        assert_eq!(nt.priority(), 25);
        assert_eq!(cb.priority(), 25);
    }

    #[test]
    fn test_priority_vector_higher_than_polynomial() {
        // Vector(30) > Polynomial(25)
        let vec = VectorDomain;
        let pol = PolynomialDomain;
        assert!(vec.priority() > pol.priority());
        assert_eq!(vec.priority(), 30);
        assert_eq!(pol.priority(), 25);
    }

    #[test]
    fn test_full_priority_table() {
        // 完整 priority 表回归测试（防止误改 priority 值导致路由顺序错误）
        assert_eq!(ArithmeticDomain.priority(), 10);
        assert_eq!(ScientificDomain.priority(), 20);
        assert_eq!(StatisticsDomain.priority(), 20);
        assert_eq!(NumberTheoryDomain.priority(), 25);
        assert_eq!(CombinatoricsDomain.priority(), 25);
        assert_eq!(PolynomialDomain.priority(), 25);
        assert_eq!(PrecisionDomain.priority(), 25);
        assert_eq!(ComplexDomain.priority(), 30);
        assert_eq!(MatrixDomain.priority(), 30);
        assert_eq!(VectorDomain.priority(), 30);
        assert_eq!(SymbolicDomain.priority(), 30);
    }
}
