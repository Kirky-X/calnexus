// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! SymbolicMath trait 实现。

use crate::api::types::{Complex, Polynomial};
use crate::api::CalNexus;
use crate::core::{parse, CalcError, EvalResult};
use crate::math;
use crate::math::symbolic::{ast_to_symbolic, symbolic_to_string, SymbolicExpr};

/// SymbolicMath API 访问器。
pub struct SymbolicMathImpl<'a> {
    #[allow(dead_code)]
    pub(crate) cn: &'a CalNexus,
}

// ── 辅助 ──

/// 解析字符串表达式为 SymbolicExpr。
fn parse_to_symbolic(expr: &str) -> Result<SymbolicExpr, CalcError> {
    let ast = parse(expr)?;
    ast_to_symbolic(&ast)
}

impl<'a> SymbolicMathImpl<'a> {
    // ── 符号演算 ──

    pub fn differentiate(&self, expr: &str, var: &str) -> Result<EvalResult, CalcError> {
        let sym = parse_to_symbolic(expr)?;
        let result = math::symbolic::simplify(&math::symbolic::diff(&sym, var));
        Ok(EvalResult::Symbolic(symbolic_to_string(&result)))
    }

    pub fn integrate(&self, expr: &str, var: &str) -> Result<EvalResult, CalcError> {
        let sym = parse_to_symbolic(expr)?;
        let result = math::symbolic::simplify(&math::symbolic::integrate(&sym, var)?);
        Ok(EvalResult::Symbolic(symbolic_to_string(&result)))
    }

    pub fn simplify(&self, expr: &str) -> Result<EvalResult, CalcError> {
        let sym = parse_to_symbolic(expr)?;
        let result = math::symbolic::simplify(&sym);
        Ok(EvalResult::Symbolic(symbolic_to_string(&result)))
    }

    pub fn limit(&self, expr: &str, var: &str, target: f64) -> Result<EvalResult, CalcError> {
        let sym = parse_to_symbolic(expr)?;
        math::symbolic::limit(&sym, var, target)
    }

    pub fn taylor_expand(
        &self,
        expr: &str,
        var: &str,
        _center: f64,
        order: usize,
    ) -> Result<EvalResult, CalcError> {
        let sym = parse_to_symbolic(expr)?;
        // math::symbolic::taylor 在 x=0 处展开；非零 center 需后续支持
        if _center != 0.0 {
            return Err(CalcError::domain(
                "taylor_expand with non-zero center not yet available via direct API".to_string(),
            ));
        }
        math::symbolic::taylor(&sym, var, order as u32)
    }

    // ── 多项式 ──

    pub fn poly_add(
        &self,
        a: &Polynomial,
        b: &Polynomial,
    ) -> Result<EvalResult, CalcError> {
        Ok(EvalResult::Polynomial(math::polynomial::add(
            a.coeffs(),
            b.coeffs(),
        )))
    }

    pub fn poly_sub(
        &self,
        a: &Polynomial,
        b: &Polynomial,
    ) -> Result<EvalResult, CalcError> {
        Ok(EvalResult::Polynomial(math::polynomial::sub(
            a.coeffs(),
            b.coeffs(),
        )))
    }

    pub fn poly_mul(
        &self,
        a: &Polynomial,
        b: &Polynomial,
    ) -> Result<EvalResult, CalcError> {
        Ok(EvalResult::Polynomial(math::polynomial::mul(
            a.coeffs(),
            b.coeffs(),
        )))
    }

    pub fn poly_div(
        &self,
        a: &Polynomial,
        b: &Polynomial,
    ) -> Result<EvalResult, CalcError> {
        let (quotient, _remainder) = math::polynomial::div(a.coeffs(), b.coeffs());
        Ok(EvalResult::Polynomial(quotient))
    }

    pub fn poly_roots(&self, p: &Polynomial) -> Result<EvalResult, CalcError> {
        math::polynomial::roots(p.coeffs())
    }

    pub fn poly_eval(&self, p: &Polynomial, x: f64) -> Result<EvalResult, CalcError> {
        math::polynomial::eval(p.coeffs(), x).map(EvalResult::Scalar)
    }

