English | [中文](README_CN.md)

<div align="center">

<img src="docs/assets/logo.png" alt="CalNexus Logo" height="240">

[![Rust 2024](https://img.shields.io/badge/Rust-2024-edded?logo=rust&style=for-the-badge)](https://www.rust-lang.org/) [![Rustc 1.97.1+](https://img.shields.io/badge/Rustc-1.97.1+-orange.svg?style=for-the-badge)](https://www.rust-lang.org/) [![crates.io](https://img.shields.io/crates/v/calnexus?style=for-the-badge)](https://crates.io/crates/calnexus) [![docs.rs](https://img.shields.io/docsrs/calnexus?style=for-the-badge)](https://docs.rs/calnexus) [![downloads](https://img.shields.io/crates/d/calnexus?style=for-the-badge)](https://crates.io/crates/calnexus) [![CI](https://img.shields.io/github/actions/workflow/status/kirky-x/calnexus/ci.yml?branch=main&label=CI&style=for-the-badge)](https://github.com/kirky-x/calnexus/actions/workflows/ci.yml) [![coverage](https://img.shields.io/badge/coverage-95.9%25%20(llvm--cov%20lines)-brightgreen?style=for-the-badge)](https://github.com/kirky-x/calnexus) [![license](https://img.shields.io/crates/l/calnexus?style=for-the-badge)](LICENSE)

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

Beyond the core capabilities listed above, LaTeX / steps / canonical formatters, bilingual (EN/ZH) error messages (embedded Fluent catalogs, always available regardless of the `icu` feature), a runtime function catalog (`--list-functions`), multi-platform prebuilt binaries, and container images are also available; see [🏗️ Architecture · Feature Flags Quick Reference](docs/en/ARCHITECTURE.md#-feature-flags-quick-reference) for the full mapping to `Cargo.toml`'s `[features]`.

---

## 🚀 Quick Start

### 📦 Installation

<details>
<summary>📄 Code · bash</summary>

```bash
cargo install calnexus --features cli
```

</details>

Requires Rust 1.97.1 or later (MSRV, matching the CI MSRV job). The core library has `default = []` with zero dependencies; enable `cli` for the command-line experience. Install commands for every feature combo (optional domains, numerical linear algebra, HTTP server, etc.) are unified in [🏗️ Architecture · Feature Flags Quick Reference](docs/en/ARCHITECTURE.md#-feature-flags-quick-reference).

Prebuilt binaries and building from source are covered in [📖 User Guide · Installation](docs/en/USER_GUIDE.md#-installation); the wasm32 support scope is in [❓ FAQ · Troubleshooting](docs/en/FAQ.md#-troubleshooting).

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

For the core concepts (domain routing, the evaluation pipeline, the three execution modes, feature gating, and the JSON contract) see [📖 User Guide · Core Concepts](docs/en/USER_GUIDE.md#-core-concepts).

---

## 📚 Documentation

<details>
<summary>📋 Table</summary>

| Document | Description |
|----------|-------------|
| [📖 User Guide](docs/en/USER_GUIDE.md) | Complete tutorial from installation to advanced usage |
| [📘 API Reference](docs/en/API_REFERENCE.md) | Expression evaluation API and direct API (`CalNexus` facade) |
| [🏗️ Architecture](docs/en/ARCHITECTURE.md) | Design principles, module layout, and data flow |
| [📈 Performance Guide](docs/en/PERFORMANCE.md) | Benchmark data, measurement methodology, and tuning tips |
| [🔒 Security](docs/en/SECURITY.md) | Security design, best practices, and vulnerability handling |
| [❓ FAQ](docs/en/FAQ.md) | Frequently asked questions |
| [🖥️ Server Guide](docs/en/SERVER.md) | HTTP deployment, observability, and MCP integration |
| [🧪 Test Scenario Matrix](docs/en/TEST_SCENARIOS.md) | Exhaustive scenario matrix of the test suites (unit/integration/E2E/property/fuzz/bench) |
| [📋 Changelog](docs/en/CHANGELOG.md) | Release notes for every version |
| [🤝 Contributing](docs/en/CONTRIBUTING.md) | How to contribute to the project |
| [📦 Online API docs](https://docs.rs/calnexus) | Latest docs.rs-generated documentation |
| [📦 crates.io](https://crates.io/crates/calnexus) | Publication page |

</details>

Design & process documents: [Code of Conduct](docs/en/CODE_OF_CONDUCT.md)

---

## 💻 Examples

Evaluation examples are provided as inline command-line invocations — copy them into a terminal and run. The ones below cover arithmetic, scientific functions, number theory, arbitrary precision, symbolic calculus, and implicit multiplication; for full REPL sessions (`:let` / `:vars` / Tab completion), rayon-parallel batch runs (`--batch`), and the functions and usage of the optional domains (time / unit / fx / numerical linear algebra), see the [📖 User Guide](docs/en/USER_GUIDE.md) and the [📖 User Guide · Domain Guide](docs/en/USER_GUIDE.md#-domain-guide).

### 🧰 CLI Flags

The behavior, output formats, and exit-code conventions of all flags — `--repl`, `--batch`, `--var`, `--precision`, `--timeout`, `--cache-size`, `--json`, `--latex` / `--canonical` / `--steps`, `--explain`, `--lang`, `--list-functions`, `--serve-http` / `--serve-mcp`, `--bind` — are documented in [📖 User Guide · CLI Usage](docs/en/USER_GUIDE.md#-cli-usage), or run `calnexus --help`.

---

## 🌐 Server Mode

The endpoint list and language negotiation, Docker / Kubernetes deployment, observability and `ratelimit` throttling, MCP client configuration and argument conventions, error semantics (400 / 422 / 503 and the CLI exit-code contract), and graceful shutdown are covered in the [🖥️ Server Guide](docs/en/SERVER.md).

---

## 🏗️ Architecture

The per-layer responsibilities and module list, mermaid architecture and evaluation-sequence diagrams, interface-isolation traits, security and performance design, and the ADR decision records are documented in the [🏗️ Architecture Document](docs/en/ARCHITECTURE.md).

---

## 🧪 Testing

### 🎯 Test Strategy

The test pyramid covers nine layers: inline unit tests in `src/`, integration tests organized by shape (`tests/`: integration, cli_integration, repl_integration, server_http_integration, server_mcp_integration, api_integration, api_trait_dispatch, time_unit_fx_integration, numerical_linalg_test, kit_module_test, performance_tests), the `tests/e2e` scenario suite (no `required-features`; internal `#[cfg]` gates cover both the enabled and disabled sides), property tests (proptest), snapshot tests (insta), security tests (DoS vectors and boundary attacks), fuzz testing (`fuzz/`, 7 cargo-fuzz targets), Criterion benchmarks (`benches/`, 4 suites), and doc tests of public APIs. The per-suite exhaustive scenario matrix, file mapping, and pass criteria are in the [🧪 Test Scenario Matrix](docs/en/TEST_SCENARIOS.md).

Run commands (matching CI) are in [🧪 Test Scenario Matrix · Run Commands](docs/en/TEST_SCENARIOS.md#-run-commands-matching-ci); the current test-scale snapshot (v0.2.0) is in [🧪 Test Scenario Matrix · Test Scale](docs/en/TEST_SCENARIOS.md#-test-scale-v020).

---

## 📊 Performance

Baselines were collected locally on a development machine (WSL2, linux 6.6) using criterion median estimates: cache hits sit in the low-microsecond range independent of expression complexity (`2+3` ≈ 0.90 µs), roughly a 12x win over the full pipeline (`2+3` ≈ 10.7 µs), with parsing and canonicalization in the sub-microsecond range; real-world performance depends on expression complexity and hardware — reproduce with `cargo bench --features cli`. The full baseline tables and measurement methodology are in [📈 Performance Guide · Baselines](docs/en/PERFORMANCE.md#-baseline-data); design points such as serialization-free hits, single-flight, the byte-weight budget, the large-result admission threshold, and nondeterministic bypass are in [📈 Performance Guide · Cache Design](docs/en/PERFORMANCE.md#-cache-design).

---

## 🔒 Security

### 🛡️ Security Design

CalNexus is an evaluator with no network access (except the `fx` upstream), no untrusted file I/O, and no plugin loading — the practical attack surface is minimal. Active defenses center on a closed recursion-depth protection loop (`MAX_AST_DEPTH=256` + iterative pre-check + RAII guards), request resource limits, parser error sanitization, and fx upstream hardening. Code-level mechanism details are in [🏗️ Architecture Document · Security Design](docs/en/ARCHITECTURE.md#-security-design); the supported-versions table, vulnerability process, and hardening best practices are in the [🔒 Security Document](docs/en/SECURITY.md).

### 🚨 Reporting a Vulnerability

Please do **not** report security vulnerabilities through public issues. Use the GitHub [Security Advisories](https://github.com/kirky-x/calnexus/security/policy) private disclosure channel as the primary route (fall back to email Kirky-X@outlook.com, handled by Kirky.X, only if that channel is unavailable). We commit to acknowledging within 48 hours and providing an initial assessment within 7 days. Full policy in [SECURITY.md](docs/en/SECURITY.md).

---

## 🗺️ Roadmap

Delivered milestones are in the [📋 Changelog · Development Roadmap](docs/en/CHANGELOG.md#-development-roadmap).

---

## 🤝 Contributing

For the detailed workflow and coding standards see the [🤝 Contributing Guide](docs/en/CONTRIBUTING.md).

---

## 📋 Changelog

Full version history in the [📋 Changelog](docs/en/CHANGELOG.md) (following [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), semantic versioning).

---

## 📄 License

This project is licensed under the [MIT License](LICENSE) with the additional [Commons Clause](LICENSE) condition (commercial use requires separate authorization). Copyright © 2026 Kirky.X🌠.

---

## 🙏 Acknowledgments

The core dependency list is in [🏗️ Architecture · Core Dependencies](docs/en/ARCHITECTURE.md#-core-dependencies).

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
<a href="docs/en/FAQ.md"><b style="color:#1E40AF">📖 FAQ</b></a><br>
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
