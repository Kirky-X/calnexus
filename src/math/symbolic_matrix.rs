// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! 符号矩阵纯函数层：以 [`SymbolicExpr`] 为元素的行列式、逆矩阵与特征值。
//!
//! 仅支持 2×2 / 3×3（超过阶数或奇异结构返回域错误），结果逐元素化简。

use crate::core::CalcError;
use crate::math::symbolic::{SymbolicExpr, simplify};

/// 符号矩阵：行优先存储的符号表达式。
pub type SymMatrix = Vec<Vec<SymbolicExpr>>;

fn validate_square(m: &SymMatrix, max_order: usize) -> Result<usize, CalcError> {
    let n = m.len();
    if n == 0 || n > max_order || m.iter().any(|row| row.len() != n) {
        return Err(CalcError::domain(format!(
            "symbolic matrix must be square with order <= {max_order}"
        ))
        .with_i18n("msg.symbolic.sym_matrix_size", vec![]));
    }
    Ok(n)
}

/// 符号加法（立即化简）。
pub(crate) fn sym_add(a: &SymbolicExpr, b: &SymbolicExpr) -> SymbolicExpr {
    simplify(&SymbolicExpr::Add(Box::new(a.clone()), Box::new(b.clone())))
}

/// 符号减法（立即化简）。
pub(crate) fn sym_sub(a: &SymbolicExpr, b: &SymbolicExpr) -> SymbolicExpr {
    simplify(&SymbolicExpr::Sub(Box::new(a.clone()), Box::new(b.clone())))
}

/// 符号乘法（立即化简）。
pub(crate) fn sym_mul(a: &SymbolicExpr, b: &SymbolicExpr) -> SymbolicExpr {
    simplify(&SymbolicExpr::Mul(Box::new(a.clone()), Box::new(b.clone())))
}

/// 符号除法（立即化简，T008 逆矩阵消费）。
#[allow(dead_code)]
pub(crate) fn sym_div(a: &SymbolicExpr, b: &SymbolicExpr) -> SymbolicExpr {
    simplify(&SymbolicExpr::Div(Box::new(a.clone()), Box::new(b.clone())))
}

/// 常量表达式转 f64（用于 T008 数值特征值路径）。
#[allow(dead_code)]
pub(crate) fn as_number(expr: &SymbolicExpr) -> Option<f64> {
    expr.as_const()
}

/// 2×2 / 3×3 符号行列式（代数余子式展开 + 化简）。
pub fn sym_det(m: &SymMatrix) -> Result<SymbolicExpr, CalcError> {
    let n = validate_square(m, 3)?;
    if n == 1 {
        return Ok(m[0][0].clone());
    }
    if n == 2 {
        return Ok(sym_sub(
            &sym_mul(&m[0][0], &m[1][1]),
            &sym_mul(&m[0][1], &m[1][0]),
        ));
    }
    // 3×3 按第一行展开
    let mut det = SymbolicExpr::Const(0.0);
    for (j, head) in m[0].iter().enumerate() {
        let mut minor = Vec::with_capacity(2);
        for row in m.iter().take(3).skip(1) {
            let reduced: Vec<SymbolicExpr> = row
                .iter()
                .enumerate()
                .filter(|(k, _)| *k != j)
                .map(|(_, value)| value.clone())
                .collect();
            minor.push(reduced);
        }
        let sub = sym_det(&minor)?;
        let term = sym_mul(head, &sub);
        det = if j % 2 == 0 {
            sym_add(&det, &term)
        } else {
            sym_sub(&det, &term)
        };
    }
    Ok(simplify(&det))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::symbolic::{ast_to_symbolic, symbolic_to_string};
    use crate::parse;

    fn sym(expr: &str) -> SymbolicExpr {
        ast_to_symbolic(&parse(expr).unwrap()).unwrap()
    }

    #[test]
    fn test_sym_det_2x2_symbolic() {
        let m = vec![vec![sym("a"), sym("b")], vec![sym("c"), sym("d")]];
        assert_eq!(symbolic_to_string(&sym_det(&m).unwrap()), "a*d-b*c");
    }

    #[test]
    fn test_sym_det_2x2_numeric() {
        let m = vec![
            vec![SymbolicExpr::Const(1.0), SymbolicExpr::Const(2.0)],
            vec![SymbolicExpr::Const(3.0), SymbolicExpr::Const(4.0)],
        ];
        assert_eq!(sym_det(&m).unwrap(), SymbolicExpr::Const(-2.0));
    }

    #[test]
    fn test_sym_det_3x3_numeric() {
        // det([[1,2,3],[4,5,6],[7,8,10]]) = -3
        let m = vec![
            vec![
                SymbolicExpr::Const(1.0),
                SymbolicExpr::Const(2.0),
                SymbolicExpr::Const(3.0),
            ],
            vec![
                SymbolicExpr::Const(4.0),
                SymbolicExpr::Const(5.0),
                SymbolicExpr::Const(6.0),
            ],
            vec![
                SymbolicExpr::Const(7.0),
                SymbolicExpr::Const(8.0),
                SymbolicExpr::Const(10.0),
            ],
        ];
        assert_eq!(sym_det(&m).unwrap(), SymbolicExpr::Const(-3.0));
    }

    #[test]
    fn test_sym_det_rejects_bad_size() {
        // 非方阵（1 行 2 列）必须拒绝
        assert!(
            sym_det(&vec![vec![
                SymbolicExpr::Const(1.0),
                SymbolicExpr::Const(2.0)
            ]])
            .is_err()
        );
        let big = vec![vec![SymbolicExpr::Const(1.0); 4]; 4];
        assert!(sym_det(&big).is_err());
    }
}
