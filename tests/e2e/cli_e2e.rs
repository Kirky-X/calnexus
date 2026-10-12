// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! S9 CLI E2E：批处理退出码契约、JSON 逐行错误结构、行数/行长上限、env 注入。
//! 全部经子进程验证，零环境污染。

#![cfg(feature = "cli")]

use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use tempfile::NamedTempFile;

fn cmd() -> Command {
    Command::cargo_bin("calnexus").expect("binary builds")
}

/// 写临时批处理文件并返回路径。
fn batch_file(contents: &str) -> NamedTempFile {
    let mut f = NamedTempFile::new().unwrap();
    use std::io::Write;
    f.write_all(contents.as_bytes()).unwrap();
    f
}

// ---------------------------------------------------------------------------
// 批处理退出码契约：0 全成 / 1 部分失败 / 2 系统错误
// ---------------------------------------------------------------------------

#[test]
fn cli_batch_all_success_exit_0() {
    let f = batch_file("1+1\n2*3\n# comment\n\nsin(0)\n");
    cmd()
        .arg("--batch")
        .arg(f.path())
        .assert()
        .code(0)
        .stdout(predicates::str::contains("2").and(predicates::str::contains("6")));
}

#[test]
fn cli_batch_partial_failure_exit_1() {
    // 部分失败退出码 1
    let f = batch_file("2+2\n1/0\n(2+3\n4*5\n");
    cmd()
        .arg("--batch")
        .arg(f.path())
        .assert()
        .code(1)
        // 成功行照常输出
        .stdout(predicates::str::contains("4"))
        .stdout(predicates::str::contains("20"));
}

#[test]
fn cli_batch_json_mixed_success_and_error_entries() {
    // JSON 逐行结构：成功行 result/domain/cache，失败行 error
    let f = batch_file("2+2\n1/0\n");
    let output = cmd()
        .arg("--json")
        .arg("--batch")
        .arg(f.path())
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).unwrap();
    let arr: Vec<serde_json::Value> = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("batch --json should emit JSON array: {e}; got {stdout}"));
    assert_eq!(arr.len(), 2);
    let ok = &arr[0];
    assert_eq!(ok["line"], 1);
    // result 为格式化字符串
    assert_eq!(ok["result"], "4");
    assert_eq!(ok["domain"], "arithmetic");
    assert_eq!(ok["cache"], "miss");
    let err = &arr[1];
    assert_eq!(err["line"], 2);
    let err_text = err["error"].as_str().unwrap_or_default();
    assert!(!err_text.is_empty(), "error entry text: {err}");
    assert!(
        err_text.contains("division by zero") || err_text.contains("DivisionByZero"),
        "got {err_text}"
    );
}

#[test]
fn cli_batch_over_1000_entries_exit_2() {
    let content: String = (0..1001).map(|i| format!("{i}+1\n")).collect();
    let f = batch_file(&content);
    cmd().arg("--batch").arg(f.path()).assert().code(2).stderr(
        predicates::str::contains("1000").or(predicates::str::contains("批")
            .or(predicates::str::contains("entries").or(predicates::str::contains("limit")))),
    );
}

#[test]
fn cli_batch_line_too_long_exit_2() {
    let long_line = "1+".repeat(2048) + "1"; // 4097 字符
    let f = batch_file(&format!("1+1\n{long_line}\n"));
    cmd()
        .arg("--batch")
        .arg(f.path())
        .assert()
        .code(2)
        .stderr(predicates::str::contains("4096"));
}

#[test]
fn cli_batch_var_binding_applies() {
    // --var 变量环境对批处理所有行生效
    let f = batch_file("x*2\nx+1\n");
    cmd()
        .arg("--var")
        .arg("x=10")
        .arg("--batch")
        .arg(f.path())
        .assert()
        .code(0)
        .stdout(predicates::str::contains("20"))
        .stdout(predicates::str::contains("11"));
}

// ---------------------------------------------------------------------------
// env 注入（子进程隔离）
// ---------------------------------------------------------------------------

#[test]
fn cli_env_cache_size_accepted() {
    // 合法缓存预算被接受，求值不受影响
    cmd()
        .env("CALNEXUS_CACHE_SIZE", "1")
        .arg("--json")
        .arg("2+3")
        .assert()
        .code(0)
        .stdout(predicates::str::contains("\"result\":5"));
}

#[test]
fn cli_env_bind_addr_only_affects_serve_mode() {
    // CALNEXUS_BIND_ADDR 在非 serve 模式下无害
    cmd()
        .env("CALNEXUS_BIND_ADDR", "127.0.0.1:1")
        .arg("--json")
        .arg("2+3")
        .assert()
        .code(0);
}

// ---------------------------------------------------------------------------
// Usage 契约：非法参数 → 退出码 2
// ---------------------------------------------------------------------------

#[test]
fn cli_precision_flag_contracts() {
    // 非数字 precision → clap 层 Usage 退出码 2
    cmd()
        .arg("--precision")
        .arg("abc")
        .arg("1/3")
        .assert()
        .code(2);
    // 超过 MAX_PRECISION(10000) → 退出码 1
    cmd()
        .arg("--precision")
        .arg("10001")
        .arg("1/3")
        .assert()
        .code(1)
        .stderr(predicates::str::contains("10000").or(predicates::str::contains("10001")));
    // precision=0 视为默认
    cmd()
        .arg("--precision")
        .arg("0")
        .arg("1/3")
        .assert()
        .code(0);
}

#[test]
fn cli_usage_exit_2_for_unrecognized_flag() {
    cmd().arg("--no-such-flag").assert().code(2);
}

#[test]
fn cli_usage_exit_2_for_invalid_var_format() {
    cmd()
        .arg("--var")
        .arg("not-a-number")
        .arg("x")
        .assert()
        .code(2);
}

// ---------------------------------------------------------------------------
// 管道下游提前关闭（`calnexus --list-functions | head -4`）：静默终止，不误报内部 bug
// ---------------------------------------------------------------------------

/// stdout 读端在子进程启动前关闭（等价 `| head -4` 先退出）：子进程首次写必触发
/// EPIPE——unix 上应被 SIGPIPE 静默终止（shell 惯例 141），任何平台 stderr 都不得
/// 出现 "internal error / report this bug" 的 panic 误报。
#[test]
fn cli_stdout_pipe_closed_downstream_dies_quietly() {
    use std::process::Stdio;
    // assert_cmd 不便直控管道 fd，直接用 std::process::Command（同 cli_integration 先例）
    let (dead_reader, writer) = std::io::pipe().expect("create pipe");
    drop(dead_reader);
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_calnexus"))
        .arg("--list-functions")
        .stdout(writer)
        .stderr(Stdio::piped())
        .output()
        .expect("spawn with closed stdout pipe");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.trim().is_empty(),
        "broken pipe 必须静默终止，不得输出 panic 误报，stderr: {stderr}"
    );
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        assert_eq!(
            output.status.signal(),
            Some(13),
            "应被 SIGPIPE 静默终止，status: {:?}",
            output.status
        );
    }
    #[cfg(not(unix))]
    assert_eq!(
        output.status.code(),
        Some(141),
        "非 unix 兜底应静默退出 141，status: {:?}",
        output.status
    );
}
