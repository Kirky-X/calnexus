//! FUZZ-003: 缓存键生成不应 panic，且等价表达式应产生相同键（无哈希碰撞 panic）。
//!
//! 运行：`cargo +nightly fuzz run cache_key_fuzz`

#![no_main]

use calnexus::{AstCanonicalizer, CacheManager, EvalContext, parse};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &str| {
    // 任意输入解析 + 规范化 + 缓存键生成
    if let Ok(ast) = parse(data) {
        if let Ok((canonical_ast, cf)) = AstCanonicalizer::canonicalize(&ast) {
            // 缓存键生成不应 panic
            let cache = CacheManager::new();
            let ctx = EvalContext::new();
            // 插入缓存
            cache.insert(&cf, &Ok(calnexus::EvalResult::Scalar(0.0)));
            // 查询缓存（应命中）
            let _ = cache.get(&cf);

            // 二次规范化应幂等
            if let Ok((_, cf2)) = AstCanonicalizer::canonicalize(&canonical_ast) {
                assert_eq!(cf.as_str(), cf2.as_str(), "canonicalize should be idempotent");
            }
        }
    }
});
