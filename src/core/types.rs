//! CalNexus 共享类型：所有计算域、解析器、缓存、CLI 共用。
//!
//! 设计依据：
//! - proposal.md §Capabilities：v0.1 AstNode 只含 Number/Variable/BinaryOp/UnaryOp/FunctionCall
//! - design.md D1：三 crate 拆分，共享类型放 calnexus-core
//! - ADD.md §3.4 代码图：v0.2 完整 AstNode 含 BigInt/Matrix/Vector，v0.1 暂不实现

use std::collections::HashMap;
use std::fmt;
use std::time::Duration;

/// 表达式抽象语法树节点。
///
/// v0.1 支持 5 种节点；v0.5 扩展 Complex/Matrix/List（design.md D2）。
#[derive(Debug, Clone, PartialEq)]
pub enum AstNode {
    /// 数字字面量（浮点）。
    Number(f64),
    /// 大整数字面量：存储原始十进制字符串以保留精度（≥16 位整数）。
    BigNumber(String),
    /// 复数字面量：`(实部, 虚部)`，如 `3+4i` → `Complex(3.0, 4.0)`。
    Complex(f64, f64),
    /// 变量引用，如 `x`、`y`。
    Variable(String),
    /// 二元运算：`lhs op rhs`。
    BinaryOp(BinaryOp, Box<AstNode>, Box<AstNode>),
    /// 一元运算：`op expr`。
    UnaryOp(UnaryOp, Box<AstNode>),
    /// 函数调用：`name(args...)`。
    FunctionCall(String, Vec<AstNode>),
    /// 矩阵字面量：行列表，如 `[[1,2],[3,4]]`。
    Matrix(Vec<Vec<AstNode>>),
    /// 列表字面量：统计域用，如 `[1,2,3,4,5]`。
    List(Vec<AstNode>),
}

/// 二元运算符。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
    Mod,
}

/// 一元运算符。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnaryOp {
    /// 负号：`-x`
    Neg,
    /// 阶乘：`x!`
    Factorial,
    /// 绝对值：`abs(x)`（作为一元形式保留，也可通过 FunctionCall("abs", ...) 表达）
    Abs,
}

/// 求值结果。
///
/// v0.1 仅支持标量；v0.5 扩展 Complex（design.md D4）与 Matrix（design.md D5）。
/// 派生 Serialize/Deserialize 以支持 oxcache 缓存序列化（ADD ADR-001）。
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum EvalResult {
    /// 标量浮点结果。
    Scalar(f64),
    /// 复数结果（实部, 虚部）。
    Complex(f64, f64),
    /// 矩阵结果（行优先存储的二维向量）。
    Matrix(Vec<Vec<f64>>),
    /// 大整数结果（任意精度）。
    BigInt(num_bigint::BigInt),
    /// 精确分数结果。
    BigRational(num_rational::BigRational),
}

impl EvalResult {
    /// 获取标量值，若非 Scalar 返回 None。
    pub fn as_scalar(&self) -> Option<f64> {
        match self {
            EvalResult::Scalar(v) => Some(*v),
            EvalResult::Complex(_, _)
            | EvalResult::Matrix(_)
            | EvalResult::BigInt(_)
            | EvalResult::BigRational(_) => None,
        }
    }

    /// 获取复数值 (re, im)，若非 Complex 返回 None。
    pub fn as_complex(&self) -> Option<(f64, f64)> {
        match self {
            EvalResult::Complex(re, im) => Some((*re, *im)),
            EvalResult::Scalar(_)
            | EvalResult::Matrix(_)
            | EvalResult::BigInt(_)
            | EvalResult::BigRational(_) => None,
        }
    }

    /// 获取矩阵引用，若非 Matrix 返回 None。
    pub fn as_matrix(&self) -> Option<&Vec<Vec<f64>>> {
        match self {
            EvalResult::Matrix(m) => Some(m),
            EvalResult::Scalar(_)
            | EvalResult::Complex(_, _)
            | EvalResult::BigInt(_)
            | EvalResult::BigRational(_) => None,
        }
    }

    /// 获取大整数引用，若非 BigInt 返回 None。
    pub fn as_bigint(&self) -> Option<&num_bigint::BigInt> {
        match self {
            EvalResult::BigInt(b) => Some(b),
            _ => None,
        }
    }

    /// 获取分数引用，若非 BigRational 返回 None。
    pub fn as_bigrational(&self) -> Option<&num_rational::BigRational> {
        match self {
            EvalResult::BigRational(r) => Some(r),
            _ => None,
        }
    }
}

