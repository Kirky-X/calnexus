# Changelog

All notable changes to CalNexus are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] - 2026-11-14

In-house dependencies moved from rc to stable pinning; official release. The public API and the `--json` contract (`v:1`) are unchanged relative to 0.2.0-rc.4.

### Fixed

- fx test race across CI parallel legs: `CALNEXUS_FX_*` are process-level environment variables, so the env mapping contract test (writer) and readers on the `rates()` path (`read_allow_stale()`/`ttl_seconds()` default-value dependencies) polluted each other when parallel scheduling overlapped — in practice `test_fetch_lock_concurrent_consistency` showed a CI-side false "always-failing fetcher got Ok(stale table)" verdict (not reproducible locally with serial `--test-threads=1`). Module-level `FX_ENV_LOCK` mutex: all tests that depend on fx env hold the lock for their whole run

### Changed

- All in-house dependencies converted to minor-precision stable pins (release orchestration requirement): `oxcache 0.5.0-rc.12 → 0.5` (released 0.5.0), `sdforge 0.5.0-rc.11 → 0.5` (released 0.5.0), `inklog 0.3.0-rc.12 → 0.3` (released 0.3.0), `limiteron 0.3.0-rc.10 → 0.3` (released 0.3.1), `confers 0.6.0-rc.13 → 0.6` (released 0.6.0), `trait-kit 0.5.0-rc.14 → 0.5` (released 0.5.0); no direct dbnexus dependency; no cross-major changes and no `--precise` rollbacks

### Ci

- CI infrastructure dependency bumps (dependabot, 5 items): `softprops/action-gh-release 3.0.4`, `Swatinem/rust-cache 2.9.3`, `zizmorcore/zizmor-action 0.8.1`, `github/codeql-action` (init/autobuild advanced to 9b32540); `.github/workflows/security.yml` gitleaks v3 now passes `GITHUB_TOKEN`, `ci.yml` all-leg adds http/mcp clippy coverage and wasm32 adds `--bins`, release.yml release-status verification fails closed on network errors

## [0.2.0-rc.4] - 2026-11-01

### Changed

- `fuzz/` crate edition 2021 → 2024 (aligned with the main crate): rustfmt style edition follows as 2024, import ordering now consistent repo-wide; CI and the local pre-commit gate add format checking for the standalone `fuzz/` workspace to prevent style drift again

## [0.2.0-rc.3] - 2026-10-20

### Added

- wasm32 build support and CI gate: `oxcache` restricted to non-wasm targets, wasm target adds `uuid/js` random sources, `tokio` made on-demand (`http`/`mcp`); core and `cli` feature builds enter the `wasm32` job in `ci.yml` (lib+bins + clippy)
- `CacheManager` no-cache fallback under wasm32 (same signatures: `get` always `None`, `insert` a no-op, `get_or_compute` evaluates directly, zeroed statistics); language detection falls back to `en`
- `eig_general`: eigenvalues of asymmetric real matrices (nalgebra real Schur + complex eigenvalue ordering), routed and synced with the `LinearAlgebra` trait; `eig` keeps symmetric semantics and its error hint points to the new function
- Special function expansion: `besselj`/`bessely`/`besseli`/`besselk` (J/Y family reuses `libm` fdlibm, I/K family A&S 9.8), `beta`, `zeta` (Euler–Maclaurin N=20/K=10 + functional equation), `airy_ai`/`airy_bi` (Maclaurin series + four-term asymptotics); domain routing, function catalog, and `ScalarMath` trait synced (new dependency `libm 0.2`)
- Numerical root finding and integration: `root(expr, var, a, b)` (Brent) and `integral(expr, var, a, b)` (Gauss–Kronrod 15 + adaptive Simpson); the math layer adds `secant`/`adaptive_simpson`/`gauss_kronrod_15`, and variable-name resolution moves to the shared `domains/common`
- wasm runtime API (`wasm-api` feature): `evaluate(expr)` returns `{"v":1,"result":...}`, `list_functions()` returns the function catalog JSON; error payload is `{"v":1,"error":{"code","kind","message"}}`; wasm32 targets only
- wasm real L1 cache: upgrade to oxcache `0.5.0-rc.9` (its wasm32 support is released independently in the oxcache repo), deleting CalNexus's wasm no-cache fallback and restoring the unified `ByteWeightCache` implementation
- E-graph simplification (`egraph` feature, based on `egg 0.9`): after greedy simplification, adds equality saturation (commutativity/associativity, identity elements, `x-x`, `x/x`, `sin²+cos²→1`) with iteration/node limits and greedy fallback on failure
- Asymmetric eigenvectors: `eig_general` extended to return values + vectors (real Schur + complex null-space back-substitution), JSON adds `vectors_re`/`vectors_im`
- numeric-frontier batch closure: `erfc` (libm fdlibm, large arguments no longer zero out from cancellation error), `lgamma` (ln|Γ(x)|, covering the large-argument range where `gamma` overflows), `polylog(s, z)` (s ∈ {2,3}; power series for |z|<1, `zeta` reuse at z=1, analytic value at z=-1, explicit Domain out of range), `derivative(f, var, x)` (central differences + Richardson extrapolation), `lstsq(A, b)` (QR main path back-substitution, rank-deficient fallback to SVD pseudo-inverse minimal-norm solution)
- The above functions are wired into: the scientific domain whitelist and expression routing (`polylog` adds an analytic integer-order `eval_polylog`), the `ScalarMath` trait (`erfc`/`lgamma`/`polylog`/`derivative`) and the `LinearAlgebra` trait (`lstsq`, `numerical` gated), and the `--list-functions` / REPL completion / MCP `list_functions` catalog
- i18n error keys (EN/ZH pairs, with `{ $dim }` / `{ $value }` interpolation): `msg.scientific.polylog_order`, `msg.scientific.polylog_domain`, `msg.numerical.lstsq_row_mismatch`, `msg.numerical.lstsq_empty_matrix`
- Anti-drift tests: the function catalog must contain the new functions; every scientific function in the catalog must be routable (a whitelist entry with a missing match arm fails); trait dispatch coverage for the new methods; CLI three-surface (`--list-functions` / `--canonical` / `--json`) output assertions

