// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! 直接 API 端到端集成测试。
//!
//! 验证 CalNexus 门面 + 5 个分组 trait 访问器的正确性。

use calnexus::{CalNexus, EvalResult, Matrix, Vector};

#[test]
fn test_scalar_add() {
    let cn = CalNexus::new();
    let result = cn.scalar().add(2.0, 3.0).unwrap();
    assert_eq!(result, EvalResult::Scalar(5.0));
}

#[test]
fn test_scalar_sub() {
    let cn = CalNexus::new();
    let result = cn.scalar().sub(10.0, 3.0).unwrap();
    assert_eq!(result, EvalResult::Scalar(7.0));
}

#[test]
fn test_scalar_mul() {
    let cn = CalNexus::new();
    let result = cn.scalar().mul(4.0, 5.0).unwrap();
    assert_eq!(result, EvalResult::Scalar(20.0));
}

#[test]
fn test_scalar_div() {
    let cn = CalNexus::new();
    let result = cn.scalar().div(10.0, 4.0).unwrap();
    assert_eq!(result, EvalResult::Scalar(2.5));
}

#[test]
fn test_scalar_div_by_zero() {
    let cn = CalNexus::new();
    let result = cn.scalar().div(1.0, 0.0);
    assert!(result.is_err());
}

#[test]
fn test_scalar_sin_pi_over_2() {
    let cn = CalNexus::new();
    let result = cn.scalar().sin(std::f64::consts::FRAC_PI_2).unwrap();
    let val = result.as_scalar().unwrap();
    assert!((val - 1.0).abs() < 1e-10);
}

#[test]
fn test_scalar_cos_zero() {
    let cn = CalNexus::new();
    let result = cn.scalar().cos(0.0).unwrap();
    assert_eq!(result, EvalResult::Scalar(1.0));
}

#[test]
fn test_linalg_det_2x2() {
    let cn = CalNexus::new();
    let m = Matrix::from_rows(&[&[1.0, 2.0], &[3.0, 4.0]]);
    let result = cn.linalg().det(&m).unwrap();
    // det = 1*4 - 2*3 = -2
    assert_eq!(result, EvalResult::Scalar(-2.0));
}

#[test]
fn test_linalg_det_3x3() {
    let cn = CalNexus::new();
    let m = Matrix::from_rows(&[&[1.0, 2.0, 3.0], &[4.0, 5.0, 6.0], &[7.0, 8.0, 10.0]]);
    let result = cn.linalg().det(&m).unwrap();
    // det = 1*(50-48) - 2*(40-42) + 3*(32-35) = 2+4-9 = -3
    assert_eq!(result, EvalResult::Scalar(-3.0));
}

#[test]
fn test_linalg_dot() {
    let cn = CalNexus::new();
    let a = Vector::new(&[1.0, 2.0, 3.0]);
    let b = Vector::new(&[4.0, 5.0, 6.0]);
    let result = cn.linalg().dot(&a, &b).unwrap();
    // 1*4 + 2*5 + 3*6 = 32
    assert_eq!(result, EvalResult::Scalar(32.0));
}

#[test]
fn test_stats_mean() {
    let cn = CalNexus::new();
    let result = cn.stats().mean(&[1.0, 2.0, 3.0, 4.0, 5.0]).unwrap();
    assert_eq!(result, EvalResult::Scalar(3.0));
}

#[test]
fn test_stats_std() {
    let cn = CalNexus::new();
    let result = cn.stats().std(&[2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0]).unwrap();
    let val = result.as_scalar().unwrap();
    // std dev ≈ 2.0
    assert!((val - 2.0).abs() < 0.1);
}

#[test]
fn test_set_var_get_var() {
    let cn = CalNexus::new();
    cn.set_var("x", 42.0);
    assert_eq!(cn.get_var("x"), Some(42.0));
    assert_eq!(cn.get_var("y"), None);
}

#[test]
fn test_clear_vars() {
    let cn = CalNexus::new();
    cn.set_var("x", 1.0);
    cn.set_var("y", 2.0);
    cn.clear_vars();
    assert_eq!(cn.get_var("x"), None);
    assert_eq!(cn.get_var("y"), None);
}

#[test]
fn test_default_instance() {
    let cn = CalNexus::default();
    let result = cn.scalar().add(1.0, 1.0).unwrap();
    assert_eq!(result, EvalResult::Scalar(2.0));
}

#[cfg(feature = "unit")]
#[test]
fn test_applied_convert() {
    let cn = CalNexus::new();
    let result = cn.applied().convert(1000.0, "m", "km").unwrap();
    assert_eq!(result, EvalResult::Scalar(1.0));
}

#[cfg(feature = "unit")]
#[test]
fn test_applied_convert_temperature() {
    let cn = CalNexus::new();
    let result = cn.applied().convert(100.0, "C", "F").unwrap();
    let val = result.as_scalar().unwrap();
    assert!((val - 212.0).abs() < 1e-9);
}
