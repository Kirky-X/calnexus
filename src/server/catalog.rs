// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! 函数目录接口（v015 T038，R-mcp-002）：运行时自描述能力。
//!
//! sdforge 0.5.0-rc.2 的 MCP capabilities 声明 `resources: false`（不支持
//! resource 暴露），故按 design 降级方案以 **tool** 形态提供函数目录；
//! CLI 侧对应 `--list-functions`。数据源为统一目录
//! [`crate::function_catalog`]（单一事实源）。

use sdforge::error::ApiError;
use sdforge::forge;

/// `list_functions` 请求（无参数，占位结构以统一 #[forge] 契约）。
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ListFunctionsRequest {}

/// 单域函数目录条目。
#[derive(Debug, Clone, serde::Serialize)]
pub struct DomainFunctions {
    /// 域名（与 DomainRouter domain_name 一致）。
    pub domain: String,
    /// 该域函数名列表。
    pub functions: Vec<String>,
}

/// `list_functions` 响应。
#[derive(Debug, Clone, serde::Serialize)]
pub struct ListFunctionsResponse {
    /// 按域分组的函数目录（仅含当前构建启用的 feature 域）。
    pub domains: Vec<DomainFunctions>,
}

/// `POST /api/v1/list_functions` + MCP `list_functions` tool。
///
/// 运行时函数目录：LLM/客户端可发现当前构建可用的全部函数（含 feature 门控域）。
#[forge(
    name = "list_functions",
    version = 1,
    path = "/list_functions",
    method = "POST",
    tool_name = "list_functions",
    description = "List all available math functions grouped by domain. Call this first to discover which functions the current build supports (feature-gated domains like time/unit/fx appear only when enabled)."
)]
pub(crate) async fn list_functions(
    req: ListFunctionsRequest,
) -> Result<ListFunctionsResponse, ApiError> {
    let _ = req; // 无字段占位请求；统一 req 包装契约（schema 由宏按参数名推导）
    list_functions_inner().await
}

/// 目录构建内核（forge wrapper 与显式 HTTP wrapper 共用）。
pub(crate) async fn list_functions_inner() -> Result<ListFunctionsResponse, ApiError> {
    Ok(ListFunctionsResponse {
        domains: crate::function_catalog::DOMAIN_FUNCTIONS
            .iter()
            .map(|(domain, functions)| DomainFunctions {
                domain: (*domain).to_string(),
                functions: functions.iter().map(|f| (*f).to_string()).collect(),
            })
            .collect(),
    })
}
