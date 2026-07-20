// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! 表达式解析器：将数学表达式字符串解析为 [`AstNode`]。
//!
//! 基于 mathexpr crate，添加：
//! - 阶乘 `!` 预处理（mathexpr 不原生支持 `!`，转换为 `factorial()`）
//! - AST 转换层（mathexpr `Expr` → CalNexus `AstNode`）
//! - 深度/长度限制（DoS 防护）
//!
//! 设计依据：design.md D2（mathexpr 集成）、D7（TDD）、expression-parsing spec

use crate::core::types::{AstNode, BinaryOp, CalcError, Span, UnaryOp};
use regex::Regex;

/// 最大 AST 深度（spec: AST 深度限制 ≤ 256）。
const MAX_AST_DEPTH: usize = 256;

/// 最大表达式长度（spec: 表达式长度限制 ≤ 4096 字符）。
pub(crate) const MAX_EXPR_LEN: usize = 4096;

/// 解析数学表达式字符串为 [`AstNode`]。
///
/// # 流程
/// 1. 长度检查（O(1) 快速失败）
/// 2. 空字符串检查
/// 3. 若以 `[` 开头：矩阵或列表字面量，走自定义解析器
/// 4. 否则：
///    a. 复数预处理（`3+4i` → `complex(3,4)`）
///    b. 阶乘预处理：`5!` → `factorial(5)`
///    c. mathexpr 解析
///    d. 转换为 CalNexus AstNode（`complex()` → `Complex`，`%` → `mod()`，`pi`/`e` → Variable）
pub fn parse(input: &str) -> Result<AstNode, CalcError> {
    // 长度检查（spec: 超长输入不进入词法分析）
    // 注意：input.len() 是字节长度（O(1)），用于快速失败；
    // Span 用字符偏移（design.md D1），故 chars().count()（O(n)）仅在错误分支计算
    if input.len() > MAX_EXPR_LEN {
        let char_count = input.chars().count();
        return Err(CalcError::parse(format!(
            "expression length {} exceeds maximum of {} characters",
            char_count, MAX_EXPR_LEN
        ))
        .with_span(Span::new(0, char_count)));
    }

    let trimmed = input.trim();

    // 空字符串检查
    if trimmed.is_empty() {
        return Err(CalcError::parse("expression is empty".to_string()).with_span(Span::new(0, 0)));
    }

    // 预处理括号字面量：将所有 `[...]` 替换为占位符 `__cb_N`
    // 使矩阵/列表字面量可出现在表达式任意位置（如 `det([[1,2]])`、`2*[[1,2]]`）
    let (without_brackets, mut placeholders) = preprocess_brackets(trimmed)?;

    // 若整个表达式就是单个括号字面量，直接返回（避免 mathexpr 处理）
    if placeholders.len() == 1 {
        let name = placeholders.keys().next().unwrap().clone();
        if without_brackets == name {
            return Ok(placeholders.remove(&name).unwrap());
        }
    }

    // 预处理大整数字面量：将 16+ 位整数替换为占位符 `__bn_N`（避免 f64 精度丢失）
    let without_bigint = preprocess_bigint(&without_brackets, &mut placeholders)?;

    // 非法连续运算符检查（mathexpr 将 `+3` 当作数字字面量，需在此显式拒绝 `++`）
    validate_no_consecutive_plus(&without_bigint)?;

    // 复数预处理：`3+4i` → `complex(3, 4)`、`2i` → `complex(0, 2)`
    let after_complex = preprocess_complex(&without_bigint)?;

    // 阶乘预处理
    let after_factorial = preprocess_factorial(&after_complex)?;

    // 隐式乘法预处理：`2x` → `2*x`、`3(x+1)` → `3*(x+1)`、`(x+1)(x-1)` → `(x+1)*(x-1)`
    let after_implicit = insert_implicit_multiplication(&after_factorial);

    // mathexpr 解析
    let expr = mathexpr::parse(&after_implicit).map_err(|e| {
        // T005: Span 指向原始 trimmed 输入而非预处理后的 after_implicit。
        // 用户看到的是原始输入，错误位置应帮助定位原始输入中的问题；
        // after_implicit 经过隐式乘法等预处理，长度可能与原始输入不同（如 2x → 2*x）。
        // Span 用字符偏移（design.md D1）。
        CalcError::parse(format!("{}", e)).with_span(Span::new(0, trimmed.chars().count()))
    })?;

    // 转换为 CalNexus AstNode（含深度检查，防止递归栈溢出）
    let mut ast = convert_with_depth(&expr, 1)?;

    // 替换占位符为实际的 Matrix/List 节点
    replace_placeholders(&mut ast, &placeholders);

    Ok(ast)
}

