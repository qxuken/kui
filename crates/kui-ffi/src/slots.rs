//! Slots across the C boundary (`docs/adr/0014-slots-an-extension-fills-in-place.md`).
//!
//! Three sides now. A C **host** declares a slot with `kui_slot` the way a
//! Rust host calls `Ui::slot`, and loads what fills it with
//! `kui_ctx_add_extension` — the C half of ADR 0014's loader, which used to
//! be Rust's alone. A C **extension** reads which slot it is filling with
//! `kui_slot_name` / `kui_slot_params`, and answers an event with
//! `kui_reply`, whose sink rides on the event (ABI 10) so that a plugin
//! linked against another copy of this library still reaches the host.

use super::*;

/// Declares a slot named `name` at the cursor, with `params` (may be NULL)
/// for whatever fills it — and fills it, then and there, with the
/// extension the full name's namespace belongs to
/// (`docs/adr/0014-slots-an-extension-fills-in-place.md`). `name` is the
/// full `namespace/slot`: the namespace this context loaded the extension
/// under with [`kui_ctx_add_extension`], and the slot in the extension's
/// own vocabulary.
///
/// Returns false when the frame already declared this name (a duplicate,
/// which warns) or on a bad context; true when the slot was declared,
/// whether or not anything filled it. `kui_key_of` answers its key either
/// way, so a host with no extensions loaded still gets a placed, empty
/// node — which is what this did for every host before extensions could be
/// loaded from C at all.
///
/// A C extension may call this from its own view too, and the slot is
/// declared inside its fill and keyed there. What it cannot do is load
/// the thing that fills it: a plugin's context has no list of its own, so
/// the name it declares has to be one the host above it already loaded
/// (`kui_core::slot`, and `env.add_extension` in Lua for the side that
/// can load). Its own slot is the one name that finds nobody, since it is
/// out of the list while it draws — that warns and draws nothing.
#[unsafe(no_mangle)]
pub extern "C" fn kui_slot(ptr: *mut KuiCtx, name: KuiStr, params: *const KuiValue) -> bool {
    guard(false, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return false;
        };
        let name = kstr(name);
        let params = unsafe { params.as_ref() }.map_or(&kui_core::Value::Null, |v| &v.0);
        // `Ui::slot_with` is `begin_slot` and the fill together, so the
        // one question is which `Ui`: under `kui_run_with` the runner's
        // own, which carries its extension list and which `borrowing_in`
        // kept a pointer to; otherwise one made here around the context's
        // core and *its* list. The core is behind a raw pointer on the
        // context so that both can be borrowed at once.
        let mut local;
        let ui: &mut kui_core::Ui<'_> = if c.host_ui.is_null() {
            local = kui_core::Ui::with_filler(unsafe { &mut *c.core }, &mut c.extensions);
            &mut local
        } else {
            unsafe { &mut *c.host_ui.cast() }
        };
        ui.slot_with(&name, params)
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

/// The host's end of a reply sink: the [`KuiReplySink`] header a plugin is
/// handed, and the list only this copy of the library ever touches.
///
/// `repr(C)` with `head` first, so the `*mut KuiReplySink` on the event can
/// be cast back to this. Nothing outside this file knows the second field
/// exists — least of all the plugin, which has only the header's function
/// pointer and calls through it.
#[repr(C)]
struct HostSink {
    head: KuiReplySink,
    replies: Vec<Value>,
}

/// The `push` a plugin's `kui_reply` forwards to. Runs in the copy of the
/// library that opened the sink, which is the whole reason it is a function
/// pointer: see [`KuiReplySink`].
unsafe extern "C" fn push_reply(sink: *mut KuiReplySink, value: *const KuiValue) -> bool {
    guard(false, || {
        let (Some(sink), Some(value)) = (unsafe { sink.cast::<HostSink>().as_mut() }, unsafe {
            value.as_ref()
        }) else {
            return false;
        };
        sink.replies.push(value.0.clone());
        true
    })
}

