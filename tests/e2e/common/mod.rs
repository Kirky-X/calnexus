// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! E2E 共享夹具：求值助手、断言助手、fx mock 基础设施。
//!
//! 求值助手均为「全新 CacheManager + 默认 EvalContext」的无状态封装，
//! 保证用例间零缓存串扰（`now`/`fx` 等非确定性函数本就旁路 L1 缓存）。

#![allow(dead_code)]

use calnexus::{
    evaluate, evaluate_with_router, CalcError, CacheManager, DomainRouter, EvalContext, EvalResult,
};

/// 默认上下文求值，返回 (result, domain, cache_hit, precision) 四元组。
pub fn eval(expr: &str) -> Result<(EvalResult, String, bool, Option<usize>), CalcError> {
    evaluate(expr, &EvalContext::new(), None, &CacheManager::new())
}

/// 求值并断言成功，返回结果值（失败时 panic 并携带表达式与错误详情）。
pub fn eval_ok(expr: &str) -> EvalResult {
    match eval(expr) {
        Ok((r, _, _, _)) => r,
        Err(e) => panic!("eval `{expr}` should succeed, got: {e:?}"),
    }
}

/// 求值并断言成功，同时返回结果与路由域名。
pub fn eval_ok_with_domain(expr: &str) -> (EvalResult, String) {
    match eval(expr) {
        Ok((r, d, _, _)) => (r, d),
        Err(e) => panic!("eval `{expr}` should succeed, got: {e:?}"),
    }
}

/// 求值并断言失败，返回 CalcError（成功时 panic）。
pub fn eval_err(expr: &str) -> CalcError {
    match eval(expr) {
        Ok((r, d, _, _)) => panic!("eval `{expr}` should fail, got {r:?} (domain {d})"),
        Err(e) => e,
    }
}

/// 经指定路由器求值（fx mock 等自定义域的注入通道）。
pub fn eval_via_router(
    expr: &str,
    router: &DomainRouter,
) -> Result<(EvalResult, String, bool, Option<usize>), CalcError> {
    evaluate_with_router(
        expr,
        &EvalContext::new(),
        None,
        &CacheManager::new(),
        router,
    )
}

/// 经指定路由器求值并断言成功，返回 (result, domain)。
pub fn eval_via_router_ok(expr: &str, router: &DomainRouter) -> (EvalResult, String) {
    match eval_via_router(expr, router) {
        Ok((r, d, _, _)) => (r, d),
        Err(e) => panic!("eval `{expr}` via router should succeed, got: {e:?}"),
    }
}

/// 浮点近似比较（1e-9 相对容差；无限值按位等同处理）。
pub fn approx_eq(a: f64, b: f64) -> bool {
    if a.is_infinite() || b.is_infinite() {
        return a == b;
    }
    (a - b).abs() <= 1e-9_f64.max(1e-9 * a.abs().max(b.abs()))
}

/// 断言 EvalResult 为 Scalar 且近似等于期望值。
pub fn assert_scalar(result: &EvalResult, expected: f64) {
    match result {
        EvalResult::Scalar(v) => assert!(
            approx_eq(*v, expected),
            "expected scalar ~{expected}, got {v}"
        ),
        other => panic!("expected Scalar(~{expected}), got {other:?}"),
    }
}

#[cfg(feature = "fx")]
pub mod fx_mock;
