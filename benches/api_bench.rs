// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! Direct API vs expression-path benchmark (trait-api-toolkit T041).
//!
//! 运行：`cargo bench --bench api_bench`
//! 对比直接 API 调用与表达式解析路径的性能差异。

use calnexus::{CalNexus, CacheManager, EvalContext, Matrix, Vector};
use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;

/// 标量加法：直接 API vs 表达式路径。
fn bench_scalar_add(c: &mut Criterion) {
    let cn = CalNexus::new();
    let ctx = EvalContext::new();

    let mut group = c.benchmark_group("scalar_add");

    group.bench_function("direct_api", |b| {
        b.iter(|| {
            let _ = black_box(cn.scalar().add(black_box(2.0), black_box(3.0)));
        });
    });

    group.bench_function("expression_path", |b| {
        let cache = CacheManager::new();
        b.iter(|| {
            let _ = black_box(calnexus::evaluate(black_box("2+3"), &ctx, None, &cache));
        });
    });

    group.finish();
}

/// 三角函数：直接 API vs 表达式路径。
fn bench_scalar_sin(c: &mut Criterion) {
    let cn = CalNexus::new();
    let ctx = EvalContext::new();

    let mut group = c.benchmark_group("scalar_sin");

    group.bench_function("direct_api", |b| {
        b.iter(|| {
            let _ = black_box(cn.scalar().sin(black_box(1.0)));
        });
    });

    group.bench_function("expression_path", |b| {
        let cache = CacheManager::new();
        b.iter(|| {
            let _ = black_box(calnexus::evaluate(black_box("sin(1.0)"), &ctx, None, &cache));
        });
    });

    group.finish();
}

/// 统计均值：直接 API vs 表达式路径。
fn bench_stats_mean(c: &mut Criterion) {
    let cn = CalNexus::new();
    let ctx = EvalContext::new();
    let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];

    let mut group = c.benchmark_group("stats_mean");

    group.bench_function("direct_api", |b| {
        b.iter(|| {
            let _ = black_box(cn.stats().mean(black_box(&data)));
        });
    });

    group.bench_function("expression_path", |b| {
        let cache = CacheManager::new();
        b.iter(|| {
            let _ = black_box(calnexus::evaluate(
                black_box("mean([1,2,3,4,5])"),
                &ctx,
                None,
                &cache,
            ));
        });
    });

    group.finish();
}

/// 矩阵行列式：直接 API vs 表达式路径。
fn bench_linalg_det(c: &mut Criterion) {
    let cn = CalNexus::new();
    let ctx = EvalContext::new();
    let m = Matrix::from_rows(&[&[1.0, 2.0], &[3.0, 4.0]]);

    let mut group = c.benchmark_group("linalg_det");

    group.bench_function("direct_api", |b| {
        b.iter(|| {
            let _ = black_box(cn.linalg().det(black_box(&m)));
        });
    });

    group.bench_function("expression_path", |b| {
        let cache = CacheManager::new();
        b.iter(|| {
            let _ = black_box(calnexus::evaluate(
                black_box("det([[1,2],[3,4]])"),
                &ctx,
                None,
                &cache,
            ));
        });
    });

    group.finish();
}

/// 向量点积：直接 API。
fn bench_linalg_dot(c: &mut Criterion) {
    let cn = CalNexus::new();
    let va = Vector::new(&[1.0, 2.0, 3.0]);
    let vb = Vector::new(&[4.0, 5.0, 6.0]);

    let mut group = c.benchmark_group("linalg_dot");
    group.bench_function("direct_api", |b| {
        b.iter(|| {
            let _ = black_box(cn.linalg().dot(black_box(&va), black_box(&vb)));
        });
    });
    group.finish();
}

/// 批量标量运算：模拟 1000 次直接 API 调用。
fn bench_batch_direct(c: &mut Criterion) {
    let cn = CalNexus::new();

    let mut group = c.benchmark_group("batch");
    group.bench_function("1000_direct_add", |b| {
        b.iter(|| {
            for i in 0..1000 {
                let _ = black_box(cn.scalar().add(i as f64, (i + 1) as f64));
            }
        });
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_scalar_add,
    bench_scalar_sin,
    bench_stats_mean,
    bench_linalg_det,
    bench_linalg_dot,
    bench_batch_direct,
);
criterion_main!(benches);
