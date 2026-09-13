// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! FUZZ-007: 嵌套列表/矩阵字面量 → 不 panic；结果为 Ok 或 DepthExceeded。
//!
//! v015 新增（R-depth-004）：既有 ast_depth_fuzz 只覆盖圆括号，恰好绕开审计发现的
//! 列表/矩阵字面量深度守卫缺口（parse_list_literal 递归无计数）。
//!
//! 运行：`cargo +nightly fuzz run list_depth_fuzz`

#![no_main]

use calnexus::{AstCanonicalizer, parse};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // 用输入字节驱动嵌套层数与字面量形态（列表 / 矩阵交替）
    let depth = (data.len().max(1) * 4).min(2000);
    let matrix_mode = data.first().is_some_and(|b| b % 2 == 1);

    let expr = if matrix_mode {
        format!("{}1{}", "[[".repeat(depth), "]]".repeat(depth))
    } else {
        let mut expr = String::new();
        for i in 0..depth {
            expr.push('[');
            expr.push_str(&(i % 10).to_string());
            expr.push(',');
        }
        expr.push('1');
        expr.push_str(&"]".repeat(depth));
        expr
    };

    // 契约：不 panic；Ok（深度内合法）或 DepthExceeded（超限拒绝）皆可
    if let Ok(ast) = parse(&expr) {
        let _ = AstCanonicalizer::canonicalize(&ast);
    }
});
