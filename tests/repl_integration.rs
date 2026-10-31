// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! 通过 pty 驱动 `calnexus --repl` 的集成测试（30s 超时）。

// expectrl OsSession 依赖 pty：Windows ConPTY 环境下提示符稳定不可达（10/10 超时），
// 门控到 unix（CI ubuntu 与 WSL 开发侧保持全覆盖）。
#![cfg(unix)]

use expectrl::process::Healthcheck;
use expectrl::session::OsSession;
use expectrl::{Expect, Session};
use std::time::{Duration, Instant};

/// REPL 提示符 `calnexus> `
const PROMPT: &str = "calnexus> ";

/// 每测试 30s 超时
const TEST_TIMEOUT: Duration = Duration::from_secs(30);

/// pty 终端类型：rustyline 对 TERM=dumb/cons25/emacs 禁用行编辑（无 Tab 补全、无历史召回），
/// 而伪终端本身具备完整终端能力，故显式声明受支持的 TERM，使测试不受宿主 TERM 影响。
const TERM: &str = "xterm-256color";

/// 启动 calnexus --repl 会话
fn spawn_repl() -> OsSession {
    let bin = assert_cmd::cargo::cargo_bin("calnexus");
    let mut cmd = std::process::Command::new(bin);
    // 语言钉定 En：REPL 输出文案（repl.bye/错误消息）随宿主语区本地化，
    // 断言串为英文——中文语区宿主上会整批假失败（同 TERM 的钉定理由）
    cmd.arg("--repl")
        .env("TERM", TERM)
        .env("CALNEXUS_LANG", "en");
    Session::spawn(cmd).expect("failed to spawn calnexus --repl")
}

/// 等待指定内容出现，超时则 panic
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

/// 基本求值 `2+3` → `5`
#[test]
fn it_cli_017_basic_eval() {
    let mut session = spawn_repl();
    expect(&mut session, PROMPT, "IT-CLI-017 init");
    send_line(&mut session, "2+3");
    expect(&mut session, "5", "IT-CLI-017 result");
    let _ = session.send_line(":quit");
}

/// `:let` 绑定变量后可在表达式中引用
#[test]
fn it_cli_018_variable_binding() {
    let mut session = spawn_repl();
    expect(&mut session, PROMPT, "IT-CLI-018 init");
    send_line(&mut session, ":let x = 3.14");
    expect(&mut session, PROMPT, "IT-CLI-018 after let");
    send_line(&mut session, "sin(x)");
    // 3.14 接近 π，故 sin(x) 结果约 0.00159
    expect(&mut session, "0.001", "IT-CLI-018 sin(3.14)");
    let _ = session.send_line(":quit");
}

/// `:vars` 列出已绑定变量
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

/// 上箭头召回历史输入
#[test]
fn it_cli_020_history_recall() {
    let mut session = spawn_repl();
    expect(&mut session, PROMPT, "IT-CLI-020 init");
    send_line(&mut session, "2+3");
    expect(&mut session, "5", "IT-CLI-020 first eval");
    expect(&mut session, PROMPT, "IT-CLI-020 second prompt");
    // 上箭头键序列：ESC [ A
    session.send(b"\x1b[A").expect("send up arrow");
    expect(&mut session, "2+3", "IT-CLI-020 history recall");
    let _ = session.send_line("");
    let _ = session.send_line(":quit");
}

/// `:quit` 退出码 0
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

/// 输入 `prefix` 后按 Tab，断言补全候选（统一函数目录派生）已写入输入行
fn assert_tab_completion(prefix: &str, expected: &str, test_name: &str) {
    let mut session = spawn_repl();
    expect(&mut session, PROMPT, test_name);
    session.send(prefix.as_bytes()).expect("send prefix");
    session.send(b"\t").expect("send tab");
    expect(&mut session, expected, test_name);
    let _ = session.send_line("");
    let _ = session.send_line(":quit");
}

/// Tab 补全 `si` → `sin`（候选取自统一函数目录）
#[test]
fn it_cli_022_tab_completion() {
    assert_tab_completion("si", "sin", "IT-CLI-022 tab completion");
}

/// 启用 `unit` 后门控函数出现在 Tab 补全（`conv` → `convert`）
#[cfg(feature = "unit")]
#[test]
fn it_cli_022_tab_completion_unit_gated() {
    assert_tab_completion("conv", "convert", "IT-CLI-022 unit gated completion");
}

/// 启用 `time` 后门控函数出现在 Tab 补全（`tod` → `today`）
#[cfg(feature = "time")]
#[test]
fn it_cli_022_tab_completion_time_gated() {
    assert_tab_completion("tod", "today", "IT-CLI-022 time gated completion");
}

/// 启用 `fx` 后门控函数出现在 Tab 补全（`fx_` → `fx_rate`）
#[cfg(feature = "fx")]
#[test]
fn it_cli_022_tab_completion_fx_gated() {
    assert_tab_completion("fx_", "fx_rate", "IT-CLI-022 fx gated completion");
}

/// 无效输入报错后会话仍可继续求值
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

// 验证 REPL 测试基础设施完整
#[test]
fn repl_infrastructure_present() {
    let _session = spawn_repl();
}