/// 计算错误。
///
/// 覆盖解析、求值、溢出、除零、定义域、深度、NaN/Inf 等所有错误路径。
/// design.md D7 要求错误必须显性化（Rule 12: Fail Loud）。
#[derive(Debug, Clone, PartialEq)]
pub enum CalcError {
    /// 表达式语法错误。
    ParseError(String),
    /// 求值过程中的通用错误。
    EvalError(String),
    /// 整数运算溢出（checked_* 检测到）。
    Overflow,
    /// 结果为 NaN 或 ±Inf。
    NaNOrInf,
    /// 函数定义域错误，如 `asin(2)`、`log(-1)`。
    DomainError(String),
    /// AST 深度超过限制（≤ 256）。
    DepthExceeded,
    /// 除零错误。
    DivisionByZero,
}

impl fmt::Display for CalcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CalcError::ParseError(msg) => write!(f, "parse error: {}", msg),
            CalcError::EvalError(msg) => write!(f, "evaluation error: {}", msg),
            CalcError::Overflow => write!(f, "integer overflow"),
            CalcError::NaNOrInf => write!(f, "result is NaN or infinity"),
            CalcError::DomainError(msg) => write!(f, "domain error: {}", msg),
            CalcError::DepthExceeded => write!(f, "AST depth exceeded limit"),
            CalcError::DivisionByZero => write!(f, "division by zero"),
        }
    }
}

impl std::error::Error for CalcError {}

/// AST 规范形式（S-表达式字符串）。
///
/// 用于 L1 缓存的键生成：等价表达式（如 `2+3` 与 `3+2`）规范化后生成相同的 `CanonicalForm`，
/// 再经 BLAKE3 哈希得到缓存键（design.md D5）。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CanonicalForm(pub String);

impl CanonicalForm {
    /// 从字符串创建规范形式。
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// 获取规范形式的字符串引用。
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CanonicalForm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// 求值上下文：变量绑定、精度、超时。
///
/// 传递给计算域的 `evaluate` 方法，提供求值所需的环境信息。
#[derive(Debug, Clone)]
pub struct EvalContext {
    /// 变量名到值的映射。
    pub vars: HashMap<String, f64>,
    /// 任意精度位数（v0.1 未实现 Precision 域，预留字段）。
    pub precision: Option<usize>,
    /// 计算超时时间。
    pub timeout: Duration,
}

impl Default for EvalContext {
    fn default() -> Self {
        Self {
            vars: HashMap::new(),
            precision: None,
            timeout: Duration::from_secs(5),
        }
    }
}

impl EvalContext {
    /// 创建空上下文（无变量、默认超时 5s）。
    pub fn new() -> Self {
        Self::default()
    }

    /// 插入变量绑定。
    pub fn with_var(mut self, name: impl Into<String>, value: f64) -> Self {
        self.vars.insert(name.into(), value);
        self
    }

