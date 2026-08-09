// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! 单位换算核心数学函数。
//!
//! 设计依据：design.md D5（8 量纲 SI 基准系数表 + 温度仿射特例）
//! Feature 门控：`unit = []`
//!
//! 从 `domains/unit.rs` 提取的纯函数：单位换算逻辑 + 错误构造。
//! 依赖 `math::unit_table` 提供查表/温度仿射/编辑距离。

use crate::core::CalcError;

use super::unit_table::{
    all_unit_names, from_kelvin, is_temperature_unit, levenshtein, lookup, to_kelvin, Dimension,
};

/// 温度单位候选集（用于 "did you mean" 建议）。
const TEMPERATURE_UNIT_NAMES: &[&str] = &["C", "F", "K", "R"];

/// 单位换算核心逻辑：分流线性路径与温度仿射路径。
///
/// 返回换算后的 f64 值。错误类型：
/// - 未知单位（含 Levenshtein ≤2 的相近建议）
/// - 量纲不匹配（消息含双方量纲名）
pub fn convert_value(value: f64, from: &str, to: &str) -> Result<f64, CalcError> {
    // 拒绝 NaN/Inf 输入，防止静默传播
    if !value.is_finite() {
        return Err(CalcError::domain(format!(
            "convert_value requires finite value, got {}",
            value
        )));
    }
    let from_is_temp = is_temperature_unit(from).unwrap_or(false);
    let to_is_temp = is_temperature_unit(to).unwrap_or(false);

    // 路径 1：from 和 to 均为温度 → 仿射换算
    if from_is_temp && to_is_temp {
        let kelvin = to_kelvin(value, from);
        return Ok(from_kelvin(kelvin, to));
    }

    // 路径 2：仅一个为温度 → 量纲不匹配
    if from_is_temp || to_is_temp {
        let from_dim = if from_is_temp {
            Dimension::Temperature
        } else {
            match lookup(from) {
                Some((d, _)) => d,
                None => return Err(unknown_unit_error(from)),
            }
        };
        let to_dim = if to_is_temp {
            Dimension::Temperature
        } else {
            match lookup(to) {
                Some((d, _)) => d,
                None => return Err(unknown_unit_error(to)),
            }
        };
        return Err(dimension_mismatch_error(from, from_dim, to, to_dim));
    }

    // 路径 3：均为线性单位 → 系数换算
    let (from_dim, from_coeff) = match lookup(from) {
        Some(x) => x,
        None => return Err(unknown_unit_error(from)),
    };
    let (to_dim, to_coeff) = match lookup(to) {
        Some(x) => x,
        None => return Err(unknown_unit_error(to)),
    };

    if from_dim != to_dim {
        return Err(dimension_mismatch_error(from, from_dim, to, to_dim));
    }

    Ok(value * from_coeff / to_coeff)
}

/// 构造"未知单位"错误，附带 Levenshtein 距离 ≤2 的相近单位建议。
///
/// 候选集 = 全部线性单位名 + 温度单位名（C/F/K/R）。
pub fn unknown_unit_error(unit: &str) -> CalcError {
    let linear = all_unit_names();
    let mut suggestions: Vec<&str> = linear
        .iter()
        .copied()
        .filter(|c| {
            let d = levenshtein(unit, c);
            d > 0 && d <= 2
        })
        .collect();
    for c in TEMPERATURE_UNIT_NAMES {
        let d = levenshtein(unit, c);
        if d > 0 && d <= 2 {
            suggestions.push(c);
        }
    }

    let detail = if suggestions.is_empty() {
        format!("unknown unit: {}", unit)
    } else {
        format!(
            "unknown unit: {}, did you mean: {}",
            unit,
            suggestions.join(", ")
        )
    };

    CalcError::domain(detail).with_i18n(
        "msg.unit.unknown_unit",
        vec![("unit".to_string(), unit.to_string())],
    )
}

