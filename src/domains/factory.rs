// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! 计算域工厂：构建默认路由器与 precision 域实例。
//!
//! 注册职责留在 domains 层，避免 src/core 反向依赖具体域类型（DIP）。

use std::sync::OnceLock;

use crate::core::{CalculationDomain, DomainRouter};

#[cfg(feature = "fx")]
use super::FxDomain;
#[cfg(feature = "time")]
use super::TimeDomain;
#[cfg(feature = "unit")]
use super::UnitDomain;
use super::{
    ArithmeticDomain, CombinatoricsDomain, ComplexDomain, MatrixDomain, NumberTheoryDomain,
    PolynomialDomain, PrecisionDomain, ScientificDomain, StatisticsDomain, SymbolicDomain,
    VectorDomain,
};

/// 构建默认路由器：注册全部 14 个域（11 核心 + 3 可选），进程级 OnceLock 缓存。
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
        #[cfg(feature = "time")]
        router.register(Box::new(TimeDomain));
        #[cfg(feature = "unit")]
        router.register(Box::new(UnitDomain));
        #[cfg(feature = "fx")]
        router.register(Box::new(FxDomain::default()));
        router
    })
}

/// 构建 PrecisionDomain 实例（precision 模式绕过路由器直接求值）。
pub(crate) fn build_precision_domain() -> Box<dyn CalculationDomain> {
    Box::new(PrecisionDomain)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== 域优先级测试（priority 属 domains 层，按分层约定在此测试） =====

    #[test]
    fn test_priority_number_theory_equals_combinatorics() {
        let nt = NumberTheoryDomain;
        let cb = CombinatoricsDomain;
        assert_eq!(nt.priority(), 25);
        assert_eq!(cb.priority(), 25);
    }

    #[test]
    fn test_priority_vector_higher_than_polynomial() {
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
        #[cfg(feature = "time")]
        assert_eq!(TimeDomain.priority(), 30);
        #[cfg(feature = "unit")]
        assert_eq!(UnitDomain.priority(), 30);
        #[cfg(feature = "fx")]
        assert_eq!(FxDomain::default().priority(), 30);
    }
}
