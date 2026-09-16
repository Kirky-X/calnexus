// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

#![allow(clippy::approx_constant, non_snake_case)]

//! CLI 集成测试：使用 assert_cmd 执行真实二进制，验证 cli-interface spec。
//!
//! 覆盖 12 个 requirements / 23 个 scenarios。

use assert_cmd::Command;

// ===== Requirement 1: Single Expression Evaluation =====

#[test]
fn test_basic_addition() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("2+3").assert().success().stdout("5\n");
}

#[test]
fn test_complex_arithmetic() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("(2+9)*7-6").assert().success().stdout("71\n");
}

// ===== Requirement 2: stdin Pipeline =====

#[test]
fn test_stdin_simple_expression() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.write_stdin("2+3").assert().success().stdout("5\n");
}

#[test]
fn test_stdin_scientific_expression() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.write_stdin("sin(pi/2)")
        .assert()
        .success()
        .stdout("1\n");
}

// ===== Requirement 3: JSON Output Format =====

#[test]
fn test_json_arithmetic() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--json").arg("2+3").assert().success().stdout(
        r#"{"cache":"miss","domain":"arithmetic","result":5.0,"v":1}
"#,
    );
}

#[test]
fn test_json_scientific() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--json")
        .arg("sin(pi/2)")
        .assert()
        .success()
        .stdout(
            r#"{"cache":"miss","domain":"scientific","result":1.0,"v":1}
"#,
        );
}

// ===== Requirement 4: Single Variable Binding =====

#[test]
fn test_single_var_arithmetic() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--var")
        .arg("x=10")
        .arg("x*2")
        .assert()
        .success()
        .stdout("20\n");
}

#[test]
fn test_single_var_scientific() {
    // 注：spec 给出的期望值 0.9999996829318346 与 sin(3.14) 不符（sin(3.14)≈0.00159），
    // 此处用 x=1 验证变量代入功能，期望值为 sin(1) 的正确结果。
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--var")
        .arg("x=1")
        .arg("sin(x)")
        .assert()
        .success()
        .stdout("0.8414709848078965\n");
}

// ===== Requirement 5: Multiple Variable Binding =====

#[test]
fn test_two_variables() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--var")
        .arg("x=1")
        .arg("--var")
        .arg("y=2")
        .arg("x+y")
        .assert()
        .success()
        .stdout("3\n");
}

#[test]
fn test_three_variables() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--var")
        .arg("x=1")
        .arg("--var")
        .arg("y=2")
        .arg("--var")
        .arg("z=3")
        .arg("x+y+z")
        .assert()
        .success()
        .stdout("6\n");
}

// ===== Requirement 6: Computation Error Exit Code =====

#[test]
fn test_division_by_zero_exit_code() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("5/0").assert().failure().code(1);
}

#[test]
fn test_modulo_by_zero_exit_code() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("10%0").assert().failure().code(1);
}

// ===== Requirement 7: Invalid Expression Exit Code =====

#[test]
fn test_double_operator_exit_code() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("2++3").assert().failure().code(1);
}

#[test]
fn test_unbalanced_parens_exit_code() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("(2+3").assert().failure().code(1);
}

// ===== Requirement 8: System Error Exit Code =====

#[test]
fn test_unknown_flag_exit_code() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--unknown-flag")
        .arg("2+3")
        .assert()
        .failure()
        .code(2);
}

// ===== Requirement 9: No Arguments Reads stdin =====

#[test]
fn test_no_args_piped_stdin() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.write_stdin("2+3").assert().success().stdout("5\n");
}

#[test]
fn test_no_args_piped_stdin_scientific() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.write_stdin("cos(0)").assert().success().stdout("1\n");
}

// ===== Requirement 11: Version Flag =====

#[test]
fn test_long_version_flag() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    let output = cmd
        .arg("--version")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).unwrap();
    assert!(
        stdout.contains("calnexus"),
        "expected version string, got: {}",
        stdout
    );
}

#[test]
fn test_short_version_flag() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    let output = cmd.arg("-V").assert().success().get_output().stdout.clone();
    let stdout = String::from_utf8(output).unwrap();
    assert!(
        stdout.contains("calnexus"),
        "expected version string, got: {}",
        stdout
    );
}

// ===== Requirement 12: Help Flag =====

#[test]
fn test_long_help_flag() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    let output = cmd
        .arg("--help")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).unwrap();
    assert!(stdout.contains("--json"), "help should mention --json");
    assert!(stdout.contains("--var"), "help should mention --var");
    assert!(
        stdout.contains("--version"),
        "help should mention --version"
    );
}

#[test]
fn test_short_help_flag() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    let output = cmd.arg("-h").assert().success().get_output().stdout.clone();
    let stdout = String::from_utf8(output).unwrap();
    assert!(!stdout.is_empty(), "help output should not be empty");
}

// ===== --precision flag 覆盖（precision 模式 + BigRational 输出） =====

#[test]
fn test_precision_flag_with_division() {
    // 覆盖 cli.rs lines 168, 172-175（precision 模式）+ lines 85, 87, 98（BigRational 输出）
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--precision")
        .arg("5")
        .arg("1/3")
        .assert()
        .success()
        .stdout("0.33333\n");
}

