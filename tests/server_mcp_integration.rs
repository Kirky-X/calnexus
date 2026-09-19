// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! MCP server 集成测试：`evaluate` tool（`#[forge]` 宏生成）。
//!
//! tool 由 `#[forge]` 声明式生成（sdforge-macros codegen）。
//!
//! **输入格式**：`{"req":{"expr":"2+3","vars":{...},"precision":N}}` —— `req` 是
//! `#[forge]` 函数参数名，macros 生成 `struct Params { req: EvaluateRequest }`
//! （macros lib.rs:1306-1317），故 tool args 必须包一层 `req`。
//!
//! **错误响应契约**（macros lib.rs:1381-1399）：`CallToolResult::error(...)`
//! 设 `is_error=Some(true)`，`content[0].text` 为 ApiError 的 serde JSON
//! （`#[serde(tag="type")]`，如 `{"type":"InvalidInput"|"ValidationError"|...}`），
//! 与 HTTP 错误 body 同构——codegen 用 `serde_json::to_value(e)` 序列化 ApiError，
//! 不经 `to_mcp_json`（该方法存在但 codegen 未调用）。
//!
//! 测试使用 `SdForgeMcpServer::call_tool_internal` 直接调用 tool，无需启动 stdio。
use calnexus::build_mcp_server;
use sdforge::rmcp::model::{CallToolResult, ContentBlock};
use serde_json::{Value, json};

/// 从 `CallToolResult` 提取首个文本内容并解析为 JSON。
///
/// `#[forge]` codegen 将 EvaluateResponse/ApiError 序列化为 JSON 字符串
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

/// 标量求值：evaluate "2+3" → {"result":5,"domain":"arithmetic","cache":"miss"}
#[test]
fn test_mcp_tool_evaluate_scalar() {
    let server = build_mcp_server();
    let result = server
        .call_tool_internal("evaluate", Some(json!({"req": {"expr": "2+3"}})))
        .expect("evaluate tool should be registered");

    assert_ne!(result.is_error, Some(true), "should not be an error result");
    let body = extract_result_json(&result);
    assert_eq!(body["result"], 5);
    assert_eq!(body["domain"], "arithmetic");
    assert_eq!(body["cache"], "miss");
}

/// 带变量求值：evaluate "x+1" vars {x:10} → result 11
/// 参照 变量语义。
#[test]
fn test_mcp_tool_evaluate_with_vars() {
    let server = build_mcp_server();
    let result = server
        .call_tool_internal(
            "evaluate",
            Some(json!({"req": {"expr": "x+1", "vars": {"x": 10.0}}})),
        )
        .expect("evaluate tool should be registered");

    assert_ne!(result.is_error, Some(true));
    let body = extract_result_json(&result);
    assert_eq!(body["result"], 11);
    assert_eq!(body["domain"], "arithmetic");
}

/// 精度模式：evaluate "1/3" precision 2 → result "0.33"
/// precision 模式 result 是格式化字符串。
#[test]
fn test_mcp_tool_evaluate_precision() {
    let server = build_mcp_server();
    let result = server
        .call_tool_internal(
            "evaluate",
            Some(json!({"req": {"expr": "1/3", "precision": 2}})),
        )
        .expect("evaluate tool should be registered");

    assert_ne!(result.is_error, Some(true));
    let body = extract_result_json(&result);
    assert_eq!(body["domain"], "precision");
    // precision 模式下 result 是字符串（BigRational 格式化）
    assert_eq!(body["result"].as_str(), Some("0.33"));
}

/// 计算错误：evaluate "2++3" → ApiError::InvalidInput 契约。
///
/// `is_error=Some(true)` + content 为 `{"type":"InvalidInput","message":"Parse: ...",...}`
/// （macros codegen 用 `serde_json::to_value(ApiError)` 序列化，与 HTTP 错误 body 同构）。
#[test]
fn test_mcp_tool_evaluate_calc_error_invalid_input() {
    let server = build_mcp_server();
    let result = server
        .call_tool_internal("evaluate", Some(json!({"req": {"expr": "2++3"}})))
        .expect("evaluate tool should be registered");

    assert_eq!(
        result.is_error,
        Some(true),
        "parse error should set is_error"
    );
    let body = extract_result_json(&result);
    assert_eq!(body["type"], "InvalidInput");
    // message 含 ErrorKind 前缀（calc_error_to_api_error 注入 "Parse:"）
    assert!(
        body["message"]
            .as_str()
            .is_some_and(|m| m.contains("Parse")),
        "message 应含 ErrorKind 前缀: {}",
        body["message"]
    );
}

/// validate precision 超限 → ApiError::ValidationError 契约。
///
/// `is_error=Some(true)` + content 为
/// `{"type":"ValidationError","field":"precision","constraint":"10001 exceeds limit 10000"}`
#[test]
fn test_mcp_tool_evaluate_validation_error_oversized_precision() {
    let server = build_mcp_server();
    let result = server
        .call_tool_internal(
            "evaluate",
            Some(json!({"req": {"expr": "2+3", "precision": 10001}})),
        )
        .expect("evaluate tool should be registered");

    assert_eq!(result.is_error, Some(true));
    let body = extract_result_json(&result);
    assert_eq!(body["type"], "ValidationError");
    assert_eq!(body["field"], "precision");
}

