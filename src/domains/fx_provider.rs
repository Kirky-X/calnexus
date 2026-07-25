// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! 汇率数据提供者：RateProvider trait + FrankfurterProvider 实现。
//!
//! 设计依据：design.md D6（ureq + dirs 本地缓存 + stale 策略）
//! Feature 门控：`fx = ["dep:ureq", "dep:dirs"]`

use std::collections::HashMap;

use crate::core::CalcError;

/// 汇率表：base 币种 + 抓取日期 + 各币种汇率。
pub struct RateTable {
    /// 基准币种（如 "EUR"）。
    pub base: String,
    /// 数据日期（RFC 3339 格式）。
    pub date: String,
    /// 各币种对 base 的汇率。
    pub rates: HashMap<String, f64>,
}

/// 汇率数据源 trait（可注入 mock 测试）。
pub(crate) trait RateProvider: Send + Sync {
    /// 获取最新汇率表。
    fn rates(&self) -> Result<RateTable, CalcError>;
}

// stub — 待 Phase 4 实现 FrankfurterProvider + 三级缓存
