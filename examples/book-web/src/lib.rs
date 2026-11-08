//! Compile the book's example programs with the browser as their host.
#![cfg(target_arch = "wasm32")]

extern crate self as kui_devtools;
extern crate self as kui_native;

use std::cell::{Cell as StateCell, RefCell};

pub use kui_core::*;
#[path = "../../../crates/kui-native/src/audio/synth.rs"]
pub mod audio;

#[path = "../../../crates/kui-native/src/decode.rs"]
mod decode;
mod effects;
pub use decode::{Animation, AnimationFrame, Pixels, decode_animation, decode_image};

#[allow(dead_code)]
#[path = "../../../crates/kui-native/src/unseen.rs"]
pub(crate) mod unseen;
mod gui {
    pub(crate) use crate::unseen;
}
#[path = "../../../crates/kui-native/src/testing.rs"]
pub mod testing;
pub type Drive<'a> = testing::Drive<&'a mut Core>;
pub use kui_core::DevtoolsDock as Dock;

#[path = "../../web/host.rs"]
mod host;

mod launch;
pub use launch::*;

include!(concat!(env!("OUT_DIR"), "/examples.rs"));

thread_local! {
    static SELECTED: RefCell<Option<BookApp>> = const { RefCell::new(None) };
    static DEMO_SIZE: StateCell<(f64, f64)> = const { StateCell::new((960.0, 640.0)) };
}

type AppModel = BookApp;

fn make_runner() -> kui_web::Runner<AppModel> {
    let app = SELECTED.with_borrow_mut(|slot| slot.take().expect("a book example was selected"));
    let mono = include_bytes!("../../../crates/kui-cli/assets/fonts/NotoSansMono-Regular.ttf");
    let fonts = host::bundled_fonts().into_iter().chain([mono.to_vec()]);
    let mut runner = kui_web::Runner::new(app, &Session::with_fonts("en-US", fonts));
    runner.core_mut().set_native_menus(false);
    runner.core_mut().set_native_menu_bar(false);
    runner
}

#[unsafe(no_mangle)]
pub extern "C" fn select_example() -> u32 {
    u32::from(launch(&host::browser_text()))
}

#[unsafe(no_mangle)]
pub extern "C" fn next_frame_at() -> f64 {
    host::RUNNER.with_borrow(|runner| runner.next_frame_at().unwrap_or(0.0))
}

#[unsafe(no_mangle)]
pub extern "C" fn demo_width() -> f64 {
    DEMO_SIZE.get().0
}

#[unsafe(no_mangle)]
pub extern "C" fn demo_height() -> f64 {
    DEMO_SIZE.get().1
}
