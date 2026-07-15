// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! HTTP server 实现：`POST /api/v1/evaluate` 端点。
//!
//! 基于 sdforge http 模块（axum + inventory 路由注册）。
//! spec.md R-sdforge-002 定义接口契约。
//!
//! # 端点
//!
//! `POST /api/v1/evaluate`
//! - Request: `{"expr":"2+3","vars":{"x":1.0},"precision":null}`
//! - Response 200: `{"result":5,"domain":"arithmetic","cache":"miss"}`
//! - Response 400: `{"error":{"kind":"Parse","message":"...","exit_code":1}}`

use super::types::{ErrorDetail, ErrorResponse, EvaluateRequest, EvaluateResponse};
use super::ServerError;
use crate::evaluate;
use crate::CacheManager;
use axum::extract::DefaultBodyLimit;
use axum::response::Response;
use axum::Router;
use sdforge::axum::extract::Json;
use sdforge::axum::http::status::StatusCode;
use sdforge::axum::routing::post;
use sdforge::axum::IntoResponse;
use sdforge::core::ApiMetadata;
use sdforge::http::{HttpRoute, RouteRegistration};
use std::future::Future;
use std::sync::OnceLock;

/// 请求体大小上限（64KB，T016 安全前置任务：防止超大请求体耗尽内存）。
const MAX_BODY_SIZE: usize = 64 * 1024;

/// 进程级共享缓存（OnceLock 懒初始化，跨请求共享）。
///
/// 使用全局 OnceLock 而非每请求新建 CacheManager，确保相同表达式的
/// 第二次请求能命中缓存（spec.md R-sdforge-002 缓存语义）。
static SHARED_CACHE: OnceLock<CacheManager> = OnceLock::new();

/// 获取共享 CacheManager 实例。
fn shared_cache() -> &'static CacheManager {
    SHARED_CACHE.get_or_init(CacheManager::new)
}

/// POST /api/v1/evaluate handler：接收 JSON 请求，调用 evaluate，返回 JSON 响应。
///
/// 处理流程：
/// 1. 安全校验（vars ≤1024 键，precision ≤10000）
/// 2. 转换为 EvalContext
/// 3. `spawn_blocking` 调用同步 `evaluate`（避免 oxcache async block_on 嵌套）
/// 4. 构造响应（200 OK / 400 Bad Request / 500 Internal Server Error）
async fn evaluate_handler(Json(req): Json<EvaluateRequest>) -> Response {
    // 1. 安全校验
    if let Err(e) = req.validate() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: ErrorDetail {
                    kind: "Validation".to_string(),
                    message: e.to_string(),
                    exit_code: 2,
                },
            }),
        )
            .into_response();
    }

    // 2. 准备 evaluate 参数
    let ctx = req.to_eval_context();
    let precision = req.precision;
    let expr = req.expr.clone();

    // 3. spawn_blocking：evaluate 是同步函数，内部 CacheManager 用 block_on
    //    调 oxcache async API。在 axum async handler 中直接调用会 panic
    //    （"Cannot start a runtime from within a runtime"）。
    //    spawn_blocking 将同步 evaluate 移到阻塞线程池执行，该线程无 runtime
    //    context，block_on 安全。shared_cache() 也必须在闭包内调用，因为
    //    CacheManager::new() 内部同样用 block_on 构建 oxcache Cache。
    //    解决性能 HIGH #2（CacheManager block_on 嵌套）的运行时风险。
    let task_result = tokio::task::spawn_blocking(move || {
        let cache: &'static CacheManager = shared_cache();
        evaluate(&expr, &ctx, precision, cache)
    })
    .await;

    // 4. 构造响应
    match task_result {
        // evaluate 成功
        Ok(Ok((eval_result, domain, cache_hit, fmt_prec))) => (
            StatusCode::OK,
            Json(EvaluateResponse::from_eval(
                eval_result,
                domain,
                cache_hit,
                fmt_prec,
            )),
        )
            .into_response(),
        // evaluate 返回计算错误（Parse/Eval/Overflow/DivisionByZero 等）
        Ok(Err(e)) => (StatusCode::BAD_REQUEST, Json(ErrorResponse::from(&e))).into_response(),
        // spawn_blocking panic（不应发生，但必须显性化处理，规则 12）
        Err(join_err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: ErrorDetail {
                    kind: "Internal".to_string(),
                    message: format!("evaluate task panicked: {}", join_err),
                    exit_code: 1,
                },
            }),
        )
            .into_response(),
    }
}