/// validate vars 键数超限（>1024）→ ApiError::ValidationError 契约。
///
/// `is_error=Some(true)` + content 为
/// `{"type":"ValidationError","field":"vars","constraint":"size 1025 exceeds limit 1024"}`
#[test]
fn test_mcp_tool_evaluate_validation_error_oversized_vars() {
    let server = build_mcp_server();
    // 构造 1025 个 vars（超过 MAX_VARS=1024）
    let mut vars = serde_json::Map::new();
    for i in 0..=1024u32 {
        vars.insert(format!("v{i}"), json!(0.0));
    }
    let result = server
        .call_tool_internal(
            "evaluate",
            Some(json!({"req": {"expr": "v1", "vars": vars}})),
        )
        .expect("evaluate tool should be registered");

    assert_eq!(result.is_error, Some(true));
    let body = extract_result_json(&result);
    assert_eq!(body["type"], "ValidationError");
    assert_eq!(body["field"], "vars");
}

/// 无效输入：evaluate null → Err(ErrorData)
///
/// `req` 字段缺失，macros codegen serde 解析
/// `Params{req:EvaluateRequest}` 失败（macros lib.rs:1311-1316），返回
/// `Err(ErrorData)`（协议层错误，非 CallToolResult）。
#[test]
fn test_mcp_tool_evaluate_invalid_input_null() {
    let server = build_mcp_server();
    let result = server.call_tool_internal("evaluate", Some(serde_json::Value::Null));
    assert!(result.is_err(), "null input should return Err(ErrorData)");
}

// ===== MCP 增强 =====

/// MCP-DESC-01: evaluate 工具 description 自包含（req 包装示例 + 上限 + 错误语义）。
#[test]
fn test_evaluate_tool_description_self_contained() {
    let server = build_mcp_server();
    let tools = server.get_all_tools();
    let eval = tools
        .iter()
        .find(|t| t.name.as_ref() == "evaluate")
        .expect("evaluate tool 应已注册");
    let desc = eval
        .description
        .as_ref()
        .map(|d| d.to_string())
        .unwrap_or_default();
    assert!(
        desc.contains("req"),
        "description 应含 req 包装示例: {}",
        desc
    );
    assert!(desc.contains("4096"), "应含 expr 上限: {}", desc);
    assert!(desc.contains("503"), "应含 503 错误语义: {}", desc);
}

/// MCP-CATALOG-01: list_functions tool 可调用并返回按域分组的目录。
#[test]
fn test_list_functions_tool_callable() {
    let server = build_mcp_server();
    let r = server
        .call_tool_internal("list_functions", Some(serde_json::json!({"req": {}})))
        .expect("list_functions call_tool_internal");
    assert!(!r.is_error.unwrap_or(false), "list_functions 应成功");
    let text = serde_json::to_string(&r.content).unwrap_or_default();
    assert!(
        text.contains("arithmetic"),
        "目录应含 arithmetic 域: {}",
        text
    );
    assert!(text.contains("symbolic"), "目录应含 symbolic 域: {}", text);
}

/// MCP-CATALOG-02: fx feature 开启时目录含 fx 域（feature 门控可见性）。
#[test]
fn test_list_functions_feature_gated_domains() {
    let server = build_mcp_server();
    let r = server
        .call_tool_internal("list_functions", Some(serde_json::json!({"req": {}})))
        .expect("list_functions call_tool_internal");
    let text = serde_json::to_string(&r.content).unwrap_or_default();
    if cfg!(feature = "fx") {
        // content 经 JSON 字符串化，内层引号已转义；用唯一函数名判定域存在
        assert!(
            text.contains("fx_rate"),
            "fx feature 下目录应含 fx 域: {}",
            text
        );
    }
}

// === 语言协商（lang 请求字段，MCP tool args 与 HTTP body 同构）===

/// MCP-COLLANG-01: evaluate tool args 带 lang=zh → 错误消息本地化为中文。
#[test]
fn test_mcp_evaluate_zh_lang_localized_error() {
    let server = build_mcp_server();
    let r = server
        .call_tool_internal(
            "evaluate",
            Some(serde_json::json!({"req": {"expr": "foo + 1", "lang": "zh"}})),
        )
        .expect("evaluate call_tool_internal");
    assert!(r.is_error.unwrap_or(false), "未定义变量应报错");
    let text = serde_json::to_string(&r.content).unwrap_or_default();
    assert!(
        text.contains("求值错误") || text.contains("未绑定变量"),
        "zh 协商应返回中文消息: {}",
        text
    );
}

/// MCP-COLLANG-02: 缺省 lang 跟随系统语言检测链（T006 起契约变更）；
/// 检测为 en 时保持英文契约。
#[test]
fn test_mcp_evaluate_default_lang_follows_detection() {
    let expect_zh = calnexus::I18n::from_detected().lang() == calnexus::Lang::Zh;
    let server = build_mcp_server();
    let r = server
        .call_tool_internal(
            "evaluate",
            Some(serde_json::json!({"req": {"expr": "foo + 1"}})),
        )
        .expect("evaluate call_tool_internal");
    let text = serde_json::to_string(&r.content).unwrap_or_default();
    if expect_zh {
        assert!(
            text.contains("未绑定变量"),
            "检测为 zh 时缺省 lang 应输出中文: {text}"
        );
    } else {
        assert!(
            text.contains("unbound variable"),
            "检测为 en 时缺省 lang 应保持英文契约: {text}"
        );
    }
}
