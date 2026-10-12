// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! Combinatorics 计算域：排列 P、组合 C、Catalan 数、Stirling 数。
//!
//! 含组合函数调用时路由至本域；u128 累积，溢出自动升级 BigInt，priority=25。

use super::common::{unsupported_function_error, unsupported_node_error};
use crate::core::CalculationDomain;
use crate::core::{
    AstNode, BinaryOp, CalcError, EvalContext, EvalResult, MAX_POW_EXPONENT, UnaryOp,
    check_pow_output_size,
};
use num_bigint::BigInt;
use num_traits::{Signed, ToPrimitive, Zero};

/// 组合函数白名单。
pub(crate) const COMBINATORICS_FUNCTIONS: &[&str] = &["P", "C", "catalan", "stirling"];

/// Combinatorics 计算域：priority=25，支持 P/C/catalan/stirling。
pub struct CombinatoricsDomain;

impl CalculationDomain for CombinatoricsDomain {
    fn domain_name(&self) -> &str {
        "combinatorics"
    }

    fn priority(&self) -> u8 {
        25
    }

    fn supports(&self, ast: &AstNode) -> bool {
        contains_combinatorics_function(ast)
    }

    fn evaluate(&self, ast: &AstNode, ctx: &EvalContext) -> Result<EvalResult, CalcError> {
        self.eval_node(ast, ctx)
    }
}

impl Default for CombinatoricsDomain {
    fn default() -> Self {
        Self
    }
}

impl CombinatoricsDomain {
    /// 递归求值 AST 节点，返回 EvalResult。
    fn eval_node(&self, ast: &AstNode, ctx: &EvalContext) -> Result<EvalResult, CalcError> {
        match ast {
            AstNode::FunctionCall(name, args) => self.eval_function(name, args, ctx),
            AstNode::Number(n) => {
                if n.fract() != 0.0 {
                    return Err(CalcError::domain(format!(
                        "combinatorics domain requires integer, got {}",
                        n
                    ))
                    .with_i18n(
                        "msg.combinatorics.requires_integer",
                        vec![("value".to_string(), n.to_string())],
                    ));
                }
                Ok(EvalResult::Scalar(*n))
            }
            AstNode::BigNumber(s) => {
                let b: BigInt = s.parse().map_err(|_| {
                    CalcError::domain(format!("invalid big number: {}", s)).with_i18n(
                        "msg.invalid_bignumber",
                        vec![("value".to_string(), s.to_string())],
                    )
                })?;
                Ok(EvalResult::BigInt(b))
            }
            AstNode::Variable(name) => ctx.get_var(name).map(EvalResult::Scalar).ok_or_else(|| {
                CalcError::eval(format!("unbound variable: {}", name)).with_i18n(
                    "msg.unbound_variable",
                    vec![("name".to_string(), name.to_string())],
                )
            }),
            AstNode::BinaryOp(op, l, r) => {
                let a = self.eval_int(l, ctx)?;
                let b = self.eval_int(r, ctx)?;
                let result = self.eval_int_binary(*op, a, b)?;
                Ok(bigint_to_result(result))
            }
            AstNode::UnaryOp(op, e) => {
                let v = self.eval_int(e, ctx)?;
                match op {
                    UnaryOp::Neg => Ok(bigint_to_result(-v)),
                    UnaryOp::Abs => Ok(bigint_to_result(v.abs())),
                    UnaryOp::Factorial => Err(CalcError::domain(
                        "factorial not supported in combinatorics domain".to_string(),
                    )
                    .with_i18n(
                        "msg.domain.factorial_not_supported",
                        vec![("domain".to_string(), "combinatorics".to_string())],
                    )),
                }
            }
            AstNode::Complex(_, _) | AstNode::Matrix(_) | AstNode::List(_) | AstNode::Str(_) => {
                Err(unsupported_node_error("combinatorics", ast))
            }
        }
    }

