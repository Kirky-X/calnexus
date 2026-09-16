// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! HTTP 限流（`ratelimit` feature）：limiteron 同步固定窗口 + sdforge 契约适配。
//!
//! 策略与标识提取均复用自研库，本模块只保留配置解析与 trait 桥接：
//!
//! - 策略：[`limiteron::sync::SyncFixedWindowLimiter`]（per-identifier 固定
//!   窗口，拒绝不消耗预算，时间注入可测）；
//! - 标识提取：`sdforge::security::extract_client_ip`（信任代理 + 防伪造：
//!   `ConnectInfo` 直连地址优先，仅可信反向代理场景信任 `X-Forwarded-For` /
//!   `X-Real-IP`，否则回退 `"unknown"` 共享桶）；
//! - 阈值经 `CALNEXUS_RATELIMIT_LIMIT`（默认 120）与
//!   `CALNEXUS_RATELIMIT_WINDOW_SECS`（默认 60）配置。
//!
//! server 经 `sdforge::http::serve_with_graceful_shutdown_connect_info` 启动，
//! 请求携带 `ConnectInfo`，限流按真实对端 IP 生效（此前手写提取器无条件信任
//! header，本地监听场景 header 即对端自报）。
//!
//! `Router::layer` 要求 `Layer + Clone`，直用 `RateLimitLayer::new`
//! （依赖上游已派生 Clone）。

use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use limiteron::sync::{RateLimitRejection, SyncFixedWindowLimiter};
use sdforge::security::{HttpRequestRateLimiter, RateLimitError, RateLimiter};

const DEFAULT_LIMIT: u64 = 120;
const DEFAULT_WINDOW_SECS: u64 = 60;

/// 固定窗口限流器：limiteron 同步策略 + sdforge 契约的薄适配层。
pub(crate) struct FixedWindowLimiter {
    inner: SyncFixedWindowLimiter,
}

impl FixedWindowLimiter {
    /// 创建指定阈值/窗口的限流器（测试注入小阈值用）。
    pub(crate) fn with_config(limit: u64, window_secs: u64) -> Self {
        Self {
            inner: SyncFixedWindowLimiter::new(limit, Duration::from_secs(window_secs.max(1))),
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
}

/// limiteron 拒绝详情 → sdforge 限流错误（携带 limit / Retry-After 数据）。
fn map_rejection(result: Result<(), RateLimitRejection>) -> Result<(), RateLimitError> {
    result.map_err(|e| RateLimitError::Exceeded {
        limit: e.limit,
        window_seconds: e.window.as_secs(),
    })
}

impl RateLimiter for FixedWindowLimiter {
    fn check<'a>(
        &'a self,
        identifier: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), RateLimitError>> + Send + 'a>> {
        Box::pin(async move { map_rejection(self.inner.check(identifier)) })
    }
}

impl HttpRequestRateLimiter for FixedWindowLimiter {
    fn check_request<'a>(
        &'a self,
        req: &'a axum::http::Request<axum::body::Body>,
    ) -> Pin<Box<dyn Future<Output = Result<(), RateLimitError>> + Send + 'a>> {
        // 同步提取标识后再进入 async 块：&Request<Body> 非 Send，不能跨 .await
        let identifier =
            sdforge::security::extract_client_ip(req).unwrap_or_else(|| "unknown".to_string());
        Box::pin(async move { self.check(&identifier).await })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 阈值内放行、超阈值拒绝并携带窗口配置、独立标识互不影响
    ///（sdforge `RateLimiter` trait 为 async 契约，经 current-thread runtime 驱动）。
    #[test]
    fn test_check_admits_rejects_and_maps_error() {
        let limiter = FixedWindowLimiter::with_config(2, 60);
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        rt.block_on(async {
            assert!(limiter.check("ip1").await.is_ok());
            assert!(limiter.check("ip1").await.is_ok());
            let err = limiter.check("ip1").await.unwrap_err();
            assert!(matches!(
                err,
                RateLimitError::Exceeded {
                    limit: 2,
                    window_seconds: 60
                }
            ));
            assert!(limiter.check("ip2").await.is_ok());
        });
    }
}
