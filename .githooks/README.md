# Git Hooks

This directory contains project-managed Git hooks for CalNexus.

## Installing the hooks

By default Git looks for hooks in `.git/hooks`. To make Git use the hooks
tracked in this repository instead, run once per clone:

```bash
git config core.hooksPath .githooks
```

That's it — the `pre-commit` hook will now run automatically before every
`git commit`.

Verify it is active:

```bash
git config --get core.hooksPath
# expected output: .githooks
```

## The `pre-commit` hook

An industrial-grade code quality gate that runs **before** each commit. It
performs 9 checks:

| # | Check | Blocking? |
|---|-------|-----------|
| 1 | `cargo fmt --all -- --check` (no auto-fix) | yes |
| 2 | `cargo clippy --features cli --all-targets -- -D warnings` | yes |
| 3 | `cargo test --features cli` | yes |
| 4 | `cargo build --release --features cli` with **zero warnings** | yes |
| 5 | Every `.rs` file starts with `// Copyright (c) 2026 Kirky.X` | yes |
| 6 | No `println!`/`dbg!` in library code (`eprintln!` OK in CLI/REPL) | yes |
| 7 | No `TODO`/`FIXME`/`HACK` comments in `src/` | yes |
| 8 | `Cargo.lock` exists and is tracked | yes |
| 9 | No source file exceeds 3000 lines | no (warn only) |

The hook prints colored `[PASS]`/`[FAIL]`/`[WARN]` output for each check and
rejects the commit (exit code 1) if any blocking check fails.

## Bypassing the hook

Use sparingly — only when a check is genuinely not applicable:

```bash
git commit --no-verify
```

## Uninstalling

Restore the default hooks location:

```bash
git config --unset core.hooksPath
```
