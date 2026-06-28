// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! REPL 模式：基于 rustyline 的交互式读-求值-打印循环（TG4）。
//!
//! 设计依据：
//! - design.md D3（ReplSession 持有 DomainRouter + CacheManager + EvalContext）
//! - v1.0 repl-mode spec
//!
//! 命令（以 `:` 开头）：
//! - `:let NAME = VALUE` — 绑定变量
//! - `:vars` — 列出所有已绑定变量
//! - `:quit` / `:q` — 退出 REPL
//! - `:help` — 显示帮助
//! - `:clear` — 清屏

use crate::cli::{build_default_router, evaluate, format_result};
use crate::core::types::{EvalContext, EvalResult};
use rustyline::completion::Completer;
use rustyline::{Editor, Helper, Highlighter, Hinter, Result as RlResult, Validator};

/// REPL 命令补全候选：函数名 + REPL 命令。
const REPL_COMMANDS: &[&str] = &[":let", ":vars", ":quit", ":q", ":help", ":clear"];

/// 已知函数名（用于 Tab 补全）。
const KNOWN_FUNCTIONS: &[&str] = &[
    "sin",
    "cos",
    "tan",
    "asin",
    "acos",
    "atan",
    "ln",
    "log",
    "exp",
    "sinh",
    "cosh",
    "tanh",
    "gamma",
    "erf",
    "abs",
    "factorial",
    "mod",
    "gcd",
    "lcm",
    "is_prime",
    "prime_sieve",
    "mod_inverse",
    "mod_pow",
    "euler_phi",
    "P",
    "C",
    "catalan",
    "stirling",
    "dot",
    "cross",
    "norm",
    "angle",
    "normalize",
    "scalar_triple",
    "poly_add",
    "poly_sub",
    "poly_mul",
    "poly_div",
    "poly_eval",
    "poly_diff",
    "poly_integrate",
    "roots",
    "factor",
    "diff",
    "integrate",
    "simplify",
    "limit",
    "taylor",
    "mean",
    "median",
    "variance",
    "stddev",
    "sum",
    "min",
    "max",
    "det",
    "transpose",
    "inverse",
    "trace",
    "complex",
    "re",
    "im",
    "conj",
    "magnitude",
    "phase",
    "precision",
];

/// REPL 会话：持有路由器、缓存、变量上下文（TG4.1）。
pub struct ReplSession {
    ctx: EvalContext,
    cache: crate::CacheManager,
}

impl ReplSession {
    /// 创建 REPL 会话，初始化空缓存与给定上下文。
    pub fn new(ctx: EvalContext) -> Self {
        Self {
            ctx,
            cache: crate::CacheManager::new(),
        }
    }

    /// 启动 REPL 主循环（TG4.4）。
    ///
    /// 读取输入 → 若 `:` 开头解析为 REPL 命令 → 否则 parse → evaluate → 打印结果。
    /// 错误打印到 stderr 但不退出。返回退出码 0。
    pub fn run(mut self) -> i32 {
        // 确认路由器可构建（提前发现注册错误）
        let _router = build_default_router();

        let mut rl: Editor<ReplHelper, _> = Editor::new().expect("failed to init rustyline Editor");
        rl.set_helper(Some(ReplHelper));

        println!("CalNexus REPL — type :help for commands, :quit to exit");
        loop {
            let readline = rl.readline("calnexus> ");
            match readline {
                Ok(line) => {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    let _ = rl.add_history_entry(trimmed);
                    if trimmed.starts_with(':') {
                        match self.handle_command(trimmed) {
                            CommandResult::Continue => {}
                            CommandResult::Quit => {
                                println!("bye");
                                return 0;
                            }
                        }
                    } else {
                        self.evaluate_line(trimmed);
                    }
                }
                Err(rustyline::error::ReadlineError::Interrupted) => {
                    // Ctrl+C：继续
                    continue;
                }
                Err(rustyline::error::ReadlineError::Eof) => {
                    // Ctrl+D：退出
                    println!("bye");
                    return 0;
                }
                Err(e) => {
                    eprintln!("REPL error: {}", e);
                    return 1;
                }
            }
        }
    }

