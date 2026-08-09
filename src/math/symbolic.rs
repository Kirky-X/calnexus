// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! 符号演算核心函数：微分/积分/化简/极限/泰勒展开。
//!
//! 从 `domains/symbolic.rs` 提取的纯数学逻辑，
//! 不含 AST 转换（`ast_to_symbolic` 依赖 `AstNode`，留在域层）。

use crate::core::{CalcError, EvalResult};
use std::collections::HashMap;

/// 符号表达式：符号变换的中间表示。
///
/// 与 [`AstNode`] 不同，`SymbolicExpr` 专用于符号运算（求导/积分/化简），
/// 不含 Matrix/List/BigNumber 等非符号节点。
#[derive(Debug, Clone, PartialEq)]
pub enum SymbolicExpr {
    /// 常数。
    Const(f64),
    /// 变量。
    Var(String),
    /// 加法 `f + g`。
    Add(Box<SymbolicExpr>, Box<SymbolicExpr>),
    /// 减法 `f - g`。
    Sub(Box<SymbolicExpr>, Box<SymbolicExpr>),
    /// 乘法 `f * g`。
    Mul(Box<SymbolicExpr>, Box<SymbolicExpr>),
    /// 除法 `f / g`。
    Div(Box<SymbolicExpr>, Box<SymbolicExpr>),
    /// 幂 `f ^ g`。
    Pow(Box<SymbolicExpr>, Box<SymbolicExpr>),
    /// 负号 `-f`。
    Neg(Box<SymbolicExpr>),
    /// 自然对数 `ln(f)`。
    Ln(Box<SymbolicExpr>),
    /// 正弦 `sin(f)`。
    Sin(Box<SymbolicExpr>),
    /// 余弦 `cos(f)`。
    Cos(Box<SymbolicExpr>),
    /// 正切 `tan(f)`。
    Tan(Box<SymbolicExpr>),
    /// 指数 `exp(f)`。
    Exp(Box<SymbolicExpr>),
}

impl SymbolicExpr {
    /// 若为常数返回其值。
    pub fn as_const(&self) -> Option<f64> {
        if let SymbolicExpr::Const(v) = self {
            Some(*v)
        } else {
            None
        }
    }

    /// 是否为零常数。
    pub fn is_zero(&self) -> bool {
        self.as_const() == Some(0.0)
    }

    /// 是否为一常数。
    pub fn is_one(&self) -> bool {
        self.as_const() == Some(1.0)
    }
}

// ============================ 格式化 (TG3.1) ============================

/// 将 [`SymbolicExpr`] 格式化为可读字符串。
pub fn symbolic_to_string(expr: &SymbolicExpr) -> String {
    match expr {
        SymbolicExpr::Const(n) => format_number(*n),
        SymbolicExpr::Var(s) => s.clone(),
        SymbolicExpr::Add(l, r) => format!("{}+{}", symbolic_to_string(l), symbolic_to_string(r)),
        SymbolicExpr::Sub(l, r) => {
            let rs = symbolic_to_string(r);
            if matches!(
                r.as_ref(),
                SymbolicExpr::Add(_, _) | SymbolicExpr::Sub(_, _)
            ) {
                format!("{}-({})", symbolic_to_string(l), rs)
            } else {
                format!("{}-{}", symbolic_to_string(l), rs)
            }
        }
        SymbolicExpr::Mul(l, r) => {
            let ls = parenthesize_for_mul(l);
            let rs = parenthesize_for_mul(r);
            format!("{}*{}", ls, rs)
        }
        SymbolicExpr::Div(l, r) => {
            let ls = parenthesize_for_mul(l);
            let rs = parenthesize_for_mul(r);
            format!("{}/{}", ls, rs)
        }
        SymbolicExpr::Pow(l, r) => {
            let ls = if matches!(l.as_ref(), SymbolicExpr::Const(_) | SymbolicExpr::Var(_)) {
                symbolic_to_string(l)
            } else {
                format!("({})", symbolic_to_string(l))
            };
            let rs = if matches!(r.as_ref(), SymbolicExpr::Const(_) | SymbolicExpr::Var(_)) {
                symbolic_to_string(r)
            } else {
                format!("({})", symbolic_to_string(r))
            };
            format!("{}^{}", ls, rs)
        }
        SymbolicExpr::Neg(e) => {
            if matches!(
                e.as_ref(),
                SymbolicExpr::Add(_, _) | SymbolicExpr::Sub(_, _)
            ) {
                format!("-({})", symbolic_to_string(e))
            } else {
                format!("-{}", symbolic_to_string(e))
            }
        }
        SymbolicExpr::Ln(e) => format!("ln({})", symbolic_to_string(e)),
        SymbolicExpr::Sin(e) => format!("sin({})", symbolic_to_string(e)),
        SymbolicExpr::Cos(e) => format!("cos({})", symbolic_to_string(e)),
        SymbolicExpr::Tan(e) => format!("tan({})", symbolic_to_string(e)),
        SymbolicExpr::Exp(e) => format!("exp({})", symbolic_to_string(e)),
    }
}

/// 乘法/除法中需要加括号的子表达式。
fn parenthesize_for_mul(expr: &SymbolicExpr) -> String {
    match expr {
        SymbolicExpr::Const(_)
        | SymbolicExpr::Var(_)
        | SymbolicExpr::Sin(_)
        | SymbolicExpr::Cos(_)
        | SymbolicExpr::Tan(_)
        | SymbolicExpr::Ln(_)
        | SymbolicExpr::Exp(_) => symbolic_to_string(expr),
        SymbolicExpr::Pow(base, exp) => {
            let _ = (base, exp);
            symbolic_to_string(expr)
        }
        _ => format!("({})", symbolic_to_string(expr)),
    }
}

/// 格式化浮点数：整数省略小数点。
fn format_number(n: f64) -> String {
    if n == n.trunc() && n.abs() < 1e16 {
        format!("{}", n as i64)
    } else {
        format!("{}", n)
    }
}

// ============================ 符号求导 diff (TG3.2) ============================

