// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! S5 Feature Combinations —— 可选特性启用/禁用两侧的行为差异矩阵。
//!
//! e2e 目标无 required-features：本模块在一切组合下编译，`#[cfg]` 门控
//! 只让当前组合下有意义的断言参与编译。`--all-features` 跑启用侧，
//! default（及单 feature 组合）跑禁用侧。

use calnexus::{I18n, Lang};

use crate::common::eval_ok;

// ---------------------------------------------------------------------------
// time
// ---------------------------------------------------------------------------

#[cfg(feature = "time")]
#[test]
fn gate_time_enabled_routes_to_time_domain() {
    let (r, domain) = crate::common::eval_ok_with_domain("date(\"2026-01-01\")");
    assert_eq!(domain, "time");
    assert!(matches!(r, calnexus::EvalResult::DateTime(_)));
}

#[cfg(not(feature = "time"))]
#[test]
fn gate_time_disabled_falls_back_to_routing_error() {
    use crate::common::eval_err;
    let err = eval_err("date(\"2026-01-01\")");
    assert_eq!(err.kind, calnexus::ErrorKind::Domain);
    assert!(err.message.contains("date"), "got {err}");
}

// ---------------------------------------------------------------------------
// unit
// ---------------------------------------------------------------------------

#[cfg(feature = "unit")]
#[test]
fn gate_unit_enabled_routes_to_unit_domain() {
    let (_, domain) = crate::common::eval_ok_with_domain("convert(1, \"km\", \"m\")");
    assert_eq!(domain, "unit");
}

#[cfg(not(feature = "unit"))]
#[test]
fn gate_unit_disabled_falls_back_to_routing_error() {
    use crate::common::eval_err;
    let err = eval_err("convert(1, \"km\", \"m\")");
    assert_eq!(err.kind, calnexus::ErrorKind::Domain);
    assert!(err.message.contains("convert"), "got {err}");
}

// ---------------------------------------------------------------------------
// fx
// ---------------------------------------------------------------------------

#[cfg(feature = "fx")]
#[test]
fn gate_fx_enabled_default_router_includes_fx() {
    // 默认路由器（evaluate）在工厂注册 FxDomain::default()：
    // 有网/有缓存 → 成功且归属 fx 域；断网且无缓存 → DependencyUnavailable。
    // 两侧均接受（CI 断网确定性；缓存文件存在与否不改变绿/红）。
    match crate::common::eval("fx_rate(\"USD\", \"EUR\")") {
        Ok((r, domain, _, _)) => {
            assert_eq!(domain, "fx");
            assert!(matches!(r, calnexus::EvalResult::Scalar(v) if v > 0.0));
        }
        Err(e) => assert!(
            matches!(
                e.kind,
                calnexus::ErrorKind::DependencyUnavailable | calnexus::ErrorKind::Domain
            ),
            "tolerated: DependencyUnavailable/Domain, got {e:?}"
        ),
    }
}

#[cfg(not(feature = "fx"))]
#[test]
fn gate_fx_disabled_falls_back_to_routing_error() {
    use crate::common::eval_err;
    let err = eval_err("fx_rate(\"USD\", \"EUR\")");
    assert_eq!(err.kind, calnexus::ErrorKind::Domain);
    assert!(err.message.contains("fx_rate"), "got {err}");
}

// ---------------------------------------------------------------------------
// numerical（经 LinearAlgebra trait 与表达式双通道）
// ---------------------------------------------------------------------------

#[cfg(feature = "numerical")]
#[test]
fn gate_numerical_enabled_trait_has_decompositions() {
    use calnexus::{CalNexus, Matrix};
    let cn = CalNexus::new();
    let m = Matrix::from_rows(&[&[2.0, 1.0], &[1.0, 2.0]]);
    // trait 方法存在且返回 Json（编译期即验证 trait 门控展开）
    assert!(cn.linalg().lu(&m).is_ok());
    assert!(cn.linalg().eig(&m).is_ok());
}

#[cfg(not(feature = "numerical"))]
#[test]
fn gate_numerical_disabled_expression_unrouted() {
    use crate::common::eval_err;
    // lu/eig 由 MatrixDomain 在 numerical 下委托；禁用时 MatrixDomain 仍
    // 接单但拒绝执行（unsupported function，Domain kind）
    for expr in ["lu([[1,2],[3,4]])", "eig([[1,0],[0,1]])"] {
        let err = eval_err(expr);
        assert_eq!(err.kind, calnexus::ErrorKind::Domain, "{expr}");
        assert!(err.message.contains("lu") || err.message.contains("eig"), "{expr}: {err}");
    }
}

// ---------------------------------------------------------------------------
// icu —— BCP-47 解析路径在两种实现下行为对齐 + 已知分歧点钉住
// ---------------------------------------------------------------------------

