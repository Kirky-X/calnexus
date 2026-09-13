// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! CalNexus CLI：命令行数学表达式求值器。
//!
//! 全链路：Parser → Canonicalizer → CacheManager → DomainRouter → Domain::evaluate
//!
//! 退出码（design.md §5.6）：
//! - 0：成功
//! - 1：计算错误 / 解析错误
//! - 2：用法错误
//! - 3：超时

use crate::core::evaluate;
use crate::domains::format_bigrational;
use crate::output::{format_canonical, format_latex, generate_steps};
use crate::{parse, AstCanonicalizer, CacheManager, CalcError, EvalContext, EvalResult};
use sdforge::clap::{self, Parser};
use std::io::{self, IsTerminal, Read};

#[derive(Parser)]
#[command(
    name = "calnexus",
    version,
    about = "CalNexus: math expression evaluator",
    long_about = None
)]
struct Cli {
    /// Expression to evaluate (reads from stdin if omitted and piped)
    expression: Option<String>,

    /// Variable binding: --var NAME=VALUE (can be repeated)
    #[arg(long = "var")]
    vars: Vec<String>,

    /// Output result as JSON with domain and cache metadata
    #[arg(long, conflicts_with_all = ["latex", "canonical", "steps"])]
    json: bool,

    /// Output detailed error explanation (conflicts with --json)
    #[arg(long, conflicts_with_all = ["json"])]
    explain: bool,

    /// Language for error messages: en or zh (default: en)
    #[arg(long, default_value = "en", value_parser = clap::builder::PossibleValuesParser::new(["en", "zh"]))]
    lang: String,

    /// Arbitrary precision mode: format result to N decimal places using BigRational arithmetic
    #[arg(long, conflicts_with_all = ["canonical", "batch"])]
    precision: Option<usize>,

    /// Start interactive REPL mode (read-eval-print loop)
    #[arg(long, conflicts_with_all = ["canonical", "latex", "steps", "batch"])]
    repl: bool,

    /// Batch evaluate expressions from file ('-' for stdin), one expression per line
    #[arg(long, conflicts_with_all = ["canonical", "latex", "steps", "precision", "repl"])]
    batch: Option<String>,

    /// Render result as LaTeX (e.g., matrices as `\begin{pmatrix}...`)
    #[arg(long, conflicts_with_all = ["json", "canonical"])]
    latex: bool,

    /// Display step-by-step evaluation (e.g., `2+9=11` for `(2+9)*7-6`)
    #[arg(long, conflicts_with_all = ["json", "canonical"])]
    steps: bool,

    /// Print canonical S-expression form (e.g., `(+ 2 3)` for `3+2`), skip evaluation
    #[arg(long, conflicts_with_all = ["json", "latex", "steps", "precision", "repl", "batch"])]
    canonical: bool,

    /// Evaluation timeout in seconds (0.1-3600, default 5; env: CALNEXUS_TIMEOUT)
    #[arg(long)]
    timeout: Option<f64>,

    /// Cache entry budget, approximate (entries x 4KB byte cap; env: CALNEXUS_CACHE_SIZE)
    #[arg(long)]
    cache_size: Option<u64>,

    /// HTTP server bind address (default 127.0.0.1:3000; env: CALNEXUS_BIND_ADDR)
    #[cfg(feature = "server")]
    #[arg(long)]
    bind: Option<String>,

    /// Start HTTP server mode (POST /api/v1/evaluate). Requires `server` feature.
    #[cfg(feature = "server")]
    #[arg(long, conflicts_with_all = ["repl", "batch", "canonical", "latex", "steps", "json", "explain", "precision", "serve_mcp"])]
    serve_http: bool,

    /// Start MCP server mode (evaluate tool, stdio transport). Requires `server` feature.
    #[cfg(feature = "server")]
    #[arg(long, conflicts_with_all = ["repl", "batch", "canonical", "latex", "steps", "json", "explain", "precision", "serve_http"])]
    serve_mcp: bool,
}

