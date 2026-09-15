// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

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

/// 初始化可观测日志：observability feature 下安装
/// tracing-subscriber EnvFilter 消费 `RUST_LOG`（缺省 warn）。幂等（Once）。
#[cfg(feature = "observability")]
pub(crate) fn init_observability() {
    use std::sync::Once;
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| "warn".to_string());
        let _ = tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::new(filter))
            .with_target(false)
            .try_init();
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
