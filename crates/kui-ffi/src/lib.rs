//! C API for kui: a cdylib plus `include/kui.h`, and [`CExtension`] for loading a C plugin into a Rust host.
//!
//! kui splits a UI into a model that lays out and paints into a display
//! list (`kui-core`), a renderer (`kui-wgpu`) and a windowed runner
//! (`kui-native`). This crate puts the model, and optionally the runner,
//! behind a flat C ABI: every `pub extern "C" fn` here is a `kui_*`
//! symbol declared in `include/kui.h`, and the `Kui*` structs are the
//! `repr(C)` mirrors the header declares field for field.
//!
//! Two kinds of program use it:
//!
//! - A **C, C++ or other-language host** links the cdylib (`libkui_ffi`)
//!   and includes `kui.h`. It either hands a window to `kui_run` and
//!   builds its view in a callback, or owns its own event loop and
//!   renderer: it feeds input with `kui_input_*`, builds a frame between
//!   [`kui_frame_begin`] and [`kui_frame_finish`], polls
//!   [`kui_poll_event`] and draws [`kui_draw_data`].
//! - A **Rust host** running `kui-native` loads a C shared library as a
//!   guest extension through [`CExtension`]: the plugin draws into a slot
//!   the host declares and gets its own events back.
//!
//! The C programs under `examples/c/` in the repository are working
//! references for every call below.
//!
//! # Conventions
//!
//! - **Strings** cross as [`KuiStr`], a `(ptr, len)` pair of UTF-8 bytes
//!   that is never NUL-terminated (`KUI_STR("literal")` in C). Invalid
//!   UTF-8 is replaced. A string the library hands back is borrowed until
//!   the next call of the same function on that context.
//! - **Values.** A [`KuiValue`] from a `kui_value_*` constructor is yours
//!   until you pass it to a function documented as consuming it (an
//!   `on_click` payload, the value given to [`kui_value_map_set`]); free
//!   anything else with [`kui_value_free`]. The payload on a polled event
//!   is borrowed until the next [`kui_poll_event`].
//! - **Panics never cross.** Every entry point catches panics and returns
//!   its failure value instead (`false`, `0` or NULL). A NULL context is
//!   answered the same way.
//! - **ABI handshake.** Call [`kui_abi_version`] first and compare it for
//!   equality with the header's `KUI_ABI_VERSION` (see
//!   [`KUI_ABI_VERSION`]). Structs the library writes into your memory
//!   lead with a `size` you set from `sizeof` (the `KUI_*_INIT`
//!   initializers do), and the library writes no further than that.
//! - **Coordinates** are logical pixels; draw data comes back in physical
//!   pixels at the frame's `scale`.
//!
//! # A host's frame loop in C
//!
//! The shape without `kui_run`: the calls a host makes around a window and
//! renderer of its own (the same calls work headless, which is how the C
//! examples test themselves).
//!
//! ```c
//! #include "kui.h"
//!
//! if (kui_abi_version() != KUI_ABI_VERSION) return 1;
//! KuiCtx *ctx = kui_ctx_new();
//! long long count = 0;
//!
//! for (;;) {
//!     /* Input from the windowing library, in logical pixels. */
//!     kui_input_cursor(ctx, mouse_x, mouse_y);
//!     if (clicked) { kui_input_mouse(ctx, true, 1); kui_input_mouse(ctx, false, 1); }
//!
//!     /* What the UI emitted; a payload is borrowed until the next poll. */
//!     KuiEvent ev = KUI_EVENT_INIT;
//!     while (kui_poll_event(ctx, &ev)) {
//!         const KuiValue *kind = kui_value_get(ev.payload, KUI_STR("kind"));
//!         KuiStr s;
//!         if (kind && kui_value_as_str(kind, &s) && kui_str_eq(s, "inc")) count++;
//!     }
//!
//!     /* Build the frame from scratch. */
//!     kui_set_time(ctx, now_seconds());
//!     kui_frame_begin(ctx, width, height, scale);
//!     KuiTheme t = KUI_THEME_INIT;
//!     kui_theme(ctx, &t);
//!     KuiSpec root = {.width = {KUI_GROW, 1}, .height = {KUI_GROW, 1},
//!                     .main_align = KUI_CENTER, .cross_align = KUI_CENTER, .bg = t.bg};
//!     kui_root(ctx, &root);
//!     char buf[32];
//!     snprintf(buf, sizeof buf, "%lld", count);
//!     KuiTextStyle big = {.size = 56};
//!     kui_text(ctx, KUI_STR(buf), &big);
//!     KuiValue *inc = kui_value_map();
//!     kui_value_map_set(inc, KUI_STR("kind"), kui_value_str(KUI_STR("inc")));
//!     kui_button(ctx, KUI_STR("+1"), inc); /* consumes inc */
//!     kui_frame_finish(ctx);
//!
//!     /* Draw it: one quad list, physical pixels, plus the glyph atlas to mirror. */
//!     KuiDrawData dd = KUI_DRAW_DATA_INIT;
//!     if (kui_draw_data(ctx, &dd)) {
//!         if (dd.atlas_dirty) upload_atlas(dd.atlas_pixels, dd.atlas_size);
//!         draw_quads(dd.quads, dd.quad_count, dd.clips);
//!     }
//!     if (!kui_animating(ctx)) wait_for_input();
//! }
//! kui_ctx_free(ctx);
//! ```
//!
//! With the `runner` feature, `kui_run(title, view, on_event, user)` does
//! all of this around a window of its own and calls `view` once per frame
//! with a context to build into; `examples/c/apps/counter.c` is that
//! program.
//!
//! # A C extension in a Rust app
//!
//! A plugin is a shared library exporting `kui_ext_abi` and `kui_ext_view`
//! (and optionally `kui_ext_name`, `kui_ext_init`, `kui_ext_slots`,
//! `kui_ext_on_event`, `kui_ext_free`); `examples/c/features/slots/panel.c`
//! is one. The host loads it and declares where it draws:
//!
//! ```rust,no_run
//! use kui_ffi::CExtension;
//! use kui_native::{App, NodeSpec, Ui};
//!
//! struct Host;
//!
//! impl App for Host {
//!     fn view(&mut self, ui: &mut Ui<'_>) {
//!         ui.open(NodeSpec::row().fill());
//!         // The plugin fills this position: `todos` is the namespace the
//!         // host loaded it under, `panel` a slot the plugin lists.
//!         ui.slot("todos/panel");
//!         ui.close();
//!     }
//! }
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // SAFETY: the plugin's code runs in this process. Loading it is
//!     // trusting it as much as linking it would be.
//!     let ext = unsafe { CExtension::open("target/debug/panel.so")? };
//!     kui_native::app("host").extension_as("todos", ext).run(Host)
//! }
//! ```
//!
//! # Where to look
//!
//! Everything is exported flat at the crate root. By job:
//!
//! - Context: [`kui_ctx_new`], [`kui_ctx_free`], [`KuiCtx`].
//! - Building a frame: [`kui_frame_begin`], [`kui_root`], [`kui_open`],
//!   [`kui_open_keyed`], [`kui_text`], [`kui_close`], [`kui_frame_finish`];
//!   [`KuiSpec`] and [`KuiTextStyle`] are what a node is built from.
//! - Input and events: [`kui_input_cursor`], [`kui_input_mouse`],
//!   [`kui_input_press`], [`kui_input_text`], [`kui_poll_event`],
//!   [`KuiEvent`].
//! - Values: [`kui_value_map`], [`kui_value_str`], [`kui_value_get`],
//!   [`kui_value_as_str`], [`KuiValue`].
//! - Drawing: [`kui_draw_data`], [`KuiDrawData`], [`KuiQuad`],
//!   [`kui_set_subpixel_text`].
//! - Widgets: [`kui_button`], [`kui_text_input`], [`kui_checkbox`],
//!   [`kui_slider`], [`kui_select`].
//! - Theme and metrics: [`kui_env_set_system`], [`kui_theme`],
//!   [`KuiTheme`], [`kui_metrics`].
//! - Windows: [`kui_window_declare`], [`kui_take_window_command`],
//!   [`KuiWindowCommand`].
//! - Accessibility: [`kui_access_tree`], [`KuiAccessNode`],
//!   [`kui_input_access`].
//! - Extensions: [`CExtension`], [`kui_ctx_add_extension`], [`kui_slot`],
//!   [`kui_reply`].
//! - Diagnostics: [`kui_set_diagnostics`], [`kui_take_warnings`],
//!   [`kui_set_devtools`].
//! - The windowed runner (feature `runner`): `kui_run`, `kui_run_with`,
//!   [`KuiRunConfig`].
//!
//! # Features
//!
//! - `runner` (on by default): `kui_run` and `kui_run_with`, the windowed
//!   runner from `kui-native`. A host that owns its own window and
//!   renderer builds with `--no-default-features` and ships less than
//!   half the library.
//!
//! The book: <https://kui-book.qxuken.dev>. Repository:
//! <https://github.com/qxuken/kui> (design records live under `docs/adr`
//! there).

