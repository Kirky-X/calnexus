// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! Server 共享缓存：HTTP/MCP 协议共享的进程级 CacheManager。
//!
//! 使用全局 OnceLock 懒初始化，确保相同表达式的第二次请求能命中缓存
//! （spec.md R-sdforge-002/R-sdforge-003 缓存语义）。
//!
//! 架构审查 MEDIUM-1 修复：原 http.rs 和 mcp.rs 各自维护独立的 SHARED_CACHE，
//! 导致 HTTP 和 MCP 请求不共享缓存。现统一到本模块，两协议共享同一缓存实例。

use crate::CacheManager;
use std::sync::OnceLock;

/// 进程级共享缓存（OnceLock 懒初始化，跨请求/协议共享）。
static SHARED_CACHE: OnceLock<CacheManager> = OnceLock::new();

/// 获取共享 CacheManager 实例。
pub(crate) fn shared_cache() -> &'static CacheManager {
    SHARED_CACHE.get_or_init(CacheManager::new)
}
