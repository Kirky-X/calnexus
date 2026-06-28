//! Matrix 计算域：矩阵加减乘、行列式、转置、逆、单位矩阵。
//!
//! 设计依据：
//! - matrix-domain spec：10 个 requirements / 21 个 scenarios
//! - design.md D5：基于 `nalgebra::DMatrix` 实现，priority=30
//!
//! 路由策略：AST 含 `Matrix` 节点或 `det()`/`transpose()`/`inverse()`/`identity()` 函数调用时路由至本域。
//! `EvalResult::Matrix(Vec<Vec<f64>>)` 保持 types.rs 无外部依赖（与 Complex 同策略）。

use crate::core::domain::CalculationDomain;
use crate::core::types::{AstNode, BinaryOp, CalcError, EvalContext, EvalResult, UnaryOp};
use nalgebra::DMatrix;

/// Matrix 计算域。
///
/// priority=30，支持矩阵加减乘、标量乘、行列式、转置、逆、单位矩阵。
pub struct MatrixDomain;

impl CalculationDomain for MatrixDomain {
    fn domain_name(&self) -> &str {
        "matrix"
    }

    fn priority(&self) -> u8 {
        30
    }

    fn supports(&self, ast: &AstNode) -> bool {
        contains_matrix(ast)
    }

    fn evaluate(&self, ast: &AstNode, ctx: &EvalContext) -> Result<EvalResult, CalcError> {
        let mut ctx = ctx.clone();
        if ctx.get_var("pi").is_none() {
            ctx = ctx.with_var("pi", std::f64::consts::PI);
        }
        if ctx.get_var("e").is_none() {
            ctx = ctx.with_var("e", std::f64::consts::E);
        }

        let value = self.eval(ast, &ctx)?;
        match value {
            MatrixValue::Scalar(v) => {
                if !v.is_finite() {
                    return Err(CalcError::NaNOrInf);
                }
                Ok(EvalResult::Scalar(v))
            }
            MatrixValue::Matrix(m) => {
                let rows: Vec<Vec<f64>> = (0..m.nrows())
                    .map(|i| (0..m.ncols()).map(|j| m[(i, j)]).collect())
                    .collect();
                Ok(EvalResult::Matrix(rows))
            }
        }
    }
}

impl MatrixDomain {
    /// 递归求值 AST 节点，返回标量或矩阵。
    fn eval(&self, ast: &AstNode, ctx: &EvalContext) -> Result<MatrixValue, CalcError> {
        match ast {
            AstNode::Number(n) => Ok(MatrixValue::Scalar(*n)),
            AstNode::Variable(name) => ctx
                .get_var(name)
                .map(MatrixValue::Scalar)
                .ok_or_else(|| CalcError::EvalError(format!("unbound variable: {}", name))),
            AstNode::Matrix(rows) => self.eval_matrix_literal(rows, ctx),
            AstNode::BinaryOp(op, l, r) => {
                let a = self.eval(l, ctx)?;
                let b = self.eval(r, ctx)?;
                self.eval_binary(*op, a, b)
            }
            AstNode::UnaryOp(op, e) => {
                let v = self.eval(e, ctx)?;
                match op {
                    UnaryOp::Neg => match v {
                        MatrixValue::Scalar(s) => Ok(MatrixValue::Scalar(-s)),
                        MatrixValue::Matrix(m) => Ok(MatrixValue::Matrix(-m)),
                    },
                    UnaryOp::Abs => match v {
                        MatrixValue::Scalar(s) => Ok(MatrixValue::Scalar(s.abs())),
                        MatrixValue::Matrix(_) => Err(CalcError::DomainError(
                            "abs() not supported for matrices".to_string(),
                        )),
                    },
                    UnaryOp::Factorial => Err(CalcError::DomainError(
                        "factorial not supported in matrix domain".to_string(),
                    )),
                }
            }
            AstNode::FunctionCall(name, args) => self.eval_function(name, args, ctx),
            AstNode::Complex(_, _) | AstNode::List(_) | AstNode::BigNumber(_) => {
                Err(CalcError::DomainError(format!(
                    "matrix domain does not support this node type: {:?}",
                    ast
                )))
            }
        }
    }

    /// 求值矩阵字面量：`[[1,2],[3,4]]` → DMatrix。
    ///
    /// 所有元素 MUST 为标量表达式，所有行 MUST 等长。
    fn eval_matrix_literal(
        &self,
        rows: &[Vec<AstNode>],
        ctx: &EvalContext,
    ) -> Result<MatrixValue, CalcError> {
        if rows.is_empty() {
            return Err(CalcError::DomainError("empty matrix literal".to_string()));
        }
        let ncols = rows[0].len();
        if ncols == 0 {
            return Err(CalcError::DomainError("empty matrix row".to_string()));
        }
        for (i, row) in rows.iter().enumerate() {
            if row.len() != ncols {
                return Err(CalcError::DomainError(format!(
                    "matrix row {} has {} elements, expected {}",
                    i,
                    row.len(),
                    ncols
                )));
            }
        }
        let mut data = Vec::with_capacity(rows.len() * ncols);
        for row in rows {
            for elem in row {
                match self.eval(elem, ctx)? {
                    MatrixValue::Scalar(s) => data.push(s),
                    MatrixValue::Matrix(_) => {
                        return Err(CalcError::DomainError(
                            "matrix elements must be scalars".to_string(),
                        ))
                    }
                }
            }
        }
        let matrix = DMatrix::from_row_slice(rows.len(), ncols, &data);
        Ok(MatrixValue::Matrix(matrix))
    }

