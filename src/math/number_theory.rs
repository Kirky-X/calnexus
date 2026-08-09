// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! 数论核心函数：GCD/LCM/素数判定/素数筛/模逆/模幂/欧拉函数。
//!
//! 从 `domains/number_theory.rs` 提取的纯数学逻辑，
//! 供 `domains/`（AST 求值路径）和 `api/`（直接 API 路径）共用。

use num_bigint::BigInt;
use num_integer::Integer as _;
use num_traits::{One, Signed, ToPrimitive, Zero};

use crate::core::CalcError;

/// Miller-Rabin 确定性基（n < 3.3×10^24 时确定性判定）。
const MR_BASES: &[u64] = &[2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37];

/// 筛法上界（~10MB 内存）。
const MAX_SIEVE_N: u64 = 10_000_000;

// ===== 公开 API =====

/// 最大公约数：`gcd(a, b)`。返回 `|a|` 和 `|b|` 的 GCD。
pub fn gcd(a: &BigInt, b: &BigInt) -> BigInt {
    a.abs().gcd(&b.abs())
}

/// 最小公倍数：`lcm(a, b)`。任一为 0 返回 0。
pub fn lcm(a: &BigInt, b: &BigInt) -> BigInt {
    if a.is_zero() || b.is_zero() {
        return BigInt::zero();
    }
    a.abs().lcm(&b.abs())
}

/// 素数判定：Miller-Rabin 算法，返回 `true` 如果 `n` 是素数。
pub fn is_prime(n: &BigInt) -> bool {
    is_prime_bigint(n)
}

/// 埃拉托斯特尼筛法：返回 ≤ `n` 的所有素数。
///
/// - `n > MAX_SIEVE_N` → `Overflow`
/// - `n < 0` → `Domain`
pub fn prime_sieve(n: &BigInt) -> Result<Vec<u64>, CalcError> {
    if n.is_negative() {
        return Err(CalcError::domain(
            "prime_sieve() requires non-negative argument".to_string(),
        ));
    }
    let n_u64 = n.to_u64().ok_or(CalcError::overflow())?;
    if n_u64 > MAX_SIEVE_N {
        return Err(CalcError::overflow());
    }
    Ok(prime_sieve_u64(n_u64))
}

/// 模逆元：求 `x` 使得 `a * x ≡ 1 (mod m)`。
///
/// - `m == 0` → `DivisionByZero`
/// - `gcd(a, m) ≠ 1` → `Domain`
pub fn mod_inverse(a: &BigInt, m: &BigInt) -> Result<BigInt, CalcError> {
    if m.is_zero() {
        return Err(CalcError::division_by_zero());
    }
    let m_abs = m.abs();
    match mod_inverse_impl(a, &m_abs) {
        Some(inv) => Ok(inv),
        None => Err(CalcError::domain(format!(
            "mod_inverse: {} and {} are not coprime",
            a, m
        ))),
    }
}

/// 模幂运算：`base^exp mod m`。
///
/// - `m == 0` → `DivisionByZero`
/// - `exp < 0` → `Domain`
pub fn mod_pow(base: &BigInt, exp: &BigInt, m: &BigInt) -> Result<BigInt, CalcError> {
    if m.is_zero() {
        return Err(CalcError::division_by_zero());
    }
    if exp.is_negative() {
        return Err(CalcError::domain(
            "mod_pow() requires non-negative exponent".to_string(),
        ));
    }
    let m_abs = m.abs();
    Ok(mod_pow_bigint(base, exp, &m_abs))
}

/// 欧拉函数 φ(n)：≤ n 且与 n 互素的正整数个数。n=0 时返回 0。
pub fn euler_phi(n: &BigInt) -> BigInt {
    if n.is_zero() {
        return BigInt::zero();
    }
    euler_phi_impl(&n.abs())
}

