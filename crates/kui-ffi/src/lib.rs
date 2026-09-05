//! C API for kui. The IR is plain data, so this layer is translation, not
//! architecture: repr(C) mirrors of the spec structs, opaque handles for
//! `Core` and `Value`, and flat builder calls delegating to `Core`'s
//! non-panicking frame API. See include/kui.h for the C-side contract.
//!
//! Conventions:
//! - Strings cross as (ptr, len), UTF-8; invalid bytes are replaced.
//! - `KuiValue*` created by `kui_value_*` constructors is owned by the caller
//!   until passed to a function documented as consuming it.
//! - Every entry point catches panics and turns them into no-ops/false.

// Safe extern fns taking raw pointers is the point of this layer: every
// entry point null-checks and catches panics instead of being `unsafe`.
#![allow(clippy::not_unsafe_ptr_arg_deref)]

// The other direction: a C shared library as a guest inside a host that
// already owns the frame. See the module docs for where its `kui_*` symbols
// come from, which is the only interesting part.
mod ext;
pub use ext::CExtension;

#[macro_use]
mod abi;
mod access;
mod convert;
mod focus;
mod frame;
mod input;
mod resources;
mod run;
mod scrolling;
mod types;
mod value;
mod widgets;
mod windows;

pub use abi::*;
pub use access::*;
pub use focus::*;
pub use frame::*;
pub use input::*;
pub use resources::*;
pub use run::*;
pub use scrolling::*;
pub use types::*;
pub use value::*;
pub use widgets::*;
pub use windows::*;

use convert::*;

use std::collections::VecDeque;
use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use kui_core::{
    Align, Color, Core, Edges, EditKey, EditOptions, Enter, FloatConfig, InputEvent, Key, Keyframe,
    Mods, MouseButton, NodeSpec, Rect, Size, Sizing, Span, TextStyle, UiEvent, Value, Vec2,
    WindowButton, WindowCommand, WindowConfig, WindowId, WindowKind,
};

// ---------------------------------------------------------------------------
// Context lifecycle + input

#[unsafe(no_mangle)]
pub extern "C" fn kui_ctx_new() -> *mut KuiCtx {
    guard(std::ptr::null_mut(), || {
        let mut owned = Box::new(Core::new());
        // A host driving its own frames opts into diagnostics explicitly,
        // like everything else it drains; kui_run follows the runner's
        // debug-build default.
        owned.set_diagnostics(false);
        let core: *mut Core = &mut *owned;
        Box::into_raw(Box::new(KuiCtx {
            core,
            _owned: Some(owned),
            events: Vec::new(),
            last_payload: None,
            last_edit_text: None,
            last_warnings: Vec::new(),
            last_access: Default::default(),
            open_tooltips: Vec::new(),
            window_commands: VecDeque::new(),
            last_window_name: None,
        }))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn kui_ctx_free(ptr: *mut KuiCtx) {
    if !ptr.is_null() {
        drop(unsafe { Box::from_raw(ptr) });
    }
}

// ---------------------------------------------------------------------------
// Host environment

/// Host facts for views to read (`refresh_hz <= 0` = unknown). Survives
/// across frames; set on change or every frame, either works.
#[unsafe(no_mangle)]
pub extern "C" fn kui_env_set(ptr: *mut KuiCtx, refresh_hz: f32, focused: bool) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().env.refresh_hz = (refresh_hz > 0.0).then_some(refresh_hz);
            c.core().env.focused = focused;
        }
    });
}

/// The frame clock for transitions (monotonic seconds, any origin). Set it
/// before each kui_frame_begin; a host that never does sees transitions
/// snap to their targets.
#[unsafe(no_mangle)]
pub extern "C" fn kui_set_time(ptr: *mut KuiCtx, now_secs: f64) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().set_time(now_secs);
        }
    });
}

/// True when the last frame left a transition mid-flight: draw another
/// frame without waiting for input.
#[unsafe(no_mangle)]
pub extern "C" fn kui_animating(ptr: *mut KuiCtx) -> bool {
    guard(false, || {
        unsafe { ctx(ptr) }.is_some_and(|c| c.core().animating())
    })
}

/// Rasterize outline glyphs as LCD subpixel coverage (`KUI_QUAD_GLYPH_SUBPIXEL`,
/// atlas rgb = per-channel coverage) instead of alpha masks. Only for
/// renderers that blend per channel; flipping it re-rasterizes every glyph.
#[unsafe(no_mangle)]
pub extern "C" fn kui_set_subpixel_text(ptr: *mut KuiCtx, on: bool) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().set_subpixel_text(on);
        }
    });
}

/// Drains the warnings the core raised since the last call (silent
/// misconfigurations it noticed while finishing frames; each once) into
/// `out`, up to `cap`; returns the count. The strings stay valid until the
/// next call on this context. Standalone contexts start with the checks
/// off (`kui_set_diagnostics` turns them on); kui_run prints them to
/// stderr itself in debug builds.
#[unsafe(no_mangle)]
pub extern "C" fn kui_take_warnings(ptr: *mut KuiCtx, out: *mut KuiWarning, cap: usize) -> usize {
    guard(0, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return 0;
        };
        if out.is_null() || cap == 0 {
            return 0;
        }
        c.last_warnings = c.core().take_warnings();
        let n = c.last_warnings.len().min(cap);
        for (i, w) in c.last_warnings.iter().take(n).enumerate() {
            let s = |s: &str| KuiStr {
                ptr: s.as_ptr(),
                len: s.len(),
            };
            unsafe {
                out.add(i).write(KuiWarning {
                    code: s(w.code),
                    key: w.key.0,
                    message: s(&w.message),
                })
            };
        }
        n
    })
}