### Changed

- Canonicalizer: `Add`/`Mul` canonical strings switch to n-ary flattening with per-item sorting (`(a+b)+c` and `a+(b+c)` share a key, complex-term order also normalized); `Sub`/`Div`/`Pow`/`Mod` stay binary
- `symbolic::simplify` recognizes `sin²θ + cos²θ → 1` (same base, including reversed order)
- Dependency anchoring (release orchestration §1.4): `oxcache 0.5.0-rc.10` (absorbing its wasm32 byte-weight / serialization capability), `sdforge 0.5.0-rc.9`, `inklog 0.3.0-rc.10`, `limiteron 0.3.0-rc.8`, `confers 0.6.0-rc.11`; `cargo tree -d` shows no second oxcache / inklog, no `[patch.crates-io]`
- Coverage gate 90 → 95: measured with the same command as CI, line coverage 95.57% (28653 lines / 1269 uncovered), `--fail-under-lines 95` exits 0

### Fixed

- Drift between the function catalog (`--list-functions` / REPL Tab completion / MCP `list_functions`) and per-domain routing whitelists: removed phantom entries `stddev`/`trace`/`re`/`im`/`magnitude`/`phase`/`matrix_exp` (`matrix_exp` exposed only via the trait API), added `count`/`identity`/`log10`/`log2`/`arg` plus 21 statistics distribution/test entries, 11 time-domain entries, and 7 vector-domain entries; the catalog is now derived from each domain's whitelist constants (single source of truth) with new anti-drift tests reconciling per domain
- `--json` key-order contract regression: `confers 0.6.0-rc.11` enabled `serde_json/preserve_order` via feature merging, degrading output from lexicographic to insertion order and breaking the key-order contract recorded for 0.2.0. Keys are now recursively re-sorted at the four serialization exits (`CalcError::to_json`, CLI success output, wasm success and error payloads) via `core::sort_json_keys`, so output no longer depends on serde_json's map implementation
- `sort_json_keys` re-export gated on `cli` / `wasm32`, fixing the `unused_imports` failure of `cargo clippy --features server --all-targets -- -D warnings`

### Version numbering note

This batch was originally written as `0.3.0`; per the release orchestration ruling (rule 5: `0.2.0-rc.2 → 0.2.0-rc.3`) it went out as a patch + pre-release number; the `--json` contract version `v:1` is unchanged and does not consume a minor.

## [0.2.0-rc.2] - 2026-10-12

### In-house dependency upgrades and feature absorption (2026-10-12)

#### Changed

