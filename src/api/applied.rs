// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! AppliedMath trait 实现。

use crate::api::CalNexus;
#[cfg(any(feature = "time", feature = "unit", feature = "fx"))]
use crate::core::{CalcError, EvalResult};

/// AppliedMath API 访问器。
pub struct AppliedMathImpl<'a> {
    #[allow(dead_code)]
    pub(crate) cn: &'a CalNexus,
}

// ── 辅助：时区解析 ──

#[cfg(feature = "time")]
fn resolve_tz(tz: Option<&str>) -> Result<jiff::tz::TimeZone, CalcError> {
    match tz {
        None | Some("UTC") => Ok(jiff::tz::TimeZone::UTC),
        Some(name) => jiff::tz::TimeZone::get(name).map_err(|_| {
            CalcError::domain(format!("unknown timezone: {}", name)).with_i18n(
                "msg.time.unknown_timezone",
                vec![("tz".to_string(), name.to_string())],
            )
        }),
    }
}

impl<'a> AppliedMathImpl<'a> {
    // ═══════════════════════════════════════════════════════════════
    //  时间：构造
    // ═══════════════════════════════════════════════════════════════

    #[cfg(feature = "time")]
    pub fn date(&self, date_str: &str) -> Result<EvalResult, CalcError> {
        let d = crate::math::time::parse_date_multi_format(date_str)?;
        let zoned = d
            .to_zoned(jiff::tz::TimeZone::UTC)
            .map_err(|_| crate::math::time::invalid_date_error(date_str))?;
        Ok(EvalResult::DateTime(crate::math::time::zoned_to_rfc3339(&zoned)))
    }

    #[cfg(feature = "time")]
    pub fn datetime(
        &self,
        datetime_str: &str,
        tz: Option<&str>,
    ) -> Result<EvalResult, CalcError> {
        let tz = resolve_tz(tz)?;
        let dt = crate::math::time::parse_datetime_multi_format(datetime_str)?;
        let zoned = dt
            .to_zoned(tz)
            .map_err(|_| crate::math::time::invalid_date_error(datetime_str))?;
        Ok(EvalResult::DateTime(crate::math::time::zoned_to_rfc3339(&zoned)))
    }

    #[cfg(feature = "time")]
    pub fn timestamp(&self, datetime_str: &str) -> Result<EvalResult, CalcError> {
        let zoned = crate::math::time::parse_str_to_zoned(datetime_str)?;
        Ok(EvalResult::Scalar(zoned.timestamp().as_second() as f64))
    }

    #[cfg(feature = "time")]
    pub fn from_timestamp(
        &self,
        secs: i64,
        tz: Option<&str>,
    ) -> Result<EvalResult, CalcError> {
        let tz = resolve_tz(tz)?;
        let ts = jiff::Timestamp::from_second(secs).map_err(|_| {
            CalcError::domain(format!("timestamp out of range: {}", secs)).with_i18n(
                "msg.time.invalid_date",
                vec![("value".to_string(), secs.to_string())],
            )
        })?;
        let zoned = ts.to_zoned(tz);
        Ok(EvalResult::DateTime(crate::math::time::zoned_to_rfc3339(&zoned)))
    }

    // ═══════════════════════════════════════════════════════════════
    //  时间：算术
    // ═══════════════════════════════════════════════════════════════

    #[cfg(feature = "time")]
    pub fn now(&self, tz: Option<&str>) -> Result<EvalResult, CalcError> {
        let tz = resolve_tz(tz)?;
        let zoned = jiff::Zoned::now().with_time_zone(tz);
        Ok(EvalResult::DateTime(crate::math::time::zoned_to_rfc3339(&zoned)))
    }

    #[cfg(feature = "time")]
    pub fn today(&self, tz: Option<&str>) -> Result<EvalResult, CalcError> {
        let tz = resolve_tz(tz)?;
        let now = jiff::Zoned::now().with_time_zone(tz.clone());
        let today = now.date().to_zoned(tz).map_err(|_| {
            CalcError::domain("failed to construct today midnight".to_string())
                .with_i18n("msg.time.invalid_date", vec![])
        })?;
        Ok(EvalResult::DateTime(crate::math::time::zoned_to_rfc3339(&today)))
    }

    #[cfg(feature = "time")]
    pub fn date_add(
        &self,
        date: &str,
        n: i64,
        unit: &str,
    ) -> Result<EvalResult, CalcError> {
        let zoned = crate::math::time::parse_str_to_zoned(date)?;
        let unit = crate::math::time::parse_time_unit(unit)?;
        let result = crate::math::time::compute_date_add(&zoned, n, unit)?;
        Ok(EvalResult::DateTime(crate::math::time::zoned_to_rfc3339(&result)))
    }

