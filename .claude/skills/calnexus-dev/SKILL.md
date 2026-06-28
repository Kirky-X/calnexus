---
name: calnexus-dev
description: CalNexus 项目使用指南。当需要了解如何构建、运行、使用 CalNexus 计算引擎的 CLI/REPL/批量模式，或查询支持的计算域与函数时使用。
---

# CalNexus 使用指南

## 项目简介

CalNexus 是一个 Rust 原生的命令行数学表达式求值引擎，统一 11 个计算域（算术、科学函数、统计、精度、数论、组合、多项式、复数、矩阵、向量、符号演算），通过单一解析器和基于优先级的域路由器分派计算。

核心管线：**parse → canonicalize → cache → route → evaluate**

## 构建与安装

```bash
# 从代码仓库克隆
git clone https://github.com/kirky-x/calnexus.git
cd calnexus

# 安装到 ~/.cargo/bin
cargo install --path . --features cli

# 本地构建（release）
cargo build --release --features cli

# 不带 CLI 的库构建（更小体积）
cargo build --no-default-features
```

> `cli` feature 控制 `clap`/`rustyline`/`rayon` 及文件 I/O。CLI/REPL/批量模式需要此 feature。

## 三种使用模式

### 1. 单表达式求值

```bash
calnexus '2+3*4'           # → 14
calnexus 'sin(pi/2)'       # → 1
calnexus 'gcd(12, 18)'     # → 6
calnexus 'factorial(5)'    # → 120
calnexus --var x=3 'x^2+1' # → 10
```

### 2. REPL 交互模式

```bash
calnexus --repl
# CalNexus REPL — type :help for commands, :quit to exit
# calnexus> :let x = 10
# calnexus> x*2          → = 20  [arithmetic]
# calnexus> diff(x^2, x) → = 2*x  [symbolic]
# calnexus> :vars        → x = 10
# calnexus> :quit        → bye
```

REPL 命令：`:help`、`:let <var> = <expr>`、`:vars`、`:clear`、`:quit`

### 3. 批量并行处理

```bash
calnexus --batch exprs.txt
# line 1: 2+3 = 5  [arithmetic]
# line 2: sin(0) = 0  [scientific]
# summary: 2 total, 2 ok, 0 errors, 0 cache hits, 0.8ms
```

## CLI 标志

| 标志                | 说明                                        |
| ------------------- | ------------------------------------------- |
| `'表达式'`          | 单表达式求值（位置参数）                     |
| `--repl`            | 启动交互式 REPL                              |
| `--batch <文件>`    | 并行批量求值文件中每行表达式                 |
| `--var x=3`         | 预绑定变量                                   |
| `--precision <N>`   | 以 N 位小数 BigRational 任意精度求值          |
| `--json`            | 输出 `result/domain/cache` JSON 结构         |
| `--latex`           | 输出 LaTeX 格式                              |
| `--steps`           | 输出求值步骤                                 |
| `--canonical`       | 输出规范化 S-表达式                          |
| `--help`            | 查看完整帮助                                 |

> `--latex`/`--canonical`/`--steps` 与 `--json`/`--repl`/`--batch`/`--precision` 互斥。

## 11 个计算域

| 域              | 优先级 | 函数                                                                  |
| --------------- | ------ | --------------------------------------------------------------------- |
| Arithmetic      | 10     | `+` `-` `*` `/` `^` `factorial` `mod` `abs`                          |
| Scientific      | 20     | `sin` `cos` `tan` `asin` `acos` `atan` `ln` `log` `exp` `sinh` `cosh` `tanh` `gamma` `erf` |
| Statistics      | 20     | `mean` `median` `variance` `stddev` `sum` `min` `max`                |
| Precision       | 25     | `precision(N, expr)`                                                  |
| NumberTheory    | 25     | `gcd` `lcm` `is_prime` `prime_sieve` `mod_inverse` `mod_pow` `euler_phi` |
| Combinatorics   | 25     | `P` `C` `catalan` `stirling`                                          |
| Polynomial      | 25     | `poly_add` `poly_sub` `poly_mul` `poly_div` `poly_eval` `poly_diff` `poly_integrate` `roots` `factor` |
| Complex         | 30     | `complex(a,b)` `re` `im` `conj` `magnitude` `phase`                   |
| Matrix          | 30     | `det` `transpose` `inverse` `trace`                                   |
| Vector          | 30     | `dot` `cross` `norm` `angle` `normalize` `scalar_triple`              |
| Symbolic        | 30     | `diff` `integrate` `simplify` `limit` `taylor`                        |

## 常用开发命令

| 任务     | 命令                                               |
| -------- | -------------------------------------------------- |
| 构建     | `cargo build --features cli`                        |
| 测试     | `cargo test --features cli`                         |
| 覆盖率   | `cargo llvm-cov --features cli --summary-only`      |
| Lint     | `cargo clippy --features cli --all-targets`         |
| 格式化   | `cargo fmt`                                         |
| Release  | `cargo build --release --features cli`              |

## 系统限制

- 表达式最大深度：**256**
- 表达式最大长度：**4096 字符**
- 缓存容量：**10000 条目**