#[test]
fn test_precision_flag_with_integer_result() {
    // --precision 3 "4/2" → "2"（整数结果仍走 precision 路径，但 rational_to_result 返回 BigInt）
    // 覆盖 precision 模式 + BigInt 输出
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--precision")
        .arg("3")
        .arg("4/2")
        .assert()
        .success()
        .stdout("2\n");
}

#[test]
fn test_precision_flag_zero_decimals() {
    // --precision 0 "1/2" → "1"（四舍五入 0.5 → 1）
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--precision")
        .arg("0")
        .arg("1/2")
        .assert()
        .success()
        .stdout("1\n");
}

// ===== precision(N, expr) 函数调用覆盖 =====

#[test]
fn test_precision_function_call() {
    // 覆盖 cli.rs lines 210-214（extract_format_precision）+ lines 85, 87, 98（BigRational 输出）
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("precision(5, 1/3)")
        .assert()
        .success()
        .stdout("0.33333\n");
}

#[test]
fn test_precision_function_call_json() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--json")
        .arg("precision(5, 1/3)")
        .assert()
        .success()
        .stdout(
            r#"{"cache":"miss","domain":"precision","result":"0.33333","v":1}
"#,
        );
}

// ===== BigInt 输出覆盖 =====

#[test]
fn test_bigint_addition_output() {
    // 覆盖 cli.rs lines 81, 97（BigInt 输出）
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("123456789012345678901234567890 + 1")
        .assert()
        .success()
        .stdout("123456789012345678901234567891\n");
}

#[test]
fn test_bigint_literal_output() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("123456789012345678901234567890")
        .assert()
        .success()
        .stdout("123456789012345678901234567890\n");
}

#[test]
fn test_bigint_json_output() {
    // 覆盖 cli.rs line 81（BigInt JSON 输出）
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--json")
        .arg("123456789012345678901234567890")
        .assert()
        .success()
        .stdout(
            r#"{"cache":"miss","domain":"precision","result":"123456789012345678901234567890","v":1}
"#,
        );
}

// ===== Complex 输出覆盖 =====

#[test]
fn test_complex_output_standard() {
    // 覆盖 cli.rs lines 69, 71, 95（Complex 输出 + format_complex 正虚部）
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("3+4i").assert().success().stdout("3+4i\n");
}

#[test]
fn test_complex_output_negative_imaginary() {
    // 覆盖 cli.rs line 225（format_complex 负虚部分支）
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("3-4i").assert().success().stdout("3-4i\n");
}

#[test]
fn test_complex_json_output() {
    // 覆盖 cli.rs lines 69, 71（Complex JSON 输出）
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--json").arg("3+4i").assert().success().stdout(
        r#"{"cache":"miss","domain":"complex","result":{"im":4.0,"re":3.0},"v":1}
"#,
    );
}

// ===== Matrix 输出覆盖 =====

#[test]
fn test_matrix_output_2x2() {
    // 覆盖 cli.rs lines 75, 77, 96（Matrix 输出 + format_matrix）
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("[[1,2],[3,4]]")
        .assert()
        .success()
        .stdout("[[1,2],[3,4]]\n");
}

#[test]
fn test_matrix_json_output() {
    // 覆盖 cli.rs lines 75, 77（Matrix JSON 输出）
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--json")
        .arg("[[1,2],[3,4]]")
        .assert()
        .success()
        .stdout(
            r#"{"cache":"miss","domain":"matrix","result":"[[1,2],[3,4]]","v":1}
"#,
        );
}

#[test]
fn test_matrix_output_1x3() {
    // 覆盖 format_matrix 不同维度
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("[[1,2,3]]")
        .assert()
        .success()
        .stdout("[[1,2,3]]\n");
}

// ===== BigRational JSON 输出覆盖 =====

#[test]
fn test_bigrational_json_output_with_precision_flag() {
    // 覆盖 cli.rs lines 85, 87（BigRational JSON 输出）
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--json")
        .arg("--precision")
        .arg("5")
        .arg("1/3")
        .assert()
        .success()
        .stdout(
            r#"{"cache":"miss","domain":"precision","result":"0.33333","v":1}
"#,
        );
}

#[test]
fn test_bigrational_output_fraction_form() {
    // 无 --precision 时 precision(0, 1/3) 应当走 precision 域
    // 但 precision(N, expr) 中 N 必须为正整数，N=0 会报错；
    // 改用 precision(5, 2/3) 验证非整数 BigRational 输出
    // 四舍五入 0.666666... → 0.66667
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("precision(5, 2/3)")
        .assert()
        .success()
        .stdout("0.66667\n");
}

// ===== 错误路径覆盖 =====

#[test]
fn test_invalid_var_missing_equals_exit_code() {
    // 覆盖 cli.rs lines 52-54, 141（parse_vars 错误 + run 返回 2）
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--var")
        .arg("invalid")
        .arg("2+3")
        .assert()
        .failure()
        .code(2);
}

#[test]
fn test_invalid_var_non_numeric_value_exit_code() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--var")
        .arg("x=abc")
        .arg("x*2")
        .assert()
        .failure()
        .code(2);
}

#[test]
fn test_empty_stdin_exit_code() {
    // echo "" | calnexus（空 stdin）→ exit 2（ErrorKind::Usage 用法错误）
    // 覆盖 cli.rs get_expression empty stdin 错误走 handle_error → exit_code 2
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.write_stdin("").assert().failure().code(2);
}

