// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! L1 缓存管理器：oxcache 同步字节权重缓存（`byte-weight` feature）的领域封装，
//! BLAKE3 单次哈希生成 256-bit 键。
//!
//! 分层（ADD ADR-001：L1-only 进程内，无 L2/Redis）：
//! - 机制层 `oxcache::sync::ByteWeightCache`：字节权重预算驱逐、single-flight
//!   （moka `try_get_with` per-key 并发去重，compute 错误传播给所有等待者且不缓存）、
//!   命中计数（leader 计 miss / follower 计 hit）
//! - 领域层（本模块）：`CanonicalForm` → BLAKE3 256-bit 键（无 hex 中间分配）、
//!   `EvalResult` 体积估算（权重与准入共用；口径=oxcache UnifiedSerializer 的
//!   JSON 序列化字节，失败回退常量 64——weigher 每次插入触发一次完整序列化）、
//!   仅缓存 `Ok(EvalResult)` 语义
//!
//! 核心类型：
//! - [`CacheKeyGen`]：将 `CanonicalForm` 单次 BLAKE3 哈希为 `[u8; 32]` 键
//! - [`CacheManager`]：线程安全的 L1 缓存，仅存储 `Ok(EvalResult)`，字节权重预算驱逐

use oxcache::sync::ByteWeightCache;

use crate::core::types::{CalcError, CanonicalForm, EvalResult};

/// 默认字节权重预算：64 MB。
pub const DEFAULT_MAX_WEIGHT_BYTES: u64 = 64 * 1024 * 1024;

/// 单条结果准入阈值：估算超过此值（256 KB）的结果不入缓存。
///
/// 历史背景：oxcache 时代缓存按条目数计（10000 条），单条 BigRational 结果可达
/// ~800KB，理论上限 8GB 内存放大；字节权重预算 + 单条准入阈值双重防护。
pub const MAX_CACHEABLE_BYTES: u64 = 256 * 1024;

/// 缓存统计快照（/metrics 端点消费）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CacheStats {
    /// 命中次数。
    pub hits: u64,
    /// 未命中次数。
    pub misses: u64,
    /// 当前条目数（moka 最终一致值）。
    pub entry_count: u64,
}

/// 缓存键生成器：使用 BLAKE3 对 `CanonicalForm` 的 S-表达式字符串单次哈希。
///
/// 生成 256-bit（32 字节）键，直接作为 moka 缓存键（无 hex String 中间分配）。
pub struct CacheKeyGen;

impl CacheKeyGen {
    /// 对 `CanonicalForm` 生成 256-bit BLAKE3 哈希键。
    ///
    /// 返回 `[u8; 32]`，BLAKE3 对空输入也有定义输出。
    pub fn hash(cf: &CanonicalForm) -> [u8; 32] {
        *blake3::hash(cf.as_str().as_bytes()).as_bytes()
    }
}

/// 序列化失败时的 weigher 回退常量（字节）。
pub const WEIGHT_FALLBACK_BYTES: u64 = 64;

/// 进程级序列化器（weigher 与准入估算共用；`OnceLock` 惰性单例）。
fn serializer() -> &'static oxcache::infra::serialization::UnifiedSerializer {
    static SERIALIZER: std::sync::OnceLock<oxcache::infra::serialization::UnifiedSerializer> =
        std::sync::OnceLock::new();
    SERIALIZER.get_or_init(oxcache::infra::serialization::UnifiedSerializer::new)
}

/// 估算结果 → 权重的映射：`Ok(n)` 取 `n`；`Err` 回退 [`WEIGHT_FALLBACK_BYTES`]。
/// 序列化失败只记一次 debug 日志（weigher 在插入热路径，失败通常系统性——
/// 逐次刷屏无诊断增量）。
fn map_estimate(size: oxcache::OxCacheResult<usize>) -> u64 {
    match size {
        Ok(n) => n as u64,
        Err(e) => {
            // 仅 observability feature 下有 tracing；其余构建静默回退
            // （weigher 热路径不做 IO 决策）。OnceLock 保证只记一次。
            static WARNED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
            if WARNED.set(()).is_ok() {
                #[cfg(feature = "observability")]
                tracing::debug!(
                    error = %e,
                    fallback = WEIGHT_FALLBACK_BYTES,
                    "cache weigher 序列化失败，回退常量权重"
                );
                #[cfg(not(feature = "observability"))]
                let _ = e;
            }
            WEIGHT_FALLBACK_BYTES
        }
    }
}

