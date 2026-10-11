// Copyright (c) 2026 Kirky.X🌠
// SPDX-License-Identifier: MIT

//! CalNexus 二进制入口；panic hook 输出 i18n 短消息，退出码：0=成功，1=计算/解析错误，
//! 2=用法错误（或未启用 cli feature），3=超时，101=panic。

use std::io::Write;

/// panic hook 用的 i18n 上下文（OnceLock 缓存，避免 panic 路径上重新探测语言）。
///
/// `I18n::t` 对缺失键返回键名而不 panic，保证 hook 自身不会失败。
fn panic_hook_i18n() -> &'static calnexus::I18n {
    static HOOK_I18N: std::sync::OnceLock<calnexus::I18n> = std::sync::OnceLock::new();
    HOOK_I18N.get_or_init(calnexus::I18n::from_detected)
}

/// 安装 panic hook：i18n 前缀+报告提示；内部用 writeln! 忽略错误（eprintln! 失败会 panic）。
fn setup_panic_hook() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        // 预热：语言检测与束构建在安装 hook 前完成，panic 路径只读缓存
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

    /// 断言 hook 文案取自 FTL 目录（非硬编码英文），且 en/zh 键均存在。
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

    /// 子进程探针：仅 `CALNEXUS_PANIC_PROBE` 置位时 panic，由下方测试以子进程调用。
    #[test]
    #[ignore = "由 test_panic_hook_outputs_localized_message 以子进程方式调用"]
    fn panic_hook_probe_child() {
        if std::env::var_os("CALNEXUS_PANIC_PROBE").is_none() {
            return;
        }
        setup_panic_hook();
        panic!("probe: intentional panic for panic-hook i18n verification");
    }

    /// 端到端验证 panic hook 文案随 LANG 切换（zh_CN.UTF-8 中文、C 英文）。
    ///
    /// 须以子进程运行（检测结果进程内 `OnceLock` 缓存，同进程无法切换语言）。
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
