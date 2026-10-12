// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! HTTP server 启动：显式注册 API 路由 + sdforge 探针/metrics + 优雅关闭（含限流）。
//!
//! 路由必须显式挂载：`#[forge]` HTTP inventory 在 rlib/测试二进制下会被链接器 GC 丢弃。

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

/// 请求体大小上限 64KB（防止超大请求体耗尽内存）。
const MAX_BODY_SIZE: usize = 64 * 1024;

const DRAIN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// 构建 Router：API 路由 + 探针 + body limit + 请求计数/上下文（ratelimit feature 下含限流）。
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

/// 挂载探针/metrics/swagger 与中间件层；层序（后加者在外）：context → 请求计数 → body limit。
fn finish_router(api: Router) -> Router {
    register_cache_readiness_check();

    // Swagger UI: /swagger-ui（docs feature）
    #[cfg(feature = "docs")]
    let mut router = Router::new().merge(sdforge::docs::swagger_ui_router());
    #[cfg(not(feature = "docs"))]
    let mut router = Router::new();

    router = router
        .merge(api)
        // 健康探针：/live 存活，/health /ready 就绪（含 cache 检查），豁免限流
        .route("/health", get(sdforge::health::readyz_handler))
        .route("/ready", get(sdforge::health::readyz_handler))
        .route("/live", get(sdforge::health::healthz_handler))
        .route("/metrics", get(metrics_handler))
        .layer(DefaultBodyLimit::max(MAX_BODY_SIZE))
        // 请求计数 → /metrics
        .layer(axum::middleware::from_fn(request_count_middleware))
        // 请求标识：X-Request-ID 透传/生成 + W3C traceparent 提取 trace_id，均回写响应头
        .layer(axum::middleware::from_fn(
            sdforge::context::context_middleware,
        ));
    router
}

/// cache 就绪检查超时上限：探针须在 SLO 内返回，超时判不健康。
const READINESS_TIMEOUT: Duration = Duration::from_secs(5);

/// 注册 cache readiness check（进程级一次；异步探针 + 超时，moka 维护不占运行时工作线程）。
fn register_cache_readiness_check() {
    use std::sync::OnceLock;
    static REGISTER: OnceLock<()> = OnceLock::new();
    REGISTER.get_or_init(|| {
        sdforge::health::register_async_readiness_check_fn("cache", || async {
            run_readiness_with_timeout("cache", READINESS_TIMEOUT, cache_stats_outcome()).await
        });
    });
}

/// cache 就绪检查体：spawn_blocking 隔离 moka 维护（同步阻塞调用），任务失败判不健康。
async fn cache_stats_outcome() -> sdforge::health::CheckOutcome {
    match tokio::task::spawn_blocking(|| super::cache::shared_cache().stats()).await {
        Ok(stats) => sdforge::health::CheckOutcome {
            name: "cache".to_string(),
            healthy: true,
            details: Some(serde_json::json!({ "entry_count": stats.entry_count })),
        },
        Err(_) => sdforge::health::CheckOutcome::unhealthy("cache", "cache stats task failed"),
    }
}

/// 限时执行就绪检查：超时判不健康并记录原因（不无限等待依赖）。
async fn run_readiness_with_timeout(
    name: &str,
    timeout: Duration,
    check: impl std::future::Future<Output = sdforge::health::CheckOutcome>,
) -> sdforge::health::CheckOutcome {
    match tokio::time::timeout(timeout, check).await {
        Ok(outcome) => outcome,
        Err(_) => sdforge::health::CheckOutcome::unhealthy(
            name,
            format!("readiness check timed out after {timeout:?}"),
        ),
    }
}

/// 与 `#[forge]` 生成的 HTTP 外壳等价：Json 提取 + `ApiError::into_response` 错误路径。
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

/// 请求计数中间件：按 (route,method,status) 记录 RED 指标（sdforge metrics）。
async fn request_count_middleware(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let started = std::time::Instant::now();
    let method = req.method().as_str().to_string();
    let route = req.uri().path().to_string();
    let resp = next.run(req).await;
    sdforge::metrics::record_request(
        &route,
        &method,
        resp.status().as_u16(),
        started.elapsed().as_secs_f64(),
    );
    resp
}

/// Metrics 查询参数。
#[derive(serde::Deserialize)]
struct MetricsQuery {
    /// 输出格式：`json` 返回 JSON，其他值返回 Prometheus 文本格式。
    format: Option<String>,
}