    /// 处理 REPL 命令（TG4.2）。
    fn handle_command(&mut self, line: &str) -> CommandResult {
        let parts: Vec<&str> = line.splitn(2, char::is_whitespace).collect();
        let cmd = parts[0];
        match cmd {
            ":quit" | ":q" => CommandResult::Quit,
            ":help" => {
                self.print_help();
                CommandResult::Continue
            }
            ":vars" => {
                self.print_vars();
                CommandResult::Continue
            }
            ":clear" => {
                // 清屏：打印 ANSI 转义序列
                print!("\x1b[2J\x1b[H");
                CommandResult::Continue
            }
            ":let" => {
                if parts.len() < 2 {
                    eprintln!("usage: :let NAME = VALUE");
                    return CommandResult::Continue;
                }
                self.handle_let(parts[1]);
                CommandResult::Continue
            }
            _ => {
                eprintln!(
                    "unknown command: {} (type :help for available commands)",
                    cmd
                );
                CommandResult::Continue
            }
        }
    }

    /// 处理 `:let NAME = VALUE` 变量绑定（TG4.2）。
    fn handle_let(&mut self, args: &str) {
        // 解析 NAME = VALUE
        let eq_parts: Vec<&str> = args.splitn(2, '=').collect();
        if eq_parts.len() != 2 {
            eprintln!("usage: :let NAME = VALUE (missing '=')");
            return;
        }
        let name = eq_parts[0].trim();
        let value_str = eq_parts[1].trim();
        if name.is_empty() {
            eprintln!("error: variable name is empty");
            return;
        }
        match value_str.parse::<f64>() {
            Ok(v) => {
                self.ctx = self.ctx.clone().with_var(name, v);
                println!("{} = {}", name, v);
            }
            Err(e) => {
                // 尝试作为表达式求值
                match evaluate(value_str, &self.ctx, None, &self.cache) {
                    Ok((EvalResult::Scalar(v), _, _, _)) => {
                        self.ctx = self.ctx.clone().with_var(name, v);
                        println!("{} = {}", name, v);
                    }
                    Ok((result, _, _, _)) => {
                        eprintln!(
                            "error: :let value must be a scalar, got {}",
                            format_result(&result, None)
                        );
                    }
                    Err(e2) => {
                        eprintln!("error: invalid value '{}': {} / {}", value_str, e, e2);
                    }
                }
            }
        }
    }

    /// 打印所有已绑定变量。
    fn print_vars(&self) {
        if self.ctx.vars.is_empty() {
            println!("(no variables bound)");
            return;
        }
        let mut names: Vec<&String> = self.ctx.vars.keys().collect();
        names.sort();
        for name in names {
            if let Some(v) = self.ctx.vars.get(name) {
                println!("{} = {}", name, v);
            }
        }
    }

    /// 打印帮助信息。
    fn print_help(&self) {
        println!("CalNexus REPL commands:");
        println!("  :let NAME = VALUE   Bind a variable (VALUE may be a number or expression)");
        println!("  :vars               List all bound variables");
        println!("  :clear              Clear the screen");
        println!("  :help               Show this help message");
        println!("  :quit / :q          Exit the REPL");
        println!();
        println!("Otherwise, type a math expression and press Enter to evaluate.");
        println!("Examples: 2+3*4, sin(pi/2), diff(x^2, x), gcd(12,18)");
    }

    /// 求值一行表达式并打印结果（TG4.4）。
    fn evaluate_line(&mut self, expr: &str) {
        match evaluate(expr, &self.ctx, self.ctx.precision, &self.cache) {
            Ok((result, domain, cache_hit, fmt_prec)) => {
                let output = format_result(&result, fmt_prec);
                println!(
                    "= {}  [{}{}]",
                    output,
                    domain,
                    if cache_hit { " (cached)" } else { "" }
                );
            }
            Err(e) => {
                eprintln!("error: {}", e);
            }
        }
    }
}

/// 命令处理结果。
enum CommandResult {
    Continue,
    Quit,
}

/// rustyline Helper：提供 Tab 补全（TG4.3）。
///
/// 手动实现 `Completer`（自定义补全逻辑），其余三个 trait 通过 derive 生成默认空实现。
/// `Helper` 是 marker trait（要求 `Completer + Hinter + Highlighter + Validator`），derive 自动生成。
#[derive(Helper, Hinter, Validator, Highlighter)]
struct ReplHelper;

