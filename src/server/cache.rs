// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! Server 共享缓存：HTTP/MCP 协议共享的进程级 CacheManager。
//!
//! 使用全局 OnceLock 懒初始化，确保相同表达式的第二次请求能命中缓存
//! （缓存语义）。统一单实例的动因：此前 HTTP/MCP 各自独立缓存导致跨协议不命中。

use crate::CacheManager;
use std::sync::OnceLock;

/// 进程级共享缓存（OnceLock 懒初始化，跨请求/协议共享）。
static SHARED_CACHE: OnceLock<CacheManager> = OnceLock::new();

/// 获取共享 CacheManager 实例。
pub(crate) fn shared_cache() -> &'static CacheManager {
    SHARED_CACHE.get_or_init(CacheManager::new)
}

/// 以指定字节预算预初始化共享缓存（server 启动前由 CLI `--cache-size` 调用）。
///
/// 已初始化时为 no-op（首次调用生效）。
// 门控与消费者对齐：唯一调用方 run_server_mode（src/cli.rs）需同时具备
// cli 与 server（http+mcp）；仅其一而无另一者时两侧都须编译剔除，
// 否则组合矩阵（如 cli+http 无 mcp）下报 dead-code
#[cfg(all(feature = "cli", feature = "server"))]
pub(crate) fn init_shared_cache(capacity_bytes: u64) {
    let _ = SHARED_CACHE.get_or_init(|| CacheManager::with_capacity_bytes(capacity_bytes));
}
