//! The small Wasm ABI shared by the browser examples.

use std::cell::RefCell;
#[cfg(feature = "rust-wgpu")]
use wasm_bindgen::prelude::*;

use kui_core::{
    ClipboardMarks, EditKey, InputEvent, Key, KeyCode, KeyMods, KeyPress, Mods, MouseButton,
    Platform, Session, Size, Vec2,
};

pub(crate) fn bundled_fonts() -> [Vec<u8>; 2] {
    [
        include_bytes!("../../crates/kui-cli/assets/fonts/NotoSans-Regular.ttf").to_vec(),
        include_bytes!("fonts/NotoSansSymbols2-Keyboard.ttf").to_vec(),
    ]
}

#[allow(dead_code)]
pub fn session() -> Session {
    Session::with_fonts("en-US", bundled_fonts())
}

thread_local! {
    pub(crate) static RUNNER: RefCell<kui_web::Runner<crate::AppModel>> = RefCell::new(crate::make_runner());
    static TEXT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

#[cfg(not(feature = "rust-wgpu"))]
#[unsafe(no_mangle)]
pub extern "C" fn frame(width: f32, height: f32, scale: f32, time: f64, redraw: u32) -> u32 {
    RUNNER.with_borrow_mut(|runner| {
        if redraw != 0 {
            runner.invalidate_canvas();
        }
        u32::from(runner.frame(Size::new(width, height), scale, time))
    })
}

#[cfg(feature = "rust-wgpu")]
#[wasm_bindgen]
pub async fn attach_wgpu(
    canvas: web_sys::HtmlCanvasElement,
    access: js_sys::Function,
    clipboard: js_sys::Function,
) -> Result<(), JsValue> {
    let painter = kui_web::WgpuPainter::new(canvas)
        .await
        .map_err(|err| JsValue::from_str(&err.to_string()))?;
    kui_web::set_browser_callbacks(access, clipboard);
    RUNNER.with_borrow_mut(|runner| runner.attach_wgpu(painter));
    Ok(())
}

#[cfg(feature = "rust-wgpu")]
#[wasm_bindgen]
pub fn frame_wgpu(
    width: f32,
    height: f32,
    scale: f32,
    time: f64,
    backing_width: u32,
    backing_height: u32,
) -> Result<u32, JsValue> {
    let result = RUNNER.with_borrow_mut(|runner| {
        runner.frame_wgpu(
            Size::new(width, height),
            scale,
            time,
            [backing_width, backing_height],
        )
    });
    match result {
        Ok(next) => Ok(u32::from(next)),
        Err(kui_web::RenderError::DeviceLost) => Ok(2),
        Err(err) => Err(JsValue::from_str(&err.to_string())),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn pointer_move(x: f32, y: f32, time: f64) -> u32 {
    RUNNER.with_borrow_mut(|runner| {
        runner.input(InputEvent::CursorMoved(Vec2::new(x, y)), time);
        u32::from(runner.pointer_cursor())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn pointer_leave(time: f64) {
    RUNNER.with_borrow_mut(|runner| runner.input(InputEvent::CursorLeft, time));
}

#[unsafe(no_mangle)]
pub extern "C" fn pointer_button(secondary: u32, down: u32, time: f64, clicks: u32) {
    let button = if secondary == 0 {
        MouseButton::Primary
    } else {
        MouseButton::Secondary
    };
    let event = if down == 0 {
        InputEvent::MouseUp { button }
    } else {
        InputEvent::MouseDown {
            button,
            clicks: clicks.clamp(1, u32::from(u8::MAX)) as u8,
        }
    };
    RUNNER.with_borrow_mut(|runner| runner.input(event, time));
}

#[unsafe(no_mangle)]
pub extern "C" fn scroll(x: f32, y: f32, time: f64) {
    RUNNER.with_borrow_mut(|runner| runner.input(InputEvent::Scroll(Vec2::new(x, y)), time));
}

#[unsafe(no_mangle)]
pub extern "C" fn prefers_scroll(x: f32, y: f32) -> u32 {
    RUNNER.with_borrow(|runner| u32::from(runner.prefers_scroll_at(Vec2::new(x, y))))
}

#[unsafe(no_mangle)]
pub extern "C" fn scroll_gesture(x: f32, y: f32, begins: u32, time: f64) {
    RUNNER.with_borrow_mut(|runner| {
        runner.input(
            InputEvent::ScrollGesture {
                delta: Vec2::new(x, y),
                begins: begins != 0,
            },
            time,
        );
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn escape(time: f64) {
    let key = KeyPress::new(KeyCode::Escape, KeyMods::NONE);
    RUNNER.with_borrow_mut(|runner| runner.keystroke(key, time));
}

#[unsafe(no_mangle)]
pub extern "C" fn set_mac_conventions(mac: u32) {
    let platform = if mac == 0 {
        Platform::Other
    } else {
        Platform::MacOs
    };
    RUNNER.with_borrow_mut(|runner| runner.set_platform(platform));
}

#[unsafe(no_mangle)]
pub extern "C" fn mirror_access() {
    RUNNER.with_borrow_mut(|runner| runner.mirror_access());
}

#[unsafe(no_mangle)]
pub extern "C" fn focus_node(key: u64, time: f64) {
    RUNNER.with_borrow_mut(|runner| runner.focus(Key(key), time));
}

#[unsafe(no_mangle)]
pub extern "C" fn activate_node(key: u64, time: f64) {
    RUNNER.with_borrow_mut(|runner| runner.activate(Key(key), time));
}

#[unsafe(no_mangle)]
pub extern "C" fn blur(time: f64) {
    RUNNER.with_borrow_mut(|runner| runner.blur(time));
}

#[unsafe(no_mangle)]
pub extern "C" fn window_focus(focused: u32, time: f64) {
    RUNNER.with_borrow_mut(|runner| runner.window_focus(focused != 0, time));
}

#[unsafe(no_mangle)]
pub extern "C" fn clipboard_selection(cut: u32, time: f64) {
    RUNNER.with_borrow_mut(|runner| runner.clipboard_selection(cut != 0, time));
}

#[unsafe(no_mangle)]
pub extern "C" fn select_text(anchor: usize, focus: usize, time: f64) {
    RUNNER.with_borrow_mut(|runner| runner.select_editor_text(anchor, focus, time));
}

#[unsafe(no_mangle)]
pub extern "C" fn text_buffer(len: usize) -> *mut u8 {
    TEXT.with_borrow_mut(|bytes| {
        bytes.resize(len, 0);
        bytes.as_mut_ptr()
    })
}

#[allow(dead_code)]
pub(crate) fn publish_browser_bytes(bytes: Vec<u8>) -> usize {
    let len = bytes.len();
    TEXT.with_borrow_mut(|buffer| *buffer = bytes);
    len
}

pub(crate) fn browser_text() -> String {
    TEXT.with_borrow(|bytes| String::from_utf8_lossy(bytes).into_owned())
}

// Wire values shared with textMode in the browser editor.
const INSERT_TEXT: u32 = 0;
const PREEDIT_TEXT: u32 = 1;
const COMMIT_TEXT: u32 = 2;
const PASTE_TEXT: u32 = 3;
const REPLACE_VALUE: u32 = 4;

#[unsafe(no_mangle)]
pub extern "C" fn text_input(kind: u32, time: f64) {
    let text = browser_text();
    RUNNER.with_borrow_mut(|runner| match kind {
        INSERT_TEXT => runner.input(InputEvent::Text(text), time),
        PREEDIT_TEXT => runner.input(InputEvent::Preedit(text, None), time),
        COMMIT_TEXT => runner.input(InputEvent::Commit(text), time),
        PASTE_TEXT => runner.input(
            InputEvent::Paste {
                text,
                marks: ClipboardMarks::default(),
            },
            time,
        ),
        REPLACE_VALUE => runner.replace_editor_value(text, time),
        _ => {}
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn edit_key(modifiers: u32, time: f64) {
    let name = browser_text();
    if let Some(key) = EditKey::from_name(&name) {
        let mods = Mods {
            shift: modifiers & 1 != 0,
            word: modifiers & 2 != 0,
            doc: modifiers & 4 != 0,
            ctrl: modifiers & 8 != 0,
        };
        RUNNER.with_borrow_mut(|runner| runner.input(InputEvent::Key(key, mods), time));
    }
}
