//! Statistics 计算域：均值、方差、标准差、中位数、最值、求和、计数。
//!
//! 设计依据：
//! - statistics-domain spec：10 个 requirements / 21 个 scenarios
//! - design.md D6：自研实现，无外部依赖，priority=20
//!
//! 路由策略：AST 含统计函数调用（mean/variance/std/median/min/max/sum/count）时路由至本域。
//! 输入为 List 节点；空列表与非数值元素（含嵌套 List/Matrix/Complex）返回 DomainError。

use crate::core::domain::CalculationDomain;
use crate::core::types::{AstNode, BinaryOp, CalcError, EvalContext, EvalResult, UnaryOp};

/// 统计函数白名单。
const STATISTICS_FUNCTIONS: &[&str] = &[
    "mean", "variance", "std", "median", "min", "max", "sum", "count",
];

/// Statistics 计算域。
///
/// priority=20，支持 mean/variance/std/median/min/max/sum/count。
/// 输入为 List 节点，空列表与非数值元素返回 DomainError。
pub struct StatisticsDomain;

impl CalculationDomain for StatisticsDomain {
    fn domain_name(&self) -> &str {
        "statistics"
    }

    fn priority(&self) -> u8 {
        20
    }

    fn supports(&self, ast: &AstNode) -> bool {
        contains_statistics_function(ast)
    }

    fn evaluate(&self, ast: &AstNode, ctx: &EvalContext) -> Result<EvalResult, CalcError> {
        let mut ctx = ctx.clone();
        if ctx.get_var("pi").is_none() {
            ctx = ctx.with_var("pi", std::f64::consts::PI);
        }
        if ctx.get_var("e").is_none() {
            ctx = ctx.with_var("e", std::f64::consts::E);
        }

        let value = self.eval_node(ast, &ctx)?;
        if !value.is_finite() {
            return Err(CalcError::NaNOrInf);
        }
        Ok(EvalResult::Scalar(value))
    }
}

impl StatisticsDomain {
    /// 递归求值 AST 节点，返回标量。
    fn eval_node(&self, ast: &AstNode, ctx: &EvalContext) -> Result<f64, CalcError> {
        match ast {
            AstNode::Number(n) => Ok(*n),
            AstNode::Variable(name) => ctx
                .get_var(name)
                .ok_or_else(|| CalcError::EvalError(format!("unbound variable: {}", name))),
            AstNode::BinaryOp(op, l, r) => {
                let a = self.eval_node(l, ctx)?;
                let b = self.eval_node(r, ctx)?;
                self.eval_binary(*op, a, b)
            }
            AstNode::UnaryOp(op, e) => {
                let v = self.eval_node(e, ctx)?;
                match op {
                    UnaryOp::Neg => Ok(-v),
                    UnaryOp::Abs => Ok(v.abs()),
                    UnaryOp::Factorial => Err(CalcError::DomainError(
                        "factorial not supported in statistics domain".to_string(),
                    )),
                }
            }
            AstNode::FunctionCall(name, args) => self.eval_function(name, args, ctx),
            AstNode::Complex(_, _) | AstNode::Matrix(_) | AstNode::List(_) | AstNode::BigNumber(_) => {
                Err(CalcError::DomainError(format!(
                    "statistics domain does not support this node type: {:?}",
                    ast
                )))
            }
        }
    }

    /// 求值标量二元运算（用于组合统计结果，如 `max([...]) + min([...])`）。
    fn eval_binary(&self, op: BinaryOp, a: f64, b: f64) -> Result<f64, CalcError> {
        let result = match op {
            BinaryOp::Add => a + b,
            BinaryOp::Sub => a - b,
            BinaryOp::Mul => a * b,
            BinaryOp::Div => {
                if b == 0.0 {
                    if a == 0.0 {
                        return Err(CalcError::NaNOrInf);
                    }
                    return Err(CalcError::DivisionByZero);
                }
                a / b
            }
            BinaryOp::Pow => {
                if a == 0.0 && b == 0.0 {
                    1.0
                } else {
                    a.powf(b)
                }
            }
            BinaryOp::Mod => {
                if b == 0.0 {
                    return Err(CalcError::DivisionByZero);
                }
                a % b
            }
        };
        if !result.is_finite() {
            return Err(CalcError::NaNOrInf);
        }
        Ok(result)
    }

