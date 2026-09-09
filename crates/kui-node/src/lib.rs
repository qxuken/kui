//! Node.js addon for kui (napi-rs). Like kui-ffi this layer is translation,
//! not architecture — but where C makes flat builder calls, JS submits a whole
//! frame at once: the jsx-runtime produces a plain-data element tree
//! (`{type, key, props, children}`), the JS package encodes it into the flat
//! binary IR stream ([`binary`]), and [`Ctx::frame_binary`] lowers it in one
//! zero-copy boundary crossing. That is the only way a frame gets in: there
//! is one element dispatcher ([`binary`]'s), not one per transport.
//! Event payloads are plain JSON both ways, which is exactly the Elm shape:
//! `onClick` carries a message value, never a closure.

use kui_ffi::CExtension;

use kui_core::{
    Appearance, AudioCommand, AudioSpec, Color, Core, EditKey, FontId, FrameSample, FrameStats,
    ImageId, InputEvent, Key, KeyCode, KeyMods, KeyPress, Locale, Mods, MotionPref, MouseButton,
    PlayOptions, PlaybackId, Rect, Size, SoundId, Span, SystemEnv, UiEvent, Value, Vec2,
    schema::color_hex_str,
};
use napi::bindgen_prelude::{Buffer, Float64Array, Uint8Array};
use napi_derive::napi;
use serde_json::{Map as JsonMap, Value as Json};

mod binary;
mod schema;

use schema::{color_of, parse_props_json};

type Result<T> = napi::Result<T>;

fn err(msg: impl AsRef<str>) -> napi::Error {
    napi::Error::from_reason(msg.as_ref())
}

/// The binary-frame protocol tables (`{version, op, prop}`). The JS encoder
/// reads its opcodes and prop ids from here at module init, so the two sides
/// cannot drift.
#[napi(ts_return_type = "Protocol")]
pub fn protocol() -> Json {
    binary::protocol_json()
}

// ---------------------------------------------------------------------------
// JSON <-> Value

fn value_of(v: &Json) -> Value {
    match v {
        Json::Null => Value::Null,
        Json::Bool(b) => Value::Bool(*b),
        Json::Number(n) => match n.as_i64() {
            Some(i) => Value::Int(i),
            None => Value::Float(n.as_f64().unwrap_or(0.0)),
        },
        Json::String(s) => Value::Str(s.clone()),
        Json::Array(a) => Value::List(a.iter().map(value_of).collect()),
        Json::Object(m) => Value::Map(m.iter().map(|(k, v)| (k.clone(), value_of(v))).collect()),
    }
}

fn json_of(v: &Value) -> Json {
    match v {
        Value::Null => Json::Null,
        Value::Bool(b) => Json::Bool(*b),
        Value::Int(i) => Json::from(*i),
        Value::Float(f) => serde_json::Number::from_f64(*f)
            .map(Json::Number)
            .unwrap_or(Json::Null),
        Value::Str(s) => Json::String(s.clone()),
        Value::List(items) => Json::Array(items.iter().map(json_of).collect()),
        Value::Map(entries) => Json::Object(
            entries
                .iter()
                .map(|(k, v)| (k.clone(), json_of(v)))
                .collect(),
        ),
    }
}

// ---------------------------------------------------------------------------
// Plain-object arguments
//
// Frames arrive as the binary IR stream and never as objects, but the
// imperative calls beside them still take plain JS values: `measureText`
// takes the same `<text>` content and style props a view would declare, and
// `play` takes an options bag. These helpers read those.

fn bool_prop(props: &JsonMap<String, Json>, key: &str) -> bool {
    props.get(key).and_then(Json::as_bool).unwrap_or(false)
}

fn empty_props() -> &'static JsonMap<String, Json> {
    static EMPTY: std::sync::OnceLock<JsonMap<String, Json>> = std::sync::OnceLock::new();
    EMPTY.get_or_init(JsonMap::new)
}

fn parts(node: &JsonMap<String, Json>) -> (&JsonMap<String, Json>, Option<&Json>, Option<&str>) {
    let props = node
        .get("props")
        .and_then(Json::as_object)
        .unwrap_or(empty_props());
    (
        props,
        node.get("children"),
        node.get("key").and_then(Json::as_str),
    )
}

/// Concatenated string content of a subtree (for <text>/<button> labels).
fn collect_text(node: Option<&Json>, out: &mut String) {
    match node {
        None | Some(Json::Null) | Some(Json::Bool(_)) => {}
        Some(Json::String(s)) => out.push_str(s),
        Some(Json::Number(n)) => out.push_str(&n.to_string()),
        Some(Json::Array(items)) => {
            for item in items {
                collect_text(Some(item), out);
            }
        }
        Some(Json::Object(o)) => collect_text(o.get("children"), out),
    }
}

#[derive(Clone, Copy, Default)]
struct SpanStyle {
    color: Option<Color>,
    bold: bool,
    italic: bool,
    underline: bool,
    strikethrough: bool,
    bg: Option<Color>,
}

struct SpanPart {
    text: String,
    style: SpanStyle,
}

fn span_of(p: &SpanPart) -> Span<'_> {
    let mut s = Span::new(&p.text);
    if p.style.bold {
        s = s.bold();
    }
    if p.style.italic {
        s = s.italic();
    }
    if let Some(c) = p.style.color {
        s = s.color(c);
    }
    if p.style.underline {
        s = s.underline();
    }
    if p.style.strikethrough {
        s = s.strikethrough();
    }
    if let Some(c) = p.style.bg {
        s = s.bg(c);
    }
    s
}

fn has_span(node: Option<&Json>) -> bool {
    match node {
        Some(Json::Array(items)) => items.iter().any(|i| has_span(Some(i))),
        Some(Json::Object(o)) => o.get("type").and_then(Json::as_str) == Some("span"),
        _ => false,
    }
}

/// Flattens `<text>` children into styled span parts; `<span>` nests and
/// inherits (bold/italic accumulate, color overrides).
fn collect_spans(node: Option<&Json>, inherit: SpanStyle, out: &mut Vec<SpanPart>) -> Result<()> {
    match node {
        None | Some(Json::Null) | Some(Json::Bool(_)) => Ok(()),
        Some(Json::String(s)) => {
            out.push(SpanPart {
                text: s.clone(),
                style: inherit,
            });
            Ok(())
        }
        Some(Json::Number(n)) => {
            out.push(SpanPart {
                text: n.to_string(),
                style: inherit,
            });
            Ok(())
        }
        Some(Json::Array(items)) => {
            for item in items {
                collect_spans(Some(item), inherit, out)?;
            }
            Ok(())
        }
        Some(Json::Object(o)) => {
            if o.get("type").and_then(Json::as_str) != Some("span") {
                return Err(err("only strings and <span> may nest inside rich <text>"));
            }
            let (props, children, _) = parts(o);
            let style = SpanStyle {
                color: match props.get("color") {
                    Some(v) => Some(color_of(v)?),
                    None => inherit.color,
                },
                bold: inherit.bold || bool_prop(props, "bold"),
                italic: inherit.italic || bool_prop(props, "italic"),
                underline: inherit.underline || bool_prop(props, "underline"),
                strikethrough: inherit.strikethrough || bool_prop(props, "strikethrough"),
                bg: match props.get("bg") {
                    Some(v) => Some(color_of(v)?),
                    None => inherit.bg,
                },
            };
            collect_spans(children, style, out)
        }
    }
}

// ---------------------------------------------------------------------------
// Context

pub(crate) fn parse_u64(s: &str) -> Result<u64> {
    let hex = s.strip_prefix("0x").unwrap_or(s);
    u64::from_str_radix(hex, 16).map_err(|_| err(format!("bad id {s:?}")))
}

fn parse_key(key: &str) -> Result<Key> {
    parse_u64(key).map(Key)
}

/// A key in the form events carry it: sixteen hex digits, `0x` or not.
/// Anything else is a label (see `resolve_key`).
fn hex_key(s: &str) -> Option<Key> {
    let hex = s.strip_prefix("0x").unwrap_or(s);
    (hex.len() == 16 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| u64::from_str_radix(hex, 16).ok().map(Key))
        .flatten()
}

/// The two spellings of a node the app can hand `focus` / `isFocused` /
/// `reveal` / `access`: the hex key an event carried, or the label its
/// `key` prop declared — resolved through the last frame (`Core::key_of`),
/// so a node never interacted with can be named. An unknown label is an
/// error naming both, since "bad id" named neither (backlog F5).
fn resolve_key(core: &mut kui_core::Core, s: &str) -> Result<Key> {
    if let Some(k) = hex_key(s) {
        return Ok(k);
    }
    core.key_of(s).ok_or_else(|| {
        err(format!(
            "no node is keyed {s:?}: pass the label a `key` prop declared in the last frame, or the \
             16-digit hex key an event carried"
        ))
    })
}

fn keycode_of(s: &str) -> Result<KeyCode> {
    KeyCode::from_name(s).ok_or_else(|| err(format!("unknown key code {s:?}")))
}

fn key_str(key: Key) -> String {
    format!("{:016x}", key.0)
}

/// A scroll offset as `{x, y}` — the shape `setScroll` takes back.
fn geometry_json(g: Option<kui_core::ScrollGeometry>) -> Option<Json> {
    let g = g?;
    let mut o = JsonMap::new();
    o.insert("x".into(), Json::from(g.rect.x as f64));
    o.insert("y".into(), Json::from(g.rect.y as f64));
    o.insert("w".into(), Json::from(g.rect.w as f64));
    o.insert("h".into(), Json::from(g.rect.h as f64));
    o.insert("contentW".into(), Json::from(g.content.w as f64));
    o.insert("contentH".into(), Json::from(g.content.h as f64));
    o.insert("offset".into(), offset_json(g.offset));
    o.insert("maxOffset".into(), offset_json(g.max_offset));
    Some(Json::Object(o))
}

fn offset_json(off: Vec2) -> Json {
    let mut o = JsonMap::new();
    o.insert("x".into(), Json::from(off.x as f64));
    o.insert("y".into(), Json::from(off.y as f64));
    Json::Object(o)
}

/// One boolean out of a `setEnv` bag, named so the error says which.
fn flag(name: &str, v: &Json) -> Result<bool> {
    v.as_bool()
        .ok_or_else(|| err(format!("setEnv(): window.{name} must be a boolean")))
}

/// `setEnv`'s `window.nativeControls`: a `{x?, y?, w, h}` rect, or null for
/// "the OS draws nothing over our content". `x` / `y` default to the window
/// origin, which is where the one real instance (the macOS traffic lights)
/// sits; a zero-sized rect is the same as null, matching `kui_env_set_window`.
fn native_controls(v: &Json) -> Result<Option<Rect>> {
    if v.is_null() {
        return Ok(None);
    }
    let o = v
        .as_object()
        .ok_or_else(|| err("setEnv(): window.nativeControls must be an object or null"))?;
    let num = |key: &str| -> Result<f32> {
        match o.get(key) {
            None => Ok(0.0),
            Some(n) => Ok(n
                .as_f64()
                .ok_or_else(|| err(format!("setEnv(): nativeControls.{key} must be a number")))?
                as f32),
        }
    };
    let (x, y, w, h) = (num("x")?, num("y")?, num("w")?, num("h")?);
    Ok((w > 0.0 && h > 0.0).then(|| Rect::new(x, y, w, h)))
}

