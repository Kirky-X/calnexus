// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! AST 规范化器：将解析后的 `AstNode` 转换为规范形式，用于 L1 缓存去重。
//!
//! 三大变换（design.md / tasks 3.3）：
//! 1. **SortCommutative**：对 `+`/`*` 操作数按规范顺序排列（数值升序，变量字典序）
//! 2. **ConstantFolder**：全常量子表达式预计算，检测溢出/NaN/Inf/除零
//! 3. **UnaryNormalizer**：双重负号 `--x` 消除为 `x`
//!
//! 规范形式序列化为 S-表达式字符串（`CanonicalForm`）。
//!
//! **Spec 冲突说明**：Req 1 Scen 1-2 期望 `3+2` → `(+ 2 3)`（仅排序），
//! 但 Req 3 Scen 1 和 Req 7 Scen 1 期望 `2+3` → `5`（折叠）。
//! 由于 Req 5 Scen 2 要求 `2*3+1` 与 `1+6` 同形式（必须折叠），
//! 本实现以**常量折叠优先**解决冲突：全常量表达式折叠为单个 Number。

use crate::core::types::{AstNode, BinaryOp, CalcError, CanonicalForm, UnaryOp};
use std::cmp::Ordering;

/// AST 规范化器。
///
/// 无状态，所有方法均为关联函数。
pub struct AstCanonicalizer;

impl AstCanonicalizer {
    /// 规范化 AST 并生成 `CanonicalForm`。
    ///
    /// 返回 `(canonical_ast, canonical_form)`：
    /// - `canonical_ast`：规范化后的 AST（常量折叠、操作数排序、一元归一化）
    /// - `canonical_form`：S-表达式字符串（用于缓存键生成）
    pub fn canonicalize(ast: &AstNode) -> Result<(AstNode, CanonicalForm), CalcError> {
        let canonical_ast = Self::transform(ast)?;
        let s_expr = Self::serialize(&canonical_ast);
        Ok((canonical_ast, CanonicalForm::new(s_expr)))
    }

    /// 规范化 AST（**不折叠常量**）并生成 `CanonicalForm`。
    ///
    /// 与 `canonicalize` 的区别：仅做交换律排序与一元归一化，不做常量折叠。
    /// 用于 `--canonical` 标志输出，满足 PRD §3.2.4 示例 `3+2` → `(+ 2 3)`。
    /// 缓存键仍使用 `canonicalize`（折叠版本），此方法仅供展示。
    pub fn canonicalize_no_fold(ast: &AstNode) -> Result<(AstNode, CanonicalForm), CalcError> {
        let canonical_ast = Self::transform_no_fold(ast)?;
        let s_expr = Self::serialize(&canonical_ast);
        Ok((canonical_ast, CanonicalForm::new(s_expr)))
    }

