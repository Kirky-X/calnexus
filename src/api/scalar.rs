// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! ScalarMath trait 实现。

use num_bigint::BigInt;

use crate::api::CalNexus;
use crate::api::types::BigNumber;
use crate::core::{CalcError, EvalResult};
use crate::math;

/// ScalarMath API 访问器。
pub struct ScalarMathImpl<'a> {
    pub(crate) cn: &'a CalNexus,
}

// ── 辅助 ──

fn scalar(v: f64) -> EvalResult {
    EvalResult::Scalar(v)
}

fn big(v: BigInt) -> EvalResult {
    EvalResult::BigInt(v)
}

fn to_bigint(b: &BigNumber) -> &BigInt {
    b.value()
}

impl<'a> ScalarMathImpl<'a> {
    // ── 算术 ──

    pub fn add(&self, a: f64, b: f64) -> Result<EvalResult, CalcError> {
        math::arithmetic::add(a, b).map(scalar)
    }

    pub fn sub(&self, a: f64, b: f64) -> Result<EvalResult, CalcError> {
        math::arithmetic::sub(a, b).map(scalar)
    }

    pub fn mul(&self, a: f64, b: f64) -> Result<EvalResult, CalcError> {
        math::arithmetic::mul(a, b).map(scalar)
    }

    pub fn div(&self, a: f64, b: f64) -> Result<EvalResult, CalcError> {
        math::arithmetic::div(a, b).map(scalar)
    }

    pub fn pow(&self, a: f64, b: f64) -> Result<EvalResult, CalcError> {
        math::arithmetic::pow(a, b).map(scalar)
    }

    pub fn rem(&self, a: f64, b: f64) -> Result<EvalResult, CalcError> {
        math::arithmetic::rem(a, b).map(scalar)
    }

    pub fn factorial(&self, n: u64) -> Result<EvalResult, CalcError> {
        math::arithmetic::factorial(n as f64).map(scalar)
    }

    pub fn abs(&self, x: f64) -> Result<EvalResult, CalcError> {
        Ok(scalar(math::arithmetic::abs(x)))
    }

    // ── 科学函数 ──

    pub fn sin(&self, x: f64) -> Result<EvalResult, CalcError> {
        math::scientific::sin(x).map(scalar)
    }

    pub fn cos(&self, x: f64) -> Result<EvalResult, CalcError> {
        math::scientific::cos(x).map(scalar)
    }

    pub fn tan(&self, x: f64) -> Result<EvalResult, CalcError> {
        math::scientific::tan(x).map(scalar)
    }

    pub fn asin(&self, x: f64) -> Result<EvalResult, CalcError> {
        math::scientific::asin(x).map(scalar)
    }

    pub fn acos(&self, x: f64) -> Result<EvalResult, CalcError> {
        math::scientific::acos(x).map(scalar)
    }

    pub fn atan(&self, x: f64) -> Result<EvalResult, CalcError> {
        math::scientific::atan(x).map(scalar)
    }

    pub fn ln(&self, x: f64) -> Result<EvalResult, CalcError> {
        math::scientific::ln(x).map(scalar)
    }

    pub fn log(&self, x: f64, base: f64) -> Result<EvalResult, CalcError> {
        math::scientific::log(x, base).map(scalar)
    }

    pub fn exp(&self, x: f64) -> Result<EvalResult, CalcError> {
        math::scientific::exp(x).map(scalar)
    }

    pub fn sinh(&self, x: f64) -> Result<EvalResult, CalcError> {
        math::scientific::sinh(x).map(scalar)
    }

    pub fn cosh(&self, x: f64) -> Result<EvalResult, CalcError> {
        math::scientific::cosh(x).map(scalar)
    }

    pub fn tanh(&self, x: f64) -> Result<EvalResult, CalcError> {
        math::scientific::tanh(x).map(scalar)
    }

    pub fn gamma(&self, x: f64) -> Result<EvalResult, CalcError> {
        math::scientific::gamma(x).map(scalar)
    }

    pub fn erf(&self, x: f64) -> Result<EvalResult, CalcError> {
        math::scientific::erf(x).map(scalar)
    }

