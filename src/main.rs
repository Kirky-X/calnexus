// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! CalNexus 二进制入口。
//!
//! 退出码：
//! - 0：成功
//! - 1：计算错误 / 解析错误
//! - 2：用法错误（或未启用 cli feature）
//! - 3：超时
//! - 101：panic（Rust 约定，由 `std::process::abort` 或未捕获 panic 触发）
//!
//! 设置 panic hook 打印用户友好的简短错误消息到 stderr，
//! 同时保留默认 backtrace（由 `RUST_BACKTRACE` 环境变量控制）。
//! 此前 panic 直接打印 `thread 'main' panicked at ...` 对终端用户不友好。
//!
//! 两条 hook 文案经 i18n 目录（FTL 键 `panic.internal_error` / `panic.report_bug`）
//! 按检测链（`CALNEXUS_LANG` → `LC_ALL` → `LC_MESSAGES` → `LANG` → sys-locale → en）
//! 输出中/英文，无硬编码英文残留。

use std::io::Write;

/// panic hook 使用的 i18n 上下文（`OnceLock` 进程内缓存一次）。
///
/// 语言经检测链确定（[`calnexus::I18n::from_detected`]），实例缓存避免在
/// panic 路径上重复探测环境/构建 Fluent 束。`I18n::t` 对缺失键返回键名本身、
/// 不 panic，因此 hook 内查询文案本身不会失败（「hook 自身绝不能再 panic」）。
fn panic_hook_i18n() -> &'static calnexus::I18n {
    static HOOK_I18N: std::sync::OnceLock<calnexus::I18n> = std::sync::OnceLock::new();
    HOOK_I18N.get_or_init(calnexus::I18n::from_detected)
}