/// 符号求导 `diff(expr, var)`。
///
/// 递归应用求导规则：
/// - 常数 → 0
/// - 变量 → 1（若为目标变量）/ 0（其他变量）
/// - 和差、积（乘积法则）、商（商法则）、幂（幂法则 + 链式）
/// - sin→cos、cos→-sin、tan→sec²、exp→exp、ln→1/x
pub fn diff(expr: &SymbolicExpr, var: &str) -> SymbolicExpr {
    match expr {
        SymbolicExpr::Const(_) => SymbolicExpr::Const(0.0),
        SymbolicExpr::Var(name) => diff_var(name, var),
        SymbolicExpr::Add(l, r) => diff_add(l.as_ref(), r.as_ref(), var),
        SymbolicExpr::Sub(l, r) => diff_sub(l.as_ref(), r.as_ref(), var),
        SymbolicExpr::Mul(f, g) => diff_mul(f.as_ref(), g.as_ref(), var),
        SymbolicExpr::Div(f, g) => diff_div(f.as_ref(), g.as_ref(), var),
        SymbolicExpr::Pow(f, g) => diff_pow(f.as_ref(), g.as_ref(), var),
        SymbolicExpr::Neg(f) => diff_neg(f.as_ref(), var),
        SymbolicExpr::Sin(f) => diff_sin(f.as_ref(), var),
        SymbolicExpr::Cos(f) => diff_cos(f.as_ref(), var),
        SymbolicExpr::Tan(f) => diff_tan(f.as_ref(), var),
        SymbolicExpr::Exp(f) => diff_exp(f.as_ref(), var),
        SymbolicExpr::Ln(f) => diff_ln(f.as_ref(), var),
    }
}

fn diff_var(name: &str, var: &str) -> SymbolicExpr {
    if name == var {
        SymbolicExpr::Const(1.0)
    } else {
        SymbolicExpr::Const(0.0)
    }
}

fn diff_add(l: &SymbolicExpr, r: &SymbolicExpr, var: &str) -> SymbolicExpr {
    SymbolicExpr::Add(Box::new(diff(l, var)), Box::new(diff(r, var)))
}

fn diff_sub(l: &SymbolicExpr, r: &SymbolicExpr, var: &str) -> SymbolicExpr {
    SymbolicExpr::Sub(Box::new(diff(l, var)), Box::new(diff(r, var)))
}

/// 乘积法则：(f*g)' = f'*g + f*g'。
fn diff_mul(f: &SymbolicExpr, g: &SymbolicExpr, var: &str) -> SymbolicExpr {
    SymbolicExpr::Add(
        Box::new(SymbolicExpr::Mul(
            Box::new(diff(f, var)),
            Box::new(g.clone()),
        )),
        Box::new(SymbolicExpr::Mul(
            Box::new(f.clone()),
            Box::new(diff(g, var)),
        )),
    )
}

/// 商法则：(f/g)' = (f'*g - f*g') / g²。
fn diff_div(f: &SymbolicExpr, g: &SymbolicExpr, var: &str) -> SymbolicExpr {
    SymbolicExpr::Div(
        Box::new(SymbolicExpr::Sub(
            Box::new(SymbolicExpr::Mul(
                Box::new(diff(f, var)),
                Box::new(g.clone()),
            )),
            Box::new(SymbolicExpr::Mul(
                Box::new(f.clone()),
                Box::new(diff(g, var)),
            )),
        )),
        Box::new(SymbolicExpr::Pow(
            Box::new(g.clone()),
            Box::new(SymbolicExpr::Const(2.0)),
        )),
    )
}

/// 幂法则：f^n → n*f^(n-1)*f'（指数为常数）；
/// 一般幂法则：f^g → f^g * (g'*ln(f) + g*f'/f)（指数非常数）。
fn diff_pow(f: &SymbolicExpr, g: &SymbolicExpr, var: &str) -> SymbolicExpr {
    if let SymbolicExpr::Const(n) = g {
        SymbolicExpr::Mul(
            Box::new(SymbolicExpr::Mul(
                Box::new(SymbolicExpr::Const(*n)),
                Box::new(SymbolicExpr::Pow(
                    Box::new(f.clone()),
                    Box::new(SymbolicExpr::Const(n - 1.0)),
                )),
            )),
            Box::new(diff(f, var)),
        )
    } else {
        SymbolicExpr::Mul(
            Box::new(SymbolicExpr::Pow(Box::new(f.clone()), Box::new(g.clone()))),
            Box::new(SymbolicExpr::Add(
                Box::new(SymbolicExpr::Mul(
                    Box::new(diff(g, var)),
                    Box::new(SymbolicExpr::Ln(Box::new(f.clone()))),
                )),
                Box::new(SymbolicExpr::Div(
                    Box::new(SymbolicExpr::Mul(
                        Box::new(g.clone()),
                        Box::new(diff(f, var)),
                    )),
                    Box::new(f.clone()),
                )),
            )),
        )
    }
}

fn diff_neg(f: &SymbolicExpr, var: &str) -> SymbolicExpr {
    SymbolicExpr::Neg(Box::new(diff(f, var)))
}

/// sin(f) → cos(f)*f'。
fn diff_sin(f: &SymbolicExpr, var: &str) -> SymbolicExpr {
    SymbolicExpr::Mul(
        Box::new(SymbolicExpr::Cos(Box::new(f.clone()))),
        Box::new(diff(f, var)),
    )
}

/// cos(f) → -sin(f)*f'。
fn diff_cos(f: &SymbolicExpr, var: &str) -> SymbolicExpr {
    SymbolicExpr::Mul(
        Box::new(SymbolicExpr::Neg(Box::new(SymbolicExpr::Sin(Box::new(
            f.clone(),
        ))))),
        Box::new(diff(f, var)),
    )
}

/// tan(f) → (1/cos²(f))*f' = sec²(f)*f'。
fn diff_tan(f: &SymbolicExpr, var: &str) -> SymbolicExpr {
    SymbolicExpr::Mul(
        Box::new(SymbolicExpr::Div(
            Box::new(SymbolicExpr::Const(1.0)),
            Box::new(SymbolicExpr::Pow(
                Box::new(SymbolicExpr::Cos(Box::new(f.clone()))),
                Box::new(SymbolicExpr::Const(2.0)),
            )),
        )),
        Box::new(diff(f, var)),
    )
}