    /// 数值微分：对表达式 `f` 关于变量 `var` 在 `x` 处的导数（中心差分 + Richardson 外推）。
    #[cfg(feature = "numerical")]
    pub fn derivative(&self, f: &str, var: &str, x: f64) -> Result<EvalResult, CalcError> {
        crate::domains::derivative(f, var, x).map(scalar)
    }

    pub fn bessel_j0(&self, x: f64) -> Result<EvalResult, CalcError> {
        math::scientific::bessel_j0(x).map(scalar)
    }

    pub fn bessel_j1(&self, x: f64) -> Result<EvalResult, CalcError> {
        math::scientific::bessel_j1(x).map(scalar)
    }

    pub fn bessel_j(&self, n: u32, x: f64) -> Result<EvalResult, CalcError> {
        math::scientific::bessel_j(n, x).map(scalar)
    }

    pub fn bessel_y0(&self, x: f64) -> Result<EvalResult, CalcError> {
        math::scientific::bessel_y0(x).map(scalar)
    }

    pub fn bessel_y1(&self, x: f64) -> Result<EvalResult, CalcError> {
        math::scientific::bessel_y1(x).map(scalar)
    }

    pub fn bessel_y(&self, n: u32, x: f64) -> Result<EvalResult, CalcError> {
        math::scientific::bessel_y(n, x).map(scalar)
    }

    pub fn bessel_i0(&self, x: f64) -> Result<EvalResult, CalcError> {
        math::scientific::bessel_i0(x).map(scalar)
    }

    pub fn bessel_i1(&self, x: f64) -> Result<EvalResult, CalcError> {
        math::scientific::bessel_i1(x).map(scalar)
    }

    pub fn bessel_i(&self, n: u32, x: f64) -> Result<EvalResult, CalcError> {
        math::scientific::bessel_i(n, x).map(scalar)
    }

    pub fn bessel_k0(&self, x: f64) -> Result<EvalResult, CalcError> {
        math::scientific::bessel_k0(x).map(scalar)
    }

    pub fn bessel_k1(&self, x: f64) -> Result<EvalResult, CalcError> {
        math::scientific::bessel_k1(x).map(scalar)
    }

    pub fn bessel_k(&self, n: u32, x: f64) -> Result<EvalResult, CalcError> {
        math::scientific::bessel_k(n, x).map(scalar)
    }

    pub fn beta(&self, a: f64, b: f64) -> Result<EvalResult, CalcError> {
        math::scientific::beta(a, b).map(scalar)
    }

    pub fn zeta(&self, s: f64) -> Result<EvalResult, CalcError> {
        math::scientific::zeta(s).map(scalar)
    }

    pub fn airy_ai(&self, x: f64) -> Result<EvalResult, CalcError> {
        math::scientific::airy_ai(x).map(scalar)
    }

    pub fn airy_bi(&self, x: f64) -> Result<EvalResult, CalcError> {
        math::scientific::airy_bi(x).map(scalar)
    }

    // ── 精度 ──

    pub fn precision_eval(&self, digits: usize, expr: &str) -> Result<EvalResult, CalcError> {
        let ctx = self
            .cn
            .ctx
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let cache = crate::core::CacheManager::new();
        let (result, _, _, _) = crate::core::evaluate(expr, &ctx, Some(digits), &cache)?;
        Ok(result)
    }

    // ── 数论 ──

    pub fn gcd(&self, a: &BigNumber, b: &BigNumber) -> Result<EvalResult, CalcError> {
        Ok(big(math::number_theory::gcd(to_bigint(a), to_bigint(b))))
    }

    pub fn lcm(&self, a: &BigNumber, b: &BigNumber) -> Result<EvalResult, CalcError> {
        Ok(big(math::number_theory::lcm(to_bigint(a), to_bigint(b))))
    }

    pub fn is_prime(&self, n: &BigNumber) -> Result<EvalResult, CalcError> {
        Ok(scalar(if math::number_theory::is_prime(to_bigint(n)) {
            1.0
        } else {
            0.0
        }))
    }

    pub fn prime_sieve(&self, n: u64) -> Result<EvalResult, CalcError> {
        let big_n = BigInt::from(n);
        let primes = math::number_theory::prime_sieve(&big_n)?;
        Ok(EvalResult::Vector(
            primes.iter().map(|&p| p as f64).collect(),
        ))
    }

