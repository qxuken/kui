//! GLFW's and SDL3's keys as kui's (backlog F239): one table, written out
//! as a header-only file for C and a package for Odin per library, so a
//! host that brings its own window forwards keys without mapping them by
//! hand — and the names cannot drift from `KeyCode::named()`, which every
//! row is checked against before anything is written.
//!
//!     cargo run -p kui-devtools --bin keytables              # write the four files
//!     cargo run -p kui-devtools --bin keytables -- --check   # fail if one is stale
//!
//! It writes `crates/kui-ffi/include/kui_glfw.h` and `kui_sdl3.h` (the C
//! pack ships `include/`) and `packages/odin/kui/glfw` and `kui/sdl3`
//! (packages of their own, so importing `kui` links neither library).
//! None of it is in libkui_ffi, which knows no window library.
//!
//! **What a forwarder sends.** The libraries name a key by its *position*,
//! after the US layout — GLFW's `GLFW_KEY_*`, SDL's scancodes — which is
//! kui's `physical`. `code` is what the layout makes of the key:
//! `glfwGetKeyName`, or SDL's keycode, a letter upper-cased under Shift
//! (or Caps Lock); the named keys are their own names. A named key —
//! the arrows, Enter, Tab, Escape, F5 — goes through `kui_input_press`,
//! so it means what it means (a modal dismissed, the ring walked). A key
//! that types — a character, Space — goes through `kui_input_key_down`,
//! the raw half a key sink and the editor's chords (Ctrl+A) read, and
//! what it types arrives once, from the library's own text event
//! (GLFW's char callback, SDL's `SDL_EVENT_TEXT_INPUT`) as
//! `kui_input_text`. Both libraries split typing from keys this way;
//! sending the text with the press as well would type it twice.
//!
//! **The pointer** (backlog F265) is the same four files' second half:
//! the cursor, the window's edge, the buttons and the scroll, sent the
//! way kui_run's runner sends winit's (`crates/kui-native`). It needs
//! state — the last press, for counting clicks, and the last scroll
//! event, for drawing a gesture's edges — which lives in a struct the
//! host owns (`KuiGlfw`, `KuiSdl3`; Odin's `State`), never in statics.
//! The rules both libraries share are one template here
//! ([`C_POINTER`], [`ODIN_POINTER`]), written into each file under its
//! own prefix, so a program including both headers defines each once.
//!
//! **Scroll.** winit says whether a delta is a wheel's notches or a
//! touch surface's pixels; GLFW and SDL3 do not. Both hand lines on
//! every platform but one: on macOS a device with precise deltas (a
//! trackpad, a Magic Mouse) arrives as its points scaled by 0.1 —
//! GLFW's `cocoa_window.m` (`scrollWheel:`, 3.4) and SDL's
//! `SDL_cocoamouse.m` (`Cocoa_HandleMouseWheel`, 3.4) both multiply
//! `scrollingDeltaX/Y` by 0.1 when `hasPreciseScrollingDeltas` — and a
//! wheel's come whole. So on macOS a delta with a fraction, or one in a
//! gesture that had one, is read as a swipe's points; everything else is
//! lines. On Wayland both libraries hand the compositor's pixels over 10
//! for a swipe and a wheel alike, which reads as lines.

use std::fmt::Write as _;
use std::path::Path;

use kui_native::KeyCode;

/// Where on the keyboard a key sits: `KUI_KLOC_*`, Odin's `Key_Location`.
#[derive(Clone, Copy, PartialEq)]
enum Loc {
    Standard,
    Left,
    Right,
    Numpad,
}

impl Loc {
    fn c(self) -> &'static str {
        match self {
            Loc::Standard => "0",
            Loc::Left => "KUI_KLOC_LEFT",
            Loc::Right => "KUI_KLOC_RIGHT",
            Loc::Numpad => "KUI_KLOC_NUMPAD",
        }
    }
    fn odin(self) -> &'static str {
        match self {
            Loc::Standard => ".Standard",
            Loc::Left => ".Left",
            Loc::Right => ".Right",
            Loc::Numpad => ".Numpad",
        }
    }
}

/// One key: kui's name and place, and each library's spelling of the
/// position — `GLFW_KEY_<glfw>` (Odin `glfw.KEY_<glfw>`) and
/// `SDL_SCANCODE_<sdl>` (Odin `.<sdl>`, a digit as `._1`). `None` where
/// the library has no such key.
struct Row {
    kui: &'static str,
    loc: Loc,
    glfw: Option<&'static str>,
    sdl: Option<&'static str>,
}

const fn row(kui: &'static str, glfw: &'static str, sdl: &'static str) -> Row {
    Row {
        kui,
        loc: Loc::Standard,
        glfw: Some(glfw),
        sdl: Some(sdl),
    }
}

fn rows() -> Vec<Row> {
    let mut v = Vec::new();
    // The letters and digits, the same spelling everywhere.
    const LETTERS: [&str; 26] = [
        "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n", "o", "p", "q", "r",
        "s", "t", "u", "v", "w", "x", "y", "z",
    ];
    const UPPER: [&str; 26] = [
        "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R",
        "S", "T", "U", "V", "W", "X", "Y", "Z",
    ];
    for (l, u) in LETTERS.iter().zip(UPPER) {
        v.push(row(l, u, u));
    }
    const DIGITS: [&str; 10] = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];
    for d in DIGITS {
        v.push(row(d, d, d));
    }
    v.extend([
        row("space", "SPACE", "SPACE"),
        row("'", "APOSTROPHE", "APOSTROPHE"),
        row(",", "COMMA", "COMMA"),
        row("-", "MINUS", "MINUS"),
        row(".", "PERIOD", "PERIOD"),
        row("/", "SLASH", "SLASH"),
        row(";", "SEMICOLON", "SEMICOLON"),
        row("=", "EQUAL", "EQUALS"),
        row("[", "LEFT_BRACKET", "LEFTBRACKET"),
        row("\\", "BACKSLASH", "BACKSLASH"),
        row("]", "RIGHT_BRACKET", "RIGHTBRACKET"),
        row("`", "GRAVE_ACCENT", "GRAVE"),
        row("escape", "ESCAPE", "ESCAPE"),
        row("enter", "ENTER", "RETURN"),
        row("tab", "TAB", "TAB"),
        row("backspace", "BACKSPACE", "BACKSPACE"),
        row("insert", "INSERT", "INSERT"),
        row("delete", "DELETE", "DELETE"),
        row("right", "RIGHT", "RIGHT"),
        row("left", "LEFT", "LEFT"),
        row("down", "DOWN", "DOWN"),
        row("up", "UP", "UP"),
        row("pageup", "PAGE_UP", "PAGEUP"),
        row("pagedown", "PAGE_DOWN", "PAGEDOWN"),
        row("home", "HOME", "HOME"),
        row("end", "END", "END"),
        row("capslock", "CAPS_LOCK", "CAPSLOCK"),
        row("scrolllock", "SCROLL_LOCK", "SCROLLLOCK"),
        row("numlock", "NUM_LOCK", "NUMLOCKCLEAR"),
        row("printscreen", "PRINT_SCREEN", "PRINTSCREEN"),
        row("pause", "PAUSE", "PAUSE"),
        row("menu", "MENU", "APPLICATION"),
    ]);
    const F: [&str; 25] = [
        "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12", "F13", "F14",
        "F15", "F16", "F17", "F18", "F19", "F20", "F21", "F22", "F23", "F24", "F25",
    ];
    const FNAMES: [&str; 25] = [
        "f1", "f2", "f3", "f4", "f5", "f6", "f7", "f8", "f9", "f10", "f11", "f12", "f13", "f14",
        "f15", "f16", "f17", "f18", "f19", "f20", "f21", "f22", "f23", "f24", "f25",
    ];
    for (i, (name, f)) in FNAMES.iter().zip(F).enumerate() {
        // SDL stops at F24.
        v.push(Row {
            kui: name,
            loc: Loc::Standard,
            glfw: Some(f),
            sdl: (i < 24).then_some(f),
        });
    }
    // The keypad: its digits and operators are the characters they type,
    // its Enter is Enter, told apart by the location (KUI_KLOC_NUMPAD).
    const KP: [(&str, &str, &str); 17] = [
        ("0", "KP_0", "KP_0"),
        ("1", "KP_1", "KP_1"),
        ("2", "KP_2", "KP_2"),
        ("3", "KP_3", "KP_3"),
        ("4", "KP_4", "KP_4"),
        ("5", "KP_5", "KP_5"),
        ("6", "KP_6", "KP_6"),
        ("7", "KP_7", "KP_7"),
        ("8", "KP_8", "KP_8"),
        ("9", "KP_9", "KP_9"),
        (".", "KP_DECIMAL", "KP_PERIOD"),
        ("/", "KP_DIVIDE", "KP_DIVIDE"),
        ("*", "KP_MULTIPLY", "KP_MULTIPLY"),
        ("-", "KP_SUBTRACT", "KP_MINUS"),
        ("+", "KP_ADD", "KP_PLUS"),
        ("enter", "KP_ENTER", "KP_ENTER"),
        ("=", "KP_EQUAL", "KP_EQUALS"),
    ];
    for (kui, glfw, sdl) in KP {
        v.push(Row {
            loc: Loc::Numpad,
            ..row(kui, glfw, sdl)
        });
    }
    // The modifiers' twins.
    const MODS: [(&str, &str, &str, &str, &str); 4] = [
        ("shift", "LEFT_SHIFT", "LSHIFT", "RIGHT_SHIFT", "RSHIFT"),
        ("ctrl", "LEFT_CONTROL", "LCTRL", "RIGHT_CONTROL", "RCTRL"),
        ("alt", "LEFT_ALT", "LALT", "RIGHT_ALT", "RALT"),
        ("super", "LEFT_SUPER", "LGUI", "RIGHT_SUPER", "RGUI"),
    ];
    for (kui, gl, sl, gr, sr) in MODS {
        v.push(Row {
            loc: Loc::Left,
            ..row(kui, gl, sl)
        });
        v.push(Row {
            loc: Loc::Right,
            ..row(kui, gr, sr)
        });
    }
    // SDL's alone: GLFW names no media keys.
    const SDL_ONLY: [(&str, &str); 14] = [
        ("menu", "MENU"),
        ("clear", "CLEAR"),
        ("volumemute", "MUTE"),
        ("volumeup", "VOLUMEUP"),
        ("volumedown", "VOLUMEDOWN"),
        ("mediaplay", "MEDIA_PLAY"),
        ("mediapause", "MEDIA_PAUSE"),
        ("mediarecord", "MEDIA_RECORD"),
        ("mediafastforward", "MEDIA_FAST_FORWARD"),
        ("mediarewind", "MEDIA_REWIND"),
        ("medianext", "MEDIA_NEXT_TRACK"),
        ("mediaprev", "MEDIA_PREVIOUS_TRACK"),
        ("mediastop", "MEDIA_STOP"),
        ("mediaplaypause", "MEDIA_PLAY_PAUSE"),
    ];
    for (kui, sdl) in SDL_ONLY {
        v.push(Row {
            kui,
            loc: Loc::Standard,
            glfw: None,
            sdl: Some(sdl),
        });
    }
    v
}

