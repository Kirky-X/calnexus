// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! 物理单位换算域：8 量纲线性/仿射换算。
//!
//! 设计依据：design.md D5（自建换算表 + 温度仿射特例）+ D8（混合表达式求值语义）
//! Feature 门控：`unit = []`
//!
//! 函数表：
//! - `convert(value, "from", "to")`：单位换算。线性单位经 SI 基准系数换算，
//!   温度（C/F/K/R）走仿射路径。
//!
//! 路由策略：AST 含 `convert` 函数调用时路由至本域。
//! evaluate 处理完整 AST（参照 number_theory.rs 递归结构）：
//! - 算术包围：`convert(100,"cm","m")*2`、`-convert(1,"km","m")` 正确求值
//! - 域内嵌套：`convert(convert(1,"m","cm"),"cm","m")` 正确求值
//! - 跨域函数混入：`sin(1)+convert(...)` 显式报错（消息含函数名）
//! - Str 仅作为 FunctionCall 实参合法；出现在 BinaryOp/UnaryOp 中报错

use crate::core::CalculationDomain;
use crate::core::{AstNode, BinaryOp, CalcError, EvalContext, EvalResult, UnaryOp};
use super::common::{ensure_math_constants, resolve_variable, unsupported_node_error, unsupported_function_error};

use crate::math::unit as math_unit;

/// 单位换算函数白名单。
///
/// `mod`/`abs` 由 parser 将 `x%y`/`abs(x)` 转换为函数调用形式（见 parser.rs），
/// 当它们包围 `convert` 结果时需路由至本域处理（算术包围语义，对齐 design.md D8）。
const UNIT_FUNCTIONS: &[&str] = &["convert", "mod", "abs"];

/// 单位换算域：支持 `convert(value, "from", "to")` 覆盖 8 个量纲。
pub struct UnitDomain;

impl CalculationDomain for UnitDomain {
    fn domain_name(&self) -> &str {
        "unit"
    }

    fn supports(&self, ast: &AstNode) -> bool {
        contains_unit_function(ast)
    }

    fn evaluate(&self, ast: &AstNode, ctx: &EvalContext) -> Result<EvalResult, CalcError> {
        let ctx = ensure_math_constants(ctx);
        self.eval_node(ast, &ctx)
    }

    fn priority(&self) -> u8 {
        30
    }
}

impl Default for UnitDomain {
    fn default() -> Self {
        Self
    }
}

impl UnitDomain {
    /// 递归求值 AST 节点，返回 EvalResult。
    fn eval_node(&self, ast: &AstNode, ctx: &EvalContext) -> Result<EvalResult, CalcError> {
        match ast {
            AstNode::FunctionCall(name, args) => self.eval_function(name, args, ctx),
            AstNode::Number(n) => Ok(EvalResult::Scalar(*n)),
            AstNode::BigNumber(s) => s.parse::<f64>().map(EvalResult::Scalar).map_err(|_| {
                CalcError::domain(format!("invalid big number literal: {}", s)).with_i18n(
                    "msg.invalid_bignumber",
                    vec![("value".to_string(), s.clone())],
                )
            }),
            AstNode::Variable(name) => resolve_variable(ctx, name).map(EvalResult::Scalar),
            AstNode::BinaryOp(op, l, r) => {
                let a = self.eval_scalar(l, ctx)?;
                let b = self.eval_scalar(r, ctx)?;
                self.apply_binary(*op, a, b).map(EvalResult::Scalar)
            }
            AstNode::UnaryOp(op, e) => {
                let v = self.eval_scalar(e, ctx)?;
                match op {
                    UnaryOp::Neg => Ok(EvalResult::Scalar(-v)),
                    UnaryOp::Abs => Ok(EvalResult::Scalar(v.abs())),
                    UnaryOp::Factorial => Err(CalcError::domain(
                        "factorial not supported in unit domain".to_string(),
                    )
                    .with_i18n(
                        "msg.unit.invalid_argument",
                        vec![(
                            "detail".to_string(),
                            "factorial not supported in unit domain".to_string(),
                        )],
                    )),
                }
            }
            // Str 仅作为 FunctionCall 实参合法；此处为操作数上下文 → 报错
            AstNode::Str(_) => Err(CalcError::domain(
                "string operand not supported in unit domain".to_string(),
            )
            .with_i18n(
                "msg.unit.invalid_argument",
                vec![(
                    "detail".to_string(),
                    "string operand not supported in unit domain".to_string(),
                )],
            )),
            AstNode::Complex(_, _) | AstNode::Matrix(_) | AstNode::List(_) => {
                Err(unsupported_node_error("unit", ast))
            }
        }
    }