/// Runs `cb` (a plugin's `kui_ext_on_event`) with a reply sink open on
/// `ev`, and returns what it replied. The sink lives on this stack frame
/// for exactly the call: `ev.reply_sink` points at it, and `push` is
/// cleared before it goes, so a plugin that stored the event and calls
/// afterwards is refused rather than heard.
///
/// Nested calls cannot happen — the runner delivers events one at a time —
/// but nothing here would mind if they did, which is the other thing the
/// process-global this replaced could not say.
pub(crate) fn collect_replies(ev: &mut KuiEvent, cb: impl FnOnce(&KuiEvent)) -> Vec<Value> {
    let mut sink = HostSink {
        head: KuiReplySink {
            push: Some(push_reply),
        },
        replies: Vec::new(),
    };
    ev.reply_sink = (&raw mut sink).cast::<KuiReplySink>();
    cb(ev);
    ev.reply_sink = std::ptr::null_mut();
    sink.head.push = None;
    sink.replies
}

/// Replies to the host from inside `kui_ext_on_event` (ADR 0014 decision
/// 6): `ev` is the event the callback was handed, `reply` is copied — you
/// keep ownership — and reaches the host's `on_event` with your origin
/// and the event's window and key. Call it as often as the event
/// deserves. Outside the callback, or on an event that carries no sink —
/// anything a host polled for itself — it does nothing and returns false.
///
/// It forwards through the sink on the event rather than into a list of
/// its own, so it does the right thing when the plugin's copy of this
/// library is not the host's. [`KuiReplySink`] is why that matters.
#[unsafe(no_mangle)]
pub extern "C" fn kui_reply(ev: *const KuiEvent, reply: *const KuiValue) -> bool {
    guard(false, || {
        let (Some(ev), Some(reply)) = (unsafe { ev.as_ref() }, unsafe { reply.as_ref() }) else {
            return false;
        };
        let sink = ev.reply_sink;
        let Some(push) = (unsafe { sink.as_ref() }).and_then(|s| s.push) else {
            return false;
        };
        unsafe { push(sink, reply) }
    })
}

// ---------------------------------------------------------------------------
// Loading extensions from C (ADR 0014)

/// Loads the shared library at `path` as an extension of this context,
/// under `namespace` — the word that makes the front of every slot name it
/// fills (`namespace/panel`). An empty `namespace` takes the extension's
/// own `kui_ext_name`, which is what a Rust host's `Extensions::push` does.
///
/// False on any refusal, with the reason readable until the next call
/// through [`kui_ctx_extension_error`]: the library will not load, it
/// declares no `kui_ext_abi` or one this build does not implement, it has
/// no `kui_ext_view`, the namespace is empty *and* the plugin named
/// itself nothing, or the namespace is already another extension's.
///
/// The context owns the extension from here: it is unloaded by
/// `kui_ctx_free`, after the plugin's own `kui_ext_free`. Load before the
/// first frame — origins are positions in this list, so a plugin added
/// between frames renumbers the ones after it.
///
/// # Safety
/// The library's entry points run in this process on this thread, on the
/// frame this context owns. Loading one is trusting it exactly as much as
/// linking it would be.
#[unsafe(no_mangle)]
pub extern "C" fn kui_ctx_add_extension(ptr: *mut KuiCtx, namespace: KuiStr, path: KuiStr) -> bool {
    guard(false, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return false;
        };
        c.last_ext_error.clear();
        // SAFETY: the caller's, and the doc comment says so. An empty
        // namespace is the extension's own name — `push_as`'s rule, not
        // one repeated here.
        let loaded = unsafe { crate::CExtension::open(&*kstr(path)) }
            .and_then(|ext| c.extensions.push_as(kstr(namespace), Box::new(ext)));
        if let Err(err) = loaded {
            c.last_ext_error = err;
        }
        c.last_ext_error.is_empty()
    })
}

/// Why the last [`kui_ctx_add_extension`] on this context said false.
/// False (and `out` untouched) when the last one succeeded, or when none
/// has run. Borrowed until the next call on this context, like every other
/// string this API hands back.
#[unsafe(no_mangle)]
pub extern "C" fn kui_ctx_extension_error(ptr: *mut KuiCtx, out: *mut KuiStr) -> bool {
    guard(false, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return false;
        };
        let Some(out) = (unsafe { out.as_mut() }) else {
            return false;
        };
        if c.last_ext_error.is_empty() {
            return false;
        }
        *out = KuiStr {
            ptr: c.last_ext_error.as_ptr(),
            len: c.last_ext_error.len(),
        };
        true
    })
}

/// How many extensions this context has loaded. Their origins are 1..=n,
/// in the order they were added; 0 is the host's own.
#[unsafe(no_mangle)]
pub extern "C" fn kui_ctx_extension_count(ptr: *mut KuiCtx) -> u32 {
    guard(0, || {
        unsafe { ctx(ptr) }.map_or(0, |c| c.extensions.len() as u32)
    })
}

