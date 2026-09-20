// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! CalNexus Server 接口层：HTTP/MCP 多协议服务封装。
//!
//! 基于 sdforge 0.5 框架，将 evaluate 函数暴露为 HTTP API 和 MCP tool。
//!
//! # Feature Gate
//!
//! - `http` feature：启用 HTTP server（`POST /api/v1/evaluate`）
//! - `mcp` feature：启用 MCP server（`evaluate` tool，stdio 传输）
//! - `server` feature：HTTP + MCP 聚合

mod cache;
mod catalog;
mod evaluate;
mod lang;
mod types;

#[cfg(all(feature = "fx", feature = "mcp"))]
mod fx_tools;
#[cfg(feature = "http")]
mod http;
#[cfg(feature = "mcp")]
mod mcp;
#[cfg(feature = "ratelimit")]
mod ratelimit;

pub(crate) use cache::shared_cache;

/// 初始化可观测日志：observability feature 下由 inklog LoggerManager 接管
/// （消费 `RUST_LOG`，缺省 warn；同时捕获 sdforge/tower 等依赖的 tracing 事件）。
/// 幂等（Once）。
///
/// inklog 构建是异步的，而本函数会被同步与异步（含 `#[tokio::test]` 的
/// current-thread runtime）两种上下文调用——统一在专用线程上建独立 runtime
/// 完成构建（常驻，LoggerManager 异步任务依赖其存活），对所有调用上下文零 panic 风险。
#[cfg(feature = "observability")]
pub(crate) fn init_observability() {
    use std::sync::Once;
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let level = std::env::var("RUST_LOG").unwrap_or_else(|_| "warn".to_string());
        let result = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("observability runtime");
            let logger = rt.block_on(async {
                inklog::LoggerManager::builder()
                    .level(&level)
                    .format("{timestamp} [{level}] {target} - {message}")
                    .console(true)
                    .console_colored(true)
                    .build()
                    .await
            });
            (logger, rt)
        })
        .join()
        .expect("inklog init thread panicked");
        let (logger, rt) = result;
        if let Ok(logger) = logger {
            // 进程生命周期内常驻；LoggerManager 异步任务依赖 runtime 存活，二者均不释放
            std::mem::forget(logger);
            std::mem::forget(rt);
        }
    });
}

#[cfg(not(feature = "observability"))]
pub(crate) fn init_observability() {}
// 消费者 run_server_mode 需同时具备 cli 与 server；门控与其一致
#[cfg(all(feature = "cli", feature = "server"))]
pub(crate) use cache::init_shared_cache;
pub use catalog::{ListFunctionsRequest, ListFunctionsResponse};
pub use evaluate::{calc_error_to_api_error, calc_error_to_api_error_i18n};
pub use types::{EvaluateRequest, EvaluateResponse, ServerError};

#[cfg(feature = "http")]
pub use http::{HttpServer, build_router};
#[cfg(feature = "mcp")]
pub use mcp::{McpServer, build_mcp_server};

#[cfg(test)]
mod observability_tests {
    /// init_observability 幂等，重复调用无 panic。
    #[test]
    fn test_init_observability_idempotent() {
        crate::server::init_observability();
        crate::server::init_observability();
    }
}
