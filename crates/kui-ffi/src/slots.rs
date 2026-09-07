//! Slots across the C boundary (`docs/adr/0014-slots-an-extension-fills-in-place.md`).
//!
//! Two sides. A C **host** declares one with `kui_slot`, the way a Rust
//! host calls `Ui::slot`. A C **extension** reads which slot it is filling
//! with `kui_slot_name` / `kui_slot_params`, and answers an event with
//! `kui_reply` — the plugin-side half of decision 6, as a function rather
//! than a changed `kui_ext_on_event` signature, so nothing in the ABI
//! moved. All four are functions; ADR 0006's rule does not bump the
//! version for a new one.

use std::cell::RefCell;

use super::*;

/// Declares a slot named `name` at the cursor, with `params` (may be NULL)
/// for whatever fills it. Under `kui_run` nothing does yet — the C runner
/// takes no extensions — so today this records the position: `kui_key_of`
/// answers the slot's key, and a later runner that loads plugins fills it
/// here. A C extension calling this from its own view declares nothing.
#[unsafe(no_mangle)]
pub extern "C" fn kui_slot(ptr: *mut KuiCtx, name: KuiStr, params: *const KuiValue) -> bool {
    guard(false, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return false;
        };
        // Read for the day the C runner fills slots; a host that passes
        // params to a runner that cannot deliver them loses nothing but
        // the read.
        let _ = unsafe { params.as_ref() };
        c.core().begin_slot(&kstr(name)).is_some()
    })
}

/// Which slot this context is a fill of; false (and `out` untouched) on
/// a context that is not an extension's — a standalone one, or a C host's
/// view callback. Borrowed for the duration of `kui_ext_view`.
#[unsafe(no_mangle)]
pub extern "C" fn kui_slot_name(ptr: *mut KuiCtx, out: *mut KuiStr) -> bool {
    guard(false, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return false;
        };
        let (Some(name), Some(out)) = (c.slot_name.as_deref(), unsafe { out.as_mut() }) else {
            return false;
        };
        *out = KuiStr {
            ptr: name.as_ptr(),
            len: name.len(),
        };
        true
    })
}

/// The namespace the host loaded this extension under — what makes the
/// slot's full name, and what tells one instance of a plugin loaded twice
/// from the other. False (and `out` untouched) on a context that is not
/// an extension's. Borrowed for the duration of `kui_ext_view`.
#[unsafe(no_mangle)]
pub extern "C" fn kui_slot_namespace(ptr: *mut KuiCtx, out: *mut KuiStr) -> bool {
    guard(false, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return false;
        };
        let (Some(ns), Some(out)) = (c.slot_namespace.as_deref(), unsafe { out.as_mut() }) else {
            return false;
        };
        *out = KuiStr {
            ptr: ns.as_ptr(),
            len: ns.len(),
        };
        true
    })
}

/// The parameters the host declared the slot with, or NULL when it passed
/// none (or this is not an extension's context). Borrowed for the
/// duration of `kui_ext_view`; read it with `kui_value_get` and friends.
#[unsafe(no_mangle)]
pub extern "C" fn kui_slot_params(ptr: *mut KuiCtx) -> *const KuiValue {
    guard(std::ptr::null(), || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return std::ptr::null();
        };
        c.slot_params
            .as_ref()
            .map_or(std::ptr::null(), |v| v as *const KuiValue)
    })
}

thread_local! {
    /// The replies the `kui_ext_on_event` in progress has made, keyed by
    /// the event it was handed so a stray call outside the callback — or
    /// with somebody else's event — is refused rather than misdelivered.
    static REPLIES: RefCell<Option<(*const KuiEvent, Vec<Value>)>> = const { RefCell::new(None) };
}

