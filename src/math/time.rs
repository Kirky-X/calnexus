// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! 时间核心函数：日期/时间解析、格式化、算术、日历计算。
//!
//! 从 `domains/time.rs` 提取的纯数学/时间逻辑，
//! 供 `domains/time.rs`（AST 求值路径）和 `api/`（直接 API 路径）共用。
//!
//! 依赖：`jiff` 库类型（Date, DateTime, Zoned, Timestamp, Span, Unit）。
//! Feature 门控：`time = ["dep:jiff"]`。

use crate::core::CalcError;
use jiff::civil::{Date, DateTime};
use jiff::fmt::strtime;
use jiff::tz::TimeZone;
use jiff::{Span, Timestamp, Unit, Zoned};
use std::sync::OnceLock;

// ============================ 日历计算 ============================

/// 闰年判定：能被 4 整除且不能被 100 整除，或能被 400 整除。
pub fn is_leap_year(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

// ============================ 解析 ============================

/// date() 候选格式表（无歧义，年在前或月份为单词）。
///
/// 顺序：ISO 8601 → YYYY/MM/DD → YYYY.MM.DD → YYYYMMDD → 英文月份名 → 中文
const DATE_FORMATS: &[&str] = &[
    "%Y-%m-%d",  // ISO 8601: 2026-07-25
    "%Y/%m/%d",  // 2026/07/25
    "%Y.%m.%d",  // 2026.07.25
    "%Y%m%d",    // 20260725（8 位 basic）
    "%d %b %Y",  // 25 Jul 2026
    "%b %d, %Y", // Jul 25, 2026
    "%B %d, %Y", // July 25, 2026
];

/// datetime() 候选格式表（日期 + 时间部分）。
const DATETIME_FORMATS: &[&str] = &[
    "%Y-%m-%dT%H:%M:%S",  // ISO 8601: 2026-07-25T12:30:00
    "%Y-%m-%dT%H:%M",     // 2026-07-25T12:30
    "%Y-%m-%d %H:%M:%S",  // 2026-07-25 12:30:00（空格分隔）
    "%Y-%m-%d %H:%M",     // 2026-07-25 12:30
    "%Y/%m/%d %H:%M:%S",  // 2026/07/25 12:30:00
    "%Y/%m/%d %H:%M",     // 2026/07/25 12:30
    "%Y%m%dT%H%M%S",      // 20260725T123000
    "%Y.%m.%d %H:%M:%S",  // 2026.07.25 12:30:00
    "%d %b %Y %H:%M:%S",  // 25 Jul 2026 12:30:00
    "%b %d, %Y %H:%M:%S", // Jul 25, 2026 12:30:00
    "%B %d, %Y %H:%M:%S", // July 25, 2026 12:30:00
];

/// 解析日期字符串为 Date（多格式自动识别）。
///
/// 歧义拒绝：纯数字 X/Y/YYYY 或 X-Y-YYYY 形式一律拒绝。
pub fn parse_date_multi_format(input: &str) -> Result<Date, CalcError> {
    // 0. 先尝试 jiff 原生 Date::from_str（ISO 8601 日期）
    if let Ok(d) = input.parse::<Date>() {
        return Ok(d);
    }
    // 1. 歧义检测：纯数字 X/Y/YYYY、X-Y-YYYY、X.Y.YYYY 一律拒绝
    if is_ambiguous_numeric_date(input) {
        return Err(ambiguous_format_error());
    }
    // 2. 中文格式 fallback：2026年7月25日
    if input.contains('年') {
        return parse_chinese_date(input)
            .map_err(|_| invalid_date_error(input));
    }
    // 3. 按候选格式表逐一尝试 strptime
    for fmt in DATE_FORMATS {
        if let Ok(d) = Date::strptime(fmt, input) {
            return Ok(d);
        }
    }
    Err(invalid_date_error(input))
}

/// 解析 datetime 字符串为 DateTime（多格式自动识别）。
pub fn parse_datetime_multi_format(input: &str) -> Result<DateTime, CalcError> {
    // 0. 先尝试 jiff 原生 DateTime::from_str（ISO 8601）
    if let Ok(dt) = input.parse::<DateTime>() {
        return Ok(dt);
    }
    // 1. 歧义检测
    if is_ambiguous_numeric_date(input) {
        return Err(ambiguous_format_error());
    }
    // 2. 中文格式 fallback
    if input.contains('年') {
        return parse_chinese_datetime(input)
            .map_err(|_| invalid_date_error(input));
    }
    // 3. 按候选格式表逐一尝试
    for fmt in DATETIME_FORMATS {
        if let Ok(dt) = DateTime::strptime(fmt, input) {
            return Ok(dt);
        }
    }
    // 4. 退化为纯日期解析（时间为 00:00:00）
    if let Ok(d) = parse_date_multi_format(input) {
        return Ok(DateTime::from(d));
    }
    Err(invalid_date_error(input))
}

/// 用 strptime 尝试解析 timestamp 字符串（带偏移量）。
#[allow(clippy::result_unit_err)]
pub fn parse_timestamp_strptime(input: &str) -> Result<Timestamp, ()> {
    let formats: &[&str] = &[
        "%Y-%m-%dT%H:%M:%S%:z",
        "%Y-%m-%dT%H:%M:%SZ",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%d %H:%M:%S%:z",
        "%Y-%m-%d %H:%M:%S",
    ];
    for fmt in formats {
        if let Ok(ts) = Timestamp::strptime(fmt, input) {
            return Ok(ts);
        }
    }
    Err(())
}

/// 解析字符串为 Zoned（多策略回退）。
///
/// 1. Zoned::from_str：标准 RFC 3339
/// 2. Z 后缀归一化
/// 3. Zoned::strptime：多种常见格式
/// 4. parse_datetime_multi_format + UTC
pub fn parse_str_to_zoned(s: &str) -> Result<Zoned, CalcError> {
    // 1. 标准 RFC 3339
    if let Ok(z) = s.parse::<Zoned>() {
        return Ok(z);
    }
    // 2. Z 后缀归一化为 +00:00
    let normalized = if let Some(stripped) = s.strip_suffix('Z') {
        format!("{}+00:00", stripped)
    } else {
        s.to_string()
    };
    // 3. strptime 常见格式回退
    const ZONED_FORMATS: &[&str] = &[
        "%Y-%m-%dT%H:%M:%S%:z",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%d %H:%M:%S%:z",
        "%Y-%m-%d %H:%M:%S",
    ];
    for fmt in ZONED_FORMATS {
        if let Ok(z) = Zoned::strptime(fmt, &normalized) {
            return Ok(z);
        }
    }
    // 4. 多格式 datetime 解析 + UTC
    let dt = parse_datetime_multi_format(s)?;
    dt.to_zoned(TimeZone::UTC)
        .map_err(|_| invalid_date_error(s))
}

// ============================ 单位解析与 Span 构造 ============================

/// 解析时间单位字符串为 jiff::Unit。
pub fn parse_time_unit(s: &str) -> Result<Unit, CalcError> {
    match s {
        "s" | "second" | "seconds" => Ok(Unit::Second),
        "min" | "minute" | "minutes" => Ok(Unit::Minute),
        "h" | "hour" | "hours" => Ok(Unit::Hour),
        "day" | "days" => Ok(Unit::Day),
        "week" | "weeks" => Ok(Unit::Week),
        "month" | "months" => Ok(Unit::Month),
        "year" | "years" => Ok(Unit::Year),
        _ => Err(CalcError::domain(format!("invalid time unit: {}", s)).with_i18n(
            "msg.time.invalid_unit",
            vec![("unit".to_string(), s.to_string())],
        )),
    }
}

/// 按单位构造 Span（n 为有符号数量）。
pub fn build_span_for_unit(unit: Unit, n: i64) -> Result<Span, CalcError> {
    let span = Span::new();
    match unit {
        Unit::Second => span.try_seconds(n),
        Unit::Minute => span.try_minutes(n),
        Unit::Hour => span.try_hours(n),
        Unit::Day => span.try_days(n),
        Unit::Week => span.try_weeks(n),
        Unit::Month => span.try_months(n),
        Unit::Year => span.try_years(n),
        _ => unreachable!(),
    }
    .map_err(|_| {
        CalcError::domain(format!("span out of range: {} {:?}", n, unit)).with_i18n(
            "msg.time.invalid_unit",
            vec![("unit".to_string(), format!("{:?}", unit))],
        )
    })
}

/// 计算 Span 在指定单位下的总数值（带符号）。
pub fn span_total_in_unit(span: &Span, unit: Unit, a: &Zoned) -> Result<f64, CalcError> {
    let result = match unit {
        Unit::Second => span.total((Unit::Second, a)),
        Unit::Minute => span.total((Unit::Minute, a)),
        Unit::Hour => span.total((Unit::Hour, a)),
        Unit::Day => span.total((Unit::Day, a)),
        Unit::Week => span.total((Unit::Week, a)),
        Unit::Month => span.total((Unit::Month, a)),
        Unit::Year => span.total((Unit::Year, a)),
        _ => unreachable!(),
    };
    result.map_err(|_| {
        CalcError::domain(format!("span total failed for {:?}", unit)).with_i18n(
            "msg.time.invalid_unit",
            vec![("unit".to_string(), format!("{:?}", unit))],
        )
    })
}

// ============================ 日期算术 ============================

/// 计算两个 Zoned 时间在指定单位下的间隔（b - a）。
pub fn compute_date_diff(a: &Zoned, b: &Zoned, unit: Unit) -> Result<f64, CalcError> {
    let span = b.since(a).map_err(|_| {
        CalcError::domain(format!("date_diff failed: {} vs {}", a, b))
            .with_i18n("msg.time.invalid_date", vec![])
    })?;
    span_total_in_unit(&span, unit, a)
}

/// 在 Zoned 时间上加上 n 个指定单位（月末 clamp）。
pub fn compute_date_add(zoned: &Zoned, n: i64, unit: Unit) -> Result<Zoned, CalcError> {
    let span = build_span_for_unit(unit, n)?;
    zoned.checked_add(span).map_err(|_| {
        CalcError::domain(format!("date_add overflow: {} + {} units", zoned, n))
            .with_i18n("msg.time.invalid_date", vec![])
    })
}

// ============================ 格式化 ============================

/// 将 Zoned 转换为 RFC 3339 字符串（用于 EvalResult::DateTime）。
///
/// 使用 strtime %Y-%m-%dT%H:%M:%S%:z 格式。
pub fn zoned_to_rfc3339(zoned: &Zoned) -> String {
    strtime::format("%Y-%m-%dT%H:%M:%S%:z", zoned).unwrap_or_else(|_| zoned.to_string())
}

// ============================ 内部辅助 ============================

/// 检测纯数字 X/Y/YYYY、X-Y-YYYY、X.Y.YYYY 形式（歧义格式）。
fn is_ambiguous_numeric_date(input: &str) -> bool {
    ambiguous_date_regex().is_match(input)
}

/// 中文日期解析：2026年7月25日
fn parse_chinese_date(input: &str) -> Result<Date, ()> {
    if let Ok(d) = Date::strptime("%Y年%m月%d日", input) {
        return Ok(d);
    }
    let caps = chinese_date_regex().captures(input).ok_or(())?;
    let year: i16 = caps[1].parse().map_err(|_| ())?;
    let month: i8 = caps[2].parse().map_err(|_| ())?;
    let day: i8 = caps[3].parse().map_err(|_| ())?;
    Date::new(year, month, day).map_err(|_| ())
}

/// 中文 datetime 解析：2026年7月25日 12:30:00
fn parse_chinese_datetime(input: &str) -> Result<DateTime, ()> {
    if let Ok(dt) = DateTime::strptime("%Y年%m月%d日 %H:%M:%S", input) {
        return Ok(dt);
    }
    if let Ok(dt) = DateTime::strptime("%Y年%m月%d日%H:%M:%S", input) {
        return Ok(dt);
    }
    if let Ok(dt) = DateTime::strptime("%Y年%m月%d日 %H:%M", input) {
        return Ok(dt);
    }
    let date = parse_chinese_date(input)?;
    if let Some(caps) = time_part_regex().captures(input) {
        let hour: i8 = caps[1].parse().map_err(|_| ())?;
        let minute: i8 = caps[2].parse().map_err(|_| ())?;
        let second: i8 = caps.get(3).map_or(0, |m| m.as_str().parse().unwrap_or(0));
        return Ok(date.at(hour, minute, second, 0));
    }
    Ok(DateTime::from(date))
}

// ============================ 错误构造 ============================

/// 构造"无效日期"错误。
pub fn invalid_date_error(value: &str) -> CalcError {
    CalcError::domain(format!("invalid date: {}", value)).with_i18n(
        "msg.time.invalid_date",
        vec![("value".to_string(), value.to_string())],
    )
}

/// 构造"格式不匹配"错误。
pub fn format_mismatch_error(fmt: &str) -> CalcError {
    CalcError::domain(format!("format mismatch: {}", fmt)).with_i18n(
        "msg.time.format_mismatch",
        vec![("fmt".to_string(), fmt.to_string())],
    )
}

/// 构造"歧义格式"错误。
fn ambiguous_format_error() -> CalcError {
    CalcError::domain("ambiguous date format, use parse_date() with explicit format".to_string())
        .with_i18n("msg.time.ambiguous_format", vec![])
}

// ============================ regex 缓存 ============================

/// 获取歧义日期检测 regex（缓存）。
fn ambiguous_date_regex() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"^\d{1,2}[/\-.]\d{1,2}[/\-.]\d{4}$").expect("valid regex"))
}

