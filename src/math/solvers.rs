// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! 方程数值求解器：Newton-Raphson / 二分法 / Brent，纯函数供 `api/` 调用。

use crate::core::CalcError;

/// Newton-Raphson 迭代求根；导数为零或超 `max_iter` 返回 `DomainError`。
pub fn newton_raphson<F, G>(
    f: F,
    df: G,
    x0: f64,
    tol: f64,
    max_iter: usize,
) -> Result<f64, CalcError>
where
    F: Fn(f64) -> f64,
    G: Fn(f64) -> f64,
{
    let mut x = x0;
    for _ in 0..max_iter {
        let fx = f(x);
        if fx.abs() < tol {
            return Ok(x);
        }
        let dfx = df(x);
        if dfx.abs() < 1e-30 {
            return Err(CalcError::domain(format!(
                "newton_raphson: derivative is zero at x={}",
                x
            ))
            .with_i18n(
                "msg.solvers.newton_derivative_zero",
                vec![("x".to_string(), x.to_string())],
            ));
        }
        x -= fx / dfx;
    }
    if f(x).abs() < tol {
        return Ok(x);
    }
    Err(CalcError::domain(format!(
        "newton_raphson: did not converge after {} iterations",
        max_iter
    ))
    .with_i18n(
        "msg.solvers.newton_not_converged",
        vec![("max_iter".to_string(), max_iter.to_string())],
    ))
}

