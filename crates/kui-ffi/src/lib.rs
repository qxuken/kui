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
mod menu;
mod resources;
#[cfg(feature = "runner")]
mod run;
mod scrolling;
mod select;
mod slots;
mod types;
mod value;
mod widgets;
mod windows;

pub use abi::*;
pub use access::*;
pub use focus::*;
pub use frame::*;
pub use input::*;
pub use menu::*;
pub use resources::*;
#[cfg(feature = "runner")]
pub use run::*;
pub use scrolling::*;
pub use select::*;
pub use slots::*;
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
    Align, Appearance, Assistive, AudioDevice, AudioEnv, Color, Core, DismissReason, Edges,
    EditKey, EditOptions, Enter, FloatConfig, InputEvent, Key, Keyframe, Locale, Mods, MotionPref,
    MouseButton, NodeSpec, Rect, Size, Sizing, Span, SystemEnv, TextStyle, UiEvent, Value, Vec2,
    WindowButton, WindowCommand, WindowConfig, WindowId, WindowKind,
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
            fragment_source: String::new(),
            fragment_draws: Vec::new(),
            texture_draws: Vec::new(),
            image_pixels: None,
            last_warnings: Vec::new(),
            last_access: Default::default(),
            last_announcements: Vec::new(),
            window_commands: VecDeque::new(),
            menu_actions: VecDeque::new(),
            menu_text: String::new(),
            menu_accel: String::new(),
            menu_html: String::new(),
            selection_text: String::new(),
            selection_html: String::new(),
            font_families: Vec::new(),
            last_window_name: None,
            slot_name: None,
            slot_namespace: None,
            slot_params: None,
            extensions: Default::default(),
            last_ext_error: String::new(),
            host_ui: std::ptr::null_mut(),
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
/// across frames; set on change or every frame, either works. A window
/// that lost the keyboard lets go of every key its sink was holding
/// (`Core::set_focused`), so the `kui_release_held_keys` a host used to
/// owe on focus loss is owed no more; the synthetic releases are polled
/// like any event.
#[unsafe(no_mangle)]
pub extern "C" fn kui_env_set(ptr: *mut KuiCtx, refresh_hz: f32, focused: bool) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().env.refresh_hz = (refresh_hz > 0.0).then_some(refresh_hz);
            c.core().set_focused(focused);
        }
    });
}

/// What the OS is set to, for views to read: `appearance` is a
/// `KUI_APPEARANCE_*`, `motion` a `KUI_MOTION_*` (0 = unknown in both, so a
/// host that never calls this reports honestly), `accent` the accent colour
/// as `0xRRGGBBAA` (0 = unknown), `locale` a BCP-47 tag (empty = unknown).
/// A tag longer than 31 bytes or not ASCII is not a tag, and reads back as
/// unknown rather than as a truncated one.
///
/// Separate from `kui_env_set` because these change when the user opens a
/// settings app, not when a window moves: push them at startup and on the
/// OS's change notification. Survives across frames either way. On a
/// context handed to `kui_run_with` it is the window's pin over the OS's
/// reading instead (backlog F47); see there.
#[unsafe(no_mangle)]
pub extern "C" fn kui_env_set_system(
    ptr: *mut KuiCtx,
    appearance: u32,
    accent: u32,
    motion: u32,
    locale: KuiStr,
) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            // An out-of-range code is ignored rather than folded onto a
            // real setting: a host built against a newer header says
            // something this build has no name for.
            let sys = SystemEnv {
                appearance: Appearance::from_code(appearance).unwrap_or_default(),
                motion: MotionPref::from_code(motion).unwrap_or_default(),
                accent: (accent != 0).then(|| Color::hex(accent)),
                locale: opt_str(locale).and_then(|tag| Locale::new(&tag)),
                // Not this setter's: the four arguments are the settings,
                // and the fifth reading has its own door below, so a host
                // re-pushing the settings on an OS notification does not
                // forget that a screen reader is attached.
                assistive: c.core().env.system.assistive,
            };
            // `set_system` re-resolves the palette from what was just
            // written, so a host that pushes the appearance and reads
            // `kui_theme` back before its next frame sees the answer
            // (ADR 0019).
            c.core().set_system(sys);
        }
    });
}

