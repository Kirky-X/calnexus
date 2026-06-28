//! FUZZ-001: 任意 UTF-8 输入 → `parse()`，不应 panic。
//!
//! 运行：`cargo +nightly fuzz run parser_fuzz`

#![no_main]

use calnexus::parse;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &str| {
    // 任意 UTF-8 字符串解析，仅要求不 panic（返回 Ok/Err 均可）
    let _ = parse(data);
});
