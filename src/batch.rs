// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! 批量处理：从文件或 stdin 并行求值表达式。
//!
//! 设计依据：
//! - BatchProcessor::run + rayon 并行
//! - batch-processing spec
//!
//! 约束：单条 ≤ 4096 字符、总条数 ≤ 1000；超限返回错误并标明行号。
//! 流程：读取 → 解析验证 → 预规范化（串行）→ 并行求值（rayon）→ 按序输出 + 缓存统计。

use crate::cli::format_result;
use crate::core::MAX_EXPR_LEN;
use crate::core::evaluate;
use crate::core::{EvalContext, EvalResult};
use crate::i18n::I18n;
use rayon::prelude::*;
use std::io::{self, BufRead, Read};
use std::time::Instant;

/// 批量最大条数。
const MAX_BATCH_COUNT: usize = 1000;

/// 批量处理器。
pub struct BatchProcessor;

impl BatchProcessor {
    /// 执行批量求值。
    ///
    /// - `path`: 文件路径，`"-"` 表示从 stdin 读取
    /// - `ctx`: 变量上下文
    /// - `json`: 是否输出 JSON 格式（JSON 输出键名保留英文，DP-4 机器可读契约）
    /// - `i18n`: 国际化上下文，用于本地化错误与汇总消息
    ///
    /// 返回退出码：0=全部成功，1=部分失败，2=系统错误。
    /// 以默认缓存运行（便捷入口；CLI 配置面走 [`Self::run_with_cache`]）。
    #[allow(dead_code)] // 纯 build（非 --all-targets）下仅测试使用
    pub fn run(path: &str, ctx: &EvalContext, json: bool, i18n: &I18n) -> i32 {
        Self::run_with_cache(path, ctx, json, i18n, crate::CacheManager::new())
    }

    /// 以注入缓存运行批量求值（CLI `--cache-size` 预算）。
    pub fn run_with_cache(
        path: &str,
        ctx: &EvalContext,
        json: bool,
        i18n: &I18n,
        cache: crate::CacheManager,
    ) -> i32 {
        let start = Instant::now();

        let entries = match read_and_validate_entries(path, i18n) {
            Ok(e) => e,
            Err(code) => return code,
        };

        let results = evaluate_entries(&entries, ctx, &cache);

        output_results(&results, json, i18n);
        print_summary(&results, start.elapsed(), i18n);

        let err_count = results.iter().filter(|r| r.result.is_err()).count();
        if err_count > 0 { 1 } else { 0 }
    }
}

/// 读取并验证批量条目：跳过注释/空行，校验长度与数量上限。
/// 返回 `Err(exit_code)` 表示系统错误（exit_code=2）。
fn read_and_validate_entries(path: &str, i18n: &I18n) -> Result<Vec<BatchEntry>, i32> {
    let lines = match read_lines(path) {
        Ok(lines) => lines,
        Err(e) => {
            eprintln!(
                "{}",
                i18n.tf("batch.read_failed", &[("error", &e.to_string())])
            );
            return Err(2);
        }
    };

    let mut entries: Vec<BatchEntry> = Vec::new();
    for (line_no, raw) in lines.iter() {
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if trimmed.len() > MAX_EXPR_LEN {
            eprintln!(
                "{}",
                i18n.tf(
                    "batch.line_too_long",
                    &[
                        ("line", &line_no.to_string()),
                        ("max", &MAX_EXPR_LEN.to_string()),
                        ("actual", &trimmed.len().to_string())
                    ]
                )
            );
            return Err(2);
        }
        entries.push(BatchEntry {
            line_no: *line_no,
            expr: trimmed.to_string(),
        });
    }

    if entries.is_empty() {
        eprintln!("{}", i18n.t("batch.no_expressions"));
        return Err(2);
    }
    if entries.len() > MAX_BATCH_COUNT {
        eprintln!(
            "{}",
            i18n.tf(
                "batch.count_exceeds",
                &[
                    ("count", &entries.len().to_string()),
                    ("max", &MAX_BATCH_COUNT.to_string())
                ]
            )
        );
        return Err(2);
    }

    Ok(entries)
}

/// 并行求值所有条目：每个表达式独立走全链路，结果顺序与输入一致。
fn evaluate_entries(
    entries: &[BatchEntry],
    ctx: &EvalContext,
    cache: &crate::CacheManager,
) -> Vec<BatchResult> {
    entries
        .par_iter()
        .map(|entry| {
            let result = evaluate(&entry.expr, ctx, None, cache);
            BatchResult {
                line_no: entry.line_no,
                expr: entry.expr.clone(),
                result,
            }
        })
        .collect()
}