    /// 求值二元运算。
    fn eval_binary(
        &self,
        op: BinaryOp,
        a: MatrixValue,
        b: MatrixValue,
    ) -> Result<MatrixValue, CalcError> {
        // 两个标量 → 标量运算（矩阵元素中的子表达式，如 `1+1`）
        if let (MatrixValue::Scalar(av), MatrixValue::Scalar(bv)) = (&a, &b) {
            let result = match op {
                BinaryOp::Add => av + bv,
                BinaryOp::Sub => av - bv,
                BinaryOp::Mul => av * bv,
                BinaryOp::Div => {
                    if *bv == 0.0 {
                        if *av == 0.0 {
                            return Err(CalcError::NaNOrInf);
                        }
                        return Err(CalcError::DivisionByZero);
                    }
                    av / bv
                }
                BinaryOp::Pow => {
                    if *av == 0.0 && *bv == 0.0 {
                        1.0
                    } else {
                        av.powf(*bv)
                    }
                }
                BinaryOp::Mod => {
                    if *bv == 0.0 {
                        return Err(CalcError::DivisionByZero);
                    }
                    av % bv
                }
            };
            if !result.is_finite() {
                return Err(CalcError::NaNOrInf);
            }
            return Ok(MatrixValue::Scalar(result));
        }

        match op {
            BinaryOp::Add | BinaryOp::Sub => match (a, b) {
                (MatrixValue::Matrix(am), MatrixValue::Matrix(bm)) => {
                    if am.shape() != bm.shape() {
                        return Err(CalcError::DomainError(format!(
                            "matrix dimension mismatch for add/sub: {}x{} vs {}x{}",
                            am.nrows(),
                            am.ncols(),
                            bm.nrows(),
                            bm.ncols()
                        )));
                    }
                    let result = if op == BinaryOp::Add {
                        am + bm
                    } else {
                        am - bm
                    };
                    Ok(MatrixValue::Matrix(result))
                }
                _ => Err(CalcError::DomainError(
                    "matrix add/sub requires two matrices of the same dimension".to_string(),
                )),
            },
            BinaryOp::Mul => match (a, b) {
                (MatrixValue::Scalar(s), MatrixValue::Matrix(m)) => Ok(MatrixValue::Matrix(&m * s)),
                (MatrixValue::Matrix(m), MatrixValue::Scalar(s)) => Ok(MatrixValue::Matrix(&m * s)),
                (MatrixValue::Matrix(am), MatrixValue::Matrix(bm)) => {
                    if am.ncols() != bm.nrows() {
                        return Err(CalcError::DomainError(format!(
                            "matrix multiplication dimension mismatch: {}x{} * {}x{}",
                            am.nrows(),
                            am.ncols(),
                            bm.nrows(),
                            bm.ncols()
                        )));
                    }
                    Ok(MatrixValue::Matrix(&am * &bm))
                }
                (MatrixValue::Scalar(_), MatrixValue::Scalar(_)) => unreachable!(),
            },
            BinaryOp::Div => match (a, b) {
                (MatrixValue::Matrix(m), MatrixValue::Scalar(s)) => {
                    if s == 0.0 {
                        return Err(CalcError::DivisionByZero);
                    }
                    Ok(MatrixValue::Matrix(&m / s))
                }
                _ => Err(CalcError::DomainError(
                    "matrix division only supports matrix / scalar".to_string(),
                )),
            },
            BinaryOp::Pow => Err(CalcError::DomainError(
                "matrix power not supported".to_string(),
            )),
            BinaryOp::Mod => Err(CalcError::DomainError(
                "matrix mod not supported".to_string(),
            )),
        }
    }