    /// 查询变量值。
    pub fn get_var(&self, name: &str) -> Option<f64> {
        self.vars.get(name).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ast_node_number_construct_and_match() {
        let node = AstNode::Number(3.14);
        match node {
            AstNode::Number(v) => assert!((v - 3.14).abs() < f64::EPSILON),
            _ => panic!("expected Number variant"),
        }
    }

    #[test]
    fn ast_node_variable_construct_and_match() {
        let node = AstNode::Variable("x".to_string());
        assert_eq!(node, AstNode::Variable("x".to_string()));
    }

    #[test]
    fn ast_node_binary_op_construct() {
        let lhs = Box::new(AstNode::Number(2.0));
        let rhs = Box::new(AstNode::Number(3.0));
        let node = AstNode::BinaryOp(BinaryOp::Add, lhs, rhs);
        match node {
            AstNode::BinaryOp(BinaryOp::Add, l, r) => {
                assert_eq!(*l, AstNode::Number(2.0));
                assert_eq!(*r, AstNode::Number(3.0));
            }
            _ => panic!("expected BinaryOp variant"),
        }
    }

    #[test]
    fn ast_node_unary_op_construct() {
        let expr = Box::new(AstNode::Number(5.0));
        let node = AstNode::UnaryOp(UnaryOp::Neg, expr);
        match node {
            AstNode::UnaryOp(UnaryOp::Neg, inner) => assert_eq!(*inner, AstNode::Number(5.0)),
            _ => panic!("expected UnaryOp variant"),
        }
    }

    #[test]
    fn ast_node_function_call_construct() {
        let node = AstNode::FunctionCall("sin".to_string(), vec![AstNode::Variable("x".to_string())]);
        match node {
            AstNode::FunctionCall(name, args) => {
                assert_eq!(name, "sin");
                assert_eq!(args.len(), 1);
            }
            _ => panic!("expected FunctionCall variant"),
        }
    }

    #[test]
    fn ast_node_clone_and_eq() {
        let node = AstNode::BinaryOp(
            BinaryOp::Mul,
            Box::new(AstNode::Number(2.0)),
            Box::new(AstNode::Variable("y".to_string())),
        );
        let cloned = node.clone();
        assert_eq!(node, cloned);
    }

    #[test]
    fn binary_op_eq_and_hash() {
        assert_eq!(BinaryOp::Add, BinaryOp::Add);
        assert_ne!(BinaryOp::Add, BinaryOp::Sub);
        // 用于 HashMap 键
        let mut map = std::collections::HashMap::new();
        map.insert(BinaryOp::Mul, "mul");
        assert_eq!(map.get(&BinaryOp::Mul), Some(&"mul"));
    }

    #[test]
    fn eval_result_scalar_as_scalar() {
        let r = EvalResult::Scalar(42.0);
        assert_eq!(r.as_scalar(), Some(42.0));
    }

    #[test]
    fn calc_error_variants_display() {
        assert_eq!(
            CalcError::ParseError("unexpected token".into()).to_string(),
            "parse error: unexpected token"
        );
        assert_eq!(CalcError::Overflow.to_string(), "integer overflow");
        assert_eq!(CalcError::NaNOrInf.to_string(), "result is NaN or infinity");
        assert_eq!(
            CalcError::DomainError("asin(2)".into()).to_string(),
            "domain error: asin(2)"
        );
        assert_eq!(CalcError::DepthExceeded.to_string(), "AST depth exceeded limit");
        assert_eq!(CalcError::DivisionByZero.to_string(), "division by zero");
        assert_eq!(
            CalcError::EvalError("unknown".into()).to_string(),
            "evaluation error: unknown"
        );
    }

    #[test]
    fn calc_error_eq() {
        assert_eq!(CalcError::Overflow, CalcError::Overflow);
        assert_ne!(CalcError::Overflow, CalcError::DivisionByZero);
        assert_eq!(
            CalcError::ParseError("e1".into()),
            CalcError::ParseError("e1".into())
        );
    }

    #[test]
    fn canonical_form_construct_and_access() {
        let cf = CanonicalForm::new("(+ 2 3)");
        assert_eq!(cf.as_str(), "(+ 2 3)");
        assert_eq!(cf.to_string(), "(+ 2 3)");
    }

    #[test]
    fn canonical_form_eq_and_hash() {
        let cf1 = CanonicalForm::new("(+ 2 3)");
        let cf2 = CanonicalForm::new("(+ 2 3)");
        assert_eq!(cf1, cf2);
        let mut set = std::collections::HashSet::new();
        set.insert(cf1);
        assert!(set.contains(&cf2));
    }

    #[test]
    fn eval_context_default_and_with_var() {
        let ctx = EvalContext::default();
        assert!(ctx.vars.is_empty());
        assert_eq!(ctx.timeout, Duration::from_secs(5));

        let ctx = EvalContext::new().with_var("x", 1.5).with_var("y", 2.0);
        assert_eq!(ctx.get_var("x"), Some(1.5));
        assert_eq!(ctx.get_var("y"), Some(2.0));
        assert_eq!(ctx.get_var("z"), None);
    }

    #[test]
    fn ast_node_debug_format() {
        let node = AstNode::Number(1.0);
        let debug = format!("{:?}", node);
        assert!(debug.contains("Number"));
    }

    // ===== v0.5 新节点测试：Complex / Matrix / List =====

    #[test]
    fn ast_node_complex_construct_and_match() {
        let node = AstNode::Complex(3.0, 4.0);
        match node {
            AstNode::Complex(re, im) => {
                assert!((re - 3.0).abs() < f64::EPSILON);
                assert!((im - 4.0).abs() < f64::EPSILON);
            }
            _ => panic!("expected Complex variant"),
        }
    }

    #[test]
    fn ast_node_complex_clone_and_eq() {
        let node = AstNode::Complex(1.5, -2.5);
        let cloned = node.clone();
        assert_eq!(node, cloned);
    }

    #[test]
    fn ast_node_complex_debug_format() {
        let node = AstNode::Complex(3.0, 4.0);
        let debug = format!("{:?}", node);
        assert!(debug.contains("Complex"));
    }

    #[test]
    fn ast_node_matrix_construct_and_match() {
        let node = AstNode::Matrix(vec![
            vec![AstNode::Number(1.0), AstNode::Number(2.0)],
            vec![AstNode::Number(3.0), AstNode::Number(4.0)],
        ]);
        match node {
            AstNode::Matrix(rows) => {
                assert_eq!(rows.len(), 2);
                assert_eq!(rows[0].len(), 2);
                assert_eq!(rows[1].len(), 2);
                assert_eq!(rows[0][0], AstNode::Number(1.0));
                assert_eq!(rows[1][1], AstNode::Number(4.0));
            }
            _ => panic!("expected Matrix variant"),
        }
    }

    #[test]
    fn ast_node_matrix_clone_and_eq() {
        let node = AstNode::Matrix(vec![
            vec![AstNode::Number(1.0), AstNode::Number(2.0)],
            vec![AstNode::Number(3.0), AstNode::Number(4.0)],
        ]);
        let cloned = node.clone();
        assert_eq!(node, cloned);
    }

    #[test]
    fn ast_node_matrix_debug_format() {
        let node = AstNode::Matrix(vec![vec![AstNode::Number(1.0)]]);
        let debug = format!("{:?}", node);
        assert!(debug.contains("Matrix"));
    }

    #[test]
    fn ast_node_matrix_non_square() {
        // 2x3 非方阵
        let node = AstNode::Matrix(vec![
            vec![AstNode::Number(1.0), AstNode::Number(2.0), AstNode::Number(3.0)],
            vec![AstNode::Number(4.0), AstNode::Number(5.0), AstNode::Number(6.0)],
        ]);
        match node {
            AstNode::Matrix(rows) => {
                assert_eq!(rows.len(), 2);
                assert_eq!(rows[0].len(), 3);
            }
            _ => panic!("expected Matrix variant"),
        }
    }

    #[test]
    fn ast_node_list_construct_and_match() {
        let node = AstNode::List(vec![
            AstNode::Number(1.0),
            AstNode::Number(2.0),
            AstNode::Number(3.0),
            AstNode::Number(4.0),
            AstNode::Number(5.0),
        ]);
        match node {
            AstNode::List(elements) => {
                assert_eq!(elements.len(), 5);
                assert_eq!(elements[0], AstNode::Number(1.0));
                assert_eq!(elements[4], AstNode::Number(5.0));
            }
            _ => panic!("expected List variant"),
        }
    }

    #[test]
    fn ast_node_list_clone_and_eq() {
        let node = AstNode::List(vec![AstNode::Number(42.0)]);
        let cloned = node.clone();
        assert_eq!(node, cloned);
    }

    #[test]
    fn ast_node_list_debug_format() {
        let node = AstNode::List(vec![AstNode::Number(1.0), AstNode::Number(2.0)]);
        let debug = format!("{:?}", node);
        assert!(debug.contains("List"));
    }

    #[test]
    fn ast_node_list_single_element() {
        let node = AstNode::List(vec![AstNode::Number(42.0)]);
        match node {
            AstNode::List(elements) => assert_eq!(elements.len(), 1),
            _ => panic!("expected List variant"),
        }
    }

    #[test]
    fn ast_node_list_empty() {
        let node = AstNode::List(vec![]);
        match node {
            AstNode::List(elements) => assert!(elements.is_empty()),
            _ => panic!("expected List variant"),
        }
    }

    // ===== EvalResult helper methods 覆盖（v0.5 新增类型） =====

    #[test]
    fn eval_result_as_scalar_non_scalar_variants() {
        // 覆盖 as_scalar 中所有非 Scalar 分支
        assert_eq!(EvalResult::Complex(1.0, 2.0).as_scalar(), None);
        assert_eq!(EvalResult::Matrix(vec![vec![1.0]]).as_scalar(), None);
        assert_eq!(
            EvalResult::BigInt(num_bigint::BigInt::from(42)).as_scalar(),
            None
        );
        assert_eq!(
            EvalResult::BigRational(num_rational::BigRational::new(
                num_bigint::BigInt::from(1),
                num_bigint::BigInt::from(3)
            ))
            .as_scalar(),
            None
        );
    }

    #[test]
    fn eval_result_as_complex_with_complex() {
        // 覆盖 as_complex 中 Complex 分支
        let r = EvalResult::Complex(3.0, 4.0);
        assert_eq!(r.as_complex(), Some((3.0, 4.0)));
    }

    #[test]
    fn eval_result_as_complex_non_complex_variants() {
        // 覆盖 as_complex 中所有非 Complex 分支
        assert_eq!(EvalResult::Scalar(1.0).as_complex(), None);
        assert_eq!(EvalResult::Matrix(vec![vec![1.0]]).as_complex(), None);
        assert_eq!(
            EvalResult::BigInt(num_bigint::BigInt::from(42)).as_complex(),
            None
        );
        assert_eq!(
            EvalResult::BigRational(num_rational::BigRational::new(
                num_bigint::BigInt::from(1),
                num_bigint::BigInt::from(3)
            ))
            .as_complex(),
            None
        );
    }

    #[test]
    fn eval_result_as_matrix_with_matrix() {
        // 覆盖 as_matrix 中 Matrix 分支
        let r = EvalResult::Matrix(vec![vec![1.0, 2.0], vec![3.0, 4.0]]);
        let m = r.as_matrix().expect("expected Some for Matrix");
        assert_eq!(m.len(), 2);
        assert_eq!(m[0], vec![1.0, 2.0]);
        assert_eq!(m[1], vec![3.0, 4.0]);
    }

    #[test]
    fn eval_result_as_matrix_non_matrix_variants() {
        // 覆盖 as_matrix 中所有非 Matrix 分支
        assert_eq!(EvalResult::Scalar(1.0).as_matrix(), None);
        assert_eq!(EvalResult::Complex(1.0, 2.0).as_matrix(), None);
        assert_eq!(
            EvalResult::BigInt(num_bigint::BigInt::from(42)).as_matrix(),
            None
        );
        assert_eq!(
            EvalResult::BigRational(num_rational::BigRational::new(
                num_bigint::BigInt::from(1),
                num_bigint::BigInt::from(3)
            ))
            .as_matrix(),
            None
        );
    }

    #[test]
    fn eval_result_as_bigint_with_bigint() {
        // 覆盖 as_bigint 中 BigInt 分支
        let r = EvalResult::BigInt(num_bigint::BigInt::from(123));
        let b = r.as_bigint().expect("expected Some for BigInt");
        assert_eq!(b, &num_bigint::BigInt::from(123));
    }

    #[test]
    fn eval_result_as_bigint_non_bigint_variants() {
        // 覆盖 as_bigint 中 _ => None 分支
        assert_eq!(EvalResult::Scalar(1.0).as_bigint(), None);
        assert_eq!(EvalResult::Complex(1.0, 2.0).as_bigint(), None);
        assert_eq!(EvalResult::Matrix(vec![vec![1.0]]).as_bigint(), None);
        assert_eq!(
            EvalResult::BigRational(num_rational::BigRational::new(
                num_bigint::BigInt::from(1),
                num_bigint::BigInt::from(3)
            ))
            .as_bigint(),
            None
        );
    }

    #[test]
    fn eval_result_as_bigrational_with_bigrational() {
        // 覆盖 as_bigrational 中 BigRational 分支
        let r = EvalResult::BigRational(num_rational::BigRational::new(
            num_bigint::BigInt::from(1),
            num_bigint::BigInt::from(3),
        ));
        let rat = r.as_bigrational().expect("expected Some for BigRational");
        assert_eq!(
            rat,
            &num_rational::BigRational::new(
                num_bigint::BigInt::from(1),
                num_bigint::BigInt::from(3)
            )
        );
    }

    #[test]
    fn eval_result_as_bigrational_non_bigrational_variants() {
        // 覆盖 as_bigrational 中 _ => None 分支
        assert_eq!(EvalResult::Scalar(1.0).as_bigrational(), None);
        assert_eq!(EvalResult::Complex(1.0, 2.0).as_bigrational(), None);
        assert_eq!(EvalResult::Matrix(vec![vec![1.0]]).as_bigrational(), None);
        assert_eq!(
            EvalResult::BigInt(num_bigint::BigInt::from(42)).as_bigrational(),
            None
        );
    }

    #[test]
    fn eval_result_bigint_negative_value() {
        // 验证 BigInt 负数也能通过 as_bigint 获取
        let r = EvalResult::BigInt(num_bigint::BigInt::from(-999));
        assert_eq!(r.as_bigint(), Some(&num_bigint::BigInt::from(-999)));
    }

    #[test]
    fn eval_result_bigrational_integer_value() {
        // 验证 BigRational 整数值（分母为 1）也能通过 as_bigrational 获取
        let r = EvalResult::BigRational(num_rational::BigRational::from_integer(
            num_bigint::BigInt::from(42),
        ));
        assert_eq!(
            r.as_bigrational(),
            Some(&num_rational::BigRational::from_integer(num_bigint::BigInt::from(
                42
            )))
        );
    }
}
