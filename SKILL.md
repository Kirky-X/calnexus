---
name: calnexus
description: CalNexus CLI 数学表达式求值器。触发场景：数学计算/符号微积分/矩阵运算/数论/批量求值/REPL 交互/任意精度计算。当用户需要求值数学表达式、求导积分、解线性代数、判断素数、组合数计算时使用。
when_to_use: calnexus、求值、diff、integrate、simplify、limit、taylor、matrix、
  determinant、gcd、lcm、is_prime、prime_sieve、mod_inverse、mod_pow、euler_phi、
  polynomial、complex、vector、statistical、precision、REPL、batch、JSON output。
---

# CalNexus

命令行数学表达式求值器，11 个计算域 + 符号微积分 + REPL + 批量处理 + 任意精度。

## 安装

### 方式一：cargo install（推荐）

```bash
cargo install calnexus --features cli
```

启用数值线性代数（`lu`/`qr`/`eig`/`svd`/`solve`）：

```bash
cargo install calnexus --features "cli numerical"
```

启用 HTTP + MCP 服务模式：

```bash
cargo install calnexus --features "cli server"
```

### 方式二：源码安装

```bash
git clone https://github.com/kirky-x/calnexus.git
cd calnexus
cargo build --release --features cli
# 二进制位于 target/release/calnexus
cargo install --path . --features cli
```

## Feature Flags

| Feature | 说明 | 启用模块 |
| --- | --- | --- |
| `cli` | CLI / REPL / 批量 | `clap`、`rustyline`、`rayon` |
| `numerical` | 数值线性代数 | `lu`/`qr`/`eig`/`svd`/`solve`（nalgebra f64） |
| `icu` | ICU4X 国际化错误消息 | 中英双语错误 |
| `http` | HTTP API（POST /api/v1/evaluate） | `sdforge`、`axum` |
| `mcp` | MCP 工具（evaluate，stdio） | `sdforge`、`anyhow` |
| `server` | HTTP + MCP 聚合 | `http` + `mcp` |

默认 `default = []`，按需启用。Rust 项目依赖使用 `x.x` minor 级版本格式。

## 快速使用

### 单表达式

```bash
calnexus '2+3*4'                    # 14
calnexus 'sin(pi/2)'                # 1
calnexus 'gcd(12, 18)'              # 6
calnexus 'factorial(5)'             # 120
calnexus --var x=3 'x^2 + 2*x + 1'  # 16
```

### 符号微积分

```bash
calnexus 'diff(x^2, x)'             # 2*x
calnexus 'integrate(x^2, x)'        # 0.3333333333333333*x^3
calnexus 'simplify(x+0)'            # x
calnexus 'limit(sin(x)/x, x, 0)'    # 1
calnexus 'taylor(exp(x), x, 3)'     # 1+x+0.5*x^2+0.16666666666666666*x^3
```

### 任意精度

```bash
calnexus --precision 50 '1/3'
# 0.33333333333333333333333333333333333333333333333333
```

### 数值线性代数（需 `numerical` feature）

```bash
calnexus 'solve([[2,1],[1,3]],[3,5])'  # [0.8,1.4]
calnexus 'lu([[4,3],[6,3]])'           # JSON: L/P/U 分解
calnexus 'eig([[2,1],[1,2]])'          # JSON: 特征值/特征向量
```

### JSON 输出（管道集成）

```bash
calnexus --json '2+3'
# {"result":5,"domain":"arithmetic","cache":"miss"}
```

### REPL 模式

```bash
calnexus --repl
# CalNexus REPL — type :help for commands, :quit to exit
# calnexus> :let x = 10
# calnexus> x*2
# = 20  [arithmetic]
# calnexus> :quit
```

REPL 命令：`:let` 绑定变量、`:vars` 查看变量、`:help` 帮助、`:quit` 退出。

### 批量处理（rayon 并行）

```bash
calnexus --batch exprs.txt
# line 1: 2+3 = 5  [arithmetic]
# line 2: sin(0) = 0  [scientific]
# summary: 2 total, 2 ok, 0 errors, 0 cache hits, 0.3ms
```

### 其他输出格式

```bash
calnexus --latex 'matrix([[1,2],[3,4]])'    # LaTeX 渲染
calnexus --canonical '3+2'                  # S-表达式：(+ 2 3)
calnexus --steps '(2+9)*7-6'                # 求解步骤
```

## 11 个计算域

| 域 | 优先级 | 函数示例 |
| --- | --- | --- |
| Arithmetic | 10 | `+ - * / ^ factorial mod abs` |
| Scientific | 20 | `sin cos tan ln log exp gamma erf` |
| Statistics | 20 | `mean median variance stddev sum min max` |
| Precision | 25 | `precision(N, expr)` BigRational |
| NumberTheory | 25 | `gcd lcm is_prime prime_sieve mod_inverse mod_pow euler_phi` |
| Combinatorics | 25 | `P C catalan stirling` |
| Polynomial | 25 | `poly_add poly_mul poly_div roots factor` |
| Complex | 30 | `complex re im conj magnitude phase` |
| Matrix | 30 | `det transpose inverse identity lu qr eig svd solve` |
| Vector | 30 | `dot cross norm angle normalize scalar_triple` |
| Symbolic | 30 | `diff integrate simplify limit taylor` |

路由策略：按优先级遍历，首个 `supports()` 命中即路由。

## 架构链路

```
parse → AstCanonicalizer → CacheManager → DomainRouter → Domain::evaluate
```

- **Parser**：mathexpr 基础，支持隐式乘法（`2x`、`3(x+1)`）与复数预处理
- **Canonicalizer**：常量折叠 + 可交换排序 + S-表达式规范形式
- **Cache**：LRU（10000 条目，BLAKE3 键哈希，线程安全）
- **Router**：按优先级调度计算域

## 退出码

- `0` 成功 / `1` 计算/解析错误 / `2` 用法错误 / `3` 超时

## 测试与质量

```bash
cargo test --features cli                              # 全量测试（1600+ 用例）
cargo test --features server --lib                     # server 模块测试
cargo clippy --all-features --all-targets -- -D warnings  # 零警告
cargo bench --features cli                             # criterion 基准
```

覆盖率 ≥ 95%（tarpaulin 门禁）。多 feature 组合测试，非 `--all-features` 单测。

## 详细参考

| 文档 | 内容 |
| --- | --- |
| [README.md](./README.md) | 完整使用文档（中文） |
| [README_EN.md](./README_EN.md) | English documentation |
| [docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md) | 架构设计与模块说明 |
| [docs/CHANGELOG.md](./docs/CHANGELOG.md) | 版本变更记录 |
| [docs/CONTRIBUTING.md](./docs/CONTRIBUTING.md) | 贡献流程 |
| [docs/SECURITY.md](./docs/SECURITY.md) | 安全策略 |
| [docs/PRD.md](./docs/PRD.md) | 产品需求文档 |
| [docs/TEST.md](./docs/TEST.md) | 测试策略与覆盖 |
| [docs/ADD.md](./docs/ADD.md) | 架构决策记录 |
| [LICENSE](./LICENSE) | MIT 许可证 |

## 许可证

MIT License © Kirky.X