#[test]
fn test_whitespace_only_stdin_exit_code() {
    // echo " " | calnexus（仅空白 stdin）→ exit 2（ErrorKind::Usage 用法错误）
    // 覆盖 cli.rs get_expression empty stdin 错误走 handle_error → exit_code 2
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.write_stdin("   \n  ").assert().failure().code(2);
}

// ===== 额外 BigInt 运算覆盖 =====

#[test]
fn test_bigint_multiplication_output() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("123456789012345678901234567890 * 2")
        .assert()
        .success()
        .stdout("246913578024691357802469135780\n");
}

#[test]
fn test_bigint_subtraction_output() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("123456789012345678901234567890 - 1")
        .assert()
        .success()
        .stdout("123456789012345678901234567889\n");
}

// ===== Statistics 域 CLI 端到端测试 =====

#[test]
fn test_statistics_mean_cli() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("mean([1,2,3,4,5])")
        .assert()
        .success()
        .stdout("3\n");
}

#[test]
fn test_statistics_sum_cli() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("sum([1,2,3,4,5])")
        .assert()
        .success()
        .stdout("15\n");
}

#[test]
fn test_statistics_median_cli() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("median([1,2,3,4,5])")
        .assert()
        .success()
        .stdout("3\n");
}

#[test]
fn test_statistics_json_output() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--json")
        .arg("mean([1,2,3,4,5])")
        .assert()
        .success()
        .stdout(
            r#"{"cache":"miss","domain":"statistics","result":3.0,"v":1}
"#,
        );
}

#[test]
fn test_statistics_stdin_pipeline() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.write_stdin("sum([10,20,30])")
        .assert()
        .success()
        .stdout("60\n");
}

#[test]
fn test_statistics_empty_list_error_cli() {
    // mean([]) → exit 1（空列表 DomainError）
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("mean([])").assert().failure().code(1);
}

// ===== 新增域 CLI 端到端测试 =====

// ----- 9.1 NumberTheory CLI 测试 -----

#[test]
fn test_cli_number_theory_gcd() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("gcd(12,18)").assert().success().stdout("6\n");
}

#[test]
fn test_cli_number_theory_is_prime() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("is_prime(7)").assert().success().stdout("1\n");
}

#[test]
fn test_cli_number_theory_prime_sieve() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("prime_sieve(10)")
        .assert()
        .success()
        .stdout("[2,3,5,7]\n");
}

#[test]
fn test_cli_number_theory_json_domain() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--json")
        .arg("gcd(12,18)")
        .assert()
        .success()
        .stdout(
            r#"{"cache":"miss","domain":"number_theory","result":6.0,"v":1}
"#,
        );
}

// ----- 9.2 Combinatorics CLI 测试 -----

#[test]
fn test_cli_combinatorics_C() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("C(10,3)").assert().success().stdout("120\n");
}

#[test]
fn test_cli_combinatorics_catalan() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("catalan(5)").assert().success().stdout("42\n");
}

#[test]
fn test_cli_combinatorics_P() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("P(5,2)").assert().success().stdout("20\n");
}

#[test]
fn test_cli_combinatorics_stdin() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.write_stdin("catalan(5)")
        .assert()
        .success()
        .stdout("42\n");
}

// ----- 9.3 Vector CLI 测试 -----

#[test]
fn test_cli_vector_dot() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("dot([1,2,3],[4,5,6])")
        .assert()
        .success()
        .stdout("32\n");
}

#[test]
fn test_cli_vector_norm() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("norm([3,4])").assert().success().stdout("5\n");
}

#[test]
fn test_cli_vector_add() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("[1,2]+[3,4]").assert().success().stdout("[4,6]\n");
}

#[test]
fn test_cli_vector_dimension_mismatch_error() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("[1,2]+[3,4,5]").assert().failure().code(1);
}

// ----- 9.4 Polynomial CLI 测试 -----

#[test]
fn test_cli_polynomial_add() {
    // poly_add(x+1, x+2) = 2x+3（系数升幂 [3,2]，输出降幂 → "2x+3"）
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("poly_add(x+1,x+2)")
        .assert()
        .success()
        .stdout("2x+3\n");
}

#[test]
fn test_cli_polynomial_roots_real() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("roots(x^2-4)")
        .assert()
        .success()
        .stdout("[2,-2]\n");
}

#[test]
fn test_cli_polynomial_roots_complex() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("roots(x^2+1)").assert().success();
    // 复根输出格式：[0+1i,0-1i]
}

#[test]
fn test_cli_polynomial_factor() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("factor(x^2-4)")
        .assert()
        .success()
        .stdout("(x-2)*(x+2)\n");
}

// ===== REPL CLI 测试 =====

#[test]
fn test_cli_repl_start_and_quit() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--repl").write_stdin(":quit\n").assert().success();
}

#[test]
fn test_cli_repl_evaluate_arithmetic() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--repl")
        .write_stdin("2+3*4\n:quit\n")
        .assert()
        .success()
        .stdout(predicates::str::contains("14"));
}

