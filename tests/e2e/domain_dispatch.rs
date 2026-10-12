// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
// CalcError 144 字节，函数签名触发 result_large_err——与 lib.rs 同论证
#![allow(clippy::result_large_err)]

//! 域级函数分派矩阵：经公共 `calnexus::evaluate()` 驱动统计/时间/数论/组合域的
//! 白名单函数与错误分支（低频分布参数、退化输入、参数类型错误）。

use super::common::{eval_err, eval_ok};
use calnexus::{ErrorKind, EvalResult};

fn scalar_of(r: EvalResult) -> f64 {
    match r {
        EvalResult::Scalar(v) => v,
        other => panic!("expected Scalar, got {other:?}"),
    }
}

fn approx(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "expected {b}, got {a}");
}

fn assert_domain_err(expr: &str) {
    assert_err_kind(expr, ErrorKind::Domain);
}

fn assert_err_kind(expr: &str, kind: ErrorKind) {
    let e = eval_err(expr);
    assert_eq!(e.kind, kind, "`{expr}` should be {kind:?}, got: {e:?}");
}

#[test]
fn statistics_distribution_functions_end_to_end() {
    let norm_inv = scalar_of(eval_ok("norm_inv(0.975, 0, 1)"));
    assert!((norm_inv - 1.959963984540054).abs() < 1e-6);
    approx(scalar_of(eval_ok("t_pdf(0, 5)")), 0.37960668982249444);
    approx(scalar_of(eval_ok("t_cdf(0, 5)")), 0.5);
    let t_inv = scalar_of(eval_ok("t_inv(0.975, 10)"));
    assert!((t_inv - 2.2281388519649385).abs() < 1e-6);
    approx(scalar_of(eval_ok("chi2_pdf(1, 1)")), 0.24197072451914337);
    let chi2_inv = scalar_of(eval_ok("chi2_inv(0.95, 1)"));
    assert!((chi2_inv - 3.841458820694124).abs() < 1e-4);
    approx(scalar_of(eval_ok("f_pdf(1, 2, 2)")), 0.25);
    approx(scalar_of(eval_ok("f_cdf(1, 2, 2)")), 0.5);
    let f_inv = scalar_of(eval_ok("f_inv(0.5, 2, 2)"));
    assert!((f_inv - 1.0).abs() < 1e-4);
    approx(
        scalar_of(eval_ok("poisson_pmf(2, 2)")),
        2.0 * (-2.0f64).exp(),
    );
    approx(
        scalar_of(eval_ok("poisson_cdf(2, 2)")),
        5.0 * (-2.0f64).exp(),
    );
    approx(scalar_of(eval_ok("binom_pmf(2, 4, 0.5)")), 0.375);
    approx(scalar_of(eval_ok("binom_cdf(2, 4, 0.5)")), 0.6875);
}

#[test]
fn statistics_distribution_low_frequency_parameters() {
    approx(
        scalar_of(eval_ok("norm_cdf(-1, 0, 1)")),
        0.15865525393145707,
    );
    let low = scalar_of(eval_ok("norm_inv(0.01, 0, 1)"));
    assert!((low - (-2.3263478740408408)).abs() < 1e-6);
    let high = scalar_of(eval_ok("norm_inv(0.99, 0, 1)"));
    assert!((high - 2.3263478740408408).abs() < 1e-6);
    approx(scalar_of(eval_ok("chi2_pdf(0, 3)")), 0.0);
    approx(scalar_of(eval_ok("f_pdf(0, 2, 2)")), 1.0);
    approx(scalar_of(eval_ok("f_pdf(0, 3, 2)")), 0.0);
    approx(scalar_of(eval_ok("f_cdf(0, 1, 2)")), 0.0);
    approx(scalar_of(eval_ok("poisson_pmf(-1, 2)")), 0.0);
    approx(scalar_of(eval_ok("poisson_pmf(2.5, 2)")), 0.0);
    approx(scalar_of(eval_ok("poisson_cdf(-1, 2)")), 0.0);
    approx(scalar_of(eval_ok("binom_pmf(-1, 4, 0.5)")), 0.0);
    approx(scalar_of(eval_ok("binom_pmf(5, 4, 0.5)")), 0.0);
    approx(scalar_of(eval_ok("binom_cdf(-1, 4, 0.5)")), 0.0);
    let tiny = scalar_of(eval_ok("f_inv(0.001, 1, 1)"));
    assert!(tiny.is_finite() && tiny > 0.0);
    let extreme = scalar_of(eval_ok("chi2_inv(0.000001, 1)"));
    assert!(extreme.is_finite());
    let saturated = scalar_of(eval_ok("chi2_inv(0.99999999, 1)"));
    assert!(saturated.is_finite());
}

