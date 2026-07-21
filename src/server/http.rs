// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! HTTP server 启动：`POST /api/v1/evaluate` 路由由 `#[forge]` 宏声明（evaluate.rs），
//! 本模块仅负责构建 Router（body limit）与启动 HttpServer。
//!
//! spec.md R-sdforge-002 定义接口契约。

use super::ServerError;
use axum::extract::DefaultBodyLimit;
use axum::Router;

/// 请求体大小上限（64KB，T016 安全前置任务：防止超大请求体耗尽内存）。
const MAX_BODY_SIZE: usize = 64 * 1024;

/// 构建 CalNexus HTTP Router：`init_all_plugins()` + `sdforge::http::build()` + body limit。
///
/// `init_all_plugins()` 替代 p1 的 `preserve_http_inventory()` 链接器 hack，
/// 确保 `#[forge]` 注册的 evaluate 路由被 `http::build()` 收集。
/// `DefaultBodyLimit` 防止超大请求体攻击（保留 p1 安全约束）。
pub fn build_router() -> Router {
    sdforge::init_all_plugins();
    sdforge::http::build().layer(DefaultBodyLimit::max(MAX_BODY_SIZE))
}

/// 优雅关闭信号：监听 Ctrl+C（SIGINT），返回时 axum 停止接受新连接、等待已有连接完成。
///
/// BUG-S-M-003: 此前 `axum::serve` 无 `with_graceful_shutdown`，收到 SIGINT 时
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
    /// BUG-S-M-003: `with_graceful_shutdown(shutdown_signal())` 让 server 收到
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

    /// BUG-S-M-003: 验证 `shutdown_signal` 返回的 future 为 `Send`。
    /// axum `with_graceful_shutdown` 要求 signal future 为 `Send`，编译期检查。
    /// 此测试在编译期捕获 Send 约束违规（若 future 非 Send，编译失败）。
    #[test]
    fn test_shutdown_signal_is_send() {
        fn assert_send<T: Send>(_t: T) {}
        let fut = shutdown_signal();
        assert_send(fut);
    }

    /// BUG-S-M-003: 验证 `shutdown_signal` 函数存在且可调用（构造即不 panic）。
    /// 不实际 await（await 需要真实信号，单元测试环境难以注入）。
    #[test]
    fn test_shutdown_signal_callable_without_panic() {
        let _fut = shutdown_signal();
        // 不 await，仅验证构造无 panic（信号 handler 安装在 await 阶段）
    }
}
