// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! 表达式解析器：将数学表达式字符串解析为 [`AstNode`]。
//! 基于 mathexpr crate，附加预处理/转换层与深度、长度限制（DoS 防护）。

use crate::core::types::{AstNode, BinaryOp, CalcError, Span, UnaryOp};
use regex::Regex;
use std::cell::Cell;

/// 最大 AST 深度（≤ 256，防递归栈溢出）。
pub(crate) const MAX_AST_DEPTH: usize = 256;

/// 最大表达式长度（≤ 4096 字符，超限快速失败）。
pub(crate) const MAX_EXPR_LEN: usize = 4096;

thread_local! {
    /// 括号字面量递归深度计数（重入路径无法经参数穿透，须跨调用保持计数）。
    static LITERAL_PARSE_DEPTH: Cell<usize> = const { Cell::new(0) };
}

/// 括号字面量解析深度 RAII 守卫：进入时深度 +1（超限报错），Drop 时 -1。
struct LiteralDepthGuard;

impl LiteralDepthGuard {
    fn enter() -> Result<Self, CalcError> {
        LITERAL_PARSE_DEPTH.with(|d| {
            let n = d.get() + 1;
            if n > MAX_AST_DEPTH {
                return Err(CalcError::depth_exceeded());
            }
            d.set(n);
            Ok(Self)
        })
    }
}

impl Drop for LiteralDepthGuard {
    fn drop(&mut self) {
        LITERAL_PARSE_DEPTH.with(|d| d.set(d.get().saturating_sub(1)));
    }
}

/// 解析数学表达式字符串为 [`AstNode`]（预处理后交 mathexpr，再转换为 AstNode）。
pub fn parse(input: &str) -> Result<AstNode, CalcError> {
    // 字节长度 O(1) 快速失败；字符计数 O(n) 仅在错误分支计算
    if input.len() > MAX_EXPR_LEN {
        let char_count = input.chars().count();
        return Err(CalcError::parse(format!(
            "expression length {} exceeds maximum of {} characters",
            char_count, MAX_EXPR_LEN
        ))
        .with_span(Span::new(0, char_count))
        .with_i18n(
            "msg.core.parse_length_exceeds",
            vec![
                ("len".to_string(), char_count.to_string()),
                ("max".to_string(), MAX_EXPR_LEN.to_string()),
            ],
        ));
    }

    let trimmed = input.trim();

    if trimmed.is_empty() {
        return Err(CalcError::parse("expression is empty".to_string())
            .with_span(Span::new(0, 0))
            .with_i18n("msg.core.parse_empty", vec![]));
    }

    // 各预处理段先做 O(n) 特征扫描，无触发字符时跳过整段
    let has_quote = trimmed.contains('"');
    let has_bracket = trimmed.contains('[');
    let has_factorial = trimmed.contains('!');
    let has_imag = trimmed.contains('i') || trimmed.contains('I');

    // 字符串字面量替换为占位符 `__str_N`；必须在 brackets/bigint 之前执行，防误提取
    let (without_strings, placeholders) = if has_quote {
        preprocess_strings(trimmed)?
    } else {
        (trimmed.to_string(), Default::default())
    };

    // 括号字面量替换为占位符 `__cb_N`，使矩阵/列表可出现在表达式任意位置
    let (without_brackets, mut placeholders) = if has_bracket {
        preprocess_brackets(&without_strings, placeholders)?
    } else {
        (without_strings, placeholders)
    };

    // 若整个表达式就是单个括号字面量，直接返回（避免 mathexpr 处理）
    if placeholders.len() == 1 {
        let name = placeholders.keys().next().unwrap().clone();
        if without_brackets == name {
            return Ok(placeholders.remove(&name).unwrap());
        }
    }

    // 预处理大整数字面量：将 16+ 位整数替换为占位符 `__bn_N`（避免 f64 精度丢失）
    let without_bigint = if has_long_digit_run(&without_brackets) {
        preprocess_bigint(&without_brackets, &mut placeholders)?
    } else {
        without_brackets
    };

    // 显式拒绝 `++`、`**`、`//`、`^^`（mathexpr 会静默接受 `++` 或给出通用错误）
    validate_no_consecutive_operators(&without_bigint)?;

    // 复数预处理：`3+4i` → `complex(3, 4)`、`2i` → `complex(0, 2)`（无 `i` 跳过 → 免 2 次 regex）
    let after_complex = if has_imag {
        preprocess_complex(&without_bigint)?
    } else {
        without_bigint
    };

    // 阶乘预处理（无 `!` 跳过）
    let after_factorial = if has_factorial {
        preprocess_factorial(&after_complex)?
    } else {
        after_complex
    };

    // 隐式乘法预处理：`2x` → `2*x`、`3(x+1)` → `3*(x+1)`、`(x+1)(x-1)` → `(x+1)*(x-1)`
    let after_implicit = insert_implicit_multiplication(&after_factorial);

    // mathexpr 内部递归先于我们的深度检查，深嵌套会栈溢出；迭代 O(n) 预检先行拒绝
    {
        let mut bracket_depth: usize = 0;
        for ch in after_implicit.chars() {
            match ch {
                '(' | '[' | '{' => {
                    bracket_depth += 1;
                    if bracket_depth > MAX_AST_DEPTH {
                        return Err(CalcError::depth_exceeded());
                    }
                }
                ')' | ']' | '}' => {
                    bracket_depth = bracket_depth.saturating_sub(1);
                }
                _ => {}
            }
        }
    }

    let expr = mathexpr::parse(&after_implicit).map_err(|e| {
        // Span 基于原始输入（after_implicit 经预处理长度会变）；文案经清洗不泄漏内部 Debug 结构
        let friendly = friendly_mathexpr_error(&e.to_string());
        CalcError::parse(friendly.clone())
            .with_span(Span::new(0, trimmed.chars().count()))
            .with_i18n(
                "msg.core.parse_mathexpr_error",
                // i18n 参数同样传清洗后的文案
                vec![("error".to_string(), friendly)],
            )
    })?;

    // 转换为 CalNexus AstNode（含深度检查，防止递归栈溢出）
    let mut ast = convert_with_depth(&expr, 1)?;

    replace_placeholders(&mut ast, &placeholders);

    Ok(ast)
}

/// 复数预处理：`a±bi`/纯 `bi` → `complex(a, b)`（先整体后纯虚数，避免误替换）。
fn preprocess_complex(input: &str) -> Result<String, CalcError> {
    // OnceLock 缓存编译后的正则，避免重复编译
    use std::sync::OnceLock;
    static RE_COMPLEX_FULL: OnceLock<Regex> = OnceLock::new();
    static RE_PURE_IMAGINARY: OnceLock<Regex> = OnceLock::new();

    let re_full = RE_COMPLEX_FULL
        .get_or_init(|| Regex::new(r"(\d+(?:\.\d+)?)\s*([+-])\s*(\d+(?:\.\d+)?)\s*i").unwrap());
    let re_pure = RE_PURE_IMAGINARY.get_or_init(|| Regex::new(r"(\d+(?:\.\d+)?)\s*i").unwrap());

    let mut result = input.to_string();

    // 先替换 `a+bi` / `a-bi`（整体匹配，避免 `4i` 被先替换）
    result = re_full
        .replace_all(&result, |caps: &regex::Captures| {
            let re = caps.get(1).unwrap().as_str();
            let sign = caps.get(2).unwrap().as_str();
            let im = caps.get(3).unwrap().as_str();
            format!("complex({re},{sign}{im})")
        })
        .to_string();

    // 再替换纯虚数 `bi` → `complex(0, b)`
    result = re_pure
        .replace_all(&result, |caps: &regex::Captures| {
            let im = caps.get(1).unwrap().as_str();
            format!("complex(0,{})", im)
        })
        .to_string();

    Ok(result)
}

/// O(n) 扫描是否存在 16+ 位连续数字（bigint 预处理的触发特征）。
fn has_long_digit_run(s: &str) -> bool {
    let mut run = 0usize;
    for c in s.chars() {
        if c.is_ascii_digit() {
            run += 1;
            if run >= 16 {
                return true;
            }
        } else {
            run = 0;
        }
    }
    false
}

/// 将 mathexpr 错误的 Debug 文案清洗为用户友好形式（不泄漏内部 Debug 结构）。
fn friendly_mathexpr_error(display: &str) -> String {
    if let Some(idx) = display.find("input: \"") {
        let rest = &display[idx + "input: \"".len()..];
        let end = rest
            .find("\", code")
            .or_else(|| rest.rfind('"'))
            .unwrap_or(rest.len());
        let raw = &rest[..end];
        if raw.is_empty() {
            return "unexpected end of expression".to_string();
        }
        // 片段截断到 24 字符，避免超长输入刷屏
        let mut frag: String = raw.chars().take(24).collect();
        if raw.chars().count() > 24 {
            frag.push('…');
        }
        return format!("unexpected input near '{}'", frag);
    }
    // mathexpr 自身友好的文案原样保留
    display.to_string()
}

