// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! S2 Edge Cases —— 数值边界、空集合、零值、极小/极大精度。
//!
//! 行为经探针实测钉住（探针结论即本模块断言的契约）：
//! - 空集合经 `evaluate` 管线一律 Domain 错误；门面 `stats()` 直接透传
//!   math 层（`mean([])` = NaN、`sum([])` = -0.0、`count([])` = 0.0）
//! - `perm/comb` k>n 在组合数学域函数层面返回 0（经门面验证），表达式
//!   `perm(3,5)` 因域 supports 校验拒绝而回落路由错误（Domain）
//! - `date_diff(a, a)` 曾在 jiff 内部 panic，修复后任意单位返回 0

use calnexus::math::precision::format_bigrational;
use calnexus::{CalNexus, EvalResult};

use crate::common::{approx_eq, assert_scalar, eval_err, eval_ok};

fn scalar_of(r: &EvalResult) -> f64 {
    match r {
        EvalResult::Scalar(v) => *v,
        other => panic!("expected Scalar, got {other:?}"),
    }
}

fn kind_of(expr: &str) -> calnexus::ErrorKind {
    eval_err(expr).kind
}

// ---------------------------------------------------------------------------
// 数值边界
// ---------------------------------------------------------------------------

#[test]
fn edge_factorial_limits() {
    // 上限内合法（10000 产生 ~35660 位数字）
    assert!(matches!(eval_ok("factorial(100)"), EvalResult::Scalar(_)));
    // 超限 → Overflow（MAX_FACTORIAL_INPUT = 10_000）
    assert_eq!(kind_of("factorial(10001)"), calnexus::ErrorKind::Overflow);
    // 0! = 1
    assert_scalar(&eval_ok("factorial(0)"), 1.0);
}

#[test]
fn edge_pow_boundaries() {
    assert_scalar(&eval_ok("0^0"), 1.0);
    assert_eq!(kind_of("0^(-1)"), calnexus::ErrorKind::NaNOrInf);
    assert!(approx_eq(scalar_of(&eval_ok("2^0.5")), 2.0_f64.sqrt()));
    // f64 上界溢出 → NaNOrInf
    assert_eq!(kind_of("1e308 * 10"), calnexus::ErrorKind::NaNOrInf);
    // 次正规区下溢为 0 不视为错误
    assert_scalar(&eval_ok("1e-300 / 1e300"), 0.0);
}

#[test]
fn edge_number_theory_boundaries() {
    // gcd(0, 0) = 0；gcd(0, n) = n（表达式层标量通道）
    assert_scalar(&eval_ok("gcd(0, 0)"), 0.0);
    assert_scalar(&eval_ok("gcd(0, 18)"), 18.0);
    // 0 与 1 非素数；2 是最小素数
    assert_scalar(&eval_ok("is_prime(0)"), 0.0);
    assert_scalar(&eval_ok("is_prime(1)"), 0.0);
    assert_scalar(&eval_ok("is_prime(2)"), 1.0);
    // 非互质模逆元 → Domain
    let err = eval_err("mod_inverse(2, 4)");
    assert_eq!(err.kind, calnexus::ErrorKind::Domain);
    assert!(err.message.contains("not coprime"), "got {err}");
    // mod_pow 模数为 0 → DivisionByZero
    assert_eq!(
        kind_of("mod_pow(2, 10, 0)"),
        calnexus::ErrorKind::DivisionByZero
    );
    // 门面 BigInt 通道：离散对数 3^x ≡ 6 (mod 17) → x = 15
    let cn = CalNexus::new();
    match cn
        .scalar()
        .discrete_log(
            &calnexus::BigNumber::from_i64(3),
            &calnexus::BigNumber::from_i64(6),
            &calnexus::BigNumber::from_i64(17),
        )
        .unwrap()
    {
        EvalResult::BigInt(v) => assert_eq!(v.to_string(), "15"),
        other => panic!("discrete_log → BigInt expected, got {other:?}"),
    }
}

#[test]
fn edge_combinatorics_boundaries_via_facade() {
    let cn = CalNexus::new();
    let s = cn.scalar();
    fn bigint_of(r: EvalResult) -> String {
        match r {
            EvalResult::BigInt(v) => v.to_string(),
            other => panic!("BigInt expected, got {other:?}"),
        }
    }
    // k > n → 0；k == n / k == 0 → 1
    assert_eq!(bigint_of(s.perm(3, 5).unwrap()), "0");
    assert_eq!(bigint_of(s.comb(5, 0).unwrap()), "1");
    assert_eq!(bigint_of(s.comb(5, 5).unwrap()), "1");
    assert_eq!(bigint_of(s.catalan(0).unwrap()), "1");
    // 大 n 不溢出（BigInt 精确通道）：C(60,30) = 118264581564861424
    assert_eq!(bigint_of(s.comb(60, 30).unwrap()), "118264581564861424");
}

