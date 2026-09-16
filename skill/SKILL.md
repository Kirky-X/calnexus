---
name: calnexus-dev
description: CalNexus 项目使用指南。当需要了解如何构建、运行、使用 CalNexus 计算引擎的 CLI/REPL/批量/服务模式，或查询支持的计算域与函数时使用。
---

# CalNexus

生产级命令行数学表达式求值器：11 个核心计算域 + 4 个可选域（time/unit/fx/numerical）+ 符号微积分 + REPL + 批量处理 + 任意精度 + HTTP/MCP 服务模式。

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
| `cli` | CLI / REPL / 批量 | `clap`（经 sdforge/cli）、`rustyline`、`rayon` |
| `numerical` | 数值线性代数分解 | 表达式 `lu`/`qr`/`eig`/`svd`/`solve`（nalgebra f64）；门面另有 `matrix_exp` |
| `time` | 时间计算域 | `date` / `datetime` / `timestamp` / `date_diff` / `date_add` / `weekday` 等 14 函数（jiff 0.2 + 内嵌 IANA tzdb） |
| `unit` | 物理单位换算 | `convert(value, "from", "to")`，8 量纲 + 温度仿射换算 |
| `fx` | 汇率换算 | `fx(value, "FROM", "TO")`、`fx_rate("FROM", "TO")`（frankfurter.dev + 三级缓存 + 熔断） |
| `icu` | ICU4X 国际化 | BCP-47 语言标签解析（错误消息中英双语） |
| `http` | HTTP API | `POST /api/v1/evaluate`、`POST /api/v1/list_functions`；`GET /health` `/ready` `/live` `/metrics`（sdforge、axum） |
| `mcp` | MCP 工具 | `evaluate` + `list_functions`（stdio，sdforge） |
| `server` | HTTP + MCP 聚合 | 展开为 `http` + `mcp`，启用 `--serve-http` / `--serve-mcp` 旗标 |
| `fx` + `server` | FX 服务端点 | HTTP 追加 `POST /api/v1/fx_budget`、`/api/v1/fx_pricing`；MCP 追加同名 2 工具（共 4） |
| `ratelimit` | HTTP 限流中间件 | 固定窗口（`CALNEXUS_RATELIMIT_LIMIT` / `CALNEXUS_RATELIMIT_WINDOW_SECS`，超限 429 + Retry-After） |
| `docs` | OpenAPI + Swagger UI | 挂载 `/swagger-ui/` |
| `observability` | tracing 日志 | server 模式 EnvFilter 消费 `RUST_LOG` |

默认 `default = []`，按需启用。

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
calnexus 'integrate(x^2, x)'        # x^3/3
calnexus 'simplify(x+0)'            # x
calnexus 'limit(sin(x)/x, x, 0)'    # 1
calnexus 'taylor(exp(x), x, 3)'     # 1+x+0.5*x^2+0.16666666666666666*x^3
```

### 任意精度

```bash
calnexus 'precision(50, 1/3)'
# 0.33333333333333333333333333333333333333333333333333（全程 BigRational 精确）
```

`--precision N` 旗标（N ≤ 10000）切到精度模式，但表达式先经 f64 求值再转换（`--precision 50 '1/3'` 输出 `0.33333333333333331482…`，带 f64 桥接误差）；需要符号级精确值时使用 `precision(N, expr)` 函数。

### 数值线性代数（需 `numerical` feature）

```bash
calnexus 'solve([[2,1],[1,3]],[3,5])'  # [0.8,1.4]
calnexus 'lu([[4,3],[6,3]])'           # JSON: L/P/U 分解
calnexus 'eig([[2,1],[1,2]])'          # JSON: 特征值/特征向量
```

### JSON 输出（管道集成）

```bash
calnexus --json '2+3'
# {"cache":"miss","domain":"arithmetic","result":5.0,"v":1}
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

REPL 命令：`:let` 绑定变量、`:vars` 查看变量、`:clear` 清空变量、`:help` 帮助、`:quit` / `:q` 退出。

### 批量处理（rayon 并行）

```bash
calnexus --batch exprs.txt
# line 1: 2+3 = 5  [arithmetic]
# line 2: sin(0) = 0  [scientific]
# summary: 2 total, 2 ok, 0 errors, 0 cache hits, 0.3ms
```

一行一表达式；`#` 注释与空行跳过；全部成功退出码 0、部分失败 1、系统错误（超限/文件不可读）2。`--json` 时输出逐行结果数组（成功行含 `result`/`domain`/`cache`，失败行含 `error`）。上限：1000 行、单行 4096 字符。

### 其他输出格式

```bash
calnexus --latex '[[1,2],[3,4]]'             # LaTeX 渲染（pmatrix）
calnexus --canonical '3+2'                  # S-表达式：(+ 2 3)
calnexus --steps '(2+9)*7-6'                # 求解步骤
```

### 服务模式（需 `server` feature）

```bash
calnexus --serve-http --bind 127.0.0.1:3000   # HTTP（默认 127.0.0.1:3000）
calnexus --serve-mcp                          # MCP（stdio）
```