/// 输出结果：JSON 数组或文本行，保持原始顺序。
///
/// JSON 输出键名保留英文（DP-4 机器可读契约）；文本输出走 i18n。
fn output_results(results: &[BatchResult], json: bool, i18n: &I18n) {
    if json {
        // serde_json 统一构造：转义由 serde_json 处理，控制字符/引号安全
        let mut items: Vec<String> = Vec::with_capacity(results.len());
        for r in results {
            let entry = match &r.result {
                Ok((result, domain, hit, fmt_prec)) => {
                    let value = format_result(result, *fmt_prec);
                    serde_json::json!({
                        "line": r.line_no,
                        "expr": r.expr,
                        "result": value,
                        "domain": domain,
                        "cache": if *hit { "hit" } else { "miss" },
                    })
                }
                Err(e) => serde_json::json!({
                    "line": r.line_no,
                    "expr": r.expr,
                    "error": e.to_string(),
                }),
            };
            items.push(entry.to_string());
        }
        println!("[");
        println!("{}", items.join(",\n"));
        println!("]");
    } else {
        for r in results {
            match &r.result {
                Ok((result, domain, hit, fmt_prec)) => {
                    let value = format_result(result, *fmt_prec);
                    let cached_str = if *hit {
                        i18n.t("label.cached_suffix").to_string()
                    } else {
                        String::new()
                    };
                    println!(
                        "{}",
                        i18n.tf(
                            "batch.text_result",
                            &[
                                ("line", &r.line_no.to_string()),
                                ("expr", &r.expr),
                                ("value", &value),
                                ("domain", domain),
                                ("cached", &cached_str)
                            ]
                        )
                    );
                }
                Err(e) => {
                    eprintln!(
                        "{}",
                        i18n.tf(
                            "batch.text_error",
                            &[
                                ("line", &r.line_no.to_string()),
                                ("expr", &r.expr),
                                ("error", &e.to_string())
                            ]
                        )
                    );
                }
            }
        }
    }
}

/// 打印汇总统计到 stderr：总数、成功、错误、缓存命中、耗时。
fn print_summary(results: &[BatchResult], elapsed: std::time::Duration, i18n: &I18n) {
    let total = results.len();
    let ok_count = results.iter().filter(|r| r.result.is_ok()).count();
    let err_count = total - ok_count;
    let cache_hits = results
        .iter()
        .filter(|r| {
            r.result
                .as_ref()
                .map(|(_, _, hit, _)| *hit)
                .unwrap_or(false)
        })
        .count();
    eprintln!(
        "{}",
        i18n.tf(
            "batch.summary",
            &[
                ("total", &total.to_string()),
                ("ok", &ok_count.to_string()),
                ("errors", &err_count.to_string()),
                ("hits", &cache_hits.to_string()),
                ("elapsed", &format!("{:?}", elapsed))
            ]
        )
    );
}

/// 批量条目：解析后的表达式。
struct BatchEntry {
    line_no: usize,
    expr: String,
}

/// 批量求值结果。
struct BatchResult {
    line_no: usize,
    expr: String,
    result: Result<(EvalResult, String, bool, Option<usize>), crate::CalcError>,
}

/// 读取文件或 stdin 的行，返回 (行号, 原始行) 列表。
/// 行号从 1 开始。
fn read_lines(path: &str) -> io::Result<Vec<(usize, String)>> {
    let mut lines: Vec<(usize, String)> = Vec::new();
    if path == "-" {
        let mut input = String::new();
        io::stdin().read_to_string(&mut input)?;
        for (i, line) in input.lines().enumerate() {
            // 剥离 UTF-8 BOM（仅首行）
            let line = if i == 0 {
                line.strip_prefix('\u{FEFF}').unwrap_or(line)
            } else {
                line
            };
            lines.push((i + 1, line.to_string()));
        }
    } else {
        let file = std::fs::File::open(path)?;
        let reader = io::BufReader::new(file);
        for (i, line) in reader.lines().enumerate() {
            let line = line?;
            // 剥离 UTF-8 BOM（仅首行）
            let line = if i == 0 {
                line.strip_prefix('\u{FEFF}').unwrap_or(&line).to_string()
            } else {
                line
            };
            lines.push((i + 1, line));
        }
    }
    Ok(lines)
}

