// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! 时间计算域：日期/时间构造、时间戳互转、间隔计算、格式解析。
//!
//! 设计依据：design.md D4（jiff 0.2 + IANA tzdb）
//! Feature 门控：`time = ["dep:jiff"]`
//!
//! 函数表（14 个）：
//! - 构造：date / datetime / timestamp / from_timestamp
//! - 算术：date_diff / date_add
//! - 格式：parse_date / format_date / reformat_date
//! - 日历：weekday / day_of_year / is_leap_year
//! - 当前：now / today（非确定性，申报旁路 L1 缓存）

use crate::core::CalculationDomain;
use crate::core::{AstNode, BinaryOp, CalcError, EvalContext, EvalResult, UnaryOp};

use jiff::civil::{Date, DateTime};
use jiff::fmt::strtime;
use jiff::tz::TimeZone;
use jiff::{Span, Timestamp, Unit, Zoned};
use std::sync::OnceLock;

/// 时间计算域：支持 date/datetime/timestamp/from_timestamp/date_diff/date_add/
/// parse_date/format_date/reformat_date/weekday/day_of_year/is_leap_year/now/today。
pub struct TimeDomain;

impl CalculationDomain for TimeDomain {
    fn domain_name(&self) -> &str {
        "time"
    }

    fn supports(&self, ast: &AstNode) -> bool {
        contains_time_function(ast)
    }

    fn evaluate(&self, ast: &AstNode, ctx: &EvalContext) -> Result<EvalResult, CalcError> {
        evaluate_time(ast, ctx)
    }

    fn priority(&self) -> u8 {
        30
    }

    fn nondeterministic_functions(&self) -> &'static [&'static str] {
        &["now", "today"]
    }
}

impl Default for TimeDomain {
    fn default() -> Self {
        Self
    }
}

/// 时间域支持的函数名集合。
const TIME_FUNCTIONS: &[&str] = &[
    "date",
    "datetime",
    "timestamp",
    "from_timestamp",
    "date_diff",
    "date_add",
    "parse_date",
    "format_date",
    "reformat_date",
    "weekday",
    "day_of_year",
    "is_leap_year",
    "now",
    "today",
];

/// 递归检查 AST 是否包含时间域函数调用。
fn contains_time_function(ast: &AstNode) -> bool {
    match ast {
        AstNode::FunctionCall(name, args) => {
            TIME_FUNCTIONS.contains(&name.as_str()) || args.iter().any(contains_time_function)
        }
        AstNode::BinaryOp(_, l, r) => contains_time_function(l) || contains_time_function(r),
        AstNode::UnaryOp(_, e) => contains_time_function(e),
        _ => false,
    }
}

// ============================================================================
// 主求值入口与 AST 递归
// ============================================================================

/// 时间域求值入口：递归求值 AST 节点。
///
/// 处理完整 AST（design D8）：域内函数调用 + 算术包围 + 嵌套调用。
fn evaluate_time(ast: &AstNode, ctx: &EvalContext) -> Result<EvalResult, CalcError> {
    match ast {
        AstNode::FunctionCall(name, args) => eval_function(name, args, ctx),
        AstNode::Number(n) => Ok(EvalResult::Scalar(*n)),
        AstNode::BigNumber(s) => {
            let n: f64 = s
                .parse()
                .map_err(|_| CalcError::domain(format!("invalid big number: {}", s)))?;
            Ok(EvalResult::Scalar(n))
        }
        AstNode::Variable(name) => {
            // 用户绑定优先，再识别数学常量 pi/e（与 steps.rs 一致）
            if let Some(v) = ctx.get_var(name) {
                return Ok(EvalResult::Scalar(v));
            }
            match name.as_str() {
                "pi" => Ok(EvalResult::Scalar(std::f64::consts::PI)),
                "e" => Ok(EvalResult::Scalar(std::f64::consts::E)),
                _ => Err(CalcError::eval(format!("unbound variable: {}", name)).with_i18n(
                    "msg.unbound_variable",
                    vec![("name".to_string(), name.clone())],
                )),
            }
        }
        AstNode::BinaryOp(op, l, r) => {
            // 时间域的算术包围：timestamp("...")+3600 等
            // 策略：先尝试将两边求值为标量（DateTime 转为 Unix 秒），做标量算术
            let a = eval_to_f64(l, ctx)?;
            let b = eval_to_f64(r, ctx)?;
            let result = eval_binary_f64(*op, a, b)?;
            Ok(EvalResult::Scalar(result))
        }
        AstNode::UnaryOp(op, e) => {
            let v = eval_to_f64(e, ctx)?;
            let result = match op {
                UnaryOp::Neg => -v,
                UnaryOp::Abs => v.abs(),
                UnaryOp::Factorial => {
                    return Err(CalcError::domain(
                        "factorial not supported in time domain".to_string(),
                    ))
                }
            };
            if result.is_nan() || result.is_infinite() {
                return Err(CalcError::nan_or_inf());
            }
            Ok(EvalResult::Scalar(result))
        }
        AstNode::Str(_) => Err(CalcError::domain(
            "string operand not supported in time domain binary/unary operations".to_string(),
        )),
        AstNode::Complex(_, _) | AstNode::Matrix(_) | AstNode::List(_) => Err(CalcError::domain(
            format!("time domain does not support this node type: {:?}", ast),
        )),
    }
}

