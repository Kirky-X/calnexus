// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! 矩阵维度 fuzz：任意维度输入不得 OOM，应被拒绝或合理处理。
//! 运行：`cargo +nightly fuzz run matrix_dim_fuzz`。

#![no_main]

use calnexus::{CacheManager, EvalContext, parse};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.len() < 2 {
        return;
    }
    // 前两个字节决定矩阵维度，限 1..=10 避免 OOM
    let rows = (data[0] as usize % 10) + 1; // 1..=10
    let cols = (data[1] as usize % 10) + 1; // 1..=10

    // 构造 rows×cols 矩阵表达式
    let mut expr = String::from("[");
    for i in 0..rows {
        if i > 0 {
            expr.push(',');
        }
        expr.push('[');
        for j in 0..cols {
            if j > 0 {
                expr.push(',');
            }
            expr.push_str(&format!("{}", i * cols + j));
        }
        expr.push(']');
    }
    expr.push(']');

    // 解析 + 求值应不 panic
    if let Ok(_) = parse(&expr) {
        let cache = CacheManager::new();
        let ctx = EvalContext::new();
        let _ = calnexus::evaluate(&expr, &ctx, None, &cache);
    }
});
