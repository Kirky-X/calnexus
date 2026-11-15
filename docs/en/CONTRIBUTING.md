# 🤝 CalNexus Contributing Guide

> First of all, thank you for wanting to contribute to CalNexus! 🎉
> This document describes how to set up the development environment and the standards to follow when submitting code.

## 📋 Table of Contents

- [👋 Welcome](#-welcome)
- [🧰 Environment Setup](#-environment-setup)
  - [Prerequisites](#prerequisites)
  - [Fork and Clone](#fork-and-clone)
  - [Complete the Setup](#complete-the-setup)
  - [Feature Flags](#feature-flags)
  - [Build and Test](#build-and-test)
- [🔄 Development Workflow](#-development-workflow)
  - [Branches and Commits](#branches-and-commits)
  - [Commit Message Convention (Conventional Commits)](#commit-message-convention-conventional-commits)
  - [Git Hooks](#git-hooks)
- [🎨 Code Style](#-code-style)
- [🧪 Testing Requirements](#-testing-requirements)
- [📥 Submitting a PR](#-submitting-a-pr)
- [📚 Related Documents](#-related-documents)

---

## 👋 Welcome

CalNexus is a command-line math expression evaluator written in Rust: 11 core computation domains + 3 optional computation domains (time / unit / fx), symbolic calculus, REPL, and batch processing. Whether it's fixing bugs, adding functions, improving docs, or strengthening tests — all contributions are welcome.

### Ways to Contribute

| Way | Entry point |
|------|------|
| 🐛 Report a bug | [Open an Issue](https://github.com/kirky-x/calnexus/issues/new) (include reproduction steps, your `calnexus` version, and OS information; for symbolic-calculus/precision bugs attach a minimal reproducing expression) |
| 💡 Suggest a feature | [Start a discussion](https://github.com/kirky-x/calnexus/discussions) |
| 🔧 Submit code | Follow the workflow below — fork and open a PR |

---

## 🧰 Environment Setup

### Prerequisites

| Tool | Requirement |
|------|------|
| Rust toolchain | ≥ 1.97.1 (installed via [rustup](https://rustup.rs)) |
| `cargo fmt` / `cargo clippy` | Bundled with the toolchain |
| protoc | Required by the HTTP integration tests (`sudo apt-get install protobuf-compiler`) |

### Fork and Clone

```bash
# 1. Fork the repository on GitHub, then:
git clone https://github.com/<your-username>/calnexus.git
cd calnexus

# 2. Add upstream so you can sync the main branch
git remote add upstream https://github.com/kirky-x/calnexus.git
```

### Complete the Setup

```bash
# Install the Rust toolchain (MSRV 1.97.1)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Enable the project-managed Git hooks (pre-commit 9 checks)
git config core.hooksPath .githooks

# Build and verify the test suite
cargo build --features cli
cargo test --features cli
```

(Optional) To adjust runtime defaults, copy the environment template:

```bash
cp .env.example .env
```

### Feature Flags

Enable features as needed during development (for the full matrix see [🏗️ Architecture](ARCHITECTURE.md)'s Feature Gate Strategy):

```bash
cargo build --features cli                  # CLI / REPL / batch
cargo build --features cli,time,unit,fx     # with optional domains
cargo build --features server               # HTTP + MCP service
cargo build --all-features                  # everything
```

### Build and Test

```bash
# Build the project
cargo build --features cli

# Run all tests (the CI matrix's six legs)
cargo test --features cli
cargo test --features cli,time,unit,fx

# Run benchmarks
cargo bench --features cli
```

---

## 🔄 Development Workflow

### Branches and Commits

```bash
# Sync upstream's main branch
git checkout main
git pull upstream main

# Create a feature branch (naming prefix matches the commit type)
git checkout -b feat/support-complex-numbers
# or a bugfix branch
git checkout -b fix/modulo-zero
```

### Commit Message Convention (Conventional Commits)

We follow the [Conventional Commits](https://conventionalcommits.org) convention:

```text
<type>(<optional scope>): <description>

[optional body]

[optional footer(s)]
```

Accepted `type` values:

| Type | Purpose |
|------|------|
| `feat` | New feature |
| `fix` | Bug fix |
| `docs` | Documentation-only changes |
| `refactor` | Code changes that are neither fixes nor features |
| `test` | Adding or correcting tests |
| `chore` | Maintenance tasks: builds, dependencies, toolchain, etc. |

Examples:

```text
feat(parser): support implicit multiplication between number and parenthesis
fix(eval): handle division by zero in modulo operator
docs(readme): clarify supported operators list
test(cache): add regression tests for L1 cache eviction
chore(deps): bump clap to 4.5
```

### Git Hooks

After `git config core.hooksPath .githooks` is enabled, the pre-commit hook runs **9 checks** before every commit:

1. `cargo fmt` (formatting must already be correct; no auto-fix)
2. `cargo clippy` (all targets, deny warnings)
3. `cargo test` (all tests must pass)
4. `cargo build --release` (zero warnings)
5. Copyright header on every `.rs` file
6. No `println!` / `dbg!` debug output in library code
7. No TODO/FIXME/HACK comments in `src/`
8. `Cargo.lock` exists and is tracked
9. File size check (warn only, non-blocking)

Temporary bypass (use sparingly): `git commit --no-verify`.

---

## 🎨 Code Style

CalNexus follows standard Rust formatting and lint conventions.

- **Format before committing**:

```bash
cargo fmt --all
```

- **Clippy zero warnings** (all features, warnings as errors):

```bash
cargo clippy --all-features --all-targets -- -D warnings
```

- Write idiomatic Rust: propagate errors with `?`, rely on the borrow checker to avoid unnecessary clones, and prefer `&str` in function signatures when ownership is not needed.
- Keep public API changes minimal and explain them in the PR description.
- New public items must carry rustdoc comments (`///`).
- Follow the repo's existing module hierarchy rules (dependency direction, mod.rs discipline) — see [🏗️ Architecture](ARCHITECTURE.md)'s Design Principles section.

---

## 🧪 Testing Requirements

- **All existing tests must pass** before merging:

```bash
cargo test --features cli
```

- **New features must come with tests**: cover the happy path and important boundaries (empty inputs, deep nesting, malformed input, etc.).
- **Bug fixes must come with regression tests**: failing before the fix, passing after.
- For changes involving parsing or evaluation, add cases to the relevant table-driven test modules so future refactors cannot silently break behavior.
- Keep tests fast and deterministic — avoid sleeps, network access, and filesystem dependencies where possible.

---

## 📥 Submitting a PR

1. **Create a feature branch from `main`** (see [Development Workflow](#-development-workflow)).
2. **Make your changes**, committing with Conventional Commits messages.
3. **Push the branch** to your fork:

```bash
git push origin feat/support-complex-numbers
```

4. **Open a Pull Request against `kirky-x/calnexus:main`** — the PR template auto-fills; complete every item.
5. **Respond to review feedback**: just add commits (avoid force-pushing during review unless explicitly asked).
6. Once review passes and CI is green, Kirky.X squash-merges your PR.

> For security-related fixes, read the reporting process in [🔒 Security](SECURITY.md) first — do not report security vulnerabilities through public Issues / PRs.

---

## 📚 Related Documents

- [🏗️ Architecture](ARCHITECTURE.md) — Module hierarchy and dependency rules
- [🔒 Security](SECURITY.md) — Vulnerability reporting process
- [Code of Conduct](CODE_OF_CONDUCT.md) — Community standards

Questions? Open a [Discussion](https://github.com/kirky-x/calnexus/discussions) or an [Issue](https://github.com/kirky-x/calnexus/issues). Happy hacking! 🦀
