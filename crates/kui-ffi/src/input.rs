//! Input in, events out: the pointer, wheel, key, text and preedit entry
//! points, the event poll, the cursor shape, and the editor text
//! a host reads back or replaces.

use super::*;

pub(crate) fn push_input(ptr: *mut KuiCtx, ev: InputEvent) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            let evs = c.core().handle_input(ev);
            c.absorb(evs);
        }
    })
}

/// Cursor position in logical coordinates.
#[unsafe(no_mangle)]
pub extern "C" fn kui_input_cursor(ptr: *mut KuiCtx, x: f32, y: f32) {
    push_input(ptr, InputEvent::CursorMoved(Vec2::new(x, y)));
}

#[unsafe(no_mangle)]
pub extern "C" fn kui_input_cursor_left(ptr: *mut KuiCtx) {
    push_input(ptr, InputEvent::CursorLeft);
}

/// A primary-button press or release; `kui_input_mouse_button` carries the
/// others. Kept as it was: it is exported ABI.
#[unsafe(no_mangle)]
pub extern "C" fn kui_input_mouse(ptr: *mut KuiCtx, down: bool, clicks: u32) {
    kui_input_mouse_button(ptr, down, MouseButton::Primary.code(), clicks);
}

/// `kui_input_mouse` for a named button (`KUI_MOUSE_*`, or `3 + n` for a
/// further button `n`).
#[unsafe(no_mangle)]
pub extern "C" fn kui_input_mouse_button(ptr: *mut KuiCtx, down: bool, button: u32, clicks: u32) {
    let button = MouseButton::from_code(button);
    push_input(
        ptr,
        if down {
            InputEvent::MouseDown {
                button,
                clicks: clicks.clamp(1, u8::MAX as u32) as u8,
            }
        } else {
            InputEvent::MouseUp { button }
        },
    );
}

/// Wheel/trackpad delta in logical px (positive y = scroll up).
#[unsafe(no_mangle)]
pub extern "C" fn kui_input_scroll(ptr: *mut KuiCtx, dx: f32, dy: f32) {
    push_input(ptr, InputEvent::Scroll(Vec2::new(dx, dy)));
}

/// Committed text input (typing, paste); routed to the focused editor.
#[unsafe(no_mangle)]
pub extern "C" fn kui_input_text(ptr: *mut KuiCtx, text: KuiStr) {
    guard((), || {
        let text = kstr(text).into_owned();
        push_input(ptr, InputEvent::Text(text));
    });
}

/// The paths a file-drag door carries: `count` borrowed `KuiStr`s, read
/// once and owned by the event.
fn paths_of(paths: *const KuiStr, count: usize) -> Vec<String> {
    if paths.is_null() || count == 0 {
        return Vec::new();
    }
    // SAFETY: the host promises `count` strings at `paths` for the call.
    unsafe { std::slice::from_raw_parts(paths, count) }
        .iter()
        .map(|s| kstr(*s).into_owned())
        .collect()
}

/// Files dragged in from the OS are over the window at (`x`, `y`) —
/// entering and moving alike (ADR 0031, decision 4): the zone under the
/// point hears `{kind:"drop", phase:"enter"|"move"}`, a zone it left
/// hears `leave`. `paths` are `count` OS paths. The driver's answer to
/// the OS (copy over a zone, not-allowed elsewhere) is `kui_drop_target`.
#[unsafe(no_mangle)]
pub extern "C" fn kui_input_drag_files(
    ptr: *mut KuiCtx,
    paths: *const KuiStr,
    count: usize,
    x: f32,
    y: f32,
) {
    guard((), || {
        let paths = paths_of(paths, count);
        push_input(
            ptr,
            InputEvent::DragFiles {
                paths,
                at: Vec2::new(x, y),
            },
        );
    });
}

/// The dragged files were released at (`x`, `y`): the zone there hears
/// `{kind:"drop", phase:"drop", paths, x, y, tag}` and no `leave` after;
/// with no zone there, nothing but the lit zone's `leave`.
#[unsafe(no_mangle)]
pub extern "C" fn kui_input_drop_files(
    ptr: *mut KuiCtx,
    paths: *const KuiStr,
    count: usize,
    x: f32,
    y: f32,
) {
    guard((), || {
        let paths = paths_of(paths, count);
        push_input(
            ptr,
            InputEvent::DropFiles {
                paths,
                at: Vec2::new(x, y),
            },
        );
    });
}

/// The dragged files left the window, or the OS ended the drag
/// elsewhere: the lit zone hears its `leave`.
#[unsafe(no_mangle)]
pub extern "C" fn kui_input_drag_cancel(ptr: *mut KuiCtx) {
    push_input(ptr, InputEvent::DragCancel);
}

