// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! trait-kit `AsyncKit` 集成层（feature `kit` 门控）。
//!
//! 把 `crate::api` 的 5 个分组 trait 门面封装为 trait-kit 模块：
//! [`CalNexusModule`] 构建共享门面 `Arc<CalNexus>`，5 个域模块
//! （[`ScalarMathModule`] 等）依赖它并各自暴露能力句柄。核心 trait 与
//! `*Impl<'_>` 结构不改动——句柄仅持有共享门面并借出既有视图，
//! 模块构建全部手写 `ModuleMeta` + `AsyncAutoBuilder`（无宏）。
//!
//! # 用法
//!
//! ```no_run
//! use calnexus::integrations::kit::{CalNexusModule, ScalarMathModule};
//! use calnexus::ScalarMath;
//! use trait_kit::prelude::*;
//!
//! # async fn demo() -> Result<(), Box<dyn std::error::Error>> {
//! let mut kit = AsyncKit::new();
//! kit.register::<CalNexusModule>()?;
//! kit.register::<ScalarMathModule>()?;
//! let kit = kit.build().await?;
//! let scalar = kit.require::<ScalarMathModule>()?;
//! let result = scalar.handle().add(1.0, 2.0)?;
//! # Ok(())
//! # }
//! ```

use std::any::TypeId;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, OnceLock};

use trait_kit::prelude::*;

use crate::api::{
    AppliedMathImpl, CalNexus, DataAnalysisImpl, LinearAlgebraImpl, ScalarMathImpl,
    SymbolicMathImpl,
};
use crate::core::{CalcError, ErrorKind};

/// [`CalNexusModule`] 构建配置：预置变量表（可选，缺省为空）。
#[derive(Debug, Clone, Default)]
pub struct CalNexusKitConfig {
    /// 预置变量（名称 → 值），构建时依次 `set_var` 写入门面上下文。
    pub variables: Vec<(String, f64)>,
}

/// 计算引擎门面模块（leaf）：构建共享 `Arc<CalNexus>` 并预置变量。
pub struct CalNexusModule;

impl ModuleMeta for CalNexusModule {
    const NAME: &'static str = "calnexus";
}

impl AsyncAutoBuilder for CalNexusModule {
    type Capability = Arc<CalNexus>;
    type Error = CalcError;

    fn build<'a>(
        kit: &'a AsyncKit,
    ) -> Pin<Box<dyn Future<Output = Result<Self::Capability, Self::Error>> + Send + 'a>> {
        Box::pin(async move {
            // 配置可选：未 set_config 时按空变量表构建。
            let config: CalNexusKitConfig = kit.config().unwrap_or_default();
            let cn = CalNexus::new();
            for (name, value) in &config.variables {
                cn.set_var(name, *value);
            }
            Ok(Arc::new(cn))
        })
    }
}

// 范式说明：五个域模块的 ModuleMeta/AsyncAutoBuilder/句柄逐一手写
// （trait-kit 集成"0 宏、全手写"范式，参照 oxcache/src/integrations/kit/
// module.rs 与 dbnexus/src/integrations/kit/module.rs）。

/// 标量数学能力句柄：持有共享门面，借出 [`ScalarMathImpl`] 视图。
#[derive(Clone)]
pub struct ScalarMathHandle {
    cn: Arc<CalNexus>,
}

impl ScalarMathHandle {
    /// 借出标量数学视图（`ScalarMath` trait 已为其实现）。
    pub fn handle(&self) -> ScalarMathImpl<'_> {
        self.cn.scalar()
    }

    /// 访问共享门面（变量上下文等全量操作）。
    pub fn facade(&self) -> &CalNexus {
        &self.cn
    }
}

/// 标量数学模块：算术/科学函数/精度/数论/组合（`ScalarMath` 域）。
pub struct ScalarMathModule;

impl ModuleMeta for ScalarMathModule {
    const NAME: &'static str = "calnexus-scalar";

    fn dependencies() -> &'static [(&'static str, TypeId)] {
        static DEPS: OnceLock<Vec<(&'static str, TypeId)>> = OnceLock::new();
        DEPS.get_or_init(|| vec![(CalNexusModule::NAME, TypeId::of::<CalNexusModule>())])
            .as_slice()
    }
}

impl AsyncAutoBuilder for ScalarMathModule {
    type Capability = ScalarMathHandle;
    type Error = CalcError;

