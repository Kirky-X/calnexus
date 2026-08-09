// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! Statistics 计算域：均值、方差、标准差、中位数、最值、求和、计数。
//!
//! 设计依据：
//! - statistics-domain spec：10 个 requirements / 21 个 scenarios
//! - design.md D6：自研实现，无外部依赖，priority=20
//!
//! 路由策略：AST 含统计函数调用（mean/variance/std/median/min/max/sum/count）时路由至本域。
//! 输入为 List 节点；空列表与非数值元素（含嵌套 List/Matrix/Complex）返回 DomainError。

use crate::core::CalculationDomain;
use crate::core::{AstNode, BinaryOp, CalcError, EvalContext, EvalResult, UnaryOp};
use super::common::{ensure_math_constants, resolve_variable, unsupported_node_error, unsupported_function_error};

use crate::math::statistics as math_stats;

/// 扩展统计函数白名单：基础 8 + 分布 16 + 检验 3 + 相关 2 = 29。
const STATISTICS_FUNCTIONS: &[&str] = &[
    // 基础统计
    "mean", "variance", "std", "median", "min", "max", "sum", "count",
    // 分布函数
    "norm_pdf", "norm_cdf", "norm_inv",
    "t_pdf", "t_cdf", "t_inv",
    "chi2_pdf", "chi2_cdf", "chi2_inv",
    "f_pdf", "f_cdf", "f_inv",
    "poisson_pmf", "poisson_cdf",
    "binom_pmf", "binom_cdf",
    // 假设检验
    "t_test_one", "t_test_two", "chi2_test",
    // 相关系数
    "pearson", "spearman",
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
        let ctx = ensure_math_constants(ctx);

        let result = self.eval_node(ast, &ctx)?;
        match result {
            EvalResult::Scalar(v) if !v.is_finite() => Err(CalcError::nan_or_inf()),
            other => Ok(other),
        }
    }
}