/// Every row names a key kui knows, by the spelling it parses.
fn check_names(rows: &[Row]) -> Result<(), String> {
    for r in rows {
        match KeyCode::from_name(r.kui) {
            Some(KeyCode::Unknown) | None => {
                return Err(format!("{:?} is no key kui names", r.kui));
            }
            Some(k) if k.name() != r.kui => {
                return Err(format!("{:?} reads back as {:?}", r.kui, k.name()));
            }
            Some(_) => {}
        }
    }
    Ok(())
}

/// A C string literal.
fn c_str(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\"))
}

/// The pointer rules both C headers share, kui_run's (`crates/kui-native`):
/// `@p@` is the header's prefix, `@S@` its state struct, `@M@` its macro
/// prefix. The struct declares the fields these read.
const C_POINTER: &str = r#"
/* kui_run's numbers (crates/kui-native): a pause this long between two
 * scroll events ends a gesture (axis_lock.rs's GAP); a swipe chooses its
 * axis after this much travel in logical px, keeps this much of its recent
 * travel an event, and turns when the other axis carries this many times
 * the locked one's, and at least this much. */
#define @M@__GAP 0.2
#define @M@__DECIDE 4.0f
#define @M@__DECAY 0.75f
#define @M@__SWITCH_RATIO 3.0f
#define @M@__SWITCH_MIN 10.0f

static inline float @p@__abs(float v) {
    return v < 0 ? -v : v;
}

/* GLFW's or SDL's window coordinates in one logical px. */
static inline float @p@__unit(const @S@ *s) {
    return s->coord_scale > 0 ? s->coord_scale : 1.0f;
}

/* The pointer at (x, y), logical px. A wheel's gesture ends when the
 * pointer moves - the hand aims somewhere new - where a swipe's does not:
 * its glide outlives the hand. */
static inline void @p@__move(@S@ *s, float x, float y) {
    if ((x != s->x || y != s->y) && s->scroll_kind == 1) s->scroll_kind = 0;
    s->x = x;
    s->y = y;
    kui_input_cursor(s->ctx, x, y);
}

/* The pointer left the window. */
static inline void @p@__leave(@S@ *s) {
    if (s->scroll_kind == 1) s->scroll_kind = 0;
    kui_input_cursor_left(s->ctx);
}

/* A swipe keeps to the axis it started on (kui_run's axis_lock.rs): a
 * finger never moves along one axis alone, and the drift would move
 * whatever scrolls the other way under the pointer. Drops the axis the
 * swipe is not locked to. */
static inline void @p@__lock(@S@ *s, float *dx, float *dy, double now) {
    if (!s->lock_live || now < s->lock_at || now - s->lock_at > @M@__GAP) {
        s->lock_axis = 0;
        s->lock_travel[0] = s->lock_travel[1] = 0;
        s->lock_recent[0] = s->lock_recent[1] = 0;
    }
    s->lock_live = true;
    s->lock_at = now;
    float ax = @p@__abs(*dx), ay = @p@__abs(*dy);
    s->lock_recent[0] = s->lock_recent[0] * @M@__DECAY + ax;
    s->lock_recent[1] = s->lock_recent[1] * @M@__DECAY + ay;
    int axis;
    if (s->lock_axis == 0) {
        s->lock_travel[0] += ax;
        s->lock_travel[1] += ay;
        if (s->lock_travel[0] + s->lock_travel[1] >= @M@__DECIDE)
            s->lock_axis = s->lock_travel[0] > s->lock_travel[1] ? 1 : 2;
        axis = s->lock_axis ? s->lock_axis : ax > ay ? 1 : 2;
    } else {
        float along = s->lock_recent[s->lock_axis - 1], across = s->lock_recent[2 - s->lock_axis];
        if (across >= @M@__SWITCH_MIN && across > along * @M@__SWITCH_RATIO) s->lock_axis = 3 - s->lock_axis;
        axis = s->lock_axis;
    }
    if (axis == 1) {
        *dy = 0;
    } else {
        *dx = 0;
    }
}

/* One scroll event at `now` seconds, in the library's units (+y up). A
 * gesture begins - and picks the scroller the rest go to - on the first
 * event, after a pause, when the kind changes, and for a wheel when the
 * pointer moved since its last notch. Lines go to kui_input_wheel, whose
 * whole notches glide; on macOS a swipe's points (the library's 0.1
 * undone) go to kui_input_scroll_gesture, held to their axis. */
static inline void @p@__scroll(@S@ *s, double dx, double dy, double now) {
    if (dx == 0 && dy == 0) return;
    bool fresh = s->scroll_kind == 0 || now < s->scroll_at || now - s->scroll_at > @M@__GAP;
    int kind = 1;
#if defined(__APPLE__)
    if (dx != (double)(long long)dx || dy != (double)(long long)dy || (!fresh && s->scroll_kind == 2)) kind = 2;
#endif
    if (fresh || kind != s->scroll_kind) s->scroll_owed = true;
    s->scroll_kind = kind;
    s->scroll_at = now;
    float px = (float)dx, py = (float)dy;
    if (kind == 2) {
        float unit = 10.0f / @p@__unit(s);
        px *= unit;
        py *= unit;
        @p@__lock(s, &px, &py, now);
        /* Kept to zero: the next event dispatched says the gesture begins. */
        if (px == 0 && py == 0) return;
    } else {
        s->lock_live = false;
    }
    bool begins = s->scroll_owed;
    s->scroll_owed = false;
    if (kind == 2) {
        kui_input_scroll_gesture(s->ctx, px, py, begins);
    } else {
        kui_input_wheel(s->ctx, px, py, begins);
    }
}
"#;

/// The state fields [`C_POINTER`] reads, after the struct's own.
const C_POINTER_FIELDS: &str = r#"    /* The pointer, logical px. */
    float x, y;
    /* The scroll gesture: its kind (0 none, 1 lines, 2 a swipe's points),
     * its last event, and whether the next dispatched begins one. */
    int scroll_kind;
    double scroll_at;
    bool scroll_owed;
    /* A swipe's axis lock: 0 undecided, 1 x, 2 y. */
    int lock_axis;
    bool lock_live;
    double lock_at;
    float lock_travel[2], lock_recent[2];
"#;

fn c_pointer(prefix: &str, state: &str, macro_prefix: &str) -> String {
    C_POINTER
        .replace("@p@", prefix)
        .replace("@S@", state)
        .replace("@M@", macro_prefix)
}

const GENERATED: &str = "Generated by `cargo run -p kui-devtools --bin keytables` from the table in\n * examples/devtools/src/bin/keytables.rs; edit the table, not this file.";

