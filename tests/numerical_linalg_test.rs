// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! 端到端集成测试：parse → canonicalize → route → evaluate 全链路传递 numerical 分解函数。
//!
//! 聚焦**管线契约**（EvalResult::Json keys / Vector 变体 / solve 还原 / precision 错误），
//! 不重复 `numerical.rs` 单测已覆盖的数学还原（L·U=P·M 等是单测职责）。

#![cfg(feature = "numerical")]

mod common;
use common::default_router;

use calnexus::{parse, AstCanonicalizer, CacheManager, EvalContext, EvalResult};

/// 全链路求值：parse → canonicalize → cache → route → evaluate（router 含 MatrixDomain）。
fn evaluate_full(expr: &str) -> Result<EvalResult, calnexus::CalcError> {
    let ast = parse(expr)?;
    let (canonical_ast, cf) = AstCanonicalizer::canonicalize(&ast)?;

    let cache = CacheManager::new();
    if let Some(cached) = cache.get(&cf) {
        return Ok(cached);
    }

    let router = default_router();
    let domain = router.route(&canonical_ast)?;
    let result = domain.evaluate(&canonical_ast, &EvalContext::new())?;
    cache.insert(&cf, &Ok(result.clone()));

    Ok(result)
}

/// 从 `EvalResult::Json` 提取 `serde_json::Value`，否则 panic。
fn as_json(r: EvalResult) -> serde_json::Value {
    match r {
        EvalResult::Json(v) => v,
        other => panic!("expected EvalResult::Json, got {other:?}"),
    }
}

#[test]
fn lu_end_to_end_returns_json_with_lup() {
    let v = as_json(evaluate_full("lu([[4,3],[6,3]])").unwrap());
    assert!(v["L"].is_array(), "lu L must be a matrix (array of arrays)");
    assert!(v["U"].is_array(), "lu U must be a matrix");
    assert!(v["P"].is_array(), "lu P must be a matrix");
}

#[test]
fn qr_end_to_end_returns_json_with_qr() {
    // 经典 Wikipedia 3×3 QR 示例
    let v = as_json(evaluate_full("qr([[12,-51,4],[6,167,-68],[-4,24,-41]])").unwrap());
    assert!(v["Q"].is_array(), "qr Q must be a matrix");
    assert!(v["R"].is_array(), "qr R must be a matrix");
}

#[test]
fn eig_end_to_end_returns_json_with_values_and_vectors() {
    let v = as_json(evaluate_full("eig([[2,1],[1,2]])").unwrap());
    assert!(v["values"].is_array(), "eig values must be an array");
    assert!(v["vectors"].is_array(), "eig vectors must be a matrix");
}

#[test]
fn svd_end_to_end_returns_json_with_usvt() {
    let v = as_json(evaluate_full("svd([[1,2],[3,4]])").unwrap());
    assert!(v["U"].is_array(), "svd U must be a matrix");
    assert!(v["S"].is_array(), "svd S must be an array");
    assert!(v["Vt"].is_array(), "svd Vt must be a matrix");
}

#[test]
fn solve_end_to_end_returns_vector_satisfying_ax_eq_b() {
    // A·x = b 端到端还原：A=[[2,1],[1,3]], b=[3,5] → x≈[0.8, 1.4]
    // 验证 2*0.8+1*1.4=3.0 与 1*0.8+3*1.4=5.0
    let r = evaluate_full("solve([[2,1],[1,3]],[3,5])").unwrap();
    let x = match r {
        EvalResult::Vector(v) => v,
        other => panic!("expected EvalResult::Vector, got {other:?}"),
    };
    assert_eq!(x.len(), 2, "solve should return 2-element vector");
    assert!((x[0] - 0.8).abs() < 1e-9, "x[0] = {} expected 0.8", x[0]);
    assert!((x[1] - 1.4).abs() < 1e-9, "x[1] = {} expected 1.4", x[1]);
}

#[test]
fn precision_wrapping_numerical_end_to_end_errors() {
    // T009 端到端：precision(50, eig(M)) 经路由（Matrix priority 30 抢）→ 专门 f64 错误
    let r = evaluate_full("precision(50, eig([[2,1],[1,2]]))");
    assert!(r.is_err());
    assert!(
        r.unwrap_err().to_string().contains("f64"),
        "should explain precision does not apply to f64 functions"
    );
}
