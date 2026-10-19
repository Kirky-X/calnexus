// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! 科学函数核心：三角/对数/指数/双曲/gamma/erf。

use crate::core::CalcError;

/// Lanczos 逼近系数（g=7, n=9），用于 gamma 函数。
const LANCZOS_G: f64 = 7.0;
const LANCZOS_COEF: [f64; 9] = [
    0.999_999_999_999_809_9,
    676.5203681218851,
    -1259.1392167224028,
    771.323_428_777_653_1,
    -176.615_029_162_140_6,
    12.507343278686905,
    -0.13857109526572012,
    9.984_369_578_019_572e-6,
    1.5056327351493116e-7,
];

/// Abramowitz & Stegun 7.1.26 逼近系数，用于 erf 函数（最大误差 ~1.5e-7）。
const ERF_A1: f64 = 0.254829592;
const ERF_A2: f64 = -0.284496736;
const ERF_A3: f64 = 1.421413741;
const ERF_A4: f64 = -1.453152027;
const ERF_A5: f64 = 1.061405429;
const ERF_P: f64 = 0.3275911;

// ===== 辅助函数 =====

/// 检查结果是否有限，非有限返回 NaNOrInf。
fn check_finite(value: f64) -> Result<f64, CalcError> {
    if !value.is_finite() {
        return Err(CalcError::nan_or_inf());
    }
    Ok(value)
}

// ===== 三角函数 =====

/// 正弦函数。NaN/Inf 输入 → NaNOrInf。
pub fn sin(x: f64) -> Result<f64, CalcError> {
    check_finite(x.sin())
}

/// 余弦函数。
pub fn cos(x: f64) -> Result<f64, CalcError> {
    check_finite(x.cos())
}

/// 正切函数。
pub fn tan(x: f64) -> Result<f64, CalcError> {
    check_finite(x.tan())
}

// ===== 反三角函数 =====

/// 反正弦函数。输入须在 [-1, 1]，否则返回 Domain。
pub fn asin(x: f64) -> Result<f64, CalcError> {
    if !(-1.0..=1.0).contains(&x) {
        return Err(
            CalcError::domain(format!("asin requires argument in [-1, 1], got {}", x))
                .with_hint("asin domain is [-1, 1]")
                .with_hint_i18n("hint.asin_domain", vec![])
                .with_i18n(
                    "msg.scientific.asin_domain",
                    vec![("value".to_string(), x.to_string())],
                ),
        );
    }
    check_finite(x.asin())
}

/// 反余弦函数。输入须在 [-1, 1]，否则返回 Domain。
pub fn acos(x: f64) -> Result<f64, CalcError> {
    if !(-1.0..=1.0).contains(&x) {
        return Err(
            CalcError::domain(format!("acos requires argument in [-1, 1], got {}", x))
                .with_hint("acos domain is [-1, 1]")
                .with_hint_i18n("hint.acos_domain", vec![])
                .with_i18n(
                    "msg.scientific.acos_domain",
                    vec![("value".to_string(), x.to_string())],
                ),
        );
    }
    check_finite(x.acos())
}

/// 反正切函数。
pub fn atan(x: f64) -> Result<f64, CalcError> {
    check_finite(x.atan())
}

// ===== 对数函数 =====

/// 自然对数。`x <= 0.0` 返回 Domain。
pub fn ln(x: f64) -> Result<f64, CalcError> {
    if x <= 0.0 {
        return Err(
            CalcError::domain(format!("ln requires positive argument, got {}", x)).with_i18n(
                "msg.scientific.ln_positive",
                vec![("value".to_string(), x.to_string())],
            ),
        );
    }
    check_finite(x.ln())
}

/// 以 10 为底的对数。`x <= 0.0` 返回 Domain。
pub fn log10(x: f64) -> Result<f64, CalcError> {
    if x <= 0.0 {
        return Err(
            CalcError::domain(format!("log10 requires positive argument, got {}", x)).with_i18n(
                "msg.scientific.log10_positive",
                vec![("value".to_string(), x.to_string())],
            ),
        );
    }
    check_finite(x.log10())
}

/// 以 2 为底的对数。`x <= 0.0` 返回 Domain。
pub fn log2(x: f64) -> Result<f64, CalcError> {
    if x <= 0.0 {
        return Err(
            CalcError::domain(format!("log2 requires positive argument, got {}", x)).with_i18n(
                "msg.scientific.log2_positive",
                vec![("value".to_string(), x.to_string())],
            ),
        );
    }
    check_finite(x.log2())
}

/// 任意底对数 `log(value, base)`：value 或 base 非正、base==1 返回 Domain。
pub fn log(value: f64, base: f64) -> Result<f64, CalcError> {
    if value <= 0.0 {
        return Err(
            CalcError::domain(format!("log requires positive value, got {}", value)).with_i18n(
                "msg.scientific.log_positive_value",
                vec![("value".to_string(), value.to_string())],
            ),
        );
    }
    if base <= 0.0 || (base - 1.0).abs() < f64::EPSILON {
        return Err(
            CalcError::domain(format!("log requires positive base != 1, got {}", base)).with_i18n(
                "msg.scientific.log_positive_base",
                vec![("value".to_string(), base.to_string())],
            ),
        );
    }
    check_finite(value.log(base))
}

