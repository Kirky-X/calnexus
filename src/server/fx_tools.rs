// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! FX 场景化工具：`fx_budget` + `fx_pricing`（`#[forge]` 双协议端点）。
//!
//! 基于 `math::fx_scenario` 纯函数 + `FrankfurterProvider` 三级缓存汇率。
//! Feature 门控：`fx`（依赖 math::fx_scenario + domains::fx_provider）。
//!
//! `#[forge]` 宏自动生成 HTTP 路由（`POST /api/v1/fx_budget`）与 MCP tool 注册。
//! `spawn_blocking` 隔离同步 `RateProvider::rates()` 调用（与 evaluate handler 策略一致）。

use crate::domains::fx_provider::{FrankfurterProvider, RateProvider};
use crate::math::fx_scenario::{budget_calculation, pricing_calculation};
use sdforge::error::ApiError;
use sdforge::forge;
use std::sync::OnceLock;

// ============================================================
// Request / Response DTOs
// ============================================================

/// `POST /api/v1/fx_budget` + MCP `fx_budget` tool 请求。
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct FxBudgetRequest {
    /// 学费金额（外币，必须 > 0）。
    pub tuition: f64,
    /// 学费货币代码（ISO 4217，如 "USD"）。
    pub tuition_currency: String,
    /// 月均生活费（外币），可选。
    #[serde(default)]
    pub living_cost_monthly: Option<f64>,
    /// 留学年限（1..=10）。
    pub duration_years: u32,
    /// 本币代码（ISO 4217，如 "CNY"）。
    pub home_currency: String,
}

/// `fx_budget` 响应。
#[derive(Debug, Clone, serde::Serialize)]
pub struct FxBudgetResponse {
    pub tuition_foreign: f64,
    pub tuition_currency: String,
    pub tuition_home: f64,
    pub home_currency: String,
    pub rate: f64,
    pub duration_years: u32,
    pub annual_tuition_home: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub living_monthly_home: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_living_home: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_cost_home: Option<f64>,
    pub exchange_risk: ExchangeRiskResponse,
}

/// 汇率风险区间响应。
#[derive(Debug, Clone, serde::Serialize)]
pub struct ExchangeRiskResponse {
    pub range: String,
    pub note: String,
}

/// `POST /api/v1/fx_pricing` + MCP `fx_pricing` tool 请求。
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct FxPricingRequest {
    /// 产品成本（人民币，必须 > 0）。
    pub cost_cny: f64,
    /// 目标利润率（0 < rate < 1，如 0.3 = 30%）。
    pub target_profit_rate: f64,
    /// 目标币种列表（逗号分隔，如 "USD,EUR,GBP"，1..=10 种）。
    pub currencies: String,
    /// 平台综合费率（0 <= rate < 1，如 0.15 = 15%）。
    #[serde(default = "default_platform_rate")]
    pub platform_rate: f64,
    /// 汇率安全缓冲（0 <= buffer <= 0.5，如 0.05 = 5%）。
    #[serde(default = "default_safety_buffer")]
    pub safety_buffer: f64,
}

fn default_platform_rate() -> f64 {
    0.15
}

fn default_safety_buffer() -> f64 {
    0.05
}

/// `fx_pricing` 响应。
#[derive(Debug, Clone, serde::Serialize)]
pub struct FxPricingResponse {
    pub cost_cny: f64,
    pub target_profit_rate: f64,
    pub platform_rate: f64,
    pub safety_buffer: f64,
    pub pricing: Vec<PricingItemResponse>,
}

/// 单币种定价明细。
#[derive(Debug, Clone, serde::Serialize)]
pub struct PricingItemResponse {
    pub currency: String,
    pub recommended_price: f64,
    pub actual_profit_rate: f64,
    pub profit_cny: f64,
    pub platform_fee_cny: f64,
}

// ============================================================
// Provider 单例
// ============================================================

/// 进程级 FrankfurterProvider 单例（与 FxDomain 独立，共享底层缓存文件）。
fn shared_provider() -> &'static FrankfurterProvider {
    static PROVIDER: OnceLock<FrankfurterProvider> = OnceLock::new();
    PROVIDER.get_or_init(FrankfurterProvider::new)
}

// ============================================================
// Request 校验
// ============================================================

