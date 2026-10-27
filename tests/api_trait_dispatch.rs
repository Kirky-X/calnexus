// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT
// CalcError 144 字节，函数签名触发 result_large_err——与 lib.rs 同论证
#![allow(clippy::result_large_err)]

//! 直接 API 层 trait 分派测试：经 trait 全限定调用驱动 `api/mod.rs` 委托块。
//! 固有方法测试（api_integration.rs）走不到 trait impl，这里以 `T: XxxMath` 泛型边界强制走委托路径。

use calnexus::{
    BigNumber, CalNexus, Complex, DataAnalysis, EvalResult, LinearAlgebra, Matrix, Polynomial,
    ScalarMath, SymbolicMath, Vector,
};

/// 提取 Scalar 结果值，非 Scalar panic。
fn scalar_of(r: EvalResult) -> f64 {
    match r {
        EvalResult::Scalar(v) => v,
        other => panic!("expected Scalar, got {other:?}"),
    }
}

/// 提取 BigInt 结果的十进制字符串，非 BigInt panic。
fn bigint_str(r: EvalResult) -> String {
    match r {
        EvalResult::BigInt(v) => v.to_string(),
        other => panic!("expected BigInt, got {other:?}"),
    }
}

fn approx(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "expected {b}, got {a}");
}

