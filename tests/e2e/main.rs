// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! E2E 场景套件（S1-S9）：happy_path/edge_cases/error_paths/fx_mock/
//! feature_gates/cache_router/security/server_e2e/cli_e2e 子模块，无 required-features。

#![allow(clippy::result_large_err)]
// 测试输入含 3.14 等近似常量，与 lib.rs 同例豁免 clippy。
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
