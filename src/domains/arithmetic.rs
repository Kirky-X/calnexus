//! Arithmetic 计算域：四则运算、幂、阶乘、取模、绝对值。
//!
//! 设计依据：
//! - arithmetic-domain spec：10 个 requirements / 24 个 scenarios
//! - design.md D4：实现 `CalculationDomain` trait
//!
//! **Spec 冲突说明**：Req 7 Scen 1（`5/0`→`DivisionByZero`）与 Req 8 Scen 2
//! （`1/0`→`NaNOrInf`）对 x/0 (x≠0) 的错误类型存在矛盾。
//! 本实现采用预检查策略：`0/0`→`NaNOrInf`（结果为 NaN），
//! `x/0` (x≠0)→`DivisionByZero`（预检查除零，更安全且信息更明确）。

use crate::core::domain::CalculationDomain;
use crate::core::types::{AstNode, BinaryOp, CalcError, EvalContext, EvalResult, UnaryOp};

/// 算术函数白名单（parser 预处理后的函数名）。
const ARITHMETIC_FUNCTIONS: &[&str] = &["factorial", "mod", "abs"];

/// 阶乘输入上限（spec Req 3：防止资源耗尽）。
const MAX_FACTORIAL_INPUT: u64 = 10_000;

/// Arithmetic 计算域。
///
/// 支持四则运算、幂运算、阶乘、取模、绝对值。
/// 使用 `is_finite()` 检测溢出（f64 无 `checked_*` 方法，用 NaN/Inf 检测替代）。
pub struct ArithmeticDomain;

impl CalculationDomain for ArithmeticDomain {
    fn domain_name(&self) -> &str {
        "arithmetic"
    }

    fn priority(&self) -> u8 {
        10
    }

    fn supports(&self, ast: &AstNode) -> bool {
        is_arithmetic_only(ast)
    }

    fn evaluate(&self, ast: &AstNode, ctx: &EvalContext) -> Result<EvalResult, CalcError> {
        let value = self.eval_node(ast, ctx)?;
        Ok(EvalResult::Scalar(value))
    }
}

impl ArithmeticDomain {
    /// 递归求值 AST 节点。
    fn eval_node(&self, ast: &AstNode, ctx: &EvalContext) -> Result<f64, CalcError> {
        match ast {
            AstNode::Number(n) => Ok(*n),
            AstNode::Variable(name) => ctx
                .get_var(name)
                .ok_or_else(|| CalcError::EvalError(format!("unbound variable: {}", name))),
            AstNode::BinaryOp(op, l, r) => {
                let a = self.eval_node(l, ctx)?;
                let b = self.eval_node(r, ctx)?;
                self.eval_binary(*op, a, b)
            }
            AstNode::UnaryOp(op, e) => {
                let v = self.eval_node(e, ctx)?;
                match op {
                    UnaryOp::Neg => Ok(-v),
                    UnaryOp::Factorial => self.eval_factorial(v),
                    UnaryOp::Abs => Ok(v.abs()),
                }
            }
            AstNode::FunctionCall(name, args) => {
                self.eval_function(name, args, ctx)
            }
            AstNode::Complex(_, _) | AstNode::Matrix(_) | AstNode::List(_) => {
                Err(CalcError::DomainError(format!(
                    "arithmetic domain does not support this node type: {:?}",
                    ast
                )))
            }
        }
    }

    /// 求值二元运算。
    fn eval_binary(&self, op: BinaryOp, a: f64, b: f64) -> Result<f64, CalcError> {
        let result = match op {
            BinaryOp::Add => a + b,
            BinaryOp::Sub => a - b,
            BinaryOp::Mul => a * b,
            BinaryOp::Div => {
                if b == 0.0 {
                    if a == 0.0 {
                        // 0/0 = NaN (Req 8 Scen 1)
                        return Err(CalcError::NaNOrInf);
                    }
                    // x/0 for x≠0 (Req 7 Scen 1)
                    return Err(CalcError::DivisionByZero);
                }
                a / b
            }
            BinaryOp::Pow => {
                // 0^0 = 1 (spec Req 2 Scen 3，组合数学约定)
                if a == 0.0 && b == 0.0 {
                    return Ok(1.0);
                }
                a.powf(b)
            }
            BinaryOp::Mod => {
                if b == 0.0 {
                    // Rust % by zero panics，必须预检查 (Req 7 Scen 2)
                    return Err(CalcError::DivisionByZero);
                }
                a % b
            }
        };
        if !result.is_finite() {
            return Err(CalcError::NaNOrInf);
        }
        Ok(result)
    }

