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

/// 符号除法（立即化简，逆矩阵消费）。
pub(crate) fn sym_div(a: &SymbolicExpr, b: &SymbolicExpr) -> SymbolicExpr {
    simplify(&SymbolicExpr::Div(Box::new(a.clone()), Box::new(b.clone())))
}

/// 常量表达式转 f64（用于数值特征值路径）。
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

/// 符号逆矩阵（伴随矩阵 / det，逐元素化简）。奇异矩阵返回域错误。
// 行列式/伴随矩阵的 i/j 显式索引贴合数学定义，改用迭代器会牺牲可读性。
#[allow(clippy::needless_range_loop)]
pub fn sym_inv(m: &SymMatrix) -> Result<SymMatrix, CalcError> {
    let n = validate_square(m, 3)?;
    let det = sym_det(m)?;
    if as_number(&det) == Some(0.0) {
        return Err(CalcError::domain("symbolic matrix is singular".to_string())
            .with_i18n("msg.symbolic.sym_matrix_singular", vec![]));
    }
    if n == 1 {
        return Ok(vec![vec![sym_div(&SymbolicExpr::Const(1.0), &det)]]);
    }
    if n == 2 {
        let a = &m[0][0];
        let b = &m[0][1];
        let c = &m[1][0];
        let d = &m[1][1];
        return Ok(vec![
            vec![
                sym_div(d, &det),
                sym_div(&SymbolicExpr::Neg(Box::new(b.clone())), &det),
            ],
            vec![
                sym_div(&SymbolicExpr::Neg(Box::new(c.clone())), &det),
                sym_div(a, &det),
            ],
        ]);
    }
    // 3×3：余子式矩阵转置为伴随矩阵
    let mut adj = vec![vec![SymbolicExpr::Const(0.0); 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            let mut minor = Vec::with_capacity(2);
            for (r, row) in m.iter().enumerate().take(3) {
                if r == i {
                    continue;
                }
                let reduced: Vec<SymbolicExpr> = row
                    .iter()
                    .enumerate()
                    .filter(|(c, _)| *c != j)
                    .map(|(_, v)| v.clone())
                    .collect();
                minor.push(reduced);
            }
            let cofactor = sym_det(&minor)?;
            let signed = if (i + j) % 2 == 0 {
                cofactor
            } else {
                SymbolicExpr::Neg(Box::new(cofactor))
            };
            adj[j][i] = signed; // 转置
        }
    }
    Ok(adj
        .iter()
        .map(|row| row.iter().map(|v| sym_div(v, &det)).collect())
        .collect())
}

/// 升幂系数向量的卷积乘法。
fn mul_coeffs(a: &[SymbolicExpr], b: &[SymbolicExpr]) -> Vec<SymbolicExpr> {
    let mut out = vec![SymbolicExpr::Const(0.0); a.len() + b.len() - 1];
    for (i, x) in a.iter().enumerate() {
        for (j, y) in b.iter().enumerate() {
            out[i + j] = sym_add(&out[i + j], &sym_mul(x, y));
        }
    }
    out
}

/// 从符号表达式提取指定变量的升幂系数；非多项式返回 None。
// 升幂系数按下标逐项合并，显式索引比迭代器更直观。
#[allow(clippy::needless_range_loop)]
fn coeffs_in_var(expr: &SymbolicExpr, var: &str) -> Option<Vec<SymbolicExpr>> {
    match expr {
        SymbolicExpr::Const(v) => Some(vec![SymbolicExpr::Const(*v)]),
        SymbolicExpr::Var(name) if name == var => {
            Some(vec![SymbolicExpr::Const(0.0), SymbolicExpr::Const(1.0)])
        }
        SymbolicExpr::Var(_) => None,
        SymbolicExpr::Add(l, r) | SymbolicExpr::Sub(l, r) => {
            let (lc, rc) = (coeffs_in_var(l, var)?, coeffs_in_var(r, var)?);
            let n = lc.len().max(rc.len());
            let mut out = vec![SymbolicExpr::Const(0.0); n];
            for i in 0..n {
                let l = lc.get(i).unwrap_or(&out[i]).clone();
                let r = rc.get(i).unwrap_or(&out[i]).clone();
                out[i] = if matches!(expr, SymbolicExpr::Add(_, _)) {
                    sym_add(&l, &r)
                } else {
                    sym_sub(&l, &r)
                };
            }
            Some(out)
        }
        SymbolicExpr::Neg(e) => Some(
            coeffs_in_var(e, var)?
                .iter()
                .map(|v| SymbolicExpr::Neg(Box::new(v.clone())))
                .collect(),
        ),
        SymbolicExpr::Mul(l, r) => {
            let lc = coeffs_in_var(l, var)?;
            let rc = coeffs_in_var(r, var)?;
            let mut out = vec![SymbolicExpr::Const(0.0); lc.len() + rc.len() - 1];
            for (i, a) in lc.iter().enumerate() {
                for (j, b) in rc.iter().enumerate() {
                    out[i + j] = sym_add(&out[i + j], &sym_mul(a, b));
                }
            }
            Some(out)
        }
        SymbolicExpr::Div(l, r) => {
            let k = r.as_const()?;
            Some(
                coeffs_in_var(l, var)?
                    .iter()
                    .map(|v| sym_div(v, &SymbolicExpr::Const(k)))
                    .collect(),
            )
        }
        SymbolicExpr::Pow(base, exp) => {
            let n = exp.as_const()?;
            if n < 0.0 || n.fract() != 0.0 {
                return None;
            }
            let bc = coeffs_in_var(base, var)?;
            let mut out = vec![SymbolicExpr::Const(1.0)];
            for _ in 0..(n as usize) {
                out = mul_coeffs(&out, &bc);
            }
            Some(out)
        }
        SymbolicExpr::Sin(_)
        | SymbolicExpr::Cos(_)
        | SymbolicExpr::Tan(_)
        | SymbolicExpr::Ln(_)
        | SymbolicExpr::Exp(_) => None,
    }
}

