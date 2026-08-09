// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! 单位换算表：静态系数量 + 温度仿射函数。
//!
//! 设计依据：design.md D5（8 量纲 SI 基准系数表 + 温度仿射特例）
//! Feature 门控：`unit = []`
//!
//! 量纲与 SI 基准：
//! - Length(m)、Mass(kg)、Temperature(K)、Volume(m³)、Area(m²)、Speed(m/s)、Data(byte)、Time(s)
//! - 温度走仿射路径（C/F/K/R），不入线性系数表
//!
//! 单位名大小写敏感（`mB` 与 `MB` 语义不同）。常见别名覆盖小写形式（如 `kilometer`→`km`）。

/// 量纲枚举：8 个物理量纲。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dimension {
    Length,
    Mass,
    Temperature,
    Volume,
    Area,
    Speed,
    Data,
    Time,
}

/// 静态线性系数量：单位名/别名 → (量纲, 到 SI 基准的系数)。
///
/// 温度（C/F/K/R）不在此表中，走 `to_kelvin`/`from_kelvin` 仿射路径。
/// 单位名大小写敏感（如 `B`、`KB`、`MB` 与 `mB` 不同）。
const LINEAR_UNITS: &[(&str, Dimension, f64)] = &[
    // ===== Length (基准: 1 m) =====
    ("m", Dimension::Length, 1.0),
    ("meter", Dimension::Length, 1.0),
    ("metre", Dimension::Length, 1.0),
    ("km", Dimension::Length, 1000.0),
    ("kilometer", Dimension::Length, 1000.0),
    ("kilometre", Dimension::Length, 1000.0),
    ("cm", Dimension::Length, 0.01),
    ("centimeter", Dimension::Length, 0.01),
    ("mm", Dimension::Length, 0.001),
    ("millimeter", Dimension::Length, 0.001),
    ("dm", Dimension::Length, 0.1),
    ("decimeter", Dimension::Length, 0.1),
    ("in", Dimension::Length, 0.0254),
    ("inch", Dimension::Length, 0.0254),
    ("ft", Dimension::Length, 0.3048),
    ("foot", Dimension::Length, 0.3048),
    ("yd", Dimension::Length, 0.9144),
    ("yard", Dimension::Length, 0.9144),
    ("mi", Dimension::Length, 1609.344),
    ("mile", Dimension::Length, 1609.344),
    ("nmi", Dimension::Length, 1852.0),
    ("nautical_mile", Dimension::Length, 1852.0),
    // ===== Mass (基准: 1 kg) =====
    ("kg", Dimension::Mass, 1.0),
    ("kilogram", Dimension::Mass, 1.0),
    ("g", Dimension::Mass, 0.001),
    ("gram", Dimension::Mass, 0.001),
    ("mg", Dimension::Mass, 1e-6),
    ("milligram", Dimension::Mass, 1e-6),
    ("t", Dimension::Mass, 1000.0),
    ("ton", Dimension::Mass, 1000.0),
    ("tonne", Dimension::Mass, 1000.0),
    ("lb", Dimension::Mass, 0.45359237),
    ("lbs", Dimension::Mass, 0.45359237),
    ("pound", Dimension::Mass, 0.45359237),
    ("oz", Dimension::Mass, 0.028349523125),
    ("ounce", Dimension::Mass, 0.028349523125),
    ("stone", Dimension::Mass, 6.35029318),
    // ===== Volume (基准: 1 m³) =====
    ("m3", Dimension::Volume, 1.0),
    ("cubic_meter", Dimension::Volume, 1.0),
    ("L", Dimension::Volume, 0.001),
    ("l", Dimension::Volume, 0.001),
    ("liter", Dimension::Volume, 0.001),
    ("litre", Dimension::Volume, 0.001),
    ("mL", Dimension::Volume, 1e-6),
    ("ml", Dimension::Volume, 1e-6),
    ("milliliter", Dimension::Volume, 1e-6),
    ("gal", Dimension::Volume, 0.003785411784),
    ("gallon", Dimension::Volume, 0.003785411784),
    ("qt", Dimension::Volume, 0.000946352946),
    ("quart", Dimension::Volume, 0.000946352946),
    ("pt", Dimension::Volume, 0.000473176473),
    ("pint", Dimension::Volume, 0.000473176473),
    ("ft3", Dimension::Volume, 0.028316846592),
    ("cubic_foot", Dimension::Volume, 0.028316846592),
    // ===== Area (基准: 1 m²) =====
    ("m2", Dimension::Area, 1.0),
    ("sq_m", Dimension::Area, 1.0),
    ("square_meter", Dimension::Area, 1.0),
    ("km2", Dimension::Area, 1e6),
    ("square_kilometer", Dimension::Area, 1e6),
    ("cm2", Dimension::Area, 1e-4),
    ("square_centimeter", Dimension::Area, 1e-4),
    ("mm2", Dimension::Area, 1e-6),
    ("square_millimeter", Dimension::Area, 1e-6),
    ("ha", Dimension::Area, 10000.0),
    ("hectare", Dimension::Area, 10000.0),
    ("acre", Dimension::Area, 4046.8564224),
    ("ft2", Dimension::Area, 0.09290304),
    ("square_foot", Dimension::Area, 0.09290304),
    ("in2", Dimension::Area, 0.00064516),
    ("square_inch", Dimension::Area, 0.00064516),
    // ===== Speed (基准: 1 m/s) =====
    ("m/s", Dimension::Speed, 1.0),
    ("mps", Dimension::Speed, 1.0),
    ("km/h", Dimension::Speed, 1000.0 / 3600.0),
    ("kmh", Dimension::Speed, 1000.0 / 3600.0),
    ("kph", Dimension::Speed, 1000.0 / 3600.0),
    ("mph", Dimension::Speed, 1609.344 / 3600.0),
    ("knot", Dimension::Speed, 1852.0 / 3600.0),
    ("kn", Dimension::Speed, 1852.0 / 3600.0),
    ("ft/s", Dimension::Speed, 0.3048),
    ("fps", Dimension::Speed, 0.3048),
    // ===== Data (基准: 1 byte) =====
    ("B", Dimension::Data, 1.0),
    ("byte", Dimension::Data, 1.0),
    // 十进制（SI 前缀）
    ("KB", Dimension::Data, 1e3),
    ("MB", Dimension::Data, 1e6),
    ("GB", Dimension::Data, 1e9),
    ("TB", Dimension::Data, 1e12),
    ("PB", Dimension::Data, 1e15),
    ("kilobyte", Dimension::Data, 1e3),
    ("megabyte", Dimension::Data, 1e6),
    ("gigabyte", Dimension::Data, 1e9),
    ("terabyte", Dimension::Data, 1e12),
    // 二进制（IEC 前缀）— 系数预先计算为字面量（f64::powi 非 const fn）
    ("KiB", Dimension::Data, 1024.0),
    ("MiB", Dimension::Data, 1048576.0),          // 1024^2
    ("GiB", Dimension::Data, 1073741824.0),       // 1024^3
    ("TiB", Dimension::Data, 1099511627776.0),    // 1024^4
    ("PiB", Dimension::Data, 1125899906842624.0), // 1024^5
    ("kibibyte", Dimension::Data, 1024.0),
    ("mebibyte", Dimension::Data, 1048576.0),
    ("gibibyte", Dimension::Data, 1073741824.0),
    // ===== Time (基准: 1 s) =====
    ("s", Dimension::Time, 1.0),
    ("sec", Dimension::Time, 1.0),
    ("second", Dimension::Time, 1.0),
    ("ms", Dimension::Time, 1e-3),
    ("millisecond", Dimension::Time, 1e-3),
    ("us", Dimension::Time, 1e-6),
    ("microsecond", Dimension::Time, 1e-6),
    ("ns", Dimension::Time, 1e-9),
    ("nanosecond", Dimension::Time, 1e-9),
    ("min", Dimension::Time, 60.0),
    ("minute", Dimension::Time, 60.0),
    ("h", Dimension::Time, 3600.0),
    ("hr", Dimension::Time, 3600.0),
    ("hour", Dimension::Time, 3600.0),
    ("day", Dimension::Time, 86400.0),
    ("d", Dimension::Time, 86400.0),
    ("week", Dimension::Time, 604800.0),
    ("wk", Dimension::Time, 604800.0),
    ("month", Dimension::Time, 2629800.0), // 平均月（365.25/12 天）
    ("year", Dimension::Time, 31557600.0), // 平均年（365.25 天）
    ("yr", Dimension::Time, 31557600.0),
];

