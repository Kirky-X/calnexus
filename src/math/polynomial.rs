// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! 多项式核心函数：加减乘除、求值、求根、微分、积分、因式分解。
//!
//! 多项式表示：系数向量 `Vec<f64>`，升幂存储（`coef[i]` = x^i 的系数）。

use crate::core::{CalcError, EvalResult};

/// 多项式加法。
pub fn add(a: &[f64], b: &[f64]) -> Vec<f64> {
    let len = a.len().max(b.len());
    let mut result = vec![0.0; len];
    for (i, &c) in a.iter().enumerate() {
        result[i] += c;
    }
    for (i, &c) in b.iter().enumerate() {
        result[i] += c;
    }
    result
}

/// 多项式减法（a - b）。
pub fn sub(a: &[f64], b: &[f64]) -> Vec<f64> {
    let neg_b: Vec<f64> = b.iter().map(|x| -x).collect();
    add(a, &neg_b)
}

/// 多项式乘法（系数向量卷积）。
pub fn mul(a: &[f64], b: &[f64]) -> Vec<f64> {
    if a.is_empty() || b.is_empty() {
        return vec![0.0];
    }
    let mut result = vec![0.0; a.len() + b.len() - 1];
    for (i, &ai) in a.iter().enumerate() {
        for (j, &bj) in b.iter().enumerate() {
            result[i + j] += ai * bj;
        }
    }
    result
}

/// 多项式长除法，返回 `(quotient, remainder)`。
/// 检查零多项式除数，返回 Inf/NaN 而非静默错误结果
pub fn div(a: &[f64], b: &[f64]) -> (Vec<f64>, Vec<f64>) {
    let a = trim(a);
    let b = trim(b);
    // 检查零多项式除数（trim 后为 [0.0] 或空）
    if b.is_empty() || (b.len() == 1 && b[0] == 0.0) {
        // 返回含 NaN 的结果，调用方通过 is_finite 检查捕获
        return (vec![f64::NAN], vec![f64::NAN]);
    }
    if a.len() < b.len() || a.is_empty() {
        return (vec![0.0], a.clone());
    }
    let mut remainder = a.clone();
    let quotient_len = a.len() - b.len() + 1;
    let mut quotient = vec![0.0; quotient_len];
    let b_lead = b[b.len() - 1];
    for i in (0..quotient_len).rev() {
        let factor = remainder[i + b.len() - 1] / b_lead;
        quotient[i] = factor;
        for j in 0..b.len() {
            remainder[i + j] -= factor * b[j];
        }
    }
    remainder.truncate(b.len() - 1);
    if remainder.is_empty() {
        remainder.push(0.0);
    }
    (quotient, remainder)
}

/// Horner 法则求值。溢出时返回 `NaNOrInf` 错误。
pub fn eval(coeffs: &[f64], x: f64) -> Result<f64, CalcError> {
    if coeffs.is_empty() {
        return Ok(0.0);
    }
    let mut result = 0.0;
    for &c in coeffs.iter().rev() {
        result = result * x + c;
        if !result.is_finite() {
            return Err(CalcError::nan_or_inf());
        }
    }
    Ok(result)
}

/// 多项式微分：`coef[i] -> coef[i+1] * (i+1)`，降次。
pub fn diff(coeffs: &[f64]) -> Vec<f64> {
    if coeffs.len() <= 1 {
        return vec![0.0];
    }
    let mut result = Vec::with_capacity(coeffs.len() - 1);
    for (i, &c) in coeffs.iter().enumerate().skip(1) {
        result.push(c * i as f64);
    }
    result
}

/// 多项式不定积分：`coef[i] -> coef[i-1] / i`，升次，常数项=0。
pub fn integrate(coeffs: &[f64]) -> Vec<f64> {
    let mut result = vec![0.0];
    for (i, &c) in coeffs.iter().enumerate() {
        result.push(c / (i + 1) as f64);
    }
    result
}

/// 去除尾部零系数（高次零系数）。
pub fn trim(coeffs: &[f64]) -> Vec<f64> {
    let mut result = coeffs.to_vec();
    while result.len() > 1 && result.last() == Some(&0.0) {
        result.pop();
    }
    result
}

/// 判断是否为零多项式。
pub fn is_zero(coeffs: &[f64]) -> bool {
    coeffs.iter().all(|&c| c == 0.0)
}