/// 复数预处理：将 `a+bi`、`a-bi`、`bi` 转换为 `complex(a, b)` 字符串。
///
/// 匹配规则（design.md D3）：
/// - `3+4i` → `complex(3, 4)`
/// - `3-4i` → `complex(3, -4)`
/// - `2i`   → `complex(0, 2)`
/// - `5`    → 不变（不触发复数）
///
/// 顺序：先匹配 `a±bi`，再匹配纯 `bi`，避免 `3+4i` 中的 `4i` 被先替换。
fn preprocess_complex(input: &str) -> Result<String, CalcError> {
    // 正则1：`a+bi` 或 `a-bi`（a/b 为数字，可能含小数点）
    // 使用 OnceLock 缓存编译后的正则，避免重复编译
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

/// 解析以 `[` 开头的字面量：矩阵 `[[...]]` 或列表 `[...]`（design.md D3）。
fn parse_bracket_literal(input: &str) -> Result<AstNode, CalcError> {
    let trimmed = input.trim();
    if trimmed.starts_with("[[") {
        parse_matrix_literal(trimmed)
    } else if trimmed.starts_with('[') {
        parse_list_literal(trimmed)
    } else {
        Err(
            CalcError::parse(format!("expected bracket literal, got: {}", trimmed))
                .with_span(Span::new(0, trimmed.chars().count())),
        )
    }
}

/// 预处理括号字面量：将所有 `[...]` 子串替换为占位符 `__cb_N`，
/// 并返回 (替换后的字符串, 占位符到 AstNode 的映射)。
///
/// 使矩阵/列表字面量可出现在表达式任意位置（如 `det([[1,2]])`、`2*[[1,2]]`）。
/// 正确匹配嵌套 `[]`，如 `[[1,2],[3,4]]`。
fn preprocess_brackets(
    input: &str,
) -> Result<(String, std::collections::HashMap<String, AstNode>), CalcError> {
    let mut result = String::with_capacity(input.len());
    let mut placeholders: std::collections::HashMap<String, AstNode> =
        std::collections::HashMap::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    let mut count = 0;

    while i < chars.len() {
        if chars[i] == '[' {
            // 找到匹配的 ]
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
                    .with_span(Span::new(start, i)));
            }
            // 提取子串并解析为 AST
            let literal: String = chars[start..i].iter().collect();
            let node = parse_bracket_literal(&literal)?;
            // 生成占位符
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

/// 预处理大整数字面量：将 16+ 位整数替换为占位符 `__bn_N`，
/// 并将原始字符串存入 placeholders 映射为 `AstNode::BigNumber`。
///
/// f64 精确整数范围 ≤ 2^53 ≈ 9e15（15-16 位），超过此范围的整数
/// 会被 mathexpr 解析为 f64 导致精度丢失。此函数在 mathexpr 解析前
/// 提取大整数字面量，保留原始十进制字符串。
///
/// 小数（含 `.` 的数字）不视为大整数，避免误匹配。
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
            // 找到连续数字的结束位置
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
                // 小数字或小数：保持原样
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
        AstNode::Number(_) | AstNode::Complex(_, _) | AstNode::BigNumber(_) => {}
    }
}

