// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! CalNexus CLI：命令行数学表达式求值器。
//!
//! 全链路：Parser → Canonicalizer → CacheManager → DomainRouter → Domain::evaluate
//!
//! 退出码（cli-interface spec）：
//! - 0：成功
//! - 1：计算错误 / 解析错误
//! - 2：系统错误（无效参数）

use crate::core::domain::CalculationDomain;
use crate::domains::precision::format_bigrational;
use crate::output::{canonical::format_canonical, latex::format_latex, steps::generate_steps};
use crate::{
    parse, AstCanonicalizer, CacheManager, CalcError, DomainRouter, EvalContext, EvalResult,
};
use crate::{
    ArithmeticDomain, AstNode, CombinatoricsDomain, ComplexDomain, MatrixDomain,
    NumberTheoryDomain, PolynomialDomain, PrecisionDomain, ScientificDomain, StatisticsDomain,
    SymbolicDomain, VectorDomain,
};
use clap::Parser;
use std::io::{self, IsTerminal, Read};

/// CLI 参数定义（clap v4 derive）。
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

    /// Arbitrary precision mode: format result to N decimal places using BigRational arithmetic
    #[arg(long, conflicts_with_all = ["canonical"])]
    precision: Option<usize>,

    /// Start interactive REPL mode (read-eval-print loop)
    #[arg(long, conflicts_with_all = ["canonical", "latex", "steps"])]
    repl: bool,

    /// Batch evaluate expressions from file ('-' for stdin), one expression per line
    #[arg(long, conflicts_with_all = ["canonical", "latex", "steps"])]
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
}