/// Turns the diagnostic checks behind kui_take_warnings on or off (off by
/// default for a standalone context: a development build opts in).
#[unsafe(no_mangle)]
pub extern "C" fn kui_set_diagnostics(ptr: *mut KuiCtx, on: bool) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().set_diagnostics(on);
        }
    });
}

// ---------------------------------------------------------------------------
// Parity with the shared prop schema. `KuiSpec` has to be a static repr(C)
// layout, so it cannot read `kui_core::schema::PROPS` at runtime the way Lua
// and Node do — instead these tests pin it to the table: a schema row with
// no C counterpart fails `every_schema_prop_has_a_c_counterpart`.

#[cfg(test)]
mod schema_parity;

#[cfg(test)]
mod tests;

// ---------------------------------------------------------------------------
// ABI parity with include/kui.h. The header is hand-written (it carries prose
// the generator would lose), so nothing in Rust makes it match the structs
// above: a field added to `KuiSpec` but missing from - or misordered in - the
// header silently shifts every field after it at runtime. This module writes
// `target/kui-abi-assert.c`, a translation unit of `_Static_assert`s pinning
// each field's offset, size and C type to what Rust actually lays out, and
// each member of the enums the API reads as list indices to its position in
// the list; examples/c/build.sh compiles it against the header, in CI too.

/// The two halves of ADR 0004's named gap: a version a host can compare,
/// and a size on every struct the library writes into the host's memory.
#[cfg(test)]
mod abi_handshake {
    use super::*;

    fn ks(s: &str) -> KuiStr {
        KuiStr {
            ptr: s.as_ptr(),
            len: s.len(),
        }
    }

    /// A finished frame holding one clickable scroll container, and the
    /// click: enough for every out-param below to have something real to
    /// refuse to write.
    fn ctx_with_a_scroller_and_a_pending_event() -> *mut KuiCtx {
        let ctx = kui_ctx_new();
        let mut spec = unsafe { std::mem::zeroed::<KuiSpec>() };
        spec.width = KuiSizing {
            tag: 2,
            value: 100.0,
        };
        spec.height = KuiSizing {
            tag: 2,
            value: 50.0,
        };
        spec.overflow = 1 | 4; // KUI_CLIP | KUI_SCROLL_Y
        kui_frame_begin(ctx, 200.0, 100.0, 1.0);
        kui_open_keyed(ctx, ks("scroller"), &spec, kui_value_str(ks("hit")));
        kui_close(ctx);
        kui_frame_finish(ctx);
        kui_input_cursor(ctx, 10.0, 10.0);
        kui_input_mouse(ctx, true, 1);
        kui_input_mouse(ctx, false, 1);
        ctx
    }

    fn window_cmds(ctx: *mut KuiCtx) -> Vec<KuiWindowCommand> {
        let mut out = Vec::new();
        let mut cmd = KuiWindowCommand::default();
        while kui_take_window_command(ctx, &raw mut cmd) {
            out.push(KuiWindowCommand {
                size: cmd.size,
                kind: cmd.kind,
                window: cmd.window,
                origin: cmd.origin,
                config: cmd.config,
                width: cmd.width,
                height: cmd.height,
            });
        }
        out
    }

    /// `(kind, phase)` of every `window` event queued.
    fn window_events(ctx: *mut KuiCtx) -> Vec<(String, String)> {
        let mut out = Vec::new();
        let mut ev = KuiEvent::default();
        while kui_poll_event(ctx, &raw mut ev) {
            let payload = unsafe { &(*ev.payload).0 };
            let kind = payload.get("kind").and_then(Value::as_str).unwrap_or("-");
            if kind != "window" {
                continue;
            }
            let phase = payload.get("phase").and_then(Value::as_str).unwrap_or("-");
            let name = payload.get("name").and_then(Value::as_str).unwrap_or("-");
            out.push((phase.to_string(), name.to_string()));
        }
        out
    }

    fn frame_declaring(ctx: *mut KuiCtx, declare: &[(f32, f32)]) {
        kui_frame_begin(ctx, 320.0, 240.0, 1.0);
        for (w, h) in declare {
            let cfg = KuiWindowConfig {
                kind: KUI_WINDOW_KIND_NORMAL,
                width: *w,
                height: *h,
                activates: 1,
            };
            kui_window_declare(ctx, ks("palette"), &cfg);
        }
        kui_frame_finish(ctx);
    }

