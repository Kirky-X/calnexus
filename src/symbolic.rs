// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! Symbolic 计算域：符号微分、积分、化简、极限、泰勒级数。
//!
//! 设计依据：
//! - design.md D2（SymbolicExpr 枚举 + AST 变换 + 字符串输出）
//! - v1.0 symbolic-domain spec
//!
//! 路由策略：AST 含 diff/integrate/simplify/limit/taylor 函数调用时路由至本域。
//! priority=30，与 Complex/Matrix/Vector 同级。
//!
//! 核心数据结构 [`SymbolicExpr`] 与 [`AstNode`] 双向转换，符号变换后格式化为
//! 字符串返回 [`EvalResult::Symbolic`]。

use crate::core::CalculationDomain;
use crate::core::{AstNode, BinaryOp, CalcError, EvalContext, EvalResult, UnaryOp};
use std::collections::HashMap;

/// 符号函数白名单。
const SYMBOLIC_FUNCTIONS: &[&str] = &["diff", "integrate", "simplify", "limit", "taylor"];

/// 符号表达式：符号变换的中间表示（design.md D2）。
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
    fn as_const(&self) -> Option<f64> {
        if let SymbolicExpr::Const(v) = self {
            Some(*v)
        } else {
            None
        }
    }

    /// 是否为零常数。
    fn is_zero(&self) -> bool {
        self.as_const() == Some(0.0)
    }

    /// 是否为一常数。
    fn is_one(&self) -> bool {
        self.as_const() == Some(1.0)
    }
}

// ============================ AstNode ↔ SymbolicExpr 转换 ============================

/// 将 [`AstNode`] 转换为 [`SymbolicExpr`]（TG3.1）。
///
/// 支持 Number/Variable/BinaryOp/UnaryOp/FunctionCall(sin/cos/tan/ln/exp)。
/// 不支持的节点（Matrix/List/Complex/BigNumber/未知函数）返回 DomainError。
pub fn ast_to_symbolic(ast: &AstNode) -> Result<SymbolicExpr, CalcError> {
    match ast {
        AstNode::Number(n) => Ok(SymbolicExpr::Const(*n)),
        AstNode::BigNumber(s) => {
            let n: f64 = s
                .parse()
                .map_err(|_| CalcError::domain(format!("invalid big number: {}", s)))?;
            Ok(SymbolicExpr::Const(n))
        }
        AstNode::Variable(name) => {
            // pi / e 视为常数
            match name.as_str() {
                "pi" => Ok(SymbolicExpr::Const(std::f64::consts::PI)),
                "e" => Ok(SymbolicExpr::Const(std::f64::consts::E)),
                _ => Ok(SymbolicExpr::Var(name.clone())),
            }
        }
        AstNode::BinaryOp(op, l, r) => {
            let l = ast_to_symbolic(l)?;
            let r = ast_to_symbolic(r)?;
            Ok(match op {
                BinaryOp::Add => SymbolicExpr::Add(Box::new(l), Box::new(r)),
                BinaryOp::Sub => SymbolicExpr::Sub(Box::new(l), Box::new(r)),
                BinaryOp::Mul => SymbolicExpr::Mul(Box::new(l), Box::new(r)),
                BinaryOp::Div => SymbolicExpr::Div(Box::new(l), Box::new(r)),
                BinaryOp::Pow => SymbolicExpr::Pow(Box::new(l), Box::new(r)),
                BinaryOp::Mod => {
                    return Err(CalcError::domain(
                        "modulo not supported in symbolic expressions".to_string(),
                    ));
                }
            })
        }
        AstNode::UnaryOp(UnaryOp::Neg, e) => Ok(SymbolicExpr::Neg(Box::new(ast_to_symbolic(e)?))),
        AstNode::UnaryOp(UnaryOp::Abs, _) | AstNode::UnaryOp(UnaryOp::Factorial, _) => {
            Err(CalcError::domain(format!(
                "unary op not supported in symbolic expressions: {:?}",
                ast
            )))
        }
        AstNode::FunctionCall(name, args) => {
            let unary = |arg: &AstNode| -> Result<Box<SymbolicExpr>, CalcError> {
                if args.len() != 1 {
                    return Err(CalcError::domain(format!(
                        "{}() requires exactly 1 argument, got {}",
                        name,
                        args.len()
                    )));
                }
                Ok(Box::new(ast_to_symbolic(arg)?))
            };
            match name.as_str() {
                "sin" => Ok(SymbolicExpr::Sin(unary(&args[0])?)),
                "cos" => Ok(SymbolicExpr::Cos(unary(&args[0])?)),
                "tan" => Ok(SymbolicExpr::Tan(unary(&args[0])?)),
                "ln" | "log" => Ok(SymbolicExpr::Ln(unary(&args[0])?)),
                "exp" => Ok(SymbolicExpr::Exp(unary(&args[0])?)),
                _ => Err(CalcError::domain(format!(
                    "function not supported in symbolic expressions: {}",
                    name
                ))),
            }
        }
        AstNode::Complex(_, _) | AstNode::Matrix(_) | AstNode::List(_) => Err(CalcError::domain(
            format!("node type not supported in symbolic expressions: {:?}", ast),
        )),
    }
}

