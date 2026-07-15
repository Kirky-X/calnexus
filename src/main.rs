// Copyright (c) 2026 Kirky.X. Licensed under the MIT License.

#[cfg(feature = "cli")]
fn main() {
    let exit_code = calnexus::run();
    std::process::exit(exit_code);
}

#[cfg(not(feature = "cli"))]
fn main() {
    eprintln!("calnexus was compiled without the 'cli' feature. Rebuild with --features cli.");
    std::process::exit(2);
}
