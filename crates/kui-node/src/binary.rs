//! The flat binary IR path: JS encodes the frame as one instruction stream
//! (a Float64Array — opcodes, prop ids, and values are all f64) plus a UTF-8
//! string table (a Uint8Array), and this module lowers it straight into the
//! core. One boundary crossing, zero per-node napi calls, strings borrowed
//! from the table — no `serde_json::Value` tree ever exists.
//!
//! The prop surface is defined once in `schema.rs`; this decoder reads
//! generic props by their schema kind and applies them through the shared
//! table, so a prop cannot mean one thing here and another in the core. Only
//! composite props (pad/border/overflow/float) and the constructor specials
//! (dir/size/key/title/keyFocus) have hand-written arms here, mirrored in
//! the encoder. This is the addon's only element dispatcher.
//!
//! JS never hardcodes ids: `protocol()` exports the tables, and the encoder
//! reads them at module init. `VERSION` is stamped into slot 0 of every
//! stream as a second guard (against a stale prebuilt addon paired with a
//! newer encoder).
//!
//! Encoder-guaranteed invariants the decoder relies on:
//! - `P_DIR` and `P_SIZE`, when present, are the first entry of a prop list
//!   (specs and styles are constructed from them, then mutated).
//! - String refs are `(offset, len)` pairs into the table; offset -1 = none.
//! - Message payloads (`onClick` etc.) and keyframe lists are JSON strings
//!   in the table.

use kui_core::{
    Align, Content, EditOptions, FloatConfig, ImageId, NodeSpec, PadShorthand, Rect, Size, Span,
    TextStyle, WindowConfig, WindowKind, widgets,
};
use serde_json::{Map as JsonMap, Value as Json};

use crate::schema::{
    self, Kind, P_BORDER, P_DIR, P_FLOAT, P_INDEX, P_KEY, P_KEY_FOCUS, P_OVERFLOW, P_PAD, P_SIZE,
    P_TITLE, P_TOOLTIP, P_WINDOWS, Parsed, PropsOut, align_idx, color_num, min_num, sizing_num,
};
use crate::{Result, err, value_of};

/// Bumped when the wire format changes (v2: enum props became list indices;
/// v3: float configs carry a has-offset flag so bare presets keep their gap;
/// v4: the pad shorthand rides the wire unresolved and each float offset
/// carries its own flag, so the encoder decides neither what `padX` falls
/// back to nor whether a lone `dx` flattens the preset's `dy`; v5: `slot`,
/// which is a new op rather than a new prop and so an encoder that has
/// never heard of it writes a stream this addon still reads - the bump is
/// for the other direction, an encoder that emits one to an addon without
/// the op).
/// v6: `<cells originLine>` — the absolute line a grid's row 0 is, so a
/// terminal's selection survives a scroll (ADR 0017, decision 4). A slot
/// in the middle of the cells op rather than a new op, so the bump is what
/// keeps an older encoder's stream from being read as if it had one.
pub const VERSION: u32 = 6;

pub const OP_END: u32 = 0;
pub const OP_ROOT: u32 = 1;
pub const OP_OPEN: u32 = 2;
pub const OP_CLOSE: u32 = 3;
pub const OP_TEXT: u32 = 4;
pub const OP_RICH_TEXT: u32 = 5;
pub const OP_BUTTON: u32 = 6;
pub const OP_EDIT: u32 = 7;
pub const OP_IMAGE: u32 = 8;
pub const OP_TITLEBAR: u32 = 9;
pub const OP_WINDOW_BUTTONS: u32 = 10;
pub const OP_LATENCY_GRAPH: u32 = 11;
pub const OP_LATENCY_HUD: u32 = 12;
pub const OP_AUDIO: u32 = 13;
pub const OP_LINE: u32 = 14;
pub const OP_CELLS: u32 = 15;
pub const OP_FRAGMENT: u32 = 16;
pub const OP_SLOT: u32 = 17;
pub const OP_MENU_BAR: u32 = 18;

