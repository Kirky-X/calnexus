// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! 向量核心函数：点积/叉积/模/归一化/加减/距离/投影/反射等。
//!
//! 将 `domains/vector.rs` 中的向量数学逻辑提取为独立 `pub fn`，
//! 接收 `&[f64]` 参数，供域层和 API 层共用。

use crate::core::CalcError;

/// 点积。要求等长。
pub fn dot(a: &[f64], b: &[f64]) -> Result<f64, CalcError> {
    if a.len() != b.len() {
        return Err(CalcError::domain(format!(
            "dot(): dimension mismatch {} vs {}", a.len(), b.len()
        )));
    }
    Ok(a.iter().zip(b.iter()).map(|(x, y)| x * y).sum())
}

/// 三维叉积。仅支持 3 维向量。
pub fn cross(a: &[f64], b: &[f64]) -> Result<Vec<f64>, CalcError> {
    if a.len() != 3 || b.len() != 3 {
        return Err(CalcError::domain("cross() requires 3-dimensional vectors".to_string()));
    }
    Ok(vec![
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ])
}

/// 向量模长（L2 范数）。
pub fn magnitude(v: &[f64]) -> f64 {
    v.iter().map(|x| x * x).sum::<f64>().sqrt()
}

/// 归一化为单位向量。零向量返回错误。
pub fn normalize(v: &[f64]) -> Result<Vec<f64>, CalcError> {
    let norm = magnitude(v);
    if norm == 0.0 {
        return Err(CalcError::domain("cannot normalize zero vector".to_string()));
    }
    Ok(v.iter().map(|x| x / norm).collect())
}

/// 逐元素加法。要求等长。
pub fn vector_add(a: &[f64], b: &[f64]) -> Result<Vec<f64>, CalcError> {
    if a.len() != b.len() {
        return Err(CalcError::domain(format!(
            "vector_add: dimension mismatch {} vs {}", a.len(), b.len()
        )));
    }
    Ok(a.iter().zip(b.iter()).map(|(x, y)| x + y).collect())
}

/// 逐元素减法。要求等长。
pub fn vector_sub(a: &[f64], b: &[f64]) -> Result<Vec<f64>, CalcError> {
    if a.len() != b.len() {
        return Err(CalcError::domain(format!(
            "vector_sub: dimension mismatch {} vs {}", a.len(), b.len()
        )));
    }
    Ok(a.iter().zip(b.iter()).map(|(x, y)| x - y).collect())
}

/// 标量乘法。
pub fn scalar_mul_vec(v: &[f64], s: f64) -> Vec<f64> {
    v.iter().map(|x| x * s).collect()
}

/// 两向量夹角（弧度）。零向量返回错误。
pub fn angle(a: &[f64], b: &[f64]) -> Result<f64, CalcError> {
    if a.len() != b.len() {
        return Err(CalcError::domain(format!(
            "angle(): dimension mismatch {} vs {}", a.len(), b.len()
        )));
    }
    let norm_a = magnitude(a);
    let norm_b = magnitude(b);
    if norm_a == 0.0 || norm_b == 0.0 {
        return Err(CalcError::domain("angle(): zero vector has no angle".to_string()));
    }
    let cos_theta = dot(a, b)? / (norm_a * norm_b);
    Ok(cos_theta.clamp(-1.0, 1.0).acos())
}

/// 三维混合积 a·(b×c)。仅支持 3 维。
pub fn scalar_triple(a: &[f64], b: &[f64], c: &[f64]) -> Result<f64, CalcError> {
    if a.len() != 3 || b.len() != 3 || c.len() != 3 {
        return Err(CalcError::domain("scalar_triple() requires 3-dimensional vectors".to_string()));
    }
    let cr = cross(b, c)?;
    dot(a, &cr)
}

