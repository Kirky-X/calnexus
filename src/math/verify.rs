// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! 表达式判等纯函数：先做符号恒零判定，再做数值采样反例搜索。
//!
//! 供 server 层 `verify` tool 使用；核心库零新增依赖，全部确定性。

use std::collections::HashMap;

use crate::core::{CalcError, parse};
use crate::math::symbolic::{SymbolicExpr, ast_to_symbolic, eval_symbolic, simplify};

/// 判等结论：恒等（符号判定或采样未发现反例）/ 数值反例。
#[derive(Debug, Clone, PartialEq)]
pub enum VerifyVerdict {
    Identical,
    Counterexample { x: f64, lhs: f64, rhs: f64 },
}

/// 递归收集符号表达式中的自由变量名（按首次出现顺序）。
fn free_variables(expr: &SymbolicExpr, out: &mut Vec<String>) {
    match expr {
        SymbolicExpr::Var(name) => {
            if !out.contains(name) {
                out.push(name.clone());
            }
        }
        SymbolicExpr::Add(l, r)
        | SymbolicExpr::Sub(l, r)
        | SymbolicExpr::Mul(l, r)
        | SymbolicExpr::Div(l, r)
        | SymbolicExpr::Pow(l, r) => {
            free_variables(l, out);
            free_variables(r, out);
        }
        SymbolicExpr::Neg(e)
        | SymbolicExpr::Ln(e)
        | SymbolicExpr::Sin(e)
        | SymbolicExpr::Cos(e)
        | SymbolicExpr::Tan(e)
        | SymbolicExpr::Exp(e) => free_variables(e, out),
        SymbolicExpr::Const(_) => {}
    }
}

/// 恒零判定：化简结果为零常数，或递归结构恒零（例如 0·f）。
fn is_zero_expr(expr: &SymbolicExpr) -> bool {
    match expr {
        SymbolicExpr::Const(v) => v.abs() < 1e-15,
        SymbolicExpr::Neg(e) => is_zero_expr(e),
        SymbolicExpr::Mul(l, r) => is_zero_expr(l) || is_zero_expr(r),
        SymbolicExpr::Div(l, r) => is_zero_expr(l) && !is_zero_expr(r),
        SymbolicExpr::Add(l, r) | SymbolicExpr::Sub(l, r) => is_zero_expr(l) && is_zero_expr(r),
        _ => false,
    }
}

/// 默认采样网格：21 个正整数点，首个非平凡反例通常在 x=2 附近。
fn default_points() -> Vec<f64> {
    (1..=21).map(|v| v as f64).collect()
}

/// 判断两个表达式是否恒等：
/// 1. `simplify(lhs - rhs)` 恒零 → [`VerifyVerdict::Identical`]；
/// 2. 否则在采样点上求值，找到 `|lhs-rhs| > 1e-9` 的点即返回反例；
/// 3. 全部采样点相等 → 数值置信的 `Identical`。
///
/// `var` 未指定时取表达式中的首个自由变量；无自由变量则直接比较常量。
pub fn verify_equality(
    lhs_expr: &str,
    rhs_expr: &str,
    var: Option<&str>,
    sample_points: Option<&[f64]>,
) -> Result<VerifyVerdict, CalcError> {
    let lhs = ast_to_symbolic(&parse(lhs_expr)?)?;
    let rhs = ast_to_symbolic(&parse(rhs_expr)?)?;
    let diff = simplify(&SymbolicExpr::Sub(
        Box::new(lhs.clone()),
        Box::new(rhs.clone()),
    ));
    if is_zero_expr(&diff) {
        return Ok(VerifyVerdict::Identical);
    }

    let mut names = Vec::new();
    free_variables(&lhs, &mut names);
    free_variables(&rhs, &mut names);
    let var_name = var.map(str::to_string).or_else(|| names.into_iter().next());

    let Some(name) = var_name else {
        let empty = HashMap::new();
        let lv = eval_symbolic(&lhs, &empty)?;
        let rv = eval_symbolic(&rhs, &empty)?;
        return Ok(if (lv - rv).abs() > 1e-9 {
            VerifyVerdict::Counterexample {
                x: 0.0,
                lhs: lv,
                rhs: rv,
            }
        } else {
            VerifyVerdict::Identical
        });
    };

    let points: Vec<f64> = sample_points
        .map(<[f64]>::to_vec)
        .unwrap_or_else(default_points);
    for x in points {
        let mut env = HashMap::new();
        env.insert(name.clone(), x);
        let lv = eval_symbolic(&lhs, &env)?;
        let rv = eval_symbolic(&rhs, &env)?;
        if (lv - rv).abs() > 1e-9 {
            return Ok(VerifyVerdict::Counterexample {
                x,
                lhs: lv,
                rhs: rv,
            });
        }
    }
    Ok(VerifyVerdict::Identical)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_polynomial_expansion() {
        let verdict = verify_equality("(x+1)^2", "x^2+2*x+1", Some("x"), None).unwrap();
        assert_eq!(verdict, VerifyVerdict::Identical);
    }

    #[test]
    fn test_counterexample_x2_vs_x3() {
        let verdict = verify_equality("x^2", "x^3", Some("x"), None).unwrap();
        assert_eq!(
            verdict,
            VerifyVerdict::Counterexample {
                x: 2.0,
                lhs: 4.0,
                rhs: 8.0
            }
        );
    }

    #[test]
    fn test_constant_identity() {
        assert_eq!(
            verify_equality("1+1", "2", None, None).unwrap(),
            VerifyVerdict::Identical
        );
    }

    #[test]
    fn test_custom_sample_points() {
        let verdict = verify_equality("x", "x+1", Some("x"), Some(&[0.0, 1.0])).unwrap();
        assert_eq!(
            verdict,
            VerifyVerdict::Counterexample {
                x: 0.0,
                lhs: 0.0,
                rhs: 1.0
            }
        );
    }
}
