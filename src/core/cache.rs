// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! L1 缓存管理器：直连 moka::sync 的进程内缓存，BLAKE3 单次哈希生成 256-bit 键。
//!
//! 设计依据（v015-comprehensive-optimization D1，替代 oxcache 封装）：
//! - ADD ADR-001：L1-only（进程内），无 L2/Redis
//! - moka::sync 直连：消除 JSON 序列化存储、临时 tokio runtime（sync_block_on）、双次哈希与 hex 分配
//! - single-flight：moka `try_get_with` per-key 并发去重；compute 错误传播给所有等待者且不缓存
//! - 容量：字节权重预算（默认 64MB，`with_capacity_bytes` 可配）；`insert` 显式跳过
//!   超过 [`MAX_CACHEABLE_BYTES`] 的大结果（大结果不入缓存），`get_or_compute` 路径由 weigher 兜底
//!
//! 核心类型：
//! - [`CacheKeyGen`]：将 `CanonicalForm` 单次 BLAKE3 哈希为 `[u8; 32]` 键
//! - [`CacheManager`]：线程安全的 L1 缓存，仅存储 `Ok(EvalResult)`，字节权重预算驱逐

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use moka::sync::Cache;

use crate::core::types::{CalcError, CanonicalForm, EvalResult};

/// 默认字节权重预算：64 MB（v015 D1）。
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
    /// 返回 `[u8; 32]`，BLAKE3 对空输入也有定义输出（Req 4 Scen 4）。
    pub fn hash(cf: &CanonicalForm) -> [u8; 32] {
        *blake3::hash(cf.as_str().as_bytes()).as_bytes()
    }
}

/// 估算 `EvalResult` 的内存占用字节数（缓存权重与准入阈值共用）。
///
/// 估算为下界近似：只计主要载荷，不追指针内层碎片；对准入决策足够。
pub fn estimate_result_bytes(r: &EvalResult) -> u64 {
    const BASE: u64 = 64;
    match r {
        EvalResult::Scalar(_) => 16,
        EvalResult::Complex(_, _) => 24,
        EvalResult::Matrix(m) => BASE + m.iter().map(|row| row.len() as u64 * 8).sum::<u64>(),
        EvalResult::Vector(v) | EvalResult::Polynomial(v) => BASE + v.len() as u64 * 8,
        EvalResult::ComplexList(v) => BASE + v.len() as u64 * 16,
        EvalResult::Steps(v) => BASE + v.iter().map(|s| s.len() as u64).sum::<u64>(),
        EvalResult::Symbolic(s) | EvalResult::LaTeX(s) | EvalResult::DateTime(s) => {
            BASE + s.len() as u64
        }
        EvalResult::Json(v) => BASE + estimate_json_bytes(v),
        EvalResult::BigInt(b) => BASE + b.bits() / 8 + 1,
        EvalResult::BigRational(q) => BASE + (q.numer().bits() + q.denom().bits()) / 8 + 2,
    }
}

/// 估算 `serde_json::Value` 的序列化字节数（递归，无分配）。
fn estimate_json_bytes(v: &serde_json::Value) -> u64 {
    match v {
        serde_json::Value::Null => 4,
        serde_json::Value::Bool(_) => 5,
        serde_json::Value::Number(_) => 16,
        serde_json::Value::String(s) => s.len() as u64 + 2,
        serde_json::Value::Array(a) => a.iter().map(estimate_json_bytes).sum::<u64>() + 2,
        serde_json::Value::Object(o) => {
            o.iter()
                .map(|(k, val)| k.len() as u64 + 4 + estimate_json_bytes(val))
                .sum::<u64>()
                + 2
        }
    }
}

/// weigher 权重：字节数截断到 u32（64MB 预算远小于 u32::MAX）。
fn result_weight(v: &Arc<EvalResult>) -> u32 {
    estimate_result_bytes(v).min(u32::MAX as u64) as u32
}

