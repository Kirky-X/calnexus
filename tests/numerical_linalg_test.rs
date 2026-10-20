// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
// CalcError 144 字节，函数签名触发 result_large_err——与 lib.rs 同论证
#![allow(clippy::result_large_err)]

//! 端到端集成测试：数字分解函数经全链路传递，聚焦管线契约而非数学还原（单测职责）。

#![cfg(feature = "numerical")]

mod common;
use common::default_router;

use calnexus::{AstCanonicalizer, CacheManager, EvalContext, EvalResult, parse};

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
fn eig_general_end_to_end_returns_complex_values() {
    // 2×2 旋转矩阵特征值为 ±i：实部 0、虚部 -1/+1
    let v = as_json(evaluate_full("eig_general([[0,-1],[1,0]])").unwrap());
    let re = v["values_re"].as_array().expect("values_re must be array");
    let im = v["values_im"].as_array().expect("values_im must be array");
    assert_eq!(re.len(), 2);
    assert_eq!(im.len(), 2);
    assert!(re.iter().all(|x| x.as_f64().unwrap().abs() < 1e-9));
    assert!((im[0].as_f64().unwrap() + 1.0).abs() < 1e-9);
    assert!((im[1].as_f64().unwrap() - 1.0).abs() < 1e-9);
}

#[test]
fn eig_general_symmetric_still_returns_zero_imaginary() {
    let v = as_json(evaluate_full("eig_general([[2,1],[1,2]])").unwrap());
    let im = v["values_im"].as_array().expect("values_im must be array");
    assert!(im.iter().all(|x| x.as_f64().unwrap().abs() < 1e-9));
}

#[test]
fn eig_general_vectors_satisfy_definition_end_to_end() {
    let v = as_json(evaluate_full("eig_general([[0,-1],[1,0]])").unwrap());
    let m = [[0.0_f64, -1.0], [1.0, 0.0]];
    let re = v["values_re"].as_array().unwrap();
    let im = v["values_im"].as_array().unwrap();
    let vr = v["vectors_re"].as_array().unwrap();
    let vi = v["vectors_im"].as_array().unwrap();
    assert_eq!(vr.len(), 2);
    assert_eq!(vi.len(), 2);
    for k in 0..2 {
        let lambda = (re[k].as_f64().unwrap(), im[k].as_f64().unwrap());
        let xr = vr[k].as_array().unwrap();
        let xi = vi[k].as_array().unwrap();
        let x = [
            (xr[0].as_f64().unwrap(), xi[0].as_f64().unwrap()),
            (xr[1].as_f64().unwrap(), xi[1].as_f64().unwrap()),
        ];
        for i in 0..2 {
            let ax_re = m[i][0] * x[0].0 + m[i][1] * x[1].0;
            let ax_im = m[i][0] * x[0].1 + m[i][1] * x[1].1;
            let lx_re = lambda.0 * x[i].0 - lambda.1 * x[i].1;
            let lx_im = lambda.0 * x[i].1 + lambda.1 * x[i].0;
            let residual = ((ax_re - lx_re).powi(2) + (ax_im - lx_im).powi(2)).sqrt();
            assert!(residual < 1e-9, "row {i} residual {residual}");
        }
    }
}

#[test]
fn eig_non_symmetric_still_errors_with_hint() {
    let r = evaluate_full("eig([[1,2],[3,4]])");
    assert!(r.is_err());
    let err = r.unwrap_err();
    assert!(
        err.hint
            .as_deref()
            .is_some_and(|h| h.contains("eig_general")),
        "eig error hint should point at eig_general, got {:?}",
        err.hint
    );
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
    // 端到端：precision(50, eig(M)) 经路由（Matrix priority 30 抢）→ 专门 f64 错误
    let r = evaluate_full("precision(50, eig([[2,1],[1,2]]))");
    assert!(r.is_err());
    assert!(
        r.unwrap_err().to_string().contains("f64"),
        "should explain precision does not apply to f64 functions"
    );
}