/// 估算 `EvalResult` 的缓存权重（字节）。
///
/// 口径=oxcache `UnifiedSerializer` 的 JSON 序列化字节数（与缓存存储形态
/// 一致，不再是独立的下界近似）；序列化失败回退 [`WEIGHT_FALLBACK_BYTES`]。
/// 代价：每次插入触发一次完整序列化（weigher  semantics），已在
/// `--cache-size` help 与模块文档注明。
pub fn estimate_result_bytes(r: &EvalResult) -> u64 {
    map_estimate(serializer().estimate_size(r))
}

/// L1 缓存管理器。
///
/// 封装 `oxcache::sync::ByteWeightCache`（Send + Sync），进程内有效，
/// 仅缓存 `Ok(EvalResult)`。容量为字节权重预算（默认 64MB），无时间 TTL
/// （仅权重驱逐）；命中/未命中计数由 oxcache 层维护。
pub struct CacheManager {
    inner: ByteWeightCache<[u8; 32], EvalResult, fn(&EvalResult) -> u64>,
}

impl CacheManager {
    /// 创建默认配置的缓存管理器（字节预算 64MB）。
    pub fn new() -> Self {
        Self::with_capacity_bytes(DEFAULT_MAX_WEIGHT_BYTES)
    }

    /// 创建指定字节权重预算的缓存管理器。
    ///
    /// CLI `--cache-size N` 按平均 4KB/条换算为 `N * 4096` 字节预算
    /// （moka weigher 模型下容量单位为权重字节，help 文档注明近似语义）。
    pub fn with_capacity_bytes(max_bytes: u64) -> Self {
        Self {
            inner: ByteWeightCache::new(max_bytes, estimate_result_bytes as fn(&EvalResult) -> u64)
                .with_max_entry_bytes(MAX_CACHEABLE_BYTES),
        }
    }

    /// 查询缓存。命中返回 `EvalResult` 克隆，未命中返回 `None`。
    pub fn get(&self, cf: &CanonicalForm) -> Option<EvalResult> {
        self.inner.get(&CacheKeyGen::hash(cf))
    }

    /// 写入缓存（仅成功结果；估算超过 `MAX_CACHEABLE_BYTES` 的大结果跳过写入）。
    pub fn insert(&self, cf: &CanonicalForm, result: &Result<EvalResult, CalcError>) {
        if let Ok(value) = result {
            self.inner.insert(CacheKeyGen::hash(cf), value.clone());
        }
    }

    /// 查询或计算（single-flight）。
    ///
    /// 基于 oxcache 层的 moka `try_get_with`：并发相同 key 仅 leader 执行
    /// `compute`，等待者共享 leader 结果；`compute` 返回 `Err` 时错误原样
    /// 传播给所有等待者（真实 CalcError 语义，无 "cache backend error" 包装）
    /// 且不写缓存。
    pub fn get_or_compute<F>(&self, cf: &CanonicalForm, compute: F) -> Result<EvalResult, CalcError>
    where
        F: FnOnce() -> Result<EvalResult, CalcError>,
    {
        self.inner.get_or_compute(CacheKeyGen::hash(cf), compute)
    }

    /// 当前缓存条目数（先同步执行 moka 待维护任务，保证读取时点准确）。
    pub fn entry_count(&self) -> u64 {
        self.inner.entry_count()
    }

    /// 缓存统计（/metrics 端点消费）。
    pub fn stats(&self) -> CacheStats {
        let stats = self.inner.stats();
        CacheStats {
            hits: stats.hits,
            misses: stats.misses,
            entry_count: stats.entry_count,
        }
    }
}

