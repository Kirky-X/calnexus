<div align="center">

<img src="docs/assets/logo.png" alt="CalNexus Logo" height="240">

[![Rust 2024](https://img.shields.io/badge/Rust-2024-edded?logo=rust&style=for-the-badge)](https://www.rust-lang.org/) [![Rustc 1.97.1+](https://img.shields.io/badge/Rustc-1.97.1+-orange.svg?style=for-the-badge)](https://www.rust-lang.org/) [![crates.io](https://img.shields.io/crates/v/calnexus?style=for-the-badge)](https://crates.io/crates/calnexus) [![docs.rs](https://img.shields.io/docsrs/calnexus?style=for-the-badge)](https://docs.rs/calnexus) [![downloads](https://img.shields.io/crates/d/calnexus?style=for-the-badge)](https://crates.io/crates/calnexus) [![CI](https://img.shields.io/github/actions/workflow/status/kirky-x/calnexus/ci.yml?branch=main&label=CI&style=for-the-badge)](https://github.com/kirky-x/calnexus/actions/workflows/ci.yml) [![coverage](https://img.shields.io/badge/coverage-90.4%25%20(llvm--cov%20lines)-brightgreen?style=for-the-badge)](https://github.com/kirky-x/calnexus) [![license](https://img.shields.io/crates/l/calnexus?style=for-the-badge)](LICENSE)

**中文** | [English](README_EN.md)

**生产级 Rust 命令行数学表达式求值器，14 个计算域一个入口**

[✨ 功能特性](#-功能特性) • [🚀 快速开始](#-快速开始) • [📚 文档](#-文档) • [💻 示例](#-示例) • [🤝 参与贡献](#-参与贡献)

</div>

---

<div align="center">

### 🎯 写一条算式，路由器替你选解法

通过 `DomainRouter` 优先级匹配自动分发，解析、缓存与并发去重交给五阶段流水线完成：

<table style="width:100%; border-collapse: collapse">
<tr>
<td align="center" width="25%">🧮<br><b>全域求解</b><br><span style="color:#64748B">11 核心 · 3 可选 · 按需编译</span></td>
<td align="center" width="25%">🧠<br><b>微积分</b><br><span style="color:#64748B">求导 · 化简 · 极限 · 泰勒</span></td>
<td align="center" width="25%">⚡<br><b>微秒级命中</b><br><span style="color:#64748B">免序列化 · 去重合并 · 复杂度无关</span></td>
<td align="center" width="25%">🌐<br><b>服务化</b><br><span style="color:#64748B">REST · MCP · 探针限流</span></td>
</tr>
</table>

</div>

---

## 📋 目录

<details open>
<summary>📑 目录</summary>

- [✨ 功能特性](#-功能特性)
- [🚀 快速开始](#-快速开始)
- [🎨 特性标志](#-特性标志)
- [📚 文档](#-文档)
- [💻 示例](#-示例)
- [🌐 服务模式](#-服务模式)
- [🏗️ 架构](#-架构)
- [🧪 测试](#-测试)
- [📊 性能](#-性能)
- [🔒 安全](#-安全)
- [🗺️ 开发路线图](#-开发路线图)
- [🤝 参与贡献](#-参与贡献)
- [📋 更新日志](#-更新日志)
- [📄 许可证](#-许可证)
- [🙏 致谢](#-致谢)
- [📞 联系与支持](#-联系与支持)
- [⭐ Star 历史](#-star-历史)

</details>

---

## ✨ 功能特性

<table style="width:100%; border-collapse: collapse">
<tr>
<td width="50%" style="vertical-align:top; padding: 12px">🧮 <b>11 核心计算域 + 3 可选域</b><br><span style="color:#64748B">算术、科学函数、统计、精度、数论、组合、多项式、复数、矩阵、向量、符号演算；可选：时间 / 单位 / 汇率（feature 门控）</span></td>
<td width="50%" style="vertical-align:top; padding: 12px">🧠 <b>符号微积分</b><br><span style="color:#64748B"><code>diff</code>、<code>integrate</code>、<code>simplify</code>、<code>limit</code>、<code>taylor</code></span></td>
</tr>
<tr>
<td width="50%" style="vertical-align:top; padding: 12px">🔢 <b>任意精度</b><br><span style="color:#64748B"><code>precision(N, expr)</code> 基于 BigRational 的任意精度计算</span></td>
<td width="50%" style="vertical-align:top; padding: 12px">📐 <b>数值线性代数</b><br><span style="color:#64748B"><code>lu</code>、<code>qr</code>、<code>eig</code>、<code>svd</code>、<code>solve</code>（<code>numerical</code> feature，nalgebra f64 近似）</span></td>
</tr>
<tr>
<td width="50%" style="vertical-align:top; padding: 12px">🖥️ <b>三种执行模式</b><br><span style="color:#64748B">单表达式、REPL（Tab 补全 + 变量绑定）、批量并行（rayon）</span></td>
<td width="50%" style="vertical-align:top; padding: 12px">⚡ <b>高性能缓存</b><br><span style="color:#64748B">oxcache 同步字节权重缓存（64MB 字节预算 + 256KB 大结果准入阈值），BLAKE3 单次哈希键，<code>try_get_with</code> 生产级 single-flight 去重</span></td>
</tr>
<tr>
<td width="50%" style="vertical-align:top; padding: 12px">🌐 <b>HTTP + MCP 双协议服务</b><br><span style="color:#64748B">REST API、健康探针、指标导出、优雅关闭；MCP stdio 服务对接 AI 客户端</span></td>
<td width="50%" style="vertical-align:top; padding: 12px">✖️ <b>隐式乘法</b><br><span style="color:#64748B"><code>2x</code>、<code>3(x+1)</code> 等数学惯用写法自动识别</span></td>
</tr>
<tr>
<td width="50%" style="vertical-align:top; padding: 12px">🗂️ <b>JSON 输出契约</b><br><span style="color:#64748B"><code>--json</code> 输出版本化结构（<code>"v":1</code>，附 JSON Schema），便于管道集成</span></td>
<td width="50%" style="vertical-align:top; padding: 12px">🧱 <b>零依赖核心库</b><br><span style="color:#64748B"><code>default = []</code>，可作为嵌入式计算引擎被其他 crate 引用</span></td>
</tr>
</table>

除上述核心能力外，LaTeX / 步骤 / 规范形式三种格式化输出、中英双语错误消息（Fluent 目录内嵌、始终可用，与 `icu` feature 无关）、运行时函数目录（`--list-functions`）、多平台预编译二进制与容器镜像等能力也随版本持续提供；逐项能力与 `Cargo.toml` 的 `[features]` 对应关系见 [🎨 特性标志](#-特性标志) 一节。

---

## 🚀 快速开始

### 📦 安装

<details>
<summary>📄 代码 · bash</summary>

```bash
cargo install calnexus --features cli
```

</details>

要求 Rust 1.97.1 及以上（MSRV，与 CI MSRV 任务一致）。核心库 `default = []` 零依赖；CLI 体验需启用 `cli`。各功能组合（可选计算域、数值线性代数、HTTP 服务等）的安装方式统一见下方 [🎨 特性标志](#-特性标志) 一节。

也可以从 [GitHub Releases](https://github.com/kirky-x/calnexus/releases) 下载多平台预编译二进制（linux x86_64/aarch64 musl 静态、macOS x86_64/aarch64、windows x86_64，附 SHA256SUMS），或从源码构建；wasm32 目标支持 core 与 `cli` 特性的库/二进制构建（CI 门禁），但无 L1 缓存且不承诺运行时可用性：

<details>
<summary>📄 代码 · bash</summary>

```bash
git clone https://github.com/kirky-x/calnexus.git
cd calnexus
cargo install --path . --features cli
```

</details>

### 💡 最小示例

以下示例可直接在终端运行：

<details>
<summary>📄 代码 · bash</summary>

```bash
$ calnexus '2+3*4'
14

$ calnexus 'diff(x^2, x)'
2*x

$ calnexus --json '2+3'
{"cache":"miss","domain":"arithmetic","result":5.0,"v":1}
```

</details>

### 🧭 核心概念

- **计算域路由**：14 个计算域各实现 `CalculationDomain` trait，`DomainRouter` 按优先级调度，首个 `supports()` 命中即路由。
- **求值流水线**：`parse → canonicalize → cache → route → evaluate` 五阶段；规范化 AST 的 BLAKE3 哈希作为缓存键。
- **三种模式**：单表达式、REPL（`:let` / `:vars` / `:quit`，Tab 补全）、批量（`--batch`，rayon 并行）。
- **特性门控**：可选计算域与服务端能力均为独立 feature，编译产物只包含启用的部分，核心库可零依赖。
- **版本化 JSON 契约**：`--json` 输出携带 `"v":1`，Schema 见 [`docs/schema/result-v1.json`](docs/schema/result-v1.json)。

---

## 🎨 特性标志

### 📦 推荐组合

<details>
<summary>📋 表格</summary>

| 组合 | 安装方式 / 特性 | 适用场景 |
|------|----------------|----------|
| 最小库 | `cargo add calnexus`（`default = []`） | 嵌入式引擎，零额外依赖 |
| CLI | `cargo install calnexus --features cli` | 命令行 / REPL / 批量 |
| CLI 全域 | `cargo install calnexus --features cli,time,unit,fx` | 加时间 / 单位 / 汇率域 |
| CLI + 数值 | `cargo install calnexus --features cli,numerical` | 数值线性代数 |
| 双语 CLI | `cargo install calnexus --features cli,icu` | 中英双语错误消息（始终内置）；`icu` 提供标准 BCP-47 语言标签解析 |
| HTTP 服务 | `cargo install calnexus --features server` | REST + MCP 双协议服务 |
| 生产服务 | `cargo install calnexus --features server,ratelimit,docs,observability` | 限流 + Swagger UI + 结构化日志 |
| 全量 | `cargo install calnexus --all-features` | 完整能力集 |

</details>

### 📋 功能矩阵

下表逐项对应 `Cargo.toml` 的 `[features]` 定义，`default = []`。

<details>
<summary>📋 表格</summary>

<table style="width:100%; border-collapse: collapse">
<tr><th style="text-align:left">特性</th><th style="text-align:center">默认</th><th style="text-align:left">说明</th></tr>
<tr><td colspan="3" style="background:#F8FAFC"><b>核心库</b></td></tr>
<tr><td><code>default</code></td><td align="center">✅</td><td>零依赖核心：解析、规范化、14 域路由、缓存，可作嵌入式引擎</td></tr>
<tr><td><code>icu</code></td><td align="center">❌</td><td>ICU4X BCP-47 语言标签解析（中英双语错误消息经 Fluent 内嵌、始终可用，不受此 feature 门控）</td></tr>
<tr><td colspan="3" style="background:#F8FAFC"><b>CLI 与交互</b></td></tr>
<tr><td><code>cli</code></td><td align="center">❌</td><td>CLI / REPL / 批量（clap、rustyline、rayon，经 sdforge/cli 间接启用）</td></tr>
<tr><td colspan="3" style="background:#F8FAFC"><b>可选计算域</b></td></tr>
<tr><td><code>time</code></td><td align="center">❌</td><td>时间计算域，jiff 0.2 + 内嵌 IANA tzdb，14 个函数</td></tr>
<tr><td><code>unit</code></td><td align="center">❌</td><td>物理单位换算域，8 量纲 + 温度仿射</td></tr>
<tr><td><code>fx</code></td><td align="center">❌</td><td>汇率换算域，frankfurter.dev API + 三级缓存 + 网络熔断</td></tr>
<tr><td><code>numerical</code></td><td align="center">❌</td><td>数值线性代数分解（lu/qr/eig/svd/solve/matrix_exp，nalgebra f64）</td></tr>
<tr><td colspan="3" style="background:#F8FAFC"><b>服务端</b></td></tr>
<tr><td><code>http</code></td><td align="center">❌</td><td>HTTP API（sdforge http/graceful/health/context，axum）</td></tr>
<tr><td><code>mcp</code></td><td align="center">❌</td><td>MCP stdio 服务（sdforge/mcp）</td></tr>
<tr><td><code>server</code></td><td align="center">❌</td><td><code>http</code> + <code>mcp</code> 聚合</td></tr>
<tr><td><code>ratelimit</code></td><td align="center">❌</td><td>HTTP 限流中间件（固定窗口 per-IP，<code>CALNEXUS_RATELIMIT_*</code> 可配，探针豁免）</td></tr>
<tr><td><code>docs</code></td><td align="center">❌</td><td>OpenAPI + Swagger UI（<code>/swagger-ui/</code>，带尾斜杠；<code>/api-docs/openapi.json</code> 暂为空 spec，见服务模式指南）</td></tr>
<tr><td><code>observability</code></td><td align="center">❌</td><td>tracing 结构化日志（EnvFilter 消费 <code>RUST_LOG</code>）</td></tr>
</table>

</details>

---

## 📚 文档

<details>
<summary>📋 表格</summary>

| 文档 | 说明 |
|------|------|
| [📖 用户指南](docs/USER_GUIDE.md) | 从安装到进阶的完整使用教程 |
| [📘 API 参考](docs/API_REFERENCE.md) | 表达式求值 API 与直接 API（`CalNexus` 门面）的详细说明 |
| [🏗️ 架构文档](docs/ARCHITECTURE.md) | 设计原则、模块划分与数据流 |
| [📈 性能指南](docs/PERFORMANCE.md) | 基准数据、性能口径与优化建议 |
| [🔒 安全文档](docs/SECURITY.md) | 安全设计、最佳实践与漏洞处理流程 |
| [❓ FAQ](docs/FAQ.md) | 常见问题解答 |
| [🖥️ 服务模式指南](docs/SERVER.md) | HTTP 部署、可观测性与 MCP 接入 |
| [🧪 测试场景矩阵](docs/TEST_SCENARIOS.md) | 测试套件场景穷举矩阵（单元/集成/E2E/属性/模糊/基准） |
| [📋 更新日志](docs/CHANGELOG.md) | 每个版本的变更记录 |
| [🤝 贡献指南](docs/CONTRIBUTING.md) | 如何参与项目开发 |
| [📦 在线 API 文档](https://docs.rs/calnexus) | docs.rs 自动生成的最新文档 |
| [📦 crates.io](https://crates.io/crates/calnexus) | 发布页面 |

</details>

设计与过程文档：[行为准则](docs/CODE_OF_CONDUCT.md)

---

## 💻 示例

求值示例以内联命令行形式提供，复制到终端即可运行，以下覆盖算术、科学函数、数论、任意精度、符号微积分与隐式乘法等核心计算域；REPL 交互（`:let` / `:vars` / Tab 补全）与批量并行求值（`--batch`，rayon）的完整会话示例，以及时间 / 单位 / 汇率 / 数值线性代数等可选域的函数与用法，见 [📖 用户指南](docs/USER_GUIDE.md) 与 [📖 用户指南 · 计算域指南](docs/USER_GUIDE.md#-计算域指南)。

### 🖥️ 求值示例

<details>
<summary>📄 代码 · bash</summary>

```bash
# 基础与科学函数
$ calnexus 'sin(pi/2)'
1
$ calnexus 'gcd(12, 18)'
6

# 任意精度（BigRational）
$ calnexus 'precision(50, 1/3)'
0.33333333333333333333333333333333333333333333333333

# 符号微积分
$ calnexus 'limit(sin(x)/x, x, 0)'
1
$ calnexus 'taylor(exp(x), x, 3)'
1+x+0.5*x^2+0.16666666666666666*x^3

# 隐式乘法
$ calnexus --var x=3 '2x'
6
```

</details>

### 🧰 CLI 旗标

`--repl`、`--batch`、`--var`、`--precision`、`--timeout`、`--cache-size`、`--json`、`--latex` / `--canonical` / `--steps`、`--explain`、`--lang`、`--list-functions`、`--serve-http` / `--serve-mcp`、`--bind` 等全部旗标的行为、输出格式与错误码约定见 [📖 用户指南 · CLI 使用](docs/USER_GUIDE.md#-cli-使用)，也可运行 `calnexus --help` 查看。

---

## 🌐 服务模式

需以 `--features server` 启用（`server = http + mcp`）：`calnexus --serve-http` 提供 REST 求值 API 与 `/health`、`/live`、`/ready`、`/metrics` 运维端点（默认 `127.0.0.1:3000`，`--bind` 可改）；`calnexus --serve-mcp` 以 stdio 传输暴露 `evaluate`、`list_functions` 工具（`fx` 构建额外注册 `fx_budget` / `fx_pricing`），可直接接入 Claude Desktop、Cursor 等客户端。

`/metrics` 契约：Prometheus 文本含缓存三族 `calnexus_cache_{hits,misses,entries}`（含 `# HELP` / `# TYPE` 注释行）+ `sdforge_http_requests_total{route,method,status}` 计数族 + `sdforge_http_request_duration_seconds` 时延直方图族；`?format=json` 调试分支恰含 `hits` / `misses` / `entry_count` / `http_route_count` 四键。route 标签取请求 path（非路由模板），含 id 的高基数 REST 路径会放大序列——本地/内网部署注意。

HTTP 端点清单与语言协商、Docker / Kubernetes 部署、可观测性与 `ratelimit` 限流、MCP 客户端配置与参数约定、错误语义（400 / 422 / 503 与 CLI 退出码契约）及优雅关闭见 [🖥️ 服务模式指南](docs/SERVER.md)。

---

## 🏗️ 架构

CalNexus 采用五层分层架构（入口层 → 编排层 → 计算域层 → 数学函数层 → 核心基础层），依赖方向严格自上而下；`default` 无特性时核心库零依赖，可作嵌入式引擎。核心求值通路为：mathexpr 解析（支持隐式乘法）→ `AstCanonicalizer` 规范化（常量折叠、可交换排序、S-表达式规范形式）→ `CacheManager` 缓存查询（BLAKE3 单次哈希键）→ `DomainRouter` 按优先级路由 → 对应 `CalculationDomain` 求值，缓存命中经 `try_get_with` single-flight 合并返回（克隆 `EvalResult`，无序列化往返）。

五层职责与模块清单、mermaid 架构图与求值时序图、接口隔离 trait、安全与性能设计、ADR 决策记录见 [🏗️ 架构文档](docs/ARCHITECTURE.md)。

---

## 🧪 测试

### 🎯 测试策略

测试金字塔覆盖九层：`src/` 内联单元测试、按形态组织的集成测试（`tests/`：integration、cli_integration、repl_integration、server_http_integration、server_mcp_integration、api_integration、time_unit_fx_integration、numerical_linalg_test）、`tests/e2e` 场景套件（无 `required-features`，内部 `#[cfg]` 门控启用/禁用两侧用例）、属性测试（proptest）、快照测试（insta）、安全测试（DoS 向量与边界攻击）、模糊测试（`fuzz/`，7 个 cargo-fuzz 目标）、Criterion 基准（`benches/`，4 组）与公开 API 文档测试。逐套件场景穷举、文件映射与通过标准见 [🧪 测试场景矩阵](docs/TEST_SCENARIOS.md)。

### ▶️ 运行命令（与 CI 一致）

<details>
<summary>📄 代码 · bash</summary>

```bash
# 全量测试（CI 矩阵六条腿：cli / cli,time,unit,fx / cli,server / cli,server,fx / cli,numerical / all）
cargo test --all-features

# 分腿运行
cargo test --features cli,time,unit,fx    # 含可选域全量
cargo test --features server              # HTTP/MCP 集成
cargo test --features "cli,numerical"     # 数值线性代数

# Lint 与格式门禁（clippy 矩阵：cli / cli,server / all）
cargo clippy --all-features --all-targets -- -D warnings
cargo fmt --all -- --check

# 覆盖率门禁：行覆盖率不低于 90%（llvm-cov）
cargo llvm-cov --features "cli,time,unit,fx" --fail-under-lines 90 --summary-only

# 基准测试
cargo bench --features cli

# 模糊测试（在 fuzz/ 目录下）
cargo fuzz run parser_fuzz
```

</details>

### 📊 测试规模

当前工作区（v0.2.0-rc.2 后）：`#[test]` / `#[tokio::test]` 函数的 grep 统计共 3133 个，其中单元测试 2557（`src/` 内联）、集成与 E2E 共 576（`tests/`，其中 E2E 场景套件 158）、模糊测试目标 7 个、Criterion 基准 4 组；分解数字随版本演进会漂移，权威口径以 `grep -rE "#\[(tokio::)?test\]" src/ tests/ | wc -l` 为准。覆盖率门禁为行覆盖率不低于 90%（llvm-cov，`--features cli,time,unit,fx` 口径，实测 90.4%），CI 执行。逐套件场景穷举见 [🧪 测试场景矩阵](docs/TEST_SCENARIOS.md)。

---

## 📊 性能

基线在开发机（WSL2、linux 6.6）本地采集（criterion 区间估计 median）：缓存命中稳定在微秒级且与表达式复杂度解耦（`2+3` 约 0.90 µs），相对全流水线（`2+3` 约 10.7 µs）约 12 倍提升，解析与规范化为亚微秒级；实际性能取决于表达式复杂度与硬件，可运行 `cargo bench --features cli` 复现。完整基线数据表与测量口径见 [📈 性能指南 · 基线数据](docs/PERFORMANCE.md#-基线数据)；命中免序列化、single-flight、字节权重预算、大结果准入阈值与非确定性旁路等设计要点见 [📈 性能指南 · 缓存设计](docs/PERFORMANCE.md#-缓存设计)。

---

## 🔒 安全

### 🛡️ 安全设计

CalNexus 是无网络（`fx` 上游除外）、无不可信文件 I/O、无插件加载的求值器，攻击面极小；主动防护围绕递归深度防护闭环（`MAX_AST_DEPTH=256` + 迭代预检 + RAII 守卫）、请求资源上限、解析错误脱敏与 fx 上游加固展开。代码级机制细节见 [🏗️ 架构文档 · 安全设计](docs/ARCHITECTURE.md#-安全设计)，支持版本表、漏洞报告流程与安全最佳实践见 [🔒 安全文档](docs/SECURITY.md)。

### ⛓️ 供应链与门禁

`cargo audit`（周度定时工作流 + 发布前门禁）、`cargo deny check`、CodeQL 静态分析与 pre-commit 钩子 9 项检查在 CI 与本地 Git 钩子双重执行，完整清单见 [🔒 安全文档 · 依赖安全](docs/SECURITY.md#依赖安全)。

### 🚨 报告安全漏洞

请勿通过公开 issue 报告安全漏洞。请优先使用 GitHub [Security Advisories](https://github.com/kirky-x/calnexus/security/policy) 私密披露通道提交报告（不可用时再邮件至 Kirky-X@outlook.com，受理人 Kirky.X）。项目承诺 48 小时内确认、7 天内给出初步评估。完整政策见 [SECURITY.md](docs/SECURITY.md)。

---

## 🗺️ 开发路线图

<details>
<summary>📋 表格</summary>

<table style="width:100%; border-collapse: collapse">
<tr><th style="text-align:center">状态</th><th style="text-align:left">方向</th><th style="text-align:left">条目</th></tr>
<tr><td align="center">✅</td><td>核心引擎（v0.1.0）</td><td>11 计算域、符号微积分、REPL、批量处理、任意精度、JSON 输出、隐式乘法</td></tr>
<tr><td align="center">✅</td><td>可选域与服务化（v0.1.3-v0.1.5）</td><td>时间 / 单位 / 汇率域、HTTP + MCP 双协议、探针分离、限流、可观测性、多平台发布与容器镜像</td></tr>
<tr><td align="center">✅</td><td>缓存引擎现代化</td><td>oxcache byte-weight 缓存（moka 内核）、single-flight、字节权重预算、BLAKE3 键、大结果准入阈值</td></tr>
<tr><td align="center">✅</td><td>WebAssembly (wasm32) 支持</td><td>core 与 cli 特性构建进入 CI 门禁：<code>cargo build --lib --bins --target wasm32-unknown-unknown --no-default-features</code> 与 <code>--features cli</code>。wasm 下使用 oxcache 真实 L1 缓存（ByteWeightCache）；<code>wasm-api</code> 可导出 <code>evaluate</code>/<code>list_functions</code> 供浏览器/Node 调用（仅构建门禁，运行时行为不承诺）；<code>fx</code>/<code>http</code>/<code>mcp</code>/<code>icu</code>/<code>time</code>/<code>unit</code> 特性组合不在支持范围</td></tr>
<tr><td align="center">📋</td><td>数值能力增强</td><td>前沿算法跟踪与功能缺口收敛</td></tr>
</table>

</details>

---

## 🤝 参与贡献

详细的贡献流程与代码规范请参阅 [🤝 贡献指南](docs/CONTRIBUTING.md)。

### 🛠️ 开发环境

工具链要求 Rust 1.97.1+（CI MSRV 任务锁定基线）；提交前运行 `cargo fmt --all -- --check` 与 `cargo clippy --all-targets --all-features -- -D warnings`；`.githooks/pre-commit` 提供 9 项检查，通过 `git config core.hooksPath .githooks` 启用；提交信息遵循 Conventional Commits（`feat`、`fix`、`docs` 等）。完整环境搭建步骤见 [🤝 贡献指南 · 环境准备](docs/CONTRIBUTING.md#-环境准备)。

### 💖 贡献方式

<table style="width:100%; border-collapse: collapse">
<tr>
<td width="33%" align="center" style="padding: 16px">

### 🐛 报告 Bug

发现问题？<br>
<a href="https://github.com/kirky-x/calnexus/issues/new">创建 Issue</a>

</td>
<td width="33%" align="center" style="padding: 16px">

### 💡 功能建议

有好想法？<br>
<a href="https://github.com/kirky-x/calnexus/issues/new">发起 Issue 讨论</a><br>
<span style="color:#64748B">本仓库未启用 Discussions，请以 Issue 形式提交</span>

</td>
<td width="33%" align="center" style="padding: 16px">

### 🔧 提交 PR

想贡献代码？<br>
<a href="https://github.com/kirky-x/calnexus/pulls">Fork 并提交 PR</a>

</td>
</tr>
</table>

提交 Issue 时请附复现步骤、`calnexus` 版本与操作系统信息；符号演算 / 精度相关 bug 请附最小复现表达式。

---

## 📋 更新日志

完整版本历史见 [📋 更新日志](docs/CHANGELOG.md)（遵循 [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) 格式，语义化版本）。

<details>
<summary>📋 表格</summary>

| 版本 | 日期 | 要点 |
|------|------|------|
| 0.2.0-rc.1 | 待发布 | i18n 迁移 Fluent、日志栈切换 inklog、E2E 场景套件、依赖全面切换 crates.io、许可补充 Commons Clause（详见 [CHANGELOG](docs/CHANGELOG.md) Unreleased） |
| 0.1.4 | 2026-07-26 | Release pipeline 闭环：`cargo publish` 自动化 + 发布前安全门禁（`cargo audit`、版本校验、dry-run 验证） |
| 0.1.3 | 2026-07-26 | 新增时间 / 单位 / 汇率三个可选计算域与 8 个向量运算；非确定性函数缓存旁路 |
| 0.1.2 | 2026-07-21 | 20 项文档-代码一致性修复；解析与规范化边界行为修复（NaN 全序、`0^0`、连续运算符校验） |

</details>

---

## 📄 许可证

本项目基于 [MIT License](LICENSE) + [Commons Clause](LICENSE) 发布（商业使用需单独授权）。Copyright © 2026 Kirky.X🌠。

---

## 🙏 致谢

### 🌟 核心依赖

CalNexus 站在以下优秀开源项目的肩膀上：

<details>
<summary>📋 表格</summary>

| 依赖 | 用途 |
|------|------|
| [mathexpr](https://crates.io/crates/mathexpr) | 表达式解析基础 |
| [oxcache](https://crates.io/crates/oxcache) | 自研高性能缓存库（`byte-weight` 同步字节权重缓存，机制层封装 moka::sync） |
| [clap](https://github.com/clap-rs/clap) | CLI 参数解析（经 sdforge/cli 间接启用） |
| [rustyline](https://github.com/kkawakam/rustyline) | REPL 行编辑与 Tab 补全 |
| [rayon](https://github.com/rayon-rs/rayon) | 数据并行批量求值 |
| [jiff](https://github.com/BurntSushi/jiff) | 日期时间与 IANA 时区（`time` feature） |
| [nalgebra](https://github.com/dimforge/nalgebra) | 数值线性代数（`numerical` feature） |
| [sdforge](https://crates.io/crates/sdforge) | CLI / HTTP / MCP 接口封装层 |
| [criterion](https://github.com/bheisler/criterion.rs) | 基准测试 |

</details>

### 💝 特别感谢

感谢 Rust 社区与所有 [贡献者](https://github.com/kirky-x/calnexus/graphs/contributors)。

---

## 📞 联系与支持

<table style="width:100%; max-width: 600px">
<tr>
<td align="center" width="33%">
<a href="https://github.com/kirky-x/calnexus/issues"><b style="color:#991B1B">Issues</b></a><br>
<span style="color:#64748B">报告问题和 Bug</span>
</td>
<td align="center" width="33%">
<a href="docs/FAQ.md"><b style="color:#1E40AF">📖 常见问题</b></a><br>
<span style="color:#64748B">查阅文档解答疑问（未启用 Discussions）</span>
</td>
<td align="center" width="33%">
<a href="https://github.com/kirky-x/calnexus"><b style="color:#1E293B">GitHub</b></a><br>
<span style="color:#64748B">查看源代码</span>
</td>
</tr>
</table>

---

## ⭐ Star 历史

[![Star History Chart](https://api.star-history.com/svg?repos=kirky-x/calnexus&type=Date)](https://star-history.com/#kirky-x/calnexus&Date)

如果这个项目对您有帮助，请考虑给它一个 ⭐️！

**由 Kirky.X 构建**

---

<sub>© 2026 Kirky.X. 保留所有权利。</sub>
