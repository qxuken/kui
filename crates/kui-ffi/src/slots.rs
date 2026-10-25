//! Components across the C boundary (ADR 0060, ADR 0067).
//!
//! A C **host** places a component with `kui_component` the way a Rust
//! host calls `Ui::component`, and loads what provides it with
//! `kui_ctx_add_extension`. A C **extension** reads which instance it is
//! drawing with `kui_instance_name` / `kui_instance_params`, and raises an
//! instance's events with `kui_emit`, whose sink rides on the event (ABI
//! 10) so that a plugin linked against another copy of this library still
//! reaches the host.

use super::*;

/// The `Ui` a host's placement goes through: under `kui_run_with` the
/// runner's own, which carries its extension list and which
/// `borrowing_in` kept a pointer to; otherwise one made here around the
/// context's core and *its* list. The core is behind a raw pointer on the
/// context so that both can be borrowed at once.
fn with_host_ui<T>(c: &mut KuiCtx, f: impl FnOnce(&mut kui_core::Ui<'_>) -> T) -> T {
    if c.host_ui.is_null() {
        let mut local = kui_core::Ui::with_filler(unsafe { &mut *c.core }, &mut c.extensions);
        f(&mut local)
    } else {
        f(unsafe { &mut *c.host_ui.cast() })
    }
}

/// Which component this context is drawing an instance of — `root` for
/// the view after the host's; false (and `out` untouched) on a context
/// that is not an extension's — a standalone one, or a C host's view
/// callback. Borrowed for the duration of `kui_ext_view`.
#[unsafe(no_mangle)]
pub extern "C" fn kui_instance_name(ptr: *mut KuiCtx, out: *mut KuiStr) -> bool {
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
/// component's full name, and what tells one load of a plugin loaded
/// twice from the other. False (and `out` untouched) on a context that is not
/// an extension's. Borrowed for the duration of `kui_ext_view`.
#[unsafe(no_mangle)]
pub extern "C" fn kui_instance_namespace(ptr: *mut KuiCtx, out: *mut KuiStr) -> bool {
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

/// The props the host placed this instance with, or NULL when it passed
/// none (or this is not an extension's context). Borrowed for the
/// duration of `kui_ext_view`; read it with `kui_value_get` and friends.
#[unsafe(no_mangle)]
pub extern "C" fn kui_instance_params(ptr: *mut KuiCtx) -> *const KuiValue {
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

/// The `push` a plugin's `kui_emit` forwards to. Runs in the copy of the
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

/// An answer to the host from inside `kui_ext_on_event`, through the sink
/// on the event rather than into a list of its own, so it works when the
/// plugin's copy of this library is not the host's ([`KuiReplySink`]).
/// False outside the callback, or on an event that carries no sink.
fn answer(ev: *const KuiEvent, reply: &KuiValue) -> bool {
    let Some(ev) = (unsafe { ev.as_ref() }) else {
        return false;
    };
    let sink = ev.reply_sink;
    let Some(push) = (unsafe { sink.as_ref() }).and_then(|s| s.push) else {
        return false;
    };
    unsafe { push(sink, reply) }
}

/// Raises one of your component's events from inside `kui_ext_on_event`
/// (ADR 0060): `prop` names the event prop the host passed its message in
/// (`onSelect` or `on_select` — either finds the host's), `fields` what
/// rides with it under the prop's field (NULL for nothing; you keep
/// ownership). The host hears the message it passed, about the instance
/// the event came from; a prop the host did not pass delivers nothing.
/// False outside the callback, or on an event that carries no sink
/// (anything a host polled for itself). Call it as often as the event
/// deserves; it is the one way an extension speaks (ADR 0067).
#[unsafe(no_mangle)]
pub extern "C" fn kui_emit(ev: *const KuiEvent, prop: KuiStr, fields: *const KuiValue) -> bool {
    guard(false, || {
        let fields = unsafe { fields.as_ref() }.map_or(kui_core::Value::Null, |v| v.0.clone());
        let reply = KuiValue(kui_core::slot::emit(&kstr(prop), fields));
        answer(ev, &reply)
    })
}

/// Places a component an extension provides (ADR 0060) at the cursor:
/// `name` is `namespace/component`, `props` what its fill reads (borrowed;
/// NULL for none) — `key` in it keys the instance, `fresh: true` asks the
/// extension every frame, and its `on*` entries are the messages the
/// component's events arrive as. `children` builds your own children
/// where the component asks for them, called with `user`, a context
/// borrowing the frame for the call, and the group (empty for the unnamed
/// one); NULL for none. An instance whose props did not change is
/// replayed without asking the extension; `namespace/root` is that
/// extension's view, drawn fresh every frame. Returns how it was filled, a
/// `KUI_FILL_*` code, or `KUI_FILL_NONE` when nothing provides it
/// (`unknown-component`) or outside a frame.
#[unsafe(no_mangle)]
pub extern "C" fn kui_component(
    ptr: *mut KuiCtx,
    name: KuiStr,
    props: *const KuiValue,
    children: Option<ChildrenFn>,
    user: *mut c_void,
) -> i32 {
    guard(-1, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return -1;
        };
        let name = kstr(name);
        let props = unsafe { props.as_ref() }.map_or(&kui_core::Value::Null, |v| &v.0);
        let fill = with_host_ui(c, |ui| match children {
            Some(cb) => ui.component_with(&name, props, |group, ui| {
                let mut inner = KuiCtx::borrowing_in(ui);
                cb(user, &mut inner, KuiStr::borrowed(group));
            }),
            None => ui.component(&name, props),
        });
        fill.map_or(-1, |f| f.code())
    })
}

/// Where your host's children of `group` go, from inside a component's
/// `kui_ext_view` (ADR 0060): built here, under the host's origin, keyed
/// by the instance and the group. Empty `group` for the unnamed one. Once
/// per group; anywhere else it builds nothing and raises
/// `children-misplaced`.
#[unsafe(no_mangle)]
pub extern "C" fn kui_children(ptr: *mut KuiCtx, group: KuiStr) {
    guard((), || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return;
        };
        let group = kstr(group);
        with_host_ui(c, |ui| ui.children(&group));
    })
}