#[test]
fn scalar_math_trait_dispatch_covers_all_methods() {
    fn drive<T: ScalarMath>(s: &T) {
        approx(scalar_of(s.add(2.0, 3.0).unwrap()), 5.0);
        approx(scalar_of(s.sub(10.0, 3.0).unwrap()), 7.0);
        approx(scalar_of(s.mul(4.0, 5.0).unwrap()), 20.0);
        approx(scalar_of(s.div(10.0, 4.0).unwrap()), 2.5);
        assert!(s.div(1.0, 0.0).is_err(), "div by zero must fail");
        approx(scalar_of(s.pow(2.0, 10.0).unwrap()), 1024.0);
        approx(scalar_of(s.rem(10.0, 3.0).unwrap()), 1.0);
        approx(scalar_of(s.factorial(5).unwrap()), 120.0);
        approx(scalar_of(s.abs(-3.0).unwrap()), 3.0);
        approx(scalar_of(s.sin(std::f64::consts::FRAC_PI_2).unwrap()), 1.0);
        approx(scalar_of(s.cos(0.0).unwrap()), 1.0);
        approx(scalar_of(s.tan(0.0).unwrap()), 0.0);
        approx(scalar_of(s.asin(1.0).unwrap()), std::f64::consts::FRAC_PI_2);
        approx(scalar_of(s.acos(1.0).unwrap()), 0.0);
        approx(scalar_of(s.atan(1.0).unwrap()), std::f64::consts::FRAC_PI_4);
        approx(scalar_of(s.ln(1.0).unwrap()), 0.0);
        approx(scalar_of(s.log(8.0, 2.0).unwrap()), 3.0);
        approx(scalar_of(s.exp(0.0).unwrap()), 1.0);
        approx(scalar_of(s.sinh(0.0).unwrap()), 0.0);
        approx(scalar_of(s.cosh(0.0).unwrap()), 1.0);
        approx(scalar_of(s.tanh(0.0).unwrap()), 0.0);
        approx(scalar_of(s.gamma(5.0).unwrap()), 24.0);
        approx(scalar_of(s.erf(0.0).unwrap()), 0.0);
        // numeric-frontier 批次：erfc / lgamma / polylog / derivative
        approx(scalar_of(s.erfc(0.0).unwrap()), 1.0);
        approx(scalar_of(s.lgamma(1.0).unwrap()), 0.0);
        approx(
            scalar_of(s.polylog(2, 1.0).unwrap()),
            std::f64::consts::PI * std::f64::consts::PI / 6.0,
        );
        // |z|<1 主级数路径（mpmath 参考：polylog(2, 0.5) = 0.5822405264650125）
        approx(scalar_of(s.polylog(2, 0.5).unwrap()), 0.5822405264650125);
        assert!(s.polylog(5, 0.5).is_err(), "polylog 阶数只支持 2/3");
        assert!(s.polylog(2, 1.5).is_err(), "|z| > 1 必须显式 Domain 拒绝");
        #[cfg(feature = "numerical")]
        approx(scalar_of(s.derivative("x^2", "x", 3.0).unwrap()), 6.0);
        let precise = s.precision_eval(30, "0.1+0.2").unwrap();
        let value = match precise {
            EvalResult::BigRational(r) => {
                r.numer().to_string().parse::<f64>().unwrap()
                    / r.denom().to_string().parse::<f64>().unwrap()
            }
            EvalResult::Scalar(v) => v,
            EvalResult::Symbolic(t) => t.parse().expect("precision_eval output must parse"),
            other => panic!("unexpected precision_eval result {other:?}"),
        };
        assert!((value - 0.3).abs() < 1e-9);
        assert_eq!(
            bigint_str(
                s.gcd(&BigNumber::from_i64(12), &BigNumber::from_i64(18))
                    .unwrap()
            ),
            "6"
        );
        assert_eq!(
            bigint_str(
                s.lcm(&BigNumber::from_i64(4), &BigNumber::from_i64(6))
                    .unwrap()
            ),
            "12"
        );
        approx(
            scalar_of(s.is_prime(&BigNumber::from_i64(17)).unwrap()),
            1.0,
        );
        approx(
            scalar_of(s.is_prime(&BigNumber::from_i64(18)).unwrap()),
            0.0,
        );
        match s.prime_sieve(30).unwrap() {
            EvalResult::Vector(primes) => {
                assert_eq!(
                    primes,
                    vec![2.0, 3.0, 5.0, 7.0, 11.0, 13.0, 17.0, 19.0, 23.0, 29.0]
                );
            }
            other => panic!("expected Vector, got {other:?}"),
        }
        assert_eq!(
            bigint_str(
                s.mod_pow(
                    &BigNumber::from_i64(3),
                    &BigNumber::from_i64(4),
                    &BigNumber::from_i64(7)
                )
                .unwrap()
            ),
            "4"
        );
        assert_eq!(
            bigint_str(
                s.mod_inverse(&BigNumber::from_i64(3), &BigNumber::from_i64(7))
                    .unwrap()
            ),
            "5"
        );
        assert_eq!(
            bigint_str(s.euler_phi(&BigNumber::from_i64(10)).unwrap()),
            "4"
        );
        assert_eq!(
            bigint_str(
                s.crt(
                    &[BigNumber::from_i64(2), BigNumber::from_i64(3)],
                    &[BigNumber::from_i64(3), BigNumber::from_i64(5)]
                )
                .unwrap()
            ),
            "8"
        );
        assert_eq!(
            bigint_str(
                s.discrete_log(
                    &BigNumber::from_i64(2),
                    &BigNumber::from_i64(8),
                    &BigNumber::from_i64(17)
                )
                .unwrap()
            ),
            "3"
        );
        assert_eq!(bigint_str(s.perm(5, 2).unwrap()), "20");
        assert_eq!(bigint_str(s.comb(5, 2).unwrap()), "10");
        assert_eq!(bigint_str(s.catalan(3).unwrap()), "5");
        assert_eq!(bigint_str(s.stirling_first(4, 2).unwrap()), "11");
        assert_eq!(bigint_str(s.stirling_second(4, 2).unwrap()), "7");
    }
    let cn = CalNexus::new();
    drive(&cn.scalar());
}