/// CLI 入口：解析参数、分发到对应模式处理函数，返回退出码。
pub fn run() -> i32 {
    let cli = Cli::parse();
    let i18n = crate::i18n::I18n::from_str(&cli.lang);

    // 配置面（v015 R-cfg-001/002）：timeout / cache-size 统一解析
    // （优先级 flag > env > default；非法 env 值显性报错而非静默回退）
    let timeout_secs = match resolve_timeout(cli.timeout) {
        Ok(t) => t,
        Err(msg) => {
            eprintln!("calnexus: {}", msg);
            return 2;
        }
    };
    let cache_budget = match cache_budget_bytes(cli.cache_size) {
        Ok(b) => b,
        Err(msg) => {
            eprintln!("calnexus: {}", msg);
            return 2;
        }
    };

    // --serve-http / --serve-mcp 模式：启动 server（阻塞运行，内部创建 tokio runtime）
    #[cfg(feature = "server")]
    if cli.serve_http || cli.serve_mcp {
        return run_server_mode(&cli);
    }

    // --repl 模式：启动交互式 REPL
    if cli.repl {
        return run_repl_mode(&cli, &i18n, timeout_secs, cache_budget);
    }

    // --batch 模式：批量求值
    if let Some(path) = &cli.batch {
        return run_batch_mode(path, &cli, &i18n, timeout_secs, cache_budget);
    }

    // 以下模式需要表达式（位置参数或 stdin）
    let expr = match get_expression(&cli) {
        Ok(e) => e,
        Err(e) => return handle_error(&e, &cli, &i18n),
    };

    let mut ctx = match parse_vars(&cli.vars) {
        Ok(ctx) => ctx,
        Err(e) => return handle_error(&e, &cli, &i18n),
    };
    ctx.precision = cli.precision;
    ctx.timeout = std::time::Duration::from_secs_f64(timeout_secs);

    if cli.canonical {
        run_canonical_mode(&expr, &cli, &i18n)
    } else if cli.latex || cli.steps {
        run_latex_steps_mode(&expr, &ctx, &cli, &i18n)
    } else {
        run_default_mode(&expr, &ctx, &cli, &i18n, cache_budget)
    }
}

/// 启动 HTTP/MCP server 模式。
#[cfg(feature = "server")]
fn run_server_mode(cli: &Cli) -> i32 {
    // server 模式同样消费配置面（v015 R-cfg-002/003）
    let cache_budget = match cache_budget_bytes(cli.cache_size) {
        Ok(b) => b,
        Err(msg) => {
            eprintln!("calnexus: {}", msg);
            return 2;
        }
    };
    crate::server::init_shared_cache(cache_budget);

    let server_result = if cli.serve_http {
        let bind = match resolve_bind(cli.bind.clone()) {
            Ok(addr) => addr,
            Err(msg) => {
                eprintln!("calnexus: {}", msg);
                return 2;
            }
        };
        crate::server::HttpServer::new().with_addr(bind).run()
    } else {
        crate::server::McpServer::new().run()
    };
    match server_result {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("{}", e);
            1
        }
    }
}

/// --repl 模式：解析变量绑定并启动交互式 REPL。
fn run_repl_mode(
    cli: &Cli,
    i18n: &crate::i18n::I18n,
    timeout_secs: f64,
    cache_budget: u64,
) -> i32 {
    let mut ctx = match parse_vars(&cli.vars) {
        Ok(ctx) => ctx,
        Err(e) => return handle_error(&e, cli, i18n),
    };
    ctx.precision = cli.precision;
    ctx.timeout = std::time::Duration::from_secs_f64(timeout_secs);
    let cache = crate::CacheManager::with_capacity_bytes(cache_budget);
    crate::repl::ReplSession::with_cache(ctx, i18n.clone(), cache).run()
}

/// --batch 模式：解析变量绑定并批量求值。
fn run_batch_mode(
    path: &str,
    cli: &Cli,
    i18n: &crate::i18n::I18n,
    timeout_secs: f64,
    cache_budget: u64,
) -> i32 {
    let mut ctx = match parse_vars(&cli.vars) {
        Ok(ctx) => ctx,
        Err(e) => return handle_error(&e, cli, i18n),
    };
    ctx.timeout = std::time::Duration::from_secs_f64(timeout_secs);
    let cache = crate::CacheManager::with_capacity_bytes(cache_budget);
    crate::batch::BatchProcessor::run_with_cache(path, &ctx, cli.json, i18n, cache)
}

