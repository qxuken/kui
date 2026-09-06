//! The access tree and the requests assistive technology sends back
//! (`docs/adr/0001-accessibility-as-data.md`).

use super::*;

/// The access tree of the last finished frame (what assistive technology
/// sees; see docs/adr/0001-accessibility-as-data.md): fills `out` with up
/// to `cap` nodes in tree order (the root first) and returns the total
/// count, so a short buffer can be resized and the call repeated. Strings
/// stay valid until the next call on this context. A host that never
/// asks pays nothing.
#[unsafe(no_mangle)]
pub extern "C" fn kui_access_tree(ptr: *mut KuiCtx, out: *mut KuiAccessNode, cap: usize) -> usize {
    guard(0, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return 0;
        };
        c.last_access = c.core().access_tree().clone();
        let total = c.last_access.nodes.len();
        if out.is_null() || cap == 0 {
            return total;
        }
        let s = |s: &Option<String>| match s {
            Some(s) => KuiStr {
                ptr: s.as_ptr(),
                len: s.len(),
            },
            None => KuiStr {
                ptr: std::ptr::null(),
                len: 0,
            },
        };
        for (i, n) in c.last_access.nodes.iter().take(cap).enumerate() {
            let mut flags = 0;
            if n.value.is_some() {
                flags |= KUI_ACCESS_HAS_VALUE;
            }
            if n.selection.is_some() {
                flags |= KUI_ACCESS_HAS_SELECTION;
            }
            if n.focused {
                flags |= KUI_ACCESS_FOCUSED;
            }
            if let Some(checked) = n.checked {
                flags |= KUI_ACCESS_CHECKED_SET;
                if checked {
                    flags |= KUI_ACCESS_CHECKED;
                }
            }
            if n.number.is_some() {
                flags |= KUI_ACCESS_HAS_NUMBER;
            }
            if n.min.is_some() {
                flags |= KUI_ACCESS_HAS_MIN;
            }
            if n.max.is_some() {
                flags |= KUI_ACCESS_HAS_MAX;
            }
            if n.scroll.is_some() {
                flags |= KUI_ACCESS_HAS_SCROLL;
            }
            if n.anchor.is_some() && n.focus.is_some() {
                flags |= KUI_ACCESS_HAS_TEXT_SELECTION;
            }
            if n.disabled {
                flags |= KUI_ACCESS_DISABLED;
            }
            if let Some(selected) = n.selected {
                flags |= KUI_ACCESS_SELECTED_SET;
                if selected {
                    flags |= KUI_ACCESS_SELECTED;
                }
            }
            if let Some(expanded) = n.expanded {
                flags |= KUI_ACCESS_EXPANDED_SET;
                if expanded {
                    flags |= KUI_ACCESS_EXPANDED;
                }
            }
            if n.pos_in_set.is_some() {
                flags |= KUI_ACCESS_HAS_POS_IN_SET;
            }
            if n.set_size.is_some() {
                flags |= KUI_ACCESS_HAS_SET_SIZE;
            }
            if n.modal {
                flags |= KUI_ACCESS_MODAL;
            }
            match n.live {
                kui_core::Live::Off => {}
                kui_core::Live::Polite => flags |= KUI_ACCESS_LIVE_POLITE,
                kui_core::Live::Assertive => flags |= KUI_ACCESS_LIVE_ASSERTIVE,
            }
            let scroll = n.scroll.unwrap_or_default();
            let (sel_start, sel_end) = n.selection.unwrap_or((0, 0));
            unsafe {
                out.add(i).write(KuiAccessNode {
                    key: n.key.0,
                    parent: n.parent.map_or(0, |k| k.0),
                    origin: n.origin.0 as u32,
                    role: role_code(n.role),
                    flags,
                    actions: n.actions,
                    name: s(&n.name),
                    description: s(&n.description),
                    value: s(&n.value),
                    x: n.rect.x,
                    y: n.rect.y,
                    w: n.rect.w,
                    h: n.rect.h,
                    caret: n.caret.unwrap_or(0) as u32,
                    selection_start: sel_start as u32,
                    selection_end: sel_end as u32,
                    value_now: n.number.unwrap_or(0.0),
                    value_min: n.min.unwrap_or(0.0),
                    value_max: n.max.unwrap_or(0.0),
                    scroll_x: scroll.x,
                    scroll_y: scroll.y,
                    scroll_max_x: scroll.max_x,
                    scroll_max_y: scroll.max_y,
                    anchor_run: n.anchor.map_or(0, |p| p.run.0),
                    anchor_char: n.anchor.map_or(0, |p| p.character as u32),
                    focus_run: n.focus.map_or(0, |p| p.run.0),
                    focus_char: n.focus.map_or(0, |p| p.character as u32),
                    run_count: n.runs.len() as u32,
                    pos_in_set: n.pos_in_set.unwrap_or(0) as u32,
                    set_size: n.set_size.unwrap_or(0) as u32,
                    orientation: orientation_code(n.orientation),
                })
            };
        }
        total
    })
}

/// A request from assistive technology on node `key`: one KUI_ACCESS_*
/// action bit (the node must advertise it in `KuiAccessNode.actions`),
/// with `value` the new text for KUI_ACCESS_SET_VALUE (empty otherwise).
/// Resolved the way the pointer or keyboard equivalent would be: a click
/// emits the node's payload, focus lands on an editor, a slider nudge
/// arrives as a `{kind="access", action, tag}` event.
#[unsafe(no_mangle)]
pub extern "C" fn kui_input_access(ptr: *mut KuiCtx, key: u64, action: u32, value: KuiStr) {
    guard((), || {
        let Some(action) = kui_core::AccessAction::ALL
            .iter()
            .copied()
            .find(|a| a.bit() == action)
        else {
            return;
        };
        let value = (!value.ptr.is_null() && value.len > 0).then(|| kstr(value).into_owned());
        push_input(
            ptr,
            InputEvent::Access(kui_core::AccessRequest {
                key: Key(key),
                action,
                value,
                anchor: None,
                focus: None,
            }),
        );
    });
}

