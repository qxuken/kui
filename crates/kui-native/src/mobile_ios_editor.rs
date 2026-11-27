use super::Proxy;
use super::{Document, KeyCode, KeyMods, KeyPress, MobileEvent, NativeText, Window};
use std::collections::HashMap;
use std::{
    cell::RefCell,
    ffi::{CStr, c_char, c_void},
    ptr::NonNull,
};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

thread_local! {
    static EVENTS: RefCell<HashMap<u64, Proxy>> = RefCell::default();
}

unsafe extern "C" {
    fn kui_ios_editor(
        view: *mut c_void,
        scene: u64,
        session: i64,
        kind: i32,
        bytes: *const u8,
        len: usize,
        anchor: i32,
        caret: i32,
        show: bool,
        refresh: bool,
    );
    fn kui_ios_copy_text(bytes: *const u8, len: usize);
    fn kui_ios_paste_text(context: *mut c_void, emit: extern "C" fn(*mut c_void, *const u8, usize));
    fn kui_ios_key_mode(view: *mut c_void, scene: u64, session: i64, enabled: bool);
}

fn view(window: &Window) -> Option<NonNull<c_void>> {
    let RawWindowHandle::UiKit(handle) = window.window_handle().ok()?.as_raw() else {
        return None;
    };
    Some(handle.ui_view)
}

pub(super) struct Editor(u64);
impl Editor {
    pub fn new(proxy: Proxy) -> Self {
        let id = proxy.scene;
        EVENTS.with_borrow_mut(|by_id| by_id.insert(id, proxy));
        Self(id)
    }
    pub fn set(
        &self,
        window: &Window,
        session: i64,
        kind: i32,
        doc: &Document,
        show: bool,
        refresh: bool,
    ) {
        if let Some(view) = view(window) {
            // The window owns this UIKit view on the event loop's main thread.
            unsafe {
                kui_ios_editor(
                    view.as_ptr(),
                    self.0,
                    session,
                    kind,
                    doc.text.as_ptr(),
                    doc.text.len(),
                    doc.selection.0,
                    doc.selection.1,
                    show,
                    refresh,
                )
            };
        }
    }

    pub(super) fn copy(&self, text: &str) {
        unsafe { kui_ios_copy_text(text.as_ptr(), text.len()) }
    }

    pub(super) fn paste(&self) -> String {
        extern "C" fn receive(context: *mut c_void, bytes: *const u8, len: usize) {
            // UIKit lends valid UTF-8 for this synchronous callback.
            let bytes = unsafe { std::slice::from_raw_parts(bytes, len) };
            if let Ok(value) = str::from_utf8(bytes) {
                unsafe { &mut *context.cast::<String>() }.push_str(value);
            }
        }
        let mut value = String::new();
        unsafe { kui_ios_paste_text((&raw mut value).cast(), receive) }
        value
    }

    pub fn key_mode(&self, window: &Window, session: i64, enabled: bool) {
        if let Some(view) = view(window) {
            // The window owns this UIKit view on the event loop's main thread.
            unsafe { kui_ios_key_mode(view.as_ptr(), self.0, session, enabled) };
        }
    }
}
impl Drop for Editor {
    fn drop(&mut self) {
        EVENTS.with_borrow_mut(|by_id| by_id.remove(&self.0));
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn kui_ios_text(
    scene: u64,
    session: i64,
    bytes: *const u8,
    len: usize,
    start: i32,
    end: i32,
    lo: i32,
    hi: i32,
) {
    // editor.m lends the UTF-8 bytes of an NSData for this call.
    let bytes = if len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(bytes, len) }
    };
    if let Ok(text) = std::str::from_utf8(bytes) {
        EVENTS.with_borrow(|proxy| {
            if let Some(proxy) = proxy.get(&scene) {
                let _ = proxy.send_event(MobileEvent::Ime(
                    session,
                    NativeText {
                        text: text.into(),
                        selection: (start, end),
                        marked: (lo, hi),
                        original: None,
                    },
                ));
            }
        });
    }
}

#[unsafe(no_mangle)]
extern "C" fn kui_ios_key(scene: u64, session: i64, enter: bool) {
    let code = if enter {
        super::KeyCode::Enter
    } else {
        super::KeyCode::Backspace
    };
    EVENTS.with_borrow(|proxy| {
        if let Some(proxy) = proxy.get(&scene) {
            let _ = proxy.send_event(MobileEvent::EditorKey(session, code));
        }
    });
}

#[unsafe(no_mangle)]
unsafe extern "C" fn kui_ios_hardware_key(
    scene: u64,
    session: i64,
    code: u32,
    text: *const c_char,
    modifiers: u32,
    pressed: bool,
) {
    // UIKit lends the characters for this call.
    let text = (!text.is_null())
        .then(|| unsafe { CStr::from_ptr(text) })
        .and_then(|text| text.to_str().ok())
        .unwrap_or_default();
    let mods = KeyMods::from_bits(modifiers);
    let physical = hid_code(code);
    let layout = match physical {
        KeyCode::Unknown | KeyCode::Char(_) => KeyCode::from_name(text).unwrap_or(physical),
        named => named,
    };
    let mut key = KeyPress::from_layout(layout, physical, mods);
    if pressed
        && matches!(
            physical,
            KeyCode::Char(_) | KeyCode::Space | KeyCode::Unknown
        )
        && !mods.super_key
        && !mods.ctrl
        && text.chars().any(|c| !c.is_control())
    {
        key.text = Some(text.into());
    }
    EVENTS.with_borrow(|proxy| {
        if let Some(proxy) = proxy.get(&scene) {
            let _ = proxy.send_event(MobileEvent::HardwareKey(session, key, pressed));
        }
    });
}

// UIKey.keyCode is a USB HID usage in the keyboard page.
fn hid_code(code: u32) -> KeyCode {
    match code {
        0x04..=0x1d => KeyCode::Char((b'a' + (code - 0x04) as u8).into()),
        0x1e..=0x27 => KeyCode::Char(b"1234567890"[(code - 0x1e) as usize].into()),
        0x28 | 0x58 => KeyCode::Enter,
        0x29 => KeyCode::Escape,
        0x2a => KeyCode::Backspace,
        0x2b => KeyCode::Tab,
        0x2c => KeyCode::Space,
        0x2d..=0x38 => KeyCode::Char(b"-=[]\\#;'\x60,./"[(code - 0x2d) as usize].into()),
        0x39 => KeyCode::CapsLock,
        0x3a..=0x45 => KeyCode::F((code - 0x39) as u8),
        0x49 => KeyCode::Insert,
        0x4a => KeyCode::Home,
        0x4b => KeyCode::PageUp,
        0x4c => KeyCode::Delete,
        0x4d => KeyCode::End,
        0x4e => KeyCode::PageDown,
        0x4f => KeyCode::Right,
        0x50 => KeyCode::Left,
        0x51 => KeyCode::Down,
        0x52 => KeyCode::Up,
        0xe0 | 0xe4 => KeyCode::Ctrl,
        0xe1 | 0xe5 => KeyCode::Shift,
        0xe2 | 0xe6 => KeyCode::Alt,
        0xe3 | 0xe7 => KeyCode::Super,
        _ => KeyCode::Unknown,
    }
}