#[test]
fn statistics_distribution_degenerate_inputs_error() {
    assert_err_kind("norm_inv(0, 0, 1)", ErrorKind::NaNOrInf);
    assert_err_kind("norm_inv(1.5, 0, 1)", ErrorKind::NaNOrInf);
    assert_err_kind("t_inv(0, 5)", ErrorKind::NaNOrInf);
    assert_err_kind("chi2_inv(0, 3)", ErrorKind::NaNOrInf);
    assert_err_kind("f_inv(0, 1, 1)", ErrorKind::NaNOrInf);
    assert_err_kind("f_pdf(0, 1, 2)", ErrorKind::NaNOrInf);
}

#[test]
fn statistics_test_and_correlation_error_branches() {
    assert!(matches!(
        eval_ok("t_test_two([1, 2, 3], [2, 4, 6])"),
        EvalResult::Json(_)
    ));
    assert!(matches!(
        eval_ok("chi2_test([10, 20, 30], [20, 20, 20])"),
        EvalResult::Json(_)
    ));
    approx(scalar_of(eval_ok("spearman([1, 2, 3], [2, 4, 6])")), 1.0);
    assert_domain_err("t_test_one([], 0)");
    assert_domain_err("t_test_two([], [])");
    assert_domain_err("chi2_test([], [])");
    assert_domain_err("chi2_test([1, 2], [1])");
    assert_domain_err("pearson([1, 2, 3], [1, 2])");
    assert_domain_err("pearson([5], [6])");
    assert_domain_err("spearman([1, 2, 3], [1, 2])");
    assert_domain_err("spearman([5], [6])");
    assert_domain_err("t_test_one(3, 0)");
    assert_domain_err("pearson([1, t_test_one([1], 0)], [1, 2])");
    assert_domain_err("norm_pdf(1, 2)");
    assert_domain_err("norm_pdf([1], 2, 3)");
}

#[test]
fn statistics_domain_structure_error_branches() {
    assert_domain_err("mean([1, 2]) + foo(1)");
    assert_domain_err("mean([1, 2, 3]) + t_test_one([1, 2, 3], 0)");
    assert_domain_err("-t_test_one([1, 2, 3], 0)");
    assert_domain_err("(mean([1, 2, 3]))!");
}

#[test]
fn number_theory_error_branches_end_to_end() {
    assert_domain_err("crt([1, 2], [5, 5])");
    assert_domain_err("crt([1], [0])");
    assert_domain_err("discrete_log(0, 1, 7)");
    assert_domain_err("discrete_log(2, 3, 7)");
    assert_err_kind("euler_phi(x)", ErrorKind::Eval);
    assert_domain_err("euler_phi(10) + 1.5");
    assert_err_kind("C(x, 2)", ErrorKind::Eval);
    assert_domain_err("catalan(3) + 1.5");
}

#[test]
fn number_theory_prime_sieve_overflow_errors() {
    let e = eval_err("prime_sieve(20000000)");
    assert_eq!(
        e.kind,
        ErrorKind::Overflow,
        "sieve over 10^7 bound must fail: {e:?}"
    );
}

#[cfg(feature = "unit")]
#[test]
fn unit_domain_unknown_function_rejected() {
    assert_domain_err("convert(1, \"km\", \"m\") + foo(1)");
}

#[cfg(feature = "time")]
mod time_dispatch {
    use super::{approx, assert_domain_err, assert_err_kind, eval_ok, scalar_of};
    use crate::common::eval_ok_with_domain;
    use calnexus::{ErrorKind, EvalResult};

    #[test]
    fn time_arithmetic_units_end_to_end() {
        assert!(matches!(
            eval_ok("date_add(\"2026-01-15T10:00:00Z\", 90, \"second\")"),
            EvalResult::DateTime(_)
        ));
        assert!(matches!(
            eval_ok("date_add(\"2026-01-15\", 2, \"week\")"),
            EvalResult::DateTime(_)
        ));
        assert!(matches!(
            eval_ok("date_add(\"2026-01-15\", 3, \"year\")"),
            EvalResult::DateTime(_)
        ));
        approx(
            scalar_of(eval_ok(
                "date_diff(\"2026-01-15T02:00:00Z\", \"2026-01-15T00:00:00Z\", \"minute\")",
            )),
            -120.0,
        );
        approx(
            scalar_of(eval_ok(
                "date_diff(\"2026-01-15\", \"2026-01-01\", \"week\")",
            )),
            -2.0,
        );
        approx(
            scalar_of(eval_ok(
                "date_diff(\"2026-01-16\", \"2026-01-01\", \"hour\")",
            )),
            -360.0,
        );
    }