impl FxBudgetRequest {
    /// 校验请求参数约束。
    fn validate(&self) -> Result<(), ApiError> {
        if self.tuition <= 0.0 {
            return Err(ApiError::validation(
                "tuition",
                "tuition must be positive",
            ));
        }
        if self.duration_years == 0 || self.duration_years > 10 {
            return Err(ApiError::validation(
                "duration_years",
                "duration_years must be between 1 and 10",
            ));
        }
        if self.tuition_currency.len() != 3 || !self.tuition_currency.chars().all(|c| c.is_ascii_alphabetic()) {
            return Err(ApiError::validation(
                "tuition_currency",
                "must be a 3-letter ISO 4217 currency code",
            ));
        }
        if self.home_currency.len() != 3 || !self.home_currency.chars().all(|c| c.is_ascii_alphabetic()) {
            return Err(ApiError::validation(
                "home_currency",
                "must be a 3-letter ISO 4217 currency code",
            ));
        }
        if let Some(living) = self.living_cost_monthly {
            if living < 0.0 {
                return Err(ApiError::validation(
                    "living_cost_monthly",
                    "living_cost_monthly must be non-negative",
                ));
            }
        }
        Ok(())
    }
}

impl FxPricingRequest {
    /// 校验请求参数约束。
    fn validate(&self) -> Result<(), ApiError> {
        if self.cost_cny <= 0.0 {
            return Err(ApiError::validation(
                "cost_cny",
                "cost_cny must be positive",
            ));
        }
        if self.target_profit_rate <= 0.0 || self.target_profit_rate >= 1.0 {
            return Err(ApiError::validation(
                "target_profit_rate",
                "target_profit_rate must be between 0 and 1 (exclusive)",
            ));
        }
        if self.platform_rate < 0.0 || self.platform_rate >= 1.0 {
            return Err(ApiError::validation(
                "platform_rate",
                "platform_rate must be in [0, 1)",
            ));
        }
        if self.safety_buffer < 0.0 || self.safety_buffer > 0.5 {
            return Err(ApiError::validation(
                "safety_buffer",
                "safety_buffer must be in [0, 0.5]",
            ));
        }
        let currencies: Vec<&str> = self
            .currencies
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        if currencies.is_empty() {
            return Err(ApiError::validation(
                "currencies",
                "currencies list must not be empty",
            ));
        }
        if currencies.len() > 10 {
            return Err(ApiError::validation(
                "currencies",
                "currencies list must not exceed 10 items",
            ));
        }
        for cur in &currencies {
            if cur.len() != 3 || !cur.chars().all(|c| c.is_ascii_alphabetic()) {
                return Err(ApiError::validation(
                    "currencies",
                    format!("'{}' is not a valid 3-letter ISO 4217 currency code", cur),
                ));
            }
        }
        Ok(())
    }
}

// ============================================================
// #[forge] handlers
// ============================================================

/// `POST /api/v1/fx_budget` + MCP `fx_budget` tool。
///
/// 留学费用预算：将外币学费和生活费换算为本币，计算总费用并给出 ±3% 汇率风险区间。
#[forge(
    name = "fx_budget",
    version = 1,
    path = "/fx_budget",
    method = "POST",
    tool_name = "fx_budget",
    description = "Calculate study abroad budget with currency conversion. Returns tuition, living costs, total, and ±3% exchange rate risk range."
)]
pub(crate) async fn fx_budget(req: FxBudgetRequest) -> Result<FxBudgetResponse, ApiError> {
    req.validate()?;

    let tuition = req.tuition;
    let tuition_currency = req.tuition_currency.clone();
    let living_cost = req.living_cost_monthly;
    let duration = req.duration_years;
    let home_currency = req.home_currency.clone();

    // spawn_blocking 隔离同步 RateProvider 调用
    let join_result = tokio::task::spawn_blocking(move || {
        let provider = shared_provider();
        let table = provider.rates().map_err(|_| {
            ApiError::service_unavailable("fx_budget", Some(5))
        })?;
        budget_calculation(
            tuition,
            &tuition_currency,
            living_cost,
            duration,
            &home_currency,
            &table,
        )
        .map_err(|e| ApiError::invalid_input(e.message, None, None))
    })
    .await;

    match join_result {
        Ok(Ok(result)) => Ok(FxBudgetResponse {
            tuition_foreign: result.tuition_foreign,
            tuition_currency: result.tuition_currency,
            tuition_home: result.tuition_home,
            home_currency: result.home_currency,
            rate: result.rate,
            duration_years: result.duration_years,
            annual_tuition_home: result.annual_tuition_home,
            living_monthly_home: result.living_monthly_home,
            total_living_home: result.total_living_home,
            total_cost_home: result.total_cost_home,
            exchange_risk: ExchangeRiskResponse {
                range: format!(
                    "{:.0}~{:.0} {}",
                    result.exchange_risk.low,
                    result.exchange_risk.high,
                    result.exchange_risk.currency
                ),
                note: "基于±3%汇率波动估算，建议分批换汇降低风险".to_string(),
            },
        }),
        Ok(Err(api_err)) => Err(api_err),
        Err(join_err) => Err(ApiError::internal_with_source(
            "fx_budget task failed",
            "spawn_blocking",
            join_err,
        )),
    }
}

