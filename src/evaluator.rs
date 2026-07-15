// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! CalNexus 求值编排层：parse → canonicalize → cache → route → domain::evaluate。
//!
//! 本模块为顶层模块（不受 feature gate），CLI、REPL、batch、HTTP/MCP server 共用。
//! 从 `src/cli.rs` 移出，使 `http`/`mcp` feature 在不启用 `cli` 时也能调用 `evaluate`。

use std::sync::OnceLock;

use crate::core::{
    parse, AstCanonicalizer, AstNode, CacheManager, CalcError, CalculationDomain, CanonicalForm,
    DomainRouter, EvalContext, EvalResult, MAX_PRECISION,
};
use crate::domains::{
    ArithmeticDomain, CombinatoricsDomain, ComplexDomain, MatrixDomain, NumberTheoryDomain,
    PolynomialDomain, PrecisionDomain, ScientificDomain, StatisticsDomain, VectorDomain,
};
use crate::SymbolicDomain;

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
    // 0. 超时检查：ctx.timeout == 0 触发立即超时（design.md §6.3：P0 不实现真正时间追踪，
    //    仅支持显式配置驱动的超时触发；P3 会添加基于 elapsed 的自动超时）
    if ctx.timeout.is_zero() {
        return Err(CalcError::timeout());
    }

    // 0.1 precision 参数上界校验：纵深防御第四层。
    //     server validate() 仅保护 HTTP/MCP 入口，CLI/REPL 直接调用 evaluate 时
    //     不会经过 validate()，必须在此处拦截超大 precision 防止 format_decimal 循环 DoS。
    //     （安全审查 HIGH-1：evaluate 是公共入口，参数校验不能依赖调用方自觉）
    if let Some(p) = precision {
        if p > MAX_PRECISION {
            return Err(CalcError::domain(format!(
                "precision {} exceeds limit {}",
                p, MAX_PRECISION
            )));
        }
    }

    // 1. 解析
    let ast = parse(expr)?;

    // 2. 规范化
    let (canonical_ast, cf) = AstCanonicalizer::canonicalize(&ast)?;

    // 2.1 构建精度感知缓存键：precision 模式与常规模式使用不同键前缀，
    //     避免同一表达式的 BigRational 结果与 Scalar 结果相互污染。
    //     （tiangang SAST CRITICAL + kueiku bug 分析发现）
    let cache_cf = if precision.is_some() {
        CanonicalForm::new(format!("precision:{}", cf.as_str()))
    } else {
        cf.clone()
    };

    // 3. precision 模式：直接使用 PrecisionDomain（绕过路由器）
    if precision.is_some() {
        if let Some(cached) = cache.get(&cache_cf) {
            return Ok((cached, "precision".to_string(), true, precision));
        }
        let domain = PrecisionDomain;
        let result = domain.evaluate(&canonical_ast, ctx)?;
        cache.insert(&cache_cf, &Ok(result.clone()));
        return Ok((result, "precision".to_string(), false, precision));
    }

    // 4. 常规模式：路由器分发
    let router = build_default_router();

    // 5. 缓存查询
    if let Some(cached) = cache.get(&cache_cf) {
        let domain = router.route(&canonical_ast)?;
        let fmt_prec = extract_format_precision(&canonical_ast);
        return Ok((cached, domain.domain_name().to_string(), true, fmt_prec));
    }

    // 6. 路由 + 求值
    let domain = router.route(&canonical_ast)?;
    let result = domain.evaluate(&canonical_ast, ctx)?;

    // 7. 写入缓存（仅 Ok 结果）
    cache.insert(&cache_cf, &Ok(result.clone()));

    // 8. 提取格式化精度（precision(N, expr) 的 N）
    let fmt_prec = extract_format_precision(&canonical_ast);
    Ok((result, domain.domain_name().to_string(), false, fmt_prec))
}

/// 从 AST 顶层提取 precision(N, expr) 调用中的 N，用于输出格式化。
///
/// 安全约束：N > MAX_PRECISION 时返回 `None`（降级为分数格式化），
/// 防止 `format_bigrational` 循环 N 次导致 DoS（tiangang SAST CRITICAL）。
fn extract_format_precision(ast: &AstNode) -> Option<usize> {
    if let AstNode::FunctionCall(name, args) = ast {
        if name == "precision" && args.len() == 2 {
            if let AstNode::Number(n) = &args[0] {
                if n.fract() == 0.0 && *n > 0.0 {
                    let n_usize = *n as usize;
                    // 纵深防御：拒绝超大精度值，防止 format_decimal 循环 DoS
                    if n_usize <= MAX_PRECISION {
                        return Some(n_usize);
                    }
                }
            }
        }
    }
    None
}