    /// 将 AST 求值为 BigInt（精确整数运算）。
    fn eval_int(&self, ast: &AstNode, ctx: &EvalContext) -> Result<BigInt, CalcError> {
        match ast {
            AstNode::Number(n) => {
                if n.fract() != 0.0 {
                    return Err(
                        CalcError::domain(format!("expected integer argument, got {}", n))
                            .with_i18n(
                                "msg.core.expected_integer",
                                vec![("value".to_string(), n.to_string())],
                            ),
                    );
                }
                if *n > i64::MAX as f64 || *n < i64::MIN as f64 {
                    return Err(CalcError::overflow());
                }
                Ok(BigInt::from(*n as i64))
            }
            AstNode::BigNumber(s) => s.parse::<BigInt>().map_err(|_| {
                CalcError::domain(format!("invalid big number: {}", s)).with_i18n(
                    "msg.invalid_bignumber",
                    vec![("value".to_string(), s.to_string())],
                )
            }),
            AstNode::Variable(name) => {
                let v = ctx.get_var(name).ok_or_else(|| {
                    CalcError::eval(format!("unbound variable: {}", name)).with_i18n(
                        "msg.unbound_variable",
                        vec![("name".to_string(), name.to_string())],
                    )
                })?;
                if v.fract() != 0.0 {
                    return Err(CalcError::domain(format!(
                        "variable {} is not an integer: {}",
                        name, v
                    ))
                    .with_i18n(
                        "msg.core.variable_not_integer",
                        vec![
                            ("name".to_string(), name.to_string()),
                            ("value".to_string(), v.to_string()),
                        ],
                    ));
                }
                if v > i64::MAX as f64 || v < i64::MIN as f64 {
                    return Err(CalcError::overflow());
                }
                Ok(BigInt::from(v as i64))
            }
            AstNode::BinaryOp(op, l, r) => {
                let a = self.eval_int(l, ctx)?;
                let b = self.eval_int(r, ctx)?;
                self.eval_int_binary(*op, a, b)
            }
            AstNode::UnaryOp(op, e) => {
                let v = self.eval_int(e, ctx)?;
                match op {
                    UnaryOp::Neg => Ok(-v),
                    UnaryOp::Abs => Ok(v.abs()),
                    UnaryOp::Factorial => Err(CalcError::domain(
                        "factorial not supported in combinatorics domain".to_string(),
                    )
                    .with_i18n(
                        "msg.domain.factorial_not_supported",
                        vec![("domain".to_string(), "combinatorics".to_string())],
                    )),
                }
            }
            AstNode::Complex(_, _)
            | AstNode::Matrix(_)
            | AstNode::List(_)
            | AstNode::FunctionCall(_, _)
            | AstNode::Str(_) => Err(CalcError::domain(format!(
                "expected integer expression, got: {:?}",
                ast
            ))
            .with_i18n(
                "msg.core.expected_integer_expression",
                vec![("got".to_string(), format!("{:?}", ast))],
            )),
        }
    }

    /// 整数二元运算。
    fn eval_int_binary(&self, op: BinaryOp, a: BigInt, b: BigInt) -> Result<BigInt, CalcError> {
        match op {
            BinaryOp::Add => Ok(a + b),
            BinaryOp::Sub => Ok(a - b),
            BinaryOp::Mul => Ok(a * b),
            BinaryOp::Div => {
                if b.is_zero() {
                    return Err(CalcError::division_by_zero());
                }
                Ok(a / b)
            }
            BinaryOp::Pow => {
                if b.is_negative() {
                    return Err(CalcError::domain(
                        "negative exponent not supported for integers".to_string(),
                    )
                    .with_i18n("msg.core.negative_exponent", vec![]));
                }
                // 安全约束：拒绝超大指数，防止绕过 precision.rs 防护触发 OOM。
                let exp_u64 = b.to_u64().ok_or(CalcError::overflow())?;
                if exp_u64 > MAX_POW_EXPONENT {
                    return Err(CalcError::domain(format!(
                        "power exponent must not exceed {} (got {})",
                        MAX_POW_EXPONENT, exp_u64
                    ))
                    .with_i18n(
                        "msg.core.power_exponent_exceeds",
                        vec![
                            ("max".to_string(), MAX_POW_EXPONENT.to_string()),
                            ("actual".to_string(), exp_u64.to_string()),
                        ],
                    ));
                }
                // 安全约束：限制大底数 × 大指数输出，复用 core 层 check_pow_output_size。
                let base_bits = a.bits();
                check_pow_output_size(base_bits, exp_u64)?;
                let exp: u32 = exp_u64.try_into().map_err(|_| CalcError::overflow())?;
                Ok(a.pow(exp))
            }
            BinaryOp::Mod => {
                if b.is_zero() {
                    return Err(CalcError::division_by_zero());
                }
                Ok(a % b)
            }
        }
    }

    /// 求值组合函数调用：白名单检查后按函数名分发。
    fn eval_function(
        &self,
        name: &str,
        args: &[AstNode],
        ctx: &EvalContext,
    ) -> Result<EvalResult, CalcError> {
        if !COMBINATORICS_FUNCTIONS.contains(&name) {
            return Err(unsupported_function_error("combinatorics", name));
        }
        match name {
            "P" => self.eval_permutation(args, ctx),
            "C" => self.eval_combination(args, ctx),
            "catalan" => self.eval_catalan(args, ctx),
            "stirling" => self.eval_stirling(args, ctx),
            _ => Err(
                CalcError::eval(format!("unknown combinatorics function: {}", name)).with_i18n(
                    "msg.unknown_function",
                    vec![("name".to_string(), name.to_string())],
                ),
            ),
        }
    }

