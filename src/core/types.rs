// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! CalNexus 共享类型：所有计算域、解析器、缓存、CLI 共用。
//!
//! 设计依据：
//! - proposal.md §Capabilities：v0.1 AstNode 只含 Number/Variable/BinaryOp/UnaryOp/FunctionCall
//! - design.md D1：三 crate 拆分，共享类型放 calnexus-core
//! - ADD.md §3.4 代码图：v0.2 完整 AstNode 含 BigInt/Matrix/Vector，v0.1 暂不实现

use std::collections::HashMap;
use std::fmt;
use std::time::Duration;

/// 精度位数上限（防止 `precision(N, expr)` 表达式语法绕过 server 层校验导致 DoS）。
///
/// 此常量是全项目共享的安全阈值：
/// - `server/types.rs::validate()` 校验请求级 `precision` 字段 ≤ 此值
/// - `evaluator.rs::extract_format_precision()` 校验表达式级 `precision(N, expr)` 的 N ≤ 此值
/// - `domains/precision.rs::extract_precision_value()` 同上，拒绝超大 N 的求值
///
/// 三处校验形成纵深防御：即使绕过 server 层（如直接调用 evaluator），core 层仍拒绝。
pub const MAX_PRECISION: usize = 10_000;

/// 阶乘输入上限（防止 `factorial(N)` / `N!` 循环 DoS）。
///
/// 安全审查 CRITICAL：`factorial(1000000000)` 可在 24 字节请求内永久挂死服务器。
/// 此常量限制阶乘输入，`factorial(10000)` 产生 ~35660 位数字（~35KB 字符串），
/// 兼顾合法重计算场景与 DoS 防护。
pub const MAX_FACTORIAL_INPUT: u64 = 10_000;

/// 幂运算指数上限（防止 `a^b` 产生超大输出 DoS）。
///
/// 安全审查 CRITICAL：`2^2000000000` 可在 17 字节请求内产生 ~6 亿位数字。
/// 此常量限制指数绝对值，`2^100000` 产生 ~30103 位数字（~30KB 字符串）。
///
/// **复审 C-1 修复**：负指数同样受限。`BigRational::pow(neg_i32)` 内部实现为
/// `Pow::pow(self, (-exp) as u64).reciprocal()`，即先计算 `a^|exp|`（巨大中间值）
/// 再取倒数。`2^(-2000000000)` 会计算 `2^2000000000`（~6 亿位数字）导致内存爆炸。
pub const MAX_POW_EXPONENT: u64 = 100_000;

/// 幂运算输出上限（bits），防止大底数 × 大指数产生超大输出 DoS。
///
/// 安全审查 HIGH（C-1 复审发现）：`(10^10000) ^ 99999` 可产生 ~1GB 输出。
/// 指数已受 `MAX_POW_EXPONENT` 约束，但底数 `a` 可为任意大小 BigInt。
/// 此常量限制 `底数bits × |指数|`，对应 ~1_000_000 位十进制数字（~1MB 输出）。
/// `2^100000` 的底数 2 的 bits=2（二进制 `10`），2×100000=200000 ≤ 3320000，通过。
/// `(10^10000)^99999` 的底数 bits≈33219，33219×99999≈3.3e9 > 3320000，拒绝。
pub const MAX_POW_OUTPUT_BITS: u64 = 3_320_000;

/// 检查幂运算输出大小是否在安全范围内（底数复合限制）。
///
/// 复合限制公式：`底数bits × |指数| ≤ MAX_POW_OUTPUT_BITS`。
///
/// 安全审查 HIGH（C-1 复审发现）：`(10^10000) ^ 99999` 可产生 ~1GB 输出。
/// 指数已受 `MAX_POW_EXPONENT` 约束（≤ 100_000），但底数 `a` 可为任意大小 BigInt，
/// 故需对"底数位数 × 指数绝对值"做复合限制。
///
/// - `2^100000`：底数 bits=2，2×100000=200000 ≤ 3_320_000，通过。
/// - `(10^10000)^99999`：底数 bits≈33219，33219×99999≈3.3e9 > 3_320_000，拒绝。
///
/// 此函数是全项目共享的安全检查（precision/number_theory/combinatorics 三域共用），
/// 避免每个域重复实现。提取到 core 层符合 DRY 原则。
pub fn check_pow_output_size(base_bits: u64, abs_exp: u64) -> Result<(), CalcError> {
    // 估算输出 bits：底数bits × |指数|（saturating 防止溢出）
    let estimated_output_bits = base_bits.saturating_mul(abs_exp);
    if estimated_output_bits > MAX_POW_OUTPUT_BITS {
        return Err(CalcError::domain(format!(
            "pow output too large: base_bits {} × exp {} = {} > {} (limit)",
            base_bits, abs_exp, estimated_output_bits, MAX_POW_OUTPUT_BITS
        ))
        .with_i18n(
            "msg.pow_output_too_large",
            vec![
                ("base_bits".to_string(), base_bits.to_string()),
                ("exp".to_string(), abs_exp.to_string()),
                ("output".to_string(), estimated_output_bits.to_string()),
                ("limit".to_string(), MAX_POW_OUTPUT_BITS.to_string()),
            ],
        ));
    }
    Ok(())
}

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
    /// 向量结果（v0.8 新增）：向量域 cross/normalize 输出、素数筛、实根列表。
    Vector(Vec<f64>),
    /// 多项式结果（v0.8 新增）：系数向量，升幂存储（coef[i] 为 x^i 的系数）。
    Polynomial(Vec<f64>),
    /// 复数列表结果（v0.8 新增）：复根列表，元素为 (实部, 虚部)。
    ComplexList(Vec<(f64, f64)>),
    /// 符号字符串结果（v0.8 新增）：因式分解等符号输出。
    Symbolic(String),
    /// LaTeX 渲染结果（v1.1 新增，ADD §3.4）：已格式化的 LaTeX 字符串。
    LaTeX(String),
    /// 求值步骤列表（v1.1 新增，ADD §3.4）：每行一步 `lhs op rhs = result`。
    Steps(Vec<String>),
    /// JSON 复合结果（p4 新增，numerical-linalg）：lu/qr/eig/svd 分解的多矩阵结构化返回。
    /// 持有 serde_json::Value；eval_result_to_json 直接透传，typed 访问器返回 None。
    Json(serde_json::Value),
}

impl EvalResult {
    /// 获取标量值，若非 Scalar 返回 None。
    pub fn as_scalar(&self) -> Option<f64> {
        match self {
            EvalResult::Scalar(v) => Some(*v),
            EvalResult::Complex(_, _)
            | EvalResult::Matrix(_)
            | EvalResult::BigInt(_)
            | EvalResult::BigRational(_)
            | EvalResult::Vector(_)
            | EvalResult::Polynomial(_)
            | EvalResult::ComplexList(_)
            | EvalResult::Symbolic(_)
            | EvalResult::LaTeX(_)
            | EvalResult::Steps(_)
            | EvalResult::Json(_) => None,
        }
    }

