// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! 直接 API 层（L4b）。
//!
//! 提供 `CalNexus` 门面结构体 + 5 个分组 trait，绕过表达式解析直接调用 math 层函数。
//!
//! 依赖方向：`api/` → `math/` → `core/`，不依赖 `domains/`。

pub mod cache;
pub mod traits;
pub mod types;

mod applied;
mod linalg;
mod scalar;
mod stats;
mod symbolic_api;

pub use self::scalar::ScalarMathImpl;
pub use self::linalg::LinearAlgebraImpl;
pub use self::stats::DataAnalysisImpl;
pub use self::symbolic_api::SymbolicMathImpl;
pub use self::applied::AppliedMathImpl;

use crate::core::{CalcError, EvalContext};
use std::sync::RwLock;

/// CalNexus 门面结构体：直接 API 入口。
///
/// 持有缓存管理器 + 变量上下文，提供 5 个分组 trait 访问器。
pub struct CalNexus {
    ctx: RwLock<EvalContext>,
}

impl CalNexus {
    /// 创建默认实例（空上下文）。
    pub fn new() -> Self {
        Self {
            ctx: RwLock::new(EvalContext::new()),
        }
    }

    /// 设置变量。
    pub fn set_var(&self, name: &str, value: f64) {
        let mut ctx = self.ctx.write().unwrap();
        *ctx = ctx.clone().with_var(name, value);
    }

    /// 获取变量。
    pub fn get_var(&self, name: &str) -> Option<f64> {
        let ctx = self.ctx.read().unwrap();
        ctx.get_var(name)
    }

    /// 清空所有变量。
    pub fn clear_vars(&self) {
        let mut ctx = self.ctx.write().unwrap();
        *ctx = EvalContext::new();
    }

    /// 获取标量数学 API。
    pub fn scalar(&self) -> ScalarMathImpl<'_> {
        ScalarMathImpl { cn: self }
    }

    /// 获取线性代数 API。
    pub fn linalg(&self) -> LinearAlgebraImpl<'_> {
        LinearAlgebraImpl { cn: self }
    }

    /// 获取数据分析 API。
    pub fn stats(&self) -> DataAnalysisImpl<'_> {
        DataAnalysisImpl { cn: self }
    }

    /// 获取符号数学 API。
    pub fn symbolic(&self) -> SymbolicMathImpl<'_> {
        SymbolicMathImpl { cn: self }
    }

    /// 获取应用数学 API（时间/单位/汇率）。
    pub fn applied(&self) -> AppliedMathImpl<'_> {
        AppliedMathImpl { cn: self }
    }

    /// 获取内部上下文（供 trait 实现使用）。
    pub(crate) fn context(&self) -> EvalContext {
        self.ctx.read().unwrap().clone()
    }
}

impl Default for CalNexus {
    fn default() -> Self {
        Self::new()
    }
}
