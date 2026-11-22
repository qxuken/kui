fn main() {
    if std::env::var_os("CARGO_FEATURE_MOBILE").is_some()
        && std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("ios")
    {
        println!("cargo:rerun-if-changed=ios/editor.m");
        println!("cargo:rerun-if-changed=ios/accessibility.m");
        cc::Build::new()
            .file("ios/editor.m")
            .file("ios/accessibility.m")
            .flag("-fobjc-arc")
            .compile("kui_editor");
    }
}
