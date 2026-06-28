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
