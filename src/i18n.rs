// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! ICU4X 国际化模块：中英双语消息目录（sdforge::i18n 宿主注册基座）。
//!
//! 消息真源仍是 `locales/{en,zh}/messages.ftl`（编译期内嵌）；目录经启动期
//! 解析注册进 sdforge 宿主注册表（库层唯一 i18n 基座，宿主注册优先于内置
//! 目录）。`icu` feature 仅控制 BCP-47 语言标签的解析实现；未知键返回键名
//! 本身（fail-loud，不 panic）。
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
#[derive(Clone)]
pub struct I18n {
    lang: Lang,
}

impl I18n {
    /// 创建指定语言的 i18n 上下文。
    pub fn new(lang: Lang) -> Self {
        Self { lang }
    }

    /// 从 BCP-47 语言标签字符串解析语言（用户显式输入路径），未知语言回退 En。
    ///
    /// 不实现 `FromStr`：此方法不会失败，而 trait 要求返回 `Result`。
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        let lang = parse_lang(s);
        Self::new(lang)
    }

    /// 从系统语言检测链构造（检测结果进程内 `OnceLock` 缓存）；用于无显式语言的入口。
    pub fn from_detected() -> Self {
        Self::new(detect_locale())
    }

    /// 获取当前语言。
    pub fn lang(&self) -> Lang {
        self.lang
    }

    /// 查询简单消息目录（无参数占位符）；未知键返回键本身。
    pub fn t(&self, key: &str) -> String {
        self.tf(key, &[])
    }

    /// 查询参数化消息目录（`{ $name }` 占位符）。
    ///
    /// 查找顺序（sdforge `translate_for`）：当前语言宿主注册 → 英文宿主注册 →
    /// 键名本身；缺参按 Fluent 语义回显紧凑 `{$name}`（注册模板即紧凑形态）。
    pub fn tf(&self, key: &str, args: &[(&str, &str)]) -> String {
        ensure_registered();
        let locale = match self.lang {
            Lang::En => "en",
            Lang::Zh => "zh",
        };
        let owned: Vec<(&str, String)> = args
            .iter()
            .map(|(name, value)| (*name, (*value).to_string()))
            .collect();
        sdforge::i18n::translate_for(locale, key, &owned)
    }
}

impl Default for I18n {
    fn default() -> Self {
        Self::new(Lang::default())
    }
}

// ============================================================================
// 系统语言检测链
// ============================================================================

/// 检测结果进程内缓存（首调用确定，此后不变；并行测试安全）。
static DETECTED_LANG: OnceLock<Lang> = OnceLock::new();

/// 系统语言检测（强制顺序）：`CALNEXUS_LANG` → `LC_ALL` → `LC_MESSAGES` →
/// `LANG` → sys-locale → en；任何失败/未知语言继续走链，链尾必为 En。
pub(crate) fn detect_locale() -> Lang {
    *DETECTED_LANG.get_or_init(|| detect_from(|key| std::env::var(key).ok()))
}

/// 检测链纯函数：env 读取经 `lookup` 注入，便于无环境副作用的测试。
fn detect_from(lookup: impl Fn(&str) -> Option<String>) -> Lang {
    detect_from_with_sys(lookup, sys_locale::get_locale)
}

/// 同 `detect_from`，系统区域探测步可注入：「回退到 En」的断言依赖该步结果，
/// 不注入则在系统区域为 zh 的机器上必然失败。
fn detect_from_with_sys(
    lookup: impl Fn(&str) -> Option<String>,
    sys_lookup: impl Fn() -> Option<String>,
) -> Lang {
    // 1. 项目覆盖变量（大写项目名）
    if let Some(value) = lookup("CALNEXUS_LANG")
        && let Some(lang) = normalize_env_value(&value)
    {
        return lang;
    }
    // 2. 显式 POSIX 环境链（显式读以在 Windows/边缘环境获得确定性）
    for key in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Some(value) = lookup(key)
            && let Some(lang) = normalize_env_value(&value)
        {
            return lang;
        }
    }
    // 3. sys-locale 系统探测
    if let Some(locale) = sys_lookup()
        && let Some(lang) = normalize_env_value(&locale)
    {
        return lang;
    }
    // 4. 终极回退
    Lang::En
}