// Safe extern fns taking raw pointers is the point of this layer: every
// entry point null-checks and catches panics instead of being `unsafe`.
#![allow(clippy::not_unsafe_ptr_arg_deref)]

// The other direction: a C shared library as a guest inside a host that
// already owns the frame.
mod ext;
pub use ext::CExtension;

#[macro_use]
mod abi;
mod access;
mod convert;
mod dialogs;
mod focus;
mod frame;
mod input;
mod menu;
mod resources;
#[cfg(feature = "runner")]
mod run;
mod scrolling;
mod select;
mod size;
mod slots;
mod types;
mod value;
mod widgets;
mod windows;

pub use abi::*;
pub use access::*;
pub use dialogs::*;
pub use focus::*;
pub use frame::*;
pub use input::*;
pub use menu::*;
pub use resources::*;
#[cfg(feature = "runner")]
pub use run::*;
pub use scrolling::*;
pub use select::*;
pub use size::*;
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
    MouseButton, NodeSpec, OptionAsAlt, Rect, Size, Sizing, Span, SystemEnv, TextStyle, UiEvent,
    Value, Vec2, WindowButton, WindowCommand, WindowConfig, WindowId, WindowKind,
};

// ---------------------------------------------------------------------------
// Context lifecycle

/// Creates a standalone context: a core of its own plus an event queue.
///
/// Free it with [`kui_ctx_free`]. Diagnostics start off; turn them on with
/// [`kui_set_diagnostics`] in a development build. Returns NULL only if
/// the core cannot be created.
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
            draws_current: false,
            image_pixels: None,
            last_warnings: Vec::new(),
            pending_warnings: Vec::new(),
            last_access: Default::default(),
            last_runs: Vec::new(),
            last_announcements: Vec::new(),
            window_commands: VecDeque::new(),
            menu_actions: VecDeque::new(),
            menu_text: String::new(),
            row_text: String::new(),
            menu_accel: String::new(),
            copy_text: String::new(),
            menu_html: String::new(),
            devtools_key: String::new(),
            file_request: None,
            file_filter_text: String::new(),
            devtools_tab: String::new(),
            selection_text: String::new(),
            selection_html: String::new(),
            font_families: Vec::new(),
            system_fonts: Vec::new(),
            nodes: None,
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

