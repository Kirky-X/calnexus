// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! 汇率换算核心数学函数。
//!
//! 设计依据：design.md D6（base 三角换算）
//! Feature 门控：`fx = ["dep:ureq", "dep:dirs"]`
//!
//! 从 `domains/fx.rs` 提取的纯函数：汇率表结构 + 三角换算逻辑 + 错误构造。
//! 不依赖任何 I/O（网络/文件），仅包含纯数据运算。

use std::collections::HashMap;

use crate::core::CalcError;

/// 汇率表：base 币种 + 抓取日期 + 各币种汇率。
#[derive(Clone, Debug)]
pub struct RateTable {
    /// 基准币种（如 "EUR"）。
    pub base: String,
    /// 数据日期（RFC 3339 格式）。
    pub date: String,
    /// 各币种对 base 的汇率。
    pub rates: HashMap<String, f64>,
}

/// 汇率换算核心逻辑：经 base 三角计算。
///
/// - 同币种（from == to）→ 恒等返回 value（不查表）
/// - base 币种汇率视为 1.0（如 EUR 在 EUR-base 表中不出现）
/// - 未知币种 → DomainError（含支持币种数量提示，i18n msg.fx.unknown_currency）
pub fn convert(value: f64, from: &str, to: &str, table: &RateTable) -> Result<f64, CalcError> {
    // 同币种恒等（避免不必要的查表与除零风险）
    if from == to {
        return Ok(value);
    }

    let rate_from = get_rate(from, table)?;
    let rate_to = get_rate(to, table)?;

    // 三角换算：value / rate[FROM] * rate[TO]
    Ok(value / rate_from * rate_to)
}

/// 获取币种对 base 的汇率。base 币种返回 1.0。
pub fn get_rate(code: &str, table: &RateTable) -> Result<f64, CalcError> {
    if code == table.base {
        return Ok(1.0);
    }
    table
        .rates
        .get(code)
        .copied()
        .ok_or_else(|| unknown_currency_error(code, table))
}

/// 构造“未知币种”错误，消息含支持币种数量。
/// rates.len() + 1 以包含 base 币种本身。
pub fn unknown_currency_error(code: &str, table: &RateTable) -> CalcError {
    CalcError::domain(format!(
        "unknown currency: {}, {} currencies supported (base: {})",
        code,
        table.rates.len() + 1,
        table.base
    ))
    .with_i18n(
        "msg.fx.unknown_currency",
        vec![("code".to_string(), code.to_string())],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ErrorKind;

    fn eur_table() -> RateTable {
        let mut rates = HashMap::new();
        rates.insert("USD".to_string(), 1.08);
        rates.insert("CNY".to_string(), 7.85);
        rates.insert("GBP".to_string(), 0.85);
        rates.insert("JPY".to_string(), 165.0);
        RateTable {
            base: "EUR".to_string(),
            date: "2026-07-25".to_string(),
            rates,
        }
    }

    // ===== 三角换算 =====

    #[test]
    fn test_convert_same_currency() {
        let table = eur_table();
        assert_eq!(convert(100.0, "USD", "USD", &table).unwrap(), 100.0);
    }

    #[test]
    fn test_convert_triangle() {
        let table = eur_table();
        let r = convert(100.0, "USD", "CNY", &table).unwrap();
        assert!((r - 100.0 / 1.08 * 7.85).abs() < 1e-9);
    }

    #[test]
    fn test_convert_with_base_from() {
        let table = eur_table();
        // EUR → USD = 100 / 1.0 * 1.08 = 108
        let r = convert(100.0, "EUR", "USD", &table).unwrap();
        assert!((r - 108.0).abs() < 1e-9);
    }

    #[test]
    fn test_convert_with_base_to() {
        let table = eur_table();
        // USD → EUR = 108 / 1.08 * 1.0 = 100
        let r = convert(108.0, "USD", "EUR", &table).unwrap();
        assert!((r - 100.0).abs() < 1e-9);
    }

    #[test]
    fn test_convert_unknown_currency() {
        let table = eur_table();
        let err = convert(100.0, "XYZ", "USD", &table).expect_err("expected error");
        assert_eq!(err.kind, ErrorKind::Domain);
        assert_eq!(err.i18n_key, Some("msg.fx.unknown_currency"));
    }

    // ===== get_rate =====

    #[test]
    fn test_get_rate_base_currency() {
        let table = eur_table();
        assert_eq!(get_rate("EUR", &table).unwrap(), 1.0);
    }

    #[test]
    fn test_get_rate_known_currency() {
        let table = eur_table();
        assert!((get_rate("USD", &table).unwrap() - 1.08).abs() < 1e-9);
    }

    #[test]
    fn test_get_rate_unknown_currency() {
        let table = eur_table();
        let err = get_rate("XYZ", &table).expect_err("expected error");
        assert_eq!(err.kind, ErrorKind::Domain);
        assert_eq!(err.i18n_key, Some("msg.fx.unknown_currency"));
    }

    // ===== unknown_currency_error =====

    #[test]
    fn test_unknown_currency_error_message_includes_count() {
        let table = eur_table();
        let err = unknown_currency_error("XYZ", &table);
        assert!(err.message.contains("XYZ"));
        assert!(err.message.contains("5 currencies"));
        assert_eq!(err.i18n_key, Some("msg.fx.unknown_currency"));
    }

    #[test]
    fn test_unknown_currency_error_empty_table() {
        let table = RateTable {
            base: "EUR".to_string(),
            date: "2026-07-25".to_string(),
            rates: HashMap::new(),
        };
        let err = unknown_currency_error("XYZ", &table);
        assert!(err.message.contains("1 currencies"));
    }

    // ===== RateTable =====

    #[test]
    fn test_rate_table_clone() {
        let table = eur_table();
        let cloned = table.clone();
        assert_eq!(cloned.base, table.base);
        assert_eq!(cloned.date, table.date);
        assert_eq!(cloned.rates.len(), table.rates.len());
    }

    #[test]
    fn test_rate_table_debug() {
        let table = eur_table();
        let s = format!("{:?}", table);
        assert!(s.contains("EUR"));
    }
}