/// 求根：支持 1-4 次多项式。>4 次返回 `Domain` 错误。
pub fn roots(coeffs: &[f64]) -> Result<EvalResult, CalcError> {
    let c = trim(coeffs);
    if c.len() == 1 {
        if c[0] == 0.0 {
            return Err(CalcError::domain(
                "roots(): zero polynomial has infinite roots".to_string(),
            ));
        }
        return Ok(EvalResult::Vector(vec![])); // 非零常数无根
    }
    match c.len() - 1 {
        1 => {
            let a = c[1];
            let b = c[0];
            Ok(EvalResult::Vector(vec![-b / a]))
        }
        2 => {
            let a = c[2];
            let b = c[1];
            let cc = c[0];
            let discriminant = b * b - 4.0 * a * cc;
            if discriminant >= 0.0 {
                let sqrt_d = discriminant.sqrt();
                let r1 = (-b + sqrt_d) / (2.0 * a);
                let r2 = (-b - sqrt_d) / (2.0 * a);
                Ok(EvalResult::Vector(vec![r1, r2]))
            } else {
                let sqrt_d = (-discriminant).sqrt();
                let re = -b / (2.0 * a);
                let im = sqrt_d / (2.0 * a);
                Ok(EvalResult::ComplexList(vec![(re, im), (re, -im)]))
            }
        }
        3 => {
            let r = solve_cubic(c[3], c[2], c[1], c[0]);
            Ok(roots_to_eval_result(r))
        }
        4 => {
            let r = solve_quartic(c[4], c[3], c[2], c[1], c[0]);
            Ok(roots_to_eval_result(r))
        }
        _ => Err(CalcError::domain(format!(
            "roots(): polynomial degree {} not supported (max degree 4)",
            c.len() - 1
        ))),
    }
}

/// 基础因式分解：≤2 次整数系数多项式。返回格式如 `"(x-2)*(x+2)"`。
pub fn factor(coeffs: &[f64]) -> Result<String, CalcError> {
    let c = trim(coeffs);
    if c.len() == 1 {
        return Ok(format!("{}", c[0] as i64));
    }
    match c.len() - 1 {
        1 => {
            let a = c[1];
            let b = c[0];
            let root = -b / a;
            Ok(format_factor_linear(a, root))
        }
        2 => {
            let a = c[2];
            let b = c[1];
            let cc = c[0];
            let discriminant = b * b - 4.0 * a * cc;
            if discriminant < 0.0 {
                return Err(CalcError::domain(
                    "factor(): complex roots cannot be factored over reals".to_string(),
                ));
            }
            let sqrt_d = discriminant.sqrt();
            let r1 = (-b + sqrt_d) / (2.0 * a);
            let r2 = (-b - sqrt_d) / (2.0 * a);
            Ok(format_factor_quadratic(a, r1, r2))
        }
        _ => Err(CalcError::domain(format!(
            "factor(): polynomial degree {} not supported (max degree 2 in v0.8)",
            c.len() - 1
        ))),
    }
}

// ===== 内部辅助函数 =====

/// 将复根列表转换为 EvalResult：全实根 → Vector，含复根 → ComplexList。
fn roots_to_eval_result(roots: Vec<(f64, f64)>) -> EvalResult {
    const EPS: f64 = 1e-10;
    let all_real = roots.iter().all(|(_, im)| im.abs() < EPS);
    if all_real {
        EvalResult::Vector(roots.into_iter().map(|(re, _)| re).collect())
    } else {
        EvalResult::ComplexList(roots)
    }
}

/// 解二次方程 at² + bt + c = 0，返回 (实部, 虚部) 对。
fn solve_quadratic_complex(a: f64, b: f64, c: f64) -> Vec<(f64, f64)> {
    let disc = b * b - 4.0 * a * c;
    if disc >= 0.0 {
        let sqrt_d = disc.sqrt();
        vec![
            ((-b + sqrt_d) / (2.0 * a), 0.0),
            ((-b - sqrt_d) / (2.0 * a), 0.0),
        ]
    } else {
        let sqrt_d = (-disc).sqrt();
        let re = -b / (2.0 * a);
        let im = sqrt_d / (2.0 * a);
        vec![(re, im), (re, -im)]
    }
}

