<h1 align="center">CalNexus</h1>

<p align="center">
  <b>A command-line math expression evaluator with 11 computation domains, symbolic calculus, REPL, and batch processing.</b><br/>
  <a href="#-quick-start">🚀 Quick Start</a> •
  <a href="#-features">✨ Features</a> •
  <a href="#-contributing">🤝 Contributing</a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/version-1.1.0-blue" alt="version" />
  <img src="https://img.shields.io/badge/license-MIT-green" alt="license" />
  <img src="https://img.shields.io/badge/build-passing-brightgreen" alt="build" />
  <img src="https://img.shields.io/badge/coverage-97.27%25-brightgreen" alt="coverage" />
</p>

***

## 📑 Table of Contents

- [Overview](#-overview)
- [Features](#-features)
- [Architecture](#-architecture)
- [Quick Start](#-quick-start)
  - [Prerequisites](#prerequisites)
  - [Installation](#installation)
  - [Usage](#usage)
- [Configuration](#-configuration)
- [API Documentation](#-api-documentation)
- [Testing](#-testing)
- [WebAssembly (wasm32) Support](#-webassembly-wasm32-support)
- [Contributing](#-contributing)
- [Roadmap](#-roadmap)
- [License](#-license)
- [Acknowledgments](#-acknowledgments)

***

## 🔭 Overview

**CalNexus** is a Rust-native command-line math expression evaluator that unifies 11 computation domains — from arithmetic and statistics to symbolic calculus and linear algebra — behind a single parser and a priority-routed domain dispatcher. It offers three execution modes (single expression, interactive REPL, and parallel batch) with an LRU cache, arbitrary-precision arithmetic, and JSON output for pipeline integration.

### 适用场景

- 场景 A：命令行快速求值与符号演算（`calnexus 'diff(x^2, x)'`）
- 场景 B：交互式探索与变量绑定（`calnexus --repl`，支持 Tab 补全）
- 场景 C：批量脚本化处理（`calnexus --batch exprs.txt`，rayon 并行）
- 场景 D：嵌入到数据管道中（`--json` 输出结构化结果）

***

## ✨ Features

| 特性            | 说明                                                                       |
| ------------- | ------------------------------------------------------------------------ |
| 🧮 11 计算域     | 算术、科学函数、统计、精度、数论、组合、多项式、复数、矩阵、向量、符号演算    |
| 🧠 符号微积分      | `diff`、`integrate`、`simplify`、`limit`、`taylor`                          |
| 🔢 任意精度       | `precision(N, expr)` 基于 BigRational 的任意精度计算                            |
| 💻 三种模式       | 单表达式、REPL（Tab 补全 + 变量绑定）、批量并行（rayon）                       |
| ⚡ 高性能缓存       | Moka L1 缓存（10000 条目，BLAKE3 哈希，线程安全）                             |
| 🔄 隐式乘法        | `2x`、`3(x+1)` 等数学惯用写法自动识别                                          |
| 📦 JSON 输出      | `--json` 输出 `result/domain/cache` 结构，便于管道集成                          |
| 🧪 工业级测试       | 1650 个测试（1369 lib + 108 CLI + 173 集成），覆盖率 97.27%，release 零警告          |

### 11 Computation Domains

| Domain          | Priority | Functions                                                                                                                              |
| --------------- | -------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| **Arithmetic**  | 10       | `+`, `-`, `*`, `/`, `^`, `factorial`, `mod`, `abs`                                                                                    |
| **Scientific**  | 20       | `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `ln`, `log`, `exp`, `sinh`, `cosh`, `tanh`, `gamma`, `erf`                              |
| **Statistics**  | 20       | `mean`, `median`, `variance`, `stddev`, `sum`, `min`, `max`                                                                           |
| **Precision**   | 25       | `precision(N, expr)` — BigRational arbitrary precision                                                                               |
| **NumberTheory**| 25       | `gcd`, `lcm`, `is_prime`, `prime_sieve`, `mod_inverse`, `mod_pow`, `euler_phi`                                                        |
| **Combinatorics**| 25      | `P`, `C`, `catalan`, `stirling`                                                                                                       |
| **Polynomial**  | 25       | `poly_add`, `poly_sub`, `poly_mul`, `poly_div`, `poly_eval`, `poly_diff`, `poly_integrate`, `roots`, `factor`                         |
| **Complex**     | 30       | `complex(a,b)`, `re`, `im`, `conj`, `magnitude`, `phase`                                                                              |
| **Matrix**      | 30       | `det`, `transpose`, `inverse`, `trace`                                                                                                |
| **Vector**      | 30       | `dot`, `cross`, `norm`, `angle`, `normalize`, `scalar_triple`                                                                         |
| **Symbolic**    | 30       | `diff`, `integrate`, `simplify`, `limit`, `taylor`                                                                                    |

### Three Modes

1. **Single expression** — `calnexus '2+3*4'`
2. **REPL** — `calnexus --repl` (interactive, with Tab completion and variable binding)
3. **Batch** — `calnexus --batch exprs.txt` (parallel evaluation with rayon)

***

## 🏗 Architecture

```
parse() → AstCanonicalizer → CacheManager → DomainRouter → Domain::evaluate()
  │            │                    │              │                │
  │            │                    │              │                ├─ ArithmeticDomain
  │            │                    │              │                ├─ ScientificDomain
  │            │                    │              │                ├─ PrecisionDomain
  │            │                    │              │                ├─ NumberTheoryDomain
  │            │                    │              │                ├─ CombinatoricsDomain
  │            │                    │              │                ├─ PolynomialDomain
  │            │                    │              │                ├─ ComplexDomain
  │            │                    │              │                ├─ MatrixDomain
  │            │                    │              │                ├─ VectorDomain
  │            │                    │              │                └─ SymbolicDomain
```

核心模块说明：

- **Parser**: mathexpr-based, with implicit multiplication and complex number preprocessing
- **Canonicalizer**: constant folding, commutative sorting, S-expression canonical form
- **Cache**: Moka L1 cache (10000 entries, BLAKE3 key hash, thread-safe)
- **Router**: Priority-sorted domain dispatch (first `supports()` wins)

***

## 🚀 Quick Start

### Prerequisites

运行本项目前，请确保环境满足以下要求：

| 依赖       | 版本       | 说明                          |
| -------- | -------- | --------------------------- |
| Rust     | >= 1.70  | 工具链（推荐 `rustup` 安装）          |
| Cargo    | 随 Rust   | 构建与包管理                      |
| `cli` feature | 可选     | 启用 CLI / REPL / batch（含 `clap`、`rustyline`、`rayon`） |

### Installation

```bash
# 1. 克隆仓库
git clone https://github.com/kirky-x/calnexus.git
cd calnexus

# 2. 安装到 ~/.cargo/bin
cargo install --path . --features cli

# 或者仅本地构建
cargo build --release --features cli
```

### Usage

#### Single Expression

```bash
$ calnexus '2+3*4'
14

$ calnexus 'sin(pi/2)'
1

$ calnexus 'gcd(12, 18)'
6

$ calnexus 'factorial(5)'
120

$ calnexus --var x=3 'x^2 + 2*x + 1'
16
```

**预期输出（首条命令）：**

```text
14
```

#### Arbitrary Precision

```bash
$ calnexus --precision 50 '1/3'
0.33333333333333333333333333333333333333333333333333
```

#### JSON Output

```bash
$ calnexus --json '2+3'
{"result":5,"domain":"arithmetic","cache":"miss"}
```

#### Symbolic Calculus

```bash
$ calnexus 'diff(x^2, x)'
2*x

$ calnexus 'simplify(x+0)'
x

$ calnexus 'limit(sin(x)/x, x, 0)'
1

$ calnexus 'taylor(exp(x), x, 3)'
1+x+0.5*x^2+0.16666666666666666*x^3
```

#### REPL Mode

```bash
$ calnexus --repl
CalNexus REPL — type :help for commands, :quit to exit
calnexus> :let x = 10
calnexus> x*2
= 20  [arithmetic]
calnexus> diff(x^2, x)
= 2*x  [symbolic]
calnexus> :vars
x = 10
calnexus> :quit
bye
```

#### Batch Processing

```bash
$ cat exprs.txt
2+3
sin(0)
# This is a comment
diff(x^2, x)

$ calnexus --batch exprs.txt
line 1: 2+3 = 5  [arithmetic]
line 2: sin(0) = 0  [scientific]
line 4: diff(x^2, x) = 2*x  [symbolic]
summary: 3 total, 3 ok, 0 errors, 0 cache hits, 1.2ms
```

#### Implicit Multiplication

```bash
$ calnexus --var x=3 '2x'
6

$ calnexus --var x=3 '3(x+1)'
12
```

***

## ⚙️ Configuration

CalNexus 通过命令行参数进行配置，无需配置文件：

| 配置项            | 环境变量 / 参数                | 默认值     | 说明                          |
| -------------- | ------------------------ | ------- | --------------------------- |
| 单表达式求值         | 位置参数 `'2+3*4'`           | —       | 直接对表达式求值并打印结果               |
| REPL 模式        | `--repl`                 | 关闭      | 启动交互式 REPL，支持 `:let`、`:vars` |
| 批量处理           | `--batch <file>`         | 关闭      | 并行求值文件中每行表达式（rayon）         |
| 变量绑定           | `--var x=3`              | —       | 为表达式预绑定变量                   |
| 任意精度           | `--precision <N>`        | 关闭      | 以 N 位小数 BigRational 求值      |
| JSON 输出        | `--json`                 | 关闭      | 输出 `result/domain/cache` 结构 |
| CLI/REPL/batch | `--features cli`（编译期）     | 启用      | 启用 `clap`/`rustyline`/`rayon` |

完整 CLI 子命令与参数可通过 `calnexus --help` 查看。

***

## 📚 API Documentation

CalNexus 是一个 Rust 库 + CLI 二进制项目，接口文档可通过以下方式查看：

- **本地 rustdoc**: `cargo doc --features cli --open` 后访问 `http://localhost:port`
- **核心入口**: `calnexus::parse()` → `AstCanonicalizer` → `CacheManager` → `DomainRouter`
- **Domain trait**: 各计算域实现 `Domain::evaluate()`，通过 `supports()` 路由
- **CLI 帮助**: `calnexus --help` / `calnexus --repl` 内 `:help`

***

## 🧪 Testing

```bash
# 运行全部测试
cargo test --features cli

# Release 构建（零警告）
cargo build --release --features cli
```

1343 tests (1134 lib + 79 CLI + 130 integration), release build with zero warnings.

***

## 🌐 WebAssembly (wasm32) Support

CalNexus targets `wasm32-unknown-unknown` with `--no-default-features` (excludes CLI/REPL/batch).

**Known limitation:** The `oxcache` dependency uses `tokio`, which depends on `mio` — `mio` does not
support `wasm32-unknown-unknown`. To enable wasm32, the cache layer needs to be refactored to use a
wasm-compatible backend (planned for v1.2). Until then, wasm32 builds fail at the `mio` compilation step.

```bash
# Attempted build (currently fails due to tokio/mio):
cargo build --target wasm32-unknown-unknown --no-default-features
```

The `cli` feature gate (`#[cfg(feature = "cli")]`) correctly isolates `clap`/`rustyline`/`rayon` and
all file I/O (`std::fs`, `std::time::Instant` in `batch.rs`). Only the cache's `tokio` dependency
prevents wasm32 compilation.

***

## 🤝 Contributing

我们欢迎所有形式的贡献！请遵循以下流程。

### 提交 Issue

- 描述问题时请提供复现步骤、`calnexus` 版本与操作系统信息
- 符号演算 / 精度相关 bug 请附上最小复现表达式

### 提交 PR

1. Fork 本仓库
2. 创建特性分支 (`git checkout -b feature/amazing-feature`)
3. **Conventional Commits** 规范：`feat:` / `fix:` / `docs:` / `refactor:` / `test:` / `chore:`
4. 提交更改 (`git commit -m 'feat: add new domain'`)
5. 确保通过测试与格式化：

```bash
cargo test --features cli      # 全部测试通过
cargo fmt --all                # 代码格式化
cargo clippy --features cli    # 无警告
```

6. 推送分支 (`git push origin feature/amazing-feature`)
7. 创建 Pull Request

***

## 🗺 Roadmap

- [x] v0.1.0 - 基础算术与科学函数求值
- [x] v0.5.0 - 多计算域路由与缓存层
- [x] v0.8.0 - REPL 模式与变量绑定
- [x] v1.0.0 - 符号微积分（diff/integrate/limit/taylor）与批量处理
- [x] v1.1.0 - 任意精度（BigRational）、JSON 输出、隐式乘法
- [ ] v1.2.0 - wasm32 支持（重构缓存层，移除 tokio/mio 依赖）

***

## 📄 License

本项目基于 [MIT License](./LICENSE) 开源。

***

## 🙏 Acknowledgments

感谢以下项目为本项目提供的支撑：

- [mathexpr](https://crates.io/crates/mathexpr) — 表达式解析基础
- [Moka](https://crates.io/crates/moka) — 高性能并发缓存
- [clap](https://crates.io/crates/clap) — CLI 参数解析
- [rustyline](https://crates.io/crates/rustyline) — REPL 行编辑与 Tab 补全
- [rayon](https://crates.io/crates/rayon) — 数据并行批量求值

***

<p align="center">
  Built with ❤️ by <a href="https://github.com/kirky-x">kirky-x</a>
</p>
