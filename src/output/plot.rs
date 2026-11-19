// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! 终端绘图输出（Braille 曲线与矩阵热图），仅 `plot` feature 下编译。
//!
//! Braille 字符 U+2800 起，每个字符承载 2 列 × 4 行共 8 个点位；零外部依赖。
// CLI --plot/--heatmap 分发在 src/cli.rs 消费（run_plot_mode / run_heatmap_mode）。

use crate::core::CalcError;

/// 终端绘图配置：字符网格尺寸与 x/y 显示范围（y 范围 None 时由采样自适应）。
#[derive(Debug, Clone)]
pub struct PlotConfig {
    pub width: usize,
    pub height: usize,
    pub x_min: f64,
    pub x_max: f64,
    pub y_min: Option<f64>,
    pub y_max: Option<f64>,
}

impl Default for PlotConfig {
    fn default() -> Self {
        Self {
            width: 80,
            height: 24,
            x_min: -10.0,
            x_max: 10.0,
            y_min: None,
            y_max: None,
        }
    }
}

impl PlotConfig {
    /// 校验尺寸与范围约束；超限报域错误。
    pub fn validate(&self) -> Result<(), CalcError> {
        if self.width == 0 || self.width > 200 || self.height == 0 || self.height > 60 {
            return Err(CalcError::domain(format!(
                "plot grid out of range: width={} height={}",
                self.width, self.height
            ))
            .with_i18n("msg.plot.invalid_config", vec![]));
        }
        if !self.x_min.is_finite() || !self.x_max.is_finite() || self.x_min >= self.x_max {
            return Err(CalcError::domain(
                "plot x range must be finite and increasing".to_string(),
            )
            .with_i18n("msg.plot.invalid_config", vec![]));
        }
        if let (Some(lo), Some(hi)) = (self.y_min, self.y_max)
            && (!lo.is_finite() || !hi.is_finite() || lo >= hi)
        {
            return Err(CalcError::domain(
                "plot y range must be finite and increasing".to_string(),
            )
            .with_i18n("msg.plot.invalid_config", vec![]));
        }
        Ok(())
    }
}

/// Braille 点位偏移表：[列][行] → U+2800 之上的 bit。
const BRAILLE_BITS: [[u32; 4]; 2] = [
    [0x01, 0x02, 0x04, 0x40], // 左列：上四点 + 下左
    [0x08, 0x10, 0x20, 0x80], // 右列：上右三点 + 最下右
];

/// 把采样点渲染成 Braille 曲线；NaN/Inf 点跳过。返回 `height` 行文本，末行附 y 范围标注。
pub fn render_curve(samples: &[(f64, f64)], cfg: &PlotConfig) -> String {
    if cfg.validate().is_err() {
        return String::new();
    }
    let (y_lo, y_hi) = match (cfg.y_min, cfg.y_max) {
        (Some(lo), Some(hi)) => (lo, hi),
        _ => {
            let finite: Vec<f64> = samples
                .iter()
                .map(|(_, y)| *y)
                .filter(|y| y.is_finite())
                .collect();
            if finite.is_empty() {
                (0.0, 1.0)
            } else {
                let lo = finite.iter().copied().fold(f64::INFINITY, f64::min);
                let hi = finite.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                if (hi - lo).abs() < 1e-15 {
                    (lo - 1.0, hi + 1.0)
                } else {
                    (lo, hi)
                }
            }
        }
    };

    let cols = cfg.width * 2;
    let rows = cfg.height * 4;
    let mut grid = vec![vec![0u32; cfg.width]; cfg.height];
    let x_span = cfg.x_max - cfg.x_min;
    let y_span = y_hi - y_lo;

    for &(x, y) in samples {
        if !x.is_finite() || !y.is_finite() {
            continue;
        }
        let px = (((x - cfg.x_min) / x_span) * (cols as f64 - 1.0)).round();
        let py = (((y_hi - y) / y_span) * (rows as f64 - 1.0)).round();
        if px < 0.0 || py < 0.0 {
            continue;
        }
        let (px, py) = (px as usize, py as usize);
        if px >= cols || py >= rows {
            continue;
        }
        grid[py / 4][px / 2] |= BRAILLE_BITS[px % 2][py % 4];
    }

    let label = format!(
        "y[{y_lo:.3},{y_hi:.3}] x[{:.3},{:.3}]",
        cfg.x_min, cfg.x_max
    );
    let mut lines: Vec<String> = grid
        .iter()
        .map(|row| {
            row.iter()
                .map(|&bits| char::from_u32(0x2800 + bits).unwrap())
                .collect()
        })
        .collect();
    if let Some(last) = lines.last_mut() {
        last.push(' ');
        last.push_str(&label);
    }
    lines.join("\n")
}

/// 强度字符集（由弱到强）；矩阵元素绝对值按最大值归一后取字符。
const HEATMAP_CHARS: [char; 10] = [' ', '.', ':', '-', '=', '+', '*', '#', '%', '@'];

/// 矩阵热图：逐元素归一到强度字符，附行列尺寸标注行。
pub fn render_heatmap(m: &[Vec<f64>]) -> String {
    if m.is_empty() {
        return "0x0".to_string();
    }
    let peak = m.iter().flatten().map(|v| v.abs()).fold(0.0_f64, f64::max);
    let mut lines: Vec<String> = m
        .iter()
        .map(|row| {
            row.iter()
                .map(|v| {
                    if peak == 0.0 || !v.is_finite() {
                        HEATMAP_CHARS[0]
                    } else {
                        let scaled = (v.abs() / peak * 9.0).round() as usize;
                        HEATMAP_CHARS[scaled.min(9)]
                    }
                })
                .collect()
        })
        .collect();
    lines.push(format!("{}x{}", m.len(), m.first().map_or(0, Vec::len)));
    lines.join("\n")
}

