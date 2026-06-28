//! 跨能力集成测试：解析 → 规范化 → 缓存查询 → 路由 → 计算 → 输出全链路。
//!
//! 任务 9.1-9.3：使用真实 ArithmeticDomain + ScientificDomain 验证端到端管线。
//! 覆盖：
//! - 9.1 全链路成功路径（算术 + 科学 + 混合 + 变量绑定）
//! - 9.2 L1 缓存去重（交换律等价 / 常量折叠等价 共享缓存）
//! - 9.3 错误传播链（7 种 CalcError 各自正确传播）

use calnexus::{
    ArithmeticDomain, AstCanonicalizer, CacheManager, CalcError, ComplexDomain, DomainRouter,
    EvalContext, EvalResult, MatrixDomain, PrecisionDomain, ScientificDomain, StatisticsDomain,
    parse,
};

/// 构建默认路由器：注册 v0.5 全部 6 个域。
/// 优先级降序：Complex/Matrix (30) > Precision (25) > Statistics (20) > Scientific (20) > Arithmetic (10)。
fn default_router() -> DomainRouter {
    let mut router = DomainRouter::new();
    router.register(Box::new(PrecisionDomain));
    router.register(Box::new(ComplexDomain));
    router.register(Box::new(MatrixDomain));
    router.register(Box::new(ScientificDomain));
    router.register(Box::new(StatisticsDomain));
    router.register(Box::new(ArithmeticDomain));
    router
}

/// 全链路求值（无变量绑定）：parse → canonicalize → cache → route → evaluate。
/// 仅用于标量结果；非标量结果返回 EvalError。
fn evaluate(expr: &str) -> Result<f64, CalcError> {
    evaluate_with_ctx(expr, &EvalContext::new())
}

/// 全链路求值（带变量绑定）。
/// 仅用于标量结果；非标量结果返回 EvalError。
fn evaluate_with_ctx(expr: &str, ctx: &EvalContext) -> Result<f64, CalcError> {
    let result = evaluate_full(expr, ctx)?;
    result.as_scalar().ok_or_else(|| {
        CalcError::EvalError("unexpected non-scalar result".to_string())
    })
}

/// 全链路求值（返回完整 EvalResult，支持 Complex/Matrix/BigInt/BigRational）。
/// 解析 → 规范化 → 缓存查询 → 路由 → 计算 → 写回缓存。
fn evaluate_full(expr: &str, ctx: &EvalContext) -> Result<EvalResult, CalcError> {
    let ast = parse(expr)?;
    let (canonical_ast, cf) = AstCanonicalizer::canonicalize(&ast)?;

    let cache = CacheManager::new();
    let router = default_router();

    // 缓存查询
    if let Some(cached) = cache.get(&cf) {
        return Ok(cached);
    }

    // 路由 + 求值
    let domain = router.route(&canonical_ast)?;
    let result = domain.evaluate(&canonical_ast, ctx)?;
    cache.insert(&cf, &Ok(result.clone()));

    Ok(result)
}

