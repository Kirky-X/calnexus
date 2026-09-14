// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! 深度嵌套输入 → 应返回 DepthExceeded 或解析错误，不 panic / 不栈溢出。
//!
//! 移除 min(512) 截断——输入长度由表达式全局上限（4096 字符）
//! 约束，截断恰好绕开了 513-2048 层的真实风险窗口。
//!
//! 运行：`cargo +nightly fuzz run ast_depth_fuzz`

#![no_main]

use calnexus::{AstCanonicalizer, parse};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &str| {
    // 构造深度嵌套：用输入字节驱动嵌套层数（深度防护在解析入口迭代预检拒绝）
    let depth = data.len() / 2;
    let expr = format!("{}1{}", "(".repeat(depth), ")".repeat(depth));
    if let Ok(ast) = parse(&expr) {
        // 规范化应不 panic（可能返回 DepthExceeded 或 Ok）
        let _ = AstCanonicalizer::canonicalize(&ast);
    }
});
