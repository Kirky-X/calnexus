// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! HTTP server 启动：API 路由（显式注册）+ sdforge 健康探针 + /metrics + 优雅关闭。
//!
//! 路由注册策略（缓存重构期间发现的关键修复）：`#[forge]` 宏生成的 HTTP
//! inventory 注册在 rlib/测试二进制场景下会被链接器 GC 静默丢弃（`#[forge]`
//! 注解对象文件若无其他符号引用即整体剔除，404 且无任何告警）。因此 HTTP 路由
//! 在本模块**显式挂载**（单一事实源），复用与 `#[forge]` 注解函数相同的内部实现
//! 与 `ApiError` 契约，行为等价；`#[forge]` 保留 MCP tool 注册与 schema 推导职责。
//!
//! sdforge 能力吸收（基座迁移，替代手写实现）：
//! - 优雅关闭：`sdforge::http::serve_with_graceful_shutdown_connect_info` +
//!   `default_shutdown_signal`（`graceful` feature），替代手写信号 select 与
//!   drain 超时包装；SIGTERM 在 select 前注册（早期信号缓冲，消除竞态）。
//!   ConnectInfo 变体使每笔请求携带真实对端地址（限流按对端 IP 生效）。
//! - 健康探针：`sdforge::health::{healthz_handler, readyz_handler}`（`health`
//!   feature）挂载 `/live` 与 `/health`、`/ready`，缓存状态经
//!   `register_readiness_check_fn` 注册为 readiness check。
//! - 请求标识：`sdforge::context::context_middleware`（`context` feature），
//!   request_id 透传/生成 + W3C traceparent 提取 trace_id + task-local 上下文，
//!   替代手写 request_id 中间件。
//! - 限流（`ratelimit` feature）：`sdforge::security::RateLimitLayer` +
//!   `HttpRequestRateLimiter` 契约，仅作用于 API 业务路由（探针/metrics 豁免）；
//!   策略为 limiteron 同步固定窗口（`super::ratelimit`）。

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use super::EvaluateRequest;
use super::ServerError;
use super::evaluate::{REQUEST_TIMEOUT_SECS, evaluate_with_timeout};
use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::routing::{get, post};

#[cfg(all(feature = "fx", feature = "mcp"))]
use super::fx_tools::{FxBudgetRequest, FxPricingRequest, fx_budget, fx_pricing};
#[cfg(feature = "ratelimit")]
use sdforge::security::HttpRequestRateLimiter;
#[cfg(feature = "ratelimit")]
use std::sync::Arc;

/// 请求体大小上限（64KB，安全前置任务：防止超大请求体耗尽内存）。
const MAX_BODY_SIZE: usize = 64 * 1024;

const DRAIN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// 构建 CalNexus HTTP Router：显式 API 路由 + sdforge 探针 + body limit +
/// 请求计数 + 请求上下文。默认限流配置（ratelimit feature）。
///
/// 路由为显式注册（见模块文档：inventory 链接器 GC 脆弱性）。
pub fn build_router() -> Router {
    #[cfg(feature = "ratelimit")]
    {
        let limiter: Arc<dyn HttpRequestRateLimiter> =
            Arc::new(super::ratelimit::FixedWindowLimiter::from_env());
        finish_router(api_router().layer(sdforge::security::RateLimitLayer::new(limiter)))
    }
    #[cfg(not(feature = "ratelimit"))]
    {
        finish_router(api_router())
    }
}

/// 以指定限流器构建 Router（限流回归测试注入小阈值用）。
///
/// 与 [`build_router`] 共享 `api_router`/`finish_router`，保证测试路径与
/// 生产路径一致（单一事实源）。
#[cfg(all(test, feature = "ratelimit"))]
pub(crate) fn build_router_with_limiter(limiter: Arc<dyn HttpRequestRateLimiter>) -> Router {
    finish_router(api_router().layer(sdforge::security::RateLimitLayer::new(limiter)))
}

