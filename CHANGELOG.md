# Changelog

All notable changes to CalNexus are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.1] - 2026-07-21

### Added

- **ICU 国际化基础设施**: 新增 `src/i18n.rs` 模块，基于 ICU4X 2.2 实现 BCP-47 locale 解析与消息本地化
  - `I18n` struct + `tf(key, args)` 方法支持 `{name}` 占位符替换
  - `Lang` 枚举（`En` / `Zh`）+ `from_locale_str()` BCP-47 解析
  - 编译时 `include_str!` 嵌入 + `OnceLock<HashMap>` 运行时零拷贝解析
- **本地化资源文件**: 新增 `locales/en.json` 与 `locales/zh.json`（各 195 键，完全同步）
  - 涵盖错误消息（`error.*` / `detail.*` / `msg.*`）、CLI 文本（`cli.*`）、REPL 文本（`repl.*`）、批处理文本（`batch.*`）
- **CalcError i18n 字段**: `ErrorKind` + `message` 之外新增 `i18n_key: Option<&'static str>` + `i18n_args: Vec<(String, String)>`，支持 `with_i18n()` 链式调用
- **SKILL.md**: 项目技能文档（192 行），含源码安装与 `cargo install` 双路径

### Changed

- **CLI 模块重构**: 通过 `sdforge::clap` 重导出间接使用 clap，移除直接 `clap` 依赖，统一 CLI 封装层
- **依赖版本对齐**: 所有依赖统一为 `x.x`（Major.Minor）格式，禁止 patch 段写法
- **代码复杂度优化**: 提取 10 个热路径函数的辅助方法，降低圈复杂度（cyc=25 → ≤15）
- **`friendly()` / `to_explain()` 走 i18n**: 优先用 `i18n_key` 查询本地化消息，回退到英文 `message`
- **`ReplSession` 持有 `I18n`**: REPL 会话独立持有 i18n 实例，支持运行时切换语言
- **`batch::run()` 接收 `I18n` 参数**: 批处理模式支持本地化输出

### Fixed

- **`--steps` 模式下 `sin(pi/2)` 计算错误**: `output::steps::walk()` 的 `Variable(name)` 分支用 `ctx.get_var(name).unwrap_or(0.0)`，导致 `pi`/`e` 数学常量被错误求值为 0.0（用户报告：`calnexus --steps 'sin(pi/2)'` 输出 `sin(0)=0`）。修复：在 Variable 分支识别 `pi`/`e` 常量，用户绑定的变量优先（与 scientific/statistics/matrix domain 预绑定 pi/e 一致）
- **`release.yml` tar 打包失败**: `tar czf .` 报 "file changed as we read it"，改用 `git archive` 基于 git 索引打包，避免文件系统变化干扰

### Removed

- **死键 `msg.core.eval_domain_error`**: src/ 下 0 命中，删除避免误导
- **重复键 `msg.arithmetic.factorial_negative` / `msg.scientific.factorial_negative` / `msg.output.factorial_negative`**: 统一为 `msg.core.factorial_negative`，消除 DRY 违规

### Security

- **`validate_inputs` precision 上界**: 在 `evaluate` 公共入口拦截超大 precision 防止 `format_decimal` 循环 DoS
- **`parse_matrix_literal` Debug 输出限制**: `format!("{:?}", row_node)` 限制 100 字符，防止大型 AST 递归 Debug 性能问题
- **DP-1 / DP-4 契约保持**: `Display` impl 与 `to_json()` 始终输出英文 `message`，机器可读契约不受 i18n 影响

### Architecture

- **DP-1**: `error_kind_prefix` 保留英文（机器可解析诊断契约）
- **DP-2**: `CalcError` 新增 `i18n_key` + `i18n_args` 字段（最小侵入式改造）
- **DP-3**: 数学公式语法保留（跨语言通用）
- **DP-4**: JSON 输出键名保留英文（机器契约）
- **DP-5**: Server 错误响应走 i18n

### Testing

- 全 feature 矩阵测试通过：`default` 1437 / `cli` 1791 / `cli,icu` 1791 / `cli,server` 1847 / `cli,server,icu` 1847
- `cargo clippy --features cli --all-targets` 零警告
- 3 维度 subagent 审查（安全 / 架构 / 性能）通过，无 CRITICAL / HIGH
- 新增 5 个回归测试覆盖 `Variable("pi")`/`Variable("e")` 在 steps 模式下的求值

## [0.1.0] - 2026-07-15

### Added

- 初始发布：11 个计算域（arithmetic / statistics / numerical / scientific / matrix / symbolic / polynomial / number_theory / combinatorics / precision / domain router）
- 命令行接口（`--expr` / `--var` / `--precision` / `--steps` / `--json` / `--domain`）
- REPL 交互模式（`:let` / `:vars` / `:clear` / `:help` / `:quit`）
- 批处理模式（stdin 读取 + `--count` / `--max-line` 限制）
- HTTP API（`/evaluate` 端点，axum 框架）
- MCP 协议接口（Model Context Protocol）
- DoS 防护：表达式长度限制（4096）、AST 深度限制（256）、precision 上界（MAX_PRECISION）、pow 输出位限制、factorial 输入上界
- 缓存层（`CacheManager` + `CanonicalForm` 规范化键）
- 超时机制（`EvalContext.timeout` + 关键节点 `check_elapsed`）