/// 安装 panic hook：打印简短错误前缀，再调用默认 hook 输出 backtrace。
///
/// 设计权衡：
/// - 保留默认 backtrace 行为（开发者调试需要，由 RUST_BACKTRACE 控制）
/// - 在默认输出前追加用户友好的简短错误前缀，结尾追加报告提示；两条文案均取
///   `panic.internal_error` / `panic.report_bug` 的译文（en/zh），输出格式
///   （`calnexus: ` 前缀 + 换行 + 默认 hook 输出 + 结尾提示）与既有实现一致
/// - hook 自身不 panic：文案来自预热后的缓存 `I18n`，写出走 `writeln!` 并忽略
///   写入错误（`eprintln!` 写 stderr 失败会 panic，在 hook 内即「panic 中 panic」→ abort）
/// - 全局生效（影响所有线程的 panic 行为）
/// - 使用 Once 保证幂等，多次调用不叠加 hook
fn setup_panic_hook() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        // 预热：语言检测与束构建在安装 hook 之前完成，此后 panic 路径只读缓存
        // （hook 内不做任何可能失败的初始化工作）。
        let i18n: &'static calnexus::I18n = panic_hook_i18n();
        let default_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let _ = writeln!(std::io::stderr(), "{}", i18n.t("panic.internal_error"));
            default_hook(info);
            let _ = writeln!(std::io::stderr(), "{}", i18n.t("panic.report_bug"));
        }));
    });
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

    /// 验证 setup_panic_hook 调用不 panic。
    #[test]
    fn test_setup_panic_hook_does_not_panic() {
        setup_panic_hook();
    }

    /// 验证 setup_panic_hook 幂等（多次调用不 panic，不叠加 hook）。
    #[test]
    fn test_setup_panic_hook_idempotent() {
        setup_panic_hook();
        setup_panic_hook();
        setup_panic_hook();
    }

    /// 验证 hook 文案取自 FTL 目录（`panic.internal_error` / `panic.report_bug`），
    /// 即主路径不再硬编码英文：两个键在 en/zh 束中均存在且与硬编码旧文案一致（en）。
    #[test]
    fn test_panic_hook_messages_come_from_catalog() {
        let en = calnexus::I18n::new(calnexus::Lang::En);
        let zh = calnexus::I18n::new(calnexus::Lang::Zh);
        // en 文案与迁移前的硬编码字面量逐字一致（输出格式不变）
        assert_eq!(
            en.t("panic.internal_error"),
            "calnexus: internal error (panic)"
        );
        assert_eq!(
            en.t("panic.report_bug"),
            "calnexus: please report this bug with the backtrace above"
        );
        // zh 为对应译文；键缺失时会回退键名本身 —— 断言非键名即证明命中目录
        assert_eq!(zh.t("panic.internal_error"), "calnexus: 内部错误（panic）");
        assert_eq!(
            zh.t("panic.report_bug"),
            "calnexus: 请携带上方 backtrace 报告此问题"
        );
        for key in ["panic.internal_error", "panic.report_bug"] {
            assert_ne!(en.t(key), key, "en catalog missing key: {key}");
            assert_ne!(zh.t(key), key, "zh catalog missing key: {key}");
        }
        assert_ne!(en.t("panic.internal_error"), zh.t("panic.internal_error"));
    }

    /// 子进程探针：仅当 `CALNEXUS_PANIC_PROBE` 置位时 panic。
    ///
    /// 由 [`test_panic_hook_outputs_localized_message`] 以子进程方式调用
    /// （`--ignored --exact --nocapture`）。常规 `cargo test` 不执行；即便显式
    /// `cargo test -- --ignored` 运行，未置位环境变量时也直接返回，不产生误报。
    #[test]
    #[ignore = "由 test_panic_hook_outputs_localized_message 以子进程方式调用"]
    fn panic_hook_probe_child() {
        if std::env::var_os("CALNEXUS_PANIC_PROBE").is_none() {
            return;
        }
        setup_panic_hook();
        panic!("probe: intentional panic for panic-hook i18n verification");
    }

    /// 端到端验证 panic hook 输出随检测链切换语言：
    /// `LANG=zh_CN.UTF-8` → 中文文案，`LANG=C` → 英文文案，且格式为
    /// `calnexus: <文案>` 前后两行包夹默认 hook 输出。
    ///
    /// 以子进程运行（检测链结果进程内 `OnceLock` 缓存，同进程无法切换语言）。
    #[test]
    fn test_panic_hook_outputs_localized_message() {
        let exe = std::env::current_exe().expect("current_exe");
        let run_probe = |lang: &str| -> String {
            let out = std::process::Command::new(&exe)
                .args([
                    "--exact",
                    "tests::panic_hook_probe_child",
                    "--ignored",
                    "--nocapture",
                ])
                .env("CALNEXUS_PANIC_PROBE", "1")
                // 清空检测链前两级（空值被检测链跳过），只经 LC_MESSAGES/LANG 注入
                .env("CALNEXUS_LANG", "")
                .env("LC_ALL", "")
                .env("LC_MESSAGES", "")
                .env("LANG", lang)
                .output()
                .expect("spawn panic probe child");
            String::from_utf8_lossy(&out.stderr).into_owned()
        };

        let zh = run_probe("zh_CN.UTF-8");
        assert!(
            zh.contains("calnexus: 内部错误（panic）"),
            "zh panic hook prefix missing, stderr:\n{zh}"
        );
        assert!(
            zh.contains("calnexus: 请携带上方 backtrace 报告此问题"),
            "zh panic hook report line missing, stderr:\n{zh}"
        );
        assert!(
            !zh.contains("calnexus: internal error (panic)"),
            "zh run must not emit hardcoded English, stderr:\n{zh}"
        );

        let en = run_probe("C");
        assert!(
            en.contains("calnexus: internal error (panic)"),
            "en panic hook prefix missing, stderr:\n{en}"
        );
        assert!(
            en.contains("calnexus: please report this bug with the backtrace above"),
            "en panic hook report line missing, stderr:\n{en}"
        );
    }
}
