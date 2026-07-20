// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! CalNexus 求值编排层：parse → canonicalize → cache → route → domain::evaluate。
//!
//! 本模块为顶层模块（不受 feature gate），CLI、REPL、batch、HTTP/MCP server 共用。
//! 从 `src/cli.rs` 移出，使 `http`/`mcp` feature 在不启用 `cli` 时也能调用 `evaluate`。

use std::time::{Duration, Instant};

use crate::core::{
    parse, AstCanonicalizer, AstNode, CacheManager, CalcError, CanonicalForm, EvalContext,
    EvalResult, MAX_PRECISION,
};
use crate::domains::{build_default_router, build_precision_domain};

/// 全链路求值：parse → canonicalize → cache → route → evaluate。
///
/// 当 `precision` 为 `Some(N)` 时，绕过路由器直接使用 PrecisionDomain
/// 进行 BigRational 求值，输出格式化为 N 位小数。
/// 返回 (result, domain_name, cache_hit, format_precision)。
/// format_precision 用于 BigRational 输出格式化（--precision N 或 precision(N, expr)）。
///
/// `cache` 参数允许调用方注入预填充的缓存（测试用），生产代码传入空缓存。
///
/// 超时策略（安全审查 CRITICAL 修复，原 design.md §6.3 P3 延后已撤销）：
/// - `ctx.timeout == 0`：立即超时（显式触发）
/// - `ctx.timeout > 0`：记录起始时间，在关键节点（parse/canonicalize/domain::evaluate 前后）
///   检查 `elapsed > timeout`，超时返回 `CalcError::timeout()`。
///   这是第二道防线，已知 DoS 向量（factorial/pow/precision）已有专门常量约束，
///   elapsed 追踪用于捕获未知的累积慢操作。
///
/// T020 重构（cyc=25 → ≤15）：拆分为 4 个职责单一的函数：
/// - `validate_inputs`：timeout=0 + precision 上界校验
/// - `build_cache_key`：精度感知缓存键构建
/// - `eval_precision_mode`：precision 模式（绕过路由器）
/// - `eval_regular_mode`：常规模式（路由器分发）
pub fn evaluate(
    expr: &str,
    ctx: &EvalContext,
    precision: Option<usize>,
    cache: &CacheManager,
) -> Result<(EvalResult, String, bool, Option<usize>), CalcError> {
    let start = Instant::now();
    // 阶段 1: 输入校验（timeout=0 + precision 上界）
    validate_inputs(ctx, precision)?;

    // 阶段 2: parse + canonicalize + 超时检查
    let ast = parse(expr)?;
    check_elapsed(start, ctx.timeout)?;
    let (canonical_ast, cf) = AstCanonicalizer::canonicalize(&ast)?;
    check_elapsed(start, ctx.timeout)?;
    let cache_cf = build_cache_key(&cf, ctx, precision);

    // 阶段 3: 按 precision 模式分发
    if precision.is_some() {
        eval_precision_mode(&canonical_ast, ctx, cache, &cache_cf, precision, start)
    } else {
        eval_regular_mode(&canonical_ast, ctx, cache, &cache_cf, start)
    }
}

/// 输入校验：timeout=0 立即超时 + precision 参数上界检查。
///
/// 安全审查 HIGH-1：evaluate 是公共入口，CLI/REPL 直接调用时不经过 server validate()，
/// 必须在此处拦截超大 precision 防止 format_decimal 循环 DoS。
fn validate_inputs(ctx: &EvalContext, precision: Option<usize>) -> Result<(), CalcError> {
    if ctx.timeout.is_zero() {
        return Err(CalcError::timeout());
    }
    if let Some(p) = precision {
        if p > MAX_PRECISION {
            return Err(CalcError::domain(format!(
                "precision {} exceeds limit {}",
                p, MAX_PRECISION
            ))
            .with_i18n(
                "msg.core.precision_exceeds_limit",
                vec![
                    ("precision".to_string(), p.to_string()),
                    ("max".to_string(), MAX_PRECISION.to_string()),
                ],
            ));
        }
    }
    Ok(())
}

