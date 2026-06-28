//! 跨能力集成测试：解析 → 规范化 → 缓存查询 → 路由 → 计算 → 输出全链路。
//!
//! 任务 9.1-9.3：使用真实 ArithmeticDomain + ScientificDomain 验证端到端管线。
//! 覆盖：
//! - 9.1 全链路成功路径（算术 + 科学 + 混合 + 变量绑定）
//! - 9.2 L1 缓存去重（交换律等价 / 常量折叠等价 共享缓存）
//! - 9.3 错误传播链（7 种 CalcError 各自正确传播）

use calnexus::{
    ArithmeticDomain, AstCanonicalizer, CacheManager, CalcError, DomainRouter, EvalContext,
    EvalResult, ScientificDomain, parse,
};

/// 构建默认路由器（Scientific 优先级 20 > Arithmetic 优先级 10）。
fn default_router() -> DomainRouter {
    let mut router = DomainRouter::new();
    router.register(Box::new(ScientificDomain));
    router.register(Box::new(ArithmeticDomain));
    router
}

/// 全链路求值（无变量绑定）：parse → canonicalize → cache → route → evaluate。
fn evaluate(expr: &str) -> Result<f64, CalcError> {
    evaluate_with_ctx(expr, &EvalContext::new())
}

/// 全链路求值（带变量绑定）。
fn evaluate_with_ctx(expr: &str, ctx: &EvalContext) -> Result<f64, CalcError> {
    let ast = parse(expr)?;
    let (canonical_ast, cf) = AstCanonicalizer::canonicalize(&ast)?;

    let cache = CacheManager::new();
    let router = default_router();

    // 缓存查询
    if let Some(cached) = cache.get(&cf) {
        return cached.as_scalar().ok_or_else(|| {
            CalcError::EvalError("unexpected non-scalar cached result".to_string())
        });
    }

    // 路由 + 求值
    let domain = router.route(&canonical_ast)?;
    let result = domain.evaluate(&canonical_ast, ctx)?;
    cache.insert(&cf, &Ok(result.clone()));

    result.as_scalar().ok_or_else(|| {
        CalcError::EvalError("unexpected non-scalar result".to_string())
    })
}

// ===== 9.1 全链路成功路径 =====

#[test]
fn test_full_pipeline_arithmetic_basic() {
    // 2+3 → 5（ArithmeticDomain）
    assert_eq!(evaluate("2+3").unwrap(), 5.0);
}

#[test]
fn test_full_pipeline_arithmetic_complex() {
    // (2+9)*7-6 → 71
    assert_eq!(evaluate("(2+9)*7-6").unwrap(), 71.0);
}

#[test]
fn test_full_pipeline_arithmetic_power_factorial() {
    // 2^10 + 5! → 1024 + 120 = 1144
    assert_eq!(evaluate("2^10 + factorial(5)").unwrap(), 1144.0);
}

#[test]
fn test_full_pipeline_arithmetic_mod_abs() {
    // 10%3 + abs(-2) → 1 + 2 = 3
    assert_eq!(evaluate("mod(10,3) + abs(-2)").unwrap(), 3.0);
}

#[test]
fn test_full_pipeline_scientific_trig() {
    // sin(pi/2) → 1（ScientificDomain）
    let result = evaluate("sin(pi/2)").unwrap();
    assert!((result - 1.0).abs() < 1e-10);
}

#[test]
fn test_full_pipeline_scientific_log() {
    // log(100, 10) → 2
    let result = evaluate("log(100, 10)").unwrap();
    assert!((result - 2.0).abs() < 1e-10);
}

#[test]
fn test_full_pipeline_scientific_gamma_erf() {
    // gamma(5) → 24, erf(1) ≈ 0.8427
    let result = evaluate("gamma(5) + erf(1)").unwrap();
    // gamma(5)=24, erf(1)≈0.8427008
    assert!((result - 24.8427008).abs() < 1e-5);
}

#[test]
fn test_full_pipeline_mixed_arithmetic_scientific() {
    // sin(pi/2) + 2*3 → 1 + 6 = 7（路由到 ScientificDomain）
    let result = evaluate("sin(pi/2) + 2*3").unwrap();
    assert!((result - 7.0).abs() < 1e-10);
}

