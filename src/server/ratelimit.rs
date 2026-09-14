// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! HTTP 限流（`ratelimit` feature）：固定窗口 per-IP 限流器 + Clone 桥接层。
//!
//! 契约来自 sdforge（`security::RateLimiter` / `HttpRequestRateLimiter` trait +
//! `RateLimitLayer`/`RateLimitMiddleware` Tower 中间件，429/Retry-After 响应由
//! 中间件统一生成）；本模块只提供策略实现与挂载胶水：
//!
//! - [`FixedWindowLimiter`]：进程内固定窗口计数器（`Mutex<HashMap>`），
//!   标识提取顺序 `ConnectInfo` → `X-Real-IP` → `X-Forwarded-For` 首段 →
//!   `"unknown"` 共享桶（当前 `serve_with_graceful_shutdown` 不暴露
//!   ConnectInfo，本地监听场景 header 即对端自报，见 README 限流一节）。
//!   阈值经 `CALNEXUS_RATELIMIT_LIMIT`（默认 120）与
//!   `CALNEXUS_RATELIMIT_WINDOW_SECS`（默认 60）配置。
//! - [`shareable_rate_limit_layer`]：`RateLimitLayer` 的 Clone 桥接。
//!   axum `Router::layer` 要求 `Layer + Clone`，而 sdforge 0.5.0-rc.4 的
//!   `RateLimitLayer` 未派生 Clone（base 上游已修复，待发版后此桥接可整体
//!   移除并直用 `RateLimitLayer::new`）。
//!
//! 未来升级路径：sdforge `LimiteronAdapter`（limiteron Governor 令牌桶/多规则）
//! 实现同一 trait，但其构造为 async，而 `build_router` 为同步单一事实源；
//! 待 sdforge 提供同步构造或 build_router 异步化后可平滑替换。

use std::collections::HashMap;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use sdforge::security::{HttpRequestRateLimiter, RateLimitError, RateLimiter};
use sdforge::tower::Layer;

/// 默认窗口内最大请求数。
const DEFAULT_LIMIT: u64 = 120;
/// 默认窗口长度（秒）。
const DEFAULT_WINDOW_SECS: u64 = 60;

/// 固定窗口限流策略：窗口内前 `limit` 次放行，后续请求拒绝至窗口滚动。
///
/// 被拒绝的请求不消耗预算（计数不增长），与常见固定窗口语义一致。
pub(crate) struct FixedWindowLimiter {
    limit: u64,
    window: Duration,
    state: Mutex<HashMap<String, WindowEntry>>,
}

#[derive(Clone, Copy)]
struct WindowEntry {
    window_start: Instant,
    count: u64,
}

impl FixedWindowLimiter {
    /// 创建指定阈值/窗口的限流器（测试注入小阈值用）。
    pub(crate) fn with_config(limit: u64, window_secs: u64) -> Self {
        Self {
            limit: limit.max(1),
            window: Duration::from_secs(window_secs.max(1)),
            state: Mutex::new(HashMap::new()),
        }
    }

    /// 从环境变量构建（非法/缺省回退默认值，见模块文档）。
    pub(crate) fn from_env() -> Self {
        let limit = std::env::var("CALNEXUS_RATELIMIT_LIMIT")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|&v| v > 0)
            .unwrap_or(DEFAULT_LIMIT);
        let window_secs = std::env::var("CALNEXUS_RATELIMIT_WINDOW_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|&v| v > 0)
            .unwrap_or(DEFAULT_WINDOW_SECS);
        Self::with_config(limit, window_secs)
    }

    /// 固定窗口判定核心（check 的同步内核，便于单测）。
    fn admit(&self, identifier: &str, now: Instant) -> Result<(), RateLimitError> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let entry = state.entry(identifier.to_string()).or_insert(WindowEntry {
            window_start: now,
            count: 0,
        });
        if now.duration_since(entry.window_start) >= self.window {
            *entry = WindowEntry {
                window_start: now,
                count: 0,
            };
        }
        if entry.count >= self.limit {
            return Err(RateLimitError::Exceeded {
                limit: self.limit,
                window_seconds: self.window.as_secs(),
            });
        }
        entry.count += 1;
        Ok(())
    }
}

impl RateLimiter for FixedWindowLimiter {
    fn check<'a>(
        &'a self,
        identifier: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), RateLimitError>> + Send + 'a>> {
        Box::pin(async move { self.admit(identifier, Instant::now()) })
    }
}

