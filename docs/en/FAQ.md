# ❓ CalNexus FAQ

> Frequently asked questions. For the usage tutorial see the [📖 User Guide](USER_GUIDE.md); if your question is not covered here, feel free to [open an Issue](https://github.com/kirky-x/calnexus/issues/new).

## 📋 Table of Contents

- [🧭 General Questions](#-general-questions)
- [📦 Installation and Features](#-installation-and-features)
- [🛠️ Usage and Features](#-usage-and-features)
- [⚡ Performance](#-performance)
- [🌐 Server Mode](#-server-mode)
- [🔧 Troubleshooting](#-troubleshooting)

---

## 🧭 General Questions

### ❓ What is CalNexus?

A command-line math expression evaluator written in Rust: 11 core computation domains (arithmetic, scientific functions, statistics, precision, number theory, combinatorics, polynomial, complex, matrix, vector, symbolic calculus) + 3 optional computation domains (time / unit / fx), unified behind a single parser and a priority-routed domain dispatcher, offering single-expression, REPL, and parallel batch modes plus HTTP / MCP dual-protocol services.

### ❓ Is CalNexus production-ready?

Yes. The core library uses `default = []` (no optional feature dependencies; all dependencies disable default features); 3000+ tests (authoritative count via `grep -rE "#\[(tokio::)?test\]" src/ tests/ | wc -l`), the 90.4% line coverage gate (llvm-cov), zero warnings across feature combinations, the CI six-leg test matrix, and server-grade error semantics (400/422/503) support production deployments. For deployment guidance see the [🖥️ Server Guide](SERVER.md).

### ❓ Which platforms are supported?

linux x86_64/aarch64 (musl static), macOS x86_64/aarch64, and windows x86_64 have prebuilt binaries; other platforms can build from source. wasm32 supports library/binary builds for the core and `cli` features (CI-gated), using oxcache's real L1 cache (`ByteWeightCache`) with language detection falling back to `en`; runtime usability is not promised (see the support scope below).

### ❓ What license does the project use?

MIT + Commons Clause (commercial use requires separate authorization). See [LICENSE](../../LICENSE).

### ❓ How can I contribute?

See the [🤝 Contributing Guide](CONTRIBUTING.md). Do not report security vulnerabilities through public Issues — see the reporting process in [🔒 Security](SECURITY.md).

---

## 📦 Installation and Features

### ❓ How do I install it?

```bash
cargo install calnexus --features cli     # CLI (recommended)
cargo add calnexus                        # Library (zero-dependency core)
```

You can also download prebuilt binaries (with SHA256SUMS) from [GitHub Releases](https://github.com/kirky-x/calnexus/releases). For all installation methods see the [📖 User Guide](USER_GUIDE.md).

### ❓ How do I choose the right feature combination?

| Need | Combination |
|------|------|
| Embedded computation engine | No features (`default = []`, zero dependencies) |
| Everyday command line | `cli` |
| Time / unit / fx | `cli,time,unit,fx` |
| Numerical linear algebra | `cli,numerical` |
| REST / MCP service | `server` (+ `ratelimit`, `observability`, `docs` as needed) |
| Bilingual EN/ZH error messages | `icu` |

For the full feature matrix see [🏗️ Architecture · Feature Flags Quick Reference](ARCHITECTURE.md#-feature-flags-quick-reference).

### ❓ What are the system requirements?

Rust 1.97.1 or later (MSRV, guarded by a dedicated CI MSRV job). The core library has no extra system dependencies; the `server` feature needs the tokio runtime; building the HTTP integration tests from source requires protoc.

---

## 🛠️ Usage and Features

### ❓ Why doesn't `--precision 50 '1/3'` give true 50-digit precision?

`--precision N` enables BigRational mode, but the input expression first goes through the f64 parser, where precision is already lost. When you need precision that truly exceeds f64, use the `precision(N, expr)` function and keep the expression purely rational (integers plus `/`, `^`, etc.), e.g. `calnexus 'precision(50, 1/3)'`.

### ❓ Is implicit multiplication supported?

Yes. Math idioms like `2x`, `3(x+1)`, and `2(x+1)(x+2)` are recognized as multiplication automatically.

### ❓ Why do `now` / `today` / `fx` return different results each time / bypass the cache?

They are **nondeterministic functions**: before routing, the evaluator detects whether the AST contains such functions and, on a hit, skips both the cache read and the cache write, preventing time/fx results from being polluted by the cache. This is intentional design.

### ❓ Where does FX data come from, and what happens offline?

From frankfurter.dev (European Central Bank reference rates, roughly 30 major currencies) or the ExchangeRate-API open endpoint (aggregated source, 160+ currencies; `CALNEXUS_FX_PROVIDER=erapi`), via a 3-level cache (memory → file → network). Different sources show systematic small differences; with `auto` mixed sources the server response marks the actual origin with `rate_source`. Offline, the local file snapshot is preferred (bounded by `CALNEXUS_FX_TTL_HOURS`, default 24 hours); setting `CALNEXUS_FX_ALLOW_STALE` accepts expired snapshots; consecutive failures reaching `CALNEXUS_FX_BREAKER_THRESHOLD` (default 3) trip the circuit breaker, which cools down for `CALNEXUS_FX_BREAKER_COOLDOWN_SECS` (default 30 seconds). FX data is for reference only.

### ❓ Which commands does the REPL support?

`:help` for help, `:let name = value` to bind a variable, `:vars` to list variables, `:quit` to exit; Tab completion is based on the unified function catalog, and feature-gated functions appear automatically once enabled.

### ❓ How do I see which functions the current build supports?

`calnexus --list-functions` (CLI), the MCP `list_functions` tool, or HTTP `POST /api/v1/list_functions`. Feature-gated domains appear only when enabled.

---

## ⚡ Performance

### ❓ How fast is CalNexus?

Local criterion baselines: cache hits for simple expressions ≈ 0.90 µs; a full-pipeline miss (parse + canonicalize + evaluate + cache insert) ≈ 10.7 µs; parsing `2+3` ≈ 277 ns. For full data and methodology see the [📈 Performance Guide](PERFORMANCE.md).

### ❓ How does the cache work?

The BLAKE3 hash of the canonicalized AST is the key, implemented on oxcache `ByteWeightCache` (moka kernel): hits return a cloned `EvalResult` (serialization-free); concurrent identical keys evaluate only once via `try_get_with` single-flight; byte-weight budget: library API default 64MB, CLI default 10000 entries (≈40MB, configurable via `--cache-size`); results larger than 256KB are not cached. See the [📈 Performance Guide](PERFORMANCE.md) for details.

### ❓ Does batch evaluation run in parallel?

Yes. `--batch` uses rayon to evaluate lines in parallel while preserving line order, and appends a summary (total / ok / errors / cache hits / elapsed) at the end.

---

## 🌐 Server Mode

### ❓ How do I configure the HTTP listen address and rate limiting?

`--bind 0.0.0.0:8080` or the `CALNEXUS_BIND_ADDR` environment variable; the `ratelimit` feature provides fixed-window per-IP rate limiting, configurable via `CALNEXUS_RATELIMIT_LIMIT` (default 120) and `CALNEXUS_RATELIMIT_WINDOW_SECS` (default 60), with probes and metrics exempt. See the [🖥️ Server Guide](SERVER.md) for details.

### ❓ What are the error response semantics?

Three classes (HTTP body `code` field): 400 `BAD_REQUEST` (evaluation failures such as syntax errors and division by zero), 422 `UNPROCESSABLE_ENTITY` (request-body parameters over limits, intercepted before evaluation, HTTP/MCP forms only, `field` names the offending field), 503 `UNAVAILABLE` (timeout or fx upstream unreachable). MCP keeps the `ApiError` `type` protocol names (`InvalidInput` / `ValidationError` / `ServiceUnavailable`). CLI exit codes: 1 = computation error, 2 = usage error, 3 = timeout/unavailable.

### ❓ How do I connect CalNexus to Claude Desktop / Cursor?

Start the stdio MCP server with `--serve-mcp` and configure `mcpServers` in the client. For client configuration examples and the tool list see the [🖥️ Server Guide](SERVER.md).

---

## 🔧 Troubleshooting

### ❓ What is the wasm32 support scope?

The core and `cli` features can build and are protected by CI gates:

```bash
cargo build --lib --bins --target wasm32-unknown-unknown --no-default-features
cargo build --lib --bins --target wasm32-unknown-unknown --features cli
```

Limitations:

- Under wasm, oxcache's `ByteWeightCache` is used (a real L1 cache); oxcache's `minimal`/async backends are still out of the wasm support scope.
- The `wasm-api` feature can export `evaluate` / `list_functions` (wasm-bindgen), build-gated only; browser/Node runtime behavior is not promised.
- Language detection falls back to `en` because there are no environment variables or system APIs.
- The `fx` / `http` / `mcp` / `icu` / `time` / `unit` feature combinations are out of scope.
- Test and benchmark targets do not compile on wasm (the dev-dependency `getrandom 0.4` requires the `wasm_js` feature).

### ❓ Evaluation reports Timeout (exit code 3)?

The expression triggered the timeout (`--timeout` / `CALNEXUS_TIMEOUT`, in seconds). Check whether the expression contains huge powers/factorials/precision arguments; known DoS vectors (factorial/pow/precision) have dedicated constant constraints, so normal expressions should not time out.

### ❓ fx functions report 503 / DependencyUnavailable?

The fx upstream is unreachable and there is no usable local snapshot (or it is expired and `CALNEXUS_FX_ALLOW_STALE` is not set). Check the network; after the circuit breaker trips, wait for the cooldown to end.

### ❓ Why does a perfectly legal expression report a Parse error?

Note: consecutive operators (`++`, `**`, etc.) are explicitly rejected; string arguments need double quotes (`convert(1000, "m", "km")`); unbalanced brackets or exceeding the depth limit (256 levels) also error. Compare against the caret position indicator in the `--explain` output.

---

## 📚 Related Documents

- [📖 User Guide](USER_GUIDE.md) — Complete usage tutorial
- [📈 Performance Guide](PERFORMANCE.md) — Benchmark data and tuning tips
- [🖥️ Server Guide](SERVER.md) — Server-mode deployment and MCP integration
- [🔒 Security](SECURITY.md) — Security design and vulnerability reporting
- [🤝 Contributing Guide](CONTRIBUTING.md) — Contributing to development