impl Completer for ReplHelper {
    type Candidate = String;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &rustyline::Context<'_>,
    ) -> RlResult<(usize, Vec<String>)> {
        Ok(complete_candidates(line, pos))
    }
}

/// 纯函数：根据当前输入行和光标位置计算补全候选（TG4.3）。
///
/// 抽离自 `Completer::complete` 以便单元测试无需构造 rustyline Context。
fn complete_candidates(line: &str, pos: usize) -> (usize, Vec<String>) {
    // 仅在行尾补全
    if pos != line.len() {
        return (0, Vec::new());
    }
    let mut candidates: Vec<String> = Vec::new();

    // 补全 REPL 命令（以 : 开头）
    if line.starts_with(':') {
        for cmd in REPL_COMMANDS {
            if cmd.starts_with(line) {
                candidates.push(cmd.to_string());
            }
        }
        return (0, candidates);
    }

    // 补全函数名：提取当前正在输入的标识符前缀
    let prefix: String = line
        .chars()
        .rev()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    if prefix.is_empty() {
        return (0, Vec::new());
    }
    let start = pos - prefix.len();
    for func in KNOWN_FUNCTIONS {
        if func.starts_with(&prefix) && *func != prefix {
            candidates.push(func.to_string());
        }
    }
    (start, candidates)
}

