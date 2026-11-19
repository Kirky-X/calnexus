// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! 算术谜题：随机运算树生成表达式，答案经求值管线复算，步骤复用步骤生成器。

use crate::core::{CacheManager, EvalContext, parse};

use super::rng::SplitMix64;

/// 算术谜题：表达式题面 + 复算答案 + 推导步骤。
#[derive(Debug, Clone)]
pub struct ArithmeticPuzzle {
    pub question: String,
    pub answer: String,
    pub steps: Vec<String>,
}

fn eval_scalar(expr: &str) -> Option<f64> {
    let cache = CacheManager::new();
    let (result, _, _, _) = crate::core::evaluate(expr, &EvalContext::new(), None, &cache).ok()?;
    result.as_scalar()
}

fn operand(rng: &mut SplitMix64, level: u8) -> u64 {
    1 + rng.next_u64() % (10 + 10 * level as u64).max(1)
}

/// 生成 4~6 节点运算表达式题；同 seed 可复现，答案与独立求值一致。
pub fn generate_arithmetic(rng: &mut SplitMix64, level: u8) -> Option<ArithmeticPuzzle> {
    for _ in 0..32 {
        let a = operand(rng, level);
        let b = operand(rng, level);
        let c = operand(rng, level);
        let d = operand(rng, level);
        let op1 = ["+", "-", "*", "/"][(rng.next_u64() % 4) as usize];
        let op2 = ["+", "-", "*", "/"][(rng.next_u64() % 4) as usize];
        let op3 = ["+", "-", "*", "/"][(rng.next_u64() % 4) as usize];
        let expr = format!("({a}{op1}{b}){op2}({c}{op3}{d})");
        let Some(value) = eval_scalar(&expr) else {
            continue; // 含除零等非法表达式，重抽
        };
        if !value.is_finite() {
            continue;
        }
        let steps = parse(&expr)
            .ok()
            .and_then(|ast| crate::output::generate_steps(&ast, &EvalContext::new()).ok())
            .unwrap_or_default();
        let answer = format_number(value);
        return Some(ArithmeticPuzzle {
            question: expr,
            answer,
            steps,
        });
    }
    None
}

fn format_number(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        format!("{v:.6}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_same_seed_reproducible() {
        let a = generate_arithmetic(&mut SplitMix64::from_seed(99), 1).unwrap();
        let b = generate_arithmetic(&mut SplitMix64::from_seed(99), 1).unwrap();
        assert_eq!(a.question, b.question);
        assert_eq!(a.answer, b.answer);
        assert_eq!(a.steps, b.steps);
    }

    #[test]
    fn test_answer_matches_independent_evaluation() {
        for seed in 0..12 {
            let puzzle = generate_arithmetic(&mut SplitMix64::from_seed(seed), 1).unwrap();
            let value = eval_scalar(&puzzle.question).unwrap();
            assert_eq!(puzzle.answer, format_number(value));
            assert!(!puzzle.steps.is_empty());
        }
    }
}
