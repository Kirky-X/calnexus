//! 表达式解析器：将数学表达式字符串解析为 [`AstNode`]。
//!
//! 基于 mathexpr crate，添加：
//! - 阶乘 `!` 预处理（mathexpr 不原生支持 `!`，转换为 `factorial()`）
//! - AST 转换层（mathexpr `Expr` → CalNexus `AstNode`）
//! - 深度/长度限制（DoS 防护）
//!
//! 设计依据：design.md D2（mathexpr 集成）、D7（TDD）、expression-parsing spec

use crate::core::types::{AstNode, BinaryOp, CalcError, UnaryOp};

/// 最大 AST 深度（spec: AST 深度限制 ≤ 256）。
const MAX_AST_DEPTH: usize = 256;

/// 最大表达式长度（spec: 表达式长度限制 ≤ 4096 字符）。
const MAX_EXPR_LEN: usize = 4096;

/// 解析数学表达式字符串为 [`AstNode`]。
///
/// # 流程
/// 1. 长度检查（O(1) 快速失败）
/// 2. 空字符串检查
/// 3. 阶乘预处理：`5!` → `factorial(5)`
/// 4. mathexpr 解析
/// 5. 转换为 CalNexus AstNode（`%` → `mod()`，`pi`/`e` → Variable），带深度检查
pub fn parse(input: &str) -> Result<AstNode, CalcError> {
    // 长度检查（spec: 超长输入不进入词法分析）
    if input.len() > MAX_EXPR_LEN {
        return Err(CalcError::ParseError(format!(
            "expression length {} exceeds maximum of {} characters",
            input.len(),
            MAX_EXPR_LEN
        )));
    }

    let trimmed = input.trim();

    // 空字符串检查
    if trimmed.is_empty() {
        return Err(CalcError::ParseError("expression is empty".to_string()));
    }

    // 非法连续运算符检查（mathexpr 将 `+3` 当作数字字面量，需在此显式拒绝 `++`）
    validate_no_consecutive_plus(trimmed)?;

    // 阶乘预处理
    let preprocessed = preprocess_factorial(trimmed)?;

    // mathexpr 解析
    let expr = mathexpr::parse(&preprocessed)
        .map_err(|e| CalcError::ParseError(format!("{}", e)))?;

    // 转换为 CalNexus AstNode（含深度检查，防止递归栈溢出）
    let ast = convert_with_depth(&expr, 1)?;

    Ok(ast)
}

/// 拒绝非法连续运算符 `++`。
///
/// mathexpr 依赖 `nom::number::complete::double`，该解析器接受 `+3` 作为数字字面量，
/// 导致 `2++3` 被静默接受为 `2 + 3.0`。此函数在解析前显式拒绝 `++` 模式。
fn validate_no_consecutive_plus(input: &str) -> Result<(), CalcError> {
    let stripped: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    if stripped.contains("++") {
        return Err(CalcError::ParseError(
            "illegal consecutive operators '++'".to_string(),
        ));
    }
    Ok(())
}

/// 预处理阶乘运算符：将 `expr!` 转换为 `factorial(expr)`。
///
/// 支持的操作数形式：
/// - 数字：`5!` → `factorial(5)`
/// - 括号表达式：`(2+3)!` → `factorial((2+3))`
/// - 变量：`x!` → `factorial(x)`
/// - 函数调用：`sin(x)!` → `factorial(sin(x))`
/// - 多重阶乘：`5!!` → `factorial(factorial(5))`
fn preprocess_factorial(input: &str) -> Result<String, CalcError> {
    let chars: Vec<char> = input.chars().collect();
    let mut result: Vec<char> = Vec::with_capacity(chars.len() + 32);
    let mut i = 0;

    while i < chars.len() {
        if chars[i] == '!' {
            let operand_start = find_operand_start(&result)?;
            let operand: String = result[operand_start..].iter().collect();
            result.truncate(operand_start);
            result.extend("factorial(".chars());
            result.extend(operand.chars());
            result.push(')');
        } else {
            result.push(chars[i]);
        }
        i += 1;
    }

    Ok(result.into_iter().collect())
}

