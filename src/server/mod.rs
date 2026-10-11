// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! Server 接口层：将 evaluate 暴露为 HTTP API 与 MCP tool（sdforge 框架）。
//!
//! feature gate：`http`（HTTP server）/ `mcp`（stdio tool）/ `server`（两者聚合）。

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

/// 初始化 inklog 日志单例（消费 `RUST_LOG`，缺省 warn）；重复调用返回 ConfigError 表已初始化。
#[cfg(feature = "observability")]
pub(crate) async fn init_observability() -> Result<(), inklog::InklogError> {
    let mut config = inklog::InklogConfig::default();
    // RUST_LOG 未设时缺省 warn；EnvFilter 原生消费 RUST_LOG 且优先于 global 默认
    config.global.level = std::env::var("RUST_LOG").unwrap_or_else(|_| "warn".to_string());
    config.global.format = "{timestamp} [{level}] {target} - {message}".to_string();
    config.console_sink = Some(inklog::ConsoleSinkConfig {
        enabled: true,
        colored: true,
        ..inklog::ConsoleSinkConfig::default()
    });
    inklog::init_inklog_logger_with_config(config).await
}

#[cfg(not(feature = "observability"))]
pub(crate) async fn init_observability() {}
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

#[cfg(all(test, feature = "observability"))]
mod observability_tests;
