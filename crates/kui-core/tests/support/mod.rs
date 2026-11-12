//! Explicit fixtures for tests that assert real font metrics or desktop shortcuts.
#![allow(dead_code)]

use kui_core::{Core, Platform, Session};

pub fn fonts() -> Core {
    let session = Session::with_fonts(
        "en-US",
        [
            include_bytes!("../../../kui-cli/assets/fonts/NotoSans-Regular.ttf").to_vec(),
            include_bytes!("../../../kui-cli/assets/fonts/NotoSans-Bold.ttf").to_vec(),
            include_bytes!("../../../kui-cli/assets/fonts/NotoSansMono-Regular.ttf").to_vec(),
        ],
    );
    Core::new_in(&session)
}

pub fn desktop() -> Core {
    let mut core = Core::new();
    core.set_platform(Platform::current());
    core
}
