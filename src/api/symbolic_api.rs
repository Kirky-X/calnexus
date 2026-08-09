// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! SymbolicMath trait 实现。

use crate::api::types::{Complex, Polynomial};
use crate::api::CalNexus;
use crate::core::{parse, CalcError, EvalResult};
use crate::math;
use crate::math::symbolic::{ast_to_symbolic, symbolic_to_string, SymbolicExpr};

/// SymbolicMath API 访问器。
pub struct SymbolicMathImpl<'a> {
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
}
