//! Node.js addon for kui (napi-rs). Like kui-ffi this layer is translation,
//! not architecture — but where C makes flat builder calls, JS submits a whole
//! frame at once: the jsx-runtime produces a plain-data element tree
//! (`{type, key, props, children}`), the JS package encodes it into the flat
//! binary IR stream ([`binary`]), and [`Ctx::frame_binary`] lowers it in one
//! zero-copy boundary crossing. That is the only way a frame gets in: there
//! is one element dispatcher ([`binary`]'s), not one per transport.
//! Event payloads are plain JSON both ways, which is exactly the Elm shape:
//! `onClick` carries a message value, never a closure.

use kui_core::{
    AudioCommand, AudioSpec, Color, Core, EditKey, FontId, FrameSample, FrameStats, ImageId,
    InputEvent, Key, KeyCode, KeyMods, KeyPress, Mods, MouseButton, PlayOptions, PlaybackId, Size,
    SoundId, Span, UiEvent, Value, Vec2,
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
}

#[napi]
impl Ctx {
    #[napi(constructor)]
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Ctx {
            core: Core::new(),
            events: Vec::new(),
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
        self.core
            .begin_frame(Size::new(width as f32, height as f32), scale as f32);
        let result = binary::lower_binary(&mut self.core, &stream, &strings);
        self.core.finish_frame();
        self.events.extend(self.core.take_pending_events());
        result
    }

    /// The frame clock for `transition` props: monotonic seconds, any
    /// origin. Set before each frame; never setting it makes transitions
    /// snap (the default for headless tests).
    #[napi]
    pub fn set_time(&mut self, now_secs: f64) {
        self.core.set_time(now_secs);
    }

    // -- Input (logical coordinates) ------------------------------------