fn edit_key_of(key: u32) -> Option<EditKey> {
    KUI_EDIT_KEYS.get(key as usize).map(|(_, k)| *k)
}

/// Physical modifier state changed (KUI_KMOD_* bits). The host polls a
/// `{kind="modifiers", shift, ctrl, alt, super}` event when it differs from
/// the last report.
#[unsafe(no_mangle)]
pub extern "C" fn kui_input_modifiers(ptr: *mut KuiCtx, mods: u32) {
    push_input(
        ptr,
        InputEvent::Modifiers(kui_core::KeyMods::from_bits(mods)),
    );
}

/// Editing key with modifier bits (1 = shift, 2 = word/alt, 4 = doc/primary).
#[unsafe(no_mangle)]
pub extern "C" fn kui_input_key(ptr: *mut KuiCtx, key: u32, mods: u32) {
    if let Some(k) = edit_key_of(key) {
        let mods = Mods {
            shift: mods & KUI_MOD_SHIFT != 0,
            word: mods & KUI_MOD_WORD != 0,
            doc: mods & KUI_MOD_DOC != 0,
        };
        push_input(ptr, InputEvent::Key(k, mods));
    }
}

fn key_press_of(
    code: KuiStr,
    physical: KuiStr,
    kmods: u32,
    text: KuiStr,
) -> Option<kui_core::KeyPress> {
    let layout = kui_core::KeyCode::from_name(&kstr(code))?;
    let mods = kui_core::KeyMods::from_bits(kmods);
    // A NULL `physical` means "the key I just named": a host that does not
    // track positions says so by omission, and gets `code` through unchanged
    // because the two agree (a letter's position is its lower-case letter,
    // as a window reports it; backlog F65). A host that does track them hands both over and
    // `from_layout` applies the same non-Latin fallback the winit driver
    // does — the rule lives in the core so no host reimplements it.
    let press = match physical.ptr.is_null() {
        true => kui_core::KeyPress::new(layout, mods),
        false => {
            let phys = kui_core::KeyCode::from_name(&kstr(physical))?;
            kui_core::KeyPress::from_layout(layout, phys, mods)
        }
    };
    // A NULL `text` means "whatever this key inserts": the plain
    // character keys insert themselves, a chord inserts nothing. Asked of
    // the key the layout named, not the US stand-in in `code`: shift on
    // the key printed `;` on a Russian layout types `Ж`, not `:` (RG28).
    let text = match text.ptr.is_null() {
        false => Some(kstr(text).into_owned()),
        true => layout.typed(mods),
    };
    Some(kui_core::KeyPress { text, ..press })
}

/// A raw key press for `on_key` sinks (the editing keys go through
/// `kui_input_key`). `code` is a single character as the layout produced it
/// ("W", "$") or a name ("left", "enter", "escape", "f5", ...); `physical`
/// is the US-QWERTY key at that *position*, spelled the same way, or NULL
/// when the host does not track positions (then it equals `code`); `kmods`
/// is KUI_KMOD_* bits; `text` is what the press inserts, or NULL to derive
/// it from `code`; `repeat` marks an auto-repeat. The focused sink polls
/// `{kind="key", phase="down", code, physical, ctrl, alt, shift, super,
/// text, repeat, tag}`. An unknown `code` or `physical` is ignored.
///
/// Passing both is what makes a keymap portable: a layout that produces
/// something outside ASCII (Cyrillic, Greek, Hebrew, Arabic) would leave a
/// Latin keymap matching nothing, so kui reports the position's US key
/// as `code` instead, as Shift prints it (unshifted under Alt) — exactly
/// as the winit runner does. A NULL `text` is still the layout's own
/// character. A host that passes NULL keeps the old behaviour.
#[unsafe(no_mangle)]
pub extern "C" fn kui_input_key_down(
    ptr: *mut KuiCtx,
    code: KuiStr,
    physical: KuiStr,
    kmods: u32,
    text: KuiStr,
    repeat: bool,
) {
    guard((), || {
        if let Some(kp) = key_press_of(code, physical, kmods, text) {
            push_input(
                ptr,
                InputEvent::KeyDown(kui_core::KeyPress { repeat, ..kp }),
            );
        }
    });
}

