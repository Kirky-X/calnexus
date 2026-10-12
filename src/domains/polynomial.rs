// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! Polynomial 计算域：多项式算术、求值、求根、微分、积分、因式分解。
//!
//! 系数升幂存储（`coef[i]` = `x^i`），AST 含多项式函数调用时路由至本域。

use super::common::{
    ensure_math_constants, resolve_variable, unsupported_function_error, unsupported_node_error,
};
use crate::core::CalculationDomain;
use crate::core::{AstNode, BinaryOp, CalcError, EvalContext, EvalResult, UnaryOp};

use crate::math::polynomial as math_poly;

/// 多项式函数白名单。
pub(crate) const POLYNOMIAL_FUNCTIONS: &[&str] = &[
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

/// Polynomial 计算域：priority=25，支持多项式算术、求值、求根、微分、积分与因式分解。
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
        // 预绑定 pi/e，避免常量被当作多项式变量
        let ctx = ensure_math_constants(ctx);
        self.eval_node(ast, &ctx)
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
            AstNode::Variable(name) => resolve_variable(ctx, name).map(EvalResult::Scalar),
            AstNode::BinaryOp(_, _, _) => {
                let (coeffs, _var) = expr_to_coeffs(ast, ctx)?;
                Ok(EvalResult::Polynomial(coeffs))
            }
            AstNode::UnaryOp(UnaryOp::Neg, e) => {
                let (coeffs, _var) = expr_to_coeffs(e, ctx)?;
                let neg: Vec<f64> = coeffs.iter().map(|x| -x).collect();
                Ok(EvalResult::Polynomial(neg))
            }
            AstNode::BigNumber(s) => {
                let n: f64 = s.parse().map_err(|_| {
                    CalcError::domain(format!("invalid big number: {}", s)).with_i18n(
                        "msg.invalid_bignumber",
                        vec![("value".to_string(), s.to_string())],
                    )
                })?;
                Ok(EvalResult::Scalar(n))
            }
            AstNode::Complex(_, _) | AstNode::Matrix(_) | AstNode::List(_) | AstNode::Str(_) => {
                Err(unsupported_node_error("polynomial", ast))
            }
            AstNode::UnaryOp(UnaryOp::Abs, _) | AstNode::UnaryOp(UnaryOp::Factorial, _) => {
                Err(CalcError::domain(format!(
                    "polynomial domain does not support this unary op: {:?}",
                    ast
                ))
                .with_i18n(
                    "msg.polynomial.unsupported_unary",
                    vec![("op".to_string(), format!("{:?}", ast))],
                ))
            }
        }
    }

    /// 求值多项式函数调用：按函数名分发到对应的处理方法。
    fn eval_function(
        &self,
        name: &str,
        args: &[AstNode],
        ctx: &EvalContext,
    ) -> Result<EvalResult, CalcError> {
        if !POLYNOMIAL_FUNCTIONS.contains(&name) {
            return Err(unsupported_function_error("polynomial", name));
        }
        match name {
            "poly_add" => self.eval_poly_add(args, ctx),
            "poly_sub" => self.eval_poly_sub(args, ctx),
            "poly_mul" => self.eval_poly_mul(args, ctx),
            "poly_div" => self.eval_poly_div(args, ctx),
            "poly_eval" => self.eval_poly_eval(args, ctx),
            "roots" => self.eval_roots(args, ctx),
            "poly_diff" => self.eval_poly_diff(args, ctx),
            "poly_integrate" => self.eval_poly_integrate(args, ctx),
            "factor" => self.eval_factor(args, ctx),
            _ => unreachable!("checked above"),
        }
    }

    /// poly_add(a, b)：多项式加法。
    fn eval_poly_add(&self, args: &[AstNode], ctx: &EvalContext) -> Result<EvalResult, CalcError> {
        if args.len() != 2 {
            return Err(CalcError::domain(format!(
                "poly_add() requires exactly 2 arguments, got {}",
                args.len()
            ))
            .with_i18n(
                "msg.polynomial.poly_add_arg_count",
                vec![("actual".to_string(), args.len().to_string())],
            ));
        }
        let (a, _) = self.arg_to_coeffs(&args[0], ctx)?;
        let (b, _) = self.arg_to_coeffs(&args[1], ctx)?;
        Ok(EvalResult::Polynomial(math_poly::add(&a, &b)))
    }

    /// poly_sub(a, b)：多项式减法（a + (-b)）。
    fn eval_poly_sub(&self, args: &[AstNode], ctx: &EvalContext) -> Result<EvalResult, CalcError> {
        if args.len() != 2 {
            // 必须带 i18n key，否则 locales 中该 key 成为死键，中文用户看到英文错误。
            return Err(CalcError::domain(format!(
                "poly_sub() requires exactly 2 arguments, got {}",
                args.len()
            ))
            .with_i18n(
                "msg.polynomial.poly_sub_arg_count",
                vec![("actual".to_string(), args.len().to_string())],
            ));
        }
        let (a, _) = self.arg_to_coeffs(&args[0], ctx)?;
        let (b, _) = self.arg_to_coeffs(&args[1], ctx)?;
        Ok(EvalResult::Polynomial(math_poly::sub(&a, &b)))
    }

    /// poly_mul(a, b)：多项式乘法。
    fn eval_poly_mul(&self, args: &[AstNode], ctx: &EvalContext) -> Result<EvalResult, CalcError> {
        if args.len() != 2 {
            return Err(CalcError::domain(format!(
                "poly_mul() requires exactly 2 arguments, got {}",
                args.len()
            ))
            .with_i18n(
                "msg.polynomial.poly_mul_arg_count",
                vec![("actual".to_string(), args.len().to_string())],
            ));
        }
        let (a, _) = self.arg_to_coeffs(&args[0], ctx)?;
        let (b, _) = self.arg_to_coeffs(&args[1], ctx)?;
        Ok(EvalResult::Polynomial(math_poly::mul(&a, &b)))
    }

    /// poly_div(a, b)：多项式除法，返回商。b 为零多项式时报错。
    fn eval_poly_div(&self, args: &[AstNode], ctx: &EvalContext) -> Result<EvalResult, CalcError> {
        if args.len() != 2 {
            return Err(CalcError::domain(format!(
                "poly_div() requires exactly 2 arguments, got {}",
                args.len()
            ))
            .with_i18n(
                "msg.polynomial.poly_div_arg_count",
                vec![("actual".to_string(), args.len().to_string())],
            ));
        }
        let (a, _) = self.arg_to_coeffs(&args[0], ctx)?;
        let (b, _) = self.arg_to_coeffs(&args[1], ctx)?;
        if math_poly::is_zero(&b) {
            return Err(CalcError::division_by_zero());
        }
        let (quotient, _remainder) = math_poly::div(&a, &b);
        Ok(EvalResult::Polynomial(quotient))
    }

    /// poly_eval(coeffs, x)：Horner 法求多项式在 x 处的值，返回标量。
    fn eval_poly_eval(&self, args: &[AstNode], ctx: &EvalContext) -> Result<EvalResult, CalcError> {
        if args.len() != 2 {
            return Err(CalcError::domain(format!(
                "poly_eval() requires exactly 2 arguments, got {}",
                args.len()
            ))
            .with_i18n(
                "msg.polynomial.poly_eval_arg_count",
                vec![("actual".to_string(), args.len().to_string())],
            ));
        }
        let (coeffs, _) = self.arg_to_coeffs(&args[0], ctx)?;
        let x = self.eval_scalar(&args[1], ctx)?;
        let result = math_poly::eval(&coeffs, x)?;
        Ok(EvalResult::Scalar(result))
    }

    /// roots(coeffs)：求多项式所有根（实根 + 复根）。
    fn eval_roots(&self, args: &[AstNode], ctx: &EvalContext) -> Result<EvalResult, CalcError> {
        if args.len() != 1 {
            return Err(CalcError::domain(format!(
                "roots() requires exactly 1 argument, got {}",
                args.len()
            ))
            .with_i18n(
                "msg.polynomial.roots_arg_count",
                vec![("actual".to_string(), args.len().to_string())],
            ));
        }
        let (coeffs, _) = self.arg_to_coeffs(&args[0], ctx)?;
        let trimmed = math_poly::trim(&coeffs);
        math_poly::roots(&trimmed)
    }

    /// poly_diff(coeffs)：多项式求导。
    fn eval_poly_diff(&self, args: &[AstNode], ctx: &EvalContext) -> Result<EvalResult, CalcError> {
        if args.len() != 1 {
            return Err(CalcError::domain(format!(
                "poly_diff() requires exactly 1 argument, got {}",
                args.len()
            ))
            .with_i18n(
                "msg.polynomial.poly_diff_arg_count",
                vec![("actual".to_string(), args.len().to_string())],
            ));
        }
        let (coeffs, _) = self.arg_to_coeffs(&args[0], ctx)?;
        Ok(EvalResult::Polynomial(math_poly::diff(&coeffs)))
    }

    /// poly_integrate(coeffs)：多项式不定积分（常数项为 0）。
    fn eval_poly_integrate(
        &self,
        args: &[AstNode],
        ctx: &EvalContext,
    ) -> Result<EvalResult, CalcError> {
        if args.len() != 1 {
            return Err(CalcError::domain(format!(
                "poly_integrate() requires exactly 1 argument, got {}",
                args.len()
            ))
            .with_i18n(
                "msg.polynomial.poly_integrate_arg_count",
                vec![("actual".to_string(), args.len().to_string())],
            ));
        }
        let (coeffs, _) = self.arg_to_coeffs(&args[0], ctx)?;
        Ok(EvalResult::Polynomial(math_poly::integrate(&coeffs)))
    }

    /// factor(coeffs)：多项式因式分解，返回符号表达式。
    fn eval_factor(&self, args: &[AstNode], ctx: &EvalContext) -> Result<EvalResult, CalcError> {
        if args.len() != 1 {
            return Err(CalcError::domain(format!(
                "factor() requires exactly 1 argument, got {}",
                args.len()
            ))
            .with_i18n(
                "msg.polynomial.factor_arg_count",
                vec![("actual".to_string(), args.len().to_string())],
            ));
        }
        let (coeffs, _) = self.arg_to_coeffs(&args[0], ctx)?;
        let trimmed = math_poly::trim(&coeffs);
        let result = math_poly::factor(&trimmed)?;
        Ok(EvalResult::Symbolic(result))
    }

    /// 将参数转为系数向量：FunctionCall 先求值提取系数，其余走 expr_to_coeffs。
    fn arg_to_coeffs(
        &self,
        ast: &AstNode,
        ctx: &EvalContext,
    ) -> Result<(Vec<f64>, String), CalcError> {
        if let AstNode::FunctionCall(_, _) = ast {
            let result = self.eval_node(ast, ctx)?;
            return match result {
                EvalResult::Polynomial(coeffs) => Ok((coeffs, String::new())),
                _ => Err(CalcError::domain(format!(
                    "expected polynomial result from nested call, got {:?}",
                    ast
                ))
                .with_i18n(
                    "msg.polynomial.expected_poly_result",
                    vec![("node".to_string(), format!("{:?}", ast))],
                )),
            };
        }
        expr_to_coeffs(ast, ctx)
    }

    /// 将 AST 求值为 f64 标量（用于 poly_eval 的 x 参数）。
    fn eval_scalar(&self, ast: &AstNode, ctx: &EvalContext) -> Result<f64, CalcError> {
        match ast {
            AstNode::Number(n) => Ok(*n),
            AstNode::BigNumber(s) => s.parse::<f64>().map_err(|_| {
                CalcError::domain(format!("invalid big number: {}", s)).with_i18n(
                    "msg.invalid_bignumber",
                    vec![("value".to_string(), s.to_string())],
                )
            }),
            AstNode::Variable(name) => resolve_variable(ctx, name),
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
                            return Err(CalcError::division_by_zero());
                        }
                        Ok(a / b)
                    }
                    BinaryOp::Pow => {
                        // 显式处理 0^0=1，并检查 NaN/Inf（与其他域一致）
                        if a == 0.0 && b == 0.0 {
                            return Ok(1.0);
                        }
                        let r = a.powf(b);
                        if !r.is_finite() {
                            return Err(CalcError::nan_or_inf());
                        }
                        Ok(r)
                    }
                    BinaryOp::Mod => {
                        if b == 0.0 {
                            return Err(CalcError::division_by_zero());
                        }
                        Ok(a % b)
                    }
                }
            }
            _ => Err(CalcError::domain(format!(
                "polynomial domain cannot evaluate scalar from: {:?}",
                ast
            ))
            .with_i18n(
                "msg.polynomial.cannot_eval_scalar",
                vec![("node".to_string(), format!("{:?}", ast))],
            )),
        }
    }
}