/// 构造"量纲不匹配"错误，消息含双方量纲名。
pub fn dimension_mismatch_error(
    from: &str,
    from_dim: Dimension,
    to: &str,
    to_dim: Dimension,
) -> CalcError {
    CalcError::domain(format!(
        "dimension mismatch: {} is {:?}, {} is {:?}",
        from, from_dim, to, to_dim
    ))
    .with_i18n(
        "msg.unit.dimension_mismatch",
        vec![
            ("from".to_string(), from.to_string()),
            ("from_dim".to_string(), format!("{:?}", from_dim)),
            ("to".to_string(), to.to_string()),
            ("to_dim".to_string(), format!("{:?}", to_dim)),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ErrorKind;

    // ===== 线性单位换算 =====

    #[test]
    fn test_convert_value_linear_m_to_km() {
        let r = convert_value(1000.0, "m", "km").unwrap();
        assert!((r - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_convert_value_linear_in_to_cm() {
        let r = convert_value(1.0, "in", "cm").unwrap();
        assert!((r - 2.54).abs() < 1e-9);
    }

    #[test]
    fn test_convert_value_linear_kg_to_lb() {
        let r = convert_value(1.0, "kg", "lb").unwrap();
        assert!((r - 2.204623).abs() < 1e-6);
    }

    #[test]
    fn test_convert_value_linear_identity() {
        let r = convert_value(42.0, "m", "m").unwrap();
        assert!((r - 42.0).abs() < 1e-9);
    }

    #[test]
    fn test_convert_value_volume_l_to_m3() {
        let r = convert_value(1.0, "L", "m3").unwrap();
        assert!((r - 0.001).abs() < 1e-12);
    }

    #[test]
    fn test_convert_value_area_ha_to_m2() {
        let r = convert_value(1.0, "ha", "m2").unwrap();
        assert!((r - 10000.0).abs() < 1e-9);
    }

    #[test]
    fn test_convert_value_speed_mps_to_kmh() {
        let r = convert_value(1.0, "m/s", "km/h").unwrap();
        assert!((r - 3.6).abs() < 1e-9);
    }

    #[test]
    fn test_convert_value_data_gib_to_b() {
        let r = convert_value(1.0, "GiB", "B").unwrap();
        assert!((r - 1073741824.0).abs() < 1e-3);
    }

    #[test]
    fn test_convert_value_time_h_to_s() {
        let r = convert_value(1.0, "h", "s").unwrap();
        assert!((r - 3600.0).abs() < 1e-9);
    }

    // ===== 温度仿射换算 =====

    #[test]
    fn test_convert_value_temperature_c_to_f_100() {
        let r = convert_value(100.0, "C", "F").unwrap();
        assert!((r - 212.0).abs() < 1e-9);
    }

    #[test]
    fn test_convert_value_temperature_c_to_f_0() {
        let r = convert_value(0.0, "C", "F").unwrap();
        assert!((r - 32.0).abs() < 1e-9);
    }

    #[test]
    fn test_convert_value_temperature_k_to_c() {
        let r = convert_value(0.0, "K", "C").unwrap();
        assert!((r - (-273.15)).abs() < 1e-9);
    }

    #[test]
    fn test_convert_value_temperature_r_to_k() {
        let r = convert_value(0.0, "R", "K").unwrap();
        assert!((r - 0.0).abs() < 1e-9);
    }

    #[test]
    fn test_convert_value_temperature_c_to_c() {
        let r = convert_value(50.0, "C", "C").unwrap();
        assert!((r - 50.0).abs() < 1e-9);
    }

    #[test]
    fn test_convert_value_temperature_k_to_f_roundtrip() {
        // 300K → F：300-273.15=26.85°C → 26.85*9/5+32 = 80.33°F
        let r = convert_value(300.0, "K", "F").unwrap();
        assert!((r - 80.33).abs() < 1e-9);
    }

    // ===== 错误路径 =====

    #[test]
    fn test_convert_value_dimension_mismatch() {
        let err = convert_value(1.0, "m", "kg").expect_err("expected error");
        assert_eq!(err.kind, ErrorKind::Domain);
        assert!(err.message.contains("Length"));
        assert!(err.message.contains("Mass"));
        assert_eq!(err.i18n_key, Some("msg.unit.dimension_mismatch"));
    }

    #[test]
    fn test_convert_value_temperature_to_linear_mismatch() {
        let err = convert_value(100.0, "C", "m").expect_err("expected error");
        assert_eq!(err.kind, ErrorKind::Domain);
        assert!(err.message.contains("Temperature"));
        assert!(err.message.contains("Length"));
    }

    #[test]
    fn test_convert_value_unknown_unit() {
        let err = convert_value(1.0, "XYZ", "m").expect_err("expected error");
        assert_eq!(err.kind, ErrorKind::Domain);
        assert_eq!(err.i18n_key, Some("msg.unit.unknown_unit"));
    }

    #[test]
    fn test_convert_value_unknown_to_unit() {
        let err = convert_value(1.0, "m", "XYZ").expect_err("expected error");
        assert_eq!(err.kind, ErrorKind::Domain);
    }

    // ===== 错误构造函数 =====

    #[test]
    fn test_unknown_unit_error_with_suggestion() {
        let err = unknown_unit_error("metr");
        assert!(err.message.contains("did you mean"));
        assert!(err.message.contains("meter") || err.message.contains("metre"));
    }

    #[test]
    fn test_unknown_unit_error_no_suggestion_for_far_unit() {
        let err = unknown_unit_error("ZZZZZZ");
        assert!(!err.message.contains("did you mean"));
    }

    #[test]
    fn test_dimension_mismatch_error_message() {
        let err = dimension_mismatch_error("m", Dimension::Length, "kg", Dimension::Mass);
        assert_eq!(err.kind, ErrorKind::Domain);
        assert!(err.message.contains("Length"));
        assert!(err.message.contains("Mass"));
        assert_eq!(err.i18n_key, Some("msg.unit.dimension_mismatch"));
        let arg_keys: Vec<&str> = err.i18n_args.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(arg_keys, vec!["from", "from_dim", "to", "to_dim"]);
    }

    #[test]
    fn test_dimension_mismatch_error_time_vs_length() {
        let err = dimension_mismatch_error("m", Dimension::Length, "s", Dimension::Time);
        assert!(err.message.contains("Length"));
        assert!(err.message.contains("Time"));
    }
}
