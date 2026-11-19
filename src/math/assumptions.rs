// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! 符号假设系统：解析 `--assume "x > 0"` 一类声明，供假设感知化简使用。
//!
//! 仅描述变量前提，不参与数值求值；核心库零新增依赖。

use crate::core::CalcError;
use std::collections::HashMap;

/// 单条变量假设。
#[derive(Debug, Clone, PartialEq)]
pub enum Assumption {
    /// `x > 0`。
    Positive,
    /// `x >= 0`。
    NonNegative,
    /// `x != 0`。
    NonZero,
    /// `n is integer`。
    Integer,
    /// `x > c`（c ≠ 0）。
    GreaterThan(f64),
    /// `x < c`（c ≠ 0）。
    LessThan(f64),
}

/// 假设集合：变量名 → 该变量的假设列表（可重复累积）。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AssumptionSet {
    vars: HashMap<String, Vec<Assumption>>,
}

impl AssumptionSet {
    /// 解析单条声明为独立假设集。
    pub fn parse(spec: &str) -> Result<Self, CalcError> {
        let mut set = Self::default();
        set.insert(spec)?;
        Ok(set)
    }

    /// 追加一条声明；同一变量的假设按出现顺序累积。
    pub fn insert(&mut self, spec: &str) -> Result<(), CalcError> {
        let (var, assumption) = parse_declaration(spec)?;
        self.vars.entry(var).or_default().push(assumption);
        Ok(())
    }

    /// 查询变量对应的假设列表。
    pub fn get(&self, var: &str) -> Option<&[Assumption]> {
        self.vars.get(var).map(Vec::as_slice)
    }

    /// 是否没有任何假设。
    pub fn is_empty(&self) -> bool {
        self.vars.values().all(Vec::is_empty)
    }

    /// 全部假设对的规范化序列化（供缓存键使用，顺序稳定）。
    pub fn canonical(&self) -> String {
        let mut keys: Vec<&String> = self.vars.keys().collect();
        keys.sort();
        let mut out = String::new();
        for key in keys {
            for assumption in &self.vars[key] {
                out.push_str(&format!("{key}:{assumption:?};"));
            }
        }
        out
    }
}

/// 解析 `var OP number` / `var is integer` 形式。
fn parse_declaration(spec: &str) -> Result<(String, Assumption), CalcError> {
    let unsupported = || {
        CalcError::usage(format!("unsupported assumption syntax: {spec}"))
            .with_i18n("msg.assume.unsupported", vec![])
    };
    let text = spec.trim();
    if text.is_empty() {
        return Err(unsupported());
    }

    if let Some(var) = text.strip_suffix(" is integer") {
        let var = var.trim();
        if is_identifier(var) {
            return Ok((var.to_string(), Assumption::Integer));
        }
        return Err(unsupported());
    }

    for op in [">=", "<=", "!=", ">", "<"] {
        if let Some((var, rhs)) = text.split_once(op) {
            let var = var.trim();
            let rhs = rhs.trim();
            if !is_identifier(var) {
                return Err(unsupported());
            }
            let value: f64 = rhs.parse().map_err(|_| unsupported())?;
            if !value.is_finite() {
                return Err(unsupported());
            }
            let assumption = match (op, value) {
                (">", 0.0) => Assumption::Positive,
                (">=", 0.0) => Assumption::NonNegative,
                ("!=", 0.0) => Assumption::NonZero,
                (">", v) => Assumption::GreaterThan(v),
                (">=", v) => Assumption::GreaterThan(v),
                ("<", v) => Assumption::LessThan(v),
                ("<=", v) => Assumption::LessThan(v),
                _ => return Err(unsupported()),
            };
            return Ok((var.to_string(), assumption));
        }
    }
    Err(unsupported())
}

fn is_identifier(name: &str) -> bool {
    !name.is_empty()
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !name.chars().next().is_some_and(|c| c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_each_variant() {
        let cases = [
            ("x > 0", Assumption::Positive),
            ("x >= 0", Assumption::NonNegative),
            ("x != 0", Assumption::NonZero),
            ("n is integer", Assumption::Integer),
            ("y > 1.5", Assumption::GreaterThan(1.5)),
            ("y < -2", Assumption::LessThan(-2.0)),
        ];
        for (spec, expected) in cases {
            let set = AssumptionSet::parse(spec).unwrap();
            let var = spec.split_whitespace().next().unwrap();
            assert_eq!(set.get(var), Some(&[expected][..]), "spec={spec}");
        }
    }

    #[test]
    fn test_fold_equivalence_and_non_equivalence() {
        assert_eq!(
            AssumptionSet::parse("y >= 0").unwrap(),
            AssumptionSet::parse("y >= 0").unwrap()
        );
        assert_ne!(
            AssumptionSet::parse("x > 0").unwrap(),
            AssumptionSet::parse("x >= 0.000001").unwrap()
        );
    }

    #[test]
    fn test_insert_accumulates() {
        let mut set = AssumptionSet::parse("x > 0").unwrap();
        set.insert("x is integer").unwrap();
        assert_eq!(set.get("x").unwrap().len(), 2);
    }

    #[test]
    fn test_unsupported_syntax() {
        for spec in ["", "x ??? 0", "x > abc", "1x > 0", "x = 0"] {
            let err = AssumptionSet::parse(spec).unwrap_err();
            assert_eq!(err.kind, crate::core::ErrorKind::Usage, "spec={spec}");
        }
    }
}