/// 创建 evaluate 路由的 HttpRoute 实例（inventory 注册用）。
fn evaluate_route_create() -> HttpRoute {
    HttpRoute::new(
        "/api/v1/evaluate".to_string(),
        post(evaluate_handler),
        evaluate_route_metadata(),
        None,
    )
}

/// 创建 evaluate 路由的 ApiMetadata（inventory 注册用）。
fn evaluate_route_metadata() -> ApiMetadata {
    ApiMetadata::new(
        "evaluate".to_string(),
        "v1".to_string(),
        "Evaluate math expression".to_string(),
        None,
        false,
    )
}

// 注册 HTTP 路由到 sdforge inventory。
// sdforge::http::build() 会自动收集此路由并构建 Router。
sdforge::inventory::submit!(RouteRegistration::new(
    "evaluate",
    "v1",
    evaluate_route_create,
    evaluate_route_metadata,
));

/// 防止链接器优化掉 CalNexus 的 HTTP inventory 注册。
///
/// inventory crate 的 static 项在某些链接器配置下可能被优化掉，
/// 导致 `sdforge::http::build()` 无法收集到本 crate 注册的路由。
/// 此函数通过引用计数强制链接器保留这些符号。
#[inline(never)]
pub(crate) fn preserve_http_inventory() {
    let _count = sdforge::inventory::iter::<RouteRegistration>().count();
    let _ = _count;
}

/// 构建 CalNexus HTTP Router：保留 inventory 注册 + body limit + `sdforge::http::build()`。
///
/// **必须使用此函数**而非直接调用 `sdforge::http::build()`，否则：
/// 1. 链接器可能优化掉 CalNexus 的 `inventory::submit!` 注册，导致路由 404
/// 2. 缺少 body size 限制，可能被超大请求体攻击
///
/// 内部先调用 `preserve_http_inventory()` 强制保留符号，再委托
/// `sdforge::http::build()` 从 inventory 收集所有 `RouteRegistration`
/// 构建 axum `Router`，最后叠加 `DefaultBodyLimit::max(MAX_BODY_SIZE)`。
pub fn build_router() -> Router {
    preserve_http_inventory();
    sdforge::http::build().layer(DefaultBodyLimit::max(MAX_BODY_SIZE))
}

/// HTTP server 配置。
#[derive(Debug, Clone)]
pub struct HttpServer {
    /// 监听地址（默认 `127.0.0.1:3000`，spec.md R-sdforge-002）。
    addr: String,
}

impl Default for HttpServer {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpServer {
    /// 创建默认配置的 HTTP server（监听 `127.0.0.1:3000`）。
    pub fn new() -> Self {
        Self {
            addr: "127.0.0.1:3000".to_string(),
        }
    }

    /// 自定义监听地址。
    pub fn with_addr(mut self, addr: impl Into<String>) -> Self {
        self.addr = addr.into();
        self
    }

    /// 获取监听地址。
    pub fn addr(&self) -> &str {
        &self.addr
    }

    /// 同步入口：创建 tokio runtime 阻塞运行 `start()`。
    /// 供 CLI `--serve-http` flag 调用。
    pub fn run(&self) -> Result<(), ServerError> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|e| ServerError::Http(format!("failed to create tokio runtime: {}", e)))?;
        runtime.block_on(self.start_inner())
    }

    /// 内部 async 启动逻辑：bind TcpListener + axum::serve。
    async fn start_inner(&self) -> Result<(), ServerError> {
        let listener = tokio::net::TcpListener::bind(&self.addr)
            .await
            .map_err(|e| ServerError::Http(format!("failed to bind {}: {}", self.addr, e)))?;
        let router = build_router();
        sdforge::axum::serve(listener, router)
            .await
            .map_err(|e| ServerError::Http(format!("server error: {}", e)))?;
        Ok(())
    }
}

impl super::ServerAdapter for HttpServer {
    fn start(&self) -> impl Future<Output = Result<(), ServerError>> + Send {
        self.start_inner()
    }
}
