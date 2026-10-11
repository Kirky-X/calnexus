// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! LaTeX 输出格式化器。
//!
//! 将 `EvalResult` 各变体（标量/复数/矩阵/向量/多项式/符号运算等）渲染为 LaTeX 字符串。

use crate::core::{AstNode, EvalResult};
use num_traits::Signed;

/// 格式化标量为 LaTeX：NaN/Inf 用 `\text{NaN}`/`\infty`，整数避免科学计数法，小数抑制浮点噪声。
pub fn format_latex_scalar(v: f64) -> String {
    if v.is_nan() {
        return "\\text{NaN}".to_string();
    }
    if v.is_infinite() {
        return if v > 0.0 {
            "\\infty".to_string()
        } else {
            "-\\infty".to_string()
        };
    }
    if v.fract() == 0.0 {
        if v.abs() < 9e15 {
            return format!("{}", v as i64);
        }
        // 超出 i64 安全范围的大整数：{:.0} 避免科学计数法
        return format!("{:.0}", v);
    }
    let default = format!("{}", v);
    if has_floating_point_noise(&default) {
        let rounded = format!("{:.10}", v);
        let trimmed = rounded.trim_end_matches('0').trim_end_matches('.');
        if !trimmed.is_empty() && trimmed != "-" {
            return trimmed.to_string();
        }
    }
    default
}

/// 检测浮点精度噪声：连续 7 个 0 或 9 表示二进制浮点误差。
fn has_floating_point_noise(s: &str) -> bool {
    s.contains("0000000") || s.contains("9999999")
}

/// 格式化复数为 LaTeX：虚部为 0 只留实部，实部为 0 只留虚部，±1 虚部省略系数。
pub fn format_latex_complex(re: f64, im: f64) -> String {
    if im == 0.0 {
        return format_latex_scalar(re);
    }
    if re == 0.0 {
        if im == 1.0 {
            return "i".to_string();
        }
        if im == -1.0 {
            return "-i".to_string();
        }
        return format!("{}i", format_latex_scalar(im));
    }
    let re_s = format_latex_scalar(re);
    if im == 1.0 {
        format!("{} + i", re_s)
    } else if im == -1.0 {
        format!("{} - i", re_s)
    } else if im > 0.0 {
        format!("{} + {}i", re_s, format_latex_scalar(im))
    } else {
        format!("{} - {}i", re_s, format_latex_scalar(-im))
    }
}

/// 格式化矩阵为 LaTeX `\begin{pmatrix}...\end{pmatrix}`，行内以 ` & `、行间以 ` \\ ` 分隔。
pub fn format_latex_matrix(m: &[Vec<f64>]) -> String {
    let rows: Vec<String> = m
        .iter()
        .map(|row| {
            let elems: Vec<String> = row.iter().map(|v| format_latex_scalar(*v)).collect();
            elems.join(" & ")
        })
        .collect();
    format!("\\begin{{pmatrix}}{}\\end{{pmatrix}}", rows.join(" \\\\ "))
}

/// 格式化向量为 LaTeX `\left[a, b, c\right]`。
pub fn format_latex_vector(v: &[f64]) -> String {
    let elems: Vec<String> = v.iter().map(|x| format_latex_scalar(*x)).collect();
    format!("\\left[{}\\right]", elems.join(", "))
}

/// 格式化多项式系数向量（升幂存储）为 LaTeX 降幂字符串 `a x^{2} + b x + c`。
pub fn format_latex_polynomial(p: &[f64]) -> String {
    if p.is_empty() {
        return "0".to_string();
    }
    let terms: Vec<String> = p
        .iter()
        .enumerate()
        .rev()
        .filter_map(|(i, &coef)| format_latex_polynomial_term(coef, i))
        .collect();
    if terms.is_empty() {
        return "0".to_string();
    }
    join_latex_polynomial_terms(&terms)
}

/// 格式化 LaTeX 多项式单项：零系数返回 None（跳过），其他返回 `Some(term)`。
fn format_latex_polynomial_term(coef: f64, i: usize) -> Option<String> {
    if coef == 0.0 {
        return None;
    }
    let term = match i {
        0 => format_latex_scalar(coef),
        1 => {
            if coef == 1.0 {
                "x".to_string()
            } else if coef == -1.0 {
                "-x".to_string()
            } else {
                format!("{}x", format_latex_scalar(coef))
            }
        }
        _ => {
            if coef == 1.0 {
                format!("x^{{{}}}", i)
            } else if coef == -1.0 {
                format!("-x^{{{}}}", i)
            } else {
                format!("{}x^{{{}}}", format_latex_scalar(coef), i)
            }
        }
    };
    Some(term)
}

