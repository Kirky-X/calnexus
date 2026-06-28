//! CalNexus CLI：命令行数学表达式求值器。
//!
//! 全链路：Parser → Canonicalizer → CacheManager → DomainRouter → Domain::evaluate
//!
//! 退出码（cli-interface spec）：
//! - 0：成功
//! - 1：计算错误 / 解析错误
//! - 2：系统错误（无效参数）

use crate::{ArithmeticDomain, ComplexDomain, MatrixDomain, ScientificDomain, StatisticsDomain};
use crate::{
    AstCanonicalizer, CacheManager, CalcError, DomainRouter, EvalContext, EvalResult, parse,
};
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
}

/// CLI 入口：解析参数、求值、输出结果，返回退出码。
pub fn run() -> i32 {
    let cli = Cli::parse();

    // 获取表达式（位置参数或 stdin）
    let expr = match get_expression(&cli) {
        Ok(e) => e,
        Err(code) => return code,
    };

    // 解析变量绑定
    let ctx = match parse_vars(&cli.vars) {
        Ok(ctx) => ctx,
        Err(msg) => {
            eprintln!("error: {}", msg);
            return 2;
        }
    };

    // 求值
    match evaluate(&expr, &ctx) {
        Ok((result, domain, cache_hit)) => {
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
                }
            } else {
                match &result {
                    EvalResult::Scalar(v) => println!("{}", v),
                    EvalResult::Complex(re, im) => println!("{}", format_complex(*re, *im)),
                    EvalResult::Matrix(m) => println!("{}", format_matrix(m)),
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
/// 返回 (result, domain_name, cache_hit)。
fn evaluate(expr: &str, ctx: &EvalContext) -> Result<(EvalResult, String, bool), CalcError> {
    // 1. 解析
    let ast = parse(expr)?;

    // 2. 规范化
    let (canonical_ast, cf) = AstCanonicalizer::canonicalize(&ast)?;

    // 3. 初始化缓存与路由器
    let cache = CacheManager::new();
    let mut router = DomainRouter::new();
    router.register(Box::new(ComplexDomain));
    router.register(Box::new(MatrixDomain));
    router.register(Box::new(ScientificDomain));
    router.register(Box::new(StatisticsDomain));
    router.register(Box::new(ArithmeticDomain));

    // 4. 缓存查询
    if let Some(cached) = cache.get(&cf) {
        let domain = router.route(&canonical_ast)?;
        return Ok((cached, domain.domain_name().to_string(), true));
    }

    // 5. 路由 + 求值
    let domain = router.route(&canonical_ast)?;
    let result = domain.evaluate(&canonical_ast, ctx)?;

    // 6. 写入缓存（仅 Ok 结果）
    cache.insert(&cf, &Ok(result.clone()));

    Ok((result, domain.domain_name().to_string(), false))
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