    /// 获取复数值 (re, im)，若非 Complex 返回 None。
    pub fn as_complex(&self) -> Option<(f64, f64)> {
        match self {
            EvalResult::Complex(re, im) => Some((*re, *im)),
            EvalResult::Scalar(_)
            | EvalResult::Matrix(_)
            | EvalResult::BigInt(_)
            | EvalResult::BigRational(_)
            | EvalResult::Vector(_)
            | EvalResult::Polynomial(_)
            | EvalResult::ComplexList(_)
            | EvalResult::Symbolic(_)
            | EvalResult::LaTeX(_)
            | EvalResult::Steps(_)
            | EvalResult::Json(_) => None,
        }
    }

    /// 获取矩阵引用，若非 Matrix 返回 None。
    pub fn as_matrix(&self) -> Option<&Vec<Vec<f64>>> {
        match self {
            EvalResult::Matrix(m) => Some(m),
            EvalResult::Scalar(_)
            | EvalResult::Complex(_, _)
            | EvalResult::BigInt(_)
            | EvalResult::BigRational(_)
            | EvalResult::Vector(_)
            | EvalResult::Polynomial(_)
            | EvalResult::ComplexList(_)
            | EvalResult::Symbolic(_)
            | EvalResult::LaTeX(_)
            | EvalResult::Steps(_)
            | EvalResult::Json(_) => None,
        }
    }

    /// 获取大整数引用，若非 BigInt 返回 None。
    pub fn as_bigint(&self) -> Option<&num_bigint::BigInt> {
        match self {
            EvalResult::BigInt(b) => Some(b),
            EvalResult::Scalar(_)
            | EvalResult::Complex(_, _)
            | EvalResult::Matrix(_)
            | EvalResult::BigRational(_)
            | EvalResult::Vector(_)
            | EvalResult::Polynomial(_)
            | EvalResult::ComplexList(_)
            | EvalResult::Symbolic(_)
            | EvalResult::LaTeX(_)
            | EvalResult::Steps(_)
            | EvalResult::Json(_) => None,
        }
    }

    /// 获取分数引用，若非 BigRational 返回 None。
    pub fn as_bigrational(&self) -> Option<&num_rational::BigRational> {
        match self {
            EvalResult::BigRational(r) => Some(r),
            EvalResult::Scalar(_)
            | EvalResult::Complex(_, _)
            | EvalResult::Matrix(_)
            | EvalResult::BigInt(_)
            | EvalResult::Vector(_)
            | EvalResult::Polynomial(_)
            | EvalResult::ComplexList(_)
            | EvalResult::Symbolic(_)
            | EvalResult::LaTeX(_)
            | EvalResult::Steps(_)
            | EvalResult::Json(_) => None,
        }
    }

    /// 获取向量引用，若非 Vector 返回 None。
    pub fn as_vector(&self) -> Option<&Vec<f64>> {
        match self {
            EvalResult::Vector(v) => Some(v),
            EvalResult::Scalar(_)
            | EvalResult::Complex(_, _)
            | EvalResult::Matrix(_)
            | EvalResult::BigInt(_)
            | EvalResult::BigRational(_)
            | EvalResult::Polynomial(_)
            | EvalResult::ComplexList(_)
            | EvalResult::Symbolic(_)
            | EvalResult::LaTeX(_)
            | EvalResult::Steps(_)
            | EvalResult::Json(_) => None,
        }
    }

    /// 获取多项式系数向量引用，若非 Polynomial 返回 None。
    pub fn as_polynomial(&self) -> Option<&Vec<f64>> {
        match self {
            EvalResult::Polynomial(p) => Some(p),
            EvalResult::Scalar(_)
            | EvalResult::Complex(_, _)
            | EvalResult::Matrix(_)
            | EvalResult::BigInt(_)
            | EvalResult::BigRational(_)
            | EvalResult::Vector(_)
            | EvalResult::ComplexList(_)
            | EvalResult::Symbolic(_)
            | EvalResult::LaTeX(_)
            | EvalResult::Steps(_)
            | EvalResult::Json(_) => None,
        }
    }

    /// 获取复数列表引用，若非 ComplexList 返回 None。
    pub fn as_complex_list(&self) -> Option<&Vec<(f64, f64)>> {
        match self {
            EvalResult::ComplexList(c) => Some(c),
            EvalResult::Scalar(_)
            | EvalResult::Complex(_, _)
            | EvalResult::Matrix(_)
            | EvalResult::BigInt(_)
            | EvalResult::BigRational(_)
            | EvalResult::Vector(_)
            | EvalResult::Polynomial(_)
            | EvalResult::Symbolic(_)
            | EvalResult::LaTeX(_)
            | EvalResult::Steps(_)
            | EvalResult::Json(_) => None,
        }
    }

    /// 获取符号字符串引用，若非 Symbolic 返回 None。
    pub fn as_symbolic(&self) -> Option<&String> {
        match self {
            EvalResult::Symbolic(s) => Some(s),
            EvalResult::Scalar(_)
            | EvalResult::Complex(_, _)
            | EvalResult::Matrix(_)
            | EvalResult::BigInt(_)
            | EvalResult::BigRational(_)
            | EvalResult::Vector(_)
            | EvalResult::Polynomial(_)
            | EvalResult::ComplexList(_)
            | EvalResult::LaTeX(_)
            | EvalResult::Steps(_)
            | EvalResult::Json(_) => None,
        }
    }

    /// 获取 LaTeX 字符串引用，若非 LaTeX 返回 None。
    pub fn as_latex(&self) -> Option<&String> {
        match self {
            EvalResult::LaTeX(s) => Some(s),
            EvalResult::Scalar(_)
            | EvalResult::Complex(_, _)
            | EvalResult::Matrix(_)
            | EvalResult::BigInt(_)
            | EvalResult::BigRational(_)
            | EvalResult::Vector(_)
            | EvalResult::Polynomial(_)
            | EvalResult::ComplexList(_)
            | EvalResult::Symbolic(_)
            | EvalResult::Steps(_)
            | EvalResult::Json(_) => None,
        }
    }

    /// 获取步骤列表引用，若非 Steps 返回 None。
    pub fn as_steps(&self) -> Option<&Vec<String>> {
        match self {
            EvalResult::Steps(v) => Some(v),
            EvalResult::Scalar(_)
            | EvalResult::Complex(_, _)
            | EvalResult::Matrix(_)
            | EvalResult::BigInt(_)
            | EvalResult::BigRational(_)
            | EvalResult::Vector(_)
            | EvalResult::Polynomial(_)
            | EvalResult::ComplexList(_)
            | EvalResult::Symbolic(_)
            | EvalResult::LaTeX(_)
            | EvalResult::Json(_) => None,
        }
    }
}

/// 源代码跨度（字符偏移）。
///
/// 用于精确定位错误在输入表达式中的位置。design.md §5.1（D1：字符偏移语义）。
/// 注意：`str::len()` 返回字节长度，`str::chars().count()` 返回字符数量；
/// 多字节 UTF-8 字符（如中文）的 Span 必须用字符偏移，否则位置错误。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Span {
    /// 起始字符偏移（含）。
    pub start: usize,
    /// 结束字符偏移（不含）。
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
    pub fn point(pos: usize) -> Self {
        Self {
            start: pos,
            end: pos + 1,
        }
    }
    pub fn is_empty(&self) -> bool {
        self.start >= self.end
    }
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.start, self.end)
    }
}