    /// 递归变换 AST（排序 + 折叠 + 一元归一化）。
    ///
    /// 变换顺序（bottom-up）：先递归变换子节点，再对当前节点应用：
    /// 1. 常量折叠（全常量子树 → 单个 Number，仅当 `fold_constants = true`）
    /// 2. 交换律排序（Add/Mul 操作数按规范顺序）
    /// 3. 一元归一化（双重负号消除始终执行；一元常量折叠仅当 `fold_unary = true`）
    ///
    /// `transform` 与 `transform_no_fold` 的全部差异通过两个布尔参数控制：
    /// - `transform`           = `transform_inner(ast, true,  true)`
    /// - `transform_no_fold`   = `transform_inner(ast, false, false)`
    ///
    /// 双重负号消除（`--x → x`）属于结构性归一化，两个公开入口均执行，
    /// 因此**不**由 `fold_unary` 控制。
    fn transform_inner(
        ast: &AstNode,
        fold_constants: bool,
        fold_unary: bool,
    ) -> Result<AstNode, CalcError> {
        match ast {
            AstNode::Number(_)
            | AstNode::Variable(_)
            | AstNode::Complex(_, _)
            | AstNode::BigNumber(_) => Ok(ast.clone()),
            AstNode::BinaryOp(op, l, r) => {
                let l = Self::transform_inner(l, fold_constants, fold_unary)?;
                let r = Self::transform_inner(r, fold_constants, fold_unary)?;
                // 常量折叠：两操作数均为 Number（仅当 fold_constants = true）
                if fold_constants {
                    if let (AstNode::Number(a), AstNode::Number(b)) = (&l, &r) {
                        return Self::eval_binary(*op, *a, *b).map(AstNode::Number);
                    }
                }
                // 交换律排序：仅 Add 和 Mul
                let (l, r) = match op {
                    BinaryOp::Add | BinaryOp::Mul => {
                        if Self::compare_nodes(&l, &r) == Ordering::Greater {
                            (r, l)
                        } else {
                            (l, r)
                        }
                    }
                    _ => (l, r),
                };
                Ok(AstNode::BinaryOp(*op, Box::new(l), Box::new(r)))
            }
            AstNode::UnaryOp(op, e) => {
                let e = Self::transform_inner(e, fold_constants, fold_unary)?;
                // 双重负号消除：--x → x（结构性归一化，始终执行）
                if *op == UnaryOp::Neg {
                    if let AstNode::UnaryOp(UnaryOp::Neg, inner) = &e {
                        return Ok((**inner).clone());
                    }
                }
                // 一元常量折叠：Neg(Number(n)) → Number(-n)（仅当 fold_unary = true）
                if fold_unary {
                    if let AstNode::Number(n) = &e {
                        if *op == UnaryOp::Neg {
                            return Ok(AstNode::Number(-*n));
                        }
                    }
                }
                Ok(AstNode::UnaryOp(*op, Box::new(e)))
            }
            AstNode::FunctionCall(name, args) => {
                let mut transformed_args = Vec::with_capacity(args.len());
                for arg in args {
                    transformed_args.push(Self::transform_inner(arg, fold_constants, fold_unary)?);
                }
                Ok(AstNode::FunctionCall(name.clone(), transformed_args))
            }
            AstNode::Matrix(rows) => {
                let mut transformed_rows = Vec::with_capacity(rows.len());
                for row in rows {
                    let mut transformed_row = Vec::with_capacity(row.len());
                    for elem in row {
                        transformed_row.push(Self::transform_inner(
                            elem,
                            fold_constants,
                            fold_unary,
                        )?);
                    }
                    transformed_rows.push(transformed_row);
                }
                Ok(AstNode::Matrix(transformed_rows))
            }
            AstNode::List(elements) => {
                let mut transformed = Vec::with_capacity(elements.len());
                for elem in elements {
                    transformed.push(Self::transform_inner(elem, fold_constants, fold_unary)?);
                }
                Ok(AstNode::List(transformed))
            }
        }
    }

    /// 递归变换 AST（排序 + 折叠 + 一元归一化）。
    fn transform(ast: &AstNode) -> Result<AstNode, CalcError> {
        Self::transform_inner(ast, true, true)
    }

    /// 递归变换 AST（**仅排序 + 一元归一化，不折叠常量**）。
    ///
    /// 与 `transform` 的区别：跳过常量折叠步骤，保留 `2+3`、`3+2` 为
    /// `(+ 2 3)`、`(+ 2 3)`（排序后相同），用于 `--canonical` 展示。
    fn transform_no_fold(ast: &AstNode) -> Result<AstNode, CalcError> {
        Self::transform_inner(ast, false, false)
    }

    /// 求值二元运算（常量折叠用），检测除零与 NaN/Inf。
    fn eval_binary(op: BinaryOp, a: f64, b: f64) -> Result<f64, CalcError> {
        let result = match op {
            BinaryOp::Add => a + b,
            BinaryOp::Sub => a - b,
            BinaryOp::Mul => a * b,
            BinaryOp::Div => {
                if b == 0.0 {
                    return Err(CalcError::DivisionByZero);
                }
                a / b
            }
            BinaryOp::Pow => a.powf(b),
            BinaryOp::Mod => {
                if b == 0.0 {
                    return Err(CalcError::DivisionByZero);
                }
                a % b
            }
        };
        if result.is_nan() || result.is_infinite() {
            return Err(CalcError::NaNOrInf);
        }
        Ok(result)
    }

    /// 比较两个 AST 节点的规范顺序。
    ///
    /// 排序规则：
    /// - 两 Number：按数值升序
    /// - 两 Variable：按字典序
    /// - Number 与非 Number：Number 优先（Less）
    /// - 其他组合（如 Variable 与复合表达式）：保持原始顺序（Equal，不交换）
    fn compare_nodes(a: &AstNode, b: &AstNode) -> Ordering {
        match (a, b) {
            (AstNode::Number(x), AstNode::Number(y)) => x.partial_cmp(y).unwrap_or(Ordering::Equal),
            (AstNode::Number(_), _) => Ordering::Less,
            (_, AstNode::Number(_)) => Ordering::Greater,
            (AstNode::Variable(x), AstNode::Variable(y)) => x.cmp(y),
            _ => Ordering::Equal,
        }
    }