/// 构建精度感知 + 上下文感知缓存键。
///
/// 缓存键组成部分：
/// 1. **CanonicalForm**：表达式规范化形式（S-表达式）
/// 2. **precision 模式前缀**：`precision:` 前缀避免 BigRational 与 Scalar 污染
///    （tiangang SAST CRITICAL + kueiku bug 分析）
/// 3. **EvalContext.vars 哈希**：按 key 字典序排序后 BLAKE3 哈希
///    （BUG-C-001 修复：原实现仅基于 CanonicalForm + precision，
///    未包含 vars，导致 REPL 中 `:let x=1; x` → 1.0 后再 `:let x=2; x`
///    仍错误命中缓存返回 1.0）
/// 4. **EvalContext.timeout**：不同 timeout 应产生不同 cache 键
///
/// # 排序与序列化策略
///
/// vars 按 key 字典序排序后逐项序列化，每项格式为 `key\0value_bits\0`。
/// - 用 `\0` 分隔符防止 `("a", 12)` 与 `("a1", 2.0)` 产生相同序列化结果
/// - 用 `f64::to_bits()` 而非 `to_string()`，避免 `-0.0` 与 `0.0` 在
///   `to_string()` 中都为 `"0"` 但 bits 不同导致语义不一致
fn build_cache_key(
    cf: &CanonicalForm,
    ctx: &EvalContext,
    precision: Option<usize>,
) -> CanonicalForm {
    let base = if precision.is_some() {
        format!("precision:{}", cf.as_str())
    } else {
        cf.as_str().to_string()
    };

    // 混入 vars 哈希：按 key 字典序排序后 BLAKE3 哈希
    let mut vars_input: Vec<u8> = Vec::new();
    let mut sorted_vars: Vec<_> = ctx.vars.iter().collect();
    sorted_vars.sort_by_key(|(k, _)| *k);
    for (k, v) in sorted_vars {
        vars_input.extend_from_slice(k.as_bytes());
        vars_input.push(0); // key/value 分隔符，防止边界混淆
        vars_input.extend_from_slice(&v.to_bits().to_le_bytes());
        vars_input.push(0); // 项分隔符，防止相邻项边界混淆
    }
    let vars_hash_hex = blake3::hash(&vars_input).to_hex();

    // 混入 timeout（as_nanos 返回 u128，覆盖所有实际 timeout 范围）
    let timeout_nanos = ctx.timeout.as_nanos();

    CanonicalForm::new(format!(
        "{}|vars={}|timeout={}",
        base, vars_hash_hex, timeout_nanos
    ))
}

/// precision 模式：直接使用 PrecisionDomain，绕过路由器。
fn eval_precision_mode(
    canonical_ast: &AstNode,
    ctx: &EvalContext,
    cache: &CacheManager,
    cache_cf: &CanonicalForm,
    precision: Option<usize>,
    start: Instant,
) -> Result<(EvalResult, String, bool, Option<usize>), CalcError> {
    // 缓存命中
    if let Some(cached) = cache.get(cache_cf) {
        return Ok((cached, "precision".to_string(), true, precision));
    }
    // 缓存未命中：求值 + 超时检查
    let domain = build_precision_domain();
    check_elapsed(start, ctx.timeout)?;
    let result = domain.evaluate(canonical_ast, ctx)?;
    check_elapsed(start, ctx.timeout)?;
    cache.insert(cache_cf, &Ok(result.clone()));
    Ok((result, "precision".to_string(), false, precision))
}

