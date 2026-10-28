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

const GENERATED: &str = "Generated by `cargo run -p kui-devtools --bin keytables` from the table in\n * examples/devtools/src/bin/keytables.rs; edit the table, not this file.";

fn glfw_h(rows: &[Row]) -> String {
    let mut o = String::new();
    let _ = write!(
        o,
        r#"/* kui_glfw.h - GLFW's keys as kui's (backlog F239). {GENERATED}
 *
 * Include after <GLFW/glfw3.h> and "kui.h". Header-only: libkui_ffi knows
 * no window library, so nothing here is in it.
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
        kui_input_release(ctx, KUI_STR(code), KUI_STR(physical), kmods);
    }} else if (types) {{
        kui_input_key_down(ctx, KUI_STR(code), KUI_STR(physical), kmods, none, action == GLFW_REPEAT);
    }} else {{
        kui_input_press(ctx, KUI_STR(code), KUI_STR(physical), kmods, none, action == GLFW_REPEAT);
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
    if (n) kui_input_text(ctx, (KuiStr){{(const uint8_t *)s, n}});
}}

#endif /* KUI_GLFW_H */
"#
    );
    o
}

fn sdl3_h(rows: &[Row]) -> String {
    let mut o = String::new();
    let _ = write!(
        o,
        r#"/* kui_sdl3.h - SDL3's keys as kui's (backlog F239). {GENERATED}
 *
 * Include after <SDL3/SDL.h> and "kui.h". Header-only: libkui_ffi knows
 * no window library, so nothing here is in it, and nothing here calls SDL.
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
        kui_input_release(ctx, KUI_STR(code), KUI_STR(physical), kmods);
    }} else if (types) {{
        kui_input_key_down(ctx, KUI_STR(code), KUI_STR(physical), kmods, none, e->repeat);
    }} else {{
        kui_input_press(ctx, KUI_STR(code), KUI_STR(physical), kmods, none, e->repeat);
    }}
}}

/* An SDL_EVENT_TEXT_INPUT, into kui as typed text. */
static inline void kui_sdl3_text(KuiCtx *ctx, const SDL_TextInputEvent *e) {{
    if (e->text && e->text[0]) kui_input_text(ctx, KUI_STR(e->text));
}}

#endif /* KUI_SDL3_H */
"#
    );
    o
}

/// An Odin string literal.
fn odin_str(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\"))
}

const ODIN_GENERATED: &str = "Generated by `cargo run -p kui-devtools --bin keytables` from the table in\n// examples/devtools/src/bin/keytables.rs; edit the table, not this file.";

fn glfw_odin(rows: &[Row]) -> String {
    let mut o = String::new();
    let _ = write!(
        o,
        r#"// GLFW's keys as kui's (backlog F239). {ODIN_GENERATED}
//
// A package of its own, so importing kui links no GLFW:
//
//     import kui_glfw "kui/glfw"
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

// A GLFW key callback's arguments, into kui.
on_key :: proc(ui: ^kui.Ui, key, scancode, action, mods: c.int) {{
	phys, location := physical(key)
	if phys == "" do return
	km := key_mods(mods)
	held: kui.Mods
	if km.shift do held += {{.Shift}}
	if km.ctrl do held += {{.Ctrl}}
	if km.alt do held += {{.Alt}}
	if km.super do held += {{.Super}}
	kui.input_modifiers(ui, held)
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
    o
}

fn sdl3_odin(rows: &[Row]) -> String {
    let mut o = String::new();
    let _ = write!(
        o,
        r#"// SDL3's keys as kui's (backlog F239). {ODIN_GENERATED}
//
// A package of its own, so importing kui links no SDL:
//
//     import kui_sdl3 "kui/sdl3"
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
    o
}

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
