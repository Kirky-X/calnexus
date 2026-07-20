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

/// CLI 参数定义（通过 sdforge::clap 重导出使用 clap v4 derive）。
///
/// 注意：`about` 字段是 clap derive 编译时字面量，无法运行时国际化。
/// 用户可见的本地化消息通过 `--lang` 切换 `I18n` 实例，由 `friendly()`/`tf()` 渲染。
/// `about` 保留英文与默认语言（en）一致；`cli.about` 键供其他场景运行时查询。
#[derive(Parser)]
#[command(
    name = "calnexus",
    version,
    about = "CalNexus: math expression evaluator"
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
    #[arg(long, conflicts_with_all = ["canonical", "latex", "steps"])]
    repl: bool,

    /// Batch evaluate expressions from file ('-' for stdin), one expression per line
    #[arg(long, conflicts_with_all = ["canonical", "latex", "steps", "precision"])]
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

    // --serve-http / --serve-mcp 模式：启动 server（阻塞运行，内部创建 tokio runtime）
    #[cfg(feature = "server")]
    if cli.serve_http || cli.serve_mcp {
        return run_server_mode(&cli);
    }

    // --repl 模式：启动交互式 REPL
    if cli.repl {
        return run_repl_mode(&cli, &i18n);
    }

    // --batch 模式：批量求值
    if let Some(path) = &cli.batch {
        return run_batch_mode(path, &cli, &i18n);
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

    if cli.canonical {
        run_canonical_mode(&expr, &cli, &i18n)
    } else if cli.latex || cli.steps {
        run_latex_steps_mode(&expr, &ctx, &cli, &i18n)
    } else {
        run_default_mode(&expr, &ctx, &cli, &i18n)
    }
}

/// 启动 HTTP/MCP server 模式。
#[cfg(feature = "server")]
fn run_server_mode(cli: &Cli) -> i32 {
    let server_result = if cli.serve_http {
        crate::server::HttpServer::new().run()
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
fn run_repl_mode(cli: &Cli, i18n: &crate::i18n::I18n) -> i32 {
    let mut ctx = match parse_vars(&cli.vars) {
        Ok(ctx) => ctx,
        Err(e) => return handle_error(&e, cli, i18n),
    };
    ctx.precision = cli.precision;
    crate::repl::ReplSession::new(ctx, i18n.clone()).run()
}

/// --batch 模式：解析变量绑定并批量求值。
fn run_batch_mode(path: &str, cli: &Cli, i18n: &crate::i18n::I18n) -> i32 {
    let ctx = match parse_vars(&cli.vars) {
        Ok(ctx) => ctx,
        Err(e) => return handle_error(&e, cli, i18n),
    };
    crate::batch::BatchProcessor::run(path, &ctx, cli.json, i18n)
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
        let cache = CacheManager::new();
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
fn run_default_mode(expr: &str, ctx: &EvalContext, cli: &Cli, i18n: &crate::i18n::I18n) -> i32 {
    let cache = CacheManager::new();
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
    if cli.json {
        // to_json() 已返回 {"error":{...}} 完整结构，无需再包装
        println!("{}", e.to_json());
    } else if cli.explain {
        eprintln!("{}", e.to_explain(i18n));
    } else {
        eprintln!("{}: {}", i18n.t("cli.error_prefix"), e.friendly(i18n));
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
        return Err(CalcError::usage("empty expression on stdin")
            .with_i18n("cli.empty_stdin", vec![]));
    }
    Ok(trimmed)
}

/// 解析 --var NAME=VALUE 列表为 EvalContext。
///
/// i18n_key 通过 `with_i18n` 附加到 CalcError，渲染时由 `handle_error` 传入 i18n 实例。
fn parse_vars(vars: &[String]) -> Result<EvalContext, CalcError> {
    let mut ctx = EvalContext::new();
    for v in vars {
        let parts: Vec<&str> = v.splitn(2, '=').collect();
        if parts.len() != 2 {
            return Err(CalcError::usage(format!(
                "invalid --var '{}', expected NAME=VALUE",
                v
            ))
            .with_i18n(
                "cli.invalid_var",
                vec![("value".to_string(), v.clone())],
            ));
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
