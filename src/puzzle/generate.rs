// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! 谜题生成门面：按题型、难度与 seed 分发到三类生成器。

use crate::core::CalcError;

use super::Puzzle;
use super::arithmetic;
use super::calculus;
use super::number_theory;
use super::rng::SplitMix64;

/// 按题型、难度与 seed 生成谜题；同三元组可复现。
pub fn generate(kind: &str, difficulty: u8, seed: u64) -> Result<Puzzle, CalcError> {
    let mut rng = SplitMix64::from_seed(seed);
    let level = difficulty.min(3);
    match kind {
        "number-theory" => {
            let puzzle = number_theory::generate_number_theory(&mut rng, level);
            Ok(Puzzle {
                kind: kind.to_string(),
                question: puzzle.question,
                answer: puzzle.answer,
                steps: puzzle.steps,
            })
        }
        "arithmetic" => arithmetic::generate_arithmetic(&mut rng, level)
            .map(|p| Puzzle {
                kind: kind.to_string(),
                question: p.question,
                answer: p.answer,
                steps: p.steps,
            })
            .ok_or_else(|| CalcError::domain("failed to generate arithmetic puzzle".to_string())),
        "calculus" => {
            let puzzle = calculus::generate_calculus(&mut rng, level);
            Ok(Puzzle {
                kind: kind.to_string(),
                question: puzzle.question,
                answer: puzzle.answer,
                steps: puzzle.steps,
            })
        }
        other => Err(CalcError::usage(format!("unknown puzzle kind: {other}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_all_kinds_reproducible() {
        for kind in ["number-theory", "arithmetic", "calculus"] {
            let a = generate(kind, 1, 42).unwrap();
            let b = generate(kind, 1, 42).unwrap();
            assert_eq!(a.question, b.question);
            assert_eq!(a.answer, b.answer);
            assert!(!a.steps.is_empty());
        }
    }

    #[test]
    fn test_unknown_kind_errors() {
        assert!(generate("unknown", 1, 0).is_err());
    }
}