    // ── 复数 ──

    pub fn complex_add(
        &self,
        a: &Complex,
        b: &Complex,
    ) -> Result<EvalResult, CalcError> {
        let r = math::complex::add(a.value(), b.value());
        Ok(EvalResult::Complex(r.re, r.im))
    }

    pub fn complex_sub(
        &self,
        a: &Complex,
        b: &Complex,
    ) -> Result<EvalResult, CalcError> {
        let r = math::complex::sub(a.value(), b.value());
        Ok(EvalResult::Complex(r.re, r.im))
    }

    pub fn complex_mul(
        &self,
        a: &Complex,
        b: &Complex,
    ) -> Result<EvalResult, CalcError> {
        let r = math::complex::mul(a.value(), b.value());
        Ok(EvalResult::Complex(r.re, r.im))
    }

    pub fn complex_div(
        &self,
        a: &Complex,
        b: &Complex,
    ) -> Result<EvalResult, CalcError> {
        let r = math::complex::div(a.value(), b.value())?;
        Ok(EvalResult::Complex(r.re, r.im))
    }

    pub fn complex_abs(&self, z: &Complex) -> Result<EvalResult, CalcError> {
        Ok(EvalResult::Scalar(math::complex::norm(z.value())))
    }

    pub fn complex_arg(&self, z: &Complex) -> Result<EvalResult, CalcError> {
        math::complex::arg(z.value()).map(EvalResult::Scalar)
    }

    pub fn complex_conj(&self, z: &Complex) -> Result<EvalResult, CalcError> {
        let r = math::complex::conj(z.value());
        Ok(EvalResult::Complex(r.re, r.im))
    }

    pub fn complex_exp(&self, z: &Complex) -> Result<EvalResult, CalcError> {
        let r = math::complex::exp(z.value());
        Ok(EvalResult::Complex(r.re, r.im))
    }

    pub fn complex_ln(&self, z: &Complex) -> Result<EvalResult, CalcError> {
        let r = math::complex::ln(z.value());
        Ok(EvalResult::Complex(r.re, r.im))
    }

    // ── 方程求解 ──

    pub fn solve_equation(
        &self,
        expr: &str,
        var: &str,
        method: &str,
        options: Option<&[f64]>,
    ) -> Result<EvalResult, CalcError> {
        let ctx = self.cn.ctx.read().unwrap();
        let cache = crate::core::CacheManager::new();
        let expr_owned = expr.to_string();
        let var_owned = var.to_string();

        // 构建 f(x) 闭包：设置变量值后求值
        let f = |x: f64| -> f64 {
            let mut local_ctx = ctx.clone();
            local_ctx.vars.insert(var_owned.clone(), x);
            match crate::core::evaluate(&expr_owned, &local_ctx, None, &cache) {
                Ok((EvalResult::Scalar(v), _, _, _)) => v,
                _ => f64::NAN,
            }
        };

        let opts = options.unwrap_or(&[]);
        match method {
            "newton" => {
                let x0 = opts.first().copied().unwrap_or(1.0);
                // 数值导数
                let h = 1e-8;
                let df = |x: f64| -> f64 { (f(x + h) - f(x - h)) / (2.0 * h) };
                let root = math::solvers::newton_raphson(&f, df, x0, 1e-12, 200)?;
                Ok(EvalResult::Scalar(root))
            }
            "bisection" => {
                if opts.len() < 2 {
                    return Err(CalcError::domain(
                        "bisection requires options [a, b]".to_string(),
                    ));
                }
                let root = math::solvers::bisection(&f, opts[0], opts[1], 1e-12, 200)?;
                Ok(EvalResult::Scalar(root))
            }
            "brent" => {
                if opts.len() < 2 {
                    return Err(CalcError::domain(
                        "brent requires options [a, b]".to_string(),
                    ));
                }
                let root = math::solvers::brent(&f, opts[0], opts[1], 1e-12, 200)?;
                Ok(EvalResult::Scalar(root))
            }
            _ => Err(CalcError::domain(format!(
                "unknown solver method: '{}'. Available: newton, bisection, brent",
                method
            ))),
        }
    }
}
