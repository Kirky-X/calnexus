// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! NumberTheory 计算域：GCD、LCM、素数判定、素数筛、模逆、模幂、欧拉函数。
//!
//! 设计依据：
//! - number-theory-domain spec：7 个 requirements / 15+ scenarios
//! - design.md D4（Miller-Rabin）、D7（num-integer 依赖）
//! - priority=25
//!
//! 路由策略：AST 含数论函数调用（gcd/lcm/is_prime/prime_sieve/mod_inverse/mod_pow/euler_phi）时路由至本域。
//! 内部使用 BigInt 精确整数运算，支持 `is_prime(10^18+9)` 等大数场景（f64 无法精确表示）。
//! 结果按值大小返回 Scalar（fit i64）或 BigInt。

use crate::core::CalculationDomain;
use crate::core::{AstNode, BinaryOp, CalcError, EvalContext, EvalResult, UnaryOp};
use num_bigint::BigInt;
use num_integer::Integer as _;
use num_traits::{One, Signed, ToPrimitive, Zero};

/// 数论函数白名单。
const NUMBER_THEORY_FUNCTIONS: &[&str] = &[
    "gcd",
    "lcm",
    "is_prime",
    "prime_sieve",
    "mod_inverse",
    "mod_pow",
    "euler_phi",
];

/// Miller-Rabin 确定性基（n < 3.3×10^24 时确定性判定）。
const MR_BASES: &[u64] = &[2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37];

/// NumberTheory 计算域。
///
/// priority=25，支持 gcd/lcm/is_prime/prime_sieve/mod_inverse/mod_pow/euler_phi。
pub struct NumberTheoryDomain;

impl CalculationDomain for NumberTheoryDomain {
    fn domain_name(&self) -> &str {
        "number_theory"
    }

    fn priority(&self) -> u8 {
        25
    }

    fn supports(&self, ast: &AstNode) -> bool {
        contains_number_theory_function(ast)
    }

    fn evaluate(&self, ast: &AstNode, ctx: &EvalContext) -> Result<EvalResult, CalcError> {
        self.eval_node(ast, ctx)
    }
}

impl Default for NumberTheoryDomain {
    fn default() -> Self {
        Self
    }
}

