# P0 任务清单 — 地基层

> change: `p0-foundation`
> TDD 循环: 定接口 → 写测试(red) → 写代码(green) → 跑测试 → commit → gitnexus analyze → 下一个

## Phase 0.1: 依赖更新

### T0.1.1 升级 oxcache 0.2 → 0.3
- [ ] 检查 oxcache 0.3 的 BREAKING CHANGES（web search crates.io）
- [ ] 更新 Cargo.toml `oxcache = { version = "0.3", ... }`
- [ ] 适配 cache.rs 中 API 变更（如有）
- [ ] `cargo test --features cli` 全绿
- [ ] commit: `chore(deps): upgrade oxcache 0.2 → 0.3`
- **验收**: cache 模块测试全绿，无 API 破坏

### T0.1.2 升级其余依赖到最新稳定版
- [ ] `cargo update` 检查可升级项
- [ ] 逐个升级：blake3 / serde / tokio / regex / num-* / nalgebra / clap / rustyline / rayon
- [ ] 每升一个跑 `cargo test --features cli`
- [ ] dev-deps 同步：proptest / assert_cmd / predicates / tempfile / serde_json / insta / criterion / expectrl
- [ ] `cargo clippy --features cli --all-targets` 零警告
- [ ] commit: `chore(deps): upgrade all dependencies to latest stable`
- **验收**: 1650 测试全绿，clippy 零警告

### T0.1.3 新增 thiserror 依赖
- [ ] Cargo.toml 新增 `thiserror = { version = "2", default-features = false }`
- [ ] `cargo build --features cli` 编译通过
- [ ] commit: `chore(deps): add thiserror 2 for error derivation`
- **验收**: 编译通过，无警告

## Phase 0.2: Feature 细粒度重划分

### T0.2.1 重写 `[features]` 节
- [ ] 定接口：新的 feature 依赖图（见 design.md §2）
- [ ] 写测试：`tests/feature_matrix.rs` — 验证各 feature 组合可编译
  - `--no-default-features`
  - `--features cli`
  - `--features icu`
  - `--features cli,icu`
  - `--features server`（应等于 http+mcp，P0 为空但可编译）
- [ ] 改 Cargo.toml `[features]`:
  ```toml
  default = []
  cli = ["dep:clap", "dep:rustyline", "dep:rayon"]
  icu = ["dep:icu"]
  http = []
  mcp = []
  fx = []
  numerical = []
  server = ["http", "mcp"]
  ```
- [ ] 新增 `icu` 依赖（但 P0.3 才填内容）: `icu = { version = "2", default-features = false, features = ["std"], optional = true }`
- [ ] 跑 feature matrix 测试
- [ ] commit: `refactor(features): fine-grained feature gates (cli/icu/http/mcp/fx/numerical/server)`
- **验收**: 7 种 feature 组合全部可编译

### T0.2.2 适配 lib.rs 的 feature gate
- [ ] 检查 lib.rs 现有 `#[cfg(feature = "cli")]` 模块声明
- [ ] 新增 `#[cfg(feature = "icu")] mod i18n;` 占位（P0.3 填内容）
- [ ] `cargo build --no-default-features` 通过（核心库零依赖）
- [ ] commit: `refactor(lib): add icu feature gate placeholder`
- **验收**: `--no-default-features` 编译通过

## Phase 0.3: ICU4X 国际化

### T0.3.1 定接口 — I18n 结构
- [ ] 定义 `src/i18n.rs`:
  ```rust
  pub enum Lang { En, Zh }
  pub struct I18n { lang: Lang }
  impl I18n {
      pub fn new(lang: Lang) -> Self;
      pub fn from_str(s: &str) -> Self;  // "en"|"zh" 解析
      pub fn t(&self, key: &str) -> &str; // 消息目录
  }
  ```
- [ ] commit 接口: `feat(i18n): define I18n interface (red phase)`

### T0.3.2 写测试 — I18n 单元测试
- [ ] `Lang::from_str("en")` / `from_str("zh")` / `from_str("fr")` 回退 en
- [ ] 全错误消息键的中英双语翻译存在
- [ ] 未知键返回键本身（fail-loud）
- [ ] 测试编译失败（red）

### T0.3.3 实现 — I18n
- [ ] 实现 `Lang` 枚举 + `from_str`（用 `icu::locid` 解析 BCP-47）
- [ ] 实现消息目录 match 表（ErrorKind → i18n key → en/zh 文本）
- [ ] 消息键清单:
  - `error.parse` / `error.eval` / `error.overflow` / `error.division_by_zero`
  - `error.domain` / `error.depth` / `error.nan_or_inf`
  - `error.undefined_symbol` / `error.timeout` / `error.usage`
- [ ] 测试全绿（green）
- [ ] commit: `feat(i18n): implement ICU4X-based bilingual error messages`
- **验收**: 全消息键中英双语，未知键 fail-loud

## Phase 0.4: 结构化错误 ④

### T0.4.1 定接口 — Span + ErrorKind + CalcError
- [ ] 定义 `Span` struct (start, end)
- [ ] 定义 `ErrorKind` enum (10 种变体 + exit_code 方法)
- [ ] 定义新 `CalcError` struct (kind, message, span, hint) + thiserror derive
- [ ] 定义便捷构造器签名 (parse/eval/overflow/...)
- [ ] commit 接口: `feat(errors): define Span/ErrorKind/CalcError interface (red phase)`