    fn build<'a>(
        kit: &'a AsyncKit,
    ) -> Pin<Box<dyn Future<Output = Result<Self::Capability, Self::Error>> + Send + 'a>> {
        Box::pin(async move {
            let cn = kit
                .require::<CalNexusModule>()
                .map_err(|e| CalcError::new(ErrorKind::Eval, format!("require calnexus: {e}")))?;
            Ok(ScalarMathHandle { cn })
        })
    }
}

/// 线性代数能力句柄：持有共享门面，借出 [`LinearAlgebraImpl`] 视图。
#[derive(Clone)]
pub struct LinearAlgebraHandle {
    cn: Arc<CalNexus>,
}

impl LinearAlgebraHandle {
    /// 借出线性代数视图（`LinearAlgebra` trait 已为其实现）。
    pub fn handle(&self) -> LinearAlgebraImpl<'_> {
        self.cn.linalg()
    }

    /// 访问共享门面（变量上下文等全量操作）。
    pub fn facade(&self) -> &CalNexus {
        &self.cn
    }
}

/// 线性代数模块：矩阵/向量/数值分解（`LinearAlgebra` 域）。
pub struct LinearAlgebraModule;

impl ModuleMeta for LinearAlgebraModule {
    const NAME: &'static str = "calnexus-linalg";

    fn dependencies() -> &'static [(&'static str, TypeId)] {
        static DEPS: OnceLock<Vec<(&'static str, TypeId)>> = OnceLock::new();
        DEPS.get_or_init(|| vec![(CalNexusModule::NAME, TypeId::of::<CalNexusModule>())])
            .as_slice()
    }
}

impl AsyncAutoBuilder for LinearAlgebraModule {
    type Capability = LinearAlgebraHandle;
    type Error = CalcError;

    fn build<'a>(
        kit: &'a AsyncKit,
    ) -> Pin<Box<dyn Future<Output = Result<Self::Capability, Self::Error>> + Send + 'a>> {
        Box::pin(async move {
            let cn = kit
                .require::<CalNexusModule>()
                .map_err(|e| CalcError::new(ErrorKind::Eval, format!("require calnexus: {e}")))?;
            Ok(LinearAlgebraHandle { cn })
        })
    }
}

/// 数据分析能力句柄：持有共享门面，借出 [`DataAnalysisImpl`] 视图。
#[derive(Clone)]
pub struct DataAnalysisHandle {
    cn: Arc<CalNexus>,
}

impl DataAnalysisHandle {
    /// 借出数据分析视图（`DataAnalysis` trait 已为其实现）。
    pub fn handle(&self) -> DataAnalysisImpl<'_> {
        self.cn.stats()
    }

    /// 访问共享门面（变量上下文等全量操作）。
    pub fn facade(&self) -> &CalNexus {
        &self.cn
    }
}

/// 数据分析模块：统计/分布/检验/回归（`DataAnalysis` 域）。
pub struct DataAnalysisModule;

impl ModuleMeta for DataAnalysisModule {
    const NAME: &'static str = "calnexus-stats";

    fn dependencies() -> &'static [(&'static str, TypeId)] {
        static DEPS: OnceLock<Vec<(&'static str, TypeId)>> = OnceLock::new();
        DEPS.get_or_init(|| vec![(CalNexusModule::NAME, TypeId::of::<CalNexusModule>())])
            .as_slice()
    }
}

impl AsyncAutoBuilder for DataAnalysisModule {
    type Capability = DataAnalysisHandle;
    type Error = CalcError;

    fn build<'a>(
        kit: &'a AsyncKit,
    ) -> Pin<Box<dyn Future<Output = Result<Self::Capability, Self::Error>> + Send + 'a>> {
        Box::pin(async move {
            let cn = kit
                .require::<CalNexusModule>()
                .map_err(|e| CalcError::new(ErrorKind::Eval, format!("require calnexus: {e}")))?;
            Ok(DataAnalysisHandle { cn })
        })
    }
}

/// 符号数学能力句柄：持有共享门面，借出 [`SymbolicMathImpl`] 视图。
#[derive(Clone)]
pub struct SymbolicMathHandle {
    cn: Arc<CalNexus>,
}

impl SymbolicMathHandle {
    /// 借出符号数学视图（`SymbolicMath` trait 已为其实现）。
    pub fn handle(&self) -> SymbolicMathImpl<'_> {
        self.cn.symbolic()
    }

    /// 访问共享门面（变量上下文等全量操作）。
    pub fn facade(&self) -> &CalNexus {
        &self.cn
    }
}

