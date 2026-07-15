// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! Snapshot tests using `insta` (TEST.md §5, SNAP-001 ~ SNAP-008).
//!
//! Run with `cargo insta test --review` to lock snapshots.
//! Snapshots live in `tests/snapshots/`.

mod common;

use common::calnexus_cli;
use std::path::Path;

/// 构建 calnexus CLI Command，并设置 INSTA_UPDATE=no（snapshot 测试专用）。
fn calnexus() -> assert_cmd::Command {
    let mut cmd = calnexus_cli();
    cmd.env("INSTA_UPDATE", "no");
    cmd
}

/// SNAP-001: symbolic `diff(x^2,x)` → `2*x`（文本输出）
#[test]
fn snap_001_symbolic_diff_text() {
    let output = calnexus()
        .arg("diff(x^2,x)")
        .output()
        .expect("failed to execute");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    insta::assert_snapshot!("snap_001_symbolic_diff_text", stdout);
}

/// SNAP-002: `--latex "diff(x^2,x)"` → `\frac{d}{dx}\left(x^{2}\right) = 2x`
#[test]
fn snap_002_latex_diff() {
    let output = calnexus()
        .args(["--latex", "diff(x^2,x)"])
        .output()
        .expect("failed to execute");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    insta::assert_snapshot!("snap_002_latex_diff", stdout);
}

/// SNAP-003: `--canonical "3+2"` → `(+ 2 3)` (PRD §3.2.4)
#[test]
fn snap_003_canonical_3plus2() {
    let output = calnexus()
        .args(["--canonical", "3+2"])
        .output()
        .expect("failed to execute");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    insta::assert_snapshot!("snap_003_canonical_3plus2", stdout);
}

/// SNAP-004: `"(2+3"` → parse error snapshot
#[test]
fn snap_004_parse_error_unbalanced() {
    let output = calnexus().arg("(2+3").output().expect("failed to execute");
    assert!(!output.status.success(), "expected non-zero exit");
    let stderr = String::from_utf8_lossy(&output.stderr);
    insta::assert_snapshot!("snap_004_parse_error_unbalanced", stderr);
}

/// SNAP-005: `--json "2+3"` → JSON snapshot
#[test]
fn snap_005_json_2plus3() {
    let output = calnexus()
        .args(["--json", "2+3"])
        .output()
        .expect("failed to execute");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    insta::assert_snapshot!("snap_005_json_2plus3", stdout);
}

/// SNAP-006: `--steps "(2+9)*7-6"` → steps snapshot
#[test]
fn snap_006_steps_complex() {
    let output = calnexus()
        .args(["--steps", "(2+9)*7-6"])
        .output()
        .expect("failed to execute");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    insta::assert_snapshot!("snap_006_steps_complex", stdout);
}

/// SNAP-007: `--latex "[[1,2],[3,4]]"` → pmatrix snapshot
#[test]
fn snap_007_latex_matrix() {
    let output = calnexus()
        .args(["--latex", "[[1,2],[3,4]]"])
        .output()
        .expect("failed to execute");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    insta::assert_snapshot!("snap_007_latex_matrix", stdout);
}

/// SNAP-008: `--batch` fixture → summary snapshot
#[test]
fn snap_008_batch_summary() {
    // 创建临时批量文件
    let tmp = tempfile::NamedTempFile::new().expect("failed to create temp file");
    std::fs::write(tmp.path(), "2+3\n4+5\n6+7\n").expect("failed to write");
    let path_str = tmp.path().to_str().expect("path to str");

    let output = calnexus()
        .args(["--batch", path_str])
        .output()
        .expect("failed to execute");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    // 替换临时路径为占位符，保证快照稳定
    let stable = stdout.replace(path_str, "<BATCH_FILE>");
    insta::assert_snapshot!("snap_008_batch_summary", stable);
}

// 验证 insta 已正确加入 dev-dependencies（编译时检查）
#[test]
fn snap_insta_dependency_compiles() {
    let _ = insta::Settings::new();
    assert!(Path::new("tests/snapshot_tests.rs").exists());
}

/// T003 Red: 多字节 UTF-8 字符的 Span 位置应为字符偏移（kueiku HIGH-1）
///
/// "你好+1" 是 4 个字符（你/好/+/1）= 8 字节（中文每字 3 字节 + ASCII 2 字节）。
/// mathexpr 不支持中文变量名，会触发解析失败。
/// Span 应为 (0, 4)（字符偏移）而非 (0, 8)（字节偏移）。
///
/// T003 Red 阶段：此测试应失败（当前代码用 after_implicit.len() 返回字节偏移 8）。
#[test]
fn test_span_multibyte_char_position() {
    let output = calnexus()
        .arg("你好+1")
        .output()
        .expect("failed to execute");
    assert!(
        !output.status.success(),
        "expected non-zero exit for invalid expr"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    // 期望字符偏移：(0, 4) — "你好+1" 是 4 个字符
    assert!(
        stderr.contains("Position 0:4"),
        "expected char offset (0:4), got stderr: {}",
        stderr
    );
    // 反向断言：不应使用字节偏移 (0, 8)
    assert!(
        !stderr.contains("Position 0:8"),
        "should not use byte offset (0:8), got stderr: {}",
        stderr
    );
}