/// 错误分类，决定退出码和呈现策略。design.md §5.2。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorKind {
    Parse,
    Eval,
    Overflow,
    DivisionByZero,
    Domain,
    Depth,
    NaNOrInf,
    UndefinedSymbol,
    Timeout,
    Usage,
}

impl ErrorKind {
    /// 退出码契约：0=成功, 1=计算错误, 2=用法错误, 3=超时。design.md §5.6。
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Timeout => 3,
            Self::Usage => 2,
            _ => 1,
        }
    }

    /// i18n 消息键，对应 i18n.rs 中的翻译条目。
    pub fn i18n_key(&self) -> &'static str {
        match self {
            Self::Parse => "error.parse",
            Self::Eval => "error.eval",
            Self::Overflow => "error.overflow",
            Self::DivisionByZero => "error.division_by_zero",
            Self::Domain => "error.domain",
            Self::Depth => "error.depth",
            Self::NaNOrInf => "error.nan_or_inf",
            Self::UndefinedSymbol => "error.undefined_symbol",
            Self::Timeout => "error.timeout",
            Self::Usage => "error.usage",
        }
    }
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.i18n_key())
    }
}

/// 计算错误。结构化设计：kind + message + span + hint + i18n。design.md §5.3。
///
/// 覆盖解析、求值、溢出、除零、定义域、深度、NaN/Inf 等所有错误路径。
/// design.md D7 要求错误必须显性化（Rule 12: Fail Loud）。
///
/// i18n 字段（Phase 4.3 新增，方案 B 最小侵入）：
/// - `i18n_key`: 参数化消息键（如 `"msg.unbound_variable"`），指向 `locales/{en,zh}.json`
/// - `i18n_args`: 占位符参数（如 `[("name", "x")]`），用于 `I18n::tf()` 替换 `{name}`
///
/// 当 `i18n_key` 为 `None` 时，`friendly()` 回退到 `message`（英文硬编码）。
/// `Display` impl 和 `to_json()` 始终用 `message`，保证机器可读契约不变（DP-1/DP-4）。
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub struct CalcError {
    pub kind: ErrorKind,
    pub message: String,
    pub span: Option<Span>,
    pub hint: Option<String>,
    pub i18n_key: Option<&'static str>,
    pub i18n_args: Vec<(String, String)>,
}

impl fmt::Display for CalcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            ErrorKind::Parse => write!(f, "parse error: {}", self.message),
            ErrorKind::Eval => write!(f, "evaluation error: {}", self.message),
            ErrorKind::Overflow => write!(f, "integer overflow"),
            ErrorKind::NaNOrInf => write!(f, "result is NaN or infinity"),
            ErrorKind::Domain => write!(f, "domain error: {}", self.message),
            ErrorKind::Depth => write!(f, "AST depth exceeded limit"),
            ErrorKind::DivisionByZero => write!(f, "division by zero"),
            ErrorKind::UndefinedSymbol => write!(f, "{}", self.message),
            ErrorKind::Timeout => write!(f, "{}", self.message),
            ErrorKind::Usage => write!(f, "{}", self.message),
        }
    }
}

