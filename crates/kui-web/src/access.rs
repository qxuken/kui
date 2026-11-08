//! The part of the core access tree consumed by the browser's DOM mirror.

use kui_core::{AccessAction, AccessNode, Core, Rect, Role, TextPos};
use serde_json::json;

#[cfg(not(feature = "rust-wgpu"))]
#[link(wasm_import_module = "canvas")]
unsafe extern "C" {
    fn access_tree(bytes: *const u8, len: usize);
    fn clipboard_text(bytes: *const u8, len: usize);
}

#[cfg(feature = "rust-wgpu")]
thread_local! {
    static CALLBACKS: std::cell::RefCell<Option<(js_sys::Function, js_sys::Function)>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(feature = "rust-wgpu")]
pub fn set_browser_callbacks(access: js_sys::Function, clipboard: js_sys::Function) {
    CALLBACKS.set(Some((access, clipboard)));
}

#[derive(Default)]
pub(crate) struct Mirror {
    last: Option<(u64, Option<Rect>)>,
}

impl Mirror {
    pub fn publish(&mut self, core: &mut Core) {
        let caret = core.ime_rect();
        let tree = core.access_tree();
        let revision = (tree.hash, caret);
        if self.last == Some(revision) {
            return;
        }
        let nodes: Vec<_> = tree
            .nodes
            .iter()
            .map(|node| {
                let editor = matches!(node.role, Role::TextInput | Role::MultilineTextInput);
                let value = editor.then(|| node.value.as_deref().unwrap_or(""));
                let selection = value.map(|value| {
                    let units = |byte| value.get(..byte).unwrap_or(value).encode_utf16().count();
                    let caret = node.caret.unwrap_or(value.len());
                    let (start, end) = node.selection.unwrap_or((caret, caret));
                    let direction = if start == end {
                        "none"
                    } else if caret == start {
                        "backward"
                    } else {
                        "forward"
                    };
                    (units(start), units(end), direction)
                });
                let rect = node.rect;
                json!({
                    "key": node.key.0.to_string(),
                    "parent": node.parent.map(|key| key.0.to_string()),
                    "role": node.role.name(),
                    "name": node.name,
                    "description": node.description,
                    "rect": [rect.x, rect.y, rect.w, rect.h],
                    "value": value,
                    "selection": selection,
                    "focusable": node.supports(AccessAction::Focus),
                    "disabled": node.disabled,
                    "selected": node.selected,
                    "live": node.live.name(),
                })
            })
            .collect();
        let data = json!({
            "nodes": nodes,
            "focus": tree.focus.map(|key| key.0.to_string()),
            "caret": caret.map(|r| [r.x, r.y, r.w, r.h]),
        })
        .to_string();
        write_to_browser(&data, false);
        self.last = Some(revision);
    }
}

/// DOM offsets count UTF-16 code units. The core addresses UTF-8 bytes in
/// each logical line, then converts those into access tree run positions.
pub(crate) fn text_position(node: &AccessNode, mut units: usize) -> Option<TextPos> {
    for (line, text) in node.value.as_deref()?.split('\n').enumerate() {
        let len = text.encode_utf16().count();
        if units <= len {
            for (byte, ch) in text.char_indices() {
                if units < ch.len_utf16() {
                    return node.text_pos(line, byte);
                }
                units -= ch.len_utf16();
            }
            return node.text_pos(line, text.len());
        }
        units -= len + 1;
    }
    None
}

pub(crate) fn copy_to_browser(text: &str) {
    write_to_browser(text, true);
}

fn write_to_browser(text: &str, clipboard: bool) {
    // SAFETY: The browser import decodes the borrowed string before returning.
    #[cfg(not(feature = "rust-wgpu"))]
    unsafe {
        if clipboard {
            clipboard_text(text.as_ptr(), text.len());
        } else {
            access_tree(text.as_ptr(), text.len());
        }
    }
    #[cfg(feature = "rust-wgpu")]
    CALLBACKS.with_borrow(|callbacks| {
        use wasm_bindgen::JsValue;
        let (access, copy) = callbacks
            .as_ref()
            .expect("browser callbacks installed at startup");
        let callback = if clipboard { copy } else { access };
        callback
            .call2(
                &JsValue::UNDEFINED,
                &JsValue::from(text.as_ptr() as u32),
                &JsValue::from(text.len() as u32),
            )
            .expect("browser callback failed");
    });
}
