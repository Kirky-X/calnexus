// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! CalNexus Server 接口层：HTTP/MCP 多协议服务封装。
//!
//! 基于 sdforge 0.4 框架，将 evaluate 函数暴露为 HTTP API 和 MCP tool。
//! spec.md R-sdforge-002/R-sdforge-003 定义接口契约。
//!
//! # Feature Gate
//!
//! - `http` feature：启用 HTTP server（`POST /api/v1/evaluate`）
//! - `mcp` feature：启用 MCP server（`evaluate` tool，stdio 传输）
//! - `server` feature：HTTP + MCP 聚合

use std::future::Future;

mod types;

#[cfg(feature = "http")]
mod http;
#[cfg(feature = "mcp")]
mod mcp;

pub use types::{ErrorDetail, ErrorResponse, EvaluateRequest, EvaluateResponse, ServerError};

#[cfg(feature = "http")]
pub use http::HttpServer;
#[cfg(feature = "mcp")]
pub use mcp::McpServer;

/// Server 适配器 trait：统一 HTTP/MCP server 的启动接口。
///
/// `start()` 返回 `Send` Future，可在 tokio multi-thread runtime 内 spawn。
/// 同步入口由 `run()` 方法提供（内部创建 tokio runtime 阻塞运行）。
pub trait ServerAdapter {
    /// 启动 server，阻塞直到 server 停止或出错。
    fn start(&self) -> impl Future<Output = Result<(), ServerError>> + Send;
}
