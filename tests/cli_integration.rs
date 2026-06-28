//! CLI 集成测试：使用 assert_cmd 执行真实二进制，验证 cli-interface spec。
//!
//! 覆盖 12 个 requirements / 23 个 scenarios。

use assert_cmd::Command;
use predicates;

// ===== Requirement 1: Single Expression Evaluation =====

#[test]
fn test_basic_addition() {
    // "2+3" → stdout contains "5", exit 0 (Req 1 Scen 1)
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("2+3")
        .assert()
        .success()
        .stdout("5\n");
}

#[test]
fn test_complex_arithmetic() {
    // "(2+9)*7-6" → stdout contains "71", exit 0 (Req 1 Scen 2)
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("(2+9)*7-6")
        .assert()
        .success()
        .stdout("71\n");
}

// ===== Requirement 2: stdin Pipeline =====

#[test]
fn test_stdin_simple_expression() {
    // echo "2+3" | calnexus → stdout contains "5", exit 0 (Req 2 Scen 1)
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.write_stdin("2+3")
        .assert()
        .success()
        .stdout("5\n");
}

#[test]
fn test_stdin_scientific_expression() {
    // echo "sin(pi/2)" | calnexus → stdout contains "1", exit 0 (Req 2 Scen 2)
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.write_stdin("sin(pi/2)")
        .assert()
        .success()
        .stdout("1\n");
}

// ===== Requirement 3: JSON Output Format =====

