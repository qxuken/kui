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
//!
//! Layout: `types` holds the repr(C) mirrors, `convert` the translations
//! into the core's own types, `abi` the version and the size-led [out]
//! handshake; the entry points sit in one module per concern they
//! delegate to (`frame`, `input`, `windows`, `resources`, `focus`,
//! `scrolling`, `access`, `widgets`, `value`, `run`), re-exported here
//! so the crate's surface is flat. What stays in this file is the
//! context itself, the host facts and the diagnostics.

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
    Align, Color, Core, DismissReason, Edges, EditKey, EditOptions, Enter, FloatConfig, InputEvent,
    Key, Keyframe, Mods, MouseButton, NodeSpec, Rect, Size, Sizing, Span, TextStyle, UiEvent,
    Value, Vec2, WindowButton, WindowCommand, WindowConfig, WindowId, WindowKind,
};

// ---------------------------------------------------------------------------
// Context lifecycle

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
            last_announcements: Vec::new(),
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
// in `types`: a field added to `KuiSpec` but missing from - or misordered in - the
// header silently shifts every field after it at runtime. This module writes
// `target/kui-abi-assert.c`, a translation unit of `_Static_assert`s pinning
// each field's offset, size and C type to what Rust actually lays out, and
// each member of the enums the API reads as list indices to its position in
// the list; examples/c/build.sh compiles it against the header, in CI too.

/// The two halves of ADR 0004's named gap: a version a host can compare,
/// and a size on every struct the library writes into the host's memory.
#[cfg(test)]
mod abi_handshake;

#[cfg(test)]
mod abi_parity;