/// 从字符缓冲区末尾向前找到操作数的起始索引。
///
/// 操作数识别规则：
/// - 若末尾是 `)`：向左匹配括号，再继续向左扫描函数名（如 `sin(x)` 的 `sin`）
/// - 否则：向左扫描连续的数字、字母、小数点、下划线
fn find_operand_start(chars: &[char]) -> Result<usize, CalcError> {
    let mut pos = chars.len();

    // 跳过尾部空格
    while pos > 0 && chars[pos - 1].is_whitespace() {
        pos -= 1;
    }

    if pos == 0 {
        return Err(CalcError::ParseError(
            "factorial operator '!' has no operand".to_string(),
        ));
    }

    // 如果最后一个字符是 ')'，向左匹配括号
    if chars[pos - 1] == ')' {
        let mut depth = 1;
        pos -= 1;
        while pos > 0 && depth > 0 {
            pos -= 1;
            match chars[pos] {
                ')' => depth += 1,
                '(' => depth -= 1,
                _ => {}
            }
        }
        if depth != 0 {
            return Err(CalcError::ParseError(
                "unmatched parenthesis in factorial operand".to_string(),
            ));
        }
        // pos 现在指向 '(' 的位置
        // 继续向左扫描函数名（如果 '(' 前面有字母）
        while pos > 0 && chars[pos - 1].is_alphabetic() {
            pos -= 1;
        }
    } else {
        // 向左扫描连续的数字、字母、小数点、下划线
        while pos > 0 {
            let c = chars[pos - 1];
            if c.is_alphanumeric() || c == '.' || c == '_' {
                pos -= 1;
            } else {
                break;
            }
        }
    }

    Ok(pos)
}

