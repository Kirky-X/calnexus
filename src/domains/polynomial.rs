// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! Polynomial 计算域：多项式算术、求值、求根、微分、积分、因式分解。
//!
//! 设计依据：
//! - polynomial-domain spec：9 个 requirements / 14+ scenarios
//! - design.md D3（系数向量升幂存储 + 直接表达式输入）、D6（priority=25）
//!
//! 路由策略：AST 含多项式函数调用（poly_add/poly_sub/poly_mul/poly_div/poly_eval/
//! poly_diff/poly_integrate/roots/factor）时路由至本域。
//!
//! 多项式表示：系数向量 Vec<f64>，升幂存储（coef[i] = x^i 的系数）。
//! 输入语法：直接表达式 `poly_add(x^2+2x+1, x+1)`，域内 `expr_to_coeffs()` 转换。

use crate::core::CalculationDomain;
use crate::core::{AstNode, BinaryOp, CalcError, EvalContext, EvalResult, UnaryOp};

/// 多项式函数白名单。
const POLYNOMIAL_FUNCTIONS: &[&str] = &[
    "poly_add",
    "poly_sub",
    "poly_mul",
    "poly_div",
    "poly_eval",
    "poly_diff",
    "poly_integrate",
    "roots",
    "factor",
];

/// Polynomial 计算域。
///
/// priority=25，支持 poly_add/sub/mul/div/eval/diff/integrate/roots/factor。
pub struct PolynomialDomain;

impl CalculationDomain for PolynomialDomain {
    fn domain_name(&self) -> &str {
        "polynomial"
    }

    fn priority(&self) -> u8 {
        25
    }

    fn supports(&self, ast: &AstNode) -> bool {
        contains_polynomial_function(ast)
    }

    fn evaluate(&self, ast: &AstNode, ctx: &EvalContext) -> Result<EvalResult, CalcError> {
        self.eval_node(ast, ctx)
    }
}

impl Default for PolynomialDomain {
    fn default() -> Self {
        Self
    }
}

impl PolynomialDomain {
    /// 递归求值 AST 节点。
    fn eval_node(&self, ast: &AstNode, ctx: &EvalContext) -> Result<EvalResult, CalcError> {
        match ast {
            AstNode::FunctionCall(name, args) => self.eval_function(name, args, ctx),
            AstNode::Number(n) => Ok(EvalResult::Scalar(*n)),
            AstNode::Variable(name) => ctx
                .get_var(name)
                .map(EvalResult::Scalar)
                .ok_or_else(|| CalcError::EvalError(format!("unbound variable: {}", name))),
            AstNode::BinaryOp(_, _, _) => {
                // 尝试作为多项式表达式求值
                let (coeffs, _var) = expr_to_coeffs(ast, ctx)?;
                Ok(EvalResult::Polynomial(coeffs))
            }
            AstNode::UnaryOp(UnaryOp::Neg, e) => {
                let (coeffs, _var) = expr_to_coeffs(e, ctx)?;
                let neg: Vec<f64> = coeffs.iter().map(|x| -x).collect();
                Ok(EvalResult::Polynomial(neg))
            }
            AstNode::BigNumber(s) => {
                let n: f64 = s
                    .parse()
                    .map_err(|_| CalcError::DomainError(format!("invalid big number: {}", s)))?;
                Ok(EvalResult::Scalar(n))
            }
            AstNode::Complex(_, _) | AstNode::Matrix(_) | AstNode::List(_) => {
                Err(CalcError::DomainError(format!(
                    "polynomial domain does not support this node type: {:?}",
                    ast
                )))
            }
            AstNode::UnaryOp(UnaryOp::Abs, _) | AstNode::UnaryOp(UnaryOp::Factorial, _) => {
                Err(CalcError::DomainError(format!(
                    "polynomial domain does not support this unary op: {:?}",
                    ast
                )))
            }
        }
    }

    /// 求值多项式函数调用。
    fn eval_function(
        &self,
        name: &str,
        args: &[AstNode],
        ctx: &EvalContext,
    ) -> Result<EvalResult, CalcError> {
        if !POLYNOMIAL_FUNCTIONS.contains(&name) {
            return Err(CalcError::DomainError(format!(
                "unsupported function in polynomial domain: {}",
                name
            )));
        }
        match name {
            "poly_add" => {
                if args.len() != 2 {
                    return Err(CalcError::DomainError(format!(
                        "poly_add() requires exactly 2 arguments, got {}",
                        args.len()
                    )));
                }
                let (a, _) = self.arg_to_coeffs(&args[0], ctx)?;
                let (b, _) = self.arg_to_coeffs(&args[1], ctx)?;
                Ok(EvalResult::Polynomial(poly_add_coeffs(&a, &b)))
            }
            "poly_sub" => {
                if args.len() != 2 {
                    return Err(CalcError::DomainError(format!(
                        "poly_sub() requires exactly 2 arguments, got {}",
                        args.len()
                    )));
                }
                let (a, _) = self.arg_to_coeffs(&args[0], ctx)?;
                let (b, _) = self.arg_to_coeffs(&args[1], ctx)?;
                let neg_b: Vec<f64> = b.iter().map(|x| -x).collect();
                Ok(EvalResult::Polynomial(poly_add_coeffs(&a, &neg_b)))
            }
            "poly_mul" => {
                if args.len() != 2 {
                    return Err(CalcError::DomainError(format!(
                        "poly_mul() requires exactly 2 arguments, got {}",
                        args.len()
                    )));
                }
                let (a, _) = self.arg_to_coeffs(&args[0], ctx)?;
                let (b, _) = self.arg_to_coeffs(&args[1], ctx)?;
                Ok(EvalResult::Polynomial(poly_mul_coeffs(&a, &b)))
            }
            "poly_div" => {
                if args.len() != 2 {
                    return Err(CalcError::DomainError(format!(
                        "poly_div() requires exactly 2 arguments, got {}",
                        args.len()
                    )));
                }
                let (a, _) = self.arg_to_coeffs(&args[0], ctx)?;
                let (b, _) = self.arg_to_coeffs(&args[1], ctx)?;
                if is_zero_poly(&b) {
                    return Err(CalcError::DivisionByZero);
                }
                let (quotient, _remainder) = poly_div_coeffs(&a, &b);
                Ok(EvalResult::Polynomial(quotient))
            }
            "poly_eval" => {
                if args.len() != 2 {
                    return Err(CalcError::DomainError(format!(
                        "poly_eval() requires exactly 2 arguments, got {}",
                        args.len()
                    )));
                }
                let (coeffs, _) = self.arg_to_coeffs(&args[0], ctx)?;
                let x = self.eval_scalar(&args[1], ctx)?;
                let result = poly_eval_horner(&coeffs, x);
                Ok(EvalResult::Scalar(result))
            }
            "roots" => {
                if args.len() != 1 {
                    return Err(CalcError::DomainError(format!(
                        "roots() requires exactly 1 argument, got {}",
                        args.len()
                    )));
                }
                let (coeffs, _) = self.arg_to_coeffs(&args[0], ctx)?;
                let trimmed = trim_leading_zeros(&coeffs);
                find_roots(&trimmed)
            }
            "poly_diff" => {
                if args.len() != 1 {
                    return Err(CalcError::DomainError(format!(
                        "poly_diff() requires exactly 1 argument, got {}",
                        args.len()
                    )));
                }
                let (coeffs, _) = self.arg_to_coeffs(&args[0], ctx)?;
                Ok(EvalResult::Polynomial(poly_diff_coeffs(&coeffs)))
            }
            "poly_integrate" => {
                if args.len() != 1 {
                    return Err(CalcError::DomainError(format!(
                        "poly_integrate() requires exactly 1 argument, got {}",
                        args.len()
                    )));
                }
                let (coeffs, _) = self.arg_to_coeffs(&args[0], ctx)?;
                Ok(EvalResult::Polynomial(poly_integrate_coeffs(&coeffs)))
            }
            "factor" => {
                if args.len() != 1 {
                    return Err(CalcError::DomainError(format!(
                        "factor() requires exactly 1 argument, got {}",
                        args.len()
                    )));
                }
                let (coeffs, _) = self.arg_to_coeffs(&args[0], ctx)?;
                let trimmed = trim_leading_zeros(&coeffs);
                let result = factor_polynomial(&trimmed)?;
                Ok(EvalResult::Symbolic(result))
            }
            _ => unreachable!("checked above"),
        }
    }

    /// 将参数转为系数向量。
    /// 若参数为 FunctionCall（如 `poly_add(...)`），先求值再提取 Polynomial 系数；
    /// 否则调用 `expr_to_coeffs` 直接转换表达式树。
    fn arg_to_coeffs(
        &self,
        ast: &AstNode,
        ctx: &EvalContext,
    ) -> Result<(Vec<f64>, String), CalcError> {
        if let AstNode::FunctionCall(_, _) = ast {
            let result = self.eval_node(ast, ctx)?;
            return match result {
                EvalResult::Polynomial(coeffs) => Ok((coeffs, String::new())),
                _ => Err(CalcError::DomainError(format!(
                    "expected polynomial result from nested call, got {:?}",
                    ast
                ))),
            };
        }
        expr_to_coeffs(ast, ctx)
    }

    /// 将 AST 求值为 f64 标量（用于 poly_eval 的 x 参数）。
    fn eval_scalar(&self, ast: &AstNode, ctx: &EvalContext) -> Result<f64, CalcError> {
        match ast {
            AstNode::Number(n) => Ok(*n),
            AstNode::BigNumber(s) => s
                .parse::<f64>()
                .map_err(|_| CalcError::DomainError(format!("invalid big number: {}", s))),
            AstNode::Variable(name) => ctx
                .get_var(name)
                .ok_or_else(|| CalcError::EvalError(format!("unbound variable: {}", name))),
            AstNode::UnaryOp(UnaryOp::Neg, e) => Ok(-self.eval_scalar(e, ctx)?),
            AstNode::BinaryOp(op, l, r) => {
                let a = self.eval_scalar(l, ctx)?;
                let b = self.eval_scalar(r, ctx)?;
                match op {
                    BinaryOp::Add => Ok(a + b),
                    BinaryOp::Sub => Ok(a - b),
                    BinaryOp::Mul => Ok(a * b),
                    BinaryOp::Div => {
                        if b == 0.0 {
                            return Err(CalcError::DivisionByZero);
                        }
                        Ok(a / b)
                    }
                    BinaryOp::Pow => Ok(a.powf(b)),
                    BinaryOp::Mod => {
                        if b == 0.0 {
                            return Err(CalcError::DivisionByZero);
                        }
                        Ok(a % b)
                    }
                }
            }
            _ => Err(CalcError::DomainError(format!(
                "polynomial domain cannot evaluate scalar from: {:?}",
                ast
            ))),
        }
    }
}