/// 合并多项式项：首项原样输出，正项用 ` + `、负项用 ` - ` 分隔（去掉前导 `-`）。
fn join_latex_polynomial_terms(terms: &[String]) -> String {
    let mut result = terms[0].clone();
    for term in &terms[1..] {
        if let Some(stripped) = term.strip_prefix('-') {
            result.push_str(" - ");
            result.push_str(stripped);
        } else {
            result.push_str(" + ");
            result.push_str(term);
        }
    }
    result
}

/// 格式化 BigInt 为 LaTeX 数字字符串（直接 `to_string()`，无需特殊转义）。
pub fn format_latex_bigint(b: &num_bigint::BigInt) -> String {
    b.to_string()
}

/// 格式化 BigRational：`fmt_prec` 为 `Some(n)` 时输出 n 位小数，否则输出 `\frac{p}{q}`。
pub fn format_latex_bigrational(r: &num_rational::BigRational, fmt_prec: Option<usize>) -> String {
    if let Some(prec) = fmt_prec {
        let neg = r.is_negative();
        let abs = r.abs();
        let numer = abs.numer();
        let denom = abs.denom();
        // 整数部分
        let int_part = numer / denom;
        let remainder = numer % denom;
        // 小数部分：remainder * 10^prec / denom
        let mut scale = num_bigint::BigInt::from(1);
        let ten = num_bigint::BigInt::from(10);
        for _ in 0..prec {
            scale *= &ten;
        }
        let scaled = remainder * &scale;
        let frac_digits = scaled / denom;
        let int_str = int_part.to_string();
        let frac_str = format!("{:0>width$}", frac_digits.to_string(), width = prec);
        let sign = if neg { "-" } else { "" };
        if prec == 0 {
            format!("{}{}", sign, int_str)
        } else {
            format!("{}{}.{}", sign, int_str, frac_str)
        }
    } else {
        format!("\\frac{{{}}}{{{}}}", r.numer(), r.denom())
    }
}

/// 格式化复根列表为 LaTeX `\left[a + bi, c + di\right]`。
pub fn format_latex_complex_list(c: &[(f64, f64)]) -> String {
    let elems: Vec<String> = c
        .iter()
        .map(|(re, im)| format_latex_complex(*re, *im))
        .collect();
    format!("\\left[{}\\right]", elems.join(", "))
}

/// 符号运算结果转 LaTeX：按 op_name 选择包装，乘号转 `\cdot`。
pub fn format_latex_symbolic(op_name: &str, ast: &AstNode, result_str: &str) -> String {
    let result_latex = symbolic_str_to_latex(result_str);
    let args = match ast {
        AstNode::FunctionCall(_, args) => args,
        _ => return result_latex,
    };
    match op_name {
        "diff" => {
            let var = args
                .get(1)
                .and_then(ast_to_latex_expr)
                .unwrap_or_else(|| "x".to_string());
            let body = args
                .first()
                .and_then(ast_to_latex_expr)
                .unwrap_or_else(|| result_latex.clone());
            format!(
                "\\frac{{d}}{{d{var}}}\\left({body}\\right) = {result_latex}",
                var = var,
                body = body,
                result_latex = result_latex
            )
        }
        "integrate" => {
            let var = args
                .get(1)
                .and_then(ast_to_latex_expr)
                .unwrap_or_else(|| "x".to_string());
            let body = args
                .first()
                .and_then(ast_to_latex_expr)
                .unwrap_or_else(|| "".to_string());
            format!(
                "\\int {body} \\, d{var} = {result_latex}",
                body = body,
                var = var,
                result_latex = result_latex
            )
        }
        "limit" => {
            let var = args
                .get(1)
                .and_then(ast_to_latex_expr)
                .unwrap_or_else(|| "x".to_string());
            let point = args
                .get(2)
                .and_then(ast_to_latex_expr)
                .unwrap_or_else(|| "0".to_string());
            let body = args
                .first()
                .and_then(ast_to_latex_expr)
                .unwrap_or_else(|| "".to_string());
            format!(
                "\\lim_{{{var} \\to {point}}} {body} = {result_latex}",
                var = var,
                point = point,
                body = body,
                result_latex = result_latex
            )
        }
        "series" | "taylor" => {
            let body = args
                .first()
                .and_then(ast_to_latex_expr)
                .unwrap_or_else(|| "".to_string());
            format!(
                "\\text{{Taylor series of}} {body} = {result_latex}",
                body = body,
                result_latex = result_latex
            )
        }
        _ => result_latex,
    }
}

