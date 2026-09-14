// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! 同步三态熔断器（吸收自研库 limiteron `circuit` 模块状态机设计，同步化裁剪）。
//!
//! ```text
//! Closed --连续失败达阈值--> Open --冷却到期（惰性）--> HalfOpen --探针成功--> Closed
//!                                                              └--探针失败--> Open（重置冷却）
//! ```
//!
//! 与 limiteron 的差异（裁剪依据）：
//! - 纯同步实现（`Mutex` + `Instant`），无 tokio（CalNexus CLI 为纯同步代码）；
//! - 半开探针准入依赖上层单飞锁（fx_provider `fetch_lock` 串行化），同一时刻至多
//!   1 个探针在途，无需 limiteron 的 `half_open_max_calls` CAS 准入；
//! - 无慢调用率熔断与错误分类器：fx 端点编译期固定，超时/连接/
//!   读取/解析全部视为源站故障，等价于 limiteron 默认 `ErrorClassifier` 对瞬态
//!   错误的全量归类。
//!
//! 收益：server 长驻模式下源站故障期间，请求不再每笔白付 HTTP 超时（5s），
//! 而是立即得到 [`CircuitError::Open`] 并进入 stale 策略降级。

use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

/// 熔断器包装的调用结果。
///
/// - [`CircuitError::Open`]：熔断打开，闭包未执行即被拒绝（快速失败）；
/// - [`CircuitError::Inner`]：调用被放行但自身失败，透传原始错误。
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum CircuitError<E> {
    Open,
    Inner(E),
}

/// 熔断器内部状态。
#[derive(Debug)]
enum State {
    /// 关闭（正常放行）：记录连续失败计数。
    Closed { failures: u32 },
    /// 打开（快速失败）：记录打开时刻，冷却到期后惰性转半开。
    Open { opened_at: Instant },
    /// 半开（探针）：放行单个探针调用，成败决定回闭合或重新打开。
    HalfOpen,
}

/// 同步三态熔断器。
///
/// 线程安全（内部 `Mutex`；fx_provider 中实际被 `fetch_lock` 串行化）。
/// 内部方法的时间参数 `now: Instant` 显式注入，单元测试不依赖系统时钟。
#[derive(Debug)]
pub(crate) struct CircuitBreaker {
    threshold: u32,
    cooldown: Duration,
    state: Mutex<State>,
}

impl CircuitBreaker {
    /// 创建熔断器：`threshold` 为连续失败阈值（<1 时抬升为 1），`cooldown` 为打开态冷却时长。
    pub(crate) fn new(threshold: u32, cooldown: Duration) -> Self {
        Self {
            threshold: threshold.max(1),
            cooldown,
            state: Mutex::new(State::Closed { failures: 0 }),
        }
    }

    /// 经熔断器执行调用：拒绝时返回 [`CircuitError::Open`]，闭包不执行。
    pub(crate) fn call<T, E>(
        &self,
        f: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, CircuitError<E>> {
        if !self.admit(Instant::now()) {
            return Err(CircuitError::Open);
        }
        match f() {
            Ok(value) => {
                self.record_success();
                Ok(value)
            }
            Err(err) => {
                self.record_failure(Instant::now());
                Err(CircuitError::Inner(err))
            }
        }
    }

    /// 判定当前时刻是否放行；Open 冷却到期时惰性转 HalfOpen（本次调用即探针）。
    fn admit(&self, now: Instant) -> bool {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        match &*state {
            State::Closed { .. } => true,
            State::Open { opened_at } => {
                if now.duration_since(*opened_at) >= self.cooldown {
                    *state = State::HalfOpen;
                    true
                } else {
                    false
                }
            }
            // 已有探针在途（被 fetch_lock 串行化时不可达，防御并发探测）
            State::HalfOpen => false,
        }
    }

    /// 记录成功：任意状态回闭合，失败计数清零。
    fn record_success(&self) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        *state = State::Closed { failures: 0 };
    }