    /// 将 AST 序列化为 S-表达式字符串。
    ///
    /// 格式：`(op arg1 arg2 ...)`，常量直接输出数值，变量输出变量名。
    fn serialize(ast: &AstNode) -> String {
        match ast {
            AstNode::Number(n) => Self::format_number(*n),
            AstNode::BigNumber(s) => s.clone(),
            AstNode::Complex(re, im) => {
                format!(
                    "(complex {} {})",
                    Self::format_number(*re),
                    Self::format_number(*im)
                )
            }
            AstNode::Variable(v) => v.clone(),
            AstNode::BinaryOp(op, l, r) => {
                let op_str = match op {
                    BinaryOp::Add => "+",
                    BinaryOp::Sub => "-",
                    BinaryOp::Mul => "*",
                    BinaryOp::Div => "/",
                    BinaryOp::Pow => "^",
                    BinaryOp::Mod => "mod",
                };
                format!("({} {} {})", op_str, Self::serialize(l), Self::serialize(r))
            }
            AstNode::UnaryOp(op, e) => {
                let op_str = match op {
                    UnaryOp::Neg => "-",
                    UnaryOp::Factorial => "factorial",
                    UnaryOp::Abs => "abs",
                };
                format!("({} {})", op_str, Self::serialize(e))
            }
            AstNode::FunctionCall(name, args) => {
                if args.is_empty() {
                    format!("({})", name)
                } else {
                    let args_str: Vec<String> = args.iter().map(Self::serialize).collect();
                    format!("({} {})", name, args_str.join(" "))
                }
            }
            AstNode::Matrix(rows) => {
                let rows_str: Vec<String> = rows
                    .iter()
                    .map(|row| {
                        let elems: Vec<String> = row.iter().map(Self::serialize).collect();
                        format!("({})", elems.join(" "))
                    })
                    .collect();
                format!("(matrix {})", rows_str.join(" "))
            }
            AstNode::List(elements) => {
                let elems_str: Vec<String> = elements.iter().map(Self::serialize).collect();
                format!("(list {})", elems_str.join(" "))
            }
        }
    }