/// 符号数学模块：符号演算/多项式/复数/方程求解（`SymbolicMath` 域）。
pub struct SymbolicMathModule;

impl ModuleMeta for SymbolicMathModule {
    const NAME: &'static str = "calnexus-symbolic";

    fn dependencies() -> &'static [(&'static str, TypeId)] {
        static DEPS: OnceLock<Vec<(&'static str, TypeId)>> = OnceLock::new();
        DEPS.get_or_init(|| vec![(CalNexusModule::NAME, TypeId::of::<CalNexusModule>())])
            .as_slice()
    }
}

impl AsyncAutoBuilder for SymbolicMathModule {
    type Capability = SymbolicMathHandle;
    type Error = CalcError;

    fn build<'a>(
        kit: &'a AsyncKit,
    ) -> Pin<Box<dyn Future<Output = Result<Self::Capability, Self::Error>> + Send + 'a>> {
        Box::pin(async move {
            let cn = kit
                .require::<CalNexusModule>()
                .map_err(|e| CalcError::new(ErrorKind::Eval, format!("require calnexus: {e}")))?;
            Ok(SymbolicMathHandle { cn })
        })
    }
}

/// 应用数学能力句柄：持有共享门面，借出 [`AppliedMathImpl`] 视图。
#[derive(Clone)]
pub struct AppliedMathHandle {
    cn: Arc<CalNexus>,
}

impl AppliedMathHandle {
    /// 借出应用数学视图（`AppliedMath` trait 已为其实现；可用方法随
    /// `time`/`unit`/`fx` feature 门控）。
    pub fn handle(&self) -> AppliedMathImpl<'_> {
        self.cn.applied()
    }

    /// 访问共享门面（变量上下文等全量操作）。
    pub fn facade(&self) -> &CalNexus {
        &self.cn
    }
}

/// 应用数学模块：时间/单位/汇率（`AppliedMath` 域，方法随 feature 门控）。
pub struct AppliedMathModule;

impl ModuleMeta for AppliedMathModule {
    const NAME: &'static str = "calnexus-applied";

    fn dependencies() -> &'static [(&'static str, TypeId)] {
        static DEPS: OnceLock<Vec<(&'static str, TypeId)>> = OnceLock::new();
        DEPS.get_or_init(|| vec![(CalNexusModule::NAME, TypeId::of::<CalNexusModule>())])
            .as_slice()
    }
}

impl AsyncAutoBuilder for AppliedMathModule {
    type Capability = AppliedMathHandle;
    type Error = CalcError;

    fn build<'a>(
        kit: &'a AsyncKit,
    ) -> Pin<Box<dyn Future<Output = Result<Self::Capability, Self::Error>> + Send + 'a>> {
        Box::pin(async move {
            let cn = kit
                .require::<CalNexusModule>()
                .map_err(|e| CalcError::new(ErrorKind::Eval, format!("require calnexus: {e}")))?;
            Ok(AppliedMathHandle { cn })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 模块名唯一性：五个域模块 + 根模块的 NAME 互不相同。
    #[test]
    fn module_names_are_unique() {
        let names = [
            CalNexusModule::NAME,
            ScalarMathModule::NAME,
            LinearAlgebraModule::NAME,
            DataAnalysisModule::NAME,
            SymbolicMathModule::NAME,
            AppliedMathModule::NAME,
        ];
        for name in &names {
            assert_eq!(
                names.iter().filter(|n| *n == name).count(),
                1,
                "module name {name} is not unique"
            );
        }
    }

    /// 依赖声明：五个域模块都且仅依赖根模块。
    #[test]
    fn domain_modules_depend_on_calnexus_module() {
        let expected = (CalNexusModule::NAME, TypeId::of::<CalNexusModule>());
        for deps in [
            ScalarMathModule::dependencies(),
            LinearAlgebraModule::dependencies(),
            DataAnalysisModule::dependencies(),
            SymbolicMathModule::dependencies(),
            AppliedMathModule::dependencies(),
        ] {
            assert_eq!(deps, &[expected], "domain module deps must be [calnexus]");
        }
        assert!(
            CalNexusModule::dependencies().is_empty(),
            "root module must be a leaf"
        );
    }

    /// 预置变量配置默认为空表。
    #[test]
    fn kit_config_defaults_to_empty_variables() {
        assert!(CalNexusKitConfig::default().variables.is_empty());
    }
}