- In-house dependency upgrades: sdforge 0.5.0-rc.5→rc.8, inklog 0.3.0-rc.5→rc.9, limiteron 0.3.0-rc.5→rc.7, confers 0.6.0-rc.6→rc.10; Cargo.lock pins oxcache 0.5.0-rc.8, utoipa 6.0, utoipa-swagger-ui 10.0 (vendored, so the docs build no longer downloads Swagger UI assets from the network)
- **(Breaking, HTTP error body)** Aligned with the sdforge unified error contract: the HTTP error body changed from direct `ApiError` serialization (variant name + that variant's fields: 422 carrying `constraint`, 503 carrying `service`/`retry_after`) to `{"code","message","trace_id","field"}` — 400 `BAD_REQUEST`, 422 `UNPROCESSABLE_ENTITY`, 503 `UNAVAILABLE` (MCP keeps `ApiError` serialization with the `type` protocol name unchanged); 422 no longer carries the `constraint` limit detail, using `field` to name the offending field instead, and 503 no longer outputs `service`/`retry_after` (`retry_after` exists only in `ServiceError` details, never in the HTTP body or response headers)
- Cache readiness check switched to an async probe (`register_async_readiness_check_fn` + `spawn_blocking` isolating moka maintenance + 5s timeout marks unhealthy), so moka maintenance no longer occupies runtime worker threads

#### Added

- `.github/workflows/feature-matrix.yml`: weekly `cargo hack check --each-feature --all-targets` matrix (single-feature compile-crack gate)

#### Fixed

- fx breaker Open-state late-success false closure (absorbed with the limiteron 0.3.0-rc.6 fix; new regression test `test_breaker_open_state_survives_late_success` anchors it)
- inklog ERROR/FATAL backstop re-delivery when channels are saturated, sink degradation/recovery ops-event self-exciting loop fix, file sink fsync and 0600 permission hardening (absorbed with the inklog upgrade, no wiring changes)

## [0.2.0-rc.1] - 2026-09-21

### v015-comprehensive-optimization (2026-09-14, spec-driven: specmark)

A one-shot full optimization after a six-dimension architecture audit (security / usability / performance / distribution / maintainability / architectural advancement), RICE-quantified priority ordering, 14-stage implementation.

#### Added

- `--timeout` / `--cache-size` / `--bind` CLI flags + `CALNEXUS_TIMEOUT` / `CALNEXUS_CACHE_SIZE` / `CALNEXUS_BIND_ADDR` environment variable fallbacks (precedence flag > env > default)
- `ErrorKind::DependencyUnavailable` (exit_code=3): upstream dependency failures strictly distinguished from client errors; HTTP maps to 503 + Retry-After
- `POST /api/v1/list_functions` + MCP `list_functions` tool + CLI `--list-functions`: runtime function catalog (unified catalog `function_catalog`, sdforge resources=false downgraded to a tool)
- `evaluate_with_router` public API: router injection, unlocking downstream custom computation domains
- `/ready` (dependency readiness) and `/live` (process liveness) probes separated, `/health` returns the real `entry_count`
- request-id/traceparent middleware (`X-Request-ID` passthrough/generation), `calnexus_http_requests_total` metrics
- `observability` feature wired to tracing-subscriber (EnvFilter consuming `RUST_LOG`, default warn)
- Multi-platform prebuilt binary releases (linux x86_64/aarch64 musl, macOS both architectures, windows x86_64 + SHA256SUMS)
- Dockerfile (multi-stage distroless non-root) and `docs/DEPLOY.md`, `docs/MCP.md`
- `docs/schema/result-v1.json`: the `--json` contract JSON Schema (Draft 2020-12)
- `.github/workflows/audit.yml` (weekly cargo audit + cargo deny) + `deny.toml` + dependabot
- CI matrix completion: server / cli,numerical / all-features test and clippy legs, MSRV 1.85 job
- Parse-error caret position indicator (text mode); contextual `undefined_symbol` hint (CLI `--var` / REPL `:let`)
- Recursion-depth protection: RAII guard for bracket literals, iterative pre-check before mathexpr, canonicalizer depth guard; `list_depth_fuzz` target
- `--json` contract version field `"v":1`; insta snapshots covering all CLI-reachable variants (SNAP-J01~J12)

#### Changed

- **Cache engine refactoring**: `CacheManager` connects directly to `moka::sync` (replacing the oxcache wrapper) — production-grade `try_get_with` single-flight, `Arc<EvalResult>` zero-copy hits, single-pass BLAKE3 keys, byte-weight budget (64MB default) + 256KB large-result admission threshold; the hit path eliminates JSON serialization and the temporary tokio runtime (`cache_hit/2+3` benchmark ≈ 1.29µs)
- The evaluator path uniformly uses `get_or_compute` (single-flight wiring); `/metrics` emits canonical Prometheus text (`# HELP`/`# TYPE`)
- HTTP routes mounted explicitly (single source of truth): fixes the longstanding production bug where `#[forge]` inventory registration was silently dropped by linker GC, leaving every `--serve-http` route 404
- fx failure three-way classification i18n (network_unreachable/invalid_response/cache_unreadable) + `CalcError.source_detail` error chain; `to_json` and batch JSON unified on serde_json (the hand-written `escape_json_string` retired)
- mathexpr error message sanitization (no longer leaks winnow's internal Debug structures); `.env.example` and the implementation aligned in both directions
- fx disk cache atomic writes (temp+rename+0600), fetch single-flight, ureq `https_only` + 1MB response-body cap, `rate_date` exposed
- Graceful shutdown drain 30s timeout; api-layer RwLock poisoning fully sanitized
- Tool descriptions self-contained (req wrapping/limits/error semantics); REPL Tab completion switched to the unified function catalog (fixes missing feature-gated functions)
- edition 2021 → 2024; release.yml actions all pinned to commit SHAs; llvm-cov 90% line coverage becomes the only coverage gate (tarpaulin dead configuration deleted)
- openspec/ converged into specmark/specs (27 capability domains); specmark is the only SDD process
- Documentation name-reality alignment: measured test count (2820), coverage badge (90.4% llvm-cov measured), Moka→moka direct connection, `Domain`→`CalculationDomain`, sdforge version numbers, wasm chapter marked experimental

#### Fixed

- **(Longstanding production bug)** Every `--serve-http` API route returned 404: after sdforge's crates.io migration, `#[forge]` HTTP inventory registration was dropped by linker GC; the 12 server integration tests had been red since the migration with no CI gate
- **(DoS gap)** Nested list/matrix literals bypassed `MAX_AST_DEPTH=256` (~1300 levels could overflow the `spawn_blocking` thread stack); mathexpr recursion ran before the depth check
- single-flight was never actually enabled on the production path (README name-reality mismatch); followers exposed real errors as "cache backend error"
- All three fx failure classes mapped to "network unreachable"; the `--timeout` hint pointed at a nonexistent flag (resolved together with the configuration surface)
- Parse errors leaked winnow Debug structures (snapshot-verified); `--json` hand-written escaping could produce invalid JSON for control characters

#### Removed

- The oxcache dependency (along with the tonic/prost build-chain burden beyond protoc — note: tonic-build still enters via sdforge's own build-dependency; protoc kept)
- `escape_json_string`, `CacheKeyGen::make_key`/`to_key_string`, the tarpaulin dead configuration, and the openspec/ directory

### Notes

- Breaking changes (0.x): `ErrorKind` gains a `DependencyUnavailable` variant (exhaustive matches need adapting); `--json` success-output `result` key order was decided by serde_json's lexicographic ordering, Scalars serialize as `5.0`, Complex outputs the `{re,im}` tagged object; `lib.rs`'s server wildcard export made explicit. All recorded in `docs/schema/result-v1.json` and the CHANGELOG.
- Performance baselines: `target/criterion/main` (cache_hit ≈ 1.29µs, parser_large_expression/4096-char ≈ 73µs). These are the historical baselines at the v015 refactor point (2026-09-14), not the current basis — the current baselines (cache_hit/`2+3` ≈ 0.902µs, 2026-09-17 commit `597ffb2`) are in [PERFORMANCE.md](PERFORMANCE.md).

## [0.1.4] - 2026-07-26

### Fixed

- **CI fmt red-line fix**: several `CalcError::domain(...).with_i18n(...)` chains and function signatures in `src/domains/fx.rs` / `time.rs` / `vector.rs` did not match rustfmt's default formatting, causing `cargo fmt --check` to fail 5 consecutive times on main-branch CI. A local `cargo fmt --all` fixed all 8 formatting deviations in one pass
- **Release pipeline closed loop (3-dimension subagent review — security/architecture/performance)**: `.github/workflows/release.yml` design flaw — it only created a GitHub Release and never triggered `cargo publish`. This fix closed all 5 HIGH + 9 MEDIUM defects:
  - **Security H-1**: GitHub Actions expression injection → `${{ }}` replaced with `env:` injection + version-number regex whitelist `^[0-9]+\.[0-9]+\.[0-9]+(-[A-Za-z0-9.]+)?$`
  - **Security M-2**: no pre-release SAST → added `cargo audit --deny warnings` (rule 24)
  - **Architecture H-1**: no pre-release verification → added `cargo publish --dry-run` package-level build verification (CI already runs tests)
  - **Architecture H-2**: missing `workflow_dispatch` trigger (commented but undefined in code) → added `workflow_dispatch` inputs.tag so publish failures can be retried manually
  - **Architecture M-1**: version verification in the wrong place → `Verify Cargo.toml version matches tag` moved before packaging, fail-fast
  - **Architecture M-2**: no `concurrency` control → added the `release-${{ github.ref }}` group with `cancel-in-progress: false`
  - **Architecture M-3**: no build verification before publish → added a `cargo publish --dry-run` step
  - **Performance H-1**: `apt-get update` on every run (10-20s) → switched to `arduino/setup-protoc@v3` (1-2s)
  - **Performance H-2**: no progress feedback from `cargo publish` → added `-v` to avoid a black-screen log
  - **Performance M-3**: `git archive` ran sequentially → changed to `&` parallel + `wait`
  - **Security LOW-1 / Architecture L-2**: `permissions` narrowed from workflow level to job level
  - Reordered: `cargo publish` runs before `softprops/action-gh-release`, so if publish fails the Release has not been created yet and can be retried at no cost

### Testing

- `cargo fmt --all -- --check`: pass (8 diffs before the fix, 0 after)
- `cargo test --features cli`: all pass
- `cargo test --features cli,time,unit,fx`: all pass (including the 10 time_unit_fx integration tests)
- `cargo clippy --features cli --all-targets -- -D warnings`: zero warnings
- `cargo publish --dry-run --allow-dirty`: pass (95 files, 2.0MiB package + full build verification + dependency fetch)
- `python3 yaml.safe_load`: release.yml YAML syntax validation passes

### Notes

- **v0.1.1 / v0.1.2 / v0.1.3 historical leftovers**: three tags triggered release.yml but only created GitHub Releases, never publishing to crates.io (crates.io stayed at v0.1.0). crates.io does not allow re-publishing the same version, so v0.1.1/0.1.2/0.1.3 are permanently missing. From v0.1.4 on, the release pipeline is closed-loop and every new tag publishes automatically
- **Unfixed item (LOW)**: third-party Actions not pinned to SHA (security M-1) — needs the Dependabot auto-update configuration; deferred as a follow-up

## [0.1.3] - 2026-07-26

### Added

- **VectorDomain 8 new vector operations**: `cosine_similarity(a,b)` / `project(a,b)` / `reflect(v,n)` / `euclidean(a,b)` / `manhattan(a,b)` / `outer(a,b)` / `lerp(a,b,t)` / Hadamard product `[a,b]*[c,d]` (List×List element-wise), with 57 new tests covering happy path / error paths / routing
- **TimeDomain**: 14 date/time functions with IANA timezone support (jiff 0.2, `time` feature) — `date` / `datetime` / `timestamp` / `from_timestamp` / `date_diff` / `date_add` / `parse_date` / `format_date` / `reformat_date` / `weekday` / `day_of_year` / `is_leap_year` / `now` / `today`
- **UnitDomain**: 8-dimension physical unit conversion with temperature affine (`unit` feature) — `convert(value, "from", "to")` covering length / mass / temperature / volume / area / speed / data / time
- **FxDomain**: Currency exchange via frankfurter.dev API with 3-level cache (`fx` feature) — `fx(value, "FROM", "TO")` / `fx_rate("FROM", "TO")`, with `CALNEXUS_FX_TTL_HOURS` and `CALNEXUS_FX_ALLOW_STALE` environment variables
- **AstNode::Str(String)**: String literal parsing in core parser (double-quoted, `\"` escape, unclosed quotes raise ParseError)
- **EvalResult::DateTime(String)**: DateTime result variant with output formatting (canonical / latex / steps / server JSON)
- **Nondeterministic function cache bypass in evaluator**: `now` / `today` / `fx` / `fx_rate` and other nondeterministic functions bypass L1 cache reads and writes, preventing time/fx results from polluting the cache

### Changed

- **Cargo.toml description**: updated from "11 computation domains" to "11 core and 4 optional (numerical/time/unit/fx) computation domains" (note: numerical is a feature-gated optional domain, but Cargo.toml briefly said 3 optional; corrected)
- **CI feature matrix**: test job adds the `--features cli,time,unit,fx` full test run and single-feature compile checks for `time` / `unit` / `fx`
- **README.md / README_EN.md**: added an optional-domains section (function table, feature switches, CALNEXUS_FX_* environment variables, FX data sources and disclaimer), updated domain counts
- **docs/ARCHITECTURE.md**: updated the L2 domain layer list to 14 domains (11 core + 3 optional), added the nondeterministic function cache bypass description

### Fixed

- `fx.rs` 6 error paths wired to i18n keys (invalid bignumber / factorial not supported / string operand / matrix operand / complex operand / unsupported binary op)
- `time.rs` 7 error paths wired to i18n keys (invalid bignumber / factorial not supported / string operand / matrix operand / complex operand / unary op unsupported / invalid date format)
- `FxDomain` constructor injects `Box<dyn RateProvider>`, removing the spec↔code contradiction (R-fx-004); removed `OnceLock`, added `new()` and `default()` constructors, `factory.rs` switched to `FxDomain::default()`

### Testing

- `cargo test --features cli,time,unit,fx --lib`: 1915 passed / 0 failed
- `cargo test --features cli,time,unit,fx --test time_unit_fx_integration`: 10 passed / 0 failed
- `cargo clippy --features cli,time,unit,fx --all-targets -- -D warnings`: zero warnings
- Coverage ≥95% (tarpaulin, fail-under=95 gate)
- specmark `time-unit-fx-domains` change archived to `specmark/archive/2026-07-25-time-unit-fx-domains/`, main specs synced

## [0.1.2] - 2026-07-21

### Fixed

- **SKILL.md frontmatter corruption fix**: the corrupted single-line merge of the `description` field is fixed, restoring the correct YAML frontmatter structure
- **Documentation-code consistency fixes**: 20 documentation/code inconsistencies fixed per the pre-release review (missing CLI flags / outdated example outputs / stale version numbers / wrong Domain tables / duplicate CHANGELOG entries / misleading ARCHITECTURE statements)

**4 subagent exhaustive analyses + TDD fixes (Core/Domains/Output/Server+i18n)**:

**Core module** (7 bugs):
- BUG-C-M-001/002/006: 3 spots in `cache.rs` silently swallowing errors via `.ok().flatten()` / `let _ =` / `unwrap_or(0)` changed to explicit `match` + `eprintln!` degrading to a cache miss (rule 12)
- BUG-C-M-003: `canonicalizer.rs` NaN comparison changed from `partial_cmp + unwrap_or(Equal)` to `f64::total_cmp`, providing the IEEE 754 totalOrder
- BUG-C-M-004: `canonicalizer.rs` explicitly handles `0^0 = 1.0`, consistent with all domains
- BUG-C-M-005: `parser.rs` `validate_no_consecutive_plus` extended to `validate_no_consecutive_operators`, checking the four operator pairs `++`/`**`/`//`/`^^`
- BUG-C-L-008: `domain.rs` `format!("{:?}", functions)` output capped at 5 items + an `... and N more` hint, preventing over-long Debug output

**Domains module** (9 MEDIUM + several LOW):
- BUG-D-M-001/002/003: `vector.rs` / `complex.rs` / `polynomial.rs` scalar Pow adds `0^0=1.0` + an `is_finite` check
- BUG-D-M-004/005: `polynomial.rs` `coeffs_from_pow` NaN check + `poly_eval_horner` signature changed to `Result<f64, CalcError>`
- BUG-D-M-006: `symbolic.rs` `taylor()` changed from `unwrap_or(0.0)` to `?` propagation
- BUG-D-M-007/008: `symbolic.rs` `simplify_pow` / `eval_symbolic` add `is_finite` checks, extracting the `check_finite()` helper
- BUG-D-M-009: `complex.rs` `arg(0+0i)` changed from silently returning 0.0 to `Err(DomainError)` + i18n key `msg.complex.arg_zero_undefined`
- BUG-D-L-001/002: `vector.rs` / `precision.rs` pre-bind pi/e at the top of `evaluate()` (following the polynomial pattern)
- LOW: 24 hardcoded error messages i18n-ized in `vector.rs` / `complex.rs` / `precision.rs`

**Output module** (12 MEDIUM):
- BUG-O-M-001/002/003: `steps.rs` `format_value` float-noise detection + large-number scientific notation + preserving the sign of `-0.0`
- BUG-O-M-004: `steps.rs` `walk` Matrix branch changed from `Ok(0.0)` to `Err(DomainError)`
- BUG-O-M-005: `steps.rs` BigNumber branch checks the f64 safe-integer range (2^53), erroring beyond it
- BUG-O-M-006: `steps.rs` gcd/lcm add `check_integer_arg` validating integrality and the i64 range
- BUG-O-M-007/008: `latex.rs` `format_latex_scalar` float-precision noise detection + large-integer scientific notation
- BUG-O-M-009/010: `latex.rs` `format_latex_complex` zero-imaginary/zero-real simplified output
- BUG-O-M-011: `latex.rs` `join_latex_polynomial_terms` separates negative terms with ` - `
- BUG-O-M-012: `latex.rs` `symbolic_str_to_latex` state machine parses `^N` into `^{N}`

**Server/i18n/entry modules** (4 MEDIUM + 1 LOW):
- BUG-S-M-001: `server/evaluate.rs` adds `REQUEST_TIMEOUT_SECS=30` + a testable `evaluate_with_timeout` entry, wrapping `spawn_blocking` in `tokio::time::timeout`
- BUG-S-M-002: `server/types.rs` `validate()` adds vars key-length (≤64) and null-byte checks
- BUG-S-M-003: `server/http.rs` adds a `shutdown_signal` listener for SIGINT/SIGTERM with `with_graceful_shutdown`
- BUG-I-M-001: `i18n.rs` `parse_lang_simple` switched to BCP-47 standard subtag splitting, supporting variants like `zh-Hans`/`zh-Hant`/`zh-Hans-CN`
- BUG-E-L-001: `main.rs` adds `setup_panic_hook` installing a panic hook that prints a user-friendly prefix + keeps the default backtrace

**Regression issues found by review subagents** (3, fixed):
- H-1 (architecture HIGH): 3 `expect()` calls in `cache.rs` changed to log+degrade (`eprintln!` + return `None`/`0`), preserving API compatibility
- M-1 (architecture MEDIUM): `complex.rs` complex Pow path adds the `0^0=1.0` + `is_finite` check, consistent with the scalar branch
- M2 (performance MEDIUM): `vector.rs` / `precision.rs` conditionally clone EvalContext, skipping the clone when pi/e are absent

### Changed

- **SKILL.md moved to `skill/SKILL.md`**: the root-level SKILL.md moved to `skill/SKILL.md` as the project usage guide; `.claude/skills/calnexus-dev/SKILL.md` slimmed to a lightweight skill reference
- **i18n key expansion**: en.json / zh.json expanded from 195 to 248 keys (53 new keys, 1 dead key `msg.output.eval_error` removed, 1 key `msg.core.parse_illegal_consecutive_ops` modified to add the `{op}` placeholder); en/zh fully symmetric
- **Cargo.toml**: the `http` feature adds the `tokio/signal` dependency for graceful-shutdown signal listening

### Security

- 3 independent review subagents (security/architecture/performance) completed the pre-commit review per rule 26
- **tiangang SAST scan**: 0 CRITICAL/HIGH (Gitleaks 0 leaks / Trivy 0 vulnerabilities / cargo-audit 0 advisories / Trufflehog 0 verified secrets)
- HTTP server request-level timeout (30s) + vars key-name validation + graceful shutdown, triple protection
- Cache-layer errors degrade to cache misses, preventing panics from affecting HTTP request handling

### Testing

- Full feature-matrix tests pass: `cli` 1856 tests passed / 0 failed / 3 ignored
- `cargo clippy --features cli --all-targets -- -D warnings` 0 warnings
- 3-dimension subagent review (security / architecture / performance) passed, no CRITICAL / HIGH blockers
- 38 new regression tests covering all fixed bugs

## [0.1.1] - 2026-07-21

### Added

- **ICU internationalization infrastructure**: new `src/i18n.rs` module based on ICU4X 2.2 for BCP-47 locale parsing and message localization
  - `I18n` struct + `tf(key, args)` method supporting `{name}` placeholder substitution
  - `Lang` enum (`En` / `Zh`) + `from_locale_str()` BCP-47 parsing
  - Compile-time `include_str!` embedding + `OnceLock<HashMap>` zero-copy runtime parsing
- **Localization resource files**: new `locales/en.json` and `locales/zh.json` (195 keys each, fully synced)
  - Covering error messages (`error.*` / `detail.*` / `msg.*`), CLI text (`cli.*`), REPL text (`repl.*`), batch text (`batch.*`)
- **CalcError i18n fields**: beyond `ErrorKind` + `message`, adds `i18n_key: Option<&'static str>` + `i18n_args: Vec<(String, String)>`, supporting `with_i18n()` chaining
- **SKILL.md**: project skill document (192 lines), with both source install and `cargo install` paths

### Changed

- **CLI module refactor**: clap is used indirectly via `sdforge::clap` re-exports, removing the direct `clap` dependency and unifying the CLI wrapper layer
- **Dependency version alignment**: all dependencies unified to the `x.x` (Major.Minor) format; patch-segment syntax banned
- **Code complexity optimization**: helper methods extracted for 10 hot-path functions, lowering cyclomatic complexity (cyc=25 → ≤15)
- **`friendly()` / `to_explain()` go through i18n**: prefer looking up the localized message via `i18n_key`, falling back to the English `message`
- **`ReplSession` holds an `I18n`**: REPL sessions hold their own i18n instance, supporting runtime language switching
- **`batch::run()` takes an `I18n` parameter**: batch mode supports localized output

### Fixed

- **`--steps` mode computing `sin(pi/2)` incorrectly**: `output::steps::walk()`'s `Variable(name)` branch used `ctx.get_var(name).unwrap_or(0.0)`, wrongly evaluating the `pi`/`e` math constants as 0.0 (user report: `calnexus --steps 'sin(pi/2)'` printed `sin(0)=0`). Fix: the Variable branch recognizes the `pi`/`e` constants, user-bound variables take precedence (consistent with scientific/statistics/matrix domain pre-binding of pi/e)
- **`release.yml` tar packaging failure**: `tar czf .` reported "file changed as we read it"; switched to `git archive` based on the git index, avoiding filesystem-churn interference

### Removed

- **Dead key `msg.core.eval_domain_error`**: 0 hits in src/, deleted to avoid misleading
- **Duplicate keys `msg.arithmetic.factorial_negative` / `msg.scientific.factorial_negative` / `msg.output.factorial_negative`**: unified into `msg.core.factorial_negative`, removing the DRY violation

### Security

- **`validate_inputs` precision upper bound**: the `evaluate` public entry intercepts oversized precision, preventing `format_decimal` loop DoS
- **`parse_matrix_literal` Debug output limit**: `format!("{:?}", row_node)` capped at 100 characters, preventing large-AST recursive Debug performance issues
- **DP-1 / DP-4 contracts preserved**: `Display` impl and `to_json()` always output the English `message`; the machine-readable contract is unaffected by i18n

### Architecture

- **DP-1**: `error_kind_prefix` stays English (machine-parsable diagnostics contract)
- **DP-2**: `CalcError` gains `i18n_key` + `i18n_args` fields (minimally invasive change)
- **DP-3**: math formula syntax preserved (universal across languages)
- **DP-4**: JSON output key names stay English (machine contract)
- **DP-5**: Server error responses go through i18n

### Testing

- Full feature-matrix tests pass: `default` 1437 / `cli` 1791 / `cli,icu` 1791 / `cli,server` 1847 / `cli,server,icu` 1847
- `cargo clippy --features cli --all-targets` zero warnings
- 3-dimension subagent review (security / architecture / performance) passed, no CRITICAL / HIGH
- 5 new regression tests covering `Variable("pi")`/`Variable("e")` evaluation in steps mode

## [0.1.0] - 2026-07-15

### Added

- Initial release: 11 computation domains (arithmetic / statistics / numerical / scientific / matrix / symbolic / polynomial / number_theory / combinatorics / precision / domain router)
- Command-line interface (`--expr` / `--var` / `--precision` / `--steps` / `--json` / `--latex` / `--canonical` / `--explain` / `--lang` / `--repl` / `--batch`)
- REPL interactive mode (`:let` / `:vars` / `:clear` / `:help` / `:quit`)
- Batch mode (stdin reading + `--count` / `--max-line` limits)
- HTTP API (`/evaluate` endpoint, axum framework)
- MCP protocol interface (Model Context Protocol)
- DoS protections: expression length limit (4096), AST depth limit (256), precision upper bound (MAX_PRECISION), pow output digit limit, factorial input upper bound
- Cache layer (`CacheManager` + `CanonicalForm` canonical keys)
- Timeout mechanism (`EvalContext.timeout` + `check_elapsed` at key nodes)

[unreleased]: https://github.com/kirky-x/calnexus/compare/v0.2.0-rc.2...HEAD
[0.2.0-rc.2]: https://github.com/kirky-x/calnexus/compare/v0.2.0-rc.1...v0.2.0-rc.2
[0.2.0-rc.1]: https://github.com/kirky-x/calnexus/compare/v0.1.4...v0.2.0-rc.1
[0.1.4]: https://github.com/kirky-x/calnexus/compare/v0.1.3...v0.1.4
[0.1.3]: https://github.com/kirky-x/calnexus/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/kirky-x/calnexus/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/kirky-x/calnexus/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/kirky-x/calnexus/releases/tag/v0.1.0

---

## 🗺️ Development Roadmap

> This section was migrated from the README's "🗺️ Roadmap" section.

<details>
<summary>📋 Table</summary>

<table style="width:100%; border-collapse: collapse">
<tr><th style="text-align:center">Status</th><th style="text-align:left">Direction</th><th style="text-align:left">Items</th></tr>
<tr><td align="center">✅</td><td>Core engine (v0.1.0)</td><td>11 computation domains, symbolic calculus, REPL, batch processing, arbitrary precision, JSON output, implicit multiplication</td></tr>
<tr><td align="center">✅</td><td>Optional domains &amp; server (v0.1.3-v0.1.5)</td><td>Time / unit / fx domains, HTTP + MCP dual protocol, probe separation, rate limiting, observability, multi-platform releases &amp; container images</td></tr>
<tr><td align="center">✅</td><td>Cache engine modernization</td><td>oxcache byte-weight cache (moka core), single-flight, byte-weight budget, BLAKE3 keys, large-result admission threshold</td></tr>
<tr><td align="center">✅</td><td>WebAssembly (wasm32) support</td><td>Core and <code>cli</code> builds are CI-gated: <code>cargo build --lib --bins --target wasm32-unknown-unknown --no-default-features</code> and <code>--features cli</code>. wasm uses oxcache's real L1 cache (ByteWeightCache); <code>wasm-api</code> exports <code>evaluate</code>/<code>list_functions</code> for browser/Node (build-gated only, runtime behavior not promised); the <code>fx</code>/<code>http</code>/<code>mcp</code>/<code>icu</code>/<code>time</code>/<code>unit</code> feature combos are out of scope</td></tr>
<tr><td align="center">✅</td><td>Numerical capability growth (v0.2.0-rc.3)</td><td>Special functions <code>erfc</code>/<code>lgamma</code>/<code>polylog</code>, numerics <code>derivative</code> (central differences + Richardson) and <code>lstsq</code> (QR with SVD pseudo-inverse fallback), alongside <code>root</code>/<code>integral</code> and the Bessel/Airy/<code>beta</code>/<code>zeta</code> family — wired into domain routing, the function catalog and the trait API</td></tr>
</table>

</details>