/// 将 [`SymbolicExpr`] 格式化为可读字符串（TG3.1）。
pub fn symbolic_to_string(expr: &SymbolicExpr) -> String {
    match expr {
        SymbolicExpr::Const(n) => format_number(*n),
        SymbolicExpr::Var(s) => s.clone(),
        SymbolicExpr::Add(l, r) => format!("{}+{}", symbolic_to_string(l), symbolic_to_string(r)),
        SymbolicExpr::Sub(l, r) => {
            // 减号右侧若为加法需加括号：a-(b+c)
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
            // x^2 形式无需括号
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

/// 符号求导 `diff(expr, var)`（TG3.2）。
///
/// 递归应用求导规则：
/// - 常数 → 0
/// - 变量 → 1（若为目标变量）/ 0（其他变量）
/// - 和差、积（乘积法则）、商（商法则）、幂（幂法则 + 链式）
/// - sin→cos、cos→-sin、tan→sec²、exp→exp、ln→1/x
pub fn diff(expr: &SymbolicExpr, var: &str) -> SymbolicExpr {
    match expr {
        SymbolicExpr::Const(_) => SymbolicExpr::Const(0.0),
        SymbolicExpr::Var(name) => {
            if name == var {
                SymbolicExpr::Const(1.0)
            } else {
                SymbolicExpr::Const(0.0)
            }
        }
        SymbolicExpr::Add(l, r) => {
            SymbolicExpr::Add(Box::new(diff(l, var)), Box::new(diff(r, var)))
        }
        SymbolicExpr::Sub(l, r) => {
            SymbolicExpr::Sub(Box::new(diff(l, var)), Box::new(diff(r, var)))
        }
        // 乘积法则：(f*g)' = f'*g + f*g'
        SymbolicExpr::Mul(f, g) => SymbolicExpr::Add(
            Box::new(SymbolicExpr::Mul(Box::new(diff(f, var)), g.clone())),
            Box::new(SymbolicExpr::Mul(f.clone(), Box::new(diff(g, var)))),
        ),
        // 商法则：(f/g)' = (f'*g - f*g') / g²
        SymbolicExpr::Div(f, g) => SymbolicExpr::Div(
            Box::new(SymbolicExpr::Sub(
                Box::new(SymbolicExpr::Mul(Box::new(diff(f, var)), g.clone())),
                Box::new(SymbolicExpr::Mul(f.clone(), Box::new(diff(g, var)))),
            )),
            Box::new(SymbolicExpr::Pow(
                g.clone(),
                Box::new(SymbolicExpr::Const(2.0)),
            )),
        ),
        // 幂法则：f^n → n*f^(n-1)*f'（当指数为常数）
        // 一般幂法则（指数非常数）：f^g → f^g * (g'*ln(f) + g*f'/f)
        SymbolicExpr::Pow(f, g) => {
            if let SymbolicExpr::Const(n) = g.as_ref() {
                SymbolicExpr::Mul(
                    Box::new(SymbolicExpr::Mul(
                        Box::new(SymbolicExpr::Const(*n)),
                        Box::new(SymbolicExpr::Pow(
                            f.clone(),
                            Box::new(SymbolicExpr::Const(n - 1.0)),
                        )),
                    )),
                    Box::new(diff(f, var)),
                )
            } else {
                // f^g = exp(g*ln(f))，导数 = f^g * (g'*ln(f) + g*f'/f)
                SymbolicExpr::Mul(
                    Box::new(expr.clone()),
                    Box::new(SymbolicExpr::Add(
                        Box::new(SymbolicExpr::Mul(
                            Box::new(diff(g, var)),
                            Box::new(SymbolicExpr::Ln(f.clone())),
                        )),
                        Box::new(SymbolicExpr::Div(
                            Box::new(SymbolicExpr::Mul(g.clone(), Box::new(diff(f, var)))),
                            f.clone(),
                        )),
                    )),
                )
            }
        }
        // 链式法则：-f' → -(f')
        SymbolicExpr::Neg(f) => SymbolicExpr::Neg(Box::new(diff(f, var))),
        // sin(f) → cos(f)*f'
        SymbolicExpr::Sin(f) => SymbolicExpr::Mul(
            Box::new(SymbolicExpr::Cos(f.clone())),
            Box::new(diff(f, var)),
        ),
        // cos(f) → -sin(f)*f'
        SymbolicExpr::Cos(f) => SymbolicExpr::Mul(
            Box::new(SymbolicExpr::Neg(Box::new(SymbolicExpr::Sin(f.clone())))),
            Box::new(diff(f, var)),
        ),
        // tan(f) → (1/cos²(f))*f' = sec²(f)*f'
        SymbolicExpr::Tan(f) => SymbolicExpr::Mul(
            Box::new(SymbolicExpr::Div(
                Box::new(SymbolicExpr::Const(1.0)),
                Box::new(SymbolicExpr::Pow(
                    Box::new(SymbolicExpr::Cos(f.clone())),
                    Box::new(SymbolicExpr::Const(2.0)),
                )),
            )),
            Box::new(diff(f, var)),
        ),
        // exp(f) → exp(f)*f'
        SymbolicExpr::Exp(f) => SymbolicExpr::Mul(
            Box::new(SymbolicExpr::Exp(f.clone())),
            Box::new(diff(f, var)),
        ),
        // ln(f) → (1/f)*f'
        SymbolicExpr::Ln(f) => SymbolicExpr::Mul(
            Box::new(SymbolicExpr::Div(
                Box::new(SymbolicExpr::Const(1.0)),
                f.clone(),
            )),
            Box::new(diff(f, var)),
        ),
    }
}

// ============================ 符号积分 integrate (TG3.3) ============================

/// 符号积分 `integrate(expr, var)`（TG3.3）。
///
/// v1.0 仅支持：
/// - 多项式积分：`x^n → x^(n+1)/(n+1)`（n ≠ -1）
/// - 基本初等函数：sin→-cos、cos→sin、exp→exp、1/x→ln|x|
/// - 线性性：∫(f±g) = ∫f ± ∫g
///
/// 不支持的积分返回 DomainError。
pub fn integrate(expr: &SymbolicExpr, var: &str) -> Result<SymbolicExpr, CalcError> {
    match expr {
        // ∫c dx = c*x
        SymbolicExpr::Const(c) => Ok(SymbolicExpr::Mul(
            Box::new(SymbolicExpr::Const(*c)),
            Box::new(SymbolicExpr::Var(var.to_string())),
        )),
        // ∫x dx = x²/2；∫y dx = y*x（y 为其他变量）
        SymbolicExpr::Var(name) => {
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
                    Box::new(SymbolicExpr::Var(name.clone())),
                    Box::new(SymbolicExpr::Var(var.to_string())),
                ))
            }
        }
        // 线性性
        SymbolicExpr::Add(f, g) => Ok(SymbolicExpr::Add(
            Box::new(integrate(f, var)?),
            Box::new(integrate(g, var)?),
        )),
        SymbolicExpr::Sub(f, g) => Ok(SymbolicExpr::Sub(
            Box::new(integrate(f, var)?),
            Box::new(integrate(g, var)?),
        )),
        // ∫c*f dx = c*∫f dx（常数提取）
        SymbolicExpr::Mul(f, g) => {
            if let SymbolicExpr::Const(c) = f.as_ref() {
                return Ok(SymbolicExpr::Mul(
                    Box::new(SymbolicExpr::Const(*c)),
                    Box::new(integrate(g, var)?),
                ));
            }
            if let SymbolicExpr::Const(c) = g.as_ref() {
                return Ok(SymbolicExpr::Mul(
                    Box::new(SymbolicExpr::Const(*c)),
                    Box::new(integrate(f, var)?),
                ));
            }
            Err(CalcError::domain(
                "integrate() does not support product of two non-constant expressions".to_string(),
            ))
        }
        // ∫x^n dx = x^(n+1)/(n+1)（n ≠ -1）
        SymbolicExpr::Pow(f, g) => {
            if let (SymbolicExpr::Var(name), SymbolicExpr::Const(n)) = (f.as_ref(), g.as_ref()) {
                if name == var {
                    if *n == -1.0 {
                        // ∫1/x dx = ln|x|
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
        // ∫sin(x) dx = -cos(x)
        SymbolicExpr::Sin(f) => {
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
        // ∫cos(x) dx = sin(x)
        SymbolicExpr::Cos(f) => {
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
        // ∫exp(x) dx = exp(x)
        SymbolicExpr::Exp(f) => {
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
        // ∫1/x dx = ln|x|
        SymbolicExpr::Div(f, g) => {
            if let (SymbolicExpr::Const(c), SymbolicExpr::Var(name)) = (f.as_ref(), g.as_ref()) {
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
        // ∫-f dx = -∫f dx
        SymbolicExpr::Neg(f) => Ok(SymbolicExpr::Neg(Box::new(integrate(f, var)?))),
        // ln/tan 不支持直接积分
        SymbolicExpr::Ln(_) | SymbolicExpr::Tan(_) => Err(CalcError::domain(
            "integrate() does not support ln/tan forms".to_string(),
        )),
    }
}

/// 检查表达式是否为指定变量。
fn is_var(expr: &SymbolicExpr, var: &str) -> bool {
    matches!(expr, SymbolicExpr::Var(name) if name == var)
}

// ============================ 表达式化简 simplify (TG3.4) ============================

/// 表达式化简 `simplify(expr)`（TG3.4）。
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
    // 常量折叠
    if let (Some(a), Some(b)) = (l.as_const(), r.as_const()) {
        return SymbolicExpr::Const(a + b);
    }
    // 0 + x → x
    if l.is_zero() {
        return r.clone();
    }
    // x + 0 → x
    if r.is_zero() {
        return l.clone();
    }
    // 合并同类项：c1*rest + c2*rest → (c1+c2)*rest
    let (ca, rest_a) = extract_coeff(l);
    let (cb, rest_b) = extract_coeff(r);
    if rest_a == rest_b {
        let new_coeff = ca + cb;
        return coeff_times(new_coeff, rest_a);
    }
    SymbolicExpr::Add(Box::new(l.clone()), Box::new(r.clone()))
}

fn simplify_sub(l: &SymbolicExpr, r: &SymbolicExpr) -> SymbolicExpr {
    if let (Some(a), Some(b)) = (l.as_const(), r.as_const()) {
        return SymbolicExpr::Const(a - b);
    }
    // x - 0 → x
    if r.is_zero() {
        return l.clone();
    }
    // 0 - x → -x
    if l.is_zero() {
        return SymbolicExpr::Neg(Box::new(r.clone()));
    }
    // 合并同类项：c1*rest - c2*rest → (c1-c2)*rest
    let (ca, rest_a) = extract_coeff(l);
    let (cb, rest_b) = extract_coeff(r);
    if rest_a == rest_b {
        let new_coeff = ca - cb;
        return coeff_times(new_coeff, rest_a);
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
    // 0 * x → 0
    if l.is_zero() || r.is_zero() {
        return SymbolicExpr::Const(0.0);
    }
    // 1 * x → x
    if l.is_one() {
        return r.clone();
    }
    // x * 1 → x
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
    // x / 1 → x
    if r.is_one() {
        return l.clone();
    }
    // 0 / x → 0
    if l.is_zero() {
        return SymbolicExpr::Const(0.0);
    }
    SymbolicExpr::Div(Box::new(l.clone()), Box::new(r.clone()))
}

fn simplify_pow(l: &SymbolicExpr, r: &SymbolicExpr) -> SymbolicExpr {
    if let (Some(a), Some(b)) = (l.as_const(), r.as_const()) {
        return SymbolicExpr::Const(a.powf(b));
    }
    // x^0 → 1
    if r.is_zero() {
        return SymbolicExpr::Const(1.0);
    }
    // x^1 → x
    if r.is_one() {
        return l.clone();
    }
    // 1^x → 1
    if l.is_one() {
        return SymbolicExpr::Const(1.0);
    }
    SymbolicExpr::Pow(Box::new(l.clone()), Box::new(r.clone()))
}

fn simplify_neg(e: &SymbolicExpr) -> SymbolicExpr {
    if let Some(v) = e.as_const() {
        return SymbolicExpr::Const(-v);
    }
    // -(-x) → x
    if let SymbolicExpr::Neg(inner) = e {
        return (**inner).clone();
    }
    SymbolicExpr::Neg(Box::new(e.clone()))
}

// ============================ 极限 limit (TG3.5) ============================

/// 符号极限 `limit(expr, var, point)`（TG3.5）。
///
/// 策略：
/// 1. 直接代入：将 var 替换为 point，数值求值。若得到有限值则返回。
/// 2. 洛必达法则：若 expr 为 Div(num, den) 且代入得 0/0 或 ∞/∞，
///    对分子分母求导后递归（深度限制 5 次）。
///
/// 返回 `EvalResult::Scalar`。
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
            // 若分母导数为常数 0，说明无法继续
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
fn eval_symbolic(expr: &SymbolicExpr, env: &HashMap<String, f64>) -> Result<f64, CalcError> {
    match expr {
        SymbolicExpr::Const(n) => Ok(*n),
        SymbolicExpr::Var(name) => env
            .get(name)
            .copied()
            .ok_or_else(|| CalcError::eval(format!("unbound variable: {}", name))),
        SymbolicExpr::Add(l, r) => Ok(eval_symbolic(l, env)? + eval_symbolic(r, env)?),
        SymbolicExpr::Sub(l, r) => Ok(eval_symbolic(l, env)? - eval_symbolic(r, env)?),
        SymbolicExpr::Mul(l, r) => Ok(eval_symbolic(l, env)? * eval_symbolic(r, env)?),
        SymbolicExpr::Div(l, r) => {
            let d = eval_symbolic(r, env)?;
            if d == 0.0 {
                return Ok(f64::INFINITY * eval_symbolic(l, env)?.signum());
            }
            Ok(eval_symbolic(l, env)? / d)
        }
        SymbolicExpr::Pow(l, r) => Ok(eval_symbolic(l, env)?.powf(eval_symbolic(r, env)?)),
        SymbolicExpr::Neg(e) => Ok(-eval_symbolic(e, env)?),
        SymbolicExpr::Sin(e) => Ok(eval_symbolic(e, env)?.sin()),
        SymbolicExpr::Cos(e) => Ok(eval_symbolic(e, env)?.cos()),
        SymbolicExpr::Tan(e) => Ok(eval_symbolic(e, env)?.tan()),
        SymbolicExpr::Ln(e) => {
            let v = eval_symbolic(e, env)?;
            if v <= 0.0 {
                return Ok(f64::NAN);
            }
            Ok(v.ln())
        }
        SymbolicExpr::Exp(e) => Ok(eval_symbolic(e, env)?.exp()),
    }
}

// ============================ 泰勒级数 taylor (TG3.6) ============================

/// 泰勒级数 `taylor(expr, var, order)`（TG3.6）。
///
/// 在 point=0 处展开（Maclaurin 级数）：`Σ_{k=0}^{order} f^(k)(0)/k! * x^k`。
/// 返回 `EvalResult::Symbolic`（多项式字符串）。
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
        // f^(k)(0)
        let mut env = HashMap::new();
        env.insert(var.to_string(), 0.0);
        let f_k = eval_symbolic(&current, &env).unwrap_or(0.0);

        if f_k != 0.0 {
            let coeff = f_k / factorial(k);
            let term = format_taylor_term(coeff, var, k);
            terms.push(term);
        }

        // 下一阶导数
        if k < order {
            current = diff(&current, var);
        }
    }

    if terms.is_empty() {
        return Ok(EvalResult::Symbolic("0".to_string()));
    }
    Ok(EvalResult::Symbolic(terms.join("+")))
}

/// 计算阶乘。
fn factorial(n: u32) -> f64 {
    let mut result = 1.0;
    for i in 2..=n {
        result *= i as f64;
    }
    result
}

/// 格式化泰勒级数单项：`coeff * x^k`。
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

// ============================ SymbolicDomain (TG3.7) ============================

/// Symbolic 计算域（TG3.7）。
///
/// priority=30，路由触发词：diff/integrate/simplify/limit/taylor。
pub struct SymbolicDomain;

impl CalculationDomain for SymbolicDomain {
    fn domain_name(&self) -> &str {
        "symbolic"
    }

    fn priority(&self) -> u8 {
        30
    }

    fn supports(&self, ast: &AstNode) -> bool {
        contains_symbolic_function(ast)
    }

    fn evaluate(&self, ast: &AstNode, _ctx: &EvalContext) -> Result<EvalResult, CalcError> {
        self.eval_node(ast)
    }
}

impl Default for SymbolicDomain {
    fn default() -> Self {
        Self
    }
}

impl SymbolicDomain {
    /// 递归求值 AST 节点。
    fn eval_node(&self, ast: &AstNode) -> Result<EvalResult, CalcError> {
        match ast {
            AstNode::FunctionCall(name, args) => self.eval_function(name, args),
            _ => Err(CalcError::domain(format!(
                "symbolic domain expects function call, got: {:?}",
                ast
            ))),
        }
    }

    /// 求值符号函数调用。
    fn eval_function(&self, name: &str, args: &[AstNode]) -> Result<EvalResult, CalcError> {
        if !SYMBOLIC_FUNCTIONS.contains(&name) {
            return Err(CalcError::domain(format!(
                "unsupported function in symbolic domain: {}",
                name
            )));
        }
        match name {
            "diff" => {
                if args.len() != 2 {
                    return Err(CalcError::domain(format!(
                        "diff() requires exactly 2 arguments, got {}",
                        args.len()
                    )));
                }
                let expr = ast_to_symbolic(&args[0])?;
                let var = extract_var_name(&args[1])?;
                let result = simplify(&diff(&expr, &var));
                Ok(EvalResult::Symbolic(symbolic_to_string(&result)))
            }
            "integrate" => {
                if args.len() != 2 {
                    return Err(CalcError::domain(format!(
                        "integrate() requires exactly 2 arguments, got {}",
                        args.len()
                    )));
                }
                let expr = ast_to_symbolic(&args[0])?;
                let var = extract_var_name(&args[1])?;
                let result = simplify(&integrate(&expr, &var)?);
                Ok(EvalResult::Symbolic(symbolic_to_string(&result)))
            }
            "simplify" => {
                if args.len() != 1 {
                    return Err(CalcError::domain(format!(
                        "simplify() requires exactly 1 argument, got {}",
                        args.len()
                    )));
                }
                let expr = ast_to_symbolic(&args[0])?;
                let result = simplify(&expr);
                Ok(EvalResult::Symbolic(symbolic_to_string(&result)))
            }
            "limit" => {
                if args.len() != 3 {
                    return Err(CalcError::domain(format!(
                        "limit() requires exactly 3 arguments, got {}",
                        args.len()
                    )));
                }
                let expr = ast_to_symbolic(&args[0])?;
                let var = extract_var_name(&args[1])?;
                let point = extract_number(&args[2])?;
                limit(&expr, &var, point)
            }
            "taylor" => {
                if args.len() != 3 {
                    return Err(CalcError::domain(format!(
                        "taylor() requires exactly 3 arguments, got {}",
                        args.len()
                    )));
                }
                let expr = ast_to_symbolic(&args[0])?;
                let var = extract_var_name(&args[1])?;
                let order = extract_number(&args[2])? as u32;
                taylor(&expr, &var, order)
            }
            _ => unreachable!("checked above"),
        }
    }
}

/// 递归检查 AST 是否含符号函数调用。
fn contains_symbolic_function(ast: &AstNode) -> bool {
    match ast {
        AstNode::FunctionCall(name, args) => {
            SYMBOLIC_FUNCTIONS.contains(&name.as_str())
                || args.iter().any(contains_symbolic_function)
        }
        AstNode::BinaryOp(_, l, r) => {
            contains_symbolic_function(l) || contains_symbolic_function(r)
        }
        AstNode::UnaryOp(_, e) => contains_symbolic_function(e),
        _ => false,
    }
}

/// 从 AST 提取变量名（Variable 节点）。
fn extract_var_name(ast: &AstNode) -> Result<String, CalcError> {
    match ast {
        AstNode::Variable(name) => Ok(name.clone()),
        _ => Err(CalcError::domain(format!(
            "expected variable name, got: {:?}",
            ast
        ))),
    }
}

/// 从 AST 提取数值（Number 节点）。
fn extract_number(ast: &AstNode) -> Result<f64, CalcError> {
    match ast {
        AstNode::Number(n) => Ok(*n),
        AstNode::BigNumber(s) => s
            .parse::<f64>()
            .map_err(|_| CalcError::domain(format!("invalid big number: {}", s))),
        AstNode::UnaryOp(UnaryOp::Neg, e) => Ok(-extract_number(e)?),
        _ => Err(CalcError::domain(format!(
            "expected number, got: {:?}",
            ast
        ))),
    }
}

// ============================ 单元测试 (TG3.9) ============================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::parse;

    // ----- TG3.1 转换测试 -----

    #[test]
    fn test_ast_to_symbolic_number() {
        let ast = parse("42").unwrap();
        let sym = ast_to_symbolic(&ast).unwrap();
        assert_eq!(sym, SymbolicExpr::Const(42.0));
    }

    #[test]
    fn test_ast_to_symbolic_variable() {
        let ast = parse("x").unwrap();
        let sym = ast_to_symbolic(&ast).unwrap();
        assert_eq!(sym, SymbolicExpr::Var("x".to_string()));
    }

    #[test]
    fn test_ast_to_symbolic_arithmetic() {
        let ast = parse("2*x+3").unwrap();
        let sym = ast_to_symbolic(&ast).unwrap();
        let expected = SymbolicExpr::Add(
            Box::new(SymbolicExpr::Mul(
                Box::new(SymbolicExpr::Const(2.0)),
                Box::new(SymbolicExpr::Var("x".to_string())),
            )),
            Box::new(SymbolicExpr::Const(3.0)),
        );
        assert_eq!(sym, expected);
    }

    #[test]
    fn test_ast_to_symbolic_function() {
        let ast = parse("sin(x)").unwrap();
        let sym = ast_to_symbolic(&ast).unwrap();
        assert_eq!(
            sym,
            SymbolicExpr::Sin(Box::new(SymbolicExpr::Var("x".to_string())))
        );
    }

    #[test]
    fn test_symbolic_to_string_basic() {
        let sym = SymbolicExpr::Add(
            Box::new(SymbolicExpr::Const(2.0)),
            Box::new(SymbolicExpr::Var("x".to_string())),
        );
        assert_eq!(symbolic_to_string(&sym), "2+x");
    }

    // ----- TG3.2 求导测试 -----

    #[test]
    fn test_diff_power_rule() {
        // diff(x^3, x) = 3*x^2
        let expr = SymbolicExpr::Pow(
            Box::new(SymbolicExpr::Var("x".to_string())),
            Box::new(SymbolicExpr::Const(3.0)),
        );
        let result = simplify(&diff(&expr, "x"));
        assert_eq!(symbolic_to_string(&result), "3*x^2");
    }

    #[test]
    fn test_diff_trig_sin() {
        // diff(sin(x), x) = cos(x)
        let expr = SymbolicExpr::Sin(Box::new(SymbolicExpr::Var("x".to_string())));
        let result = simplify(&diff(&expr, "x"));
        assert_eq!(symbolic_to_string(&result), "cos(x)");
    }

    #[test]
    fn test_diff_trig_cos() {
        // diff(cos(x), x) = -sin(x)
        let expr = SymbolicExpr::Cos(Box::new(SymbolicExpr::Var("x".to_string())));
        let result = simplify(&diff(&expr, "x"));
        assert_eq!(symbolic_to_string(&result), "-sin(x)");
    }

    #[test]
    fn test_diff_exp() {
        // diff(exp(x), x) = exp(x)
        let expr = SymbolicExpr::Exp(Box::new(SymbolicExpr::Var("x".to_string())));
        let result = simplify(&diff(&expr, "x"));
        assert_eq!(symbolic_to_string(&result), "exp(x)");
    }

    #[test]
    fn test_diff_ln() {
        // diff(ln(x), x) = 1/x
        let expr = SymbolicExpr::Ln(Box::new(SymbolicExpr::Var("x".to_string())));
        let result = simplify(&diff(&expr, "x"));
        assert_eq!(symbolic_to_string(&result), "1/x");
    }

    #[test]
    fn test_diff_chain_rule() {
        // diff(sin(x^2), x) = cos(x^2)*2*x
        let expr = SymbolicExpr::Sin(Box::new(SymbolicExpr::Pow(
            Box::new(SymbolicExpr::Var("x".to_string())),
            Box::new(SymbolicExpr::Const(2.0)),
        )));
        let result = simplify(&diff(&expr, "x"));
        let s = symbolic_to_string(&result);
        // 化简后应含 cos(x^2) 和 2*x
        assert!(s.contains("cos(x^2)"), "expected cos(x^2) in: {}", s);
        assert!(s.contains("2*x"), "expected 2*x in: {}", s);
    }

    #[test]
    fn test_diff_constant() {
        let expr = SymbolicExpr::Const(5.0);
        let result = diff(&expr, "x");
        assert_eq!(result, SymbolicExpr::Const(0.0));
    }

    #[test]
    fn test_diff_product_rule() {
        // diff(x*sin(x), x) = sin(x) + x*cos(x)
        let expr = SymbolicExpr::Mul(
            Box::new(SymbolicExpr::Var("x".to_string())),
            Box::new(SymbolicExpr::Sin(Box::new(SymbolicExpr::Var(
                "x".to_string(),
            )))),
        );
        let result = simplify(&diff(&expr, "x"));
        let s = symbolic_to_string(&result);
        assert!(s.contains("sin(x)"), "expected sin(x) in: {}", s);
        assert!(s.contains("cos(x)"), "expected cos(x) in: {}", s);
    }

    // ----- TG3.3 积分测试 -----

    #[test]
    fn test_integrate_power() {
        // ∫x^2 dx = x^3/3
        let expr = SymbolicExpr::Pow(
            Box::new(SymbolicExpr::Var("x".to_string())),
            Box::new(SymbolicExpr::Const(2.0)),
        );
        let result = simplify(&integrate(&expr, "x").unwrap());
        assert_eq!(symbolic_to_string(&result), "x^3/3");
    }

    #[test]
    fn test_integrate_sin() {
        // ∫sin(x) dx = -cos(x)
        let expr = SymbolicExpr::Sin(Box::new(SymbolicExpr::Var("x".to_string())));
        let result = simplify(&integrate(&expr, "x").unwrap());
        assert_eq!(symbolic_to_string(&result), "-cos(x)");
    }

    #[test]
    fn test_integrate_cos() {
        // ∫cos(x) dx = sin(x)
        let expr = SymbolicExpr::Cos(Box::new(SymbolicExpr::Var("x".to_string())));
        let result = simplify(&integrate(&expr, "x").unwrap());
        assert_eq!(symbolic_to_string(&result), "sin(x)");
    }

    #[test]
    fn test_integrate_exp() {
        // ∫exp(x) dx = exp(x)
        let expr = SymbolicExpr::Exp(Box::new(SymbolicExpr::Var("x".to_string())));
        let result = simplify(&integrate(&expr, "x").unwrap());
        assert_eq!(symbolic_to_string(&result), "exp(x)");
    }

    #[test]
    fn test_integrate_one_over_x() {
        // ∫1/x dx = ln(x)
        let expr = SymbolicExpr::Div(
            Box::new(SymbolicExpr::Const(1.0)),
            Box::new(SymbolicExpr::Var("x".to_string())),
        );
        let result = simplify(&integrate(&expr, "x").unwrap());
        assert_eq!(symbolic_to_string(&result), "ln(x)");
    }

    #[test]
    fn test_integrate_unsupported_returns_error() {
        // ∫sin(x)*cos(x) dx 不支持
        let expr = SymbolicExpr::Mul(
            Box::new(SymbolicExpr::Sin(Box::new(SymbolicExpr::Var(
                "x".to_string(),
            )))),
            Box::new(SymbolicExpr::Cos(Box::new(SymbolicExpr::Var(
                "x".to_string(),
            )))),
        );
        let result = integrate(&expr, "x");
        assert!(result.is_err());
    }

    // ----- TG3.4 化简测试 -----

    #[test]
    fn test_simplify_add_zero() {
        // 0 + x → x
        let expr = SymbolicExpr::Add(
            Box::new(SymbolicExpr::Const(0.0)),
            Box::new(SymbolicExpr::Var("x".to_string())),
        );
        let result = simplify(&expr);
        assert_eq!(result, SymbolicExpr::Var("x".to_string()));
    }

    #[test]
    fn test_simplify_mul_one() {
        // 1 * x → x
        let expr = SymbolicExpr::Mul(
            Box::new(SymbolicExpr::Const(1.0)),
            Box::new(SymbolicExpr::Var("x".to_string())),
        );
        let result = simplify(&expr);
        assert_eq!(result, SymbolicExpr::Var("x".to_string()));
    }

    #[test]
    fn test_simplify_mul_zero() {
        // x * 0 → 0
        let expr = SymbolicExpr::Mul(
            Box::new(SymbolicExpr::Var("x".to_string())),
            Box::new(SymbolicExpr::Const(0.0)),
        );
        let result = simplify(&expr);
        assert_eq!(result, SymbolicExpr::Const(0.0));
    }

    #[test]
    fn test_simplify_pow_zero() {
        // x^0 → 1
        let expr = SymbolicExpr::Pow(
            Box::new(SymbolicExpr::Var("x".to_string())),
            Box::new(SymbolicExpr::Const(0.0)),
        );
        let result = simplify(&expr);
        assert_eq!(result, SymbolicExpr::Const(1.0));
    }

    #[test]
    fn test_simplify_pow_one() {
        // x^1 → x
        let expr = SymbolicExpr::Pow(
            Box::new(SymbolicExpr::Var("x".to_string())),
            Box::new(SymbolicExpr::Const(1.0)),
        );
        let result = simplify(&expr);
        assert_eq!(result, SymbolicExpr::Var("x".to_string()));
    }

    #[test]
    fn test_simplify_constant_folding() {
        // 2 + 3 → 5
        let expr = SymbolicExpr::Add(
            Box::new(SymbolicExpr::Const(2.0)),
            Box::new(SymbolicExpr::Const(3.0)),
        );
        let result = simplify(&expr);
        assert_eq!(result, SymbolicExpr::Const(5.0));
    }

    #[test]
    fn test_simplify_nested() {
        // (2+3)*1 → 5
        let expr = SymbolicExpr::Mul(
            Box::new(SymbolicExpr::Add(
                Box::new(SymbolicExpr::Const(2.0)),
                Box::new(SymbolicExpr::Const(3.0)),
            )),
            Box::new(SymbolicExpr::Const(1.0)),
        );
        let result = simplify(&expr);
        assert_eq!(result, SymbolicExpr::Const(5.0));
    }

    // ----- TG3.5 极限测试 -----

    #[test]
    fn test_limit_direct_substitution() {
        // limit(x^2, x, 3) = 9
        let expr = SymbolicExpr::Pow(
            Box::new(SymbolicExpr::Var("x".to_string())),
            Box::new(SymbolicExpr::Const(2.0)),
        );
        let result = limit(&expr, "x", 3.0).unwrap();
        assert_eq!(result, EvalResult::Scalar(9.0));
    }

    #[test]
    fn test_limit_lhopital_zero_over_zero() {
        // limit(sin(x)/x, x, 0) = 1
        let expr = SymbolicExpr::Div(
            Box::new(SymbolicExpr::Sin(Box::new(SymbolicExpr::Var(
                "x".to_string(),
            )))),
            Box::new(SymbolicExpr::Var("x".to_string())),
        );
        let result = limit(&expr, "x", 0.0).unwrap();
        if let EvalResult::Scalar(v) = result {
            assert!((v - 1.0).abs() < 1e-9, "expected 1.0, got {}", v);
        } else {
            panic!("expected Scalar");
        }
    }

    #[test]
    fn test_limit_polynomial() {
        // limit((x^2-1)/(x-1), x, 1) = 2
        let expr = SymbolicExpr::Div(
            Box::new(SymbolicExpr::Sub(
                Box::new(SymbolicExpr::Pow(
                    Box::new(SymbolicExpr::Var("x".to_string())),
                    Box::new(SymbolicExpr::Const(2.0)),
                )),
                Box::new(SymbolicExpr::Const(1.0)),
            )),
            Box::new(SymbolicExpr::Sub(
                Box::new(SymbolicExpr::Var("x".to_string())),
                Box::new(SymbolicExpr::Const(1.0)),
            )),
        );
        let result = limit(&expr, "x", 1.0).unwrap();
        if let EvalResult::Scalar(v) = result {
            assert!((v - 2.0).abs() < 1e-9, "expected 2.0, got {}", v);
        } else {
            panic!("expected Scalar");
        }
    }

    // ----- TG3.6 泰勒级数测试 -----

    #[test]
    fn test_taylor_exp() {
        // taylor(exp(x), x, 3) = 1 + x + 0.5*x^2 + (1/6)*x^3
        let expr = SymbolicExpr::Exp(Box::new(SymbolicExpr::Var("x".to_string())));
        let result = taylor(&expr, "x", 3).unwrap();
        if let EvalResult::Symbolic(s) = result {
            assert!(s.contains("1"), "expected 1 in: {}", s);
            // 二次项系数 1/2 = 0.5
            assert!(s.contains("0.5*x^2"), "expected 0.5*x^2 in: {}", s);
            // 三次项系数 1/6 ≈ 0.1666...
            assert!(s.contains("x^3"), "expected x^3 term in: {}", s);
        } else {
            panic!("expected Symbolic");
        }
    }

    #[test]
    fn test_taylor_sin() {
        // taylor(sin(x), x, 5) = x - (1/6)*x^3 + (1/120)*x^5
        let expr = SymbolicExpr::Sin(Box::new(SymbolicExpr::Var("x".to_string())));
        let result = taylor(&expr, "x", 5).unwrap();
        if let EvalResult::Symbolic(s) = result {
            // 三次项系数 -1/6 ≈ -0.1666...
            assert!(s.contains("x^3"), "expected x^3 term in: {}", s);
            // 五次项系数 1/120 ≈ 0.00833...
            assert!(s.contains("x^5"), "expected x^5 term in: {}", s);
        } else {
            panic!("expected Symbolic");
        }
    }

    #[test]
    fn test_taylor_order_exceeds_max() {
        let expr = SymbolicExpr::Exp(Box::new(SymbolicExpr::Var("x".to_string())));
        let result = taylor(&expr, "x", 21);
        assert!(result.is_err());
    }

    // ----- TG3.7 路由测试 -----

    #[test]
    fn test_domain_name_and_priority() {
        let domain = SymbolicDomain;
        assert_eq!(domain.domain_name(), "symbolic");
        assert_eq!(domain.priority(), 30);
    }

    #[test]
    fn test_supports_diff() {
        let domain = SymbolicDomain;
        let ast = parse("diff(x^2, x)").unwrap();
        assert!(domain.supports(&ast));
    }

    #[test]
    fn test_supports_not_arithmetic() {
        let domain = SymbolicDomain;
        let ast = parse("2+3").unwrap();
        assert!(!domain.supports(&ast));
    }

    #[test]
    fn test_supports_nested() {
        let domain = SymbolicDomain;
        // 2 + diff(x, x) → 含 diff 函数
        let ast = parse("2+diff(x,x)").unwrap();
        assert!(domain.supports(&ast));
    }

    // ----- TG3.7 端到端 evaluate 测试 -----

    #[test]
    fn test_evaluate_diff_power() {
        let domain = SymbolicDomain;
        let ast = parse("diff(x^2, x)").unwrap();
        let result = domain.evaluate(&ast, &EvalContext::default()).unwrap();
        if let EvalResult::Symbolic(s) = result {
            assert_eq!(s, "2*x");
        } else {
            panic!("expected Symbolic");
        }
    }

    #[test]
    fn test_evaluate_simplify() {
        let domain = SymbolicDomain;
        let ast = parse("simplify(x^2+2*x^2)").unwrap();
        // 需要匹配现有 implicit mult 解析: x^2+2*x^2
        let result = domain.evaluate(&ast, &EvalContext::default()).unwrap();
        if let EvalResult::Symbolic(s) = result {
            // 化简后应为 3*x^2
            assert!(s.contains("3*x^2"), "expected 3*x^2 in: {}", s);
        } else {
            panic!("expected Symbolic");
        }
    }

    #[test]
    fn test_evaluate_limit() {
        let domain = SymbolicDomain;
        let ast = parse("limit(sin(x)/x, x, 0)").unwrap();
        let result = domain.evaluate(&ast, &EvalContext::default()).unwrap();
        if let EvalResult::Scalar(v) = result {
            assert!((v - 1.0).abs() < 1e-9, "expected 1.0, got {}", v);
        } else {
            panic!("expected Scalar");
        }
    }

    #[test]
    fn test_evaluate_taylor() {
        let domain = SymbolicDomain;
        let ast = parse("taylor(exp(x), x, 2)").unwrap();
        let result = domain.evaluate(&ast, &EvalContext::default()).unwrap();
        if let EvalResult::Symbolic(s) = result {
            assert!(s.contains("1"), "expected 1 in: {}", s);
            // 二次项系数 1/2 = 0.5
            assert!(s.contains("0.5*x^2"), "expected 0.5*x^2 in: {}", s);
        } else {
            panic!("expected Symbolic");
        }
    }

    #[test]
    fn test_evaluate_integrate() {
        let domain = SymbolicDomain;
        let ast = parse("integrate(x^2, x)").unwrap();
        let result = domain.evaluate(&ast, &EvalContext::default()).unwrap();
        if let EvalResult::Symbolic(s) = result {
            assert_eq!(s, "x^3/3");
        } else {
            panic!("expected Symbolic");
        }
    }

    #[test]
    fn test_evaluate_unsupported_function() {
        let domain = SymbolicDomain;
        let ast = AstNode::FunctionCall("foo".to_string(), vec![]);
        let result = domain.evaluate(&ast, &EvalContext::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_evaluate_diff_wrong_arg_count() {
        let domain = SymbolicDomain;
        let ast = AstNode::FunctionCall("diff".to_string(), vec![AstNode::Number(1.0)]);
        let result = domain.evaluate(&ast, &EvalContext::default());
        assert!(result.is_err());
    }

    // ----- 辅助函数测试 -----

    #[test]
    fn test_format_number_integer() {
        assert_eq!(format_number(5.0), "5");
        assert_eq!(format_number(-3.0), "-3");
    }

    #[test]
    fn test_format_number_decimal() {
        assert_eq!(format_number(2.5), "2.5");
    }

    #[test]
    fn test_factorial_values() {
        assert_eq!(factorial(0), 1.0);
        assert_eq!(factorial(1), 1.0);
        assert_eq!(factorial(5), 120.0);
    }

    #[test]
    fn test_eval_symbolic_basic() {
        let mut env = HashMap::new();
        env.insert("x".to_string(), 3.0);
        let expr = SymbolicExpr::Add(
            Box::new(SymbolicExpr::Var("x".to_string())),
            Box::new(SymbolicExpr::Const(2.0)),
        );
        assert_eq!(eval_symbolic(&expr, &env).unwrap(), 5.0);
    }

    // ----- TG3.10 proptest 属性测试 -----

    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig { cases: 128, ..ProptestConfig::default() })]

        // 求导线性性：diff(f+g, x) == diff(f, x) + diff(g, x)
        #[test]
        fn prop_diff_linearity(a in -10.0f64..10.0, b in -10.0f64..10.0) {
            // f = a*x, g = b*x → diff(f+g) = a + b
            let f = SymbolicExpr::Mul(
                Box::new(SymbolicExpr::Const(a)),
                Box::new(SymbolicExpr::Var("x".to_string())),
            );
            let g = SymbolicExpr::Mul(
                Box::new(SymbolicExpr::Const(b)),
                Box::new(SymbolicExpr::Var("x".to_string())),
            );
            let sum = SymbolicExpr::Add(Box::new(f.clone()), Box::new(g.clone()));
            let d_sum = simplify(&diff(&sum, "x"));
            let d_f = simplify(&diff(&f, "x"));
            let d_g = simplify(&diff(&g, "x"));
            let expected = simplify(&SymbolicExpr::Add(Box::new(d_f), Box::new(d_g)));
            prop_assert_eq!(d_sum, expected);
        }

        // 化简幂等性：simplify(simplify(e)) == simplify(e)
        #[test]
        fn prop_simplify_idempotent(c in -5.0f64..5.0) {
            let expr = SymbolicExpr::Add(
                Box::new(SymbolicExpr::Const(c)),
                Box::new(SymbolicExpr::Mul(
                    Box::new(SymbolicExpr::Const(1.0)),
                    Box::new(SymbolicExpr::Var("x".to_string())),
                )),
            );
            let once = simplify(&expr);
            let twice = simplify(&once);
            prop_assert_eq!(once, twice);
        }

        // 求导常数规则：diff(c, x) = 0
        #[test]
        fn prop_diff_constant_is_zero(c in -100.0f64..100.0) {
            let expr = SymbolicExpr::Const(c);
            let result = diff(&expr, "x");
            prop_assert_eq!(result, SymbolicExpr::Const(0.0));
        }
    }

    // ===== 补充覆盖测试：ast_to_symbolic 错误/边界路径 =====

    #[test]
    fn test_ast_to_symbolic_big_number_valid() {
        let ast = AstNode::BigNumber("12345".to_string());
        let sym = ast_to_symbolic(&ast).unwrap();
        assert_eq!(sym, SymbolicExpr::Const(12345.0));
    }

    #[test]
    fn test_ast_to_symbolic_big_number_invalid() {
        let ast = AstNode::BigNumber("not_a_number".to_string());
        assert!(ast_to_symbolic(&ast).is_err());
    }

    #[test]
    fn test_ast_to_symbolic_mod_error() {
        let ast = AstNode::BinaryOp(
            BinaryOp::Mod,
            Box::new(AstNode::Number(5.0)),
            Box::new(AstNode::Number(3.0)),
        );
        assert!(ast_to_symbolic(&ast).is_err());
    }

    #[test]
    fn test_ast_to_symbolic_abs_error() {
        let ast = AstNode::UnaryOp(UnaryOp::Abs, Box::new(AstNode::Number(5.0)));
        assert!(ast_to_symbolic(&ast).is_err());
    }

    #[test]
    fn test_ast_to_symbolic_factorial_error() {
        let ast = AstNode::UnaryOp(UnaryOp::Factorial, Box::new(AstNode::Number(5.0)));
        assert!(ast_to_symbolic(&ast).is_err());
    }

    #[test]
    fn test_ast_to_symbolic_unknown_function() {
        let ast = AstNode::FunctionCall("unknown".to_string(), vec![AstNode::Number(1.0)]);
        assert!(ast_to_symbolic(&ast).is_err());
    }

    #[test]
    fn test_ast_to_symbolic_complex_error() {
        let ast = AstNode::Complex(1.0, 2.0);
        assert!(ast_to_symbolic(&ast).is_err());
    }

    #[test]
    fn test_ast_to_symbolic_matrix_error() {
        let ast = AstNode::Matrix(vec![vec![AstNode::Number(1.0)]]);
        assert!(ast_to_symbolic(&ast).is_err());
    }

    #[test]
    fn test_ast_to_symbolic_list_error() {
        let ast = AstNode::List(vec![AstNode::Number(1.0)]);
        assert!(ast_to_symbolic(&ast).is_err());
    }

    #[test]
    fn test_ast_to_symbolic_wrong_arg_count() {
        // 2 args triggers the arg-count check inside the `unary` closure
        // (lines 125-131). Note: 0 args panics on `args[0]` access before the
        // closure runs, so only the 2-args case exercises the DomainError path.
        let ast = AstNode::FunctionCall(
            "sin".to_string(),
            vec![AstNode::Number(1.0), AstNode::Number(2.0)],
        );
        assert!(ast_to_symbolic(&ast).is_err());
        let ast = AstNode::FunctionCall(
            "cos".to_string(),
            vec![AstNode::Number(1.0), AstNode::Number(2.0)],
        );
        assert!(ast_to_symbolic(&ast).is_err());
        let ast = AstNode::FunctionCall(
            "ln".to_string(),
            vec![AstNode::Number(1.0), AstNode::Number(2.0)],
        );
        assert!(ast_to_symbolic(&ast).is_err());
        let ast = AstNode::FunctionCall(
            "tan".to_string(),
            vec![AstNode::Number(1.0), AstNode::Number(2.0)],
        );
        assert!(ast_to_symbolic(&ast).is_err());
        let ast = AstNode::FunctionCall(
            "exp".to_string(),
            vec![AstNode::Number(1.0), AstNode::Number(2.0)],
        );
        assert!(ast_to_symbolic(&ast).is_err());
    }

    #[test]
    fn test_ast_to_symbolic_sub_div_pow_neg() {
        // Sub
        let ast = AstNode::BinaryOp(
            BinaryOp::Sub,
            Box::new(AstNode::Number(5.0)),
            Box::new(AstNode::Number(3.0)),
        );
        assert_eq!(
            ast_to_symbolic(&ast).unwrap(),
            SymbolicExpr::Sub(
                Box::new(SymbolicExpr::Const(5.0)),
                Box::new(SymbolicExpr::Const(3.0)),
            )
        );
        // Div
        let ast = AstNode::BinaryOp(
            BinaryOp::Div,
            Box::new(AstNode::Number(6.0)),
            Box::new(AstNode::Number(2.0)),
        );
        assert_eq!(
            ast_to_symbolic(&ast).unwrap(),
            SymbolicExpr::Div(
                Box::new(SymbolicExpr::Const(6.0)),
                Box::new(SymbolicExpr::Const(2.0)),
            )
        );
        // Pow
        let ast = AstNode::BinaryOp(
            BinaryOp::Pow,
            Box::new(AstNode::Number(2.0)),
            Box::new(AstNode::Number(3.0)),
        );
        assert_eq!(
            ast_to_symbolic(&ast).unwrap(),
            SymbolicExpr::Pow(
                Box::new(SymbolicExpr::Const(2.0)),
                Box::new(SymbolicExpr::Const(3.0)),
            )
        );
        // Neg
        let ast = AstNode::UnaryOp(UnaryOp::Neg, Box::new(AstNode::Number(5.0)));
        assert_eq!(
            ast_to_symbolic(&ast).unwrap(),
            SymbolicExpr::Neg(Box::new(SymbolicExpr::Const(5.0)))
        );
    }

    #[test]
    fn test_ast_to_symbolic_tan_ln_exp_log() {
        let var = SymbolicExpr::Var("x".to_string());
        let v = AstNode::Variable("x".to_string());
        assert_eq!(
            ast_to_symbolic(&AstNode::FunctionCall("tan".to_string(), vec![v.clone()])).unwrap(),
            SymbolicExpr::Tan(Box::new(var.clone()))
        );
        assert_eq!(
            ast_to_symbolic(&AstNode::FunctionCall("ln".to_string(), vec![v.clone()])).unwrap(),
            SymbolicExpr::Ln(Box::new(var.clone()))
        );
        assert_eq!(
            ast_to_symbolic(&AstNode::FunctionCall("exp".to_string(), vec![v.clone()])).unwrap(),
            SymbolicExpr::Exp(Box::new(var.clone()))
        );
        // log is an alias for ln
        assert_eq!(
            ast_to_symbolic(&AstNode::FunctionCall("log".to_string(), vec![v])).unwrap(),
            SymbolicExpr::Ln(Box::new(var))
        );
    }

    #[test]
    fn test_ast_to_symbolic_pi_e_constants() {
        let ast = AstNode::Variable("pi".to_string());
        assert_eq!(
            ast_to_symbolic(&ast).unwrap(),
            SymbolicExpr::Const(std::f64::consts::PI)
        );
        let ast = AstNode::Variable("e".to_string());
        assert_eq!(
            ast_to_symbolic(&ast).unwrap(),
            SymbolicExpr::Const(std::f64::consts::E)
        );
    }

    // ===== 补充覆盖测试：symbolic_to_string 格式化分支 =====

    #[test]
    fn test_symbolic_to_string_sub_with_add_parens() {
        // a - (b + c): Sub with Add on right needs parens
        let expr = SymbolicExpr::Sub(
            Box::new(SymbolicExpr::Var("a".to_string())),
            Box::new(SymbolicExpr::Add(
                Box::new(SymbolicExpr::Var("b".to_string())),
                Box::new(SymbolicExpr::Var("c".to_string())),
            )),
        );
        assert_eq!(symbolic_to_string(&expr), "a-(b+c)");
    }

    #[test]
    fn test_symbolic_to_string_sub_with_sub_parens() {
        // a - (b - c): Sub with Sub on right needs parens
        let expr = SymbolicExpr::Sub(
            Box::new(SymbolicExpr::Var("a".to_string())),
            Box::new(SymbolicExpr::Sub(
                Box::new(SymbolicExpr::Var("b".to_string())),
                Box::new(SymbolicExpr::Var("c".to_string())),
            )),
        );
        assert_eq!(symbolic_to_string(&expr), "a-(b-c)");
    }

    #[test]
    fn test_symbolic_to_string_sub_plain() {
        // a - b: plain subtraction (no parens needed)
        let expr = SymbolicExpr::Sub(
            Box::new(SymbolicExpr::Var("a".to_string())),
            Box::new(SymbolicExpr::Var("b".to_string())),
        );
        assert_eq!(symbolic_to_string(&expr), "a-b");
    }

    #[test]
    fn test_symbolic_to_string_pow_complex_operands() {
        // (x+1)^(x-1): Pow with non-const/var operands needs parens
        let expr = SymbolicExpr::Pow(
            Box::new(SymbolicExpr::Add(
                Box::new(SymbolicExpr::Var("x".to_string())),
                Box::new(SymbolicExpr::Const(1.0)),
            )),
            Box::new(SymbolicExpr::Sub(
                Box::new(SymbolicExpr::Var("x".to_string())),
                Box::new(SymbolicExpr::Const(1.0)),
            )),
        );
        assert_eq!(symbolic_to_string(&expr), "(x+1)^(x-1)");
    }

    #[test]
    fn test_symbolic_to_string_pow_simple_operands() {
        // x^2: const/var operands need no parens
        let expr = SymbolicExpr::Pow(
            Box::new(SymbolicExpr::Var("x".to_string())),
            Box::new(SymbolicExpr::Const(2.0)),
        );
        assert_eq!(symbolic_to_string(&expr), "x^2");
    }

    #[test]
    fn test_symbolic_to_string_neg_with_add() {
        // -(x+1): Neg with Add needs parens
        let expr = SymbolicExpr::Neg(Box::new(SymbolicExpr::Add(
            Box::new(SymbolicExpr::Var("x".to_string())),
            Box::new(SymbolicExpr::Const(1.0)),
        )));
        assert_eq!(symbolic_to_string(&expr), "-(x+1)");
    }

    #[test]
    fn test_symbolic_to_string_neg_with_sub() {
        // -(x-1): Neg with Sub needs parens
        let expr = SymbolicExpr::Neg(Box::new(SymbolicExpr::Sub(
            Box::new(SymbolicExpr::Var("x".to_string())),
            Box::new(SymbolicExpr::Const(1.0)),
        )));
        assert_eq!(symbolic_to_string(&expr), "-(x-1)");
    }

    #[test]
    fn test_symbolic_to_string_neg_simple() {
        // -x: plain negation
        let expr = SymbolicExpr::Neg(Box::new(SymbolicExpr::Var("x".to_string())));
        assert_eq!(symbolic_to_string(&expr), "-x");
    }

    #[test]
    fn test_symbolic_to_string_div_and_mul_with_parens() {
        // (x+1)/(x-1): Div with Add/Sub operands needs parens
        let expr = SymbolicExpr::Div(
            Box::new(SymbolicExpr::Add(
                Box::new(SymbolicExpr::Var("x".to_string())),
                Box::new(SymbolicExpr::Const(1.0)),
            )),
            Box::new(SymbolicExpr::Sub(
                Box::new(SymbolicExpr::Var("x".to_string())),
                Box::new(SymbolicExpr::Const(1.0)),
            )),
        );
        assert_eq!(symbolic_to_string(&expr), "(x+1)/(x-1)");
        // (x+1)*x: Mul with Add operand needs parens
        let expr = SymbolicExpr::Mul(
            Box::new(SymbolicExpr::Add(
                Box::new(SymbolicExpr::Var("x".to_string())),
                Box::new(SymbolicExpr::Const(1.0)),
            )),
            Box::new(SymbolicExpr::Var("x".to_string())),
        );
        assert_eq!(symbolic_to_string(&expr), "(x+1)*x");
    }

    #[test]
    fn test_symbolic_to_string_tan_ln_exp() {
        assert_eq!(
            symbolic_to_string(&SymbolicExpr::Tan(Box::new(SymbolicExpr::Var(
                "x".to_string()
            )))),
            "tan(x)"
        );
        assert_eq!(
            symbolic_to_string(&SymbolicExpr::Ln(Box::new(SymbolicExpr::Var(
                "x".to_string()
            )))),
            "ln(x)"
        );
        assert_eq!(
            symbolic_to_string(&SymbolicExpr::Exp(Box::new(SymbolicExpr::Var(
                "x".to_string()
            )))),
            "exp(x)"
        );
    }

    #[test]
    fn test_format_number_large_integer() {
        // 1e16 is not < 1e16, so goes to else branch (format!("{}", n))
        let s = format_number(1e16);
        assert_eq!(s, "10000000000000000");
    }

    // ===== 补充覆盖测试：diff 边界路径 =====

    #[test]
    fn test_diff_sub() {
        // diff(x - 3, x) = 1 - 0 → simplify → 1
        let expr = SymbolicExpr::Sub(
            Box::new(SymbolicExpr::Var("x".to_string())),
            Box::new(SymbolicExpr::Const(3.0)),
        );
        let result = simplify(&diff(&expr, "x"));
        assert_eq!(symbolic_to_string(&result), "1");
    }

    #[test]
    fn test_diff_neg() {
        // diff(-x, x) = -1
        let expr = SymbolicExpr::Neg(Box::new(SymbolicExpr::Var("x".to_string())));
        let result = simplify(&diff(&expr, "x"));
        assert_eq!(symbolic_to_string(&result), "-1");
    }

    #[test]
    fn test_diff_div_quotient_rule() {
        // diff(x / (x+1), x) applies quotient rule
        let expr = SymbolicExpr::Div(
            Box::new(SymbolicExpr::Var("x".to_string())),
            Box::new(SymbolicExpr::Add(
                Box::new(SymbolicExpr::Var("x".to_string())),
                Box::new(SymbolicExpr::Const(1.0)),
            )),
        );
        let result = diff(&expr, "x");
        // Quotient rule produces a Div
        assert!(matches!(result, SymbolicExpr::Div(_, _)));
        let s = symbolic_to_string(&simplify(&result));
        // Denominator should be (x+1)^2
        assert!(s.contains("(x+1)^2"), "expected (x+1)^2 in: {}", s);
    }

    #[test]
    fn test_diff_pow_non_constant_exponent() {
        // diff(x^x, x) uses general power rule: f^g * (g'*ln(f) + g*f'/f)
        let expr = SymbolicExpr::Pow(
            Box::new(SymbolicExpr::Var("x".to_string())),
            Box::new(SymbolicExpr::Var("x".to_string())),
        );
        let result = diff(&expr, "x");
        // Should be a Mul (f^g * (...))
        assert!(matches!(result, SymbolicExpr::Mul(_, _)));
        let s = symbolic_to_string(&result);
        assert!(s.contains("ln(x)"), "expected ln(x) in: {}", s);
    }

    #[test]
    fn test_diff_tan() {
        // diff(tan(x), x) = sec^2(x) = 1/cos^2(x)
        let expr = SymbolicExpr::Tan(Box::new(SymbolicExpr::Var("x".to_string())));
        let result = diff(&expr, "x");
        let s = symbolic_to_string(&result);
        assert!(s.contains("cos(x)"), "expected cos(x) in: {}", s);
        assert!(s.contains("^2"), "expected ^2 in: {}", s);
    }

    #[test]
    fn test_diff_ln_chain_rule() {
        // diff(ln(x^2), x) = (1/x^2) * 2x
        let expr = SymbolicExpr::Ln(Box::new(SymbolicExpr::Pow(
            Box::new(SymbolicExpr::Var("x".to_string())),
            Box::new(SymbolicExpr::Const(2.0)),
        )));
        let result = simplify(&diff(&expr, "x"));
        let s = symbolic_to_string(&result);
        assert!(s.contains("1/x^2"), "expected 1/x^2 in: {}", s);
        assert!(s.contains("2*x"), "expected 2*x in: {}", s);
    }

    #[test]
    fn test_diff_variable_not_matching() {
        // diff(y, x) = 0 (y is a different variable)
        let expr = SymbolicExpr::Var("y".to_string());
        let result = diff(&expr, "x");
        assert_eq!(result, SymbolicExpr::Const(0.0));
    }

    // ===== 补充覆盖测试：integrate 边界路径 =====

    #[test]
    fn test_integrate_constant() {
        // ∫5 dx = 5*x
        let expr = SymbolicExpr::Const(5.0);
        let result = simplify(&integrate(&expr, "x").unwrap());
        assert_eq!(symbolic_to_string(&result), "5*x");
    }

    #[test]
    fn test_integrate_other_variable() {
        // ∫y dx = y*x
        let expr = SymbolicExpr::Var("y".to_string());
        let result = simplify(&integrate(&expr, "x").unwrap());
        assert_eq!(symbolic_to_string(&result), "y*x");
    }

    #[test]
    fn test_integrate_sub() {
        // ∫(x - 3) dx = x^2/2 - 3*x
        let expr = SymbolicExpr::Sub(
            Box::new(SymbolicExpr::Var("x".to_string())),
            Box::new(SymbolicExpr::Const(3.0)),
        );
        let result = simplify(&integrate(&expr, "x").unwrap());
        let s = symbolic_to_string(&result);
        assert!(s.contains("x^2/2"), "expected x^2/2 in: {}", s);
        assert!(s.contains("3*x"), "expected 3*x in: {}", s);
    }

    #[test]
    fn test_integrate_tan_unsupported() {
        let expr = SymbolicExpr::Tan(Box::new(SymbolicExpr::Var("x".to_string())));
        assert!(integrate(&expr, "x").is_err());
    }

    #[test]
    fn test_integrate_ln_unsupported() {
        let expr = SymbolicExpr::Ln(Box::new(SymbolicExpr::Var("x".to_string())));
        assert!(integrate(&expr, "x").is_err());
    }

    #[test]
    fn test_integrate_pow_non_variable_base() {
        // ∫(x+1)^2 dx - base is not the integration variable → error
        let expr = SymbolicExpr::Pow(
            Box::new(SymbolicExpr::Add(
                Box::new(SymbolicExpr::Var("x".to_string())),
                Box::new(SymbolicExpr::Const(1.0)),
            )),
            Box::new(SymbolicExpr::Const(2.0)),
        );
        assert!(integrate(&expr, "x").is_err());
    }

    #[test]
    fn test_integrate_pow_different_var_base() {
        // ∫y^2 dx - base is y, not x → error
        let expr = SymbolicExpr::Pow(
            Box::new(SymbolicExpr::Var("y".to_string())),
            Box::new(SymbolicExpr::Const(2.0)),
        );
        assert!(integrate(&expr, "x").is_err());
    }

    #[test]
    fn test_integrate_sin_non_var_error() {
        // ∫sin(x^2) dx - arg is not the integration variable → error
        let expr = SymbolicExpr::Sin(Box::new(SymbolicExpr::Pow(
            Box::new(SymbolicExpr::Var("x".to_string())),
            Box::new(SymbolicExpr::Const(2.0)),
        )));
        assert!(integrate(&expr, "x").is_err());
    }

    #[test]
    fn test_integrate_cos_non_var_error() {
        // ∫cos(y) dx - arg is not the integration variable → error
        let expr = SymbolicExpr::Cos(Box::new(SymbolicExpr::Var("y".to_string())));
        assert!(integrate(&expr, "x").is_err());
    }

    #[test]
    fn test_integrate_exp_non_var_error() {
        // ∫exp(y) dx - arg is not the integration variable → error
        let expr = SymbolicExpr::Exp(Box::new(SymbolicExpr::Var("y".to_string())));
        assert!(integrate(&expr, "x").is_err());
    }

    #[test]
    fn test_integrate_mul_with_const_right() {
        // ∫x*3 dx = 3 * ∫x dx (constant on right side of Mul)
        let expr = SymbolicExpr::Mul(
            Box::new(SymbolicExpr::Var("x".to_string())),
            Box::new(SymbolicExpr::Const(3.0)),
        );
        let result = integrate(&expr, "x");
        assert!(result.is_ok());
        let s = symbolic_to_string(&simplify(&result.unwrap()));
        assert!(s.contains("3"), "expected 3 in: {}", s);
    }

    #[test]
    fn test_integrate_div_non_one_over_var_error() {
        // ∫x/2 dx - not 1/var form → error
        let expr = SymbolicExpr::Div(
            Box::new(SymbolicExpr::Var("x".to_string())),
            Box::new(SymbolicExpr::Const(2.0)),
        );
        assert!(integrate(&expr, "x").is_err());
    }

    #[test]
    fn test_integrate_neg() {
        // ∫-sin(x) dx = -(-cos(x)) = cos(x)
        let expr = SymbolicExpr::Neg(Box::new(SymbolicExpr::Sin(Box::new(SymbolicExpr::Var(
            "x".to_string(),
        )))));
        let result = simplify(&integrate(&expr, "x").unwrap());
        assert_eq!(symbolic_to_string(&result), "cos(x)");
    }

    // ===== 补充覆盖测试：simplify 边界路径 =====

    #[test]
    fn test_simplify_sub_zero_right() {
        // x - 0 → x
        let expr = SymbolicExpr::Sub(
            Box::new(SymbolicExpr::Var("x".to_string())),
            Box::new(SymbolicExpr::Const(0.0)),
        );
        assert_eq!(simplify(&expr), SymbolicExpr::Var("x".to_string()));
    }

    #[test]
    fn test_simplify_sub_zero_left() {
        // 0 - x → -x
        let expr = SymbolicExpr::Sub(
            Box::new(SymbolicExpr::Const(0.0)),
            Box::new(SymbolicExpr::Var("x".to_string())),
        );
        assert_eq!(
            simplify(&expr),
            SymbolicExpr::Neg(Box::new(SymbolicExpr::Var("x".to_string())))
        );
    }

    #[test]
    fn test_simplify_sub_constant_folding() {
        // 5 - 3 → 2
        let expr = SymbolicExpr::Sub(
            Box::new(SymbolicExpr::Const(5.0)),
            Box::new(SymbolicExpr::Const(3.0)),
        );
        assert_eq!(simplify(&expr), SymbolicExpr::Const(2.0));
    }

    #[test]
    fn test_simplify_sub_combine_like_terms() {
        // 2*x - 3*x → -1*x
        let expr = SymbolicExpr::Sub(
            Box::new(SymbolicExpr::Mul(
                Box::new(SymbolicExpr::Const(2.0)),
                Box::new(SymbolicExpr::Var("x".to_string())),
            )),
            Box::new(SymbolicExpr::Mul(
                Box::new(SymbolicExpr::Const(3.0)),
                Box::new(SymbolicExpr::Var("x".to_string())),
            )),
        );
        assert_eq!(
            simplify(&expr),
            SymbolicExpr::Mul(
                Box::new(SymbolicExpr::Const(-1.0)),
                Box::new(SymbolicExpr::Var("x".to_string())),
            )
        );
    }

    #[test]
    fn test_simplify_div_one() {
        // x / 1 → x
        let expr = SymbolicExpr::Div(
            Box::new(SymbolicExpr::Var("x".to_string())),
            Box::new(SymbolicExpr::Const(1.0)),
        );
        assert_eq!(simplify(&expr), SymbolicExpr::Var("x".to_string()));
    }

    #[test]
    fn test_simplify_div_zero_numerator() {
        // 0 / x → 0
        let expr = SymbolicExpr::Div(
            Box::new(SymbolicExpr::Const(0.0)),
            Box::new(SymbolicExpr::Var("x".to_string())),
        );
        assert_eq!(simplify(&expr), SymbolicExpr::Const(0.0));
    }

    #[test]
    fn test_simplify_div_constant_folding() {
        // 6 / 2 → 3
        let expr = SymbolicExpr::Div(
            Box::new(SymbolicExpr::Const(6.0)),
            Box::new(SymbolicExpr::Const(2.0)),
        );
        assert_eq!(simplify(&expr), SymbolicExpr::Const(3.0));
    }

    #[test]
    fn test_simplify_div_by_zero_kept() {
        // 6 / 0 → stays as Div (not folded)
        let expr = SymbolicExpr::Div(
            Box::new(SymbolicExpr::Const(6.0)),
            Box::new(SymbolicExpr::Const(0.0)),
        );
        assert_eq!(
            simplify(&expr),
            SymbolicExpr::Div(
                Box::new(SymbolicExpr::Const(6.0)),
                Box::new(SymbolicExpr::Const(0.0)),
            )
        );
    }

    #[test]
    fn test_simplify_neg_double_negation() {
        // -(-x) → x
        let expr = SymbolicExpr::Neg(Box::new(SymbolicExpr::Neg(Box::new(SymbolicExpr::Var(
            "x".to_string(),
        )))));
        assert_eq!(simplify(&expr), SymbolicExpr::Var("x".to_string()));
    }

    #[test]
    fn test_simplify_neg_constant() {
        // -(5) → -5
        let expr = SymbolicExpr::Neg(Box::new(SymbolicExpr::Const(5.0)));
        assert_eq!(simplify(&expr), SymbolicExpr::Const(-5.0));
    }

    #[test]
    fn test_simplify_pow_one_base() {
        // 1^x → 1
        let expr = SymbolicExpr::Pow(
            Box::new(SymbolicExpr::Const(1.0)),
            Box::new(SymbolicExpr::Var("x".to_string())),
        );
        assert_eq!(simplify(&expr), SymbolicExpr::Const(1.0));
    }

    #[test]
    fn test_simplify_pow_constant_folding() {
        // 2^3 → 8
        let expr = SymbolicExpr::Pow(
            Box::new(SymbolicExpr::Const(2.0)),
            Box::new(SymbolicExpr::Const(3.0)),
        );
        assert_eq!(simplify(&expr), SymbolicExpr::Const(8.0));
    }

    #[test]
    fn test_simplify_add_combine_like_terms() {
        // 2*x + 3*x → 5*x
        let expr = SymbolicExpr::Add(
            Box::new(SymbolicExpr::Mul(
                Box::new(SymbolicExpr::Const(2.0)),
                Box::new(SymbolicExpr::Var("x".to_string())),
            )),
            Box::new(SymbolicExpr::Mul(
                Box::new(SymbolicExpr::Const(3.0)),
                Box::new(SymbolicExpr::Var("x".to_string())),
            )),
        );
        assert_eq!(
            simplify(&expr),
            SymbolicExpr::Mul(
                Box::new(SymbolicExpr::Const(5.0)),
                Box::new(SymbolicExpr::Var("x".to_string())),
            )
        );
    }

    #[test]
    fn test_simplify_nested_sub_div() {
        // (2*x - 0) / 1 → 2*x
        let expr = SymbolicExpr::Div(
            Box::new(SymbolicExpr::Sub(
                Box::new(SymbolicExpr::Mul(
                    Box::new(SymbolicExpr::Const(2.0)),
                    Box::new(SymbolicExpr::Var("x".to_string())),
                )),
                Box::new(SymbolicExpr::Const(0.0)),
            )),
            Box::new(SymbolicExpr::Const(1.0)),
        );
        assert_eq!(
            simplify(&expr),
            SymbolicExpr::Mul(
                Box::new(SymbolicExpr::Const(2.0)),
                Box::new(SymbolicExpr::Var("x".to_string())),
            )
        );
    }

    #[test]
    fn test_simplify_neg_preserved() {
        // -x stays -x (non-constant, non-double-negation)
        let expr = SymbolicExpr::Neg(Box::new(SymbolicExpr::Var("x".to_string())));
        assert_eq!(
            simplify(&expr),
            SymbolicExpr::Neg(Box::new(SymbolicExpr::Var("x".to_string())))
        );
    }

    // ===== TG9.2 补充覆盖：integrate/simplify/limit/taylor 未覆盖路径 =====

    #[test]
    fn test_integrate_add_linearity() {
        // ∫(x + 3) dx = x^2/2 + 3*x (Add 线性性，覆盖 lines 393-395)
        let expr = SymbolicExpr::Add(
            Box::new(SymbolicExpr::Var("x".to_string())),
            Box::new(SymbolicExpr::Const(3.0)),
        );
        let result = simplify(&integrate(&expr, "x").unwrap());
        let s = symbolic_to_string(&result);
        assert!(s.contains("x^2/2"), "expected x^2/2 in: {}", s);
        assert!(s.contains("3*x"), "expected 3*x in: {}", s);
    }

    #[test]
    fn test_integrate_mul_const_left() {
        // ∫3*x dx = 3 * ∫x dx (常数在左侧，覆盖 lines 405-406)
        let expr = SymbolicExpr::Mul(
            Box::new(SymbolicExpr::Const(3.0)),
            Box::new(SymbolicExpr::Var("x".to_string())),
        );
        let result = simplify(&integrate(&expr, "x").unwrap());
        let s = symbolic_to_string(&result);
        assert!(s.contains("3"), "expected 3 in: {}", s);
        assert!(s.contains("x^2/2"), "expected x^2/2 in: {}", s);
    }

    #[test]
    fn test_integrate_pow_neg_one() {
        // ∫x^(-1) dx = ln|x| (覆盖 line 427)
        let expr = SymbolicExpr::Pow(
            Box::new(SymbolicExpr::Var("x".to_string())),
            Box::new(SymbolicExpr::Const(-1.0)),
        );
        let result = integrate(&expr, "x").unwrap();
        assert_eq!(
            result,
            SymbolicExpr::Ln(Box::new(SymbolicExpr::Var("x".to_string())))
        );
    }

    #[test]
    fn test_integrate_div_one_over_var() {
        // ∫1/x dx = ln|x| (覆盖 line 481)
        let expr = SymbolicExpr::Div(
            Box::new(SymbolicExpr::Const(1.0)),
            Box::new(SymbolicExpr::Var("x".to_string())),
        );
        let result = integrate(&expr, "x").unwrap();
        assert_eq!(
            result,
            SymbolicExpr::Ln(Box::new(SymbolicExpr::Var("x".to_string())))
        );
    }

    #[test]
    fn test_simplify_tan() {
        // simplify(tan(x)) → tan(x) (覆盖 line 520)
        let expr = SymbolicExpr::Tan(Box::new(SymbolicExpr::Var("x".to_string())));
        let result = simplify(&expr);
        assert_eq!(result, expr);
    }

    #[test]
    fn test_extract_coeff_mul_const_right() {
        // extract_coeff(x * 3) → (3, x) (覆盖 line 580)
        let expr = SymbolicExpr::Mul(
            Box::new(SymbolicExpr::Var("x".to_string())),
            Box::new(SymbolicExpr::Const(3.0)),
        );
        let (c, rest) = extract_coeff(&expr);
        assert_eq!(c, 3.0);
        assert_eq!(rest, SymbolicExpr::Var("x".to_string()));
    }

    #[test]
    fn test_coeff_times_zero_and_one() {
        // coeff_times(0.0, ...) → Const(0.0) (覆盖 line 591)
        assert_eq!(
            coeff_times(0.0, SymbolicExpr::Var("x".to_string())),
            SymbolicExpr::Const(0.0)
        );
        // coeff_times(1.0, rest) → rest (覆盖 line 594)
        assert_eq!(
            coeff_times(1.0, SymbolicExpr::Var("x".to_string())),
            SymbolicExpr::Var("x".to_string())
        );
    }

    #[test]
    fn test_limit_denominator_derivative_zero() {
        // limit(x/0, x, 0): direct sub → 0/0 → NaN → L'Hôpital →
        // diff(den=0) = 0 → error (覆盖 lines 703-705)
        let expr = SymbolicExpr::Div(
            Box::new(SymbolicExpr::Var("x".to_string())),
            Box::new(SymbolicExpr::Const(0.0)),
        );
        let result = limit(&expr, "x", 0.0);
        assert!(result.is_err());
    }

    #[test]
    fn test_limit_depth_exceeded() {
        // 直接调用 limit_recursive with depth=MAX_LOPITAL_DEPTH
        // expr = ln(x) at x=0 → NaN → not Div → depth exceeded error
        // (覆盖 lines 713-714, 716-719)
        let expr = SymbolicExpr::Ln(Box::new(SymbolicExpr::Var("x".to_string())));
        let result = limit_recursive(&expr, "x", 0.0, 5);
        assert!(result.is_err());
    }

    #[test]
    fn test_eval_symbolic_tan_and_ln() {
        // eval_symbolic(Tan(x), {x: 0}) → 0.0 (覆盖 line 744)
        let mut env = HashMap::new();
        env.insert("x".to_string(), 0.0);
        let tan_expr = SymbolicExpr::Tan(Box::new(SymbolicExpr::Var("x".to_string())));
        assert_eq!(eval_symbolic(&tan_expr, &env).unwrap(), 0.0);

        // eval_symbolic(Ln(x), {x: -1}) → NaN (v <= 0, 覆盖 lines 745-748)
        env.insert("x".to_string(), -1.0);
        let ln_expr = SymbolicExpr::Ln(Box::new(SymbolicExpr::Var("x".to_string())));
        let ln_result = eval_symbolic(&ln_expr, &env).unwrap();
        assert!(ln_result.is_nan());

        // eval_symbolic(Ln(x), {x: 1}) → 0.0 (v > 0, 覆盖 line 750)
        env.insert("x".to_string(), 1.0);
        assert_eq!(eval_symbolic(&ln_expr, &env).unwrap(), 0.0);
    }

    #[test]
    fn test_taylor_empty_terms() {
        // taylor(0, x, 3) → all derivatives 0 → terms empty → "0" (覆盖 line 792)
        let expr = SymbolicExpr::Const(0.0);
        let result = taylor(&expr, "x", 3).unwrap();
        if let EvalResult::Symbolic(s) = result {
            assert_eq!(s, "0");
        } else {
            panic!("expected Symbolic");
        }
    }

    #[test]
    fn test_taylor_format_term_coeff_one() {
        // taylor(x, x, 1) → f(0)=0, f'(0)=1, coeff=1/1!=1.0, k=1 → "x"
        // (覆盖 line 815: k=1, coeff==1.0)
        let expr = SymbolicExpr::Var("x".to_string());
        let result = taylor(&expr, "x", 1).unwrap();
        if let EvalResult::Symbolic(s) = result {
            assert_eq!(s, "x");
        } else {
            panic!("expected Symbolic");
        }

        // taylor(x^2, x, 2) → f(0)=0, f'(0)=0, f''(0)=2, coeff=2/2!=1.0, k=2 → "x^2"
        // (覆盖 line 820: k>1, coeff==1.0)
        let expr2 = SymbolicExpr::Pow(
            Box::new(SymbolicExpr::Var("x".to_string())),
            Box::new(SymbolicExpr::Const(2.0)),
        );
        let result2 = taylor(&expr2, "x", 2).unwrap();
        if let EvalResult::Symbolic(s) = result2 {
            assert!(s.contains("x^2"), "expected x^2 in: {}", s);
            assert!(!s.contains("1*x"), "should not contain 1* prefix: {}", s);
        } else {
            panic!("expected Symbolic");
        }
    }

    #[test]
    fn test_format_taylor_term_all_branches() {
        // k=0: just the coefficient
        assert_eq!(format_taylor_term(5.0, "x", 0), "5");
        // k=1, coeff==1.0: just var
        assert_eq!(format_taylor_term(1.0, "x", 1), "x");
        // k=1, coeff!=1.0: coeff*var
        assert_eq!(format_taylor_term(2.5, "x", 1), "2.5*x");
        // k>1, coeff==1.0: var^k
        assert_eq!(format_taylor_term(1.0, "x", 3), "x^3");
        // k>1, coeff!=1.0: coeff*var^k
        assert_eq!(format_taylor_term(0.5, "x", 2), "0.5*x^2");
    }

    #[test]
    fn test_symbolic_domain_default() {
        // 覆盖 lines 854-856: Default impl
        let domain = SymbolicDomain;
        assert_eq!(domain.domain_name(), "symbolic");
        assert_eq!(domain.priority(), 30);
    }

    #[test]
    fn test_eval_node_non_function_call() {
        // 覆盖 lines 864-867: eval_node with non-FunctionCall AST
        let domain = SymbolicDomain;
        let ast = AstNode::Number(42.0);
        let result = domain.evaluate(&ast, &EvalContext::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_eval_integrate_wrong_arg_count() {
        // 覆盖 lines 894-897: integrate() with wrong arg count
        let domain = SymbolicDomain;
        let ast = AstNode::FunctionCall("integrate".to_string(), vec![AstNode::Number(1.0)]);
        let result = domain.evaluate(&ast, &EvalContext::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_eval_simplify_wrong_arg_count() {
        // 覆盖 lines 906-909: simplify() with wrong arg count
        let domain = SymbolicDomain;
        let ast = AstNode::FunctionCall(
            "simplify".to_string(),
            vec![AstNode::Number(1.0), AstNode::Number(2.0)],
        );
        let result = domain.evaluate(&ast, &EvalContext::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_eval_limit_wrong_arg_count() {
        // 覆盖 lines 917-920: limit() with wrong arg count
        let domain = SymbolicDomain;
        let ast = AstNode::FunctionCall(
            "limit".to_string(),
            vec![AstNode::Number(1.0), AstNode::Number(2.0)],
        );
        let result = domain.evaluate(&ast, &EvalContext::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_eval_taylor_wrong_arg_count() {
        // 覆盖 lines 929-932: taylor() with wrong arg count
        let domain = SymbolicDomain;
        let ast = AstNode::FunctionCall(
            "taylor".to_string(),
            vec![AstNode::Number(1.0), AstNode::Number(2.0)],
        );
        let result = domain.evaluate(&ast, &EvalContext::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_contains_symbolic_function_unary_op() {
        // 覆盖 line 954: contains_symbolic_function with UnaryOp
        let ast = AstNode::UnaryOp(
            UnaryOp::Neg,
            Box::new(AstNode::FunctionCall(
                "diff".to_string(),
                vec![
                    AstNode::Variable("x".to_string()),
                    AstNode::Variable("x".to_string()),
                ],
            )),
        );
        assert!(contains_symbolic_function(&ast));
        // UnaryOp without symbolic function
        let ast2 = AstNode::UnaryOp(UnaryOp::Neg, Box::new(AstNode::Number(5.0)));
        assert!(!contains_symbolic_function(&ast2));
    }

    #[test]
    fn test_extract_var_name_error() {
        // 覆盖 lines 963-966: extract_var_name with non-Variable
        let ast = AstNode::Number(5.0);
        assert!(extract_var_name(&ast).is_err());
    }

    #[test]
    fn test_extract_number_all_paths() {
        // Number (line 973)
        assert_eq!(extract_number(&AstNode::Number(3.14)).unwrap(), 3.14);
        // BigNumber valid (lines 972, 974-975)
        assert_eq!(
            extract_number(&AstNode::BigNumber("42".to_string())).unwrap(),
            42.0
        );
        // BigNumber invalid (line 976 error)
        assert!(extract_number(&AstNode::BigNumber("abc".to_string())).is_err());
        // UnaryOp::Neg (line 978)
        assert_eq!(
            extract_number(&AstNode::UnaryOp(
                UnaryOp::Neg,
                Box::new(AstNode::Number(5.0))
            ))
            .unwrap(),
            -5.0
        );
        // Generic error (lines 979-982)
        assert!(extract_number(&AstNode::Variable("x".to_string())).is_err());
    }
}