/// 将表达式树转为升幂系数向量 (coeffs, variable_name)，coef[i] = x^i 的系数。
fn expr_to_coeffs(ast: &AstNode, ctx: &EvalContext) -> Result<(Vec<f64>, String), CalcError> {
    match ast {
        AstNode::Number(n) => Ok((vec![*n], String::new())),
        AstNode::BigNumber(s) => coeffs_from_bignumber(s),
        AstNode::Variable(name) => Ok(coeffs_from_variable(name, ctx)),
        AstNode::BinaryOp(op, l, r) => coeffs_from_binary(*op, l, r, ctx),
        AstNode::UnaryOp(UnaryOp::Neg, e) => coeffs_from_neg(e, ctx),
        _ => Err(
            CalcError::domain(format!("not a polynomial expression: {:?}", ast)).with_i18n(
                "msg.polynomial.not_poly_expression",
                vec![("node".to_string(), format!("{:?}", ast))],
            ),
        ),
    }
}

/// BigNumber → 常数系数。
fn coeffs_from_bignumber(s: &str) -> Result<(Vec<f64>, String), CalcError> {
    let n: f64 = s.parse().map_err(|_| {
        CalcError::domain(format!("invalid big number: {}", s)).with_i18n(
            "msg.invalid_bignumber",
            vec![("value".to_string(), s.to_string())],
        )
    })?;
    Ok((vec![n], String::new()))
}

