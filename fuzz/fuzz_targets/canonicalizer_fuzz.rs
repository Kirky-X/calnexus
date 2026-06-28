// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! FUZZ-004: 规范化器幂等性 — `canonicalize(canonicalize(x)) == canonicalize(x)`，不 panic。
//!
//! 运行：`cargo +nightly fuzz run canonicalizer_fuzz`

#![no_main]

use calnexus::{AstCanonicalizer, parse};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &str| {
    if let Ok(ast) = parse(data) {
        if let Ok((canon_ast, cf1)) = AstCanonicalizer::canonicalize(&ast) {
            // 二次规范化
            if let Ok((_, cf2)) = AstCanonicalizer::canonicalize(&canon_ast) {
                // 幂等性：二次规范化结果应一致
                assert_eq!(
                    cf1.as_str(),
                    cf2.as_str(),
                    "canonicalize not idempotent for input: {:?}",
                    data
                );
            }
        }
    }
});
