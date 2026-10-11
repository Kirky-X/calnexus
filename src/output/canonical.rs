// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! 规范形式输出格式化器。
//!
//! 暴露 `CanonicalForm` 的字符串形式，作为 `--canonical` 标志的输出。

use crate::core::CanonicalForm;

/// 格式化 `CanonicalForm` 为字符串。
pub fn format_canonical(cf: &CanonicalForm) -> String {
    cf.as_str().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_basic_plus_expression() {
        let cf = CanonicalForm::new("(+ 2 3)");
        assert_eq!(format_canonical(&cf), "(+ 2 3)");
    }

    #[test]
    fn canonical_complex_s_expr() {
        let cf = CanonicalForm::new("(* (+ 1 2) (- 4 1))");
        assert_eq!(format_canonical(&cf), "(* (+ 1 2) (- 4 1))");
    }

    #[test]
    fn canonical_single_value() {
        let cf = CanonicalForm::new("42");
        assert_eq!(format_canonical(&cf), "42");
    }

    #[test]
    fn canonical_empty_string() {
        let cf = CanonicalForm::new("");
        assert_eq!(format_canonical(&cf), "");
    }

    #[test]
    fn canonical_preserves_whitespace() {
        let cf = CanonicalForm::new("(+  2  3)");
        assert_eq!(format_canonical(&cf), "(+  2  3)");
    }
}
