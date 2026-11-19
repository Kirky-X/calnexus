// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! 数论谜题：随机半素数分解题（答案可经素性判定与乘法复验）。

use crate::math::number_theory::is_prime;
use num_bigint::BigInt;

use super::rng::SplitMix64;

/// 数论谜题：半素数 n = p × q，附试除轨迹。
#[derive(Debug, Clone)]
#[allow(dead_code)] // 复验字段由单元测试消费
pub struct NumberTheoryPuzzle {
    pub n: u64,
    pub p: u64,
    pub q: u64,
    pub question: String,
    pub answer: String,
    pub steps: Vec<String>,
}

fn difficulty_bounds(level: u8) -> (u64, u64) {
    let shift = (8 + 4 * level as u32).min(16);
    let lo = 1u64 << shift;
    let hi = (lo << 1).saturating_mul(if level >= 3 { 2 } else { 1 });
    (lo, hi)
}

fn random_prime(rng: &mut SplitMix64, lo: u64, hi: u64) -> u64 {
    loop {
        let candidate = lo + (rng.next_u64() % (hi - lo).max(1));
        if is_prime(&BigInt::from(candidate)) {
            return candidate;
        }
    }
}

/// 生成半素数分解题；同 seed 与难度可复现。
pub fn generate_number_theory(rng: &mut SplitMix64, level: u8) -> NumberTheoryPuzzle {
    let (lo, hi) = difficulty_bounds(level);
    let p = random_prime(rng, lo, hi);
    let q = random_prime(rng, lo, hi);
    let n = p.saturating_mul(q);

    let mut steps = Vec::new();
    let mut divisor = 2u64;
    while divisor < p.min(q) && steps.len() < 8 {
        if n.is_multiple_of(divisor) {
            break;
        }
        steps.push(format!("{divisor} does not divide {n}"));
        divisor += 1;
    }
    let (small, large) = if p <= q { (p, q) } else { (q, p) };
    steps.push(format!("{small} divides {n} → {n}/{small} = {large}"));

    NumberTheoryPuzzle {
        n,
        p: small,
        q: large,
        question: format!("Factorize {n}"),
        answer: format!("{small} × {large}"),
        steps,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_same_seed_reproducible() {
        let a = generate_number_theory(&mut SplitMix64::from_seed(42), 1);
        let b = generate_number_theory(&mut SplitMix64::from_seed(42), 1);
        assert_eq!(a.n, b.n);
        assert_eq!(a.answer, b.answer);
        assert_eq!(a.steps, b.steps);
    }

    #[test]
    fn test_answer_is_valid_factorization() {
        for level in 0..=3 {
            let puzzle =
                generate_number_theory(&mut SplitMix64::from_seed(7 + level as u64), level);
            assert_eq!(puzzle.p * puzzle.q, puzzle.n);
            assert!(is_prime(&BigInt::from(puzzle.p)));
            assert!(is_prime(&BigInt::from(puzzle.q)));
            assert!(puzzle.question.contains(&puzzle.n.to_string()));
            assert!(puzzle.answer.contains(&puzzle.p.to_string()));
            assert!(!puzzle.steps.is_empty());
        }
    }
}