/// 解三次方程 ax³ + bx² + cx + d = 0（Cardano 公式）。
fn solve_cubic(a: f64, b: f64, c: f64, d: f64) -> Vec<(f64, f64)> {
    const EPS: f64 = 1e-12;
    let b = b / a;
    let c = c / a;
    let d = d / a;

    let p = c - b * b / 3.0;
    let q = 2.0 * b * b * b / 27.0 - b * c / 3.0 + d;
    let shift = -b / 3.0;
    let disc = (q / 2.0).powi(2) + (p / 3.0).powi(3);

    if disc > EPS {
        let sqrt_d = disc.sqrt();
        let u = (-q / 2.0 + sqrt_d).cbrt();
        let v = (-q / 2.0 - sqrt_d).cbrt();
        let t1 = u + v;
        let re = -(u + v) / 2.0;
        let im = (u - v) * 3.0_f64.sqrt() / 2.0;
        vec![(t1 + shift, 0.0), (re + shift, im), (re + shift, -im)]
    } else if disc < -EPS {
        let m = 2.0 * (-p / 3.0).sqrt();
        let arg = (3.0 * q / (p * m)).clamp(-1.0, 1.0);
        let theta = arg.acos() / 3.0;
        let two_pi_3 = 2.0 * std::f64::consts::PI / 3.0;
        vec![
            (m * theta.cos() + shift, 0.0),
            (m * (theta - two_pi_3).cos() + shift, 0.0),
            (m * (theta + two_pi_3).cos() + shift, 0.0),
        ]
    } else if p.abs() < EPS {
        vec![(shift, 0.0), (shift, 0.0), (shift, 0.0)]
    } else {
        let u = (-q / 2.0).cbrt();
        vec![(2.0 * u + shift, 0.0), (-u + shift, 0.0), (-u + shift, 0.0)]
    }
}

/// 解四次方程 ax⁴ + bx³ + cx² + dx + e = 0（Ferrari 方法）。
fn solve_quartic(a: f64, b: f64, c: f64, d: f64, e: f64) -> Vec<(f64, f64)> {
    const EPS: f64 = 1e-12;
    let b = b / a;
    let c = c / a;
    let d = d / a;
    let e = e / a;

    let p = c - 3.0 * b * b / 8.0;
    let q = d - b * c / 2.0 + b * b * b / 8.0;
    let r = e - b * d / 4.0 + b * b * c / 16.0 - 3.0 * b.powi(4) / 256.0;
    let shift = -b / 4.0;

    if q.abs() < EPS {
        let disc = p * p - 4.0 * r;
        if disc >= 0.0 {
            let sqrt_disc = disc.sqrt();
            let t2_1 = (-p + sqrt_disc) / 2.0;
            let t2_2 = (-p - sqrt_disc) / 2.0;
            let mut roots = Vec::new();
            for &t2 in &[t2_1, t2_2] {
                if t2 >= 0.0 {
                    let t = t2.sqrt();
                    roots.push((t + shift, 0.0));
                    roots.push((-t + shift, 0.0));
                } else {
                    let t = (-t2).sqrt();
                    roots.push((shift, t));
                    roots.push((shift, -t));
                }
            }
            roots
        } else {
            let sqrt_disc = (-disc).sqrt();
            let re_t2 = -p / 2.0;
            let im_t2 = sqrt_disc / 2.0;
            let mut roots = Vec::new();
            for &sign in &[1.0, -1.0] {
                let (sr, si) = complex_sqrt(re_t2, im_t2 * sign);
                roots.push((sr + shift, si));
                roots.push((-sr + shift, -si));
            }
            roots
        }
    } else {
        let resolvent_roots = solve_cubic(1.0, 2.0 * p, p * p - 4.0 * r, -q * q);
        let m = match resolvent_roots
            .iter()
            .find(|(re, im)| im.abs() < EPS && *re > 0.0)
            .map(|(re, _)| *re)
        {
            Some(v) => v,
            // 将 .expect() 转为空结果返回，避免 panic
            None => return Vec::new(),
        };

        let sqrt_m = m.sqrt();
        let half_pm = (p + m) / 2.0;
        let q_term = q / (2.0 * sqrt_m);

        let mut roots = Vec::new();
        roots.extend(solve_quadratic_complex(1.0, -sqrt_m, half_pm + q_term));
        roots.extend(solve_quadratic_complex(1.0, sqrt_m, half_pm - q_term));
        roots.into_iter().map(|(re, im)| (re + shift, im)).collect()
    }
}

/// 复数平方根 √(re + im·i)。
fn complex_sqrt(re: f64, im: f64) -> (f64, f64) {
    let mag = (re * re + im * im).sqrt();
    let sqrt_re = ((mag + re) / 2.0).sqrt();
    let mut sqrt_im = ((mag - re) / 2.0).sqrt();
    if im < 0.0 {
        sqrt_im = -sqrt_im;
    }
    (sqrt_re, sqrt_im)
}

fn format_factor_linear(a: f64, r: f64) -> String {
    let lead = if a == 1.0 {
        String::new()
    } else if a == -1.0 {
        "-".to_string()
    } else {
        format!("{}*", a as i64)
    };
    let root_str = if r == 0.0 {
        "x".to_string()
    } else if r > 0.0 {
        format!("(x-{})", r as i64)
    } else {
        format!("(x+{})", (-r) as i64)
    };
    format!("{}{}", lead, root_str)
}

