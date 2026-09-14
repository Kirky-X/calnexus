# 📊 CalNexus 性能指南

> 本文档给出 CalNexus 的基准测试方法、当前基线数据与性能设计说明。
> 实际性能取决于表达式复杂度与硬件，请以 `cargo bench --features cli` 实测为准。

## 📋 目录

<details open>
<summary>📑 目录</summary>

- [🎯 概述](#-概述)
- [🧪 基准测试](#-基准测试)
  - [运行基准测试](#运行基准测试)
  - [基准套件](#基准套件)
- [📈 基线数据](#-基线数据)
  - [缓存命中 vs 全流水线](#缓存命中-vs-全流水线)
  - [解析器](#解析器)
  - [规范化器](#规范化器)
- [💾 缓存设计](#-缓存设计)
- [⚙️ 优化建议](#️-优化建议)
- [📚 相关文档](#-相关文档)

</details>

---

## 🎯 概述

CalNexus 的性能设计围绕一条原则展开：**让重复求值走缓存热路径，让首次求值尽量便宜**。热路径（缓存命中）返回 `Arc<EvalResult>` 零拷贝，无 JSON 序列化、无临时 runtime；冷路径（未命中）经过 解析 → 规范化 → 域路由 → 求值 四个阶段，每个阶段都有独立的基准守护。

### 性能目标

| 目标 | 状态 |
|------|------|
| 缓存命中路径微秒级 | ✅ 简单表达式约 1.3 µs |
| single-flight 去重（并发相同键只求值一次） | ✅ `try_get_with` |
| 缓存占用有界 | ✅ 字节权重预算 + 大结果准入阈值 |
| 非确定性结果零污染 | ✅ `now`/`today`/`fx` 旁路缓存 |

---

## 🧪 基准测试

### 运行基准测试

```bash
# 运行全部基准（需 cli feature）
cargo bench --features cli

# 运行指定套件
cargo bench --features cli --bench cache_bench
cargo bench --features cli --bench parser_bench
cargo bench --features cli --bench domain_bench
cargo bench --features cli --bench api_bench

# 保存基线，供后续版本对比
cargo bench --features cli --bench cache_bench -- --save-baseline v0.1.5
```

结果输出在 `target/criterion/`，附带 HTML 报告（`target/criterion/report/index.html`）。

### 基准套件

| 套件 | 基准目标 |
|------|----------|
| `cache_bench.rs` | L1 缓存命中/未命中性能 |
| `parser_bench.rs` | 解析器吞吐、大表达式（4096 字符）、规范化器 |
| `domain_bench.rs` | 域路由 + 求值、批量 1000 表达式、素数判定 |
| `api_bench.rs` | 直接 API vs 表达式路径对比 |

---

## 📈 基线数据

> 口径：基线在开发机（WSL2、linux 6.6）本地采集，数值为 criterion 区间估计的 median，随版本演进会漂移；本文档在基准更新后同步。

### 缓存命中 vs 全流水线

| 用例 | 耗时 | 说明 |
|------|------|------|
| `cache_hit / 2+3` | 约 1.29 µs | 纯缓存读路径（BLAKE3 键 + Arc 返回） |
| `cache_hit / 100!` | 约 1.8 µs | 命中成本与表达式内容无关 |
| `cache_hit / sin(1.5)+cos(0.5)` | 约 3.4 µs | |
| `cache_miss / 2+3` | 约 13.1 µs | 解析 + 规范化 + 求值 + 入缓存全流水线 |
| `cache_miss / sin(1.5)+cos(0.5)` | 约 15.3 µs | |
| `cache_miss / diff(x^2, x)` | 约 15.7 µs | 符号微积分 |
| `cache_miss / matrix([[1,2],[3,4]])` | 约 15.9 µs | 矩阵字面量 |

缓存命中相对全流水线约 **10 倍**提升；命中成本稳定在微秒级且与表达式复杂度解耦（键为规范化形式的 BLAKE3 哈希）。

### 解析器

| 用例 | 耗时 |
|------|------|
| `2+3` | 约 286 ns |
| `(2+9)*7-6` | 约 712 ns |
| `1+2*3-4*5*6` | 约 740 ns |
| `diff(x^2, x)` | 约 945 ns |
| `sin(x)+cos(y)` | 约 1.1 µs |
| `sum([1,2,3,4,5])` | 约 2.0 µs |
| `matrix([[1,2],[3,4]])` | 约 2.3 µs |

### 规范化器

| 用例 | 耗时 |
|------|------|
| `2+3` | 约 45 ns |
| `x+y` | 约 112 ns |
| `sin(x)+cos(x)` | 约 279 ns |
| `x^2+2*x+1` | 约 417 ns |
| `a+b+c+d+e` | 约 427 ns |

规范化是缓存键正确性的前提（`x+y` 与 `y+x` 规范为同一形式），成本为亚微秒级，占比可忽略。

---

## 💾 缓存设计

`CacheManager` 直连 `moka::sync`（v0.1.5 起替代 oxcache→tokio 封装链）：

- **零拷贝命中**：缓存存储 `Arc<EvalResult>`，命中路径无序列化、无克隆、无临时 tokio runtime。
- **single-flight**：`try_get_with` 保证并发相同键只真实求值一次，其余等待方共享同一 `Arc`；求值错误原样传播（不缓存错误）。
- **单次哈希键**：BLAKE3 对规范化 S-表达式一次哈希；`--precision` 与变量上下文参与键构建（精度感知）。
- **字节权重预算**：默认 64MB，`--cache-size` 以条目数配置（× 4KB 近似折算）。
- **大结果准入阈值**：大于 256KB 的结果不入缓存，防止单条目挤占预算。
- **非确定性旁路**：`now` / `today` / `fx` / `fx_rate` 在路由前检测，命中则同时跳过 `get` 与 `insert`，避免时间/汇率结果被缓存污染。检测开销为 O(AST 节点数) 的 HashSet 查询。

---

## ⚙️ 优化建议

### 服务端：预算与超时

- 容器部署时按内存预算设置 `CALNEXUS_CACHE_SIZE`（条目 × 4KB 近似）；求值为 CPU 密集型，副本数按核数扩展优于调大缓存。
- 为求值设置 `--timeout` / `CALNEXUS_TIMEOUT` 兜底；`Timeout` 与上游故障映射 503，不占用缓存。

### 批量：利用并行与缓存

- `--batch` 已用 rayon 并行；重复表达式自动经 single-flight 合并为一次求值。
- 同一文件中语义相同的表达式（如 `x+y` 与 `y+x`）经规范化后共享缓存条目。

### 库集成：选对入口

- 已知运算的程序化调用用**直接 API**（`CalNexus` 门面），跳过解析/规范化开销；`api_bench` 套件可对比两条路径。
- 用户输入或表达式字符串场景用 `evaluate()`，享受域路由与缓存。
- 下游注入自定义域时用 `evaluate_with_router`，避免替换全局路由器。

### 复现与回归

- 升级版本后建议 `--save-baseline` 保存基线，用 `--baseline` 对比回归。

---

## 📚 相关文档

- [🏗️ 架构文档](ARCHITECTURE.md) — 缓存与求值流水线的设计细节
- [📘 API 参考](API_REFERENCE.md) — 表达式 API 与直接 API