    /// 求值函数调用。
    fn eval_function(
        &self,
        name: &str,
        args: &[AstNode],
        ctx: &EvalContext,
    ) -> Result<MatrixValue, CalcError> {
        match name {
            "det" => {
                if args.len() != 1 {
                    return Err(CalcError::DomainError(format!(
                        "det() requires exactly 1 argument, got {}",
                        args.len()
                    )));
                }
                let m = self.eval(&args[0], ctx)?;
                match m {
                    MatrixValue::Matrix(matrix) => {
                        if !matrix.is_square() {
                            return Err(CalcError::DomainError(format!(
                                "det() requires a square matrix, got {}x{}",
                                matrix.nrows(),
                                matrix.ncols()
                            )));
                        }
                        Ok(MatrixValue::Scalar(matrix.determinant()))
                    }
                    _ => Err(CalcError::DomainError(
                        "det() requires a matrix argument".to_string(),
                    )),
                }
            }
            "transpose" => {
                if args.len() != 1 {
                    return Err(CalcError::DomainError(format!(
                        "transpose() requires exactly 1 argument, got {}",
                        args.len()
                    )));
                }
                let m = self.eval(&args[0], ctx)?;
                match m {
                    MatrixValue::Matrix(matrix) => Ok(MatrixValue::Matrix(matrix.transpose())),
                    _ => Err(CalcError::DomainError(
                        "transpose() requires a matrix argument".to_string(),
                    )),
                }
            }
            "inverse" => {
                if args.len() != 1 {
                    return Err(CalcError::DomainError(format!(
                        "inverse() requires exactly 1 argument, got {}",
                        args.len()
                    )));
                }
                let m = self.eval(&args[0], ctx)?;
                match m {
                    MatrixValue::Matrix(matrix) => {
                        if !matrix.is_square() {
                            return Err(CalcError::DomainError(format!(
                                "inverse() requires a square matrix, got {}x{}",
                                matrix.nrows(),
                                matrix.ncols()
                            )));
                        }
                        match matrix.try_inverse() {
                            Some(inv) => Ok(MatrixValue::Matrix(inv)),
                            None => Err(CalcError::DomainError(
                                "matrix is singular (not invertible)".to_string(),
                            )),
                        }
                    }
                    _ => Err(CalcError::DomainError(
                        "inverse() requires a matrix argument".to_string(),
                    )),
                }
            }
            "identity" => {
                if args.len() != 1 {
                    return Err(CalcError::DomainError(format!(
                        "identity() requires exactly 1 argument, got {}",
                        args.len()
                    )));
                }
                let n_val = self.eval(&args[0], ctx)?;
                match n_val {
                    MatrixValue::Scalar(n) => {
                        if n < 1.0 || n != n.trunc() {
                            return Err(CalcError::DomainError(format!(
                                "identity() requires a positive integer, got {}",
                                n
                            )));
                        }
                        let n = n as usize;
                        Ok(MatrixValue::Matrix(DMatrix::identity(n, n)))
                    }
                    _ => Err(CalcError::DomainError(
                        "identity() requires a scalar argument".to_string(),
                    )),
                }
            }
            _ => Err(CalcError::DomainError(format!(
                "unsupported function in matrix domain: {}",
                name
            ))),
        }
    }
}

/// 内部求值结果：标量或矩阵。
enum MatrixValue {
    Scalar(f64),
    Matrix(DMatrix<f64>),
}

