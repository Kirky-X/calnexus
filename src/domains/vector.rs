// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! Vector 计算域：向量算术、点积、叉积、模长、夹角、混合积、归一化。
//!
//! 设计依据：
//! - vector-domain spec：7 个 requirements / 13+ scenarios
//! - design.md D2（复用 AstNode::List）、D6（priority=30）
//!
//! 路由策略：
//! - AST 含向量函数调用（dot/cross/norm/angle/normalize/scalar_triple）时路由至本域
//! - AST 含 List 节点参与的 BinaryOp(Add/Sub/Mul) 时路由至本域（向量算术）
//!
//! 输入：AstNode::List 节点，元素必须为标量数值。
//! 输出：EvalResult::Vector 或 EvalResult::Scalar。

use crate::core::CalculationDomain;
use crate::core::{AstNode, BinaryOp, CalcError, EvalContext, EvalResult, UnaryOp};
use nalgebra::DVector;

/// 向量函数白名单。
const VECTOR_FUNCTIONS: &[&str] = &[
    "dot",
    "cross",
    "norm",
    "angle",
    "normalize",
    "scalar_triple",
];

/// Vector 计算域。
///
/// priority=30，支持 dot/cross/norm/angle/normalize/scalar_triple 及向量算术。
pub struct VectorDomain;

impl CalculationDomain for VectorDomain {
    fn domain_name(&self) -> &str {
        "vector"
    }

    fn priority(&self) -> u8 {
        30
    }

    fn supports(&self, ast: &AstNode) -> bool {
        contains_vector_function(ast) || contains_vector_arithmetic(ast)
    }

    fn evaluate(&self, ast: &AstNode, ctx: &EvalContext) -> Result<EvalResult, CalcError> {
        self.eval_node(ast, ctx)
    }
}

impl Default for VectorDomain {
    fn default() -> Self {
        Self
    }
}

impl VectorDomain {
    /// 递归求值 AST 节点。
    fn eval_node(&self, ast: &AstNode, ctx: &EvalContext) -> Result<EvalResult, CalcError> {
        match ast {
            AstNode::FunctionCall(name, args) => self.eval_function(name, args, ctx),
            AstNode::Number(n) => Ok(EvalResult::Scalar(*n)),
            AstNode::BigNumber(s) => {
                let n: f64 = s
                    .parse()
                    .map_err(|_| CalcError::domain(format!("invalid big number: {}", s)))?;
                Ok(EvalResult::Scalar(n))
            }
            AstNode::Variable(name) => ctx
                .get_var(name)
                .map(EvalResult::Scalar)
                .ok_or_else(|| CalcError::eval(format!("unbound variable: {}", name))),
            AstNode::List(_) => {
                // 裸 List 节点 → 转为 Vector 结果
                let v = self.list_to_vector(ast, ctx)?;
                Ok(EvalResult::Vector(v))
            }
            AstNode::BinaryOp(op, l, r) => {
                // 向量算术：List op List 或 scalar * List
                let is_left_list = is_list_node(l);
                let is_right_list = is_list_node(r);
                if is_left_list || is_right_list {
                    return self.eval_vector_binary(*op, l, r, ctx);
                }
                // 标量算术
                let a = self.eval_scalar(l, ctx)?;
                let b = self.eval_scalar(r, ctx)?;
                Ok(EvalResult::Scalar(self.eval_scalar_binary(*op, a, b)?))
            }
            AstNode::UnaryOp(op, e) => {
                match op {
                    UnaryOp::Neg => {
                        if is_list_node(e) {
                            let v = self.list_to_vector(e, ctx)?;
                            let neg: Vec<f64> = v.iter().map(|x| -x).collect();
                            Ok(EvalResult::Vector(neg))
                        } else {
                            let v = self.eval_scalar(e, ctx)?;
                            Ok(EvalResult::Scalar(-v))
                        }
                    }
                    UnaryOp::Abs => {
                        // norm(list) 的等价
                        if is_list_node(e) {
                            let v = self.list_to_vector(e, ctx)?;
                            let dvec = DVector::from_vec(v);
                            Ok(EvalResult::Scalar(dvec.norm()))
                        } else {
                            let v = self.eval_scalar(e, ctx)?;
                            Ok(EvalResult::Scalar(v.abs()))
                        }
                    }
                    UnaryOp::Factorial => Err(CalcError::domain(
                        "factorial not supported in vector domain".to_string(),
                    )),
                }
            }
            AstNode::Complex(_, _) | AstNode::Matrix(_) => Err(CalcError::domain(format!(
                "vector domain does not support this node type: {:?}",
                ast
            ))),
        }
    }

    /// 求值标量节点，返回 f64。
    fn eval_scalar(&self, ast: &AstNode, ctx: &EvalContext) -> Result<f64, CalcError> {
        match ast {
            AstNode::Number(n) => Ok(*n),
            AstNode::BigNumber(s) => s
                .parse::<f64>()
                .map_err(|_| CalcError::domain(format!("invalid big number: {}", s))),
            AstNode::Variable(name) => ctx
                .get_var(name)
                .ok_or_else(|| CalcError::eval(format!("unbound variable: {}", name))),
            AstNode::BinaryOp(op, l, r) => {
                let a = self.eval_scalar(l, ctx)?;
                let b = self.eval_scalar(r, ctx)?;
                self.eval_scalar_binary(*op, a, b)
            }
            AstNode::UnaryOp(op, e) => {
                let v = self.eval_scalar(e, ctx)?;
                match op {
                    UnaryOp::Neg => Ok(-v),
                    UnaryOp::Abs => Ok(v.abs()),
                    UnaryOp::Factorial => Err(CalcError::domain(
                        "factorial not supported in vector domain".to_string(),
                    )),
                }
            }
            _ => Err(CalcError::domain(format!(
                "expected scalar expression, got: {:?}",
                ast
            ))),
        }
    }