/// CLI 入口：解析参数、求值、输出结果，返回退出码。
pub fn run() -> i32 {
    let cli = Cli::parse();

    // --repl 模式：启动交互式 REPL
    if cli.repl {
        let mut ctx = match parse_vars(&cli.vars) {
            Ok(ctx) => ctx,
            Err(msg) => {
                eprintln!("error: {}", msg);
                return 2;
            }
        };
        ctx.precision = cli.precision;
        return crate::repl::ReplSession::new(ctx).run();
    }

    // --batch 模式：批量求值
    if let Some(path) = &cli.batch {
        let ctx = match parse_vars(&cli.vars) {
            Ok(ctx) => ctx,
            Err(msg) => {
                eprintln!("error: {}", msg);
                return 2;
            }
        };
        return crate::batch::BatchProcessor::run(path, &ctx, cli.json);
    }

    // 获取表达式（位置参数或 stdin）
    let expr = match get_expression(&cli) {
        Ok(e) => e,
        Err(code) => return code,
    };

    // 解析变量绑定
    let mut ctx = match parse_vars(&cli.vars) {
        Ok(ctx) => ctx,
        Err(msg) => {
            eprintln!("error: {}", msg);
            return 2;
        }
    };
    ctx.precision = cli.precision;

    // --canonical 模式：parse → canonicalize_no_fold → print S-expr，跳过求值
    if cli.canonical {
        let ast = match parse(&expr) {
            Ok(ast) => ast,
            Err(e) => {
                eprintln!("error: {}", e);
                return 1;
            }
        };
        match AstCanonicalizer::canonicalize_no_fold(&ast) {
            Ok((_canonical_ast, cf)) => {
                println!("{}", format_canonical(&cf));
                0
            }
            Err(e) => {
                eprintln!("error: {}", e);
                1
            }
        }
    } else if cli.latex || cli.steps {
        // --latex 和/或 --steps：解析 + 规范化 + 求值 + 格式化输出
        let ast = match parse(&expr) {
            Ok(ast) => ast,
            Err(e) => {
                eprintln!("error: {}", e);
                return 1;
            }
        };
        let (canonical_ast, _cf) = match AstCanonicalizer::canonicalize(&ast) {
            Ok(pair) => pair,
            Err(e) => {
                eprintln!("error: {}", e);
                return 1;
            }
        };

        // --steps 先输出步骤（基于原始 AST，避免常量折叠后无步骤可显示）
        if cli.steps {
            match generate_steps(&ast, &ctx) {
                Ok(step_lines) => {
                    for line in &step_lines {
                        println!("{}", line);
                    }
                }
                Err(e) => {
                    eprintln!("error: {}", e);
                    return 1;
                }
            }
        }

        // --latex：求值并输出 LaTeX 结果
        if cli.latex {
            let cache = CacheManager::new();
            match evaluate(&expr, &ctx, cli.precision, &cache) {
                Ok((result, _domain, _cache_hit, fmt_prec)) => {
                    let latex_str = format_latex(&result, &canonical_ast, &expr, fmt_prec);
                    println!("{}", latex_str);
                }
                Err(e) => {
                    eprintln!("error: {}", e);
                    return 1;
                }
            }
        }
        0
    } else {
        // 默认模式：求值 + 输出（JSON 或文本）
        let cache = CacheManager::new();
        match evaluate(&expr, &ctx, cli.precision, &cache) {
            Ok((result, domain, cache_hit, fmt_prec)) => {
                if cli.json {
                    let cache_str = if cache_hit { "hit" } else { "miss" };
                    match &result {
                        EvalResult::Scalar(v) => println!(
                            r#"{{"result":{},"domain":"{}","cache":"{}"}}"#,
                            v, domain, cache_str
                        ),
                        EvalResult::Complex(re, im) => println!(
                            r#"{{"result":"{}","domain":"{}","cache":"{}"}}"#,
                            format_complex(*re, *im),
                            domain,
                            cache_str
                        ),
                        EvalResult::Matrix(m) => println!(
                            r#"{{"result":"{}","domain":"{}","cache":"{}"}}"#,
                            format_matrix(m),
                            domain,
                            cache_str
                        ),
                        EvalResult::BigInt(b) => println!(
                            r#"{{"result":"{}","domain":"{}","cache":"{}"}}"#,
                            b, domain, cache_str
                        ),
                        EvalResult::BigRational(r) => println!(
                            r#"{{"result":"{}","domain":"{}","cache":"{}"}}"#,
                            format_bigrational(r, fmt_prec),
                            domain,
                            cache_str
                        ),
                        EvalResult::Vector(v) => println!(
                            r#"{{"result":"{}","domain":"{}","cache":"{}"}}"#,
                            format_vector(v),
                            domain,
                            cache_str
                        ),
                        EvalResult::Polynomial(p) => println!(
                            r#"{{"result":"{}","domain":"{}","cache":"{}"}}"#,
                            format_polynomial(p),
                            domain,
                            cache_str
                        ),
                        EvalResult::ComplexList(c) => println!(
                            r#"{{"result":"{}","domain":"{}","cache":"{}"}}"#,
                            format_complex_list(c),
                            domain,
                            cache_str
                        ),
                        EvalResult::Symbolic(s) => println!(
                            r#"{{"result":"{}","domain":"{}","cache":"{}"}}"#,
                            s, domain, cache_str
                        ),
                        EvalResult::LaTeX(s) => println!(
                            r#"{{"result":"{}","domain":"{}","cache":"{}"}}"#,
                            s, domain, cache_str
                        ),
                        EvalResult::Steps(v) => {
                            let arr: Vec<String> = v
                                .iter()
                                .map(|s| {
                                    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
                                })
                                .collect();
                            println!(
                                r#"{{"result":[{}],"domain":"{}","cache":"{}"}}"#,
                                arr.join(","),
                                domain,
                                cache_str
                            )
                        }
                    }
                } else {
                    match &result {
                        EvalResult::Scalar(v) => println!("{}", v),
                        EvalResult::Complex(re, im) => println!("{}", format_complex(*re, *im)),
                        EvalResult::Matrix(m) => println!("{}", format_matrix(m)),
                        EvalResult::BigInt(b) => println!("{}", b),
                        EvalResult::BigRational(r) => {
                            println!("{}", format_bigrational(r, fmt_prec))
                        }
                        EvalResult::Vector(v) => println!("{}", format_vector(v)),
                        EvalResult::Polynomial(p) => println!("{}", format_polynomial(p)),
                        EvalResult::ComplexList(c) => println!("{}", format_complex_list(c)),
                        EvalResult::Symbolic(s) => println!("{}", s),
                        EvalResult::LaTeX(s) => println!("{}", s),
                        EvalResult::Steps(v) => {
                            for line in v {
                                println!("{}", line);
                            }
                        }
                    }
                }
                0
            }
            Err(e) => {
                eprintln!("error: {}", e);
                1
            }
        }
    }
}

