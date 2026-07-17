// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! Combinatorics 计算域：排列 P、组合 C、Catalan 数、Stirling 数。
//!
//! 设计依据：
//! - combinatorics-domain spec：5 个 requirements / 13+ scenarios
//! - design.md D5（u128→BigInt 自动升级）、D6（priority=25）
//!
//! 路由策略：AST 含组合函数调用（P/C/catalan/stirling）时路由至本域。
//! 内部用 u128 累积，溢出时自动升级为 BigInt，返回 Scalar（fit i64）或 BigInt。

use crate::core::CalculationDomain;
use crate::core::{
    check_pow_output_size, AstNode, BinaryOp, CalcError, EvalContext, EvalResult, UnaryOp,
    MAX_POW_EXPONENT,
};
use num_bigint::BigInt;
use num_traits::{One, Signed, ToPrimitive, Zero};

/// 组合函数白名单。
const COMBINATORICS_FUNCTIONS: &[&str] = &["P", "C", "catalan", "stirling"];

/// Combinatorics 计算域。
///
/// priority=25，支持 P/C/catalan/stirling。
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
                    )));
                }
                Ok(EvalResult::Scalar(*n))
            }
            AstNode::BigNumber(s) => {
                let b: BigInt = s
                    .parse()
                    .map_err(|_| CalcError::domain(format!("invalid big number: {}", s)))?;
                Ok(EvalResult::BigInt(b))
            }
            AstNode::Variable(name) => ctx
                .get_var(name)
                .map(EvalResult::Scalar)
                .ok_or_else(|| CalcError::eval(format!("unbound variable: {}", name))),
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
                    )),
                }
            }
            AstNode::Complex(_, _) | AstNode::Matrix(_) | AstNode::List(_) => {
                Err(CalcError::domain(format!(
                    "combinatorics domain does not support this node type: {:?}",
                    ast
                )))
            }
        }
    }

    /// 将 AST 求值为 BigInt（精确整数运算）。
    fn eval_int(&self, ast: &AstNode, ctx: &EvalContext) -> Result<BigInt, CalcError> {
        match ast {
            AstNode::Number(n) => {
                if n.fract() != 0.0 {
                    return Err(CalcError::domain(format!(
                        "expected integer argument, got {}",
                        n
                    )));
                }
                if *n > i64::MAX as f64 || *n < i64::MIN as f64 {
                    return Err(CalcError::overflow());
                }
                Ok(BigInt::from(*n as i64))
            }
            AstNode::BigNumber(s) => s
                .parse::<BigInt>()
                .map_err(|_| CalcError::domain(format!("invalid big number: {}", s))),
            AstNode::Variable(name) => {
                let v = ctx
                    .get_var(name)
                    .ok_or_else(|| CalcError::eval(format!("unbound variable: {}", name)))?;
                if v.fract() != 0.0 {
                    return Err(CalcError::domain(format!(
                        "variable {} is not an integer: {}",
                        name, v
                    )));
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
                    )),
                }
            }
            AstNode::Complex(_, _)
            | AstNode::Matrix(_)
            | AstNode::List(_)
            | AstNode::FunctionCall(_, _) => Err(CalcError::domain(format!(
                "expected integer expression, got: {:?}",
                ast
            ))),
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
                    ));
                }
                // 安全约束1：拒绝超大指数，防止 DoS（与 precision.rs 一致）。
                // 安全审查 CRITICAL 修复：number_theory/combinatorics 域原无防护，
                // 攻击者可通过 `C(1,1) + 2^4000000000` 绕过 precision.rs 的防护
                // （24 字节请求触发 ~1.2GB 输出导致 OOM）。
                let exp_u64 = b.to_u64().ok_or(CalcError::overflow())?;
                if exp_u64 > MAX_POW_EXPONENT {
                    return Err(CalcError::domain(format!(
                        "power exponent must not exceed {} (got {})",
                        MAX_POW_EXPONENT, exp_u64
                    )));
                }
                // 安全约束2：底数复合限制，防止大底数 × 大指数产生超大输出 DoS。
                // 复用 core 层 `check_pow_output_size`，三域共用同一检查。
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

    /// 求值组合函数调用（dispatch table）。
    ///
    /// 重构说明：原实现 cyc=38（每个 case 内联参数验证 + 计算），
    /// 重构后主函数仅做白名单检查 + match 分发（cyc=6），具体逻辑下沉到
    /// `eval_permutation`/`eval_combination`/`eval_catalan`/`eval_stirling` 4 个方法，
    /// 共用参数验证提取为 `eval_two_non_negative_args`/`eval_one_non_negative_arg`。
    fn eval_function(
        &self,
        name: &str,
        args: &[AstNode],
        ctx: &EvalContext,
    ) -> Result<EvalResult, CalcError> {
        if !COMBINATORICS_FUNCTIONS.contains(&name) {
            return Err(CalcError::domain(format!(
                "unsupported function in combinatorics domain: {}",
                name
            )));
        }
        match name {
            "P" => self.eval_permutation(args, ctx),
            "C" => self.eval_combination(args, ctx),
            "catalan" => self.eval_catalan(args, ctx),
            "stirling" => self.eval_stirling(args, ctx),
            _ => unreachable!(),
        }
    }

    /// 求值两参数非负整数参数（P/C/stirling 共用 helper）。
    ///
    /// 校验：参数数量 == 2，n/k 均非负。
    /// 错误消息含函数名（通过 `name` 参数注入），保持与原实现一致。
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
            )));
        }
        let n = self.eval_int(&args[0], ctx)?;
        let k = self.eval_int(&args[1], ctx)?;
        if n.is_negative() || k.is_negative() {
            return Err(CalcError::domain(format!(
                "{}() requires non-negative arguments",
                name
            )));
        }
        Ok((n, k))
    }

    /// 求值单参数非负整数参数（catalan 用 helper）。
    ///
    /// 校验：参数数量 == 1，n 非负。
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
            )));
        }
        let n = self.eval_int(&args[0], ctx)?;
        if n.is_negative() {
            return Err(CalcError::domain(format!(
                "{}() requires non-negative argument",
                name
            )));
        }
        Ok(n)
    }

    /// P(n,k) 排列数：n!/(n-k)!。
    ///
    /// 边界：k > n 返回 0；k=0 返回 1；k=n 返回 n!。
    fn eval_permutation(
        &self,
        args: &[AstNode],
        ctx: &EvalContext,
    ) -> Result<EvalResult, CalcError> {
        let (n, k) = self.eval_two_non_negative_args(args, ctx, "P")?;
        if k > n {
            return Ok(EvalResult::Scalar(0.0));
        }
        let result = permutation(&n, &k)?;
        Ok(bigint_to_result(result))
    }

    /// C(n,k) 组合数：n!/((n-k)!*k!)。
    ///
    /// 边界：k > n 返回 0；k=0 返回 1；k=n 返回 1；对称性 C(n,k)==C(n,n-k)。
    fn eval_combination(
        &self,
        args: &[AstNode],
        ctx: &EvalContext,
    ) -> Result<EvalResult, CalcError> {
        let (n, k) = self.eval_two_non_negative_args(args, ctx, "C")?;
        if k > n {
            return Ok(EvalResult::Scalar(0.0));
        }
        let result = combination(&n, &k)?;
        Ok(bigint_to_result(result))
    }

    /// catalan(n) Catalan 数：C(2n,n)/(n+1)。
    ///
    /// 边界：n=0 返回 1；n=1 返回 1。
    fn eval_catalan(&self, args: &[AstNode], ctx: &EvalContext) -> Result<EvalResult, CalcError> {
        let n = self.eval_one_non_negative_arg(args, ctx, "catalan")?;
        let result = catalan(&n)?;
        Ok(bigint_to_result(result))
    }

    /// stirling(n,k) 第二类 Stirling 数：将 n 个元素划分为 k 个非空子集的方式数。
    ///
    /// 注意：stirling 不做 k>n 返回 0 的特殊处理（由 stirling_second 内部处理）。
    fn eval_stirling(&self, args: &[AstNode], ctx: &EvalContext) -> Result<EvalResult, CalcError> {
        let (n, k) = self.eval_two_non_negative_args(args, ctx, "stirling")?;
        let result = stirling_second(&n, &k)?;
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

/// 排列数 P(n, k) = n!/(n-k)! = n*(n-1)*...*(n-k+1)。
/// DoS 防护：k 上界 10000，超限返回 Overflow。
fn permutation(n: &BigInt, k: &BigInt) -> Result<BigInt, CalcError> {
    if k.is_zero() {
        return Ok(BigInt::one());
    }
    if k > n {
        return Ok(BigInt::zero());
    }
    const MAX_PERMUTATION_K: u64 = 10000;
    let k_u64 = k.to_u64().ok_or(CalcError::overflow())?;
    if k_u64 > MAX_PERMUTATION_K {
        return Err(CalcError::overflow());
    }
    let mut result = BigInt::one();
    let mut current = n.clone();
    for _ in 0..k_u64 {
        result *= &current;
        current -= 1;
    }
    Ok(result)
}

/// 组合数 C(n, k) = n!/(k!(n-k)!) = P(n,k)/k!。
/// DoS 防护：k 上界 10000，超限返回 Overflow。
fn combination(n: &BigInt, k: &BigInt) -> Result<BigInt, CalcError> {
    if k.is_zero() || k == n {
        return Ok(BigInt::one());
    }
    if k > n {
        return Ok(BigInt::zero());
    }
    // C(n,k) = C(n, n-k)，取较小的 k 提高效率
    let k_opt = if k < &(n - k) { k.clone() } else { n - k };
    const MAX_COMBINATION_K: u64 = 10000;
    let k_u64 = k_opt.to_u64().ok_or(CalcError::overflow())?;
    if k_u64 > MAX_COMBINATION_K {
        return Err(CalcError::overflow());
    }
    let mut result = BigInt::one();
    let mut current = n.clone();
    for i in 0..k_u64 {
        result *= &current;
        result /= BigInt::from(i + 1);
        current -= 1;
    }
    Ok(result)
}

/// Catalan 数 C(n) = C(2n,n)/(n+1)。
/// DoS 防护：n 上界 5000，超限返回 Overflow。
fn catalan(n: &BigInt) -> Result<BigInt, CalcError> {
    if n.is_zero() {
        return Ok(BigInt::one());
    }
    const MAX_CATALAN_N: u64 = 5000;
    let n_u64 = n.to_u64().ok_or(CalcError::overflow())?;
    if n_u64 > MAX_CATALAN_N {
        return Err(CalcError::overflow());
    }
    let two_n = n * 2;
    let c_2n_n = combination(&two_n, n)?;
    Ok(c_2n_n / (n + 1))
}

/// 第二类 Stirling 数 S(n, k)：将 n 个元素划分为 k 个非空子集的方式数。
/// 递推：S(n,k) = k*S(n-1,k) + S(n-1,k-1)
/// 边界：S(0,0)=1, S(n,0)=0 (n>0), S(0,k)=0 (k>0), S(n,k)=0 (k>n)
/// DoS 防护：n、k 上界 5000，超限返回 Overflow。
fn stirling_second(n: &BigInt, k: &BigInt) -> Result<BigInt, CalcError> {
    if n.is_zero() && k.is_zero() {
        return Ok(BigInt::one());
    }
    if n.is_zero() || k.is_zero() {
        return Ok(BigInt::zero());
    }
    if k > n {
        return Ok(BigInt::zero());
    }
    const MAX_STIRLING_N: u64 = 5000;
    let n_u64 = n.to_u64().ok_or(CalcError::overflow())?;
    let k_u64 = k.to_u64().ok_or(CalcError::overflow())?;
    if n_u64 > MAX_STIRLING_N || k_u64 > MAX_STIRLING_N {
        return Err(CalcError::overflow());
    }
    // DP 表
    let mut dp: Vec<Vec<BigInt>> =
        vec![vec![BigInt::zero(); k_u64 as usize + 1]; n_u64 as usize + 1];
    dp[0][0] = BigInt::one();
    for i in 1..=n_u64 as usize {
        for j in 1..=k_u64 as usize {
            if j > i {
                break;
            }
            dp[i][j] = &dp[i - 1][j - 1] + &dp[i - 1][j] * BigInt::from(j);
        }
    }
    Ok(dp[n_u64 as usize][k_u64 as usize].clone())
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
        | AstNode::BigNumber(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::parse;
    use crate::core::ErrorKind;

    fn eval(input: &str) -> Result<EvalResult, CalcError> {
        let ast = parse(input).unwrap();
        let domain = CombinatoricsDomain;
        let ctx = EvalContext::new();
        domain.evaluate(&ast, &ctx)
    }

    fn eval_scalar(input: &str) -> Result<f64, CalcError> {
        eval(input).map(|r| r.as_scalar().expect("expected scalar result"))
    }

    // ===== UT-CMB-001: P(n,k) =====

    #[test]
    fn test_permutation_basic() {
        assert_eq!(eval_scalar("P(5,2)").unwrap(), 20.0);
    }

    // ===== UT-CMB-002: C(n,k) =====

    #[test]
    fn test_combination_basic() {
        assert_eq!(eval_scalar("C(10,3)").unwrap(), 120.0);
    }

    // ===== UT-CMB-003: Catalan =====

    #[test]
    fn test_catalan_basic() {
        assert_eq!(eval_scalar("catalan(5)").unwrap(), 42.0);
    }

    // ===== UT-CMB-004: Stirling =====

    #[test]
    fn test_stirling_basic() {
        assert_eq!(eval_scalar("stirling(5,2)").unwrap(), 15.0);
    }

    // ===== UT-CMB-005: C(n,0) =====

    #[test]
    fn test_combination_zero_k() {
        assert_eq!(eval_scalar("C(5,0)").unwrap(), 1.0);
    }

    // ===== UT-CMB-006: C(n,n) =====

    #[test]
    fn test_combination_n_n() {
        assert_eq!(eval_scalar("C(5,5)").unwrap(), 1.0);
    }

    // ===== UT-CMB-007: k>n =====

    #[test]
    fn test_combination_k_greater_than_n() {
        assert_eq!(eval_scalar("C(3,5)").unwrap(), 0.0);
    }

    // ===== UT-CMB-008: n<0 =====

    #[test]
    fn test_combination_negative_n() {
        let result = eval("C(-1,2)");
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    // ===== UT-CMB-009: k<0 =====

    #[test]
    fn test_combination_negative_k() {
        let result = eval("C(5,-1)");
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    // ===== UT-CMB-010: 大数 C =====

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

    // ===== UT-CMB-011: 大数 P =====

    #[test]
    fn test_permutation_large() {
        // P(20,10) = 20*19*...*11
        let result = eval_scalar("P(20,10)").unwrap();
        // P(20,10) = 670442572800
        assert_eq!(result, 670442572800.0);
    }

    // ===== UT-CMB-012: Catalan 边界 =====

    #[test]
    fn test_catalan_zero() {
        assert_eq!(eval_scalar("catalan(0)").unwrap(), 1.0);
    }

    // ===== UT-CMB-013: Catalan 大数 =====

    #[test]
    fn test_catalan_large() {
        // catalan(30) = 3814986502092304
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

    // ===== UT-CMB-014: Stirling 边界 =====

    #[test]
    fn test_stirling_zero_zero() {
        assert_eq!(eval_scalar("stirling(0,0)").unwrap(), 1.0);
    }

    // ===== UT-CMB-015: 溢出处理 =====

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
        // P(5,5) = 120 = 5!
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
        // C(10,3) == C(10,7)
        assert_eq!(
            eval_scalar("C(10,3)").unwrap(),
            eval_scalar("C(10,7)").unwrap()
        );
    }

    #[test]
    fn test_catalan_sequence() {
        // Catalan 数列：1, 1, 2, 5, 14, 42, 132, 429...
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
        // S(3,2) = 3
        assert_eq!(eval_scalar("stirling(3,2)").unwrap(), 3.0);
        // S(4,2) = 7
        assert_eq!(eval_scalar("stirling(4,2)").unwrap(), 7.0);
        // S(4,3) = 6
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

    // ===== 底层算法测试 =====

    #[test]
    fn test_permutation_known() {
        assert_eq!(
            permutation(&BigInt::from(5), &BigInt::from(2)).unwrap(),
            BigInt::from(20)
        );
        assert_eq!(
            permutation(&BigInt::from(5), &BigInt::from(0)).unwrap(),
            BigInt::from(1)
        );
        assert_eq!(
            permutation(&BigInt::from(3), &BigInt::from(5)).unwrap(),
            BigInt::from(0)
        );
    }

    #[test]
    fn test_combination_known() {
        assert_eq!(
            combination(&BigInt::from(10), &BigInt::from(3)).unwrap(),
            BigInt::from(120)
        );
        assert_eq!(
            combination(&BigInt::from(5), &BigInt::from(0)).unwrap(),
            BigInt::from(1)
        );
        assert_eq!(
            combination(&BigInt::from(5), &BigInt::from(5)).unwrap(),
            BigInt::from(1)
        );
        assert_eq!(
            combination(&BigInt::from(3), &BigInt::from(5)).unwrap(),
            BigInt::from(0)
        );
    }

    #[test]
    fn test_catalan_known() {
        assert_eq!(catalan(&BigInt::from(0)).unwrap(), BigInt::from(1));
        assert_eq!(catalan(&BigInt::from(1)).unwrap(), BigInt::from(1));
        assert_eq!(catalan(&BigInt::from(5)).unwrap(), BigInt::from(42));
    }

    #[test]
    fn test_stirling_known() {
        assert_eq!(
            stirling_second(&BigInt::from(0), &BigInt::from(0)).unwrap(),
            BigInt::from(1)
        );
        assert_eq!(
            stirling_second(&BigInt::from(5), &BigInt::from(2)).unwrap(),
            BigInt::from(15)
        );
        assert_eq!(
            stirling_second(&BigInt::from(3), &BigInt::from(2)).unwrap(),
            BigInt::from(3)
        );
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
        // eval_node Number success (integer)
        let ast = AstNode::Number(42.0);
        let result = CombinatoricsDomain
            .evaluate(&ast, &EvalContext::new())
            .unwrap();
        assert_eq!(result.as_scalar().unwrap(), 42.0);
    }

    #[test]
    fn test_eval_node_bignumber() {
        // eval_node BigNumber success
        let ast = AstNode::BigNumber("12345678901234567890".to_string());
        let result = CombinatoricsDomain
            .evaluate(&ast, &EvalContext::new())
            .unwrap();
        assert!(matches!(result, EvalResult::BigInt(_)));
    }

    #[test]
    fn test_eval_node_binaryop_success() {
        // eval_node BinaryOp success: 2+3 = 5
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
        // eval_node UnaryOp::Neg
        let ast = AstNode::UnaryOp(UnaryOp::Neg, Box::new(AstNode::Number(5.0)));
        let result = CombinatoricsDomain
            .evaluate(&ast, &EvalContext::new())
            .unwrap();
        assert_eq!(result.as_scalar().unwrap(), -5.0);
    }

    #[test]
    fn test_eval_node_unary_factorial_rejected() {
        // eval_node UnaryOp::Factorial error
        let ast = AstNode::UnaryOp(UnaryOp::Factorial, Box::new(AstNode::Number(5.0)));
        let result = CombinatoricsDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_eval_int_non_integer() {
        // eval_int non-integer error via function arg
        let ast = AstNode::FunctionCall(
            "P".to_string(),
            vec![AstNode::Number(3.14), AstNode::Number(2.0)],
        );
        let result = CombinatoricsDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_eval_int_overflow() {
        // eval_int overflow: number > i64::MAX
        let ast = AstNode::FunctionCall(
            "P".to_string(),
            vec![AstNode::Number(1.0e20), AstNode::Number(2.0)],
        );
        let result = CombinatoricsDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Overflow));
    }

    #[test]
    fn test_eval_int_bignumber_arg() {
        // eval_int BigNumber success via function arg
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
        // eval_int Variable success
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
        // eval_int Variable non-integer error
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
        // eval_int BinaryOp: P(3+2, 2) = P(5,2) = 20
        assert_eq!(eval_scalar("P(3+2, 2)").unwrap(), 20.0);
    }

    #[test]
    fn test_eval_int_unary_abs() {
        // eval_int UnaryOp::Abs: P(abs(-5), 2) = P(5,2) = 20
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
        // eval_int UnaryOp::Factorial error
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
        // eval_int Complex rejection
        let ast = AstNode::FunctionCall(
            "P".to_string(),
            vec![AstNode::Complex(1.0, 2.0), AstNode::Number(2.0)],
        );
        let result = CombinatoricsDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_eval_int_binary_sub_mul() {
        // eval_int_binary Sub and Mul: P(6-1, 2*1) = P(5,2) = 20
        assert_eq!(eval_scalar("P(6-1, 2*1)").unwrap(), 20.0);
    }

    #[test]
    fn test_eval_int_binary_div_success() {
        // eval_int_binary Div success: P(10/2, 2) = P(5,2) = 20
        assert_eq!(eval_scalar("P(10/2, 2)").unwrap(), 20.0);
    }

    #[test]
    fn test_eval_int_binary_mod_success() {
        // eval_int_binary Mod success via direct AST: P(Mod(7,2), 2) = P(1,2) = 0
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
        // eval_int_binary Mod by zero
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
        // eval_int_binary Pow success: P(2^3, 2) = P(8,2) = 56
        assert_eq!(eval_scalar("P(2^3, 2)").unwrap(), 56.0);
    }

    #[test]
    fn test_eval_int_binary_pow_negative() {
        // eval_int_binary Pow negative exponent error
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
        // eval_int_binary Pow overflow (exponent too large for u32)
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

    // ===== T013: eval_function dispatch table 行为锁定测试（重构前 Red → 重构后 Green） =====
    //
    // 目的：锁定 eval_function 在 P/C/catalan/stirling 各 case 的行为，
    // 确保 T014 重构（提取 eval_permutation/eval_combination/eval_catalan/eval_stirling
    // + eval_two_non_negative_args/eval_one_non_negative_arg helper）后行为不变。
    //
    // 覆盖维度：
    // 1. 正常计算（各函数的 happy path）
    // 2. 参数数量错误（少参数/多参数）
    // 3. 负参数错误（n<0, k<0）
    // 4. 边界条件（k>n 返回 0）
    // 5. 不支持的函数名

    #[test]
    fn test_eval_function_dispatch_table() {
        // ===== P(n,k) 排列 =====
        // 正常计算：P(5,2) = 5!/(5-2)! = 20
        assert_eq!(eval_scalar("P(5,2)").unwrap(), 20.0);
        // 边界 k>n：P(2,5) = 0
        assert_eq!(eval_scalar("P(2,5)").unwrap(), 0.0);
        // 边界 k=0：P(5,0) = 1
        assert_eq!(eval_scalar("P(5,0)").unwrap(), 1.0);
        // 边界 k=n：P(5,5) = 120
        assert_eq!(eval_scalar("P(5,5)").unwrap(), 120.0);
        // 参数数量错误：P(5) → Err Domain
        assert!(matches!(eval("P(5)"), Err(e) if e.kind == ErrorKind::Domain));
        // 参数数量错误：P(5,2,1) → Err Domain
        assert!(matches!(eval("P(5,2,1)"), Err(e) if e.kind == ErrorKind::Domain));
        // 负参数 n：P(-1,2) → Err Domain
        assert!(matches!(eval("P(-1,2)"), Err(e) if e.kind == ErrorKind::Domain));
        // 负参数 k：P(5,-1) → Err Domain
        assert!(matches!(eval("P(5,-1)"), Err(e) if e.kind == ErrorKind::Domain));

        // ===== C(n,k) 组合 =====
        // 正常计算：C(10,3) = 120
        assert_eq!(eval_scalar("C(10,3)").unwrap(), 120.0);
        // 边界 k>n：C(3,5) = 0
        assert_eq!(eval_scalar("C(3,5)").unwrap(), 0.0);
        // 边界 k=0：C(5,0) = 1
        assert_eq!(eval_scalar("C(5,0)").unwrap(), 1.0);
        // 边界 k=n：C(5,5) = 1
        assert_eq!(eval_scalar("C(5,5)").unwrap(), 1.0);
        // 对称性：C(n,k) == C(n,n-k)
        assert_eq!(
            eval_scalar("C(10,3)").unwrap(),
            eval_scalar("C(10,7)").unwrap()
        );
        // 参数数量错误：C(5) → Err Domain
        assert!(matches!(eval("C(5)"), Err(e) if e.kind == ErrorKind::Domain));
        // 负参数：C(-1,2) → Err Domain
        assert!(matches!(eval("C(-1,2)"), Err(e) if e.kind == ErrorKind::Domain));

        // ===== catalan(n) =====
        // 正常计算：catalan(5) = 42
        assert_eq!(eval_scalar("catalan(5)").unwrap(), 42.0);
        // 边界 n=0：catalan(0) = 1
        assert_eq!(eval_scalar("catalan(0)").unwrap(), 1.0);
        // 边界 n=1：catalan(1) = 1
        assert_eq!(eval_scalar("catalan(1)").unwrap(), 1.0);
        // 参数数量错误：catalan(5,2) → Err Domain
        assert!(matches!(eval("catalan(5,2)"), Err(e) if e.kind == ErrorKind::Domain));
        // 负参数：catalan(-1) → Err Domain
        assert!(matches!(eval("catalan(-1)"), Err(e) if e.kind == ErrorKind::Domain));

        // ===== stirling(n,k) 第二类 Stirling 数 =====
        // 正常计算：stirling(5,2) = 15
        assert_eq!(eval_scalar("stirling(5,2)").unwrap(), 15.0);
        // 边界 k=0：stirling(0,0) = 1
        assert_eq!(eval_scalar("stirling(0,0)").unwrap(), 1.0);
        // 参数数量错误：stirling(5) → Err Domain
        assert!(matches!(eval("stirling(5)"), Err(e) if e.kind == ErrorKind::Domain));
        // 负参数：stirling(-1,2) → Err Domain
        assert!(matches!(eval("stirling(-1,2)"), Err(e) if e.kind == ErrorKind::Domain));
    }

    // ===== proptest 属性测试 =====

    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig { cases: 128, ..ProptestConfig::default() })]

        /// 属性：C(n,k) == C(n,n-k)
        #[test]
        fn prop_combination_symmetry(n in 0u64..50, k in 0u64..50) {
            if k <= n {
                let c1 = combination(&BigInt::from(n), &BigInt::from(k)).unwrap();
                let c2 = combination(&BigInt::from(n), &BigInt::from(n - k)).unwrap();
                prop_assert_eq!(c1, c2);
            }
        }

        /// 属性：C(n,0) == 1
        #[test]
        fn prop_combination_zero_k(n in 0u64..100) {
            prop_assert_eq!(combination(&BigInt::from(n), &BigInt::from(0)).unwrap(), BigInt::from(1));
        }

        /// 属性：P(n,n) == n!
        #[test]
        fn prop_permutation_n_n_is_factorial(n in 0u64..20) {
            let p = permutation(&BigInt::from(n), &BigInt::from(n)).unwrap();
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
                let c = combination(&BigInt::from(n), &BigInt::from(k)).unwrap();
                let p = permutation(&BigInt::from(n), &BigInt::from(k)).unwrap();
                prop_assert!(c <= p);
            }
        }
    }
}
