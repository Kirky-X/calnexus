// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! 汇率换算核心函数：base 三角换算，纯数据运算不依赖 I/O。

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
    /// 数据来源标识（"frankfurter" | "erapi"），用于混源可观测。
    pub source: String,
}

/// 汇率换算：经 base 三角计算；同币种原样返回，base 汇率视为 1.0，未知币种报 Domain。
pub fn convert(value: f64, from: &str, to: &str, table: &RateTable) -> Result<f64, CalcError> {
    if from == to {
        return Ok(value);
    }

    let rate_from = get_rate(from, table)?;
    let rate_to = get_rate(to, table)?;

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

/// Unix 秒 → UTC 日期字符串（YYYY-MM-DD）。
///
/// civil-from-days 算法（Howard Hinnant 公版实现），无新增依赖；用于 er-api 的
/// `time_last_update_unix` 归一。
pub(crate) fn date_from_unix_days(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let (year, month, day) = civil_from_days(days);
    format!("{:04}-{:02}-{:02}", year, month, day)
}

/// 1970-01-01 起的天数 → (年, 月, 日)，含闰年规则。
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let year = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let month = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let year = if month <= 2 { year + 1 } else { year };
    (year, month as u32, day)
}

/// 构造“未知币种”错误，消息含支持币种数量。
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
            source: "frankfurter".to_string(),
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
        let r = convert(100.0, "EUR", "USD", &table).unwrap();
        assert!((r - 108.0).abs() < 1e-9);
    }

    #[test]
    fn test_convert_with_base_to() {
        let table = eur_table();
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
            source: "frankfurter".to_string(),
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
        assert_eq!(cloned.source, table.source);
    }

    #[test]
    fn test_rate_table_debug() {
        let table = eur_table();
        let s = format!("{:?}", table);
        assert!(s.contains("EUR"));
    }

    // ===== date_from_unix_days（er-api 日期归一） =====

    #[test]
    fn test_date_from_unix_days_epoch() {
        assert_eq!(date_from_unix_days(0), "1970-01-01");
    }

    #[test]
    fn test_date_from_unix_days_leap_day() {
        // 2024-02-29T00:00:00Z
        assert_eq!(date_from_unix_days(1_709_164_800), "2024-02-29");
    }

    #[test]
    fn test_date_from_unix_days_year_end() {
        // 2023-12-31T00:00:00Z（跨年边界前一日）
        assert_eq!(date_from_unix_days(1_703_980_800), "2023-12-31");
    }

    #[test]
    fn test_date_from_unix_days_year_start() {
        // 2024-01-01T00:00:00Z
        assert_eq!(date_from_unix_days(1_704_067_200), "2024-01-01");
    }

    #[test]
    fn test_date_from_unix_days_known_timestamp() {
        // 1761234567 → 2025-10-23T07:49:27Z
        assert_eq!(date_from_unix_days(1_761_234_567), "2025-10-23");
    }
}
