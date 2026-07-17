// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! HTTP server 集成测试：`POST /api/v1/evaluate` 端点（`#[forge]` 宏生成）。
//!
//! P3（sdforge-forge-migration）：路由由 `#[forge]` 声明式生成，错误响应遵循
//! sdforge `ApiError` 标准契约（spec.md R-sdforge-002）：
//! - 计算错误（Parse/DivisionByZero/...）→ 400 `{"type":"InvalidInput","message":"{Kind}: ...",...}`
//! - validate vars/precision 超限 → 422 `{"type":"ValidationError","field":"...","constraint":"..."}`
//!
//! 测试使用 `tower::ServiceExt::oneshot` 直接测试 Router，无需启动真实 server。

#![cfg(feature = "server")]

use calnexus::build_router;
use http_body_util::BodyExt;
use sdforge::axum::http::status::StatusCode;
use sdforge::axum::http::Request;
use sdforge::axum::Body;
use serde_json::{json, Value};
use tower::ServiceExt;

/// 构建 POST /api/v1/evaluate 请求。
fn make_evaluate_request(body: Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/api/v1/evaluate")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap()
}

/// 发送请求并返回 (status, body_json)。
async fn send_request(body: Value) -> (StatusCode, Value) {
    let router = build_router();
    let response = router
        .oneshot(make_evaluate_request(body))
        .await
        .expect("router oneshot failed");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body collection failed")
        .to_bytes();
    let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

/// 标量求值：POST "2+3" → 200 {"result":5,"domain":"arithmetic","cache":"miss"}
#[tokio::test]
async fn test_http_evaluate_scalar() {
    let (status, body) = send_request(json!({"expr": "2+3"})).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["result"], 5);
    assert_eq!(body["domain"], "arithmetic");
    assert_eq!(body["cache"], "miss");
}

/// 带变量求值：POST {"expr":"x+1","vars":{"x":10}} → 200 {"result":11,...}
#[tokio::test]
async fn test_http_evaluate_with_vars() {
    let (status, body) = send_request(json!({"expr": "x+1", "vars": {"x": 10.0}})).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["result"], 11);
    assert_eq!(body["domain"], "arithmetic");
}

/// 精度模式：POST {"expr":"1/3","precision":5} → 200 {"result":"0.33333","domain":"precision",...}
#[tokio::test]
async fn test_http_evaluate_precision() {
    let (status, body) = send_request(json!({"expr": "1/3", "precision": 5})).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["domain"], "precision");
    // precision 模式下 result 是字符串（BigRational 格式化）
    assert!(body["result"].as_str().is_some());
}

/// 缓存行为：首次请求 miss，第二次请求 hit。
///
/// spec.md R-sdforge-002 缓存语义：相同表达式第二次请求命中缓存。
#[tokio::test]
async fn test_http_evaluate_cache_miss() {
    // 首次请求：缓存未命中
    let (status, body) = send_request(json!({"expr": "7+8"})).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["cache"], "miss");
    assert_eq!(body["result"], 15);

    // 第二次请求相同表达式：缓存命中
    let (status, body) = send_request(json!({"expr": "7+8"})).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["cache"], "hit");
    assert_eq!(body["result"], 15);
}

/// 计算错误（1/0）→ ApiError::InvalidInput 契约（spec.md R-sdforge-002）。
///
/// 400 + `{"type":"InvalidInput","message":"DivisionByZero: ...","field":null,"value":null}`
#[tokio::test]
async fn test_http_evaluate_calc_error_invalid_input() {
    let (status, body) = send_request(json!({"expr": "1/0"})).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["type"], "InvalidInput");
    // message 含 ErrorKind 前缀（calc_error_to_api_error 注入 "DivisionByZero:"）
    assert!(
        body["message"]
            .as_str()
            .is_some_and(|m| m.contains("DivisionByZero")),
        "message 应含 ErrorKind 前缀: {}",
        body["message"]
    );
}

/// validate precision 超限 → ApiError::ValidationError 契约（spec.md R-sdforge-002）。
///
/// 422 + `{"type":"ValidationError","field":"precision","constraint":"10001 exceeds limit 10000"}`
#[tokio::test]
async fn test_http_evaluate_validation_error_oversized_precision() {
    let (status, body) = send_request(json!({"expr": "2+3", "precision": 10001})).await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["type"], "ValidationError");
    assert_eq!(body["field"], "precision");
    assert!(
        body["constraint"]
            .as_str()
            .is_some_and(|c| c.contains("exceeds limit")),
        "constraint 应描述限额: {}",
        body["constraint"]
    );
}

/// validate vars 键数超限（>1024）→ ApiError::ValidationError 契约（spec.md R-sdforge-002）。
///
/// 422 + `{"type":"ValidationError","field":"vars","constraint":"size 1025 exceeds limit 1024"}`
#[tokio::test]
async fn test_http_evaluate_validation_error_oversized_vars() {
    // 构造 1025 个 vars（超过 MAX_VARS=1024）
    let mut vars = serde_json::Map::new();
    for i in 0..=1024u32 {
        vars.insert(format!("v{i}"), json!(0.0));
    }
    let (status, body) = send_request(json!({"expr": "v1", "vars": vars})).await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["type"], "ValidationError");
    assert_eq!(body["field"], "vars");
}