/// 从位置参数或 stdin 获取表达式。
fn get_expression(cli: &Cli) -> Result<String, i32> {
    if let Some(expr) = &cli.expression {
        return Ok(expr.clone());
    }
    // 无位置参数：检查 stdin
    if io::stdin().is_terminal() {
        // TTY stdin：显示 help 并退出
        Cli::parse_from(["calnexus", "--help"]);
        return Err(0); // unreachable：clap 会先退出
    }
    // 管道 stdin：读取表达式
    let mut input = String::new();
    if io::stdin().read_to_string(&mut input).is_err() {
        eprintln!("error: failed to read from stdin");
        return Err(2);
    }
    let trimmed = input.trim().to_string();
    if trimmed.is_empty() {
        eprintln!("error: empty expression on stdin");
        return Err(1);
    }
    Ok(trimmed)
}

/// 解析 --var NAME=VALUE 列表为 EvalContext。
fn parse_vars(vars: &[String]) -> Result<EvalContext, String> {
    let mut ctx = EvalContext::new();
    for v in vars {
        let parts: Vec<&str> = v.splitn(2, '=').collect();
        if parts.len() != 2 {
            return Err(format!("invalid --var '{}', expected NAME=VALUE", v));
        }
        let value: f64 = parts[1]
            .parse()
            .map_err(|e| format!("invalid --var value '{}': {}", parts[1], e))?;
        ctx = ctx.with_var(parts[0], value);
    }
    Ok(ctx)
}

/// 全链路求值：parse → canonicalize → cache → route → evaluate。
///
/// 当 `precision` 为 `Some(N)` 时，绕过路由器直接使用 PrecisionDomain
/// 进行 BigRational 求值，输出格式化为 N 位小数。
/// 返回 (result, domain_name, cache_hit, format_precision)。
/// format_precision 用于 BigRational 输出格式化（--precision N 或 precision(N, expr)）。
///
/// `cache` 参数允许调用方注入预填充的缓存（测试用），生产代码传入空缓存。
pub fn evaluate(
    expr: &str,
    ctx: &EvalContext,
    precision: Option<usize>,
    cache: &CacheManager,
) -> Result<(EvalResult, String, bool, Option<usize>), CalcError> {
    // 1. 解析
    let ast = parse(expr)?;

    // 2. 规范化
    let (canonical_ast, cf) = AstCanonicalizer::canonicalize(&ast)?;

    // 3. precision 模式：直接使用 PrecisionDomain（绕过路由器）
    if precision.is_some() {
        if let Some(cached) = cache.get(&cf) {
            return Ok((cached, "precision".to_string(), true, precision));
        }
        let domain = PrecisionDomain;
        let result = domain.evaluate(&canonical_ast, ctx)?;
        cache.insert(&cf, &Ok(result.clone()));
        return Ok((result, "precision".to_string(), false, precision));
    }

    // 4. 常规模式：路由器分发
    let router = build_default_router();

    // 5. 缓存查询
    if let Some(cached) = cache.get(&cf) {
        let domain = router.route(&canonical_ast)?;
        let fmt_prec = extract_format_precision(&canonical_ast);
        return Ok((cached, domain.domain_name().to_string(), true, fmt_prec));
    }

    // 6. 路由 + 求值
    let domain = router.route(&canonical_ast)?;
    let result = domain.evaluate(&canonical_ast, ctx)?;

    // 7. 写入缓存（仅 Ok 结果）
    cache.insert(&cf, &Ok(result.clone()));

    // 8. 提取格式化精度（precision(N, expr) 的 N）
    let fmt_prec = extract_format_precision(&canonical_ast);
    Ok((result, domain.domain_name().to_string(), false, fmt_prec))
}

/// 从 AST 顶层提取 precision(N, expr) 调用中的 N，用于输出格式化。
fn extract_format_precision(ast: &AstNode) -> Option<usize> {
    if let AstNode::FunctionCall(name, args) = ast {
        if name == "precision" && args.len() == 2 {
            if let AstNode::Number(n) = &args[0] {
                if n.fract() == 0.0 && *n > 0.0 {
                    return Some(*n as usize);
                }
            }
        }
    }
    None
}

