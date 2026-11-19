// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! 轻量确定性伪随机数生成器 SplitMix64（谜题出题复用，零外部依赖）。

/// SplitMix64 状态机：同 seed 产生逐位一致的序列。
pub struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    /// 以给定种子创建生成器。
    pub fn from_seed(seed: u64) -> Self {
        Self { state: seed }
    }

    /// 下一个 64 位伪随机数。
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_same_seed_same_sequence() {
        let mut a = SplitMix64::from_seed(42);
        let mut b = SplitMix64::from_seed(42);
        for _ in 0..16 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn test_different_seed_different_first_value() {
        let a = SplitMix64::from_seed(1).next_u64();
        let b = SplitMix64::from_seed(2).next_u64();
        assert_ne!(a, b);
    }
}
