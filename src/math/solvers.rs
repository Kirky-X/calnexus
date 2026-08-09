// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! 方程数值求解器：Newton-Raphson / 二分法 / Brent 方法。
//!
//! 纯数学函数，供 `api/` 层直接调用。

use crate::core::CalcError;

/// Newton-Raphson 迭代求根。
///
/// - `f`: 目标函数
/// - `df`: 导数函数
/// - `x0`: 初始猜测
/// - `tol`: 收敛容差
/// - `max_iter`: 最大迭代次数
///
/// 导数为零 → `DomainError`；超 `max_iter` → `DomainError`。
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
                "newton_raphson: derivative is zero at x={}", x
            )));
        }
        x -= fx / dfx;
    }
    // 最终检查
    if f(x).abs() < tol {
        return Ok(x);
    }
    Err(CalcError::domain(format!(
        "newton_raphson: did not converge after {} iterations",
        max_iter
    )))
}

/// 二分法求根。
///
/// 要求 `f(a) · f(b) ≤ 0`，否则返回 `DomainError`。
pub fn bisection<F>(
    f: F,
    a: f64,
    b: f64,
    tol: f64,
    max_iter: usize,
) -> Result<f64, CalcError>
where
    F: Fn(f64) -> f64,
{
    let mut lo = a;
    let mut hi = b;
    let f_lo = f(lo);
    let f_hi = f(hi);
    if f_lo * f_hi > 0.0 {
        return Err(CalcError::domain(format!(
            "bisection: f({}) and f({}) have the same sign", lo, hi
        )));
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

/// Brent 方法求根（逆二次插值 + 二分回退）。
///
/// 要求 `f(a) · f(b) ≤ 0`。
pub fn brent<F>(
    f: F,
    a: f64,
    b: f64,
    tol: f64,
    max_iter: usize,
) -> Result<f64, CalcError>
where
    F: Fn(f64) -> f64,
{
    let mut a = a;
    let mut b = b;
    let mut fa = f(a);
    let mut fb = f(b);
    if fa * fb > 0.0 {
        return Err(CalcError::domain(format!(
            "brent: f({}) and f({}) have the same sign", a, b
        )));
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
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_approx(actual: f64, expected: f64, tol: f64, label: &str) {
        assert!(
            (actual - expected).abs() < tol,
            "{}: expected {} but got {} (diff={})",
            label, expected, actual, (actual - expected).abs()
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
        let root = newton_raphson(
            |x| x.cos() - x,
            |x| -x.sin() - 1.0,
            0.5,
            1e-12,
            100,
        )
        .unwrap();
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
        let result = newton_raphson(|x| x.atan() * 100.0, |x| 100.0 / (1.0 + x * x), 1e10, 1e-12, 2);
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
}
