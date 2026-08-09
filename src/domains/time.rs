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
use crate::math::time as math_time;

use jiff::civil::Date;
use jiff::fmt::strtime;
use jiff::tz::TimeZone;
use jiff::{Timestamp, Zoned};

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
            let n: f64 = s.parse().map_err(|_| {
                CalcError::domain(format!("invalid big number: {}", s)).with_i18n(
                    "msg.time.invalid_bignumber",
                    vec![("value".to_string(), s.clone())],
                )
            })?;
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
                _ => Err(
                    CalcError::eval(format!("unbound variable: {}", name)).with_i18n(
                        "msg.unbound_variable",
                        vec![("name".to_string(), name.clone())],
                    ),
                ),
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
                    )
                    .with_i18n("msg.time.factorial_not_supported", vec![]))
                }
            };
            if result.is_nan() || result.is_infinite() {
                return Err(CalcError::nan_or_inf());
            }
            Ok(EvalResult::Scalar(result))
        }
        AstNode::Str(_) => Err(CalcError::domain(
            "string operand not supported in time domain binary/unary operations".to_string(),
        )
        .with_i18n("msg.time.string_operand_not_supported", vec![])),
        AstNode::Complex(_, _) | AstNode::Matrix(_) | AstNode::List(_) => Err(CalcError::domain(
            format!("time domain does not support this node type: {:?}", ast),
        )
        .with_i18n(
            "msg.time.unsupported_node",
            vec![("node".to_string(), format!("{:?}", ast))],
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
            let zoned = math_time::parse_str_to_zoned(&s)?;
            Ok(zoned.timestamp().as_second() as f64)
        }
        other => Err(CalcError::domain(format!(
            "time domain expected scalar or datetime, got {:?}",
            other
        ))
        .with_i18n(
            "msg.time.expected_scalar_or_datetime",
            vec![("got".to_string(), format!("{:?}", other))],
        )),
    }
}

// ============================================================================
// 函数分发
// ============================================================================

/// 求值时间函数调用：按函数名分发到对应的处理方法。
fn eval_function(name: &str, args: &[AstNode], ctx: &EvalContext) -> Result<EvalResult, CalcError> {
    if !TIME_FUNCTIONS.contains(&name) {
        return Err(
            CalcError::domain(format!("unsupported function in time domain: {}", name)).with_i18n(
                "msg.unknown_function",
                vec![("name".to_string(), name.to_string())],
            ),
        );
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
        _ => Err(
            CalcError::domain(format!("expected string argument, got {:?}", node)).with_i18n(
                "msg.time.requires_string_arg",
                vec![("node".to_string(), format!("{:?}", node))],
            ),
        ),
    }
}

/// 从 AST 提取整数参数（Number 节点，要求整数值）。
fn extract_i64(node: &AstNode, ctx: &EvalContext) -> Result<i64, CalcError> {
    let v = eval_to_f64(node, ctx)?;
    if v.fract() != 0.0 {
        return Err(
            CalcError::domain(format!("expected integer argument, got {}", v)).with_i18n(
                "msg.time.requires_integer_arg",
                vec![("value".to_string(), v.to_string())],
            ),
        );
    }
    if v > i64::MAX as f64 || v < i64::MIN as f64 {
        return Err(CalcError::overflow());
    }
    Ok(v as i64)
}

