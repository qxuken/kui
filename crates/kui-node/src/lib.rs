//! Node.js addon for kui (napi-rs). Like kui-ffi this layer is translation,
//! not architecture — but where C makes flat builder calls, JS submits a whole
//! frame at once: the jsx-runtime produces a plain-data element tree
//! (`{type, key, props, children}`), the JS package encodes it into the flat
//! binary IR stream ([`binary`]), and [`Ctx::frame_binary`] lowers it in one
//! zero-copy boundary crossing. [`Ctx::frame_object`] (napi walking the
//! object graph) and [`Ctx::frame_json`] are the slower reference transports.
//! Event payloads are plain JSON both ways, which is exactly the Elm shape:
//! `onClick` carries a message value, never a closure.

use kui_core::{
    Align, AudioCommand, AudioSpec, Color, Core, EditKey, EditOptions, FontId, FrameSample,
    FrameStats, ImageId, InputEvent, Key, KeyCode, KeyMods, KeyPress, Mods, MouseButton,
    PlayOptions, PlaybackId, Size, SoundId, Span, TextStyle, UiEvent, Value, Vec2,
};
use napi::bindgen_prelude::{Buffer, Float64Array, Uint8Array};
use napi_derive::napi;
use serde_json::{Map as JsonMap, Value as Json};

mod binary;
mod schema;

use schema::{align_of, color_of, parse_props_json};

type Result<T> = napi::Result<T>;

fn err(msg: impl AsRef<str>) -> napi::Error {
    napi::Error::from_reason(msg.as_ref())
}

/// The binary-frame protocol tables (`{version, op, prop}`). The JS encoder
/// reads its opcodes and prop ids from here at module init, so the two sides
/// cannot drift.
#[napi]
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
// Prop parsing