    /// ADR 0004's step 3 through the C surface: a declaration opens a
    /// window (once, with the first config, warning about the second),
    /// an OS close reported back keeps it closed while declared, and the
    /// declaration lapsing and starting again opens it anew.
    #[test]
    fn a_declared_window_opens_closes_and_warns_through_the_c_api() {
        let ctx = kui_ctx_new();
        kui_set_diagnostics(ctx, true);
        assert_eq!(kui_ctx_window(ctx), 0);
        let mut name = KuiStr {
            ptr: std::ptr::null(),
            len: 0,
        };
        assert!(kui_ctx_window_name(ctx, &raw mut name));
        assert_eq!(&*kstr(name), "main");

        frame_declaring(ctx, &[(400.0, 300.0), (500.0, 500.0)]);
        let cmds = window_cmds(ctx);
        assert_eq!(cmds.len(), 1);
        assert_eq!(
            (cmds[0].kind, cmds[0].window, cmds[0].origin),
            (KUI_CMD_OPEN, 1, 0)
        );
        assert_eq!(
            (
                cmds[0].config.width,
                cmds[0].config.height,
                cmds[0].config.activates
            ),
            (400.0, 300.0, 1),
            "the first declaration's config, not the second's"
        );
        assert_eq!(cmds[0].size, std::mem::size_of::<KuiWindowCommand>() as u32);
        assert_eq!(
            window_events(ctx),
            vec![("opened".into(), "palette".into())]
        );

        // The user closes it.
        kui_window_closed(ctx, 1);
        assert_eq!(
            window_events(ctx),
            vec![("closed".into(), "palette".into())]
        );
        // Still declared: nothing reopens, and the diagnostics say why.
        frame_declaring(ctx, &[(400.0, 300.0)]);
        assert!(window_cmds(ctx).is_empty());
        let mut warnings = [KuiWarning {
            code: ks(""),
            key: 0,
            message: ks(""),
        }; 8];
        let n = kui_take_warnings(ctx, warnings.as_mut_ptr(), 8);
        let codes: Vec<String> = warnings[..n]
            .iter()
            .map(|w| kstr(w.code).into_owned())
            .collect();
        assert_eq!(
            codes,
            vec![
                "duplicate-window-config".to_string(),
                "window-declared-while-closed".to_string()
            ]
        );

        // The declaration lapses, then starts again: a new window, new id.
        frame_declaring(ctx, &[]);
        assert!(window_cmds(ctx).is_empty());
        frame_declaring(ctx, &[(400.0, 300.0)]);
        let cmds = window_cmds(ctx);
        assert_eq!(cmds.len(), 1);
        assert_eq!((cmds[0].kind, cmds[0].window), (KUI_CMD_OPEN, 2));
        // And stops: the diff closes it.
        frame_declaring(ctx, &[]);
        let cmds = window_cmds(ctx);
        assert_eq!(cmds.len(), 1);
        assert_eq!((cmds[0].kind, cmds[0].window), (KUI_CMD_CLOSE, 2));
        assert_eq!(
            window_events(ctx),
            vec![
                ("opened".into(), "palette".into()),
                ("closed".into(), "palette".into())
            ]
        );
        kui_ctx_free(ctx);
    }

    /// The size handshake on the new [out] struct, the way `kui_poll_event`
    /// has it: a reservation below the layout is refused before anything
    /// is popped, so the command is still there for a proper call.
    #[test]
    fn a_short_window_command_reservation_is_refused_and_keeps_the_command() {
        let ctx = kui_ctx_new();
        frame_declaring(ctx, &[(400.0, 300.0)]);
        let mut short = KuiWindowCommand {
            size: 4,
            kind: 0xdead,
            ..Default::default()
        };
        assert!(!kui_take_window_command(ctx, &raw mut short));
        assert_eq!(short.kind, 0xdead, "nothing was written");
        let cmds = window_cmds(ctx);
        assert_eq!(cmds.len(), 1, "the refused command is still queued");
        assert_eq!(cmds[0].kind, KUI_CMD_OPEN);
        kui_ctx_free(ctx);
    }

    /// A context standing in for a declared window: the id
    /// `kui_env_set_window` gives it is what its events carry and what
    /// its name resolves through.
    #[test]
    fn env_set_window_names_the_context_and_stamps_its_events() {
        let ctx = kui_ctx_new();
        frame_declaring(ctx, &[(400.0, 300.0)]);
        window_cmds(ctx);
        window_events(ctx);
        kui_env_set_window(ctx, 1, false, false, false, 0.0, 0.0);
        assert_eq!(kui_ctx_window(ctx), 1);
        let mut name = KuiStr {
            ptr: std::ptr::null(),
            len: 0,
        };
        assert!(kui_ctx_window_name(ctx, &raw mut name));
        assert_eq!(&*kstr(name), "palette");
        // A frame at another viewport raises a resize, stamped with the id.
        kui_frame_begin(ctx, 200.0, 100.0, 1.0);
        kui_window_declare(ctx, ks("palette"), std::ptr::null());
        kui_frame_finish(ctx);
        let mut ev = KuiEvent::default();
        assert!(kui_poll_event(ctx, &raw mut ev));
        assert_eq!(ev.window, 1);
        kui_ctx_free(ctx);
    }

    #[test]
    fn the_exported_version_is_the_one_the_header_states() {
        // The C side of this is a _Static_assert in mod abi_parity; this is
        // the half that survives the header being absent.
        assert_eq!(kui_abi_version(), KUI_ABI_VERSION);
    }

    /// The whole point of the size field, on a struct that has already
    /// grown: a caller that reserved only the first two fields gets them,
    /// and the third — the appended one — is left exactly as it was.
    ///
    /// This was written before any real [out] struct had grown, so that
    /// the truncating path would not be first exercised by the change that
    /// depends on it. `KuiEvent` has since grown `window`, and
    /// [`an_abi_3_host_polls_events_without_seeing_the_appended_window`]
    /// runs the same path through the public API — this one stays as the
    /// unit-level statement of the rule, on a struct with nothing else
    /// going on.
    #[test]
    fn a_short_reservation_is_filled_only_as_far_as_it_goes() {
        #[repr(C)]
        #[derive(Clone, Copy)]
        struct Grown {
            size: u32,
            was_always_here: u32,
            appended_later: u32,
        }
        // SAFETY: repr(C) with `size: u32` first.
        unsafe impl OutParam for Grown {
            const ABI_V1_SIZE: u32 = abi_through!(Grown, was_always_here, u32);
            fn size_mut(&mut self) -> &mut u32 {
                &mut self.size
            }
        }

        // A host built before `appended_later` existed: it reserved the
        // whole struct it knew, which is the ABI-1 layout.
        let mut old = Grown {
            size: Grown::ABI_V1_SIZE,
            was_always_here: 0,
            appended_later: 0xdeadbeef,
        };
        assert!(write_out(
            &raw mut old,
            Grown {
                size: 0,
                was_always_here: 7,
                appended_later: 9,
            },
        ));
        assert_eq!(old.was_always_here, 7);
        assert_eq!(
            old.appended_later, 0xdeadbeef,
            "the library wrote past what the caller reserved"
        );
        assert_eq!(old.size, Grown::ABI_V1_SIZE, "size reports what was filled");

        // A host built after: it gets everything, and `size` says so.
        let mut new = Grown {
            size: std::mem::size_of::<Grown>() as u32,
            was_always_here: 0,
            appended_later: 0,
        };
        assert!(write_out(
            &raw mut new,
            Grown {
                size: 0,
                was_always_here: 7,
                appended_later: 9,
            },
        ));
        assert_eq!((new.was_always_here, new.appended_later), (7, 9));
        assert_eq!(new.size, std::mem::size_of::<Grown>() as u32);

        // And `size` coming back as the filled count is idempotent, so a
        // loop reusing one struct clamps to the same prefix every time.
        assert!(write_out(
            &raw mut old,
            Grown {
                size: 0,
                was_always_here: 11,
                appended_later: 0,
            },
        ));
        assert_eq!((old.size, old.was_always_here), (Grown::ABI_V1_SIZE, 11));
        assert_eq!(old.appended_later, 0xdeadbeef);
    }