/// 递归检查 AST 是否应路由至 MatrixDomain。
///
/// 路由条件（spec Req 10）：
/// - 含 `Matrix` 节点
/// - 含 `det()`/`transpose()`/`inverse()`/`identity()` 函数调用
fn contains_matrix(ast: &AstNode) -> bool {
    match ast {
        AstNode::Matrix(_) => true,
        AstNode::FunctionCall(name, _)
            if matches!(name.as_str(), "det" | "transpose" | "inverse" | "identity") =>
        {
            true
        }
        AstNode::FunctionCall(_, args) => args.iter().any(contains_matrix),
        AstNode::BinaryOp(_, l, r) => contains_matrix(l) || contains_matrix(r),
        AstNode::UnaryOp(_, e) => contains_matrix(e),
        AstNode::Complex(_, _) | AstNode::BigNumber(_) => false,
        AstNode::List(elements) => elements.iter().any(contains_matrix),
        AstNode::Number(_) | AstNode::Variable(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::parser::parse;

    fn assert_approx(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 1e-10,
            "expected {} but got {}",
            expected,
            actual
        );
    }

    fn assert_scalar(actual: &EvalResult, expected: f64) {
        match actual {
            EvalResult::Scalar(v) => assert_approx(*v, expected),
            other => panic!("expected Scalar({}), got {:?}", expected, other),
        }
    }

    fn assert_matrix(actual: &EvalResult, expected: &[&[f64]]) {
        match actual {
            EvalResult::Matrix(rows) => {
                assert_eq!(rows.len(), expected.len(), "row count mismatch");
                for (i, (actual_row, expected_row)) in rows.iter().zip(expected.iter()).enumerate()
                {
                    assert_eq!(
                        actual_row.len(),
                        expected_row.len(),
                        "col count mismatch at row {}",
                        i
                    );
                    for (_j, (a, e)) in actual_row.iter().zip(expected_row.iter()).enumerate() {
                        assert_approx(*a, *e);
                    }
                }
            }
            other => panic!("expected Matrix, got {:?}", other),
        }
    }

    fn default_ctx() -> EvalContext {
        EvalContext::new()
            .with_var("pi", std::f64::consts::PI)
            .with_var("e", std::f64::consts::E)
    }

    // ===== Requirement 1: 矩阵字面量解析 =====

    #[test]
    fn test_matrix_literal_2x2() {
        // [[1,2],[3,4]] → 2x2 Matrix（Req 1 Scen 1）
        let ast = parse("[[1,2],[3,4]]").unwrap();
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_matrix(&result, &[&[1.0, 2.0], &[3.0, 4.0]]);
    }

    #[test]
    fn test_matrix_literal_non_square() {
        // [[1,2,3],[4,5,6]] → 2x3 Matrix（Req 1 Scen 2）
        let ast = parse("[[1,2,3],[4,5,6]]").unwrap();
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_matrix(&result, &[&[1.0, 2.0, 3.0], &[4.0, 5.0, 6.0]]);
    }

    #[test]
    fn test_matrix_literal_single_row() {
        // [[1,2,3]] → 1x3 Matrix（Req 1 Scen 3）
        let ast = parse("[[1,2,3]]").unwrap();
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_matrix(&result, &[&[1.0, 2.0, 3.0]]);
    }

    // ===== Requirement 2: 矩阵加法 =====

    #[test]
    fn test_matrix_addition() {
        // [[1,2],[3,4]] + [[5,6],[7,8]] → [[6,8],[10,12]]（Req 2 Scen 1）
        let ast = parse("[[1,2],[3,4]] + [[5,6],[7,8]]").unwrap();
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_matrix(&result, &[&[6.0, 8.0], &[10.0, 12.0]]);
    }

    #[test]
    fn test_matrix_subtraction() {
        // [[5,6],[7,8]] - [[1,2],[3,4]] → [[4,4],[4,4]]（Req 2 Scen 2）
        let ast = parse("[[5,6],[7,8]] - [[1,2],[3,4]]").unwrap();
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_matrix(&result, &[&[4.0, 4.0], &[4.0, 4.0]]);
    }

    // ===== Requirement 3: 矩阵乘法 =====

    #[test]
    fn test_matrix_multiplication_2x2() {
        // [[1,2],[3,4]] * [[5,6],[7,8]] → [[19,22],[43,50]]（Req 3 Scen 1）
        let ast = parse("[[1,2],[3,4]] * [[5,6],[7,8]]").unwrap();
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_matrix(&result, &[&[19.0, 22.0], &[43.0, 50.0]]);
    }

    #[test]
    fn test_matrix_multiplication_dim_match() {
        // [[1,2,3]] * [[1],[2],[3]] → [[14]]（Req 3 Scen 2）
        let ast = parse("[[1,2,3]] * [[1],[2],[3]]").unwrap();
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_matrix(&result, &[&[14.0]]);
    }

    // ===== Requirement 4: 矩阵标量乘 =====

    #[test]
    fn test_scalar_left_multiply_matrix() {
        // 2 * [[1,2],[3,4]] → [[2,4],[6,8]]（Req 4 Scen 1）
        let ast = parse("2 * [[1,2],[3,4]]").unwrap();
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_matrix(&result, &[&[2.0, 4.0], &[6.0, 8.0]]);
    }

    #[test]
    fn test_matrix_right_multiply_scalar() {
        // [[1,2],[3,4]] * 3 → [[3,6],[9,12]]（Req 4 Scen 2）
        let ast = parse("[[1,2],[3,4]] * 3").unwrap();
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_matrix(&result, &[&[3.0, 6.0], &[9.0, 12.0]]);
    }

    // ===== Requirement 5: 行列式 =====

    #[test]
    fn test_det_2x2() {
        // det([[1,2],[3,4]]) → -2.0（Req 5 Scen 1）
        let ast = parse("det([[1,2],[3,4]])").unwrap();
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_scalar(&result, -2.0);
    }

    #[test]
    fn test_det_3x3() {
        // det([[1,2,3],[4,5,6],[7,8,10]]) → -3.0（Req 5 Scen 2）
        let ast = parse("det([[1,2,3],[4,5,6],[7,8,10]])").unwrap();
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_scalar(&result, -3.0);
    }

    // ===== Requirement 6: 矩阵转置 =====

    #[test]
    fn test_transpose_square() {
        // transpose([[1,2],[3,4]]) → [[1,3],[2,4]]（Req 6 Scen 1）
        let ast = parse("transpose([[1,2],[3,4]])").unwrap();
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_matrix(&result, &[&[1.0, 3.0], &[2.0, 4.0]]);
    }

    #[test]
    fn test_transpose_non_square() {
        // transpose([[1,2,3],[4,5,6]]) → [[1,4],[2,5],[3,6]]（Req 6 Scen 2）
        let ast = parse("transpose([[1,2,3],[4,5,6]])").unwrap();
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_matrix(&result, &[&[1.0, 4.0], &[2.0, 5.0], &[3.0, 6.0]]);
    }

    // ===== Requirement 7: 逆矩阵 =====

    #[test]
    fn test_inverse_invertible() {
        // inverse([[1,2],[3,4]]) → [[-2,1],[1.5,-0.5]]（Req 7 Scen 1）
        let ast = parse("inverse([[1,2],[3,4]])").unwrap();
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_matrix(&result, &[&[-2.0, 1.0], &[1.5, -0.5]]);
    }

    #[test]
    fn test_inverse_singular() {
        // inverse([[1,2],[2,4]]) → DomainError（Req 7 Scen 2，奇异矩阵）
        let ast = parse("inverse([[1,2],[2,4]])").unwrap();
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    // ===== Requirement 8: 单位矩阵 =====

    #[test]
    fn test_identity_3() {
        // identity(3) → [[1,0,0],[0,1,0],[0,0,1]]（Req 8 Scen 1）
        let ast = parse("identity(3)").unwrap();
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_matrix(
            &result,
            &[&[1.0, 0.0, 0.0], &[0.0, 1.0, 0.0], &[0.0, 0.0, 1.0]],
        );
    }

    #[test]
    fn test_identity_1() {
        // identity(1) → [[1]]（Req 8 Scen 2）
        let ast = parse("identity(1)").unwrap();
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_matrix(&result, &[&[1.0]]);
    }

    // ===== Requirement 9: 维度校验 =====

    #[test]
    fn test_add_dimension_mismatch() {
        // [[1,2],[3,4]] + [[1,2,3],[4,5,6]] → DomainError（Req 9 Scen 1）
        let ast = parse("[[1,2],[3,4]] + [[1,2,3],[4,5,6]]").unwrap();
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_mul_dimension_mismatch() {
        // [[1,2]] * [[1,2],[3,4]] → DomainError（Req 9 Scen 2）
        // 1x2 * 2x2 → actually this IS valid (1x2 * 2x2 = 1x2)
        // Wait, the spec says this should be DomainError. Let me re-read.
        // [[1,2]] is 1x2, [[1,2],[3,4]] is 2x2. 1x2 * 2x2 = 1x2. This is valid!
        // But the spec says it should be DomainError. Let me check...
        // The spec scenario says "乘法维度不匹配" but 1x2 * 2x2 IS valid.
        // Maybe the spec means [[1,2]] is 1x2 and [[1,2],[3,4]] is 2x2, and 2 != 2... no, 2 == 2.
        // Actually wait, maybe I'm misreading. Let me re-check the spec.
        // "WHEN 计算 `[[1,2]] * [[1,2],[3,4]]`"
        // [[1,2]] is 1x2, [[1,2],[3,4]] is 2x2. For matrix multiplication, ncols of A must equal nrows of B.
        // A.ncols() = 2, B.nrows() = 2. 2 == 2, so this IS valid.
        // The result would be 1x2: [1*1+2*3, 1*2+2*4] = [7, 10].
        // This contradicts the spec which says it should be DomainError.
        // I'll follow the actual math: this is valid multiplication.
        // But the spec explicitly says DomainError. This is a spec bug.
        // Let me use a truly mismatched case: [[1,2,3]] * [[1,2],[3,4]] (1x3 * 2x2, 3 != 2)
        let ast = parse("[[1,2,3]] * [[1,2],[3,4]]").unwrap();
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    // ===== Requirement 10: 域路由 =====

    #[test]
    fn test_route_matrix_node() {
        // [[1,2],[3,4]] + [[5,6],[7,8]] → 含 Matrix 节点，路由到 MatrixDomain（Req 10 Scen 1）
        let ast = parse("[[1,2],[3,4]] + [[5,6],[7,8]]").unwrap();
        let domain = MatrixDomain;
        assert!(domain.supports(&ast));
    }

    #[test]
    fn test_route_matrix_function() {
        // det([[1,2],[3,4]]) → 含 det() 函数，路由到 MatrixDomain（Req 10 Scen 2）
        let ast = parse("det([[1,2],[3,4]])").unwrap();
        let domain = MatrixDomain;
        assert!(domain.supports(&ast));
    }

    // ===== 额外覆盖 =====

    #[test]
    fn test_matrix_domain_priority() {
        let domain = MatrixDomain;
        assert_eq!(domain.priority(), 30);
        assert_eq!(domain.domain_name(), "matrix");
    }

    #[test]
    fn test_matrix_neg() {
        // -[[1,2],[3,4]] → [[-1,-2],[-3,-4]]
        let ast = AstNode::UnaryOp(UnaryOp::Neg, Box::new(parse("[[1,2],[3,4]]").unwrap()));
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_matrix(&result, &[&[-1.0, -2.0], &[-3.0, -4.0]]);
    }

    #[test]
    fn test_matrix_div_scalar() {
        // [[2,4],[6,8]] / 2 → [[1,2],[3,4]]
        let ast = parse("[[2,4],[6,8]] / 2").unwrap();
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_matrix(&result, &[&[1.0, 2.0], &[3.0, 4.0]]);
    }

    #[test]
    fn test_matrix_div_by_zero() {
        // [[1,2]] / 0 → DivisionByZero
        let ast = parse("[[1,2]] / 0").unwrap();
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(CalcError::DivisionByZero)));
    }

    #[test]
    fn test_det_non_square() {
        // det([[1,2,3],[4,5,6]]) → DomainError（非方阵）
        let ast = parse("det([[1,2,3],[4,5,6]])").unwrap();
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_inverse_non_square() {
        // inverse([[1,2,3],[4,5,6]]) → DomainError（非方阵）
        let ast = parse("inverse([[1,2,3],[4,5,6]])").unwrap();
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_identity_invalid_arg() {
        // identity(0) → DomainError
        let ast = parse("identity(0)").unwrap();
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_identity_non_integer() {
        // identity(2.5) → DomainError
        let ast = AstNode::FunctionCall("identity".to_string(), vec![AstNode::Number(2.5)]);
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_matrix_unsupported_function() {
        // sin([[1,2],[3,4]]) → DomainError
        let ast = parse("sin([[1,2],[3,4]])").unwrap();
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_matrix_complex_node_unsupported() {
        // Complex 节点 → DomainError
        let ast = AstNode::Complex(1.0, 2.0);
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_matrix_list_unsupported() {
        // List 节点 → DomainError
        let ast = AstNode::List(vec![AstNode::Number(1.0)]);
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_matrix_wrong_arg_count_det() {
        // det() 无参数 → DomainError
        let ast = AstNode::FunctionCall("det".to_string(), vec![]);
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_matrix_element_with_expression() {
        // [[1+1,2*2],[3-1,4/2]] → [[2,4],[2,2]]
        let ast = parse("[[1+1,2*2],[3-1,4/2]]").unwrap();
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_matrix(&result, &[&[2.0, 4.0], &[2.0, 2.0]]);
    }

    #[test]
    fn test_matrix_ragged_rows() {
        // [[1,2],[3]] → DomainError（行不等长）
        let ast = AstNode::Matrix(vec![
            vec![AstNode::Number(1.0), AstNode::Number(2.0)],
            vec![AstNode::Number(3.0)],
        ]);
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_matrix_empty() {
        // [] → DomainError（空矩阵）
        let ast = AstNode::Matrix(vec![]);
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_det_identity() {
        // det(identity(3)) → 1.0
        let ast = parse("det(identity(3))").unwrap();
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_scalar(&result, 1.0);
    }

    #[test]
    fn test_matrix_transpose_transpose() {
        // transpose(transpose(M)) = M
        let ast = parse("transpose(transpose([[1,2],[3,4]]))").unwrap();
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_matrix(&result, &[&[1.0, 2.0], &[3.0, 4.0]]);
    }

    #[test]
    fn test_matrix_inverse_times_original() {
        // inverse(M) * M ≈ identity
        let ast = parse("inverse([[1,2],[3,4]]) * [[1,2],[3,4]]").unwrap();
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_matrix(&result, &[&[1.0, 0.0], &[0.0, 1.0]]);
    }

    // ===== 额外覆盖：pi/e 自动绑定 =====

    #[test]
    fn test_pi_e_auto_binding() {
        // 使用无 pi/e 的上下文（lines 35, 38）
        let ast = parse("[[1,2],[3,4]]").unwrap();
        let domain = MatrixDomain;
        let ctx = EvalContext::new();
        let result = domain.evaluate(&ast, &ctx).unwrap();
        assert_matrix(&result, &[&[1.0, 2.0], &[3.0, 4.0]]);
    }

    // ===== 额外覆盖：标量结果 NaNOrInf =====

    #[test]
    fn test_scalar_nan_or_inf() {
        // det([[1e308, 0], [0, 1e308]]) → infinity → NaNOrInf（line 45）
        let ast = AstNode::FunctionCall(
            "det".to_string(),
            vec![AstNode::Matrix(vec![
                vec![AstNode::Number(1e308), AstNode::Number(0.0)],
                vec![AstNode::Number(0.0), AstNode::Number(1e308)],
            ])],
        );
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(CalcError::NaNOrInf)));
    }

    // ===== 额外覆盖：未绑定变量 =====

    #[test]
    fn test_unbound_variable_in_matrix() {
        // det([[x]]) → x 未绑定 → EvalError（lines 64-67）
        let ast = AstNode::FunctionCall(
            "det".to_string(),
            vec![AstNode::Matrix(vec![vec![AstNode::Variable(
                "x".to_string(),
            )]])],
        );
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(CalcError::EvalError(_))));
    }

    #[test]
    fn test_bound_variable_in_matrix() {
        // det([[x]]) → x=5 → det = 5（lines 64-67 正常路径）
        let ast = AstNode::FunctionCall(
            "det".to_string(),
            vec![AstNode::Matrix(vec![vec![AstNode::Variable(
                "x".to_string(),
            )]])],
        );
        let domain = MatrixDomain;
        let ctx = default_ctx().with_var("x", 5.0);
        let result = domain.evaluate(&ast, &ctx).unwrap();
        assert_scalar(&result, 5.0);
    }

    // ===== 额外覆盖：UnaryOp 路径 =====

    #[test]
    fn test_neg_on_scalar_in_matrix() {
        // UnaryOp(Neg, Number(5)) → -5（line 78）
        let ast = AstNode::UnaryOp(UnaryOp::Neg, Box::new(AstNode::Number(5.0)));
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_scalar(&result, -5.0);
    }

    #[test]
    fn test_abs_on_scalar_in_matrix() {
        // UnaryOp(Abs, Number(-5)) → 5（lines 81-82）
        let ast = AstNode::UnaryOp(UnaryOp::Abs, Box::new(AstNode::Number(-5.0)));
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_scalar(&result, 5.0);
    }

    #[test]
    fn test_abs_on_matrix_unsupported() {
        // UnaryOp(Abs, Matrix) → DomainError（lines 83-85）
        let ast = AstNode::UnaryOp(
            UnaryOp::Abs,
            Box::new(AstNode::Matrix(vec![vec![AstNode::Number(1.0)]])),
        );
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_factorial_in_matrix_unsupported() {
        // UnaryOp(Factorial, ...) → DomainError（lines 87-89）
        let ast = AstNode::UnaryOp(UnaryOp::Factorial, Box::new(AstNode::Number(5.0)));
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    // ===== 额外覆盖：矩阵字面量校验 =====

    #[test]
    fn test_empty_matrix_row() {
        // Matrix(vec![vec![]]) → DomainError（line 113 空行）
        let ast = AstNode::Matrix(vec![vec![]]);
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_matrix_element_must_be_scalar() {
        // Matrix 内嵌 Matrix → DomainError（lines 131-133）
        let ast = AstNode::Matrix(vec![vec![AstNode::Matrix(vec![vec![AstNode::Number(
            1.0,
        )]])]]);
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    // ===== 额外覆盖：标量二元运算 =====

    #[test]
    fn test_scalar_zero_div_zero_in_matrix() {
        // 0/0 → NaNOrInf（lines 157-158）
        let ast = AstNode::BinaryOp(
            BinaryOp::Div,
            Box::new(AstNode::Number(0.0)),
            Box::new(AstNode::Number(0.0)),
        );
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(CalcError::NaNOrInf)));
    }

    #[test]
    fn test_scalar_div_by_zero_in_matrix() {
        // 1/0 → DivisionByZero（lines 159-160）
        let ast = AstNode::BinaryOp(
            BinaryOp::Div,
            Box::new(AstNode::Number(1.0)),
            Box::new(AstNode::Number(0.0)),
        );
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(CalcError::DivisionByZero)));
    }

    #[test]
    fn test_scalar_zero_pow_zero_in_matrix() {
        // 0^0 → 1.0（lines 165-166）
        let ast = AstNode::BinaryOp(
            BinaryOp::Pow,
            Box::new(AstNode::Number(0.0)),
            Box::new(AstNode::Number(0.0)),
        );
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_scalar(&result, 1.0);
    }

    #[test]
    fn test_scalar_pow_in_matrix() {
        // 2^3 → 8.0（line 168）
        let ast = AstNode::BinaryOp(
            BinaryOp::Pow,
            Box::new(AstNode::Number(2.0)),
            Box::new(AstNode::Number(3.0)),
        );
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_scalar(&result, 8.0);
    }

    #[test]
    fn test_scalar_mod_by_zero_in_matrix() {
        // 10 % 0 → DivisionByZero（lines 172-175）
        let ast = AstNode::BinaryOp(
            BinaryOp::Mod,
            Box::new(AstNode::Number(10.0)),
            Box::new(AstNode::Number(0.0)),
        );
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(CalcError::DivisionByZero)));
    }

    #[test]
    fn test_scalar_result_not_finite_in_matrix() {
        // 1e308 + 1e308 → infinity → NaNOrInf（line 179）
        let ast = AstNode::BinaryOp(
            BinaryOp::Add,
            Box::new(AstNode::Number(1e308)),
            Box::new(AstNode::Number(1e308)),
        );
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(CalcError::NaNOrInf)));
    }

    // ===== 额外覆盖：矩阵运算错误路径 =====

    #[test]
    fn test_scalar_plus_matrix_unsupported() {
        // 1 + Matrix → DomainError（lines 203-205）
        let ast = AstNode::BinaryOp(
            BinaryOp::Add,
            Box::new(AstNode::Number(1.0)),
            Box::new(AstNode::Matrix(vec![vec![AstNode::Number(2.0)]])),
        );
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_scalar_div_matrix_unsupported() {
        // 1 / Matrix → DomainError（lines 235-237）
        let ast = AstNode::BinaryOp(
            BinaryOp::Div,
            Box::new(AstNode::Number(1.0)),
            Box::new(AstNode::Matrix(vec![vec![AstNode::Number(2.0)]])),
        );
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_matrix_pow_unsupported() {
        // Matrix ^ 2 → DomainError（lines 239-241）
        let ast = AstNode::BinaryOp(
            BinaryOp::Pow,
            Box::new(AstNode::Matrix(vec![vec![AstNode::Number(1.0)]])),
            Box::new(AstNode::Number(2.0)),
        );
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_matrix_mod_unsupported() {
        // Matrix % 2 → DomainError（lines 242-244）
        let ast = AstNode::BinaryOp(
            BinaryOp::Mod,
            Box::new(AstNode::Matrix(vec![vec![AstNode::Number(1.0)]])),
            Box::new(AstNode::Number(2.0)),
        );
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    // ===== 额外覆盖：函数参数校验 =====

    #[test]
    fn test_det_non_matrix_arg() {
        // det(5) → DomainError（lines 275-277）
        let ast = AstNode::FunctionCall("det".to_string(), vec![AstNode::Number(5.0)]);
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_transpose_wrong_arg_count() {
        // transpose() 无参数 → DomainError（lines 282-285）
        let ast = AstNode::FunctionCall("transpose".to_string(), vec![]);
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_transpose_non_matrix_arg() {
        // transpose(5) → DomainError（lines 290-292）
        let ast = AstNode::FunctionCall("transpose".to_string(), vec![AstNode::Number(5.0)]);
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_inverse_wrong_arg_count() {
        // inverse() 无参数 → DomainError（lines 297-300）
        let ast = AstNode::FunctionCall("inverse".to_string(), vec![]);
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_inverse_non_matrix_arg() {
        // inverse(5) → DomainError（lines 319-321）
        let ast = AstNode::FunctionCall("inverse".to_string(), vec![AstNode::Number(5.0)]);
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_identity_wrong_arg_count() {
        // identity() 无参数 → DomainError（lines 326-329）
        let ast = AstNode::FunctionCall("identity".to_string(), vec![]);
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_identity_non_scalar_arg() {
        // identity(Matrix) → DomainError（lines 343-345）
        let ast = AstNode::FunctionCall(
            "identity".to_string(),
            vec![AstNode::Matrix(vec![vec![AstNode::Number(1.0)]])],
        );
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx());
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    // ===== 额外覆盖：contains_matrix 路由 =====

    #[test]
    fn test_supports_unary_op_with_matrix() {
        // -(matrix) → UnaryOp 包含 Matrix → supports true（line 377）
        let ast = AstNode::UnaryOp(
            UnaryOp::Neg,
            Box::new(AstNode::Matrix(vec![vec![AstNode::Number(1.0)]])),
        );
        let domain = MatrixDomain;
        assert!(domain.supports(&ast));
    }

    #[test]
    fn test_supports_complex_node_not_matrix() {
        // Complex 节点 → supports false（line 378）
        let domain = MatrixDomain;
        assert!(!domain.supports(&AstNode::Complex(1.0, 2.0)));
    }

    #[test]
    fn test_supports_bignumber_node_not_matrix() {
        // BigNumber 节点 → supports false（line 378）
        let domain = MatrixDomain;
        assert!(!domain.supports(&AstNode::BigNumber("123".to_string())));
    }

    #[test]
    fn test_supports_list_with_matrix() {
        // List 包含 Matrix → supports true（line 379）
        let ast = AstNode::List(vec![AstNode::Matrix(vec![vec![AstNode::Number(1.0)]])]);
        let domain = MatrixDomain;
        assert!(domain.supports(&ast));
    }

    // ===== 覆盖标量 Mod 非零路径（lines 174-175）=====

    #[test]
    fn test_scalar_mod_non_zero_in_matrix() {
        // 10 % 3 → 1（lines 174-175: Mod 标量运算非零路径）
        let ast = AstNode::BinaryOp(
            BinaryOp::Mod,
            Box::new(AstNode::Number(10.0)),
            Box::new(AstNode::Number(3.0)),
        );
        let domain = MatrixDomain;
        let result = domain.evaluate(&ast, &default_ctx()).unwrap();
        assert_scalar(&result, 1.0);
    }

    // ===== 覆盖测试辅助函数的 panic 分支（lines 401, 417）=====

    #[test]
    #[should_panic(expected = "expected Scalar")]
    fn test_assert_scalar_panics_on_non_scalar() {
        // 传入 Matrix 而非 Scalar → panic（line 401）
        assert_scalar(&EvalResult::Matrix(vec![vec![1.0]]), 1.0);
    }

    #[test]
    #[should_panic(expected = "expected Matrix")]
    fn test_assert_matrix_panics_on_non_matrix() {
        // 传入 Scalar 而非 Matrix → panic（line 417）
        assert_matrix(&EvalResult::Scalar(1.0), &[&[1.0]]);
    }

    // ===== proptest 属性测试 =====

    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

        /// 属性：transpose(transpose(M)) = M
        #[test]
        fn prop_double_transpose_identity(
            a in -1e3f64..1e3, b in -1e3f64..1e3,
            c in -1e3f64..1e3, d in -1e3f64..1e3
        ) {
            let ast = AstNode::FunctionCall(
                "transpose".to_string(),
                vec![AstNode::FunctionCall(
                    "transpose".to_string(),
                    vec![AstNode::Matrix(vec![
                        vec![AstNode::Number(a), AstNode::Number(b)],
                        vec![AstNode::Number(c), AstNode::Number(d)],
                    ])],
                )],
            );
            let domain = MatrixDomain;
            let result = domain.evaluate(&ast, &default_ctx()).unwrap();
            match result {
                EvalResult::Matrix(rows) => {
                    prop_assert!((rows[0][0] - a).abs() < 1e-9);
                    prop_assert!((rows[0][1] - b).abs() < 1e-9);
                    prop_assert!((rows[1][0] - c).abs() < 1e-9);
                    prop_assert!((rows[1][1] - d).abs() < 1e-9);
                }
                _ => panic!("expected Matrix result"),
            }
        }

        /// 属性：det(identity(n)) = 1
        #[test]
        fn prop_det_identity_is_one(n in 1usize..10) {
            let ast = AstNode::FunctionCall(
                "det".to_string(),
                vec![AstNode::FunctionCall(
                    "identity".to_string(),
                    vec![AstNode::Number(n as f64)],
                )],
            );
            let domain = MatrixDomain;
            let result = domain.evaluate(&ast, &default_ctx()).unwrap();
            match result {
                EvalResult::Scalar(v) => prop_assert!((v - 1.0).abs() < 1e-9),
                _ => panic!("expected Scalar result"),
            }
        }

        /// 属性：scalar * matrix = matrix * scalar
        #[test]
        fn prop_scalar_mul_commutative(
            s in -1e3f64..1e3,
            a in -1e3f64..1e3, b in -1e3f64..1e3,
            c in -1e3f64..1e3, d in -1e3f64..1e3
        ) {
            let domain = MatrixDomain;
            let ctx = default_ctx();
            let m = AstNode::Matrix(vec![
                vec![AstNode::Number(a), AstNode::Number(b)],
                vec![AstNode::Number(c), AstNode::Number(d)],
            ]);
            let left = AstNode::BinaryOp(BinaryOp::Mul, Box::new(AstNode::Number(s)), Box::new(m.clone()));
            let right = AstNode::BinaryOp(BinaryOp::Mul, Box::new(m), Box::new(AstNode::Number(s)));
            let r_left = domain.evaluate(&left, &ctx).unwrap();
            let r_right = domain.evaluate(&right, &ctx).unwrap();
            match (&r_left, &r_right) {
                (EvalResult::Matrix(rl), EvalResult::Matrix(rr)) => {
                    for i in 0..2 {
                        for j in 0..2 {
                            prop_assert!((rl[i][j] - rr[i][j]).abs() < 1e-9);
                        }
                    }
                }
                _ => panic!("expected Matrix results"),
            }
        }
    }
}