/// 特征多项式系数（升幂）：det(λI − A)，λ 以 `var` 表示。
pub fn sym_charpoly(m: &SymMatrix, var: &str) -> Result<Vec<SymbolicExpr>, CalcError> {
    let n = validate_square(m, 3)?;
    let lambda = SymbolicExpr::Var(var.to_string());
    let mut shifted = vec![vec![SymbolicExpr::Const(0.0); n]; n];
    for i in 0..n {
        for j in 0..n {
            shifted[i][j] = if i == j {
                sym_sub(&lambda, &m[i][j])
            } else {
                SymbolicExpr::Neg(Box::new(m[j][i].clone()))
            };
        }
    }
    let det = sym_det(&shifted)?;
    coeffs_in_var(&det, var).ok_or_else(|| {
        CalcError::domain("failed to extract characteristic polynomial".to_string())
            .with_i18n("msg.symbolic.sym_matrix_size", vec![])
    })
}

/// 数值特征值：2×2 走精确根式，3×3 经特征多项式系数 → `polynomial::roots`。
pub fn sym_eigvals(m: &SymMatrix) -> Result<crate::core::EvalResult, CalcError> {
    let n = validate_square(m, 3)?;
    if m.iter().flatten().any(|e| as_number(e).is_none()) {
        return Err(
            CalcError::domain("eigenvalues require constant matrix entries".to_string())
                .with_i18n("msg.symbolic.sym_matrix_size", vec![]),
        );
    }
    if n == 2 {
        let a = as_number(&m[0][0]).unwrap();
        let b = as_number(&m[0][1]).unwrap();
        let c = as_number(&m[1][0]).unwrap();
        let d = as_number(&m[1][1]).unwrap();
        let trace = a + d;
        let disc = (a - d).powi(2) + 4.0 * b * c;
        if disc >= 0.0 {
            let root = disc.sqrt();
            return Ok(crate::core::EvalResult::Vector(vec![
                (trace - root) / 2.0,
                (trace + root) / 2.0,
            ]));
        }
        let im = (-disc).sqrt() / 2.0;
        let re = trace / 2.0;
        return Ok(crate::core::EvalResult::ComplexList(vec![
            (re, im),
            (re, -im),
        ]));
    }
    let coeffs: Vec<f64> = sym_charpoly(m, "lambda")?
        .iter()
        .map(as_number)
        .collect::<Option<_>>()
        .ok_or_else(|| CalcError::domain("non-numeric characteristic coefficient".to_string()))?;
    crate::math::polynomial::roots(&coeffs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::EvalResult;
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
    #[test]
    fn test_sym_inv_2x2_symbolic() {
        let m = vec![vec![sym("a"), sym("b")], vec![sym("c"), sym("d")]];
        let inv = sym_inv(&m).unwrap();
        let got: Vec<Vec<String>> = inv
            .iter()
            .map(|row| row.iter().map(symbolic_to_string).collect())
            .collect();
        assert!(got[0][0].contains("d"), "{got:?}");
        assert!(got[0][1].contains("-b"), "{got:?}");
        assert!(got[1][0].contains("-c"), "{got:?}");
        assert!(got[1][1].contains("a"), "{got:?}");
    }

    #[test]
    fn test_sym_inv_singular_rejected() {
        let m = vec![
            vec![SymbolicExpr::Const(1.0), SymbolicExpr::Const(2.0)],
            vec![SymbolicExpr::Const(2.0), SymbolicExpr::Const(4.0)],
        ];
        assert!(sym_inv(&m).is_err());
    }

    #[test]
    fn test_sym_eigvals_2x2() {
        let m = vec![
            vec![SymbolicExpr::Const(2.0), SymbolicExpr::Const(1.0)],
            vec![SymbolicExpr::Const(1.0), SymbolicExpr::Const(2.0)],
        ];
        let EvalResult::Vector(v) = sym_eigvals(&m).unwrap() else {
            panic!("expected real eigenvalues");
        };
        let mut got = v;
        got.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert!((got[0] - 1.0).abs() < 1e-12);
        assert!((got[1] - 3.0).abs() < 1e-12);
    }

    #[test]
    fn test_sym_charpoly_2x2() {
        let m = vec![
            vec![SymbolicExpr::Const(2.0), SymbolicExpr::Const(1.0)],
            vec![SymbolicExpr::Const(1.0), SymbolicExpr::Const(2.0)],
        ];
        let coeffs = sym_charpoly(&m, "lambda").unwrap();
        assert_eq!(coeffs.len(), 3);
        let nums: Vec<f64> = coeffs.iter().map(|c| c.as_const().unwrap()).collect();
        assert_eq!(nums, vec![3.0, -4.0, 1.0]);
    }
}
