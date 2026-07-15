// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! HTTP server 集成测试：POST /api/v1/evaluate 端点。
//!
//! T015（Red 阶段）：测试定义期望行为，evaluate_handler 返回 501，测试失败。
//! T016（Green 阶段）：实现 evaluate_handler，测试通过。
//!
//! 测试使用 `tower::ServiceExt::oneshot` 直接测试 Router，无需启动真实 server。

#![cfg(feature = "server")]

use calnexus::build_router;
use http_body_util::BodyExt;
use sdforge::axum::Body;
use sdforge::axum::http::Request;
use sdforge::axum::http::status::StatusCode;
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

/// 测试标量求值：POST "2+3" → 200 {"result":5,"domain":"arithmetic","cache":"miss"}
#[tokio::test]
async fn test_http_evaluate_scalar() {
    let (status, body) = send_request(json!({"expr": "2+3"})).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["result"], 5);
    assert_eq!(body["domain"], "arithmetic");
    assert_eq!(body["cache"], "miss");
}

/// 测试带变量求值：POST {"expr":"x+1","vars":{"x":10}} → 200 {"result":11,...}
#[tokio::test]
async fn test_http_evaluate_with_vars() {
    let (status, body) = send_request(json!({
        "expr": "x+1",
        "vars": {"x": 10.0}
    }))
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["result"], 11);
    assert_eq!(body["domain"], "arithmetic");
}

/// 测试错误退出码：POST "1/0" → 400 {"error":{"kind":"DivisionByZero","exit_code":1,...}}
#[tokio::test]
async fn test_http_evaluate_error_exit_code() {
    let (status, body) = send_request(json!({"expr": "1/0"})).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["exit_code"], 1);
    assert!(body["error"]["kind"].as_str().is_some());
    assert!(body["error"]["message"].as_str().is_some());
}

/// 测试精度模式：POST {"expr":"1/3","precision":5} → 200 {"result":"0.33333","domain":"precision",...}
#[tokio::test]
async fn test_http_evaluate_precision() {
    let (status, body) = send_request(json!({
        "expr": "1/3",
        "precision": 5
    }))
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["domain"], "precision");
    // precision 模式下 result 是字符串（BigRational 格式化）
    assert!(body["result"].as_str().is_some());
}

/// 测试缓存行为：首次请求 miss，第二次请求 hit。
///
/// 使用唯一表达式避免与其他测试共享 `SHARED_CACHE`（进程级 OnceLock）
/// 导致的非确定性。spec.md R-sdforge-002 缓存语义：相同表达式第二次请求命中缓存。
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
