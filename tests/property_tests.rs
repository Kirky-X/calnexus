// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

#![allow(clippy::approx_constant, non_snake_case)]

//! Property-based tests using `proptest` (TEST.md §4, PROP-001 ~ PROP-012).
//!
//! 每个 property 测试默认 256 cases；CI 可通过 `PROPTEST_CASES=1024` 提升。

use calnexus::{parse, AstCanonicalizer, CacheManager, EvalContext, EvalResult};
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 256,
        ..ProptestConfig::default()
    })]

    /// PROP-001: `a+b == b+a`（交换律，对 i32 范围内整数成立）
    #[test]
    fn prop_001_addition_commutative(a in -1000i32..1000, b in -1000i32..1000) {
        let cache = CacheManager::new();
        let ctx = EvalContext::new();
        let left = format!("{}+{}", a, b);
        let right = format!("{}+{}", b, a);
        let (r1, _, _, _) = calnexus::cli::evaluate(&left, &ctx, None, &cache).unwrap();
        let (r2, _, _, _) = calnexus::cli::evaluate(&right, &ctx, None, &cache).unwrap();
        prop_assert_eq!(r1, r2);
    }

    /// PROP-002: `a*b == b*a`（乘法交换律）
    #[test]
    fn prop_002_multiplication_commutative(a in -100i32..100, b in -100i32..100) {
        let cache = CacheManager::new();
        let ctx = EvalContext::new();
        let left = format!("{}*{}", a, b);
        let right = format!("{}*{}", b, a);
        let (r1, _, _, _) = calnexus::cli::evaluate(&left, &ctx, None, &cache).unwrap();
        let (r2, _, _, _) = calnexus::cli::evaluate(&right, &ctx, None, &cache).unwrap();
        prop_assert_eq!(r1, r2);
    }

    /// PROP-003: `(a+b)+c == a+(b+c)`（加法结合律）
    #[test]
    fn prop_003_addition_associative(a in -100i32..100, b in -100i32..100, c in -100i32..100) {
        let cache = CacheManager::new();
        let ctx = EvalContext::new();
        let left = format!("({}+{})+{}", a, b, c);
        let right = format!("{}+({}+{})", a, b, c);
        let (r1, _, _, _) = calnexus::cli::evaluate(&left, &ctx, None, &cache).unwrap();
        let (r2, _, _, _) = calnexus::cli::evaluate(&right, &ctx, None, &cache).unwrap();
        prop_assert_eq!(r1, r2);
    }

    /// PROP-004: `(a*b)*c == a*(b*c)`（乘法结合律）
    #[test]
    fn prop_004_multiplication_associative(a in -10i32..10, b in -10i32..10, c in -10i32..10) {
        let cache = CacheManager::new();
        let ctx = EvalContext::new();
        let left = format!("({}*{})*{}", a, b, c);
        let right = format!("{}*({}*{})", a, b, c);
        let (r1, _, _, _) = calnexus::cli::evaluate(&left, &ctx, None, &cache).unwrap();
        let (r2, _, _, _) = calnexus::cli::evaluate(&right, &ctx, None, &cache).unwrap();
        prop_assert_eq!(r1, r2);
    }

    /// PROP-005: `a*(b+c) == a*b + a*c`（分配律）
    #[test]
    fn prop_005_distributive_law(a in -10i32..10, b in -10i32..10, c in -10i32..10) {
        let cache = CacheManager::new();
        let ctx = EvalContext::new();
        let left = format!("{}*({}+{})", a, b, c);
        let right = format!("{}*{}+{}*{}", a, b, a, c);
        let (r1, _, _, _) = calnexus::cli::evaluate(&left, &ctx, None, &cache).unwrap();
        let (r2, _, _, _) = calnexus::cli::evaluate(&right, &ctx, None, &cache).unwrap();
        prop_assert_eq!(r1, r2);
    }

    /// PROP-006: 常量折叠等价性：`a*b+c` 求值结果正确
    #[test]
    fn prop_006_constant_folding_equivalence(a in 1i32..20, b in 1i32..20, c in 1i32..20) {
        let cache = CacheManager::new();
        let ctx = EvalContext::new();
        let folded = format!("{}*{}+{}", a, b, c);
        let (r, _, _, _) = calnexus::cli::evaluate(&folded, &ctx, None, &cache).unwrap();
        let expected = (a as f64) * (b as f64) + (c as f64);
        prop_assert_eq!(r, EvalResult::Scalar(expected));
    }

    /// PROP-007: `canonicalize(a+b)` 与 `canonicalize(b+a)` 字符串相等（缓存键去重）
    #[test]
    fn prop_007_canonical_form_equivalence_for_commutative(a in 0i32..100, b in 0i32..100) {
        let ast1 = parse(&format!("{}+{}", a, b)).unwrap();
        let ast2 = parse(&format!("{}+{}", b, a)).unwrap();
        let (_, cf1) = AstCanonicalizer::canonicalize(&ast1).unwrap();
        let (_, cf2) = AstCanonicalizer::canonicalize(&ast2).unwrap();
        prop_assert_eq!(cf1.as_str(), cf2.as_str());
    }

    /// PROP-008: `canonicalize(canonicalize(x)) == canonicalize(x)`（幂等性）
    #[test]
    fn prop_008_canonicalize_idempotent(a in 0i32..50, b in 0i32..50) {
        let ast = parse(&format!("{}+{}", a, b)).unwrap();
        let (canon1, cf1) = AstCanonicalizer::canonicalize(&ast).unwrap();
        let (_, cf2) = AstCanonicalizer::canonicalize(&canon1).unwrap();
        prop_assert_eq!(cf1.as_str(), cf2.as_str());
    }

    /// PROP-009: `a-b == a+(-b)`（减法等价于加负数）
    #[test]
    fn prop_009_subtraction_as_addition_of_negation(a in -100i32..100, b in -100i32..100) {
        let cache = CacheManager::new();
        let ctx = EvalContext::new();
        let left = format!("{}-{}", a, b);
        let right = format!("{}+(-{})", a, b);
        let (r1, _, _, _) = calnexus::cli::evaluate(&left, &ctx, None, &cache).unwrap();
        let (r2, _, _, _) = calnexus::cli::evaluate(&right, &ctx, None, &cache).unwrap();
        // f64 比较使用近似相等（避免 -0 vs 0）
        if let (EvalResult::Scalar(v1), EvalResult::Scalar(v2)) = (r1, r2) {
            prop_assert!((v1 - v2).abs() < 1e-10, "v1={}, v2={}", v1, v2);
        } else {
            panic!("expected Scalar results");
        }
    }

    /// PROP-010: `a/b*b == a`（对 b != 0）
    #[test]
    fn prop_010_division_then_multiplication_restores(a in -100.0f64..100.0, b in 1.0f64..100.0) {
        let cache = CacheManager::new();
        let ctx = EvalContext::new();
        let expr = format!("({}/{}){}", a, b, b);
        let (r, _, _, _) = calnexus::cli::evaluate(&expr, &ctx, None, &cache).unwrap();
        if let EvalResult::Scalar(v) = r {
            prop_assert!((v - a).abs() < 1e-6, "v={}, a={}", v, a);
        } else {
            panic!("expected Scalar");
        }
    }

    /// PROP-011: 同一表达式第二次求值应命中缓存（cache:hit）
    #[test]
    fn prop_011_cache_hit_on_second_eval(a in 0i32..100, b in 0i32..100) {
        let cache = CacheManager::new();
        let ctx = EvalContext::new();
        let expr = format!("{}+{}", a, b);
        let _ = calnexus::cli::evaluate(&expr, &ctx, None, &cache).unwrap();
        let (_, _, cache_hit, _) = calnexus::cli::evaluate(&expr, &ctx, None, &cache).unwrap();
        prop_assert!(cache_hit, "second eval should hit cache");
    }

    /// PROP-012: `|sin(x)^2 + cos(x)^2 - 1| < 1e-10`（毕达哥拉斯恒等式）
    #[test]
    fn prop_012_pythagorean_trig_identity(x in 0.0f64..6.283185307179586) {
        let cache = CacheManager::new();
        let ctx = EvalContext::new();
        let expr = format!("sin({})^2+cos({})^2", x, x);
        let (r, _, _, _) = calnexus::cli::evaluate(&expr, &ctx, None, &cache).unwrap();
        if let EvalResult::Scalar(v) = r {
            prop_assert!((v - 1.0).abs() < 1e-10, "v={}, expected 1.0", v);
        } else {
            panic!("expected Scalar");
        }
    }
}
