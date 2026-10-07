//! Context menus: opening one over a node, and closing it.
//!
//! There is no element here and no node to declare. The menu is state the
//! core holds and draws, so a host asks for one the way it asks for focus
//! — a call, not a declaration — and hears what was chosen as an ordinary
//! event on the node it named.

use super::*;

/// One row of a menu ([`kui_open_menu`], [`kui_menu_bar`],
/// [`kui_select`]), read while the call runs.
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
    /// Non-zero draws a checkmark beside the row (and sets the platform's
    /// own check state where the host renders the menu): a setting the row
    /// *is*, not a command it runs.
    pub checked: u32,
}

impl KuiMenuItem {
    /// Reads one item. `None` for a role this build does not know, which
    /// is a host built against a newer header.
    pub(crate) fn to_core(self) -> Option<kui_core::MenuItem> {
        let role = *kui_core::MenuRole::ALL.get(self.role as usize)?;
        Some(kui_core::MenuItem {
            label: kstr(self.label).into_owned(),
            role,
            enabled: self.enabled != 0,
            checked: self.checked != 0,
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

/// Tells the core this host can show the platform's definition panel
/// (macOS's Look Up). The standard Look Up row is then offered where it
/// means something, and a force click over text asks for one; without
/// this the core neither offers nor asks, because a row that does nothing
/// is worse than a row that is not there.
#[unsafe(no_mangle)]
pub extern "C" fn kui_set_lookup_available(ptr: *mut KuiCtx, on: bool) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().set_lookup_available(on);
        }
    });
}

/// Tells the core that this host shows menus itself — the platform's own,
/// however it draws them. The core then keeps the open menu as state and
/// draws none of it: read it with `kui_menu_items`, show it, and report
/// back with `kui_activate_menu_item` or `kui_close_menu`.
#[unsafe(no_mangle)]
pub extern "C" fn kui_set_native_menus(ptr: *mut KuiCtx, on: bool) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().set_native_menus(on);
        }
    });
}

/// Reports that the host's menu chose row `index`: the same path a press
/// on the drawn menu's row takes. An index past the end closes the menu
/// and posts nothing. Returns false when nothing was taken: no menu was
/// open, or the row cannot be chosen — disabled, or a separator — in
/// which case the menu stays open and nothing is posted.
#[unsafe(no_mangle)]
pub extern "C" fn kui_activate_menu_item(ptr: *mut KuiCtx, index: usize) -> bool {
    guard(false, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return false;
        };
        let Some(events) = c.core().activate_menu_item(index) else {
            return false;
        };
        c.absorb(events);
        true
    })
}

/// Asks for the selection as text.
///
/// `KUI_COPY_READY` writes the selection into `out` — borrowed until the
/// next call on this context. `KUI_COPY_ASKED` means the selection reaches
/// rows a virtual list never built: a `{kind:"selectionrange", from, to}`
/// event is waiting in the queue, the rows behind that gap are the host's,
/// and the host answers with `kui_answer_selection_range`, whose text then
/// arrives as a `KUI_MENU_ACTION_SET_CLIPBOARD`. `KUI_COPY_NOTHING` is
/// nothing selected.
#[unsafe(no_mangle)]
pub extern "C" fn kui_request_copy(ptr: *mut KuiCtx, out: *mut KuiStr) -> u32 {
    guard(2, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return 2;
        };
        match c.core().request_copy() {
            kui_core::CopyRequest::Ready(text) => {
                c.copy_text = text;
                if !out.is_null() {
                    unsafe {
                        out.write(KuiStr {
                            ptr: c.copy_text.as_ptr(),
                            len: c.copy_text.len(),
                        })
                    };
                }
                KUI_COPY_READY
            }
            kui_core::CopyRequest::Asked => KUI_COPY_ASKED,
            kui_core::CopyRequest::Nothing => KUI_COPY_NOTHING,
        }
    })
}

/// Answers a `selectionrange` ask with the text for the range it named,
/// whole. False when nothing asked — a late answer cannot overwrite what
/// has been copied since.
#[unsafe(no_mangle)]
pub extern "C" fn kui_answer_selection_range(ptr: *mut KuiCtx, text: KuiStr) -> bool {
    guard(false, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return false;
        };
        let text = kstr(text).into_owned();
        c.core().answer_selection_range(&text)
    })
}