/// 构建默认路由器：注册全部 11 个计算域（含 SymbolicDomain）。
/// 供 `evaluate` 与 REPL 共用。进程级缓存（OnceLock），只构建一次，
/// 避免每次请求重复分配 11 个 Box。
pub(crate) fn build_default_router() -> &'static DomainRouter {
    static DEFAULT_ROUTER: OnceLock<DomainRouter> = OnceLock::new();
    DEFAULT_ROUTER.get_or_init(|| {
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
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{BinaryOp, EvalContext};

    // 覆盖 extract_format_precision：当 precision(N, expr) 中 N 为 Number 但非正整数（如浮点数）时，
    // 内层 if 条件为 false，返回 None。
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

    // 覆盖 extract_format_precision：当 precision(N, expr) 中 N 非 Number（如 Variable）时，
    // if let 不匹配，返回 None。
    #[test]
    fn test_extract_format_precision_non_number_n() {
        let ast = AstNode::FunctionCall(
            "precision".to_string(),
            vec![AstNode::Variable("x".to_string()), AstNode::Number(1.0)],
        );
        assert_eq!(extract_format_precision(&ast), None);
    }

    // 覆盖 extract_format_precision：当 precision(N, expr) 中 N 为 BigNumber 时，if let Number 不匹配。
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

    // precision 模式缓存命中路径：首次调用 miss→计算并缓存，第二次调用 hit→返回缓存值。
    // 验证修复后的行为：precision 模式使用 `precision:` 前缀键，与常规模式隔离，
    // 避免 BigRational 结果与 Scalar 结果相互污染（kueiku CRITICAL）。
    #[test]
    fn test_evaluate_precision_mode_cache_hit() {
        let cache = CacheManager::new();
        let ctx = EvalContext::new();

        // 首次调用：miss → 计算并缓存到 precision: 前缀键
        let (result1, domain1, cache_hit1, fmt_prec1) =
            evaluate("2+3", &ctx, Some(5), &cache).unwrap();
        assert_eq!(result1, EvalResult::BigInt(num_bigint::BigInt::from(5)));
        assert_eq!(domain1, "precision");
        assert!(!cache_hit1);
        assert_eq!(fmt_prec1, Some(5));

        // 第二次调用：hit → 返回缓存值
        let (result2, domain2, cache_hit2, fmt_prec2) =
            evaluate("2+3", &ctx, Some(5), &cache).unwrap();
        assert_eq!(result2, result1);
        assert_eq!(domain2, "precision");
        assert!(cache_hit2);
        assert_eq!(fmt_prec2, Some(5));
    }

    // 缓存键隔离验证：常规模式预填充 Scalar 后，precision 模式不应命中该缓存。
    // 防止 `evaluate("2+3", None)` 缓存 Scalar(5.0) 后，
    // `evaluate("2+3", Some(5))` 错误返回 Scalar 而非 BigRational（kueiku CRITICAL 回归测试）。
    #[test]
    fn test_evaluate_precision_mode_does_not_pollute_regular_cache() {
        let cache = CacheManager::new();
        let ast = parse("2+3").unwrap();
        let (_, cf) = AstCanonicalizer::canonicalize(&ast).unwrap();

        // 常规模式预填充 Scalar(5.0)
        cache.insert(&cf, &Ok(EvalResult::Scalar(5.0)));

        let ctx = EvalContext::new();
        // precision 模式不应命中常规模式缓存，应重新计算返回 BigRational/BigInt
        let (result, domain, cache_hit, fmt_prec) = evaluate("2+3", &ctx, Some(5), &cache).unwrap();
        assert_ne!(result, EvalResult::Scalar(5.0));
        assert_eq!(result, EvalResult::BigInt(num_bigint::BigInt::from(5)));
        assert_eq!(domain, "precision");
        assert!(!cache_hit);
        assert_eq!(fmt_prec, Some(5));
    }

    // precision(N, expr) 表达式语法 DoS 防护回归测试（tiangang CRITICAL）。
    // 当 N 超过 MAX_PRECISION 时，extract_format_precision 返回 None（降级），
    // extract_precision_value 返回 Err（拒绝求值），防止 format_decimal 循环 DoS。
    #[test]
    fn test_evaluate_precision_bypass_dos_rejected() {
        let cache = CacheManager::new();
        let ctx = EvalContext::new();
        // N = MAX_PRECISION + 1，应被 extract_precision_value 拒绝
        let huge_n = MAX_PRECISION + 1;
        let expr = format!("precision({}, 1/3)", huge_n);
        let result = evaluate(&expr, &ctx, None, &cache);
        assert!(
            result.is_err(),
            "precision N > MAX_PRECISION must be rejected"
        );
    }

    // precision 参数本身 DoS 防护回归测试（安全审查 HIGH-1）。
    // evaluate 是公共入口，CLI/REPL 直接调用时不经过 server validate()，
    // 必须在 evaluate 入口拦截 precision > MAX_PRECISION，防止 format_decimal 循环 DoS。
    #[test]
    fn test_evaluate_precision_param_exceeds_limit_rejected() {
        let cache = CacheManager::new();
        let ctx = EvalContext::new();
        let result = evaluate("1/3", &ctx, Some(MAX_PRECISION + 1), &cache);
        assert!(
            result.is_err(),
            "precision param > MAX_PRECISION must be rejected"
        );
    }

    // 常规模式缓存命中路径：预填充缓存后，evaluate 应直接返回缓存值
    #[test]
    fn test_evaluate_regular_mode_cache_hit() {
        let cache = CacheManager::new();
        let ast = parse("2+3").unwrap();
        let (_, cf) = AstCanonicalizer::canonicalize(&ast).unwrap();

        cache.insert(&cf, &Ok(EvalResult::Scalar(5.0)));

        let ctx = EvalContext::new();
        let (result, domain, cache_hit, fmt_prec) = evaluate("2+3", &ctx, None, &cache).unwrap();

        assert_eq!(result, EvalResult::Scalar(5.0));
        assert_eq!(domain, "arithmetic");
        assert!(cache_hit);
        assert_eq!(fmt_prec, None);
    }

    // v0.8 域 CLI 单元测试
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
        let _ = domain;
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
}
