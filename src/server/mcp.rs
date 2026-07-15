// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! MCP server 实现：`evaluate` tool，stdio 传输。
//!
//! 基于 sdforge mcp 模块（rmcp SDK + inventory 工具注册）。
//! spec.md R-sdforge-003 定义接口契约。
//!
//! # Tool
//!
//! `evaluate`
//! - Args: `{"expr":"2+3","vars":{"x":1.0},"precision":null}`
//! - Result: `{"result":5,"domain":"arithmetic","cache":"miss"}`
//! - Error: 同 HTTP ErrorDetail

use super::ServerError;

/// MCP server 配置。
#[derive(Debug, Clone)]
pub struct McpServer {
    /// server 名称（默认 "calnexus-mcp"）。
    server_name: String,
}

impl Default for McpServer {
    fn default() -> Self {
        Self::new()
    }
}

impl McpServer {
    /// 创建默认配置的 MCP server。
    pub fn new() -> Self {
        Self {
            server_name: "calnexus-mcp".to_string(),
        }
    }

    /// 自定义 server 名称。
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.server_name = name.into();
        self
    }

    /// 获取 server 名称。
    pub fn server_name(&self) -> &str {
        &self.server_name
    }

    /// 同步入口：创建 tokio runtime 阻塞运行 `start()`。
    /// 供 CLI `--serve-mcp` flag 调用。T018 实现 ServerAdapter 后启用。
    pub fn run(&self) -> Result<(), ServerError> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|e| ServerError::Mcp(format!("failed to create tokio runtime: {}", e)))?;
        runtime.block_on(async move { self.start_inner().await })
    }

    /// 内部 async 启动逻辑（T018 实现）。
    async fn start_inner(&self) -> Result<(), ServerError> {
        // T018 将实现：SdForgeTool trait + inventory 注册 + sdforge::mcp::serve_stdio
        Err(ServerError::Mcp("not yet implemented (T018)".into()))
    }
}