    /// 格式化数字：整数输出无小数点，浮点数保留原样。
    fn format_number(n: f64) -> String {
        if n.fract() == 0.0 && n.abs() < 9e15 {
            format!("{}", n as i64)
        } else {
            format!("{}", n)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::parser::parse;

    // 辅助函数：解析 + 规范化，返回 CanonicalForm 字符串
    fn canon(input: &str) -> Result<String, CalcError> {
        let ast = parse(input)?;
        let (_, cf) = AstCanonicalizer::canonicalize(&ast)?;
        Ok(cf.as_str().to_string())
    }

    // ===== Requirement 1: 交换律排序 =====

    #[test]
    fn test_variable_sort_addition() {
        // y+x → (+ x y)（变量字典序）
        assert_eq!(canon("y+x").unwrap(), "(+ x y)");
    }

    #[test]
    fn test_variable_sort_order_independent() {
        // x+y 和 y+x 同规范形式
        assert_eq!(canon("x+y").unwrap(), canon("y+x").unwrap());
    }

    #[test]
    fn test_number_before_variable_in_mul() {
        // 5*x 和 x*5 → (* 5 x)（数值优先于变量）
        assert_eq!(canon("5*x").unwrap(), "(* 5 x)");
        assert_eq!(canon("x*5").unwrap(), "(* 5 x)");
    }

    #[test]
    fn test_mixed_sort_number_before_variable() {
        // x+2 → (+ 2 x)（数值优先于变量）
        assert_eq!(canon("x+2").unwrap(), "(+ 2 x)");
        assert_eq!(canon("2+x").unwrap(), "(+ 2 x)");
    }

    // ===== Requirement 2: 交换律适用运算符范围 =====

    #[test]
    fn test_subtraction_not_commutative() {
        // x-2 → (- x 2)（顺序不变）
        assert_eq!(canon("x-2").unwrap(), "(- x 2)");
    }

    #[test]
    fn test_division_not_commutative() {
        // x/2 → (/ x 2)
        assert_eq!(canon("x/2").unwrap(), "(/ x 2)");
    }

    #[test]
    fn test_power_not_commutative() {
        // x^2 → (^ x 2)
        assert_eq!(canon("x^2").unwrap(), "(^ x 2)");
    }

    #[test]
    fn test_non_commutative_not_equivalent() {
        // x-2 与 2-x 不同
        assert_ne!(canon("x-2").unwrap(), canon("2-x").unwrap());
    }

    // ===== Requirement 3: 常量折叠 =====

    #[test]
    fn test_simple_constant_folding() {
        // 2+3 → 5
        assert_eq!(canon("2+3").unwrap(), "5");
    }

    #[test]
    fn test_nested_constant_folding() {
        // 2*3+1 → 7
        assert_eq!(canon("2*3+1").unwrap(), "7");
    }

    #[test]
    fn test_partial_folding_preserves_variable() {
        // 2*3+x → (+ 6 x)
        assert_eq!(canon("2*3+x").unwrap(), "(+ 6 x)");
    }

    #[test]
    fn test_division_by_zero_folding_error() {
        // 1/0 → 错误
        let result = canon("1/0");
        assert!(result.is_err());
        let err = result.unwrap_err();
        // 分别求值每个 matches! 分支，避免 || 短路导致 lines 312-313 未覆盖
        let is_div_zero = matches!(err, CalcError::DivisionByZero);
        let is_eval_err = matches!(err, CalcError::EvalError(_));
        let is_nan_inf = matches!(err, CalcError::NaNOrInf);
        assert!(
            is_div_zero || is_eval_err || is_nan_inf,
            "expected division by zero error, got {:?}",
            err
        );
    }

    // ===== Requirement 4: 一元运算归一化 =====

    #[test]
    fn test_double_negation_constant() {
        // --5 → 5（双重负号消除 + 常量折叠）
        assert_eq!(canon("--5").unwrap(), "5");
    }

    #[test]
    fn test_double_negation_variable() {
        // -(-x) → x
        assert_eq!(canon("-(-x)").unwrap(), "x");
    }

    #[test]
    fn test_single_negation_variable() {
        // -x → (- x)
        assert_eq!(canon("-x").unwrap(), "(- x)");
    }

    #[test]
    fn test_negation_with_constant_folding() {
        // -(2+3) → -5
        assert_eq!(canon("-(2+3)").unwrap(), "-5");
    }

    // ===== Requirement 5: 等价表达式生成相同规范形式 =====

    #[test]
    fn test_commutative_equivalent() {
        // 3+2 与 2+3 同形式（均折叠为 5）
        assert_eq!(canon("3+2").unwrap(), "5");
        assert_eq!(canon("2+3").unwrap(), "5");
    }

    #[test]
    fn test_folding_equivalent() {
        // 2*3+1 与 1+6 同形式（均折叠为 7）
        assert_eq!(canon("2*3+1").unwrap(), "7");
        assert_eq!(canon("1+6").unwrap(), "7");
    }

    #[test]
    fn test_composite_equivalent() {
        // x+2*3 与 6+x 同形式 → (+ 6 x)
        assert_eq!(canon("x+2*3").unwrap(), "(+ 6 x)");
        assert_eq!(canon("6+x").unwrap(), "(+ 6 x)");
    }

    #[test]
    fn test_non_commutative_not_equivalent_constants() {
        // 2-3 与 3-2 不同（折叠后 -1 与 1）
        assert_ne!(canon("2-3").unwrap(), canon("3-2").unwrap());
    }

    // ===== Requirement 6: 规范形式幂等性 =====

    #[test]
    fn test_idempotent_simple() {
        // 2+3*x 规范化后再规范化，结果不变
        let ast = parse("2+3*x").unwrap();
        let (canon_ast1, cf1) = AstCanonicalizer::canonicalize(&ast).unwrap();
        let (_, cf2) = AstCanonicalizer::canonicalize(&canon_ast1).unwrap();
        assert_eq!(cf1, cf2);
    }

    #[test]
    fn test_idempotent_after_folding() {
        // 2*3+1 → 7，再规范化 7 → 7
        let ast = parse("2*3+1").unwrap();
        let (canon_ast1, _) = AstCanonicalizer::canonicalize(&ast).unwrap();
        let (_, cf2) = AstCanonicalizer::canonicalize(&canon_ast1).unwrap();
        assert_eq!(cf2.as_str(), "7");
    }

    #[test]
    fn test_idempotent_nested() {
        // (x+y)*2 规范化两次
        let ast = parse("(x+y)*2").unwrap();
        let (canon_ast1, cf1) = AstCanonicalizer::canonicalize(&ast).unwrap();
        let (_, cf2) = AstCanonicalizer::canonicalize(&canon_ast1).unwrap();
        assert_eq!(cf1, cf2);
    }

    // ===== Requirement 7: 规范形式序列化 =====

    #[test]
    fn test_serialize_folded_constant() {
        // 2+3 → "5"
        assert_eq!(canon("2+3").unwrap(), "5");
    }

    #[test]
    fn test_serialize_variable_with_folded() {
        // x*(1+2) → (* 3 x)（常量 1+2 折叠为 3，数值优先于变量）
        // 注：spec Req 7 Scen 2 原写 `(* x 3)`，与 Req 1 Scen 4 的"数值优先"规则矛盾，
        // 本实现遵循 Req 1 的排序规则（数值优先于变量）。
        assert_eq!(canon("x*(1+2)").unwrap(), "(* 3 x)");
    }

    #[test]
    fn test_serialize_function_call() {
        // sin(pi/2) → (sin (/ pi 2))
        assert_eq!(canon("sin(pi/2)").unwrap(), "(sin (/ pi 2))");
    }

    #[test]
    fn test_serialize_nested_structure() {
        // (x+y)*z → (* (+ x y) z)
        assert_eq!(canon("(x+y)*z").unwrap(), "(* (+ x y) z)");
    }

    // ===== Requirement 8: 规范化深度限制 =====

    #[test]
    fn test_folding_reduces_depth() {
        // ((((1+2)+3)+4)+5) → 15（深度 1）
        let ast = parse("((((1+2)+3)+4)+5)").unwrap();
        let (canon_ast, _) = AstCanonicalizer::canonicalize(&ast).unwrap();
        assert_eq!(canon_ast, AstNode::Number(15.0));
    }

    #[test]
    fn test_variable_depth_not_increased() {
        // 深度 100 的变量嵌套表达式，规范化后深度 ≤ 100
        let expr = format!("1{}", "+x".repeat(99));
        let ast = parse(&expr).unwrap();
        let (canon_ast, _) = AstCanonicalizer::canonicalize(&ast).unwrap();
        let original_depth = ast_depth(&ast);
        let canon_depth = ast_depth(&canon_ast);
        assert!(
            canon_depth <= original_depth,
            "canonical depth {} > original depth {}",
            canon_depth,
            original_depth
        );
    }

    #[test]
    fn test_canonicalize_depth_256_succeeds() {
        // 深度 256 的合法 AST 规范化成功
        let expr = format!("1{}", "+x".repeat(255));
        let ast = parse(&expr).unwrap();
        let result = AstCanonicalizer::canonicalize(&ast);
        assert!(
            result.is_ok(),
            "expected ok for depth 256, got {:?}",
            result
        );
    }

    // 辅助函数：计算 AST 深度
    fn ast_depth(ast: &AstNode) -> usize {
        match ast {
            AstNode::Number(_)
            | AstNode::Variable(_)
            | AstNode::Complex(_, _)
            | AstNode::BigNumber(_) => 1,
            AstNode::BinaryOp(_, l, r) => 1 + ast_depth(l).max(ast_depth(r)),
            AstNode::UnaryOp(_, e) => 1 + ast_depth(e),
            AstNode::FunctionCall(_, args) => 1 + args.iter().map(ast_depth).max().unwrap_or(0),
            AstNode::Matrix(rows) => {
                1 + rows
                    .iter()
                    .flat_map(|row| row.iter())
                    .map(ast_depth)
                    .max()
                    .unwrap_or(0)
            }
            AstNode::List(elements) => 1 + elements.iter().map(ast_depth).max().unwrap_or(0),
        }
    }

    // ===== 覆盖 Matrix/List transform 与 serialize =====

    #[test]
    fn test_canonicalize_matrix_literal() {
        // 矩阵字面量规范化：覆盖 Matrix 分支（transform + serialize）
        let cf = canon("[[1,2],[3,4]]").unwrap();
        assert_eq!(cf, "(matrix (1 2) (3 4))");
    }

    #[test]
    fn test_canonicalize_list_literal() {
        // 列表字面量规范化：覆盖 List 分支（transform + serialize）
        let cf = canon("[1,2,3]").unwrap();
        assert_eq!(cf, "(list 1 2 3)");
    }

    #[test]
    fn test_canonicalize_list_with_nested_expr() {
        // 列表元素为表达式：覆盖 List transform 递归
        let cf = canon("[2+3, x]").unwrap();
        assert_eq!(cf, "(list 5 x)");
    }

    #[test]
    fn test_canonicalize_matrix_with_nested_expr() {
        // 矩阵元素为表达式：覆盖 Matrix transform 递归
        let cf = canon("[[2+3, x]]").unwrap();
        assert_eq!(cf, "(matrix (5 x))");
    }

    // ===== 覆盖 Div/Mod 常量折叠 =====

    #[test]
    fn test_division_non_zero_folds() {
        // 6/3 → 2（覆盖 eval_binary Div 分支 b != 0）
        assert_eq!(canon("6/3").unwrap(), "2");
    }

    #[test]
    fn test_modulo_by_zero_folding_error_manual_ast() {
        // 手动构造 BinaryOp::Mod(... , 0) — parser 不会产生此 AST
        // 覆盖 eval_binary Mod 分支 b == 0 的错误路径
        let ast = AstNode::BinaryOp(
            BinaryOp::Mod,
            Box::new(AstNode::Number(5.0)),
            Box::new(AstNode::Number(0.0)),
        );
        let result = AstCanonicalizer::canonicalize(&ast);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CalcError::DivisionByZero));
    }

