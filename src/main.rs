// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

#[cfg(feature = "cli")]
fn main() {
    let exit_code = calnexus::run();
    std::process::exit(exit_code);
}

#[cfg(not(feature = "cli"))]
fn main() {
    // i18n 模块始终编译（无 cfg gate），可在此使用默认语言（en）输出错误。
    let i18n = calnexus::I18n::default();
    eprintln!("{}", i18n.t("main.feature_missing"));
    std::process::exit(2);
}