    /// 标量二元运算。
    fn eval_scalar_binary(&self, op: BinaryOp, a: f64, b: f64) -> Result<f64, CalcError> {
        let result = match op {
            BinaryOp::Add => a + b,
            BinaryOp::Sub => a - b,
            BinaryOp::Mul => a * b,
            BinaryOp::Div => {
                if b == 0.0 {
                    if a == 0.0 {
                        return Err(CalcError::nan_or_inf());
                    }
                    return Err(CalcError::division_by_zero());
                }
                a / b
            }
            BinaryOp::Pow => a.powf(b),
            BinaryOp::Mod => {
                if b == 0.0 {
                    return Err(CalcError::division_by_zero());
                }
                a % b
            }
        };
        if !result.is_finite() {
            return Err(CalcError::nan_or_inf());
        }
        Ok(result)
    }

    /// 向量二元运算：List op List 或 scalar op List。
    fn eval_vector_binary(
        &self,
        op: BinaryOp,
        l: &AstNode,
        r: &AstNode,
        ctx: &EvalContext,
    ) -> Result<EvalResult, CalcError> {
        let is_left_list = is_list_node(l);
        let is_right_list = is_list_node(r);

        match (is_left_list, is_right_list, op) {
            // List + List / List - List：逐元素
            (true, true, BinaryOp::Add) | (true, true, BinaryOp::Sub) => {
                let a = self.list_to_vector(l, ctx)?;
                let b = self.list_to_vector(r, ctx)?;
                if a.len() != b.len() {
                    return Err(CalcError::domain(format!(
                        "vector dimension mismatch: {} vs {}",
                        a.len(),
                        b.len()
                    )));
                }
                let result: Vec<f64> = match op {
                    BinaryOp::Add => a.iter().zip(b.iter()).map(|(x, y)| x + y).collect(),
                    BinaryOp::Sub => a.iter().zip(b.iter()).map(|(x, y)| x - y).collect(),
                    _ => unreachable!(),
                };
                Ok(EvalResult::Vector(result))
            }
            // scalar * List / List * scalar：数乘
            (false, true, BinaryOp::Mul) => {
                let scalar = self.eval_scalar(l, ctx)?;
                let v = self.list_to_vector(r, ctx)?;
                let result: Vec<f64> = v.iter().map(|x| scalar * x).collect();
                Ok(EvalResult::Vector(result))
            }
            (true, false, BinaryOp::Mul) => {
                let scalar = self.eval_scalar(r, ctx)?;
                let v = self.list_to_vector(l, ctx)?;
                let result: Vec<f64> = v.iter().map(|x| scalar * x).collect();
                Ok(EvalResult::Vector(result))
            }
            _ => Err(CalcError::domain(format!(
                "unsupported vector binary operation: {:?}",
                op
            ))),
        }
    }

    /// 求值向量函数调用：按函数名分发到对应的处理方法。
    fn eval_function(
        &self,
        name: &str,
        args: &[AstNode],
        ctx: &EvalContext,
    ) -> Result<EvalResult, CalcError> {
        if !VECTOR_FUNCTIONS.contains(&name) {
            return Err(CalcError::domain(format!(
                "unsupported function in vector domain: {}",
                name
            )));
        }
        match name {
            "dot" => self.eval_dot(args, ctx),
            "cross" => self.eval_cross(args, ctx),
            "norm" => self.eval_norm(args, ctx),
            "angle" => self.eval_angle(args, ctx),
            "normalize" => self.eval_normalize(args, ctx),
            "scalar_triple" => self.eval_scalar_triple(args, ctx),
            _ => unreachable!(),
        }
    }

    /// dot(a, b)：向量点积，返回标量。
    fn eval_dot(&self, args: &[AstNode], ctx: &EvalContext) -> Result<EvalResult, CalcError> {
        if args.len() != 2 {
            return Err(CalcError::domain(format!(
                "dot() requires exactly 2 arguments, got {}",
                args.len()
            )));
        }
        let a = self.list_to_vector(&args[0], ctx)?;
        let b = self.list_to_vector(&args[1], ctx)?;
        if a.len() != b.len() {
            return Err(CalcError::domain(format!(
                "dot(): dimension mismatch {} vs {}",
                a.len(),
                b.len()
            )));
        }
        let dv_a = DVector::from_vec(a);
        let dv_b = DVector::from_vec(b);
        Ok(EvalResult::Scalar(dv_a.dot(&dv_b)))
    }

    /// cross(a, b)：三维向量叉积，返回向量。
    fn eval_cross(&self, args: &[AstNode], ctx: &EvalContext) -> Result<EvalResult, CalcError> {
        if args.len() != 2 {
            return Err(CalcError::domain(format!(
                "cross() requires exactly 2 arguments, got {}",
                args.len()
            )));
        }
        let a = self.list_to_vector(&args[0], ctx)?;
        let b = self.list_to_vector(&args[1], ctx)?;
        if a.len() != 3 || b.len() != 3 {
            return Err(CalcError::domain(
                "cross() requires 3-dimensional vectors".to_string(),
            ));
        }
        let cross = vec![
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ];
        Ok(EvalResult::Vector(cross))
    }