/// API 业务路由（限流作用域：evaluate / list_functions / fx 工具）。
#[cfg_attr(not(all(feature = "fx", feature = "mcp")), allow(unused_mut))] // fx+mcp 之外无需重新赋值
fn api_router() -> Router {
    let mut api = Router::new()
        .route("/api/v1/evaluate", post(evaluate_http_handler))
        .route("/api/v1/list_functions", post(list_functions_http_handler));

    // fx 场景工具路由（与 fx_tools 模块同门控：fx + mcp）
    #[cfg(all(feature = "fx", feature = "mcp"))]
    {
        api = api
            .route("/api/v1/fx_budget", post(fx_budget_http_handler))
            .route("/api/v1/fx_pricing", post(fx_pricing_http_handler));
    }
    api
}

/// 挂载探针/metrics/swagger 与全局中间件层（body limit → 请求计数 → 上下文）。
///
/// 层序（axum 后加者在外层先执行）：context 中间件最外层（为全链路安装
/// task-local 请求上下文），请求计数其次，body limit 最内。
fn finish_router(api: Router) -> Router {
    register_cache_readiness_check();

    // Swagger UI: /swagger-ui（docs feature）
    #[cfg(feature = "docs")]
    let mut router = Router::new().merge(sdforge::docs::swagger_ui_router());
    #[cfg(not(feature = "docs"))]
    let mut router = Router::new();

    router = router
        .merge(api)
        // 健康探针（sdforge health）：/live 存活（进程语义），/health 与 /ready
        // 就绪（含注册的 cache readiness check），探针豁免限流。
        .route("/health", get(sdforge::health::readyz_handler))
        .route("/ready", get(sdforge::health::readyz_handler))
        .route("/live", get(sdforge::health::healthz_handler))
        .route("/metrics", get(metrics_handler))
        .layer(DefaultBodyLimit::max(MAX_BODY_SIZE))
        // 请求计数 → /metrics
        .layer(axum::middleware::from_fn(request_count_middleware))
        // 请求标识传播（sdforge context）：X-Request-ID 透传/生成 +
        // W3C traceparent 提取 trace_id + 响应回写 X-Request-ID/X-Trace-ID
        .layer(axum::middleware::from_fn(
            sdforge::context::context_middleware,
        ));
    router
}

/// 注册 cache readiness check（进程级一次；/health 与 /ready 的 checks 数组消费）。
fn register_cache_readiness_check() {
    use std::sync::OnceLock;
    static REGISTER: OnceLock<()> = OnceLock::new();
    REGISTER.get_or_init(|| {
        sdforge::health::register_readiness_check_fn("cache", || {
            let stats = super::cache::shared_cache().stats();
            sdforge::health::CheckOutcome {
                name: "cache".to_string(),
                healthy: true,
                details: Some(serde_json::json!({ "entry_count": stats.entry_count })),
            }
        });
    });
}

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

async fn list_functions_http_handler(
    axum::extract::Json(_req): axum::extract::Json<super::catalog::ListFunctionsRequest>,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    match super::catalog::list_functions_inner().await {
        Ok(resp) => axum::Json(resp).into_response(),
        Err(api_err) => api_err.into_response(),
    }
}

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

/// 全局 HTTP 请求计数（/metrics 的 calnexus_http_requests_total）。
static HTTP_REQUESTS_TOTAL: AtomicU64 = AtomicU64::new(0);

/// 请求计数中间件：标识传播已委托 sdforge context
/// 中间件，本层仅维护 /metrics 的全局计数。
async fn request_count_middleware(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    HTTP_REQUESTS_TOTAL.fetch_add(1, Ordering::Relaxed);
    next.run(req).await
}