    /// 将 AST 求值为 f64 标量（用于 BinaryOp/UnaryOp 操作数）。
    fn eval_scalar(&self, ast: &AstNode, ctx: &EvalContext) -> Result<f64, CalcError> {
        match self.eval_node(ast, ctx)? {
            EvalResult::Scalar(v) => Ok(v),
            _ => Err(
                CalcError::domain(format!("expected scalar result from: {:?}", ast)).with_i18n(
                    "msg.unit.invalid_argument",
                    vec![(
                        "detail".to_string(),
                        format!("expected scalar result from: {:?}", ast),
                    )],
                ),
            ),
        }
    }

    /// 二元运算（仅标量）。
    fn apply_binary(&self, op: BinaryOp, a: f64, b: f64) -> Result<f64, CalcError> {
        match op {
            BinaryOp::Add => Ok(a + b),
            BinaryOp::Sub => Ok(a - b),
            BinaryOp::Mul => Ok(a * b),
            BinaryOp::Div => {
                if b == 0.0 {
                    return Err(CalcError::division_by_zero());
                }
                Ok(a / b)
            }
            BinaryOp::Pow => {
                if a == 0.0 && b < 0.0 {
                    return Err(CalcError::domain(
                        "0 cannot be raised to a negative power".to_string(),
                    )
                    .with_i18n(
                        "msg.unit.invalid_argument",
                        vec![(
                            "detail".to_string(),
                            "0 cannot be raised to a negative power".to_string(),
                        )],
                    ));
                }
                Ok(a.powf(b))
            }
            BinaryOp::Mod => {
                if b == 0.0 {
                    return Err(CalcError::division_by_zero());
                }
                Ok(a % b)
            }
        }
    }

    /// 求值函数调用：按函数名分发。
    fn eval_function(
        &self,
        name: &str,
        args: &[AstNode],
        ctx: &EvalContext,
    ) -> Result<EvalResult, CalcError> {
        if !UNIT_FUNCTIONS.contains(&name) {
            return Err(unsupported_function_error("unit", name));
        }
        // UNIT_FUNCTIONS: convert / mod / abs
        match name {
            "convert" => self.eval_convert(args, ctx),
            "mod" => self.eval_mod(args, ctx),
            "abs" => self.eval_abs(args, ctx),
            _ => Err(CalcError::eval(format!("unknown unit function: {}", name)).with_i18n(
                "msg.unknown_function",
                vec![("name".to_string(), name.to_string())],
            )),
        }
    }

    /// `mod(a, b)`：取模函数（parser 将 `a%b` 转换为 `mod(a,b)`）。
    fn eval_mod(&self, args: &[AstNode], ctx: &EvalContext) -> Result<EvalResult, CalcError> {
        if args.len() != 2 {
            return Err(CalcError::domain(format!(
                "mod() requires exactly 2 arguments, got {}",
                args.len()
            ))
            .with_i18n(
                "msg.unit.invalid_argument",
                vec![(
                    "detail".to_string(),
                    format!("mod() requires exactly 2 arguments, got {}", args.len()),
                )],
            ));
        }
        let a = self.eval_scalar(&args[0], ctx)?;
        let b = self.eval_scalar(&args[1], ctx)?;
        self.apply_binary(BinaryOp::Mod, a, b)
            .map(EvalResult::Scalar)
    }