    /// 记录失败：闭合态累加计数（达阈值转打开）；半开探针失败立即重新打开。
    fn record_failure(&self, now: Instant) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        match &*state {
            State::Closed { failures } => {
                let failures = *failures + 1;
                *state = if failures >= self.threshold {
                    State::Open { opened_at: now }
                } else {
                    State::Closed { failures }
                };
            }
            // Open 态经 call() 不可达（拒绝不计失败，不刷新冷却）；此处覆盖半开探针失败 → 重新打开
            State::HalfOpen | State::Open { .. } => *state = State::Open { opened_at: now },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_call_passthrough_success_and_error() {
        let b = CircuitBreaker::new(3, Duration::from_secs(30));
        assert_eq!(b.call(|| Ok::<i32, &str>(42)), Ok(42));
        assert_eq!(
            b.call(|| Err::<i32, &str>("x")),
            Err(CircuitError::Inner("x"))
        );
    }

    #[test]
    fn test_threshold_failures_open_circuit_and_fast_fail() {
        let b = CircuitBreaker::new(3, Duration::from_secs(30));
        let calls = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let counter = calls.clone();
        let call = || {
            let c = counter.clone();
            b.call(move || -> Result<(), &str> {
                c.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Err("boom")
            })
        };
        // 前 3 次真实执行并失败
        assert!(matches!(call(), Err(CircuitError::Inner("boom"))));
        assert!(matches!(call(), Err(CircuitError::Inner("boom"))));
        assert!(matches!(call(), Err(CircuitError::Inner("boom"))));
        // 第 4 次：熔断打开，闭包不执行（计数停在 3）
        assert!(matches!(call(), Err(CircuitError::Open)));
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 3);
    }

    #[test]
    fn test_success_resets_failure_count() {
        let b = CircuitBreaker::new(3, Duration::from_secs(30));
        let t0 = Instant::now();
        b.record_failure(t0);
        b.record_failure(t0);
        b.record_success();
        b.record_failure(t0);
        b.record_failure(t0);
        // 连续失败仅 2 次（< 阈值），仍闭合放行
        assert!(b.admit(t0));
    }

    #[test]
    fn test_open_rejects_until_cooldown_then_allows_probe() {
        let b = CircuitBreaker::new(1, Duration::from_secs(10));
        let t0 = Instant::now();
        b.record_failure(t0); // 达阈值立即打开
        assert!(!b.admit(t0 + Duration::from_secs(5)));
        // 冷却到期：惰性转半开并放行探针
        assert!(b.admit(t0 + Duration::from_secs(10)));
    }

    #[test]
    fn test_rejections_do_not_extend_cooldown() {
        let b = CircuitBreaker::new(1, Duration::from_secs(10));
        let t0 = Instant::now();
        b.record_failure(t0);
        assert!(!b.admit(t0 + Duration::from_secs(3)));
        assert!(!b.admit(t0 + Duration::from_secs(6)));
        // 拒绝不刷新冷却：到期时刻仍然放行
        assert!(b.admit(t0 + Duration::from_secs(10)));
    }

    #[test]
    fn test_half_open_probe_success_closes() {
        let b = CircuitBreaker::new(2, Duration::from_secs(10));
        let t0 = Instant::now();
        b.record_failure(t0);
        b.record_failure(t0); // 打开
        assert!(b.admit(t0 + Duration::from_secs(10))); // 探针放行（半开）
        b.record_success(); // 探针成功 → 闭合，计数清零
        assert!(b.admit(t0 + Duration::from_secs(11)));
        b.record_failure(t0 + Duration::from_secs(11));
        // 仅 1 次失败（< 阈值 2），仍闭合
        assert!(b.admit(t0 + Duration::from_secs(12)));
    }

    #[test]
    fn test_half_open_probe_failure_reopens() {
        let b = CircuitBreaker::new(1, Duration::from_secs(10));
        let t0 = Instant::now();
        b.record_failure(t0); // 打开
        assert!(b.admit(t0 + Duration::from_secs(10))); // 探针放行（半开）
        b.record_failure(t0 + Duration::from_secs(10)); // 探针失败 → 重新打开（新冷却）
        assert!(!b.admit(t0 + Duration::from_secs(15)));
        assert!(b.admit(t0 + Duration::from_secs(20))); // 新冷却到期再探
    }

    #[test]
    fn test_threshold_below_one_is_raised_to_one() {
        let b = CircuitBreaker::new(0, Duration::from_secs(30));
        let t0 = Instant::now();
        b.record_failure(t0);
        assert!(!b.admit(t0));
    }
}
