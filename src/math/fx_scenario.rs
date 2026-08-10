// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! FX 场景化计算纯函数：留学预算 + 跨境定价。
//!
//! 从 `mcp_exchange_master` 整合的场景化工具，提取为不依赖 I/O 的纯函数。
//! 接受 `&RateTable` 参数，由调用方（server 层）注入汇率数据。

use crate::core::CalcError;
use crate::math::fx::{convert, RateTable};

/// 留学预算计算结果。
#[derive(Clone, Debug, PartialEq)]
pub struct BudgetResult {
    /// 外币学费原值。
    pub tuition_foreign: f64,
    /// 学费货币代码。
    pub tuition_currency: String,
    /// 学费换算为本币金额。
    pub tuition_home: f64,
    /// 本币代码。
    pub home_currency: String,
    /// 汇率（1 单位外币 = ? 本币）。
    pub rate: f64,
    /// 留学年限。
    pub duration_years: u32,
    /// 年均学费（本币）。
    pub annual_tuition_home: f64,
    /// 月生活费（外币），None 表示未提供。
    pub living_monthly_foreign: Option<f64>,
    /// 月生活费（本币），None 表示未提供。
    pub living_monthly_home: Option<f64>,
    /// 总生活费（本币），None 表示未提供。
    pub total_living_home: Option<f64>,
    /// 总费用（学费 + 生活费，本币）。None 表示无生活费数据。
    pub total_cost_home: Option<f64>,
    /// 汇率风险区间（±3%）。
    pub exchange_risk: ExchangeRisk,
}

/// 汇率风险区间。
#[derive(Clone, Debug, PartialEq)]
pub struct ExchangeRisk {
    /// 下限（本币）。
    pub low: f64,
    /// 上限（本币）。
    pub high: f64,
    /// 本币代码。
    pub currency: String,
}

/// 跨境定价单项结果。
#[derive(Clone, Debug, PartialEq)]
pub struct PricingItem {
    /// 目标货币代码。
    pub currency: String,
    /// 建议售价（外币）。
    pub recommended_price: f64,
    /// 实际利润率（回算后）。
    pub actual_profit_rate: f64,
    /// 利润（本币）。
    pub profit_cny: f64,
    /// 平台费（本币）。
    pub platform_fee_cny: f64,
}

/// 跨境定价计算结果。
#[derive(Clone, Debug, PartialEq)]
pub struct PricingResult {
    /// 人民币成本。
    pub cost_cny: f64,
    /// 各币种定价明细。
    pub pricing: Vec<PricingItem>,
}

/// 留学预算计算（纯函数，不依赖 I/O）。
///
/// 将外币学费和生活费换算为本币，计算总费用并给出 ±3% 汇率风险区间。
pub fn budget_calculation(
    tuition: f64,
    tuition_currency: &str,
    living_cost_monthly: Option<f64>,
    duration_years: u32,
    home_currency: &str,
    table: &RateTable,
) -> Result<BudgetResult, CalcError> {
    // 输入校验
    if tuition <= 0.0 {
        return Err(CalcError::domain("tuition must be positive".to_string()));
    }
    if duration_years == 0 || duration_years > 10 {
        return Err(CalcError::domain(
            "duration_years must be between 1 and 10".to_string(),
        ));
    }

    // 汇率：1 单位 tuition_currency = ? home_currency
    let rate = convert(1.0, tuition_currency, home_currency, table)?;

    // 学费换算
    let tuition_home = convert(tuition, tuition_currency, home_currency, table)?;
    let annual_tuition_home = tuition_home / duration_years as f64;

    // 生活费换算
    let (living_monthly_home, total_living_home, total_cost_home) =
        match living_cost_monthly {
            Some(monthly) if monthly > 0.0 => {
                let monthly_home =
                    convert(monthly, tuition_currency, home_currency, table)?;
                let total_living = monthly_home * 12.0 * duration_years as f64;
                let total = tuition_home + total_living;
                (Some(monthly_home), Some(total_living), Some(total))
            }
            _ => (None, None, None),
        };

    // ±3% 汇率风险区间（基于总费用或仅学费）
    let base_for_risk = total_cost_home.unwrap_or(tuition_home);
    let exchange_risk = ExchangeRisk {
        low: base_for_risk * 0.97,
        high: base_for_risk * 1.03,
        currency: home_currency.to_string(),
    };

    Ok(BudgetResult {
        tuition_foreign: tuition,
        tuition_currency: tuition_currency.to_string(),
        tuition_home,
        home_currency: home_currency.to_string(),
        rate,
        duration_years,
        annual_tuition_home,
        living_monthly_foreign: living_cost_monthly,
        living_monthly_home,
        total_living_home,
        total_cost_home,
        exchange_risk,
    })
}

