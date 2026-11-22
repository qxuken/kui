#![cfg(any(target_os = "android", target_os = "ios"))]

use kui_native::{Core, mobile};
use kui_runner::embedded::EmbeddedApp;

const PACK: &[u8] = include_bytes!(env!(
    "KUI_TASKS_PACK",
    "Set KUI_TASKS_PACK to the Tasks .kui archive"
));

fn core() -> Core {
    let font = include_bytes!("../../../../../../crates/kui-cli/assets/fonts/NotoSans-Regular.ttf");
    let mut core = Core::new_in(&mobile::session_with_japanese_fallback(font));
    core.set_native_menus(false);
    core.set_native_menu_bar(false);
    core
}

#[cfg(target_os = "ios")]
pub fn start() {
    let library =
        std::path::PathBuf::from(std::env::var_os("HOME").expect("app sandbox")).join("Library");
    let (app, extensions) = EmbeddedApp::load(
        PACK,
        &library.join("Application Support/KUI"),
        &library.join("Caches/KUI"),
    )
    .expect("load embedded Tasks pack");
    mobile::run_with_extensions(core(), app, extensions).expect("iOS runner");
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(activity: mobile::AndroidApp) {
    let files = activity.internal_data_path().expect("app files directory");
    let cache = files.parent().expect("app sandbox").join("cache");
    let (app, extensions) =
        EmbeddedApp::load(PACK, &files.join("kui"), &cache).expect("load embedded Tasks pack");
    mobile::run_with_extensions(activity, core(), app, extensions).expect("Android runner");
}
