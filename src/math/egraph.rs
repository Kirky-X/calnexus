// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! E-graph（equality saturation）化简：`SymbolicExpr` ↔ `egg` 语言映射与规则集。
//!
//! 仅在 `egraph` feature 下编译；饱和失败（超限/不可映射）返回 `None`，
//! 调用方回退到贪心化简。

use egg::{
    AstSize, EGraph, Extractor, Id, RecExpr, Rewrite, Runner, Subst, define_language, rewrite as rw,
};

use super::symbolic::SymbolicExpr;

define_language! {
    /// E-graph 语言：常量以十进制字符串表示（`0`/`2` 等 pattern 可直接匹配）。
    pub enum SymLang {
        Num(String),
        Var(String),
        "+" = Add([Id; 2]),
        "-" = Sub([Id; 2]),
        "*" = Mul([Id; 2]),
        "/" = Div([Id; 2]),
        "^" = Pow([Id; 2]),
        "neg" = Neg(Id),
        "ln" = Ln(Id),
        "sin" = Sin(Id),
        "cos" = Cos(Id),
        "tan" = Tan(Id),
        "exp" = Exp(Id),
    }
}

/// 饱和迭代上限（安全边界，见 spec R-symbolic-simplification-103）。
const ITER_LIMIT: usize = 8;
/// E-graph 节点上限（安全边界）。
const NODE_LIMIT: usize = 10_000;

/// 常量格式化：整数无小数点，浮点保留原样（与 canonicalizer 口径一致）。
fn format_const(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() <= i64::MAX as f64 {
        format!("{}", v as i64)
    } else {
        format!("{}", v)
    }
}

/// `SymbolicExpr` → `RecExpr`；遇到无法表示的节点返回 `None`。
fn to_rec(expr: &SymbolicExpr) -> Option<RecExpr<SymLang>> {
    fn go(expr: &SymbolicExpr, rec: &mut RecExpr<SymLang>) -> Option<Id> {
        let id = match expr {
            SymbolicExpr::Const(v) => rec.add(SymLang::Num(format_const(*v))),
            SymbolicExpr::Var(name) => rec.add(SymLang::Var(name.clone())),
            SymbolicExpr::Add(l, r) => {
                let a = go(l, rec)?;
                let b = go(r, rec)?;
                rec.add(SymLang::Add([a, b]))
            }
            SymbolicExpr::Sub(l, r) => {
                let a = go(l, rec)?;
                let b = go(r, rec)?;
                rec.add(SymLang::Sub([a, b]))
            }
            SymbolicExpr::Mul(l, r) => {
                let a = go(l, rec)?;
                let b = go(r, rec)?;
                rec.add(SymLang::Mul([a, b]))
            }
            SymbolicExpr::Div(l, r) => {
                let a = go(l, rec)?;
                let b = go(r, rec)?;
                rec.add(SymLang::Div([a, b]))
            }
            SymbolicExpr::Pow(l, r) => {
                let a = go(l, rec)?;
                let b = go(r, rec)?;
                rec.add(SymLang::Pow([a, b]))
            }
            SymbolicExpr::Neg(e) => {
                let a = go(e, rec)?;
                rec.add(SymLang::Neg(a))
            }
            SymbolicExpr::Ln(e) => {
                let a = go(e, rec)?;
                rec.add(SymLang::Ln(a))
            }
            SymbolicExpr::Sin(e) => {
                let a = go(e, rec)?;
                rec.add(SymLang::Sin(a))
            }
            SymbolicExpr::Cos(e) => {
                let a = go(e, rec)?;
                rec.add(SymLang::Cos(a))
            }
            SymbolicExpr::Tan(e) => {
                let a = go(e, rec)?;
                rec.add(SymLang::Tan(a))
            }
            SymbolicExpr::Exp(e) => {
                let a = go(e, rec)?;
                rec.add(SymLang::Exp(a))
            }
        };
        Some(id)
    }

    let mut rec = RecExpr::default();
    go(expr, &mut rec)?;
    Some(rec)
}