fn glfw_h(rows: &[Row]) -> String {
    let mut o = String::new();
    let _ = write!(
        o,
        r#"/* kui_glfw.h - GLFW's keys and pointer as kui's (backlog F239, F265).
 * {GENERATED}
 *
 * Include after <GLFW/glfw3.h> and "kui.h". Header-only: libkui_ffi knows
 * no window library, so nothing here is in it.
 *
 * A host with no GLFW callbacks of its own installs every one, keys and
 * pointer, on state it owns (the pointer half, below):
 *
 *     static KuiGlfw input;
 *     input.ctx = ui;
 *     glfwSetInputMode(window, GLFW_LOCK_KEY_MODS, GLFW_TRUE);
 *     kui_glfw_install(&input, window);
 *
 * Keys, by hand:
 *
 *     glfwSetInputMode(window, GLFW_LOCK_KEY_MODS, GLFW_TRUE);
 *     static void on_key(GLFWwindow *w, int key, int scancode, int action, int mods) {{
 *         kui_glfw_key(ui, key, scancode, action, mods);
 *     }}
 *     static void on_char(GLFWwindow *w, unsigned int codepoint) {{
 *         kui_glfw_char(ui, codepoint);
 *     }}
 *
 * GLFW names a key by its place on a US keyboard (GLFW_KEY_*), which is
 * kui's `physical`; `code` is what the layout makes of it
 * (glfwGetKeyName), a letter upper-cased under Shift or Caps Lock - which
 * GLFW reports only with GLFW_LOCK_KEY_MODS on. A named key (the arrows,
 * Enter, Tab, Escape, F5) goes through kui_input_press, so it means what
 * it means; a key that types (a character, Space) goes through
 * kui_input_key_down, the raw half a key sink and an editor's chords read,
 * and what it types arrives once, from the char callback, as
 * kui_input_text. A key sink's `text` for it is kui's guess from `code`.
 * kui_input_modifiers follows every key. */
#ifndef KUI_GLFW_H
#define KUI_GLFW_H

#include <stdbool.h>
#include <stdint.h>
#include <string.h>

/* kui's name for the key at GLFW `key`'s place, NULL for a key kui names
 * none for (GLFW_KEY_WORLD_1, _2); `location`, when not NULL, gets its
 * KUI_KLOC_* (0 for most). */
static inline const char *kui_glfw_physical(int key, uint32_t *location) {{
    uint32_t loc = 0;
    const char *name = NULL;
    switch (key) {{
"#
    );
    for r in rows {
        if let Some(g) = r.glfw {
            let _ = writeln!(
                o,
                "    case GLFW_KEY_{g}: name = {}; loc = {}; break;",
                c_str(r.kui),
                r.loc.c()
            );
        }
    }
    let _ = write!(
        o,
        r#"    default: break;
    }}
    if (location) *location = loc;
    return name;
}}

/* GLFW's `mods` as KUI_KMOD_* and KUI_KLOCK_* bits. */
static inline uint32_t kui_glfw_mods(int mods) {{
    uint32_t k = 0;
    if (mods & GLFW_MOD_SHIFT) k |= KUI_KMOD_SHIFT;
    if (mods & GLFW_MOD_CONTROL) k |= KUI_KMOD_CTRL;
    if (mods & GLFW_MOD_ALT) k |= KUI_KMOD_ALT;
    if (mods & GLFW_MOD_SUPER) k |= KUI_KMOD_SUPER;
    if (mods & GLFW_MOD_CAPS_LOCK) k |= KUI_KLOCK_CAPS;
    if (mods & GLFW_MOD_NUM_LOCK) k |= KUI_KLOCK_NUM;
    return k;
}}

/* Whether `s` is one UTF-8 character. */
static inline bool kui_glfw__one_char(const char *s) {{
    size_t n = strlen(s);
    unsigned char c = (unsigned char)s[0];
    size_t want = c < 0x80 ? 1 : (c >> 5) == 6 ? 2 : (c >> 4) == 14 ? 3 : (c >> 3) == 30 ? 4 : 0;
    return n > 0 && n == want;
}}

/* A GLFW key callback's arguments, into kui. */
static inline void kui_glfw_key(KuiCtx *ctx, int key, int scancode, int action, int mods) {{
    uint32_t location = 0;
    const char *physical = kui_glfw_physical(key, &location);
    if (!physical) return;
    uint32_t kmods = kui_glfw_mods(mods);
    kui_input_modifiers(ctx, kmods & (KUI_KMOD_SHIFT | KUI_KMOD_CTRL | KUI_KMOD_ALT | KUI_KMOD_SUPER));
    kmods |= location;
    bool types = physical[1] == '\0' || strcmp(physical, "space") == 0;
    char code[8];
    const char *layout = glfwGetKeyName(key, scancode);
    if (layout && kui_glfw__one_char(layout)) {{
        types = true;
    }} else {{
        layout = physical;
    }}
    size_t n = strlen(layout);
    memcpy(code, layout, n + 1);
    bool upper = ((mods & GLFW_MOD_SHIFT) != 0) != ((mods & GLFW_MOD_CAPS_LOCK) != 0);
    if (upper && n == 1 && code[0] >= 'a' && code[0] <= 'z') code[0] = (char)(code[0] - 32);
    KuiStr none = {{NULL, 0}};
    if (action == GLFW_RELEASE) {{
        kui_input_release(ctx, kui__cstr(code), kui__cstr(physical), kmods);
    }} else if (types) {{
        kui_input_key_down(ctx, kui__cstr(code), kui__cstr(physical), kmods, none, action == GLFW_REPEAT);
    }} else {{
        kui_input_press(ctx, kui__cstr(code), kui__cstr(physical), kmods, none, action == GLFW_REPEAT);
    }}
}}

/* A GLFW char callback's codepoint, into kui as typed text. */
static inline void kui_glfw_char(KuiCtx *ctx, unsigned int cp) {{
    char s[4];
    size_t n = 0;
    if (cp < 0x80) {{
        s[n++] = (char)cp;
    }} else if (cp < 0x800) {{
        s[n++] = (char)(0xc0 | (cp >> 6));
        s[n++] = (char)(0x80 | (cp & 0x3f));
    }} else if (cp < 0x10000) {{
        s[n++] = (char)(0xe0 | (cp >> 12));
        s[n++] = (char)(0x80 | ((cp >> 6) & 0x3f));
        s[n++] = (char)(0x80 | (cp & 0x3f));
    }} else if (cp < 0x110000) {{
        s[n++] = (char)(0xf0 | (cp >> 18));
        s[n++] = (char)(0x80 | ((cp >> 12) & 0x3f));
        s[n++] = (char)(0x80 | ((cp >> 6) & 0x3f));
        s[n++] = (char)(0x80 | (cp & 0x3f));
    }}
    if (n) {{
        KuiStr typed = {{(const uint8_t *)s, n}};
        kui_input_text(ctx, typed);
    }}
}}
"#
    );
    o.push_str(GLFW_POINTER_HEAD);
    o.push_str(C_POINTER_FIELDS);
    o.push_str(GLFW_POINTER_STATE_TAIL);
    o.push_str(&c_pointer("kui_glfw", "KuiGlfw", "KUI_GLFW"));
    o.push_str(GLFW_POINTER);
    o.push_str("\n#endif /* KUI_GLFW_H */\n");
    o
}

/// kui_glfw.h's pointer half, up to the state's shared fields.
const GLFW_POINTER_HEAD: &str = r#"
/* -- The pointer (backlog F265) ------------------------------------------
 *
 * The cursor, the window's edge, the buttons and the scroll, sent the way
 * kui_run sends them, from state the host owns and zeroes: kui_glfw_install
 * sets every callback (above), or a host with callbacks of its own calls
 * one helper in each.
 *
 *     static KuiGlfw input;  // input.ctx = ui;
 *     kui_glfw_cursor(&input, x, y);                  // cursor position
 *     kui_glfw_enter(&input, entered);                // cursor enter
 *     kui_glfw_button(&input, button, action, mods);  // mouse button
 *     kui_glfw_scroll(&input, dx, dy);                // scroll
 *
 * Clicks: GLFW counts none, so a press of the primary button within 0.4 s
 * and 4 logical px of the last counts up 1, 2, 3 (a word, a line) and
 * round again - kui_run's rule, which reads no platform's double-click
 * time either. Every button goes through kui_input_mouse_button: GLFW's
 * left, right and middle are kui's primary, secondary and middle, and its
 * button n past them kui's n (KUI_MOUSE_OTHER + n - 3).
 *
 * Scroll: GLFW's deltas are a wheel's lines (Windows, X11: 1.0 a notch, a
 * high-resolution wheel's fractions between) and go to kui_input_wheel,
 * whose whole notches glide - except on macOS, where GLFW hands a device
 * with precise deltas (a trackpad, a Magic Mouse) as its points scaled by
 * 0.1 (cocoa_window.m's scrollWheel:) and says nothing of which it was. A
 * wheel's deltas come whole there, so a delta with a fraction, or one in a
 * gesture that had one, is a swipe's: ten times it goes to
 * kui_input_scroll_gesture, held to the axis the swipe started on. A
 * gesture begins, as kui_run's do, on the first event, after a pause of
 * 0.2 s, when lines turn to points or back, and for a wheel when the
 * pointer moved since its last notch.
 *
 * What GLFW does not report, this cannot follow: it has no touch phase,
 * so a finger put down while the last swipe still glides continues that
 * gesture until a pause (kui_run begins a new one), and the pointer moving
 * never ends a swipe's gesture, as it would a smooth-scrolling mouse's
 * there. On macOS a swipe whose first delta happens to be whole is read
 * as a notch. On Wayland GLFW hands a swipe's pixels over 10 too, which
 * read as lines: four times as far as the swipe went.
 *
 * Time: glfwGetTime(), unless KUI_GLFW_TIME() names another clock in
 * seconds before this header is included (a test does). */
#ifndef KUI_GLFW_TIME
#define KUI_GLFW_TIME() glfwGetTime()
#endif

/* What the pointer helpers keep. Zero it, set ctx, and keep it as long as
 * the window sends to it. */
typedef struct KuiGlfw {
    KuiCtx *ctx;
    /* GLFW screen coordinates in one of kui's logical px; 0 is 1. They
     * are the same when the frame's size is glfwGetWindowSize's and its
     * scale the framebuffer's over it - always on macOS and Wayland, where
     * screen coordinates are points. On Windows and X11 they are pixels: a
     * host that scales its UI by glfwGetWindowContentScale sets that
     * scale here. */
    float coord_scale;
    /* The rest is the helpers' own. */
"#;

/// kui_glfw.h's state, after the shared fields.
const GLFW_POINTER_STATE_TAIL: &str = r#"    /* The last primary press and the count it made. */
    double click_at;
    float click_x, click_y;
    uint32_t clicks;
    /* kui_glfw_install's: the window, the callbacks the host set before,
     * and the next installed state. */
    GLFWwindow *window;
    GLFWcursorposfun prev_cursor;
    GLFWcursorenterfun prev_enter;
    GLFWmousebuttonfun prev_button;
    GLFWscrollfun prev_scroll;
    GLFWkeyfun prev_key;
    GLFWcharfun prev_char;
    struct KuiGlfw *next;
} KuiGlfw;
"#;

/// kui_glfw.h's pointer helpers and kui_glfw_install, after the shared
/// rules.
const GLFW_POINTER: &str = r#"
/* kui_run's: presses of the primary button within this long (s) and this
 * far apart (logical px) count up. */
#define KUI_GLFW__CLICK_TIME 0.4
#define KUI_GLFW__CLICK_SLOP 4.0f

/* A cursor-position callback's arguments, into kui. */
static inline void kui_glfw_cursor(KuiGlfw *g, double x, double y) {
    float unit = kui_glfw__unit(g);
    kui_glfw__move(g, (float)(x / unit), (float)(y / unit));
}

/* A cursor-enter callback's argument, into kui: leaving is
 * kui_input_cursor_left; entering waits for the cursor's position. */
static inline void kui_glfw_enter(KuiGlfw *g, int entered) {
    if (!entered) kui_glfw__leave(g);
}

/* A mouse-button callback's arguments, into kui: the modifiers it carries,
 * then the button, a primary press counted. */
