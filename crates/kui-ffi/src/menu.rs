//! Context menus (`docs/adr/0017-selection-as-a-scope.md`, decision 5):
//! opening one over a node, and closing it.
//!
//! There is no element here and no node to declare. The menu is state the
//! core holds and draws, so a host asks for one the way it asks for focus
//! — a call, not a declaration — and hears what was chosen as an ordinary
//! event on the node it named.

use super::*;

/// One row of a menu (`kui_open_menu`). [in], read while the call runs.
///
/// `label` may be empty for a standard `role`, which then reads the way
/// the platform words it. `id` is what the row posts when chosen; NULL
/// posts the row's text. `accel` is drawn right-aligned and bound to
/// nothing — the shortcut is the host's, and this only says which one.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct KuiMenuItem {
    pub label: KuiStr,
    /// A `KUI_MENU_*` role.
    pub role: u32,
    /// Zero disables the row: drawn dimmed, not focusable, not choosable.
    /// A row present-but-dead rather than absent, so a menu's shape does
    /// not change under the pointer.
    pub enabled: u32,
    pub id: *const KuiValue,
    pub accel: KuiStr,
}

impl KuiMenuItem {
    /// Reads one item. `None` for a role this build does not know, which
    /// is a host built against a newer header.
    fn to_core(self) -> Option<kui_core::MenuItem> {
        let role = match self.role {
            0 => kui_core::MenuRole::Custom,
            1 => kui_core::MenuRole::Separator,
            2 => kui_core::MenuRole::Cut,
            3 => kui_core::MenuRole::Copy,
            4 => kui_core::MenuRole::Paste,
            5 => kui_core::MenuRole::SelectAll,
            6 => kui_core::MenuRole::LookUp,
            _ => return None,
        };
        Some(kui_core::MenuItem {
            label: kstr(self.label).into_owned(),
            role,
            enabled: self.enabled != 0,
            id: unsafe { self.id.as_ref() }.map(|v| v.0.clone()),
            accel: opt_str(self.accel).map(|s| s.into_owned()),
        })
    }
}

/// Opens a context menu at `(x, y)` (logical viewport px) over `key`, with
/// `count` items read from `items`. The next frame draws it.
///
/// Choosing a row posts `{kind:"menu", role, item}` on `key` and closes
/// the menu; a press outside it or Escape closes it and posts nothing. The
/// standard roles the core can carry out it carries out — the clipboard
/// three come back through `kui_take_menu_actions` — and a `KUI_MENU_CUSTOM`
/// row is the host's to act on.
///
/// A key of 0, no items, or an item with an unknown role: nothing opens
/// and this returns false.
#[unsafe(no_mangle)]
pub extern "C" fn kui_open_menu(
    ptr: *mut KuiCtx,
    key: u64,
    x: f32,
    y: f32,
    items: *const KuiMenuItem,
    count: usize,
) -> bool {
    guard(false, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return false;
        };
        if key == 0 || items.is_null() || count == 0 {
            return false;
        }
        let rows = unsafe { std::slice::from_raw_parts(items, count) };
        let mut parsed = Vec::with_capacity(count);
        for row in rows {
            match row.to_core() {
                Some(item) => parsed.push(item),
                None => return false,
            }
        }
        c.core().open_menu(kui_core::Menu::new(
            Key(key),
            kui_core::Vec2::new(x, y),
            parsed,
        ));
        true
    })
}

/// Closes whatever menu is open; true when there was one.
#[unsafe(no_mangle)]
pub extern "C" fn kui_close_menu(ptr: *mut KuiCtx) -> bool {
    guard(false, || {
        unsafe { ctx(ptr) }.is_some_and(|c| c.core().close_menu())
    })
}

/// Drains what choosing a menu row left for the host: the clipboard, which
/// is the host's in this library. `KUI_MENU_ACTION_SET_CLIPBOARD` carries
/// the text to put there — the core worked out *what*, which is the half
/// only it can do — and `KUI_MENU_ACTION_PASTE` asks for what is there,
/// which a host delivers back with `kui_input_text`.
///
/// Writes one action into `out` and returns true; false when the queue is
/// empty. `text` is borrowed until the next call on this context.
#[unsafe(no_mangle)]
pub extern "C" fn kui_take_menu_action(ptr: *mut KuiCtx, out: *mut KuiMenuAction) -> bool {
    guard(false, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return false;
        };
        if c.menu_actions.is_empty() {
            c.menu_actions = c.core().take_menu_actions().into();
        }
        let Some(action) = c.menu_actions.pop_front() else {
            return false;
        };
        let (kind, text) = match action {
            kui_core::MenuAction::SetClipboard(t) => (0u32, t),
            kui_core::MenuAction::Paste => (1u32, String::new()),
        };
        c.menu_text = text;
        let written = KuiMenuAction {
            size: std::mem::size_of::<KuiMenuAction>() as u32,
            kind,
            text: KuiStr {
                ptr: c.menu_text.as_ptr(),
                len: c.menu_text.len(),
            },
        };
        write_out(out, written)
    })
}