    /// 求值两个非负整数参数（P/C/stirling 共用）：数量为 2，n/k 非负。
    fn eval_two_non_negative_args(
        &self,
        args: &[AstNode],
        ctx: &EvalContext,
        name: &str,
    ) -> Result<(BigInt, BigInt), CalcError> {
        if args.len() != 2 {
            return Err(CalcError::domain(format!(
                "{}() requires exactly 2 arguments, got {}",
                name,
                args.len()
            ))
            .with_i18n(
                "msg.combinatorics.arg_count_2",
                vec![
                    ("name".to_string(), name.to_string()),
                    ("actual".to_string(), args.len().to_string()),
                ],
            ));
        }
        let n = self.eval_int(&args[0], ctx)?;
        let k = self.eval_int(&args[1], ctx)?;
        if n.is_negative() || k.is_negative() {
            return Err(
                CalcError::domain(format!("{}() requires non-negative arguments", name)).with_i18n(
                    "msg.combinatorics.requires_non_negative",
                    vec![("name".to_string(), name.to_string())],
                ),
            );
        }
        Ok((n, k))
    }

    /// 求值单个非负整数参数（catalan 共用）：数量为 1，n 非负。
    fn eval_one_non_negative_arg(
        &self,
        args: &[AstNode],
        ctx: &EvalContext,
        name: &str,
    ) -> Result<BigInt, CalcError> {
        if args.len() != 1 {
            return Err(CalcError::domain(format!(
                "{}() requires exactly 1 argument, got {}",
                name,
                args.len()
            ))
            .with_i18n(
                "msg.combinatorics.arg_count_1",
                vec![
                    ("name".to_string(), name.to_string()),
                    ("actual".to_string(), args.len().to_string()),
                ],
            ));
        }
        let n = self.eval_int(&args[0], ctx)?;
        if n.is_negative() {
            return Err(
                CalcError::domain(format!("{}() requires non-negative argument", name)).with_i18n(
                    "msg.combinatorics.requires_non_negative_arg",
                    vec![("name".to_string(), name.to_string())],
                ),
            );
        }
        Ok(n)
    }

    /// P(n,k) 排列数（委托给 `math::combinatorics::perm`）。
    fn eval_permutation(
        &self,
        args: &[AstNode],
        ctx: &EvalContext,
    ) -> Result<EvalResult, CalcError> {
        let (n, k) = self.eval_two_non_negative_args(args, ctx, "P")?;
        if k > n {
            return Ok(EvalResult::Scalar(0.0));
        }
        let result = crate::math::combinatorics::perm(&n, &k)?;
        Ok(bigint_to_result(result))
    }

    /// C(n,k) 组合数（委托给 `math::combinatorics::comb`）。
    fn eval_combination(
        &self,
        args: &[AstNode],
        ctx: &EvalContext,
    ) -> Result<EvalResult, CalcError> {
        let (n, k) = self.eval_two_non_negative_args(args, ctx, "C")?;
        if k > n {
            return Ok(EvalResult::Scalar(0.0));
        }
        let result = crate::math::combinatorics::comb(&n, &k)?;
        Ok(bigint_to_result(result))
    }

    /// catalan(n) Catalan 数（委托给 `math::combinatorics::catalan`）。
    fn eval_catalan(&self, args: &[AstNode], ctx: &EvalContext) -> Result<EvalResult, CalcError> {
        let n = self.eval_one_non_negative_arg(args, ctx, "catalan")?;
        let result = crate::math::combinatorics::catalan(&n)?;
        Ok(bigint_to_result(result))
    }

    /// stirling(n,k) 第二类 Stirling 数（委托给 `math::combinatorics::stirling_second`）。
    fn eval_stirling(&self, args: &[AstNode], ctx: &EvalContext) -> Result<EvalResult, CalcError> {
        let (n, k) = self.eval_two_non_negative_args(args, ctx, "stirling")?;
        let result = crate::math::combinatorics::stirling_second(&n, &k)?;
        Ok(bigint_to_result(result))
    }
}

/// 将 BigInt 转换为 EvalResult：fit i64 → Scalar，否则 → BigInt。
fn bigint_to_result(b: BigInt) -> EvalResult {
    if let Some(n) = b.to_i64() {
        EvalResult::Scalar(n as f64)
    } else {
        EvalResult::BigInt(b)
    }
}