// ===== 指数函数 =====

/// 指数函数 `e^x`。溢出返回 NaNOrInf。
pub fn exp(x: f64) -> Result<f64, CalcError> {
    check_finite(x.exp())
}

// ===== 双曲函数 =====

/// 双曲正弦。
pub fn sinh(x: f64) -> Result<f64, CalcError> {
    check_finite(x.sinh())
}

/// 双曲余弦。
pub fn cosh(x: f64) -> Result<f64, CalcError> {
    check_finite(x.cosh())
}

/// 双曲正切。
pub fn tanh(x: f64) -> Result<f64, CalcError> {
    check_finite(x.tanh())
}

// ===== 特殊函数 =====

/// gamma 函数：Lanczos 逼近 + 反射公式；非正整数极点返回 Domain。
pub fn gamma(x: f64) -> Result<f64, CalcError> {
    // 非正整数（0, -1, -2, …）是 gamma 函数的极点
    if x <= 0.0 && x == x.floor() && x.is_finite() {
        return Err(CalcError::domain(format!(
            "gamma({}) is undefined: pole at non-positive integer",
            x
        ))
        .with_hint("gamma is defined for positive reals and non-integer negatives")
        .with_hint_i18n("hint.gamma_domain", vec![])
        .with_i18n(
            "msg.scientific.gamma_pole",
            vec![("value".to_string(), x.to_string())],
        ));
    }
    check_finite(lanczos_gamma(x))
}

/// Abramowitz & Stegun 7.1.26 逼近计算 erf 函数（最大误差 ~1.5e-7）。
pub fn erf(x: f64) -> Result<f64, CalcError> {
    check_finite(erf_raw(x))
}

/// 互补误差函数 erfc(x) = 1 - erf(x)：委托 libm fdlibm（全实轴精度 ~1e-16）。
/// 大 x 区间不因相消误差归零（区别于 `1 - erf(x)` 的 A&S 逼近路径）。
pub fn erfc(x: f64) -> Result<f64, CalcError> {
    check_finite(libm::erfc(x))
}

/// 对数 Gamma 函数 lgamma(x) = ln|Γ(x)|：委托 libm fdlibm。
/// 覆盖 `gamma` 溢出的大参数区间（如 lgamma(200) 有限）；负整数极点返回 inf 被 check_finite 拒绝。
pub fn lgamma(x: f64) -> Result<f64, CalcError> {
    check_finite(libm::lgamma(x))
}

/// 二/三阶对数积分 polylog(s, z)，`s ∈ {2, 3}`。
///
/// 实现策略：`|z| < 1` 用幂级数 `Σ z^k / k^s`（收敛快，`z=0.9` 约 350 项达 1e-16）；
/// `z = 1` 复用 `zeta(s)`；`z = -1` 用解析值（`Li2(-1) = -π²/12`、`Li3(-1) = -3ζ(3)/4`）；
/// `|z| > 1` 或 `s ∉ {2,3}` 返回 `Domain`；`|z|` 极度接近 1 时级数不收敛，显性返回 `Domain`。
pub fn polylog(s: u32, z: f64) -> Result<f64, CalcError> {
    if s != 2 && s != 3 {
        return Err(CalcError::domain(format!(
            "polylog() order must be 2 or 3, got {}",
            s
        )));
    }
    if !z.is_finite() || !(-1.0..=1.0).contains(&z) {
        return Err(CalcError::domain(format!(
            "polylog({}, {}) is undefined: |z| must be <= 1",
            s, z
        )));
    }
    if z == 1.0 {
        return zeta(s as f64);
    }
    if z == -1.0 {
        return match s {
            2 => Ok(-std::f64::consts::PI * std::f64::consts::PI / 12.0),
            _ => Ok(-0.75 * zeta(3.0)?),
        };
    }
    if z.abs() >= 1.0 - 1e-6 {
        return Err(CalcError::domain(format!(
            "polylog({}, {}) is too close to 1: series would not converge",
            s, z
        )));
    }
    let mut sum = 0.0f64;
    let mut power = z;
    let mut k = 1u64;
    loop {
        let term = power / (k as f64).powi(s as i32);
        sum += term;
        if term.abs() < 1e-17 || k >= 5_000_000 {
            break;
        }
        power *= z;
        k += 1;
    }
    check_finite(sum)
}