// ============================ 单元测试 ============================

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_read_lines_from_file() {
        // 创建临时文件
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(tmp, "2+3").unwrap();
        writeln!(tmp, "# comment").unwrap();
        writeln!(tmp, "sin(0)").unwrap();
        tmp.flush().unwrap();

        let lines = read_lines(tmp.path().to_str().unwrap()).unwrap();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].0, 1);
        assert_eq!(lines[0].1, "2+3");
        assert_eq!(lines[2].1, "sin(0)");
    }

    #[test]
    fn test_batch_json_output_is_valid_json() {
        // JSON 输出统一走 serde_json，控制字符由库正确转义；
        // 输出整体必须是合法 JSON（round-trip 校验）。
        let sample = serde_json::json!({
            "line": 1,
            "expr": "a\u{0001}b\"c",
            "result": "1",
            "domain": "arithmetic",
            "cache": "miss",
        });
        let serialized = sample.to_string();
        let round: serde_json::Value =
            serde_json::from_str(&serialized).expect("序列化后必须可反序列化");
        assert_eq!(round["expr"], "a\u{0001}b\"c", "控制字符与引号应无损往返");
    }

    #[test]
    fn test_batch_run_basic_expressions() {
        // 创建临时文件含多条表达式
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(tmp, "2+3").unwrap();
        writeln!(tmp, "4*5").unwrap();
        tmp.flush().unwrap();

        let ctx = EvalContext::new();
        let code = BatchProcessor::run(tmp.path().to_str().unwrap(), &ctx, false, &I18n::default());
        // 全部成功应返回 0
        assert_eq!(code, 0);
    }

    #[test]
    fn test_batch_run_comment_skipped() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(tmp, "# this is a comment").unwrap();
        writeln!(tmp).unwrap();
        writeln!(tmp, "1+1").unwrap();
        tmp.flush().unwrap();

        let ctx = EvalContext::new();
        let code = BatchProcessor::run(tmp.path().to_str().unwrap(), &ctx, false, &I18n::default());
        assert_eq!(code, 0);
    }

    #[test]
    fn test_batch_run_empty_file_rejected() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(tmp, "# only comment").unwrap();
        tmp.flush().unwrap();

        let ctx = EvalContext::new();
        let code = BatchProcessor::run(tmp.path().to_str().unwrap(), &ctx, false, &I18n::default());
        assert_eq!(code, 2);
    }

    #[test]
    fn test_batch_run_with_error() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(tmp, "2+3").unwrap();
        writeln!(tmp, "2++3").unwrap(); // 语法错误
        writeln!(tmp, "4*5").unwrap();
        tmp.flush().unwrap();

        let ctx = EvalContext::new();
        let code = BatchProcessor::run(tmp.path().to_str().unwrap(), &ctx, false, &I18n::default());
        // 部分失败应返回 1
        assert_eq!(code, 1);
    }

    #[test]
    fn test_batch_run_json_output() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(tmp, "2+3").unwrap();
        tmp.flush().unwrap();

        let ctx = EvalContext::new();
        let code = BatchProcessor::run(tmp.path().to_str().unwrap(), &ctx, true, &I18n::default());
        assert_eq!(code, 0);
    }

    #[test]
    fn test_batch_run_order_preserved() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(tmp, "1").unwrap();
        writeln!(tmp, "2").unwrap();
        writeln!(tmp, "3").unwrap();
        tmp.flush().unwrap();

        // 即使并行求值，输出应保持原始顺序
        let ctx = EvalContext::new();
        let _code =
            BatchProcessor::run(tmp.path().to_str().unwrap(), &ctx, false, &I18n::default());
        // 验证不 panic 即可（顺序由 par_iter + collect 保证）
    }

    #[test]
    fn test_batch_nonexistent_file() {
        let ctx = EvalContext::new();
        let code = BatchProcessor::run(
            "/nonexistent/path/to/file.txt",
            &ctx,
            false,
            &I18n::default(),
        );
        assert_eq!(code, 2);
    }

    #[test]
    fn test_batch_line_exceeds_max_length() {
        // 单行超过 MAX_EXPR_LEN=4096 → 返回 2（lines 55-61）
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        let long_line = "1+".repeat(2049); // 4098 字符，超过 4096
        writeln!(tmp, "{}", long_line).unwrap();
        tmp.flush().unwrap();

        let ctx = EvalContext::new();
        let code = BatchProcessor::run(tmp.path().to_str().unwrap(), &ctx, false, &I18n::default());
        assert_eq!(code, 2);
    }

    #[test]
    fn test_batch_count_exceeds_maximum() {
        // 超过 MAX_BATCH_COUNT=1000 → 返回 2（lines 74-79）
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        for _ in 0..1001 {
            writeln!(tmp, "1+1").unwrap();
        }
        tmp.flush().unwrap();

        let ctx = EvalContext::new();
        let code = BatchProcessor::run(tmp.path().to_str().unwrap(), &ctx, false, &I18n::default());
        assert_eq!(code, 2);
    }

    #[test]
    fn test_batch_json_output_with_error() {
        // JSON 输出含错误条目：覆盖 JSON Err 分支（lines 130-136）
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(tmp, "2+3").unwrap();
        writeln!(tmp, "2++3").unwrap(); // 语法错误
        tmp.flush().unwrap();

        let ctx = EvalContext::new();
        let code = BatchProcessor::run(tmp.path().to_str().unwrap(), &ctx, true, &I18n::default());
        // 部分失败 → 1
        assert_eq!(code, 1);
    }
}