/// `RecExpr` → `SymbolicExpr`（提取结果）。
fn from_rec(rec: &RecExpr<SymLang>) -> Option<SymbolicExpr> {
    fn go(rec: &RecExpr<SymLang>, id: Id) -> Option<SymbolicExpr> {
        let expr = match &rec[id] {
            SymLang::Num(s) => SymbolicExpr::Const(s.parse::<f64>().ok()?),
            SymLang::Var(name) => SymbolicExpr::Var(name.clone()),
            SymLang::Add([a, b]) => {
                SymbolicExpr::Add(Box::new(go(rec, *a)?), Box::new(go(rec, *b)?))
            }
            SymLang::Sub([a, b]) => {
                SymbolicExpr::Sub(Box::new(go(rec, *a)?), Box::new(go(rec, *b)?))
            }
            SymLang::Mul([a, b]) => {
                SymbolicExpr::Mul(Box::new(go(rec, *a)?), Box::new(go(rec, *b)?))
            }
            SymLang::Div([a, b]) => {
                SymbolicExpr::Div(Box::new(go(rec, *a)?), Box::new(go(rec, *b)?))
            }
            SymLang::Pow([a, b]) => {
                SymbolicExpr::Pow(Box::new(go(rec, *a)?), Box::new(go(rec, *b)?))
            }
            SymLang::Neg(a) => SymbolicExpr::Neg(Box::new(go(rec, *a)?)),
            SymLang::Ln(a) => SymbolicExpr::Ln(Box::new(go(rec, *a)?)),
            SymLang::Sin(a) => SymbolicExpr::Sin(Box::new(go(rec, *a)?)),
            SymLang::Cos(a) => SymbolicExpr::Cos(Box::new(go(rec, *a)?)),
            SymLang::Tan(a) => SymbolicExpr::Tan(Box::new(go(rec, *a)?)),
            SymLang::Exp(a) => SymbolicExpr::Exp(Box::new(go(rec, *a)?)),
        };
        Some(expr)
    }
    let root = Id::from(rec.as_ref().len().checked_sub(1)?);
    go(rec, root)
}

/// `?a` 不是常量 0（`x/0` 不参与 `x/x → 1` 化简）。
fn is_nonzero_const(egraph: &mut EGraph<SymLang, ()>, _id: Id, subst: &Subst) -> bool {
    let a: egg::Var = "?a".parse().expect("valid pattern var");
    let id = subst[a];
    !matches!(egraph[id].nodes.first(), Some(SymLang::Num(n)) if n == "0")
}

/// 规则集：交换/结合、恒等元、自减/自除、三角恒等。
fn rules() -> Vec<Rewrite<SymLang, ()>> {
    vec![
        rw!("add-comm"; "(+ ?a ?b)" => "(+ ?b ?a)"),
        rw!("add-assoc"; "(+ (+ ?a ?b) ?c)" => "(+ ?a (+ ?b ?c))"),
        rw!("mul-comm"; "(* ?a ?b)" => "(* ?b ?a)"),
        rw!("mul-assoc"; "(* (* ?a ?b) ?c)" => "(* ?a (* ?b ?c))"),
        rw!("add-zero"; "(+ ?a 0)" => "?a"),
        rw!("mul-one"; "(* ?a 1)" => "?a"),
        rw!("mul-zero"; "(* ?a 0)" => "0"),
        rw!("sub-self"; "(- ?a ?a)" => "0"),
        rw!("div-self"; "(/ ?a ?a)" => "1" if is_nonzero_const),
        rw!("pythagorean"; "(+ (^ (sin ?x) 2) (^ (cos ?x) 2))" => "1"),
    ]
}

/// 对表达式做 equality saturation 并提取最优形式；失败返回 `None`。
pub fn saturate(expr: &SymbolicExpr) -> Option<SymbolicExpr> {
    let rec = to_rec(expr)?;
    let runner = Runner::default()
        .with_iter_limit(ITER_LIMIT)
        .with_node_limit(NODE_LIMIT)
        .with_expr(&rec)
        .run(&rules());
    let extractor = Extractor::new(&runner.egraph, AstSize);
    let (_, best) = extractor.find_best(runner.roots[0]);
    from_rec(&best).map(|expr| sort_commutative(&expr))
}