/// 二分法求根；要求 `f(a) · f(b) ≤ 0`，否则返回 `DomainError`。
pub fn bisection<F>(f: F, a: f64, b: f64, tol: f64, max_iter: usize) -> Result<f64, CalcError>
where
    F: Fn(f64) -> f64,
{
    let mut lo = a;
    let mut hi = b;
    let f_lo = f(lo);
    let f_hi = f(hi);
    if f_lo * f_hi > 0.0 {
        return Err(CalcError::domain(format!(
            "bisection: f({}) and f({}) have the same sign",
            lo, hi
        ))
        .with_i18n(
            "msg.solvers.bisection_same_sign",
            vec![
                ("a".to_string(), lo.to_string()),
                ("b".to_string(), hi.to_string()),
            ],
        ));
    }
    for _ in 0..max_iter {
        let mid = (lo + hi) / 2.0;
        let f_mid = f(mid);
        if f_mid.abs() < tol || (hi - lo) / 2.0 < tol {
            return Ok(mid);
        }
        if f_lo * f_mid <= 0.0 {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    let mid = (lo + hi) / 2.0;
    Ok(mid)
}

/// Brent 求根（逆二次插值 + 二分回退）；要求 `f(a) · f(b) ≤ 0`。
pub fn brent<F>(f: F, a: f64, b: f64, tol: f64, max_iter: usize) -> Result<f64, CalcError>
where
    F: Fn(f64) -> f64,
{
    let mut a = a;
    let mut b = b;
    let mut fa = f(a);
    let mut fb = f(b);
    if fa * fb > 0.0 {
        return Err(
            CalcError::domain(format!("brent: f({}) and f({}) have the same sign", a, b))
                .with_i18n(
                    "msg.solvers.brent_same_sign",
                    vec![
                        ("a".to_string(), a.to_string()),
                        ("b".to_string(), b.to_string()),
                    ],
                ),
        );
    }
    if fa.abs() < fb.abs() {
        std::mem::swap(&mut a, &mut b);
        std::mem::swap(&mut fa, &mut fb);
    }
    let mut c = a;
    let mut fc = fa;
    let mut mflag = true;
    let mut s;
    let mut d = 0.0;

    for _ in 0..max_iter {
        if fb.abs() < tol {
            return Ok(b);
        }
        if (b - a).abs() < tol {
            return Ok(b);
        }
        if fa != fc && fb != fc {
            // 逆二次插值
            s = a * fb * fc / ((fa - fb) * (fa - fc))
                + b * fa * fc / ((fb - fa) * (fb - fc))
                + c * fa * fb / ((fc - fa) * (fc - fb));
        } else {
            // 二分法
            s = b - fb * (b - a) / (fb - fa);
        }
        // 条件检查：是否回退到二分
        let cond1 = if a < b {
            !(s > (3.0 * a + b) / 4.0 && s < b)
        } else {
            !(s > b && s < (3.0 * a + b) / 4.0)
        };
        let cond2 = mflag && (s - b).abs() >= (b - c).abs() / 2.0;
        let cond3 = !mflag && (s - b).abs() >= (c - d).abs() / 2.0;
        let cond4 = mflag && (b - c).abs() < tol;
        let cond5 = !mflag && (c - d).abs() < tol;
        if cond1 || cond2 || cond3 || cond4 || cond5 {
            s = (a + b) / 2.0;
            mflag = true;
        } else {
            mflag = false;
        }
        let fs = f(s);
        d = c;
        c = b;
        fc = fb;
        if fa * fs < 0.0 {
            b = s;
            fb = fs;
        } else {
            a = s;
            fa = fs;
        }
        if fa.abs() < fb.abs() {
            std::mem::swap(&mut a, &mut b);
            std::mem::swap(&mut fa, &mut fb);
        }
    }
    if fb.abs() < tol {
        return Ok(b);
    }
    Err(CalcError::domain(format!(
        "brent: did not converge after {} iterations",
        max_iter
    ))
    .with_i18n(
        "msg.solvers.brent_not_converged",
        vec![("max_iter".to_string(), max_iter.to_string())],
    ))
}

/// 割线法求根；斜率接近 0 或超 `max_iter` 返回 `DomainError`。
pub fn secant<F>(f: F, x0: f64, x1: f64, tol: f64, max_iter: usize) -> Result<f64, CalcError>
where
    F: Fn(f64) -> f64,
{
    let mut prev = x0;
    let mut curr = x1;
    let mut f_prev = f(prev);
    let mut f_curr = f(curr);
    if f_prev.abs() < tol {
        return Ok(prev);
    }
    if f_curr.abs() < tol {
        return Ok(curr);
    }
    for _ in 0..max_iter {
        let denom = f_curr - f_prev;
        if denom.abs() < 1e-300 {
            return Err(
                CalcError::domain(format!("secant: zero denominator at x={}", curr)).with_i18n(
                    "msg.solvers.secant_zero_denominator",
                    vec![("x".to_string(), curr.to_string())],
                ),
            );
        }
        let next = curr - f_curr * (curr - prev) / denom;
        if !next.is_finite() {
            return Err(CalcError::nan_or_inf());
        }
        prev = curr;
        f_prev = f_curr;
        curr = next;
        f_curr = f(curr);
        if f_curr.abs() < tol || (curr - prev).abs() < tol {
            return Ok(curr);
        }
    }
    Err(CalcError::domain(format!(
        "secant: did not converge after {} iterations",
        max_iter
    ))
    .with_i18n(
        "msg.solvers.secant_not_converged",
        vec![("max_iter".to_string(), max_iter.to_string())],
    ))
}

/// 自适应 Simpson 积分；`a > b` 自动交换并取负，递归深度耗尽返回 `DomainError`。
pub fn adaptive_simpson<F>(
    f: F,
    a: f64,
    b: f64,
    tol: f64,
    max_depth: usize,
) -> Result<f64, CalcError>
where
    F: Fn(f64) -> f64,
{
    if a == b {
        return Ok(0.0);
    }
    let (lo, hi, sign) = if a < b { (a, b, 1.0) } else { (b, a, -1.0) };
    let fa = f(lo);
    let fm = f(0.5 * (lo + hi));
    let fb = f(hi);
    if !fa.is_finite() || !fm.is_finite() || !fb.is_finite() {
        return Err(CalcError::nan_or_inf());
    }
    let whole = (hi - lo) / 6.0 * (fa + 4.0 * fm + fb);
    let value = simpson_rec(&f, lo, hi, fa, fm, fb, whole, tol, max_depth)?;
    Ok(sign * value)
}

// 递归参数已按 Simpson 自适应算法必需量拆分，结构体包装无收益
#[allow(clippy::too_many_arguments)]
fn simpson_rec<F>(
    f: &F,
    a: f64,
    b: f64,
    fa: f64,
    fm: f64,
    fb: f64,
    whole: f64,
    tol: f64,
    depth: usize,
) -> Result<f64, CalcError>
where
    F: Fn(f64) -> f64,
{
    let m = 0.5 * (a + b);
    let lm = 0.5 * (a + m);
    let rm = 0.5 * (m + b);
    let flm = f(lm);
    let frm = f(rm);
    if !flm.is_finite() || !frm.is_finite() {
        return Err(CalcError::nan_or_inf());
    }
    let left = (m - a) / 6.0 * (fa + 4.0 * flm + fm);
    let right = (b - m) / 6.0 * (fm + 4.0 * frm + fb);
    let delta = left + right - whole;
    if delta.abs() <= 15.0 * tol {
        return Ok(left + right + delta / 15.0);
    }
    if depth == 0 {
        return Err(CalcError::domain(
            "adaptive_simpson: maximum recursion depth reached".to_string(),
        )
        .with_i18n("msg.solvers.simpson_depth", vec![]));
    }
    let lv = simpson_rec(f, a, m, fa, flm, fm, left, tol / 2.0, depth - 1)?;
    let rv = simpson_rec(f, m, b, fm, frm, fb, right, tol / 2.0, depth - 1)?;
    Ok(lv + rv)
}

/// QUADPACK qk15 Kronrod 节点（含中心 0）。
const XGK: [f64; 8] = [
    0.991_455_371_120_812_6,
    0.949_107_912_342_758_5,
    0.864_864_423_359_769_1,
    0.741_531_185_599_394_2,
    0.586_087_235_467_691_2,
    0.405_845_151_377_397_2,
    0.207_784_955_007_898_4,
    0.0,
];

/// Kronrod 权重。
const WGK: [f64; 8] = [
    0.022_935_322_010_529_22,
    0.063_092_092_629_978_55,
    0.104_790_010_322_249_8,
    0.140_653_259_715_525_2,
    0.169_004_726_639_267_1,
    0.190_350_578_064_784_9,
    0.204_432_940_075_298_2,
    0.209_482_141_084_727_8,
];

/// 嵌入 Gauss-7 权重。
const WG: [f64; 4] = [
    0.129_484_966_168_869_7,
    0.279_705_391_489_276_7,
    0.381_830_050_505_118_9,
    0.417_959_183_673_469_4,
];

/// Gauss–Kronrod 15 点积分；返回（积分值, 误差估计）；`a > b` 自动交换并取负。
pub fn gauss_kronrod_15<F>(f: F, a: f64, b: f64) -> Result<(f64, f64), CalcError>
where
    F: Fn(f64) -> f64,
{
    if a == b {
        return Ok((0.0, 0.0));
    }
    let (lo, hi, sign) = if a < b { (a, b, 1.0) } else { (b, a, -1.0) };
    let center = 0.5 * (lo + hi);
    let half = 0.5 * (hi - lo);
    let fc = f(center);
    if !fc.is_finite() {
        return Err(CalcError::nan_or_inf());
    }
    let mut res_gk = WGK[7] * fc;
    let mut res_g = WG[3] * fc;
    for i in 0..7 {
        let dx = half * XGK[i];
        let f1 = f(center - dx);
        let f2 = f(center + dx);
        if !f1.is_finite() || !f2.is_finite() {
            return Err(CalcError::nan_or_inf());
        }
        let sum = f1 + f2;
        res_gk += WGK[i] * sum;
        match i {
            1 => res_g += WG[0] * sum,
            3 => res_g += WG[1] * sum,
            5 => res_g += WG[2] * sum,
            _ => {}
        }
    }
    let value = res_gk * half;
    let err = (res_gk - res_g).abs() * half;
    Ok((sign * value, err))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_approx(actual: f64, expected: f64, tol: f64, label: &str) {
        assert!(
            (actual - expected).abs() < tol,
            "{}: expected {} but got {} (diff={})",
            label,
            expected,
            actual,
            (actual - expected).abs()
        );
    }

    // ===== Newton-Raphson =====

    #[test]
    fn test_newton_sqrt2() {
        // f(x) = x² - 2, f'(x) = 2x → √2
        let root = newton_raphson(|x| x * x - 2.0, |x| 2.0 * x, 1.5, 1e-12, 100).unwrap();
        assert_approx(root, std::f64::consts::SQRT_2, 1e-10, "sqrt2");
    }

    #[test]
    fn test_newton_cos_eq_x() {
        // cos(x) = x → x ≈ 0.7390851
        let root = newton_raphson(|x| x.cos() - x, |x| -x.sin() - 1.0, 0.5, 1e-12, 100).unwrap();
        assert_approx(root, 0.7390851332151607, 1e-10, "cos(x)=x");
    }

    #[test]
    fn test_newton_zero_derivative() {
        // f(x) = x² + 1 (no real root), start at x=0 where f'(0)=0
        let result = newton_raphson(|x| x * x + 1.0, |x| 2.0 * x, 0.0, 1e-12, 100);
        assert!(result.is_err());
    }

    #[test]
    fn test_newton_no_convergence() {
        // f(x) = atan(x) * 100 with very few iterations
        let result = newton_raphson(
            |x| x.atan() * 100.0,
            |x| 100.0 / (1.0 + x * x),
            1e10,
            1e-12,
            2,
        );
        assert!(result.is_err());
    }

    // ===== Bisection =====

    #[test]
    fn test_bisection_pi() {
        // sin(x) = 0 in [3, 4] → π
        let root = bisection(|x| x.sin(), 3.0, 4.0, 1e-12, 100).unwrap();
        assert_approx(root, std::f64::consts::PI, 1e-10, "pi");
    }

    #[test]
    fn test_bisection_cubic() {
        // x³ - x - 1 = 0 in [1, 2] → ≈ 1.3247
        let root = bisection(|x| x * x * x - x - 1.0, 1.0, 2.0, 1e-12, 100).unwrap();
        assert_approx(root, 1.324717957244746, 1e-10, "cubic");
    }

    #[test]
    fn test_bisection_same_sign() {
        // f(x) = x² + 1 > 0 everywhere → error
        let result = bisection(|x| x * x + 1.0, 0.0, 1.0, 1e-12, 100);
        assert!(result.is_err());
    }

    // ===== Brent =====

    #[test]
    fn test_brent_cubic() {
        // x³ - x - 1 = 0 in [1, 2]
        let root = brent(|x| x * x * x - x - 1.0, 1.0, 2.0, 1e-12, 100).unwrap();
        assert_approx(root, 1.324717957244746, 1e-10, "brent cubic");
    }

    #[test]
    fn test_brent_exp_minus_3() {
        // e^x - 3 = 0 in [0, 2] → ln(3)
        let root = brent(|x| x.exp() - 3.0, 0.0, 2.0, 1e-12, 100).unwrap();
        assert_approx(root, 3.0_f64.ln(), 1e-10, "brent ln3");
    }

    #[test]
    fn test_brent_same_sign() {
        let result = brent(|x| x * x + 1.0, 0.0, 1.0, 1e-12, 100);
        assert!(result.is_err());
    }

    // ===== Secant =====

    #[test]
    fn test_secant_sqrt2() {
        let root = secant(|x| x * x - 2.0, 1.0, 2.0, 1e-12, 100).unwrap();
        assert_approx(root, std::f64::consts::SQRT_2, 1e-10, "secant sqrt2");
    }

    #[test]
    fn test_secant_zero_denominator_errors() {
        // 常数函数 → 割线斜率为 0
        let result = secant(|_| 1.0, 0.0, 1.0, 1e-12, 10);
        assert!(result.is_err());
    }

    // ===== 数值积分 =====

    #[test]
    fn test_adaptive_simpson_sin() {
        let v = adaptive_simpson(|x: f64| x.sin(), 0.0, std::f64::consts::PI, 1e-10, 50).unwrap();
        assert_approx(v, 2.0, 1e-9, "adaptive simpson sin");
    }

    #[test]
    fn test_adaptive_simpson_empty_interval() {
        let v = adaptive_simpson(|x: f64| x.sin(), 1.0, 1.0, 1e-10, 50).unwrap();
        assert_eq!(v, 0.0);
    }

    #[test]
    fn test_gauss_kronrod_sin() {
        let (v, err) = gauss_kronrod_15(|x: f64| x.sin(), 0.0, std::f64::consts::PI).unwrap();
        assert_approx(v, 2.0, 1e-12, "gk15 sin");
        assert!(err < 1e-11, "error estimate too large: {err}");
    }

    #[test]
    fn test_gauss_kronrod_reversed_interval() {
        let (v, _) = gauss_kronrod_15(|x: f64| x, 1.0, 0.0).unwrap();
        assert_approx(v, -0.5, 1e-12, "gk15 reversed");
    }
}