/// 余弦相似度 = dot(a,b) / (norm(a) * norm(b))。零向量返回错误。
pub fn cosine_similarity(a: &[f64], b: &[f64]) -> Result<f64, CalcError> {
    if a.len() != b.len() {
        return Err(CalcError::domain(format!(
            "cosine_similarity(): dimension mismatch {} vs {}", a.len(), b.len()
        )));
    }
    let norm_a = magnitude(a);
    let norm_b = magnitude(b);
    if norm_a == 0.0 || norm_b == 0.0 {
        return Err(CalcError::domain("cosine_similarity(): zero vector".to_string()));
    }
    let cos = dot(a, b)? / (norm_a * norm_b);
    Ok(cos.clamp(-1.0, 1.0))
}

/// 投影 = (dot(a,b) / dot(b,b)) * b。零向量 b 返回错误。
pub fn project(a: &[f64], b: &[f64]) -> Result<Vec<f64>, CalcError> {
    if a.len() != b.len() {
        return Err(CalcError::domain(format!(
            "project(): dimension mismatch {} vs {}", a.len(), b.len()
        )));
    }
    let b_dot_b = dot(b, b)?;
    if b_dot_b == 0.0 {
        return Err(CalcError::domain("project(): cannot project onto zero vector".to_string()));
    }
    let scalar = dot(a, b)? / b_dot_b;
    Ok(scalar_mul_vec(b, scalar))
}

/// 反射 = v - 2 * (dot(v,n) / dot(n,n)) * n。零法向量返回错误。
pub fn reflect(v: &[f64], n: &[f64]) -> Result<Vec<f64>, CalcError> {
    if v.len() != n.len() {
        return Err(CalcError::domain(format!(
            "reflect(): dimension mismatch {} vs {}", v.len(), n.len()
        )));
    }
    let n_dot_n = dot(n, n)?;
    if n_dot_n == 0.0 {
        return Err(CalcError::domain("reflect(): zero normal vector".to_string()));
    }
    let scalar = 2.0 * dot(v, n)? / n_dot_n;
    let scaled_n = scalar_mul_vec(n, scalar);
    vector_sub(v, &scaled_n)
}

/// 欧几里得距离。
pub fn euclidean(a: &[f64], b: &[f64]) -> Result<f64, CalcError> {
    if a.len() != b.len() {
        return Err(CalcError::domain(format!(
            "euclidean(): dimension mismatch {} vs {}", a.len(), b.len()
        )));
    }
    Ok(a.iter().zip(b.iter()).map(|(x, y)| (x - y).powi(2)).sum::<f64>().sqrt())
}

/// 曼哈顿距离。
pub fn manhattan(a: &[f64], b: &[f64]) -> Result<f64, CalcError> {
    if a.len() != b.len() {
        return Err(CalcError::domain(format!(
            "manhattan(): dimension mismatch {} vs {}", a.len(), b.len()
        )));
    }
    Ok(a.iter().zip(b.iter()).map(|(x, y)| (x - y).abs()).sum())
}

/// 外积 a × b^T，返回 Vec<Vec<f64>>（行优先矩阵）。
pub fn outer(a: &[f64], b: &[f64]) -> Vec<Vec<f64>> {
    a.iter().map(|ai| b.iter().map(|bi| ai * bi).collect()).collect()
}

/// 线性插值（向量版）：a + t * (b - a)。
pub fn lerp_vec(a: &[f64], b: &[f64], t: f64) -> Result<Vec<f64>, CalcError> {
    if a.len() != b.len() {
        return Err(CalcError::domain(format!(
            "lerp(): dimension mismatch {} vs {}", a.len(), b.len()
        )));
    }
    Ok(a.iter().zip(b.iter()).map(|(ai, bi)| ai + t * (bi - ai)).collect())
}

/// 线性插值（标量版）。
pub fn lerp_scalar(a: f64, b: f64, t: f64) -> f64 {
    a + t * (b - a)
}

