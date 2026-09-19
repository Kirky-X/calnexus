// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! S8 Server E2E —— HTTP 端点矩阵、MCP 工具、docs/ratelimit feature 行为。
//!
//! 与既有 server_http/server_mcp 集成测试互补：本模块补齐
//! `list_functions`、fx 双端点/双工具（网络容忍）、`docs` feature 的
//! swagger 路由与 `ratelimit` feature 的 429 行为（此前零集成覆盖）。
//! HTTP 经 tower oneshot 直测 Router；ratelimit 经子进程隔离
//! （进程内改 CALNEXUS_* 会污染并行测试的共享路由器）。

#![cfg(feature = "server")]

use calnexus::build_router;
use http_body_util::BodyExt;
use sdforge::axum::Body;
use sdforge::axum::http::Request;
use sdforge::axum::http::status::StatusCode;
use serde_json::{Value, json};
use tower::ServiceExt;

use sdforge::axum::Router as SdRouter;

async fn send(router: &SdRouter, req: Request<Body>) -> (StatusCode, Value) {
    let response = router.clone().oneshot(req).await.expect("oneshot failed");
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

async fn post_json(uri: &str, body: Value) -> (StatusCode, Value) {
    let req = Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    send(&build_router(), req).await
}

async fn get(uri: &str) -> (StatusCode, Value) {
    let req = Request::builder()
        .method("GET")
        .uri(uri)
        .body(Body::empty())
        .unwrap();
    send(&build_router(), req).await
}

// ---------------------------------------------------------------------------
// HTTP 端点矩阵
// ---------------------------------------------------------------------------

#[tokio::test]
async fn http_evaluate_and_list_functions() {
    // evaluate 基线
    let (status, body) = post_json("/api/v1/evaluate", json!({"expr": "2+3*4"})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["result"], 14);
    assert_eq!(body["domain"], "arithmetic");

    // list_functions（既有套件未覆盖的端点）
    let (status, body) = post_json("/api/v1/list_functions", json!({})).await;
    assert_eq!(status, StatusCode::OK);
    let text = serde_json::to_string(&body).unwrap();
    assert!(
        text.contains("gcd") || text.contains("functions"),
        "got {text}"
    );
}

#[tokio::test]
async fn http_health_and_metrics_endpoints() {
    for uri in ["/health", "/ready"] {
        let (status, body) = get(uri).await;
        assert_eq!(status, StatusCode::OK, "{uri}");
        assert_eq!(body["status"], "ready", "{uri}");
    }
    let (status, body) = get("/live").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "healthy");
    // ready 携带 cache 检查与 entry_count
    let (_, body) = get("/ready").await;
    let text = serde_json::to_string(&body).unwrap();
    assert!(
        text.contains("cache") && text.contains("entry_count"),
        "got {text}"
    );

    // Prometheus 文本 + JSON 双格式
    let (status, body) = get("/metrics").await;
    assert_eq!(status, StatusCode::OK);
    let text = serde_json::to_string(&body).unwrap();
    assert!(!text.is_empty());
    let (status, body) = get("/metrics?format=json").await;
    assert_eq!(status, StatusCode::OK);
    // JSON 格式键名不带 calnexus_ 前缀（Prometheus 文本才带）
    assert!(
        body.get("hits").is_some() && body.get("entry_count").is_some(),
        "got {body}"
    );
}

#[tokio::test]
async fn http_error_contract_and_validation() {
    // 计算错误 → 400 InvalidInput，message 带 ErrorKind 前缀
    let (status, body) = post_json("/api/v1/evaluate", json!({"expr": "1/0"})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["type"], "InvalidInput");
    let msg = body["message"].as_str().unwrap_or("");
    assert!(
        msg.contains("DivisionByZero") || msg.contains("division"),
        "got {msg}"
    );

    // 参数校验 → 422 ValidationError
    let (status, body) =
        post_json("/api/v1/evaluate", json!({"expr": "1", "precision": 99999})).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["type"], "ValidationError");
    let (status, body) = post_json(
        "/api/v1/evaluate",
        json!({"expr": "1", "vars": {"a": 0.0, "b": 0.0}}),
    )
    .await;
    // 2 个变量合法（上限 1024），此请求应成功——对照契约
    assert_eq!(status, StatusCode::OK, "got {body}");
}

