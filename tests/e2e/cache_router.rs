// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! S6 缓存与路由 —— CacheManager 字节权重淘汰/统计、CacheKeyGen、
//! DomainRouter 优先级/去重/不确定性、evaluate 缓存命中契约。
//!
//! 既有套件仅覆盖基础 get/insert/命中；本模块补齐
//! `with_capacity_bytes` 淘汰语义、`stats()`、`MAX_CACHEABLE_BYTES`
//! 大结果旁路与路由器结构契约（此前零覆盖）。

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use calnexus::{
    AstNode, CacheKeyGen, CacheManager, CalculationDomain, CanonicalForm, DomainRouter,
    EvalContext, EvalResult, evaluate, evaluate_with_router,
};

use crate::common::{approx_eq, eval, eval_err};

fn canonical(expr: &str) -> CanonicalForm {
    let ast = calnexus::parse(expr).unwrap();
    let (_, cf) = calnexus::AstCanonicalizer::canonicalize(&ast).unwrap();
    cf
}

// ---------------------------------------------------------------------------
// CacheManager 基础与统计
// ---------------------------------------------------------------------------

#[test]
fn cache_insert_get_and_entry_count() {
    let cache = CacheManager::new();
    // 规范化会常量折叠："1+2" 与 "3+0" 折叠为同一规范形式 "3"（缓存去重点）
    let cf = canonical("1+2");
    assert_eq!(cache.entry_count(), 0);
    cache.insert(&cf, &Ok(EvalResult::Scalar(3.0)));
    assert_eq!(cache.entry_count(), 1);
    assert_eq!(cache.get(&cf), Some(EvalResult::Scalar(3.0)));
    // 折叠等价写法命中同一条目；不等价表达式（含变量，不折叠）缺失
    assert_eq!(cache.get(&canonical("3+0")), Some(EvalResult::Scalar(3.0)));
    assert_eq!(cache.get(&canonical("x+1")), None);
}

#[test]
fn cache_errors_never_stored() {
    let cache = CacheManager::new();
    // 用变量表达式避开规范化期的常量折叠（"1/0" 在折叠期即报错，
    // 根本到不了缓存写入点——该行为由 pipeline_canonical_folds_constants 钉住）
    let cf = canonical("1/x");
    cache.insert(&cf, &Err(calnexus::CalcError::division_by_zero()));
    assert_eq!(cache.entry_count(), 0, "error results must not be cached");
    assert_eq!(cache.get(&cf), None);
}

#[test]
fn cache_stats_counts_hits_and_misses() {
    let cache = CacheManager::new();
    let cf = canonical("2*3");
    let _ = cache.get(&cf); // miss
    cache.insert(&cf, &Ok(EvalResult::Scalar(6.0)));
    let _ = cache.get(&cf); // hit
    let _ = cache.get(&cf); // hit
    let stats = cache.stats();
    assert!(stats.misses >= 1);
    assert!(stats.hits >= 2);
    assert_eq!(stats.entry_count, cache.entry_count());
}

#[test]
fn cache_get_or_compute_single_compute() {
    let cache = CacheManager::new();
    let cf = canonical("10+5");
    let calls = Arc::new(AtomicUsize::new(0));
    let c2 = Arc::clone(&calls);
    let v1 = cache
        .get_or_compute(&cf, || {
            c2.fetch_add(1, Ordering::SeqCst);
            Ok(EvalResult::Scalar(15.0))
        })
        .unwrap();
    let v2 = cache
        .get_or_compute(&cf, || {
            c2.fetch_add(1, Ordering::SeqCst);
            Ok(EvalResult::Scalar(15.0))
        })
        .unwrap();
    assert!(approx_eq(
        match v1 {
            EvalResult::Scalar(x) => x,
            _ => panic!(),
        },
        15.0
    ));
    assert_eq!(v1, v2);
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "second call must hit cache"
    );
}

#[test]
fn cache_capacity_eviction_bounds_entries() {
    // Scalar 结果估算 16 字节：160 字节预算至多容纳 ~10 条
    let cache = CacheManager::with_capacity_bytes(160);
    for i in 0..50 {
        let cf = canonical(&format!("{i}+0"));
        cache.insert(&cf, &Ok(EvalResult::Scalar(i as f64)));
    }
    let count = cache.entry_count();
    assert!(count < 50, "byte-weight budget must evict, got {count}");
    assert!(count >= 1, "eviction must not empty the cache");
    // 大预算不淘汰
    let cache = CacheManager::with_capacity_bytes(64 * 1024 * 1024);
    for i in 0..50 {
        let cf = canonical(&format!("{i}+0"));
        cache.insert(&cf, &Ok(EvalResult::Scalar(i as f64)));
    }
    assert_eq!(cache.entry_count(), 50);
}