static inline void kui_glfw_button(KuiGlfw *g, int button, int action, int mods) {
    if (button < 0) return;
    kui_input_modifiers(g->ctx, kui_glfw_mods(mods) & (KUI_KMOD_SHIFT | KUI_KMOD_CTRL | KUI_KMOD_ALT | KUI_KMOD_SUPER));
    bool down = action != GLFW_RELEASE;
    uint32_t clicks = 1;
    if (down && button == GLFW_MOUSE_BUTTON_LEFT) {
        double now = KUI_GLFW_TIME();
        bool again = g->clicks > 0 && now >= g->click_at && now - g->click_at < KUI_GLFW__CLICK_TIME &&
                     kui_glfw__abs(g->x - g->click_x) < KUI_GLFW__CLICK_SLOP &&
                     kui_glfw__abs(g->y - g->click_y) < KUI_GLFW__CLICK_SLOP;
        clicks = again ? g->clicks % 3 + 1 : 1;
        g->clicks = clicks;
        g->click_at = now;
        g->click_x = g->x;
        g->click_y = g->y;
    }
    kui_input_mouse_button(g->ctx, down, (uint32_t)button, clicks);
}

/* A scroll callback's arguments, into kui. */
static inline void kui_glfw_scroll(KuiGlfw *g, double dx, double dy) {
    kui_glfw__scroll(g, dx, dy, KUI_GLFW_TIME());
}

/* -- kui_glfw_install ------------------------------------------------------
 *
 * Sets the cursor-position, cursor-enter, mouse-button, scroll, key and
 * char callbacks on `window`, each calling the one the host set before it
 * first (so set yours first), then kui - as Dear ImGui's GLFW backend
 * chains. It sets no input mode: GLFW_LOCK_KEY_MODS, which Caps Lock's
 * upper case needs, is the host's to turn on, since its own callbacks then
 * hear the lock bits too.
 *
 * GLFW's window user pointer stays the host's. A callback finds its state
 * by the window instead, in a list of the installed states threaded
 * through the states themselves - the table ImGui's backend keeps, with
 * nothing allocated and no cap. The list is the translation unit's that
 * calls kui_glfw_install (a static inline function's static is its own in
 * each), as the callbacks it sets are, so install and uninstall from one
 * file. kui_glfw_uninstall puts the host's callbacks back and forgets the
 * window: call it before the state goes. */
static inline KuiGlfw **kui_glfw__installed(void) {
    static KuiGlfw *head;
    return &head;
}

static inline KuiGlfw *kui_glfw__find(GLFWwindow *window) {
    for (KuiGlfw *g = *kui_glfw__installed(); g; g = g->next) {
        if (g->window == window) return g;
    }
    return NULL;
}

static inline void kui_glfw__link(KuiGlfw *g, GLFWwindow *window) {
    g->window = window;
    g->next = *kui_glfw__installed();
    *kui_glfw__installed() = g;
}

static inline void kui_glfw__unlink(KuiGlfw *g) {
    for (KuiGlfw **at = kui_glfw__installed(); *at; at = &(*at)->next) {
        if (*at == g) {
            *at = g->next;
            break;
        }
    }
    g->next = NULL;
    g->window = NULL;
}

static inline void kui_glfw__on_cursor(GLFWwindow *w, double x, double y) {
    KuiGlfw *g = kui_glfw__find(w);
    if (!g) return;
    if (g->prev_cursor) g->prev_cursor(w, x, y);
    kui_glfw_cursor(g, x, y);
}

static inline void kui_glfw__on_enter(GLFWwindow *w, int entered) {
    KuiGlfw *g = kui_glfw__find(w);
    if (!g) return;
    if (g->prev_enter) g->prev_enter(w, entered);
    kui_glfw_enter(g, entered);
}

static inline void kui_glfw__on_button(GLFWwindow *w, int button, int action, int mods) {
    KuiGlfw *g = kui_glfw__find(w);
    if (!g) return;
    if (g->prev_button) g->prev_button(w, button, action, mods);
    kui_glfw_button(g, button, action, mods);
}

static inline void kui_glfw__on_scroll(GLFWwindow *w, double dx, double dy) {
    KuiGlfw *g = kui_glfw__find(w);
    if (!g) return;
    if (g->prev_scroll) g->prev_scroll(w, dx, dy);
    kui_glfw_scroll(g, dx, dy);
}

static inline void kui_glfw__on_key(GLFWwindow *w, int key, int scancode, int action, int mods) {
    KuiGlfw *g = kui_glfw__find(w);
    if (!g) return;
    if (g->prev_key) g->prev_key(w, key, scancode, action, mods);
    kui_glfw_key(g->ctx, key, scancode, action, mods);
}

static inline void kui_glfw__on_char(GLFWwindow *w, unsigned int codepoint) {
    KuiGlfw *g = kui_glfw__find(w);
    if (!g) return;
    if (g->prev_char) g->prev_char(w, codepoint);
    kui_glfw_char(g->ctx, codepoint);
}

/* Every callback on `window`, into kui through `g`, chained to the host's. */
static inline void kui_glfw_install(KuiGlfw *g, GLFWwindow *window) {
    if (g->window) kui_glfw__unlink(g);
    kui_glfw__link(g, window);
    g->prev_cursor = glfwSetCursorPosCallback(window, kui_glfw__on_cursor);
    g->prev_enter = glfwSetCursorEnterCallback(window, kui_glfw__on_enter);
    g->prev_button = glfwSetMouseButtonCallback(window, kui_glfw__on_button);
    g->prev_scroll = glfwSetScrollCallback(window, kui_glfw__on_scroll);
    g->prev_key = glfwSetKeyCallback(window, kui_glfw__on_key);
    g->prev_char = glfwSetCharCallback(window, kui_glfw__on_char);
}

/* The host's callbacks back on the window kui_glfw_install took, and the
 * window forgotten. */
static inline void kui_glfw_uninstall(KuiGlfw *g) {
    GLFWwindow *w = g->window;
    if (!w) return;
    glfwSetCursorPosCallback(w, g->prev_cursor);
    glfwSetCursorEnterCallback(w, g->prev_enter);
    glfwSetMouseButtonCallback(w, g->prev_button);
    glfwSetScrollCallback(w, g->prev_scroll);
    glfwSetKeyCallback(w, g->prev_key);
    glfwSetCharCallback(w, g->prev_char);
    g->prev_cursor = NULL;
    g->prev_enter = NULL;
    g->prev_button = NULL;
    g->prev_scroll = NULL;
    g->prev_key = NULL;
    g->prev_char = NULL;
    kui_glfw__unlink(g);
}
"#;

fn sdl3_h(rows: &[Row]) -> String {
    let mut o = String::new();
    let _ = write!(
        o,
        r#"/* kui_sdl3.h - SDL3's keys and pointer as kui's (backlog F239, F265).
 * {GENERATED}
 *
 * Include after <SDL3/SDL.h> and "kui.h". Header-only: libkui_ffi knows
 * no window library, so nothing here is in it, and nothing here calls SDL.
 *
 * Every event it knows - keys, text, the pointer and the wheel - in one
 * call, on state the host owns (the pointer half, below):
 *
 *     static KuiSdl3 input;
 *     input.ctx = ui;
 *     SDL_StartTextInput(window);
 *     while (SDL_PollEvent(&ev)) kui_sdl3_event(&input, &ev);
 *
 * Keys, by hand:
 *
 *     SDL_StartTextInput(window);
 *     switch (ev.type) {{
 *     case SDL_EVENT_KEY_DOWN: case SDL_EVENT_KEY_UP: kui_sdl3_key(ui, &ev.key); break;
 *     case SDL_EVENT_TEXT_INPUT: kui_sdl3_text(ui, &ev.text); break;
 *     }}
 *
 * SDL names a key by its place on a US keyboard (its scancode), which is
 * kui's `physical`; `code` is what the layout makes of it (the event's
 * keycode), a letter upper-cased under Shift or Caps Lock. A named key
 * (the arrows, Enter, Tab, Escape, F5) goes through kui_input_press, so it
 * means what it means; a key that types (a character, Space) goes through
 * kui_input_key_down, the raw half a key sink and an editor's chords read,
 * and what it types arrives once, from SDL_EVENT_TEXT_INPUT, as
 * kui_input_text. A key sink's `text` for it is kui's guess from `code`.
 * kui_input_modifiers follows every key. */
#ifndef KUI_SDL3_H
#define KUI_SDL3_H

#include <stdbool.h>
#include <stdint.h>
#include <string.h>

/* kui's name for the key at SDL scancode `sc`'s place, NULL for a key kui
 * names none for; `location`, when not NULL, gets its KUI_KLOC_* (0 for
 * most). */
static inline const char *kui_sdl3_physical(SDL_Scancode sc, uint32_t *location) {{
    uint32_t loc = 0;
    const char *name = NULL;
    switch (sc) {{
"#
    );
    for r in rows {
        if let Some(s) = r.sdl {
            let _ = writeln!(
                o,
                "    case SDL_SCANCODE_{s}: name = {}; loc = {}; break;",
                c_str(r.kui),
                r.loc.c()
            );
        }
    }
    let _ = write!(
        o,
        r#"    default: break;
    }}
    if (location) *location = loc;
    return name;
}}

/* SDL's modifier state as KUI_KMOD_* and KUI_KLOCK_* bits. */
static inline uint32_t kui_sdl3_mods(SDL_Keymod mod) {{
    uint32_t k = 0;
    if (mod & SDL_KMOD_SHIFT) k |= KUI_KMOD_SHIFT;
    if (mod & SDL_KMOD_CTRL) k |= KUI_KMOD_CTRL;
    if (mod & SDL_KMOD_ALT) k |= KUI_KMOD_ALT;
    if (mod & SDL_KMOD_GUI) k |= KUI_KMOD_SUPER;
    if (mod & SDL_KMOD_CAPS) k |= KUI_KLOCK_CAPS;
    if (mod & SDL_KMOD_NUM) k |= KUI_KLOCK_NUM;
    return k;
}}

/* An SDL_EVENT_KEY_DOWN or SDL_EVENT_KEY_UP, into kui. */
static inline void kui_sdl3_key(KuiCtx *ctx, const SDL_KeyboardEvent *e) {{
    uint32_t location = 0;
    const char *physical = kui_sdl3_physical(e->scancode, &location);
    if (!physical) return;
    uint32_t kmods = kui_sdl3_mods(e->mod);
    kui_input_modifiers(ctx, kmods & (KUI_KMOD_SHIFT | KUI_KMOD_CTRL | KUI_KMOD_ALT | KUI_KMOD_SUPER));
    kmods |= location;
    bool types = physical[1] == '\0' || strcmp(physical, "space") == 0;
    char code[8];
    uint32_t k = (uint32_t)e->key;
    size_t n = 0;
    if ((k & SDLK_SCANCODE_MASK) == 0 && k > 0x20 && k != 0x7f && k < 0x110000) {{
        /* The layout's character, UTF-8. */
        if (k < 0x80) {{
            code[n++] = (char)k;
        }} else if (k < 0x800) {{
            code[n++] = (char)(0xc0 | (k >> 6));
            code[n++] = (char)(0x80 | (k & 0x3f));
        }} else if (k < 0x10000) {{
            code[n++] = (char)(0xe0 | (k >> 12));
            code[n++] = (char)(0x80 | ((k >> 6) & 0x3f));
            code[n++] = (char)(0x80 | (k & 0x3f));
        }} else {{
            code[n++] = (char)(0xf0 | (k >> 18));
            code[n++] = (char)(0x80 | ((k >> 12) & 0x3f));
            code[n++] = (char)(0x80 | ((k >> 6) & 0x3f));
            code[n++] = (char)(0x80 | (k & 0x3f));
        }}
        code[n] = '\0';
        types = true;
    }} else {{
        n = strlen(physical);
        memcpy(code, physical, n + 1);
    }}
    bool upper = ((e->mod & SDL_KMOD_SHIFT) != 0) != ((e->mod & SDL_KMOD_CAPS) != 0);
    if (upper && n == 1 && code[0] >= 'a' && code[0] <= 'z') code[0] = (char)(code[0] - 32);
    KuiStr none = {{NULL, 0}};
    if (!e->down) {{
        kui_input_release(ctx, kui__cstr(code), kui__cstr(physical), kmods);
    }} else if (types) {{
        kui_input_key_down(ctx, kui__cstr(code), kui__cstr(physical), kmods, none, e->repeat);
    }} else {{
        kui_input_press(ctx, kui__cstr(code), kui__cstr(physical), kmods, none, e->repeat);
    }}
}}

/* An SDL_EVENT_TEXT_INPUT, into kui as typed text. */
static inline void kui_sdl3_text(KuiCtx *ctx, const SDL_TextInputEvent *e) {{
    if (e->text && e->text[0]) kui_input_text(ctx, kui__cstr(e->text));
}}
"#
    );
    o.push_str(SDL3_POINTER_HEAD);
    o.push_str(C_POINTER_FIELDS);
    o.push_str("} KuiSdl3;\n");
    o.push_str(&c_pointer("kui_sdl3", "KuiSdl3", "KUI_SDL3"));
    o.push_str(SDL3_POINTER);
    o.push_str("\n#endif /* KUI_SDL3_H */\n");
    o
}