pub fn protocol_json() -> Json {
    let mut o = JsonMap::new();
    o.insert("version".into(), Json::from(VERSION));
    o.insert(
        "op".into(),
        Json::Object(
            [
                ("end", OP_END),
                ("root", OP_ROOT),
                ("open", OP_OPEN),
                ("close", OP_CLOSE),
                ("text", OP_TEXT),
                ("richText", OP_RICH_TEXT),
                ("button", OP_BUTTON),
                ("edit", OP_EDIT),
                ("image", OP_IMAGE),
                ("titlebar", OP_TITLEBAR),
                ("windowButtons", OP_WINDOW_BUTTONS),
                ("latencyGraph", OP_LATENCY_GRAPH),
                ("latencyHud", OP_LATENCY_HUD),
                ("audio", OP_AUDIO),
                ("line", OP_LINE),
                ("cells", OP_CELLS),
                ("fragment", OP_FRAGMENT),
                ("slot", OP_SLOT),
                ("menuBar", OP_MENU_BAR),
            ]
            .into_iter()
            .map(|(k, v)| (k.to_string(), Json::from(v)))
            .collect::<JsonMap<_, _>>(),
        ),
    );
    o.insert("prop".into(), schema::protocol_props());
    // Value tables the encoder would otherwise restate: align indices and
    // the float preset names, both in the order the decoder reads them.
    o.insert(
        "align".into(),
        Json::Array(
            schema::ALIGNS
                .iter()
                .map(|a| Json::String((*a).into()))
                .collect(),
        ),
    );
    o.insert(
        "floatPreset".into(),
        Json::Array(
            kui_core::FLOAT_PRESETS
                .iter()
                .map(|p| Json::String((*p).into()))
                .collect(),
        ),
    );
    for (name, table) in schema::protocol_tables() {
        o.insert(name.into(), table);
    }
    Json::Object(o)
}

struct Reader<'a> {
    s: &'a [f64],
    i: usize,
    strings: &'a [u8],
}

impl<'a> Reader<'a> {
    fn f(&mut self) -> Result<f64> {
        let v = *self
            .s
            .get(self.i)
            .ok_or_else(|| err("binary frame truncated"))?;
        self.i += 1;
        Ok(v)
    }

    fn u(&mut self) -> Result<u32> {
        Ok(self.f()? as u32)
    }

    /// A `(offset, len)` string ref; offset -1 = absent.
    fn str_ref(&mut self) -> Result<Option<&'a str>> {
        let off = self.f()?;
        let len = self.f()? as usize;
        if off < 0.0 {
            return Ok(None);
        }
        let off = off as usize;
        let bytes = self
            .strings
            .get(off..off + len)
            .ok_or_else(|| err("string ref out of bounds"))?;
        std::str::from_utf8(bytes)
            .map(Some)
            .map_err(|_| err("string table is not UTF-8"))
    }

    fn req_str(&mut self) -> Result<&'a str> {
        self.str_ref()?
            .ok_or_else(|| err("missing required string"))
    }
}

/// A float attach point on the wire: a "declared" flag then its two align
/// indices, always all three so the stream stays fixed-width.
fn opt_attach(r: &mut Reader<'_>) -> Result<Option<(Align, Align)>> {
    let declared = r.u()? == 1;
    let (x, y) = (r.u()? as usize, r.u()? as usize);
    Ok(declared.then(|| (align_idx(x), align_idx(y))))
}

/// An optional float offset component: a "declared" flag then its value,
/// always both so the stream stays fixed-width.
fn opt_f32(r: &mut Reader<'_>) -> Result<Option<f32>> {
    let declared = r.u()? == 1;
    let v = r.f()? as f32;
    Ok(declared.then_some(v))
}

fn payload(s: &str) -> Result<kui_core::Value> {
    let json: Json = serde_json::from_str(s).map_err(|e| err(format!("bad payload JSON: {e}")))?;
    Ok(value_of(&json))
}

/// The binary prop parser —
/// composites hand-written, everything else read by schema kind and applied
/// through the shared table.
fn read_props(r: &mut Reader<'_>) -> Result<PropsOut> {
    read_props_over(r, PropsOut::new())
}

