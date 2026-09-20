// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! 服务器语言协商：请求级 `lang` 字段 → `I18n` 上下文。
//!
//! 协商规则（HTTP body 与 MCP tool args 同构生效）：
//! - `lang` 缺省 / 空串 → `I18n::default()`（英文，协议默认语言）
//! - `lang` 为 BCP-47 标签 → `I18n::from_str` 解析（"zh"/"zh-CN"/"zh-Hans" → 中文；
//!   未知标签宽松回退英文，与 CLI `--lang` 行为一致）
//!
//! 契约：不带 `lang`（或 `lang: "en"`）的响应与历史行为逐字节一致；
//! 仅显式 `lang=zh` 时人可读文案（错误消息、fx note）切换中文。
//! 机器可读字段（`kind` 协议名、503 `service`/`retry_after`、422 校验约束文本）保持英文。
use crate::i18n::I18n;

/// 从请求级 `lang` 字段解析 i18n 上下文。
///
/// 显式值宽松解析（未知值回退英文）；缺省/空值走系统语言检测链。
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

    /// 缺省 lang 跟随系统语言检测链（不再固定英文，契约）。
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