/// Lanczos 逼近计算 gamma 函数（内部，返回原始 f64）。
fn lanczos_gamma(x: f64) -> f64 {
    if x < 0.5 {
        std::f64::consts::PI / ((std::f64::consts::PI * x).sin() * lanczos_gamma(1.0 - x))
    } else {
        let x = x - 1.0;
        let mut a = LANCZOS_COEF[0];
        let t = x + LANCZOS_G + 0.5;
        for (i, coef) in LANCZOS_COEF.iter().enumerate().skip(1) {
            a += coef / (x + i as f64);
        }
        (2.0 * std::f64::consts::PI).sqrt() * t.powf(x + 0.5) * (-t).exp() * a
    }
}

/// A&S 7.1.26 erf 逼近（内部，返回原始 f64）。
fn erf_raw(x: f64) -> f64 {
    // erf(0) = 0 数学上精确，A&S 逼近在此点有 ~1e-7 误差，特判以保证 spec 精确匹配。
    if x == 0.0 {
        return 0.0;
    }
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();
    let t = 1.0 / (1.0 + ERF_P * x);
    let y = 1.0
        - (((((ERF_A5 * t + ERF_A4) * t) + ERF_A3) * t + ERF_A2) * t + ERF_A1) * t * (-x * x).exp();
    sign * y
}

// ===== Bessel 函数（J/Y 族委托 libm fdlibm）=====

/// Bessel 阶数上限（防递推不稳定与 CPU DoS）。
const MAX_BESSEL_ORDER: u32 = 100;

/// 阶数上限校验（trait 与域路由共用同一口径）。
fn require_order(n: u32, name: &str) -> Result<(), CalcError> {
    if n > MAX_BESSEL_ORDER {
        return Err(CalcError::domain(format!(
            "{}() order must be <= {}, got {}",
            name, MAX_BESSEL_ORDER, n
        ))
        .with_i18n(
            "msg.scientific.bessel_order_invalid",
            vec![
                ("name".to_string(), name.to_string()),
                ("value".to_string(), n.to_string()),
            ],
        ));
    }
    Ok(())
}

/// 0 阶第一类 Bessel 函数 J0(x)。
pub fn bessel_j0(x: f64) -> Result<f64, CalcError> {
    check_finite(libm::j0(x))
}

/// 1 阶第一类 Bessel 函数 J1(x)。
pub fn bessel_j1(x: f64) -> Result<f64, CalcError> {
    check_finite(libm::j1(x))
}

/// 整数阶第一类 Bessel 函数 J_n(x)。
pub fn bessel_j(n: u32, x: f64) -> Result<f64, CalcError> {
    require_order(n, "besselj")?;
    check_finite(libm::jn(n as i32, x))
}

/// 0 阶第二类 Bessel 函数 Y0(x)；x <= 0 返回 Domain。
pub fn bessel_y0(x: f64) -> Result<f64, CalcError> {
    require_positive(x, "bessely")?;
    check_finite(libm::y0(x))
}

/// 1 阶第二类 Bessel 函数 Y1(x)；x <= 0 返回 Domain。
pub fn bessel_y1(x: f64) -> Result<f64, CalcError> {
    require_positive(x, "bessely")?;
    check_finite(libm::y1(x))
}

/// 整数阶第二类 Bessel 函数 Y_n(x)；x <= 0 返回 Domain。
pub fn bessel_y(n: u32, x: f64) -> Result<f64, CalcError> {
    require_order(n, "bessely")?;
    require_positive(x, "bessely")?;
    check_finite(libm::yn(n as i32, x))
}

/// 非正参数拒绝（Y/K 族定义域）。
fn require_positive(x: f64, name: &str) -> Result<(), CalcError> {
    if x <= 0.0 {
        return Err(
            CalcError::domain(format!("{}() requires x > 0, got {}", name, x)).with_i18n(
                "msg.scientific.bessel_positive_domain",
                vec![
                    ("name".to_string(), name.to_string()),
                    ("value".to_string(), x.to_string()),
                ],
            ),
        );
    }
    Ok(())
}

// ===== 修正 Bessel 函数 I/K（A&S 9.8）=====

/// 0 阶第一类修正 Bessel 函数 I0(x)。
pub fn bessel_i0(x: f64) -> Result<f64, CalcError> {
    check_finite(bessel_i0_raw(x))
}

/// 1 阶第一类修正 Bessel 函数 I1(x)。
pub fn bessel_i1(x: f64) -> Result<f64, CalcError> {
    check_finite(bessel_i1_raw(x))
}

/// 整数阶第一类修正 Bessel 函数 I_n(x)。
pub fn bessel_i(n: u32, x: f64) -> Result<f64, CalcError> {
    require_order(n, "besseli")?;
    match n {
        0 => bessel_i0(x),
        1 => bessel_i1(x),
        _ => {
            if x == 0.0 {
                return Ok(0.0);
            }
            let mut i_prev = bessel_i0_raw(x);
            let mut i_curr = bessel_i1_raw(x);
            for k in 1..n {
                let i_next = i_prev - (2.0 * k as f64 / x) * i_curr;
                i_prev = i_curr;
                i_curr = i_next;
            }
            check_finite(i_curr)
        }
    }
}

