---
name: calnexus-dev
description: CalNexus 项目使用指南。当需要了解如何构建、运行、使用 CalNexus 计算引擎的 CLI/REPL/批量模式，或查询支持的计算域与函数时使用。
---

# CalNexus 使用指南

命令行数学表达式求值引擎，11 个计算域 + 符号微积分 + REPL + 批量处理 + 任意精度。

核心管线：**parse → canonicalize → cache → route → evaluate**

## 构建

```bash
cargo build --features cli
cargo test --features cli
cargo clippy --features cli --all-targets -- -D warnings
```

## 三种使用模式

### 单表达式求值

```bash
calnexus '2+3*4'           # → 14
calnexus 'sin(pi/2)'       # → 1
calnexus --var x=3 'x^2+1' # → 10
```

### REPL 交互

```bash
calnexus --repl
# calnexus> :let x = 10
# calnexus> x*2  → = 20  [arithmetic]
# calnexus> :quit
```

### 批量处理

```bash
calnexus --batch exprs.txt
# summary: 2 total, 2 ok, 0 errors
```

## CLI 标志

| 标志 | 说明 |
| --- | --- |
| `'表达式'` | 单表达式求值 |
| `--repl` | REPL 交互 |
| `--batch <文件>` | 批量求值 |
| `--var x=3` | 预绑定变量 |
| `--precision <N>` | 任意精度 |
| `--json` | JSON 输出 |
| `--latex` | LaTeX 输出 |
| `--steps` | 求解步骤 |
| `--canonical` | S-表达式 |
| `--explain` | 错误解释 |
| `--lang <en\|zh>` | 消息语言 |
| `--serve-http` | HTTP 服务（需 `server` feature） |
| `--serve-mcp` | MCP 服务（需 `server` feature） |

## 11 个计算域

| 域 | 优先级 | 函数 |
| --- | --- | --- |
| Arithmetic | 10 | `+ - * / ^ factorial mod abs` |
| Scientific | 20 | `sin cos tan asin acos atan ln log exp sinh cosh tanh gamma erf` |
| Statistics | 20 | `mean median variance stddev sum min max` |
| Precision | 25 | `precision(N, expr)` |
| NumberTheory | 25 | `gcd lcm is_prime prime_sieve mod_inverse mod_pow euler_phi` |
| Combinatorics | 25 | `P C catalan stirling` |
| Polynomial | 25 | `poly_add poly_sub poly_mul poly_div poly_eval poly_diff poly_integrate roots factor` |
| Complex | 30 | `complex conj arg abs exp ln` |
| Matrix | 30 | `det transpose inverse identity lu qr eig svd solve` |
| Vector | 30 | `dot cross norm angle normalize scalar_triple` |
| Symbolic | 30 | `diff integrate simplify limit taylor` |

## 退出码

`0` 成功 / `1` 计算/解析错误 / `2` 用法错误 / `3` 超时

## 系统限制

- 表达式最大深度：256
- 表达式最大长度：4096 字符
- 缓存容量：10000 条目