/// Frees a context from [`kui_ctx_new`], its queued events, every string
/// it lent out and every extension it loaded. NULL is a no-op.
#[unsafe(no_mangle)]
pub extern "C" fn kui_ctx_free(ptr: *mut KuiCtx) {
    if !ptr.is_null() {
        drop(unsafe { Box::from_raw(ptr) });
    }
}

// ---------------------------------------------------------------------------
// Host environment

/// Window facts for views to read: the display's refresh rate
/// (`refresh_hz <= 0` is unknown) and whether the window has keyboard
/// focus.
///
/// Sticky across frames; set it on change or every frame. A window that
/// lost focus releases every key a sink was holding, and those releases
/// are polled like any event.
#[unsafe(no_mangle)]
pub extern "C" fn kui_env_set(ptr: *mut KuiCtx, refresh_hz: f32, focused: bool) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().env.refresh_hz = (refresh_hz > 0.0).then_some(refresh_hz);
            c.core().set_focused(focused);
        }
    });
}

/// The OS's settings, for views and the theme to read: `appearance` is a
/// `KUI_APPEARANCE_*`, `motion` a `KUI_MOTION_*` (0 is unknown for both),
/// `accent` the accent colour as `0xRRGGBBAA` (0 is unknown) and `locale`
/// a BCP-47 tag (empty is unknown; one over 31 bytes or not ASCII reads
/// back as unknown).
///
/// Push them at startup and on the OS's change notification; they stick
/// across frames. The palette [`kui_theme`] reports is re-derived at once.
/// An out-of-range code is ignored. On a context handed to `kui_run_with`
/// these become the window's pin over the OS's own reading; a zero field
/// keeps following the OS.
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
            // `kui_theme` back before its next frame sees the answer.
            c.core().set_system(sys);
        }
    });
}

/// Whether assistive technology is listening, for views to read: a
/// `KUI_ASSISTIVE_*` (0 is unknown, which is what a host with no
/// accessibility bridge reports by never calling this).
///
/// A host bridging the platform's accessibility API pushes `LISTENING`
/// when a client first asks for the tree and `NONE` if the platform says
/// the client left. An out-of-range code is ignored.
#[unsafe(no_mangle)]
pub extern "C" fn kui_env_set_assistive(ptr: *mut KuiCtx, assistive: u32) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().env.system.assistive = Assistive::from_code(assistive).unwrap_or_default();
        }
    });
}

/// What the host's audio output is doing, for views to read: `device` a
/// `KUI_AUDIO_DEVICE_*` (0 is closed, which a host with no device reports
/// by never calling this) and `live` the number of playbacks started or
/// waiting on the open. A fact, not a command: nothing here closes the
/// device. An out-of-range code is ignored.
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