/// I0 原始实现（A&S 9.8.1）。
fn bessel_i0_raw(x: f64) -> f64 {
    let ax = x.abs();
    if ax < 3.75 {
        let t = x / 3.75;
        let t2 = t * t;
        1.0 + t2
            * (3.5156229
                + t2 * (3.0899424
                    + t2 * (1.2067492 + t2 * (0.2659732 + t2 * (0.0360768 + t2 * 0.0045813)))))
    } else {
        let u = 3.75 / ax;
        let p = 0.39894228
            + u * (0.01328592
                + u * (0.00225319
                    + u * (-0.00157565
                        + u * (0.00916281
                            + u * (-0.02057706
                                + u * (0.02635537 + u * (-0.01647633 + u * 0.00392377)))))));
        ax.exp() * p / ax.sqrt()
    }
}

/// I1 原始实现（A&S 9.8.3）。
fn bessel_i1_raw(x: f64) -> f64 {
    let ax = x.abs();
    if ax < 3.75 {
        let t = x / 3.75;
        let t2 = t * t;
        x * (0.5
            + t2 * (0.87890594
                + t2 * (0.51498869
                    + t2 * (0.15084934 + t2 * (0.02658733 + t2 * (0.00301532 + t2 * 0.00032411))))))
    } else {
        let u = 3.75 / ax;
        let p = 0.39894228
            + u * (-0.03988024
                + u * (-0.00362018
                    + u * (0.00163801
                        + u * (-0.01031555
                            + u * (0.02282967
                                + u * (-0.02895312 + u * (0.01787654 + u * (-0.00420059))))))));
        let v = ax.exp() * p / ax.sqrt();
        if x < 0.0 { -v } else { v }
    }
}

/// 0 阶第二类修正 Bessel 函数 K0(x)；x <= 0 返回 Domain。
pub fn bessel_k0(x: f64) -> Result<f64, CalcError> {
    require_positive(x, "besselk")?;
    check_finite(bessel_k0_raw(x))
}

/// 1 阶第二类修正 Bessel 函数 K1(x)；x <= 0 返回 Domain。
pub fn bessel_k1(x: f64) -> Result<f64, CalcError> {
    require_positive(x, "besselk")?;
    check_finite(bessel_k1_raw(x))
}

/// 整数阶第二类修正 Bessel 函数 K_n(x)；x <= 0 返回 Domain。
pub fn bessel_k(n: u32, x: f64) -> Result<f64, CalcError> {
    require_order(n, "besselk")?;
    match n {
        0 => bessel_k0(x),
        1 => bessel_k1(x),
        _ => {
            require_positive(x, "besselk")?;
            let mut k_prev = bessel_k0_raw(x);
            let mut k_curr = bessel_k1_raw(x);
            for j in 1..n {
                let k_next = k_prev + (2.0 * j as f64 / x) * k_curr;
                k_prev = k_curr;
                k_curr = k_next;
            }
            check_finite(k_curr)
        }
    }
}

/// K0 原始实现：`-[ln(x/2)+γ]I0 + Σ H_k/(k!)²·(x/2)^{2k}`（系数经 mpmath 校验）。
fn bessel_k0_raw(x: f64) -> f64 {
    if x < 2.0 {
        const H: [f64; 6] = [1.0, 1.5, 11.0 / 6.0, 25.0 / 12.0, 137.0 / 60.0, 49.0 / 20.0];
        const FACT_SQ: [f64; 6] = [1.0, 4.0, 36.0, 576.0, 14400.0, 518400.0];
        let z = (x / 2.0) * (x / 2.0);
        let mut poly = 0.0;
        let mut power = z;
        for k in 0..6 {
            poly += H[k] / FACT_SQ[k] * power;
            power *= z;
        }
        -((x / 2.0).ln() + EULER_GAMMA) * bessel_i0_raw(x) + poly
    } else {
        let u = 2.0 / x;
        let p = 1.25331414
            + u * (-0.07832358
                + u * (0.02189568
                    + u * (-0.01062446 + u * (0.00587872 + u * (-0.00251540 + u * 0.00053208)))));
        (-x).exp() * p / x.sqrt()
    }
}

/// K1 原始实现：`[x(ln(x/2)+γ)I1 + Σ b_k(x/2)^{2k}] / x`（b_k 由 mpmath 反推）。
fn bessel_k1_raw(x: f64) -> f64 {
    if x < 2.0 {
        const B: [f64; 7] = [
            1.0,
            -1.0,
            -1.25,
            -0.27777653,
            -0.02720293,
            -0.00151013,
            -0.0000594864,
        ];
        let h = x / 2.0;
        let z = h * h;
        let mut poly = 0.0;
        let mut power = 1.0;
        for b in B {
            poly += b * power;
            power *= z;
        }
        (((x / 2.0).ln() + EULER_GAMMA) * bessel_i1_raw(x) + poly) / x
    } else {
        let u = 2.0 / x;
        let p = 1.25331414
            + u * (0.23498619
                + u * (-0.03655620
                    + u * (0.01504268 + u * (-0.00780353 + u * (0.00325614 + u * (-0.00068245))))));
        (-x).exp() * p / x.sqrt()
    }
}