/// exp(f) → exp(f)*f'。
fn diff_exp(f: &SymbolicExpr, var: &str) -> SymbolicExpr {
    SymbolicExpr::Mul(
        Box::new(SymbolicExpr::Exp(Box::new(f.clone()))),
        Box::new(diff(f, var)),
    )
}

/// ln(f) → (1/f)*f'。
fn diff_ln(f: &SymbolicExpr, var: &str) -> SymbolicExpr {
    SymbolicExpr::Mul(
        Box::new(SymbolicExpr::Div(
            Box::new(SymbolicExpr::Const(1.0)),
            Box::new(f.clone()),
        )),
        Box::new(diff(f, var)),
    )
}

// ============================ 符号积分 integrate (TG3.3) ============================

/// 符号积分 `integrate(expr, var)`。
///
/// v1.0 仅支持：
/// - 多项式积分：`x^n → x^(n+1)/(n+1)`（n ≠ -1）
/// - 基本初等函数：sin→-cos、cos→sin、exp→exp、1/x→ln|x|
/// - 线性性：∫(f±g) = ∫f ± ∫g
///
/// 不支持的积分返回 DomainError。
pub fn integrate(expr: &SymbolicExpr, var: &str) -> Result<SymbolicExpr, CalcError> {
    match expr {
        SymbolicExpr::Const(c) => integrate_const(*c, var),
        SymbolicExpr::Var(name) => integrate_var(name, var),
        SymbolicExpr::Add(f, g) => integrate_add(f.as_ref(), g.as_ref(), var),
        SymbolicExpr::Sub(f, g) => integrate_sub(f.as_ref(), g.as_ref(), var),
        SymbolicExpr::Mul(f, g) => integrate_mul(f.as_ref(), g.as_ref(), var),
        SymbolicExpr::Div(f, g) => integrate_div(f.as_ref(), g.as_ref(), var),
        SymbolicExpr::Pow(f, g) => integrate_pow(f.as_ref(), g.as_ref(), var),
        SymbolicExpr::Neg(f) => integrate_neg(f.as_ref(), var),
        SymbolicExpr::Sin(f) => integrate_sin(f.as_ref(), var),
        SymbolicExpr::Cos(f) => integrate_cos(f.as_ref(), var),
        SymbolicExpr::Exp(f) => integrate_exp(f.as_ref(), var),
        SymbolicExpr::Ln(_) | SymbolicExpr::Tan(_) => Err(CalcError::domain(
            "integrate() does not support ln/tan forms".to_string(),
        )),
    }
}

fn integrate_const(c: f64, var: &str) -> Result<SymbolicExpr, CalcError> {
    Ok(SymbolicExpr::Mul(
        Box::new(SymbolicExpr::Const(c)),
        Box::new(SymbolicExpr::Var(var.to_string())),
    ))
}

fn integrate_var(name: &str, var: &str) -> Result<SymbolicExpr, CalcError> {
    if name == var {
        Ok(SymbolicExpr::Div(
            Box::new(SymbolicExpr::Pow(
                Box::new(SymbolicExpr::Var(var.to_string())),
                Box::new(SymbolicExpr::Const(2.0)),
            )),
            Box::new(SymbolicExpr::Const(2.0)),
        ))
    } else {
        Ok(SymbolicExpr::Mul(
            Box::new(SymbolicExpr::Var(name.to_string())),
            Box::new(SymbolicExpr::Var(var.to_string())),
        ))
    }
}

fn integrate_add(f: &SymbolicExpr, g: &SymbolicExpr, var: &str) -> Result<SymbolicExpr, CalcError> {
    Ok(SymbolicExpr::Add(
        Box::new(integrate(f, var)?),
        Box::new(integrate(g, var)?),
    ))
}

fn integrate_sub(f: &SymbolicExpr, g: &SymbolicExpr, var: &str) -> Result<SymbolicExpr, CalcError> {
    Ok(SymbolicExpr::Sub(
        Box::new(integrate(f, var)?),
        Box::new(integrate(g, var)?),
    ))
}

/// ∫c*f dx = c*∫f dx（常数提取）；两个非常数之积不支持。
fn integrate_mul(f: &SymbolicExpr, g: &SymbolicExpr, var: &str) -> Result<SymbolicExpr, CalcError> {
    if let SymbolicExpr::Const(c) = f {
        return Ok(SymbolicExpr::Mul(
            Box::new(SymbolicExpr::Const(*c)),
            Box::new(integrate(g, var)?),
        ));
    }
    if let SymbolicExpr::Const(c) = g {
        return Ok(SymbolicExpr::Mul(
            Box::new(SymbolicExpr::Const(*c)),
            Box::new(integrate(f, var)?),
        ));
    }
    Err(CalcError::domain(
        "integrate() does not support product of two non-constant expressions".to_string(),
    ))
}

/// ∫x^n dx = x^(n+1)/(n+1)（n ≠ -1）；∫1/x dx = ln|x|。
fn integrate_pow(f: &SymbolicExpr, g: &SymbolicExpr, var: &str) -> Result<SymbolicExpr, CalcError> {
    if let (SymbolicExpr::Var(name), SymbolicExpr::Const(n)) = (f, g) {
        if name == var {
            if *n == -1.0 {
                return Ok(SymbolicExpr::Ln(Box::new(SymbolicExpr::Var(
                    var.to_string(),
                ))));
            }
            return Ok(SymbolicExpr::Div(
                Box::new(SymbolicExpr::Pow(
                    Box::new(SymbolicExpr::Var(var.to_string())),
                    Box::new(SymbolicExpr::Const(n + 1.0)),
                )),
                Box::new(SymbolicExpr::Const(n + 1.0)),
            ));
        }
    }
    Err(CalcError::domain(
        "integrate() only supports power of the integration variable".to_string(),
    ))
}

/// ∫1/x dx = ln|x|（仅支持 Div(Const(1), Var) 形式）。
fn integrate_div(f: &SymbolicExpr, g: &SymbolicExpr, var: &str) -> Result<SymbolicExpr, CalcError> {
    if let (SymbolicExpr::Const(c), SymbolicExpr::Var(name)) = (f, g) {
        if *c == 1.0 && name == var {
            return Ok(SymbolicExpr::Ln(Box::new(SymbolicExpr::Var(
                var.to_string(),
            ))));
        }
    }
    Err(CalcError::domain(
        "integrate() only supports 1/var form for division".to_string(),
    ))
}