/// 温度单位集合（仿射路径）。
const TEMPERATURE_UNITS: &[&str] = &["C", "F", "K", "R"];

/// 大小写敏感查表：返回线性单位的 (量纲, 到 SI 基准的系数)。
///
/// 温度单位不在表中，返回 None（调用方应用 `is_temperature_unit` 走仿射路径）。
pub fn lookup(name: &str) -> Option<(Dimension, f64)> {
    LINEAR_UNITS
        .iter()
        .find(|(n, _, _)| *n == name)
        .map(|(_, dim, coef)| (*dim, *coef))
}

/// 判断单位名是否为温度单位（C/F/K/R）。
pub fn is_temperature_unit(name: &str) -> Option<bool> {
    // 显式返回 Option 以表达"匹配查询"语义：当 name 是温度单位时返回 Some(true)，
    // 否则返回 Some(false)。调用方通常先用 lookup 判定线性，再用本函数判定温度。
    // 注：保持返回 Option<bool> 便于将来扩展（如未知单位 None）。
    Some(TEMPERATURE_UNITS.contains(&name))
}

/// 将温度值从给定单位转换为开尔文。
///
/// 覆盖：C（摄氏）、F（华氏）、K（开尔文）、R（兰氏度）。
/// 未知单位返回 NaN（调用方应先用 `is_temperature_unit` 校验）。
pub fn to_kelvin(value: f64, unit: &str) -> f64 {
    match unit {
        "C" => value + 273.15,
        "F" => (value - 32.0) * 5.0 / 9.0 + 273.15,
        "K" => value,
        "R" => value * 5.0 / 9.0,
        _ => f64::NAN,
    }
}