/// 获取中文日期 regex（缓存）。
fn chinese_date_regex() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"(\d{4})年(\d{1,2})月(\d{1,2})日").expect("valid regex"))
}

/// 获取时间部分 regex（缓存）。
fn time_part_regex() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"(\d{1,2}):(\d{2})(?::(\d{2}))?").expect("valid regex"))
}

// ============================ 单元测试 ============================

#[cfg(test)]
mod tests {
    use super::*;

    // ===== is_leap_year =====

    #[test]
    fn test_is_leap_year_2024() {
        assert!(is_leap_year(2024));
    }

    #[test]
    fn test_is_leap_year_2000() {
        assert!(is_leap_year(2000));
    }

    #[test]
    fn test_is_leap_year_2400() {
        assert!(is_leap_year(2400));
    }

    #[test]
    fn test_is_leap_year_2026() {
        assert!(!is_leap_year(2026));
    }

    #[test]
    fn test_is_leap_year_1900() {
        assert!(!is_leap_year(1900));
    }

    #[test]
    fn test_is_leap_year_2100() {
        assert!(!is_leap_year(2100));
    }

    // ===== parse_date_multi_format =====

    #[test]
    fn test_parse_date_iso() {
        assert!(parse_date_multi_format("2026-07-25").is_ok());
    }