/// [`read_props`] applied over `out` rather than the schema defaults: what
/// a composite that keeps its own look reads its admitted rows into (the
/// stock button over `widgets::button_spec`).
fn read_props_over(r: &mut Reader<'_>, mut out: PropsOut) -> Result<PropsOut> {
    let n = r.u()?;
    for _ in 0..n {
        let id = r.u()?;
        match id {
            P_DIR => {
                if r.u()? == 1 {
                    out.spec = NodeSpec::row();
                }
            }
            P_SIZE => out.style = TextStyle::new(r.f()? as f32),
            P_PAD => {
                // The shorthand rides the wire as declared (a set mask plus
                // the seven values), so the encoder never has to know what
                // `padX` falls back to either.
                let set = r.u()?;
                let mut pad = PadShorthand::default();
                for (bit, slot) in [
                    &mut pad.all,
                    &mut pad.x,
                    &mut pad.y,
                    &mut pad.l,
                    &mut pad.r,
                    &mut pad.t,
                    &mut pad.b,
                ]
                .into_iter()
                .enumerate()
                {
                    let v = r.f()? as f32;
                    if set & (1 << bit) != 0 {
                        *slot = Some(v);
                    }
                }
                out.apply_pad(pad);
            }
            P_BORDER => {
                let (w, c) = (r.f()?, r.f()?);
                out.spec = std::mem::take(&mut out.spec).border(w as f32, color_num(c as u32));
            }
            P_OVERFLOW => {
                let bits = r.u()?;
                out.with_spec(|s| s.overflow_bits(bits));
            }
            P_FLOAT => {
                let preset = r.u()? as usize;
                let base = FloatConfig::preset_at(preset)
                    .ok_or_else(|| err(format!("unknown float preset {preset}")))?;
                // Each override rides with a "was it declared" flag, so a
                // bare `below` keeps its built-in gap while a config object
                // that names only `dx` moves it sideways.
                let at = opt_attach(r)?;
                let self_at = opt_attach(r)?;
                // dx and dy carry their own flags: `{ anchor: "below", dx }`
                // moves it sideways and leaves the preset's gap.
                let (dx, dy) = (opt_f32(r)?, opt_f32(r)?);
                let fit = r.u()? == 1;
                let cfg = FloatConfig::build(base, at, self_at, dx, dy, fit);
                out.spec = std::mem::take(&mut out.spec).float(cfg);
            }
            P_KEY_FOCUS => out.key_focus = true,
            P_KEY => out.key = Some(r.req_str()?.to_string()),
            // A row number, so anything that is not one is refused rather
            // than folded to 0 — where it would take row 0's key, and two
            // of them in one frame would share it silently.
            P_INDEX => {
                let i = r.f()?;
                // NaN fails `is_finite`, so the comparisons below are on
                // numbers and read as written.
                if !i.is_finite() || i < 0.0 || i.fract() != 0.0 {
                    return Err(err(format!(
                        "index must be a whole row number, not {i}: it is the data index the row \
                         is keyed by"
                    )));
                }
                out.index = Some(i as u64);
            }
            P_TITLE => out.title = Some(r.req_str()?.to_string()),
            // A count, then per window: name, kind, width, height (zero =
            // the default size), activates, and the anchor rect a popup is
            // placed against (four zeros for a normal window).
            P_WINDOWS => {
                let n = r.u()?;
                for _ in 0..n {
                    let name = r.req_str()?.to_string();
                    let kind = match r.u()? {
                        0 => WindowKind::Normal,
                        1 => WindowKind::Popup,
                        k => return Err(err(format!("unknown window kind {k}"))),
                    };
                    let (w, h) = (r.f()? as f32, r.f()? as f32);
                    let activates = r.u()? == 1;
                    let anchor =
                        Rect::new(r.f()? as f32, r.f()? as f32, r.f()? as f32, r.f()? as f32);
                    let mut cfg = WindowConfig {
                        kind,
                        activates,
                        anchor,
                        ..WindowConfig::default()
                    };
                    if w > 0.0 && h > 0.0 {
                        cfg.size = Size::new(w, h);
                    }
                    out.windows.push((name, cfg));
                }
            }
            P_TOOLTIP => out.apply_tooltip(r.req_str()?),
            id => {
                let def = schema::by_id(id).ok_or_else(|| err(format!("unknown prop id {id}")))?;
                let parsed = match &def.kind {
                    Kind::F32 => Parsed::F32(r.f()? as f32),
                    Kind::Color => Parsed::Color(color_num(r.f()? as u32)),
                    Kind::Flag => Parsed::Flag,
                    Kind::Enum(names) => {
                        let i = r.u()? as usize;
                        if i >= names.len() {
                            return Err(err(format!("bad enum index {i} for {}", def.name)));
                        }
                        Parsed::Enum(i)
                    }
                    Kind::Sizing => {
                        let (m, v) = (r.u()?, r.f()?);
                        Parsed::Sizing(sizing_num(m, v))
                    }
                    Kind::Min => {
                        let (m, v) = (r.u()?, r.f()?);
                        Parsed::Min(min_num(m, v))
                    }
                    Kind::Msg | Kind::Tag => Parsed::Msg(payload(r.req_str()?)?),
                    Kind::Str => Parsed::Str(r.req_str()?.to_string()),
                    Kind::Resource => Parsed::Resource(crate::parse_u64(r.req_str()?)?),
                    // Carried as JSON like a message; the core reads the stops.
                    Kind::Keyframes => Parsed::Keyframes(
                        kui_core::keyframes::parse(&payload(r.req_str()?)?).map_err(err)?,
                    ),
                    Kind::Enter => {
                        Parsed::Enter(kui_core::enter::parse(&payload(r.req_str()?)?).map_err(err)?)
                    }
                };
                schema::apply(def, parsed, &mut out)?;
            }
        }
    }
    Ok(out)
}

