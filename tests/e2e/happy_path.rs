// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! S1 Happy Path —— 各域函数在合法输入下的预期行为。
//!
//! 覆盖：门面五访问器（含既有套件缺失的 `cn.symbolic()` 全链路与
//! trait 泛型分发通道）、`evaluate` 管线全部可达 `EvalResult` 变体、
//! time/unit/numerical 可选域启用侧表达式正常路径。
//!
//! 注：`format_bigrational` 走 `math::precision` 公开路径（crate 根
//! 再导出被 cli/http/mcp 门控，而本目标需在全部 feature 组合下编译）。

use calnexus::math::precision::format_bigrational;
use calnexus::{
    BigNumber, CalNexus, Complex, EvalResult, Matrix, Polynomial, ScalarMath, SymbolicMath, Vector,
};

use crate::common::{approx_eq, assert_scalar, eval_ok_with_domain};

fn scalar_of(r: &EvalResult) -> f64 {
    match r {
        EvalResult::Scalar(v) => *v,
        other => panic!("expected Scalar, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// 门面 · ScalarMath
// ---------------------------------------------------------------------------

#[test]
fn facade_scalar_arithmetic_inherent() {
    let cn = CalNexus::new();
    let s = cn.scalar();
    assert_scalar(&s.add(2.0, 3.0).unwrap(), 5.0);
    assert_scalar(&s.sub(7.5, 2.5).unwrap(), 5.0);
    assert_scalar(&s.mul(4.0, 2.5).unwrap(), 10.0);
    assert_scalar(&s.div(9.0, 3.0).unwrap(), 3.0);
    assert_scalar(&s.pow(2.0, 10.0).unwrap(), 1024.0);
    assert_scalar(&s.rem(10.0, 3.0).unwrap(), 1.0);
    assert_scalar(&s.abs(-4.2).unwrap(), 4.2);
    assert_scalar(&s.factorial(10).unwrap(), 3_628_800.0);
}

#[test]
fn facade_scalar_arithmetic_via_trait_generic() {
    // trait 泛型分发通道（公共 Semver 面的另一半）
    fn drive<T: ScalarMath>(s: &T) -> (f64, f64) {
        let a = scalar_of(&s.add(20.0, 22.0).unwrap());
        let b = scalar_of(&s.mul(6.0, 7.0).unwrap());
        (a, b)
    }
    let cn = CalNexus::new();
    let (a, b) = drive(&cn.scalar());
    assert_eq!((a, b), (42.0, 42.0));
}

#[test]
fn facade_scientific_functions() {
    let cn = CalNexus::new();
    let s = cn.scalar();
    assert!(approx_eq(scalar_of(&s.sin(0.0).unwrap()), 0.0));
    assert!(approx_eq(scalar_of(&s.cos(0.0).unwrap()), 1.0));
    assert!(approx_eq(scalar_of(&s.tan(0.0).unwrap()), 0.0));
    assert!(approx_eq(
        scalar_of(&s.asin(1.0).unwrap()),
        std::f64::consts::FRAC_PI_2
    ));
    assert!(approx_eq(scalar_of(&s.acos(1.0).unwrap()), 0.0));
    assert!(approx_eq(
        scalar_of(&s.atan(1.0).unwrap()),
        std::f64::consts::FRAC_PI_4
    ));
    assert!(approx_eq(
        scalar_of(&s.ln(std::f64::consts::E).unwrap()),
        1.0
    ));
    assert!(approx_eq(scalar_of(&s.log(8.0, 2.0).unwrap()), 3.0));
    assert!(approx_eq(
        scalar_of(&s.exp(1.0).unwrap()),
        std::f64::consts::E
    ));
    assert!(approx_eq(scalar_of(&s.sinh(0.0).unwrap()), 0.0));
    assert!(approx_eq(scalar_of(&s.cosh(0.0).unwrap()), 1.0));
    assert!(approx_eq(scalar_of(&s.tanh(0.0).unwrap()), 0.0));
    // Γ(5) = 4! = 24；erf(0) = 0
    assert!(approx_eq(scalar_of(&s.gamma(5.0).unwrap()), 24.0));
    assert!(approx_eq(scalar_of(&s.erf(0.0).unwrap()), 0.0));
}

#[test]
fn facade_number_theory() {
    let cn = CalNexus::new();
    let s = cn.scalar();
    let a = BigNumber::from_i64(48);
    let b = BigNumber::from_i64(18);
    match s.gcd(&a, &b).unwrap() {
        EvalResult::BigInt(g) => assert_eq!(g.to_string(), "6"),
        other => panic!("gcd → BigInt(6) expected, got {other:?}"),
    }
    match s.lcm(&a, &b).unwrap() {
        EvalResult::BigInt(l) => assert_eq!(l.to_string(), "144"),
        other => panic!("lcm → BigInt(144) expected, got {other:?}"),
    }
    match s.is_prime(&BigNumber::from_i64(97)).unwrap() {
        EvalResult::Scalar(v) => assert_eq!(v, 1.0),
        other => panic!("is_prime(97) truthy expected, got {other:?}"),
    }
    match s.is_prime(&BigNumber::from_i64(96)).unwrap() {
        EvalResult::Scalar(v) => assert_eq!(v, 0.0),
        other => panic!("is_prime(96) falsy expected, got {other:?}"),
    }
    // 2^10 mod 1000 = 24
    match s
        .mod_pow(
            &BigNumber::from_i64(2),
            &BigNumber::from_i64(10),
            &BigNumber::from_i64(1000),
        )
        .unwrap()
    {
        EvalResult::BigInt(v) => assert_eq!(v.to_string(), "24"),
        other => panic!("mod_pow → BigInt(24) expected, got {other:?}"),
    }
    // φ(10) = 4
    match s.euler_phi(&BigNumber::from_i64(10)).unwrap() {
        EvalResult::BigInt(v) => assert_eq!(v.to_string(), "4"),
        other => panic!("euler_phi(10) → BigInt(4) expected, got {other:?}"),
    }
    // CRT: x ≡ 2 (mod 3), x ≡ 3 (mod 5) → x = 8
    match s
        .crt(
            &[BigNumber::from_i64(2), BigNumber::from_i64(3)],
            &[BigNumber::from_i64(3), BigNumber::from_i64(5)],
        )
        .unwrap()
    {
        EvalResult::BigInt(v) => assert_eq!(v.to_string(), "8"),
        other => panic!("crt → BigInt(8) expected, got {other:?}"),
    }
    // 3 ≡ 1 (mod 7) 的模逆元为 5
    match s
        .mod_inverse(&BigNumber::from_i64(3), &BigNumber::from_i64(7))
        .unwrap()
    {
        EvalResult::BigInt(v) => assert_eq!(v.to_string(), "5"),
        other => panic!("mod_inverse → BigInt(5) expected, got {other:?}"),
    }
}

#[test]
fn facade_combinatorics() {
    let cn = CalNexus::new();
    let s = cn.scalar();
    // 组合数学门面走 BigInt 精确通道（非标量）
    fn bigint_of(r: EvalResult) -> String {
        match r {
            EvalResult::BigInt(v) => v.to_string(),
            other => panic!("BigInt expected, got {other:?}"),
        }
    }
    assert_eq!(bigint_of(s.perm(5, 2).unwrap()), "20");
    assert_eq!(bigint_of(s.comb(5, 2).unwrap()), "10");
    assert_eq!(bigint_of(s.catalan(3).unwrap()), "5");
    // S(4,2) = 7（第二类斯特林数）；s(4,4-2) = 11（第一类带符号）
    assert_eq!(bigint_of(s.stirling_second(4, 2).unwrap()), "7");
    let first = bigint_of(s.stirling_first(4, 2).unwrap());
    assert!(
        first == "11" || first == "-11",
        "stirling_first(4,2), got {first}"
    );
}

#[test]
fn facade_precision_eval() {
    let cn = CalNexus::new();
    match cn.scalar().precision_eval(5, "1/3").unwrap() {
        EvalResult::BigRational(q) => {
            assert_eq!(format_bigrational(&q, Some(5)), "0.33333");
        }
        other => panic!("precision_eval → BigRational expected, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// 门面 · LinearAlgebra
// ---------------------------------------------------------------------------

#[test]
fn facade_matrix_ops() {
    let cn = CalNexus::new();
    let l = cn.linalg();
    let m = Matrix::from_rows(&[&[1.0, 2.0], &[3.0, 4.0]]);
    assert!(approx_eq(scalar_of(&l.det(&m).unwrap()), -2.0));

    match l.inverse(&m).unwrap() {
        EvalResult::Matrix(rows) => {
            assert!(approx_eq(rows[0][0], -2.0));
            assert!(approx_eq(rows[0][1], 1.0));
            assert!(approx_eq(rows[1][0], 1.5));
            assert!(approx_eq(rows[1][1], -0.5));
        }
        other => panic!("inverse → Matrix expected, got {other:?}"),
    }

    match l.transpose(&m).unwrap() {
        EvalResult::Matrix(rows) => {
            assert!(approx_eq(rows[0][1], 3.0));
            assert!(approx_eq(rows[1][0], 2.0));
        }
        other => panic!("transpose → Matrix expected, got {other:?}"),
    }

    match l.identity(3).unwrap() {
        EvalResult::Matrix(rows) => {
            assert_eq!(rows.len(), 3);
            assert!(approx_eq(rows[2][2], 1.0));
            assert!(approx_eq(rows[0][1], 0.0));
        }
        other => panic!("identity → Matrix expected, got {other:?}"),
    }

    let n = Matrix::from_rows(&[&[5.0, 6.0], &[7.0, 8.0]]);
    match l.mat_add(&m, &n).unwrap() {
        EvalResult::Matrix(rows) => assert!(approx_eq(rows[1][1], 12.0)),
        other => panic!("mat_add → Matrix expected, got {other:?}"),
    }
    match l.mat_sub(&n, &m).unwrap() {
        EvalResult::Matrix(rows) => assert!(approx_eq(rows[0][0], 4.0)),
        other => panic!("mat_sub → Matrix expected, got {other:?}"),
    }
    match l.mat_mul(&m, &n).unwrap() {
        EvalResult::Matrix(rows) => {
            assert!(approx_eq(rows[0][0], 19.0));
            assert!(approx_eq(rows[1][1], 50.0));
        }
        other => panic!("mat_mul → Matrix expected, got {other:?}"),
    }
    match l.scalar_mul(2.0, &m).unwrap() {
        EvalResult::Matrix(rows) => assert!(approx_eq(rows[1][0], 6.0)),
        other => panic!("scalar_mul → Matrix expected, got {other:?}"),
    }
}

#[test]
fn facade_vector_ops() {
    let cn = CalNexus::new();
    let l = cn.linalg();
    let a = Vector::new(&[1.0, 2.0, 3.0]);
    let b = Vector::new(&[4.0, 5.0, 6.0]);
    assert_scalar(&l.dot(&a, &b).unwrap(), 32.0);
    match l.cross(&a, &b).unwrap() {
        EvalResult::Vector(v) => {
            assert!(approx_eq(v[0], -3.0));
            assert!(approx_eq(v[1], 6.0));
            assert!(approx_eq(v[2], -3.0));
        }
        other => panic!("cross → Vector expected, got {other:?}"),
    }
    match l.normalize(&a).unwrap() {
        EvalResult::Vector(v) => assert!(approx_eq(v[2], 3.0 / 14.0_f64.sqrt())),
        other => panic!("normalize → Vector expected, got {other:?}"),
    }
    assert!(approx_eq(
        scalar_of(&l.magnitude(&a).unwrap()),
        14.0_f64.sqrt()
    ));
    match l.vector_add(&a, &b).unwrap() {
        EvalResult::Vector(v) => assert!(approx_eq(v[1], 7.0)),
        other => panic!("vector_add → Vector expected, got {other:?}"),
    }
    match l.vector_sub(&b, &a).unwrap() {
        EvalResult::Vector(v) => assert!(approx_eq(v[0], 3.0)),
        other => panic!("vector_sub → Vector expected, got {other:?}"),
    }
}

#[cfg(feature = "numerical")]
#[test]
fn facade_linalg_numerical() {
    let cn = CalNexus::new();
    let l = cn.linalg();
    // 对称正定矩阵，特征值 1 与 3
    let m = Matrix::from_rows(&[&[2.0, 1.0], &[1.0, 2.0]]);
    match l.eig(&m).unwrap() {
        EvalResult::Json(v) => {
            assert!(v.get("values").is_some(), "eig JSON keys: {v}");
            assert!(v.get("vectors").is_some());
        }
        other => panic!("eig → Json expected, got {other:?}"),
    }
    for (name, r) in [
        ("svd", l.svd(&m).unwrap()),
        ("lu", l.lu(&m).unwrap()),
        ("qr", l.qr(&m).unwrap()),
    ] {
        match r {
            EvalResult::Json(v) => assert!(!v.to_string().is_empty(), "{name} JSON empty"),
            other => panic!("{name} → Json expected, got {other:?}"),
        }
    }
    // A x = b：A=[[2,1],[1,2]], b=[3,3] → x=[1,1]
    let a = Matrix::from_rows(&[&[2.0, 1.0], &[1.0, 2.0]]);
    let b = Vector::new(&[3.0, 3.0]);
    match l.solve(&a, &b).unwrap() {
        EvalResult::Vector(x) => {
            assert!(approx_eq(x[0], 1.0));
            assert!(approx_eq(x[1], 1.0));
        }
        other => panic!("solve → Vector expected, got {other:?}"),
    }
    // exp([[0]]) = [[1]]
    let zero = Matrix::from_rows(&[&[0.0]]);
    match l.matrix_exp(&zero).unwrap() {
        EvalResult::Matrix(rows) => assert!(approx_eq(rows[0][0], 1.0)),
        other => panic!("matrix_exp → Matrix expected, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// 门面 · DataAnalysis
// ---------------------------------------------------------------------------

#[test]
fn facade_stats_basic() {
    let cn = CalNexus::new();
    let st = cn.stats();
    let data = [1.0, 2.0, 3.0, 4.0, 5.0];
    assert!(approx_eq(scalar_of(&st.mean(&data).unwrap()), 3.0));
    // 总体方差（除以 n 而非 n-1）：Σ(x-m)²/5 = 2.0
    assert!(approx_eq(scalar_of(&st.variance(&data).unwrap()), 2.0));
    assert!(approx_eq(
        scalar_of(&st.std(&data).unwrap()),
        2.0_f64.sqrt()
    ));
    assert!(approx_eq(scalar_of(&st.median(&data).unwrap()), 3.0));
    assert!(approx_eq(scalar_of(&st.min(&data).unwrap()), 1.0));
    assert!(approx_eq(scalar_of(&st.max(&data).unwrap()), 5.0));
    assert!(approx_eq(scalar_of(&st.sum(&data).unwrap()), 15.0));
    assert!(approx_eq(scalar_of(&st.count(&data).unwrap()), 5.0));
}

#[test]
fn facade_distributions() {
    let cn = CalNexus::new();
    let st = cn.stats();
    // 标准正态 pdf(0)=0.3989…，cdf(0)=0.5
    assert!(approx_eq(
        scalar_of(&st.norm_pdf(0.0, 0.0, 1.0).unwrap()),
        0.398_942_280_401_432_7
    ));
    assert!(approx_eq(
        scalar_of(&st.norm_cdf(0.0, 0.0, 1.0).unwrap()),
        0.5
    ));
    assert!(approx_eq(
        scalar_of(&st.norm_inv(0.5, 0.0, 1.0).unwrap()),
        0.0
    ));
    // t 分布关于 0 对称：t_cdf(0)=0.5
    assert!(approx_eq(scalar_of(&st.t_cdf(0.0, 10.0).unwrap()), 0.5));
    assert!(approx_eq(
        scalar_of(&st.t_pdf(0.0, 10.0).unwrap()),
        0.389_108_383_929_251_2
    ));
    assert!(approx_eq(scalar_of(&st.t_inv(0.5, 10.0).unwrap()), 0.0));
    // χ²：pdf 值与 cdf(0)=0
    assert!(approx_eq(
        scalar_of(&st.chi2_pdf(1.0, 2.0).unwrap()),
        0.303_265_329_877_984_8
    ));
    assert!(approx_eq(scalar_of(&st.chi2_cdf(0.0, 2.0).unwrap()), 0.0));
    assert!(scalar_of(&st.chi2_inv(0.5, 2.0).unwrap()) > 0.0);
    // F 分布 pdf 非负、分位数正值
    assert!(scalar_of(&st.f_pdf(1.0, 3.0, 5.0).unwrap()) > 0.0);
    assert!(scalar_of(&st.f_cdf(1.0, 3.0, 5.0).unwrap()) > 0.0);
    assert!(scalar_of(&st.f_inv(0.5, 3.0, 5.0).unwrap()) > 0.0);
    // Poisson: pmf(0; 1) = e^-1
    assert!(approx_eq(
        scalar_of(&st.poisson_pmf(0.0, 1.0).unwrap()),
        1.0 / std::f64::consts::E
    ));
    assert!(scalar_of(&st.poisson_cdf(2.0, 1.0).unwrap()) < 1.0);
    // Binomial: pmf(0; 3, 0.5) = 0.125
    assert!(approx_eq(
        scalar_of(&st.binom_pmf(0.0, 3.0, 0.5).unwrap()),
        0.125
    ));
    assert!(scalar_of(&st.binom_cdf(3.0, 3.0, 0.5).unwrap()) > 0.99);
}

#[test]
fn facade_hypothesis_and_correlation() {
    let cn = CalNexus::new();
    let st = cn.stats();
    // 假设检验门面把 math 层 HashMap 转成 Vector——std HashMap 随机序，
    // 契约只能钉住长度与有限性（表达式层无 t_test/chi2_test 函数，
    // 该门面即唯一入口）
    let a = [1.0, 2.0, 3.0, 4.0, 5.0];
    match st.t_test_one(&a, 3.0).unwrap() {
        EvalResult::Vector(v) => {
            assert_eq!(v.len(), 4);
            assert!(v.iter().all(|x| x.is_finite()), "all finite, got {v:?}");
            // t(mu=mean)=0 与 p=1 必在值集合中
            assert!(v.contains(&0.0), "t=0 expected in {v:?}");
            assert!(v.contains(&1.0), "p=1 expected in {v:?}");
        }
        other => panic!("t_test_one → Vector expected, got {other:?}"),
    }
    match st.t_test_two(&a, &a).unwrap() {
        EvalResult::Vector(v) => {
            assert_eq!(v.len(), 5);
            assert!(v.iter().all(|x| x.is_finite()));
            assert!(v.contains(&0.0), "identical samples → t=0 in {v:?}");
        }
        other => panic!("t_test_two → Vector expected, got {other:?}"),
    }
    let observed = [10.0, 20.0, 30.0];
    match st.chi2_test(&observed, &observed).unwrap() {
        EvalResult::Vector(v) => {
            assert_eq!(v.len(), 3);
            assert!(v.contains(&0.0), "identical obs/exp → chi2=0 in {v:?}");
        }
        other => panic!("chi2_test → Vector expected, got {other:?}"),
    }
    let x = [1.0, 2.0, 3.0, 4.0];
    let y = [2.0, 4.0, 6.0, 8.0];
    assert!(approx_eq(scalar_of(&st.pearson(&x, &y).unwrap()), 1.0));
    assert!(approx_eq(scalar_of(&st.spearman(&x, &y).unwrap()), 1.0));
    match st.lin_reg(&x, &y).unwrap() {
        EvalResult::Json(v) => {
            let slope = v
                .get("slope")
                .and_then(|s| s.as_f64())
                .expect("slope field");
            assert!(approx_eq(slope, 2.0));
        }
        other => panic!("lin_reg → Json expected, got {other:?}"),
    }
    match st.poly_reg(&x, &y, 1).unwrap() {
        EvalResult::Json(v) => assert!(!v.to_string().is_empty()),
        other => panic!("poly_reg → Json expected, got {other:?}"),
    }
    // multi_reg 特征按列组织：每列长度 == y.len()；内部自加截距，
    // 显式传全 1 列会奇异（Domain），故仅传 x 特征列
    let x2 = vec![vec![1.0, 2.0, 3.0, 4.0]];
    match st.multi_reg(&x2, &y).unwrap() {
        EvalResult::Json(v) => assert!(!v.to_string().is_empty()),
        other => panic!("multi_reg → Json expected, got {other:?}"),
    }
    // 特征列长度不匹配 → Domain
    let bad = vec![vec![1.0, 2.0]];
    let err = st.multi_reg(&bad, &y).unwrap_err();
    assert_eq!(err.kind, calnexus::ErrorKind::Domain);
}

// ---------------------------------------------------------------------------
// 门面 · SymbolicMath（既有套件缺口：cn.symbolic() 全链路）
// ---------------------------------------------------------------------------

#[test]
fn facade_symbolic_calculus() {
    let cn = CalNexus::new();
    let sym = cn.symbolic();
    match sym.differentiate("x^2", "x").unwrap() {
        EvalResult::Symbolic(s) => assert!(
            s.replace(' ', "").contains("2*x") || s.contains("2x"),
            "d/dx x^2 = 2x, got {s}"
        ),
        other => panic!("differentiate → Symbolic expected, got {other:?}"),
    }
    match sym.integrate("x", "x").unwrap() {
        EvalResult::Symbolic(s) => assert!(
            s.contains("x^2") || s.contains("x²"),
            "∫x dx = x^2/2, got {s}"
        ),
        other => panic!("integrate → Symbolic expected, got {other:?}"),
    }
    match sym.simplify("x+x").unwrap() {
        EvalResult::Symbolic(s) => {
            assert!(s.contains("2*x") || s.contains("2x"), "x+x → 2x, got {s}")
        }
        other => panic!("simplify → Symbolic expected, got {other:?}"),
    }
    assert!(approx_eq(
        scalar_of(&sym.limit("x^2", "x", 3.0).unwrap()),
        9.0
    ));
    // L'Hôpital：sin(x)/x 在 x→0 为 1
    assert!(approx_eq(
        scalar_of(&sym.limit("sin(x)/x", "x", 0.0).unwrap()),
        1.0
    ));
    match sym.taylor_expand("exp(x)", "x", 0.0, 3).unwrap() {
        EvalResult::Symbolic(s) => assert!(s.contains('x'), "taylor of exp(x), got {s}"),
        other => panic!("taylor → Symbolic expected, got {other:?}"),
    }
}

#[test]
fn facade_symbolic_polynomial() {
    let cn = CalNexus::new();
    let sym = cn.symbolic();
    let p = Polynomial::new(&[1.0, 1.0]); // 1 + x
    let q = Polynomial::new(&[2.0, 1.0]); // 2 + x
    match sym.poly_add(&p, &q).unwrap() {
        EvalResult::Polynomial(c) => {
            assert!(approx_eq(c[0], 3.0));
            assert!(approx_eq(c[1], 2.0));
        }
        other => panic!("poly_add → Polynomial expected, got {other:?}"),
    }
    match sym.poly_sub(&q, &p).unwrap() {
        EvalResult::Polynomial(c) => assert!(approx_eq(c[0], 1.0)),
        other => panic!("poly_sub → Polynomial expected, got {other:?}"),
    }
    match sym.poly_mul(&p, &q).unwrap() {
        EvalResult::Polynomial(c) => {
            assert!(approx_eq(c[2], 1.0)); // x²
            assert!(approx_eq(c[0], 2.0));
        }
        other => panic!("poly_mul → Polynomial expected, got {other:?}"),
    }
    // (x² - 1) ÷ (x - 1) = x + 1
    let num = Polynomial::new(&[-1.0, 0.0, 1.0]);
    let den = Polynomial::new(&[-1.0, 1.0]);
    match sym.poly_div(&num, &den).unwrap() {
        EvalResult::Polynomial(c) => {
            assert!(approx_eq(c[0], 1.0));
            assert!(approx_eq(c[1], 1.0));
        }
        other => panic!("poly_div → Polynomial expected, got {other:?}"),
    }
    // 二次式 x²+1 → 复根 ±i（ComplexList）；一次式回落实根 Vector
    match sym.poly_roots(&Polynomial::new(&[1.0, 0.0, 1.0])).unwrap() {
        EvalResult::ComplexList(roots) => {
            assert_eq!(roots.len(), 2);
            assert!(approx_eq(roots[0].1.abs(), 1.0) && approx_eq(roots[0].0, 0.0));
            assert!(approx_eq(roots[1].1.abs(), 1.0) && approx_eq(roots[1].0, 0.0));
        }
        other => panic!("poly_roots(x²+1) → ComplexList expected, got {other:?}"),
    }
    match sym.poly_roots(&Polynomial::new(&[0.0, 1.0])).unwrap() {
        EvalResult::Vector(roots) => {
            assert_eq!(roots.len(), 1);
            assert!(approx_eq(roots[0], 0.0));
        }
        other => panic!("poly_roots(x) → Vector expected, got {other:?}"),
    }
    assert!(approx_eq(scalar_of(&sym.poly_eval(&p, 2.0).unwrap()), 3.0));
}

#[test]
fn facade_symbolic_complex() {
    let cn = CalNexus::new();
    let sym = cn.symbolic();
    let a = Complex::new(1.0, 2.0);
    let b = Complex::new(3.0, -1.0);
    match sym.complex_add(&a, &b).unwrap() {
        EvalResult::Complex(re, im) => {
            assert!(approx_eq(re, 4.0));
            assert!(approx_eq(im, 1.0));
        }
        other => panic!("complex_add → Complex expected, got {other:?}"),
    }
    match sym.complex_sub(&a, &b).unwrap() {
        EvalResult::Complex(re, im) => {
            assert!(approx_eq(re, -2.0));
            assert!(approx_eq(im, 3.0));
        }
        other => panic!("complex_sub → Complex expected, got {other:?}"),
    }
    // (1+2i)(3-i) = 5 + 5i
    match sym.complex_mul(&a, &b).unwrap() {
        EvalResult::Complex(re, im) => {
            assert!(approx_eq(re, 5.0));
            assert!(approx_eq(im, 5.0));
        }
        other => panic!("complex_mul → Complex expected, got {other:?}"),
    }
    // z/z = 1
    match sym.complex_div(&a, &a).unwrap() {
        EvalResult::Complex(re, im) => {
            assert!(approx_eq(re, 1.0));
            assert!(approx_eq(im, 0.0));
        }
        other => panic!("complex_div → Complex expected, got {other:?}"),
    }
    assert!(approx_eq(
        scalar_of(&sym.complex_abs(&Complex::new(3.0, 4.0)).unwrap()),
        5.0
    ));
    assert!(approx_eq(
        scalar_of(&sym.complex_arg(&Complex::new(1.0, 0.0)).unwrap()),
        0.0
    ));
    match sym.complex_conj(&a).unwrap() {
        EvalResult::Complex(re, im) => {
            assert!(approx_eq(re, 1.0));
            assert!(approx_eq(im, -2.0));
        }
        other => panic!("complex_conj → Complex expected, got {other:?}"),
    }
    // e^{iπ} ≈ -1
    match sym
        .complex_exp(&Complex::new(0.0, std::f64::consts::PI))
        .unwrap()
    {
        EvalResult::Complex(re, im) => {
            assert!(approx_eq(re, -1.0));
            assert!(approx_eq(im, 0.0));
        }
        other => panic!("complex_exp → Complex expected, got {other:?}"),
    }
    match sym.complex_ln(&Complex::new(1.0, 0.0)).unwrap() {
        EvalResult::Complex(re, im) => {
            assert!(approx_eq(re, 0.0));
            assert!(approx_eq(im, 0.0));
        }
        other => panic!("complex_ln → Complex expected, got {other:?}"),
    }
}

#[test]
fn facade_symbolic_solve_equation() {
    let cn = CalNexus::new();
    let sym = cn.symbolic();
    // x² - 4 = 0 在 x0=1 附近牛顿收敛到 x=2
    let root_newton = scalar_of(
        &sym.solve_equation("x^2 - 4", "x", "newton", Some(&[1.0]))
            .unwrap(),
    );
    assert!(
        approx_eq(root_newton, 2.0),
        "newton root, got {root_newton}"
    );
    let root_bisect = scalar_of(
        &sym.solve_equation("x^2 - 4", "x", "bisection", Some(&[0.0, 10.0]))
            .unwrap(),
    );
    assert!(
        approx_eq(root_bisect, 2.0),
        "bisection root, got {root_bisect}"
    );
}

#[test]
fn symbolic_via_trait_generic_dispatch() {
    fn drive<T: SymbolicMath>(s: &T) -> String {
        match s.differentiate("x^3", "x").unwrap() {
            EvalResult::Symbolic(s) => s,
            other => panic!("Symbolic expected, got {other:?}"),
        }
    }
    let cn = CalNexus::new();
    let d = drive(&cn.symbolic());
    assert!(d.contains('3'), "d/dx x^3 should mention 3, got {d}");
}

// ---------------------------------------------------------------------------
// 门面 · AppliedMath（time/unit 门控）
// ---------------------------------------------------------------------------

#[cfg(feature = "time")]
#[test]
fn facade_time_construction_and_arithmetic() {
    let cn = CalNexus::new();
    let ap = cn.applied();
    match ap.date("2026-07-25").unwrap() {
        EvalResult::DateTime(s) => assert!(s.starts_with("2026-07-25T"), "got {s}"),
        other => panic!("date → DateTime expected, got {other:?}"),
    }
    match ap.datetime("2026-07-25 12:00:00", Some("UTC")).unwrap() {
        EvalResult::DateTime(s) => assert!(s.contains("12:00:00"), "got {s}"),
        other => panic!("datetime → DateTime expected, got {other:?}"),
    }
    match ap.timestamp("2026-01-01T00:00:00Z").unwrap() {
        EvalResult::Scalar(v) => assert!(approx_eq(v, 1_767_225_600.0)),
        other => panic!("timestamp → Scalar expected, got {other:?}"),
    }
    match ap.from_timestamp(1_767_225_600, Some("UTC")).unwrap() {
        EvalResult::DateTime(s) => assert!(s.starts_with("2026-01-01T00:00:00"), "got {s}"),
        other => panic!("from_timestamp → DateTime expected, got {other:?}"),
    }
    // date_add 跨闰月钳制：2026-01-31 + 1 month = 2026-02-28
    match ap.date_add("2026-01-31", 1, "month").unwrap() {
        EvalResult::DateTime(s) => assert!(s.starts_with("2026-02-28"), "got {s}"),
        other => panic!("date_add → DateTime expected, got {other:?}"),
    }
    assert!(approx_eq(
        scalar_of(
            &ap.date_diff("2026-01-01", "2026-07-25", Some("day"))
                .unwrap()
        ),
        205.0
    ));
    // now/today 只断言类型（值随时间变化）
    assert!(matches!(
        ap.now(Some("UTC")).unwrap(),
        EvalResult::DateTime(_)
    ));
    assert!(matches!(ap.today(None).unwrap(), EvalResult::DateTime(_)));
}

#[cfg(feature = "time")]
#[test]
fn facade_time_format_and_calendar() {
    let cn = CalNexus::new();
    let ap = cn.applied();
    match ap.format_date("2026-07-25", "%Y/%m/%d", None).unwrap() {
        EvalResult::Symbolic(s) => assert_eq!(s, "2026/07/25"),
        other => panic!("format_date expected, got {other:?}"),
    }
    match ap
        .reformat_date("2026-07-25", "%Y-%m-%d", "%d/%m/%Y")
        .unwrap()
    {
        EvalResult::Symbolic(s) => assert_eq!(s, "25/07/2026"),
        other => panic!("reformat_date expected, got {other:?}"),
    }
    // 2026-07-25 是周六（ISO weekday 6）
    assert!(approx_eq(
        scalar_of(&ap.weekday("2026-07-25").unwrap()),
        6.0
    ));
    // 2026-07-25 是当年第 206 天
    assert!(approx_eq(
        scalar_of(&ap.day_of_year("2026-07-25").unwrap()),
        206.0
    ));
    assert!(approx_eq(scalar_of(&ap.is_leap_year(2024).unwrap()), 1.0));
    assert!(approx_eq(scalar_of(&ap.is_leap_year(2026).unwrap()), 0.0));
    assert!(approx_eq(scalar_of(&ap.is_leap_year(2000).unwrap()), 1.0));
    assert!(approx_eq(scalar_of(&ap.is_leap_year(1900).unwrap()), 0.0));
}

#[cfg(feature = "unit")]
#[test]
fn facade_unit_convert() {
    let cn = CalNexus::new();
    let ap = cn.applied();
    assert!(approx_eq(
        scalar_of(&ap.convert(5.0, "km", "m").unwrap()),
        5000.0
    ));
    assert!(approx_eq(
        scalar_of(&ap.convert(1.0, "kg", "g").unwrap()),
        1000.0
    ));
    // 温度：100°C = 212°F
    assert!(approx_eq(
        scalar_of(&ap.convert(100.0, "C", "F").unwrap()),
        212.0
    ));
    // 数据：1 KiB = 1024 B（二进制前缀与十进制 KB 区分）
    assert!(approx_eq(
        scalar_of(&ap.convert(1.0, "KiB", "B").unwrap()),
        1024.0
    ));
}

// ---------------------------------------------------------------------------
// evaluate 管线 · EvalResult 变体矩阵
// ---------------------------------------------------------------------------

#[test]
fn pipeline_variant_scalar_and_matrix_and_vector() {
    let (r, domain) = eval_ok_with_domain("2+3*4");
    assert_scalar(&r, 14.0);
    assert_eq!(domain, "arithmetic");

    let (r, domain) = eval_ok_with_domain("[[1,2],[3,4]]");
    match r {
        EvalResult::Matrix(rows) => assert_eq!(rows.len(), 2),
        other => panic!("Matrix expected, got {other:?}"),
    }
    assert_eq!(domain, "matrix");

    let (r, _) = eval_ok_with_domain("[1,2]+[3,4]");
    match r {
        EvalResult::Vector(v) => {
            assert!(approx_eq(v[0], 4.0));
            assert!(approx_eq(v[1], 6.0));
        }
        other => panic!("Vector expected, got {other:?}"),
    }
}

#[test]
fn pipeline_variant_complex_and_complex_list() {
    let (r, _) = eval_ok_with_domain("3+4i");
    match r {
        EvalResult::Complex(re, im) => {
            assert!(approx_eq(re, 3.0));
            assert!(approx_eq(im, 4.0));
        }
        other => panic!("Complex expected, got {other:?}"),
    }
    let (r, domain) = eval_ok_with_domain("roots(x^2+1)");
    match r {
        EvalResult::ComplexList(roots) => assert_eq!(roots.len(), 2),
        other => panic!("ComplexList expected, got {other:?}"),
    }
    assert_eq!(domain, "polynomial");
}

#[test]
fn pipeline_variant_polynomial_and_symbolic() {
    let (r, _) = eval_ok_with_domain("poly_add(x+1,x+2)");
    match r {
        EvalResult::Polynomial(c) => {
            assert!(approx_eq(c[0], 3.0));
            assert!(approx_eq(c[1], 2.0));
        }
        other => panic!("Polynomial expected, got {other:?}"),
    }
    let (r, domain) = eval_ok_with_domain("diff(x^2,x)");
    match r {
        EvalResult::Symbolic(s) => assert!(s.contains('2'), "got {s}"),
        other => panic!("Symbolic expected, got {other:?}"),
    }
    assert_eq!(domain, "symbolic");
}

#[test]
fn pipeline_variant_bigint_and_bigrational() {
    let (r, domain) = eval_ok_with_domain("123456789012345678901234567890");
    match r {
        EvalResult::BigInt(v) => assert_eq!(v.to_string(), "123456789012345678901234567890"),
        other => panic!("BigInt expected, got {other:?}"),
    }
    assert_eq!(domain, "precision");

    let (r, _) = eval_ok_with_domain("precision(5, 1/3)");
    match r {
        EvalResult::BigRational(q) => {
            assert_eq!(format_bigrational(&q, Some(5)), "0.33333");
        }
        other => panic!("BigRational expected, got {other:?}"),
    }
}

#[cfg(feature = "time")]
#[test]
fn pipeline_variant_datetime() {
    let (r, domain) = eval_ok_with_domain("date(\"2026-07-25\")");
    match r {
        EvalResult::DateTime(s) => assert!(s.starts_with("2026-07-25T")),
        other => panic!("DateTime expected, got {other:?}"),
    }
    assert_eq!(domain, "time");
}

#[cfg(feature = "numerical")]
#[test]
fn pipeline_variant_json() {
    let (r, domain) = eval_ok_with_domain("lu([[2,1],[1,2]])");
    match r {
        EvalResult::Json(v) => {
            assert!(v.get("L").is_some() && v.get("U").is_some(), "lu keys: {v}");
        }
        other => panic!("Json expected, got {other:?}"),
    }
    assert_eq!(domain, "matrix");
}

// ---------------------------------------------------------------------------
// 可选域表达式正常路径（time/unit/numerical）
// ---------------------------------------------------------------------------

#[cfg(feature = "time")]
#[test]
fn pipeline_time_expressions() {
    // 多格式日期解析（ISO / 斜杠 / 紧凑数字）
    for (d, expected_start) in [
        ("2026-07-25", "2026-07-25"),
        ("2026/07/25", "2026-07-25"),
        ("20260725", "2026-07-25"),
    ] {
        let (r, _) = eval_ok_with_domain(&format!("date(\"{d}\")"));
        match r {
            EvalResult::DateTime(s) => assert!(s.starts_with(expected_start), "{d} → {s}"),
            other => panic!("date({d}) → DateTime expected, got {other:?}"),
        }
    }
    let (r, _) = eval_ok_with_domain("date_diff(\"2026-01-01\", \"2026-01-02\", \"hour\")");
    assert_scalar(&r, 24.0);
    let (r, _) = eval_ok_with_domain("date_add(\"2026-12-31\", 1, \"day\")");
    match r {
        EvalResult::DateTime(s) => assert!(s.starts_with("2027-01-01"), "got {s}"),
        other => panic!("expected DateTime, got {other:?}"),
    }
    let (r, _) = eval_ok_with_domain("weekday(\"2026-09-14\")");
    assert_scalar(&r, 1.0); // 周一
    let (r, _) = eval_ok_with_domain("is_leap_year(2024)");
    assert_scalar(&r, 1.0);
}

#[cfg(feature = "unit")]
#[test]
fn pipeline_unit_expressions() {
    for (expr, expected) in [
        ("convert(5, \"km\", \"m\")", 5000.0),
        ("convert(1, \"m\", \"cm\")", 100.0),
        ("convert(0, \"C\", \"K\")", 273.15),
        ("convert(1, \"MB\", \"KB\")", 1000.0),
        ("convert(1, \"h\", \"min\")", 60.0),
    ] {
        let (r, domain) = eval_ok_with_domain(expr);
        assert_scalar(&r, expected);
        assert_eq!(domain, "unit", "{expr}");
    }
}

#[cfg(feature = "numerical")]
#[test]
fn pipeline_numerical_expressions() {
    for (expr, keys) in [
        ("lu([[4,2],[2,3]])", vec!["L", "U"]),
        ("qr([[1,2],[3,4]])", vec!["Q", "R"]),
        ("eig([[2,0],[0,3]])", vec!["values", "vectors"]),
        ("svd([[3,0],[0,2]])", vec!["U", "S"]),
    ] {
        let (r, domain) = eval_ok_with_domain(expr);
        match r {
            EvalResult::Json(v) => {
                for k in keys {
                    assert!(v.get(k).is_some(), "{expr} missing key {k}: {v}");
                }
            }
            other => panic!("{expr} → Json expected, got {other:?}"),
        }
        assert_eq!(domain, "matrix", "{expr}");
    }
    let (r, _) = eval_ok_with_domain("solve([[2,1],[1,2]], [3,3])");
    match r {
        EvalResult::Vector(x) => {
            assert!(approx_eq(x[0], 1.0));
            assert!(approx_eq(x[1], 1.0));
        }
        other => panic!("solve → Vector expected, got {other:?}"),
    }
}