/// 归一化单个环境变量值：剥离 `@modifier` 与 `.codeset`、`_` 换 `-`，取 primary
/// subtag 映射 Zh/En；`C`/`POSIX`/空值/不支持语言返回 `None`（回退链继续）。
fn normalize_env_value(raw: &str) -> Option<Lang> {
    let trimmed = raw.trim();
    let no_modifier = trimmed.split('@').next().unwrap_or("");
    let no_codeset = no_modifier.split('.').next().unwrap_or("");
    let tag = no_codeset.replace('_', "-");
    if matches!(tag.as_str(), "" | "C" | "POSIX") {
        return None;
    }
    // 取 primary subtag；用 split('-') 而非 starts_with("zh")，避免误匹配 "zhongwen"
    let primary = tag.split('-').next().unwrap_or("").to_ascii_lowercase();
    match primary.as_str() {
        "zh" => Some(Lang::Zh),
        "en" => Some(Lang::En),
        _ => None,
    }
}

// ============================================================================
// 消息目录（sdforge::i18n 宿主注册）
// ============================================================================

/// 英文目录（编译期内嵌磁盘文件，不可能失同步）。
const EN_FTL: &str = include_str!("../locales/en/messages.ftl");
/// 中文目录（编译期内嵌磁盘文件，不可能失同步）。
const ZH_FTL: &str = include_str!("../locales/zh/messages.ftl");

/// 注册单次执行守卫（注册幂等，守卫只为省去重复解析）。
static REGISTERED: OnceLock<()> = OnceLock::new();

/// 把双 locale FTL 目录注册进 sdforge 宿主注册表（单次执行，幂等）。
///
/// sdforge 查找顺序：宿主注册（最高优先）→ 内置 en 目录 → 键本身；本仓全量
/// 键经宿主注册后，查找行为为「当前语言 → en → 键名本身」，与切换前的双束
/// 实现一致。键位还原：FTL 标识符不允许 `.`，本仓约定调用面点分键 ↔ FTL
/// dash 标识符一一对应（见 FTL 头注释），注册前 dash 还原为点分。
pub fn ensure_registered() {
    REGISTERED.get_or_init(|| {
        register_locale("en", EN_FTL);
        register_locale("zh", ZH_FTL);
    });
}

/// 注册单 locale 目录条目。
fn register_locale(locale: &str, ftl: &'static str) {
    for (id, value) in parse_entries(ftl) {
        sdforge::i18n::register_translation(locale, &id.replace('-', "."), &value);
    }
}

/// 解析单行 `key = value` 条目（跳过空行/`#` 注释），值端经 [`template_value`]
/// 桥接归一为 sdforge 模板。目录约定纯单行形态（守卫测试保证），无多行延续值。
fn parse_entries(ftl: &'static str) -> impl Iterator<Item = (&'static str, String)> {
    ftl.lines().filter_map(|line| {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            return None;
        }
        let (id, raw_value) = trimmed.split_once('=')?;
        let id = id.trim();
        if id.is_empty() {
            return None;
        }
        Some((id, template_value(raw_value)))
    })
}

/// FTL 值 → sdforge 注册模板，两处桥接归一：
///
/// 1. 全包字符串字面量 `{ "..." }` 还原为字面文本：FTL 引号字符串保留首尾
///    空白（如 `label-cached_suffix = { " (cached)" }` 的前导空格），字面量
///    内部不做占位符处理（Fluent 语义：字面量内容原样输出）。
/// 2. 其余值把 `{ $name }` 占位符规范为紧凑 `{$name}`：sdforge 缺参时原样
///    回显模板占位符，紧凑形态与 Fluent 缺参回显形态一致，保证切换前后渲染
///    逐字节等价（sdforge kit 桥接 `host_to_kit_template` 同款归一）。
fn template_value(raw: &str) -> String {
    let value = raw.trim();
    if let Some(inner) = unwrap_string_literal(value) {
        return inner;
    }
    compact_placeholders(value)
}

