// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! Observability 初始化单例语义测试（`observability` feature）。
//!
//! 独立于 `mod.rs` 存放：测试体含 `thread::spawn` / `mem::forget` /
//! 临时 runtime，驻留 mod.rs 会使 R-server-logging-003 的
//! `grep -n "mem::forget\|thread::spawn" src/server/mod.rs` 零命中验收
//! 无法成立，亦违背 mod.rs 只放声明与 re-export 的纪律。

/// init_observability 单例语义：首调用 Ok，二次调用返回
/// `Err(InklogError::ConfigError)`（inklog 单例哨兵），调用方显式识别为
/// "已初始化" 而非 panic 或吞错。
///
/// 结构说明：inklog init 构建的 LoggerManager 在 runtime 析构时会挂起
/// （sink worker 通道握手等退出协同，实测 >5min 不返回），因此本测试在
/// 独立线程内自建 multi_thread runtime（与生产 `HttpServer::run` 同
/// 风味，http.rs），两次调用均带 10s 超时护栏，结果经 channel
/// 回传后 `mem::forget(runtime)` 跳过析构——进程退出由测试二进制统一
/// 回收。断言的是真实单例契约，不是"不报错"。
///
/// 进程内唯一初始化点：`start_inner` 的 bind 失败路径先于 init 返回，
/// bind 测试不再抢占单例，故"首调用 Ok"与并行执行顺序解耦。
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
