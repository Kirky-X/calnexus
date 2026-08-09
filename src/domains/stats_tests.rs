// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! 统计假设检验与相关分析模块。
//!
//! 依赖 `stats_special` 和 `stats_distributions` 模块。

use std::collections::HashMap;

use super::stats_distributions::{chi2_cdf, t_cdf};

// ===== t 检验 =====

/// 单样本 t 检验（双尾）。
///
/// H₀: μ = mu，H₁: μ ≠ mu。
/// 返回 HashMap 包含 "t", "df", "p", "mean"。
pub fn t_test_one(data: &[f64], mu: f64) -> HashMap<String, f64> {
    let n = data.len() as f64;
    let mean = data.iter().sum::<f64>() / n;
    let var = data.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0);
    let se = (var / n).sqrt();
    let t = if se > 0.0 { (mean - mu) / se } else { 0.0 };
    let df = n - 1.0;
    // 双尾 p 值
    let p = 2.0 * (1.0 - t_cdf(t.abs(), df));

    let mut result = HashMap::new();
    result.insert("t".into(), t);
    result.insert("df".into(), df);
    result.insert("p".into(), p);
    result.insert("mean".into(), mean);
    result
}

/// 双样本 Welch t 检验（双尾）。
///
/// H₀: μ₁ = μ₂，H₁: μ₁ ≠ μ₂。
/// 使用 Welch-Satterthwaite 近似自由度。
/// 返回 HashMap 包含 "t", "df", "p", "mean1", "mean2"。
pub fn t_test_two(a: &[f64], b: &[f64]) -> HashMap<String, f64> {
    let n1 = a.len() as f64;
    let n2 = b.len() as f64;
    let mean1 = a.iter().sum::<f64>() / n1;
    let mean2 = b.iter().sum::<f64>() / n2;
    let var1 = a.iter().map(|&x| (x - mean1).powi(2)).sum::<f64>() / (n1 - 1.0);
    let var2 = b.iter().map(|&x| (x - mean2).powi(2)).sum::<f64>() / (n2 - 1.0);
    let se = (var1 / n1 + var2 / n2).sqrt();
    let t = if se > 0.0 {
        (mean1 - mean2) / se
    } else {
        0.0
    };

    // Welch-Satterthwaite 自由度
    let s1_n1 = var1 / n1;
    let s2_n2 = var2 / n2;
    let df = (s1_n1 + s2_n2).powi(2)
        / (s1_n1.powi(2) / (n1 - 1.0) + s2_n2.powi(2) / (n2 - 1.0));

    let p = 2.0 * (1.0 - t_cdf(t.abs(), df));

    let mut result = HashMap::new();
    result.insert("t".into(), t);
    result.insert("df".into(), df);
    result.insert("p".into(), p);
    result.insert("mean1".into(), mean1);
    result.insert("mean2".into(), mean2);
    result
}

// ===== χ² 拟合优度检验 =====

/// χ² 拟合优度检验。
///
/// 计算 χ² = Σ(Oᵢ - Eᵢ)² / Eᵢ，df = k - 1。
/// 返回 HashMap 包含 "chi2", "df", "p"。
pub fn chi2_test(observed: &[f64], expected: &[f64]) -> HashMap<String, f64> {
    assert_eq!(
        observed.len(),
        expected.len(),
        "observed and expected must have same length"
    );
    assert!(
        expected.iter().all(|&e| e > 0.0),
        "all expected values must be positive"
    );

    let chi2 = observed
        .iter()
        .zip(expected.iter())
        .map(|(o, e)| (o - e).powi(2) / e)
        .sum::<f64>();
    let df = (observed.len() - 1) as f64;
    let p = 1.0 - chi2_cdf(chi2, df);

    let mut result = HashMap::new();
    result.insert("chi2".into(), chi2);
    result.insert("df".into(), df);
    result.insert("p".into(), p);
    result
}

// ===== 相关系数 =====

/// Pearson 相关系数。
///
/// r = Σ(xᵢ-x̄)(yᵢ-ȳ) / √(Σ(xᵢ-x̄)² · Σ(yᵢ-ȳ)²)
pub fn pearson(x: &[f64], y: &[f64]) -> f64 {
    assert_eq!(x.len(), y.len(), "x and y must have same length");
    let n = x.len() as f64;
    let mean_x = x.iter().sum::<f64>() / n;
    let mean_y = y.iter().sum::<f64>() / n;

    let mut cov = 0.0;
    let mut var_x = 0.0;
    let mut var_y = 0.0;
    for i in 0..x.len() {
        let dx = x[i] - mean_x;
        let dy = y[i] - mean_y;
        cov += dx * dy;
        var_x += dx * dx;
        var_y += dy * dy;
    }

    let denom = (var_x * var_y).sqrt();
    if denom == 0.0 {
        0.0
    } else {
        cov / denom
    }
}

