fn main() {
    if std::env::var_os("CARGO_FEATURE_MOBILE").is_none() {
        return;
    }
    match std::env::var("CARGO_CFG_TARGET_OS").as_deref() {
        Ok("ios") => {
            println!("cargo:rerun-if-changed=ios/spelling.m");
            println!("cargo:rerun-if-changed=ios/editor.m");
            println!("cargo:rerun-if-changed=ios/accessibility.m");
            println!("cargo:rerun-if-changed=ios/japanese_font.m");
            println!("cargo:rustc-link-lib=framework=CoreText");
            cc::Build::new()
                .file("ios/editor.m")
                .file("ios/spelling.m")
                .file("ios/accessibility.m")
                .file("ios/japanese_font.m")
                .flag("-fobjc-arc")
                .compile("kui_mobile");
        }
        Ok("android") => {
            println!("cargo:rerun-if-changed=android/japanese_font.c");
            println!("cargo:rustc-link-lib=android");
            cc::Build::new()
                .file("android/japanese_font.c")
                .define("__ANDROID_UNAVAILABLE_SYMBOLS_ARE_WEAK__", None)
                // The bridge checks the API level before using API 29 symbols.
                .flag("-Wno-unguarded-availability")
                .compile("kui_mobile");
        }
        _ => {}
    }
}