/// kui_sdl3.h's pointer half, up to the state's shared fields.
const SDL3_POINTER_HEAD: &str = r#"
/* -- The pointer (backlog F265) ------------------------------------------
 *
 * The cursor, the window's edge, the buttons and the wheel, sent the way
 * kui_run sends them, from state the host owns and zeroes: kui_sdl3_event
 * (above) forwards each, or a host handling some itself calls one helper
 * for each event.
 *
 *     static KuiSdl3 input;  // input.ctx = ui;
 *     kui_sdl3_cursor(&input, &ev.motion);   // SDL_EVENT_MOUSE_MOTION
 *     kui_sdl3_enter(&input, false);         // SDL_EVENT_WINDOW_MOUSE_LEAVE
 *     kui_sdl3_button(&input, &ev.button);   // SDL_EVENT_MOUSE_BUTTON_*
 *     kui_sdl3_scroll(&input, &ev.wheel);    // SDL_EVENT_MOUSE_WHEEL
 *
 * Clicks: SDL counts them (the event's `clicks`, by the platform's
 * double-click time where it has one), and kui takes 1, 2, 3 (a word, a
 * line) and round again, as kui_run counts; only the primary button's
 * count. Every button goes through kui_input_mouse_button: SDL's left,
 * right and middle are kui's primary, secondary and middle, X1 and X2 kui's
 * KUI_MOUSE_OTHER and the one after it.
 *
 * Scroll: SDL's deltas are a wheel's lines (Windows, X11 and Wayland:
 * 1.0 a notch, a high-resolution wheel's fractions between) and go to
 * kui_input_wheel, whose whole notches glide - except on macOS, where SDL
 * 3.4 hands a device with precise deltas (a trackpad, a Magic Mouse) as
 * its points scaled by 0.1 (SDL_cocoamouse.m's Cocoa_HandleMouseWheel)
 * and says nothing of which it was. A wheel's deltas come whole there, so
 * a delta with a fraction, or one in a gesture that had one, is a
 * swipe's: ten times it goes to kui_input_scroll_gesture, held to the axis
 * the swipe started on. A gesture begins, as kui_run's do, on the first
 * event, after a pause of 0.2 s by the events' timestamps, when lines turn
 * to points or back, and for a wheel when the pointer moved since its last
 * notch. SDL's x is positive to the right and kui's, as winit's and
 * GLFW's, to the left, so it is turned; `direction` is not read, the
 * deltas being the content's way already.
 *
 * What SDL does not report, this cannot follow: it has no touch phase,
 * so a finger put down while the last swipe still glides continues that
 * gesture until a pause (kui_run begins a new one), and the pointer moving
 * never ends a swipe's gesture, as it would a smooth-scrolling mouse's
 * there. On macOS a swipe whose first delta happens to be whole is read
 * as a notch. On Wayland a swipe's pixels arrive over 10 as well, which
 * read as lines: four times as far as the swipe went. IME composition
 * (SDL_EVENT_TEXT_EDITING) is not forwarded. */

/* What the pointer helpers keep. Zero it, set ctx, and keep it as long as
 * events come for it. */
