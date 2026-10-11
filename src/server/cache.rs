// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! Server 共享缓存：HTTP/MCP 协议共享的进程级 CacheManager（OnceLock 懒初始化）。

use crate::CacheManager;
use std::sync::OnceLock;

/// 进程级共享缓存（OnceLock 懒初始化，跨请求/协议共享）。
static SHARED_CACHE: OnceLock<CacheManager> = OnceLock::new();

/// 获取共享 CacheManager 实例。
pub(crate) fn shared_cache() -> &'static CacheManager {
    SHARED_CACHE.get_or_init(CacheManager::new)
}

/// 以字节预算预初始化共享缓存（已初始化时 no-op；仅 cli+server 双 feature 下编译）。
#[cfg(all(feature = "cli", feature = "server"))]
pub(crate) fn init_shared_cache(capacity_bytes: u64) {
    let _ = SHARED_CACHE.get_or_init(|| CacheManager::with_capacity_bytes(capacity_bytes));
}
