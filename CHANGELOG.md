# Changelog

本项目所有重要变更均记录于此文件。格式遵循 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，
版本号遵循 [Semantic Versioning](https://semver.org/lang/zh-CN/)。

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