    fn input(&mut self, ev: InputEvent) {
        self.events.extend(self.core.handle_input(ev));
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
    #[napi(ts_args_type = "code: string, mods?: KeySinkMods, repeat?: boolean")]
    pub fn key_down(
        &mut self,
        code: String,
        mods: Option<Json>,
        repeat: Option<bool>,
    ) -> Result<()> {
        let kp = self.key_press(&code, mods.as_ref())?;
        self.input(InputEvent::KeyDown(KeyPress {
            repeat: repeat.unwrap_or(false),
            ..kp
        }));
        Ok(())
    }

    /// The release of a key, spelled the way `keyDown` spells it: the sink
    /// hears `{kind:"key", phase:"up", ...}` with `text` null. A release
    /// whose press the sink never got resolves nothing, and moving focus
    /// while a key is held delivers the `up` first.
    #[napi(ts_args_type = "code: string, mods?: KeySinkMods")]
    pub fn key_up(&mut self, code: String, mods: Option<Json>) -> Result<()> {
        let kp = self.key_press(&code, mods.as_ref())?;
        self.input(InputEvent::KeyUp(kp.released()));
        Ok(())
    }

    /// One press from the `{shift, ctrl, alt, super}` shape both key calls
    /// take, with the text a plain key would insert already resolved.
    fn key_press(&self, code: &str, mods: Option<&Json>) -> Result<KeyPress> {
        let m = mods.and_then(Json::as_object).unwrap_or(empty_props());
        let kmods = KeyMods {
            shift: bool_prop(m, "shift"),
            ctrl: bool_prop(m, "ctrl"),
            alt: bool_prop(m, "alt"),
            super_key: bool_prop(m, "super"),
        };
        let code = keycode_of(code)?;
        let text = if !kmods.ctrl && !kmods.alt && !kmods.super_key {
            match code {
                KeyCode::Char(c) => Some(c.to_string()),
                KeyCode::Space => Some(" ".to_string()),
                _ => None,
            }
        } else {
            None
        };
        Ok(KeyPress {
            code,
            mods: kmods,
            text,
            repeat: false,
        })
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

    /// A custom driver reports a playback finished on its own; a tagged
    /// one becomes a `sound` event in `pollEvents`.
    #[napi]
    pub fn audio_ended(&mut self, playback: f64) {
        self.core.audio_ended(PlaybackId(playback as u64));
        self.events.extend(self.core.take_pending_events());
    }

    // -- Queries ---------------------------------------------------------

    /// The window title the last frame declared (a root `<box title>`), or
    /// null when it declared none. `runWindowed` applies it to the real
    /// window; a bare `Ctx` hands it back so a test can assert on it.
    #[napi]
    pub fn window_title(&self) -> Option<String> {
        self.core.window_title().map(str::to_string)
    }

    // -- Draw output ------------------------------------------------------

    /// Raw quads for the finished frame, `quadStride()` bytes each, laid out
    /// as kui-ffi's KuiQuad (see include/kui.h). Copied into the Buffer.
    #[napi]
    pub fn quads(&mut self) -> Buffer {
        let (dl, _) = self.core.output();
        let bytes = unsafe {
            std::slice::from_raw_parts(
                dl.quads.as_ptr().cast::<u8>(),
                std::mem::size_of_val(dl.quads.as_slice()),
            )
        };
        Buffer::from(bytes.to_vec())
    }
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

// ---------------------------------------------------------------------------
// Windowed runner (winit + wgpu via kui's PumpRunner)

/// The `kui::App` behind a Node window. JS never gets called from inside
/// winit: it stores the next view tree between pumps (`set_view`), and this
/// lowers the stored tree whenever the runner redraws. Events collect here
/// and JS drains them after each pump — the same data-only boundary as the
/// headless `Ctx`, now with a real window around it.
#[derive(Default)]
struct TreeApp {
    /// The last frame JS submitted: the binary IR stream and its string
    /// table, kept so a redraw between pumps can re-lower it.
    tree: Option<(Vec<f64>, Vec<u8>)>,
    events: Vec<UiEvent>,
    error: Option<String>,
}

impl kui::App for TreeApp {
    fn view(&mut self, ui: &mut kui::Ui<'_>) {
        let Some((stream, strings)) = &self.tree else {
            return;
        };
        if let Err(e) = binary::lower_binary(ui.core(), stream, strings) {
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
/// you are building your own loop. One window per process; winit event loops
/// are not recreatable on every platform.
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
    #[napi]
    pub fn set_view_binary(&mut self, stream: Float64Array, strings: Uint8Array) {
        self.runner.app_mut().tree = Some((stream.to_vec(), strings.to_vec()));
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

/// Pending UI events as `[{origin, key, payload}]` — payloads plain JSON, so
/// your Elm messages come back out as data.
fn events_json(events: Vec<UiEvent>) -> Json {
    Json::Array(
        events
            .into_iter()
            .map(|ev| {
                let mut o = JsonMap::new();
                o.insert("origin".into(), Json::from(ev.origin.0));
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

            /// Events since the last poll: `[{origin, key, payload}]`,
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
                self.$events().extend(pending);
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

            /// Summary of the last frame's display list.
            #[napi(ts_return_type = "FrameStats")]
            pub fn stats(&mut self) -> Json {
                stats_json(self.$core())
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
            /// screenshot.
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

            // -- Accessibility ---------------------------------------------

            /// What assistive technology sees of the last frame (see
            /// `AccessTree`). A window hands it to the platform by itself
            /// (AccessKit); this is for tests and tooling.
            #[napi(ts_return_type = "AccessTree")]
            pub fn access_tree(&mut self) -> Json {
                access_tree_json(self.$core().access_tree())
            }

            /// A request from assistive technology on a node (`key`, hex as in
            /// events): an `AccessAction` name the node advertises, with
            /// `value` the new text for `setValue`. Resolved like its
            /// pointer/keyboard equivalent, so the resulting events come out of
            /// `pollEvents`. A real screen reader's requests arrive through a
            /// window on their own.
            #[napi(ts_args_type = "key: string, action: AccessAction, value?: string | AccessArg")]
            pub fn access(
                &mut self,
                key: String,
                action: String,
                value: Option<Json>,
            ) -> Result<()> {
                let req = access_request(&key, &action, value)?;
                let events = self.$core().handle_input(InputEvent::Access(req));
                self.$events().extend(events);
                self.$redraw();
                Ok(())
            }

            // -- Hover / press ---------------------------------------------

            /// Hover state as of the last frame (keys come from events, e.g.
            /// an `onHover` enter). For plain hover styling prefer the
            /// `hoverBg` / `pressedBg` props — the core resolves those without
            /// a round trip.
            #[napi]
            pub fn is_hovered(&mut self, key: String) -> Result<bool> {
                Ok(self.$core().is_hovered(parse_key(&key)?))
            }

            #[napi]
            pub fn is_pressed(&mut self, key: String) -> Result<bool> {
                Ok(self.$core().is_pressed(parse_key(&key)?))
            }

            // -- Focus ------------------------------------------------------

            /// Whether a node holds keyboard focus — any node: an editor, an
            /// `onKey` sink, a button Tab landed on (see `focused`).
            #[napi]
            pub fn is_focused(&mut self, key: String) -> Result<bool> {
                Ok(self.$core().is_focused(parse_key(&key)?))
            }

            /// The node holding keyboard focus (hex key), or null. Tab /
            /// Shift-Tab (`key("tab")`) walk every control in tree order,
            /// Enter and Space press the focused one, and the arrows nudge a
            /// focused slider.
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
            #[napi]
            pub fn focus(&mut self, key: String) -> Result<()> {
                self.$core().set_focus(Some(parse_key(&key)?));
                self.$redraw();
                Ok(())
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
            /// the last wins.
            #[napi]
            pub fn reveal(&mut self, key: String) -> Result<()> {
                self.$core().reveal(parse_key(&key)?);
                self.$redraw();
                Ok(())
            }

            /// A scroll container's retained offset `{x, y}` as the last layout
            /// clamped it (positive = content moved up / left) — the number to
            /// keep in a model and hand back to `setScroll`. Zero for a node
            /// that never scrolled.
            #[napi(ts_return_type = "ScrollOffset")]
            pub fn scroll_offset(&mut self, key: String) -> Result<Json> {
                Ok(offset_json(self.$core().scroll_offset(parse_key(&key)?)))
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
                Ok(geometry_json(
                    self.$core().scroll_geometry(parse_key(&key)?),
                ))
            }

            /// Sets that offset the way the wheel would; the next frame's
            /// layout clamps it, so `(0, 0)` jumps to the top and a huge `y` to
            /// the end without knowing the content height.
            #[napi]
            pub fn set_scroll(&mut self, key: String, x: f64, y: f64) -> Result<()> {
                self.$core()
                    .set_scroll(parse_key(&key)?, Vec2::new(x as f32, y as f32));
                self.$redraw();
                Ok(())
            }

            // -- Editors ----------------------------------------------------

            #[napi]
            pub fn edit_text(&mut self, key: String) -> Result<Option<String>> {
                Ok(self.$core().edit_text(parse_key(&key)?))
            }

            #[napi]
            pub fn set_edit_text(&mut self, key: String, text: String) -> Result<()> {
                self.$core().set_edit_text(parse_key(&key)?, &text);
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
    redraw = no_redraw,
    audio = no_flush,
);

core_methods!(
    KuiWindow,
    core = core_mut,
    events = events_mut,
    redraw = request_redraw,
    audio = flush_audio,
);

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
fn access_request(key: &str, action: &str, arg: Option<Json>) -> Result<kui_core::AccessRequest> {
    let action = kui_core::AccessAction::parse(action)
        .ok_or_else(|| err(format!("unknown access action {action:?}")))?;
    let mut req = kui_core::AccessRequest::new(parse_key(key)?, action);
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
    tag: Option<Value>,
) -> AudioSpec {
    let mut spec = AudioSpec::new(src).paused(paused);
    if let Some(v) = volume {
        spec = spec.volume(v as f32);
    }
    if looped {
        spec = spec.looped();
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