fn integrate_neg(f: &SymbolicExpr, var: &str) -> Result<SymbolicExpr, CalcError> {
    Ok(SymbolicExpr::Neg(Box::new(integrate(f, var)?)))
}

fn integrate_sin(f: &SymbolicExpr, var: &str) -> Result<SymbolicExpr, CalcError> {
    if is_var(f, var) {
        Ok(SymbolicExpr::Neg(Box::new(SymbolicExpr::Cos(Box::new(
            SymbolicExpr::Var(var.to_string()),
        )))))
    } else {
        Err(CalcError::domain(
            "integrate() only supports sin(var) form".to_string(),
        ))
    }
}

fn integrate_cos(f: &SymbolicExpr, var: &str) -> Result<SymbolicExpr, CalcError> {
    if is_var(f, var) {
        Ok(SymbolicExpr::Sin(Box::new(SymbolicExpr::Var(
            var.to_string(),
        ))))
    } else {
        Err(CalcError::domain(
            "integrate() only supports cos(var) form".to_string(),
        ))
    }
}

fn integrate_exp(f: &SymbolicExpr, var: &str) -> Result<SymbolicExpr, CalcError> {
    if is_var(f, var) {
        Ok(SymbolicExpr::Exp(Box::new(SymbolicExpr::Var(
            var.to_string(),
        ))))
    } else {
        Err(CalcError::domain(
            "integrate() only supports exp(var) form".to_string(),
        ))
    }
}

/// 检查表达式是否为指定变量。
fn is_var(expr: &SymbolicExpr, var: &str) -> bool {
    matches!(expr, SymbolicExpr::Var(name) if name == var)
}

// ============================ 表达式化简 simplify (TG3.4) ============================

/// 表达式化简 `simplify(expr)`。
///
/// 应用规则：
/// - 常量折叠：`Const(a) op Const(b) → Const(a op b)`
/// - 代数恒等式：`0+x→x`、`x+0→x`、`0*x→0`、`1*x→x`、`x^0→1`、`x^1→x`
/// - 递归化简子表达式
pub fn simplify(expr: &SymbolicExpr) -> SymbolicExpr {
    match expr {
        SymbolicExpr::Const(_) | SymbolicExpr::Var(_) => expr.clone(),
        SymbolicExpr::Add(l, r) => simplify_add(&simplify(l), &simplify(r)),
        SymbolicExpr::Sub(l, r) => simplify_sub(&simplify(l), &simplify(r)),
        SymbolicExpr::Mul(l, r) => simplify_mul(&simplify(l), &simplify(r)),
        SymbolicExpr::Div(l, r) => simplify_div(&simplify(l), &simplify(r)),
        SymbolicExpr::Pow(l, r) => simplify_pow(&simplify(l), &simplify(r)),
        SymbolicExpr::Neg(e) => simplify_neg(&simplify(e)),
        SymbolicExpr::Sin(e) => SymbolicExpr::Sin(Box::new(simplify(e))),
        SymbolicExpr::Cos(e) => SymbolicExpr::Cos(Box::new(simplify(e))),
        SymbolicExpr::Tan(e) => SymbolicExpr::Tan(Box::new(simplify(e))),
        SymbolicExpr::Ln(e) => SymbolicExpr::Ln(Box::new(simplify(e))),
        SymbolicExpr::Exp(e) => SymbolicExpr::Exp(Box::new(simplify(e))),
    }
}

fn simplify_add(l: &SymbolicExpr, r: &SymbolicExpr) -> SymbolicExpr {
    if let (Some(a), Some(b)) = (l.as_const(), r.as_const()) {
        return SymbolicExpr::Const(a + b);
    }
    if l.is_zero() {
        return r.clone();
    }
    if r.is_zero() {
        return l.clone();
    }
    // 合并同类项
    let (ca, rest_a) = extract_coeff(l);
    let (cb, rest_b) = extract_coeff(r);
    if rest_a == rest_b {
        return coeff_times(ca + cb, rest_a);
    }
    SymbolicExpr::Add(Box::new(l.clone()), Box::new(r.clone()))
}

fn simplify_sub(l: &SymbolicExpr, r: &SymbolicExpr) -> SymbolicExpr {
    if let (Some(a), Some(b)) = (l.as_const(), r.as_const()) {
        return SymbolicExpr::Const(a - b);
    }
    if r.is_zero() {
        return l.clone();
    }
    if l.is_zero() {
        return SymbolicExpr::Neg(Box::new(r.clone()));
    }
    let (ca, rest_a) = extract_coeff(l);
    let (cb, rest_b) = extract_coeff(r);
    if rest_a == rest_b {
        return coeff_times(ca - cb, rest_a);
    }
    SymbolicExpr::Sub(Box::new(l.clone()), Box::new(r.clone()))
}

/// 提取表达式的常数系数与剩余部分：(c, rest) 使得 expr == c * rest。
fn extract_coeff(expr: &SymbolicExpr) -> (f64, SymbolicExpr) {
    match expr {
        SymbolicExpr::Const(c) => (*c, SymbolicExpr::Const(1.0)),
        SymbolicExpr::Mul(l, r) => {
            if let SymbolicExpr::Const(c) = l.as_ref() {
                return (*c, r.as_ref().clone());
            }
            if let SymbolicExpr::Const(c) = r.as_ref() {
                return (*c, l.as_ref().clone());
            }
            (1.0, expr.clone())
        }
        _ => (1.0, expr.clone()),
    }
}

/// 构造 `coeff * rest`，应用化简：0→0、1→rest。
fn coeff_times(coeff: f64, rest: SymbolicExpr) -> SymbolicExpr {
    if coeff == 0.0 {
        return SymbolicExpr::Const(0.0);
    }
    if coeff == 1.0 {
        return rest;
    }
    SymbolicExpr::Mul(Box::new(SymbolicExpr::Const(coeff)), Box::new(rest))
}

