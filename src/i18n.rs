// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! ICU4X 国际化模块：基于 `icu` crate 提供中英双语错误消息。
//!
//! P0 阶段为占位模块，T0.3 将填充完整实现：
//! - `Lang` 枚举（En/Zh）
//! - `I18n` 结构体 + `from_str` / `t` 方法
//! - 消息目录 match 表（ErrorKind → i18n key → en/zh 文本）
