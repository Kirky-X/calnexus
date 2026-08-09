// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! ScalarMath trait 实现。

use num_bigint::BigInt;

use crate::api::types::BigNumber;
use crate::api::CalNexus;
use crate::core::{CalcError, EvalResult};
use crate::math;

/// ScalarMath API 访问器。
pub struct ScalarMathImpl<'a> {
    #[allow(dead_code)]
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

    // ── 精度 ──

    pub fn precision_eval(&self, digits: usize, expr: &str) -> Result<EvalResult, CalcError> {
        let ctx = self.cn.ctx.read().unwrap();
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
        Ok(EvalResult::Vector(primes.iter().map(|&p| p as f64).collect()))
    }

    pub fn mod_pow(
        &self,
        base: &BigNumber,
        exp: &BigNumber,
        m: &BigNumber,
    ) -> Result<EvalResult, CalcError> {
        math::number_theory::mod_pow(to_bigint(base), to_bigint(exp), to_bigint(m)).map(big)
    }

    pub fn mod_inverse(
        &self,
        a: &BigNumber,
        m: &BigNumber,
    ) -> Result<EvalResult, CalcError> {
        math::number_theory::mod_inverse(to_bigint(a), to_bigint(m)).map(big)
    }

    pub fn euler_phi(&self, n: &BigNumber) -> Result<EvalResult, CalcError> {
        Ok(big(math::number_theory::euler_phi(to_bigint(n))))
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
