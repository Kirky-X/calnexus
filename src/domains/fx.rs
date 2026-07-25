// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! 汇率换算域：fx/fx_rate 函数 + frankfurter.dev 在线 API。
//!
//! 设计依据：design.md D6（Provider trait 注入 + 三级缓存 + stale 策略）+ D8（混合表达式求值语义）
//! Feature 门控：`fx = ["dep:ureq", "dep:dirs"]`
//!
//! 函数表：
//! - `fx(value, "FROM", "TO")`：换算金额，经 base 三角计算 `value / rate[FROM] * rate[TO]`
//! - `fx_rate("FROM", "TO")`：查询汇率，等价于 `fx(1, FROM, TO)`
//!
//! 路由策略：AST 含 fx/fx_rate 函数调用时路由至本域。
//! evaluate 处理完整 AST（参照 number_theory.rs / unit.rs 递归结构）：
//! - 算术包围：`fx(100,"USD","CNY")+1`、`-fx(...)` 正确求值
//! - 域内嵌套：`fx(fx(100,"USD","EUR"),"EUR","CNY")` 正确求值
//! - 跨域函数混入：`sin(1)+fx(...)` 显式报错（消息含函数名）
//! - Str 仅作为 FunctionCall 实参合法；出现在 BinaryOp/UnaryOp 中报错
//!
//! 非确定性：fx/fx_rate 申报为非确定性函数（D3），永不进 L1 缓存。

use crate::core::CalculationDomain;
use crate::core::{AstNode, BinaryOp, CalcError, EvalContext, EvalResult, UnaryOp};

use super::fx_provider::{FrankfurterProvider, RateProvider, RateTable};

/// fx 域函数白名单（用于路由 supports() 和非确定性申报）。
///
/// 仅 fx/fx_rate 触发路由至本域；mod/abs 由 parser 从 `a%b`/`abs(x)` 转换而来，
/// 仅在算术包围 fx 结果时随父表达式进入本域（不在 contains_fx_function 中识别，
/// 避免误路由纯 `mod(1,2)` 表达式）。
const FX_FUNCTIONS: &[&str] = &["fx", "fx_rate"];

/// 求值阶段支持的函数白名单（含算术包围函数 mod/abs）。
///
/// 与 FX_FUNCTIONS 的区别：本表用于 eval_function 分发，额外包含 mod/abs，
/// 以处理 `fx(...)%1000`（解析为 `mod(fx(...), 1000)`）等算术包围表达式。
const FX_EVAL_FUNCTIONS: &[&str] = &["fx", "fx_rate", "mod", "abs"];

/// 汇率换算域：支持 `fx(value, "FROM", "TO")` 和 `fx_rate("FROM", "TO")`。
///
/// 构造函数 `new(provider)` 接受 `Box<dyn RateProvider>` 注入（R-fx-004）：
/// 测试套件用 mock provider，CI 断网环境全绿；生产用 `default()` 注入
/// FrankfurterProvider。
pub struct FxDomain {
    provider: Box<dyn RateProvider>,
}

impl FxDomain {
    /// 注入指定 provider 构造 FxDomain（测试入口，避免出网）。
    pub(crate) fn new(provider: Box<dyn RateProvider>) -> Self {
        Self { provider }
    }
}

impl Default for FxDomain {
    fn default() -> Self {
        Self::new(Box::new(FrankfurterProvider::new()))
    }
}

impl CalculationDomain for FxDomain {
    fn domain_name(&self) -> &str {
        "fx"
    }

    fn supports(&self, ast: &AstNode) -> bool {
        contains_fx_function(ast)
    }

    fn evaluate(&self, ast: &AstNode, ctx: &EvalContext) -> Result<EvalResult, CalcError> {
        eval_with_provider(ast, ctx, self.provider.as_ref())
    }

    fn priority(&self) -> u8 {
        30
    }

    fn nondeterministic_functions(&self) -> &'static [&'static str] {
        FX_FUNCTIONS
    }
}