/// 将 mathexpr 的 `Expr` 转换为 CalNexus 的 [`AstNode`]，带深度检查。
///
/// 转换规则：
/// - `BinOp::Mod` → `FunctionCall("mod", ...)`（spec 要求取模为函数调用形式）
/// - 0-arity `FunctionCall("pi"/"e")` → `Variable("pi"/"e")`（spec 要求常量为变量形式）
/// - `UnaryMinus` → `UnaryOp(Neg, ...)`
/// - `CurrentValue` (`_`) → `Variable("_")`
///
/// 深度检查在转换时执行，超过 MAX_AST_DEPTH 立即返回错误，防止递归栈溢出。
fn convert_with_depth(expr: &mathexpr::Expr, depth: usize) -> Result<AstNode, CalcError> {
    if depth > MAX_AST_DEPTH {
        return Err(CalcError::DepthExceeded);
    }

    use mathexpr::{BinOp as MBinOp, Expr};

    match expr {
        Expr::Number(n) => Ok(AstNode::Number(*n)),
        Expr::Variable(name) => Ok(AstNode::Variable(name.clone())),
        Expr::CurrentValue => Ok(AstNode::Variable("_".to_string())),
        Expr::BinaryOp { op, left, right } => {
            let l = convert_with_depth(left, depth + 1)?;
            let r = convert_with_depth(right, depth + 1)?;
            match op {
                MBinOp::Mod => Ok(AstNode::FunctionCall(
                    "mod".to_string(),
                    vec![l, r],
                )),
                MBinOp::Add => Ok(AstNode::BinaryOp(BinaryOp::Add, Box::new(l), Box::new(r))),
                MBinOp::Sub => Ok(AstNode::BinaryOp(BinaryOp::Sub, Box::new(l), Box::new(r))),
                MBinOp::Mul => Ok(AstNode::BinaryOp(BinaryOp::Mul, Box::new(l), Box::new(r))),
                MBinOp::Div => Ok(AstNode::BinaryOp(BinaryOp::Div, Box::new(l), Box::new(r))),
                MBinOp::Pow => Ok(AstNode::BinaryOp(BinaryOp::Pow, Box::new(l), Box::new(r))),
            }
        }
        Expr::UnaryMinus(inner) => {
            let e = convert_with_depth(inner, depth + 1)?;
            Ok(AstNode::UnaryOp(UnaryOp::Neg, Box::new(e)))
        }
        Expr::FunctionCall { name, args } => {
            // 0-arity 的 pi/e 转换为 Variable（spec: 数学常量解析）
            if args.is_empty() && (name == "pi" || name == "e") {
                return Ok(AstNode::Variable(name.clone()));
            }
            let mut converted_args = Vec::with_capacity(args.len());
            for arg in args {
                converted_args.push(convert_with_depth(arg, depth + 1)?);
            }
            Ok(AstNode::FunctionCall(name.clone(), converted_args))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::*;

    // 辅助函数
    fn binop(op: BinaryOp, l: AstNode, r: AstNode) -> AstNode {
        AstNode::BinaryOp(op, Box::new(l), Box::new(r))
    }
    fn unary(op: UnaryOp, e: AstNode) -> AstNode {
        AstNode::UnaryOp(op, Box::new(e))
    }
    fn call(name: &str, args: Vec<AstNode>) -> AstNode {
        AstNode::FunctionCall(name.to_string(), args)
    }
    fn num(n: f64) -> AstNode {
        AstNode::Number(n)
    }
    fn var(name: &str) -> AstNode {
        AstNode::Variable(name.to_string())
    }

    // ===== Requirement 1: 基本四则运算解析 =====

    #[test]
    fn test_simple_addition() {
        let ast = parse("2+3").unwrap();
        assert_eq!(ast, binop(BinaryOp::Add, num(2.0), num(3.0)));
    }

    #[test]
    fn test_mixed_arithmetic_precedence() {
        let ast = parse("(2+9)*7-6").unwrap();
        assert_eq!(
            ast,
            binop(
                BinaryOp::Sub,
                binop(
                    BinaryOp::Mul,
                    binop(BinaryOp::Add, num(2.0), num(9.0)),
                    num(7.0)
                ),
                num(6.0)
            )
        );
    }

    #[test]
    fn test_left_associative_subtraction() {
        let ast = parse("10-3-2").unwrap();
        assert_eq!(
            ast,
            binop(
                BinaryOp::Sub,
                binop(BinaryOp::Sub, num(10.0), num(3.0)),
                num(2.0)
            )
        );
    }

    // ===== Requirement 2: 幂运算解析 =====

    #[test]
    fn test_simple_power() {
        let ast = parse("2^10").unwrap();
        assert_eq!(ast, binop(BinaryOp::Pow, num(2.0), num(10.0)));
    }

    #[test]
    fn test_power_right_associative() {
        let ast = parse("2^3^2").unwrap();
        assert_eq!(
            ast,
            binop(
                BinaryOp::Pow,
                num(2.0),
                binop(BinaryOp::Pow, num(3.0), num(2.0))
            )
        );
    }

    #[test]
    fn test_power_higher_precedence_than_mul() {
        let ast = parse("2*3^2").unwrap();
        assert_eq!(
            ast,
            binop(
                BinaryOp::Mul,
                num(2.0),
                binop(BinaryOp::Pow, num(3.0), num(2.0))
            )
        );
    }

    // ===== Requirement 3: 阶乘运算解析 =====

    #[test]
    fn test_integer_factorial() {
        let ast = parse("5!").unwrap();
        assert_eq!(ast, call("factorial", vec![num(5.0)]));
    }

    #[test]
    fn test_factorial_on_parenthesized_expr() {
        let ast = parse("(2+3)!").unwrap();
        assert_eq!(
            ast,
            call("factorial", vec![binop(BinaryOp::Add, num(2.0), num(3.0))])
        );
    }

    #[test]
    fn test_factorial_with_unary_minus() {
        let ast = parse("-5!").unwrap();
        assert_eq!(ast, unary(UnaryOp::Neg, call("factorial", vec![num(5.0)])));
    }

    // ===== Requirement 4: 取模运算解析 =====

    #[test]
    fn test_simple_modulo() {
        let ast = parse("10%3").unwrap();
        assert_eq!(ast, call("mod", vec![num(10.0), num(3.0)]));
    }

    #[test]
    fn test_modulo_with_arithmetic() {
        let ast = parse("10%3+1").unwrap();
        assert_eq!(
            ast,
            binop(
                BinaryOp::Add,
                call("mod", vec![num(10.0), num(3.0)]),
                num(1.0)
            )
        );
    }

    #[test]
    fn test_modulo_with_subexpressions() {
        let ast = parse("(2+8)%(4-1)").unwrap();
        assert_eq!(
            ast,
            call(
                "mod",
                vec![
                    binop(BinaryOp::Add, num(2.0), num(8.0)),
                    binop(BinaryOp::Sub, num(4.0), num(1.0))
                ]
            )
        );
    }

    // ===== Requirement 5: 函数调用解析 =====

    #[test]
    fn test_single_arg_function() {
        let ast = parse("sin(pi/2)").unwrap();
        assert_eq!(
            ast,
            call("sin", vec![binop(BinaryOp::Div, var("pi"), num(2.0))])
        );
    }

    #[test]
    fn test_multi_arg_function() {
        let ast = parse("log(100, 10)").unwrap();
        assert_eq!(ast, call("log", vec![num(100.0), num(10.0)]));
    }

    #[test]
    fn test_special_function_gamma() {
        let ast = parse("gamma(5)").unwrap();
        assert_eq!(ast, call("gamma", vec![num(5.0)]));
    }

    #[test]
    fn test_nested_function_calls() {
        let ast = parse("sin(cos(0))").unwrap();
        assert_eq!(ast, call("sin", vec![call("cos", vec![num(0.0)])]));
    }

    // ===== Requirement 6: 变量引用解析 =====

    #[test]
    fn test_simple_variable_expr() {
        let ast = parse("x+y").unwrap();
        assert_eq!(ast, binop(BinaryOp::Add, var("x"), var("y")));
    }

    #[test]
    fn test_variable_as_function_arg() {
        let ast = parse("sin(x)").unwrap();
        assert_eq!(ast, call("sin", vec![var("x")]));
    }

    #[test]
    fn test_multichar_variable_names() {
        let ast = parse("alpha+beta_1").unwrap();
        assert_eq!(ast, binop(BinaryOp::Add, var("alpha"), var("beta_1")));
    }

    // ===== Requirement 7: 括号嵌套解析 =====

    #[test]
    fn test_double_nested_parens() {
        let ast = parse("((1+2)*3)").unwrap();
        assert_eq!(
            ast,
            binop(
                BinaryOp::Mul,
                binop(BinaryOp::Add, num(1.0), num(2.0)),
                num(3.0)
            )
        );
    }

    #[test]
    fn test_multi_level_parens() {
        let ast = parse("(1+(2+3))*4").unwrap();
        assert_eq!(
            ast,
            binop(
                BinaryOp::Mul,
                binop(
                    BinaryOp::Add,
                    num(1.0),
                    binop(BinaryOp::Add, num(2.0), num(3.0))
                ),
                num(4.0)
            )
        );
    }

    #[test]
    fn test_optional_parens_equivalent() {
        let with_parens = parse("(1+2)").unwrap();
        let without_parens = parse("1+2").unwrap();
        assert_eq!(with_parens, without_parens);
    }

    // ===== Requirement 8: 数学常量解析 =====

    #[test]
    fn test_pi_constant() {
        let ast = parse("pi").unwrap();
        assert_eq!(ast, var("pi"));
    }

    #[test]
    fn test_e_constant_in_expr() {
        let ast = parse("e*2").unwrap();
        assert_eq!(ast, binop(BinaryOp::Mul, var("e"), num(2.0)));
    }

    #[test]
    fn test_constant_in_function_arg() {
        let ast = parse("cos(pi)").unwrap();
        assert_eq!(ast, call("cos", vec![var("pi")]));
    }

    // ===== Requirement 9: 无效表达式拒绝 =====

    #[test]
    fn test_empty_string_rejected() {
        let err = parse("").unwrap_err();
        assert!(matches!(err, CalcError::ParseError(msg) if msg.contains("empty")));
    }

    #[test]
    fn test_unmatched_parens_rejected() {
        let err1 = parse("(2+3").unwrap_err();
        assert!(matches!(err1, CalcError::ParseError(_)));

        let err2 = parse("2+3)").unwrap_err();
        assert!(matches!(err2, CalcError::ParseError(_)));
    }

    #[test]
    fn test_consecutive_operators_rejected() {
        // mathexpr 将 `+3` 当作数字字面量，需由 CalNexus 预处理层显式拒绝 `++`
        let err = parse("2++3").unwrap_err();
        assert!(matches!(err, CalcError::ParseError(msg) if msg.contains("consecutive operators")));
    }

    #[test]
    fn test_unclosed_function_rejected() {
        let err = parse("sin(").unwrap_err();
        assert!(matches!(err, CalcError::ParseError(_)));
    }

    #[test]
    fn test_operator_only_rejected() {
        assert!(parse("+").is_err());
        assert!(parse("*").is_err());
    }

    // ===== Requirement 10: AST 深度限制 =====

    #[test]
    fn test_depth_at_limit_passes() {
        // 256 个 1 用 + 连接 → 左结合 AST 深度 = 256
        let expr = format!("1{}", "+1".repeat(255));
        assert!(parse(&expr).is_ok());
    }

    #[test]
    fn test_depth_exceeds_limit_rejected() {
        // 257 个 1 用 + 连接 → AST 深度 = 257
        let expr = format!("1{}", "+1".repeat(256));
        let err = parse(&expr).unwrap_err();
        assert_eq!(err, CalcError::DepthExceeded);
    }

    #[test]
    fn test_depth_check_at_parse_time() {
        let deep_expr = format!("1{}", "+1".repeat(256));
        assert!(matches!(parse(&deep_expr), Err(CalcError::DepthExceeded)));
    }

    // ===== Requirement 11: 表达式长度限制 =====

    #[test]
    fn test_length_at_limit_passes() {
        // 构造恰好 4096 字符的合法表达式（长变量名 + 1，AST 深度 = 2）
        let var_name = "a".repeat(4094);
        let expr = format!("{}+1", var_name);
        assert_eq!(expr.len(), MAX_EXPR_LEN);
        assert!(parse(&expr).is_ok());
    }

    #[test]
    fn test_length_exceeds_limit_rejected() {
        // 4097 字符
        let mut expr = String::from("a");
        while expr.len() <= MAX_EXPR_LEN {
            expr.push_str("+1");
        }
        assert!(expr.len() > MAX_EXPR_LEN);
        let err = parse(&expr).unwrap_err();
        assert!(matches!(err, CalcError::ParseError(msg) if msg.contains("length")));
    }

    #[test]
    fn test_oversized_input_fast_fail() {
        let expr = "a".repeat(100_000);
        let err = parse(&expr).unwrap_err();
        assert!(matches!(err, CalcError::ParseError(_)));
    }

    // ===== 额外边界测试 =====

    #[test]
    fn test_whitespace_trimmed() {
        let ast = parse("  2+3  ").unwrap();
        assert_eq!(ast, binop(BinaryOp::Add, num(2.0), num(3.0)));
    }

    #[test]
    fn test_decimal_numbers() {
        let ast = parse("3.14").unwrap();
        assert_eq!(ast, num(3.14));
    }

    #[test]
    fn test_double_factorial() {
        // 5!! → factorial(factorial(5))
        let ast = parse("5!!").unwrap();
        assert_eq!(ast, call("factorial", vec![call("factorial", vec![num(5.0)])]));
    }

    #[test]
    fn test_factorial_on_variable() {
        let ast = parse("x!").unwrap();
        assert_eq!(ast, call("factorial", vec![var("x")]));
    }

    #[test]
    fn test_factorial_on_function_call() {
        let ast = parse("sin(x)!").unwrap();
        assert_eq!(ast, call("factorial", vec![call("sin", vec![var("x")])]));
    }

    #[test]
    fn test_factorial_in_expression() {
        // "2*3!" → Mul(2, factorial(3))
        let ast = parse("2*3!").unwrap();
        assert_eq!(
            ast,
            binop(BinaryOp::Mul, num(2.0), call("factorial", vec![num(3.0)]))
        );
    }

    // ===== proptest 属性测试（任务 2.5） =====

    use proptest::prelude::*;

    /// 生成简单合法表达式："数字 op 数字"
    fn valid_expr_strategy() -> impl Strategy<Value = String> {
        let num = (1u32..10000).prop_map(|n| n.to_string());
        let op = prop_oneof![Just("+"), Just("-"), Just("*"), Just("/")];
        (num.clone(), op, num).prop_map(|(a, op, b)| format!("{}{}{}", a, op, b))
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

        // 属性 1：解析幂等性 — 同一表达式多次解析结果相同
        #[test]
        fn prop_parse_idempotent(expr in valid_expr_strategy()) {
            let first = parse(&expr);
            let second = parse(&expr);
            prop_assert_eq!(first, second);
        }

        // 属性 2：合法字符集生成的表达式可解析
        #[test]
        fn prop_valid_expr_parseable(expr in valid_expr_strategy()) {
            let result = parse(&expr);
            prop_assert!(result.is_ok(), "expected Ok for valid expr {:?}, got {:?}", expr, result);
        }

        // 属性 3：深度限制 — N 个 1 用 + 连接，AST 深度 = N
        #[test]
        fn prop_depth_limit(n in 1usize..300) {
            let expr = format!("1{}", "+1".repeat(n - 1));
            let result = parse(&expr);
            if n <= 256 {
                prop_assert!(result.is_ok(), "depth {} should pass, got {:?}", n, result);
            } else {
                prop_assert!(matches!(result, Err(CalcError::DepthExceeded)),
                    "depth {} should fail, got {:?}", n, result);
            }
        }
    }
}
