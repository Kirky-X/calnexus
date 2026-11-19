// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! 方程求解纯函数层：多项式精确根（≤4 次）、Durand-Kerner 数值根与超越方程区间求根。
//!
//! 依赖方向：`math/` → `core/`（仅 types/error），复用 `math::polynomial`、`math::solvers`
//! 与 `math::symbolic`；不依赖 `domains/` 或 `api/`。全部为确定性函数，可进缓存。

use std::collections::HashMap;

use crate::core::CalcError;
use crate::math::polynomial;
use crate::math::solvers::bisection;
use crate::math::symbolic::{SymbolicExpr, eval_symbolic};

/// 一元方程求解结果。
#[derive(Debug, Clone, PartialEq)]
pub enum SolutionSet {
    /// 精确根（含复根），元素为 (实部, 虚部)。
    Exact(Vec<(f64, f64)>),
    /// 实数值根（超越方程 / 高次多项式实根）。
    Numeric(Vec<f64>),
    /// 高次多项式的数值根（含复根），元素为 (实部, 虚部)。
    NumericComplex(Vec<(f64, f64)>),
}

/// 不等式比较方向（求解域消费，与 core::CompareOp 一一对应）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InequalityOp {
    Lt,
    Le,
    Gt,
    Ge,
}

/// 复数（Durand-Kerner 内部使用；核心库零外部依赖）。
#[derive(Debug, Clone, Copy, PartialEq)]
struct Complex {
    re: f64,
    im: f64,
}

impl Complex {
    fn new(re: f64, im: f64) -> Self {
        Self { re, im }
    }

    fn add(self, other: Self) -> Self {
        Self::new(self.re + other.re, self.im + other.im)
    }

    fn sub(self, other: Self) -> Self {
        Self::new(self.re - other.re, self.im - other.im)
    }

    fn mul(self, other: Self) -> Self {
        Self::new(
            self.re * other.re - self.im * other.im,
            self.re * other.im + self.im * other.re,
        )
    }

    fn div(self, other: Self) -> Self {
        let den = other.re * other.re + other.im * other.im;
        Self::new(
            (self.re * other.re + self.im * other.im) / den,
            (self.im * other.re - self.re * other.im) / den,
        )
    }

    fn abs(self) -> f64 {
        (self.re * self.re + self.im * self.im).sqrt()
    }

    /// 非负整数次幂（重复乘法，避免引入外部复数 crate）。
    fn pow_nat(self, n: usize) -> Self {
        let mut result = Self::new(1.0, 0.0);
        for _ in 0..n {
            result = result.mul(self);
        }
        result
    }
}

/// 多项式加法（系数升幂）。
fn poly_add(a: &[f64], b: &[f64]) -> Vec<f64> {
    let n = a.len().max(b.len());
    let mut out = vec![0.0; n];
    for (i, &v) in a.iter().enumerate() {
        out[i] += v;
    }
    for (i, &v) in b.iter().enumerate() {
        out[i] += v;
    }
    out
}

/// 多项式取负。
fn poly_neg(a: &[f64]) -> Vec<f64> {
    a.iter().map(|v| -v).collect()
}

/// 多项式乘法（卷积）。
fn poly_mul(a: &[f64], b: &[f64]) -> Vec<f64> {
    if a.is_empty() || b.is_empty() {
        return Vec::new();
    }
    let mut out = vec![0.0; a.len() + b.len() - 1];
    for (i, &x) in a.iter().enumerate() {
        for (j, &y) in b.iter().enumerate() {
            out[i + j] += x * y;
        }
    }
    out
}

/// 多项式标量缩放。
fn poly_scale(a: &[f64], k: f64) -> Vec<f64> {
    a.iter().map(|v| v * k).collect()
}