#[test]
fn test_cli_repl_evaluate_sin() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--repl")
        .write_stdin("sin(0)\n:quit\n")
        .assert()
        .success()
        .stdout(predicates::str::contains("0"));
}

#[test]
fn test_cli_repl_let_and_use() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--repl")
        .write_stdin(":let x = 10\nx*2\n:quit\n")
        .assert()
        .success()
        .stdout(predicates::str::contains("20"));
}

#[test]
fn test_cli_repl_vars_command() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--repl")
        .write_stdin(":let x = 42\n:vars\n:quit\n")
        .assert()
        .success()
        .stdout(predicates::str::contains("42"));
}

// ===== 批量 CLI 测试 =====

#[test]
fn test_cli_batch_basic() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    use std::io::Write;
    writeln!(tmp, "2+3").unwrap();
    writeln!(tmp, "4*5").unwrap();
    tmp.flush().unwrap();

    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--batch")
        .arg(tmp.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("5"))
        .stdout(predicates::str::contains("20"));
}

#[test]
fn test_cli_batch_comment_skipped() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    use std::io::Write;
    writeln!(tmp, "# comment").unwrap();
    writeln!(tmp, "1+1").unwrap();
    tmp.flush().unwrap();

    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--batch")
        .arg(tmp.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("2"));
}

#[test]
fn test_cli_batch_stdin() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--batch")
        .arg("-")
        .write_stdin("2+3\n4*5\n")
        .assert()
        .success()
        .stdout(predicates::str::contains("5"))
        .stdout(predicates::str::contains("20"));
}

#[test]
fn test_cli_batch_json_output() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    use std::io::Write;
    writeln!(tmp, "2+3").unwrap();
    tmp.flush().unwrap();

    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--batch")
        .arg(tmp.path())
        .arg("--json")
        .assert()
        .success()
        .stdout(predicates::str::contains("\"result\""))
        .stdout(predicates::str::contains("\"5\""));
}

#[test]
fn test_cli_batch_nonexistent_file() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--batch")
        .arg("/nonexistent/path/file.txt")
        .assert()
        .failure()
        .code(2);
}

// ===== Symbolic CLI 测试 =====

#[test]
fn test_cli_symbolic_diff_power() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("diff(x^2, x)")
        .assert()
        .success()
        .stdout(predicates::str::contains("2"));
}

#[test]
fn test_cli_symbolic_diff_sin() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("diff(sin(x), x)")
        .assert()
        .success()
        .stdout(predicates::str::contains("cos"));
}

#[test]
fn test_cli_symbolic_simplify() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("simplify(x+0)").assert().success().stdout("x\n");
}

#[test]
fn test_cli_symbolic_limit() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("limit(sin(x)/x, x, 0)")
        .assert()
        .success()
        .stdout(predicates::str::contains("1"));
}

// ===== JSON 输出路径覆盖（lines 125,127,131,133,137,139,143） =====

#[test]
fn test_json_vector_output() {
    // 覆盖 cli.rs lines 125, 127（Vector JSON 输出分支）
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--json")
        .arg("[1,2]+[3,4]")
        .assert()
        .success()
        .stdout(
            r#"{"cache":"miss","domain":"vector","result":"[4,6]","v":1}
"#,
        );
}

#[test]
fn test_json_polynomial_output() {
    // 覆盖 cli.rs lines 131, 133（Polynomial JSON 输出分支）
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--json")
        .arg("poly_add(x+1,x+2)")
        .assert()
        .success()
        .stdout(
            r#"{"cache":"miss","domain":"polynomial","result":"2x+3","v":1}
"#,
        );
}

#[test]
fn test_json_complex_list_output() {
    // 覆盖 cli.rs lines 137, 139（ComplexList JSON 输出分支）
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--json")
        .arg("roots(x^2+1)")
        .assert()
        .success()
        .stdout(
            r#"{"cache":"miss","domain":"polynomial","result":"[-0+1i,-0-1i]","v":1}
"#,
        );
}

#[test]
fn test_json_symbolic_output() {
    // 覆盖 cli.rs line 143（Symbolic JSON 输出分支）
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--json")
        .arg("diff(x^2, x)")
        .assert()
        .success()
        .stdout(
            r#"{"cache":"miss","domain":"symbolic","result":"2*x","v":1}
"#,
        );
}

// ===== parse_vars 错误路径覆盖（lines 55-57, 68-70） =====

#[test]
fn test_repl_invalid_var_exit_code() {
    // 覆盖 cli.rs lines 55, 56, 57（REPL 模式 parse_vars 错误 + return 2）
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--repl")
        .arg("--var")
        .arg("invalid")
        .write_stdin(":quit\n")
        .assert()
        .failure()
        .code(2);
}

#[test]
fn test_batch_invalid_var_exit_code() {
    // 覆盖 cli.rs lines 68, 69, 70（batch 模式 parse_vars 错误 + return 2）
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    use std::io::Write;
    writeln!(tmp, "2+3").unwrap();
    tmp.flush().unwrap();
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--batch")
        .arg(tmp.path())
        .arg("--var")
        .arg("invalid")
        .assert()
        .failure()
        .code(2);
}

// ===== REPL format_result 分支覆盖（lines 296-302） =====
// format_result 在 REPL evaluate_line 中调用，需通过 REPL 触发各 EvalResult 变体

