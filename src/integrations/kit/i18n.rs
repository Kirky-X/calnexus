// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! trait-kit i18n 能力模块：把 trait-kit `I18nFormatter`（ICU4X 数字/日期/
//! 复数/排序格式化）以 kit 模块形态暴露给宿主应用。
//!
//! 定位（playbook C 分层：消息层走 sdforge::i18n，格式化层走 trait-kit
//! i18n）：计算结果的数学表示是 re-entry 契约（输出须可原样回输计算），
//! 本仓自身输出不做本地化分组；宿主 UI（人读面）显示计算结果时经此模块
//! 获得 ICU 格式化能力（千分位分组、本地化小数点、复数类别、排序规则）。

use std::future::Future;
use std::pin::Pin;

use trait_kit::i18n::I18nFormatter;
use trait_kit::prelude::*;

use crate::core::{CalcError, ErrorKind};

/// [`I18nModule`] 构建配置。
#[derive(Debug, Clone, Default)]
pub struct I18nKitConfig {
    /// BCP-47 语言标签（如 `zh-CN` / `en-US`）。空串按 `"en"` 构建。
    pub locale: String,
}

/// ICU 格式化能力句柄：代理 [`I18nFormatter`] 四类格式化方法。
#[derive(Clone)]
pub struct I18nHandle {
    formatter: I18nFormatter,
}

impl I18nHandle {
    /// 按本地化分组格式化数字（如 zh/en 均为 `1,234,567.89`，de 为
    /// `1.234.567,89`）。非有限值（NaN/±inf）返回错误。
    pub fn format_number(&self, value: f64) -> Result<String, CalcError> {
        self.formatter
            .format_number(value)
            .map_err(i18n_error_to_calc)
    }

    /// 按本地化中等长度模式格式化 ISO 日历日期（年/月/日）。
    pub fn format_date(&self, year: i32, month: u8, day: u8) -> Result<String, CalcError> {
        self.formatter
            .format_date(year, month, day)
            .map_err(i18n_error_to_calc)
    }

    /// 该 locale 语言的复数类别名（cardinal 规则；如 en 的 `1` → `"one"`）。
    ///
    /// 返回类别名 `String` 而非枚举：`PluralCategory` 未被 trait-kit 公开
    /// re-export，宿主侧按字符串消费（`one`/`other`/`zero`/`few`/`many`）。
    pub fn plural_category(&self, count: u64) -> Result<String, CalcError> {
        self.formatter
            .plural_category(count)
            .map(|c| c.to_string())
            .map_err(i18n_error_to_calc)
    }

    /// 按该 locale 的排序规则比较两字符串（如中文拼音/笔画序）。
    pub fn compare(&self, a: &str, b: &str) -> Result<std::cmp::Ordering, CalcError> {
        self.formatter
            .compare(a, b)
            .map_err(i18n_error_to_calc)
    }
}

/// trait-kit [`trait_kit::i18n::I18nError`] → [`CalcError`]（单点换算）。
fn i18n_error_to_calc(err: trait_kit::i18n::I18nError) -> CalcError {
    CalcError::new(
        ErrorKind::Eval,
        format!("i18n formatting failed: {err}"),
    )
}

/// i18n 格式化模块（leaf）：构建指定 locale 的 [`I18nFormatter`]。
pub struct I18nModule;

impl ModuleMeta for I18nModule {
    const NAME: &'static str = "calnexus-i18n";
}

impl AsyncAutoBuilder for I18nModule {
    type Capability = I18nHandle;
    type Error = CalcError;

    fn build<'a>(
        kit: &'a AsyncKit,
    ) -> Pin<Box<dyn Future<Output = Result<Self::Capability, Self::Error>> + Send + 'a>> {
        Box::pin(async move {
            // 配置可选：未 set_config 或 locale 为空按 "en"（与 I18n 默认 En 一致）。
            let config: I18nKitConfig = kit.config().unwrap_or_default();
            let locale = if config.locale.is_empty() {
                "en"
            } else {
                config.locale.as_str()
            };
            let formatter = I18nFormatter::new(locale).map_err(|e| {
                CalcError::new(ErrorKind::Eval, format!("invalid locale {locale:?}: {e}"))
            })?;
            Ok(I18nHandle { formatter })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 默认配置 locale 为空串（按 "en" 构建的哨兵，值语义测试在集成测试）。
    #[test]
    fn i18n_kit_config_defaults_to_empty_locale() {
        assert!(I18nKitConfig::default().locale.is_empty());
    }
}