/// 解析以 `[` 开头的字面量：矩阵 `[[...]]` 或列表 `[...]`。
fn parse_bracket_literal(input: &str) -> Result<AstNode, CalcError> {
    // 深度守卫：所有括号字面量递归的单一深度检查点
    let _depth = LiteralDepthGuard::enter()?;
    let trimmed = input.trim();
    if trimmed.starts_with("[[") {
        parse_matrix_literal(trimmed)
    } else if trimmed.starts_with('[') {
        parse_list_literal(trimmed)
    } else {
        Err(
            CalcError::parse(format!("expected bracket literal, got: {}", trimmed))
                .with_span(Span::new(0, trimmed.chars().count()))
                .with_i18n(
                    "msg.core.parse_expected_bracket",
                    vec![("token".to_string(), trimmed.to_string())],
                ),
        )
    }
}

/// 将 `"..."` 替换为占位符 `__str_N`；必须在 brackets/bigint 之前执行，防止误提取。
fn preprocess_strings(
    input: &str,
) -> Result<(String, std::collections::HashMap<String, AstNode>), CalcError> {
    let mut result = String::with_capacity(input.len());
    let mut placeholders: std::collections::HashMap<String, AstNode> =
        std::collections::HashMap::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    let mut count = 0;

    while i < chars.len() {
        if chars[i] == '"' {
            // 进入字符串字面量：扫描到匹配的 `"`，处理 `\"` 转义
            let start = i; // 起始 `"` 的字符偏移
            i += 1; // 跳过起始 `"`
            let mut content = String::new();
            let mut closed = false;
            while i < chars.len() {
                if chars[i] == '\\' {
                    // 转义序列：`\"` → `"`，`\\` → `\`
                    if i + 1 < chars.len() && chars[i + 1] == '"' {
                        content.push('"');
                        i += 2;
                    } else if i + 1 < chars.len() && chars[i + 1] == '\\' {
                        content.push('\\');
                        i += 2;
                    } else {
                        // 单独的反斜杠：保留原样（不视为转义）
                        content.push('\\');
                        i += 1;
                    }
                } else if chars[i] == '"' {
                    i += 1;
                    closed = true;
                    break;
                } else {
                    content.push(chars[i]);
                    i += 1;
                }
            }
            if !closed {
                // 字符偏移用 start..i（i 已到字符串末尾）
                return Err(CalcError::parse("unclosed string literal".to_string())
                    .with_span(Span::new(start, i))
                    .with_i18n("msg.core.parse_unclosed_string", vec![]));
            }
            let placeholder = format!("__str_{}", count);
            count += 1;
            placeholders.insert(placeholder.clone(), AstNode::Str(content));
            result.push_str(&placeholder);
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }

    Ok((result, placeholders))
}

/// 将 `[...]` 字面量替换为占位符 `__cb_N`，可出现在表达式任意位置（追加到既有占位符表）。
fn preprocess_brackets(
    input: &str,
    mut placeholders: std::collections::HashMap<String, AstNode>,
) -> Result<(String, std::collections::HashMap<String, AstNode>), CalcError> {
    let mut result = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    // 各占位符前缀独立编号（__cb_N、__str_N、__bn_N 互不冲突）
    let mut count = 0;

    while i < chars.len() {
        if chars[i] == '[' {
            let start = i;
            let mut depth = 1;
            i += 1;
            while i < chars.len() && depth > 0 {
                if chars[i] == '[' {
                    depth += 1;
                } else if chars[i] == ']' {
                    depth -= 1;
                }
                i += 1;
            }
            if depth != 0 {
                return Err(CalcError::parse("unmatched '[' in expression".to_string())
                    .with_span(Span::new(start, i))
                    .with_i18n("msg.core.parse_unmatched_bracket", vec![]));
            }
            let literal: String = chars[start..i].iter().collect();
            let node = parse_bracket_literal(&literal)?;
            let placeholder = format!("__cb_{}", count);
            count += 1;
            placeholders.insert(placeholder.clone(), node);
            result.push_str(&placeholder);
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }

    Ok((result, placeholders))
}

/// 16+ 位整数替换为占位符 `__bn_N` 并存为 `BigNumber`，避免 mathexpr f64 精度丢失。
fn preprocess_bigint(
    input: &str,
    placeholders: &mut std::collections::HashMap<String, AstNode>,
) -> Result<String, CalcError> {
    let mut result = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    let mut count = 0;

    while i < chars.len() {
        if chars[i].is_ascii_digit() {
            let start = i;
            while i < chars.len() && chars[i].is_ascii_digit() {
                i += 1;
            }
            let digit_count = i - start;
            // 检查是否为小数的一部分（前一个或后一个字符为 `.`）
            let prev_char = if start > 0 {
                Some(chars[start - 1])
            } else {
                None
            };
            let next_char = if i < chars.len() {
                Some(chars[i])
            } else {
                None
            };
            let is_decimal = prev_char == Some('.') || next_char == Some('.');

            if digit_count >= 16 && !is_decimal {
                let digits: String = chars[start..i].iter().collect();
                let placeholder = format!("__bn_{}", count);
                count += 1;
                placeholders.insert(placeholder.clone(), AstNode::BigNumber(digits));
                result.push_str(&placeholder);
            } else {
                for c in &chars[start..i] {
                    result.push(*c);
                }
            }
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }

    Ok(result)
}

/// 递归替换 AST 中的占位符变量为实际的 Matrix/List/BigNumber 节点。
fn replace_placeholders(
    ast: &mut AstNode,
    placeholders: &std::collections::HashMap<String, AstNode>,
) {
    match ast {
        AstNode::Variable(name) => {
            if let Some(node) = placeholders.get(name) {
                *ast = node.clone();
            }
        }
        AstNode::BinaryOp(_, l, r) => {
            replace_placeholders(l, placeholders);
            replace_placeholders(r, placeholders);
        }
        AstNode::UnaryOp(_, e) => {
            replace_placeholders(e, placeholders);
        }
        AstNode::FunctionCall(_, args) => {
            for arg in args {
                replace_placeholders(arg, placeholders);
            }
        }
        AstNode::Matrix(rows) => {
            for row in rows {
                for elem in row {
                    replace_placeholders(elem, placeholders);
                }
            }
        }
        AstNode::List(elements) => {
            for elem in elements {
                replace_placeholders(elem, placeholders);
            }
        }
        AstNode::Number(_) | AstNode::Complex(_, _) | AstNode::BigNumber(_) | AstNode::Str(_) => {}
    }
}

/// 解析矩阵字面量 `[[row1],[row2],...]`（元素递归调用 [`parse`]）。
fn parse_matrix_literal(input: &str) -> Result<AstNode, CalcError> {
    let trimmed = input.trim();
    if !trimmed.starts_with("[[") || !trimmed.ends_with("]]") {
        return Err(
            CalcError::parse(format!("invalid matrix literal: {}", trimmed))
                .with_span(Span::new(0, trimmed.chars().count()))
                .with_i18n(
                    "msg.core.parse_invalid_matrix_literal",
                    vec![("detail".to_string(), trimmed.to_string())],
                ),
        );
    }
    // 去掉外层 `[[` 和 `]]`，得到 `row1],[row2],[...`
    let inner = &trimmed[2..trimmed.len() - 2];
    let rows_str = split_by_pattern(inner, "],[");
    let mut rows: Vec<Vec<AstNode>> = Vec::with_capacity(rows_str.len());
    for row_str in &rows_str {
        // 每行用 `[` 和 `]` 包裹，再用 parse_list_literal 解析
        let row_full = format!("[{}]", row_str.trim());
        let row_node = parse_list_literal(&row_full)?;
        match row_node {
            AstNode::List(elements) => rows.push(elements),
            _ => {
                // 限制 Debug 输出长度防止大型 AST 递归 Debug 导致性能问题
                let node_debug: String = format!("{:?}", row_node).chars().take(100).collect();
                return Err(CalcError::parse(format!(
                    "expected list row in matrix, got: {}",
                    node_debug
                ))
                .with_span(Span::new(0, trimmed.chars().count()))
                .with_i18n(
                    "msg.core.parse_expected_list_row",
                    vec![("node".to_string(), node_debug)],
                ));
            }
        }
    }
    if rows.is_empty() {
        return Err(CalcError::parse("empty matrix literal".to_string())
            .with_span(Span::new(0, trimmed.chars().count()))
            .with_i18n("msg.core.parse_empty_matrix_literal", vec![]));
    }
    Ok(AstNode::Matrix(rows))
}

/// 解析列表字面量 `[elem1,elem2,...]`（元素递归调用 [`parse`]）。
fn parse_list_literal(input: &str) -> Result<AstNode, CalcError> {
    let trimmed = input.trim();
    if !trimmed.starts_with('[') || !trimmed.ends_with(']') {
        return Err(
            CalcError::parse(format!("invalid list literal: {}", trimmed))
                .with_span(Span::new(0, trimmed.chars().count()))
                .with_i18n(
                    "msg.core.parse_invalid_list_literal",
                    vec![("detail".to_string(), trimmed.to_string())],
                ),
        );
    }
    let inner = &trimmed[1..trimmed.len() - 1];
    let inner_trimmed = inner.trim();
    if inner_trimmed.is_empty() {
        return Ok(AstNode::List(vec![]));
    }
    // 用顶层 `,` 分割元素（跳过嵌套 `()` 和 `[]`）
    let parts = split_top_level_commas(inner_trimmed);
    let mut elements: Vec<AstNode> = Vec::with_capacity(parts.len());
    for part in &parts {
        elements.push(parse(part)?);
    }
    Ok(AstNode::List(elements))
}

/// 用指定分隔符模式分割字符串，跳过嵌套的 `()` 和 `[]`。
fn split_by_pattern(input: &str, sep: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = input.chars().collect();
    let sep_chars: Vec<char> = sep.chars().collect();
    let mut i = 0;
    let mut bracket_depth = 0i32;
    let mut paren_depth = 0i32;

    while i < chars.len() {
        // 先检查分隔符（使用"之前"的深度，即当前字符尚未影响深度）
        if bracket_depth == 0 && paren_depth == 0 && i + sep_chars.len() <= chars.len() {
            let candidate: String = chars[i..i + sep_chars.len()].iter().collect();
            if candidate == sep {
                result.push(current.clone());
                current.clear();
                i += sep_chars.len();
                continue;
            }
        }
        // 更新深度（当前字符影响后续深度判断）
        let c = chars[i];
        if c == '[' {
            bracket_depth += 1;
        } else if c == ']' {
            bracket_depth -= 1;
        } else if c == '(' {
            paren_depth += 1;
        } else if c == ')' {
            paren_depth -= 1;
        }
        current.push(c);
        i += 1;
    }
    result.push(current);
    result
}

/// 用顶层 `,` 分割字符串，跳过嵌套的 `()` 和 `[]`。
fn split_top_level_commas(input: &str) -> Vec<String> {
    split_by_pattern(input, ",")
}

/// 拒绝非法连续运算符 `++`、`**`、`//`、`^^`（mathexpr 会静默接受 `++`；`--` 合法不检查）。
fn validate_no_consecutive_operators(input: &str) -> Result<(), CalcError> {
    // 非法连续运算符列表（不含 `--`，因为双重负号合法）
    const ILLEGAL_CONSECUTIVE_OPS: &[&str] = &["++", "**", "//", "^^"];

    // 预先 collect，避免每字符 × 4 运算符的重复分配
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        // 仅当首字符可能是非法运算符首字符时才查第二字符（快路径跳过字母数字）
        if c == '+' || c == '*' || c == '/' || c == '^' {
            for &op in ILLEGAL_CONSECUTIVE_OPS {
                let op_first = op.as_bytes()[0] as char;
                if c != op_first {
                    continue;
                }
                let mut j = i + 1;
                while j < chars.len() && chars[j].is_whitespace() {
                    j += 1;
                }
                // 检查是否匹配：第一个字符 + (可选空格) + 第二个字符
                if j < chars.len() && chars[j] == op.as_bytes()[1] as char {
                    return Err(CalcError::parse(format!(
                        "illegal consecutive operators '{}'",
                        op
                    ))
                    .with_span(Span::new(i, j + 1))
                    .with_i18n(
                        "msg.core.parse_illegal_consecutive_ops",
                        vec![("op".to_string(), op.to_string())],
                    ));
                }
            }
        }
        i += 1;
    }
    Ok(())
}

/// 阶乘预处理：`expr!` → `factorial(expr)`（数字/括号/变量/函数均可作操作数）。
fn preprocess_factorial(input: &str) -> Result<String, CalcError> {
    let chars: Vec<char> = input.chars().collect();
    let mut result: Vec<char> = Vec::with_capacity(chars.len() + 32);
    let mut i = 0;

    while i < chars.len() {
        if chars[i] == '!' {
            let operand_start =
                find_operand_start(&result).map_err(|e| e.with_span(Span::point(i)))?;
            let operand: String = result[operand_start..].iter().collect();
            result.truncate(operand_start);
            result.extend("factorial(".chars());
            result.extend(operand.chars());
            result.push(')');
        } else {
            result.push(chars[i]);
        }
        i += 1;
    }

    Ok(result.into_iter().collect())
}

/// 从缓冲区末端向前找阶乘操作数起点（末尾为 `)` 时继续向左含函数名）。
fn find_operand_start(chars: &[char]) -> Result<usize, CalcError> {
    let mut pos = chars.len();

    while pos > 0 && chars[pos - 1].is_whitespace() {
        pos -= 1;
    }

    if pos == 0 {
        return Err(
            CalcError::parse("factorial operator '!' has no operand".to_string())
                .with_i18n("msg.core.parse_factorial_no_operand", vec![]),
        );
    }

    if chars[pos - 1] == ')' {
        pos = match_paren_backward(chars, pos - 1)?;
        // '(' 前可能还有函数名，继续向左扫描
        pos = scan_identifier_backward(chars, pos);
    } else {
        while pos > 0 {
            let c = chars[pos - 1];
            if c.is_alphanumeric() || c == '.' || c == '_' {
                pos -= 1;
            } else {
                break;
            }
        }
    }

    Ok(pos)
}

/// 向左匹配括号返回对应 `(` 索引，失败返回 `unmatched parenthesis` 错误。
fn match_paren_backward(chars: &[char], close_idx: usize) -> Result<usize, CalcError> {
    let mut depth = 1;
    let mut pos = close_idx;
    while pos > 0 && depth > 0 {
        pos -= 1;
        match chars[pos] {
            ')' => depth += 1,
            '(' => depth -= 1,
            _ => {}
        }
    }
    if depth != 0 {
        return Err(
            CalcError::parse("unmatched parenthesis in factorial operand".to_string())
                .with_i18n("msg.core.parse_unmatched_paren_factorial", vec![]),
        );
    }
    Ok(pos)
}

/// 从 `pos` 向左扫描连续的字母字符（函数名标识符），返回新的位置。
fn scan_identifier_backward(chars: &[char], mut pos: usize) -> usize {
    while pos > 0 && chars[pos - 1].is_alphabetic() {
        pos -= 1;
    }
    pos
}

/// 隐式乘法预处理：相邻 token 间插入 `*`（标识符内数字不触发，如 `beta_1`）。
fn insert_implicit_multiplication(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut result = String::with_capacity(chars.len() + 16);

    let mut prev_was_number_end = false; // 前一个字符是数字且不属于标识符
    let mut prev_was_close_paren = false;
    let mut in_identifier = false; // 当前正在扫描标识符（以字母/下划线开头）

    for (i, &c) in chars.iter().enumerate() {
        if i > 0 {
            // 科学计数法排除：数字后跟 e/E 且后接数字或 +/-数字 → 不插入 *
            if prev_was_number_end && (c == 'e' || c == 'E') && is_scientific_notation(&chars, i) {
                // 不插入 *，但也不重置状态（e 后面跟数字时由标识符逻辑处理）
            } else if should_insert_implicit_mult(prev_was_number_end, prev_was_close_paren, c) {
                result.push('*');
                in_identifier = false;
            }
        }

        result.push(c);

        prev_was_close_paren = c == ')';

        if c.is_alphabetic() || c == '_' {
            in_identifier = true;
            prev_was_number_end = false;
        } else if c.is_ascii_digit() {
            // 在标识符内的数字不视为数字结尾
            prev_was_number_end = !in_identifier;
        } else {
            in_identifier = false;
            prev_was_number_end = false;
        }
    }

    result
}

/// 判断 `chars[pos]` 处的 `e`/`E` 是否为科学计数法一部分（后接数字或 +/-数字）。
fn is_scientific_notation(chars: &[char], pos: usize) -> bool {
    if let Some(&next) = chars.get(pos + 1) {
        if next.is_ascii_digit() {
            return true;
        }
        if (next == '+' || next == '-')
            && let Some(&next2) = chars.get(pos + 2)
        {
            return next2.is_ascii_digit();
        }
    }
    false
}

/// 判断是否应在当前字符前插入隐式乘法 `*`。
fn should_insert_implicit_mult(
    prev_was_number_end: bool,
    prev_was_close_paren: bool,
    curr: char,
) -> bool {
    let curr_is_var_start = curr.is_alphabetic() || curr == '_';
    let curr_is_digit = curr.is_ascii_digit();
    let curr_is_open_paren = curr == '(';

    // 数字 → 变量/`(`（排除标识符内数字，由 prev_was_number_end 保证）
    if prev_was_number_end && (curr_is_var_start || curr_is_open_paren) {
        return true;
    }
    // `)` → 变量/数字/`(`
    if prev_was_close_paren && (curr_is_var_start || curr_is_digit || curr_is_open_paren) {
        return true;
    }
    false
}

/// 将 mathexpr 的 `Expr` 转换为 [`AstNode`]，超深立即报错防递归栈溢出。
fn convert_with_depth(expr: &mathexpr::Expr, depth: usize) -> Result<AstNode, CalcError> {
    if depth > MAX_AST_DEPTH {
        return Err(CalcError::depth_exceeded());
    }

    use mathexpr::{BinOp as MBinOp, Expr};

    match expr {
        Expr::Number(n) => Ok(AstNode::Number(*n)),
        Expr::Variable(name) => Ok(AstNode::Variable(name.clone())),
        Expr::CurrentValue => Ok(AstNode::Variable("_".to_string())),
        Expr::BinaryOp { op, left, right } => {
            let l = convert_with_depth(left, depth + 1)?;
            let r = convert_with_depth(right, depth + 1)?;
            match op {
                MBinOp::Mod => Ok(AstNode::FunctionCall("mod".to_string(), vec![l, r])),
                MBinOp::Add => Ok(AstNode::BinaryOp(BinaryOp::Add, Box::new(l), Box::new(r))),
                MBinOp::Sub => Ok(AstNode::BinaryOp(BinaryOp::Sub, Box::new(l), Box::new(r))),
                MBinOp::Mul => Ok(AstNode::BinaryOp(BinaryOp::Mul, Box::new(l), Box::new(r))),
                MBinOp::Div => Ok(AstNode::BinaryOp(BinaryOp::Div, Box::new(l), Box::new(r))),
                MBinOp::Pow => Ok(AstNode::BinaryOp(BinaryOp::Pow, Box::new(l), Box::new(r))),
            }
        }
        Expr::UnaryMinus(inner) => {
            let e = convert_with_depth(inner, depth + 1)?;
            Ok(AstNode::UnaryOp(UnaryOp::Neg, Box::new(e)))
        }
        Expr::FunctionCall { name, args } => {
            // 0-arity 的 pi/e 转换为 Variable
            if args.is_empty() && (name == "pi" || name == "e") {
                return Ok(AstNode::Variable(name.clone()));
            }
            let mut converted_args = Vec::with_capacity(args.len());
            for arg in args {
                converted_args.push(convert_with_depth(arg, depth + 1)?);
            }
            // 复数字面量：`complex(re, im)` → `Complex`（`-4` 会被解析为 Neg(Number)，需规范化）
            if name == "complex"
                && converted_args.len() == 2
                && let Some(complex) = try_complex_literal(&converted_args)
            {
                return Ok(complex);
            }
            Ok(AstNode::FunctionCall(name.clone(), converted_args))
        }
    }
}

/// 将 `complex(re, im)` 参数规范化为 `AstNode::Complex`（im 可为 Neg(Number)，否则回退 FunctionCall）。
fn try_complex_literal(args: &[AstNode]) -> Option<AstNode> {
    let re_val = match &args[0] {
        AstNode::Number(n) => Some(*n),
        _ => None,
    };
    let im_val = match &args[1] {
        AstNode::Number(n) => Some(*n),
        AstNode::UnaryOp(UnaryOp::Neg, inner) => {
            if let AstNode::Number(n) = inner.as_ref() {
                Some(-*n)
            } else {
                None
            }
        }
        _ => None,
    };
    match (re_val, im_val) {
        (Some(re), Some(im)) => Some(AstNode::Complex(re, im)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::*;

    // 辅助函数
    fn binop(op: BinaryOp, l: AstNode, r: AstNode) -> AstNode {
        AstNode::BinaryOp(op, Box::new(l), Box::new(r))
    }
    fn unary(op: UnaryOp, e: AstNode) -> AstNode {
        AstNode::UnaryOp(op, Box::new(e))
    }
    fn call(name: &str, args: Vec<AstNode>) -> AstNode {
        AstNode::FunctionCall(name.to_string(), args)
    }
    fn num(n: f64) -> AstNode {
        AstNode::Number(n)
    }
    fn var(name: &str) -> AstNode {
        AstNode::Variable(name.to_string())
    }

    // ===== 基本四则运算解析 =====

    #[test]
    fn test_simple_addition() {
        let ast = parse("2+3").unwrap();
        assert_eq!(ast, binop(BinaryOp::Add, num(2.0), num(3.0)));
    }

    #[test]
    fn test_mixed_arithmetic_precedence() {
        let ast = parse("(2+9)*7-6").unwrap();
        assert_eq!(
            ast,
            binop(
                BinaryOp::Sub,
                binop(
                    BinaryOp::Mul,
                    binop(BinaryOp::Add, num(2.0), num(9.0)),
                    num(7.0)
                ),
                num(6.0)
            )
        );
    }

    #[test]
    fn test_left_associative_subtraction() {
        let ast = parse("10-3-2").unwrap();
        assert_eq!(
            ast,
            binop(
                BinaryOp::Sub,
                binop(BinaryOp::Sub, num(10.0), num(3.0)),
                num(2.0)
            )
        );
    }

    // ===== 幂运算解析 =====

    #[test]
    fn test_simple_power() {
        let ast = parse("2^10").unwrap();
        assert_eq!(ast, binop(BinaryOp::Pow, num(2.0), num(10.0)));
    }

    #[test]
    fn test_power_right_associative() {
        let ast = parse("2^3^2").unwrap();
        assert_eq!(
            ast,
            binop(
                BinaryOp::Pow,
                num(2.0),
                binop(BinaryOp::Pow, num(3.0), num(2.0))
            )
        );
    }

    #[test]
    fn test_power_higher_precedence_than_mul() {
        let ast = parse("2*3^2").unwrap();
        assert_eq!(
            ast,
            binop(
                BinaryOp::Mul,
                num(2.0),
                binop(BinaryOp::Pow, num(3.0), num(2.0))
            )
        );
    }

    // ===== 阶乘运算解析 =====

    #[test]
    fn test_integer_factorial() {
        let ast = parse("5!").unwrap();
        assert_eq!(ast, call("factorial", vec![num(5.0)]));
    }

    #[test]
    fn test_factorial_on_parenthesized_expr() {
        let ast = parse("(2+3)!").unwrap();
        assert_eq!(
            ast,
            call("factorial", vec![binop(BinaryOp::Add, num(2.0), num(3.0))])
        );
    }

    #[test]
    fn test_factorial_with_unary_minus() {
        let ast = parse("-5!").unwrap();
        assert_eq!(ast, unary(UnaryOp::Neg, call("factorial", vec![num(5.0)])));
    }

    // ===== 取模运算解析 =====

    #[test]
    fn test_simple_modulo() {
        let ast = parse("10%3").unwrap();
        assert_eq!(ast, call("mod", vec![num(10.0), num(3.0)]));
    }

    #[test]
    fn test_modulo_with_arithmetic() {
        let ast = parse("10%3+1").unwrap();
        assert_eq!(
            ast,
            binop(
                BinaryOp::Add,
                call("mod", vec![num(10.0), num(3.0)]),
                num(1.0)
            )
        );
    }

    #[test]
    fn test_modulo_with_subexpressions() {
        let ast = parse("(2+8)%(4-1)").unwrap();
        assert_eq!(
            ast,
            call(
                "mod",
                vec![
                    binop(BinaryOp::Add, num(2.0), num(8.0)),
                    binop(BinaryOp::Sub, num(4.0), num(1.0))
                ]
            )
        );
    }

    // ===== 函数调用解析 =====

    #[test]
    fn test_single_arg_function() {
        let ast = parse("sin(pi/2)").unwrap();
        assert_eq!(
            ast,
            call("sin", vec![binop(BinaryOp::Div, var("pi"), num(2.0))])
        );
    }

    #[test]
    fn test_multi_arg_function() {
        let ast = parse("log(100, 10)").unwrap();
        assert_eq!(ast, call("log", vec![num(100.0), num(10.0)]));
    }

    #[test]
    fn test_special_function_gamma() {
        let ast = parse("gamma(5)").unwrap();
        assert_eq!(ast, call("gamma", vec![num(5.0)]));
    }

    #[test]
    fn test_nested_function_calls() {
        let ast = parse("sin(cos(0))").unwrap();
        assert_eq!(ast, call("sin", vec![call("cos", vec![num(0.0)])]));
    }

    // ===== 变量引用解析 =====

    #[test]
    fn test_simple_variable_expr() {
        let ast = parse("x+y").unwrap();
        assert_eq!(ast, binop(BinaryOp::Add, var("x"), var("y")));
    }

    #[test]
    fn test_variable_as_function_arg() {
        let ast = parse("sin(x)").unwrap();
        assert_eq!(ast, call("sin", vec![var("x")]));
    }

    #[test]
    fn test_multichar_variable_names() {
        let ast = parse("alpha+beta_1").unwrap();
        assert_eq!(ast, binop(BinaryOp::Add, var("alpha"), var("beta_1")));
    }

    // ===== 括号嵌套解析 =====

    #[test]
    fn test_double_nested_parens() {
        let ast = parse("((1+2)*3)").unwrap();
        assert_eq!(
            ast,
            binop(
                BinaryOp::Mul,
                binop(BinaryOp::Add, num(1.0), num(2.0)),
                num(3.0)
            )
        );
    }

    #[test]
    fn test_multi_level_parens() {
        let ast = parse("(1+(2+3))*4").unwrap();
        assert_eq!(
            ast,
            binop(
                BinaryOp::Mul,
                binop(
                    BinaryOp::Add,
                    num(1.0),
                    binop(BinaryOp::Add, num(2.0), num(3.0))
                ),
                num(4.0)
            )
        );
    }

    #[test]
    fn test_optional_parens_equivalent() {
        let with_parens = parse("(1+2)").unwrap();
        let without_parens = parse("1+2").unwrap();
        assert_eq!(with_parens, without_parens);
    }

    // ===== 数学常量解析 =====

    #[test]
    fn test_pi_constant() {
        let ast = parse("pi").unwrap();
        assert_eq!(ast, var("pi"));
    }

    #[test]
    fn test_e_constant_in_expr() {
        let ast = parse("e*2").unwrap();
        assert_eq!(ast, binop(BinaryOp::Mul, var("e"), num(2.0)));
    }

    #[test]
    fn test_constant_in_function_arg() {
        let ast = parse("cos(pi)").unwrap();
        assert_eq!(ast, call("cos", vec![var("pi")]));
    }

    // ===== 无效表达式拒绝 =====

    #[test]
    fn test_empty_string_rejected() {
        let err = parse("").unwrap_err();
        assert!(err.kind == ErrorKind::Parse && err.message.contains("empty"));
    }

    #[test]
    fn test_unmatched_parens_rejected() {
        let err1 = parse("(2+3").unwrap_err();
        assert!(err1.kind == ErrorKind::Parse);

        let err2 = parse("2+3)").unwrap_err();
        assert!(err2.kind == ErrorKind::Parse);
    }

    #[test]
    fn test_consecutive_operators_rejected() {
        // mathexpr 将 `+3` 当作数字字面量，需由 CalNexus 预处理层显式拒绝 `++`
        let err = parse("2++3").unwrap_err();
        assert!(err.kind == ErrorKind::Parse && err.message.contains("consecutive operators"));
    }

    // ===== 回归测试：显式拒绝 **、//、^^（-- 合法，双重负号） =====
    #[test]
    fn test_consecutive_mul_operators_rejected() {
        // `2**3` 应被显式拒绝，错误信息必须明确指出 `**` 并使用专用 i18n key
        let err = parse("2**3").unwrap_err();
        assert_eq!(err.kind, ErrorKind::Parse);
        assert!(
            err.message.contains("consecutive operators"),
            "error message should mention 'consecutive operators', got: {}",
            err.message
        );
        assert!(
            err.message.contains("**"),
            "error message should mention '**', got: {}",
            err.message
        );
        assert_eq!(
            err.i18n_key,
            Some("msg.core.parse_illegal_consecutive_ops"),
            "should use dedicated i18n key, got: {:?}",
            err.i18n_key
        );
    }

    #[test]
    fn test_consecutive_div_operators_rejected() {
        // `2//3` 应被显式拒绝
        let err = parse("2//3").unwrap_err();
        assert_eq!(err.kind, ErrorKind::Parse);
        assert!(
            err.message.contains("consecutive operators") && err.message.contains("//"),
            "error message should mention 'consecutive operators' and '//', got: {}",
            err.message
        );
        assert_eq!(err.i18n_key, Some("msg.core.parse_illegal_consecutive_ops"));
    }

    #[test]
    fn test_consecutive_pow_operators_rejected() {
        // `2^^3` 应被显式拒绝
        let err = parse("2^^3").unwrap_err();
        assert_eq!(err.kind, ErrorKind::Parse);
        assert!(
            err.message.contains("consecutive operators") && err.message.contains("^^"),
            "error message should mention 'consecutive operators' and '^^', got: {}",
            err.message
        );
        assert_eq!(err.i18n_key, Some("msg.core.parse_illegal_consecutive_ops"));
    }

    #[test]
    fn test_consecutive_operators_with_whitespace_rejected() {
        // `2* *3` 中间有空格也应被检测
        let err = parse("2* *3").unwrap_err();
        assert_eq!(err.kind, ErrorKind::Parse);
        assert_eq!(err.i18n_key, Some("msg.core.parse_illegal_consecutive_ops"));
    }

    #[test]
    fn test_double_negation_still_allowed() {
        // `--` 合法（双重负号），扩展检查不能误伤
        let result = parse("2--3");
        assert!(
            result.is_ok(),
            "'--' should be allowed (double negation), got error: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_single_operator_still_allowed() {
        // 单个运算符仍合法：扩展检查不能误伤正常表达式
        assert!(parse("2*3").is_ok(), "'*' alone should be allowed");
        assert!(parse("6/3").is_ok(), "'/' alone should be allowed");
        assert!(parse("2^3").is_ok(), "'^' alone should be allowed");
        assert!(parse("2+3").is_ok(), "'+' alone should be allowed");
    }

    #[test]
    fn test_unclosed_function_rejected() {
        let err = parse("sin(").unwrap_err();
        assert!(err.kind == ErrorKind::Parse);
    }

    #[test]
    fn test_operator_only_rejected() {
        assert!(parse("+").is_err());
        assert!(parse("*").is_err());
    }

    // ===== 10: AST 深度限制 =====

    #[test]
    fn test_depth_at_limit_passes() {
        // 256 个 1 用 + 连接 → 左结合 AST 深度 = 256
        let expr = format!("1{}", "+1".repeat(255));
        assert!(parse(&expr).is_ok());
    }

    #[test]
    fn test_depth_exceeds_limit_rejected() {
        // 257 个 1 用 + 连接 → AST 深度 = 257
        let expr = format!("1{}", "+1".repeat(256));
        let err = parse(&expr).unwrap_err();
        assert_eq!(err, CalcError::depth_exceeded());
    }

    #[test]
    fn test_depth_check_at_parse_time() {
        let deep_expr = format!("1{}", "+1".repeat(256));
        assert!(matches!(parse(&deep_expr), Err(e) if e.kind == ErrorKind::Depth));
    }

    // ===== 11: 表达式长度限制 =====

    #[test]
    fn test_length_at_limit_passes() {
        // 构造恰好 4096 字符的合法表达式（长变量名 + 1，AST 深度 = 2）
        let var_name = "a".repeat(4094);
        let expr = format!("{}+1", var_name);
        assert_eq!(expr.len(), MAX_EXPR_LEN);
        assert!(parse(&expr).is_ok());
    }

    #[test]
    fn test_length_exceeds_limit_rejected() {
        // 4097 字符
        let mut expr = String::from("a");
        while expr.len() <= MAX_EXPR_LEN {
            expr.push_str("+1");
        }
        assert!(expr.len() > MAX_EXPR_LEN);
        let err = parse(&expr).unwrap_err();
        assert!(err.kind == ErrorKind::Parse && err.message.contains("length"));
    }

    #[test]
    fn test_oversized_input_fast_fail() {
        let expr = "a".repeat(100_000);
        let err = parse(&expr).unwrap_err();
        assert!(err.kind == ErrorKind::Parse);
    }

    // ===== 额外边界测试 =====

    #[test]
    fn test_whitespace_trimmed() {
        let ast = parse("  2+3  ").unwrap();
        assert_eq!(ast, binop(BinaryOp::Add, num(2.0), num(3.0)));
    }

    #[test]
    fn test_decimal_numbers() {
        let ast = parse("3.14").unwrap();
        assert_eq!(ast, num(3.14));
    }

    #[test]
    fn test_double_factorial() {
        // 5!! → factorial(factorial(5))
        let ast = parse("5!!").unwrap();
        assert_eq!(
            ast,
            call("factorial", vec![call("factorial", vec![num(5.0)])])
        );
    }

    #[test]
    fn test_factorial_on_variable() {
        let ast = parse("x!").unwrap();
        assert_eq!(ast, call("factorial", vec![var("x")]));
    }

    #[test]
    fn test_factorial_on_function_call() {
        let ast = parse("sin(x)!").unwrap();
        assert_eq!(ast, call("factorial", vec![call("sin", vec![var("x")])]));
    }

    #[test]
    fn test_factorial_in_expression() {
        // "2*3!" → Mul(2, factorial(3))
        let ast = parse("2*3!").unwrap();
        assert_eq!(
            ast,
            binop(BinaryOp::Mul, num(2.0), call("factorial", vec![num(3.0)]))
        );
    }

    // ===== 复数字面量解析测试 =====

    #[test]
    fn test_complex_standard_literal() {
        // `3+4i` → Complex(3, 4)
        let ast = parse("3+4i").unwrap();
        assert_eq!(ast, AstNode::Complex(3.0, 4.0));
    }

    #[test]
    fn test_complex_pure_imaginary() {
        // `2i` → Complex(0, 2)
        let ast = parse("2i").unwrap();
        assert_eq!(ast, AstNode::Complex(0.0, 2.0));
    }

    #[test]
    fn test_complex_negative_imaginary() {
        // `3-4i` → Complex(3, -4)
        let ast = parse("3-4i").unwrap();
        assert_eq!(ast, AstNode::Complex(3.0, -4.0));
    }

    #[test]
    fn test_complex_real_only_not_triggered() {
        // `5` → Number(5)，不触发复数
        let ast = parse("5").unwrap();
        assert_eq!(ast, num(5.0));
    }

    // ===== 矩阵字面量解析测试 =====

    #[test]
    fn test_matrix_2x2_literal() {
        // `[[1,2],[3,4]]` → 2x2 Matrix
        let ast = parse("[[1,2],[3,4]]").unwrap();
        assert_eq!(
            ast,
            AstNode::Matrix(vec![vec![num(1.0), num(2.0)], vec![num(3.0), num(4.0)],])
        );
    }

    #[test]
    fn test_matrix_2x3_literal() {
        // `[[1,2,3],[4,5,6]]` → 2x3 Matrix
        let ast = parse("[[1,2,3],[4,5,6]]").unwrap();
        assert_eq!(
            ast,
            AstNode::Matrix(vec![
                vec![num(1.0), num(2.0), num(3.0)],
                vec![num(4.0), num(5.0), num(6.0)],
            ])
        );
    }

    #[test]
    fn test_matrix_1x3_literal() {
        // `[[1,2,3]]` → 1x3 Matrix
        let ast = parse("[[1,2,3]]").unwrap();
        assert_eq!(
            ast,
            AstNode::Matrix(vec![vec![num(1.0), num(2.0), num(3.0)]])
        );
    }

    // ===== 列表字面量解析测试 =====

    #[test]
    fn test_list_standard_literal() {
        // `[1,2,3,4,5]` → 5 元素 List
        let ast = parse("[1,2,3,4,5]").unwrap();
        assert_eq!(
            ast,
            AstNode::List(vec![num(1.0), num(2.0), num(3.0), num(4.0), num(5.0),])
        );
    }

    #[test]
    fn test_list_single_element() {
        // `[42]` → 单元素 List
        let ast = parse("[42]").unwrap();
        assert_eq!(ast, AstNode::List(vec![num(42.0)]));
    }

    // ===== 覆盖 parse_bracket_literal else 分支 =====

    #[test]
    fn test_parse_bracket_literal_non_bracket_input() {
        // 直接调用 parse_bracket_literal 传入非括号字符串，覆盖 else 分支
        let result = parse_bracket_literal("abc");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.kind == ErrorKind::Parse && err.message.contains("expected bracket literal"));
    }

    // ===== 覆盖 preprocess_brackets 未匹配 '[' 错误 =====

    #[test]
    fn test_unmatched_open_bracket_rejected() {
        // `[[1,2]` 只有一个 `]`，覆盖未匹配 '[' 错误
        let result = parse("[[1,2]");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.kind == ErrorKind::Parse && err.message.contains("unmatched '['"));
    }

    // ===== 覆盖 replace_placeholders Matrix/List 分支 =====

    #[test]
    fn test_replace_placeholders_in_matrix() {
        // 直接调用 replace_placeholders，覆盖 Matrix 分支
        let mut ast = AstNode::Matrix(vec![vec![
            AstNode::Variable("__bn_0".to_string()),
            AstNode::Number(1.0),
        ]]);
        let mut placeholders = std::collections::HashMap::new();
        placeholders.insert(
            "__bn_0".to_string(),
            AstNode::BigNumber("1234567890123456".to_string()),
        );
        replace_placeholders(&mut ast, &placeholders);
        let AstNode::Matrix(rows) = &ast else {
            panic!("expected Matrix after placeholder replacement")
        };
        assert_eq!(
            rows[0][0],
            AstNode::BigNumber("1234567890123456".to_string())
        );
        assert_eq!(rows[0][1], AstNode::Number(1.0));
    }

    #[test]
    fn test_replace_placeholders_in_list() {
        // 直接调用 replace_placeholders，覆盖 List 分支
        let mut ast = AstNode::List(vec![
            AstNode::Variable("__bn_0".to_string()),
            AstNode::Number(2.0),
        ]);
        let mut placeholders = std::collections::HashMap::new();
        placeholders.insert(
            "__bn_0".to_string(),
            AstNode::BigNumber("9876543210987654".to_string()),
        );
        replace_placeholders(&mut ast, &placeholders);
        let AstNode::List(elements) = &ast else {
            panic!("expected List after placeholder replacement")
        };
        assert_eq!(
            elements[0],
            AstNode::BigNumber("9876543210987654".to_string())
        );
        assert_eq!(elements[1], AstNode::Number(2.0));
    }

    // ===== 覆盖 parse_matrix_literal / parse_list_literal 无效输入 =====

    #[test]
    fn test_parse_matrix_literal_invalid_input() {
        // 直接调用 parse_matrix_literal 传入非矩阵字符串
        let result = parse_matrix_literal("[1,2]");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.kind == ErrorKind::Parse && err.message.contains("invalid matrix literal"));
    }

    #[test]
    fn test_parse_list_literal_invalid_input() {
        // 直接调用 parse_list_literal 传入非列表字符串
        let result = parse_list_literal("1,2");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.kind == ErrorKind::Parse && err.message.contains("invalid list literal"));
    }

    // ===== 覆盖 split_by_pattern 括号深度分支 =====

    #[test]
    fn test_list_with_parens_in_elements() {
        // 列表元素含括号表达式，覆盖 split_by_pattern 的 paren_depth 分支
        let ast = parse("[sin(x), 2]").unwrap();
        assert_eq!(
            ast,
            AstNode::List(vec![call("sin", vec![var("x")]), num(2.0)])
        );
    }

    #[test]
    fn test_matrix_with_parens_in_elements() {
        // 矩阵元素含括号表达式：覆盖 split_by_pattern 的 paren_depth 分支
        let ast = parse("[[sin(x), 2]]").unwrap();
        assert_eq!(
            ast,
            AstNode::Matrix(vec![vec![call("sin", vec![var("x")]), num(2.0)]])
        );
    }

    // ===== 覆盖 find_operand_start 边界 =====

    #[test]
    fn test_factorial_with_space_before_operator() {
        // `5 !` → factorial(5)，覆盖跳过尾部空格的路径
        let ast = parse("5 !").unwrap();
        assert_eq!(ast, call("factorial", vec![num(5.0)]));
    }

    #[test]
    fn test_factorial_no_operand_error() {
        // `!5` — `!` 前无操作数，覆盖 pos==0 错误路径
        let result = parse("!5");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.kind == ErrorKind::Parse && err.message.contains("no operand"));
    }

    #[test]
    fn test_factorial_on_nested_parens() {
        // `((1+2))!` → factorial(((1+2)))，覆盖嵌套 `)` 匹配
        let ast = parse("((1+2))!").unwrap();
        assert_eq!(
            ast,
            call("factorial", vec![binop(BinaryOp::Add, num(1.0), num(2.0))])
        );
    }

    #[test]
    fn test_factorial_unmatched_paren_error() {
        // `1+2)!` — `!` 前有未匹配的 `)`，覆盖 unmatched parenthesis 错误
        let result = parse("1+2)!");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.kind == ErrorKind::Parse && err.message.contains("unmatched parenthesis"));
    }

    // ===== 覆盖 convert_with_depth CurrentValue =====

    #[test]
    fn test_parse_current_value_underscore() {
        // `_` → mathexpr CurrentValue → Variable("_")
        let ast = parse("_").unwrap();
        assert_eq!(ast, var("_"));
    }

    // ===== 覆盖 complex() 函数非 Number 参数路径 =====

    #[test]
    fn test_complex_call_with_variable_real() {
        // `complex(x, 4)` — re 不是 Number，回退普通 FunctionCall
        let ast = parse("complex(x, 4)").unwrap();
        assert_eq!(ast, call("complex", vec![var("x"), num(4.0)]));
    }

    #[test]
    fn test_complex_call_with_variable_imag() {
        // `complex(3, x)` — im 不是 Number/Neg，回退普通 FunctionCall
        let ast = parse("complex(3, x)").unwrap();
        assert_eq!(ast, call("complex", vec![num(3.0), var("x")]));
    }

    #[test]
    fn test_complex_call_with_neg_variable_imag() {
        // `complex(3, -x)` — im 是 Neg(Variable)，回退普通 FunctionCall
        let ast = parse("complex(3, -x)").unwrap();
        assert_eq!(
            ast,
            call("complex", vec![num(3.0), unary(UnaryOp::Neg, var("x"))])
        );
    }

    // ===== 隐式乘法预处理测试 =====

    #[test]
    fn test_implicit_mult_number_before_variable() {
        // `2x` → `2*x` → Mul(2, x)
        let ast = parse("2x").unwrap();
        assert_eq!(ast, binop(BinaryOp::Mul, num(2.0), var("x")));
    }

    #[test]
    fn test_implicit_mult_number_before_paren() {
        // `3(x+1)` → `3*(x+1)` → Mul(3, Add(x, 1))
        let ast = parse("3(x+1)").unwrap();
        assert_eq!(
            ast,
            binop(
                BinaryOp::Mul,
                num(3.0),
                binop(BinaryOp::Add, var("x"), num(1.0))
            )
        );
    }

    #[test]
    fn test_implicit_mult_close_paren_before_open_paren() {
        // `(x+1)(x-1)` → `(x+1)*(x-1)` → Mul(Add(x,1), Sub(x,1))
        let ast = parse("(x+1)(x-1)").unwrap();
        assert_eq!(
            ast,
            binop(
                BinaryOp::Mul,
                binop(BinaryOp::Add, var("x"), num(1.0)),
                binop(BinaryOp::Sub, var("x"), num(1.0))
            )
        );
    }

    #[test]
    fn test_implicit_mult_variable_before_paren_not_triggered() {
        // `sin(x)` → FunctionCall, NOT Mul(sin, x)
        let ast = parse("sin(x)").unwrap();
        assert_eq!(ast, call("sin", vec![var("x")]));
    }

    #[test]
    fn test_implicit_mult_number_before_constant() {
        // `2pi` → `2*pi` → Mul(2, Variable("pi"))
        let ast = parse("2pi").unwrap();
        assert_eq!(ast, binop(BinaryOp::Mul, num(2.0), var("pi")));
    }

    #[test]
    fn test_implicit_mult_close_paren_before_variable() {
        // `(x+1)x` → `(x+1)*x`
        let ast = parse("(x+1)x").unwrap();
        assert_eq!(
            ast,
            binop(
                BinaryOp::Mul,
                binop(BinaryOp::Add, var("x"), num(1.0)),
                var("x")
            )
        );
    }

    #[test]
    fn test_implicit_mult_close_paren_before_number() {
        // `(x+1)2` → `(x+1)*2`
        let ast = parse("(x+1)2").unwrap();
        assert_eq!(
            ast,
            binop(
                BinaryOp::Mul,
                binop(BinaryOp::Add, var("x"), num(1.0)),
                num(2.0)
            )
        );
    }

    #[test]
    fn test_implicit_mult_identifier_digit_not_triggered() {
        // `beta_1` → 单个变量，不插入 `*`
        let ast = parse("beta_1+1").unwrap();
        assert_eq!(ast, binop(BinaryOp::Add, var("beta_1"), num(1.0)));
    }

    #[test]
    fn test_implicit_mult_decimal_number() {
        // `3.14x` → `3.14*x`（小数点不干扰数字结尾判断）
        let ast = parse("3.14x").unwrap();
        assert_eq!(ast, binop(BinaryOp::Mul, num(3.14), var("x")));
    }

    // ===== Symbolic 函数解析测试 =====

    #[test]
    fn test_parse_diff_function() {
        // `diff(x^2, x)` → FunctionCall("diff", [Pow(x, 2), Variable("x")])
        let ast = parse("diff(x^2, x)").unwrap();
        assert_eq!(
            ast,
            call(
                "diff",
                vec![binop(BinaryOp::Pow, var("x"), num(2.0)), var("x"),]
            )
        );
    }

    #[test]
    fn test_parse_integrate_function() {
        // `integrate(sin(x), x)` → FunctionCall("integrate", [Sin(x), Variable("x")])
        let ast = parse("integrate(sin(x), x)").unwrap();
        assert_eq!(
            ast,
            call("integrate", vec![call("sin", vec![var("x")]), var("x"),])
        );
    }

    #[test]
    fn test_parse_simplify_function() {
        // `simplify(x^2+2*x+1)` → FunctionCall("simplify", [...])
        let ast = parse("simplify(x^2+2*x+1)").unwrap();
        let expected_inner = binop(
            BinaryOp::Add,
            binop(
                BinaryOp::Add,
                binop(BinaryOp::Pow, var("x"), num(2.0)),
                binop(BinaryOp::Mul, num(2.0), var("x")),
            ),
            num(1.0),
        );
        assert_eq!(ast, call("simplify", vec![expected_inner]));
    }

    #[test]
    fn test_parse_limit_function() {
        // `limit(x^2, x, 0)` → FunctionCall("limit", [Pow(x,2), Variable("x"), Number(0)])
        let ast = parse("limit(x^2, x, 0)").unwrap();
        assert_eq!(
            ast,
            call(
                "limit",
                vec![binop(BinaryOp::Pow, var("x"), num(2.0)), var("x"), num(0.0),]
            )
        );
    }

    #[test]
    fn test_parse_taylor_function() {
        // `taylor(sin(x), x, 5)` → FunctionCall("taylor", [Sin(x), Variable("x"), Number(5)])
        let ast = parse("taylor(sin(x), x, 5)").unwrap();
        assert_eq!(
            ast,
            call(
                "taylor",
                vec![call("sin", vec![var("x")]), var("x"), num(5.0),]
            )
        );
    }

    #[test]
    fn test_symbolic_function_not_implicit_mult() {
        // `diff(x)` 是函数调用，不是 `diff * (x)` 隐式乘法
        let ast = parse("diff(x)").unwrap();
        assert_eq!(ast, call("diff", vec![var("x")]));
        assert!(!matches!(ast, AstNode::BinaryOp(BinaryOp::Mul, _, _)));
    }

    // ===== 覆盖 is_scientific_notation 各分支 =====

    #[test]
    fn test_is_scientific_notation_e_followed_by_digit() {
        // e 后跟数字 → true
        let chars: Vec<char> = "1e5".chars().collect();
        assert!(is_scientific_notation(&chars, 1));
    }

    #[test]
    fn test_is_scientific_notation_e_followed_by_plus_digit() {
        // e+ 后跟数字 → true
        let chars: Vec<char> = "1e+5".chars().collect();
        assert!(is_scientific_notation(&chars, 1));
    }

    #[test]
    fn test_is_scientific_notation_e_followed_by_minus_digit() {
        // e- 后跟数字 → true
        let chars: Vec<char> = "1e-5".chars().collect();
        assert!(is_scientific_notation(&chars, 1));
    }

    #[test]
    fn test_is_scientific_notation_e_followed_by_plus_non_digit() {
        // e+ 后跟非数字 → false（lines 562-566）
        let chars: Vec<char> = "1e+x".chars().collect();
        assert!(!is_scientific_notation(&chars, 1));
    }

    #[test]
    fn test_is_scientific_notation_e_at_end() {
        // e 在字符串末尾 → false
        let chars: Vec<char> = "1e".chars().collect();
        assert!(!is_scientific_notation(&chars, 1));
    }

    #[test]
    fn test_is_scientific_notation_e_plus_at_end() {
        // e+ 在字符串末尾 → false
        let chars: Vec<char> = "1e+".chars().collect();
        assert!(!is_scientific_notation(&chars, 1));
    }

    #[test]
    fn test_is_scientific_notation_e_followed_by_non_digit_non_sign() {
        // e 后跟非数字非符号 → false
        let chars: Vec<char> = "1ex".chars().collect();
        assert!(!is_scientific_notation(&chars, 1));
    }

    // ===== proptest 属性测试 =====

    use proptest::prelude::*;

    /// 生成简单合法表达式："数字 op 数字"
    fn valid_expr_strategy() -> impl Strategy<Value = String> {
        let num = (1u32..10000).prop_map(|n| n.to_string());
        let op = prop_oneof![Just("+"), Just("-"), Just("*"), Just("/")];
        (num.clone(), op, num).prop_map(|(a, op, b)| format!("{}{}{}", a, op, b))
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

        // 属性 1：解析幂等性 — 同一表达式多次解析结果相同
        #[test]
        fn prop_parse_idempotent(expr in valid_expr_strategy()) {
            let first = parse(&expr);
            let second = parse(&expr);
            prop_assert_eq!(first, second);
        }

        // 属性 2：合法字符集生成的表达式可解析
        #[test]
        fn prop_valid_expr_parseable(expr in valid_expr_strategy()) {
            let result = parse(&expr);
            prop_assert!(result.is_ok(), "expected Ok for valid expr {:?}, got {:?}", expr, result);
        }

        // 属性 3：深度限制 — N 个 1 用 + 连接，AST 深度 = N
        #[test]
        fn prop_depth_limit(n in 1usize..300) {
            let expr = format!("1{}", "+1".repeat(n - 1));
            let result = parse(&expr);
            if n <= 256 {
                prop_assert!(result.is_ok(), "depth {} should pass, got {:?}", n, result);
            } else {
                prop_assert!(matches!(&result, Err(e) if e.kind == ErrorKind::Depth),
                    "depth {} should fail, got {:?}", n, result);
            }
        }
    }

    // ===== Span 精确性测试 =====

    /// 辅助：断言错误带有 span 且等于期望值。
    fn assert_span(err: &CalcError, expected: Span) {
        assert_eq!(
            err.span,
            Some(expected),
            "span mismatch: got {:?}, expected {:?}, message: {}",
            err.span,
            expected,
            err.message
        );
    }

    #[test]
    fn test_span_empty_expression() {
        // parse("") → 空表达式错误，span = (0, 0)
        let err = parse("").unwrap_err();
        assert_eq!(err.kind, ErrorKind::Parse);
        assert_span(&err, Span::new(0, 0));
    }

    #[test]
    fn test_span_whitespace_only_expression() {
        // parse(" ") → trim 后为空，span = (0, 0)
        let err = parse("   ").unwrap_err();
        assert_eq!(err.kind, ErrorKind::Parse);
        assert_span(&err, Span::new(0, 0));
    }

    #[test]
    fn test_span_length_exceeded() {
        // 超长表达式，span 覆盖整个输入（字符偏移）
        let expr = "a".repeat(MAX_EXPR_LEN + 1);
        let err = parse(&expr).unwrap_err();
        assert_eq!(err.kind, ErrorKind::Parse);
        assert_span(&err, Span::new(0, expr.chars().count()));
    }

    #[test]
    fn test_span_mathexpr_parse_failure() {
        // `@` 不是合法字符，mathexpr 解析失败，span = (0, 1)
        let err = parse("@").unwrap_err();
        assert_eq!(err.kind, ErrorKind::Parse);
        assert_span(&err, Span::new(0, 1));
    }

    /// 隐式乘法预处理后，错误 Span 仍指向原始输入而非 after_implicit。
    #[test]
    fn test_span_mathexpr_failure_points_to_original_input() {
        let err = parse("2x@").unwrap_err();
        assert_eq!(err.kind, ErrorKind::Parse);
        // 期望 Span 指向原始输入 "2x@"（3 字符），而非 after_implicit "2*x@"（4 字符）
        assert_span(&err, Span::new(0, 3));
    }

    #[test]
    fn test_span_unmatched_open_bracket() {
        // `[[1,2]` 未匹配 `[`，span = (0, 6)
        let err = parse("[[1,2]").unwrap_err();
        assert_eq!(err.kind, ErrorKind::Parse);
        assert_span(&err, Span::new(0, 6));
    }

    #[test]
    fn test_span_invalid_bracket_literal() {
        // parse_bracket_literal("abc") → 非括号字面量，span = (0, 3)
        let err = parse_bracket_literal("abc").unwrap_err();
        assert_eq!(err.kind, ErrorKind::Parse);
        assert_span(&err, Span::new(0, 3));
    }

    #[test]
    fn test_span_invalid_matrix_literal() {
        // parse_matrix_literal("[1,2]") → 不以 `[[` 开头，span = (0, 5)
        let err = parse_matrix_literal("[1,2]").unwrap_err();
        assert_eq!(err.kind, ErrorKind::Parse);
        assert_span(&err, Span::new(0, 5));
    }

    #[test]
    fn test_span_invalid_list_literal() {
        // parse_list_literal("1,2") → 不以 `[` 开头，span = (0, 3)
        let err = parse_list_literal("1,2").unwrap_err();
        assert_eq!(err.kind, ErrorKind::Parse);
        assert_span(&err, Span::new(0, 3));
    }

    #[test]
    fn test_span_consecutive_plus() {
        // `2++3` → `++` 从位置 1 开始，span = (1, 3)
        let err = parse("2++3").unwrap_err();
        assert_eq!(err.kind, ErrorKind::Parse);
        assert_span(&err, Span::new(1, 3));
    }

    #[test]
    fn test_span_consecutive_plus_with_spaces() {
        // `2 + + 3` span 基于原始输入（+ 在位置 2 和 4），而非去空格后
        let err = parse("2 + + 3").unwrap_err();
        assert_eq!(err.kind, ErrorKind::Parse);
        assert_span(&err, Span::new(2, 5));
    }

    #[test]
    fn test_span_factorial_no_operand() {
        // `!5` → `!` 在位置 0，无操作数，span = point(0) = (0, 1)
        let err = parse("!5").unwrap_err();
        assert_eq!(err.kind, ErrorKind::Parse);
        assert_span(&err, Span::point(0));
    }

    #[test]
    fn test_span_factorial_unmatched_paren() {
        // `1+2)!` → `!` 在位置 4，`!` 前有未匹配的 `)`
        let err = parse("1+2)!").unwrap_err();
        assert_eq!(err.kind, ErrorKind::Parse);
        assert_span(&err, Span::point(4));
    }

    // ===== 字符串字面量解析（Str 节点、转义、未闭合、管线顺序） =====

    /// `date("2026-07-25")` → FunctionCall("date", [Str("2026-07-25")])
    #[test]
    fn test_parse_string_literal_in_function_arg() {
        let ast = parse(r#"date("2026-07-25")"#).unwrap();
        assert_eq!(
            ast,
            call("date", vec![AstNode::Str("2026-07-25".to_string())])
        );
    }

    /// 多参数函数中字符串可出现在任意位置（首参/中参/尾参）
    #[test]
    fn test_parse_string_literal_at_any_position() {
        // 字符串在尾参：convert(100,"cm","m")
        let ast = parse(r#"convert(100,"cm","m")"#).unwrap();
        assert_eq!(
            ast,
            call(
                "convert",
                vec![
                    num(100.0),
                    AstNode::Str("cm".to_string()),
                    AstNode::Str("m".to_string()),
                ]
            )
        );

        // 字符串在首参：format_date("%Y", ts)
        let ast = parse(r#"format_date("%Y", 0)"#).unwrap();
        assert_eq!(
            ast,
            call(
                "format_date",
                vec![AstNode::Str("%Y".to_string()), num(0.0)]
            )
        );

        // 字符串在中参：reformat_date(input, "%d/%m/%Y", "%Y-%m-%d")
        let ast = parse(r#"reformat_date(0, "%d/%m/%Y", "%Y-%m-%d")"#).unwrap();
        assert_eq!(
            ast,
            call(
                "reformat_date",
                vec![
                    num(0.0),
                    AstNode::Str("%d/%m/%Y".to_string()),
                    AstNode::Str("%Y-%m-%d".to_string()),
                ]
            )
        );
    }

    /// `\"` 转义为字面双引号
    #[test]
    fn test_parse_string_literal_with_escaped_quote() {
        let ast = parse(r#"f("a\"b")"#).unwrap();
        assert_eq!(ast, call("f", vec![AstNode::Str("a\"b".to_string())]));
    }

    /// 未闭合引号返回 ParseError，消息含 "unclosed string"
    #[test]
    fn test_parse_string_literal_unclosed_rejected() {
        let err = parse(r#"f("unclosed)"#).unwrap_err();
        assert_eq!(err.kind, ErrorKind::Parse);
        assert!(
            err.message.contains("unclosed string"),
            "error message should mention 'unclosed string', got: {}",
            err.message
        );
    }

    /// 空字符串 `f("")` 合法，产出 Str("")
    #[test]
    fn test_parse_string_literal_empty() {
        let ast = parse(r#"f("")"#).unwrap();
        assert_eq!(ast, call("f", vec![AstNode::Str("".to_string())]));
    }

    /// 字符串内的 `[` 不被 brackets 预处理提取：`f("[1,2]")` → Str("[1,2]")
    #[test]
    fn test_parse_string_literal_preserves_brackets() {
        let ast = parse(r#"f("[1,2]")"#).unwrap();
        assert_eq!(ast, call("f", vec![AstNode::Str("[1,2]".to_string())]));
    }

    /// 字符串内的长数字不被 bigint 预处理提取（产出 Str 而非 BigNumber）
    #[test]
    fn test_parse_string_literal_preserves_bigint() {
        let ast = parse(r#"f("12345678901234567890")"#).unwrap();
        assert_eq!(
            ast,
            call("f", vec![AstNode::Str("12345678901234567890".to_string())])
        );
    }

    /// 边界：字符串含特殊字符（空格、`-`、`/`、`:`、UTF-8 中文）
    #[test]
    fn test_parse_string_literal_with_special_chars() {
        let ast = parse(r#"f("2026-07-25 12:30:00")"#).unwrap();
        assert_eq!(
            ast,
            call("f", vec![AstNode::Str("2026-07-25 12:30:00".to_string())])
        );
    }

    /// 边界：字符串内的嵌套引号（已转义）不影响外层解析
    #[test]
    fn test_parse_string_literal_with_nested_escaped_quotes() {
        // 输入: f("a\"b\"c") → Str 内容为 a"b"c
        let ast = parse(r#"f("a\"b\"c")"#).unwrap();
        assert_eq!(ast, call("f", vec![AstNode::Str("a\"b\"c".to_string())]));
    }

    /// 多个字符串字面量验证占位符 `__str_N` 递增编号
    #[test]
    fn test_parse_multiple_string_literals() {
        let ast = parse(r#"f("a", "b", "c")"#).unwrap();
        assert_eq!(
            ast,
            call(
                "f",
                vec![
                    AstNode::Str("a".to_string()),
                    AstNode::Str("b".to_string()),
                    AstNode::Str("c".to_string()),
                ]
            )
        );
    }
}
