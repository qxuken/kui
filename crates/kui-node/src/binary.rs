//! The flat binary frame path: JS encodes the frame as one instruction stream
//! (a Float64Array of opcodes, prop ids and values) plus a UTF-8 string table
//! (a Uint8Array), and this module lowers it straight into the core. One
//! boundary crossing, no per-node napi calls, strings borrowed from the
//! table, and no `serde_json::Value` tree in between.
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
    Align, Color, Content, EditOptions, FloatConfig, ImageId, Key, NodeSpec, PadShorthand, Sizing,
    Span, TextStyle, TokenKind, TokenLookup, Warning, WindowConfig, widgets,
};
use serde_json::{Map as JsonMap, Value as Json};

use crate::schema::{
    self, Kind, P_ALWAYS_ON_TOP, P_BORDER, P_DIR, P_FLOAT, P_IME_OFF, P_INDEX, P_KEY, P_KEY_FOCUS,
    P_OPTION_AS_ALT, P_OVERFLOW, P_PAD, P_ROW_COUNT, P_SECURE_INPUT, P_SIZE, P_TITLE, P_TOOLTIP,
    P_WINDOWS, Parsed, PropsOut, SIZE_MODE_CALC, SIZE_MODE_TREE, align_idx, color_num, min_num,
    sizing_num,
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
/// terminal's selection survives a scroll. A slot
/// in the middle of the cells op rather than a new op, so the bump is what
/// keeps an older encoder's stream from being read as if it had one.
/// v7: a `windows` entry's `activates` slot is 0/1/2, 2 for "unsaid" — the
/// kind's own default, decided by the core (`WindowConfig::of_kind`) and
/// not by the encoder; and `measureText` sends its text as one encoded
/// element (`measure_binary`) rather than a JSON tree.
/// v8: `image` carries its `sampling` and `fit` rows as two slots before
/// its props, and `polygon` is a new op — the slots in the
/// middle of an existing op are what the bump is for.
/// v9: `fragment` carries its `image` handle as two slots after `src`
/// — slots in the middle of an op again.
/// v10: a prop id may carry `TOKEN_TAG`.
/// v11: a `$name` reaches the three slots it could not: a
/// tagged `min` row is one slot, the index; a `line`'s flags word bit 2
/// says its width slot is a length index; a `cells`' cursor-shape slot
/// bit 4 says its colour slot is a colour index. And a keyframe stop or
/// an entrance resolves a `$name` in the core — no wire change, but the
/// same release. Also: the edit op's flags word bit 4 says
/// the op is the stock field (`<input>`, `widgets::text_input`) and its
/// prop list is empty; and `tooltip` is a new op, the node form.
/// v12: the root's `windows` list rides as one JSON string, read entry by
/// entry through `WindowConfig::from_value` — the ten-slot
/// stanza v7 shaped is gone, and with it the encoder's own copy of the
/// kind list and of what a zero size means.
/// v14: `devtoolsTab` is a new op: name, label, slot (or
/// none), then a flags word whose bit 1 says the content follows to a
/// CLOSE — the encoder's function child, called only for the tab the
/// driver read as on show. A new op, so the bump is for an encoder that
/// emits one to an addon without it.
/// v15: `select` is a new op: label, the options as one
/// JSON string read by `MenuItem::options_from_value`, then the current
/// index plus one (0 = none). No prop list: the field reads no row but
/// its own. A new op, so the bump is for an encoder that emits one to an
/// addon without it.
/// v16: the float stanza's last slot, `fit` as 0 or 1 until now, is a
/// flags word — 1 `fit`, 2 `clip` — so an encoder that
/// sets 2 to an addon reading `== 1` would lose `fit` with the clip.
/// v13: an underline's own colour and shape. A span carries
/// a third colour slot, the underline's, with flags bits 256 (has one),
/// 512 (it is a token index), 1024 (wavy) and 2048 (dotted); a cell
/// carries a fourth slot, its underline colour (0 = fg), and the JSX
/// array may have four or five entries a cell. Slots in the middle of two
/// ops, which is what the bump is for; `underlineColor` and
/// `underlineStyle` on a `<text>` are ordinary schema rows.
/// v17: a span carries a fifth slot after its underline colour, its
/// background's radius in logical px, 0 for the square
/// background every span had. A slot in the middle of an op again.
/// v18: `family` is a strref, a stock name or an installed family's,
/// where it was the index into `schema::FAMILIES`.
/// v19: size expressions. A sizing, a min or a max whose
/// mode is `SIZE_MODE_CALC` (4) is followed by a strref, the spelling,
/// and one whose mode is `SIZE_MODE_TREE` (5) by a count and the
/// expression in prefix code (`calc::from_code`); `maxWidth` and
/// `maxHeight` are two slots, (mode, value), where they were one.
/// v20: `path` is a new op (ADR 0040).
pub const VERSION: u32 = 21;

/// The bit an encoder sets on a prop id to say the value slot holds a
/// token index rather than a value: `bg="$peach"` rides as `P_BG | TOKEN_TAG` then the index
/// `TokenRef::index` gives, and `read_props` resolves it through the
/// core's lookup before the schema applies it. Ids are small, so the bit
/// is free; a reference costs the wire nothing and the decoder one mask.
pub const TOKEN_TAG: u32 = 0x8000;

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
pub const OP_POLYGON: u32 = 19;
pub const OP_TOOLTIP: u32 = 20;
pub const OP_DEVTOOLS_TAB: u32 = 21;
pub const OP_SELECT: u32 = 22;
/// A stock toggle: the kind — 0 checkbox, 1 radio, 2 switch —
/// then the button's layout: text, key?, click payload?, a prop list of
/// the rows it admits.
pub const OP_TOGGLE: u32 = 23;
/// The stock slider: label, key?, a prop list of the rows it
/// admits.
pub const OP_SLIDER: u32 = 24;
/// A radio group: label, a prop list of box rows, children
/// until the CLOSE op.
pub const OP_RADIO_GROUP: u32 = 25;
/// A path (ADR 0040): `d` as a strref (none for the flat form), a count
/// and that many floats of the flat op form, the stroke width slot, a
/// flags word (bit 1: the width slot is a length token's index; bit 2:
/// even-odd; bit 4: `rotate` was declared; bit 8: `pivot` was; bit 16:
/// the stroke is dashed), the turns, the pivot's x and y (ADR 0041; zero
/// when not declared), the dash when bit 16 says so — its four lengths
/// and its offset (v21, backlog V2) — then props.
pub const OP_PATH: u32 = 26;

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
                ("polygon", OP_POLYGON),
                ("tooltip", OP_TOOLTIP),
                ("devtoolsTab", OP_DEVTOOLS_TAB),
                ("select", OP_SELECT),
                ("toggle", OP_TOGGLE),
                ("slider", OP_SLIDER),
                ("radioGroup", OP_RADIO_GROUP),
                ("path", OP_PATH),
            ]
            .into_iter()
            .map(|(k, v)| (k.to_string(), Json::from(v)))
            .collect::<JsonMap<_, _>>(),
        ),
    );
    o.insert("prop".into(), schema::protocol_props());
    // The verb table (backlog B1a), for the suite's pin of the two classes
    // against it and for the docs generator: one row per verb, each cell
    // `{is}` (the binding's spelling), `{as}` (the same thing in another
    // form) or `{no}` (why there is none).
    o.insert(
        "doors".into(),
        Json::Array(
            kui_core::schema::DOORS
                .iter()
                .map(|d| {
                    let cell = |c: kui_core::schema::Cell| {
                        let (k, v) = match c {
                            kui_core::schema::Cell::Is(s) => ("is", s),
                            kui_core::schema::Cell::As(s) => ("as", s),
                            kui_core::schema::Cell::No(s) => ("no", s),
                        };
                        Json::Object(
                            [(k.to_string(), Json::String(v.into()))]
                                .into_iter()
                                .collect(),
                        )
                    };
                    Json::Object(
                        [
                            ("rust", Json::String(d.rust.into())),
                            ("c", cell(d.c)),
                            ("node", cell(d.node)),
                            ("lua", cell(d.lua)),
                            ("doc", Json::String(d.doc.into())),
                        ]
                        .into_iter()
                        .map(|(k, v)| (k.to_string(), v))
                        .collect(),
                    )
                })
                .collect(),
        ),
    );
    o.insert("tokenTag".into(), Json::from(TOKEN_TAG));
    // The role names a `$name` may take in front of the app's tokens, in
    // wire order (ADR 0027, decision 6): the encoder resolves `$surface`
    // to index 1 of the colour space and `$radius` to its metric's, the
    // same indices `TokenRef::index` gives the core.
    let roles = |names: Vec<&str>| Json::Array(names.into_iter().map(Json::from).collect());
    o.insert(
        "tokenRoles".into(),
        Json::Object(
            [
                (
                    "colors".to_string(),
                    roles(
                        kui_core::schema::THEME_ROLES
                            .iter()
                            .map(|r| r.node)
                            .collect(),
                    ),
                ),
                (
                    "lengths".to_string(),
                    roles(
                        kui_core::schema::METRIC_ROLES
                            .iter()
                            .map(|r| r.node)
                            .collect(),
                    ),
                ),
            ]
            .into_iter()
            .collect(),
        ),
    );
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
    // A `cells` cursor's shapes, in wire order (backlog AR40).
    o.insert(
        "cellCursors".into(),
        Json::Array(
            kui_core::CellCursor::NAMES
                .iter()
                .map(|a| Json::String((*a).into()))
                .collect(),
        ),
    );
    // An underline's shapes, in wire order — what a `<span underlineStyle>`
    // is checked against (backlog K4).
    o.insert(
        "underlineStyles".into(),
        Json::Array(
            kui_core::UnderlineStyle::NAMES
                .iter()
                .map(|a| Json::String((*a).into()))
                .collect(),
        ),
    );
    // The root's Option-as-Alt sides, in wire order (backlog F113).
    o.insert(
        "optionAsAlt".into(),
        Json::Array(
            kui_core::OptionAsAlt::ALL
                .iter()
                .map(|v| Json::String(v.name().into()))
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

    /// A counted run of slots: the count, then that many (a size
    /// expression's prefix code, v19).
    fn code(&mut self) -> Result<&'a [f64]> {
        let n = self.u()? as usize;
        let run = self
            .s
            .get(self.i..self.i + n)
            .ok_or_else(|| err("binary frame truncated"))?;
        self.i += n;
        Ok(run)
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
/// `Some` of a size expression, `None` for one the full table refused
/// ([`kui_core::calc::is_full`]) — the prop left undeclared — and the
/// frame's error for a bad one.
fn kept<T>(r: std::result::Result<T, String>) -> Result<Option<T>> {
    match r {
        Ok(v) => Ok(Some(v)),
        Err(e) if kui_core::calc::is_full(&e) => Ok(None),
        Err(e) => Err(err(e)),
    }
}

fn read_props(r: &mut Reader<'_>, refs: &mut Refs<'_>) -> Result<PropsOut> {
    read_props_over(r, PropsOut::new(), refs)
}

/// What a token index resolves through while a prop list is read: the
/// core's lookup for the window being lowered, and the indices
/// that named nothing in it. The encoder writes an index only for a name
/// it resolved against *its* map, but the map is the surface's and the
/// table is a core's — `setTokens` reaches the main window's core, and a
/// second window lowers against its own — so a miss is a slot left at its
/// default and an `unknown-token` line, never a frame refused.
struct Refs<'a> {
    look: TokenLookup<'a>,
    missed: Vec<(TokenKind, u32)>,
    /// The by-name half, for a keyframe stop or an entrance, which cross
    /// as plain data with the `$name` still in them.
    names: kui_core::NameRefs<'a>,
}

/// What a lowering could not resolve: token indices the table does not
/// hold, and names a stop or an entrance spelled that nothing declared.
struct Missed {
    by_index: Vec<(TokenKind, u32)>,
    by_name: Vec<kui_core::TokenError>,
    /// `family` names nothing installed or loaded matched.
    families: Vec<String>,
}

impl<'a> Refs<'a> {
    fn new(look: TokenLookup<'a>) -> Self {
        Self {
            look,
            missed: Vec::new(),
            names: kui_core::NameRefs::new(look),
        }
    }

    fn take(mut self) -> Missed {
        Missed {
            by_index: std::mem::take(&mut self.missed),
            by_name: self.names.take_missed(),
            families: self.names.take_missed_families(),
        }
    }

    fn color(&mut self, index: f64) -> Option<Color> {
        let c = self.look.color_at(index as u32);
        if c.is_none() {
            self.missed.push((TokenKind::Color, index as u32));
        }
        c
    }

    fn length(&mut self, index: f64) -> Option<f32> {
        let v = self.look.length_at(index as u32);
        if v.is_none() {
            self.missed.push((TokenKind::Length, index as u32));
        }
        v
    }
}

/// The warnings a lowering's misses become, once the borrow of the core
/// is handed back: keyed by the index, since the name never crossed —
/// except for a stop's or an entrance's, which did.
fn warn_missed(core: &mut kui_core::Core, missed: Missed) {
    for e in &missed.by_name {
        core.warn_unknown_token(e);
    }
    for name in &missed.families {
        core.warn_unknown_family(name);
    }
    for (kind, index) in missed.by_index {
        core.warn(Warning {
            code: kui_core::diag::UNKNOWN_TOKEN,
            key: Key::ROOT.str(kui_core::diag::UNKNOWN_TOKEN).str(&format!(
                "#{}{}",
                kind.name(),
                index
            )),
            message: format!(
                "a `$name` on a {} slot resolved to index {index}, which this window's table \
                 does not hold, so the slot keeps its default: tokens are declared per window \
                 (`setTokens` reaches the main window's core), and a second window has none \
                 unless it declares them",
                kind.name()
            ),
        });
    }
}

/// One length by wire index, for a slot outside a prop list (a line's
/// width); a miss is raised the way a prop's is.
fn lookup_length(ui: &mut kui_core::Ui<'_>, index: f64) -> Option<f32> {
    let mut refs = Refs::new(ui.core().token_lookup());
    let v = refs.length(index);
    let missed = refs.take();
    warn_missed(ui.core(), missed);
    v
}

/// One colour by wire index (a grid's cursor colour), the same way.
fn lookup_color(ui: &mut kui_core::Ui<'_>, index: f64) -> Option<Color> {
    let mut refs = Refs::new(ui.core().token_lookup());
    let c = refs.color(index);
    let missed = refs.take();
    warn_missed(ui.core(), missed);
    c
}

/// Reads one prop list through the core's lookup and raises what missed.
/// A stroke's dash off the stream when its flag says there is one: four
/// lengths and an offset, as the encoder normalized them (backlog V2).
fn read_dash(r: &mut Reader<'_>, dashed: bool) -> Result<kui_core::Dash> {
    if !dashed {
        return Ok(kui_core::Dash::SOLID);
    }
    let mut pattern = [0.0; 4];
    for l in &mut pattern {
        *l = r.f()? as f32;
    }
    Ok(kui_core::Dash {
        pattern,
        offset: r.f()? as f32,
    })
}

fn lower_props(r: &mut Reader<'_>, ui: &mut kui_core::Ui<'_>) -> Result<PropsOut> {
    lower_props_over(r, PropsOut::new(), ui)
}

fn lower_props_over(
    r: &mut Reader<'_>,
    out: PropsOut,
    ui: &mut kui_core::Ui<'_>,
) -> Result<PropsOut> {
    let (p, missed) = {
        let mut refs = Refs::new(ui.core().token_lookup());
        (read_props_over(r, out, &mut refs), refs.take())
    };
    warn_missed(ui.core(), missed);
    p
}

fn lower_spans<'a>(r: &mut Reader<'a>, ui: &mut kui_core::Ui<'_>) -> Result<Vec<Span<'a>>> {
    let (spans, missed) = {
        let mut refs = Refs::new(ui.core().token_lookup());
        (read_spans(r, &mut refs), refs.take())
    };
    warn_missed(ui.core(), missed);
    spans
}

/// [`read_props`] applied over `out` rather than the schema defaults: what
/// a composite that keeps its own look reads its admitted rows into (the
/// stock button over `widgets::button_spec`).
fn read_props_over(r: &mut Reader<'_>, mut out: PropsOut, refs: &mut Refs<'_>) -> Result<PropsOut> {
    let n = r.u()?;
    for _ in 0..n {
        let raw = r.u()?;
        // A tagged id's value slot holds a token index (ADR 0027): a
        // length for a size, a `pad` edge, a border width or an f32/sizing
        // row, a colour for a colour row or a border colour. Resolved here,
        // before `schema::apply`, so the core never sees a reference.
        let is_ref = raw & TOKEN_TAG != 0;
        let id = raw & !TOKEN_TAG;
        match id {
            P_DIR => match r.u()? {
                1 => out.spec = NodeSpec::row(),
                // A table: a column whose rows' cells line up (ADR 0033).
                2 => out.spec = NodeSpec::table(),
                _ => {}
            },
            P_SIZE if is_ref => {
                if let Some(px) = refs.length(r.f()?) {
                    out.style = TextStyle::new(px);
                }
            }
            P_SIZE => out.style = TextStyle::new(r.f()? as f32),
            P_PAD => {
                // The shorthand rides the wire as declared (a set mask plus
                // the seven values), so the encoder never has to know what
                // `padX` falls back to either. Tagged, a second mask says
                // which of the seven are token indices.
                let set = r.u()?;
                let ref_mask = if is_ref { r.u()? } else { 0 };
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
                    let raw = r.f()?;
                    let v = if ref_mask & (1 << bit) != 0 {
                        refs.length(raw)
                    } else {
                        Some(raw as f32)
                    };
                    if set & (1 << bit) != 0 {
                        *slot = v;
                    }
                }
                out.apply_pad(pad);
            }
            P_BORDER => {
                // Tagged: a flags word first — 1 the width is an index, 2
                // the colour is.
                let flags = if is_ref { r.u()? } else { 0 };
                let (w, c) = (r.f()?, r.f()?);
                let w = if flags & 1 != 0 {
                    refs.length(w).unwrap_or(0.0)
                } else {
                    w as f32
                };
                let c = if flags & 2 != 0 {
                    refs.color(c).unwrap_or(Color::TRANSPARENT)
                } else {
                    color_num(c as u32)
                };
                out.spec = std::mem::take(&mut out.spec).border(w, c);
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
                // A flags word: 1 `fit`, 2 `clip` (v16; the slot was a
                // bare `fit` 0/1 before, which bit 1 still reads as).
                let flags = r.u()?;
                let (fit, clip) = (flags & 1 != 0, flags & 2 != 0);
                let cfg = FloatConfig::build(base, at, self_at, dx, dy, fit, clip);
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
            // A row count, refused on the same terms as `index`.
            P_ROW_COUNT => {
                let n = r.f()?;
                if !n.is_finite() || n < 0.0 || n.fract() != 0.0 {
                    return Err(err(format!(
                        "rowCount must be a whole number of rows, not {n}: it is how many indexed \
                         rows the list has, built or not"
                    )));
                }
                out.row_count = Some(n as u64);
            }
            P_TITLE => out.title = Some(r.req_str()?.to_string()),
            // Root only, like `title`; a flag, like `keyFocus`.
            P_ALWAYS_ON_TOP => out.always_on_top = true,
            // The same shape (backlog F85).
            P_SECURE_INPUT => out.secure_input = true,
            // And again (backlog F125).
            P_IME_OFF => out.ime_off = true,
            // Root only, a side by its number (`OptionAsAlt::index`,
            // backlog F113); the encoder writes nothing for "none" and
            // refuses a name it does not know, so a number past the four
            // is a bad stream.
            P_OPTION_AS_ALT => {
                let i = r.u()?;
                out.option_as_alt = kui_core::OptionAsAlt::from_index(i)
                    .ok_or_else(|| err(format!("unknown optionAsAlt {i}")))?;
            }
            // One JSON blob, the list as the view wrote it, each entry
            // read by `WindowConfig::from_value` — the reader Lua's list
            // goes through, so the two cannot disagree on what a zero
            // width means or which kinds there are (v12, backlog AR43;
            // `menuBar` rides the same way).
            P_WINDOWS => {
                let list = payload(r.req_str()?)?;
                let kui_core::Value::List(entries) = &list else {
                    return Err(err("windows: a list of names or entries"));
                };
                for entry in entries {
                    out.windows.push(
                        WindowConfig::from_value(entry)
                            .map_err(|e| err(format!("windows: {e}")))?,
                    );
                }
            }
            P_TOOLTIP => out.apply_tooltip(r.req_str()?),
            _ if is_ref
                && !matches!(
                    schema::by_id(id).map(|d| &d.kind),
                    Some(Kind::F32 | Kind::Color | Kind::Sizing | Kind::Min | Kind::Max)
                ) =>
            {
                return Err(err(format!(
                    "prop id {id} carries a token reference, and only a colour, a length, a min or a \
                     sizing row takes one"
                )));
            }
            id => {
                let def = schema::by_id(id).ok_or_else(|| err(format!("unknown prop id {id}")))?;
                let parsed = match &def.kind {
                    // A reference that names nothing in this window's
                    // table leaves the row unapplied — the default the
                    // encoder would have left it at for an unknown name.
                    Kind::F32 if is_ref => match refs.length(r.f()?) {
                        Some(px) => Parsed::F32(px),
                        None => continue,
                    },
                    Kind::Color if is_ref => match refs.color(r.f()?) {
                        Some(c) => Parsed::Color(c),
                        None => continue,
                    },
                    // A sizing reference is one slot, and it is always a
                    // fixed length in px.
                    Kind::Sizing if is_ref => match refs.length(r.f()?) {
                        Some(px) => Parsed::Sizing(Sizing::Fixed(px)),
                        None => continue,
                    },
                    // A min reference is one slot too: a fixed clamp of
                    // that many px (v11, AR14).
                    Kind::Min | Kind::Max if is_ref => match refs.length(r.f()?) {
                        Some(px) => Parsed::Bound(kui_core::Bound::Px(px)),
                        None => continue,
                    },
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
                    // A size expression (v19): its spelling as a strref, or
                    // as data in prefix code after a count (backlog F109).
                    // An expression the full table refused leaves the row
                    // at its default, as a reference that misses does,
                    // and the core warns (backlog RG93).
                    Kind::Sizing => Parsed::Sizing(match r.u()? {
                        SIZE_MODE_CALC => match kept(schema::sizing_str(r.req_str()?))? {
                            Some(s) => s,
                            None => continue,
                        },
                        SIZE_MODE_TREE => match kept(kui_core::calc::sizing_code(r.code()?))? {
                            Some(s) => s,
                            None => continue,
                        },
                        m => sizing_num(m, r.f()?),
                    }),
                    Kind::Min | Kind::Max => Parsed::Bound(match r.u()? {
                        SIZE_MODE_CALC => {
                            let s = r.req_str()?;
                            let b = if matches!(def.kind, Kind::Min) {
                                schema::min_str(s)
                            } else {
                                schema::max_str(s)
                            };
                            match kept(b)? {
                                Some(b) => b,
                                None => continue,
                            }
                        }
                        SIZE_MODE_TREE => match kept(kui_core::calc::bound_code(r.code()?))? {
                            Some(b) => b,
                            None => continue,
                        },
                        m => min_num(m, r.f()?),
                    }),
                    Kind::Msg | Kind::Tag => Parsed::Msg(payload(r.req_str()?)?),
                    Kind::Str => Parsed::Str(r.req_str()?.to_string()),
                    // A strref since v18: a stock family or an installed
                    // one by name, registered as it is read (ADR 0037).
                    Kind::Family => Parsed::Family(refs.names.family(r.req_str()?)),
                    Kind::Resource => Parsed::Resource(crate::parse_u64(r.req_str()?)?),
                    // Carried as JSON like a message; the core reads the stops.
                    // A `$name` in a stop resolves in the core through
                    // the by-name refs, and misses the way a prop's does
                    // (AR14).
                    Kind::Keyframes => Parsed::Keyframes(
                        kui_core::keyframes::parse_with(
                            &payload(r.req_str()?)?,
                            Some(&mut refs.names),
                        )
                        .map_err(err)?,
                    ),
                    Kind::Enter => Parsed::Enter(
                        kui_core::enter::parse_with(&payload(r.req_str()?)?, Some(&mut refs.names))
                            .map_err(err)?,
                    ),
                    // JSON too; a `$name` stop resolves as a keyframe's does.
                    Kind::Gradient => Parsed::Gradient(
                        kui_core::gradient::parse_with(
                            &payload(r.req_str()?)?,
                            Some(&mut refs.names),
                        )
                        .map_err(err)?,
                    ),
                };
                schema::apply(def, parsed, &mut out)?;
            }
        }
    }
    Ok(out)
}

/// The span list of a rich text: a count, then per span its text and the
/// flags the encoder's `collectSpans` packed (1 bold, 2 italic, 4 has
/// colour, 8 underline, 16 strikethrough, 32 has bg, 64 the colour is a
/// token index, 128 the bg is, 256 has an underline colour, 512 it is a
/// token index, 1024 the underline is wavy, 2048 dotted) with the three
/// colours (v13), then the background's radius (v17).
fn read_spans<'a>(r: &mut Reader<'a>, refs: &mut Refs<'_>) -> Result<Vec<Span<'a>>> {
    let nspans = r.u()? as usize;
    let mut spans = Vec::with_capacity(nspans);
    for _ in 0..nspans {
        let text = r.req_str()?;
        let flags = r.u()?;
        let color = r.f()?;
        let bg = r.f()?;
        let ul = r.f()?;
        let radius = r.f()?;
        let mut s = Span::new(text);
        if flags & 1 != 0 {
            s = s.bold();
        }
        if flags & 2 != 0 {
            s = s.italic();
        }
        if flags & 4 != 0 {
            let c = if flags & 64 != 0 {
                refs.color(color)
            } else {
                Some(color_num(color as u32))
            };
            if let Some(c) = c {
                s = s.color(c);
            }
        }
        if flags & 8 != 0 {
            s = s.underline();
        }
        if flags & 16 != 0 {
            s = s.strikethrough();
        }
        if flags & 32 != 0 {
            let c = if flags & 128 != 0 {
                refs.color(bg)
            } else {
                Some(color_num(bg as u32))
            };
            if let Some(c) = c {
                s = s.bg(c);
            }
        }
        if flags & 256 != 0 {
            let c = if flags & 512 != 0 {
                refs.color(ul)
            } else {
                Some(color_num(ul as u32))
            };
            if let Some(c) = c {
                s = s.underline_color(c);
            }
        }
        if flags & 1024 != 0 {
            s = s.underline_style(kui_core::UnderlineStyle::Wavy);
        } else if flags & 2048 != 0 {
            s = s.underline_style(kui_core::UnderlineStyle::Dotted);
        }
        if radius > 0.0 {
            s = s.bg_radius(radius as f32);
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
            let (p, missed) = {
                let mut refs = Refs::new(core.token_lookup());
                (read_props(&mut r, &mut refs), refs.take())
            };
            warn_missed(core, missed);
            Ok(core.measure_text(content, &p?.style, max_w))
        }
        OP_RICH_TEXT => {
            let (p, spans, missed) = {
                let mut refs = Refs::new(core.token_lookup());
                let p = read_props(&mut r, &mut refs);
                let spans = read_spans(&mut r, &mut refs);
                (p, spans, refs.take())
            };
            warn_missed(core, missed);
            Ok(core.measure_rich_text(&spans?, &p?.style, max_w))
        }
        op => Err(err(format!(
            "measureText expects one text element, got op {op}"
        ))),
    }
}

fn decode_op(op: u32, r: &mut Reader<'_>, ui: &mut kui_core::Ui<'_>) -> Result<()> {
    match op {
        OP_OPEN => {
            let p = lower_props(r, ui)?;
            ui.core().open_from(p, Content::Box);
            decode_until_close(r, ui)?;
            ui.core().close();
            Ok(())
        }
        OP_TEXT => {
            let content = r.req_str()?;
            let p = lower_props(r, ui)?;
            ui.core().text_node(content, p.style);
            Ok(())
        }
        OP_RICH_TEXT => {
            let p = lower_props(r, ui)?;
            let spans = lower_spans(r, ui)?;
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
            base.spec = widgets::button_spec(&ui.theme(), &ui.metrics());
            let p = lower_props_over(r, base, ui)?;
            // An `index` keys the button by its row, as it does a box
            // (backlog AR40): declared beside `key`, the index wins.
            match p.index {
                Some(i) => widgets::button_indexed(
                    ui,
                    i,
                    label,
                    p.spec.on_click(msg),
                    p.tooltip.as_deref(),
                ),
                None => widgets::button_with(
                    ui,
                    label_key,
                    label,
                    p.spec.on_click(msg),
                    p.tooltip.as_deref(),
                ),
            }
            Ok(())
        }
        OP_EDIT => {
            let label = r.req_str()?;
            let initial = r.str_ref()?.unwrap_or("");
            let flags = r.u()?;
            // A leaf: its `tooltip` floats beside it (backlog RG113).
            let p = lower_props(r, ui)?.for_leaf();
            // Bit 4: the stock field (`<input label initial>`), the same
            // widget Lua's `input { }` and C's `kui_text_input` lower to.
            if flags & 4 != 0 {
                widgets::text_input(ui, label, initial);
                return Ok(());
            }
            let opts = EditOptions {
                style: p.style,
                multiline: flags & 1 != 0,
                autofocus: flags & 2 != 0,
                wrap: p.wrap,
                ..Default::default()
            };
            ui.core().text_edit(label, initial, &opts, p.spec);
            Ok(())
        }
        // src (hi, lo), sampling, fit, then props (ADR 0025, decision 4).
        OP_IMAGE => {
            let (hi, lo) = (r.f()? as u64, r.f()? as u64);
            let sampling = r.u()? as usize;
            let fit = r.u()? as usize;
            // A leaf: its `tooltip` floats beside it (backlog RG113).
            let p = lower_props(r, ui)?.for_leaf();
            let opts = kui_core::ImageOpts {
                sampling: kui_core::Sampling::ALL
                    .get(sampling)
                    .copied()
                    .unwrap_or_default(),
                fit: kui_core::ImageFit::ALL
                    .get(fit)
                    .copied()
                    .unwrap_or_default(),
            };
            ui.core()
                .image_node_with(ImageId::from_ffi((hi << 32) | lo), opts, p.spec);
            Ok(())
        }
        // n, then n (x, y) pairs, then the prop list — `bg` is the fill,
        // and the core decides the box (ADR 0025, decision 6).
        OP_POLYGON => {
            let n = r.u()? as usize;
            let mut points = Vec::with_capacity(n);
            for _ in 0..n {
                let x = r.f()? as f32;
                let y = r.f()? as f32;
                points.push(kui_core::Vec2::new(x, y));
            }
            let p = lower_props(r, ui)?;
            ui.core().open_from(p, Content::Polygon(&points));
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
        // `d` as a strref or the flat op form, the stroke width slot,
        // flags (1 width is a length token, 2 even-odd, 4 rotate, 8
        // pivot, 16 dashed), the turns and the pivot, the dash's five
        // floats when it is dashed, then the prop
        // list — `bg` is the fill, `color` the stroke's
        // (docs/adr/0040-a-path-is-a-mask-in-the-atlas.md).
        OP_PATH => {
            let d = r.str_ref()?;
            let n = r.u()? as usize;
            let mut floats = Vec::with_capacity(n);
            for _ in 0..n {
                floats.push(r.f()? as f32);
            }
            let width_slot = r.f()?;
            let flags = r.u()?;
            let turns = r.f()? as f32;
            let pivot = kui_core::Vec2::new(r.f()? as f32, r.f()? as f32);
            let turn = (flags & 12 != 0).then_some(kui_core::Turn {
                turns,
                pivot: (flags & 8 != 0).then_some(pivot),
            });
            let dash = read_dash(r, flags & 16 != 0)?;
            let p = lower_props(r, ui)?;
            let width = if flags & 1 != 0 {
                lookup_length(ui, width_slot).unwrap_or(0.0)
            } else {
                width_slot as f32
            };
            // No `width` is no stroke; no `color` is the foreground.
            let stroke = (width > 0.0).then(|| {
                kui_core::Stroke::new(width, p.style.color.unwrap_or(ui.theme().fg)).dashed(dash)
            });
            let rule = if flags & 2 != 0 {
                kui_core::FillRule::EvenOdd
            } else {
                kui_core::FillRule::NonZero
            };
            match d {
                Some(d) => {
                    ui.core()
                        .open_from(p, Content::PathD(d, rule, stroke, turn));
                }
                None => {
                    // The encoder checked the numbers are finite, not
                    // that they are the form: the core says so.
                    ui.core()
                        .open_from(p, Content::PathFlat(&floats, rule, stroke, turn));
                }
            }
            Ok(())
        }
        // n, then n (x, y) pairs, width, flags (1 curve, 2 the width is
        // a token, 4 dashed), the dash's four lengths and its offset when
        // it is dashed (v21, backlog V2), then the prop
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
            let width_slot = r.f()?;
            let flags = r.u()?;
            let dash = read_dash(r, flags & 4 != 0)?;
            let p = lower_props(r, ui)?;
            // Bit 2: the width slot is a length token's index (v11, AR14);
            // one the table does not hold is the default stroke, 1 px.
            let width = if flags & 2 != 0 {
                lookup_length(ui, width_slot).unwrap_or(1.0)
            } else {
                width_slot as f32
            };
            // No `color` is the theme's foreground, as for a text run.
            let stroke_color = p.style.color.unwrap_or(ui.theme().fg);
            let mut stroke = kui_core::Stroke::new(width, stroke_color);
            stroke.curve = flags & 1 != 0;
            stroke.dash = dash;
            ui.core().open_from(p, Content::Line(&points, stroke));
            Ok(())
        }
        // src (hi, lo), image (hi, lo; both zero for none), param count,
        // the params, then props. An open node: the encoder emits OP_CLOSE
        // for it like any other parent, so a fragment's children paint
        // over it.
        OP_FRAGMENT => {
            let (hi, lo) = (r.f()? as u64, r.f()? as u64);
            let (ihi, ilo) = (r.f()? as u64, r.f()? as u64);
            let n = r.u()? as usize;
            let mut params = Vec::with_capacity(n);
            for _ in 0..n {
                params.push(r.f()? as f32);
            }
            let p = lower_props(r, ui)?;
            let image = (ihi << 32) | ilo;
            let frag = kui_core::FragmentRef {
                id: kui_core::FragmentId::from_ffi((hi << 32) | lo),
                image: (image != 0).then(|| kui_core::ImageId::from_ffi(image)),
            };
            ui.core().open_from(p, Content::Fragment(frag, &params));
            decode_until_close(r, ui)?;
            ui.core().close();
            Ok(())
        }
        OP_CELLS => {
            // rows, cols, cursor (present, row, col, shape, colour), then
            // four slots a cell: codepoint | flags << 21, fg, bg, and the
            // underline's own colour, 0 for fg (v13, backlog K4).
            let rows = r.u()? as usize;
            let cols = r.u()? as usize;
            let has_cursor = r.u()? == 1;
            let crow = r.u()? as usize;
            let ccol = r.u()? as usize;
            let cshape_slot = r.u()? as usize;
            let ccolor_slot = r.f()?;
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
                let ul = r.f()? as u32;
                cells.push(kui_core::Cell {
                    ch: char::from_u32(packed & 0x1f_ffff).unwrap_or(' '),
                    fg,
                    bg,
                    flags: (packed >> 21) as u8,
                    ul,
                });
            }
            let p = lower_props(r, ui)?;
            // Bit 4 of the shape slot: the colour slot is a colour token's
            // index (v11, AR14); one the table does not hold is the
            // default, white.
            let cshape = cshape_slot & 3;
            let ccolor = if cshape_slot & 4 != 0 {
                lookup_color(ui, ccolor_slot).unwrap_or(Color::WHITE)
            } else {
                color_num(ccolor_slot as u32)
            };
            let cursor = has_cursor.then(|| {
                (
                    crow,
                    ccol,
                    kui_core::CellCursor::from_index(cshape).unwrap_or(kui_core::CellCursor::Block),
                    ccolor,
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
        // The node form of a tooltip: `value` alone, or children until
        // the CLOSE op, the way a titlebar's are.
        OP_TOOLTIP => {
            let value = r.str_ref()?.map(str::to_string);
            let has_children = r.u()? == 1;
            if has_children {
                let mut result = Ok(());
                widgets::tooltip_with(ui, |ui| {
                    result = decode_until_close(r, ui);
                });
                result
            } else {
                widgets::tooltip(ui, value.as_deref().unwrap_or(""));
                Ok(())
            }
        }
        // A devtools tab (ADR 0032): the extension form names a slot; the
        // host form's content follows when the encoder called the function
        // child, which it did only for the tab the driver read as on show —
        // so the laziness is the encoder's, and the core builds what came.
        OP_DEVTOOLS_TAB => {
            let name = r.req_str()?;
            let label = r.req_str()?;
            let slot = r.str_ref()?;
            let flags = r.u()?;
            match slot {
                Some(slot) => {
                    ui.devtools_tab(name, label, slot);
                    Ok(())
                }
                None if flags & 1 != 0 => {
                    let mut result = Ok(());
                    ui.devtools_tab_declared(name, label, |ui| {
                        result = decode_until_close(r, ui);
                    });
                    result
                }
                None => {
                    ui.devtools_tab_declare(name, label, None);
                    Ok(())
                }
            }
        }
        OP_SELECT => {
            // Label, the options as one JSON blob (strings, or the item
            // objects `openMenu` takes), the current index plus one.
            let label = r.req_str()?;
            let options = crate::select_options_of(r.req_str()?).map_err(|e| err(e.to_string()))?;
            let current = r.u()?;
            let current = (current > 0).then(|| current as usize - 1);
            widgets::select_items(ui, label, &options, current);
            Ok(())
        }
        OP_TOGGLE => {
            let kind = match r.u()? {
                0 => widgets::Toggle::Checkbox,
                1 => widgets::Toggle::Radio,
                2 => widgets::Toggle::Switch,
                k => return Err(err(format!("toggle kind {k} out of range"))),
            };
            let text = r.req_str()?;
            let key = r.str_ref()?;
            let msg = match r.str_ref()? {
                Some(s) => payload(s)?,
                None => kui_core::Value::Null,
            };
            let mut base = PropsOut::new();
            base.spec = widgets::toggle_spec(&ui.metrics()).on_click(msg);
            let p = lower_props_over(r, base, ui)?;
            widgets::toggle_with(
                ui,
                kind,
                key.unwrap_or(text),
                text,
                p.spec,
                p.tooltip.as_deref(),
            );
            Ok(())
        }
        OP_SLIDER => {
            // Named by its label whatever it is keyed by.
            let label = r.req_str()?;
            let key = r.str_ref()?;
            let mut base = PropsOut::new();
            base.spec = widgets::slider_spec(&ui.metrics()).label(label);
            let p = lower_props_over(r, base, ui)?;
            widgets::slider_with(ui, key.unwrap_or(label), p.spec, p.tooltip.as_deref());
            Ok(())
        }
        OP_RADIO_GROUP => {
            let label = r.req_str()?.to_string();
            let p = lower_props(r, ui)?;
            let mut result = Ok(());
            widgets::radio_group_with(ui, &label, p.spec, |ui| {
                result = decode_until_close(r, ui);
            });
            result
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
    let root = lower_props(&mut r, ui)?;
    ui.core().configure_root_from(root);
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
        let core = kui_core::Core::new();
        let mut refs = Refs::new(core.token_lookup());
        read_props(&mut r, &mut refs).unwrap()
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
                    Parsed::Bound(kui_core::Bound::Fit)
                }
                Kind::Max => {
                    stream.extend([0.0, 37.0]);
                    Parsed::Bound(kui_core::Bound::Px(37.0))
                }
                Kind::Msg | Kind::Tag => {
                    stream.extend([0.0, 1.0]);
                    Parsed::Msg(Value::Int(7))
                }
                Kind::Str => {
                    stream.extend([0.0, 1.0]);
                    Parsed::Str("7".into())
                }
                // "7" names no family, so it is sans — and the stream
                // still reads as one strref.
                Kind::Family => {
                    stream.extend([0.0, 1.0]);
                    Parsed::Family(kui_core::FontFamily::Sans)
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
                Kind::Gradient => {
                    strings = br##"{"to":"right","stops":["#112233",["#ffffff",0.5]]}"##;
                    stream.extend([0.0, strings.len() as f64]);
                    Parsed::Gradient(kui_core::Gradient::to(
                        kui_core::Side::Right,
                        [
                            kui_core::GradientStop::from(kui_core::Color::hex(0x112233ff)),
                            kui_core::GradientStop::from((kui_core::Color::WHITE, 0.5)),
                        ],
                    ))
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
                                .inside(Align::End, Align::End)
                                .offset(-8.0, -8.0)
                                .fit(),
                        )
                    });
                }
                "keyFocus" => expected.key_focus = true,
                "alwaysOnTop" => expected.always_on_top = true,
                "secureInput" => expected.secure_input = true,
                "imeOff" => expected.ime_off = true,
                "optionAsAlt" => {
                    s.push(2.0);
                    expected.option_as_alt = kui_core::OptionAsAlt::Right;
                }
                "key" => {
                    s.extend([0.0, 3.0]);
                    strings = b"abc";
                    expected.key = Some("abc".into());
                }
                "index" => {
                    s.push(7.0);
                    expected.index = Some(7);
                }
                "rowCount" => {
                    s.push(2000.0);
                    expected.row_count = Some(2000);
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
                    // One JSON blob (v12): a popup 400x300, non-activating,
                    // anchored to a 60x20 rect at (10, 20) — read by
                    // `WindowConfig::from_value`, the same reader Lua's
                    // list goes through.
                    const LIST: &[u8] = br#"[{"name":"abc","kind":"popup","width":400,"height":300,"activates":false,"anchor":{"x":10,"y":20,"w":60,"h":20}}]"#;
                    s.extend([0.0, LIST.len() as f64]);
                    strings = LIST;
                    expected.windows.push((
                        "abc".into(),
                        WindowConfig {
                            kind: kui_core::WindowKind::Popup,
                            size: Size::new(400.0, 300.0),
                            activates: false,
                            anchor: kui_core::Rect::new(10.0, 20.0, 60.0, 20.0),
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
                false,
            ))
        });
        assert_eq!(decode(&s, b""), expected);
    }

    /// The float stanza's last slot is a flags word (v16): 1 `fit`, 2
    /// `clip`, each read on its own, so `{ anchor: 'parent', clip: true }`
    /// does not also flip to stay on screen.
    #[test]
    fn the_float_flags_word_carries_fit_and_clip() {
        let float = |flags: f64| {
            let s = [
                1.0,
                P_FLOAT as f64,
                0.0, // parent
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
                flags,
            ];
            decode(&s, b"").spec.layout.float.unwrap()
        };
        assert!(!float(0.0).fit && !float(0.0).clip);
        assert!(float(1.0).fit && !float(1.0).clip);
        assert!(!float(2.0).fit && float(2.0).clipped_by_parent());
        assert!(float(3.0).fit && float(3.0).clip);
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
        kui_core::Ui::wrap(&mut core).finish();
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
