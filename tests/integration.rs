// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
// CalcError 144 字节触发 result_large_err，与 lib.rs 同论证
#![allow(clippy::result_large_err)]
#![allow(clippy::approx_constant, non_snake_case)]

//! 跨能力集成测试：解析→规范化→缓存→路由→计算→输出全链路
//! （含错误传播与 L1 缓存去重：交换律等价 / 常量折叠等价 共享缓存）。

mod common;
use common::default_router;

use calnexus::{
    AstCanonicalizer, CacheManager, CalcError, ErrorKind, EvalContext, EvalResult, PrecisionDomain,
    parse,
};

/// 全链路标量求值（无变量绑定）；非标量结果返回 EvalError。
fn evaluate(expr: &str) -> Result<f64, CalcError> {
    evaluate_with_ctx(expr, &EvalContext::new())
}

/// 全链路标量求值（带变量绑定）；非标量结果返回 EvalError。
fn evaluate_with_ctx(expr: &str, ctx: &EvalContext) -> Result<f64, CalcError> {
    let result = evaluate_full(expr, ctx)?;
    result
        .as_scalar()
        .ok_or_else(|| CalcError::eval("unexpected non-scalar result".to_string()))
}

/// 全链路求值，返回完整 EvalResult（支持 Complex/Matrix/BigInt/BigRational）。
fn evaluate_full(expr: &str, ctx: &EvalContext) -> Result<EvalResult, CalcError> {
    let ast = parse(expr)?;
    let (canonical_ast, cf) = AstCanonicalizer::canonicalize(&ast)?;

    let cache = CacheManager::new();
    let router = default_router();

    if let Some(cached) = cache.get(&cf) {
        return Ok(cached);
    }

    let domain = router.route(&canonical_ast)?;
    let result = domain.evaluate(&canonical_ast, ctx)?;
    cache.insert(&cf, &Ok(result.clone()));

    Ok(result)
}

/// 直接使用 PrecisionDomain 的全链路求值（绕过路由器）。
fn evaluate_precision(expr: &str, ctx: &EvalContext) -> Result<EvalResult, CalcError> {
    use calnexus::CalculationDomain;
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

// ===== 全链路成功路径 =====

#[test]
fn test_full_pipeline_arithmetic_basic() {
    assert_eq!(evaluate("2+3").unwrap(), 5.0);
}

#[test]
fn test_full_pipeline_arithmetic_complex() {
    assert_eq!(evaluate("(2+9)*7-6").unwrap(), 71.0);
}

#[test]
fn test_full_pipeline_arithmetic_power_factorial() {
    assert_eq!(evaluate("2^10 + factorial(5)").unwrap(), 1144.0);
}

#[test]
fn test_full_pipeline_arithmetic_mod_abs() {
    assert_eq!(evaluate("mod(10,3) + abs(-2)").unwrap(), 3.0);
}

#[test]
fn test_full_pipeline_scientific_trig() {
    let result = evaluate("sin(pi/2)").unwrap();
    assert!((result - 1.0).abs() < 1e-10);
}

#[test]
fn test_full_pipeline_scientific_log() {
    let result = evaluate("log(100, 10)").unwrap();
    assert!((result - 2.0).abs() < 1e-10);
}

#[test]
fn test_full_pipeline_scientific_gamma_erf() {
    let result = evaluate("gamma(5) + erf(1)").unwrap();
    assert!((result - 24.8427008).abs() < 1e-5);
}

#[test]
fn test_full_pipeline_mixed_arithmetic_scientific() {
    // 混合表达式路由到 ScientificDomain
    let result = evaluate("sin(pi/2) + 2*3").unwrap();
    assert!((result - 7.0).abs() < 1e-10);
}

#[test]
fn test_full_pipeline_variable_binding() {
    let ctx = EvalContext::new().with_var("x", 10.0).with_var("y", 20.0);
    assert_eq!(evaluate_with_ctx("x*y", &ctx).unwrap(), 200.0);
}

#[test]
fn test_full_pipeline_constant_folding_in_pipeline() {
    // 常量折叠后路由到 Arithmetic
    assert_eq!(evaluate("2*3+1").unwrap(), 7.0);
}

#[test]
fn test_full_pipeline_pi_e_constants() {
    // 验证 ScientificDomain 预绑定的 e 常量
    let result = evaluate("exp(0)").unwrap();
    assert!((result - 1.0).abs() < 1e-10);
}

// ===== L1 缓存去重 =====

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
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let ast1 = parse("2+3").unwrap();
    let ast2 = parse("3+2").unwrap();
    let (_, cf1) = AstCanonicalizer::canonicalize(&ast1).unwrap();
    let (_, cf2) = AstCanonicalizer::canonicalize(&ast2).unwrap();
    assert_eq!(cf1, cf2);

    let cache = CacheManager::new();
    let call_count = Arc::new(AtomicUsize::new(0));
    let cc = Arc::clone(&call_count);

    // 第一次：miss → compute
    let r1 = cache
        .get_or_compute(&cf1, || {
            cc.fetch_add(1, Ordering::SeqCst);
            Ok(EvalResult::Scalar(5.0))
        })
        .unwrap();
    assert_eq!(r1, EvalResult::Scalar(5.0));
    assert_eq!(call_count.load(Ordering::SeqCst), 1);

    // 第二次：等价表达式 → 命中缓存 → 不调用 compute
    let r2 = cache
        .get_or_compute(&cf2, || {
            cc.fetch_add(1, Ordering::SeqCst);
            Ok(EvalResult::Scalar(5.0))
        })
        .unwrap();
    assert_eq!(r2, EvalResult::Scalar(5.0));
    assert_eq!(call_count.load(Ordering::SeqCst), 1, "等价表达式应命中缓存");
}