    #[test]
    fn test_parse_date_slash() {
        assert!(parse_date_multi_format("2026/07/25").is_ok());
    }

    #[test]
    fn test_parse_date_dot() {
        assert!(parse_date_multi_format("2026.07.25").is_ok());
    }

    #[test]
    fn test_parse_date_basic_format() {
        assert!(parse_date_multi_format("20260725").is_ok());
    }

    #[test]
    fn test_parse_date_english_abbr() {
        assert!(parse_date_multi_format("25 Jul 2026").is_ok());
    }

    #[test]
    fn test_parse_date_english_abbr2() {
        assert!(parse_date_multi_format("Jul 25, 2026").is_ok());
    }

    #[test]
    fn test_parse_date_english_full() {
        assert!(parse_date_multi_format("July 25, 2026").is_ok());
    }

    #[test]
    fn test_parse_date_chinese() {
        assert!(parse_date_multi_format("2026年7月25日").is_ok());
    }

    #[test]
    fn test_parse_date_ambiguous_rejected() {
        assert!(parse_date_multi_format("01/02/2026").is_err());
        assert!(parse_date_multi_format("13/07/2026").is_err());
        assert!(parse_date_multi_format("01-02-2026").is_err());
        assert!(parse_date_multi_format("01.02.2026").is_err());
    }

