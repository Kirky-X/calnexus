// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! E2E 场景套件 —— 基于 specmark change `e2e-scenario-suite`。
//!
//! 按五类场景组织子模块（specmark/specs/test-coverage R-coverage-002）：
//!
//! | 模块             | 场景类型                     |
//! |------------------|------------------------------|
//! | `happy_path`     | S1 正常路径                  |
//! | `edge_cases`     | S2 边界条件                  |
//! | `error_paths`    | S3 异常路径                  |
//! | `fx_mock`        | S4 fx 域离线全链路（mock）   |
//! | `feature_gates`  | S5 特性组合矩阵              |
//! | `cache_router`   | S6 缓存与路由                |
//! | `security`       | S7 安全与健壮性              |
//! | `server_e2e`     | S8 HTTP/MCP/ratelimit/docs   |
//! | `cli_e2e`        | S9 CLI/批处理缺口            |
//!
//! 目标无 required-features：一切 feature 组合下编译，模块内 `#[cfg]`
//! 门控启用/禁用两侧用例。套件不发起出网请求（fx 走 mock 域 + 容忍冒烟）。

#![allow(clippy::result_large_err)]
// 测试输入使用 3.14 等近似常量值（非数学常量误用），与 lib.rs 同例豁免。
#![allow(clippy::approx_constant)]

mod common;

mod cache_router;
mod edge_cases;
mod error_paths;
mod feature_gates;
mod happy_path;
mod security;

#[cfg(feature = "cli")]
mod cli_e2e;
#[cfg(feature = "fx")]
mod fx_mock;
#[cfg(feature = "server")]
mod server_e2e;