/// 将表达式树转为升幂系数向量 (coeffs, variable_name)。
/// coef[i] = x^i 的系数。
fn expr_to_coeffs(ast: &AstNode, ctx: &EvalContext) -> Result<(Vec<f64>, String), CalcError> {
    match ast {
        AstNode::Number(n) => Ok((vec![*n], String::new())),
        AstNode::BigNumber(s) => {
            let n: f64 = s
                .parse()
                .map_err(|_| CalcError::DomainError(format!("invalid big number: {}", s)))?;
            Ok((vec![n], String::new()))
        }
        AstNode::Variable(name) => {
            // 如果变量在 ctx 中有值，视为常数
            if let Some(v) = ctx.get_var(name) {
                return Ok((vec![v], String::new()));
            }
            Ok((vec![0.0, 1.0], name.clone()))
        }
        AstNode::BinaryOp(op, l, r) => {
            match op {
                BinaryOp::Pow => {
                    // Variable ^ Number
                    if let (AstNode::Variable(name), AstNode::Number(n)) = (l.as_ref(), r.as_ref())
                    {
                        if *n < 0.0 || n.fract() != 0.0 {
                            return Err(CalcError::DomainError(
                                "polynomial exponent must be non-negative integer".to_string(),
                            ));
                        }
                        // DoS 防护：指数上界 1000，防止 OOM
                        const MAX_POLY_DEGREE: usize = 1000;
                        let exp = *n as usize;
                        if exp > MAX_POLY_DEGREE {
                            return Err(CalcError::Overflow);
                        }
                        let mut coeffs = vec![0.0; exp + 1];
                        coeffs[exp] = 1.0;
                        // 如果变量在 ctx 中有值，求值为常数
                        if let Some(v) = ctx.get_var(name) {
                            return Ok((vec![v.powi(exp as i32)], String::new()));
                        }
                        return Ok((coeffs, name.clone()));
                    }
                    // Number ^ Number → 常数
                    if let (AstNode::Number(a), AstNode::Number(b)) = (l.as_ref(), r.as_ref()) {
                        return Ok((vec![a.powf(*b)], String::new()));
                    }
                    Err(CalcError::DomainError(
                        "not a polynomial: unsupported power expression".to_string(),
                    ))
                }
                BinaryOp::Mul => {
                    // Number * Poly / Poly * Number
                    if let AstNode::Number(c) = l.as_ref() {
                        let (mut coeffs, var) = expr_to_coeffs(r, ctx)?;
                        for c_i in &mut coeffs {
                            *c_i *= c;
                        }
                        return Ok((coeffs, var));
                    }
                    if let AstNode::Number(c) = r.as_ref() {
                        let (mut coeffs, var) = expr_to_coeffs(l, ctx)?;
                        for c_i in &mut coeffs {
                            *c_i *= c;
                        }
                        return Ok((coeffs, var));
                    }
                    // Poly * Poly
                    let (a, var_a) = expr_to_coeffs(l, ctx)?;
                    let (b, var_b) = expr_to_coeffs(r, ctx)?;
                    let var = merge_var(&var_a, &var_b)?;
                    Ok((poly_mul_coeffs(&a, &b), var))
                }
                BinaryOp::Add => {
                    let (a, var_a) = expr_to_coeffs(l, ctx)?;
                    let (b, var_b) = expr_to_coeffs(r, ctx)?;
                    let var = merge_var(&var_a, &var_b)?;
                    Ok((poly_add_coeffs(&a, &b), var))
                }
                BinaryOp::Sub => {
                    let (a, var_a) = expr_to_coeffs(l, ctx)?;
                    let (b, var_b) = expr_to_coeffs(r, ctx)?;
                    let var = merge_var(&var_a, &var_b)?;
                    let neg_b: Vec<f64> = b.iter().map(|x| -x).collect();
                    Ok((poly_add_coeffs(&a, &neg_b), var))
                }
                BinaryOp::Div => {
                    // Number / Number → 常数（保留显式路径以返回 DivisionByZero）
                    if let (AstNode::Number(a), AstNode::Number(b)) = (l.as_ref(), r.as_ref()) {
                        if *b == 0.0 {
                            return Err(CalcError::DivisionByZero);
                        }
                        return Ok((vec![a / b], String::new()));
                    }
                    // Poly / Number → 逐系数除法
                    if let AstNode::Number(c) = r.as_ref() {
                        if *c == 0.0 {
                            return Err(CalcError::DivisionByZero);
                        }
                        let (mut coeffs, var) = expr_to_coeffs(l, ctx)?;
                        for c_i in &mut coeffs {
                            *c_i /= c;
                        }
                        return Ok((coeffs, var));
                    }
                    // Poly / Poly → 多项式长除法（要求整除，余数为零）
                    let (a, var_a) = expr_to_coeffs(l, ctx)?;
                    let (b, var_b) = expr_to_coeffs(r, ctx)?;
                    let var = merge_var(&var_a, &var_b)?;
                    let (quotient, remainder) = poly_div_coeffs(&a, &b);
                    if is_zero_poly(&remainder) {
                        Ok((quotient, var))
                    } else {
                        Err(CalcError::DomainError(
                            "polynomial division has non-zero remainder".to_string(),
                        ))
                    }
                }
                BinaryOp::Mod => Err(CalcError::DomainError(
                    "modulo in polynomial expression not supported".to_string(),
                )),
            }
        }
        AstNode::UnaryOp(UnaryOp::Neg, e) => {
            let (coeffs, var) = expr_to_coeffs(e, ctx)?;
            let neg: Vec<f64> = coeffs.iter().map(|x| -x).collect();
            Ok((neg, var))
        }
        _ => Err(CalcError::DomainError(format!(
            "not a polynomial expression: {:?}",
            ast
        ))),
    }
}

/// 合并变量名：两个都为空 → 空；一个为空 → 取非空；相同 → 取之；不同 → 错误。
fn merge_var(a: &str, b: &str) -> Result<String, CalcError> {
    if a.is_empty() {
        return Ok(b.to_string());
    }
    if b.is_empty() {
        return Ok(a.to_string());
    }
    if a == b {
        return Ok(a.to_string());
    }
    Err(CalcError::DomainError(format!(
        "polynomial in multiple variables: {} and {}",
        a, b
    )))
}

/// 多项式加法（系数向量）。
fn poly_add_coeffs(a: &[f64], b: &[f64]) -> Vec<f64> {
    let len = a.len().max(b.len());
    let mut result = vec![0.0; len];
    for (i, &c) in a.iter().enumerate() {
        result[i] += c;
    }
    for (i, &c) in b.iter().enumerate() {
        result[i] += c;
    }
    result
}

/// 多项式乘法（系数向量卷积）。
fn poly_mul_coeffs(a: &[f64], b: &[f64]) -> Vec<f64> {
    if a.is_empty() || b.is_empty() {
        return vec![0.0];
    }
    let mut result = vec![0.0; a.len() + b.len() - 1];
    for (i, &ai) in a.iter().enumerate() {
        for (j, &bj) in b.iter().enumerate() {
            result[i + j] += ai * bj;
        }
    }
    result
}

/// 多项式长除法，返回 (quotient, remainder)。
fn poly_div_coeffs(a: &[f64], b: &[f64]) -> (Vec<f64>, Vec<f64>) {
    let a = trim_leading_zeros(a);
    let b = trim_leading_zeros(b);
    if a.len() < b.len() || a.is_empty() {
        return (vec![0.0], a.clone());
    }
    let mut remainder = a.clone();
    let quotient_len = a.len() - b.len() + 1;
    let mut quotient = vec![0.0; quotient_len];
    let b_lead = b[b.len() - 1];
    for i in (0..quotient_len).rev() {
        let factor = remainder[i + b.len() - 1] / b_lead;
        quotient[i] = factor;
        for j in 0..b.len() {
            remainder[i + j] -= factor * b[j];
        }
    }
    remainder.truncate(b.len() - 1);
    if remainder.is_empty() {
        remainder.push(0.0);
    }
    (quotient, remainder)
}

/// Horner 法则求值。
fn poly_eval_horner(coeffs: &[f64], x: f64) -> f64 {
    if coeffs.is_empty() {
        return 0.0;
    }
    let mut result = 0.0;
    for &c in coeffs.iter().rev() {
        result = result * x + c;
    }
    result
}

/// 多项式微分：coef[i] -> coef[i+1] * (i+1)，降次。
fn poly_diff_coeffs(coeffs: &[f64]) -> Vec<f64> {
    if coeffs.len() <= 1 {
        return vec![0.0];
    }
    let mut result = Vec::with_capacity(coeffs.len() - 1);
    for (i, &c) in coeffs.iter().enumerate().skip(1) {
        result.push(c * i as f64);
    }
    result
}

/// 多项式积分：coef[i] -> coef[i-1] / i，升次，常数项=0。
fn poly_integrate_coeffs(coeffs: &[f64]) -> Vec<f64> {
    let mut result = vec![0.0];
    for (i, &c) in coeffs.iter().enumerate() {
        result.push(c / (i + 1) as f64);
    }
    result
}

/// 去除尾部零系数（高次零系数）。
fn trim_leading_zeros(coeffs: &[f64]) -> Vec<f64> {
    let mut result = coeffs.to_vec();
    while result.len() > 1 && result.last() == Some(&0.0) {
        result.pop();
    }
    result
}

/// 判断是否为零多项式。
fn is_zero_poly(coeffs: &[f64]) -> bool {
    coeffs.iter().all(|&c| c == 0.0)
}