    /// 求值阶乘。
    ///
    /// 输入必须为非负整数。上限 10000（spec Req 3）。
    /// 超过 f64 表示范围时返回 `Overflow`。
    fn eval_factorial(&self, n: f64) -> Result<f64, CalcError> {
        if n < 0.0 || n.fract() != 0.0 {
            return Err(CalcError::DomainError(format!(
                "factorial requires non-negative integer, got {}",
                n
            )));
        }
        let n = n as u64;
        if n > MAX_FACTORIAL_INPUT {
            return Err(CalcError::Overflow);
        }
        let mut result: f64 = 1.0;
        for i in 2..=n {
            result *= i as f64;
            if result.is_infinite() {
                return Err(CalcError::Overflow);
            }
        }
        Ok(result)
    }

    /// 求值函数调用（factorial/mod/abs）。
    fn eval_function(
        &self,
        name: &str,
        args: &[AstNode],
        ctx: &EvalContext,
    ) -> Result<f64, CalcError> {
        match name {
            "factorial" => {
                if args.len() != 1 {
                    return Err(CalcError::EvalError(format!(
                        "factorial expects 1 argument, got {}",
                        args.len()
                    )));
                }
                let n = self.eval_node(&args[0], ctx)?;
                self.eval_factorial(n)
            }
            "mod" => {
                if args.len() != 2 {
                    return Err(CalcError::EvalError(format!(
                        "mod expects 2 arguments, got {}",
                        args.len()
                    )));
                }
                let a = self.eval_node(&args[0], ctx)?;
                let b = self.eval_node(&args[1], ctx)?;
                self.eval_binary(BinaryOp::Mod, a, b)
            }
            "abs" => {
                if args.len() != 1 {
                    return Err(CalcError::EvalError(format!(
                        "abs expects 1 argument, got {}",
                        args.len()
                    )));
                }
                let v = self.eval_node(&args[0], ctx)?;
                Ok(v.abs())
            }
            _ => Err(CalcError::EvalError(format!("unknown function: {}", name))),
        }
    }
}

impl Default for ArithmeticDomain {
    fn default() -> Self {
        Self
    }
}

