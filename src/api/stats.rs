// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! DataAnalysis trait 实现。

use std::collections::HashMap;

use crate::api::CalNexus;
use crate::core::{CalcError, EvalResult};
use crate::math;

/// DataAnalysis API 访问器。
pub struct DataAnalysisImpl<'a> {
    pub(crate) cn: &'a CalNexus,
}

// ── 辅助 ──

fn scalar(v: f64) -> EvalResult {
    EvalResult::Scalar(v)
}

/// 将 HashMap<String, f64> 统计检验结果转为 EvalResult::Vector（值列表）。
fn hashmap_to_vector(hm: HashMap<String, f64>) -> EvalResult {
    let vals: Vec<f64> = hm.values().copied().collect();
    EvalResult::Vector(vals)
}

impl<'a> DataAnalysisImpl<'a> {
    // ── 基础统计 ──

    pub fn mean(&self, data: &[f64]) -> Result<EvalResult, CalcError> {
        Ok(scalar(math::statistics::mean(data)))
    }

    pub fn variance(&self, data: &[f64]) -> Result<EvalResult, CalcError> {
        Ok(scalar(math::statistics::variance(data)))
    }

    pub fn std(&self, data: &[f64]) -> Result<EvalResult, CalcError> {
        Ok(scalar(math::statistics::std(data)))
    }

    pub fn median(&self, data: &[f64]) -> Result<EvalResult, CalcError> {
        Ok(scalar(math::statistics::median(data)))
    }

    pub fn min(&self, data: &[f64]) -> Result<EvalResult, CalcError> {
        Ok(scalar(math::statistics::min(data)))
    }

    pub fn max(&self, data: &[f64]) -> Result<EvalResult, CalcError> {
        Ok(scalar(math::statistics::max(data)))
    }

    pub fn sum(&self, data: &[f64]) -> Result<EvalResult, CalcError> {
        Ok(scalar(math::statistics::sum(data)))
    }

    pub fn count(&self, data: &[f64]) -> Result<EvalResult, CalcError> {
        Ok(scalar(math::statistics::count(data)))
    }

    // ── 分布函数 ──

    pub fn norm_pdf(&self, x: f64, mu: f64, sigma: f64) -> Result<EvalResult, CalcError> {
        Ok(scalar(math::statistics::norm_pdf(x, mu, sigma)))
    }

    pub fn norm_cdf(&self, x: f64, mu: f64, sigma: f64) -> Result<EvalResult, CalcError> {
        Ok(scalar(math::statistics::norm_cdf(x, mu, sigma)))
    }

    pub fn norm_inv(&self, p: f64, mu: f64, sigma: f64) -> Result<EvalResult, CalcError> {
        Ok(scalar(math::statistics::norm_inv(p, mu, sigma)))
    }

    pub fn t_pdf(&self, x: f64, df: f64) -> Result<EvalResult, CalcError> {
        Ok(scalar(math::statistics::t_pdf(x, df)))
    }

    pub fn t_cdf(&self, x: f64, df: f64) -> Result<EvalResult, CalcError> {
        Ok(scalar(math::statistics::t_cdf(x, df)))
    }

    pub fn t_inv(&self, p: f64, df: f64) -> Result<EvalResult, CalcError> {
        Ok(scalar(math::statistics::t_inv(p, df)))
    }

    pub fn chi2_pdf(&self, x: f64, k: f64) -> Result<EvalResult, CalcError> {
        Ok(scalar(math::statistics::chi2_pdf(x, k)))
    }

    pub fn chi2_cdf(&self, x: f64, k: f64) -> Result<EvalResult, CalcError> {
        Ok(scalar(math::statistics::chi2_cdf(x, k)))
    }

    pub fn chi2_inv(&self, p: f64, k: f64) -> Result<EvalResult, CalcError> {
        Ok(scalar(math::statistics::chi2_inv(p, k)))
    }

    pub fn f_pdf(&self, x: f64, d1: f64, d2: f64) -> Result<EvalResult, CalcError> {
        Ok(scalar(math::statistics::f_pdf(x, d1, d2)))
    }

    pub fn f_cdf(&self, x: f64, d1: f64, d2: f64) -> Result<EvalResult, CalcError> {
        Ok(scalar(math::statistics::f_cdf(x, d1, d2)))
    }

    pub fn f_inv(&self, p: f64, d1: f64, d2: f64) -> Result<EvalResult, CalcError> {
        Ok(scalar(math::statistics::f_inv(p, d1, d2)))
    }

    pub fn poisson_pmf(&self, k: f64, lambda: f64) -> Result<EvalResult, CalcError> {
        Ok(scalar(math::statistics::poisson_pmf(k, lambda)))
    }

    pub fn poisson_cdf(&self, k: f64, lambda: f64) -> Result<EvalResult, CalcError> {
        Ok(scalar(math::statistics::poisson_cdf(k, lambda)))
    }

    pub fn binom_pmf(&self, k: f64, n: f64, p: f64) -> Result<EvalResult, CalcError> {
        Ok(scalar(math::statistics::binom_pmf(k, n, p)))
    }

    pub fn binom_cdf(&self, k: f64, n: f64, p: f64) -> Result<EvalResult, CalcError> {
        Ok(scalar(math::statistics::binom_cdf(k, n, p)))
    }

    // ── 假设检验 ──

    pub fn t_test_one(&self, data: &[f64], mu: f64) -> Result<EvalResult, CalcError> {
        Ok(hashmap_to_vector(math::statistics::t_test_one(data, mu)))
    }

    pub fn t_test_two(&self, a: &[f64], b: &[f64]) -> Result<EvalResult, CalcError> {
        Ok(hashmap_to_vector(math::statistics::t_test_two(a, b)))
    }

    pub fn chi2_test(&self, observed: &[f64], expected: &[f64]) -> Result<EvalResult, CalcError> {
        Ok(hashmap_to_vector(
            math::statistics::chi2_test(observed, expected),
        ))
    }

    // ── 相关 ──

    pub fn pearson(&self, x: &[f64], y: &[f64]) -> Result<EvalResult, CalcError> {
        Ok(scalar(math::statistics::pearson(x, y)))
    }

    pub fn spearman(&self, x: &[f64], y: &[f64]) -> Result<EvalResult, CalcError> {
        Ok(scalar(math::statistics::spearman(x, y)))
    }
}