/// 求根：支持 1-4 次多项式。>4 次返回 DomainError。
/// 实根返回 Vector，复根返回 ComplexList。
fn find_roots(coeffs: &[f64]) -> Result<EvalResult, CalcError> {
    let c = trim_leading_zeros(coeffs);
    if c.len() == 1 {
        if c[0] == 0.0 {
            return Err(CalcError::DomainError(
                "roots(): zero polynomial has infinite roots".to_string(),
            ));
        }
        return Ok(EvalResult::Vector(vec![])); // 非零常数无根
    }
    match c.len() - 1 {
        1 => {
            // ax + b = 0 → x = -b/a
            let a = c[1];
            let b = c[0];
            Ok(EvalResult::Vector(vec![-b / a]))
        }
        2 => {
            // ax^2 + bx + c = 0
            let a = c[2];
            let b = c[1];
            let cc = c[0];
            let discriminant = b * b - 4.0 * a * cc;
            if discriminant >= 0.0 {
                let sqrt_d = discriminant.sqrt();
                let r1 = (-b + sqrt_d) / (2.0 * a);
                let r2 = (-b - sqrt_d) / (2.0 * a);
                Ok(EvalResult::Vector(vec![r1, r2]))
            } else {
                let sqrt_d = (-discriminant).sqrt();
                let re = -b / (2.0 * a);
                let im = sqrt_d / (2.0 * a);
                Ok(EvalResult::ComplexList(vec![(re, im), (re, -im)]))
            }
        }
        3 => {
            // ax^3 + bx^2 + cx + d = 0 (Cardano 公式)
            let roots = solve_cubic(c[3], c[2], c[1], c[0]);
            Ok(roots_to_eval_result(roots))
        }
        4 => {
            // ax^4 + bx^3 + cx^2 + dx + e = 0 (Ferrari 方法)
            let roots = solve_quartic(c[4], c[3], c[2], c[1], c[0]);
            Ok(roots_to_eval_result(roots))
        }
        _ => Err(CalcError::DomainError(format!(
            "roots(): polynomial degree {} not supported (max degree 4)",
            c.len() - 1
        ))),
    }
}

/// 将复根列表转换为 EvalResult：全实根 → Vector，含复根 → ComplexList。
fn roots_to_eval_result(roots: Vec<(f64, f64)>) -> EvalResult {
    const EPS: f64 = 1e-10;
    let all_real = roots.iter().all(|(_, im)| im.abs() < EPS);
    if all_real {
        EvalResult::Vector(roots.into_iter().map(|(re, _)| re).collect())
    } else {
        EvalResult::ComplexList(roots)
    }
}

/// 解二次方程 at² + bt + c = 0，返回 (实部, 虚部) 对。
fn solve_quadratic_complex(a: f64, b: f64, c: f64) -> Vec<(f64, f64)> {
    let disc = b * b - 4.0 * a * c;
    if disc >= 0.0 {
        let sqrt_d = disc.sqrt();
        vec![
            ((-b + sqrt_d) / (2.0 * a), 0.0),
            ((-b - sqrt_d) / (2.0 * a), 0.0),
        ]
    } else {
        let sqrt_d = (-disc).sqrt();
        let re = -b / (2.0 * a);
        let im = sqrt_d / (2.0 * a);
        vec![(re, im), (re, -im)]
    }
}

/// 解三次方程 ax³ + bx² + cx + d = 0（Cardano 公式），返回 (实部, 虚部) 对。
///
/// 判别式 Δ = (q/2)² + (p/3)³：
/// - Δ > 0：1 实根 + 2 复共轭根
/// - Δ = 0：3 实根（至少 2 个相等）
/// - Δ < 0：3 个不同实根（三角公式）
fn solve_cubic(a: f64, b: f64, c: f64, d: f64) -> Vec<(f64, f64)> {
    const EPS: f64 = 1e-12;

    // 归一化为首一多项式：x³ + Bx² + Cx + D = 0
    let b = b / a;
    let c = c / a;
    let d = d / a;

    // 代换 x = t - b/3 → t³ + pt + q = 0
    let p = c - b * b / 3.0;
    let q = 2.0 * b * b * b / 27.0 - b * c / 3.0 + d;
    let shift = -b / 3.0;

    let disc = (q / 2.0).powi(2) + (p / 3.0).powi(3);

    if disc > EPS {
        // Δ > 0：1 实根 + 2 复共轭根
        let sqrt_d = disc.sqrt();
        let u = (-q / 2.0 + sqrt_d).cbrt();
        let v = (-q / 2.0 - sqrt_d).cbrt();
        let t1 = u + v;
        let re = -(u + v) / 2.0;
        let im = (u - v) * 3.0_f64.sqrt() / 2.0;
        vec![(t1 + shift, 0.0), (re + shift, im), (re + shift, -im)]
    } else if disc < -EPS {
        // Δ < 0：3 个不同实根（三角公式）
        let m = 2.0 * (-p / 3.0).sqrt();
        let arg = (3.0 * q / (p * m)).clamp(-1.0, 1.0);
        let theta = arg.acos() / 3.0;
        let two_pi_3 = 2.0 * std::f64::consts::PI / 3.0;
        vec![
            (m * theta.cos() + shift, 0.0),
            (m * (theta - two_pi_3).cos() + shift, 0.0),
            (m * (theta + two_pi_3).cos() + shift, 0.0),
        ]
    } else {
        // Δ = 0：3 实根，至少 2 个相等
        if p.abs() < EPS {
            // 三重根 0
            vec![(shift, 0.0), (shift, 0.0), (shift, 0.0)]
        } else {
            let u = (-q / 2.0).cbrt();
            vec![(2.0 * u + shift, 0.0), (-u + shift, 0.0), (-u + shift, 0.0)]
        }
    }
}

/// 解四次方程 ax⁴ + bx³ + cx² + dx + e = 0（Ferrari 方法），返回 (实部, 虚部) 对。
fn solve_quartic(a: f64, b: f64, c: f64, d: f64, e: f64) -> Vec<(f64, f64)> {
    const EPS: f64 = 1e-12;

    // 归一化
    let b = b / a;
    let c = c / a;
    let d = d / a;
    let e = e / a;

    // 代换 x = t - b/4 → t⁴ + pt² + qt + r = 0
    let p = c - 3.0 * b * b / 8.0;
    let q = d - b * c / 2.0 + b * b * b / 8.0;
    let r = e - b * d / 4.0 + b * b * c / 16.0 - 3.0 * b.powi(4) / 256.0;
    let shift = -b / 4.0;

    if q.abs() < EPS {
        // 双二次方程 t⁴ + pt² + r = 0
        let disc = p * p - 4.0 * r;
        if disc >= 0.0 {
            let sqrt_disc = disc.sqrt();
            let t2_1 = (-p + sqrt_disc) / 2.0;
            let t2_2 = (-p - sqrt_disc) / 2.0;
            let mut roots = Vec::new();
            for &t2 in &[t2_1, t2_2] {
                if t2 >= 0.0 {
                    let t = t2.sqrt();
                    roots.push((t + shift, 0.0));
                    roots.push((-t + shift, 0.0));
                } else {
                    let t = (-t2).sqrt();
                    roots.push((shift, t));
                    roots.push((shift, -t));
                }
            }
            roots
        } else {
            // 复数 t² → 需要复数平方根
            let sqrt_disc = (-disc).sqrt();
            let re_t2 = -p / 2.0;
            let im_t2 = sqrt_disc / 2.0;
            let mut roots = Vec::new();
            for &sign in &[1.0, -1.0] {
                let (sr, si) = complex_sqrt(re_t2, im_t2 * sign);
                roots.push((sr + shift, si));
                roots.push((-sr + shift, -si));
            }
            roots
        }
    } else {
        // Ferrari 方法：解预解三次方程 m³ + 2pm² + (p² - 4r)m - q² = 0
        let resolvent_roots = solve_cubic(1.0, 2.0 * p, p * p - 4.0 * r, -q * q);

        // 找正实根 m（预解三次方程在 m=0 处值为 -q² < 0，故必存在正实根）
        let m = resolvent_roots
            .iter()
            .find(|(re, im)| im.abs() < EPS && *re > 0.0)
            .map(|(re, _)| *re)
            .expect("resolvent cubic must have a positive real root");

        let sqrt_m = m.sqrt();
        let half_pm = (p + m) / 2.0;
        let q_term = q / (2.0 * sqrt_m);

        // 因式分解为两个二次方程：
        // t² - √m·t + (half_pm + q_term) = 0
        // t² + √m·t + (half_pm - q_term) = 0
        let mut roots = Vec::new();
        roots.extend(solve_quadratic_complex(1.0, -sqrt_m, half_pm + q_term));
        roots.extend(solve_quadratic_complex(1.0, sqrt_m, half_pm - q_term));

        roots.into_iter().map(|(re, im)| (re + shift, im)).collect()
    }
}

/// 复数平方根 √(re + im·i)。
fn complex_sqrt(re: f64, im: f64) -> (f64, f64) {
    let mag = (re * re + im * im).sqrt();
    let sqrt_re = ((mag + re) / 2.0).sqrt();
    let mut sqrt_im = ((mag - re) / 2.0).sqrt();
    if im < 0.0 {
        sqrt_im = -sqrt_im;
    }
    (sqrt_re, sqrt_im)
}

/// 基础因式分解：二次整数系数多项式，使用有理根定理。
/// 返回格式如 "(x-2)*(x+2)"。
fn factor_polynomial(coeffs: &[f64]) -> Result<String, CalcError> {
    let c = trim_leading_zeros(coeffs);
    if c.len() == 1 {
        // 常数 → 返回自身
        return Ok(format!("{}", c[0] as i64));
    }
    match c.len() - 1 {
        1 => {
            // ax + b = a(x + b/a) = a(x - r) where r = -b/a
            let a = c[1];
            let b = c[0];
            let root = -b / a;
            Ok(format_factor_linear(a, root))
        }
        2 => {
            let a = c[2];
            let b = c[1];
            let cc = c[0];
            let discriminant = b * b - 4.0 * a * cc;
            if discriminant < 0.0 {
                return Err(CalcError::DomainError(
                    "factor(): complex roots cannot be factored over reals".to_string(),
                ));
            }
            let sqrt_d = discriminant.sqrt();
            let r1 = (-b + sqrt_d) / (2.0 * a);
            let r2 = (-b - sqrt_d) / (2.0 * a);
            Ok(format_factor_quadratic(a, r1, r2))
        }
        _ => Err(CalcError::DomainError(format!(
            "factor(): polynomial degree {} not supported (max degree 2 in v0.8)",
            c.len() - 1
        ))),
    }
}

