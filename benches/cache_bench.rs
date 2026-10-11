// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! 缓存命中/未命中基准，基线存于 `target/criterion/`。
//! 运行：`cargo bench --bench cache_bench`。

use calnexus::{AstCanonicalizer, CacheManager, EvalContext, parse};
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::hint::black_box;

/// 缓存命中（二次求值）：目标 <100μs。
fn bench_cache_hit(c: &mut Criterion) {
    let expressions = vec!["2+3", "sin(1.5)+cos(0.5)", "(2+9)*7-6", "100!"];
    let mut group = c.benchmark_group("cache_hit");

    for expr in &expressions {
        let cache = CacheManager::new();
        let ctx = EvalContext::new();
        // 预填充：第一次求值写入缓存
        let ast = parse(expr).expect("parse failed");
        let (_canonical_ast, _) = AstCanonicalizer::canonicalize(&ast).expect("canon failed");
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

/// 缓存未命中（冷启动首次求值）：目标 <1ms。
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