    /// norm(v)：向量模长（L2 范数），返回标量。
    fn eval_norm(&self, args: &[AstNode], ctx: &EvalContext) -> Result<EvalResult, CalcError> {
        if args.len() != 1 {
            return Err(CalcError::domain(format!(
                "norm() requires exactly 1 argument, got {}",
                args.len()
            )));
        }
        let v = self.list_to_vector(&args[0], ctx)?;
        let dv = DVector::from_vec(v);
        Ok(EvalResult::Scalar(dv.norm()))
    }

    /// angle(a, b)：两向量夹角（弧度），返回标量。
    fn eval_angle(&self, args: &[AstNode], ctx: &EvalContext) -> Result<EvalResult, CalcError> {
        if args.len() != 2 {
            return Err(CalcError::domain(format!(
                "angle() requires exactly 2 arguments, got {}",
                args.len()
            )));
        }
        let a = self.list_to_vector(&args[0], ctx)?;
        let b = self.list_to_vector(&args[1], ctx)?;
        if a.len() != b.len() {
            return Err(CalcError::domain(format!(
                "angle(): dimension mismatch {} vs {}",
                a.len(),
                b.len()
            )));
        }
        let dv_a = DVector::from_vec(a);
        let dv_b = DVector::from_vec(b);
        let norm_a = dv_a.norm();
        let norm_b = dv_b.norm();
        if norm_a == 0.0 || norm_b == 0.0 {
            return Err(CalcError::domain(
                "angle(): zero vector has no angle".to_string(),
            ));
        }
        let cos_theta = dv_a.dot(&dv_b) / (norm_a * norm_b);
        let cos_clamped = cos_theta.clamp(-1.0, 1.0);
        Ok(EvalResult::Scalar(cos_clamped.acos()))
    }

    /// normalize(v)：向量归一化为单位向量，返回向量。
    fn eval_normalize(&self, args: &[AstNode], ctx: &EvalContext) -> Result<EvalResult, CalcError> {
        if args.len() != 1 {
            return Err(CalcError::domain(format!(
                "normalize() requires exactly 1 argument, got {}",
                args.len()
            )));
        }
        let v = self.list_to_vector(&args[0], ctx)?;
        let dv = DVector::from_vec(v);
        let norm = dv.norm();
        if norm == 0.0 {
            return Err(CalcError::domain(
                "normalize(): cannot normalize zero vector".to_string(),
            ));
        }
        let normalized = &dv / norm;
        Ok(EvalResult::Vector(normalized.iter().cloned().collect()))
    }

    /// scalar_triple(a, b, c)：三维向量混合积 a·(b×c)，返回标量。
    fn eval_scalar_triple(
        &self,
        args: &[AstNode],
        ctx: &EvalContext,
    ) -> Result<EvalResult, CalcError> {
        if args.len() != 3 {
            return Err(CalcError::domain(format!(
                "scalar_triple() requires exactly 3 arguments, got {}",
                args.len()
            )));
        }
        let a = self.list_to_vector(&args[0], ctx)?;
        let b = self.list_to_vector(&args[1], ctx)?;
        let c = self.list_to_vector(&args[2], ctx)?;
        if a.len() != 3 || b.len() != 3 || c.len() != 3 {
            return Err(CalcError::domain(
                "scalar_triple() requires 3-dimensional vectors".to_string(),
            ));
        }
        let cross = [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ];
        let result = cross[0] * c[0] + cross[1] * c[1] + cross[2] * c[2];
        Ok(EvalResult::Scalar(result))
    }

    /// 将 AstNode::List 转为 Vec<f64>。非数值元素返回 DomainError。
    fn list_to_vector(&self, ast: &AstNode, ctx: &EvalContext) -> Result<Vec<f64>, CalcError> {
        match ast {
            AstNode::List(elements) => {
                let mut values = Vec::with_capacity(elements.len());
                for elem in elements {
                    let v = self.eval_scalar(elem, ctx)?;
                    if !v.is_finite() {
                        return Err(CalcError::nan_or_inf());
                    }
                    values.push(v);
                }
                Ok(values)
            }
            _ => Err(CalcError::domain(format!(
                "expected vector (list), got: {:?}",
                ast
            ))),
        }
    }
}

/// 判断节点是否为 List（或含 List 的表达式）。
fn is_list_node(ast: &AstNode) -> bool {
    matches!(ast, AstNode::List(_))
}

/// 递归检查 AST 是否含向量函数调用。
fn contains_vector_function(ast: &AstNode) -> bool {
    match ast {
        AstNode::FunctionCall(name, _) if VECTOR_FUNCTIONS.contains(&name.as_str()) => true,
        AstNode::FunctionCall(_, args) => args.iter().any(contains_vector_function),
        AstNode::BinaryOp(_, l, r) => contains_vector_function(l) || contains_vector_function(r),
        AstNode::UnaryOp(_, e) => contains_vector_function(e),
        AstNode::Matrix(rows) => rows.iter().flatten().any(contains_vector_function),
        AstNode::List(elements) => elements.iter().any(contains_vector_function),
        AstNode::Number(_)
        | AstNode::Variable(_)
        | AstNode::Complex(_, _)
        | AstNode::BigNumber(_) => false,
    }
}

