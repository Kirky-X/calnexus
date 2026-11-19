// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! CLI 绘图/谜题集成测试（required-features = cli, plot）。

use assert_cmd::Command;

#[test]
fn test_plot_sin_prints_braille() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    let out = cmd
        .arg("--plot")
        .arg("sin(x)")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(out).unwrap();
    assert!(
        stdout
            .chars()
            .any(|c| ('\u{2800}'..='\u{28ff}').contains(&c)),
        "expected Braille dots, got: {stdout}"
    );
    assert!(stdout.contains("y["), "{stdout}");
}

#[test]
fn test_plot_constant_is_usage_error() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--plot").arg("42").assert().code(2);
}

#[test]
fn test_plot_x_range_invalid_is_usage_error() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--plot")
        .arg("--x-range")
        .arg("5:1")
        .arg("sin(x)")
        .assert()
        .code(2);
}

#[test]
fn test_heatmap_identity_matrix() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    let out = cmd
        .arg("--heatmap")
        .arg("identity(4)")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(out).unwrap();
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 5, "{stdout}");
    assert!(lines[4].contains("4x4"), "{stdout}");
    assert!(lines[0].chars().next().unwrap() > lines[0].chars().nth(1).unwrap());
}

#[test]
fn test_heatmap_rejects_scalar() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--heatmap").arg("42").assert().failure().code(1);
}