#[test]
fn test_full_pipeline_variable_binding() {
    // x=10, y=20 → x*y = 200
    let ctx = EvalContext::new()
        .with_var("x", 10.0)
        .with_var("y", 20.0);
    assert_eq!(evaluate_with_ctx("x*y", &ctx).unwrap(), 200.0);
}

#[test]
fn test_full_pipeline_constant_folding_in_pipeline() {
    // 2*3+1 → 常量折叠为 7，路由到 Arithmetic，求值 7
    assert_eq!(evaluate("2*3+1").unwrap(), 7.0);
}

#[test]
fn test_full_pipeline_pi_e_constants() {
    // e^0 → 1（ScientificDomain 预绑定 e）
    let result = evaluate("exp(0)").unwrap();
    assert!((result - 1.0).abs() < 1e-10);
}

// ===== 9.2 L1 缓存去重 =====

#[test]
fn test_cache_dedup_commutative_addition() {
    // 2+3 与 3+2 规范形式相同 → 共享缓存
    let ast1 = parse("2+3").unwrap();
    let ast2 = parse("3+2").unwrap();
    let (_, cf1) = AstCanonicalizer::canonicalize(&ast1).unwrap();
    let (_, cf2) = AstCanonicalizer::canonicalize(&ast2).unwrap();
    assert_eq!(cf1, cf2, "交换律等价表达式应有相同规范形式");

    let cache = CacheManager::new();
    cache.insert(&cf1, &Ok(EvalResult::Scalar(5.0)));
    assert_eq!(
        cache.get(&cf2),
        Some(EvalResult::Scalar(5.0)),
        "3+2 应命中 2+3 的缓存"
    );
}

#[test]
fn test_cache_dedup_constant_folding() {
    // 2*3+1 与 1+6 均折叠为 7 → 共享缓存
    let ast1 = parse("2*3+1").unwrap();
    let ast2 = parse("1+6").unwrap();
    let (_, cf1) = AstCanonicalizer::canonicalize(&ast1).unwrap();
    let (_, cf2) = AstCanonicalizer::canonicalize(&ast2).unwrap();
    assert_eq!(cf1, cf2, "常量折叠等价表达式应有相同规范形式");

    let cache = CacheManager::new();
    cache.insert(&cf1, &Ok(EvalResult::Scalar(7.0)));
    assert_eq!(
        cache.get(&cf2),
        Some(EvalResult::Scalar(7.0)),
        "1+6 应命中 2*3+1 的缓存"
    );
}

#[test]
fn test_cache_dedup_double_negation() {
    // --5 与 5 规范形式相同 → 共享缓存
    let ast1 = parse("--5").unwrap();
    let ast2 = parse("5").unwrap();
    let (_, cf1) = AstCanonicalizer::canonicalize(&ast1).unwrap();
    let (_, cf2) = AstCanonicalizer::canonicalize(&ast2).unwrap();
    assert_eq!(cf1, cf2, "--5 与 5 应有相同规范形式");

    let cache = CacheManager::new();
    cache.insert(&cf1, &Ok(EvalResult::Scalar(5.0)));
    assert_eq!(cache.get(&cf2), Some(EvalResult::Scalar(5.0)));
}

#[test]
fn test_cache_miss_non_equivalent() {
    // 2-3 与 3-2 规范形式不同 → 不共享缓存
    let ast1 = parse("2-3").unwrap();
    let ast2 = parse("3-2").unwrap();
    let (_, cf1) = AstCanonicalizer::canonicalize(&ast1).unwrap();
    let (_, cf2) = AstCanonicalizer::canonicalize(&ast2).unwrap();
    assert_ne!(cf1, cf2, "2-3 与 3-2 应有不同规范形式");

    let cache = CacheManager::new();
    cache.insert(&cf1, &Ok(EvalResult::Scalar(-1.0)));
    assert_eq!(cache.get(&cf2), None, "3-2 不应命中 2-3 的缓存");
}