/// 递归求值 AST 节点（使用指定 provider）。
///
/// 测试入口：直接传入 mock provider，避免出网。
fn eval_with_provider(
    ast: &AstNode,
    ctx: &EvalContext,
    provider: &dyn RateProvider,
) -> Result<EvalResult, CalcError> {
    match ast {
        AstNode::FunctionCall(name, args) => eval_function(name, args, ctx, provider),
        AstNode::Number(n) => Ok(EvalResult::Scalar(*n)),
        AstNode::BigNumber(s) => s.parse::<f64>().map(EvalResult::Scalar).map_err(|_| {
            CalcError::domain(format!("invalid big number literal: {}", s)).with_i18n(
                "msg.invalid_bignumber",
                vec![("value".to_string(), s.clone())],
            )
        }),
        AstNode::Variable(name) => {
            if let Some(v) = ctx.get_var(name) {
                Ok(EvalResult::Scalar(v))
            } else if name == "pi" {
                Ok(EvalResult::Scalar(std::f64::consts::PI))
            } else if name == "e" {
                Ok(EvalResult::Scalar(std::f64::consts::E))
            } else {
                Err(
                    CalcError::eval(format!("unbound variable: {}", name)).with_i18n(
                        "msg.unbound_variable",
                        vec![("name".to_string(), name.clone())],
                    ),
                )
            }
        }
        AstNode::BinaryOp(op, l, r) => {
            let a = eval_scalar(l, ctx, provider)?;
            let b = eval_scalar(r, ctx, provider)?;
            apply_binary(*op, a, b).map(EvalResult::Scalar)
        }
        AstNode::UnaryOp(op, e) => {
            let v = eval_scalar(e, ctx, provider)?;
            match op {
                UnaryOp::Neg => Ok(EvalResult::Scalar(-v)),
                UnaryOp::Abs => Ok(EvalResult::Scalar(v.abs())),
                UnaryOp::Factorial => Err(CalcError::domain(
                    "factorial not supported in fx domain".to_string(),
                )
                .with_i18n("msg.fx.factorial_not_supported", vec![])),
            }
        }
        // Str 仅作为 FunctionCall 实参合法；此处为操作数上下文 → 报错
        AstNode::Str(_) => Err(CalcError::domain(
            "string operand not supported in fx domain".to_string(),
        )
        .with_i18n("msg.fx.string_operand_not_supported", vec![])),
        AstNode::Complex(_, _) | AstNode::Matrix(_) | AstNode::List(_) => Err(CalcError::domain(
            format!("fx domain does not support this node type: {:?}", ast),
        )
        .with_i18n(
            "msg.fx.unsupported_node",
            vec![("node".to_string(), format!("{:?}", ast))],
        )),
    }
}

/// 将 AST 求值为 f64 标量（用于 BinaryOp/UnaryOp 操作数）。
fn eval_scalar(
    ast: &AstNode,
    ctx: &EvalContext,
    provider: &dyn RateProvider,
) -> Result<f64, CalcError> {
    match eval_with_provider(ast, ctx, provider)? {
        EvalResult::Scalar(v) => Ok(v),
        _ => Err(CalcError::domain(format!(
            "expected scalar result from: {:?}",
            ast
        ))
        .with_i18n(
            "msg.fx.expected_scalar",
            vec![("node".to_string(), format!("{:?}", ast))],
        )),
    }
}

/// 二元运算（仅标量）。
fn apply_binary(op: BinaryOp, a: f64, b: f64) -> Result<f64, CalcError> {
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
                .with_i18n("msg.fx.zero_negative_power", vec![]));
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
    name: &str,
    args: &[AstNode],
    ctx: &EvalContext,
    provider: &dyn RateProvider,
) -> Result<EvalResult, CalcError> {
    if !FX_EVAL_FUNCTIONS.contains(&name) {
        return Err(
            CalcError::domain(format!("unsupported function in fx domain: {}", name)).with_i18n(
                "msg.unknown_function",
                vec![("name".to_string(), name.to_string())],
            ),
        );
    }
    match name {
        "fx" => eval_fx(args, ctx, provider),
        "fx_rate" => eval_fx_rate(args, ctx, provider),
        "mod" => eval_mod(args, ctx, provider),
        "abs" => eval_abs(args, ctx, provider),
        _ => unreachable!(),
    }
}

/// `mod(a, b)`：取模函数（parser 将 `a%b` 转换为 `mod(a,b)`）。
///
/// 当 `fx(...)%1000` 等表达式被路由至本域时，需支持 mod 分发。
fn eval_mod(
    args: &[AstNode],
    ctx: &EvalContext,
    provider: &dyn RateProvider,
) -> Result<EvalResult, CalcError> {
    if args.len() != 2 {
        return Err(CalcError::domain(format!(
            "mod() requires exactly 2 arguments, got {}",
            args.len()
        ))
        .with_i18n(
            "msg.function_arg_count",
            vec![
                ("name".to_string(), "mod".to_string()),
                ("expected".to_string(), "2".to_string()),
                ("actual".to_string(), args.len().to_string()),
            ],
        ));
    }
    let a = eval_scalar(&args[0], ctx, provider)?;
    let b = eval_scalar(&args[1], ctx, provider)?;
    apply_binary(BinaryOp::Mod, a, b).map(EvalResult::Scalar)
}