// ===== 测试 =====

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_approx(actual: f64, expected: f64, tol: f64, label: &str) {
        assert!((actual - expected).abs() < tol,
            "{}: expected {} but got {} (diff={})", label, expected, actual, (actual - expected).abs());
    }

    #[test]
    fn test_dot() {
        assert_approx(dot(&[1.0, 2.0, 3.0], &[4.0, 5.0, 6.0]).unwrap(), 32.0, 1e-10, "dot");
    }

    #[test]
    fn test_dot_dim_mismatch() {
        assert!(dot(&[1.0, 2.0], &[1.0, 2.0, 3.0]).is_err());
    }

    #[test]
    fn test_cross() {
        let r = cross(&[1.0, 0.0, 0.0], &[0.0, 1.0, 0.0]).unwrap();
        assert_approx(r[0], 0.0, 1e-10, "cross[0]");
        assert_approx(r[1], 0.0, 1e-10, "cross[1]");
        assert_approx(r[2], 1.0, 1e-10, "cross[2]");
    }

    #[test]
    fn test_cross_not_3d() {
        assert!(cross(&[1.0, 2.0], &[3.0, 4.0]).is_err());
    }

    #[test]
    fn test_magnitude() {
        assert_approx(magnitude(&[3.0, 4.0]), 5.0, 1e-10, "magnitude");
    }

    #[test]
    fn test_normalize() {
        let r = normalize(&[3.0, 4.0]).unwrap();
        assert_approx(r[0], 0.6, 1e-10, "normalize[0]");
        assert_approx(r[1], 0.8, 1e-10, "normalize[1]");
    }

    #[test]
    fn test_normalize_zero() {
        assert!(normalize(&[0.0, 0.0]).is_err());
    }

    #[test]
    fn test_vector_add_sub() {
        let s = vector_add(&[1.0, 2.0], &[3.0, 4.0]).unwrap();
        assert_approx(s[0], 4.0, 1e-10, "add[0]");
        let d = vector_sub(&[5.0, 6.0], &[1.0, 2.0]).unwrap();
        assert_approx(d[0], 4.0, 1e-10, "sub[0]");
    }

    #[test]
    fn test_scalar_mul_vec() {
        let r = scalar_mul_vec(&[1.0, 2.0, 3.0], 2.0);
        assert_approx(r[2], 6.0, 1e-10, "scalar_mul");
    }

    #[test]
    fn test_angle() {
        let a = angle(&[1.0, 0.0], &[0.0, 1.0]).unwrap();
        assert_approx(a, std::f64::consts::FRAC_PI_2, 1e-10, "angle perpendicular");
    }

    #[test]
    fn test_cosine_similarity() {
        let c = cosine_similarity(&[1.0, 0.0], &[1.0, 0.0]).unwrap();
        assert_approx(c, 1.0, 1e-10, "cosine same");
    }

    #[test]
    fn test_project() {
        let p = project(&[3.0, 4.0], &[1.0, 0.0]).unwrap();
        assert_approx(p[0], 3.0, 1e-10, "project[0]");
        assert_approx(p[1], 0.0, 1e-10, "project[1]");
    }

    #[test]
    fn test_reflect() {
        let r = reflect(&[1.0, 1.0], &[0.0, 1.0]).unwrap();
        assert_approx(r[0], 1.0, 1e-10, "reflect[0]");
        assert_approx(r[1], -1.0, 1e-10, "reflect[1]");
    }

    #[test]
    fn test_euclidean() {
        assert_approx(euclidean(&[0.0, 0.0], &[3.0, 4.0]).unwrap(), 5.0, 1e-10, "euclidean");
    }

    #[test]
    fn test_manhattan() {
        assert_approx(manhattan(&[0.0, 0.0], &[3.0, 4.0]).unwrap(), 7.0, 1e-10, "manhattan");
    }

    #[test]
    fn test_outer() {
        let m = outer(&[1.0, 2.0], &[3.0, 4.0]);
        assert_approx(m[0][0], 3.0, 1e-10, "outer[0][0]");
        assert_approx(m[1][1], 8.0, 1e-10, "outer[1][1]");
    }

    #[test]
    fn test_lerp_vec() {
        let r = lerp_vec(&[0.0, 0.0], &[10.0, 20.0], 0.5).unwrap();
        assert_approx(r[0], 5.0, 1e-10, "lerp[0]");
        assert_approx(r[1], 10.0, 1e-10, "lerp[1]");
    }

    #[test]
    fn test_lerp_scalar() {
        assert_approx(lerp_scalar(0.0, 10.0, 0.5), 5.0, 1e-10, "lerp_scalar");
    }
}