/// Puts `text` on the system clipboard, as a `KUI_MENU_ACTION_SET_CLIPBOARD`
/// the host drains: the action a menu's Copy queues, callable from an
/// `on_key` sink that hears the raw `Ctrl-c`. `html` is a second flavour
/// beside the text,
/// never in place of it; an empty `html` is none. Under `kui_run` the
/// runner applies it after every input and every frame.
#[unsafe(no_mangle)]
pub extern "C" fn kui_set_clipboard(ptr: *mut KuiCtx, text: KuiStr, html: KuiStr) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            let text = kstr(text).into_owned();
            let html = kstr(html).into_owned();
            c.core()
                .set_clipboard(text, (!html.is_empty()).then_some(html));
        }
    });
}

/// Puts a secret on the system clipboard, as a
/// `KUI_MENU_ACTION_SET_CLIPBOARD_SECRET` the host drains and writes
/// marked concealed and transient, the way a password manager does:
/// `org.nspasteboard.ConcealedType` and `TransientType`
/// on macOS, the exclusion formats on Windows — so no clipboard manager
/// shows or keeps it. Under `kui_run` the runner writes it.
#[unsafe(no_mangle)]
pub extern "C" fn kui_set_clipboard_secret(ptr: *mut KuiCtx, text: KuiStr) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            let text = kstr(text).into_owned();
            c.core().set_clipboard_secret(text);
        }
    });
}

/// Asks for what is on the clipboard, as a `KUI_MENU_ACTION_PASTE` the
/// host drains and answers with [`kui_input_paste`] (the text and the
/// pasteboard's `KUI_PASTE_*` markers) or [`kui_input_commit`]: a focused
/// editor takes
/// the text as typing, a focused `on_key` sink hears it as
/// `{kind:"text", text, tag}` — so an app that owns its text inserts a
/// paste the way it inserts a committed IME string, and the clipboard is
/// read on the host's side, where the permission lives. Under `kui_run`
/// the runner does both halves.
#[unsafe(no_mangle)]
pub extern "C" fn kui_request_paste(ptr: *mut KuiCtx) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().request_paste();
        }
    });
}

/// Whether a paste asked for is still unanswered: `kui_request_paste`
/// queues one ask at a time, and the `kui_input_commit` that answers it
/// (an empty one for an empty clipboard) is what lets the next through.
#[unsafe(no_mangle)]
pub extern "C" fn kui_awaiting_paste(ptr: *mut KuiCtx) -> bool {
    guard(false, || {
        unsafe { ctx(ptr) }.is_some_and(|c| c.core().awaiting_paste())
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
/// which a host delivers back with `kui_input_commit` (an editor takes it
/// as typing; a sink hears it as `{kind:"text"}`).
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
        // Checked before the action leaves the queue, as kui_poll_event
        // does: an `out` this library cannot write keeps it for the next
        // call rather than dropping a clipboard write.
        if !out_accepts(out) {
            return false;
        }
        let Some(action) = c.menu_actions.pop_front() else {
            return false;
        };
        let mut at = kui_core::Vec2::new(0.0, 0.0);
        let (kind, text, html) = match action {
            kui_core::MenuAction::SetClipboard { text, html } => {
                (KUI_MENU_ACTION_SET_CLIPBOARD, text, html)
            }
            kui_core::MenuAction::SetClipboardSecret { text } => {
                (KUI_MENU_ACTION_SET_CLIPBOARD_SECRET, text, None)
            }
            kui_core::MenuAction::Paste => (KUI_MENU_ACTION_PASTE, String::new(), None),
            // The core only asks for a panel a host said it can show
            // (`kui_set_lookup_available`), so this arrives exactly where
            // a host is ready for it.
            kui_core::MenuAction::LookUp { text, at: point } => {
                at = point;
                (KUI_MENU_ACTION_LOOK_UP, text, None)
            }
        };
        c.menu_text = text;
        c.menu_html = html.unwrap_or_default();
        let written = KuiMenuAction {
            size: std::mem::size_of::<KuiMenuAction>() as u32,
            kind,
            text: KuiStr {
                ptr: c.menu_text.as_ptr(),
                len: c.menu_text.len(),
            },
            html: KuiStr {
                ptr: c.menu_html.as_ptr(),
                len: c.menu_html.len(),
            },
            x: at.x,
            y: at.y,
        };
        write_out(out, written)
    })
}

// -- The application menu bar -----------------------------------------------

/// One menu of the application menu bar ([`kui_menu_bar`]), read while
/// the call runs; the core copies what it needs.
///
/// `items` is `count` rows in the same `KuiMenuItem` a context menu takes,
/// which is the point: an Edit menu's Copy is the same row the right-click
/// Copy is, and the core performs it the same way.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct KuiMenu {
    pub label: KuiStr,
    pub items: *const KuiMenuItem,
    pub count: usize,
    /// Zero disables the whole menu: dimmed, and it opens nothing.
    pub enabled: u32,
}

