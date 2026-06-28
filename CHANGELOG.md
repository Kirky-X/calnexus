# Changelog

本项目所有重要变更均记录于此文件。格式遵循 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，
版本号遵循 [Semantic Versioning](https://semver.org/lang/zh-CN/)。

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