/// `setEnv`'s `system`: the four OS settings, each with an explicit "the
/// host cannot tell" — `"unknown"` for the two enums, `null` for the two
/// values. Applied field by field, so a host that learns one of them says
/// only that one.
fn system_env(sys: &mut SystemEnv, o: &JsonMap<String, Json>) -> Result<()> {
    let name = |key: &str, v: &Json| -> Result<String> {
        v.as_str()
            .map(str::to_string)
            .ok_or_else(|| err(format!("setEnv(): system.{key} must be a string")))
    };
    for (key, v) in o {
        match key.as_str() {
            "appearance" => {
                sys.appearance = Appearance::parse(&name(key, v)?).ok_or_else(|| {
                    err(format!(
                        "setEnv(): system.appearance is one of {:?}",
                        kui_core::schema::APPEARANCES
                    ))
                })?
            }
            "motion" => {
                sys.motion = MotionPref::parse(&name(key, v)?).ok_or_else(|| {
                    err(format!(
                        "setEnv(): system.motion is one of {:?}",
                        kui_core::schema::MOTIONS
                    ))
                })?
            }
            // A colour the way a prop takes one — `0xRRGGBBAA` or
            // `"#rrggbb"` — so what `env()` hands back and what a view
            // paints with are the same value. Null is "cannot tell", and so
            // is a fully transparent colour whichever way it was spelled:
            // an accent nobody can see is not an accent, and zero is what a
            // C host passes for "I was not told".
            "accent" => {
                sys.accent = match v {
                    Json::Null => None,
                    Json::String(s) => Some(
                        color_hex_str(s)
                            .map_err(|e| err(format!("setEnv(): system.accent: {e}")))?,
                    ),
                    _ => v
                        .as_u64()
                        .and_then(|n| u32::try_from(n).ok())
                        .map(Color::hex)
                        .ok_or_else(|| {
                            err("setEnv(): system.accent must be 0xRRGGBBAA, \"#rrggbb\" or null")
                        })
                        .map(Some)?,
                }
                .filter(|c| c.is_visible())
            }
            // A tag that does not fit is not a tag: say so rather than
            // storing a truncated one, since a test that silently lost its
            // locale would read as the host not knowing.
            "locale" => {
                sys.locale = match v {
                    Json::Null => None,
                    _ => {
                        let tag = name(key, v)?;
                        Some(Locale::new(&tag).ok_or_else(|| {
                            err(format!(
                                "setEnv(): system.locale must be an ASCII BCP-47 tag of at most {} bytes, or null",
                                Locale::CAP
                            ))
                        })?)
                    }
                }
            }
            _ => return Err(err(format!("setEnv(): unknown system key {key:?}"))),
        }
    }
    Ok(())
}

fn edit_key_of(name: &str) -> Result<EditKey> {
    Ok(match name {
        "left" => EditKey::Left,
        "right" => EditKey::Right,
        "up" => EditKey::Up,
        "down" => EditKey::Down,
        "home" => EditKey::Home,
        "end" => EditKey::End,
        "pageup" => EditKey::PageUp,
        "pagedown" => EditKey::PageDown,
        "backspace" => EditKey::Backspace,
        "delete" => EditKey::Delete,
        "enter" => EditKey::Enter,
        "tab" => EditKey::Tab,
        "selectall" => EditKey::SelectAll,
        "escape" => EditKey::Escape,
        "undo" => EditKey::Undo,
        "redo" => EditKey::Redo,
        other => return Err(err(format!("unknown key {other:?}"))),
    })
}

/// A headless kui core: build frames from JSX trees, feed input, poll events.
/// Everything a window does except open one, so an app's behaviour is
/// testable without a display.
#[napi]
pub struct Ctx {
    core: Core,
    events: Vec<UiEvent>,
    /// The extensions this context hosts, in origin order (ADR 0014):
    /// what `addExtension` loads, what a `<slot>` fills from, and what an
    /// event whose origin is not the host's is delivered to.
    extensions: kui_core::Extensions,
}