/// 中国剩余定理：求解同余方程组 x ≡ rᵢ (mod mᵢ)。
///
/// 返回最小非负解。模数不必两两互素——不兼容时返回 `DomainError`。
pub fn crt(remainders: &[BigInt], moduli: &[BigInt]) -> Result<BigInt, CalcError> {
    if remainders.is_empty() || moduli.is_empty() {
        return Err(CalcError::domain(
            "crt(): empty input".to_string(),
        ));
    }
    if remainders.len() != moduli.len() {
        return Err(CalcError::domain(
            "crt(): remainders and moduli length mismatch".to_string(),
        ));
    }
    if moduli.iter().any(|m| m.is_zero()) {
        return Err(CalcError::domain(
            "crt(): zero modulus".to_string(),
        ));
    }
    // 迭代两两合并
    let mut cur_r = ((&remainders[0] % &moduli[0]) + &moduli[0]) % &moduli[0];
    let mut cur_m = moduli[0].abs();
    for i in 1..remainders.len() {
        let r_i = ((&remainders[i] % &moduli[i]) + &moduli[i]) % &moduli[i];
        let m_i = moduli[i].abs();
        let (g, s, _) = extended_gcd(&cur_m, &m_i);
        let diff = &r_i - &cur_r;
        if !(&diff % &g).is_zero() {
            return Err(CalcError::domain(format!(
                "crt(): incompatible congruences at index {} (gcd={} does not divide diff={})",
                i, g, diff
            )));
        }
        let lcm = &cur_m / &g * &m_i;
        // cur_r + cur_m * ((diff/g * s) mod (lcm/cur_m))
        let step = (&diff / &g * &s) % &m_i;
        cur_r = ((&cur_r + &cur_m * &step) % &lcm + &lcm) % &lcm;
        cur_m = lcm;
    }
    Ok(cur_r)
}

/// 离散对数（Baby-step Giant-step）：求最小非负 x 使得 g^x ≡ h (mod p)。
///
/// - p 须为素数，g ≠ 0
/// - O(√p) 时间和空间
pub fn discrete_log(g: &BigInt, h: &BigInt, p: &BigInt) -> Result<BigInt, CalcError> {
    if p.is_zero() {
        return Err(CalcError::division_by_zero());
    }
    if !is_prime(p) {
        return Err(CalcError::domain(format!(
            "discrete_log(): modulus {} is not prime", p
        )));
    }
    if g.is_zero() {
        return Err(CalcError::domain(
            "discrete_log(): base g must not be zero".to_string(),
        ));
    }
    let p_abs = p.abs();
    let one = BigInt::one();
    let h_mod = ((h % &p_abs) + &p_abs) % &p_abs;
    let g_mod = ((g % &p_abs) + &p_abs) % &p_abs;

    // m = ⌈√p⌉
    let m = sqrt_bigint(&p_abs) + &one;
    let m_usize = m.to_usize().ok_or_else(|| {
        CalcError::overflow()
    })?;

    // Baby step: table[g^j mod p] = j for j in 0..m
    let mut table = std::collections::HashMap::new();
    let mut power = BigInt::one();
    for j in 0..m_usize {
        table.insert(power.clone(), j);
        power = (&power * &g_mod) % &p_abs;
    }
    // Giant step factor: g^(-m) mod p
    let g_m = mod_pow_bigint(&g_mod, &m, &p_abs);
    let g_m_inv = mod_inverse_impl(&g_m, &p_abs).ok_or_else(|| {
        CalcError::domain("discrete_log(): base not invertible mod p".to_string())
    })?;

    // Giant step: check h * (g^(-m))^i for i in 0..m
    let mut gamma = h_mod.clone();
    for i in 0..m_usize {
        if let Some(&j) = table.get(&gamma) {
            return Ok(BigInt::from(i) * &m + BigInt::from(j));
        }
        gamma = (&gamma * &g_m_inv) % &p_abs;
    }
    Err(CalcError::domain(
        "discrete_log(): no solution found".to_string(),
    ))
}

/// BigInt 整数平方根（⌊√n⌋）。
fn sqrt_bigint(n: &BigInt) -> BigInt {
    if n.is_negative() {
        return BigInt::zero();
    }
    if n < &BigInt::from(2) {
        return n.clone();
    }
    // 牛顿法
    let mut x = BigInt::from((n.to_u64().unwrap_or(u64::MAX) as f64).sqrt() as u64);
    let two = BigInt::from(2);
    loop {
        let x1 = (&x + n / &x) / &two;
        if x1 >= x {
            break;
        }
        x = x1;
    }
    // 确保 x² ≤ n < (x+1)²
    while &x * &x > *n {
        x -= 1;
    }
    x
}

// ===== 内部实现 =====

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
    // 额外 13 轮用确定性基 + 偏移
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
    // HIGH #36 修复：归一化 base 为非负值，防止负数模幂结果错误
    let mut base = ((base % m) + m) % m;
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