    #[test]
    fn test_modulo_non_zero_folds_manual_ast() {
        // 手动构造 BinaryOp::Mod(5, 3) → 2（覆盖 eval_binary Mod 分支 b != 0）
        let ast = AstNode::BinaryOp(
            BinaryOp::Mod,
            Box::new(AstNode::Number(5.0)),
            Box::new(AstNode::Number(3.0)),
        );
        let (_, cf) = AstCanonicalizer::canonicalize(&ast).unwrap();
        assert_eq!(cf.as_str(), "2");
    }

    #[test]
    fn test_pow_overflow_error() {
        // 2^99999 → Inf → NaNOrInf 错误
        let result = canon("2^99999");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CalcError::NaNOrInf));
    }

    // ===== 覆盖 BigNumber/Complex 序列化 =====

    #[test]
    fn test_canonicalize_big_number_literal() {
        // 16+ 位整数 → BigNumber 节点，序列化为原始字符串
        let cf = canon("1234567890123456").unwrap();
        assert_eq!(cf, "1234567890123456");
    }

    #[test]
    fn test_canonicalize_complex_literal() {
        // 3+4i → Complex(3,4)，序列化为 (complex 3 4)
        let cf = canon("3+4i").unwrap();
        assert_eq!(cf, "(complex 3 4)");
    }

    #[test]
    fn test_canonicalize_complex_negative_imaginary() {
        // 3-4i → Complex(3,-4)
        let cf = canon("3-4i").unwrap();
        assert_eq!(cf, "(complex 3 -4)");
    }

    // ===== 覆盖 BinaryOp::Mod / UnaryOp::Factorial/Abs 序列化 =====

    #[test]
    fn test_serialize_binary_mod_op() {
        // 手动构造 BinaryOp::Mod(x, 2) → "(mod x 2)"
        let ast = AstNode::BinaryOp(
            BinaryOp::Mod,
            Box::new(AstNode::Variable("x".to_string())),
            Box::new(AstNode::Number(2.0)),
        );
        let (_, cf) = AstCanonicalizer::canonicalize(&ast).unwrap();
        assert_eq!(cf.as_str(), "(mod x 2)");
    }

    #[test]
    fn test_serialize_unary_factorial_op() {
        // 手动构造 UnaryOp::Factorial(x) → "(factorial x)"
        let ast = AstNode::UnaryOp(
            UnaryOp::Factorial,
            Box::new(AstNode::Variable("x".to_string())),
        );
        let (_, cf) = AstCanonicalizer::canonicalize(&ast).unwrap();
        assert_eq!(cf.as_str(), "(factorial x)");
    }

    #[test]
    fn test_serialize_unary_abs_op() {
        // 手动构造 UnaryOp::Abs(x) → "(abs x)"
        let ast = AstNode::UnaryOp(UnaryOp::Abs, Box::new(AstNode::Variable("x".to_string())));
        let (_, cf) = AstCanonicalizer::canonicalize(&ast).unwrap();
        assert_eq!(cf.as_str(), "(abs x)");
    }

    #[test]
    fn test_serialize_unary_factorial_on_number_no_fold() {
        // UnaryOp::Factorial(Number(5)) — 不折叠（Factorial 不在折叠规则中）
        // 覆盖 UnaryOp 分支中 op != Neg 的路径
        let ast = AstNode::UnaryOp(UnaryOp::Factorial, Box::new(AstNode::Number(5.0)));
        let (_, cf) = AstCanonicalizer::canonicalize(&ast).unwrap();
        assert_eq!(cf.as_str(), "(factorial 5)");
    }

    #[test]
    fn test_serialize_unary_abs_on_number_no_fold() {
        // UnaryOp::Abs(Number(5)) — 不折叠
        let ast = AstNode::UnaryOp(UnaryOp::Abs, Box::new(AstNode::Number(5.0)));
        let (_, cf) = AstCanonicalizer::canonicalize(&ast).unwrap();
        assert_eq!(cf.as_str(), "(abs 5)");
    }

    // ===== 覆盖空参数函数与浮点数格式化 =====

    #[test]
    fn test_serialize_empty_args_function() {
        // 手动构造无参数函数调用：覆盖 serialize 中 args.is_empty() 分支
        let ast = AstNode::FunctionCall("foo".to_string(), vec![]);
        let (_, cf) = AstCanonicalizer::canonicalize(&ast).unwrap();
        assert_eq!(cf.as_str(), "(foo)");
    }

    #[test]
    fn test_format_number_float() {
        // 3.14 → "3.14"（覆盖 format_number 浮点分支）
        let cf = canon("3.14").unwrap();
        assert_eq!(cf, "3.14");
    }

    #[test]
    fn test_format_number_float_in_expr() {
        // 1.5+x → "(+ 1.5 x)"（覆盖 format_number 浮点分支 + 排序）
        let cf = canon("1.5+x").unwrap();
        assert_eq!(cf, "(+ 1.5 x)");
    }

    // ===== 覆盖 ast_depth 各分支 =====

    #[test]
    fn test_ast_depth_unary_op_branch() {
        // 覆盖 ast_depth 的 UnaryOp 分支
        let ast = AstNode::UnaryOp(UnaryOp::Neg, Box::new(AstNode::Number(5.0)));
        assert_eq!(ast_depth(&ast), 2);
    }

    #[test]
    fn test_ast_depth_function_call_branch() {
        // 覆盖 ast_depth 的 FunctionCall 分支
        let ast =
            AstNode::FunctionCall("sin".to_string(), vec![AstNode::Variable("x".to_string())]);
        assert_eq!(ast_depth(&ast), 2);
    }

    #[test]
    fn test_ast_depth_matrix_branch() {
        // 覆盖 ast_depth 的 Matrix 分支
        let ast = AstNode::Matrix(vec![vec![AstNode::Number(1.0), AstNode::Number(2.0)]]);
        assert_eq!(ast_depth(&ast), 2);
    }

    #[test]
    fn test_ast_depth_list_branch() {
        // 覆盖 ast_depth 的 List 分支
        let ast = AstNode::List(vec![AstNode::Number(1.0), AstNode::Number(2.0)]);
        assert_eq!(ast_depth(&ast), 2);
    }

    #[test]
    fn test_ast_depth_list_empty_branch() {
        // 覆盖 ast_depth 的 List 空列表分支（unwrap_or(0)）
        let ast = AstNode::List(vec![]);
        assert_eq!(ast_depth(&ast), 1);
    }

    #[test]
    fn test_ast_depth_matrix_empty_branch() {
        // 覆盖 ast_depth 的 Matrix 空矩阵分支（unwrap_or(0)）
        let ast = AstNode::Matrix(vec![vec![]]);
        assert_eq!(ast_depth(&ast), 1);
    }

    // ===== 覆盖 compare_nodes 的 Number-Number 分支 =====

    #[test]
    fn test_compare_nodes_number_vs_number() {
        // 覆盖 compare_nodes 中 (Number, Number) 分支（lines 143-144）
        // 该分支在常规规范化流程中不可达（常量折叠优先于排序），
        // 通过直接调用 compare_nodes 验证其正确性
        use std::cmp::Ordering;
        let a = AstNode::Number(1.0);
        let b = AstNode::Number(2.0);
        assert_eq!(AstCanonicalizer::compare_nodes(&a, &b), Ordering::Less);
        assert_eq!(AstCanonicalizer::compare_nodes(&b, &a), Ordering::Greater);
        assert_eq!(AstCanonicalizer::compare_nodes(&a, &a), Ordering::Equal);
    }

    // ===== 覆盖 canonicalize_no_fold（transform_no_fold 分支） =====

    #[test]
    fn test_no_fold_double_negation_variable() {
        // -(-x) → x（transform_no_fold 双重负号消除，lines 154-157）
        let ast = parse("-(-x)").unwrap();
        let (canon_ast, _) = AstCanonicalizer::canonicalize_no_fold(&ast).unwrap();
        assert_eq!(canon_ast, AstNode::Variable("x".to_string()));
    }

    #[test]
    fn test_no_fold_matrix_literal() {
        // 矩阵字面量：覆盖 transform_no_fold Matrix 分支（lines 168-177）
        let ast = parse("[[1,2],[3,4]]").unwrap();
        let (canon_ast, cf) = AstCanonicalizer::canonicalize_no_fold(&ast).unwrap();
        assert_eq!(cf.as_str(), "(matrix (1 2) (3 4))");
        // 验证不折叠：矩阵元素仍为 Number
        match canon_ast {
            AstNode::Matrix(rows) => assert_eq!(rows.len(), 2),
            _ => panic!("expected matrix"),
        }
    }

    #[test]
    fn test_no_fold_list_literal() {
        // 列表字面量：覆盖 transform_no_fold List 分支（lines 179-184）
        let ast = parse("[1,2,3]").unwrap();
        let (canon_ast, cf) = AstCanonicalizer::canonicalize_no_fold(&ast).unwrap();
        assert_eq!(cf.as_str(), "(list 1 2 3)");
        match canon_ast {
            AstNode::List(elems) => assert_eq!(elems.len(), 3),
            _ => panic!("expected list"),
        }
    }

    #[test]
    fn test_no_fold_preserves_constants() {
        // 2+3 → (+ 2 3)（不折叠为 5，仅排序）
        let ast = parse("3+2").unwrap();
        let (_, cf) = AstCanonicalizer::canonicalize_no_fold(&ast).unwrap();
        assert_eq!(cf.as_str(), "(+ 2 3)");
    }

    // ===== proptest 属性测试（任务 3.5） =====

    use proptest::prelude::*;

    /// 生成单字母变量名
    fn var_strategy() -> impl Strategy<Value = String> {
        (0u8..26u8).prop_map(|i| ((b'a' + i) as char).to_string())
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

        // 交换律：a+b 与 b+a 同规范形式（使用变量避免常量折叠）
        #[test]
        fn prop_commutativity_add(a in var_strategy(), b in var_strategy()) {
            let cf1 = canon(&format!("{}+{}", a, b)).unwrap();
            let cf2 = canon(&format!("{}+{}", b, a)).unwrap();
            prop_assert_eq!(cf1, cf2);
        }

        // 交换律：a*b 与 b*a 同规范形式
        #[test]
        fn prop_commutativity_mul(a in var_strategy(), b in var_strategy()) {
            let cf1 = canon(&format!("{}*{}", a, b)).unwrap();
            let cf2 = canon(&format!("{}*{}", b, a)).unwrap();
            prop_assert_eq!(cf1, cf2);
        }

        // 结合律（常量）：(a+b)+c 与 a+(b+c) 同规范形式（通过常量折叠实现）
        #[test]
        fn prop_associativity_constants(
            a in 1u32..1000, b in 1u32..1000, c in 1u32..1000
        ) {
            let cf1 = canon(&format!("({}+{})+{}", a, b, c)).unwrap();
            let cf2 = canon(&format!("{}+({}+{})", a, b, c)).unwrap();
            prop_assert_eq!(cf1, cf2);
        }

        // 幂等性：规范化已规范化的 AST 结果不变
        #[test]
        fn prop_idempotent(a in 1u32..1000, b in 1u32..1000, c in var_strategy()) {
            let expr = format!("{}*{}+{}", a, b, c);
            let ast = parse(&expr).unwrap();
            let (canon_ast, cf1) = AstCanonicalizer::canonicalize(&ast).unwrap();
            let (_, cf2) = AstCanonicalizer::canonicalize(&canon_ast).unwrap();
            prop_assert_eq!(cf1, cf2);
        }
    }
}
