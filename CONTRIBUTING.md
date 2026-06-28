# Contributing to CalNexus

First of all, thank you for taking the time to contribute to CalNexus! 🎉

This document describes how to set up a development environment and the conventions to follow when submitting code.

- [Development Environment Setup](#development-environment-setup)
- [Commit Convention](#commit-convention)
- [Pull Request Flow](#pull-request-flow)
- [Code Style](#code-style)
- [Testing Requirements](#testing-requirements)

## Development Environment Setup

CalNexus is a command-line math expression evaluator written in Rust.

1. **Fork** this repository on GitHub.
2. **Clone** your fork locally:
   ```bash
   git clone https://github.com/<your-username>/calnexus.git
   cd calnexus
   ```
3. **Add the upstream remote** to keep your fork in sync:
   ```bash
   git remote add upstream https://github.com/kirky-x/calnexus.git
   ```
4. **Install Rust** (stable toolchain). The recommended way is via [rustup](https://rustup.rs):
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```
5. **Build** the project with the `cli` feature enabled:
   ```bash
   cargo build --features cli
   ```
6. **Run the test suite** to verify everything works:
   ```bash
   cargo test --features cli
   ```
7. (Optional) Copy the environment template if you want to tweak runtime defaults:
   ```bash
   cp .env.example .env
   ```

## Commit Convention

We follow the [Conventional Commits](https://conventionalcommits.org) specification. Each commit message should be structured as:

```
<type>(<optional scope>): <description>

[optional body]

[optional footer(s)]
```

The accepted `type` values are:

| Type       | Purpose                                                         |
|------------|-----------------------------------------------------------------|
| `feat`     | A new feature                                                   |
| `fix`      | A bug fix                                                       |
| `docs`     | Documentation only changes                                      |
| `refactor` | Code changes that neither fix a bug nor add a feature           |
| `test`     | Adding missing tests or correcting existing tests               |
| `chore`    | Build, dependencies, tooling, or other maintenance tasks        |

Examples:

```
feat(parser): support implicit multiplication between number and parenthesis
fix(eval): handle division by zero in modulo operator
docs(readme): clarify supported operators list
test(cache): add regression tests for L1 cache eviction
chore(deps): bump clap to 4.5
```

A properly formatted commit message is enforced on the CI; please make sure your commits conform before pushing.

## Pull Request Flow

1. **Create a feature branch** from `main`, using a descriptive name prefixed with the change type:
   ```bash
   git checkout main
   git pull upstream main
   git checkout -b feat/support-complex-numbers
   ```
2. **Make your changes**, committing with Conventional Commits messages.
3. **Push** the branch to your fork:
   ```bash
   git push origin feat/support-complex-numbers
   ```
4. **Open a Pull Request** against `kirky-x/calnexus:main`. The PR template will be filled in automatically — please complete every section.
5. **Address review feedback** by pushing additional commits (avoid force-pushing during review unless explicitly requested).
6. Once approved and CI is green, a maintainer will squash-merge your PR.

## Code Style

CalNexus follows the standard Rust formatting and lint conventions.

- **Format your code** before committing:
  ```bash
   cargo fmt --all
  ```
- **Run clippy** with warnings treated as errors across all features:
  ```bash
  cargo clippy --all-features -- -D warnings
  ```
- Prefer idiomatic Rust: use `?` for error propagation, leverage the borrow checker instead of cloning unnecessarily, and prefer `&str` over `String` in function signatures where ownership is not required.
- Keep public API changes minimal and document them in the PR description.
- New public items must have rustdoc comments (`///`).

## Testing Requirements

- **All existing tests must pass** before a PR can be merged:
  ```bash
  cargo test --features cli
  ```
- **New features must come with tests.** Aim for meaningful coverage of the happy path and important edge cases (e.g., empty input, deeply nested expressions, malformed input).
- **Bug fixes must include a regression test** that fails before the fix and passes afterward.
- If your change affects parsing or evaluation, add cases to the relevant table-driven test modules so that future refactors do not silently break behavior.
- Keep tests fast and deterministic — avoid sleeping, network access, or filesystem dependencies where possible.

---

If you have any questions, feel free to open a discussion or an issue. Happy hacking! 🦀
