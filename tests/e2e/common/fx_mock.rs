// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! fx 域离线测试基础设施：手工汇率表 + MockFxDomain。
//!
//! `FxDomain::new` 为 `pub(crate)`，外部测试无法向真实 FxDomain 注入
//! mock provider；但 `CalculationDomain` trait 与 `math::fx` 纯函数层
//! 全部公开 —— MockFxDomain 以与真实 FxDomain 相同的公开语义
//! （`fx(v,"FROM","TO")` = `v / rate[FROM] * rate[TO]`、`fx_rate` = `fx(1,…)`)
//! 委托同一 math 层，经 `evaluate_with_router` 驱动完整
//! parse → canonicalize → route → evaluate 管线，全程零出网。
//!
//! 跟进项：若 `RateProvider` 未来提供公开注入口，本模块应迁移至
//! 真实 `FxDomain` + mock provider。

#![cfg(feature = "fx")]

use std::collections::HashMap;

use calnexus::domains::fx_provider::RateProvider;
use calnexus::math::fx::{self, RateTable};
use calnexus::{
    AstNode, BinaryOp, CalcError, CalculationDomain, DomainRouter, EvalContext, EvalResult, UnaryOp,
};

/// 手工汇率夹具（base EUR，含 4 个报价币种；数值为易手算的整数倍）。
///
/// convert 语义：`value / rate[FROM] * rate[TO]`，故
/// `fx(110, "USD", "CNY")` = 110 / 1.1 * 7.8 = 780.0。
pub fn mock_rate_table() -> RateTable {
    RateTable {
        base: "EUR".to_string(),
        date: "2026-09-01".to_string(),
        rates: HashMap::from([
            ("USD".to_string(), 1.1),
            ("CNY".to_string(), 7.8),
            ("JPY".to_string(), 160.0),
            ("GBP".to_string(), 0.85),
        ]),
    }
}

/// 实现 `RateProvider` 的固定汇率源（math 层直驱测试用）。
pub struct StaticProvider {
    table: RateTable,
}

impl StaticProvider {
    pub fn new(table: RateTable) -> Self {
        Self { table }
    }
}

impl RateProvider for StaticProvider {
    fn rates(&self) -> Result<RateTable, CalcError> {
        Ok(RateTable {
            base: self.table.base.clone(),
            date: self.table.date.clone(),
            rates: self.table.rates.clone(),
        })
    }
}

/// 恒失败汇率源：确定性复现上游不可达（DependencyUnavailable 传播）。
pub struct FailingProvider;

impl RateProvider for FailingProvider {
    fn rates(&self) -> Result<RateTable, CalcError> {
        Err(CalcError::dependency_unavailable(
            "mock FX rate source unreachable",
        ))
    }
}

/// AST 是否包含 fx/fx_rate 调用（镜像真实 FxDomain 的 supports 语义：
/// 直接调用与 BinaryOp/UnaryOp 包装均命中）。
fn contains_fx_call(ast: &AstNode) -> bool {
    match ast {
        AstNode::FunctionCall(name, args) => {
            matches!(name.as_str(), "fx" | "fx_rate") || args.iter().any(contains_fx_call)
        }
        AstNode::BinaryOp(_, l, r) => contains_fx_call(l) || contains_fx_call(r),
        AstNode::UnaryOp(_, e) => contains_fx_call(e),
        _ => false,
    }
}

fn expect_str<'a>(arg: &'a AstNode, param: &str) -> Result<&'a str, CalcError> {
    match arg {
        AstNode::Str(s) => Ok(s.as_str()),
        other => Err(CalcError::domain(format!(
            "fx() requires string argument for '{param}', got: {other:?}"
        ))),
    }
}

/// fx 域 mock：持有注入的汇率源，语义对齐真实 FxDomain（委托 `math::fx`）。
///
/// 与真实域同构地经 `RateProvider::rates()` 取表——成功源用
/// [`StaticProvider`]，失败源（`FailingProvider`）可确定性复现
/// `DependencyUnavailable` 传播路径。
pub struct MockFxDomain {
    provider: Box<dyn RateProvider>,
}

impl MockFxDomain {
    pub fn new(table: RateTable) -> Self {
        Self::with_provider(Box::new(StaticProvider::new(table)))
    }

    pub fn with_provider(provider: Box<dyn RateProvider>) -> Self {
        Self { provider }
    }

    /// 仅注册本域的独立路由器（fx 表达式链路专用，成功源）。
    pub fn router() -> DomainRouter {
        let mut router = DomainRouter::new();
        router.register(Box::new(MockFxDomain::new(mock_rate_table())));
        router
    }

    /// 注册恒失败源的路由器（DependencyUnavailable 路径专用）。
    pub fn failing_router() -> DomainRouter {
        let mut router = DomainRouter::new();
        router.register(Box::new(MockFxDomain::with_provider(Box::new(
            FailingProvider,
        ))));
        router
    }