/// The application menu for this frame: `count` menus read from `menus`,
/// in bar order, declared and, where the platform has no menu bar of its
/// own, drawn into the frame right here as a row of titles that drop
/// their menus.
///
/// One call and not two, because what the menu is and where its strip goes
/// are one decision. Where the platform owns the bar
/// (`kui_set_native_menu_bar`) nothing is drawn and the declaration still
/// stands, so a host calls this unconditionally and is portable; a host
/// with a bar of its own reads the declaration back with
/// `kui_menu_bar_menu_count` / `_menu` / `_item` and reports a choice with
/// `kui_activate_menu_bar_item`.
///
/// Sticky and diffed, like `kui_window_title`: a frame that does not call
/// this leaves the last declaration in force, the same declaration again
/// changes nothing, and `count == 0` takes the menu away. Choosing an item
/// posts `{kind:"menu", role, item}` — the same event the context menu
/// posts — on the bar's own node, or on the root where the platform drew
/// it. Returns false, declaring and drawing nothing, for an item with a
/// role this build does not know. Call between `kui_frame_begin` and
/// `kui_frame_finish`.
#[unsafe(no_mangle)]
pub extern "C" fn kui_menu_bar(ptr: *mut KuiCtx, menus: *const KuiMenu, count: usize) -> bool {
    guard(false, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return false;
        };
        let mut out = Vec::with_capacity(count);
        if !menus.is_null() {
            for menu in unsafe { std::slice::from_raw_parts(menus, count) } {
                let mut items = Vec::with_capacity(menu.count);
                if !menu.items.is_null() {
                    for row in unsafe { std::slice::from_raw_parts(menu.items, menu.count) } {
                        match row.to_core() {
                            Some(item) => items.push(item),
                            None => return false,
                        }
                    }
                }
                out.push(kui_core::BarMenu {
                    label: kstr(menu.label).into_owned(),
                    items,
                    enabled: menu.enabled != 0,
                });
            }
        }
        let mut ui = kui_core::Ui::wrap(c.core());
        kui_core::widgets::menu_bar(&mut ui, kui_core::MenuBar::new(out));
        true
    })
}

/// Tells the core that the platform owns the menu bar, so `kui_menu_bar`
/// draws nothing and the host is the one that hands the declaration over
/// (`kui_menu_bar_menu_count` / `kui_menu_bar_item` read it back) and
/// reports what was chosen with `kui_activate_menu_bar_item`.
///
/// Off by default: a host that says nothing draws its own bar.
#[unsafe(no_mangle)]
pub extern "C" fn kui_set_native_menu_bar(ptr: *mut KuiCtx, on: bool) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().set_native_menu_bar(on);
        }
    });
}

/// How many menus the declaration in force has, and a revision that
/// changes only when the declaration does — a host with a native bar
/// keeps the last number it built and rebuilds nothing until it moves.
/// Writes the revision into `revision` when it is not NULL.
#[unsafe(no_mangle)]
pub extern "C" fn kui_menu_bar_menu_count(ptr: *mut KuiCtx, revision: *mut u64) -> usize {
    guard(0, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return 0;
        };
        if !revision.is_null() {
            unsafe { revision.write(c.core().menu_bar_revision()) };
        }
        c.core().menu_bar().map_or(0, |b| b.menus.len())
    })
}

/// Reads one menu of the declaration back: its title into `label` and how
/// many rows it has. Both borrowed until the next call on this context.
/// Returns false for a menu past the end.
#[unsafe(no_mangle)]
pub extern "C" fn kui_menu_bar_menu(
    ptr: *mut KuiCtx,
    menu: usize,
    label: *mut KuiStr,
    enabled: *mut bool,
) -> usize {
    guard(0, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return 0;
        };
        let Some(m) = c.core().menu_bar().and_then(|b| b.menus.get(menu)) else {
            return 0;
        };
        let (text, on, count) = (m.label.clone(), m.enabled, m.items.len());
        c.row_text = text;
        if !label.is_null() {
            unsafe {
                label.write(KuiStr {
                    ptr: c.row_text.as_ptr(),
                    len: c.row_text.len(),
                })
            };
        }
        if !enabled.is_null() {
            unsafe { enabled.write(on) };
        }
        count
    })
}