#[napi]
impl Ctx {
    #[napi(constructor)]
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Ctx {
            core: Core::new(),
            events: Vec::new(),
            extensions: Default::default(),
        }
    }

    /// `frame` from an already-encoded binary instruction stream, for
    /// callers that own their encoder (`createEncoder(protocol())`) — the
    /// fastest path, and what the JS drivers use. Buffers are read
    /// zero-copy.
    #[napi]
    pub fn frame_binary(
        &mut self,
        width: f64,
        height: f64,
        scale: f64,
        stream: Float64Array,
        strings: Uint8Array,
    ) -> Result<()> {
        let scale = if scale > 0.0 { scale } else { 1.0 };
        // `frame_with`, not `frame`: the extension list is what fills a
        // `<slot>` the view declares, and `Ui::finish` is what lets each
        // extension take the `ns/root` the view did not declare and warn
        // about a slot nothing did (ADR 0014). Two disjoint fields, so the
        // core can be borrowed for the frame and the list for the filling.
        let mut ui = self.core.frame_with(
            Size::new(width as f32, height as f32),
            scale as f32,
            &mut self.extensions,
        );
        let result = binary::lower_binary(&mut ui, &stream, &strings);
        ui.finish();
        let pending = self.core.take_pending_events();
        absorb(&mut self.extensions, pending, &mut self.events);
        result
    }

    /// Loads a C extension: a shared library exporting the seven
    /// `kui_ext_*` entry points `crates/kui-ffi/include/kui.h` describes
    /// (ADR 0014). `namespace` is the word that fronts every slot name it
    /// fills — `<slot name="todos/panel"/>` for `addExtension('todos', …)`
    /// — and an empty one takes the plugin's own `kui_ext_name`. Throws
    /// with the reason if the library will not load, declares no
    /// `kui_ext_abi` or one this build does not implement, has no
    /// `kui_ext_view`, or wants a namespace another extension has.
    ///
    /// Load before the first frame: origins are positions in the list, so
    /// one added later renumbers the ones after it. The context owns it and
    /// unloads it when the context goes.
    ///
    /// **A plugin runs in this process**, on this thread, on this app's
    /// frame — loading one is trusting it as much as linking it would be.
    /// Only C shared libraries: there is no script-loads-script path here,
    /// and a Lua extension is loaded by a Rust host or not at all.
    #[napi]
    pub fn add_extension(&mut self, namespace: String, path: String) -> Result<()> {
        load_extension(&mut self.extensions, namespace, path)
    }

    /// The namespaces of the loaded extensions, in origin order: index `i`
    /// is origin `i + 1`, and origin 0 is the app's own nodes. What turns
    /// the `origin` on an event into the name this app gave the plugin.
    #[napi]
    pub fn extension_namespaces(&self) -> Vec<String> {
        self.extensions
            .iter()
            .map(|(ns, _)| ns.to_string())
            .collect()
    }
    /// The frame clock for `transition` props: monotonic seconds, any
    /// origin. Set before each frame; never setting it makes transitions
    /// snap. A bare `Ctx` is the only place to call it: `createApp`'s loop
    /// owns the clock, stamps it before every frame it draws, and replaces
    /// this method on its surface with one that throws — `app.advance(ms)`
    /// is what moves time there.
    #[napi]
    pub fn set_time(&mut self, now_secs: f64) {
        self.core.set_time(now_secs);
    }

    /// Declares host facts a real window would have pushed — what C spells
    /// `kui_env_set` + `kui_env_set_window`, in one call shaped like what
    /// `env()` reads back. Only the keys you pass move; the rest keep their
    /// values, so `setEnv({window: {customChrome: true}})` is the whole of
    /// "pretend this app draws its own titlebar" and `<titlebar>`,
    /// `<windowButtons>` and `widgets::window_buttons` start building
    /// something. `refreshHz: null` means "the host cannot tell" (the
    /// default), and `nativeControls: null` means the OS draws nothing over
    /// our content. `window.id` is the one fact here an app never chooses —
    /// a driver assigns it — and setting it is how a headless test says
    /// "these events came from that window"; it is 0 otherwise.
    ///
    /// Headless only, and on purpose: a `KuiWindow` has no such call because
    /// its runner reports the real window every frame, and anything set here
    /// would be overwritten before the next view ran.
    #[napi(ts_args_type = "env: EnvInput")]
    pub fn set_env(&mut self, env: Json) -> Result<()> {
        let o = env
            .as_object()
            .ok_or_else(|| err("setEnv() takes an object"))?;
        for name in o.keys() {
            if !matches!(name.as_str(), "refreshHz" | "focused" | "system" | "window") {
                return Err(err(format!("setEnv(): unknown key {name:?}")));
            }
        }
        if let Some(hz) = o.get("refreshHz") {
            // A number or an explicit null; anything else is a typo worth
            // hearing about, since a silently ignored rate looks like the
            // 120 Hz fallback.
            self.core.env.refresh_hz = match hz {
                Json::Null => None,
                _ => Some(
                    hz.as_f64()
                        .ok_or_else(|| err("setEnv(): refreshHz must be a number or null"))?
                        as f32,
                ),
            }
            .filter(|hz| *hz > 0.0);
        }
        if let Some(f) = o.get("focused") {
            self.core.env.focused = f
                .as_bool()
                .ok_or_else(|| err("setEnv(): focused must be a boolean"))?;
        }
        if let Some(s) = o.get("system") {
            let s = s
                .as_object()
                .ok_or_else(|| err("setEnv(): system must be an object"))?;
            system_env(&mut self.core.env.system, s)?;
        }
        let Some(w) = o.get("window") else {
            return Ok(());
        };
        let w = w
            .as_object()
            .ok_or_else(|| err("setEnv(): window must be an object"))?;
        let win = &mut self.core.env.window;
        for (name, v) in w {
            match name.as_str() {
                "id" => {
                    win.id = kui_core::WindowId(
                        v.as_u64()
                            .and_then(|n| u32::try_from(n).ok())
                            .ok_or_else(|| err("setEnv(): window.id must be a u32"))?,
                    )
                }
                "customChrome" => win.custom_chrome = flag(name, v)?,
                "maximized" => win.maximized = flag(name, v)?,
                "fullscreen" => win.fullscreen = flag(name, v)?,
                "nativeControls" => win.native_controls = native_controls(v)?,
                _ => return Err(err(format!("setEnv(): unknown window key {name:?}"))),
            }
        }
        Ok(())
    }

    // -- Input (logical coordinates) ------------------------------------

    fn input(&mut self, ev: InputEvent) {
        let evs = self.core.handle_input(ev);
        self.take_events(evs);
    }

    #[napi]
    pub fn cursor(&mut self, x: f64, y: f64) {
        self.input(InputEvent::CursorMoved(Vec2::new(x as f32, y as f32)));
    }

    #[napi]
    pub fn cursor_left(&mut self) {
        self.input(InputEvent::CursorLeft);
    }

    /// A button press or release. `clicks`: 1 single, 2 double (word
    /// select), 3 triple (line select). `button` defaults to "primary", and
    /// only that one presses, drags, places the caret and clicks;
    /// "secondary" asks the node under the pointer for a context menu and
    /// moves nothing else, and nothing routes "middle" yet.
    #[napi(ts_args_type = "down: boolean, clicks?: number, button?: MouseButtonName")]
    pub fn mouse(&mut self, down: bool, clicks: Option<u32>, button: Option<String>) -> Result<()> {
        let button = match &button {
            Some(name) => MouseButton::from_name(name)
                .ok_or_else(|| err(format!("unknown mouse button {name:?}")))?,
            None => MouseButton::Primary,
        };
        self.input(if down {
            InputEvent::MouseDown {
                button,
                clicks: clicks.unwrap_or(1).clamp(1, 255) as u8,
            }
        } else {
            InputEvent::MouseUp { button }
        });
        Ok(())
    }

    #[napi]
    pub fn scroll(&mut self, dx: f64, dy: f64) {
        self.input(InputEvent::Scroll(Vec2::new(dx as f32, dy as f32)));
    }

    /// Committed text input (typing, paste); routed to the focused editor.
    #[napi]
    pub fn text(&mut self, text: String) {
        self.input(InputEvent::Text(text));
    }

    /// Text an IME committed at the end of a composition: a focused
    /// `<edit>` takes it, otherwise the focused `onKey` sink hears it as
    /// `{kind:"text", text, tag}` — the one committed text a `key` event
    /// never carries. `type`/`press` stay the way to type.
    #[napi]
    pub fn commit(&mut self, text: String) {
        self.input(InputEvent::Commit(text));
    }

    /// An in-progress IME composition: `text` is the uncommitted string
    /// (empty ends the composition without a commit), `cursor` the byte
    /// range inside it the IME's caret covers, or null. A focused `<edit>`
    /// shows it inline; otherwise the focused `onKey` sink hears
    /// `{kind:"preedit", text, cursor, tag}`.
    #[napi(ts_args_type = "text: string, cursor?: [number, number] | null")]
    pub fn preedit(&mut self, text: String, cursor: Option<Vec<u32>>) {
        let cursor = cursor
            .filter(|c| c.len() == 2)
            .map(|c| (c[0] as usize, c[1] as usize));
        self.input(InputEvent::Preedit(text, cursor));
    }

    /// Editing key by name ("left", "backspace", "enter", ...) with optional
    /// modifiers `{shift, word, doc}`.
    #[napi(ts_args_type = "name: EditKeyName, mods?: KeyMods")]
    pub fn key(&mut self, name: String, mods: Option<Json>) -> Result<()> {
        let key = edit_key_of(&name)?;
        let m = mods
            .as_ref()
            .and_then(Json::as_object)
            .unwrap_or(empty_props());
        let mods = Mods {
            shift: bool_prop(m, "shift"),
            word: bool_prop(m, "word"),
            doc: bool_prop(m, "doc"),
        };
        self.input(InputEvent::Key(key, mods));
        Ok(())
    }

    /// Raw key press for `onKey` sinks (modal keymaps): a single character
    /// (layout-resolved, e.g. "W" or "$"), a name ("left", "enter", "escape",
    /// "f5", ...), with mods `{shift, ctrl, alt, super}`. Editing keys for
    /// focused editors still go through `key()`. `repeat` marks a press the
    /// OS auto-repeated. The sink hears `{kind:"key", phase:"down", ...}`.
    ///
    /// `physical` is the US-QWERTY key at that *position*, spelled the same
    /// way; omit it and it equals `code`. Passing both is how a driver
    /// reports a non-US layout, and it is what makes the reported `code`
    /// portable: a layout producing something outside ASCII would leave a
    /// Latin keymap matching nothing, so the position's US letter stands in.
    #[napi(ts_args_type = "code: string, mods?: KeySinkMods, repeat?: boolean, physical?: string")]
    pub fn key_down(
        &mut self,
        code: String,
        mods: Option<Json>,
        repeat: Option<bool>,
        physical: Option<String>,
    ) -> Result<()> {
        let kp = self.key_press(&code, mods.as_ref(), physical.as_deref())?;
        self.input(InputEvent::KeyDown(KeyPress {
            repeat: repeat.unwrap_or(false),
            ..kp
        }));
        Ok(())
    }

    /// The release of a key, spelled the way `keyDown` spells it (`physical`
    /// included): a sink that declared `keyUp` hears `{kind:"key",
    /// phase:"up", ...}` with `text` null; one that did not hears nothing,
    /// since presses only is the keymap default. A release whose press the
    /// sink never got resolves nothing, and moving focus while a key is
    /// held delivers the `up` first.
    #[napi(ts_args_type = "code: string, mods?: KeySinkMods, physical?: string")]
    pub fn key_up(
        &mut self,
        code: String,
        mods: Option<Json>,
        physical: Option<String>,
    ) -> Result<()> {
        let kp = self.key_press(&code, mods.as_ref(), physical.as_deref())?;
        self.input(InputEvent::KeyUp(kp.released()));
        Ok(())
    }

    /// A whole key going down, the way a window sends it: the raw press
    /// to an `onKey` sink, and then what the core is asked to do with that
    /// key — Escape dismisses a modal, Tab walks the focus ring, an arrow
    /// nudges a focused slider, Space presses a focused control, a
    /// printable character reaches the focused editor.
    ///
    /// **This is the one to reach for.** `keyDown` and `key` are its two
    /// halves, kept for a test that means to drive one channel and not the
    /// other; a test that means "the user pressed this key" wants both,
    /// and `keyDown("escape")` leaving a modal open is what having to
    /// choose used to cost (backlog F6).
    ///
    /// Spelled exactly as `keyDown`: a single character (layout-resolved,
    /// e.g. "W" or "$") or a name ("left", "enter", "escape", "f5", ...),
    /// with mods `{shift, ctrl, alt, super}`, `repeat` for an OS
    /// auto-repeat, and `physical` for the US-QWERTY key at that position.
    /// `release()` is the other end of the same key.
    #[napi(ts_args_type = "code: string, mods?: KeySinkMods, repeat?: boolean, physical?: string")]
    pub fn press(
        &mut self,
        code: String,
        mods: Option<Json>,
        repeat: Option<bool>,
        physical: Option<String>,
    ) -> Result<()> {
        let kp = self.key_press(&code, mods.as_ref(), physical.as_deref())?;
        let evs = self.core.press(KeyPress {
            repeat: repeat.unwrap_or(false),
            ..kp
        });
        self.take_events(evs);
        Ok(())
    }

    /// The same key coming up, spelled the way `press` spells it. One
    /// channel, because only one has a second half: the editing keys act
    /// on the way down, so this is `keyUp` under the name that pairs with
    /// `press`. A sink that declared `keyUp` hears it; one that did not
    /// hears nothing.
    #[napi(ts_args_type = "code: string, mods?: KeySinkMods, physical?: string")]
    pub fn release(
        &mut self,
        code: String,
        mods: Option<Json>,
        physical: Option<String>,
    ) -> Result<()> {
        let kp = self.key_press(&code, mods.as_ref(), physical.as_deref())?;
        let evs = self.core.release(kp);
        self.take_events(evs);
        Ok(())
    }

    /// One press from the `{shift, ctrl, alt, super}` shape both key calls
    /// take, with the text a plain key would insert already resolved.
    fn key_press(
        &self,
        code: &str,
        mods: Option<&Json>,
        physical: Option<&str>,
    ) -> Result<KeyPress> {
        let m = mods.and_then(Json::as_object).unwrap_or(empty_props());
        let kmods = KeyMods {
            shift: bool_prop(m, "shift"),
            ctrl: bool_prop(m, "ctrl"),
            alt: bool_prop(m, "alt"),
            super_key: bool_prop(m, "super"),
        };
        let layout = keycode_of(code)?;
        // No `physical` means "the key I just named", so the two agree and
        // `code` passes through; with one, the core applies the same
        // non-Latin fallback every driver gets.
        let press = match physical {
            None => KeyPress::new(layout, kmods),
            Some(p) => KeyPress::from_layout(layout, keycode_of(p)?, kmods),
        };
        let text = if !kmods.ctrl && !kmods.alt && !kmods.super_key {
            match press.code {
                KeyCode::Char(c) => Some(c.to_string()),
                KeyCode::Space => Some(" ".to_string()),
                _ => None,
            }
        } else {
            None
        };
        Ok(KeyPress { text, ..press })
    }

    /// Physical modifier state changed: `{shift, ctrl, alt, super}`. The
    /// host receives `{kind:"modifiers", ...}` when it differs from the
    /// last report.
    #[napi(ts_args_type = "mods?: KeySinkMods")]
    pub fn modifiers(&mut self, mods: Option<Json>) {
        let m = mods
            .as_ref()
            .and_then(Json::as_object)
            .unwrap_or(empty_props());
        self.input(InputEvent::Modifiers(KeyMods {
            shift: bool_prop(m, "shift"),
            ctrl: bool_prop(m, "ctrl"),
            alt: bool_prop(m, "alt"),
            super_key: bool_prop(m, "super"),
        }));
    }

    // -- Audio: the headless half. `play` and friends are shared (see
    // `core_methods!`); nothing sounds, so the commands queue up here.

    /// Drains the audio commands the core queued, as plain objects
    /// (`{kind:"play", playback, sound, volume, loop, fadeIn}`, ...) — what a
    /// windowed driver would play. For tests and custom drivers.
    #[napi(ts_return_type = "AudioCommand[]")]
    pub fn audio_commands(&mut self) -> Json {
        audio_commands_json(self.core.take_audio_commands())
    }

    /// Drains the announcements queued since the last drain, as plain
    /// objects (`{text, live}`). `runWindowed` drains and delivers them
    /// itself; a bare `Ctx` hands them back so a driver or a test can see
    /// what a frame asked to say.
    #[napi(ts_return_type = "Announcement[]")]
    pub fn announcements(&mut self) -> Json {
        Json::Array(
            self.core
                .take_announcements()
                .into_iter()
                .map(|a| {
                    let mut o = JsonMap::new();
                    o.insert("text".into(), Json::String(a.text));
                    o.insert("live".into(), Json::String(a.live.name().into()));
                    Json::Object(o)
                })
                .collect(),
        )
    }

    /// A custom driver reports a playback finished on its own; a tagged
    /// one becomes a `sound` event in `pollEvents`.
    #[napi]
    pub fn audio_ended(&mut self, playback: f64) {
        self.core.audio_ended(PlaybackId(playback as u64));
        let pending = self.core.take_pending_events();
        self.take_events(pending);
    }

    // -- Windows (headless) -----------------------------------------------
    // A `KuiWindow`'s runner applies these itself; a bare `Ctx` hands them
    // back so a driver or a test can see what a frame asked for.

    /// Drains the window commands the core queued, as plain objects: what
    /// chrome nodes asked for (`{kind:"startDrag"|"close"|"minimize"|
    /// "toggleMaximize", window}`) and what the declared window set decided
    /// (`{kind:"open", window, owner, origin, config:{kind, width, height,
    /// activates, anchor}}` / `{kind:"close", window}`).
    #[napi(ts_return_type = "WindowCommand[]")]
    pub fn window_commands(&mut self) -> Json {
        window_commands_json(self.core.take_window_commands())
    }

    /// A custom driver reports that the OS closed window `id`: it stays
    /// closed while still declared, whatever only it declared closes with
    /// it, and `{kind:"window", phase:"closed", name, id}` lands in
    /// `pollEvents`. Nothing happens for the main window (0) or for a
    /// window the diff already closed.
    #[napi]
    pub fn window_closed(&mut self, id: u32) {
        self.core.window_closed(kui_core::WindowId(id));
        let pending = self.core.take_pending_events();
        self.take_events(pending);
    }

    /// A custom driver reports that window `id` was asked to go away: a
    /// press landed outside it (`"outside"`) or Escape reached it
    /// (`"escape"`). `{kind:"dismiss", reason, name, id}` lands in
    /// `pollEvents` and **nothing closes** — the app stops declaring the
    /// window on the frame it decides to, exactly as it answers a `modal`
    /// node's dismissal. Nothing happens for a window that is not open.
    #[napi]
    pub fn window_dismissed(&mut self, id: u32, reason: String) {
        let reason = match &*reason {
            "escape" => kui_core::DismissReason::Escape,
            _ => kui_core::DismissReason::Outside,
        };
        self.core.dismiss_window(kui_core::WindowId(id), reason);
        let pending = self.core.take_pending_events();
        self.take_events(pending);
    }

    // -- Queries ---------------------------------------------------------

    /// The window title the last frame declared (a root `<box title>`), or
    /// null when it declared none. `runWindowed` applies it to the real
    /// window; a bare `Ctx` hands it back so a test can assert on it.
    #[napi]
    pub fn window_title(&self) -> Option<String> {
        self.core.window_title().map(str::to_string)
    }
}

/// Window commands as the objects `windowCommands()` hands out.
fn window_commands_json(cmds: Vec<kui_core::WindowCommand>) -> Json {
    use kui_core::WindowCommand;
    Json::Array(
        cmds.into_iter()
            .map(|cmd| {
                let mut o = JsonMap::new();
                let kind = match cmd {
                    WindowCommand::StartDrag(_) => "startDrag",
                    WindowCommand::Close(_) => "close",
                    WindowCommand::Minimize(_) => "minimize",
                    WindowCommand::ToggleMaximize(_) => "toggleMaximize",
                    WindowCommand::Open { .. } => "open",
                    WindowCommand::SetSize { .. } => "setSize",
                    WindowCommand::Focus(_) => "focus",
                };
                o.insert("kind".into(), Json::String(kind.into()));
                o.insert("window".into(), Json::from(cmd.window().0));
                if let WindowCommand::SetSize { size, .. } = cmd {
                    o.insert("width".into(), Json::from(size.w as f64));
                    o.insert("height".into(), Json::from(size.h as f64));
                }
                if let WindowCommand::Open {
                    owner,
                    origin,
                    config,
                    ..
                } = cmd
                {
                    o.insert("owner".into(), Json::from(owner.0));
                    o.insert("origin".into(), Json::from(origin.0));
                    let mut c = JsonMap::new();
                    c.insert(
                        "kind".into(),
                        Json::String(
                            match config.kind {
                                kui_core::WindowKind::Normal => "normal",
                                kui_core::WindowKind::Popup => "popup",
                            }
                            .into(),
                        ),
                    );
                    c.insert("width".into(), Json::from(config.size.w as f64));
                    c.insert("height".into(), Json::from(config.size.h as f64));
                    c.insert("activates".into(), Json::Bool(config.activates));
                    c.insert("anchor".into(), rect_json(config.anchor));
                    o.insert("config".into(), Json::Object(c));
                }
                Json::Object(o)
            })
            .collect(),
    )
}

/// `{x, y, w, h}` — the shape `scrollGeometry` already returns for a box.
fn rect_json(r: Rect) -> Json {
    let mut o = JsonMap::new();
    o.insert("x".into(), Json::from(r.x as f64));
    o.insert("y".into(), Json::from(r.y as f64));
    o.insert("w".into(), Json::from(r.w as f64));
    o.insert("h".into(), Json::from(r.h as f64));
    Json::Object(o)
}