/// 跨境电商多币种定价计算（纯函数，不依赖 I/O）。
///
/// 根据人民币成本、目标利润率、平台费率，计算各币种安全售价并回算实际利润率。
pub fn pricing_calculation(
    cost_cny: f64,
    target_profit_rate: f64,
    currencies: &[&str],
    platform_rate: f64,
    safety_buffer: f64,
    table: &RateTable,
) -> Result<PricingResult, CalcError> {
    // 输入校验
    if cost_cny <= 0.0 {
        return Err(CalcError::domain("cost_cny must be positive".to_string()));
    }
    if target_profit_rate <= 0.0 || target_profit_rate >= 1.0 {
        return Err(CalcError::domain(
            "target_profit_rate must be between 0 and 1 (exclusive)".to_string(),
        ));
    }
    if platform_rate < 0.0 || platform_rate >= 1.0 {
        return Err(CalcError::domain(
            "platform_rate must be in [0, 1)".to_string(),
        ));
    }
    if safety_buffer < 0.0 || safety_buffer > 0.5 {
        return Err(CalcError::domain(
            "safety_buffer must be in [0, 0.5]".to_string(),
        ));
    }
    if currencies.is_empty() {
        return Err(CalcError::domain(
            "currencies list must not be empty".to_string(),
        ));
    }
    if currencies.len() > 10 {
        return Err(CalcError::domain(
            "currencies list must not exceed 10 items".to_string(),
        ));
    }

    let mut pricing = Vec::with_capacity(currencies.len());

    for &cur in currencies {
        // 1 外币 = ? CNY（经 base 三角换算）
        let one_foreign_in_cny = convert(1.0, cur, "CNY", table)?;

        // 安全汇率（考虑缓冲）：1外币 = ?CNY（打折后）
        let safe_rate = one_foreign_in_cny * (1.0 - safety_buffer);

        // 目标收入(CNY) = 成本 / (1 - 目标利润率)
        let target_revenue_cny = cost_cny / (1.0 - target_profit_rate);

        // 售价(外币) = 目标收入CNY / 安全汇率 / (1 - 平台费率)
        // 即：售价 × safe_rate × (1 - platform_rate) >= target_revenue_cny
        let price_foreign = target_revenue_cny / safe_rate / (1.0 - platform_rate);

        // 回算验证（用实时汇率，非安全汇率）
        let gross_cny = price_foreign * one_foreign_in_cny;
        let platform_fee_cny = gross_cny * platform_rate;
        let net_cny = gross_cny - platform_fee_cny;
        let profit_cny = net_cny - cost_cny;
        let actual_profit_rate = profit_cny / cost_cny;

        pricing.push(PricingItem {
            currency: cur.to_string(),
            recommended_price: (price_foreign * 100.0).round() / 100.0, // 保留2位小数
            actual_profit_rate: (actual_profit_rate * 10000.0).round() / 10000.0, // 4位
            profit_cny: (profit_cny * 100.0).round() / 100.0,
            platform_fee_cny: (platform_fee_cny * 100.0).round() / 100.0,
        });
    }

    Ok(PricingResult {
        cost_cny,
        pricing,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// 测试用 mock 汇率表：EUR-base，含 USD/CNY/GBP/JPY。
    fn mock_table() -> RateTable {
        let mut rates = HashMap::new();
        rates.insert("USD".to_string(), 1.08);
        rates.insert("CNY".to_string(), 7.85);
        rates.insert("GBP".to_string(), 0.85);
        rates.insert("JPY".to_string(), 165.0);
        RateTable {
            base: "EUR".to_string(),
            date: "2026-08-10".to_string(),
            rates,
        }
    }

    // ===== budget_calculation() 测试 =====

    #[test]
    fn test_budget_happy_path_with_living() {
        // 50000 USD 学费 + 2000 USD/月生活费，4年，换算为 CNY
        // rate(USD→CNY) = 1/1.08 * 7.85 ≈ 7.2685
        let table = mock_table();
        let result = budget_calculation(
            50000.0,
            "USD",
            Some(2000.0),
            4,
            "CNY",
            &table,
        )
        .unwrap();

        // 学费换算
        let expected_rate = 1.0 / 1.08 * 7.85;
        assert!(
            (result.rate - expected_rate).abs() < 1e-6,
            "rate mismatch"
        );
        let expected_tuition_home = 50000.0 / 1.08 * 7.85;
        assert!(
            (result.tuition_home - expected_tuition_home).abs() < 0.01,
            "tuition_home mismatch"
        );

        // 年均学费
        assert!(
            (result.annual_tuition_home - expected_tuition_home / 4.0).abs() < 0.01
        );

        // 生活费
        let expected_monthly_home = 2000.0 / 1.08 * 7.85;
        assert!(
            (result.living_monthly_home.unwrap() - expected_monthly_home).abs() < 0.01
        );
        let expected_total_living = expected_monthly_home * 12.0 * 4.0;
        assert!(
            (result.total_living_home.unwrap() - expected_total_living).abs() < 0.01
        );

        // 总费用
        let expected_total = expected_tuition_home + expected_total_living;
        assert!(
            (result.total_cost_home.unwrap() - expected_total).abs() < 0.01
        );

        // ±3% 风险区间
        assert!(
            (result.exchange_risk.low - expected_total * 0.97).abs() < 0.01
        );
        assert!(
            (result.exchange_risk.high - expected_total * 1.03).abs() < 0.01
        );
        assert_eq!(result.exchange_risk.currency, "CNY");
    }

    #[test]
    fn test_budget_tuition_only_no_living() {
        let table = mock_table();
        let result =
            budget_calculation(10000.0, "USD", None, 1, "CNY", &table).unwrap();

        assert!(result.living_monthly_home.is_none());
        assert!(result.total_living_home.is_none());
        assert!(result.total_cost_home.is_none());

        // 风险区间基于仅学费
        let tuition_home = 10000.0 / 1.08 * 7.85;
        assert!(
            (result.exchange_risk.low - tuition_home * 0.97).abs() < 0.01
        );
    }

    #[test]
    fn test_budget_same_currency_identity() {
        let table = mock_table();
        let result =
            budget_calculation(50000.0, "USD", None, 2, "USD", &table).unwrap();

        assert!((result.rate - 1.0).abs() < 1e-9, "same currency rate should be 1.0");
        assert!((result.tuition_home - 50000.0).abs() < 1e-9);
        assert!((result.annual_tuition_home - 25000.0).abs() < 1e-9);
    }

    #[test]
    fn test_budget_unknown_currency_error() {
        let table = mock_table();
        let result = budget_calculation(50000.0, "XYZ", None, 2, "CNY", &table);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind, crate::core::ErrorKind::Domain);
    }

    #[test]
    fn test_budget_zero_duration_error() {
        let table = mock_table();
        let result = budget_calculation(50000.0, "USD", None, 0, "CNY", &table);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind, crate::core::ErrorKind::Domain);
    }

    #[test]
    fn test_budget_zero_tuition_error() {
        let table = mock_table();
        let result = budget_calculation(0.0, "USD", None, 2, "CNY", &table);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind, crate::core::ErrorKind::Domain);
    }

    #[test]
    fn test_budget_duration_exceeds_max_error() {
        let table = mock_table();
        let result = budget_calculation(50000.0, "USD", None, 11, "CNY", &table);
        assert!(result.is_err());
    }

    // ===== pricing_calculation() 测试 =====

    #[test]
    fn test_pricing_happy_path_single_currency() {
        let table = mock_table();
        let result = pricing_calculation(
            50.0,
            0.3,
            &["USD"],
            0.15,
            0.05,
            &table,
        )
        .unwrap();

        assert_eq!(result.cost_cny, 50.0);
        assert_eq!(result.pricing.len(), 1);

        let item = &result.pricing[0];
        assert_eq!(item.currency, "USD");
        assert!(item.recommended_price > 0.0);
        // 实际利润率应接近目标（因安全缓冲略高）
        assert!(
            item.actual_profit_rate >= 0.3,
            "actual profit rate {} should be >= target 0.3 (safety buffer)",
            item.actual_profit_rate
        );
        assert!(item.profit_cny > 0.0);
        assert!(item.platform_fee_cny > 0.0);
    }

    #[test]
    fn test_pricing_multi_currency() {
        let table = mock_table();
        let result = pricing_calculation(
            100.0,
            0.25,
            &["USD", "EUR", "GBP"],
            0.10,
            0.05,
            &table,
        )
        .unwrap();

        assert_eq!(result.pricing.len(), 3);
        for item in &result.pricing {
            assert!(item.recommended_price > 0.0);
            assert!(item.profit_cny > 0.0);
        }
    }

    #[test]
    fn test_pricing_zero_cost_error() {
        let table = mock_table();
        let result = pricing_calculation(0.0, 0.3, &["USD"], 0.15, 0.05, &table);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind, crate::core::ErrorKind::Domain);
    }

    #[test]
    fn test_pricing_negative_cost_error() {
        let table = mock_table();
        let result =
            pricing_calculation(-10.0, 0.3, &["USD"], 0.15, 0.05, &table);
        assert!(result.is_err());
    }

    #[test]
    fn test_pricing_profit_rate_zero_error() {
        let table = mock_table();
        let result = pricing_calculation(50.0, 0.0, &["USD"], 0.15, 0.05, &table);
        assert!(result.is_err());
    }

    #[test]
    fn test_pricing_profit_rate_one_error() {
        let table = mock_table();
        let result = pricing_calculation(50.0, 1.0, &["USD"], 0.15, 0.05, &table);
        assert!(result.is_err());
    }

    #[test]
    fn test_pricing_empty_currencies_error() {
        let table = mock_table();
        let result = pricing_calculation(50.0, 0.3, &[], 0.15, 0.05, &table);
        assert!(result.is_err());
    }

    #[test]
    fn test_pricing_too_many_currencies_error() {
        let table = mock_table();
        let currencies: Vec<&str> = (0..11).map(|_| "USD").collect();
        let result = pricing_calculation(50.0, 0.3, &currencies, 0.15, 0.05, &table);
        assert!(result.is_err());
    }

    #[test]
    fn test_pricing_unknown_currency_error() {
        let table = mock_table();
        let result =
            pricing_calculation(50.0, 0.3, &["XYZ"], 0.15, 0.05, &table);
        assert!(result.is_err());
    }

    #[test]
    fn test_pricing_zero_platform_rate() {
        let table = mock_table();
        let result =
            pricing_calculation(50.0, 0.3, &["USD"], 0.0, 0.05, &table).unwrap();
        assert_eq!(result.pricing[0].platform_fee_cny, 0.0);
    }

    #[test]
    fn test_pricing_zero_safety_buffer() {
        let table = mock_table();
        let result =
            pricing_calculation(50.0, 0.3, &["USD"], 0.15, 0.0, &table).unwrap();
        // 无缓冲时售价应低于有缓冲时（更安全但利润更紧）
        assert!(result.pricing[0].recommended_price > 0.0);
    }
}