/// This window's palette as of the current or last frame: one
/// `0xRRGGBBAA` per role, derived from what [`kui_env_set_system`] reported
/// unless the host pinned something with [`kui_theme_set_accent`] or
/// [`kui_theme_set`].
///
/// The stock widgets, the focus ring, the scrollbars and any text with a
/// zero `color` already follow it. Read it for paint of your own:
/// `KuiTheme t = KUI_THEME_INIT; kui_theme(ctx, &t);` then
/// `spec.bg = t.surface`.
///
/// False for a bad context, a NULL `out`, or a `size` below the first
/// ABI's layout.
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

/// Keeps following the OS's light or dark base but paints `accent`
/// (`0xRRGGBBAA`) instead of the OS's accent; zero goes back to the OS's.
///
/// Everything derived from the accent moves with it: a button's hover and
/// pressed shades, the label on it, the selection tint and the focus ring.
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

/// Pins the whole palette to exactly these colours, following neither the
/// OS's appearance nor its accent; NULL goes back to deriving both.
///
/// Every field is read (`size` is ignored), so start from [`kui_theme`]
/// and change the roles you mean to change: a zeroed role is transparent,
/// not "leave it".
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

/// Declares the named colours and lengths of the calling origin: the
/// host's outside a plugin's view, the plugin's own inside `kui_ext_view`.
///
/// Replaces the table whole, so an app whose lengths follow a viewport
/// tier declares again on a resize. A name a theme or metrics role owns is
/// dropped with a `reserved-token` warning. Either array may be NULL with
/// a zero count. A C spec carries plain numbers, so read a token back with
/// [`kui_token_color`] or [`kui_token_length`] and write the value; the
/// table gives the name to the devtools inspector, to guests reading the
/// host's vocabulary, and to a Lua panel referencing `"$name"`.
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

/// Adds derived colour tokens to the calling origin's table, after
/// [`kui_tokens_set`]: each a name, the colour token or theme role it
/// derives from, and a chain of ops applied in order.
///
/// The ops: `KUI_OP_LIFT` and `KUI_OP_DARKEN` move toward white or black
/// by `t`, `KUI_OP_RAISE` toward the front of the base in effect,
/// `KUI_OP_ALPHA` sets the alpha, `KUI_OP_MIX` mixes toward the token
/// `other` names, and `KUI_OP_READABLE` moves toward black or white until
/// the contrast ratio `t` against `other` is met. A source that is neither
/// a token declared before it nor a role drops that token with an
/// `unknown-token` warning. Returns false, adding nothing, for a malformed
/// op: one past `KUI_OP_READABLE`, or `other` given to a verb that takes
/// none or missing from one that does.
#[unsafe(no_mangle)]
pub extern "C" fn kui_tokens_derive(
    ptr: *mut KuiCtx,
    derived: *const KuiDerivedToken,
    count: usize,
) -> bool {
    guard(false, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return false;
        };
        if derived.is_null() || count == 0 {
            return true;
        }
        let mut t = c.core().tokens().cloned().unwrap_or_default();
        for d in unsafe { std::slice::from_raw_parts(derived, count) } {
            let ops = if d.ops.is_null() {
                &[][..]
            } else {
                unsafe { std::slice::from_raw_parts(d.ops, d.op_count) }
            };
            let mut chain = Vec::with_capacity(ops.len());
            for op in ops {
                let Some(verb) = kui_core::ColorOp::VERBS.get(op.op as usize) else {
                    return false;
                };
                let other = kstr(op.other);
                let other = (!other.is_empty()).then_some(&*other);
                let Some(step) = kui_core::ColorOp::parse(verb, other, op.t) else {
                    return false;
                };
                chain.push(step);
            }
            t = t.derive(kstr(d.name).into_owned(), &kstr(d.from), chain);
        }
        c.core().set_tokens(t);
        true
    })
}

/// A colour token by name, resolved for this frame's appearance, as
/// `0xRRGGBBAA` through `out`. The calling origin's table is tried first,
/// then the host's; a theme role's name (`surface`) answers with the role.
/// False for a name nothing declared or one that is a length, which also
/// raises `unknown-token` once per name.
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
/// [`kui_token_color`].
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

/// The sizes the stock widgets are built from, in logical px.
/// `KuiMetrics m = KUI_METRICS_INIT; kui_metrics(ctx, &m);` then
/// `spec.radius = m.radius` makes a control of your own agree with the
/// stock ones. False for a bad context, a NULL `out`, or a `size` below
/// the first ABI's layout.
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

/// Makes these the metrics every stock widget from the next node on is
/// built from; NULL restores the stock set. Start from [`kui_metrics`] and
/// change the fields you mean to change: a zeroed metric is zero, not
/// "leave it". Nothing in the OS is followed; density is the host's call.
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