/// 解析矩阵字面量 `[[row1],[row2],...]`。
///
/// 每行由 `[elem1,elem2,...]` 组成，元素递归调用 [`parse`]。
fn parse_matrix_literal(input: &str) -> Result<AstNode, CalcError> {
    let trimmed = input.trim();
    // 必须以 `[[` 开头、`]]` 结尾
    if !trimmed.starts_with("[[") || !trimmed.ends_with("]]") {
        return Err(
            CalcError::parse(format!("invalid matrix literal: {}", trimmed))
                .with_span(Span::new(0, trimmed.chars().count())),
        );
    }
    // 去掉外层 `[[` 和 `]]`，得到 `row1],[row2],[...`
    let inner = &trimmed[2..trimmed.len() - 2];
    // 用 `],[` 分割行
    let rows_str = split_by_pattern(inner, "],[");
    let mut rows: Vec<Vec<AstNode>> = Vec::with_capacity(rows_str.len());
    for row_str in &rows_str {
        // 每行用 `[` 和 `]` 包裹，再用 parse_list_literal 解析
        let row_full = format!("[{}]", row_str.trim());
        let row_node = parse_list_literal(&row_full)?;
        match row_node {
            AstNode::List(elements) => rows.push(elements),
            _ => {
                return Err(CalcError::parse(format!(
                    "expected list row in matrix, got: {:?}",
                    row_node
                ))
                .with_span(Span::new(0, trimmed.chars().count())))
            }
        }
    }
    if rows.is_empty() {
        return Err(CalcError::parse("empty matrix literal".to_string())
            .with_span(Span::new(0, trimmed.chars().count())));
    }
    Ok(AstNode::Matrix(rows))
}

/// 解析列表字面量 `[elem1,elem2,...]`。
///
/// 元素递归调用 [`parse`]，支持任意表达式元素。
fn parse_list_literal(input: &str) -> Result<AstNode, CalcError> {
    let trimmed = input.trim();
    // 必须以 `[` 开头、`]` 结尾
    if !trimmed.starts_with('[') || !trimmed.ends_with(']') {
        return Err(
            CalcError::parse(format!("invalid list literal: {}", trimmed))
                .with_span(Span::new(0, trimmed.chars().count())),
        );
    }
    // 去掉 `[` 和 `]`
    let inner = &trimmed[1..trimmed.len() - 1];
    let inner_trimmed = inner.trim();
    // 空列表 `[]`
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

/// 拒绝非法连续运算符 `++`。
///
/// mathexpr 依赖 `nom::number::complete::double`，该解析器接受 `+3` 作为数字字面量，
/// 导致 `2++3` 被静默接受为 `2 + 3.0`。此函数在解析前显式拒绝 `++` 模式。
///
/// Span：从第一个 `+` 到第二个 `+`（含），基于原始输入的字符位置。
fn validate_no_consecutive_plus(input: &str) -> Result<(), CalcError> {
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '+' {
            let start = i;
            // 跳过空格查找下一个 `+`
            let mut j = i + 1;
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
            if j < chars.len() && chars[j] == '+' {
                return Err(
                    CalcError::parse("illegal consecutive operators '++'".to_string())
                        .with_span(Span::new(start, j + 1)),
                );
            }
        }
        i += 1;
    }
    Ok(())
}

/// 预处理阶乘运算符：将 `expr!` 转换为 `factorial(expr)`。
///
/// 支持的操作数形式：
/// - 数字：`5!` → `factorial(5)`
/// - 括号表达式：`(2+3)!` → `factorial((2+3))`
/// - 变量：`x!` → `factorial(x)`
/// - 函数调用：`sin(x)!` → `factorial(sin(x))`
/// - 多重阶乘：`5!!` → `factorial(factorial(5))`
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