impl Default for CacheManager {
    fn default() -> Self {
        Self::new()
    }
}

// 编译期 Send + Sync 约束检查
// coverage 运行时排除：const fn 在编译期执行，无法被行覆盖
#[cfg(not(coverage))]
const _: () = {
    const fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<CacheManager>();
    assert_send_sync::<CacheKeyGen>();
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::canonicalizer::AstCanonicalizer;
    use crate::core::parser::parse;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;
    use std::time::Duration;

    fn canon(input: &str) -> CanonicalForm {
        let ast = parse(input).unwrap();
        let (_, cf) = AstCanonicalizer::canonicalize(&ast).unwrap();
        cf
    }

    // ===== Requirement 1: 缓存命中 =====

    #[test]
    fn test_cache_hit_returns_cached_value() {
        let cache = CacheManager::new();
        let cf = CanonicalForm::new("(+ 2 3)");
        cache.insert(&cf, &Ok(EvalResult::Scalar(5.0)));

        let hit = cache.get(&cf);
        assert_eq!(hit, Some(EvalResult::Scalar(5.0)));
    }

    #[test]
    fn test_cache_hit_returns_clone_not_reference() {
        let cache = CacheManager::new();
        let cf = CanonicalForm::new("test");
        cache.insert(&cf, &Ok(EvalResult::Scalar(42.0)));

        let _hit1 = cache.get(&cf).unwrap();
        let hit2 = cache.get(&cf).unwrap();
        assert_eq!(hit2, EvalResult::Scalar(42.0));
    }

    // ===== Requirement 2: 缓存未命中 =====

    #[test]
    fn test_cache_miss_returns_none() {
        let cache = CacheManager::new();
        let cf = CanonicalForm::new("(+ 1 2)");
        assert_eq!(cache.get(&cf), None);
    }

    #[test]
    fn test_insert_then_immediate_hit() {
        let cache = CacheManager::new();
        let cf = CanonicalForm::new("6*7");
        cache.insert(&cf, &Ok(EvalResult::Scalar(42.0)));

        let hit = cache.get(&cf);
        assert_eq!(hit, Some(EvalResult::Scalar(42.0)));
    }

    #[test]
    fn test_get_or_compute_calls_compute_on_miss() {
        let cache = CacheManager::new();
        let cf = canon("6*7");

        let call_count = Arc::new(AtomicUsize::new(0));
        let cc = Arc::clone(&call_count);

        let result = cache.get_or_compute(&cf, || {
            cc.fetch_add(1, Ordering::SeqCst);
            Ok(EvalResult::Scalar(42.0))
        });

        assert_eq!(result.unwrap(), EvalResult::Scalar(42.0));
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    // ===== Requirement 3: 等价表达式缓存去重 =====

    #[test]
    fn test_commutative_equivalent_share_cache() {
        let cache = CacheManager::new();
        let cf_2plus3 = canon("2+3");
        let cf_3plus2 = canon("3+2");
        assert_eq!(cf_2plus3, cf_3plus2, "规范形式应相同");

        cache.insert(&cf_2plus3, &Ok(EvalResult::Scalar(5.0)));
        assert_eq!(cache.get(&cf_3plus2), Some(EvalResult::Scalar(5.0)));
    }

    #[test]
    fn test_constant_folding_equivalent_share_cache() {
        let cache = CacheManager::new();
        let cf_1 = canon("2*3+1");
        let cf_2 = canon("1+6");
        assert_eq!(cf_1, cf_2, "规范形式应相同");

        cache.insert(&cf_1, &Ok(EvalResult::Scalar(7.0)));
        assert_eq!(cache.get(&cf_2), Some(EvalResult::Scalar(7.0)));
    }

    #[test]
    fn test_non_equivalent_do_not_share_cache() {
        let cache = CacheManager::new();
        let cf_2minus3 = canon("2-3");
        let cf_3minus2 = canon("3-2");
        assert_ne!(cf_2minus3, cf_3minus2, "规范形式应不同");

        cache.insert(&cf_2minus3, &Ok(EvalResult::Scalar(-1.0)));
        assert_eq!(cache.get(&cf_3minus2), None, "不同规范形式不应命中");
    }

    // ===== Requirement 4: 缓存键生成 =====

    #[test]
    fn test_same_canonical_form_same_key() {
        let cf1 = CanonicalForm::new("(+ 2 3)");
        let cf2 = CanonicalForm::new("(+ 2 3)");
        assert_eq!(CacheKeyGen::hash(&cf1), CacheKeyGen::hash(&cf2));
    }

    #[test]
    fn test_different_canonical_form_different_key() {
        let cf1 = CanonicalForm::new("(+ 2 3)");
        let cf2 = CanonicalForm::new("(* 2 3)");
        assert_ne!(CacheKeyGen::hash(&cf1), CacheKeyGen::hash(&cf2));
    }

    #[test]
    fn test_key_length_is_32_bytes() {
        let cf = CanonicalForm::new("(+ 2 3)");
        let key = CacheKeyGen::hash(&cf);
        assert_eq!(key.len(), 32);
    }

    #[test]
    fn test_empty_string_generates_key() {
        let cf = CanonicalForm::new("");
        let key = CacheKeyGen::hash(&cf);
        assert_eq!(key.len(), 32);
        assert!(key.iter().any(|&b| b != 0));
    }

    // ===== Requirement 5: 缓存 TTL 等于进程生命周期 =====

    #[test]
    fn test_cache_persists_within_process() {
        let cache = CacheManager::new();
        let cf = CanonicalForm::new("(+ 1 1)");
        cache.insert(&cf, &Ok(EvalResult::Scalar(2.0)));

        for _ in 0..10 {
            assert_eq!(cache.get(&cf), Some(EvalResult::Scalar(2.0)));
        }
    }

    #[test]
    fn test_no_time_based_eviction() {
        let cache = CacheManager::new();
        let cf = CanonicalForm::new("(+ 1 1)");
        cache.insert(&cf, &Ok(EvalResult::Scalar(2.0)));

        thread::sleep(Duration::from_millis(50));

        assert_eq!(cache.get(&cf), Some(EvalResult::Scalar(2.0)));
    }

    // ===== Requirement 6: 缓存线程安全 =====

    #[test]
    fn test_cache_manager_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>(_: &T) {}
        let cache = CacheManager::new();
        assert_send_sync(&cache);
    }

    #[test]
    fn test_concurrent_read_hits() {
        let cache = Arc::new(CacheManager::new());
        let cf = Arc::new(CanonicalForm::new("(+ 2 3)"));
        cache.insert(&cf, &Ok(EvalResult::Scalar(5.0)));

        let mut handles = vec![];
        for _ in 0..8 {
            let cache = Arc::clone(&cache);
            let cf = Arc::clone(&cf);
            handles.push(thread::spawn(move || {
                for _ in 0..1000 {
                    assert_eq!(cache.get(&cf), Some(EvalResult::Scalar(5.0)));
                }
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
    }

    #[test]
    fn test_concurrent_writes_no_conflict() {
        let cache = Arc::new(CacheManager::new());

        let mut handles = vec![];
        for i in 0..8u64 {
            let cache = Arc::clone(&cache);
            handles.push(thread::spawn(move || {
                let cf = CanonicalForm::new(format!("(+ {} {})", i, i).as_str());
                let result = EvalResult::Scalar((i * 2) as f64);
                cache.insert(&cf, &Ok(result.clone()));
                assert_eq!(cache.get(&cf), Some(result));
            }));
        }
        for h in handles {
            h.join().unwrap();
        }

        for i in 0..8u64 {
            let cf = CanonicalForm::new(format!("(+ {} {})", i, i).as_str());
            let expected = EvalResult::Scalar((i * 2) as f64);
            assert_eq!(
                cache.get(&cf),
                Some(expected),
                "线程 {} 的缓存条目应可读",
                i
            );
        }
    }

    // ===== Requirement 7: 缓存不存储错误结果 =====

    #[test]
    fn test_error_result_not_cached() {
        let cache = CacheManager::new();
        let cf = CanonicalForm::new("1/0");

        let result = cache.get_or_compute(&cf, || Err(CalcError::division_by_zero()));

        assert!(result.is_err());
        assert_eq!(cache.get(&cf), None, "错误结果不应写入缓存");
    }

    #[test]
    fn test_nan_error_not_cached() {
        let cache = CacheManager::new();
        let cf = CanonicalForm::new("sqrt(-1)");

        let result = cache.get_or_compute(&cf, || Err(CalcError::nan_or_inf()));

        assert!(result.is_err());
        assert_eq!(cache.get(&cf), None, "NaN 错误不应写入缓存");
    }

    #[test]
    fn test_only_success_cached() {
        let cache = CacheManager::new();
        let cf_ok = CanonicalForm::new("(+ 1 2)");
        let cf_err = CanonicalForm::new("(1/0)");

        cache.insert(&cf_ok, &Ok(EvalResult::Scalar(3.0)));
        cache.insert(&cf_err, &Err(CalcError::division_by_zero()));

        assert_eq!(cache.get(&cf_ok), Some(EvalResult::Scalar(3.0)));
        assert_eq!(cache.get(&cf_err), None, "错误结果不应写入");
    }

    // ===== get_or_compute 完整流程 =====

    #[test]
    fn test_get_or_compute_hits_cache_on_second_call() {
        let cache = CacheManager::new();
        let cf = canon("2+3");

        let call_count = Arc::new(AtomicUsize::new(0));

        let cc = Arc::clone(&call_count);
        let r1 = cache.get_or_compute(&cf, || {
            cc.fetch_add(1, Ordering::SeqCst);
            Ok(EvalResult::Scalar(5.0))
        });
        assert_eq!(r1.unwrap(), EvalResult::Scalar(5.0));
        assert_eq!(call_count.load(Ordering::SeqCst), 1);

        let r2 = cache.get_or_compute(&cf, || Ok(EvalResult::Scalar(999.0)));
        assert_eq!(r2.unwrap(), EvalResult::Scalar(5.0), "应返回缓存值");
        assert_eq!(call_count.load(Ordering::SeqCst), 1, "compute 不应被调用");
    }

    // ===== entry_count / Default 覆盖 =====

    #[test]
    fn test_entry_count_zero_on_empty_cache() {
        let cache = CacheManager::new();
        assert_eq!(cache.entry_count(), 0, "fresh cache should have 0 entries");
    }

    #[test]
    fn test_entry_count_increases_after_insert() {
        let cache = CacheManager::new();
        let cf = CanonicalForm::new("(+ 1 2)");
        cache.insert(&cf, &Ok(EvalResult::Scalar(3.0)));
        assert_eq!(cache.entry_count(), 1);
    }

    #[test]
    fn test_default_creates_working_cache() {
        let cache = CacheManager::default();
        let cf = CanonicalForm::new("(+ 5 7)");
        assert_eq!(cache.get(&cf), None);
        cache.insert(&cf, &Ok(EvalResult::Scalar(12.0)));
        assert_eq!(cache.get(&cf), Some(EvalResult::Scalar(12.0)));
    }

    // ===== single-flight 语义 =====

    #[test]
    fn test_get_or_compute_single_flight_dedup_exact_one() {
        // 并发相同 key 恰执行一次 compute（moka try_get_with per-key 去重保证）
        let cache = Arc::new(CacheManager::new());
        let cf = canon("42+58");

        let compute_count = Arc::new(AtomicUsize::new(0));
        let barrier = Arc::new(std::sync::Barrier::new(10));

        let mut handles = vec![];
        for _ in 0..10 {
            let cache = Arc::clone(&cache);
            let cf = cf.clone();
            let count = Arc::clone(&compute_count);
            let barrier = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                barrier.wait();
                cache.get_or_compute(&cf, || {
                    count.fetch_add(1, Ordering::SeqCst);
                    thread::sleep(Duration::from_millis(20));
                    Ok(EvalResult::Scalar(100.0))
                })
            }));
        }

        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        for r in &results {
            assert_eq!(r.as_ref().unwrap(), &EvalResult::Scalar(100.0));
        }
        assert_eq!(
            compute_count.load(Ordering::SeqCst),
            1,
            "single-flight 去重：compute 应恰执行 1 次"
        );
    }

    #[test]
    fn test_follower_receives_real_error_not_backend_error() {
        // compute 错误原样传播给所有等待者（含 follower），
        // 不得出现 oxcache 时代的 "cache backend error" 包装。
        let cache = Arc::new(CacheManager::new());
        let cf = canon("err-path/0");

        let barrier = Arc::new(std::sync::Barrier::new(4));
        let mut handles = vec![];
        for _ in 0..4 {
            let cache = Arc::clone(&cache);
            let cf = cf.clone();
            let barrier = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                barrier.wait();
                cache.get_or_compute(&cf, || {
                    thread::sleep(Duration::from_millis(20));
                    Err(CalcError::division_by_zero())
                })
            }));
        }

        for h in handles {
            let r = h.join().unwrap();
            let err = r.unwrap_err();
            assert_eq!(err.message, "division by zero");
            assert!(!err.message.contains("cache backend error"));
            assert_eq!(err.kind, crate::core::types::ErrorKind::DivisionByZero);
        }
        // 错误不写入缓存
        assert_eq!(cache.get(&cf), None);
    }

    // ===== 字节权重与大结果准入 =====

    #[test]
    fn test_large_result_not_cached_but_returned() {
        // 超过 MAX_CACHEABLE_BYTES（256KB）的结果跳过写入，但求值结果原样返回
        let cache = CacheManager::new();
        let cf = CanonicalForm::new("(big-matrix)");
        // 512×512 f64 ≈ 2MB > 256KB
        let big = EvalResult::Matrix(vec![vec![0.0f64; 512]; 512]);

        cache.insert(&cf, &Ok(big.clone()));
        assert_eq!(cache.entry_count(), 0, "大结果不应写入缓存（准入阈值策略）");

        // get_or_compute 路径：由 weigher 字节预算兜底驱逐，但返回值不受影响
        let big2 = big.clone();
        let r = cache.get_or_compute(&cf, || Ok(big2)).unwrap();
        assert!(matches!(r, EvalResult::Matrix(_)));
    }

    #[test]
    fn test_byte_weight_budget_bounds_entries() {
        // 4KB 预算 + 每条 8KB 权重的条目 → 至多容纳 1 条
        let cache = CacheManager::with_capacity_bytes(4096);
        for i in 0..6 {
            let cf = CanonicalForm::new(format!("(heavy {})", i).as_str());
            let heavy = EvalResult::Matrix(vec![vec![0.0f64; 1024]; 1]); // ~8KB + BASE
            cache.insert(&cf, &Ok(heavy));
        }
        assert!(
            cache.entry_count() <= 2,
            "字节预算应限制条目数（8KB 条目 / 4KB 预算），实际 {}",
            cache.entry_count()
        );
    }

    #[test]
    fn test_stats_reports_hits_and_misses() {
        let cache = CacheManager::new();
        let cf = CanonicalForm::new("(stats-test 1)");
        let _ = cache.get(&cf); // miss
        cache.insert(&cf, &Ok(EvalResult::Scalar(1.0)));
        let _ = cache.get(&cf); // hit

        let stats = cache.stats();
        assert!(stats.misses >= 1, "应记录至少一次 miss");
        assert!(stats.hits >= 1, "应记录至少一次 hit");
        assert_eq!(stats.entry_count, cache.entry_count());
    }

    // ===== 估算函数（口径：oxcache UnifiedSerializer JSON 序列化字节） =====

    #[test]
    fn test_estimate_result_bytes_variants_positive() {
        // 每个 EvalResult 变体估重 > 0（序列化字节口径）
        let cases = [
            EvalResult::Scalar(1.0),
            EvalResult::Complex(1.0, 2.0),
            EvalResult::Matrix(vec![vec![0.0; 4]; 4]),
            EvalResult::Vector(vec![1.0, 2.0]),
            EvalResult::Polynomial(vec![1.0, 0.0, 3.5]),
            EvalResult::ComplexList(vec![(1.0, 2.0)]),
            EvalResult::Steps(vec!["a".to_string()]),
            EvalResult::Symbolic("x+1".to_string()),
            EvalResult::LaTeX("\\frac{1}{2}".to_string()),
            EvalResult::DateTime("2026-10-09T00:00:00Z".to_string()),
            EvalResult::Json(serde_json::json!({"a": 1})),
            EvalResult::BigInt(num_bigint::BigInt::from(1u8)),
            EvalResult::BigRational(num_rational::BigRational::new(1.into(), 2.into())),
        ];
        for case in &cases {
            let w = estimate_result_bytes(case);
            assert!(w > 0, "变体 {:?} 估重应 > 0，实际 {}", case, w);
        }
    }

    #[test]
    fn test_estimate_monotonic_with_payload() {
        // 载荷增大 → 估重单调不减（序列化字节口径，Scalar→Json 字符串变长）
        let small = estimate_result_bytes(&EvalResult::Json(serde_json::json!({"s": "x"})));
        let large = estimate_result_bytes(&EvalResult::Json(
            serde_json::json!({"s": "x".repeat(1024)}),
        ));
        assert!(
            large >= small,
            "载荷增大估重不应减少：small={} large={}",
            small,
            large
        );
        let m_small = estimate_result_bytes(&EvalResult::Matrix(vec![vec![0.0; 10]]));
        let m_large = estimate_result_bytes(&EvalResult::Matrix(vec![vec![0.0; 100]]));
        assert!(m_large > m_small, "矩阵增大估重应增长");
    }

    #[test]
    fn test_map_estimate_failure_falls_back_to_constant() {
        // 序列化失败路径：Err → 回退常量 64，不 panic（纯映射单测）
        let err = oxcache::OxCacheError::InvalidInput("injected".to_string());
        assert_eq!(map_estimate(Err(err)), WEIGHT_FALLBACK_BYTES);
        assert_eq!(map_estimate(Ok(128)), 128);
    }

    // ===== proptest 属性测试 =====

    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

        #[test]
        fn prop_commutative_expressions_share_cache(
            a in (0u8..26u8).prop_map(|i| ((b'a' + i) as char).to_string()),
            b in (0u8..26u8).prop_map(|i| ((b'a' + i) as char).to_string())
        ) {
            let cache = CacheManager::new();
            let cf_ab = canon(&format!("{}+{}", a, b));
            let cf_ba = canon(&format!("{}+{}", b, a));
            prop_assert_eq!(&cf_ab, &cf_ba, "规范形式应相同");

            cache.insert(&cf_ab, &Ok(EvalResult::Scalar(5.0)));
            prop_assert_eq!(cache.get(&cf_ba), Some(EvalResult::Scalar(5.0)));
        }

        #[test]
        fn prop_cache_hit_repeatable(
            x in (1u8..100u8).prop_map(|i| format!("{}+{}", i, i+1))
        ) {
            let cache = CacheManager::new();
            let cf = canon(&x);
            cache.insert(&cf, &Ok(EvalResult::Scalar(42.0)));
            prop_assert_eq!(cache.get(&cf), Some(EvalResult::Scalar(42.0)));
            prop_assert_eq!(cache.get(&cf), Some(EvalResult::Scalar(42.0)));
        }

        #[test]
        fn prop_distinct_expressions_distinct_cache(
            a in (1u8..50u8).prop_map(|i| format!("{}*2", i)),
            b in (51u8..100u8).prop_map(|i| format!("{}*2", i))
        ) {
            let cache = CacheManager::new();
            let cf_a = canon(&a);
            let cf_b = canon(&b);
            if cf_a != cf_b {
                cache.insert(&cf_a, &Ok(EvalResult::Scalar(1.0)));
                prop_assert_eq!(cache.get(&cf_b), None, "不同表达式不应命中");
            }
        }
    }
}
