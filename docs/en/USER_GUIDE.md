# 📖 CalNexus User Guide

> This document is the complete usage tutorial for CalNexus, covering all CLI capabilities from installation to advanced usage.
> For a quick overview, return to the [README](../../README.md); for programmatic calls, see the [📘 API Reference](API_REFERENCE.md).

## 📋 Table of Contents

- [🎯 Introduction](#-introduction)
- [🚀 Quick Start](#-quick-start)
  - [📌 Prerequisites](#-prerequisites)
  - [📦 Installation](#-installation)
  - [💡 First Steps](#-first-steps)
- [🔑 Core Concepts](#-core-concepts)
  - [Three Execution Modes](#three-execution-modes)
  - [Computation Domains and Priority Routing](#computation-domains-and-priority-routing)
  - [L1 Cache and Nondeterministic Functions](#l1-cache-and-nondeterministic-functions)
- [🖥️ CLI Usage](#-cli-usage)
  - [Single-Expression Evaluation](#single-expression-evaluation)
  - [Variable Binding and Implicit Multiplication](#variable-binding-and-implicit-multiplication)
  - [Precision Control](#precision-control)
  - [Output Formats](#output-formats)
  - [JSON Output Contract](#json-output-contract)
  - [Error Output and Exit Codes](#error-output-and-exit-codes)
  - [Error Message Language](#error-message-language)
  - [REPL Interactive Mode](#repl-interactive-mode)
  - [Batch Processing](#batch-processing)
  - [Function Catalog](#function-catalog)
- [🧮 Domain Guide](#-domain-guide)
  - [Core Domains at a Glance](#core-domains-at-a-glance)
  - [Numerical Root Finding and Integration](#numerical-root-finding-and-integration)
  - [Time Domain (time feature)](#time-domain-time-feature)
  - [Unit Domain (unit feature)](#unit-domain-unit-feature)
  - [FX Domain (fx feature)](#fx-domain-fx-feature)
  - [Numerical Linear Algebra (numerical feature)](#numerical-linear-algebra-numerical-feature)
- [🌐 Server Mode](#-server-mode)
- [⚙️ Environment Variables](#-environment-variables)
- [🧰 Troubleshooting](#-troubleshooting)
- [📚 Related Documents](#-related-documents)
- [💻 Evaluation Examples](#-evaluation-examples)

---

## 🎯 Introduction

**CalNexus** is a command-line math expression evaluator written in Rust. It unifies 11 core computation domains — from arithmetic and statistics to symbolic calculus and linear algebra — plus 3 optional computation domains (time / unit / fx) behind a single parser and a priority-routed domain dispatcher. It offers three execution modes (single expression, interactive REPL, parallel batch), together with an L1 cache, arbitrary-precision arithmetic, and JSON output for pipeline integration.

---

## 🚀 Quick Start

### 📌 Prerequisites

| Dependency | Version | Notes |
|------|------|------|
| Rust | >= 1.97.1 | Toolchain (install via `rustup` recommended) |
| Cargo | Bundled with Rust | Build and package management |

No Rust environment is needed if you use prebuilt binaries directly (see Installation below).

### 📦 Installation

**Option 1: crates.io (recommended)**

```bash
cargo install calnexus --features cli
```

**Option 2: prebuilt binaries**

Download the archive for your platform (linux x86_64/aarch64 musl static, macOS x86_64/aarch64, windows x86_64) from [GitHub Releases](https://github.com/kirky-x/calnexus/releases), verify the SHA256, and extract it into your PATH.

**Option 3: build from source**

```bash
git clone https://github.com/kirky-x/calnexus.git
cd calnexus
cargo install --path . --features cli
```

The time / unit / fx domains need extra features: `cargo install calnexus --features cli,time,unit,fx`. For all feature combinations see [🏗️ Architecture · Feature Flags Quick Reference](ARCHITECTURE.md#-feature-flags-quick-reference).

### 💡 First Steps

```bash
$ calnexus '2+3*4'
14
```

If you see the result, the installation works. Next, try symbolic calculus with `calnexus 'diff(x^2, x)'`, or enter interactive mode with `calnexus --repl`.

---

## 🔑 Core Concepts

### Three Execution Modes

| Mode | Command | Use case |
|------|------|----------|
| Single expression | `calnexus '2+3*4'` | Quick evaluation in scripts and the command line |
| REPL | `calnexus --repl` | Interactive exploration, variable binding |
| Batch | `calnexus --batch exprs.txt` | Line-by-line parallel evaluation of a file (rayon) |

### Computation Domains and Priority Routing

Each computation domain implements the `CalculationDomain` trait, declaring the function set it supports (`supports()`) and its priority. `DomainRouter` dispatches by priority; the first matching domain performs the evaluation, and the `[arithmetic]` / `[symbolic]` markers in the output indicate the hit domain. The full function catalog of the current build can be listed with `calnexus --list-functions`.

### L1 Cache and Nondeterministic Functions

The BLAKE3 hash of the canonicalized AST serves as the cache key; a hit returns a cloned evaluation result (not zero-copy). Concurrent evaluations of the same expression are collapsed through single-flight so it is evaluated only once. `now` / `today` / `fx` / `fx_rate` are **nondeterministic functions**: every evaluation bypasses cache reads and writes, preventing time/fx results from being polluted by the cache.

---

## 🖥️ CLI Usage

### Single-Expression Evaluation

```bash
$ calnexus '2+3*4'
14

$ calnexus 'sin(pi/2)'
1

$ calnexus 'gcd(12, 18)'
6
```

### Variable Binding and Implicit Multiplication

`--var name=value` pre-binds variables for the expression (repeatable):

```bash
$ calnexus --var x=3 'x^2 + 2*x + 1'
16

# Implicit multiplication: math idioms like 2x and 3(x+1) are recognized automatically
$ calnexus --var x=3 '2x'
6

$ calnexus --var x=3 '3(x+1)'
12
```

Unbound variables produce an error with a hint:

```bash
$ calnexus 'foo + 1'
error: Evaluation error: Unbound variable: foo
  Hint: define it via --var foo=<value>
```

### Precision Control

Behavioral differences between the two approaches (both at 30 digits — compare the outputs):

```bash
# Function form: true BigRational arbitrary precision (recommended)
$ calnexus 'precision(30, 1/3)'
0.333333333333333333333333333333

# Flag form: enables BigRational mode, but the expression first goes through the f64 parser, where precision is already lost
$ calnexus --precision 30 '1/3'
0.333333333333333314829616256247
```

When you need precision that truly exceeds f64, use the `precision(N, expr)` function form with a purely rational expression. `precision(N, ...)` does not apply to numerical linear algebra functions (they return f64 approximations); wrapping them yields an explicit error.

### Output Formats

| Flag | Output | Example |
|------|------|------|
| (default) | Text result | `14` |
| `--json` | Versioned JSON (see next section) | `{"cache":"miss","domain":"arithmetic","result":5.0,"v":1}` |
| `--latex` | LaTeX form | `calnexus --latex 'diff(x^2, x)'` → `\frac{d}{dx}\left(x^{2}\right) = 2 \cdot x` |
| `--canonical` | S-expression canonical form | `calnexus --canonical 'y+x'` → `(+ x y)` |
| `--steps` | Solution steps | `calnexus --steps '2+3*4'` → `3*4=12`, `2+12=14` |
| `--explain` | Detailed error explanation (mutually exclusive with `--json`) | See [error output](#error-output-and-exit-codes) |

Mutual exclusions (as declared by clap): `--latex` / `--steps` / `--canonical` are pairwise exclusive, and each is exclusive with `--json` / `--repl` / `--batch`; only `--latex` / `--steps` can combine with `--precision` (`--canonical` is exclusive with it; in practice `--latex --precision 5 '1/3'` prints `0.33333` normally).

### JSON Output Contract

`--json` emits a versioned structure (`"v":1`; JSON Schema in [`docs/schema/result-v1.json`](../schema/result-v1.json)):

```bash
$ calnexus --json '2+3'
{"cache":"miss","domain":"arithmetic","result":5.0,"v":1}
```

**result shape × domain mapping table** (v1 contract):

| result shape | Triggering domain/variant | Example |
|-------------|-------------|------|
| number | Scalar results of arithmetic/scientific/statistics etc. (Scalar) | `5.0` |
| `{"re":…,"im":…}` | Complex | `{"im":4.0,"re":3.0}` |
| string (text shape) | Matrix/Vector/Polynomial/Symbolic/BigRational/BigInt/complex root list/JSON composite | `"[[1,2],[3,4]]"`, `"0.33333"` |
| string array | Evaluation steps (Steps, batch/library paths only) | `["2+9=11"]` |

### Error Output and Exit Codes

In JSON mode, errors are emitted as a structured object:

```bash
$ calnexus --json '1/0'
{"error":{"exit_code":1,"hint":"check divisor before division","kind":"DivisionByZero","message":"division by zero"},"v":1}
```

In text mode, use `--explain` for the full diagnostics:

```text
$ calnexus --explain '1/0'
Division by zero: divisor is zero
  Hint: check divisor before division

  Error Kind: DivisionByZero
  Exit Code: 1
  Suggestion: check divisor before division
```

Exit code convention: **1** = evaluation error, **2** = usage/system error, **3** = timeout/dependency unavailable (e.g. fx upstream failure).

When the downstream of a pipe closes early (e.g. `calnexus --list-functions | head -4`), the process is silently terminated by SIGPIPE per Unix convention (shell reports 141) without a spurious panic report.

### Error Message Language

`--lang <en|zh>` (defaults to automatic system-language detection, falling back to `en`) switches the human-readable messages:

```bash
$ calnexus --lang zh 'foo + 1'
错误: 求值错误: 未绑定变量: foo
  提示: 请通过 --var foo=<值> 定义
```

### REPL Interactive Mode

```bash
$ calnexus --repl
CalNexus REPL — type :help for commands, :quit to exit
calnexus> :let x = 10
calnexus> x*2
= 20  [arithmetic]
calnexus> diff(x^2, x)
= 2*x  [symbolic]
calnexus> :vars
x = 10
calnexus> :quit
bye
```

REPL commands: `:help` for help, `:let name = value` to bind a variable, `:vars` to list variables, `:clear` to clear the screen, `:quit` (or `:q`) to exit. Tab completion is supported (based on the unified function catalog; feature-gated functions appear automatically once enabled).

### Batch Processing

One expression per line; lines starting with `#` are comments; evaluation is parallelized with rayon:

```text
$ calnexus --batch exprs.txt
line 1: 2+3 = 5  [arithmetic]
line 2: sin(0) = 0  [scientific]
line 4: diff(x^2, x) = 2*x  [symbolic]
summary: 3 total, 3 ok, 0 errors, 0 cache hits, 4.589395ms
```

### Function Catalog

`--list-functions` prints the function catalog of the current build (grouped by domain; feature-gated domains appear only when enabled; the catalog is a display list — actual evaluability follows each domain's whitelist):

```text
$ calnexus --list-functions
arithmetic:
  abs
  mod
  factorial
scientific:
  ...
```

---

## 🧮 Domain Guide

### Core Domains at a Glance

| Domain | Priority | Representative functions |
|--------|--------|----------|
| Arithmetic | 10 | `+` `-` `*` `/` `^` `factorial` `mod` `abs` |
| Scientific | 20 | `sin` `cos` `tan` `ln` `log10` `log2` `exp` `gamma` `erf` `besselj` `bessely` `besseli` `besselk` `beta` `zeta` `airy_ai` `airy_bi` `root` `integral` |
| Statistics | 20 | `mean` `median` `variance` `std` `sum` `min` `max` `count` |
| Precision | 25 | `precision(N, expr)` |
| NumberTheory | 25 | `gcd` `lcm` `is_prime` `prime_sieve` `mod_inverse` `mod_pow` `euler_phi` |
| Combinatorics | 25 | `P` `C` `catalan` `stirling` |
| Polynomial | 25 | `poly_add` `poly_mul` `roots` `factor` `poly_diff` |
| Complex | 30 | `complex(a,b)` `conj` `arg` `abs` `exp` `ln` |
| Matrix | 30 | `det` `transpose` `inverse` `identity` |
| Vector | 30 | `dot` `cross` `norm` `angle` `normalize` `cosine_similarity` + Hadamard product `[a,b]*[c,d]` |
| Symbolic | 30 | `diff` `integrate` `simplify` `limit` `taylor` |

### Numerical Root Finding and Integration

`root(expr, var, a, b)` finds a root of the expression in `[a, b]` with the Brent method (errors out if the interval does not bracket a root); `integral(expr, var, a, b)` computes a definite integral with 15-point Gauss–Kronrod, falling back to adaptive Simpson when the error estimate exceeds the threshold. The variable name must be the second argument:

```text
root(x^2-2, x, 1, 2)          → 1.414213562…
integral(sin(x), x, 0, pi)    → 2
integral(x^2, x, 0, 1)        → 0.333333333…
```

### Time Domain (time feature)

14 functions: `date` / `datetime` / `timestamp` / `from_timestamp` / `date_diff` / `date_add` / `parse_date` / `format_date` / `reformat_date` / `weekday` / `day_of_year` / `is_leap_year` / `now` / `today`.

Built on jiff 0.2 with an embedded IANA tzdb; supports cross-timezone date/time construction, interval arithmetic, and multi-format auto-detection (ISO 8601 / Chinese / English month names):

```bash
$ calnexus 'weekday("2026-01-01")'
4

$ calnexus 'date_add("2026-01-01", 30, "days")'
2026-01-31T00:00:00+00:00

$ calnexus 'date_diff("2026-01-01", "2026-09-15")'
257

$ calnexus 'format_date("2026-01-01", "%Y/%m/%d")'
2026/01/01
```

### Unit Domain (unit feature)

`convert(value, "from", "to")`: linear conversion across 8 dimensions (length / mass / volume / area / speed / data / time) + affine temperature conversion (C/F/K/R). Unknown units come with nearby suggestions at Levenshtein distance ≤ 2:

```bash
$ calnexus 'convert(1000, "m", "km")'
1

$ calnexus 'convert(0, "C", "F")'
32
```

### FX Domain (fx feature)

`fx(value, "FROM", "TO")` / `fx_rate("FROM", "TO")`: fetches rates via frankfurter.dev (European Central Bank ECB reference rates, roughly 30 major currencies) or the ExchangeRate-API open endpoint (aggregated source, 160+ currencies), with a 3-level cache (memory → file → network) + a network circuit breaker (fast-fail after consecutive failures). `CALNEXUS_FX_PROVIDER` selects the data source: `frankfurter` (default) / `erapi` / `auto` (fixed chain `frankfurter → erapi`; failover only when the primary source fails). `fx` / `fx_rate` are nondeterministic functions and bypass the cache.

> Different sources show systematic small differences for the same day (aggregated source vs ECB basis); when mixing sources with `auto`, the server response's `rate_source` field can be used for auditing. FX data is for reference only and does not constitute trading advice.

Environment variables: `CALNEXUS_FX_PROVIDER` (data source, default frankfurter; invalid values fall back to frankfurter), `CALNEXUS_FX_TTL_HOURS` (snapshot TTL, default 24), `CALNEXUS_FX_ALLOW_STALE` (whether to serve an expired snapshot when the network fails), `CALNEXUS_FX_BREAKER_THRESHOLD` / `CALNEXUS_FX_BREAKER_COOLDOWN_SECS` (breaker threshold / cooldown, default 3 failures / 30 seconds).

### Numerical Linear Algebra (numerical feature)

5 kinds of numerical decompositions return JSON (`lu`/`qr`/`eig`/`svd`) or vectors (`solve`); results are nalgebra f64 approximations:

```bash
$ calnexus 'solve([[2,1],[1,3]],[3,5])'
[0.8,1.4]

$ calnexus 'lu([[4,3],[6,3]])'
{"L":[[1.0,0.0],[0.6666666666666666,1.0]],"P":[[0.0,1.0],[1.0,0.0]],"U":[[6.0,3.0],[0.0,1.0]]}
```

---

## 🌐 Server Mode

`--serve-http` serves the REST API and operational endpoints; `--serve-mcp` serves the MCP stdio server (both require the `server` feature). For the endpoint list, deployment, observability, and MCP client configuration, see the Server Guide:

- [🖥️ Server Guide](SERVER.md) — HTTP endpoints and deployment, observability, MCP client configuration and tool list

---

## ⚙️ Environment Variables

Configuration precedence: **command-line flags > environment variables > defaults**.

| Environment variable | Default | Description |
|----------|------|------|
| `CALNEXUS_TIMEOUT` | — | Evaluation timeout (seconds), maps to `--timeout` |
| `CALNEXUS_CACHE_SIZE` | 10000 entries (× 4KB ≈ 40MB) | Cache budget, maps to `--cache-size`; the library API `CacheManager::new()` defaults to a 64MB byte weight |
| `CALNEXUS_BIND_ADDR` | `127.0.0.1:3000` | HTTP server listen address, maps to `--bind` |
| `CALNEXUS_FX_PROVIDER` | `frankfurter` | fx data source: `frankfurter` / `erapi` / `auto` (invalid values fall back to `frankfurter`) |
| `CALNEXUS_FX_TTL_HOURS` | 24 | FX rate snapshot TTL |
| `CALNEXUS_FX_ALLOW_STALE` | — | Whether to serve an expired snapshot when the network fails |
| `CALNEXUS_FX_BREAKER_THRESHOLD` | 3 | FX network circuit-breaker failure threshold |
| `CALNEXUS_FX_BREAKER_COOLDOWN_SECS` | 30 | FX circuit-breaker cooldown (seconds) |
| `CALNEXUS_RATELIMIT_LIMIT` | 120 | Requests allowed per rate-limit window (`ratelimit` feature) |
| `CALNEXUS_RATELIMIT_WINDOW_SECS` | 60 | HTTP rate-limit window duration (`ratelimit` feature) |
| `RUST_LOG` | `warn` | Log level (`observability` feature) |

---

## 🧰 Troubleshooting

For frequently asked questions see the [❓ FAQ](FAQ.md), covering: precision not meeting expectations, fx network-failure behavior, the wasm32 support scope and build commands, wasm-api exports, feature-combination selection, and more.

---

## 📚 Related Documents

- [📘 API Reference](API_REFERENCE.md) — Programmatic calls (expression API and direct API)
- [🏗️ Architecture](ARCHITECTURE.md) — Design principles, module layout, and data flow
- [📈 Performance Guide](PERFORMANCE.md) — Benchmark data and tuning tips
- [❓ FAQ](FAQ.md) — Frequently asked questions
- [🖥️ Server Guide](SERVER.md) — Server-mode deployment and MCP integration
- [📋 Changelog](CHANGELOG.md) — Release notes

---

## 💻 Evaluation Examples

<details>
<summary>📄 Code · bash</summary>

```bash
# Basics and scientific functions
$ calnexus 'sin(pi/2)'
1
$ calnexus 'gcd(12, 18)'
6

# Arbitrary precision (BigRational)
$ calnexus 'precision(50, 1/3)'
0.33333333333333333333333333333333333333333333333333

# Symbolic calculus
$ calnexus 'limit(sin(x)/x, x, 0)'
1
$ calnexus 'taylor(exp(x), x, 3)'
1+x+0.5*x^2+0.16666666666666666*x^3

# Implicit multiplication
$ calnexus --var x=3 '2x'
6
```

</details>