/// Runs `cb` (a plugin's `kui_ext_on_event`) with a reply sink open for
/// `ev`, and returns what it replied. Nested calls cannot happen — the
/// runner delivers events one at a time — so the sink is a single slot.
pub(crate) fn collect_replies(ev: &KuiEvent, cb: impl FnOnce()) -> Vec<Value> {
    REPLIES.with(|r| *r.borrow_mut() = Some((ev as *const KuiEvent, Vec::new())));
    cb();
    REPLIES
        .with(|r| r.borrow_mut().take())
        .map_or_else(Vec::new, |(_, replies)| replies)
}

/// Replies to the host from inside `kui_ext_on_event` (ADR 0014 decision
/// 6): `ev` is the event the callback was handed, `reply` is copied — you
/// keep ownership — and reaches the host's `on_event` with your origin
/// and the event's window and key. Call it as often as the event
/// deserves. Outside the callback, or with an event that is not the one
/// in progress, it does nothing and returns false.
#[unsafe(no_mangle)]
pub extern "C" fn kui_reply(ev: *const KuiEvent, reply: *const KuiValue) -> bool {
    guard(false, || {
        let Some(reply) = (unsafe { reply.as_ref() }) else {
            return false;
        };
        REPLIES.with(|r| {
            let mut sink = r.borrow_mut();
            match sink.as_mut() {
                Some((current, replies)) if std::ptr::eq(*current, ev) => {
                    replies.push(reply.0.clone());
                    true
                }
                _ => false,
            }
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ks(s: &str) -> KuiStr {
        KuiStr {
            ptr: s.as_ptr(),
            len: s.len(),
        }
    }

    /// A C host's slot: declared at the cursor, findable by name, and a
    /// second declaration of the name refused with the warning.
    #[test]
    fn a_host_slot_is_declared_and_named() {
        let ctx = kui_ctx_new();
        // A standalone context starts with diagnostics off; the duplicate
        // below is what they are for here.
        kui_set_diagnostics(ctx, true);
        kui_frame_begin(ctx, 200.0, 100.0, 1.0);
        let root: KuiSpec = unsafe { std::mem::zeroed() };
        kui_root(ctx, &root);
        assert!(kui_slot(ctx, ks("side"), std::ptr::null()));
        assert!(
            !kui_slot(ctx, ks("side"), std::ptr::null()),
            "declared twice"
        );
        kui_frame_finish(ctx);
        assert_eq!(
            kui_key_of(ctx, ks("side")),
            kui_core::Key::ROOT.str("side").0,
            "the slot's key is `parent.str(name)`, and `kui_key_of` answers it"
        );
        let c = unsafe { ctx.as_mut() }.unwrap();
        let ws = c.core().take_warnings();
        assert_eq!(ws.len(), 1);
        assert_eq!(ws[0].code, kui_core::diag::DUPLICATE_SLOT);
        // Not an extension's context: no slot to read.
        let mut name = ks("");
        assert!(!kui_slot_name(ctx, &mut name));
        assert!(!kui_slot_namespace(ctx, &mut name));
        assert!(kui_slot_params(ctx).is_null());
        kui_ctx_free(ctx);
    }

    /// `kui_reply` only counts inside the callback it was made for.
    #[test]
    fn replies_are_collected_for_the_event_in_progress_only() {
        let payload = KuiValue(Value::Null);
        let ev = KuiEvent {
            payload: &payload,
            ..Default::default()
        };
        let other = KuiEvent {
            payload: &payload,
            ..Default::default()
        };
        let reply = KuiValue(Value::map([("kind", "open".into())]));
        assert!(!kui_reply(&ev, &reply), "no callback in progress");
        let got = collect_replies(&ev, || {
            assert!(kui_reply(&ev, &reply));
            assert!(!kui_reply(&other, &reply), "somebody else's event");
            assert!(!kui_reply(&ev, std::ptr::null()), "no value");
            assert!(kui_reply(&ev, &reply));
        });
        assert_eq!(got, vec![reply.0.clone(), reply.0.clone()]);
        assert!(!kui_reply(&ev, &reply), "the sink closed with the callback");
    }
}