#[test]
fn cache_oversized_results_bypass_storage() {
    // MAX_CACHEABLE_BYTES = 256KB：320_064 字节的 Vector 超限 → 返回但不存
    let cache = CacheManager::new();
    let big = EvalResult::Vector(vec![0.0; 40_000]);
    let cf = CanonicalForm::new("big-vector-fixture");
    cache.insert(&cf, &Ok(big.clone()));
    assert_eq!(cache.entry_count(), 0, "oversized result must bypass L1");
    // get_or_compute 契约（oxcache 文档化行为）：单条准入阈值只约束
    // insert；try_get_with 路径对成功结果总是入缓存（内存上界由总权重
    // 预算的淘汰兜底），返回值不受阈值影响
    let calls = Arc::new(AtomicUsize::new(0));
    let c2 = Arc::clone(&calls);
    let v = cache
        .get_or_compute(&cf, || {
            c2.fetch_add(1, Ordering::SeqCst);
            Ok(big.clone())
        })
        .unwrap();
    assert_eq!(v, big);
    let v2 = cache
        .get_or_compute(&cf, || {
            c2.fetch_add(1, Ordering::SeqCst);
            Ok(big.clone())
        })
        .unwrap();
    assert_eq!(v2, big);
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "get_or_compute caches oversized results (total-budget bounded)"
    );
}

// ---------------------------------------------------------------------------
// CacheKeyGen
// ---------------------------------------------------------------------------

#[test]
fn cache_key_gen_stable_and_discriminating() {
    let h1 = CacheKeyGen::hash(&canonical("1+2"));
    let h2 = CacheKeyGen::hash(&canonical("1+2"));
    let h3 = CacheKeyGen::hash(&canonical("2+1"));
    let h4 = CacheKeyGen::hash(&canonical("1+3"));
    // 同一规范形式哈希稳定（BLAKE3 确定性）
    assert_eq!(h1, h2);
    // 规范化等价（交换律）→ 同一缓存键
    assert_eq!(h1, h3);
    // 不同表达式 → 不同键
    assert_ne!(h1, h4);
    // 哈希长度 = 32 字节（BLAKE3 摘要）
    let raw: [u8; 32] = CacheKeyGen::hash(&canonical("7"));
    assert_eq!(raw.len(), 32);
}

// ---------------------------------------------------------------------------
// evaluate 管线缓存命中契约
// ---------------------------------------------------------------------------

#[test]
fn cache_evaluate_hit_on_repeat() {
    let cache = CacheManager::new();
    let ctx = EvalContext::new();
    let (_, _, hit1, _) = evaluate("3+4*2", &ctx, None, &cache).unwrap();
    let (_, _, hit2, _) = evaluate("3+4*2", &ctx, None, &cache).unwrap();
    assert!(!hit1);
    assert!(hit2, "second identical evaluation must hit cache");
}

#[test]
fn cache_canonical_dedup_across_spellings() {
    // 规范形式等价的不同写法共享缓存条目
    let cache = CacheManager::new();
    let ctx = EvalContext::new();
    let (r1, _, hit1, _) = evaluate("1+2", &ctx, None, &cache).unwrap();
    let (r2, _, hit2, _) = evaluate("2+1", &ctx, None, &cache).unwrap();
    assert!(!hit1);
    assert!(hit2, "canonicalized 2+1 should reuse 1+2 entry");
    assert_eq!(r1, r2);
}

#[test]
fn cache_vars_change_forces_miss() {
    // 缓存键包含变量环境：同名表达式不同变量值不得串值
    let cache = CacheManager::new();
    let ctx_a = EvalContext::new().with_var("x", 1.0);
    let ctx_b = EvalContext::new().with_var("x", 2.0);
    let (r1, _, hit1, _) = evaluate("x*10", &ctx_a, None, &cache).unwrap();
    let (r2, _, hit2, _) = evaluate("x*10", &ctx_b, None, &cache).unwrap();
    assert!(!hit1);
    assert!(!hit2, "different var binding must be a cache miss");
    assert_ne!(r1, r2);
    // 同变量环境重复求值 → 命中
    let (_, _, hit3, _) = evaluate("x*10", &ctx_b, None, &cache).unwrap();
    assert!(hit3);
}

#[test]
fn cache_precision_mode_bypasses_router() {
    // precision 模式走独立缓存键（precision 语义参与键构建）
    let cache = CacheManager::new();
    let ctx = EvalContext::new();
    let (r1, _, hit1, p1) = evaluate("1/3", &ctx, Some(5), &cache).unwrap();
    let (r2, _, hit2, p2) = evaluate("1/3", &ctx, Some(5), &cache).unwrap();
    assert!(matches!(r1, EvalResult::BigRational(_)));
    assert_eq!(r1, r2);
    assert!(!hit1 && hit2);
    assert_eq!(p1, Some(5));
    assert_eq!(p2, Some(5));
}

#[test]
fn cache_nonexistent_expr_not_cached() {
    // 错误结果不入缓存：同一非法表达式两次求值均真实计算
    let cache = CacheManager::new();
    let ctx = EvalContext::new();
    let e1 = evaluate("1/0", &ctx, None, &cache).unwrap_err();
    let e2 = evaluate("1/0", &ctx, None, &cache).unwrap_err();
    assert_eq!(e1.kind, e2.kind);
    assert_eq!(cache.entry_count(), 0);
}