/// 递归检查 AST 是否含组合函数调用。
fn contains_combinatorics_function(ast: &AstNode) -> bool {
    match ast {
        AstNode::FunctionCall(name, _) if COMBINATORICS_FUNCTIONS.contains(&name.as_str()) => true,
        AstNode::FunctionCall(_, args) => args.iter().any(contains_combinatorics_function),
        AstNode::BinaryOp(_, l, r) => {
            contains_combinatorics_function(l) || contains_combinatorics_function(r)
        }
        AstNode::UnaryOp(_, e) => contains_combinatorics_function(e),
        AstNode::Matrix(rows) => rows.iter().flatten().any(contains_combinatorics_function),
        AstNode::List(elements) => elements.iter().any(contains_combinatorics_function),
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
        let domain = CombinatoricsDomain;
        let ctx = EvalContext::new();
        domain.evaluate(&ast, &ctx)
    }

    fn eval_scalar(input: &str) -> Result<f64, CalcError> {
        eval(input).map(|r| r.as_scalar().expect("expected scalar result"))
    }

    // ===== P(n,k) =====

    #[test]
    fn test_permutation_basic() {
        assert_eq!(eval_scalar("P(5,2)").unwrap(), 20.0);
    }

    // ===== C(n,k) =====

    #[test]
    fn test_combination_basic() {
        assert_eq!(eval_scalar("C(10,3)").unwrap(), 120.0);
    }

    // ===== Catalan =====

    #[test]
    fn test_catalan_basic() {
        assert_eq!(eval_scalar("catalan(5)").unwrap(), 42.0);
    }

    // ===== Stirling =====

    #[test]
    fn test_stirling_basic() {
        assert_eq!(eval_scalar("stirling(5,2)").unwrap(), 15.0);
    }

    // ===== C(n,0) =====

    #[test]
    fn test_combination_zero_k() {
        assert_eq!(eval_scalar("C(5,0)").unwrap(), 1.0);
    }

    // ===== C(n,n) =====

    #[test]
    fn test_combination_n_n() {
        assert_eq!(eval_scalar("C(5,5)").unwrap(), 1.0);
    }

    // ===== k>n =====

    #[test]
    fn test_combination_k_greater_than_n() {
        assert_eq!(eval_scalar("C(3,5)").unwrap(), 0.0);
    }

    // ===== n<0 =====

    #[test]
    fn test_combination_negative_n() {
        let result = eval("C(-1,2)");
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    // ===== k<0 =====

    #[test]
    fn test_combination_negative_k() {
        let result = eval("C(5,-1)");
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    // ===== 大数 C =====

    #[test]
    fn test_combination_large() {
        // C(100,50) ≈ 10^29，超 u64 但在 u128 内
        let result = eval("C(100,50)").unwrap();
        match result {
            EvalResult::BigInt(b) => {
                let expected: BigInt = "100891344545564193334812497256".parse().unwrap();
                assert_eq!(b, expected);
            }
            EvalResult::Scalar(v) => panic!("expected BigInt for C(100,50), got Scalar({})", v),
            _ => panic!("unexpected result type"),
        }
    }

    // ===== 大数 P =====

    #[test]
    fn test_permutation_large() {
        let result = eval_scalar("P(20,10)").unwrap();
        assert_eq!(result, 670442572800.0);
    }

    // ===== Catalan 边界 =====

    #[test]
    fn test_catalan_zero() {
        assert_eq!(eval_scalar("catalan(0)").unwrap(), 1.0);
    }

    // ===== Catalan 大数 =====

    #[test]
    fn test_catalan_large() {
        let result = eval("catalan(30)").unwrap();
        match result {
            EvalResult::BigInt(b) => {
                let expected: BigInt = "3814986502092304".parse().unwrap();
                assert_eq!(b, expected);
            }
            EvalResult::Scalar(v) => {
                // 可能 fit i64
                assert_eq!(v, 3814986502092304.0);
            }
            _ => panic!("unexpected result type"),
        }
    }

    // ===== Stirling 边界 =====

    #[test]
    fn test_stirling_zero_zero() {
        assert_eq!(eval_scalar("stirling(0,0)").unwrap(), 1.0);
    }

    // ===== 溢出处理 =====

    #[test]
    fn test_permutation_overflow_to_bigint() {
        // P(1000,500) 会溢出 u128，升级为 BigInt
        let result = eval("P(1000,500)");
        assert!(result.is_ok());
        match result.unwrap() {
            EvalResult::BigInt(_) => {} // 预期升级
            EvalResult::Scalar(_) => {} // 也可能 fit（不太可能但接受）
            _ => panic!("unexpected result type"),
        }
    }

    // ===== 补充边界测试 =====

    #[test]
    fn test_permutation_k_zero() {
        assert_eq!(eval_scalar("P(5,0)").unwrap(), 1.0);
    }

    #[test]
    fn test_permutation_n_equals_k() {
        assert_eq!(eval_scalar("P(5,5)").unwrap(), 120.0);
    }

    #[test]
    fn test_permutation_k_greater_than_n() {
        assert_eq!(eval_scalar("P(3,5)").unwrap(), 0.0);
    }

    #[test]
    fn test_permutation_negative() {
        assert!(matches!(eval("P(-1,2)"), Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_combination_symmetry() {
        assert_eq!(
            eval_scalar("C(10,3)").unwrap(),
            eval_scalar("C(10,7)").unwrap()
        );
    }

    #[test]
    fn test_catalan_sequence() {
        assert_eq!(eval_scalar("catalan(1)").unwrap(), 1.0);
        assert_eq!(eval_scalar("catalan(2)").unwrap(), 2.0);
        assert_eq!(eval_scalar("catalan(3)").unwrap(), 5.0);
        assert_eq!(eval_scalar("catalan(4)").unwrap(), 14.0);
        assert_eq!(eval_scalar("catalan(6)").unwrap(), 132.0);
    }

    #[test]
    fn test_catalan_negative() {
        assert!(matches!(
            eval("catalan(-1)"),
            Err(e) if e.kind == ErrorKind::Domain
        ));
    }

    #[test]
    fn test_stirling_known_values() {
        assert_eq!(eval_scalar("stirling(3,2)").unwrap(), 3.0);
        assert_eq!(eval_scalar("stirling(4,2)").unwrap(), 7.0);
        assert_eq!(eval_scalar("stirling(4,3)").unwrap(), 6.0);
    }

    #[test]
    fn test_stirling_k_greater_than_n() {
        assert_eq!(eval_scalar("stirling(2,5)").unwrap(), 0.0);
    }

    #[test]
    fn test_stirling_n_zero_k_positive() {
        assert_eq!(eval_scalar("stirling(0,5)").unwrap(), 0.0);
    }

    #[test]
    fn test_stirling_n_positive_k_zero() {
        assert_eq!(eval_scalar("stirling(5,0)").unwrap(), 0.0);
    }

    #[test]
    fn test_stirling_negative() {
        assert!(matches!(
            eval("stirling(-1,2)"),
            Err(e) if e.kind == ErrorKind::Domain
        ));
    }

    // ===== 域元信息测试 =====

    #[test]
    fn test_domain_info() {
        let domain = CombinatoricsDomain;
        assert_eq!(domain.domain_name(), "combinatorics");
        assert_eq!(domain.priority(), 25);
    }

    #[test]
    fn test_default_impl() {
        let domain = CombinatoricsDomain;
        assert_eq!(domain.domain_name(), "combinatorics");
    }

    #[test]
    fn test_supports_P() {
        let ast = parse("P(5,2)").unwrap();
        assert!(CombinatoricsDomain.supports(&ast));
    }

    #[test]
    fn test_supports_nested() {
        let ast = parse("C(10,3) + 1").unwrap();
        assert!(CombinatoricsDomain.supports(&ast));
    }

    #[test]
    fn test_supports_unary() {
        let ast = AstNode::UnaryOp(UnaryOp::Neg, Box::new(parse("catalan(5)").unwrap()));
        assert!(CombinatoricsDomain.supports(&ast));
    }

    #[test]
    fn test_supports_matrix() {
        let ast = AstNode::Matrix(vec![vec![parse("P(5,2)").unwrap()]]);
        assert!(CombinatoricsDomain.supports(&ast));
    }

    #[test]
    fn test_supports_list() {
        let ast = AstNode::List(vec![parse("stirling(5,2)").unwrap()]);
        assert!(CombinatoricsDomain.supports(&ast));
    }

    #[test]
    fn test_not_supports_arithmetic() {
        let ast = parse("1+2").unwrap();
        assert!(!CombinatoricsDomain.supports(&ast));
    }

    // ===== 错误路径测试 =====

    #[test]
    fn test_unsupported_function() {
        let ast = AstNode::FunctionCall("sin".to_string(), vec![AstNode::Number(1.0)]);
        let result = CombinatoricsDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_P_wrong_args() {
        let ast = AstNode::FunctionCall("P".to_string(), vec![AstNode::Number(1.0)]);
        let result = CombinatoricsDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_catalan_wrong_args() {
        let ast = AstNode::FunctionCall(
            "catalan".to_string(),
            vec![AstNode::Number(1.0), AstNode::Number(2.0)],
        );
        let result = CombinatoricsDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_eval_node_float_rejected() {
        let ast = AstNode::Number(3.14);
        let result = CombinatoricsDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_eval_node_complex_rejected() {
        let ast = AstNode::Complex(1.0, 2.0);
        let result = CombinatoricsDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_eval_node_list_rejected() {
        let ast = AstNode::List(vec![AstNode::Number(1.0)]);
        let result = CombinatoricsDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_unbound_variable() {
        let ast = AstNode::Variable("x".to_string());
        let result = CombinatoricsDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Eval));
    }

    #[test]
    fn test_div_by_zero() {
        let ast = AstNode::BinaryOp(
            BinaryOp::Div,
            Box::new(AstNode::Number(10.0)),
            Box::new(AstNode::Number(0.0)),
        );
        let result = CombinatoricsDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::DivisionByZero));
    }

    #[test]
    fn test_negative_exponent() {
        let ast = AstNode::BinaryOp(
            BinaryOp::Pow,
            Box::new(AstNode::Number(2.0)),
            Box::new(AstNode::Number(-1.0)),
        );
        let result = CombinatoricsDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_unary_abs() {
        let ast = AstNode::UnaryOp(UnaryOp::Abs, Box::new(AstNode::Number(-5.0)));
        let result = CombinatoricsDomain
            .evaluate(&ast, &EvalContext::new())
            .unwrap();
        assert_eq!(result.as_scalar().unwrap(), 5.0);
    }

    #[test]
    fn test_big_number_invalid() {
        let ast = AstNode::BigNumber("not_a_number".to_string());
        let result = CombinatoricsDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_bigint_to_result() {
        assert_eq!(bigint_to_result(BigInt::from(42)), EvalResult::Scalar(42.0));
        let large = BigInt::from(2).pow(100);
        assert!(matches!(bigint_to_result(large), EvalResult::BigInt(_)));
    }

    // ===== 覆盖率补充测试 =====

    #[test]
    fn test_eval_node_integer_number() {
        let ast = AstNode::Number(42.0);
        let result = CombinatoricsDomain
            .evaluate(&ast, &EvalContext::new())
            .unwrap();
        assert_eq!(result.as_scalar().unwrap(), 42.0);
    }

    #[test]
    fn test_eval_node_bignumber() {
        let ast = AstNode::BigNumber("12345678901234567890".to_string());
        let result = CombinatoricsDomain
            .evaluate(&ast, &EvalContext::new())
            .unwrap();
        assert!(matches!(result, EvalResult::BigInt(_)));
    }

    #[test]
    fn test_eval_node_binaryop_success() {
        let ast = AstNode::BinaryOp(
            BinaryOp::Add,
            Box::new(AstNode::Number(2.0)),
            Box::new(AstNode::Number(3.0)),
        );
        let result = CombinatoricsDomain
            .evaluate(&ast, &EvalContext::new())
            .unwrap();
        assert_eq!(result.as_scalar().unwrap(), 5.0);
    }

    #[test]
    fn test_eval_node_unary_neg() {
        let ast = AstNode::UnaryOp(UnaryOp::Neg, Box::new(AstNode::Number(5.0)));
        let result = CombinatoricsDomain
            .evaluate(&ast, &EvalContext::new())
            .unwrap();
        assert_eq!(result.as_scalar().unwrap(), -5.0);
    }

    #[test]
    fn test_eval_node_unary_factorial_rejected() {
        let ast = AstNode::UnaryOp(UnaryOp::Factorial, Box::new(AstNode::Number(5.0)));
        let result = CombinatoricsDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_eval_int_non_integer() {
        let ast = AstNode::FunctionCall(
            "P".to_string(),
            vec![AstNode::Number(3.14), AstNode::Number(2.0)],
        );
        let result = CombinatoricsDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_eval_int_overflow() {
        let ast = AstNode::FunctionCall(
            "P".to_string(),
            vec![AstNode::Number(1.0e20), AstNode::Number(2.0)],
        );
        let result = CombinatoricsDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Overflow));
    }

    #[test]
    fn test_eval_int_bignumber_arg() {
        let ast = AstNode::FunctionCall(
            "C".to_string(),
            vec![AstNode::BigNumber("100".to_string()), AstNode::Number(50.0)],
        );
        let result = CombinatoricsDomain
            .evaluate(&ast, &EvalContext::new())
            .unwrap();
        assert!(matches!(result, EvalResult::BigInt(_)));
    }

    #[test]
    fn test_eval_int_variable_success() {
        let ctx = EvalContext::new().with_var("x", 5.0);
        let ast = AstNode::FunctionCall(
            "P".to_string(),
            vec![AstNode::Variable("x".to_string()), AstNode::Number(2.0)],
        );
        let result = CombinatoricsDomain.evaluate(&ast, &ctx).unwrap();
        assert_eq!(result.as_scalar().unwrap(), 20.0);
    }

    #[test]
    fn test_eval_int_variable_non_integer() {
        let ctx = EvalContext::new().with_var("x", 3.14);
        let ast = AstNode::FunctionCall(
            "P".to_string(),
            vec![AstNode::Variable("x".to_string()), AstNode::Number(2.0)],
        );
        let result = CombinatoricsDomain.evaluate(&ast, &ctx);
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_eval_int_binaryop() {
        // 覆盖 eval_int 的 BinaryOp 路径
        assert_eq!(eval_scalar("P(3+2, 2)").unwrap(), 20.0);
    }

    #[test]
    fn test_eval_int_unary_abs() {
        // 覆盖 eval_int 的 UnaryOp::Abs 路径
        let ast = AstNode::FunctionCall(
            "P".to_string(),
            vec![
                AstNode::UnaryOp(UnaryOp::Abs, Box::new(AstNode::Number(-5.0))),
                AstNode::Number(2.0),
            ],
        );
        let result = CombinatoricsDomain
            .evaluate(&ast, &EvalContext::new())
            .unwrap();
        assert_eq!(result.as_scalar().unwrap(), 20.0);
    }

    #[test]
    fn test_eval_int_unary_factorial_rejected() {
        let ast = AstNode::FunctionCall(
            "P".to_string(),
            vec![
                AstNode::UnaryOp(UnaryOp::Factorial, Box::new(AstNode::Number(5.0))),
                AstNode::Number(2.0),
            ],
        );
        let result = CombinatoricsDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_eval_int_complex_rejected() {
        let ast = AstNode::FunctionCall(
            "P".to_string(),
            vec![AstNode::Complex(1.0, 2.0), AstNode::Number(2.0)],
        );
        let result = CombinatoricsDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_eval_int_binary_sub_mul() {
        // 覆盖 eval_int_binary 的 Sub/Mul 路径
        assert_eq!(eval_scalar("P(6-1, 2*1)").unwrap(), 20.0);
    }

    #[test]
    fn test_eval_int_binary_div_success() {
        // 覆盖 eval_int_binary 的 Div 路径
        assert_eq!(eval_scalar("P(10/2, 2)").unwrap(), 20.0);
    }

    #[test]
    fn test_eval_int_binary_mod_success() {
        // 覆盖 eval_int_binary 的 Mod 路径（直接构造 AST）
        let ast = AstNode::FunctionCall(
            "P".to_string(),
            vec![
                AstNode::BinaryOp(
                    BinaryOp::Mod,
                    Box::new(AstNode::Number(7.0)),
                    Box::new(AstNode::Number(2.0)),
                ),
                AstNode::Number(2.0),
            ],
        );
        let result = CombinatoricsDomain
            .evaluate(&ast, &EvalContext::new())
            .unwrap();
        assert_eq!(result.as_scalar().unwrap(), 0.0);
    }

    #[test]
    fn test_eval_int_binary_mod_by_zero() {
        let ast = AstNode::BinaryOp(
            BinaryOp::Mod,
            Box::new(AstNode::Number(10.0)),
            Box::new(AstNode::Number(0.0)),
        );
        let result = CombinatoricsDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::DivisionByZero));
    }

    #[test]
    fn test_C_wrong_args() {
        let ast = AstNode::FunctionCall("C".to_string(), vec![AstNode::Number(1.0)]);
        let result = CombinatoricsDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_stirling_wrong_args() {
        let ast = AstNode::FunctionCall("stirling".to_string(), vec![AstNode::Number(1.0)]);
        let result = CombinatoricsDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_eval_int_binary_pow_success() {
        // 覆盖 eval_int_binary 的 Pow 路径
        assert_eq!(eval_scalar("P(2^3, 2)").unwrap(), 56.0);
    }

    #[test]
    fn test_eval_int_binary_pow_negative() {
        let ast = AstNode::FunctionCall(
            "P".to_string(),
            vec![
                AstNode::BinaryOp(
                    BinaryOp::Pow,
                    Box::new(AstNode::Number(2.0)),
                    Box::new(AstNode::Number(-1.0)),
                ),
                AstNode::Number(2.0),
            ],
        );
        let result = CombinatoricsDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_eval_int_binary_pow_overflow() {
        let ast = AstNode::FunctionCall(
            "P".to_string(),
            vec![
                AstNode::BinaryOp(
                    BinaryOp::Pow,
                    Box::new(AstNode::Number(2.0)),
                    Box::new(AstNode::BigNumber("99999999999999999999999999".to_string())),
                ),
                AstNode::Number(2.0),
            ],
        );
        let result = CombinatoricsDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Overflow));
    }

    // ===== eval_function 分发行为锁定测试（正常/参数错误/边界/未知函数） =====

    #[test]
    fn test_eval_function_dispatch_table() {
        // ===== P(n,k) 排列 =====
        // 正常计算
        assert_eq!(eval_scalar("P(5,2)").unwrap(), 20.0);
        // 边界 k>n
        assert_eq!(eval_scalar("P(2,5)").unwrap(), 0.0);
        // 边界 k=0
        assert_eq!(eval_scalar("P(5,0)").unwrap(), 1.0);
        // 边界 k=n
        assert_eq!(eval_scalar("P(5,5)").unwrap(), 120.0);
        // 参数数量错误
        assert!(matches!(eval("P(5)"), Err(e) if e.kind == ErrorKind::Domain));
        // 参数数量过多
        assert!(matches!(eval("P(5,2,1)"), Err(e) if e.kind == ErrorKind::Domain));
        // 负参数 n
        assert!(matches!(eval("P(-1,2)"), Err(e) if e.kind == ErrorKind::Domain));
        // 负参数 k
        assert!(matches!(eval("P(5,-1)"), Err(e) if e.kind == ErrorKind::Domain));

        // ===== C(n,k) 组合 =====
        // 正常计算
        assert_eq!(eval_scalar("C(10,3)").unwrap(), 120.0);
        // 边界 k>n
        assert_eq!(eval_scalar("C(3,5)").unwrap(), 0.0);
        // 边界 k=0
        assert_eq!(eval_scalar("C(5,0)").unwrap(), 1.0);
        // 边界 k=n
        assert_eq!(eval_scalar("C(5,5)").unwrap(), 1.0);
        // 对称性：C(n,k) == C(n,n-k)
        assert_eq!(
            eval_scalar("C(10,3)").unwrap(),
            eval_scalar("C(10,7)").unwrap()
        );
        // 参数数量错误
        assert!(matches!(eval("C(5)"), Err(e) if e.kind == ErrorKind::Domain));
        // 负参数
        assert!(matches!(eval("C(-1,2)"), Err(e) if e.kind == ErrorKind::Domain));

        // ===== catalan(n) =====
        // 正常计算
        assert_eq!(eval_scalar("catalan(5)").unwrap(), 42.0);
        // 边界 n=0
        assert_eq!(eval_scalar("catalan(0)").unwrap(), 1.0);
        // 边界 n=1
        assert_eq!(eval_scalar("catalan(1)").unwrap(), 1.0);
        // 参数数量错误
        assert!(matches!(eval("catalan(5,2)"), Err(e) if e.kind == ErrorKind::Domain));
        // 负参数
        assert!(matches!(eval("catalan(-1)"), Err(e) if e.kind == ErrorKind::Domain));

        // ===== stirling(n,k) 第二类 Stirling 数 =====
        // 正常计算
        assert_eq!(eval_scalar("stirling(5,2)").unwrap(), 15.0);
        // 边界 k=0
        assert_eq!(eval_scalar("stirling(0,0)").unwrap(), 1.0);
        // 参数数量错误
        assert!(matches!(eval("stirling(5)"), Err(e) if e.kind == ErrorKind::Domain));
        // 负参数
        assert!(matches!(eval("stirling(-1,2)"), Err(e) if e.kind == ErrorKind::Domain));
    }

    // proptest 属性测试使用 math 模块函数。

    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig { cases: 128, ..ProptestConfig::default() })]

        /// 属性：C(n,k) == C(n,n-k)
        #[test]
        fn prop_combination_symmetry(n in 0u64..50, k in 0u64..50) {
            if k <= n {
                let c1 = crate::math::combinatorics::comb(&BigInt::from(n), &BigInt::from(k)).unwrap();
                let c2 = crate::math::combinatorics::comb(&BigInt::from(n), &BigInt::from(n - k)).unwrap();
                prop_assert_eq!(c1, c2);
            }
        }

        /// 属性：C(n,0) == 1
        #[test]
        fn prop_combination_zero_k(n in 0u64..100) {
            prop_assert_eq!(crate::math::combinatorics::comb(&BigInt::from(n), &BigInt::from(0)).unwrap(), BigInt::from(1));
        }

        /// 属性：P(n,n) == n!
        #[test]
        fn prop_permutation_n_n_is_factorial(n in 0u64..20) {
            let p = crate::math::combinatorics::perm(&BigInt::from(n), &BigInt::from(n)).unwrap();
            let mut factorial = BigInt::from(1);
            for i in 1..=n {
                factorial *= BigInt::from(i);
            }
            prop_assert_eq!(p, factorial);
        }

        /// 属性：C(n,k) <= P(n,k)
        #[test]
        fn prop_combination_le_permutation(n in 0u64..50, k in 0u64..50) {
            if k <= n {
                let c = crate::math::combinatorics::comb(&BigInt::from(n), &BigInt::from(k)).unwrap();
                let p = crate::math::combinatorics::perm(&BigInt::from(n), &BigInt::from(k)).unwrap();
                prop_assert!(c <= p);
            }
        }
    }
}
