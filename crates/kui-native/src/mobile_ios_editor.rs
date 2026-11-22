use super::Proxy;
use super::{Document, MobileEvent, NativeText, Window};
use std::collections::HashMap;
use std::{cell::RefCell, ffi::c_void};
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
        if let Ok(handle) = window.window_handle()
            && let RawWindowHandle::UiKit(handle) = handle.as_raw()
        {
            // The window owns this UIKit view on the event loop's main thread.
            unsafe {
                kui_ios_editor(
                    handle.ui_view.as_ptr(),
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
