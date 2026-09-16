// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! S4 fx 域离线全链路 —— mock 域 + `math::fx` + `math::fx_scenario`。
//!
//! 真实 `FxDomain::new` 为 `pub(crate)`，无法注入 mock provider；本模块
//! 经公开 `CalculationDomain`/`RateProvider`/`math::fx` 构建同构 mock，
//! 驱动 `evaluate_with_router` 完整管线，全程零出网。真实 FxDomain 的
//! 网络容忍冒烟见 `error_paths::error_fx_real_domain_offline_tolerant`。

#![cfg(feature = "fx")]

use calnexus::domains::RateProvider;
use calnexus::math::fx::{self, RateTable};
use calnexus::math::fx_scenario::{budget_calculation, pricing_calculation};
use calnexus::{ErrorKind, EvalResult};

use crate::common::fx_mock::{MockFxDomain, StaticProvider, mock_rate_table};
use crate::common::{approx_eq, assert_scalar, eval_via_router, eval_via_router_ok};

fn scalar_of(r: &EvalResult) -> f64 {
    match r {
        EvalResult::Scalar(v) => *v,
        other => panic!("expected Scalar, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// MockFxDomain × evaluate_with_router 全链路
// ---------------------------------------------------------------------------

#[test]
fn fx_pipeline_expression_conversion() {
    // fx(110, "USD", "CNY") = 110 / 1.1 * 7.8 = 780
    let (r, domain) = eval_via_router_ok("fx(110, \"USD\", \"CNY\")", &MockFxDomain::router());
    assert_scalar(&r, 780.0);
    assert_eq!(domain, "fx");
}

#[test]
fn fx_pipeline_rate_expression() {
    // fx_rate 等价 fx(1,…)：USD→EUR = 1 / 1.1 = 0.9090…
    let (r, _) = eval_via_router_ok("fx_rate(\"USD\", \"EUR\")", &MockFxDomain::router());
    assert!(approx_eq(scalar_of(&r), 1.0 / 1.1));
    // 同币种汇率恒为 1
    let (r, _) = eval_via_router_ok("fx_rate(\"USD\", \"USD\")", &MockFxDomain::router());
    assert_scalar(&r, 1.0);
}

#[test]
fn fx_pipeline_arithmetic_wrapping() {
    // 算术包装：fx 结果参与二元/一元运算仍路由至 fx 域
    let (r, domain) =
        eval_via_router_ok("fx(100, \"USD\", \"EUR\") * 2 + 1", &MockFxDomain::router());
    assert!(approx_eq(scalar_of(&r), (100.0 / 1.1) * 2.0 + 1.0));
    assert_eq!(domain, "fx");
    let (r, _) = eval_via_router_ok("-fx_rate(\"EUR\", \"JPY\")", &MockFxDomain::router());
    assert!(approx_eq(scalar_of(&r), -160.0));
    // abs 函数包装（fx 白名单包装函数）
    let (r, _) = eval_via_router_ok("abs(fx_rate(\"EUR\", \"USD\"))", &MockFxDomain::router());
    assert!(approx_eq(scalar_of(&r), 1.1));
}

#[test]
fn fx_pipeline_variables_in_amount() {
    use calnexus::EvalContext;
    // 金额位支持变量与嵌套算术（镜像真实域语义）
    let ctx = EvalContext::new().with_var("amount", 55.0);
    let ast = calnexus::parse("fx(amount * 2, \"USD\", \"CNY\")").unwrap();
    let (canonical, _) = calnexus::AstCanonicalizer::canonicalize(&ast).unwrap();
    let router = MockFxDomain::router();
    let r = router
        .route(&canonical)
        .unwrap()
        .evaluate(&canonical, &ctx)
        .unwrap();
    assert_scalar(&r, 780.0);
}

#[test]
fn fx_pipeline_errors() {
    let router = MockFxDomain::router();
    // 未知币种 → Domain + 支持币种数提示（base EUR + 4 报价 = 5）
    let err = eval_via_router("fx(1, \"USD\", \"XXX\")", &router).unwrap_err();
    assert_eq!(err.kind, ErrorKind::Domain);
    assert!(err.message.contains("unknown currency: XXX"), "got {err}");
    assert!(
        err.message.contains('5'),
        "should mention 5 supported, got {err}"
    );
    // 参数个数错误
    let err = eval_via_router("fx(1, \"USD\")", &router).unwrap_err();
    assert_eq!(err.kind, ErrorKind::Domain);
    let err = eval_via_router("fx_rate(\"USD\")", &router).unwrap_err();
    assert_eq!(err.kind, ErrorKind::Domain);
    // 币种位必须是字符串字面量
    let err = eval_via_router("fx(1, 2, 3)", &router).unwrap_err();
    assert_eq!(err.kind, ErrorKind::Domain);
    assert!(err.message.contains("string argument"), "got {err}");
    // 嵌入跨域函数 → fx 域拒绝并点名
    let err = eval_via_router("fx(sin(1), \"USD\", \"EUR\")", &router).unwrap_err();
    assert_eq!(err.kind, ErrorKind::Domain);
    assert!(err.message.contains("sin"), "got {err}");
    // Str 操作数上下文拒绝
    let err = eval_via_router("\"USD\" + 1", &router).unwrap_err();
    assert_eq!(err.kind, ErrorKind::Domain);
}

#[test]
fn fx_pipeline_nondeterministic_bypasses_cache() {
    use calnexus::{CacheManager, EvalContext, evaluate_with_router};
    // fx/fx_rate 声明为非确定性：同一表达式两次求值均 cache miss
    let router = MockFxDomain::router();
    let cache = CacheManager::new();
    let ctx = EvalContext::new();
    let (_, _, hit1, _) =
        evaluate_with_router("fx_rate(\"USD\", \"EUR\")", &ctx, None, &cache, &router).unwrap();
    let (_, _, hit2, _) =
        evaluate_with_router("fx_rate(\"USD\", \"EUR\")", &ctx, None, &cache, &router).unwrap();
    assert!(!hit1 && !hit2, "fx must bypass L1 cache");
    // 路由器视角：is_nondeterministic 识别 fx 调用
    let ast = calnexus::parse("fx_rate(\"USD\", \"EUR\")").unwrap();
    let (canonical, _) = calnexus::AstCanonicalizer::canonicalize(&ast).unwrap();
    assert!(router.is_nondeterministic(&canonical));
    // 对照：算术表达式是确定性的
    let ast = calnexus::parse("1+1").unwrap();
    let (canonical, _) = calnexus::AstCanonicalizer::canonicalize(&ast).unwrap();
    assert!(!router.is_nondeterministic(&canonical));
}

// ---------------------------------------------------------------------------
// math::fx 纯函数层
// ---------------------------------------------------------------------------

#[test]
fn fx_math_layer_convert_and_rate() {
    let table = mock_rate_table();
    // 三角换算语义：v / rate[F] * rate[T]
    assert!(approx_eq(
        fx::convert(110.0, "USD", "CNY", &table).unwrap(),
        780.0
    ));
    assert!(approx_eq(
        fx::convert(1.0, "EUR", "USD", &table).unwrap(),
        1.1
    ));
    // base 币种 rate 恒 1
    assert!(approx_eq(fx::get_rate("EUR", &table).unwrap(), 1.0));
    assert!(approx_eq(fx::get_rate("JPY", &table).unwrap(), 160.0));
    // 往返一致性
    let out = fx::convert(123.45, "GBP", "JPY", &table).unwrap();
    let back = fx::convert(out, "JPY", "GBP", &table).unwrap();
    assert!(approx_eq(back, 123.45), "round-trip drift: {back}");
}

#[test]
fn fx_math_layer_unknown_currency() {
    let table = mock_rate_table();
    let err = fx::convert(1.0, "USD", "FOO", &table).unwrap_err();
    assert_eq!(err.kind, ErrorKind::Domain);
    assert!(err.message.contains("FOO"), "got {err}");
    let err = fx::get_rate("BAR", &table).unwrap_err();
    assert_eq!(err.kind, ErrorKind::Domain);
    // unknown_currency_error 构造器：消息含支持数量与 base
    let e = fx::unknown_currency_error("QQQ", &table);
    assert!(
        e.message.contains("QQQ") && e.message.contains("EUR"),
        "got {e}"
    );
}

#[test]
fn fx_rate_provider_trait_objects() {
    // 公开 RateProvider trait 可被下游实现与分发（Semver 面验证）
    let p: Box<dyn RateProvider> = Box::new(StaticProvider::new(mock_rate_table()));
    let table = p.rates().unwrap();
    assert_eq!(table.base, "EUR");
    assert_eq!(table.date, "2026-09-01");
    assert_eq!(table.rates.len(), 4);
    assert!(approx_eq(table.rates["CNY"], 7.8));
}

// ---------------------------------------------------------------------------
// math::fx_scenario 场景层（预算 / 定价）
// ---------------------------------------------------------------------------

#[test]
fn fx_scenario_budget_happy() {
    let table = mock_rate_table();
    // 留学预算：11000 USD 学费 + 1000/月生活费 × 4 年 → CNY
    let b = budget_calculation(11000.0, "USD", Some(1000.0), 4, "CNY", &table).unwrap();
    assert!(approx_eq(b.rate, 7.8 / 1.1));
    assert!(approx_eq(b.tuition_home, 78_000.0));
    assert!(approx_eq(b.annual_tuition_home, 19_500.0));
    assert!(approx_eq(
        b.living_monthly_home.unwrap(),
        1000.0 / 1.1 * 7.8
    ));
    assert!(approx_eq(
        b.total_living_home.unwrap(),
        1000.0 / 1.1 * 7.8 * 48.0
    ));
    assert!(approx_eq(
        b.total_cost_home.unwrap(),
        78_000.0 + 1000.0 / 1.1 * 7.8 * 48.0
    ));
    // ±3% 风险区间基于总费用
    let total = b.total_cost_home.unwrap();
    assert!(approx_eq(b.exchange_risk.low, total * 0.97));
    assert!(approx_eq(b.exchange_risk.high, total * 1.03));
    assert_eq!(b.exchange_risk.currency, "CNY");
    // 无生活费时风险区间仅基于学费
    let b2 = budget_calculation(11000.0, "USD", None, 4, "CNY", &table).unwrap();
    assert!(b2.living_monthly_home.is_none() && b2.total_cost_home.is_none());
    assert!(approx_eq(b2.exchange_risk.low, 78_000.0 * 0.97));
}

#[test]
fn fx_scenario_budget_validation() {
    let table = mock_rate_table();
    for (tuition, duration) in [(0.0, 4u32), (-1.0, 4), (1000.0, 0), (1000.0, 11)] {
        let err = budget_calculation(tuition, "USD", None, duration, "CNY", &table).unwrap_err();
        assert_eq!(
            err.kind,
            ErrorKind::Domain,
            "tuition={tuition} duration={duration}"
        );
    }
    // 未知币种传播
    let err = budget_calculation(1000.0, "FOO", None, 4, "CNY", &table).unwrap_err();
    assert!(err.message.contains("FOO"), "got {err}");
}

#[test]
fn fx_scenario_pricing_happy() {
    let table = mock_rate_table();
    // 定价：成本 100 CNY、目标利润率 20%、平台费 5%、无缓冲
    let p = pricing_calculation(100.0, 0.2, &["USD", "JPY"], 0.05, 0.0, &table).unwrap();
    assert!(approx_eq(p.cost_cny, 100.0));
    assert_eq!(p.pricing.len(), 2);
    // 语义钉住：target_profit_rate 是对营收的利润率（目标收入 =
    // cost/(1-0.2) = 125），actual_profit_rate 是对成本的利润率
    // （(125-100)/100 = 25%）——两口径并存且都由回算字段体现
    let usd_rate = 7.8 / 1.1;
    let usd = &p.pricing[0];
    // 售价/平台费字段四舍五入到 2 位小数（展示语义）：
    // raw = 125 / 7.0909… / 0.95 = 18.5574… → 18.56
    assert!(approx_eq(usd.recommended_price, 18.56));
    assert!(approx_eq(
        usd.recommended_price,
        ((125.0 / usd_rate / 0.95) * 100.0_f64).round() / 100.0_f64
    ));
    // 回算使用未舍入售价：净收入恰为目标收入 125 → 利润 25（成本口径 25%）
    assert!(approx_eq(usd.actual_profit_rate, 0.25));
    assert!(approx_eq(usd.profit_cny, 25.0));
    assert!(approx_eq(usd.platform_fee_cny, 6.58));
    // 平台费为 0 时回算不变：实际利润率仍为成本口径 25%
    let p2 = pricing_calculation(100.0, 0.2, &["USD"], 0.0, 0.0, &table).unwrap();
    assert!(approx_eq(p2.pricing[0].actual_profit_rate, 0.25));
    assert!(approx_eq(p2.pricing[0].profit_cny, 25.0));
}

#[test]
fn fx_scenario_pricing_validation() {
    let table = mock_rate_table();
    // 成本 / 利润率 / 平台费 / 缓冲越界
    for (cost, profit, platform, buffer) in [
        (0.0, 0.2, 0.05, 0.0),
        (-5.0, 0.2, 0.05, 0.0),
        (100.0, 0.0, 0.05, 0.0),
        (100.0, 1.0, 0.05, 0.0),
        (100.0, -0.1, 0.05, 0.0),
        (100.0, 0.2, 1.0, 0.0),
        (100.0, 0.2, -0.1, 0.0),
        (100.0, 0.2, 0.05, 0.6),
        (100.0, 0.2, 0.05, -0.1),
    ] {
        let err =
            pricing_calculation(cost, profit, &["USD"], platform, buffer, &table).unwrap_err();
        assert_eq!(
            err.kind,
            ErrorKind::Domain,
            "cost={cost} profit={profit} platform={platform} buffer={buffer}"
        );
    }
    // 币种列表空 / 超 10 个
    let err = pricing_calculation(100.0, 0.2, &[], 0.05, 0.0, &table).unwrap_err();
    assert_eq!(err.kind, ErrorKind::Domain);
    let many: Vec<&str> = ["USD", "EUR", "GBP", "JPY", "CNY"]
        .iter()
        .cycle()
        .take(11)
        .copied()
        .collect();
    let err = pricing_calculation(100.0, 0.2, &many, 0.05, 0.0, &table).unwrap_err();
    assert_eq!(err.kind, ErrorKind::Domain);
    // 未知币种
    let err = pricing_calculation(100.0, 0.2, &["FOO"], 0.05, 0.0, &table).unwrap_err();
    assert!(err.message.contains("FOO"), "got {err}");
}

// ---------------------------------------------------------------------------
// RateTable 公开结构契约
// ---------------------------------------------------------------------------

#[test]
fn fx_rate_table_public_fields() {
    // RateTable 字段公开可构造——下游/测试注入汇率表的合法通道
    let table = RateTable {
        base: "USD".to_string(),
        date: "2026-09-15".to_string(),
        rates: [("EUR".to_string(), 0.9)].into_iter().collect(),
    };
    assert!(approx_eq(
        fx::convert(90.0, "EUR", "USD", &table).unwrap(),
        100.0
    ));
}
