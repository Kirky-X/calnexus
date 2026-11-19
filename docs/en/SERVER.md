# 🖥️ CalNexus Server Guide

> This document covers running, deploying, observing, and integrating MCP in server mode.
> Server mode requires `--features server` (`server = http + mcp`).

## 📋 Table of Contents

- [🚀 Quick Start](#-quick-start)
- [🌐 HTTP API and Operational Endpoints](#-http-api-and-operational-endpoints)
- [🚢 Deployment](#-deployment)
  - [Running Locally](#running-locally)
  - [Docker](#docker)
  - [Kubernetes Essentials](#kubernetes-essentials)
- [📈 Observability](#-observability)
- [🔌 MCP Integration](#-mcp-integration)
  - [Client Configuration](#client-configuration)
  - [Tools](#tools)
  - [Argument Wrapping Convention (Important)](#argument-wrapping-convention-important)
  - [Discovering Available Functions](#discovering-available-functions)
- [🚨 Error Semantics (SLO-Friendly)](#-error-semantics-slo-friendly)
- [📚 Related Documents](#-related-documents)
- [📈 /metrics Output Contract](#metrics-output-contract)

---

## 🚀 Quick Start

```bash
cargo install calnexus --features server
calnexus --serve-http                    # default 127.0.0.1:3000
calnexus --serve-http --bind 0.0.0.0:8080
```

---

## 🌐 HTTP API and Operational Endpoints

| Endpoint | Method | Description |
|------|------|------|
| `/api/v1/evaluate` | POST | Expression evaluation (JSON request/response) |
| `/api/v1/list_functions` | POST | Runtime function catalog (including feature-gated domain visibility) |
| `/api/v1/explain` | POST | Step-by-step derivation of an expression |
| `/api/v1/to_latex` | POST | Convert an expression to LaTeX |
| `/api/v1/verify` | POST | Verify expression equivalence, with numeric counterexample |
| `/api/v1/lint` | POST | Static expression diagnostics |
| `/api/v1/fx_budget` | POST | Convert a study-abroad budget into the home currency with a ±3% FX risk band (requires `fx` + `mcp` features) |
| `/api/v1/fx_pricing` | POST | Compute a safe selling price per currency from cost and target margin (requires `fx` + `mcp` features) |
| `/health` | GET | Combined health check (includes L1 cache status) |
| `/live` | GET | Liveness probe (pure process-alive semantics) |
| `/ready` | GET | Readiness probe (dependency-ready semantics) |
| `/metrics` | GET | Metrics export (Prometheus text; `?format=json` returns JSON) |

**Language negotiation**: the `evaluate`, `fx_budget`, and `fx_pricing` request bodies all support an optional `lang` field (BCP-47 tag, e.g. `"en"`/`"zh-CN"`). Missing or unknown values fall back to English (the protocol default); with `"zh"`, human-readable messages such as error text and fx risk hints switch to Chinese, while machine-readable fields (the `code` error code etc.) keep the English contract. MCP tool args support `lang` symmetrically:

```bash
curl -X POST localhost:3000/api/v1/evaluate -H 'content-type: application/json' \
  -d '{"expr": "foo + 1", "lang": "zh"}'
# → {"code":"BAD_REQUEST","message":"求值错误: 未绑定变量: foo","trace_id":"...",...}
```

The `docs` feature serves Swagger UI (`/swagger-ui/`, trailing slash required — routes are mounted with axum exact matching, `GET /swagger-ui` returns 404); note that `/api-docs/openapi.json` currently returns an empty spec (`#[forge]`'s OpenAPI route registration is gated on the downstream crate's `openapi` feature name, which CalNexus does not define — a known gap; fixing it also requires addressing the sdforge macro's coarse-grained request-body schema mapping).

---

## 🚢 Deployment

### Running Locally

```bash
calnexus --serve-http                # default 127.0.0.1:3000
calnexus --serve-http --bind 0.0.0.0:8080
```

### Docker

```bash
docker build -t calnexus:latest .
docker run -p 3000:3000 calnexus:latest
```

The container is a multi-stage distroless non-root image; `CALNEXUS_BIND_ADDR` defaults to `0.0.0.0:3000` (when exposing publicly, place it behind a reverse proxy / Ingress).

### Kubernetes Essentials

- **readinessProbe**: `GET /ready` (dependency-ready semantics: the in-process cache is readable)
- **livenessProbe**: `GET /live` (pure process-alive semantics)
- **terminationGracePeriodSeconds**: ≥ 45 (request-level timeout 30s + drain headroom 15s; the server's drain timeout is fixed at 30s, after which the process force-exits)
- **Resources**: evaluation is CPU-intensive; scaling replicas by core count beats enlarging the cache; the cache byte budget is controlled via `CALNEXUS_CACHE_SIZE` (entries × 4KB approximation)

Graceful shutdown is built in with the `http` feature: SIGTERM/Ctrl+C → drain for at most 30s → force exit.

---

## 📈 Observability

- `GET /metrics`: Prometheus text (the `calnexus_cache_*` cache families + sdforge metrics' `sdforge_http_requests_total{route,method,status}` and the latency histogram); the `?format=json` debug branch includes `http_route_count`. Note that the route label takes the request path, so high-cardinality REST paths containing ids will multiply the series
- `X-Request-ID`: passed through inbound or auto-generated (`req-*`), written back on the response; `traceparent` is passed through as well (trace_id extracted and written back as `X-Trace-ID`)
- With the `observability` feature build, `RUST_LOG` controls the log level (default warn)
- The `ratelimit` feature provides fixed-window per-IP rate limiting: `CALNEXUS_RATELIMIT_LIMIT` (default 120), `CALNEXUS_RATELIMIT_WINDOW_SECS` (default 60), probes and metrics exempt

---

## 🔌 MCP Integration

`calnexus --serve-mcp` exposes MCP tools over stdio transport, stateless, and can be connected from Claude Desktop, Cursor, or any MCP client.

### Client Configuration

`claude_desktop_config.json` for Claude Desktop:

```json
{
  "mcpServers": {
    "calnexus": {
      "command": "calnexus",
      "args": ["--serve-mcp"]
    }
  }
}
```

> `calnexus` must be on PATH (`cargo install calnexus --features cli`), or replace it with an absolute path.
> Builds with the `fx` feature additionally register the `fx_budget`/`fx_pricing` tools.

Generic stdio client:

```json
{
  "mcpServers": {
    "calnexus": {
      "command": "/usr/local/bin/calnexus",
      "args": ["--serve-mcp"],
      "env": {}
    }
  }
}
```

### Tools

| Tool | Args (req-wrapped) | Returns |
|------|------------------|------|
| `evaluate` | `expr` (required), `vars` (optional), `precision` (optional), `lang` (optional) | `{result, domain, cache}` |
| `list_functions` | none (`{"req": {}}`) | Function catalog grouped by domain (including feature-gated domain visibility) |
| `explain` | `expr`, `vars`?, `lang`? | `{v, steps}` derivation steps |
| `to_latex` | `expr`, `lang`? | `{v, latex}` |
| `verify` | `lhs`, `rhs`, `var`?, `sample_points`?, `lang`? | `{v, equal, counterexample?}` |
| `lint` | `expr`, `lang`? | `{v, findings}` with stable diagnostic codes |
| `fx_budget` * | `tuition`, `tuition_currency`, `home_currency`, `living_cost_monthly`?, `duration_years` | Home-currency cost + ±3% risk band + `rate_date` + `rate_source` |
| `fx_pricing` * | `cost_cny`, `target_profit_rate`, `currencies`, `platform_rate`?, `safety_buffer`? | Suggested selling price per currency + `rate_date` + `rate_source` |

\* Requires an `fx` feature build.

### Argument Wrapping Convention (Important)

All arguments must be wrapped in a `req` object:

```json
{"req": {"expr": "diff(x^2, x)"}}
{"req": {"expr": "x+1", "vars": {"x": 10}}}
```

Unwrapped `{"expr": "..."}` is rejected by schema validation.

### Discovering Available Functions

Call the `list_functions` tool (or the CLI `calnexus --list-functions`, or HTTP `POST /api/v1/list_functions`) to get the full function catalog of the current build; feature-gated domains (time/unit/fx) appear only when enabled.

---

## 🚨 Error Semantics (SLO-Friendly)

The server's three-way error classification and the CLI exit codes come from the same `ErrorKind` contract; note that 422 comes from a separate request-validation layer, not from evaluation errors:

| HTTP (`code` field) | MCP (`type` field) | Meaning | Trigger |
|------|-----|------|----------|
| 400 `BAD_REQUEST` | `InvalidInput` | Evaluation/parsing failure | The 9 evaluation-class `ErrorKind`s (Parse / Eval / Overflow / DivisionByZero / Domain / Depth / NaNOrInf / UndefinedSymbol / Usage), message prefixed with the kind name |
| 422 `UNPROCESSABLE_ENTITY` | `ValidationError` | Request-body parameter validation failure | DTO validation layer (`server/types.rs::validate()`): expr ≤ 4096 chars, vars ≤ 1024 keys, precision ≤ 10000; intercepted before evaluation, the HTTP body's `field` names the offending field |
| 503 `UNAVAILABLE` | `ServiceUnavailable` | Server/upstream failure | `Timeout` / `DependencyUnavailable` (fx upstream unreachable); `retry_after` exists only in `ApiError`/`ServiceError` details, never in the HTTP body or response headers |

The HTTP error body is the sdforge unified contract `{"code","message","trace_id","field"}` (`trace_id` from the request context; `field` only on validation errors); MCP keeps `ApiError` serialization (`type` is the serde variant name: `InvalidInput` / `ValidationError` / `ServiceUnavailable`).

CLI exit codes (the `ErrorKind::exit_code()` contract): **1** = computation error, **2** = usage error (`Usage`), **3** = timeout/upstream unavailable. The 422 validation layer exists only in the HTTP/MCP forms; the corresponding constraints on the CLI form are enforced by clap / `--precision` validation.

---

## 📚 Related Documents

- [📖 User Guide](USER_GUIDE.md) — Environment variables and CLI flags
- [📈 Performance Guide](PERFORMANCE.md) — Cache budget and capacity planning
- [🔒 Security](SECURITY.md) — Server-mode security best practices
- [📘 API Reference](API_REFERENCE.md) — Error contract and the full `ErrorKind` table
- [🏗️ Architecture](ARCHITECTURE.md) — server module and data flow

---

## 📈 /metrics Output Contract

> This section was migrated from the README's "🌐 Server Mode" section.

The `/metrics` contract: the Prometheus text carries the three cache families `calnexus_cache_{hits,misses,entries}` (with `# HELP` / `# TYPE` comment lines) plus the `sdforge_http_requests_total{route,method,status}` counter family and the `sdforge_http_request_duration_seconds` latency histogram family; the `?format=json` debug branch carries exactly the four keys `hits` / `misses` / `entry_count` / `http_route_count`. The `route` label takes the request path (not the route template), so high-cardinality REST paths containing ids will multiply the series — relevant for local/intranet deployments.
