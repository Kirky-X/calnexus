# 📖 CalNexus 用户指南

> 本文档是 CalNexus 的完整使用教程，覆盖从安装到进阶的全部 CLI 能力。
> 快速概览请回到 [README](../README.md)，程序化调用请参阅 [📘 API 参考](API_REFERENCE.md)。

## 📋 目录

<details open>
<summary>📑 目录</summary>

- [🎯 简介](#-简介)
- [🚀 快速开始](#-快速开始)
  - [📌 前置条件](#-前置条件)
  - [📦 安装](#-安装)
  - [💡 第一步](#-第一步)
- [🔑 核心概念](#-核心概念)
  - [三种执行模式](#三种执行模式)
  - [计算域与优先级路由](#计算域与优先级路由)
  - [L1 缓存与非确定性函数](#l1-缓存与非确定性函数)
- [🖥️ CLI 使用](#️-cli-使用)
  - [单表达式求值](#单表达式求值)
  - [变量绑定与隐式乘法](#变量绑定与隐式乘法)
  - [精度控制](#精度控制)
  - [输出格式](#输出格式)
  - [JSON 输出契约](#json-输出契约)
  - [错误输出与退出码](#错误输出与退出码)
  - [错误消息语言](#错误消息语言)
  - [REPL 交互模式](#repl-交互模式)
  - [批量处理](#批量处理)
  - [函数目录](#函数目录)
- [🧮 计算域指南](#-计算域指南)
  - [核心域速查](#核心域速查)
  - [时间域（time feature）](#时间域time-feature)
  - [单位域（unit feature）](#单位域unit-feature)
  - [汇率域（fx feature）](#汇率域fx-feature)
  - [数值线性代数（numerical feature）](#数值线性代数numerical-feature)
- [🌐 服务模式](#-服务模式)
- [⚙️ 环境变量](#️-环境变量)
- [🧰 故障排查](#-故障排查)
- [📚 相关文档](#-相关文档)

</details>

---

## 🎯 简介

**CalNexus** 是一个用 Rust 编写的命令行数学表达式求值器，将 11 个核心计算域 —— 从算术、统计到符号微积分与线性代数 —— 加上 3 个可选计算域（时间 / 单位 / 汇率）统一在单一解析器与按优先级路由的计算域调度器之后。它提供三种执行模式（单表达式、交互式 REPL、并行批量），并配备 L1 缓存、任意精度算术与面向管道集成的 JSON 输出。

---

## 🚀 快速开始

### 📌 前置条件

| 依赖 | 版本 | 说明 |
|------|------|------|
| Rust | >= 1.97.1 | 工具链（推荐使用 `rustup` 安装） |
| Cargo | 随 Rust | 构建与包管理 |

直接使用预编译二进制则无需 Rust 环境（见下文安装）。

### 📦 安装

**方式一：crates.io（推荐）**

```bash
cargo install calnexus --features cli
```

**方式二：预编译二进制**

从 [GitHub Releases](https://github.com/kirky-x/calnexus/releases) 下载对应平台（linux x86_64/aarch64 musl 静态、macOS x86_64/aarch64、windows x86_64）压缩包，校验 SHA256 后解压至 PATH。

**方式三：从源码构建**

```bash
git clone https://github.com/kirky-x/calnexus.git
cd calnexus
cargo install --path . --features cli
```

时间 / 单位 / 汇率域需额外 feature：`cargo install calnexus --features cli,time,unit,fx`。全部 feature 组合见 README 的 [🎨 特性标志](../README.md#-特性标志)。

### 💡 第一步

```bash
$ calnexus '2+3*4'
14
```

出现结果即安装成功。接下来可以试试符号微积分 `calnexus 'diff(x^2, x)'`，或进入交互模式 `calnexus --repl`。

---

## 🔑 核心概念

### 三种执行模式

| 模式 | 命令 | 适用场景 |
|------|------|----------|
| 单表达式 | `calnexus '2+3*4'` | 脚本与命令行快速求值 |
| REPL | `calnexus --repl` | 交互式探索、变量绑定 |
| 批量 | `calnexus --batch exprs.txt` | 文件逐行并行求值（rayon） |

### 计算域与优先级路由

每个计算域实现 `CalculationDomain` trait，声明自己支持的函数集合（`supports()`）与优先级（priority）。`DomainRouter` 按优先级调度，首个命中的域执行求值，输出中的 `[arithmetic]` / `[symbolic]` 等标记即命中域。当前构建支持的完整函数目录可用 `calnexus --list-functions` 查看。

### L1 缓存与非确定性函数

规范化 AST 的 BLAKE3 哈希作为缓存键，命中直接返回零拷贝结果；并发相同表达式经 single-flight 只求值一次。`now` / `today` / `fx` / `fx_rate` 为**非确定性函数**，每次求值旁路缓存读写，避免时间/汇率结果被缓存污染。

---

## 🖥️ CLI 使用

### 单表达式求值

```bash
$ calnexus '2+3*4'
14

$ calnexus 'sin(pi/2)'
1

$ calnexus 'gcd(12, 18)'
6
```

### 变量绑定与隐式乘法

`--var name=value` 为表达式预绑定变量（可多次）：

```bash
$ calnexus --var x=3 'x^2 + 2*x + 1'
16

# 隐式乘法：2x、3(x+1) 等数学惯用写法自动识别
$ calnexus --var x=3 '2x'
6

$ calnexus --var x=3 '3(x+1)'
12
```

未绑定变量会得到带提示的错误：

```bash
$ calnexus 'foo + 1'
error: Evaluation error: Unbound variable: foo
  Hint: define it via --var foo=<value>
```

### 精度控制

两种途径的行为差异（同为 30 位，注意输出对比）：

```bash
# 函数形式：真 BigRational 任意精度（推荐）
$ calnexus 'precision(30, 1/3)'
0.333333333333333333333333333333

# 旗标形式：启用 BigRational 模式，但表达式先经 f64 解析器，精度已在此处损失
$ calnexus --precision 30 '1/3'
0.333333333333333314829616256247
```

需要真正超越 f64 的精度时，请以纯有理数表达式使用 `precision(N, expr)` 函数形式。`precision(N, ...)` 不适用于数值线性代数函数（返回 f64 近似），包裹会得到明确错误。

### 输出格式

| 旗标 | 输出 | 示例 |
|------|------|------|
| （默认） | 文本结果 | `14` |
| `--json` | 版本化 JSON（见下节） | `{"cache":"miss","domain":"arithmetic","result":5.0,"v":1}` |
| `--latex` | LaTeX 形式 | `calnexus --latex 'diff(x^2, x)'` → `\frac{d}{dx}\left(x^{2}\right) = 2 \cdot x` |
| `--canonical` | S-表达式规范形式 | `calnexus --canonical 'y+x'` → `(+ x y)` |
| `--steps` | 求解步骤 | `calnexus --steps '2+3*4'` → `3*4=12`、`2+12=14` |
| `--explain` | 详细错误解释（与 `--json` 互斥） | 见[错误输出](#错误输出与退出码) |

`--latex` / `--canonical` / `--steps` 与 `--json` / `--repl` / `--batch` / `--precision` 互斥。

### JSON 输出契约

`--json` 输出版本化结构（`"v":1`，JSON Schema 见 [`docs/schema/result-v1.json`](schema/result-v1.json)）：

```bash
$ calnexus --json '2+3'
{"cache":"miss","domain":"arithmetic","result":5.0,"v":1}
```

**result 字段形态 × domain 对照表**（v1 契约）：

| result 形态 | 触发域/变体 | 示例 |
|-------------|-------------|------|
| number | 算术/科学/统计等标量结果（Scalar） | `5.0` |
| `{"re":…,"im":…}` | 复数（Complex） | `{"im":4.0,"re":3.0}` |
| string（文本形态） | 矩阵/向量/多项式/符号/BigRational/BigInt/复根列表/JSON 复合 | `"[[1,2],[3,4]]"`、`"0.33333"` |
| string 数组 | 求值步骤（Steps，仅批量/库路径） | `["2+9=11"]` |

### 错误输出与退出码

JSON 模式下错误输出结构化对象：

```bash
$ calnexus --json '1/0'
{"error":{"exit_code":1,"hint":"check divisor before division","kind":"DivisionByZero","message":"division by zero"},"v":1}
```

文本模式下可用 `--explain` 获取完整诊断：

```text
$ calnexus --explain '1/0'
Division by zero: divisor is zero
  Hint: check divisor before division

  Error Kind: DivisionByZero
  Exit Code: 1
  Suggestion: check divisor before division
```

退出码约定：**1** = 求值错误、**2** = 用法/系统错误、**3** = 超时/不可用（如 fx 上游故障）。

### 错误消息语言

`--lang <en|zh>`（默认 `en`）切换人类可读文案：

```bash
$ calnexus --lang zh 'foo + 1'
错误: 求值错误: 未绑定变量: foo
  提示: 请通过 --var foo=<值> 定义
```

### REPL 交互模式

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

REPL 命令：`:help` 帮助、`:let name = value` 绑定变量、`:vars` 查看变量、`:clear` 清屏、`:quit`（或 `:q`）退出。支持 Tab 补全（基于统一函数目录，feature 门控函数在启用后自动出现）。

### 批量处理

每行一个表达式，`#` 开头为注释，rayon 并行求值：

```text
$ calnexus --batch exprs.txt
line 1: 2+3 = 5  [arithmetic]
line 2: sin(0) = 0  [scientific]
line 4: diff(x^2, x) = 2*x  [symbolic]
summary: 3 total, 3 ok, 0 errors, 0 cache hits, 4.589395ms
```

### 函数目录

`--list-functions` 输出当前构建的完整函数目录（按域分组，feature 门控域仅在启用时出现）：

```text
$ calnexus --list-functions
arithmetic:
  abs
  mod
  factorial
scientific:
  ...
```

---

## 🧮 计算域指南

### 核心域速查

| 计算域 | 优先级 | 代表函数 |
|--------|--------|----------|
| Arithmetic | 10 | `+` `-` `*` `/` `^` `factorial` `mod` `abs` |
| Scientific | 20 | `sin` `cos` `tan` `ln` `log10` `log2` `exp` `gamma` `erf` |
| Statistics | 20 | `mean` `median` `variance` `std` `sum` `min` `max` `count` |
| Precision | 25 | `precision(N, expr)` |
| NumberTheory | 25 | `gcd` `lcm` `is_prime` `prime_sieve` `mod_inverse` `mod_pow` `euler_phi` |
| Combinatorics | 25 | `P` `C` `catalan` `stirling` |
| Polynomial | 25 | `poly_add` `poly_mul` `roots` `factor` `poly_diff` |
| Complex | 30 | `complex(a,b)` `conj` `arg` `abs` `exp` `ln` |
| Matrix | 30 | `det` `transpose` `inverse` `identity` |
| Vector | 30 | `dot` `cross` `norm` `angle` `normalize` `cosine_similarity` + Hadamard 积 `[a,b]*[c,d]` |
| Symbolic | 30 | `diff` `integrate` `simplify` `limit` `taylor` |

### 时间域（time feature）

14 个函数：`date` / `datetime` / `timestamp` / `from_timestamp` / `date_diff` / `date_add` / `parse_date` / `format_date` / `reformat_date` / `weekday` / `day_of_year` / `is_leap_year` / `now` / `today`。

基于 jiff 0.2，内嵌 IANA tzdb，支持跨时区日期/时间构造、算术间隔与多格式自动识别（ISO 8601 / 中文 / 英文月份名）：

```bash
$ calnexus 'weekday("2026-01-01")'
4

$ calnexus 'date_add("2026-01-01", 30, "days")'
2026-01-31T00:00:00+00:00

$ calnexus 'date_diff("2026-01-01", "2026-09-15")'
257

$ calnexus 'format_date("2026-01-01", "%Y/%m/%d")'
2026/01/01
```

### 单位域（unit feature）

`convert(value, "from", "to")`：8 量纲线性换算（长度 / 质量 / 体积 / 面积 / 速度 / 数据 / 时间）+ 温度仿射换算（C/F/K/R）。未知单位附 Levenshtein 距离 ≤2 的相近建议：

```bash
$ calnexus 'convert(1000, "m", "km")'
1

$ calnexus 'convert(0, "C", "F")'
32
```

### 汇率域（fx feature）

`fx(value, "FROM", "TO")` / `fx_rate("FROM", "TO")`：经 frankfurter.dev 开放 API 拉取欧洲央行参考汇率，三级缓存（内存 → 文件 → 网络）+ 网络熔断（连续失败快速失败）。`fx` / `fx_rate` 为非确定性函数，旁路缓存。

> 汇率数据仅供参考，不构成交易建议。

环境变量：`CALNEXUS_FX_TTL_HOURS`（快照有效期，默认 24）、`CALNEXUS_FX_ALLOW_STALE`（网络失败时是否使用过期快照）、`CALNEXUS_FX_BREAKER_THRESHOLD` / `CALNEXUS_FX_BREAKER_COOLDOWN_SECS`（熔断阈值 / 冷却，默认 3 次 / 30 秒）。

### 数值线性代数（numerical feature）

5 类数值分解返回 JSON（`lu`/`qr`/`eig`/`svd`）或向量（`solve`），结果为 nalgebra f64 近似：

```bash
$ calnexus 'solve([[2,1],[1,3]],[3,5])'
[0.8,1.4]

$ calnexus 'lu([[4,3],[6,3]])'
{"L":[[1.0,0.0],[0.6666666666666666,1.0]],"P":[[0.0,1.0],[1.0,0.0]],"U":[[6.0,3.0],[0.0,1.0]]}
```

---

## 🌐 服务模式

`--serve-http` 提供 REST API 与运维端点，`--serve-mcp` 提供 MCP stdio 服务（均需 `server` feature）。端点清单、部署、可观测性与 MCP 客户端配置见服务模式指南：

- [🖥️ 服务模式指南](SERVER.md) — HTTP 端点与部署、可观测性、MCP 客户端配置与工具清单

---

## ⚙️ 环境变量

配置优先级：**命令行旗标 > 环境变量 > 默认值**。

| 环境变量 | 默认 | 说明 |
|----------|------|------|
| `CALNEXUS_TIMEOUT` | — | 求值超时（秒），对应 `--timeout` |
| `CALNEXUS_CACHE_SIZE` | 64MB（字节权重） | 缓存预算，对应 `--cache-size`（条目 × 4KB 近似） |
| `CALNEXUS_BIND_ADDR` | `127.0.0.1:3000` | HTTP 服务监听地址，对应 `--bind` |
| `CALNEXUS_FX_TTL_HOURS` | 24 | fx 汇率快照有效期 |
| `CALNEXUS_FX_ALLOW_STALE` | — | 网络失败时是否使用过期快照 |
| `CALNEXUS_FX_BREAKER_THRESHOLD` | 3 | fx 网络熔断失败阈值 |
| `CALNEXUS_FX_BREAKER_COOLDOWN_SECS` | 30 | fx 熔断冷却时长（秒） |
| `CALNEXUS_RATELIMIT_LIMIT` | 120 | HTTP 限流窗口内请求数（`ratelimit` feature） |
| `CALNEXUS_RATELIMIT_WINDOW_SECS` | 60 | HTTP 限流窗口时长（`ratelimit` feature） |
| `RUST_LOG` | `warn` | 日志级别（`observability` feature） |

---

## 🧰 故障排查

常见问题与解答见 [❓ FAQ](FAQ.md)，包括：精度不符合预期、fx 网络失败的行为、wasm32 构建状态、feature 组合选择等。

---

## 📚 相关文档

- [📘 API 参考](API_REFERENCE.md) — 程序化调用（表达式 API 与直接 API）
- [🏗️ 架构文档](ARCHITECTURE.md) — 设计原则、模块划分与数据流
- [📈 性能指南](PERFORMANCE.md) — 基准数据与优化建议
- [❓ FAQ](FAQ.md) — 常见问题解答
- [🖥️ 服务模式指南](SERVER.md) — 服务模式部署与 MCP 接入
- [📋 更新日志](CHANGELOG.md) — 版本变更记录