/// 全包字符串字面量还原：`{ "..." }` → 内部文本（`\"`→`"`、`\\`→`\` 转义还原）。
fn unwrap_string_literal(value: &str) -> Option<String> {
    let inner = value.strip_prefix("{ \"")?.strip_suffix("\" }")?;
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    Some(out)
}

/// 占位符紧凑化：`{ $name }` → `{$name}`；非 `$` 占位符的大括号原样保留
/// （推进一格继续扫描，花括号内容不丢——sdforge `format_template` 同语义）。
fn compact_placeholders(value: &str) -> String {
    if !value.contains('{') {
        return value.to_string();
    }
    let mut out = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(open) = rest.find('{') {
        let Some(close_rel) = rest[open..].find('}') else {
            // 无闭合大括号：余文原样保留（目录守卫保证不出现）。
            out.push_str(rest);
            return out;
        };
        let inner = rest[open + 1..open + close_rel].trim();
        let placeholder = inner
            .strip_prefix('$')
            .map(str::trim)
            .filter(|name| !name.is_empty());
        match placeholder {
            Some(name) => {
                out.push_str(&rest[..open]);
                out.push_str("{$");
                out.push_str(name);
                out.push('}');
                rest = &rest[open + close_rel + 1..];
            }
            None => {
                out.push_str(&rest[..open + 1]);
                rest = &rest[open + 1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// 全局 `t()`：供无法持有 `I18n` 实例的位置使用（语言经检测链缓存）。
///
/// 仅供测试调用，故以 `#[cfg(test)]` 门控避免死代码。
#[cfg(test)]
pub(crate) fn global_t(key: &str) -> String {
    static GLOBAL_I18N: OnceLock<I18n> = OnceLock::new();
    GLOBAL_I18N.get_or_init(I18n::from_detected).t(key)
}

// ============================================================================
// 用户显式输入的 BCP-47 解析（--lang / HTTP lang 字段）
// ============================================================================

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
/// 按 `-` 分割取 primary subtag，识别所有 `zh-*` 变体（zh-Hans/zh-Hant/私有标签等）。
#[cfg(not(feature = "icu"))]
fn parse_lang_simple(s: &str) -> Lang {
    let lower = s.to_ascii_lowercase();
    // 取第一段作为 language primary subtag（如 "zh-Hans-CN" → "zh"）
    let primary = lower.split('-').next().unwrap_or("");
    match primary {
        "zh" => Lang::Zh,
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

    #[test]
    fn test_default_is_english() {
        let i18n = I18n::default();
        assert_eq!(i18n.lang(), Lang::En);
    }

    // ===== I18n::new =====

    #[test]
    fn test_new_creates_with_specified_lang() {
        let en = I18n::new(Lang::En);
        assert_eq!(en.lang(), Lang::En);

        let zh = I18n::new(Lang::Zh);
        assert_eq!(zh.lang(), Lang::Zh);
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

    /// "zhongwen" 以 "zh" 开头但不是有效的中文语言代码，应回退到 En。
    #[test]
    fn test_from_str_zhongwen_falls_back_to_en() {
        let i18n = I18n::from_str("zhongwen");
        assert_eq!(
            i18n.lang(),
            Lang::En,
            "zhongwen should fall back to En, not Zh"
        );
    }

    // ===== BCP-47 脚本子标签解析 =====

    #[test]
    fn test_from_str_zh_hans() {
        assert_eq!(I18n::from_str("zh-Hans").lang(), Lang::Zh);
    }

    #[test]
    fn test_from_str_zh_hant() {
        assert_eq!(I18n::from_str("zh-Hant").lang(), Lang::Zh);
    }

    #[test]
    fn test_from_str_zh_hans_cn() {
        assert_eq!(I18n::from_str("zh-Hans-CN").lang(), Lang::Zh);
    }

    #[test]
    fn test_from_str_zh_hant_tw() {
        assert_eq!(I18n::from_str("zh-Hant-TW").lang(), Lang::Zh);
    }

    #[test]
    fn test_from_str_zh_hans_sg() {
        assert_eq!(I18n::from_str("zh-Hans-SG").lang(), Lang::Zh);
    }

    #[test]
    fn test_from_str_zh_hant_hk() {
        assert_eq!(I18n::from_str("zh-Hant-HK").lang(), Lang::Zh);
    }

    #[test]
    fn test_from_str_zh_hans_mixed_case() {
        assert_eq!(I18n::from_str("ZH-hans").lang(), Lang::Zh);
        assert_eq!(I18n::from_str("zh-HANS").lang(), Lang::Zh);
    }

    /// "zh-x-calnexus" 是合法 BCP-47 私有用标签，应识别为 Zh。
    #[test]
    fn test_from_str_zh_private_use() {
        assert_eq!(I18n::from_str("zh-x-calnexus").lang(), Lang::Zh);
    }

    /// "zh-Latn-pinyin" 是合法 BCP-47 标签（拼音罗马化），应识别为 Zh。
    #[test]
    fn test_from_str_zh_latn_pinyin() {
        assert_eq!(I18n::from_str("zh-Latn-pinyin").lang(), Lang::Zh);
    }

    /// "zhongwen-Hans" 不是合法 BCP-47 language（primary subtag "zhongwen" 非短码），应回退到 En。
    #[test]
    fn test_from_str_zhongwen_hans_falls_back_to_en() {
        assert_eq!(I18n::from_str("zhongwen-Hans").lang(), Lang::En);
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
        // 5 个标签键
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
        // 5 个标签键
        assert_eq!(zh.t("label.position"), "位置");
        assert_eq!(zh.t("label.hint"), "提示");
        assert_eq!(zh.t("label.error_kind"), "错误类别");
        assert_eq!(zh.t("label.exit_code"), "退出码");
        assert_eq!(zh.t("label.suggestion"), "建议");
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

    // ===== I18n::t — 未知键 fail-loud（不 panic） =====

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

    /// 缺失键英文束回退后仍缺失时返回键名本身，不 panic。
    #[test]
    fn test_missing_key_falls_back_through_en_bundle_without_panic() {
        for lang in [Lang::En, Lang::Zh] {
            let i18n = I18n::new(lang);
            assert_eq!(i18n.t("no.such.key"), "no.such.key");
            assert_eq!(
                i18n.tf("no.such.key", &[("name", "x")]),
                "no.such.key",
                "tf missing key must return key itself without panic"
            );
        }
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
        // 未知键返回键本身，不做替换
        assert_eq!(
            en.tf("nonexistent.key", &[("name", "x")]),
            "nonexistent.key"
        );
    }

    /// 缺参回显：注册模板为紧凑形态，sdforge 缺参原样回显 → 与 Fluent 语义一致。
    #[test]
    fn test_tf_missing_placeholder_preserved() {
        let i18n = I18n::new(Lang::En);
        // 只提供 name，缺失 expected/actual —— 回显 {$expected}/{$actual}
        assert_eq!(
            i18n.tf("msg.function_arg_count", &[("name", "sin")]),
            "Function sin expects {$expected} args, got {$actual}"
        );
    }

    /// 空参数时 tf 与 t 等价（同一渲染路径，缺参占位符回显）。
    #[test]
    fn test_tf_empty_args_equivalent_to_t() {
        let en = I18n::new(Lang::En);
        assert_eq!(
            en.tf("msg.unbound_variable", &[]),
            "Unbound variable: {$name}"
        );
        assert_eq!(en.tf("error.parse", &[]), en.t("error.parse"));

        let zh = I18n::new(Lang::Zh);
        assert_eq!(zh.tf("msg.unbound_variable", &[]), "未绑定变量: {$name}");
        assert_eq!(zh.tf("error.parse", &[]), zh.t("error.parse"));
    }

    /// 未知语言（"ar"）解析为 En，取 en 束。
    #[test]
    fn test_unknown_language_uses_en_bundle() {
        let i18n = I18n::from_str("ar");
        assert_eq!(i18n.lang(), Lang::En);
        assert_eq!(i18n.t("error.parse"), "Parse error");
        assert_eq!(
            i18n.tf("msg.unbound_variable", &[("name", "foo")]),
            "Unbound variable: foo"
        );
    }

    /// 首尾空白语义保留（FTL 引号字符串）：cached_suffix 的前导空格不丢。
    #[test]
    fn test_leading_whitespace_preserved() {
        let en = I18n::new(Lang::En);
        assert_eq!(en.t("label.cached_suffix"), " (cached)");
        let zh = I18n::new(Lang::Zh);
        assert_eq!(zh.t("label.cached_suffix"), " （已缓存）");
    }

    // ===== 守卫测试 =====

    /// 从 FTL 文本提取消息键集合（`id = value` 行，跳过注释/空行）。
    fn ftl_keys(ftl: &str) -> Vec<String> {
        ftl.lines()
            .filter_map(|line| line.split_once(" = "))
            .map(|(id, _)| id.trim().to_string())
            .collect()
    }

    /// 键齐性：EN/ZH 两束 FTL 键集合必须一致。
    #[test]
    fn test_en_zh_ftl_key_parity() {
        let en_keys = ftl_keys(EN_FTL);
        let zh_keys = ftl_keys(ZH_FTL);
        let mut en_set = en_keys.clone();
        en_set.sort();
        en_set.dedup();
        let mut zh_set = zh_keys.clone();
        zh_set.sort();
        zh_set.dedup();
        assert_eq!(
            en_set.len(),
            zh_keys.len(),
            "duplicate keys in ZH FTL: expected {} unique, got {}",
            en_set.len(),
            zh_keys.len()
        );
        assert_eq!(en_set.len(), zh_set.len(), "en/zh key counts differ");
        for key in &en_set {
            assert!(
                zh_set.contains(key),
                "key '{key}' exists in en/messages.ftl but missing in zh/messages.ftl"
            );
        }
        for key in &zh_set {
            assert!(
                en_set.contains(key),
                "key '{key}' exists in zh/messages.ftl but missing in en/messages.ftl"
            );
        }
        // JSON 迁移基数：370 迁移键 + 21 新增键（panic/server/clap help）
        assert!(
            en_set.len() >= 391,
            "catalog should have at least 391 keys, got {}",
            en_set.len()
        );
        // 集合不含 '.'（FTL 标识符不允许）
        assert!(
            en_set.iter().all(|k| !k.contains('.')),
            "FTL ids must not contain '.'"
        );
    }

    /// 磁盘/内嵌同步守卫：include_str! 常量与磁盘文件逐字节一致。
    #[test]
    fn test_embedded_ftl_matches_locales_dir() {
        assert_eq!(
            EN_FTL,
            std::fs::read_to_string("locales/en/messages.ftl")
                .expect("read locales/en/messages.ftl"),
            "embedded EN_FTL out of sync with locales/en/messages.ftl"
        );
        assert_eq!(
            ZH_FTL,
            std::fs::read_to_string("locales/zh/messages.ftl")
                .expect("read locales/zh/messages.ftl"),
            "embedded ZH_FTL out of sync with locales/zh/messages.ftl"
        );
        // 目录内只允许 {en,zh}/messages.ftl 成对文件，防止新增资源忘记内嵌
        let mut disk: Vec<String> = Vec::new();
        for entry in std::fs::read_dir("locales").expect("read locales dir") {
            let entry = entry.expect("dir entry");
            if !entry.file_type().expect("file type").is_dir() {
                panic!("locales/ 顶层不应有散落文件: {:?}", entry.path());
            }
            let dir_name = entry.file_name().to_string_lossy().to_string();
            let ftl = entry.path().join("messages.ftl");
            assert!(ftl.exists(), "locales/{dir_name}/ 缺少 messages.ftl");
            disk.push(dir_name);
        }
        disk.sort();
        assert_eq!(disk, vec!["en".to_string(), "zh".to_string()]);
    }

    // ===== 检测链：env 读取抽纯函数 detect_from =====

    /// 构造合成 env 查表闭包（不触碰进程环境，并行测试安全）。
    fn env_of<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        let map: std::collections::HashMap<&str, &str> = pairs.iter().copied().collect();
        move |key| map.get(key).map(|v| v.to_string())
    }

    #[test]
    fn test_detect_zh_cn_utf8() {
        assert_eq!(detect_from(env_of(&[("LANG", "zh_CN.UTF-8")])), Lang::Zh);
    }

    #[test]
    fn test_detect_zh_tw() {
        assert_eq!(detect_from(env_of(&[("LANG", "zh_TW")])), Lang::Zh);
    }

    #[test]
    fn test_detect_fr_fr_falls_back_to_en() {
        // sys-locale 注入 None：断言与真机区域设置解耦
        assert_eq!(
            detect_from_with_sys(env_of(&[("LANG", "fr_FR")]), || None),
            Lang::En
        );
    }

    #[test]
    fn test_detect_c_locale_falls_back_to_en() {
        assert_eq!(
            detect_from_with_sys(env_of(&[("LANG", "C")]), || None),
            Lang::En
        );
        assert_eq!(
            detect_from_with_sys(env_of(&[("LANG", "POSIX")]), || None),
            Lang::En
        );
        assert_eq!(
            detect_from_with_sys(env_of(&[("LANG", "C.UTF-8")]), || None),
            Lang::En
        );
    }

    #[test]
    fn test_detect_empty_and_malformed_fall_back_to_en() {
        assert_eq!(
            detect_from_with_sys(env_of(&[("LANG", "")]), || None),
            Lang::En
        );
        assert_eq!(
            detect_from_with_sys(env_of(&[("LANG", "   ")]), || None),
            Lang::En
        );
        assert_eq!(
            detect_from_with_sys(env_of(&[("LANG", "!!!")]), || None),
            Lang::En
        );
        assert_eq!(
            detect_from_with_sys(env_of(&[("LANG", "zhongwen")]), || None),
            Lang::En
        );
        assert_eq!(detect_from_with_sys(env_of(&[]), || None), Lang::En);
    }

    #[test]
    fn test_detect_sys_locale_used_when_env_unparseable() {
        // env 均不可解析时系统区域生效（注入固定值钉住该步接线）
        assert_eq!(
            detect_from_with_sys(env_of(&[("LANG", "C")]), || Some("zh-CN".to_string())),
            Lang::Zh
        );
        assert_eq!(
            detect_from_with_sys(env_of(&[]), || Some("en-US".to_string())),
            Lang::En
        );
    }

    #[test]
    fn test_detect_chain_order() {
        // CALNEXUS_LANG 优先于 LC_ALL
        assert_eq!(
            detect_from(env_of(&[
                ("CALNEXUS_LANG", "en"),
                ("LC_ALL", "zh_CN.UTF-8"),
                ("LANG", "zh_CN.UTF-8")
            ])),
            Lang::En
        );
        assert_eq!(
            detect_from(env_of(&[
                ("CALNEXUS_LANG", "zh"),
                ("LC_ALL", "en_US.UTF-8")
            ])),
            Lang::Zh
        );
        // LC_ALL 优先于 LC_MESSAGES 与 LANG
        assert_eq!(
            detect_from(env_of(&[
                ("LC_ALL", "zh_CN.UTF-8"),
                ("LC_MESSAGES", "en_US.UTF-8"),
                ("LANG", "en_US.UTF-8")
            ])),
            Lang::Zh
        );
        // LC_MESSAGES 优先于 LANG
        assert_eq!(
            detect_from(env_of(&[
                ("LC_MESSAGES", "zh_CN.UTF-8"),
                ("LANG", "en_US.UTF-8")
            ])),
            Lang::Zh
        );
        // 前级不支持语言（fr）→ 回退链继续，后级 zh 生效
        assert_eq!(
            detect_from(env_of(&[
                ("LC_ALL", "fr_FR.UTF-8"),
                ("LANG", "zh_CN.UTF-8")
            ])),
            Lang::Zh
        );
    }

    /// 结果域收敛：任何输入链的结果只能是 En 或 Zh。
    #[test]
    fn test_detect_result_domain_is_en_or_zh() {
        let samples = [
            "zh",
            "zh-Hans",
            "zh-HK",
            "en",
            "en-GB",
            "de_DE",
            "ja",
            "ko_KR.eucKR",
            "C",
            "POSIX",
            "",
            "zhongwen",
            "@@@",
            "en_x_@broken",
        ];
        for s in samples {
            let lang = detect_from(env_of(&[("LANG", s)]));
            assert!(matches!(lang, Lang::En | Lang::Zh), "input {s:?}");
        }
    }

    // ===== normalize_env_value 细节 =====

    #[test]
    fn test_normalize_env_value_variants() {
        assert_eq!(normalize_env_value("zh_CN.UTF-8"), Some(Lang::Zh));
        assert_eq!(normalize_env_value("zh_TW"), Some(Lang::Zh));
        assert_eq!(normalize_env_value("zh-Hant-HK"), Some(Lang::Zh));
        assert_eq!(normalize_env_value("ZH"), Some(Lang::Zh));
        assert_eq!(normalize_env_value("en_US.UTF-8"), Some(Lang::En));
        assert_eq!(normalize_env_value("fr_FR"), None);
        assert_eq!(normalize_env_value("C"), None);
        assert_eq!(normalize_env_value("POSIX"), None);
        assert_eq!(normalize_env_value(""), None);
        // @modifier 剥离
        assert_eq!(normalize_env_value("zh_CN@pinyin"), Some(Lang::Zh));
    }

    // ===== sdforge 宿主注册直测（显式 locale，不触碰全局 locale 状态）=====

    /// 注册幂等：重复调用后宿主注册内容不变（键值集合逐一核对）。
    #[test]
    fn test_register_idempotent_and_verbatim() {
        ensure_registered();
        ensure_registered();
        for (locale, ftl) in [("en", EN_FTL), ("zh", ZH_FTL)] {
            for (id, value) in parse_entries(ftl) {
                let key = id.replace('-', ".");
                let rendered = sdforge::i18n::translate_for(locale, &key, &[]);
                assert_eq!(
                    rendered, value,
                    "sdforge registry must serve ({locale}, {key}) verbatim"
                );
            }
        }
    }

    /// 字符串字面量桥接：FTL `{ "..." }` 还原为字面文本（前导空格保留）。
    #[test]
    fn test_string_literal_unwrapped() {
        assert_eq!(
            unwrap_string_literal("{ \" (cached)\" }"),
            Some(" (cached)".to_string())
        );
        assert_eq!(
            unwrap_string_literal("{ \"a\\\\b\" }"),
            Some("a\\b".to_string())
        );
        assert_eq!(
            unwrap_string_literal("{ \"a\\\"b\" }"),
            Some("a\"b".to_string())
        );
        assert_eq!(unwrap_string_literal("no literal"), None);
    }

    /// 占位符紧凑化：模板注册形态 `{ $name }` → `{$name}`；非 `$` 大括号保留。
    #[test]
    fn test_compact_placeholders() {
        assert_eq!(compact_placeholders("a { $x } b"), "a {$x} b");
        assert_eq!(compact_placeholders("no brace"), "no brace");
        assert_eq!(compact_placeholders("{name} literal"), "{name} literal");
        assert_eq!(compact_placeholders("{ $a } and { $b }"), "{$a} and {$b}");
    }

    /// 未知键经 sdforge 终端回退返回键本身（显式 locale 查询路径）。
    #[test]
    fn test_translate_for_unknown_key_returns_key() {
        ensure_registered();
        assert_eq!(
            sdforge::i18n::translate_for("en", "nonexistent-key", &[]),
            "nonexistent-key"
        );
        assert_eq!(
            sdforge::i18n::translate_for("zh", "no.such.key", &[]),
            "no.such.key"
        );
    }

    // ===== global_t（panic hook 路径，不 panic）=====

    #[test]
    fn test_global_t_known_and_missing_keys() {
        assert!(!global_t("panic.internal_error").is_empty());
        assert_eq!(global_t("no.such.key"), "no.such.key");
    }
}