/// 采样聚合结果：按字符列压缩后的 (x, y) 点与最终 y 范围。
#[derive(Debug, Clone)]
pub struct AggregatedSamples {
    pub points: Vec<(f64, f64)>,
    pub y_min: f64,
    pub y_max: f64,
}

/// 等距采样按字符列取 y 的 min/max 两点聚合；y 范围缺省时按有效点自适应并留 8% 边距。
/// 全 NaN/Inf 输入报域错误。
pub fn aggregate_samples(
    samples: &[(f64, f64)],
    cfg: &PlotConfig,
) -> Result<AggregatedSamples, CalcError> {
    cfg.validate()?;
    let finite: Vec<(f64, f64)> = samples
        .iter()
        .copied()
        .filter(|(x, y)| x.is_finite() && y.is_finite() && *x >= cfg.x_min && *x <= cfg.x_max)
        .collect();
    if finite.is_empty() {
        return Err(CalcError::domain("no finite samples to plot".to_string())
            .with_i18n("msg.plot.no_valid_samples", vec![]));
    }

    let x_span = cfg.x_max - cfg.x_min;
    let mut columns: Vec<Vec<f64>> = vec![Vec::new(); cfg.width.max(1)];
    for (x, y) in &finite {
        let ratio = (x - cfg.x_min) / x_span;
        let idx = ((ratio * columns.len() as f64).floor() as usize).min(columns.len() - 1);
        columns[idx].push(*y);
    }

    let (mut y_min, mut y_max) = match (cfg.y_min, cfg.y_max) {
        (Some(lo), Some(hi)) => (lo, hi),
        _ => {
            let lo = finite.iter().map(|(_, y)| *y).fold(f64::INFINITY, f64::min);
            let hi = finite
                .iter()
                .map(|(_, y)| *y)
                .fold(f64::NEG_INFINITY, f64::max);
            let pad = if (hi - lo).abs() < 1e-15 {
                1.0
            } else {
                (hi - lo) * 0.08
            };
            (lo - pad, hi + pad)
        }
    };
    if (y_max - y_min).abs() < 1e-15 {
        y_min -= 1.0;
        y_max += 1.0;
    }

    let column_width = x_span / columns.len() as f64;
    let mut points = Vec::new();
    for (idx, values) in columns.iter().enumerate() {
        if values.is_empty() {
            continue;
        }
        let center = cfg.x_min + column_width * (idx as f64 + 0.5);
        let lo = values.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        points.push((center, lo));
        if (hi - lo).abs() > 1e-15 {
            points.push((center, hi));
        }
    }
    Ok(AggregatedSamples {
        points,
        y_min,
        y_max,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_curve_line_count_and_labels() {
        let cfg = PlotConfig {
            width: 20,
            height: 6,
            ..Default::default()
        };
        let samples: Vec<(f64, f64)> = (0..100)
            .map(|i| {
                let x = -10.0 + 20.0 * i as f64 / 99.0;
                (x, x)
            })
            .collect();
        let out = render_curve(&samples, &cfg);
        assert_eq!(out.lines().count(), 6);
        assert!(out.contains("y["), "{out}");
        assert!(
            out.chars().any(|c| ('\u{2801}'..='\u{28ff}').contains(&c)),
            "no braille dots"
        );
    }

    #[test]
    fn test_invalid_config_rejected() {
        assert!(
            PlotConfig {
                width: 201,
                ..Default::default()
            }
            .validate()
            .is_err()
        );
        assert!(
            PlotConfig {
                height: 61,
                ..Default::default()
            }
            .validate()
            .is_err()
        );
        assert!(
            PlotConfig {
                x_min: 5.0,
                x_max: 1.0,
                ..Default::default()
            }
            .validate()
            .is_err()
        );
    }

    #[test]
    fn test_nan_samples_skipped() {
        let cfg = PlotConfig {
            width: 10,
            height: 4,
            ..Default::default()
        };
        let out = render_curve(&[(0.0, f64::NAN), (1.0, f64::INFINITY)], &cfg);
        assert_eq!(out.lines().count(), 4);
        assert!(out.contains("y["));
    }
    #[test]
    fn test_aggregate_sin_adaptive_bounds() {
        let cfg = PlotConfig {
            width: 40,
            height: 12,
            ..Default::default()
        };
        let samples: Vec<(f64, f64)> = (0..=800)
            .map(|i| {
                let x = -10.0 + 20.0 * i as f64 / 800.0;
                (x, x.sin())
            })
            .collect();
        let agg = aggregate_samples(&samples, &cfg).unwrap();
        assert!((agg.y_min + 1.16).abs() < 0.02, "y_min={}", agg.y_min);
        assert!((agg.y_max - 1.16).abs() < 0.02, "y_max={}", agg.y_max);
        assert!(!agg.points.is_empty());
    }

    #[test]
    fn test_aggregate_all_nan_errors() {
        let cfg = PlotConfig::default();
        let samples = [(f64::NAN, f64::NAN), (0.0, f64::INFINITY)];
        assert!(aggregate_samples(&samples, &cfg).is_err());
    }
    #[test]
    fn test_render_heatmap_diagonal_stronger() {
        let m = vec![vec![1.0, 0.0], vec![0.0, 1.0]];
        let out = render_heatmap(&m);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 3);
        assert!(lines[2].contains("2x2"));
        let strong = HEATMAP_CHARS
            .iter()
            .position(|&c| c == lines[0].chars().next().unwrap())
            .unwrap();
        let weak = HEATMAP_CHARS
            .iter()
            .position(|&c| c == lines[0].chars().nth(1).unwrap())
            .unwrap();
        assert!(strong > weak);
    }
}