fn http_requests_total() -> u64 {
    HTTP_REQUESTS_TOTAL.load(Ordering::Relaxed)
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
///
/// 不用 sdforge `metrics` feature 的 `/metrics`：MetricsRegistry 只覆盖 HTTP
/// RED 指标（路由/方法/状态），不支持缓存 gauge 等自定义指标族。
async fn metrics_handler(query: axum::extract::Query<MetricsQuery>) -> axum::response::Response {
    use axum::response::IntoResponse;
    let stats = super::cache::shared_cache().stats();
    if query.format.as_deref() == Some("json") {
        let body = serde_json::json!({
            "hits": stats.hits,
            "misses": stats.misses,
            "entry_count": stats.entry_count,
            "http_requests_total": http_requests_total(),
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
             calnexus_cache_entries {}\n\
             # HELP calnexus_http_requests_total Total HTTP requests handled.\n\
             # TYPE calnexus_http_requests_total counter\n\
             calnexus_http_requests_total {}\n",
            stats.hits,
            stats.misses,
            stats.entry_count,
            http_requests_total()
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

/// HTTP server 配置。
#[derive(Debug, Clone)]
pub struct HttpServer {
    /// 监听地址（默认 `127.0.0.1:3000`）。
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

    /// 内部 async 启动逻辑：bind TcpListener + sdforge 优雅关闭序列。
    ///
    /// `serve_with_graceful_shutdown_connect_info`（sdforge `graceful` feature）
    /// 执行生产关闭序列：停止 accept → drain 最长 [`DRAIN_TIMEOUT`]（超时强退，
    /// k8s terminationGracePeriod 语义）→ stop hooks；`default_shutdown_signal`
    /// 在 select 前注册 SIGTERM handler（早期信号缓冲，无竞态窗口）。
    /// ConnectInfo 变体使请求携带真实对端地址（`super::ratelimit` 标识提取消费）。
    async fn start_inner(&self) -> Result<(), ServerError> {
        crate::server::init_observability();
        let listener = tokio::net::TcpListener::bind(&self.addr)
            .await
            .map_err(|e| ServerError::Http(format!("failed to bind {}: {}", self.addr, e)))?;
        let router = build_router();
        sdforge::http::serve_with_graceful_shutdown_connect_info(
            router,
            listener,
            sdforge::http::default_shutdown_signal(),
            sdforge::http::GracefulShutdownConfig::with_drain_timeout(DRAIN_TIMEOUT),
        )
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

    /// 编译期验证 sdforge 优雅关闭触发器 future 为 `Send`。
    /// axum `with_graceful_shutdown`（serve_with_graceful_shutdown 内部）要求
    /// signal future 为 `Send`；若上游违约此处编译失败。
    #[test]
    fn test_default_shutdown_signal_is_send() {
        fn assert_send<T: Send + ?Sized>(_: &T) {}
        let fut = sdforge::http::default_shutdown_signal();
        assert_send(&fut);
    }

    /// DRAIN_TIMEOUT 与 sdforge `GracefulShutdownConfig::default()` 语义对齐
    /// （30s，k8s terminationGracePeriod 常见基线）。
    #[test]
    fn test_drain_timeout_matches_sdforge_default() {
        assert_eq!(
            DRAIN_TIMEOUT,
            sdforge::http::GracefulShutdownConfig::default().drain_timeout
        );
    }

    /// 探针与就绪检查挂载回归：/live 存活形状、/health 与 /ready 含 cache 检查。
    #[cfg(feature = "http")]
    #[tokio::test]
    async fn test_sdforge_probe_shapes() {
        use axum::body::Body;
        use axum::http::Request;
        use http_body_util::BodyExt;
        use tower::ServiceExt;

        let router = build_router();
        for path in ["/health", "/ready"] {
            let resp = router
                .clone()
                .oneshot(
                    Request::builder()
                        .method("GET")
                        .uri(path)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(resp.status(), 200, "{path} 应就绪");
            let bytes = BodyExt::collect(resp.into_body()).await.unwrap().to_bytes();
            let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(json["status"], "ready", "{path} 状态应为 ready");
            let checks = json["checks"].as_array().unwrap();
            assert!(
                checks
                    .iter()
                    .any(|c| c["name"] == "cache" && c["healthy"] == true),
                "{path} 应含 cache readiness check: {json}"
            );
            assert!(
                checks[0]["details"]["entry_count"].is_u64(),
                "cache check 应携带 entry_count 明细"
            );
        }

        let resp = router
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/live")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let bytes = BodyExt::collect(resp.into_body()).await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["status"], "healthy");
        assert!(json.get("checks").is_none(), "/live 不应含检查器");
    }

    /// 请求计数与 sdforge context 标识回写回归：请求后 /metrics 计数增长，
    /// 响应携带 X-Request-ID（入站透传）与 X-Trace-ID（从 W3C traceparent 提取）。
    #[tokio::test]
    async fn test_context_middleware_and_request_count() {
        use axum::body::Body;
        use axum::http::Request;
        use tower::ServiceExt;

        let router = build_router();
        let resp = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/live")
                    .header("x-request-id", "my-trace-42")
                    .header(
                        "traceparent",
                        "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            resp.headers()
                .get("x-request-id")
                .and_then(|v| v.to_str().ok()),
            Some("my-trace-42"),
            "入站 X-Request-ID 应透传"
        );
        assert_eq!(
            resp.headers()
                .get("x-trace-id")
                .and_then(|v| v.to_str().ok()),
            Some("4bf92f3577b34da6a3ce929d0e0e4736"),
            "应从 W3C traceparent 提取 trace_id 回写 X-Trace-ID"
        );

        let before = http_requests_total();
        let _ = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/live")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(http_requests_total() > before, "请求计数应单调增长");
    }

    #[cfg(feature = "ratelimit")]
    mod ratelimit_mount {
        use super::super::{HttpRequestRateLimiter, build_router_with_limiter};
        use std::sync::Arc;
        use tower::ServiceExt;

        /// 限流挂载回归：小阈值限流器下第 3 个请求 429 + Retry-After。
        /// 验证限流层在 Router::layer 路径可用（sdforge RateLimitLayer 直挂）。
        #[tokio::test]
        async fn test_ratelimit_small_threshold_rejects_with_429() {
            use crate::server::ratelimit::FixedWindowLimiter;

            let limiter: Arc<dyn HttpRequestRateLimiter> =
                Arc::new(FixedWindowLimiter::with_config(2, 60));
            let router = build_router_with_limiter(limiter);

            let request = || {
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/api/v1/list_functions")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from("{}"))
                    .unwrap()
            };

            for i in 0..2 {
                let resp = router.clone().oneshot(request()).await.unwrap();
                assert_eq!(resp.status(), 200, "第 {} 个请求应在阈值内放行", i + 1);
            }

            let resp = router.oneshot(request()).await.unwrap();
            assert_eq!(resp.status(), 429, "超出阈值应返回 429");
            assert_eq!(
                resp.headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok()),
                Some("60"),
                "429 应携带 Retry-After（秒）"
            );
        }

        /// 探针豁免限流：即使 API 路由预算耗尽，探针与 /metrics 仍 200 可达。
        #[tokio::test]
        async fn test_probes_exempt_from_ratelimit() {
            use crate::server::ratelimit::FixedWindowLimiter;

            let limiter: Arc<dyn HttpRequestRateLimiter> =
                Arc::new(FixedWindowLimiter::with_config(1, 60));
            let router = build_router_with_limiter(limiter);

            let api_request = || {
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/api/v1/list_functions")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from("{}"))
                    .unwrap()
            };
            let _ = router.clone().oneshot(api_request()).await.unwrap();

            for path in ["/live", "/metrics", "/health", "/ready"] {
                let resp = router
                    .clone()
                    .oneshot(
                        axum::http::Request::builder()
                            .method("GET")
                            .uri(path)
                            .body(axum::body::Body::empty())
                            .unwrap(),
                    )
                    .await
                    .unwrap();
                assert_eq!(resp.status(), 200, "探针/metrics 应豁免限流: {path}");
            }
        }
    }
}