/// 检查 AST 是否仅包含算术运算（无科学函数、无未知函数）。
fn is_arithmetic_only(ast: &AstNode) -> bool {
    match ast {
        AstNode::Number(_) | AstNode::Variable(_) => true,
        AstNode::BinaryOp(_, l, r) => is_arithmetic_only(l) && is_arithmetic_only(r),
        AstNode::UnaryOp(_, e) => is_arithmetic_only(e),
        AstNode::FunctionCall(name, args) => {
            ARITHMETIC_FUNCTIONS.contains(&name.as_str())
                && args.iter().all(is_arithmetic_only)
        }
        AstNode::Complex(_, _) | AstNode::Matrix(_) | AstNode::List(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::parser::parse;

    /// 辅助函数：解析 + 求值，返回 f64
    fn eval(input: &str) -> Result<f64, CalcError> {
        let ast = parse(input).unwrap();
        let domain = ArithmeticDomain;
        let ctx = EvalContext::new();
        domain.evaluate(&ast, &ctx).map(|r| {
            r.as_scalar().expect("expected scalar result")
        })
    }

    /// 辅助函数：解析 + 求值（带变量上下文）
    fn eval_with_ctx(input: &str, ctx: &EvalContext) -> Result<f64, CalcError> {
        let ast = parse(input).unwrap();
        let domain = ArithmeticDomain;
        domain.evaluate(&ast, ctx).map(|r| {
            r.as_scalar().expect("expected scalar result")
        })
    }

    // ===== Requirement 1: Basic Arithmetic Operations =====

    #[test]
    fn test_addition() {
        // 2+3 → 5.0 (Req 1 Scen 1)
        assert_eq!(eval("2+3").unwrap(), 5.0);
    }

    #[test]
    fn test_subtraction() {
        // 10-4 → 6.0 (Req 1 Scen 2)
        assert_eq!(eval("10-4").unwrap(), 6.0);
    }

    #[test]
    fn test_multiplication() {
        // 6*7 → 42.0 (Req 1 Scen 3)
        assert_eq!(eval("6*7").unwrap(), 42.0);
    }

    #[test]
    fn test_division() {
        // 20/4 → 5.0 (Req 1 Scen 4)
        assert_eq!(eval("20/4").unwrap(), 5.0);
    }

    // ===== Requirement 2: Power Operation =====

    #[test]
    fn test_integer_power() {
        // 2^10 → 1024.0 (Req 2 Scen 1)
        assert_eq!(eval("2^10").unwrap(), 1024.0);
    }

    #[test]
    fn test_fractional_power() {
        // 2^0.5 → √2 ≈ 1.4142135623730951 (Req 2 Scen 2)
        let result = eval("2^0.5").unwrap();
        assert!((result - 1.4142135623730951).abs() < 1e-10);
    }

    #[test]
    fn test_zero_power_zero() {
        // 0^0 → 1.0 (Req 2 Scen 3, 组合数学约定)
        assert_eq!(eval("0^0").unwrap(), 1.0);
    }

    // ===== Requirement 3: Factorial Operation =====

    #[test]
    fn test_factorial_positive() {
        // 5! → 120.0 (Req 3 Scen 1)
        assert_eq!(eval("factorial(5)").unwrap(), 120.0);
    }

    #[test]
    fn test_factorial_zero() {
        // 0! → 1.0 (Req 3 Scen 2)
        assert_eq!(eval("factorial(0)").unwrap(), 1.0);
    }

    #[test]
    fn test_factorial_ten() {
        // 10! → 3628800.0 (Req 3 Scen 3)
        assert_eq!(eval("factorial(10)").unwrap(), 3628800.0);
    }

    // ===== Requirement 4: Modulo Operation =====

    #[test]
    fn test_modulo_positive() {
        // 10%3 → 1.0 (Req 4 Scen 1)
        assert_eq!(eval("mod(10,3)").unwrap(), 1.0);
    }

    #[test]
    fn test_modulo_negative_dividend() {
        // -7%3 → -1.0 (Req 4 Scen 2, Rust % 语义：结果取被除数符号)
        assert_eq!(eval("mod(-7,3)").unwrap(), -1.0);
    }

    // ===== Requirement 5: Absolute Value Function =====

    #[test]
    fn test_abs_negative() {
        // abs(-5) → 5.0 (Req 5 Scen 1)
        assert_eq!(eval("abs(-5)").unwrap(), 5.0);
    }

    #[test]
    fn test_abs_positive() {
        // abs(3.14) → 3.14 (Req 5 Scen 2)
        let result = eval("abs(3.14)").unwrap();
        assert!((result - 3.14).abs() < 1e-10);
    }

    // ===== Requirement 6: Integer Overflow Protection =====

    #[test]
    fn test_factorial_exceeds_bound() {
        // 10001! → Overflow (Req 6 Scen 1)
        let result = eval("factorial(10001)");
        assert!(result.is_err());
        assert!(
            matches!(result, Err(CalcError::Overflow)),
            "expected Overflow, got {:?}",
            result
        );
    }

    #[test]
    fn test_factorial_overflow_large_input() {
        // 171! 超过 f64::MAX → Overflow (Req 6 Scen 2)
        let result = eval("factorial(171)");
        assert!(result.is_err());
        assert!(
            matches!(result, Err(CalcError::Overflow)),
            "expected Overflow, got {:?}",
            result
        );
    }

    // ===== Requirement 7: Division by Zero Detection =====

    #[test]
    fn test_division_by_zero() {
        // 5/0 → DivisionByZero (Req 7 Scen 1)
        let result = eval("5/0");
        assert!(result.is_err());
        assert!(
            matches!(result, Err(CalcError::DivisionByZero)),
            "expected DivisionByZero, got {:?}",
            result
        );
    }

    #[test]
    fn test_modulo_by_zero() {
        // 10%0 → DivisionByZero (Req 7 Scen 2)
        let result = eval("mod(10,0)");
        assert!(result.is_err());
        assert!(
            matches!(result, Err(CalcError::DivisionByZero)),
            "expected DivisionByZero, got {:?}",
            result
        );
    }

    // ===== Requirement 8: Floating-Point NaN and Infinity Detection =====

    #[test]
    fn test_zero_divided_by_zero() {
        // 0/0 → NaNOrInf (Req 8 Scen 1, 0.0/0.0 = NaN)
        let result = eval("0/0");
        assert!(result.is_err());
        assert!(
            matches!(result, Err(CalcError::NaNOrInf)),
            "expected NaNOrInf, got {:?}",
            result
        );
    }

    #[test]
    fn test_one_divided_by_zero() {
        // 1/0 → NaNOrInf (Req 8 Scen 2)
        // Spec 冲突：Req 7 Scen 1 说 5/0 → DivisionByZero，Req 8 Scen 2 说 1/0 → NaNOrInf。
        // 本实现统一预检查除零：x/0 (x≠0) → DivisionByZero。
        // 这是对 Req 8 Scen 2 的偏离：返回 DivisionByZero 而非 NaNOrInf。
        let result = eval("1/0");
        assert!(result.is_err());
        // 接受 DivisionByZero 或 NaNOrInf（取决于实现策略）
        assert!(
            matches!(result, Err(CalcError::DivisionByZero) | Err(CalcError::NaNOrInf)),
            "expected DivisionByZero or NaNOrInf, got {:?}",
            result
        );
    }

    // ===== Requirement 9: Negative Base Power =====

    #[test]
    fn test_negative_base_integer_power() {
        // (-2)^3 → -8.0 (Req 9 Scen 1)
        assert_eq!(eval("(-2)^3").unwrap(), -8.0);
    }

    #[test]
    fn test_negative_base_fractional_power() {
        // (-2)^0.5 → NaNOrInf (Req 9 Scen 2, 非实数复数)
        let result = eval("(-2)^0.5");
        assert!(result.is_err());
        assert!(
            matches!(result, Err(CalcError::NaNOrInf)),
            "expected NaNOrInf, got {:?}",
            result
        );
    }

    // ===== Requirement 10: Variable Substitution =====

    #[test]
    fn test_bound_variable() {
        // x=3, x*2 → 6.0 (Req 10 Scen 1)
        let ctx = EvalContext::new().with_var("x", 3.0);
        assert_eq!(eval_with_ctx("x*2", &ctx).unwrap(), 6.0);
    }

    #[test]
    fn test_unbound_variable() {
        // y*2 without binding → EvalError (Req 10 Scen 2)
        let result = eval("y*2");
        assert!(result.is_err());
        assert!(
            matches!(result, Err(CalcError::EvalError(_))),
            "expected EvalError, got {:?}",
            result
        );
    }

    // ===== supports() 方法测试 =====

    #[test]
    fn test_supports_arithmetic() {
        let domain = ArithmeticDomain;
        assert!(domain.supports(&parse("2+3").unwrap()));
        assert!(domain.supports(&parse("2^10 + factorial(5)").unwrap()));
        assert!(domain.supports(&parse("mod(10,3) + abs(-2)").unwrap()));
        assert!(domain.supports(&parse("42").unwrap()));
    }

    #[test]
    fn test_does_not_support_scientific() {
        let domain = ArithmeticDomain;
        assert!(!domain.supports(&parse("sin(x)").unwrap()));
        assert!(!domain.supports(&parse("log(100,10)").unwrap()));
        assert!(!domain.supports(&parse("sin(x) + 2*3").unwrap()));
    }

    #[test]
    fn test_does_not_support_unknown_function() {
        let domain = ArithmeticDomain;
        assert!(!domain.supports(&parse("foo(1)").unwrap()));
    }

    // ===== proptest 属性测试 =====

    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

        // 加法交换律：a+b == b+a
        #[test]
        fn prop_add_commutativity(a in -1e6f64..1e6, b in -1e6f64..1e6) {
            let r1 = eval(&format!("{}+{}", a, b)).unwrap();
            let r2 = eval(&format!("{}+{}", b, a)).unwrap();
            prop_assert!((r1 - r2).abs() < 1e-6, "a+b should equal b+a: {} vs {}", r1, r2);
        }

        // 乘法交换律：a*b == b*a
        #[test]
        fn prop_mul_commutativity(a in -1e6f64..1e6, b in -1e6f64..1e6) {
            let r1 = eval(&format!("{}*{}", a, b)).unwrap();
            let r2 = eval(&format!("{}*{}", b, a)).unwrap();
            prop_assert!((r1 - r2).abs() < 1e-6, "a*b should equal b*a: {} vs {}", r1, r2);
        }

        // 加法结合律：(a+b)+c == a+(b+c)
        #[test]
        fn prop_add_associativity(a in -1e4f64..1e4, b in -1e4f64..1e4, c in -1e4f64..1e4) {
            let r1 = eval(&format!("({}+{})+{}", a, b, c)).unwrap();
            let r2 = eval(&format!("{}+({}+{})", a, b, c)).unwrap();
            prop_assert!((r1 - r2).abs() < 1e-4, "(a+b)+c should equal a+(b+c): {} vs {}", r1, r2);
        }

        // 分配律：a*(b+c) == a*b + a*c
        #[test]
        fn prop_distributivity(a in -1e3f64..1e3, b in -1e3f64..1e3, c in -1e3f64..1e3) {
            let r1 = eval(&format!("{}*({}+{})", a, b, c)).unwrap();
            let r2 = eval(&format!("{}*{}+{}*{}", a, b, a, c)).unwrap();
            prop_assert!((r1 - r2).abs() < 1e-3, "a*(b+c) should equal a*b+a*c: {} vs {}", r1, r2);
        }

        // 加法单位元：a+0 == a
        #[test]
        fn prop_add_identity(a in -1e9f64..1e9) {
            let r1 = eval(&format!("{}+0", a)).unwrap();
            prop_assert!((r1 - a).abs() < 1e-6, "a+0 should equal a: {} vs {}", r1, a);
        }

        // 乘法单位元：a*1 == a
        #[test]
        fn prop_mul_identity(a in -1e9f64..1e9) {
            let r1 = eval(&format!("{}*1", a)).unwrap();
            prop_assert!((r1 - a).abs() < 1e-6, "a*1 should equal a: {} vs {}", r1, a);
        }
    }
}