    /// The same rule on the real struct, through the real entry point:
    /// `KuiEvent.window` is ABI 4's append, and a host that predates it —
    /// one whose `KuiEvent` ends after `payload`, which is the ABI-1 floor
    /// `out_accepts` measures against — keeps polling events and simply
    /// never sees the new field.
    ///
    /// The old host is spelled as a reservation rather than as a second
    /// struct because that is all the library ever sees of it: four bytes
    /// of `size`, and a promise about what lies behind them.
    #[test]
    fn an_abi_3_host_polls_events_without_seeing_the_appended_window() {
        let ctx = ctx_with_a_scroller_and_a_pending_event();
        let abi3 = KuiEvent::ABI_V1_SIZE;
        assert!(
            (abi3 as usize) < std::mem::size_of::<KuiEvent>(),
            "`window` must sit past the ABI-1 layout, or this proves nothing"
        );

        let mut ev = KuiEvent {
            size: abi3,
            window: 0xdead,
            ..Default::default()
        };
        assert!(kui_poll_event(ctx, &raw mut ev), "the event still arrives");
        assert_ne!(ev.key, 0, "and the fields it knows are filled");
        assert!(!ev.payload.is_null());
        assert_eq!(ev.size, abi3, "`size` reports the prefix that was filled");
        assert_eq!(
            ev.window, 0xdead,
            "the library wrote past what an ABI 3 host reserved"
        );

        kui_ctx_free(ctx);
    }

    /// And a host built against this ABI gets the field, which is 0 until
    /// ADR 0004's step 3 opens a second window.
    #[test]
    fn a_current_host_sees_window_and_it_is_the_main_one() {
        let ctx = ctx_with_a_scroller_and_a_pending_event();
        let mut ev = KuiEvent::default();
        assert!(kui_poll_event(ctx, &raw mut ev));
        assert_eq!(ev.window, 0);
        assert_eq!(ev.size, std::mem::size_of::<KuiEvent>() as u32);
        kui_ctx_free(ctx);
    }

    /// A reservation smaller than ABI 1 is refused rather than guessed at —
    /// and refused *before* the queue moves, so the event is still there
    /// for a caller that asks properly.
    #[test]
    fn an_unreadable_reservation_refuses_without_dropping_the_event() {
        let ctx = ctx_with_a_scroller_and_a_pending_event();
        // 4 bytes: what an un-set `size`, or one from a host predating the
        // field, looks like to this library.
        let mut stale = KuiEvent {
            size: 4,
            ..Default::default()
        };
        assert!(!kui_poll_event(ctx, &raw mut stale));
        assert_eq!(stale.key, 0, "nothing was written");

        let mut ev = KuiEvent::default();
        assert!(
            kui_poll_event(ctx, &raw mut ev),
            "the event was not dropped"
        );
        assert!(!ev.payload.is_null());
        kui_ctx_free(ctx);
    }

    /// The same refusal on the other three, so the rule is the family's and
    /// not `kui_poll_event`'s.
    #[test]
    fn every_out_param_refuses_a_reservation_it_cannot_honour() {
        let ctx = ctx_with_a_scroller_and_a_pending_event();

        // A key with real geometry behind it, so the refusal below is the
        // reservation being rejected and not the lookup missing.
        let scroller = kui_child_key(ctx, ks("scroller"));
        let mut geom = KuiScrollGeometry::default();
        assert!(kui_scroll_geometry(ctx, scroller, &raw mut geom));
        assert!(geom.h > 0.0);
        let mut short = KuiScrollGeometry {
            size: 2,
            ..Default::default()
        };
        assert!(!kui_scroll_geometry(ctx, scroller, &raw mut short));
        assert_eq!(short.h, 0.0, "nothing was written");

        let mut m = KuiTextMetrics {
            size: 0,
            ..Default::default()
        };
        let style: KuiTextStyle = unsafe { std::mem::zeroed() };
        assert!(!kui_measure_text(ctx, ks("hello"), &style, 0.0, &raw mut m));
        assert_eq!(m.width, 0.0);

        let mut draw = KuiDrawData {
            size: 1,
            ..Default::default()
        };
        assert!(!kui_draw_data(ctx, &raw mut draw));
        assert!(draw.quads.is_null());
        // Refused, so the atlas is still owed to whoever asks next.
        let mut good = KuiDrawData::default();
        assert!(kui_draw_data(ctx, &raw mut good));
        assert_eq!(good.size, std::mem::size_of::<KuiDrawData>() as u32);

        // NULL is the older half of the same rule and still holds.
        assert!(!kui_poll_event(ctx, std::ptr::null_mut()));
        assert!(!kui_draw_data(ctx, std::ptr::null_mut()));
        kui_ctx_free(ctx);
    }
}

