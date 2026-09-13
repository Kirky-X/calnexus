// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! HTTP server 启动：API 路由（显式注册）+ 健康探针 + /metrics + HttpServer。
//!
//! spec.md R-sdforge-002 定义接口契约。
//!
//! 路由注册策略（v015 缓存重构期间发现的关键修复）：`#[forge]` 宏生成的 HTTP
//! inventory 注册在 rlib/测试二进制场景下会被链接器 GC 静默丢弃（`#[forge]`
//! 注解对象文件若无其他符号引用即整体剔除，404 且无任何告警）。因此 HTTP 路由
//! 在本模块**显式挂载**（单一事实源），复用与 `#[forge]` 注解函数相同的内部实现
//! 与 `ApiError` 契约，行为等价；`#[forge]` 保留 MCP tool 注册与 schema 推导职责。

use std::time::Duration;

use super::evaluate::{evaluate_with_timeout, REQUEST_TIMEOUT_SECS};
use super::ServerError;
use super::EvaluateRequest;
use axum::extract::DefaultBodyLimit;
use axum::routing::{get, post};
use axum::Router;

#[cfg(all(feature = "fx", feature = "mcp"))]
use super::fx_tools::{fx_budget, fx_pricing, FxBudgetRequest, FxPricingRequest};

/// 请求体大小上限（64KB，T016 安全前置任务：防止超大请求体耗尽内存）。
const MAX_BODY_SIZE: usize = 64 * 1024;

/// 构建 CalNexus HTTP Router：显式 API 路由 + 探针端点 + body limit。
///
/// 路由为显式注册（见模块文档：inventory 链接器 GC 脆弱性）。
/// `DefaultBodyLimit` 防止超大请求体攻击（保留 p1 安全约束）。
pub fn build_router() -> Router {
    let mut router = Router::new()
        .route("/api/v1/evaluate", post(evaluate_http_handler))
        .route("/health", get(health_handler))
        .route("/ready", get(readiness_handler))
        .route("/live", get(liveness_handler))
        .route("/metrics", get(metrics_handler))
        .layer(DefaultBodyLimit::max(MAX_BODY_SIZE));

    // fx 场景工具路由（与 fx_tools 模块同门控：fx + mcp）
    #[cfg(all(feature = "fx", feature = "mcp"))]
    {
        router = router
            .route("/api/v1/fx_budget", post(fx_budget_http_handler))
            .route("/api/v1/fx_pricing", post(fx_pricing_http_handler));
    }

    // Swagger UI: /swagger-ui（docs feature）
    #[cfg(feature = "docs")]
    {
        router = router.merge(sdforge::docs::swagger_ui_router());
    }

    // Rate limit middleware（ratelimit feature）
    // TODO: RateLimitLayer 未实现 Clone，无法直接用于 Router::layer()。
    // 待 sdforge 上游修复后启用。
    #[cfg(feature = "ratelimit")]
    {
        // 预留：RateLimitLayer 需要实现 Clone 才能挂载到 axum Router
    }

    router
}

/// `POST /api/v1/evaluate`：Json 提取 → `evaluate_with_timeout` → ApiError 契约。
///
/// 与 `#[forge]` 宏生成的 HTTP 外壳行为等价（Json 提取 + `.0` 解包 +
/// `ApiError::into_response` 错误路径）。
async fn evaluate_http_handler(
    axum::extract::Json(req): axum::extract::Json<EvaluateRequest>,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    match evaluate_with_timeout(req, Duration::from_secs(REQUEST_TIMEOUT_SECS)).await {
        Ok(resp) => axum::Json(resp).into_response(),
        Err(api_err) => api_err.into_response(),
    }
}

/// `POST /api/v1/fx_budget`：Json 提取 → `fx_budget`（显式挂载，同上）。
#[cfg(all(feature = "fx", feature = "mcp"))]
async fn fx_budget_http_handler(
    axum::extract::Json(req): axum::extract::Json<FxBudgetRequest>,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    match fx_budget(req).await {
        Ok(resp) => axum::Json(resp).into_response(),
        Err(api_err) => api_err.into_response(),
    }
}

/// `POST /api/v1/fx_pricing`：Json 提取 → `fx_pricing`（显式挂载，同上）。
#[cfg(all(feature = "fx", feature = "mcp"))]
async fn fx_pricing_http_handler(
    axum::extract::Json(req): axum::extract::Json<FxPricingRequest>,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    match fx_pricing(req).await {
        Ok(resp) => axum::Json(resp).into_response(),
        Err(api_err) => api_err.into_response(),
    }
}

/// GET /health：健康检查（含检查器明细与真实缓存统计）。
async fn health_handler() -> axum::Json<serde_json::Value> {
    use serde_json::json;
    let stats = super::cache::shared_cache().stats();
    axum::Json(json!({
        "status": "healthy",
        "checks": {
            "cache": {
                "status": "healthy",
                "details": "L1 in-memory cache operational",
                "entry_count": stats.entry_count,
            }
        },
    }))
}

/// GET /ready：就绪探针（依赖就绪语义：进程内缓存可读）。
async fn readiness_handler() -> axum::Json<serde_json::Value> {
    use serde_json::json;
    let stats = super::cache::shared_cache().stats();
    axum::Json(json!({
        "status": "healthy",
        "checks": {
            "cache": {
                "status": "healthy",
                "entry_count": stats.entry_count,
            }
        },
    }))
}

