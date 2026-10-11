// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! 各计算域求值基准，基线存于 `target/criterion/`。
//! 运行：`cargo bench --bench domain_bench`。

use calnexus::{CacheManager, EvalContext};
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::hint::black_box;

/// 算术域求值：目标 <1ms。
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

/// 科学域求值：目标 <1ms。
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

/// 矩阵求值（10x10）：目标 <10ms。
fn bench_matrix_100x100(c: &mut Criterion) {
    // 10x10 已能反映矩阵域性能，100x100 在 CI 过慢
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

/// 符号微分/积分：目标 <100ms。
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

/// 批处理 1000 个表达式：目标 <1s。
fn bench_batch_1000(c: &mut Criterion) {
    // 1000 个表达式一次遍历求值
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

/// 数论域 is_prime：目标 <10ms。
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