// ============================ 单元测试 (TG4.6) ============================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repl_session_new() {
        let session = ReplSession::new(EvalContext::new());
        assert!(session.ctx.vars.is_empty());
    }

    #[test]
    fn test_repl_session_with_vars() {
        let ctx = EvalContext::new().with_var("x", 3.14);
        let session = ReplSession::new(ctx);
        assert_eq!(session.ctx.get_var("x"), Some(3.14));
    }

    #[test]
    fn test_handle_let_numeric() {
        let mut session = ReplSession::new(EvalContext::new());
        session.handle_let("x = 42");
        assert_eq!(session.ctx.get_var("x"), Some(42.0));
    }

    #[test]
    fn test_handle_let_expression() {
        let mut session = ReplSession::new(EvalContext::new());
        // :let y = 2+3*4 → y = 14
        session.handle_let("y = 2+3*4");
        assert_eq!(session.ctx.get_var("y"), Some(14.0));
    }

    #[test]
    fn test_handle_let_missing_equals() {
        let mut session = ReplSession::new(EvalContext::new());
        session.handle_let("x 42");
        // 应不绑定任何变量
        assert!(session.ctx.vars.is_empty());
    }

    #[test]
    fn test_handle_let_empty_name() {
        let mut session = ReplSession::new(EvalContext::new());
        session.handle_let(" = 42");
        assert!(session.ctx.vars.is_empty());
    }

    #[test]
    fn test_command_quit() {
        let mut session = ReplSession::new(EvalContext::new());
        let result = session.handle_command(":quit");
        assert!(matches!(result, CommandResult::Quit));
    }

    #[test]
    fn test_command_q_alias() {
        let mut session = ReplSession::new(EvalContext::new());
        let result = session.handle_command(":q");
        assert!(matches!(result, CommandResult::Quit));
    }

    #[test]
    fn test_command_help() {
        let mut session = ReplSession::new(EvalContext::new());
        let result = session.handle_command(":help");
        assert!(matches!(result, CommandResult::Continue));
    }

    #[test]
    fn test_command_vars_empty() {
        let mut session = ReplSession::new(EvalContext::new());
        let result = session.handle_command(":vars");
        assert!(matches!(result, CommandResult::Continue));
    }

    #[test]
    fn test_command_vars_with_bindings() {
        let ctx = EvalContext::new().with_var("x", 1.0).with_var("y", 2.0);
        let mut session = ReplSession::new(ctx);
        let result = session.handle_command(":vars");
        assert!(matches!(result, CommandResult::Continue));
    }

    #[test]
    fn test_command_let_then_vars() {
        let mut session = ReplSession::new(EvalContext::new());
        session.handle_command(":let a = 10");
        session.handle_command(":let b = 20");
        assert_eq!(session.ctx.get_var("a"), Some(10.0));
        assert_eq!(session.ctx.get_var("b"), Some(20.0));
    }

    #[test]
    fn test_command_unknown() {
        let mut session = ReplSession::new(EvalContext::new());
        let result = session.handle_command(":unknown");
        assert!(matches!(result, CommandResult::Continue));
    }

    #[test]
    fn test_command_let_no_args() {
        let mut session = ReplSession::new(EvalContext::new());
        let result = session.handle_command(":let");
        assert!(matches!(result, CommandResult::Continue));
        assert!(session.ctx.vars.is_empty());
    }

    #[test]
    fn test_evaluate_line_arithmetic() {
        let mut session = ReplSession::new(EvalContext::new());
        // 不应 panic，错误打印到 stderr
        session.evaluate_line("2+3*4");
    }

    #[test]
    fn test_evaluate_line_error_recovery() {
        let mut session = ReplSession::new(EvalContext::new());
        // 语法错误：不应 panic
        session.evaluate_line("2++3");
        // 后续仍可正常求值
        session.evaluate_line("1+1");
    }

    #[test]
    fn test_evaluate_line_with_var() {
        let ctx = EvalContext::new().with_var("x", 5.0);
        let mut session = ReplSession::new(ctx);
        session.evaluate_line("x*2");
    }

    #[test]
    fn test_evaluate_line_symbolic() {
        let mut session = ReplSession::new(EvalContext::new());
        session.evaluate_line("diff(x^2, x)");
    }

    #[test]
    fn test_completer_repl_command() {
        let (start, candidates) = complete_candidates(":l", 2);
        assert_eq!(start, 0);
        assert!(candidates.contains(&":let".to_string()));
    }

    #[test]
    fn test_completer_function_name() {
        let (start, candidates) = complete_candidates("si", 2);
        assert_eq!(start, 0);
        assert!(candidates.contains(&"sin".to_string()));
    }

    #[test]
    fn test_completer_no_match() {
        let (_start, candidates) = complete_candidates("zzz", 3);
        assert!(candidates.is_empty());
    }

    #[test]
    fn test_completer_exact_match_excluded() {
        // 完全匹配时不补全（func == prefix 排除）
        let (_start, candidates) = complete_candidates("sin", 3);
        assert!(!candidates.contains(&"sin".to_string()));
    }

    #[test]
    fn test_command_clear() {
        // :clear 命令：打印 ANSI 清屏序列，返回 Continue
        // 覆盖 lines 173-176（:clear 分支）
        let mut session = ReplSession::new(EvalContext::new());
        let result = session.handle_command(":clear");
        assert!(matches!(result, CommandResult::Continue));
    }

    #[test]
    fn test_handle_let_non_scalar_result() {
        // :let x = 3+4i → evaluate 返回 Complex（非 Scalar）
        // 覆盖 lines 222-226（Ok 分支但结果非 Scalar）
        let mut session = ReplSession::new(EvalContext::new());
        session.handle_let("x = 3+4i");
        // 非标量结果不应绑定变量
        assert!(session.ctx.get_var("x").is_none());
    }

    #[test]
    fn test_handle_let_eval_error() {
        // :let x = 2++3 → parse 失败 → evaluate 返回 Err
        // 覆盖 lines 228-230（evaluate 错误分支）
        let mut session = ReplSession::new(EvalContext::new());
        session.handle_let("x = 2++3");
        // 求值失败不应绑定变量
        assert!(session.ctx.get_var("x").is_none());
    }

    #[test]
    fn test_complete_candidates_pos_not_at_end() {
        // 光标不在行尾时直接返回空候选
        // 覆盖 line 315（pos != line.len()）
        let (start, candidates) = complete_candidates("sin", 1);
        assert_eq!(start, 0);
        assert!(candidates.is_empty());
    }

    #[test]
    fn test_complete_candidates_empty_prefix() {
        // 行尾为非标识符字符时，提取的前缀为空 → 返回空候选
        // 覆盖 line 339（prefix.is_empty()）
        let (start, candidates) = complete_candidates("(", 1);
        assert_eq!(start, 0);
        assert!(candidates.is_empty());
    }
}
