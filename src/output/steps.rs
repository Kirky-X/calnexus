// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! 求值步骤生成器（v1.1 新增）。
//!
//! 遍历 AST 以求值顺序生成步骤列表，每行格式 `lhs op rhs = partial_result`。
//!
//! PRD §4.1.1 示例：
//! - `--steps "(2+9)*7-6"` 输出 `2+9=11 → 11*7=77 → 77-6=71`
//!
//! 设计依据：
//! - design.md D3：步骤是 CLI 侧展示关注点，纯 AST walker
//! - design.md Risks：复用 256 深度限制防止栈溢出
//! - tasks.md 3.1：post-order 遍历 + 256 深度上限

use crate::core::{AstNode, BinaryOp, CalcError, EvalContext, UnaryOp};

/// 最大 AST 深度（与 parser 保持一致）。
const MAX_DEPTH: usize = 256;

/// 生成求值步骤列表。
///
/// 遍历 AST 后序（先子节点，后父节点），每个非叶节点输出一行
/// `lhs op rhs = partial_result`。叶节点（Number/Variable/Complex/Matrix/List）
/// 不输出步骤，但返回其值用于父节点的步骤。
///
/// `ctx` 提供变量绑定；未绑定的变量视为 0.0（与 ArithmeticDomain 行为一致）。
pub fn generate_steps(ast: &AstNode, ctx: &EvalContext) -> Result<Vec<String>, CalcError> {
    let mut steps = Vec::new();
    let _ = walk(ast, ctx, &mut steps, 0)?;
    Ok(steps)
}

/// 递归遍历 AST，返回当前节点的数值结果（用于父节点的步骤格式化）。
///
/// `depth` 为当前递归深度，超过 `MAX_DEPTH` 返回 `DepthExceeded`。
fn walk(
    node: &AstNode,
    ctx: &EvalContext,
    steps: &mut Vec<String>,
    depth: usize,
) -> Result<f64, CalcError> {
    if depth > MAX_DEPTH {
        return Err(CalcError::depth_exceeded());
    }
    match node {
        AstNode::Number(n) => Ok(*n),
        AstNode::BigNumber(s) => s
            .parse::<f64>()
            .map_err(|_| {
                CalcError::eval(format!("invalid BigNumber: {}", s)).with_i18n(
                    "msg.output.invalid_bignumber",
                    vec![("value".to_string(), s.to_string())],
                )
            }),
        AstNode::Complex(re, _im) => Ok(*re),
        AstNode::Variable(name) => {
            // 用户绑定的变量优先（与 scientific/statistics/matrix domain 预绑定 pi/e 一致）
            if let Some(v) = ctx.get_var(name) {
                return Ok(v);
            }
            // 数学常量 pi/e：parser.rs line 708 将 0-arity FunctionCall("pi"/"e") 转为 Variable，
            // 此处识别为数学常量，而非未绑定变量（否则 pi/e 被错误求值为 0.0）
            match name.as_str() {
                "pi" => Ok(std::f64::consts::PI),
                "e" => Ok(std::f64::consts::E),
                _ => Ok(0.0), // 未绑定变量视为 0.0（与 ArithmeticDomain 行为一致）
            }
        }
        AstNode::BinaryOp(op, lhs, rhs) => {
            let l = walk(lhs, ctx, steps, depth + 1)?;
            let r = walk(rhs, ctx, steps, depth + 1)?;
            let result = eval_binary(*op, l, r)?;
            let op_str = binary_op_str(*op);
            let lhs_str = format_value(l);
            let rhs_str = format_value(r);
            let result_str = format_value(result);
            steps.push(format!("{}{}{}={}", lhs_str, op_str, rhs_str, result_str));
            Ok(result)
        }
        AstNode::UnaryOp(op, expr) => {
            let v = walk(expr, ctx, steps, depth + 1)?;
            let result = eval_unary(*op, v)?;
            let inner_str = format_value(v);
            let result_str = format_value(result);
            match op {
                UnaryOp::Neg => steps.push(format!("-{}={}", inner_str, result_str)),
                UnaryOp::Factorial => steps.push(format!("{}!={}", inner_str, result_str)),
                UnaryOp::Abs => steps.push(format!("abs({})={}", inner_str, result_str)),
            }
            Ok(result)
        }
        AstNode::FunctionCall(name, args) => {
            // 求值所有参数，对每个参数子树递归生成步骤
            let mut arg_values = Vec::with_capacity(args.len());
            for arg in args {
                arg_values.push(walk(arg, ctx, steps, depth + 1)?);
            }
            let result = eval_function(name, &arg_values)?;
            let args_str: Vec<String> = arg_values.iter().map(|v| format_value(*v)).collect();
            let args_joined = args_str.join(",");
            let result_str = format_value(result);
            steps.push(format!("{}({})={}", name, args_joined, result_str));
            Ok(result)
        }
        AstNode::Matrix(rows) => {
            // 矩阵不输出步骤，返回 0.0（步骤模式不适用于矩阵运算）
            let _ = rows;
            Ok(0.0)
        }
        AstNode::List(elements) => {
            // 列表：递归求值每个元素但不输出步骤
            let mut sum = 0.0;
            for e in elements {
                sum += walk(e, ctx, steps, depth + 1)?;
            }
            Ok(sum)
        }
    }
}