#[test]
fn linear_algebra_trait_dispatch_covers_all_methods() {
    fn drive<T: LinearAlgebra>(l: &T) {
        let a = Matrix::from_rows(&[&[1.0, 2.0], &[3.0, 4.0]]);
        let b = Matrix::from_rows(&[&[5.0, 6.0], &[7.0, 8.0]]);
        approx(scalar_of(l.det(&a).unwrap()), -2.0);
        match l.inverse(&a).unwrap() {
            EvalResult::Matrix(inv) => {
                approx(inv[0][0], -2.0);
                approx(inv[0][1], 1.0);
                approx(inv[1][0], 1.5);
                approx(inv[1][1], -0.5);
            }
            other => panic!("expected Matrix, got {other:?}"),
        }
        assert!(
            l.inverse(&Matrix::from_rows(&[&[1.0, 2.0], &[2.0, 4.0]]))
                .is_err()
        );
        match l.transpose(&a).unwrap() {
            EvalResult::Matrix(t) => {
                approx(t[0][1], 3.0);
                approx(t[1][0], 2.0);
            }
            other => panic!("expected Matrix, got {other:?}"),
        }
        match l.identity(3).unwrap() {
            EvalResult::Matrix(id) => {
                approx(id[1][1], 1.0);
                approx(id[0][1], 0.0);
            }
            other => panic!("expected Matrix, got {other:?}"),
        }
        match l.mat_add(&a, &b).unwrap() {
            EvalResult::Matrix(m) => approx(m[1][1], 12.0),
            other => panic!("expected Matrix, got {other:?}"),
        }
        match l.mat_sub(&b, &a).unwrap() {
            EvalResult::Matrix(m) => approx(m[0][0], 4.0),
            other => panic!("expected Matrix, got {other:?}"),
        }
        match l.mat_mul(&a, &b).unwrap() {
            EvalResult::Matrix(m) => {
                approx(m[0][0], 19.0);
                approx(m[1][1], 50.0);
            }
            other => panic!("expected Matrix, got {other:?}"),
        }
        match l.scalar_mul(2.0, &a).unwrap() {
            EvalResult::Matrix(m) => approx(m[1][0], 6.0),
            other => panic!("expected Matrix, got {other:?}"),
        }
        let u = Vector::new(&[1.0, 2.0, 3.0]);
        let v = Vector::new(&[4.0, 5.0, 6.0]);
        approx(scalar_of(l.dot(&u, &v).unwrap()), 32.0);
        // numeric-frontier 批次：lstsq（超定系统 QR 主路径，解 ≈ [3, 3]）
        #[cfg(feature = "numerical")]
        {
            let la = Matrix::from_rows(&[&[1.0, 1.0], &[1.0, 2.0], &[1.0, 3.0]]);
            let lb = Vector::new(&[6.0, 9.0, 12.0]);
            match l.lstsq(&la, &lb).unwrap() {
                EvalResult::Vector(x) => {
                    approx(x[0], 3.0);
                    approx(x[1], 3.0);
                }
                other => panic!("expected Vector, got {other:?}"),
            }
            // 行数与 b 长度不一致必须以维度类错误显性拒绝（按 ErrorKind 断言，不耦合文案）
            assert!(
                l.lstsq(&la, &Vector::new(&[1.0, 2.0]))
                    .err()
                    .is_some_and(|e| e.kind == calnexus::ErrorKind::Domain),
                "lstsq 行数不匹配必须报错"
            );
        }
        match l.cross(&u, &v).unwrap() {
            EvalResult::Vector(w) => {
                approx(w[0], -3.0);
                approx(w[1], 6.0);
                approx(w[2], -3.0);
            }
            other => panic!("expected Vector, got {other:?}"),
        }
        match l.normalize(&u).unwrap() {
            EvalResult::Vector(n) => {
                approx(n[0], (1.0f64 / 14.0f64).sqrt());
            }
            other => panic!("expected Vector, got {other:?}"),
        }
        approx(scalar_of(l.magnitude(&u).unwrap()), 14.0f64.sqrt());
        match l.vector_add(&u, &v).unwrap() {
            EvalResult::Vector(w) => approx(w[2], 9.0),
            other => panic!("expected Vector, got {other:?}"),
        }
        match l.vector_sub(&v, &u).unwrap() {
            EvalResult::Vector(w) => approx(w[0], 3.0),
            other => panic!("expected Vector, got {other:?}"),
        }
    }
    let cn = CalNexus::new();
    drive(&cn.linalg());
}