#[test]
fn test_repl_complex_format_result() {
    // 覆盖 cli.rs line 296（format_result::Complex）
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--repl")
        .write_stdin("3+4i\n:quit\n")
        .assert()
        .success()
        .stdout(predicates::str::contains("3+4i"));
}

#[test]
fn test_repl_matrix_format_result() {
    // 覆盖 cli.rs line 297（format_result::Matrix）
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--repl")
        .write_stdin("[[1,2],[3,4]]\n:quit\n")
        .assert()
        .success()
        .stdout(predicates::str::contains("[[1,2],[3,4]]"));
}

#[test]
fn test_repl_bigint_format_result() {
    // 覆盖 cli.rs line 298（format_result::BigInt）
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--repl")
        .write_stdin("123456789012345678901234567890\n:quit\n")
        .assert()
        .success()
        .stdout(predicates::str::contains("123456789012345678901234567890"));
}

#[test]
fn test_repl_bigrational_format_result() {
    // 覆盖 cli.rs line 299（format_result::BigRational）
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--repl")
        .arg("--precision")
        .arg("5")
        .write_stdin("1/3\n:quit\n")
        .assert()
        .success()
        .stdout(predicates::str::contains("0.33333"));
}

#[test]
fn test_repl_vector_format_result() {
    // 覆盖 cli.rs line 300（format_result::Vector）
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--repl")
        .write_stdin("[1,2]+[3,4]\n:quit\n")
        .assert()
        .success()
        .stdout(predicates::str::contains("[4,6]"));
}

#[test]
fn test_repl_polynomial_format_result() {
    // 覆盖 cli.rs line 301（format_result::Polynomial）
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--repl")
        .write_stdin("poly_add(x+1,x+2)\n:quit\n")
        .assert()
        .success()
        .stdout(predicates::str::contains("2x+3"));
}

#[test]
fn test_repl_complex_list_format_result() {
    // 覆盖 cli.rs line 302（format_result::ComplexList）
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--repl")
        .write_stdin("roots(x^2+1)\n:quit\n")
        .assert()
        .success()
        .stdout(predicates::str::contains("1i"));
}

// ===== format_polynomial 分支覆盖（lines 343,349,351,357-360,362,369,375） =====

#[test]
fn test_polynomial_sub_x2_minus_x() {
    // 覆盖 cli.rs lines 343（coef=0 跳过）, 351（i=1,coef=-1 → "-x"）,
    //   357-358（i=2,coef=1 → "x^2"）, 375（负项拼接）
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("poly_sub(x^2, x)")
        .assert()
        .success()
        .stdout("x^2-x\n");
}

#[test]
fn test_polynomial_neg_x2() {
    // 覆盖 cli.rs lines 359-360（i>=2,coef=-1 → "-x^N"）
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("poly_sub(0, x^2)")
        .assert()
        .success()
        .stdout("-x^2\n");
}

#[test]
fn test_polynomial_x_leading_one() {
    // 覆盖 cli.rs line 349（i=1,coef=1 → "x"）
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("poly_add(x, 0)").assert().success().stdout("x\n");
}

#[test]
fn test_polynomial_all_zero_coeffs() {
    // 覆盖 cli.rs line 369（所有系数为零 → "0"）
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("poly_sub(x, x)").assert().success().stdout("0\n");
}

#[test]
fn test_polynomial_general_coef_high_degree() {
    // 覆盖 cli.rs line 362（i>=2,一般系数 → "cx^N"）
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("poly_mul(2, x^2)")
        .assert()
        .success()
        .stdout("2x^2\n");
}

// ===== 新增 CLI 标志集成测试 =====

#[test]
fn it_cli_003_latex_output() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.args(["--latex", "diff(x^2,x)"])
        .assert()
        .success()
        .stdout(predicates::str::contains("\\frac{d}{dx}"));
}

/// `--canonical "3+2"` → `(+ 2 3)`（PRD §3.2.4）
#[test]
fn it_cli_009_canonical_output() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.args(["--canonical", "3+2"])
        .assert()
        .success()
        .stdout("(+ 2 3)\n");
}

#[test]
fn it_cli_010_steps_output() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.args(["--steps", "(2+9)*7-6"])
        .assert()
        .success()
        .stdout(predicates::str::contains("2+9=11"));
}

#[test]
fn it_cli_latex_json_conflict_exit_2() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.args(["--latex", "--json", "2+3"])
        .assert()
        .failure()
        .code(2);
}

#[test]
fn it_cli_canonical_repl_conflict_exit_2() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.args(["--canonical", "--repl"])
        .assert()
        .failure()
        .code(2);
}

#[test]
fn it_cli_canonical_batch_conflict_exit_2() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.args(["--canonical", "--batch", "-"])
        .assert()
        .failure()
        .code(2);
}

// ===== 不可覆盖行说明 =====
// 以下 cli.rs 行因技术限制无法通过集成测试覆盖：
// - lines 178, 179：TTY stdin 路径（io::stdin().is_terminal() 为 true 时显示 help）。
//   集成测试中 stdin 始终为管道（非 TTY），此分支不可达。
//   line 179 的 `return Err(0)` 标注为 unreachable，clap --help 会先退出。
// - lines 184, 185：stdin read_to_string 错误路径。管道 stdin 不会产生 I/O 错误，
//   无法在集成测试中模拟。
// - line 338：format_polynomial 空系数向量（p.is_empty()）。
//   多项式域对所有输入至少返回 [0.0]，无法产生空向量。

