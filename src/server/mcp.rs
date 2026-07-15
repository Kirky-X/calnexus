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
//! - Result: `{"result":5,"domain":"arithmetic","cache":"miss"}`（JSON 字符串放入 ContentBlock::Text）
//! - Error: `{"error":{"kind":"Parse","message":"...","exit_code":1}}` + `is_error=true`

use super::shared_cache;
use super::types::{ErrorDetail, ErrorResponse, EvaluateRequest, EvaluateResponse};
use super::ServerError;
use crate::evaluate;
use crate::CacheManager;
use sdforge::core::ApiMetadata;
use sdforge::mcp::{McpToolRegistration, SdForgeMcpServer, SdForgeTool};
use sdforge::rmcp::model::{CallToolResult, ContentBlock, ErrorData};
use std::future::Future;
use std::sync::Arc;

/// `evaluate` tool：将 CalNexus 的 `evaluate` 函数暴露为 MCP tool。
///
/// 实现 `SdForgeTool` trait。`call()` 内部用 `std::thread::scope` + `spawn`
/// 在独立线程执行 evaluate，避免 oxcache async `block_on` 在 tokio runtime
/// context 中嵌套 panic（"Cannot start a runtime from within a runtime"）。
/// 与 HTTP 模块 `spawn_blocking` 策略同理，但 MCP 的 `call()` 是同步方法，
/// 无法使用 `spawn_blocking`（返回 Future 需 await），故用 `thread::scope`。
#[derive(Debug, Default)]
struct EvaluateTool;

impl SdForgeTool for EvaluateTool {
    fn name(&self) -> &str {
        "evaluate"
    }

    fn description(&self) -> &str {
        "Evaluate a math expression. Returns {result, domain, cache}. Supports vars and precision."
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "expr": {
                    "type": "string",
                    "description": "Math expression to evaluate (e.g. \"2+3\", \"sin(1)\", \"gcd(12,18)\")"
                },
                "vars": {
                    "type": "object",
                    "description": "Variable bindings (e.g. {\"x\": 1.0})",
                    "additionalProperties": {"type": "number"}
                },
                "precision": {
                    "type": ["integer", "null"],
                    "description": "Precision digits for BigRational mode (null = regular mode)",
                    "minimum": 0
                }
            },
            "required": ["expr"]
        })
    }

    fn call(&self, input: Option<serde_json::Value>) -> Result<CallToolResult, ErrorData> {
        // 1. 解析输入参数（None / Null → invalid_params）
        let input = input.unwrap_or(serde_json::Value::Null);
        let req: EvaluateRequest = serde_json::from_value(input)
            .map_err(|e| ErrorData::invalid_params(format!("invalid arguments: {}", e), None))?;

        // 2. 安全校验（vars ≤1024 键，precision ≤10000），与 HTTP handler 同理
        if let Err(e) = req.validate() {
            let resp = ErrorResponse {
                error: ErrorDetail {
                    kind: "Validation".to_string(),
                    message: e.to_string(),
                    exit_code: 2,
                },
            };
            let json = serde_json::to_string(&resp).unwrap_or_else(|_| "{}".to_string());
            // 校验失败是 tool 级错误（用户输入问题），用 CallToolResult::error 而非 Err(ErrorData)
            return Ok(CallToolResult::error(vec![ContentBlock::text(json)]));
        }

        // 3. 准备 evaluate 参数
        let ctx = req.to_eval_context();
        let precision = req.precision;
        let expr = req.expr.clone();

        // 4. 在独立线程执行 evaluate
        //    SdForgeTool::call 是同步方法，但 rmcp 的 ServerHandler::call_tool（async）
        //    从 tokio runtime 内调用 call_tool_internal → call()。
        //    CacheManager 内部用 runtime().block_on() 调 oxcache async API，
        //    在 runtime context 中会 panic。std::thread::scope + spawn 创建无
        //    runtime context 的线程，block_on 安全（与 HTTP spawn_blocking 同理）。
        let result = std::thread::scope(|s| {
            s.spawn(move || {
                let cache: &'static CacheManager = shared_cache();
                evaluate(&expr, &ctx, precision, cache)
            })
            .join()
        });

        // 5. 处理结果（显性化所有失败路径，规则 12）
        match result {
            // evaluate 成功
            Ok(Ok((eval_result, domain, cache_hit, fmt_prec))) => {
                let resp = EvaluateResponse::from_eval(eval_result, domain, cache_hit, fmt_prec);
                let json = serde_json::to_string(&resp).unwrap_or_else(|_| "{}".to_string());
                Ok(CallToolResult::success(vec![ContentBlock::text(json)]))
            }
            // evaluate 返回计算错误（Parse/Eval/Overflow/DivisionByZero 等）
            Ok(Err(e)) => {
                let resp = ErrorResponse::from(&e);
                let json = serde_json::to_string(&resp).unwrap_or_else(|_| "{}".to_string());
                Ok(CallToolResult::error(vec![ContentBlock::text(json)]))
            }
            // spawn 线程 panic（不应发生，但必须显性化处理，规则 12）
            Err(join_err) => {
                let resp = ErrorResponse {
                    error: ErrorDetail {
                        kind: "Internal".to_string(),
                        message: format!("evaluate thread panicked: {:?}", join_err),
                        exit_code: 1,
                    },
                };
                let json = serde_json::to_string(&resp).unwrap_or_else(|_| "{}".to_string());
                Ok(CallToolResult::error(vec![ContentBlock::text(json)]))
            }
        }
    }
}

