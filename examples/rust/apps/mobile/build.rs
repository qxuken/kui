fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("android") {
        println!("cargo:rustc-link-lib=android");
        // NDK r27 needs explicit ELF alignment for Android's 16 KB compatibility check.
        println!("cargo:rustc-link-arg-cdylib=-Wl,-z,max-page-size=16384,-z,common-page-size=16384");
        println!("cargo:rerun-if-changed=src/japanese_android.c");
        cc::Build::new()
            .file("src/japanese_android.c")
            .define("__ANDROID_UNAVAILABLE_SYMBOLS_ARE_WEAK__", None)
            // The C bridge checks android_get_device_api_level before using API 29.
            .flag("-Wno-unguarded-availability")
            .compile("japanese_font");
    }
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("ios") {
        println!("cargo:rustc-link-lib=framework=CoreText");
        println!("cargo:rerun-if-changed=src/japanese.m");
        cc::Build::new()
            .file("src/japanese.m")
            .flag("-fobjc-arc")
            .compile("japanese_font");
        #[cfg(feature = "editing-regression")]
        {
            println!("cargo:rerun-if-changed=regression/probe.m");
            cc::Build::new()
                .file("regression/probe.m")
                .flag("-fobjc-arc")
                .compile("editing_probe");
        }
    }
}
