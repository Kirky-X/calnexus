// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! MCP server 启动：构建/启动 SdForgeMcpServer（stdio 传输）；
//! `evaluate` tool 由 `#[forge]` 宏声明（evaluate.rs）。

use super::ServerError;
use sdforge::mcp::SdForgeMcpServer;

/// 构建 MCP server：`init_all_plugins()` 确保 `#[forge]` 注册的 tool 被 `mcp::build()` 收集。
pub fn build_mcp_server() -> SdForgeMcpServer {
    sdforge::init_all_plugins();
    sdforge::mcp::build()
}

/// MCP server 配置（stdio 传输，仅 server_name）。
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

    /// 同步入口（CLI `--serve-mcp` 调用）；multi-thread runtime 为 MCP `call()` 内 `block_in_place` 要求。
    pub fn run(&self) -> Result<(), ServerError> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|e| ServerError::Mcp(format!("failed to create tokio runtime: {}", e)))?;
        runtime.block_on(async move { self.start_inner().await })
    }

    /// 内部 async 启动逻辑：`init_all_plugins()` + `with_server_info` + `serve_stdio`。
    async fn start_inner(&self) -> Result<(), ServerError> {
        sdforge::init_all_plugins();
        let server = SdForgeMcpServer::with_server_info(
            self.server_name.clone(),
            env!("CARGO_PKG_VERSION").to_string(),
        );
        sdforge::mcp::serve_stdio(server)
            .await
            .map_err(|e| ServerError::Mcp(format!("stdio serve error: {}", e)))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_server_new_default_name() {
        let server = McpServer::new();
        assert_eq!(server.server_name(), "calnexus-mcp");
    }

    #[test]
    fn test_mcp_server_default_equals_new() {
        let server = McpServer::default();
        assert_eq!(server.server_name(), "calnexus-mcp");
    }

    #[test]
    fn test_mcp_server_with_name_custom() {
        let server = McpServer::new().with_name("custom-mcp");
        assert_eq!(server.server_name(), "custom-mcp");
    }

    /// 验证 `#[forge]` evaluate tool 注册与调用：tool_count 不符=inventory 丢失，panic=runtime 错误。
    #[test]
    fn test_forge_evaluate_registers_and_calls() {
        let server = build_mcp_server();
        // fx feature 下 4 个 tool（evaluate/list_functions/fx_budget/fx_pricing），否则 2 个
        let expected_count = if cfg!(feature = "fx") { 4 } else { 2 };
        assert_eq!(
            server.tool_count(),
            expected_count,
            "forge tools must register via #[forge]+init_all_plugins; \
             count mismatch ⇒ inventory submit lost (linker) → A2 回退"
        );
        let r1 = server
            .call_tool_internal("evaluate", Some(serde_json::json!({"req":{"expr":"2+3"}})))
            .expect("scalar call_tool_internal no panic");
        assert!(!r1.is_error.unwrap_or(false), "2+3 should succeed");
        let r2 = server
            .call_tool_internal(
                "evaluate",
                Some(serde_json::json!({"req":{"expr":"x+1","vars":{"x":10}}})),
            )
            .expect("vars call_tool_internal no panic");
        assert!(!r2.is_error.unwrap_or(false), "x+1 vars should succeed");
    }
}