/// `abs(x)`：绝对值函数（parser 将 `abs(x)` 保留为函数调用形式）。
///
/// 当 `abs(-fx(...))` 等表达式被路由至本域时，需支持 abs 分发。
fn eval_abs(
    args: &[AstNode],
    ctx: &EvalContext,
    provider: &dyn RateProvider,
) -> Result<EvalResult, CalcError> {
    if args.len() != 1 {
        return Err(CalcError::domain(format!(
            "abs() requires exactly 1 argument, got {}",
            args.len()
        ))
        .with_i18n(
            "msg.function_arg_count",
            vec![
                ("name".to_string(), "abs".to_string()),
                ("expected".to_string(), "1".to_string()),
                ("actual".to_string(), args.len().to_string()),
            ],
        ));
    }
    let v = eval_scalar(&args[0], ctx, provider)?;
    Ok(EvalResult::Scalar(v.abs()))
}

/// `fx(value, "FROM", "TO")`：换算金额。
///
/// 经 base 三角计算：`value / rate[FROM] * rate[TO]`。
/// 未知币种 → CalcError::domain（含支持币种数量提示）。
fn eval_fx(
    args: &[AstNode],
    ctx: &EvalContext,
    provider: &dyn RateProvider,
) -> Result<EvalResult, CalcError> {
    if args.len() != 3 {
        return Err(CalcError::domain(format!(
            "fx() requires exactly 3 arguments (value, from, to), got {}",
            args.len()
        ))
        .with_i18n(
            "msg.function_arg_count",
            vec![
                ("name".to_string(), "fx".to_string()),
                ("expected".to_string(), "3".to_string()),
                ("actual".to_string(), args.len().to_string()),
            ],
        ));
    }

    let value = eval_scalar(&args[0], ctx, provider)?;
    let from = expect_str_arg(&args[1], "from")?;
    let to = expect_str_arg(&args[2], "to")?;

    let table = provider.rates()?;
    let result = convert(value, from, to, &table)?;
    Ok(EvalResult::Scalar(result))
}

/// `fx_rate("FROM", "TO")`：查询汇率，等价于 `fx(1, FROM, TO)`。
fn eval_fx_rate(
    args: &[AstNode],
    _ctx: &EvalContext,
    provider: &dyn RateProvider,
) -> Result<EvalResult, CalcError> {
    if args.len() != 2 {
        return Err(CalcError::domain(format!(
            "fx_rate() requires exactly 2 arguments (from, to), got {}",
            args.len()
        ))
        .with_i18n(
            "msg.function_arg_count",
            vec![
                ("name".to_string(), "fx_rate".to_string()),
                ("expected".to_string(), "2".to_string()),
                ("actual".to_string(), args.len().to_string()),
            ],
        ));
    }

    let from = expect_str_arg(&args[0], "from")?;
    let to = expect_str_arg(&args[1], "to")?;

    let table = provider.rates()?;
    let result = convert(1.0, from, to, &table)?;
    Ok(EvalResult::Scalar(result))
}

/// 期望 AST 节点为 Str 字面量，返回其字符串内容。否则返回 DomainError。
fn expect_str_arg<'a>(arg: &'a AstNode, param_name: &str) -> Result<&'a str, CalcError> {
    match arg {
        AstNode::Str(s) => Ok(s.as_str()),
        _ => Err(CalcError::domain(format!(
            "fx() requires string argument for '{}', got: {:?}",
            param_name, arg
        ))
        .with_i18n(
            "msg.fx.requires_string_arg",
            vec![
                ("param".to_string(), param_name.to_string()),
                ("node".to_string(), format!("{:?}", arg)),
            ],
        )),
    }
}

/// 汇率换算核心逻辑：经 base 三角计算。
///
/// - 同币种（from == to）→ 恒等返回 value（不查表）
/// - base 币种汇率视为 1.0（如 EUR 在 EUR-base 表中不出现）
/// - 未知币种 → DomainError（含支持币种数量提示，i18n msg.fx.unknown_currency）
fn convert(value: f64, from: &str, to: &str, table: &RateTable) -> Result<f64, CalcError> {
    // 同币种恒等（避免不必要的查表与除零风险）
    if from == to {
        return Ok(value);
    }

    let rate_from = get_rate(from, table)?;
    let rate_to = get_rate(to, table)?;

    // 三角换算：value / rate[FROM] * rate[TO]
    Ok(value / rate_from * rate_to)
}

/// 获取币种对 base 的汇率。base 币种返回 1.0。
fn get_rate(code: &str, table: &RateTable) -> Result<f64, CalcError> {
    if code == table.base {
        return Ok(1.0);
    }
    table
        .rates
        .get(code)
        .copied()
        .ok_or_else(|| unknown_currency_error(code, table))
}

/// 构造"未知币种"错误，消息含支持币种数量。
fn unknown_currency_error(code: &str, table: &RateTable) -> CalcError {
    CalcError::domain(format!(
        "unknown currency: {}, {} currencies supported (base: {})",
        code,
        table.rates.len(),
        table.base
    ))
    .with_i18n(
        "msg.fx.unknown_currency",
        vec![("code".to_string(), code.to_string())],
    )
}

