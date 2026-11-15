# 🔒 CalNexus Security

> This document describes CalNexus's supported versions, vulnerability reporting process, security design overview, and usage best practices.

## 📋 Table of Contents

- [📌 Supported Versions](#-supported-versions)
  - [Minimum Supported Rust Version (MSRV)](#minimum-supported-rust-version-msrv)
  - [Dependency Security](#dependency-security)
- [🐛 Vulnerability Reporting Process](#-vulnerability-reporting-process)
  - [How to Report](#how-to-report)
  - [What to Include](#what-to-include)
  - [Our Commitments](#our-commitments)
  - [Disclosure Timeline](#disclosure-timeline)
- [🛡️ Security Design Overview](#-security-design-overview)
  - [Attack Surface](#attack-surface)
  - [Resource and Depth Protections](#resource-and-depth-protections)
  - [Error Message Sanitization](#error-message-sanitization)
  - [Network Upstream Hardening (fx)](#network-upstream-hardening-fx)
  - [Error Semantics Tiering](#error-semantics-tiering)
- [✅ Security Best Practices](#-security-best-practices)
  - [For Library Users](#for-library-users)
  - [For Server Deployments](#for-server-deployments)
- [📚 Related Documents](#-related-documents)

---

## 📌 Supported Versions

CalNexus is in the 0.x stage and provides security fixes only for the **latest minor version line**:

| Version | Support status |
|------|----------|
| 0.x (latest minor) | ✅ Security updates supported |
| Older 0.x versions | ❌ Not supported |

Before reporting a vulnerability, please upgrade to the most recent release if possible (see [Releases](https://github.com/kirky-x/calnexus/releases)).

### Minimum Supported Rust Version (MSRV)

The MSRV is **Rust 1.97.1** (aligned in three places: the repo's `Cargo.toml` `rust-version` and the CI MSRV job). Older toolchains no longer receive security fixes; keep your toolchain updated.

### Dependency Security

```bash
# Run security audits (RustSec advisory scanning)
cargo audit

# Licenses / banned dependencies / source verification
cargo deny check
```

- All dependencies disable default features and enable only what is needed, shrinking the transitive dependency surface.
- A [weekly scheduled workflow](https://github.com/kirky-x/calnexus/actions/workflows/audit.yml) runs `cargo audit` + `cargo deny` automatically, and dependabot tracks dependency updates.
- [CodeQL static security analysis](https://github.com/kirky-x/calnexus/actions/workflows/codeql.yml) (push/PR triggered + weekly schedule Sundays 00:00 UTC).
- The release pipeline has a pre-release `cargo audit --deny warnings` gate and version-consistency verification.
- All third-party GitHub Actions are pinned to commit SHAs.

---

## 🐛 Vulnerability Reporting Process

### How to Report

**Do not report security vulnerabilities through public GitHub Issues.**

**Preferred channel**: submit a report through the GitHub [Security Advisories](https://github.com/kirky-x/calnexus/security/policy) private disclosure channel (repo page **Security → Report a vulnerability**).

**Fallback channel**: only if the preferred channel is unavailable, email [Kirky-X@outlook.com](mailto:Kirky-X@outlook.com) (handled by Kirky.X).

We follow the responsible disclosure model and ask reporters to do the same: please do not disclose the issue publicly before the fix is released and confirmed by you.

### What to Include

An ideal report contains:

- A clear description of the vulnerability and its potential impact
- Step-by-step reproduction instructions, including the **exact expression or input** that triggers the issue
- The CalNexus version and platform used for testing
- Fix suggestions (if any)

### Our Commitments

- **Within 48 hours**: acknowledge receipt of the report
- **Within 7 days**: provide an initial assessment (confirmed or not, severity)
- **As soon as severity allows**: prepare and release a fix, keeping you updated
- **After the fix ships**: coordinate public disclosure with you and credit your contribution (anonymously if you prefer)

### Disclosure Timeline

1. Report arrives → acknowledged within 48h
2. Assessment and grading → initial conclusion within 7 days
3. Fix development and verification (including regression tests + fuzz target coverage)
4. Release → coordinated public disclosure and credit

---

## 🛡️ Security Design Overview

### Attack Surface

CalNexus is a **command-line math expression evaluator**: no networking (except the `fx` rate upstream), no untrusted-path file I/O, no external plugin loading — the practical attack surface is minimal. Security reviews focus on:

- **Expression injection / untrusted input**: crafted malicious expressions causing panic, infinite loops, or resource exhaustion
- **Depth overflow / recursion limits**: deeply nested expressions blowing the parser/evaluator stack or exhausting memory; any means of bypassing the depth limit is a security-grade defect
- **Numeric boundaries**: numeric anomalies with security impact, such as overflow or NaN propagation

Findings outside these categories but with security impact are equally welcome — better a false alarm than a miss.

### Resource and Depth Protections

- **Closed recursion-depth protection loop**: `MAX_AST_DEPTH=256`, iterative bracket pre-check before mathexpr recursion, thread-local RAII guard for bracket literals, canonicalizer depth guard. The historical DoS gap (nested list/matrix literals at ~1300 levels bypassing the depth check) is fixed, with new fuzz targets such as `list_depth_fuzz` guarding it.
- **Request resource limits**: expr ≤ 4096 chars, vars ≤ 1024 keys, precision ≤ 10000 digits; known DoS vectors (factorial/pow/precision) have dedicated constant constraints; `--timeout` backstops accumulated slow operations.
- **Fuzz testing**: 7 cargo-fuzz targets continuously cover parser, ast_depth, list_depth, canonicalizer, cache_key, numeric_boundary, matrix_dim.
- **Security tests**: `tests/security_tests.rs` covers DoS vectors and boundary attacks.

### Error Message Sanitization

Parse errors never leak winnow's internal Debug structures; error messages are sanitized through the i18n layer, and both `--explain` and the JSON error object expose only contract fields such as `kind` / `message` / `hint` / `span`.

### Network Upstream Hardening (fx)

The fx rate domain is the only network surface. Hardening measures: ureq `https_only` (no plaintext), 1MB response-body cap, atomic disk-cache writes (temp + rename + 0600 permissions), fetch single-flight, and a three-state synchronous circuit breaker (consecutive-failure threshold / cooldown, configurable via `CALNEXUS_FX_BREAKER_*`).

### Error Semantics Tiering

`ErrorKind::DependencyUnavailable` (exit_code=3) is strictly distinguished from client input errors: in server deployments they map to 503 (`code=UNAVAILABLE`) and 400 respectively, preventing upstream failures from being misjudged as client problems and polluting caller retry and alerting policies.

---

## ✅ Security Best Practices

### For Library Users

- When handling **untrusted input**, always enable `--timeout` / `CALNEXUS_TIMEOUT` and keep depth/resource limits at their defaults (the limits are security lines, not tuning knobs to relax freely).
- Prefer injecting variables via `EvalContext` / `--var` rather than string-concatenating expressions.
- Watch RustSec advisories and keep dependencies and the toolchain updated (`cargo audit` can be integrated into CI).

### For Server Deployments

- The server listen address defaults to `127.0.0.1:3000`; when exposing publicly, place it behind a reverse proxy / Ingress before binding `0.0.0.0`.
- Set a memory budget for containers and configure `CALNEXUS_CACHE_SIZE` accordingly; K8s `terminationGracePeriodSeconds` ≥ 45 (drain 30s + headroom).
- Enable the `ratelimit` feature for rate limiting (probes and metrics are automatically exempt), feed `/metrics` into monitoring, and distinguish 503 (`code=UNAVAILABLE`) from 429 (with `Retry-After`) to tell upstream failures from throttling.
- See the [🖥️ Server Guide](SERVER.md) for details.

---

## 📚 Related Documents

- [🏗️ Architecture](ARCHITECTURE.md) — Code-level details of the protection mechanisms
- [🖥️ Server Guide](SERVER.md) — Server-mode security deployment
- [📋 Changelog](CHANGELOG.md) — Security fix records