fn format_factor_quadratic(a: f64, r1: f64, r2: f64) -> String {
    let lead = if a == 1.0 {
        String::new()
    } else if a == -1.0 {
        "-".to_string()
    } else {
        format!("{}*", a as i64)
    };
    let f1 = format_factor_term(r1);
    let f2 = format_factor_term(r2);
    format!("{}{}*{}", lead, f1, f2)
}

fn format_factor_term(r: f64) -> String {
    if r == 0.0 {
        "x".to_string()
    } else if r > 0.0 {
        format!("(x-{})", r as i64)
    } else {
        format!("(x+{})", (-r) as i64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f64 = 1e-9;

    fn approx_eq(a: &[f64], b: &[f64]) -> bool {
        a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| (x - y).abs() < EPS)
    }

    // ===== 加法 =====

    #[test]
    fn test_add_same_degree() {
        // (1 + 2x) + (3 + 4x) = (4 + 6x)
        assert!(approx_eq(&add(&[1.0, 2.0], &[3.0, 4.0]), &[4.0, 6.0]));
    }

    #[test]
    fn test_add_different_degree() {
        // (1) + (0 + 0 + 1x²) = (1 + 0 + 1x²)
        assert!(approx_eq(&add(&[1.0], &[0.0, 0.0, 1.0]), &[1.0, 0.0, 1.0]));
    }

    #[test]
    fn test_add_zero() {
        assert!(approx_eq(&add(&[1.0, 2.0], &[0.0]), &[1.0, 2.0]));
    }

    // ===== 减法 =====

    #[test]
    fn test_sub() {
        // (3 + 4x) - (1 + 2x) = (2 + 2x)
        assert!(approx_eq(&sub(&[3.0, 4.0], &[1.0, 2.0]), &[2.0, 2.0]));
    }

    // ===== 乘法 =====

    #[test]
    fn test_mul_linear() {
        // (1 + x) * (1 + x) = (1 + 2x + x²)
        assert!(approx_eq(&mul(&[1.0, 1.0], &[1.0, 1.0]), &[1.0, 2.0, 1.0]));
    }

    #[test]
    fn test_mul_by_scalar() {
        assert!(approx_eq(&mul(&[2.0, 3.0], &[5.0]), &[10.0, 15.0]));
    }

    #[test]
    fn test_mul_empty() {
        assert_eq!(mul(&[], &[1.0, 2.0]), vec![0.0]);
    }

    // ===== 除法 =====

    #[test]
    fn test_div_exact() {
        // (x² - 1) / (x - 1) = (x + 1)
        // coeffs: [-1, 0, 1] / [-1, 1] = [1, 1]
        let (q, _r) = div(&[-1.0, 0.0, 1.0], &[-1.0, 1.0]);
        assert!(approx_eq(&q, &[1.0, 1.0]));
    }

    #[test]
    fn test_div_with_remainder() {
        // (x² + 1) / (x + 1) → q = (x - 1), r = 2
        let (q, r) = div(&[1.0, 0.0, 1.0], &[1.0, 1.0]);
        assert!(approx_eq(&q, &[-1.0, 1.0]));
        assert!(approx_eq(&r, &[2.0]));
    }

    #[test]
    fn test_div_lower_degree() {
        // (x) / (x² + 1) → q = 0, r = x
        let (q, _r) = div(&[0.0, 1.0], &[1.0, 0.0, 1.0]);
        assert!(approx_eq(&q, &[0.0]));
    }

    // ===== 求值 =====

    #[test]
    fn test_eval_horner() {
        // 1 + 2x + 3x² at x=2 → 1 + 4 + 12 = 17
        assert!((eval(&[1.0, 2.0, 3.0], 2.0).unwrap() - 17.0).abs() < EPS);
    }

    #[test]
    fn test_eval_empty() {
        assert!((eval(&[], 5.0).unwrap() - 0.0).abs() < EPS);
    }

    #[test]
    fn test_eval_constant() {
        assert!((eval(&[42.0], 100.0).unwrap() - 42.0).abs() < EPS);
    }

    #[test]
    fn test_eval_overflow() {
        assert!(matches!(eval(&[1.0, 1.0, 1.0, 1.0, 1.0], 1e308), Err(ref e) if e.kind == crate::core::ErrorKind::NaNOrInf));
    }

    // ===== 微分 =====

    #[test]
    fn test_diff() {
        // d/dx(1 + 2x + 3x²) = 2 + 6x
        assert!(approx_eq(&diff(&[1.0, 2.0, 3.0]), &[2.0, 6.0]));
    }

    #[test]
    fn test_diff_constant() {
        assert!(approx_eq(&diff(&[5.0]), &[0.0]));
    }

    // ===== 积分 =====

    #[test]
    fn test_integrate() {
        // ∫(2 + 6x)dx = 0 + 2x + 3x²
        assert!(approx_eq(&integrate(&[2.0, 6.0]), &[0.0, 2.0, 3.0]));
    }

    #[test]
    fn test_integrate_zero() {
        assert!(approx_eq(&integrate(&[0.0]), &[0.0, 0.0]));
    }

    // ===== trim / is_zero =====

    #[test]
    fn test_trim() {
        assert!(approx_eq(&trim(&[1.0, 2.0, 0.0, 0.0]), &[1.0, 2.0]));
    }

    #[test]
    fn test_trim_no_trailing_zeros() {
        assert!(approx_eq(&trim(&[1.0, 2.0]), &[1.0, 2.0]));
    }

    #[test]
    fn test_trim_single_zero() {
        assert!(approx_eq(&trim(&[0.0]), &[0.0]));
    }

    #[test]
    fn test_is_zero_true() {
        assert!(is_zero(&[0.0, 0.0, 0.0]));
    }

    #[test]
    fn test_is_zero_false() {
        assert!(!is_zero(&[0.0, 1.0]));
    }

    // ===== 求根 =====

    #[test]
    fn test_roots_linear() {
        // 2x + 6 = 0 → x = -3
        if let EvalResult::Vector(v) = roots(&[6.0, 2.0]).unwrap() {
            assert!((v[0] - (-3.0)).abs() < EPS);
        } else {
            panic!("expected Vector");
        }
    }

    #[test]
    fn test_roots_quadratic_real() {
        // x² - 4 = 0 → x = ±2
        if let EvalResult::Vector(v) = roots(&[-4.0, 0.0, 1.0]).unwrap() {
            assert!((v[0] - 2.0).abs() < EPS);
            assert!((v[1] - (-2.0)).abs() < EPS);
        } else {
            panic!("expected Vector");
        }
    }

    #[test]
    fn test_roots_quadratic_complex() {
        // x² + 1 = 0 → x = ±i
        if let EvalResult::ComplexList(v) = roots(&[1.0, 0.0, 1.0]).unwrap() {
            assert!((v[0].1 - 1.0).abs() < EPS);
            assert!((v[1].1 - (-1.0)).abs() < EPS);
        } else {
            panic!("expected ComplexList");
        }
    }

    #[test]
    fn test_roots_constant_zero() {
        assert!(matches!(roots(&[0.0]), Err(ref e) if e.kind == crate::core::ErrorKind::Domain));
    }

    #[test]
    fn test_roots_constant_nonzero() {
        if let EvalResult::Vector(v) = roots(&[5.0]).unwrap() {
            assert!(v.is_empty());
        } else {
            panic!("expected empty Vector");
        }
    }

    #[test]
    fn test_roots_degree_too_high() {
        assert!(matches!(roots(&[1.0, 0.0, 0.0, 0.0, 0.0, 1.0]), Err(ref e) if e.kind == crate::core::ErrorKind::Domain));
    }

    // ===== 因式分解 =====

    #[test]
    fn test_factor_linear() {
        // x - 2 → "(x-2)"
        let result = factor(&[-2.0, 1.0]).unwrap();
        assert_eq!(result, "(x-2)");
    }

    #[test]
    fn test_factor_quadratic() {
        // x² - 4 → "(x-2)*(x+2)"
        let result = factor(&[-4.0, 0.0, 1.0]).unwrap();
        assert_eq!(result, "(x-2)*(x+2)");
    }

    #[test]
    fn test_factor_complex_roots_error() {
        assert!(matches!(factor(&[1.0, 0.0, 1.0]), Err(ref e) if e.kind == crate::core::ErrorKind::Domain));
    }

    // ===== diff + integrate 互逆 =====

    #[test]
    fn test_diff_integrate_roundtrip() {
        let coeffs = [3.0, 2.0, 5.0]; // 3 + 2x + 5x²
        let d = diff(&coeffs); // 2 + 10x
        let i = integrate(&d); // 0 + 2x + 5x²
        // integrate(diff(f)) = f - f(0), so constant term differs
        assert!((i[0] - 0.0).abs() < EPS);
        assert!((i[1] - coeffs[1]).abs() < EPS);
        assert!((i[2] - coeffs[2]).abs() < EPS);
    }
}
