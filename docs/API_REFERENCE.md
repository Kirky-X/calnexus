# 📘 CalNexus API 参考

> 本文档描述 CalNexus 的两类公开 API：**表达式求值 API**（`evaluate()`，经解析与域路由的完整流水线）与**直接 API**（`CalNexus` 门面，跳过解析的程序化调用）。
> 直接 API 是表达式求值 API 的补充而非替代。

## 📋 目录

- [🎯 概述](#-概述)
  - [API 设计原则](#api-设计原则)
  - [📦 特性说明](#-特性说明)
- [🧱 核心 API](#-核心-api)
  - [表达式求值入口](#表达式求值入口)
  - [CalNexus 门面](#calnexus-门面)
  - [变量绑定](#变量绑定)
- [🔌 分组 API](#-分组-api)
  - [标量运算 — cn.scalar()](#标量运算--cnscalar)
  - [线性代数 — cn.linalg()](#线性代数--cnlinalg)
  - [数据分析 — cn.stats()](#数据分析--cnstats)
  - [符号数学 — cn.symbolic()](#符号数学--cnsymbolic)
  - [应用数学 — cn.applied()](#应用数学--cnapplied)
- [🧱 类型包装器](#-类型包装器)
- [🚨 错误类型](#-错误类型)
- [🚪 特性门控 API](#-特性门控-api)
- [💡 使用示例](#-使用示例)
- [📚 相关文档](#-相关文档)

---

## 🎯 概述

### API 设计原则

- **两条入口，一个内核**：表达式 API 面向用户输入（解析 → 规范化 → 缓存 → 域路由），直接 API 面向程序化调用（跳过解析/规范化开销）；两者共享同一数学函数层（`math/`）。
- **依赖单向**：`api/` → `math/` → `core/`，严格单向；直接 API 不依赖 `domains/`。
- **零依赖默认**：`default = []`，核心 API 无可选依赖，可作为嵌入式计算引擎。
- **门控可选能力**：可选计算域与数值分解均为独立 feature，未启用则不可见。

### 📦 特性说明

| Feature | 启用模块 | 说明 |
|---------|----------|------|
| _(default, 无)_ | `scalar` / `linalg` / `stats` / `symbolic` | 核心 API，零额外依赖 |
| `numerical` | `linalg.eig/svd/lu/qr/solve/matrix_exp` | 数值线性代数分解（nalgebra f64） |
| `unit` | `applied.convert()` | 8 量纲物理单位换算 |
| `time` | `applied.date/datetime/now/today/...` | 13 个时间函数（jiff 0.2 + IANA tzdb） |
| `fx` | `applied.fx/fx_rate` | 汇率换算（frankfurter.dev API + 三级缓存） |

> 服务端 feature（`cli`/`http`/`mcp`/`server`/`icu`/`ratelimit`/`docs`/`observability`）见 [🏗️ 架构文档](ARCHITECTURE.md) 的 Feature Gate 策略一节。

---

## 🧱 核心 API

### 表达式求值入口

```rust
pub fn evaluate(
    expr: &str,
    ctx: &EvalContext,
    precision: Option<usize>,
    cache: &CacheManager,
) -> Result<(EvalResult, String, bool, Option<usize>), CalcError>

pub fn evaluate_with_router(
    expr: &str,
    ctx: &EvalContext,
    precision: Option<usize>,
    cache: &CacheManager,
    router: &DomainRouter,
) -> Result<(EvalResult, String, bool, Option<usize>), CalcError>
```

- `evaluate` 编排五阶段流水线：输入校验 → parse + canonicalize + 超时检查 → 缓存键构建 → 按 precision 模式分发（`precision` 模式绕过路由器走 BigRational；常规模式经 `DomainRouter` 域路由）。
- 返回值四元组：`(结果, 命中域名, 是否缓存命中, 格式化精度回显)`。第 3 元素 `false` 表示本次真实求值（含非确定性旁路），`true` 表示命中缓存（含 single-flight follower 共享）；第 4 元素 `Option<usize>` 为表达式 `precision(N, expr)` 中的 N（precision 模式下为传入的 precision 参数），供输出层格式化使用。
- `evaluate_with_router` 允许注入自定义 `DomainRouter`（下游可扩展自定义 `CalculationDomain`）；`evaluate` 绑定进程级默认路由器。
- 缓存经 `try_get_with` single-flight：并发相同键只真实求值一次；非确定性函数（`now` / `today` / `fx` / `fx_rate`）旁路缓存读写。

### CalNexus 门面

`CalNexus` 是直接 API 的门面结构体，持有变量上下文，提供 5 个分组访问器：

```rust
use calnexus::CalNexus;

let cn = CalNexus::new();     // 创建默认实例
// 或
let cn = CalNexus::default(); // 同上

// 标量运算
let result = cn.scalar().add(2.0, 3.0).unwrap();
assert_eq!(result.as_scalar(), Some(5.0));

// 三角函数
let result = cn.scalar().sin(std::f64::consts::FRAC_PI_2).unwrap();
assert_eq!(result.as_scalar(), Some(1.0));
```

分组访问器与实现：

| 访问器 | trait | 方法数 | 覆盖 |
|--------|-------|--------|------|
| `cn.scalar()` | `ScalarMath` | 37 | 算术(8) + 科学函数(14) + 精度(1) + 数论(9) + 组合(5) |
| `cn.linalg()` | `LinearAlgebra` | 20 | 矩阵(8) + 向量(6) + 数值分解(6) |
| `cn.stats()` | `DataAnalysis` | 32 | 基础统计(8) + 分布(16) + 假设检验(3) + 相关(2) + 回归(3) |
| `cn.symbolic()` | `SymbolicMath` | 21 | 符号演算(5) + 多项式(6) + 复数(9) + 方程求解(1) |
| `cn.applied()` | `AppliedMath` | 16 | 时间(13) + 单位(1) + 汇率(2)，feature 门控 |

### 变量绑定

```rust
cn.set_var("x", 10.0);
assert_eq!(cn.get_var("x"), Some(10.0));

cn.clear_vars();
assert_eq!(cn.get_var("x"), None);
```

变量通过 `RwLock<EvalContext>` 管理，支持多线程并发读取、独占写入。

---

## 🔌 分组 API

### 标量运算 — cn.scalar()

#### 算术

| 方法 | 签名 | 说明 |
|------|------|------|
| `add` | `(a: f64, b: f64) → Result<EvalResult, CalcError>` | 加法 |
| `sub` | `(a: f64, b: f64) → Result<EvalResult, CalcError>` | 减法 |
| `mul` | `(a: f64, b: f64) → Result<EvalResult, CalcError>` | 乘法 |
| `div` | `(a: f64, b: f64) → Result<EvalResult, CalcError>` | 除法（除零返回错误） |
| `pow` | `(a: f64, b: f64) → Result<EvalResult, CalcError>` | 幂运算 |
| `rem` | `(a: f64, b: f64) → Result<EvalResult, CalcError>` | 取模 |
| `factorial` | `(n: u64) → Result<EvalResult, CalcError>` | 阶乘 |
| `abs` | `(x: f64) → Result<EvalResult, CalcError>` | 绝对值 |

#### 科学函数

| 方法 | 签名 | 说明 |
|------|------|------|
| `sin` | `(x: f64) → Result<EvalResult, CalcError>` | 正弦 |
| `cos` | `(x: f64) → Result<EvalResult, CalcError>` | 余弦 |
| `tan` | `(x: f64) → Result<EvalResult, CalcError>` | 正切 |
| `asin` | `(x: f64) → Result<EvalResult, CalcError>` | 反正弦 |
| `acos` | `(x: f64) → Result<EvalResult, CalcError>` | 反余弦 |
| `atan` | `(x: f64) → Result<EvalResult, CalcError>` | 反正切 |
| `ln` | `(x: f64) → Result<EvalResult, CalcError>` | 自然对数 |
| `log` | `(x: f64, base: f64) → Result<EvalResult, CalcError>` | 任意底数对数 |
| `exp` | `(x: f64) → Result<EvalResult, CalcError>` | 指数函数 |
| `sinh` | `(x: f64) → Result<EvalResult, CalcError>` | 双曲正弦 |
| `cosh` | `(x: f64) → Result<EvalResult, CalcError>` | 双曲余弦 |
| `tanh` | `(x: f64) → Result<EvalResult, CalcError>` | 双曲正切 |
| `gamma` | `(x: f64) → Result<EvalResult, CalcError>` | Gamma 函数 |
| `erf` | `(x: f64) → Result<EvalResult, CalcError>` | 误差函数 |

#### 精度

| 方法 | 签名 | 说明 |
|------|------|------|
| `precision_eval` | `(digits: usize, expr: &str) → Result<EvalResult, CalcError>` | BigRational 任意精度求值 |

#### 数论

| 方法 | 签名 | 说明 |
|------|------|------|
| `gcd` | `(a: &BigNumber, b: &BigNumber) → Result<EvalResult, CalcError>` | 最大公约数 |
| `lcm` | `(a: &BigNumber, b: &BigNumber) → Result<EvalResult, CalcError>` | 最小公倍数 |
| `is_prime` | `(n: &BigNumber) → Result<EvalResult, CalcError>` | 素数判定 |
| `prime_sieve` | `(n: u64) → Result<EvalResult, CalcError>` | 素数筛 |
| `mod_pow` | `(base: &BigNumber, exp: &BigNumber, m: &BigNumber) → Result<EvalResult, CalcError>` | 模幂运算 |
| `mod_inverse` | `(a: &BigNumber, m: &BigNumber) → Result<EvalResult, CalcError>` | 模逆元 |
| `euler_phi` | `(n: &BigNumber) → Result<EvalResult, CalcError>` | 欧拉函数 |
| `crt` | `(remainders: &[BigNumber], moduli: &[BigNumber]) → Result<EvalResult, CalcError>` | 中国剩余定理 |
| `discrete_log` | `(g: &BigNumber, h: &BigNumber, p: &BigNumber) → Result<EvalResult, CalcError>` | 离散对数（BSGS） |

#### 组合数学

| 方法 | 签名 | 说明 |
|------|------|------|
| `perm` | `(n: u64, k: u64) → Result<EvalResult, CalcError>` | 排列数 P(n,k) |
| `comb` | `(n: u64, k: u64) → Result<EvalResult, CalcError>` | 组合数 C(n,k) |
| `catalan` | `(n: u64) → Result<EvalResult, CalcError>` | Catalan 数 |
| `stirling_first` | `(n: u64, k: u64) → Result<EvalResult, CalcError>` | 第一类 Stirling 数 |
| `stirling_second` | `(n: u64, k: u64) → Result<EvalResult, CalcError>` | 第二类 Stirling 数 |

### 线性代数 — cn.linalg()

#### 矩阵运算

| 方法 | 签名 | 说明 |
|------|------|------|
| `det` | `(m: &Matrix) → Result<EvalResult, CalcError>` | 矩阵行列式 |
| `inverse` | `(m: &Matrix) → Result<EvalResult, CalcError>` | 矩阵求逆 |
| `transpose` | `(m: &Matrix) → Result<EvalResult, CalcError>` | 矩阵转置 |
| `identity` | `(n: usize) → Result<EvalResult, CalcError>` | 单位矩阵 |
| `mat_add` | `(a: &Matrix, b: &Matrix) → Result<EvalResult, CalcError>` | 矩阵加法 |
| `mat_sub` | `(a: &Matrix, b: &Matrix) → Result<EvalResult, CalcError>` | 矩阵减法 |
| `mat_mul` | `(a: &Matrix, b: &Matrix) → Result<EvalResult, CalcError>` | 矩阵乘法 |
| `scalar_mul` | `(s: f64, m: &Matrix) → Result<EvalResult, CalcError>` | 标量乘矩阵 |

#### 向量运算

| 方法 | 签名 | 说明 |
|------|------|------|
| `dot` | `(a: &Vector, b: &Vector) → Result<EvalResult, CalcError>` | 向量点积 |
| `cross` | `(a: &Vector, b: &Vector) → Result<EvalResult, CalcError>` | 向量叉积 |
| `normalize` | `(a: &Vector) → Result<EvalResult, CalcError>` | 单位化 |
| `magnitude` | `(a: &Vector) → Result<EvalResult, CalcError>` | 模长 |
| `vector_add` | `(a: &Vector, b: &Vector) → Result<EvalResult, CalcError>` | 向量加法 |
| `vector_sub` | `(a: &Vector, b: &Vector) → Result<EvalResult, CalcError>` | 向量减法 |

#### 数值分解（`numerical` feature）

| 方法 | 签名 | 说明 |
|------|------|------|
| `eig` | `(m: &Matrix) → Result<EvalResult, CalcError>` | 特征值/特征向量 |
| `svd` | `(m: &Matrix) → Result<EvalResult, CalcError>` | 奇异值分解 |
| `lu` | `(m: &Matrix) → Result<EvalResult, CalcError>` | LU 分解 |
| `qr` | `(m: &Matrix) → Result<EvalResult, CalcError>` | QR 分解 |
| `solve` | `(a: &Matrix, b: &Vector) → Result<EvalResult, CalcError>` | 线性方程组求解 |
| `matrix_exp` | `(m: &Matrix) → Result<EvalResult, CalcError>` | 矩阵指数 |

### 数据分析 — cn.stats()

#### 基础统计

| 方法 | 签名 | 说明 |
|------|------|------|
| `mean` | `(data: &[f64]) → Result<EvalResult, CalcError>` | 均值 |
| `variance` | `(data: &[f64]) → Result<EvalResult, CalcError>` | 方差 |
| `std` | `(data: &[f64]) → Result<EvalResult, CalcError>` | 标准差 |
| `median` | `(data: &[f64]) → Result<EvalResult, CalcError>` | 中位数 |
| `min` | `(data: &[f64]) → Result<EvalResult, CalcError>` | 最小值 |
| `max` | `(data: &[f64]) → Result<EvalResult, CalcError>` | 最大值 |
| `sum` | `(data: &[f64]) → Result<EvalResult, CalcError>` | 求和 |
| `count` | `(data: &[f64]) → Result<EvalResult, CalcError>` | 计数 |

#### 分布函数

| 方法 | 签名 | 说明 |
|------|------|------|
| `norm_pdf` | `(x: f64, mu: f64, sigma: f64) → Result<EvalResult, CalcError>` | 正态分布概率密度 |
| `norm_cdf` | `(x: f64, mu: f64, sigma: f64) → Result<EvalResult, CalcError>` | 正态分布累积分布 |
| `norm_inv` | `(p: f64, mu: f64, sigma: f64) → Result<EvalResult, CalcError>` | 正态分布逆函数 |
| `t_pdf` | `(x: f64, df: f64) → Result<EvalResult, CalcError>` | t 分布概率密度 |
| `t_cdf` | `(x: f64, df: f64) → Result<EvalResult, CalcError>` | t 分布累积分布 |
| `t_inv` | `(p: f64, df: f64) → Result<EvalResult, CalcError>` | t 分布逆函数 |
| `chi2_pdf` | `(x: f64, k: f64) → Result<EvalResult, CalcError>` | 卡方分布概率密度 |
| `chi2_cdf` | `(x: f64, k: f64) → Result<EvalResult, CalcError>` | 卡方分布累积分布 |
| `chi2_inv` | `(p: f64, k: f64) → Result<EvalResult, CalcError>` | 卡方分布逆函数 |
| `f_pdf` | `(x: f64, d1: f64, d2: f64) → Result<EvalResult, CalcError>` | F 分布概率密度 |
| `f_cdf` | `(x: f64, d1: f64, d2: f64) → Result<EvalResult, CalcError>` | F 分布累积分布 |
| `f_inv` | `(p: f64, d1: f64, d2: f64) → Result<EvalResult, CalcError>` | F 分布逆函数 |
| `poisson_pmf` | `(k: f64, lambda: f64) → Result<EvalResult, CalcError>` | 泊松分布概率质量 |
| `poisson_cdf` | `(k: f64, lambda: f64) → Result<EvalResult, CalcError>` | 泊松分布累积分布 |
| `binom_pmf` | `(k: f64, n: f64, p: f64) → Result<EvalResult, CalcError>` | 二项分布概率质量 |
| `binom_cdf` | `(k: f64, n: f64, p: f64) → Result<EvalResult, CalcError>` | 二项分布累积分布 |

#### 假设检验

| 方法 | 签名 | 说明 |
|------|------|------|
| `t_test_one` | `(data: &[f64], mu: f64) → Result<EvalResult, CalcError>` | 单样本 t 检验 |
| `t_test_two` | `(a: &[f64], b: &[f64]) → Result<EvalResult, CalcError>` | 双样本 t 检验 |
| `chi2_test` | `(observed: &[f64], expected: &[f64]) → Result<EvalResult, CalcError>` | 卡方检验 |

#### 相关分析

| 方法 | 签名 | 说明 |
|------|------|------|
| `pearson` | `(x: &[f64], y: &[f64]) → Result<EvalResult, CalcError>` | Pearson 相关系数 |
| `spearman` | `(x: &[f64], y: &[f64]) → Result<EvalResult, CalcError>` | Spearman 秩相关 |

#### 回归分析

| 方法 | 签名 | 说明 |
|------|------|------|
| `lin_reg` | `(x: &[f64], y: &[f64]) → Result<EvalResult, CalcError>` | 线性回归 |
| `poly_reg` | `(x: &[f64], y: &[f64], degree: usize) → Result<EvalResult, CalcError>` | 多项式回归 |
| `multi_reg` | `(x: &[Vec<f64>], y: &[f64]) → Result<EvalResult, CalcError>` | 多元回归 |

### 符号数学 — cn.symbolic()

#### 符号演算

| 方法 | 签名 | 说明 |
|------|------|------|
| `differentiate` | `(expr: &str, var: &str) → Result<EvalResult, CalcError>` | 符号微分 |
| `integrate` | `(expr: &str, var: &str) → Result<EvalResult, CalcError>` | 符号积分 |
| `simplify` | `(expr: &str) → Result<EvalResult, CalcError>` | 表达式化简 |
| `limit` | `(expr: &str, var: &str, target: f64) → Result<EvalResult, CalcError>` | 极限 |
| `taylor_expand` | `(expr: &str, var: &str, center: f64, order: usize) → Result<EvalResult, CalcError>` | 泰勒展开 |

#### 多项式运算

| 方法 | 签名 | 说明 |
|------|------|------|
| `poly_add` | `(a: &Polynomial, b: &Polynomial) → Result<EvalResult, CalcError>` | 多项式加法 |
| `poly_sub` | `(a: &Polynomial, b: &Polynomial) → Result<EvalResult, CalcError>` | 多项式减法 |
| `poly_mul` | `(a: &Polynomial, b: &Polynomial) → Result<EvalResult, CalcError>` | 多项式乘法 |
| `poly_div` | `(a: &Polynomial, b: &Polynomial) → Result<EvalResult, CalcError>` | 多项式除法 |
| `poly_roots` | `(p: &Polynomial) → Result<EvalResult, CalcError>` | 多项式求根 |
| `poly_eval` | `(p: &Polynomial, x: f64) → Result<EvalResult, CalcError>` | 多项式求值 |

#### 复数运算

| 方法 | 签名 | 说明 |
|------|------|------|
| `complex_add` | `(a: &Complex, b: &Complex) → Result<EvalResult, CalcError>` | 复数加法 |
| `complex_sub` | `(a: &Complex, b: &Complex) → Result<EvalResult, CalcError>` | 复数减法 |
| `complex_mul` | `(a: &Complex, b: &Complex) → Result<EvalResult, CalcError>` | 复数乘法 |
| `complex_div` | `(a: &Complex, b: &Complex) → Result<EvalResult, CalcError>` | 复数除法 |
| `complex_abs` | `(z: &Complex) → Result<EvalResult, CalcError>` | 复数模 |
| `complex_arg` | `(z: &Complex) → Result<EvalResult, CalcError>` | 复数辐角 |
| `complex_conj` | `(z: &Complex) → Result<EvalResult, CalcError>` | 复共轭 |
| `complex_exp` | `(z: &Complex) → Result<EvalResult, CalcError>` | 复指数 |
| `complex_ln` | `(z: &Complex) → Result<EvalResult, CalcError>` | 复对数 |

#### 方程求解

| 方法 | 签名 | 说明 |
|------|------|------|
| `solve_equation` | `(expr: &str, var: &str, method: &str, options: Option<&[f64]>) → Result<EvalResult, CalcError>` | 方程数值求解 |

`solve_equation` 支持三种方法：

- `"newton"`：牛顿法，options = `Some(&[x0])`（初始猜测）
- `"bisection"`：二分法，options = `Some(&[a, b])`（区间）
- `"brent"`：Brent 方法，options = `Some(&[a, b])`（区间）

### 应用数学 — cn.applied()

#### 时间（`time` feature）

| 方法 | 签名 | 说明 |
|------|------|------|
| `date` | `(date_str: &str) → Result<EvalResult, CalcError>` | 日期构造 |
| `datetime` | `(datetime_str: &str, tz: Option<&str>) → Result<EvalResult, CalcError>` | 日期时间构造 |
| `timestamp` | `(datetime_str: &str) → Result<EvalResult, CalcError>` | 日期转时间戳 |
| `from_timestamp` | `(secs: i64, tz: Option<&str>) → Result<EvalResult, CalcError>` | 时间戳转日期 |
| `now` | `(tz: Option<&str>) → Result<EvalResult, CalcError>` | 当前时间（非确定性，旁路缓存） |
| `today` | `(tz: Option<&str>) → Result<EvalResult, CalcError>` | 当前日期（非确定性，旁路缓存） |
| `date_add` | `(date: &str, n: i64, unit: &str) → Result<EvalResult, CalcError>` | 日期加法 |
| `date_diff` | `(a: &str, b: &str, unit: Option<&str>) → Result<EvalResult, CalcError>` | 日期差 |
| `format_date` | `(date: &str, fmt: &str, tz: Option<&str>) → Result<EvalResult, CalcError>` | 日期格式化 |
| `reformat_date` | `(input: &str, from_fmt: &str, to_fmt: &str) → Result<EvalResult, CalcError>` | 日期格式转换 |
| `weekday` | `(date: &str) → Result<EvalResult, CalcError>` | 星期几 |
| `day_of_year` | `(date: &str) → Result<EvalResult, CalcError>` | 年内第几天 |
| `is_leap_year` | `(year: i64) → Result<EvalResult, CalcError>` | 是否闰年 |

#### 单位换算（`unit` feature）

| 方法 | 签名 | 说明 |
|------|------|------|
| `convert` | `(value: f64, from: &str, to: &str) → Result<EvalResult, CalcError>` | 单位换算 |

#### 汇率换算（`fx` feature）

| 方法 | 签名 | 说明 |
|------|------|------|
| `fx` | `(amount: f64, from: &str, to: &str) → Result<EvalResult, CalcError>` | 汇率换算（非确定性，旁路缓存） |
| `fx_rate` | `(from: &str, to: &str) → Result<EvalResult, CalcError>` | 查询汇率（非确定性，旁路缓存） |

---

## 🧱 类型包装器

直接 API 使用 5 个类型安全包装器，与 `EvalResult` 双向转换：

| 包装器 | 封装类型 | 构造方法 |
|--------|----------|----------|
| `Matrix` | `Vec<Vec<f64>>` | `Matrix::from_rows(&[&[f64]])` |
| `Vector` | `Vec<f64>` | `Vector::new(&[f64])` |
| `Complex` | `Complex64` | `Complex::new(re, im)` |
| `Polynomial` | `Vec<f64>` (升幂) | `Polynomial::new(&[f64])` |
| `BigNumber` | `BigInt` | `BigNumber::from_i64(v)` / `BigNumber::new(bigint)` |

### 转换

```rust
use calnexus::{Matrix, EvalResult};

// 包装器 → EvalResult
let m = Matrix::from_rows(&[&[1.0, 2.0], &[3.0, 4.0]]);
let result: EvalResult = m.into();

// EvalResult → 包装器
let m2 = Matrix::try_from(result).unwrap();
```

---

## 🚨 错误类型

所有 API 的错误类型为 `CalcError`，其 `kind` 字段为 `ErrorKind` 枚举：

| ErrorKind | 退出码 | 服务端映射 | 触发场景 |
|-----------|--------|-----------|----------|
| `Parse` | 1 | 400 InvalidInput | 表达式语法错误 |
| `Eval` | 1 | 400 InvalidInput | 求值失败（定义域、未实现运算等） |
| `Overflow` | 1 | 400 InvalidInput | 数值溢出 |
| `DivisionByZero` | 1 | 400 InvalidInput | 除零 |
| `Domain` | 1 | 400 InvalidInput | 函数定义域错误 |
| `Depth` | 1 | 400 InvalidInput | 超出 AST 深度上限（`MAX_AST_DEPTH=256`） |
| `NaNOrInf` | 1 | 400 InvalidInput | NaN / 无穷结果 |
| `UndefinedSymbol` | 1 | 400 InvalidInput | 未绑定变量（附 `--var` / `:let` 提示） |
| `Usage` | 2 | 400 InvalidInput（message 前缀 `Usage:`） | 用法/参数错误 |
| `Timeout` | 3 | 503 ServiceUnavailable | 超过 `--timeout` |
| `DependencyUnavailable` | 3 | 503 ServiceUnavailable | 上游依赖故障（fx 汇率源不可达），带 `Retry-After` |

> **422 不来自 `ErrorKind`**：HTTP/MCP 的 422 `ValidationError` 由独立的请求体校验层（`server/types.rs::validate()`：expr ≤ 4096 字符、vars ≤ 1024 键、precision ≤ 10000）在求值之前产生；CLI 形态无此层，对应约束由 clap / `--precision` 校验承担（退出码 2）。

`CalcError` 携带 `message`（本地化文案，`--lang` / HTTP `lang` 协商）、`hint`（修复建议）、`source_detail`（错误链）。退出码契约由 `ErrorKind::exit_code()` 统一给出：0 = 成功、1 = 计算错误、2 = 用法错误、3 = 超时/上游不可用。

---

## 🚪 特性门控 API

| 特性 | 公开项 | 说明 |
|------|--------|------|
| `time` | `TimeDomain`、`applied()` 时间方法 | jiff 0.2 + 内嵌 IANA tzdb |
| `unit` | `UnitDomain`、`applied().convert()` | 8 量纲 + 温度仿射 |
| `fx` | `FxDomain`、`applied().fx/fx_rate` | frankfurter.dev + 三级缓存 + 熔断；`CALNEXUS_FX_*` 环境变量 |
| `numerical` | `linalg()` 数值分解方法 | nalgebra f64 近似 |
| `server` | `HttpServer` / `build_router` / `McpServer` / `build_mcp_server` | HTTP + MCP 服务 |
| `icu` | 影响 `I18n` 的 BCP-47 解析实现（`I18n` / `Lang` 类型始终可用，不受门控） | 启用 ICU4X 语言标签解析；未启用时 `from_str` 退化为简单字符串匹配 |

各计算域类型（`ArithmeticDomain` 等 14 个）均实现 `CalculationDomain` trait 并可经 `evaluate_with_router` 注入；`build_default_router()` 注册全部启用域。

---

## 💡 使用示例

### 场景选择

| 场景 | 推荐 API | 原因 |
|------|----------|------|
| 用户输入表达式 | `evaluate()` | 需要解析 + 域路由 |
| CLI/REPL/Server | `evaluate()` | 表达式字符串入口 |
| 程序化调用已知运算 | 直接 API | 跳过解析/规范化开销 |
| 嵌入式计算引擎 | 直接 API | 无需表达式解析依赖 |

### 基础调用

```rust
use calnexus::{CalNexus, BigNumber, Matrix, Vector};

let cn = CalNexus::new();

// 矩阵行列式
let m = Matrix::from_rows(&[&[1.0, 2.0], &[3.0, 4.0]]);
let r = cn.linalg().det(&m).unwrap();
assert_eq!(r.as_scalar(), Some(-2.0));

// 向量点积
let a = Vector::new(&[1.0, 2.0, 3.0]);
let b = Vector::new(&[4.0, 5.0, 6.0]);
let r = cn.linalg().dot(&a, &b).unwrap();
assert_eq!(r.as_scalar(), Some(32.0));

// 中国剩余定理：x≡2(mod 3), x≡3(mod 5), x≡2(mod 7) → x=23
let r = vec![BigNumber::from_i64(2), BigNumber::from_i64(3), BigNumber::from_i64(2)];
let m = vec![BigNumber::from_i64(3), BigNumber::from_i64(5), BigNumber::from_i64(7)];
let result = cn.scalar().crt(&r, &m).unwrap();
// result = BigInt(23)

// 线性回归
let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
let y = vec![2.0, 4.0, 6.0, 8.0, 10.0];
let r = cn.stats().lin_reg(&x, &y).unwrap();
// 返回 JSON: {"slope": 2.0, "intercept": 0.0, "r_squared": 1.0}

// 方程求解：x²-2=0，牛顿法
let r = cn.symbolic().solve_equation("x^2 - 2", "x", "newton", Some(&[1.5])).unwrap();
let root = r.as_scalar().unwrap();
assert!((root - std::f64::consts::SQRT_2).abs() < 1e-10);

// 单位换算：1000 米 → 1 千米
let r = cn.applied().convert(1000.0, "m", "km").unwrap();
assert_eq!(r.as_scalar(), Some(1.0));
```

### 自定义路由器注入

```rust
use calnexus::{evaluate_with_router, CacheManager, DomainRouter, EvalContext};

let router = DomainRouter::new(); // 空路由器（无内置域）
let cache = CacheManager::new();
let ctx = EvalContext::new();
// 空路由器下任何表达式都返回路由错误，证明路由器确实被注入消费
let err = evaluate_with_router("1+1", &ctx, None, &cache, &router).unwrap_err();
```

---

## 📚 相关文档

- [📖 用户指南](USER_GUIDE.md) — CLI 使用教程
- [🏗️ 架构文档](ARCHITECTURE.md) — 模块划分、依赖方向与数据流
- [📈 性能指南](PERFORMANCE.md) — 直接 API 与表达式路径的性能对比
- [📦 在线 API 文档](https://docs.rs/calnexus) — docs.rs 生成的 rustdoc
