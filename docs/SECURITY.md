# 🔒 CalNexus 安全文档

> 本文档描述 CalNexus 的支持版本、漏洞报告流程、安全设计概览与使用最佳实践。

## 📋 目录

- [📌 支持版本](#-支持版本)
  - [最低支持 Rust 版本（MSRV）](#最低支持-rust-版本msrv)
  - [依赖安全](#依赖安全)
- [🐛 漏洞报告流程](#-漏洞报告流程)
  - [如何报告](#如何报告)
  - [报告内容](#报告内容)
  - [我们的承诺](#我们的承诺)
  - [漏洞披露时间线](#漏洞披露时间线)
- [🛡️ 安全设计概览](#️-安全设计概览)
  - [攻击面](#攻击面)
  - [资源与深度防护](#资源与深度防护)
  - [错误信息脱敏](#错误信息脱敏)
  - [网络上游加固（fx）](#网络上游加固fx)
  - [错误语义分级](#错误语义分级)
- [✅ 安全最佳实践](#-安全最佳实践)
  - [面向库使用者](#面向库使用者)
  - [面向服务端部署](#面向服务端部署)
- [📚 相关文档](#-相关文档)

---

## 📌 支持版本

CalNexus 目前处于 0.x 阶段，仅对**最新次版本线**提供安全修复：

| 版本 | 支持状态 |
|------|----------|
| 0.x（最新次版本） | ✅ 支持安全更新 |
| 更早的 0.x 版本 | ❌ 不支持 |

报告漏洞前请尽可能升级到最近发布版本（见 [Releases](https://github.com/kirky-x/calnexus/releases)）。

### 最低支持 Rust 版本（MSRV）

MSRV 为 **Rust 1.97.1**（与仓库 `Cargo.toml` 的 `rust-version` 及 CI MSRV 任务三处对齐）。旧工具链不再获得安全修复，请保持工具链更新。

### 依赖安全

```bash
# 运行安全审计（RustSec 公告扫描）
cargo audit

# 许可证 / 禁用依赖 / 来源校验
cargo deny check
```

- 全部依赖禁用默认特性、按需最小化启用，缩小传递依赖面。
- [周度定时工作流](https://github.com/kirky-x/calnexus/actions/workflows/audit.yml)自动执行 `cargo audit` + `cargo deny`，dependabot 跟踪依赖更新。
- [CodeQL 静态安全分析](https://github.com/kirky-x/calnexus/actions/workflows/codeql.yml)（push/PR 触发 + 周日 UTC 0 点周度定时）。
- 发布流水线内置发布前 `cargo audit --deny warnings` 门禁与版本一致性校验。
- 第三方 GitHub Actions 全部 commit SHA 钉扎。

---

## 🐛 漏洞报告流程

### 如何报告

**请勿通过公开 GitHub Issue 报告安全漏洞。**

请发送邮件至 **[security@calnexus.dev](mailto:security@calnexus.dev)** 私密披露。我们遵循负责任披露（responsible disclosure）模型，也请求报告者同样遵守：修复发布并经您确认前，请勿公开披露问题。

### 报告内容

理想的报告应包含：

- 漏洞的清晰描述与潜在影响
- 逐步复现说明，含触发问题的**确切表达式或输入**
- 测试所用的 CalNexus 版本与平台
- 修复建议（如有）

### 我们的承诺

- **48 小时内**：确认收到报告
- **7 天内**：给出初步评估（是否确认、严重程度）
- **视严重程度尽快**：准备并发布修复，同步进展
- **修复发布后**：与您协调公开披露，并致谢您的贡献（可匿名）

### 漏洞披露时间线

1. 报告送达 → 48h 内确认
2. 评估与定级 → 7 天内初步结论
3. 修复开发与验证（含回归测试 + fuzz 目标覆盖）
4. 版本发布 → 协调公开披露与致谢

---

## 🛡️ 安全设计概览

### 攻击面

CalNexus 是**命令行数学表达式求值器**：无网络功能（`fx` 汇率上游除外）、无不可信路径文件 I/O、无外部插件加载，实际攻击面极小。安全评审重点关注：

- **表达式注入 / 不可信输入**：构造恶意表达式导致 panic、死循环或资源耗尽
- **深度溢出 / 递归限制**：深层嵌套表达式击穿解析器/求值器栈或耗尽内存；任何绕过深度限制的手段都是安全级缺陷
- **数值边界**：触发溢出、NaN 传播等具有安全影响的数值异常

超出上述类别但具有安全影响的发现，同样欢迎报告——宁可误报，不可漏报。

### 资源与深度防护

- **递归深度防护闭环**：`MAX_AST_DEPTH=256`、mathexpr 递归前迭代括号预检、括号字面量 thread-local RAII 守卫、canonicalizer 深度守卫。历史 DoS 缺口（嵌套列表/矩阵字面量约 1300 层绕过深度检查）已修复并新增 `list_depth_fuzz` 等 fuzz 目标守护。
- **请求资源上限**：expr ≤ 4096 字符、vars ≤ 1024 键、precision ≤ 10000 位；已知 DoS 向量（factorial/pow/precision）有专门常量约束；`--timeout` 兜底累积慢操作。
- **模糊测试**：7 个 cargo-fuzz 目标持续覆盖 parser、ast_depth、list_depth、canonicalizer、cache_key、numeric_boundary、matrix_dim。
- **安全测试**：`tests/security_tests.rs` 覆盖 DoS 向量与边界攻击。

### 错误信息脱敏

解析错误不泄漏 winnow 内部 Debug 结构；错误文案经 i18n 层清洗后输出，`--explain` 与 JSON 错误对象均只暴露 `kind` / `message` / `hint` / `span` 等契约字段。

### 网络上游加固（fx）

fx 汇率域是唯一的网络面，加固措施：ureq `https_only`（禁明文）、响应体 1MB 上限、磁盘缓存原子写（temp + rename + 0600 权限）、拉取 single-flight、三态同步熔断器（连续失败阈值 / 冷却期，`CALNEXUS_FX_BREAKER_*` 可配）。

### 错误语义分级

`ErrorKind::DependencyUnavailable`（exit_code=3）与客户端输入错误严格区分：服务化部署下分别映射 503（带 `Retry-After`）与 400，避免上游故障被误判为客户端问题、污染调用方重试与告警策略。

---

## ✅ 安全最佳实践

### 面向库使用者

- 处理**不可信输入**时始终启用 `--timeout` / `CALNEXUS_TIMEOUT`，并让深度/资源上限保持默认值（上限是安全防线，不是可随意放松的调优项）。
- 优先通过 `EvalContext` / `--var` 注入变量，而不是字符串拼接表达式。
- 关注 RustSec 公告，保持依赖与工具链更新（`cargo audit` 可集成进 CI）。

### 面向服务端部署

- 服务监听地址默认 `127.0.0.1:3000`；对外暴露时置于反向代理 / Ingress 之后，再绑定 `0.0.0.0`。
- 为容器设置内存预算并相应配置 `CALNEXUS_CACHE_SIZE`；K8s `terminationGracePeriodSeconds` ≥ 45（drain 30s + 余量）。
- 启用 `ratelimit` feature 限流（探针与 metrics 自动豁免），`/metrics` 接入监控，关注 503 与 `Retry-After` 信号区分上游故障。
- 详见 [🖥️ 服务模式指南](SERVER.md)。

---

## 📚 相关文档

- [🏗️ 架构文档](ARCHITECTURE.md) — 防护机制的代码级细节
- [🖥️ 服务模式指南](SERVER.md) — 服务化安全部署
- [📋 更新日志](CHANGELOG.md) — 安全修复记录