### T0.4.2 写测试 — 错误类型单元测试
- [ ] Span::new / Span::point / Span::is_empty
- [ ] ErrorKind::exit_code: 全 10 种映射 (1/2/3)
- [ ] CalcError 便捷构造器: 全 10 种
- [ ] CalcError::with_span / with_hint 链式
- [ ] friendly()/to_json()/to_explain() 三态呈现（有/无 span+hint）
- [ ] Display trait 输出 message
- [ ] Error trait 可 downcast
- [ ] 测试编译失败（red）

### T0.4.3 实现 — 新 CalcError
- [ ] 实现 Span + ErrorKind + CalcError struct
- [ ] thiserror derive `#[error("{message}")]`
- [ ] 实现全部便捷构造器
- [ ] 实现 friendly() / to_json() / to_explain()
- [ ] 测试全绿（green）
- [ ] commit: `feat(errors): implement structured CalcError with Span/Kind/hint/thiserror`

### T0.4.4 迁移 — 全局替换旧 CalcError 构造
- [ ] `grep -rn "CalcError::ParseError"` 找所有调用点
- [ ] 替换为 `CalcError::parse(...)`
- [ ] 替换 `CalcError::EvalError` → `CalcError::eval(...)`
- [ ] 替换 `CalcError::Overflow` → `CalcError::overflow()`
- [ ] 替换 `CalcError::NaNOrInf` → `CalcError::nan_or_inf()`
- [ ] 替换 `CalcError::DomainError` → `CalcError::domain(...)`
- [ ] 替换 `CalcError::DepthExceeded` → `CalcError::depth_exceeded()`
- [ ] 替换 `CalcError::DivisionByZero` → `CalcError::division_by_zero()`
- [ ] `cargo test --features cli` 全 1650 测试绿
- [ ] commit: `refactor(errors): migrate all CalcError constructors to new API`
- **验收**: 零旧构造残留，全测试绿

### T0.4.5 Span 生成 — parser 预处理层
- [ ] 在 parser.rs 的 `preprocess_*` 函数中，错误路径添加 Span（已有字符索引）
- [ ] `preprocess_brackets` 错误 → `CalcError::parse(...).with_span(...)`
- [ ] `validate_no_consecutive_plus` 错误 → with_span
- [ ] `preprocess_complex` 错误 → with_span
- [ ] `preprocess_factorial` 错误 → with_span
- [ ] mathexpr 解析失败 → 用 `after_implicit` 字符串长度作为 span 末端
- [ ] 写测试: 语法错误 Span 精确性（位置断言）
- [ ] commit: `feat(parser): generate Span in preprocessing layer`
- **验收**: 语法错误有精确位置

### T0.4.6 hint 生成 — 关键错误路径
- [ ] UndefinedSymbol: hint "try :let <name> = <value>"
- [ ] DivisionByZero: hint "check divisor before division"
- [ ] DomainError(asin(2)): hint "asin domain is [-1, 1]"
- [ ] DepthExceeded: hint "simplify nested expressions (max 256)"
- [ ] Timeout: hint "increase --timeout or simplify expression"
- [ ] 写测试: 关键错误路径 hint 断言
- [ ] commit: `feat(errors): add hints for common error paths`

### T0.4.7 CLI 退出码 + --explain flag
- [ ] cli.rs: `--explain` flag (与 --json 互斥)
- [ ] cli.rs: `--lang <en|zh>` flag (默认 en)
- [ ] cli.rs: 错误时调用 `error.kind.exit_code()` 返回正确退出码
- [ ] cli.rs: `--explain` 时调用 `error.to_explain(&i18n)`
- [ ] cli.rs: `--json` 错误时在 JSON 中加 `error` 对象
- [ ] 写集成测试: 退出码 0/1/2/3 + --explain + --lang
- [ ] commit: `feat(cli): add --explain flag, --lang flag, exit code 3 for timeout`
- **验收**: 退出码契约完整，--explain/--lang 端到端工作

### T0.4.8 解除 SEC-007 ignore
- [ ] 找到 `tests/security_tests.rs` 中 SEC-007 `#[ignore]`
- [ ] 实现 Timeout 错误的触发路径（或在测试中模拟）
- [ ] 移除 `#[ignore]`
- [ ] 测试绿
- [ ] commit: `test(security): un-ignore SEC-007 timeout test`
- **验收**: SEC-007 不再 ignore

## Phase 0.5: 收尾

### T0.5.1 覆盖率检查
- [ ] `cargo llvm-cov --features cli --fail-under-lines 95`
- [ ] 若不达标，补充未覆盖路径测试
- [ ] commit: `test(coverage): ensure ≥95% line coverage for P0`

### T0.5.2 clippy + fmt
- [ ] `cargo fmt --all`
- [ ] `cargo clippy --features cli --all-targets` 零警告
- [ ] commit: `chore: fmt + clippy cleanup`

### T0.5.3 CHANGELOG 记录
- [ ] docs/CHANGELOG.md 新增 `[0.2.0]` 条目（Unreleased 节）
- [ ] 记录 P0 全部变更
- [ ] commit: `docs(changelog): record P0 foundation changes`

## 验收标准总览

| 标准 | 目标 |
|---|---|
| 测试数 | ≥ 1650（不回退）+ 新增错误/icu 测试 |
| 行覆盖率 | ≥ 95% |
| clippy | 零警告 |
| feature 组合 | 7 种全部可编译 |
| 退出码 | 0/1/2/3 完整契约 |
| 错误三态 | friendly/json/explain 全实现 |
| icu 双语 | 中英全消息键 |
| 旧测试 | 1650 全绿（迁移后） |