#[test]
fn edge_scientific_domain_boundaries() {
    // 反三角定义域外 → Domain（含 hint）
    let err = eval_err("asin(2)");
    assert_eq!(err.kind, calnexus::ErrorKind::Domain);
    assert_eq!(err.hint.as_deref(), Some("asin domain is [-1, 1]"));
    // 对数零/负值 → Domain
    assert_eq!(kind_of("ln(0)"), calnexus::ErrorKind::Domain);
    assert_eq!(kind_of("log(0, 10)"), calnexus::ErrorKind::Domain);
    // Γ 函数非正整数极点 → Domain
    let err = eval_err("gamma(0)");
    assert_eq!(err.kind, calnexus::ErrorKind::Domain);
    assert!(err.message.contains("pole"), "got {err}");
}

#[test]
fn edge_sqrt_not_registered() {
    // sqrt 未注册为域函数（复数运算经 `i` 字面量与 complex_* 通道）——
    // 钉住「未知函数回落 Domain 路由错误」契约
    let err = eval_err("sqrt(-1)");
    assert_eq!(err.kind, calnexus::ErrorKind::Domain);
    assert!(err.message.contains("sqrt"), "got {err}");
}

// ---------------------------------------------------------------------------
// 空集合与单元素
// ---------------------------------------------------------------------------

#[test]
fn edge_empty_collections_rejected_by_pipeline() {
    for expr in [
        "mean([])",
        "variance([])",
        "std([])",
        "median([])",
        "min([])",
        "max([])",
        "sum([])",
        "count([])",
    ] {
        let err = eval_err(expr);
        assert_eq!(err.kind, calnexus::ErrorKind::Domain, "{expr}");
        assert!(
            err.message.contains("non-empty"),
            "{expr} should mention non-empty, got {err}"
        );
    }
}

#[test]
fn edge_empty_collections_via_facade_passthrough() {
    // 门面直通 math 层：NaN/±0.0 而非错误（与管线行为刻意不同，两处均钉住）
    let cn = CalNexus::new();
    let st = cn.stats();
    match st.mean(&[]).unwrap() {
        EvalResult::Scalar(v) => assert!(v.is_nan(), "facade mean([]) = NaN, got {v}"),
        other => panic!("Scalar expected, got {other:?}"),
    }
    assert_eq!(scalar_of(&st.count(&[]).unwrap()), 0.0);
    match st.sum(&[]).unwrap() {
        EvalResult::Scalar(v) => assert_eq!(v, 0.0), // -0.0 == 0.0
        other => panic!("Scalar expected, got {other:?}"),
    }
}

#[test]
fn edge_single_element_stats() {
    // 单元素集合：方差/标准差为 0，均值为元素本身
    assert_scalar(&eval_ok("variance([5])"), 0.0);
    assert_scalar(&eval_ok("std([5])"), 0.0);
    assert_scalar(&eval_ok("mean([5])"), 5.0);
    assert_scalar(&eval_ok("median([42])"), 42.0);
}

#[test]
fn edge_even_length_median() {
    // 偶数长度中位数 = 中间两数均值
    assert_scalar(&eval_ok("median([1,2,3,4])"), 2.5);
}

// ---------------------------------------------------------------------------
// 零值
// ---------------------------------------------------------------------------

#[test]
fn edge_zero_division_and_mod() {
    assert_eq!(kind_of("1/0"), calnexus::ErrorKind::DivisionByZero);
    assert_eq!(kind_of("5 % 0"), calnexus::ErrorKind::DivisionByZero);
}

#[test]
fn edge_zero_vector() {
    let err = eval_err("normalize([0,0])");
    assert_eq!(err.kind, calnexus::ErrorKind::Domain);
    assert!(err.message.contains("zero vector"), "got {err}");
    // 零向量模长为 0
    let cn = CalNexus::new();
    assert!(approx_eq(
        scalar_of(
            &cn.linalg()
                .magnitude(&calnexus::Vector::new(&[0.0, 0.0]))
                .unwrap()
        ),
        0.0
    ));
}

#[test]
fn edge_zero_matrix() {
    assert_scalar(&eval_ok("det([[0]])"), 0.0);
    let err = eval_err("identity(0)");
    assert_eq!(err.kind, calnexus::ErrorKind::Domain);
    assert!(err.message.contains("positive"), "got {err}");
    // 奇异矩阵求逆 → Domain
    let err = eval_err("inverse([[1,2],[2,4]])");
    assert_eq!(err.kind, calnexus::ErrorKind::Domain);
}

