// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! 函数目录（v015 T038，R-mcp-002）：按域分组的全量函数名静态表。
//!
//! 单一事实源：REPL Tab 补全、CLI `--list-functions`、MCP `list_functions` tool
//! 共用本表。feature 门控域（time/unit/fx/numerical）由消费方按 cfg 过滤。
//!
//! 审计背景：原 REPL KNOWN_FUNCTIONS 缺失 feature 门控函数（convert/fx/now 等），
//! 统一目录后此类漂移不再发生。

/// 各域函数目录：`(域名, 函数名列表)`。
///
/// 域名与 `DomainRouter` 的 `domain_name()` 一致；可选域以 cfg 惯例标注注释。
pub const DOMAIN_FUNCTIONS: &[(&str, &[&str])] = &[
    (
        "arithmetic",
        &["abs", "mod", "factorial"],
    ),
    (
        "scientific",
        &[
            "sin", "cos", "tan", "asin", "acos", "atan", "ln", "log", "exp", "sinh", "cosh",
            "tanh", "gamma", "erf",
        ],
    ),
    (
        "statistics",
        &["mean", "median", "variance", "stddev", "sum", "min", "max"],
    ),
    (
        "precision",
        &["precision"],
    ),
    (
        "number_theory",
        &[
            "gcd", "lcm", "is_prime", "prime_sieve", "mod_inverse", "mod_pow", "euler_phi",
        ],
    ),
    (
        "combinatorics",
        &["P", "C", "catalan", "stirling"],
    ),
    (
        "polynomial",
        &[
            "poly_add", "poly_sub", "poly_mul", "poly_div", "poly_eval", "poly_diff",
            "poly_integrate", "roots", "factor",
        ],
    ),
    (
        "complex",
        &["complex", "re", "im", "conj", "magnitude", "phase"],
    ),
    (
        "matrix",
        &["det", "transpose", "inverse", "trace"],
    ),
    (
        "vector",
        &["dot", "cross", "norm", "angle", "normalize", "scalar_triple"],
    ),
    (
        "symbolic",
        &["diff", "integrate", "simplify", "limit", "taylor"],
    ),
    // ---- 可选域（feature 门控） ----
    // `time` feature
    #[cfg(feature = "time")]
    (
        "time",
        &["now", "today", "date_diff"],
    ),
    // `unit` feature
    #[cfg(feature = "unit")]
    (
        "unit",
        &["convert"],
    ),
    // `fx` feature
    #[cfg(feature = "fx")]
    (
        "fx",
        &["fx", "fx_rate"],
    ),
    // `numerical` feature（经 matrix 域委托）
    #[cfg(feature = "numerical")]
    (
        "numerical",
        &["lu", "qr", "eig", "svd", "solve", "matrix_exp"],
    ),
];

/// 全部函数名扁平列表（REPL Tab 补全用，按域序去重）。
pub fn all_function_names() -> Vec<&'static str> {
    let mut names: Vec<&'static str> = Vec::new();
    for (_, funcs) in DOMAIN_FUNCTIONS {
        for f in *funcs {
            if !names.contains(f) {
                names.push(f);
            }
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_catalog_covers_core_domains() {
        let domains: Vec<&str> = DOMAIN_FUNCTIONS.iter().map(|(d, _)| *d).collect();
        for required in [
            "arithmetic",
            "scientific",
            "statistics",
            "number_theory",
            "matrix",
            "vector",
            "symbolic",
        ] {
            assert!(domains.contains(&required), "目录应含 {} 域", required);
        }
    }

    #[test]
    fn test_all_function_names_deduped_and_nonempty() {
        let names = all_function_names();
        assert!(!names.is_empty());
        let unique: std::collections::HashSet<_> = names.iter().collect();
        assert_eq!(unique.len(), names.len(), "函数名应去重");
    }

    #[cfg(feature = "fx")]
    #[test]
    fn test_fx_domain_in_catalog_when_feature_on() {
        let domains: Vec<&str> = DOMAIN_FUNCTIONS.iter().map(|(d, _)| *d).collect();
        assert!(domains.contains(&"fx"));
        assert!(all_function_names().contains(&"fx"));
    }
}