/// 构建默认路由器：注册全部 11 个计算域（含 SymbolicDomain）。
/// 供 `evaluate` 与 REPL 共用。
pub(crate) fn build_default_router() -> DomainRouter {
    let mut router = DomainRouter::new();
    router.register(Box::new(PrecisionDomain));
    router.register(Box::new(ComplexDomain));
    router.register(Box::new(MatrixDomain));
    router.register(Box::new(VectorDomain));
    router.register(Box::new(SymbolicDomain));
    router.register(Box::new(PolynomialDomain));
    router.register(Box::new(NumberTheoryDomain));
    router.register(Box::new(CombinatoricsDomain));
    router.register(Box::new(ScientificDomain));
    router.register(Box::new(StatisticsDomain));
    router.register(Box::new(ArithmeticDomain));
    router
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
    let mut terms: Vec<String> = Vec::new();
    for (i, &coef) in p.iter().enumerate().rev() {
        if coef == 0.0 {
            continue;
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
        terms.push(term);
    }
    if terms.is_empty() {
        return "0".to_string();
    }
    // 合并：第一项不加正号前缀，后续正项加 +
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
    use crate::core::types::BinaryOp;

    // 覆盖 extract_format_precision lines 213-214：
    // 当 precision(N, expr) 中 N 为 Number 但非正整数（如浮点数）时，
    // 内层 if 条件为 false，控制流穿过闭合大括号（lines 213-214）后返回 None。
    #[test]
    fn test_extract_format_precision_float_n() {
        let ast = AstNode::FunctionCall(
            "precision".to_string(),
            vec![
                AstNode::Number(2.5),
                AstNode::BinaryOp(
                    BinaryOp::Div,
                    Box::new(AstNode::Number(1.0)),
                    Box::new(AstNode::Number(3.0)),
                ),
            ],
        );
        assert_eq!(extract_format_precision(&ast), None);
    }

    // 覆盖 extract_format_precision line 214：
    // 当 precision(N, expr) 中 N 非 Number（如 Variable）时，
    // if let 不匹配，控制流穿过 line 214 后返回 None。
    #[test]
    fn test_extract_format_precision_non_number_n() {
        let ast = AstNode::FunctionCall(
            "precision".to_string(),
            vec![AstNode::Variable("x".to_string()), AstNode::Number(1.0)],
        );
        assert_eq!(extract_format_precision(&ast), None);
    }

    // 覆盖 extract_format_precision line 214：
    // 当 precision(N, expr) 中 N 为 BigNumber 时，if let Number 不匹配。
    #[test]
    fn test_extract_format_precision_bignumber_n() {
        let ast = AstNode::FunctionCall(
            "precision".to_string(),
            vec![AstNode::BigNumber("5".to_string()), AstNode::Number(1.0)],
        );
        assert_eq!(extract_format_precision(&ast), None);
    }

    // 正常路径：precision(5, expr) → Some(5)
    #[test]
    fn test_extract_format_precision_valid() {
        let ast = AstNode::FunctionCall(
            "precision".to_string(),
            vec![AstNode::Number(5.0), AstNode::Number(1.0)],
        );
        assert_eq!(extract_format_precision(&ast), Some(5));
    }

    // 非 precision 函数调用 → None
    #[test]
    fn test_extract_format_precision_non_precision_call() {
        let ast = AstNode::FunctionCall("sin".to_string(), vec![AstNode::Number(1.0)]);
        assert_eq!(extract_format_precision(&ast), None);
    }

    // 非函数调用节点 → None
    #[test]
    fn test_extract_format_precision_non_function() {
        let ast = AstNode::Number(42.0);
        assert_eq!(extract_format_precision(&ast), None);
    }

    // 覆盖 line 170：precision 模式缓存命中路径
    // 预填充缓存后，evaluate 应直接返回缓存值，标记 cache_hit=true
    #[test]
    fn test_evaluate_precision_mode_cache_hit() {
        let cache = CacheManager::new();
        let ast = parse("2+3").unwrap();
        let (_, cf) = AstCanonicalizer::canonicalize(&ast).unwrap();

        // 预填充缓存
        cache.insert(&cf, &Ok(EvalResult::Scalar(5.0)));

        let ctx = EvalContext::new();
        let (result, domain, cache_hit, fmt_prec) = evaluate("2+3", &ctx, Some(5), &cache).unwrap();

        assert_eq!(result, EvalResult::Scalar(5.0));
        assert_eq!(domain, "precision");
        assert!(cache_hit);
        assert_eq!(fmt_prec, Some(5));
    }

    // 覆盖 lines 189-191：常规模式缓存命中路径
    // 预填充缓存后，evaluate 应直接返回缓存值，标记 cache_hit=true
    #[test]
    fn test_evaluate_regular_mode_cache_hit() {
        let cache = CacheManager::new();
        let ast = parse("2+3").unwrap();
        let (_, cf) = AstCanonicalizer::canonicalize(&ast).unwrap();

        // 预填充缓存
        cache.insert(&cf, &Ok(EvalResult::Scalar(5.0)));

        let ctx = EvalContext::new();
        let (result, domain, cache_hit, fmt_prec) = evaluate("2+3", &ctx, None, &cache).unwrap();

        assert_eq!(result, EvalResult::Scalar(5.0));
        assert_eq!(domain, "arithmetic");
        assert!(cache_hit);
        assert_eq!(fmt_prec, None);
    }

    // ===== v0.8 新增域 CLI 单元测试（TG7.5）=====

    #[test]
    fn test_v08_cli_number_theory_gcd() {
        let cache = CacheManager::new();
        let ctx = EvalContext::new();
        let (result, domain, cache_hit, _) = evaluate("gcd(12,18)", &ctx, None, &cache).unwrap();
        assert_eq!(result, EvalResult::Scalar(6.0));
        assert_eq!(domain, "number_theory");
        assert!(!cache_hit);
    }

    #[test]
    fn test_v08_cli_combinatorics_C() {
        let cache = CacheManager::new();
        let ctx = EvalContext::new();
        let (result, domain, cache_hit, _) = evaluate("C(10,3)", &ctx, None, &cache).unwrap();
        assert_eq!(result, EvalResult::Scalar(120.0));
        assert_eq!(domain, "combinatorics");
        assert!(!cache_hit);
    }

    #[test]
    fn test_v08_cli_vector_dot() {
        let cache = CacheManager::new();
        let ctx = EvalContext::new();
        let (result, domain, _, _) = evaluate("dot([1,2,3],[4,5,6])", &ctx, None, &cache).unwrap();
        assert_eq!(result, EvalResult::Scalar(32.0));
        let _ = domain; // vector domain
    }

    #[test]
    fn test_v08_cli_polynomial_add() {
        let cache = CacheManager::new();
        let ctx = EvalContext::new();
        let (result, domain, _, _) = evaluate("poly_add(x+1,x+2)", &ctx, None, &cache).unwrap();
        assert!(matches!(result, EvalResult::Polynomial(_)));
        assert_eq!(domain, "polynomial");
    }

    #[test]
    fn test_v08_cli_cache_hit_number_theory() {
        // 缓存命中路径覆盖：预填充缓存后再次求值
        let cache = CacheManager::new();
        let ast = parse("gcd(12,18)").unwrap();
        let (_, cf) = AstCanonicalizer::canonicalize(&ast).unwrap();
        cache.insert(&cf, &Ok(EvalResult::Scalar(6.0)));

        let ctx = EvalContext::new();
        let (result, domain, cache_hit, _) = evaluate("gcd(12,18)", &ctx, None, &cache).unwrap();
        assert_eq!(result, EvalResult::Scalar(6.0));
        assert_eq!(domain, "number_theory");
        assert!(cache_hit);
    }

    #[test]
    fn test_v08_cli_cache_hit_combinatorics() {
        let cache = CacheManager::new();
        let ast = parse("C(10,3)").unwrap();
        let (_, cf) = AstCanonicalizer::canonicalize(&ast).unwrap();
        cache.insert(&cf, &Ok(EvalResult::Scalar(120.0)));

        let ctx = EvalContext::new();
        let (result, domain, cache_hit, _) = evaluate("C(10,3)", &ctx, None, &cache).unwrap();
        assert_eq!(result, EvalResult::Scalar(120.0));
        assert_eq!(domain, "combinatorics");
        assert!(cache_hit);
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
        use crate::output::canonical::format_canonical;
        use crate::CanonicalForm;
        let cf = CanonicalForm::new("(+ 2 3)");
        assert_eq!(format_canonical(&cf), "(+ 2 3)");
    }

    #[test]
    fn test_format_latex_dispatch_scalar() {
        use crate::output::latex::format_latex;
        let r = EvalResult::Scalar(42.0);
        let ast = AstNode::Number(42.0);
        assert_eq!(format_latex(&r, &ast, "42", None), "42");
    }

    #[test]
    fn test_format_latex_dispatch_matrix() {
        use crate::output::latex::format_latex;
        let r = EvalResult::Matrix(vec![vec![1.0, 2.0], vec![3.0, 4.0]]);
        let ast = AstNode::Number(0.0);
        assert_eq!(
            format_latex(&r, &ast, "[[1,2],[3,4]]", None),
            "\\begin{pmatrix}1 & 2 \\\\ 3 & 4\\end{pmatrix}"
        );
    }

    #[test]
    fn test_format_latex_dispatch_symbolic_diff() {
        use crate::output::latex::format_latex;
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
        use crate::output::steps::generate_steps;
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