/// Whether assistive technology is listening, for views to read
/// (`env.system.assistive`): a `KUI_ASSISTIVE_*` (0 = unknown, so a host
/// with no accessibility bridge reports honestly by never calling this).
/// A host that bridges the platform's accessibility API itself pushes
/// `LISTENING` when a client first asks it for the tree and `NONE` if its
/// platform ever tells it the client left — which, of the AccessKit
/// adapters, only AT-SPI does. An out-of-range code is ignored rather
/// than folded onto a real reading, the way an unknown appearance is.
///
/// Its own setter rather than a fifth argument on `kui_env_set_system`
/// because it is not a setting and does not arrive with them — and
/// because adding an argument would be an ABI break for every host, while
/// a setter is additive (backlog F48).
#[unsafe(no_mangle)]
pub extern "C" fn kui_env_set_assistive(ptr: *mut KuiCtx, assistive: u32) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().env.system.assistive = Assistive::from_code(assistive).unwrap_or_default();
        }
    });
}

/// What the host's audio output is doing, for views to read (`env.audio`):
/// `device` a `KUI_AUDIO_DEVICE_*` (0 = closed, so a host with no device
/// reports honestly by never calling this), `live` the playbacks started
/// or waiting on the open. A fact, not a verb — nothing here closes the
/// device. An out-of-range code is ignored rather than folded onto a real
/// state, the way an unknown appearance is.
#[unsafe(no_mangle)]
pub extern "C" fn kui_env_set_audio(ptr: *mut KuiCtx, device: u32, live: u32) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().env.audio = AudioEnv {
                device: AudioDevice::from_code(device).unwrap_or_default(),
                live,
            };
        }
    });
}

/// This window's palette, as of the current or last frame
/// (`docs/adr/0019-a-theme-derived-from-appearance-and-accent.md`): one
/// `0xRRGGBBAA` per role, derived from what `kui_env_set_system` reported
/// unless this host pinned something with the two setters below.
///
/// The stock widgets already read it — a button, the context menu, a
/// tooltip, a field, the scrollbars, the focus ring and any text with a
/// zero `color` all follow it — so a C host that reports the OS appearance
/// and nothing else already follows the OS. Read it for the paint of your
/// own: `KuiTheme t = KUI_THEME_INIT; kui_theme(ctx, &t);` then
/// `spec.bg = t.surface`.
///
/// False for a bad context, a NULL `out`, or a reservation smaller than
/// the ABI-1 layout.
#[unsafe(no_mangle)]
pub extern "C" fn kui_theme(ptr: *mut KuiCtx, out: *mut KuiTheme) -> bool {
    guard(false, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return false;
        };
        let t = *c.core().theme();
        write_out(
            out,
            KuiTheme {
                appearance: t.appearance.code(),
                disabled_opacity: t.disabled_opacity,
                bg: t.bg.to_hex(),
                surface: t.surface.to_hex(),
                raised: t.raised.to_hex(),
                sunken: t.sunken.to_hex(),
                border: t.border.to_hex(),
                border_strong: t.border_strong.to_hex(),
                fg: t.fg.to_hex(),
                muted: t.muted.to_hex(),
                faint: t.faint.to_hex(),
                accent: t.accent.to_hex(),
                accent_hover: t.accent_hover.to_hex(),
                accent_pressed: t.accent_pressed.to_hex(),
                on_accent: t.on_accent.to_hex(),
                accent_soft: t.accent_soft.to_hex(),
                selection: t.selection.to_hex(),
                focus_ring: t.focus_ring.to_hex(),
                hover: t.hover.to_hex(),
                pressed: t.pressed.to_hex(),
                success: t.success.to_hex(),
                warning: t.warning.to_hex(),
                danger: t.danger.to_hex(),
                scrollbar: t.scrollbar.to_hex(),
                scrollbar_active: t.scrollbar_active.to_hex(),
                ..Default::default()
            },
        )
    })
}

/// Keep following the OS's light/dark, but paint `accent` instead of the
/// OS's — a host with a brand colour of its own. `0xRRGGBBAA`; zero goes
/// back to following the OS for the accent too, which is the default.
///
/// Everything that comes off the accent moves with it: the button's hover
/// and pressed shades, the label on it (black or white, by luminance), the
/// selection tint and the focus ring.
#[unsafe(no_mangle)]
pub extern "C" fn kui_theme_set_accent(ptr: *mut KuiCtx, accent: u32) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            match accent {
                0 => c.core().derive_theme(),
                hex => c.core().set_accent(Color::hex(hex)),
            }
        }
    })
}