/// 二元浮点运算（与 arithmetic 域语义一致）。
fn eval_binary_f64(op: BinaryOp, a: f64, b: f64) -> Result<f64, CalcError> {
    let result = match op {
        BinaryOp::Add => a + b,
        BinaryOp::Sub => a - b,
        BinaryOp::Mul => a * b,
        BinaryOp::Div => {
            if b == 0.0 {
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
    if result.is_nan() || result.is_infinite() {
        return Err(CalcError::nan_or_inf());
    }
    Ok(result)
}

/// 将 AST 求值为 f64（用于算术包围场景）。
///
/// - Scalar → 直接取值
/// - DateTime → 转为 Unix 秒（时间戳）
/// - 其他 → 错误
fn eval_to_f64(ast: &AstNode, ctx: &EvalContext) -> Result<f64, CalcError> {
    match evaluate_time(ast, ctx)? {
        EvalResult::Scalar(n) => Ok(n),
        EvalResult::DateTime(s) => {
            // DateTime 字符串解析为 Zoned 取 Unix 秒
            let zoned = parse_str_to_zoned(&s)?;
            Ok(zoned.timestamp().as_second() as f64)
        }
        other => Err(CalcError::domain(format!(
            "time domain expected scalar or datetime, got {:?}",
            other
        ))),
    }
}

// ============================================================================
// 函数分发
// ============================================================================

/// 求值时间函数调用：按函数名分发到对应的处理方法。
fn eval_function(
    name: &str,
    args: &[AstNode],
    ctx: &EvalContext,
) -> Result<EvalResult, CalcError> {
    if !TIME_FUNCTIONS.contains(&name) {
        return Err(CalcError::domain(format!(
            "unsupported function in time domain: {}",
            name
        ))
        .with_i18n(
            "msg.unknown_function",
            vec![("name".to_string(), name.to_string())],
        ));
    }
    match name {
        "date" => eval_date(args, ctx),
        "datetime" => eval_datetime(args, ctx),
        "timestamp" => eval_timestamp(args, ctx),
        "from_timestamp" => eval_from_timestamp(args, ctx),
        "date_diff" => eval_date_diff(args, ctx),
        "date_add" => eval_date_add(args, ctx),
        "parse_date" => eval_parse_date(args, ctx),
        "format_date" => eval_format_date(args, ctx),
        "reformat_date" => eval_reformat_date(args, ctx),
        "weekday" => eval_weekday(args, ctx),
        "day_of_year" => eval_day_of_year(args, ctx),
        "is_leap_year" => eval_is_leap_year(args, ctx),
        "now" => eval_now(args, ctx),
        "today" => eval_today(args, ctx),
        _ => unreachable!(),
    }
}

// ============================================================================
// 辅助：参数提取
// ============================================================================

/// 从 AST 提取字符串参数（Str 节点）。
fn extract_str(node: &AstNode) -> Result<String, CalcError> {
    match node {
        AstNode::Str(s) => Ok(s.clone()),
        _ => Err(CalcError::domain(format!(
            "expected string argument, got {:?}",
            node
        ))),
    }
}

/// 从 AST 提取整数参数（Number 节点，要求整数值）。
fn extract_i64(node: &AstNode, ctx: &EvalContext) -> Result<i64, CalcError> {
    let v = eval_to_f64(node, ctx)?;
    if v.fract() != 0.0 {
        return Err(CalcError::domain(format!(
            "expected integer argument, got {}",
            v
        )));
    }
    if v > i64::MAX as f64 || v < i64::MIN as f64 {
        return Err(CalcError::overflow());
    }
    Ok(v as i64)
}

/// 解析时区参数：args[idx] 为可选时区字符串，缺省返回 UTC。
fn resolve_timezone(args: &[AstNode], idx: usize, _ctx: &EvalContext) -> Result<TimeZone, CalcError> {
    if idx >= args.len() {
        return Ok(TimeZone::UTC);
    }
    let tz_name = extract_str(&args[idx])?;
    if tz_name.eq_ignore_ascii_case("UTC") {
        return Ok(TimeZone::UTC);
    }
    TimeZone::get(&tz_name).map_err(|_| {
        CalcError::domain(format!("unknown timezone: {}", tz_name)).with_i18n(
            "msg.time.unknown_timezone",
            vec![("tz".to_string(), tz_name.clone())],
        )
    })
}

// ============================================================================
// T009-T010: date / datetime / timestamp / from_timestamp
// ============================================================================

/// date(str) → DateTime（该日 00:00:00 UTC）
///
/// 多格式自动识别（design D4 有序候选格式表）。
fn eval_date(args: &[AstNode], _ctx: &EvalContext) -> Result<EvalResult, CalcError> {
    if args.len() != 1 {
        return Err(CalcError::domain(format!(
            "date() requires exactly 1 argument, got {}",
            args.len()
        ))
        .with_i18n(
            "msg.function_arg_count",
            vec![
                ("name".to_string(), "date".to_string()),
                ("expected".to_string(), "1".to_string()),
                ("actual".to_string(), args.len().to_string()),
            ],
        ));
    }
    let input = extract_str(&args[0])?;
    let date = parse_date_multi_format(&input)?;
    let zoned = date
        .to_zoned(TimeZone::UTC)
        .map_err(|_| invalid_date_error(&input))?;
    Ok(EvalResult::DateTime(zoned_to_rfc3339(&zoned)))
}

/// datetime(str[, tz]) → DateTime
///
/// 多格式自动识别；tz 缺省 UTC。
fn eval_datetime(args: &[AstNode], ctx: &EvalContext) -> Result<EvalResult, CalcError> {
    if args.len() != 1 && args.len() != 2 {
        return Err(CalcError::domain(format!(
            "datetime() requires 1 or 2 arguments, got {}",
            args.len()
        ))
        .with_i18n(
            "msg.function_arg_count",
            vec![
                ("name".to_string(), "datetime".to_string()),
                ("expected".to_string(), "1 or 2".to_string()),
                ("actual".to_string(), args.len().to_string()),
            ],
        ));
    }
    let input = extract_str(&args[0])?;
    let tz = resolve_timezone(args, 1, ctx)?;
    let datetime = parse_datetime_multi_format(&input)?;
    let zoned = datetime.to_zoned(tz).map_err(|_| invalid_date_error(&input))?;
    Ok(EvalResult::DateTime(zoned_to_rfc3339(&zoned)))
}

/// timestamp(str) → Scalar（Unix 秒，含小数毫秒）
///
/// 接受 RFC 3339 / ISO 8601 时间戳字符串。
fn eval_timestamp(args: &[AstNode], _ctx: &EvalContext) -> Result<EvalResult, CalcError> {
    if args.len() != 1 {
        return Err(CalcError::domain(format!(
            "timestamp() requires exactly 1 argument, got {}",
            args.len()
        ))
        .with_i18n(
            "msg.function_arg_count",
            vec![
                ("name".to_string(), "timestamp".to_string()),
                ("expected".to_string(), "1".to_string()),
                ("actual".to_string(), args.len().to_string()),
            ],
        ));
    }
    let input = extract_str(&args[0])?;
    // 优先尝试 RFC 3339 / ISO 8601 解析（Timestamp::from_str），
    // 失败时回退到 strptime 常见格式
    let ts: Timestamp = input
        .parse::<Timestamp>()
        .or_else(|_| parse_timestamp_strptime(&input))
        .map_err(|_| invalid_date_error(&input))?;
    Ok(EvalResult::Scalar(ts.as_second() as f64))
}

/// from_timestamp(secs[, tz]) → DateTime
///
/// secs 为 Unix 秒数；tz 缺省 UTC。
fn eval_from_timestamp(args: &[AstNode], ctx: &EvalContext) -> Result<EvalResult, CalcError> {
    if args.len() != 1 && args.len() != 2 {
        return Err(CalcError::domain(format!(
            "from_timestamp() requires 1 or 2 arguments, got {}",
            args.len()
        ))
        .with_i18n(
            "msg.function_arg_count",
            vec![
                ("name".to_string(), "from_timestamp".to_string()),
                ("expected".to_string(), "1 or 2".to_string()),
                ("actual".to_string(), args.len().to_string()),
            ],
        ));
    }
    let secs = extract_i64(&args[0], ctx)?;
    let tz = resolve_timezone(args, 1, ctx)?;
    let ts = Timestamp::from_second(secs).map_err(|_| {
        CalcError::domain(format!("timestamp out of range: {}", secs)).with_i18n(
            "msg.time.invalid_date",
            vec![("value".to_string(), secs.to_string())],
        )
    })?;
    let zoned = ts.to_zoned(tz);
    Ok(EvalResult::DateTime(zoned_to_rfc3339(&zoned)))
}

// ============================================================================
// T011: date_diff / date_add
// ============================================================================

/// date_diff(a, b[, unit]) → Scalar（带符号间隔）
///
/// unit ∈ s/min/h/day/week/month/year，缺省 day。
/// 语义：返回 a 到 b 的间隔（即 b - a）；b 晚于 a 时为正，b 早于 a 时为负。
/// 注意：参数顺序为 (a, b)，计算 b.since(a)（spec R-time-003 验收标准）。
fn eval_date_diff(args: &[AstNode], ctx: &EvalContext) -> Result<EvalResult, CalcError> {
    if args.len() != 2 && args.len() != 3 {
        return Err(CalcError::domain(format!(
            "date_diff() requires 2 or 3 arguments, got {}",
            args.len()
        ))
        .with_i18n(
            "msg.function_arg_count",
            vec![
                ("name".to_string(), "date_diff".to_string()),
                ("expected".to_string(), "2 or 3".to_string()),
                ("actual".to_string(), args.len().to_string()),
            ],
        ));
    }
    let a = eval_to_zoned(&args[0], ctx)?;
    let b = eval_to_zoned(&args[1], ctx)?;
    let unit_str = if args.len() == 3 {
        extract_str(&args[2])?
    } else {
        "day".to_string()
    };
    let unit = parse_time_unit(&unit_str)?;
    // 计算 b.since(a) = b - a（spec：a 到 b 的间隔；b 晚于 a 为正）
    let span = b.since(&a).map_err(|_| {
        CalcError::domain(format!("date_diff failed: {} vs {}", a, b))
            .with_i18n("msg.time.invalid_date", vec![])
    })?;
    let value = span_total_in_unit(&span, unit, &a, &b)?;
    Ok(EvalResult::Scalar(value))
}

/// date_add(d, n, unit) → DateTime（月末 clamp）
///
/// n 可为负数；月加减遵循 jiff 的 month-end clamp 语义。
fn eval_date_add(args: &[AstNode], ctx: &EvalContext) -> Result<EvalResult, CalcError> {
    if args.len() != 3 {
        return Err(CalcError::domain(format!(
            "date_add() requires exactly 3 arguments, got {}",
            args.len()
        ))
        .with_i18n(
            "msg.function_arg_count",
            vec![
                ("name".to_string(), "date_add".to_string()),
                ("expected".to_string(), "3".to_string()),
                ("actual".to_string(), args.len().to_string()),
            ],
        ));
    }
    let zoned = eval_to_zoned(&args[0], ctx)?;
    let n = extract_i64(&args[1], ctx)?;
    let unit_str = extract_str(&args[2])?;
    let unit = parse_time_unit(&unit_str)?;
    // 按单位构造 Span（jiff 公开 API：try_years/try_months/...，无统一 try_unit）
    let span = build_span_for_unit(unit, n)?;
    let new_zoned = zoned.checked_add(span).map_err(|_| {
        CalcError::domain(format!("date_add overflow: {} + {} units", zoned, n))
            .with_i18n("msg.time.invalid_date", vec![])
    })?;
    Ok(EvalResult::DateTime(zoned_to_rfc3339(&new_zoned)))
}

// ============================================================================
// T012 / T033: parse_date / format_date / reformat_date
// ============================================================================

/// parse_date(str, fmt) → DateTime（按 strptime 格式解析）
fn eval_parse_date(args: &[AstNode], _ctx: &EvalContext) -> Result<EvalResult, CalcError> {
    if args.len() != 2 {
        return Err(CalcError::domain(format!(
            "parse_date() requires exactly 2 arguments, got {}",
            args.len()
        ))
        .with_i18n(
            "msg.function_arg_count",
            vec![
                ("name".to_string(), "parse_date".to_string()),
                ("expected".to_string(), "2".to_string()),
                ("actual".to_string(), args.len().to_string()),
            ],
        ));
    }
    let input = extract_str(&args[0])?;
    let fmt = extract_str(&args[1])?;
    // 先尝试解析为 Date（无时间部分）
    match Date::strptime(&fmt, &input) {
        Ok(date) => {
            let zoned = date
                .to_zoned(TimeZone::UTC)
                .map_err(|_| invalid_date_error(&input))?;
            Ok(EvalResult::DateTime(zoned_to_rfc3339(&zoned)))
        }
        Err(_) => {
            // 失败时尝试解析为 Zoned（含时间/时区）
            match Zoned::strptime(&fmt, &input) {
                Ok(zoned) => Ok(EvalResult::DateTime(zoned_to_rfc3339(&zoned))),
                Err(_) => Err(format_mismatch_error(&fmt)),
            }
        }
    }
}

/// format_date(d_or_ts, fmt[, tz]) → Symbolic（按 strftime 格式输出）
///
/// d_or_ts 接受 DateTime 字符串或 Unix 秒（Scalar）。
fn eval_format_date(args: &[AstNode], ctx: &EvalContext) -> Result<EvalResult, CalcError> {
    if args.len() != 2 && args.len() != 3 {
        return Err(CalcError::domain(format!(
            "format_date() requires 2 or 3 arguments, got {}",
            args.len()
        ))
        .with_i18n(
            "msg.function_arg_count",
            vec![
                ("name".to_string(), "format_date".to_string()),
                ("expected".to_string(), "2 or 3".to_string()),
                ("actual".to_string(), args.len().to_string()),
            ],
        ));
    }
    let zoned = eval_to_zoned(&args[0], ctx)?;
    let tz = resolve_timezone(args, 2, ctx)?;
    let zoned = if args.len() == 3 {
        zoned.with_time_zone(tz)
    } else {
        zoned
    };
    let fmt = extract_str(&args[1])?;
    let formatted = strtime::format(&fmt, &zoned).map_err(|_| format_mismatch_error(&fmt))?;
    Ok(EvalResult::Symbolic(formatted))
}

/// reformat_date(str, from_fmt, to_fmt) → Symbolic（字符串→字符串互转）
///
/// 内部复用 parse_date + format_date 逻辑。
fn eval_reformat_date(args: &[AstNode], _ctx: &EvalContext) -> Result<EvalResult, CalcError> {
    if args.len() != 3 {
        return Err(CalcError::domain(format!(
            "reformat_date() requires exactly 3 arguments, got {}",
            args.len()
        ))
        .with_i18n(
            "msg.function_arg_count",
            vec![
                ("name".to_string(), "reformat_date".to_string()),
                ("expected".to_string(), "3".to_string()),
                ("actual".to_string(), args.len().to_string()),
            ],
        ));
    }
    let input = extract_str(&args[0])?;
    let from_fmt = extract_str(&args[1])?;
    let to_fmt = extract_str(&args[2])?;
    // 解析阶段：先 Date 后 Zoned
    let zoned = match Date::strptime(&from_fmt, &input) {
        Ok(date) => date
            .to_zoned(TimeZone::UTC)
            .map_err(|_| invalid_date_error(&input))?,
        Err(_) => match Zoned::strptime(&from_fmt, &input) {
            Ok(z) => z,
            Err(_) => return Err(format_mismatch_error(&from_fmt)),
        },
    };
    // 格式化阶段
    let formatted = strtime::format(&to_fmt, &zoned).map_err(|_| format_mismatch_error(&to_fmt))?;
    Ok(EvalResult::Symbolic(formatted))
}

// ============================================================================
// T013: weekday / day_of_year / is_leap_year
// ============================================================================

/// weekday(d) → Scalar（ISO 1=Mon..7=Sun）
fn eval_weekday(args: &[AstNode], ctx: &EvalContext) -> Result<EvalResult, CalcError> {
    if args.len() != 1 {
        return Err(CalcError::domain(format!(
            "weekday() requires exactly 1 argument, got {}",
            args.len()
        ))
        .with_i18n(
            "msg.function_arg_count",
            vec![
                ("name".to_string(), "weekday".to_string()),
                ("expected".to_string(), "1".to_string()),
                ("actual".to_string(), args.len().to_string()),
            ],
        ));
    }
    let date = eval_to_date(&args[0], ctx)?;
    let weekday = date.weekday();
    Ok(EvalResult::Scalar(weekday.to_monday_one_offset() as f64))
}

/// day_of_year(d) → Scalar（1..=366）
fn eval_day_of_year(args: &[AstNode], ctx: &EvalContext) -> Result<EvalResult, CalcError> {
    if args.len() != 1 {
        return Err(CalcError::domain(format!(
            "day_of_year() requires exactly 1 argument, got {}",
            args.len()
        ))
        .with_i18n(
            "msg.function_arg_count",
            vec![
                ("name".to_string(), "day_of_year".to_string()),
                ("expected".to_string(), "1".to_string()),
                ("actual".to_string(), args.len().to_string()),
            ],
        ));
    }
    let date = eval_to_date(&args[0], ctx)?;
    Ok(EvalResult::Scalar(date.day_of_year() as f64))
}

/// is_leap_year(year) → Scalar（0/1）
fn eval_is_leap_year(args: &[AstNode], ctx: &EvalContext) -> Result<EvalResult, CalcError> {
    if args.len() != 1 {
        return Err(CalcError::domain(format!(
            "is_leap_year() requires exactly 1 argument, got {}",
            args.len()
        ))
        .with_i18n(
            "msg.function_arg_count",
            vec![
                ("name".to_string(), "is_leap_year".to_string()),
                ("expected".to_string(), "1".to_string()),
                ("actual".to_string(), args.len().to_string()),
            ],
        ));
    }
    let year = extract_i64(&args[0], ctx)?;
    // 闰年判定：能被 4 整除且不能被 100 整除，或能被 400 整除
    let is_leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
    Ok(EvalResult::Scalar(if is_leap { 1.0 } else { 0.0 }))
}

// ============================================================================
// T014: now / today（非确定性）
// ============================================================================

/// now([tz]) → DateTime（当前时刻）
fn eval_now(args: &[AstNode], ctx: &EvalContext) -> Result<EvalResult, CalcError> {
    if args.len() > 1 {
        return Err(CalcError::domain(format!(
            "now() requires 0 or 1 argument, got {}",
            args.len()
        ))
        .with_i18n(
            "msg.function_arg_count",
            vec![
                ("name".to_string(), "now".to_string()),
                ("expected".to_string(), "0 or 1".to_string()),
                ("actual".to_string(), args.len().to_string()),
            ],
        ));
    }
    let tz = resolve_timezone(args, 0, ctx)?;
    let now = Zoned::now().with_time_zone(tz);
    Ok(EvalResult::DateTime(zoned_to_rfc3339(&now)))
}

/// today([tz]) → DateTime（当日 00:00:00）
fn eval_today(args: &[AstNode], ctx: &EvalContext) -> Result<EvalResult, CalcError> {
    if args.len() > 1 {
        return Err(CalcError::domain(format!(
            "today() requires 0 or 1 argument, got {}",
            args.len()
        ))
        .with_i18n(
            "msg.function_arg_count",
            vec![
                ("name".to_string(), "today".to_string()),
                ("expected".to_string(), "0 or 1".to_string()),
                ("actual".to_string(), args.len().to_string()),
            ],
        ));
    }
    let tz = resolve_timezone(args, 0, ctx)?;
    let now = Zoned::now().with_time_zone(tz.clone());
    let today = now.date().to_zoned(tz).map_err(|_| {
        CalcError::domain("failed to construct today midnight".to_string())
            .with_i18n("msg.time.invalid_date", vec![])
    })?;
    Ok(EvalResult::DateTime(zoned_to_rfc3339(&today)))
}

// ============================================================================
// T034: 多格式自动识别（design D4 有序候选格式表）
// ============================================================================

/// date() 候选格式表（无歧义，年在前或月份为单词）。
///
/// 顺序：ISO 8601 → YYYY/MM/DD → YYYY.MM.DD → YYYYMMDD → 英文月份名 → 中文
const DATE_FORMATS: &[&str] = &[
    "%Y-%m-%d",   // ISO 8601: 2026-07-25
    "%Y/%m/%d",   // 2026/07/25
    "%Y.%m.%d",   // 2026.07.25
    "%Y%m%d",     // 20260725（8 位 basic）
    "%d %b %Y",   // 25 Jul 2026
    "%b %d, %Y",  // Jul 25, 2026
    "%B %d, %Y",  // July 25, 2026
];

/// datetime() 候选格式表（日期 + 时间部分）。
const DATETIME_FORMATS: &[&str] = &[
    "%Y-%m-%dT%H:%M:%S",      // ISO 8601: 2026-07-25T12:30:00
    "%Y-%m-%dT%H:%M",         // 2026-07-25T12:30
    "%Y-%m-%d %H:%M:%S",      // 2026-07-25 12:30:00（空格分隔）
    "%Y-%m-%d %H:%M",         // 2026-07-25 12:30
    "%Y/%m/%d %H:%M:%S",      // 2026/07/25 12:30:00
    "%Y/%m/%d %H:%M",         // 2026/07/25 12:30
    "%Y%m%dT%H%M%S",          // 20260725T123000
    "%Y.%m.%d %H:%M:%S",      // 2026.07.25 12:30:00
    "%d %b %Y %H:%M:%S",      // 25 Jul 2026 12:30:00
    "%b %d, %Y %H:%M:%S",     // Jul 25, 2026 12:30:00
    "%B %d, %Y %H:%M:%S",     // July 25, 2026 12:30:00
];

/// 解析日期字符串为 Date（多格式自动识别）。
///
/// 歧义拒绝：纯数字 X/Y/YYYY 或 X-Y-YYYY 形式一律拒绝。
fn parse_date_multi_format(input: &str) -> Result<Date, CalcError> {
    // 0. 先尝试 jiff 原生 Date::from_str（ISO 8601 日期）
    if let Ok(d) = input.parse::<Date>() {
        return Ok(d);
    }
    // 1. 歧义检测：纯数字 X/Y/YYYY、X-Y-YYYY、X.Y.YYYY 一律拒绝
    if is_ambiguous_numeric_date(input) {
        return Err(ambiguous_format_error());
    }
    // 2. 中文格式 fallback：2026年7月25日
    if input.contains('年') {
        return parse_chinese_date(input).map_err(|_| invalid_date_error(input));
    }
    // 3. 按候选格式表逐一尝试 strptime
    for fmt in DATE_FORMATS {
        if let Ok(d) = Date::strptime(fmt, input) {
            return Ok(d);
        }
    }
    Err(invalid_date_error(input))
}

/// 解析 datetime 字符串为 DateTime（多格式自动识别）。
fn parse_datetime_multi_format(input: &str) -> Result<DateTime, CalcError> {
    // 0. 先尝试 jiff 原生 DateTime::from_str（ISO 8601）
    if let Ok(dt) = input.parse::<DateTime>() {
        return Ok(dt);
    }
    // 1. 歧义检测
    if is_ambiguous_numeric_date(input) {
        return Err(ambiguous_format_error());
    }
    // 2. 中文格式 fallback
    if input.contains('年') {
        return parse_chinese_datetime(input).map_err(|_| invalid_date_error(input));
    }
    // 3. 按候选格式表逐一尝试
    for fmt in DATETIME_FORMATS {
        if let Ok(dt) = DateTime::strptime(fmt, input) {
            return Ok(dt);
        }
    }
    // 4. 退化为纯日期解析（时间为 00:00:00）
    if let Ok(d) = parse_date_multi_format(input) {
        return Ok(DateTime::from(d));
    }
    Err(invalid_date_error(input))
}

/// 用 strptime 尝试解析 timestamp 字符串（带偏移量）。
fn parse_timestamp_strptime(input: &str) -> Result<Timestamp, ()> {
    let formats: &[&str] = &[
        "%Y-%m-%dT%H:%M:%S%:z",
        "%Y-%m-%dT%H:%M:%SZ",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%d %H:%M:%S%:z",
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%d",
    ];
    for fmt in formats {
        if let Ok(ts) = Timestamp::strptime(fmt, input) {
            return Ok(ts);
        }
    }
    Err(())
}

/// 检测纯数字 X/Y/YYYY、X-Y-YYYY、X.Y.YYYY 形式（歧义格式）。
///
/// 即使日>12 可推断也拒绝，保持行为一致性（design D4 歧义拒绝）。
fn is_ambiguous_numeric_date(input: &str) -> bool {
    // 匹配 N/N/YYYY 或 N-N-YYYY 或 N.N.YYYY 形式（日在前的纯数字）
    // 关键特征：第一个分隔符前只有 1-2 位数字（年在前时为 4 位）
    ambiguous_date_regex().is_match(input)
}

/// 中文日期解析：2026年7月25日
fn parse_chinese_date(input: &str) -> Result<Date, ()> {
    // 先尝试 strptime 字面量匹配（jiff 支持 UTF-8 字面量）
    if let Ok(d) = Date::strptime("%Y年%m月%d日", input) {
        return Ok(d);
    }
    // fallback：正则预提取年月日
    let caps = chinese_date_regex().captures(input).ok_or(())?;
    let year: i16 = caps[1].parse().map_err(|_| ())?;
    let month: i8 = caps[2].parse().map_err(|_| ())?;
    let day: i8 = caps[3].parse().map_err(|_| ())?;
    Date::new(year, month, day).map_err(|_| ())
}

/// 中文 datetime 解析：2026年7月25日 12:30:00 或 2026年7月25日12:30:00
fn parse_chinese_datetime(input: &str) -> Result<DateTime, ()> {
    // 先尝试带时间的 strptime
    if let Ok(dt) = DateTime::strptime("%Y年%m月%d日 %H:%M:%S", input) {
        return Ok(dt);
    }
    if let Ok(dt) = DateTime::strptime("%Y年%m月%d日%H:%M:%S", input) {
        return Ok(dt);
    }
    if let Ok(dt) = DateTime::strptime("%Y年%m月%d日 %H:%M", input) {
        return Ok(dt);
    }
    // fallback：提取日期部分 + 时间部分
    let date = parse_chinese_date(input)?;
    // 提取时间部分（HH:MM 或 HH:MM:SS）
    if let Some(caps) = time_part_regex().captures(input) {
        let hour: i8 = caps[1].parse().map_err(|_| ())?;
        let minute: i8 = caps[2].parse().map_err(|_| ())?;
        let second: i8 = caps.get(3).map_or(0, |m| m.as_str().parse().unwrap_or(0));
        return Ok(date.at(hour, minute, second, 0));
    }
    Ok(DateTime::from(date))
}

// ============================================================================
// 辅助：AST → Date / Zoned 转换
// ============================================================================

/// 将 AST 求值为 Zoned（用于 date_diff / date_add）。
///
/// 接受 Str（日期字符串）或 Scalar（Unix 秒）或 DateTime 结果（嵌套调用）。
fn eval_to_zoned(ast: &AstNode, ctx: &EvalContext) -> Result<Zoned, CalcError> {
    match ast {
        AstNode::Str(s) => parse_str_to_zoned(s),
        AstNode::Number(n) => {
            let ts = Timestamp::from_second(*n as i64).map_err(|_| {
                CalcError::domain(format!("timestamp out of range: {}", n)).with_i18n(
                    "msg.time.invalid_date",
                    vec![("value".to_string(), n.to_string())],
                )
            })?;
            Ok(ts.to_zoned(TimeZone::UTC))
        }
        _ => {
            // 嵌套函数调用：先求值为 EvalResult，再转换
            let result = evaluate_time(ast, ctx)?;
            match result {
                EvalResult::DateTime(s) => parse_str_to_zoned(&s),
                EvalResult::Scalar(n) => {
                    let ts = Timestamp::from_second(n as i64).map_err(|_| {
                        CalcError::domain(format!("timestamp out of range: {}", n)).with_i18n(
                            "msg.time.invalid_date",
                            vec![("value".to_string(), n.to_string())],
                        )
                    })?;
                    Ok(ts.to_zoned(TimeZone::UTC))
                }
                other => Err(CalcError::domain(format!(
                    "expected datetime or scalar, got {:?}",
                    other
                ))),
            }
        }
    }
}

/// 解析字符串为 Zoned（多策略回退）。
///
/// 1. Zoned::from_str：标准 RFC 3339（含时区名后缀，如 `2026-07-25T00:00:00+08:00[Asia/Shanghai]`）
/// 2. Z 后缀归一化：`...Z` → `...+00:00`，再用 strptime `%:z` 解析
/// 3. Zoned::strptime：多种常见格式（含 +HH:MM 偏移、无偏移）
/// 4. parse_datetime_multi_format + UTC：无时区信息的日期字符串
fn parse_str_to_zoned(s: &str) -> Result<Zoned, CalcError> {
    // 1. 标准 RFC 3339（temporal parser，接受 Z、+HH:MM、[tzname] 等）
    if let Ok(z) = s.parse::<Zoned>() {
        return Ok(z);
    }
    // 2. Z 后缀归一化为 +00:00（strptime %Z 不支持 Z 字面量作为偏移）
    let normalized = if let Some(stripped) = s.strip_suffix('Z') {
        format!("{}+00:00", stripped)
    } else {
        s.to_string()
    };
    // 3. strptime 常见格式回退
    const ZONED_FORMATS: &[&str] = &[
        "%Y-%m-%dT%H:%M:%S%:z",  // 2026-07-25T12:30:00+00:00
        "%Y-%m-%dT%H:%M:%S",      // 2026-07-25T12:30:00（无偏移，UTC）
        "%Y-%m-%d %H:%M:%S%:z",   // 2026-07-25 12:30:00+00:00
        "%Y-%m-%d %H:%M:%S",      // 2026-07-25 12:30:00
    ];
    for fmt in ZONED_FORMATS {
        if let Ok(z) = Zoned::strptime(fmt, &normalized) {
            return Ok(z);
        }
    }
    // 4. 多格式 datetime 解析 + UTC
    let dt = parse_datetime_multi_format(s)?;
    dt.to_zoned(TimeZone::UTC).map_err(|_| invalid_date_error(s))
}

/// 将 AST 求值为 Date（用于 weekday / day_of_year）。
fn eval_to_date(ast: &AstNode, ctx: &EvalContext) -> Result<Date, CalcError> {
    match ast {
        AstNode::Str(s) => parse_date_multi_format(s),
        AstNode::Number(n) => {
            let ts = Timestamp::from_second(*n as i64).map_err(|_| {
                CalcError::domain(format!("timestamp out of range: {}", n)).with_i18n(
                    "msg.time.invalid_date",
                    vec![("value".to_string(), n.to_string())],
                )
            })?;
            Ok(ts.to_zoned(TimeZone::UTC).date())
        }
        _ => {
            let zoned = eval_to_zoned(ast, ctx)?;
            Ok(zoned.date())
        }
    }
}

// ============================================================================
// 辅助：单位解析与 Span 构造
// ============================================================================

/// 解析时间单位字符串为 jiff::Unit。
fn parse_time_unit(s: &str) -> Result<Unit, CalcError> {
    match s {
        "s" | "second" | "seconds" => Ok(Unit::Second),
        "min" | "minute" | "minutes" => Ok(Unit::Minute),
        "h" | "hour" | "hours" => Ok(Unit::Hour),
        "day" | "days" => Ok(Unit::Day),
        "week" | "weeks" => Ok(Unit::Week),
        "month" | "months" => Ok(Unit::Month),
        "year" | "years" => Ok(Unit::Year),
        _ => Err(CalcError::domain(format!("invalid time unit: {}", s)).with_i18n(
            "msg.time.invalid_unit",
            vec![("unit".to_string(), s.to_string())],
        )),
    }
}

/// 按单位构造 Span（n 为有符号数量）。
fn build_span_for_unit(unit: Unit, n: i64) -> Result<Span, CalcError> {
    let span = Span::new();
    match unit {
        Unit::Second => span.try_seconds(n),
        Unit::Minute => span.try_minutes(n),
        Unit::Hour => span.try_hours(n),
        Unit::Day => span.try_days(n),
        Unit::Week => span.try_weeks(n),
        Unit::Month => span.try_months(n),
        Unit::Year => span.try_years(n),
        _ => unreachable!(),
    }
    .map_err(|_| {
        CalcError::domain(format!("span out of range: {} {:?}", n, unit))
            .with_i18n("msg.time.invalid_unit", vec![("unit".to_string(), format!("{:?}", unit))])
    })
}

/// 计算 Span 在指定单位下的总数值（带符号）。
///
/// jiff 的 Zoned::since 默认返回包含 year/month 等日历单位的 Span，
/// 因此 span.total(Unit::Day) 会因月长度不定而失败。
/// 解决：所有单位都传入相对参考点（Zoned）以提供日历上下文。
fn span_total_in_unit(
    span: &Span,
    unit: Unit,
    a: &Zoned,
    _b: &Zoned,
) -> Result<f64, CalcError> {
    // 所有单位都使用 (Unit, &Zoned) 形式提供相对参考点
    let result = match unit {
        Unit::Second => span.total((Unit::Second, a)),
        Unit::Minute => span.total((Unit::Minute, a)),
        Unit::Hour => span.total((Unit::Hour, a)),
        Unit::Day => span.total((Unit::Day, a)),
        Unit::Week => span.total((Unit::Week, a)),
        Unit::Month => span.total((Unit::Month, a)),
        Unit::Year => span.total((Unit::Year, a)),
        _ => unreachable!(),
    };
    result.map_err(|_| {
        CalcError::domain(format!("span total failed for {:?}", unit)).with_i18n(
            "msg.time.invalid_unit",
            vec![("unit".to_string(), format!("{:?}", unit))],
        )
    })
}

// ============================================================================
// 辅助：错误构造与格式化
// ============================================================================

/// 构造"无效日期"错误。
fn invalid_date_error(value: &str) -> CalcError {
    CalcError::domain(format!("invalid date: {}", value)).with_i18n(
        "msg.time.invalid_date",
        vec![("value".to_string(), value.to_string())],
    )
}

/// 构造"格式不匹配"错误。
fn format_mismatch_error(fmt: &str) -> CalcError {
    CalcError::domain(format!("format mismatch: {}", fmt)).with_i18n(
        "msg.time.format_mismatch",
        vec![("fmt".to_string(), fmt.to_string())],
    )
}

/// 构造"歧义格式"错误。
fn ambiguous_format_error() -> CalcError {
    CalcError::domain("ambiguous date format, use parse_date() with explicit format".to_string())
        .with_i18n("msg.time.ambiguous_format", vec![])
}

/// 将 Zoned 转换为 RFC 3339 字符串（用于 EvalResult::DateTime）。
///
/// 使用 strtime %Y-%m-%dT%H:%M:%S%:z 格式，输出 `YYYY-MM-DDTHH:MM:SS+HH:MM`。
/// 不带时区名后缀（与 design D2 一致），UTC 输出为 `+00:00`。
fn zoned_to_rfc3339(zoned: &Zoned) -> String {
    strtime::format("%Y-%m-%dT%H:%M:%S%:z", zoned)
        .unwrap_or_else(|_| zoned.to_string())
}

// ============================================================================
// 辅助：regex 缓存（OnceLock 模式，避免重复编译）
// ============================================================================

/// 获取歧义日期检测 regex（缓存）。
fn ambiguous_date_regex() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(r"^\d{1,2}[/\-.]\d{1,2}[/\-.]\d{4}$").expect("valid regex")
    })
}

