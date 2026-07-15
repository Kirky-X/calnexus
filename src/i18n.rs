// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! ICU4X 国际化模块：中英双语错误消息。
//!
//! 设计依据：design.md §4 (ICU4X 设计)
//! - `I18n` 结构体始终可用（不受 feature gate 限制）
//! - `icu` feature 仅控制是否使用 `icu::locale` 解析 BCP-47 语言标签
//! - 无 `icu` feature 时，`from_str` 使用简单字符串匹配
//! - 消息目录使用 match 表，未知键返回键本身（fail-loud）

/// 支持的语言。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lang {
    /// 英语（默认回退语言）
    #[default]
    En,
    /// 中文
    Zh,
}

/// 国际化上下文：持有当前语言，提供消息目录查询。
pub struct I18n {
    lang: Lang,
}

impl I18n {
    /// 创建指定语言的 i18n 上下文。
    pub fn new(lang: Lang) -> Self {
        Self { lang }
    }

    /// 从 BCP-47 语言标签字符串解析语言。
    ///
    /// - "en"/"en-US"/"en-GB" → `Lang::En`
    /// - "zh"/"zh-CN"/"zh-TW" → `Lang::Zh`
    /// - 未知语言 → 回退 `Lang::En`
    ///
    /// 注意：不实现 `std::str::FromStr`，因为此方法不会失败（总是回退到 En），
    /// 而 `FromStr` 要求返回 `Result`。
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        let lang = parse_lang(s);
        Self::new(lang)
    }

    /// 获取当前语言。
    pub fn lang(&self) -> Lang {
        self.lang
    }

    /// 查询消息目录。
    ///
    /// 已知键返回对应语言的翻译文本；未知键返回键本身（fail-loud）。
    pub fn t<'a>(&self, key: &'a str) -> &'a str {
        match (key, self.lang) {
            // error.parse
            ("error.parse", Lang::En) => "Parse error",
            ("error.parse", Lang::Zh) => "解析错误",
            // error.eval
            ("error.eval", Lang::En) => "Evaluation error",
            ("error.eval", Lang::Zh) => "求值错误",
            // error.overflow
            ("error.overflow", Lang::En) => "Arithmetic overflow",
            ("error.overflow", Lang::Zh) => "算术溢出",
            // error.division_by_zero
            ("error.division_by_zero", Lang::En) => "Division by zero",
            ("error.division_by_zero", Lang::Zh) => "除以零",
            // error.domain
            ("error.domain", Lang::En) => "Domain error",
            ("error.domain", Lang::Zh) => "定义域错误",
            // error.depth
            ("error.depth", Lang::En) => "Maximum recursion depth exceeded",
            ("error.depth", Lang::Zh) => "超过最大递归深度",
            // error.nan_or_inf
            ("error.nan_or_inf", Lang::En) => "Result is NaN or infinity",
            ("error.nan_or_inf", Lang::Zh) => "结果为 NaN 或无穷大",
            // error.undefined_symbol
            ("error.undefined_symbol", Lang::En) => "Undefined symbol",
            ("error.undefined_symbol", Lang::Zh) => "未定义符号",
            // error.timeout
            ("error.timeout", Lang::En) => "Evaluation timed out",
            ("error.timeout", Lang::Zh) => "求值超时",
            // error.usage
            ("error.usage", Lang::En) => "Usage error",
            ("error.usage", Lang::Zh) => "用法错误",
            // label.position — friendly()/to_explain() 中 Span 位置标签
            ("label.position", Lang::En) => "Position",
            ("label.position", Lang::Zh) => "位置",
            // label.hint — friendly()/to_explain() 中提示标签
            ("label.hint", Lang::En) => "Hint",
            ("label.hint", Lang::Zh) => "提示",
            // label.error_kind — to_explain() 中错误类别标签
            ("label.error_kind", Lang::En) => "Error Kind",
            ("label.error_kind", Lang::Zh) => "错误类别",
            // label.exit_code — to_explain() 中退出码标签
            ("label.exit_code", Lang::En) => "Exit Code",
            ("label.exit_code", Lang::Zh) => "退出码",
            // label.suggestion — to_explain() 中建议标签
            ("label.suggestion", Lang::En) => "Suggestion",
            ("label.suggestion", Lang::Zh) => "建议",
            // 未知键：返回键本身（fail-loud）
            _ => key,
        }
    }
}

impl Default for I18n {
    fn default() -> Self {
        Self::new(Lang::default())
    }
}

/// 解析 BCP-47 语言标签为 `Lang`。
///
/// 有 `icu` feature 时使用 `icu::locid::Locale` 解析；否则使用简单字符串匹配。
fn parse_lang(s: &str) -> Lang {
    #[cfg(feature = "icu")]
    {
        parse_lang_icu(s)
    }
    #[cfg(not(feature = "icu"))]
    {
        parse_lang_simple(s)
    }
}