#[cfg(feature = "unit")]
#[test]
fn edge_zero_unit_conversion() {
    assert_scalar(&eval_ok("convert(0, \"km\", \"m\")"), 0.0);
    // 绝对零度：0K = -273.15°C（单位域做纯公式换算）
    assert!(approx_eq(
        scalar_of(&eval_ok("convert(0, \"K\", \"C\")")),
        -273.15
    ));
    // 低于绝对零度的输入不校验物理可行性（纯公式语义钉住）
    assert!(approx_eq(
        scalar_of(&eval_ok("convert(-300, \"C\", \"K\")")),
        -26.85
    ));
}

// ---------------------------------------------------------------------------
// 精度极值
// ---------------------------------------------------------------------------

#[test]
fn edge_precision_limits() {
    // MAX_PRECISION = 10_000：上界可用，超界拒绝
    let r = eval_ok("precision(10000, 1/3)");
    assert!(matches!(r, EvalResult::BigRational(_)));
    let err = eval_err("precision(10001, 1/3)");
    assert_eq!(err.kind, calnexus::ErrorKind::Domain);
    assert!(err.message.contains("10000"), "got {err}");
}

#[test]
fn edge_precision_formatting() {
    let cn = CalNexus::new();
    let q = match cn.scalar().precision_eval(50, "1/3").unwrap() {
        EvalResult::BigRational(q) => q,
        other => panic!("BigRational expected, got {other:?}"),
    };
    // 门面 precision_eval 语义：先按 f64 求值再转 BigRational（有理数是
    // f64 值的精确表示，非符号级 1/3——后者由管线 precision(N, expr)
    // 提供，见 happy_path::pipeline_variant_bigint_and_bigrational）
    assert_eq!(format_bigrational(&q, Some(5)), "0.33333");
    assert_eq!(format_bigrational(&q, None), q.to_string()); // 整数位精确还原
    let half = match cn.scalar().precision_eval(5, "1/2").unwrap() {
        EvalResult::BigRational(q) => q,
        other => panic!("BigRational expected, got {other:?}"),
    };
    // 0.5 可被 f64 精确表示 → 门面路径同样得到符号级 1/2
    assert_eq!(format_bigrational(&half, Some(1)), "0.5");
    assert_eq!(format_bigrational(&half, None), "1/2");
    let half = match cn.scalar().precision_eval(5, "1/2").unwrap() {
        EvalResult::BigRational(q) => q,
        other => panic!("BigRational expected, got {other:?}"),
    };
    assert_eq!(format_bigrational(&half, Some(1)), "0.5");
}

#[test]
fn edge_bigint_precision_preserved() {
    // 20 位整数字面量绕过 f64 精度损失
    let r = eval_ok("123456789012345678901234567890");
    match r {
        EvalResult::BigInt(v) => assert_eq!(v.to_string(), "123456789012345678901234567890"),
        other => panic!("BigInt expected, got {other:?}"),
    }
    // 大整数参与运算仍保持 BigInt 通道（≥16 位整数字面量 → BigNumber AST）
    match eval_ok("123456789012345678 + 0") {
        EvalResult::BigInt(v) => assert_eq!(v.to_string(), "123456789012345678"),
        other => panic!("BigInt expected, got {other:?}"),
    }
}

#[test]
fn edge_f64_representation_quirks() {
    // f64 语义钉住：0.1+0.2 落在 0.3 附近但位模式不等（未做十进制补偿）
    let v = scalar_of(&eval_ok("0.1 + 0.2"));
    assert!((v - 0.3).abs() < 1e-15 && v != 0.3);
}

// ---------------------------------------------------------------------------
// 日期边界
// ---------------------------------------------------------------------------

#[cfg(feature = "time")]
#[test]
fn edge_date_boundaries() {
    // 非法日期（平年 2 月 29）→ Domain；闰年合法
    let err = eval_err("date(\"2026-02-29\")");
    assert_eq!(err.kind, calnexus::ErrorKind::Domain);
    assert!(err.message.contains("invalid date"), "got {err}");
    assert!(matches!(
        eval_ok("date(\"2024-02-29\")"),
        EvalResult::DateTime(_)
    ));
    // 歧义数字格式显式拒绝并给出 parse_date 指引
    let err = eval_err("date(\"01/02/2026\")");
    assert_eq!(err.kind, calnexus::ErrorKind::Domain);
    assert!(err.message.contains("ambiguous"), "got {err}");
    // 非闰世纪 1900 非闰年；整除 400 的 2000 是闰年
    assert_scalar(&eval_ok("is_leap_year(1900)"), 0.0);
    // Unix 纪元往返
    match eval_ok("from_timestamp(0, \"UTC\")") {
        EvalResult::DateTime(s) => assert!(s.starts_with("1970-01-01T00:00:00"), "got {s}"),
        other => panic!("DateTime expected, got {other:?}"),
    }
    assert_scalar(&eval_ok("timestamp(\"1969-12-31T00:00:00Z\")"), -86_400.0);
    // 负偏移加法：2026-01-01 - 1 day = 2025-12-31
    match eval_ok("date_add(\"2026-01-01\", -1, \"day\")") {
        EvalResult::DateTime(s) => assert!(s.starts_with("2025-12-31"), "got {s}"),
        other => panic!("DateTime expected, got {other:?}"),
    }
}