/// 创建 EvaluateTool 实例（inventory 注册用 fn 指针）。
fn create_evaluate_tool() -> Arc<dyn SdForgeTool> {
    Arc::new(EvaluateTool)
}

/// 创建 evaluate tool 的 ApiMetadata（inventory 注册用 fn 指针）。
fn evaluate_tool_metadata() -> ApiMetadata {
    ApiMetadata::new(
        "calnexus".to_string(),
        "v1".to_string(),
        "CalNexus math expression evaluator".to_string(),
        None,
        false,
    )
}

// 注册 MCP tool 到 sdforge inventory。
// sdforge::mcp::build() 会自动收集此注册并构建 SdForgeMcpServer。
sdforge::inventory::submit!(McpToolRegistration::new(
    "evaluate",
    "v1",
    create_evaluate_tool,
    evaluate_tool_metadata,
));

/// 防止链接器优化掉 CalNexus 的 MCP inventory 注册。
///
/// inventory crate 的 static 项在某些链接器配置下可能被优化掉，
/// 导致 `sdforge::mcp::build()` 无法收集到本 crate 注册的 tool。
/// 此函数通过引用计数强制链接器保留这些符号（与 HTTP 模块同理）。
#[inline(never)]
pub(crate) fn preserve_mcp_inventory() {
    let _count = sdforge::inventory::iter::<McpToolRegistration>().count();
    let _ = _count;
}

/// 构建 CalNexus MCP server：保留 inventory 注册 + `sdforge::mcp::build()`。
///
/// **必须使用此函数**而非直接调用 `sdforge::mcp::build()`，否则链接器可能
/// 优化掉 CalNexus 的 `inventory::submit!` 注册，导致 `evaluate` tool 丢失
/// （与 HTTP `build_router()` 同理的链接器优化问题）。
pub fn build_mcp_server() -> SdForgeMcpServer {
    preserve_mcp_inventory();
    sdforge::mcp::build()
}

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
    /// 供 CLI `--serve-mcp` flag 调用。
    pub fn run(&self) -> Result<(), ServerError> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|e| ServerError::Mcp(format!("failed to create tokio runtime: {}", e)))?;
        runtime.block_on(async move { self.start_inner().await })
    }

    /// 内部 async 启动逻辑：构建 server + serve_stdio。
    ///
    /// 使用 `SdForgeMcpServer::with_server_info` 传入配置的 server 名称。
    /// 注意：sdforge 在有已注册 tool 时会从首个 tool 的 metadata 派生
    /// server_name（即 "calnexus"），`self.server_name` 仅在无 tool 时作 fallback。
    async fn start_inner(&self) -> Result<(), ServerError> {
        preserve_mcp_inventory();
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

impl super::ServerAdapter for McpServer {
    fn start(&self) -> impl Future<Output = Result<(), ServerError>> + Send {
        self.start_inner()
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

    #[test]
    fn test_evaluate_tool_name() {
        let tool = EvaluateTool;
        assert_eq!(tool.name(), "evaluate");
    }

    #[test]
    fn test_evaluate_tool_description() {
        let tool = EvaluateTool;
        assert!(tool.description().contains("Evaluate"));
    }

    #[test]
    fn test_evaluate_tool_input_schema() {
        let tool = EvaluateTool;
        let schema = tool.input_schema();
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["expr"].is_object());
        let required = schema["required"].as_array().unwrap();
        assert!(required.contains(&serde_json::json!("expr")));
    }

    #[test]
    fn test_evaluate_tool_call_invalid_input_null() {
        let tool = EvaluateTool;
        // Value::Null 无法反序列化为 EvaluateRequest（缺少必填 expr 字段）
        let result = tool.call(Some(serde_json::Value::Null));
        assert!(result.is_err());
    }

    #[test]
    fn test_evaluate_tool_call_validation_error_oversized_precision() {
        let tool = EvaluateTool;
        // precision 10001 > MAX_PRECISION 10000，validate() 应拒绝
        let result = tool.call(Some(serde_json::json!({
            "expr": "2+3",
            "precision": 10001
        })));
        // 验证错误返回 Ok(CallToolResult::error(...)) 而非 Err(ErrorData)
        assert!(result.is_ok());
        let call_result = result.unwrap();
        assert_eq!(call_result.is_error, Some(true));
    }

    #[test]
    fn test_create_evaluate_tool() {
        let tool = create_evaluate_tool();
        assert_eq!(tool.name(), "evaluate");
    }

    #[test]
    fn test_evaluate_tool_metadata() {
        let meta = evaluate_tool_metadata();
        assert_eq!(meta.name(), "calnexus");
        assert_eq!(meta.version(), "v1");
        assert!(meta.description().contains("CalNexus"));
    }

    #[test]
    fn test_preserve_mcp_inventory() {
        // 仅验证不 panic（链接器保留符号）
        preserve_mcp_inventory();
    }
}