    #[cfg(feature = "time")]
    pub fn date_diff(
        &self,
        a: &str,
        b: &str,
        unit: Option<&str>,
    ) -> Result<EvalResult, CalcError> {
        let za = crate::math::time::parse_str_to_zoned(a)?;
        let zb = crate::math::time::parse_str_to_zoned(b)?;
        let unit_str = unit.unwrap_or("day");
        let unit = crate::math::time::parse_time_unit(unit_str)?;
        let result = crate::math::time::compute_date_diff(&za, &zb, unit)?;
        Ok(EvalResult::Scalar(result))
    }

    // ═══════════════════════════════════════════════════════════════
    //  时间：格式与日历
    // ═══════════════════════════════════════════════════════════════

    #[cfg(feature = "time")]
    pub fn format_date(
        &self,
        date: &str,
        fmt: &str,
        tz: Option<&str>,
    ) -> Result<EvalResult, CalcError> {
        let zoned = crate::math::time::parse_str_to_zoned(date)?;
        let zoned = match tz {
            Some(tz_name) => {
                let tz = resolve_tz(Some(tz_name))?;
                zoned.with_time_zone(tz)
            }
            None => zoned,
        };
        let formatted = jiff::fmt::strtime::format(fmt, &zoned)
            .map_err(|_| crate::math::time::format_mismatch_error(fmt))?;
        Ok(EvalResult::Symbolic(formatted))
    }

    #[cfg(feature = "time")]
    pub fn reformat_date(
        &self,
        input: &str,
        from_fmt: &str,
        to_fmt: &str,
    ) -> Result<EvalResult, CalcError> {
        // 解析阶段：先 Date 后 Zoned
        let zoned = match jiff::civil::Date::strptime(from_fmt, input) {
            Ok(date) => date
                .to_zoned(jiff::tz::TimeZone::UTC)
                .map_err(|_| crate::math::time::invalid_date_error(input))?,
            Err(_) => match jiff::Zoned::strptime(from_fmt, input) {
                Ok(z) => z,
                Err(_) => return Err(crate::math::time::format_mismatch_error(from_fmt)),
            },
        };
        // 格式化阶段
        let formatted = jiff::fmt::strtime::format(to_fmt, &zoned)
            .map_err(|_| crate::math::time::format_mismatch_error(to_fmt))?;
        Ok(EvalResult::Symbolic(formatted))
    }

    #[cfg(feature = "time")]
    pub fn weekday(&self, date: &str) -> Result<EvalResult, CalcError> {
        let d = crate::math::time::parse_date_multi_format(date)?;
        Ok(EvalResult::Scalar(d.weekday().to_monday_one_offset() as f64))
    }

    #[cfg(feature = "time")]
    pub fn day_of_year(&self, date: &str) -> Result<EvalResult, CalcError> {
        let d = crate::math::time::parse_date_multi_format(date)?;
        Ok(EvalResult::Scalar(d.day_of_year() as f64))
    }

    #[cfg(feature = "time")]
    pub fn is_leap_year(&self, year: i64) -> Result<EvalResult, CalcError> {
        Ok(EvalResult::Scalar(if crate::math::time::is_leap_year(year) {
            1.0
        } else {
            0.0
        }))
    }

    // ═══════════════════════════════════════════════════════════════
    //  单位换算
    // ═══════════════════════════════════════════════════════════════

    #[cfg(feature = "unit")]
    pub fn convert(
        &self,
        value: f64,
        from: &str,
        to: &str,
    ) -> Result<EvalResult, CalcError> {
        crate::math::unit::convert_value(value, from, to).map(EvalResult::Scalar)
    }

    // ═══════════════════════════════════════════════════════════════
    //  汇率换算
    // ═══════════════════════════════════════════════════════════════

    #[cfg(feature = "fx")]
    pub fn fx(
        &self,
        amount: f64,
        from: &str,
        to: &str,
    ) -> Result<EvalResult, CalcError> {
        use crate::domains::fx_provider::{FrankfurterProvider, RateProvider};
        let provider = FrankfurterProvider::default();
        let table = provider.rates()?;
        crate::math::fx::convert(amount, from, to, &table).map(EvalResult::Scalar)
    }

    #[cfg(feature = "fx")]
    pub fn fx_rate(
        &self,
        from: &str,
        to: &str,
    ) -> Result<EvalResult, CalcError> {
        use crate::domains::fx_provider::{FrankfurterProvider, RateProvider};
        let provider = FrankfurterProvider::default();
        let table = provider.rates()?;
        let rate_from = crate::math::fx::get_rate(from, &table)?;
        let rate_to = crate::math::fx::get_rate(to, &table)?;
        Ok(EvalResult::Scalar(rate_to / rate_from))
    }
}