#[test]
fn gate_icu_common_tags_agree() {
    // 两种实现共同契约：主子标签识别 zh，未知回落 En
    assert_eq!(I18n::from_str("zh").lang(), Lang::Zh);
    assert_eq!(I18n::from_str("zh-CN").lang(), Lang::Zh);
    assert_eq!(I18n::from_str("zh-Hans-CN").lang(), Lang::Zh);
    assert_eq!(I18n::from_str("zh-TW").lang(), Lang::Zh);
    assert_eq!(I18n::from_str("zhongwen").lang(), Lang::En);
    assert_eq!(I18n::from_str("en").lang(), Lang::En);
    assert_eq!(I18n::from_str("fr-FR").lang(), Lang::En);
}

#[cfg(feature = "icu")]
#[test]
fn gate_icu_strict_bcp47_rejects_trailing_separator() {
    // icu::locale 严格校验：尾随分隔符非法 → 回落 En
    assert_eq!(I18n::from_str("zh-").lang(), Lang::En);
}

#[cfg(not(feature = "icu"))]
#[test]
fn gate_icu_simple_split_tolerates_trailing_separator() {
    // 简单实现按 '-' 切主子标签："zh-" → "zh" → Zh
    assert_eq!(I18n::from_str("zh-").lang(), Lang::Zh);
}

// ---------------------------------------------------------------------------
// server 聚合特性 = http + mcp
// ---------------------------------------------------------------------------

#[cfg(feature = "server")]
#[test]
fn gate_server_implies_http_and_mcp() {
    // server 展开 http/mcp 后，双 server 构造器均可用（编译期 + 运行期）
    let http = calnexus::HttpServer::new();
    assert_eq!(http.addr(), "127.0.0.1:3000");
    let mcp = calnexus::build_mcp_server();
    assert!(mcp.tool_count() >= 2, "at least evaluate + list_functions");
}

#[cfg(all(feature = "http", not(feature = "mcp")))]
#[test]
fn gate_http_without_mcp_builds_router() {
    // http 单独启用：router 可构建，MCP 侧不可见（编译期排除）
    let _router = calnexus::build_router();
}

// ---------------------------------------------------------------------------
// format_bigrational 双路径可见性
// ---------------------------------------------------------------------------

#[test]
fn gate_format_bigrational_reachable() {
    use calnexus::math::precision::format_bigrational as fmt;
    let cn = calnexus::CalNexus::new();
    let q = match cn.scalar().precision_eval(5, "1/2").unwrap() {
        calnexus::EvalResult::BigRational(q) => q,
        other => panic!("BigRational expected, got {other:?}"),
    };
    assert_eq!(fmt(&q, None), "1/2");
    // crate 根再导出仅在 cli/http/mcp 下可见——当前组合可见时验证一致性
    #[cfg(any(feature = "cli", feature = "http", feature = "mcp"))]
    {
        assert_eq!(calnexus::format_bigrational(&q, Some(2)), "0.50");
    }
}

// ---------------------------------------------------------------------------
// 门面 trait 面完整存在性（编译期断言经真实调用消费）
// ---------------------------------------------------------------------------

#[test]
fn gate_facade_accessors_all_constructible() {
    use calnexus::CalNexus;
    let cn = CalNexus::default();
    // 五访问器在当前组合下均可产出并消费（trait 分发通道）
    assert!(cn.scalar().add(1.0, 1.0).is_ok());
    assert!(cn.linalg().dot(
        &calnexus::Vector::new(&[1.0]),
        &calnexus::Vector::new(&[1.0])
    ).is_ok());
    assert!(cn.stats().sum(&[1.0]).is_ok());
    assert!(cn.symbolic().simplify("x").is_ok());
    // applied 面至少有一种方法随组合可用：unit convert 或 time today；
    // 两者皆无时仅验证访问器可产出
    let ap = cn.applied();
    #[cfg(feature = "unit")]
    assert!(ap.convert(1.0, "m", "cm").is_ok());
    #[cfg(not(feature = "unit"))]
    {
        let _ = &ap; // 访问器可产出即为本组合下的契约
    }
    // Default 与 new 等价
    let _eq: CalNexus = Default::default();
}

// ---------------------------------------------------------------------------
// 求值结果域归属随门控变化的正交性
// ---------------------------------------------------------------------------

#[test]
fn gate_core_domains_always_available() {
    // 11 核心域不受任何可选 feature 影响
    let cases = [
        ("2+3", "arithmetic"),
        ("sin(0)", "scientific"),
        ("mean([1,2])", "statistics"),
        ("gcd(12,8)", "number_theory"),
        // 组合数学域表达式名为数学记号 P/C（perm/comb 仅是门面方法名）
        ("C(4,2)", "combinatorics"),
        ("P(4,2)", "combinatorics"),
        ("diff(x^2,x)", "symbolic"),
        ("det([[1]])", "matrix"),
        ("dot([1],[1])", "vector"),
        ("12345678901234567890", "precision"),
        ("roots(x^2+1)", "polynomial"),
        ("abs(3+4i)", "complex"),
    ];
    for (expr, domain) in cases {
        let (_, d) = crate::common::eval_ok_with_domain(expr);
        assert_eq!(d, domain, "{expr}");
    }
}

#[test]
fn gate_eval_ok_smoke_for_gated_combos() {
    // 组合矩阵下的最小健全性：核心管线始终可用
    assert!(matches!(eval_ok("6*7"), calnexus::EvalResult::Scalar(42.0)));
}
