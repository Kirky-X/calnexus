// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! 终端绘图输出（Braille 曲线与矩阵热图），仅 `plot` feature 下编译。
//!
//! Braille 字符 U+2800 起，每个字符承载 2 列 × 4 行共 8 个点位；零外部依赖。

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
        let mut cfg = PlotConfig::default();
        cfg.width = 201;
        assert!(cfg.validate().is_err());
        cfg.width = 80;
        cfg.height = 61;
        assert!(cfg.validate().is_err());
        cfg.height = 24;
        cfg.x_min = 5.0;
        cfg.x_max = 1.0;
        assert!(cfg.validate().is_err());
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
}