/// The span list of a rich text: a count, then per span its text and the
/// flags the encoder's `collectSpans` packed (1 bold, 2 italic, 4 has
/// colour, 8 underline, 16 strikethrough, 32 has bg) with the two colours.
fn read_spans<'a>(r: &mut Reader<'a>) -> Result<Vec<Span<'a>>> {
    let nspans = r.u()? as usize;
    let mut spans = Vec::with_capacity(nspans);
    for _ in 0..nspans {
        let text = r.req_str()?;
        let flags = r.u()?;
        let color = r.f()?;
        let bg = r.f()?;
        let mut s = Span::new(text);
        if flags & 1 != 0 {
            s = s.bold();
        }
        if flags & 2 != 0 {
            s = s.italic();
        }
        if flags & 4 != 0 {
            s = s.color(color_num(color as u32));
        }
        if flags & 8 != 0 {
            s = s.underline();
        }
        if flags & 16 != 0 {
            s = s.strikethrough();
        }
        if flags & 32 != 0 {
            s = s.bg(color_num(bg as u32));
        }
        spans.push(s);
    }
    Ok(spans)
}

/// `measureText`'s door: `stream` is one text element as the encoder's
/// `encodeText` writes it — the version, then `OP_TEXT` or `OP_RICH_TEXT`
/// with its content and style props — and the answer is what the same
/// text would lay out to. One reader for the frame and the query, so a
/// measured label and a drawn one are shaped from the same runs.
pub fn measure_binary(
    core: &mut kui_core::Core,
    stream: &[f64],
    strings: &[u8],
    max_width: Option<f64>,
) -> Result<kui_core::TextMetrics> {
    let mut r = Reader {
        s: stream,
        i: 0,
        strings,
    };
    let version = r.u()?;
    if version != VERSION {
        return Err(err(format!(
            "binary text version {version} != addon version {VERSION} — encoder and addon are out of sync"
        )));
    }
    let max_w = max_width
        .filter(|w| w.is_finite() && *w > 0.0)
        .map(|w| w as f32);
    match r.u()? {
        OP_TEXT => {
            let content = r.req_str()?;
            let p = read_props(&mut r)?;
            Ok(core.measure_text(content, &p.style, max_w))
        }
        OP_RICH_TEXT => {
            let p = read_props(&mut r)?;
            let spans = read_spans(&mut r)?;
            Ok(core.measure_rich_text(&spans, &p.style, max_w))
        }
        op => Err(err(format!(
            "measureText expects one text element, got op {op}"
        ))),
    }
}