/// The host facts `env()` hands back, as `{refreshHz, frameBudgetMs, focused,
/// viewport, window}`. Two places this differs from the same table in Lua,
/// deliberately:
///
/// - `refreshHz` is `null` when the host cannot tell, where Lua leaves the
///   key out. A stable shape is worth more here than a shorter object: JS
///   code destructures it, and TypeScript can then say `number | null`.
/// - `window.nativeControls` is the whole `Rect`, where Lua flattens it to
///   `controls_w` / `controls_h` (and C's `kui_env_set_window` takes the same
///   two numbers). That flattening assumes the OS controls sit at the window
///   origin, which is true of the macOS traffic lights and of nothing in
///   particular; the core holds a rect, so hand back a rect.
///
/// `viewport` is the frame's, not `Env`'s — it is the other host fact a view
/// wants at the same moment, and Lua carries it in the same table (as
/// `viewport_w` / `viewport_h`). Node spells it `{width, height, scale}`,
/// the `WindowSize` shape `runWindowed` already uses.
fn env_json(core: &mut Core) -> Json {
    let env = core.env;
    let mut vp = JsonMap::new();
    vp.insert("width".into(), Json::from(core.viewport().w as f64));
    vp.insert("height".into(), Json::from(core.viewport().h as f64));
    vp.insert("scale".into(), Json::from(core.scale() as f64));

    let win = env.window;
    let mut w = JsonMap::new();
    w.insert("id".into(), Json::from(win.id.0));
    w.insert("customChrome".into(), Json::Bool(win.custom_chrome));
    w.insert("maximized".into(), Json::Bool(win.maximized));
    w.insert("fullscreen".into(), Json::Bool(win.fullscreen));
    w.insert(
        "nativeControls".into(),
        win.native_controls.map_or(Json::Null, rect_json),
    );

    // What the user set in the OS. The two enums always have a key —
    // "unknown" is one of their readings, not the absence of one — and the
    // two values are null when the host cannot tell, the shape `refreshHz`
    // already uses. `accent` comes back as the `0xRRGGBBAA` number a prop
    // takes, so a view paints with it without converting.
    let sys = env.system;
    let mut sy = JsonMap::new();
    sy.insert("appearance".into(), Json::from(sys.appearance.name()));
    sy.insert(
        "accent".into(),
        sys.accent.map_or(Json::Null, |c| Json::from(c.to_hex())),
    );
    sy.insert("motion".into(), Json::from(sys.motion.name()));
    sy.insert(
        "locale".into(),
        sys.locale
            .map_or(Json::Null, |l| Json::from(l.as_str().to_string())),
    );

    let mut o = JsonMap::new();
    o.insert(
        "refreshHz".into(),
        env.refresh_hz
            .map_or(Json::Null, |hz| Json::from(hz as f64)),
    );
    o.insert(
        "frameBudgetMs".into(),
        Json::from(env.frame_budget_ms() as f64),
    );
    o.insert("focused".into(), Json::Bool(env.focused));
    o.insert("system".into(), Json::Object(sy));
    o.insert("viewport".into(), Json::Object(vp));
    o.insert("window".into(), Json::Object(w));
    Json::Object(o)
}

/// `{quadCount, viewportW, viewportH, scale, atlasSize}` for the last frame.
fn stats_json(core: &mut Core) -> Json {
    let (dl, atlas) = core.output();
    let mut o = JsonMap::new();
    o.insert("quadCount".into(), Json::from(dl.quads.len()));
    o.insert("viewportW".into(), Json::from(dl.viewport.w as f64));
    o.insert("viewportH".into(), Json::from(dl.viewport.h as f64));
    o.insert("scale".into(), Json::from(dl.scale as f64));
    o.insert("atlasSize".into(), Json::from(atlas.size));
    Json::Object(o)
}

fn sample_json(s: FrameSample) -> Json {
    let mut o = JsonMap::new();
    o.insert("inputMs".into(), Json::from(s.input_ms as f64));
    o.insert("viewMs".into(), Json::from(s.view_ms as f64));
    o.insert("layoutMs".into(), Json::from(s.layout_ms as f64));
    o.insert("renderMs".into(), Json::from(s.render_ms as f64));
    o.insert("waitMs".into(), Json::from(s.wait_ms as f64));
    o.insert("totalMs".into(), Json::from(s.total() as f64));
    o.insert("workMs".into(), Json::from(s.work() as f64));
    Json::Object(o)
}

/// The runner's frame-timing ring as `{frames, last, avgTotalMs,
/// maxTotalMs, avgWorkMs, maxWorkMs}`; `last` is null before the first
/// frame. The same numbers the latency HUD draws.
fn frame_stats_json(stats: &FrameStats) -> Json {
    let mut o = JsonMap::new();
    o.insert("frames".into(), Json::from(stats.len()));
    o.insert("last".into(), stats.last().map_or(Json::Null, sample_json));
    o.insert("avgTotalMs".into(), Json::from(stats.avg_total() as f64));
    o.insert("maxTotalMs".into(), Json::from(stats.max_total() as f64));
    o.insert("avgWorkMs".into(), Json::from(stats.avg_work() as f64));
    o.insert("maxWorkMs".into(), Json::from(stats.max_work() as f64));
    Json::Object(o)
}

/// Byte stride of one quad in the `quads()` buffer.
#[napi]
pub fn quad_stride() -> u32 {
    std::mem::size_of::<kui_core::Quad>() as u32
}

/// Byte stride of one clip in the `clips()` buffer.
#[napi]
pub fn clip_stride() -> u32 {
    std::mem::size_of::<kui_core::Clip>() as u32
}

// ---------------------------------------------------------------------------
// Windowed runner (winit + wgpu via kui's PumpRunner)

/// The `kui::App` behind a Node window. JS never gets called from inside
/// winit: it stores the next view tree per window between pumps
/// (`set_view`), and this lowers the stored tree whenever the runner
/// redraws that window. Events collect here and JS drains them after each
/// pump — the same data-only boundary as the headless `Ctx`, now with real
/// windows around it.
#[derive(Default)]
struct TreeApp {
    /// The last frame JS submitted for each window, by name: the binary IR
    /// stream and its string table, kept so a redraw between pumps can
    /// re-lower it. A window with no tree yet draws nothing.
    trees: std::collections::HashMap<String, (Vec<f64>, Vec<u8>)>,
    events: Vec<UiEvent>,
    error: Option<String>,
}

impl kui::App for TreeApp {
    fn view(&mut self, ui: &mut kui::Ui<'_>) {
        let name = ui.window_name();
        let Some((stream, strings)) = self.trees.get(&*name) else {
            return;
        };
        if let Err(e) = binary::lower_binary(ui, stream, strings) {
            self.error = Some(e.reason.to_string());
        }
    }

    fn on_event(&mut self, ev: UiEvent) {
        self.events.push(ev);
    }
}

// Stand-in for "no bound on this axis" when only one half of `maxWidth` /
// `maxHeight` is given: logical px past any display, and small enough that
// the platforms still convert it to physical px without overflowing.
const UNBOUNDED_SIZE: f64 = 65_535.0;

/// A real kui window (winit + wgpu) driven from Node. The event loop is
/// pumped, not run: call `pump()` from a timer loop so winit and libuv share
/// the main thread — or prefer `runWindowed`, which does that for you, unless
/// you are building your own loop. One event loop per process (winit event
/// loops are not recreatable on every platform), any number of windows on
/// it: a view whose root declares `windows` opens more, `windows()` lists
/// them, and `setView` takes the name of the one a tree is for.
#[napi]
pub struct KuiWindow {
    runner: kui::PumpRunner<TreeApp>,
}

#[napi]
impl KuiWindow {
    /// Options: `{width, height, minWidth, minHeight, maxWidth, maxHeight,
    /// chrome: "native" | "custom" | "borderless"}`. The min/max pairs bound
    /// what the user can resize the window to; either half may stand alone.
    #[napi(constructor, ts_args_type = "title: string, options?: WindowOptions")]
    pub fn new(title: String, options: Option<Json>) -> Result<Self> {
        let o = options
            .as_ref()
            .and_then(Json::as_object)
            .unwrap_or(empty_props());
        let mut launcher = kui::app(&title);
        let w = o.get("width").and_then(Json::as_f64);
        let h = o.get("height").and_then(Json::as_f64);
        if let (Some(w), Some(h)) = (w, h) {
            launcher = launcher.size(w, h);
        }
        let num = |k: &str| o.get(k).and_then(Json::as_f64).filter(|v| v.is_finite());
        // A lone `minWidth` leaves the other axis free: 0 for a missing min,
        // and for a missing max a bound no display reaches.
        let (min_w, min_h) = (num("minWidth"), num("minHeight"));
        if min_w.is_some() || min_h.is_some() {
            launcher = launcher.min_size(min_w.unwrap_or(0.0), min_h.unwrap_or(0.0));
        }
        let (max_w, max_h) = (num("maxWidth"), num("maxHeight"));
        if max_w.is_some() || max_h.is_some() {
            launcher = launcher.max_size(
                max_w.unwrap_or(UNBOUNDED_SIZE),
                max_h.unwrap_or(UNBOUNDED_SIZE),
            );
        }
        // Extensions, before the window opens: `extensions: [{path,
        // namespace?}]`. Origins are positions in the list, and the runner
        // owns it — so the routing a headless `Ctx` does for itself is the
        // runner's here, and `TreeApp::on_event` only ever sees what came
        // back out of it. C shared libraries only (ADR 0014); see
        // `Ctx::addExtension`.
        for e in o
            .get("extensions")
            .and_then(Json::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[])
        {
            let (path, ns) = match e {
                Json::String(p) => (p.as_str(), ""),
                Json::Object(m) => (
                    m.get("path").and_then(Json::as_str).unwrap_or(""),
                    m.get("namespace").and_then(Json::as_str).unwrap_or(""),
                ),
                _ => ("", ""),
            };
            if path.is_empty() {
                return Err(err("each `extensions` entry needs a `path`"));
            }
            // SAFETY: the caller's; the option is documented as loading
            // code into this process.
            let ext = unsafe { kui_ffi::CExtension::open(path) }.map_err(err)?;
            launcher = launcher.try_extension_as(ns, ext).map_err(err)?;
        }
        launcher = match o.get("chrome").and_then(Json::as_str) {
            Some("custom") => launcher.custom_titlebar(),
            Some("borderless") => launcher.borderless(),
            _ => launcher,
        };
        let runner = launcher
            .open(TreeApp::default())
            .map_err(|e| err(format!("failed to open window: {e}")))?;
        Ok(KuiWindow { runner })
    }

    /// `setView` with a flat binary instruction stream (see `Ctx::frame_binary`).
    /// Copied once so redraws (resize, hover) can re-lower it between pumps.
    /// `window` names which window the tree is for — `"main"` when left
    /// out; the names `windows()` lists otherwise.
    #[napi]
    pub fn set_view_binary(
        &mut self,
        stream: Float64Array,
        strings: Uint8Array,
        window: Option<String>,
    ) {
        let name = window.unwrap_or_else(|| kui_core::session::MAIN_WINDOW_NAME.to_string());
        self.runner
            .app_mut()
            .trees
            .insert(name, (stream.to_vec(), strings.to_vec()));
        self.runner.request_redraw();
    }

    /// Processes pending OS events without blocking. Returns false once the
    /// window has closed.
    #[napi]
    pub fn pump(&mut self) -> Result<bool> {
        let alive = self.runner.pump();
        if let Some(e) = self.runner.app_mut().error.take() {
            return Err(err(format!("view lowering failed: {e}")));
        }
        Ok(alive)
    }

    /// The window's inner size in logical px plus its scale factor:
    /// `{width, height, scale}`. Readable before the first frame (in
    /// `setup`), and re-reported as a `{kind:"resize", width, height,
    /// scale}` event through `pollEvents` — a `ResizeMsg` — whenever the
    /// window changes size or moves to a display with another DPI.
    #[napi(ts_return_type = "WindowSize")]
    pub fn size(&self) -> Json {
        let (size, scale) = self.runner.window_size();
        size_json(size, scale)
    }

    /// Frame timing measured by the runner — what the latency HUD draws,
    /// as data: `{frames, last: {inputMs, viewMs, layoutMs, renderMs,
    /// waitMs, totalMs, workMs} | null, avgTotalMs, maxTotalMs, avgWorkMs,
    /// maxWorkMs}` over the last 120 frames. `waitMs` is vsync
    /// backpressure; `workMs` is everything else.
    #[napi(ts_return_type = "FrameTiming")]
    pub fn frame_stats(&mut self) -> Json {
        frame_stats_json(&self.runner.core_mut().stats)
    }

    /// Asks the window to close; the next pump returns false.
    #[napi]
    pub fn close(&mut self) {
        self.runner.request_exit();
    }
}

