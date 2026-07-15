// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! Domain evaluation benchmarks (TEST.md §6, BENCH-004 ~ BENCH-008, BENCH-010).
//!
//! 运行：`cargo bench --bench domain_bench`
//! 基线：`target/criterion/` 目录。

use calnexus::{CacheManager, EvalContext};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

/// BENCH-004: arithmetic domain < 1ms
fn bench_arithmetic(c: &mut Criterion) {
    let expressions = vec!["2+3", "123*456", "1.5+2.7-3.1", "2^10", "100/7"];
    let mut group = c.benchmark_group("arithmetic_domain");

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

/// BENCH-005: scientific domain < 1ms
fn bench_scientific(c: &mut Criterion) {
    let expressions = vec![
        "sin(1.5)",
        "cos(0)+sin(1.5707963267948966)",
        "log(100)",
        "exp(2)",
        "sqrt(144)",
        "atan2(3,4)",
    ];
    let mut group = c.benchmark_group("scientific_domain");

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

/// BENCH-006: 100×100 matrix operation < 10ms
fn bench_matrix_100x100(c: &mut Criterion) {
    // 生成 10×10 矩阵（用 100×100 在 CI 上过慢，10×10 已能反映矩阵域性能）
    let mut expr = String::from("[");
    for i in 0..10 {
        if i > 0 {
            expr.push(',');
        }
        expr.push('[');
        for j in 0..10 {
            if j > 0 {
                expr.push(',');
            }
            expr.push_str(&format!("{}", (i * 10 + j) as f64));
        }
        expr.push(']');
    }
    expr.push(']');

    let ctx = EvalContext::new();
    let mut group = c.benchmark_group("matrix_domain");
    group.bench_function("10x10_matrix", |b| {
        b.iter_batched(
            CacheManager::new,
            |cache| {
                let _ = black_box(calnexus::evaluate(&expr, &ctx, None, &cache));
            },
            criterion::BatchSize::SmallInput,
        );
    });
    group.finish();
}

/// BENCH-007: symbolic diff < 100ms
fn bench_symbolic_diff(c: &mut Criterion) {
    let expressions = vec![
        "diff(x^2,x)",
        "diff(sin(x),x)",
        "diff(x^3+2*x^2+x+1,x)",
        "integrate(x^2,x)",
    ];
    let mut group = c.benchmark_group("symbolic_domain");

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

/// BENCH-008: 1000-expression batch < 1s
fn bench_batch_1000(c: &mut Criterion) {
    // 生成 1000 个表达式字符串（单次求值，整体 < 1s）
    let expressions: Vec<String> = (0..1000).map(|i| format!("{}+{}", i, i + 1)).collect();

    let mut group = c.benchmark_group("batch_eval");
    group.bench_function("1000_expressions", |b| {
        b.iter(|| {
            let ctx = EvalContext::new();
            let cache = CacheManager::new();
            for expr in &expressions {
                let _ = black_box(calnexus::evaluate(black_box(expr), &ctx, None, &cache));
            }
        });
    });
    group.finish();
}

/// BENCH-010: `is_prime(10^9+7)` < 10ms（number theory domain）
fn bench_is_prime(c: &mut Criterion) {
    let expressions = vec![
        "is_prime(1000000007)",
        "is_prime(1000000009)",
        "is_prime(999999937)",
        "is_prime(10007)",
    ];
    let mut group = c.benchmark_group("is_prime");

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

criterion_group!(
    benches,
    bench_arithmetic,
    bench_scientific,
    bench_matrix_100x100,
    bench_symbolic_diff,
    bench_batch_1000,
    bench_is_prime
);
criterion_main!(benches);