/// L1 缓存管理器。
///
/// 直连 `moka::sync`（Send + Sync），进程内有效，仅缓存 `Ok(EvalResult)`。
/// 容量为字节权重预算（默认 64MB），无时间 TTL（仅权重驱逐）。
/// 命中/未命中计数由本类型维护（moka 0.12 不内建计数器）。
pub struct CacheManager {
    inner: Cache<[u8; 32], Arc<EvalResult>>,
    hits: AtomicU64,
    misses: AtomicU64,
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
        let inner = Cache::builder()
            .max_capacity(max_bytes)
            .weigher(|_k, v: &Arc<EvalResult>| result_weight(v))
            .build();
        Self {
            inner,
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
        }
    }

    /// 查询缓存。命中返回 `EvalResult` 克隆，未命中返回 `None`。
    pub fn get(&self, cf: &CanonicalForm) -> Option<EvalResult> {
        match self.inner.get(&CacheKeyGen::hash(cf)) {
            Some(v) => {
                self.hits.fetch_add(1, Ordering::Relaxed);
                Some((*v).clone())
            }
            None => {
                self.misses.fetch_add(1, Ordering::Relaxed);
                None
            }
        }
    }

    /// 写入缓存（仅成功结果；估算超过 [`MAX_CACHEABLE_BYTES`] 的大结果跳过写入）。
    pub fn insert(&self, cf: &CanonicalForm, result: &Result<EvalResult, CalcError>) {
        if let Ok(value) = result {
            if estimate_result_bytes(value) > MAX_CACHEABLE_BYTES {
                return;
            }
            self.inner
                .insert(CacheKeyGen::hash(cf), Arc::new(value.clone()));
        }
    }

    /// 查询或计算（single-flight）。
    ///
    /// 基于 moka `try_get_with`：并发相同 key 仅 leader 执行 `compute`，
    /// 等待者共享 leader 结果；`compute` 返回 `Err` 时错误原样传播给所有
    /// 等待者（真实 CalcError 语义，无 "cache backend error" 包装）且不写缓存。
    pub fn get_or_compute<F>(&self, cf: &CanonicalForm, compute: F) -> Result<EvalResult, CalcError>
    where
        F: FnOnce() -> Result<EvalResult, CalcError>,
    {
        let ran = AtomicBool::new(false);
        let outcome = self.inner.try_get_with(CacheKeyGen::hash(cf), || {
            ran.store(true, Ordering::Relaxed);
            compute().map(Arc::new)
        });
        if ran.load(Ordering::Relaxed) {
            // leader 实际执行 compute：计一次 miss（follower 共享结果计 hit）
            self.misses.fetch_add(1, Ordering::Relaxed);
        } else {
            self.hits.fetch_add(1, Ordering::Relaxed);
        }
        match outcome {
            Ok(v) => Ok(Arc::unwrap_or_clone(v)),
            Err(e) => Err(Arc::unwrap_or_clone(e)),
        }
    }

    /// 当前缓存条目数（先同步执行 moka 待维护任务，保证读取时点准确）。
    pub fn entry_count(&self) -> u64 {
        self.inner.run_pending_tasks();
        self.inner.entry_count()
    }

    /// 缓存统计（/metrics 端点消费）。
    pub fn stats(&self) -> CacheStats {
        self.inner.run_pending_tasks();
        CacheStats {
            hits: self.hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
            entry_count: self.inner.entry_count(),
        }
    }
}

impl Default for CacheManager {
    fn default() -> Self {
        Self::new()
    }
}

// 编译期 Send + Sync 约束检查（Req 6 Scen 1）
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
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;
    use std::time::Duration;

    // 辅助函数：解析 + 规范化，返回 CanonicalForm
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

    // ===== v015 single-flight 语义（R-cache-002） =====

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
        // R-cache-002：compute 错误原样传播给所有等待者（含 follower），
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

    // ===== v015 字节权重与大结果准入（R-cache-003） =====

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

    // ===== 估算函数 =====

    #[test]
    fn test_estimate_result_bytes_variants() {
        assert_eq!(estimate_result_bytes(&EvalResult::Scalar(1.0)), 16);
        assert!(
            estimate_result_bytes(&EvalResult::Matrix(vec![vec![0.0; 100]; 100])) > 100 * 100 * 8,
            "矩阵估算应随尺寸增长"
        );
        assert!(
            estimate_result_bytes(&EvalResult::BigInt(num_bigint::BigInt::from(1u8))) > 0,
            "BigInt 估算非负"
        );
        assert!(
            estimate_result_bytes(&EvalResult::Json(serde_json::json!({"a": 1}))) > 0,
            "Json 估算非负"
        );
        // 单调性：更大的矩阵估算更大
        let small = estimate_result_bytes(&EvalResult::Matrix(vec![vec![0.0; 10]]));
        let large = estimate_result_bytes(&EvalResult::Matrix(vec![vec![0.0; 100]]));
        assert!(large > small);
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
