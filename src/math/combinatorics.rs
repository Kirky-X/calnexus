// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! 组合数学核心函数：排列/组合/Catalan 数/Stirling 数。
//!
//! 从 `domains/combinatorics.rs` 提取的纯数学逻辑，
//! 供 `domains/`（AST 求值路径）和 `api/`（直接 API 路径）共用。

use num_bigint::BigInt;
use num_traits::{One, ToPrimitive, Zero};

use crate::core::CalcError;

// ===== 公开 API =====

/// 排列数 P(n, k) = n!/(n-k)! = n*(n-1)*...*(n-k+1)。
///
/// - k=0 → 1
/// - k>n → 0
/// - k > 10000 → `Overflow`（DoS 防护）
pub fn perm(n: &BigInt, k: &BigInt) -> Result<BigInt, CalcError> {
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
///
/// - k=0 或 k=n → 1
/// - k>n → 0
/// - k > 10000（优化后） → `Overflow`（DoS 防护）
pub fn comb(n: &BigInt, k: &BigInt) -> Result<BigInt, CalcError> {
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
///
/// - n=0 → 1
/// - n > 5000 → `Overflow`（DoS 防护）
pub fn catalan(n: &BigInt) -> Result<BigInt, CalcError> {
    if n.is_zero() {
        return Ok(BigInt::one());
    }
    const MAX_CATALAN_N: u64 = 5000;
    let n_u64 = n.to_u64().ok_or(CalcError::overflow())?;
    if n_u64 > MAX_CATALAN_N {
        return Err(CalcError::overflow());
    }
    let two_n = n * 2;
    let c_2n_n = comb(&two_n, n)?;
    Ok(c_2n_n / (n + 1))
}

/// 第二类 Stirling 数 S(n, k)：将 n 个元素划分为 k 个非空子集的方式数。
///
/// 递推：S(n,k) = k*S(n-1,k) + S(n-1,k-1)
/// 边界：S(0,0)=1, S(n,0)=0 (n>0), S(0,k)=0 (k>0), S(n,k)=0 (k>n)
/// n/k > 5000 → `Overflow`（DoS 防护）
pub fn stirling_second(n: &BigInt, k: &BigInt) -> Result<BigInt, CalcError> {
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

#[cfg(test)]
mod tests {
    use super::*;

    // ===== perm =====

    #[test]
    fn test_perm_basic() {
        assert_eq!(perm(&BigInt::from(5), &BigInt::from(2)).unwrap(), BigInt::from(20));
    }

    #[test]
    fn test_perm_k_zero() {
        assert_eq!(perm(&BigInt::from(5), &BigInt::from(0)).unwrap(), BigInt::from(1));
    }

    #[test]
    fn test_perm_k_greater_than_n() {
        assert_eq!(perm(&BigInt::from(3), &BigInt::from(5)).unwrap(), BigInt::from(0));
    }

    #[test]
    fn test_perm_n_equals_k() {
        // P(5,5) = 5! = 120
        assert_eq!(perm(&BigInt::from(5), &BigInt::from(5)).unwrap(), BigInt::from(120));
    }

    // ===== comb =====

    #[test]
    fn test_comb_basic() {
        assert_eq!(comb(&BigInt::from(10), &BigInt::from(3)).unwrap(), BigInt::from(120));
    }

    #[test]
    fn test_comb_k_zero() {
        assert_eq!(comb(&BigInt::from(5), &BigInt::from(0)).unwrap(), BigInt::from(1));
    }

    #[test]
    fn test_comb_k_equals_n() {
        assert_eq!(comb(&BigInt::from(5), &BigInt::from(5)).unwrap(), BigInt::from(1));
    }

    #[test]
    fn test_comb_k_greater_than_n() {
        assert_eq!(comb(&BigInt::from(3), &BigInt::from(5)).unwrap(), BigInt::from(0));
    }

    #[test]
    fn test_comb_symmetry() {
        // C(10,3) == C(10,7)
        assert_eq!(
            comb(&BigInt::from(10), &BigInt::from(3)).unwrap(),
            comb(&BigInt::from(10), &BigInt::from(7)).unwrap()
        );
    }

    // ===== catalan =====

    #[test]
    fn test_catalan_zero() {
        assert_eq!(catalan(&BigInt::from(0)).unwrap(), BigInt::from(1));
    }

    #[test]
    fn test_catalan_one() {
        assert_eq!(catalan(&BigInt::from(1)).unwrap(), BigInt::from(1));
    }

    #[test]
    fn test_catalan_five() {
        assert_eq!(catalan(&BigInt::from(5)).unwrap(), BigInt::from(42));
    }

    #[test]
    fn test_catalan_sequence() {
        // 1, 1, 2, 5, 14, 42, 132, 429
        let expected = [1, 1, 2, 5, 14, 42, 132, 429];
        for (i, &e) in expected.iter().enumerate() {
            assert_eq!(catalan(&BigInt::from(i as u64)).unwrap(), BigInt::from(e));
        }
    }

    // ===== stirling_second =====

    #[test]
    fn test_stirling_zero_zero() {
        assert_eq!(stirling_second(&BigInt::from(0), &BigInt::from(0)).unwrap(), BigInt::from(1));
    }

    #[test]
    fn test_stirling_n_zero_k_positive() {
        assert_eq!(stirling_second(&BigInt::from(0), &BigInt::from(5)).unwrap(), BigInt::from(0));
    }

    #[test]
    fn test_stirling_k_zero_n_positive() {
        assert_eq!(stirling_second(&BigInt::from(5), &BigInt::from(0)).unwrap(), BigInt::from(0));
    }

    #[test]
    fn test_stirling_k_greater_than_n() {
        assert_eq!(stirling_second(&BigInt::from(2), &BigInt::from(5)).unwrap(), BigInt::from(0));
    }

    #[test]
    fn test_stirling_known_values() {
        assert_eq!(stirling_second(&BigInt::from(3), &BigInt::from(2)).unwrap(), BigInt::from(3));
        assert_eq!(stirling_second(&BigInt::from(4), &BigInt::from(2)).unwrap(), BigInt::from(7));
        assert_eq!(stirling_second(&BigInt::from(5), &BigInt::from(2)).unwrap(), BigInt::from(15));
        assert_eq!(stirling_second(&BigInt::from(4), &BigInt::from(3)).unwrap(), BigInt::from(6));
    }
}