/// The release of a key pressed with `kui_input_key_down`, spelled the same
/// way (`physical` included, NULL for "same as `code`"); the sink polls
/// `{kind="key", phase="up", ...}` with a null `text`.
/// A release whose press the sink never got resolves nothing, and moving
/// focus while a key is held delivers the "up" first, so a held-key binding
/// (WASD, press-and-hold) cannot be left stuck down.
#[unsafe(no_mangle)]
pub extern "C" fn kui_input_key_up(ptr: *mut KuiCtx, code: KuiStr, physical: KuiStr, kmods: u32) {
    guard((), || {
        if let Some(kp) = key_press_of(
            code,
            physical,
            kmods,
            KuiStr {
                ptr: std::ptr::null(),
                len: 0,
            },
        ) {
            push_input(ptr, InputEvent::KeyUp(kp.released()));
        }
    });
}

/// A whole key going down, the way a window sends it — the call a host
/// driving kui from its own event loop wants, and the one a headless test
/// wants (backlog F6). Spelled exactly as `kui_input_key_down`, and it
/// sends that press first; then it asks the core what that key *means*,
/// which is what `kui_input_key` carries on its own: Escape dismisses a
/// modal, Tab walks the focus ring, an arrow nudges a focused slider,
/// Space presses a focused control, a printable character reaches the
/// focused editor.
///
/// The two older calls stay as the halves, for a host that means to drive
/// one channel and not the other. A host that means "the user pressed this
/// key" wants this one: `kui_input_key_down(ctx, KUI_STR("escape"), ...)`
/// alone leaves a modal open, because it is only half of what a keyboard
/// does. An unknown `code` or `physical` is ignored, as there.
#[unsafe(no_mangle)]
pub extern "C" fn kui_input_press(
    ptr: *mut KuiCtx,
    code: KuiStr,
    physical: KuiStr,
    kmods: u32,
    text: KuiStr,
    repeat: bool,
) {
    guard((), || {
        let Some(kp) = key_press_of(code, physical, kmods, text) else {
            return;
        };
        if let Some(c) = unsafe { ctx(ptr) } {
            let evs = c.core().press(kui_core::KeyPress { repeat, ..kp });
            c.absorb(evs);
        }
    });
}

/// The same key coming up, spelled the way `kui_input_press` spells it
/// (`physical` included, {NULL, 0} for "same as `code`"). One channel,
/// because only one has a second half: the editing keys act on the way
/// down, so this is `kui_input_key_up` under the name that pairs with
/// `kui_input_press`.
#[unsafe(no_mangle)]
pub extern "C" fn kui_input_release(ptr: *mut KuiCtx, code: KuiStr, physical: KuiStr, kmods: u32) {
    guard((), || {
        let Some(kp) = key_press_of(
            code,
            physical,
            kmods,
            KuiStr {
                ptr: std::ptr::null(),
                len: 0,
            },
        ) else {
            return;
        };
        if let Some(c) = unsafe { ctx(ptr) } {
            let evs = c.core().release(kp);
            c.absorb(evs);
        }
    });
}

/// Lets go of every key the focused sink is holding, as if the user had
/// released them. Hosts call it when the window loses the keyboard: the OS
/// stops delivering key events to it, so the release of anything held over
/// an app switch would never arrive. Focus moves do this by themselves.
#[unsafe(no_mangle)]
pub extern "C" fn kui_release_held_keys(ptr: *mut KuiCtx) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().release_held_keys();
        }
    });
}

/// Pops the next pending UI event. The payload pointer stays valid until the
/// next poll call on the same context (or context free).
#[unsafe(no_mangle)]
pub extern "C" fn kui_poll_event(ptr: *mut KuiCtx, out: *mut KuiEvent) -> bool {
    guard(false, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return false;
        };
        // Drop the previously handed-out payload.
        c.last_payload = None;
        // Also whatever a call between frames left pending — the synthetic
        // key releases `kui_focus` / `kui_release_held_keys` force.
        let pending = c.core().take_pending_events();
        c.absorb(pending);
        // Refuse before popping: a reservation this library cannot write
        // into must leave the queue where it was, not swallow an event.
        if c.events.is_empty() || !out_accepts(out) {
            return false;
        }
        let ev = c.events.remove(0);
        let payload = Box::new(KuiValue(ev.payload));
        let payload_ptr: *const KuiValue = &*payload;
        c.last_payload = Some(payload);
        write_out(
            out,
            KuiEvent {
                origin: ev.origin.0,
                key: ev.key.0,
                payload: payload_ptr,
                window: ev.window.0,
                slot: ev.slot.map_or(0, |k| k.0),
                ..Default::default()
            },
        )
    })
}

