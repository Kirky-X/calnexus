// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! REPL integration tests using `expectrl` (TEST.md §9, IT-CLI-017 ~ IT-CLI-023).
//!
//! 通过 pty 驱动 `calnexus --repl`，验证交互式行编辑、变量绑定、历史等。
//! 每个 30s 超时（CI 防挂起）。易 flaky 的测试标记 `#[ignore]`。

use expectrl::process::Healthcheck;
use expectrl::session::OsSession;
use expectrl::{spawn, Expect};
use std::time::{Duration, Instant};

/// REPL 提示符（src/repl.rs: `calnexus> `）
const PROMPT: &str = "calnexus> ";

/// 每测试 30s 超时
const TEST_TIMEOUT: Duration = Duration::from_secs(30);

/// 启动 calnexus --repl 会话
fn spawn_repl() -> OsSession {
    let bin = assert_cmd::cargo::cargo_bin("calnexus");
    let cmd_str = format!("{} --repl", bin.to_string_lossy());
    spawn(&cmd_str).expect("failed to spawn calnexus --repl")
}

/// 等待 REPL 出现提示符或指定内容，超时则 panic
fn expect(session: &mut OsSession, pattern: &str, test_name: &str) {
    let start = Instant::now();
    loop {
        if start.elapsed() > TEST_TIMEOUT {
            panic!("{}: timeout waiting for {:?}", test_name, pattern);
        }
        match session.expect(pattern) {
            Ok(_) => return,
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("EOF") || msg.contains("closed") {
                    panic!(
                        "{}: session EOF waiting for {:?}: {}",
                        test_name, pattern, msg
                    );
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
}

/// 向 REPL 发送一行（带换行）
fn send_line(session: &mut OsSession, line: &str) {
    session.send_line(line).expect("failed to send line");
}

/// IT-CLI-017: 基本求值 `2+3` → `5`
#[test]
fn it_cli_017_basic_eval() {
    let mut session = spawn_repl();
    expect(&mut session, PROMPT, "IT-CLI-017 init");
    send_line(&mut session, "2+3");
    expect(&mut session, "5", "IT-CLI-017 result");
    let _ = session.send_line(":quit");
}

/// IT-CLI-018: `:let x = 3.14` 后 `sin(x)` ≈ 0.00159...
#[test]
fn it_cli_018_variable_binding() {
    let mut session = spawn_repl();
    expect(&mut session, PROMPT, "IT-CLI-018 init");
    send_line(&mut session, ":let x = 3.14");
    expect(&mut session, PROMPT, "IT-CLI-018 after let");
    send_line(&mut session, "sin(x)");
    // sin(3.14) ≈ 0.0015926529164069226（3.14 接近 π）
    expect(&mut session, "0.001", "IT-CLI-018 sin(3.14)");
    let _ = session.send_line(":quit");
}

/// IT-CLI-019: `:vars` 列出已绑定变量 `x = 3.14`
#[test]
fn it_cli_019_view_vars() {
    let mut session = spawn_repl();
    expect(&mut session, PROMPT, "IT-CLI-019 init");
    send_line(&mut session, ":let x = 3.14");
    expect(&mut session, PROMPT, "IT-CLI-019 after let");
    send_line(&mut session, ":vars");
    expect(&mut session, "x", "IT-CLI-019 vars contains x");
    expect(&mut session, "3.14", "IT-CLI-019 vars contains value");
    let _ = session.send_line(":quit");
}

/// IT-CLI-020: 上箭头召回历史输入。
/// 标记 `#[ignore]` 因 pty 上箭头键码易在 CI flaky。
#[test]
#[ignore = "history recall via pty is flaky on CI; run with --ignored"]
fn it_cli_020_history_recall() {
    let mut session = spawn_repl();
    expect(&mut session, PROMPT, "IT-CLI-020 init");
    send_line(&mut session, "2+3");
    expect(&mut session, "5", "IT-CLI-020 first eval");
    expect(&mut session, PROMPT, "IT-CLI-020 second prompt");
    // 上箭头：ESC [ A（Unix 终端序列）
    session.send(b"\x1b[A").expect("send up arrow");
    expect(&mut session, "2+3", "IT-CLI-020 history recall");
    let _ = session.send_line("");
    let _ = session.send_line(":quit");
}

/// IT-CLI-021: `:quit` 退出码 0
#[test]
fn it_cli_021_quit_exit_zero() {
    let mut session = spawn_repl();
    expect(&mut session, PROMPT, "IT-CLI-021 init");
    send_line(&mut session, ":quit");
    expect(&mut session, "bye", "IT-CLI-021 bye message");
    let start = Instant::now();
    loop {
        if start.elapsed() > TEST_TIMEOUT {
            panic!("IT-CLI-021: timeout waiting for process exit");
        }
        match session.is_alive() {
            Ok(false) => break,
            Ok(true) => {}
            Err(_) => break,
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// IT-CLI-022: Tab 补全 `si` → `sin(`。
/// 标记 `#[ignore]` 因 rustyline Tab 补全在 pty 下易 flaky。
#[test]
#[ignore = "tab completion via pty is flaky on CI; run with --ignored"]
fn it_cli_022_tab_completion() {
    let mut session = spawn_repl();
    expect(&mut session, PROMPT, "IT-CLI-022 init");
    session.send(b"si").expect("send si");
    session.send(b"\t").expect("send tab");
    expect(&mut session, "sin(", "IT-CLI-022 tab completion");
    let _ = session.send_line("");
    let _ = session.send_line(":quit");
}

/// IT-CLI-023: 无效 `2+` 显示错误但无崩溃，后续 `2+3` 正常求值
#[test]
fn it_cli_023_error_recovery() {
    let mut session = spawn_repl();
    expect(&mut session, PROMPT, "IT-CLI-023 init");
    send_line(&mut session, "2+");
    expect(&mut session, "error", "IT-CLI-023 error shown");
    expect(&mut session, PROMPT, "IT-CLI-023 prompt after error");
    send_line(&mut session, "2+3");
    expect(&mut session, "5", "IT-CLI-023 recovery eval");
    let _ = session.send_line(":quit");
}

// 验证 REPL 测试基础设施完整（expectrl 已在 dev-dependencies）
#[test]
fn repl_infrastructure_present() {
    let _session = spawn_repl();
}