    #[test]
    fn test_parse_date_invalid() {
        assert!(parse_date_multi_format("2026-02-30").is_err());
        assert!(parse_date_multi_format("invalid").is_err());
        assert!(parse_date_multi_format("").is_err());
    }

    // ===== parse_datetime_multi_format =====

    #[test]
    fn test_parse_datetime_iso() {
        let dt = parse_datetime_multi_format("2026-07-25T12:30:00").unwrap();
        assert_eq!(dt.hour(), 12);
        assert_eq!(dt.minute(), 30);
    }

    #[test]
    fn test_parse_datetime_space_separated() {
        assert!(parse_datetime_multi_format("2026/07/25 12:30:00").is_ok());
    }

    #[test]
    fn test_parse_datetime_fallback_to_date() {
        // 纯日期字符串应退化为 00:00:00
        let dt = parse_datetime_multi_format("2026-07-25").unwrap();
        assert_eq!(dt.hour(), 0);
    }

    // ===== parse_timestamp_strptime =====

    #[test]
    fn test_parse_timestamp_rfc3339() {
        assert!(parse_timestamp_strptime("2026-07-25T00:00:00Z").is_ok()
            || parse_timestamp_strptime("2026-07-25T00:00:00+00:00").is_ok());
    }

    #[test]
    fn test_parse_timestamp_invalid() {
        assert!(parse_timestamp_strptime("not-a-timestamp").is_err());
        // 纯日期字符串无法解析为 Timestamp（缺时区信息）
        assert!(parse_timestamp_strptime("2026-07-25").is_err());
    }

