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
mod abi_handshake;

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