    pub fn mod_pow(
        &self,
        base: &BigNumber,
        exp: &BigNumber,
        m: &BigNumber,
    ) -> Result<EvalResult, CalcError> {
        math::number_theory::mod_pow(to_bigint(base), to_bigint(exp), to_bigint(m)).map(big)
    }

    pub fn mod_inverse(&self, a: &BigNumber, m: &BigNumber) -> Result<EvalResult, CalcError> {
        math::number_theory::mod_inverse(to_bigint(a), to_bigint(m)).map(big)
    }

    pub fn euler_phi(&self, n: &BigNumber) -> Result<EvalResult, CalcError> {
        Ok(big(math::number_theory::euler_phi(to_bigint(n))))
    }

    pub fn crt(
        &self,
        remainders: &[BigNumber],
        moduli: &[BigNumber],
    ) -> Result<EvalResult, CalcError> {
        let r: Vec<BigInt> = remainders.iter().map(|b| b.value().clone()).collect();
        let m: Vec<BigInt> = moduli.iter().map(|b| b.value().clone()).collect();
        math::number_theory::crt(&r, &m).map(big)
    }

    pub fn discrete_log(
        &self,
        g: &BigNumber,
        h: &BigNumber,
        p: &BigNumber,
    ) -> Result<EvalResult, CalcError> {
        math::number_theory::discrete_log(to_bigint(g), to_bigint(h), to_bigint(p)).map(big)
    }

    // ── 组合 ──

    pub fn perm(&self, n: u64, k: u64) -> Result<EvalResult, CalcError> {
        let bn = BigInt::from(n);
        let bk = BigInt::from(k);
        math::combinatorics::perm(&bn, &bk).map(big)
    }

    pub fn comb(&self, n: u64, k: u64) -> Result<EvalResult, CalcError> {
        let bn = BigInt::from(n);
        let bk = BigInt::from(k);
        math::combinatorics::comb(&bn, &bk).map(big)
    }

    pub fn catalan(&self, n: u64) -> Result<EvalResult, CalcError> {
        let bn = BigInt::from(n);
        math::combinatorics::catalan(&bn).map(big)
    }

    pub fn stirling_first(&self, n: u64, k: u64) -> Result<EvalResult, CalcError> {
        let bn = BigInt::from(n);
        let bk = BigInt::from(k);
        math::combinatorics::stirling_first(&bn, &bk).map(big)
    }

    pub fn stirling_second(&self, n: u64, k: u64) -> Result<EvalResult, CalcError> {
        let bn = BigInt::from(n);
        let bk = BigInt::from(k);
        math::combinatorics::stirling_second(&bn, &bk).map(big)
    }
}

#[cfg(all(test, feature = "numerical"))]
mod tests {
    use super::*;

    /// 公开可达：x^2 关于 x 在 3 处的导数为 6。
    #[test]
    fn derivative_quadratic() {
        let cn = CalNexus::new();
        let r = cn.scalar().derivative("x^2", "x", 3.0).unwrap();
        match r {
            EvalResult::Scalar(v) => assert!((v - 6.0).abs() < 1e-6, "d={v}"),
            other => panic!("expected scalar, got {other:?}"),
        }
    }

    /// sin'(0) = 1。
    #[test]
    fn derivative_sin_at_zero() {
        let cn = CalNexus::new();
        let r = cn.scalar().derivative("sin(x)", "x", 0.0).unwrap();
        match r {
            EvalResult::Scalar(v) => assert!((v - 1.0).abs() < 1e-6, "d={v}"),
            other => panic!("expected scalar, got {other:?}"),
        }
    }

    /// 表达式求值结果非标量时返回 Domain 错误，并挂 i18n 键。
    #[test]
    fn derivative_non_scalar_expression_is_domain_error() {
        let cn = CalNexus::new();
        let err = cn
            .scalar()
            .derivative("[[1,2],[3,4]]", "x", 0.0)
            .unwrap_err();
        assert_eq!(err.kind, crate::core::ErrorKind::Domain, "err={err:?}");
        assert_eq!(
            err.i18n_key,
            Some("msg.numerical.derivative_not_scalar"),
            "i18n key not attached: {err:?}"
        );
    }
}