fn bool_prop(props: &JsonMap<String, Json>, key: &str) -> bool {
    props.get(key).and_then(Json::as_bool).unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Element-tree lowering

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

fn lower(core: &mut Core, node: &Json) -> Result<()> {
    match node {
        Json::Null | Json::Bool(_) => Ok(()),
        Json::String(s) => {
            core.text_node(s, TextStyle::default());
            Ok(())
        }
        Json::Number(n) => {
            core.text_node(&n.to_string(), TextStyle::default());
            Ok(())
        }
        Json::Array(items) => {
            for item in items {
                lower(core, item)?;
            }
            Ok(())
        }
        Json::Object(o) => lower_element(core, o),
    }
}

fn lower_element(core: &mut Core, node: &JsonMap<String, Json>) -> Result<()> {
    let ty = node
        .get("type")
        .and_then(Json::as_str)
        .ok_or_else(|| err("element without a type — did it come from kui/jsx-runtime?"))?;
    let (props, children, key) = parts(node);
    match ty {
        "box" => {
            let p = parse_props_json(props)?;
            let node_key = match key {
                Some(label) => core.open_keyed(label, p.spec),
                None => core.open(p.spec),
            };
            // A key sink can grab the keyboard declaratively (modal apps);
            // the core still routes to a focused edit widget first.
            if p.key_focus {
                core.set_key_focus(Some(node_key));
            }
            lower(core, children.unwrap_or(&Json::Null))?;
            // Hover hint: floats below the box while hovered (the parser
            // made the spec hoverable).
            if let Some(hint) = &p.tooltip
                && core.is_hovered(node_key)
            {
                kui_core::widgets::tooltip(&mut kui_core::Ui::wrap(core), hint);
            }
            core.close();
            Ok(())
        }
        "text" => {
            let style = parse_props_json(props)?.style;
            if has_span(children) {
                let mut parts_out = Vec::new();
                collect_spans(children, SpanStyle::default(), &mut parts_out)?;
                let spans: Vec<Span<'_>> = parts_out.iter().map(span_of).collect();
                core.rich_text_node(&spans, style);
            } else {
                let mut content = String::new();
                collect_text(children, &mut content);
                core.text_node(&content, style);
            }
            Ok(())
        }
        "span" => Err(err("<span> only works inside <text>")),
        "button" => {
            let mut label = String::new();
            collect_text(children, &mut label);
            let msg = props.get("onClick").map(value_of).unwrap_or(Value::Null);
            let label_key = key.unwrap_or(&label);
            // The same data as kui_core::widgets::button: hover/pressed
            // colors are declared on the spec, resolved by the core.
            core.open_keyed(label_key, kui_core::widgets::button_spec().on_click(msg));
            core.text_node(
                &label,
                TextStyle::new(kui_core::widgets::BUTTON_TEXT).color(Color::WHITE),
            );
            core.close();
            Ok(())
        }
        "edit" => {
            let label = key
                .or_else(|| props.get("id").and_then(Json::as_str))
                .ok_or_else(|| err("<edit> needs a key or id prop (state is retained by key)"))?;
            let initial = props.get("initial").and_then(Json::as_str).unwrap_or("");
            let p = parse_props_json(props)?;
            let opts = EditOptions {
                style: p.style,
                multiline: bool_prop(props, "multiline"),
                autofocus: bool_prop(props, "autofocus"),
                ..Default::default()
            };
            core.text_edit(label, initial, &opts, p.spec);
            Ok(())
        }
        "image" => {
            let src = props
                .get("src")
                .and_then(Json::as_str)
                .ok_or_else(|| err("<image> needs a src (an id from addImage)"))?;
            core.image_node(
                ImageId::from_ffi(parse_u64(src)?),
                parse_props_json(props)?.spec,
            );
            Ok(())
        }
        // A retained playback keyed by node; see `Core::audio_node`.
        "audio" => {
            let src = props
                .get("src")
                .and_then(Json::as_str)
                .ok_or_else(|| err("<audio> needs a src (an id from addSound)"))?;
            let spec = audio_spec_of(
                SoundId::from_ffi(parse_u64(src)?),
                props.get("volume").and_then(Json::as_f64),
                bool_prop(props, "loop"),
                bool_prop(props, "paused"),
                props.get("tag").map(value_of),
            );
            match key {
                Some(label) => core.audio_node_keyed(label, spec),
                None => core.audio_node(spec),
            };
            Ok(())
        }
        // Adaptive titlebar: drag strip + window buttons per env.window facts.
        // With children it hosts custom content (tabs etc.); with only a
        // `title` prop it draws the standard centered-left title.
        "titlebar" => {
            let empty = match children {
                None | Some(Json::Null) => true,
                Some(Json::Array(a)) => a.is_empty(),
                _ => false,
            };
            let mut ui = kui_core::Ui::wrap(core);
            if empty {
                let title = props.get("title").and_then(Json::as_str).unwrap_or("");
                kui_core::widgets::titlebar(&mut ui, title);
                Ok(())
            } else {
                let mut result = Ok(());
                kui_core::widgets::titlebar_with(&mut ui, |ui| {
                    result = lower(ui.core(), children.unwrap_or(&Json::Null));
                });
                result
            }
        }
        "windowButtons" => {
            kui_core::widgets::window_buttons(&mut kui_core::Ui::wrap(core));
            Ok(())
        }
        // Per-phase frame-latency bars vs the display's budget. Reads
        // core.stats — populated by the windowed runner; empty when headless.
        "latencyGraph" => {
            kui_core::widgets::latency_graph(&mut kui_core::Ui::wrap(core));
            Ok(())
        }
        // The graph in a translucent panel floating in a viewport corner.
        // `at` picks the corner, [x, y] align values; default end/end.
        "latencyHud" => {
            let (mut x, mut y) = (Align::End, Align::End);
            if let Some(at) = props.get("at").and_then(Json::as_array)
                && at.len() == 2
            {
                x = align_of(&at[0])?;
                y = align_of(&at[1])?;
            }
            kui_core::widgets::latency_hud_at(&mut kui_core::Ui::wrap(core), x, y);
            Ok(())
        }
        "fragment" => lower(core, children.unwrap_or(&Json::Null)),
        other => Err(err(format!("unknown element <{other}>"))),
    }
}

/// Lowers a whole frame: a root `<box>` configures the root node; anything
/// else becomes a child of a default column root.
fn lower_root(core: &mut Core, tree: &Json) -> Result<()> {
    if let Json::Object(o) = tree
        && o.get("type").and_then(Json::as_str) == Some("box")
    {
        let (props, children, _) = parts(o);
        let p = parse_props_json(props)?;
        // The root box may declare this frame's window title; the windowed
        // driver diffs and applies it.
        if let Some(t) = &p.title {
            core.set_window_title(t);
        }
        core.configure_root(p.spec);
        if p.key_focus {
            core.set_key_focus(Some(core.root_key()));
        }
        return lower(core, children.unwrap_or(&Json::Null));
    }
    lower(core, tree)
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
    let mut chars = s.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        return Ok(KeyCode::Char(c));
    }
    if let Some(n) = s.strip_prefix('f').and_then(|n| n.parse::<u8>().ok())
        && (1..=24).contains(&n)
    {
        return Ok(KeyCode::F(n));
    }
    Ok(match s {
        "space" => KeyCode::Space,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "pageup" => KeyCode::PageUp,
        "pagedown" => KeyCode::PageDown,
        "backspace" => KeyCode::Backspace,
        "delete" => KeyCode::Delete,
        "enter" => KeyCode::Enter,
        "tab" => KeyCode::Tab,
        "escape" => KeyCode::Escape,
        "insert" => KeyCode::Insert,
        other => return Err(err(format!("unknown key code {other:?}"))),
    })
}