/// Spearman 秩相关系数。
///
/// 先对 x, y 分别排名，再计算排名的 Pearson 相关系数。
pub fn spearman(x: &[f64], y: &[f64]) -> f64 {
    assert_eq!(x.len(), y.len(), "x and y must have same length");
    let rank_x = rank(x);
    let rank_y = rank(y);
    pearson(&rank_x, &rank_y)
}

/// 排名函数（平均秩处理并列）。
fn rank(data: &[f64]) -> Vec<f64> {
    let n = data.len();
    let mut indexed: Vec<(usize, f64)> = data.iter().copied().enumerate().collect();
    indexed.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

    let mut ranks = vec![0.0; n];
    let mut i = 0;
    while i < n {
        let mut j = i;
        // 找到所有相同值的范围 [i, j)
        while j < n && (indexed[j].1 - indexed[i].1).abs() < 1e-15 {
            j += 1;
        }
        // 平均秩（1-based）
        let avg_rank = (i + 1 + j) as f64 / 2.0;
        for k in i..j {
            ranks[indexed[k].0] = avg_rank;
        }
        i = j;
    }
    ranks
}

// ===== 测试 =====

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_approx(actual: f64, expected: f64, tol: f64, label: &str) {
        assert!(
            (actual - expected).abs() < tol,
            "{}: expected {} but got {} (diff={})",
            label,
            expected,
            actual,
            (actual - expected).abs()
        );
    }

    // ===== 单样本 t 检验 =====

    #[test]
    fn test_t_test_one_at_mean() {
        // data = [1,2,3,4,5], mu = 3.0 (= sample mean)
        // t = 0, p = 1.0
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let result = t_test_one(&data, 3.0);
        assert_approx(*result.get("t").unwrap(), 0.0, 1e-10, "t_test_one t at mean");
        assert_approx(*result.get("p").unwrap(), 1.0, 1e-10, "t_test_one p at mean");
        assert_approx(*result.get("mean").unwrap(), 3.0, 1e-10, "t_test_one mean");
        assert_approx(*result.get("df").unwrap(), 4.0, 1e-10, "t_test_one df");
    }

    #[test]
    fn test_t_test_one_far_from_mean() {
        // data = [1,2,3,4,5], mu = 0.0
        // mean=3, var=2.5, se=sqrt(0.5)≈0.7071, t=3/0.7071≈4.2426
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let result = t_test_one(&data, 0.0);
        let t = *result.get("t").unwrap();
        assert!(t > 4.0, "t_test_one t should be > 4, got {}", t);
        let p = *result.get("p").unwrap();
        assert!(p < 0.02, "t_test_one p should be < 0.02, got {}", p);
    }

    // ===== 双样本 t 检验 =====

    #[test]
    fn test_t_test_two_shifted() {
        // a=[1,2,3,4,5], b=[2,3,4,5,6]
        // mean1=3, mean2=4, var1=var2=2.5, se=sqrt(0.5+0.5)=1, t=-1
        let a = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let b = vec![2.0, 3.0, 4.0, 5.0, 6.0];
        let result = t_test_two(&a, &b);
        assert_approx(
            *result.get("t").unwrap(),
            -1.0,
            1e-10,
            "t_test_two t",
        );
        assert_approx(*result.get("df").unwrap(), 8.0, 0.1, "t_test_two df");
    }

    #[test]
    fn test_t_test_two_identical() {
        // 相同样本 → t=0, p=1
        let a = vec![1.0, 2.0, 3.0];
        let result = t_test_two(&a, &a);
        assert_approx(*result.get("t").unwrap(), 0.0, 1e-10, "t_test_two identical t");
        assert_approx(*result.get("p").unwrap(), 1.0, 1e-10, "t_test_two identical p");
    }

    // ===== χ² 拟合优度 =====

    #[test]
    fn test_chi2_test_uniform() {
        // observed=[16,18,16,14,12,12], expected=[16,16,16,16,16,16]
        // chi2 = 0+0.25+0+0.25+1+1 = 2.5... wait
        // Actually: (16-16)²/16 + (18-16)²/16 + (16-16)²/16 + (14-16)²/16 + (12-16)²/16 + (12-16)²/16
        //         = 0 + 4/16 + 0 + 4/16 + 16/16 + 16/16 = 0 + 0.25 + 0 + 0.25 + 1 + 1 = 2.5
        // But task says chi2=2.0. Let me recalculate...
        // Task says chi2=2.0, let me just verify the computation.
        let observed = vec![16.0, 18.0, 16.0, 14.0, 12.0, 12.0];
        let expected = vec![16.0, 16.0, 16.0, 16.0, 16.0, 16.0];
        let result = chi2_test(&observed, &expected);
        // chi2 = 0 + 0.25 + 0 + 0.25 + 1 + 1 = 2.5
        assert_approx(*result.get("chi2").unwrap(), 2.5, 1e-10, "chi2_test chi2");
        assert_approx(*result.get("df").unwrap(), 5.0, 1e-10, "chi2_test df");
        let p = *result.get("p").unwrap();
        // p = 1 - chi2_cdf(2.5, 5) ≈ 0.78
        assert!(p > 0.5 && p < 0.9, "chi2_test p should be moderate, got {}", p);
    }

    #[test]
    fn test_chi2_test_perfect_fit() {
        // 完全吻合 → chi2=0, p=1
        let observed = vec![10.0, 20.0, 30.0];
        let expected = vec![10.0, 20.0, 30.0];
        let result = chi2_test(&observed, &expected);
        assert_approx(*result.get("chi2").unwrap(), 0.0, 1e-15, "chi2_test perfect chi2");
        assert_approx(*result.get("p").unwrap(), 1.0, 1e-10, "chi2_test perfect p");
    }

    // ===== Pearson 相关 =====

    #[test]
    fn test_pearson_perfect_positive() {
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let y = vec![2.0, 4.0, 6.0, 8.0, 10.0];
        assert_approx(pearson(&x, &y), 1.0, 1e-10, "pearson perfect positive");
    }

    #[test]
    fn test_pearson_perfect_negative() {
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let y = vec![5.0, 4.0, 3.0, 2.0, 1.0];
        assert_approx(pearson(&x, &y), -1.0, 1e-10, "pearson perfect negative");
    }

    #[test]
    fn test_pearson_zero_correlation() {
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let y = vec![1.0, -1.0, 1.0, -1.0, 1.0];
        let r = pearson(&x, &y);
        assert!(r.abs() < 0.1, "pearson near-zero correlation, got {}", r);
    }

    // ===== Spearman 秩相关 =====

    #[test]
    fn test_spearman_perfect_positive() {
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let y = vec![2.0, 4.0, 6.0, 8.0, 10.0];
        assert_approx(spearman(&x, &y), 1.0, 1e-10, "spearman perfect positive");
    }

    #[test]
    fn test_spearman_perfect_negative() {
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let y = vec![5.0, 4.0, 3.0, 2.0, 1.0];
        assert_approx(spearman(&x, &y), -1.0, 1e-10, "spearman perfect negative");
    }

    #[test]
    fn test_spearman_with_ties() {
        // 有并列值的情况
        let x = vec![1.0, 2.0, 2.0, 4.0, 5.0];
        let y = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let rho = spearman(&x, &y);
        assert!(rho > 0.9, "spearman with ties should be high positive, got {}", rho);
    }

    // ===== 排名函数 =====

    #[test]
    fn test_rank_no_ties() {
        let data = vec![3.0, 1.0, 4.0, 2.0];
        let r = rank(&data);
        assert_approx(r[0], 3.0, 1e-15, "rank[0]");
        assert_approx(r[1], 1.0, 1e-15, "rank[1]");
        assert_approx(r[2], 4.0, 1e-15, "rank[2]");
        assert_approx(r[3], 2.0, 1e-15, "rank[3]");
    }

    #[test]
    fn test_rank_with_ties() {
        // [10, 20, 20, 40] → ranks [1, 2.5, 2.5, 4]
        let data = vec![10.0, 20.0, 20.0, 40.0];
        let r = rank(&data);
        assert_approx(r[0], 1.0, 1e-15, "rank tied[0]");
        assert_approx(r[1], 2.5, 1e-15, "rank tied[1]");
        assert_approx(r[2], 2.5, 1e-15, "rank tied[2]");
        assert_approx(r[3], 4.0, 1e-15, "rank tied[3]");
    }
}