#[tokio::test]
async fn http_request_id_and_lang() {
    // X-Request-ID 透传
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/evaluate")
        .header("content-type", "application/json")
        .header("X-Request-ID", "e2e-req-42")
        .body(Body::from(
            serde_json::to_vec(&json!({"expr": "1+1"})).unwrap(),
        ))
        .unwrap();
    let router = build_router();
    let response = router.oneshot(req).await.unwrap();
    assert_eq!(
        response
            .headers()
            .get("x-request-id")
            .map(|v| v.to_str().unwrap()),
        Some("e2e-req-42")
    );
    // lang 字段协商 zh
    let (status, body) = post_json("/api/v1/evaluate", json!({"expr": "1/0", "lang": "zh"})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let msg = body["message"].as_str().unwrap_or("");
    assert!(
        msg.chars().any(|c| c >= '\u{4e00}'),
        "zh localized, got {msg}"
    );
}

#[cfg(feature = "fx")]
#[tokio::test]
async fn http_fx_endpoints_network_tolerant() {
    // fx 预算端点：有网/有缓存 → 200；断网 → 503（上游不可用）。两侧均绿。
    let (status, body) = post_json(
        "/api/v1/fx_budget",
        json!({
            "tuition": 11000,
            "tuition_currency": "USD",
            "duration_years": 4,
            "home_currency": "CNY"
        }),
    )
    .await;
    match status {
        StatusCode::OK => {
            assert!(body["tuition_home"].as_f64().unwrap() > 0.0);
            assert_eq!(body["home_currency"], "CNY");
        }
        StatusCode::SERVICE_UNAVAILABLE => assert_eq!(body["type"], "ServiceUnavailable"),
        other => panic!("tolerated 200/503, got {other}"),
    }
    // 校验错误（与服务商可用性无关，确定性）：tuition<=0 → 400
    let (status, _) = post_json(
        "/api/v1/fx_budget",
        json!({"tuition": -5, "tuition_currency": "USD", "duration_years": 4, "home_currency": "CNY"}),
    )
    .await;
    // 负值学费在 #[forge] 参数校验层拦截 → 422 ValidationError
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[cfg(feature = "docs")]
#[tokio::test]
async fn http_docs_feature_serves_swagger_ui() {
    // docs feature 挂载 /swagger-ui/（零覆盖面补齐）
    let (status, _) = get("/swagger-ui/").await;
    assert_eq!(status, StatusCode::OK);
}

// ---------------------------------------------------------------------------
// MCP 工具面
// ---------------------------------------------------------------------------

fn tool_text(result: sdforge::rmcp::model::CallToolResult) -> (bool, Value) {
    use sdforge::rmcp::model::ContentBlock;
    let is_err = result.is_error == Some(true);
    let text = result
        .content
        .first()
        .map(|c| match c {
            ContentBlock::Text(t) => t.text.to_string(),
            _ => String::new(),
        })
        .unwrap_or_default();
    (
        is_err,
        serde_json::from_str(&text).unwrap_or(Value::String(text)),
    )
}

#[test]
fn mcp_tool_catalog_and_evaluate() {
    let server = calnexus::build_mcp_server();
    // 基础工具 2 个；fx+server 组合追加 2 个（4）
    #[cfg(feature = "fx")]
    assert_eq!(server.tool_count(), 4);
    #[cfg(not(feature = "fx"))]
    assert_eq!(server.tool_count(), 2);

    let names: Vec<String> = server
        .get_all_tools()
        .iter()
        .map(|t| t.name.as_ref().to_string())
        .collect();
    assert!(names.contains(&"evaluate".to_string()));
    assert!(names.contains(&"list_functions".to_string()));

    // evaluate 成功
    let result = server
        .call_tool_internal("evaluate", Some(json!({"req": {"expr": "2+3"}})))
        .expect("call should not fail at transport level");
    let (is_err, body) = tool_text(result);
    assert!(!is_err);
    assert_eq!(body["result"], 5);

    // 解析错误 → is_error + InvalidInput JSON
    let result = server
        .call_tool_internal("evaluate", Some(json!({"req": {"expr": "(2+3"}})))
        .expect("call should not fail at transport level");
    let (is_err, body) = tool_text(result);
    assert!(is_err);
    assert_eq!(body["type"], "InvalidInput");
}

#[cfg(feature = "fx")]
#[test]
fn mcp_fx_tools_network_tolerant() {
    let server = calnexus::build_mcp_server();
    let result = server
        .call_tool_internal(
            "fx_budget",
            Some(json!({"req": {
                "tuition": 11000.0,
                "tuition_currency": "USD",
                "duration_years": 4,
                "home_currency": "CNY"
            }})),
        )
        .expect("call should not fail at transport level");
    let (is_err, body) = tool_text(result);
    match body {
        Value::Object(ref o) if o.get("tuition_home").is_some() => {
            assert!(!is_err);
            assert!(o["tuition_home"].as_f64().unwrap() > 0.0);
        }
        Value::Object(ref o) => {
            // 上游不可达：ApiError JSON，is_error 置位
            assert!(is_err, "fx_budget offline tolerated as error, got {body}");
            assert!(o.get("type").is_some(), "ApiError shape, got {body}");
        }
        other => panic!("unexpected fx_budget content: {other}"),
    }
}

// ---------------------------------------------------------------------------
// ratelimit feature —— 子进程隔离验证 429
// ---------------------------------------------------------------------------

#[cfg(all(feature = "cli", feature = "http", feature = "ratelimit"))]
#[test]
fn ratelimit_fixed_window_returns_429_via_subprocess() {
    use std::io::{BufRead, BufReader};
    use std::net::TcpStream;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    // 空闲端口
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);

    let mut child = Command::new(env!("CARGO_BIN_EXE_calnexus"))
        .args(["--serve-http", "--bind", &addr.to_string()])
        .env("CALNEXUS_RATELIMIT_LIMIT", "2")
        .env("CALNEXUS_RATELIMIT_WINDOW_SECS", "60")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn calnexus --serve-http");

    let result = (|| -> Result<(), String> {
        // 有界重试等待监听
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut stream = None;
        while Instant::now() < deadline {
            if let Ok(s) = TcpStream::connect(addr) {
                stream = Some(s);
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let stream = stream.ok_or("server did not listen in time")?;
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .map_err(|e| e.to_string())?;

        // 保持默认 keep-alive（Connection: close 会在首个响应后断管）
        let req = format!(
            "POST /api/v1/evaluate HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\nContent-Length: 14\r\n\r\n{{\"expr\":\"1+1\"}}"
        );
        let mut statuses = Vec::new();
        for _ in 0..4 {
            let mut s = stream.try_clone().map_err(|e| e.to_string())?;
            use std::io::Write;
            s.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
            let mut reader = BufReader::new(s);
            let mut line = String::new();
            reader.read_line(&mut line).map_err(|e| e.to_string())?;
            let status = line.split_whitespace().nth(1).unwrap_or("?").to_string();
            statuses.push(status);
        }
        // 前 2 个放行（200），第 3 个起 429（固定窗口 60s 内）
        assert_eq!(statuses[0], "200", "statuses: {statuses:?}");
        assert_eq!(statuses[1], "200", "statuses: {statuses:?}");
        assert_eq!(statuses[2], "429", "statuses: {statuses:?}");
        Ok(())
    })();

    let _ = child.kill();
    let _ = child.wait();
    result.unwrap_or_else(|e| panic!("ratelimit e2e: {e}"));
}