/// Pin the whole palette: exactly these colours, following neither the
/// OS's appearance nor its accent. NULL goes back to deriving both.
///
/// The struct is read as the host filled it — `size` is ignored here,
/// since the host wrote every byte it declared — so build one from
/// `kui_theme` and change the roles you mean to change, rather than
/// zeroing a fresh one: a zeroed role is transparent, not "leave it".
#[unsafe(no_mangle)]
pub extern "C" fn kui_theme_set(ptr: *mut KuiCtx, theme: *const KuiTheme) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            match unsafe { theme.as_ref() } {
                None => c.core().derive_theme(),
                Some(t) => c.core().set_theme(theme_of(t)),
            }
        }
    })
}

/// Declare the named colours and lengths this origin references
/// (`docs/adr/0027-tokens-beside-the-theme.md`): the host's outside a
/// plugin's view, the plugin's own inside `kui_ext_view`, so a plugin's
/// declaration never replaces the host's. Replaces that table whole; an
/// app whose lengths follow a viewport tier declares again on a resize. A
/// name a theme or metrics role owns is dropped with a `reserved-token`
/// warning. Either array may be NULL with a zero count.
///
/// A C prop carries no reference — `KuiSpec.bg` is a bare `uint32_t` —
/// so a C host reads a token back with `kui_token_color` /
/// `kui_token_length` and writes the value; what the table buys it is
/// the name in the devtools' inspector, a guest reading the host's
/// vocabulary, and one declaration for a Lua panel it hosts to reference
/// by `"$name"`.
#[unsafe(no_mangle)]
pub extern "C" fn kui_tokens_set(
    ptr: *mut KuiCtx,
    colors: *const KuiColorToken,
    color_count: usize,
    lengths: *const KuiLengthToken,
    length_count: usize,
) {
    guard((), || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return;
        };
        let mut t = kui_core::Tokens::new();
        if !colors.is_null() {
            for tok in unsafe { std::slice::from_raw_parts(colors, color_count) } {
                t = t.color_themed(
                    kstr(tok.name).into_owned(),
                    Color::hex(tok.light),
                    Color::hex(tok.dark),
                );
            }
        }
        if !lengths.is_null() {
            for tok in unsafe { std::slice::from_raw_parts(lengths, length_count) } {
                t = t.length(kstr(tok.name).into_owned(), tok.value);
            }
        }
        c.core().set_tokens(t);
    })
}

/// A colour token by name, resolved for this frame's appearance — the
/// running origin's table over the host's, and a theme role's name
/// (`surface`) answers with the role. `0xRRGGBBAA` through `out`; false
/// for a name nothing declared or one that is a length, which also raises
/// `unknown-token` once per name.
#[unsafe(no_mangle)]
pub extern "C" fn kui_token_color(ptr: *mut KuiCtx, name: KuiStr, out: *mut u32) -> bool {
    guard(false, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return false;
        };
        let Some(out) = (unsafe { out.as_mut() }) else {
            return false;
        };
        let name = kstr(name);
        match c.core().token_lookup().color(&name) {
            Ok(col) => {
                *out = col.to_hex();
                true
            }
            Err(e) => {
                c.core().warn_unknown_token(&e);
                false
            }
        }
    })
}

/// A length token by name, in logical px; a metrics role's name
/// (`radius`) answers with the metric. False and `unknown-token` as for
/// `kui_token_color`.
#[unsafe(no_mangle)]
pub extern "C" fn kui_token_length(ptr: *mut KuiCtx, name: KuiStr, out: *mut f32) -> bool {
    guard(false, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return false;
        };
        let Some(out) = (unsafe { out.as_mut() }) else {
            return false;
        };
        let name = kstr(name);
        match c.core().token_lookup().length(&name) {
            Ok(v) => {
                *out = v;
                true
            }
            Err(e) => {
                c.core().warn_unknown_token(&e);
                false
            }
        }
    })
}

/// The sizes the stock widgets are built from (backlog T2): the palette's
/// other axis. `KuiMetrics m = KUI_METRICS_INIT; kui_metrics(ctx, &m);`
/// then `spec.radius = m.radius` makes a control of your own agree with
/// the stock ones. Logical px, before the scale factor. False for a bad
/// context, a NULL `out`, or a reservation smaller than the ABI-1 layout.
#[unsafe(no_mangle)]
pub extern "C" fn kui_metrics(ptr: *mut KuiCtx, out: *mut KuiMetrics) -> bool {
    guard(false, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return false;
        };
        let m = KuiMetrics::of(c.core().metrics());
        write_out(out, m)
    })
}