    /// `abs(x)`：绝对值函数（parser 将 `abs(x)` 保留为函数调用形式）。
    fn eval_abs(&self, args: &[AstNode], ctx: &EvalContext) -> Result<EvalResult, CalcError> {
        if args.len() != 1 {
            return Err(CalcError::domain(format!(
                "abs() requires exactly 1 argument, got {}",
                args.len()
            ))
            .with_i18n(
                "msg.unit.invalid_argument",
                vec![(
                    "detail".to_string(),
                    format!("abs() requires exactly 1 argument, got {}", args.len()),
                )],
            ));
        }
        let v = self.eval_scalar(&args[0], ctx)?;
        Ok(EvalResult::Scalar(v.abs()))
    }

    /// `convert(value, "from", "to")`：单位换算。
    ///
    /// 线性单位经 SI 基准系数换算：`value * from_coeff / to_coeff`
    /// 温度（C/F/K/R）走仿射路径：`from_kelvin(to_kelvin(value, from), to)`
    fn eval_convert(&self, args: &[AstNode], ctx: &EvalContext) -> Result<EvalResult, CalcError> {
        if args.len() != 3 {
            return Err(CalcError::domain(format!(
                "convert() requires exactly 3 arguments, got {}",
                args.len()
            ))
            .with_i18n(
                "msg.unit.invalid_argument",
                vec![(
                    "detail".to_string(),
                    format!("convert() requires exactly 3 arguments, got {}", args.len()),
                )],
            ));
        }

        // value 参数：递归求值为标量（支持嵌套表达式）
        let value = self.eval_scalar(&args[0], ctx)?;

        // from / to 参数：必须是 Str 字面量
        let from = match &args[1] {
            AstNode::Str(s) => s.as_str(),
            _ => {
                return Err(CalcError::domain(format!(
                    "convert() requires string argument for 'from' unit, got: {:?}",
                    args[1]
                ))
                .with_i18n(
                    "msg.unit.invalid_argument",
                    vec![(
                        "detail".to_string(),
                        "convert() 'from' unit must be a string literal".to_string(),
                    )],
                ));
            }
        };
        let to = match &args[2] {
            AstNode::Str(s) => s.as_str(),
            _ => {
                return Err(CalcError::domain(format!(
                    "convert() requires string argument for 'to' unit, got: {:?}",
                    args[2]
                ))
                .with_i18n(
                    "msg.unit.invalid_argument",
                    vec![(
                        "detail".to_string(),
                        "convert() 'to' unit must be a string literal".to_string(),
                    )],
                ));
            }
        };

        math_unit::convert_value(value, from, to).map(EvalResult::Scalar)
    }
}