impl StatisticsDomain {
    /// 递归求值 AST 节点，返回 EvalResult（支持标量和 JSON）。
    fn eval_node(&self, ast: &AstNode, ctx: &EvalContext) -> Result<EvalResult, CalcError> {
        match ast {
            AstNode::Number(n) => Ok(EvalResult::Scalar(*n)),
            AstNode::Variable(name) => resolve_variable(ctx, name).map(EvalResult::Scalar),
            AstNode::BinaryOp(op, l, r) => {
                let a = self.eval_node(l, ctx)?.as_scalar().ok_or_else(|| {
                    CalcError::domain("binary op requires scalar operands".to_string())
                })?;
                let b = self.eval_node(r, ctx)?.as_scalar().ok_or_else(|| {
                    CalcError::domain("binary op requires scalar operands".to_string())
                })?;
                self.eval_binary(*op, a, b).map(EvalResult::Scalar)
            }
            AstNode::UnaryOp(op, e) => {
                let v = self.eval_node(e, ctx)?.as_scalar().ok_or_else(|| {
                    CalcError::domain("unary op requires scalar operand".to_string())
                })?;
                match op {
                    UnaryOp::Neg => Ok(EvalResult::Scalar(-v)),
                    UnaryOp::Abs => Ok(EvalResult::Scalar(v.abs())),
                    UnaryOp::Factorial => Err(CalcError::domain(
                        "factorial not supported in statistics domain".to_string(),
                    )
                    .with_i18n("msg.statistics.factorial_not_supported", vec![])),
                }
            }
            AstNode::FunctionCall(name, args) => self.eval_function(name, args, ctx),
            AstNode::Complex(_, _)
            | AstNode::Matrix(_)
            | AstNode::List(_)
            | AstNode::BigNumber(_)
            | AstNode::Str(_) => Err(unsupported_node_error("statistics", ast)),
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
                        return Err(CalcError::nan_or_inf());
                    }
                    return Err(CalcError::division_by_zero());
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

    /// 求值函数调用（支持标量和 JSON 结果）。
    fn eval_function(
        &self,
        name: &str,
        args: &[AstNode],
        ctx: &EvalContext,
    ) -> Result<EvalResult, CalcError> {
        if !STATISTICS_FUNCTIONS.contains(&name) {
            return Err(unsupported_function_error("statistics", name));
        }

        // 基础统计函数：单列表参数
        if matches!(name, "mean" | "variance" | "std" | "median" | "min" | "max" | "sum" | "count") {
            if args.len() != 1 {
                return Err(CalcError::domain(format!(
                    "{}() requires exactly 1 argument, got {}",
                    name, args.len()
                )).with_i18n("msg.statistics.arg_count",
                    vec![("name".to_string(), name.to_string()), ("actual".to_string(), args.len().to_string())]));
            }
            let values = self.extract_list(&args[0], ctx)?;
            if values.is_empty() {
                return Err(CalcError::domain(format!("{}() requires a non-empty list", name))
                    .with_i18n("msg.statistics.requires_non_empty_list",
                        vec![("name".to_string(), name.to_string())]));
            }
            return self.eval_basic_stat(name, &values).map(EvalResult::Scalar);
        }

        // 分布函数：全部标量参数
        if let Some(v) = self.try_eval_distribution(name, args, ctx)? {
            return Ok(EvalResult::Scalar(v));
        }

        // 假设检验函数：返回列表 + 标量混合参数 → JSON
        if let Some(result) = self.try_eval_test(name, args, ctx)? {
            return Ok(result);
        }

        // 相关系数函数：双列表参数
        if let Some(v) = self.try_eval_correlation(name, args, ctx)? {
            return Ok(EvalResult::Scalar(v));
        }

        Err(CalcError::domain(format!("unhandled statistics function: {}", name)))
    }

    /// 基础统计函数求值。
    fn eval_basic_stat(&self, name: &str, values: &[f64]) -> Result<f64, CalcError> {
        Ok(match name {
            "mean" => math_stats::mean(values),
            "variance" => math_stats::variance(values),
            "std" => math_stats::std(values),
            "median" => math_stats::median(values),
            "min" => math_stats::min(values),
            "max" => math_stats::max(values),
            "sum" => math_stats::sum(values),
            "count" => math_stats::count(values),
            _ => return Err(CalcError::domain(format!("unknown basic stat function: {}", name))),
        })
    }

    /// 求值标量参数（从 AST 参数列表提取第 idx 个标量）。
    fn eval_scalar_arg(&self, arg: &AstNode, ctx: &EvalContext) -> Result<f64, CalcError> {
        self.eval_node(arg, ctx)?.as_scalar().ok_or_else(|| {
            CalcError::domain("expected scalar argument".to_string())
        })
    }

    /// 尝试求值分布函数。返回 Ok(None) 表示不是分布函数。
    fn try_eval_distribution(
        &self, name: &str, args: &[AstNode], ctx: &EvalContext,
    ) -> Result<Option<f64>, CalcError> {
        let scalars = || -> Result<Vec<f64>, CalcError> {
            args.iter().map(|a| self.eval_scalar_arg(a, ctx)).collect()
        };
        let v = match name {
            "norm_pdf" => { let s = scalars()?; self.check_len(name, &s, 3)?; math_stats::norm_pdf(s[0], s[1], s[2]) }
            "norm_cdf" => { let s = scalars()?; self.check_len(name, &s, 3)?; math_stats::norm_cdf(s[0], s[1], s[2]) }
            "norm_inv" => { let s = scalars()?; self.check_len(name, &s, 3)?; math_stats::norm_inv(s[0], s[1], s[2]) }
            "t_pdf" => { let s = scalars()?; self.check_len(name, &s, 2)?; math_stats::t_pdf(s[0], s[1]) }
            "t_cdf" => { let s = scalars()?; self.check_len(name, &s, 2)?; math_stats::t_cdf(s[0], s[1]) }
            "t_inv" => { let s = scalars()?; self.check_len(name, &s, 2)?; math_stats::t_inv(s[0], s[1]) }
            "chi2_pdf" => { let s = scalars()?; self.check_len(name, &s, 2)?; math_stats::chi2_pdf(s[0], s[1]) }
            "chi2_cdf" => { let s = scalars()?; self.check_len(name, &s, 2)?; math_stats::chi2_cdf(s[0], s[1]) }
            "chi2_inv" => { let s = scalars()?; self.check_len(name, &s, 2)?; math_stats::chi2_inv(s[0], s[1]) }
            "f_pdf" => { let s = scalars()?; self.check_len(name, &s, 3)?; math_stats::f_pdf(s[0], s[1], s[2]) }
            "f_cdf" => { let s = scalars()?; self.check_len(name, &s, 3)?; math_stats::f_cdf(s[0], s[1], s[2]) }
            "f_inv" => { let s = scalars()?; self.check_len(name, &s, 3)?; math_stats::f_inv(s[0], s[1], s[2]) }
            "poisson_pmf" => { let s = scalars()?; self.check_len(name, &s, 2)?; math_stats::poisson_pmf(s[0], s[1]) }
            "poisson_cdf" => { let s = scalars()?; self.check_len(name, &s, 2)?; math_stats::poisson_cdf(s[0], s[1]) }
            "binom_pmf" => { let s = scalars()?; self.check_len(name, &s, 3)?; math_stats::binom_pmf(s[0], s[1], s[2]) }
            "binom_cdf" => { let s = scalars()?; self.check_len(name, &s, 3)?; math_stats::binom_cdf(s[0], s[1], s[2]) }
            _ => return Ok(None),
        };
        Ok(Some(v))
    }

    /// 尝试求值假设检验函数。返回 Ok(None) 表示不是检验函数。
    fn try_eval_test(
        &self, name: &str, args: &[AstNode], ctx: &EvalContext,
    ) -> Result<Option<EvalResult>, CalcError> {
        let result = match name {
            "t_test_one" => {
                self.check_arg_count(name, args, 2)?;
                let data = self.extract_list(&args[0], ctx)?;
                let mu = self.eval_scalar_arg(&args[1], ctx)?;
                if data.is_empty() {
                    return Err(CalcError::domain("t_test_one requires non-empty data".to_string()));
                }
                let map = math_stats::t_test_one(&data, mu);
                EvalResult::Json(serde_json::to_value(&map).map_err(|e| CalcError::domain(format!("failed to serialize t_test_one result: {e}")))?)
            }
            "t_test_two" => {
                self.check_arg_count(name, args, 2)?;
                let a = self.extract_list(&args[0], ctx)?;
                let b = self.extract_list(&args[1], ctx)?;
                if a.is_empty() || b.is_empty() {
                    return Err(CalcError::domain("t_test_two requires non-empty data".to_string()));
                }
                let map = math_stats::t_test_two(&a, &b);
                EvalResult::Json(serde_json::to_value(&map).map_err(|e| CalcError::domain(format!("failed to serialize t_test_two result: {e}")))?)
            }
            "chi2_test" => {
                self.check_arg_count(name, args, 2)?;
                let observed = self.extract_list(&args[0], ctx)?;
                let expected = self.extract_list(&args[1], ctx)?;
                if observed.is_empty() || expected.is_empty() {
                    return Err(CalcError::domain("chi2_test requires non-empty data".to_string()));
                }
                if observed.len() != expected.len() {
                    return Err(CalcError::domain(format!(
                        "chi2_test: observed ({}) and expected ({}) must have same length",
                        observed.len(), expected.len()
                    )));
                }
                let map = math_stats::chi2_test(&observed, &expected);
                EvalResult::Json(serde_json::to_value(&map).map_err(|e| CalcError::domain(format!("failed to serialize chi2_test result: {e}")))?)
            }
            _ => return Ok(None),
        };
        Ok(Some(result))
    }

    /// 尝试求值相关系数函数。返回 Ok(None) 表示不是相关函数。
    fn try_eval_correlation(
        &self, name: &str, args: &[AstNode], ctx: &EvalContext,
    ) -> Result<Option<f64>, CalcError> {
        let v = match name {
            "pearson" => {
                self.check_arg_count(name, args, 2)?;
                let x = self.extract_list(&args[0], ctx)?;
                let y = self.extract_list(&args[1], ctx)?;
                if x.len() != y.len() {
                    return Err(CalcError::domain(format!(
                        "pearson: x ({}) and y ({}) must have same length",
                        x.len(), y.len()
                    )));
                }
                if x.len() < 2 {
                    return Err(CalcError::domain("pearson requires at least 2 data points".to_string()));
                }
                math_stats::pearson(&x, &y)
            }
            "spearman" => {
                self.check_arg_count(name, args, 2)?;
                let x = self.extract_list(&args[0], ctx)?;
                let y = self.extract_list(&args[1], ctx)?;
                if x.len() != y.len() {
                    return Err(CalcError::domain(format!(
                        "spearman: x ({}) and y ({}) must have same length",
                        x.len(), y.len()
                    )));
                }
                if x.len() < 2 {
                    return Err(CalcError::domain("spearman requires at least 2 data points".to_string()));
                }
                math_stats::spearman(&x, &y)
            }
            _ => return Ok(None),
        };
        Ok(Some(v))
    }

    /// 检查参数数量。
    fn check_arg_count(&self, name: &str, args: &[AstNode], expected: usize) -> Result<(), CalcError> {
        if args.len() != expected {
            return Err(CalcError::domain(format!(
                "{}() requires {} arguments, got {}", name, expected, args.len()
            )).with_i18n("msg.statistics.arg_count",
                vec![("name".to_string(), name.to_string()), ("actual".to_string(), args.len().to_string())]));
        }
        Ok(())
    }

    /// 检查标量参数数量。
    fn check_len(&self, name: &str, s: &[f64], expected: usize) -> Result<(), CalcError> {
        if s.len() != expected {
            return Err(CalcError::domain(format!(
                "{}() requires {} arguments, got {}", name, expected, s.len()
            )));
        }
        Ok(())
    }

    /// 从 List 节点提取数值列表。
    fn extract_list(&self, ast: &AstNode, ctx: &EvalContext) -> Result<Vec<f64>, CalcError> {
        match ast {
            AstNode::List(elements) => {
                let mut values = Vec::with_capacity(elements.len());
                for elem in elements {
                    let v = self.eval_node(elem, ctx)?.as_scalar().ok_or_else(|| {
                        CalcError::domain("list elements must be scalar".to_string())
                    })?;
                    values.push(v);
                }
                Ok(values)
            }
            _ => Err(CalcError::domain(format!(
                "statistics functions require a list argument, got: {:?}",
                ast
            ))
            .with_i18n(
                "msg.statistics.requires_list_arg",
                vec![("ast".to_string(), format!("{:?}", ast))],
            )),
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
        AstNode::Matrix(_) => false,
        AstNode::List(elements) => elements.iter().any(contains_statistics_function),
        AstNode::Number(_)
        | AstNode::Variable(_)
        | AstNode::Complex(_, _)
        | AstNode::BigNumber(_)
        | AstNode::Str(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::parse;
    use crate::core::ErrorKind;

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
        domain
            .evaluate(&ast, &ctx)
            .map(|r| r.as_scalar().expect("expected scalar result"))
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
            matches!(&result, Err(e) if e.kind == ErrorKind::Domain),
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
            matches!(&result, Err(e) if e.kind == ErrorKind::Domain),
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
            matches!(&result, Err(e) if e.kind == ErrorKind::Domain),
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
            matches!(&result, Err(e) if e.kind == ErrorKind::Domain),
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
        assert!(matches!(&result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_non_list_argument() {
        // mean(5) → DomainError（参数非 List）
        let ast = AstNode::FunctionCall("mean".to_string(), vec![AstNode::Number(5.0)]);
        let domain = StatisticsDomain;
        let ctx = EvalContext::new();
        let result = domain.evaluate(&ast, &ctx);
        assert!(matches!(&result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_unsupported_function() {
        // sin([1,2,3]) → DomainError（sin 不是统计函数）
        let ast = AstNode::FunctionCall(
            "sin".to_string(),
            vec![AstNode::List(vec![
                AstNode::Number(1.0),
                AstNode::Number(2.0),
            ])],
        );
        let domain = StatisticsDomain;
        let ctx = EvalContext::new();
        let result = domain.evaluate(&ast, &ctx);
        assert!(matches!(&result, Err(e) if e.kind == ErrorKind::Domain));
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

    // ===== 覆盖未覆盖分支的补充测试 =====

    #[test]
    fn test_evaluate_result_not_finite() {
        // line 48: evaluate() 结果非有限 → NaNOrInf
        // sum([1e308, 1e308]) = inf
        let ast = AstNode::FunctionCall(
            "sum".to_string(),
            vec![AstNode::List(vec![
                AstNode::Number(1e308),
                AstNode::Number(1e308),
            ])],
        );
        let domain = StatisticsDomain;
        let result = domain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(&result, Err(e) if e.kind == ErrorKind::NaNOrInf));
    }

    #[test]
    fn test_unbound_variable_in_list() {
        // lines 59-61: 列表中含未绑定变量 → EvalError
        let ast = AstNode::FunctionCall(
            "mean".to_string(),
            vec![AstNode::List(vec![AstNode::Variable("y".to_string())])],
        );
        let domain = StatisticsDomain;
        let result = domain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(&result, Err(e) if e.kind == ErrorKind::Eval));
    }

    #[test]
    fn test_unary_abs_manual_ast() {
        // line 71: UnaryOp::Abs（parser 不产生此节点）
        let ast = AstNode::UnaryOp(UnaryOp::Abs, Box::new(AstNode::Number(-5.0)));
        let domain = StatisticsDomain;
        let result = domain.evaluate(&ast, &EvalContext::new()).unwrap();
        assert_eq!(result.as_scalar().unwrap(), 5.0);
    }

    #[test]
    fn test_unary_factorial_not_supported() {
        // lines 72-74: UnaryOp::Factorial → DomainError
        let ast = AstNode::UnaryOp(UnaryOp::Factorial, Box::new(AstNode::Number(5.0)));
        let domain = StatisticsDomain;
        let result = domain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(&result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_scalar_zero_div_zero() {
        // lines 94-96: 0/0 → NaNOrInf
        let ast = AstNode::BinaryOp(
            BinaryOp::Div,
            Box::new(AstNode::Number(0.0)),
            Box::new(AstNode::Number(0.0)),
        );
        let domain = StatisticsDomain;
        let result = domain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(&result, Err(e) if e.kind == ErrorKind::NaNOrInf));
    }

    #[test]
    fn test_scalar_div_by_zero() {
        // line 98: x/0 (x≠0) → DivisionByZero
        let ast = AstNode::BinaryOp(
            BinaryOp::Div,
            Box::new(AstNode::Number(5.0)),
            Box::new(AstNode::Number(0.0)),
        );
        let domain = StatisticsDomain;
        let result = domain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(&result, Err(e) if e.kind == ErrorKind::DivisionByZero));
    }

    #[test]
    fn test_scalar_div_normal() {
        // line 100: 正常除法 a / b
        let ast = AstNode::BinaryOp(
            BinaryOp::Div,
            Box::new(AstNode::Number(10.0)),
            Box::new(AstNode::Number(2.0)),
        );
        let domain = StatisticsDomain;
        let result = domain.evaluate(&ast, &EvalContext::new()).unwrap();
        assert_eq!(result.as_scalar().unwrap(), 5.0);
    }

    #[test]
    fn test_scalar_zero_pow_zero() {
        // lines 103-104: 0^0 → 1.0
        let ast = AstNode::BinaryOp(
            BinaryOp::Pow,
            Box::new(AstNode::Number(0.0)),
            Box::new(AstNode::Number(0.0)),
        );
        let domain = StatisticsDomain;
        let result = domain.evaluate(&ast, &EvalContext::new()).unwrap();
        assert_eq!(result.as_scalar().unwrap(), 1.0);
    }

    #[test]
    fn test_scalar_pow_normal() {
        // line 106: a.powf(b) 正常路径
        let ast = AstNode::BinaryOp(
            BinaryOp::Pow,
            Box::new(AstNode::Number(2.0)),
            Box::new(AstNode::Number(3.0)),
        );
        let domain = StatisticsDomain;
        let result = domain.evaluate(&ast, &EvalContext::new()).unwrap();
        assert_eq!(result.as_scalar().unwrap(), 8.0);
    }

    #[test]
    fn test_scalar_mod_by_zero() {
        // lines 110-111: mod by zero → DivisionByZero
        let ast = AstNode::BinaryOp(
            BinaryOp::Mod,
            Box::new(AstNode::Number(10.0)),
            Box::new(AstNode::Number(0.0)),
        );
        let domain = StatisticsDomain;
        let result = domain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(&result, Err(e) if e.kind == ErrorKind::DivisionByZero));
    }

    #[test]
    fn test_scalar_mod_normal() {
        // line 113: a % b 正常路径
        let ast = AstNode::BinaryOp(
            BinaryOp::Mod,
            Box::new(AstNode::Number(10.0)),
            Box::new(AstNode::Number(3.0)),
        );
        let domain = StatisticsDomain;
        let result = domain.evaluate(&ast, &EvalContext::new()).unwrap();
        assert_eq!(result.as_scalar().unwrap(), 1.0);
    }

    #[test]
    fn test_scalar_result_not_finite() {
        // line 117: eval_binary 结果非有限 → NaNOrInf
        let ast = AstNode::BinaryOp(
            BinaryOp::Add,
            Box::new(AstNode::Number(1e308)),
            Box::new(AstNode::Number(1e308)),
        );
        let domain = StatisticsDomain;
        let result = domain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(&result, Err(e) if e.kind == ErrorKind::NaNOrInf));
    }

    #[test]
    fn test_default_impl() {
        // lines 203-205: Default impl
        let domain = StatisticsDomain;
        assert_eq!(domain.domain_name(), "statistics");
        assert_eq!(domain.priority(), 20);
    }

    #[test]
    fn test_contains_statistics_unary_op() {
        // line 216: contains_statistics_function for UnaryOp
        let ast = AstNode::UnaryOp(
            UnaryOp::Neg,
            Box::new(AstNode::FunctionCall(
                "mean".to_string(),
                vec![AstNode::List(vec![AstNode::Number(1.0)])],
            )),
        );
        let domain = StatisticsDomain;
        assert!(domain.supports(&ast));
    }

    #[test]
    fn test_contains_statistics_matrix() {
        // Matrix 不支持，eval_node 会拒绝
        let ast = AstNode::Matrix(vec![vec![AstNode::FunctionCall(
            "sum".to_string(),
            vec![AstNode::List(vec![AstNode::Number(1.0)])],
        )]]);
        let domain = StatisticsDomain;
        assert!(!domain.supports(&ast));
    }

    #[test]
    fn test_contains_statistics_list() {
        // line 218: contains_statistics_function for List
        let ast = AstNode::List(vec![AstNode::FunctionCall(
            "count".to_string(),
            vec![AstNode::List(vec![AstNode::Number(1.0)])],
        )]);
        let domain = StatisticsDomain;
        assert!(domain.supports(&ast));
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

    // ===== Phase 4 集成测试：分布函数、检验函数、相关函数 =====

    fn eval_to_result(input: &str) -> Result<EvalResult, CalcError> {
        let ast = parse(input).unwrap();
        let domain = StatisticsDomain;
        let ctx = EvalContext::new()
            .with_var("pi", std::f64::consts::PI)
            .with_var("e", std::f64::consts::E);
        domain.evaluate(&ast, &ctx)
    }

    fn eval_scalar(input: &str) -> f64 {
        eval_to_result(input).unwrap().as_scalar().expect("expected scalar")
    }

    #[test]
    fn test_norm_cdf_integration() {
        let v = eval_scalar("norm_cdf(1.96, 0, 1)");
        assert!((v - 0.975).abs() < 0.001, "norm_cdf(1.96,0,1) = {}", v);
    }

    #[test]
    fn test_norm_pdf_integration() {
        let v = eval_scalar("norm_pdf(0, 0, 1)");
        assert!((v - 0.3989).abs() < 0.001, "norm_pdf(0,0,1) = {}", v);
    }

    #[test]
    fn test_t_cdf_integration() {
        let v = eval_scalar("t_cdf(2.228, 10)");
        assert!((v - 0.975).abs() < 0.001, "t_cdf(2.228,10) = {}", v);
    }

    #[test]
    fn test_chi2_cdf_integration() {
        let v = eval_scalar("chi2_cdf(5.991, 5)");
        assert!((v - 0.6929).abs() < 0.001, "chi2_cdf(5.991,5) = {}", v);
    }

    #[test]
    fn test_f_cdf_integration() {
        let v = eval_scalar("f_cdf(4.24, 5, 10)");
        assert!((v - 0.975).abs() < 0.01, "f_cdf(4.24,5,10) = {}", v);
    }

    #[test]
    fn test_poisson_pmf_integration() {
        let v = eval_scalar("poisson_pmf(3, 2)");
        assert!((v - 0.1804).abs() < 0.001, "poisson_pmf(3,2) = {}", v);
    }

    #[test]
    fn test_binom_cdf_integration() {
        let v = eval_scalar("binom_cdf(5, 10, 0.5)");
        assert!((v - 0.6230).abs() < 0.001, "binom_cdf(5,10,0.5) = {}", v);
    }

    #[test]
    fn test_t_test_one_integration() {
        let result = eval_to_result("t_test_one([1,2,3,4,5], 3)").unwrap();
        match result {
            EvalResult::Json(ref v) => {
                let t = v["t"].as_f64().unwrap();
                let p = v["p"].as_f64().unwrap();
                assert!(t.abs() < 1e-10, "t should be 0, got {}", t);
                assert!((p - 1.0).abs() < 1e-10, "p should be 1, got {}", p);
            }
            _ => panic!("expected Json result from t_test_one"),
        }
    }

    #[test]
    fn test_chi2_test_integration() {
        let result = eval_to_result("chi2_test([16,18,16,14,12,12], [16,16,16,16,16,16])").unwrap();
        match result {
            EvalResult::Json(ref v) => {
                let chi2 = v["chi2"].as_f64().unwrap();
                assert!((chi2 - 2.5).abs() < 1e-10, "chi2 should be 2.5, got {}", chi2);
            }
            _ => panic!("expected Json result from chi2_test"),
        }
    }

    #[test]
    fn test_pearson_integration() {
        let v = eval_scalar("pearson([1,2,3,4,5], [2,4,6,8,10])");
        assert!((v - 1.0).abs() < 1e-10, "pearson perfect positive = {}", v);
    }

    #[test]
    fn test_spearman_integration() {
        let v = eval_scalar("spearman([1,2,3,4,5], [5,4,3,2,1])");
        assert!((v - (-1.0)).abs() < 1e-10, "spearman perfect negative = {}", v);
    }

    #[test]
    fn test_route_distribution_function() {
        let ast = parse("norm_cdf(1.96, 0, 1)").unwrap();
        let domain = StatisticsDomain;
        assert!(domain.supports(&ast));
    }

    #[test]
    fn test_route_test_function() {
        let ast = parse("t_test_one([1,2,3], 2)").unwrap();
        let domain = StatisticsDomain;
        assert!(domain.supports(&ast));
    }

    #[test]
    fn test_route_correlation_function() {
        let ast = parse("pearson([1,2,3], [4,5,6])").unwrap();
        let domain = StatisticsDomain;
        assert!(domain.supports(&ast));
    }
}