    fn eval_scalar(&self, ast: &AstNode, ctx: &EvalContext) -> Result<f64, CalcError> {
        match self.eval_node(ast, ctx)? {
            EvalResult::Scalar(v) => Ok(v),
            other => Err(CalcError::domain(format!(
                "expected scalar operand, got {other:?}"
            ))),
        }
    }

    fn eval_node(&self, ast: &AstNode, ctx: &EvalContext) -> Result<EvalResult, CalcError> {
        match ast {
            AstNode::Number(n) => Ok(EvalResult::Scalar(*n)),
            AstNode::BigNumber(s) => s
                .parse::<f64>()
                .map(EvalResult::Scalar)
                .map_err(|_| CalcError::domain(format!("invalid big number literal: {s}"))),
            AstNode::Variable(name) => ctx
                .get_var(name)
                .map(EvalResult::Scalar)
                .ok_or_else(|| CalcError::undefined_symbol(name)),
            AstNode::BinaryOp(op, l, r) => {
                let a = self.eval_scalar(l, ctx)?;
                let b = self.eval_scalar(r, ctx)?;
                let v = match op {
                    BinaryOp::Add => a + b,
                    BinaryOp::Sub => a - b,
                    BinaryOp::Mul => a * b,
                    BinaryOp::Div => {
                        if b == 0.0 {
                            return Err(CalcError::division_by_zero());
                        }
                        a / b
                    }
                    BinaryOp::Pow => {
                        if a == 0.0 && b < 0.0 {
                            return Err(CalcError::domain(
                                "0 cannot be raised to a negative power",
                            ));
                        }
                        a.powf(b)
                    }
                    BinaryOp::Mod => a % b,
                };
                Ok(EvalResult::Scalar(v))
            }
            AstNode::UnaryOp(op, e) => {
                let v = self.eval_scalar(e, ctx)?;
                match op {
                    UnaryOp::Neg => Ok(EvalResult::Scalar(-v)),
                    UnaryOp::Abs => Ok(EvalResult::Scalar(v.abs())),
                    UnaryOp::Factorial => {
                        Err(CalcError::domain("factorial not supported in fx domain"))
                    }
                }
            }
            AstNode::FunctionCall(name, args) => self.eval_call(name, args, ctx),
            // Str 仅作为函数实参合法；操作数上下文拒绝（对齐真实域语义）。
            AstNode::Str(_) => Err(CalcError::domain(
                "string operand not supported in fx domain",
            )),
            AstNode::Complex(..) | AstNode::Matrix(_) | AstNode::List(_) => {
                Err(CalcError::domain("unsupported node in fx domain"))
            }
        }
    }

    fn eval_call(
        &self,
        name: &str,
        args: &[AstNode],
        ctx: &EvalContext,
    ) -> Result<EvalResult, CalcError> {
        match name {
            "fx" => {
                let [value, from, to] = args else {
                    return Err(CalcError::domain(format!(
                        "fx() requires exactly 3 arguments (value, from, to), got {}",
                        args.len()
                    )));
                };
                let value = self.eval_scalar(value, ctx)?;
                let from = expect_str(from, "from")?;
                let to = expect_str(to, "to")?;
                let table = self.provider.rates()?;
                fx::convert(value, from, to, &table).map(EvalResult::Scalar)
            }
            "fx_rate" => {
                let [from, to] = args else {
                    return Err(CalcError::domain(format!(
                        "fx_rate() requires exactly 2 arguments (from, to), got {}",
                        args.len()
                    )));
                };
                let from = expect_str(from, "from")?;
                let to = expect_str(to, "to")?;
                let table = self.provider.rates()?;
                fx::convert(1.0, from, to, &table).map(EvalResult::Scalar)
            }
            // mod/abs 为 fx 域算术包装白名单（`fx(..)%1000`、`abs(-fx(..))`）
            "mod" => {
                let [a, b] = args else {
                    return Err(CalcError::domain(format!(
                        "mod() requires exactly 2 arguments, got {}",
                        args.len()
                    )));
                };
                let a = self.eval_scalar(a, ctx)?;
                let b = self.eval_scalar(b, ctx)?;
                Ok(EvalResult::Scalar(a % b))
            }
            "abs" => {
                let [v] = args else {
                    return Err(CalcError::domain(format!(
                        "abs() requires exactly 1 argument, got {}",
                        args.len()
                    )));
                };
                let v = self.eval_scalar(v, ctx)?;
                Ok(EvalResult::Scalar(v.abs()))
            }
            other => Err(CalcError::domain(format!(
                "unknown function in fx domain: {other}"
            ))),
        }
    }
}

impl CalculationDomain for MockFxDomain {
    fn domain_name(&self) -> &str {
        "fx"
    }

    fn supports(&self, ast: &AstNode) -> bool {
        contains_fx_call(ast)
    }

    fn evaluate(&self, ast: &AstNode, ctx: &EvalContext) -> Result<EvalResult, CalcError> {
        self.eval_node(ast, ctx)
    }

    fn priority(&self) -> u8 {
        30
    }

    fn nondeterministic_functions(&self) -> &'static [&'static str] {
        &["fx", "fx_rate"]
    }
}