    /// 求值统计函数调用。
    fn eval_function(
        &self,
        name: &str,
        args: &[AstNode],
        ctx: &EvalContext,
    ) -> Result<f64, CalcError> {
        if !STATISTICS_FUNCTIONS.contains(&name) {
            return Err(CalcError::DomainError(format!(
                "unsupported function in statistics domain: {}",
                name
            )));
        }
        if args.len() != 1 {
            return Err(CalcError::DomainError(format!(
                "{}() requires exactly 1 argument, got {}",
                name,
                args.len()
            )));
        }
        let values = self.extract_list(&args[0], ctx)?;
        // 空列表 → DomainError（spec Req 8）
        if values.is_empty() {
            return Err(CalcError::DomainError(format!(
                "{}() requires a non-empty list",
                name
            )));
        }
        match name {
            "mean" => Ok(values.iter().sum::<f64>() / values.len() as f64),
            "variance" => {
                let mean = values.iter().sum::<f64>() / values.len() as f64;
                let var = values.iter().map(|x| (x - mean).powi(2)).sum::<f64>()
                    / values.len() as f64;
                Ok(var)
            }
            "std" => {
                let mean = values.iter().sum::<f64>() / values.len() as f64;
                let var = values.iter().map(|x| (x - mean).powi(2)).sum::<f64>()
                    / values.len() as f64;
                Ok(var.sqrt())
            }
            "median" => {
                let mut sorted = values.clone();
                sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                let n = sorted.len();
                if n % 2 == 1 {
                    Ok(sorted[n / 2])
                } else {
                    Ok((sorted[n / 2 - 1] + sorted[n / 2]) / 2.0)
                }
            }
            "min" => Ok(values.iter().cloned().fold(f64::INFINITY, f64::min)),
            "max" => Ok(values.iter().cloned().fold(f64::NEG_INFINITY, f64::max)),
            "sum" => Ok(values.iter().sum()),
            "count" => Ok(values.len() as f64),
            _ => unreachable!(),
        }
    }

    /// 从 List 节点提取数值列表。
    /// 元素递归求值，必须为标量；List/Matrix/Complex 节点 → DomainError（spec Req 9）。
    fn extract_list(&self, ast: &AstNode, ctx: &EvalContext) -> Result<Vec<f64>, CalcError> {
        match ast {
            AstNode::List(elements) => {
                let mut values = Vec::with_capacity(elements.len());
                for elem in elements {
                    let v = self.eval_node(elem, ctx)?;
                    values.push(v);
                }
                Ok(values)
            }
            _ => Err(CalcError::DomainError(format!(
                "statistics functions require a list argument, got: {:?}",
                ast
            ))),
        }
    }
}

impl Default for StatisticsDomain {
    fn default() -> Self {
        Self
    }
}