// ===== 覆盖 cli.rs --canonical / --latex / --steps 错误路径 =====

/// `--canonical "2++3"` → 解析错误，退出码 1（lines 119-121）
#[test]
fn it_cli_canonical_parse_error_exit_1() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.args(["--canonical", "2++3"]).assert().failure().code(1);
}

// 注：lines 129-131（--canonical canonicalize 错误）不可覆盖：
// canonicalize_no_fold 不做常量折叠，对任何合法解析 AST 均返回 Ok。

/// `--latex "2++3"` → 解析错误，退出码 1（lines 138-140）
#[test]
fn it_cli_latex_parse_error_exit_1() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.args(["--latex", "2++3"]).assert().failure().code(1);
}

/// `--latex "1/0"` → 规范化错误，退出码 1（lines 145-147）
#[test]
fn it_cli_latex_canonicalize_error_exit_1() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.args(["--latex", "1/0"]).assert().failure().code(1);
}

/// `--steps "1/0"` → 步骤生成错误（除零），退出码 1（lines 159-161）
#[test]
fn it_cli_steps_div_zero_exit_1() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.args(["--steps", "1/0"]).assert().failure().code(1);
}

/// `--latex "sqrt(-1)"` → 求值错误（域错误），退出码 1（lines 174-176）
#[test]
fn it_cli_latex_eval_error_exit_1() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.args(["--latex", "sqrt(-1)"]).assert().failure().code(1);
}

// ===== T0.4.7: --explain / --lang / --json error / 退出码契约 =====

#[test]
fn test_explain_parse_error_exit_1() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.args(["--explain", "2++3"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicates::str::contains("Parse error"));
}

#[test]
fn test_explain_div_zero_hint() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.args(["--explain", "1/0"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicates::str::contains("check divisor"));
}

#[test]
fn test_lang_zh_parse_error() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.args(["--lang", "zh", "2++3"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicates::str::contains("解析错误"));
}

#[test]
fn test_lang_en_parse_error() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.args(["--lang", "en", "2++3"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicates::str::contains("Parse error"));
}

/// `--json "2++3"` → stdout 输出 JSON error 对象（非双层嵌套），退出码 1
#[test]
fn test_json_error_output_exit_1() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    // JSON 键序由 serde_json 决定（字典序），断言改为语义校验（解析后查字段）
    let output = cmd
        .args(["--json", "2++3"])
        .output()
        .expect("failed to execute");
    assert_eq!(output.status.code(), Some(1));
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("错误输出必须是合法 JSON");
    assert_eq!(json["error"]["kind"], "Parse");
    assert_eq!(json["error"]["exit_code"], 1);
}

#[test]
fn test_explain_conflicts_with_json() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.args(["--explain", "--json", "2+3"])
        .assert()
        .failure()
        .code(2);
}

#[test]
fn test_success_exit_0() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("2+3").assert().success().code(0);
}

#[test]
fn test_explain_domain_hint() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.args(["--explain", "asin(2)"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicates::str::contains("asin domain is [-1, 1]"));
}

/// `--explain --lang en "2++3"` → stderr 不含任何中文字符
///
/// 验证 friendly()/to_explain() 中的 5 个硬编码中文标签（位置/提示/错误类别/退出码/建议）
/// 在 --lang en 时不出现。
#[test]
fn test_explain_lang_en_no_chinese_labels() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    let assert = cmd
        .args(["--explain", "--lang", "en", "2++3"])
        .assert()
        .failure()
        .code(1);
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    // 不含任何 CJK 字符（Unicode 范围 \u4e00-\u9fff）
    let has_cjk = stderr
        .chars()
        .any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c));
    assert!(
        !has_cjk,
        "stderr with --lang en should not contain CJK characters, got: {}",
        stderr
    );
}

// ===== --json 模式下 parse_vars / get_expression 错误应走 JSON 输出 =====
// 当前 Bug：parse_vars() 返回 Result<_, String>，get_expression() 返回 Result<_, i32>，
// 错误时直接 eprintln 文本 + 返回退出码，绕过 handle_error()，
// 导致 --json/--explain 模式下错误输出格式不一致（总是 eprintln 文本而非 JSON）。

/// `--json --var invalid 2+3` → stdout 应含 `"error"` JSON 字段，stderr 不应含 `error: invalid --var`。
///
/// parse_vars 错误必须走 JSON 输出路径（而非 eprintln 文本路径）。
#[test]
fn test_json_mode_var_error_output_format() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    let assert = cmd
        .args(["--json", "--var", "invalid", "2+3"])
        .assert()
        .failure();
    let output = assert.get_output();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    // --json 模式错误应走 JSON 输出（stdout 含 "error" 字段）
    assert!(
        stdout.contains("\"error\""),
        "stdout should contain JSON error field, got stdout: {:?}, stderr: {:?}",
        stdout,
        stderr
    );
    // 不应走 eprintln 文本路径（stderr 不应含 "error: invalid --var"）
    assert!(
        !stderr.contains("error: invalid --var"),
        "stderr should not contain eprintln text path, got stderr: {:?}",
        stderr
    );
}