#[test]
fn data_analysis_trait_dispatch_covers_all_methods() {
    fn drive<T: DataAnalysis>(d: &T) {
        let data = [2.0, 4.0, 6.0];
        approx(scalar_of(d.mean(&data).unwrap()), 4.0);
        approx(scalar_of(d.variance(&data).unwrap()), 8.0 / 3.0);
        approx(scalar_of(d.std(&data).unwrap()), (8.0f64 / 3.0).sqrt());
        approx(scalar_of(d.median(&data).unwrap()), 4.0);
        approx(scalar_of(d.median(&[1.0, 2.0, 3.0, 4.0]).unwrap()), 2.5);
        approx(scalar_of(d.min(&data).unwrap()), 2.0);
        approx(scalar_of(d.max(&data).unwrap()), 6.0);
        approx(scalar_of(d.sum(&data).unwrap()), 12.0);
        approx(scalar_of(d.count(&data).unwrap()), 3.0);
        approx(
            scalar_of(d.norm_pdf(0.0, 0.0, 1.0).unwrap()),
            0.3989422804014327,
        );
        approx(scalar_of(d.norm_cdf(0.0, 0.0, 1.0).unwrap()), 0.5);
        let norm_inv = scalar_of(d.norm_inv(0.975, 0.0, 1.0).unwrap());
        assert!((norm_inv - 1.959963984540054).abs() < 1e-6);
        approx(scalar_of(d.t_pdf(0.0, 5.0).unwrap()), 0.37960668982249444);
        approx(scalar_of(d.t_cdf(0.0, 5.0).unwrap()), 0.5);
        let t_inv = scalar_of(d.t_inv(0.975, 10.0).unwrap());
        assert!((t_inv - 2.2281388519649385).abs() < 1e-6);
        approx(
            scalar_of(d.chi2_pdf(1.0, 1.0).unwrap()),
            0.24197072451914337,
        );
        let chi2_cdf = scalar_of(d.chi2_cdf(3.841458820694124, 1.0).unwrap());
        assert!((chi2_cdf - 0.95).abs() < 1e-6);
        let chi2_inv = scalar_of(d.chi2_inv(0.95, 1.0).unwrap());
        assert!((chi2_inv - 3.841458820694124).abs() < 1e-4);
        approx(scalar_of(d.f_pdf(1.0, 2.0, 2.0).unwrap()), 0.25);
        approx(scalar_of(d.f_cdf(1.0, 2.0, 2.0).unwrap()), 0.5);
        let f_inv = scalar_of(d.f_inv(0.5, 2.0, 2.0).unwrap());
        assert!((f_inv - 1.0).abs() < 1e-4);
        approx(
            scalar_of(d.poisson_pmf(2.0, 2.0).unwrap()),
            2.0 * (-2.0f64).exp(),
        );
        approx(
            scalar_of(d.poisson_cdf(2.0, 2.0).unwrap()),
            5.0 * (-2.0f64).exp(),
        );
        approx(scalar_of(d.binom_pmf(2.0, 4.0, 0.5).unwrap()), 0.375);
        approx(scalar_of(d.binom_cdf(2.0, 4.0, 0.5).unwrap()), 0.6875);
        assert!(d.t_test_one(&data, 3.0).is_ok());
        assert!(d.t_test_one(&[], 0.0).is_ok());
        assert!(d.t_test_two(&[1.0, 2.0, 3.0], &[2.0, 4.0, 6.0]).is_ok());
        assert!(d.t_test_two(&[], &[]).is_ok());
        assert!(
            d.chi2_test(&[10.0, 20.0, 30.0], &[20.0, 20.0, 20.0])
                .is_ok()
        );
        approx(
            scalar_of(d.pearson(&[1.0, 2.0, 3.0], &[2.0, 4.0, 6.0]).unwrap()),
            1.0,
        );
        approx(
            scalar_of(d.spearman(&[1.0, 2.0, 3.0], &[2.0, 4.0, 6.0]).unwrap()),
            1.0,
        );
        match d.lin_reg(&[1.0, 2.0, 3.0], &[2.0, 4.0, 6.0]).unwrap() {
            EvalResult::Json(j) => approx(j["slope"].as_f64().unwrap(), 2.0),
            other => panic!("expected Json, got {other:?}"),
        }
        let degenerate = d.lin_reg(&[2.0, 2.0, 2.0], &[1.0, 2.0, 3.0]).unwrap();
        match degenerate {
            EvalResult::Json(j) => approx(j["slope"].as_f64().unwrap(), 0.0),
            other => panic!("expected Json, got {other:?}"),
        }
        match d
            .poly_reg(&[1.0, 2.0, 3.0, 4.0], &[1.0, 4.0, 9.0, 16.0], 2)
            .unwrap()
        {
            EvalResult::Json(j) => {
                let coeffs = j["coefficients"].as_array().unwrap();
                approx(coeffs[2].as_f64().unwrap(), 1.0);
            }
            other => panic!("expected Json, got {other:?}"),
        }
        assert!(
            d.poly_reg(&[1.0, 1.0, 1.0], &[1.0, 2.0, 3.0], 1).is_err(),
            "constant x must yield singular XTX error"
        );
        assert!(d.poly_reg(&[1.0, 2.0], &[1.0, 2.0], 5).is_err());
        // 每个内向量是一个特征（长度=样本数），y = 1 + 2·f0 + 3·f1
        match d
            .multi_reg(
                &[vec![1.0, 2.0, 3.0], vec![0.0, 1.0, 4.0]],
                &[3.0, 8.0, 19.0],
            )
            .unwrap()
        {
            EvalResult::Json(j) => {
                let coeffs = j["coefficients"].as_array().unwrap();
                approx(coeffs[0].as_f64().unwrap(), 1.0);
                approx(coeffs[1].as_f64().unwrap(), 2.0);
                approx(coeffs[2].as_f64().unwrap(), 3.0);
            }
            other => panic!("expected Json, got {other:?}"),
        }
        assert!(
            d.multi_reg(&[vec![1.0, 2.0]], &[1.0, 2.0, 3.0]).is_err(),
            "feature length mismatch must fail"
        );
        assert!(
            d.multi_reg(&[vec![1.0, 2.0], vec![3.0, 4.0]], &[1.0, 2.0])
                .is_err(),
            "features >= samples must be rejected as underdetermined"
        );
        assert!(
            d.multi_reg(
                &[vec![1.0, 2.0, 3.0], vec![2.0, 4.0, 6.0]],
                &[1.0, 2.0, 3.0]
            )
            .is_err(),
            "collinear features must yield singular matrix error"
        );
    }
    let cn = CalNexus::new();
    drive(&cn.stats());
}