fn simplify_mul(l: &SymbolicExpr, r: &SymbolicExpr) -> SymbolicExpr {
    if let (Some(a), Some(b)) = (l.as_const(), r.as_const()) {
        return SymbolicExpr::Const(a * b);
    }
    if l.is_zero() || r.is_zero() {
        return SymbolicExpr::Const(0.0);
    }
    if l.is_one() {
        return r.clone();
    }
    if r.is_one() {
        return l.clone();
    }
    SymbolicExpr::Mul(Box::new(l.clone()), Box::new(r.clone()))
}

fn simplify_div(l: &SymbolicExpr, r: &SymbolicExpr) -> SymbolicExpr {
    if let (Some(a), Some(b)) = (l.as_const(), r.as_const()) {
        if b == 0.0 {
            return SymbolicExpr::Div(Box::new(l.clone()), Box::new(r.clone()));
        }
        return SymbolicExpr::Const(a / b);
    }
    if r.is_one() {
        return l.clone();
    }
    if l.is_zero() {
        return SymbolicExpr::Const(0.0);
    }
    SymbolicExpr::Div(Box::new(l.clone()), Box::new(r.clone()))
}

fn simplify_pow(l: &SymbolicExpr, r: &SymbolicExpr) -> SymbolicExpr {
    if let (Some(a), Some(b)) = (l.as_const(), r.as_const()) {
        let val = a.powf(b);
        if val.is_finite() {
            return SymbolicExpr::Const(val);
        }
        return SymbolicExpr::Pow(Box::new(l.clone()), Box::new(r.clone()));
    }
    if r.is_zero() {
        return SymbolicExpr::Const(1.0);
    }
    if r.is_one() {
        return l.clone();
    }
    if l.is_one() {
        return SymbolicExpr::Const(1.0);
    }
    SymbolicExpr::Pow(Box::new(l.clone()), Box::new(r.clone()))
}

fn simplify_neg(e: &SymbolicExpr) -> SymbolicExpr {
    if let Some(v) = e.as_const() {
        return SymbolicExpr::Const(-v);
    }
    if let SymbolicExpr::Neg(inner) = e {
        return (**inner).clone();
    }
    SymbolicExpr::Neg(Box::new(e.clone()))
}

// ============================ 极限 limit (TG3.5) ============================

/// 符号极限 `limit(expr, var, point)`。
///
/// 策略：
/// 1. 直接代入：将 var 替换为 point，数值求值。若得到有限值则返回。
/// 2. 洛必达法则：若 expr 为 Div(num, den) 且代入得 0/0 或 ∞/∞，
///    对分子分母求导后递归（深度限制 5 次）。
pub fn limit(expr: &SymbolicExpr, var: &str, point: f64) -> Result<EvalResult, CalcError> {
    limit_recursive(expr, var, point, 0)
}

fn limit_recursive(
    expr: &SymbolicExpr,
    var: &str,
    point: f64,
    depth: usize,
) -> Result<EvalResult, CalcError> {
    const MAX_LOPITAL_DEPTH: usize = 5;

    // 尝试直接代入
    let mut env = HashMap::new();
    env.insert(var.to_string(), point);
    match eval_symbolic(expr, &env) {
        Ok(v) if v.is_finite() => return Ok(EvalResult::Scalar(v)),
        _ => {}
    }

    // 0/0 或 ∞/∞ → 洛必达
    if depth < MAX_LOPITAL_DEPTH {
        if let SymbolicExpr::Div(num, den) = expr {
            let d_num = diff(num, var);
            let d_den = diff(den, var);
            if d_den.is_zero() {
                return Err(CalcError::domain(
                    "limit(): denominator derivative is zero, cannot apply L'Hôpital".to_string(),
                ));
            }
            return limit_recursive(
                &SymbolicExpr::Div(Box::new(d_num), Box::new(d_den)),
                var,
                point,
                depth + 1,
            );
        }
    }

    Err(CalcError::domain(format!(
        "limit() could not resolve indeterminate form (depth {})",
        depth
    )))
}

/// 数值求值 [`SymbolicExpr`]。
pub fn eval_symbolic(expr: &SymbolicExpr, env: &HashMap<String, f64>) -> Result<f64, CalcError> {
    match expr {
        SymbolicExpr::Const(n) => Ok(*n),
        SymbolicExpr::Var(name) => env.get(name).copied().ok_or_else(|| {
            CalcError::eval(format!("unbound variable: {}", name))
        }),
        SymbolicExpr::Add(l, r) => {
            let r = eval_symbolic(l, env)? + eval_symbolic(r, env)?;
            check_finite(r)
        }
        SymbolicExpr::Sub(l, r) => {
            let r = eval_symbolic(l, env)? - eval_symbolic(r, env)?;
            check_finite(r)
        }
        SymbolicExpr::Mul(l, r) => {
            let r = eval_symbolic(l, env)? * eval_symbolic(r, env)?;
            check_finite(r)
        }
        SymbolicExpr::Div(l, r) => eval_div(l, r, env),
        SymbolicExpr::Pow(l, r) => {
            let base = eval_symbolic(l, env)?;
            let exp = eval_symbolic(r, env)?;
            if base == 0.0 && exp == 0.0 {
                return Ok(1.0);
            }
            let r = base.powf(exp);
            check_finite(r)
        }
        SymbolicExpr::Neg(e) => Ok(-eval_symbolic(e, env)?),
        SymbolicExpr::Sin(e) => {
            let r = eval_symbolic(e, env)?.sin();
            check_finite(r)
        }
        SymbolicExpr::Cos(e) => {
            let r = eval_symbolic(e, env)?.cos();
            check_finite(r)
        }
        SymbolicExpr::Tan(e) => {
            let r = eval_symbolic(e, env)?.tan();
            check_finite(r)
        }
        SymbolicExpr::Ln(e) => eval_ln(e, env),
        SymbolicExpr::Exp(e) => {
            let r = eval_symbolic(e, env)?.exp();
            check_finite(r)
        }
    }
}

fn check_finite(v: f64) -> Result<f64, CalcError> {
    if !v.is_finite() {
        return Err(CalcError::nan_or_inf());
    }
    Ok(v)
}

