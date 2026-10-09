//! The Node.js addon behind the `@qxuken/kui` npm package, built with napi-rs.
//!
//! `kui-node` is the native half of kui for Node: it exposes a headless
//! [`Ctx`] and a windowed `KuiWindow` to JavaScript, and the npm package
//! wraps them in a JSX runtime and an Elm-style app loop. It is not
//! published to crates.io; it ships as a prebuilt library inside the npm
//! package, and its JavaScript API is documented there. Like the other
//! bindings it is translation over [`kui_core`] (the model and layout) and
//! `kui_native` (the window), not a second implementation.
//!
//! A frame comes in whole. The JSX runtime produces a plain-data element
//! tree, the package's encoder turns it into one flat instruction stream (a
//! `Float64Array` of opcodes, prop ids and values, plus a UTF-8 string
//! table), and [`Ctx::frame_binary`] lowers that stream straight into the
//! core in a single zero-copy crossing, with no per-node calls and no JSON
//! tree in between. The opcode and prop tables come from [`protocol`], which
//! the encoder reads at module init so the two sides cannot drift, and a
//! version number in slot 0 of every stream guards a stale prebuilt against
//! a newer encoder. Event payloads are plain JSON both ways: `onClick`
//! carries a message value, never a closure.
//!
//! # Where to look
//!
//! - [`Ctx`]: the headless core, for tests and tools; everything a window
//!   does except open one.
//! - [`Ctx::frame_binary`]: the one door a frame comes through.
//! - [`protocol`]: the opcode, prop and schema tables the JS encoder and the
//!   TypeScript generator read.
//! - [`KuiWindow`]: the windowed surface `runWindowed` drives.
//! - `RowHeights`: the measured-heights helper behind the package's `list()`.
//!
//! Build from the repository with `cargo build -p kui-node --release` and
//! point `KUI_NODE_LIB` at the resulting library; see `packages/kui/README.md`.
//! Book: <https://kui-book.qxuken.dev>. Repository: <https://github.com/qxuken/kui>.

use kui_ffi::CExtension;

use kui_core::{
    Appearance, Assistive, AudioCommand, AudioSpec, Color, Core, EditKey, FontId, FrameSample,
    FrameStats, ImageId, InputEvent, Key, KeyCode, KeyMods, KeyPress, Locale, Mods, MotionPref,
    MouseButton, PlayOptions, PlaybackId, Rect, Size, SoundId, SystemEnv, Tokens, UiEvent, Value,
    Vec2, schema::color_hex_str,
};
use napi::Env;
use napi::ValueType;
use napi::bindgen_prelude::{
    Buffer, Either, Float64Array, Function, FunctionRef, Null, Object, Uint8Array, Unknown,
};
use napi_derive::napi;
use serde_json::{Map as JsonMap, Value as Json};

mod binary;
mod rows;
mod schema;

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
        // An integer JS can hold exactly is an Int; past 2^53 it was a JS
        // float that happened to be whole, and stays one — as an Int it
        // came back to the app as a BigInt, `!==` the number it sent.
        Json::Number(n) => match n.as_i64() {
            Some(i) if i.unsigned_abs() <= 1 << 53 => Value::Int(i),
            _ => Value::Float(n.as_f64().unwrap_or(0.0)),
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

/// A readback shape as the object Node hands out: the core's `to_value`
/// form (`kui_core::Handles::HEX`, so a key or a resource id is sixteen
/// hex digits) with every map key turned from snake_case to camelCase —
/// `content_w` → `contentW`, `pos_in_set` → `posInSet` — mechanically,
/// at every depth. The key set each shape ends up with is
/// pinned in `readback_pins`. Event payloads never come through here:
/// they are the app's own data, spelled however the app spelled them.
fn readback(v: &Value) -> Json {
    match v {
        Value::List(items) => Json::Array(items.iter().map(readback).collect()),
        Value::Map(entries) => Json::Object(
            entries
                .iter()
                .map(|(k, v)| (camel(k), readback(v)))
                .collect(),
        ),
        other => json_of(other),
    }
}

/// `snake_case` to `camelCase`; a key with no underscore is unchanged.
fn camel(k: &str) -> String {
    let mut out = String::with_capacity(k.len());
    let mut up = false;
    for c in k.chars() {
        if c == '_' {
            up = true;
        } else if up {
            out.extend(c.to_uppercase());
            up = false;
        } else {
            out.push(c);
        }
    }
    out
}

const HEX: kui_core::Handles = kui_core::Handles::HEX;

// ---------------------------------------------------------------------------
// Plain-object arguments
//
// Frames arrive as the binary IR stream and never as objects, but the
// imperative calls beside them still take plain JS values: `play` takes an
// options bag, `key` and `mouse` take a modifiers object. These helpers read
// those. `measureText` is not one of them — its text and style cross as
// one encoded text element, read by `binary::measure_binary`.

fn bool_prop(props: &JsonMap<String, Json>, key: &str) -> bool {
    props.get(key).and_then(Json::as_bool).unwrap_or(false)
}

/// `icon.rgba`'s bytes, as `window_options` takes them out.
type IconBytes = Vec<u8>;

/// A window's options as JSON, but for `icon.rgba`, taken out as bytes:
/// N-API's JSON conversion reads a typed array index by index into a map
/// of string keys — a quarter of a million of them for a 256 px icon —
/// where the bytes are one copy.
fn window_options(options: Option<Object>) -> Result<(JsonMap<String, Json>, Option<IconBytes>)> {
    let mut map = JsonMap::new();
    let mut rgba = None;
    let Some(o) = options else {
        return Ok((map, rgba));
    };
    for key in Object::keys(&o)? {
        if key != "icon" {
            if let Some(v) = o.get::<Json>(&key)? {
                map.insert(key, v);
            }
            continue;
        }
        let Some(icon) = o.get::<Unknown>(&key)? else {
            continue;
        };
        if icon.get_type()? != ValueType::Object {
            map.insert(key.clone(), o.get::<Json>(&key)?.unwrap_or(Json::Null));
            continue;
        }
        // SAFETY: checked to be an object just above.
        let icon: Object = unsafe { icon.cast()? };
        let mut fields = JsonMap::new();
        for k in Object::keys(&icon)? {
            if k == "rgba" {
                let bytes = icon.get::<Uint8Array>(&k).map_err(|_| {
                    err("window options: icon's `rgba` must be a Uint8Array of width * height * 4 bytes")
                })?;
                rgba = bytes.map(|b| b.to_vec());
            } else if let Some(v) = icon.get::<Json>(&k)? {
                fields.insert(k, v);
            }
        }
        map.insert(key, Json::Object(fields));
    }
    Ok((map, rgba))
}

fn empty_props() -> &'static JsonMap<String, Json> {
    static EMPTY: std::sync::OnceLock<JsonMap<String, Json>> = std::sync::OnceLock::new();
    EMPTY.get_or_init(JsonMap::new)
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
/// error naming both, since "bad id" named neither.
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

/// The same two spellings for a *query* — `isHovered`, `scrollGeometry`,
/// `editText` and the rest — where a name nothing declared is not an error
/// but the answer. Every one of them already has a "no such node" reply for
/// a hex key no layout resolved (`false`, `null`, a zero offset), and a
/// label is the spelling a view uses *before* the node exists: the first
/// frame of a virtual list asks its container for geometry that is not
/// there yet. So the command verbs throw, where a typo is a bug the app
/// wants named, and the queries answer.
fn resolve_query(core: &mut kui_core::Core, s: &str) -> Option<Key> {
    match hex_key(s) {
        Some(k) => Some(k),
        None => core.key_of(s),
    }
}

/// The rows of a menu — `openMenu`'s items and a `<menuBar>` menu's alike:
/// `{label, role, enabled, checked, id, accel}` objects, with everything
/// but `label` optional. An unknown `role` is an error rather than a silent
/// `custom`: a menu whose Copy row quietly stopped being Copy would look
/// like the core ignoring it.
/// One row as either reader reports it — `menu()`'s and `menuBar()`'s
/// alike, and as C's `kui_menu_item` / `kui_menu_bar_item` spell it:
/// the text the drawn menu would show, the role's accelerator where the
/// row declared none, `enabled` and `checked` both present. One function
/// so the two menus cannot read one row two ways.
fn menu_item_json(item: &kui_core::MenuItem) -> Json {
    let mut o = JsonMap::new();
    o.insert("label".into(), Json::from(item.text()));
    o.insert("role".into(), Json::from(item.role.name()));
    o.insert("enabled".into(), Json::Bool(item.enabled));
    o.insert("checked".into(), Json::Bool(item.checked));
    o.insert(
        "accel".into(),
        item.accel_text().map_or(Json::Null, Json::from),
    );
    // A submenu's rows, read the same way; only on a row that has one, so
    // every other row reads as it always did (backlog F128).
    if item.has_submenu() {
        o.insert(
            "items".into(),
            Json::Array(item.submenu.iter().map(menu_item_json).collect()),
        );
    }
    Json::Object(o)
}
/// A menu's rows, with the keys of them no row reads
/// (`MenuItem::stray_keys`, a submenu's rows' included).
fn menu_items(v: &Json) -> Result<(Vec<kui_core::MenuItem>, Vec<String>)> {
    let rows = value_of(v);
    let items = kui_core::MenuItem::list_from_value(&rows).map_err(err)?;
    Ok((items, kui_core::MenuItem::stray_keys(&rows)))
}

/// Raises `unknown_menu_item_key` for each of `keys`, as a dropped prop is.
pub(crate) fn warn_stray_menu_keys(core: &mut kui_core::Core, keys: Vec<String>) {
    if core.diagnostics() {
        for k in keys {
            core.warn(kui_core::diag::unknown_menu_item_key(&k));
        }
    }
}

/// The root's `menu` prop, as the JSON the encoder writes: a list of
/// `{ label, items, enabled? }`, whose items are the same objects
/// `openMenu` takes. One
/// reader for both menus, so a row can never mean two things.
pub(crate) fn menu_bar_of(json: &str) -> Result<(kui_core::MenuBar, Vec<String>)> {
    let parsed: Json = serde_json::from_str(&crate::binary::well_formed(json))
        .map_err(|e| err(format!("menu: {e}")))?;
    let menus = value_of(&parsed);
    let bar = kui_core::MenuBar::from_value(&menus).map_err(err)?;
    Ok((bar, kui_core::MenuBar::stray_keys(&menus)))
}

/// A `<select>`'s `options` prop, as the JSON the encoder writes: strings
/// and the item objects `openMenu` takes, read by the core's one reader.
pub(crate) fn select_options_of(json: &str) -> Result<Vec<kui_core::MenuItem>> {
    let parsed: Json = serde_json::from_str(&crate::binary::well_formed(json))
        .map_err(|e| err(format!("options: {e}")))?;
    kui_core::MenuItem::options_from_value(&value_of(&parsed)).map_err(err)
}

fn keycode_of(s: &str) -> Result<KeyCode> {
    KeyCode::from_name(s).ok_or_else(|| err(format!("unknown key code {s:?}")))
}

fn key_str(key: Key) -> String {
    format!("{:016x}", key.0)
}

/// `ScrollGeometry` as `scrollGeometry()` hands it out; `offset` is the
/// `{x, y}` `setScroll` takes back.
fn geometry_json(g: Option<kui_core::ScrollGeometry>) -> Option<Json> {
    g.map(|g| readback(&g.to_value()))
}

/// A scroll offset as `{x, y}` — the shape `setScroll` takes back.
fn offset_json(off: Vec2) -> Json {
    readback(&off.to_value())
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

/// `KuiWindow`'s `system` option: `setEnv`'s partial, applied to a blank
/// reading — so every field left out, `"unknown"` or null stays at the
/// default, which is the "not pinned" the runner's merge reads it as
/// (`SystemEnv::over`).
fn pinned_system(v: &Json) -> Result<SystemEnv> {
    let o = v
        .as_object()
        .ok_or_else(|| err("KuiWindow: `system` must be an object"))?;
    let mut sys = SystemEnv::default();
    system_env("KuiWindow `system`", &mut sys, o)?;
    Ok(sys)
}

/// `setEnv`'s `system`: the four OS settings and whether assistive
/// technology is listening, each with an explicit "the host cannot tell" —
/// `"unknown"` for the three enums, `null` for the two values. Applied
/// field by field, so a host that learns one of them says only that one.
fn system_env(who: &str, sys: &mut SystemEnv, o: &JsonMap<String, Json>) -> Result<()> {
    let name = |key: &str, v: &Json| -> Result<String> {
        v.as_str()
            .map(str::to_string)
            .ok_or_else(|| err(format!("{who}: system.{key} must be a string")))
    };
    for (key, v) in o {
        match key.as_str() {
            "appearance" => {
                sys.appearance = Appearance::parse(&name(key, v)?).ok_or_else(|| {
                    err(format!(
                        "{who}: system.appearance is one of {:?}",
                        kui_core::schema::APPEARANCES
                    ))
                })?
            }
            "motion" => {
                sys.motion = MotionPref::parse(&name(key, v)?).ok_or_else(|| {
                    err(format!(
                        "{who}: system.motion is one of {:?}",
                        kui_core::schema::MOTIONS
                    ))
                })?
            }
            // Not a setting, but the same shape of fact and the same door:
            // a test declares that something is listening the way it
            // declares reduced motion (backlog F48).
            "assistive" => {
                sys.assistive = Assistive::parse(&name(key, v)?).ok_or_else(|| {
                    err(format!(
                        "setEnv(): system.assistive is one of {:?}",
                        kui_core::schema::ASSISTIVE
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
                        color_hex_str(s).map_err(|e| err(format!("{who}: system.accent: {e}")))?,
                    ),
                    _ => v
                        .as_u64()
                        .and_then(|n| u32::try_from(n).ok())
                        .map(Color::hex)
                        .ok_or_else(|| {
                            err("{who}: system.accent must be 0xRRGGBBAA, \"#rrggbb\" or null")
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
                                "{who}: system.locale must be an ASCII BCP-47 tag of at most {} bytes, or null",
                                Locale::CAP
                            ))
                        })?)
                    }
                }
            }
            _ => return Err(err(format!("{who}: unknown system key {key:?}"))),
        }
    }
    Ok(())
}

fn edit_key_of(name: &str) -> Result<EditKey> {
    EditKey::from_name(name).ok_or_else(|| err(format!("unknown key {name:?}")))
}

/// A headless kui core: build frames from JSX trees, feed input, poll events.
/// Everything a window does except open one, so an app's behaviour is
/// testable without a display.
#[napi]
pub struct Ctx {
    core: Core,
    events: Vec<UiEvent>,
    /// The extensions this context hosts, in origin order:
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

    /// Starts the next frame's record before its view runs: from here
    /// until the frame after it begins, `frameCause()` and `owedBy()`
    /// answer the frame the next `frame` call builds. A
    /// view runs before its frame — it returns the tree `frame` is handed
    /// — so without this it reads the frame before; `createApp`'s loop
    /// calls it ahead of every view. Twice before one frame is once.
    #[napi]
    pub fn begin_frame_cause(&mut self) {
        self.core.begin_frame_cause();
    }