/// The pointer shape for where the pointer is now (KUI_CURSOR_*, never 0):
/// the `cursor` the topmost node under it declared, the I-beam over text,
/// the arrow otherwise. A query, not a queue — read it after each input and
/// each frame and apply it to the real window when it changes. Hosts
/// without a pointer simply never call.
#[unsafe(no_mangle)]
pub extern "C" fn kui_cursor_shape(ptr: *mut KuiCtx) -> u32 {
    guard(0, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return 0;
        };
        let shape = c.core().cursor_shape();
        // KUI_CURSOR_* = schema index + 1, matching KuiSpec.cursor.
        kui_core::schema::CURSORS
            .iter()
            .position(|n| *n == shape.name())
            .map_or(0, |i| i as u32 + 1)
    })
}

/// Text an IME committed at the end of a composition: a focused editor
/// takes it as `kui_input_text` would; otherwise the focused `onKey` sink
/// hears `{kind:"text", text, tag}` — the one committed text a `key`
/// event never carries (backlog C17). Typing stays on `kui_input_text`.
#[unsafe(no_mangle)]
pub extern "C" fn kui_input_commit(ptr: *mut KuiCtx, text: KuiStr) {
    guard((), || {
        let text = kstr(text).into_owned();
        push_input(ptr, InputEvent::Commit(text));
    });
}

/// The clipboard's answer to a paste (`KUI_MENU_ACTION_PASTE`), with the
/// pasteboard's markers as `KUI_PASTE_*` bits (backlog F84): routed as
/// `kui_input_commit` is, and a focused `onKey` sink hears
/// `{kind:"text", text, tag}` with `concealed: true` / `transient: true`
/// for the bits that are set. A host that cannot read the markers
/// answers with 0, or with `kui_input_commit`, which is the same answer.
#[unsafe(no_mangle)]
pub extern "C" fn kui_input_paste(ptr: *mut KuiCtx, text: KuiStr, marks: u32) {
    guard((), || {
        let text = kstr(text).into_owned();
        let marks = kui_core::ClipboardMarks::from_bits(marks);
        push_input(ptr, InputEvent::Paste { text, marks });
    });
}

/// In-progress IME composition, shown at the focused editor's caret, or
/// with no editor focused delivered to the focused `onKey` sink as
/// `{kind:"preedit", text, cursor, tag}`. Empty text clears it; the commit
/// arrives via `kui_input_commit`. `cursor_start`/`cursor_end` are byte
/// offsets into `text`, or `UINT32_MAX` for none.
#[unsafe(no_mangle)]
pub extern "C" fn kui_input_preedit(
    ptr: *mut KuiCtx,
    text: KuiStr,
    cursor_start: u32,
    cursor_end: u32,
) {
    guard((), || {
        let text = kstr(text).into_owned();
        let cursor =
            (cursor_start != u32::MAX).then_some((cursor_start as usize, cursor_end as usize));
        push_input(ptr, InputEvent::Preedit(text, cursor));
    });
}

/// Current text of an editor. The returned view is valid until the next
/// kui_edit_text call (or context free).
#[unsafe(no_mangle)]
pub extern "C" fn kui_edit_text(ptr: *mut KuiCtx, key: u64, out: *mut KuiStr) -> bool {
    guard(false, || {
        let (Some(c), Some(out)) = (unsafe { ctx(ptr) }, unsafe { out.as_mut() }) else {
            return false;
        };
        let Some(text) = c.core().edit_text(Key(key)) else {
            return false;
        };
        c.last_edit_text = Some(text);
        let s = c.last_edit_text.as_ref().unwrap();
        *out = KuiStr {
            ptr: s.as_ptr(),
            len: s.len(),
        };
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn kui_edit_set_text(ptr: *mut KuiCtx, key: u64, text: KuiStr) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            let text = kstr(text).into_owned();
            c.core().set_edit_text(Key(key), &text);
        }
    });
}

/// The same call by the label the view declares (`kui_edit`'s `label`),
/// for the host that has no key to give: a key comes from an event the
/// node fired, and an editor being opened for the first time has fired
/// none (backlog F32). A label some frame declared is applied at once; one
/// nothing has declared is held for the next frame that declares an editor
/// under it, seeding a new editor over its `initial` and replacing a
/// retained one's draft. Held for that one frame — a label nothing
/// declares on it drops its text and raises `edit-text-without-editor`
/// (`kui_take_warnings`).
#[unsafe(no_mangle)]
pub extern "C" fn kui_edit_set_text_label(ptr: *mut KuiCtx, label: KuiStr, text: KuiStr) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            let label = kstr(label).into_owned();
            let text = kstr(text).into_owned();
            c.core().set_edit_text_by_label(&label, &text);
        }
    });
}