#[test]
fn symbolic_math_trait_dispatch_covers_all_methods() {
    fn drive<T: SymbolicMath>(m: &T) {
        match m.differentiate("x^2", "x").unwrap() {
            EvalResult::Symbolic(s) => assert!(s.contains("2"), "d/dx x^2 should mention 2: {s}"),
            other => panic!("expected Symbolic, got {other:?}"),
        }
        assert!(m.integrate("2*x", "x").is_ok());
        assert!(m.simplify("x+x").is_ok());
        approx(scalar_of(m.limit("1/x", "x", 1.0).unwrap()), 1.0);
        assert!(
            m.taylor_expand("x^2", "x", 1.0, 2).is_err(),
            "non-zero center must be rejected"
        );
        let p = Polynomial::new(&[1.0, 2.0]);
        let q = Polynomial::new(&[3.0]);
        match m.poly_add(&p, &q).unwrap() {
            EvalResult::Polynomial(c) => {
                approx(c[0], 4.0);
                approx(c[1], 2.0);
            }
            other => panic!("expected Polynomial, got {other:?}"),
        }
        match m.poly_sub(&q, &p).unwrap() {
            EvalResult::Polynomial(c) => {
                approx(c[0], 2.0);
                approx(c[1], -2.0);
            }
            other => panic!("expected Polynomial, got {other:?}"),
        }
        match m.poly_mul(&p, &q).unwrap() {
            EvalResult::Polynomial(c) => {
                approx(c[0], 3.0);
                approx(c[1], 6.0);
            }
            other => panic!("expected Polynomial, got {other:?}"),
        }
        match m.poly_div(&p, &q).unwrap() {
            EvalResult::Polynomial(c) => approx(c[1], 2.0 / 3.0),
            other => panic!("expected Polynomial, got {other:?}"),
        }
        match m.poly_roots(&Polynomial::new(&[-4.0, 0.0, 1.0])).unwrap() {
            EvalResult::ComplexList(roots) => assert_eq!(roots.len(), 2),
            EvalResult::Vector(roots) => assert_eq!(roots.len(), 2),
            other => panic!("unexpected poly_roots result {other:?}"),
        }
        approx(scalar_of(m.poly_eval(&p, 2.0).unwrap()), 5.0);
        let z1 = Complex::new(1.0, 2.0);
        let z2 = Complex::new(3.0, -1.0);
        match m.complex_add(&z1, &z2).unwrap() {
            EvalResult::Complex(re, im) => {
                approx(re, 4.0);
                approx(im, 1.0);
            }
            other => panic!("expected Complex, got {other:?}"),
        }
        match m.complex_sub(&z1, &z2).unwrap() {
            EvalResult::Complex(re, im) => {
                approx(re, -2.0);
                approx(im, 3.0);
            }
            other => panic!("expected Complex, got {other:?}"),
        }
        match m.complex_mul(&z1, &z2).unwrap() {
            EvalResult::Complex(re, im) => {
                approx(re, 5.0);
                approx(im, 5.0);
            }
            other => panic!("expected Complex, got {other:?}"),
        }
        let z = Complex::new(3.0, 4.0);
        approx(scalar_of(m.complex_abs(&z).unwrap()), 5.0);
        approx(
            scalar_of(m.complex_arg(&Complex::new(1.0, 0.0)).unwrap()),
            0.0,
        );
        match m.complex_conj(&z1).unwrap() {
            EvalResult::Complex(re, im) => {
                approx(re, 1.0);
                approx(im, -2.0);
            }
            other => panic!("expected Complex, got {other:?}"),
        }
        match m.complex_exp(&Complex::new(0.0, 0.0)).unwrap() {
            EvalResult::Complex(re, im) => {
                approx(re, 1.0);
                approx(im, 0.0);
            }
            other => panic!("expected Complex, got {other:?}"),
        }
        match m.complex_ln(&Complex::new(1.0, 0.0)).unwrap() {
            EvalResult::Complex(re, im) => {
                approx(re, 0.0);
                approx(im, 0.0);
            }
            other => panic!("expected Complex, got {other:?}"),
        }
        approx(
            scalar_of(
                m.solve_equation("x - 2", "x", "newton", Some(&[2.0][..]))
                    .unwrap(),
            ),
            2.0,
        );
        approx(
            scalar_of(
                m.solve_equation("x^2 - 4", "x", "bisection", Some(&[0.0, 5.0][..]))
                    .unwrap(),
            ),
            2.0,
        );
        approx(
            scalar_of(
                m.solve_equation("x^2 - 4", "x", "brent", Some(&[0.0, 5.0][..]))
                    .unwrap(),
            ),
            2.0,
        );
        // 端点 b 处 f(b)=0：brent 首步直接返回
        approx(
            scalar_of(
                m.solve_equation("x - 2", "x", "brent", Some(&[0.0, 2.0][..]))
                    .unwrap(),
            ),
            2.0,
        );
        assert!(
            m.solve_equation("x^2 - 4", "x", "bisection", Some(&[][..]))
                .is_err(),
            "bisection without options must fail"
        );
        assert!(
            m.solve_equation("x^2 - 4", "x", "brent", Some(&[][..]))
                .is_err(),
            "brent without options must fail"
        );
        assert!(
            m.solve_equation("x^2 - 4", "x", "secant", Some(&[][..]))
                .is_err()
        );
        assert!(
            m.solve_equation("[1, 2]", "x", "newton", Some(&[1.0][..]))
                .is_err(),
            "non-scalar objective must fail to converge"
        );
    }
    let cn = CalNexus::new();
    drive(&cn.symbolic());
}