/// The frame clock for transitions, in monotonic seconds from any origin.
/// Set it before each [`kui_frame_begin`]; a host that never does sees
/// transitions snap to their targets.
#[unsafe(no_mangle)]
pub extern "C" fn kui_set_time(ptr: *mut KuiCtx, now_secs: f64) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().set_time(now_secs);
        }
    });
}

/// The frame clock in seconds, as `kui_set_time` last set it (0 before
/// any): what a view times its own deadlines by, so that moving the clock
/// in a test moves them with the transitions (backlog F134).
#[unsafe(no_mangle)]
pub extern "C" fn kui_now(ptr: *mut KuiCtx) -> f64 {
    guard(0.0, || unsafe { ctx(ptr) }.map_or(0.0, |c| c.core().now()))
}

/// True when the last frame left a transition mid-flight: draw another
/// frame without waiting for input.
#[unsafe(no_mangle)]
pub extern "C" fn kui_animating(ptr: *mut KuiCtx) -> bool {
    guard(false, || {
        unsafe { ctx(ptr) }.is_some_and(|c| c.core().animating())
    })
}

/// Why the last frame wants another, as `KUI_OWED_*` bits:
/// [`kui_animating`] taken apart by kind. A host draws another frame for
/// any of them; a test masks `KUI_OWED_CYCLE` off to wait for transitions
/// to settle under a keyframe cycle that never will.
#[unsafe(no_mangle)]
pub extern "C" fn kui_owed(ptr: *mut KuiCtx) -> u32 {
    guard(0, || {
        unsafe { ctx(ptr) }.map_or(0, |c| {
            let o = c.core().owed();
            (o.transition as u32 * KUI_OWED_TRANSITION)
                | (o.cycle as u32 * KUI_OWED_CYCLE)
                | (o.depart as u32 * KUI_OWED_DEPART)
                | (o.requested as u32 * KUI_OWED_REQUESTED)
                | (o.autoscroll as u32 * KUI_OWED_AUTOSCROLL)
                | (o.scroll as u32 * KUI_OWED_SCROLL)
        })
    })
}

/// Turns the trace of why frames run on or off: the digest
/// [`kui_frame_unchanged`] compares. [`kui_frame_cause`] is kept either
/// way.
#[unsafe(no_mangle)]
pub extern "C" fn kui_set_frame_trace(ptr: *mut KuiCtx, on: bool) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().set_frame_trace(on);
        }
    })
}

/// The `KUI_FRAME_CAUSE_*` bits of the frame being built, or between
/// frames the last one's.
#[unsafe(no_mangle)]
pub extern "C" fn kui_frame_cause(ptr: *mut KuiCtx) -> u32 {
    guard(0, || {
        unsafe { ctx(ptr) }.map_or(0, |c| c.core().frame_cause().bits())
    })
}

/// Adds `KUI_FRAME_CAUSE_*` bits to the next frame's reasons: the host's
/// own (a wake, a resize, a blink) beside the input the `kui_input_*`
/// calls record.
#[unsafe(no_mangle)]
pub extern "C" fn kui_note_frame_cause(ptr: *mut KuiCtx, cause: u32) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core()
                .note_frame_cause(kui_core::FrameCause::from_bits(cause));
        }
    })
}

/// 1 when the last finished frame drew exactly what the one before drew,
/// 0 when not, -1 when untraced ([`kui_set_frame_trace`]) or on the first
/// traced frame.
#[unsafe(no_mangle)]
pub extern "C" fn kui_frame_unchanged(ptr: *mut KuiCtx) -> i32 {
    guard(-1, || {
        unsafe { ctx(ptr) }.map_or(-1, |c| match c.core().frame_unchanged() {
            Some(same) => same as i32,
            None => -1,
        })
    })
}

/// Rasterizes outline glyphs as LCD subpixel coverage
/// (`KUI_QUAD_GLYPH_SUBPIXEL` quads, the atlas's RGB being per-channel
/// coverage) instead of alpha masks. Only for a renderer that blends per
/// channel (dual-source blending); flipping it re-rasterizes every glyph.
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
/// misconfigurations it noticed while finishing frames, each once) into
/// `out`, up to `cap` (the rest wait for the next call), and returns the
/// count. The strings stay valid
/// until the next call on this context. A standalone context starts with
/// the checks off ([`kui_set_diagnostics`] turns them on); `kui_run`
/// prints them to stderr itself in debug builds.
#[unsafe(no_mangle)]
pub extern "C" fn kui_take_warnings(ptr: *mut KuiCtx, out: *mut KuiWarning, cap: usize) -> usize {
    guard(0, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return 0;
        };
        if out.is_null() || cap == 0 {
            return 0;
        }
        // Drained from the core into the context's queue, and handed out
        // from there `cap` at a time: the rest wait, as kui.h says, where
        // they were dropped.
        let raised = c.core().take_warnings();
        c.pending_warnings.extend(raised);
        let n = c.pending_warnings.len().min(cap);
        c.last_warnings = c.pending_warnings.drain(..n).collect();
        for (i, w) in c.last_warnings.iter().enumerate() {
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

/// Turns the diagnostic checks behind [`kui_take_warnings`] on or off. Off
/// by default for a standalone context; a development build opts in.
#[unsafe(no_mangle)]
pub extern "C" fn kui_set_diagnostics(ptr: *mut KuiCtx, on: bool) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().set_diagnostics(on);
        }
    });
}

