// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! 测试共享 helper：消除 4 个测试文件中 calnexus_cli() / default_router() 的复制粘贴。
// 不同测试文件只用部分 helper，允许 dead_code。
#![allow(dead_code)]

use assert_cmd::Command;
use calnexus::{
    ArithmeticDomain, CombinatoricsDomain, ComplexDomain, DomainRouter, MatrixDomain,
    NumberTheoryDomain, PolynomialDomain, PrecisionDomain, ScientificDomain, StatisticsDomain,
    SymbolicDomain, VectorDomain,
};

/// 构建 calnexus CLI Command（cargo-bin）。
pub fn calnexus_cli() -> Command {
    Command::cargo_bin("calnexus").expect("calnexus binary not found")
}

/// 构建默认路由器：注册全部 11 个域。
/// 优先级降序：Complex/Matrix/Vector (30+) > Symbolic (30) > Precision/NumberTheory/Combinatorics/Polynomial (25) > Statistics/Scientific (20) > Arithmetic (10)。
pub fn default_router() -> DomainRouter {
    let mut router = DomainRouter::new();
    router.register(Box::new(PrecisionDomain));
    router.register(Box::new(ComplexDomain));
    router.register(Box::new(MatrixDomain));
    router.register(Box::new(VectorDomain));
    router.register(Box::new(SymbolicDomain));
    router.register(Box::new(NumberTheoryDomain));
    router.register(Box::new(CombinatoricsDomain));
    router.register(Box::new(PolynomialDomain));
    router.register(Box::new(ScientificDomain));
    router.register(Box::new(StatisticsDomain));
    router.register(Box::new(ArithmeticDomain));
    router
}