// ---------------------------------------------------------------------------
// The shared surface

/// Pending UI events as `[{origin, window, key, payload}]` — payloads plain
/// JSON, so your Elm messages come back out as data.
///
/// `origin` and `window` are not two readings of the same thing: origin is
/// which frontend drew the node (0 = your app, 1+ = an extension), window
/// is which OS window it happened in, and an extension draws into all of
/// them. `window` is 0 until a frame declares a second one.
fn events_json(events: Vec<UiEvent>) -> Json {
    Json::Array(
        events
            .into_iter()
            .map(|ev| {
                let mut o = JsonMap::new();
                o.insert("origin".into(), Json::from(ev.origin.0));
                o.insert("window".into(), Json::from(ev.window.0));
                o.insert("key".into(), Json::String(key_str(ev.key)));
                o.insert("payload".into(), json_of(&ev.payload));
                Json::Object(o)
            })
            .collect(),
    )
}

/// Every call that is nothing but a hop to the core, written once and
/// generated for both `Ctx` and `KuiWindow`. The two classes differ only in
/// how they reach the core and what has to happen afterwards, so that is all
/// the macro takes:
///
/// - `core` / `events`: private accessors for the `Core` and the pending-event
///   buffer, passed by name rather than as `self.field` because a `self` from
///   the call site cannot cross into a macro-defined method (E0424).
/// - `redraw`: run after anything that changes what the next frame shows. A
///   `Ctx` has no window to invalidate, so it is a no-op there.
/// - `audio`: run after anything that changes playback. A headless `Ctx`
///   queues the commands for `audioCommands`; a window flushes them to its
///   device at once.
///
/// Adding a call here puts it on both classes, which is the point: this list
/// is the whole shared API, and there is no second copy to forget.
macro_rules! core_methods {
    (
        $ty:ident,
        core = $core:ident,
        events = $events:ident,
        take = $take:ident,
        redraw = $redraw:ident,
        audio = $audio:ident $(,)?
    ) => {
        #[napi]
        impl $ty {
            // -- Resources: images ----------------------------------------

            /// Registers a w×h RGBA image (pixels copied); returns its id for
            /// `<image src={id}>`. Stable until `removeImage`.
            #[napi]
            pub fn add_image(&mut self, width: u32, height: u32, rgba: Buffer) -> Result<String> {
                add_image_impl(self.$core(), width, height, &rgba)
            }

            #[napi]
            pub fn remove_image(&mut self, id: String) -> Result<()> {
                self.$core()
                    .remove_image(ImageId::from_ffi(parse_u64(&id)?));
                Ok(())
            }

            // -- Resources: fragments -------------------------------------

            /// Registers a WGSL fragment function; returns its id for
            /// `<fragment src={id}>`. Throws when the source does not
            /// compile, with the compiler's message in the app's own line
            /// numbers. Idempotent by source, so the same text gets the
            /// same id without being validated twice.
            #[napi]
            pub fn add_fragment(&mut self, wgsl: String) -> Result<String> {
                add_fragment_impl(self.$core(), &wgsl)
            }

            #[napi]
            pub fn remove_fragment(&mut self, id: String) -> Result<()> {
                self.$core()
                    .remove_fragment(kui_core::FragmentId::from_ffi(parse_u64(&id)?));
                Ok(())
            }

            // -- Resources: fonts -----------------------------------------

            /// Registers a font from file bytes (TTF/OTF/TTC); returns its id
            /// for the `font` prop on `<text>` / `<edit>`. Throws when the
            /// data holds no usable face.
            #[napi]
            pub fn add_font(&mut self, data: Buffer) -> Result<String> {
                add_font_impl(self.$core(), &data)
            }

            /// Registers an installed font by family name; null when none
            /// matches (see `systemFontFamilies`). Also finds families loaded
            /// with `loadFontsDir` / `loadFontFile`; the same family gets the
            /// same id.
            #[napi]
            pub fn add_system_font(&mut self, name: String) -> Option<String> {
                self.$core().add_system_font(&name).map(font_str)
            }

            /// Registers a font file by path (memory-mapped); throws when it
            /// cannot be read or holds no usable face.
            #[napi]
            pub fn load_font_file(&mut self, path: String) -> Result<String> {
                load_font_file_impl(self.$core(), &path)
            }

            /// Loads every font file under a folder (recursively) so its
            /// families can be picked by name with `addSystemFont`; returns
            /// the face count.
            #[napi]
            pub fn load_fonts_dir(&mut self, dir: String) -> u32 {
                self.$core().load_fonts_dir(&dir) as u32
            }

            #[napi]
            pub fn remove_font(&mut self, id: String) -> Result<()> {
                self.$core().remove_font(FontId::from_ffi(parse_u64(&id)?));
                Ok(())
            }

            /// Family names of every font the core can see, installed or
            /// loaded (sorted).
            #[napi]
            pub fn system_font_families(&mut self) -> Vec<String> {
                self.$core().system_font_families()
            }

            // -- Audio -----------------------------------------------------

            /// Registers a sound from its encoded bytes (wav/ogg/mp3/flac);
            /// returns its id for `<audio src>`, the `clickSound` /
            /// `hoverSound` props and `play`. Stable until `removeSound`.
            #[napi]
            pub fn add_sound(&mut self, data: Buffer) -> Result<String> {
                add_sound_impl(self.$core(), &data)
            }

            #[napi]
            pub fn remove_sound(&mut self, id: String) -> Result<()> {
                self.$core()
                    .remove_sound(SoundId::from_ffi(parse_u64(&id)?));
                self.$audio();
                Ok(())
            }

            /// Starts a playback: `{volume, loop, fadeIn, tag}`; returns its
            /// id for `stop` / `setVolume` / `pause` / `resume`. A `tag` comes
            /// back as a `SoundMsg` when the playback finishes on its own.
            /// Says something once, with no node behind it: `announce("Saved")`,
            /// `announce("3 results", "assertive")`. `"off"` and an empty string
            /// are both no-ops. A region whose message is on screen is the `live`
            /// prop instead
            /// (`docs/adr/0008-live-regions-and-announcements.md`).
            ///
            /// Call it from an event handler. Called while building a frame it
            /// fires every frame, which the core reports as
            /// `announcement-repeated`.
            #[napi(ts_args_type = "text: string, live?: Live")]
            pub fn announce(&mut self, text: String, live: Option<String>) -> Result<()> {
                let live = live.as_deref().unwrap_or("polite");
                let i = kui_core::schema::LIVE.iter().position(|v| *v == live);
                let Some(i) = i else {
                    return Err(err(format!(
                        "bad politeness {live:?} (one of {})",
                        kui_core::schema::LIVE.join(" | ")
                    )));
                };
                self.$core().announce(&text, kui_core::Live::from_index(i));
                Ok(())
            }

            /// A window plays it on its own device at once; headless nothing
            /// sounds and the command queues for `audioCommands()`.
            #[napi(ts_args_type = "sound: string, opts?: PlayOptions")]
            pub fn play(&mut self, sound: String, opts: Option<Json>) -> Result<f64> {
                let id = play_impl(self.$core(), &sound, opts.as_ref())?;
                self.$audio();
                Ok(id)
            }

            #[napi(ts_args_type = "playback: number, fadeMs?: number")]
            pub fn stop(&mut self, playback: f64, fade_ms: Option<f64>) {
                self.$core()
                    .stop(PlaybackId(playback as u64), fade_ms.unwrap_or(0.0) as f32);
                self.$audio();
            }

            #[napi(ts_args_type = "playback: number, volume: number, tweenMs?: number")]
            pub fn set_volume(&mut self, playback: f64, volume: f64, tween_ms: Option<f64>) {
                self.$core().set_volume(
                    PlaybackId(playback as u64),
                    volume as f32,
                    tween_ms.unwrap_or(0.0) as f32,
                );
                self.$audio();
            }

            #[napi(ts_args_type = "playback: number, fadeMs?: number")]
            pub fn pause(&mut self, playback: f64, fade_ms: Option<f64>) {
                self.$core()
                    .pause(PlaybackId(playback as u64), fade_ms.unwrap_or(0.0) as f32);
                self.$audio();
            }

            #[napi(ts_args_type = "playback: number, fadeMs?: number")]
            pub fn resume(&mut self, playback: f64, fade_ms: Option<f64>) {
                self.$core()
                    .resume(PlaybackId(playback as u64), fade_ms.unwrap_or(0.0) as f32);
                self.$audio();
            }

            #[napi(ts_args_type = "volume: number, tweenMs?: number")]
            pub fn set_master_volume(&mut self, volume: f64, tween_ms: Option<f64>) {
                self.$core()
                    .set_master_volume(volume as f32, tween_ms.unwrap_or(0.0) as f32);
                self.$audio();
            }

            // -- Events ----------------------------------------------------

            /// Events since the last poll: `[{origin, window, key, payload}]`,
            /// payloads as plain data (your Elm messages come back out
            /// here). `A` types them — the app's own union, or one core
            /// message type when only that is being watched.
            #[napi(
                ts_generic_types = "A = AppMsg | CoreMsg",
                ts_return_type = "UiEvent<A>[]"
            )]
            pub fn poll_events(&mut self) -> Json {
                // Also whatever a call between frames left pending — the
                // synthetic key releases `focus` / `blur` force, a `resize`.
                // A window's runner drains these at its next redraw anyway;
                // draining here means both classes hand them over at the
                // same moment rather than a pump apart.
                let pending = self.$core().take_pending_events();
                self.$take(pending);
                events_json(std::mem::take(self.$events()))
            }

            // -- Frame -----------------------------------------------------

            /// True when the last frame left a transition mid-flight. A window
            /// schedules its own redraws for that; this is for tests and
            /// drivers that want to know when motion has settled.
            #[napi]
            pub fn animating(&mut self) -> bool {
                self.$core().animating()
            }

            /// Byte budget for the shaped-text cache: every text a frame
            /// draws is shaped once and kept, and past this many
            /// (estimated) bytes the least recently drawn entries go at
            /// the start of the next frame — never what the last frame
            /// drew. Default 64 MB; a terminal streaming new lines lowers
            /// it, a viewer that wants every page it showed kept warm
            /// raises it.
            #[napi]
            pub fn set_text_cache_budget(&mut self, bytes: f64) {
                self.$core().set_text_cache_budget(bytes.max(0.0) as usize);
            }

            /// What the shaped-text cache holds, in the estimated bytes
            /// the budget is charged against.
            #[napi]
            pub fn text_cache_bytes(&mut self) -> f64 {
                self.$core().text_cache_bytes() as f64
            }

            /// Summary of the last frame's display list.
            #[napi(ts_return_type = "FrameStats")]
            pub fn stats(&mut self) -> Json {
                stats_json(self.$core())
            }

            /// Raw quads for the finished frame, `quadStride()` bytes each,
            /// laid out as kui-ffi's KuiQuad (see include/kui.h) and decoded
            /// by `decodeQuads`. Copied into the Buffer. A window answers
            /// with what its last pump drew, so a smoke test can read the
            /// frame the shipping driver painted and not only a headless
            /// one's (backlog F19). Drive that window with `access(key,
            /// action)` — `click`, `type` and `key` are refused there,
            /// because the OS is what drives a real window.
            #[napi]
            pub fn quads(&mut self) -> Buffer {
                let (dl, _) = self.$core().output();
                let bytes = unsafe {
                    std::slice::from_raw_parts(
                        dl.quads.as_ptr().cast::<u8>(),
                        std::mem::size_of_val(dl.quads.as_slice()),
                    )
                };
                Buffer::from(bytes.to_vec())
            }

            /// The clips this frame's quads name through their `clip`
            /// index, `clipStride()` bytes each, laid out as kui-ffi's
            /// KuiClip (see include/kui.h) and decoded by `decodeClips`.
            /// Entry zero clips nothing, so a quad always has one; the
            /// list is empty only on a frame that drew nothing.
            #[napi]
            pub fn clips(&mut self) -> Buffer {
                let (dl, _) = self.$core().output();
                let bytes = unsafe {
                    std::slice::from_raw_parts(
                        dl.clips.as_ptr().cast::<u8>(),
                        std::mem::size_of_val(dl.clips.as_slice()),
                    )
                };
                Buffer::from(bytes.to_vec())
            }

            /// This frame's fragment draws, in the order their quads index
            /// them by `uv[0]`: seventeen doubles each, the handle as two
            /// 32-bit halves and then the sixteen parameters. The
            /// parameters ride a side list rather than the quad, so
            /// `quads()` alone cannot show them and a corpus adapter
            /// needs this to compare them
            /// (`docs/adr/0015-a-fragment-element-and-the-painter-it-is-not.md`).
            /// Empty on a frame that draws no fragment.
            #[napi]
            pub fn fragment_draws(&mut self) -> Vec<f64> {
                let (dl, _) = self.$core().output();
                let mut out = Vec::with_capacity(dl.fragments.len() * 18);
                for f in &dl.fragments {
                    let raw = f.id.to_ffi();
                    out.push((raw >> 32) as f64);
                    out.push((raw & 0xffff_ffff) as f64);
                    out.extend(f.params.iter().map(|v| *v as f64));
                }
                out
            }

            // -- Environment -----------------------------------------------

            /// Host facts the frame driver pushed in: what the window and the
            /// display are doing, as of now (see `Env`). This is the same
            /// surface Lua's `view(env)` reads and C's `kui_env_set*` writes —
            /// a JSX app needs it to build its own titlebar (inset past the
            /// macOS traffic lights, pick the maximize glyph), to dim its
            /// chrome when the window loses focus, or to pace itself against
            /// the real refresh rate.
            ///
            /// Note `env().focused` is the *window*'s keyboard focus, not the
            /// focused node's key — that is `focused()`, one call up.
            #[napi(ts_return_type = "Env")]
            pub fn env(&mut self) -> Json {
                env_json(self.$core())
            }

            // -- Queries ---------------------------------------------------

            /// Measures text the way layout would, without adding a node:
            /// `{width, height, lines}` in logical px, wrapped to `maxWidth`
            /// when given. `content` is whatever `<text>` takes (a string, or
            /// children with `<span>`s); `style` the `<text>` props (`size`,
            /// `font`, `wrap`, `maxLines`, `ellipsis`, ...). Works before the
            /// first frame; a window answers at its own scale once a frame has
            /// run. Size a column to its widest label, or pick the tier that
            /// fits, from these numbers instead of constants found by
            /// screenshot. The metrics do not scale linearly: `measured ×
            /// zoom` is not `measure(size × zoom)`, because shaping rounds
            /// per size, so anything that zooms measures at the size it
            /// draws.
            #[napi(
                ts_args_type = "content: KuiNode, style?: TextProps, maxWidth?: number",
                ts_return_type = "TextMetrics"
            )]
            pub fn measure_text(
                &mut self,
                content: Json,
                style: Option<Json>,
                max_width: Option<f64>,
            ) -> Result<Json> {
                measure_text_impl(self.$core(), &content, style.as_ref(), max_width)
            }

            /// Drains the warnings the core raised since the last call
            /// (see `Warning`), each distinct (code, node) pair once.
            /// `createApp` collects them on `app.warnings` for you, and
            /// `runWindowed` prints them, unless either was told not to.
            #[napi(ts_return_type = "Warning[]")]
            pub fn warnings(&mut self) -> Json {
                warnings_json(self.$core().take_warnings())
            }

            /// Turns the per-frame diagnostic checks behind `warnings` on or
            /// off. A bare `Ctx` has them on; `createApp` / `runWindowed`
            /// turn them off under `NODE_ENV=production`.
            #[napi]
            pub fn set_diagnostics(&mut self, on: bool) {
                self.$core().set_diagnostics(on);
            }

            /// The prop names the encoder threw away while lowering a tree,
            /// as `[element, name]` pairs, raised as `unknown-prop` warnings
            /// (see `Warning`). A name outside the schema never reaches the
            /// binary stream, so the encoder is the only side that sees it;
            /// `frame` / `setView` report what they dropped through here.
            /// Behind the same `setDiagnostics` gate, and once per name.
            #[napi(ts_args_type = "props: [string, string][]")]
            pub fn warn_unknown_props(&mut self, props: Vec<Vec<String>>) {
                for pair in &props {
                    let [element, name] = &pair[..] else { continue };
                    let w = kui_core::diag::unknown_prop(
                        element,
                        name,
                        kui_core::schema::Spelling::Camel,
                    );
                    self.$core().warn(w);
                }
            }

            // -- Accessibility ---------------------------------------------

            /// What assistive technology sees of the last frame (see
            /// `AccessTree`). A window hands it to the platform by itself
            /// (AccessKit); this is for tests and tooling.
            #[napi(ts_return_type = "AccessTree")]
            pub fn access_tree(&mut self) -> Json {
                access_tree_json(self.$core().access_tree())
            }

            /// A request from assistive technology on a node: an `AccessAction`
            /// name the node advertises, with `value` the new text for
            /// `setValue`. `key` is either spelling of the node — the hex key
            /// an event carried (16 digits), or the label its `key` prop
            /// declared, resolved through the last frame (see `focus`).
            /// Resolved like its pointer/keyboard equivalent, so the resulting
            /// events come out of `pollEvents`. A real screen reader's
            /// requests arrive through a window on their own.
            #[napi(ts_args_type = "key: string, action: AccessAction, value?: string | AccessArg")]
            pub fn access(
                &mut self,
                key: String,
                action: String,
                value: Option<Json>,
            ) -> Result<()> {
                let key = resolve_key(self.$core(), &key)?;
                let req = access_request(key, &action, value)?;
                let events = self.$core().handle_input(InputEvent::Access(req));
                self.$take(events);
                self.$redraw();
                Ok(())
            }

            // -- Hover / press ---------------------------------------------

            /// Hover state as of the last frame. `key` is either spelling,
            /// as for `focus`: the label a `key` prop declared, or the hex
            /// key an event carried. For plain hover styling prefer the
            /// `hoverBg` / `pressedBg` props — the core resolves those without
            /// a round trip.
            #[napi]
            pub fn is_hovered(&mut self, key: String) -> Result<bool> {
                let key = resolve_key(self.$core(), &key)?;
                Ok(self.$core().is_hovered(key))
            }

            /// Press state as of the last frame; `key` is either spelling,
            /// as for `isHovered`.
            #[napi]
            pub fn is_pressed(&mut self, key: String) -> Result<bool> {
                let key = resolve_key(self.$core(), &key)?;
                Ok(self.$core().is_pressed(key))
            }

            // -- Pointer ----------------------------------------------------

            /// The pointer shape for where the pointer is now, in the `cursor`
            /// prop's own vocabulary: derived from the topmost node under it
            /// — an editor is `'text'`, an `onClick` or `focusable` node
            /// `'pointer'`, an `onDrag` node `'grab'` (`'grabbing'` while it
            /// drags), a plain box or no pointer at all `'default'` — or
            /// whatever that node's `cursor` overrode it with. A window
            /// applies it to the real cursor by itself and only touches it
            /// when the answer changes; this is for tests and drivers.
            #[napi(ts_return_type = "CursorShape")]
            pub fn cursor_shape(&mut self) -> String {
                self.$core().cursor_shape().name().to_string()
            }

            // -- Focus ------------------------------------------------------

            /// Whether a node holds keyboard focus — any node: an editor, an
            /// `onKey` sink, a button Tab landed on (see `focused`). `key` is
            /// a hex key or a declared label, as for `focus`.
            #[napi]
            pub fn is_focused(&mut self, key: String) -> Result<bool> {
                let core = self.$core();
                let key = resolve_key(core, &key)?;
                Ok(core.is_focused(key))
            }

            /// The node holding keyboard focus (hex key), or null. Tab /
            /// Shift-Tab walk every control in tree order, Enter and Space
            /// press the focused one, and the arrows nudge a focused
            /// slider — `press("tab")`, `press(" ")`, `press("right")`,
            /// which is what a keyboard sends. (`key("tab")` is the half
            /// of that press the core acts on, for a test that means to
            /// drive one channel; `keyDown` is the other half, the one an
            /// `onKey` sink hears.)
            #[napi]
            pub fn focused(&mut self) -> Option<String> {
                self.$core().focus().map(key_str)
            }

            /// Whether focus got where it is by keyboard or assistive
            /// technology rather than a click — when it shows (the ring, or
            /// `focusBg`).
            #[napi]
            pub fn focus_visible(&mut self) -> bool {
                self.$core().focus_visible()
            }

            /// Moves keyboard focus to a node now (an editor, an `onKey` sink,
            /// a control, a `focusable` box); `keyFocus` on a box is the
            /// declarative, edge-triggered form.
            ///
            /// `key` is either spelling of the node: the 16-digit hex key an
            /// event carried, or the label its `key` prop declared —
            /// `focus('note')` — resolved through the last frame, so a node
            /// the user has never touched can be named. Labels are unique
            /// among siblings, not across the tree: when two nodes declare
            /// the same one, the first in tree order wins and an
            /// `ambiguous-key` warning says so. A label no node declared
            /// throws.
            #[napi]
            pub fn focus(&mut self, key: String) -> Result<()> {
                let core = self.$core();
                let key = resolve_key(core, &key)?;
                core.set_focus(Some(key));
                self.$redraw();
                Ok(())
            }

            /// The hex key of the node a label names — the label a `key`
            /// prop declared, resolved through the frame being built so
            /// far and then the last finished one — or null when no node
            /// declared it. The door for holding a key across frames;
            /// every call that takes a key takes the label too, so this
            /// is for caching one, or for checking that a name reached
            /// the view. Two nodes on one label under different parents
            /// resolve to the first in tree order and raise
            /// `ambiguous-key`.
            #[napi]
            pub fn key_of(&mut self, label: String) -> Option<String> {
                self.$core().key_of(&label).map(key_str)
            }

            #[napi]
            pub fn blur(&mut self) {
                self.$core().set_focus(None);
                self.$redraw();
            }

            /// What Tab does, as a call — for an `onKey` sink that binds Tab
            /// itself and wants to hand the keyboard on: the next focusable
            /// node in tree order, wrapping.
            #[napi]
            pub fn focus_next(&mut self) {
                self.$core().focus_next(true);
                self.$redraw();
            }

            /// What Shift-Tab does.
            #[napi]
            pub fn focus_prev(&mut self) {
                self.$core().focus_next(false);
                self.$redraw();
            }

            // -- Scrolling --------------------------------------------------

            /// Scrolls whatever contains a node so it shows — "scroll to the
            /// selected row", which needs the container geometry only the core
            /// has. The request resolves against the *next* frame's layout (one
            /// is requested), so a row the view is about to declare for the
            /// first time reveals fine. If that frame does not declare the key,
            /// or nothing above it scrolls, it is a no-op and is not kept for a
            /// later frame; two reveals before one frame are contradictory, so
            /// the last wins. `key` is a hex key or a declared label, as for
            /// `focus` — a label resolves through the *last* frame, so a row
            /// the coming frame declares for the first time is reachable by
            /// its hex key only.
            #[napi]
            pub fn reveal(&mut self, key: String) -> Result<()> {
                let core = self.$core();
                let key = resolve_key(core, &key)?;
                core.reveal(key);
                self.$redraw();
                Ok(())
            }

            /// A scroll container's retained offset `{x, y}` as the last layout
            /// clamped it (positive = content moved up / left) — the number to
            /// keep in a model and hand back to `setScroll`. Zero for a node
            /// that never scrolled.
            #[napi(ts_return_type = "ScrollOffset")]
            pub fn scroll_offset(&mut self, key: String) -> Result<Json> {
                let key = resolve_key(self.$core(), &key)?;
                Ok(offset_json(self.$core().scroll_offset(key)))
            }

            /// Everything the last layout resolved for the scroll container
            /// `key`: its box `{x, y, w, h}`, its content size `{contentW,
            /// contentH}` and the clamped `offset` — `null` for a key no layout
            /// has resolved as a container.
            ///
            /// This is what makes a long list affordable. The core builds every
            /// child a view declares, so ten thousand rows cost ten thousand
            /// rows; knowing `h` and `offset.y`, a view renders the rows that
            /// fit plus two spacers holding the space of the rest. Read while
            /// building, it describes the previous frame, so a resize slices one
            /// frame late — render a row or two extra at each end.
            #[napi(ts_return_type = "ScrollGeometry | null")]
            pub fn scroll_geometry(&mut self, key: String) -> Result<Option<Json>> {
                let key = resolve_key(self.$core(), &key)?;
                Ok(geometry_json(self.$core().scroll_geometry(key)))
            }

            /// Where a point lands in the text a keyed node drew: a byte
            /// offset into its text and the visual line, or null for a key
            /// that drew no text. `x`/`y` are the logical viewport px a
            /// `click` or `drag` event carries, so a custom editor turns the
            /// event into a caret position with one call — no prefix
            /// measuring, no cell-width arithmetic. A node holding several
            /// text runs (a `line` row of token runs) answers across them
            /// in order, the way the access tree reads the line. Answered
            /// from the frame that finished: the layout the pointer was
            /// over.
            #[napi(ts_return_type = "TextHit | null")]
            pub fn text_hit(&mut self, key: String, x: f64, y: f64) -> Result<Option<Json>> {
                let key = resolve_key(self.$core(), &key)?;
                Ok(self
                    .$core()
                    .text_hit(key, Vec2::new(x as f32, y as f32))
                    .map(|h| {
                        let mut o = JsonMap::new();
                        o.insert("byte".into(), Json::from(h.byte as u64));
                        o.insert("line".into(), Json::from(h.line));
                        Json::Object(o)
                    }))
            }

            /// The caret rect for a byte offset in the text a keyed node
            /// drew: logical viewport px, zero wide, one line tall — where
            /// a caret, a selection edge or an IME candidate window goes.
            /// A byte past the text is the end; null for a key that drew
            /// no text.
            #[napi(ts_return_type = "Rect | null")]
            pub fn caret_rect(&mut self, key: String, byte: f64) -> Result<Option<Json>> {
                let key = resolve_key(self.$core(), &key)?;
                Ok(self
                    .$core()
                    .caret_rect(key, byte.max(0.0) as usize)
                    .map(rect_json))
            }

            /// Where the OS candidate window goes while a composition is
            /// under way: the focused `<edit>`'s caret, or a custom editor's
            /// `line` carrying `caret`; null when nothing with a caret is
            /// focused. The windowed driver applies it itself; headless,
            /// it is what a test reads to see the anchor moved.
            #[napi(ts_return_type = "Rect | null")]
            pub fn ime_rect(&mut self) -> Option<Json> {
                self.$core().ime_rect().map(rect_json)
            }

            /// Sets that offset the way the wheel would; the next frame's
            /// layout clamps it, so `(0, 0)` jumps to the top and a huge `y` to
            /// the end without knowing the content height.
            #[napi]
            pub fn set_scroll(&mut self, key: String, x: f64, y: f64) -> Result<()> {
                let key = resolve_key(self.$core(), &key)?;
                self.$core().set_scroll(key, Vec2::new(x as f32, y as f32));
                self.$redraw();
                Ok(())
            }

            // -- Windows ----------------------------------------------------

            /// The names of every window open right now, `"main"` first,
            /// then in the order they opened — what a view's root
            /// `windows` declared and the diff has opened. `view(model,
            /// window)` is called once per name.
            #[napi]
            pub fn windows(&mut self) -> Vec<String> {
                self.$core()
                    .windows()
                    .into_iter()
                    .map(|(_, name)| name.to_string())
                    .collect()
            }

            /// The name of the window this core draws: `"main"`, or the
            /// name the declaration that opened `env().window.id` used.
            #[napi]
            pub fn window_name(&mut self) -> String {
                self.$core().window_name().to_string()
            }

            /// Asks the driver to resize a window to `width`×`height` logical
            /// px. A request and not a declaration: a window's `size` config
            /// is read on the frame it opens and never again, because the user
            /// owns a window's size once it exists, so this is the only way an
            /// app moves a live one. Queued the way `reveal` is — a `KuiWindow`
            /// applies it on its next pump, and the window answers with the
            /// ordinary `resize` event carrying the size it actually became,
            /// while a headless `Ctx` has no window and simply keeps the
            /// request. `window` is the id events carry (`env().window.id`),
            /// 0 for the main window.
            #[napi]
            pub fn set_window_size(&mut self, window: u32, width: f64, height: f64) -> Result<()> {
                self.$core().set_window_size(
                    kui_core::WindowId(window),
                    Size::new(width as f32, height as f32),
                );
                self.$redraw();
                Ok(())
            }

            /// Asks the driver to give a window keyboard focus; queued the same
            /// way. Advisory: whether the window manager agreed shows up as
            /// `env().focused` on the frames that follow, not as a reply.
            #[napi]
            pub fn focus_window(&mut self, window: u32) -> Result<()> {
                self.$core().focus_window(kui_core::WindowId(window));
                self.$redraw();
                Ok(())
            }

            // -- Editors ----------------------------------------------------

            #[napi]
            pub fn edit_text(&mut self, key: String) -> Result<Option<String>> {
                let key = resolve_key(self.$core(), &key)?;
                Ok(self.$core().edit_text(key))
            }

            /// Replaces an editor's text, leaving the caret at the end.
            ///
            /// It reaches an editor that does not exist yet: the `update`
            /// that opens a rename field runs a frame ahead of the view
            /// that declares it, so the text is held for the frame that
            /// declares this name and seeds the editor there, over
            /// `initial`. Name it by the label its `key` prop declares —
            /// the spelling that needs nothing to exist yet, since the
            /// hex key comes from an event the editor has not fired.
            /// Either spelling works, as for `focus`; a label the last
            /// frame declared lands at once, and one it did not is held.
            /// Held for that one frame — a name nothing declares on it
            /// drops its text with an `edit-text-without-editor` warning,
            /// so a name the view spells differently is a line rather
            /// than a field that opens with the wrong text.
            #[napi]
            pub fn set_edit_text(&mut self, key: String, text: String) -> Result<()> {
                let core = self.$core();
                match hex_key(&key) {
                    Some(k) => core.set_edit_text(k, &text),
                    None => core.set_edit_text_by_label(&key, &text),
                }
                self.$redraw();
                Ok(())
            }
        }
    };
}

