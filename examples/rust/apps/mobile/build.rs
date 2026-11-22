fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("android") {
        // NDK r27 needs explicit ELF alignment for Android's 16 KB compatibility check.
        println!(
            "cargo:rustc-link-arg-cdylib=-Wl,-z,max-page-size=16384,-z,common-page-size=16384"
        );
    }
    #[cfg(feature = "editing-regression")]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("ios") {
        println!("cargo:rerun-if-changed=regression/probe.m");
        cc::Build::new()
            .file("regression/probe.m")
            .flag("-fobjc-arc")
            .compile("editing_probe");
    }
}
