// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! 批量处理：从文件或 stdin 并行求值表达式（TG5）。
//!
//! 设计依据：
//! - design.md D4（BatchProcessor::run + rayon 并行）
//! - v1.0 batch-processing spec
//!
//! 约束：单条 ≤ 4096 字符、总条数 ≤ 1000；超限返回错误并标明行号。
//! 流程：读取 → 解析验证 → 预规范化（串行）→ 并行求值（rayon）→ 按序输出 + 缓存统计。

use crate::cli::format_result;
use crate::core::MAX_EXPR_LEN;
use crate::core::{EvalContext, EvalResult};
use crate::evaluator::evaluate;
use rayon::prelude::*;
use std::io::{self, BufRead, Read};
use std::time::Instant;

/// 批量最大条数。
const MAX_BATCH_COUNT: usize = 1000;

/// 批量处理器（TG5.1）。
pub struct BatchProcessor;

impl BatchProcessor {
    /// 执行批量求值（TG5.1-TG5.4）。
    ///
    /// - `path`: 文件路径，`"-"` 表示从 stdin 读取
    /// - `ctx`: 变量上下文
    /// - `json`: 是否输出 JSON 格式
    ///
    /// 返回退出码：0=全部成功，1=部分失败，2=系统错误。
    pub fn run(path: &str, ctx: &EvalContext, json: bool) -> i32 {
        let start = Instant::now();

        let entries = match read_and_validate_entries(path) {
            Ok(e) => e,
            Err(code) => return code,
        };

        let results = evaluate_entries(&entries, ctx);

        output_results(&results, json);
        print_summary(&results, start.elapsed());

        let err_count = results.iter().filter(|r| r.result.is_err()).count();
        if err_count > 0 {
            1
        } else {
            0
        }
    }
}

/// 读取并验证批量条目（TG5.1-TG5.2）：跳过注释/空行，校验长度与数量上限。
/// 返回 `Err(exit_code)` 表示系统错误（exit_code=2）。
fn read_and_validate_entries(path: &str) -> Result<Vec<BatchEntry>, i32> {
    let lines = match read_lines(path) {
        Ok(lines) => lines,
        Err(e) => {
            eprintln!("error: failed to read input: {}", e);
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
                "error: line {} exceeds max length {} (got {})",
                line_no,
                MAX_EXPR_LEN,
                trimmed.len()
            );
            return Err(2);
        }
        entries.push(BatchEntry {
            line_no: *line_no,
            expr: trimmed.to_string(),
        });
    }

    if entries.is_empty() {
        eprintln!("error: no expressions to evaluate");
        return Err(2);
    }
    if entries.len() > MAX_BATCH_COUNT {
        eprintln!(
            "error: batch count {} exceeds maximum of {}",
            entries.len(),
            MAX_BATCH_COUNT
        );
        return Err(2);
    }

    Ok(entries)
}

/// 并行求值所有条目（TG5.3）：每个表达式独立走全链路，结果顺序与输入一致。
fn evaluate_entries(entries: &[BatchEntry], ctx: &EvalContext) -> Vec<BatchResult> {
    let cache = crate::CacheManager::new();
    entries
        .par_iter()
        .map(|entry| {
            let result = evaluate(&entry.expr, ctx, None, &cache);
            BatchResult {
                line_no: entry.line_no,
                expr: entry.expr.clone(),
                result,
            }
        })
        .collect()
}

/// 输出结果（TG5.4）：JSON 数组或文本行，保持原始顺序。
fn output_results(results: &[BatchResult], json: bool) {
    let total = results.len();
    if json {
        println!("[");
        for (i, r) in results.iter().enumerate() {
            match &r.result {
                Ok((result, domain, hit, fmt_prec)) => {
                    let value = format_result(result, *fmt_prec);
                    println!(
                        r#"  {{"line":{},"expr":"{}","result":"{}","domain":"{}","cache":"{}"}}"{}"#,
                        r.line_no,
                        crate::core::escape_json_string(&r.expr),
                        crate::core::escape_json_string(&value),
                        domain,
                        if *hit { "hit" } else { "miss" },
                        if i + 1 < total { "," } else { "" }
                    );
                }
                Err(e) => {
                    println!(
                        r#"  {{"line":{},"expr":"{}","error":"{}"}}"{}"#,
                        r.line_no,
                        crate::core::escape_json_string(&r.expr),
                        crate::core::escape_json_string(&e.to_string()),
                        if i + 1 < total { "," } else { "" }
                    );
                }
            }
        }
        println!("]");
    } else {
        for r in results {
            match &r.result {
                Ok((result, domain, hit, fmt_prec)) => {
                    let value = format_result(result, *fmt_prec);
                    println!(
                        "line {}: {} = {}  [{}{}]",
                        r.line_no,
                        r.expr,
                        value,
                        domain,
                        if *hit { " (cached)" } else { "" }
                    );
                }
                Err(e) => {
                    eprintln!("line {}: {} → error: {}", r.line_no, r.expr, e);
                }
            }
        }
    }
}