/// --canonical 模式：parse → canonicalize_no_fold → 输出 S-expr，跳过求值。
fn run_canonical_mode(expr: &str, cli: &Cli, i18n: &crate::i18n::I18n) -> i32 {
    let ast = match parse(expr) {
        Ok(ast) => ast,
        Err(e) => return handle_error(&e, cli, i18n),
    };
    match AstCanonicalizer::canonicalize_no_fold(&ast) {
        Ok((_canonical_ast, cf)) => {
            println!("{}", format_canonical(&cf));
            0
        }
        Err(e) => handle_error(&e, cli, i18n),
    }
}

/// --latex 和/或 --steps 模式：解析 + 规范化 + 求值 + 格式化输出。
fn run_latex_steps_mode(expr: &str, ctx: &EvalContext, cli: &Cli, i18n: &crate::i18n::I18n) -> i32 {
    let ast = match parse(expr) {
        Ok(ast) => ast,
        Err(e) => return handle_error(&e, cli, i18n),
    };
    let (canonical_ast, _cf) = match AstCanonicalizer::canonicalize(&ast) {
        Ok(pair) => pair,
        Err(e) => return handle_error(&e, cli, i18n),
    };

    // --steps 先输出步骤（基于原始 AST，避免常量折叠后无步骤可显示）
    if cli.steps {
        match generate_steps(&ast, ctx) {
            Ok(step_lines) => {
                for line in &step_lines {
                    println!("{}", line);
                }
            }
            Err(e) => return handle_error(&e, cli, i18n),
        }
    }

    // --latex：求值并输出 LaTeX 结果
    if cli.latex {
        let cache = CacheManager::with_capacity_bytes(crate::core::DEFAULT_MAX_WEIGHT_BYTES);
        match evaluate(expr, ctx, cli.precision, &cache) {
            Ok((result, _domain, _cache_hit, fmt_prec)) => {
                let latex_str = format_latex(&result, &canonical_ast, expr, fmt_prec);
                println!("{}", latex_str);
            }
            Err(e) => return handle_error(&e, cli, i18n),
        }
    }
    0
}

/// 默认模式：求值 + 输出（JSON 或文本）。
fn run_default_mode(
    expr: &str,
    ctx: &EvalContext,
    cli: &Cli,
    i18n: &crate::i18n::I18n,
    cache_budget: u64,
) -> i32 {
    let cache = CacheManager::with_capacity_bytes(cache_budget);
    match evaluate(expr, ctx, cli.precision, &cache) {
        Ok((result, domain, cache_hit, fmt_prec)) => {
            if cli.json {
                println!(
                    "{}",
                    format_json_output(&result, &domain, cache_hit, fmt_prec)
                );
            } else {
                println!("{}", format_result(&result, fmt_prec));
            }
            0
        }
        Err(e) => handle_error(&e, cli, i18n),
    }
}