/// `POST /api/v1/fx_pricing` + MCP `fx_pricing` tool。
///
/// 跨境电商多站点定价：根据人民币成本和目标利润率，计算各币种安全售价。
#[forge(
    name = "fx_pricing",
    version = 1,
    path = "/fx_pricing",
    method = "POST",
    tool_name = "fx_pricing",
    description = "Cross-border e-commerce multi-currency pricing. Calculates recommended prices with safety buffer and platform fees."
)]
pub(crate) async fn fx_pricing(
    req: FxPricingRequest,
) -> Result<FxPricingResponse, ApiError> {
    req.validate()?;

    let cost = req.cost_cny;
    let profit_rate = req.target_profit_rate;
    let platform_rate = req.platform_rate;
    let safety_buffer = req.safety_buffer;
    let currencies: Vec<String> = req
        .currencies
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let join_result = tokio::task::spawn_blocking(move || {
        let provider = shared_provider();
        let table = provider.rates().map_err(|_| {
            ApiError::service_unavailable("fx_pricing", Some(5))
        })?;
        let cur_refs: Vec<&str> = currencies.iter().map(|s| s.as_str()).collect();
        pricing_calculation(cost, profit_rate, &cur_refs, platform_rate, safety_buffer, &table)
            .map_err(|e| ApiError::invalid_input(e.message, None, None))
    })
    .await;

    match join_result {
        Ok(Ok(result)) => Ok(FxPricingResponse {
            cost_cny: result.cost_cny,
            target_profit_rate: profit_rate,
            platform_rate,
            safety_buffer,
            pricing: result
                .pricing
                .into_iter()
                .map(|item| PricingItemResponse {
                    currency: item.currency,
                    recommended_price: item.recommended_price,
                    actual_profit_rate: item.actual_profit_rate,
                    profit_cny: item.profit_cny,
                    platform_fee_cny: item.platform_fee_cny,
                })
                .collect(),
        }),
        Ok(Err(api_err)) => Err(api_err),
        Err(join_err) => Err(ApiError::internal_with_source(
            "fx_pricing task failed",
            "spawn_blocking",
            join_err,
        )),
    }
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    // === DTO 序列化/反序列化 ===

    #[test]
    fn test_fx_budget_request_deserialize() {
        let json = r#"{
            "tuition": 50000.0,
            "tuition_currency": "USD",
            "living_cost_monthly": 2000.0,
            "duration_years": 4,
            "home_currency": "CNY"
        }"#;
        let req: FxBudgetRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.tuition, 50000.0);
        assert_eq!(req.tuition_currency, "USD");
        assert_eq!(req.living_cost_monthly, Some(2000.0));
        assert_eq!(req.duration_years, 4);
        assert_eq!(req.home_currency, "CNY");
    }

    #[test]
    fn test_fx_budget_request_no_living() {
        let json = r#"{
            "tuition": 10000.0,
            "tuition_currency": "EUR",
            "duration_years": 2,
            "home_currency": "CNY"
        }"#;
        let req: FxBudgetRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.living_cost_monthly, None);
    }

    #[test]
    fn test_fx_pricing_request_deserialize() {
        let json = r#"{
            "cost_cny": 50.0,
            "target_profit_rate": 0.3,
            "currencies": "USD,EUR,GBP"
        }"#;
        let req: FxPricingRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.cost_cny, 50.0);
        assert_eq!(req.target_profit_rate, 0.3);
        assert_eq!(req.currencies, "USD,EUR,GBP");
        assert_eq!(req.platform_rate, 0.15); // default
        assert_eq!(req.safety_buffer, 0.05); // default
    }

    #[test]
    fn test_fx_pricing_request_custom_rates() {
        let json = r#"{
            "cost_cny": 100.0,
            "target_profit_rate": 0.25,
            "currencies": "USD",
            "platform_rate": 0.10,
            "safety_buffer": 0.03
        }"#;
        let req: FxPricingRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.platform_rate, 0.10);
        assert_eq!(req.safety_buffer, 0.03);
    }

    // === 校验测试 ===

    #[test]
    fn test_fx_budget_validate_accepts_valid() {
        let req = FxBudgetRequest {
            tuition: 50000.0,
            tuition_currency: "USD".into(),
            living_cost_monthly: Some(2000.0),
            duration_years: 4,
            home_currency: "CNY".into(),
        };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn test_fx_budget_validate_rejects_zero_tuition() {
        let req = FxBudgetRequest {
            tuition: 0.0,
            tuition_currency: "USD".into(),
            living_cost_monthly: None,
            duration_years: 2,
            home_currency: "CNY".into(),
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_fx_budget_validate_rejects_zero_duration() {
        let req = FxBudgetRequest {
            tuition: 50000.0,
            tuition_currency: "USD".into(),
            living_cost_monthly: None,
            duration_years: 0,
            home_currency: "CNY".into(),
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_fx_budget_validate_rejects_invalid_currency() {
        let req = FxBudgetRequest {
            tuition: 50000.0,
            tuition_currency: "USDX".into(), // 4 letters
            living_cost_monthly: None,
            duration_years: 2,
            home_currency: "CNY".into(),
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_fx_pricing_validate_accepts_valid() {
        let req = FxPricingRequest {
            cost_cny: 50.0,
            target_profit_rate: 0.3,
            currencies: "USD,EUR".into(),
            platform_rate: 0.15,
            safety_buffer: 0.05,
        };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn test_fx_pricing_validate_rejects_empty_currencies() {
        let req = FxPricingRequest {
            cost_cny: 50.0,
            target_profit_rate: 0.3,
            currencies: "".into(),
            platform_rate: 0.15,
            safety_buffer: 0.05,
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_fx_pricing_validate_rejects_too_many_currencies() {
        let req = FxPricingRequest {
            cost_cny: 50.0,
            target_profit_rate: 0.3,
            currencies: "USD,EUR,GBP,JPY,CNY,KRW,AUD,CAD,CHF,SGD,NZD".into(),
            platform_rate: 0.15,
            safety_buffer: 0.05,
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_fx_pricing_validate_rejects_profit_rate_one() {
        let req = FxPricingRequest {
            cost_cny: 50.0,
            target_profit_rate: 1.0,
            currencies: "USD".into(),
            platform_rate: 0.15,
            safety_buffer: 0.05,
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_fx_pricing_validate_rejects_negative_cost() {
        let req = FxPricingRequest {
            cost_cny: -10.0,
            target_profit_rate: 0.3,
            currencies: "USD".into(),
            platform_rate: 0.15,
            safety_buffer: 0.05,
        };
        assert!(req.validate().is_err());
    }

    // === Response 序列化 ===

    #[test]
    fn test_fx_budget_response_serialize() {
        let resp = FxBudgetResponse {
            tuition_foreign: 50000.0,
            tuition_currency: "USD".into(),
            tuition_home: 362500.0,
            home_currency: "CNY".into(),
            rate: 7.25,
            duration_years: 4,
            annual_tuition_home: 90625.0,
            living_monthly_home: Some(14500.0),
            total_living_home: Some(696000.0),
            total_cost_home: Some(1058500.0),
            exchange_risk: ExchangeRiskResponse {
                range: "1027345~1089655 CNY".into(),
                note: "test".into(),
            },
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains(r#""tuition_foreign":50000.0"#));
        assert!(json.contains(r#""tuition_currency":"USD""#));
        assert!(json.contains(r#""exchange_risk""#));
    }

    #[test]
    fn test_fx_pricing_response_serialize() {
        let resp = FxPricingResponse {
            cost_cny: 50.0,
            target_profit_rate: 0.3,
            platform_rate: 0.15,
            safety_buffer: 0.05,
            pricing: vec![PricingItemResponse {
                currency: "USD".into(),
                recommended_price: 10.99,
                actual_profit_rate: 0.287,
                profit_cny: 14.35,
                platform_fee_cny: 1.65,
            }],
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains(r#""cost_cny":50.0"#));
        assert!(json.contains(r#""recommended_price":10.99"#));
        assert!(json.contains(r#""currency":"USD""#));
    }

    // === shared_provider 单例 ===

    #[test]
    fn test_shared_provider_returns_same_instance() {
        let p1 = shared_provider() as *const FrankfurterProvider;
        let p2 = shared_provider() as *const FrankfurterProvider;
        assert_eq!(p1, p2, "shared_provider should return same instance");
    }
}