impl Ctx {
    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn events_mut(&mut self) -> &mut Vec<UiEvent> {
        &mut self.events
    }

    /// Where events the core just produced go. A headless context is its
    /// own runner, so it routes them the way `Shell::route_events` does:
    /// an extension's event to the extension, its replies to the queue the
    /// app polls. See [`absorb`].
    fn take_events(&mut self, events: Vec<UiEvent>) {
        absorb(&mut self.extensions, events, &mut self.events);
    }

    /// Nothing to invalidate: a headless `Ctx` shows whatever the next
    /// `frame` submits.
    fn no_redraw(&mut self) {}

    /// Nothing to flush: the commands wait in the core for `audioCommands`.
    fn no_flush(&mut self) {}
}

impl KuiWindow {
    fn core_mut(&mut self) -> &mut Core {
        self.runner.core_mut()
    }

    fn events_mut(&mut self) -> &mut Vec<UiEvent> {
        &mut self.runner.app_mut().events
    }

    /// Through the runner's own list, for the same reason a headless
    /// `Ctx` routes through its own: what arrives here is what *this*
    /// driver asked the core for — a `press`, an access action, the
    /// pending events `pollEvents` drains — and the loop never saw those,
    /// so nobody has routed them yet. (Whatever `TreeApp::on_event`
    /// queued during a `pump` was routed on the way in and is already in
    /// `events_mut`; it does not pass through here.)
    fn take_events(&mut self, events: Vec<UiEvent>) {
        self.runner
            .route_events(events, |app, ev| app.events.push(ev));
    }