#[test]
fn test_cache_get_or_compute_dedup() {
    // 用 get_or_compute 验证：等价表达式第二次不调用 compute
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    let ast1 = parse("2+3").unwrap();
    let ast2 = parse("3+2").unwrap();
    let (_, cf1) = AstCanonicalizer::canonicalize(&ast1).unwrap();
    let (_, cf2) = AstCanonicalizer::canonicalize(&ast2).unwrap();
    assert_eq!(cf1, cf2);

    let cache = CacheManager::new();
    let call_count = Arc::new(AtomicUsize::new(0));
    let cc = Arc::clone(&call_count);

    // 第一次：miss → compute
    let r1 = cache.get_or_compute(&cf1, || {
        cc.fetch_add(1, Ordering::SeqCst);
        Ok(EvalResult::Scalar(5.0))
    }).unwrap();
    assert_eq!(r1, EvalResult::Scalar(5.0));
    assert_eq!(call_count.load(Ordering::SeqCst), 1);

    // 第二次：等价表达式 → 命中缓存 → 不调用 compute
    let r2 = cache.get_or_compute(&cf2, || {
        cc.fetch_add(1, Ordering::SeqCst);
        Ok(EvalResult::Scalar(5.0))
    }).unwrap();
    assert_eq!(r2, EvalResult::Scalar(5.0));
    assert_eq!(call_count.load(Ordering::SeqCst), 1, "等价表达式应命中缓存");
}

#[test]
fn test_cache_does_not_store_errors() {
    // 错误结果不写入缓存
    let cf = calnexus::CanonicalForm::new("(+ 1 2)");
    let cache = CacheManager::new();
    cache.insert(&cf, &Err(CalcError::DivisionByZero));
    assert_eq!(cache.get(&cf), None, "错误结果不应写入缓存");
}

// ===== 9.3 错误传播链 =====

#[test]
fn test_error_parse_error() {
    // 语法错误 → ParseError
    let result = evaluate("2++");
    assert!(result.is_err());
    assert!(
        matches!(result, Err(CalcError::ParseError(_))),
        "expected ParseError, got {:?}",
        result
    );
}

#[test]
fn test_error_unbalanced_parens() {
    // 不平衡括号 → ParseError
    let result = evaluate("(2+3");
    assert!(result.is_err());
    assert!(matches!(result, Err(CalcError::ParseError(_))));
}

#[test]
fn test_error_depth_exceeded() {
    // 深度超过 256 → DepthExceeded
    let deep_expr = format!("1{}", "+1".repeat(300));
    let result = evaluate(&deep_expr);
    assert!(result.is_err());
    assert!(
        matches!(result, Err(CalcError::DepthExceeded)),
        "expected DepthExceeded, got {:?}",
        result
    );
}

#[test]
fn test_error_division_by_zero() {
    // 5/0 → DivisionByZero（规范化阶段检测）
    let result = evaluate("5/0");
    assert!(result.is_err());
    assert!(
        matches!(result, Err(CalcError::DivisionByZero)),
        "expected DivisionByZero, got {:?}",
        result
    );
}

#[test]
fn test_error_modulo_by_zero() {
    // 10%0 → DivisionByZero
    let result = evaluate("mod(10,0)");
    assert!(result.is_err());
    assert!(
        matches!(result, Err(CalcError::DivisionByZero)),
        "expected DivisionByZero, got {:?}",
        result
    );
}

#[test]
fn test_error_nan_or_inf_overflow() {
    // 1e308+1e308 → Inf → NaNOrInf（规范化阶段检测）
    let result = evaluate("1e308+1e308");
    assert!(result.is_err());
    assert!(
        matches!(result, Err(CalcError::NaNOrInf)),
        "expected NaNOrInf, got {:?}",
        result
    );
}

#[test]
fn test_error_domain_error_asin_out_of_range() {
    // asin(2) → DomainError（ScientificDomain 求值阶段）
    let result = evaluate("asin(2)");
    assert!(result.is_err());
    assert!(
        matches!(result, Err(CalcError::DomainError(_))),
        "expected DomainError, got {:?}",
        result
    );
}

#[test]
fn test_error_domain_error_log_negative() {
    // ln(-1) → DomainError
    let result = evaluate("ln(-1)");
    assert!(result.is_err());
    assert!(matches!(result, Err(CalcError::DomainError(_))));
}

#[test]
fn test_error_domain_error_unknown_function() {
    // foo(1) → 无域支持 → DomainError（路由阶段）
    let result = evaluate("foo(1)");
    assert!(result.is_err());
    assert!(matches!(result, Err(CalcError::DomainError(_))));
}

#[test]
fn test_error_overflow_factorial_too_large() {
    // factorial(10001) → Overflow（ArithmeticDomain 求值阶段）
    let result = evaluate("factorial(10001)");
    assert!(result.is_err());
    assert!(
        matches!(result, Err(CalcError::Overflow)),
        "expected Overflow, got {:?}",
        result
    );
}