impl CalcError {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            span: None,
            hint: None,
            i18n_key: None,
            i18n_args: Vec::new(),
        }
    }
    pub fn with_span(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// 附加国际化消息键 + 参数（链式）。
    ///
    /// 设置后，`friendly()` 和 `to_explain()` 会用 `I18n::tf(key, args)` 替换 `message`
    /// 部分（kind 前缀保留）。`Display` impl 和 `to_json()` 始终用 `message`，保证
    /// 机器可读契约不变（DP-1/DP-4）。
    ///
    /// # 示例
    /// ```ignore
    /// CalcError::eval(format!("unbound variable: {}", name))
    ///     .with_i18n("msg.unbound_variable", vec![("name".to_string(), name.to_string())])
    /// ```
    pub fn with_i18n(
        mut self,
        key: &'static str,
        args: Vec<(String, String)>,
    ) -> Self {
        self.i18n_key = Some(key);
        self.i18n_args = args;
        self
    }

    // 便捷构造器（签名与旧 enum 变体兼容，迁移用）
    pub fn parse(msg: impl Into<String>) -> Self {
        Self::new(ErrorKind::Parse, msg)
    }
    pub fn eval(msg: impl Into<String>) -> Self {
        Self::new(ErrorKind::Eval, msg)
    }
    pub fn overflow() -> Self {
        Self::new(ErrorKind::Overflow, "integer overflow")
            .with_i18n("detail.overflow", vec![])
    }
    pub fn nan_or_inf() -> Self {
        Self::new(ErrorKind::NaNOrInf, "result is NaN or infinity")
            .with_i18n("detail.nan_or_inf", vec![])
    }
    pub fn domain(msg: impl Into<String>) -> Self {
        Self::new(ErrorKind::Domain, msg)
    }
    pub fn depth_exceeded() -> Self {
        Self::new(ErrorKind::Depth, "AST depth exceeded limit")
            .with_hint("simplify nested expressions (max 256)")
            .with_i18n("detail.depth_exceeded", vec![])
    }
    pub fn division_by_zero() -> Self {
        Self::new(ErrorKind::DivisionByZero, "division by zero")
            .with_hint("check divisor before division")
            .with_i18n("detail.division_by_zero", vec![])
    }
    pub fn undefined_symbol(name: &str) -> Self {
        Self::new(
            ErrorKind::UndefinedSymbol,
            format!("undefined symbol: {}", name),
        )
        .with_hint(format!("try defining it first: :let {} = <value>", name))
        .with_i18n(
            "msg.undefined_symbol",
            vec![("name".to_string(), name.to_string())],
        )
    }
    pub fn timeout() -> Self {
        Self::new(ErrorKind::Timeout, "evaluation timed out")
            .with_hint("increase --timeout or simplify expression")
            .with_i18n("detail.timeout", vec![])
    }
    pub fn usage(msg: impl Into<String>) -> Self {
        Self::new(ErrorKind::Usage, msg)
    }

    /// 中文友好文本（终端默认）。design.md §5.5。
    ///
    /// Phase 4.3: 当 `i18n_key` 存在时，detail 部分用 `I18n::tf(key, args)` 替换
    /// `message`，实现参数化消息国际化。kind 前缀始终用 `ErrorKind::i18n_key()`。
    /// `Display` impl 和 `to_json()` 不受影响（始终用 `message`，机器可读契约不变）。
    pub fn friendly(&self, i18n: &crate::i18n::I18n) -> String {
        let mut s = i18n.t(self.kind.i18n_key()).to_string();
        if let Some(span) = self.span {
            s.push_str(&format!(
                " ({} {}:{})",
                i18n.t("label.position"),
                span.start,
                span.end
            ));
        }
        // 优先用 i18n_key + i18n_args（参数化消息），fallback 到 message（英文硬编码）
        let detail = if let Some(key) = self.i18n_key {
            let args_ref: Vec<(&str, &str)> = self
                .i18n_args
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect();
            i18n.tf(key, &args_ref)
        } else {
            self.message.clone()
        };
        s.push_str(&format!(": {}", detail));
        if let Some(hint) = &self.hint {
            s.push_str(&format!("\n  {}: {}", i18n.t("label.hint"), hint));
        }
        s
    }

    /// JSON 机器可读（--json）。手动构造避免 serde_json 运行时依赖。
    pub fn to_json(&self) -> String {
        let span = match &self.span {
            Some(s) => format!(r#","span":{{"start":{},"end":{}}}"#, s.start, s.end),
            None => String::new(),
        };
        let hint = match &self.hint {
            Some(h) => format!(r#","hint":"{}""#, escape_json_string(h)),
            None => String::new(),
        };
        format!(
            r#"{{"error":{{"kind":"{:?}","message":"{}"{}{},"exit_code":{}}}}}"#,
            self.kind,
            escape_json_string(&self.message),
            span,
            hint,
            self.kind.exit_code()
        )
    }

    /// 教育模式（--explain）。design.md §5.5。
    pub fn to_explain(&self, i18n: &crate::i18n::I18n) -> String {
        let mut s = self.friendly(i18n);
        s.push_str(&format!(
            "\n\n  {}: {:?}",
            i18n.t("label.error_kind"),
            self.kind
        ));
        s.push_str(&format!(
            "\n  {}: {}",
            i18n.t("label.exit_code"),
            self.kind.exit_code()
        ));
        if let Some(hint) = &self.hint {
            s.push_str(&format!("\n  {}: {}", i18n.t("label.suggestion"), hint));
        }
        s
    }
}

pub fn escape_json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str(r#"\""#),
            '\\' => out.push_str(r"\\"),
            '\n' => out.push_str(r"\n"),
            '\r' => out.push_str(r"\r"),
            '\t' => out.push_str(r"\t"),
            c if c.is_control() => out.push_str(&format!(r"\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

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
        let AstNode::Number(v) = node else {
            panic!("expected Number variant")
        };
        assert!((v - 3.14).abs() < f64::EPSILON);
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
        let AstNode::BinaryOp(BinaryOp::Add, l, r) = node else {
            panic!("expected BinaryOp variant")
        };
        assert_eq!(*l, AstNode::Number(2.0));
        assert_eq!(*r, AstNode::Number(3.0));
    }

    #[test]
    fn ast_node_unary_op_construct() {
        let expr = Box::new(AstNode::Number(5.0));
        let node = AstNode::UnaryOp(UnaryOp::Neg, expr);
        let AstNode::UnaryOp(UnaryOp::Neg, inner) = node else {
            panic!("expected UnaryOp variant")
        };
        assert_eq!(*inner, AstNode::Number(5.0));
    }

    #[test]
    fn ast_node_function_call_construct() {
        let node =
            AstNode::FunctionCall("sin".to_string(), vec![AstNode::Variable("x".to_string())]);
        let AstNode::FunctionCall(name, args) = node else {
            panic!("expected FunctionCall variant")
        };
        assert_eq!(name, "sin");
        assert_eq!(args.len(), 1);
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
            CalcError::parse("unexpected token").to_string(),
            "parse error: unexpected token"
        );
        assert_eq!(CalcError::overflow().to_string(), "integer overflow");
        assert_eq!(
            CalcError::nan_or_inf().to_string(),
            "result is NaN or infinity"
        );
        assert_eq!(
            CalcError::domain("asin(2)").to_string(),
            "domain error: asin(2)"
        );
        assert_eq!(
            CalcError::depth_exceeded().to_string(),
            "AST depth exceeded limit"
        );
        assert_eq!(
            CalcError::division_by_zero().to_string(),
            "division by zero"
        );
        assert_eq!(
            CalcError::eval("unknown").to_string(),
            "evaluation error: unknown"
        );
    }

    #[test]
    fn calc_error_eq() {
        assert_eq!(CalcError::overflow(), CalcError::overflow());
        assert_ne!(CalcError::overflow(), CalcError::division_by_zero());
        assert_eq!(CalcError::parse("e1"), CalcError::parse("e1"));
    }

    // ===== Span 测试 =====

    #[test]
    fn span_new_and_accessors() {
        let s = Span::new(3, 7);
        assert_eq!(s.start, 3);
        assert_eq!(s.end, 7);
        assert!(!s.is_empty());

        let empty = Span::new(5, 5);
        assert!(empty.is_empty());

        let empty2 = Span::new(5, 3);
        assert!(empty2.is_empty());
    }

    #[test]
    fn span_point() {
        let p = Span::point(10);
        assert_eq!(p.start, 10);
        assert_eq!(p.end, 11);
        assert!(!p.is_empty());
    }

    #[test]
    fn span_display() {
        assert_eq!(Span::new(3, 7).to_string(), "3:7");
        assert_eq!(Span::point(0).to_string(), "0:1");
    }

    #[test]
    fn span_default() {
        let s = Span::default();
        assert_eq!(s.start, 0);
        assert_eq!(s.end, 0);
        assert!(s.is_empty());
    }

    // ===== ErrorKind 测试 =====

    #[test]
    fn error_kind_exit_code_all_variants() {
        assert_eq!(ErrorKind::Parse.exit_code(), 1);
        assert_eq!(ErrorKind::Eval.exit_code(), 1);
        assert_eq!(ErrorKind::Overflow.exit_code(), 1);
        assert_eq!(ErrorKind::DivisionByZero.exit_code(), 1);
        assert_eq!(ErrorKind::Domain.exit_code(), 1);
        assert_eq!(ErrorKind::Depth.exit_code(), 1);
        assert_eq!(ErrorKind::NaNOrInf.exit_code(), 1);
        assert_eq!(ErrorKind::UndefinedSymbol.exit_code(), 1);
        assert_eq!(ErrorKind::Usage.exit_code(), 2);
        assert_eq!(ErrorKind::Timeout.exit_code(), 3);
    }

    #[test]
    fn error_kind_i18n_key_all_variants() {
        assert_eq!(ErrorKind::Parse.i18n_key(), "error.parse");
        assert_eq!(ErrorKind::Eval.i18n_key(), "error.eval");
        assert_eq!(ErrorKind::Overflow.i18n_key(), "error.overflow");
        assert_eq!(
            ErrorKind::DivisionByZero.i18n_key(),
            "error.division_by_zero"
        );
        assert_eq!(ErrorKind::Domain.i18n_key(), "error.domain");
        assert_eq!(ErrorKind::Depth.i18n_key(), "error.depth");
        assert_eq!(ErrorKind::NaNOrInf.i18n_key(), "error.nan_or_inf");
        assert_eq!(
            ErrorKind::UndefinedSymbol.i18n_key(),
            "error.undefined_symbol"
        );
        assert_eq!(ErrorKind::Timeout.i18n_key(), "error.timeout");
        assert_eq!(ErrorKind::Usage.i18n_key(), "error.usage");
    }

    #[test]
    fn error_kind_display() {
        assert_eq!(ErrorKind::Parse.to_string(), "error.parse");
        assert_eq!(ErrorKind::Timeout.to_string(), "error.timeout");
    }

    // ===== CalcError 构造器测试 =====

    #[test]
    fn calc_error_all_constructors() {
        let e = CalcError::parse("bad syntax");
        assert_eq!(e.kind, ErrorKind::Parse);
        assert_eq!(e.message, "bad syntax");
        assert!(e.span.is_none());
        assert!(e.hint.is_none());

        let e = CalcError::eval("runtime issue");
        assert_eq!(e.kind, ErrorKind::Eval);
        assert_eq!(e.message, "runtime issue");

        let e = CalcError::overflow();
        assert_eq!(e.kind, ErrorKind::Overflow);
        assert_eq!(e.message, "integer overflow");

        let e = CalcError::nan_or_inf();
        assert_eq!(e.kind, ErrorKind::NaNOrInf);
        assert_eq!(e.message, "result is NaN or infinity");

        let e = CalcError::domain("asin(2)");
        assert_eq!(e.kind, ErrorKind::Domain);
        assert_eq!(e.message, "asin(2)");

        let e = CalcError::depth_exceeded();
        assert_eq!(e.kind, ErrorKind::Depth);
        assert_eq!(e.message, "AST depth exceeded limit");
        assert_eq!(
            e.hint.as_deref(),
            Some("simplify nested expressions (max 256)")
        );

        let e = CalcError::division_by_zero();
        assert_eq!(e.kind, ErrorKind::DivisionByZero);
        assert_eq!(e.message, "division by zero");
        assert_eq!(e.hint.as_deref(), Some("check divisor before division"));

        let e = CalcError::undefined_symbol("foo");
        assert_eq!(e.kind, ErrorKind::UndefinedSymbol);
        assert_eq!(e.message, "undefined symbol: foo");
        assert!(e.hint.is_some());
        assert!(e.hint.as_ref().unwrap().contains("foo"));

        let e = CalcError::timeout();
        assert_eq!(e.kind, ErrorKind::Timeout);
        assert_eq!(e.message, "evaluation timed out");
        assert_eq!(
            e.hint.as_deref(),
            Some("increase --timeout or simplify expression")
        );

        let e = CalcError::usage("invalid flag");
        assert_eq!(e.kind, ErrorKind::Usage);
        assert_eq!(e.message, "invalid flag");
    }

    #[test]
    fn calc_error_with_span_chaining() {
        let e = CalcError::parse("unexpected token").with_span(Span::new(5, 7));
        assert_eq!(e.kind, ErrorKind::Parse);
        assert_eq!(e.span, Some(Span::new(5, 7)));
    }

    #[test]
    fn calc_error_with_hint_chaining() {
        let e = CalcError::division_by_zero().with_hint("check divisor");
        assert_eq!(e.kind, ErrorKind::DivisionByZero);
        assert_eq!(e.hint.as_deref(), Some("check divisor"));
    }

    #[test]
    fn calc_error_with_span_and_hint_chaining() {
        let e = CalcError::domain("asin(2)")
            .with_span(Span::point(0))
            .with_hint("asin domain is [-1, 1]");
        assert_eq!(e.span, Some(Span::point(0)));
        assert_eq!(e.hint.as_deref(), Some("asin domain is [-1, 1]"));
    }

    #[test]
    fn calc_error_display_all_kinds() {
        assert_eq!(CalcError::parse("msg").to_string(), "parse error: msg");
        assert_eq!(CalcError::eval("msg").to_string(), "evaluation error: msg");
        assert_eq!(CalcError::overflow().to_string(), "integer overflow");
        assert_eq!(
            CalcError::nan_or_inf().to_string(),
            "result is NaN or infinity"
        );
        assert_eq!(CalcError::domain("msg").to_string(), "domain error: msg");
        assert_eq!(
            CalcError::depth_exceeded().to_string(),
            "AST depth exceeded limit"
        );
        assert_eq!(
            CalcError::division_by_zero().to_string(),
            "division by zero"
        );
        assert_eq!(
            CalcError::undefined_symbol("foo").to_string(),
            "undefined symbol: foo"
        );
        assert_eq!(CalcError::timeout().to_string(), "evaluation timed out");
        assert_eq!(CalcError::usage("bad flag").to_string(), "bad flag");
    }

    // ===== 三态呈现测试 =====

    #[test]
    fn calc_error_friendly_en_without_span_hint() {
        let i18n = crate::i18n::I18n::new(crate::i18n::Lang::En);
        let e = CalcError::parse("unexpected token");
        let friendly = e.friendly(&i18n);
        assert!(friendly.contains("Parse error"));
        assert!(friendly.contains("unexpected token"));
    }

    #[test]
    fn calc_error_friendly_zh_with_span_and_hint() {
        let i18n = crate::i18n::I18n::new(crate::i18n::Lang::Zh);
        let e = CalcError::domain("asin(2)")
            .with_span(Span::new(0, 6))
            .with_hint("asin domain is [-1, 1]");
        let friendly = e.friendly(&i18n);
        assert!(friendly.contains("定义域错误"));
        assert!(friendly.contains("位置 0:6"));
        assert!(friendly.contains("asin(2)"));
        assert!(friendly.contains("提示"));
        assert!(friendly.contains("asin domain is [-1, 1]"));
    }

    #[test]
    fn calc_error_to_json_without_span_hint() {
        let e = CalcError::parse("bad token");
        let json = e.to_json();
        assert!(json.contains(r#""kind":"Parse""#));
        assert!(json.contains(r#""message":"bad token""#));
        assert!(json.contains(r#""exit_code":1"#));
        assert!(!json.contains(r#""span""#));
        assert!(!json.contains(r#""hint""#));
    }

    #[test]
    fn calc_error_to_json_with_span_and_hint() {
        let e = CalcError::domain("asin(2)")
            .with_span(Span::new(0, 6))
            .with_hint("check domain");
        let json = e.to_json();
        assert!(json.contains(r#""kind":"Domain""#));
        assert!(json.contains(r#""message":"asin(2)""#));
        assert!(json.contains(r#""span":{"start":0,"end":6}"#));
        assert!(json.contains(r#""hint":"check domain""#));
        assert!(json.contains(r#""exit_code":1"#));
    }

    #[test]
    fn calc_error_to_json_escapes_quotes() {
        let e = CalcError::parse(r#"bad "token""#);
        let json = e.to_json();
        assert!(json.contains(r#"\"token\""#));
    }

    #[test]
    fn calc_error_to_json_timeout_exit_code() {
        let e = CalcError::timeout();
        let json = e.to_json();
        assert!(json.contains(r#""exit_code":3"#));
    }

    #[test]
    fn calc_error_to_json_usage_exit_code() {
        let e = CalcError::usage("bad flag");
        let json = e.to_json();
        assert!(json.contains(r#""exit_code":2"#));
    }

    #[test]
    fn calc_error_to_explain_en() {
        let i18n = crate::i18n::I18n::new(crate::i18n::Lang::En);
        let e = CalcError::division_by_zero().with_hint("check divisor");
        let explain = e.to_explain(&i18n);
        assert!(explain.contains("Division by zero"));
        // T002 diting HIGH-1 修复：--lang en 时所有标签应为英文，不得混入中文
        assert!(explain.contains("Error Kind: DivisionByZero"));
        assert!(explain.contains("Exit Code: 1"));
        assert!(explain.contains("Suggestion: check divisor"));
        // 反向断言：不应含任何中文标签
        assert!(!explain.contains("错误类别"));
        assert!(!explain.contains("退出码"));
        assert!(!explain.contains("建议"));
    }

    #[test]
    fn calc_error_to_explain_zh() {
        let i18n = crate::i18n::I18n::new(crate::i18n::Lang::Zh);
        let e = CalcError::timeout();
        let explain = e.to_explain(&i18n);
        assert!(explain.contains("求值超时"));
        assert!(explain.contains("错误类别: Timeout"));
        assert!(explain.contains("退出码: 3"));
    }

    // ===== Phase 4.3: i18n_key + i18n_args 字段测试 =====

    #[test]
    fn calc_error_new_has_no_i18n_key() {
        let e = CalcError::parse("test");
        assert!(e.i18n_key.is_none());
        assert!(e.i18n_args.is_empty());
    }

    #[test]
    fn calc_error_with_i18n_sets_key_and_args() {
        let e = CalcError::eval("unbound variable: x")
            .with_i18n("msg.unbound_variable", vec![("name".to_string(), "x".to_string())]);
        assert_eq!(e.i18n_key, Some("msg.unbound_variable"));
        assert_eq!(e.i18n_args, vec![("name".to_string(), "x".to_string())]);
    }

    #[test]
    fn calc_error_with_i18n_chains_with_span_and_hint() {
        let e = CalcError::domain("test")
            .with_span(Span::new(0, 4))
            .with_hint("hint")
            .with_i18n("msg.unknown_function", vec![("name".to_string(), "sin".to_string())]);
        assert_eq!(e.span, Some(Span::new(0, 4)));
        assert_eq!(e.hint.as_deref(), Some("hint"));
        assert_eq!(e.i18n_key, Some("msg.unknown_function"));
        assert_eq!(e.i18n_args, vec![("name".to_string(), "sin".to_string())]);
    }

    #[test]
    fn calc_error_friendly_uses_i18n_key_when_set_en() {
        let i18n = crate::i18n::I18n::new(crate::i18n::Lang::En);
        let e = CalcError::eval("unbound variable: x")
            .with_i18n("msg.unbound_variable", vec![("name".to_string(), "x".to_string())]);
        let friendly = e.friendly(&i18n);
        // kind 前缀（英文）
        assert!(friendly.contains("Evaluation error"));
        // 参数化消息（英文，替换占位符）
        assert!(friendly.contains("Unbound variable: x"));
        // 不应包含原始英文 message（被 i18n 替换）
        assert!(!friendly.contains("unbound variable: x"));
    }

    #[test]
    fn calc_error_friendly_uses_i18n_key_when_set_zh() {
        let i18n = crate::i18n::I18n::new(crate::i18n::Lang::Zh);
        let e = CalcError::eval("unbound variable: x")
            .with_i18n("msg.unbound_variable", vec![("name".to_string(), "x".to_string())]);
        let friendly = e.friendly(&i18n);
        // kind 前缀（中文）
        assert!(friendly.contains("求值错误"));
        // 参数化消息（中文，替换占位符）
        assert!(friendly.contains("未绑定变量: x"));
        // 不应包含原始英文 message
        assert!(!friendly.contains("unbound variable: x"));
    }

    #[test]
    fn calc_error_friendly_falls_back_to_message_when_no_i18n_key() {
        let i18n = crate::i18n::I18n::new(crate::i18n::Lang::Zh);
        let e = CalcError::domain("asin(2)");
        let friendly = e.friendly(&i18n);
        // i18n_key=None 时 fallback 到 message
        assert!(friendly.contains("asin(2)"));
    }

    #[test]
    fn calc_error_friendly_multi_placeholder_i18n_en() {
        let i18n = crate::i18n::I18n::new(crate::i18n::Lang::En);
        let e = CalcError::domain("matrix dimension mismatch: 3x3 vs 2x2")
            .with_i18n(
                "msg.matrix_dim_mismatch",
                vec![
                    ("expected".to_string(), "3x3".to_string()),
                    ("actual".to_string(), "2x2".to_string()),
                ],
            );
        let friendly = e.friendly(&i18n);
        assert!(friendly.contains("Domain error"));
        assert!(friendly.contains("Matrix dimension mismatch: expected 3x3, got 2x2"));
    }

    #[test]
    fn calc_error_friendly_multi_placeholder_i18n_zh() {
        let i18n = crate::i18n::I18n::new(crate::i18n::Lang::Zh);
        let e = CalcError::domain("matrix dimension mismatch: 3x3 vs 2x2")
            .with_i18n(
                "msg.matrix_dim_mismatch",
                vec![
                    ("expected".to_string(), "3x3".to_string()),
                    ("actual".to_string(), "2x2".to_string()),
                ],
            );
        let friendly = e.friendly(&i18n);
        assert!(friendly.contains("定义域错误"));
        assert!(friendly.contains("矩阵维度不匹配: 期望 3x3, 实际 2x2"));
    }

    #[test]
    fn calc_error_to_json_ignores_i18n_key_uses_message() {
        let e = CalcError::eval("unbound variable: x")
            .with_i18n("msg.unbound_variable", vec![("name".to_string(), "x".to_string())]);
        let json = e.to_json();
        // JSON 输出始终用 message（机器可读契约，DP-4）
        assert!(json.contains(r#""message":"unbound variable: x""#));
        // JSON 不应包含 i18n 翻译后的文本
        assert!(!json.contains("Unbound variable"));
    }

    #[test]
    fn calc_error_display_ignores_i18n_key_uses_message() {
        let e = CalcError::eval("unbound variable: x")
            .with_i18n("msg.unbound_variable", vec![("name".to_string(), "x".to_string())]);
        let display = e.to_string();
        // Display 始终用 message（DP-1 机器可读契约）
        assert_eq!(display, "evaluation error: unbound variable: x");
    }

    #[test]
    fn calc_error_clone_preserves_i18n_fields() {
        let e = CalcError::eval("test")
            .with_i18n("msg.unknown_function", vec![("name".to_string(), "sin".to_string())]);
        let cloned = e.clone();
        assert_eq!(e.i18n_key, cloned.i18n_key);
        assert_eq!(e.i18n_args, cloned.i18n_args);
    }

    // ===== Error trait 测试 =====

    #[test]
    fn calc_error_error_trait_downcast() {
        let e = CalcError::parse("test error");
        let boxed: Box<dyn std::error::Error> = Box::new(e.clone());
        let downcasted = boxed.downcast_ref::<CalcError>();
        assert!(downcasted.is_some());
    }

    #[test]
    fn calc_error_clone_preserves_all_fields() {
        let e = CalcError::domain("asin(2)")
            .with_span(Span::new(0, 6))
            .with_hint("domain hint");
        let cloned = e.clone();
        assert_eq!(e, cloned);
        assert_eq!(e.span, cloned.span);
        assert_eq!(e.hint, cloned.hint);
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
        let AstNode::Complex(re, im) = node else {
            panic!("expected Complex variant")
        };
        assert!((re - 3.0).abs() < f64::EPSILON);
        assert!((im - 4.0).abs() < f64::EPSILON);
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
        let AstNode::Matrix(rows) = node else {
            panic!("expected Matrix variant")
        };
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].len(), 2);
        assert_eq!(rows[1].len(), 2);
        assert_eq!(rows[0][0], AstNode::Number(1.0));
        assert_eq!(rows[1][1], AstNode::Number(4.0));
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
            vec![
                AstNode::Number(1.0),
                AstNode::Number(2.0),
                AstNode::Number(3.0),
            ],
            vec![
                AstNode::Number(4.0),
                AstNode::Number(5.0),
                AstNode::Number(6.0),
            ],
        ]);
        let AstNode::Matrix(rows) = node else {
            panic!("expected Matrix variant")
        };
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].len(), 3);
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
        let AstNode::List(elements) = node else {
            panic!("expected List variant")
        };
        assert_eq!(elements.len(), 5);
        assert_eq!(elements[0], AstNode::Number(1.0));
        assert_eq!(elements[4], AstNode::Number(5.0));
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
        let AstNode::List(elements) = node else {
            panic!("expected List variant")
        };
        assert_eq!(elements.len(), 1);
    }

    #[test]
    fn ast_node_list_empty() {
        let node = AstNode::List(vec![]);
        let AstNode::List(elements) = node else {
            panic!("expected List variant")
        };
        assert!(elements.is_empty());
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
        // v0.8 新变体
        assert_eq!(EvalResult::Vector(vec![1.0, 2.0]).as_scalar(), None);
        assert_eq!(EvalResult::Polynomial(vec![1.0, 2.0]).as_scalar(), None);
        assert_eq!(EvalResult::ComplexList(vec![(1.0, 2.0)]).as_scalar(), None);
        assert_eq!(EvalResult::Symbolic("x-2".to_string()).as_scalar(), None);
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
        // v0.8 新变体
        assert_eq!(EvalResult::Vector(vec![1.0]).as_complex(), None);
        assert_eq!(EvalResult::Polynomial(vec![1.0]).as_complex(), None);
        assert_eq!(EvalResult::ComplexList(vec![(1.0, 2.0)]).as_complex(), None);
        assert_eq!(EvalResult::Symbolic("s".to_string()).as_complex(), None);
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
        // v0.8 新变体
        assert_eq!(EvalResult::Vector(vec![1.0]).as_matrix(), None);
        assert_eq!(EvalResult::Polynomial(vec![1.0]).as_matrix(), None);
        assert_eq!(EvalResult::ComplexList(vec![(1.0, 2.0)]).as_matrix(), None);
        assert_eq!(EvalResult::Symbolic("s".to_string()).as_matrix(), None);
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
            Some(&num_rational::BigRational::from_integer(
                num_bigint::BigInt::from(42)
            ))
        );
    }

    // ===== v0.8 新增变体测试：Vector / Polynomial / ComplexList / Symbolic =====

    #[test]
    fn eval_result_vector_construct_and_as_vector() {
        let r = EvalResult::Vector(vec![1.0, 2.0, 3.0]);
        let v = r.as_vector().expect("expected Some for Vector");
        assert_eq!(v, &vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn eval_result_vector_clone_and_eq() {
        let r = EvalResult::Vector(vec![1.0, 2.0]);
        assert_eq!(r, r.clone());
    }

    #[test]
    fn eval_result_vector_empty() {
        let r = EvalResult::Vector(vec![]);
        assert_eq!(r.as_vector(), Some(&Vec::<f64>::new()));
    }

    #[test]
    fn eval_result_as_vector_non_vector_variants() {
        assert_eq!(EvalResult::Scalar(1.0).as_vector(), None);
        assert_eq!(EvalResult::Complex(1.0, 2.0).as_vector(), None);
        assert_eq!(EvalResult::Matrix(vec![vec![1.0]]).as_vector(), None);
        assert_eq!(
            EvalResult::BigInt(num_bigint::BigInt::from(42)).as_vector(),
            None
        );
        assert_eq!(
            EvalResult::BigRational(num_rational::BigRational::new(
                num_bigint::BigInt::from(1),
                num_bigint::BigInt::from(3)
            ))
            .as_vector(),
            None
        );
        assert_eq!(EvalResult::Polynomial(vec![1.0]).as_vector(), None);
        assert_eq!(EvalResult::ComplexList(vec![(1.0, 2.0)]).as_vector(), None);
        assert_eq!(EvalResult::Symbolic("s".to_string()).as_vector(), None);
    }

    #[test]
    fn eval_result_polynomial_construct_and_as_polynomial() {
        // x^2 + 2x + 1 → [1, 2, 1]（升幂）
        let r = EvalResult::Polynomial(vec![1.0, 2.0, 1.0]);
        let p = r.as_polynomial().expect("expected Some for Polynomial");
        assert_eq!(p, &vec![1.0, 2.0, 1.0]);
    }

    #[test]
    fn eval_result_polynomial_clone_and_eq() {
        let r = EvalResult::Polynomial(vec![0.0, 1.0]);
        assert_eq!(r, r.clone());
    }

    #[test]
    fn eval_result_as_polynomial_non_polynomial_variants() {
        assert_eq!(EvalResult::Scalar(1.0).as_polynomial(), None);
        assert_eq!(EvalResult::Vector(vec![1.0]).as_polynomial(), None);
        assert_eq!(
            EvalResult::ComplexList(vec![(1.0, 2.0)]).as_polynomial(),
            None
        );
        assert_eq!(EvalResult::Symbolic("s".to_string()).as_polynomial(), None);
    }

    #[test]
    fn eval_result_complex_list_construct_and_as_complex_list() {
        let r = EvalResult::ComplexList(vec![(0.0, 1.0), (0.0, -1.0)]);
        let c = r.as_complex_list().expect("expected Some for ComplexList");
        assert_eq!(c, &vec![(0.0, 1.0), (0.0, -1.0)]);
    }

    #[test]
    fn eval_result_complex_list_clone_and_eq() {
        let r = EvalResult::ComplexList(vec![(1.0, 2.0)]);
        assert_eq!(r, r.clone());
    }

    #[test]
    fn eval_result_as_complex_list_non_complex_list_variants() {
        assert_eq!(EvalResult::Scalar(1.0).as_complex_list(), None);
        assert_eq!(EvalResult::Vector(vec![1.0]).as_complex_list(), None);
        assert_eq!(EvalResult::Polynomial(vec![1.0]).as_complex_list(), None);
        assert_eq!(
            EvalResult::Symbolic("s".to_string()).as_complex_list(),
            None
        );
    }

    #[test]
    fn eval_result_symbolic_construct_and_as_symbolic() {
        let r = EvalResult::Symbolic("(x-2)*(x+2)".to_string());
        let s = r.as_symbolic().expect("expected Some for Symbolic");
        assert_eq!(s, &"(x-2)*(x+2)".to_string());
    }

    #[test]
    fn eval_result_symbolic_clone_and_eq() {
        let r = EvalResult::Symbolic("x+1".to_string());
        assert_eq!(r, r.clone());
    }

    #[test]
    fn eval_result_as_symbolic_non_symbolic_variants() {
        assert_eq!(EvalResult::Scalar(1.0).as_symbolic(), None);
        assert_eq!(EvalResult::Vector(vec![1.0]).as_symbolic(), None);
        assert_eq!(EvalResult::Polynomial(vec![1.0]).as_symbolic(), None);
        assert_eq!(
            EvalResult::ComplexList(vec![(1.0, 2.0)]).as_symbolic(),
            None
        );
    }

    #[test]
    fn eval_result_new_variants_debug_format() {
        assert!(format!("{:?}", EvalResult::Vector(vec![1.0])).contains("Vector"));
        assert!(format!("{:?}", EvalResult::Polynomial(vec![1.0])).contains("Polynomial"));
        assert!(format!("{:?}", EvalResult::ComplexList(vec![(1.0, 2.0)])).contains("ComplexList"));
        assert!(format!("{:?}", EvalResult::Symbolic("s".to_string())).contains("Symbolic"));
    }

    // ===== v1.1 新增 LaTeX / Steps 变体测试（ADD §3.4） =====

    #[test]
    fn eval_result_latex_construct_and_match() {
        let r = EvalResult::LaTeX("\\frac{d}{dx}\\left(x^{2}\\right) = 2x".to_string());
        let EvalResult::LaTeX(s) = r else {
            panic!("expected LaTeX variant")
        };
        assert!(s.contains("\\frac{d}{dx}"));
    }

    #[test]
    fn eval_result_steps_construct_and_match() {
        let r = EvalResult::Steps(vec!["2+9=11".to_string(), "11*7=77".to_string()]);
        let EvalResult::Steps(v) = r else {
            panic!("expected Steps variant")
        };
        assert_eq!(v.len(), 2);
        assert_eq!(v[0], "2+9=11");
    }

    #[test]
    fn eval_result_latex_clone_and_eq() {
        let r = EvalResult::LaTeX("x^2".to_string());
        assert_eq!(r, r.clone());
    }

    #[test]
    fn eval_result_steps_clone_and_eq() {
        let r = EvalResult::Steps(vec!["1+1=2".to_string()]);
        assert_eq!(r, r.clone());
    }

    #[test]
    fn eval_result_latex_debug_format() {
        let r = EvalResult::LaTeX("x".to_string());
        assert!(format!("{:?}", r).contains("LaTeX"));
    }

    #[test]
    fn eval_result_steps_debug_format() {
        let r = EvalResult::Steps(vec!["1+1=2".to_string()]);
        assert!(format!("{:?}", r).contains("Steps"));
    }

    #[test]
    fn eval_result_as_latex_returns_some_for_latex() {
        let r = EvalResult::LaTeX("x^2".to_string());
        assert_eq!(r.as_latex(), Some(&"x^2".to_string()));
    }

    #[test]
    fn eval_result_as_latex_returns_none_for_others() {
        assert_eq!(EvalResult::Scalar(1.0).as_latex(), None);
        assert_eq!(EvalResult::Symbolic("x".to_string()).as_latex(), None);
        assert_eq!(
            EvalResult::Steps(vec!["1+1=2".to_string()]).as_latex(),
            None
        );
    }

    #[test]
    fn eval_result_as_steps_returns_some_for_steps() {
        let r = EvalResult::Steps(vec!["2+2=4".to_string(), "4*3=12".to_string()]);
        let s = r.as_steps().unwrap();
        assert_eq!(s.len(), 2);
        assert_eq!(s[1], "4*3=12");
    }

    #[test]
    fn eval_result_as_steps_returns_none_for_others() {
        assert_eq!(EvalResult::Scalar(1.0).as_steps(), None);
        assert_eq!(EvalResult::LaTeX("x".to_string()).as_steps(), None);
        assert_eq!(EvalResult::Symbolic("x".to_string()).as_steps(), None);
    }

    #[test]
    fn eval_result_latex_serde_roundtrip() {
        let r = EvalResult::LaTeX("\\frac{1}{2}".to_string());
        let json = serde_json::to_string(&r).expect("serialize");
        let r2: EvalResult = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(r, r2);
    }

    #[test]
    fn eval_result_steps_serde_roundtrip() {
        let r = EvalResult::Steps(vec!["1+1=2".to_string(), "2+2=4".to_string()]);
        let json = serde_json::to_string(&r).expect("serialize");
        let r2: EvalResult = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(r, r2);
    }

    #[test]
    fn eval_result_new_variants_return_none_from_old_accessors() {
        // 确保新变体在旧访问器中均返回 None
        let latex = EvalResult::LaTeX("x".to_string());
        let steps = EvalResult::Steps(vec!["1+1=2".to_string()]);
        assert_eq!(latex.as_scalar(), None);
        assert_eq!(latex.as_complex(), None);
        assert_eq!(latex.as_matrix(), None);
        assert_eq!(latex.as_bigint(), None);
        assert_eq!(latex.as_bigrational(), None);
        assert_eq!(latex.as_vector(), None);
        assert_eq!(latex.as_polynomial(), None);
        assert_eq!(latex.as_complex_list(), None);
        assert_eq!(latex.as_symbolic(), None);
        assert_eq!(steps.as_scalar(), None);
        assert_eq!(steps.as_complex(), None);
        assert_eq!(steps.as_matrix(), None);
        assert_eq!(steps.as_bigint(), None);
        assert_eq!(steps.as_bigrational(), None);
        assert_eq!(steps.as_vector(), None);
        assert_eq!(steps.as_polynomial(), None);
        assert_eq!(steps.as_complex_list(), None);
        assert_eq!(steps.as_symbolic(), None);
    }

    // ===== p4 新增 Json 变体测试（numerical-linalg） =====

    #[test]
    fn eval_result_json_construct_and_match() {
        let v = serde_json::json!({"values": [1.0, 2.0], "vectors": [[1.0, 0.0], [0.0, 1.0]]});
        let r = EvalResult::Json(v.clone());
        let EvalResult::Json(inner) = r else {
            panic!("expected Json variant")
        };
        assert_eq!(inner, v);
    }

    #[test]
    fn eval_result_json_clone_and_eq() {
        let r = EvalResult::Json(serde_json::json!({"a": 1}));
        assert_eq!(r, r.clone());
    }

    #[test]
    fn eval_result_json_serde_roundtrip() {
        let r = EvalResult::Json(serde_json::json!({"U": [[1.0]], "S": [1.0], "Vt": [[1.0]]}));
        let s = serde_json::to_string(&r).expect("serialize");
        let r2: EvalResult = serde_json::from_str(&s).expect("deserialize");
        assert_eq!(r, r2);
    }

    #[test]
    fn eval_result_json_returns_none_from_typed_accessors() {
        // Json 是复合结果，所有 typed 访问器（as_scalar 等）返回 None
        let r = EvalResult::Json(serde_json::json!({"a": 1}));
        assert_eq!(r.as_scalar(), None);
        assert_eq!(r.as_complex(), None);
        assert_eq!(r.as_matrix(), None);
        assert_eq!(r.as_bigint(), None);
        assert_eq!(r.as_bigrational(), None);
        assert_eq!(r.as_vector(), None);
        assert_eq!(r.as_polynomial(), None);
        assert_eq!(r.as_complex_list(), None);
        assert_eq!(r.as_symbolic(), None);
        assert_eq!(r.as_latex(), None);
        assert_eq!(r.as_steps(), None);
    }
}