#[test]
fn test_cache_does_not_store_errors() {
    let cf = calnexus::CanonicalForm::new("(+ 1 2)");
    let cache = CacheManager::new();
    cache.insert(&cf, &Err(CalcError::division_by_zero()));
    assert_eq!(cache.get(&cf), None, "错误结果不应写入缓存");
}

// ===== 错误传播链 =====

#[test]
fn test_error_parse_error() {
    let result = evaluate("2++");
    assert!(result.is_err());
    assert!(
        matches!(&result, Err(e) if e.kind == ErrorKind::Parse),
        "expected ParseError, got {:?}",
        result
    );
}

#[test]
fn test_error_unbalanced_parens() {
    let result = evaluate("(2+3");
    assert!(result.is_err());
    assert!(matches!(&result, Err(e) if e.kind == ErrorKind::Parse));
}

#[test]
fn test_error_depth_exceeded() {
    // 深度超过 256 → DepthExceeded
    let deep_expr = format!("1{}", "+1".repeat(300));
    let result = evaluate(&deep_expr);
    assert!(result.is_err());
    assert!(
        matches!(&result, Err(e) if e.kind == ErrorKind::Depth),
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
        matches!(&result, Err(e) if e.kind == ErrorKind::DivisionByZero),
        "expected DivisionByZero, got {:?}",
        result
    );
}

#[test]
fn test_error_modulo_by_zero() {
    let result = evaluate("mod(10,0)");
    assert!(result.is_err());
    assert!(
        matches!(&result, Err(e) if e.kind == ErrorKind::DivisionByZero),
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
        matches!(&result, Err(e) if e.kind == ErrorKind::NaNOrInf),
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
        matches!(&result, Err(e) if e.kind == ErrorKind::Domain),
        "expected DomainError, got {:?}",
        result
    );
}

#[test]
fn test_error_domain_error_log_negative() {
    let result = evaluate("ln(-1)");
    assert!(result.is_err());
    assert!(matches!(&result, Err(e) if e.kind == ErrorKind::Domain));
}

#[test]
fn test_error_domain_error_unknown_function() {
    // foo(1) → 无域支持 → DomainError（路由阶段）
    let result = evaluate("foo(1)");
    assert!(result.is_err());
    assert!(matches!(&result, Err(e) if e.kind == ErrorKind::Domain));
}

#[test]
fn test_error_overflow_factorial_too_large() {
    // factorial(10001) → Overflow（ArithmeticDomain 求值阶段）
    let result = evaluate("factorial(10001)");
    assert!(result.is_err());
    assert!(
        matches!(&result, Err(e) if e.kind == ErrorKind::Overflow),
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
        matches!(&result, Err(e) if e.kind == ErrorKind::Overflow),
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
        matches!(&result, Err(e) if e.kind == ErrorKind::Eval),
        "expected EvalError, got {:?}",
        result
    );
}

#[test]
fn test_all_seven_error_variants_covered() {
    // 元测试：7 种 CalcError 变体均在此覆盖
    let covered = [
        matches!(evaluate("2++"), Err(e) if e.kind == ErrorKind::Parse),
        matches!(evaluate("factorial(1,2)"), Err(e) if e.kind == ErrorKind::Eval),
        matches!(evaluate("factorial(10001)"), Err(e) if e.kind == ErrorKind::Overflow),
        matches!(evaluate("1e308+1e308"), Err(e) if e.kind == ErrorKind::NaNOrInf),
        matches!(evaluate("asin(2)"), Err(e) if e.kind == ErrorKind::Domain),
        matches!(
            evaluate(&format!("1{}", "+1".repeat(300))),
            Err(e) if e.kind == ErrorKind::Depth
        ),
        matches!(evaluate("5/0"), Err(e) if e.kind == ErrorKind::DivisionByZero),
    ];
    for (i, ok) in covered.iter().enumerate() {
        assert!(ok, "error variant #{} not properly triggered", i);
    }
}

// ===== 性能验证 =====

#[test]
fn test_cold_start_performance() {
    // 冷启动目标 <100ms（debug 仅作参考）
    let start = std::time::Instant::now();
    let result = evaluate("2+3").unwrap();
    let elapsed = start.elapsed();

    assert_eq!(result, 5.0);
    assert!(
        elapsed.as_millis() < 100,
        "cold start took {:?}, expected < 100ms",
        elapsed
    );
    eprintln!("cold start (2+3): {:?}", elapsed);
}