/// 解析时区参数：args[idx] 为可选时区字符串，缺省返回 UTC。
fn resolve_timezone(
    args: &[AstNode],
    idx: usize,
    _ctx: &EvalContext,
) -> Result<TimeZone, CalcError> {
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
    let date = math_time::parse_date_multi_format(&input)?;
    let zoned = date
        .to_zoned(TimeZone::UTC)
        .map_err(|_| math_time::invalid_date_error(&input))?;
    Ok(EvalResult::DateTime(math_time::zoned_to_rfc3339(&zoned)))
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
    let datetime = math_time::parse_datetime_multi_format(&input)?;
    let zoned = datetime
        .to_zoned(tz)
        .map_err(|_| math_time::invalid_date_error(&input))?;
    Ok(EvalResult::DateTime(math_time::zoned_to_rfc3339(&zoned)))
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
        .or_else(|_| math_time::parse_timestamp_strptime(&input))
        .map_err(|_| math_time::invalid_date_error(&input))?;
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
    Ok(EvalResult::DateTime(math_time::zoned_to_rfc3339(&zoned)))
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
    let unit = math_time::parse_time_unit(&unit_str)?;
    // 计算 b.since(a) = b - a（spec：a 到 b 的间隔；b 晚于 a 为正）
    let value = math_time::compute_date_diff(&a, &b, unit)?;
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
    let unit = math_time::parse_time_unit(&unit_str)?;
    // 按单位构造 Span 并加法（jiff 公开 API）
    let new_zoned = math_time::compute_date_add(&zoned, n, unit)?;
    Ok(EvalResult::DateTime(math_time::zoned_to_rfc3339(&new_zoned)))
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
                .map_err(|_| math_time::invalid_date_error(&input))?;
            Ok(EvalResult::DateTime(math_time::zoned_to_rfc3339(&zoned)))
        }
        Err(_) => {
            // 失败时尝试解析为 Zoned（含时间/时区）
            match Zoned::strptime(&fmt, &input) {
                Ok(zoned) => Ok(EvalResult::DateTime(math_time::zoned_to_rfc3339(&zoned))),
                Err(_) => Err(math_time::format_mismatch_error(&fmt)),
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
    let formatted = strtime::format(&fmt, &zoned).map_err(|_| math_time::format_mismatch_error(&fmt))?;
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
            .map_err(|_| math_time::invalid_date_error(&input))?,
        Err(_) => match Zoned::strptime(&from_fmt, &input) {
            Ok(z) => z,
            Err(_) => return Err(math_time::format_mismatch_error(&from_fmt)),
        },
    };
    // 格式化阶段
    let formatted = strtime::format(&to_fmt, &zoned).map_err(|_| math_time::format_mismatch_error(&to_fmt))?;
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
    Ok(EvalResult::DateTime(math_time::zoned_to_rfc3339(&now)))
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
    Ok(EvalResult::DateTime(math_time::zoned_to_rfc3339(&today)))
}


// ============================================================================
// 辅助：AST → Date / Zoned 转换
// ============================================================================

/// 将 AST 求值为 Zoned（用于 date_diff / date_add）。
///
/// 接受 Str（日期字符串）或 Scalar（Unix 秒）或 DateTime 结果（嵌套调用）。
fn eval_to_zoned(ast: &AstNode, ctx: &EvalContext) -> Result<Zoned, CalcError> {
    match ast {
        AstNode::Str(s) => math_time::parse_str_to_zoned(s),
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
                EvalResult::DateTime(s) => math_time::parse_str_to_zoned(&s),
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


/// 将 AST 求值为 Date（用于 weekday / day_of_year）。
fn eval_to_date(ast: &AstNode, ctx: &EvalContext) -> Result<Date, CalcError> {
    match ast {
        AstNode::Str(s) => math_time::parse_date_multi_format(s),
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
        assert!(
            result.contains("+08:00"),
            "expected +08:00 offset, got {}",
            result
        );
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
        assert!(
            result > 1.7e9 && result < 1.8e9,
            "expected ~1.78e9, got {}",
            result
        );
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
        assert!(
            result.starts_with("2026-02-28"),
            "expected 2026-02-28, got {}",
            result
        );
    }

    #[test]
    fn test_date_add_month_leap_year() {
        // 2024-01-31 + 1 month = 2024-02-29（闰年 clamp）
        let result = eval_datetime_str(r#"date_add("2024-01-31",1,"month")"#).unwrap();
        assert!(
            result.starts_with("2024-02-29"),
            "expected 2024-02-29, got {}",
            result
        );
    }

    #[test]
    fn test_date_add_negative() {
        let result = eval_datetime_str(r#"date_add("2026-07-25",-1,"day")"#).unwrap();
        assert!(
            result.starts_with("2026-07-24"),
            "expected 2026-07-24, got {}",
            result
        );
    }

    #[test]
    fn test_date_add_days() {
        let result = eval_datetime_str(r#"date_add("2026-01-01",31,"day")"#).unwrap();
        assert!(
            result.starts_with("2026-02-01"),
            "expected 2026-02-01, got {}",
            result
        );
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
        let result = eval_symbolic(r#"reformat_date("25/07/2026","%d/%m/%Y","%Y-%m-%d")"#).unwrap();
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
            math_time::parse_str_to_zoned(&result).is_ok(),
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
        assert!(
            result.contains("T00:00:00"),
            "expected midnight, got {}",
            result
        );
        assert!(
            math_time::parse_str_to_zoned(&result).is_ok(),
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