/// 从字符缓冲区末尾向前找到操作数的起始索引。
///
/// 操作数识别规则：
/// - 若末尾是 `)`：向左匹配括号，再继续向左扫描函数名（如 `sin(x)` 的 `sin`）
/// - 否则：向左扫描连续的数字、字母、小数点、下划线
fn find_operand_start(chars: &[char]) -> Result<usize, CalcError> {
    let mut pos = chars.len();

    // 跳过尾部空格
    while pos > 0 && chars[pos - 1].is_whitespace() {
        pos -= 1;
    }

    if pos == 0 {
        return Err(CalcError::parse(
            "factorial operator '!' has no operand".to_string(),
        ));
    }

    // 如果最后一个字符是 ')'，向左匹配括号
    if chars[pos - 1] == ')' {
        pos = match_paren_backward(chars, pos - 1)?;
        // pos 现在指向 '(' 的位置
        // 继续向左扫描函数名（如果 '(' 前面有字母）
        pos = scan_identifier_backward(chars, pos);
    } else {
        // 向左扫描连续的数字、字母、小数点、下划线
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

/// 从 `close_idx`（指向 `)`）向左匹配括号，返回对应 `(` 的索引。
///
/// 失败时返回 `unmatched parenthesis` 错误。
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
        return Err(CalcError::parse(
            "unmatched parenthesis in factorial operand".to_string(),
        ));
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

/// 隐式乘法预处理：在相邻 token 间插入 `*`。
///
/// 插入规则（spec: Implicit Multiplication）：
/// - 数字 → 变量/`(`：`2x` → `2*x`、`3(x+1)` → `3*(x+1)`
/// - `)` → 变量/数字/`(`：`(x+1)x` → `(x+1)*x`、`(x+1)2` → `(x+1)*2`、`(x+1)(x-1)` → `(x+1)*(x-1)`
/// - 变量 → `(` 不插入（函数调用，如 `sin(x)`）
///
/// 注意：标识符内的数字（如 `__cb_0`、`beta_1`）不视为数字结尾，不触发插入。
fn insert_implicit_multiplication(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut result = String::with_capacity(chars.len() + 16);

    // 跟踪前一个字符的状态
    let mut prev_was_number_end = false; // 前一个字符是数字且不属于标识符
    let mut prev_was_close_paren = false;
    let mut in_identifier = false; // 当前正在扫描标识符（以字母/下划线开头）

    for (i, &c) in chars.iter().enumerate() {
        if i > 0 {
            // 科学计数法排除：数字后跟 e/E 且后接数字或 +/-数字 → 不插入 *
            // 例如 1e308、1e-5、1E+10
            if prev_was_number_end && (c == 'e' || c == 'E') && is_scientific_notation(&chars, i) {
                // 不插入 *，但也不重置状态（e 后面跟数字时由标识符逻辑处理）
            } else if should_insert_implicit_mult(prev_was_number_end, prev_was_close_paren, c) {
                result.push('*');
                in_identifier = false;
            }
        }

        result.push(c);

        // 更新状态
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

/// 判断 `chars[pos]` 处的 `e`/`E` 是否为科学计数法的一部分。
/// 即 `e` 后面紧跟数字，或 `+`/`-` 后跟数字。
fn is_scientific_notation(chars: &[char], pos: usize) -> bool {
    // pos 指向 'e' 或 'E'，检查下一个字符
    if let Some(&next) = chars.get(pos + 1) {
        if next.is_ascii_digit() {
            return true;
        }
        if next == '+' || next == '-' {
            // 检查 + / - 后是否有数字
            if let Some(&next2) = chars.get(pos + 2) {
                return next2.is_ascii_digit();
            }
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

/// 将 mathexpr 的 `Expr` 转换为 CalNexus 的 [`AstNode`]，带深度检查。
///
/// 转换规则：
/// - `BinOp::Mod` → `FunctionCall("mod", ...)`（spec 要求取模为函数调用形式）
/// - 0-arity `FunctionCall("pi"/"e")` → `Variable("pi"/"e")`（spec 要求常量为变量形式）
/// - `UnaryMinus` → `UnaryOp(Neg, ...)`
/// - `CurrentValue` (`_`) → `Variable("_")`
///
/// 深度检查在转换时执行，超过 MAX_AST_DEPTH 立即返回错误，防止递归栈溢出。
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
            // 0-arity 的 pi/e 转换为 Variable（spec: 数学常量解析）
            if args.is_empty() && (name == "pi" || name == "e") {
                return Ok(AstNode::Variable(name.clone()));
            }
            let mut converted_args = Vec::with_capacity(args.len());
            for arg in args {
                converted_args.push(convert_with_depth(arg, depth + 1)?);
            }
            // 复数字面量：`complex(re, im)` → `Complex(re, im)`（design.md D3）
            // mathexpr 可能将 `-4` 解析为 `UnaryOp(Neg, Number(4))`，需规范化
            if name == "complex" && converted_args.len() == 2 {
                if let Some(complex) = try_complex_literal(&converted_args) {
                    return Ok(complex);
                }
            }
            Ok(AstNode::FunctionCall(name.clone(), converted_args))
        }
    }
}

/// 尝试将 `complex(re, im)` 的两个参数规范化为 `AstNode::Complex(re, im)`。
///
/// 规则（design.md D3）：
/// - re 必须为 `Number(n)`
/// - im 必须为 `Number(n)` 或 `UnaryOp(Neg, Number(n))`（mathexpr 解析 `-4` 为后者）
///
/// 返回 `Some(Complex(re, im))` 当两个参数都符合规则；否则返回 `None`，由调用方回退到普通 FunctionCall。
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

    // ===== Requirement 1: 基本四则运算解析 =====

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

    // ===== Requirement 2: 幂运算解析 =====

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

    // ===== Requirement 3: 阶乘运算解析 =====

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

    // ===== Requirement 4: 取模运算解析 =====

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

    // ===== Requirement 5: 函数调用解析 =====

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

    // ===== Requirement 6: 变量引用解析 =====

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

    // ===== Requirement 7: 括号嵌套解析 =====

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

    // ===== Requirement 8: 数学常量解析 =====

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

    // ===== Requirement 9: 无效表达式拒绝 =====

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

    // ===== Requirement 10: AST 深度限制 =====

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

    // ===== Requirement 11: 表达式长度限制 =====

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

    // ===== v0.5 复数字面量解析测试（任务 11.3） =====

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

    // ===== v0.5 矩阵字面量解析测试（任务 11.5） =====

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

    // ===== v0.5 列表字面量解析测试（任务 11.7） =====

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
        // 直接调用 parse_bracket_literal 传入非括号字符串
        // 覆盖 else 分支（lines 134-137）
        let result = parse_bracket_literal("abc");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.kind == ErrorKind::Parse && err.message.contains("expected bracket literal"));
    }

    // ===== 覆盖 preprocess_brackets 未匹配 '[' 错误 =====

    #[test]
    fn test_unmatched_open_bracket_rejected() {
        // `[[1,2]` — 只有一个 `]`，深度不为 0
        // 覆盖 lines 170-172
        let result = parse("[[1,2]");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.kind == ErrorKind::Parse && err.message.contains("unmatched '['"));
    }

    // ===== 覆盖 replace_placeholders Matrix/List 分支 =====

    #[test]
    fn test_replace_placeholders_in_matrix() {
        // 直接调用 replace_placeholders，覆盖 Matrix 分支（lines 265-269）
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
        // 直接调用 replace_placeholders，覆盖 List 分支（lines 272-275）
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
        // 覆盖 lines 288-291
        let result = parse_matrix_literal("[1,2]");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.kind == ErrorKind::Parse && err.message.contains("invalid matrix literal"));
    }

    #[test]
    fn test_parse_list_literal_invalid_input() {
        // 直接调用 parse_list_literal 传入非列表字符串
        // 覆盖 lines 325-328
        let result = parse_list_literal("1,2");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.kind == ErrorKind::Parse && err.message.contains("invalid list literal"));
    }

    // ===== 覆盖 split_by_pattern 括号深度分支 =====

    #[test]
    fn test_list_with_parens_in_elements() {
        // 列表元素含括号表达式：覆盖 split_by_pattern 的 paren_depth 分支（lines 374, 376）
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
        // `5 !` → factorial(5)
        // 覆盖 find_operand_start 中跳过尾部空格的循环（lines 444-445）
        let ast = parse("5 !").unwrap();
        assert_eq!(ast, call("factorial", vec![num(5.0)]));
    }

    #[test]
    fn test_factorial_no_operand_error() {
        // `!5` — `!` 前无操作数
        // 覆盖 find_operand_start 中 pos==0 错误路径（lines 447-450）
        let result = parse("!5");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.kind == ErrorKind::Parse && err.message.contains("no operand"));
    }

    #[test]
    fn test_factorial_on_nested_parens() {
        // `((1+2))!` → factorial(((1+2)))
        // 覆盖 find_operand_start 中嵌套 `)` 的 depth+=1 分支（line 460）
        let ast = parse("((1+2))!").unwrap();
        assert_eq!(
            ast,
            call("factorial", vec![binop(BinaryOp::Add, num(1.0), num(2.0))])
        );
    }

    #[test]
    fn test_factorial_unmatched_paren_error() {
        // `1+2)!` — `!` 前有未匹配的 `)`，向左扫描括号时 depth 始终 > 0
        // 覆盖 find_operand_start lines 465-468（unmatched parenthesis 错误）
        let result = parse("1+2)!");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.kind == ErrorKind::Parse && err.message.contains("unmatched parenthesis"));
    }

    // ===== 覆盖 convert_with_depth CurrentValue =====

    #[test]
    fn test_parse_current_value_underscore() {
        // `_` → mathexpr CurrentValue → AstNode::Variable("_")
        // 覆盖 line 509
        let ast = parse("_").unwrap();
        assert_eq!(ast, var("_"));
    }

    // ===== 覆盖 complex() 函数非 Number 参数路径 =====

    #[test]
    fn test_complex_call_with_variable_real() {
        // `complex(x, 4)` — re 不是 Number，覆盖 line 543 (_ => None for re_val)
        let ast = parse("complex(x, 4)").unwrap();
        assert_eq!(ast, call("complex", vec![var("x"), num(4.0)]));
    }

    #[test]
    fn test_complex_call_with_variable_imag() {
        // `complex(3, x)` — im 不是 Number/Neg，覆盖 line 554 (_ => None for im_val)
        let ast = parse("complex(3, x)").unwrap();
        assert_eq!(ast, call("complex", vec![num(3.0), var("x")]));
    }

    #[test]
    fn test_complex_call_with_neg_variable_imag() {
        // `complex(3, -x)` — im 是 Neg(Variable)，覆盖 line 551 (None for Neg non-Number)
        let ast = parse("complex(3, -x)").unwrap();
        assert_eq!(
            ast,
            call("complex", vec![num(3.0), unary(UnaryOp::Neg, var("x"))])
        );
    }

    // ===== 隐式乘法预处理测试（任务 1.4） =====

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

    // ===== TG6.4: Symbolic 函数解析测试 =====

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
        // 确保不是 Mul(Variable("diff"), Variable("x"))
        assert!(!matches!(ast, AstNode::BinaryOp(BinaryOp::Mul, _, _)));
    }

    // ===== 覆盖 is_scientific_notation 各分支 =====

    #[test]
    fn test_is_scientific_notation_e_followed_by_digit() {
        // e 后跟数字 → true（lines 559-561）
        let chars: Vec<char> = "1e5".chars().collect();
        assert!(is_scientific_notation(&chars, 1));
    }

    #[test]
    fn test_is_scientific_notation_e_followed_by_plus_digit() {
        // e+ 后跟数字 → true（lines 562-565）
        let chars: Vec<char> = "1e+5".chars().collect();
        assert!(is_scientific_notation(&chars, 1));
    }

    #[test]
    fn test_is_scientific_notation_e_followed_by_minus_digit() {
        // e- 后跟数字 → true（lines 562-565）
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
        // e 在字符串末尾 → false（lines 568-569）
        let chars: Vec<char> = "1e".chars().collect();
        assert!(!is_scientific_notation(&chars, 1));
    }

    #[test]
    fn test_is_scientific_notation_e_plus_at_end() {
        // e+ 在字符串末尾 → false（lines 566-569）
        let chars: Vec<char> = "1e+".chars().collect();
        assert!(!is_scientific_notation(&chars, 1));
    }

    #[test]
    fn test_is_scientific_notation_e_followed_by_non_digit_non_sign() {
        // e 后跟非数字非符号 → false（lines 568-569）
        let chars: Vec<char> = "1ex".chars().collect();
        assert!(!is_scientific_notation(&chars, 1));
    }

    // ===== proptest 属性测试（任务 2.5） =====

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

    // ===== T0.4.5: Span 精确性测试 =====

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
        // parse("   ") → trim 后为空，span = (0, 0)
        let err = parse("   ").unwrap_err();
        assert_eq!(err.kind, ErrorKind::Parse);
        assert_span(&err, Span::new(0, 0));
    }

    #[test]
    fn test_span_length_exceeded() {
        // 超长表达式，span 覆盖整个输入（字符偏移，design.md D1）
        let expr = "a".repeat(MAX_EXPR_LEN + 1);
        let err = parse(&expr).unwrap_err();
        assert_eq!(err.kind, ErrorKind::Parse);
        assert_span(&err, Span::new(0, expr.chars().count()));
    }

    #[test]
    fn test_span_mathexpr_parse_failure() {
        // `@` 不是合法字符，mathexpr 解析失败
        // 预处理后 after_implicit = "@"，span = (0, 1)
        let err = parse("@").unwrap_err();
        assert_eq!(err.kind, ErrorKind::Parse);
        assert_span(&err, Span::new(0, 1));
    }

    /// T005 Red: 隐式乘法预处理后错误位置仍指向原始输入
    ///
    /// `"2x@"` 经过隐式乘法预处理变成 `"2*x@"`（4 字符），mathexpr 解析失败。
    /// Span 应为 (0, 3)（原始 trimmed 输入长度）而非 (0, 4)（after_implicit 长度）。
    ///
    /// 用户看到的是原始输入 `"2x@"`，错误位置应帮助用户定位原始输入中的问题，
    /// 而不是预处理后字符串的位置（用户不知道预处理的存在）。
    #[test]
    fn test_span_mathexpr_failure_points_to_original_input() {
        let err = parse("2x@").unwrap_err();
        assert_eq!(err.kind, ErrorKind::Parse);
        // 期望 Span 指向原始输入 "2x@"（3 字符），而非 after_implicit "2*x@"（4 字符）
        assert_span(&err, Span::new(0, 3));
    }

    #[test]
    fn test_span_unmatched_open_bracket() {
        // `[[1,2]` — 未匹配 `[`，span 从 `[` 开始到扫描结束
        // 输入: [[1,2]  (6 字符)
        // chars = ['[','[','1',',','2',']']
        // i=0 遇 `[`，start=0，扫描到 i=6 时 depth=1≠0
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
        // `2 + + 3` → 去空格后 `2++3`，但 span 应基于原始输入
        // 原始输入中第一个 `+` 在位置 2，第二个 `+` 在位置 4
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
}