/// GET /metrics：缓存统计 + HTTP RED 指标；默认 Prometheus 文本，`?format=json` 返回 JSON。
async fn metrics_handler(query: axum::extract::Query<MetricsQuery>) -> axum::response::Response {
    use axum::response::IntoResponse;
    let stats = super::cache::shared_cache().stats();
    if query.format.as_deref() == Some("json") {
        let body = serde_json::json!({
            "hits": stats.hits,
            "misses": stats.misses,
            "entry_count": stats.entry_count,
            "http_route_count": sdforge::metrics::global_registry().route_count(),
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
             {}",
            stats.hits,
            stats.misses,
            stats.entry_count,
            sdforge::metrics::global_registry().render()
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

    /// 同步入口：创建 tokio runtime 阻塞运行 `start_inner()`（供 CLI `--serve-http` 调用）。
    pub fn run(&self) -> Result<(), ServerError> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|e| ServerError::Http(format!("failed to create tokio runtime: {}", e)))?;
        let result = runtime.block_on(self.start_inner());
        // inklog sink worker 寄生本 runtime 且库无 shutdown API，runtime 析构会死等；跳过析构
        std::mem::forget(runtime);
        result
    }

    /// 内部 async 启动：bind TcpListener + sdforge 优雅关闭（drain 上限 [`DRAIN_TIMEOUT`]）。
    async fn start_inner(&self) -> Result<(), ServerError> {
        // bind 先于日志初始化：bind 失败不触碰日志单例（避免 worker 寄生致退出死等）
        let listener = tokio::net::TcpListener::bind(&self.addr)
            .await
            .map_err(|e| ServerError::Http(format!("failed to bind {}: {}", self.addr, e)))?;
        // 日志初始化：ConfigError 视为已初始化幂等成功，其余错误上抛不静默降级
        #[cfg(feature = "observability")]
        match crate::server::init_observability().await {
            Ok(()) => {}
            Err(inklog::InklogError::ConfigError(_)) => {}
            Err(e) => {
                return Err(ServerError::Http(format!(
                    "observability init failed: {}",
                    e
                )));
            }
        }
        #[cfg(not(feature = "observability"))]
        crate::server::init_observability().await;
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

    /// 编译期验证 shutdown signal future 为 `Send`（axum `with_graceful_shutdown` 要求）。
    #[test]
    fn test_default_shutdown_signal_is_send() {
        fn assert_send<T: Send + ?Sized>(_: &T) {}
        let fut = sdforge::http::default_shutdown_signal();
        assert_send(&fut);
    }

    /// DRAIN_TIMEOUT 与 sdforge GracefulShutdownConfig 默认值（30s）对齐。
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

    /// 计数与标识回写回归：请求后计数增长，响应含 X-Request-ID 与 X-Trace-ID。
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

        // 请求后 sdforge registry 应记录 /live 路由序列（RED 计数）
        let registry = sdforge::metrics::global_registry();
        registry.reset();
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
        assert!(
            registry.route_count() >= 1,
            "请求后 sdforge registry 应记录路由序列（RED 计数取代手写 static）"
        );
    }

    /// 就绪检查超时兜底：限时内未完成判不健康（探针不随依赖阻塞无限等待）。
    #[tokio::test]
    async fn test_readiness_timeout_marks_unhealthy() {
        let outcome =
            run_readiness_with_timeout("cache", Duration::from_millis(10), std::future::pending())
                .await;
        assert!(!outcome.healthy, "超时应判不健康");
        assert_eq!(outcome.name, "cache");
        assert!(
            outcome
                .details
                .as_ref()
                .and_then(|d| d["error"].as_str())
                .is_some_and(|e| e.contains("timed out")),
            "超时原因应写入 details.error: {:?}",
            outcome.details
        );
    }

    /// 异步 cache 就绪检查：spawn_blocking 隔离 moka 维护后仍反映 entry_count。
    #[tokio::test]
    async fn test_cache_stats_outcome_reports_entry_count() {
        let cf = crate::core::CanonicalForm::new("(readiness-async 1)");
        super::super::cache::shared_cache().insert(&cf, &Ok(crate::core::EvalResult::Scalar(1.0)));

        let outcome = cache_stats_outcome().await;
        assert!(outcome.healthy, "cache 检查应健康");
        assert_eq!(outcome.name, "cache");
        assert!(
            outcome
                .details
                .as_ref()
                .and_then(|d| d["entry_count"].as_u64())
                .is_some_and(|n| n >= 1),
            "插入条目后 entry_count 应 ≥1: {:?}",
            outcome.details
        );
    }

    #[cfg(feature = "ratelimit")]
    mod ratelimit_mount {
        use super::super::{HttpRequestRateLimiter, build_router_with_limiter};
        use std::sync::Arc;
        use tower::ServiceExt;

        /// 限流挂载回归：阈值 2 时第 3 个请求 429 + Retry-After。
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
