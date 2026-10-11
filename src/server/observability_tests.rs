// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! Observability 初始化单例语义测试（`observability` feature）。
//! 独立于 mod.rs 存放：测试体含 thread::spawn / mem::forget，不驻留 mod.rs。

/// 单例语义：首调用 Ok，二次 ConfigError；LoggerManager 析构会挂起，故 mem::forget 兜底。
#[test]
fn test_init_observability_singleton_semantics() {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("observability test runtime");
        let call = || async {
            tokio::time::timeout(
                std::time::Duration::from_secs(10),
                crate::server::init_observability(),
            )
            .await
        };
        let first = rt.block_on(call());
        let second = if first.as_ref().is_ok_and(Result::is_ok) {
            Some(rt.block_on(call()))
        } else {
            None
        };
        let _ = tx.send((first, second));
        std::mem::forget(rt);
    });
    let (first, second) = rx
        .recv_timeout(std::time::Duration::from_secs(30))
        .expect("init 结果回传超时（30s）——inklog init 不应无限阻塞");
    let first = first.expect("首次 init 超时（10s）");
    assert!(
        first.is_ok(),
        "进程内首次 init 应成功，实际 {:?}",
        first.err()
    );
    let second = second
        .expect("首次 init 未成功，二次调用未执行")
        .expect("二次 init 超时（10s）");
    match second {
        Err(inklog::InklogError::ConfigError(_)) => {}
        other => panic!(
            "二次 init 应返回 InklogError::ConfigError，实际 {:?}",
            other
        ),
    }
}
