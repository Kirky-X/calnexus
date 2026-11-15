# 📊 CalNexus Performance Guide

> This document gives CalNexus's benchmark methodology, current baseline data, and performance design notes.
> Real-world performance depends on expression complexity and hardware; always trust your own `cargo bench --features cli` measurements.

## 📋 Table of Contents

- [🎯 Overview](#-overview)
- [🧪 Benchmarking](#-benchmarking)
  - [Running the Benchmarks](#running-the-benchmarks)
  - [Benchmark Suites](#benchmark-suites)
- [📈 Baseline Data](#-baseline-data)
  - [Cache Hit vs Full Pipeline (cache_bench)](#cache-hit-vs-full-pipeline-cache_bench)
  - [Parser (parser_bench)](#parser-parser_bench)
  - [Canonicalizer (parser_bench / canonicalizer group)](#canonicalizer-parser_bench--canonicalizer-group)
  - [Domain Routing + Evaluation (domain_bench)](#domain-routing--evaluation-domain_bench)
  - [Direct API vs Expression Path (api_bench)](#direct-api-vs-expression-path-api_bench)
- [💾 Cache Design](#-cache-design)
- [⚙️ Optimization Tips](#️-optimization-tips)
- [📚 Related Documents](#-related-documents)

---

## 🎯 Overview

CalNexus's performance design revolves around one principle: **route repeated evaluations through the cache hot path, and make the first evaluation as cheap as possible**. The hot path (cache hit) returns a clone of `EvalResult` (no JSON serialization, no temporary runtime); the cold path (miss) goes through parse → canonicalize → domain routing → evaluate, with an independent benchmark guarding each stage.

### Performance goals

| Goal | Status |
|------|------|
| Sub-microsecond cache-hit path | ✅ ~0.90 µs for simple expressions |
| single-flight dedup (concurrent identical keys evaluate once) | ✅ `try_get_with` |
| Bounded cache footprint | ✅ byte-weight budget + large-result admission threshold |
| Zero pollution from nondeterministic results | ✅ `now`/`today`/`fx` bypass the cache |

---

## 🧪 Benchmarking

### Running the Benchmarks

```bash
# Run all benchmarks (requires the cli feature)
cargo bench --features cli

# Run a specific suite
cargo bench --features cli --bench cache_bench
cargo bench --features cli --bench parser_bench
cargo bench --features cli --bench domain_bench
cargo bench --features cli --bench api_bench

# Save a baseline for later version comparisons
cargo bench --features cli --bench cache_bench -- --save-baseline v0.1.5
```

Results are written to `target/criterion/`, with an HTML report (`target/criterion/report/index.html`).

### Benchmark Suites

| Suite | Benchmark targets |
|------|----------|
| `cache_bench.rs` | L1 cache hit/miss performance |
| `parser_bench.rs` | Parser throughput, large expressions (4096 chars), canonicalizer |
| `domain_bench.rs` | Domain routing + evaluation, batch of 1000 expressions, primality testing |
| `api_bench.rs` | Direct API vs expression path comparison |

---

## 📈 Baseline Data

> **Measurement basis**: baseline `main` (`--save-baseline main`, saved at `target/criterion/main.baseline` for the perf_001/002 regression gates in `performance_tests.rs`). Environment: development machine WSL2 (linux 6.6.87), rustc 1.97.1, criterion defaults (100 samples), commit `597ffb2`, collected 2026-09-17. The tables show criterion median point estimates; they drift with versions/hardware — trust your own `cargo bench --features cli` runs.

### Cache Hit vs Full Pipeline (cache_bench)

| Case | Median | Notes |
|------|--------|------|
| `cache_hit / 2+3` | 0.902 µs | Pure cache read path (BLAKE3 key + Arc return) |
| `cache_hit / (2+9)*7-6` | 1.509 µs | |
| `cache_hit / 100!` | 1.399 µs | Hit cost nearly decoupled from expression content |
| `cache_hit / sin(1.5)+cos(0.5)` | 2.362 µs | |
| `cache_miss / 2+3` | 10.715 µs | Full pipeline: parse + canonicalize + evaluate + cache insert |
| `cache_miss / sin(1.5)+cos(0.5)` | 12.522 µs | |
| `cache_miss / diff(x^2, x)` | 15.430 µs | Symbolic calculus |
| `cache_miss / matrix([[1,2],[3,4]])` | 16.981 µs | Matrix literal |
| `cache_miss / (2+9)*7-6` | 12.728 µs | |

A cache hit is roughly **12×** faster than the full pipeline; hit cost stays in the sub-microsecond~microsecond range and is nearly decoupled from expression complexity.

### Parser (parser_bench)

| Case | Median |
|------|--------|
| `2+3` | 277 ns |
| `(2+9)*7-6` | 700 ns |
| `1+2*3-4*5*6` | 746 ns |
| `diff(x^2, x)` | 949 ns |
| `log(10)+exp(2)_sqrt(16)` | 1.100 µs |
| `sin(x)+cos(y)` | 1.122 µs |
| `sum([1,2,3,4,5])` | 1.935 µs |
| `matrix([[1,2],[3,4]])` | 2.244 µs |
| 4096-character large expression | 75.921 µs |

### Canonicalizer (parser_bench / canonicalizer group)

| Case | Median |
|------|--------|
| `2+3` | 42.4 ns |
| `(2+9)*7-6` | 67.6 ns |
| `x+y` | 112.0 ns |
| `y+x` | 112.2 ns (same level as `x+y`; canonicalizes to the same cache key) |
| `sin(x)+cos(x)` | 285.1 ns |
| `x*y+z*x` | 309.3 ns |
| `x^2+2*x+1` | 418.5 ns |
| `a+b+c+d+e` | 429.9 ns |

### Domain Routing + Evaluation (domain_bench)

| Group | Case | Median |
|----|------|--------|
| arithmetic | `2+3` | 13.462 µs |
| arithmetic | `2^10` / `100/7` / `123.456` / `1.5+2.7-3.1` | 13.1 ~ 13.7 µs |
| scientific | `sqrt(144)` / `log(100)` / `exp(2)` / `sin(1.5)` / `atan2(3,4)` / `cos(0)+sin(π/2)` | 10.4 ~ 15.0 µs |
| symbolic | `diff(x^2,x)` / `diff(sin(x),x)` / `integrate(x^2,x)` | 11.6 µs |
| symbolic | `diff(x^3+2*x^2+x+1, x)` | 16.246 µs |
| matrix | 10×10 matrix evaluation | 48.295 µs |
| number_theory | `is_prime(10007 / 999999937 / 1000000007 / 1000000009)` | 14.3 ~ 15.5 µs |
| batch | 1000-expression full pipeline (rayon parallel) | 1.844 ms |
| batch | 1000 direct API `add` calls | 7.633 µs |

A single domain evaluation costs about the same as a cache-miss full pipeline (≈10~17 µs); the dominant term is the full pipeline plus the in-domain computation.

### Direct API vs Expression Path (api_bench)

| Operation | Direct API | Expression path | Win |
|------|----------|-----------|------|
| `add(2,3)` | 1.96 ns | 920 ns | ≈470× |
| `sin(1.0)` | 7.5 ns | 1.350 µs | ≈180× |
| `dot([1,2,3],[4,5,6])` | 4.0 ns | — (no comparison case) | — |
| `det(3×3)` | 25.7 ns | 4.139 µs | ≈160× |
| `mean([1..5])` | 2.6 ns | 3.151 µs | ≈1200× |

For known operations, programmatic calls should use the `CalNexus` facade direct API (skipping parsing/canonicalization/routing); the gap is 2~3 orders of magnitude.

---

## 💾 Cache Design

`CacheManager` is based on oxcache `ByteWeightCache` (moka kernel, sync byte-weight budget + `try_get_with` single-flight; imported with `default-features = false` to avoid the tokio/redis backend stack):

- **Serialization-free hits**: the cache stores `EvalResult`; the hit path clones the value and returns it, with no JSON serialization and no temporary tokio runtime.
- **single-flight**: `try_get_with` guarantees concurrent identical keys are really evaluated once; all other waiters receive their own clone of the cached value; evaluation errors propagate as-is (errors are never cached).
- **Single-pass hash key**: BLAKE3 hashes the canonical S-expression once; `--precision` and the variable context participate in key construction (precision-aware).
- **Byte-weight budget**: 64MB default; `--cache-size` configures it by entry count (× 4KB approximation).
- **Large-result admission threshold**: results larger than 256KB are not cached, preventing a single entry from dominating the budget.
- **Nondeterministic bypass**: `now` / `today` / `fx` / `fx_rate` are detected before routing; on a hit, both `get` and `insert` are skipped, preventing time/fx results from polluting the cache. Detection costs a HashSet lookup, O(AST node count).

---

## ⚙️ Optimization Tips

### Server: budget and timeouts

- For container deployments, set `CALNEXUS_CACHE_SIZE` according to the memory budget (entries × 4KB approximation); evaluation is CPU-intensive, so scaling replicas by core count beats enlarging the cache.
- Set `--timeout` / `CALNEXUS_TIMEOUT` as a backstop for evaluation; `Timeout` and upstream failures map to 503 and do not occupy the cache.

### Batch: exploit parallelism and cache

- `--batch` already parallelizes with rayon; repeated expressions are automatically collapsed into one evaluation via single-flight.
- Semantically identical expressions in the same file (e.g. `x+y` and `y+x`) share a cache entry after canonicalization.

### Library integration: pick the right entry

- For programmatic calls of known operations use the **direct API** (the `CalNexus` facade), skipping parsing/canonicalization overhead; the `api_bench` suite can compare both paths.
- For user input or expression strings use `evaluate()`, benefiting from domain routing and the cache.
- To inject a custom domain from downstream use `evaluate_with_router`, avoiding replacement of the global router.

### Reproduction and regression

- After upgrading, save a baseline with `--save-baseline` and compare regressions with `--baseline`.
- Note: baselines live in `target/criterion/` (not committed); after `cargo clean` they must be re-collected; after re-measuring, update this document's baseline sections and measurement basis.

---

## 📚 Related Documents

- [🏗️ Architecture](ARCHITECTURE.md) — Design details of the cache and evaluation pipeline
- [📘 API Reference](API_REFERENCE.md) — Expression API and direct API