/// 将 `AstNode` 表达式转换为 LaTeX 字符串（仅支持常见运算符）。
fn ast_to_latex_expr(node: &AstNode) -> Option<String> {
    match node {
        AstNode::Number(n) => Some(format_latex_scalar(*n)),
        AstNode::BigNumber(s) => Some(s.clone()),
        AstNode::Variable(name) => Some(name.clone()),
        AstNode::BinaryOp(op, lhs, rhs) => {
            let l = ast_to_latex_expr(lhs)?;
            let r = ast_to_latex_expr(rhs)?;
            let op_str = match op {
                crate::core::BinaryOp::Add => format!("{} + {}", l, r),
                crate::core::BinaryOp::Sub => format!("{} - {}", l, r),
                crate::core::BinaryOp::Mul => format!("{} \\cdot {}", l, r),
                crate::core::BinaryOp::Div => format!("\\frac{{{}}}{{{}}}", l, r),
                crate::core::BinaryOp::Pow => format!("{}^{{{}}}", l, r),
                crate::core::BinaryOp::Mod => format!("{} \\bmod {}", l, r),
            };
            Some(op_str)
        }
        AstNode::UnaryOp(op, expr) => {
            let e = ast_to_latex_expr(expr)?;
            Some(match op {
                crate::core::UnaryOp::Neg => format!("-{}", e),
                crate::core::UnaryOp::Factorial => format!("{}!", e),
                crate::core::UnaryOp::Abs => format!("\\left|{}\\right|", e),
            })
        }
        AstNode::FunctionCall(name, args) => {
            let args_latex: Vec<String> = args.iter().filter_map(ast_to_latex_expr).collect();
            Some(format!("{}({})", name, args_latex.join(", ")))
        }
        _ => None,
    }
}

/// 符号结果串转 LaTeX：`*` → ` \cdot `，`^N` → `^{N}`（指数可带前导负号），其余原样。
fn symbolic_str_to_latex(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut result = String::with_capacity(s.len() * 2);
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '*' => {
                result.push_str(" \\cdot ");
                i += 1;
            }
            '^' => {
                result.push_str("^{");
                i += 1;
                // 捕获可选的负指数符号
                if i < chars.len() && chars[i] == '-' {
                    result.push('-');
                    i += 1;
                }
                // 捕获指数数字序列
                while i < chars.len() && chars[i].is_ascii_digit() {
                    result.push(chars[i]);
                    i += 1;
                }
                result.push('}');
            }
            _ => {
                result.push(c);
                i += 1;
            }
        }
    }
    result
}

