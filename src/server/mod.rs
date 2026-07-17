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

mod cache;
mod evaluate;
mod types;

#[cfg(feature = "http")]
mod http;
#[cfg(feature = "mcp")]
mod mcp;

pub(crate) use cache::shared_cache;
pub use evaluate::calc_error_to_api_error;
pub use types::{EvaluateRequest, EvaluateResponse, ServerError};

#[cfg(feature = "http")]
pub use http::{build_router, HttpServer};
#[cfg(feature = "mcp")]
pub use mcp::{build_mcp_server, McpServer};