#[cfg(feature = "time")]
#[test]
fn edge_date_diff_zero_span_regression() {
    // 回归：零跨度 × 日历单位曾在 jiff 内部断言 panic（修复见
    // src/math/time.rs span_total_in_unit），date_diff(a, a) 必须为 0
    for unit in ["day", "week", "month", "year", "hour", "min", "s"] {
        let expr = format!("date_diff(\"2026-01-01\", \"2026-01-01\", \"{unit}\")");
        assert_scalar(&eval_ok(&expr), 0.0);
    }
}

#[cfg(feature = "time")]
#[test]
fn edge_date_diff_signed_and_month_clamp() {
    // 负方向带符号
    assert_scalar(
        &eval_ok("date_diff(\"2026-01-01\", \"2025-12-31\", \"day\")"),
        -1.0,
    );
    // 月末钳制：2026-01-31 + 1 month = 2026-02-28（非闰年）
    match eval_ok("date_add(\"2026-01-31\", 1, \"month\")") {
        EvalResult::DateTime(s) => assert!(s.starts_with("2026-02-28"), "got {s}"),
        other => panic!("DateTime expected, got {other:?}"),
    }
    // 闰年 2 月钳制到 29
    match eval_ok("date_add(\"2024-01-31\", 1, \"month\")") {
        EvalResult::DateTime(s) => assert!(s.starts_with("2024-02-29"), "got {s}"),
        other => panic!("DateTime expected, got {other:?}"),
    }
}

#[cfg(feature = "time")]
#[test]
fn edge_unknown_timezone_and_chinese_date() {
    // 未知时区 → Domain（datetime/now 的 tz 实参校验）
    let err = eval_err("datetime(\"2026-01-01 10:00:00\", \"Bad/Zone\")");
    assert_eq!(err.kind, calnexus::ErrorKind::Domain);
    assert!(err.message.contains("unknown timezone"), "got {err}");
    let err = eval_err("date_add(\"2026-01-01\", 1, \"badunit\")");
    assert_eq!(err.kind, calnexus::ErrorKind::Domain);
    assert!(err.message.contains("invalid time unit"), "got {err}");
    let err = eval_err("date_diff(\"bad-date\", \"2026-01-01\", \"day\")");
    assert!(err.message.contains("invalid date"), "got {err}");
    // 中文日期格式支持
    match eval_ok("date(\"2026年7月25日\")") {
        EvalResult::DateTime(s) => assert!(s.starts_with("2026-07-25"), "got {s}"),
        other => panic!("DateTime expected, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// 单位边界
// ---------------------------------------------------------------------------

#[cfg(feature = "unit")]
#[test]
fn edge_unit_case_sensitivity() {
    // 单位名大小写敏感：十进制 KB ≠ 二进制 KiB；小写 kb 未定义
    assert_scalar(&eval_ok("convert(1, \"KB\", \"B\")"), 1000.0);
    assert_scalar(&eval_ok("convert(1, \"KiB\", \"B\")"), 1024.0);
    let err = eval_err("convert(1, \"kb\", \"B\")");
    assert_eq!(err.kind, calnexus::ErrorKind::Domain);
    assert!(err.message.contains("unknown unit"), "got {err}");
}

#[cfg(feature = "unit")]
#[test]
fn edge_unit_dimension_mismatch() {
    let err = eval_err("convert(1, \"kg\", \"m\")");
    assert_eq!(err.kind, calnexus::ErrorKind::Domain);
    assert!(
        err.message.contains("Mass") && err.message.contains("Length"),
        "dimension names in message, got {err}"
    );
}

#[cfg(feature = "unit")]
#[test]
fn edge_unit_average_calendar_units() {
    // 时间单位用平均历法值：d=86400s、wk=604800s、yr=31_557_600s
    assert_scalar(&eval_ok("convert(1, \"yr\", \"s\")"), 31_557_600.0);
    assert_scalar(&eval_ok("convert(1, \"wk\", \"s\")"), 604_800.0);
    assert_scalar(&eval_ok("convert(1, \"d\", \"h\")"), 24.0);
}