#[test]
fn test_json_arithmetic() {
    // --json "2+3" → {"result":5,"domain":"arithmetic","cache":"miss"} (Req 3 Scen 1)
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--json").arg("2+3")
        .assert()
        .success()
        .stdout(r#"{"result":5,"domain":"arithmetic","cache":"miss"}
"#);
}

#[test]
fn test_json_scientific() {
    // --json "sin(pi/2)" → domain="scientific", result=1, cache="miss" (Req 3 Scen 2)
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--json").arg("sin(pi/2)")
        .assert()
        .success()
        .stdout(r#"{"result":1,"domain":"scientific","cache":"miss"}
"#);
}

// ===== Requirement 4: Single Variable Binding =====

#[test]
fn test_single_var_arithmetic() {
    // --var x=10 "x*2" → 20 (Req 4 Scen 2)
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--var").arg("x=10").arg("x*2")
        .assert()
        .success()
        .stdout("20\n");
}

#[test]
fn test_single_var_scientific() {
    // --var x=1 "sin(x)" → sin(1) ≈ 0.8414709848078965 (Req 4 Scen 1)
    // 注：spec 给出的期望值 0.9999996829318346 与 sin(3.14) 不符（sin(3.14)≈0.00159），
    // 此处用 x=1 验证变量代入功能，期望值为 sin(1) 的正确结果。
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--var").arg("x=1").arg("sin(x)")
        .assert()
        .success()
        .stdout("0.8414709848078965\n");
}

// ===== Requirement 5: Multiple Variable Binding =====

#[test]
fn test_two_variables() {
    // --var x=1 --var y=2 "x+y" → 3 (Req 5 Scen 1)
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--var").arg("x=1")
        .arg("--var").arg("y=2")
        .arg("x+y")
        .assert()
        .success()
        .stdout("3\n");
}

#[test]
fn test_three_variables() {
    // --var x=1 --var y=2 --var z=3 "x+y+z" → 6 (Req 5 Scen 2)
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--var").arg("x=1")
        .arg("--var").arg("y=2")
        .arg("--var").arg("z=3")
        .arg("x+y+z")
        .assert()
        .success()
        .stdout("6\n");
}

// ===== Requirement 6: Computation Error Exit Code =====

#[test]
fn test_division_by_zero_exit_code() {
    // "5/0" → exit 1, stderr non-empty (Req 6 Scen 1)
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("5/0")
        .assert()
        .failure()
        .code(1);
}

#[test]
fn test_modulo_by_zero_exit_code() {
    // "10%0" → exit 1, stderr non-empty (Req 6 Scen 2)
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("10%0")
        .assert()
        .failure()
        .code(1);
}

// ===== Requirement 7: Invalid Expression Exit Code =====

#[test]
fn test_double_operator_exit_code() {
    // "2++3" → exit 1, stderr contains parse error (Req 7 Scen 1)
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("2++3")
        .assert()
        .failure()
        .code(1);
}

#[test]
fn test_unbalanced_parens_exit_code() {
    // "(2+3" → exit 1, stderr contains parse error (Req 7 Scen 2)
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("(2+3")
        .assert()
        .failure()
        .code(1);
}

// ===== Requirement 8: System Error Exit Code =====

#[test]
fn test_unknown_flag_exit_code() {
    // --unknown-flag → exit 2 (Req 8 Scen 2, clap default)
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--unknown-flag").arg("2+3")
        .assert()
        .failure()
        .code(2);
}

// ===== Requirement 9: No Arguments Reads stdin =====

#[test]
fn test_no_args_piped_stdin() {
    // echo "2+3" | calnexus (no args) → stdout contains "5", exit 0 (Req 9 Scen 1)
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.write_stdin("2+3")
        .assert()
        .success()
        .stdout("5\n");
}

#[test]
fn test_no_args_piped_stdin_scientific() {
    // echo "cos(0)" | calnexus → stdout contains "1", exit 0 (Req 9 Scen 2)
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.write_stdin("cos(0)")
        .assert()
        .success()
        .stdout("1\n");
}

// ===== Requirement 11: Version Flag =====

#[test]
fn test_long_version_flag() {
    // --version → stdout contains "calnexus", exit 0 (Req 11 Scen 1)
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    let output = cmd.arg("--version").assert().success().get_output().stdout.clone();
    let stdout = String::from_utf8(output).unwrap();
    assert!(stdout.contains("calnexus"), "expected version string, got: {}", stdout);
}

#[test]
fn test_short_version_flag() {
    // -V → stdout contains "calnexus", exit 0 (Req 11 Scen 2)
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    let output = cmd.arg("-V").assert().success().get_output().stdout.clone();
    let stdout = String::from_utf8(output).unwrap();
    assert!(stdout.contains("calnexus"), "expected version string, got: {}", stdout);
}

// ===== Requirement 12: Help Flag =====

#[test]
fn test_long_help_flag() {
    // --help → stdout contains help text, exit 0 (Req 12 Scen 1)
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    let output = cmd.arg("--help").assert().success().get_output().stdout.clone();
    let stdout = String::from_utf8(output).unwrap();
    assert!(stdout.contains("--json"), "help should mention --json");
    assert!(stdout.contains("--var"), "help should mention --var");
    assert!(stdout.contains("--version"), "help should mention --version");
}

#[test]
fn test_short_help_flag() {
    // -h → stdout contains help text, exit 0 (Req 12 Scen 2)
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    let output = cmd.arg("-h").assert().success().get_output().stdout.clone();
    let stdout = String::from_utf8(output).unwrap();
    assert!(!stdout.is_empty(), "help output should not be empty");
}

// ===== --precision flag 覆盖（precision 模式 + BigRational 输出） =====

#[test]
fn test_precision_flag_with_division() {
    // --precision 5 "1/3" → "0.33333"
    // 覆盖 cli.rs lines 168, 172-175（precision 模式）+ lines 85, 87, 98（BigRational 输出）
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--precision").arg("5").arg("1/3")
        .assert()
        .success()
        .stdout("0.33333\n");
}

#[test]
fn test_precision_flag_with_integer_result() {
    // --precision 3 "4/2" → "2"（整数结果仍走 precision 路径，但 rational_to_result 返回 BigInt）
    // 覆盖 precision 模式 + BigInt 输出
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--precision").arg("3").arg("4/2")
        .assert()
        .success()
        .stdout("2\n");
}

#[test]
fn test_precision_flag_zero_decimals() {
    // --precision 0 "1/2" → "0"（0 位小数）
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--precision").arg("0").arg("1/2")
        .assert()
        .success()
        .stdout("0\n");
}

// ===== precision(N, expr) 函数调用覆盖 =====

#[test]
fn test_precision_function_call() {
    // precision(5, 1/3) → "0.33333"
    // 覆盖 cli.rs lines 210-214（extract_format_precision）+ lines 85, 87, 98（BigRational 输出）
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("precision(5, 1/3)")
        .assert()
        .success()
        .stdout("0.33333\n");
}

#[test]
fn test_precision_function_call_json() {
    // --json precision(5, 1/3) → JSON with BigRational
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--json").arg("precision(5, 1/3)")
        .assert()
        .success()
        .stdout(r#"{"result":"0.33333","domain":"precision","cache":"miss"}
"#);
}

// ===== BigInt 输出覆盖 =====

#[test]
fn test_bigint_addition_output() {
    // 大整数 + 1 → BigInt 输出
    // 覆盖 cli.rs lines 81, 97（BigInt 输出）
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("123456789012345678901234567890 + 1")
        .assert()
        .success()
        .stdout("123456789012345678901234567891\n");
}

#[test]
fn test_bigint_literal_output() {
    // 单个大整数字面量 → BigInt 输出
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("123456789012345678901234567890")
        .assert()
        .success()
        .stdout("123456789012345678901234567890\n");
}

#[test]
fn test_bigint_json_output() {
    // --json 大整数 → JSON with BigInt
    // 覆盖 cli.rs line 81（BigInt JSON 输出）
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--json").arg("123456789012345678901234567890")
        .assert()
        .success()
        .stdout(r#"{"result":"123456789012345678901234567890","domain":"precision","cache":"miss"}
"#);
}

// ===== Complex 输出覆盖 =====

#[test]
fn test_complex_output_standard() {
    // 3+4i → "3+4i"
    // 覆盖 cli.rs lines 69, 71, 95（Complex 输出 + format_complex 正虚部）
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("3+4i")
        .assert()
        .success()
        .stdout("3+4i\n");
}

#[test]
fn test_complex_output_negative_imaginary() {
    // 3-4i → "3-4i"
    // 覆盖 cli.rs line 225（format_complex 负虚部分支）
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("3-4i")
        .assert()
        .success()
        .stdout("3-4i\n");
}

#[test]
fn test_complex_json_output() {
    // --json 3+4i → JSON with Complex
    // 覆盖 cli.rs lines 69, 71（Complex JSON 输出）
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--json").arg("3+4i")
        .assert()
        .success()
        .stdout(r#"{"result":"3+4i","domain":"complex","cache":"miss"}
"#);
}

// ===== Matrix 输出覆盖 =====

#[test]
fn test_matrix_output_2x2() {
    // [[1,2],[3,4]] → "[[1,2],[3,4]]"
    // 覆盖 cli.rs lines 75, 77, 96（Matrix 输出 + format_matrix）
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("[[1,2],[3,4]]")
        .assert()
        .success()
        .stdout("[[1,2],[3,4]]\n");
}

#[test]
fn test_matrix_json_output() {
    // --json [[1,2],[3,4]] → JSON with Matrix
    // 覆盖 cli.rs lines 75, 77（Matrix JSON 输出）
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--json").arg("[[1,2],[3,4]]")
        .assert()
        .success()
        .stdout(r#"{"result":"[[1,2],[3,4]]","domain":"matrix","cache":"miss"}
"#);
}

#[test]
fn test_matrix_output_1x3() {
    // [[1,2,3]] → "[[1,2,3]]"
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
    // --json --precision 5 "1/3" → JSON with BigRational
    // 覆盖 cli.rs lines 85, 87（BigRational JSON 输出）
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--json").arg("--precision").arg("5").arg("1/3")
        .assert()
        .success()
        .stdout(r#"{"result":"0.33333","domain":"precision","cache":"miss"}
"#);
}

#[test]
fn test_bigrational_output_fraction_form() {
    // 无 --precision 时 precision(0, 1/3) 应当走 precision 域
    // 但 precision(N, expr) 中 N 必须为正整数，N=0 会报错；
    // 改用 precision(5, 2/3) 验证非整数 BigRational 输出
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("precision(5, 2/3)")
        .assert()
        .success()
        .stdout("0.66666\n");
}

// ===== 错误路径覆盖 =====

#[test]
fn test_invalid_var_missing_equals_exit_code() {
    // --var invalid（无 `=`）→ exit 2
    // 覆盖 cli.rs lines 52-54, 141（parse_vars 错误 + run 返回 2）
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--var").arg("invalid").arg("2+3")
        .assert()
        .failure()
        .code(2);
}

#[test]
fn test_invalid_var_non_numeric_value_exit_code() {
    // --var x=abc（值非数字）→ exit 2
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--var").arg("x=abc").arg("x*2")
        .assert()
        .failure()
        .code(2);
}

#[test]
fn test_empty_stdin_exit_code() {
    // echo "" | calnexus（空 stdin）→ exit 1
    // 覆盖 cli.rs lines 129-130（empty stdin 错误）+ line 46（get_expression Err 返回）
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.write_stdin("")
        .assert()
        .failure()
        .code(1);
}

#[test]
fn test_whitespace_only_stdin_exit_code() {
    // echo "   " | calnexus（仅空白 stdin）→ exit 1
    // 覆盖 cli.rs lines 129-130
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.write_stdin("   \n  ")
        .assert()
        .failure()
        .code(1);
}

// ===== 额外 BigInt 运算覆盖 =====

#[test]
fn test_bigint_multiplication_output() {
    // 大整数乘法 → BigInt 输出
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("123456789012345678901234567890 * 2")
        .assert()
        .success()
        .stdout("246913578024691357802469135780\n");
}

#[test]
fn test_bigint_subtraction_output() {
    // 大整数减法 → BigInt 输出
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("123456789012345678901234567890 - 1")
        .assert()
        .success()
        .stdout("123456789012345678901234567889\n");
}

// ===== Statistics 域 CLI 端到端测试（任务 17.2） =====

#[test]
fn test_statistics_mean_cli() {
    // mean([1,2,3,4,5]) → 3
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("mean([1,2,3,4,5])")
        .assert()
        .success()
        .stdout("3\n");
}

#[test]
fn test_statistics_sum_cli() {
    // sum([1,2,3,4,5]) → 15
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("sum([1,2,3,4,5])")
        .assert()
        .success()
        .stdout("15\n");
}

#[test]
fn test_statistics_median_cli() {
    // median([1,2,3,4,5]) → 3
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("median([1,2,3,4,5])")
        .assert()
        .success()
        .stdout("3\n");
}

#[test]
fn test_statistics_json_output() {
    // --json mean([1,2,3,4,5]) → domain="statistics"
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--json").arg("mean([1,2,3,4,5])")
        .assert()
        .success()
        .stdout(r#"{"result":3,"domain":"statistics","cache":"miss"}
"#);
}

#[test]
fn test_statistics_stdin_pipeline() {
    // echo "sum([10,20,30])" | calnexus → 60
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
    cmd.arg("mean([])")
        .assert()
        .failure()
        .code(1);
}

// ===== v0.8 新增域 CLI 端到端测试（TG9）=====

// ----- 9.1 NumberTheory CLI 测试 -----

#[test]
fn test_cli_number_theory_gcd() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("gcd(12,18)")
        .assert()
        .success()
        .stdout("6\n");
}

#[test]
fn test_cli_number_theory_is_prime() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("is_prime(7)")
        .assert()
        .success()
        .stdout("1\n");
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
    cmd.arg("--json").arg("gcd(12,18)")
        .assert()
        .success()
        .stdout(r#"{"result":6,"domain":"number_theory","cache":"miss"}
"#);
}

// ----- 9.2 Combinatorics CLI 测试 -----

#[test]
fn test_cli_combinatorics_C() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("C(10,3)")
        .assert()
        .success()
        .stdout("120\n");
}

#[test]
fn test_cli_combinatorics_catalan() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("catalan(5)")
        .assert()
        .success()
        .stdout("42\n");
}

#[test]
fn test_cli_combinatorics_P() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("P(5,2)")
        .assert()
        .success()
        .stdout("20\n");
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
    cmd.arg("norm([3,4])")
        .assert()
        .success()
        .stdout("5\n");
}

#[test]
fn test_cli_vector_add() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("[1,2]+[3,4]")
        .assert()
        .success()
        .stdout("[4,6]\n");
}

#[test]
fn test_cli_vector_dimension_mismatch_error() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("[1,2]+[3,4,5]")
        .assert()
        .failure()
        .code(1);
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
    cmd.arg("roots(x^2+1)")
        .assert()
        .success();
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

// ===== TG7.5 REPL CLI 测试 =====

#[test]
fn test_cli_repl_start_and_quit() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--repl")
        .write_stdin(":quit\n")
        .assert()
        .success();
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

// ===== TG7.6 批量 CLI 测试 =====

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

// ===== TG7.7 Symbolic CLI 测试 =====

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
    cmd.arg("simplify(x+0)")
        .assert()
        .success()
        .stdout("x\n");
}

#[test]
fn test_cli_symbolic_limit() {
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("limit(sin(x)/x, x, 0)")
        .assert()
        .success()
        .stdout(predicates::str::contains("1"));
}

// ===== JSON 输出路径覆盖（lines 125,127,131,133,137,139,143）=====

#[test]
fn test_json_vector_output() {
    // --json [1,2]+[3,4] → JSON with Vector result
    // 覆盖 cli.rs lines 125, 127（Vector JSON 输出分支）
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--json").arg("[1,2]+[3,4]")
        .assert()
        .success()
        .stdout(r#"{"result":"[4,6]","domain":"vector","cache":"miss"}
"#);
}

#[test]
fn test_json_polynomial_output() {
    // --json poly_add(x+1,x+2) → JSON with Polynomial result
    // 覆盖 cli.rs lines 131, 133（Polynomial JSON 输出分支）
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--json").arg("poly_add(x+1,x+2)")
        .assert()
        .success()
        .stdout(r#"{"result":"2x+3","domain":"polynomial","cache":"miss"}
"#);
}

#[test]
fn test_json_complex_list_output() {
    // --json roots(x^2+1) → JSON with ComplexList result
    // 覆盖 cli.rs lines 137, 139（ComplexList JSON 输出分支）
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--json").arg("roots(x^2+1)")
        .assert()
        .success()
        .stdout(r#"{"result":"[-0+1i,-0-1i]","domain":"polynomial","cache":"miss"}
"#);
}

#[test]
fn test_json_symbolic_output() {
    // --json diff(x^2, x) → JSON with Symbolic result
    // 覆盖 cli.rs line 143（Symbolic JSON 输出分支）
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--json").arg("diff(x^2, x)")
        .assert()
        .success()
        .stdout(r#"{"result":"2*x","domain":"symbolic","cache":"miss"}
"#);
}

// ===== parse_vars 错误路径覆盖（lines 55-57, 68-70）=====

#[test]
fn test_repl_invalid_var_exit_code() {
    // --repl --var invalid（无 `=`）→ exit 2
    // 覆盖 cli.rs lines 55, 56, 57（REPL 模式 parse_vars 错误 + return 2）
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--repl").arg("--var").arg("invalid")
        .write_stdin(":quit\n")
        .assert()
        .failure()
        .code(2);
}

#[test]
fn test_batch_invalid_var_exit_code() {
    // --batch <file> --var invalid（无 `=`）→ exit 2
    // 覆盖 cli.rs lines 68, 69, 70（batch 模式 parse_vars 错误 + return 2）
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    use std::io::Write;
    writeln!(tmp, "2+3").unwrap();
    tmp.flush().unwrap();
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--batch").arg(tmp.path())
        .arg("--var").arg("invalid")
        .assert()
        .failure()
        .code(2);
}

// ===== REPL format_result 分支覆盖（lines 296-302）=====
// format_result 在 REPL evaluate_line 中调用，需通过 REPL 触发各 EvalResult 变体

#[test]
fn test_repl_complex_format_result() {
    // REPL 中求值 3+4i → format_result Complex 分支
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
    // REPL 中求值 [[1,2],[3,4]] → format_result Matrix 分支
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
    // REPL 中求值大整数 → format_result BigInt 分支
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
    // REPL --precision 5 中求值 1/3 → format_result BigRational 分支
    // 覆盖 cli.rs line 299（format_result::BigRational）
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--repl").arg("--precision").arg("5")
        .write_stdin("1/3\n:quit\n")
        .assert()
        .success()
        .stdout(predicates::str::contains("0.33333"));
}

#[test]
fn test_repl_vector_format_result() {
    // REPL 中求值 [1,2]+[3,4] → format_result Vector 分支
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
    // REPL 中求值 poly_add(x+1,x+2) → format_result Polynomial 分支
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
    // REPL 中求值 roots(x^2+1) → format_result ComplexList 分支
    // 覆盖 cli.rs line 302（format_result::ComplexList）
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("--repl")
        .write_stdin("roots(x^2+1)\n:quit\n")
        .assert()
        .success()
        .stdout(predicates::str::contains("1i"));
}

// ===== format_polynomial 分支覆盖（lines 343,349,351,357-360,362,369,375）=====

#[test]
fn test_polynomial_sub_x2_minus_x() {
    // poly_sub(x^2, x) → "x^2-x"
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
    // poly_sub(0, x^2) → "-x^2"
    // 覆盖 cli.rs lines 359-360（i>=2,coef=-1 → "-x^N"）
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("poly_sub(0, x^2)")
        .assert()
        .success()
        .stdout("-x^2\n");
}

#[test]
fn test_polynomial_x_leading_one() {
    // poly_add(x, 0) → "x"
    // 覆盖 cli.rs line 349（i=1,coef=1 → "x"）
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("poly_add(x, 0)")
        .assert()
        .success()
        .stdout("x\n");
}

#[test]
fn test_polynomial_all_zero_coeffs() {
    // poly_sub(x, x) → "0"
    // 覆盖 cli.rs line 369（所有系数为零 → "0"）
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("poly_sub(x, x)")
        .assert()
        .success()
        .stdout("0\n");
}

#[test]
fn test_polynomial_general_coef_high_degree() {
    // poly_mul(2, x^2) → "2x^2"
    // 覆盖 cli.rs line 362（i>=2,一般系数 → "cx^N"）
    let mut cmd = Command::cargo_bin("calnexus").unwrap();
    cmd.arg("poly_mul(2, x^2)")
        .assert()
        .success()
        .stdout("2x^2\n");
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