/// The laid-out text of editor node `key` as runs (see `KuiAccessRun`):
/// fills `out` with up to `cap` of them and returns the total. Runs are
/// what `KuiAccessNode.anchor_run` / `focus_run` and
/// `kui_input_access_text` refer to. Borrowed until the next
/// `kui_access_tree` / `kui_access_runs` on the context; a node with no
/// text (or no such node) has none.
#[unsafe(no_mangle)]
pub extern "C" fn kui_access_runs(
    ptr: *mut KuiCtx,
    key: u64,
    out: *mut KuiAccessRun,
    cap: usize,
) -> usize {
    guard(0, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return 0;
        };
        c.last_access = c.core().access_tree().clone();
        let Some(node) = c.last_access.get(Key(key)) else {
            return 0;
        };
        let total = node.runs.len();
        if out.is_null() || cap == 0 {
            return total;
        }
        for (i, r) in node.runs.iter().take(cap).enumerate() {
            unsafe {
                out.add(i).write(KuiAccessRun {
                    key: r.key.0,
                    line: r.line as u32,
                    start: r.start as u32,
                    end: r.end as u32,
                    text: KuiStr {
                        ptr: r.text.as_ptr(),
                        len: r.text.len(),
                    },
                    x: r.rect.x,
                    y: r.rect.y,
                    w: r.rect.w,
                    h: r.rect.h,
                    char_count: r.char_lengths.len() as u32,
                    char_lengths: r.char_lengths.as_ptr(),
                    char_positions: r.char_positions.as_ptr(),
                    char_widths: r.char_widths.as_ptr(),
                    word_start_count: r.word_starts.len() as u32,
                    word_starts: r.word_starts.as_ptr(),
                    rtl: r.rtl as u32,
                })
            };
        }
        total
    })
}

/// A text request from assistive technology on editor node `key`:
/// KUI_ACCESS_SET_TEXT_SELECTION with the selection as run positions
/// (`anchor_*` the end that stays, `focus_*` the caret), or
/// KUI_ACCESS_REPLACE_SELECTED_TEXT / KUI_ACCESS_SET_VALUE with `value`.
/// A built-in editor applies it (a `changed` event follows an edit); a
/// custom editor gets it as a `{kind="access", action, anchor, focus,
/// text, tag}` event to apply itself.
#[unsafe(no_mangle)]
pub extern "C" fn kui_input_access_text(
    ptr: *mut KuiCtx,
    key: u64,
    action: u32,
    anchor_run: u64,
    anchor_char: u32,
    focus_run: u64,
    focus_char: u32,
    value: KuiStr,
) {
    guard((), || {
        let Some(action) = kui_core::AccessAction::ALL
            .iter()
            .copied()
            .find(|a| a.bit() == action)
        else {
            return;
        };
        let mut req = kui_core::AccessRequest::new(Key(key), action);
        if !value.ptr.is_null() && value.len > 0 {
            req.value = Some(kstr(value).into_owned());
        }
        if action == kui_core::AccessAction::SetTextSelection {
            req.anchor = Some(kui_core::TextPos {
                run: Key(anchor_run),
                character: anchor_char as usize,
            });
            req.focus = Some(kui_core::TextPos {
                run: Key(focus_run),
                character: focus_char as usize,
            });
        }
        push_input(ptr, InputEvent::Access(req));
    });
}

/// Says something once, with no node behind it: "Saved", "3 results".
/// `live` is KUI_LIVE_POLITE or KUI_LIVE_ASSERTIVE; KUI_LIVE_OFF and an
/// empty string are both no-ops, the first so a caller can gate politeness
/// without a branch. A region whose message is on screen is `KuiSpec.live`
/// instead (see `docs/adr/0008-live-regions-and-announcements.md`).
///
/// A host holding the context calls this from wherever the event is
/// handled; called from a frame builder it fires every frame, which the
/// core reports as `announcement-repeated`.
#[unsafe(no_mangle)]
pub extern "C" fn kui_announce(ptr: *mut KuiCtx, text: KuiStr, live: u32) {
    guard((), || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return;
        };
        let text = kstr(text);
        c.core()
            .announce(&text, kui_core::Live::from_index(live as usize));
    })
}

/// Drains queued announcements into `out` (up to `cap`; the rest are
/// dropped, so size it generously) and returns the count. The strings stay
/// valid until the next call on this context.
///
/// Drain every frame whether or not assistive technology is attached and
/// discard what you cannot deliver: an announcement kept is an
/// announcement said minutes late. kui_run does this itself.
#[unsafe(no_mangle)]
pub extern "C" fn kui_take_announcements(
    ptr: *mut KuiCtx,
    out: *mut KuiAnnouncement,
    cap: usize,
) -> usize {
    guard(0, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return 0;
        };
        if out.is_null() || cap == 0 {
            return 0;
        }
        c.last_announcements = c.core().take_announcements();
        let n = c.last_announcements.len().min(cap);
        for (i, a) in c.last_announcements.iter().take(n).enumerate() {
            unsafe {
                out.add(i).write(KuiAnnouncement {
                    text: KuiStr {
                        ptr: a.text.as_ptr(),
                        len: a.text.len(),
                    },
                    live: match a.live {
                        kui_core::Live::Off => KUI_LIVE_OFF,
                        kui_core::Live::Polite => KUI_LIVE_POLITE,
                        kui_core::Live::Assertive => KUI_LIVE_ASSERTIVE,
                    },
                })
            };
        }
        n
    })
}