#[cfg(test)]
mod abi_parity {
    use super::*;
    use std::fmt::Write as _;

    /// One repr(C) struct's ABI, restated as a table and emitted as C.
    ///
    /// The table is pinned to the Rust definition at compile time: the
    /// destructuring names every field, so adding one to the struct stops
    /// this module compiling until it is listed here (and mirrored in
    /// include/kui.h), and each binding is checked against the Rust type the
    /// row declares, so `radius: f32 => "float"` cannot outlive a change to
    /// `radius: u32`. Offsets, sizes and alignments come from Rust itself.
    macro_rules! abi_struct {
        ($out:expr, $ty:ident { $($f:ident : $rt:ty => $c:literal),* $(,)? }) => {{
            // Rebuilding the struct field by field is the pin: a field
            // added to $ty and not to the table below is a missing field in
            // this initializer (E0063, by name), and a field whose Rust type
            // changed no longer matches the `$rt` the row declares.
            #[allow(dead_code)]
            fn pinned(v: $ty) -> $ty {
                $(let $f: $rt = v.$f;)*
                $ty { $($f),* }
            }
            writeln!(
                $out,
                "KUI_STRUCT({}, {}, {});",
                stringify!($ty),
                std::mem::size_of::<$ty>(),
                std::mem::align_of::<$ty>(),
            )
            .unwrap();
            $(writeln!(
                $out,
                "KUI_FIELD({}, {}, {}, {}, {});",
                stringify!($ty),
                stringify!($f),
                $c,
                std::mem::offset_of!($ty, $f),
                std::mem::size_of::<$rt>(),
            )
            .unwrap();)*
            writeln!($out).unwrap();
        }};
    }

    /// One [out] struct's size handshake, restated as C.
    ///
    /// The `abi_struct!` row above it already pins `size`'s offset, size and
    /// type; this adds the two facts that make the handshake work and that
    /// no field-by-field check would notice: that `size` is the *first*
    /// field (so a library can read a caller's reservation before trusting
    /// anything else in the struct), and that the layout has never shrunk
    /// below what ABI 1 shipped (so `ABI_V1_SIZE` is still a floor and not
    /// a ceiling). The floor comes from Rust's own `OutParam` impl, so the
    /// two cannot drift.
    macro_rules! abi_out_struct {
        ($out:expr, $ty:ident) => {{
            writeln!(
                $out,
                "KUI_OUT_STRUCT({}, {});",
                stringify!($ty),
                <$ty as OutParam>::ABI_V1_SIZE,
            )
            .unwrap();
            writeln!($out).unwrap();
        }};
    }

    /// One C enum whose members are the indices of a list Rust owns,
    /// restated as C.
    ///
    /// The values come from Rust and the spellings from the header, and the
    /// array's length is the list's, so a member added to the list — a role
    /// appended to `Role::ALL` — stops this module compiling until the
    /// header's name for it is listed here, and a name the header does not
    /// define stops the generated C compiling. `$base` is the value index 0
    /// takes in C: 1 wherever zero has to stay free for "unset".
    macro_rules! abi_enum {
        ($out:expr, $list:expr, $base:expr => [$($c:literal),* $(,)?]) => {{
            const NAMES: [&str; $list.len()] = [$($c),*];
            for (i, name) in NAMES.iter().enumerate() {
                writeln!($out, "KUI_ENUM({}, {});", name, i + $base).unwrap();
            }
            writeln!($out).unwrap();
        }};
    }

    const PRELUDE: &str = r#"/* Generated by `cargo test -p kui-ffi abi_parity` (see
 * crates/kui-ffi/src/lib.rs, mod abi_parity) - do not edit, do not commit.
 * examples/c/build.sh regenerates and compiles it.
 *
 * Every field of every public repr(C) struct in the C API is pinned to the
 * offset, size and type Rust lays it out with, so a header that has drifted
 * from crates/kui-ffi/src/lib.rs fails to compile here instead of misreading
 * every field after the drift at runtime. Nothing links: the asserts are
 * settled in the front end.
 */
#include "kui.h"