/// Turns the core's devtools panel on or off: the event stream, the
/// runtime's facts and the tree, drawn by the core beside the host's tree
/// (where [`kui_set_devtools_dock`] says). Its controls and its
/// `Ctrl+Shift+<letter>` chords are handled inside the `kui_input_*`
/// calls, so nothing of it reaches the host's events. `KUI_DEVTOOLS=1` in
/// the environment makes the same call for a window `kui_run` opens; a
/// headless context never reads it.
#[unsafe(no_mangle)]
pub extern "C" fn kui_set_devtools(ptr: *mut KuiCtx, on: bool) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().set_devtools(on);
        }
    });
}

/// Where the devtools panel sits: `"left"`, `"right"`, `"bottom"`,
/// `"window"` (one of its own, named `kui-devtools`, opened through an
/// ordinary `KUI_CMD_OPEN` that the host builds nothing into) or `"off"`
/// (hidden, chords still live); `"side"` means the right. Returns false
/// for any other word.
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

/// Whether the devtools panel is on.
#[unsafe(no_mangle)]
pub extern "C" fn kui_devtools(ptr: *mut KuiCtx) -> bool {
    guard(false, || {
        unsafe { ctx(ptr) }.is_some_and(|c| c.core().devtools())
    })
}

/// Where the devtools panel sits, as a word [`kui_set_devtools_dock`]
/// takes (`"right"` for the side); the string is static. False on a bad
/// context.
#[unsafe(no_mangle)]
pub extern "C" fn kui_devtools_dock(ptr: *mut KuiCtx, out: *mut KuiStr) -> bool {
    guard(false, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return false;
        };
        let name = c.core().devtools_dock().name();
        if let Some(out) = unsafe { out.as_mut() } {
            *out = KuiStr {
                ptr: name.as_ptr(),
                len: name.len(),
            };
        }
        true
    })
}

/// Where the last frame laid the host out in its window, in logical px:
/// the whole window with the devtools panel off or in its own window, the
/// pane beside the dock otherwise, and zeros before the first frame.
/// Scaled by the frame's scale it separates the host's quads from the
/// dock's in [`kui_draw_data`], except the root's background, which also
/// fills the window beneath the pane. False on a bad context, a NULL
/// `out` or a short `size`.
#[unsafe(no_mangle)]
pub extern "C" fn kui_host_rect(ptr: *mut KuiCtx, out: *mut KuiLayoutRect) -> bool {
    guard(false, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return false;
        };
        let r = c.core().host_rect();
        write_out(
            out,
            KuiLayoutRect {
                x: r.x,
                y: r.y,
                w: r.w,
                h: r.h,
                ..Default::default()
            },
        )
    })
}

/// Seeds the devtools panel's theme override, which its `T` and `A`
/// chords cycle from: `base` is `"light"`, `"dark"` or empty for the
/// app's own; `accent` a `0xRRGGBBAA` colour, or 0 for none. False for
/// any other base word.
#[unsafe(no_mangle)]
pub extern "C" fn kui_set_devtools_theme(ptr: *mut KuiCtx, base: KuiStr, accent: u32) -> bool {
    guard(false, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return false;
        };
        let base = match kstr(base).as_ref() {
            "" => None,
            "light" => Some(kui_core::Appearance::Light),
            "dark" => Some(kui_core::Appearance::Dark),
            _ => return false,
        };
        let accent = (accent != 0).then(|| color_of(accent));
        c.core().set_devtools_theme(base, accent);
        true
    })
}

