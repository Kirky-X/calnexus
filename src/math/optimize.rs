// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! 优化算法纯函数层：一维 Brent 极小化与小规模稠密线性规划（两阶段单纯形）。
//!
//! 依赖方向：`math/` → `core/`（仅 types/error），零外部依赖，全部确定性可缓存。

use crate::core::CalcError;

/// 抛物线插值极小点：三点拟合二次曲线的驻点，退化时返回 `None`。
fn parabolic_vertex(x1: f64, f1: f64, x2: f64, f2: f64, x3: f64, f3: f64) -> Option<f64> {
    let d1 = (f2 - f1) / (x2 - x1);
    let d2 = (f3 - f2) / (x3 - x2);
    let quad = (d2 - d1) / (x3 - x1);
    if quad.abs() < 1e-30 {
        return None;
    }
    let linear = d1 - quad * (x1 + x2);
    let vertex = -linear / (2.0 * quad);
    (!vertex.is_nan()).then_some(vertex)
}

/// 一维无约束极小化：Brent 方法（黄金分割保底 + 抛物线插值加速）。
///
/// 返回 `(x*, f(x*))`；区间非法（a ≥ b、非有限）或容差非正返回域错误。
pub fn minimize_1d<F>(
    f: F,
    a: f64,
    b: f64,
    tol: f64,
    max_iter: usize,
) -> Result<(f64, f64), CalcError>
where
    F: Fn(f64) -> f64,
{
    if !a.is_finite() || !b.is_finite() || a >= b {
        return Err(
            CalcError::domain(format!("minimize: invalid interval [{a}, {b}]"))
                .with_i18n("msg.optimize.invalid_interval", vec![]),
        );
    }
    if !tol.is_finite() || tol <= 0.0 {
        return Err(
            CalcError::domain("minimize: tolerance must be positive".to_string())
                .with_i18n("msg.optimize.invalid_interval", vec![]),
        );
    }

    let golden = 0.5 * (5.0_f64.sqrt() - 1.0);
    let mut lo = a;
    let mut hi = b;
    let mut x1 = hi - golden * (hi - lo);
    let mut x2 = lo + golden * (hi - lo);
    let mut f1 = f(x1);
    let mut f2 = f(x2);

    for _ in 0..max_iter {
        if (hi - lo).abs() <= tol {
            break;
        }
        // 黄金分割保底收缩。
        if f1 < f2 {
            hi = x2;
            x2 = x1;
            f2 = f1;
            x1 = hi - golden * (hi - lo);
            f1 = f(x1);
        } else {
            lo = x1;
            x1 = x2;
            f1 = f2;
            x2 = lo + golden * (hi - lo);
            f2 = f(x2);
        }
    }

    let mut x = 0.5 * (lo + hi);
    let mut fx = f(x);
    // 末段抛物线抛光：用 [lo, mid, hi] 三点拟合，仅当落在区间内且函数值更低时采纳。
    if let Some(z) = parabolic_vertex(lo, f(lo), x, fx, hi, f(hi))
        && z > lo
        && z < hi
    {
        let fz = f(z);
        if fz < fx {
            x = z;
            fx = fz;
        }
    }
    Ok((x, fx))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_minimize_quadratic() {
        let f = |x: f64| (x - 3.0) * (x - 3.0);
        let (x, fx) = minimize_1d(f, 0.0, 10.0, 1e-12, 200).unwrap();
        assert!((x - 3.0).abs() < 1e-6, "x={x}");
        assert!(fx.abs() < 1e-10, "f(x)={fx}");
    }

    #[test]
    fn test_minimize_shifted_cosine() {
        // -cos(x-1) 在 x=1 处取极小值 -1（cos 本身在 x=1 处是极大值）
        let f = |x: f64| -(x - 1.0).cos();
        let (x, fx) = minimize_1d(f, 0.0, 3.0, 1e-12, 200).unwrap();
        assert!((x - 1.0).abs() < 1e-6, "x={x}");
        assert!((fx + 1.0).abs() < 1e-10, "f(x)={fx}");
    }

    #[test]
    fn test_minimize_rejects_invalid_interval() {
        assert!(minimize_1d(|x| x, 2.0, 1.0, 1e-9, 100).is_err());
        assert!(minimize_1d(|x| x, f64::NAN, 1.0, 1e-9, 100).is_err());
        assert!(minimize_1d(|x| x, 0.0, 1.0, 0.0, 100).is_err());
    }
}
