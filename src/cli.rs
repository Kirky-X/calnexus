//! CalNexus CLI：命令行数学表达式求值器。
//!
//! 全链路：Parser → Canonicalizer → CacheManager → DomainRouter → Domain::evaluate
//!
//! 退出码（cli-interface spec）：
//! - 0：成功
//! - 1：计算错误 / 解析错误
//! - 2：系统错误（无效参数）

use crate::{ArithmeticDomain, AstNode, CombinatoricsDomain, ComplexDomain, MatrixDomain, NumberTheoryDomain, PolynomialDomain, PrecisionDomain, ScientificDomain, StatisticsDomain, SymbolicDomain, VectorDomain};
use crate::core::domain::CalculationDomain;
use crate::{
    AstCanonicalizer, CacheManager, CalcError, DomainRouter, EvalContext, EvalResult, parse,
};
use crate::domains::precision::format_bigrational;
use clap::Parser;
use std::io::{self, IsTerminal, Read};

/// CLI 参数定义（clap v4 derive）。
#[derive(Parser)]
#[command(name = "calnexus", version, about = "CalNexus: math expression evaluator")]
struct Cli {
    /// Expression to evaluate (reads from stdin if omitted and piped)
    expression: Option<String>,

    /// Variable binding: --var NAME=VALUE (can be repeated)
    #[arg(long = "var")]
    vars: Vec<String>,

    /// Output result as JSON with domain and cache metadata
    #[arg(long)]
    json: bool,

    /// Arbitrary precision mode: format result to N decimal places using BigRational arithmetic
    #[arg(long)]
    precision: Option<usize>,

    /// Start interactive REPL mode (read-eval-print loop)
    #[arg(long)]
    repl: bool,

    /// Batch evaluate expressions from file ('-' for stdin), one expression per line
    #[arg(long)]
    batch: Option<String>,
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

    // 求值
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
                }
            } else {
                match &result {
                    EvalResult::Scalar(v) => println!("{}", v),
                    EvalResult::Complex(re, im) => println!("{}", format_complex(*re, *im)),
                    EvalResult::Matrix(m) => println!("{}", format_matrix(m)),
                    EvalResult::BigInt(b) => println!("{}", b),
                    EvalResult::BigRational(r) => println!("{}", format_bigrational(r, fmt_prec)),
                    EvalResult::Vector(v) => println!("{}", format_vector(v)),
                    EvalResult::Polynomial(p) => println!("{}", format_polynomial(p)),
                    EvalResult::ComplexList(c) => println!("{}", format_complex_list(c)),
                    EvalResult::Symbolic(s) => println!("{}", s),
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
pub(crate) fn evaluate(expr: &str, ctx: &EvalContext, precision: Option<usize>, cache: &CacheManager) -> Result<(EvalResult, String, bool, Option<usize>), CalcError> {
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
            vec![
                AstNode::Variable("x".to_string()),
                AstNode::Number(1.0),
            ],
        );
        assert_eq!(extract_format_precision(&ast), None);
    }

    // 覆盖 extract_format_precision line 214：
    // 当 precision(N, expr) 中 N 为 BigNumber 时，if let Number 不匹配。
    #[test]
    fn test_extract_format_precision_bignumber_n() {
        let ast = AstNode::FunctionCall(
            "precision".to_string(),
            vec![
                AstNode::BigNumber("5".to_string()),
                AstNode::Number(1.0),
            ],
        );
        assert_eq!(extract_format_precision(&ast), None);
    }

    // 正常路径：precision(5, expr) → Some(5)
    #[test]
    fn test_extract_format_precision_valid() {
        let ast = AstNode::FunctionCall(
            "precision".to_string(),
            vec![
                AstNode::Number(5.0),
                AstNode::Number(1.0),
            ],
        );
        assert_eq!(extract_format_precision(&ast), Some(5));
    }

    // 非 precision 函数调用 → None
    #[test]
    fn test_extract_format_precision_non_precision_call() {
        let ast = AstNode::FunctionCall(
            "sin".to_string(),
            vec![AstNode::Number(1.0)],
        );
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
        let (result, domain, cache_hit, fmt_prec) =
            evaluate("2+3", &ctx, Some(5), &cache).unwrap();

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
        let (result, domain, cache_hit, fmt_prec) =
            evaluate("2+3", &ctx, None, &cache).unwrap();

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
        let (result, domain, cache_hit, _) =
            evaluate("gcd(12,18)", &ctx, None, &cache).unwrap();
        assert_eq!(result, EvalResult::Scalar(6.0));
        assert_eq!(domain, "number_theory");
        assert!(!cache_hit);
    }

    #[test]
    fn test_v08_cli_combinatorics_C() {
        let cache = CacheManager::new();
        let ctx = EvalContext::new();
        let (result, domain, cache_hit, _) =
            evaluate("C(10,3)", &ctx, None, &cache).unwrap();
        assert_eq!(result, EvalResult::Scalar(120.0));
        assert_eq!(domain, "combinatorics");
        assert!(!cache_hit);
    }

    #[test]
    fn test_v08_cli_vector_dot() {
        let cache = CacheManager::new();
        let ctx = EvalContext::new();
        let (result, domain, _, _) =
            evaluate("dot([1,2,3],[4,5,6])", &ctx, None, &cache).unwrap();
        assert_eq!(result, EvalResult::Scalar(32.0));
        let _ = domain; // vector domain
    }

    #[test]
    fn test_v08_cli_polynomial_add() {
        let cache = CacheManager::new();
        let ctx = EvalContext::new();
        let (result, domain, _, _) =
            evaluate("poly_add(x+1,x+2)", &ctx, None, &cache).unwrap();
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
        let (result, domain, cache_hit, _) =
            evaluate("gcd(12,18)", &ctx, None, &cache).unwrap();
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
        let (result, domain, cache_hit, _) =
            evaluate("C(10,3)", &ctx, None, &cache).unwrap();
        assert_eq!(result, EvalResult::Scalar(120.0));
        assert_eq!(domain, "combinatorics");
        assert!(cache_hit);
    }
}