/// Respells the chord that moves the keyboard into the devtools panel and
/// back out (and brings a hidden panel back) from its default
/// `"ctrl+shift+i"`: `"f12"`, `"mod+shift+d"` (`mod` is Command on macOS,
/// Control elsewhere), any spelling a [`KuiMenuItem`]'s `accel` takes. The
/// panel's other chords stay `Ctrl+Shift+<letter>`. False for a spelling
/// kui cannot parse, which leaves the chord as it was.
#[unsafe(no_mangle)]
pub extern "C" fn kui_set_devtools_key(ptr: *mut KuiCtx, key: KuiStr) -> bool {
    guard(false, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return false;
        };
        let Some(accel) = kui_core::Accel::parse(&kstr(key)) else {
            return false;
        };
        c.core().set_devtools_key(accel);
        true
    })
}

/// The chord [`kui_set_devtools_key`] set, or the default, in its portable
/// spelling (`"ctrl+shift+i"`, `"f12"`, `"super+alt+d"`); borrowed until
/// the next call. False on a bad context.
#[unsafe(no_mangle)]
pub extern "C" fn kui_devtools_key(ptr: *mut KuiCtx, out: *mut KuiStr) -> bool {
    guard(false, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return false;
        };
        c.devtools_key = c.core().devtools_key().spelling();
        if let Some(out) = unsafe { out.as_mut() } {
            *out = KuiStr {
                ptr: c.devtools_key.as_ptr(),
                len: c.devtools_key.len(),
            };
        }
        true
    })
}

/// Declares a devtools tab an extension fills: `name` is the tab's
/// identity, `label` what the strip shows, `slot` the full
/// `namespace/slot` the extension names. While the tab is on show the
/// panel declares that slot in the tab's body; otherwise the extension is
/// not asked, and no `unknown-slot` is raised. Call it every frame, panel
/// on or off. False for a name already declared this frame
/// (`duplicate-tab`) or outside a frame.
#[unsafe(no_mangle)]
pub extern "C" fn kui_devtools_tab(
    ptr: *mut KuiCtx,
    name: KuiStr,
    label: KuiStr,
    slot: KuiStr,
) -> bool {
    guard(false, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return false;
        };
        c.core()
            .devtools_tab_declare(&kstr(name), &kstr(label), Some(&kstr(slot)))
    })
}

/// Declares a devtools tab the host draws itself, and opens its content
/// node only while the tab is on show: true means the node is open, so
/// build inside it and [`kui_close`]; false means the tab was declared and
/// nothing opened, so skip the body and do not close. What the host builds
/// keeps its own keys and events, painted as a layer over the panel's tab
/// body and clipped to it:
/// `if (kui_devtools_tab_open(ctx, KUI_STR("syntax"), KUI_STR("Tree-sitter"))) { ...; kui_close(ctx); }`.
#[unsafe(no_mangle)]
pub extern "C" fn kui_devtools_tab_open(ptr: *mut KuiCtx, name: KuiStr, label: KuiStr) -> bool {
    guard(false, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return false;
        };
        let name = kstr(name);
        let core = c.core();
        if !core.devtools_tab_declare(&name, &kstr(label), None) || !core.devtools_tab_shown(&name)
        {
            return false;
        }
        core.devtools_tab_open(&name);
        true
    })
}

/// The node the devtools tree tab has selected, as a key, or 0 for none:
/// what an inspector in a declared tab reads.
#[unsafe(no_mangle)]
pub extern "C" fn kui_devtools_selected(ptr: *mut KuiCtx) -> u64 {
    guard(0, || {
        unsafe { ctx(ptr) }.map_or(0, |c| c.core().devtools_selected().map_or(0, |k| k.0))
    })
}

/// The tree row under the pointer, as a key, or 0.
#[unsafe(no_mangle)]
pub extern "C" fn kui_devtools_hovered(ptr: *mut KuiCtx) -> u64 {
    guard(0, || {
        unsafe { ctx(ptr) }.map_or(0, |c| c.core().devtools_hovered().map_or(0, |k| k.0))
    })
}

/// The node the picker is over while picking, as a key, or 0.
#[unsafe(no_mangle)]
pub extern "C" fn kui_devtools_picked(ptr: *mut KuiCtx) -> u64 {
    guard(0, || {
        unsafe { ctx(ptr) }.map_or(0, |c| c.core().devtools_picked().map_or(0, |k| k.0))
    })
}

/// Raises the devtools picker from outside the panel (an inspector asking
/// "which node?") or puts it away. While it is up the node under the
/// pointer is [`kui_devtools_picked`], and a press lands it in
/// [`kui_devtools_selected`]. Raised while a declared tab is on show the
/// pick leaves that tab up; otherwise it shows the tree tab. A hidden
/// panel comes back docked.
#[unsafe(no_mangle)]
pub extern "C" fn kui_set_devtools_pick(ptr: *mut KuiCtx, on: bool) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().set_devtools_pick(on);
        }
    });
}