/// `echo "" | calnexus --json`（空 stdin）→ stdout 应含 `"error"` JSON 字段，stderr 不应含 `error: empty expression`。
///
/// get_expression 错误必须走 JSON 输出路径（而非 eprintln 文本路径）。
#[test]
fn test_json_mode_empty_stdin_error_output_format() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    let assert = cmd.arg("--json").write_stdin("").assert().failure();
    let output = assert.get_output();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    // --json 模式错误应走 JSON 输出（stdout 含 "error" 字段）
    assert!(
        stdout.contains("\"error\""),
        "stdout should contain JSON error field, got stdout: {:?}, stderr: {:?}",
        stdout,
        stderr
    );
    // 不应走 eprintln 文本路径（stderr 不应含 "error: empty expression"）
    assert!(
        !stderr.contains("error: empty expression"),
        "stderr should not contain eprintln text path, got stderr: {:?}",
        stderr
    );
}

/// --lang 未知值时 clap 退出码 2（fail-loud）
///
/// PossibleValuesParser 限制 --lang 只接受 "en"/"zh"，
/// 传入 "fr" 时 clap 报错并退出码 2。
#[test]
fn test_lang_invalid_value_exit_2() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    let assert = cmd.args(["--lang", "fr", "2+3"]).assert().failure();
    let output = assert.get_output();
    assert_eq!(
        output.status.code(),
        Some(2),
        "invalid --lang value should exit with code 2, got: {:?}",
        output.status
    );
}

// ===== --serve-http/--serve-mcp flag 冲突与 feature 门控 =====

/// --serve-http 与 --repl 冲突时 clap 退出码 2（server feature 启用）。
#[cfg(feature = "server")]
#[test]
fn test_serve_http_flag_conflicts_repl() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    let assert = cmd.args(["--serve-http", "--repl"]).assert().failure();
    let output = assert.get_output();
    assert_eq!(
        output.status.code(),
        Some(2),
        "--serve-http --repl should exit with code 2 (conflict), got: {:?}",
        output.status
    );
}

/// --serve-mcp 与 --batch 冲突时 clap 退出码 2（server feature 启用）。
#[cfg(feature = "server")]
#[test]
fn test_serve_mcp_flag_conflicts_batch() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    let assert = cmd.args(["--serve-mcp", "--batch", "-"]).assert().failure();
    let output = assert.get_output();
    assert_eq!(
        output.status.code(),
        Some(2),
        "--serve-mcp --batch should exit with code 2 (conflict), got: {:?}",
        output.status
    );
}

/// --serve-http 与 --serve-mcp 互相冲突时 clap 退出码 2。
/// 防止静默吞 flag（失败必须显性化）。
#[cfg(feature = "server")]
#[test]
fn test_serve_http_conflicts_serve_mcp() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    let assert = cmd.args(["--serve-http", "--serve-mcp"]).assert().failure();
    let output = assert.get_output();
    assert_eq!(
        output.status.code(),
        Some(2),
        "--serve-http --serve-mcp should exit with code 2 (mutual conflict), got: {:?}",
        output.status
    );
}

/// --serve-http 与 --json 冲突时 clap 退出码 2（server feature 启用）。
#[cfg(feature = "server")]
#[test]
fn test_serve_http_conflicts_json() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    let assert = cmd.args(["--serve-http", "--json"]).assert().failure();
    let output = assert.get_output();
    assert_eq!(
        output.status.code(),
        Some(2),
        "--serve-http --json should exit with code 2 (conflict), got: {:?}",
        output.status
    );
}

/// 无 server feature 时 --serve-http 是 unknown argument，clap 退出码 2。
/// 验证 feature 门控正确：无 server feature 时 flag 不存在。
#[cfg(not(feature = "server"))]
#[test]
fn test_serve_http_without_server_feature() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    let assert = cmd.args(["--serve-http"]).assert().failure();
    let output = assert.get_output();
    assert_eq!(
        output.status.code(),
        Some(2),
        "--serve-http without server feature should exit with code 2 (unknown arg), got: {:?}",
        output.status
    );
}

/// `--batch --precision` 组合应显式冲突退出码 2。
///
/// 原行为：`batch.rs:88` 硬编码 `None` 忽略 precision，违反失败显性化原则。
/// 添加 `conflicts_with_all` 互斥，clap 在参数解析阶段拒绝组合。
///
/// 使用临时文件（非 stdin）确保 batch 模式实际运行：未修复时退出 0（静默忽略 precision），
/// 修复后退出 2（冲突）。若用 stdin，空输入也会退出 2，无法区分冲突与空输入。
#[test]
fn test_batch_precision_conflict_exit_2() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    use std::io::Write;
    writeln!(tmp, "2+3").unwrap();
    tmp.flush().unwrap();

    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    let assert = cmd
        .arg("--batch")
        .arg(tmp.path())
        .arg("--precision")
        .arg("5")
        .assert()
        .failure();
    let output = assert.get_output();
    assert_eq!(
        output.status.code(),
        Some(2),
        "--batch --precision should exit with code 2 (conflict), got: {:?}",
        output.status
    );
}