/// 对 Add/Mul 做展平 + 稳定字典序排序，消除提取顺序对结果字符串的影响。
fn sort_commutative(expr: &SymbolicExpr) -> SymbolicExpr {
    fn collect<'a>(is_add: bool, expr: &'a SymbolicExpr, out: &mut Vec<&'a SymbolicExpr>) {
        match expr {
            SymbolicExpr::Add(l, r) if is_add => {
                collect(true, l, out);
                collect(true, r, out);
            }
            SymbolicExpr::Mul(l, r) if !is_add => {
                collect(false, l, out);
                collect(false, r, out);
            }
            _ => out.push(expr),
        }
    }

    fn rebuild(is_add: bool, terms: Vec<SymbolicExpr>) -> SymbolicExpr {
        let mut iter = terms.into_iter();
        let mut acc = iter.next().expect("non-empty terms");
        for term in iter {
            acc = if is_add {
                SymbolicExpr::Add(Box::new(acc), Box::new(term))
            } else {
                SymbolicExpr::Mul(Box::new(acc), Box::new(term))
            };
        }
        acc
    }

    match expr {
        SymbolicExpr::Add(_, _) | SymbolicExpr::Mul(_, _) => {
            let is_add = matches!(expr, SymbolicExpr::Add(_, _));
            let mut raw = Vec::new();
            collect(is_add, expr, &mut raw);
            let mut terms: Vec<SymbolicExpr> = raw.into_iter().map(sort_commutative).collect();
            terms.sort_by_key(super::symbolic::symbolic_to_string);
            rebuild(is_add, terms)
        }
        SymbolicExpr::Sub(l, r) => {
            SymbolicExpr::Sub(Box::new(sort_commutative(l)), Box::new(sort_commutative(r)))
        }
        SymbolicExpr::Div(l, r) => {
            SymbolicExpr::Div(Box::new(sort_commutative(l)), Box::new(sort_commutative(r)))
        }
        SymbolicExpr::Pow(l, r) => {
            SymbolicExpr::Pow(Box::new(sort_commutative(l)), Box::new(sort_commutative(r)))
        }
        SymbolicExpr::Neg(e) => SymbolicExpr::Neg(Box::new(sort_commutative(e))),
        SymbolicExpr::Ln(e) => SymbolicExpr::Ln(Box::new(sort_commutative(e))),
        SymbolicExpr::Sin(e) => SymbolicExpr::Sin(Box::new(sort_commutative(e))),
        SymbolicExpr::Cos(e) => SymbolicExpr::Cos(Box::new(sort_commutative(e))),
        SymbolicExpr::Tan(e) => SymbolicExpr::Tan(Box::new(sort_commutative(e))),
        SymbolicExpr::Exp(e) => SymbolicExpr::Exp(Box::new(sort_commutative(e))),
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::symbolic::symbolic_to_string;

    fn var(name: &str) -> SymbolicExpr {
        SymbolicExpr::Var(name.to_string())
    }

    fn c(value: f64) -> SymbolicExpr {
        SymbolicExpr::Const(value)
    }

    fn add(a: SymbolicExpr, b: SymbolicExpr) -> SymbolicExpr {
        SymbolicExpr::Add(Box::new(a), Box::new(b))
    }

    fn mul(a: SymbolicExpr, b: SymbolicExpr) -> SymbolicExpr {
        SymbolicExpr::Mul(Box::new(a), Box::new(b))
    }

    fn div(a: SymbolicExpr, b: SymbolicExpr) -> SymbolicExpr {
        SymbolicExpr::Div(Box::new(a), Box::new(b))
    }

    fn sub(a: SymbolicExpr, b: SymbolicExpr) -> SymbolicExpr {
        SymbolicExpr::Sub(Box::new(a), Box::new(b))
    }

    fn sin_sq(x: &str) -> SymbolicExpr {
        SymbolicExpr::Pow(
            Box::new(SymbolicExpr::Sin(Box::new(var(x)))),
            Box::new(c(2.0)),
        )
    }

    fn cos_sq(x: &str) -> SymbolicExpr {
        SymbolicExpr::Pow(
            Box::new(SymbolicExpr::Cos(Box::new(var(x)))),
            Box::new(c(2.0)),
        )
    }

    fn simplify_via_egraph(expr: &SymbolicExpr) -> String {
        symbolic_to_string(&saturate(expr).expect("saturate should succeed"))
    }

    #[test]
    fn test_egraph_pythagorean_identity() {
        let expr = add(sin_sq("x"), cos_sq("x"));
        assert_eq!(simplify_via_egraph(&expr), "1");
    }

    #[test]
    fn test_egraph_sub_self_is_zero() {
        assert_eq!(simplify_via_egraph(&sub(var("x"), var("x"))), "0");
    }

    #[test]
    fn test_egraph_mul_zero_is_zero() {
        assert_eq!(simplify_via_egraph(&mul(var("x"), c(0.0))), "0");
    }

    #[test]
    fn test_egraph_div_self_is_one() {
        assert_eq!(simplify_via_egraph(&div(var("x"), var("x"))), "1");
    }

    #[test]
    fn test_egraph_div_zero_by_zero_not_one() {
        let out = simplify_via_egraph(&div(c(0.0), c(0.0)));
        assert_ne!(out, "1");
    }

    #[test]
    fn test_egraph_associativity_equivalence() {
        let left = add(add(var("a"), var("b")), var("c"));
        let right = add(add(var("a"), var("c")), var("b"));
        assert_eq!(simplify_via_egraph(&left), simplify_via_egraph(&right));
    }
}
