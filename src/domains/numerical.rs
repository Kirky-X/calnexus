// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! Numerical 线性代数分解（feature-gated，`numerical` feature 启用）。
//!
//! 提供 eig / SVD / LU / QR / solve(Ax=b) 五个分解函数，基于 nalgebra 分解 API。
//! 由 MatrixDomain 委托调用（design D2）；路由入口仍是 MatrixDomain（priority=30）。
//!
//! design.md：
//! - D1：复用 nalgebra（不引入新依赖）
//! - D2：实现拆到本文件（控制 matrix.rs 行数），MatrixDomain 委托
//! - D3：复合返回走 `EvalResult::Json`
//!
//! T003 起填充分解实现。T002 仅验证 nalgebra 分解 API 可用性。

#![cfg(feature = "numerical")]

#[cfg(test)]
mod tests {
    use nalgebra::{DMatrix, SymmetricEigen, LU, QR, SVD};

    /// T002 验证：nalgebra 0.35 + `features=["std"]` 下 LU/QR/SVD/SymmetricEigen 可用。
    ///
    /// 闭环 design R1 风险点：`default-features = false` 是否影响分解 API。
    /// 若本测试编译失败（符号缺失或 trait bound 不满足），则需 `cargo add nalgebra`
    /// 补所需 feature 到 `numerical` feature 段。
    #[test]
    fn nalgebra_decompose_symbols_available() {
        let m: DMatrix<f64> = DMatrix::from_row_slice(2, 2, &[1.0, 2.0, 3.0, 4.0]);
        let _lu = LU::new(m.clone());
        let _qr = QR::new(m.clone());
        // nalgebra 0.35 SVD::new 签名：(matrix, compute_u, compute_v) —— 需显式指定是否计算 U/V
        let _svd = SVD::new(m.clone(), true, true);
        // SymmetricEigen 要求实对称矩阵
        let sym: DMatrix<f64> = DMatrix::from_row_slice(2, 2, &[2.0, 1.0, 1.0, 2.0]);
        let _eig = SymmetricEigen::new(sym);
    }
}
