// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! Cache benchmarks (TEST.md §6, BENCH-002 / BENCH-003).
//!
//! 运行：`cargo bench --bench cache_bench`
//! 基线：`target/criterion/` 目录。

use calnexus::{parse, AstCanonicalizer, CacheManager, EvalContext};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use std::hint::black_box;

/// BENCH-002: cache hit < 100μs（第二次求值命中缓存）
fn bench_cache_hit(c: &mut Criterion) {
    let expressions = vec!["2+3", "sin(1.5)+cos(0.5)", "(2+9)*7-6", "100!"];
    let mut group = c.benchmark_group("cache_hit");

    for expr in &expressions {
        let cache = CacheManager::new();
        let ctx = EvalContext::new();
        // 预填充缓存：第一次求值写入
        let ast = parse(expr).expect("parse failed");
        let (_canonical_ast, _) = AstCanonicalizer::canonicalize(&ast).expect("canon failed");
        // 调用 evaluate 写入缓存
        let _ = calnexus::evaluate(expr, &ctx, None, &cache);

        group.bench_with_input(BenchmarkId::from_parameter(expr), expr, |b, e| {
            b.iter(|| {
                // 第二次求值应命中缓存
                let _ = black_box(calnexus::evaluate(black_box(e), &ctx, None, &cache));
            });
        });
    }
    group.finish();
}

/// BENCH-003: cache miss < 1ms（第一次求值，冷启动）
fn bench_cache_miss(c: &mut Criterion) {
    let expressions = vec![
        "2+3",
        "sin(1.5)+cos(0.5)",
        "(2+9)*7-6",
        "matrix([[1,2],[3,4]])",
        "diff(x^2,x)",
    ];
    let mut group = c.benchmark_group("cache_miss");

    for expr in &expressions {
        let ctx = EvalContext::new();
        group.bench_with_input(BenchmarkId::from_parameter(expr), expr, |b, e| {
            b.iter_batched(
                CacheManager::new,
                |cache| {
                    let _ = black_box(calnexus::evaluate(black_box(e), &ctx, None, &cache));
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

criterion_group!(benches, bench_cache_hit, bench_cache_miss);
criterion_main!(benches);