/// The namespace the extension at `origin` was loaded under — what turns
/// an event's `origin` back into a name the host chose. False (and `out`
/// untouched) for 0 (the host) or an origin nothing was loaded at.
/// Borrowed until the next call.
#[unsafe(no_mangle)]
pub extern "C" fn kui_ctx_extension_namespace(
    ptr: *mut KuiCtx,
    origin: u16,
    out: *mut KuiStr,
) -> bool {
    guard(false, || {
        let (Some(c), Some(out)) = (unsafe { ctx(ptr) }, unsafe { out.as_mut() }) else {
            return false;
        };
        let Some(ns) = c.extensions.namespace_of(kui_core::OriginId(origin)) else {
            return false;
        };
        *out = KuiStr {
            ptr: ns.as_ptr(),
            len: ns.len(),
        };
        true
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

    /// `kui_reply` reaches the host only through the sink the callback was
    /// handed, and only while it is open.
    #[test]
    fn replies_are_collected_for_the_event_in_progress_only() {
        let payload = KuiValue(Value::Null);
        let mut ev = KuiEvent {
            payload: &payload,
            ..Default::default()
        };
        // An event nobody opened a sink on: what a host polls for itself.
        let other = KuiEvent {
            payload: &payload,
            ..Default::default()
        };
        let reply = KuiValue(Value::map([("kind", "open".into())]));
        assert!(!kui_reply(&ev, &reply), "no callback in progress");
        let got = collect_replies(&mut ev, |ev| {
            assert!(kui_reply(ev, &reply));
            assert!(!kui_reply(&other, &reply), "an event with no sink");
            assert!(!kui_reply(ev, std::ptr::null()), "no value");
            assert!(kui_reply(ev, &reply));
        });
        assert_eq!(got, vec![reply.0.clone(), reply.0.clone()]);
        assert!(!kui_reply(&ev, &reply), "the sink closed with the callback");
        assert!(ev.reply_sink.is_null(), "and the event no longer names one");
    }

    /// The C loader's refusals, and what it says about them. The happy
    /// path needs a real plugin and so lives in `examples/c/host.c`'s
    /// `--headless`, which the build scripts run; this is the half that
    /// needs no compiler.
    #[test]
    fn a_c_host_is_told_why_an_extension_would_not_load() {
        let ctx = kui_ctx_new();
        assert_eq!(kui_ctx_extension_count(ctx), 0);

        let mut out = ks("");
        assert!(
            !kui_ctx_extension_error(ctx, &mut out),
            "nothing has been tried yet"
        );

        // The platform's own C runtime: loads, and declares no kui_ext_abi —
        // which is the plugin built against a header from before the symbol
        // existed, the case the check is for.
        let lib = crate::ext::tests::a_library_with_no_kui_symbols();
        assert!(!kui_ctx_add_extension(ctx, ks("libc"), ks(lib)));
        assert!(kui_ctx_extension_error(ctx, &mut out));
        let msg =
            std::str::from_utf8(unsafe { std::slice::from_raw_parts(out.ptr, out.len) }).unwrap();
        assert!(
            msg.contains("declares no ABI"),
            "the reason should be the ABI refusal, got {msg:?}"
        );
        assert_eq!(kui_ctx_extension_count(ctx), 0, "nothing was kept");

        // A path that is not a library at all.
        assert!(!kui_ctx_add_extension(
            ctx,
            ks("nope"),
            ks("no/such/library")
        ));
        assert!(kui_ctx_extension_error(ctx, &mut out));

        // No extension at any origin, so nothing names one.
        assert!(
            !kui_ctx_extension_namespace(ctx, 0, &mut out),
            "0 is the host"
        );
        assert!(!kui_ctx_extension_namespace(ctx, 1, &mut out));

        // And a slot still places its node with nothing to fill it, which is
        // what lets a host lay out before it has a plugin.
        kui_frame_begin(ctx, 200.0, 100.0, 1.0);
        assert!(kui_slot(ctx, ks("todos/panel"), std::ptr::null()));
        assert!(
            !kui_slot(ctx, ks("todos/panel"), std::ptr::null()),
            "duplicate"
        );
        kui_frame_finish(ctx);
        kui_ctx_free(ctx);
    }
}