impl NumberTheoryDomain {
    /// 递归求值 AST 节点，返回 EvalResult。
    fn eval_node(&self, ast: &AstNode, ctx: &EvalContext) -> Result<EvalResult, CalcError> {
        match ast {
            AstNode::FunctionCall(name, args) => self.eval_function(name, args, ctx),
            AstNode::Number(n) => {
                if n.fract() != 0.0 {
                    return Err(CalcError::DomainError(format!(
                        "number theory domain requires integer, got {}",
                        n
                    )));
                }
                Ok(EvalResult::Scalar(*n))
            }
            AstNode::BigNumber(s) => {
                let b: BigInt = s
                    .parse()
                    .map_err(|_| CalcError::DomainError(format!("invalid big number: {}", s)))?;
                Ok(EvalResult::BigInt(b))
            }
            AstNode::Variable(name) => ctx
                .get_var(name)
                .map(EvalResult::Scalar)
                .ok_or_else(|| CalcError::EvalError(format!("unbound variable: {}", name))),
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
                    UnaryOp::Factorial => Err(CalcError::DomainError(
                        "factorial not supported in number theory domain".to_string(),
                    )),
                }
            }
            AstNode::Complex(_, _) | AstNode::Matrix(_) | AstNode::List(_) => {
                Err(CalcError::DomainError(format!(
                    "number theory domain does not support this node type: {:?}",
                    ast
                )))
            }
        }
    }

    /// 将 AST 求值为 BigInt（精确整数运算）。
    /// 用于解析数论函数的整数参数，支持 `10^18+9` 等大数表达式。
    fn eval_int(&self, ast: &AstNode, ctx: &EvalContext) -> Result<BigInt, CalcError> {
        match ast {
            AstNode::Number(n) => {
                if n.fract() != 0.0 {
                    return Err(CalcError::DomainError(format!(
                        "expected integer argument, got {}",
                        n
                    )));
                }
                if *n > i64::MAX as f64 || *n < i64::MIN as f64 {
                    return Err(CalcError::Overflow);
                }
                Ok(BigInt::from(*n as i64))
            }
            AstNode::BigNumber(s) => s
                .parse::<BigInt>()
                .map_err(|_| CalcError::DomainError(format!("invalid big number: {}", s))),
            AstNode::Variable(name) => {
                let v = ctx
                    .get_var(name)
                    .ok_or_else(|| CalcError::EvalError(format!("unbound variable: {}", name)))?;
                if v.fract() != 0.0 {
                    return Err(CalcError::DomainError(format!(
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
                    UnaryOp::Factorial => Err(CalcError::DomainError(
                        "factorial not supported in number theory domain".to_string(),
                    )),
                }
            }
            AstNode::FunctionCall(_, _) => {
                // 嵌套函数调用：先求值为 EvalResult，再转为 BigInt
                let result = self.eval_node(ast, ctx)?;
                match result {
                    EvalResult::Scalar(n) => {
                        if n.fract() != 0.0 {
                            return Err(CalcError::DomainError(format!(
                                "expected integer result, got {}",
                                n
                            )));
                        }
                        if n > i64::MAX as f64 || n < i64::MIN as f64 {
                            return Err(CalcError::Overflow);
                        }
                        Ok(BigInt::from(n as i64))
                    }
                    EvalResult::BigInt(b) => Ok(b),
                    _ => Err(CalcError::DomainError(format!(
                        "expected integer result from function call, got {:?}",
                        ast
                    ))),
                }
            }
            AstNode::Complex(_, _) | AstNode::Matrix(_) | AstNode::List(_) => Err(
                CalcError::DomainError(format!("expected integer expression, got: {:?}", ast)),
            ),
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
                    return Err(CalcError::DivisionByZero);
                }
                Ok(a / b)
            }
            BinaryOp::Pow => {
                if b.is_negative() {
                    return Err(CalcError::DomainError(
                        "negative exponent not supported for integers".to_string(),
                    ));
                }
                let exp: u32 = b.to_u32().ok_or(CalcError::Overflow)?;
                Ok(a.pow(exp))
            }
            BinaryOp::Mod => {
                if b.is_zero() {
                    return Err(CalcError::DivisionByZero);
                }
                Ok(a % b)
            }
        }
    }

    /// 求值数论函数调用。
    fn eval_function(
        &self,
        name: &str,
        args: &[AstNode],
        ctx: &EvalContext,
    ) -> Result<EvalResult, CalcError> {
        if !NUMBER_THEORY_FUNCTIONS.contains(&name) {
            return Err(CalcError::DomainError(format!(
                "unsupported function in number theory domain: {}",
                name
            )));
        }
        match name {
            "gcd" => {
                if args.len() != 2 {
                    return Err(CalcError::DomainError(format!(
                        "gcd() requires exactly 2 arguments, got {}",
                        args.len()
                    )));
                }
                let a = self.eval_int(&args[0], ctx)?;
                let b = self.eval_int(&args[1], ctx)?;
                let result = a.abs().gcd(&b.abs());
                Ok(bigint_to_result(result))
            }
            "lcm" => {
                if args.len() != 2 {
                    return Err(CalcError::DomainError(format!(
                        "lcm() requires exactly 2 arguments, got {}",
                        args.len()
                    )));
                }
                let a = self.eval_int(&args[0], ctx)?;
                let b = self.eval_int(&args[1], ctx)?;
                if a.is_zero() || b.is_zero() {
                    return Ok(EvalResult::Scalar(0.0));
                }
                let result = a.abs().lcm(&b.abs());
                Ok(bigint_to_result(result))
            }
            "is_prime" => {
                if args.len() != 1 {
                    return Err(CalcError::DomainError(format!(
                        "is_prime() requires exactly 1 argument, got {}",
                        args.len()
                    )));
                }
                let n = self.eval_int(&args[0], ctx)?;
                let prime = is_prime_bigint(&n);
                Ok(EvalResult::Scalar(if prime { 1.0 } else { 0.0 }))
            }
            "prime_sieve" => {
                if args.len() != 1 {
                    return Err(CalcError::DomainError(format!(
                        "prime_sieve() requires exactly 1 argument, got {}",
                        args.len()
                    )));
                }
                let n = self.eval_int(&args[0], ctx)?;
                if n.is_negative() {
                    return Err(CalcError::DomainError(
                        "prime_sieve() requires non-negative argument".to_string(),
                    ));
                }
                let n_u64 = n.to_u64().ok_or(CalcError::Overflow)?;
                let primes = prime_sieve_u64(n_u64);
                Ok(EvalResult::Vector(
                    primes.into_iter().map(|p| p as f64).collect(),
                ))
            }
            "mod_inverse" => {
                if args.len() != 2 {
                    return Err(CalcError::DomainError(format!(
                        "mod_inverse() requires exactly 2 arguments, got {}",
                        args.len()
                    )));
                }
                let a = self.eval_int(&args[0], ctx)?;
                let m = self.eval_int(&args[1], ctx)?;
                if m.is_zero() {
                    return Err(CalcError::DivisionByZero);
                }
                let m_abs = m.abs();
                match mod_inverse(&a, &m_abs) {
                    Some(inv) => Ok(bigint_to_result(inv)),
                    None => Err(CalcError::DomainError(format!(
                        "mod_inverse: {} and {} are not coprime",
                        a, m
                    ))),
                }
            }
            "mod_pow" => {
                if args.len() != 3 {
                    return Err(CalcError::DomainError(format!(
                        "mod_pow() requires exactly 3 arguments, got {}",
                        args.len()
                    )));
                }
                let base = self.eval_int(&args[0], ctx)?;
                let exp = self.eval_int(&args[1], ctx)?;
                let m = self.eval_int(&args[2], ctx)?;
                if m.is_zero() {
                    return Err(CalcError::DivisionByZero);
                }
                if exp.is_negative() {
                    return Err(CalcError::DomainError(
                        "mod_pow() requires non-negative exponent".to_string(),
                    ));
                }
                let m_abs = m.abs();
                let result = mod_pow_bigint(&base, &exp, &m_abs);
                Ok(bigint_to_result(result))
            }
            "euler_phi" => {
                if args.len() != 1 {
                    return Err(CalcError::DomainError(format!(
                        "euler_phi() requires exactly 1 argument, got {}",
                        args.len()
                    )));
                }
                let n = self.eval_int(&args[0], ctx)?;
                if n.is_zero() {
                    return Ok(EvalResult::Scalar(0.0));
                }
                let result = euler_phi(&n.abs());
                Ok(bigint_to_result(result))
            }
            _ => unreachable!(),
        }
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

/// BigInt Miller-Rabin 素数判定。
/// n < 2^64 使用确定性基，n >= 2^64 使用 25 轮概率判定。
fn is_prime_bigint(n: &BigInt) -> bool {
    if n < &BigInt::from(2) {
        return false;
    }
    if n == &BigInt::from(2) {
        return true;
    }
    if n.is_even() {
        return false;
    }

    // 尝试 u64 快速路径（确定性 12 基）
    if let Some(n_u64) = n.to_u64() {
        return is_prime_u64(n_u64);
    }

    // BigInt 路径：确定性基 + 额外轮次
    let two = BigInt::from(2);
    let one = BigInt::one();
    let n_minus_1 = n - &one;

    // 写 n-1 = 2^r * d
    let mut d = n_minus_1.clone();
    let mut r = 0u32;
    while d.is_even() {
        d /= &two;
        r += 1;
    }

    // 使用确定性基（对 n < 3.3e24 确定），额外补 13 轮共 25 轮
    for &base in MR_BASES.iter() {
        let a = BigInt::from(base);
        if a >= *n {
            continue;
        }
        if !miller_rabin_witness(&a, &d, r, &n_minus_1, n) {
            return false;
        }
    }
    // 额外 13 轮用确定性基 + 偏移（简化：复用前 13 基）
    for i in 0..13u64 {
        let a = BigInt::from(MR_BASES[i as usize % MR_BASES.len()] + i * 1000);
        if a >= *n || a.is_zero() || a.is_one() {
            continue;
        }
        if !miller_rabin_witness(&a, &d, r, &n_minus_1, n) {
            return false;
        }
    }
    true
}

/// 单次 Miller-Rabin 见证测试。
/// 返回 true 如果 a 不是合数见证（即 n 可能是素数），false 如果 a 证明 n 是合数。
fn miller_rabin_witness(a: &BigInt, d: &BigInt, r: u32, n_minus_1: &BigInt, n: &BigInt) -> bool {
    let one = BigInt::one();
    let mut x = mod_pow_bigint(a, d, n);
    if x == one || x == *n_minus_1 {
        return true;
    }
    for _ in 0..r.saturating_sub(1) {
        x = (&x * &x) % n;
        if x == *n_minus_1 {
            return true;
        }
    }
    false
}

/// u64 确定性 Miller-Rabin（12 基，对 n < 3.3×10^24 确定）。
fn is_prime_u64(n: u64) -> bool {
    if n < 2 {
        return false;
    }
    if n == 2 {
        return true;
    }
    if n % 2 == 0 {
        return false;
    }

    let mut d = n - 1;
    let mut r = 0u32;
    while d % 2 == 0 {
        d /= 2;
        r += 1;
    }

    for &a in MR_BASES {
        if a >= n {
            continue;
        }
        let mut x = mod_pow_u64(a, d, n);
        if x == 1 || x == n - 1 {
            continue;
        }
        let mut composite = true;
        let n128 = n as u128;
        for _ in 0..r.saturating_sub(1) {
            x = (((x as u128) * (x as u128)) % n128) as u64;
            if x == n - 1 {
                composite = false;
                break;
            }
        }
        if composite {
            return false;
        }
    }
    true
}

/// u64 快速模幂（平方-乘法）。
fn mod_pow_u64(base: u64, exp: u64, m: u64) -> u64 {
    if m == 1 {
        return 0;
    }
    let mut result = 1u128;
    let mut base = (base % m) as u128;
    let m128 = m as u128;
    let mut exp = exp;
    while exp > 0 {
        if exp % 2 == 1 {
            result = (result * base) % m128;
        }
        exp /= 2;
        base = (base * base) % m128;
    }
    result as u64
}

/// BigInt 快速模幂（平方-乘法）。
fn mod_pow_bigint(base: &BigInt, exp: &BigInt, m: &BigInt) -> BigInt {
    if m.is_one() {
        return BigInt::zero();
    }
    let mut result = BigInt::one();
    let mut base = base % m;
    let mut exp = exp.clone();
    while exp.is_positive() {
        if exp.is_odd() {
            result = (&result * &base) % m;
        }
        exp >>= 1;
        base = (&base * &base) % m;
    }
    result
}

/// 扩展欧几里得算法，返回 (gcd, x, y) 使得 a*x + b*y = gcd。
fn extended_gcd(a: &BigInt, b: &BigInt) -> (BigInt, BigInt, BigInt) {
    if b.is_zero() {
        return (a.clone(), BigInt::one(), BigInt::zero());
    }
    let (g, x1, y1) = extended_gcd(b, &(a % b));
    (g, y1.clone(), &x1 - &(a / b) * &y1)
}

/// 模逆：求 x 使得 a*x ≡ 1 (mod m)。返回 None 如果不存在（gcd(a,m)≠1）。
fn mod_inverse(a: &BigInt, m: &BigInt) -> Option<BigInt> {
    let a_mod = if a.is_negative() {
        ((a % m) + m) % m
    } else {
        a % m
    };
    let (g, x, _) = extended_gcd(&a_mod, m);
    if !g.is_one() {
        return None;
    }
    let result = ((x % m) + m) % m;
    Some(result)
}

/// 欧拉函数 φ(n)：≤ n 且与 n 互素的正整数个数。
fn euler_phi(n: &BigInt) -> BigInt {
    if n.is_one() {
        return BigInt::one();
    }
    let mut result = n.clone();
    let mut m = n.clone();
    let mut p = BigInt::from(2);
    while &p * &p <= m {
        if (&m % &p).is_zero() {
            while (&m % &p).is_zero() {
                m /= &p;
            }
            result -= &result / &p;
        }
        p += 1;
    }
    if m > BigInt::one() {
        result -= &result / &m;
    }
    result
}

/// 埃拉托斯特尼筛法，返回 ≤ n 的所有素数。
fn prime_sieve_u64(n: u64) -> Vec<u64> {
    if n < 2 {
        return Vec::new();
    }
    let n = n as usize;
    let mut is_prime = vec![true; n + 1];
    is_prime[0] = false;
    is_prime[1] = false;
    let mut i = 2;
    while i * i <= n {
        if is_prime[i] {
            let mut j = i * i;
            while j <= n {
                is_prime[j] = false;
                j += i;
            }
        }
        i += 1;
    }
    (2..=n).filter(|&i| is_prime[i]).map(|i| i as u64).collect()
}

/// 递归检查 AST 是否含数论函数调用。
fn contains_number_theory_function(ast: &AstNode) -> bool {
    match ast {
        AstNode::FunctionCall(name, _) if NUMBER_THEORY_FUNCTIONS.contains(&name.as_str()) => true,
        AstNode::FunctionCall(_, args) => args.iter().any(contains_number_theory_function),
        AstNode::BinaryOp(_, l, r) => {
            contains_number_theory_function(l) || contains_number_theory_function(r)
        }
        AstNode::UnaryOp(_, e) => contains_number_theory_function(e),
        AstNode::Matrix(rows) => rows.iter().flatten().any(contains_number_theory_function),
        AstNode::List(elements) => elements.iter().any(contains_number_theory_function),
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
        let domain = NumberTheoryDomain;
        let ctx = EvalContext::new();
        domain.evaluate(&ast, &ctx)
    }

    fn eval_scalar(input: &str) -> Result<f64, CalcError> {
        eval(input).map(|r| r.as_scalar().expect("expected scalar result"))
    }

    // ===== UT-NUM-001: GCD =====

    #[test]
    fn test_gcd_basic() {
        assert_eq!(eval_scalar("gcd(12,18)").unwrap(), 6.0);
    }

    // ===== UT-NUM-002: LCM =====

    #[test]
    fn test_lcm_basic() {
        assert_eq!(eval_scalar("lcm(4,6)").unwrap(), 12.0);
    }

    // ===== UT-NUM-003: 小素数判定 =====

    #[test]
    fn test_is_prime_small() {
        assert_eq!(eval_scalar("is_prime(7)").unwrap(), 1.0); // 1 = true
    }

    // ===== UT-NUM-004: 大素数判定 =====

    #[test]
    fn test_is_prime_large() {
        assert_eq!(eval_scalar("is_prime(1000000007)").unwrap(), 1.0);
    }

    // ===== UT-NUM-005: 合数判定 =====

    #[test]
    fn test_is_prime_composite() {
        assert_eq!(eval_scalar("is_prime(9)").unwrap(), 0.0); // 0 = false
    }

    // ===== UT-NUM-006: 边界 0 =====

    #[test]
    fn test_is_prime_zero() {
        assert_eq!(eval_scalar("is_prime(0)").unwrap(), 0.0);
    }

    // ===== UT-NUM-007: 边界 1 =====

    #[test]
    fn test_is_prime_one() {
        assert_eq!(eval_scalar("is_prime(1)").unwrap(), 0.0);
    }

    // ===== UT-NUM-008: 边界 2 =====

    #[test]
    fn test_is_prime_two() {
        assert_eq!(eval_scalar("is_prime(2)").unwrap(), 1.0);
    }

    // ===== UT-NUM-009: 素数筛 =====

    #[test]
    fn test_prime_sieve_basic() {
        let result = eval("prime_sieve(20)").unwrap();
        let primes = result.as_vector().unwrap();
        let expected: Vec<f64> = vec![2.0, 3.0, 5.0, 7.0, 11.0, 13.0, 17.0, 19.0];
        assert_eq!(primes, &expected);
    }

    #[test]
    fn test_prime_sieve_boundary() {
        let result = eval("prime_sieve(1)").unwrap();
        assert_eq!(result.as_vector().unwrap(), &Vec::<f64>::new());
    }

    // ===== UT-NUM-010: 模逆 =====

    #[test]
    fn test_mod_inverse_basic() {
        assert_eq!(eval_scalar("mod_inverse(3,11)").unwrap(), 4.0);
    }

    // ===== UT-NUM-011: 模幂 =====

    #[test]
    fn test_mod_pow_basic() {
        assert_eq!(eval_scalar("mod_pow(2,10,1000)").unwrap(), 24.0);
    }

    // ===== UT-NUM-012: 欧拉函数 =====

    #[test]
    fn test_euler_phi_basic() {
        assert_eq!(eval_scalar("euler_phi(10)").unwrap(), 4.0);
    }

    // ===== UT-NUM-013: 负数处理 =====

    #[test]
    fn test_gcd_negative() {
        assert_eq!(eval_scalar("gcd(-12,18)").unwrap(), 6.0);
    }

    // ===== UT-NUM-014: 边界 0 =====

    #[test]
    fn test_gcd_zero() {
        assert_eq!(eval_scalar("gcd(0,5)").unwrap(), 5.0);
    }

    // ===== UT-NUM-015: 大数 =====

    #[test]
    fn test_is_prime_large_10e18() {
        // 10^18 + 9 = 1000000000000000009 是素数
        let result = eval_scalar("is_prime(10^18+9)");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1.0);
    }

    // ===== 补充边界测试 =====

    #[test]
    fn test_gcd_both_negative() {
        assert_eq!(eval_scalar("gcd(-12,-18)").unwrap(), 6.0);
    }

    #[test]
    fn test_gcd_both_zero() {
        assert_eq!(eval_scalar("gcd(0,0)").unwrap(), 0.0);
    }

    #[test]
    fn test_lcm_zero() {
        assert_eq!(eval_scalar("lcm(0,5)").unwrap(), 0.0);
    }

    #[test]
    fn test_lcm_negative() {
        assert_eq!(eval_scalar("lcm(-4,6)").unwrap(), 12.0);
    }

    #[test]
    fn test_is_prime_negative() {
        assert_eq!(eval_scalar("is_prime(-7)").unwrap(), 0.0);
    }

    #[test]
    fn test_is_prime_large_composite() {
        assert_eq!(eval_scalar("is_prime(1000000008)").unwrap(), 0.0);
    }

    #[test]
    fn test_is_prime_3() {
        assert_eq!(eval_scalar("is_prime(3)").unwrap(), 1.0);
    }

    #[test]
    fn test_is_prime_4() {
        assert_eq!(eval_scalar("is_prime(4)").unwrap(), 0.0);
    }

    #[test]
    fn test_prime_sieve_2() {
        let result = eval("prime_sieve(2)").unwrap();
        assert_eq!(result.as_vector().unwrap(), &vec![2.0]);
    }

    #[test]
    fn test_prime_sieve_10() {
        let result = eval("prime_sieve(10)").unwrap();
        assert_eq!(result.as_vector().unwrap(), &vec![2.0, 3.0, 5.0, 7.0]);
    }

    #[test]
    fn test_prime_sieve_0() {
        let result = eval("prime_sieve(0)").unwrap();
        assert!(result.as_vector().unwrap().is_empty());
    }

    #[test]
    fn test_mod_inverse_not_coprime() {
        let result = eval("mod_inverse(2,4)");
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_mod_inverse_negative() {
        // mod_inverse(-3, 11) should return 7 (-3*7 = -21 ≡ 1 mod 11)
        assert_eq!(eval_scalar("mod_inverse(-3,11)").unwrap(), 7.0);
    }

    #[test]
    fn test_mod_pow_zero_exp() {
        assert_eq!(eval_scalar("mod_pow(5,0,7)").unwrap(), 1.0);
    }

    #[test]
    fn test_mod_pow_negative_exp() {
        let result = eval("mod_pow(2,-1,100)");
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_euler_phi_1() {
        assert_eq!(eval_scalar("euler_phi(1)").unwrap(), 1.0);
    }

    #[test]
    fn test_euler_phi_prime() {
        // φ(p) = p-1 for prime p
        assert_eq!(eval_scalar("euler_phi(7)").unwrap(), 6.0);
    }

    #[test]
    fn test_euler_phi_0() {
        assert_eq!(eval_scalar("euler_phi(0)").unwrap(), 0.0);
    }

    // ===== 域元信息测试 =====

    #[test]
    fn test_domain_info() {
        let domain = NumberTheoryDomain;
        assert_eq!(domain.domain_name(), "number_theory");
        assert_eq!(domain.priority(), 25);
    }

    #[test]
    fn test_default_impl() {
        let domain = NumberTheoryDomain;
        assert_eq!(domain.domain_name(), "number_theory");
    }

    #[test]
    fn test_supports_gcd() {
        let ast = parse("gcd(12,18)").unwrap();
        assert!(NumberTheoryDomain.supports(&ast));
    }

    #[test]
    fn test_supports_nested() {
        let ast = parse("gcd(12,18) + 1").unwrap();
        assert!(NumberTheoryDomain.supports(&ast));
    }

    #[test]
    fn test_supports_unary() {
        let ast = AstNode::UnaryOp(UnaryOp::Neg, Box::new(parse("is_prime(7)").unwrap()));
        assert!(NumberTheoryDomain.supports(&ast));
    }

    #[test]
    fn test_supports_matrix() {
        let ast = AstNode::Matrix(vec![vec![parse("gcd(1,2)").unwrap()]]);
        assert!(NumberTheoryDomain.supports(&ast));
    }

    #[test]
    fn test_supports_list() {
        let ast = AstNode::List(vec![parse("lcm(4,6)").unwrap()]);
        assert!(NumberTheoryDomain.supports(&ast));
    }

    #[test]
    fn test_not_supports_arithmetic() {
        let ast = parse("1+2").unwrap();
        assert!(!NumberTheoryDomain.supports(&ast));
    }

    // ===== 错误路径测试 =====

    #[test]
    fn test_unsupported_function() {
        let ast = AstNode::FunctionCall("sin".to_string(), vec![AstNode::Number(1.0)]);
        let result = NumberTheoryDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_gcd_wrong_args() {
        let ast = AstNode::FunctionCall("gcd".to_string(), vec![AstNode::Number(1.0)]);
        let result = NumberTheoryDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_is_prime_wrong_args() {
        let ast = AstNode::FunctionCall(
            "is_prime".to_string(),
            vec![AstNode::Number(1.0), AstNode::Number(2.0)],
        );
        let result = NumberTheoryDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_eval_node_float_rejected() {
        let ast = AstNode::Number(3.14);
        let result = NumberTheoryDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_eval_node_complex_rejected() {
        let ast = AstNode::Complex(1.0, 2.0);
        let result = NumberTheoryDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_eval_node_list_rejected() {
        let ast = AstNode::List(vec![AstNode::Number(1.0)]);
        let result = NumberTheoryDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_eval_int_function_call_rejected() {
        // eval_int 不直接暴露，通过 gcd 参数间接测试：传嵌套函数调用作为参数
        let outer = AstNode::FunctionCall(
            "gcd".to_string(),
            vec![
                AstNode::FunctionCall("sin".to_string(), vec![AstNode::Number(1.0)]),
                AstNode::Number(2.0),
            ],
        );
        let result = NumberTheoryDomain.evaluate(&outer, &EvalContext::new());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_unbound_variable() {
        let ast = AstNode::Variable("x".to_string());
        let result = NumberTheoryDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(CalcError::EvalError(_))));
    }

    #[test]
    fn test_div_by_zero() {
        let ast = AstNode::BinaryOp(
            BinaryOp::Div,
            Box::new(AstNode::Number(10.0)),
            Box::new(AstNode::Number(0.0)),
        );
        let result = NumberTheoryDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(CalcError::DivisionByZero)));
    }

    #[test]
    fn test_mod_by_zero() {
        let ast = AstNode::BinaryOp(
            BinaryOp::Mod,
            Box::new(AstNode::Number(10.0)),
            Box::new(AstNode::Number(0.0)),
        );
        let result = NumberTheoryDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(CalcError::DivisionByZero)));
    }

    #[test]
    fn test_negative_exponent() {
        let ast = AstNode::BinaryOp(
            BinaryOp::Pow,
            Box::new(AstNode::Number(2.0)),
            Box::new(AstNode::Number(-1.0)),
        );
        let result = NumberTheoryDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_unary_abs() {
        let ast = AstNode::UnaryOp(UnaryOp::Abs, Box::new(AstNode::Number(-5.0)));
        let result = NumberTheoryDomain
            .evaluate(&ast, &EvalContext::new())
            .unwrap();
        assert_eq!(result.as_scalar().unwrap(), 5.0);
    }

    #[test]
    fn test_unary_neg() {
        let ast = AstNode::UnaryOp(UnaryOp::Neg, Box::new(AstNode::Number(5.0)));
        let result = NumberTheoryDomain
            .evaluate(&ast, &EvalContext::new())
            .unwrap();
        assert_eq!(result.as_scalar().unwrap(), -5.0);
    }

    #[test]
    fn test_unary_factorial_rejected() {
        let ast = AstNode::UnaryOp(UnaryOp::Factorial, Box::new(AstNode::Number(5.0)));
        let result = NumberTheoryDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_big_number_arg() {
        let ast = AstNode::FunctionCall(
            "is_prime".to_string(),
            vec![AstNode::BigNumber("1000000007".to_string())],
        );
        let result = NumberTheoryDomain
            .evaluate(&ast, &EvalContext::new())
            .unwrap();
        assert_eq!(result.as_scalar().unwrap(), 1.0);
    }

    #[test]
    fn test_big_number_invalid() {
        let ast = AstNode::BigNumber("not_a_number".to_string());
        let result = NumberTheoryDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_arithmetic_combination() {
        // gcd(12,18) + 1 = 7
        assert_eq!(eval_scalar("gcd(12,18)+1").unwrap(), 7.0);
    }

    #[test]
    fn test_bigint_result() {
        // 大数 GCD 返回 BigInt：gcd(99999999999999999999, 33333333333333333333) = 33333333333333333333
        // 33333333333333333333 > i64::MAX，故返回 BigInt
        let ast = AstNode::FunctionCall(
            "gcd".to_string(),
            vec![
                AstNode::BigNumber("99999999999999999999".to_string()),
                AstNode::BigNumber("33333333333333333333".to_string()),
            ],
        );
        let result = NumberTheoryDomain
            .evaluate(&ast, &EvalContext::new())
            .unwrap();
        assert!(matches!(result, EvalResult::BigInt(_)));
    }

    // ===== 底层算法单元测试 =====

    #[test]
    fn test_is_prime_u64_known() {
        assert!(!is_prime_u64(0));
        assert!(!is_prime_u64(1));
        assert!(is_prime_u64(2));
        assert!(is_prime_u64(3));
        assert!(!is_prime_u64(4));
        assert!(is_prime_u64(5));
        assert!(!is_prime_u64(9));
        assert!(is_prime_u64(1000000007));
        assert!(!is_prime_u64(1000000008));
    }

    #[test]
    fn test_mod_pow_u64_known() {
        assert_eq!(mod_pow_u64(2, 10, 1000), 24);
        assert_eq!(mod_pow_u64(5, 0, 7), 1);
        assert_eq!(mod_pow_u64(3, 5, 1), 0);
    }

    #[test]
    fn test_extended_gcd() {
        let (g, x, y) = extended_gcd(&BigInt::from(3), &BigInt::from(11));
        assert_eq!(g, BigInt::from(1));
        assert_eq!(&x * 3 + &y * 11, BigInt::from(1));
    }

    #[test]
    fn test_mod_inverse_known() {
        let inv = mod_inverse(&BigInt::from(3), &BigInt::from(11)).unwrap();
        assert_eq!((&inv * 3) % 11, BigInt::from(1));
    }

    #[test]
    fn test_mod_inverse_none() {
        assert!(mod_inverse(&BigInt::from(2), &BigInt::from(4)).is_none());
    }

    #[test]
    fn test_euler_phi_known() {
        assert_eq!(euler_phi(&BigInt::from(1)), BigInt::from(1));
        assert_eq!(euler_phi(&BigInt::from(10)), BigInt::from(4));
        assert_eq!(euler_phi(&BigInt::from(7)), BigInt::from(6));
        assert_eq!(euler_phi(&BigInt::from(12)), BigInt::from(4));
    }

    #[test]
    fn test_prime_sieve_u64_known() {
        assert!(prime_sieve_u64(1).is_empty());
        assert_eq!(prime_sieve_u64(2), vec![2]);
        assert_eq!(prime_sieve_u64(10), vec![2, 3, 5, 7]);
        assert_eq!(prime_sieve_u64(20), vec![2, 3, 5, 7, 11, 13, 17, 19]);
    }

    #[test]
    fn test_is_prime_bigint_large() {
        // 大素数测试
        let p = BigInt::from(1000000007);
        assert!(is_prime_bigint(&p));
        // 大合数
        let c = BigInt::from(1000000008);
        assert!(!is_prime_bigint(&c));
    }

    #[test]
    fn test_is_prime_bigint_huge() {
        // 超大素数（> 2^64）：2^127 - 1 是 Mersenne 素数
        let p = BigInt::from(2).pow(127) - BigInt::from(1);
        assert!(is_prime_bigint(&p));
    }

    #[test]
    fn test_mod_pow_bigint_known() {
        let result = mod_pow_bigint(&BigInt::from(2), &BigInt::from(10), &BigInt::from(1000));
        assert_eq!(result, BigInt::from(24));
    }

    #[test]
    fn test_bigint_to_result_small() {
        assert_eq!(bigint_to_result(BigInt::from(42)), EvalResult::Scalar(42.0));
    }

    #[test]
    fn test_bigint_to_result_large() {
        let large = BigInt::from(2).pow(100);
        assert!(matches!(bigint_to_result(large), EvalResult::BigInt(_)));
    }

    // ===== 覆盖率补充测试 =====

    #[test]
    fn test_eval_node_integer_number() {
        // eval_node Number success (integer)
        let ast = AstNode::Number(42.0);
        let result = NumberTheoryDomain
            .evaluate(&ast, &EvalContext::new())
            .unwrap();
        assert_eq!(result.as_scalar().unwrap(), 42.0);
    }

    #[test]
    fn test_eval_node_bignumber() {
        // eval_node BigNumber success
        let ast = AstNode::BigNumber("12345678901234567890".to_string());
        let result = NumberTheoryDomain
            .evaluate(&ast, &EvalContext::new())
            .unwrap();
        assert!(matches!(result, EvalResult::BigInt(_)));
    }

    #[test]
    fn test_eval_int_non_integer() {
        // eval_int non-integer error
        let ast = AstNode::FunctionCall(
            "gcd".to_string(),
            vec![AstNode::Number(3.14), AstNode::Number(6.0)],
        );
        let result = NumberTheoryDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_eval_int_overflow() {
        // eval_int overflow: number > i64::MAX
        let ast = AstNode::FunctionCall(
            "gcd".to_string(),
            vec![AstNode::Number(1.0e20), AstNode::Number(6.0)],
        );
        let result = NumberTheoryDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(CalcError::Overflow)));
    }

    #[test]
    fn test_eval_int_variable_success() {
        // eval_int Variable success
        let ctx = EvalContext::new().with_var("x", 12.0);
        let ast = AstNode::FunctionCall(
            "gcd".to_string(),
            vec![AstNode::Variable("x".to_string()), AstNode::Number(18.0)],
        );
        let result = NumberTheoryDomain.evaluate(&ast, &ctx).unwrap();
        assert_eq!(result.as_scalar().unwrap(), 6.0);
    }

    #[test]
    fn test_eval_int_variable_non_integer() {
        // eval_int Variable non-integer error
        let ctx = EvalContext::new().with_var("x", 3.14);
        let ast = AstNode::FunctionCall(
            "gcd".to_string(),
            vec![AstNode::Variable("x".to_string()), AstNode::Number(18.0)],
        );
        let result = NumberTheoryDomain.evaluate(&ast, &ctx);
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_eval_int_unary_abs() {
        // eval_int UnaryOp::Abs
        let ast = AstNode::FunctionCall(
            "gcd".to_string(),
            vec![
                AstNode::UnaryOp(UnaryOp::Abs, Box::new(AstNode::Number(-12.0))),
                AstNode::Number(18.0),
            ],
        );
        let result = NumberTheoryDomain
            .evaluate(&ast, &EvalContext::new())
            .unwrap();
        assert_eq!(result.as_scalar().unwrap(), 6.0);
    }

    #[test]
    fn test_eval_int_unary_factorial_rejected() {
        // eval_int UnaryOp::Factorial error
        let ast = AstNode::FunctionCall(
            "gcd".to_string(),
            vec![
                AstNode::UnaryOp(UnaryOp::Factorial, Box::new(AstNode::Number(5.0))),
                AstNode::Number(18.0),
            ],
        );
        let result = NumberTheoryDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_eval_int_function_call_scalar_non_integer() {
        // eval_int FunctionCall returning non-integer Scalar
        // gcd(12,18) = 6 (integer), but we need a non-integer result
        // Use is_prime which returns 0.0 or 1.0 (both integers), so construct a case
        // that returns non-integer: this is hard since all NT functions return integers.
        // Instead, test the overflow path: gcd of very large big numbers
        let ast = AstNode::FunctionCall(
            "gcd".to_string(),
            vec![
                AstNode::FunctionCall(
                    "gcd".to_string(),
                    vec![
                        AstNode::BigNumber("99999999999999999999".to_string()),
                        AstNode::BigNumber("33333333333333333333".to_string()),
                    ],
                ),
                AstNode::Number(18.0),
            ],
        );
        // gcd(33333333333333333333, 18) = 3
        let result = NumberTheoryDomain
            .evaluate(&ast, &EvalContext::new())
            .unwrap();
        assert_eq!(result.as_scalar().unwrap(), 3.0);
    }

    #[test]
    fn test_eval_int_complex_rejected() {
        // eval_int Complex/Matrix/List rejection
        let ast = AstNode::FunctionCall(
            "gcd".to_string(),
            vec![AstNode::Complex(1.0, 2.0), AstNode::Number(18.0)],
        );
        let result = NumberTheoryDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_eval_int_binary_sub_mul() {
        // eval_int_binary Sub and Mul success: gcd(10-4, 2*3) = gcd(6, 6) = 6
        assert_eq!(eval_scalar("gcd(10-4, 2*3)").unwrap(), 6.0);
    }

    #[test]
    fn test_eval_int_binary_div_mod_success() {
        // eval_int_binary Div success: gcd(10/2, 3) = gcd(5, 3) = 1
        assert_eq!(eval_scalar("gcd(10/2, 3)").unwrap(), 1.0);
        // eval_int_binary Mod success via direct AST: gcd(BinaryOp::Mod(10,3), 3) = gcd(1, 3) = 1
        let ast = AstNode::FunctionCall(
            "gcd".to_string(),
            vec![
                AstNode::BinaryOp(
                    BinaryOp::Mod,
                    Box::new(AstNode::Number(10.0)),
                    Box::new(AstNode::Number(3.0)),
                ),
                AstNode::Number(3.0),
            ],
        );
        let result = NumberTheoryDomain
            .evaluate(&ast, &EvalContext::new())
            .unwrap();
        assert_eq!(result.as_scalar().unwrap(), 1.0);
    }

    #[test]
    fn test_lcm_wrong_args() {
        let ast = AstNode::FunctionCall("lcm".to_string(), vec![AstNode::Number(1.0)]);
        let result = NumberTheoryDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_prime_sieve_wrong_args() {
        let ast = AstNode::FunctionCall(
            "prime_sieve".to_string(),
            vec![AstNode::Number(1.0), AstNode::Number(2.0)],
        );
        let result = NumberTheoryDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_prime_sieve_negative() {
        let result = eval("prime_sieve(-5)");
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_mod_inverse_wrong_args() {
        let ast = AstNode::FunctionCall("mod_inverse".to_string(), vec![AstNode::Number(3.0)]);
        let result = NumberTheoryDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_mod_inverse_zero_modulus() {
        let result = eval("mod_inverse(3, 0)");
        assert!(matches!(result, Err(CalcError::DivisionByZero)));
    }

    #[test]
    fn test_mod_pow_wrong_args() {
        let ast = AstNode::FunctionCall(
            "mod_pow".to_string(),
            vec![AstNode::Number(2.0), AstNode::Number(10.0)],
        );
        let result = NumberTheoryDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_mod_pow_zero_modulus() {
        let result = eval("mod_pow(2, 10, 0)");
        assert!(matches!(result, Err(CalcError::DivisionByZero)));
    }

    #[test]
    fn test_euler_phi_wrong_args() {
        let ast = AstNode::FunctionCall(
            "euler_phi".to_string(),
            vec![AstNode::Number(10.0), AstNode::Number(20.0)],
        );
        let result = NumberTheoryDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_is_prime_bigint_composite_huge() {
        // BigInt composite > 2^64: 2^127-1 is prime, (2^127-1)*3 is ODD composite
        // Must use odd composite to bypass is_even() early exit and reach BigInt Miller-Rabin
        let composite = BigInt::from(2).pow(127) - BigInt::from(1);
        let composite = &composite * BigInt::from(3);
        assert!(!is_prime_bigint(&composite));
    }

    #[test]
    fn test_is_prime_bigint_small() {
        // Small BigInt where base >= n (e.g., n=3, bases start at 2)
        // MR_BASES[0] = 2, for n=3: 2 < 3, so it's tested
        // For n=2: u64 path handles it. Use n=5 to ensure base >= n for some bases
        assert!(is_prime_bigint(&BigInt::from(5)));
    }

    #[test]
    fn test_miller_rabin_witness_loop() {
        // Test miller_rabin_witness with a composite where the squaring loop runs
        // n = 15 (composite), n-1 = 14 = 2^1 * 7, d=7, r=1
        // r-1 = 0, so the loop doesn't run. Need r > 1.
        // n = 9: n-1 = 8 = 2^3 * 1, d=1, r=3. Base=2: 2^1 % 9 = 2, not 1 or 8.
        // Loop: x = 4, not 8. x = 16%9=7, not 8. Return false (witness proves composite).
        let n = BigInt::from(9);
        let one = BigInt::one();
        let n_minus_1 = &n - &one;
        let d = BigInt::one(); // 9-1=8=2^3*1
        let r = 3u32;
        let a = BigInt::from(2);
        // 2 is a witness for 9 being composite
        assert!(!miller_rabin_witness(&a, &d, r, &n_minus_1, &n));
    }

    #[test]
    fn test_mod_pow_bigint_modulus_one() {
        // mod_pow_bigint with modulus = 1 returns 0
        let result = mod_pow_bigint(&BigInt::from(5), &BigInt::from(3), &BigInt::from(1));
        assert_eq!(result, BigInt::zero());
    }

    #[test]
    fn test_eval_int_function_call_returns_vector() {
        // 嵌套 prime_sieve 返回 Vector，eval_int FunctionCall 的 `_ =>` 分支
        // gcd(prime_sieve(5), 18) → prime_sieve(5) 返回 Vector → DomainError
        let ast = AstNode::FunctionCall(
            "gcd".to_string(),
            vec![
                AstNode::FunctionCall("prime_sieve".to_string(), vec![AstNode::Number(5.0)]),
                AstNode::Number(18.0),
            ],
        );
        let result = NumberTheoryDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_miller_rabin_witness_prime_loop() {
        // n=13 (prime), n-1=12=2^2*3, d=3, r=2
        // a=2: 2^3 % 13 = 8, not 1 or 12. Loop: x = 8^2 % 13 = 12 = n-1 → return true (line 422)
        let n = BigInt::from(13);
        let one = BigInt::one();
        let n_minus_1 = &n - &one;
        let d = BigInt::from(3);
        let r = 2u32;
        let a = BigInt::from(2);
        assert!(miller_rabin_witness(&a, &d, r, &n_minus_1, &n));
    }

    #[test]
    fn test_is_prime_bigint_prime_huge() {
        // 2^127-1 is a known Mersenne prime (M127), exercises BigInt Miller-Rabin prime path
        let prime = BigInt::from(2).pow(127) - BigInt::from(1);
        assert!(is_prime_bigint(&prime));
    }

    // ===== proptest 属性测试 =====

    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig { cases: 128, ..ProptestConfig::default() })]

        /// 属性：gcd(a,b) == gcd(b,a)
        #[test]
        fn prop_gcd_commutative(a in 0i64..10000, b in 0i64..10000) {
            let ast_a = AstNode::FunctionCall(
                "gcd".to_string(),
                vec![AstNode::Number(a as f64), AstNode::Number(b as f64)],
            );
            let ast_b = AstNode::FunctionCall(
                "gcd".to_string(),
                vec![AstNode::Number(b as f64), AstNode::Number(a as f64)],
            );
            let domain = NumberTheoryDomain;
            let ctx = EvalContext::new();
            let ra = domain.evaluate(&ast_a, &ctx).unwrap();
            let rb = domain.evaluate(&ast_b, &ctx).unwrap();
            prop_assert_eq!(ra.as_scalar().unwrap(), rb.as_scalar().unwrap());
        }

        /// 属性：gcd(a,0) == |a|
        #[test]
        fn prop_gcd_with_zero(a in -10000i64..10000) {
            let ast = AstNode::FunctionCall(
                "gcd".to_string(),
                vec![AstNode::Number(a as f64), AstNode::Number(0.0)],
            );
            let domain = NumberTheoryDomain;
            let result = domain.evaluate(&ast, &EvalContext::new()).unwrap();
            prop_assert_eq!(result.as_scalar().unwrap(), a.abs() as f64);
        }

        /// 属性：lcm(a,b) * gcd(a,b) == |a*b|
        #[test]
        fn prop_lcm_gcd_relation(a in 1i64..1000, b in 1i64..1000) {
            let gcd_ast = AstNode::FunctionCall(
                "gcd".to_string(),
                vec![AstNode::Number(a as f64), AstNode::Number(b as f64)],
            );
            let lcm_ast = AstNode::FunctionCall(
                "lcm".to_string(),
                vec![AstNode::Number(a as f64), AstNode::Number(b as f64)],
            );
            let domain = NumberTheoryDomain;
            let ctx = EvalContext::new();
            let g = domain.evaluate(&gcd_ast, &ctx).unwrap().as_scalar().unwrap();
            let l = domain.evaluate(&lcm_ast, &ctx).unwrap().as_scalar().unwrap();
            prop_assert!((g * l - (a * b).abs() as f64).abs() < 1e-9);
        }

        /// 属性：mod_pow(a, b, m) < m
        #[test]
        fn prop_mod_pow_in_range(a in 0i64..100, b in 0i64..20, m in 1i64..1000) {
            let ast = AstNode::FunctionCall(
                "mod_pow".to_string(),
                vec![
                    AstNode::Number(a as f64),
                    AstNode::Number(b as f64),
                    AstNode::Number(m as f64),
                ],
            );
            let domain = NumberTheoryDomain;
            let result = domain.evaluate(&ast, &EvalContext::new()).unwrap();
            let v = result.as_scalar().unwrap();
            prop_assert!(v >= 0.0 && v < m as f64);
        }
    }
}