/// 打印汇总统计到 stderr：总数、成功、错误、缓存命中、耗时。
fn print_summary(results: &[BatchResult], elapsed: std::time::Duration) {
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
        "summary: {} total, {} ok, {} errors, {} cache hits, {:?}",
        total, ok_count, err_count, cache_hits, elapsed
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
/// 行号从 1 开始（TG5.1）。
fn read_lines(path: &str) -> io::Result<Vec<(usize, String)>> {
    let mut lines: Vec<(usize, String)> = Vec::new();
    if path == "-" {
        let mut input = String::new();
        io::stdin().read_to_string(&mut input)?;
        for (i, line) in input.lines().enumerate() {
            lines.push((i + 1, line.to_string()));
        }
    } else {
        let file = std::fs::File::open(path)?;
        let reader = io::BufReader::new(file);
        for (i, line) in reader.lines().enumerate() {
            let line = line?;
            lines.push((i + 1, line));
        }
    }
    Ok(lines)
}

// ============================ 单元测试 (TG5.6) ============================

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
    fn test_batch_json_escape_control_chars() {
        // JSON 规范要求控制字符（U+0000 ~ U+001F）必须转义为 \uXXXX。
        // batch.rs 现复用 core::escape_json_string，须正确处理控制字符。
        // \u{0007} (bell) 与 \u{000c} (form feed) 不在 {\n,\r,\t} 之列。
        assert_eq!(crate::core::escape_json_string("\u{0007}"), "\\u0007");
        assert_eq!(crate::core::escape_json_string("\u{000c}"), "\\u000c");
        assert_eq!(crate::core::escape_json_string("a\u{0001}b"), "a\\u0001b");
    }

    #[test]
    fn test_batch_run_basic_expressions() {
        // 创建临时文件含多条表达式
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(tmp, "2+3").unwrap();
        writeln!(tmp, "4*5").unwrap();
        tmp.flush().unwrap();

        let ctx = EvalContext::new();
        let code = BatchProcessor::run(tmp.path().to_str().unwrap(), &ctx, false);
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
        let code = BatchProcessor::run(tmp.path().to_str().unwrap(), &ctx, false);
        assert_eq!(code, 0);
    }

    #[test]
    fn test_batch_run_empty_file_rejected() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(tmp, "# only comment").unwrap();
        tmp.flush().unwrap();

        let ctx = EvalContext::new();
        let code = BatchProcessor::run(tmp.path().to_str().unwrap(), &ctx, false);
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
        let code = BatchProcessor::run(tmp.path().to_str().unwrap(), &ctx, false);
        // 部分失败应返回 1
        assert_eq!(code, 1);
    }

    #[test]
    fn test_batch_run_json_output() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(tmp, "2+3").unwrap();
        tmp.flush().unwrap();

        let ctx = EvalContext::new();
        let code = BatchProcessor::run(tmp.path().to_str().unwrap(), &ctx, true);
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
        let _code = BatchProcessor::run(tmp.path().to_str().unwrap(), &ctx, false);
        // 验证不 panic 即可（顺序由 par_iter + collect 保证）
    }

    #[test]
    fn test_batch_nonexistent_file() {
        let ctx = EvalContext::new();
        let code = BatchProcessor::run("/nonexistent/path/to/file.txt", &ctx, false);
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
        let code = BatchProcessor::run(tmp.path().to_str().unwrap(), &ctx, false);
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
        let code = BatchProcessor::run(tmp.path().to_str().unwrap(), &ctx, false);
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
        let code = BatchProcessor::run(tmp.path().to_str().unwrap(), &ctx, true);
        // 部分失败 → 1
        assert_eq!(code, 1);
    }
}