/// Makes these the frame's metrics: every stock widget from the next node
/// on is built from them. NULL restores the stock set. Read one with
/// `kui_metrics` and change the fields you mean to change rather than
/// zeroing a fresh struct — a zeroed metric is zero, not "leave it".
/// Density is the host's to choose; nothing in the OS is followed.
#[unsafe(no_mangle)]
pub extern "C" fn kui_metrics_set(ptr: *mut KuiCtx, metrics: *const KuiMetrics) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            match unsafe { metrics.as_ref() } {
                None => c.core().set_metrics(kui_core::Metrics::default()),
                Some(m) => c.core().set_metrics(m.to_core()),
            }
        }
    })
}

/// A `KuiTheme` the host filled, as a core [`kui_core::Theme`]. An
/// appearance code past the end is `unknown`, the way every other code is.
fn theme_of(t: &KuiTheme) -> kui_core::Theme {
    kui_core::Theme {
        appearance: Appearance::from_code(t.appearance).unwrap_or_default(),
        disabled_opacity: t.disabled_opacity,
        bg: Color::hex(t.bg),
        surface: Color::hex(t.surface),
        raised: Color::hex(t.raised),
        sunken: Color::hex(t.sunken),
        border: Color::hex(t.border),
        border_strong: Color::hex(t.border_strong),
        fg: Color::hex(t.fg),
        muted: Color::hex(t.muted),
        faint: Color::hex(t.faint),
        accent: Color::hex(t.accent),
        accent_hover: Color::hex(t.accent_hover),
        accent_pressed: Color::hex(t.accent_pressed),
        on_accent: Color::hex(t.on_accent),
        accent_soft: Color::hex(t.accent_soft),
        selection: Color::hex(t.selection),
        focus_ring: Color::hex(t.focus_ring),
        hover: Color::hex(t.hover),
        pressed: Color::hex(t.pressed),
        success: Color::hex(t.success),
        warning: Color::hex(t.warning),
        danger: Color::hex(t.danger),
        scrollbar: Color::hex(t.scrollbar),
        scrollbar_active: Color::hex(t.scrollbar_active),
    }
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

/// Byte budget for the shaped-text cache: past it the least recently drawn
/// entries are evicted at the start of the next frame, never what the last
/// frame drew. Default 64 MB (`DEFAULT_TEXT_CACHE_BYTES`).
#[unsafe(no_mangle)]
pub extern "C" fn kui_set_text_cache_budget(ptr: *mut KuiCtx, bytes: usize) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().set_text_cache_budget(bytes);
        }
    });
}

/// What the shaped-text cache holds, in the estimated bytes the budget is
/// charged against.
#[unsafe(no_mangle)]
pub extern "C" fn kui_text_cache_bytes(ptr: *mut KuiCtx) -> usize {
    guard(0, || {
        unsafe { ctx(ptr) }.map_or(0, |c| c.core().text_cache_bytes())
    })
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

/// Turns the core's devtools panel on or off (`docs/adr/0024`): the event
/// stream, the runtime's facts and the tree, drawn by the core beside the
/// host's tree in the main window — or where `kui_set_devtools_dock` says
/// — with its controls and its `Ctrl+Shift+<letter>` chords acted on inside
/// `kui_input`, so nothing of it reaches the host's events. `KUI_DEVTOOLS=1`
/// in the environment is the same call made by nobody, for a window
/// `kui_run` opens; a headless context never reads it.
#[unsafe(no_mangle)]
pub extern "C" fn kui_set_devtools(ptr: *mut KuiCtx, on: bool) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().set_devtools(on);
        }
    });
}

/// Where the devtools panel sits: `"left"`, `"right"`, `"bottom"`, `"window"`
/// (one of its own, named `kui-devtools`, opened through the ordinary
/// `KUI_CMD_OPEN`; the host builds nothing into it) or `"off"` (hidden, the
/// chords still live); `"side"` is the right. Returns false for any other
/// word.
#[unsafe(no_mangle)]
pub extern "C" fn kui_set_devtools_dock(ptr: *mut KuiCtx, dock: KuiStr) -> bool {
    guard(false, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return false;
        };
        let Some(dock) = kui_core::DevtoolsDock::parse(&kstr(dock)) else {
            return false;
        };
        c.core().set_devtools_dock(dock);
        true
    })
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
// the list; the `cbuild` tool compiles it against the header, in CI too.

/// The two halves of ADR 0004's named gap: a version a host can compare,
/// and a size on every struct the library writes into the host's memory.
#[cfg(test)]
mod abi_handshake;

#[cfg(test)]
mod abi_parity;