#[test]
fn test_cache_hit_performance() {
    // 预填充缓存后测量 cache.get() 单次耗时
    use calnexus::CanonicalForm;

    let cf = CanonicalForm::new("(+ 2 3)");
    let cache = CacheManager::new();
    cache.insert(&cf, &Ok(EvalResult::Scalar(5.0)));

    let _ = cache.get(&cf);

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

    // 目标 <100μs/hit（debug 放宽到 1ms）
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

// ===== 跨域集成测试：Complex/Matrix/Statistics/Precision 全链路 =====
//
// 覆盖四个新域的解析 → 规范化 → 缓存 → 路由 → 计算 → 输出全链路。

// ----- Complex 域全链路 -----

#[test]
fn test_complex_pipeline_literal() {
    let result = evaluate_full("3+4i", &EvalContext::new()).unwrap();
    assert_eq!(result, EvalResult::Complex(3.0, 4.0));
}

#[test]
fn test_complex_pipeline_addition() {
    let result = evaluate_full("(1+2i)+(3+4i)", &EvalContext::new()).unwrap();
    assert_eq!(result, EvalResult::Complex(4.0, 6.0));
}

#[test]
fn test_complex_pipeline_multiplication() {
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
    let result = evaluate("abs(3+4i)").unwrap();
    assert!((result - 5.0).abs() < 1e-10);
}

#[test]
fn test_complex_pipeline_conj() {
    let result = evaluate_full("conj(3+4i)", &EvalContext::new()).unwrap();
    assert_eq!(result, EvalResult::Complex(3.0, -4.0));
}

#[test]
fn test_complex_pipeline_route_by_function() {
    // 验证 conj() 函数触发 ComplexDomain 路由
    let ast = parse("conj(3+4i)").unwrap();
    let (canonical_ast, _) = AstCanonicalizer::canonicalize(&ast).unwrap();
    let router = default_router();
    let domain = router.route(&canonical_ast).unwrap();
    assert_eq!(domain.domain_name(), "complex");
}

// ----- Matrix 域全链路 -----

#[test]
fn test_matrix_pipeline_literal() {
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
    let result = evaluate("det([[1,2],[3,4]])").unwrap();
    assert!((result - (-2.0)).abs() < 1e-10);
}

#[test]
fn test_matrix_pipeline_transpose() {
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
    let result = evaluate("mean([1,2,3,4,5])").unwrap();
    assert!((result - 3.0).abs() < 1e-10);
}

#[test]
fn test_statistics_pipeline_sum() {
    let result = evaluate("sum([1,2,3,4,5])").unwrap();
    assert!((result - 15.0).abs() < 1e-10);
}

#[test]
fn test_statistics_pipeline_median_odd() {
    let result = evaluate("median([1,2,3,4,5])").unwrap();
    assert!((result - 3.0).abs() < 1e-10);
}

#[test]
fn test_statistics_pipeline_median_even() {
    // 偶数个元素取中间两数均值
    let result = evaluate("median([1,2,3,4])").unwrap();
    assert!((result - 2.5).abs() < 1e-10);
}

#[test]
fn test_statistics_pipeline_variance() {
    // 总体方差（分母 N）
    let result = evaluate("variance([1,2,3,4,5])").unwrap();
    assert!((result - 2.0).abs() < 1e-10);
}

#[test]
fn test_statistics_pipeline_std() {
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
    // 空列表 → DomainError
    let result = evaluate("mean([])");
    assert!(result.is_err());
    assert!(matches!(&result, Err(e) if e.kind == ErrorKind::Domain));
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
    let result = evaluate_full("123456789012345678901234567890 + 1", &EvalContext::new()).unwrap();
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
    let result = evaluate_full("123456789012345678901234567890 * 2", &EvalContext::new()).unwrap();
    match result {
        EvalResult::BigInt(b) => {
            assert_eq!(b.to_string(), "246913578024691357802469135780");
        }
        other => panic!("expected BigInt, got {:?}", other),
    }
}

#[test]
fn test_precision_pipeline_bigrational_division() {
    // 规范化阶段 1/3 折叠为 f64 近似，验证结果仍为 BigRational 且格式化正确
    let result = evaluate_full("precision(5, 1/3)", &EvalContext::new()).unwrap();
    match &result {
        EvalResult::BigRational(r) => {
            let formatted = calnexus::format_bigrational(r, Some(5));
            assert_eq!(formatted, "0.33333");
        }
        other => panic!("expected BigRational, got {:?}", other),
    }
}

#[test]
fn test_precision_pipeline_bigrational_addition() {
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
    // 验证自动约分
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
    // 用 BigNumber 基数避免常量折叠，保留 BigInt 精确求值路径
    let result = evaluate_full("123456789012345678901234567890^2", &EvalContext::new()).unwrap();
    match result {
        EvalResult::BigInt(b) => {
            assert_eq!(
                b.to_string(),
                "15241578753238836750495351562536198787501905199875019052100"
            );
        }
        other => panic!("expected BigInt, got {:?}", other),
    }
}

#[test]
fn test_precision_pipeline_direct_precision_domain() {
    // 验证 --precision N 模式的等价路径（绕过路由器）
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
fn test_special_functions_end_to_end() {
    let cases = [
        ("besselj(0,1)", 0.7651976865579666, 1e-12),
        ("bessely(0,1)", 0.08825696421567697, 1e-12),
        ("besseli(0,1)", 1.2660658777520084, 1e-7),
        ("besselk(0,1)", 0.42102443824070834, 1e-7),
        ("beta(2,3)", 1.0 / 12.0, 1e-12),
        ("zeta(2)", 1.6449340668482264, 1e-12),
        ("airy_ai(0)", 0.3550280538878172, 1e-12),
        ("airy_bi(0)", 0.6149266274460007, 1e-12),
    ];
    for (expr, expected, tol) in cases {
        let got = evaluate(expr).unwrap_or_else(|e| panic!("{expr} failed: {e}"));
        assert!(
            (got - expected).abs() <= tol * expected.abs().max(1.0),
            "{expr}: got {got}, expected {expected}"
        );
    }
}

#[test]
fn test_special_function_domains_end_to_end() {
    assert_eq!(
        evaluate("besselj(1.5,1)").unwrap_err().kind,
        ErrorKind::Domain
    );
    assert_eq!(
        evaluate("besselj(101,1)").unwrap_err().kind,
        ErrorKind::Domain
    );
    assert_eq!(
        evaluate("bessely(0,0)").unwrap_err().kind,
        ErrorKind::Domain
    );
    assert_eq!(evaluate("beta(0,1)").unwrap_err().kind, ErrorKind::Domain);
    assert_eq!(evaluate("zeta(1)").unwrap_err().kind, ErrorKind::Domain);
}

#[test]
fn test_list_functions_includes_special_functions() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_calnexus"))
        .arg("--list-functions")
        .output()
        .expect("run calnexus --list-functions");
    assert!(output.status.success(), "--list-functions failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    for name in [
        "besselj", "bessely", "besseli", "besselk", "beta", "zeta", "airy_ai", "airy_bi",
    ] {
        assert!(stdout.contains(name), "missing {name} in --list-functions");
    }
}

#[test]
fn test_root_and_integral_end_to_end() {
    let root = evaluate("root(x^2-2, x, 1, 2)").unwrap();
    assert!((root - std::f64::consts::SQRT_2).abs() < 1e-9, "got {root}");

    let area = evaluate("integral(sin(x), x, 0, pi)").unwrap();
    assert!((area - 2.0).abs() < 1e-9, "got {area}");

    let cubic = evaluate("integral(x^2, x, 0, 1)").unwrap();
    assert!((cubic - 1.0 / 3.0).abs() < 1e-9, "got {cubic}");

    let reversed = evaluate("integral(x, x, 1, 0)").unwrap();
    assert!((reversed + 0.5).abs() < 1e-12, "got {reversed}");
}

#[test]
fn test_root_and_integral_error_paths() {
    assert_eq!(
        evaluate("root(x^2+1, x, -1, 1)").unwrap_err().kind,
        ErrorKind::Domain
    );
    assert_eq!(
        evaluate("root(1, 1, 0, 1)").unwrap_err().kind,
        ErrorKind::Domain
    );
    assert_eq!(
        evaluate("integral(1, 1, 0, 1)").unwrap_err().kind,
        ErrorKind::Domain
    );
}

#[test]
fn test_cache_dedup_complex_equivalent() {
    // 复数节点不参与 AST 交换律排序，但 Add 展平后规范串按条目排序，顺序仍归一
    let ast1 = parse("(1+2i)+(3+4i)").unwrap();
    let ast2 = parse("(3+4i)+(1+2i)").unwrap();
    let (_, cf1) = AstCanonicalizer::canonicalize(&ast1).unwrap();
    let (_, cf2) = AstCanonicalizer::canonicalize(&ast2).unwrap();
    assert_eq!(cf1, cf2, "加法展平后复数项顺序应归一");

    let r1 = evaluate_full("(1+2i)+(3+4i)", &EvalContext::new()).unwrap();
    let r2 = evaluate_full("(3+4i)+(1+2i)", &EvalContext::new()).unwrap();
    assert_eq!(r1, r2, "求值结果应相同");
    assert_eq!(r1, EvalResult::Complex(4.0, 6.0));

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
    // 列表元素顺序不同则规范形式不同（List 不交换，防止错误去重）
    let ast1 = parse("sum([1,2,3])").unwrap();
    let ast2 = parse("sum([3,2,1])").unwrap();
    let (_, cf1) = AstCanonicalizer::canonicalize(&ast1).unwrap();
    let (_, cf2) = AstCanonicalizer::canonicalize(&ast2).unwrap();
    assert_ne!(cf1, cf2, "不同顺序的列表不应有相同规范形式");
}

// ----- 新域错误传播链 -----

#[test]
fn test_error_complex_unbound_variable() {
    // 复数域中未绑定变量 → EvalError
    let result = evaluate_full("conj(x)", &EvalContext::new());
    assert!(result.is_err());
}

#[test]
fn test_error_matrix_dimension_mismatch() {
    // 矩阵维度不一致 → DomainError
    let result = evaluate_full("[[1,2],[3,4]] + [[1,2,3]]", &EvalContext::new());
    assert!(result.is_err());
    assert!(matches!(&result, Err(e) if e.kind == ErrorKind::Domain));
}

#[test]
fn test_error_matrix_singular_inverse() {
    // 奇异矩阵求逆 → DomainError
    let result = evaluate_full("inverse([[1,2],[2,4]])", &EvalContext::new());
    assert!(result.is_err());
    assert!(matches!(&result, Err(e) if e.kind == ErrorKind::Domain));
}

#[test]
fn test_error_statistics_non_numeric_list() {
    // 含变量的列表非数值 → DomainError 或 EvalError
    let result = evaluate("mean([1,x,3])");
    assert!(result.is_err());
}

#[test]
fn test_error_precision_division_by_zero() {
    let result = evaluate_precision("1/0", &EvalContext::new());
    assert!(result.is_err());
    assert!(
        matches!(&result, Err(e) if e.kind == ErrorKind::DivisionByZero),
        "expected DivisionByZero, got {:?}",
        result
    );
}

// ----- 元测试：全域路由覆盖 -----

#[test]
fn test_all_ten_domains_routed() {
    // 元测试：10 个域均被路由器正确分发
    let cases: &[(&str, &str)] = &[
        ("2+3", "arithmetic"),
        ("sin(pi/2)", "scientific"),
        ("3+4i", "complex"),
        ("[[1,2],[3,4]]", "matrix"),
        ("mean([1,2,3])", "statistics"),
        ("123456789012345678901234567890", "precision"),
        ("gcd(12,18)", "number_theory"),
        ("C(10,3)", "combinatorics"),
        ("dot([1,2],[3,4])", "vector"),
        ("poly_add(x+1,x-1)", "polynomial"),
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

// ===== 新增域端到端集成测试 =====

// ----- 8.2 NumberTheory 跨域集成测试 -----

#[test]
fn test_number_theory_gcd_pipeline() {
    let result = evaluate("gcd(12,18)");
    assert_eq!(result.unwrap(), 6.0);
}

#[test]
fn test_number_theory_lcm_pipeline() {
    let result = evaluate("lcm(4,6)");
    assert_eq!(result.unwrap(), 12.0);
}

#[test]
fn test_number_theory_is_prime_pipeline() {
    assert_eq!(evaluate("is_prime(7)").unwrap(), 1.0);
    assert_eq!(evaluate("is_prime(9)").unwrap(), 0.0);
}

#[test]
fn test_number_theory_prime_sieve_pipeline() {
    let result = evaluate_full("prime_sieve(20)", &EvalContext::new()).unwrap();
    let v = result.as_vector().expect("expected Vector");
    assert_eq!(v, &[2.0, 3.0, 5.0, 7.0, 11.0, 13.0, 17.0, 19.0]);
}

#[test]
fn test_number_theory_mod_pow_pipeline() {
    assert_eq!(evaluate("mod_pow(2,10,1000)").unwrap(), 24.0);
}

#[test]
fn test_number_theory_euler_phi_pipeline() {
    assert_eq!(evaluate("euler_phi(10)").unwrap(), 4.0);
}

#[test]
fn test_number_theory_mod_inverse_pipeline() {
    assert_eq!(evaluate("mod_inverse(3,11)").unwrap(), 4.0);
}

// ----- 8.3 Combinatorics 跨域集成测试 -----

#[test]
fn test_combinatorics_P_pipeline() {
    assert_eq!(evaluate("P(5,2)").unwrap(), 20.0);
}

#[test]
fn test_combinatorics_C_pipeline() {
    assert_eq!(evaluate("C(10,3)").unwrap(), 120.0);
}

#[test]
fn test_combinatorics_catalan_pipeline() {
    assert_eq!(evaluate("catalan(5)").unwrap(), 42.0);
}

#[test]
fn test_combinatorics_stirling_pipeline() {
    assert_eq!(evaluate("stirling(4,2)").unwrap(), 7.0);
}

#[test]
fn test_combinatorics_C_large_bigint() {
    // C(100,50) ≈ 10^29 → BigInt
    let result = evaluate_full("C(100,50)", &EvalContext::new()).unwrap();
    assert!(matches!(result, EvalResult::BigInt(_)));
}

#[test]
fn test_combinatorics_C_zero_k() {
    assert_eq!(evaluate("C(5,0)").unwrap(), 1.0);
}

// ----- 8.4 Vector 跨域集成测试 -----

#[test]
fn test_vector_dot_pipeline() {
    assert_eq!(evaluate("dot([1,2,3],[4,5,6])").unwrap(), 32.0);
}

#[test]
fn test_vector_norm_pipeline() {
    assert_eq!(evaluate("norm([3,4])").unwrap(), 5.0);
}

#[test]
fn test_vector_cross_pipeline() {
    let result = evaluate_full("cross([1,0,0],[0,1,0])", &EvalContext::new()).unwrap();
    let v = result.as_vector().expect("expected Vector");
    assert_eq!(v, &[0.0, 0.0, 1.0]);
}

#[test]
fn test_vector_arithmetic_add_pipeline() {
    let result = evaluate_full("[1,2]+[3,4]", &EvalContext::new()).unwrap();
    let v = result.as_vector().expect("expected Vector");
    assert_eq!(v, &[4.0, 6.0]);
}

#[test]
fn test_vector_angle_pipeline() {
    let result = evaluate("angle([1,0],[1,1])").unwrap();
    assert!((result - std::f64::consts::FRAC_PI_4).abs() < 1e-9);
}

#[test]
fn test_vector_scalar_triple_pipeline() {
    assert_eq!(
        evaluate("scalar_triple([1,0,0],[0,1,0],[0,0,1])").unwrap(),
        1.0
    );
}

#[test]
fn test_vector_dimension_mismatch_error() {
    let result = evaluate_full("[1,2]+[3,4,5]", &EvalContext::new());
    assert!(result.is_err());
    assert!(matches!(&result, Err(e) if e.kind == ErrorKind::Domain));
}

#[test]
fn test_vector_normalize_pipeline() {
    let result = evaluate_full("normalize([3,4])", &EvalContext::new()).unwrap();
    let v = result.as_vector().expect("expected Vector");
    assert!((v[0] - 0.6).abs() < 1e-9);
    assert!((v[1] - 0.8).abs() < 1e-9);
}

// ----- 8.5 Polynomial 跨域集成测试 -----

#[test]
fn test_polynomial_add_pipeline() {
    let result = evaluate_full("poly_add(x+1,x+2)", &EvalContext::new()).unwrap();
    let p = result.as_polynomial().expect("expected Polynomial");
    assert_eq!(p, &[3.0, 2.0]); // 3+2x（升幂存储）
}

#[test]
fn test_polynomial_sub_pipeline() {
    let result = evaluate_full("poly_sub(x^2+2*x,x+1)", &EvalContext::new()).unwrap();
    let p = result.as_polynomial().expect("expected Polynomial");
    assert_eq!(p, &[-1.0, 1.0, 1.0]); // -1+x+x^2
}

#[test]
fn test_polynomial_mul_pipeline() {
    let result = evaluate_full("poly_mul(x+1,x+1)", &EvalContext::new()).unwrap();
    let p = result.as_polynomial().expect("expected Polynomial");
    assert_eq!(p, &[1.0, 2.0, 1.0]); // 1+2x+x^2
}

#[test]
fn test_polynomial_div_pipeline() {
    let result = evaluate_full("poly_div(x^2-1,x-1)", &EvalContext::new()).unwrap();
    let p = result.as_polynomial().expect("expected Polynomial");
    assert_eq!(p, &[1.0, 1.0]); // 1+x
}

#[test]
fn test_polynomial_eval_pipeline() {
    assert_eq!(evaluate("poly_eval(x^2+1,2)").unwrap(), 5.0);
}

#[test]
fn test_polynomial_roots_real_pipeline() {
    let result = evaluate_full("roots(x^2-4)", &EvalContext::new()).unwrap();
    let v = result.as_vector().expect("expected Vector");
    assert_eq!(v.len(), 2);
    assert!((v[0] - 2.0).abs() < 1e-9 || (v[0] + 2.0).abs() < 1e-9);
}

#[test]
fn test_polynomial_roots_complex_pipeline() {
    let result = evaluate_full("roots(x^2+1)", &EvalContext::new()).unwrap();
    let c = result.as_complex_list().expect("expected ComplexList");
    assert_eq!(c.len(), 2);
}

#[test]
fn test_polynomial_diff_pipeline() {
    let result = evaluate_full("poly_diff(x^3+2*x)", &EvalContext::new()).unwrap();
    let p = result.as_polynomial().expect("expected Polynomial");
    assert_eq!(p, &[2.0, 0.0, 3.0]); // 2+3x^2
}

#[test]
fn test_polynomial_integrate_pipeline() {
    let result = evaluate_full("poly_integrate(2*x)", &EvalContext::new()).unwrap();
    let p = result.as_polynomial().expect("expected Polynomial");
    assert_eq!(p, &[0.0, 0.0, 1.0]); // x^2
}

#[test]
fn test_polynomial_factor_pipeline() {
    let result = evaluate_full("factor(x^2-4)", &EvalContext::new()).unwrap();
    let s = result.as_symbolic().expect("expected Symbolic");
    assert!(s.contains("(x-2)"));
    assert!(s.contains("(x+2)"));
}

// ----- 8.6 跨域缓存去重测试 -----

#[test]
fn test_v08_cache_dedup_same_expression() {
    let cache = CacheManager::new();
    let ast = parse("gcd(12,18)").unwrap();
    let (canonical_ast, cf) = AstCanonicalizer::canonicalize(&ast).unwrap();
    let router = default_router();

    let domain = router.route(&canonical_ast).unwrap();
    let result1 = domain
        .evaluate(&canonical_ast, &EvalContext::new())
        .unwrap();
    cache.insert(&cf, &Ok(result1.clone()));

    assert!(cache.get(&cf).is_some());
    assert_eq!(cache.get(&cf).unwrap(), result1);
}

#[test]
fn test_v08_cache_dedup_different_expression() {
    let cache = CacheManager::new();
    let ast1 = parse("gcd(12,18)").unwrap();
    let (_, cf1) = AstCanonicalizer::canonicalize(&ast1).unwrap();
    let ast2 = parse("gcd(8,12)").unwrap();
    let (_, cf2) = AstCanonicalizer::canonicalize(&ast2).unwrap();

    cache.insert(&cf1, &Ok(EvalResult::Scalar(6.0)));
    assert!(cache.get(&cf1).is_some());
    assert!(cache.get(&cf2).is_none());
}

// ----- 8.7 错误传播测试 -----

#[test]
fn test_v08_error_unknown_function() {
    let result = evaluate("unknown_func(1)");
    assert!(result.is_err());
    assert!(matches!(&result, Err(e) if e.kind == ErrorKind::Domain));
}

#[test]
fn test_v08_error_negative_combinatorics() {
    let result = evaluate("C(-1,2)");
    assert!(result.is_err());
    assert!(matches!(&result, Err(e) if e.kind == ErrorKind::Domain));
}

#[test]
fn test_v08_error_vector_dimension_mismatch() {
    let result = evaluate_full("[1,2]+[3,4,5]", &EvalContext::new());
    assert!(result.is_err());
    assert!(matches!(&result, Err(e) if e.kind == ErrorKind::Domain));
}

#[test]
fn test_v08_error_divide_by_zero_polynomial() {
    let result = evaluate_full("poly_div(x+1,0)", &EvalContext::new());
    assert!(result.is_err());
    assert!(matches!(&result, Err(e) if e.kind == ErrorKind::DivisionByZero));
}

#[test]
fn test_v08_error_non_polynomial_expression() {
    // sin(x) 不是多项式
    let result = evaluate_full("poly_add(sin(x),x)", &EvalContext::new());
    assert!(result.is_err());
    assert!(matches!(&result, Err(e) if e.kind == ErrorKind::Domain));
}

// ===== Symbolic 域集成测试 =====

#[test]
fn test_v10_symbolic_diff_power_rule() {
    let result = evaluate_full("diff(x^2, x)", &EvalContext::new());
    let s = result.unwrap().as_symbolic().unwrap().clone();
    assert!(s.contains("2"), "expected coefficient 2 in '{}'", s);
    assert!(s.contains("x"), "expected variable x in '{}'", s);
}

#[test]
fn test_v10_symbolic_diff_trig() {
    let result = evaluate_full("diff(sin(x), x)", &EvalContext::new());
    let s = result.unwrap().as_symbolic().unwrap().clone();
    assert!(s.contains("cos"), "expected cos in '{}'", s);
}

#[test]
fn test_v10_symbolic_diff_chain_rule() {
    // 链式法则求导应包含 cos
    let result = evaluate_full("diff(sin(x^2), x)", &EvalContext::new());
    let s = result.unwrap().as_symbolic().unwrap().clone();
    assert!(s.contains("cos"), "expected cos in '{}'", s);
}

#[test]
fn test_v10_symbolic_simplify_add_zero() {
    let result = evaluate_full("simplify(x+0)", &EvalContext::new());
    let s = result.unwrap().as_symbolic().unwrap().clone();
    assert_eq!(s, "x");
}

#[test]
fn test_v10_symbolic_simplify_mul_one() {
    let result = evaluate_full("simplify(1*x)", &EvalContext::new());
    let s = result.unwrap().as_symbolic().unwrap().clone();
    assert_eq!(s, "x");
}

#[test]
fn test_v10_symbolic_limit_direct() {
    let result = evaluate_full("limit(x^2, x, 3)", &EvalContext::new());
    let v = result.unwrap().as_scalar().unwrap();
    assert!((v - 9.0).abs() < 1e-9, "expected 9.0, got {}", v);
}

#[test]
fn test_v10_symbolic_limit_lhopital() {
    // L'Hôpital 法则求极限
    let result = evaluate_full("limit(sin(x)/x, x, 0)", &EvalContext::new());
    let v = result.unwrap().as_scalar().unwrap();
    assert!((v - 1.0).abs() < 1e-9, "expected 1.0, got {}", v);
}

#[test]
fn test_v10_symbolic_taylor_exp() {
    let result = evaluate_full("taylor(exp(x), x, 3)", &EvalContext::new());
    let s = result.unwrap().as_symbolic().unwrap().clone();
    assert!(s.contains("1"), "expected constant 1 in '{}'", s);
    assert!(s.contains("x"), "expected x term in '{}'", s);
}

// ===== BigNumber 路由修复测试 =====

#[test]
fn test_v10_bignumber_is_prime_routes_to_number_theory() {
    // BigNumber 参数应路由到 NumberTheory 而非 Precision
    let ast = parse("is_prime(1000000000000000009)").unwrap();
    let (canonical, _) = AstCanonicalizer::canonicalize(&ast).unwrap();
    let router = default_router();
    let domain = router.route(&canonical).unwrap();
    assert_eq!(domain.domain_name(), "number_theory");
}

#[test]
fn test_v10_bignumber_gcd_routes_to_number_theory() {
    // BigNumber 参数路由到 NumberTheory
    let ast = parse("gcd(1000000000000000009, 1000000000000000007)").unwrap();
    let (canonical, _) = AstCanonicalizer::canonicalize(&ast).unwrap();
    let router = default_router();
    let domain = router.route(&canonical).unwrap();
    assert_eq!(domain.domain_name(), "number_theory");
}

#[test]
fn test_v10_bignumber_pure_routes_to_precision() {
    // 纯 BigNumber 算术 → 路由到 Precision
    let ast = parse("1000000000000000009+1").unwrap();
    let (canonical, _) = AstCanonicalizer::canonicalize(&ast).unwrap();
    let router = default_router();
    let domain = router.route(&canonical).unwrap();
    assert_eq!(domain.domain_name(), "precision");
}

// ===== 隐式乘法集成测试 =====

#[test]
fn test_v10_implicit_mult_2x() {
    let ctx = EvalContext::new().with_var("x", 3.0);
    let result = evaluate_with_ctx("2x", &ctx).unwrap();
    assert_eq!(result, 6.0);
}

#[test]
fn test_v10_implicit_mult_2x_no_var() {
    // 未绑定变量时应报错
    let result = evaluate("2x");
    assert!(result.is_err());
}

#[test]
fn test_v10_implicit_mult_paren() {
    let ctx = EvalContext::new().with_var("x", 3.0);
    let result = evaluate_with_ctx("3(x+1)", &ctx).unwrap();
    assert_eq!(result, 12.0);
}

// ===== 多项式除法与高次求根测试 =====

#[test]
fn test_v10_poly_div_exact() {
    let result = evaluate_full("poly_div(x^2-1, x-1)", &EvalContext::new());
    let p = result.unwrap().as_polynomial().unwrap().clone();
    // 升幂存储：[1, 1] → 1 + x
    assert_eq!(p, vec![1.0, 1.0]);
}

#[test]
fn test_v10_poly_div_by_number() {
    let result = evaluate_full("poly_div(2*x+4, 2)", &EvalContext::new());
    let p = result.unwrap().as_polynomial().unwrap().clone();
    assert_eq!(p, vec![2.0, 1.0]);
}

#[test]
fn test_v10_roots_cubic_real() {
    let result = evaluate_full("roots(x^3-6*x^2+11*x-6)", &EvalContext::new());
    let v = result.unwrap().as_vector().unwrap().clone();
    assert_eq!(v.len(), 3);
    let mut sorted = v.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert!((sorted[0] - 1.0).abs() < 1e-6);
    assert!((sorted[1] - 2.0).abs() < 1e-6);
    assert!((sorted[2] - 3.0).abs() < 1e-6);
}

#[test]
fn test_v10_roots_cubic_one_real() {
    // 一个实根两个复根 → ComplexList
    let result = evaluate_full("roots(x^3+1)", &EvalContext::new());
    let v = result.unwrap().as_complex_list().unwrap().clone();
    assert_eq!(v.len(), 3);
    // 至少有一个实根 -1
    assert!(
        v.iter()
            .any(|(re, im)| (re - (-1.0)).abs() < 1e-6 && im.abs() < 1e-6)
    );
}

#[test]
fn test_v10_roots_quartic() {
    let result = evaluate_full("roots(x^4-5*x^2+4)", &EvalContext::new());
    let v = result.unwrap().as_vector().unwrap().clone();
    assert_eq!(v.len(), 4);
    let mut sorted = v.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert!((sorted[0] - (-2.0)).abs() < 1e-5);
    assert!((sorted[1] - (-1.0)).abs() < 1e-5);
    assert!((sorted[2] - 1.0).abs() < 1e-5);
    assert!((sorted[3] - 2.0).abs() < 1e-5);
}

#[test]
fn test_v10_roots_quartic_repeated() {
    // 重根表达式求根不应失败
    let result = evaluate_full("roots(x^4-2*x^2+1)", &EvalContext::new());
    let v = result.unwrap().as_vector().unwrap().clone();
    assert!(!v.is_empty());
}

// ===== 公开 API 稳定性测试（模块移动后不应破坏） =====
//
// 验证模块移动后公开 API 导出不变（re-export 断裂则无法编译）。

#[test]
fn test_public_api_unchanged_after_move() {
    // core 层导出经 calnexus:: 访问；提取 type alias 以避免 clippy::type_complexity。
    type EvaluateFn =
        fn(
            &str,
            &calnexus::EvalContext,
            Option<usize>,
            &calnexus::CacheManager,
        )
            -> Result<(calnexus::EvalResult, String, bool, Option<usize>), calnexus::CalcError>;
    let _parse: fn(&str) -> Result<calnexus::AstNode, calnexus::CalcError> = calnexus::parse;
    let _evaluate: EvaluateFn = calnexus::evaluate;
    let _: calnexus::DomainRouter = calnexus::DomainRouter::new();
    let _: calnexus::AstCanonicalizer = calnexus::AstCanonicalizer;
    let _: calnexus::CacheManager = calnexus::CacheManager::new();

    // domains 层 unit struct 直接构造，不调 ::default()。
    let _: calnexus::SymbolicDomain = calnexus::SymbolicDomain;
    let _: calnexus::ArithmeticDomain = calnexus::ArithmeticDomain;
    let _: calnexus::CombinatoricsDomain = calnexus::CombinatoricsDomain;
    let _: calnexus::ComplexDomain = calnexus::ComplexDomain;
    let _: calnexus::MatrixDomain = calnexus::MatrixDomain;
    let _: calnexus::NumberTheoryDomain = calnexus::NumberTheoryDomain;
    let _: calnexus::PolynomialDomain = calnexus::PolynomialDomain;
    let _: calnexus::PrecisionDomain = calnexus::PrecisionDomain;
    let _: calnexus::ScientificDomain = calnexus::ScientificDomain;
    let _: calnexus::StatisticsDomain = calnexus::StatisticsDomain;
    let _: calnexus::VectorDomain = calnexus::VectorDomain;

    // I18n 非 unit struct，仍用 ::default()
    let _: calnexus::I18n = calnexus::I18n::default();

    // 运行时验证 evaluate 可正常调用
    let ctx = calnexus::EvalContext::new();
    let cache = calnexus::CacheManager::new();
    let (result, domain, cache_hit, fmt_prec) =
        calnexus::evaluate("2+3", &ctx, None, &cache).unwrap();
    assert_eq!(result, calnexus::EvalResult::Scalar(5.0));
    assert_eq!(domain, "arithmetic");
    assert!(!cache_hit);
    assert_eq!(fmt_prec, None);
}

// ===== SymbolicDomain 通过 domains 命名空间访问测试 =====
//
// 验证 crate 根 re-export 与 domains 命名空间两条访问路径。

#[test]
fn test_symbolic_domain_accessible_via_domains() {
    // 路径 1：crate 根 re-export
    let _: calnexus::SymbolicDomain = calnexus::SymbolicDomain;

    // 路径 2：domains 子模块命名空间
    let _: calnexus::domains::SymbolicDomain = calnexus::domains::SymbolicDomain;

    // 两条路径应指向同一类型
    fn assert_same_type(_: calnexus::SymbolicDomain) {}
    assert_same_type(calnexus::domains::SymbolicDomain);

    // SymbolicDomain 注册到 DomainRouter 后处理符号表达式（无需 use trait）
    let mut router = calnexus::DomainRouter::new();
    router.register(Box::new(calnexus::domains::SymbolicDomain));
    let ctx = calnexus::EvalContext::new();
    let cache = calnexus::CacheManager::new();
    let (result, domain, _hit, _fmt) =
        calnexus::evaluate("simplify(2*x+3*x)", &ctx, None, &cache).unwrap();
    match result {
        calnexus::EvalResult::Symbolic(s) => {
            assert!(
                s.contains("5"),
                "expected simplified to contain 5, got: {}",
                s
            );
            assert!(
                s.contains("x"),
                "expected simplified to contain x, got: {}",
                s
            );
        }
        other => panic!("expected Symbolic result, got {:?}", other),
    }
    assert_eq!(domain, "symbolic");
}