/// 递归检查 AST 是否含 fx/fx_rate 函数调用。
fn contains_fx_function(ast: &AstNode) -> bool {
    match ast {
        AstNode::FunctionCall(name, args) => {
            FX_FUNCTIONS.contains(&name.as_str()) || args.iter().any(contains_fx_function)
        }
        AstNode::BinaryOp(_, l, r) => contains_fx_function(l) || contains_fx_function(r),
        AstNode::UnaryOp(_, e) => contains_fx_function(e),
        AstNode::Matrix(rows) => rows.iter().flatten().any(contains_fx_function),
        AstNode::List(elements) => elements.iter().any(contains_fx_function),
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
    use std::collections::HashMap;

    /// 测试用 mock provider：返回预设的汇率表（不出网）。
    struct MockRateProvider {
        rates: HashMap<String, f64>,
        date: String,
        base: String,
    }

    impl RateProvider for MockRateProvider {
        fn rates(&self) -> Result<RateTable, CalcError> {
            Ok(RateTable {
                base: self.base.clone(),
                date: self.date.clone(),
                rates: self.rates.clone(),
            })
        }
    }

    /// 默认 mock 汇率表：EUR-base，含 USD/CNY/GBP/JPY。
    fn mock_provider() -> MockRateProvider {
        let mut rates = HashMap::new();
        rates.insert("USD".to_string(), 1.08);
        rates.insert("CNY".to_string(), 7.85);
        rates.insert("GBP".to_string(), 0.85);
        rates.insert("JPY".to_string(), 165.0);
        MockRateProvider {
            rates,
            date: "2026-07-25".to_string(),
            base: "EUR".to_string(),
        }
    }

    /// 使用 mock provider 求值表达式。
    fn eval(input: &str) -> Result<EvalResult, CalcError> {
        let ast = parse(input).unwrap();
        let ctx = EvalContext::new();
        let provider = mock_provider();
        eval_with_provider(&ast, &ctx, &provider)
    }

    /// 使用 mock provider 求值并提取标量。
    fn eval_scalar(input: &str) -> Result<f64, CalcError> {
        eval(input).map(|r| r.as_scalar().expect("expected scalar result"))
    }

    // ===== 域元信息测试 =====

    #[test]
    fn test_domain_info() {
        let domain = FxDomain::new(Box::new(mock_provider()));
        assert_eq!(domain.domain_name(), "fx");
        assert_eq!(domain.priority(), 30);
    }

    #[test]
    fn test_default_impl() {
        let domain = FxDomain::default();
        assert_eq!(domain.domain_name(), "fx");
    }

    #[test]
    fn test_nondeterministic_functions() {
        let domain = FxDomain::new(Box::new(mock_provider()));
        assert_eq!(domain.nondeterministic_functions(), &["fx", "fx_rate"]);
    }

    #[test]
    fn test_supports_fx() {
        let ast = parse(r#"fx(100,"USD","CNY")"#).unwrap();
        assert!(FxDomain::default().supports(&ast));
    }

    #[test]
    fn test_supports_fx_rate() {
        let ast = parse(r#"fx_rate("USD","CNY")"#).unwrap();
        assert!(FxDomain::default().supports(&ast));
    }

    #[test]
    fn test_supports_nested_in_binary() {
        let ast = parse(r#"fx(100,"USD","CNY")+1"#).unwrap();
        assert!(FxDomain::default().supports(&ast));
    }

    #[test]
    fn test_supports_unary() {
        let ast = AstNode::UnaryOp(
            UnaryOp::Neg,
            Box::new(parse(r#"fx(1,"USD","EUR")"#).unwrap()),
        );
        assert!(FxDomain::default().supports(&ast));
    }

    #[test]
    fn test_not_supports_pure_arithmetic() {
        let ast = parse("1+2").unwrap();
        assert!(!FxDomain::default().supports(&ast));
    }

    #[test]
    fn test_not_supports_scientific() {
        let ast = parse("sin(1)").unwrap();
        assert!(!FxDomain::default().supports(&ast));
    }

    #[test]
    fn test_supports_matrix_with_fx() {
        let ast = AstNode::Matrix(vec![vec![parse(r#"fx(1,"USD","EUR")"#).unwrap()]]);
        assert!(FxDomain::default().supports(&ast));
    }

    #[test]
    fn test_supports_list_with_fx() {
        let ast = AstNode::List(vec![parse(r#"fx(1,"USD","EUR")"#).unwrap()]);
        assert!(FxDomain::default().supports(&ast));
    }

    // ===== R-fx-001: fx() 换算 =====

    #[test]
    fn test_fx_usd_to_cny() {
        // fx(100, "USD", "CNY") = 100 / 1.08 * 7.85
        let result = eval_scalar(r#"fx(100,"USD","CNY")"#).unwrap();
        let expected = 100.0 / 1.08 * 7.85;
        assert!(
            (result - expected).abs() < 1e-9,
            "got {}, expected {}",
            result,
            expected
        );
    }

    #[test]
    fn test_fx_same_currency_identity() {
        // fx(1, "USD", "USD") = 1（同币种恒等）
        let result = eval_scalar(r#"fx(1,"USD","USD")"#).unwrap();
        assert!((result - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_fx_same_currency_preserves_value() {
        // fx(42, "USD", "USD") = 42
        let result = eval_scalar(r#"fx(42,"USD","USD")"#).unwrap();
        assert!((result - 42.0).abs() < 1e-9);
    }

    #[test]
    fn test_fx_base_currency_from() {
        // fx(100, "EUR", "USD") = 100 / 1.0 * 1.08 = 108
        let result = eval_scalar(r#"fx(100,"EUR","USD")"#).unwrap();
        assert!((result - 108.0).abs() < 1e-9);
    }

    #[test]
    fn test_fx_base_currency_to() {
        // fx(108, "USD", "EUR") = 108 / 1.08 * 1.0 = 100
        let result = eval_scalar(r#"fx(108,"USD","EUR")"#).unwrap();
        assert!((result - 100.0).abs() < 1e-9);
    }

    #[test]
    fn test_fx_unknown_currency_from() {
        let result = eval(r#"fx(100,"XYZ","USD")"#);
        let err = result.expect_err("expected error");
        assert_eq!(err.kind, ErrorKind::Domain);
        assert!(err.message.contains("XYZ"), "msg: {}", err.message);
        assert_eq!(err.i18n_key, Some("msg.fx.unknown_currency"));
        // 消息含支持币种数量
        assert!(err.message.contains("4 currencies"), "msg: {}", err.message);
    }

    #[test]
    fn test_fx_unknown_currency_to() {
        let result = eval(r#"fx(100,"USD","XYZ")"#);
        let err = result.expect_err("expected error");
        assert_eq!(err.kind, ErrorKind::Domain);
        assert!(err.message.contains("XYZ"));
        assert_eq!(err.i18n_key, Some("msg.fx.unknown_currency"));
    }

    #[test]
    fn test_fx_wrong_arg_count() {
        let ast = AstNode::FunctionCall(
            "fx".to_string(),
            vec![AstNode::Number(100.0), AstNode::Str("USD".to_string())],
        );
        let result = eval_with_provider(&ast, &EvalContext::new(), &mock_provider());
        let err = result.expect_err("expected error");
        assert_eq!(err.kind, ErrorKind::Domain);
        assert_eq!(err.i18n_key, Some("msg.function_arg_count"));
    }

    #[test]
    fn test_fx_non_string_from_arg() {
        let result = eval(r#"fx(100, 2, "USD")"#);
        let err = result.expect_err("expected error");
        assert_eq!(err.kind, ErrorKind::Domain);
        assert!(err.message.contains("from"));
    }

    #[test]
    fn test_fx_non_string_to_arg() {
        let result = eval(r#"fx(100, "USD", 3)"#);
        let err = result.expect_err("expected error");
        assert_eq!(err.kind, ErrorKind::Domain);
        assert!(err.message.contains("to"));
    }

    // ===== R-fx-001: fx_rate() 查询 =====

    #[test]
    fn test_fx_rate_usd_to_cny() {
        // fx_rate("USD", "CNY") = 1 / 1.08 * 7.85
        let result = eval_scalar(r#"fx_rate("USD","CNY")"#).unwrap();
        let expected = 1.0 / 1.08 * 7.85;
        assert!((result - expected).abs() < 1e-9);
    }

    #[test]
    fn test_fx_rate_eur_to_usd() {
        // fx_rate("EUR", "USD") = 1 / 1.0 * 1.08 = 1.08
        let result = eval_scalar(r#"fx_rate("EUR","USD")"#).unwrap();
        assert!((result - 1.08).abs() < 1e-9);
    }

    #[test]
    fn test_fx_rate_same_currency() {
        let result = eval_scalar(r#"fx_rate("USD","USD")"#).unwrap();
        assert!((result - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_fx_rate_unknown_currency() {
        let result = eval(r#"fx_rate("USD","XYZ")"#);
        let err = result.expect_err("expected error");
        assert_eq!(err.kind, ErrorKind::Domain);
        assert_eq!(err.i18n_key, Some("msg.fx.unknown_currency"));
    }

    #[test]
    fn test_fx_rate_wrong_arg_count() {
        let ast =
            AstNode::FunctionCall("fx_rate".to_string(), vec![AstNode::Str("USD".to_string())]);
        let result = eval_with_provider(&ast, &EvalContext::new(), &mock_provider());
        let err = result.expect_err("expected error");
        assert_eq!(err.kind, ErrorKind::Domain);
        assert_eq!(err.i18n_key, Some("msg.function_arg_count"));
    }

    // ===== R-fx-004: 算术包围与跨域函数混入（design D8）=====

    #[test]
    fn test_arithmetic_wrapping_add() {
        // fx(100,"USD","CNY")+1 = (100/1.08*7.85) + 1
        let result = eval_scalar(r#"fx(100,"USD","CNY")+1"#).unwrap();
        let expected = 100.0 / 1.08 * 7.85 + 1.0;
        assert!((result - expected).abs() < 1e-9);
    }

    #[test]
    fn test_arithmetic_wrapping_mul() {
        // fx(100,"USD","CNY")*2 = (100/1.08*7.85) * 2
        let result = eval_scalar(r#"fx(100,"USD","CNY")*2"#).unwrap();
        let expected = 100.0 / 1.08 * 7.85 * 2.0;
        assert!((result - expected).abs() < 1e-9);
    }

    #[test]
    fn test_arithmetic_wrapping_unary_neg() {
        // -fx(100,"USD","CNY") = -(100/1.08*7.85)
        let result = eval_scalar(r#"-fx(100,"USD","CNY")"#).unwrap();
        let expected = -(100.0 / 1.08 * 7.85);
        assert!((result - expected).abs() < 1e-9);
    }

    #[test]
    fn test_arithmetic_wrapping_unary_abs() {
        // abs(-fx(100,"USD","CNY")) = 100/1.08*7.85
        let result = eval_scalar(r#"abs(-fx(100,"USD","CNY"))"#).unwrap();
        let expected = 100.0 / 1.08 * 7.85;
        assert!((result - expected).abs() < 1e-9);
    }

    #[test]
    fn test_arithmetic_wrapping_sub() {
        // fx(100,"USD","CNY") - fx(50,"USD","CNY") = (100-50)/1.08*7.85
        let result = eval_scalar(r#"fx(100,"USD","CNY")-fx(50,"USD","CNY")"#).unwrap();
        let expected = 50.0 / 1.08 * 7.85;
        assert!((result - expected).abs() < 1e-9);
    }

    #[test]
    fn test_arithmetic_wrapping_div() {
        // fx(100,"USD","CNY") / 2 = (100/1.08*7.85) / 2
        let result = eval_scalar(r#"fx(100,"USD","CNY")/2"#).unwrap();
        let expected = 100.0 / 1.08 * 7.85 / 2.0;
        assert!((result - expected).abs() < 1e-9);
    }

    #[test]
    fn test_arithmetic_wrapping_pow() {
        // fx(2,"USD","USD")^3 = 8
        let result = eval_scalar(r#"fx(2,"USD","USD")^3"#).unwrap();
        assert!((result - 8.0).abs() < 1e-9);
    }

    #[test]
    fn test_arithmetic_wrapping_mod() {
        // fx(1010,"USD","USD") % 1000 = 10
        let result = eval_scalar(r#"fx(1010,"USD","USD")%1000"#).unwrap();
        assert!((result - 10.0).abs() < 1e-9);
    }

    #[test]
    fn test_nested_fx() {
        // fx(fx(100,"USD","EUR"),"EUR","CNY") = fx(100/1.08, "EUR", "CNY") = 100/1.08 * 7.85
        let result = eval_scalar(r#"fx(fx(100,"USD","EUR"),"EUR","CNY")"#).unwrap();
        let expected = 100.0 / 1.08 * 7.85;
        assert!((result - expected).abs() < 1e-9);
    }

    #[test]
    fn test_cross_domain_function_rejected() {
        // sin(1) + fx(100,"USD","CNY") → DomainError 含 "sin"
        let result = eval(r#"sin(1)+fx(100,"USD","CNY")"#);
        let err = result.expect_err("expected error");
        assert_eq!(err.kind, ErrorKind::Domain);
        assert!(err.message.contains("sin"), "msg: {}", err.message);
        assert_eq!(err.i18n_key, Some("msg.unknown_function"));
    }

    #[test]
    fn test_cross_domain_function_alone_rejected() {
        // 单独的 sin(1) 在 fx 域 evaluate 中也应被拒绝
        let ast = AstNode::FunctionCall("sin".to_string(), vec![AstNode::Number(1.0)]);
        let result = eval_with_provider(&ast, &EvalContext::new(), &mock_provider());
        let err = result.expect_err("expected error");
        assert_eq!(err.kind, ErrorKind::Domain);
        assert!(err.message.contains("sin"));
    }

    #[test]
    fn test_unknown_function_in_evaluate() {
        let ast = AstNode::FunctionCall("foo_bar".to_string(), vec![AstNode::Number(1.0)]);
        let result = eval_with_provider(&ast, &EvalContext::new(), &mock_provider());
        let err = result.expect_err("expected error");
        assert_eq!(err.kind, ErrorKind::Domain);
        assert!(err.message.contains("foo_bar"));
    }

    // ===== Str 操作数测试（Str 仅作为 FunctionCall 实参合法）=====

    #[test]
    fn test_str_in_binary_op_rejected() {
        // 1 + "a" → Str 出现在 BinaryOp 中 → DomainError
        let ast = AstNode::BinaryOp(
            BinaryOp::Add,
            Box::new(AstNode::Number(1.0)),
            Box::new(AstNode::Str("a".to_string())),
        );
        let result = eval_with_provider(&ast, &EvalContext::new(), &mock_provider());
        let err = result.expect_err("expected error");
        assert_eq!(err.kind, ErrorKind::Domain);
    }

    #[test]
    fn test_str_in_unary_op_rejected() {
        let ast = AstNode::UnaryOp(UnaryOp::Neg, Box::new(AstNode::Str("a".to_string())));
        let result = eval_with_provider(&ast, &EvalContext::new(), &mock_provider());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_str_alone_rejected() {
        let ast = AstNode::Str("hello".to_string());
        let result = eval_with_provider(&ast, &EvalContext::new(), &mock_provider());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    // ===== 不支持的节点类型 =====

    #[test]
    fn test_complex_rejected() {
        let ast = AstNode::Complex(1.0, 2.0);
        let result = eval_with_provider(&ast, &EvalContext::new(), &mock_provider());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_matrix_rejected() {
        let ast = AstNode::Matrix(vec![vec![AstNode::Number(1.0)]]);
        let result = eval_with_provider(&ast, &EvalContext::new(), &mock_provider());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_list_rejected() {
        let ast = AstNode::List(vec![AstNode::Number(1.0)]);
        let result = eval_with_provider(&ast, &EvalContext::new(), &mock_provider());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    // ===== 变量与常量 =====

    #[test]
    fn test_variable_lookup() {
        // fx(x, "USD", "CNY") where x = 100
        let ctx = EvalContext::new().with_var("x", 100.0);
        let ast = parse(r#"fx(x,"USD","CNY")"#).unwrap();
        let provider = mock_provider();
        let result = eval_with_provider(&ast, &ctx, &provider).unwrap();
        let expected = 100.0 / 1.08 * 7.85;
        assert!((result.as_scalar().unwrap() - expected).abs() < 1e-9);
    }

    #[test]
    fn test_pi_constant() {
        // fx(pi, "USD", "USD") = pi（恒等换算）
        let result = eval_scalar(r#"fx(pi,"USD","USD")"#).unwrap();
        assert!((result - std::f64::consts::PI).abs() < 1e-9);
    }

    #[test]
    fn test_e_constant() {
        let result = eval_scalar(r#"fx(e,"USD","USD")"#).unwrap();
        assert!((result - std::f64::consts::E).abs() < 1e-9);
    }

    #[test]
    fn test_unbound_variable() {
        let result = eval(r#"fx(x,"USD","USD")"#);
        let err = result.expect_err("expected error");
        assert_eq!(err.kind, ErrorKind::Eval);
        assert_eq!(err.i18n_key, Some("msg.unbound_variable"));
    }

    // ===== 二元运算错误路径 =====

    #[test]
    fn test_division_by_zero() {
        let ast = AstNode::BinaryOp(
            BinaryOp::Div,
            Box::new(parse(r#"fx(1,"USD","USD")"#).unwrap()),
            Box::new(AstNode::Number(0.0)),
        );
        let result = eval_with_provider(&ast, &EvalContext::new(), &mock_provider());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::DivisionByZero));
    }

    #[test]
    fn test_mod_by_zero() {
        let ast = AstNode::BinaryOp(
            BinaryOp::Mod,
            Box::new(parse(r#"fx(1,"USD","USD")"#).unwrap()),
            Box::new(AstNode::Number(0.0)),
        );
        let result = eval_with_provider(&ast, &EvalContext::new(), &mock_provider());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::DivisionByZero));
    }

    #[test]
    fn test_zero_to_negative_power() {
        let ast = AstNode::BinaryOp(
            BinaryOp::Pow,
            Box::new(AstNode::Number(0.0)),
            Box::new(AstNode::Number(-1.0)),
        );
        let result = eval_with_provider(&ast, &EvalContext::new(), &mock_provider());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_factorial_unary_rejected() {
        let ast = AstNode::UnaryOp(UnaryOp::Factorial, Box::new(AstNode::Number(5.0)));
        let result = eval_with_provider(&ast, &EvalContext::new(), &mock_provider());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    // ===== BigNumber 支持 =====

    #[test]
    fn test_bignumber_arg() {
        // 大数作为 fx 的 value 参数
        let ast = AstNode::FunctionCall(
            "fx".to_string(),
            vec![
                AstNode::BigNumber("100".to_string()),
                AstNode::Str("USD".to_string()),
                AstNode::Str("CNY".to_string()),
            ],
        );
        let result = eval_with_provider(&ast, &EvalContext::new(), &mock_provider()).unwrap();
        let expected = 100.0 / 1.08 * 7.85;
        assert!((result.as_scalar().unwrap() - expected).abs() < 1e-9);
    }

    #[test]
    fn test_invalid_bignumber() {
        let ast = AstNode::BigNumber("not_a_number".to_string());
        let result = eval_with_provider(&ast, &EvalContext::new(), &mock_provider());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    // ===== 底层函数单元测试 =====

    #[test]
    fn test_convert_same_currency() {
        let table = RateTable {
            base: "EUR".to_string(),
            date: "2026-07-25".to_string(),
            rates: {
                let mut m = HashMap::new();
                m.insert("USD".to_string(), 1.08);
                m
            },
        };
        assert_eq!(convert(100.0, "USD", "USD", &table).unwrap(), 100.0);
    }

    #[test]
    fn test_convert_triangle() {
        let table = RateTable {
            base: "EUR".to_string(),
            date: "2026-07-25".to_string(),
            rates: {
                let mut m = HashMap::new();
                m.insert("USD".to_string(), 1.08);
                m.insert("CNY".to_string(), 7.85);
                m
            },
        };
        let r = convert(100.0, "USD", "CNY", &table).unwrap();
        assert!((r - 100.0 / 1.08 * 7.85).abs() < 1e-9);
    }

    #[test]
    fn test_convert_with_base() {
        let table = RateTable {
            base: "EUR".to_string(),
            date: "2026-07-25".to_string(),
            rates: {
                let mut m = HashMap::new();
                m.insert("USD".to_string(), 1.08);
                m
            },
        };
        // EUR → USD = 100 / 1.0 * 1.08 = 108
        let r = convert(100.0, "EUR", "USD", &table).unwrap();
        assert!((r - 108.0).abs() < 1e-9);
        // USD → EUR = 108 / 1.08 * 1.0 = 100
        let r = convert(108.0, "USD", "EUR", &table).unwrap();
        assert!((r - 100.0).abs() < 1e-9);
    }

    #[test]
    fn test_convert_unknown_currency() {
        let table = RateTable {
            base: "EUR".to_string(),
            date: "2026-07-25".to_string(),
            rates: HashMap::new(),
        };
        let err = convert(100.0, "XYZ", "USD", &table).expect_err("expected error");
        assert_eq!(err.kind, ErrorKind::Domain);
        assert_eq!(err.i18n_key, Some("msg.fx.unknown_currency"));
    }

    #[test]
    fn test_get_rate_base_currency() {
        let table = RateTable {
            base: "EUR".to_string(),
            date: "2026-07-25".to_string(),
            rates: HashMap::new(),
        };
        assert_eq!(get_rate("EUR", &table).unwrap(), 1.0);
    }

    #[test]
    fn test_get_rate_unknown_currency() {
        let table = RateTable {
            base: "EUR".to_string(),
            date: "2026-07-25".to_string(),
            rates: HashMap::new(),
        };
        let err = get_rate("XYZ", &table).expect_err("expected error");
        assert_eq!(err.kind, ErrorKind::Domain);
        assert_eq!(err.i18n_key, Some("msg.fx.unknown_currency"));
    }

    #[test]
    fn test_unknown_currency_error_message_includes_count() {
        let table = RateTable {
            base: "EUR".to_string(),
            date: "2026-07-25".to_string(),
            rates: {
                let mut m = HashMap::new();
                m.insert("USD".to_string(), 1.08);
                m.insert("CNY".to_string(), 7.85);
                m
            },
        };
        let err = unknown_currency_error("XYZ", &table);
        assert!(err.message.contains("XYZ"));
        assert!(err.message.contains("2 currencies"));
        assert_eq!(err.i18n_key, Some("msg.fx.unknown_currency"));
    }

    #[test]
    fn test_expect_str_arg_with_string() {
        let node = AstNode::Str("USD".to_string());
        assert_eq!(expect_str_arg(&node, "from").unwrap(), "USD");
    }

    #[test]
    fn test_expect_str_arg_with_number() {
        let node = AstNode::Number(1.0);
        let err = expect_str_arg(&node, "from").expect_err("expected error");
        assert_eq!(err.kind, ErrorKind::Domain);
        assert!(err.message.contains("from"));
    }

    #[test]
    fn test_contains_fx_function_str_returns_false() {
        let ast = AstNode::Str("fx".to_string());
        assert!(!contains_fx_function(&ast));
    }

    #[test]
    fn test_contains_fx_function_number_returns_false() {
        let ast = AstNode::Number(1.0);
        assert!(!contains_fx_function(&ast));
    }

    #[test]
    fn test_contains_fx_function_complex_returns_false() {
        let ast = AstNode::Complex(1.0, 2.0);
        assert!(!contains_fx_function(&ast));
    }

    #[test]
    fn test_contains_fx_function_bignumber_returns_false() {
        let ast = AstNode::BigNumber("123".to_string());
        assert!(!contains_fx_function(&ast));
    }
}