/// 使用 `icu::locale` 解析 BCP-47 语言标签。
#[cfg(feature = "icu")]
fn parse_lang_icu(s: &str) -> Lang {
    use icu::locale::Locale;

    match Locale::try_from_str(s) {
        Ok(locale) => {
            let lang = locale.id.language;
            if lang.as_str() == "zh" {
                Lang::Zh
            } else {
                Lang::En
            }
        }
        Err(_) => Lang::En,
    }
}

/// 使用简单字符串匹配解析语言标签（无 `icu` feature 时的回退）。
///
/// 仅精确匹配 `zh`、`zh-CN`、`zh-TW`（忽略大小写），避免 `starts_with("zh")`
/// 误匹配 `zhongwen` 等字符串。
#[cfg(not(feature = "icu"))]
fn parse_lang_simple(s: &str) -> Lang {
    let lower = s.to_ascii_lowercase();
    match lower.as_str() {
        "zh" | "zh-cn" | "zh-tw" => Lang::Zh,
        _ => Lang::En,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== Lang 默认值 =====

    #[test]
    fn test_lang_default_is_english() {
        assert_eq!(Lang::default(), Lang::En);
    }

    // ===== I18n::new =====

    #[test]
    fn test_new_creates_with_specified_lang() {
        let en = I18n::new(Lang::En);
        assert_eq!(en.lang(), Lang::En);

        let zh = I18n::new(Lang::Zh);
        assert_eq!(zh.lang(), Lang::Zh);
    }

    #[test]
    fn test_default_is_english() {
        let i18n = I18n::default();
        assert_eq!(i18n.lang(), Lang::En);
    }

    // ===== I18n::from_str — 基本解析 =====

    #[test]
    fn test_from_str_en() {
        assert_eq!(I18n::from_str("en").lang(), Lang::En);
    }

    #[test]
    fn test_from_str_zh() {
        assert_eq!(I18n::from_str("zh").lang(), Lang::Zh);
    }

    // ===== I18n::from_str — BCP-47 变体 =====

    #[test]
    fn test_from_str_en_us() {
        assert_eq!(I18n::from_str("en-US").lang(), Lang::En);
    }

    #[test]
    fn test_from_str_en_gb() {
        assert_eq!(I18n::from_str("en-GB").lang(), Lang::En);
    }

    #[test]
    fn test_from_str_zh_cn() {
        assert_eq!(I18n::from_str("zh-CN").lang(), Lang::Zh);
    }

    #[test]
    fn test_from_str_zh_tw() {
        assert_eq!(I18n::from_str("zh-TW").lang(), Lang::Zh);
    }

    // ===== I18n::from_str — 未知语言回退 =====

    #[test]
    fn test_from_str_french_fallback_to_en() {
        assert_eq!(I18n::from_str("fr").lang(), Lang::En);
    }

    #[test]
    fn test_from_str_japanese_fallback_to_en() {
        assert_eq!(I18n::from_str("ja").lang(), Lang::En);
    }

    #[test]
    fn test_from_str_empty_string_fallback_to_en() {
        assert_eq!(I18n::from_str("").lang(), Lang::En);
    }

    #[test]
    fn test_from_str_invalid_string_fallback_to_en() {
        assert_eq!(I18n::from_str("!!!").lang(), Lang::En);
    }

    /// T010 Red: "zhongwen" 以 "zh" 开头但不是有效的中文语言代码，应回退到 En
    #[test]
    fn test_from_str_zhongwen_falls_back_to_en() {
        let i18n = I18n::from_str("zhongwen");
        assert_eq!(i18n.lang(), Lang::En, "zhongwen should fall back to En, not Zh");
    }

    // ===== I18n::from_str — 大小写不敏感 =====

    #[test]
    fn test_from_str_case_insensitive_zh() {
        assert_eq!(I18n::from_str("ZH").lang(), Lang::Zh);
        assert_eq!(I18n::from_str("Zh").lang(), Lang::Zh);
    }

    #[test]
    fn test_from_str_case_insensitive_en() {
        assert_eq!(I18n::from_str("EN").lang(), Lang::En);
        assert_eq!(I18n::from_str("En").lang(), Lang::En);
    }

    // ===== I18n::t — 全消息键中英双语 =====

    #[test]
    fn test_all_message_keys_have_english_translation() {
        let i18n = I18n::new(Lang::En);
        let keys = [
            "error.parse",
            "error.eval",
            "error.overflow",
            "error.division_by_zero",
            "error.domain",
            "error.depth",
            "error.nan_or_inf",
            "error.undefined_symbol",
            "error.timeout",
            "error.usage",
            "label.position",
            "label.hint",
            "label.error_kind",
            "label.exit_code",
            "label.suggestion",
        ];
        for key in &keys {
            let msg = i18n.t(key);
            assert!(
                msg != *key,
                "key '{}' has no English translation (returned key itself)",
                key
            );
            assert!(
                !msg.is_empty(),
                "key '{}' has empty English translation",
                key
            );
        }
    }

    #[test]
    fn test_all_message_keys_have_chinese_translation() {
        let i18n = I18n::new(Lang::Zh);
        let keys = [
            "error.parse",
            "error.eval",
            "error.overflow",
            "error.division_by_zero",
            "error.domain",
            "error.depth",
            "error.nan_or_inf",
            "error.undefined_symbol",
            "error.timeout",
            "error.usage",
            "label.position",
            "label.hint",
            "label.error_kind",
            "label.exit_code",
            "label.suggestion",
        ];
        for key in &keys {
            let msg = i18n.t(key);
            assert!(
                msg != *key,
                "key '{}' has no Chinese translation (returned key itself)",
                key
            );
            assert!(
                !msg.is_empty(),
                "key '{}' has empty Chinese translation",
                key
            );
        }
    }

    // ===== I18n::t — 中英翻译不同 =====

    #[test]
    fn test_en_and_zh_translations_differ() {
        let en = I18n::new(Lang::En);
        let zh = I18n::new(Lang::Zh);
        let keys = [
            "error.parse",
            "error.eval",
            "error.overflow",
            "error.division_by_zero",
            "error.domain",
            "error.depth",
            "error.nan_or_inf",
            "error.undefined_symbol",
            "error.timeout",
            "error.usage",
            "label.position",
            "label.hint",
            "label.error_kind",
            "label.exit_code",
            "label.suggestion",
        ];
        for key in &keys {
            assert_ne!(
                en.t(key),
                zh.t(key),
                "key '{}' has identical en/zh translations",
                key
            );
        }
    }

    // ===== I18n::t — 未知键 fail-loud =====

    #[test]
    fn test_unknown_key_returns_key_itself() {
        let en = I18n::new(Lang::En);
        assert_eq!(en.t("nonexistent.key"), "nonexistent.key");

        let zh = I18n::new(Lang::Zh);
        assert_eq!(zh.t("nonexistent.key"), "nonexistent.key");
    }

    #[test]
    fn test_empty_key_returns_empty_string() {
        let en = I18n::new(Lang::En);
        assert_eq!(en.t(""), "");
    }

    // ===== I18n::t — 具体翻译内容验证 =====

    #[test]
    fn test_specific_english_translations() {
        let en = I18n::new(Lang::En);
        assert_eq!(en.t("error.parse"), "Parse error");
        assert_eq!(en.t("error.eval"), "Evaluation error");
        assert_eq!(en.t("error.overflow"), "Arithmetic overflow");
        assert_eq!(en.t("error.division_by_zero"), "Division by zero");
        assert_eq!(en.t("error.domain"), "Domain error");
        assert_eq!(en.t("error.depth"), "Maximum recursion depth exceeded");
        assert_eq!(en.t("error.nan_or_inf"), "Result is NaN or infinity");
        assert_eq!(en.t("error.undefined_symbol"), "Undefined symbol");
        assert_eq!(en.t("error.timeout"), "Evaluation timed out");
        assert_eq!(en.t("error.usage"), "Usage error");
        // 5 个标签键（T002 diting HIGH-1 修复）
        assert_eq!(en.t("label.position"), "Position");
        assert_eq!(en.t("label.hint"), "Hint");
        assert_eq!(en.t("label.error_kind"), "Error Kind");
        assert_eq!(en.t("label.exit_code"), "Exit Code");
        assert_eq!(en.t("label.suggestion"), "Suggestion");
    }

    #[test]
    fn test_specific_chinese_translations() {
        let zh = I18n::new(Lang::Zh);
        assert_eq!(zh.t("error.parse"), "解析错误");
        assert_eq!(zh.t("error.eval"), "求值错误");
        assert_eq!(zh.t("error.overflow"), "算术溢出");
        assert_eq!(zh.t("error.division_by_zero"), "除以零");
        assert_eq!(zh.t("error.domain"), "定义域错误");
        assert_eq!(zh.t("error.depth"), "超过最大递归深度");
        assert_eq!(zh.t("error.nan_or_inf"), "结果为 NaN 或无穷大");
        assert_eq!(zh.t("error.undefined_symbol"), "未定义符号");
        assert_eq!(zh.t("error.timeout"), "求值超时");
        assert_eq!(zh.t("error.usage"), "用法错误");
        // 5 个标签键（T002 diting HIGH-1 修复）
        assert_eq!(zh.t("label.position"), "位置");
        assert_eq!(zh.t("label.hint"), "提示");
        assert_eq!(zh.t("label.error_kind"), "错误类别");
        assert_eq!(zh.t("label.exit_code"), "退出码");
        assert_eq!(zh.t("label.suggestion"), "建议");
    }
}
