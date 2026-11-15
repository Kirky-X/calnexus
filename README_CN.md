[English](README.md) | 中文

<div align="center">

<img src="docs/assets/logo.png" alt="CalNexus Logo" height="240">

[![Rust 2024](https://img.shields.io/badge/Rust-2024-edded?logo=rust&style=for-the-badge)](https://www.rust-lang.org/) [![Rustc 1.97.1+](https://img.shields.io/badge/Rustc-1.97.1+-orange.svg?style=for-the-badge)](https://www.rust-lang.org/) [![crates.io](https://img.shields.io/crates/v/calnexus?style=for-the-badge)](https://crates.io/crates/calnexus) [![docs.rs](https://img.shields.io/docsrs/calnexus?style=for-the-badge)](https://docs.rs/calnexus) [![downloads](https://img.shields.io/crates/d/calnexus?style=for-the-badge)](https://crates.io/crates/calnexus) [![CI](https://img.shields.io/github/actions/workflow/status/kirky-x/calnexus/ci.yml?branch=main&label=CI&style=for-the-badge)](https://github.com/kirky-x/calnexus/actions/workflows/ci.yml) [![coverage](https://img.shields.io/badge/coverage-95.9%25%20(llvm--cov%20lines)-brightgreen?style=for-the-badge)](https://github.com/kirky-x/calnexus) [![license](https://img.shields.io/crates/l/calnexus?style=for-the-badge)](LICENSE)

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

除上述核心能力外，LaTeX / 步骤 / 规范形式三种格式化输出、中英双语错误消息（Fluent 目录内嵌、始终可用，与 `icu` feature 无关）、运行时函数目录（`--list-functions`）、多平台预编译二进制与容器镜像等能力也随版本持续提供；逐项能力与 `Cargo.toml` 的 `[features]` 对应关系见 [🏗️ 架构文档 · 特性标志速查](docs/cn/ARCHITECTURE.md#-特性标志速查)。

---

## 🚀 快速开始

### 📦 安装

<details>
<summary>📄 代码 · bash</summary>

```bash
cargo install calnexus --features cli
```

</details>

要求 Rust 1.97.1 及以上（MSRV，与 CI MSRV 任务一致）。核心库 `default = []` 零依赖；CLI 体验需启用 `cli`。各功能组合（可选计算域、数值线性代数、HTTP 服务等）的安装方式统一见 [🏗️ 架构文档 · 特性标志速查](docs/cn/ARCHITECTURE.md#-特性标志速查)。

预编译二进制下载与源码构建见 [📖 用户指南 · 安装](docs/cn/USER_GUIDE.md#-安装)；wasm32 支持范围见 [❓ FAQ · 故障排查](docs/cn/FAQ.md#-故障排查)。

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

核心概念（计算域路由、求值流水线、三种执行模式、特性门控与 JSON 契约）见 [📖 用户指南 · 核心概念](docs/cn/USER_GUIDE.md#-核心概念)。

---

## 📚 文档

<details>
<summary>📋 表格</summary>

| 文档 | 说明 |
|------|------|
| [📖 用户指南](docs/cn/USER_GUIDE.md) | 从安装到进阶的完整使用教程 |
| [📘 API 参考](docs/cn/API_REFERENCE.md) | 表达式求值 API 与直接 API（`CalNexus` 门面）的详细说明 |
| [🏗️ 架构文档](docs/cn/ARCHITECTURE.md) | 设计原则、模块划分与数据流 |
| [📈 性能指南](docs/cn/PERFORMANCE.md) | 基准数据、性能口径与优化建议 |
| [🔒 安全文档](docs/cn/SECURITY.md) | 安全设计、最佳实践与漏洞处理流程 |
| [❓ FAQ](docs/cn/FAQ.md) | 常见问题解答 |
| [🖥️ 服务模式指南](docs/cn/SERVER.md) | HTTP 部署、可观测性与 MCP 接入 |
| [🧪 测试场景矩阵](docs/cn/TEST_SCENARIOS.md) | 测试套件场景穷举矩阵（单元/集成/E2E/属性/模糊/基准） |
| [📋 更新日志](docs/cn/CHANGELOG.md) | 每个版本的变更记录 |
| [🤝 贡献指南](docs/cn/CONTRIBUTING.md) | 如何参与项目开发 |
| [📦 在线 API 文档](https://docs.rs/calnexus) | docs.rs 自动生成的最新文档 |
| [📦 crates.io](https://crates.io/crates/calnexus) | 发布页面 |

</details>

设计与过程文档：[行为准则](docs/cn/CODE_OF_CONDUCT.md)

---

## 💻 示例

求值示例以内联命令行形式提供，复制到终端即可运行，以下覆盖算术、科学函数、数论、任意精度、符号微积分与隐式乘法等核心计算域；REPL 交互（`:let` / `:vars` / Tab 补全）与批量并行求值（`--batch`，rayon）的完整会话示例，以及时间 / 单位 / 汇率 / 数值线性代数等可选域的函数与用法，见 [📖 用户指南](docs/cn/USER_GUIDE.md) 与 [📖 用户指南 · 计算域指南](docs/cn/USER_GUIDE.md#-计算域指南)。

### 🧰 CLI 旗标

`--repl`、`--batch`、`--var`、`--precision`、`--timeout`、`--cache-size`、`--json`、`--latex` / `--canonical` / `--steps`、`--explain`、`--lang`、`--list-functions`、`--serve-http` / `--serve-mcp`、`--bind` 等全部旗标的行为、输出格式与错误码约定见 [📖 用户指南 · CLI 使用](docs/cn/USER_GUIDE.md#-cli-使用)，也可运行 `calnexus --help` 查看。

---

## 🌐 服务模式

HTTP 端点清单与语言协商、Docker / Kubernetes 部署、可观测性与 `ratelimit` 限流、MCP 客户端配置与参数约定、错误语义（400 / 422 / 503 与 CLI 退出码契约）及优雅关闭见 [🖥️ 服务模式指南](docs/cn/SERVER.md)。

---

## 🏗️ 架构

五层职责与模块清单、mermaid 架构图与求值时序图、接口隔离 trait、安全与性能设计、ADR 决策记录见 [🏗️ 架构文档](docs/cn/ARCHITECTURE.md)。

---

## 🧪 测试

### 🎯 测试策略

测试金字塔覆盖九层：`src/` 内联单元测试、按形态组织的集成测试（`tests/`：integration、cli_integration、repl_integration、server_http_integration、server_mcp_integration、api_integration、time_unit_fx_integration、numerical_linalg_test）、`tests/e2e` 场景套件（无 `required-features`，内部 `#[cfg]` 门控启用/禁用两侧用例）、属性测试（proptest）、快照测试（insta）、安全测试（DoS 向量与边界攻击）、模糊测试（`fuzz/`，7 个 cargo-fuzz 目标）、Criterion 基准（`benches/`，4 组）与公开 API 文档测试。逐套件场景穷举、文件映射与通过标准见 [🧪 测试场景矩阵](docs/cn/TEST_SCENARIOS.md)。

运行命令（与 CI 一致）见 [🧪 测试场景矩阵 · 执行命令](docs/cn/TEST_SCENARIOS.md#-执行命令与-ci-一致)，当前测试规模快照（v0.2.0）见 [🧪 测试场景矩阵 · 测试规模](docs/cn/TEST_SCENARIOS.md#-测试规模v020)。

---

## 📊 性能

基线在开发机（WSL2、linux 6.6）本地采集（criterion 区间估计 median）：缓存命中稳定在微秒级且与表达式复杂度解耦（`2+3` 约 0.90 µs），相对全流水线（`2+3` 约 10.7 µs）约 12 倍提升，解析与规范化为亚微秒级；实际性能取决于表达式复杂度与硬件，可运行 `cargo bench --features cli` 复现。完整基线数据表与测量口径见 [📈 性能指南 · 基线数据](docs/cn/PERFORMANCE.md#-基线数据)；命中免序列化、single-flight、字节权重预算、大结果准入阈值与非确定性旁路等设计要点见 [📈 性能指南 · 缓存设计](docs/cn/PERFORMANCE.md#-缓存设计)。

---

## 🔒 安全

### 🛡️ 安全设计

CalNexus 是无网络（`fx` 上游除外）、无不可信文件 I/O、无插件加载的求值器，攻击面极小；主动防护围绕递归深度防护闭环（`MAX_AST_DEPTH=256` + 迭代预检 + RAII 守卫）、请求资源上限、解析错误脱敏与 fx 上游加固展开。代码级机制细节见 [🏗️ 架构文档 · 安全设计](docs/cn/ARCHITECTURE.md#-安全设计)，支持版本表、漏洞报告流程与安全最佳实践见 [🔒 安全文档](docs/cn/SECURITY.md)。

### 🚨 报告安全漏洞

请勿通过公开 issue 报告安全漏洞。请优先使用 GitHub [Security Advisories](https://github.com/kirky-x/calnexus/security/policy) 私密披露通道提交报告（不可用时再邮件至 Kirky-X@outlook.com，受理人 Kirky.X）。项目承诺 48 小时内确认、7 天内给出初步评估。完整政策见 [SECURITY.md](docs/cn/SECURITY.md)。

---

## 🗺️ 开发路线图

已交付里程碑见 [📋 更新日志 · 开发路线图](docs/cn/CHANGELOG.md#-开发路线图)。

---

## 🤝 参与贡献

详细的贡献流程与代码规范请参阅 [🤝 贡献指南](docs/cn/CONTRIBUTING.md)。

---

## 📋 更新日志

完整版本历史见 [📋 更新日志](docs/cn/CHANGELOG.md)（遵循 [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) 格式，语义化版本）。

---

## 📄 许可证

本项目基于 [MIT License](LICENSE) + [Commons Clause](LICENSE) 发布（商业使用需单独授权）。Copyright © 2026 Kirky.X🌠。

---

## 🙏 致谢

核心依赖清单见 [🏗️ 架构文档 · 核心依赖](docs/cn/ARCHITECTURE.md#-核心依赖)。

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
<a href="docs/cn/FAQ.md"><b style="color:#1E40AF">📖 常见问题</b></a><br>
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