#[test]
fn wrapper_try_from_rejects_mismatched_variants() {
    assert_eq!(
        Vector::try_from(EvalResult::Scalar(1.0)).unwrap_err(),
        "expected Vector result"
    );
    assert_eq!(
        Complex::try_from(EvalResult::Scalar(1.0)).unwrap_err(),
        "expected Complex result"
    );
    assert_eq!(
        Polynomial::try_from(EvalResult::Scalar(1.0)).unwrap_err(),
        "expected Polynomial result"
    );
    assert_eq!(
        BigNumber::try_from(EvalResult::Scalar(1.0)).unwrap_err(),
        "expected BigInt result"
    );
}

#[cfg(feature = "time")]
mod applied_time {
    use super::{approx, scalar_of};
    use calnexus::{AppliedMath, AppliedMathImpl, CalNexus};

    fn drive_time<T: AppliedMath>(a: &T) {
        match a.date("2026-01-15").unwrap() {
            calnexus::EvalResult::DateTime(s) => assert!(s.starts_with("2026-01-15")),
            other => panic!("expected DateTime, got {other:?}"),
        }
        match a.datetime("2026-01-15 10:30", None).unwrap() {
            calnexus::EvalResult::DateTime(s) => assert!(s.contains("10:30")),
            other => panic!("expected DateTime, got {other:?}"),
        }
        assert!(a.datetime("2026-01-15 10:30", Some("UTC")).is_ok());
        assert!(
            a.datetime("2026-01-15", Some("Bad/Tz")).is_err(),
            "unknown timezone must fail"
        );
        approx(
            scalar_of(a.timestamp("2026-01-15T00:00:00Z").unwrap()),
            1768435200.0,
        );
        assert!(a.from_timestamp(1768435200, Some("UTC")).is_ok());
        assert!(
            a.from_timestamp(i64::MAX, None).is_err(),
            "out-of-range timestamp must fail"
        );
        assert!(a.now(None).is_ok());
        assert!(a.now(Some("Mars/Olympus")).is_err());
        assert!(a.today(None).is_ok());
        assert!(a.today(Some("Mars/Olympus")).is_err());
        assert!(a.date_add("2026-01-15", 1, "day").is_ok());
        approx(
            scalar_of(
                a.date_diff("2026-01-15", "2026-01-01", Some("day"))
                    .unwrap(),
            ),
            -14.0,
        );
        match a
            .format_date("2026-01-15T00:00:00Z", "%Y-%m-%d", None)
            .unwrap()
        {
            calnexus::EvalResult::Symbolic(s) => assert_eq!(s, "2026-01-15"),
            other => panic!("expected Symbolic, got {other:?}"),
        }
        assert!(
            a.format_date("2026-01-15T00:00:00Z", "%Y", Some("Asia/Shanghai"))
                .is_ok()
        );
        assert!(a.format_date("not-a-date", "%Y", None).is_err());
        match a
            .reformat_date("15/01/2026", "%d/%m/%Y", "%Y-%m-%d")
            .unwrap()
        {
            calnexus::EvalResult::Symbolic(s) => assert_eq!(s, "2026-01-15"),
            other => panic!("expected Symbolic, got {other:?}"),
        }
        match a
            .reformat_date("2026-01-15 10:30", "%Y-%m-%d %H:%M", "%d.%m.%Y %H:%M")
            .unwrap()
        {
            calnexus::EvalResult::Symbolic(s) => assert_eq!(s, "15.01.2026 00:00"),
            other => panic!("expected Symbolic, got {other:?}"),
        }
        match a.reformat_date("1768435200", "%s", "%Y-%m-%d").unwrap() {
            calnexus::EvalResult::Symbolic(s) => assert_eq!(s, "2026-01-15"),
            other => panic!("expected Symbolic, got {other:?}"),
        }
        assert!(a.reformat_date("bad", "%Y-%m-%d", "%d/%m/%Y").is_err());
        approx(scalar_of(a.weekday("2026-01-15").unwrap()), 4.0);
        approx(scalar_of(a.day_of_year("2026-02-01").unwrap()), 32.0);
        approx(scalar_of(a.is_leap_year(2024).unwrap()), 1.0);
        approx(scalar_of(a.is_leap_year(2023).unwrap()), 0.0);
        approx(scalar_of(a.is_leap_year(2000).unwrap()), 1.0);
    }