/// Reads one row of one menu: its text into `label`, its accelerator into
/// `accel` (empty when it has none), and its role and flags through the
/// out pointers. Both strings are borrowed until the next call on this
/// context. False for a row that is not there.
#[unsafe(no_mangle)]
pub extern "C" fn kui_menu_bar_item(
    ptr: *mut KuiCtx,
    menu: usize,
    item: usize,
    label: *mut KuiStr,
    accel: *mut KuiStr,
    role: *mut u32,
    flags: *mut u32,
) -> bool {
    guard(false, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return false;
        };
        let Some(row) = c.core().menu_bar().and_then(|b| b.item(menu, item)) else {
            return false;
        };
        let row = row.clone();
        write_row(c, &row, label, accel, role, flags);
        true
    })
}

/// One row of either menu, spelled the one way a host reads a row: what
/// the drawn menu would show (`MenuItem::text`, `MenuItem::accel_text` —
/// the role's default where the row declared none), the `KUI_MENU_*` role
/// and the `KUI_MENU_ITEM_*` flags. Shared by `kui_menu_bar_item` and
/// `kui_menu_item` so the bar and the context menu cannot read one item
/// two ways. Any out pointer may be NULL; the strings are borrowed until
/// the next call on this context.
fn write_row(
    c: &mut KuiCtx,
    row: &kui_core::MenuItem,
    label: *mut KuiStr,
    accel: *mut KuiStr,
    role: *mut u32,
    flags: *mut u32,
) {
    c.row_text = row.text().to_string();
    c.menu_accel = row.accel_text().unwrap_or_default().to_string();
    if !label.is_null() {
        unsafe {
            label.write(KuiStr {
                ptr: c.row_text.as_ptr(),
                len: c.row_text.len(),
            })
        };
    }
    if !accel.is_null() {
        unsafe {
            accel.write(KuiStr {
                ptr: c.menu_accel.as_ptr(),
                len: c.menu_accel.len(),
            })
        };
    }
    if !role.is_null() {
        unsafe { role.write(role_code(row.role)) };
    }
    if !flags.is_null() {
        let mut bits = 0;
        if row.enabled {
            bits |= KUI_MENU_ITEM_ENABLED;
        }
        if row.checked {
            bits |= KUI_MENU_ITEM_CHECKED;
        }
        unsafe { flags.write(bits) };
    }
}

/// The menu this window has open, for a host that said it shows menus
/// itself (`kui_set_native_menus`): how many rows it has, writing the node
/// it is about into `target` and where it opened (logical viewport px)
/// into `x` / `y` — any of the three may be NULL. Zero when none is open,
/// which is unambiguous because a menu never opens with no rows. What
/// Node's `menu()` and `Core::menu` answer, so a C host is not the one
/// binding told to read what is open and given nothing to read it with.
#[unsafe(no_mangle)]
pub extern "C" fn kui_menu_item_count(
    ptr: *mut KuiCtx,
    target: *mut u64,
    x: *mut f32,
    y: *mut f32,
) -> usize {
    guard(0, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return 0;
        };
        let Some(menu) = c.core().menu() else {
            return 0;
        };
        if !target.is_null() {
            unsafe { target.write(menu.target.0) };
        }
        if !x.is_null() {
            unsafe { x.write(menu.at.x) };
        }
        if !y.is_null() {
            unsafe { y.write(menu.at.y) };
        }
        menu.items.len()
    })
}

/// Reads row `item` of the open menu, spelled exactly as `kui_menu_bar_item`
/// spells a bar's row. False for a row that is not there, including when
/// no menu is open. Answer with `kui_activate_menu_item` or
/// `kui_close_menu`.
#[unsafe(no_mangle)]
pub extern "C" fn kui_menu_item(
    ptr: *mut KuiCtx,
    item: usize,
    label: *mut KuiStr,
    accel: *mut KuiStr,
    role: *mut u32,
    flags: *mut u32,
) -> bool {
    guard(false, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return false;
        };
        let Some(row) = c.core().menu().and_then(|m| m.items.get(item)) else {
            return false;
        };
        let row = row.clone();
        write_row(c, &row, label, accel, role, flags);
        true
    })
}

/// Reports that the platform's menu bar chose row `item` of menu `menu`:
/// the same path a press on the drawn bar's row takes. Out of range does
/// nothing. Returns whether an item was performed.
#[unsafe(no_mangle)]
pub extern "C" fn kui_activate_menu_bar_item(ptr: *mut KuiCtx, menu: usize, item: usize) -> bool {
    guard(false, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return false;
        };
        let events = c.core().activate_menu_bar_item(menu, item);
        let any = !events.is_empty();
        c.absorb(events);
        any
    })
}

/// The `KUI_MENU_*` code for a role, the inverse of `KuiMenuItem::to_core`.
fn role_code(role: kui_core::MenuRole) -> u32 {
    kui_core::MenuRole::ALL
        .iter()
        .position(|r| *r == role)
        .expect("every role is in MenuRole::ALL") as u32
}