/// 顶层格式化器：按 `EvalResult` 变体分发；`ast` 供符号包装，`fmt_prec` 供 BigRational 位数。
pub fn format_latex(
    result: &EvalResult,
    ast: &AstNode,
    _expr: &str,
    fmt_prec: Option<usize>,
) -> String {
    match result {
        EvalResult::Scalar(v) => format_latex_scalar(*v),
        EvalResult::Complex(re, im) => format_latex_complex(*re, *im),
        EvalResult::Matrix(m) => format_latex_matrix(m),
        EvalResult::BigInt(b) => format_latex_bigint(b),
        EvalResult::BigRational(r) => format_latex_bigrational(r, fmt_prec),
        EvalResult::Vector(v) => format_latex_vector(v),
        EvalResult::Polynomial(p) => format_latex_polynomial(p),
        EvalResult::ComplexList(c) => format_latex_complex_list(c),
        EvalResult::Symbolic(s) => {
            // 符号运算：根据 AST 顶层 FunctionCall 名选择包装
            if let AstNode::FunctionCall(name, _) = ast
                && matches!(
                    name.as_str(),
                    "diff" | "integrate" | "limit" | "series" | "taylor"
                )
            {
                return format_latex_symbolic(name, ast, s);
            }
            symbolic_str_to_latex(s)
        }
        EvalResult::LaTeX(s) => s.clone(),
        EvalResult::Steps(v) => v.join(" \\to "),
        // Json 复合结果（lu/qr/eig/svd 分解）无标准 LaTeX 形式，直接输出 JSON 字符串
        EvalResult::Json(v) => v.to_string(),
        // DateTime：RFC3339 串用 \text{} 包装，避免 `-`/`:` 被解释为减法和除法
        EvalResult::DateTime(s) => format!("\\text{{{}}}", s),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{BinaryOp, UnaryOp};

    #[test]
    fn latex_scalar_integer() {
        assert_eq!(format_latex_scalar(42.0), "42");
        assert_eq!(format_latex_scalar(-7.0), "-7");
    }

    #[test]
    fn latex_scalar_decimal() {
        assert_eq!(format_latex_scalar(3.14), "3.14");
    }

    #[test]
    fn latex_scalar_nan_inf() {
        assert_eq!(format_latex_scalar(f64::NAN), "\\text{NaN}");
        assert_eq!(format_latex_scalar(f64::INFINITY), "\\infty");
        assert_eq!(format_latex_scalar(f64::NEG_INFINITY), "-\\infty");
    }

    #[test]
    fn latex_complex_positive_im() {
        assert_eq!(format_latex_complex(3.0, 4.0), "3 + 4i");
    }

    #[test]
    fn latex_complex_negative_im() {
        assert_eq!(format_latex_complex(3.0, -4.0), "3 - 4i");
    }

    #[test]
    fn latex_matrix_2x2() {
        let m = vec![vec![1.0, 2.0], vec![3.0, 4.0]];
        assert_eq!(
            format_latex_matrix(&m),
            "\\begin{pmatrix}1 & 2 \\\\ 3 & 4\\end{pmatrix}"
        );
    }

    #[test]
    fn latex_vector() {
        assert_eq!(
            format_latex_vector(&[1.0, 2.0, 3.0]),
            "\\left[1, 2, 3\\right]"
        );
    }

    #[test]
    fn latex_polynomial_simple() {
        assert_eq!(format_latex_polynomial(&[1.0, 2.0, 1.0]), "x^{2} + 2x + 1");
    }

    #[test]
    fn latex_polynomial_negative_coef() {
        // [1, -1] → -x + 1
        assert_eq!(format_latex_polynomial(&[1.0, -1.0]), "-x + 1");
    }

    #[test]
    fn latex_polynomial_zero() {
        assert_eq!(format_latex_polynomial(&[]), "0");
        assert_eq!(format_latex_polynomial(&[0.0, 0.0]), "0");
    }

    #[test]
    fn latex_bigint() {
        let b = num_bigint::BigInt::from(123456789);
        assert_eq!(format_latex_bigint(&b), "123456789");
    }

    #[test]
    fn latex_bigrational_fraction() {
        let r = num_rational::BigRational::new(
            num_bigint::BigInt::from(1),
            num_bigint::BigInt::from(3),
        );
        assert_eq!(format_latex_bigrational(&r, None), "\\frac{1}{3}");
    }

    #[test]
    fn latex_bigrational_decimal() {
        let r = num_rational::BigRational::new(
            num_bigint::BigInt::from(1),
            num_bigint::BigInt::from(2),
        );
        assert_eq!(format_latex_bigrational(&r, Some(3)), "0.500");
    }

    #[test]
    fn latex_complex_list() {
        let c = vec![(1.0, 2.0), (3.0, -4.0)];
        assert_eq!(
            format_latex_complex_list(&c),
            "\\left[1 + 2i, 3 - 4i\\right]"
        );
    }

    #[test]
    fn latex_symbolic_diff() {
        let ast = AstNode::FunctionCall(
            "diff".to_string(),
            vec![
                AstNode::BinaryOp(
                    BinaryOp::Pow,
                    Box::new(AstNode::Variable("x".to_string())),
                    Box::new(AstNode::Number(2.0)),
                ),
                AstNode::Variable("x".to_string()),
            ],
        );
        let s = format_latex_symbolic("diff", &ast, "2*x");
        assert_eq!(s, "\\frac{d}{dx}\\left(x^{2}\\right) = 2 \\cdot x");
    }

    #[test]
    fn latex_symbolic_integrate() {
        let ast = AstNode::FunctionCall(
            "integrate".to_string(),
            vec![
                AstNode::Variable("x".to_string()),
                AstNode::Variable("x".to_string()),
            ],
        );
        let s = format_latex_symbolic("integrate", &ast, "x^2/2");
        assert!(s.contains("\\int"));
        assert!(s.contains("\\, dx"));
    }

    #[test]
    fn latex_symbolic_limit() {
        let ast = AstNode::FunctionCall(
            "limit".to_string(),
            vec![
                AstNode::Variable("x".to_string()),
                AstNode::Variable("x".to_string()),
                AstNode::Number(0.0),
            ],
        );
        let s = format_latex_symbolic("limit", &ast, "1");
        assert!(s.contains("\\lim_{x \\to 0}"));
    }

    #[test]
    fn latex_top_level_scalar() {
        let r = EvalResult::Scalar(5.0);
        let ast = AstNode::Number(5.0);
        assert_eq!(format_latex(&r, &ast, "2+3", None), "5");
    }

    #[test]
    fn latex_top_level_matrix() {
        let r = EvalResult::Matrix(vec![vec![1.0, 2.0], vec![3.0, 4.0]]);
        let ast = AstNode::Number(0.0); // 占位
        assert_eq!(
            format_latex(&r, &ast, "[[1,2],[3,4]]", None),
            "\\begin{pmatrix}1 & 2 \\\\ 3 & 4\\end{pmatrix}"
        );
    }

    #[test]
    fn latex_top_level_symbolic_diff() {
        let r = EvalResult::Symbolic("2*x".to_string());
        let ast = AstNode::FunctionCall(
            "diff".to_string(),
            vec![
                AstNode::BinaryOp(
                    BinaryOp::Pow,
                    Box::new(AstNode::Variable("x".to_string())),
                    Box::new(AstNode::Number(2.0)),
                ),
                AstNode::Variable("x".to_string()),
            ],
        );
        let s = format_latex(&r, &ast, "diff(x^2,x)", None);
        assert_eq!(s, "\\frac{d}{dx}\\left(x^{2}\\right) = 2 \\cdot x");
    }

    #[test]
    fn latex_top_level_latex_passthrough() {
        let r = EvalResult::LaTeX("\\alpha".to_string());
        let ast = AstNode::Number(0.0);
        assert_eq!(format_latex(&r, &ast, "", None), "\\alpha");
    }

    #[test]
    fn latex_top_level_steps_join() {
        let r = EvalResult::Steps(vec!["2+9=11".to_string(), "11*7=77".to_string()]);
        let ast = AstNode::Number(0.0);
        assert_eq!(format_latex(&r, &ast, "", None), "2+9=11 \\to 11*7=77");
    }

    #[test]
    fn latex_symbolic_str_to_latex_multiplies() {
        assert_eq!(symbolic_str_to_latex("2*x*y"), "2 \\cdot x \\cdot y");
    }

    #[test]
    fn latex_ast_to_latex_expr_pow() {
        let node = AstNode::BinaryOp(
            BinaryOp::Pow,
            Box::new(AstNode::Variable("x".to_string())),
            Box::new(AstNode::Number(2.0)),
        );
        assert_eq!(ast_to_latex_expr(&node), Some("x^{2}".to_string()));
    }

    #[test]
    fn latex_ast_to_latex_expr_div() {
        let node = AstNode::BinaryOp(
            BinaryOp::Div,
            Box::new(AstNode::Number(1.0)),
            Box::new(AstNode::Number(2.0)),
        );
        assert_eq!(ast_to_latex_expr(&node), Some("\\frac{1}{2}".to_string()));
    }

    #[test]
    fn latex_ast_to_latex_expr_unary_neg() {
        let node = AstNode::UnaryOp(UnaryOp::Neg, Box::new(AstNode::Variable("x".to_string())));
        assert_eq!(ast_to_latex_expr(&node), Some("-x".to_string()));
    }

    #[test]
    fn latex_ast_to_latex_expr_factorial() {
        let node = AstNode::UnaryOp(UnaryOp::Factorial, Box::new(AstNode::Number(5.0)));
        assert_eq!(ast_to_latex_expr(&node), Some("5!".to_string()));
    }

    // ===== 覆盖 format_latex_polynomial 各分支 =====

    #[test]
    fn latex_polynomial_leading_coef_one_x() {
        // i==1, coef==1.0 → "x"
        assert_eq!(format_latex_polynomial(&[5.0, 1.0]), "x + 5");
    }

    #[test]
    fn latex_polynomial_high_degree_neg_one_coef() {
        // i>1, coef==-1.0 → "-x^{i}"
        assert_eq!(format_latex_polynomial(&[0.0, 0.0, -1.0]), "-x^{2}");
    }

    #[test]
    fn latex_polynomial_high_degree_general_coef() {
        // i>1, coef 非 ±1.0 → "cx^{i}"
        assert_eq!(format_latex_polynomial(&[0.0, 0.0, 3.0]), "3x^{2}");
    }

    #[test]
    fn latex_polynomial_negative_term_concatenation() {
        // 非首项以 '-' 开头时用 " - " 分隔（与 " + " 对称）
        assert_eq!(format_latex_polynomial(&[1.0, -2.0, 1.0]), "x^{2} - 2x + 1");
    }

    // ===== 覆盖 format_latex_bigrational prec==0 分支 =====

    #[test]
    fn latex_bigrational_zero_precision() {
        // prec==0 → 仅整数部分
        let r = num_rational::BigRational::new(
            num_bigint::BigInt::from(7),
            num_bigint::BigInt::from(2),
        );
        assert_eq!(format_latex_bigrational(&r, Some(0)), "3");
    }

    // ===== 覆盖 format_latex_symbolic 非 FunctionCall 与 series/taylor/unknown =====

    #[test]
    fn latex_symbolic_non_functioncall_ast() {
        // ast 非 FunctionCall → 直接返回 result_latex
        let ast = AstNode::Number(0.0);
        let s = format_latex_symbolic("diff", &ast, "2*x");
        assert_eq!(s, "2 \\cdot x");
    }

    #[test]
    fn latex_symbolic_series_branch() {
        // series/taylor 分支
        let ast = AstNode::FunctionCall(
            "series".to_string(),
            vec![AstNode::Variable("x".to_string())],
        );
        let s = format_latex_symbolic("series", &ast, "1+x+x^2");
        assert!(s.contains("\\text{Taylor series of}"));
        assert!(s.contains("x"));
    }

    #[test]
    fn latex_symbolic_taylor_branch() {
        // taylor 分支
        let ast = AstNode::FunctionCall(
            "taylor".to_string(),
            vec![AstNode::Variable("x".to_string())],
        );
        let s = format_latex_symbolic("taylor", &ast, "1+x");
        assert!(s.contains("\\text{Taylor series of}"));
    }

    #[test]
    fn latex_symbolic_unknown_op() {
        // 未知 op_name → 返回 result_latex
        let ast = AstNode::FunctionCall(
            "unknown".to_string(),
            vec![AstNode::Variable("x".to_string())],
        );
        let s = format_latex_symbolic("unknown", &ast, "42");
        assert_eq!(s, "42");
    }

    // ===== 覆盖 ast_to_latex_expr 各分支 =====

    #[test]
    fn latex_ast_to_latex_expr_bignumber() {
        // BigNumber → 原始字符串
        let node = AstNode::BigNumber("1234567890123456".to_string());
        assert_eq!(
            ast_to_latex_expr(&node),
            Some("1234567890123456".to_string())
        );
    }

    #[test]
    fn latex_ast_to_latex_expr_add_sub_mul_mod() {
        // Add/Sub/Mul/Mod 分支
        let add = AstNode::BinaryOp(
            BinaryOp::Add,
            Box::new(AstNode::Variable("a".to_string())),
            Box::new(AstNode::Variable("b".to_string())),
        );
        assert_eq!(ast_to_latex_expr(&add), Some("a + b".to_string()));

        let sub = AstNode::BinaryOp(
            BinaryOp::Sub,
            Box::new(AstNode::Variable("a".to_string())),
            Box::new(AstNode::Variable("b".to_string())),
        );
        assert_eq!(ast_to_latex_expr(&sub), Some("a - b".to_string()));

        let mul = AstNode::BinaryOp(
            BinaryOp::Mul,
            Box::new(AstNode::Variable("a".to_string())),
            Box::new(AstNode::Variable("b".to_string())),
        );
        assert_eq!(ast_to_latex_expr(&mul), Some("a \\cdot b".to_string()));

        let modulo = AstNode::BinaryOp(
            BinaryOp::Mod,
            Box::new(AstNode::Variable("a".to_string())),
            Box::new(AstNode::Variable("b".to_string())),
        );
        assert_eq!(ast_to_latex_expr(&modulo), Some("a \\bmod b".to_string()));
    }

    #[test]
    fn latex_ast_to_latex_expr_abs() {
        // UnaryOp::Abs → \left|...\right|
        let node = AstNode::UnaryOp(UnaryOp::Abs, Box::new(AstNode::Variable("x".to_string())));
        assert_eq!(
            ast_to_latex_expr(&node),
            Some("\\left|x\\right|".to_string())
        );
    }

    #[test]
    fn latex_ast_to_latex_expr_function_call() {
        // FunctionCall → name(args)
        let node = AstNode::FunctionCall(
            "sin".to_string(),
            vec![AstNode::Number(0.0), AstNode::Variable("x".to_string())],
        );
        assert_eq!(ast_to_latex_expr(&node), Some("sin(0, x)".to_string()));
    }

    #[test]
    fn latex_ast_to_latex_expr_unsupported_returns_none() {
        // 不支持的节点类型 → None
        let node = AstNode::Complex(1.0, 2.0);
        assert_eq!(ast_to_latex_expr(&node), None);

        let node = AstNode::Matrix(vec![vec![AstNode::Number(1.0)]]);
        assert_eq!(ast_to_latex_expr(&node), None);

        let node = AstNode::List(vec![AstNode::Number(1.0)]);
        assert_eq!(ast_to_latex_expr(&node), None);
    }

    // ===== 覆盖 format_latex 顶层分发各变体 =====

    #[test]
    fn latex_top_level_complex() {
        // Complex 变体
        let r = EvalResult::Complex(3.0, 4.0);
        let ast = AstNode::Number(0.0);
        assert_eq!(format_latex(&r, &ast, "", None), "3 + 4i");
    }

    #[test]
    fn latex_top_level_bigint() {
        // BigInt 变体
        let r = EvalResult::BigInt(num_bigint::BigInt::from(42));
        let ast = AstNode::Number(0.0);
        assert_eq!(format_latex(&r, &ast, "", None), "42");
    }

    #[test]
    fn latex_top_level_bigrational() {
        // BigRational 变体
        let r = EvalResult::BigRational(num_rational::BigRational::new(
            num_bigint::BigInt::from(1),
            num_bigint::BigInt::from(3),
        ));
        let ast = AstNode::Number(0.0);
        assert_eq!(format_latex(&r, &ast, "", None), "\\frac{1}{3}");
    }

    #[test]
    fn latex_top_level_vector() {
        // Vector 变体
        let r = EvalResult::Vector(vec![1.0, 2.0, 3.0]);
        let ast = AstNode::Number(0.0);
        assert_eq!(format_latex(&r, &ast, "", None), "\\left[1, 2, 3\\right]");
    }

    #[test]
    fn latex_top_level_polynomial() {
        // Polynomial 变体
        let r = EvalResult::Polynomial(vec![1.0, 2.0, 1.0]);
        let ast = AstNode::Number(0.0);
        assert_eq!(format_latex(&r, &ast, "", None), "x^{2} + 2x + 1");
    }

    #[test]
    fn latex_top_level_complex_list() {
        // ComplexList 变体
        let r = EvalResult::ComplexList(vec![(1.0, 2.0), (3.0, -4.0)]);
        let ast = AstNode::Number(0.0);
        assert_eq!(
            format_latex(&r, &ast, "", None),
            "\\left[1 + 2i, 3 - 4i\\right]"
        );
    }

    // ===== 覆盖 Symbolic fallthrough 路径 =====

    #[test]
    fn latex_top_level_symbolic_non_matching_function() {
        // Symbolic 变体：FunctionCall 名不在包装白名单
        let r = EvalResult::Symbolic("2*x".to_string());
        let ast = AstNode::FunctionCall("foo".to_string(), vec![]);
        assert_eq!(format_latex(&r, &ast, "", None), "2 \\cdot x");
    }

    #[test]
    fn latex_top_level_symbolic_non_function() {
        // Symbolic 变体，ast 非 FunctionCall
        let r = EvalResult::Symbolic("2*x".to_string());
        let ast = AstNode::Number(0.0);
        assert_eq!(format_latex(&r, &ast, "", None), "2 \\cdot x");
    }

    // ===== format_latex_scalar 浮点精度噪声 =====

    #[test]
    fn latex_scalar_floating_point_noise() {
        // 0.1+0.2 = 0.30000000000000004，应显示为 0.3
        assert_eq!(format_latex_scalar(0.1 + 0.2), "0.3");
    }

    #[test]
    fn latex_scalar_preserves_precision_for_irreducible() {
        // 确保修复不破坏完整精度表示
        assert_eq!(format_latex_scalar(1.0 / 3.0), "0.3333333333333333");
        assert_eq!(
            format_latex_scalar(std::f64::consts::PI),
            "3.141592653589793"
        );
    }

    // ===== format_latex_scalar 大整数科学计数法 =====

    #[test]
    fn latex_scalar_large_integer_no_scientific() {
        // 大整数不应输出科学计数法
        assert_eq!(format_latex_scalar(1e15), "1000000000000000");
        assert_eq!(format_latex_scalar(1e20), "100000000000000000000");
        assert_eq!(format_latex_scalar(1e21), "1000000000000000000000");
    }

    // ===== format_latex_complex 简化 =====

    #[test]
    fn latex_complex_zero_imaginary_simplified() {
        // 虚部为 0 时应简化为实部（不输出 "+ 0i"）
        assert_eq!(format_latex_complex(3.0, 0.0), "3");
        assert_eq!(format_latex_complex(-3.0, 0.0), "-3");
    }

    #[test]
    fn latex_complex_zero_real_simplified() {
        // 实部为 0 时应简化为虚部（不输出 "0 + 4i"）
        assert_eq!(format_latex_complex(0.0, 4.0), "4i");
        assert_eq!(format_latex_complex(0.0, -4.0), "-4i");
    }

    #[test]
    fn latex_complex_both_zero_simplified() {
        // 边界：实部和虚部都为 0 → "0"
        assert_eq!(format_latex_complex(0.0, 0.0), "0");
    }

    #[test]
    fn latex_complex_one_imaginary_simplified() {
        // 边界：虚部为 ±1 时不输出系数（"i" 而非 "1i"）
        assert_eq!(format_latex_complex(2.0, 1.0), "2 + i");
        assert_eq!(format_latex_complex(2.0, -1.0), "2 - i");
        assert_eq!(format_latex_complex(0.0, 1.0), "i");
        assert_eq!(format_latex_complex(0.0, -1.0), "-i");
    }

    #[test]
    fn latex_complex_general_form_preserved() {
        // 确保修复不破坏一般形式
        assert_eq!(format_latex_complex(3.0, 4.0), "3 + 4i");
        assert_eq!(format_latex_complex(3.0, -4.0), "3 - 4i");
    }

    // ===== join_latex_polynomial_terms 负项空格 =====

    #[test]
    fn latex_polynomial_negative_terms_with_spaces() {
        // 负项统一用 " - " 分隔（与正项 " + " 对称）
        assert_eq!(format_latex_polynomial(&[1.0, -2.0, 1.0]), "x^{2} - 2x + 1");
        // [-1, 2] → "2x - 1"
        assert_eq!(format_latex_polynomial(&[-1.0, 2.0]), "2x - 1");
    }

    // ===== symbolic_str_to_latex 幂运算 =====

    #[test]
    fn latex_symbolic_str_to_latex_pow() {
        // "x^2" 应转换为 "x^{2}"（LaTeX 规范）
        assert_eq!(symbolic_str_to_latex("x^2"), "x^{2}");
        assert_eq!(symbolic_str_to_latex("x^10"), "x^{10}");
    }

    #[test]
    fn latex_symbolic_str_to_latex_combined_ops() {
        // 综合测试：乘法和幂混合
        assert_eq!(symbolic_str_to_latex("2*x^2"), "2 \\cdot x^{2}");
        assert_eq!(symbolic_str_to_latex("x^2*y"), "x^{2} \\cdot y");
    }

    #[test]
    fn latex_symbolic_str_to_latex_preserves_plain() {
        // 确保修复不破坏无幂运算的字符串
        assert_eq!(symbolic_str_to_latex("2*x"), "2 \\cdot x");
        assert_eq!(symbolic_str_to_latex("42"), "42");
    }
}
