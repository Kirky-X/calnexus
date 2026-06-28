---
name: calnexus-dev
description: Reference guide for working on the CalNexus Rust project. Use when writing or modifying CalNexus Rust code, adding computation domains, modifying the CLI/REPL/batch entry modes, running tests, or checking coverage and linting.
---

# CalNexus Development Guide

## Project Overview

CalNexus is a Rust computation engine exposing **11 computation domains** through **3 modes** (CLI, REPL, batch). The pipeline flows: **parse → canonicalize → cache → route → evaluate**.

- **parse** turns raw input into an AST.
- **canonicalize** normalizes expressions into a canonical form.
- **cache** avoids recomputation (capped at 10000 entries).
- **route** selects the first domain whose `supports()` matches (priority-based).
- **evaluate** produces results plus optional LaTeX / steps / canonical output.

## Development Commands

| Task     | Command                                               |
| :------- | :---------------------------------------------------- |
| Build    | `cargo build --features cli`                          |
| Test     | `cargo test --features cli`                           |
| Coverage | `cargo llvm-cov --fail-under-lines 90 --features cli` |
| Lint     | `cargo clippy --all-features -- -D warnings`          |
| Format   | `cargo fmt`                                           |
| Release  | `cargo build --release --features cli`                |

Always pass `--features cli` for any command that exercises the CLI surface.

## Architecture Rules

- **Single crate.** Modules live under:
  - `src/core/` — `parser`, `canonicalizer`, `cache`, `domain`, `types`
  - `src/domains/` — the 11 computation domains
  - `src/output/` — `latex`, `steps`, `canonical` renderers
  - `src/cli.rs`, `src/repl.rs`, `src/batch.rs` — the three entry modes
- **Feature gate.** The `cli` feature controls `clap`, `rustyline`, `rayon`, and file I/O. Gate CLI-only code behind `#[cfg(feature = "cli")]`.
- **Domain trait.** Routing is priority-based: iterate domains in priority order; the first `supports()` that returns `true` wins. New domains must implement the trait and register with a sensible priority.
- **Limits (enforce everywhere):**
  - Expression depth: **256**
  - Total length: **4096 chars**
  - Cache entries: **10000**

## Coding Conventions

- **Comments in Chinese** — match the existing code style throughout the codebase.
- **Conventional Commits** — `feat:`, `fix:`, `docs:`, `test:`, `chore:` (add a scope when helpful, e.g. `feat(domain):`).
- **Every new function needs tests.** No untested public functions.
- **Use proptest** for property-based tests on parsers, canonicalizers, and domains.

## Testing Conventions

- **Unit tests** live in each module's `#[cfg(test)] mod tests` block, next to the code under test.
- **Integration tests** live in the top-level `tests/` directory.
- **CLI tests** use `assert_cmd` to exercise the binary end-to-end.
- **Snapshot tests** use `insta` for LaTeX / steps / canonical output.
- **Coverage target: 100%** — accept 90%+ only for genuinely hard-to-test paths (error formatting, panics on impossible states).

## Adding a New Domain

1. Create `src/domains/<name>.rs` implementing the domain trait.
2. Implement `supports()` and `evaluate()`; choose a priority that orders it correctly against existing domains.
3. Register the domain in the router.
4. Add unit tests (including proptest for invariants) and an integration test under `tests/`.
5. Update output renderers if the domain exposes new canonical forms.
6. Run `cargo fmt && cargo clippy --all-features -- -D warnings && cargo test --features cli` before committing.