/// Euler–Mascheroni 常数。
const EULER_GAMMA: f64 = 0.5772156649015329;

// ===== Beta / Riemann Zeta =====

/// Beta 函数 B(a,b) = Γ(a)Γ(b)/Γ(a+b)；a,b > 0。
pub fn beta(a: f64, b: f64) -> Result<f64, CalcError> {
    if a <= 0.0 || b <= 0.0 {
        return Err(
            CalcError::domain(format!("beta() requires a, b > 0, got ({}, {})", a, b)).with_i18n(
                "msg.scientific.beta_domain",
                vec![
                    ("a".to_string(), a.to_string()),
                    ("b".to_string(), b.to_string()),
                ],
            ),
        );
    }
    check_finite(lanczos_gamma(a) * lanczos_gamma(b) / lanczos_gamma(a + b))
}

/// Riemann Zeta 函数 ζ(s)：`s>0 且 s≠1` 用 Euler–Maclaurin；`s<0` 用函数方程；`s=0` 返回 -0.5。
pub fn zeta(s: f64) -> Result<f64, CalcError> {
    if !s.is_finite() {
        return Err(CalcError::nan_or_inf());
    }
    if s == 1.0 {
        return Err(CalcError::domain("zeta(1) is undefined: pole".to_string())
            .with_i18n("msg.scientific.zeta_pole", vec![]));
    }
    if s == 0.0 {
        return Ok(-0.5);
    }
    if s < 0.0 {
        // 函数方程：ζ(s) = 2^s π^(s-1) sin(πs/2) Γ(1-s) ζ(1-s)
        let value = 2f64.powf(s)
            * std::f64::consts::PI.powf(s - 1.0)
            * (std::f64::consts::PI * s / 2.0).sin()
            * lanczos_gamma(1.0 - s)
            * zeta(1.0 - s)?;
        return check_finite(value);
    }
    check_finite(zeta_em(s))
}

/// Euler–Maclaurin 展开（N=20，Bernoulli B2..B20）。
fn zeta_em(s: f64) -> f64 {
    const N: usize = 20;
    const BERNOULLI: [f64; 10] = [
        1.0 / 6.0,
        -1.0 / 30.0,
        1.0 / 42.0,
        -1.0 / 30.0,
        5.0 / 66.0,
        -691.0 / 2730.0,
        7.0 / 6.0,
        -3617.0 / 510.0,
        43867.0 / 798.0,
        -174611.0 / 330.0,
    ];
    let n = N as f64;
    let mut total: f64 = (1..N).map(|k| (k as f64).powf(-s)).sum();
    total += n.powf(1.0 - s) / (s - 1.0);
    total += 0.5 * n.powf(-s);
    let mut rising = s;
    let mut factorial = 2.0;
    for (idx, &b) in BERNOULLI.iter().enumerate() {
        let j = idx + 1;
        if j > 1 {
            let jf = j as f64;
            rising *= (s + 2.0 * jf - 3.0) * (s + 2.0 * jf - 2.0);
            factorial *= (2.0 * jf - 1.0) * (2.0 * jf);
        }
        total += b / factorial * rising * n.powf(-s - 2.0 * j as f64 + 1.0);
    }
    total
}

// ===== Airy 函数（A&S 10.4）=====

/// Airy Ai 函数。
pub fn airy_ai(x: f64) -> Result<f64, CalcError> {
    check_finite(airy_pair(x).0)
}

/// Airy Bi 函数。
pub fn airy_bi(x: f64) -> Result<f64, CalcError> {
    check_finite(airy_pair(x).1)
}

/// `|x| <= 8` 用 Maclaurin 级数，否则用四项渐近展开。
fn airy_pair(x: f64) -> (f64, f64) {
    if x.abs() <= 8.0 {
        airy_series(x)
    } else {
        airy_asymptotic(x)
    }
}

/// Maclaurin 级数：Ai = c1·f - c2·g，Bi = √3(c1·f + c2·g)。
fn airy_series(x: f64) -> (f64, f64) {
    const C1: f64 = 0.3550280538878172;
    const C2: f64 = 0.2588194037928068;
    let x3 = x * x * x;
    let mut f = 1.0;
    let mut g = x;
    let mut term_f = 1.0;
    let mut term_g = x;
    for k in 1..=80 {
        let kf = k as f64;
        term_f *= x3 / ((3.0 * kf - 1.0) * (3.0 * kf));
        term_g *= x3 / ((3.0 * kf) * (3.0 * kf + 1.0));
        f += term_f;
        g += term_g;
    }
    (C1 * f - C2 * g, (3.0f64).sqrt() * (C1 * f + C2 * g))
}