/// 获取中文日期 regex（缓存）。
fn chinese_date_regex() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(r"(\d{4})年(\d{1,2})月(\d{1,2})日").expect("valid regex")
    })
}

/// 获取时间部分 regex（缓存）。
fn time_part_regex() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"(\d{1,2}):(\d{2})(?::(\d{2}))?").expect("valid regex"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::parse;
    use crate::core::ErrorKind;

    /// 测试辅助：求值表达式字符串。
    fn eval(input: &str) -> Result<EvalResult, CalcError> {
        let ast = parse(input).unwrap();
        let domain = TimeDomain;
        let ctx = EvalContext::new();
        domain.evaluate(&ast, &ctx)
    }

    /// 测试辅助：求值并提取标量。
    fn eval_scalar(input: &str) -> Result<f64, CalcError> {
        eval(input).map(|r| r.as_scalar().expect("expected scalar result"))
    }

    /// 测试辅助：求值并提取 DateTime 字符串。
    fn eval_datetime_str(input: &str) -> Result<String, CalcError> {
        eval(input).map(|r| match r {
            EvalResult::DateTime(s) => s,
            _ => panic!("expected datetime result, got {:?}", r),
        })
    }

    /// 测试辅助：求值并提取 Symbolic 字符串。
    fn eval_symbolic(input: &str) -> Result<String, CalcError> {
        eval(input).map(|r| match r {
            EvalResult::Symbolic(s) => s,
            _ => panic!("expected symbolic result, got {:?}", r),
        })
    }

    // ===== T009-T010: date / datetime / timestamp / from_timestamp =====

    #[test]
    fn test_date_iso_basic() {
        let result = eval_datetime_str(r#"date("2026-07-25")"#).unwrap();
        assert_eq!(result, "2026-07-25T00:00:00+00:00");
    }

    #[test]
    fn test_date_invalid() {
        let result = eval(r#"date("2026-02-30")"#);
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_datetime_with_timezone() {
        let result =
            eval_datetime_str(r#"datetime("2026-07-25T12:30:00","Asia/Shanghai")"#).unwrap();
        assert!(result.contains("+08:00"), "expected +08:00 offset, got {}", result);
    }

    #[test]
    fn test_datetime_unknown_timezone() {
        let result = eval(r#"datetime("2026-01-01T00:00:00","Not/AZone")"#);
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_datetime_default_utc() {
        let result = eval_datetime_str(r#"datetime("2026-07-25T12:30:00")"#).unwrap();
        assert_eq!(result, "2026-07-25T12:30:00+00:00");
    }

    #[test]
    fn test_timestamp_iso() {
        let result = eval_scalar(r#"timestamp("2026-07-25T00:00:00Z")"#).unwrap();
        // 2026-07-25T00:00:00Z 的 Unix 时间戳
        // 验证可通过 from_timestamp 反算
        assert!(result > 1.7e9 && result < 1.8e9, "expected ~1.78e9, got {}", result);
    }

    #[test]
    fn test_from_timestamp_zero() {
        let result = eval_datetime_str(r#"from_timestamp(0,"UTC")"#).unwrap();
        assert_eq!(result, "1970-01-01T00:00:00+00:00");
    }

    #[test]
    fn test_from_timestamp_default_utc() {
        let result = eval_datetime_str("from_timestamp(0)").unwrap();
        assert_eq!(result, "1970-01-01T00:00:00+00:00");
    }

    #[test]
    fn test_from_timestamp_out_of_range() {
        let result = eval("from_timestamp(1000000000000000000)");
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_timestamp_from_timestamp_roundtrip() {
        // timestamp → from_timestamp → timestamp 应保持一致（整秒）
        let ts = eval_scalar(r#"timestamp("2026-07-25T00:00:00Z")"#).unwrap();
        let dt = eval_datetime_str(&format!("from_timestamp({})", ts as i64)).unwrap();
        let ts2 = eval_scalar(&format!(r#"timestamp("{}")"#, dt)).unwrap();
        assert_eq!(ts, ts2);
    }

    // ===== T010: 算术包围与嵌套 =====

    #[test]
    fn test_timestamp_arithmetic() {
        // timestamp("...")+3600 应正确计算
        let result = eval_scalar(r#"timestamp("2026-07-25T00:00:00Z")+3600"#).unwrap();
        let base = eval_scalar(r#"timestamp("2026-07-25T00:00:00Z")"#).unwrap();
        assert_eq!(result, base + 3600.0);
    }

    #[test]
    fn test_nested_function_call() {
        // weekday(from_timestamp(0)) 嵌套正确
        // 1970-01-01 是周四，ISO weekday=4
        let result = eval_scalar("weekday(from_timestamp(0))").unwrap();
        assert_eq!(result, 4.0);
    }

    #[test]
    fn test_unknown_function_rejected() {
        let result = eval("sin(1)");
        let err = result.unwrap_err();
        assert_eq!(err.kind, ErrorKind::Domain);
        assert!(err.message.contains("sin"));
    }

    #[test]
    fn test_wrong_arg_count() {
        let result = eval(r#"date("2026-07-25","extra")"#);
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    // ===== T011: date_diff / date_add =====

    #[test]
    fn test_date_diff_days() {
        let result = eval_scalar(r#"date_diff("2026-01-01","2026-07-25")"#).unwrap();
        // 2026-01-01 到 2026-07-25 = 205 天
        // 注意：date_diff(a, b) 计算 a.since(b)
        // 如果 a=2026-01-01, b=2026-07-25，则 a < b，结果应为负
        // 但 spec 说 date_diff("2026-01-01","2026-07-25") = 205
        // 这意味着参数顺序是 (start, end)，计算 end - start
        // 重新审视：spec 说 date_diff(a, b) 返回 a 到 b 的间隔
        // 所以 date_diff("2026-01-01","2026-07-25") = 205（从 1-1 到 7-25）
        // 而 date_diff("2026-07-25","2026-01-01") = -205
        // 这意味着实际计算 b.since(a)
        assert_eq!(result, 205.0);
    }

    #[test]
    fn test_date_diff_negative() {
        let result = eval_scalar(r#"date_diff("2026-07-25","2026-01-01")"#).unwrap();
        assert_eq!(result, -205.0);
    }

    #[test]
    fn test_date_diff_seconds() {
        let result = eval_scalar(r#"date_diff("1970-01-01","1970-01-02","s")"#).unwrap();
        assert_eq!(result, 86400.0);
    }

    #[test]
    fn test_date_diff_hours() {
        let result = eval_scalar(r#"date_diff("1970-01-01","1970-01-02","h")"#).unwrap();
        assert_eq!(result, 24.0);
    }

    #[test]
    fn test_date_diff_invalid_unit() {
        let result = eval(r#"date_diff("2026-01-01","2026-07-25","invalid")"#);
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_date_add_month_clamp() {
        // 2026-01-31 + 1 month = 2026-02-28（非闰年 clamp）
        let result = eval_datetime_str(r#"date_add("2026-01-31",1,"month")"#).unwrap();
        assert!(result.starts_with("2026-02-28"), "expected 2026-02-28, got {}", result);
    }

    #[test]
    fn test_date_add_month_leap_year() {
        // 2024-01-31 + 1 month = 2024-02-29（闰年 clamp）
        let result = eval_datetime_str(r#"date_add("2024-01-31",1,"month")"#).unwrap();
        assert!(result.starts_with("2024-02-29"), "expected 2024-02-29, got {}", result);
    }

    #[test]
    fn test_date_add_negative() {
        let result = eval_datetime_str(r#"date_add("2026-07-25",-1,"day")"#).unwrap();
        assert!(result.starts_with("2026-07-24"), "expected 2026-07-24, got {}", result);
    }

    #[test]
    fn test_date_add_days() {
        let result = eval_datetime_str(r#"date_add("2026-01-01",31,"day")"#).unwrap();
        assert!(result.starts_with("2026-02-01"), "expected 2026-02-01, got {}", result);
    }

    // ===== T012: parse_date / format_date =====

    #[test]
    fn test_parse_date_basic() {
        let result = eval_datetime_str(r#"parse_date("25/07/2026","%d/%m/%Y")"#).unwrap();
        assert_eq!(result, "2026-07-25T00:00:00+00:00");
    }

    #[test]
    fn test_format_date_from_timestamp() {
        let result = eval_symbolic(r#"format_date(0,"%Y-%m-%d","UTC")"#).unwrap();
        assert_eq!(result, "1970-01-01");
    }

    #[test]
    fn test_format_date_from_datetime() {
        let result = eval_symbolic(r#"format_date("2026-07-25T12:30:00Z","%Y-%m-%d")"#).unwrap();
        assert_eq!(result, "2026-07-25");
    }

    #[test]
    fn test_parse_date_mismatch() {
        let result = eval(r#"parse_date("invalid","%Y-%m-%d")"#);
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    // ===== T033: reformat_date =====

    #[test]
    fn test_reformat_date_basic() {
        let result =
            eval_symbolic(r#"reformat_date("25/07/2026","%d/%m/%Y","%Y-%m-%d")"#).unwrap();
        assert_eq!(result, "2026-07-25");
    }

    #[test]
    fn test_reformat_date_parse_fail() {
        let result = eval(r#"reformat_date("invalid","%d/%m/%Y","%Y-%m-%d")"#);
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    // ===== T034: 多格式自动识别 =====

    #[test]
    fn test_date_multi_format_slash() {
        let result = eval_datetime_str(r#"date("2026/07/25")"#).unwrap();
        assert_eq!(result, "2026-07-25T00:00:00+00:00");
    }

    #[test]
    fn test_date_multi_format_basic() {
        let result = eval_datetime_str(r#"date("20260725")"#).unwrap();
        assert_eq!(result, "2026-07-25T00:00:00+00:00");
    }

    #[test]
    fn test_date_multi_format_english_abbr() {
        let result = eval_datetime_str(r#"date("25 Jul 2026")"#).unwrap();
        assert_eq!(result, "2026-07-25T00:00:00+00:00");
    }

    #[test]
    fn test_date_multi_format_english_abbr2() {
        let result = eval_datetime_str(r#"date("Jul 25, 2026")"#).unwrap();
        assert_eq!(result, "2026-07-25T00:00:00+00:00");
    }

    #[test]
    fn test_date_multi_format_english_full() {
        let result = eval_datetime_str(r#"date("July 25, 2026")"#).unwrap();
        assert_eq!(result, "2026-07-25T00:00:00+00:00");
    }

    #[test]
    fn test_date_multi_format_chinese() {
        let result = eval_datetime_str(r#"date("2026年7月25日")"#).unwrap();
        assert_eq!(result, "2026-07-25T00:00:00+00:00");
    }

    #[test]
    fn test_date_multi_format_dot() {
        let result = eval_datetime_str(r#"date("2026.07.25")"#).unwrap();
        assert_eq!(result, "2026-07-25T00:00:00+00:00");
    }

    #[test]
    fn test_datetime_multi_format_space() {
        let result = eval_datetime_str(r#"datetime("2026/07/25 12:30:00")"#).unwrap();
        assert!(result.starts_with("2026-07-25T12:30:00"), "got {}", result);
    }

    #[test]
    fn test_datetime_multi_format_iso() {
        let result = eval_datetime_str(r#"datetime("2026-07-25T12:30:00")"#).unwrap();
        assert!(result.starts_with("2026-07-25T12:30:00"), "got {}", result);
    }

    #[test]
    fn test_date_ambiguous_rejected() {
        // 01/02/2026 歧义（月/日顺序不定）
        let result = eval(r#"date("01/02/2026")"#);
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_date_ambiguous_rejected2() {
        // 13/07/2026 即使可推断也拒绝
        let result = eval(r#"date("13/07/2026")"#);
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    // ===== T013: weekday / day_of_year / is_leap_year =====

    #[test]
    fn test_weekday_basic() {
        // 2026-07-25 是周六，ISO 6
        let result = eval_scalar(r#"weekday("2026-07-25")"#).unwrap();
        assert_eq!(result, 6.0);
    }

    #[test]
    fn test_weekday_thursday() {
        // 1970-01-01 是周四，ISO 4
        let result = eval_scalar(r#"weekday("1970-01-01")"#).unwrap();
        assert_eq!(result, 4.0);
    }

    #[test]
    fn test_day_of_year_basic() {
        // 2026-07-25 是第 206 天
        let result = eval_scalar(r#"day_of_year("2026-07-25")"#).unwrap();
        assert_eq!(result, 206.0);
    }

    #[test]
    fn test_is_leap_year_2024() {
        let result = eval_scalar("is_leap_year(2024)").unwrap();
        assert_eq!(result, 1.0);
    }

    #[test]
    fn test_is_leap_year_2026() {
        let result = eval_scalar("is_leap_year(2026)").unwrap();
        assert_eq!(result, 0.0);
    }

    #[test]
    fn test_is_leap_year_1900() {
        // 整百非闰
        let result = eval_scalar("is_leap_year(1900)").unwrap();
        assert_eq!(result, 0.0);
    }

    #[test]
    fn test_is_leap_year_2000() {
        // 400 整除为闰
        let result = eval_scalar("is_leap_year(2000)").unwrap();
        assert_eq!(result, 1.0);
    }

    // ===== T014: now / today =====

    #[test]
    fn test_now_returns_valid_rfc3339() {
        let result = eval_datetime_str("now()").unwrap();
        // 应可通过 parse_str_to_zoned 反解析（兼容 strtime 格式输出）
        assert!(
            parse_str_to_zoned(&result).is_ok(),
            "now() result should be parseable: {}",
            result
        );
    }

    #[test]
    fn test_now_with_timezone() {
        let result = eval_datetime_str(r#"now("Asia/Shanghai")"#).unwrap();
        assert!(result.contains("+08:00"), "expected +08:00, got {}", result);
    }

    #[test]
    fn test_today_returns_valid_date() {
        let result = eval_datetime_str("today()").unwrap();
        // 应为当日 00:00:00
        assert!(result.contains("T00:00:00"), "expected midnight, got {}", result);
        assert!(
            parse_str_to_zoned(&result).is_ok(),
            "today() result should be parseable: {}",
            result
        );
    }

    // ===== 域元信息测试 =====

    #[test]
    fn test_domain_info() {
        let domain = TimeDomain;
        assert_eq!(domain.domain_name(), "time");
        assert_eq!(domain.priority(), 30);
        assert_eq!(domain.nondeterministic_functions(), &["now", "today"]);
    }

    #[test]
    fn test_default_impl() {
        let domain = TimeDomain;
        assert_eq!(domain.domain_name(), "time");
    }

    #[test]
    fn test_supports_date() {
        let ast = parse(r#"date("2026-07-25")"#).unwrap();
        assert!(TimeDomain.supports(&ast));
    }

    #[test]
    fn test_supports_nested() {
        let ast = parse(r#"timestamp("2026-07-25T00:00:00Z")+3600"#).unwrap();
        assert!(TimeDomain.supports(&ast));
    }

    #[test]
    fn test_supports_now() {
        let ast = parse("now()").unwrap();
        assert!(TimeDomain.supports(&ast));
    }

    #[test]
    fn test_not_supports_arithmetic() {
        let ast = parse("1+2").unwrap();
        assert!(!TimeDomain.supports(&ast));
    }

    #[test]
    fn test_not_supports_sin() {
        let ast = parse("sin(1)").unwrap();
        assert!(!TimeDomain.supports(&ast));
    }

    // ===== 错误路径测试 =====

    #[test]
    fn test_unsupported_function() {
        let ast = AstNode::FunctionCall("foo".to_string(), vec![]);
        let result = TimeDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_date_wrong_args() {
        let ast = AstNode::FunctionCall(
            "date".to_string(),
            vec![AstNode::Str("2026-07-25".to_string()), AstNode::Number(1.0)],
        );
        let result = TimeDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_str_operand_rejected() {
        let ast = AstNode::Str("hello".to_string());
        let result = TimeDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_complex_rejected() {
        let ast = AstNode::Complex(1.0, 2.0);
        let result = TimeDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_matrix_rejected() {
        let ast = AstNode::Matrix(vec![vec![AstNode::Number(1.0)]]);
        let result = TimeDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    #[test]
    fn test_list_rejected() {
        let ast = AstNode::List(vec![AstNode::Number(1.0)]);
        let result = TimeDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    // ===== 变量与常量测试 =====

    #[test]
    fn test_variable_pi() {
        let ast = AstNode::Variable("pi".to_string());
        let result = TimeDomain.evaluate(&ast, &EvalContext::new()).unwrap();
        assert_eq!(result.as_scalar().unwrap(), std::f64::consts::PI);
    }

    #[test]
    fn test_variable_e() {
        let ast = AstNode::Variable("e".to_string());
        let result = TimeDomain.evaluate(&ast, &EvalContext::new()).unwrap();
        assert_eq!(result.as_scalar().unwrap(), std::f64::consts::E);
    }

    #[test]
    fn test_variable_user_binding() {
        let ctx = EvalContext::new().with_var("x", 42.0);
        let ast = AstNode::Variable("x".to_string());
        let result = TimeDomain.evaluate(&ast, &ctx).unwrap();
        assert_eq!(result.as_scalar().unwrap(), 42.0);
    }

    #[test]
    fn test_unbound_variable() {
        let ast = AstNode::Variable("unknown".to_string());
        let result = TimeDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Eval));
    }

    // ===== 算术运算测试 =====

    #[test]
    fn test_binary_add() {
        let ast = AstNode::BinaryOp(
            BinaryOp::Add,
            Box::new(AstNode::Number(2.0)),
            Box::new(AstNode::Number(3.0)),
        );
        let result = TimeDomain.evaluate(&ast, &EvalContext::new()).unwrap();
        assert_eq!(result.as_scalar().unwrap(), 5.0);
    }

    #[test]
    fn test_binary_div_by_zero() {
        let ast = AstNode::BinaryOp(
            BinaryOp::Div,
            Box::new(AstNode::Number(10.0)),
            Box::new(AstNode::Number(0.0)),
        );
        let result = TimeDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::DivisionByZero));
    }

    #[test]
    fn test_unary_neg() {
        let ast = AstNode::UnaryOp(UnaryOp::Neg, Box::new(AstNode::Number(5.0)));
        let result = TimeDomain.evaluate(&ast, &EvalContext::new()).unwrap();
        assert_eq!(result.as_scalar().unwrap(), -5.0);
    }

    #[test]
    fn test_unary_abs() {
        let ast = AstNode::UnaryOp(UnaryOp::Abs, Box::new(AstNode::Number(-5.0)));
        let result = TimeDomain.evaluate(&ast, &EvalContext::new()).unwrap();
        assert_eq!(result.as_scalar().unwrap(), 5.0);
    }

    #[test]
    fn test_unary_factorial_rejected() {
        let ast = AstNode::UnaryOp(UnaryOp::Factorial, Box::new(AstNode::Number(5.0)));
        let result = TimeDomain.evaluate(&ast, &EvalContext::new());
        assert!(matches!(result, Err(e) if e.kind == ErrorKind::Domain));
    }

    // ===== 底层函数单元测试 =====

    #[test]
    fn test_parse_date_multi_format_all_formats() {
        assert!(parse_date_multi_format("2026-07-25").is_ok());
        assert!(parse_date_multi_format("2026/07/25").is_ok());
        assert!(parse_date_multi_format("2026.07.25").is_ok());
        assert!(parse_date_multi_format("20260725").is_ok());
        assert!(parse_date_multi_format("25 Jul 2026").is_ok());
        assert!(parse_date_multi_format("Jul 25, 2026").is_ok());
        assert!(parse_date_multi_format("July 25, 2026").is_ok());
        assert!(parse_date_multi_format("2026年7月25日").is_ok());
    }

    #[test]
    fn test_parse_date_multi_format_ambiguous() {
        assert!(parse_date_multi_format("01/02/2026").is_err());
        assert!(parse_date_multi_format("13/07/2026").is_err());
        assert!(parse_date_multi_format("01-02-2026").is_err());
        assert!(parse_date_multi_format("01.02.2026").is_err());
    }

    #[test]
    fn test_parse_date_multi_format_invalid() {
        assert!(parse_date_multi_format("2026-02-30").is_err());
        assert!(parse_date_multi_format("invalid").is_err());
        assert!(parse_date_multi_format("").is_err());
    }

    #[test]
    fn test_is_leap_year_logic() {
        // 闰年
        assert!(is_leap_year(2024));
        assert!(is_leap_year(2000));
        assert!(is_leap_year(2400));
        // 非闰年
        assert!(!is_leap_year(2026));
        assert!(!is_leap_year(1900));
        assert!(!is_leap_year(2100));
        assert!(!is_leap_year(2023));
    }

    /// 辅助函数：直接判定闰年（用于测试）。
    fn is_leap_year(year: i64) -> bool {
        (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
    }

    // ===== from_timestamp 嵌套测试 =====

    #[test]
    fn test_from_timestamp_nested_in_weekday() {
        // weekday(from_timestamp(0)) = 4（周四）
        let result = eval_scalar("weekday(from_timestamp(0))").unwrap();
        assert_eq!(result, 4.0);
    }

    #[test]
    fn test_from_timestamp_nested_in_date_diff() {
        // date_diff(from_timestamp(0), from_timestamp(86400)) = 1 day
        let result = eval_scalar("date_diff(from_timestamp(0),from_timestamp(86400))").unwrap();
        assert_eq!(result, 1.0);
    }

    // ===== i18n 错误消息测试 =====

    #[test]
    fn test_invalid_date_has_i18n_key() {
        let result = eval(r#"date("2026-02-30")"#);
        let err = result.unwrap_err();
        assert_eq!(err.i18n_key, Some("msg.time.invalid_date"));
    }

    #[test]
    fn test_unknown_timezone_has_i18n_key() {
        let result = eval(r#"datetime("2026-01-01T00:00:00","Not/AZone")"#);
        let err = result.unwrap_err();
        assert_eq!(err.i18n_key, Some("msg.time.unknown_timezone"));
    }

    #[test]
    fn test_invalid_unit_has_i18n_key() {
        let result = eval(r#"date_diff("2026-01-01","2026-07-25","invalid")"#);
        let err = result.unwrap_err();
        assert_eq!(err.i18n_key, Some("msg.time.invalid_unit"));
    }

    #[test]
    fn test_format_mismatch_has_i18n_key() {
        let result = eval(r#"parse_date("invalid","%Y-%m-%d")"#);
        let err = result.unwrap_err();
        assert_eq!(err.i18n_key, Some("msg.time.format_mismatch"));
    }

    #[test]
    fn test_ambiguous_format_has_i18n_key() {
        let result = eval(r#"date("01/02/2026")"#);
        let err = result.unwrap_err();
        assert_eq!(err.i18n_key, Some("msg.time.ambiguous_format"));
    }
}
