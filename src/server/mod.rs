// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! CalNexus Server 接口层：HTTP/MCP 多协议服务封装。
//!
//! 基于 sdforge 0.5 框架，将 evaluate 函数暴露为 HTTP API 和 MCP tool。
//! spec.md R-sdforge-002/R-sdforge-003 定义接口契约。
//!
//! # Feature Gate
//!
//! - `http` feature：启用 HTTP server（`POST /api/v1/evaluate`）
//! - `mcp` feature：启用 MCP server（`evaluate` tool，stdio 传输）
//! - `server` feature：HTTP + MCP 聚合

mod cache;
mod catalog;
mod evaluate;
mod types;

#[cfg(all(feature = "fx", feature = "mcp"))]
mod fx_tools;
#[cfg(feature = "http")]
mod http;
#[cfg(feature = "mcp")]
mod mcp;

pub(crate) use cache::shared_cache;

/// 初始化可观测日志（v015 T033，R-srv-004）：observability feature 下安装
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
#[cfg(feature = "cli")]
pub(crate) use cache::init_shared_cache;
pub use catalog::{ListFunctionsRequest, ListFunctionsResponse};
pub use evaluate::calc_error_to_api_error;
pub use types::{EvaluateRequest, EvaluateResponse, ServerError};

#[cfg(feature = "http")]
pub use http::{build_router, HttpServer};
#[cfg(feature = "mcp")]
pub use mcp::{build_mcp_server, McpServer};