fn decode_op(op: u32, r: &mut Reader<'_>, ui: &mut kui_core::Ui<'_>) -> Result<()> {
    match op {
        OP_OPEN => {
            let p = read_props(r)?;
            ui.core().open_from(p, Content::Box);
            decode_until_close(r, ui)?;
            ui.core().close();
            Ok(())
        }
        OP_TEXT => {
            let content = r.req_str()?;
            let p = read_props(r)?;
            ui.core().text_node(content, p.style);
            Ok(())
        }
        OP_RICH_TEXT => {
            let p = read_props(r)?;
            let spans = read_spans(r)?;
            ui.core().rich_text_node(&spans, p.style);
            Ok(())
        }
        OP_BUTTON => {
            // Text, key?, click payload?, then a prop list holding only the
            // rows the stock button admits (`schema::BUTTON_ROWS_JSX`; the
            // encoder writes no other), read over `widgets::button_spec` so
            // the look stays the widget's — a `dir` in that list would
            // rebuild the spec from nothing, which is why the list is
            // closed at the encoder rather than merged here.
            let label = r.req_str()?;
            let key = r.str_ref()?;
            let msg = match r.str_ref()? {
                Some(s) => payload(s)?,
                None => kui_core::Value::Null,
            };
            let label_key = key.unwrap_or(label);
            let mut base = PropsOut::new();
            base.spec = widgets::button_spec();
            let p = read_props_over(r, base)?;
            widgets::button_with(
                ui,
                label_key,
                label,
                p.spec.on_click(msg),
                p.tooltip.as_deref(),
            );
            Ok(())
        }
        OP_EDIT => {
            let label = r.req_str()?;
            let initial = r.str_ref()?.unwrap_or("");
            let flags = r.u()?;
            let p = read_props(r)?;
            let opts = EditOptions {
                style: p.style,
                multiline: flags & 1 != 0,
                autofocus: flags & 2 != 0,
                ..Default::default()
            };
            ui.core().text_edit(label, initial, &opts, p.spec);
            Ok(())
        }
        OP_IMAGE => {
            let (hi, lo) = (r.f()? as u64, r.f()? as u64);
            let p = read_props(r)?;
            ui.core()
                .image_node(ImageId::from_ffi((hi << 32) | lo), p.spec);
            Ok(())
        }
        // key?, src (hi, lo), flags (1 loop | 2 paused | 4 finish), volume
        // (-1 = absent), tag JSON? — a retained playback keyed by node.
        OP_AUDIO => {
            let key = r.str_ref()?;
            let (hi, lo) = (r.f()? as u64, r.f()? as u64);
            let flags = r.u()?;
            let volume = r.f()?;
            let tag = match r.str_ref()? {
                Some(s) => Some(payload(s)?),
                None => None,
            };
            let spec = crate::audio_spec_of(
                kui_core::SoundId::from_ffi((hi << 32) | lo),
                (volume >= 0.0).then_some(volume),
                flags & 1 != 0,
                flags & 2 != 0,
                flags & 4 != 0,
                tag,
            );
            match key {
                Some(label) => ui.core().audio_node_keyed(label, spec),
                None => ui.core().audio_node(spec),
            };
            Ok(())
        }
        // `<slot name params>`: a position among the current node's
        // children that an extension fills, in place and now
        // (`docs/adr/0014-slots-an-extension-fills-in-place.md`). `name` is
        // the full `namespace/slot` — the namespace the host loaded the
        // extension under (`addExtension`), and the slot in the extension's
        // own vocabulary. Through `Ui::slot_with` rather than
        // `Core::begin_slot`, because the `Ui` is what carries the filler:
        // the runner's list for a window, the context's for a headless one.
        // With nothing loaded it still places the node, so a view can
        // declare its layout before it has a plugin to put in it.
        OP_SLOT => {
            let name = r.req_str()?;
            let params = match r.str_ref()? {
                Some(s) => payload(s)?,
                None => kui_core::Value::Null,
            };
            ui.slot_with(name, &params);
            Ok(())
        }
        // n, then n (x, y) pairs, width, flags (1 curve), then the prop
        // list — `color` lands in the style, `key` in `p.key`, and the
        // core decides the box (docs/adr/0010-a-segment-primitive.md).
        OP_LINE => {
            let n = r.u()? as usize;
            let mut points = Vec::with_capacity(n);
            for _ in 0..n {
                let x = r.f()? as f32;
                let y = r.f()? as f32;
                points.push(kui_core::Vec2::new(x, y));
            }
            let width = r.f()? as f32;
            let flags = r.u()?;
            let p = read_props(r)?;
            // No `color` is the theme's foreground, as for a text run.
            let stroke_color = p.style.color.unwrap_or(ui.theme().fg);
            let mut stroke = kui_core::Stroke::new(width, stroke_color);
            stroke.curve = flags & 1 != 0;
            ui.core().open_from(p, Content::Line(&points, stroke));
            Ok(())
        }
        // src (hi, lo), param count, the params, then props. An open node:
        // the encoder emits OP_CLOSE for it like any other parent, so a
        // fragment's children paint over it.
        OP_FRAGMENT => {
            let (hi, lo) = (r.f()? as u64, r.f()? as u64);
            let n = r.u()? as usize;
            let mut params = Vec::with_capacity(n);
            for _ in 0..n {
                params.push(r.f()? as f32);
            }
            let p = read_props(r)?;
            let id = kui_core::FragmentId::from_ffi((hi << 32) | lo);
            ui.core().open_from(p, Content::Fragment(id, &params));
            decode_until_close(r, ui)?;
            ui.core().close();
            Ok(())
        }
        OP_CELLS => {
            // rows, cols, cursor (present, row, col, shape, colour), then
            // three slots a cell: codepoint | flags << 21, fg, bg.
            let rows = r.u()? as usize;
            let cols = r.u()? as usize;
            let has_cursor = r.u()? == 1;
            let crow = r.u()? as usize;
            let ccol = r.u()? as usize;
            let cshape = r.u()? as usize;
            let ccolor = r.f()? as u32;
            let origin_line = r.f()?.max(0.0) as u64;
            let n = r.u()? as usize;
            if n != rows * cols {
                return Err(err(format!("<cells> carries {n} cells for {rows}×{cols}")));
            }
            let mut cells = Vec::with_capacity(n);
            for _ in 0..n {
                let packed = r.f()? as u32;
                let fg = r.f()? as u32;
                let bg = r.f()? as u32;
                cells.push(kui_core::Cell {
                    ch: char::from_u32(packed & 0x1f_ffff).unwrap_or(' '),
                    fg,
                    bg,
                    flags: (packed >> 21) as u8,
                });
            }
            let p = read_props(r)?;
            let cursor = has_cursor.then(|| {
                (
                    crow,
                    ccol,
                    kui_core::CellCursor::from_index(cshape).unwrap_or(kui_core::CellCursor::Block),
                    color_num(ccolor),
                )
            });
            let grid = kui_core::CellGrid {
                rows,
                cols,
                cells: &cells,
                style: p.style,
                cursor,
                origin_line,
            };
            ui.core().open_from(p, Content::Cells(&grid));
            Ok(())
        }
        OP_TITLEBAR => {
            let title = r.str_ref()?.map(str::to_string);
            let has_children = r.u()? == 1;
            if has_children {
                // titlebar_with opens and closes its own container, so the
                // children loop must only consume up to the CLOSE op. The
                // widget is handed this `ui` rather than a fresh wrap of
                // its core, so the filler survives into the children and a
                // `<slot>` inside a titlebar fills like one anywhere else.
                let mut result = Ok(());
                widgets::titlebar_with(ui, |ui| {
                    result = decode_until_close(r, ui);
                });
                result
            } else {
                widgets::titlebar(ui, title.as_deref().unwrap_or(""));
                Ok(())
            }
        }
        OP_WINDOW_BUTTONS => {
            widgets::window_buttons(ui);
            Ok(())
        }
        OP_MENU_BAR => {
            // One JSON blob, read by the same parser `openMenu`'s items go
            // through: a menu nests, and two readers of one shape are two
            // things to keep in step.
            let bar = crate::menu_bar_of(r.req_str()?).map_err(|e| err(e.to_string()))?;
            widgets::menu_bar(ui, bar);
            Ok(())
        }
        OP_LATENCY_GRAPH => {
            widgets::latency_graph(ui);
            Ok(())
        }
        OP_LATENCY_HUD => {
            let (x, y) = (r.u()?, r.u()?);
            widgets::latency_hud_at(ui, align_idx(x as usize), align_idx(y as usize));
            Ok(())
        }
        other => Err(err(format!("unknown opcode {other}"))),
    }
}