/// 把符号表达式转为指定变量的升幂系数向量；非多项式返回 `None`。
fn symbolic_to_coeffs(expr: &SymbolicExpr, var: &str) -> Option<Vec<f64>> {
    match expr {
        SymbolicExpr::Const(v) => Some(vec![*v]),
        SymbolicExpr::Var(name) if name == var => Some(vec![0.0, 1.0]),
        SymbolicExpr::Var(_) => None,
        SymbolicExpr::Add(l, r) => {
            let lc = symbolic_to_coeffs(l, var)?;
            let rc = symbolic_to_coeffs(r, var)?;
            Some(poly_add(&lc, &rc))
        }
        SymbolicExpr::Sub(l, r) => {
            let lc = symbolic_to_coeffs(l, var)?;
            let rc = symbolic_to_coeffs(r, var)?;
            Some(poly_add(&lc, &poly_neg(&rc)))
        }
        SymbolicExpr::Neg(inner) => Some(poly_neg(&symbolic_to_coeffs(inner, var)?)),
        SymbolicExpr::Mul(l, r) => {
            let lc = symbolic_to_coeffs(l, var)?;
            let rc = symbolic_to_coeffs(r, var)?;
            Some(poly_mul(&lc, &rc))
        }
        SymbolicExpr::Div(l, r) => {
            let k = r.as_const()?;
            if k == 0.0 {
                return None;
            }
            Some(poly_scale(&symbolic_to_coeffs(l, var)?, 1.0 / k))
        }
        SymbolicExpr::Pow(base, exp) => {
            let n = exp.as_const()?;
            if n < 0.0 || n.fract() != 0.0 || n > 64.0 {
                return None;
            }
            let bc = symbolic_to_coeffs(base, var)?;
            let mut out = vec![1.0];
            for _ in 0..(n as usize) {
                out = poly_mul(&out, &bc);
            }
            Some(out)
        }
        SymbolicExpr::Ln(_)
        | SymbolicExpr::Sin(_)
        | SymbolicExpr::Cos(_)
        | SymbolicExpr::Tan(_)
        | SymbolicExpr::Exp(_) => None,
    }
}

/// `polynomial::roots` 的输出形态转换为精确根列表。
fn exact_roots(result: crate::core::EvalResult) -> SolutionSet {
    match result {
        crate::core::EvalResult::Vector(v) => {
            SolutionSet::Exact(v.into_iter().map(|x| (x, 0.0)).collect())
        }
        crate::core::EvalResult::ComplexList(v) => SolutionSet::Exact(v),
        _ => SolutionSet::Exact(Vec::new()),
    }
}

/// 复数多项式求值（Horner 方案）。
fn eval_complex(coeffs: &[f64], x: Complex) -> Complex {
    let mut acc = Complex::new(0.0, 0.0);
    for &c in coeffs.iter().rev() {
        acc = acc.mul(x).add(Complex::new(c, 0.0));
    }
    acc
}

/// Durand-Kerner 同时迭代求全部复根（>4 次多项式）。
fn durand_kerner(coeffs: &[f64]) -> Vec<(f64, f64)> {
    let degree = coeffs.len() - 1;
    let mut roots: Vec<Complex> = (0..degree)
        .map(|k| Complex::new(0.4, 0.9).pow_nat(k + 1))
        .collect();

    for _ in 0..1000 {
        let mut next = Vec::with_capacity(degree);
        let mut max_step = 0.0_f64;
        for i in 0..degree {
            let p = eval_complex(coeffs, roots[i]);
            let mut den = Complex::new(1.0, 0.0);
            for (j, &r) in roots.iter().enumerate() {
                if i != j {
                    den = den.mul(roots[i].sub(r));
                }
            }
            if den.abs() < 1e-30 {
                next.push(roots[i]);
                continue;
            }
            let updated = roots[i].sub(p.div(den));
            max_step = max_step.max(updated.sub(roots[i]).abs());
            next.push(updated);
        }
        roots = next;
        if max_step < 1e-14 {
            break;
        }
    }

    roots
        .into_iter()
        .map(|z| {
            let im = if z.im.abs() < 1e-9 { 0.0 } else { z.im };
            (z.re, im)
        })
        .collect()
}

/// 区间内扫描定位变号小区间，再经二分法求全部实根。
pub fn solve_in_interval(
    diff: &SymbolicExpr,
    var: &str,
    a: f64,
    b: f64,
) -> Result<SolutionSet, CalcError> {
    if !a.is_finite() || !b.is_finite() || a > b {
        return Err(
            CalcError::domain(format!("invalid solve interval [{a}, {b}]"))
                .with_i18n("msg.solve.invalid_interval", vec![]),
        );
    }

    let f = |x: f64| {
        let mut env = HashMap::new();
        env.insert(var.to_string(), x);
        eval_symbolic(diff, &env).unwrap_or(f64::NAN)
    };

    const STEPS: usize = 256;
    const TOL: f64 = 1e-12;
    let mut roots: Vec<f64> = Vec::new();
    let width = (b - a) / STEPS as f64;
    let mut prev_x = a;
    let mut prev_f = f(prev_x);
    if prev_f == 0.0 {
        roots.push(prev_x);
    }
    for k in 1..=STEPS {
        let x = if k == STEPS { b } else { a + width * k as f64 };
        let fx = f(x);
        if fx == 0.0 {
            roots.push(x);
        } else if prev_f.is_finite()
            && fx.is_finite()
            && prev_f * fx < 0.0
            && let Ok(r) = bisection(f, prev_x, x, TOL, 200)
        {
            roots.push(r);
        }
        prev_x = x;
        prev_f = fx;
    }

    roots.sort_by(|l, r| l.partial_cmp(r).unwrap_or(std::cmp::Ordering::Equal));
    roots.dedup_by(|l, r| (*l - *r).abs() < 1e-8);
    if roots.is_empty() {
        return Err(
            CalcError::domain(format!("no root bracket found in [{a}, {b}]"))
                .with_i18n("msg.solve.no_bracket_root", vec![]),
        );
    }
    Ok(SolutionSet::Numeric(roots))
}

