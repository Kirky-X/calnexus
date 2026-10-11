// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! 服务器语言协商：请求级 `lang` 字段 → `I18n` 上下文。
//! 缺省/空值走系统语言检测链，未知标签宽松回退英文；机器可读协议字段恒为英文。
use crate::i18n::I18n;

/// 从请求级 `lang` 解析 i18n 上下文（缺省/空值走系统检测，未知值回退英文）。
pub(crate) fn resolve_i18n(lang: Option<&str>) -> I18n {
    match lang {
        Some(s) if !s.trim().is_empty() => I18n::from_str(s.trim()),
        _ => I18n::from_detected(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{self, Lang};

    /// 缺省 lang 跟随系统语言检测链。
    #[test]
    fn none_uses_detected_locale() {
        assert_eq!(resolve_i18n(None).lang(), i18n::detect_locale());
    }

    #[test]
    fn empty_and_blank_use_detected_locale() {
        assert_eq!(resolve_i18n(Some("")).lang(), i18n::detect_locale());
        assert_eq!(resolve_i18n(Some("   ")).lang(), i18n::detect_locale());
    }

    #[test]
    fn explicit_english_resolves_to_english() {
        assert_eq!(resolve_i18n(Some("en")).lang(), Lang::En);
        assert_eq!(resolve_i18n(Some("en-US")).lang(), Lang::En);
    }

    #[test]
    fn chinese_variants_resolve_to_chinese() {
        assert_eq!(resolve_i18n(Some("zh")).lang(), Lang::Zh);
        assert_eq!(resolve_i18n(Some("zh-CN")).lang(), Lang::Zh);
        assert_eq!(resolve_i18n(Some("zh-Hans")).lang(), Lang::Zh);
    }

    #[test]
    fn unknown_lang_falls_back_to_english() {
        assert_eq!(resolve_i18n(Some("fr")).lang(), Lang::En);
        assert_eq!(resolve_i18n(Some("not-a-lang")).lang(), Lang::En);
    }

    #[test]
    fn whitespace_is_trimmed() {
        assert_eq!(resolve_i18n(Some(" zh ")).lang(), Lang::Zh);
    }
}