    #[test]
    fn applied_math_trait_dispatch_covers_time_methods() {
        let cn = CalNexus::new();
        let a: AppliedMathImpl<'_> = cn.applied();
        drive_time(&a);
    }
}

#[cfg(feature = "unit")]
#[test]
fn applied_math_trait_dispatch_converts_units() {
    use calnexus::{AppliedMath, CalNexus};
    let cn = CalNexus::new();
    let a = cn.applied();
    let v = AppliedMath::convert(&a, 1.0, "km", "m").unwrap();
    assert!(matches!(v, calnexus::EvalResult::Scalar(x) if (x - 1000.0).abs() < 1e-9));
    assert!(AppliedMath::convert(&a, 1.0, "km", "not_a_unit").is_err());
}

#[cfg(feature = "fx")]
#[test]
fn applied_math_trait_dispatch_fx_network_tolerant() {
    use calnexus::{AppliedMath, CalNexus, EvalResult};
    let cn = CalNexus::new();
    let a = cn.applied();
    for r in [
        AppliedMath::fx(&a, 100.0, "USD", "EUR"),
        AppliedMath::fx_rate(&a, "USD", "EUR"),
    ] {
        match r {
            Ok(EvalResult::Scalar(v)) => assert!(v.is_finite()),
            Ok(other) => panic!("expected Scalar, got {other:?}"),
            Err(e) => assert_eq!(e.kind, calnexus::ErrorKind::Domain),
        }
    }
}