/// 标识提取：`ConnectInfo<SocketAddr>`（真实对端，需 into_make_service_with_connect_info）
/// → `X-Real-IP` → `X-Forwarded-For` 首段 → `"unknown"` 共享桶。
fn extract_identifier(req: &axum::http::Request<axum::body::Body>) -> String {
    if let Some(info) = req
        .extensions()
        .get::<axum::extract::ConnectInfo<SocketAddr>>()
    {
        return info.0.ip().to_string();
    }
    for header in ["x-real-ip", "x-forwarded-for"] {
        if let Some(value) = req
            .headers()
            .get(header)
            .and_then(|v| v.to_str().ok())
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            // X-Forwarded-For 可能是链表，取第一段（最初客户端）
            let token = value.split(',').next().unwrap_or(value).trim();
            if !token.is_empty() {
                return token.to_string();
            }
        }
    }
    "unknown".to_string()
}

impl HttpRequestRateLimiter for FixedWindowLimiter {
    fn check_request<'a>(
        &'a self,
        req: &'a axum::http::Request<axum::body::Body>,
    ) -> Pin<Box<dyn Future<Output = Result<(), RateLimitError>> + Send + 'a>> {
        // 同步提取标识后再进入 async 块：&Request<Body> 非 Send，不能跨 .await
        let identifier = extract_identifier(req);
        Box::pin(async move { self.check(&identifier).await })
    }
}

/// `RateLimitLayer` 的 Clone 桥接层（sdforge 0.5.0-rc.4 上游补齐前的临时胶水，
/// 见模块文档；字段为 `Arc`，clone 语义与上游修复后的 derive 完全一致）。
#[derive(Clone)]
pub(crate) struct ShareableRateLimitLayer {
    limiter: Arc<dyn HttpRequestRateLimiter>,
}

impl<S> Layer<S> for ShareableRateLimitLayer {
    type Service = sdforge::security::RateLimitMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        sdforge::security::RateLimitLayer::new(self.limiter.clone()).layer(inner)
    }
}

/// 以共享限流器构建可直接挂载 `Router::layer` 的限流层。
pub(crate) fn shareable_rate_limit_layer(
    limiter: Arc<dyn HttpRequestRateLimiter>,
) -> ShareableRateLimitLayer {
    ShareableRateLimitLayer { limiter }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 阈值内放行、超阈值拒绝、拒绝不消耗预算。
    #[test]
    fn test_fixed_window_admit_and_reject() {
        let limiter = FixedWindowLimiter::with_config(2, 60);
        let now = Instant::now();
        assert!(limiter.admit("ip1", now).is_ok());
        assert!(limiter.admit("ip1", now).is_ok());
        let err = limiter.admit("ip1", now).unwrap_err();
        assert!(matches!(
            err,
            RateLimitError::Exceeded {
                limit: 2,
                window_seconds: 60
            }
        ));
        // 独立标识互不影响
        assert!(limiter.admit("ip2", now).is_ok());
    }

    /// 窗口滚动后计数重置、恢复放行。
    #[test]
    fn test_fixed_window_resets_after_window() {
        let limiter = FixedWindowLimiter::with_config(1, 60);
        let start = Instant::now();
        assert!(limiter.admit("ip1", start).is_ok());
        assert!(limiter.admit("ip1", start).is_err());
        let later = start + Duration::from_secs(61);
        assert!(limiter.admit("ip1", later).is_ok(), "窗口滚动后应重新放行");
    }

    /// 标识提取：header 优先级与 X-Forwarded-For 首段截取。
    #[test]
    fn test_extract_identifier_headers() {
        let build = |headers: &[(&str, &str)]| {
            let mut builder = axum::http::Request::builder().uri("/");
            for (k, v) in headers {
                builder = builder.header(*k, *v);
            }
            builder.body(axum::body::Body::empty()).unwrap()
        };
        let req = build(&[("x-real-ip", "203.0.113.7")]);
        assert_eq!(extract_identifier(&req), "203.0.113.7");
        let req = build(&[("x-forwarded-for", "198.51.100.1, 10.0.0.1")]);
        assert_eq!(extract_identifier(&req), "198.51.100.1");
        let req = build(&[]);
        assert_eq!(
            extract_identifier(&req),
            "unknown",
            "无任何来源时应共享 unknown 桶"
        );
    }
}
