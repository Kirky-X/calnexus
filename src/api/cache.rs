// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! API 缓存键构建。

use crate::core::EvalContext;

/// 构建 API 缓存键。
///
/// 格式：`api:<func_name>|<args>\0分隔|vars=<hash>|timeout=<nanos>`
pub fn build_api_cache_key(
    func_name: &str,
    args: &[String],
    _ctx: &EvalContext,
) -> String {
    let args_str = args.join("\0");
    format!("api:{}|{}", func_name, args_str)
}