/// 递归检查 AST 是否含 `convert` 函数调用。
fn contains_unit_function(ast: &AstNode) -> bool {
    match ast {
        AstNode::FunctionCall(name, args) => {
            UNIT_FUNCTIONS.contains(&name.as_str()) || args.iter().any(contains_unit_function)
        }
        AstNode::BinaryOp(_, l, r) => contains_unit_function(l) || contains_unit_function(r),
        AstNode::UnaryOp(_, e) => contains_unit_function(e),
        AstNode::Matrix(rows) => rows.iter().flatten().any(contains_unit_function),
        AstNode::List(elements) => elements.iter().any(contains_unit_function),
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

    fn eval(input: &str) -> Result<EvalResult, CalcError> {
        let ast = parse(input).unwrap();
        let domain = UnitDomain;
        let ctx = EvalContext::new();
        domain.evaluate(&ast, &ctx)
    }

    fn eval_scalar(input: &str) -> Result<f64, CalcError> {
        eval(input).map(|r| r.as_scalar().expect("expected scalar result"))
    }

    // ===== 域元信息测试 =====

    #[test]
    fn test_domain_info() {
        let domain = UnitDomain;
        assert_eq!(domain.domain_name(), "unit");
        assert_eq!(domain.priority(), 30);
    }

    #[test]
    fn test_default_impl() {
        let domain = UnitDomain;
        assert_eq!(domain.domain_name(), "unit");
    }

    #[test]
    fn test_supports_convert() {
        let ast = parse(r#"convert(100,"cm","m")"#).unwrap();
        assert!(UnitDomain.supports(&ast));
    }

    #[test]
    fn test_supports_nested_in_binary() {
        let ast = parse(r#"convert(100,"cm","m")*2"#).unwrap();
        assert!(UnitDomain.supports(&ast));
    }

    #[test]
    fn test_supports_unary() {
        let ast = AstNode::UnaryOp(
            UnaryOp::Neg,
            Box::new(parse(r#"convert(1,"km","m")"#).unwrap()),
        );
        assert!(UnitDomain.supports(&ast));
    }

    #[test]
    fn test_not_supports_pure_arithmetic() {
        let ast = parse("1+2").unwrap();
        assert!(!UnitDomain.supports(&ast));
    }

    #[test]
    fn test_not_supports_scientific() {
        let ast = parse("sin(1)").unwrap();
        assert!(!UnitDomain.supports(&ast));
    }

    // ===== R-unit-001: 线性单位换算 =====

    #[test]
    fn test_convert_length_m_to_km() {
        assert!((eval_scalar(r#"convert(1000,"m","km")"#).unwrap() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_convert_length_in_to_cm() {
        assert!((eval_scalar(r#"convert(1,"in","cm")"#).unwrap() - 2.54).abs() < 1e-9);
    }

    #[test]
    fn test_convert_mass_kg_to_lb() {
        assert!((eval_scalar(r#"convert(1,"kg","lb")"#).unwrap() - 2.204623).abs() < 1e-6);
    }

    #[test]
    fn test_convert_mass_t_to_kg() {
        assert!((eval_scalar(r#"convert(1,"t","kg")"#).unwrap() - 1000.0).abs() < 1e-9);
    }

    #[test]
    fn test_convert_volume_l_to_m3() {
        assert!((eval_scalar(r#"convert(1,"L","m3")"#).unwrap() - 0.001).abs() < 1e-12);
    }

    #[test]
    fn test_convert_volume_gal_to_l() {
        assert!((eval_scalar(r#"convert(1,"gal","L")"#).unwrap() - 3.785412).abs() < 1e-5);
    }

    #[test]
    fn test_convert_area_ha_to_m2() {
        assert!((eval_scalar(r#"convert(1,"ha","m2")"#).unwrap() - 10000.0).abs() < 1e-9);
    }

    #[test]
    fn test_convert_speed_mps_to_kmh() {
        assert!((eval_scalar(r#"convert(1,"m/s","km/h")"#).unwrap() - 3.6).abs() < 1e-9);
    }

    #[test]
    fn test_convert_data_gib_to_b() {
        assert!((eval_scalar(r#"convert(1,"GiB","B")"#).unwrap() - 1073741824.0).abs() < 1e-3);
    }

    #[test]
    fn test_convert_data_kb_to_b() {
        assert!((eval_scalar(r#"convert(1,"KB","B")"#).unwrap() - 1000.0).abs() < 1e-9);
    }

    #[test]
    fn test_convert_time_h_to_s() {
        assert!((eval_scalar(r#"convert(1,"h","s")"#).unwrap() - 3600.0).abs() < 1e-9);
    }

    #[test]
    fn test_convert_time_week_to_s() {
        assert!((eval_scalar(r#"convert(1,"week","s")"#).unwrap() - 604800.0).abs() < 1e-9);
    }

    #[test]
    fn test_convert_identity() {
        // 同单位换算恒等：convert(x,"m","m") = x
        assert!((eval_scalar(r#"convert(42,"m","m")"#).unwrap() - 42.0).abs() < 1e-9);
    }

    // ===== R-unit-002: 温度仿射换算 =====

    #[test]
    fn test_convert_temperature_c_to_f_100() {
        assert!((eval_scalar(r#"convert(100,"C","F")"#).unwrap() - 212.0).abs() < 1e-9);
    }

    #[test]
    fn test_convert_temperature_c_to_f_0() {
        assert!((eval_scalar(r#"convert(0,"C","F")"#).unwrap() - 32.0).abs() < 1e-9);
    }

    #[test]
    fn test_convert_temperature_k_to_c() {
        assert!((eval_scalar(r#"convert(0,"K","C")"#).unwrap() - (-273.15)).abs() < 1e-9);
    }

    #[test]
    fn test_convert_temperature_r_to_k() {
        assert!((eval_scalar(r#"convert(0,"R","K")"#).unwrap() - 0.0).abs() < 1e-9);
    }

    #[test]
    fn test_convert_temperature_c_to_c_identity() {
        assert!((eval_scalar(r#"convert(50,"C","C")"#).unwrap() - 50.0).abs() < 1e-9);
    }

    #[test]
    fn test_convert_temperature_k_to_f_roundtrip() {
        // 300K → F：300-273.15=26.85°C → 26.85*9/5+32 = 80.33°F
        assert!((eval_scalar(r#"convert(300,"K","F")"#).unwrap() - 80.33).abs() < 1e-9);
    }

    // ===== R-unit-003: 错误显性化 =====

    #[test]
    fn test_convert_dimension_mismatch() {
        let result = eval(r#"convert(1,"m","kg")"#);
        let err = result.expect_err("expected error");
        assert_eq!(err.kind, ErrorKind::Domain);
        // 消息含双方量纲名（Length / Mass）
        assert!(err.message.contains("Length"), "msg: {}", err.message);
        assert!(err.message.contains("Mass"), "msg: {}", err.message);
        // i18n key
        assert_eq!(err.i18n_key, Some("msg.unit.dimension_mismatch"));
    }

    #[test]
    fn test_convert_dimension_mismatch_temperature_to_linear() {
        // 温度与其他量纲混换（如 C→m）→ 量纲不匹配错误
        let result = eval(r#"convert(100,"C","m")"#);
        let err = result.expect_err("expected error");
        assert_eq!(err.kind, ErrorKind::Domain);
        assert!(err.message.contains("Temperature"));
        assert!(err.message.contains("Length"));
    }

    #[test]
    fn test_convert_unknown_unit_with_suggestion() {
        // "metre" 未收录，应建议 "meter"/"metre"（其实 metre 在表中，换个未收录的）
        // 用 "metr" 应建议 "meter"/"metre"（距离 1）
        let result = eval(r#"convert(1,"metr","m")"#);
        let err = result.expect_err("expected error");
        assert_eq!(err.kind, ErrorKind::Domain);
        // 消息含 "did you mean" 建议且包含相近单位
        assert!(err.message.contains("did you mean"), "msg: {}", err.message);
        // 应建议 meter 或 metre（距离 1）
        assert!(
            err.message.contains("meter") || err.message.contains("metre"),
            "msg: {}",
            err.message
        );
        assert_eq!(err.i18n_key, Some("msg.unit.unknown_unit"));
    }

    #[test]
    fn test_convert_unknown_unit_no_suggestion() {
        // 完全无相近单位的未知单位
        let result = eval(r#"convert(1,"XYZABC","m")"#);
        let err = result.expect_err("expected error");
        assert_eq!(err.kind, ErrorKind::Domain);
        assert!(err.message.contains("XYZABC"));
        assert!(
            !err.message.contains("did you mean"),
            "should not have suggestions: {}",
            err.message
        );
    }

    #[test]
    fn test_convert_case_sensitive_unknown() {
        // "mB" 不在表中（与 "MB" 不同）
        let result = eval(r#"convert(1,"mB","KB")"#);
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_convert_non_string_from_arg() {
        // 单位参数非 Str → CalcError::domain
        let result = eval(r#"convert(1, 2, "m")"#);
        let err = result.expect_err("expected error");
        assert_eq!(err.kind, ErrorKind::Domain);
        assert_eq!(err.i18n_key, Some("msg.unit.invalid_argument"));
    }

    #[test]
    fn test_convert_non_string_to_arg() {
        let result = eval(r#"convert(1, "m", 3)"#);
        let err = result.expect_err("expected error");
        assert_eq!(err.kind, ErrorKind::Domain);
        assert_eq!(err.i18n_key, Some("msg.unit.invalid_argument"));
    }

    #[test]
    fn test_convert_wrong_arg_count() {
        let ast = AstNode::FunctionCall(
            "convert".to_string(),
            vec![AstNode::Number(1.0), AstNode::Str("m".to_string())],
        );
        let result = UnitDomain.evaluate(&ast, &EvalContext::new());
        let err = result.expect_err("expected error");
        assert_eq!(err.kind, ErrorKind::Domain);
        assert_eq!(err.i18n_key, Some("msg.unit.invalid_argument"));
    }

    // ===== R-unit-004: 算术包围与跨域函数混入 =====

    #[test]
    fn test_arithmetic_wrapping_mul() {
        // convert(100,"cm","m")*2 = 1*2 = 2
        assert!((eval_scalar(r#"convert(100,"cm","m")*2"#).unwrap() - 2.0).abs() < 1e-9);
    }

    #[test]
    fn test_arithmetic_wrapping_add() {
        // convert(1,"km","m")+1 = 1000+1 = 1001
        assert!((eval_scalar(r#"convert(1,"km","m")+1"#).unwrap() - 1001.0).abs() < 1e-9);
    }

    #[test]
    fn test_arithmetic_wrapping_unary_neg() {
        // -convert(1,"km","m") = -1000
        assert!((eval_scalar(r#"-convert(1,"km","m")"#).unwrap() - (-1000.0)).abs() < 1e-9);
    }

    #[test]
    fn test_arithmetic_wrapping_unary_abs() {
        // abs(-convert(1,"km","m")) = 1000
        assert!((eval_scalar(r#"abs(-convert(1,"km","m"))"#).unwrap() - 1000.0).abs() < 1e-9);
    }

    #[test]
    fn test_nested_convert() {
        // convert(convert(1,"m","cm"),"cm","m") = convert(100,"cm","m") = 1
        assert!(
            (eval_scalar(r#"convert(convert(1,"m","cm"),"cm","m")"#).unwrap() - 1.0).abs() < 1e-9
        );
    }

    #[test]
    fn test_cross_domain_function_rejected() {
        // sin(1)+convert(1,"km","m") → DomainError 含 "sin"
        let result = eval(r#"sin(1)+convert(1,"km","m")"#);
        let err = result.expect_err("expected error");
        assert_eq!(err.kind, ErrorKind::Domain);
        assert!(err.message.contains("sin"), "msg: {}", err.message);
        assert_eq!(err.i18n_key, Some("msg.domain.unsupported_function"));
    }

    #[test]
    fn test_cross_domain_function_alone_rejected() {
        // 单独的 sin(1) 在 unit 域 evaluate 中也应被拒绝
        // 但 supports() 不会路由到 unit（因 AST 不含 convert）
        // 此测试验证 evaluate 直接调用时的行为
        let ast = AstNode::FunctionCall("sin".to_string(), vec![AstNode::Number(1.0)]);
        let result = UnitDomain.evaluate(&ast, &EvalContext::new());
        let err = result.expect_err("expected error");
        assert_eq!(err.kind, ErrorKind::Domain);
        assert!(err.message.contains("sin"));
    }

    #[test]
    fn test_unknown_function_in_evaluate() {
        let ast = AstNode::FunctionCall("foo_bar".to_string(), vec![AstNode::Number(1.0)]);
        let result = UnitDomain.evaluate(&ast, &EvalContext::new());
        let err = result.expect_err("expected error");
        assert_eq!(err.kind, ErrorKind::Domain);
        assert!(err.message.contains("foo_bar"));
    }

    // ===== Str 操作数测试 =====

    #[test]
    fn test_str_in_binary_op_rejected() {
        // 1 + "a" → Str 出现在 BinaryOp 中 → CalcError::domain
        let ast = AstNode::BinaryOp(
            BinaryOp::Add,
            Box::new(AstNode::Number(1.0)),
            Box::new(AstNode::Str("a".to_string())),
        );
        let result = UnitDomain.evaluate(&ast, &EvalContext::new());
        let err = result.expect_err("expected error");
        assert_eq!(err.kind, ErrorKind::Domain);
        assert_eq!(err.i18n_key, Some("msg.unit.invalid_argument"));
    }

    #[test]
    fn test_str_in_unary_op_rejected() {
        let ast = AstNode::UnaryOp(UnaryOp::Neg, Box::new(AstNode::Str("a".to_string())));
        let result = UnitDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_str_alone_rejected() {
        let ast = AstNode::Str("hello".to_string());
        let result = UnitDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    // ===== 不支持的节点类型 =====

    #[test]
    fn test_complex_rejected() {
        let ast = AstNode::Complex(1.0, 2.0);
        let result = UnitDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_matrix_rejected() {
        let ast = AstNode::Matrix(vec![vec![AstNode::Number(1.0)]]);
        let result = UnitDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_list_rejected() {
        let ast = AstNode::List(vec![AstNode::Number(1.0)]);
        let result = UnitDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    // ===== 变量与常量 =====

    #[test]
    fn test_variable_lookup() {
        let ctx = EvalContext::new().with_var("x", 100.0);
        let ast = parse(r#"convert(x,"cm","m")"#).unwrap();
        let result = UnitDomain.evaluate(&ast, &ctx).unwrap();
        assert!((result.as_scalar().unwrap() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_pi_constant() {
        // convert(pi,"m","m") = pi（恒等换算）
        let result = eval_scalar(r#"convert(pi,"m","m")"#).unwrap();
        assert!((result - std::f64::consts::PI).abs() < 1e-9);
    }

    #[test]
    fn test_e_constant() {
        let result = eval_scalar(r#"convert(e,"m","m")"#).unwrap();
        assert!((result - std::f64::consts::E).abs() < 1e-9);
    }

    #[test]
    fn test_unbound_variable() {
        let result = eval(r#"convert(x,"m","m")"#);
        let err = result.expect_err("expected error");
        assert_eq!(err.kind, ErrorKind::Eval);
        assert_eq!(err.i18n_key, Some("msg.unbound_variable"));
    }

    // ===== 二元运算错误路径 =====

    #[test]
    fn test_division_by_zero() {
        let ast = AstNode::BinaryOp(
            BinaryOp::Div,
            Box::new(parse(r#"convert(1,"m","m")"#).unwrap()),
            Box::new(AstNode::Number(0.0)),
        );
        let result = UnitDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::DivisionByZero));
    }

    #[test]
    fn test_mod_by_zero() {
        let ast = AstNode::BinaryOp(
            BinaryOp::Mod,
            Box::new(parse(r#"convert(1,"m","m")"#).unwrap()),
            Box::new(AstNode::Number(0.0)),
        );
        let result = UnitDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::DivisionByZero));
    }

    #[test]
    fn test_zero_to_negative_power() {
        let ast = AstNode::BinaryOp(
            BinaryOp::Pow,
            Box::new(AstNode::Number(0.0)),
            Box::new(AstNode::Number(-1.0)),
        );
        let result = UnitDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_factorial_unary_rejected() {
        let ast = AstNode::UnaryOp(UnaryOp::Factorial, Box::new(AstNode::Number(5.0)));
        let result = UnitDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    // ===== BigNumber 支持 =====

    #[test]
    fn test_bignumber_arg() {
        // 大数作为 convert 的 value 参数
        let ast = AstNode::FunctionCall(
            "convert".to_string(),
            vec![
                AstNode::BigNumber("1000".to_string()),
                AstNode::Str("m".to_string()),
                AstNode::Str("km".to_string()),
            ],
        );
        let result = UnitDomain.evaluate(&ast, &EvalContext::new()).unwrap();
        assert!((result.as_scalar().unwrap() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_invalid_bignumber() {
        let ast = AstNode::BigNumber("not_a_number".to_string());
        let result = UnitDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    // ===== 幂运算场景 =====

    #[test]
    fn test_power_operation() {
        // convert(1,"m","m")^2 = 1
        assert!((eval_scalar(r#"convert(1,"m","m")^2"#).unwrap() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_subtraction() {
        // convert(1,"km","m") - convert(500,"m","m") = 1000 - 500 = 500
        assert!(
            (eval_scalar(r#"convert(1,"km","m")-convert(500,"m","m")"#).unwrap() - 500.0).abs()
                < 1e-9
        );
    }

    #[test]
    fn test_division() {
        // convert(1,"km","m") / 2 = 500
        assert!((eval_scalar(r#"convert(1,"km","m")/2"#).unwrap() - 500.0).abs() < 1e-9);
    }

    #[test]
    fn test_modulo() {
        // convert(1010,"m","m") % 1000 = 10
        assert!((eval_scalar(r#"convert(1010,"m","m")%1000"#).unwrap() - 10.0).abs() < 1e-9);
    }

    // ===== 错误消息细节验证 =====

    #[test]
    fn test_unknown_unit_error_includes_unit_name() {
        let result = eval(r#"convert(1,"ZZZZ","m")"#);
        let err = result.expect_err("expected error");
        assert!(err.message.contains("ZZZZ"));
        assert_eq!(err.i18n_key, Some("msg.unit.unknown_unit"));
        assert_eq!(
            err.i18n_args,
            vec![("unit".to_string(), "ZZZZ".to_string())]
        );
    }

    #[test]
    fn test_dimension_mismatch_error_args() {
        let result = eval(r#"convert(1,"m","s")"#);
        let err = result.expect_err("expected error");
        // Length vs Time
        assert!(err.message.contains("Length"));
        assert!(err.message.contains("Time"));
        assert_eq!(err.i18n_key, Some("msg.unit.dimension_mismatch"));
        let arg_keys: Vec<&str> = err.i18n_args.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(arg_keys, vec!["from", "from_dim", "to", "to_dim"]);
    }

    // ===== supports 矩阵/列表场景 =====

    #[test]
    fn test_supports_matrix_with_convert() {
        let ast = AstNode::Matrix(vec![vec![parse(r#"convert(1,"m","m")"#).unwrap()]]);
        assert!(UnitDomain.supports(&ast));
    }

    #[test]
    fn test_supports_list_with_convert() {
        let ast = AstNode::List(vec![parse(r#"convert(1,"m","m")"#).unwrap()]);
        assert!(UnitDomain.supports(&ast));
    }

    // ===== 8 量纲端到端覆盖（spec R-unit-001 全部验收点）=====

    #[test]
    fn test_all_8_dimensions_end_to_end() {
        // Length
        assert!((eval_scalar(r#"convert(1000,"m","km")"#).unwrap() - 1.0).abs() < 1e-9);
        // Mass
        assert!((eval_scalar(r#"convert(1,"t","kg")"#).unwrap() - 1000.0).abs() < 1e-9);
        // Temperature
        assert!((eval_scalar(r#"convert(100,"C","F")"#).unwrap() - 212.0).abs() < 1e-9);
        // Volume
        assert!((eval_scalar(r#"convert(1,"L","m3")"#).unwrap() - 0.001).abs() < 1e-12);
        // Area
        assert!((eval_scalar(r#"convert(1,"ha","m2")"#).unwrap() - 10000.0).abs() < 1e-9);
        // Speed
        assert!((eval_scalar(r#"convert(1,"m/s","km/h")"#).unwrap() - 3.6).abs() < 1e-9);
        // Data
        assert!((eval_scalar(r#"convert(1,"GiB","B")"#).unwrap() - 1073741824.0).abs() < 1e-3);
        // Time
        assert!((eval_scalar(r#"convert(1,"h","s")"#).unwrap() - 3600.0).abs() < 1e-9);
    }

    // ===== 底层函数单元测试已迁移至 math::unit =====

    #[test]
    fn test_contains_unit_function_str_returns_false() {
        let ast = AstNode::Str("convert".to_string());
        assert!(!contains_unit_function(&ast));
    }

    #[test]
    fn test_contains_unit_function_number_returns_false() {
        let ast = AstNode::Number(1.0);
        assert!(!contains_unit_function(&ast));
    }

    #[test]
    fn test_contains_unit_function_complex_returns_false() {
        let ast = AstNode::Complex(1.0, 2.0);
        assert!(!contains_unit_function(&ast));
    }

    #[test]
    fn test_contains_unit_function_bignumber_returns_false() {
        let ast = AstNode::BigNumber("123".to_string());
        assert!(!contains_unit_function(&ast));
    }
}
