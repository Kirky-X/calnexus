// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! 优化算法纯函数层：一维 Brent 极小化与小规模稠密线性规划（两阶段单纯形）。
//!
//! 依赖方向：`math/` → `core/`（仅 types/error），零外部依赖，全部确定性可缓存。

use crate::core::CalcError;

/// 抛物线插值极小点：三点拟合二次曲线的驻点，退化时返回 `None`。
fn parabolic_vertex(x1: f64, f1: f64, x2: f64, f2: f64, x3: f64, f3: f64) -> Option<f64> {
    let d1 = (f2 - f1) / (x2 - x1);
    let d2 = (f3 - f2) / (x3 - x2);
    let quad = (d2 - d1) / (x3 - x1);
    if quad.abs() < 1e-30 {
        return None;
    }
    let linear = d1 - quad * (x1 + x2);
    let vertex = -linear / (2.0 * quad);
    (!vertex.is_nan()).then_some(vertex)
}

/// 一维无约束极小化：Brent 方法（黄金分割保底 + 抛物线插值加速）。
///
/// 返回 `(x*, f(x*))`；区间非法（a ≥ b、非有限）或容差非正返回域错误。
pub fn minimize_1d<F>(
    f: F,
    a: f64,
    b: f64,
    tol: f64,
    max_iter: usize,
) -> Result<(f64, f64), CalcError>
where
    F: Fn(f64) -> f64,
{
    if !a.is_finite() || !b.is_finite() || a >= b {
        return Err(
            CalcError::domain(format!("minimize: invalid interval [{a}, {b}]"))
                .with_i18n("msg.optimize.invalid_interval", vec![]),
        );
    }
    if !tol.is_finite() || tol <= 0.0 {
        return Err(
            CalcError::domain("minimize: tolerance must be positive".to_string())
                .with_i18n("msg.optimize.invalid_interval", vec![]),
        );
    }

    let golden = 0.5 * (5.0_f64.sqrt() - 1.0);
    let mut lo = a;
    let mut hi = b;
    let mut x1 = hi - golden * (hi - lo);
    let mut x2 = lo + golden * (hi - lo);
    let mut f1 = f(x1);
    let mut f2 = f(x2);

    for _ in 0..max_iter {
        if (hi - lo).abs() <= tol {
            break;
        }
        // 黄金分割保底收缩。
        if f1 < f2 {
            hi = x2;
            x2 = x1;
            f2 = f1;
            x1 = hi - golden * (hi - lo);
            f1 = f(x1);
        } else {
            lo = x1;
            x1 = x2;
            f1 = f2;
            x2 = lo + golden * (hi - lo);
            f2 = f(x2);
        }
    }

    let mut x = 0.5 * (lo + hi);
    let mut fx = f(x);
    // 末段抛物线抛光：用 [lo, mid, hi] 三点拟合，仅当落在区间内且函数值更低时采纳。
    if let Some(z) = parabolic_vertex(lo, f(lo), x, fx, hi, f(hi))
        && z > lo
        && z < hi
    {
        let fz = f(z);
        if fz < fx {
            x = z;
            fx = fz;
        }
    }
    Ok((x, fx))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_minimize_quadratic() {
        let f = |x: f64| (x - 3.0) * (x - 3.0);
        let (x, fx) = minimize_1d(f, 0.0, 10.0, 1e-12, 200).unwrap();
        assert!((x - 3.0).abs() < 1e-6, "x={x}");
        assert!(fx.abs() < 1e-10, "f(x)={fx}");
    }

    #[test]
    fn test_minimize_shifted_cosine() {
        // -cos(x-1) 在 x=1 处取极小值 -1（cos 本身在 x=1 处是极大值）
        let f = |x: f64| -(x - 1.0).cos();
        let (x, fx) = minimize_1d(f, 0.0, 3.0, 1e-12, 200).unwrap();
        assert!((x - 1.0).abs() < 1e-6, "x={x}");
        assert!((fx + 1.0).abs() < 1e-10, "f(x)={fx}");
    }

    #[test]
    fn test_minimize_rejects_invalid_interval() {
        assert!(minimize_1d(|x| x, 2.0, 1.0, 1e-9, 100).is_err());
        assert!(minimize_1d(|x| x, f64::NAN, 1.0, 1e-9, 100).is_err());
        assert!(minimize_1d(|x| x, 0.0, 1.0, 0.0, 100).is_err());
    }
    #[test]
    fn test_solve_lp_standard_optimal() {
        // min -3x-2y s.t. 2x+y<=20, x+2y<=20, x,y>=0 → x=y=20/3, obj=-100/3
        let c = [-3.0, -2.0];
        let a = [vec![2.0, 1.0], vec![1.0, 2.0]];
        let b = [20.0, 20.0];
        let sol = solve_lp(&LpProblem {
            c: &c,
            a_ub: &a,
            b_ub: &b,
        })
        .unwrap();
        assert_eq!(sol.status, LpStatus::Optimal);
        assert!((sol.x[0] - 20.0 / 3.0).abs() < 1e-9, "x={:?}", sol.x);
        assert!((sol.x[1] - 20.0 / 3.0).abs() < 1e-9, "x={:?}", sol.x);
        assert!((sol.objective + 100.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn test_solve_lp_infeasible() {
        // x<=1 且 x>=2（写成 -x<=-2）
        let c = [1.0];
        let a = [vec![1.0], vec![-1.0]];
        let b = [1.0, -2.0];
        let sol = solve_lp(&LpProblem {
            c: &c,
            a_ub: &a,
            b_ub: &b,
        })
        .unwrap();
        assert_eq!(sol.status, LpStatus::Infeasible);
    }

    #[test]
    fn test_solve_lp_unbounded() {
        // min -x，x>=0 无上界
        let c = [-1.0];
        let sol = solve_lp(&LpProblem {
            c: &c,
            a_ub: &[],
            b_ub: &[],
        })
        .unwrap();
        assert_eq!(sol.status, LpStatus::Unbounded);
    }

    #[test]
    fn test_solve_lp_degenerate_no_cycle() {
        // 退化顶点：多个约束在同一点相交，Bland 规则保证有限步收敛
        let c = [-1.0, -1.0];
        let a = [vec![1.0, 0.0], vec![0.0, 1.0], vec![1.0, 1.0]];
        let b = [1.0, 1.0, 2.0];
        let sol = solve_lp(&LpProblem {
            c: &c,
            a_ub: &a,
            b_ub: &b,
        })
        .unwrap();
        assert_eq!(sol.status, LpStatus::Optimal);
        assert!((sol.objective + 2.0).abs() < 1e-9);
    }

    #[test]
    fn test_solve_lp_dimension_mismatch() {
        let c = [1.0, 2.0];
        let a = [vec![1.0]];
        let b = [1.0];
        assert!(
            solve_lp(&LpProblem {
                c: &c,
                a_ub: &a,
                b_ub: &b
            })
            .is_err()
        );
    }
}

/// 小规模稠密线性规划问题：`min c·x` s.t. `A_ub·x ≤ b_ub`, `x ≥ 0`。
#[derive(Debug, Clone)]
pub struct LpProblem<'a> {
    pub c: &'a [f64],
    pub a_ub: &'a [Vec<f64>],
    pub b_ub: &'a [f64],
}

/// 线性规划求解状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LpStatus {
    Optimal,
    Infeasible,
    Unbounded,
}

/// 线性规划解：状态、原变量取值与目标函数值。
#[derive(Debug, Clone, PartialEq)]
pub struct LpSolution {
    pub status: LpStatus,
    pub x: Vec<f64>,
    pub objective: f64,
}

/// 单纯形表：前 m 行为约束，末行为目标行（最大化形式），末列为 RHS。
struct Tableau {
    rows: Vec<Vec<f64>>,
    basis: Vec<usize>,
    nvars: usize,
}

impl Tableau {
    fn m(&self) -> usize {
        self.basis.len()
    }

    fn pivot(&mut self, r: usize, col: usize) {
        let pv = self.rows[r][col];
        for j in 0..=self.nvars {
            self.rows[r][j] /= pv;
        }
        self.basis[r] = col;
        for i in 0..=self.m() {
            if i == r {
                continue;
            }
            let factor = self.rows[i][col];
            if factor.abs() > 1e-15 {
                for j in 0..=self.nvars {
                    self.rows[i][j] -= factor * self.rows[r][j];
                }
            }
        }
    }

    /// Bland 规则：选首个正 reduced cost 列与最小比值行（平局取基变量下标小者）。
    fn simplex(&mut self, max_iter: usize) -> LpStatus {
        for _ in 0..max_iter {
            let Some(col) = (0..self.nvars).find(|&j| self.rows[self.m()][j] > 1e-9) else {
                return LpStatus::Optimal;
            };
            let mut best: Option<(usize, f64)> = None;
            for i in 0..self.m() {
                let a = self.rows[i][col];
                if a <= 1e-9 {
                    continue;
                }
                let ratio = self.rows[i][self.nvars] / a;
                let replace = match best {
                    None => true,
                    Some((br, bratio)) => {
                        ratio < bratio - 1e-12
                            || ((ratio - bratio).abs() <= 1e-12 && self.basis[i] < self.basis[br])
                    }
                };
                if replace {
                    best = Some((i, ratio));
                }
            }
            let Some((r, _)) = best else {
                return LpStatus::Unbounded;
            };
            self.pivot(r, col);
        }
        LpStatus::Optimal
    }

    /// 用当前基变量消去目标行，使基变量 reduced cost 归零。
    fn clear_objective_basis(&mut self, limit: usize) {
        let mi = self.m();
        for i in 0..mi {
            let col = self.basis[i];
            if col >= limit {
                continue;
            }
            let objective_row = self.rows[mi].clone();
            let f = objective_row[col];
            if f.abs() > 1e-15 {
                for j in 0..=self.nvars {
                    self.rows[mi][j] -= f * self.rows[i][j];
                }
            }
        }
    }
}

/// 两阶段单纯形（Bland 防循环）：先以人工变量求可行基，再优化原目标。
pub fn solve_lp(problem: &LpProblem<'_>) -> Result<LpSolution, CalcError> {
    let n = problem.c.len();
    let m = problem.b_ub.len();
    if m != problem.a_ub.len() || problem.a_ub.iter().any(|row| row.len() != n) {
        return Err(CalcError::domain("lp dimension mismatch".to_string())
            .with_i18n("msg.optimize.lp_dimension_mismatch", vec![]));
    }

    // 变量布局：x(0..n) | slack(n..n+m) | artificial(n+m..n+2m)
    let nvars = n + 2 * m;
    let mut t = Tableau {
        rows: vec![vec![0.0; nvars + 1]; m + 1],
        basis: vec![0usize; m],
        nvars,
    };
    for i in 0..m {
        let sign = if problem.b_ub[i] < 0.0 { -1.0 } else { 1.0 };
        for j in 0..n {
            t.rows[i][j] = sign * problem.a_ub[i][j];
        }
        t.rows[i][n + i] = sign; // slack
        t.rows[i][n + m + i] = 1.0; // artificial
        t.rows[i][nvars] = sign * problem.b_ub[i];
        t.basis[i] = n + m + i;
    }
    // Phase 1：最大化 -Σ artificial
    for i in 0..m {
        t.rows[m][n + m + i] = -1.0;
    }
    t.clear_objective_basis(nvars);
    let phase1 = t.simplex(1000);
    // 目标行 RHS = -z = Σ artificial（本表约定）；此值 > 0 即原问题不可行。
    let artificial_sum = t.rows[m][nvars];
    if phase1 != LpStatus::Optimal || artificial_sum.abs() > 1e-9 {
        return Ok(LpSolution {
            status: LpStatus::Infeasible,
            x: vec![0.0; n],
            objective: 0.0,
        });
    }

    // 移除人工变量列：仅保留 x 与 slack。
    let keep: Vec<usize> = (0..n + m).collect();
    let mut rows2: Vec<Vec<f64>> = t
        .rows
        .iter()
        .map(|row| {
            let mut new_row: Vec<f64> = keep.iter().map(|&j| row[j]).collect();
            new_row.push(row[nvars]);
            new_row
        })
        .collect();
    for b in &mut t.basis {
        if *b >= n + m {
            *b = n; // 退化的人工基变量占位；后续不参与目标消元
        }
    }
    // Phase 2：最大化 -c·x
    for (j, cell) in rows2[m].iter_mut().enumerate().take(n) {
        *cell = -problem.c[j];
    }
    for cell in rows2[m].iter_mut().skip(n).take(m) {
        *cell = 0.0;
    }
    rows2[m][n + m] = 0.0;
    let mut t2 = Tableau {
        rows: rows2,
        basis: t.basis,
        nvars: n + m,
    };
    t2.clear_objective_basis(n + m);
    let status = t2.simplex(1000);
    let mut x = vec![0.0; n];
    for i in 0..m {
        let col = t2.basis[i];
        if col < n {
            x[col] = t2.rows[i][n + m];
        }
    }
    let objective = problem.c.iter().zip(&x).map(|(c, x)| c * x).sum();
    Ok(LpSolution {
        status,
        x,
        objective,
    })
}