// ---------------------------------------------------------------------------
// Loading extensions from C

/// Loads the extension at `path` into this context, under `namespace` —
/// the word that makes the front of every component name it provides
/// (`namespace/panel`). A `.lua` file, or a directory holding `init.lua`,
/// loads as a Lua extension; anything else as a C shared library (ADR
/// 0059, [`crate::open_extension`]). An empty `namespace` takes the
/// extension's own name — a plugin's `kui_ext_name`, a script's file name
/// — which is what a Rust host's `Extensions::push` does.
///
/// False on any refusal, with the reason readable until the next call
/// through [`kui_ctx_extension_error`]: the library will not load, it
/// declares no `kui_ext_abi` or one this build does not implement, it has
/// no `kui_ext_view`, the script fails to load or run, the namespace is
/// empty *and* the extension named itself nothing, or the namespace is
/// already another extension's.
///
/// The context owns the extension from here: it is unloaded by
/// `kui_ctx_free`, after the plugin's own `kui_ext_free`. Load before the
/// first frame — origins are positions in this list, so a plugin added
/// between frames renumbers the ones after it.
///
/// It has to be a context of your own, so this refuses one that borrows
/// somebody else's frame — a plugin's `kui_ext_view`, or a view callback
/// under `kui_run_with`. Such a context draws its components from the list one
/// level up (`host_ui`) and dies at the end of the call, so a plugin
/// loaded into it would be unloaded again having drawn nothing. Saying so
/// is the point: the alternative is that this answers true and the
/// `kui_component` after it quietly draws nothing.
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
        if !c.host_ui.is_null() {
            // A borrowed frame: this context's list is not the one drawing
            // its components, so anything put in it would be loaded, never
            // asked to draw, and unloaded when the call returns.
            c.last_ext_error = "cannot load an extension into a context that borrows a frame: \
                                its components are drawn from the list one level up. Load into \
                                a context of your own, before `kui_run_with`"
                .to_owned();
            return false;
        }
        // SAFETY: the caller's, and the doc comment says so. An empty
        // namespace is the extension's own name — `push_as`'s rule, not
        // one repeated here.
        let loaded = unsafe { crate::open_extension(&*kstr(path)) }
            .and_then(|ext| c.extensions.push_as(kstr(namespace), ext));
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

    /// A context that is not an extension's has no instance to read.
    #[test]
    fn a_host_context_names_no_instance() {
        let ctx = kui_ctx_new();
        let mut name = ks("");
        assert!(!kui_instance_name(ctx, &mut name));
        assert!(!kui_instance_namespace(ctx, &mut name));
        assert!(kui_instance_params(ctx).is_null());
        kui_ctx_free(ctx);
    }

    /// `kui_emit` reaches the host only through the sink the callback was
    /// handed, and only while it is open.
    #[test]
    fn emits_are_collected_for_the_event_in_progress_only() {
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
        let fields = KuiValue(Value::map([("kind", "open".into())]));
        let emitted = kui_core::slot::emit("on_open", fields.0.clone());
        assert!(!kui_emit(&ev, ks("on_open"), &fields), "no callback in progress");
        let got = collect_replies(&mut ev, |ev| {
            assert!(kui_emit(ev, ks("on_open"), &fields));
            assert!(!kui_emit(&other, ks("on_open"), &fields), "an event with no sink");
            assert!(kui_emit(ev, ks("on_open"), &fields));
        });
        assert_eq!(got, vec![emitted.clone(), emitted]);
        assert!(!kui_emit(&ev, ks("on_open"), &fields), "the sink closed with the callback");
        assert!(ev.reply_sink.is_null(), "and the event no longer names one");
    }

    /// The C loader's refusals, and what it says about them. The happy
    /// path needs a real plugin and so lives in `examples/c/features/slots/host.c`'s
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

        // And a component nobody provides answers that, rather than
        // drawing something.
        kui_frame_begin(ctx, 200.0, 100.0, 1.0);
        assert_eq!(
            kui_component(ctx, ks("todos/panel"), std::ptr::null(), None, std::ptr::null_mut()),
            -1
        );
        kui_frame_finish(ctx);
        kui_ctx_free(ctx);
    }

    /// A context that borrows a frame refuses a plugin instead of taking
    /// one into a list nothing fills. The path is the one a script would
    /// take (`examples/lua/features/plugins/panel.lua` loads from its view), so the answer
    /// has to be a reason and not a quiet `true`.
    #[test]
    fn a_borrowed_frame_will_not_take_an_extension() {
        let mut core = kui_core::Core::new();
        let mut ui = core.frame(kui_core::Size::new(200.0, 100.0), 1.0);
        let mut borrowed = KuiCtx::borrowing_in(&mut ui);
        let ctx: *mut KuiCtx = &mut borrowed;

        // The library itself is beside the point: this is refused before
        // anything is opened, so even one that would load is.
        let lib = crate::ext::tests::a_library_with_no_kui_symbols();
        assert!(!kui_ctx_add_extension(ctx, ks("libc"), ks(lib)));
        assert_eq!(kui_ctx_extension_count(ctx), 0);

        let mut out = ks("");
        assert!(kui_ctx_extension_error(ctx, &mut out));
        let msg =
            std::str::from_utf8(unsafe { std::slice::from_raw_parts(out.ptr, out.len) }).unwrap();
        assert!(
            msg.contains("borrows a frame"),
            "the reason should name the borrowed frame, got {msg:?}"
        );
    }
}
