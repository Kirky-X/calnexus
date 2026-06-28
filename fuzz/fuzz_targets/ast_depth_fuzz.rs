//! FUZZ-002: 深度嵌套输入 → 应返回 DepthExceeded 或解析错误，不 panic / 不栈溢出。
//!
//! 运行：`cargo +nightly fuzz run ast_depth_fuzz`

#![no_main]

use calnexus::{AstCanonicalizer, parse};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &str| {
    // 构造深度嵌套：用输入字节驱动嵌套屽数
    let depth = data.len().min(512);
    let expr = format!("{}1{}", "(".repeat(depth), ")".repeat(depth));
    if let Ok(ast) = parse(&expr) {
        // 规范化应不 panic（可能返回 DepthExceeded 或 Ok）
        let _ = AstCanonicalizer::canonicalize(&ast);
    }
});
