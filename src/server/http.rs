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

use super::ServerError;

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
    /// 供 CLI `--serve-http` flag 调用。T016 实现 ServerAdapter 后启用。
    pub fn run(&self) -> Result<(), ServerError> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|e| ServerError::Http(format!("failed to create tokio runtime: {}", e)))?;
        runtime.block_on(async move { self.start_inner().await })
    }

    /// 内部 async 启动逻辑（T016 实现）。
    async fn start_inner(&self) -> Result<(), ServerError> {
        // T016 将实现：sdforge::http::build() + axum::serve + inventory 路由注册
        Err(ServerError::Http("not yet implemented (T016)".into()))
    }
}