fn key_str(key: Key) -> String {
    format!("{:016x}", key.0)
}

/// A scroll offset as `{x, y}` — the shape `setScroll` takes back.
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

    /// Builds one frame from a jsx-runtime element tree, napi walking the JS
    /// object graph property by property (each read is an N-API call into
    /// V8) — the reference transport, ~10x slower than `frame_binary`. The
    /// package's `Ctx.frame` encodes to the binary stream instead. A root
    /// `<box>` configures the root node; anything else becomes a child of a
    /// default column root.
    #[napi]
    pub fn frame_object(&mut self, width: f64, height: f64, scale: f64, tree: Json) -> Result<()> {
        let scale = if scale > 0.0 { scale } else { 1.0 };
        self.core
            .begin_frame(Size::new(width as f32, height as f32), scale as f32);
        let result = lower_root(&mut self.core, &tree);
        // Finish even on lowering errors so the context stays usable.
        self.core.finish_frame();
        self.events.extend(self.core.take_pending_events());
        result
    }

    /// `frame_object` with the tree as a JSON string: one string crosses the
    /// boundary and serde parses it. Readable on the wire, so it is the
    /// debugging transport when the binary encoder is suspect.
    #[napi]
    pub fn frame_json(&mut self, width: f64, height: f64, scale: f64, tree: String) -> Result<()> {
        let tree: Json =
            serde_json::from_str(&tree).map_err(|e| err(format!("bad frame JSON: {e}")))?;
        self.frame_object(width, height, scale, tree)
    }

    /// `frame` with the tree as a flat binary instruction stream (see
    /// `binary.rs` and the JS encoder) — the fastest path, and what the JS
    /// drivers use. Buffers are read zero-copy.
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

    /// True when the last frame left a transition mid-flight.
    #[napi]
    pub fn animating(&self) -> bool {
        self.core.animating()
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

    /// `clicks`: 1 single, 2 double (word select), 3 triple (line select).
    /// `button`: "primary" (the default), "secondary" — which asks the node
    /// under the pointer for a context menu and moves nothing else — or
    /// "middle", which nothing routes yet.
    #[napi]
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
    #[napi]
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
    /// focused editors still go through `key()`.
    #[napi]
    pub fn key_down(&mut self, code: String, mods: Option<Json>) -> Result<()> {
        let m = mods
            .as_ref()
            .and_then(Json::as_object)
            .unwrap_or(empty_props());
        let kmods = KeyMods {
            shift: bool_prop(m, "shift"),
            ctrl: bool_prop(m, "ctrl"),
            alt: bool_prop(m, "alt"),
            super_key: bool_prop(m, "super"),
        };
        let code = keycode_of(&code)?;
        let text = if !kmods.ctrl && !kmods.alt && !kmods.super_key {
            match code {
                KeyCode::Char(c) => Some(c.to_string()),
                KeyCode::Space => Some(" ".to_string()),
                _ => None,
            }
        } else {
            None
        };
        self.input(InputEvent::KeyDown(KeyPress {
            code,
            mods: kmods,
            text,
            repeat: false,
        }));
        Ok(())
    }

    /// Physical modifier state changed: `{shift, ctrl, alt, super}`. The
    /// host receives `{kind:"modifiers", ...}` when it differs from the
    /// last report.
    #[napi]
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

    /// Registers a w×h RGBA image (pixels copied); returns its id for
    /// `<image src={id}>`. Stable until `removeImage`.
    #[napi]
    pub fn add_image(&mut self, width: u32, height: u32, rgba: Buffer) -> Result<String> {
        add_image_impl(&mut self.core, width, height, &rgba)
    }

    #[napi]
    pub fn remove_image(&mut self, id: String) -> Result<()> {
        self.core.remove_image(ImageId::from_ffi(parse_u64(&id)?));
        Ok(())
    }

    /// Registers a font from file bytes (TTF/OTF/TTC); returns its id for
    /// the `font` prop on `<text>` / `<edit>`. Throws when the data holds
    /// no usable face.
    #[napi]
    pub fn add_font(&mut self, data: Buffer) -> Result<String> {
        add_font_impl(&mut self.core, &data)
    }

    /// Registers an installed font by family name; null when none matches
    /// (see `systemFontFamilies`). Also finds families loaded with
    /// `loadFontsDir` / `loadFontFile`; the same family gets the same id.
    #[napi]
    pub fn add_system_font(&mut self, name: String) -> Option<String> {
        self.core.add_system_font(&name).map(font_str)
    }

    /// Registers a font file by path (memory-mapped); throws when it cannot
    /// be read or holds no usable face.
    #[napi]
    pub fn load_font_file(&mut self, path: String) -> Result<String> {
        load_font_file_impl(&mut self.core, &path)
    }

    /// Loads every font file under a folder (recursively) so its families
    /// can be picked by name with `addSystemFont`; returns the face count.
    #[napi]
    pub fn load_fonts_dir(&mut self, dir: String) -> u32 {
        self.core.load_fonts_dir(&dir) as u32
    }

    #[napi]
    pub fn remove_font(&mut self, id: String) -> Result<()> {
        self.core.remove_font(FontId::from_ffi(parse_u64(&id)?));
        Ok(())
    }

    /// Family names of every installed font (sorted).
    #[napi]
    pub fn system_font_families(&self) -> Vec<String> {
        self.core.system_font_families()
    }

    // -- Audio ----------------------------------------------------------
    // Headless: nothing plays; the commands queue up for `audioCommands`
    // (tests, custom drivers). `KuiWindow` has the same calls with a device
    // behind them.

    /// Registers a sound from its encoded bytes (wav/ogg/mp3/flac); returns
    /// its id for `<audio src>`, the `clickSound` / `hoverSound` props and
    /// `play`. Stable until `removeSound`.
    #[napi]
    pub fn add_sound(&mut self, data: Buffer) -> Result<String> {
        add_sound_impl(&mut self.core, &data)
    }

    #[napi]
    pub fn remove_sound(&mut self, id: String) -> Result<()> {
        self.core.remove_sound(SoundId::from_ffi(parse_u64(&id)?));
        Ok(())
    }

    /// Starts a playback: `{volume, loop, fadeIn, tag}`; returns its id for
    /// `stop` / `setVolume` / `pause` / `resume`. A `tag` comes back as
    /// `{kind:"sound", phase:"ended", playback, tag}` when the playback
    /// finishes on its own.
    #[napi]
    pub fn play(&mut self, sound: String, opts: Option<Json>) -> Result<f64> {
        play_impl(&mut self.core, &sound, opts.as_ref())
    }

    #[napi]
    pub fn stop(&mut self, playback: f64, fade_ms: Option<f64>) {
        self.core
            .stop(PlaybackId(playback as u64), fade_ms.unwrap_or(0.0) as f32);
    }

    #[napi]
    pub fn set_volume(&mut self, playback: f64, volume: f64, tween_ms: Option<f64>) {
        self.core.set_volume(
            PlaybackId(playback as u64),
            volume as f32,
            tween_ms.unwrap_or(0.0) as f32,
        );
    }

    #[napi]
    pub fn pause(&mut self, playback: f64, fade_ms: Option<f64>) {
        self.core
            .pause(PlaybackId(playback as u64), fade_ms.unwrap_or(0.0) as f32);
    }

    #[napi]
    pub fn resume(&mut self, playback: f64, fade_ms: Option<f64>) {
        self.core
            .resume(PlaybackId(playback as u64), fade_ms.unwrap_or(0.0) as f32);
    }

    #[napi]
    pub fn set_master_volume(&mut self, volume: f64, tween_ms: Option<f64>) {
        self.core
            .set_master_volume(volume as f32, tween_ms.unwrap_or(0.0) as f32);
    }

    /// Drains the audio commands the core queued, as plain objects
    /// (`{kind:"play", playback, sound, volume, loop, fadeIn}`, ...) — what a
    /// windowed driver would play. For tests and custom drivers.
    #[napi]
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

    /// Drains pending UI events: `[{origin, key, payload}]`, payloads as
    /// plain JSON (your Elm messages come back out here).
    #[napi]
    pub fn poll_events(&mut self) -> Json {
        let events = std::mem::take(&mut self.events);
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

    // -- Queries ---------------------------------------------------------

    /// Measures text the way layout would, without adding a node:
    /// `{width, height, lines}` in logical px, wrapped to `maxWidth` when
    /// given. `content` is whatever `<text>` takes (a string, or children
    /// with `<span>`s); `style` the `<text>` props (`size`, `font`, `wrap`,
    /// `maxLines`, `ellipsis`, ...). Works before the first frame.
    #[napi]
    pub fn measure_text(
        &mut self,
        content: Json,
        style: Option<Json>,
        max_width: Option<f64>,
    ) -> Result<Json> {
        measure_text_impl(&mut self.core, &content, style.as_ref(), max_width)
    }

    /// Drains the warnings the core raised since the last call:
    /// `[{code, key, message}]`, each distinct (code, node) pair once. See
    /// `Warning` in index.d.ts.
    #[napi]
    pub fn warnings(&mut self) -> Json {
        warnings_json(self.core.take_warnings())
    }

    /// The window title the last frame declared (a root `<box title>`), or
    /// null when it declared none. `runWindowed` applies it to the real
    /// window; a bare `Ctx` hands it back so a test can assert on it.
    #[napi]
    pub fn window_title(&self) -> Option<String> {
        self.core.window_title().map(str::to_string)
    }

    /// The access tree of the last frame — what assistive technology
    /// sees; see `AccessTree` in index.d.ts.
    #[napi]
    pub fn access_tree(&mut self) -> Json {
        access_tree_json(self.core.access_tree())
    }

    /// A request from assistive technology on a node (`key`, hex as in
    /// events): an `AccessAction` name the node advertises, with `value`
    /// the new text for `setValue`. Resolved like its pointer/keyboard
    /// equivalent, so the resulting events come out of `pollEvents`.
    #[napi]
    pub fn access(&mut self, key: String, action: String, value: Option<Json>) -> Result<()> {
        let req = access_request(&key, &action, value)?;
        self.input(InputEvent::Access(req));
        Ok(())
    }

    /// Turns the per-frame diagnostic checks behind `warnings` on or off.
    #[napi]
    pub fn set_diagnostics(&mut self, on: bool) {
        self.core.set_diagnostics(on);
    }

    #[napi]
    pub fn is_hovered(&self, key: String) -> Result<bool> {
        Ok(self.core.is_hovered(parse_key(&key)?))
    }

    #[napi]
    pub fn is_pressed(&self, key: String) -> Result<bool> {
        Ok(self.core.is_pressed(parse_key(&key)?))
    }

    /// Whether a node holds keyboard focus — any node: an editor, an
    /// `onKey` sink, a button Tab landed on (see `focused`).
    #[napi]
    pub fn is_focused(&self, key: String) -> Result<bool> {
        Ok(self.core.is_focused(parse_key(&key)?))
    }

    /// The node holding keyboard focus (hex key), or null.
    #[napi]
    pub fn focused(&self) -> Option<String> {
        self.core.focus().map(key_str)
    }

    /// Whether focus got where it is by keyboard or assistive technology
    /// rather than a click — when it shows (the ring, or `focusBg`).
    #[napi]
    pub fn focus_visible(&self) -> bool {
        self.core.focus_visible()
    }

    /// Moves keyboard focus to a node now (an editor, an `onKey` sink, a
    /// control, a `focusable` box); `keyFocus` on a box is the declarative,
    /// edge-triggered form.
    #[napi]
    pub fn focus(&mut self, key: String) -> Result<()> {
        self.core.set_focus(Some(parse_key(&key)?));
        Ok(())
    }

    #[napi]
    pub fn blur(&mut self) {
        self.core.set_focus(None);
    }

    /// What Tab does: the next focusable node in tree order, wrapping.
    #[napi]
    pub fn focus_next(&mut self) {
        self.core.focus_next(true);
    }

    /// What Shift-Tab does.
    #[napi]
    pub fn focus_prev(&mut self) {
        self.core.focus_next(false);
    }

    /// Scrolls whatever contains a node so it shows — "scroll to the
    /// selected row", which needs the container geometry only the core has.
    /// The request resolves against the *next* frame's layout (one is
    /// requested), so a row the view is about to declare for the first
    /// time reveals fine. If that frame does not declare the key, or
    /// nothing above it scrolls, it is a no-op and is not kept for a later
    /// frame; two reveals before one frame are contradictory, so the last
    /// wins.
    #[napi]
    pub fn reveal(&mut self, key: String) -> Result<()> {
        self.core.reveal(parse_key(&key)?);
        Ok(())
    }

    /// A scroll container's retained offset `{x, y}` as the last layout
    /// clamped it (positive = content moved up / left) — the number to keep
    /// in a model and hand back to `setScroll`. Zero for a node that never
    /// scrolled.
    #[napi]
    pub fn scroll_offset(&self, key: String) -> Result<Json> {
        Ok(offset_json(self.core.scroll_offset(parse_key(&key)?)))
    }

    /// Sets that offset the way the wheel would; the next frame's layout
    /// clamps it, so `(0, 0)` jumps to the top and a huge `y` to the end
    /// without knowing the content height.
    #[napi]
    pub fn set_scroll(&mut self, key: String, x: f64, y: f64) -> Result<()> {
        self.core
            .set_scroll(parse_key(&key)?, Vec2::new(x as f32, y as f32));
        Ok(())
    }

    #[napi]
    pub fn edit_text(&self, key: String) -> Result<Option<String>> {
        Ok(self.core.edit_text(parse_key(&key)?))
    }

    #[napi]
    pub fn set_edit_text(&mut self, key: String, text: String) -> Result<()> {
        self.core.set_edit_text(parse_key(&key)?, &text);
        Ok(())
    }

    // -- Draw output ------------------------------------------------------

    /// Summary of the finished frame's display list.
    #[napi]
    pub fn stats(&mut self) -> Json {
        stats_json(&mut self.core)
    }

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
enum ViewData {
    Json(Json),
    Binary(Vec<f64>, Vec<u8>),
}

#[derive(Default)]
struct TreeApp {
    tree: Option<ViewData>,
    events: Vec<UiEvent>,
    error: Option<String>,
}

impl kui::App for TreeApp {
    fn view(&mut self, ui: &mut kui::Ui<'_>) {
        let result = match &self.tree {
            None => return,
            Some(ViewData::Json(tree)) => lower_root(ui.core(), tree),
            Some(ViewData::Binary(stream, strings)) => {
                binary::lower_binary(ui.core(), stream, strings)
            }
        };
        if let Err(e) = result {
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

/// A real kui window driven from Node. The event loop is pumped, not run:
/// call `pump()` from a timer loop (see `runWindowed` in the JS package) so
/// winit and libuv share the main thread. One window per process — winit
/// event loops are not recreatable on every platform.
#[napi]
pub struct KuiWindow {
    runner: kui::PumpRunner<TreeApp>,
}

#[napi]
impl KuiWindow {
    /// Options: `{width, height, minWidth, minHeight, maxWidth, maxHeight,
    /// chrome: "native" | "custom" | "borderless"}`. The min/max pairs bound
    /// what the user can resize the window to; either half may stand alone.
    #[napi(constructor)]
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

    /// Stores the tree future redraws lower, and schedules one — napi walks
    /// the object graph (see `Ctx::frame_object`); the package's `setView`
    /// encodes to the binary stream instead.
    #[napi]
    pub fn set_view_object(&mut self, tree: Json) {
        self.runner.app_mut().tree = Some(ViewData::Json(tree));
        self.runner.request_redraw();
    }

    /// `setViewObject` with the tree as a JSON string (see `Ctx::frame_json`).
    #[napi]
    pub fn set_view_json(&mut self, tree: String) -> Result<()> {
        let tree: Json =
            serde_json::from_str(&tree).map_err(|e| err(format!("bad view JSON: {e}")))?;
        self.set_view_object(tree);
        Ok(())
    }

    /// `setView` with a flat binary instruction stream (see `Ctx::frame_binary`).
    /// Copied once so redraws (resize, hover) can re-lower it between pumps.
    #[napi]
    pub fn set_view_binary(&mut self, stream: Float64Array, strings: Uint8Array) {
        self.runner.app_mut().tree = Some(ViewData::Binary(stream.to_vec(), strings.to_vec()));
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
    /// scale}` event through `pollEvents` whenever the window changes size
    /// or moves to a display with another DPI.
    #[napi]
    pub fn size(&self) -> Json {
        let (size, scale) = self.runner.window_size();
        size_json(size, scale)
    }

    /// True when the last frame left a transition mid-flight. The window
    /// schedules its own redraws for that; this is for tests and drivers
    /// that want to know when motion has settled.
    #[napi]
    pub fn animating(&mut self) -> bool {
        self.runner.core_mut().animating()
    }

    /// Summary of the last frame's display list (same shape as `Ctx.stats`).
    #[napi]
    pub fn stats(&mut self) -> Json {
        stats_json(self.runner.core_mut())
    }

    /// Frame timing measured by the runner — what the latency HUD draws,
    /// as data: `{frames, last: {inputMs, viewMs, layoutMs, renderMs,
    /// waitMs, totalMs, workMs} | null, avgTotalMs, maxTotalMs, avgWorkMs,
    /// maxWorkMs}` over the last 120 frames. `waitMs` is vsync
    /// backpressure; `workMs` is everything else.
    #[napi]
    pub fn frame_stats(&mut self) -> Json {
        frame_stats_json(&self.runner.core_mut().stats)
    }

    /// Drains UI events collected since the last call (same shape as
    /// `Ctx.pollEvents`).
    #[napi]
    pub fn poll_events(&mut self) -> Json {
        let events = std::mem::take(&mut self.runner.app_mut().events);
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

    /// Asks the window to close; the next pump returns false.
    #[napi]
    pub fn close(&mut self) {
        self.runner.request_exit();
    }

    #[napi]
    pub fn edit_text(&mut self, key: String) -> Result<Option<String>> {
        Ok(self.runner.core_mut().edit_text(parse_key(&key)?))
    }

    #[napi]
    pub fn set_edit_text(&mut self, key: String, text: String) -> Result<()> {
        self.runner
            .core_mut()
            .set_edit_text(parse_key(&key)?, &text);
        self.runner.request_redraw();
        Ok(())
    }

    #[napi]
    pub fn is_focused(&mut self, key: String) -> Result<bool> {
        Ok(self.runner.core_mut().is_focused(parse_key(&key)?))
    }

    /// The node holding keyboard focus (hex key), or null.
    #[napi]
    pub fn focused(&mut self) -> Option<String> {
        self.runner.core_mut().focus().map(key_str)
    }

    /// Whether focus got where it is by keyboard or assistive technology
    /// rather than a click (when the ring / `focusBg` shows).
    #[napi]
    pub fn focus_visible(&mut self) -> bool {
        self.runner.core_mut().focus_visible()
    }

    /// Moves keyboard focus to a node now; `keyFocus` on a box is the
    /// declarative, edge-triggered form.
    #[napi]
    pub fn focus(&mut self, key: String) -> Result<()> {
        self.runner.core_mut().set_focus(Some(parse_key(&key)?));
        self.runner.request_redraw();
        Ok(())
    }

    #[napi]
    pub fn blur(&mut self) {
        self.runner.core_mut().set_focus(None);
        self.runner.request_redraw();
    }

    /// What Tab does: the next focusable node in tree order, wrapping.
    #[napi]
    pub fn focus_next(&mut self) {
        self.runner.core_mut().focus_next(true);
        self.runner.request_redraw();
    }

    /// What Shift-Tab does.
    #[napi]
    pub fn focus_prev(&mut self) {
        self.runner.core_mut().focus_next(false);
        self.runner.request_redraw();
    }

    /// Scrolling as data, as on `Ctx`: reveal a node, or read and write a
    /// container's retained offset. `reveal` resolves against the next
    /// frame's layout — a key that frame does not declare is a no-op.
    #[napi]
    pub fn reveal(&mut self, key: String) -> Result<()> {
        self.runner.core_mut().reveal(parse_key(&key)?);
        self.runner.request_redraw();
        Ok(())
    }

    #[napi]
    pub fn scroll_offset(&mut self, key: String) -> Result<Json> {
        Ok(offset_json(
            self.runner.core_mut().scroll_offset(parse_key(&key)?),
        ))
    }

    #[napi]
    pub fn set_scroll(&mut self, key: String, x: f64, y: f64) -> Result<()> {
        self.runner
            .core_mut()
            .set_scroll(parse_key(&key)?, Vec2::new(x as f32, y as f32));
        self.runner.request_redraw();
        Ok(())
    }

    /// Hover state as of the last frame (keys come from events, e.g. an
    /// `onHover` enter). For plain hover styling prefer the `hoverBg` /
    /// `pressedBg` props — the core resolves those without a round trip.
    #[napi]
    pub fn is_hovered(&mut self, key: String) -> Result<bool> {
        Ok(self.runner.core_mut().is_hovered(parse_key(&key)?))
    }

    #[napi]
    pub fn is_pressed(&mut self, key: String) -> Result<bool> {
        Ok(self.runner.core_mut().is_pressed(parse_key(&key)?))
    }

    /// Registers a w×h RGBA image (pixels copied); returns its id for
    /// `<image src={id}>`. Stable until `removeImage`.
    #[napi]
    pub fn add_image(&mut self, width: u32, height: u32, rgba: Buffer) -> Result<String> {
        add_image_impl(self.runner.core_mut(), width, height, &rgba)
    }

    #[napi]
    pub fn remove_image(&mut self, id: String) -> Result<()> {
        self.runner
            .core_mut()
            .remove_image(ImageId::from_ffi(parse_u64(&id)?));
        Ok(())
    }

    /// Registers a font from file bytes; see `Ctx.addFont`.
    #[napi]
    pub fn add_font(&mut self, data: Buffer) -> Result<String> {
        add_font_impl(self.runner.core_mut(), &data)
    }

    /// Registers an installed font by family name; see `Ctx.addSystemFont`.
    #[napi]
    pub fn add_system_font(&mut self, name: String) -> Option<String> {
        self.runner.core_mut().add_system_font(&name).map(font_str)
    }

    /// Registers a font file by path; see `Ctx.loadFontFile`.
    #[napi]
    pub fn load_font_file(&mut self, path: String) -> Result<String> {
        load_font_file_impl(self.runner.core_mut(), &path)
    }

    /// Loads a folder of fonts; see `Ctx.loadFontsDir`.
    #[napi]
    pub fn load_fonts_dir(&mut self, dir: String) -> u32 {
        self.runner.core_mut().load_fonts_dir(&dir) as u32
    }

    #[napi]
    pub fn remove_font(&mut self, id: String) -> Result<()> {
        self.runner
            .core_mut()
            .remove_font(FontId::from_ffi(parse_u64(&id)?));
        Ok(())
    }

    // -- Audio: the same calls as `Ctx`, played by the window's device at
    // once rather than at the next pump.

    /// Registers a sound; see `Ctx.addSound`.
    #[napi]
    pub fn add_sound(&mut self, data: Buffer) -> Result<String> {
        add_sound_impl(self.runner.core_mut(), &data)
    }

    #[napi]
    pub fn remove_sound(&mut self, id: String) -> Result<()> {
        self.runner
            .core_mut()
            .remove_sound(SoundId::from_ffi(parse_u64(&id)?));
        self.runner.flush_audio();
        Ok(())
    }

    /// Starts a playback; see `Ctx.play`. Tagged playbacks report
    /// `{kind:"sound", phase:"ended", playback, tag}` through `pollEvents`.
    #[napi]
    pub fn play(&mut self, sound: String, opts: Option<Json>) -> Result<f64> {
        let id = play_impl(self.runner.core_mut(), &sound, opts.as_ref())?;
        self.runner.flush_audio();
        Ok(id)
    }

    #[napi]
    pub fn stop(&mut self, playback: f64, fade_ms: Option<f64>) {
        self.runner
            .core_mut()
            .stop(PlaybackId(playback as u64), fade_ms.unwrap_or(0.0) as f32);
        self.runner.flush_audio();
    }

    #[napi]
    pub fn set_volume(&mut self, playback: f64, volume: f64, tween_ms: Option<f64>) {
        self.runner.core_mut().set_volume(
            PlaybackId(playback as u64),
            volume as f32,
            tween_ms.unwrap_or(0.0) as f32,
        );
        self.runner.flush_audio();
    }

    #[napi]
    pub fn pause(&mut self, playback: f64, fade_ms: Option<f64>) {
        self.runner
            .core_mut()
            .pause(PlaybackId(playback as u64), fade_ms.unwrap_or(0.0) as f32);
        self.runner.flush_audio();
    }

    #[napi]
    pub fn resume(&mut self, playback: f64, fade_ms: Option<f64>) {
        self.runner
            .core_mut()
            .resume(PlaybackId(playback as u64), fade_ms.unwrap_or(0.0) as f32);
        self.runner.flush_audio();
    }

    #[napi]
    pub fn set_master_volume(&mut self, volume: f64, tween_ms: Option<f64>) {
        self.runner
            .core_mut()
            .set_master_volume(volume as f32, tween_ms.unwrap_or(0.0) as f32);
        self.runner.flush_audio();
    }

    #[napi]
    pub fn system_font_families(&mut self) -> Vec<String> {
        self.runner.core_mut().system_font_families()
    }

    /// Measures text the way layout would; see `Ctx.measureText`. Answers
    /// at the window's scale once a frame has run.
    #[napi]
    pub fn measure_text(
        &mut self,
        content: Json,
        style: Option<Json>,
        max_width: Option<f64>,
    ) -> Result<Json> {
        measure_text_impl(self.runner.core_mut(), &content, style.as_ref(), max_width)
    }

    /// Drains the core's warnings; see `Ctx.warnings`. `runWindowed`
    /// drains and prints them itself unless told not to.
    #[napi]
    pub fn warnings(&mut self) -> Json {
        warnings_json(self.runner.core_mut().take_warnings())
    }

    /// The access tree of the last frame; see `Ctx.accessTree`. The window
    /// hands it to the platform by itself (AccessKit); this is for tests
    /// and tooling.
    #[napi]
    pub fn access_tree(&mut self) -> Json {
        access_tree_json(self.runner.core_mut().access_tree())
    }

    /// A request from assistive technology; see `Ctx.access`. A real
    /// screen reader's requests arrive through the window on their own.
    #[napi]
    pub fn access(&mut self, key: String, action: String, value: Option<Json>) -> Result<()> {
        let req = access_request(&key, &action, value)?;
        let events = self.runner.core_mut().handle_input(InputEvent::Access(req));
        self.runner.app_mut().events.extend(events);
        self.runner.request_redraw();
        Ok(())
    }

    #[napi]
    pub fn set_diagnostics(&mut self, on: bool) {
        self.runner.core_mut().set_diagnostics(on);
    }
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