/// 求解 `lhs - rhs = 0`：多项式走精确根（≤4 次）/ Durand-Kerner（>4 次），
/// 非多项式在默认区间 `[-2π, 2π]` 扫描定界后数值求根。
pub fn solve_equation(
    lhs: &SymbolicExpr,
    rhs: &SymbolicExpr,
    var: &str,
) -> Result<SolutionSet, CalcError> {
    let diff = SymbolicExpr::Sub(Box::new(lhs.clone()), Box::new(rhs.clone()));
    if let Some(coeffs) = symbolic_to_coeffs(&diff, var) {
        let trimmed = polynomial::trim(&coeffs);
        if polynomial::is_zero(&trimmed) {
            return Err(CalcError::domain(
                "equation is an identity with infinite roots".to_string(),
            )
            .with_i18n("msg.solve.identity_infinite_roots", vec![]));
        }
        let degree = trimmed.len() - 1;
        if degree <= 4 {
            return Ok(exact_roots(polynomial::roots(&trimmed)?));
        }
        return Ok(SolutionSet::NumericComplex(durand_kerner(&trimmed)));
    }
    solve_in_interval(
        &diff,
        var,
        -2.0 * std::f64::consts::PI,
        2.0 * std::f64::consts::PI,
    )
}

/// 中心差分数值导数：与 `domains::numerical::derivative` 同算法，
/// 置于 math 层以避免 `math/` 反向依赖 `domains/`（分层约束）。
fn numeric_derivative(f: impl Fn(f64) -> f64, x: f64) -> f64 {
    let h = 1e-6 * (1.0 + x.abs());
    (f(x + h) - f(x - h)) / (2.0 * h)
}