/// 测试矩阵 A802：`--repl --batch` 互斥。
///
/// 此前 `repl` 与 `batch` 未声明互相 conflicts_with_all，运行时优先级链
/// (`run()` 中 `if cli.repl { ... }` 先于 `if let Some(path) = &cli.batch`) 会导致
/// batch 被 silent 吞掉（静默 fallback 到 REPL，隐性失败，违反失败显性化）。
/// 添加双向 conflict 后 clap 在解析阶段拒绝，退出 2。
#[test]
fn test_repl_batch_conflict_exit_2() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    use std::io::Write;
    writeln!(tmp, "2+3").unwrap();
    tmp.flush().unwrap();

    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    // 用临时文件（非 stdin）确保走 batch 路径而非空输入；冲突在 clap 解析阶段触发，
    // REPL 不会启动，但提供 quit 输入以防万一。
    let assert = cmd
        .arg("--repl")
        .arg("--batch")
        .arg(tmp.path())
        .write_stdin(":quit\n")
        .assert()
        .failure();
    let output = assert.get_output();
    assert_eq!(
        output.status.code(),
        Some(2),
        "--repl --batch should exit with code 2 (conflict), got: {:?}",
        output.status
    );
}

// ===== 配置面 =====

/// CFG-001: --timeout 0.1 使慢表达式超时，退出码 3（Timeout）。
#[test]
fn timeout_flag_slow_expression_exits_3() {
    // debug 构建下 sieve(10_000_000) 远超 0.1s；超时在 evaluate 阶段边界命中
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    let output = cmd
        .args(["--timeout", "0.1", "prime_sieve(10000000)"])
        .env_remove("CALNEXUS_TIMEOUT")
        .output()
        .expect("failed to execute");
    assert_eq!(
        output.status.code(),
        Some(3),
        "--timeout 0.1 慢表达式应以退出码 3（Timeout）结束，实际 {:?}，stderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
}

/// CFG-001b: CALNEXUS_TIMEOUT env 生效（无 flag 时）；非法 env 值报错退出码 2。
#[test]
fn timeout_env_var_fallback_and_validation() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    let output = cmd
        .args(["prime_sieve(10000000)"])
        .env("CALNEXUS_TIMEOUT", "0.1")
        .output()
        .expect("failed to execute");
    assert_eq!(
        output.status.code(),
        Some(3),
        "env CALNEXUS_TIMEOUT=0.1 应超时退出 3"
    );

    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    let output = cmd
        .args(["--timeout", "9999", "1+1"])
        .env_remove("CALNEXUS_TIMEOUT")
        .output()
        .expect("failed to execute");
    assert_eq!(
        output.status.code(),
        Some(2),
        "--timeout 9999 超范围应退出 2"
    );
}

/// CFG-002: --cache-size 旗标被接受；0 值被拒绝（退出码 2）。
#[test]
fn cache_size_flag_accepted_and_validated() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.args(["--cache-size", "2", "1+1"])
        .env_remove("CALNEXUS_CACHE_SIZE")
        .assert()
        .success()
        .stdout("2\n");

    let output = Command::cargo_bin("calnexus")
        .unwrap()
        .args(["--cache-size", "0", "1+1"])
        .output()
        .expect("failed to execute");
    assert_eq!(
        output.status.code(),
        Some(2),
        "--cache-size 0 应被拒绝（退出码 2）"
    );
}

/// CFG-003: --serve-http --bind 自定义端口可监听；flag 优先于 CALNEXUS_BIND_ADDR。
/// 需要 server feature（CI server 腿执行）。
#[cfg(feature = "server")]
#[test]
fn bind_flag_serve_http_listens_on_custom_port() {
    use std::io::Read;
    use std::time::Duration;

    // 找一个空闲端口
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    // assert_cmd 的 spawn 受限，此用例直接用 std::process::Command
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_calnexus"))
        .args(["--serve-http", "--bind", &format!("127.0.0.1:{port}")])
        // flag 优先：env 故意给一个会被覆盖的值
        .env("CALNEXUS_BIND_ADDR", "127.0.0.1:1")
        .spawn()
        .expect("spawn --serve-http");

    let mut connected = false;
    for _ in 0..100 {
        match std::net::TcpStream::connect(("127.0.0.1", port)) {
            Ok(mut s) => {
                let _ = s.read_exact(&mut [0u8; 0]);
                connected = true;
                break;
            }
            Err(_) => std::thread::sleep(Duration::from_millis(100)),
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    assert!(connected, "--bind {port} 应可连接（flag 生效且优先于 env）");
}

/// ERR-CARET: 文本模式 stderr 含表达式行 + caret 指示。
#[test]
fn parse_error_text_mode_shows_caret() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    let output = cmd.arg("(2+3").output().expect("failed to execute");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("(2+3"), "应回显表达式: {}", stderr);
    assert!(stderr.contains("  | "), "应含表达式行指示符: {}", stderr);
    assert!(stderr.contains('^'), "应含 caret 指示: {}", stderr);
}

/// ERR-HINT-CLI: undefined_symbol 在 CLI 语境 hint 含 --var。
#[test]
fn undefined_symbol_hint_suggests_cli_var_flag() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    let output = cmd.arg("x+1").output().expect("failed to execute");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--var x=<value>"),
        "CLI 语境应提示 --var: {}",
        stderr
    );
}