/// 将开尔文温度转换为给定单位的值。
///
/// 覆盖：C/F/K/R。未知单位返回 NaN。
pub fn from_kelvin(kelvin: f64, unit: &str) -> f64 {
    match unit {
        "C" => kelvin - 273.15,
        "F" => (kelvin - 273.15) * 9.0 / 5.0 + 32.0,
        "K" => kelvin,
        "R" => kelvin * 9.0 / 5.0,
        _ => f64::NAN,
    }
}

/// Levenshtein 编辑距离：两字符串间的最小单字符插入/删除/替换次数。
///
/// 用于未知单位的 "did you mean" 建议（距离 ≤2 视为相近）。
pub fn levenshtein(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let a_len = a_chars.len();
    let b_len = b_chars.len();

    if a_len == 0 {
        return b_len;
    }
    if b_len == 0 {
        return a_len;
    }

    // 滚动数组优化空间至 O(b_len)
    let mut prev: Vec<usize> = (0..=b_len).collect();
    let mut curr: Vec<usize> = vec![0; b_len + 1];

    for i in 1..=a_len {
        curr[0] = i;
        for j in 1..=b_len {
            let cost = if a_chars[i - 1] == b_chars[j - 1] {
                0
            } else {
                1
            };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    prev[b_len]
}

/// 收集所有已注册的线性单位名（用于 "did you mean" 建议候选集）。
///
/// 不含温度单位（温度单位数量固定，建议直接枚举）。
pub fn all_unit_names() -> Vec<&'static str> {
    LINEAR_UNITS.iter().map(|(n, _, _)| *n).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== T018 验收：长度换算 =====

    #[test]
    fn test_length_m_to_km() {
        // 1000 m = 1 km：from_coeff=1, to_coeff=1000 → 1000 * 1 / 1000 = 1
        let (from_dim, from_c) = lookup("m").unwrap();
        let (to_dim, to_c) = lookup("km").unwrap();
        assert_eq!(from_dim, Dimension::Length);
        assert_eq!(to_dim, Dimension::Length);
        let result = 1000.0 * from_c / to_c;
        assert!((result - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_length_in_to_cm() {
        // 1 in = 2.54 cm：1 * 0.0254 / 0.01 = 2.54
        let (_, from_c) = lookup("in").unwrap();
        let (_, to_c) = lookup("cm").unwrap();
        let result = 1.0 * from_c / to_c;
        assert!((result - 2.54).abs() < 1e-9);
    }

    #[test]
    fn test_length_aliases() {
        // 别名等价性
        assert_eq!(lookup("meter"), lookup("m"));
        assert_eq!(lookup("kilometer"), lookup("km"));
        assert_eq!(lookup("inch"), lookup("in"));
        assert_eq!(lookup("foot"), lookup("ft"));
    }

    #[test]
    fn test_length_case_sensitive() {
        // mB vs MB 是不同单位
        assert!(lookup("mB").is_none());
        assert!(lookup("MB").is_some());
    }

    // ===== T018 验收：质量换算 =====

    #[test]
    fn test_mass_kg_to_lb() {
        // 1 kg ≈ 2.204623 lb
        let (_, from_c) = lookup("kg").unwrap();
        let (_, to_c) = lookup("lb").unwrap();
        let result = 1.0 * from_c / to_c;
        assert!((result - 2.204623).abs() < 1e-6);
    }

    #[test]
    fn test_mass_t_to_kg() {
        // 1 t = 1000 kg：1 * 1000 / 1 = 1000
        let (_, from_c) = lookup("t").unwrap();
        let (_, to_c) = lookup("kg").unwrap();
        let result = 1.0 * from_c / to_c;
        assert!((result - 1000.0).abs() < 1e-9);
    }

    // ===== T018 验收：温度仿射换算 =====

    #[test]
    fn test_temperature_c_to_f_100() {
        // 100°C = 212°F
        let k = to_kelvin(100.0, "C");
        let f = from_kelvin(k, "F");
        assert!((f - 212.0).abs() < 1e-9);
    }

    #[test]
    fn test_temperature_c_to_f_0() {
        // 0°C = 32°F
        let k = to_kelvin(0.0, "C");
        let f = from_kelvin(k, "F");
        assert!((f - 32.0).abs() < 1e-9);
    }

    #[test]
    fn test_temperature_k_to_c() {
        // 0K = -273.15°C
        let c = from_kelvin(0.0, "C");
        assert!((c - (-273.15)).abs() < 1e-9);
    }

    #[test]
    fn test_temperature_r_to_k() {
        // 0°R = 0K
        let k = to_kelvin(0.0, "R");
        assert!((k - 0.0).abs() < 1e-9);
    }

    #[test]
    fn test_temperature_k_identity() {
        // K → K 恒等
        assert!((to_kelvin(42.0, "K") - 42.0).abs() < 1e-9);
        assert!((from_kelvin(42.0, "K") - 42.0).abs() < 1e-9);
    }

    #[test]
    fn test_temperature_unknown_unit_nan() {
        assert!(to_kelvin(100.0, "X").is_nan());
        assert!(from_kelvin(100.0, "X").is_nan());
    }

    #[test]
    fn test_is_temperature_unit_known() {
        assert_eq!(is_temperature_unit("C"), Some(true));
        assert_eq!(is_temperature_unit("F"), Some(true));
        assert_eq!(is_temperature_unit("K"), Some(true));
        assert_eq!(is_temperature_unit("R"), Some(true));
    }

    #[test]
    fn test_is_temperature_unit_not_temperature() {
        assert_eq!(is_temperature_unit("m"), Some(false));
        assert_eq!(is_temperature_unit("kg"), Some(false));
    }

    #[test]
    fn test_temperature_not_in_linear_table() {
        // 温度单位不应在 LINEAR_UNITS 中
        assert!(lookup("C").is_none());
        assert!(lookup("F").is_none());
        assert!(lookup("K").is_none());
        assert!(lookup("R").is_none());
    }

    // ===== T018 验收：体积换算 =====

    #[test]
    fn test_volume_l_to_m3() {
        // 1 L = 0.001 m³
        let (_, from_c) = lookup("L").unwrap();
        let (_, to_c) = lookup("m3").unwrap();
        let result = 1.0 * from_c / to_c;
        assert!((result - 0.001).abs() < 1e-12);
    }

    #[test]
    fn test_volume_gal_to_l() {
        // 1 gal(US) ≈ 3.785412 L：1 * 0.003785411784 / 0.001 ≈ 3.785412
        let (_, from_c) = lookup("gal").unwrap();
        let (_, to_c) = lookup("L").unwrap();
        let result = 1.0 * from_c / to_c;
        assert!((result - 3.785412).abs() < 1e-5);
    }

    // ===== T018 验收：面积换算 =====

    #[test]
    fn test_area_ha_to_m2() {
        // 1 ha = 10000 m²
        let (_, from_c) = lookup("ha").unwrap();
        let (_, to_c) = lookup("m2").unwrap();
        let result = 1.0 * from_c / to_c;
        assert!((result - 10000.0).abs() < 1e-9);
    }

    #[test]
    fn test_area_km2_to_m2() {
        let (_, from_c) = lookup("km2").unwrap();
        let (_, to_c) = lookup("m2").unwrap();
        let result = 1.0 * from_c / to_c;
        assert!((result - 1e6).abs() < 1e-3);
    }

    // ===== T018 验收：速度换算 =====

    #[test]
    fn test_speed_mps_to_kmh() {
        // 1 m/s = 3.6 km/h
        let (_, from_c) = lookup("m/s").unwrap();
        let (_, to_c) = lookup("km/h").unwrap();
        let result = 1.0 * from_c / to_c;
        assert!((result - 3.6).abs() < 1e-9);
    }

    #[test]
    fn test_speed_kmh_to_mps() {
        // 反向：3.6 km/h = 1 m/s
        let (_, from_c) = lookup("km/h").unwrap();
        let (_, to_c) = lookup("m/s").unwrap();
        let result = 3.6 * from_c / to_c;
        assert!((result - 1.0).abs() < 1e-9);
    }

    // ===== T018 验收：数据量换算 =====

    #[test]
    fn test_data_gib_to_b() {
        // 1 GiB = 1073741824 B
        let (_, from_c) = lookup("GiB").unwrap();
        let (_, to_c) = lookup("B").unwrap();
        let result = 1.0 * from_c / to_c;
        assert!((result - 1073741824.0).abs() < 1e-3);
    }

    #[test]
    fn test_data_kb_to_b() {
        // 1 KB = 1000 B（十进制，≠ KiB）
        let (_, from_c) = lookup("KB").unwrap();
        let (_, to_c) = lookup("B").unwrap();
        let result = 1.0 * from_c / to_c;
        assert!((result - 1000.0).abs() < 1e-9);
    }

    #[test]
    fn test_data_kib_vs_kb_distinct() {
        // KiB ≠ KB：1024 vs 1000
        let (_, kib) = lookup("KiB").unwrap();
        let (_, kb) = lookup("KB").unwrap();
        assert!((kib - 1024.0).abs() < 1e-9);
        assert!((kb - 1000.0).abs() < 1e-9);
        assert_ne!(kib, kb);
    }

    // ===== T018 验收：时间换算 =====

    #[test]
    fn test_time_h_to_s() {
        // 1 h = 3600 s
        let (_, from_c) = lookup("h").unwrap();
        let (_, to_c) = lookup("s").unwrap();
        let result = 1.0 * from_c / to_c;
        assert!((result - 3600.0).abs() < 1e-9);
    }

    #[test]
    fn test_time_week_to_s() {
        // 1 week = 604800 s
        let (_, from_c) = lookup("week").unwrap();
        let (_, to_c) = lookup("s").unwrap();
        let result = 1.0 * from_c / to_c;
        assert!((result - 604800.0).abs() < 1e-9);
    }

    #[test]
    fn test_time_min_to_s() {
        let (_, from_c) = lookup("min").unwrap();
        let result = 1.0 * from_c / 1.0;
        assert!((result - 60.0).abs() < 1e-9);
    }

    #[test]
    fn test_time_day_to_s() {
        let (_, from_c) = lookup("day").unwrap();
        let result = 1.0 * from_c;
        assert!((result - 86400.0).abs() < 1e-9);
    }

    // ===== 同单位换算恒等 =====

    #[test]
    fn test_identity_conversion() {
        let units = ["m", "kg", "L", "m2", "m/s", "B", "s"];
        for u in units {
            let (dim, c) = lookup(u).unwrap();
            let result = 42.0 * c / c;
            assert!(
                (result - 42.0).abs() < 1e-9,
                "identity failed for {} ({:?})",
                u,
                dim
            );
        }
    }

    // ===== 未知单位 =====

    #[test]
    fn test_lookup_unknown() {
        assert!(lookup("metre_xyz").is_none());
        assert!(lookup("").is_none());
        assert!(lookup("XYZ").is_none());
    }

    // ===== Levenshtein 距离 =====

    #[test]
    fn test_levenshtein_identical() {
        assert_eq!(levenshtein("meter", "meter"), 0);
    }

    #[test]
    fn test_levenshtein_empty() {
        assert_eq!(levenshtein("", "abc"), 3);
        assert_eq!(levenshtein("abc", ""), 3);
        assert_eq!(levenshtein("", ""), 0);
    }

    #[test]
    fn test_levenshtein_one_edit() {
        // 替换：meter → metor（替换 1 字符）
        assert_eq!(levenshtein("meter", "metor"), 1);
        // 插入：meter → meters
        assert_eq!(levenshtein("meter", "meters"), 1);
        // 删除：meters → meter
        assert_eq!(levenshtein("meters", "meter"), 1);
    }

    #[test]
    fn test_levenshtein_two_edits() {
        // meter → metre 需 2 次替换（e↔r 交换位置，Levenshtein 不支持转置）
        assert_eq!(levenshtein("meter", "metre"), 2);
        // metre → meters 需 1 次替换 + 1 次插入 = 2
        assert_eq!(levenshtein("metre", "meters"), 2);
    }

    #[test]
    fn test_levenshtein_case_sensitive() {
        // 大小写敏感：MB vs mB 距离 1
        assert_eq!(levenshtein("MB", "mB"), 1);
    }

    // ===== all_unit_names =====

    #[test]
    fn test_all_unit_names_contains_common() {
        let names = all_unit_names();
        assert!(names.contains(&"m"));
        assert!(names.contains(&"kg"));
        assert!(names.contains(&"s"));
        assert!(names.contains(&"B"));
        assert!(names.contains(&"km"));
    }

    #[test]
    fn test_all_unit_names_excludes_temperature() {
        let names = all_unit_names();
        // 温度单位不在 LINEAR_UNITS 中
        assert!(!names.contains(&"C"));
        assert!(!names.contains(&"F"));
        assert!(!names.contains(&"K"));
        assert!(!names.contains(&"R"));
    }

    // ===== 覆盖率补充：Dimension 枚举可调试 =====

    #[test]
    fn test_dimension_debug() {
        let s = format!("{:?}", Dimension::Length);
        assert_eq!(s, "Length");
        let s = format!("{:?}", Dimension::Temperature);
        assert_eq!(s, "Temperature");
    }

    #[test]
    fn test_dimension_eq_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(Dimension::Length);
        assert!(set.contains(&Dimension::Length));
        assert!(!set.contains(&Dimension::Mass));
    }
}