/// Whether the panel's picker is up.
#[unsafe(no_mangle)]
pub extern "C" fn kui_devtools_picking(ptr: *mut KuiCtx) -> bool {
    guard(false, || {
        unsafe { ctx(ptr) }.is_some_and(|c| c.core().devtools_picking())
    })
}

/// Shows the devtools tab named `name`: one of the panel's own (`facts`,
/// `events`, `tree`) or a declared tab's. A declared name the panel does
/// not list yet is kept and shows once a frame declares it; the return
/// says whether the panel lists it now (false on a bad context too). A
/// hidden panel comes back docked; [`kui_set_devtools`] is still the
/// host's to call. Call it once, not every frame, or it pins the strip
/// against the user's own clicks.
#[unsafe(no_mangle)]
pub extern "C" fn kui_set_devtools_tab(ptr: *mut KuiCtx, name: KuiStr) -> bool {
    guard(false, || {
        unsafe { ctx(ptr) }.is_some_and(|c| c.core().set_devtools_tab(&kstr(name)))
    })
}

/// The devtools tab currently selected, by name, panel on or off;
/// borrowed until the next call. False on a bad context.
#[unsafe(no_mangle)]
pub extern "C" fn kui_devtools_current_tab(ptr: *mut KuiCtx, out: *mut KuiStr) -> bool {
    guard(false, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return false;
        };
        c.devtools_tab = c.core().devtools_current_tab();
        if let Some(out) = unsafe { out.as_mut() } {
            *out = KuiStr {
                ptr: c.devtools_tab.as_ptr(),
                len: c.devtools_tab.len(),
            };
        }
        true
    })
}

/// Selects a node in the devtools tree tab and reveals it there, as the
/// picker does; 0 clears.
#[unsafe(no_mangle)]
pub extern "C" fn kui_set_devtools_selected(ptr: *mut KuiCtx, key: u64) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core()
                .set_devtools_selected((key != 0).then_some(kui_core::Key(key)));
        }
    });
}

/// The key legend the devtools facts tab shows: `count` pairs, the keys
/// in `keys` and what each does in `what`, index for index.
#[unsafe(no_mangle)]
pub extern "C" fn kui_set_devtools_legend(
    ptr: *mut KuiCtx,
    keys: *const KuiStr,
    what: *const KuiStr,
    count: usize,
) {
    guard((), || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return;
        };
        let rows: Vec<(String, String)> = if keys.is_null() || what.is_null() {
            Vec::new()
        } else {
            let keys = unsafe { std::slice::from_raw_parts(keys, count) };
            let what = unsafe { std::slice::from_raw_parts(what, count) };
            keys.iter()
                .zip(what)
                .map(|(k, w)| (kstr(*k).into_owned(), kstr(*w).into_owned()))
                .collect()
        };
        let borrowed: Vec<(&str, &str)> =
            rows.iter().map(|(k, w)| (k.as_str(), w.as_str())).collect();
        c.core().set_devtools_legend(&borrowed);
    });
}

/// Turns the per-frame node snapshot behind [`kui_nodes`] on or off. Off
/// by default: the copy costs a pass over every node each frame.
#[unsafe(no_mangle)]
pub extern "C" fn kui_set_inspect(ptr: *mut KuiCtx, on: bool) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().set_inspect(on);
        }
    });
}

/// The last finished frame's nodes in tree order, as a list of maps, each
/// with `key`, `parent`, `depth`, `kind`, `label`, `rect`, `role`,
/// `text`, `flags`, `layer`, `origin`, `children`, the layout spec and
/// `events` (the node's payloads by handler name). Read it with
/// [`kui_value_at`] and [`kui_value_get`]. Empty until
/// `kui_set_inspect(ctx, true)` and a frame after it. Borrowed until the
/// next call; NULL on a bad context.
#[unsafe(no_mangle)]
pub extern "C" fn kui_nodes(ptr: *mut KuiCtx) -> *const KuiValue {
    guard(std::ptr::null(), || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return std::ptr::null();
        };
        let list = c
            .core()
            .nodes()
            .iter()
            .map(|n| {
                let mut v = n.to_value(kui_core::Handles::INT);
                if let Value::Map(entries) = &mut v {
                    entries.push((
                        "events".into(),
                        Value::Map(
                            n.events
                                .iter()
                                .map(|(name, v)| ((*name).to_string(), v.clone()))
                                .collect(),
                        ),
                    ));
                }
                v
            })
            .collect();
        c.nodes = Some(Box::new(KuiValue(Value::List(list))));
        c.nodes
            .as_deref()
            .map_or(std::ptr::null(), |v| v as *const KuiValue)
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

/// The two halves of the ABI handshake: a version a host can compare, and
/// a size on every struct the library writes into the host's memory.
#[cfg(test)]
mod abi_handshake;

#[cfg(test)]
mod abi_parity;