/// 计算二元运算结果。
fn eval_binary(op: BinaryOp, a: f64, b: f64) -> Result<f64, CalcError> {
    let result = match op {
        BinaryOp::Add => a + b,
        BinaryOp::Sub => a - b,
        BinaryOp::Mul => a * b,
        BinaryOp::Div => {
            if b == 0.0 {
                return Err(CalcError::division_by_zero());
            }
            a / b
        }
        BinaryOp::Pow => {
            if a == 0.0 && b < 0.0 {
                return Err(CalcError::division_by_zero());
            }
            a.powf(b)
        }
        BinaryOp::Mod => {
            if b == 0.0 {
                return Err(CalcError::division_by_zero());
            }
            a % b
        }
    };
    if result.is_nan() || result.is_infinite() {
        return Err(CalcError::nan_or_inf());
    }
    Ok(result)
}

/// 计算一元运算结果。
fn eval_unary(op: UnaryOp, v: f64) -> Result<f64, CalcError> {
    let result = match op {
        UnaryOp::Neg => -v,
        UnaryOp::Factorial => {
            if v < 0.0 || v.fract() != 0.0 {
                return Err(CalcError::domain(format!(
                    "factorial requires non-negative integer, got {}",
                    v
                ))
                .with_i18n(
                    "msg.core.factorial_negative",
                    vec![("value".to_string(), v.to_string())],
                ));
            }
            if v > 170.0 {
                return Err(CalcError::overflow());
            }
            let n = v as u64;
            let mut acc: f64 = 1.0;
            for i in 2..=n {
                acc *= i as f64;
                if !acc.is_finite() {
                    return Err(CalcError::overflow());
                }
            }
            acc
        }
        UnaryOp::Abs => v.abs(),
    };
    if result.is_nan() || result.is_infinite() {
        return Err(CalcError::nan_or_inf());
    }
    Ok(result)
}

