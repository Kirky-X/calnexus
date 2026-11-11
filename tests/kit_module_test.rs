// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! trait-kit 集成模块外部驱动测试：build 快乐路径、`kit.require::<M>()`
//! 全模块、错误分支（未注册 require、trait 错误传播）。

#![cfg(feature = "kit")]

use std::sync::Arc;

use calnexus::integrations::kit::{
    AppliedMathModule, CalNexusKitConfig, CalNexusModule, DataAnalysisModule, LinearAlgebraModule,
    ScalarMathModule, SymbolicMathModule,
};
// 域 trait 方法经 `*Impl` 的 inherent 转发暴露，无需 trait 进入作用域。
use calnexus::{CalcError, ErrorKind, EvalResult};
use trait_kit::prelude::*;

/// 提取 [`EvalResult`] 的数值输出（Scalar 变体断言辅助）。
fn number_of(result: EvalResult) -> f64 {
    match result {
        EvalResult::Scalar(n) => n,
        other => panic!("expected Scalar, got {other:?}"),
    }
}

/// 注册全部 6 个模块并构建。
async fn full_kit() -> AsyncKit<AsyncReady> {
    let mut kit = AsyncKit::new();
    kit.register::<CalNexusModule>().expect("register calnexus");
    kit.register::<ScalarMathModule>().expect("register scalar");
    kit.register::<LinearAlgebraModule>()
        .expect("register linalg");
    kit.register::<DataAnalysisModule>()
        .expect("register stats");
    kit.register::<SymbolicMathModule>()
        .expect("register symbolic");
    kit.register::<AppliedMathModule>()
        .expect("register applied");
    kit.build().await.expect("build kit")
}

/// 快乐路径：全模块构建成功，require 返回可用句柄，各域 trait 方法接线正确。
#[tokio::test]
async fn build_and_require_all_modules_happy_path() {
    let kit = full_kit().await;

    // 根能力：Arc<CalNexus> 门面可直接使用。
    let facade: Arc<calnexus::CalNexus> = kit.require::<CalNexusModule>().expect("require root");

    // ScalarMath 域。
    let scalar = kit.require::<ScalarMathModule>().expect("require scalar");
    assert_eq!(number_of(scalar.handle().add(1.0, 2.0).expect("add")), 3.0);
    // 句柄与根门面共享同一实例：预置变量经句柄 facade 可见。
    facade.set_var("k", 7.0);
    assert_eq!(scalar.facade().get_var("k"), Some(7.0));

    // LinearAlgebra 域。
    let linalg = kit
        .require::<LinearAlgebraModule>()
        .expect("require linalg");
    let m = calnexus::Matrix::from_rows(&[&[2.0, 0.0], &[0.0, 2.0]]);
    let det = linalg.handle().det(&m).expect("det");
    assert_eq!(number_of(det), 4.0);

    // DataAnalysis 域。
    let stats = kit.require::<DataAnalysisModule>().expect("require stats");
    let mean = stats.handle().mean(&[1.0, 2.0, 3.0]).expect("mean");
    assert_eq!(number_of(mean), 2.0);

    // SymbolicMath 域。
    let symbolic = kit
        .require::<SymbolicMathModule>()
        .expect("require symbolic");
    let d = symbolic.handle().differentiate("x^2", "x").expect("diff");
    assert!(matches!(d, EvalResult::Symbolic(_)), "got {d:?}");

    // AppliedMath 域：视图可借出（方法集合随 time/unit/fx feature 门控，
    // 此处仅验证句柄与门面接线）。
    let applied = kit.require::<AppliedMathModule>().expect("require applied");
    assert_eq!(applied.facade().get_var("k"), Some(7.0));
}

/// 配置路径：`CalNexusKitConfig` 预置变量写入共享门面，域句柄可见。
#[tokio::test]
async fn kit_config_preset_variables_visible_through_handles() {
    let mut kit = AsyncKit::new();
    kit.set_config(CalNexusKitConfig {
        variables: vec![("answer".to_string(), 42.0)],
    });
    kit.register::<CalNexusModule>().expect("register");
    kit.register::<ScalarMathModule>().expect("register");
    let kit = kit.build().await.expect("build");

    let facade = kit.require::<CalNexusModule>().expect("require");
    assert_eq!(facade.get_var("answer"), Some(42.0));
    let scalar = kit.require::<ScalarMathModule>().expect("require");
    assert_eq!(scalar.facade().get_var("answer"), Some(42.0));
}

/// 错误分支：require 未注册模块返回显式错误（不 panic）。
#[tokio::test]
async fn require_unregistered_module_fails_explicitly() {
    let mut kit = AsyncKit::new();
    kit.register::<CalNexusModule>()
        .expect("register root only");
    let kit = kit.build().await.expect("build");
    // `ScalarMathHandle` 未实现 Debug，不能用 `expect_err`；match 提取错误分支。
    let err = match kit.require::<ScalarMathModule>() {
        Err(e) => e,
        Ok(_) => panic!("require must fail for unregistered module"),
    };
    assert!(
        err.to_string().to_lowercase().contains("scalar"),
        "error should name the missing module, got: {err}"
    );
}

/// 错误分支：域模块构建时上游缺席（根模块未注册，但依赖声明已登记）
/// 令整图构建失败，错误显式传播。
#[tokio::test]
async fn build_fails_when_dependency_not_registered() {
    let mut kit = AsyncKit::new();
    kit.register::<ScalarMathModule>().expect("register scalar");
    // 故意不注册 CalNexusModule。
    let result = kit.build().await;
    assert!(result.is_err(), "build must fail with missing dependency");
}

/// 错误分支：trait 层错误经句柄原样传播（除零 → DivisionByZero）。
#[tokio::test]
async fn calc_error_propagates_through_handle() {
    let kit = full_kit().await;
    let scalar = kit.require::<ScalarMathModule>().expect("require");
    let err: CalcError = scalar
        .handle()
        .div(1.0, 0.0)
        .expect_err("division by zero must error");
    assert_eq!(err.kind, ErrorKind::DivisionByZero);
}

/// 手写范式契约：域模块依赖声明指向根模块（防回归为 leaf）。
#[test]
fn domain_module_dependencies_declared() {
    use std::any::TypeId;
    let expected = &[(CalNexusModule::NAME, TypeId::of::<CalNexusModule>())];
    assert_eq!(ScalarMathModule::dependencies(), expected);
    assert_eq!(LinearAlgebraModule::dependencies(), expected);
    assert_eq!(DataAnalysisModule::dependencies(), expected);
    assert_eq!(SymbolicMathModule::dependencies(), expected);
    assert_eq!(AppliedMathModule::dependencies(), expected);
    assert!(CalNexusModule::dependencies().is_empty());
}
