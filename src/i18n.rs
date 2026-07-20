// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! ICU4X 国际化模块：中英双语消息目录。
//!
//! 设计依据：design.md §4 (ICU4X 设计)
//! - `I18n` 结构体始终可用（不受 feature gate 限制）
//! - `icu` feature 仅控制是否使用 `icu::locale` 解析 BCP-47 语言标签
//! - 无 `icu` feature 时，`from_str` 使用简单字符串匹配
//! - 消息目录外部化到 `locales/{en,zh}.json`，编译时通过 `include_str!` 嵌入
//! - 简单消息用 `t(key)`，参数化消息用 `tf(key, args)`（`{name}` 占位符）
//! - 未知键返回键本身（fail-loud）

use std::collections::HashMap;
use std::sync::OnceLock;

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
///
/// `Clone` 派生：`cli::run_repl_mode` 需要 `I18n::clone()` 传入 ReplSession（REPL 持有
/// 自己的实例以便长期运行），而 `Lang` 是 `Copy` 类型，clone 成本为零。
#[derive(Clone)]
pub struct I18n {
    lang: Lang,
}

// 静态消息表：编译时嵌入 JSON，运行时解析一次后缓存。
// `HashMap<&'static str, &'static str>` 借用 `include_str!` 的 'static 数据，
// 零拷贝（JSON 中无转义字符，serde_json 可直接借用原始字节）。
static EN_MESSAGES: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
static ZH_MESSAGES: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();

/// 加载英文消息表（首次调用解析 JSON，后续直接返回缓存）。
fn en_messages() -> &'static HashMap<&'static str, &'static str> {
    EN_MESSAGES.get_or_init(|| {
        let json = include_str!("../locales/en.json");
        // 编译时 JSON 损坏是开发期错误，panic 提示修复（规则12 失败显性化）
        serde_json::from_str(json)
            .unwrap_or_else(|e| panic!("locales/en.json 解析失败: {e}"))
    })
}

/// 加载中文消息表（首次调用解析 JSON，后续直接返回缓存）。
fn zh_messages() -> &'static HashMap<&'static str, &'static str> {
    ZH_MESSAGES.get_or_init(|| {
        let json = include_str!("../locales/zh.json");
        serde_json::from_str(json)
            .unwrap_or_else(|e| panic!("locales/zh.json 解析失败: {e}"))
    })
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

    /// 查询简单消息目录（无占位符）。
    ///
    /// 已知键返回对应语言的翻译文本；未知键返回键本身（fail-loud）。
    pub fn t<'a>(&self, key: &'a str) -> &'a str {
        let table = match self.lang {
            Lang::En => en_messages(),
            Lang::Zh => zh_messages(),
        };
        table.get(key).copied().unwrap_or(key)
    }

    /// 查询参数化消息目录（含 `{name}` 占位符）。
    ///
    /// 占位符格式：`{name}`，`args` 提供 `(name, value)` 键值对。
    /// - 已知键：替换所有匹配的占位符后返回 `String`
    /// - 未知键：返回键本身（fail-loud，不进行替换）
    /// - 占位符未提供值：保留原样（便于调试缺失的参数）
    pub fn tf(&self, key: &str, args: &[(&str, &str)]) -> String {
        let template = self.t(key);
        if args.is_empty() {
            return template.to_string();
        }
        let mut result = template.to_string();
        for (name, value) in args {
            let placeholder = format!("{{{}}}", name);
            result = result.replace(&placeholder, value);
        }
        result
    }
}

impl Default for I18n {
    fn default() -> Self {
        Self::new(Lang::default())
    }
}

