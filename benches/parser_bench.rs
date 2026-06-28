// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! Parser & canonicalizer benchmarks (TEST.md §6, BENCH-001 / BENCH-009).
//!
//! 运行：`cargo bench --bench parser_bench`
//! 基线：`target/criterion/` 目录。

use calnexus::{parse, AstCanonicalizer};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

/// BENCH-001: parser throughput ≥ 10000 expr/s（目标：单表达式解析 < 100μs）
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

/// BENCH-009: canonicalizer < 10μs（目标：规范化单表达式 < 10μs）
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

criterion_group!(benches, bench_parser_throughput, bench_canonicalizer);
criterion_main!(benches);
