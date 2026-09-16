# 🤝 CalNexus 贡献指南

> 首先感谢你愿意为 CalNexus 贡献力量！🎉
> 本文档描述如何搭建开发环境，以及提交代码需要遵循的规范。

## 📋 目录

- [👋 欢迎](#-欢迎)
- [🧰 环境准备](#-环境准备)
  - [前置条件](#前置条件)
  - [Fork 与 Clone](#fork-与-clone)
  - [完成环境搭建](#完成环境搭建)
  - [特性标志](#特性标志)
  - [构建与测试](#构建与测试)
- [🔄 开发工作流](#-开发工作流)
  - [分支与提交](#分支与提交)
  - [提交信息规范（Conventional Commits）](#提交信息规范conventional-commits)
  - [Git 钩子](#git-钩子)
- [🎨 代码风格](#-代码风格)
- [🧪 测试要求](#-测试要求)
- [📥 提交 PR](#-提交-pr)
- [📚 相关文档](#-相关文档)

---

## 👋 欢迎

CalNexus 是一个用 Rust 编写的命令行数学表达式求值器：11 个核心计算域 + 3 个可选计算域（时间 / 单位 / 汇率）、符号微积分、REPL 与批量处理。无论是修复 Bug、新增函数、改进文档还是完善测试，我们都欢迎。

### 贡献方式

| 方式 | 入口 |
|------|------|
| 🐛 报告 Bug | [创建 Issue](https://github.com/kirky-x/calnexus/issues/new)（附复现步骤、`calnexus` 版本与操作系统信息；符号演算/精度 bug 请附最小复现表达式） |
| 💡 功能建议 | [发起讨论](https://github.com/kirky-x/calnexus/discussions) |
| 🔧 提交代码 | 按下文流程 Fork 并提交 PR |

---

## 🧰 环境准备

### 前置条件

| 工具 | 要求 |
|------|------|
| Rust 工具链 | ≥ 1.97.1（[rustup](https://rustup.rs) 安装） |
| `cargo fmt` / `cargo clippy` | 随工具链 |
| protoc | HTTP 集成测试需要（`sudo apt-get install protobuf-compiler`） |

### Fork 与 Clone

```bash
# 1. 在 GitHub 上 Fork 仓库，然后：
git clone https://github.com/<your-username>/calnexus.git
cd calnexus

# 2. 添加 upstream 以便同步 main 分支
git remote add upstream https://github.com/kirky-x/calnexus.git
```

### 完成环境搭建

```bash
# 安装 Rust 工具链（MSRV 1.97.1）
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 启用项目管理的 Git 钩子（pre-commit 9 项检查）
git config core.hooksPath .githooks

# 构建并验证测试套件
cargo build --features cli
cargo test --features cli
```

（可选）如需调整运行时默认值，可复制环境模板：

```bash
cp .env.example .env
```

### 特性标志

开发时按需启用 feature（完整矩阵见 [🏗️ 架构文档](ARCHITECTURE.md) 的 Feature Gate 策略）：

```bash
cargo build --features cli                  # CLI / REPL / 批量
cargo build --features cli,time,unit,fx     # 含可选计算域
cargo build --features server               # HTTP + MCP 服务
cargo build --all-features                  # 全量
```

### 构建与测试

```bash
# 构建项目
cargo build --features cli

# 运行全部测试（CI 矩阵六条腿）
cargo test --features cli
cargo test --features cli,time,unit,fx

# 运行基准测试
cargo bench --features cli
```

---

## 🔄 开发工作流

### 分支与提交

```bash
# 同步 upstream 的 main 分支
git checkout main
git pull upstream main

# 创建特性分支（命名前缀与提交类型一致）
git checkout -b feat/support-complex-numbers
# 或缺陷修复分支
git checkout -b fix/modulo-zero
```

### 提交信息规范（Conventional Commits）

我们遵循 [Conventional Commits](https://conventionalcommits.org) 规范：

```text
<type>(<optional scope>): <description>

[optional body]

[optional footer(s)]
```

接受的 `type` 值：

| Type | 用途 |
|------|------|
| `feat` | 新功能 |
| `fix` | 缺陷修复 |
| `docs` | 仅文档变更 |
| `refactor` | 既非修复也非功能的代码变更 |
| `test` | 补充或修正测试 |
| `chore` | 构建、依赖、工具链等维护任务 |

示例：

```text
feat(parser): support implicit multiplication between number and parenthesis
fix(eval): handle division by zero in modulo operator
docs(readme): clarify supported operators list
test(cache): add regression tests for L1 cache eviction
chore(deps): bump clap to 4.5
```

### Git 钩子

`git config core.hooksPath .githooks` 启用后，pre-commit 钩子会在每次提交前运行 **9 项检查**：

1. `cargo fmt`（格式必须已正确，不自动修复）
2. `cargo clippy`（全 targets，deny warnings）
3. `cargo test`（全部测试通过）
4. `cargo build --release`（零警告）
5. 每个 `.rs` 文件的版权头
6. 库代码中禁止 `println!` / `dbg!` 调试输出
7. `src/` 中禁止 TODO/FIXME/HACK 注释
8. `Cargo.lock` 存在且被追踪
9. 文件体积检查（仅警告，不阻断）

临时绕过（慎用）：`git commit --no-verify`。

---

## 🎨 代码风格

CalNexus 遵循标准 Rust 格式与 lint 约定。

- **提交前格式化**：

```bash
cargo fmt --all
```

- **Clippy 零警告**（全 feature，warnings 视为错误）：

```bash
cargo clippy --all-features --all-targets -- -D warnings
```

- 编写地道 Rust：用 `?` 传播错误，依靠借用检查器避免不必要的 clone，函数签名在无需所有权时优先 `&str`。
- 保持公开 API 变更最小，并在 PR 描述中说明。
- 新增公开项必须带 rustdoc 注释（`///`）。
- 遵循仓库既有的模块层次规则（依赖方向、mod.rs 纪律），见 [🏗️ 架构文档](ARCHITECTURE.md) 的设计原则一节。

---

## 🧪 测试要求

- **所有存量测试必须通过**才能合并：

```bash
cargo test --features cli
```

- **新功能必须附带测试**：覆盖 happy path 与重要边界（空输入、深嵌套、畸形输入等）。
- **缺陷修复必须附带回归测试**：修复前失败、修复后通过。
- 涉及解析或求值的变更，请把用例加入相应的表驱动测试模块，避免未来重构悄悄破坏行为。
- 测试保持快速且确定——尽量避免 sleep、网络访问与文件系统依赖。

---

## 📥 提交 PR

1. **从 `main` 创建特性分支**（见[开发工作流](#-开发工作流)）。
2. **完成修改**，以 Conventional Commits 信息提交。
3. **推送分支**到你的 Fork：

```bash
git push origin feat/support-complex-numbers
```

4. **向 `kirky-x/calnexus:main` 发起 Pull Request**——PR 模板会自动填充，请逐项补全。
5. **响应评审意见**：追加提交即可（评审期间避免 force-push，除非被明确要求）。
6. 评审通过且 CI 全绿后，维护者会 squash-merge 你的 PR。

> 安全相关的修复请先阅读 [🔒 安全文档](SECURITY.md) 的报告流程——安全漏洞请勿走公开 Issue / PR。

---

## 📚 相关文档

- [🏗️ 架构文档](ARCHITECTURE.md) — 模块层次与依赖规则
- [🔒 安全文档](SECURITY.md) — 漏洞报告流程
- [行为准则](CODE_OF_CONDUCT.md) — 社区规范

有问题？欢迎开 [Discussion](https://github.com/kirky-x/calnexus/discussions) 或 [Issue](https://github.com/kirky-x/calnexus/issues)。Happy hacking! 🦀