    /// Loads a C extension: a shared library exporting the seven
    /// `kui_ext_*` entry points `crates/kui-ffi/include/kui.h` describes.
    /// `namespace` is the word that fronts every slot name it
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
            if !matches!(
                name.as_str(),
                "refreshHz" | "focused" | "system" | "window" | "audio"
            ) {
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
            let focused = f
                .as_bool()
                .ok_or_else(|| err("setEnv(): focused must be a boolean"))?;
            // A window that lost the keyboard lets go of every key its
            // sink was holding (`Core::set_focused`), so a test can hand
            // the OS's Cmd-Tab to a headless context and see the `up`s
            // the windowed driver would have produced.
            self.core.set_focused(focused);
            let pending = self.core.take_pending_events();
            self.take_events(pending);
        }
        if let Some(s) = o.get("system") {
            let s = s
                .as_object()
                .ok_or_else(|| err("setEnv(): system must be an object"))?;
            // Through `set_system`, which re-resolves the palette from
            // what was just written: a test that sets the appearance and
            // reads `theme()` back without drawing sees the answer.
            let mut sys = self.core.env.system;
            system_env("setEnv()", &mut sys, s)?;
            self.core.set_system(sys);
        }
        if let Some(a) = o.get("audio") {
            // What a driver with a device would report: the state by its
            // schema name, the count as a number. Field by field, like
            // `system`.
            let a = a
                .as_object()
                .ok_or_else(|| err("setEnv(): audio must be an object"))?;
            let au = &mut self.core.env.audio;
            for (name, v) in a {
                match name.as_str() {
                    "device" => {
                        au.device = v
                            .as_str()
                            .and_then(kui_core::AudioDevice::parse)
                            .ok_or_else(|| {
                                err(format!(
                                    "setEnv(): audio.device is one of {:?}",
                                    kui_core::schema::AUDIO_DEVICES
                                ))
                            })?
                    }
                    "live" => {
                        au.live = v
                            .as_u64()
                            .and_then(|n| u32::try_from(n).ok())
                            .ok_or_else(|| err("setEnv(): audio.live must be a u32"))?
                    }
                    _ => return Err(err(format!("setEnv(): unknown audio key {name:?}"))),
                }
            }
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
                "alwaysOnTop" => win.always_on_top = flag(name, v)?,
                "nativeControls" => win.native_controls = native_controls(v)?,
                "backdrop" => {
                    win.backdrop = v
                        .as_str()
                        .and_then(kui_core::Backdrop::from_name)
                        .ok_or_else(|| {
                            err(format!(
                                "setEnv(): window.backdrop is one of {:?}",
                                kui_core::schema::BACKDROPS
                            ))
                        })?
                }
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

    /// Files dragged in from the OS are over the window at (`x`, `y`) —
    /// entering and moving alike: the `onDrop` zone under the
    /// point hears `{kind:"drop", phase:"enter"|"move", paths, x, y,
    /// tag}`, a zone it left hears `leave`, a repeat at the same point is
    /// nothing. `dropTarget()` afterwards is what a driver answers the OS
    /// with.
    #[napi]
    pub fn drag_files(&mut self, paths: Vec<String>, x: f64, y: f64) {
        self.input(InputEvent::DragFiles {
            paths,
            at: Vec2::new(x as f32, y as f32),
        });
    }

    /// A file dialog's answer, as a host that showed it reports it: the
    /// paths picked, none for a cancelled dialog. Whoever asked with
    /// `requestFiles` hears `{kind:"files", paths, tag}`; with nothing
    /// asked it is dropped.
    #[napi]
    pub fn answer_files(&mut self, paths: Vec<String>) {
        self.input(InputEvent::Files(paths));
    }

    /// The OS asked the app to open these documents (backlog F124) — the
    /// Finder's Open With, a file dropped on the Dock icon, `open -a`: the
    /// app hears `{kind:"open", paths}` on the root whether or not it
    /// asked; none is nothing. A `KuiWindow` on macOS hears it from the
    /// runner; this is the headless drive.
    #[napi]
    pub fn open_documents(&mut self, paths: Vec<String>) {
        self.input(InputEvent::Open(paths));
    }

    /// The dragged files released at (`x`, `y`): the zone there hears
    /// `{kind:"drop", phase:"drop", paths, x, y, tag}` and no `leave`
    /// after it; with no zone there, nothing but the lit zone's `leave`.
    #[napi]
    pub fn drop_files(&mut self, paths: Vec<String>, x: f64, y: f64) {
        self.input(InputEvent::DropFiles {
            paths,
            at: Vec2::new(x as f32, y as f32),
        });
    }

    /// The dragged files left the window, or the OS ended the drag
    /// elsewhere: the lit zone hears its `leave`.
    #[napi]
    pub fn drag_cancel(&mut self) {
        self.input(InputEvent::DragCancel);
    }

    /// A button press or release. `clicks`: 1 single, 2 double (word
    /// select), 3 triple (line select) — the grain everywhere text can be
    /// selected: an editor, a `selectable` scope, and a `cells` grid,
    /// where it counts in cells. `button` defaults to "primary", and
    /// only that one presses, drags, places the caret and clicks;
    /// "secondary" asks the node under the pointer for a context menu and
    /// moves nothing else, and every button but the primary reaches a node
    /// that claims it with `onButton`, press to release.
    /// A button past the middle one is its number, `3 + n` — the code a
    /// `button` event carries for it.
    #[napi(ts_args_type = "down: boolean, clicks?: number, button?: MouseButtonName | number")]
    pub fn mouse(
        &mut self,
        down: bool,
        clicks: Option<u32>,
        button: Option<Either<String, u32>>,
    ) -> Result<()> {
        let button = match &button {
            Some(Either::A(name)) => MouseButton::from_name(name)
                .ok_or_else(|| err(format!("unknown mouse button {name:?}")))?,
            Some(Either::B(code)) => MouseButton::from_code(*code),
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

    /// One event of a scroll gesture: `begins`
    /// on its first, then the rest go to the target it picked, wherever
    /// the pointer or the content has gone since — the latching a native
    /// swipe gets, for a headless test. `scroll` is a gesture of its own.
    #[napi]
    pub fn scroll_gesture(&mut self, dx: f64, dy: f64, begins: bool) {
        self.input(InputEvent::ScrollGesture {
            delta: Vec2::new(dx as f32, dy as f32),
            begins,
        });
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

    /// The clipboard's answer to a paste (`requestPaste()`), with what
    /// the pasteboard marked it: routed as `commit` is, and
    /// a focused `onKey` sink hears `{kind:"text", text, tag}` with
    /// `concealed: true` / `transient: true` for the markers set. No
    /// marks is a paste nothing marked, the same answer `commit` gives.
    #[napi(
        ts_args_type = "text: string, marks?: { concealed?: boolean; transient?: boolean } | null"
    )]
    pub fn paste(&mut self, text: String, marks: Option<Json>) {
        let flag = |k: &str| {
            marks
                .as_ref()
                .and_then(|m| m.get(k))
                .and_then(Json::as_bool)
                .unwrap_or(false)
        };
        let marks = kui_core::ClipboardMarks {
            concealed: flag("concealed"),
            transient: flag("transient"),
        };
        self.input(InputEvent::Paste { text, marks });
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
    /// way; omit it and it is the position's US key: the lower-case letter
    /// for a letter, `code` for everything else — the pair a window
    /// reports for ⇧Z is `code: "Z", physical: "z"`, and so is this door's.
    /// Passing both is how a driver reports a non-US
    /// layout, and it is what makes the reported `code` portable: a
    /// layout producing something outside ASCII would leave a Latin
    /// keymap matching nothing, so the position's US key stands in, as
    /// Shift prints it (`"J"`, `":"`; unshifted under Alt). What the press
    /// types is still `code` as passed — the layout's own character.
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
    /// choose used to cost.
    ///
    /// Spelled exactly as `keyDown`: a single character (layout-resolved,
    /// e.g. "W" or "$") or a name ("left", "enter", "escape", "f5", ...),
    /// with mods `{shift, ctrl, alt, super}`, `repeat` for an OS
    /// auto-repeat, and `physical` for the US-QWERTY key at that position.
    /// `release()` is the other end of the same key.
    ///
    /// Spell it the way the OS does, because nothing here re-spells it: a
    /// shifted letter is the upper-case letter with `shift` set —
    /// `press("Z", { shift: true, super: true })` is ⇧⌘Z — and
    /// `press("z", { shift: true })` is a chord no keyboard produces, which
    /// a handler switching on `"z"` hears headless and never from a user.
    /// Fold a one-character `code` to lower case under a chord if a keymap
    /// binds letters.
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
    /// take — with `location` ("left", "right", "numpad"; "standard" when
    /// absent) and `capsLock` / `numLock` beside them (the event's
    /// `caps_lock` / `num_lock` too), and `layout` ("latin",
    /// "nonLatin") — and
    /// the text a plain key would insert already resolved.
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
        // The layout's script, "latin" when absent (backlog F115).
        let script = match m.get("layout").and_then(Json::as_str) {
            None => kui_core::LayoutScript::Latin,
            Some(s) => kui_core::LayoutScript::from_name(s)
                .ok_or_else(|| err(format!("unknown layout script {s:?}")))?,
        };
        // No `physical` means "the key I just named" — the core folds a
        // letter to its lower-case position (F65) — so `code` passes
        // through; with one, the core applies the same non-Latin fallback
        // every driver gets.
        let press = match physical {
            None => KeyPress::new(layout, kmods),
            Some(p) => KeyPress::from_layout_in(layout, keycode_of(p)?, kmods, script),
        };
        // What the press types is the layout's key, not the US stand-in
        // in `code`: ⇧ on the key printed `;` on a Russian layout types
        // `Ж`, not `:` (RG28).
        let location = match m.get("location").and_then(Json::as_str) {
            None => kui_core::KeyLocation::Standard,
            Some(l) => kui_core::KeyLocation::from_name(l)
                .ok_or_else(|| err(format!("unknown key location {l:?}")))?,
        };
        Ok(KeyPress {
            text: layout.typed(kmods),
            location,
            // Either spelling: `capsLock` as a caller writes it, and
            // `caps_lock` as the `key` event it hears spells it, so a press
            // handed back keeps its locks (backlog RG86).
            locks: kui_core::KeyLocks {
                caps: bool_prop(m, "capsLock") || bool_prop(m, "caps_lock"),
                num: bool_prop(m, "numLock") || bool_prop(m, "num_lock"),
            },
            ..press
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

    /// The same driver reports that a `stop` it drained landed on a
    /// playback still running, `at` seconds in: a one-shot `<audio>` that
    /// went away without `finish` is named in a `truncated-playback`
    /// warning (`warnings()`); any other stop reports nothing.
    #[napi]
    pub fn audio_truncated(&mut self, playback: f64, at: f64) {
        self.core.audio_truncated(PlaybackId(playback as u64), at);
    }

    /// The same driver reports that its device refused a `play` it
    /// drained: a tagged playback becomes a `sound` event with phase
    /// `refused` in `pollEvents`, and the node that asked is named in a
    /// `playback-refused` warning either way.
    #[napi]
    pub fn audio_refused(&mut self, playback: f64) {
        self.core.audio_refused(PlaybackId(playback as u64));
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

    /// Whether the last frame asked for the window above every other
    /// app's (a root `<box alwaysOnTop>`); false when it did
    /// not. `runWindowed` applies it to the real window on change and
    /// reports what the platform did as `env().window.alwaysOnTop`; a
    /// bare `Ctx` hands the ask back so a test can assert on it.
    #[napi]
    pub fn always_on_top(&self) -> bool {
        self.core.always_on_top()
    }

    /// Whether the last frame asked for secure keyboard entry (a root
    /// `<box secureInput>`); false when it did not.
    /// `runWindowed` turns it on while that window has the keyboard and
    /// keeps the platform's count balanced; a bare `Ctx` hands the ask
    /// back so a test can assert on it.
    #[napi]
    pub fn secure_input(&self) -> bool {
        self.core.secure_input()
    }

    /// Which Option keys the last frame asked to act as Alt on macOS (a
    /// root `<box optionAsAlt="left">`): `"none"`, `"left"`,
    /// `"right"` or `"both"`, `"none"` when it did not ask. `runWindowed`
    /// applies it to the window on change; a bare `Ctx` hands the ask back
    /// so a test can assert on it.
    #[napi(ts_return_type = "'none' | 'left' | 'right' | 'both'")]
    pub fn option_as_alt(&self) -> &'static str {
        self.core.option_as_alt().name()
    }

    /// Whether the last frame asked for the platform's input method off
    /// in its window (a root `<box imeOff>`) — no composition, and on a
    /// Mac no dead keys and no press-and-hold; false when it did not.
    /// `runWindowed` applies it to the window on change; a bare `Ctx`
    /// hands the ask back so a test can assert on it.
    #[napi]
    pub fn ime_off(&self) -> bool {
        self.core.ime_off()
    }
}

/// Window commands as the objects `windowCommands()` hands out.
fn window_commands_json(cmds: Vec<kui_core::WindowCommand>) -> Json {
    Json::Array(cmds.iter().map(|c| readback(&c.to_value())).collect())
}

/// One row of `nodes()`: `NodeInfo` with the key as a hex string, sizing
/// spelled the way `docs/props.md` does, and the role by its schema name.
fn node_info_json(n: &kui_core::NodeInfo) -> Json {
    let mut o = readback(&n.to_value(HEX));
    // `events` holds the app's own payloads, spelled however the app
    // spelled them: the camel pass stops at the handler names.
    if let Json::Object(o) = &mut o {
        let mut events = JsonMap::new();
        for (name, v) in &n.events {
            events.insert((*name).into(), json_of(v));
        }
        o.insert("events".into(), Json::Object(events));
    }
    o
}

/// `{byte, line}`; see `TextHit` in index.d.ts.
fn text_hit_json(h: kui_core::TextHit) -> Json {
    readback(&h.to_value())
}

/// `{width, height, lines}`; see `TextMetrics` in index.d.ts.
fn text_metrics_json(m: kui_core::TextMetrics) -> Json {
    readback(&m.to_value())
}

/// `{x, y, w, h}` — the shape `scrollGeometry` already returns for a box.
fn rect_json(r: Rect) -> Json {
    readback(&r.to_value())
}

/// `ctx.env()`: every row of `schema::ENV_FIELDS` that has a Node key,
/// read by the row's own getter and filed under its camelCase path —
/// `system.appearance` is `{system: {appearance}}`. A key is always there:
/// "the host cannot tell" is `null`, a stable shape to destructure. The
/// rows Node spells as calls on the context (`focused()`, `region()`) have
/// no key here and are not in the object.
fn env_json(core: &mut Core) -> Json {
    let facts = core.env_facts();
    let mut o = JsonMap::new();
    for row in kui_core::schema::ENV_FIELDS {
        let value = json_of(&(row.get)(&facts));
        for path in row.node {
            let (head, tail) = path
                .split_once('.')
                .map_or((*path, None), |(h, t)| (h, Some(t)));
            match tail {
                None => {
                    o.insert(head.into(), value.clone());
                }
                Some(leaf) => {
                    let nested = o
                        .entry(head.to_string())
                        .or_insert_with(|| Json::Object(JsonMap::new()));
                    if let Json::Object(m) = nested {
                        m.insert(leaf.into(), value.clone());
                    }
                }
            }
        }
    }
    Json::Object(o)
}

/// One colour out of JSON, the way any prop takes one: a `0xRRGGBBAA`
/// number or a `"#rgb"` / `"#rrggbb"` / `"#rrggbbaa"` string.
fn color_from_json(v: &Json) -> Result<Color> {
    match v {
        Json::String(s) => color_hex_str(s).map_err(err),
        _ => v
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .map(Color::hex)
            .ok_or_else(|| err("expected 0xRRGGBBAA or \"#rrggbb\"")),
    }
}

/// `setTheme`'s object: a base named by `appearance` (the OS's when
/// absent), then role overrides on top of it. `accent` is applied through
/// `Theme::with_accent`, so naming it also moves its hover, its pressed
/// shade, the label on it, the ring and the selection tint — and any of
/// those named explicitly wins over what the accent derived.
fn theme_from_json(core: &Core, v: &Json) -> Result<kui_core::Theme> {
    let Json::Object(o) = v else {
        return Err(err("setTheme(): expected an object of role overrides"));
    };
    let appearance = match o.get("appearance") {
        None | Some(Json::Null) => core.env.system.appearance,
        Some(Json::String(s)) => Appearance::parse(s).ok_or_else(|| {
            err(format!(
                "setTheme(): appearance is one of {:?}",
                kui_core::schema::APPEARANCES
            ))
        })?,
        Some(_) => return Err(err("setTheme(): appearance must be a string")),
    };
    let mut t = kui_core::Theme::derive(appearance, core.env.system.accent);
    if let Some(v) = o.get("accent") {
        t = t.with_accent(color_from_json(v).map_err(|e| err(format!("setTheme(): accent: {e}")))?);
    }
    if let Some(v) = o.get("disabledOpacity") {
        t.disabled_opacity = v
            .as_f64()
            .ok_or_else(|| err("setTheme(): disabledOpacity must be a number"))?
            as f32;
    }
    // Every other key is a role, and an unknown one is a typo worth
    // saying out loud rather than a value silently doing nothing.
    for (key, v) in o {
        if matches!(key.as_str(), "appearance" | "accent" | "disabledOpacity") {
            continue;
        }
        let Some(role) = kui_core::schema::THEME_ROLES.iter().find(|r| r.node == key) else {
            return Err(err(format!("setTheme(): no such role {key:?}")));
        };
        let c = color_from_json(v).map_err(|e| err(format!("setTheme(): {key}: {e}")))?;
        (role.set)(&mut t, c);
    }
    Ok(t)
}

/// The palette the core derived from `env.system`, as roles rather than
/// values: one `0xRRGGBBAA` number per row of
/// `schema::THEME_ROLES`, under that row's camelCase spelling, plus
/// `appearance` (which base it came from) and `disabledOpacity` (a
/// multiplier, not a colour). Numbers rather than `#hex` strings because
/// that is what a `color` prop already takes, so `bg={theme.surface}` is
/// the whole of using one.
/// `setTokensRaw`'s object: `{ colors: [[name, colour | { light, dark }],
/// …], lengths: [[name, px], …] }` — the declaration `index.js` flattened
/// from the user's `{ colors: {…}, lengths: {…} }`, as *pairs* because
/// declaration order is the wire index and a JSON object's key order does
/// not survive the crossing (`serde_json` sorts it). Names apart by kind
/// because a colour and a length are both a number here, and the kind
/// cannot be read off the value.
/// A colour value that is an object with `from` is a derived token,
/// its `ops` a list of `[verb, …]` tuples `index.js` has already
/// normalised.
fn tokens_from_json(v: &Json) -> Result<Tokens> {
    let Json::Object(o) = v else {
        return Err(err("setTokens(): expected { colors?, lengths? }"));
    };
    let pairs = |half: &Json, what: &str| -> Result<Vec<(String, Json)>> {
        let Json::Array(items) = half else {
            return Err(err(format!(
                "setTokens(): {what} must be an object of name → value"
            )));
        };
        items
            .iter()
            .map(|item| match item {
                Json::Array(pair) if pair.len() == 2 => match &pair[0] {
                    Json::String(name) => Ok((name.clone(), pair[1].clone())),
                    _ => Err(err("setTokens(): a token name is a string")),
                },
                _ => Err(err("setTokens(): expected [name, value] pairs")),
            })
            .collect()
    };
    let mut t = Tokens::new();
    if let Some(colors) = o.get("colors") {
        for (name, v) in pairs(colors, "colors")? {
            let name = &name;
            let v = &v;
            t = match v {
                // A recipe (ADR 0028): `{ from, ops: [[verb, …], …] }`, each
                // op a tuple — the verb, a colour operand for the two that
                // take one, then the number. `index.js` has already
                // wrapped a bare single tuple.
                Json::Object(recipe) if recipe.contains_key("from") => {
                    let from = recipe.get("from").and_then(Json::as_str).ok_or_else(|| {
                        err(format!(
                            "setTokens(): {name}.from names a colour token or role"
                        ))
                    })?;
                    for k in recipe.keys() {
                        if k != "from" && k != "ops" {
                            return Err(err(format!(
                                "setTokens(): {name}: unknown key {k:?} (from, ops)"
                            )));
                        }
                    }
                    let ops = match recipe.get("ops") {
                        None | Some(Json::Null) => Vec::new(),
                        Some(Json::Array(ops)) => ops
                            .iter()
                            .map(|op| color_op_from_json(op, name))
                            .collect::<Result<Vec<_>>>()?,
                        Some(_) => {
                            return Err(err(format!(
                                "setTokens(): {name}.ops is a list of [verb, …] tuples"
                            )));
                        }
                    };
                    t.derive(name, from, ops)
                }
                Json::Object(halves) => {
                    let half = |k: &str| -> Result<Color> {
                        let h = halves.get(k).ok_or_else(|| {
                            err(format!("setTokens(): {name} needs both light and dark"))
                        })?;
                        color_from_json(h).map_err(|e| err(format!("setTokens(): {name}.{k}: {e}")))
                    };
                    t.color_themed(name, half("light")?, half("dark")?)
                }
                v => {
                    let c =
                        color_from_json(v).map_err(|e| err(format!("setTokens(): {name}: {e}")))?;
                    t.color(name, c)
                }
            };
        }
    }
    if let Some(lengths) = o.get("lengths") {
        for (name, v) in pairs(lengths, "lengths")? {
            let px = v
                .as_f64()
                .ok_or_else(|| err(format!("setTokens(): {name}: a length is a number")))?;
            t = t.length(name, px as f32);
        }
    }
    for k in o.keys() {
        if k != "colors" && k != "lengths" {
            return Err(err(format!(
                "setTokens(): unknown key {k:?} (colors, lengths)"
            )));
        }
    }
    Ok(t)
}

/// One step of a derived token's recipe as `setTokensRaw` receives it: a
/// `[verb, number]` or `[verb, colour, number]` tuple, checked against
/// which the verb takes (`ColorOp::takes_color`).
fn color_op_from_json(op: &Json, token: &str) -> Result<kui_core::ColorOp> {
    let Json::Array(parts) = op else {
        return Err(err(format!(
            "setTokens(): {token}.ops: each op is a [verb, …] tuple"
        )));
    };
    let verb = parts.first().and_then(Json::as_str).ok_or_else(|| {
        err(format!(
            "setTokens(): {token}.ops: an op starts with its verb"
        ))
    })?;
    let Some(takes_color) = kui_core::ColorOp::takes_color(verb) else {
        return Err(err(format!(
            "setTokens(): {token}.ops: unknown verb {verb:?} (lift, darken, raise, alpha, mix, readable)"
        )));
    };
    let arity = if takes_color { 3 } else { 2 };
    if parts.len() != arity {
        return Err(err(format!(
            "setTokens(): {token}.ops: {verb} takes {} — [{verb}, {}]",
            if takes_color {
                "a colour and a number"
            } else {
                "one number"
            },
            if takes_color { "token, t" } else { "t" }
        )));
    }
    let color = takes_color
        .then(|| {
            parts[1].as_str().ok_or_else(|| {
                err(format!(
                    "setTokens(): {token}.ops: {verb}'s colour is a token or role name"
                ))
            })
        })
        .transpose()?;
    let n = parts[arity - 1].as_f64().ok_or_else(|| {
        err(format!(
            "setTokens(): {token}.ops: {verb}'s number is a number"
        ))
    })?;
    Ok(kui_core::ColorOp::parse(verb, color, n as f32).expect("checked above"))
}

/// The tokens a reader in this core sees, resolved for the frame: the
/// running origin's over the host's, colours as `0xRRGGBBAA` numbers and
/// lengths as px, under `colors` and `lengths`.
fn tokens_json(core: &Core) -> Json {
    let look = core.token_lookup();
    let colors: JsonMap<String, Json> = look
        .colors()
        .into_iter()
        .map(|(n, c)| (n.to_string(), Json::from(c.to_hex())))
        .collect();
    let lengths: JsonMap<String, Json> = look
        .lengths()
        .into_iter()
        .map(|(n, v)| (n.to_string(), Json::from(v as f64)))
        .collect();
    let mut o = JsonMap::new();
    o.insert("colors".into(), Json::Object(colors));
    o.insert("lengths".into(), Json::Object(lengths));
    Json::Object(o)
}

fn theme_json(core: &Core) -> Json {
    let t = *core.theme();
    let mut o = JsonMap::new();
    for role in kui_core::schema::THEME_ROLES {
        o.insert(role.node.into(), Json::from((role.get)(&t).to_hex()));
    }
    o.insert("appearance".into(), Json::from(t.appearance.name()));
    o.insert(
        "disabledOpacity".into(),
        Json::from(t.disabled_opacity as f64),
    );
    Json::Object(o)
}

/// The sizes the stock widgets are built from: one number per
/// row of `schema::METRIC_ROLES`, under that row's camelCase spelling.
fn metrics_json(core: &Core) -> Json {
    let m = *core.metrics();
    let mut o = JsonMap::new();
    for role in kui_core::schema::METRIC_ROLES {
        o.insert(role.node.into(), Json::from((role.get)(&m) as f64));
    }
    Json::Object(o)
}

/// `setMetrics`'s object: overrides on top of the set in effect — so an
/// app that changes the radius keeps everything else — or on top of a
/// named set when `base` is `"comfortable"` or `"compact"`; `scale`
/// multiplies every length after the overrides. An unknown key throws.
fn metrics_from_json(core: &Core, v: &Json) -> Result<kui_core::Metrics> {
    let Json::Object(o) = v else {
        return Err(err("setMetrics(): expected an object of overrides"));
    };
    let mut m = match o.get("base") {
        None | Some(Json::Null) => *core.metrics(),
        Some(Json::String(s)) if s == "comfortable" => kui_core::Metrics::comfortable(),
        Some(Json::String(s)) if s == "compact" => kui_core::Metrics::compact(),
        Some(_) => return Err(err("setMetrics(): base is \"comfortable\" or \"compact\"")),
    };
    for (key, v) in o {
        if matches!(key.as_str(), "base" | "scale") {
            continue;
        }
        let Some(role) = kui_core::schema::METRIC_ROLES
            .iter()
            .find(|r| r.node == key)
        else {
            return Err(err(format!("setMetrics(): no such metric {key:?}")));
        };
        let n = v
            .as_f64()
            .ok_or_else(|| err(format!("setMetrics(): {key} must be a number")))?;
        (role.set)(&mut m, n as f32);
    }
    if let Some(v) = o.get("scale") {
        let f = v
            .as_f64()
            .ok_or_else(|| err("setMetrics(): scale must be a number"))?;
        m = m.scaled(f as f32);
    }
    Ok(m)
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

/// `Core::owed` as `{transition, cycle, depart, requested, autoscroll,
/// scroll}`.
fn owed_json(o: kui_core::Owed) -> Json {
    let mut m = JsonMap::new();
    m.insert("transition".into(), Json::from(o.transition));
    m.insert("cycle".into(), Json::from(o.cycle));
    m.insert("depart".into(), Json::from(o.depart));
    m.insert("requested".into(), Json::from(o.requested));
    m.insert("autoscroll".into(), Json::from(o.autoscroll));
    m.insert("scroll".into(), Json::from(o.scroll));
    Json::Object(m)
}

/// `OwedBy` as `owedBy()` hands it out: each holder `{key, name, slots}`
/// with the key as `keyOf` spells it, each request `{why, file, line}`.
fn owed_by_json(by: &kui_core::OwedBy) -> Json {
    let holder = |h: &kui_core::FrameHolder| {
        let mut m = JsonMap::new();
        m.insert("key".into(), Json::from(key_str(h.key)));
        m.insert("name".into(), Json::from(h.name.clone()));
        m.insert("slots".into(), Json::from(h.slots.clone()));
        Json::Object(m)
    };
    let list = |hs: &[kui_core::FrameHolder]| Json::Array(hs.iter().map(holder).collect());
    let mut o = JsonMap::new();
    o.insert("transitions".into(), list(&by.transitions));
    o.insert("cycles".into(), list(&by.cycles));
    o.insert("departures".into(), list(&by.departures));
    o.insert("scrolls".into(), list(&by.scrolls));
    o.insert(
        "autoscroll".into(),
        by.autoscroll.as_ref().map_or(Json::Null, holder),
    );
    o.insert("animate".into(), list(&by.animate));
    let requests = by
        .requests
        .iter()
        .map(|r| {
            let mut m = JsonMap::new();
            m.insert("why".into(), Json::from(r.why));
            m.insert("file".into(), Json::from(r.at.file()));
            m.insert("line".into(), Json::from(r.at.line()));
            Json::Object(m)
        })
        .collect();
    o.insert("requests".into(), Json::Array(requests));
    Json::Object(o)
}

/// The runner's frame-timing ring as `{frames, framesTotal, pumps,
/// wokenPumps, last, avgTotalMs, maxTotalMs, avgWorkMs, maxWorkMs}`;
/// `last` is null before the first frame. The same numbers the latency
/// HUD draws, plus the three monotonic counts a bench asserts a rate from.
fn frame_stats_json(stats: &FrameStats, pumps: u64, woken_pumps: u64) -> Json {
    let mut o = JsonMap::new();
    o.insert("frames".into(), Json::from(stats.len()));
    o.insert("framesTotal".into(), Json::from(stats.total));
    o.insert("pumps".into(), Json::from(pumps));
    o.insert("wokenPumps".into(), Json::from(woken_pumps));
    o.insert("last".into(), stats.last().map_or(Json::Null, sample_json));
    o.insert("avgTotalMs".into(), Json::from(stats.avg_total() as f64));
    o.insert("maxTotalMs".into(), Json::from(stats.max_total() as f64));
    o.insert("avgWorkMs".into(), Json::from(stats.avg_work() as f64));
    o.insert("maxWorkMs".into(), Json::from(stats.max_work() as f64));
    Json::Object(o)
}

/// Decoded pixels (`decodeImage`): `width` by `height`, four bytes each
/// (RGBA), row by row from the top left, alpha not premultiplied — what
/// `addImage`, `updateImage` and a window's `icon` take.
#[napi(object)]
pub struct DecodedImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Buffer,
}

/// Every frame of an animated image (`decodeAnimation`), each the whole
/// canvas, with the seconds each shows and how many times the sequence
/// plays (absent: for ever).
#[napi(object)]
pub struct DecodedAnimation {
    pub width: u32,
    pub height: u32,
    pub frames: Vec<Buffer>,
    pub delays: Vec<f64>,
    pub loops: Option<u32>,
}

/// Which frame shows at a moment (`animationAt`), and the seconds after
/// the start the next is due — `Infinity` once a finite animation has
/// played out.
#[napi(object)]
pub struct AnimationShowing {
    pub index: u32,
    pub next: f64,
}

/// Decodes PNG, JPEG, WebP or GIF bytes (an animated file's first frame)
/// with the runner's decoder (backlog F138), so an app that ships a photo
/// needs no image package: `ctx.addImage(d.width, d.height, d.rgba)`.
/// Throws, naming why, for bytes that are not an image or do not decode.
#[napi]
pub fn decode_image(bytes: Buffer) -> Result<DecodedImage> {
    let p = kui_native::decode_image(&bytes)
        .map_err(|e| napi::Error::from_reason(format!("decodeImage: {e}")))?;
    Ok(DecodedImage {
        width: p.width,
        height: p.height,
        rgba: p.rgba.into(),
    })
}

/// Decodes every frame of an animated GIF, PNG (APNG) or WebP, each the
/// whole canvas, and the seconds each shows; a GIF frame asking for 10 ms
/// or less shows for 100, as browsers show it. A still image is one frame
/// shown for ever. Play it on the frame clock with `animationAt`,
/// `updateImage` and `requestFrameAt`.
#[napi]
pub fn decode_animation(bytes: Buffer) -> Result<DecodedAnimation> {
    let a = kui_native::decode_animation(&bytes)
        .map_err(|e| napi::Error::from_reason(format!("decodeAnimation: {e}")))?;
    Ok(DecodedAnimation {
        width: a.width,
        height: a.height,
        delays: a.frames.iter().map(|f| f.delay).collect(),
        frames: a.frames.into_iter().map(|f| f.rgba.into()).collect(),
        loops: a.loops,
    })
}

/// Which of the frames with these `delays` shows `elapsed` seconds after
/// the animation started, playing `loops` times (null for ever), and when
/// the next is due: `animationAt(gif.delays, gif.loops, ctx.now() -
/// started)`, then `updateImage` when the index moved and
/// `requestFrameAt(started + next)`.
#[napi]
pub fn animation_at(delays: Vec<f64>, loops: Option<u32>, elapsed: f64) -> AnimationShowing {
    let a = kui_native::Animation {
        width: 0,
        height: 0,
        frames: delays
            .into_iter()
            .map(|delay| kui_native::AnimationFrame {
                rgba: Vec::new(),
                delay,
            })
            .collect(),
        loops,
    };
    let s = a.at(elapsed);
    AnimationShowing {
        index: s.index as u32,
        next: s.next,
    }
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

/// The `kui_native::App` behind a Node window. JS never gets called from inside
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
    /// What `onTeardown` registered: the one call JS *does* get from
    /// inside winit, since the moment it is for — the
    /// window going for good — is on macOS the process going too, with
    /// no pump after it for JS to drain anything from.
    teardown: Option<FunctionRef<(), Null>>,
    /// The `Env` of the `pump` / `pumpUntil` in flight, so `teardown` can
    /// call back into JS from the runner's end inside it; `None` between
    /// pumps, where there is no JS frame to call from — a runner dropped
    /// by the collector retires without one, and its `teardown` is not
    /// run (nothing could be: a finalizer may not call into JS).
    env: Option<napi::sys::napi_env>,
    /// What the `teardown` callback threw, if it did: napi clears the
    /// pending exception and hands it back as the call's `Err`, so it is
    /// kept here for the pump in flight to return — the original JS
    /// value, since the error holds a reference to it — rather than
    /// dropped on the floor with the session it failed to save.
    teardown_error: Option<napi::Error>,
}

impl kui_native::App for TreeApp {
    /// The window going for good, to JS: once, synchronously,
    /// inside the pump it happened in — the close button's, a `close()`'s,
    /// or the one an OS Quit ends the process inside of, where nothing
    /// after `await runWindowed(...)` ever runs. A throw out of the
    /// callback comes out of that `pump()` as the throw it is
    /// (`teardown_error`).
    fn teardown(&mut self) {
        let (Some(f), Some(env)) = (&self.teardown, self.env) else {
            return;
        };
        let env = napi::Env::from_raw(env);
        match f.borrow_back(&env).and_then(|f| f.call(())) {
            Ok(Null) => {}
            Err(e) => self.teardown_error = Some(e),
        }
    }

    fn view(&mut self, ui: &mut kui_native::Ui<'_>) {
        let name = ui.window_name();
        let Some((stream, strings)) = self.trees.get(&*name) else {
            return;
        };
        if let Err(e) = binary::lower_binary(ui, stream, strings) {
            self.error = Some(e.reason.to_string());
        }
    }

    /// Kept, not answered: `update` runs in JS after the pump, which is
    /// what `Launcher::deferred_events` tells the shell below.
    fn on_event(&mut self, ev: UiEvent) {
        self.events.push(ev);
    }
}

// Stand-in for "no bound on this axis" when only one half of `maxWidth` /
// `maxHeight` is given: logical px past any display, and small enough that
// the platforms still convert it to physical px without overflowing.
const UNBOUNDED_SIZE: f64 = 65_535.0;

/// How `useWindow` names a window: by the name `windows()` lists, or by
/// the id an event carries.
enum WindowRef {
    Name(String),
    Id(u32),
}

/// A real kui window (winit + wgpu) driven from Node. The event loop is
/// pumped, not run: call `pump()` between libuv turns so winit and libuv share
/// the main thread — or prefer `runWindowed`, which does that for you, unless
/// you are building your own loop. One event loop per process (winit event
/// loops are not recreatable on every platform), any number of windows on
/// it: a view whose root declares `windows` opens more, `windows()` lists
/// them, and `setView` takes the name of the one a tree is for.
#[napi]
pub struct KuiWindow {
    runner: kui_native::PumpRunner<TreeApp>,
    /// The window every per-window door addresses (`useWindow`): the
    /// main window until `runWindowed` aims the surface at the window
    /// whose view or event it is handing to the app.
    addressed: kui_core::WindowId,
}

#[napi]
impl KuiWindow {
    /// Options: `{width, height, minWidth, minHeight, maxWidth, maxHeight,
    /// chrome: "native" | "custom" | "borderless", textAa: "auto" | "gray"
    /// | "subpixel", frameLatency, system, icon, backdrop}`. The min/max pairs bound what the user can
    /// resize the window to; either half may stand alone. `system` pins part of `env.system` over what the OS
    /// says, for the life of the window — `{motion: 'reduced'}` is what a
    /// user who asked for less motion would get, on a machine whose owner
    /// did not; `icon: {rgba, width, height, resource}` is every window's
    /// icon; see `WindowOptions`.
    #[napi(constructor, ts_args_type = "title: string, options?: WindowOptions")]
    pub fn new(title: String, options: Option<Object>) -> Result<Self> {
        let (o, icon_rgba) = window_options(options)?;
        let o = &o;
        // `TreeApp::on_event` only keeps an event; JS runs `update` after
        // the pump and submits the next view. So an input leaves its frame
        // to that answer, and a click paints once — the button let go and
        // the count moved in one frame, not two.
        let mut launcher = kui_native::app(&title).deferred_events();
        // A size is both halves or neither: a lone `width` used to be
        // dropped silently, against the encoder's own "refuse, don't drop"
        // (backlog AR40). The min/max pairs below are different — either
        // half of a bound stands alone, and says so.
        let w = o.get("width").and_then(Json::as_f64);
        let h = o.get("height").and_then(Json::as_f64);
        match (w, h) {
            (Some(w), Some(h)) => launcher = launcher.size(w, h),
            (None, None) => {}
            _ => {
                return Err(err(
                    "window options: `width` and `height` go together (a min or max bound may stand alone)",
                ));
            }
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
        launcher = match o.get("chrome") {
            None | Some(Json::Null) => launcher,
            Some(Json::String(s)) if s == "native" => launcher,
            Some(Json::String(s)) if s == "custom" => launcher.custom_titlebar(),
            Some(Json::String(s)) if s == "borderless" => launcher.borderless(),
            Some(other) => {
                return Err(err(format!(
                    "window options: chrome must be \"native\", \"custom\" or \"borderless\", not {other}"
                )));
            }
        };
        // The glyph antialiasing, the launcher's `text_aa`; `KUI_TEXT_AA`
        // in the environment still wins, for an A/B by hand.
        launcher = match o.get("textAa") {
            None | Some(Json::Null) => launcher,
            Some(Json::String(s)) if s == "auto" => launcher.text_aa(kui_native::TextAa::Auto),
            Some(Json::String(s)) if s == "gray" => launcher.text_aa(kui_native::TextAa::Grayscale),
            Some(Json::String(s)) if s == "subpixel" => {
                launcher.text_aa(kui_native::TextAa::Subpixel)
            }
            Some(other) => {
                return Err(err(format!(
                    "window options: textAa must be \"auto\", \"gray\" or \"subpixel\", not {other}"
                )));
            }
        };
        // What shows through the window (backlog F126), the launcher's
        // `backdrop`; what came of it is `env().window.backdrop`.
        launcher = match o.get("backdrop") {
            None | Some(Json::Null) => launcher,
            Some(Json::String(s)) => match kui_core::Backdrop::from_name(s) {
                Some(b) => launcher.backdrop(b),
                None => {
                    return Err(err(format!(
                        "window options: backdrop is one of {:?}, not {s:?}",
                        kui_core::schema::BACKDROPS
                    )));
                }
            },
            Some(other) => {
                return Err(err(format!(
                    "window options: backdrop is one of {:?}, not {other}",
                    kui_core::schema::BACKDROPS
                )));
            }
        };
        // Frames queued ahead of the one on screen (backlog C47), the
        // launcher's `frame_latency`; `KUI_FRAME_LATENCY` still wins.
        match o.get("frameLatency") {
            None | Some(Json::Null) => {}
            Some(Json::Number(n)) if n.as_u64().is_some_and(|n| (1..=3).contains(&n)) => {
                launcher = launcher.frame_latency(n.as_u64().unwrap_or(2) as u32);
            }
            Some(other) => {
                return Err(err(format!(
                    "window options: frameLatency must be 1, 2 or 3, not {other}"
                )));
            }
        }
        // `system: {motion: 'reduced'}` — the same partial `setEnv` takes
        // headless, read the same way, but pinned at the launcher rather
        // than pushed into a core: the runner writes the real reading
        // before every frame, and only a merge inside that write survives
        // it (backlog F47). `'unknown'` and null are "not pinned".
        if let Some(sys) = o.get("system") {
            launcher = launcher.system(pinned_system(sys)?);
        }
        // `icon: {rgba, width, height}` and/or `{resource}` — the
        // launcher's `icon` and `icon_resource` (backlog F86); the pixels
        // were taken out as bytes by `window_options`.
        match o.get("icon") {
            None | Some(Json::Null) => {}
            Some(Json::Object(icon)) => {
                let dim = |k: &str| {
                    icon.get(k)
                        .and_then(Json::as_u64)
                        .and_then(|v| u32::try_from(v).ok())
                };
                match (icon_rgba, dim("width"), dim("height")) {
                    (Some(rgba), Some(w), Some(h)) => {
                        launcher = launcher
                            .try_icon(rgba, w, h)
                            .map_err(|e| err(format!("window options: {e}")))?;
                    }
                    (None, None, None) => {}
                    _ => {
                        return Err(err(
                            "window options: icon's `rgba` (a Uint8Array), `width` and `height` go together",
                        ));
                    }
                }
                match icon.get("resource") {
                    None | Some(Json::Null) => {}
                    Some(r) => {
                        let id = r.as_u64().and_then(|v| u16::try_from(v).ok()).ok_or_else(|| {
                            err(format!("window options: icon's `resource` must be a resource id from 0 to 65535, not {r}"))
                        })?;
                        launcher = launcher.icon_resource(id);
                    }
                }
            }
            Some(other) => {
                return Err(err(format!(
                    "window options: icon must be {{rgba, width, height}} and/or {{resource}}, not {other}"
                )));
            }
        }
        let runner = launcher
            .open(TreeApp::default())
            .map_err(|e| err(format!("failed to open window: {e}")))?;
        Ok(KuiWindow {
            runner,
            addressed: kui_core::WindowId::MAIN,
        })
    }

    /// Which window the per-window doors — `focus`, `editText`,
    /// `setEditText`, `isHovered`, `scrollGeometry`, `openMenu`,
    /// `selectionText`, `setTheme`, `setMetrics`, `setTokens`, the
    /// devtools setters, input injection — address from here on: a name
    /// from `windows()`, or the id an event carries in `window`; left out,
    /// the main window. Returns whether that window is open now; until it
    /// is, the doors address the main window. `runWindowed` aims the
    /// surface at the window whose view it is calling and at the window
    /// an event came from before handing the surface to `update`, so an
    /// app that never calls this reads and writes the window it is being
    /// asked about. Resources, `windows()`, `pump` and
    /// `pollEvents` are the session's and unaffected.
    #[napi(ts_args_type = "window?: string | number")]
    pub fn use_window(&mut self, window: Option<Either<String, f64>>) -> bool {
        self.address(window.map(|w| match w {
            Either::A(name) => WindowRef::Name(name),
            Either::B(id) => WindowRef::Id(id as u32),
        }))
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
    pub fn pump(&mut self, env: Env) -> Result<bool> {
        self.runner.app_mut().env = Some(env.raw());
        let alive = self.runner.pump();
        self.after_pump(alive)
    }

    /// What both pumps do on the way out: the env lent to the app for the
    /// pump is taken back, and a throw the pump gathered — the teardown
    /// callback's, or a view that would not lower — is the pump's own.
    fn after_pump(&mut self, alive: bool) -> Result<bool> {
        let app = self.runner.app_mut();
        app.env = None;
        if let Some(e) = app.teardown_error.take() {
            return Err(e);
        }
        if let Some(e) = app.error.take() {
            return Err(err(format!("view lowering failed: {e}")));
        }
        Ok(alive)
    }

    /// Registers what the window calls as it goes for good — its close
    /// button, `close()`, Quit from the menu or the dock — once, from
    /// inside the `pump()` that saw it and before that pump returns.
    /// On macOS a Quit ends the process inside that pump:
    /// `pump()` never returns, `runWindowed` never resolves and nothing
    /// after it runs, not even `process.on('exit')` — so this is the only
    /// thing an app runs on ⌘Q. `runWindowed` registers its config's
    /// `teardown(model)` here; a driver of its own does the same. The
    /// window is gone by then: the callback saves and returns, and does
    /// not call the window's doors. The last registration wins.
    #[napi(ts_args_type = "callback: () => void")]
    pub fn on_teardown(&mut self, callback: Function<'_, (), Null>) -> Result<()> {
        self.runner.app_mut().teardown = Some(callback.create_ref()?);
        Ok(())
    }

    /// `pump`, but parked for up to `timeoutMs` — returning the moment an
    /// OS event or a `Waker` wake arrives, and otherwise when the time is
    /// up. Returns false once the window has closed. `timeoutMs` is clamped
    /// to an hour; anything not finite and positive parks not at all.
    ///
    /// **What this buys is latency, and it is not what `runWindowed` idles
    /// on.** The wake is the event itself rather than the next tick after
    /// it, so the period stops setting the response time — but two measured
    /// things make it the wrong default, and both surprise:
    ///
    /// - **It does not save CPU; it costs.** winit 0.30's macOS pump stops
    ///   on the *first* wake of any kind, so a park does not hold: a 200 ms
    ///   park runs 101 ms on average. Against polling at the same period, a
    ///   32 ms period is 49 pumps a second and 4.59% of a core parked,
    ///   against 30 pumps and 1.80% polled.
    /// - **A blocked main thread is a blocked libuv**, and Node cannot be
    ///   asked whether that is safe — `process.getActiveResourcesInfo()`
    ///   reports nothing for an in-flight `fs.readFile`. Behind a 50 ms
    ///   park, 200 sequential `await readFile` went from 6 ms to 4 s.
    ///
    /// So reach for it only in a loop that owns its whole process and does
    /// no async I/O, and wants the latency. Otherwise `pump()` on a timer
    /// is both cheaper and safer — which is what `runWindowed` does.
    #[napi]
    pub fn pump_until(&mut self, env: Env, timeout_ms: f64) -> Result<bool> {
        // Clamped, not just checked: `Duration::from_secs_f64` panics on a
        // large finite value rather than saturating, so a caller spelling
        // "forever" as `Number.MAX_VALUE` would take the process with it.
        const MAX_PARK_MS: f64 = 60.0 * 60.0 * 1e3;
        let ms = if timeout_ms.is_finite() && timeout_ms > 0.0 {
            timeout_ms.min(MAX_PARK_MS)
        } else {
            0.0
        };
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs_f64(ms / 1e3);
        self.runner.app_mut().env = Some(env.raw());
        let alive = self.runner.pump_until(deadline);
        self.after_pump(alive)
    }

    /// How long until the window next needs a pump, in ms — `null` when
    /// nothing the runner knows about is due.
    ///
    /// A driver on a fixed interval hits every deadline the shell has
    /// already worked out a whole interval late: the caret blink, a
    /// transition's next frame, the audio poll, a window's first-frame
    /// retry. Ask after each pump and sleep to the answer instead, which is
    /// what `runWindowed` does. It only ever asks for *sooner* — whether an
    /// OS event is waiting is not something it can say, so a driver still
    /// needs a ceiling of its own.
    #[napi]
    pub fn next_deadline_ms(&self) -> Option<f64> {
        self.runner.next_deadline().map(|d| {
            d.saturating_duration_since(std::time::Instant::now())
                .as_secs_f64()
                * 1e3
        })
    }

    /// The viewport the app lays out into, in logical px, plus the scale
    /// factor: `{width, height, scale}` — the window's inner size, less
    /// the devtools' dock while the panel is docked.
    /// Readable before the first frame (in `setup` and `init`, where
    /// `env().viewport` is still 0×0), and re-reported as a
    /// `{kind:"resize", width, height, scale}` event through `pollEvents`
    /// — a `ResizeMsg` — whenever the window changes size, moves to a
    /// display with another DPI, or the dock comes, goes or is dragged.
    /// It used to be the window's inner size, so an app that seeded
    /// its tiers from it under `KUI_DEVTOOLS=1` drew for the whole window.
    #[napi(ts_return_type = "WindowSize")]
    pub fn size(&mut self) -> Json {
        let (window, scale) = self.runner.window_size();
        let size = self.runner.core_mut().host_area(window);
        size_json(size, scale)
    }

    /// Frame timing measured by the runner — what the latency HUD draws,
    /// as data: `{frames, framesTotal, pumps, wokenPumps, last: {inputMs,
    /// viewMs, layoutMs, renderMs, waitMs, totalMs, workMs} | null,
    /// avgTotalMs, maxTotalMs, avgWorkMs, maxWorkMs}`. The averages and
    /// maxima are over the last 120 frames and `frames` is how many of
    /// those the ring holds — its fill, one per painted frame up to 120, so
    /// a window that paints only when something changes stays below it for
    /// as long as it idles. `framesTotal` and `pumps` are the monotonic
    /// counts of every frame painted and every `pump()` taken, so two
    /// readings a second apart are that second's frame and
    /// pump rates. `wokenPumps` counts the pumps that found an OS event or
    /// a wake — a key, the pointer crossing, a focus change, a resize, a
    /// reader asking — so two readings a second apart with it
    /// unmoved are a second the desktop left the window alone, and every
    /// frame in it was the app's own. `waitMs` is vsync backpressure;
    /// `workMs` is everything else.
    #[napi(ts_return_type = "FrameTiming")]
    pub fn frame_stats(&mut self) -> Json {
        let (pumps, woken) = (self.runner.pumps(), self.runner.woken_pumps());
        frame_stats_json(&self.runner.core_mut().stats, pumps, woken)
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
                // The slot the node was filled into, as its key (`keyOf` of
                // the slot's full name answers the same), or null for a
                // node the app drew (backlog K2).
                o.insert(
                    "slot".into(),
                    ev.slot.map_or(Json::Null, |k| Json::String(key_str(k))),
                );
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

            /// Replaces an image's pixels in place (copied): the id is
            /// unchanged, so every `<image src={id}>` shows the new pixels
            /// next frame with no view change; `width`/`height` may differ
            /// from the registration. From the first update on the image
            /// is drawn from a texture of its own — a video frame, a
            /// camera, a plot the app rasterised itself. A dead id warns
            /// `foreign-resource` and changes nothing.
            #[napi]
            pub fn update_image(
                &mut self,
                id: String,
                width: u32,
                height: u32,
                rgba: Buffer,
            ) -> Result<()> {
                let id = ImageId::from_ffi(parse_u64(&id)?);
                update_image_impl(self.$core(), id, width, height, &rgba)
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

            /// Scans the system's fonts again, so a font installed while the
            /// app runs is found (kui scans them once a process); returns how
            /// many faces came and went, 0 when nothing did. Tens of
            /// milliseconds: call it when the set may have changed, not
            /// every frame.
            #[napi]
            pub fn reload_system_fonts(&mut self) -> u32 {
                let changed = self.$core().reload_system_fonts();
                if changed > 0 {
                    self.$redraw();
                }
                changed as u32
            }

            /// The fonts asked, in order, for a character the text's own
            /// family has no glyph for, before the platform's fallback list
            /// — whose first choice on macOS is the system's proportional
            /// face. Ids from `addFont` / `addSystemFont` / `loadFontFile`;
            /// one that names no font is left out, and `[]` is the
            /// platform's list alone. `mono` text asks them straight after
            /// its own face, ahead of the machine's other monospaced faces.
            /// A new list shapes every text again; the same list twice is
            /// nothing.
            #[napi]
            pub fn set_fallback_fonts(&mut self, ids: Vec<String>) -> Result<()> {
                let ids = ids
                    .iter()
                    .map(|id| parse_u64(id).map(FontId::from_ffi))
                    .collect::<Result<Vec<_>>>()?;
                self.$core().set_fallback_fonts(&ids);
                self.$redraw();
                Ok(())
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

            /// The families `systemFontFamilies` names, in its order, each
            /// with what its faces say they are: `monospaced` (every face
            /// fixed-pitch), `weights` (sorted, each once) and `italic`.
            /// Read from what the font database recorded
            /// when it scanned each face, so a font picker can put the
            /// monospaced ones first without loading a file or shaping a
            /// glyph.
            #[napi(ts_return_type = "Array<SystemFont>")]
            pub fn system_fonts(&mut self) -> Json {
                Json::Array(
                    self.$core()
                        .system_fonts()
                        .into_iter()
                        .map(|f| {
                            serde_json::json!({
                                "family": f.family,
                                "monospaced": f.monospaced,
                                "weights": f.weights,
                                "italic": f.italic,
                            })
                        })
                        .collect(),
                )
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
            /// prop instead.
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
                let events = std::mem::take(self.$events());
                // An input that reached the app declined to ask for its own
                // frame (`Launcher::deferred_events`), leaving it to this
                // host — so handing the batch over is where the host owes
                // one. `runWindowed` draws on any non-empty drain and its
                // `setView` asks too, coalescing into the same frame; the
                // request here is what makes an input produce a frame for
                // *any* host, including one whose handler submits no view.
                if !events.is_empty() {
                    self.$redraw();
                }
                events_json(events)
            }

            // -- Frame -----------------------------------------------------

            /// True when the last frame left a transition mid-flight. A window
            /// schedules its own redraws for that; this is for tests and
            /// drivers that want to know when motion has settled.
            #[napi]
            pub fn animating(&mut self) -> bool {
                self.$core().animating()
            }

            /// What the last frame left owed, by kind — `animating()`
            /// taken apart: `{transition, cycle, depart, requested,
            /// autoscroll, scroll}`. To the window they are one, and it redraws
            /// for any of them; to a test they differ, since a keyframe
            /// `repeat` cycle never ends and `settled()` never resolves
            /// under one. `quiet()` on the loop waits on everything but
            /// `cycle`.
            #[napi(ts_return_type = "Owed")]
            pub fn owed(&mut self) -> Json {
                owed_json(self.$core().owed())
            }

            /// Turns on the trace of why frames run: who
            /// holds each owed frame (`owedBy()`) and whether each frame
            /// changed what is drawn (`frameUnchanged()`). Off by default,
            /// where neither costs anything; `frameCause()` is kept either
            /// way.
            #[napi]
            pub fn set_frame_trace(&mut self, on: bool) {
                self.$core().set_frame_trace(on);
            }

            /// Why a frame runs, as the names of its reasons: the input it
            /// answers (`key`, `pointerMove`, `wheel`, …), what a window's
            /// runner saw (`wake`, `resize`, `caret`, `retry`, …) and
            /// `owed` when the frame before left one owed.
            /// Empty for a frame nothing here asked for.
            ///
            /// Which frame: on a `Ctx`, from inside a `createApp` view,
            /// the one that view is for (the loop calls
            /// `beginFrameCause()` first), and between frames the last one
            /// built. On a `KuiWindow`, always the last frame drawn: a
            /// window's view runs when the model changes, ahead of the
            /// frame that shows it, and the frames a transition or a
            /// blink runs call no view at all.
            #[napi(ts_return_type = "FrameCauseName[]")]
            pub fn frame_cause(&mut self) -> Vec<&'static str> {
                self.$core().frame_cause().names().collect()
            }

            /// Who held the frame the last one left owed — `owed()` with
            /// names: the nodes mid-transition (with their slots), the
            /// cycles, the departures, the easing scrollers, the held
            /// drag's scroller, the `animate` nodes, and each line that
            /// asked for a frame. Read from inside a `createApp` view on a
            /// `Ctx`, the reason that view's frame exists; between frames,
            /// the last frame built's; on a `KuiWindow`, the last frame
            /// drawn's, as `frameCause()` says. Empty
            /// unless `setFrameTrace(true)`.
            #[napi(ts_return_type = "OwedBy")]
            pub fn owed_by(&mut self) -> Json {
                owed_by_json(self.$core().owed_by())
            }

            /// Whether the last finished frame drew exactly what the one
            /// before drew; `null` untraced and on the first traced frame.
            #[napi]
            pub fn frame_unchanged(&mut self) -> Option<bool> {
                self.$core().frame_unchanged()
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
            /// one's. Drive that window with `access(key,
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
            /// them by `uv[0]`: twenty-four doubles each — the handle as
            /// two 32-bit halves, the sixteen parameters, then where the
            /// draw's `image` is (0 none, 1 the atlas, 2 a texture of its
            /// own), the `textureDraws` index when it is 2, and the texel
            /// rect `x, y, w, h`. The parameters ride a side
            /// list rather than the quad, so `quads()` alone cannot show
            /// them and a corpus adapter needs this to compare them.
            /// Empty on a frame that draws no fragment.
            #[napi]
            pub fn fragment_draws(&mut self) -> Vec<f64> {
                let (dl, _) = self.$core().output();
                let mut out = Vec::with_capacity(dl.fragments.len() * 24);
                for f in &dl.fragments {
                    let raw = f.id.to_ffi();
                    out.push((raw >> 32) as f64);
                    out.push((raw & 0xffff_ffff) as f64);
                    out.extend(f.params.iter().map(|v| *v as f64));
                    // Where the image is: 0 none, 1 the atlas, 2 a texture
                    // — then the `textureDraws` index (0 unless 2) and the
                    // texel rect.
                    let (from, index) = match f.image {
                        kui_core::FragmentImage::None => (0.0, 0.0),
                        kui_core::FragmentImage::Atlas(_) => (1.0, 0.0),
                        kui_core::FragmentImage::Texture { index, .. } => (2.0, index as f64),
                    };
                    out.push(from);
                    out.push(index);
                    out.extend(f.image.uv().iter().map(|v| *v as f64));
                }
                out
            }

            /// This frame's texture draws, in the order their quads index
            /// them by `uv[0]`: nine doubles each — the image handle as two
            /// 32-bit halves, the pixels' revision, width and height, and
            /// the texel rect `x, y, w, h` in the image's own texels (the
            /// whole image, or the crop a `fit="cover"` made). The side
            /// list a `quads()` texture quad points at.
            /// Empty on a frame that draws no texture-backed image.
            #[napi]
            pub fn texture_draws(&mut self) -> Vec<f64> {
                let (dl, _) = self.$core().output();
                let mut out = Vec::with_capacity(dl.textures.len() * 9);
                for (t, px) in dl.textures.iter().zip(&dl.texture_pixels) {
                    let raw = t.id.to_ffi();
                    out.push((raw >> 32) as f64);
                    out.push((raw & 0xffff_ffff) as f64);
                    out.push(px.rev as f64);
                    out.push(px.width as f64);
                    out.push(px.height as f64);
                    out.extend(t.uv.iter().map(|v| *v as f64));
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

            /// The palette this window paints with: one `0xRRGGBBAA` number
            /// per role, derived from `env().system` unless this app said
            /// otherwise (`setAccent` / `setTheme`). A view reads it and
            /// paints with it — `<box bg={theme.surface}>` — and the stock
            /// widgets already do, so a JSX app that only uses `<button>`,
            /// the context menu and `<text>` follows the OS's light and
            /// dark without reading this at all.
            #[napi(ts_return_type = "Theme")]
            pub fn theme(&mut self) -> Json {
                theme_json(self.$core())
            }

            /// Keep following the OS's light/dark, but paint this accent
            /// instead of the OS's — an app with a brand colour. A
            /// `0xRRGGBBAA` number or a `"#hex"` string, as any colour
            /// prop takes; `null` goes back to the OS's own.
            #[napi(ts_args_type = "accent: number | string | null")]
            pub fn set_accent(&mut self, accent: Json) -> Result<()> {
                match accent {
                    Json::Null => self.$core().derive_theme(),
                    v => {
                        let c =
                            color_from_json(&v).map_err(|e| err(format!("setAccent(): {e}")))?;
                        self.$core().set_accent(c);
                    }
                }
                Ok(())
            }

            /// Pin the palette: an object of role overrides on top of the
            /// base named by `appearance` (`"light"`, `"dark"`, or absent
            /// for the OS's), each value a colour the way a prop takes
            /// one. Follows nothing afterwards — `setAccent(null)` is how
            /// an app goes back to following the OS.
            ///
            /// `{ appearance: "dark", accent: "#d2691e" }` is a dark app
            /// with one colour changed; every role the object does not
            /// name keeps the base's, and the ones derived from the accent
            /// (its hover, its pressed shade, the label on it, the ring,
            /// the selection tint) are recomputed unless named too.
            #[napi(ts_args_type = "theme: ThemeOverrides")]
            pub fn set_theme(&mut self, theme: Json) -> Result<()> {
                let t = theme_from_json(self.$core(), &theme)?;
                self.$core().set_theme(t);
                Ok(())
            }

            /// The sizes the stock widgets are built from — the palette's
            /// other axis: one number per metric, logical px before the
            /// scale factor. Read it so a control of your own agrees with
            /// `<button>` on a radius and a padding: `<box radius={metrics.radius}>`.
            #[napi(ts_return_type = "Metrics")]
            pub fn metrics(&mut self) -> Json {
                metrics_json(self.$core())
            }

            /// Makes these the frame's metrics: overrides on top of the
            /// set in effect, or on top of `base: "comfortable"` (the
            /// stock set) / `"compact"` (a dense tool's), then `scale`
            /// multiplying every length — a density slider. `null`
            /// restores the stock set. Density is the app's to choose;
            /// nothing in the OS is followed.
            #[napi(ts_args_type = "metrics: MetricsOverrides | null")]
            pub fn set_metrics(&mut self, metrics: Json) -> Result<()> {
                let m = match metrics {
                    Json::Null => kui_core::Metrics::default(),
                    v => metrics_from_json(self.$core(), &v)?,
                };
                self.$core().set_metrics(m);
                Ok(())
            }

            /// Declare the app's named colours and lengths: `{ colors:
            /// { peach: '#ffcc99', ink: { light, dark } }, lengths: {
            /// sideW: 132 } }`. Replaces the table whole, so an app whose
            /// lengths change with a viewport tier declares again on
            /// `resize`. A name a theme or metrics role owns is dropped
            /// with a `reserved-token` warning. A colour may be a
            /// recipe over an earlier one: `{ from: 'peach',
            /// ops: [['lift', 0.3]] }`, dropped with `unknown-token` when
            /// its source is not there. Reference one in a prop
            /// as `'$peach'` — `defineTokens` types the names. The raw
            /// addon door; `index.js` wraps it to keep the encoder's map
            /// in step, so call `setTokens` and not this.
            #[napi(ts_args_type = "tokens: { colors: [string, unknown][], lengths: [string, number][] }")]
            pub fn set_tokens_raw(&mut self, tokens: Json) -> Result<()> {
                let t = tokens_from_json(&tokens)?;
                self.$core().set_tokens(t);
                Ok(())
            }

            /// The tokens a view here sees this frame, resolved: colours as
            /// `0xRRGGBBAA` for the appearance in effect, lengths in px,
            /// under `colors` and `lengths`. Roles are not listed — read
            /// them off `theme()` and `metrics()`.
            #[napi(ts_return_type = "ResolvedTokens")]
            pub fn tokens(&mut self) -> Json {
                tokens_json(self.$core())
            }

            /// `frame` / `setView` report the `$name` references the
            /// encoder could not resolve — nothing declared, or the other
            /// kind — through here, as `unknown-token`, once per name.
            #[napi(ts_args_type = "names: [string, string][]")]
            pub fn warn_unknown_tokens(&mut self, names: Vec<Vec<String>>) {
                for pair in &names {
                    let [name, wanted] = &pair[..] else { continue };
                    let wanted = match wanted.as_str() {
                        "length" => kui_core::TokenKind::Length,
                        _ => kui_core::TokenKind::Color,
                    };
                    let e = match self.$core().token_lookup().resolve(name) {
                        Some(r) if r.kind() != wanted => kui_core::TokenError::Kind {
                            name: name.clone(),
                            is: r.kind(),
                            wanted,
                        },
                        _ => kui_core::TokenError::Unknown(name.clone()),
                    };
                    self.$core().warn_unknown_token(&e);
                }
            }

            // -- Queries ---------------------------------------------------

            /// `measureText`'s door (index.js adds `measureText` itself):
            /// one `<text>` element as `encoder.encodeText` writes it, and
            /// the answer is what that text lays out to — `{width, height,
            /// lines}` in logical px at the context's scale, capped to
            /// `maxWidth` when given. The metrics do not scale linearly:
            /// `measured × zoom` is not `measure(size × zoom)`, because
            /// shaping rounds per size, so anything that zooms measures at
            /// the size it draws.
            #[napi(ts_return_type = "TextMetrics")]
            pub fn measure_text_binary(
                &mut self,
                stream: Float64Array,
                strings: Uint8Array,
                max_width: Option<f64>,
            ) -> Result<Json> {
                let m = binary::measure_binary(self.$core(), &stream, &strings, max_width)?;
                Ok(text_metrics_json(m))
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

            /// Every warning the core has raised so far, drained or not,
            /// oldest first — the log `warnings()` leaves behind, for a
            /// reader that is not the driver (a devtools stream).
            #[napi(ts_return_type = "Warning[]")]
            pub fn warnings_raised(&mut self) -> Json {
                warnings_json(self.$core().warnings_raised().to_vec())
            }

            /// Turns the per-frame node snapshot behind `nodes()` on or off
            /// (off unless a devtool asked: the copy is O(nodes) a frame).
            #[napi]
            pub fn set_inspect(&mut self, on: bool) {
                self.$core().set_inspect(on);
            }

            /// Turns the core's devtools panel on or off: the event stream, the runtime's facts and
            /// the tree, drawn by the core beside the app's own tree in the
            /// main window — or where `setDevtoolsDock` says — with its
            /// controls and its `Ctrl+Shift+<letter>` chords handled inside
            /// the core, so nothing of it reaches `update`. `KUI_DEVTOOLS=1`
            /// in the environment is the same call made by nobody, for a
            /// `KuiWindow`; a headless `Ctx` never reads it.
            #[napi]
            pub fn set_devtools(&mut self, on: bool) {
                self.$core().set_devtools(on);
            }

            /// Whether the devtools panel is on.
            #[napi]
            pub fn devtools(&mut self) -> bool {
                self.$core().devtools()
            }

            /// Where the devtools panel sits: `"left"`, `"right"`,
            /// `"bottom"`, `"window"` (one of its own, named
            /// `kui-devtools`) or `"off"` (hidden, the chords still live);
            /// `"side"` is the right. Throws on any other word.
            #[napi(ts_args_type = "dock: DevtoolsDock")]
            pub fn set_devtools_dock(&mut self, dock: String) -> Result<()> {
                let dock = kui_core::DevtoolsDock::parse(&dock).ok_or_else(|| {
                    err(format!(
                        "setDevtoolsDock(): {dock:?} is not left, right, bottom, window or off"
                    ))
                })?;
                self.$core().set_devtools_dock(dock);
                Ok(())
            }

            /// Where the devtools panel sits (see `setDevtoolsDock`).
            #[napi(ts_return_type = "DevtoolsDock")]
            pub fn devtools_dock(&mut self) -> String {
                self.$core().devtools_dock().name().to_string()
            }

            /// Where the last frame laid the app out in its window, in
            /// logical px: `{x, y, w, h}`, the viewport `env().viewport`
            /// sizes with its origin — `x` is the pane's width under a
            /// left dock, and the rect is the whole window with the panel
            /// off, in its own window, or anywhere but the main window.
            /// `quads()` are in physical px, so multiply by the scale
            /// (`env().viewport.scale`) to filter them: a quad inside is
            /// the app's, one outside is the dock's — but for the root's
            /// `bg`, which is the window's too and under a dock also fills
            /// the whole window beneath the pane. All zeros before the
            /// first frame, like `env().viewport`; a window's `size()` is
            /// the reading from before it.
            #[napi(ts_return_type = "Rect")]
            pub fn host_area(&mut self) -> Json {
                rect_json(self.$core().host_rect())
            }

            /// Seeds the panel's theme override, what its `T` and `A`
            /// chords cycle from: `base` is `"light"`, `"dark"` or `null`
            /// for the app's own; `accent` an `#rrggbb` string or `null`.
            #[napi(ts_args_type = "base: 'light' | 'dark' | null, accent: string | null")]
            pub fn set_devtools_theme(
                &mut self,
                base: Option<String>,
                accent: Option<String>,
            ) -> Result<()> {
                let base = match base.as_deref() {
                    None => None,
                    Some("light") => Some(kui_core::Appearance::Light),
                    Some("dark") => Some(kui_core::Appearance::Dark),
                    Some(other) => {
                        return Err(err(format!(
                            "setDevtoolsTheme(): base {other:?} is not light, dark or null"
                        )));
                    }
                };
                let accent = match accent {
                    None => None,
                    Some(s) => Some(
                        color_hex_str(&s)
                            .map_err(|e| err(format!("setDevtoolsTheme(): accent {s:?}: {e}")))?,
                    ),
                };
                self.$core().set_devtools_theme(base, accent);
                Ok(())
            }

            /// Respells the chord that moves the keyboard into the panel
            /// and back out — and brings a hidden panel back — from its
            /// default `"ctrl+shift+i"`: `"f12"`, `"mod+shift+d"` (`mod`
            /// is Command on macOS, Control elsewhere), `"⌥⌘I"`, any
            /// spelling a menu item's `accel` takes. The panel's other
            /// chords stay `Ctrl+Shift+<letter>`. With another chord set,
            /// `Ctrl+Shift+I` reaches the app like any other press. Throws
            /// on a spelling kui cannot name.
            #[napi]
            pub fn set_devtools_key(&mut self, key: String) -> Result<()> {
                let accel = kui_core::Accel::parse(&key).ok_or_else(|| {
                    err(format!("setDevtoolsKey(): {key:?} is not a chord kui can name"))
                })?;
                self.$core().set_devtools_key(accel);
                Ok(())
            }

            /// The chord `setDevtoolsKey` set, or the default, in its
            /// portable spelling: `"ctrl+shift+i"`, `"f12"`,
            /// `"super+alt+d"`.
            #[napi]
            pub fn devtools_key(&mut self) -> String {
                self.$core().devtools_key().spelling()
            }

            /// The declared devtools tab on show, by name, or `null` for
            /// one of the panel's own, the panel off or popped out.
            /// What `frame` / `setView` read once before encoding,
            /// so a `<devtoolsTab>`'s function child is called only for
            /// that tab.
            #[napi]
            pub fn devtools_shown_tab(&mut self) -> Option<String> {
                self.$core().devtools_shown_tab()
            }

            /// The node the panel's tree tab has selected, as a hex key,
            /// or `null` — what an inspector in a
            /// declared tab reads to say which node it is about.
            #[napi]
            pub fn devtools_selected(&mut self) -> Option<String> {
                self.$core().devtools_selected().map(key_str)
            }

            /// The tree row under the pointer, as a hex key, or `null`.
            #[napi]
            pub fn devtools_hovered(&mut self) -> Option<String> {
                self.$core().devtools_hovered().map(key_str)
            }

            /// The node the picker is over while picking, or `null`.
            #[napi]
            pub fn devtools_picked(&mut self) -> Option<String> {
                self.$core().devtools_picked().map(key_str)
            }

            /// Raises the panel's picker from outside it — an inspector in
            /// a `<devtoolsTab>` asking "which node?" — or puts it away.
            /// Picking happens over the app in
            /// the main window: `devtoolsPicked()` is the node under the
            /// pointer while it is up, and the press lands it in
            /// `devtoolsSelected()`. Raised while a declared tab is on
            /// show, the pick leaves that tab up; raised otherwise — a
            /// tab named but not declared yet included — it is the
            /// `Ctrl+Shift+P` pick and shows the tree tab. A hidden
            /// panel comes back docked.
            #[napi]
            pub fn set_devtools_pick(&mut self, on: bool) {
                self.$core().set_devtools_pick(on);
            }

            /// Whether the panel's picker is up.
            #[napi]
            pub fn devtools_picking(&mut self) -> bool {
                self.$core().devtools_picking()
            }

            /// Shows the panel's tab named `name` from the app's side —
            /// what the strip's click and `Ctrl+Shift+N` do, for a command
            /// that jumps to the app's own tab. `name` is one
            /// of the panel's own (`facts`, `events`, `tree`, in any case)
            /// or a `<devtoolsTab>`'s, exactly as declared. A declared
            /// name the panel does not list
            /// yet is kept and shows once a frame declares it; the return
            /// says whether the panel lists it now. A hidden panel comes
            /// back docked; `setDevtools(true)` is still the app's to
            /// call. Call it once, not every frame: it would pin the strip
            /// against the user's own clicks.
            #[napi(ts_args_type = "name: DevtoolsTab | (string & {})")]
            pub fn set_devtools_tab(&mut self, name: String) -> bool {
                self.$core().set_devtools_tab(&name)
            }

            /// The tab the panel is on, by name: one of its own or a
            /// `<devtoolsTab>`'s — the selection itself, panel on or off,
            /// unlike `devtoolsShownTab()`, which is the encoder's reading
            /// of a declared tab on show.
            #[napi(ts_return_type = "DevtoolsTab | (string & {})")]
            pub fn devtools_current_tab(&mut self) -> String {
                self.$core().devtools_current_tab()
            }

            /// Selects a node in the panel's tree tab from outside it and
            /// reveals it there, as the picker does; `null` clears. `key`
            /// is a hex key an event carried or `keyOf` answered.
            #[napi]
            pub fn set_devtools_selected(&mut self, key: Option<String>) -> Result<()> {
                let key = match key {
                    None => None,
                    Some(k) => Some(parse_key(&k)?),
                };
                self.$core().set_devtools_selected(key);
                Ok(())
            }

            /// The key legend the panel's facts tab shows: `[keys, what]`
            /// pairs.
            #[napi(ts_args_type = "legend: [string, string][]")]
            pub fn set_devtools_legend(&mut self, legend: Vec<Vec<String>>) {
                let rows: Vec<(&str, &str)> = legend
                    .iter()
                    .filter_map(|r| Some((r.first()?.as_str(), r.get(1)?.as_str())))
                    .collect();
                self.$core().set_devtools_legend(&rows);
            }

            /// The last finished frame's nodes in tree order, each with what
            /// it is, the label it was opened under, where layout put it
            /// (in the app's viewport, like `layoutOf`), and
            /// the declarations that explain the rest — what a tree view
            /// and a node inspector are built from. Empty until
            /// `setInspect(true)` and a frame after it.
            #[napi(ts_return_type = "NodeInfo[]")]
            pub fn nodes(&mut self) -> Json {
                Json::Array(self.$core().nodes().iter().map(node_info_json).collect())
            }

            /// The prop names the encoder threw away while lowering a tree,
            /// as `[element, name]` pairs, raised as `unknown-prop` warnings
            /// (see `Warning`). A name outside the schema never reaches the
            /// binary stream, so the encoder is the only side that sees it;
            /// `frame` / `setView` report what they dropped through here.
            /// Behind the same `setDiagnostics` gate, and once per name. A
            /// pair under `protocol().menuItem.name` is a key a `<select>`
            /// option object carried that no menu row reads.
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
                // Input, so the sound it may have asked for (`clickSound`)
                // goes to the device now, before the frame the redraw
                // draws reads `env.audio` — the order the runner's own
                // input path keeps (backlog F56).
                self.$audio();
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
                let Some(key) = resolve_query(self.$core(), &key) else {
                    return Ok(false);
                };
                Ok(self.$core().is_hovered(key))
            }

            /// Press state as of the last frame; `key` is either spelling,
            /// as for `isHovered`.
            #[napi]
            pub fn is_pressed(&mut self, key: String) -> Result<bool> {
                let Some(key) = resolve_query(self.$core(), &key) else {
                    return Ok(false);
                };
                Ok(self.$core().is_pressed(key))
            }

            /// Whether files dragged in from the OS are over `key` — for
            /// drop-dependent layout; the colour swap is the
            /// `dropBg` prop. `key` is either spelling, as for `isHovered`.
            #[napi]
            pub fn is_drop_target(&mut self, key: String) -> Result<bool> {
                let Some(key) = resolve_query(self.$core(), &key) else {
                    return Ok(false);
                };
                Ok(self.$core().is_drop_target(key))
            }

            /// The `onDrop` zone the dragged files are over, as the hex key
            /// an event carries, or null — what a driver answers the OS
            /// with after each `dragFiles`, and what a test reads to say a
            /// zone was found.
            #[napi]
            pub fn drop_target(&mut self) -> Option<String> {
                self.$core().drop_target().map(key_str)
            }

            // -- Pointer ----------------------------------------------------

            /// The pointer shape for where the pointer is now, in the `cursor`
            /// prop's own vocabulary: whatever the topmost node under it
            /// declared with `cursor`, and when it declared nothing,
            /// `'text'` over an editor or a `selectable` scope and
            /// `'default'` over everything else — an `onClick`, `focusable`
            /// or `onDrag` node included, as a native button is, so a hand
            /// or a grab is the view's to declare (the stock `button`
            /// declares `'pointer'` itself). A captured drag keeps the
            /// dragged node's shape wherever the pointer goes; no pointer at
            /// all is `'default'`. A window applies it to the real cursor by
            /// itself and only touches it when the answer changes; this is
            /// for tests and drivers.
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
                let Some(key) = resolve_query(core, &key) else {
                    return Ok(false);
                };
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

            /// The caret's blink phase — `true` draws it. A
            /// custom editor reads it in `view` and skips its caret node
            /// on the off phase, keeping the `caret` row on its `line`
            /// either way; the window's clock sets it while a focused
            /// `<edit>` or such a line has a caret, and parks it hidden
            /// while the window has no keyboard. Always `true` headless.
            #[napi]
            pub fn caret_visible(&mut self) -> bool {
                self.$core().caret_visible()
            }

            /// The frame clock in seconds (backlog F134): the one the
            /// tweens and cycles read, `setTime` or the loop's own, 0
            /// before either. Read a deadline off it rather than off
            /// `Date.now()`, so `advance` moves both.
            #[napi]
            pub fn now(&mut self) -> f64 {
                self.$core().now()
            }

            /// Asks for a frame at `at` on the frame clock (backlog F135),
            /// in seconds: a toast's expiry, a sequence's next beat. Nothing
            /// is owed until then, so `animating()` stays false; a window
            /// sleeps to it and draws, and the earliest time asked for wins.
            /// A headless loop's `advance` past it draws the frame it ends
            /// on, as it does for a `tick`.
            #[napi]
            pub fn request_frame_at(&mut self, at: f64) {
                self.$core().request_frame_at(at);
            }

            /// The exit the node `key` leaves by if it leaves in the frame
            /// that finishes next (backlog F136), over the `exit` it
            /// declared: `{dx: 400, opacity: 0}`, the same fields as
            /// `exit`. A card a button throws aside is removed in the same
            /// handler, with no frame drawn first to aim it. It aims a node
            /// that declares an `exit`, with that node's transition, and is
            /// forgotten when that frame finishes.
            #[napi]
            pub fn exit_with(&mut self, key: String, exit: Json) -> Result<()> {
                let e = kui_core::enter::parse(&value_of(&exit))
                    .map_err(|m| napi::Error::from_reason(format!("exitWith: {m}")))?;
                let core = self.$core();
                let key = resolve_key(core, &key)?;
                core.set_exit(key, e);
                Ok(())
            }

            /// The frame-clock time the next asked-for frame is due at, in
            /// seconds; `null` when none was asked for.
            #[napi]
            pub fn next_frame_at(&mut self) -> Option<f64> {
                self.$core().next_frame_at()
            }

            /// Whether there is a caret to blink: a focused `<edit>`'s, or
            /// the `caret` a `line` under the focused sink declares — unless
            /// the line declares it `caretSolid`, which
            /// anchors and reads but arms no clock. What the window's
            /// clock is armed on; headless, what a test reads to see that
            /// an idle view asks for no frame.
            #[napi]
            pub fn has_caret(&mut self) -> bool {
                self.$core().has_caret()
            }

            /// The driver's half of the blink: sets the phase. A window
            /// runs its own clock; headless, a test drives it to see the
            /// off phase drawn.
            #[napi]
            pub fn set_caret_visible(&mut self, visible: bool) {
                self.$core().set_caret_visible(visible);
                self.$redraw();
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
            /// `ambiguous-key` — among the host's own first, and a
            /// plugin filling a slot is answered from its own nodes only.
            #[napi]
            pub fn key_of(&mut self, label: String) -> Option<String> {
                self.$core().key_of(&label).map(key_str)
            }

            /// The hex key of the first node in the last finished frame
            /// whose accessible name is `name` (backlog F137) — its
            /// `label` prop, else its own text, else a control's derived
            /// name — so a test clicks "the button named Like" as a reader
            /// would: `ctx.click(ctx.keyNamed('Like'))`. Not `keyOf`'s key
            /// label, which a reader never hears. Null when no node has the
            /// name; two raise `ambiguous-name`.
            #[napi]
            pub fn key_named(&mut self, name: String) -> Option<String> {
                self.$core().key_named(&name).map(key_str)
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

            /// Enters a focus region — a box declared `focusRegion`, named by
            /// the label its `key` prop declares or by the hex key an event
            /// carried — or the main ring for `null`. Focus lands on what that
            /// ring last held if the node is still there, else its
            /// `initialFocus`, else its first stop, and shows.
            ///
            /// Resolved when the next frame finishes, like `focusNext`, so
            /// the `update` that toggles a dock on may enter it in the same
            /// turn — which is why a label is taken as a name to hold rather
            /// than resolved now: the node need not exist yet. A frame that
            /// then declares no `focusRegion` under the name raises
            /// `focus-region-without-node` and moves nothing.
            #[napi]
            pub fn focus_region(&mut self, key: Option<String>) {
                let core = self.$core();
                match key {
                    None => core.focus_region(None),
                    Some(s) => match hex_key(&s) {
                        Some(k) => core.focus_region(Some(k)),
                        None => core.focus_region_by_label(&s),
                    },
                }
                self.$redraw();
            }

            /// The focus region in effect — the hex key of the `focusRegion`
            /// node whose ring Tab walks — or `null` for the main ring. What
            /// a chord that toggles between a dock and the app reads to know
            /// which way it is going.
            #[napi]
            pub fn region(&mut self) -> Option<String> {
                self.$core().region().map(key_str)
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
            /// `focus`. A label the last frame did not declare is resolved
            /// when the coming frame finishes, so a row that frame declares
            /// for the first time is reachable by name too; one it does not
            /// declare either is a `label-without-node` warning.
            #[napi]
            pub fn reveal(&mut self, key: String) -> Result<()> {
                let core = self.$core();
                match hex_key(&key).or_else(|| core.key_of(&key)) {
                    Some(k) => core.reveal(k),
                    None => core.reveal_label(&key),
                }
                self.$redraw();
                Ok(())
            }

            /// A scroll container's retained offset `{x, y}` as the last layout
            /// clamped it (positive = content moved up / left) — the number to
            /// keep in a model and hand back to `setScroll`. Zero for a node
            /// that never scrolled.
            #[napi(ts_return_type = "ScrollOffset")]
            pub fn scroll_offset(&mut self, key: String) -> Result<Json> {
                let Some(key) = resolve_query(self.$core(), &key) else {
                    return Ok(offset_json(kui_core::Vec2::ZERO));
                };
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
                let Some(key) = resolve_query(self.$core(), &key) else {
                    return Ok(None);
                };
                Ok(geometry_json(self.$core().scroll_geometry(key)))
            }

            /// The rect the last frame laid `key` out at, `{x, y, w, h}`
            /// in logical viewport px, for a node that declared `onLayout`
            /// — the `layout` event's numbers, read back during the next
            /// build with no event and no model field; `null` for any
            /// other key. Read while building, it
            /// describes the previous frame, like `scrollGeometry`.
            #[napi(ts_return_type = "{ x: number, y: number, w: number, h: number } | null")]
            pub fn layout_of(&mut self, key: String) -> Result<Option<Json>> {
                let Some(key) = resolve_query(self.$core(), &key) else {
                    return Ok(None);
                };
                Ok(self.$core().layout_of(key).map(|r| readback(&r.to_value())))
            }

            /// Where a point lands in the text a keyed node drew: a byte
            /// offset into its text and the visual row within that node —
            /// counted across every run the key covers, so a `line` row of
            /// inline runs is one row and a wrapped run as many as it
            /// wrapped to; not the ordinal `line` node a pointer event's
            /// `line` names — or null for a key that drew no
            /// text. A `role="none"` subtree under the key (a gutter) is
            /// not its text, as the access tree reads it. `x`/`y` are the logical viewport px a
            /// `click` or `drag` event carries, so a custom editor turns the
            /// event into a caret position with one call — no prefix
            /// measuring, no cell-width arithmetic. A node holding several
            /// text runs (a `line` row of token runs) answers across them
            /// in order, the way the access tree reads the line. Answered
            /// from the frame that finished: the layout the pointer was
            /// over.
            #[napi(ts_return_type = "TextHit | null")]
            pub fn text_hit(&mut self, key: String, x: f64, y: f64) -> Result<Option<Json>> {
                let Some(key) = resolve_query(self.$core(), &key) else {
                    return Ok(None);
                };
                Ok(self
                    .$core()
                    .text_hit(key, Vec2::new(x as f32, y as f32))
                    .map(text_hit_json))
            }

            /// Opens a context menu at `(x, y)` over the node `key`, with
            /// `items` as plain objects: `{label, role, enabled, id,
            /// accel}`, all but `label` optional. `role` is one of
            /// `custom` (the default), `separator`, `cut`, `copy`,
            /// `paste`, `selectAll` or `lookUp`; the standard ones take
            /// their own wording when `label` is empty, and the core
            /// performs the ones it can.
            ///
            /// Choosing a row posts `{kind:"menu", role, item}` on `key`
            /// and closes the menu; a press outside it or Escape closes it
            /// with nothing posted. What an app answering its own
            /// `onContextMenu` calls — and what the core calls itself for
            /// a right-click nobody claimed, so the two menus are one
            /// implementation.
            #[napi(ts_args_type = "key: string, x: number, y: number, items: MenuItemInput[]")]
            pub fn open_menu(&mut self, key: String, x: f64, y: f64, items: Json) -> Result<bool> {
                let Some(target) = resolve_query(self.$core(), &key) else {
                    return Ok(false);
                };
                let (items, stray) = menu_items(&items)?;
                warn_stray_menu_keys(self.$core(), stray);
                self.$core().open_menu(kui_core::Menu::new(
                    target,
                    Vec2::new(x as f32, y as f32),
                    items,
                ));
                self.$redraw();
                Ok(true)
            }

            /// Drains what choosing a menu row left for the host: the
            /// clipboard, which is the host's in this library. Each entry
            /// is `{kind}` — `"setClipboard"` with `text` (and `html`
            /// where there is formatting to carry), `"paste"` asking for
            /// what is on the clipboard (deliver it back with `commit()`,
            /// which a focused editor takes as typing and a focused
            /// `onKey` sink hears as `{kind:"text"}`),
            /// or `"lookUp"` with the `text` to show a definition panel
            /// for at `x`, `y`.
            ///
            /// A windowed app never needs this — the driver drains it —
            /// but a headless one does: nothing else empties the queue,
            /// and a Copy nobody drains is a copy that never happened.
            /// Asks for the platform's Open, Save or folder dialog:
            /// `{mode, multiple, title, filters: [{name, extensions}],
            /// directory, fileName, tag}`, every field optional. The answer
            /// is a `{kind:"files", paths, tag}` event — `paths` empty when
            /// the user cancelled. A window's runner shows the dialog; a
            /// headless context queues it for `takeFileRequests`. False when
            /// one is already out: one dialog at a time.
            #[napi(ts_args_type = "dialog?: FileDialogOptions")]
            pub fn request_files(&mut self, dialog: Option<Json>) -> Result<bool> {
                let v = dialog.as_ref().map_or(Value::Null, value_of);
                let dialog = kui_core::FileDialog::from_value(&v)
                    .map_err(|e| err(format!("requestFiles(): {e}")))?;
                let asked = self.$core().request_files(dialog);
                self.$redraw();
                Ok(asked)
            }

            /// Whether a file dialog asked for is still unanswered.
            #[napi]
            pub fn awaiting_files(&mut self) -> bool {
                self.$core().awaiting_files()
            }

            /// The file dialog asked for and not yet taken — at most one —
            /// as `requestFiles` took it, for a host that shows it itself.
            /// A window never needs this: its runner drains and shows it.
            /// Answer with `answerFiles`.
            #[napi(ts_return_type = "FileDialogOptions[]")]
            pub fn take_file_requests(&mut self) -> Json {
                Json::Array(
                    self.$core()
                        .take_file_requests()
                        .iter()
                        .map(|d| readback(&d.to_value()))
                        .collect(),
                )
            }

            #[napi(ts_return_type = "MenuAction[]")]
            pub fn take_menu_actions(&mut self) -> Result<Json> {
                let out: Vec<Json> = self
                    .$core()
                    .take_menu_actions()
                    .into_iter()
                    .map(|a| {
                        let mut o = JsonMap::new();
                        match a {
                            kui_core::MenuAction::SetClipboard { text, html } => {
                                o.insert("kind".into(), Json::from("setClipboard"));
                                o.insert("text".into(), Json::from(text));
                                o.insert("html".into(), html.map_or(Json::Null, Json::String));
                            }
                            kui_core::MenuAction::SetClipboardSecret { text } => {
                                o.insert("kind".into(), Json::from("setClipboardSecret"));
                                o.insert("text".into(), Json::from(text));
                            }
                            kui_core::MenuAction::Paste => {
                                o.insert("kind".into(), Json::from("paste"));
                            }
                            kui_core::MenuAction::LookUp { text, at } => {
                                o.insert("kind".into(), Json::from("lookUp"));
                                o.insert("text".into(), Json::from(text));
                                o.insert("x".into(), Json::from(at.x));
                                o.insert("y".into(), Json::from(at.y));
                            }
                        }
                        Json::Object(o)
                    })
                    .collect();
                Ok(Json::Array(out))
            }

            /// Puts `text` on the system clipboard — the action a menu's
            /// Copy queues, with a door on it for an `onKey` sink that
            /// hears the raw `Ctrl-c` and had nowhere to bind it.
            /// `html` is a second flavour beside the text for the
            /// host to offer, never in place of it. A window applies it
            /// at its next drain (after every input and every frame); a
            /// headless `Ctx` hands it out through `takeMenuActions()`.
            #[napi]
            pub fn set_clipboard(&mut self, text: String, html: Option<String>) {
                self.$core().set_clipboard(text, html);
            }

            /// Puts a secret on the system clipboard the way a password
            /// manager does: a window writes it marked
            /// concealed and transient — `org.nspasteboard.ConcealedType`
            /// and `TransientType` on macOS, the exclusion formats on
            /// Windows — so no clipboard manager shows or keeps it. A
            /// headless `Ctx` hands it out through `takeMenuActions()` as
            /// `{kind:"setClipboardSecret", text}`.
            #[napi]
            pub fn set_clipboard_secret(&mut self, text: String) {
                self.$core().set_clipboard_secret(text);
            }

            /// Asks for what is on the clipboard — the action a menu's
            /// Paste queues. A window reads the clipboard and hands the
            /// text back as a paste: a focused `<edit>` takes it as
            /// typing, and a focused `onKey` sink hears it as
            /// `{kind:"text", text, tag}` — with `concealed: true` /
            /// `transient: true` where the pasteboard marked it so
            /// — so an app that owns its text inserts a
            /// paste the way it inserts a committed IME string and never
            /// reads the clipboard itself. Headless, the request comes out
            /// of `takeMenuActions()` as `{kind:"paste"}` and the test
            /// answers it with `paste(text, marks)` or `commit(...)`.
            #[napi]
            pub fn request_paste(&mut self) {
                self.$core().request_paste();
            }

            /// Whether a paste asked for is still unanswered: one ask at a
            /// time — a second `requestPaste` while one is out is dropped,
            /// and the `commit` that answers it (an empty one for an empty
            /// clipboard) lets the next through.
            #[napi]
            pub fn awaiting_paste(&mut self) -> bool {
                self.$core().awaiting_paste()
            }

            /// The menu this window has open, or null:
            /// `{target, x, y, items}`. What a host rendering menus itself
            /// reads after `setNativeMenus(true)` — the core then keeps
            /// the menu as state and draws none of it — and answers with
            /// `activateMenuItem` or `closeMenu`. A row reads exactly as a
            /// `menuBar()` row does (`menu_item_json`).
            #[napi(ts_return_type = "OpenMenu | null")]
            pub fn menu(&mut self) -> Result<Option<Json>> {
                let Some(menu) = self.$core().menu().cloned() else {
                    return Ok(None);
                };
                let items: Vec<Json> = menu.items.iter().map(menu_item_json).collect();
                let mut o = JsonMap::new();
                o.insert("target".into(), Json::from(key_str(menu.target)));
                o.insert("x".into(), Json::from(menu.at.x));
                o.insert("y".into(), Json::from(menu.at.y));
                o.insert("items".into(), Json::Array(items));
                Ok(Some(Json::Object(o)))
            }

            /// Tells the core this host shows menus itself. It then keeps
            /// the open menu as state and draws none of it: read it with
            /// `menu()`, show it, and report back with `activateMenuItem`
            /// or `closeMenu`. Off by default, which is the menu this
            /// library draws.
            #[napi]
            pub fn set_native_menus(&mut self, on: bool) -> Result<()> {
                self.$core().set_native_menus(on);
                self.$redraw();
                Ok(())
            }

            /// The application menu the frame declared, or null:
            /// `{revision, menus: [{label, enabled, items}]}`. What a
            /// host with a menu bar of its own reads after
            /// `setNativeMenuBar(true)`; `revision` changes only when the
            /// declaration does, so a host rebuilds nothing until it moves.
            #[napi(ts_return_type = "MenuBarState | null")]
            pub fn menu_bar(&mut self) -> Result<Option<Json>> {
                let revision = self.$core().menu_bar_revision();
                let Some(bar) = self.$core().menu_bar().cloned() else {
                    return Ok(None);
                };
                let menus: Vec<Json> = bar
                    .menus
                    .iter()
                    .map(|menu| {
                        let items: Vec<Json> = menu.items.iter().map(menu_item_json).collect();
                        let mut o = JsonMap::new();
                        o.insert("label".into(), Json::from(menu.label.clone()));
                        o.insert("enabled".into(), Json::Bool(menu.enabled));
                        o.insert("items".into(), Json::Array(items));
                        Json::Object(o)
                    })
                    .collect();
                let mut o = JsonMap::new();
                o.insert("revision".into(), Json::from(revision));
                o.insert("menus".into(), Json::Array(menus));
                Ok(Some(Json::Object(o)))
            }

            /// Tells the core the platform owns the menu bar, so
            /// `<menuBar/>` draws nothing and this host is the one handing
            /// the declaration over (`menuBar()`) and reporting what was
            /// chosen (`activateMenuBarItem`). Off by default, which is
            /// the bar this library draws.
            #[napi]
            pub fn set_native_menu_bar(&mut self, on: bool) -> Result<()> {
                self.$core().set_native_menu_bar(on);
                self.$redraw();
                Ok(())
            }

            /// Reports that the platform's menu bar chose row `item` of
            /// menu `menu` — the same path a press on the drawn bar's row
            /// takes. False for a row that is not there.
            #[napi]
            pub fn activate_menu_bar_item(&mut self, menu: u32, item: u32) -> Result<bool> {
                let events = self
                    .$core()
                    .activate_menu_bar_item(menu as usize, item as usize);
                let any = !events.is_empty();
                self.$take(events);
                self.$redraw();
                Ok(any)
            }

            /// `activateMenuBarItem` for a row inside a submenu of menu
            /// `menu`, by its path through the rows' `items`. False for a
            /// row that is not there or that opens a submenu.
            #[napi]
            pub fn activate_menu_bar_path(&mut self, menu: u32, path: Vec<u32>) -> Result<bool> {
                let path: Vec<usize> = path.into_iter().map(|i| i as usize).collect();
                let events = self.$core().activate_menu_bar_path(menu as usize, &path);
                let any = !events.is_empty();
                self.$take(events);
                self.$redraw();
                Ok(any)
            }

            /// Tells the core this host can show the platform's definition
            /// panel. The standard Look Up row is then offered where it
            /// means something, and a force click over text asks for one.
            #[napi]
            pub fn set_lookup_available(&mut self, on: bool) -> Result<()> {
                self.$core().set_lookup_available(on);
                Ok(())
            }

            /// Reports that the host's own menu chose row `index` — the
            /// same path a press on the drawn menu's row takes. An index
            /// past the end closes the menu and posts nothing. False when
            /// nothing was taken: no menu was open, or the row cannot be
            /// chosen — disabled, or a separator — in which case the menu
            /// stays open and nothing is posted.
            #[napi]
            pub fn activate_menu_item(&mut self, index: u32) -> Result<bool> {
                let Some(events) = self.$core().activate_menu_item(index as usize) else {
                    return Ok(false);
                };
                self.$take(events);
                self.$redraw();
                Ok(true)
            }

            /// `activateMenuItem` for a row inside a submenu, by its path
            /// through the rows' `items`: `[2, 0]` is the first row of the
            /// third row's submenu. False where `activateMenuItem` is, and
            /// for a row that opens a submenu, which is never chosen.
            #[napi]
            pub fn activate_menu_path(&mut self, path: Vec<u32>) -> Result<bool> {
                let path: Vec<usize> = path.into_iter().map(|i| i as usize).collect();
                let Some(events) = self.$core().activate_menu_path(&path) else {
                    return Ok(false);
                };
                self.$take(events);
                self.$redraw();
                Ok(true)
            }

            /// Closes whatever menu is open; true when there was one.
            #[napi]
            pub fn close_menu(&mut self) -> Result<bool> {
                let closed = self.$core().close_menu();
                if closed {
                    self.$redraw();
                }
                Ok(closed)
            }

            /// Asks for the selection as text: `{ text, asked }`.
            ///
            /// `text` is the selection when the core has all of it. When
            /// the selection reaches rows a virtual list never built,
            /// `asked` is true instead and a `{kind:"selectionrange",
            /// from:{index, byte}, to:{index, byte}}` event is posted on
            /// the scope — the rows behind that gap are the app's, so the
            /// app answers with `answerSelectionRange`, and the answer is
            /// what reaches the clipboard.
            #[napi(ts_return_type = "{ text: string | null, asked: boolean }")]
            pub fn request_copy(&mut self) -> Result<Json> {
                let (text, asked) = match self.$core().request_copy() {
                    kui_core::CopyRequest::Ready(t) => (Some(t), false),
                    kui_core::CopyRequest::Asked => (None, true),
                    kui_core::CopyRequest::Nothing => (None, false),
                };
                let mut o = JsonMap::new();
                o.insert("text".into(), text.map_or(Json::Null, Json::String));
                o.insert("asked".into(), Json::Bool(asked));
                Ok(Json::Object(o))
            }

            /// Answers a `selectionrange` ask with the text for the range
            /// it named, whole. False when nothing asked — a late answer
            /// cannot overwrite what has been copied since.
            #[napi]
            pub fn answer_selection_range(&mut self, text: String) -> Result<bool> {
                Ok(self.$core().answer_selection_range(&text))
            }

            /// The window's selected text: what a `selectable` scope has
            /// selected, or the focused `<edit>`'s selection, whichever
            /// the window holds — starting either clears the other, so
            /// there is never a choice to make. Null with no selection,
            /// `""` when a selection exists but covers nothing (a press
            /// that placed both ends together).
            #[napi]
            pub fn selection_text(&mut self) -> Result<Option<String>> {
                Ok(self.$core().copy_selection())
            }

            /// The text selection's two ends as the drag made them:
            /// `{anchor: {index, byte}, focus: {index, byte}}`, `index` the
            /// data index of the virtualised row the end is in (null
            /// outside every virtualised row — the `index` a
            /// `selectionrange` ask would name) and `byte` the offset in
            /// that row's own text. Directed, so a Shift-click that kept
            /// the anchor reads as one. Null with no text
            /// selection; a grid's is `cellSelection()`.
            #[napi(ts_return_type = "SelectionEnds | null")]
            pub fn selection_ends(&mut self) -> Result<Json> {
                Ok(match self.$core().selection_ends() {
                    Some((a, f)) => readback(&Value::map([
                        ("anchor", a.to_value()),
                        ("focus", f.to_value()),
                    ])),
                    None => Json::Null,
                })
            }

            /// A `cells` grid's selection, the window's when it lives in
            /// one: the grid's key, `anchor` and `focus` as the drag made
            /// them — each an absolute `line` (`originLine` plus the row,
            /// so a scroll does not move it) and a `col` — and `block`
            /// for a rectangular one. Null when the
            /// window's selection is not a grid's; a text selection's ends
            /// are `selectionEnds()`.
            #[napi(ts_return_type = "CellSelection | null")]
            pub fn cell_selection(&mut self) -> Json {
                match self.$core().cell_selection() {
                    Some(sel) => readback(&sel.to_value(HEX)),
                    None => Json::Null,
                }
            }

            /// The selection as HTML, carrying the formatting the text
            /// declared — bold, italic, a span's own colour — and *not*
            /// the node's colour, which is the app's theme rather than
            /// the text's.
            /// Null with no text selection. Meant as a second clipboard
            /// flavour beside the plain text, never instead of it.
            #[napi]
            pub fn selection_html(&mut self) -> Result<Option<String>> {
                Ok(self.$core().selection_html())
            }

            /// Selects every run inside the selection scope a keyed node
            /// declared (`selectable`), first byte to last — Select All,
            /// scoped. False when that node drew no text, or is not a
            /// scope. Text the frame built but never drew is included:
            /// the selection is over the scope's text, not over what fits
            /// on screen.
            #[napi]
            pub fn select_all_in(&mut self, key: String) -> Result<bool> {
                let Some(key) = resolve_query(self.$core(), &key) else {
                    return Ok(false);
                };
                Ok(self.$core().select_all_in(key))
            }

            /// Drops the window's selection, whichever it is. True when
            /// there was one to drop.
            #[napi]
            pub fn clear_selection(&mut self) -> Result<bool> {
                Ok(self.$core().clear_selection())
            }

            /// The caret rect for a byte offset in the text a keyed node
            /// drew: logical viewport px, zero wide, one line tall — where
            /// a caret, a selection edge or an IME candidate window goes.
            /// A byte past the text is the end; null for a key that drew
            /// no text.
            #[napi(ts_return_type = "Rect | null")]
            pub fn caret_rect(&mut self, key: String, byte: f64) -> Result<Option<Json>> {
                let Some(key) = resolve_query(self.$core(), &key) else {
                    return Ok(None);
                };
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
                let core = self.$core();
                let at = Vec2::new(x as f32, y as f32);
                // A label not declared yet waits for the coming frame, as
                // `reveal`'s does (backlog DX15).
                match hex_key(&key).or_else(|| core.key_of(&key)) {
                    Some(k) => core.set_scroll(k, at),
                    None => core.set_scroll_label(&key, at),
                }
                self.$redraw();
                Ok(())
            }

            /// Moves the scroll container `key` by the content that moved
            /// under it — `drawn` for where the content is drawn (and an
            /// eased leg's start), `target` for the retained offset — with
            /// no ease asked or ended and no frame asked for: a correction to
            /// the frame the view is building. What `list()` calls when the
            /// rows it measured came out another height than the estimate
            /// they stood at, so the row under the pointer stays put.
            /// A label nothing declared yet is the first
            /// frame, which has nothing to correct.
            #[napi]
            pub fn shift_scroll(&mut self, key: String, drawn: f64, target: f64) {
                if let Some(key) = resolve_query(self.$core(), &key) {
                    self.$core().shift_scroll(
                        key,
                        Vec2::new(0.0, drawn as f32),
                        Vec2::new(0.0, target as f32),
                    );
                }
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
                let Some(key) = resolve_query(self.$core(), &key) else {
                    return Ok(None);
                };
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
            ///
            /// A redraw is asked for only when the text reached an editor.
            /// A held seed changed nothing on screen, and the frame that
            /// will — the view that declares the editor — is the app's:
            /// a redraw here re-lowered the *retained* tree, which declares
            /// no editor, and that was the frame the hold expired on when
            /// the call came from a `dispatch` outside the loop
            /// (`runWindowed` draws that model before it pumps).
            #[napi]
            pub fn set_edit_text(&mut self, key: String, text: String) -> Result<()> {
                let core = self.$core();
                let applied = match hex_key(&key) {
                    Some(k) => core.set_edit_text(k, &text),
                    None => core.set_edit_text_by_label(&key, &text),
                };
                if applied {
                    self.$redraw();
                }
                Ok(())
            }
        }
    };
}

#[napi]
impl Ctx {
    /// A headless context is one window, the main: this answers whether
    /// `window` names it (`"main"`, `0`, or left out) and addresses
    /// nothing else — the same door `KuiWindow` has, so a loop or a test
    /// can call it on either surface.
    #[napi(ts_args_type = "window?: string | number")]
    pub fn use_window(&mut self, window: Option<Either<String, f64>>) -> bool {
        match window {
            None => true,
            Some(Either::A(name)) => name == kui_core::session::MAIN_WINDOW_NAME,
            Some(Either::B(id)) => id == 0.0,
        }
    }
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
    /// The addressed window's core — the main window's when the
    /// addressed one is not open (it closed, or has not opened yet),
    /// which `useWindow` reported at the time.
    fn core_mut(&mut self) -> &mut Core {
        let id = if self.runner.core_mut_of(self.addressed).is_some() {
            self.addressed
        } else {
            kui_core::WindowId::MAIN
        };
        self.runner
            .core_mut_of(id)
            .expect("the main window's core always exists")
    }

    /// Aims every per-window door at `window`. `None` is the main window.
    /// Returns whether that window is open now; the doors address the
    /// main window until it is.
    fn address(&mut self, window: Option<WindowRef>) -> bool {
        let id = match window {
            None => Some(kui_core::WindowId::MAIN),
            Some(WindowRef::Id(id)) => Some(kui_core::WindowId(id)),
            Some(WindowRef::Name(name)) => self.runner.window_id(&name),
        };
        let Some(id) = id else {
            self.addressed = kui_core::WindowId::MAIN;
            return false;
        };
        self.addressed = id;
        self.runner.core_mut_of(id).is_some()
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
            .route_events(events, |app, ev, _| app.events.push(ev));
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
/// and about what.
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
    readback(&tree.to_value(HEX))
}

fn warnings_json(warnings: Vec<kui_core::Warning>) -> Json {
    Json::Array(
        warnings
            .iter()
            .map(|w| readback(&w.to_value(HEX)))
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
            "no usable font face in {path:?} (unreadable, not TTF/OTF/TTC, \
             or no face with the head, hhea and hmtx tables its glyphs are measured by)"
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
    Json::Array(cmds.iter().map(|c| readback(&c.to_value(HEX))).collect())
}

fn add_font_impl(core: &mut Core, data: &[u8]) -> Result<String> {
    core.add_font_data(data.to_vec())
        .map(font_str)
        .ok_or_else(|| {
            err(
                "no usable font face in the data (expected TTF/OTF/TTC bytes, \
                 a face with the head, hhea and hmtx tables its glyphs are measured by)",
            )
        })
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

fn update_image_impl(
    core: &mut Core,
    id: ImageId,
    width: u32,
    height: u32,
    rgba: &[u8],
) -> Result<()> {
    let expected = (width as usize) * (height as usize) * 4;
    if width == 0 || height == 0 || rgba.len() != expected {
        return Err(err(format!(
            "rgba must be width*height*4 = {expected} bytes, got {}",
            rgba.len()
        )));
    }
    // Into a buffer the core recycles, not a fresh `to_vec` a frame
    // (backlog W20).
    core.update_image_with(id, width, height, |px| px.copy_from_slice(rgba));
    Ok(())
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

/// `KuiWindow`'s `system` option is `setEnv`'s partial over a blank
/// reading, so what is left out is unknown — "not pinned" to the runner's
/// merge.
#[cfg(test)]
mod pinned_system_tests {
    use super::*;

    #[test]
    fn the_partial_pins_what_it_names_and_nothing_else() {
        let sys = pinned_system(&serde_json::json!({ "motion": "reduced" })).unwrap();
        assert_eq!(
            sys,
            SystemEnv {
                motion: MotionPref::Reduced,
                ..Default::default()
            }
        );
        // The explicit unknowns are the default too: pinned to nothing.
        let sys = pinned_system(&serde_json::json!({
            "motion": "unknown", "appearance": "unknown", "accent": null, "locale": null
        }))
        .unwrap();
        assert_eq!(sys, SystemEnv::default());
        // Every field, in `env()`'s spellings.
        let sys = pinned_system(&serde_json::json!({
            "appearance": "dark", "accent": "#d2691e", "motion": "full", "locale": "pt-BR"
        }))
        .unwrap();
        assert_eq!(sys.appearance, Appearance::Dark);
        assert_eq!(sys.accent, Some(Color::hex(0xd2691eff)));
        assert_eq!(sys.motion, MotionPref::Full);
        assert_eq!(
            sys.locale.map(|l| l.as_str().to_string()),
            Some("pt-BR".into())
        );
        // A misspelling says which door it came through.
        let e = pinned_system(&serde_json::json!({ "motion": "less" })).unwrap_err();
        assert!(
            e.reason.starts_with("KuiWindow `system`: system.motion"),
            "{}",
            e.reason
        );
        assert!(
            pinned_system(&serde_json::json!("reduced")).is_err(),
            "an object, not a string"
        );
    }
}

#[cfg(test)]
mod frame_stats_tests {
    use super::*;

    #[test]
    fn frame_stats_shape_before_and_after_frames() {
        let mut st = FrameStats::default();
        let empty = frame_stats_json(&st, 0, 0);
        assert_eq!(empty["frames"], Json::from(0));
        assert_eq!(empty["framesTotal"], Json::from(0));
        assert_eq!(empty["pumps"], Json::from(0));
        assert_eq!(empty["wokenPumps"], Json::from(0));
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
        // F94: the woken pumps are their own count, not the pumps'.
        let o = frame_stats_json(&st, 7, 3);
        assert_eq!(o["frames"], Json::from(2));
        assert_eq!(o["framesTotal"], Json::from(2));
        assert_eq!(o["pumps"], Json::from(7));
        assert_eq!(o["wokenPumps"], Json::from(3));
        assert_eq!(o["last"]["viewMs"], Json::from(3.0));
        assert_eq!(o["last"]["totalMs"], Json::from(3.0));
        assert_eq!(o["maxTotalMs"], Json::from(10.5));
        assert_eq!(o["maxWorkMs"], Json::from(6.5));
        assert_eq!(o["avgTotalMs"], Json::from(6.75));
        // F62: the ring saturates at 120 and `frames` with it; the total
        // is what a test counting frames reads.
        for _ in 0..200 {
            st.push(FrameSample::default());
        }
        let o = frame_stats_json(&st, 7, 3);
        assert_eq!(o["frames"], Json::from(120));
        assert_eq!(o["framesTotal"], Json::from(202));
    }
}

/// The key set of every readback shape, pinned (backlog AR1). Node's key
/// names are public API — `index.d.ts` declares them, test.mjs and the
/// corpus adapter read them — so the serialisers may move (into the core,
/// as `to_value`, with one snake→camel pass here) only under a test that
/// says the keys did not. Each shape is built with every optional part
/// present, so the nested keys are in the set too; a key path is
/// `a.b[].c`, arrays walked for the union of their elements.
#[cfg(test)]
mod readback_pins {
    use super::*;
    use kui_core::{
        AccessAction, AccessRequest, EditOptions, NodeSpec, Role, TextPos, TextStyle,
        WindowCommand, WindowConfig, WindowId, WindowKind, tree::OriginId,
    };
    use std::collections::BTreeSet;

    fn key_paths(v: &Json, prefix: &str, out: &mut BTreeSet<String>) {
        match v {
            Json::Object(m) => {
                for (k, v) in m {
                    let path = if prefix.is_empty() {
                        k.clone()
                    } else {
                        format!("{prefix}.{k}")
                    };
                    out.insert(path.clone());
                    key_paths(v, &path, out);
                }
            }
            Json::Array(a) => {
                let path = format!("{prefix}[]");
                for v in a {
                    key_paths(v, &path, out);
                }
            }
            _ => {}
        }
    }

    #[track_caller]
    fn assert_keys(shape: &str, v: &Json, expected: &[&str]) {
        let mut got = BTreeSet::new();
        key_paths(v, "", &mut got);
        let want: BTreeSet<String> = expected.iter().map(|s| s.to_string()).collect();
        assert_eq!(
            got, want,
            "{shape}: the key set moved — index.d.ts, test.mjs and the corpus adapter read these names"
        );
    }

    /// A frame with one of everything the readbacks describe: a scroller
    /// over a focused editor with a selection, a slider, a checkbox with a
    /// hover payload. `set_inspect` on, so `nodes()` fills.
    fn rich_core() -> (Core, Key) {
        let mut core = Core::new();
        core.set_inspect(true);
        let draw = |core: &mut Core| {
            let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
            ui.configure_root(NodeSpec::column().fill());
            let mut doc = Key::ROOT;
            ui.with_keyed(
                "scroller",
                NodeSpec::column().scroll_y().size(300.0, 60.0).pad(4.0),
                |ui| {
                    doc = ui.text_edit(
                        "doc",
                        "hello world\nsecond line\nthird\nfourth\nfifth",
                        &EditOptions {
                            multiline: true,
                            autofocus: true,
                            ..Default::default()
                        },
                        NodeSpec::column().width(280.0).label("Doc"),
                    );
                },
            );
            ui.leaf_keyed(
                "vol",
                NodeSpec::row()
                    .role(Role::Slider)
                    .label("Volume")
                    .value_now(0.4)
                    .value_min(0.0)
                    .value_max(1.0)
                    .on_drag("vol"),
            );
            ui.text_in_keyed(
                "remember",
                NodeSpec::row()
                    .role(Role::Checkbox)
                    .checked(true)
                    .on_click("toggle")
                    .on_hover("hov"),
                "Remember me",
                TextStyle::new(12.0),
            );
            ui.finish();
            doc
        };
        let doc = draw(&mut core);
        // A selection in the editor, so `anchor`/`focus` and `selection`
        // are objects and not nulls.
        let run = core.access_tree().get(doc).unwrap().runs[0].key;
        core.handle_input(InputEvent::Access(
            AccessRequest::new(doc, AccessAction::SetTextSelection)
                .with_selection(TextPos { run, character: 0 }, TextPos { run, character: 5 }),
        ));
        draw(&mut core);
        (core, doc)
    }

    #[test]
    fn scroll_geometry_keys() {
        let (core, _) = rich_core();
        let g = core.scroll_geometry(Key::ROOT.str("scroller"));
        assert!(g.is_some());
        assert_keys(
            "ScrollGeometry",
            &geometry_json(g).unwrap(),
            &[
                "x",
                "y",
                "w",
                "h",
                "contentW",
                "contentH",
                "offset",
                "offset.x",
                "offset.y",
                "maxOffset",
                "maxOffset.x",
                "maxOffset.y",
            ],
        );
    }

    #[test]
    fn text_hit_caret_rect_and_metrics_keys() {
        let (core, _) = rich_core();
        // The label inside the checkbox row: a text query answers to the
        // enclosing key (C18).
        let label = Key::ROOT.str("remember");
        let r = core.nodes().iter().find(|n| n.key == label).unwrap().rect;
        let hit = core
            .text_hit(label, Vec2::new(r.x + 4.0, r.y + r.h / 2.0))
            .expect("a hit");
        assert_keys("TextHit", &text_hit_json(hit), &["byte", "line"]);
        let rect = core.caret_rect(label, 3).expect("a caret");
        assert_keys("Rect", &rect_json(rect), &["x", "y", "w", "h"]);
        let m = kui_core::TextMetrics {
            width: 1.0,
            height: 2.0,
            lines: 1,
        };
        assert_keys(
            "TextMetrics",
            &text_metrics_json(m),
            &["width", "height", "lines"],
        );
        assert_keys(
            "WindowSize",
            &size_json(Size::new(1.0, 2.0), 2.0),
            &["width", "height", "scale"],
        );
    }

    #[test]
    fn node_info_keys() {
        let (core, _) = rich_core();
        let all = Json::Array(core.nodes().iter().map(node_info_json).collect());
        assert!(core.nodes().iter().any(|n| n.scroll.is_some()));
        assert!(core.nodes().iter().any(|n| !n.events.is_empty()));
        assert_keys(
            "NodeInfo",
            &all,
            &[
                "[].key",
                "[].parent",
                "[].depth",
                "[].kind",
                "[].label",
                "[].rect",
                "[].rect.x",
                "[].rect.y",
                "[].rect.w",
                "[].rect.h",
                "[].dir",
                "[].width",
                "[].height",
                "[].bg",
                "[].float",
                "[].role",
                "[].text",
                "[].flags",
                "[].layer",
                "[].origin",
                "[].children",
                "[].padding",
                "[].padding.t",
                "[].padding.r",
                "[].padding.b",
                "[].padding.l",
                "[].gap",
                "[].mainAlign",
                "[].crossAlign",
                "[].wrap",
                "[].table",
                "[].minWidth",
                "[].minHeight",
                "[].maxWidth",
                "[].maxHeight",
                "[].radius",
                "[].borderWidth",
                "[].borderColor",
                "[].opacity",
                "[].backdropBlur",
                "[].rotate",
                "[].scale",
                "[].scroll",
                "[].scroll.x",
                "[].scroll.y",
                "[].events",
                "[].events.click",
                "[].events.drag",
                "[].events.hover",
            ],
        );
        // The sizing spelling is part of the shape too.
        let root = &core.nodes()[0];
        assert_eq!(node_info_json(root)["width"], Json::from("grow(1)"));
        let spellings: Vec<String> = core
            .nodes()
            .iter()
            .map(|n| node_info_json(n)["width"].as_str().unwrap().to_string())
            .collect();
        assert!(spellings.contains(&"300px".to_string()), "{spellings:?}");
        assert!(spellings.contains(&"fit".to_string()), "{spellings:?}");
    }

    #[test]
    fn access_tree_keys() {
        let (mut core, doc) = rich_core();
        let tree = core.access_tree().clone();
        let node = tree.get(doc).unwrap();
        assert!(node.anchor.is_some() && node.selection.is_some() && !node.runs.is_empty());
        assert!(tree.nodes.iter().any(|n| n.scroll.is_some()));
        assert_keys(
            "AccessTree",
            &access_tree_json(&tree),
            &[
                "nodes",
                "focus",
                "hash",
                "nodes[].key",
                "nodes[].parent",
                "nodes[].origin",
                "nodes[].role",
                "nodes[].name",
                "nodes[].description",
                "nodes[].rect",
                "nodes[].rect.x",
                "nodes[].rect.y",
                "nodes[].rect.w",
                "nodes[].rect.h",
                "nodes[].value",
                "nodes[].caret",
                "nodes[].selection",
                "nodes[].anchor",
                "nodes[].anchor.run",
                "nodes[].anchor.character",
                "nodes[].focus",
                "nodes[].focus.run",
                "nodes[].focus.character",
                "nodes[].runs",
                "nodes[].runs[].key",
                "nodes[].runs[].line",
                "nodes[].runs[].start",
                "nodes[].runs[].end",
                "nodes[].runs[].text",
                "nodes[].runs[].rect",
                "nodes[].runs[].rect.x",
                "nodes[].runs[].rect.y",
                "nodes[].runs[].rect.w",
                "nodes[].runs[].rect.h",
                "nodes[].runs[].charLengths",
                "nodes[].runs[].charPositions",
                "nodes[].runs[].charWidths",
                "nodes[].runs[].wordStarts",
                "nodes[].runs[].rtl",
                "nodes[].checked",
                "nodes[].mixed",
                "nodes[].selected",
                "nodes[].expanded",
                "nodes[].posInSet",
                "nodes[].setSize",
                "nodes[].orientation",
                "nodes[].live",
                "nodes[].valueNow",
                "nodes[].valueMin",
                "nodes[].valueMax",
                "nodes[].valueStep",
                "nodes[].focused",
                "nodes[].disabled",
                "nodes[].modal",
                "nodes[].scroll",
                "nodes[].scroll.x",
                "nodes[].scroll.y",
                "nodes[].scroll.maxX",
                "nodes[].scroll.maxY",
                "nodes[].actions",
            ],
        );
    }

    #[test]
    fn window_command_keys() {
        let cmds = vec![
            WindowCommand::StartDrag(WindowId::MAIN),
            WindowCommand::Close(WindowId::MAIN),
            WindowCommand::Minimize(WindowId::MAIN),
            WindowCommand::ToggleMaximize(WindowId::MAIN),
            WindowCommand::Open {
                id: WindowId(1),
                owner: WindowId::MAIN,
                origin: OriginId::HOST,
                config: WindowConfig::of_kind(WindowKind::Popup),
            },
            WindowCommand::SetSize {
                window: WindowId::MAIN,
                size: Size::new(1.0, 2.0),
            },
            WindowCommand::Focus(WindowId::MAIN),
            WindowCommand::Redraw(WindowId::MAIN),
        ];
        assert_keys(
            "WindowCommand",
            &window_commands_json(cmds),
            &[
                "[].kind",
                "[].window",
                "[].width",
                "[].height",
                "[].owner",
                "[].origin",
                "[].config",
                "[].config.kind",
                "[].config.width",
                "[].config.height",
                "[].config.activates",
                "[].config.anchor",
                "[].config.anchor.x",
                "[].config.anchor.y",
                "[].config.anchor.w",
                "[].config.anchor.h",
            ],
        );
    }

    #[test]
    fn warning_keys() {
        let w = kui_core::Warning {
            code: kui_core::diag::DUPLICATE_KEY,
            key: Key::ROOT,
            message: "m".into(),
        };
        assert_keys(
            "Warning",
            &warnings_json(vec![w]),
            &["[].code", "[].key", "[].message"],
        );
    }

    #[test]
    fn audio_command_keys() {
        let pb = PlaybackId(1);
        let sound = SoundId::from_ffi(7);
        let cmds = vec![
            AudioCommand::Play {
                playback: pb,
                sound,
                volume: 1.0,
                looped: false,
                fade_in_ms: 0.0,
            },
            AudioCommand::Stop {
                playback: pb,
                fade_ms: 0.0,
            },
            AudioCommand::SetVolume {
                playback: pb,
                volume: 1.0,
                tween_ms: 0.0,
            },
            AudioCommand::Pause {
                playback: pb,
                fade_ms: 0.0,
            },
            AudioCommand::Resume {
                playback: pb,
                fade_ms: 0.0,
            },
            AudioCommand::MasterVolume {
                volume: 1.0,
                tween_ms: 0.0,
            },
            AudioCommand::Unload { sound },
        ];
        assert_keys(
            "AudioCommand",
            &audio_commands_json(cmds),
            &[
                "[].kind",
                "[].playback",
                "[].sound",
                "[].volume",
                "[].loop",
                "[].fadeIn",
                "[].fade",
                "[].tween",
            ],
        );
    }
}

#[cfg(test)]
mod readback_payloads {
    use super::*;

    /// A `nodes()` row's `events` carry the app's payloads as the app
    /// spelled them: the camel pass that names the row's own keys stops
    /// at the handler names.
    #[test]
    fn an_apps_payload_keys_are_not_camelised() {
        let mut core = Core::new();
        core.set_inspect(true);
        let mut ui = core.frame(Size::new(100.0, 100.0), 1.0);
        ui.configure_root(kui_core::NodeSpec::column().fill());
        ui.leaf_keyed(
            "b",
            kui_core::NodeSpec::row().on_click(Value::map([("by_amount", Value::Int(2))])),
        );
        ui.finish();
        let nodes = core.nodes();
        let row = nodes
            .iter()
            .find(|n| n.label.as_deref() == Some("b"))
            .unwrap();
        let o = node_info_json(row);
        assert_eq!(o["events"]["click"]["by_amount"], Json::from(2));
        assert_eq!(
            o["mainAlign"],
            Json::from("start"),
            "the row's own keys still camelise"
        );
    }
}
