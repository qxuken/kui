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
    Align, Color, Core, EditKey, EditOptions, FontId, ImageId, InputEvent, Key, KeyCode, KeyMods,
    KeyPress, Mods, Size, Span, TextStyle, UiEvent, Value, Vec2,
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
                let spans: Vec<Span<'_>> = parts_out
                    .iter()
                    .map(|p| {
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
                    })
                    .collect();
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
    #[napi]
    pub fn mouse(&mut self, down: bool, clicks: Option<u32>) {
        self.input(if down {
            InputEvent::MouseDown(clicks.unwrap_or(1).clamp(1, 255) as u8)
        } else {
            InputEvent::MouseUp
        });
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

    #[napi]
    pub fn is_hovered(&self, key: String) -> Result<bool> {
        Ok(self.core.is_hovered(parse_key(&key)?))
    }

    #[napi]
    pub fn is_pressed(&self, key: String) -> Result<bool> {
        Ok(self.core.is_pressed(parse_key(&key)?))
    }

    #[napi]
    pub fn is_focused(&self, key: String) -> Result<bool> {
        Ok(self.core.is_focused(parse_key(&key)?))
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
        let (dl, atlas) = self.core.output();
        let mut o = JsonMap::new();
        o.insert("quadCount".into(), Json::from(dl.quads.len()));
        o.insert("viewportW".into(), Json::from(dl.viewport.w as f64));
        o.insert("viewportH".into(), Json::from(dl.viewport.h as f64));
        o.insert("scale".into(), Json::from(dl.scale as f64));
        o.insert("atlasSize".into(), Json::from(atlas.size));
        Json::Object(o)
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

    #[napi]
    pub fn system_font_families(&mut self) -> Vec<String> {
        self.runner.core_mut().system_font_families()
    }
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