## 计算域（11 核心 + 4 可选）

| 域 | 优先级 | 函数（表达式面，经路由实测） |
| --- | --- | --- |
| Arithmetic | 10 | `+ - * / ^ mod abs factorial` |
| Scientific | 20 | `sin cos tan asin acos atan ln log(x,base) log10 log2 exp sinh cosh tanh gamma erf` |
| Statistics | 20 | `mean median variance std sum min max count` |
| Precision | 25 | `precision(N, expr)` BigRational |
| NumberTheory | 25 | `gcd lcm is_prime prime_sieve mod_inverse mod_pow euler_phi` |
| Combinatorics | 25 | `P(n,k) C(n,k) catalan stirling` |
| Polynomial | 25 | `poly_add poly_sub poly_mul poly_div poly_eval poly_diff poly_integrate roots factor` |
| Complex | 30 | 复数字面量 `3+4i`；`complex(a,b) conj arg abs` |
| Matrix | 30 | `det transpose inverse identity` + `lu qr eig svd solve`（`numerical` feature） |
| Vector | 30 | `dot cross norm angle normalize scalar_triple cosine_similarity euclidean manhattan project reflect outer lerp` + Hadamard 积 `[a,b]*[c,d]` |
| Symbolic | 30 | `diff integrate simplify limit taylor` |
| Time（`time`） | 30 | `date datetime timestamp from_timestamp date_diff date_add weekday day_of_year is_leap_year now today` 等 14 函数 |
| Unit（`unit`） | 30 | `convert(value, "from", "to")` |
| FX（`fx`） | 30 | `fx(value, "FROM", "TO")`、`fx_rate("FROM", "TO")` |
| Numerical（`numerical`） | — | 经 Matrix 域委托 `lu qr eig svd solve`（无独立域） |

路由策略：按优先级降序遍历，首个 `supports()` 命中即路由。`calnexus --list-functions` 输出运行时函数目录（按域分组）。

> 注：`--list-functions` 目录与域实际路由存在少量漂移（目录多列了 `trace`/`re`/`im`/`magnitude`/`phase`/`stddev`，漏列 `count`/`identity`/`log10`/`log2`）；本表以实际可路由行为为准。

## 架构链路

```
parse → AstCanonicalizer → CacheManager → DomainRouter → CalculationDomain::evaluate
```

- **Parser**：mathexpr 基础，支持隐式乘法（`2x`、`3(x+1)`）、复数后缀（`4i`）与括号字面量预处理；迭代预检拒绝超 256 层嵌套
- **Canonicalizer**：常量折叠 + 可交换排序 + S-表达式规范形式（`1+2` 与 `3+0` 折叠同键）
- **Cache**：oxcache 字节权重缓存（默认 64 MiB 预算、单条准入 256 KiB、BLAKE3 键哈希、`try_get_with` single-flight；`--cache-size N` 条目预算按 N×4KiB 折算）；错误结果不入缓存，`now`/`today`/`fx` 等非确定性函数旁路
- **Router**：按优先级调度计算域；`evaluate_with_router` 支持注入自定义域

## 退出码

- `0` 成功 / `1` 计算/解析错误 / `2` 用法错误 / `3` 超时或依赖不可用（如 FX 上游熔断）

## 测试与质量

```bash
cargo test --all-features                              # 全量测试（约 3000 用例）
cargo test --all-features --test e2e                   # E2E 场景套件（tests/e2e/，五类场景 139 用例）
cargo clippy --all-features --all-targets -- -D warnings  # 零警告
calnexus --list-functions                              # 运行时函数目录（按域分组）
cargo bench --features cli                             # criterion 基准
```

CI 矩阵：clippy 三腿（`cli` / `cli,server` / all）+ test 六腿（`cli` / `cli,time,unit,fx` / `cli,server` / `cli,server,fx` / `cli,numerical` / all）+ 单 feature check。覆盖率门禁为行覆盖率 ≥ 90%（llvm-cov，`--features "cli,time,unit,fx"` 口径，实测 90.4%）。测试用例权威口径见 README（v0.1.4：3007 个 `#[test]` 函数）。

## 详细参考

| 文档 | 内容 |
| --- | --- |
| [README.md](../README.md) | 完整使用文档（中文） |
| [README_EN.md](../README_EN.md) | English documentation |
| [docs/ARCHITECTURE.md](../docs/ARCHITECTURE.md) | 架构设计、模块说明与架构决策记录（ADR） |
| [docs/TEST_SCENARIOS.md](../docs/TEST_SCENARIOS.md) | 逐套件测试场景矩阵 |
| [docs/CHANGELOG.md](../docs/CHANGELOG.md) | 版本变更记录 |
| [docs/CONTRIBUTING.md](../docs/CONTRIBUTING.md) | 贡献流程 |
| [docs/SECURITY.md](../docs/SECURITY.md) | 安全策略 |
| [docs/archive/](../docs/archive/) | 历史过程文档归档（PRD、研究分析） |
| [LICENSE](../LICENSE) | MIT 许可证 |

## 许可证

MIT License © Kirky.X
