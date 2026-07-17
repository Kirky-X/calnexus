// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! MCP server 启动：`evaluate` tool 由 `#[forge]` 宏声明（evaluate.rs），
//! 本模块仅负责构建/启动 SdForgeMcpServer（stdio 传输）。
//!
//! spec.md R-sdforge-003 定义接口契约。

use super::ServerError;
use sdforge::mcp::SdForgeMcpServer;

/// 构建 CalNexus MCP server：`init_all_plugins()` + `sdforge::mcp::build()`。
///
/// `init_all_plugins()` 替代 p1 的 `preserve_mcp_inventory()` 链接器 hack，
/// 确保 `#[forge]` 注册的 evaluate tool 被 `mcp::build()` 收集。
pub fn build_mcp_server() -> SdForgeMcpServer {
    sdforge::init_all_plugins();
    sdforge::mcp::build()
}

/// MCP server 配置（stdio 传输，无 host/port 概念，仅 server_name）。
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

    /// 同步入口：创建 multi-thread tokio runtime 阻塞运行 `start()`。
    /// 供 CLI `--serve-mcp` flag 调用。multi-thread 是 `#[forge]` MCP `call()`
    /// 内部 `block_in_place` 的要求（spec.md R-sdforge-007 约束）。
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

    /// T002(h) spike gate：验证 `#[forge]` 宏生成的 evaluate tool 经
    /// `init_all_plugins()` + inventory 收集后注册成功，且调用无 runtime panic。
    ///
    /// `call_tool_internal` → `tool().call()` 内部 `Handle::try_current()` 为 Err
    /// 分支会自创 multi-thread runtime（macros line 1354-1359），故本同步测试
    /// 无需外层 tokio runtime。
    ///
    /// 失败条件（→ 触发 design A2 MCP 回退）：
    /// - `tool_count != 1`：inventory submit 丢失（链接器未收集 #[forge] 注册）
    /// - panic：`block_in_place` / `spawn_blocking` / evaluate 调用链 runtime 错误
    #[test]
    fn test_forge_evaluate_registers_and_calls() {
        let server = build_mcp_server();
        assert_eq!(
            server.tool_count(),
            1,
            "evaluate tool must register via #[forge]+init_all_plugins; \
             count==0 ⇒ inventory submit lost (linker) → A2 回退"
        );
        // 标量 2+3 → 5（成功路径）
        let r1 = server
            .call_tool_internal("evaluate", Some(serde_json::json!({"req":{"expr":"2+3"}})))
            .expect("scalar call_tool_internal no panic");
        assert!(!r1.is_error.unwrap_or(false), "2+3 should succeed");
        // 带变量 x+1 (x=10) → 11（成功路径）
        let r2 = server
            .call_tool_internal(
                "evaluate",
                Some(serde_json::json!({"req":{"expr":"x+1","vars":{"x":10}}})),
            )
            .expect("vars call_tool_internal no panic");
        assert!(!r2.is_error.unwrap_or(false), "x+1 vars should succeed");
    }
}