#define KUI_STRUCT(T, size, align)                                            \
    _Static_assert(sizeof(T) == (size), "sizeof(" #T ") differs from Rust");  \
    _Static_assert(_Alignof(T) == (align), "_Alignof(" #T ") differs from Rust")

/* CT is the C type the field must have: same size at the same offset is not
 * enough, `float` and `uint32_t` swap silently. Arrays are named by what
 * they decay to (`float *` for `float[4]`); their length is covered by the
 * size assert. */
#define KUI_FIELD(T, f, CT, off, size)                                        \
    _Static_assert(offsetof(T, f) == (off), #T "." #f ": offset differs from Rust");    \
    _Static_assert(sizeof(((T *)0)->f) == (size), #T "." #f ": size differs from Rust"); \
    _Static_assert(_Generic(((T *)0)->f, CT: 1, default: 0), #T "." #f ": type differs from Rust")

/* An [out] struct: one the library writes into memory the host reserved.
 * `size` has to lead it, because the library reads the host's reservation
 * out of those four bytes before it trusts any other byte of the struct;
 * and v1 is the layout ABI 1 shipped, which is a floor the struct may grow
 * past but must never fall below - a removed or narrowed field would have
 * an old host's reservation accepted and then under-filled. */
#define KUI_OUT_STRUCT(T, v1)                                                 \
    _Static_assert(offsetof(T, size) == 0, #T ".size must be the first field"); \
    _Static_assert(sizeof(T) >= (v1), #T " shrank below its ABI 1 layout")

/* One member of an enum the library reads as an index into a list Rust owns.
 * The value assert catches a member that drifted; the name itself catches
 * the commoner failure, a list that grew and a header that did not, which
 * fails here as an undeclared identifier instead of leaving C the one
 * binding that cannot say the new value. */
#define KUI_ENUM(name, value)                                                 \
    _Static_assert((name) == (value), #name " differs from Rust")
"#;

    fn asserts() -> String {
        let mut o = String::from(PRELUDE);
        o.push('\n');

        // The header's KUI_ABI_VERSION is what a host compares against
        // kui_abi_version() at startup, so a bump made in one place and not
        // the other would make that check pass on a mismatched pair.
        writeln!(
            o,
            "_Static_assert(KUI_ABI_VERSION == {}u, \"KUI_ABI_VERSION differs from Rust\");\n",
            KUI_ABI_VERSION,
        )
        .unwrap();

        // The enums whose members are positions in a list the core owns, so
        // that a list growing on the Rust side cannot leave the header - the
        // one binding that does not read the tables at runtime - without a
        // name for the new member.
        abi_enum!(o, kui_core::Role::ALL, 1 => [
            "KUI_ROLE_NONE", "KUI_ROLE_BUTTON", "KUI_ROLE_CHECKBOX",
            "KUI_ROLE_RADIO", "KUI_ROLE_SWITCH", "KUI_ROLE_SLIDER",
            "KUI_ROLE_TAB", "KUI_ROLE_TAB_LIST", "KUI_ROLE_LINK",
            "KUI_ROLE_HEADING", "KUI_ROLE_LIST", "KUI_ROLE_LIST_ITEM",
            "KUI_ROLE_IMAGE", "KUI_ROLE_DIALOG", "KUI_ROLE_GROUP",
            "KUI_ROLE_WINDOW", "KUI_ROLE_TITLE_BAR", "KUI_ROLE_STATIC_TEXT",
            "KUI_ROLE_TEXT_INPUT", "KUI_ROLE_MULTILINE_TEXT_INPUT",
            "KUI_ROLE_SCROLL_VIEW", "KUI_ROLE_LINE",
            "KUI_ROLE_RADIO_GROUP", "KUI_ROLE_MENU", "KUI_ROLE_MENU_ITEM",
        ]);
        abi_enum!(o, kui_core::schema::ORIENTATIONS, 1 => [
            "KUI_ORIENTATION_HORIZONTAL", "KUI_ORIENTATION_VERTICAL",
        ]);
        abi_enum!(o, kui_core::schema::CURSORS, 1 => [
            "KUI_CURSOR_DEFAULT", "KUI_CURSOR_TEXT", "KUI_CURSOR_POINTER",
            "KUI_CURSOR_GRAB", "KUI_CURSOR_GRABBING", "KUI_CURSOR_NOT_ALLOWED",
            "KUI_CURSOR_EW_RESIZE", "KUI_CURSOR_NS_RESIZE",
            "KUI_CURSOR_NWSE_RESIZE", "KUI_CURSOR_NESW_RESIZE",
        ]);
        abi_enum!(o, kui_core::schema::EXPANDED, 1 => [
            "KUI_EXPANDED_COLLAPSED", "KUI_EXPANDED_EXPANDED",
        ]);
        abi_enum!(o, kui_core::schema::WINDOW_ROLES, 1 => [
            "KUI_WINDOW_DRAG", "KUI_WINDOW_CLOSE", "KUI_WINDOW_MINIMIZE",
            "KUI_WINDOW_MAXIMIZE",
        ]);
        abi_enum!(o, kui_core::schema::ALIGNS, 0 => [
            "KUI_START", "KUI_CENTER", "KUI_END",
        ]);
        abi_enum!(o, kui_core::schema::FAMILIES, 0 => [
            "KUI_FONT_SANS", "KUI_FONT_SERIF", "KUI_FONT_MONO",
        ]);
        abi_enum!(o, kui_core::schema::WRAPS, 0 => [
            "KUI_WRAP_WORD", "KUI_WRAP_GLYPH", "KUI_WRAP_NONE",
        ]);
        abi_enum!(o, kui_core::schema::EASINGS, 0 => [
            "KUI_EASE_OUT", "KUI_EASE_LINEAR", "KUI_EASE_IN",
            "KUI_EASE_IN_OUT", "KUI_EASE_SPRING", "KUI_EASE_BOUNCY",
        ]);
        abi_enum!(o, kui_core::schema::REPEATS, 0 => [
            "KUI_REPEAT_NORMAL", "KUI_REPEAT_REVERSE", "KUI_REPEAT_ALTERNATE",
            "KUI_REPEAT_ALTERNATE_REVERSE",
        ]);

        abi_struct!(o, KuiScrollGeometry {
            size: u32 => "uint32_t",
            x: f32 => "float",
            y: f32 => "float",
            w: f32 => "float",
            h: f32 => "float",
            content_w: f32 => "float",
            content_h: f32 => "float",
            offset_x: f32 => "float",
            offset_y: f32 => "float",
            max_offset_x: f32 => "float",
            max_offset_y: f32 => "float",
        });
        abi_out_struct!(o, KuiScrollGeometry);

        abi_struct!(o, KuiStr {
            ptr: *const u8 => "const uint8_t *",
            len: usize => "size_t",
        });

        abi_struct!(o, KuiSizing {
            tag: u32 => "uint32_t",
            value: f32 => "float",
        });

        abi_struct!(o, KuiKeyframe {
            set: u32 => "uint32_t",
            at: f32 => "float",
            width: KuiSizing => "KuiSizing",
            height: KuiSizing => "KuiSizing",
            bg: u32 => "uint32_t",
            radius: f32 => "float",
            opacity: f32 => "float",
        });

        abi_struct!(o, KuiEnter {
            set: u32 => "uint32_t",
            dx: f32 => "float",
            dy: f32 => "float",
            width: KuiSizing => "KuiSizing",
            height: KuiSizing => "KuiSizing",
            bg: u32 => "uint32_t",
            radius: f32 => "float",
            opacity: f32 => "float",
        });

        abi_struct!(o, KuiSpec {
            width: KuiSizing => "KuiSizing",
            height: KuiSizing => "KuiSizing",
            min_w: f32 => "float",
            max_w: f32 => "float",
            min_h: f32 => "float",
            max_h: f32 => "float",
            dir: u32 => "uint32_t",
            pad_l: f32 => "float",
            pad_r: f32 => "float",
            pad_t: f32 => "float",
            pad_b: f32 => "float",
            gap: f32 => "float",
            main_align: u32 => "uint32_t",
            cross_align: u32 => "uint32_t",
            bg: u32 => "uint32_t",
            border_color: u32 => "uint32_t",
            border_w: f32 => "float",
            radius: f32 => "float",
            overflow: u32 => "uint32_t",
            float_mode: u32 => "uint32_t",
            float_anchor_x: u32 => "uint32_t",
            float_anchor_y: u32 => "uint32_t",
            float_self_x: u32 => "uint32_t",
            float_self_y: u32 => "uint32_t",
            float_dx: f32 => "float",
            float_dy: f32 => "float",
            float_fit: u32 => "uint32_t",
            hoverable: u32 => "uint32_t",
            window_role: u32 => "uint32_t",
            transition_ms: f32 => "float",
            easing: u32 => "uint32_t",
            slide: u32 => "uint32_t",
            hover_bg: u32 => "uint32_t",
            pressed_bg: u32 => "uint32_t",
            hover_group: KuiStr => "KuiStr",
            per_corner: u32 => "uint32_t",
            radius_tl: f32 => "float",
            radius_tr: f32 => "float",
            radius_br: f32 => "float",
            radius_bl: f32 => "float",
            repeat: u32 => "uint32_t",
            delay_ms: f32 => "float",
            keyframes: *const KuiKeyframe => "const KuiKeyframe *",
            keyframes_len: usize => "size_t",
            enter: KuiEnter => "KuiEnter",
            click_sound: u64 => "uint64_t",
            hover_sound: u64 => "uint64_t",
            on_layout: *const KuiValue => "const KuiValue *",
            role: u32 => "uint32_t",
            label: KuiStr => "KuiStr",
            checked: u32 => "uint32_t",
            value_set: u32 => "uint32_t",
            value_now: f32 => "float",
            value_min: f32 => "float",
            value_max: f32 => "float",
            caret: u32 => "uint32_t",
            selection_anchor: u32 => "uint32_t",
            focusable: u32 => "uint32_t",
            disabled: u32 => "uint32_t",
            focus_bg: u32 => "uint32_t",
            tooltip: KuiStr => "KuiStr",
            modal: *const KuiValue => "const KuiValue *",
            on_context_menu: *const KuiValue => "const KuiValue *",
            cursor: u32 => "uint32_t",
            selected: u32 => "uint32_t",
            expanded: u32 => "uint32_t",
            opacity_set: u32 => "uint32_t",
            opacity: f32 => "float",
            shadow_color: u32 => "uint32_t",
            shadow_blur: f32 => "float",
            shadow_x: f32 => "float",
            shadow_y: f32 => "float",
            shadow_spread: f32 => "float",
            wrap_children: u32 => "uint32_t",
            cross_gap: f32 => "float",
            initial_focus: u32 => "uint32_t",
            exit: KuiEnter => "KuiEnter",
        });

        abi_struct!(o, KuiAccessNode {
            key: u64 => "uint64_t",
            parent: u64 => "uint64_t",
            origin: u32 => "uint32_t",
            role: u32 => "uint32_t",
            flags: u32 => "uint32_t",
            actions: u32 => "uint32_t",
            name: KuiStr => "KuiStr",
            description: KuiStr => "KuiStr",
            value: KuiStr => "KuiStr",
            x: f32 => "float",
            y: f32 => "float",
            w: f32 => "float",
            h: f32 => "float",
            caret: u32 => "uint32_t",
            selection_start: u32 => "uint32_t",
            selection_end: u32 => "uint32_t",
            value_now: f32 => "float",
            value_min: f32 => "float",
            value_max: f32 => "float",
            scroll_x: f32 => "float",
            scroll_y: f32 => "float",
            scroll_max_x: f32 => "float",
            scroll_max_y: f32 => "float",
            anchor_run: u64 => "uint64_t",
            anchor_char: u32 => "uint32_t",
            focus_run: u64 => "uint64_t",
            focus_char: u32 => "uint32_t",
            run_count: u32 => "uint32_t",
            pos_in_set: u32 => "uint32_t",
            set_size: u32 => "uint32_t",
            orientation: u32 => "uint32_t",
        });

        abi_struct!(o, KuiAccessRun {
            key: u64 => "uint64_t",
            line: u32 => "uint32_t",
            start: u32 => "uint32_t",
            end: u32 => "uint32_t",
            text: KuiStr => "KuiStr",
            x: f32 => "float",
            y: f32 => "float",
            w: f32 => "float",
            h: f32 => "float",
            char_count: u32 => "uint32_t",
            char_lengths: *const u8 => "const uint8_t *",
            char_positions: *const f32 => "const float *",
            char_widths: *const f32 => "const float *",
            word_start_count: u32 => "uint32_t",
            word_starts: *const u8 => "const uint8_t *",
            rtl: u32 => "uint32_t",
        });

        abi_struct!(o, KuiTextMetrics {
            size: u32 => "uint32_t",
            width: f32 => "float",
            height: f32 => "float",
            lines: u32 => "uint32_t",
        });
        abi_out_struct!(o, KuiTextMetrics);

        abi_struct!(o, KuiWarning {
            code: KuiStr => "KuiStr",
            key: u64 => "uint64_t",
            message: KuiStr => "KuiStr",
        });

        abi_struct!(o, KuiPlay {
            volume: f32 => "float",
            looped: u32 => "uint32_t",
            fade_in_ms: f32 => "float",
        });

        abi_struct!(o, KuiAudio {
            src: u64 => "uint64_t",
            volume: f32 => "float",
            looped: u32 => "uint32_t",
            paused: u32 => "uint32_t",
        });

        abi_struct!(o, KuiAudioCommand {
            kind: u32 => "uint32_t",
            playback: u64 => "uint64_t",
            sound: u64 => "uint64_t",
            volume: f32 => "float",
            ms: f32 => "float",
            looped: u32 => "uint32_t",
        });

        abi_struct!(o, KuiTextStyle {
            size: f32 => "float",
            line_height: f32 => "float",
            color: u32 => "uint32_t",
            family: u32 => "uint32_t",
            font: u64 => "uint64_t",
            wrap: u32 => "uint32_t",
            max_lines: u32 => "uint32_t",
            ellipsis: u32 => "uint32_t",
        });

        abi_struct!(o, KuiSpan {
            text: KuiStr => "KuiStr",
            color: u32 => "uint32_t",
            flags: u32 => "uint32_t",
        });

        abi_struct!(o, KuiEvent {
            size: u32 => "uint32_t",
            origin: u16 => "uint16_t",
            key: u64 => "uint64_t",
            payload: *const KuiValue => "const KuiValue *",
            window: u32 => "uint32_t",
        });
        abi_out_struct!(o, KuiEvent);

        abi_struct!(o, KuiWindowConfig {
            kind: u32 => "uint32_t",
            width: f32 => "float",
            height: f32 => "float",
            activates: u32 => "uint32_t",
        });

        abi_struct!(o, KuiWindowCommand {
            size: u32 => "uint32_t",
            kind: u32 => "uint32_t",
            window: u32 => "uint32_t",
            origin: u16 => "uint16_t",
            config: KuiWindowConfig => "KuiWindowConfig",
            width: f32 => "float",
            height: f32 => "float",
        });
        abi_out_struct!(o, KuiWindowCommand);

        // The command verbs and the window kind are plain constants on
        // both sides; pinned here so the header cannot renumber one.
        for (name, value) in [
            ("KUI_CMD_START_DRAG", KUI_CMD_START_DRAG),
            ("KUI_CMD_CLOSE", KUI_CMD_CLOSE),
            ("KUI_CMD_MINIMIZE", KUI_CMD_MINIMIZE),
            ("KUI_CMD_SET_SIZE", KUI_CMD_SET_SIZE),
            ("KUI_CMD_FOCUS", KUI_CMD_FOCUS),
            ("KUI_CMD_TOGGLE_MAXIMIZE", KUI_CMD_TOGGLE_MAXIMIZE),
            ("KUI_CMD_OPEN", KUI_CMD_OPEN),
            ("KUI_WINDOW_KIND_NORMAL", KUI_WINDOW_KIND_NORMAL),
            ("KUI_WINDOW_MAIN", WindowId::MAIN.0),
        ] {
            writeln!(o, "KUI_ENUM({name}, {value});").unwrap();
        }
        writeln!(o).unwrap();

        abi_struct!(o, KuiQuad {
            x: f32 => "float",
            y: f32 => "float",
            w: f32 => "float",
            h: f32 => "float",
            color: [f32; 4] => "float *",
            border_color: [f32; 4] => "float *",
            radius: [f32; 4] => "float *",
            border_w: f32 => "float",
            blur: f32 => "float",
            kind: u32 => "uint32_t",
            uv: [u32; 4] => "uint32_t *",
            clip: [f32; 4] => "float *",
            clip_radius: [f32; 4] => "float *",
        });

        abi_struct!(o, KuiDrawData {
            size: u32 => "uint32_t",
            quads: *const KuiQuad => "const KuiQuad *",
            quad_count: usize => "size_t",
            viewport_w: f32 => "float",
            viewport_h: f32 => "float",
            scale: f32 => "float",
            atlas_pixels: *const u8 => "const uint8_t *",
            atlas_size: u32 => "uint32_t",
            atlas_dirty: bool => "bool",
            atlas_epoch: u64 => "uint64_t",
        });
        abi_out_struct!(o, KuiDrawData);

        o
    }

    /// Writes the asserts next to the built library, where
    /// examples/c/build.sh picks them up. The C compiler is the check; this
    /// test only produces what it checks (and fails if the tree is not
    /// writable), so the two halves stay one `./examples/c/build.sh` apart.
    #[test]
    fn writes_the_c_abi_asserts() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target");
        std::fs::create_dir_all(&dir).expect("create target dir");
        let path = dir.join("kui-abi-assert.c");
        let text = asserts();
        assert!(text.contains("KUI_FIELD(KuiSpec, focus_bg,"));
        assert!(text.contains("KUI_OUT_STRUCT(KuiEvent,"));
        assert!(text.contains("KUI_OUT_STRUCT(KuiWindowCommand,"));
        assert!(text.contains("KUI_ENUM(KUI_ROLE_LINE, 22);"));
        std::fs::write(&path, text).expect("write kui-abi-assert.c");
    }
}
