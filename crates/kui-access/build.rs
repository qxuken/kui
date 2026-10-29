//! One cfg, `unix_atspi`: the targets `accesskit_unix` serves (AT-SPI over
//! D-Bus), named once rather than spelled out at every use.

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rustc-check-cfg=cfg(unix_atspi)");
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if matches!(
        os.as_str(),
        "linux" | "dragonfly" | "freebsd" | "netbsd" | "openbsd"
    ) {
        println!("cargo::rustc-cfg=unix_atspi");
    }
}