    // ===== parse_str_to_zoned =====

    #[test]
    fn test_parse_str_to_zoned_rfc3339() {
        let z = parse_str_to_zoned("2026-07-25T00:00:00+00:00").unwrap();
        assert_eq!(z.date().year(), 2026);
    }

    #[test]
    fn test_parse_str_to_zoned_z_suffix() {
        // Z 后缀应被归一化为 +00:00
        let result = parse_str_to_zoned("2026-07-25T00:00:00Z");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_str_to_zoned_date_only() {
        let z = parse_str_to_zoned("2026-07-25").unwrap();
        assert_eq!(z.date().year(), 2026);
        assert_eq!(z.date().month(), 7);
    }

    // ===== parse_time_unit =====

    #[test]
    fn test_parse_time_unit_seconds() {
        assert_eq!(parse_time_unit("s").unwrap(), Unit::Second);
        assert_eq!(parse_time_unit("second").unwrap(), Unit::Second);
        assert_eq!(parse_time_unit("seconds").unwrap(), Unit::Second);
    }

    #[test]
    fn test_parse_time_unit_days() {
        assert_eq!(parse_time_unit("day").unwrap(), Unit::Day);
        assert_eq!(parse_time_unit("days").unwrap(), Unit::Day);
    }

    #[test]
    fn test_parse_time_unit_all() {
        assert_eq!(parse_time_unit("min").unwrap(), Unit::Minute);
        assert_eq!(parse_time_unit("h").unwrap(), Unit::Hour);
        assert_eq!(parse_time_unit("week").unwrap(), Unit::Week);
        assert_eq!(parse_time_unit("month").unwrap(), Unit::Month);
        assert_eq!(parse_time_unit("year").unwrap(), Unit::Year);
    }

    #[test]
    fn test_parse_time_unit_invalid() {
        assert!(parse_time_unit("invalid").is_err());
    }

    // ===== build_span_for_unit =====

    #[test]
    fn test_build_span_days() {
        let span = build_span_for_unit(Unit::Day, 5).unwrap();
        assert_eq!(span.total((Unit::Day, &Zoned::now())).unwrap(), 5.0);
    }

    #[test]
    fn test_build_span_negative() {
        let span = build_span_for_unit(Unit::Hour, -3).unwrap();
        assert_eq!(span.total((Unit::Hour, &Zoned::now())).unwrap(), -3.0);
    }

    // ===== compute_date_diff =====

    #[test]
    fn test_compute_date_diff_days() {
        let a = parse_str_to_zoned("2026-01-01").unwrap();
        let b = parse_str_to_zoned("2026-07-25").unwrap();
        let diff = compute_date_diff(&a, &b, Unit::Day).unwrap();
        assert_eq!(diff, 205.0);
    }

    #[test]
    fn test_compute_date_diff_negative() {
        let a = parse_str_to_zoned("2026-07-25").unwrap();
        let b = parse_str_to_zoned("2026-01-01").unwrap();
        let diff = compute_date_diff(&a, &b, Unit::Day).unwrap();
        assert_eq!(diff, -205.0);
    }

    #[test]
    fn test_compute_date_diff_seconds() {
        let a = parse_str_to_zoned("1970-01-01").unwrap();
        let b = parse_str_to_zoned("1970-01-02").unwrap();
        let diff = compute_date_diff(&a, &b, Unit::Second).unwrap();
        assert_eq!(diff, 86400.0);
    }

    // ===== compute_date_add =====

    #[test]
    fn test_compute_date_add_days() {
        let z = parse_str_to_zoned("2026-01-01").unwrap();
        let result = compute_date_add(&z, 31, Unit::Day).unwrap();
        assert_eq!(result.date().month(), 2);
        assert_eq!(result.date().day(), 1);
    }

    #[test]
    fn test_compute_date_add_month_clamp() {
        // 2026-01-31 + 1 month = 2026-02-28（非闰年 clamp）
        let z = parse_str_to_zoned("2026-01-31").unwrap();
        let result = compute_date_add(&z, 1, Unit::Month).unwrap();
        assert_eq!(result.date().month(), 2);
        assert_eq!(result.date().day(), 28);
    }

    #[test]
    fn test_compute_date_add_month_leap_year() {
        // 2024-01-31 + 1 month = 2024-02-29（闰年 clamp）
        let z = parse_str_to_zoned("2024-01-31").unwrap();
        let result = compute_date_add(&z, 1, Unit::Month).unwrap();
        assert_eq!(result.date().month(), 2);
        assert_eq!(result.date().day(), 29);
    }

    #[test]
    fn test_compute_date_add_negative() {
        let z = parse_str_to_zoned("2026-07-25").unwrap();
        let result = compute_date_add(&z, -1, Unit::Day).unwrap();
        assert_eq!(result.date().day(), 24);
    }

    // ===== zoned_to_rfc3339 =====

    #[test]
    fn test_zoned_to_rfc3339_utc() {
        let z = parse_str_to_zoned("2026-07-25T12:30:00+00:00").unwrap();
        let s = zoned_to_rfc3339(&z);
        assert_eq!(s, "2026-07-25T12:30:00+00:00");
    }

    #[test]
    fn test_zoned_to_rfc3339_roundtrip() {
        let original = "2026-07-25T12:30:00+00:00";
        let z = parse_str_to_zoned(original).unwrap();
        let s = zoned_to_rfc3339(&z);
        assert_eq!(s, original);
    }

    // ===== format_mismatch_error =====

    #[test]
    fn test_format_mismatch_error_has_i18n() {
        let err = format_mismatch_error("%Y-%m-%d");
        assert_eq!(err.i18n_key, Some("msg.time.format_mismatch"));
    }
}