/// 计算函数调用结果（仅常见数学函数）。
fn eval_function(name: &str, args: &[f64]) -> Result<f64, CalcError> {
    use std::f64::consts::PI;
    let result = match (name, args) {
        ("sin", &[x]) => x.sin(),
        ("cos", &[x]) => x.cos(),
        ("tan", &[x]) => x.tan(),
        ("asin", &[x]) => {
            check_unit_range(name, x)?;
            x.asin()
        }
        ("acos", &[x]) => {
            check_unit_range(name, x)?;
            x.acos()
        }
        ("atan", &[x]) => x.atan(),
        ("atan2", &[y, x]) => y.atan2(x),
        ("sqrt", &[x]) => {
            check_non_negative(name, x)?;
            x.sqrt()
        }
        ("exp", &[x]) => x.exp(),
        ("ln", &[x]) | ("log", &[x]) => {
            check_positive(name, x)?;
            x.ln()
        }
        ("log10", &[x]) => {
            check_positive(name, x)?;
            x.log10()
        }
        ("log2", &[x]) => {
            check_positive(name, x)?;
            x.log2()
        }
        ("abs", &[x]) => x.abs(),
        ("floor", &[x]) => x.floor(),
        ("ceil", &[x]) => x.ceil(),
        ("round", &[x]) => x.round(),
        ("gcd", &[a, b]) => {
            let a_int = a as i64;
            let b_int = b as i64;
            gcd(a_int, b_int) as f64
        }
        ("lcm", &[a, b]) => {
            let a_int = a as i64;
            let b_int = b as i64;
            lcm(a_int, b_int) as f64
        }
        ("min", &[a, b]) => a.min(b),
        ("max", &[a, b]) => a.max(b),
        ("pow", &[a, b]) => a.powf(b),
        ("pi", &[]) => PI,
        ("e", &[]) => std::f64::consts::E,
        _ => {
            return Err(CalcError::eval(format!(
                "unknown function '{}' with {} args in steps",
                name,
                args.len()
            ))
            .with_i18n(
                "msg.output.unknown_function",
                vec![
                    ("name".to_string(), name.to_string()),
                    ("actual".to_string(), args.len().to_string()),
                ],
            ))
        }
    };
    if result.is_nan() || result.is_infinite() {
        return Err(CalcError::nan_or_inf());
    }
    Ok(result)
}

/// 验证参数在 `[-1, 1]` 范围内（asin/acos 域约束）。
fn check_unit_range(name: &str, x: f64) -> Result<(), CalcError> {
    if !(-1.0..=1.0).contains(&x) {
        return Err(CalcError::domain(format!(
            "{} domain [-1,1], got {}",
            name, x
        ))
        .with_i18n(
            "msg.output.domain_range",
            vec![
                ("name".to_string(), name.to_string()),
                ("value".to_string(), x.to_string()),
            ],
        ));
    }
    Ok(())
}

/// 验证参数为非负数（sqrt 域约束）。
fn check_non_negative(name: &str, x: f64) -> Result<(), CalcError> {
    if x < 0.0 {
        return Err(CalcError::domain(format!(
            "{} requires non-negative, got {}",
            name, x
        ))
        .with_i18n(
            "msg.output.requires_non_negative",
            vec![
                ("name".to_string(), name.to_string()),
                ("value".to_string(), x.to_string()),
            ],
        ));
    }
    Ok(())
}

/// 验证参数为正数（ln/log/log10/log2 域约束）。
fn check_positive(name: &str, x: f64) -> Result<(), CalcError> {
    if x <= 0.0 {
        return Err(CalcError::domain(format!(
            "{} requires positive, got {}",
            name, x
        ))
        .with_i18n(
            "msg.output.requires_positive",
            vec![
                ("name".to_string(), name.to_string()),
                ("value".to_string(), x.to_string()),
            ],
        ));
    }
    Ok(())
}

fn gcd(a: i64, b: i64) -> i64 {
    let mut a = a.abs();
    let mut b = b.abs();
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}

fn lcm(a: i64, b: i64) -> i64 {
    if a == 0 || b == 0 {
        return 0;
    }
    (a.abs() / gcd(a, b)) * b.abs()
}

/// 二元运算符字符串表示。
fn binary_op_str(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::Pow => "^",
        BinaryOp::Mod => "%",
    }
}