/// Runs ops until the matching CLOSE, which it consumes but does not act on —
/// the caller owns closing (or not, for widgets that close themselves).
fn decode_until_close(r: &mut Reader<'_>, ui: &mut kui_core::Ui<'_>) -> Result<()> {
    loop {
        match r.u()? {
            OP_CLOSE => return Ok(()),
            op => decode_op(op, r, ui)?,
        }
    }
}

/// Lowers a whole encoded frame (the binary analogue of `lower_root`).
pub fn lower_binary(ui: &mut kui_core::Ui<'_>, stream: &[f64], strings: &[u8]) -> Result<()> {
    let mut r = Reader {
        s: stream,
        i: 0,
        strings,
    };
    let version = r.u()?;
    if version != VERSION {
        return Err(err(format!(
            "binary frame version {version} != addon version {VERSION} — encoder and addon are out of sync"
        )));
    }
    if r.u()? != OP_ROOT {
        return Err(err("binary frame must start with the root op"));
    }
    ui.core().configure_root_from(read_props(&mut r)?);
    loop {
        match r.u()? {
            OP_END => return Ok(()),
            op => decode_op(op, &mut r, ui)?,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kui_core::schema::{CUSTOM, PROPS};
    use kui_core::{Align, Color, Size, Sizing, Value};

    /// Decodes one prop list from a hand-built stream.
    fn decode(stream: &[f64], strings: &[u8]) -> PropsOut {
        let mut r = Reader {
            s: stream,
            i: 0,
            strings,
        };
        read_props(&mut r).unwrap()
    }

    /// Every generic schema row, written by its kind the way encoder.js
    /// does, decodes to the same `PropsOut` the schema apply produces. The
    /// stream layout per kind is the protocol; a row the decoder rejects or
    /// misreads fails here, and a new `Kind` variant fails to compile until
    /// `read_props` (and the encoder) learn its slots.
    #[test]
    fn every_schema_row_round_trips_through_the_stream() {
        for def in PROPS {
            let mut stream = vec![1.0, def.id as f64];
            let mut strings: &[u8] = b"7";
            let sample = match def.kind {
                Kind::F32 => {
                    stream.push(37.0);
                    Parsed::F32(37.0)
                }
                Kind::Color => {
                    stream.push(0x11223344u32 as f64);
                    Parsed::Color(color_num(0x11223344))
                }
                Kind::Flag => Parsed::Flag,
                Kind::Enum(_) => {
                    stream.push(1.0);
                    Parsed::Enum(1)
                }
                Kind::Sizing => {
                    stream.extend([3.0, 0.5]);
                    Parsed::Sizing(Sizing::Percent(0.5))
                }
                Kind::Min => {
                    stream.extend([1.0, 0.0]);
                    Parsed::Min(kui_core::Min::FIT)
                }
                Kind::Msg | Kind::Tag => {
                    stream.extend([0.0, 1.0]);
                    Parsed::Msg(Value::Int(7))
                }
                Kind::Str => {
                    stream.extend([0.0, 1.0]);
                    Parsed::Str("7".into())
                }
                Kind::Resource => {
                    stream.extend([0.0, 1.0]);
                    Parsed::Resource(7)
                }
                Kind::Keyframes => {
                    strings = br#"[{"at":0.5,"radius":7}]"#;
                    stream.extend([0.0, strings.len() as f64]);
                    Parsed::Keyframes(vec![kui_core::Keyframe::default().at(0.5).radius(7.0)])
                }
                Kind::Enter => {
                    strings = br#"{"dx":-7,"radius":7}"#;
                    stream.extend([0.0, strings.len() as f64]);
                    Parsed::Enter(kui_core::Enter::from(-7.0, 0.0).radius(7.0))
                }
            };
            let mut expected = PropsOut::new();
            schema::apply(def, sample, &mut expected).unwrap();
            assert_eq!(
                decode(&stream, strings),
                expected,
                "{}: binary decode disagrees with the schema",
                def.name
            );
        }
    }

    /// Every `CUSTOM` row has a hand-written decoder arm: each is written
    /// the way encoder.js does and checked against the Rust builder. A new
    /// custom prop without a mapping here panics with instructions.
    #[test]
    fn every_custom_prop_has_a_decoder_arm() {
        for def in CUSTOM {
            let (name, id) = (def.name, def.id);
            let mut s = vec![1.0, id as f64];
            let mut strings: &[u8] = b"";
            let mut expected = PropsOut::new();
            match name {
                "dir" => {
                    s.push(1.0);
                    expected.spec = NodeSpec::row();
                }
                "size" => {
                    s.push(21.0);
                    expected.style = TextStyle::new(21.0);
                }
                "pad" => {
                    // padL/R/T/B only: set mask 0b1111000, then the seven
                    // values in declaration order (pad, X, Y, L, R, T, B).
                    s.extend([0b111_1000 as f64, 0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 4.0]);
                    expected.apply_pad(PadShorthand {
                        l: Some(1.0),
                        r: Some(2.0),
                        t: Some(3.0),
                        b: Some(4.0),
                        ..PadShorthand::default()
                    });
                }
                "border" => {
                    s.extend([2.0, 0x2a2d3affu32 as f64]);
                    expected.with_spec(|x| x.border(2.0, Color::hex(0x2a2d3aff)));
                }
                "overflow" => {
                    s.push(7.0);
                    expected.with_spec(|x| x.clip().scroll_x().scroll_y());
                }
                "float" => {
                    // viewport, at=(end,end), self=(end,end), dx=-8, dy=-8, fit
                    s.extend([1.0, 1.0, 2.0, 2.0, 1.0, 2.0, 2.0, 1.0, -8.0, 1.0, -8.0, 1.0]);
                    expected.with_spec(|x| {
                        x.float(
                            FloatConfig::viewport()
                                .at(Align::End, Align::End)
                                .self_at(Align::End, Align::End)
                                .offset(-8.0, -8.0)
                                .fit(),
                        )
                    });
                }
                "keyFocus" => expected.key_focus = true,
                "key" => {
                    s.extend([0.0, 3.0]);
                    strings = b"abc";
                    expected.key = Some("abc".into());
                }
                "index" => {
                    s.push(7.0);
                    expected.index = Some(7);
                }
                "title" => {
                    s.extend([0.0, 3.0]);
                    strings = b"abc";
                    expected.title = Some("abc".into());
                }
                "tooltip" => {
                    s.extend([0.0, 3.0]);
                    strings = b"abc";
                    expected.apply_tooltip("abc");
                }
                "windows" => {
                    // One window: "abc", a popup 400x300, non-activating,
                    // anchored to a 60x20 rect at (10, 20).
                    s.extend([
                        1.0, 0.0, 3.0, 1.0, 400.0, 300.0, 0.0, 10.0, 20.0, 60.0, 20.0,
                    ]);
                    strings = b"abc";
                    expected.windows.push((
                        "abc".into(),
                        WindowConfig {
                            kind: WindowKind::Popup,
                            size: Size::new(400.0, 300.0),
                            activates: false,
                            anchor: Rect::new(10.0, 20.0, 60.0, 20.0),
                        },
                    ));
                }
                other => panic!(
                    "custom prop {other:?} has no binary decoder arm: add one in read_props, \
                     mirror it in packages/kui/encoder.js, and map it here"
                ),
            }
            assert_eq!(
                decode(&s, strings),
                expected,
                "{name}: binary decode disagrees with the Rust builder"
            );
        }
    }

    /// A bare float preset on the wire keeps the preset's own gap (the bug
    /// the JS parity test first caught: v2 clobbered it with offset 0,0).
    #[test]
    fn bare_float_presets_keep_their_offset() {
        let s = [
            1.0,
            P_FLOAT as f64,
            2.0, // below
            0.0,
            0.0,
            0.0, // no at
            0.0,
            0.0,
            0.0, // no self
            0.0,
            0.0, // no dx
            0.0,
            0.0, // no dy
            0.0, // fit
        ];
        let mut expected = PropsOut::new();
        expected.with_spec(|x| x.float(FloatConfig::below()));
        assert_eq!(decode(&s, b""), expected);
    }

    /// A preset base with only `dx` declared keeps the preset's own `dy` —
    /// the disagreement the scene corpus caught between this path and the
    /// JSON one when the two offsets shared a flag.
    #[test]
    fn a_declared_dx_leaves_the_presets_dy_alone() {
        let s = [
            1.0,
            P_FLOAT as f64,
            2.0, // below
            0.0,
            0.0,
            0.0, // no at
            0.0,
            0.0,
            0.0, // no self
            1.0,
            6.0, // dx = 6
            0.0,
            0.0, // no dy
            0.0, // fit
        ];
        let mut expected = PropsOut::new();
        expected.with_spec(|x| {
            x.float(FloatConfig::build(
                FloatConfig::below(),
                None,
                None,
                Some(6.0),
                None,
                false,
            ))
        });
        assert_eq!(decode(&s, b""), expected);
    }

    /// A whole frame lowers headlessly, and the version/root guards hold.
    #[test]
    fn lower_binary_smoke_and_guards() {
        let mut core = kui_core::Core::new();
        core.begin_frame(Size::new(200.0, 100.0), 1.0);
        let v = VERSION as f64;
        // root {bg}, text "hi" {}, open {} close, end
        let stream = [
            v,
            OP_ROOT as f64,
            1.0,
            schema::P_BG as f64,
            0x202030ffu32 as f64,
            OP_TEXT as f64,
            0.0,
            2.0,
            0.0,
            OP_OPEN as f64,
            0.0,
            OP_CLOSE as f64,
            OP_END as f64,
        ];
        // No filler: `Ui::wrap` is the frame without one, which is what a
        // view with no extensions loaded gets and what every op but
        // `OP_SLOT` cares about.
        lower_binary(&mut kui_core::Ui::wrap(&mut core), &stream, b"hi").unwrap();
        core.finish_frame();
        assert!(!core.output().0.quads.is_empty(), "root bg draws a quad");

        let stale = [v + 1.0, OP_ROOT as f64, 0.0, OP_END as f64];
        assert!(lower_binary(&mut kui_core::Ui::wrap(&mut core), &stale, b"").is_err());
        assert!(
            lower_binary(&mut kui_core::Ui::wrap(&mut core), &[v, OP_END as f64], b"").is_err()
        );
        assert!(
            lower_binary(
                &mut kui_core::Ui::wrap(&mut core),
                &[v, OP_ROOT as f64],
                b""
            )
            .is_err()
        );
    }
}