#[test]
fn test_error_overflow_factorial_infinite() {
    // factorial(1000) → 中间结果溢出 f64 → Overflow
    let result = evaluate("factorial(1000)");
    assert!(result.is_err());
    assert!(
        matches!(result, Err(CalcError::Overflow)),
        "expected Overflow, got {:?}",
        result
    );
}

#[test]
fn test_error_eval_error_unbound_variable() {
    // 未绑定变量 → 路由到 ArithmeticDomain，求值时 EvalError 或 DomainError
    let result = evaluate("x+1");
    assert!(result.is_err(), "unbound variable should error");
}

#[test]
fn test_error_eval_error_wrong_arg_count() {
    // factorial(1,2) → 参数数量错误 → EvalError
    let result = evaluate("factorial(1,2)");
    assert!(result.is_err());
    assert!(
        matches!(result, Err(CalcError::EvalError(_))),
        "expected EvalError, got {:?}",
        result
    );
}

#[test]
fn test_all_seven_error_variants_covered() {
    // 元测试：确认 7 种 CalcError 变体均已在集成测试中覆盖传播路径
    // ParseError, EvalError, Overflow, NaNOrInf, DomainError, DepthExceeded, DivisionByZero
    let covered = [
        matches!(evaluate("2++"), Err(CalcError::ParseError(_))),           // ParseError
        matches!(evaluate("factorial(1,2)"), Err(CalcError::EvalError(_))), // EvalError
        matches!(evaluate("factorial(10001)"), Err(CalcError::Overflow)),   // Overflow
        matches!(evaluate("1e308+1e308"), Err(CalcError::NaNOrInf)),        // NaNOrInf
        matches!(evaluate("asin(2)"), Err(CalcError::DomainError(_))),      // DomainError
        matches!(
            evaluate(&format!("1{}", "+1".repeat(300))),
            Err(CalcError::DepthExceeded)
        ), // DepthExceeded
        matches!(evaluate("5/0"), Err(CalcError::DivisionByZero)), // DivisionByZero
    ];
    for (i, ok) in covered.iter().enumerate() {
        assert!(ok, "error variant #{} not properly triggered", i);
    }
}

// ===== 9.6 / 9.7 性能验证 =====

#[test]
fn test_cold_start_performance() {
    // 9.6 验证冷启动性能：全链路求值（parse → canonicalize → cache → route → evaluate）
    // 目标 < 100ms（release 构建）。debug 构建可能较慢，仅作参考。
    let start = std::time::Instant::now();
    let result = evaluate("2+3").unwrap();
    let elapsed = start.elapsed();

    assert_eq!(result, 5.0);
    // release 构建应 < 1ms，debug 构建也远低于 100ms
    assert!(
        elapsed.as_millis() < 100,
        "cold start took {:?}, expected < 100ms",
        elapsed
    );
    eprintln!("cold start (2+3): {:?}", elapsed);
}

#[test]
fn test_cache_hit_performance() {
    // 9.7 验证缓存命中性能：重复求值同一表达式 < 100μs
    // 预填充缓存后，测量 cache.get() 的耗时
    use calnexus::CanonicalForm;

    let cf = CanonicalForm::new("(+ 2 3)");
    let cache = CacheManager::new();
    cache.insert(&cf, &Ok(EvalResult::Scalar(5.0)));

    // 预热
    let _ = cache.get(&cf);

    // 测量 1000 次缓存命中
    let iterations = 1000;
    let start = std::time::Instant::now();
    for _ in 0..iterations {
        let cached = cache.get(&cf);
        debug_assert_eq!(cached, Some(EvalResult::Scalar(5.0)));
    }
    let elapsed = start.elapsed();
    let per_hit = elapsed.as_nanos() / iterations as u128;

    eprintln!(
        "cache hit: {} iterations in {:?} ({} ns/hit, {} μs/hit)",
        iterations,
        elapsed,
        per_hit,
        per_hit / 1000
    );

    // 目标 < 100μs/hit（release 构建）。debug 构建可能较慢，放宽到 1ms。
    let limit_ns = if cfg!(debug_assertions) {
        1_000_000 // 1ms (debug)
    } else {
        100_000 // 100μs (release)
    };
    assert!(
        per_hit < limit_ns,
        "cache hit took {} ns, expected < {} ns",
        per_hit,
        limit_ns
    );
}