fn eval_div(
    l: &SymbolicExpr,
    r: &SymbolicExpr,
    env: &HashMap<String, f64>,
) -> Result<f64, CalcError> {
    let d = eval_symbolic(r, env)?;
    if d == 0.0 {
        return Err(CalcError::division_by_zero());
    }
    Ok(eval_symbolic(l, env)? / d)
}

fn eval_ln(e: &SymbolicExpr, env: &HashMap<String, f64>) -> Result<f64, CalcError> {
    let v = eval_symbolic(e, env)?;
    if v <= 0.0 {
        return Err(CalcError::domain(format!(
            "ln requires positive argument, got {}",
            v
        )));
    }
    Ok(v.ln())
}

// ============================ 泰勒级数 taylor (TG3.6) ============================

/// 泰勒级数 `taylor(expr, var, order)`。
///
/// 在 point=0 处展开（Maclaurin 级数）。
pub fn taylor(expr: &SymbolicExpr, var: &str, order: u32) -> Result<EvalResult, CalcError> {
    if order > 20 {
        return Err(CalcError::domain(format!(
            "taylor() order {} exceeds maximum of 20",
            order
        )));
    }

    let mut terms: Vec<String> = Vec::new();
    let mut current = expr.clone();

    for k in 0..=order {
        let mut env = HashMap::new();
        env.insert(var.to_string(), 0.0);
        let f_k = eval_symbolic(&current, &env)?;

        if f_k != 0.0 && f_k.is_finite() {
            let coeff = f_k / factorial(k);
            let term = format_taylor_term(coeff, var, k);
            terms.push(term);
        }

        if k < order {
            current = diff(&current, var);
        }
    }

    if terms.is_empty() {
        return Ok(EvalResult::Symbolic("0".to_string()));
    }
    Ok(EvalResult::Symbolic(terms.join("+")))
}

fn factorial(n: u32) -> f64 {
    let mut result = 1.0;
    for i in 2..=n {
        result *= i as f64;
    }
    result
}

fn format_taylor_term(coeff: f64, var: &str, k: u32) -> String {
    let c = format_number(coeff);
    match k {
        0 => c,
        1 => {
            if coeff == 1.0 {
                var.to_string()
            } else {
                format!("{}*{}", c, var)
            }
        }
        _ => {
            if coeff == 1.0 {
                format!("{}^{}", var, k)
            } else {
                format!("{}*{}^{}", c, var, k)
            }
        }
    }
}

// ============================ 单元测试 ============================

#[cfg(test)]
mod tests {
    use super::*;

    fn var(name: &str) -> SymbolicExpr {
        SymbolicExpr::Var(name.to_string())
    }

    fn const_(v: f64) -> SymbolicExpr {
        SymbolicExpr::Const(v)
    }

    // --- SymbolicExpr helpers ---

    #[test]
    fn test_as_const() {
        assert_eq!(const_(3.0).as_const(), Some(3.0));
        assert_eq!(var("x").as_const(), None);
    }

    #[test]
    fn test_is_zero_is_one() {
        assert!(const_(0.0).is_zero());
        assert!(!const_(1.0).is_zero());
        assert!(const_(1.0).is_one());
        assert!(!const_(0.0).is_one());
    }

    // --- symbolic_to_string ---

    #[test]
    fn test_to_string_const() {
        assert_eq!(symbolic_to_string(&const_(3.0)), "3");
        assert_eq!(symbolic_to_string(&const_(3.14)), "3.14");
    }

    #[test]
    fn test_to_string_var() {
        assert_eq!(symbolic_to_string(&var("x")), "x");
    }

    #[test]
    fn test_to_string_add_sub() {
        let expr = SymbolicExpr::Add(Box::new(var("x")), Box::new(const_(1.0)));
        assert_eq!(symbolic_to_string(&expr), "x+1");

        let expr = SymbolicExpr::Sub(
            Box::new(var("x")),
            Box::new(SymbolicExpr::Add(Box::new(var("y")), Box::new(const_(1.0)))),
        );
        assert_eq!(symbolic_to_string(&expr), "x-(y+1)");
    }

    #[test]
    fn test_to_string_mul_div() {
        let expr = SymbolicExpr::Mul(Box::new(var("x")), Box::new(var("y")));
        assert_eq!(symbolic_to_string(&expr), "x*y");

        let expr = SymbolicExpr::Div(Box::new(var("x")), Box::new(const_(2.0)));
        assert_eq!(symbolic_to_string(&expr), "x/2");
    }

    #[test]
    fn test_to_string_pow() {
        let expr = SymbolicExpr::Pow(Box::new(var("x")), Box::new(const_(2.0)));
        assert_eq!(symbolic_to_string(&expr), "x^2");
    }

    #[test]
    fn test_to_string_neg() {
        let expr = SymbolicExpr::Neg(Box::new(var("x")));
        assert_eq!(symbolic_to_string(&expr), "-x");

        let expr = SymbolicExpr::Neg(Box::new(SymbolicExpr::Add(
            Box::new(var("x")),
            Box::new(var("y")),
        )));
        assert_eq!(symbolic_to_string(&expr), "-(x+y)");
    }

    #[test]
    fn test_to_string_functions() {
        assert_eq!(
            symbolic_to_string(&SymbolicExpr::Sin(Box::new(var("x")))),
            "sin(x)"
        );
        assert_eq!(
            symbolic_to_string(&SymbolicExpr::Cos(Box::new(var("x")))),
            "cos(x)"
        );
        assert_eq!(
            symbolic_to_string(&SymbolicExpr::Tan(Box::new(var("x")))),
            "tan(x)"
        );
        assert_eq!(
            symbolic_to_string(&SymbolicExpr::Ln(Box::new(var("x")))),
            "ln(x)"
        );
        assert_eq!(
            symbolic_to_string(&SymbolicExpr::Exp(Box::new(var("x")))),
            "exp(x)"
        );
    }

    // --- diff ---

    #[test]
    fn test_diff_const() {
        let result = diff(&const_(5.0), "x");
        assert_eq!(result, const_(0.0));
    }

    #[test]
    fn test_diff_var() {
        let result = diff(&var("x"), "x");
        assert_eq!(result, const_(1.0));

        let result = diff(&var("y"), "x");
        assert_eq!(result, const_(0.0));
    }

