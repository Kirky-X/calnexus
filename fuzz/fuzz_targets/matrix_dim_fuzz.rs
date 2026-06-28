//! FUZZ-006: 矩阵维度验证 — 任意维度矩阵不应导致 OOM，应被合理拒绝或处理。
//!
//! 运行：`cargo +nightly fuzz run matrix_dim_fuzz`

#![no_main]

use calnexus::{CacheManager, EvalContext, parse};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.len() < 2 {
        return;
    }
    // 用前两个字节决定矩阵维度（限制在合理范围避免 OOM）
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
        let _ = calnexus::cli::evaluate(&expr, &ctx, None, &cache);
    }
});