/// 格式化 EvalResult 为 JSON 输出字符串。
/// Scalar 直接输出数字；Steps 输出数组；其他变体输出字符串。
fn format_json_output(
    result: &EvalResult,
    domain: &str,
    cache_hit: bool,
    fmt_prec: Option<usize>,
) -> String {
    let cache_str = if cache_hit { "hit" } else { "miss" };
    match result {
        EvalResult::Scalar(v) => format!(
            r#"{{"result":{},"domain":"{}","cache":"{}"}}"#,
            v, domain, cache_str
        ),
        EvalResult::Steps(v) => {
            let arr: Vec<String> = v
                .iter()
                .map(|s| format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")))
                .collect();
            format!(
                r#"{{"result":[{}],"domain":"{}","cache":"{}"}}"#,
                arr.join(","),
                domain,
                cache_str
            )
        }
        _ => {
            let value = format_result(result, fmt_prec);
            format!(
                r#"{{"result":"{}","domain":"{}","cache":"{}"}}"#,
                value, domain, cache_str
            )
        }
    }
}

/// 根据 CLI 配置输出 CalcError 并返回退出码。
///
/// - `--json`：输出 JSON 错误对象到 stdout
/// - `--explain`：输出详细解释到 stderr
/// - 默认：输出友好提示到 stderr
///
/// 退出码由 `ErrorKind::exit_code()` 决定（0/1/2/3）。
fn handle_error(e: &CalcError, cli: &Cli, i18n: &crate::i18n::I18n) -> i32 {
    // caret 渲染需要原始表达式：位置参数场景直接可得（stdin 场景为 None，无 caret）
    handle_error_with_expr(e, cli.expression.as_deref(), cli, i18n)
}

/// 错误渲染（v015 T021）：
/// - `--json`：结构化输出（无 caret）
/// - `--explain`：教育模式
/// - 文本模式：friendly + （有 span 时）表达式行 + `^` 位置指示
/// - `undefined_symbol` 的 hint 按上下文分发：CLI 给 `--var`，REPL 给 `:let`
fn handle_error_with_expr(
    e: &CalcError,
    expr: Option<&str>,
    cli: &Cli,
    i18n: &crate::i18n::I18n,
) -> i32 {
    if cli.json {
        // to_json() 已返回 {"error":{...}} 完整结构，无需再包装
        println!("{}", e.to_json());
        return e.kind.exit_code();
    }

    // undefined_symbol hint 上下文化：CLI 语境给 --var（REPL 语境保留 :let）
    let mut contextual = e.clone();
    let is_undefined_symbol = contextual.kind == CalcError::undefined_symbol("").kind
        || contextual.i18n_key == Some("msg.unbound_variable");
    if is_undefined_symbol {
        // 变量名优先取 i18n 参数，其次兼容两种 message 前缀
        // （undefined_symbol() 前缀 / mathexpr "Unbound variable: x"）
        let name = contextual
            .i18n_args
            .iter()
            .find(|(k, _)| k == "name")
            .map(|(_, v)| v.clone())
            .or_else(|| {
                contextual
                    .message
                    .strip_prefix("undefined symbol: ")
                    .or_else(|| contextual.message.strip_prefix("Unbound variable: "))
                    .map(str::to_string)
            });
        if let Some(name) = name {
            contextual.hint = Some(format!("define it via --var {name}=<value>"));
        }
    }
    let e = &contextual;

    if cli.explain {
        eprintln!("{}", e.to_explain(i18n));
    } else {
        eprintln!("{}: {}", i18n.t("cli.error_prefix"), e.friendly(i18n));
    }

    // caret 位置指示（仅文本模式；span 指向原始输入）
    if let (Some(expr_text), Some(span)) = (expr, &e.span) {
        let chars: Vec<char> = expr_text.chars().collect();
        let start = span.start.min(chars.len());
        let end = span.end.min(chars.len()).max(start).max(start + 1);
        eprintln!("  | {}", expr_text);
        eprint!("  | ");
        for _ in 0..start {
            eprint!(" ");
        }
        for _ in start..end {
            eprint!("^");
        }
        eprintln!();
    }

    e.kind.exit_code()
}

/// 从位置参数或 stdin 获取表达式。
///
/// i18n_key 通过 `with_i18n` 附加到 CalcError，渲染时由 `handle_error` 传入 i18n 实例。
fn get_expression(cli: &Cli) -> Result<String, CalcError> {
    if let Some(expr) = &cli.expression {
        return Ok(expr.clone());
    }
    // 无位置参数：检查 stdin
    if io::stdin().is_terminal() {
        // TTY stdin：显示 help 并退出
        Cli::parse_from(["calnexus", "--help"]);
        return Err(CalcError::usage(String::new())); // unreachable：clap 会先退出
    }
    // 管道 stdin：读取表达式
    let mut input = String::new();
    if io::stdin().read_to_string(&mut input).is_err() {
        return Err(CalcError::usage("failed to read from stdin")
            .with_i18n("cli.stdin_read_failed", vec![]));
    }
    let trimmed = input.trim().to_string();
    if trimmed.is_empty() {
        return Err(
            CalcError::usage("empty expression on stdin").with_i18n("cli.empty_stdin", vec![])
        );
    }
    Ok(trimmed)
}

/// 解析 --var NAME=VALUE 列表为 EvalContext。
///
/// i18n_key 通过 `with_i18n` 附加到 CalcError，渲染时由 `handle_error` 传入 i18n 实例。
/// 解析求值超时（秒），优先级 flag > env(`CALNEXUS_TIMEOUT`) > 默认 5.0（v015 R-cfg-001）。
fn resolve_timeout(cli_value: Option<f64>) -> Result<f64, String> {
    const DEFAULT_TIMEOUT_SECS: f64 = 5.0;
    const MIN: f64 = 0.1;
    const MAX: f64 = 3600.0;
    let raw = match cli_value {
        Some(v) => v,
        None => match std::env::var("CALNEXUS_TIMEOUT") {
            Ok(s) => s.trim().parse::<f64>().map_err(|_| {
                format!("invalid CALNEXUS_TIMEOUT '{s}': expected seconds (e.g. 0.1)")
            })?,
            Err(_) => DEFAULT_TIMEOUT_SECS,
        },
    };
    if !(MIN..=MAX).contains(&raw) {
        return Err(format!("--timeout {raw} out of range ({MIN}-{MAX} seconds)"));
    }
    Ok(raw)
}

/// 解析缓存条目预算并换算为字节权重上限（条目 × 4KB，近似语义；v015 R-cfg-002）。
fn cache_budget_bytes(cli_value: Option<u64>) -> Result<u64, String> {
    const DEFAULT_CACHE_SIZE: u64 = 10_000;
    const BYTES_PER_ENTRY: u64 = 4096;
    let entries = match cli_value {
        Some(n) => n,
        None => match std::env::var("CALNEXUS_CACHE_SIZE") {
            Ok(s) => s.trim().parse::<u64>().map_err(|_| {
                format!("invalid CALNEXUS_CACHE_SIZE '{s}': expected positive integer")
            })?,
            Err(_) => DEFAULT_CACHE_SIZE,
        },
    };
    if entries == 0 {
        return Err("--cache-size must be a positive integer".to_string());
    }
    Ok(entries.saturating_mul(BYTES_PER_ENTRY).max(BYTES_PER_ENTRY))
}

/// 解析 HTTP 绑定地址，优先级 flag > env(`CALNEXUS_BIND_ADDR`) > 默认 127.0.0.1:3000
/// （v015 R-cfg-003；仅 server feature）。
#[cfg(feature = "server")]
fn resolve_bind(cli_value: Option<String>) -> Result<String, String> {
    const DEFAULT_BIND: &str = "127.0.0.1:3000";
    match cli_value {
        Some(addr) => Ok(addr),
        None => match std::env::var("CALNEXUS_BIND_ADDR") {
            Ok(s) if !s.trim().is_empty() => Ok(s.trim().to_string()),
            Ok(_) => Err("invalid CALNEXUS_BIND_ADDR: empty value".to_string()),
            Err(_) => Ok(DEFAULT_BIND),
        },
    }
}

fn parse_vars(vars: &[String]) -> Result<EvalContext, CalcError> {    let mut ctx = EvalContext::new();
    for v in vars {
        let parts: Vec<&str> = v.splitn(2, '=').collect();
        if parts.len() != 2 {
            return Err(
                CalcError::usage(format!("invalid --var '{}', expected NAME=VALUE", v))
                    .with_i18n("cli.invalid_var", vec![("value".to_string(), v.clone())]),
            );
        }
        let value: f64 = parts[1].parse::<f64>().map_err(|e| {
            CalcError::usage(format!("invalid --var value '{}': {}", parts[1], e)).with_i18n(
                "cli.invalid_var_value",
                vec![
                    ("value".to_string(), parts[1].to_string()),
                    ("error".to_string(), e.to_string()),
                ],
            )
        })?;
        ctx = ctx.with_var(parts[0], value);
    }
    Ok(ctx)
}

/// 格式化 EvalResult 为人类可读字符串（非 JSON 模式）。
/// 供 CLI 与 REPL 共用。
pub(crate) fn format_result(result: &EvalResult, fmt_prec: Option<usize>) -> String {
    match result {
        EvalResult::Scalar(v) => v.to_string(),
        EvalResult::Complex(re, im) => format_complex(*re, *im),
        EvalResult::Matrix(m) => format_matrix(m),
        EvalResult::BigInt(b) => b.to_string(),
        EvalResult::BigRational(r) => format_bigrational(r, fmt_prec),
        EvalResult::Vector(v) => format_vector(v),
        EvalResult::Polynomial(p) => format_polynomial(p),
        EvalResult::ComplexList(c) => format_complex_list(c),
        EvalResult::Symbolic(s) => s.clone(),
        EvalResult::LaTeX(s) => s.clone(),
        EvalResult::Steps(v) => v.join("\n"),
        EvalResult::Json(v) => v.to_string(),
        // DateTime（time-unit-fx-domains D2）：RFC3339 字符串直接输出
        EvalResult::DateTime(s) => s.clone(),
    }
}

/// 格式化复数为 `re+imi` 形式（如 `3+4i`、`-2-3i`、`5+0i`）。
fn format_complex(re: f64, im: f64) -> String {
    if im >= 0.0 {
        format!("{}+{}i", re, im)
    } else {
        format!("{}{}i", re, im)
    }
}

/// 格式化矩阵为 `[[a,b],[c,d]]` 形式。
fn format_matrix(m: &[Vec<f64>]) -> String {
    let rows: Vec<String> = m
        .iter()
        .map(|row| {
            let elems: Vec<String> = row.iter().map(|v| v.to_string()).collect();
            format!("[{}]", elems.join(","))
        })
        .collect();
    format!("[{}]", rows.join(","))
}

/// 格式化向量为 `[a,b,c]` 形式。
fn format_vector(v: &[f64]) -> String {
    let elems: Vec<String> = v.iter().map(|x| x.to_string()).collect();
    format!("[{}]", elems.join(","))
}

/// 格式化多项式系数向量（升幂存储）为降幂字符串形式 `a+bx+cx^2`。
/// 例如 `[1,2,1]` → `x^2+2x+1`，`[2,3,1]` → `x^2+3x+2`，`[5]` → `5`。
fn format_polynomial(p: &[f64]) -> String {
    if p.is_empty() {
        return "0".to_string();
    }
    let terms: Vec<String> = p
        .iter()
        .enumerate()
        .rev()
        .filter_map(|(i, &coef)| format_polynomial_term(coef, i))
        .collect();
    if terms.is_empty() {
        return "0".to_string();
    }
    join_polynomial_terms(&terms)
}

/// 格式化多项式单项：零系数返回 None（跳过），其他返回 `Some(term)`。
///
/// - i=0：纯常数项 `c`
/// - i=1：一次项 `x` / `-x` / `cx`
/// - i≥2：高次项 `x^i` / `-x^i` / `cx^i`
fn format_polynomial_term(coef: f64, i: usize) -> Option<String> {
    if coef == 0.0 {
        return None;
    }
    let term = match i {
        0 => format!("{}", coef),
        1 => {
            if coef == 1.0 {
                "x".to_string()
            } else if coef == -1.0 {
                "-x".to_string()
            } else {
                format!("{}x", coef)
            }
        }
        _ => {
            if coef == 1.0 {
                format!("x^{}", i)
            } else if coef == -1.0 {
                format!("-x^{}", i)
            } else {
                format!("{}x^{}", coef, i)
            }
        }
    };
    Some(term)
}

/// 合并多项式单项列表为字符串：第一项不加正号前缀，后续正项加 `+`，负项直接拼接（已含 `-`）。
fn join_polynomial_terms(terms: &[String]) -> String {
    let mut result = terms[0].clone();
    for term in &terms[1..] {
        if term.starts_with('-') {
            result.push_str(term);
        } else {
            result.push('+');
            result.push_str(term);
        }
    }
    result
}

/// 格式化复数列表为 `[a+bi,c+di]` 形式。
fn format_complex_list(c: &[(f64, f64)]) -> String {
    let elems: Vec<String> = c.iter().map(|(re, im)| format_complex(*re, *im)).collect();
    format!("[{}]", elems.join(","))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AstNode, BinaryOp};
    /// v015 T016（R-cfg-001 验收）：locales 文案中引用的每个 `--flag` 必须真实存在于
    /// clap 定义——防止错误提示再次指向不存在的旗标（`--timeout` 事故回归门）。
    #[test]
    fn locale_hint_flags_exist_in_clap_definition() {
        use clap::CommandFactory;
        let mut seen: Vec<String> = Vec::new();
        let cmd = Cli::command();
        let long_flags: Vec<String> = cmd
            .get_arguments()
            .filter_map(|a| a.get_long().map(|l| l.to_string()))
            .collect();

        for locale in ["en", "zh"] {
            let path = format!("locales/{locale}.json");
            let raw = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("read {path}: {e}"));
            let value: serde_json::Value = serde_json::from_str(&raw)
                .unwrap_or_else(|e| panic!("parse {path}: {e}"));
            collect_json_strings(&value, &mut seen);
        }

        let flag_re = regex::Regex::new(r"--[a-z][a-z0-9_-]*").unwrap();
        // `--features` 属于 cargo 构建旗标（非 calnexus CLI 旗标），合法出现于特性提示
        let external_flags = ["features"];
        let mut violations: Vec<String> = Vec::new();
        for text in &seen {
            for m in flag_re.find_iter(text) {
                let flag = m.as_str().trim_start_matches('-').to_string();
                if !long_flags.contains(&flag) && !external_flags.contains(&flag.as_str()) {
                    violations.push(format!("{text:?} 引用不存在的 --{flag}"));
                }
            }
        }
        assert!(
            violations.is_empty(),
            "locales 中存在指向不存在旗标的 hint:\n{}",
            violations.join("\n")
        );
    }

    /// 递归收集 JSON 中所有字符串叶子。
    fn collect_json_strings(v: &serde_json::Value, out: &mut Vec<String>) {
        match v {
            serde_json::Value::String(s) => out.push(s.clone()),
            serde_json::Value::Array(a) => a.iter().for_each(|x| collect_json_strings(x, out)),
            serde_json::Value::Object(o) => o.values().for_each(|x| collect_json_strings(x, out)),
            _ => {}
        }
    }



    // ===== v1.1 新增 CLI 标志测试 =====

    #[test]
    fn test_canonicalize_no_fold_basic_addition() {
        // PRD §3.2.4: --canonical "3+2" → "(+ 2 3)"
        let ast = parse("3+2").unwrap();
        let (_, cf) = AstCanonicalizer::canonicalize_no_fold(&ast).unwrap();
        assert_eq!(cf.as_str(), "(+ 2 3)");
    }

    #[test]
    fn test_canonicalize_no_fold_equivalent_expressions() {
        // 3+2 和 2+3 应产生相同的规范形式
        let ast1 = parse("3+2").unwrap();
        let ast2 = parse("2+3").unwrap();
        let (_, cf1) = AstCanonicalizer::canonicalize_no_fold(&ast1).unwrap();
        let (_, cf2) = AstCanonicalizer::canonicalize_no_fold(&ast2).unwrap();
        assert_eq!(cf1.as_str(), cf2.as_str());
        assert_eq!(cf1.as_str(), "(+ 2 3)");
    }

    #[test]
    fn test_canonicalize_no_fold_multiplication() {
        let ast = parse("4*5").unwrap();
        let (_, cf) = AstCanonicalizer::canonicalize_no_fold(&ast).unwrap();
        assert_eq!(cf.as_str(), "(* 4 5)");
    }

    #[test]
    fn test_canonicalize_no_fold_does_not_constant_fold() {
        // 与 canonicalize（折叠版本）对比：canonicalize_no_fold 保留 (+ 2 3)
        let ast = parse("2+3").unwrap();
        let (_, cf_fold) = AstCanonicalizer::canonicalize(&ast).unwrap();
        let (_, cf_no_fold) = AstCanonicalizer::canonicalize_no_fold(&ast).unwrap();
        assert_eq!(cf_fold.as_str(), "5", "folded version");
        assert_eq!(cf_no_fold.as_str(), "(+ 2 3)", "no-fold version");
    }

    #[test]
    fn test_canonicalize_no_fold_preserves_double_neg_elimination() {
        // 2-(-x) → 2+x after double-neg elimination. Use 2--3 syntax (parser-dependent).
        // 使用 -(2+3) 形式：保留外层 neg，内部 (+ 2 3) 排序
        let ast = parse("-(2+3)").unwrap();
        let (_, cf) = AstCanonicalizer::canonicalize_no_fold(&ast).unwrap();
        assert_eq!(cf.as_str(), "(- (+ 2 3))");
    }

    #[test]
    fn test_canonicalize_no_fold_does_not_fold_neg_number() {
        // -5 保留为 (- 5)，不折叠为 -5
        // 由于 parser 可能将 -5 直接解析为 Number(-5)，需构造 -(5) 形式
        let ast = parse("0-5").unwrap();
        let (_, cf) = AstCanonicalizer::canonicalize_no_fold(&ast).unwrap();
        assert_eq!(cf.as_str(), "(- 0 5)");
    }

    #[test]
    fn test_canonicalize_no_fold_function_call() {
        let ast = parse("sin(x)+cos(y)").unwrap();
        let (_, cf) = AstCanonicalizer::canonicalize_no_fold(&ast).unwrap();
        assert_eq!(cf.as_str(), "(+ (sin x) (cos y))");
    }

    #[test]
    fn test_format_canonical_wrapper() {
        use crate::output::format_canonical;
        use crate::CanonicalForm;
        let cf = CanonicalForm::new("(+ 2 3)");
        assert_eq!(format_canonical(&cf), "(+ 2 3)");
    }

    #[test]
    fn test_format_latex_dispatch_scalar() {
        use crate::output::format_latex;
        let r = EvalResult::Scalar(42.0);
        let ast = AstNode::Number(42.0);
        assert_eq!(format_latex(&r, &ast, "42", None), "42");
    }

    #[test]
    fn test_format_latex_dispatch_matrix() {
        use crate::output::format_latex;
        let r = EvalResult::Matrix(vec![vec![1.0, 2.0], vec![3.0, 4.0]]);
        let ast = AstNode::Number(0.0);
        assert_eq!(
            format_latex(&r, &ast, "[[1,2],[3,4]]", None),
            "\\begin{pmatrix}1 & 2 \\\\ 3 & 4\\end{pmatrix}"
        );
    }

    #[test]
    fn test_format_latex_dispatch_symbolic_diff() {
        use crate::output::format_latex;
        let r = EvalResult::Symbolic("2*x".to_string());
        let ast = AstNode::FunctionCall(
            "diff".to_string(),
            vec![
                AstNode::BinaryOp(
                    BinaryOp::Pow,
                    Box::new(AstNode::Variable("x".to_string())),
                    Box::new(AstNode::Number(2.0)),
                ),
                AstNode::Variable("x".to_string()),
            ],
        );
        let s = format_latex(&r, &ast, "diff(x^2,x)", None);
        assert_eq!(s, "\\frac{d}{dx}\\left(x^{2}\\right) = 2 \\cdot x");
    }

    #[test]
    fn test_generate_steps_basic() {
        use crate::output::generate_steps;
        // 步骤必须基于原始 AST（未折叠），否则常量折叠后无步骤可显示
        let ast = parse("(2+9)*7-6").unwrap();
        let ctx = EvalContext::new();
        let steps = generate_steps(&ast, &ctx).unwrap();
        // PRD §4.1.1: 2+9=11 → 11*7=77 → 77-6=71
        assert_eq!(steps, vec!["2+9=11", "11*7=77", "77-6=71"]);
    }

    #[test]
    fn test_format_result_handles_latex_variant() {
        let r = EvalResult::LaTeX("\\alpha".to_string());
        assert_eq!(format_result(&r, None), "\\alpha");
    }

    #[test]
    fn test_format_result_handles_steps_variant() {
        let r = EvalResult::Steps(vec!["1+1=2".to_string(), "2+2=4".to_string()]);
        assert_eq!(format_result(&r, None), "1+1=2\n2+2=4");
    }
}
