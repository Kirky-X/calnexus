<div align="center">

<img src="docs/assets/logo.png" alt="CalNexus Logo" height="240">

[![Rust 2024](https://img.shields.io/badge/Rust-2024-edded?logo=rust&style=for-the-badge)](https://www.rust-lang.org/) [![Rustc 1.97.1+](https://img.shields.io/badge/Rustc-1.97.1+-orange.svg?style=for-the-badge)](https://www.rust-lang.org/) [![crates.io](https://img.shields.io/crates/v/calnexus?style=for-the-badge)](https://crates.io/crates/calnexus) [![docs.rs](https://img.shields.io/docsrs/calnexus?style=for-the-badge)](https://docs.rs/calnexus) [![downloads](https://img.shields.io/crates/d/calnexus?style=for-the-badge)](https://crates.io/crates/calnexus) [![CI](https://img.shields.io/github/actions/workflow/status/kirky-x/calnexus/ci.yml?branch=main&label=CI&style=for-the-badge)](https://github.com/kirky-x/calnexus/actions/workflows/ci.yml) [![coverage](https://img.shields.io/badge/coverage-90.4%25%20(llvm--cov%20lines)-brightgreen?style=for-the-badge)](https://github.com/kirky-x/calnexus) [![license](https://img.shields.io/crates/l/calnexus?style=for-the-badge)](LICENSE)

[中文](README.md) | **English**

**Production-grade command-line math expression evaluator in Rust — 14 computation domains behind one entry point**

[✨ Features](#-features) • [🚀 Quick Start](#-quick-start) • [📚 Documentation](#-documentation) • [💻 Examples](#-examples) • [🤝 Contributing](#-contributing)

</div>

---

<div align="center">

### 🎯 Write a Formula, Let the Router Pick the Solver

The `DomainRouter` matches by priority and dispatches on the first hit; parsing, caching, and concurrent dedup are handled by the five-stage pipeline:

<table style="width:100%; border-collapse: collapse">
<tr>
<td align="center" width="25%">🧮<br><b>Full-Domain Coverage</b><br><span style="color:#64748B">11 core · 3 optional · on-demand builds</span></td>
<td align="center" width="25%">🧠<br><b>Calculus</b><br><span style="color:#64748B">differentiate · simplify · limits · Taylor</span></td>
<td align="center" width="25%">⚡<br><b>Microsecond Hits</b><br><span style="color:#64748B">serialization-free · single-flight · complexity-independent</span></td>
<td align="center" width="25%">🌐<br><b>Server Mode</b><br><span style="color:#64748B">REST · MCP · probes &amp; rate limits</span></td>
</tr>
</table>

</div>

---

## 📋 Table of Contents

<details open>
<summary>📑 Table of Contents</summary>

- [✨ Features](#-features)
- [🚀 Quick Start](#-quick-start)
- [🎨 Feature Flags](#-feature-flags)
- [📚 Documentation](#-documentation)
- [💻 Examples](#-examples)
- [🌐 Server Mode](#-server-mode)
- [🏗️ Architecture](#-architecture)
- [🧪 Testing](#-testing)
- [📊 Performance](#-performance)
- [🔒 Security](#-security)
- [🗺️ Roadmap](#-roadmap)
- [🤝 Contributing](#-contributing)
- [📋 Changelog](#-changelog)
- [📄 License](#-license)
- [🙏 Acknowledgments](#-acknowledgments)
- [📞 Contact & Support](#-contact--support)
- [⭐ Star History](#-star-history)

</details>

---

## ✨ Features

<table style="width:100%; border-collapse: collapse">
<tr>
<td width="50%" style="vertical-align:top; padding: 12px">🧮 <b>11 Core Domains + 3 Optional</b><br><span style="color:#64748B">Arithmetic, scientific functions, statistics, precision, number theory, combinatorics, polynomial, complex, matrix, vector, symbolic calculus; optional: time / unit / fx (feature-gated)</span></td>
<td width="50%" style="vertical-align:top; padding: 12px">🧠 <b>Symbolic Calculus</b><br><span style="color:#64748B"><code>diff</code>, <code>integrate</code>, <code>simplify</code>, <code>limit</code>, <code>taylor</code></span></td>
</tr>
<tr>
<td width="50%" style="vertical-align:top; padding: 12px">🔢 <b>Arbitrary Precision</b><br><span style="color:#64748B"><code>precision(N, expr)</code> BigRational-based arbitrary precision</span></td>
<td width="50%" style="vertical-align:top; padding: 12px">📐 <b>Numerical Linear Algebra</b><br><span style="color:#64748B"><code>lu</code>, <code>qr</code>, <code>eig</code>, <code>svd</code>, <code>solve</code> (<code>numerical</code> feature, nalgebra f64 approximation)</span></td>
</tr>
<tr>
<td width="50%" style="vertical-align:top; padding: 12px">🖥️ <b>Three Execution Modes</b><br><span style="color:#64748B">Single expression, REPL (Tab completion + variable binding), parallel batch (rayon)</span></td>
<td width="50%" style="vertical-align:top; padding: 12px">⚡ <b>High-Performance Cache</b><br><span style="color:#64748B">oxcache sync byte-weight cache (64MB byte budget + 256KB large-result admission threshold), single-pass BLAKE3 keys, <code>try_get_with</code> production single-flight dedup</span></td>
</tr>
<tr>
<td width="50%" style="vertical-align:top; padding: 12px">🌐 <b>HTTP + MCP Dual Protocol</b><br><span style="color:#64748B">REST API, health probes, metrics export, graceful shutdown; MCP stdio server for AI clients</span></td>
<td width="50%" style="vertical-align:top; padding: 12px">✖️ <b>Implicit Multiplication</b><br><span style="color:#64748B">Auto-recognition of math idioms like <code>2x</code>, <code>3(x+1)</code></span></td>
</tr>
<tr>
<td width="50%" style="vertical-align:top; padding: 12px">🗂️ <b>Versioned JSON Contract</b><br><span style="color:#64748B"><code>--json</code> emits a versioned structure (<code>"v":1</code>, with JSON Schema) for pipeline integration</span></td>
<td width="50%" style="vertical-align:top; padding: 12px">🧱 <b>Zero-Dependency Core</b><br><span style="color:#64748B"><code>default = []</code> — usable as an embedded computation engine by other crates</span></td>
</tr>
</table>

Beyond the core capabilities listed above, LaTeX / steps / canonical formatters, bilingual (EN/ZH) error messages (embedded Fluent catalogs, always available regardless of the `icu` feature), a runtime function catalog (`--list-functions`), multi-platform prebuilt binaries, and container images are also available; see the [🎨 Feature Flags](#-feature-flags) section for the full mapping to `Cargo.toml`'s `[features]`.

---

## 🚀 Quick Start

### 📦 Installation

<details>
<summary>📄 Code · bash</summary>

```bash
cargo install calnexus --features cli
```

</details>

Requires Rust 1.97.1 or later (MSRV, matching the CI MSRV job). The core library has `default = []` with zero dependencies; enable `cli` for the command-line experience. Install commands for every feature combo (optional domains, numerical linear algebra, HTTP server, etc.) are unified in the [🎨 Feature Flags](#-feature-flags) section below.

Prebuilt multi-platform binaries (linux x86_64/aarch64 musl static, macOS x86_64/aarch64, windows x86_64, with SHA256SUMS) are available from [GitHub Releases](https://github.com/kirky-x/calnexus/releases), or build from source; the wasm32 target supports library/binary builds for the core and `cli` features (CI-gated), but has no L1 cache and does not promise runtime usability:

<details>
<summary>📄 Code · bash</summary>

```bash
git clone https://github.com/kirky-x/calnexus.git
cd calnexus
cargo install --path . --features cli
```

</details>

### 💡 Minimal Example

The following examples run directly in your terminal:

<details>
<summary>📄 Code · bash</summary>

```bash
$ calnexus '2+3*4'
14

$ calnexus 'diff(x^2, x)'
2*x

$ calnexus --json '2+3'
{"cache":"miss","domain":"arithmetic","result":5.0,"v":1}
```

</details>

### 🧭 Core Concepts

- **Domain routing**: each of the 14 computation domains implements the `CalculationDomain` trait; `DomainRouter` dispatches by priority — the first `supports()` hit wins.
- **Evaluation pipeline**: `parse → canonicalize → cache → route → evaluate`; the BLAKE3 hash of the canonicalized AST is the cache key.
- **Three modes**: single expression, REPL (`:let` / `:vars` / `:quit`, Tab completion), batch (`--batch`, rayon-parallel).
- **Feature gating**: optional domains and server capabilities are independent features; builds contain only what you enable, and the core library can be zero-dependency.
- **Versioned JSON contract**: `--json` output carries `"v":1`; see [`docs/schema/result-v1.json`](docs/schema/result-v1.json) for the schema.

---

## 🎨 Feature Flags

### 📦 Recommended Combos

<details>
<summary>📋 Table</summary>

| Combo | Install / features | Use case |
|-------|--------------------|----------|
| Minimal library | `cargo add calnexus` (`default = []`) | Embedded engine, zero extra deps |
| CLI | `cargo install calnexus --features cli` | Command line / REPL / batch |
| CLI + all domains | `cargo install calnexus --features cli,time,unit,fx` | Adds time / unit / fx domains |
| CLI + numerical | `cargo install calnexus --features cli,numerical` | Numerical linear algebra |
| Bilingual CLI | `cargo install calnexus --features cli,icu` | EN/ZH error messages always built in; `icu` adds standard BCP-47 locale-tag parsing |
| HTTP server | `cargo install calnexus --features server` | REST + MCP dual-protocol service |
| Production server | `cargo install calnexus --features server,ratelimit,docs,observability` | Rate limiting + Swagger UI + structured logs |
| Full | `cargo install calnexus --all-features` | Everything |

</details>

### 📋 Feature Matrix

The table below mirrors `Cargo.toml`'s `[features]` definition; `default = []`.

<details>
<summary>📋 Table</summary>

<table style="width:100%; border-collapse: collapse">
<tr><th style="text-align:left">Feature</th><th style="text-align:center">Default</th><th style="text-align:left">Description</th></tr>
<tr><td colspan="3" style="background:#F8FAFC"><b>Core library</b></td></tr>
<tr><td><code>default</code></td><td align="center">✅</td><td>Zero-dependency core: parsing, canonicalization, 14-domain routing, cache — usable as an embedded engine</td></tr>
<tr><td><code>icu</code></td><td align="center">❌</td><td>ICU4X BCP-47 locale-tag parsing (EN/ZH error messages are always available via embedded Fluent catalogs, not gated by this feature)</td></tr>
<tr><td colspan="3" style="background:#F8FAFC"><b>CLI & interaction</b></td></tr>
<tr><td><code>cli</code></td><td align="center">❌</td><td>CLI / REPL / batch (clap, rustyline, rayon; enabled indirectly via sdforge/cli)</td></tr>
<tr><td colspan="3" style="background:#F8FAFC"><b>Optional computation domains</b></td></tr>
<tr><td><code>time</code></td><td align="center">❌</td><td>Time domain: jiff 0.2 with bundled IANA tzdb, 14 functions</td></tr>
<tr><td><code>unit</code></td><td align="center">❌</td><td>Physical unit conversion domain: 8 dimensions + affine temperature</td></tr>
<tr><td><code>fx</code></td><td align="center">❌</td><td>Currency conversion domain: frankfurter.dev API + 3-level cache + network circuit breaker</td></tr>
<tr><td><code>numerical</code></td><td align="center">❌</td><td>Numerical linear algebra decompositions (lu/qr/eig/svd/solve/matrix_exp, nalgebra f64)</td></tr>
<tr><td colspan="3" style="background:#F8FAFC"><b>Server</b></td></tr>
<tr><td><code>http</code></td><td align="center">❌</td><td>HTTP API (sdforge http/graceful/health/context, axum)</td></tr>
<tr><td><code>mcp</code></td><td align="center">❌</td><td>MCP stdio server (sdforge/mcp)</td></tr>
<tr><td><code>server</code></td><td align="center">❌</td><td>Aggregates <code>http</code> + <code>mcp</code></td></tr>
<tr><td><code>ratelimit</code></td><td align="center">❌</td><td>HTTP rate-limiting middleware (fixed window per-IP, <code>CALNEXUS_RATELIMIT_*</code> configurable, probes exempt)</td></tr>
<tr><td><code>docs</code></td><td align="center">❌</td><td>OpenAPI + Swagger UI (<code>/swagger-ui/</code>, trailing slash required; <code>/api-docs/openapi.json</code> carries an empty spec for now — see the Server Guide)</td></tr>
<tr><td><code>observability</code></td><td align="center">❌</td><td>tracing structured logs (EnvFilter consuming <code>RUST_LOG</code>)</td></tr>
</table>

</details>

---

## 📚 Documentation

<details>
<summary>📋 Table</summary>

| Document | Description |
|----------|-------------|
| [📖 User Guide](docs/USER_GUIDE.md) | Complete tutorial from installation to advanced usage |
| [📘 API Reference](docs/API_REFERENCE.md) | Expression evaluation API and direct API (`CalNexus` facade) |
| [🏗️ Architecture](docs/ARCHITECTURE.md) | Design principles, module layout, and data flow |
| [📈 Performance Guide](docs/PERFORMANCE.md) | Benchmark data, measurement methodology, and tuning tips |
| [🔒 Security](docs/SECURITY.md) | Security design, best practices, and vulnerability handling |
| [❓ FAQ](docs/FAQ.md) | Frequently asked questions |
| [🖥️ Server Guide](docs/SERVER.md) | HTTP deployment, observability, and MCP integration |
| [🧪 Test Scenario Matrix](docs/TEST_SCENARIOS.md) | Exhaustive scenario matrix of the test suites (unit/integration/E2E/property/fuzz/bench) |
| [📋 Changelog](docs/CHANGELOG.md) | Release notes for every version |
| [🤝 Contributing](docs/CONTRIBUTING.md) | How to contribute to the project |
| [📦 Online API docs](https://docs.rs/calnexus) | Latest docs.rs-generated documentation |
| [📦 crates.io](https://crates.io/crates/calnexus) | Publication page |

</details>

Design & process documents: [Code of Conduct](docs/CODE_OF_CONDUCT.md) · [archived process documents](docs/archive/) (PRD, test plan v0.2, research analysis)

---

## 💻 Examples

Evaluation examples are provided as inline command-line invocations — copy them into a terminal and run. The ones below cover arithmetic, scientific functions, number theory, arbitrary precision, symbolic calculus, and implicit multiplication; for full REPL sessions (`:let` / `:vars` / Tab completion), rayon-parallel batch runs (`--batch`), and the functions and usage of the optional domains (time / unit / fx / numerical linear algebra), see the [📖 User Guide](docs/USER_GUIDE.md) and the [📖 User Guide · Domain Guide](docs/USER_GUIDE.md#-计算域指南).

### 🖥️ Evaluation Examples

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

### 🧰 CLI Flags

The behavior, output formats, and exit-code conventions of all flags — `--repl`, `--batch`, `--var`, `--precision`, `--timeout`, `--cache-size`, `--json`, `--latex` / `--canonical` / `--steps`, `--explain`, `--lang`, `--list-functions`, `--serve-http` / `--serve-mcp`, `--bind` — are documented in [📖 User Guide · CLI Usage](docs/USER_GUIDE.md#-cli-使用), or run `calnexus --help`.

---

## 🌐 Server Mode

Enabled with `--features server` (`server = http + mcp`): `calnexus --serve-http` serves the REST evaluation API plus the `/health`, `/live`, `/ready`, and `/metrics` operational endpoints (default `127.0.0.1:3000`, configurable via `--bind`); `calnexus --serve-mcp` exposes the `evaluate` and `list_functions` tools over stdio (the `fx` build additionally registers `fx_budget` / `fx_pricing`), ready for Claude Desktop, Cursor, and other MCP clients.

`/metrics` contract: the Prometheus text carries the three cache families `calnexus_cache_{hits,misses,entries}` (with `# HELP` / `# TYPE` comment lines) plus the `sdforge_http_requests_total{route,method,status}` counter family and the `sdforge_http_request_duration_seconds` latency histogram family; the `?format=json` debug branch carries exactly the four keys `hits` / `misses` / `entry_count` / `http_route_count`. The `route` label takes the request path (not the route template), so high-cardinality REST paths containing ids will multiply the series — relevant for local/intranet deployments.

The endpoint list and language negotiation, Docker / Kubernetes deployment, observability and `ratelimit` throttling, MCP client configuration and argument conventions, error semantics (400 / 422 / 503 and the CLI exit-code contract), and graceful shutdown are covered in the [🖥️ Server Guide](docs/SERVER.md).

---

## 🏗️ Architecture

CalNexus uses a five-layer architecture (entry → orchestration → computation domains → math functions → core infrastructure) with strictly top-down dependencies; with no features enabled the core library is dependency-free and embeddable. The core evaluation path: mathexpr parsing (implicit multiplication) → `AstCanonicalizer` normalization (constant folding, commutative sorting, S-expression canonical form) → `CacheManager` lookup (single-pass BLAKE3 key) → `DomainRouter` priority dispatch → the matching `CalculationDomain` evaluates, with cache hits collapsed through `try_get_with` single-flight and returned as a cloned `EvalResult` (no serialization round-trip).

The per-layer responsibilities and module list, mermaid architecture and evaluation-sequence diagrams, interface-isolation traits, security and performance design, and the ADR decision records are documented in the [🏗️ Architecture Document](docs/ARCHITECTURE.md).

---

## 🧪 Testing

### 🎯 Test Strategy

The test pyramid covers nine layers: inline unit tests in `src/`, integration tests organized by shape (`tests/`: integration, cli_integration, repl_integration, server_http_integration, server_mcp_integration, api_integration, time_unit_fx_integration, numerical_linalg_test), the `tests/e2e` scenario suite (no `required-features`; internal `#[cfg]` gates cover both the enabled and disabled sides), property tests (proptest), snapshot tests (insta), security tests (DoS vectors and boundary attacks), fuzz testing (`fuzz/`, 7 cargo-fuzz targets), Criterion benchmarks (`benches/`, 4 suites), and doc tests of public APIs. The per-suite exhaustive scenario matrix, file mapping, and pass criteria are in the [🧪 Test Scenario Matrix](docs/TEST_SCENARIOS.md).

### ▶️ Run Commands (matching CI)

<details>
<summary>📄 Code · bash</summary>

```bash
# Full test run (CI matrix has six legs: cli / cli,time,unit,fx / cli,server / cli,server,fx / cli,numerical / all)
cargo test --all-features

# Per-leg runs
cargo test --features cli,time,unit,fx    # full suite with optional domains
cargo test --features server              # HTTP/MCP integration
cargo test --features "cli,numerical"     # numerical linear algebra

# Lint and format gates (clippy matrix: cli / cli,server / all)
cargo clippy --all-features --all-targets -- -D warnings
cargo fmt --all -- --check

# Coverage gate: at least 90% line coverage (llvm-cov)
cargo llvm-cov --features "cli,time,unit,fx" --fail-under-lines 90 --summary-only

# Benchmarks
cargo bench --features cli

# Fuzz testing (from the fuzz/ directory)
cargo fuzz run parser_fuzz
```

</details>

### 📊 Test Scale

Current working tree (after v0.2.0-rc.2): grep statistics count 3133 `#[test]` / `#[tokio::test]` functions in total — 2557 unit tests (inline in `src/`), 576 integration & E2E tests (`tests/`, of which the E2E scenario suite has 158), 7 fuzz targets, and 4 Criterion benchmark suites; per-suite numbers drift over time, and the authoritative count is `grep -rE "#\[(tokio::)?test\]" src/ tests/ | wc -l`. The coverage gate is at least 90% line coverage (llvm-cov, measured with `--features cli,time,unit,fx`; currently 90.4%), enforced in CI. For a per-suite exhaustive scenario listing see the [🧪 Test Scenario Matrix](docs/TEST_SCENARIOS.md).

---

## 📊 Performance

Baselines were collected locally on a development machine (WSL2, linux 6.6) using criterion median estimates: cache hits sit in the low-microsecond range independent of expression complexity (`2+3` ≈ 0.90 µs), roughly a 12x win over the full pipeline (`2+3` ≈ 10.7 µs), with parsing and canonicalization in the sub-microsecond range; real-world performance depends on expression complexity and hardware — reproduce with `cargo bench --features cli`. The full baseline tables and measurement methodology are in [📈 Performance Guide · Baselines](docs/PERFORMANCE.md#-基线数据); design points such as serialization-free hits, single-flight, the byte-weight budget, the large-result admission threshold, and nondeterministic bypass are in [📈 Performance Guide · Cache Design](docs/PERFORMANCE.md#-缓存设计).

---

## 🔒 Security

### 🛡️ Security Design

CalNexus is an evaluator with no network access (except the `fx` upstream), no untrusted file I/O, and no plugin loading — the practical attack surface is minimal. Active defenses center on a closed recursion-depth protection loop (`MAX_AST_DEPTH=256` + iterative pre-check + RAII guards), request resource limits, parser error sanitization, and fx upstream hardening. Code-level mechanism details are in [🏗️ Architecture Document · Security Design](docs/ARCHITECTURE.md#-安全设计); the supported-versions table, vulnerability process, and hardening best practices are in the [🔒 Security Document](docs/SECURITY.md).

### ⛓️ Supply Chain & Gates

`cargo audit` (weekly scheduled workflow + pre-release gate), `cargo deny check`, CodeQL static analysis, and the 9-check pre-commit hook run both in CI and via local Git hooks; the full list is in [🔒 Security Document · Dependency Security](docs/SECURITY.md#依赖安全).

### 🚨 Reporting a Vulnerability

Please do **not** report security vulnerabilities through public issues. Use the GitHub [Security Advisories](https://github.com/kirky-x/calnexus/security/policy) private disclosure channel as the primary route (fall back to email Kirky-X@outlook.com, handled by Kirky.X, only if that channel is unavailable). We commit to acknowledging within 48 hours and providing an initial assessment within 7 days. Full policy in [SECURITY.md](docs/SECURITY.md).

---

## 🗺️ Roadmap

<details>
<summary>📋 Table</summary>

<table style="width:100%; border-collapse: collapse">
<tr><th style="text-align:center">Status</th><th style="text-align:left">Direction</th><th style="text-align:left">Items</th></tr>
<tr><td align="center">✅</td><td>Core engine (v0.1.0)</td><td>11 computation domains, symbolic calculus, REPL, batch processing, arbitrary precision, JSON output, implicit multiplication</td></tr>
<tr><td align="center">✅</td><td>Optional domains & server (v0.1.3-v0.1.5)</td><td>Time / unit / fx domains, HTTP + MCP dual protocol, probe separation, rate limiting, observability, multi-platform releases & container images</td></tr>
<tr><td align="center">✅</td><td>Cache engine modernization</td><td>oxcache byte-weight cache (moka core), single-flight, byte-weight budget, BLAKE3 keys, large-result admission threshold</td></tr>
<tr><td align="center">✅</td><td>WebAssembly (wasm32) support</td><td>Core and `cli` builds are CI-gated: <code>cargo build --lib --bins --target wasm32-unknown-unknown --no-default-features</code> and <code>--features cli</code>. wasm uses oxcache's real L1 cache (ByteWeightCache); <code>wasm-api</code> exports <code>evaluate</code>/<code>list_functions</code> for browser/Node (build-gated only, runtime behavior not promised); the <code>fx</code>/<code>http</code>/<code>mcp</code>/<code>icu</code>/<code>time</code>/<code>unit</code> feature combos are out of scope</td></tr>
<tr><td align="center">✅</td><td>Numerical capability growth (v0.2.0-rc.3)</td><td>Special functions <code>erfc</code>/<code>lgamma</code>/<code>polylog</code>, numerics <code>derivative</code> (central differences + Richardson) and <code>lstsq</code> (QR with SVD pseudo-inverse fallback), alongside <code>root</code>/<code>integral</code> and the Bessel/Airy/<code>beta</code>/<code>zeta</code> family — wired into domain routing, the function catalog and the trait API; further frontier items tracked in the <a href="docs/archive/RESEARCH_ANALYSIS.md">research analysis</a></td></tr>
</table>

</details>

---

## 🤝 Contributing

For the detailed workflow and coding standards see the [🤝 Contributing Guide](docs/CONTRIBUTING.md).

### 🛠️ Development Environment

The toolchain requires Rust 1.97.1+ (baseline pinned by the CI MSRV job); before committing run `cargo fmt --all -- --check` and `cargo clippy --all-targets --all-features -- -D warnings`; `.githooks/pre-commit` provides 9 checks, enabled via `git config core.hooksPath .githooks`; commit messages follow Conventional Commits (`feat`, `fix`, `docs`, etc.). For full environment setup steps see the [🤝 Contributing Guide · Environment Setup](docs/CONTRIBUTING.md#-环境准备).

### 💖 Ways to Contribute

<table style="width:100%; border-collapse: collapse">
<tr>
<td width="33%" align="center" style="padding: 16px">

### 🐛 Report a Bug

Found a problem?<br>
<a href="https://github.com/kirky-x/calnexus/issues/new">Open an Issue</a>

</td>
<td width="33%" align="center" style="padding: 16px">

### 💡 Suggest a Feature

Have an idea?<br>
<a href="https://github.com/kirky-x/calnexus/issues/new">Open an Issue</a><br>
<span style="color:#64748B">Discussions are not enabled on this repo — please submit via Issue</span>

</td>
<td width="33%" align="center" style="padding: 16px">

### 🔧 Submit a PR

Want to contribute code?<br>
<a href="https://github.com/kirky-x/calnexus/pulls">Fork & Open a PR</a>

</td>
</tr>
</table>

When filing an issue, please include reproduction steps, your `calnexus` version, and OS information; for symbolic-calculus / precision bugs attach a minimal reproducing expression.

---

## 📋 Changelog

Full version history in the [📋 Changelog](docs/CHANGELOG.md) (following [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), semantic versioning).

<details>
<summary>📋 Table</summary>

| Version | Date | Highlights |
|---------|------|------------|
| 0.2.0-rc.1 | unreleased | Fluent i18n migration, logging switched to inklog, E2E scenario suite, dependencies moved to crates.io, Commons Clause license condition (see the [CHANGELOG](docs/CHANGELOG.md) Unreleased section) |
| 0.1.4 | 2026-07-26 | Release pipeline closed loop: automated `cargo publish` + pre-release security gates (`cargo audit`, version verification, dry-run validation) |
| 0.1.3 | 2026-07-26 | Added time / unit / fx optional domains and 8 vector operations; nondeterministic function cache bypass |
| 0.1.2 | 2026-07-21 | 20 doc-code consistency fixes; parser & canonicalizer boundary fixes (NaN total order, `0^0`, consecutive-operator validation) |

</details>

---

## 📄 License

This project is licensed under the [MIT License](LICENSE) with the additional [Commons Clause](LICENSE) condition (commercial use requires separate authorization). Copyright © 2026 Kirky.X🌠.

---

## 🙏 Acknowledgments

### 🌟 Core Dependencies

CalNexus stands on the shoulders of these excellent open-source projects:

<details>
<summary>📋 Table</summary>

| Dependency | Purpose |
|------------|---------|
| [mathexpr](https://crates.io/crates/mathexpr) | Expression parsing foundation |
| [oxcache](https://crates.io/crates/oxcache) | In-house high-performance cache library (`byte-weight` sync byte-weight cache, wrapping moka::sync) |
| [clap](https://github.com/clap-rs/clap) | CLI argument parsing (indirect via sdforge/cli) |
| [rustyline](https://github.com/kkawakam/rustyline) | REPL line editing and Tab completion |
| [rayon](https://github.com/rayon-rs/rayon) | Data-parallel batch evaluation |
| [jiff](https://github.com/BurntSushi/jiff) | Date/time and IANA time zones (`time` feature) |
| [nalgebra](https://github.com/dimforge/nalgebra) | Numerical linear algebra (`numerical` feature) |
| [sdforge](https://crates.io/crates/sdforge) | CLI / HTTP / MCP interface facade |
| [criterion](https://github.com/bheisler/criterion.rs) | Benchmarking |

</details>

### 💝 Special Thanks

Thanks to the Rust community and all [contributors](https://github.com/kirky-x/calnexus/graphs/contributors).

---

## 📞 Contact & Support

<table style="width:100%; max-width: 600px">
<tr>
<td align="center" width="33%">
<a href="https://github.com/kirky-x/calnexus/issues"><b style="color:#991B1B">Issues</b></a><br>
<span style="color:#64748B">Report problems and bugs</span>
</td>
<td align="center" width="33%">
<a href="docs/FAQ.md"><b style="color:#1E40AF">📖 FAQ</b></a><br>
<span style="color:#64748B">Browse the docs for answers (Discussions not enabled)</span>
</td>
<td align="center" width="33%">
<a href="https://github.com/kirky-x/calnexus"><b style="color:#1E293B">GitHub</b></a><br>
<span style="color:#64748B">Browse the source</span>
</td>
</tr>
</table>

---

## ⭐ Star History

[![Star History Chart](https://api.star-history.com/svg?repos=kirky-x/calnexus&type=Date)](https://star-history.com/#kirky-x/calnexus&Date)

If this project helps you, please consider giving it a ⭐️!

**Built by Kirky.X**

---

<sub>© 2026 Kirky.X. All rights reserved.</sub>
