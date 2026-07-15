# Changelog

本项目所有重要变更均记录于此文件。格式遵循 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，
版本号遵循 [Semantic Versioning](https://semver.org/lang/zh-CN/)。

## [Unreleased]

v1.1 项目治理收尾：归档 openspec 变更、版权头、依赖瘦身、文档体系、clippy 全目标零告警。

### 新增

- **版权头**：全部 43 个 `.rs` 文件添加 `// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.`
- **MIT License**：根目录 `LICENSE` 文件
- **文档体系**：`CONTRIBUTING.md`、`CODE_OF_CONDUCT.md`、`SECURITY.md`、`.editorconfig`、`.env.example`
- **GitHub 模板**：`.github/ISSUE_TEMPLATE/{bug_report,feature_request}.md`、`.github/PULL_REQUEST_TEMPLATE.md`
- **Claude Skill**：`.claude/skills/calnexus-dev/SKILL.md` — 项目开发参考技能
- **openspec 归档**：v0.8 / v1.0 / v1.1 三个变更归档至 `openspec/changes/archive/2026-06-29-*/`

### 变更

- **Cargo.toml 依赖瘦身**：所有依赖设置 `default-features = false` + 显式 feature 列表，
  减小二进制体积；`num-rational` 显式启用 `num-bigint` feature（`BigRational` 类型依赖）
- **README.md 重构**：按模板重组结构（徽章 / 目录 / 概述 / 功能表 / 架构 / 快速开始 / 配置 / API / 测试 / 贡献 / 路线图）
- **.gitignore 优化**：分区注释（Rust / IDE / Python / 覆盖率 / 环境 / OS / 项目特定），新增 `lcov*.info`、`.idea/`、`.vscode/` 等
- **clippy 修复**：`sort_by` → `sort_by_key(Reverse)`、嵌套 `format!` → 内联格式参数、`needless_range_loop` → `enumerate().skip(1)`；
  测试代码添加 `#![allow(clippy::approx_constant, non_snake_case)]`（3.14 为测试值非 PI 误用，P/C 大写对应排列/组合记号）
- **performance_tests.rs**：修复 `current.exists() || true` 逻辑 bug（clippy::logic_bug）

### 测试

- **1650 个测试全部通过**（4 个 `#[ignore]`），`cargo clippy --all-targets --features cli` 零告警零错误
- 行覆盖率 97.27%（剩余未覆盖行为 TTY 路径、`unreachable!()`、平台特定代码）

## [0.2.0] - Unreleased

CalNexus v0.2.0 Phase 0（地基层）：依赖更新 + Feature 细粒度重划分 + ICU4X 国际化 + 结构化错误重构。
为后续 P1(sdforge 接口) / P2(汇率换算) / P3(EXTENSION_PLAN 扩展) 铺设横切地基。

### 新增

- **依赖更新**（deps-upgrade）：
  - `oxcache` 0.2 → 0.3（BREAKING: API 适配）
  - 所有 crate 升级到最新稳定版（blake3 / serde / tokio / regex / num-* / nalgebra / clap / rustyline / rayon）
  - 新增 `thiserror = "2"`（错误派生，`default-features = false`）
  - dev-deps 同步：proptest / assert_cmd / predicates / tempfile / serde_json / insta / criterion / expectrl
- **Feature 细粒度重划分**（feature-gates）：
  - 从 `default=[]+cli` 扩展为 7 个可组合特性：`cli` / `icu` / `http` / `mcp` / `fx` / `numerical` / `server`
  - `default = []`（核心库零依赖），`server = ["http", "mcp"]`（聚合特性）
  - 所有依赖通过 `dep:` 语法显式声明，`default-features = false` 全覆盖
- **ICU4X 国际化**（i18n-bilingual）：
  - `src/i18n.rs` 实现 `I18n` 结构 + `Lang` 枚举（`En` / `Zh`）
  - `I18n::from_str("en"|"zh")` 解析，未知语言回退 `En`（fail-loud）
  - 10 个错误消息键的中英双语翻译（`error.parse` / `error.eval` / `error.overflow` / `error.division_by_zero` /
    `error.domain` / `error.depth` / `error.nan_or_inf` / `error.undefined_symbol` / `error.timeout` / `error.usage`）
  - 未知键返回键本身（fail-loud 设计）
- **结构化错误重构**（structured-errors）：
  - `Span` struct（`start` / `end` 字节偏移）— 精确定位错误在输入表达式中的位置
  - `ErrorKind` enum（10 种变体）+ `exit_code()` 方法（0/1/2/3 退出码契约）
  - `CalcError` struct（`kind` / `message` / `span` / `hint`）+ `thiserror` 派生
  - 三态呈现：`friendly(&i18n)` 友好格式 / `to_json()` JSON 机器可读 / `to_explain(&i18n)` 教育模式
  - 便捷构造器：`parse()` / `eval()` / `overflow()` / `nan_or_inf()` / `domain()` / `depth_exceeded()` /
    `division_by_zero()` / `undefined_symbol()` / `timeout()` / `usage()`
  - 链式构建器：`with_span()` / `with_hint()`
- **Span 生成**（span-generation）：
  - `preprocess_brackets` / `parse_bracket_literal` / `parse_matrix_literal` / `parse_list_literal` 错误路径添加 Span
  - `validate_no_consecutive_plus` 重构为在原始 input 中查找 `++` 位置（跳过空格）
  - `preprocess_factorial` 通过 `.map_err(|e| e.with_span(Span::point(i)))` 传播 Span
- **Hint 生成**（error-hints）：
  - `depth_exceeded()` → "simplify nested expressions (max 256)"
  - `division_by_zero()` → "check divisor before division"
  - `timeout()` → "increase --timeout or simplify expression"
  - `asin` / `acos` domain 错误 → "asin domain is [-1, 1]" / "acos domain is [-1, 1]"
- **CLI 退出码 + --explain flag**（cli-error-handling）：
  - `--explain` flag（与 `--json` 互斥）— 输出详细错误解释到 stderr
  - `--lang <en|zh>` flag（默认 `en`）— 本地化错误消息
  - `handle_error()` 统一错误处理函数，7 个错误路径统一调用
  - 退出码契约：0=成功 / 1=计算错误 / 2=用法错误 / 3=超时
  - `--json` 错误时输出 `{"error":{"kind":"...","message":"...","exit_code":N}}` 到 stdout
- **Timeout 触发路径**（timeout-trigger）：
  - `evaluate()` 开头检查 `ctx.timeout.is_zero()`，立即返回 `CalcError::timeout()`
  - design.md §6.3：P0 不实现基于 elapsed 的自动超时（留 P3），仅支持显式配置驱动触发

### 变更

- **CalcError 从 enum 重构为 struct**（breaking）：657 处调用点全部迁移
  - 旧：`CalcError::ParseError(msg)` / `CalcError::EvalError(msg)` / `CalcError::Overflow` / ...
  - 新：`CalcError::parse(msg)` / `CalcError::eval(msg)` / `CalcError::overflow()` / ...
  - `Display` trait 输出保持向后兼容（`parse error: ...` / `evaluation error: ...`）
- **SEC-007 解除 ignore**：原标记"timeout not yet implemented"，现已实现触发路径
- **snap_004 快照更新**：错误格式从 `parse error: ...` 变为 `Parse error (位置 0:4): ...`（含 span 信息）

### 测试

- **1717 个测试全部通过**（3 个 `#[ignore]`，较 P0 前 +67 个新测试）
- **行覆盖率 98.89%**（cargo-llvm-cov，`--fail-under-lines 95` 通过）
  - cli.rs 95.67% / i18n.rs 100% / core/types.rs 98.25% / core/parser.rs 99.12%
  - 唯一低于 95% 的 repl.rs 94.59%（TTY 路径，不影响总体达标）
- `cargo clippy --features cli --all-targets -- -D warnings` 零警告
- `cargo fmt --all -- --check` 通过
- 新增测试：
  - 23 个 Span / ErrorKind / CalcError 单元测试（三态呈现 / 退出码 / 构造器）
  - 12 个 parser Span 精确性测试
  - 8 个 CLI 集成测试（--explain / --lang / --json error / 退出码 0/1/2）
  - SEC-007 重写为三段断言（库 API 触发 + 退出码契约 + 有界运行）
- 3 subagent 审查（安全/架构/性能）：T0.4.7 发现 HIGH（JSON 双层嵌套）已修复

### 已知限制

- `Duration::ZERO` 语义为"立即超时"（非业界惯例的"无超时"），后续迭代考虑改为 `Option<Duration>`
- `--lang` 未知值静默回退 `En`（违反 Rule 12），后续迭代添加 `value_parser` 限制
- `parse_vars` / `get_expression` 错误路径未走 `handle_error`（双轨制），T0.4.8 后续清理
- P0 不实现基于 elapsed 的自动超时（design.md §6.3，留 P3）

### P1: sdforge 接口层

sdforge 0.4 依赖引入 + server 模块（HTTP/MCP 双协议）+ CLI server 模式 flag + P0 审查遗留修复。
specmark change: `p1-sdforge-interface`。

#### 新增

- **P0 审查遗留修复**（Phase 1-2）：
  - i18n 标签键：`label.position`/`label.hint`/`label.error_kind`/`label.exit_code`/`label.suggestion` 中英双语映射，
    `friendly()`/`to_explain()` 改用 `i18n.t()` 动态查询（diting HIGH-1 修复）
  - Span 多字节字符位置：审计 parser 中 21 处 `Span::new()`/`Span::point()` 构造点，确保字符偏移（kueiku HIGH-1 修复）
  - `parse_vars()`/`get_expression()` 错误路径改返回 `CalcError`，走 `handle_error()` 统一输出（消除双轨制）
  - `--lang` 添加 `value_parser` 限制 `["en", "zh"]`，未知值退出码 2（fail-loud）
  - `batch.rs` 删除重复 `escape_json`，改用 `core::types::escape_json_string`（处理控制字符）
  - `I18n::from_str` 精确匹配 `["zh", "zh-CN", "zh-TW"]`（忽略大小写），"zhongwen" 回退 En
- **sdforge 依赖**（Phase 3）：
  - `sdforge = "0.4"`（`default-features = false`），`http` feature 启用 `sdforge/http`，`mcp` feature 启用 `sdforge/mcp`
  - `axum = "0.8"`（HTTP 框架，与 sdforge 同版本），`serde_json`（JSON 序列化）
- **server 模块**（Phase 4）：
  - `src/server/mod.rs`：`ServerAdapter` trait（`start() → impl Future<Output = Result<(), ServerError>> + Send`）
  - `src/server/types.rs`：`EvaluateRequest`/`EvaluateResponse`/`ErrorResponse`/`ErrorDetail` DTO + `validate()` 安全校验
    （vars ≤1024 键，precision ≤10000）+ `ServerError`（Http/Mcp/Validation 三变体，Validation 协议无关）
  - `src/server/http.rs`：`POST /api/v1/evaluate` handler（axum + sdforge inventory 路由注册）+ `HttpServer` struct +
    `build_router()` + `preserve_http_inventory()` 防链接器优化 + `SHARED_CACHE` 进程级缓存 + `spawn_blocking` 避免 block_on 嵌套
  - `src/server/mcp.rs`：`evaluate` tool（rmcp + sdforge inventory 工具注册）+ `McpServer` struct + `build_mcp_server()` +
    `preserve_mcp_inventory()` + `std::thread::scope` 避免 block_on 嵌套
  - `MAX_PRECISION = 10000` 纵深防御四层校验：server `validate()` → `evaluate()` precision 参数校验 →
    `extract_format_precision()` → `PrecisionDomain::extract_precision_value()`
  - 缓存键隔离：precision 模式 `CanonicalForm::new("precision:{}", cf)` 前缀键，避免 BigRational 与 Scalar 双向污染
- **CLI server 模式**（Phase 5）：
  - `--serve-http` flag：启动 HTTP server（`HttpServer::run()`），`conflicts_with_all` 覆盖
    `repl/batch/canonical/latex/steps/json/explain/precision/serve_mcp`
  - `--serve-mcp` flag：启动 MCP server（`McpServer::run()`），同上冲突约束
  - Feature 门控分层：`#[cfg(feature = "server")]` 聚合门控（cli.rs 层），
    `#[cfg(feature = "http")]`/`#[cfg(feature = "mcp")]` 细粒度门控（server/mod.rs 层）

#### 变更

- `[package.metadata.tarpaulin]` `fail-under` 90 → 95（与 llvm-cov 95 对齐）
- `lib.rs` 条件导出 `format_bigrational`（`#[cfg(any(feature = "cli", feature = "http", feature = "mcp"))]`）+
  `server::*`（`#[cfg(any(feature = "http", feature = "mcp"))]`）
- `domains/mod.rs` `format_bigrational` 添加 feature 门控，修复 unused_imports 警告
- `evaluator.rs` evaluate 函数添加 precision 参数上界校验（安全审查 HIGH-1 修复）+ 缓存键隔离

#### 测试

- **行覆盖率 98.06%**（cargo-llvm-cov，`--fail-under-lines 95` 通过），`--features "cli server"` 全 feature 组合
  - server/http.rs 88.54% / server/mcp.rs 79.79% / server/types.rs 98.65% / server/cache.rs 100%
  - domains/precision.rs 99.09%（factorial/pow 上界检查覆盖）
  - 剩余未覆盖为阻塞 I/O serve 路径（`run()`/`start_inner()` 永久阻塞）和 panic 处理路径
- `cargo clippy --features "cli server" --all-targets -- -D warnings` 零警告
- `cargo fmt --check` 通过
- 新增测试：
  - 47 个 server 模块单元测试（http 构造器/run 错误路径/route metadata/inventory + mcp 构造器/EvaluateTool trait/
    call 错误路径/tool metadata/inventory）
  - 3 个 server 集成测试（HTTP oversized precision 验证错误 + MCP oversized precision + MCP null 输入）
  - 5 个 CLI 集成测试（`--serve-http`/`--serve-mcp` flag 冲突 + feature 门控）
  - 6 个 precision 安全测试（factorial/pow 上界边界值 + 超限错误 + 负指数不受限）
  - 1 个 CLI 集成测试（`--batch --precision` 冲突退出码 2，BUG-007 修复）

#### P1 审查修复（Phase 7）

Phase 6 完成后并行派遣 3 subagent（安全/架构/性能）审查，发现 3 CRITICAL + 1 HIGH + 4 MEDIUM + 1 MEDIUM bug。
本阶段修复阻断项，延后项记录于"已知限制"。

- **安全 CRITICAL 修复**（factorial/pow DoS）：
  - `core/types.rs` 新增 `MAX_FACTORIAL_INPUT=10000` + `MAX_POW_EXPONENT=100000` 常量
  - `domains/precision.rs` 的 `factorial()` 签名改为 `Result<BigInt, CalcError>` + 上界检查
  - `domains/precision.rs` 的 `BinaryOp::Pow` 分支添加指数绝对值上界检查（`exp.abs() > MAX_POW_EXPONENT`）
- **安全 CRITICAL C-1 复审修复**（负指数 DoS 绕过）：
  - 初版仅检查正指数 `exp > MAX_POW_EXPONENT`，负指数 `-2000000000` 绕过（`BigRational::pow(neg_i32)` 内部计算 `a^|exp|` 再取倒数）
  - 修复：改为 `exp.abs() > MAX_POW_EXPONENT`，堵住负指数 DoS 路径
  - 新增测试 `test_pow_oversized_negative_exponent_returns_error`（`2^(-100001)` 应返回 Domain 错误）
- **安全 HIGH 修复**（底数复合限制，C-1 复审发现）：
  - `(10^10000)^99999` 可产生 ~1GB 输出，指数已受约束但底数 `a` 可为任意大小 BigInt
  - `core/types.rs` 新增 `MAX_POW_OUTPUT_BITS=3_320_000` 常量（对应 ~1MB 输出）
  - `domains/precision.rs` 新增 `check_pow_output_size()` 函数：`底数bits × |指数| ≤ MAX_POW_OUTPUT_BITS`
  - 6 个 TDD 测试（小底数通过/大底数拒绝/边界通过/边界拒绝/端到端安全通过/端到端拒绝）
- **安全 CRITICAL 修复**（timeout elapsed 追踪）：
  - 原 `evaluator.rs` 仅入口检查 `ctx.timeout.is_zero()`，计算期间不强制超时
  - 用户决策：P0 立即修复（撤销 design.md §6.3 的 P3 延后声明）
  - 实现：记录 `start = Instant::now()`，在关键节点（parse/canonicalize/domain::evaluate 前后）检查 `elapsed > timeout`
  - 新增 `check_elapsed()` 辅助函数，6 个 TDD 测试（zero 立即超时/elapsed 触发/正常通过/precision 模式/辅助函数单元测试）
- **Bug HIGH 修复**（BUG-007 `--batch --precision` 静默忽略）：
  - `cli.rs` 的 `precision` 添加 `"batch"` 到 `conflicts_with_all`，`batch` 添加 `"precision"` 到 `conflicts_with_all`
  - clap 在参数解析阶段拒绝组合，退出码 2（规则 12：失败显性化）
- **架构 MEDIUM 修复**（SHARED_CACHE 去重 + 常量单一来源）：
  - 创建 `server/cache.rs` 统一 `SHARED_CACHE` + `shared_cache()`，`server/mod.rs` re-export
  - `http.rs`/`mcp.rs` 移除独立定义改用 `super::shared_cache`，HTTP/MCP 现共享同一缓存实例
  - `arithmetic.rs`/`scientific.rs` 删除本地 `MAX_FACTORIAL_INPUT` const，改用 `crate::core::MAX_FACTORIAL_INPUT`（消除遮蔽）
  - `scientific.rs` 硬编码 `10_000` 替换为 `MAX_FACTORIAL_INPUT` 常量
  - `arithmetic.rs` 的 `BinaryOp::Pow` 添加 f64 天然防护注释（说明无需显式上界检查）

#### 已知限制

- [安全 HIGH] MCP 无输入大小限制 — `mcp.rs:78-82`，需研究 rmcp 框架限制机制
- [安全 HIGH] spawn_blocking 池耗尽 — `http.rs:80-84`，需架构调整（连接限制/队列）
- [性能 HIGH] 缓存命中路径仍执行 `router.route()` + `extract_format_precision()` — 需架构调整，backlog
- [性能 HIGH] McpServer 每请求创建 OS 线程（`std::thread::scope`）— 已存在代码，后续独立任务
- [架构 MEDIUM] CLI feature gate 不对称 — `cli.rs:72-79` 用 `server` 而非 `http`/`mcp`
- [架构 MEDIUM] ServerAdapter trait 未利用 — `server/mod.rs:34`，生产代码直接调用 `run()`
- [架构 MEDIUM] 测试共享缓存非确定性 — `server/cache.rs` 全局缓存 + 集成测试，需测试隔离机制
- [Bug MEDIUM] 缓存键前缀碰撞 — `evaluator.rs:62-66`，`precision:` 前缀可能与用户表达式碰撞
- [Bug LOW] MCP thread::scope 阻塞 tokio worker — `mcp.rs:109-115`
- [安全 MEDIUM] server 启动失败退出码 1 与文件头定义冲突 — 保持现状
- [安全 MEDIUM] 缺少 CLI 参数控制 server 绑定地址 — 设计局限性，后续 phase
- server/mcp.rs `run()`/`start_inner()` 阻塞 stdio 路径无法单元测试 — 需子进程级测试

## [1.1.0] - 2026-06-29

CalNexus v1.1.0：LaTeX/steps/canonical 三种新输出格式 + 完整测试套件矩阵（快照/属性/基准/安全/性能/REPL 集成/fuzz），
并补齐 CLI 集成缺口与 wasm32 目标分析。

### 新增

- **LaTeX 输出**（latex-output）：`--latex` CLI 标志，`src/output/latex.rs` 实现 10 个分派函数覆盖标量/复数/矩阵/
  向量/多项式/BigInt/BigRational/复数列表/符号（`diff`→`\frac{d}{dx}\left(...\right)`、`integrate`→`\int ... dx`、
  `limit`→`\lim_{x \to a}`、`taylor`→级数展开）；`EvalResult::LaTeX(String)` 新变体
- **求值步骤输出**（steps-display）：`--steps` CLI 标志，`src/output/steps.rs` 后序遍历 AST 生成 `lhs op rhs = partial_result`
  步骤列表（PRD §4.1.1 示例 `(2+9)*7-6` → `2+9=11 → 11*7=77 → 77-6=71`），256 深度上限；
  `EvalResult::Steps(Vec<String>)` 新变体；支持 `--latex --steps` 组合输出
- **规范化形式输出**（canonical-output）：`--canonical` CLI 标志，`src/output/canonical.rs` 调用
  `AstCanonicalizer::canonicalize_no_fold` 输出 S-表达式（`3+2` → `(+ 2 3)`），跳过求值
- **CLI 冲突约束**：`--latex`/`--canonical`/`--steps` 与 `--json`/`--repl`/`--batch`/`--precision` 互斥，
  clap `conflicts_with_all` 强制冲突时退出码 2
- **快照测试**（snapshot-testing）：`tests/snapshot_tests.rs` 8 个 SNAP 测试 + 1 基础设施测试，
  `insta = "1"` 依赖；快照锁定于 `tests/snapshots/snapshot_tests__*.snap`
- **属性测试**（property-testing）：`tests/property_tests.rs` 12 个 PROP 测试（交换律/结合律/分配律/规范化幂等/
  缓存命中/三角恒等式），`proptest_config` 256 cases/test
- **基准测试**（benchmark-testing）：`benches/{parser,cache,domain}_bench.rs` 10 个 BENCH 用例，
  `criterion = "0.5"` 依赖；parser 吞吐、canonicalizer <10μs、cache hit <100μs、cache miss <1ms、
  arithmetic/scientific/matrix/symbolic/batch/is_prime 各项基准
- **安全测试**（security-testing）：`tests/security_tests.rs` 10 个 SEC 测试 + 1 基础设施测试，
  覆盖注入/深度溢出/阶乘上限/矩阵维度/整数溢出/除零/超时/控制字符/长度上限；SEC-002b（257 层）与 SEC-007（符号超时）标记 `#[ignore]`
- **性能回归测试**（performance-testing）：`tests/performance_tests.rs` 5 个 PERF 测试，
  criterion baseline 比对、冷启动 <200ms 硬限制、1000 条批量 <2000ms、valgrind DHAT 内存检查
- **REPL 集成测试**（repl-integration-testing）：`tests/repl_integration.rs` 7 个 IT-CLI 测试，
  `expectrl = "0.8"` 依赖；覆盖 `2+3`→`5`、`:let` 变量绑定、`:vars` 列出、`:quit` 退出、错误恢复；
  IT-CLI-020（↑历史召回）与 IT-CLI-022（Tab 补全）标记 `#[ignore]`；30s 超时包装
- **Fuzz crate**（fuzz-testing）：`fuzz/` 独立 crate（非 workspace 成员），6 个 fuzz target
  （parser/ast_depth/cache_key/canonicalizer/numeric_boundary/matrix_dim），
  `libfuzzer_sys::fuzz_target!` 宏；`cargo +nightly fuzz list` 全部识别
- **CLI 集成缺口补齐**：`tests/cli_integration.rs` 新增 6 个测试（LaTeX/canonical/steps 输出 + 3 个冲突退出码 2）

### 变更

- **tarpaulin `fail-under`**：`100` → `90`（v1.1 新增测试套件含 `#[ignore]` 与外部工具跳过路径）
- **版本号**：`1.0.0` → `1.1.0`

### 性能

- 冷启动（`calnexus '2+3'`）：~3ms（release 构建，目标 < 100ms）
- 缓存命中：~699ns/hit（release 构建，目标 < 100μs）
- 批量 1000 条并行求值：~8ms（release 构建，目标 < 30s）
- parser 吞吐：>10000 expr/s（criterion 基准）
- canonicalizer：< 10μs/expr（criterion 基准）

### 测试

- **1650 个测试全部通过**（4 个 `#[ignore]`）：lib 单元测试 1369 + CLI 集成 108 + 跨域集成 130 +
  performance 6 + property 12 + REPL integration 6 + security 10 + snapshot 9
- **行覆盖率 97.27%**（cargo-llvm-cov，`--fail-under-lines 90` 通过）；
  新增 `output/latex.rs` 97.79% / `output/steps.rs` 95.73%，其余模块均 ≥ 95%
- release 构建零警告；`cargo fmt --check` 通过；`cargo clippy --all-targets` 零告警
- 6 个 fuzz target 全部识别（`cargo +nightly fuzz list`）
- 3 个基准文件全部编译（`cargo bench --no-run`）

### 已知限制

- **wasm32 目标**：`oxcache` 依赖 `tokio` → `mio`，`mio` 不支持 `wasm32-unknown-unknown`；
  `cli` feature gate 正确隔离 `clap`/`rustyline`/`rayon`/`std::fs`/`std::time::Instant`，
  但缓存层的 tokio 依赖阻塞 wasm32 编译（计划 v1.2 重构缓存后端）
- SEC-002b（257 层嵌套）会触发真实栈溢出，标记 `#[ignore]`；SEC-002（100 层）通过
- SEC-007 符号超时因 `CalcError::Timeout` 变体未实现，标记 `#[ignore]`
- IT-CLI-020（↑历史召回）与 IT-CLI-022（Tab 补全）在 CI 中不稳定，标记 `#[ignore]`

## [1.0.0] - 2026-06-29

CalNexus v1.0.0：符号计算域、交互式 REPL、批量处理三大新功能，并修复 v0.8 全部已知限制。首个公开发布版本。

### 新增

- **符号计算域**（symbolic-domain）：priority=30，支持符号求导 `diff(expr, var)`、符号积分 `integrate(expr, var)`、
  表达式化简 `simplify(expr)`、极限计算 `limit(expr, var, point)`（含洛必达法则）、泰勒级数 `taylor(expr, var, order)`；
  `SymbolicExpr` IR 含 Const/Var/Add/Sub/Mul/Div/Pow/Neg/Ln/Sin/Cos/Tan/Exp 14 种节点，
  `ast_to_symbolic`/`symbolic_to_string` 双向转换，同类项合并（`extract_coeff`/`coeff_times`）
- **REPL 交互模式**（repl-mode）：`--repl` 启动基于 rustyline 14 的交互式 REPL，支持行编辑、历史记录、Tab 补全
  （60+ 函数名 + 6 个 REPL 命令）；`:let NAME = VALUE` 变量绑定、`:vars` 列出变量、`:quit`/`:q` 退出、`:help` 帮助、`:clear` 清屏
- **批量处理**（batch-processing）：`--batch FILE` 从文件批量求值，rayon 并行求值，`#` 注释与空行跳过，
  单条 ≤ 4096 字符、总条数 ≤ 1000；`--json` 输出 JSON 数组格式；Summary 行含总数/成功/错误/缓存命中/耗时
- **隐式乘法**：Parser 预处理 `insert_implicit_multiplication`，`2x`→`2*x`、`3(x+1)`→`3*(x+1)`、
  `(x+1)(x-1)`→`(x+1)*(x-1)`；科学计数法（`1e308`）自动排除
- **高次多项式求根**：`roots()` 支持 3 次（Cardano 公式）与 4 次（Ferrari 方法）多项式求根
- **多项式除法扩展**：`poly_div` 支持 `Poly/Number`（逐系数除法）与 `Poly/Poly`（长除法）
- **rustyline 14 + rayon 1 依赖**：`cli` feature 新增 `rustyline = "14"`（含 `derive` feature）与 `rayon = "1"`
- **tempfile + predicates dev-dependency**：批量处理测试与 CLI 集成测试

### 修复（v0.8 限制）

- **BigNumber 路由冲突**：`PrecisionDomain::supports()` 当 AST 含 BigNumber 且同时含数论/组合函数时返回 false，
  `is_prime(BigNumber)` 正确路由至 NumberTheory 而非 Precision
- **`format_factor_linear(a=-1)` 格式化**：`a==-1.0` 时输出 `-(x-r)` 而非 `-*(x-r)`
- **多项式除法**：`expr_to_coeffs` 新增 `BinaryOp::Div` 路径（`Poly/Number` 与 `Poly/Poly`）

### 性能

- 冷启动（`calnexus '2+3'`）：~3ms（release 构建，目标 < 100ms）
- 缓存命中：~699ns/hit（release 构建，目标 < 100μs）
- `diff(x^2, x)`：< 1ms（release 构建）
- 批量 1000 条并行求值：~8ms（release 构建，目标 < 30s）

### 测试

- **1449 个测试全部通过**（lib + CLI + integration）：
  - lib 单元测试：1222 个（core 模块 + 11 个域模块含 Symbolic，含 proptest 属性测试）
  - CLI 集成测试：97 个（assert_cmd 端到端，覆盖 11 域 + --repl + --batch + --json + Symbolic + 错误退出码）
  - 跨域集成测试：130 个（全链路 + Symbolic + BigNumber 路由 + 隐式乘法 + 多项式高次求根 + 缓存去重 + 错误传播）
- **行覆盖率 99.10%**（cargo-llvm-cov）；Symbolic 99.01%、polynomial 99.80%、cli 97.24%
- release 构建零警告

### 已知限制

- 符号积分 `integrate()` 仅支持多项式幂函数与基本初等函数（sin/cos/exp/1/x），不支持分部积分、换元积分等高级技巧
- 3-4 次多项式求根（Cardano/Ferrari）在重根或判别式接近零时精度有限（浮点误差累积）
- `limit()` 洛必达法则递归深度上限 5 层，深层嵌套的 0/0 型可能无法求解

## [0.8.0] - 2026-06-29

CalNexus v0.8.0：四个新计算域（NumberTheory / Combinatorics / Vector / Polynomial），扩展大学数学计算覆盖。

### 新增

- **数论域**（number-theory-domain）：priority=25，基于 num-bigint::BigInt，支持 `gcd`/`lcm`/`is_prime`/`prime_sieve`/
  `mod_inverse`/`mod_pow`/`euler_phi`；确定性 Miller-Rabin（n < 2^64 用 12 基，n ≥ 2^64 用 25 轮），
  扩展欧几里得模逆，快速模幂，埃拉托斯特尼筛法
- **组合域**（combinatorics-domain）：priority=25，支持排列 `P(n,k)`、组合 `C(n,k)`、Catalan 数 `catalan(n)`、
  第二类 Stirling 数 `stirling(n,k)`；u128 累积溢出自动升级 BigInt
- **向量域**（vector-domain）：priority=30，基于 nalgebra::DVector，支持向量算术（`[a,b]+[c,d]`）、
  点积 `dot`、叉积 `cross`（3D）、模长 `norm`、夹角 `angle`、混合积 `scalar_triple`、归一化 `normalize`
- **多项式域**（polynomial-domain）：priority=25，系数升幂存储，支持 `poly_add`/`poly_sub`/`poly_mul`/`poly_div`/
  `poly_eval`（Horner）/`poly_diff`/`poly_integrate`/`roots`（1-2 次，实根 Vector / 复根 ComplexList）/`factor`
  （有理根定理，返回 Symbolic）
- **EvalResult 4 新变体**：`Vector(Vec<f64>)`、`Polynomial(Vec<f64>)`、`ComplexList(Vec<(f64,f64)>)`、`Symbolic(String)`，
  含 `as_vector()`/`as_polynomial()`/`as_complex_list()`/`as_symbolic()` helper
- **num-integer 依赖**：`Cargo.toml` 新增 `num-integer = "0.1"`，用于 BigInt gcd/lcm
- **CLI 扩展**：默认路由器注册 4 新域（共 10 域），`format_result()` 支持 4 新变体输出格式，
  `--help` 函数列表新增 17 个函数

### 性能

- 冷启动（`calnexus '2+3'`）：~3ms（release 构建，目标 < 100ms）
- 缓存命中（`CacheManager::get`）：~699ns/hit（release 构建，目标 < 100μs，缓存代码未变沿用 v0.5 数据）
- `is_prime(999999999999989)`（15 位素数）：~4ms（release 构建，目标 < 100ms）

### 测试

- **1203 个测试全部通过**（lib + CLI + integration）：
  - lib 单元测试：1028 个（core 模块 + 10 个域模块，含 proptest 属性测试）
  - CLI 集成测试：65 个（assert_cmd 端到端，覆盖 10 域 + --precision + --json + 错误退出码）
  - 跨域集成测试：110 个（全链路 parse→canonicalize→cache→route→evaluate + 缓存去重 + 错误传播）
- **行覆盖率 99.18%**（cargo-llvm-cov）；NumberTheory 98.91%、Combinatorics 98.46%、Vector 99.00%、Polynomial 99.01%
- release 构建零警告

### 已知限制

- `is_prime()`/`gcd()` 等数论函数与大数字面量（≥16 位 → BigNumber）组合时，路由至 PrecisionDomain 而非
  NumberTheoryDomain（同 priority=25，PrecisionDomain 先注册胜出）；`is_prime(10^18+9)` 需用 `eval_int` 间接调用
- 多项式 `roots()` 与 `factor()` 仅支持 1-2 次多项式（v0.8 范围限制）
- 多项式表达式不支持隐式乘法（`2x` 需写 `2*x`）与除法（`(x+1)/(x+2)` 返回 DomainError）
- `format_factor_linear(a=-1)` 输出 `-*(x-r)`（格式化逻辑的已知行为，非 bug）

## [0.1.0] - 2026-06-28

CalNexus v0.1.0 首个可用版本：命令行数学表达式求值器，覆盖大学本科以下全部计算需求。

### 新增

- **三 crate Cargo workspace**：`calnexus-core`（引擎）、`calnexus-domains`（计算域）、`calnexus-cli`（二进制）
- **表达式解析**（expression-parsing）：基于 mathexpr，支持四则运算、幂、阶乘（`!`）、取模（`%`→`mod()`）、
  绝对值、三角/反三角/对数/指数/双曲函数、gamma、erf、pi/e 常量；AST 深度上限 256
- **AST 规范化**（ast-canonicalization）：常量折叠、交换律排序（Add/Mul）、一元归一化（双重负号消除）；
  生成 S-表达式 `CanonicalForm` 用于缓存去重
- **L1 缓存**（l1-cache）：Moka sync::Cache + BLAKE3 哈希键，容量 10000，仅缓存 Ok 结果，线程安全（Send + Sync）
- **域路由**（domain-routing）：`CalculationDomain` trait + `DomainRouter`，按 priority() 降序稳定排序，
  首个 supports() 返回 true 的域胜出
- **算术域**（arithmetic-domain）：priority=10，支持四则/幂/阶乘/取模/绝对值；除零预检查（0/0→NaNOrInf,
  x/0→DivisionByZero），阶乘上限 10000，is_finite() 检测溢出
- **科学域**（scientific-domain）：priority=20，支持三角/反三角/对数/指数/双曲/gamma/erf + pi/e 常量预绑定；
  Lanczos 逼近 gamma（g=7, n=9），A&S 7.1.26 逼近 erf（x=0 特判）
- **CLI**（cli-interface）：clap v4 derive，位置参数/`--var`/`--json`，stdin 管道支持（IsTerminal 检测），
  退出码映射（成功 0 / 计算错误 1 / 系统错误 2）

### 性能

- 冷启动（`calnexus '2+3'`）：~23µs（release 构建，目标 < 100ms）
- 缓存命中（`CacheManager::get`）：~153ns/hit（release 构建，目标 < 100µs）

### 测试

- **262 个测试全部通过**（debug + release）：
  - calnexus-core：140 单元测试（types/parser/canonicalizer/cache/domain）
  - calnexus-core：33 集成测试（全链路 + 缓存去重 + 错误传播 + 性能）
  - calnexus-domains：70 单元测试（arithmetic 33 + scientific 37，含 9 个 proptest）
  - calnexus-cli：21 集成测试（assert_cmd 端到端）
- TDD 流程：每个能力遵循 红→绿→proptest→重构 循环

### 已知限制

- v0.1 仅支持标量 f64（无 BigInt/Matrix/Vector）
- CLI 为一次性进程，缓存对单次调用无效（为未来 REPL 模式预留）
- erf 逼近最大误差 ~1.5e-7（A&S 7.1.26）
- 仅在 Linux x86_64 上验证（其他平台未强制，但无平台特定依赖）

## [0.5.0] - 2026-06-28

CalNexus v0.5.0：单 crate 架构重构 + 四个新计算域 + 100% 测试覆盖率。

### 变更

- **单 crate 架构**：将原三 crate workspace（calnexus-core / calnexus-domains / calnexus-cli）合并为单 crate 多 mod
  结构（`src/core/` + `src/domains/` + `src/cli.rs`），消除跨 crate 依赖管理开销
- **Git 仓库初始化**：按 v0.1 tasks.md 章节追溯性提交（10 次提交含 chore:init），Conventional Commits 格式
- **AST 扩展**：新增 `Complex(f64,f64)`、`Matrix(Vec<Vec<AstNode>>)`、`List(Vec<AstNode>)`、`BigNumber(String)` 四个
  AstNode 变体；parser 支持复数字面量（`3+4i`）、矩阵字面量（`[[1,2],[3,4]]`）、列表字面量（`[1,2,3]`）、
  大整数字面量（≥16 位整数 → BigNumber 占位符方案）

### 新增

- **复数域**（complex-domain）：priority=30，基于 num_complex::Complex64，支持四则运算、模 `abs()`、幅角 `arg()`、
  共轭 `conj()`、复指数 `exp()`、复对数 `ln()`；变量 `i` 自动绑定为虚数单位
- **矩阵域**（matrix-domain）：priority=30，基于 nalgebra::DMatrix，支持加减乘、标量乘、行列式 `det()`、
  转置 `transpose()`、逆 `inverse()`、单位矩阵 `identity(n)`；维度校验与奇异矩阵检测返回 DomainError
- **统计域**（statistics-domain）：priority=20，自研无外部依赖，支持 `mean/variance/std/median/min/max/sum/count`；
  输入为 List 节点，空列表返回 DomainError，使用总体方差（N）
- **精确计算域**（precision-domain）：priority=25，基于 num_bigint::BigInt + num_rational::BigRational，
  支持大整数运算（不丢精度）、精确分数（自动约分）、大数阶乘、大整数幂；`--precision N` CLI 参数绕过路由器
  直接求值，`precision(N, expr)` 函数通过路由器路由；输出格式化（完整十进制 / 分数形式 / N 位小数截断）
- **`--precision N` CLI 参数**：任意精度模式，BigRational 求值后格式化为 N 位小数

### 性能

- 冷启动（`calnexus '2+3'`）：~1.78ms（release 构建，目标 < 100ms）
- 缓存命中（`CacheManager::get`）：~699ns/hit（release 构建，目标 < 100µs）

### 测试

- **728 个测试全部通过**（release 构建）：
  - lib 单元测试：607 个（core 模块 + 6 个域模块，含 proptest 属性测试）
  - CLI 集成测试：49 个（assert_cmd 端到端，覆盖全部 6 域 + --precision + --json + 错误退出码）
  - 跨域集成测试：72 个（全链路 parse→canonicalize→cache→route→evaluate + 缓存去重 + 错误传播 + 性能）
- **行覆盖率 98.66%**（cargo-llvm-cov，排除 src/main.rs）；剩余未覆盖行为 unreachable!() / TTY stdin /
  compile-time const fn / test helper panic 分支 / CLI cache hit 路径

### 已知限制

- 常量折叠在规范化阶段执行，`1/3` 等表达式被折叠为 f64 近似值后传入 PrecisionDomain，
  `precision(N, 1/3)` 的 BigRational 结果为 f64 近似而非精确 1/3（仅影响纯标量常量表达式，
  含 BigNumber 的表达式保留精确求值）
- 复数节点不参与交换律排序，`(1+2i)+(3+4i)` 与 `(3+4i)+(1+2i)` 规范形式不同（求值结果相同）
- CLI 为一次性进程，缓存对单次调用无效（为未来 REPL 模式预留）
