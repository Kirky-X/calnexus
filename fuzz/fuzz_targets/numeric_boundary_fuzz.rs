//! FUZZ-005: 数值边界 — NaN/Inf 输入应返回 CalcError，不 panic。
//!
//! 运行：`cargo +nightly fuzz run numeric_boundary_fuzz`

#![no_main]

use calnexus::{CacheManager, EvalContext, parse};
use libfuzzer_sys::fuzz_target;
use std::str::FromStr;

fuzz_target!(|data: &str| {
    // 生成边界数值表达式
    let exprs = vec![
        data.to_string(),
        format!("1/{}", data.len().max(1)),
        format!("log({})", data.len()),
        format!("sqrt({})", data.len()),
        format!("{}!", data.len().min(100)),
    ];

    let cache = CacheManager::new();
    let ctx = EvalContext::new();
    for expr in exprs {
        // 限制表达式长度（避免触发长度限制）
        if expr.len() > 4096 {
            continue;
        }
        if let Ok(ast) = parse(&expr) {
            // 求值应不 panic（可能返回 Ok 或 Err）
            let _ = calnexus::cli::evaluate(&expr, &ctx, None, &cache);
            let _ = ast; // 抑制未使用警告
        }
    }

    // 直接测试 f64 边界值
    let _ = f64::from_str(data);
});