/// 模逆实现：求 x 使得 a*x ≡ 1 (mod m)。返回 None 如果 gcd(a,m)≠1。
fn mod_inverse_impl(a: &BigInt, m: &BigInt) -> Option<BigInt> {
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

/// 欧拉函数实现。
fn euler_phi_impl(n: &BigInt) -> BigInt {
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

#[cfg(test)]
mod tests {
    use super::*;

    // ===== gcd =====

    #[test]
    fn test_gcd_basic() {
        assert_eq!(gcd(&BigInt::from(12), &BigInt::from(18)), BigInt::from(6));
    }

    #[test]
    fn test_gcd_negative() {
        assert_eq!(gcd(&BigInt::from(-12), &BigInt::from(18)), BigInt::from(6));
    }

    #[test]
    fn test_gcd_both_negative() {
        assert_eq!(gcd(&BigInt::from(-12), &BigInt::from(-18)), BigInt::from(6));
    }

    #[test]
    fn test_gcd_zero() {
        assert_eq!(gcd(&BigInt::from(0), &BigInt::from(5)), BigInt::from(5));
    }

    #[test]
    fn test_gcd_both_zero() {
        assert_eq!(gcd(&BigInt::from(0), &BigInt::from(0)), BigInt::from(0));
    }

    // ===== lcm =====

    #[test]
    fn test_lcm_basic() {
        assert_eq!(lcm(&BigInt::from(4), &BigInt::from(6)), BigInt::from(12));
    }

    #[test]
    fn test_lcm_zero() {
        assert_eq!(lcm(&BigInt::from(0), &BigInt::from(5)), BigInt::from(0));
    }

    #[test]
    fn test_lcm_negative() {
        assert_eq!(lcm(&BigInt::from(-4), &BigInt::from(6)), BigInt::from(12));
    }

    // ===== is_prime =====

    #[test]
    fn test_is_prime_small() {
        assert!(is_prime(&BigInt::from(7)));
        assert!(!is_prime(&BigInt::from(9)));
    }

    #[test]
    fn test_is_prime_boundaries() {
        assert!(!is_prime(&BigInt::from(0)));
        assert!(!is_prime(&BigInt::from(1)));
        assert!(is_prime(&BigInt::from(2)));
        assert!(is_prime(&BigInt::from(3)));
        assert!(!is_prime(&BigInt::from(4)));
    }

    #[test]
    fn test_is_prime_large() {
        assert!(is_prime(&BigInt::from(1_000_000_007)));
        assert!(!is_prime(&BigInt::from(1_000_000_008)));
    }

    #[test]
    fn test_is_prime_negative() {
        assert!(!is_prime(&BigInt::from(-7)));
    }

    #[test]
    fn test_is_prime_huge() {
        // 2^127 - 1 is Mersenne prime
        let p = BigInt::from(2).pow(127) - BigInt::from(1);
        assert!(is_prime(&p));
    }

    // ===== prime_sieve =====

    #[test]
    fn test_prime_sieve_basic() {
        assert_eq!(
            prime_sieve(&BigInt::from(20)).unwrap(),
            vec![2, 3, 5, 7, 11, 13, 17, 19]
        );
    }

    #[test]
    fn test_prime_sieve_boundary() {
        assert!(prime_sieve(&BigInt::from(1)).unwrap().is_empty());
        assert_eq!(prime_sieve(&BigInt::from(2)).unwrap(), vec![2]);
    }

    #[test]
    fn test_prime_sieve_zero() {
        assert!(prime_sieve(&BigInt::from(0)).unwrap().is_empty());
    }

    #[test]
    fn test_prime_sieve_negative() {
        assert!(prime_sieve(&BigInt::from(-5)).is_err());
    }

    // ===== mod_inverse =====

    #[test]
    fn test_mod_inverse_basic() {
        let inv = mod_inverse(&BigInt::from(3), &BigInt::from(11)).unwrap();
        assert_eq!((&inv * 3) % 11, BigInt::from(1));
    }

    #[test]
    fn test_mod_inverse_negative() {
        let inv = mod_inverse(&BigInt::from(-3), &BigInt::from(11)).unwrap();
        // (-3) * inv ≡ 1 (mod 11)，验证 (inv * (-3) + k*11) == 1
        let check = ((&inv * BigInt::from(-3)) % BigInt::from(11) + BigInt::from(11))
            % BigInt::from(11);
        assert_eq!(check, BigInt::from(1));
    }

    #[test]
    fn test_mod_inverse_not_coprime() {
        assert!(mod_inverse(&BigInt::from(2), &BigInt::from(4)).is_err());
    }

    #[test]
    fn test_mod_inverse_zero_modulus() {
        assert!(mod_inverse(&BigInt::from(3), &BigInt::from(0)).is_err());
    }

    // ===== mod_pow =====

    #[test]
    fn test_mod_pow_basic() {
        let r = mod_pow(&BigInt::from(2), &BigInt::from(10), &BigInt::from(1000)).unwrap();
        assert_eq!(r, BigInt::from(24));
    }

    #[test]
    fn test_mod_pow_zero_exp() {
        let r = mod_pow(&BigInt::from(5), &BigInt::from(0), &BigInt::from(7)).unwrap();
        assert_eq!(r, BigInt::from(1));
    }

    #[test]
    fn test_mod_pow_negative_exp() {
        assert!(mod_pow(&BigInt::from(2), &BigInt::from(-1), &BigInt::from(100)).is_err());
    }

    #[test]
    fn test_mod_pow_zero_modulus() {
        assert!(mod_pow(&BigInt::from(2), &BigInt::from(10), &BigInt::from(0)).is_err());
    }

    #[test]
    fn test_mod_pow_modulus_one() {
        let r = mod_pow(&BigInt::from(5), &BigInt::from(3), &BigInt::from(1)).unwrap();
        assert_eq!(r, BigInt::from(0));
    }

    // ===== euler_phi =====

    #[test]
    fn test_euler_phi_one() {
        assert_eq!(euler_phi(&BigInt::from(1)), BigInt::from(1));
    }

    #[test]
    fn test_euler_phi_ten() {
        assert_eq!(euler_phi(&BigInt::from(10)), BigInt::from(4));
    }

    #[test]
    fn test_euler_phi_prime() {
        assert_eq!(euler_phi(&BigInt::from(7)), BigInt::from(6));
    }

    #[test]
    fn test_euler_phi_twelve() {
        assert_eq!(euler_phi(&BigInt::from(12)), BigInt::from(4));
    }

    #[test]
    fn test_euler_phi_zero() {
        assert_eq!(euler_phi(&BigInt::from(0)), BigInt::from(0));
    }

    // ===== crt =====

    #[test]
    fn test_crt_classic() {
        // x ≡ 2 (mod 3), x ≡ 3 (mod 5), x ≡ 2 (mod 7) → x = 23
        let r = vec![BigInt::from(2), BigInt::from(3), BigInt::from(2)];
        let m = vec![BigInt::from(3), BigInt::from(5), BigInt::from(7)];
        assert_eq!(crt(&r, &m).unwrap(), BigInt::from(23));
    }

    #[test]
    fn test_crt_single() {
        let r = vec![BigInt::from(3)];
        let m = vec![BigInt::from(7)];
        assert_eq!(crt(&r, &m).unwrap(), BigInt::from(3));
    }

    #[test]
    fn test_crt_non_coprime_compatible() {
        // x ≡ 1 (mod 2), x ≡ 3 (mod 4) → x = 3 (compatible: gcd(2,4)=2 divides 3-1=2)
        let r = vec![BigInt::from(1), BigInt::from(3)];
        let m = vec![BigInt::from(2), BigInt::from(4)];
        assert_eq!(crt(&r, &m).unwrap(), BigInt::from(3));
    }

    #[test]
    fn test_crt_incompatible() {
        // x ≡ 0 (mod 2), x ≡ 1 (mod 4) → no solution
        let r = vec![BigInt::from(0), BigInt::from(1)];
        let m = vec![BigInt::from(2), BigInt::from(4)];
        assert!(crt(&r, &m).is_err());
    }

    #[test]
    fn test_crt_empty() {
        assert!(crt(&[], &[]).is_err());
    }

    // ===== discrete_log =====

    #[test]
    fn test_discrete_log_basic() {
        // 2^x ≡ 8 (mod 11) → x = 3
        let x = discrete_log(&BigInt::from(2), &BigInt::from(8), &BigInt::from(11)).unwrap();
        assert_eq!(x, BigInt::from(3));
    }

    #[test]
    fn test_discrete_log_large() {
        // 2^x ≡ 5 (mod 13)
        let x = discrete_log(&BigInt::from(2), &BigInt::from(5), &BigInt::from(13)).unwrap();
        // verify: 2^x mod 13 == 5
        assert_eq!(mod_pow(&BigInt::from(2), &x, &BigInt::from(13)).unwrap(), BigInt::from(5));
    }

    #[test]
    fn test_discrete_log_zero_base() {
        assert!(discrete_log(&BigInt::from(0), &BigInt::from(1), &BigInt::from(7)).is_err());
    }

    #[test]
    fn test_discrete_log_non_prime_mod() {
        assert!(discrete_log(&BigInt::from(2), &BigInt::from(1), &BigInt::from(4)).is_err());
    }
}
