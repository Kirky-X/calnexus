// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! CalNexus Server 接口层：HTTP/MCP 多协议服务封装。
//!
//! 基于 sdforge 0.5 框架，将 evaluate 函数暴露为 HTTP API 和 MCP tool。
//!
//! # Feature Gate
//!
//! - `http` feature：启用 HTTP server（`POST /api/v1/evaluate`）
//! - `mcp` feature：启用 MCP server（`evaluate` tool，stdio 传输）
//! - `server` feature：HTTP + MCP 聚合

mod cache;
mod catalog;
mod evaluate;
mod lang;
mod types;

#[cfg(all(feature = "fx", feature = "mcp"))]
mod fx_tools;
#[cfg(feature = "http")]
mod http;
#[cfg(feature = "mcp")]
mod mcp;
#[cfg(feature = "ratelimit")]
mod ratelimit;

pub(crate) use cache::shared_cache;

/// 初始化可观测日志：observability feature 下由 inklog 单例承载
/// （消费 `RUST_LOG`，缺省 warn；同时捕获 sdforge/tower 等依赖的 tracing 事件）。
///
/// async：`init_inklog_logger_with_config` 构建为异步；生产调用点
/// （`HttpServer::start_inner`）本身在 async 上下文中直接 `.await`，
/// 同步测试点经临时 runtime `block_on`。Once/专用线程/常驻 runtime 全部
/// 退役——inklog 内部 mem::forget(manager) 持有 sink worker，worker 线程
/// 寄生调用方 runtime：服务路径寄生于 `HttpServer::run` 的常驻 runtime
/// （进程存活期一致），测试路径随测例 runtime 退出（单例哨兵使后续
/// init 显式报 ConfigError，测试进程内可预期）。
///
/// 单例语义（失败显性化）：首次调用返回 `Ok(())`；重复调用返回
/// `Err(InklogError::ConfigError)`（哨兵已占），调用方显式识别为
/// "已初始化" 幂等成功；其余错误（配置非法等）原样上抛由调用方处置。
#[cfg(feature = "observability")]
pub(crate) async fn init_observability() -> Result<(), inklog::InklogError> {
    let mut config = inklog::InklogConfig::default();
    // RUST_LOG 未设时全局级别 warn（缺省语义）；inklog EnvFilter 原生消费
    // RUST_LOG 且优先级高于 global 默认，与旧 LoggerManager builder 路径一致
    config.global.level = std::env::var("RUST_LOG").unwrap_or_else(|_| "warn".to_string());
    config.global.format = "{timestamp} [{level}] {target} - {message}".to_string();
    config.console_sink = Some(inklog::ConsoleSinkConfig {
        enabled: true,
        colored: true,
        ..inklog::ConsoleSinkConfig::default()
    });
    inklog::init_inklog_logger_with_config(config).await
}

#[cfg(not(feature = "observability"))]
pub(crate) async fn init_observability() {}
// 消费者 run_server_mode 需同时具备 cli 与 server；门控与其一致
#[cfg(all(feature = "cli", feature = "server"))]
pub(crate) use cache::init_shared_cache;
pub use catalog::{ListFunctionsRequest, ListFunctionsResponse};
pub use evaluate::{calc_error_to_api_error, calc_error_to_api_error_i18n};
pub use types::{EvaluateRequest, EvaluateResponse, ServerError};

#[cfg(feature = "http")]
pub use http::{HttpServer, build_router};
#[cfg(feature = "mcp")]
pub use mcp::{McpServer, build_mcp_server};

#[cfg(all(test, feature = "observability"))]
mod observability_tests {
    /// init_observability 单例语义：首调用 Ok，二次调用返回
    /// `Err(InklogError::ConfigError)`（inklog 单例哨兵），调用方显式识别为
    /// "已初始化" 而非 panic 或吞错。
    ///
    /// 结构说明：inklog init 构建的 LoggerManager 在 runtime 析构时会挂起
    /// （sink worker 通道握手等退出协同，实测 >5min 不返回），因此本测试在
    /// 独立线程内自建 multi_thread runtime（与生产 `HttpServer::run` 同
    /// 风味，http.rs:296），两次调用均带 10s 超时护栏，结果经 channel
    /// 回传后 `mem::forget(runtime)` 跳过析构——进程退出由测试二进制统一
    /// 回收。断言的是真实单例契约，不是"不报错"。
    #[test]
    fn test_init_observability_singleton_semantics() {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .expect("observability test runtime");
            let call = || async {
                tokio::time::timeout(
                    std::time::Duration::from_secs(10),
                    crate::server::init_observability(),
                )
                .await
            };
            let first = rt.block_on(call());
            let second = if first.as_ref().is_ok_and(Result::is_ok) {
                Some(rt.block_on(call()))
            } else {
                None
            };
            let _ = tx.send((first, second));
            std::mem::forget(rt);
        });
        let (first, second) = rx
            .recv_timeout(std::time::Duration::from_secs(30))
            .expect("init 结果回传超时（30s）——inklog init 不应无限阻塞");
        let first = first.expect("首次 init 超时（10s）");
        assert!(
            first.is_ok(),
            "进程内首次 init 应成功，实际 {:?}",
            first.err()
        );
        let second = second
            .expect("首次 init 未成功，二次调用未执行")
            .expect("二次 init 超时（10s）");
        match second {
            Err(inklog::InklogError::ConfigError(_)) => {}
            other => panic!(
                "二次 init 应返回 InklogError::ConfigError，实际 {:?}",
                other
            ),
        }
    }
}