    fn request_redraw(&mut self) {
        self.runner.request_redraw();
    }

    fn flush_audio(&mut self) {
        self.runner.flush_audio();
    }
}

core_methods!(
    Ctx,
    core = core_mut,
    events = events_mut,
    take = take_events,
    redraw = no_redraw,
    audio = no_flush,
);

core_methods!(
    KuiWindow,
    core = core_mut,
    events = events_mut,
    take = take_events,
    redraw = request_redraw,
    audio = flush_audio,
);

/// Takes a batch of events the core just produced: the host's own are
/// queued for `pollEvents`, and one whose origin names a loaded extension
/// is delivered to it here instead — its replies queued in its place,
/// carrying its origin, window and key, so the host learns who answered
/// and about what (ADR 0014 decision 6).
///
/// This is `Shell::route_events` in the Rust runner and `KuiCtx::absorb`
/// in the C one — all three the same `Extensions::route`, for the same
/// reason: a reply is addressed by being one, not routed by origin. A
/// reply from a plugin *another extension* placed goes to that extension
/// first and arrives here as whatever it answered.
fn absorb(
    extensions: &mut kui_core::Extensions,
    events: impl IntoIterator<Item = UiEvent>,
    out: &mut Vec<UiEvent>,
) {
    extensions.route(events, |ev| out.push(ev));
}

/// Loads a C extension from `path` under `namespace` (empty = the
/// extension's own `kui_ext_name`), the way `kui_ctx_add_extension` does
/// for a C host. The mechanism is C shared libraries and only that: a
/// plugin is a `.so` / `.dylib` / `.dll` exporting the seven `kui_ext_*`
/// entry points `crates/kui-ffi/include/kui.h` describes.
///
/// # Safety
/// The library's code runs in this process, on this thread, on this app's
/// frame. Loading one is trusting it as much as linking it would be.
fn load_extension(
    extensions: &mut kui_core::Extensions,
    namespace: String,
    path: String,
) -> Result<()> {
    // SAFETY: the caller's, and `addExtension`'s doc comment says so.
    let ext = unsafe { CExtension::open(&path) }.map_err(err)?;
    extensions.push_as(namespace, Box::new(ext)).map_err(err)
}
/// `measureText(content, style, maxWidth)` → `{width, height, lines}`.
fn measure_text_impl(
    core: &mut Core,
    content: &Json,
    style: Option<&Json>,
    max_width: Option<f64>,
) -> Result<Json> {
    let props = style.and_then(Json::as_object).unwrap_or(empty_props());
    let style = parse_props_json(props)?.style;
    let max_w = max_width
        .filter(|w| w.is_finite() && *w > 0.0)
        .map(|w| w as f32);
    let m = if has_span(Some(content)) {
        let mut parts_out = Vec::new();
        collect_spans(Some(content), SpanStyle::default(), &mut parts_out)?;
        let spans: Vec<Span<'_>> = parts_out.iter().map(span_of).collect();
        core.measure_rich_text(&spans, &style, max_w)
    } else {
        let mut text = String::new();
        collect_text(Some(content), &mut text);
        core.measure_text(&text, &style, max_w)
    };
    let mut o = JsonMap::new();
    o.insert("width".into(), Json::from(m.width as f64));
    o.insert("height".into(), Json::from(m.height as f64));
    o.insert("lines".into(), Json::from(m.lines));
    Ok(Json::Object(o))
}

/// `access(key, action, arg)`: `arg` is the new text as a string
/// (`setValue`, `replaceSelectedText`), or `{anchor: {run, character},
/// focus: {run, character}}` for `setTextSelection` (run keys from the
/// node's `runs`; an object may also carry `text`).
fn access_request(key: Key, action: &str, arg: Option<Json>) -> Result<kui_core::AccessRequest> {
    let action = kui_core::AccessAction::parse(action)
        .ok_or_else(|| err(format!("unknown access action {action:?}")))?;
    let mut req = kui_core::AccessRequest::new(key, action);
    let pos = |v: &Json| -> Result<kui_core::TextPos> {
        let run = v
            .get("run")
            .and_then(Json::as_str)
            .ok_or_else(|| err("a text position needs a run key"))?;
        let character = v
            .get("character")
            .and_then(Json::as_u64)
            .ok_or_else(|| err("a text position needs a character index"))?;
        Ok(kui_core::TextPos {
            run: parse_key(run)?,
            character: character as usize,
        })
    };
    match arg {
        None | Some(Json::Null) => {}
        Some(Json::String(s)) => req.value = Some(s),
        Some(Json::Object(o)) => {
            if let Some(t) = o.get("text").and_then(Json::as_str) {
                req.value = Some(t.to_string());
            }
            if let (Some(a), Some(f)) = (o.get("anchor"), o.get("focus")) {
                req.anchor = Some(pos(a)?);
                req.focus = Some(pos(f)?);
            }
        }
        Some(other) => return Err(err(format!("bad access argument {other}"))),
    }
    Ok(req)
}

