// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! Parser & canonicalizer benchmarks (TEST.md §7).
//!
//! 运行：`cargo bench --bench parser_bench`
//! 基线：`target/criterion/` 目录。

use calnexus::{AstCanonicalizer, parse};
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::hint::black_box;

/// parser throughput ≥ 10000 expr/s（目标：单表达式解析 < 100μs）
fn bench_parser_throughput(c: &mut Criterion) {
    let expressions = vec![
        "2+3",
        "sin(x)+cos(y)",
        "(2+9)*7-6",
        "matrix([[1,2],[3,4]])",
        "diff(x^2,x)",
        "1+2*3-4/5^6",
        "log(10)+exp(2)*sqrt(16)",
        "sum([1,2,3,4,5])",
    ];

    let mut group = c.benchmark_group("parser_throughput");
    for expr in &expressions {
        group.bench_with_input(BenchmarkId::from_parameter(expr), expr, |b, e| {
            b.iter(|| {
                let _ = black_box(parse(black_box(e)));
            });
        });
    }
    group.finish();
}

/// 4096 字符大表达式解析基准（优化放大场景）。
fn bench_parser_large_expression(c: &mut Criterion) {
    // 构造 ~4090 字符的长算术表达式（50 项 × ~80 字符）
    let terms: Vec<String> = (0..50)
        .map(|i| {
            format!(
                "({}+{})*{} - sin({}) + cos({})",
                i * 37,
                i * 13,
                i + 1,
                i,
                i * 2
            )
        })
        .collect();
    let large = terms.join(" + ");
    assert!(large.len() <= 4096, "基准表达式不得超过 4096 上限");

    let mut group = c.benchmark_group("parser_large_expression");
    group.bench_with_input("4096-char", large.as_str(), |b, e| {
        b.iter(|| {
            let _ = black_box(parse(black_box(e)));
        });
    });
    group.finish();
}

/// canonicalizer < 10μs（目标：规范化单表达式 < 10μs）
fn bench_canonicalizer(c: &mut Criterion) {
    let expressions = vec![
        "2+3",
        "x+y",
        "y+x",
        "(2+9)*7-6",
        "x*y+z*x",
        "a+b+c+d+e",
        "sin(x)+cos(x)",
        "x^2+2*x+1",
    ];

    let mut group = c.benchmark_group("canonicalizer");
    for expr in &expressions {
        let ast = parse(expr).expect("parse failed");
        group.bench_with_input(BenchmarkId::from_parameter(expr), &ast, |b, ast| {
            b.iter(|| {
                let _ = black_box(AstCanonicalizer::canonicalize(black_box(ast)));
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_parser_throughput,
    bench_canonicalizer,
    bench_parser_large_expression
);
criterion_main!(benches);