typedef struct KuiSdl3 {
    KuiCtx *ctx;
    /* SDL's window coordinates in one of kui's logical px; 0 is 1. They
     * are the same when the frame's size is SDL_GetWindowSize's and its
     * scale SDL_GetWindowPixelDensity's - always on macOS and Wayland,
     * where window coordinates are points. On Windows and X11 they are
     * pixels: a host that scales its UI by SDL_GetWindowDisplayScale sets
     * that scale here. */
    float coord_scale;
    /* The window whose events kui_sdl3_event forwards; 0 is every one. */
    SDL_WindowID window;
    /* The rest is the helpers' own. */
"#;

/// kui_sdl3.h's pointer helpers and kui_sdl3_event, after the shared
/// rules.
const SDL3_POINTER: &str = r#"
/* An SDL_EVENT_MOUSE_MOTION, into kui. */
static inline void kui_sdl3_cursor(KuiSdl3 *s, const SDL_MouseMotionEvent *e) {
    float unit = kui_sdl3__unit(s);
    kui_sdl3__move(s, e->x / unit, e->y / unit);
}

/* SDL_EVENT_WINDOW_MOUSE_ENTER (true) or _LEAVE (false), into kui: leaving
 * is kui_input_cursor_left; entering waits for the cursor's position. */
static inline void kui_sdl3_enter(KuiSdl3 *s, bool entered) {
    if (!entered) kui_sdl3__leave(s);
}

/* An SDL_EVENT_MOUSE_BUTTON_DOWN or _UP, into kui: the pointer where the
 * event says it is, then the button. */
static inline void kui_sdl3_button(KuiSdl3 *s, const SDL_MouseButtonEvent *e) {
    if (e->button == 0) return;
    float unit = kui_sdl3__unit(s);
    float x = e->x / unit, y = e->y / unit;
    if (x != s->x || y != s->y) kui_sdl3__move(s, x, y);
    uint32_t button = e->button == SDL_BUTTON_LEFT    ? KUI_MOUSE_PRIMARY
                      : e->button == SDL_BUTTON_RIGHT  ? KUI_MOUSE_SECONDARY
                      : e->button == SDL_BUTTON_MIDDLE ? KUI_MOUSE_MIDDLE
                                                       : (uint32_t)e->button - 1;
    uint32_t clicks = button == KUI_MOUSE_PRIMARY && e->clicks > 0 ? (uint32_t)(e->clicks - 1) % 3 + 1 : 1;
    kui_input_mouse_button(s->ctx, e->down, button, clicks);
}

/* An SDL_EVENT_MOUSE_WHEEL, into kui. */
static inline void kui_sdl3_scroll(KuiSdl3 *s, const SDL_MouseWheelEvent *e) {
    kui_sdl3__scroll(s, -(double)e->x, (double)e->y, (double)e->timestamp / 1e9);
}

static inline bool kui_sdl3__ours(const KuiSdl3 *s, SDL_WindowID window) {
    return s->window == 0 || window == s->window;
}

/* Every event kind above, into kui: keys, text, the pointer, the wheel and
 * the pointer leaving the window. True when it was one of those, for this
 * state's window - not whether kui wanted it, which kui_pointer_wants and
 * kui_keyboard_wants say. */
static inline bool kui_sdl3_event(KuiSdl3 *s, const SDL_Event *e) {
    switch (e->type) {
    case SDL_EVENT_KEY_DOWN:
    case SDL_EVENT_KEY_UP:
        if (!kui_sdl3__ours(s, e->key.windowID)) return false;
        kui_sdl3_key(s->ctx, &e->key);
        return true;
    case SDL_EVENT_TEXT_INPUT:
        if (!kui_sdl3__ours(s, e->text.windowID)) return false;
        kui_sdl3_text(s->ctx, &e->text);
        return true;
    case SDL_EVENT_MOUSE_MOTION:
        if (!kui_sdl3__ours(s, e->motion.windowID)) return false;
        kui_sdl3_cursor(s, &e->motion);
        return true;
    case SDL_EVENT_MOUSE_BUTTON_DOWN:
    case SDL_EVENT_MOUSE_BUTTON_UP:
        if (!kui_sdl3__ours(s, e->button.windowID)) return false;
        kui_sdl3_button(s, &e->button);
        return true;
    case SDL_EVENT_MOUSE_WHEEL:
        if (!kui_sdl3__ours(s, e->wheel.windowID)) return false;
        kui_sdl3_scroll(s, &e->wheel);
        return true;
    case SDL_EVENT_WINDOW_MOUSE_ENTER:
    case SDL_EVENT_WINDOW_MOUSE_LEAVE:
        if (!kui_sdl3__ours(s, e->window.windowID)) return false;
        kui_sdl3_enter(s, e->type == SDL_EVENT_WINDOW_MOUSE_ENTER);
        return true;
    default:
        return false;
    }
}
"#;

/// An Odin string literal.
fn odin_str(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\"))
}

/// The state fields [`ODIN_POINTER`] reads, after the struct's own.
const ODIN_POINTER_FIELDS: &str = r#"	// The pointer, logical px.
	x, y:        f32,
	// The scroll gesture: its kind (0 none, 1 lines, 2 a swipe's points),
	// its last event, and whether the next dispatched begins one.
	scroll_kind: int,
	scroll_at:   f64,
	scroll_owed: bool,
	// A swipe's axis lock: 0 undecided, 1 x, 2 y.
	lock_axis:   int,
	lock_live:   bool,
	lock_at:     f64,
	lock_travel: [2]f32,
	lock_recent: [2]f32,
"#;

/// The pointer rules both Odin packages share: [`C_POINTER`]'s, line for
/// line.
const ODIN_POINTER: &str = r#"
// kui_run's numbers (crates/kui-native): a pause this long between two
// scroll events ends a gesture (axis_lock.rs's GAP); a swipe chooses its
// axis after this much travel in logical px, keeps this much of its recent
// travel an event, and turns when the other axis carries this many times
// the locked one's, and at least this much.
@(private)
GAP :: 0.2
@(private)
DECIDE :: f32(4)
@(private)
DECAY :: f32(0.75)
@(private)
SWITCH_RATIO :: f32(3)
@(private)
SWITCH_MIN :: f32(10)

// The library's window coordinates in one logical px.
@(private)
unit :: proc(s: ^State) -> f32 {
	return s.coord_scale > 0 ? s.coord_scale : 1
}

// The pointer at (x, y), logical px. A wheel's gesture ends when the
// pointer moves - the hand aims somewhere new - where a swipe's does not:
// its glide outlives the hand.
@(private)
move :: proc(s: ^State, x, y: f32) {
	if (x != s.x || y != s.y) && s.scroll_kind == 1 do s.scroll_kind = 0
	s.x = x
	s.y = y
	kui.input_cursor(s.ui, x, y)
}

// The pointer left the window.
@(private)
leave :: proc(s: ^State) {
	if s.scroll_kind == 1 do s.scroll_kind = 0
	kui.input_cursor_left(s.ui)
}

// A swipe keeps to the axis it started on (kui_run's axis_lock.rs): a
// finger never moves along one axis alone, and the drift would move
// whatever scrolls the other way under the pointer. Drops the axis the
// swipe is not locked to.
@(private)
lock :: proc(s: ^State, dx, dy: ^f32, now: f64) {
	if !s.lock_live || now < s.lock_at || now - s.lock_at > GAP {
		s.lock_axis = 0
		s.lock_travel = {}
		s.lock_recent = {}
	}
	s.lock_live = true
	s.lock_at = now
	ax, ay := abs(dx^), abs(dy^)
	s.lock_recent = {s.lock_recent[0] * DECAY + ax, s.lock_recent[1] * DECAY + ay}
	axis: int
	if s.lock_axis == 0 {
		s.lock_travel += {ax, ay}
		if s.lock_travel[0] + s.lock_travel[1] >= DECIDE {
			s.lock_axis = s.lock_travel[0] > s.lock_travel[1] ? 1 : 2
		}
		axis = s.lock_axis != 0 ? s.lock_axis : (ax > ay ? 1 : 2)
	} else {
		along, across := s.lock_recent[s.lock_axis - 1], s.lock_recent[2 - s.lock_axis]
		if across >= SWITCH_MIN && across > along * SWITCH_RATIO do s.lock_axis = 3 - s.lock_axis
		axis = s.lock_axis
	}
	if axis == 1 {
		dy^ = 0
	} else {
		dx^ = 0
	}
}

// One scroll event at `now` seconds, in the library's units (+y up). A
// gesture begins - and picks the scroller the rest go to - on the first
// event, after a pause, when the kind changes, and for a wheel when the
// pointer moved since its last notch. Lines go to kui.input_wheel, whose
// whole notches glide; on macOS a swipe's points (the library's 0.1
// undone) go to kui.input_scroll_gesture, held to their axis.
@(private)
scroll :: proc(s: ^State, dx, dy: f64, now: f64) {
	if dx == 0 && dy == 0 do return
	fresh := s.scroll_kind == 0 || now < s.scroll_at || now - s.scroll_at > GAP
	kind := 1
	when ODIN_OS == .Darwin {
		if dx != f64(i64(dx)) || dy != f64(i64(dy)) || (!fresh && s.scroll_kind == 2) do kind = 2
	}
	if fresh || kind != s.scroll_kind do s.scroll_owed = true
	s.scroll_kind = kind
	s.scroll_at = now
	px, py := f32(dx), f32(dy)
	if kind == 2 {
		u := 10 / unit(s)
		px *= u
		py *= u
		lock(s, &px, &py, now)
		// Kept to zero: the next event dispatched says the gesture begins.
		if px == 0 && py == 0 do return
	} else {
		s.lock_live = false
	}
	begins := s.scroll_owed
	s.scroll_owed = false
	if kind == 2 {
		kui.input_scroll_gesture(s.ui, px, py, begins)
	} else {
		kui.input_wheel(s.ui, px, py, begins)
	}
}
"#;

const ODIN_GENERATED: &str = "Generated by `cargo run -p kui-devtools --bin keytables` from the table in\n// examples/devtools/src/bin/keytables.rs; edit the table, not this file.";

fn glfw_odin(rows: &[Row]) -> String {
    let mut o = String::new();
    let _ = write!(
        o,
        r#"// GLFW's keys and pointer as kui's (backlog F239, F265).
// {ODIN_GENERATED}
//
// A package of its own, so importing kui links no GLFW. A host with no
// GLFW callbacks of its own installs every one, keys and pointer, on
// state it owns (the pointer half, below):
//
//     import kui_glfw "kui/glfw"
//     @(static) input: kui_glfw.State
//     input.ui = ui
//     glfw.SetInputMode(window, glfw.LOCK_KEY_MODS, 1)
//     kui_glfw.install(&input, window)
//
// Keys, by hand:
//
//     glfw.SetInputMode(window, glfw.LOCK_KEY_MODS, 1)
//     glfw.SetKeyCallback(window, proc "c" (w: glfw.WindowHandle, key, scancode, action, mods: c.int) {{
//         context = runtime.default_context()
//         kui_glfw.on_key(ui, key, scancode, action, mods)
//     }})
//     glfw.SetCharCallback(window, proc "c" (w: glfw.WindowHandle, r: rune) {{
//         context = runtime.default_context()
//         kui_glfw.on_char(ui, r)
//     }})
//
// GLFW names a key by its place on a US keyboard (glfw.KEY_*), which is
// kui's `physical`; `code` is what the layout makes of it
// (glfw.GetKeyName), a letter upper-cased under Shift or Caps Lock - which
// GLFW reports only with LOCK_KEY_MODS on. A named key (the arrows, Enter,
// Tab, Escape, F5) goes through kui.input_press, so it means what it
// means; a key that types (a character, Space) goes through
// kui.input_key_down, the raw half a key sink and an editor's chords read,
// and what it types arrives once, from the char callback, as
// kui.input_text. kui.input_modifiers follows every key.
package kui_glfw

import "base:runtime"
import "core:c"
import "core:unicode/utf8"
import "vendor:glfw"
import kui ".."

// kui's name for the key at GLFW `key`'s place and where it sits; "" for
// a key kui names none for (glfw.KEY_WORLD_1, _2).
physical :: proc(key: c.int) -> (name: string, location: kui.Key_Location) {{
	switch key {{
"#
    );
    for r in rows {
        if let Some(g) = r.glfw {
            let _ = writeln!(
                o,
                "\tcase glfw.KEY_{g}: return {}, {}",
                odin_str(r.kui),
                r.loc.odin()
            );
        }
    }
    let _ = write!(
        o,
        r#"	}}
	return "", .Standard
}}

// GLFW's `mods` as kui's.
key_mods :: proc(mods: c.int) -> kui.Key_Mods {{
	return kui.Key_Mods {{
		shift = mods & glfw.MOD_SHIFT != 0,
		ctrl = mods & glfw.MOD_CONTROL != 0,
		alt = mods & glfw.MOD_ALT != 0,
		super = mods & glfw.MOD_SUPER != 0,
		caps_lock = mods & glfw.MOD_CAPS_LOCK != 0,
		num_lock = mods & glfw.MOD_NUM_LOCK != 0,
	}}
}}

// The modifiers `km` holds down, as kui.input_modifiers takes them.
@(private)
held :: proc(km: kui.Key_Mods) -> (m: kui.Mods) {{
	if km.shift do m += {{.Shift}}
	if km.ctrl do m += {{.Ctrl}}
	if km.alt do m += {{.Alt}}
	if km.super do m += {{.Super}}
	return
}}

// A GLFW key callback's arguments, into kui.
on_key :: proc(ui: ^kui.Ui, key, scancode, action, mods: c.int) {{
	phys, location := physical(key)
	if phys == "" do return
	km := key_mods(mods)
	kui.input_modifiers(ui, held(km))
	km.location = location
	types := len(phys) == 1 || phys == "space"
	code := phys
	layout := glfw.GetKeyName(key, scancode)
	if layout != "" && utf8.rune_count_in_string(layout) == 1 {{
		code = layout
		types = true
	}}
	buf: [1]u8
	if km.shift != km.caps_lock && len(code) == 1 && code[0] >= 'a' && code[0] <= 'z' {{
		buf[0] = code[0] - 32
		code = string(buf[:])
	}}
	switch {{
	case action == glfw.RELEASE:
		kui.input_release(ui, code, phys, km)
	case types:
		kui.input_key_down(ui, code, phys, km, "", action == glfw.REPEAT)
	case:
		kui.input_press(ui, code, phys, km, "", action == glfw.REPEAT)
	}}
}}

// A GLFW char callback's rune, into kui as typed text.
on_char :: proc(ui: ^kui.Ui, r: rune) {{
	bytes, n := utf8.encode_rune(r)
	kui.input_text(ui, string(bytes[:n]))
}}
"#
    );
    o.push_str(ODIN_GLFW_POINTER_HEAD);
    o.push_str(ODIN_POINTER_FIELDS);
    o.push_str(ODIN_GLFW_STATE_TAIL);
    o.push_str(ODIN_POINTER);
    o.push_str(ODIN_GLFW_POINTER);
    o
}

/// kui/glfw's pointer half, up to the state's shared fields.
const ODIN_GLFW_POINTER_HEAD: &str = r#"
// -- The pointer (backlog F265) ---------------------------------------------
//
// The cursor, the window's edge, the buttons and the scroll, sent the way
// kui_run sends them, from state the host owns and zeroes: install sets
// every callback (above), or a host with callbacks of its own calls one
// proc in each - on_cursor, on_enter, on_button, on_scroll.
//
// Clicks: GLFW counts none, so a press of the primary button within 0.4 s
// and 4 logical px of the last counts up 1, 2, 3 (a word, a line) and
// round again - kui_run's rule. Every button goes through
// kui.input_mouse_button, GLFW's button n as kui's n.
//
// Scroll: GLFW's deltas are a wheel's lines, to kui.input_wheel - except
// on macOS, where GLFW hands a device with precise deltas (a trackpad) as
// its points scaled by 0.1 and a wheel's whole: there a delta with a
// fraction, or one in a gesture that had one, goes ten times over to
// kui.input_scroll_gesture, held to its axis. A gesture begins, as
// kui_run's do, on the first event, after a pause of 0.2 s, when lines
// turn to points or back, and for a wheel when the pointer moved. GLFW
// reports no touch phase, so a finger put down mid-glide continues the
// glide's gesture until a pause; kui_glfw.h says the rest of what it
// cannot follow.

// What the pointer procs keep. Zero it, set ui, and keep it as long as the
// window sends to it.
State :: struct {
	ui:          ^kui.Ui,
	// GLFW screen coordinates in one of kui's logical px; 0 is 1. They are
	// the same when the frame's size is glfw.GetWindowSize's and its scale
	// the framebuffer's over it - always on macOS and Wayland. On Windows
	// and X11 they are pixels: a host that scales its UI by
	// glfw.GetWindowContentScale sets that scale here.
	coord_scale: f32,
	// The rest is the procs' own.
"#;

/// kui/glfw's state, after the shared fields.
const ODIN_GLFW_STATE_TAIL: &str = r#"	// The last primary press and the count it made.
	click_at:    f64,
	click_x:     f32,
	click_y:     f32,
	clicks:      u32,
	// install's: the window, the callbacks the host set before, and the
	// next installed state.
	window:      glfw.WindowHandle,
	prev_cursor: glfw.CursorPosProc,
	prev_enter:  glfw.CursorEnterProc,
	prev_button: glfw.MouseButtonProc,
	prev_scroll: glfw.ScrollProc,
	prev_key:    glfw.KeyProc,
	prev_char:   glfw.CharProc,
	next:        ^State,
}
"#;

/// kui/glfw's pointer procs and install, after the shared rules.
const ODIN_GLFW_POINTER: &str = r#"
// kui_run's: presses of the primary button within this long (s) and this
// far apart (logical px) count up.
@(private)
CLICK_TIME :: 0.4
@(private)
CLICK_SLOP :: f32(4)

// A cursor-position callback's arguments, into kui.
on_cursor :: proc(s: ^State, x, y: f64) {
	u := f64(unit(s))
	move(s, f32(x / u), f32(y / u))
}

// A cursor-enter callback's argument, into kui: leaving is
// kui.input_cursor_left; entering waits for the cursor's position.
on_enter :: proc(s: ^State, entered: c.int) {
	if entered == 0 do leave(s)
}

// A mouse-button callback's arguments, into kui: the modifiers it carries,
// then the button, a primary press counted.
on_button :: proc(s: ^State, button, action, mods: c.int) {
	if button < 0 do return
	kui.input_modifiers(s.ui, held(key_mods(mods)))
	down := action != glfw.RELEASE
	clicks: u32 = 1
	if down && button == glfw.MOUSE_BUTTON_LEFT {
		now := glfw.GetTime()
		again :=
			s.clicks > 0 &&
			now >= s.click_at &&
			now - s.click_at < CLICK_TIME &&
			abs(s.x - s.click_x) < CLICK_SLOP &&
			abs(s.y - s.click_y) < CLICK_SLOP
		clicks = again ? s.clicks % 3 + 1 : 1
		s.clicks = clicks
		s.click_at = now
		s.click_x = s.x
		s.click_y = s.y
	}
	kui.input_mouse_button(s.ui, down, kui.Mouse(button), clicks)
}

// A scroll callback's arguments, into kui.
on_scroll :: proc(s: ^State, dx, dy: f64) {
	scroll(s, dx, dy, glfw.GetTime())
}

// -- install ------------------------------------------------------------------
//
// Sets the cursor-position, cursor-enter, mouse-button, scroll, key and
// char callbacks on `window`, each calling the one the host set before it
// first (so set yours first), then kui - as Dear ImGui's GLFW backend
// chains. It sets no input mode: LOCK_KEY_MODS is the host's to turn on.
// GLFW's window user pointer stays the host's: a callback finds its state
// by the window in a list threaded through the installed states, the
// table ImGui's backend keeps. uninstall puts the host's callbacks back;
// call it before the state goes.

@(private)
installed: ^State

@(private)
find :: proc(w: glfw.WindowHandle) -> ^State {
	for s := installed; s != nil; s = s.next {
		if s.window == w do return s
	}
	return nil
}

@(private)
unlink :: proc(s: ^State) {
	for at := &installed; at^ != nil; at = &at^.next {
		if at^ == s {
			at^ = s.next
			break
		}
	}
	s.next = nil
	s.window = nil
}

@(private)
cb_cursor :: proc "c" (w: glfw.WindowHandle, x, y: f64) {
	context = runtime.default_context()
	s := find(w)
	if s == nil do return
	if s.prev_cursor != nil do s.prev_cursor(w, x, y)
	on_cursor(s, x, y)
}

@(private)
cb_enter :: proc "c" (w: glfw.WindowHandle, entered: c.int) {
	context = runtime.default_context()
	s := find(w)
	if s == nil do return
	if s.prev_enter != nil do s.prev_enter(w, entered)
	on_enter(s, entered)
}

@(private)
cb_button :: proc "c" (w: glfw.WindowHandle, button, action, mods: c.int) {
	context = runtime.default_context()
	s := find(w)
	if s == nil do return
	if s.prev_button != nil do s.prev_button(w, button, action, mods)
	on_button(s, button, action, mods)
}

@(private)
cb_scroll :: proc "c" (w: glfw.WindowHandle, dx, dy: f64) {
	context = runtime.default_context()
	s := find(w)
	if s == nil do return
	if s.prev_scroll != nil do s.prev_scroll(w, dx, dy)
	on_scroll(s, dx, dy)
}

@(private)
cb_key :: proc "c" (w: glfw.WindowHandle, key, scancode, action, mods: c.int) {
	context = runtime.default_context()
	s := find(w)
	if s == nil do return
	if s.prev_key != nil do s.prev_key(w, key, scancode, action, mods)
	on_key(s.ui, key, scancode, action, mods)
}

@(private)
cb_char :: proc "c" (w: glfw.WindowHandle, r: rune) {
	context = runtime.default_context()
	s := find(w)
	if s == nil do return
	if s.prev_char != nil do s.prev_char(w, r)
	on_char(s.ui, r)
}

// Every callback on `window`, into kui through `s`, chained to the host's.
install :: proc(s: ^State, window: glfw.WindowHandle) {
	if s.window != nil do unlink(s)
	s.window = window
	s.next = installed
	installed = s
	s.prev_cursor = glfw.SetCursorPosCallback(window, cb_cursor)
	s.prev_enter = glfw.SetCursorEnterCallback(window, cb_enter)
	s.prev_button = glfw.SetMouseButtonCallback(window, cb_button)
	s.prev_scroll = glfw.SetScrollCallback(window, cb_scroll)
	s.prev_key = glfw.SetKeyCallback(window, cb_key)
	s.prev_char = glfw.SetCharCallback(window, cb_char)
}

// The host's callbacks back on the window install took, and the window
// forgotten.
uninstall :: proc(s: ^State) {
	w := s.window
	if w == nil do return
	glfw.SetCursorPosCallback(w, s.prev_cursor)
	glfw.SetCursorEnterCallback(w, s.prev_enter)
	glfw.SetMouseButtonCallback(w, s.prev_button)
	glfw.SetScrollCallback(w, s.prev_scroll)
	glfw.SetKeyCallback(w, s.prev_key)
	glfw.SetCharCallback(w, s.prev_char)
	s.prev_cursor = nil
	s.prev_enter = nil
	s.prev_button = nil
	s.prev_scroll = nil
	s.prev_key = nil
	s.prev_char = nil
	unlink(s)
}
"#;

fn sdl3_odin(rows: &[Row]) -> String {
    let mut o = String::new();
    let _ = write!(
        o,
        r#"// SDL3's keys and pointer as kui's (backlog F239, F265).
// {ODIN_GENERATED}
//
// A package of its own, so importing kui links no SDL. Every event it
// knows - keys, text, the pointer and the wheel - in one call, on state
// the host owns (the pointer half, below):
//
//     import kui_sdl3 "kui/sdl3"
//     @(static) input: kui_sdl3.State
//     input.ui = ui
//     sdl.StartTextInput(window)
//     for sdl.PollEvent(&ev) do kui_sdl3.on_event(&input, &ev)
//
// Keys, by hand:
//
//     sdl.StartTextInput(window)
//     #partial switch ev.type {{
//     case .KEY_DOWN, .KEY_UP: kui_sdl3.on_key(ui, &ev.key)
//     case .TEXT_INPUT: kui_sdl3.on_text(ui, &ev.text)
//     }}
//
// SDL names a key by its place on a US keyboard (its scancode), which is
// kui's `physical`; `code` is what the layout makes of it (the event's
// keycode), a letter upper-cased under Shift or Caps Lock. A named key
// (the arrows, Enter, Tab, Escape, F5) goes through kui.input_press, so it
// means what it means; a key that types (a character, Space) goes through
// kui.input_key_down, the raw half a key sink and an editor's chords read,
// and what it types arrives once, from TEXT_INPUT, as kui.input_text.
// kui.input_modifiers follows every key.
package kui_sdl3

import "core:unicode/utf8"
import sdl "vendor:sdl3"
import kui ".."

// kui's name for the key at scancode `sc`'s place and where it sits; ""
// for a key kui names none for.
physical :: proc(sc: sdl.Scancode) -> (name: string, location: kui.Key_Location) {{
	#partial switch sc {{
"#
    );
    for r in rows {
        if let Some(s) = r.sdl {
            let member = if s.starts_with(|c: char| c.is_ascii_digit()) {
                format!("_{s}")
            } else {
                s.to_string()
            };
            let _ = writeln!(
                o,
                "\tcase .{member}: return {}, {}",
                odin_str(r.kui),
                r.loc.odin()
            );
        }
    }
    let _ = write!(
        o,
        r#"	}}
	return "", .Standard
}}

// SDL's modifier state as kui's.
key_mods :: proc(mod: sdl.Keymod) -> kui.Key_Mods {{
	return kui.Key_Mods {{
		shift = mod & sdl.KMOD_SHIFT != {{}},
		ctrl = mod & sdl.KMOD_CTRL != {{}},
		alt = mod & sdl.KMOD_ALT != {{}},
		super = mod & sdl.KMOD_GUI != {{}},
		caps_lock = .CAPS in mod,
		num_lock = .NUM in mod,
	}}
}}

// An SDL KEY_DOWN or KEY_UP, into kui.
on_key :: proc(ui: ^kui.Ui, e: ^sdl.KeyboardEvent) {{
	phys, location := physical(e.scancode)
	if phys == "" do return
	km := key_mods(e.mod)
	held: kui.Mods
	if km.shift do held += {{.Shift}}
	if km.ctrl do held += {{.Ctrl}}
	if km.alt do held += {{.Alt}}
	if km.super do held += {{.Super}}
	kui.input_modifiers(ui, held)
	km.location = location
	types := len(phys) == 1 || phys == "space"
	code := phys
	buf: [4]u8
	k := u32(e.key)
	if k & u32(sdl.K_SCANCODE_MASK) == 0 && k > 0x20 && k != 0x7f && k < 0x110000 {{
		// The layout's character.
		bytes, n := utf8.encode_rune(rune(k))
		buf = bytes
		code = string(buf[:n])
		types = true
	}}
	if km.shift != km.caps_lock && len(code) == 1 && code[0] >= 'a' && code[0] <= 'z' {{
		buf[0] = code[0] - 32
		code = string(buf[:1])
	}}
	switch {{
	case !e.down:
		kui.input_release(ui, code, phys, km)
	case types:
		kui.input_key_down(ui, code, phys, km, "", e.repeat)
	case:
		kui.input_press(ui, code, phys, km, "", e.repeat)
	}}
}}

// An SDL TEXT_INPUT, into kui as typed text.
on_text :: proc(ui: ^kui.Ui, e: ^sdl.TextInputEvent) {{
	if e.text != nil do kui.input_text(ui, string(e.text))
}}
"#
    );
    o.push_str(ODIN_SDL3_POINTER_HEAD);
    o.push_str(ODIN_POINTER_FIELDS);
    o.push_str("}\n");
    o.push_str(ODIN_POINTER);
    o.push_str(ODIN_SDL3_POINTER);
    o
}

/// kui/sdl3's pointer half, up to the state's shared fields.
const ODIN_SDL3_POINTER_HEAD: &str = r#"
// -- The pointer (backlog F265) ---------------------------------------------
//
// The cursor, the window's edge, the buttons and the wheel, sent the way
// kui_run sends them, from state the host owns and zeroes: on_event
// (above) forwards each, or a host handling some itself calls on_cursor,
// on_enter, on_button and on_scroll.
//
// Clicks: SDL counts them (the event's `clicks`), and kui takes 1, 2, 3 (a
// word, a line) and round again, as kui_run counts; only the primary
// button's count. Every button goes through kui.input_mouse_button: SDL's
// left, right and middle are kui's primary, secondary and middle, X1 and
// X2 kui's .Other and the one after it.
//
// Scroll: SDL's deltas are a wheel's lines, to kui.input_wheel - except on
// macOS, where SDL hands a device with precise deltas (a trackpad) as its
// points scaled by 0.1 and a wheel's whole: there a delta with a fraction,
// or one in a gesture that had one, goes ten times over to
// kui.input_scroll_gesture, held to its axis. A gesture begins, as
// kui_run's do, on the first event, after a pause of 0.2 s by the events'
// timestamps, when lines turn to points or back, and for a wheel when the
// pointer moved. SDL's x is positive to the right and kui's to the left,
// so it is turned. SDL reports no touch phase, so a finger put down
// mid-glide continues the glide's gesture until a pause; kui_sdl3.h says
// the rest of what it cannot follow.

// What the pointer procs keep. Zero it, set ui, and keep it as long as
// events come for it.
State :: struct {
	ui:          ^kui.Ui,
	// SDL's window coordinates in one of kui's logical px; 0 is 1. They are
	// the same when the frame's size is sdl.GetWindowSize's and its scale
	// sdl.GetWindowPixelDensity's - always on macOS and Wayland. On Windows
	// and X11 they are pixels: a host that scales its UI by
	// sdl.GetWindowDisplayScale sets that scale here.
	coord_scale: f32,
	// The window whose events on_event forwards; 0 is every one.
	window:      sdl.WindowID,
	// The rest is the procs' own.
"#;

/// kui/sdl3's pointer procs and on_event, after the shared rules.
const ODIN_SDL3_POINTER: &str = r#"
// A MOUSE_MOTION, into kui.
on_cursor :: proc(s: ^State, e: ^sdl.MouseMotionEvent) {
	u := unit(s)
	move(s, e.x / u, e.y / u)
}

// WINDOW_MOUSE_ENTER (true) or _LEAVE (false), into kui: leaving is
// kui.input_cursor_left; entering waits for the cursor's position.
on_enter :: proc(s: ^State, entered: bool) {
	if !entered do leave(s)
}

// A MOUSE_BUTTON_DOWN or _UP, into kui: the pointer where the event says
// it is, then the button.
on_button :: proc(s: ^State, e: ^sdl.MouseButtonEvent) {
	if e.button == 0 do return
	u := unit(s)
	x, y := e.x / u, e.y / u
	if x != s.x || y != s.y do move(s, x, y)
	button: kui.Mouse
	switch e.button {
	case sdl.BUTTON_LEFT:
		button = .Primary
	case sdl.BUTTON_RIGHT:
		button = .Secondary
	case sdl.BUTTON_MIDDLE:
		button = .Middle
	case:
		button = kui.Mouse(e.button - 1)
	}
	clicks: u32 = 1
	if button == .Primary && e.clicks > 0 do clicks = u32(e.clicks - 1) % 3 + 1
	kui.input_mouse_button(s.ui, e.down, button, clicks)
}

// A MOUSE_WHEEL, into kui.
on_scroll :: proc(s: ^State, e: ^sdl.MouseWheelEvent) {
	scroll(s, -f64(e.x), f64(e.y), f64(e.timestamp) / 1e9)
}

@(private)
ours :: proc(s: ^State, window: sdl.WindowID) -> bool {
	return s.window == 0 || window == s.window
}

// Every event kind above, into kui: keys, text, the pointer, the wheel and
// the pointer leaving the window. True when it was one of those, for this
// state's window - not whether kui wanted it, which kui.pointer_wants and
// kui.keyboard_wants say.
on_event :: proc(s: ^State, e: ^sdl.Event) -> bool {
	#partial switch e.type {
	case .KEY_DOWN, .KEY_UP:
		if !ours(s, e.key.windowID) do return false
		on_key(s.ui, &e.key)
	case .TEXT_INPUT:
		if !ours(s, e.text.windowID) do return false
		on_text(s.ui, &e.text)
	case .MOUSE_MOTION:
		if !ours(s, e.motion.windowID) do return false
		on_cursor(s, &e.motion)
	case .MOUSE_BUTTON_DOWN, .MOUSE_BUTTON_UP:
		if !ours(s, e.button.windowID) do return false
		on_button(s, &e.button)
	case .MOUSE_WHEEL:
		if !ours(s, e.wheel.windowID) do return false
		on_scroll(s, &e.wheel)
	case .WINDOW_MOUSE_ENTER, .WINDOW_MOUSE_LEAVE:
		if !ours(s, e.window.windowID) do return false
		on_enter(s, e.type == .WINDOW_MOUSE_ENTER)
	case:
		return false
	}
	return true
}
"#;

fn main() {
    let check = std::env::args().any(|a| a == "--check");
    let rows = rows();
    if let Err(e) = check_names(&rows) {
        eprintln!("keytables: {e}");
        std::process::exit(1);
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let files = [
        ("crates/kui-ffi/include/kui_glfw.h", glfw_h(&rows)),
        ("crates/kui-ffi/include/kui_sdl3.h", sdl3_h(&rows)),
        ("packages/odin/kui/glfw/glfw.odin", glfw_odin(&rows)),
        ("packages/odin/kui/sdl3/sdl3.odin", sdl3_odin(&rows)),
    ];
    let mut stale = Vec::new();
    for (rel, text) in &files {
        let path = root.join(rel);
        let now = std::fs::read_to_string(&path).unwrap_or_default();
        if now == *text {
            continue;
        }
        if check {
            stale.push(*rel);
            continue;
        }
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).expect("the directory");
        }
        std::fs::write(&path, text).expect("written");
        println!("wrote {rel}");
    }
    if !stale.is_empty() {
        eprintln!(
            "keytables: stale: {} (run `cargo run -p kui-devtools --bin keytables`)",
            stale.join(", ")
        );
        std::process::exit(1);
    }
    println!(
        "keytables: {} rows, {} GLFW keys, {} SDL3 scancodes",
        rows.len(),
        rows.iter().filter(|r| r.glfw.is_some()).count(),
        rows.iter().filter(|r| r.sdl.is_some()).count()
    );
}