/// `{nodes: [...], focus, hash}`; see `AccessTree` in index.d.ts.
fn access_tree_json(tree: &kui_core::AccessTree) -> Json {
    let opt_str = |s: &Option<String>| s.as_ref().map_or(Json::Null, |s| Json::String(s.clone()));
    let opt_num = |v: Option<f32>| v.map_or(Json::Null, |v| Json::from(v as f64));
    let nodes = tree
        .nodes
        .iter()
        .map(|n| {
            let mut o = JsonMap::new();
            o.insert("key".into(), Json::String(key_str(n.key)));
            o.insert(
                "parent".into(),
                n.parent.map_or(Json::Null, |k| Json::String(key_str(k))),
            );
            o.insert("origin".into(), Json::from(n.origin.0));
            o.insert("role".into(), Json::String(n.role.name().into()));
            o.insert("name".into(), opt_str(&n.name));
            o.insert("description".into(), opt_str(&n.description));
            let mut rect = JsonMap::new();
            rect.insert("x".into(), Json::from(n.rect.x as f64));
            rect.insert("y".into(), Json::from(n.rect.y as f64));
            rect.insert("w".into(), Json::from(n.rect.w as f64));
            rect.insert("h".into(), Json::from(n.rect.h as f64));
            o.insert("rect".into(), Json::Object(rect));
            o.insert("value".into(), opt_str(&n.value));
            o.insert(
                "caret".into(),
                n.caret.map_or(Json::Null, |c| Json::from(c as u64)),
            );
            o.insert(
                "selection".into(),
                n.selection.map_or(Json::Null, |(a, b)| {
                    Json::Array(vec![Json::from(a as u64), Json::from(b as u64)])
                }),
            );
            let pos_json = |p: Option<kui_core::TextPos>| {
                p.map_or(Json::Null, |p| {
                    let mut o = JsonMap::new();
                    o.insert("run".into(), Json::String(key_str(p.run)));
                    o.insert("character".into(), Json::from(p.character as u64));
                    Json::Object(o)
                })
            };
            o.insert("anchor".into(), pos_json(n.anchor));
            o.insert("focus".into(), pos_json(n.focus));
            o.insert(
                "runs".into(),
                Json::Array(
                    n.runs
                        .iter()
                        .map(|r| {
                            let mut o = JsonMap::new();
                            o.insert("key".into(), Json::String(key_str(r.key)));
                            o.insert("line".into(), Json::from(r.line as u64));
                            o.insert("start".into(), Json::from(r.start as u64));
                            o.insert("end".into(), Json::from(r.end as u64));
                            o.insert("text".into(), Json::String(r.text.clone()));
                            let mut rect = JsonMap::new();
                            rect.insert("x".into(), Json::from(r.rect.x as f64));
                            rect.insert("y".into(), Json::from(r.rect.y as f64));
                            rect.insert("w".into(), Json::from(r.rect.w as f64));
                            rect.insert("h".into(), Json::from(r.rect.h as f64));
                            o.insert("rect".into(), Json::Object(rect));
                            let nums = |v: &[f32]| {
                                Json::Array(v.iter().map(|x| Json::from(*x as f64)).collect())
                            };
                            let bytes =
                                |v: &[u8]| Json::Array(v.iter().map(|x| Json::from(*x)).collect());
                            o.insert("charLengths".into(), bytes(&r.char_lengths));
                            o.insert("charPositions".into(), nums(&r.char_positions));
                            o.insert("charWidths".into(), nums(&r.char_widths));
                            o.insert("wordStarts".into(), bytes(&r.word_starts));
                            o.insert("rtl".into(), Json::Bool(r.rtl));
                            Json::Object(o)
                        })
                        .collect(),
                ),
            );
            o.insert("checked".into(), n.checked.map_or(Json::Null, Json::Bool));
            o.insert("selected".into(), n.selected.map_or(Json::Null, Json::Bool));
            o.insert("expanded".into(), n.expanded.map_or(Json::Null, Json::Bool));
            o.insert(
                "posInSet".into(),
                n.pos_in_set.map_or(Json::Null, |v| Json::from(v as u64)),
            );
            o.insert(
                "setSize".into(),
                n.set_size.map_or(Json::Null, |v| Json::from(v as u64)),
            );
            o.insert(
                "orientation".into(),
                n.orientation
                    .map_or(Json::Null, |o| Json::String(o.name().to_string())),
            );
            o.insert("live".into(), Json::String(n.live.name().into()));
            o.insert("valueNow".into(), opt_num(n.number));
            o.insert("valueMin".into(), opt_num(n.min));
            o.insert("valueMax".into(), opt_num(n.max));
            o.insert("focused".into(), Json::Bool(n.focused));
            o.insert("disabled".into(), Json::Bool(n.disabled));
            o.insert("modal".into(), Json::Bool(n.modal));
            o.insert(
                "scroll".into(),
                n.scroll.map_or(Json::Null, |s| {
                    let mut sc = JsonMap::new();
                    sc.insert("x".into(), Json::from(s.x as f64));
                    sc.insert("y".into(), Json::from(s.y as f64));
                    sc.insert("maxX".into(), Json::from(s.max_x as f64));
                    sc.insert("maxY".into(), Json::from(s.max_y as f64));
                    Json::Object(sc)
                }),
            );
            o.insert(
                "actions".into(),
                Json::Array(
                    n.action_list()
                        .into_iter()
                        .map(|a| Json::String(a.name().into()))
                        .collect(),
                ),
            );
            Json::Object(o)
        })
        .collect();
    let mut o = JsonMap::new();
    o.insert("nodes".into(), Json::Array(nodes));
    o.insert(
        "focus".into(),
        tree.focus.map_or(Json::Null, |k| Json::String(key_str(k))),
    );
    o.insert("hash".into(), Json::String(format!("{:016x}", tree.hash)));
    Json::Object(o)
}

fn warnings_json(warnings: Vec<kui_core::Warning>) -> Json {
    Json::Array(
        warnings
            .into_iter()
            .map(|w| {
                let mut o = JsonMap::new();
                o.insert("code".into(), Json::String(w.code.into()));
                o.insert("key".into(), Json::String(key_str(w.key)));
                o.insert("message".into(), Json::String(w.message));
                Json::Object(o)
            })
            .collect(),
    )
}

fn size_json(size: Size, scale: f32) -> Json {
    let mut o = JsonMap::new();
    o.insert("width".into(), Json::from(size.w as f64));
    o.insert("height".into(), Json::from(size.h as f64));
    o.insert("scale".into(), Json::from(scale as f64));
    Json::Object(o)
}

fn font_str(id: FontId) -> String {
    format!("{:016x}", id.to_ffi())
}

fn load_font_file_impl(core: &mut Core, path: &str) -> Result<String> {
    core.load_font_file(path).map(font_str).ok_or_else(|| {
        err(format!(
            "no usable font face in {path:?} (unreadable, or not TTF/OTF/TTC)"
        ))
    })
}

fn sound_str(id: SoundId) -> String {
    format!("{:016x}", id.to_ffi())
}

fn add_sound_impl(core: &mut Core, data: &[u8]) -> Result<String> {
    if data.is_empty() {
        return Err(err("addSound: empty buffer"));
    }
    Ok(sound_str(core.add_sound(data.to_vec())))
}

/// `<audio>` props to the core's spec; absent `volume` keeps the default.
fn audio_spec_of(
    src: SoundId,
    volume: Option<f64>,
    looped: bool,
    paused: bool,
    finish: bool,
    tag: Option<Value>,
) -> AudioSpec {
    let mut spec = AudioSpec::new(src).paused(paused);
    if let Some(v) = volume {
        spec = spec.volume(v as f32);
    }
    if looped {
        spec = spec.looped();
    }
    if finish {
        spec = spec.finish();
    }
    spec.tag = tag;
    spec
}

/// `play(sound, {volume, loop, fadeIn, tag})`; the playback id as a JS
/// number (a small counter, exact).
fn play_impl(core: &mut Core, sound: &str, opts: Option<&Json>) -> Result<f64> {
    let o = opts.and_then(Json::as_object).unwrap_or(empty_props());
    let mut po = PlayOptions::default();
    if let Some(v) = o.get("volume").and_then(Json::as_f64) {
        po.volume = v as f32;
    }
    po.looped = bool_prop(o, "loop");
    if let Some(v) = o.get("fadeIn").and_then(Json::as_f64) {
        po.fade_in_ms = v as f32;
    }
    po.tag = o.get("tag").map(value_of);
    Ok(core.play(SoundId::from_ffi(parse_u64(sound)?), po).0 as f64)
}

fn audio_commands_json(cmds: Vec<AudioCommand>) -> Json {
    let obj = |pairs: Vec<(&str, Json)>| {
        let mut o = JsonMap::new();
        for (k, v) in pairs {
            o.insert(k.into(), v);
        }
        Json::Object(o)
    };
    let pb = |p: PlaybackId| Json::from(p.0);
    Json::Array(
        cmds.into_iter()
            .map(|c| match c {
                AudioCommand::Play {
                    playback,
                    sound,
                    volume,
                    looped,
                    fade_in_ms,
                } => obj(vec![
                    ("kind", "play".into()),
                    ("playback", pb(playback)),
                    ("sound", Json::String(sound_str(sound))),
                    ("volume", Json::from(volume as f64)),
                    ("loop", Json::Bool(looped)),
                    ("fadeIn", Json::from(fade_in_ms as f64)),
                ]),
                AudioCommand::Stop { playback, fade_ms } => obj(vec![
                    ("kind", "stop".into()),
                    ("playback", pb(playback)),
                    ("fade", Json::from(fade_ms as f64)),
                ]),
                AudioCommand::SetVolume {
                    playback,
                    volume,
                    tween_ms,
                } => obj(vec![
                    ("kind", "setVolume".into()),
                    ("playback", pb(playback)),
                    ("volume", Json::from(volume as f64)),
                    ("tween", Json::from(tween_ms as f64)),
                ]),
                AudioCommand::Pause { playback, fade_ms } => obj(vec![
                    ("kind", "pause".into()),
                    ("playback", pb(playback)),
                    ("fade", Json::from(fade_ms as f64)),
                ]),
                AudioCommand::Resume { playback, fade_ms } => obj(vec![
                    ("kind", "resume".into()),
                    ("playback", pb(playback)),
                    ("fade", Json::from(fade_ms as f64)),
                ]),
                AudioCommand::MasterVolume { volume, tween_ms } => obj(vec![
                    ("kind", "masterVolume".into()),
                    ("volume", Json::from(volume as f64)),
                    ("tween", Json::from(tween_ms as f64)),
                ]),
                AudioCommand::Unload { sound } => obj(vec![
                    ("kind", "unload".into()),
                    ("sound", Json::String(sound_str(sound))),
                ]),
            })
            .collect(),
    )
}

fn add_font_impl(core: &mut Core, data: &[u8]) -> Result<String> {
    core.add_font_data(data.to_vec())
        .map(font_str)
        .ok_or_else(|| err("no usable font face in the data (expected TTF/OTF/TTC bytes)"))
}

/// `addFragment`: validate, then mint. The warning the core raises is not
/// enough on its own here — a JS caller expects a thrown error with the
/// message, not a handle of 0 and a diagnostic it has to go looking for —
/// so the message is read back out of the warning the core just raised.
fn add_fragment_impl(core: &mut Core, wgsl: &str) -> Result<String> {
    match core.add_fragment(wgsl) {
        Some(id) => Ok(format!("{:016x}", id.to_ffi())),
        None => {
            let why = core
                .take_warnings()
                .into_iter()
                .find(|w| w.code == kui_core::diag::FRAGMENT_REJECTED)
                .map(|w| w.message)
                .unwrap_or_else(|| "the fragment source does not compile".into());
            Err(err(why))
        }
    }
}

fn add_image_impl(core: &mut Core, width: u32, height: u32, rgba: &[u8]) -> Result<String> {
    let expected = (width as usize) * (height as usize) * 4;
    if width == 0 || height == 0 || rgba.len() != expected {
        return Err(err(format!(
            "rgba must be width*height*4 = {expected} bytes, got {}",
            rgba.len()
        )));
    }
    let id = core.resources.add_image(width, height, rgba.to_vec());
    Ok(format!("{:016x}", id.to_ffi()))
}

#[cfg(test)]
mod frame_stats_tests {
    use super::*;

    #[test]
    fn frame_stats_shape_before_and_after_frames() {
        let mut st = FrameStats::default();
        let empty = frame_stats_json(&st);
        assert_eq!(empty["frames"], Json::from(0));
        assert_eq!(empty["last"], Json::Null);
        assert_eq!(empty["avgTotalMs"], Json::from(0.0));

        st.push(FrameSample {
            input_ms: 0.5,
            view_ms: 1.0,
            layout_ms: 2.0,
            render_ms: 3.0,
            wait_ms: 4.0,
        });
        st.push(FrameSample {
            view_ms: 3.0,
            ..Default::default()
        });
        let o = frame_stats_json(&st);
        assert_eq!(o["frames"], Json::from(2));
        assert_eq!(o["last"]["viewMs"], Json::from(3.0));
        assert_eq!(o["last"]["totalMs"], Json::from(3.0));
        assert_eq!(o["maxTotalMs"], Json::from(10.5));
        assert_eq!(o["maxWorkMs"], Json::from(6.5));
        assert_eq!(o["avgTotalMs"], Json::from(6.75));
    }
}
