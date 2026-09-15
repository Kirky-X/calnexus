# ❓ CalNexus FAQ

> 常见问题解答。使用教程见 [📖 用户指南](USER_GUIDE.md)，问题未覆盖时欢迎[提 Issue](https://github.com/kirky-x/calnexus/issues/new)。

## 📋 目录

- [🧭 通用问题](#-通用问题)
- [📦 安装与特性](#-安装与特性)
- [🛠️ 使用与特性](#️-使用与特性)
- [⚡ 性能](#-性能)
- [🌐 服务模式](#-服务模式)
- [🔧 故障排查](#-故障排查)

---

## 🧭 通用问题

### ❓ 什么是 CalNexus？

一个用 Rust 编写的命令行数学表达式求值器：11 个核心计算域（算术、科学函数、统计、精度、数论、组合、多项式、复数、矩阵、向量、符号演算）+ 3 个可选计算域（时间 / 单位 / 汇率），统一在单一解析器与按优先级路由的域调度器之后，提供单表达式、REPL、并行批量三种模式与 HTTP / MCP 双协议服务。

### ❓ CalNexus 可以用于生产环境吗？

可以。核心库 `default = []` 零依赖；2820 个测试、90.4% 行覆盖门禁（llvm-cov）、全 feature 组合零警告、CI 六腿测试矩阵与服务化错误语义（400/422/503）为生产部署提供保障。部署指引见 [🖥️ 服务模式指南](SERVER.md)。

### ❓ 支持哪些平台？

linux x86_64/aarch64（musl 静态）、macOS x86_64/aarch64、windows x86_64 有预编译二进制；其余平台可从源码构建。wasm32 为实验性路线图目标，当前不可构建（见下文故障排查）。

### ❓ 项目采用什么许可证？

MIT。详见 [LICENSE](../LICENSE)。

### ❓ 如何参与贡献？

参阅 [🤝 贡献指南](CONTRIBUTING.md)。安全漏洞请勿走公开 Issue，参见 [🔒 安全文档](SECURITY.md) 的报告流程。

---

## 📦 安装与特性

### ❓ 如何安装？

```bash
cargo install calnexus --features cli     # CLI（推荐）
cargo add calnexus                        # 库（零依赖核心）
```

也可从 [GitHub Releases](https://github.com/kirky-x/calnexus/releases) 下载预编译二进制（附 SHA256SUMS）。完整安装方式见 [📖 用户指南](USER_GUIDE.md)。

### ❓ 如何选择合适的 feature 组合？

| 需求 | 组合 |
|------|------|
| 嵌入式计算引擎 | 无 feature（`default = []`，零依赖） |
| 日常命令行 | `cli` |
| 时间 / 单位 / 汇率 | `cli,time,unit,fx` |
| 数值线性代数 | `cli,numerical` |
| REST / MCP 服务 | `server`（+ `ratelimit`、`observability`、`docs` 按需） |
| 中英双语错误消息 | `icu` |

完整功能矩阵见 README 的 [🎨 特性标志](../README.md#-特性标志)。

### ❓ 系统要求是什么？

Rust 1.97.1 及以上（MSRV，CI 有对应 MSRV 任务守护）。核心库无额外系统依赖；`server` feature 需 tokio 运行时；源码构建 HTTP 集成测试需要 protoc。

---

## 🛠️ 使用与特性

### ❓ 为什么 `--precision 50 '1/3'` 的结果不是真正的 50 位精度？

`--precision N` 启用 BigRational 模式，但输入表达式先经 f64 解析器，精度已在此处损失。需要真正超越 f64 的精度时，使用 `precision(N, expr)` 函数并保证表达式为纯有理数运算（整数与 `/`、`^` 等），例如 `calnexus 'precision(50, 1/3)'`。

### ❓ 支持隐式乘法吗？

支持。`2x`、`3(x+1)`、`2(x+1)(x+2)` 等数学惯用写法自动识别为乘法。

### ❓ `now` / `today` / `fx` 的结果为什么每次都不同 / 不走缓存？

它们是**非确定性函数**：求值器在路由前检测 AST 是否含这类函数，命中则同时跳过缓存读与写，避免时间/汇率结果被缓存污染。这是有意设计。

### ❓ 汇率数据来自哪里？断网时行为如何？

来自 frankfurter.dev（欧洲央行参考汇率），经三级缓存（内存 → 文件 → 网络）。断网时优先使用本地文件快照（受 `CALNEXUS_FX_TTL_HOURS` 约束，默认 24 小时）；设置 `CALNEXUS_FX_ALLOW_STALE` 后可接受过期快照；连续失败达到 `CALNEXUS_FX_BREAKER_THRESHOLD`（默认 3 次）触发熔断，冷却 `CALNEXUS_FX_BREAKER_COOLDOWN_SECS`（默认 30 秒）。汇率仅供参考。

### ❓ REPL 支持哪些命令？

`:help` 帮助、`:let name = value` 绑定变量、`:vars` 查看变量、`:quit` 退出；Tab 补全基于统一函数目录，feature 门控函数在启用后自动出现。

### ❓ 如何查看当前构建支持哪些函数？

`calnexus --list-functions`（CLI）或 MCP `list_functions` 工具、HTTP `POST /api/v1/list_functions`。feature 门控域仅在启用时出现。

---

## ⚡ 性能

### ❓ CalNexus 有多快？

本地 criterion 基线：简单表达式缓存命中约 1.3 µs，未命中全流水线（解析+规范化+求值+入缓存）约 13 µs，解析器对 `2+3` 约 286 ns。完整数据与口径见 [📈 性能指南](PERFORMANCE.md)。

### ❓ 缓存如何工作？

规范化 AST 的 BLAKE3 哈希为键，moka::sync 直连实现：命中返回 `Arc<EvalResult>` 零拷贝；并发相同键经 `try_get_with` single-flight 只求值一次；字节权重预算默认 64MB（`--cache-size` 可配）；大于 256KB 的结果不入缓存。详见 [📈 性能指南](PERFORMANCE.md)。

### ❓ 批量求值会并行吗？

会。`--batch` 使用 rayon 并行求值各行，输出保持行序，末尾附 summary（总数 / 成功 / 失败 / 缓存命中 / 耗时）。

---

## 🌐 服务模式

### ❓ HTTP 服务如何配置监听地址与限流？

`--bind 0.0.0.0:8080` 或环境变量 `CALNEXUS_BIND_ADDR`；`ratelimit` feature 提供固定窗口 per-IP 限流，`CALNEXUS_RATELIMIT_LIMIT`（默认 120）与 `CALNEXUS_RATELIMIT_WINDOW_SECS`（默认 60）可配，探针与 metrics 豁免。详见 [🖥️ 服务模式指南](SERVER.md)。

### ❓ 错误响应的语义是什么？

三分类：400 `InvalidInput`（语法错误、除零等求值失败）、422 `ValidationError`（请求体参数超限，求值前拦截，HTTP/MCP 形态独有）、503 `ServiceUnavailable`（超时或 fx 上游不可达，带 `Retry-After`）。CLI 退出码：1 = 计算错误、2 = 用法错误、3 = 超时/不可用。

### ❓ 如何把 CalNexus 接入 Claude Desktop / Cursor？

以 `--serve-mcp` 启动 stdio MCP 服务，在客户端配置 `mcpServers` 即可。客户端配置示例与工具清单见 [🖥️ 服务模式指南](SERVER.md)。

---

## 🔧 故障排查

### ❓ wasm32 为什么构建失败？

`server` feature 引入的 `tokio::rt` 不支持 wasm32；缓存层已重构为 moka::sync 直连（v0.1.5 移除 oxcache→tokio 链），CLI-only 形态理论上可编译但未验证。在移除 tokio 依赖的构建组合评估完成前，wasm32 构建**不受 CI 门禁保护、不承诺可用**。

### ❓ 求值报 Timeout（退出码 3）？

表达式触发超时（`--timeout` / `CALNEXUS_TIMEOUT`，秒）。检查表达式是否含超大幂/阶乘/precision 参数；已知 DoS 向量（factorial/pow/precision）有专门常量约束，正常表达式不应超时。

### ❓ fx 函数报 503 / DependencyUnavailable？

fx 上游不可达且本地无可用快照（或已过期且未开 `CALNEXUS_FX_ALLOW_STALE`）。检查网络；熔断触发后需等冷却期结束。

### ❓ 表达式明明合法却报 Parse 错误？

注意：连续运算符（`++`、`**` 等）被显式拒绝；字符串参数需双引号包裹（`convert(1000, "m", "km")`）；括号不平衡或超出深度上限（256 层）也会报错。对照 `--explain` 输出中的 caret 位置指示排查。

---

## 📚 相关文档

- [📖 用户指南](USER_GUIDE.md) — 完整使用教程
- [📈 性能指南](PERFORMANCE.md) — 基准数据与优化建议
- [🖥️ 服务模式指南](SERVER.md) — 服务模式部署与 MCP 接入
- [🔒 安全文档](SECURITY.md) — 安全设计与漏洞报告
- [🤝 贡献指南](CONTRIBUTING.md) — 参与开发