    #[test]
    fn test_diff_add_sub() {
        let expr = SymbolicExpr::Add(Box::new(var("x")), Box::new(var("x")));
        let result = simplify(&diff(&expr, "x"));
        assert_eq!(result, const_(2.0));
    }

    #[test]
    fn test_diff_mul_power_rule() {
        // d/dx(x^2) = 2*x
        let expr = SymbolicExpr::Pow(Box::new(var("x")), Box::new(const_(2.0)));
        let result = simplify(&diff(&expr, "x"));
        // Should simplify to 2*x (or equivalent)
        let s = symbolic_to_string(&result);
        assert!(s.contains("2") && s.contains("x"), "got: {}", s);
    }

    #[test]
    fn test_diff_sin() {
        // d/dx(sin(x)) = cos(x)
        let expr = SymbolicExpr::Sin(Box::new(var("x")));
        let result = simplify(&diff(&expr, "x"));
        assert_eq!(result, SymbolicExpr::Cos(Box::new(var("x"))));
    }

    #[test]
    fn test_diff_cos() {
        // d/dx(cos(x)) = -sin(x)
        let expr = SymbolicExpr::Cos(Box::new(var("x")));
        let result = simplify(&diff(&expr, "x"));
        assert_eq!(
            result,
            SymbolicExpr::Neg(Box::new(SymbolicExpr::Sin(Box::new(var("x")))))
        );
    }

    #[test]
    fn test_diff_exp() {
        // d/dx(exp(x)) = exp(x)
        let expr = SymbolicExpr::Exp(Box::new(var("x")));
        let result = simplify(&diff(&expr, "x"));
        assert_eq!(result, SymbolicExpr::Exp(Box::new(var("x"))));
    }

    #[test]
    fn test_diff_ln() {
        // d/dx(ln(x)) = 1/x
        let expr = SymbolicExpr::Ln(Box::new(var("x")));
        let result = simplify(&diff(&expr, "x"));
        assert_eq!(
            result,
            SymbolicExpr::Div(Box::new(const_(1.0)), Box::new(var("x")))
        );
    }

    #[test]
    fn test_diff_tan() {
        // d/dx(tan(x)) = 1/cos^2(x)
        let expr = SymbolicExpr::Tan(Box::new(var("x")));
        let result = diff(&expr, "x");
        let s = symbolic_to_string(&simplify(&result));
        assert!(s.contains("cos") && s.contains("2"), "got: {}", s);
    }

    #[test]
    fn test_diff_neg() {
        let expr = SymbolicExpr::Neg(Box::new(var("x")));
        let result = simplify(&diff(&expr, "x"));
        assert_eq!(result, const_(-1.0));
    }

    // --- integrate ---

    #[test]
    fn test_integrate_const() {
        // ∫5 dx = 5*x
        let result = integrate(&const_(5.0), "x").unwrap();
        let s = symbolic_to_string(&simplify(&result));
        assert_eq!(s, "5*x");
    }

    #[test]
    fn test_integrate_var() {
        // ∫x dx = x^2/2
        let result = integrate(&var("x"), "x").unwrap();
        let s = symbolic_to_string(&simplify(&result));
        assert!(s.contains("x^2") && s.contains("/2"), "got: {}", s);
    }

    #[test]
    fn test_integrate_sin() {
        // ∫sin(x) dx = -cos(x)
        let expr = SymbolicExpr::Sin(Box::new(var("x")));
        let result = integrate(&expr, "x").unwrap();
        let s = symbolic_to_string(&simplify(&result));
        assert!(s.contains("cos"), "got: {}", s);
    }

    #[test]
    fn test_integrate_cos() {
        // ∫cos(x) dx = sin(x)
        let expr = SymbolicExpr::Cos(Box::new(var("x")));
        let result = integrate(&expr, "x").unwrap();
        let s = symbolic_to_string(&simplify(&result));
        assert!(s.contains("sin"), "got: {}", s);
    }

    #[test]
    fn test_integrate_exp() {
        // ∫exp(x) dx = exp(x)
        let expr = SymbolicExpr::Exp(Box::new(var("x")));
        let result = integrate(&expr, "x").unwrap();
        let s = symbolic_to_string(&simplify(&result));
        assert!(s.contains("exp"), "got: {}", s);
    }

    #[test]
    fn test_integrate_pow_xn() {
        // ∫x^2 dx = x^3/3
        let expr = SymbolicExpr::Pow(Box::new(var("x")), Box::new(const_(2.0)));
        let result = integrate(&expr, "x").unwrap();
        let s = symbolic_to_string(&simplify(&result));
        assert!(s.contains("x^3"), "got: {}", s);
    }

    #[test]
    fn test_integrate_pow_x_neg1() {
        // ∫x^(-1) dx = ln(x)
        let expr = SymbolicExpr::Pow(Box::new(var("x")), Box::new(const_(-1.0)));
        let result = integrate(&expr, "x").unwrap();
        let s = symbolic_to_string(&simplify(&result));
        assert!(s.contains("ln"), "got: {}", s);
    }

    #[test]
    fn test_integrate_div_1_x() {
        // ∫1/x dx = ln(x)
        let expr = SymbolicExpr::Div(Box::new(const_(1.0)), Box::new(var("x")));
        let result = integrate(&expr, "x").unwrap();
        let s = symbolic_to_string(&simplify(&result));
        assert!(s.contains("ln"), "got: {}", s);
    }

    #[test]
    fn test_integrate_unsupported() {
        // ∫tan(x) → error
        let expr = SymbolicExpr::Tan(Box::new(var("x")));
        assert!(integrate(&expr, "x").is_err());
    }

    #[test]
    fn test_integrate_mul_const_extract() {
        // ∫3*x dx = 3*x^2/2
        let expr = SymbolicExpr::Mul(Box::new(const_(3.0)), Box::new(var("x")));
        let result = integrate(&expr, "x").unwrap();
        let s = symbolic_to_string(&simplify(&result));
        assert!(s.contains("3") && s.contains("x^2"), "got: {}", s);
    }

    // --- simplify ---