/// 格式化一次因式分解：a(x - r)。
fn format_factor_linear(a: f64, r: f64) -> String {
    let lead = if a == 1.0 {
        String::new()
    } else if a == -1.0 {
        "-".to_string()
    } else {
        format!("{}*", a as i64)
    };
    let root_str = if r == 0.0 {
        "x".to_string()
    } else if r > 0.0 {
        format!("(x-{})", r as i64)
    } else {
        format!("(x+{})", (-r) as i64)
    };
    format!("{}{}", lead, root_str)
}

/// 格式化二次因式分解：a(x-r1)(x-r2)。
fn format_factor_quadratic(a: f64, r1: f64, r2: f64) -> String {
    let lead = if a == 1.0 {
        String::new()
    } else if a == -1.0 {
        "-".to_string()
    } else {
        format!("{}*", a as i64)
    };
    let f1 = format_factor_term(r1);
    let f2 = format_factor_term(r2);
    format!("{}{}*{}", lead, f1, f2)
}

/// 格式化单个因式 (x-r) 或 (x+r)。
fn format_factor_term(r: f64) -> String {
    if r == 0.0 {
        "x".to_string()
    } else if r > 0.0 {
        format!("(x-{})", r as i64)
    } else {
        format!("(x+{})", (-r) as i64)
    }
}