/// 3 参形式 `solve(f(x)=0, x, a, b)`：扫描定界 + 二分，中心差分导数做 Newton 加速。
///
/// Newton 步若跳出区间或导数近零则保留二分根；区间内无变号返回带
/// `msg.solve.no_bracket_root` 键的域错误。
pub fn solve_equation_in_interval(
    lhs: &SymbolicExpr,
    rhs: &SymbolicExpr,
    var: &str,
    a: f64,
    b: f64,
) -> Result<SolutionSet, CalcError> {
    let diff = SymbolicExpr::Sub(Box::new(lhs.clone()), Box::new(rhs.clone()));
    let bracketed = solve_in_interval(&diff, var, a, b)?;
    let SolutionSet::Numeric(roots) = bracketed else {
        return Ok(bracketed);
    };

    let f = |x: f64| {
        let mut env = HashMap::new();
        env.insert(var.to_string(), x);
        eval_symbolic(&diff, &env).unwrap_or(f64::NAN)
    };

    let refined: Vec<f64> = roots
        .into_iter()
        .map(|root| {
            let mut x = root;
            for _ in 0..20 {
                let fx = f(x);
                if fx.abs() < 1e-14 {
                    break;
                }
                let dfx = numeric_derivative(f, x);
                if !dfx.is_finite() || dfx.abs() < 1e-12 {
                    break;
                }
                let next = x - fx / dfx;
                if !next.is_finite() || next < a || next > b {
                    break;
                }
                x = next;
            }
            x
        })
        .collect();
    Ok(SolutionSet::Numeric(refined))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::EvalResult;

    fn var(name: &str) -> SymbolicExpr {
        SymbolicExpr::Var(name.to_string())
    }
    fn konst(v: f64) -> SymbolicExpr {
        SymbolicExpr::Const(v)
    }

    #[test]
    fn test_solve_quadratic_exact_roots() {
        let lhs = SymbolicExpr::Sub(
            Box::new(SymbolicExpr::Pow(Box::new(var("x")), Box::new(konst(2.0)))),
            Box::new(konst(1.0)),
        );
        let got = solve_equation(&lhs, &konst(0.0), "x").unwrap();
        let SolutionSet::Exact(mut roots) = got else {
            panic!("expected exact roots");
        };
        roots.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        assert_eq!(roots.len(), 2);
        assert!((roots[0].0 + 1.0).abs() < 1e-12);
        assert!((roots[1].0 - 1.0).abs() < 1e-12);
        assert!(roots.iter().all(|r| r.1 == 0.0));
    }

    #[test]
    fn test_solve_quintic_numeric_roots_include_complex() {
        // (x-1)^5 = 0 → 五重根 1（数值解因重根收敛到 1±小虚部）
        let base = SymbolicExpr::Sub(Box::new(var("x")), Box::new(konst(1.0)));
        let lhs = SymbolicExpr::Pow(Box::new(base), Box::new(konst(5.0)));
        let got = solve_equation(&lhs, &konst(0.0), "x").unwrap();
        let SolutionSet::NumericComplex(roots) = got else {
            panic!("expected numeric complex roots");
        };
        assert_eq!(roots.len(), 5);
        assert!(roots.iter().any(|r| (r.0 - 1.0).abs() < 1e-3));
    }

    #[test]
    fn test_solve_x5_minus_one_numeric() {
        let lhs = SymbolicExpr::Sub(
            Box::new(SymbolicExpr::Pow(Box::new(var("x")), Box::new(konst(5.0)))),
            Box::new(konst(1.0)),
        );
        let got = solve_equation(&lhs, &konst(0.0), "x").unwrap();
        let SolutionSet::NumericComplex(roots) = got else {
            panic!("expected numeric complex roots");
        };
        assert_eq!(roots.len(), 5);
        assert!(
            roots
                .iter()
                .any(|r| (r.0 - 1.0).abs() < 1e-6 && r.1.abs() < 1e-6)
        );
        assert_eq!(roots.iter().filter(|r| r.1.abs() > 1e-6).count(), 4);
    }

    #[test]
    fn test_solve_transcendental_in_default_interval() {
        let lhs = SymbolicExpr::Sin(Box::new(var("x")));
        let got = solve_equation(&lhs, &konst(0.5), "x").unwrap();
        let SolutionSet::Numeric(roots) = got else {
            panic!("expected numeric roots");
        };
        assert!(
            roots
                .iter()
                .any(|r| (r - std::f64::consts::FRAC_PI_6).abs() < 1e-8)
        );
    }

    #[test]
    fn test_symbolic_to_coeffs_rejects_transcendental() {
        assert!(symbolic_to_coeffs(&SymbolicExpr::Sin(Box::new(var("x"))), "x").is_none());
        assert_eq!(
            symbolic_to_coeffs(
                &SymbolicExpr::Sub(Box::new(var("x")), Box::new(konst(3.0))),
                "x"
            ),
            Some(vec![-3.0, 1.0])
        );
    }

    #[test]
    fn test_exact_roots_from_vector() {
        let got = exact_roots(EvalResult::Vector(vec![2.0, -2.0]));
        assert_eq!(got, SolutionSet::Exact(vec![(2.0, 0.0), (-2.0, 0.0)]));
    }
    #[test]
    fn test_solve_cos_in_bracket_interval() {
        let got = solve_equation_in_interval(
            &SymbolicExpr::Cos(Box::new(var("x"))),
            &konst(0.0),
            "x",
            0.0,
            std::f64::consts::PI,
        )
        .unwrap();
        let SolutionSet::Numeric(roots) = got else {
            panic!("expected numeric roots");
        };
        assert_eq!(roots.len(), 1);
        assert!((roots[0] - std::f64::consts::FRAC_PI_2).abs() < 1e-8);
    }

    #[test]
    fn test_solve_interval_no_bracket_returns_domain_error() {
        // x^2 + 1 在 [0, 1] 内无实根，扫描不到变号区间
        let lhs = SymbolicExpr::Add(
            Box::new(SymbolicExpr::Pow(Box::new(var("x")), Box::new(konst(2.0)))),
            Box::new(konst(1.0)),
        );
        let err = solve_equation_in_interval(&lhs, &konst(0.0), "x", 0.0, 1.0).unwrap_err();
        assert_eq!(err.kind, crate::core::ErrorKind::Domain);
        assert!(err.to_string().contains("no root bracket"));
    }

    #[test]
    fn test_solve_interval_rejects_invalid_bounds() {
        let err = solve_equation_in_interval(&var("x"), &konst(0.0), "x", 2.0, 1.0).unwrap_err();
        assert_eq!(err.kind, crate::core::ErrorKind::Domain);
    }
}