/// 检查 AST 是否含 List 节点参与的 BinaryOp(Add/Sub/Mul)（向量算术）。
fn contains_vector_arithmetic(ast: &AstNode) -> bool {
    match ast {
        AstNode::BinaryOp(op, l, r) => {
            let is_arith = matches!(op, BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul);
            if is_arith && (is_list_node(l) || is_list_node(r)) {
                return true;
            }
            contains_vector_arithmetic(l) || contains_vector_arithmetic(r)
        }
        AstNode::UnaryOp(_, e) => contains_vector_arithmetic(e),
        AstNode::FunctionCall(_, args) => args.iter().any(contains_vector_arithmetic),
        AstNode::Matrix(rows) => rows.iter().flatten().any(contains_vector_arithmetic),
        AstNode::List(elements) => elements.iter().any(contains_vector_arithmetic),
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
    use crate::core::ErrorKind;

    fn eval(input: &str) -> Result<EvalResult, CalcError> {
        let ast = parse(input).unwrap();
        let domain = VectorDomain;
        let ctx = EvalContext::new();
        domain.evaluate(&ast, &ctx)
    }

    fn eval_scalar(input: &str) -> Result<f64, CalcError> {
        eval(input).map(|r| r.as_scalar().expect("expected scalar result"))
    }

    fn eval_vector(input: &str) -> Result<Vec<f64>, CalcError> {
        eval(input).map(|r| r.as_vector().expect("expected vector result").clone())
    }

    fn assert_approx(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 1e-9,
            "expected {} but got {}",
            expected,
            actual
        );
    }

    // ===== UT-VEC-001: 向量加法 =====

    #[test]
    fn test_vector_add() {
        let result = eval_vector("[1,2,3]+[4,5,6]").unwrap();
        assert_eq!(result, vec![5.0, 7.0, 9.0]);
    }

    // ===== UT-VEC-002: 向量减法 =====

    #[test]
    fn test_vector_sub() {
        let result = eval_vector("[5,7,9]-[1,2,3]").unwrap();
        assert_eq!(result, vec![4.0, 5.0, 6.0]);
    }

    // ===== UT-VEC-003: 向量数乘 =====

    #[test]
    fn test_vector_scalar_mul() {
        let result = eval_vector("3*[1,2,3]").unwrap();
        assert_eq!(result, vec![3.0, 6.0, 9.0]);
    }

    #[test]
    fn test_vector_scalar_mul_right() {
        let result = eval_vector("[1,2,3]*3").unwrap();
        assert_eq!(result, vec![3.0, 6.0, 9.0]);
    }

    // ===== UT-VEC-004: 点积 =====

    #[test]
    fn test_dot_product() {
        assert_eq!(eval_scalar("dot([1,2,3],[4,5,6])").unwrap(), 32.0);
    }

    // ===== UT-VEC-005: 叉积 =====

    #[test]
    fn test_cross_product() {
        let result = eval_vector("cross([1,0,0],[0,1,0])").unwrap();
        assert_eq!(result, vec![0.0, 0.0, 1.0]);
    }

    // ===== UT-VEC-006: 模长 =====

    #[test]
    fn test_norm() {
        assert_eq!(eval_scalar("norm([3,4])").unwrap(), 5.0);
    }

    // ===== UT-VEC-007: 夹角 =====

    #[test]
    fn test_angle() {
        let result = eval_scalar("angle([1,0],[0,1])").unwrap();
        assert_approx(result, std::f64::consts::PI / 2.0);
    }

    // ===== UT-VEC-008: 混合积 =====

    #[test]
    fn test_scalar_triple() {
        assert_eq!(
            eval_scalar("scalar_triple([1,0,0],[0,1,0],[0,0,1])").unwrap(),
            1.0
        );
    }

    // ===== UT-VEC-009: 维度不匹配 =====

    #[test]
    fn test_dimension_mismatch_dot() {
        let result = eval("dot([1,2],[1,2,3])");
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    // ===== UT-VEC-010: 零向量 =====

    #[test]
    fn test_zero_vector_norm() {
        assert_eq!(eval_scalar("norm([0,0,0])").unwrap(), 0.0);
    }

    // ===== 补充测试 =====

    #[test]
    fn test_normalize() {
        let result = eval_vector("normalize([3,4])").unwrap();
        assert_approx(result[0], 0.6);
        assert_approx(result[1], 0.8);
    }

    #[test]
    fn test_normalize_zero_vector() {
        let result = eval("normalize([0,0])");
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_angle_zero_vector() {
        let result = eval("angle([0,0],[1,0])");
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_cross_2d_error() {
        let result = eval("cross([1,2],[3,4])");
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_scalar_triple_2d_error() {
        let result = eval("scalar_triple([1,0],[0,1],[1,1])");
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_vector_add_mismatch() {
        let result = eval("[1,2]+[1,2,3]");
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_vector_neg() {
        let result = eval_vector("-[1,2,3]").unwrap();
        assert_eq!(result, vec![-1.0, -2.0, -3.0]);
    }

    #[test]
    fn test_bare_list() {
        let result = eval_vector("[1,2,3]").unwrap();
        assert_eq!(result, vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_dot_2d() {
        assert_eq!(eval_scalar("dot([1,2],[3,4])").unwrap(), 11.0);
    }

    #[test]
    fn test_norm_3d() {
        assert_approx(eval_scalar("norm([1,2,2])").unwrap(), 3.0);
    }

    #[test]
    fn test_cross_known() {
        let result = eval_vector("cross([2,3,4],[5,6,7])").unwrap();
        // cross = (3*7-4*6, 4*5-2*7, 2*6-3*5) = (-3, 6, -3)
        assert_eq!(result, vec![-3.0, 6.0, -3.0]);
    }

    #[test]
    fn test_scalar_triple_known() {
        // scalar_triple([1,2,3],[4,5,6],[7,8,9]) = 0 (共面)
        assert_eq!(
            eval_scalar("scalar_triple([1,2,3],[4,5,6],[7,8,9])").unwrap(),
            0.0
        );
    }

    #[test]
    fn test_angle_parallel() {
        let result = eval_scalar("angle([1,0],[2,0])").unwrap();
        assert_approx(result, 0.0);
    }

    #[test]
    fn test_angle_opposite() {
        let result = eval_scalar("angle([1,0],[-1,0])").unwrap();
        assert_approx(result, std::f64::consts::PI);
    }

    // ===== 域元信息测试 =====

    #[test]
    fn test_domain_info() {
        let domain = VectorDomain;
        assert_eq!(domain.domain_name(), "vector");
        assert_eq!(domain.priority(), 30);
    }

    #[test]
    fn test_default_impl() {
        let domain = VectorDomain;
        assert_eq!(domain.domain_name(), "vector");
    }

    #[test]
    fn test_supports_dot() {
        let ast = parse("dot([1,2],[3,4])").unwrap();
        assert!(VectorDomain.supports(&ast));
    }

    #[test]
    fn test_supports_vector_add() {
        let ast = parse("[1,2]+[3,4]").unwrap();
        assert!(VectorDomain.supports(&ast));
    }

    #[test]
    fn test_supports_scalar_mul() {
        let ast = parse("3*[1,2,3]").unwrap();
        assert!(VectorDomain.supports(&ast));
    }

    #[test]
    fn test_not_supports_plain_arithmetic() {
        let ast = parse("1+2").unwrap();
        assert!(!VectorDomain.supports(&ast));
    }

    #[test]
    fn test_not_supports_statistics() {
        let ast = parse("mean([1,2,3])").unwrap();
        assert!(!VectorDomain.supports(&ast));
    }

    // ===== 错误路径测试 =====

    #[test]
    fn test_unsupported_function() {
        let ast = AstNode::FunctionCall("sin".to_string(), vec![AstNode::Number(1.0)]);
        let result = VectorDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_dot_wrong_args() {
        let ast = AstNode::FunctionCall("dot".to_string(), vec![AstNode::Number(1.0)]);
        let result = VectorDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_norm_wrong_args() {
        let ast = AstNode::FunctionCall(
            "norm".to_string(),
            vec![AstNode::Number(1.0), AstNode::Number(2.0)],
        );
        let result = VectorDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_non_list_argument() {
        let ast = AstNode::FunctionCall(
            "dot".to_string(),
            vec![AstNode::Number(1.0), AstNode::Number(2.0)],
        );
        let result = VectorDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_unbound_variable_in_list() {
        let ast = AstNode::List(vec![AstNode::Variable("y".to_string())]);
        let result = VectorDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Eval));
    }

    #[test]
    fn test_complex_in_list_rejected() {
        let ast = AstNode::List(vec![AstNode::Complex(1.0, 2.0)]);
        let result = VectorDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_unsupported_vector_op() {
        // List / List 不支持
        let ast = AstNode::BinaryOp(
            BinaryOp::Div,
            Box::new(AstNode::List(vec![AstNode::Number(1.0)])),
            Box::new(AstNode::List(vec![AstNode::Number(2.0)])),
        );
        let result = VectorDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_unary_factorial_rejected() {
        let ast = AstNode::UnaryOp(UnaryOp::Factorial, Box::new(AstNode::Number(5.0)));
        let result = VectorDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_matrix_rejected() {
        let ast = AstNode::Matrix(vec![vec![AstNode::Number(1.0)]]);
        let result = VectorDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_div_by_zero_scalar() {
        let ast = AstNode::BinaryOp(
            BinaryOp::Div,
            Box::new(AstNode::Number(5.0)),
            Box::new(AstNode::Number(0.0)),
        );
        let result = VectorDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::DivisionByZero));
    }

    #[test]
    fn test_zero_div_zero_scalar() {
        let ast = AstNode::BinaryOp(
            BinaryOp::Div,
            Box::new(AstNode::Number(0.0)),
            Box::new(AstNode::Number(0.0)),
        );
        let result = VectorDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::NaNOrInf));
    }

    #[test]
    fn test_unary_abs_scalar() {
        let ast = AstNode::UnaryOp(UnaryOp::Abs, Box::new(AstNode::Number(-5.0)));
        let result = VectorDomain.evaluate(&ast, &EvalContext::new()).unwrap();
        assert_eq!(result.as_scalar().unwrap(), 5.0);
    }

    #[test]
    fn test_unary_abs_vector() {
        let ast = AstNode::UnaryOp(
            UnaryOp::Abs,
            Box::new(AstNode::List(vec![
                AstNode::Number(3.0),
                AstNode::Number(4.0),
            ])),
        );
        let result = VectorDomain.evaluate(&ast, &EvalContext::new()).unwrap();
        assert_approx(result.as_scalar().unwrap(), 5.0);
    }

    // ===== 覆盖率补充测试 =====

    #[test]
    fn test_eval_node_bignumber() {
        // eval_node BigNumber path
        let ast = AstNode::BigNumber("42".to_string());
        let result = VectorDomain.evaluate(&ast, &EvalContext::new()).unwrap();
        assert_eq!(result.as_scalar().unwrap(), 42.0);
    }

    #[test]
    fn test_eval_node_bignumber_invalid() {
        let ast = AstNode::BigNumber("not_a_number".to_string());
        let result = VectorDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_eval_node_variable_bound() {
        // eval_node Variable with bound value
        let ctx = EvalContext::new().with_var("x", 7.0);
        let ast = AstNode::Variable("x".to_string());
        let result = VectorDomain.evaluate(&ast, &ctx).unwrap();
        assert_eq!(result.as_scalar().unwrap(), 7.0);
    }

    #[test]
    fn test_eval_node_unbound_variable() {
        let ast = AstNode::Variable("y".to_string());
        let result = VectorDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Eval));
    }

    #[test]
    fn test_eval_scalar_binaryop() {
        // eval_scalar with nested BinaryOp (plain scalars, no List): (2+3) * (4-1)
        let ast = AstNode::BinaryOp(
            BinaryOp::Mul,
            Box::new(parse("2+3").unwrap()),
            Box::new(parse("4-1").unwrap()),
        );
        let result = VectorDomain.evaluate(&ast, &EvalContext::new()).unwrap();
        // 5 * 3 = 15
        assert_eq!(result.as_scalar().unwrap(), 15.0);
    }

    #[test]
    fn test_eval_scalar_unary_neg() {
        // eval_scalar UnaryOp::Neg via vector binary: -3 * [1,2]
        let ast = AstNode::BinaryOp(
            BinaryOp::Mul,
            Box::new(AstNode::UnaryOp(
                UnaryOp::Neg,
                Box::new(AstNode::Number(3.0)),
            )),
            Box::new(AstNode::List(vec![
                AstNode::Number(1.0),
                AstNode::Number(2.0),
            ])),
        );
        let result = VectorDomain.evaluate(&ast, &EvalContext::new()).unwrap();
        assert_eq!(result.as_vector().unwrap(), &vec![-3.0, -6.0]);
    }

    #[test]
    fn test_eval_scalar_unary_abs() {
        // eval_scalar UnaryOp::Abs: abs(-3) * [1,2]
        let ast = AstNode::BinaryOp(
            BinaryOp::Mul,
            Box::new(AstNode::UnaryOp(
                UnaryOp::Abs,
                Box::new(AstNode::Number(-3.0)),
            )),
            Box::new(AstNode::List(vec![
                AstNode::Number(1.0),
                AstNode::Number(2.0),
            ])),
        );
        let result = VectorDomain.evaluate(&ast, &EvalContext::new()).unwrap();
        assert_eq!(result.as_vector().unwrap(), &vec![3.0, 6.0]);
    }

    #[test]
    fn test_eval_scalar_bignumber() {
        // eval_scalar BigNumber path: poly_eval-like scalar context
        let ast = AstNode::BinaryOp(
            BinaryOp::Mul,
            Box::new(AstNode::BigNumber("3".to_string())),
            Box::new(AstNode::List(vec![
                AstNode::Number(1.0),
                AstNode::Number(2.0),
            ])),
        );
        let result = VectorDomain.evaluate(&ast, &EvalContext::new()).unwrap();
        assert_eq!(result.as_vector().unwrap(), &vec![3.0, 6.0]);
    }

    #[test]
    fn test_eval_scalar_bignumber_invalid() {
        let ast = AstNode::BigNumber("xyz".to_string());
        // Use a context that forces eval_scalar path
        let ast = AstNode::BinaryOp(
            BinaryOp::Mul,
            Box::new(ast),
            Box::new(AstNode::List(vec![AstNode::Number(1.0)])),
        );
        let result = VectorDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_scalar_pow() {
        // eval_scalar_binary Pow: 2^3 * [1,2]
        let ast = AstNode::BinaryOp(
            BinaryOp::Mul,
            Box::new(parse("2^3").unwrap()),
            Box::new(AstNode::List(vec![
                AstNode::Number(1.0),
                AstNode::Number(2.0),
            ])),
        );
        let result = VectorDomain.evaluate(&ast, &EvalContext::new()).unwrap();
        assert_eq!(result.as_vector().unwrap(), &vec![8.0, 16.0]);
    }

    #[test]
    fn test_scalar_mod() {
        // eval_scalar_binary Mod: 10%3 * [1,2]
        let ast = AstNode::BinaryOp(
            BinaryOp::Mul,
            Box::new(AstNode::BinaryOp(
                BinaryOp::Mod,
                Box::new(AstNode::Number(10.0)),
                Box::new(AstNode::Number(3.0)),
            )),
            Box::new(AstNode::List(vec![
                AstNode::Number(1.0),
                AstNode::Number(2.0),
            ])),
        );
        let result = VectorDomain.evaluate(&ast, &EvalContext::new()).unwrap();
        assert_eq!(result.as_vector().unwrap(), &vec![1.0, 2.0]);
    }

    #[test]
    fn test_scalar_mod_by_zero() {
        // eval_scalar_binary Mod by zero
        let ast = AstNode::BinaryOp(
            BinaryOp::Mod,
            Box::new(AstNode::Number(10.0)),
            Box::new(AstNode::Number(0.0)),
        );
        let result = VectorDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::DivisionByZero));
    }

    #[test]
    fn test_scalar_div_zero() {
        // eval_scalar_binary Div by zero (a != 0)
        let ast = AstNode::BinaryOp(
            BinaryOp::Div,
            Box::new(AstNode::Number(5.0)),
            Box::new(AstNode::Number(0.0)),
        );
        let result = VectorDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::DivisionByZero));
    }

    #[test]
    fn test_scalar_pow_inf() {
        // eval_scalar_binary Pow producing inf → NaNOrInf
        let ast = AstNode::BinaryOp(
            BinaryOp::Pow,
            Box::new(AstNode::Number(f64::INFINITY)),
            Box::new(AstNode::Number(f64::INFINITY)),
        );
        let result = VectorDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::NaNOrInf));
    }

    #[test]
    fn test_scalar_div_inf() {
        // eval_scalar_binary Div producing inf → NaNOrInf
        let ast = AstNode::BinaryOp(
            BinaryOp::Div,
            Box::new(AstNode::Number(1.0)),
            Box::new(AstNode::Number(0.0)),
        );
        // 1.0 / 0.0 → DivisionByZero (a != 0, b == 0)
        let result = VectorDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::DivisionByZero));
    }

    #[test]
    fn test_unary_factorial_in_scalar() {
        // eval_scalar UnaryOp::Factorial error
        let ast = AstNode::BinaryOp(
            BinaryOp::Mul,
            Box::new(AstNode::UnaryOp(
                UnaryOp::Factorial,
                Box::new(AstNode::Number(5.0)),
            )),
            Box::new(AstNode::List(vec![AstNode::Number(1.0)])),
        );
        let result = VectorDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_eval_scalar_complex_rejected() {
        // eval_scalar wildcard `_ =>` with Complex
        let ast = AstNode::Complex(1.0, 2.0);
        let result = VectorDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_eval_scalar_matrix_rejected() {
        // eval_node Matrix rejection
        let ast = AstNode::Matrix(vec![vec![AstNode::Number(1.0)]]);
        let result = VectorDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_unsupported_vector_binary_div() {
        // List / List unsupported
        let ast = AstNode::BinaryOp(
            BinaryOp::Div,
            Box::new(AstNode::List(vec![
                AstNode::Number(1.0),
                AstNode::Number(2.0),
            ])),
            Box::new(AstNode::List(vec![
                AstNode::Number(3.0),
                AstNode::Number(4.0),
            ])),
        );
        let result = VectorDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_list_to_vector_non_list() {
        // list_to_vector with non-list argument via dot()
        let ast = AstNode::FunctionCall(
            "dot".to_string(),
            vec![
                AstNode::Number(1.0),
                AstNode::List(vec![AstNode::Number(1.0)]),
            ],
        );
        let result = VectorDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_list_to_vector_inf_element() {
        // list_to_vector with inf element → NaNOrInf
        let ast = AstNode::List(vec![AstNode::Number(f64::INFINITY)]);
        let result = VectorDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::NaNOrInf));
    }

    #[test]
    fn test_cross_wrong_args() {
        let ast = AstNode::FunctionCall("cross".to_string(), vec![AstNode::Number(1.0)]);
        let result = VectorDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_angle_wrong_args() {
        let ast = AstNode::FunctionCall("angle".to_string(), vec![AstNode::Number(1.0)]);
        let result = VectorDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_normalize_wrong_args() {
        let ast = AstNode::FunctionCall("normalize".to_string(), vec![]);
        let result = VectorDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_scalar_triple_wrong_args() {
        let ast = AstNode::FunctionCall("scalar_triple".to_string(), vec![AstNode::Number(1.0)]);
        let result = VectorDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_angle_dimension_mismatch() {
        let ast = parse("angle([1,2],[1,2,3])").unwrap();
        let result = VectorDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_contains_vector_function_recursive() {
        // contains_vector_function via BinaryOp nesting
        let ast = parse("dot([1,2],[3,4]) + 5").unwrap();
        assert!(VectorDomain.supports(&ast));
    }

    #[test]
    fn test_contains_vector_arithmetic_unary() {
        // contains_vector_arithmetic via UnaryOp
        let ast = AstNode::UnaryOp(UnaryOp::Neg, Box::new(parse("[1,2]+[3,4]").unwrap()));
        assert!(VectorDomain.supports(&ast));
    }

    #[test]
    fn test_contains_vector_arithmetic_function() {
        // contains_vector_arithmetic via FunctionCall arg
        let ast = parse("norm([1,2] + [3,4])").unwrap();
        assert!(VectorDomain.supports(&ast));
    }

    #[test]
    fn test_contains_vector_function_in_matrix() {
        // contains_vector_function via Matrix (returns false but exercises path)
        let ast = AstNode::Matrix(vec![vec![parse("dot([1,2],[3,4])").unwrap()]]);
        assert!(VectorDomain.supports(&ast));
    }

    #[test]
    fn test_contains_vector_arithmetic_in_list() {
        // contains_vector_arithmetic via List
        let ast = AstNode::List(vec![parse("[1,2]+[3,4]").unwrap()]);
        assert!(VectorDomain.supports(&ast));
    }

    #[test]
    fn test_not_supports_bignumber() {
        let ast = AstNode::BigNumber("42".to_string());
        assert!(!VectorDomain.supports(&ast));
    }

    #[test]
    fn test_not_supports_complex() {
        let ast = AstNode::Complex(1.0, 2.0);
        assert!(!VectorDomain.supports(&ast));
    }

    #[test]
    fn test_eval_scalar_number_direct() {
        // eval_scalar with plain Number (via non-list binary)
        let ast = AstNode::BinaryOp(
            BinaryOp::Add,
            Box::new(AstNode::Number(2.0)),
            Box::new(AstNode::Number(3.0)),
        );
        let result = VectorDomain.evaluate(&ast, &EvalContext::new()).unwrap();
        assert_eq!(result.as_scalar().unwrap(), 5.0);
    }

    #[test]
    fn test_eval_scalar_variable() {
        // eval_scalar Variable via non-list binary
        let ctx = EvalContext::new().with_var("x", 5.0);
        let ast = AstNode::BinaryOp(
            BinaryOp::Add,
            Box::new(AstNode::Number(2.0)),
            Box::new(AstNode::Variable("x".to_string())),
        );
        let result = VectorDomain.evaluate(&ast, &ctx).unwrap();
        assert_eq!(result.as_scalar().unwrap(), 7.0);
    }

    #[test]
    fn test_eval_scalar_unbound_variable() {
        // eval_scalar unbound variable via non-list binary
        let ast = AstNode::BinaryOp(
            BinaryOp::Add,
            Box::new(AstNode::Number(2.0)),
            Box::new(AstNode::Variable("z".to_string())),
        );
        let result = VectorDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Eval));
    }

    // ===== 覆盖 eval_node 裸 Number / UnaryOp::Neg 非列表 / Div 成功 =====

    #[test]
    fn test_eval_node_bare_number() {
        // eval_node 直接处理 Number → Scalar（line 64）
        let ast = AstNode::Number(42.0);
        let result = VectorDomain.evaluate(&ast, &EvalContext::new()).unwrap();
        assert_eq!(result.as_scalar().unwrap(), 42.0);
    }

    #[test]
    fn test_eval_node_unary_neg_non_list() {
        // UnaryOp::Neg 非列表操作数 → 标量取反（lines 100-101）
        let ast = AstNode::UnaryOp(UnaryOp::Neg, Box::new(AstNode::Number(5.0)));
        let result = VectorDomain.evaluate(&ast, &EvalContext::new()).unwrap();
        assert_eq!(result.as_scalar().unwrap(), -5.0);
    }

    #[test]
    fn test_eval_scalar_binary_div_success() {
        // eval_scalar_binary Div 成功路径 b != 0（lines 171-172）
        let ast = AstNode::BinaryOp(
            BinaryOp::Div,
            Box::new(AstNode::Number(10.0)),
            Box::new(AstNode::Number(2.0)),
        );
        let result = VectorDomain.evaluate(&ast, &EvalContext::new()).unwrap();
        assert_eq!(result.as_scalar().unwrap(), 5.0);
    }

    #[test]
    fn test_contains_vector_arithmetic_in_matrix() {
        // contains_vector_arithmetic Matrix 分支（line 431）
        let ast = AstNode::Matrix(vec![vec![parse("[1,2]+[3,4]").unwrap()]]);
        assert!(VectorDomain.supports(&ast));
    }

    // ===== proptest 属性测试 =====

    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig { cases: 128, ..ProptestConfig::default() })]

        /// 属性：dot(a,b) == dot(b,a)
        #[test]
        fn prop_dot_commutative(a in -100i64..100, b in -100i64..100, c in -100i64..100) {
            let ast_a = AstNode::FunctionCall(
                "dot".to_string(),
                vec![
                    AstNode::List(vec![AstNode::Number(a as f64), AstNode::Number(b as f64), AstNode::Number(c as f64)]),
                    AstNode::List(vec![AstNode::Number(c as f64), AstNode::Number(b as f64), AstNode::Number(a as f64)]),
                ],
            );
            let ast_b = AstNode::FunctionCall(
                "dot".to_string(),
                vec![
                    AstNode::List(vec![AstNode::Number(c as f64), AstNode::Number(b as f64), AstNode::Number(a as f64)]),
                    AstNode::List(vec![AstNode::Number(a as f64), AstNode::Number(b as f64), AstNode::Number(c as f64)]),
                ],
            );
            let domain = VectorDomain;
            let ctx = EvalContext::new();
            let ra = domain.evaluate(&ast_a, &ctx).unwrap().as_scalar().unwrap();
            let rb = domain.evaluate(&ast_b, &ctx).unwrap().as_scalar().unwrap();
            prop_assert!((ra - rb).abs() < 1e-9);
        }

        /// 属性：norm(v) >= 0
        #[test]
        fn prop_norm_non_negative(a in -100i64..100, b in -100i64..100) {
            let ast = AstNode::FunctionCall(
                "norm".to_string(),
                vec![AstNode::List(vec![AstNode::Number(a as f64), AstNode::Number(b as f64)])],
            );
            let domain = VectorDomain;
            let n = domain.evaluate(&ast, &EvalContext::new()).unwrap().as_scalar().unwrap();
            prop_assert!(n >= -1e-9, "norm {} should be non-negative", n);
        }

        /// 属性：dot(v,v) == norm(v)^2
        #[test]
        fn prop_dot_self_equals_norm_sq(a in -10i64..10, b in -10i64..10) {
            let list = AstNode::List(vec![AstNode::Number(a as f64), AstNode::Number(b as f64)]);
            let dot_ast = AstNode::FunctionCall(
                "dot".to_string(),
                vec![list.clone(), list.clone()],
            );
            let norm_ast = AstNode::FunctionCall("norm".to_string(), vec![list]);
            let domain = VectorDomain;
            let ctx = EvalContext::new();
            let d = domain.evaluate(&dot_ast, &ctx).unwrap().as_scalar().unwrap();
            let n = domain.evaluate(&norm_ast, &ctx).unwrap().as_scalar().unwrap();
            prop_assert!((d - n * n).abs() < 1e-9);
        }
    }
}
