// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! 深度嵌套输入 → DepthExceeded 或解析错误，不 panic / 不栈溢出。
//!
//! 运行：`cargo +nightly fuzz run ast_depth_fuzz`

#![no_main]

use calnexus::{AstCanonicalizer, parse};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &str| {
    // 输入字节驱动嵌套层数（深度防护在解析入口迭代预检拒绝）
    let depth = data.len() / 2;
    let expr = format!("{}1{}", "(".repeat(depth), ")".repeat(depth));
    if let Ok(ast) = parse(&expr) {
        // 规范化不 panic
        let _ = AstCanonicalizer::canonicalize(&ast);
    }
});
