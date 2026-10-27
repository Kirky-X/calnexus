// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! 测试共享 helper：calnexus_cli() 与 default_router()；部分文件只用其一，允许 dead_code。
#![allow(dead_code)]

use assert_cmd::Command;
use calnexus::{
    ArithmeticDomain, CombinatoricsDomain, ComplexDomain, DomainRouter, MatrixDomain,
    NumberTheoryDomain, PolynomialDomain, PrecisionDomain, ScientificDomain, StatisticsDomain,
    SymbolicDomain, VectorDomain,
};

/// 构建 calnexus CLI Command（cargo-bin）。
///
/// 钉定 `CALNEXUS_LANG=en`：使用方断言文本按英文编写，与真机系统区域解耦
/// （zh 机器上 sys-locale 回退会输出中文导致断言失败；CI 与本地同一口径）。
pub fn calnexus_cli() -> Command {
    let mut cmd = Command::cargo_bin("calnexus").expect("calnexus binary not found");
    cmd.env("CALNEXUS_LANG", "en");
    cmd
}

/// 构建默认路由器：注册全部 11 个域（按优先级降序注册）。
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