/// 常规模式：路由器分发 + 缓存查询 + 格式化精度提取。
fn eval_regular_mode(
    canonical_ast: &AstNode,
    ctx: &EvalContext,
    cache: &CacheManager,
    cache_cf: &CanonicalForm,
    start: Instant,
) -> Result<(EvalResult, String, bool, Option<usize>), CalcError> {
    let router = build_default_router();

    // 缓存命中
    if let Some(cached) = cache.get(cache_cf) {
        let domain = router.route(canonical_ast)?;
        let fmt_prec = extract_format_precision(canonical_ast);
        return Ok((cached, domain.domain_name().to_string(), true, fmt_prec));
    }

    // 缓存未命中：路由 + 求值 + 超时检查
    let domain = router.route(canonical_ast)?;
    check_elapsed(start, ctx.timeout)?;
    let result = domain.evaluate(canonical_ast, ctx)?;
    check_elapsed(start, ctx.timeout)?;
    cache.insert(cache_cf, &Ok(result.clone()));
    let fmt_prec = extract_format_precision(canonical_ast);
    Ok((result, domain.domain_name().to_string(), false, fmt_prec))
}

/// 检查是否已超时，超时则返回 `CalcError::timeout()`。
///
/// 辅助函数，避免在 `evaluate` 中重复 `if start.elapsed() > timeout` 模板。
/// `Instant::elapsed()` 开销 ~20ns，关键节点检查对性能无显著影响。
fn check_elapsed(start: Instant, timeout: Duration) -> Result<(), CalcError> {
    if start.elapsed() > timeout {
        return Err(CalcError::timeout());
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{BinaryOp, ErrorKind, EvalContext};

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
    //
    // BUG-C-001 修复后，缓存键需经 build_cache_key 混入 ctx.vars 哈希与 timeout，
    // 故预填充时也必须用 build_cache_key 构造正确键。
    #[test]
    fn test_evaluate_regular_mode_cache_hit() {
        let cache = CacheManager::new();
        let ast = parse("2+3").unwrap();
        let (_, cf) = AstCanonicalizer::canonicalize(&ast).unwrap();

        let ctx = EvalContext::new();
        let cache_cf = build_cache_key(&cf, &ctx, None);
        cache.insert(&cache_cf, &Ok(EvalResult::Scalar(5.0)));

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

        let ctx = EvalContext::new();
        // BUG-C-001: 缓存键需经 build_cache_key 混入 ctx.vars 哈希与 timeout
        let cache_cf = build_cache_key(&cf, &ctx, None);
        cache.insert(&cache_cf, &Ok(EvalResult::Scalar(6.0)));

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

        let ctx = EvalContext::new();
        // BUG-C-001: 缓存键需经 build_cache_key 混入 ctx.vars 哈希与 timeout
        let cache_cf = build_cache_key(&cf, &ctx, None);
        cache.insert(&cache_cf, &Ok(EvalResult::Scalar(120.0)));

        let (result, domain, cache_hit, _) = evaluate("C(10,3)", &ctx, None, &cache).unwrap();
        assert_eq!(result, EvalResult::Scalar(120.0));
        assert_eq!(domain, "combinatorics");
        assert!(cache_hit);
    }

    // ===== timeout elapsed 追踪测试（安全审查 CRITICAL 修复）=====

    /// timeout=0 应立即超时（显式触发，已有逻辑，回归测试）。
    #[test]
    fn test_evaluate_timeout_zero_immediate() {
        let cache = CacheManager::new();
        let mut ctx = EvalContext::new();
        ctx.timeout = Duration::ZERO;
        let result = evaluate("1+1", &ctx, None, &cache);
        assert!(
            matches!(result, Err(ref e) if e.kind == ErrorKind::Timeout),
            "timeout=0 should trigger immediate timeout, got {:?}",
            result
        );
    }

    /// timeout elapsed 追踪：设置极小 timeout（非 zero），关键节点应检测到超时。
    ///
    /// 安全审查 CRITICAL：原 `evaluator.rs:36-38` 仅入口检查 `is_zero()`，
    /// 计算期间不强制超时。本测试验证 elapsed 追踪已实现。
    ///
    /// 策略：设置 timeout=1ns，sleep 1ms 确保时间过期，然后调用 evaluate。
    /// 入口检查 `is_zero()` 不触发（1ns != 0），但关键节点检查 `elapsed > 1ns` 应触发。
    #[test]
    fn test_evaluate_timeout_elapsed_triggers() {
        let cache = CacheManager::new();
        let mut ctx = EvalContext::new();
        ctx.timeout = Duration::from_nanos(1);
        // 确保时间过期（1ms >> 1ns）
        std::thread::sleep(Duration::from_millis(1));
        let result = evaluate("1+1", &ctx, None, &cache);
        assert!(
            matches!(result, Err(ref e) if e.kind == ErrorKind::Timeout),
            "should timeout (elapsed > 1ns), got {:?}",
            result
        );
    }

    /// 正常 timeout 不应误触发：默认 5s 超时下，简单表达式应成功。
    ///
    /// 验证 elapsed 检查不会误拦合法计算。
    #[test]
    fn test_evaluate_timeout_normal_succeeds() {
        let cache = CacheManager::new();
        let ctx = EvalContext::new(); // 默认 timeout=5s
        let result = evaluate("1+1", &ctx, None, &cache);
        assert!(
            result.is_ok(),
            "normal eval should succeed, got {:?}",
            result
        );
    }

    /// precision 模式下的 timeout elapsed 追踪。
    ///
    /// 验证 precision 模式也受 elapsed 检查保护。
    #[test]
    fn test_evaluate_timeout_elapsed_precision_mode() {
        let cache = CacheManager::new();
        let mut ctx = EvalContext::new();
        ctx.timeout = Duration::from_nanos(1);
        std::thread::sleep(Duration::from_millis(1));
        let result = evaluate("1+1", &ctx, Some(5), &cache);
        assert!(
            matches!(result, Err(ref e) if e.kind == ErrorKind::Timeout),
            "precision mode should also timeout, got {:?}",
            result
        );
    }

    /// `check_elapsed` 辅助函数单元测试：未超时返回 Ok。
    #[test]
    fn test_check_elapsed_ok_when_not_expired() {
        let start = Instant::now();
        let timeout = Duration::from_secs(60);
        assert!(check_elapsed(start, timeout).is_ok());
    }

    /// `check_elapsed` 辅助函数单元测试：已超时返回 Timeout 错误。
    #[test]
    fn test_check_elapsed_err_when_expired() {
        let start = Instant::now();
        let timeout = Duration::from_nanos(1);
        std::thread::sleep(Duration::from_millis(1));
        let result = check_elapsed(start, timeout);
        assert!(
            matches!(result, Err(ref e) if e.kind == ErrorKind::Timeout),
            "expired should return Timeout error, got {:?}",
            result
        );
    }

    // ===== T019: evaluate 三阶段分发回归测试（Phase 6 Red）=====
    //
    // 目的：重构 evaluate（cyc=25 → ≤15）前后行为不变。
    // 覆盖三阶段：
    //   1. 输入校验（timeout=0 / precision > MAX / parse 错误）
    //   2. 执行分发（precision 模式 miss+hit / 常规模式 miss+hit / 多域路由）
    //   3. 输出格式化（format_precision 提取 / cache 写入）
    #[test]
    fn test_evaluate_three_phases() {
        // ----- 阶段 1: 输入校验 -----
        // 1a. timeout=0 → 立即超时
        {
            let cache = CacheManager::new();
            let mut ctx = EvalContext::new();
            ctx.timeout = Duration::ZERO;
            let r = evaluate("1+1", &ctx, None, &cache);
            assert!(matches!(r, Err(ref e) if e.kind == ErrorKind::Timeout));
        }
        // 1b. precision > MAX_PRECISION → 拒绝
        {
            let cache = CacheManager::new();
            let ctx = EvalContext::new();
            let r = evaluate("1/3", &ctx, Some(MAX_PRECISION + 1), &cache);
            assert!(r.is_err());
        }
        // 1c. parse 错误传播
        {
            let cache = CacheManager::new();
            let ctx = EvalContext::new();
            let r = evaluate("(1+2", &ctx, None, &cache);
            assert!(r.is_err());
        }

        // ----- 阶段 2: 执行分发 -----
        // 2a. precision 模式 miss → 计算并缓存
        {
            let cache = CacheManager::new();
            let ctx = EvalContext::new();
            let (r, domain, hit, fmt) = evaluate("2+3", &ctx, Some(5), &cache).unwrap();
            assert_eq!(r, EvalResult::BigInt(num_bigint::BigInt::from(5)));
            assert_eq!(domain, "precision");
            assert!(!hit);
            assert_eq!(fmt, Some(5));
        }
        // 2b. precision 模式 hit → 返回缓存
        {
            let cache = CacheManager::new();
            let ctx = EvalContext::new();
            let _ = evaluate("2+3", &ctx, Some(5), &cache).unwrap();
            let (r, domain, hit, fmt) = evaluate("2+3", &ctx, Some(5), &cache).unwrap();
            assert_eq!(r, EvalResult::BigInt(num_bigint::BigInt::from(5)));
            assert_eq!(domain, "precision");
            assert!(hit);
            assert_eq!(fmt, Some(5));
        }
        // 2c. 常规模式 miss → 路由到 arithmetic
        {
            let cache = CacheManager::new();
            let ctx = EvalContext::new();
            let (r, domain, hit, fmt) = evaluate("2+3", &ctx, None, &cache).unwrap();
            assert_eq!(r, EvalResult::Scalar(5.0));
            assert_eq!(domain, "arithmetic");
            assert!(!hit);
            assert_eq!(fmt, None);
        }
        // 2d. 常规模式 hit → 返回缓存
        {
            let cache = CacheManager::new();
            let ctx = EvalContext::new();
            let _ = evaluate("2+3", &ctx, None, &cache).unwrap();
            let (r, domain, hit, fmt) = evaluate("2+3", &ctx, None, &cache).unwrap();
            assert_eq!(r, EvalResult::Scalar(5.0));
            assert_eq!(domain, "arithmetic");
            assert!(hit);
            assert_eq!(fmt, None);
        }
        // 2e. 多域路由：number_theory
        {
            let cache = CacheManager::new();
            let ctx = EvalContext::new();
            let (r, domain, _, _) = evaluate("gcd(12,18)", &ctx, None, &cache).unwrap();
            assert_eq!(r, EvalResult::Scalar(6.0));
            assert_eq!(domain, "number_theory");
        }
        // 2f. 多域路由：combinatorics
        {
            let cache = CacheManager::new();
            let ctx = EvalContext::new();
            let (r, domain, _, _) = evaluate("C(10,3)", &ctx, None, &cache).unwrap();
            assert_eq!(r, EvalResult::Scalar(120.0));
            assert_eq!(domain, "combinatorics");
        }

        // ----- 阶段 3: 输出格式化 -----
        // 3a. precision(N, expr) → fmt_prec = Some(N)
        {
            let cache = CacheManager::new();
            let ctx = EvalContext::new();
            let (r, domain, _, fmt) = evaluate("precision(5, 1/3)", &ctx, None, &cache).unwrap();
            let _ = r;
            let _ = domain;
            assert_eq!(fmt, Some(5));
        }
        // 3b. 常规表达式 → fmt_prec = None
        {
            let cache = CacheManager::new();
            let ctx = EvalContext::new();
            let (_, _, _, fmt) = evaluate("sin(0)", &ctx, None, &cache).unwrap();
            assert_eq!(fmt, None);
        }
        // 3c. 缓存写入验证：miss 后第二次应 hit
        {
            let cache = CacheManager::new();
            let ctx = EvalContext::new();
            let (_, _, hit1, _) = evaluate("6*7", &ctx, None, &cache).unwrap();
            assert!(!hit1);
            let (_, _, hit2, _) = evaluate("6*7", &ctx, None, &cache).unwrap();
            assert!(hit2);
        }
    }

    // ===== BUG-C-001 回归测试：cache 键必须包含 EvalContext.vars 与 timeout =====
    //
    // 背景：原 build_cache_key 仅基于 CanonicalForm + precision，未包含 EvalContext.vars。
    // REPL 中 `:let x=1; x` → 1.0，再 `:let x=2; x` 仍返回 1.0（cache 命中错误结果）。
    // 修复后 build_cache_key 混入 vars 的 BLAKE3 哈希（按 key 字典序排序）和 timeout。
    //
    // 测试 1：相同表达式、不同 vars 值必须产生不同 cache 键，不应互相命中。
    #[test]
    fn test_cache_key_includes_vars_different_values_do_not_collide() {
        let cache = CacheManager::new();
        let ctx1 = EvalContext::new().with_var("x", 1.0);
        let ctx2 = EvalContext::new().with_var("x", 2.0);

        // 第一次：x=1 → 计算 1.0，未命中
        let (r1, _, hit1, _) = evaluate("x", &ctx1, None, &cache).unwrap();
        assert_eq!(r1, EvalResult::Scalar(1.0));
        assert!(!hit1, "first call must miss");

        // 第二次：x=2 → 必须重新计算 2.0，不应误命中 x=1 的缓存
        let (r2, _, hit2, _) = evaluate("x", &ctx2, None, &cache).unwrap();
        assert_eq!(
            r2,
            EvalResult::Scalar(2.0),
            "different vars must not return cached value from other vars"
        );
        assert!(!hit2, "different vars must not hit cache");
    }

    // 测试 2：相同表达式、相同 vars、不同 timeout 必须产生不同 cache 键。
    #[test]
    fn test_cache_key_includes_timeout_different_values_do_not_collide() {
        let cf = CanonicalForm::new("(+ 2 3)");
        let ctx1 = EvalContext::new();
        let mut ctx2 = EvalContext::new();
        ctx2.timeout = Duration::from_secs(10);

        let key1 = build_cache_key(&cf, &ctx1, None);
        let key2 = build_cache_key(&cf, &ctx2, None);
        assert_ne!(
            key1, key2,
            "different timeout must produce different cache key"
        );
    }

    // 测试 3：相同表达式、不同 vars 必须产生不同 cache 键（直接测试 build_cache_key）。
    #[test]
    fn test_build_cache_key_includes_vars() {
        let cf = CanonicalForm::new("(+ 2 3)");
        let ctx_empty = EvalContext::new();
        let ctx_x1 = EvalContext::new().with_var("x", 1.0);
        let ctx_x2 = EvalContext::new().with_var("x", 2.0);
        let ctx_y = EvalContext::new().with_var("y", 1.0);

        let key_empty = build_cache_key(&cf, &ctx_empty, None);
        let key_x1 = build_cache_key(&cf, &ctx_x1, None);
        let key_x2 = build_cache_key(&cf, &ctx_x2, None);
        let key_y = build_cache_key(&cf, &ctx_y, None);

        // 任意两个不同 vars 状态必须产生不同 cache 键
        assert_ne!(key_empty, key_x1, "empty vars vs vars=x:1 must differ");
        assert_ne!(key_x1, key_x2, "vars x:1 vs x:2 must differ");
        assert_ne!(key_x1, key_y, "vars x:1 vs y:1 must differ");

        // 相同 ctx 应产生相同 cache 键（确定性）
        let key_x1_again = build_cache_key(&cf, &ctx_x1, None);
        assert_eq!(key_x1, key_x1_again, "same ctx must produce same key");
    }

    // 测试 4：precision 模式前缀仍然生效（与 BUG-C-001 修复共存）。
    #[test]
    fn test_build_cache_key_precision_prefix_preserved() {
        let cf = CanonicalForm::new("(+ 2 3)");
        let ctx = EvalContext::new();
        let key_reg = build_cache_key(&cf, &ctx, None);
        let key_prec = build_cache_key(&cf, &ctx, Some(5));
        assert_ne!(
            key_reg, key_prec,
            "precision mode key must differ from regular mode"
        );
        assert!(
            key_prec.as_str().contains("precision:"),
            "precision mode key must contain precision: prefix, got {}",
            key_prec
        );
    }

    // ===== BUG-C-002 回归测试：precision(N, 1/3) 必须返回 N 位精确的 1/3 =====
    //
    // 背景：canonicalizer.transform_function 对所有函数参数做常量折叠，
    // 导致 precision(20, 1/3) 的 args[1]=1/3 被折叠为 f64 的 0.333...，
    // Precision 域收到的是 f64 近似值而非精确分数 1/3。
    // 修复后 transform_function 对 name=="precision" 的 args[1] 跳过常量折叠，
    // 保留 BinaryOp 结构交由 Precision 域内部 BigRational 求值。
    //
    // 验证：precision(20, 1/3) 应返回 BigRational(1/3)，
    // 经 format_bigrational(r, Some(20)) 格式化后应输出 20 位精确的 1/3。
    #[test]
    fn test_precision_function_preserves_exact_rational() {
        let cache = CacheManager::new();
        let ctx = EvalContext::new();
        let (result, domain, _, fmt_prec) =
            evaluate("precision(20, 1/3)", &ctx, None, &cache).unwrap();

        // 必须路由到 precision 域
        assert_eq!(domain, "precision");
        // 顶层 precision(20, expr) 提取 N=20 用于格式化
        assert_eq!(fmt_prec, Some(20));

        // 结果必须是 BigRational(1/3)，不是 f64 近似的 BigRational
        let r = match result {
            EvalResult::BigRational(r) => r,
            other => panic!("expected BigRational(1/3), got {:?}", other),
        };
        let one = num_bigint::BigInt::from(1);
        let three = num_bigint::BigInt::from(3);
        let expected = num_rational::BigRational::new(one, three);
        assert_eq!(
            r, expected,
            "precision(20, 1/3) must be exact 1/3, got {}",
            r
        );

        // 格式化验证：20 位精确 1/3 应为 0.33333333333333333333
        let formatted = crate::domains::format_bigrational(&r, Some(20));
        assert_eq!(
            formatted, "0.33333333333333333333",
            "20-digit 1/3 must be exact, got {}",
            formatted
        );
    }

    // BUG-C-002 边界场景：precision 函数嵌套调用时仍需保留精确分数语义。
    //
    // 验证：precision(10, 1/6) 应返回 BigRational(1/6)，
    // 经 10 位格式化为 0.1666666667（10 位小数）。
    #[test]
    fn test_precision_function_preserves_exact_rational_other_fraction() {
        let cache = CacheManager::new();
        let ctx = EvalContext::new();
        let (result, _, _, fmt_prec) = evaluate("precision(10, 1/6)", &ctx, None, &cache).unwrap();
        assert_eq!(fmt_prec, Some(10));

        let r = result.as_bigrational().expect("expected BigRational");
        let one = num_bigint::BigInt::from(1);
        let six = num_bigint::BigInt::from(6);
        let expected = num_rational::BigRational::new(one, six);
        assert_eq!(*r, expected, "precision(10, 1/6) must be exact 1/6, got {}", r);

        let formatted = crate::domains::format_bigrational(r, Some(10));
        // 1/6 = 0.166666...（无限循环 6），10 位小数为 0.1666666666（10 个 6，无舍入）
        assert_eq!(formatted, "0.1666666666", "got {}", formatted);
    }
}
