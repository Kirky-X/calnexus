# Changelog

All notable changes to CalNexus are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.2] - 2026-07-21

### Fixed

- **SKILL.md 前移损坏修复**: `description` 字段内容合并到单行的损坏已修复，恢复正确 YAML 前移结构
- **文档-代码一致性修复**: 根据发布前审查修复 20 项文档与代码不一致（CLI 标志缺失/示例输出过期/版本号陈旧/Domain 表格错误/CHANGELOG 重复/ARCHITECTURE 误导性陈述）

**4 subagent 穷举分析 + TDD 修复（Core/Domains/Output/Server+i18n）**：

**Core 模块**（7 个 bugs）：
- BUG-C-M-001/002/006: `cache.rs` 3 处 `.ok().flatten()` / `let _ =` / `unwrap_or(0)` 静默吞错改为显式 `match` + `eprintln!` 降级为 cache miss（规则 12）
- BUG-C-M-003: `canonicalizer.rs` NaN 比较从 `partial_cmp + unwrap_or(Equal)` 改为 `f64::total_cmp`，提供 IEEE 754 totalOrder 全序
- BUG-C-M-004: `canonicalizer.rs` 显式处理 `0^0 = 1.0`，与所有域保持一致
- BUG-C-M-005: `parser.rs` `validate_no_consecutive_plus` 扩展为 `validate_no_consecutive_operators`，检查 `++`/`**`/`//`/`^^` 四种运算符
- BUG-C-L-008: `domain.rs` `format!("{:?}", functions)` 输出限制为 5 个 + `... and N more` 提示，防止超长 Debug 输出

**Domains 模块**（9 个 MEDIUM + 多个 LOW）：
- BUG-D-M-001/002/003: `vector.rs` / `complex.rs` / `polynomial.rs` 标量 Pow 添加 `0^0=1.0` + `is_finite` 检查
- BUG-D-M-004/005: `polynomial.rs` `coeffs_from_pow` NaN 检查 + `poly_eval_horner` 签名改为 `Result<f64, CalcError>`
- BUG-D-M-006: `symbolic.rs` `taylor()` 从 `unwrap_or(0.0)` 改为 `?` 传播错误
- BUG-D-M-007/008: `symbolic.rs` `simplify_pow` / `eval_symbolic` 添加 `is_finite` 检查，提取 `check_finite()` 辅助函数
- BUG-D-M-009: `complex.rs` `arg(0+0i)` 从静默返回 0.0 改为 `Err(DomainError)` + i18n 键 `msg.complex.arg_zero_undefined`
- BUG-D-L-001/002: `vector.rs` / `precision.rs` `evaluate()` 顶部预绑定 pi/e（参考 polynomial 模式）
- LOW: `vector.rs` / `complex.rs` / `precision.rs` 24 处硬编码错误消息 i18n 化

**Output 模块**（12 个 MEDIUM）：
- BUG-O-M-001/002/003: `steps.rs` `format_value` 浮点噪声检测 + 大数科学计数法 + `-0.0` 保留负号
- BUG-O-M-004: `steps.rs` `walk` Matrix 分支从 `Ok(0.0)` 改为 `Err(DomainError)`
- BUG-O-M-005: `steps.rs` BigNumber 分支检查 f64 安全整数范围（2^53），超出报错
- BUG-O-M-006: `steps.rs` gcd/lcm 添加 `check_integer_arg` 验证整数性和 i64 范围
- BUG-O-M-007/008: `latex.rs` `format_latex_scalar` 浮点精度噪声检测 + 大整数科学计数法
- BUG-O-M-009/010: `latex.rs` `format_latex_complex` 零虚部/零实部简化输出
- BUG-O-M-011: `latex.rs` `join_latex_polynomial_terms` 负项用 ` - ` 分隔
- BUG-O-M-012: `latex.rs` `symbolic_str_to_latex` 状态机解析 `^N` 转换为 `^{N}`

**Server/i18n/入口模块**（4 个 MEDIUM + 1 LOW）：
- BUG-S-M-001: `server/evaluate.rs` 新增 `REQUEST_TIMEOUT_SECS=30` + `evaluate_with_timeout` 可测试入口，`tokio::time::timeout` 包裹 `spawn_blocking`
- BUG-S-M-002: `server/types.rs` `validate()` 新增 vars 键名长度（≤64）和 null 字节校验
- BUG-S-M-003: `server/http.rs` 新增 `shutdown_signal` 监听 SIGINT/SIGTERM，`with_graceful_shutdown` 优雅关闭
- BUG-I-M-001: `i18n.rs` `parse_lang_simple` 改为 BCP-47 标准分割子标签，支持 `zh-Hans`/`zh-Hant`/`zh-Hans-CN` 等变体
- BUG-E-L-001: `main.rs` 新增 `setup_panic_hook` 安装 panic hook，打印用户友好前缀 + 保留默认 backtrace

**审查 subagent 发现的回归问题**（3 个，已修复）：
- H-1（架构 HIGH）: `cache.rs` 3 处 `expect()` 改为 log+降级（`eprintln!` + 返回 `None`/`0`），保留 API 兼容性
- M-1（架构 MEDIUM）: `complex.rs` 复数 Pow 路径补 `0^0=1.0` + `is_finite` 检查，与标量分支一致
- M2（性能 MEDIUM）: `vector.rs` / `precision.rs` 条件性 clone EvalContext，跳过无 pi/e 缺失时的 clone

### Changed

- **SKILL.md 移至 `skill/SKILL.md`**: 根目录 SKILL.md 移至 `skill/SKILL.md` 作为项目使用指南，`.claude/skills/calnexus-dev/SKILL.md` 精简为轻量 skill 参考
- **i18n 键扩展**: en.json / zh.json 从 195 键扩展到 248 键（新增 53 键，删除 1 死键 `msg.output.eval_error`，修改 1 键 `msg.core.parse_illegal_consecutive_ops` 添加 `{op}` 占位符），en/zh 完全对等
- **Cargo.toml**: `http` feature 新增 `tokio/signal` 依赖，支持 graceful shutdown 信号监听

### Security

- 3 个独立审查 subagent（安全/架构/性能）按规则 26 完成 commit 前审查
- **tiangang SAST 扫描**: 0 CRITICAL/HIGH（Gitleaks 0 泄漏 / Trivy 0 漏洞 / cargo-audit 0 通报 / Trufflehog 0 验证密钥）
- HTTP server 请求级超时（30s）+ vars 键名校验 + 优雅关闭三重防护
- 缓存层错误降级为 cache miss，避免 panic 影响 HTTP 请求处理

### Testing

- 全 feature 矩阵测试通过：`cli` 1856 测试通过 / 0 失败 / 3 ignored
- `cargo clippy --features cli --all-targets -- -D warnings` 0 警告
- 3 维度 subagent 审查（安全 / 架构 / 性能）通过，无 CRITICAL / HIGH 阻断项
- 新增 38 个回归测试覆盖所有修复的 bugs

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

[unreleased]: https://github.com/kirky-x/calnexus/compare/v0.1.2...HEAD
[0.1.2]: https://github.com/kirky-x/calnexus/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/kirky-x/calnexus/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/kirky-x/calnexus/releases/tag/v0.1.0
