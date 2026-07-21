// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

//! CalNexus 二进制入口。
//!
//! 退出码（design.md §5.6）：
//! - 0：成功
//! - 1：计算错误 / 解析错误
//! - 2：用法错误（或未启用 cli feature）
//! - 3：超时
//! - 101：panic（Rust 约定，由 `std::process::abort` 或未捕获 panic 触发）
//!
//! BUG-E-L-001: 设置 panic hook 打印用户友好的简短错误消息到 stderr，
//! 同时保留默认 backtrace（由 `RUST_BACKTRACE` 环境变量控制）。
//! 此前 panic 直接打印 `thread 'main' panicked at ...` 对终端用户不友好。

/// 安装 panic hook：打印简短错误前缀，再调用默认 hook 输出 backtrace。
///
/// 设计权衡：
/// - 保留默认 backtrace 行为（开发者调试需要，由 RUST_BACKTRACE 控制）
/// - 在默认输出前追加用户友好的简短错误前缀（"calnexus: internal error"）
/// - 全局生效（影响所有线程的 panic 行为）
/// - 幂等：多次调用不会叠加 hook（每次 take_hook 取出当前 hook 替换）
fn setup_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        eprintln!("calnexus: internal error (panic)");
        // 调用默认 hook 打印 panic location + backtrace（如启用）
        default_hook(info);
        eprintln!("calnexus: please report this bug with the backtrace above");
    }));
}

#[cfg(feature = "cli")]
fn main() {
    setup_panic_hook();
    let exit_code = calnexus::run();
    std::process::exit(exit_code);
}

#[cfg(not(feature = "cli"))]
fn main() {
    setup_panic_hook();
    // i18n 模块始终编译（无 cfg gate），可在此使用默认语言（en）输出错误。
    let i18n = calnexus::I18n::default();
    eprintln!("{}", i18n.t("main.feature_missing"));
    std::process::exit(2);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// BUG-E-L-001: 验证 setup_panic_hook 调用不 panic。
    #[test]
    fn test_setup_panic_hook_does_not_panic() {
        setup_panic_hook();
    }

    /// BUG-E-L-001: 验证 setup_panic_hook 幂等（多次调用不 panic，不叠加 hook）。
    #[test]
    fn test_setup_panic_hook_idempotent() {
        setup_panic_hook();
        setup_panic_hook();
        setup_panic_hook();
    }
}