#[test]
fn pipeline_canonical_folds_constants() {
    // 规范化期常量折叠："1/0" 在 canonicalize 阶段即 DivisionByZero
    // （到不了求值/缓存层）；"3+0" 与 "3" 折叠等价
    let cache = CacheManager::new();
    let ctx = EvalContext::new();
    let e = evaluate("1/0", &ctx, None, &cache).unwrap_err();
    assert_eq!(e.kind, calnexus::ErrorKind::DivisionByZero);
    let (r1, d1, _, _) = evaluate("3+0", &ctx, None, &cache).unwrap();
    let (_, d2, hit2, _) = evaluate("3", &ctx, None, &cache).unwrap();
    assert_eq!(d1, "arithmetic");
    assert_eq!(d2, "arithmetic");
    assert_eq!(r1, EvalResult::Scalar(3.0));
    assert!(hit2, "3 reuses folded 3+0 cache entry");
    let _ = eval_err("1/0"); // 复用公共助手
}

// ---------------------------------------------------------------------------
// DomainRouter 结构契约
// ---------------------------------------------------------------------------

/// 测试用自定义域：仅在表达式包含指定函数名时接单。
struct TagDomain {
    name: &'static str,
    tag_fn: &'static str,
    priority: u8,
}

impl CalculationDomain for TagDomain {
    fn domain_name(&self) -> &str {
        self.name
    }
    fn supports(&self, ast: &AstNode) -> bool {
        matches!(ast, AstNode::FunctionCall(n, _) if n == self.tag_fn)
    }
    fn evaluate(
        &self,
        _ast: &AstNode,
        _ctx: &EvalContext,
    ) -> Result<EvalResult, calnexus::CalcError> {
        Ok(EvalResult::Scalar(self.priority as f64))
    }
    fn priority(&self) -> u8 {
        self.priority
    }
}

#[test]
fn router_empty_rejects_everything() {
    // 默认路由器总有算术域；空路由器的拒绝行为经 evaluate_with_router 验证
    let router = DomainRouter::new();
    let cache = CacheManager::new();
    let ctx = EvalContext::new();
    let err = evaluate_with_router("1+1", &ctx, None, &cache, &router).unwrap_err();
    assert!(err.message.contains("no registered domain"), "got {err}");
}

#[test]
fn router_register_dedups_by_name() {
    let mut router = DomainRouter::new();
    router.register(Box::new(TagDomain {
        name: "tag",
        tag_fn: "ping",
        priority: 50,
    }));
    router.register(Box::new(TagDomain {
        name: "tag",
        tag_fn: "ping",
        priority: 60, // 后注册同 名域被去重
    }));
    assert_eq!(router.domain_count(), 1);
}

#[test]
fn router_priority_descending_and_resolution() {
    let mut router = DomainRouter::new();
    router.register(Box::new(TagDomain {
        name: "low",
        tag_fn: "ping",
        priority: 10,
    }));
    router.register(Box::new(TagDomain {
        name: "high",
        tag_fn: "ping",
        priority: 90,
    }));
    // domain_names 按优先级降序
    let names = router.domain_names();
    assert_eq!(names.first().copied(), Some("high"));
    assert_eq!(names.last().copied(), Some("low"));
    // 高优先级域赢得路由
    let ast = calnexus::parse("ping()").unwrap();
    let (canonical, _) = calnexus::AstCanonicalizer::canonicalize(&ast).unwrap();
    let winner = router.route(&canonical).unwrap();
    assert_eq!(winner.domain_name(), "high");
    let cache = CacheManager::new();
    let ctx = EvalContext::new();
    let (r, d, _, _) = evaluate_with_router("ping()", &ctx, None, &cache, &router).unwrap();
    assert_eq!(d, "high");
    assert!(approx_eq(
        match r {
            EvalResult::Scalar(x) => x,
            _ => panic!(),
        },
        90.0
    ));
}

#[test]
fn router_is_nondeterministic_respects_registration() {
    let mut router = DomainRouter::new();
    router.register(Box::new(TagDomain {
        name: "clocky",
        tag_fn: "tick",
        priority: 30,
    }));
    let nd = router.is_nondeterministic(&calnexus::parse("tick()").unwrap());
    // TagDomain 未声明 nondeterministic_functions → false
    assert!(!nd);
}

#[test]
fn router_full_pipeline_with_custom_domain() {
    // 自定义域注入端到端：parse → canonicalize → route → evaluate
    let mut router = DomainRouter::new();
    router.register(Box::new(TagDomain {
        name: "answer",
        tag_fn: "answer",
        priority: 30,
    }));
    let cache = CacheManager::new();
    let ctx = EvalContext::new();
    let (r, d, _, _) = evaluate_with_router("answer(42)", &ctx, None, &cache, &router).unwrap();
    assert_eq!(d, "answer");
    assert!(approx_eq(
        match r {
            EvalResult::Scalar(x) => x,
            _ => panic!(),
        },
        30.0
    ));
    let _ = eval("2+3"); // 保持公共助手导入
}