/// 格式化数值为步骤中的字符串（整数显示为整数，否则小数）。
fn format_value(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{}", v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::BinaryOp;
    use crate::core::ErrorKind;

    #[test]
    fn steps_basic_arithmetic_2plus3() {
        let ast = AstNode::BinaryOp(
            BinaryOp::Add,
            Box::new(AstNode::Number(2.0)),
            Box::new(AstNode::Number(3.0)),
        );
        let steps = generate_steps(&ast, &EvalContext::new()).unwrap();
        assert_eq!(steps, vec!["2+3=5"]);
    }

    #[test]
    fn steps_complex_expression_2plus9times7minus6() {
        // (2+9)*7-6
        let ast = AstNode::BinaryOp(
            BinaryOp::Sub,
            Box::new(AstNode::BinaryOp(
                BinaryOp::Mul,
                Box::new(AstNode::BinaryOp(
                    BinaryOp::Add,
                    Box::new(AstNode::Number(2.0)),
                    Box::new(AstNode::Number(9.0)),
                )),
                Box::new(AstNode::Number(7.0)),
            )),
            Box::new(AstNode::Number(6.0)),
        );
        let steps = generate_steps(&ast, &EvalContext::new()).unwrap();
        assert_eq!(
            steps,
            vec!["2+9=11", "11*7=77", "77-6=71"],
            "PRD §4.1.1 example"
        );
    }

    #[test]
    fn steps_division() {
        let ast = AstNode::BinaryOp(
            BinaryOp::Div,
            Box::new(AstNode::Number(10.0)),
            Box::new(AstNode::Number(2.0)),
        );
        let steps = generate_steps(&ast, &EvalContext::new()).unwrap();
        assert_eq!(steps, vec!["10/2=5"]);
    }

    #[test]
    fn steps_pow() {
        let ast = AstNode::BinaryOp(
            BinaryOp::Pow,
            Box::new(AstNode::Number(2.0)),
            Box::new(AstNode::Number(3.0)),
        );
        let steps = generate_steps(&ast, &EvalContext::new()).unwrap();
        assert_eq!(steps, vec!["2^3=8"]);
    }

    #[test]
    fn steps_mod() {
        let ast = AstNode::BinaryOp(
            BinaryOp::Mod,
            Box::new(AstNode::Number(10.0)),
            Box::new(AstNode::Number(3.0)),
        );
        let steps = generate_steps(&ast, &EvalContext::new()).unwrap();
        assert_eq!(steps, vec!["10%3=1"]);
    }

    #[test]
    fn steps_unary_neg() {
        let ast = AstNode::UnaryOp(UnaryOp::Neg, Box::new(AstNode::Number(5.0)));
        let steps = generate_steps(&ast, &EvalContext::new()).unwrap();
        assert_eq!(steps, vec!["-5=-5"]);
    }

    #[test]
    fn steps_unary_factorial() {
        let ast = AstNode::UnaryOp(UnaryOp::Factorial, Box::new(AstNode::Number(5.0)));
        let steps = generate_steps(&ast, &EvalContext::new()).unwrap();
        assert_eq!(steps, vec!["5!=120"]);
    }

    #[test]
    fn steps_unary_abs() {
        let ast = AstNode::UnaryOp(UnaryOp::Abs, Box::new(AstNode::Number(-5.0)));
        let steps = generate_steps(&ast, &EvalContext::new()).unwrap();
        assert_eq!(steps, vec!["abs(-5)=5"]);
    }

    #[test]
    fn steps_function_sin() {
        let ast = AstNode::FunctionCall("sin".to_string(), vec![AstNode::Number(0.0)]);
        let steps = generate_steps(&ast, &EvalContext::new()).unwrap();
        assert_eq!(steps, vec!["sin(0)=0"]);
    }

    #[test]
    fn steps_function_with_nested_args() {
        // sin(2+3)
        let ast = AstNode::FunctionCall(
            "sin".to_string(),
            vec![AstNode::BinaryOp(
                BinaryOp::Add,
                Box::new(AstNode::Number(2.0)),
                Box::new(AstNode::Number(3.0)),
            )],
        );
        let steps = generate_steps(&ast, &EvalContext::new()).unwrap();
        assert_eq!(steps, vec!["2+3=5", "sin(5)=-0.9589242746631385"]);
    }

    #[test]
    fn steps_variable_substitution() {
        // x*2 with x=10
        let ast = AstNode::BinaryOp(
            BinaryOp::Mul,
            Box::new(AstNode::Variable("x".to_string())),
            Box::new(AstNode::Number(2.0)),
        );
        let ctx = EvalContext::new().with_var("x", 10.0);
        let steps = generate_steps(&ast, &ctx).unwrap();
        assert_eq!(steps, vec!["10*2=20"]);
    }

    #[test]
    fn steps_unbound_variable_defaults_zero() {
        // 未绑定变量视为 0.0
        let ast = AstNode::BinaryOp(
            BinaryOp::Add,
            Box::new(AstNode::Variable("y".to_string())),
            Box::new(AstNode::Number(2.0)),
        );
        let ctx = EvalContext::new();
        let steps = generate_steps(&ast, &ctx).unwrap();
        assert_eq!(steps, vec!["0+2=2"]);
    }

    #[test]
    fn steps_nested_expression() {
        // ((1+2)*(3+4))-5
        let ast = AstNode::BinaryOp(
            BinaryOp::Sub,
            Box::new(AstNode::BinaryOp(
                BinaryOp::Mul,
                Box::new(AstNode::BinaryOp(
                    BinaryOp::Add,
                    Box::new(AstNode::Number(1.0)),
                    Box::new(AstNode::Number(2.0)),
                )),
                Box::new(AstNode::BinaryOp(
                    BinaryOp::Add,
                    Box::new(AstNode::Number(3.0)),
                    Box::new(AstNode::Number(4.0)),
                )),
            )),
            Box::new(AstNode::Number(5.0)),
        );
        let steps = generate_steps(&ast, &EvalContext::new()).unwrap();
        // 1+2=3, 3+4=7, 3*7=21, 21-5=16
        assert_eq!(steps, vec!["1+2=3", "3+4=7", "3*7=21", "21-5=16"]);
    }

    #[test]
    fn steps_division_by_zero_returns_error() {
        let ast = AstNode::BinaryOp(
            BinaryOp::Div,
            Box::new(AstNode::Number(1.0)),
            Box::new(AstNode::Number(0.0)),
        );
        let err = generate_steps(&ast, &EvalContext::new()).unwrap_err();
        assert_eq!(err, CalcError::division_by_zero());
    }

    #[test]
    fn steps_factorial_overflow_returns_error() {
        let ast = AstNode::UnaryOp(UnaryOp::Factorial, Box::new(AstNode::Number(200.0)));
        let err = generate_steps(&ast, &EvalContext::new()).unwrap_err();
        assert_eq!(err, CalcError::overflow());
    }

    #[test]
    fn steps_factorial_negative_returns_domain_error() {
        let ast = AstNode::UnaryOp(UnaryOp::Factorial, Box::new(AstNode::Number(-3.0)));
        let err = generate_steps(&ast, &EvalContext::new()).unwrap_err();
        assert!(err.kind == ErrorKind::Domain);
    }

    #[test]
    fn steps_log_negative_returns_domain_error() {
        let ast = AstNode::FunctionCall("ln".to_string(), vec![AstNode::Number(-1.0)]);
        let err = generate_steps(&ast, &EvalContext::new()).unwrap_err();
        assert!(err.kind == ErrorKind::Domain);
    }

    #[test]
    fn steps_sqrt_negative_returns_domain_error() {
        let ast = AstNode::FunctionCall("sqrt".to_string(), vec![AstNode::Number(-4.0)]);
        let err = generate_steps(&ast, &EvalContext::new()).unwrap_err();
        assert!(err.kind == ErrorKind::Domain);
    }

    #[test]
    fn steps_unknown_function_returns_error() {
        let ast = AstNode::FunctionCall("unknownfn".to_string(), vec![AstNode::Number(1.0)]);
        let err = generate_steps(&ast, &EvalContext::new()).unwrap_err();
        assert!(err.kind == ErrorKind::Eval);
    }

    #[test]
    fn steps_complex_leaf_node_no_step_emitted() {
        // Complex 字面量作为叶节点不输出步骤
        let ast = AstNode::Complex(3.0, 4.0);
        let steps = generate_steps(&ast, &EvalContext::new()).unwrap();
        assert!(steps.is_empty());
    }

    #[test]
    fn steps_matrix_leaf_no_step_emitted() {
        let ast = AstNode::Matrix(vec![vec![AstNode::Number(1.0)]]);
        let steps = generate_steps(&ast, &EvalContext::new()).unwrap();
        assert!(steps.is_empty());
    }

    #[test]
    fn steps_list_emits_no_step_but_walks_elements() {
        // [1+1, 2+2] → 1+1=2, 2+2=4 (子节点步骤仍输出)
        let ast = AstNode::List(vec![
            AstNode::BinaryOp(
                BinaryOp::Add,
                Box::new(AstNode::Number(1.0)),
                Box::new(AstNode::Number(1.0)),
            ),
            AstNode::BinaryOp(
                BinaryOp::Add,
                Box::new(AstNode::Number(2.0)),
                Box::new(AstNode::Number(2.0)),
            ),
        ]);
        let steps = generate_steps(&ast, &EvalContext::new()).unwrap();
        assert_eq!(steps, vec!["1+1=2", "2+2=4"]);
    }

    #[test]
    fn steps_pi_constant() {
        let ast = AstNode::FunctionCall("pi".to_string(), vec![]);
        let steps = generate_steps(&ast, &EvalContext::new()).unwrap();
        assert_eq!(steps.len(), 1);
        assert!(steps[0].starts_with("pi()="));
    }

    // ===== 回归测试：Variable("pi")/Variable("e") 应返回数学常量，而非 0.0 =====
    // 用户报告 bug：`calnexus --steps 'sin(pi/2)'` 输出 `0/2=0 → sin(0)=0`
    // 根因：parser 将 `pi`/`e` 转为 Variable（parser.rs line 708），但 steps.rs walk() 的
    // Variable 分支用 `ctx.get_var(name).unwrap_or(0.0)`，未绑定变量被错误求值为 0.0。

    #[test]
    fn steps_variable_pi_returns_math_constant() {
        // Variable("pi") 单独求值：叶节点不输出步骤，但值应为 PI
        let ast = AstNode::Variable("pi".to_string());
        let steps = generate_steps(&ast, &EvalContext::new()).unwrap();
        assert!(steps.is_empty(), "叶节点不应输出步骤");
        // 通过父节点验证 Variable("pi") 的值
        let ast = AstNode::BinaryOp(
            BinaryOp::Div,
            Box::new(AstNode::Variable("pi".to_string())),
            Box::new(AstNode::Number(2.0)),
        );
        let steps = generate_steps(&ast, &EvalContext::new()).unwrap();
        assert_eq!(steps, vec!["3.141592653589793/2=1.5707963267948966"]);
    }

    #[test]
    fn steps_variable_e_returns_math_constant() {
        // Variable("e") 通过父节点验证值应为 E
        let ast = AstNode::BinaryOp(
            BinaryOp::Mul,
            Box::new(AstNode::Variable("e".to_string())),
            Box::new(AstNode::Number(1.0)),
        );
        let steps = generate_steps(&ast, &EvalContext::new()).unwrap();
        assert_eq!(steps, vec!["2.718281828459045*1=2.718281828459045"]);
    }

    #[test]
    fn steps_sin_pi_over_2_equals_one() {
        // sin(pi/2) = 1（用户报告的 bug 回归测试）
        let ast = AstNode::FunctionCall(
            "sin".to_string(),
            vec![AstNode::BinaryOp(
                BinaryOp::Div,
                Box::new(AstNode::Variable("pi".to_string())),
                Box::new(AstNode::Number(2.0)),
            )],
        );
        let steps = generate_steps(&ast, &EvalContext::new()).unwrap();
        assert_eq!(
            steps,
            vec![
                "3.141592653589793/2=1.5707963267948966",
                "sin(1.5707963267948966)=1"
            ]
        );
    }

    #[test]
    fn steps_unbound_variable_still_defaults_zero() {
        // 未绑定变量（非 pi/e）仍视为 0.0（保持向后兼容）
        let ast = AstNode::BinaryOp(
            BinaryOp::Add,
            Box::new(AstNode::Variable("y".to_string())),
            Box::new(AstNode::Number(2.0)),
        );
        let ctx = EvalContext::new();
        let steps = generate_steps(&ast, &ctx).unwrap();
        assert_eq!(steps, vec!["0+2=2"]);
    }

    #[test]
    fn steps_user_var_pi_overrides_constant() {
        // 用户显式绑定 pi 时，覆盖数学常量（与 scientific/statistics/matrix domain 一致）
        // 叶节点不输出步骤，通过父节点验证
        let ctx = EvalContext::new().with_var("pi", 100.0);
        let ast = AstNode::BinaryOp(
            BinaryOp::Add,
            Box::new(AstNode::Variable("pi".to_string())),
            Box::new(AstNode::Number(0.0)),
        );
        let steps = generate_steps(&ast, &ctx).unwrap();
        assert_eq!(steps, vec!["100+0=100"]);
    }

    #[test]
    fn steps_format_value_integer_and_decimal() {
        assert_eq!(format_value(42.0), "42");
        assert_eq!(format_value(3.14), "3.14");
        assert_eq!(format_value(-7.0), "-7");
    }

    #[test]
    fn steps_eval_function_min_max() {
        assert_eq!(eval_function("min", &[3.0, 5.0]).unwrap(), 3.0);
        assert_eq!(eval_function("max", &[3.0, 5.0]).unwrap(), 5.0);
    }

    #[test]
    fn steps_eval_function_gcd_lcm() {
        assert_eq!(eval_function("gcd", &[12.0, 18.0]).unwrap(), 6.0);
        assert_eq!(eval_function("lcm", &[4.0, 6.0]).unwrap(), 12.0);
    }

    #[test]
    fn steps_eval_function_pow() {
        assert_eq!(eval_function("pow", &[2.0, 10.0]).unwrap(), 1024.0);
    }

    #[test]
    fn steps_eval_function_constants() {
        assert!((eval_function("pi", &[]).unwrap() - std::f64::consts::PI).abs() < 1e-10);
        assert!((eval_function("e", &[]).unwrap() - std::f64::consts::E).abs() < 1e-10);
    }

    #[test]
    fn steps_eval_binary_returns_nan_or_inf_error() {
        // (-1)^0.5 = NaN（负数的非整数次幂）
        let err = eval_binary(BinaryOp::Pow, -1.0, 0.5).unwrap_err();
        assert_eq!(err, CalcError::nan_or_inf());
    }

    #[test]
    fn steps_eval_unary_neg_nan() {
        let err = eval_unary(UnaryOp::Neg, f64::NAN).unwrap_err();
        assert_eq!(err, CalcError::nan_or_inf());
    }

    // ===== 覆盖 walk DepthExceeded 路径 =====

    #[test]
    fn steps_depth_exceeded_returns_error() {
        // 构造 258 层嵌套加法，超过 MAX_DEPTH=256（line 43）
        let mut ast = AstNode::Number(1.0);
        for _ in 0..258 {
            ast = AstNode::BinaryOp(BinaryOp::Add, Box::new(ast), Box::new(AstNode::Number(1.0)));
        }
        let err = generate_steps(&ast, &EvalContext::new()).unwrap_err();
        assert_eq!(err, CalcError::depth_exceeded());
    }

    // ===== 覆盖 BigNumber 叶节点路径 =====

    #[test]
    fn steps_bignumber_leaf_valid() {
        // BigNumber 解析成功（lines 47-48）
        let ast = AstNode::BigNumber("42".to_string());
        let steps = generate_steps(&ast, &EvalContext::new()).unwrap();
        assert!(steps.is_empty()); // 叶节点不输出步骤
    }

    #[test]
    fn steps_bignumber_leaf_invalid() {
        // BigNumber 解析失败（line 49）
        let ast = AstNode::BigNumber("not_a_number".to_string());
        let err = generate_steps(&ast, &EvalContext::new()).unwrap_err();
        assert!(err.kind == ErrorKind::Eval);
    }

    // ===== 覆盖 eval_binary 边界路径 =====

    #[test]
    fn steps_zero_pow_negative_returns_error() {
        // 0^(-1) → DivisionByZero（line 118）
        let err = eval_binary(BinaryOp::Pow, 0.0, -1.0).unwrap_err();
        assert_eq!(err, CalcError::division_by_zero());
    }

    #[test]
    fn steps_mod_by_zero_returns_error() {
        // 10 % 0 → DivisionByZero（line 124）
        let err = eval_binary(BinaryOp::Mod, 10.0, 0.0).unwrap_err();
        assert_eq!(err, CalcError::division_by_zero());
    }

    // ===== 覆盖 eval_function asin/acos/sqrt/ln/log10/log2 =====

    #[test]
    fn steps_asin_domain_error_and_success() {
        // asin 越界 → DomainError（lines 175-179）
        let err = eval_function("asin", &[2.0]).unwrap_err();
        assert!(err.kind == ErrorKind::Domain);
        // asin 合法 → 返回值（line 181）
        let result = eval_function("asin", &[0.5]).unwrap();
        assert!((result - 0.5f64.asin()).abs() < 1e-10);
    }

    #[test]
    fn steps_acos_domain_error_and_success() {
        // acos 越界 → DomainError（lines 184-188）
        let err = eval_function("acos", &[2.0]).unwrap_err();
        assert!(err.kind == ErrorKind::Domain);
        // acos 合法 → 返回值（line 190）
        let result = eval_function("acos", &[0.5]).unwrap();
        assert!((result - 0.5f64.acos()).abs() < 1e-10);
    }

    #[test]
    fn steps_sqrt_success() {
        // sqrt(4) = 2（line 201，错误路径已覆盖）
        let result = eval_function("sqrt", &[4.0]).unwrap();
        assert!((result - 2.0).abs() < 1e-10);
    }

    #[test]
    fn steps_ln_success() {
        // ln(1) = 0（line 211，错误路径已覆盖）
        let result = eval_function("ln", &[1.0]).unwrap();
        assert!((result - 0.0).abs() < 1e-10);
    }

    #[test]
    fn steps_log10_error_and_success() {
        // log10(0) → DomainError（lines 214-218）
        let err = eval_function("log10", &[0.0]).unwrap_err();
        assert!(err.kind == ErrorKind::Domain);
        // log10(100) = 2（line 220）
        let result = eval_function("log10", &[100.0]).unwrap();
        assert!((result - 2.0).abs() < 1e-10);
    }

    #[test]
    fn steps_log2_error_and_success() {
        // log2(0) → DomainError（lines 223-227）
        let err = eval_function("log2", &[0.0]).unwrap_err();
        assert!(err.kind == ErrorKind::Domain);
        // log2(8) = 3（line 229）
        let result = eval_function("log2", &[8.0]).unwrap();
        assert!((result - 3.0).abs() < 1e-10);
    }

    // ===== 覆盖 eval_function 返回 NaN/Inf 检查 =====

    #[test]
    fn steps_function_returns_nan_or_inf_error() {
        // exp(1000) → Inf → NaNOrInf 错误（line 259）
        let err = eval_function("exp", &[1000.0]).unwrap_err();
        assert_eq!(err, CalcError::nan_or_inf());
    }

    // ===== 覆盖 lcm 零值路径 =====

    #[test]
    fn steps_lcm_with_zero() {
        // lcm(0, 5) = 0（line 277）
        let result = eval_function("lcm", &[0.0, 5.0]).unwrap();
        assert_eq!(result, 0.0);
    }
}
