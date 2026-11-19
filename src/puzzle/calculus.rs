// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! 微积分谜题：随机整系数多项式求导/积分题，答案经多项式纯函数复验。

use crate::math::polynomial;

use super::rng::SplitMix64;

/// 微积分谜题：多项式题面 + 升幂系数答案 + 逐项法则说明。
#[derive(Debug, Clone)]
#[allow(dead_code)] // 复验字段由单元测试消费
pub struct CalculusPuzzle {
    pub question: String,
    pub answer: String,
    pub coeffs: Vec<f64>,
    pub steps: Vec<String>,
}

/// 升幂系数渲染为可读多项式（如 `1+2x+3x^2`）。
fn render_poly(coeffs: &[f64]) -> String {
    let mut parts = Vec::new();
    for (i, &c) in coeffs.iter().enumerate() {
        if c == 0.0 {
            continue;
        }
        let coef = if c.fract() == 0.0 {
            format!("{}", c as i64)
        } else {
            format!("{c:.4}")
        };
        parts.push(match i {
            0 => coef,
            1 => format!("{coef}x"),
            _ => format!("{coef}x^{i}"),
        });
    }
    if parts.is_empty() {
        "0".to_string()
    } else {
        parts.join("+")
    }
}

/// 生成随机 3~5 次整系数多项式题；同 seed 可复现。
pub fn generate_calculus(rng: &mut SplitMix64, level: u8) -> CalculusPuzzle {
    let degree = 3 + (rng.next_u64() % (2 + level.min(2) as u64)) as usize;
    let mut coeffs = Vec::with_capacity(degree + 1);
    for i in 0..=degree {
        let value = (rng.next_u64() % 9) as i64 - 4;
        // 常数项允许为 0，其余保证首项非零以便次数明确
        let value = if i == degree && value == 0 { 1 } else { value };
        coeffs.push(value as f64);
    }

    let integrate = rng.next_u64().is_multiple_of(2);
    let (answer_coeffs, kind) = if integrate {
        (polynomial::integrate(&coeffs), "poly_integrate")
    } else {
        (polynomial::diff(&coeffs), "diff")
    };

    let mut steps = Vec::new();
    for (i, &c) in coeffs.iter().enumerate() {
        if c == 0.0 {
            continue;
        }
        steps.push(if integrate {
            format!("∫ {c}x^{i} dx = {}x^{}", c / (i + 1) as f64, i + 1)
        } else if i == 0 {
            format!("d/dx ({c}) = 0")
        } else {
            format!("d/dx ({c}x^{i}) = {}x^{}", c * i as f64, i - 1)
        });
    }

    CalculusPuzzle {
        question: format!("{kind}({}, x)", render_poly(&coeffs)),
        answer: render_poly(&answer_coeffs),
        coeffs,
        steps,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_same_seed_reproducible() {
        let a = generate_calculus(&mut SplitMix64::from_seed(11), 1);
        let b = generate_calculus(&mut SplitMix64::from_seed(11), 1);
        assert_eq!(a.question, b.question);
        assert_eq!(a.answer, b.answer);
        assert_eq!(a.steps, b.steps);
    }

    #[test]
    fn test_answer_matches_polynomial_ops() {
        for seed in 0..10 {
            let puzzle = generate_calculus(&mut SplitMix64::from_seed(seed), 1);
            let expected = if puzzle.question.starts_with("poly_integrate") {
                render_poly(&polynomial::integrate(&puzzle.coeffs))
            } else {
                render_poly(&polynomial::diff(&puzzle.coeffs))
            };
            assert_eq!(puzzle.answer, expected);
            assert!(!puzzle.steps.is_empty());
        }
    }
}