/// A&S 10.4 四项渐近展开（u1=5/72, u2=385/10368, u3=85085/2239488）。
fn airy_asymptotic(x: f64) -> (f64, f64) {
    const U1: f64 = 5.0 / 72.0;
    const U2: f64 = 385.0 / 10368.0;
    const U3: f64 = 85085.0 / 2239488.0;
    let ax = x.abs();
    let zeta = (2.0 / 3.0) * ax.powf(1.5);
    let inv = 1.0 / zeta;
    let common = 1.0 / (std::f64::consts::PI.sqrt() * ax.powf(0.25));
    if x > 0.0 {
        let ai =
            (-zeta).exp() * 0.5 * common * (1.0 - U1 * inv + U2 * inv * inv - U3 * inv * inv * inv);
        let bi = zeta.exp() * common * (1.0 + U1 * inv + U2 * inv * inv + U3 * inv * inv * inv);
        (ai, bi)
    } else {
        let phase = zeta + std::f64::consts::FRAC_PI_4;
        let (s, c) = phase.sin_cos();
        let even = 1.0 - U2 * inv * inv;
        let odd = U1 * inv - U3 * inv * inv * inv;
        (common * (s * even - c * odd), common * (c * even + s * odd))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ErrorKind;

    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-10
    }

    fn approx_rel(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol * b.abs().max(1.0)
    }

    // ===== 特殊函数：erfc =====

    #[test]
    fn test_erfc_small_and_large_args() {
        assert!(approx_rel(erfc(0.0).unwrap(), 1.0, 1e-12));
        assert!(approx_rel(
            erfc(1.0).unwrap(),
            0.157_299_207_050_285_1,
            1e-12
        ));
        assert!(approx_rel(
            erfc(2.0).unwrap(),
            0.004_677_734_981_186_824,
            1e-12
        ));
        let v = erfc(10.0).unwrap();
        assert!(
            (v / 2.088_487_583_762_545e-45 - 1.0).abs() < 1e-9,
            "erfc(10) = {v}"
        );
    }

    #[test]
    fn test_erfc_monotonic_and_non_negative() {
        let a = erfc(2.0).unwrap();
        let b = erfc(3.0).unwrap();
        assert!(a > b && b >= 0.0, "erfc(2)={a}, erfc(3)={b}");
        assert!(erfc(-10.0).unwrap() > 1.0);
    }

    // ===== 特殊函数：lgamma =====

    #[test]
    fn test_lgamma_known_values() {
        assert!(approx_rel(lgamma(1.0).unwrap(), 0.0, 1e-12));
        assert!(approx_rel(
            lgamma(0.5).unwrap(),
            0.572_364_942_924_700_1,
            1e-12
        ));
        assert!(approx_rel(
            lgamma(10.0).unwrap(),
            12.801_827_480_081_469,
            1e-12
        ));
    }

    #[test]
    fn test_lgamma_reflection_and_large_arg() {
        // 负半整数：ln|Γ(-0.5)| = ln(2√π)
        assert!(approx_rel(
            lgamma(-0.5).unwrap(),
            1.265_512_123_484_645_4,
            1e-12
        ));
        // gamma(200) 溢出，但 lgamma 保持有限
        let v = lgamma(200.0).unwrap();
        assert!(v.is_finite() && v > 800.0, "lgamma(200) = {v}");
    }

    // ===== 特殊函数：polylog =====

    #[test]
    fn test_polylog_known_values() {
        let pi2_over_6 = std::f64::consts::PI * std::f64::consts::PI / 6.0;
        assert!(approx_rel(polylog(2, 1.0).unwrap(), pi2_over_6, 1e-12));
        assert!(approx_rel(
            polylog(2, 0.5).unwrap(),
            0.582_240_526_465_012_5,
            1e-12
        ));
        assert!(approx_rel(
            polylog(3, 0.5).unwrap(),
            0.537_213_193_608_040_2,
            1e-12
        ));
    }

    #[test]
    fn test_polylog_domain_errors() {
        assert!(matches!(polylog(4, 0.5), Err(e) if e.kind == ErrorKind::Domain));
        assert!(matches!(polylog(2, 1.5), Err(e) if e.kind == ErrorKind::Domain));
    }

    // ===== Bessel J 族 =====

    #[test]
    fn test_bessel_j0_at_one() {
        assert!(approx_rel(
            bessel_j0(1.0).unwrap(),
            0.7651976865579666,
            1e-12
        ));
    }

    #[test]
    fn test_bessel_j1_at_one() {
        assert!(approx_rel(
            bessel_j1(1.0).unwrap(),
            0.4400505857449335,
            1e-12
        ));
    }

    #[test]
    fn test_bessel_j2_at_one() {
        assert!(approx_rel(
            bessel_j(2, 1.0).unwrap(),
            0.1149034849319005,
            1e-12
        ));
    }

    #[test]
    fn test_bessel_j_negative_arg_is_odd_for_odd_order() {
        let pos = bessel_j(1, 2.0).unwrap();
        let neg = bessel_j(1, -2.0).unwrap();
        assert!(approx_rel(neg, -pos, 1e-12));
    }

    // ===== Bessel Y 族 =====

    #[test]
    fn test_bessel_y0_at_one() {
        let v = bessel_y0(1.0).unwrap();
        assert!(approx_rel(v, 0.08825696421567697, 1e-12), "got {v}");
    }

    #[test]
    fn test_bessel_y1_at_one() {
        assert!(approx_rel(
            bessel_y1(1.0).unwrap(),
            -0.7812128213002887,
            1e-12
        ));
    }

    #[test]
    fn test_bessel_y_non_positive_domain() {
        assert!(matches!(bessel_y0(0.0), Err(e) if e.kind == ErrorKind::Domain));
        assert!(matches!(bessel_y1(-1.0), Err(e) if e.kind == ErrorKind::Domain));
        assert!(matches!(bessel_y(2, 0.0), Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_bessel_order_limit_enforced_in_math_layer() {
        assert!(matches!(bessel_j(101, 1.0), Err(e) if e.kind == ErrorKind::Domain));
        assert!(matches!(bessel_y(101, 1.0), Err(e) if e.kind == ErrorKind::Domain));
        assert!(matches!(bessel_i(101, 1.0), Err(e) if e.kind == ErrorKind::Domain));
        assert!(matches!(bessel_k(101, 1.0), Err(e) if e.kind == ErrorKind::Domain));
    }

    // ===== 修正 Bessel I/K 族 =====

    #[test]
    fn test_bessel_i0_i1_at_one() {
        assert!(approx_rel(
            bessel_i0(1.0).unwrap(),
            1.2660658777520084,
            1e-7
        ));
        assert!(approx_rel(bessel_i1(1.0).unwrap(), 0.565159103992485, 1e-7));
        assert!(approx_rel(
            bessel_i(2, 1.0).unwrap(),
            0.13574766976703828,
            1e-6
        ));
    }

    #[test]
    fn test_bessel_i_odd_symmetry() {
        let pos = bessel_i(1, 2.0).unwrap();
        let neg = bessel_i(1, -2.0).unwrap();
        assert!(approx_rel(neg, -pos, 1e-7));
    }

    #[test]
    fn test_bessel_k0_k1_at_one() {
        assert!(approx_rel(
            bessel_k0(1.0).unwrap(),
            0.42102443824070834,
            1e-7
        ));
        assert!(approx_rel(
            bessel_k1(1.0).unwrap(),
            0.6019072301972346,
            1e-7
        ));
    }

    #[test]
    fn test_bessel_k_non_positive_domain() {
        assert!(matches!(bessel_k0(0.0), Err(e) if e.kind == ErrorKind::Domain));
        assert!(matches!(bessel_k1(-1.0), Err(e) if e.kind == ErrorKind::Domain));
        assert!(matches!(bessel_k(2, 0.0), Err(e) if e.kind == ErrorKind::Domain));
    }

    // ===== Beta / Zeta =====

    #[test]
    fn test_beta_values() {
        assert!(approx_rel(beta(2.0, 3.0).unwrap(), 1.0 / 12.0, 1e-12));
        assert!(approx_rel(
            beta(0.5, 0.5).unwrap(),
            std::f64::consts::PI,
            1e-12
        ));
    }

    #[test]
    fn test_beta_domain() {
        assert!(matches!(beta(0.0, 1.0), Err(e) if e.kind == ErrorKind::Domain));
        assert!(matches!(beta(1.0, -1.0), Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_zeta_positive_values() {
        assert!(approx_rel(zeta(2.0).unwrap(), 1.6449340668482264, 1e-12));
        assert!(approx_rel(zeta(4.0).unwrap(), 1.0823232337111381, 1e-12));
        assert!(approx_rel(zeta(0.5).unwrap(), -1.4603545088095866, 1e-12));
    }

    #[test]
    fn test_zeta_non_positive_values() {
        assert!((zeta(0.0).unwrap() + 0.5).abs() < 1e-15);
        assert!(approx_rel(zeta(-1.0).unwrap(), -1.0 / 12.0, 1e-12));
    }

    #[test]
    fn test_zeta_pole() {
        assert!(matches!(zeta(1.0), Err(e) if e.kind == ErrorKind::Domain));
    }

    // ===== Airy =====

    #[test]
    fn test_airy_at_zero_and_one() {
        assert!(approx_rel(airy_ai(0.0).unwrap(), 0.3550280538878172, 1e-12));
        assert!(approx_rel(airy_bi(0.0).unwrap(), 0.6149266274460007, 1e-12));
        assert!(approx_rel(airy_ai(1.0).unwrap(), 0.1352924163128814, 1e-12));
    }

    #[test]
    fn test_airy_asymptotic_positive() {
        assert!(approx_rel(
            airy_ai(10.0).unwrap(),
            1.1047532552898654e-10,
            2e-6
        ));
        assert!(approx_rel(airy_bi(10.0).unwrap(), 455641153.54822654, 2e-6));
    }

    #[test]
    fn test_airy_asymptotic_negative() {
        assert!(approx_rel(
            airy_ai(-10.0).unwrap(),
            0.040241238486441955,
            2e-6
        ));
    }

    #[test]
    fn test_airy_rejects_nan() {
        assert!(matches!(airy_ai(f64::NAN), Err(e) if e.kind == ErrorKind::NaNOrInf));
        assert!(matches!(airy_bi(f64::INFINITY), Err(e) if e.kind == ErrorKind::NaNOrInf));
    }

    // ===== 三角函数 =====

    #[test]
    fn test_sin_zero() {
        assert!(approx(sin(0.0).unwrap(), 0.0));
    }

    #[test]
    fn test_cos_zero() {
        assert!(approx(cos(0.0).unwrap(), 1.0));
    }

    #[test]
    fn test_sin_pi_over_two() {
        assert!(approx(sin(std::f64::consts::FRAC_PI_2).unwrap(), 1.0));
    }

    #[test]
    fn test_tan_pi_over_four() {
        assert!(approx(tan(std::f64::consts::FRAC_PI_4).unwrap(), 1.0));
    }

    // ===== 反三角函数 =====

    #[test]
    fn test_asin_one() {
        assert!(approx(asin(1.0).unwrap(), std::f64::consts::FRAC_PI_2));
    }

    #[test]
    fn test_asin_out_of_range() {
        let r = asin(2.0);
        assert!(matches!(&r, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_acos_zero() {
        assert!(approx(acos(0.0).unwrap(), std::f64::consts::FRAC_PI_2));
    }

    #[test]
    fn test_acos_out_of_range() {
        let r = acos(-1.5);
        assert!(matches!(&r, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_atan_one() {
        assert!(approx(atan(1.0).unwrap(), std::f64::consts::FRAC_PI_4));
    }

    // ===== 对数函数 =====

    #[test]
    fn test_ln_e() {
        assert!(approx(ln(std::f64::consts::E).unwrap(), 1.0));
    }

    #[test]
    fn test_ln_one() {
        assert!(approx(ln(1.0).unwrap(), 0.0));
    }

    #[test]
    fn test_ln_negative() {
        let r = ln(-1.0);
        assert!(matches!(&r, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_ln_zero() {
        let r = ln(0.0);
        assert!(matches!(&r, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_log10_100() {
        assert!(approx(log10(100.0).unwrap(), 2.0));
    }

    #[test]
    fn test_log2_8() {
        assert!(approx(log2(8.0).unwrap(), 3.0));
    }

    #[test]
    fn test_log_arbitrary_base() {
        assert!(approx(log(100.0, 10.0).unwrap(), 2.0));
    }

    #[test]
    fn test_log_bad_base() {
        let r = log(100.0, 1.0);
        assert!(matches!(&r, Err(e) if e.kind == ErrorKind::Domain));
    }

    // ===== 指数函数 =====

    #[test]
    fn test_exp_zero() {
        assert!(approx(exp(0.0).unwrap(), 1.0));
    }

    #[test]
    fn test_exp_one() {
        assert!(approx(exp(1.0).unwrap(), std::f64::consts::E));
    }

    #[test]
    fn test_exp_overflow() {
        let r = exp(1000.0);
        assert!(matches!(&r, Err(e) if e.kind == ErrorKind::NaNOrInf));
    }

    // ===== 双曲函数 =====

    #[test]
    fn test_sinh_zero() {
        assert!(approx(sinh(0.0).unwrap(), 0.0));
    }

    #[test]
    fn test_cosh_zero() {
        assert!(approx(cosh(0.0).unwrap(), 1.0));
    }

    #[test]
    fn test_tanh_zero() {
        assert!(approx(tanh(0.0).unwrap(), 0.0));
    }

    // ===== 特殊函数 =====

    #[test]
    fn test_gamma_five() {
        assert!(approx(gamma(5.0).unwrap(), 24.0));
    }

    #[test]
    fn test_gamma_one() {
        assert!(approx(gamma(1.0).unwrap(), 1.0));
    }

    #[test]
    fn test_gamma_reflection() {
        // gamma(-0.5) = -2*sqrt(pi)
        assert!(approx(
            gamma(-0.5).unwrap(),
            -2.0 * std::f64::consts::PI.sqrt()
        ));
    }

    #[test]
    fn test_erf_zero() {
        assert!(approx(erf(0.0).unwrap(), 0.0));
    }

    #[test]
    fn test_erf_large() {
        assert!(approx(erf(100.0).unwrap(), 1.0));
    }
}