    #[test]
    fn time_format_pipeline_end_to_end() {
        match eval_ok("format_date(\"2026-01-15T00:00:00Z\", \"%Y\", \"Asia/Shanghai\")") {
            EvalResult::Symbolic(s) => assert_eq!(s, "2026"),
            other => panic!("expected Symbolic, got {other:?}"),
        }
        // reformat Zoned 分支：%s 仅 Zoned 可解析
        match eval_ok("reformat_date(\"1768435200\", \"%s\", \"%Y-%m-%d\")") {
            EvalResult::Symbolic(s) => assert_eq!(s, "2026-01-15"),
            other => panic!("expected Symbolic, got {other:?}"),
        }
        // format_date 嵌套标量（星期数）当 Unix 秒用
        assert!(matches!(
            eval_ok("format_date(weekday(\"2026-01-15\"), \"%Y\")"),
            EvalResult::Symbolic(_)
        ));
        // DateTime → Unix 秒（eval_to_f64 DateTime 分支，纯日期补午夜）
        approx(
            scalar_of(eval_ok("datetime(\"2026-01-15\") + 0")),
            1768435200.0,
        );
    }

    #[test]
    fn time_parse_multi_format_end_to_end() {
        assert!(matches!(
            eval_ok("date(\"2026年7月25日\")"),
            EvalResult::DateTime(_)
        ));
        assert!(matches!(
            eval_ok("datetime(\"2026年7月25日 12:30\")"),
            EvalResult::DateTime(_)
        ));
        // 纯日期串经 datetime() 补齐午夜（退化解析分支）
        assert!(matches!(
            eval_ok("datetime(\"2026-01-15\")"),
            EvalResult::DateTime(_)
        ));
        // weekday 接受 Unix 秒数字面量（eval_to_date Number 分支）
        approx(scalar_of(eval_ok("weekday(1767225600)")), 4.0);
        let (result, domain) = eval_ok_with_domain("date_diff(\"2026-01-02\", \"2026-01-01\") - 5");
        assert_eq!(domain, "time");
        approx(scalar_of(result), -6.0);
        approx(
            scalar_of(eval_ok("date_diff(\"2026-01-03\", \"2026-01-01\") ^ 2")),
            4.0,
        );
    }

    #[test]
    fn time_domain_error_branches() {
        assert_domain_err("date(\"x\", \"y\")");
        assert_domain_err("date(42)");
        assert_domain_err("date(\"not-a-date\")");
        assert_domain_err("date(\"7/4/2026\")");
        assert_domain_err("datetime(\"7/4/2026 10:00\")");
        // RFC3339 偏移后缀不被 datetime() 多格式接受 → invalid date
        assert_domain_err("datetime(\"2026-01-15T00:00:00Z\")");
        assert_domain_err("timestamp(\"garbage\")");
        assert_domain_err("timestamp(\"2026-01-15T00:00:00Z\", 2)");
        assert_err_kind("from_timestamp(1e19)", ErrorKind::Overflow);
        assert_domain_err("from_timestamp(1768435200, \"Bad/Tz\")");
        assert_domain_err("date_add(\"bad\", 1, \"day\")");
        assert_domain_err("date_add(\"2026-01-15\", 1, \"fortnight\")");
        assert_domain_err("date_add(\"2026-01-15\", 1000000000, \"year\")");
        assert_domain_err("date_diff(\"bad\", \"2026-01-01\")");
        assert_domain_err("format_date(\"bad\", \"%Y\")");
        assert_domain_err("format_date(\"2026-01-15T00:00:00Z\", \"%Y\", \"Bad/Tz\")");
        assert_domain_err("format_date(1e19, \"%Y\")");
        assert_domain_err("format_date(day_of_year(\"2026-01-15\") + 1e19, \"%Y\")");
        assert_domain_err("format_date(format_date(\"2026-01-15T00:00:00Z\", \"%Y\"), \"%Y\")");
        assert_domain_err("reformat_date(\"bad\", \"%Y-%m-%d\", \"%d/%m/%Y\")");
        assert_domain_err("weekday(\"bad\")");
        assert_domain_err("weekday(1e19)");
        assert_domain_err("day_of_year(\"bad\")");
        assert_domain_err("is_leap_year(\"bad\")");
        assert_domain_err("now(\"Mars/Olympus\")");
        assert_domain_err("today(\"Mars/Olympus\")");
        assert_err_kind(
            "date_diff(\"2026-01-02\", \"2026-01-01\") / 0",
            ErrorKind::DivisionByZero,
        );
        // inf - inf → NaN → nan_or_inf（时间域包围算术）
        assert_err_kind(
            "date_diff(\"2026-01-02\", \"2026-01-01\") + ((2 ^ 10000) - (2 ^ 10000))",
            ErrorKind::NaNOrInf,
        );
        assert_domain_err("timestamp(format_date(\"2026-01-15T00:00:00Z\", \"%Y\"))");
        // 非字符串实参走 extract_str 错误分支
        assert_domain_err("timestamp(datetime(\"2026-01-15T00:00:00Z\"))");
    }
}