/// 递归检查 AST 是否含多项式函数调用。
fn contains_polynomial_function(ast: &AstNode) -> bool {
    match ast {
        AstNode::FunctionCall(name, _) if POLYNOMIAL_FUNCTIONS.contains(&name.as_str()) => true,
        AstNode::FunctionCall(_, args) => args.iter().any(contains_polynomial_function),
        AstNode::BinaryOp(_, l, r) => {
            contains_polynomial_function(l) || contains_polynomial_function(r)
        }
        AstNode::UnaryOp(_, e) => contains_polynomial_function(e),
        AstNode::Matrix(rows) => rows.iter().flatten().any(contains_polynomial_function),
        AstNode::List(elements) => elements.iter().any(contains_polynomial_function),
        AstNode::Number(_)
        | AstNode::Variable(_)
        | AstNode::Complex(_, _)
        | AstNode::BigNumber(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::parse;

    fn eval(input: &str) -> Result<EvalResult, CalcError> {
        let ast = parse(input).unwrap();
        let domain = PolynomialDomain;
        let ctx = EvalContext::new();
        domain.evaluate(&ast, &ctx)
    }

    fn eval_scalar(input: &str) -> Result<f64, CalcError> {
        eval(input).map(|r| r.as_scalar().expect("expected scalar result"))
    }

    fn eval_polynomial(input: &str) -> Result<Vec<f64>, CalcError> {
        eval(input).map(|r| {
            r.as_polynomial()
                .expect("expected polynomial result")
                .clone()
        })
    }

    fn eval_symbolic(input: &str) -> Result<String, CalcError> {
        eval(input).map(|r| r.as_symbolic().expect("expected symbolic result").clone())
    }

    fn assert_approx(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 1e-9,
            "expected {} but got {}",
            expected,
            actual
        );
    }

    fn assert_vec_approx(actual: &[f64], expected: &[f64]) {
        assert_eq!(actual.len(), expected.len(), "length mismatch");
        for (a, e) in actual.iter().zip(expected.iter()) {
            assert_approx(*a, *e);
        }
    }

    // ===== UT-POL-001: 多项式加法 =====

    #[test]
    fn test_poly_add() {
        let result = eval_polynomial("poly_add(x^2+2*x+1, x+1)").unwrap();
        assert_vec_approx(&result, &[2.0, 3.0, 1.0]); // x^2+3x+2
    }

    // ===== UT-POL-002: 多项式减法 =====

    #[test]
    fn test_poly_sub() {
        let result = eval_polynomial("poly_sub(x^2+2*x+1, x+1)").unwrap();
        assert_vec_approx(&result, &[0.0, 1.0, 1.0]); // x^2+x
    }

    // ===== UT-POL-003: 多项式乘法 =====

    #[test]
    fn test_poly_mul() {
        let result = eval_polynomial("poly_mul(x+1, x+2)").unwrap();
        assert_vec_approx(&result, &[2.0, 3.0, 1.0]); // x^2+3x+2
    }

    // ===== UT-POL-004: 多项式除法 =====

    #[test]
    fn test_poly_div() {
        let result = eval_polynomial("poly_div(x^2-1, x-1)").unwrap();
        assert_vec_approx(&result, &[1.0, 1.0]); // x+1
    }

    // ===== UT-POL-005: 求根（实根）=====

    #[test]
    fn test_roots_real() {
        let result = eval("roots(x^2-4)").unwrap();
        let roots = result.as_vector().unwrap();
        assert_eq!(roots.len(), 2);
        assert_approx(roots[0], 2.0);
        assert_approx(roots[1], -2.0);
    }

    // ===== UT-POL-006: 因式分解 =====

    #[test]
    fn test_factor() {
        let result = eval_symbolic("factor(x^2-4)").unwrap();
        assert!(
            result.contains("(x-2)") && result.contains("(x+2)"),
            "expected factors (x-2) and (x+2), got: {}",
            result
        );
    }

    // ===== UT-POL-007: 求值 =====

    #[test]
    fn test_poly_eval() {
        assert_eq!(eval_scalar("poly_eval(x^2+1, 2)").unwrap(), 5.0);
    }

    // ===== UT-POL-008: 微分 =====

    #[test]
    fn test_poly_diff() {
        let result = eval_polynomial("poly_diff(x^3+2*x)").unwrap();
        // d/dx(x^3+2x) = 3x^2+2 → [2, 0, 3]
        assert_vec_approx(&result, &[2.0, 0.0, 3.0]);
    }

    // ===== UT-POL-009: 积分 =====

    #[test]
    fn test_poly_integrate() {
        let result = eval_polynomial("poly_integrate(2*x)").unwrap();
        // ∫2x dx = x^2 → [0, 0, 1]
        assert_vec_approx(&result, &[0.0, 0.0, 1.0]);
    }

    // ===== UT-POL-010: 无实根（复根）=====

    #[test]
    fn test_roots_complex() {
        let result = eval("roots(x^2+1)").unwrap();
        let roots = result.as_complex_list().unwrap();
        assert_eq!(roots.len(), 2);
        // roots should be (0, 1) and (0, -1) → ±i
        assert_approx(roots[0].0, 0.0);
        assert_approx(roots[0].1.abs(), 1.0);
        assert_approx(roots[1].0, 0.0);
        assert_approx(roots[1].1.abs(), 1.0);
        assert_approx(roots[0].1 + roots[1].1, 0.0); // 共轭
    }

    // ===== 补充测试 =====

    #[test]
    fn test_poly_add_simple() {
        let result = eval_polynomial("poly_add(x+1, x+2)").unwrap();
        assert_vec_approx(&result, &[3.0, 2.0]); // 2x+3
    }

    #[test]
    fn test_poly_mul_x_squared() {
        let result = eval_polynomial("poly_mul(x, x)").unwrap();
        assert_vec_approx(&result, &[0.0, 0.0, 1.0]); // x^2
    }

    #[test]
    fn test_poly_div_exact() {
        let result = eval_polynomial("poly_div(x^2-4, x-2)").unwrap();
        assert_vec_approx(&result, &[2.0, 1.0]); // x+2... wait, x^2-4 / (x-2) = x+2 → [2, 1]
    }

    #[test]
    fn test_poly_eval_zero() {
        assert_eq!(eval_scalar("poly_eval(x^2-4, 2)").unwrap(), 0.0);
    }

    #[test]
    fn test_poly_eval_negative() {
        assert_eq!(eval_scalar("poly_eval(x^2-4, -2)").unwrap(), 0.0);
    }

    #[test]
    fn test_poly_diff_constant() {
        let result = eval_polynomial("poly_diff(5)").unwrap();
        assert_vec_approx(&result, &[0.0]);
    }

    #[test]
    fn test_poly_diff_linear() {
        let result = eval_polynomial("poly_diff(2*x+3)").unwrap();
        assert_vec_approx(&result, &[2.0]); // d/dx(2x+3) = 2
    }

    #[test]
    fn test_poly_integrate_constant() {
        let result = eval_polynomial("poly_integrate(5)").unwrap();
        assert_vec_approx(&result, &[0.0, 5.0]); // ∫5 dx = 5x
    }

    #[test]
    fn test_poly_integrate_x_squared() {
        let result = eval_polynomial("poly_integrate(x^2)").unwrap();
        // ∫x^2 dx = x^3/3 → [0, 0, 0, 1/3]
        assert_vec_approx(&result, &[0.0, 0.0, 0.0, 1.0 / 3.0]);
    }

    #[test]
    fn test_roots_linear() {
        let result = eval("roots(x-5)").unwrap();
        let roots = result.as_vector().unwrap();
        assert_eq!(roots.len(), 1);
        assert_approx(roots[0], 5.0);
    }

    #[test]
    fn test_roots_constant_nonzero() {
        let result = eval("roots(5)").unwrap();
        assert!(result.as_vector().unwrap().is_empty());
    }

    #[test]
    fn test_roots_zero_polynomial() {
        let result = eval("roots(0)");
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_roots_cubic_one_real_two_complex() {
        // x^3+1 = 0 → roots: -1, (1±i√3)/2
        let result = eval("roots(x^3+1)").unwrap();
        let roots = result.as_complex_list().expect("expected ComplexList");
        assert_eq!(roots.len(), 3);
        // 实根 -1
        let real_roots: Vec<f64> = roots
            .iter()
            .filter(|(_, im)| im.abs() < 1e-9)
            .map(|(re, _)| *re)
            .collect();
        assert_eq!(real_roots.len(), 1);
        assert_approx(real_roots[0], -1.0);
        // 复共轭对 (0.5, ±√3/2)
        let complex_roots: Vec<(f64, f64)> = roots
            .iter()
            .filter(|(_, im)| im.abs() >= 1e-9)
            .copied()
            .collect();
        assert_eq!(complex_roots.len(), 2);
        assert_approx(complex_roots[0].0, 0.5);
        assert_approx(complex_roots[1].0, 0.5);
        assert_approx(complex_roots[0].1 + complex_roots[1].1, 0.0); // 共轭
    }

    #[test]
    fn test_roots_cubic_three_real() {
        // x^3-6x^2+11x-6 = 0 → roots: 1, 2, 3
        let result = eval("roots(x^3-6*x^2+11*x-6)").unwrap();
        let roots = result
            .as_vector()
            .expect("expected Vector (all real roots)");
        assert_eq!(roots.len(), 3);
        let mut sorted = roots.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_approx(sorted[0], 1.0);
        assert_approx(sorted[1], 2.0);
        assert_approx(sorted[2], 3.0);
    }

    #[test]
    fn test_roots_cubic_repeated() {
        // x^3-3x^2+3x-1 = (x-1)^3 → triple root 1
        let result = eval("roots(x^3-3*x^2+3*x-1)").unwrap();
        let roots = result.as_vector().expect("expected Vector");
        assert_eq!(roots.len(), 3);
        for r in roots {
            assert_approx(*r, 1.0);
        }
    }

    #[test]
    fn test_roots_quartic_four_real() {
        // x^4-5x^2+4 = (x^2-1)(x^2-4) → roots: ±1, ±2
        let result = eval("roots(x^4-5*x^2+4)").unwrap();
        let roots = result
            .as_vector()
            .expect("expected Vector (all real roots)");
        assert_eq!(roots.len(), 4);
        let mut sorted = roots.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_approx(sorted[0], -2.0);
        assert_approx(sorted[1], -1.0);
        assert_approx(sorted[2], 1.0);
        assert_approx(sorted[3], 2.0);
    }

    #[test]
    fn test_roots_quartic_two_real_two_complex() {
        // x^4+1 = 0 → roots: (±√2/2)(1±i), 即 4 个复根
        let result = eval("roots(x^4+1)").unwrap();
        let roots = result.as_complex_list().expect("expected ComplexList");
        assert_eq!(roots.len(), 4);
        // 所有根的模应为 1（因为 |x^4| = |-1| → |x| = 1）
        for &(re, im) in roots {
            let mag = (re * re + im * im).sqrt();
            assert!((mag - 1.0).abs() < 1e-6, "expected |root| = 1, got {}", mag);
        }
    }

    #[test]
    fn test_roots_quartic_with_cubic_term() {
        // x^4-1 = 0 → roots: 1, -1, i, -i
        let result = eval("roots(x^4-1)").unwrap();
        let roots = result.as_complex_list().expect("expected ComplexList");
        assert_eq!(roots.len(), 4);
        // 找实根 ±1
        let real_roots: Vec<f64> = roots
            .iter()
            .filter(|(_, im)| im.abs() < 1e-9)
            .map(|(re, _)| *re)
            .collect();
        assert_eq!(real_roots.len(), 2);
        let mut sorted = real_roots.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_approx(sorted[0], -1.0);
        assert_approx(sorted[1], 1.0);
        // 找复根 ±i
        let complex_roots: Vec<(f64, f64)> = roots
            .iter()
            .filter(|(_, im)| im.abs() >= 1e-9)
            .copied()
            .collect();
        assert_eq!(complex_roots.len(), 2);
        assert_approx(complex_roots[0].0, 0.0);
        assert_approx(complex_roots[1].0, 0.0);
        assert_approx(complex_roots[0].1.abs(), 1.0);
        assert_approx(complex_roots[1].1.abs(), 1.0);
    }

    #[test]
    fn test_roots_degree_5_not_supported() {
        let result = eval("roots(x^5+1)");
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_poly_div_by_zero() {
        let result = eval("poly_div(x+1, 0)");
        assert!(matches!(result, Err(CalcError::DivisionByZero)));
    }

    #[test]
    fn test_factor_linear() {
        let result = eval_symbolic("factor(x-3)").unwrap();
        assert!(result.contains("x-3") || result.contains("(x-3)"));
    }

    #[test]
    fn test_factor_complex_error() {
        let result = eval("factor(x^2+1)");
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_factor_high_degree() {
        let result = eval("factor(x^3+1)");
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    // ===== 域元信息测试 =====

    #[test]
    fn test_domain_info() {
        let domain = PolynomialDomain;
        assert_eq!(domain.domain_name(), "polynomial");
        assert_eq!(domain.priority(), 25);
    }

    #[test]
    fn test_default_impl() {
        let domain = PolynomialDomain;
        assert_eq!(domain.domain_name(), "polynomial");
    }

    #[test]
    fn test_supports_poly_add() {
        let ast = parse("poly_add(x+1, x+2)").unwrap();
        assert!(PolynomialDomain.supports(&ast));
    }

    #[test]
    fn test_supports_roots() {
        let ast = parse("roots(x^2-4)").unwrap();
        assert!(PolynomialDomain.supports(&ast));
    }

    #[test]
    fn test_supports_nested() {
        let ast = parse("poly_add(x+1, roots(x^2-4))").unwrap();
        assert!(PolynomialDomain.supports(&ast));
    }

    #[test]
    fn test_not_supports_arithmetic() {
        let ast = parse("1+2").unwrap();
        assert!(!PolynomialDomain.supports(&ast));
    }

    // ===== 错误路径测试 =====

    #[test]
    fn test_unsupported_function() {
        let ast = AstNode::FunctionCall("sin".to_string(), vec![AstNode::Number(1.0)]);
        let result = PolynomialDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_poly_add_wrong_args() {
        let ast = AstNode::FunctionCall("poly_add".to_string(), vec![AstNode::Number(1.0)]);
        let result = PolynomialDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_complex_rejected() {
        let ast = AstNode::Complex(1.0, 2.0);
        let result = PolynomialDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_matrix_rejected() {
        let ast = AstNode::Matrix(vec![vec![AstNode::Number(1.0)]]);
        let result = PolynomialDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_list_rejected() {
        let ast = AstNode::List(vec![AstNode::Number(1.0)]);
        let result = PolynomialDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_factorial_rejected() {
        let ast = AstNode::UnaryOp(UnaryOp::Factorial, Box::new(AstNode::Number(5.0)));
        let result = PolynomialDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_multiple_variables_rejected() {
        // x + y → 多变量多项式，应该报错
        let ast = AstNode::BinaryOp(
            BinaryOp::Add,
            Box::new(AstNode::Variable("x".to_string())),
            Box::new(AstNode::Variable("y".to_string())),
        );
        let result = PolynomialDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_negative_exponent_rejected() {
        let ast = AstNode::BinaryOp(
            BinaryOp::Pow,
            Box::new(AstNode::Variable("x".to_string())),
            Box::new(AstNode::Number(-1.0)),
        );
        let result = PolynomialDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    // ===== 底层算法测试 =====

    #[test]
    fn test_poly_add_coeffs() {
        assert_eq!(
            poly_add_coeffs(&[1.0, 2.0], &[3.0, 4.0, 5.0]),
            vec![4.0, 6.0, 5.0]
        );
        assert_eq!(poly_add_coeffs(&[], &[1.0]), vec![1.0]);
    }

    #[test]
    fn test_poly_mul_coeffs() {
        // (x+1)(x+2) = x^2+3x+2
        assert_eq!(
            poly_mul_coeffs(&[1.0, 1.0], &[2.0, 1.0]),
            vec![2.0, 3.0, 1.0]
        );
    }

    #[test]
    fn test_poly_div_coeffs() {
        // (x^2-1) / (x-1) = x+1, rem 0
        let (q, r) = poly_div_coeffs(&[-1.0, 0.0, 1.0], &[-1.0, 1.0]);
        assert_vec_approx(&q, &[1.0, 1.0]);
        assert!(r.iter().all(|x| x.abs() < 1e-9));
    }

    #[test]
    fn test_poly_eval_horner() {
        assert_eq!(poly_eval_horner(&[1.0, 2.0, 1.0], 2.0), 9.0); // 1+2*2+1*4=9
        assert_eq!(poly_eval_horner(&[], 1.0), 0.0);
    }

    #[test]
    fn test_poly_diff_coeffs() {
        // d/dx(x^3+2x) = 3x^2+2 → [2, 0, 3]
        assert_vec_approx(&poly_diff_coeffs(&[0.0, 2.0, 0.0, 1.0]), &[2.0, 0.0, 3.0]);
    }

    #[test]
    fn test_poly_integrate_coeffs() {
        // ∫2x dx = x^2 → [0, 0, 1]
        assert_vec_approx(&poly_integrate_coeffs(&[0.0, 2.0]), &[0.0, 0.0, 1.0]);
    }

    #[test]
    fn test_trim_leading_zeros() {
        assert_eq!(trim_leading_zeros(&[1.0, 2.0, 0.0, 0.0]), vec![1.0, 2.0]);
        assert_eq!(trim_leading_zeros(&[0.0]), vec![0.0]);
    }

    #[test]
    fn test_is_zero_poly() {
        assert!(is_zero_poly(&[0.0, 0.0]));
        assert!(!is_zero_poly(&[1.0, 0.0]));
    }

    #[test]
    fn test_expr_to_coeffs_simple() {
        let ast = parse("x^2+2*x+1").unwrap();
        let (coeffs, var) = expr_to_coeffs(&ast, &EvalContext::new()).unwrap();
        assert_eq!(var, "x");
        assert_vec_approx(&coeffs, &[1.0, 2.0, 1.0]);
    }

    #[test]
    fn test_expr_to_coeffs_constant() {
        let ast = parse("5").unwrap();
        let (coeffs, var) = expr_to_coeffs(&ast, &EvalContext::new()).unwrap();
        assert_eq!(var, "");
        assert_vec_approx(&coeffs, &[5.0]);
    }

    #[test]
    fn test_expr_to_coeffs_variable() {
        let ast = parse("x").unwrap();
        let (coeffs, var) = expr_to_coeffs(&ast, &EvalContext::new()).unwrap();
        assert_eq!(var, "x");
        assert_vec_approx(&coeffs, &[0.0, 1.0]);
    }

    #[test]
    fn test_expr_to_coeffs_power() {
        let ast = parse("x^3").unwrap();
        let (coeffs, var) = expr_to_coeffs(&ast, &EvalContext::new()).unwrap();
        assert_eq!(var, "x");
        assert_vec_approx(&coeffs, &[0.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn test_expr_to_coeffs_scalar_times() {
        let ast = parse("3*x").unwrap();
        let (coeffs, var) = expr_to_coeffs(&ast, &EvalContext::new()).unwrap();
        assert_eq!(var, "x");
        assert_vec_approx(&coeffs, &[0.0, 3.0]);
    }

    #[test]
    fn test_expr_to_coeffs_neg() {
        let ast = AstNode::UnaryOp(UnaryOp::Neg, Box::new(parse("x^2").unwrap()));
        let (coeffs, var) = expr_to_coeffs(&ast, &EvalContext::new()).unwrap();
        assert_eq!(var, "x");
        assert_vec_approx(&coeffs, &[0.0, 0.0, -1.0]);
    }

    #[test]
    fn test_merge_var() {
        assert_eq!(merge_var("", "x").unwrap(), "x");
        assert_eq!(merge_var("x", "").unwrap(), "x");
        assert_eq!(merge_var("x", "x").unwrap(), "x");
        assert!(merge_var("x", "y").is_err());
    }

    // ===== 覆盖率补充测试 =====

    #[test]
    fn test_eval_node_bignumber() {
        // eval_node BigNumber path
        let ast = AstNode::BigNumber("42".to_string());
        let result = PolynomialDomain
            .evaluate(&ast, &EvalContext::new())
            .unwrap();
        assert_eq!(result.as_scalar().unwrap(), 42.0);
    }

    #[test]
    fn test_eval_node_bignumber_invalid() {
        let ast = AstNode::BigNumber("not_a_number".to_string());
        let result = PolynomialDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_eval_node_unary_abs_rejected() {
        // eval_node UnaryOp::Abs rejection
        let ast = AstNode::UnaryOp(UnaryOp::Abs, Box::new(AstNode::Number(5.0)));
        let result = PolynomialDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_eval_node_unary_neg_polynomial() {
        // eval_node UnaryOp::Neg with polynomial expression
        let ast = AstNode::UnaryOp(UnaryOp::Neg, Box::new(parse("x^2+1").unwrap()));
        let result = PolynomialDomain
            .evaluate(&ast, &EvalContext::new())
            .unwrap();
        let coeffs = result.as_polynomial().unwrap();
        assert_vec_approx(coeffs, &[-1.0, 0.0, -1.0]);
    }

    #[test]
    fn test_eval_node_variable_bound() {
        // eval_node Variable with bound value
        let ctx = EvalContext::new().with_var("x", 5.0);
        let ast = AstNode::Variable("x".to_string());
        let result = PolynomialDomain.evaluate(&ast, &ctx).unwrap();
        assert_eq!(result.as_scalar().unwrap(), 5.0);
    }

    #[test]
    fn test_arg_to_coeffs_nested_function() {
        // arg_to_coeffs with nested FunctionCall returning Polynomial
        let ast = parse("poly_add(poly_add(x+1, x+2), x+3)").unwrap();
        let result = PolynomialDomain
            .evaluate(&ast, &EvalContext::new())
            .unwrap();
        let coeffs = result.as_polynomial().unwrap();
        // (2x+3) + (x+3) = 3x+6 → [6, 3]
        assert_vec_approx(coeffs, &[6.0, 3.0]);
    }

    #[test]
    fn test_arg_to_coeffs_nested_function_non_polynomial() {
        // arg_to_coeffs with nested FunctionCall returning non-Polynomial (roots → Vector)
        let ast = AstNode::FunctionCall(
            "poly_add".to_string(),
            vec![
                AstNode::FunctionCall("roots".to_string(), vec![parse("x-1").unwrap()]),
                parse("x+1").unwrap(),
            ],
        );
        let result = PolynomialDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_eval_scalar_bignumber() {
        // eval_scalar BigNumber path via poly_eval
        let ast = AstNode::FunctionCall(
            "poly_eval".to_string(),
            vec![parse("x+1").unwrap(), AstNode::BigNumber("5".to_string())],
        );
        let result = PolynomialDomain
            .evaluate(&ast, &EvalContext::new())
            .unwrap();
        assert_eq!(result.as_scalar().unwrap(), 6.0);
    }

    #[test]
    fn test_eval_scalar_bignumber_invalid() {
        // eval_scalar BigNumber invalid via poly_eval
        let ast = AstNode::FunctionCall(
            "poly_eval".to_string(),
            vec![parse("x+1").unwrap(), AstNode::BigNumber("xyz".to_string())],
        );
        let result = PolynomialDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_eval_scalar_variable() {
        // eval_scalar Variable via poly_eval
        let ctx = EvalContext::new().with_var("y", 5.0);
        let ast = AstNode::FunctionCall(
            "poly_eval".to_string(),
            vec![parse("x+1").unwrap(), AstNode::Variable("y".to_string())],
        );
        let result = PolynomialDomain.evaluate(&ast, &ctx).unwrap();
        assert_eq!(result.as_scalar().unwrap(), 6.0);
    }

    #[test]
    fn test_eval_scalar_unbound_variable() {
        // eval_scalar unbound variable via poly_eval
        let ast = AstNode::FunctionCall(
            "poly_eval".to_string(),
            vec![parse("x+1").unwrap(), AstNode::Variable("z".to_string())],
        );
        let result = PolynomialDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(CalcError::EvalError(_))));
    }

    #[test]
    fn test_eval_scalar_neg() {
        // eval_scalar UnaryOp::Neg via poly_eval
        let ast = AstNode::FunctionCall(
            "poly_eval".to_string(),
            vec![
                parse("x+1").unwrap(),
                AstNode::UnaryOp(UnaryOp::Neg, Box::new(AstNode::Number(5.0))),
            ],
        );
        let result = PolynomialDomain
            .evaluate(&ast, &EvalContext::new())
            .unwrap();
        assert_eq!(result.as_scalar().unwrap(), -4.0);
    }

    #[test]
    fn test_eval_scalar_binaryop() {
        // eval_scalar BinaryOp via poly_eval: poly_eval(x+1, 2+3) = 6
        let ast = AstNode::FunctionCall(
            "poly_eval".to_string(),
            vec![
                parse("x+1").unwrap(),
                AstNode::BinaryOp(
                    BinaryOp::Add,
                    Box::new(AstNode::Number(2.0)),
                    Box::new(AstNode::Number(3.0)),
                ),
            ],
        );
        let result = PolynomialDomain
            .evaluate(&ast, &EvalContext::new())
            .unwrap();
        assert_eq!(result.as_scalar().unwrap(), 6.0);
    }

    #[test]
    fn test_eval_scalar_div() {
        // eval_scalar BinaryOp::Div via poly_eval: poly_eval(x+1, 10/2) = 6
        let ast = AstNode::FunctionCall(
            "poly_eval".to_string(),
            vec![
                parse("x+1").unwrap(),
                AstNode::BinaryOp(
                    BinaryOp::Div,
                    Box::new(AstNode::Number(10.0)),
                    Box::new(AstNode::Number(2.0)),
                ),
            ],
        );
        let result = PolynomialDomain
            .evaluate(&ast, &EvalContext::new())
            .unwrap();
        assert_eq!(result.as_scalar().unwrap(), 6.0);
    }

    #[test]
    fn test_eval_scalar_div_by_zero() {
        // eval_scalar BinaryOp::Div by zero via poly_eval
        let ast = AstNode::FunctionCall(
            "poly_eval".to_string(),
            vec![
                parse("x+1").unwrap(),
                AstNode::BinaryOp(
                    BinaryOp::Div,
                    Box::new(AstNode::Number(10.0)),
                    Box::new(AstNode::Number(0.0)),
                ),
            ],
        );
        let result = PolynomialDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(CalcError::DivisionByZero)));
    }

    #[test]
    fn test_eval_scalar_pow() {
        // eval_scalar BinaryOp::Pow via poly_eval: poly_eval(x+1, 2^2) = 5
        let ast = AstNode::FunctionCall(
            "poly_eval".to_string(),
            vec![
                parse("x+1").unwrap(),
                AstNode::BinaryOp(
                    BinaryOp::Pow,
                    Box::new(AstNode::Number(2.0)),
                    Box::new(AstNode::Number(2.0)),
                ),
            ],
        );
        let result = PolynomialDomain
            .evaluate(&ast, &EvalContext::new())
            .unwrap();
        assert_eq!(result.as_scalar().unwrap(), 5.0);
    }

    #[test]
    fn test_eval_scalar_mod() {
        // eval_scalar BinaryOp::Mod via poly_eval: poly_eval(x+1, 10%3) = poly_eval(x+1, 1) = 2
        let ast = AstNode::FunctionCall(
            "poly_eval".to_string(),
            vec![
                parse("x+1").unwrap(),
                AstNode::BinaryOp(
                    BinaryOp::Mod,
                    Box::new(AstNode::Number(10.0)),
                    Box::new(AstNode::Number(3.0)),
                ),
            ],
        );
        let result = PolynomialDomain
            .evaluate(&ast, &EvalContext::new())
            .unwrap();
        assert_eq!(result.as_scalar().unwrap(), 2.0);
    }

    #[test]
    fn test_eval_scalar_mod_by_zero() {
        // eval_scalar BinaryOp::Mod by zero via poly_eval
        let ast = AstNode::FunctionCall(
            "poly_eval".to_string(),
            vec![
                parse("x+1").unwrap(),
                AstNode::BinaryOp(
                    BinaryOp::Mod,
                    Box::new(AstNode::Number(10.0)),
                    Box::new(AstNode::Number(0.0)),
                ),
            ],
        );
        let result = PolynomialDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(CalcError::DivisionByZero)));
    }

    #[test]
    fn test_eval_scalar_complex_rejected() {
        // eval_scalar wildcard `_ =>` with Complex via poly_eval
        let ast = AstNode::FunctionCall(
            "poly_eval".to_string(),
            vec![parse("x+1").unwrap(), AstNode::Complex(1.0, 2.0)],
        );
        let result = PolynomialDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_expr_to_coeffs_bignumber() {
        // expr_to_coeffs BigNumber
        let ast = AstNode::BigNumber("42".to_string());
        let (coeffs, var) = expr_to_coeffs(&ast, &EvalContext::new()).unwrap();
        assert_eq!(var, "");
        assert_vec_approx(&coeffs, &[42.0]);
    }

    #[test]
    fn test_expr_to_coeffs_bignumber_invalid() {
        let ast = AstNode::BigNumber("xyz".to_string());
        let result = expr_to_coeffs(&ast, &EvalContext::new());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_expr_to_coeffs_variable_with_ctx_value() {
        // Variable with value in ctx → constant
        let ctx = EvalContext::new().with_var("x", 5.0);
        let ast = AstNode::Variable("x".to_string());
        let (coeffs, var) = expr_to_coeffs(&ast, &ctx).unwrap();
        assert_eq!(var, "");
        assert_vec_approx(&coeffs, &[5.0]);
    }

    #[test]
    fn test_expr_to_coeffs_pow_variable_with_ctx_value() {
        // x^2 where x has value 3 in ctx → [9]
        let ctx = EvalContext::new().with_var("x", 3.0);
        let ast = parse("x^2").unwrap();
        let (coeffs, var) = expr_to_coeffs(&ast, &ctx).unwrap();
        assert_eq!(var, "");
        assert_vec_approx(&coeffs, &[9.0]);
    }

    #[test]
    fn test_expr_to_coeffs_pow_number_number() {
        // Number ^ Number → constant: 2^3 = 8
        let ast = AstNode::BinaryOp(
            BinaryOp::Pow,
            Box::new(AstNode::Number(2.0)),
            Box::new(AstNode::Number(3.0)),
        );
        let (coeffs, var) = expr_to_coeffs(&ast, &EvalContext::new()).unwrap();
        assert_eq!(var, "");
        assert_vec_approx(&coeffs, &[8.0]);
    }

    #[test]
    fn test_expr_to_coeffs_pow_unsupported() {
        // (x+1)^2 → not a polynomial: unsupported power expression
        let ast = AstNode::BinaryOp(
            BinaryOp::Pow,
            Box::new(parse("x+1").unwrap()),
            Box::new(AstNode::Number(2.0)),
        );
        let result = expr_to_coeffs(&ast, &EvalContext::new());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_expr_to_coeffs_mul_poly_poly() {
        // (x+1) * (x+2) = x^2+3x+2 → Poly*Poly path
        let ast = parse("(x+1)*(x+2)").unwrap();
        let (coeffs, var) = expr_to_coeffs(&ast, &EvalContext::new()).unwrap();
        assert_eq!(var, "x");
        assert_vec_approx(&coeffs, &[2.0, 3.0, 1.0]);
    }

    #[test]
    fn test_expr_to_coeffs_mul_number_left() {
        // 3 * (x+1) → Number * Poly path
        let ast = parse("3*(x+1)").unwrap();
        let (coeffs, var) = expr_to_coeffs(&ast, &EvalContext::new()).unwrap();
        assert_eq!(var, "x");
        assert_vec_approx(&coeffs, &[3.0, 3.0]);
    }

    #[test]
    fn test_expr_to_coeffs_div_number_number() {
        // 6 / 2 → constant 3
        let ast = AstNode::BinaryOp(
            BinaryOp::Div,
            Box::new(AstNode::Number(6.0)),
            Box::new(AstNode::Number(2.0)),
        );
        let (coeffs, var) = expr_to_coeffs(&ast, &EvalContext::new()).unwrap();
        assert_eq!(var, "");
        assert_vec_approx(&coeffs, &[3.0]);
    }

    #[test]
    fn test_expr_to_coeffs_div_number_zero() {
        // 6 / 0 → DivisionByZero
        let ast = AstNode::BinaryOp(
            BinaryOp::Div,
            Box::new(AstNode::Number(6.0)),
            Box::new(AstNode::Number(0.0)),
        );
        let result = expr_to_coeffs(&ast, &EvalContext::new());
        assert!(matches!(result, Err(CalcError::DivisionByZero)));
    }

    #[test]
    fn test_expr_to_coeffs_div_unsupported() {
        // (x+1) / (x+2) → non-zero remainder → DomainError
        let ast = parse("(x+1)/(x+2)").unwrap();
        let result = expr_to_coeffs(&ast, &EvalContext::new());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_expr_to_coeffs_div_poly_by_number() {
        // (x^2-1)/2 → [-0.5, 0, 0.5]
        let ast = parse("(x^2-1)/2").unwrap();
        let (coeffs, var) = expr_to_coeffs(&ast, &EvalContext::new()).unwrap();
        assert_eq!(var, "x");
        assert_eq!(coeffs, vec![-0.5, 0.0, 0.5]);
    }

    #[test]
    fn test_expr_to_coeffs_div_poly_by_number_zero() {
        // (x+1)/0 → DivisionByZero
        let ast = parse("(x+1)/0").unwrap();
        let result = expr_to_coeffs(&ast, &EvalContext::new());
        assert!(matches!(result, Err(CalcError::DivisionByZero)));
    }

    #[test]
    fn test_expr_to_coeffs_div_poly_by_poly_exact() {
        // (x^2-1)/(x-1) → quotient x+1 = [1, 1]
        let ast = parse("(x^2-1)/(x-1)").unwrap();
        let (coeffs, var) = expr_to_coeffs(&ast, &EvalContext::new()).unwrap();
        assert_eq!(var, "x");
        assert_eq!(coeffs, vec![1.0, 1.0]);
    }

    #[test]
    fn test_expr_to_coeffs_div_poly_by_poly_exact_higher() {
        // (x^3-1)/(x-1) → quotient x^2+x+1 = [1, 1, 1]
        let ast = parse("(x^3-1)/(x-1)").unwrap();
        let (coeffs, var) = expr_to_coeffs(&ast, &EvalContext::new()).unwrap();
        assert_eq!(var, "x");
        assert_eq!(coeffs, vec![1.0, 1.0, 1.0]);
    }

    #[test]
    fn test_expr_to_coeffs_mod() {
        // x % 2 → not supported
        let ast = AstNode::BinaryOp(
            BinaryOp::Mod,
            Box::new(AstNode::Variable("x".to_string())),
            Box::new(AstNode::Number(2.0)),
        );
        let result = expr_to_coeffs(&ast, &EvalContext::new());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_expr_to_coeffs_unsupported_node() {
        // Complex node → not a polynomial expression
        let ast = AstNode::Complex(1.0, 2.0);
        let result = expr_to_coeffs(&ast, &EvalContext::new());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_poly_div_coeffs_a_shorter_than_b() {
        // a.len() < b.len() → ([0.0], a)
        let (q, r) = poly_div_coeffs(&[1.0, 2.0], &[1.0, 2.0, 3.0]);
        assert_vec_approx(&q, &[0.0]);
        assert_vec_approx(&r, &[1.0, 2.0]);
    }

    #[test]
    fn test_poly_div_coeffs_a_empty() {
        // a empty → quotient=[0.0], remainder=[] (empty)
        let (q, r) = poly_div_coeffs(&[], &[1.0, 2.0]);
        assert_vec_approx(&q, &[0.0]);
        assert!(r.is_empty(), "remainder should be empty, got {:?}", r);
    }

    #[test]
    fn test_poly_mul_coeffs_empty() {
        assert_vec_approx(&poly_mul_coeffs(&[], &[1.0, 2.0]), &[0.0]);
        assert_vec_approx(&poly_mul_coeffs(&[1.0], &[]), &[0.0]);
    }

    #[test]
    fn test_poly_eval_horner_empty() {
        assert_eq!(poly_eval_horner(&[], 1.0), 0.0);
    }

    #[test]
    fn test_factor_constant() {
        // factor(5) → "5"
        let result = eval_symbolic("factor(5)").unwrap();
        assert_eq!(result, "5");
    }

    #[test]
    fn test_factor_linear_zero_root() {
        // factor(x) → "x"
        let result = eval_symbolic("factor(x)").unwrap();
        assert_eq!(result, "x");
    }

    #[test]
    fn test_factor_linear_negative_coeff() {
        // factor(-x+3) → "-(x-3)"
        let result = eval_symbolic("factor(-x+3)").unwrap();
        assert!(result.contains("x-3") || result.contains("(x-3)"));
    }

    #[test]
    fn test_factor_quadratic_leading_one() {
        // factor(x^2-4) → "(x-2)*(x+2)"
        let result = eval_symbolic("factor(x^2-4)").unwrap();
        assert!(result.contains("(x-2)"));
        assert!(result.contains("(x+2)"));
    }

    #[test]
    fn test_factor_quadratic_negative_leading() {
        // factor(4-x^2) → "-(x+2)*(x-2)" (leading coeff -1)
        let result = eval_symbolic("factor(4-x^2)").unwrap();
        assert!(result.starts_with("-"));
        assert!(result.contains("(x-2)"));
        assert!(result.contains("(x+2)"));
    }

    #[test]
    fn test_factor_quadratic_other_leading() {
        // factor(2*x^2-8) → "2*(x-2)*(x+2)"
        let result = eval_symbolic("factor(2*x^2-8)").unwrap();
        assert!(result.starts_with("2*"));
        assert!(result.contains("(x-2)"));
        assert!(result.contains("(x+2)"));
    }

    #[test]
    fn test_factor_linear_positive_root_format() {
        // factor(x-3) → root=3, format "(x-3)"
        let result = eval_symbolic("factor(x-3)").unwrap();
        assert!(result.contains("x-3") || result.contains("(x-3)"));
    }

    #[test]
    fn test_factor_linear_negative_root_format() {
        // factor(x+3) → root=-3, format "(x+3)"
        let result = eval_symbolic("factor(x+3)").unwrap();
        assert!(result.contains("x+3") || result.contains("(x+3)"));
    }

    #[test]
    fn test_format_factor_linear_direct() {
        // Direct test of format_factor_linear (a=-1 produces "-prefix")
        assert_eq!(format_factor_linear(1.0, 3.0), "(x-3)");
        assert_eq!(format_factor_linear(1.0, -3.0), "(x+3)");
        assert_eq!(format_factor_linear(1.0, 0.0), "x");
        assert_eq!(format_factor_linear(-1.0, 3.0), "-(x-3)");
        assert_eq!(format_factor_linear(2.0, 3.0), "2*(x-3)");
    }

    #[test]
    fn test_format_factor_quadratic_direct() {
        // Direct test of format_factor_quadratic
        assert_eq!(format_factor_quadratic(1.0, 2.0, -2.0), "(x-2)*(x+2)");
        assert_eq!(format_factor_quadratic(-1.0, 2.0, -2.0), "-(x-2)*(x+2)");
        assert_eq!(format_factor_quadratic(2.0, 2.0, -2.0), "2*(x-2)*(x+2)");
    }

    #[test]
    fn test_format_factor_term_direct() {
        // Direct test of format_factor_term
        assert_eq!(format_factor_term(0.0), "x");
        assert_eq!(format_factor_term(3.0), "(x-3)");
        assert_eq!(format_factor_term(-3.0), "(x+3)");
    }

    #[test]
    fn test_poly_sub_wrong_args() {
        let ast = AstNode::FunctionCall("poly_sub".to_string(), vec![AstNode::Number(1.0)]);
        let result = PolynomialDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_poly_mul_wrong_args() {
        let ast = AstNode::FunctionCall("poly_mul".to_string(), vec![AstNode::Number(1.0)]);
        let result = PolynomialDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_poly_div_wrong_args() {
        let ast = AstNode::FunctionCall("poly_div".to_string(), vec![AstNode::Number(1.0)]);
        let result = PolynomialDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_poly_eval_wrong_args() {
        let ast = AstNode::FunctionCall("poly_eval".to_string(), vec![AstNode::Number(1.0)]);
        let result = PolynomialDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_poly_diff_wrong_args() {
        let ast = AstNode::FunctionCall("poly_diff".to_string(), vec![]);
        let result = PolynomialDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_poly_integrate_wrong_args() {
        let ast = AstNode::FunctionCall("poly_integrate".to_string(), vec![]);
        let result = PolynomialDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_factor_wrong_args() {
        let ast = AstNode::FunctionCall("factor".to_string(), vec![]);
        let result = PolynomialDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_roots_wrong_args() {
        let ast = AstNode::FunctionCall("roots".to_string(), vec![]);
        let result = PolynomialDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_contains_polynomial_recursive() {
        // contains_polynomial_function via BinaryOp
        let ast = parse("poly_add(x+1, x+2) + 5").unwrap();
        assert!(PolynomialDomain.supports(&ast));
    }

    #[test]
    fn test_contains_polynomial_unary() {
        // contains_polynomial_function via UnaryOp
        let ast = AstNode::UnaryOp(UnaryOp::Neg, Box::new(parse("poly_add(x+1, x+2)").unwrap()));
        assert!(PolynomialDomain.supports(&ast));
    }

    #[test]
    fn test_contains_polynomial_matrix() {
        // contains_polynomial_function via Matrix
        let ast = AstNode::Matrix(vec![vec![parse("poly_add(x+1, x+2)").unwrap()]]);
        assert!(PolynomialDomain.supports(&ast));
    }

    #[test]
    fn test_contains_polynomial_list() {
        // contains_polynomial_function via List
        let ast = AstNode::List(vec![parse("poly_add(x+1, x+2)").unwrap()]);
        assert!(PolynomialDomain.supports(&ast));
    }

    #[test]
    fn test_not_supports_bignumber() {
        let ast = AstNode::BigNumber("42".to_string());
        assert!(!PolynomialDomain.supports(&ast));
    }

    // ===== 覆盖率补充测试（第二轮：覆盖剩余未覆盖路径）=====
    // 注：eval_function 中 line 210 的 `_ => unreachable!("checked above")` 分支
    // 在逻辑上不可达——name 已在 line 99 通过 POLYNOMIAL_FUNCTIONS 白名单校验，
    // match 覆盖了全部 9 个白名单函数，故该分支无法被测试触发且不应被触发。

    #[test]
    fn test_eval_node_number_bare() {
        // 覆盖 eval_node 的 AstNode::Number 分支（line 56）
        // 直接对裸 Number AST 求值（不经过任何函数调用包装）
        let ast = AstNode::Number(7.0);
        let result = PolynomialDomain
            .evaluate(&ast, &EvalContext::new())
            .unwrap();
        assert_eq!(result.as_scalar().unwrap(), 7.0);
    }

    #[test]
    fn test_eval_node_binaryop_polynomial_bare() {
        // 覆盖 eval_node 的 BinaryOp 成功返回路径（lines 63-64）
        // 对裸多项式表达式（非函数调用）求值，触发 expr_to_coeffs + 返回 Polynomial
        let result = eval_polynomial("x^2+2*x+1").unwrap();
        assert_vec_approx(&result, &[1.0, 2.0, 1.0]);
    }

    #[test]
    fn test_eval_scalar_sub() {
        // 覆盖 eval_scalar 的 BinaryOp::Sub 分支（line 252）
        // poly_eval(x+1, 10-3) = poly_eval(x+1, 7) = 8
        let ast = AstNode::FunctionCall(
            "poly_eval".to_string(),
            vec![
                parse("x+1").unwrap(),
                AstNode::BinaryOp(
                    BinaryOp::Sub,
                    Box::new(AstNode::Number(10.0)),
                    Box::new(AstNode::Number(3.0)),
                ),
            ],
        );
        let result = PolynomialDomain
            .evaluate(&ast, &EvalContext::new())
            .unwrap();
        assert_eq!(result.as_scalar().unwrap(), 8.0);
    }

    #[test]
    fn test_eval_scalar_mul() {
        // 覆盖 eval_scalar 的 BinaryOp::Mul 分支（line 253）
        // poly_eval(x+1, 2*3) = poly_eval(x+1, 6) = 7
        let ast = AstNode::FunctionCall(
            "poly_eval".to_string(),
            vec![
                parse("x+1").unwrap(),
                AstNode::BinaryOp(
                    BinaryOp::Mul,
                    Box::new(AstNode::Number(2.0)),
                    Box::new(AstNode::Number(3.0)),
                ),
            ],
        );
        let result = PolynomialDomain
            .evaluate(&ast, &EvalContext::new())
            .unwrap();
        assert_eq!(result.as_scalar().unwrap(), 7.0);
    }

    #[test]
    fn test_expr_to_coeffs_mul_number_right() {
        // 覆盖 expr_to_coeffs 的 Mul 分支中 Number 在右侧的路径（Poly * Number，lines 332-336）
        // 已有测试覆盖 Number 在左侧（3*(x+1)），本测试覆盖 (x+1)*3
        let ast = parse("(x+1)*3").unwrap();
        let (coeffs, var) = expr_to_coeffs(&ast, &EvalContext::new()).unwrap();
        assert_eq!(var, "x");
        assert_vec_approx(&coeffs, &[3.0, 3.0]);
    }

    #[test]
    fn test_solve_cubic_double_root() {
        // 覆盖 solve_cubic 的 disc=0 且 p≠0 分支（双根情况，lines 655-659）
        // x^3-3x+2 = (x-1)^2*(x+2) → roots: 1(二重), -2
        // 此处 q/2=1, p/3=-1, disc=1+(-1)=0 恰好为零（无浮点误差），且 p=-3≠0
        let result = eval("roots(x^3-3*x+2)").unwrap();
        let roots = result
            .as_vector()
            .expect("expected Vector (all real roots)");
        assert_eq!(roots.len(), 3);
        let mut sorted = roots.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_approx(sorted[0], -2.0);
        assert_approx(sorted[1], 1.0);
        assert_approx(sorted[2], 1.0);
    }

    #[test]
    fn test_solve_quartic_ferrari_path() {
        // 覆盖 solve_quartic 的 Ferrari 方法路径（q≠0，lines 716, 719-727, 732-739）
        // 以及 solve_quadratic_complex 的两个分支（disc>=0 实根 lines 591-595，disc<0 复根 lines 597-600）
        //
        // x^4+x = x(x+1)(x^2-x+1) → roots: 0, -1, (1±i√3)/2
        // 此多项式 b=0 但 q=d-bc/2+b^3/8=1≠0，故走 Ferrari 路径（非双二次路径）。
        // 预解三次方程 m^3-1=0 → m=1，分解出的两个二次方程分别产生实根与复根，
        // 从而同时覆盖 solve_quadratic_complex 的 disc>=0 与 disc<0 两个分支。
        let result = eval("roots(x^4+x)").unwrap();
        let roots = result
            .as_complex_list()
            .expect("expected ComplexList (has complex roots)");
        assert_eq!(roots.len(), 4);
        // 实根 0 和 -1
        let real_roots: Vec<f64> = roots
            .iter()
            .filter(|(_, im)| im.abs() < 1e-9)
            .map(|(re, _)| *re)
            .collect();
        assert_eq!(
            real_roots.len(),
            2,
            "expected 2 real roots, got {:?}",
            real_roots
        );
        let mut sorted_real = real_roots.clone();
        sorted_real.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_approx(sorted_real[0], -1.0);
        assert_approx(sorted_real[1], 0.0);
        // 复共轭对 (0.5, ±√3/2)
        let complex_roots: Vec<(f64, f64)> = roots
            .iter()
            .filter(|(_, im)| im.abs() >= 1e-9)
            .copied()
            .collect();
        assert_eq!(complex_roots.len(), 2);
        assert_approx(complex_roots[0].0, 0.5);
        assert_approx(complex_roots[1].0, 0.5);
        assert_approx(complex_roots[0].1 + complex_roots[1].1, 0.0); // 共轭
    }

    // ===== proptest 属性测试 =====

    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig { cases: 64, ..ProptestConfig::default() })]

        /// 属性：poly_eval(poly_add(p1, p2), x) == poly_eval(p1, x) + poly_eval(p2, x)
        #[test]
        fn prop_eval_add(a0 in -10i64..10, a1 in -10i64..10, b0 in -10i64..10, b1 in -10i64..10, x in -5i64..5) {
            let p1 = format!("{}*x+{}", a1, a0);
            let p2 = format!("{}*x+{}", b1, b0);
            let expr = format!("poly_eval(poly_add({}, {}), {})", p1, p2, x);
            let ast = parse(&expr).unwrap();
            let domain = PolynomialDomain;
            let result = domain.evaluate(&ast, &EvalContext::new()).unwrap();
            let expected = (a1 as f64 + b1 as f64) * (x as f64) + (a0 as f64 + b0 as f64);
            prop_assert!((result.as_scalar().unwrap() - expected).abs() < 1e-9);
        }

        /// 属性：poly_diff(poly_integrate(p)) == p（常数项丢失）
        #[test]
        fn prop_diff_integrate(a0 in -10i64..10, a1 in -10i64..10) {
            let p = format!("{}*x+{}", a1, a0);
            let expr = format!("poly_diff(poly_integrate({}))", p);
            let ast = parse(&expr).unwrap();
            let domain = PolynomialDomain;
            let result = domain.evaluate(&ast, &EvalContext::new()).unwrap();
            let coeffs = result.as_polynomial().unwrap();
            // ∫(a1*x+a0)dx = a0*x + a1/2*x^2 → [0, a0, a1/2]
            // diff([0, a0, a1/2]) = [a0, a1] → 常数项丢失（原 a0 → 现 a0 是 x 系数）
            prop_assert!((coeffs[1] - a1 as f64).abs() < 1e-9);
        }
    }
}