/// 递归检查 AST 是否含统计函数调用（spec Req 10）。
fn contains_statistics_function(ast: &AstNode) -> bool {
    match ast {
        AstNode::FunctionCall(name, _) if STATISTICS_FUNCTIONS.contains(&name.as_str()) => true,
        AstNode::FunctionCall(_, args) => args.iter().any(contains_statistics_function),
        AstNode::BinaryOp(_, l, r) => {
            contains_statistics_function(l) || contains_statistics_function(r)
        }
        AstNode::UnaryOp(_, e) => contains_statistics_function(e),
        AstNode::Matrix(rows) => rows.iter().flatten().any(contains_statistics_function),
        AstNode::List(elements) => elements.iter().any(contains_statistics_function),
        AstNode::Number(_) | AstNode::Variable(_) | AstNode::Complex(_, _) | AstNode::BigNumber(_) => false,
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

    fn eval(input: &str) -> Result<f64, CalcError> {
        let ast = parse(input).unwrap();
        let domain = StatisticsDomain;
        let ctx = EvalContext::new()
            .with_var("pi", std::f64::consts::PI)
            .with_var("e", std::f64::consts::E);
        domain.evaluate(&ast, &ctx).map(|r| {
            r.as_scalar().expect("expected scalar result")
        })
    }

    // ===== Requirement 1: 列表字面量解析（通过 count 间接验证）=====

    #[test]
    fn test_standard_list_parse() {
        // count([1,2,3,4,5]) → 5（Req 1 Scen 1，5 元素 List）
        assert_eq!(eval("count([1,2,3,4,5])").unwrap(), 5.0);
    }

    #[test]
    fn test_single_element_list_parse() {
        // count([42]) → 1（Req 1 Scen 2，1 元素 List）
        assert_eq!(eval("count([42])").unwrap(), 1.0);
    }

    // ===== Requirement 2: 均值 =====

    #[test]
    fn test_mean_standard() {
        // mean([1,2,3,4,5]) → 3.0（Req 2 Scen 1）
        assert_eq!(eval("mean([1,2,3,4,5])").unwrap(), 3.0);
    }

    #[test]
    fn test_mean_single_element() {
        // mean([5]) → 5.0（Req 2 Scen 2）
        assert_eq!(eval("mean([5])").unwrap(), 5.0);
    }

    // ===== Requirement 3: 方差（总体方差，除以 N）=====

    #[test]
    fn test_variance_standard() {
        // variance([1,2,3,4,5]) → 2.0（Req 3 Scen 1，总体方差）
        assert_eq!(eval("variance([1,2,3,4,5])").unwrap(), 2.0);
    }

    #[test]
    fn test_variance_identical_elements() {
        // variance([3,3,3,3]) → 0.0（Req 3 Scen 2）
        assert_eq!(eval("variance([3,3,3,3])").unwrap(), 0.0);
    }

    // ===== Requirement 4: 标准差 =====

    #[test]
    fn test_std_standard() {
        // std([1,2,3,4,5]) → √2 ≈ 1.4142135623730951（Req 4 Scen 1）
        assert_approx(eval("std([1,2,3,4,5])").unwrap(), 1.4142135623730951);
    }

    #[test]
    fn test_std_identical_elements() {
        // std([5,5,5]) → 0.0（Req 4 Scen 2）
        assert_eq!(eval("std([5,5,5])").unwrap(), 0.0);
    }

    // ===== Requirement 5: 中位数 =====

    #[test]
    fn test_median_odd_length() {
        // median([1,2,3,4,5]) → 3.0（Req 5 Scen 1）
        assert_eq!(eval("median([1,2,3,4,5])").unwrap(), 3.0);
    }

    #[test]
    fn test_median_even_length() {
        // median([1,2,3,4]) → 2.5（Req 5 Scen 2）
        assert_eq!(eval("median([1,2,3,4])").unwrap(), 2.5);
    }

    #[test]
    fn test_median_unsorted() {
        // median([3,1,4,1,5]) → 3.0（Req 5 Scen 3，排序后取中位数）
        assert_eq!(eval("median([3,1,4,1,5])").unwrap(), 3.0);
    }

    // ===== Requirement 6: 最值 =====

    #[test]
    fn test_min() {
        // min([3,1,4,1,5,9,2,6]) → 1.0（Req 6 Scen 1）
        assert_eq!(eval("min([3,1,4,1,5,9,2,6])").unwrap(), 1.0);
    }

    #[test]
    fn test_max() {
        // max([3,1,4,1,5,9,2,6]) → 9.0（Req 6 Scen 2）
        assert_eq!(eval("max([3,1,4,1,5,9,2,6])").unwrap(), 9.0);
    }

    // ===== Requirement 7: 求和与计数 =====

    #[test]
    fn test_sum() {
        // sum([1,2,3,4,5]) → 15.0（Req 7 Scen 1）
        assert_eq!(eval("sum([1,2,3,4,5])").unwrap(), 15.0);
    }

    #[test]
    fn test_count() {
        // count([1,2,3,4,5]) → 5.0（Req 7 Scen 2）
        assert_eq!(eval("count([1,2,3,4,5])").unwrap(), 5.0);
    }

    // ===== Requirement 8: 空列表处理 =====

    #[test]
    fn test_empty_list_mean() {
        // mean([]) → DomainError（Req 8 Scen 1）
        let result = eval("mean([])");
        assert!(result.is_err());
        assert!(
            matches!(result, Err(CalcError::DomainError(_))),
            "expected DomainError, got {:?}",
            result
        );
    }

    #[test]
    fn test_empty_list_sum() {
        // sum([]) → DomainError（Req 8 Scen 2）
        let result = eval("sum([])");
        assert!(result.is_err());
        assert!(
            matches!(result, Err(CalcError::DomainError(_))),
            "expected DomainError, got {:?}",
            result
        );
    }

    // ===== Requirement 9: 非数值列表 =====

    #[test]
    fn test_nested_list_rejected() {
        // mean([1, [2,3], 4]) → DomainError（Req 9 Scen 2，含嵌套列表）
        let result = eval("mean([1, [2,3], 4])");
        assert!(result.is_err());
        assert!(
            matches!(result, Err(CalcError::DomainError(_))),
            "expected DomainError, got {:?}",
            result
        );
    }

    #[test]
    fn test_matrix_in_list_rejected() {
        // max([1, [[2,3],[4,5]], 6]) → DomainError（Req 9 Scen 2，含矩阵）
        let result = eval("max([1, [[2,3],[4,5]], 6])");
        assert!(result.is_err());
        assert!(
            matches!(result, Err(CalcError::DomainError(_))),
            "expected DomainError, got {:?}",
            result
        );
    }

    // ===== Requirement 10: 域路由 =====

    #[test]
    fn test_route_statistics_function() {
        // mean([1,2,3,4,5]) → 含统计函数，路由到 StatisticsDomain（Req 10 Scen 1）
        let ast = parse("mean([1,2,3,4,5])").unwrap();
        let domain = StatisticsDomain;
        assert!(domain.supports(&ast));
    }

    #[test]
    fn test_route_nested_statistics() {
        // max([1,2,3]) + min([4,5,6]) → 含统计函数，路由到 StatisticsDomain（Req 10 Scen 2）
        let ast = parse("max([1,2,3]) + min([4,5,6])").unwrap();
        let domain = StatisticsDomain;
        assert!(domain.supports(&ast));
    }

    // ===== 额外覆盖 =====

    #[test]
    fn test_statistics_domain_priority() {
        let domain = StatisticsDomain;
        assert_eq!(domain.priority(), 20);
        assert_eq!(domain.domain_name(), "statistics");
    }

    #[test]
    fn test_combined_statistics_expression() {
        // max([1,2,3]) + min([4,5,6]) → 3 + 4 = 7
        assert_eq!(eval("max([1,2,3]) + min([4,5,6])").unwrap(), 7.0);
    }

    #[test]
    fn test_mean_minus_mean() {
        // mean([1,2,3]) - mean([4,5,6]) → 2 - 5 = -3
        assert_eq!(eval("mean([1,2,3]) - mean([4,5,6])").unwrap(), -3.0);
    }

    #[test]
    fn test_list_with_arithmetic_expressions() {
        // mean([1+1, 2*2, 3-1]) → mean([2, 4, 2]) → 8/3
        let result = eval("mean([1+1, 2*2, 3-1])").unwrap();
        assert_approx(result, 8.0 / 3.0);
    }

    #[test]
    fn test_wrong_arg_count() {
        // mean([1,2], [3,4]) → DomainError（参数数量错误）
        let ast = AstNode::FunctionCall(
            "mean".to_string(),
            vec![
                AstNode::List(vec![AstNode::Number(1.0), AstNode::Number(2.0)]),
                AstNode::List(vec![AstNode::Number(3.0), AstNode::Number(4.0)]),
            ],
        );
        let domain = StatisticsDomain;
        let ctx = EvalContext::new();
        let result = domain.evaluate(&ast, &ctx);
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_non_list_argument() {
        // mean(5) → DomainError（参数非 List）
        let ast = AstNode::FunctionCall(
            "mean".to_string(),
            vec![AstNode::Number(5.0)],
        );
        let domain = StatisticsDomain;
        let ctx = EvalContext::new();
        let result = domain.evaluate(&ast, &ctx);
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_unsupported_function() {
        // sin([1,2,3]) → DomainError（sin 不是统计函数）
        let ast = AstNode::FunctionCall(
            "sin".to_string(),
            vec![AstNode::List(vec![AstNode::Number(1.0), AstNode::Number(2.0)])],
        );
        let domain = StatisticsDomain;
        let ctx = EvalContext::new();
        let result = domain.evaluate(&ast, &ctx);
        assert!(matches!(result, Err(CalcError::DomainError(_))));
    }

    #[test]
    fn test_negative_numbers_in_list() {
        // mean([-1, -2, 3, 4]) → 4/4 = 1.0
        assert_eq!(eval("mean([-1, -2, 3, 4])").unwrap(), 1.0);
    }

    #[test]
    fn test_median_two_elements() {
        // median([1, 2]) → 1.5
        assert_eq!(eval("median([1, 2])").unwrap(), 1.5);
    }

    #[test]
    fn test_min_max_with_negatives() {
        assert_eq!(eval("min([-3, -1, -2])").unwrap(), -3.0);
        assert_eq!(eval("max([-3, -1, -2])").unwrap(), -1.0);
    }

    #[test]
    fn test_sum_single_element() {
        assert_eq!(eval("sum([42])").unwrap(), 42.0);
    }

    // ===== proptest 属性测试（任务 14.7）=====

    use proptest::prelude::*;

    /// 生成非空 f64 列表策略（有限值，避免 NaN/Inf 干扰）
    fn non_empty_finite_list() -> impl Strategy<Value = Vec<f64>> {
        prop::collection::vec(-1e3f64..1e3, 1..20)
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

        /// 属性：mean = sum / count
        #[test]
        fn prop_mean_equals_sum_over_count(values in non_empty_finite_list()) {
            let elements: Vec<AstNode> = values.iter().map(|&v| AstNode::Number(v)).collect();
            let ast = AstNode::FunctionCall(
                "mean".to_string(),
                vec![AstNode::List(elements)],
            );
            let domain = StatisticsDomain;
            let ctx = EvalContext::new();
            let result = domain.evaluate(&ast, &ctx).unwrap();
            let expected = values.iter().sum::<f64>() / values.len() as f64;
            match result {
                EvalResult::Scalar(v) => prop_assert!((v - expected).abs() < 1e-9),
                _ => panic!("expected Scalar"),
            }
        }

        /// 属性：variance ≥ 0
        #[test]
        fn prop_variance_non_negative(values in non_empty_finite_list()) {
            let elements: Vec<AstNode> = values.iter().map(|&v| AstNode::Number(v)).collect();
            let ast = AstNode::FunctionCall(
                "variance".to_string(),
                vec![AstNode::List(elements)],
            );
            let domain = StatisticsDomain;
            let ctx = EvalContext::new();
            let result = domain.evaluate(&ast, &ctx).unwrap();
            match result {
                EvalResult::Scalar(v) => prop_assert!(v >= -1e-9, "variance {} should be non-negative", v),
                _ => panic!("expected Scalar"),
            }
        }

        /// 属性：min ≤ median ≤ max
        #[test]
        fn prop_median_in_range(values in non_empty_finite_list()) {
            let elements: Vec<AstNode> = values.iter().map(|&v| AstNode::Number(v)).collect();
            let ctx = EvalContext::new();
            let domain = StatisticsDomain;

            let min_ast = AstNode::FunctionCall("min".to_string(), vec![AstNode::List(elements.clone())]);
            let max_ast = AstNode::FunctionCall("max".to_string(), vec![AstNode::List(elements.clone())]);
            let med_ast = AstNode::FunctionCall("median".to_string(), vec![AstNode::List(elements)]);

            let min_v = domain.evaluate(&min_ast, &ctx).unwrap().as_scalar().unwrap();
            let max_v = domain.evaluate(&max_ast, &ctx).unwrap().as_scalar().unwrap();
            let med_v = domain.evaluate(&med_ast, &ctx).unwrap().as_scalar().unwrap();

            prop_assert!(med_v >= min_v - 1e-9, "median {} < min {}", med_v, min_v);
            prop_assert!(med_v <= max_v + 1e-9, "median {} > max {}", med_v, max_v);
        }
    }
}