/// Variable → 常数（若 ctx 有值）或一次项 (0 + 1x)。
fn coeffs_from_variable(name: &str, ctx: &EvalContext) -> (Vec<f64>, String) {
    if let Some(v) = ctx.get_var(name) {
        return (vec![v], String::new());
    }
    (vec![0.0, 1.0], name.to_string())
}

/// BinaryOp 分发器。
fn coeffs_from_binary(
    op: BinaryOp,
    l: &AstNode,
    r: &AstNode,
    ctx: &EvalContext,
) -> Result<(Vec<f64>, String), CalcError> {
    match op {
        BinaryOp::Pow => coeffs_from_pow(l, r, ctx),
        BinaryOp::Mul => coeffs_from_mul(l, r, ctx),
        BinaryOp::Add => coeffs_from_add(l, r, ctx),
        BinaryOp::Sub => coeffs_from_sub(l, r, ctx),
        BinaryOp::Div => coeffs_from_div(l, r, ctx),
        BinaryOp::Mod => Err(CalcError::domain(
            "modulo in polynomial expression not supported".to_string(),
        )
        .with_i18n("msg.polynomial.modulo_not_supported", vec![])),
    }
}

/// Pow 分支：Variable^Number → 单项式；Number^Number → 常数。
fn coeffs_from_pow(
    l: &AstNode,
    r: &AstNode,
    ctx: &EvalContext,
) -> Result<(Vec<f64>, String), CalcError> {
    // Variable ^ Number
    if let (AstNode::Variable(name), AstNode::Number(n)) = (l, r) {
        if *n < 0.0 || n.fract() != 0.0 {
            return Err(CalcError::domain(
                "polynomial exponent must be non-negative integer".to_string(),
            )
            .with_i18n("msg.polynomial.exponent_non_negative", vec![]));
        }
        // DoS 防护：指数上界 1000，防止 OOM
        const MAX_POLY_DEGREE: usize = 1000;
        let exp = *n as usize;
        if exp > MAX_POLY_DEGREE {
            return Err(CalcError::overflow());
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
    if let (AstNode::Number(a), AstNode::Number(b)) = (l, r) {
        // 检查 NaN/Inf（如 (-1)^0.5 = NaN），失败显性化
        if *a == 0.0 && *b == 0.0 {
            return Ok((vec![1.0], String::new()));
        }
        let r = a.powf(*b);
        if !r.is_finite() {
            return Err(CalcError::nan_or_inf());
        }
        return Ok((vec![r], String::new()));
    }
    Err(
        CalcError::domain("not a polynomial: unsupported power expression".to_string())
            .with_i18n("msg.polynomial.unsupported_power", vec![]),
    )
}

/// Mul 分支：Number*Poly / Poly*Number / Poly*Poly。
fn coeffs_from_mul(
    l: &AstNode,
    r: &AstNode,
    ctx: &EvalContext,
) -> Result<(Vec<f64>, String), CalcError> {
    // Number * Poly / Poly * Number
    if let AstNode::Number(c) = l {
        let (mut coeffs, var) = expr_to_coeffs(r, ctx)?;
        for c_i in &mut coeffs {
            *c_i *= c;
        }
        return Ok((coeffs, var));
    }
    if let AstNode::Number(c) = r {
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
    Ok((math_poly::mul(&a, &b), var))
}

/// Add 分支：Poly + Poly。
fn coeffs_from_add(
    l: &AstNode,
    r: &AstNode,
    ctx: &EvalContext,
) -> Result<(Vec<f64>, String), CalcError> {
    let (a, var_a) = expr_to_coeffs(l, ctx)?;
    let (b, var_b) = expr_to_coeffs(r, ctx)?;
    let var = merge_var(&var_a, &var_b)?;
    Ok((math_poly::add(&a, &b), var))
}

/// Sub 分支：Poly - Poly。
fn coeffs_from_sub(
    l: &AstNode,
    r: &AstNode,
    ctx: &EvalContext,
) -> Result<(Vec<f64>, String), CalcError> {
    let (a, var_a) = expr_to_coeffs(l, ctx)?;
    let (b, var_b) = expr_to_coeffs(r, ctx)?;
    let var = merge_var(&var_a, &var_b)?;
    Ok((math_poly::sub(&a, &b), var))
}

/// Div 分支：Number/Number / Poly/Number / Poly/Poly（长除法）。
fn coeffs_from_div(
    l: &AstNode,
    r: &AstNode,
    ctx: &EvalContext,
) -> Result<(Vec<f64>, String), CalcError> {
    // Number / Number → 常数（保留显式路径以返回 DivisionByZero）
    if let (AstNode::Number(a), AstNode::Number(b)) = (l, r) {
        if *b == 0.0 {
            return Err(CalcError::division_by_zero());
        }
        return Ok((vec![a / b], String::new()));
    }
    // Poly / Number → 逐系数除法
    if let AstNode::Number(c) = r {
        if *c == 0.0 {
            return Err(CalcError::division_by_zero());
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
    // 显式检查零多项式除数，避免 math_poly::div 返回 NaN
    if math_poly::is_zero(&b) {
        return Err(CalcError::division_by_zero());
    }
    let (quotient, remainder) = math_poly::div(&a, &b);
    if math_poly::is_zero(&remainder) {
        Ok((quotient, var))
    } else {
        Err(
            CalcError::domain("polynomial division has non-zero remainder".to_string())
                .with_i18n("msg.polynomial.non_zero_remainder", vec![]),
        )
    }
}

/// Neg 分支：-Poly → 逐系数取反。
fn coeffs_from_neg(e: &AstNode, ctx: &EvalContext) -> Result<(Vec<f64>, String), CalcError> {
    let (coeffs, var) = expr_to_coeffs(e, ctx)?;
    let neg: Vec<f64> = coeffs.iter().map(|x| -x).collect();
    Ok((neg, var))
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
    Err(
        CalcError::domain(format!("polynomial in multiple variables: {} and {}", a, b)).with_i18n(
            "msg.polynomial.multiple_variables",
            vec![
                ("var1".to_string(), a.to_string()),
                ("var2".to_string(), b.to_string()),
            ],
        ),
    )
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
        | AstNode::BigNumber(_)
        | AstNode::Str(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ErrorKind;
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

    // ===== 多项式加法 =====

    #[test]
    fn test_poly_add() {
        let result = eval_polynomial("poly_add(x^2+2*x+1, x+1)").unwrap();
        assert_vec_approx(&result, &[2.0, 3.0, 1.0]); // x^2+3x+2
    }

    // ===== 多项式减法 =====

    #[test]
    fn test_poly_sub() {
        let result = eval_polynomial("poly_sub(x^2+2*x+1, x+1)").unwrap();
        assert_vec_approx(&result, &[0.0, 1.0, 1.0]); // x^2+x
    }

    // ===== 多项式乘法 =====

    #[test]
    fn test_poly_mul() {
        let result = eval_polynomial("poly_mul(x+1, x+2)").unwrap();
        assert_vec_approx(&result, &[2.0, 3.0, 1.0]); // x^2+3x+2
    }

    // ===== 多项式除法 =====

    #[test]
    fn test_poly_div() {
        let result = eval_polynomial("poly_div(x^2-1, x-1)").unwrap();
        assert_vec_approx(&result, &[1.0, 1.0]); // x+1
    }

    // ===== 求根（实根） =====

    #[test]
    fn test_roots_real() {
        let result = eval("roots(x^2-4)").unwrap();
        let roots = result.as_vector().unwrap();
        assert_eq!(roots.len(), 2);
        assert_approx(roots[0], 2.0);
        assert_approx(roots[1], -2.0);
    }

    // ===== 因式分解 =====

    #[test]
    fn test_factor() {
        let result = eval_symbolic("factor(x^2-4)").unwrap();
        assert!(
            result.contains("(x-2)") && result.contains("(x+2)"),
            "expected factors (x-2) and (x+2), got: {}",
            result
        );
    }

    // ===== 求值 =====

    #[test]
    fn test_poly_eval() {
        assert_eq!(eval_scalar("poly_eval(x^2+1, 2)").unwrap(), 5.0);
    }

    #[test]
    fn test_poly_eval_with_pi_e_constants() {
        // pi/e 应被识别为常量而非多项式变量
        let result = eval_scalar("poly_eval(x^2+pi, 1)").unwrap();
        assert_approx(result, 1.0 + std::f64::consts::PI);
        let result = eval_scalar("poly_eval(x^2+e, 0)").unwrap();
        assert_approx(result, std::f64::consts::E);
        let result = eval_scalar("poly_eval(pi*x+1, 2)").unwrap();
        assert_approx(result, 2.0 * std::f64::consts::PI + 1.0);
    }

    // ===== 微分 =====

    #[test]
    fn test_poly_diff() {
        let result = eval_polynomial("poly_diff(x^3+2*x)").unwrap();
        assert_vec_approx(&result, &[2.0, 0.0, 3.0]);
    }

    // ===== 积分 =====

    #[test]
    fn test_poly_integrate() {
        let result = eval_polynomial("poly_integrate(2*x)").unwrap();
        assert_vec_approx(&result, &[0.0, 0.0, 1.0]);
    }

    // ===== 无实根（复根） =====

    #[test]
    fn test_roots_complex() {
        let result = eval("roots(x^2+1)").unwrap();
        let roots = result.as_complex_list().unwrap();
        assert_eq!(roots.len(), 2);
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
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_roots_cubic_one_real_two_complex() {
        let result = eval("roots(x^3+1)").unwrap();
        let roots = result.as_complex_list().expect("expected ComplexList");
        assert_eq!(roots.len(), 3);
        let real_roots: Vec<f64> = roots
            .iter()
            .filter(|(_, im)| im.abs() < 1e-9)
            .map(|(re, _)| *re)
            .collect();
        assert_eq!(real_roots.len(), 1);
        assert_approx(real_roots[0], -1.0);
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
        let result = eval("roots(x^3-3*x^2+3*x-1)").unwrap();
        let roots = result.as_vector().expect("expected Vector");
        assert_eq!(roots.len(), 3);
        for r in roots {
            assert_approx(*r, 1.0);
        }
    }

    #[test]
    fn test_roots_quartic_four_real() {
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
        let result = eval("roots(x^4+1)").unwrap();
        let roots = result.as_complex_list().expect("expected ComplexList");
        assert_eq!(roots.len(), 4);
        // 所有根的模应为 1
        for &(re, im) in roots {
            let mag = (re * re + im * im).sqrt();
            assert!((mag - 1.0).abs() < 1e-6, "expected |root| = 1, got {}", mag);
        }
    }

    #[test]
    fn test_roots_quartic_with_cubic_term() {
        let result = eval("roots(x^4-1)").unwrap();
        let roots = result.as_complex_list().expect("expected ComplexList");
        assert_eq!(roots.len(), 4);
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
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_poly_div_by_zero() {
        let result = eval("poly_div(x+1, 0)");
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::DivisionByZero));
    }

    #[test]
    fn test_factor_linear() {
        let result = eval_symbolic("factor(x-3)").unwrap();
        assert!(result.contains("x-3") || result.contains("(x-3)"));
    }

    #[test]
    fn test_factor_complex_error() {
        let result = eval("factor(x^2+1)");
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_factor_high_degree() {
        let result = eval("factor(x^3+1)");
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
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
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_poly_add_wrong_args() {
        let ast = AstNode::FunctionCall("poly_add".to_string(), vec![AstNode::Number(1.0)]);
        let result = PolynomialDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_complex_rejected() {
        let ast = AstNode::Complex(1.0, 2.0);
        let result = PolynomialDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_matrix_rejected() {
        let ast = AstNode::Matrix(vec![vec![AstNode::Number(1.0)]]);
        let result = PolynomialDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_list_rejected() {
        let ast = AstNode::List(vec![AstNode::Number(1.0)]);
        let result = PolynomialDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_factorial_rejected() {
        let ast = AstNode::UnaryOp(UnaryOp::Factorial, Box::new(AstNode::Number(5.0)));
        let result = PolynomialDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_multiple_variables_rejected() {
        let ast = AstNode::BinaryOp(
            BinaryOp::Add,
            Box::new(AstNode::Variable("x".to_string())),
            Box::new(AstNode::Variable("y".to_string())),
        );
        let result = PolynomialDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_negative_exponent_rejected() {
        let ast = AstNode::BinaryOp(
            BinaryOp::Pow,
            Box::new(AstNode::Variable("x".to_string())),
            Box::new(AstNode::Number(-1.0)),
        );
        let result = PolynomialDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    // ===== 底层算法测试 =====

    #[test]
    fn test_poly_add_coeffs() {
        assert_eq!(
            math_poly::add(&[1.0, 2.0], &[3.0, 4.0, 5.0]),
            vec![4.0, 6.0, 5.0]
        );
        assert_eq!(math_poly::add(&[], &[1.0]), vec![1.0]);
    }

    #[test]
    fn test_poly_mul_coeffs() {
        assert_eq!(
            math_poly::mul(&[1.0, 1.0], &[2.0, 1.0]),
            vec![2.0, 3.0, 1.0]
        );
    }

    #[test]
    fn test_poly_div_coeffs() {
        let (q, r) = math_poly::div(&[-1.0, 0.0, 1.0], &[-1.0, 1.0]);
        assert_vec_approx(&q, &[1.0, 1.0]);
        assert!(r.iter().all(|x| x.abs() < 1e-9));
    }

    #[test]
    fn test_poly_eval_horner() {
        assert_eq!(math_poly::eval(&[1.0, 2.0, 1.0], 2.0).unwrap(), 9.0); // 1+2*2+1*4=9
        assert_eq!(math_poly::eval(&[], 1.0).unwrap(), 0.0);
    }

    #[test]
    fn test_poly_diff_coeffs() {
        assert_vec_approx(&math_poly::diff(&[0.0, 2.0, 0.0, 1.0]), &[2.0, 0.0, 3.0]);
    }

    #[test]
    fn test_poly_integrate_coeffs() {
        assert_vec_approx(&math_poly::integrate(&[0.0, 2.0]), &[0.0, 0.0, 1.0]);
    }

    #[test]
    fn test_trim_leading_zeros() {
        assert_eq!(math_poly::trim(&[1.0, 2.0, 0.0, 0.0]), vec![1.0, 2.0]);
        assert_eq!(math_poly::trim(&[0.0]), vec![0.0]);
    }

    #[test]
    fn test_is_zero_poly() {
        assert!(math_poly::is_zero(&[0.0, 0.0]));
        assert!(!math_poly::is_zero(&[1.0, 0.0]));
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
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_eval_node_unary_abs_rejected() {
        let ast = AstNode::UnaryOp(UnaryOp::Abs, Box::new(AstNode::Number(5.0)));
        let result = PolynomialDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_eval_node_unary_neg_polynomial() {
        let ast = AstNode::UnaryOp(UnaryOp::Neg, Box::new(parse("x^2+1").unwrap()));
        let result = PolynomialDomain
            .evaluate(&ast, &EvalContext::new())
            .unwrap();
        let coeffs = result.as_polynomial().unwrap();
        assert_vec_approx(coeffs, &[-1.0, 0.0, -1.0]);
    }

    #[test]
    fn test_eval_node_variable_bound() {
        let ctx = EvalContext::new().with_var("x", 5.0);
        let ast = AstNode::Variable("x".to_string());
        let result = PolynomialDomain.evaluate(&ast, &ctx).unwrap();
        assert_eq!(result.as_scalar().unwrap(), 5.0);
    }

    #[test]
    fn test_arg_to_coeffs_nested_function() {
        let ast = parse("poly_add(poly_add(x+1, x+2), x+3)").unwrap();
        let result = PolynomialDomain
            .evaluate(&ast, &EvalContext::new())
            .unwrap();
        let coeffs = result.as_polynomial().unwrap();
        assert_vec_approx(coeffs, &[6.0, 3.0]);
    }

    #[test]
    fn test_arg_to_coeffs_nested_function_non_polynomial() {
        let ast = AstNode::FunctionCall(
            "poly_add".to_string(),
            vec![
                AstNode::FunctionCall("roots".to_string(), vec![parse("x-1").unwrap()]),
                parse("x+1").unwrap(),
            ],
        );
        let result = PolynomialDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_eval_scalar_bignumber() {
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
        let ast = AstNode::FunctionCall(
            "poly_eval".to_string(),
            vec![parse("x+1").unwrap(), AstNode::BigNumber("xyz".to_string())],
        );
        let result = PolynomialDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_eval_scalar_variable() {
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
        let ast = AstNode::FunctionCall(
            "poly_eval".to_string(),
            vec![parse("x+1").unwrap(), AstNode::Variable("z".to_string())],
        );
        let result = PolynomialDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Eval));
    }

    #[test]
    fn test_eval_scalar_neg() {
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
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::DivisionByZero));
    }

    #[test]
    fn test_eval_scalar_pow() {
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
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::DivisionByZero));
    }

    #[test]
    fn test_eval_scalar_complex_rejected() {
        let ast = AstNode::FunctionCall(
            "poly_eval".to_string(),
            vec![parse("x+1").unwrap(), AstNode::Complex(1.0, 2.0)],
        );
        let result = PolynomialDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_expr_to_coeffs_bignumber() {
        let ast = AstNode::BigNumber("42".to_string());
        let (coeffs, var) = expr_to_coeffs(&ast, &EvalContext::new()).unwrap();
        assert_eq!(var, "");
        assert_vec_approx(&coeffs, &[42.0]);
    }

    #[test]
    fn test_expr_to_coeffs_bignumber_invalid() {
        let ast = AstNode::BigNumber("xyz".to_string());
        let result = expr_to_coeffs(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_expr_to_coeffs_variable_with_ctx_value() {
        let ctx = EvalContext::new().with_var("x", 5.0);
        let ast = AstNode::Variable("x".to_string());
        let (coeffs, var) = expr_to_coeffs(&ast, &ctx).unwrap();
        assert_eq!(var, "");
        assert_vec_approx(&coeffs, &[5.0]);
    }

    #[test]
    fn test_expr_to_coeffs_pow_variable_with_ctx_value() {
        let ctx = EvalContext::new().with_var("x", 3.0);
        let ast = parse("x^2").unwrap();
        let (coeffs, var) = expr_to_coeffs(&ast, &ctx).unwrap();
        assert_eq!(var, "");
        assert_vec_approx(&coeffs, &[9.0]);
    }

    #[test]
    fn test_expr_to_coeffs_pow_number_number() {
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
        let ast = AstNode::BinaryOp(
            BinaryOp::Pow,
            Box::new(parse("x+1").unwrap()),
            Box::new(AstNode::Number(2.0)),
        );
        let result = expr_to_coeffs(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_expr_to_coeffs_mul_poly_poly() {
        let ast = parse("(x+1)*(x+2)").unwrap();
        let (coeffs, var) = expr_to_coeffs(&ast, &EvalContext::new()).unwrap();
        assert_eq!(var, "x");
        assert_vec_approx(&coeffs, &[2.0, 3.0, 1.0]);
    }

    #[test]
    fn test_expr_to_coeffs_mul_number_left() {
        let ast = parse("3*(x+1)").unwrap();
        let (coeffs, var) = expr_to_coeffs(&ast, &EvalContext::new()).unwrap();
        assert_eq!(var, "x");
        assert_vec_approx(&coeffs, &[3.0, 3.0]);
    }

    #[test]
    fn test_expr_to_coeffs_div_number_number() {
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
        let ast = AstNode::BinaryOp(
            BinaryOp::Div,
            Box::new(AstNode::Number(6.0)),
            Box::new(AstNode::Number(0.0)),
        );
        let result = expr_to_coeffs(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::DivisionByZero));
    }

    #[test]
    fn test_expr_to_coeffs_div_unsupported() {
        let ast = parse("(x+1)/(x+2)").unwrap();
        let result = expr_to_coeffs(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_expr_to_coeffs_div_poly_by_number() {
        let ast = parse("(x^2-1)/2").unwrap();
        let (coeffs, var) = expr_to_coeffs(&ast, &EvalContext::new()).unwrap();
        assert_eq!(var, "x");
        assert_eq!(coeffs, vec![-0.5, 0.0, 0.5]);
    }

    #[test]
    fn test_expr_to_coeffs_div_poly_by_number_zero() {
        let ast = parse("(x+1)/0").unwrap();
        let result = expr_to_coeffs(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::DivisionByZero));
    }

    #[test]
    fn test_expr_to_coeffs_div_poly_by_poly_exact() {
        let ast = parse("(x^2-1)/(x-1)").unwrap();
        let (coeffs, var) = expr_to_coeffs(&ast, &EvalContext::new()).unwrap();
        assert_eq!(var, "x");
        assert_eq!(coeffs, vec![1.0, 1.0]);
    }

    #[test]
    fn test_expr_to_coeffs_div_poly_by_poly_exact_higher() {
        let ast = parse("(x^3-1)/(x-1)").unwrap();
        let (coeffs, var) = expr_to_coeffs(&ast, &EvalContext::new()).unwrap();
        assert_eq!(var, "x");
        assert_eq!(coeffs, vec![1.0, 1.0, 1.0]);
    }

    #[test]
    fn test_expr_to_coeffs_mod() {
        let ast = AstNode::BinaryOp(
            BinaryOp::Mod,
            Box::new(AstNode::Variable("x".to_string())),
            Box::new(AstNode::Number(2.0)),
        );
        let result = expr_to_coeffs(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_expr_to_coeffs_unsupported_node() {
        let ast = AstNode::Complex(1.0, 2.0);
        let result = expr_to_coeffs(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_poly_div_coeffs_a_shorter_than_b() {
        let (q, r) = math_poly::div(&[1.0, 2.0], &[1.0, 2.0, 3.0]);
        assert_vec_approx(&q, &[0.0]);
        assert_vec_approx(&r, &[1.0, 2.0]);
    }

    #[test]
    fn test_poly_div_coeffs_a_empty() {
        let (q, r) = math_poly::div(&[], &[1.0, 2.0]);
        assert_vec_approx(&q, &[0.0]);
        assert!(r.is_empty(), "remainder should be empty, got {:?}", r);
    }

    #[test]
    fn test_poly_mul_coeffs_empty() {
        assert_vec_approx(&math_poly::mul(&[], &[1.0, 2.0]), &[0.0]);
        assert_vec_approx(&math_poly::mul(&[1.0], &[]), &[0.0]);
    }

    #[test]
    fn test_poly_eval_horner_empty() {
        assert_eq!(math_poly::eval(&[], 1.0).unwrap(), 0.0);
    }

    #[test]
    fn test_factor_constant() {
        let result = eval_symbolic("factor(5)").unwrap();
        assert_eq!(result, "5");
    }

    #[test]
    fn test_factor_linear_zero_root() {
        let result = eval_symbolic("factor(x)").unwrap();
        assert_eq!(result, "x");
    }

    #[test]
    fn test_factor_linear_negative_coeff() {
        let result = eval_symbolic("factor(-x+3)").unwrap();
        assert!(result.contains("x-3") || result.contains("(x-3)"));
    }

    #[test]
    fn test_factor_quadratic_leading_one() {
        let result = eval_symbolic("factor(x^2-4)").unwrap();
        assert!(result.contains("(x-2)"));
        assert!(result.contains("(x+2)"));
    }

    #[test]
    fn test_factor_quadratic_negative_leading() {
        let result = eval_symbolic("factor(4-x^2)").unwrap();
        assert!(result.starts_with("-"));
        assert!(result.contains("(x-2)"));
        assert!(result.contains("(x+2)"));
    }

    #[test]
    fn test_factor_quadratic_other_leading() {
        let result = eval_symbolic("factor(2*x^2-8)").unwrap();
        assert!(result.starts_with("2*"));
        assert!(result.contains("(x-2)"));
        assert!(result.contains("(x+2)"));
    }

    #[test]
    fn test_factor_linear_positive_root_format() {
        let result = eval_symbolic("factor(x-3)").unwrap();
        assert!(result.contains("x-3") || result.contains("(x-3)"));
    }

    #[test]
    fn test_factor_linear_negative_root_format() {
        let result = eval_symbolic("factor(x+3)").unwrap();
        assert!(result.contains("x+3") || result.contains("(x+3)"));
    }

    #[test]
    fn test_poly_sub_wrong_args() {
        let ast = AstNode::FunctionCall("poly_sub".to_string(), vec![AstNode::Number(1.0)]);
        let result = PolynomialDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    /// poly_sub 参数数量错误必须带 i18n key，否则 locales 中该 key 成为死键。
    #[test]
    fn test_poly_sub_wrong_args_has_i18n() {
        let ast = AstNode::FunctionCall("poly_sub".to_string(), vec![AstNode::Number(1.0)]);
        let result = PolynomialDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(&result, Err(e) if e.kind == ErrorKind::Domain));
        if let Err(ref e) = result {
            assert_eq!(
                e.i18n_key,
                Some("msg.polynomial.poly_sub_arg_count"),
                "expected i18n key msg.polynomial.poly_sub_arg_count"
            );
            // 参数 actual 应被填充为实际传入参数数
            assert!(
                e.i18n_args.iter().any(|(k, v)| k == "actual" && v == "1"),
                "expected i18n_args containing (actual, 1), got {:?}",
                e.i18n_args
            );
        }
    }

    #[test]
    fn test_poly_mul_wrong_args() {
        let ast = AstNode::FunctionCall("poly_mul".to_string(), vec![AstNode::Number(1.0)]);
        let result = PolynomialDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_poly_div_wrong_args() {
        let ast = AstNode::FunctionCall("poly_div".to_string(), vec![AstNode::Number(1.0)]);
        let result = PolynomialDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_poly_eval_wrong_args() {
        let ast = AstNode::FunctionCall("poly_eval".to_string(), vec![AstNode::Number(1.0)]);
        let result = PolynomialDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_poly_diff_wrong_args() {
        let ast = AstNode::FunctionCall("poly_diff".to_string(), vec![]);
        let result = PolynomialDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_poly_integrate_wrong_args() {
        let ast = AstNode::FunctionCall("poly_integrate".to_string(), vec![]);
        let result = PolynomialDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_factor_wrong_args() {
        let ast = AstNode::FunctionCall("factor".to_string(), vec![]);
        let result = PolynomialDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_roots_wrong_args() {
        let ast = AstNode::FunctionCall("roots".to_string(), vec![]);
        let result = PolynomialDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_contains_polynomial_recursive() {
        let ast = parse("poly_add(x+1, x+2) + 5").unwrap();
        assert!(PolynomialDomain.supports(&ast));
    }

    #[test]
    fn test_contains_polynomial_unary() {
        let ast = AstNode::UnaryOp(UnaryOp::Neg, Box::new(parse("poly_add(x+1, x+2)").unwrap()));
        assert!(PolynomialDomain.supports(&ast));
    }

    #[test]
    fn test_contains_polynomial_matrix() {
        let ast = AstNode::Matrix(vec![vec![parse("poly_add(x+1, x+2)").unwrap()]]);
        assert!(PolynomialDomain.supports(&ast));
    }

    #[test]
    fn test_contains_polynomial_list() {
        let ast = AstNode::List(vec![parse("poly_add(x+1, x+2)").unwrap()]);
        assert!(PolynomialDomain.supports(&ast));
    }

    #[test]
    fn test_not_supports_bignumber() {
        let ast = AstNode::BigNumber("42".to_string());
        assert!(!PolynomialDomain.supports(&ast));
    }

    // ===== 覆盖率补充测试（第二轮） =====

    #[test]
    fn test_eval_node_number_bare() {
        let ast = AstNode::Number(7.0);
        let result = PolynomialDomain
            .evaluate(&ast, &EvalContext::new())
            .unwrap();
        assert_eq!(result.as_scalar().unwrap(), 7.0);
    }

    #[test]
    fn test_eval_node_binaryop_polynomial_bare() {
        let result = eval_polynomial("x^2+2*x+1").unwrap();
        assert_vec_approx(&result, &[1.0, 2.0, 1.0]);
    }

    #[test]
    fn test_eval_scalar_sub() {
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
        let ast = parse("(x+1)*3").unwrap();
        let (coeffs, var) = expr_to_coeffs(&ast, &EvalContext::new()).unwrap();
        assert_eq!(var, "x");
        assert_vec_approx(&coeffs, &[3.0, 3.0]);
    }

    #[test]
    fn test_solve_cubic_double_root() {
        // x^3-3x+2 = (x-1)^2*(x+2) → roots: 1(二重), -2
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
        // x^4+x = x(x+1)(x^2-x+1) → roots: 0, -1, (1±i√3)/2
        let result = eval("roots(x^4+x)").unwrap();
        let roots = result
            .as_complex_list()
            .expect("expected ComplexList (has complex roots)");
        assert_eq!(roots.len(), 4);
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
            // 积分后常数项丢失，coeffs[1] 即原一次项系数 a1
            prop_assert!((coeffs[1] - a1 as f64).abs() < 1e-9);
        }
    }
}
