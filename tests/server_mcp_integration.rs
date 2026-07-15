// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! MCP server 集成测试：`evaluate` tool。
//!
//! T017（Red 阶段）：测试定义期望行为，`evaluate` tool 未注册，
//! `call_tool_internal` 返回 `Err(ErrorData)`（tool not found），测试失败。
//! T018（Green 阶段）：实现 `EvaluateTool` + inventory 注册，测试通过。
//!
//! 测试使用 `SdForgeMcpServer::call_tool_internal` 直接调用 tool，
//! 无需启动 stdio 传输。

#![cfg(feature = "server")]

use calnexus::build_mcp_server;
use sdforge::rmcp::model::{CallToolResult, ContentBlock};
use serde_json::{json, Value};

/// 从 `CallToolResult` 提取首个文本内容并解析为 JSON。
///
/// evaluate tool 将 `EvaluateResponse`/`ErrorResponse` 序列化为 JSON 字符串
/// 放入 `ContentBlock::Text`。此辅助函数提取并解析该 JSON。
fn extract_result_json(result: &CallToolResult) -> Value {
    let text = result.content.iter().find_map(|c| match c {
        ContentBlock::Text(t) => Some(t.text.as_str()),
        _ => None,
    });
    let text = text.expect("expected text content in CallToolResult");
    serde_json::from_str(text).expect("text content should be valid JSON")
}

/// 测试 tool list 包含 evaluate tool。
///
/// spec.md R-sdforge-003：MCP tool list 包含 `evaluate` tool。
#[test]
fn test_mcp_tool_list_contains_evaluate() {
    let server = build_mcp_server();
    let tools = server.get_all_tools();
    let has_evaluate = tools.iter().any(|t| t.name.as_ref() == "evaluate");
    assert!(
        has_evaluate,
        "evaluate tool should be registered; found tools: {:?}",
        tools.iter().map(|t| t.name.as_ref()).collect::<Vec<_>>()
    );
}

/// 测试标量求值：evaluate "2+3" → {"result":5,"domain":"arithmetic","cache":"miss"}
///
/// spec.md R-sdforge-003：`evaluate` tool args `{"expr":"2+3"}` → result 5。
#[test]
fn test_mcp_tool_evaluate_scalar() {
    let server = build_mcp_server();
    let result = server
        .call_tool_internal("evaluate", Some(json!({"expr": "2+3"})))
        .expect("evaluate tool should be registered");

    assert_ne!(result.is_error, Some(true), "should not be an error result");
    let body = extract_result_json(&result);
    assert_eq!(body["result"], 5);
    assert_eq!(body["domain"], "arithmetic");
    assert_eq!(body["cache"], "miss");
}

/// 测试带变量求值：evaluate "x+1" vars {x:10} → result 11
///
/// spec.md R-sdforge-003 参照 R-sdforge-002 变量语义。
#[test]
fn test_mcp_tool_evaluate_with_vars() {
    let server = build_mcp_server();
    let result = server
        .call_tool_internal(
            "evaluate",
            Some(json!({"expr": "x+1", "vars": {"x": 10.0}})),
        )
        .expect("evaluate tool should be registered");

    assert_ne!(result.is_error, Some(true));
    let body = extract_result_json(&result);
    assert_eq!(body["result"], 11);
    assert_eq!(body["domain"], "arithmetic");
}

/// 测试错误结果：evaluate "2++3" → is_error Some(true)
///
/// spec.md R-sdforge-003：`evaluate` tool args `{"expr":"2++3"}` → error result。
#[test]
fn test_mcp_tool_evaluate_error() {
    let server = build_mcp_server();
    let result = server
        .call_tool_internal("evaluate", Some(json!({"expr": "2++3"})))
        .expect("evaluate tool should be registered");

    assert_eq!(
        result.is_error,
        Some(true),
        "parse error should set is_error"
    );
    let body = extract_result_json(&result);
    assert!(
        body.get("error").is_some(),
        "error response should contain 'error'"
    );
    assert!(body["error"]["kind"].as_str().is_some());
    assert!(body["error"]["message"].as_str().is_some());
}

/// 测试精度模式：evaluate "1/3" precision 2 → result "0.33"
///
/// spec.md R-sdforge-002/R-sdforge-003：precision 模式 result 是格式化字符串。
#[test]
fn test_mcp_tool_evaluate_precision() {
    let server = build_mcp_server();
    let result = server
        .call_tool_internal("evaluate", Some(json!({"expr": "1/3", "precision": 2})))
        .expect("evaluate tool should be registered");

    assert_ne!(result.is_error, Some(true));
    let body = extract_result_json(&result);
    assert_eq!(body["domain"], "precision");
    // precision 模式下 result 是字符串（BigRational 格式化）
    assert_eq!(body["result"].as_str(), Some("0.33"));
}