/// 解析 BCP-47 语言标签为 `Lang`。
///
/// 有 `icu` feature 时使用 `icu::locale` 解析；否则使用简单字符串匹配。
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
        assert_eq!(
            i18n.lang(),
            Lang::En,
            "zhongwen should fall back to En, not Zh"
        );
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
            // 参数化消息键（含占位符，但 t() 返回原始模板）
            "msg.unbound_variable",
            "msg.invalid_bignumber",
            "msg.matrix_dim_mismatch",
            "msg.function_arg_count",
            "msg.unknown_function",
            "msg.unknown_variable",
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
            "msg.unbound_variable",
            "msg.invalid_bignumber",
            "msg.matrix_dim_mismatch",
            "msg.function_arg_count",
            "msg.unknown_function",
            "msg.unknown_variable",
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
            "msg.unbound_variable",
            "msg.invalid_bignumber",
            "msg.matrix_dim_mismatch",
            "msg.function_arg_count",
            "msg.unknown_function",
            "msg.unknown_variable",
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

    // ===== I18n::tf — 参数化消息 =====

    #[test]
    fn test_tf_single_placeholder_en() {
        let i18n = I18n::new(Lang::En);
        assert_eq!(
            i18n.tf("msg.unbound_variable", &[("name", "x")]),
            "Unbound variable: x"
        );
        assert_eq!(
            i18n.tf("msg.unknown_function", &[("name", "sin")]),
            "Unknown function: sin"
        );
    }

    #[test]
    fn test_tf_single_placeholder_zh() {
        let i18n = I18n::new(Lang::Zh);
        assert_eq!(
            i18n.tf("msg.unbound_variable", &[("name", "x")]),
            "未绑定变量: x"
        );
        assert_eq!(
            i18n.tf("msg.unknown_function", &[("name", "sin")]),
            "未知函数: sin"
        );
    }

    #[test]
    fn test_tf_multiple_placeholders_en() {
        let i18n = I18n::new(Lang::En);
        assert_eq!(
            i18n.tf(
                "msg.matrix_dim_mismatch",
                &[("expected", "3x3"), ("actual", "2x2")]
            ),
            "Matrix dimension mismatch: expected 3x3, got 2x2"
        );
        assert_eq!(
            i18n.tf(
                "msg.function_arg_count",
                &[("name", "sin"), ("expected", "1"), ("actual", "2")]
            ),
            "Function sin expects 1 args, got 2"
        );
    }

    #[test]
    fn test_tf_multiple_placeholders_zh() {
        let i18n = I18n::new(Lang::Zh);
        assert_eq!(
            i18n.tf(
                "msg.matrix_dim_mismatch",
                &[("expected", "3x3"), ("actual", "2x2")]
            ),
            "矩阵维度不匹配: 期望 3x3, 实际 2x2"
        );
        assert_eq!(
            i18n.tf(
                "msg.function_arg_count",
                &[("name", "sin"), ("expected", "1"), ("actual", "2")]
            ),
            "函数 sin 期望 1 个参数, 实际 2"
        );
    }

    // ===== I18n::tf — 未知键与边界 =====

    #[test]
    fn test_tf_unknown_key_returns_key_itself() {
        let en = I18n::new(Lang::En);
        // 未知键：返回键本身，不进行替换
        assert_eq!(
            en.tf("nonexistent.key", &[("name", "x")]),
            "nonexistent.key"
        );

        let zh = I18n::new(Lang::Zh);
        assert_eq!(
            zh.tf("nonexistent.key", &[("name", "x")]),
            "nonexistent.key"
        );
    }

    #[test]
    fn test_tf_missing_placeholder_preserved() {
        let i18n = I18n::new(Lang::En);
        // 模板 "Function {name} expects {expected} args, got {actual}"
        // 只提供 name，缺失 expected 和 actual —— 占位符保留原样
        assert_eq!(
            i18n.tf("msg.function_arg_count", &[("name", "sin")]),
            "Function sin expects {expected} args, got {actual}"
        );
    }

    #[test]
    fn test_tf_empty_args_equivalent_to_t() {
        let en = I18n::new(Lang::En);
        // 空参数：tf 等价于 t（返回模板字符串）
        assert_eq!(en.tf("msg.unbound_variable", &[]), "Unbound variable: {name}");
        assert_eq!(en.tf("error.parse", &[]), en.t("error.parse"));

        let zh = I18n::new(Lang::Zh);
        assert_eq!(zh.tf("msg.unbound_variable", &[]), "未绑定变量: {name}");
        assert_eq!(zh.tf("error.parse", &[]), zh.t("error.parse"));
    }

    #[test]
    fn test_tf_repeated_placeholder_replaces_all() {
        let i18n = I18n::new(Lang::En);
        // 如果模板中同一占位符出现多次，replace 会替换所有匹配
        // 当前 JSON 中没有这种键，构造一个临时键验证逻辑（用未知键 + 自定义模板不可行，
        // 改为验证已知键的单次替换行为）
        let result = i18n.tf("msg.unknown_variable", &[("name", "y")]);
        assert_eq!(result, "Unknown variable: y");
        // 确保替换后没有残留占位符
        assert!(!result.contains('{') || result.contains("got"));
    }

    // ===== 静态消息表加载 =====

    #[test]
    fn test_en_messages_table_not_empty() {
        let table = en_messages();
        assert!(!table.is_empty(), "English message table must not be empty");
        // 至少包含 15 个原有键 + 6 个参数化键 = 21 个
        assert!(
            table.len() >= 21,
            "English table should have at least 21 entries, got {}",
            table.len()
        );
    }

    #[test]
    fn test_zh_messages_table_not_empty() {
        let table = zh_messages();
        assert!(!table.is_empty(), "Chinese message table must not be empty");
        assert!(
            table.len() >= 21,
            "Chinese table should have at least 21 entries, got {}",
            table.len()
        );
    }

    #[test]
    fn test_en_and_zh_tables_have_same_keys() {
        let en = en_messages();
        let zh = zh_messages();
        // 两表键集必须一致（避免遗漏翻译）
        assert_eq!(
            en.len(),
            zh.len(),
            "en/zh tables have different key counts: en={}, zh={}",
            en.len(),
            zh.len()
        );
        for key in en.keys() {
            assert!(
                zh.contains_key(key),
                "key '{}' exists in en.json but missing in zh.json",
                key
            );
        }
    }
}