/// 全链路求值（直接使用 PrecisionDomain，绕过路由器）。
/// 用于 --precision N 模式与 precision(N, expr) 函数路径的对照验证。
fn evaluate_precision(expr: &str, ctx: &EvalContext) -> Result<EvalResult, CalcError> {
    use calnexus::core::domain::CalculationDomain;
    let ast = parse(expr)?;
    let (canonical_ast, cf) = AstCanonicalizer::canonicalize(&ast)?;

    let cache = CacheManager::new();
    if let Some(cached) = cache.get(&cf) {
        return Ok(cached);
    }
    let domain = PrecisionDomain;
    let result = domain.evaluate(&canonical_ast, ctx)?;
    cache.insert(&cf, &Ok(result.clone()));
    Ok(result)
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

// ===== 17.1 跨域集成测试：Complex/Matrix/Statistics/Precision 全链路 =====
//
// 任务 17.1：扩展 tests/integration.rs，覆盖 v0.5 四个新域的
// 解析 → 规范化 → 缓存查询 → 路由 → 计算 → 输出 全链路。

// ----- Complex 域全链路 -----

#[test]
fn test_complex_pipeline_literal() {
    // 3+4i → parse → canonicalize → route(ComplexDomain) → evaluate → Complex(3,4)
    let result = evaluate_full("3+4i", &EvalContext::new()).unwrap();
    assert_eq!(result, EvalResult::Complex(3.0, 4.0));
}

#[test]
fn test_complex_pipeline_addition() {
    // (1+2i)+(3+4i) → 4+6i
    let result = evaluate_full("(1+2i)+(3+4i)", &EvalContext::new()).unwrap();
    assert_eq!(result, EvalResult::Complex(4.0, 6.0));
}

#[test]
fn test_complex_pipeline_multiplication() {
    // (1+2i)*(3-4i) → 3-4i+6i-8i² = 3+2i+8 = 11+2i
    let result = evaluate_full("(1+2i)*(3-4i)", &EvalContext::new()).unwrap();
    match result {
        EvalResult::Complex(re, im) => {
            assert!((re - 11.0).abs() < 1e-10, "re={}, expected 11", re);
            assert!((im - 2.0).abs() < 1e-10, "im={}, expected 2", im);
        }
        other => panic!("expected Complex, got {:?}", other),
    }
}

#[test]
fn test_complex_pipeline_abs() {
    // abs(3+4i) → 5（标量结果）
    let result = evaluate("abs(3+4i)").unwrap();
    assert!((result - 5.0).abs() < 1e-10);
}

#[test]
fn test_complex_pipeline_conj() {
    // conj(3+4i) → 3-4i
    let result = evaluate_full("conj(3+4i)", &EvalContext::new()).unwrap();
    assert_eq!(result, EvalResult::Complex(3.0, -4.0));
}

#[test]
fn test_complex_pipeline_route_by_function() {
    // conj(3+4i) 无 Complex 字面量但含 conj() → 仍路由到 ComplexDomain
    // 注：实际上 conj 参数含 Complex 节点，此处验证 conj() 函数路由触发
    let ast = parse("conj(3+4i)").unwrap();
    let (canonical_ast, _) = AstCanonicalizer::canonicalize(&ast).unwrap();
    let router = default_router();
    let domain = router.route(&canonical_ast).unwrap();
    assert_eq!(domain.domain_name(), "complex");
}

// ----- Matrix 域全链路 -----

#[test]
fn test_matrix_pipeline_literal() {
    // [[1,2],[3,4]] → Matrix([[1,2],[3,4]])
    let result = evaluate_full("[[1,2],[3,4]]", &EvalContext::new()).unwrap();
    match result {
        EvalResult::Matrix(m) => {
            assert_eq!(m, vec![vec![1.0, 2.0], vec![3.0, 4.0]]);
        }
        other => panic!("expected Matrix, got {:?}", other),
    }
}

#[test]
fn test_matrix_pipeline_addition() {
    // [[1,2],[3,4]] + [[5,6],[7,8]] → [[6,8],[10,12]]
    let result = evaluate_full("[[1,2],[3,4]] + [[5,6],[7,8]]", &EvalContext::new()).unwrap();
    match result {
        EvalResult::Matrix(m) => {
            assert_eq!(m, vec![vec![6.0, 8.0], vec![10.0, 12.0]]);
        }
        other => panic!("expected Matrix, got {:?}", other),
    }
}

#[test]
fn test_matrix_pipeline_multiplication() {
    // [[1,2],[3,4]] * [[5,6],[7,8]] → [[19,22],[43,50]]
    let result = evaluate_full("[[1,2],[3,4]] * [[5,6],[7,8]]", &EvalContext::new()).unwrap();
    match result {
        EvalResult::Matrix(m) => {
            assert_eq!(m, vec![vec![19.0, 22.0], vec![43.0, 50.0]]);
        }
        other => panic!("expected Matrix, got {:?}", other),
    }
}

#[test]
fn test_matrix_pipeline_determinant() {
    // det([[1,2],[3,4]]) → -2（标量结果）
    let result = evaluate("det([[1,2],[3,4]])").unwrap();
    assert!((result - (-2.0)).abs() < 1e-10);
}

#[test]
fn test_matrix_pipeline_transpose() {
    // transpose([[1,2],[3,4]]) → [[1,3],[2,4]]
    let result = evaluate_full("transpose([[1,2],[3,4]])", &EvalContext::new()).unwrap();
    match result {
        EvalResult::Matrix(m) => {
            assert_eq!(m, vec![vec![1.0, 3.0], vec![2.0, 4.0]]);
        }
        other => panic!("expected Matrix, got {:?}", other),
    }
}

#[test]
fn test_matrix_pipeline_route_by_function() {
    // det([[1,2],[3,4]]) 含 det() → 路由到 MatrixDomain
    let ast = parse("det([[1,2],[3,4]])").unwrap();
    let (canonical_ast, _) = AstCanonicalizer::canonicalize(&ast).unwrap();
    let router = default_router();
    let domain = router.route(&canonical_ast).unwrap();
    assert_eq!(domain.domain_name(), "matrix");
}

// ----- Statistics 域全链路 -----

#[test]
fn test_statistics_pipeline_mean() {
    // mean([1,2,3,4,5]) → 3
    let result = evaluate("mean([1,2,3,4,5])").unwrap();
    assert!((result - 3.0).abs() < 1e-10);
}

#[test]
fn test_statistics_pipeline_sum() {
    // sum([1,2,3,4,5]) → 15
    let result = evaluate("sum([1,2,3,4,5])").unwrap();
    assert!((result - 15.0).abs() < 1e-10);
}

#[test]
fn test_statistics_pipeline_median_odd() {
    // median([1,2,3,4,5]) → 3（奇数个元素）
    let result = evaluate("median([1,2,3,4,5])").unwrap();
    assert!((result - 3.0).abs() < 1e-10);
}

#[test]
fn test_statistics_pipeline_median_even() {
    // median([1,2,3,4]) → 2.5（偶数个元素取中间两数均值）
    let result = evaluate("median([1,2,3,4])").unwrap();
    assert!((result - 2.5).abs() < 1e-10);
}

#[test]
fn test_statistics_pipeline_variance() {
    // variance([1,2,3,4,5]) → 2（总体方差 N）
    let result = evaluate("variance([1,2,3,4,5])").unwrap();
    assert!((result - 2.0).abs() < 1e-10);
}

#[test]
fn test_statistics_pipeline_std() {
    // std([1,2,3,4,5]) → √2
    let result = evaluate("std([1,2,3,4,5])").unwrap();
    assert!((result - std::f64::consts::SQRT_2).abs() < 1e-10);
}

#[test]
fn test_statistics_pipeline_min_max_count() {
    // min/max/count 综合验证
    assert!((evaluate("min([3,1,4,1,5,9,2,6])").unwrap() - 1.0).abs() < 1e-10);
    assert!((evaluate("max([3,1,4,1,5,9,2,6])").unwrap() - 9.0).abs() < 1e-10);
    assert!((evaluate("count([3,1,4,1,5,9,2,6])").unwrap() - 8.0).abs() < 1e-10);
}

#[test]
fn test_statistics_pipeline_route_by_function() {
    // mean([1,2,3]) 含 mean() → 路由到 StatisticsDomain
    let ast = parse("mean([1,2,3])").unwrap();
    let (canonical_ast, _) = AstCanonicalizer::canonicalize(&ast).unwrap();
    let router = default_router();
    let domain = router.route(&canonical_ast).unwrap();
    assert_eq!(domain.domain_name(), "statistics");
}

#[test]
fn test_statistics_pipeline_empty_list_error() {
    // mean([]) → DomainError（空列表）
    let result = evaluate("mean([])");
    assert!(result.is_err());
    assert!(matches!(result, Err(CalcError::DomainError(_))));
}

// ----- Precision 域全链路（路由器路径） -----

#[test]
fn test_precision_pipeline_bigint_literal() {
    // 16+ 位整数字面量 → BigNumber → 路由 PrecisionDomain → BigInt
    let result = evaluate_full("123456789012345678901234567890", &EvalContext::new()).unwrap();
    match result {
        EvalResult::BigInt(b) => {
            assert_eq!(b.to_string(), "123456789012345678901234567890");
        }
        other => panic!("expected BigInt, got {:?}", other),
    }
}

#[test]
fn test_precision_pipeline_bigint_addition() {
    // 大整数 + 1 → BigInt
    let result = evaluate_full(
        "123456789012345678901234567890 + 1",
        &EvalContext::new(),
    )
    .unwrap();
    match result {
        EvalResult::BigInt(b) => {
            assert_eq!(b.to_string(), "123456789012345678901234567891");
        }
        other => panic!("expected BigInt, got {:?}", other),
    }
}

#[test]
fn test_precision_pipeline_bigint_multiplication() {
    // 大整数乘法不丢精度
    let result = evaluate_full(
        "123456789012345678901234567890 * 2",
        &EvalContext::new(),
    )
    .unwrap();
    match result {
        EvalResult::BigInt(b) => {
            assert_eq!(b.to_string(), "246913578024691357802469135780");
        }
        other => panic!("expected BigInt, got {:?}", other),
    }
}

#[test]
fn test_precision_pipeline_bigrational_division() {
    // precision(5, 1/3) → BigRational（f64 近似，因为规范化阶段 1/3 被常量折叠为 0.333...）
    // 注：1/3 在规范化阶段被折叠为 f64 近似值，PrecisionDomain 接收的是 Number(0.333...)
    //     通过 BigRational::from_float 转换为近似分数。格式化到 5 位小数仍输出 "0.33333"。
    //     此处验证：结果是 BigRational 变体 + 格式化输出正确
    let result = evaluate_full("precision(5, 1/3)", &EvalContext::new()).unwrap();
    match &result {
        EvalResult::BigRational(r) => {
            // 格式化到 5 位小数应输出 0.33333（验证精度保留）
            let formatted = calnexus::domains::precision::format_bigrational(r, Some(5));
            assert_eq!(formatted, "0.33333");
        }
        other => panic!("expected BigRational, got {:?}", other),
    }
}

#[test]
fn test_precision_pipeline_bigrational_addition() {
    // precision(5, 1/3 + 1/6) → BigRational(1/2)
    let result = evaluate_full("precision(5, 1/3 + 1/6)", &EvalContext::new()).unwrap();
    match result {
        EvalResult::BigRational(r) => {
            assert_eq!(r.numer(), &num_bigint::BigInt::from(1));
            assert_eq!(r.denom(), &num_bigint::BigInt::from(2));
        }
        other => panic!("expected BigRational, got {:?}", other),
    }
}

#[test]
fn test_precision_pipeline_bigrational_reduction() {
    // precision(5, 2/4) → BigRational(1/2)（自动约分）
    let result = evaluate_full("precision(5, 2/4)", &EvalContext::new()).unwrap();
    match result {
        EvalResult::BigRational(r) => {
            assert_eq!(r.numer(), &num_bigint::BigInt::from(1));
            assert_eq!(r.denom(), &num_bigint::BigInt::from(2));
        }
        other => panic!("expected BigRational, got {:?}", other),
    }
}

#[test]
fn test_precision_pipeline_big_power() {
    // 大整数幂：123456789012345678901234567890^2 → BigInt
    // 注：2^100 两个操作数均为 Number，规范化阶段被常量折叠为 f64；
    //     此处使用 BigNumber 基数避免折叠，保留 BigInt 精确求值路径。
    let result = evaluate_full(
        "123456789012345678901234567890^2",
        &EvalContext::new(),
    )
    .unwrap();
    match result {
        EvalResult::BigInt(b) => {
            assert_eq!(b.to_string(), "15241578753238836750495351562536198787501905199875019052100");
        }
        other => panic!("expected BigInt, got {:?}", other),
    }
}

#[test]
fn test_precision_pipeline_direct_precision_domain() {
    // 通过 evaluate_precision（绕过路由器）直接使用 PrecisionDomain
    // 验证 --precision N 模式的等价路径
    let result = evaluate_precision("1/3 + 1/6", &EvalContext::new()).unwrap();
    match result {
        EvalResult::BigRational(r) => {
            assert_eq!(r.numer(), &num_bigint::BigInt::from(1));
            assert_eq!(r.denom(), &num_bigint::BigInt::from(2));
        }
        other => panic!("expected BigRational, got {:?}", other),
    }
}

#[test]
fn test_precision_pipeline_route_by_bignumber() {
    // 含 BigNumber 字面量 → 路由到 PrecisionDomain
    let ast = parse("123456789012345678901234567890 + 1").unwrap();
    let (canonical_ast, _) = AstCanonicalizer::canonicalize(&ast).unwrap();
    let router = default_router();
    let domain = router.route(&canonical_ast).unwrap();
    assert_eq!(domain.domain_name(), "precision");
}

#[test]
fn test_precision_pipeline_route_by_function() {
    // precision(5, 1/3) 含 precision() → 路由到 PrecisionDomain
    let ast = parse("precision(5, 1/3)").unwrap();
    let (canonical_ast, _) = AstCanonicalizer::canonicalize(&ast).unwrap();
    let router = default_router();
    let domain = router.route(&canonical_ast).unwrap();
    assert_eq!(domain.domain_name(), "precision");
}

// ----- 跨域缓存去重（新域） -----

#[test]
fn test_cache_dedup_complex_equivalent() {
    // 复数节点不参与交换律排序（canonicalizer 仅排序 Number/Variable 操作数）。
    // (1+2i)+(3+4i) 与 (3+4i)+(1+2i) 规范形式不同，但求值结果相同。
    // 此处验证：相同表达式重复求值产生相同规范形式 → 共享缓存；
    //           不同顺序的复数加法规范形式不同 → 不共享缓存（符合当前设计）。
    let ast1 = parse("(1+2i)+(3+4i)").unwrap();
    let ast2 = parse("(3+4i)+(1+2i)").unwrap();
    let (_, cf1) = AstCanonicalizer::canonicalize(&ast1).unwrap();
    let (_, cf2) = AstCanonicalizer::canonicalize(&ast2).unwrap();
    assert_ne!(cf1, cf2, "复数节点不参与交换律排序，规范形式应不同");

    // 但两者求值结果相同
    let r1 = evaluate_full("(1+2i)+(3+4i)", &EvalContext::new()).unwrap();
    let r2 = evaluate_full("(3+4i)+(1+2i)", &EvalContext::new()).unwrap();
    assert_eq!(r1, r2, "求值结果应相同");
    assert_eq!(r1, EvalResult::Complex(4.0, 6.0));

    // 相同表达式重复求值 → 相同规范形式 → 共享缓存
    let cache = CacheManager::new();
    cache.insert(&cf1, &Ok(EvalResult::Complex(4.0, 6.0)));
    assert_eq!(
        cache.get(&cf1),
        Some(EvalResult::Complex(4.0, 6.0)),
        "相同表达式应命中缓存"
    );
}

#[test]
fn test_cache_dedup_statistics_equivalent() {
    // sum([1,2,3]) 与 sum([3,2,1]) 列表元素顺序不同 → 规范形式不同 → 不共享缓存
    // （List 不交换，验证列表不被错误去重）
    let ast1 = parse("sum([1,2,3])").unwrap();
    let ast2 = parse("sum([3,2,1])").unwrap();
    let (_, cf1) = AstCanonicalizer::canonicalize(&ast1).unwrap();
    let (_, cf2) = AstCanonicalizer::canonicalize(&ast2).unwrap();
    assert_ne!(cf1, cf2, "不同顺序的列表不应有相同规范形式");
}

// ----- 新域错误传播链 -----

#[test]
fn test_error_complex_unbound_variable() {
    // 复数域中未绑定变量（非 i） → EvalError
    let result = evaluate_full("conj(x)", &EvalContext::new());
    assert!(result.is_err());
}

#[test]
fn test_error_matrix_dimension_mismatch() {
    // [[1,2],[3,4]] + [[1,2,3]] → 维度不一致 → DomainError
    let result = evaluate_full("[[1,2],[3,4]] + [[1,2,3]]", &EvalContext::new());
    assert!(result.is_err());
    assert!(matches!(result, Err(CalcError::DomainError(_))));
}

#[test]
fn test_error_matrix_singular_inverse() {
    // inverse([[1,2],[2,4]]) → 奇异矩阵 → DomainError
    let result = evaluate_full("inverse([[1,2],[2,4]])", &EvalContext::new());
    assert!(result.is_err());
    assert!(matches!(result, Err(CalcError::DomainError(_))));
}

#[test]
fn test_error_statistics_non_numeric_list() {
    // mean([1,x,3]) 含变量 x → DomainError 或 EvalError（非数值列表）
    let result = evaluate("mean([1,x,3])");
    assert!(result.is_err());
}

#[test]
fn test_error_precision_division_by_zero() {
    // precision(5, 1/0) → DivisionByZero
    let result = evaluate_precision("1/0", &EvalContext::new());
    assert!(result.is_err());
    assert!(
        matches!(result, Err(CalcError::DivisionByZero)),
        "expected DivisionByZero, got {:?}",
        result
    );
}

// ----- 元测试：v0.5 全域路由覆盖 -----

#[test]
fn test_all_six_domains_routed() {
    // 元测试：验证 v0.5 全部 6 个域均能被路由器正确分发
    let cases: &[(&str, &str)] = &[
        ("2+3", "arithmetic"),
        ("sin(pi/2)", "scientific"),
        ("3+4i", "complex"),
        ("[[1,2],[3,4]]", "matrix"),
        ("mean([1,2,3])", "statistics"),
        ("123456789012345678901234567890", "precision"),
    ];
    for (expr, expected_domain) in cases {
        let ast = parse(expr).unwrap();
        let (canonical_ast, _) = AstCanonicalizer::canonicalize(&ast).unwrap();
        let router = default_router();
        let domain = router.route(&canonical_ast).unwrap();
        assert_eq!(
            domain.domain_name(),
            *expected_domain,
            "expr {:?} should route to {:?}, got {:?}",
            expr,
            expected_domain,
            domain.domain_name()
        );
    }
}