/// GET /live：存活探针（纯进程语义，无检查器明细）。
async fn liveness_handler() -> axum::Json<serde_json::Value> {
    use serde_json::json;
    axum::Json(json!({
        "status": "healthy",
        "checks": {},
    }))
}

/// Metrics 查询参数。
#[derive(serde::Deserialize)]
struct MetricsQuery {
    /// 输出格式：`json` 返回 JSON，其他值返回 Prometheus 文本格式。
    format: Option<String>,
}

/// GET /metrics：返回缓存统计数据（CacheManager::stats）。
///
/// - 默认返回 Prometheus 文本格式（`Content-Type: text/plain; version=0.0.4`，
///   含 `# HELP`/`# TYPE` 注释行，指标族 `calnexus_cache_*`）
/// - `?format=json` 返回 JSON 格式（调试友好）
async fn metrics_handler(
    query: axum::extract::Query<MetricsQuery>,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    let stats = super::cache::shared_cache().stats();
    if query.format.as_deref() == Some("json") {
        let body = serde_json::json!({
            "hits": stats.hits,
            "misses": stats.misses,
            "entry_count": stats.entry_count,
        });
        (
            [(axum::http::header::CONTENT_TYPE, "application/json")],
            body.to_string(),
        )
            .into_response()
    } else {
        let prometheus = format!(
            "# HELP calnexus_cache_hits Cache hit count.\n\
             # TYPE calnexus_cache_hits counter\n\
             calnexus_cache_hits {}\n\
             # HELP calnexus_cache_misses Cache miss count.\n\
             # TYPE calnexus_cache_misses counter\n\
             calnexus_cache_misses {}\n\
             # HELP calnexus_cache_entries Current number of cached entries.\n\
             # TYPE calnexus_cache_entries gauge\n\
             calnexus_cache_entries {}\n",
            stats.hits, stats.misses, stats.entry_count
        );
        (
            [(
                axum::http::header::CONTENT_TYPE,
                "text/plain; version=0.0.4",
            )],
            prometheus,
        )
            .into_response()
    }
}

/// 优雅关闭信号：监听 Ctrl+C（SIGINT），返回时 axum 停止接受新连接、等待已有连接完成。
///
/// 此前 `axum::serve` 无 `with_graceful_shutdown`，收到 SIGINT 时
/// 立即终止所有连接，可能导致正在执行的 evaluate 请求被中断、响应丢失。
/// 现通过 `with_graceful_shutdown(shutdown_signal())` 让 server 在收到 Ctrl+C 后
/// 进入 drain 阶段（停止 accept、等待 in-flight 请求完成）。
///
/// 单元测试通过 `assert_shutdown_signal_is_send` 验证 future 为 `Send`（axum 要求）。
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("install ctrl_c signal handler failed");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler failed")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
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

    /// 内部 async 启动逻辑：bind TcpListener + axum::serve（含 graceful shutdown）。
    ///
    /// `with_graceful_shutdown(shutdown_signal())` 让 server 收到
    /// Ctrl+C / SIGTERM 后进入 drain 阶段，等待 in-flight 请求完成再退出。
    async fn start_inner(&self) -> Result<(), ServerError> {
        let listener = tokio::net::TcpListener::bind(&self.addr)
            .await
            .map_err(|e| ServerError::Http(format!("failed to bind {}: {}", self.addr, e)))?;
        let router = build_router();
        sdforge::axum::serve(listener, router)
            .with_graceful_shutdown(shutdown_signal())
            .await
            .map_err(|e| ServerError::Http(format!("server error: {}", e)))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_server_new_default_addr() {
        let server = HttpServer::new();
        assert_eq!(server.addr(), "127.0.0.1:3000");
    }

    #[test]
    fn test_http_server_default_equals_new() {
        let server = HttpServer::default();
        assert_eq!(server.addr(), "127.0.0.1:3000");
    }

    #[test]
    fn test_http_server_with_addr_custom() {
        let server = HttpServer::new().with_addr("0.0.0.0:8080");
        assert_eq!(server.addr(), "0.0.0.0:8080");
    }

    #[test]
    fn test_http_server_run_bind_error() {
        // Port 99999 > 65535, bind will fail immediately
        let server = HttpServer::new().with_addr("127.0.0.1:99999");
        let result = server.run();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ServerError::Http(_)));
        assert!(err.to_string().contains("failed to bind"));
    }

    #[tokio::test]
    async fn test_http_start_inner_bind_error() {
        let server = HttpServer::new().with_addr("127.0.0.1:99999");
        let result = server.start_inner().await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ServerError::Http(_)));
    }

    /// 验证 `shutdown_signal` 返回的 future 为 `Send`。
    /// axum `with_graceful_shutdown` 要求 signal future 为 `Send`，编译期检查。
    /// 此测试在编译期捕获 Send 约束违规（若 future 非 Send，编译失败）。
    #[test]
    fn test_shutdown_signal_is_send() {
        fn assert_send<T: Send>(_t: T) {}
        let fut = shutdown_signal();
        assert_send(fut);
    }

    /// 验证 `shutdown_signal` 函数存在且可调用（构造即不 panic）。
    /// 不实际 await（await 需要真实信号，单元测试环境难以注入）。
    #[test]
    fn test_shutdown_signal_callable_without_panic() {
        let _fut = shutdown_signal();
        // 不 await，仅验证构造无 panic（信号 handler 安装在 await 阶段）
    }
}
