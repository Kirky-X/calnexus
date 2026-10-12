// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! 全量函数名表（单一事实源）：供 REPL 补全、CLI `--list-functions`、MCP `list_functions` 共用。

/// 各域函数目录：`(域名, 函数名列表)`；域名与 `DomainRouter::domain_name()` 一致。
/// 列表逐域派生自各域路由白名单常量（单一事实源，新增函数只改白名单），防目录漂移。
pub const DOMAIN_FUNCTIONS: &[(&str, &[&str])] = &[
    ("arithmetic", crate::domains::ARITHMETIC_FUNCTIONS),
    ("scientific", crate::domains::SCIENTIFIC_FUNCTIONS),
    ("statistics", crate::domains::STATISTICS_FUNCTIONS),
    ("precision", &["precision"]),
    ("number_theory", crate::domains::NUMBER_THEORY_FUNCTIONS),
    ("combinatorics", crate::domains::COMBINATORICS_FUNCTIONS),
    ("polynomial", crate::domains::POLYNOMIAL_FUNCTIONS),
    ("complex", crate::domains::COMPLEX_FUNCTIONS),
    ("matrix", crate::domains::MATRIX_FUNCTIONS),
    ("vector", crate::domains::VECTOR_FUNCTIONS),
    ("symbolic", crate::domains::SYMBOLIC_FUNCTIONS),
    // ---- 可选域（feature 门控） ----
    #[cfg(feature = "time")]
    ("time", crate::domains::TIME_FUNCTIONS),
    #[cfg(feature = "unit")]
    ("unit", crate::domains::UNIT_FUNCTIONS),
    #[cfg(feature = "fx")]
    ("fx", crate::domains::FX_FUNCTIONS),
    // `numerical` feature（matrix 域 numerical 短路函数；matrix_exp 仅经 trait API 暴露）
    #[cfg(feature = "numerical")]
    ("numerical", crate::domains::NUMERICAL_FUNCTIONS),
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
            "precision",
            "number_theory",
            "combinatorics",
            "polynomial",
            "complex",
            "matrix",
            "vector",
            "symbolic",
        ] {
            assert!(domains.contains(&required), "目录应含已注册域 {}", required);
        }
    }

    /// 目录与各域路由白名单逐域对账：幻影项（目录有、路由无）与漏列（路由有、目录无）都在此暴露。
    #[test]
    fn test_catalog_matches_routing_whitelists() {
        for (domain, funcs) in DOMAIN_FUNCTIONS {
            let expected: &[&str] = match *domain {
                "arithmetic" => crate::domains::ARITHMETIC_FUNCTIONS,
                "scientific" => crate::domains::SCIENTIFIC_FUNCTIONS,
                "statistics" => crate::domains::STATISTICS_FUNCTIONS,
                "number_theory" => crate::domains::NUMBER_THEORY_FUNCTIONS,
                "combinatorics" => crate::domains::COMBINATORICS_FUNCTIONS,
                "polynomial" => crate::domains::POLYNOMIAL_FUNCTIONS,
                "complex" => crate::domains::COMPLEX_FUNCTIONS,
                "matrix" => crate::domains::MATRIX_FUNCTIONS,
                "vector" => crate::domains::VECTOR_FUNCTIONS,
                "symbolic" => crate::domains::SYMBOLIC_FUNCTIONS,
                #[cfg(feature = "time")]
                "time" => crate::domains::TIME_FUNCTIONS,
                #[cfg(feature = "unit")]
                "unit" => crate::domains::UNIT_FUNCTIONS,
                #[cfg(feature = "fx")]
                "fx" => crate::domains::FX_FUNCTIONS,
                #[cfg(feature = "numerical")]
                "numerical" => crate::domains::NUMERICAL_FUNCTIONS,
                // precision 域无白名单常量，目录即事实源
                "precision" => continue,
                other => panic!("目录出现未知域 {}，请同步 factory 注册与白名单", other),
            };
            assert_eq!(*funcs, expected, "{} 目录与路由白名单漂移", domain);
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