    #[test]
    fn test_simplify_const折叠() {
        let expr = SymbolicExpr::Add(Box::new(const_(2.0)), Box::new(const_(3.0)));
        assert_eq!(simplify(&expr), const_(5.0));
    }

    #[test]
    fn test_simplify_zero加() {
        let expr = SymbolicExpr::Add(Box::new(const_(0.0)), Box::new(var("x")));
        assert_eq!(simplify(&expr), var("x"));

        let expr = SymbolicExpr::Add(Box::new(var("x")), Box::new(const_(0.0)));
        assert_eq!(simplify(&expr), var("x"));
    }

    #[test]
    fn test_simplify零乘() {
        let expr = SymbolicExpr::Mul(Box::new(const_(0.0)), Box::new(var("x")));
        assert_eq!(simplify(&expr), const_(0.0));
    }

    #[test]
    fn test_simplify一乘() {
        let expr = SymbolicExpr::Mul(Box::new(const_(1.0)), Box::new(var("x")));
        assert_eq!(simplify(&expr), var("x"));

        let expr = SymbolicExpr::Mul(Box::new(var("x")), Box::new(const_(1.0)));
        assert_eq!(simplify(&expr), var("x"));
    }

    #[test]
    fn test_simplify_pow零() {
        let expr = SymbolicExpr::Pow(Box::new(var("x")), Box::new(const_(0.0)));
        assert_eq!(simplify(&expr), const_(1.0));
    }

    #[test]
    fn test_simplify_pow一() {
        let expr = SymbolicExpr::Pow(Box::new(var("x")), Box::new(const_(1.0)));
        assert_eq!(simplify(&expr), var("x"));
    }

    #[test]
    fn test_simplify_neg_neg() {
        let expr = SymbolicExpr::Neg(Box::new(SymbolicExpr::Neg(Box::new(var("x")))));
        assert_eq!(simplify(&expr), var("x"));
    }

    #[test]
    fn test_simplify_div_by_one() {
        let expr = SymbolicExpr::Div(Box::new(var("x")), Box::new(const_(1.0)));
        assert_eq!(simplify(&expr), var("x"));
    }

    #[test]
    fn test_simplify_combine_like_terms() {
        // 2*x + 3*x = 5*x
        let expr = SymbolicExpr::Add(
            Box::new(SymbolicExpr::Mul(Box::new(const_(2.0)), Box::new(var("x")))),
            Box::new(SymbolicExpr::Mul(Box::new(const_(3.0)), Box::new(var("x")))),
        );
        let result = simplify(&expr);
        assert_eq!(
            result,
            SymbolicExpr::Mul(Box::new(const_(5.0)), Box::new(var("x")))
        );
    }

    // --- limit ---

    #[test]
    fn test_limit_direct_substitution() {
        // lim(x→1) x^2 = 1
        let expr = SymbolicExpr::Pow(Box::new(var("x")), Box::new(const_(2.0)));
        let result = limit(&expr, "x", 1.0).unwrap();
        assert_eq!(result, EvalResult::Scalar(1.0));
    }

    #[test]
    fn test_limit_lopital() {
        // lim(x→0) sin(x)/x = 1 (L'Hôpital)
        let expr = SymbolicExpr::Div(
            Box::new(SymbolicExpr::Sin(Box::new(var("x")))),
            Box::new(var("x")),
        );
        let result = limit(&expr, "x", 0.0).unwrap();
        if let EvalResult::Scalar(v) = result {
            assert!((v - 1.0).abs() < 1e-10, "got: {}", v);
        } else {
            panic!("expected Scalar");
        }
    }

    // --- eval_symbolic ---

    #[test]
    fn test_eval_symbolic_basic() {
        let expr = SymbolicExpr::Add(Box::new(var("x")), Box::new(const_(1.0)));
        let mut env = HashMap::new();
        env.insert("x".to_string(), 5.0);
        assert_eq!(eval_symbolic(&expr, &env).unwrap(), 6.0);
    }

    #[test]
    fn test_eval_symbolic_div_by_zero() {
        let expr = SymbolicExpr::Div(Box::new(const_(1.0)), Box::new(const_(0.0)));
        let env = HashMap::new();
        assert!(eval_symbolic(&expr, &env).is_err());
    }

    #[test]
    fn test_eval_symbolic_ln_negative() {
        let expr = SymbolicExpr::Ln(Box::new(const_(-1.0)));
        let env = HashMap::new();
        assert!(eval_symbolic(&expr, &env).is_err());
    }

    #[test]
    fn test_eval_symbolic_pow_0_0() {
        let expr = SymbolicExpr::Pow(Box::new(const_(0.0)), Box::new(const_(0.0)));
        let env = HashMap::new();
        assert_eq!(eval_symbolic(&expr, &env).unwrap(), 1.0);
    }

    #[test]
    fn test_eval_symbolic_unbound_var() {
        let expr = SymbolicExpr::Var("y".to_string());
        let env = HashMap::new();
        assert!(eval_symbolic(&expr, &env).is_err());
    }

    // --- taylor ---

    #[test]
    fn test_taylor_exp_order3() {
        // exp(x) ≈ 1 + x + x^2/2 + x^3/6
        let expr = SymbolicExpr::Exp(Box::new(var("x")));
        let result = taylor(&expr, "x", 3).unwrap();
        if let EvalResult::Symbolic(s) = result {
            assert!(s.contains("1"), "got: {}", s);
            assert!(s.contains("x"), "got: {}", s);
        } else {
            panic!("expected Symbolic");
        }
    }

    #[test]
    fn test_taylor_order_too_high() {
        let expr = SymbolicExpr::Exp(Box::new(var("x")));
        assert!(taylor(&expr, "x", 21).is_err());
    }

    #[test]
    fn test_taylor_const_zero() {
        // taylor(0, x, 5) = "0"
        let expr = const_(0.0);
        let result = taylor(&expr, "x", 5).unwrap();
        assert_eq!(result, EvalResult::Symbolic("0".to_string()));
    }

    // --- factorial ---

    #[test]
    fn test_factorial() {
        assert_eq!(factorial(0), 1.0);
        assert_eq!(factorial(1), 1.0);
        assert_eq!(factorial(5), 120.0);
    }
}
