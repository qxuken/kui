//! The flat binary IR path: JS encodes the frame as one instruction stream
//! (a Float64Array — opcodes, prop ids, and values are all f64) plus a UTF-8
//! string table (a Uint8Array), and this module lowers it straight into the
//! core. One boundary crossing, zero per-node napi calls, strings borrowed
//! from the table — no `serde_json::Value` tree ever exists.
//!
//! The prop surface is defined once in `schema.rs`; this decoder reads
//! generic props by their schema kind and applies them through the same
//! table the JSON path uses, so the two transports cannot disagree. Only
//! composite props (pad/border/overflow/float) and the constructor specials
//! (dir/size/key/title/keyFocus) have hand-written arms here, mirrored in
//! the encoder.
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
//! - Message payloads (`onClick` etc.) are JSON strings in the table.

use kui_core::{
    Color, Core, Edges, EditOptions, FloatConfig, ImageId, NodeSpec, Span, TextStyle, widgets,
};
use serde_json::{Map as JsonMap, Value as Json};

use crate::schema::{
    self, Kind, P_BORDER, P_DIR, P_FLOAT, P_KEY, P_KEY_FOCUS, P_OVERFLOW, P_PAD, P_SIZE, P_TITLE,
    Parsed, PropsOut, align_idx, color_num, sizing_num,
};
use crate::{Result, err, value_of};

/// Bumped when the wire format changes (v2: enum props became list indices).
pub const VERSION: u32 = 2;

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
            ]
            .into_iter()
            .map(|(k, v)| (k.to_string(), Json::from(v)))
            .collect::<JsonMap<_, _>>(),
        ),
    );
    o.insert("prop".into(), schema::protocol_props());
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

fn payload(s: &str) -> Result<kui_core::Value> {
    let json: Json = serde_json::from_str(s).map_err(|e| err(format!("bad payload JSON: {e}")))?;
    Ok(value_of(&json))
}

/// The binary prop parser: same structure as `schema::parse_props_json` —
/// composites hand-written, everything else read by schema kind and applied
/// through the shared table.
fn read_props(r: &mut Reader<'_>) -> Result<PropsOut> {
    let n = r.u()?;
    let mut out = PropsOut::new();
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
                let (l, rr, t, b) = (r.f()?, r.f()?, r.f()?, r.f()?);
                out.spec = std::mem::take(&mut out.spec).padding(Edges {
                    l: l as f32,
                    r: rr as f32,
                    t: t as f32,
                    b: b as f32,
                });
            }
            P_BORDER => {
                let (w, c) = (r.f()?, r.f()?);
                out.spec = std::mem::take(&mut out.spec).border(w as f32, color_num(c as u32));
            }
            P_OVERFLOW => {
                let bits = r.u()?;
                let mut spec = std::mem::take(&mut out.spec);
                if bits & 1 != 0 {
                    spec = spec.clip();
                }
                if bits & 2 != 0 {
                    spec = spec.scroll_x();
                }
                if bits & 4 != 0 {
                    spec = spec.scroll_y();
                }
                out.spec = spec;
            }
            P_FLOAT => {
                let preset = r.u()?;
                let mut cfg = match preset {
                    1 => FloatConfig::viewport(),
                    2 => FloatConfig::below(),
                    3 => FloatConfig::above(),
                    _ => FloatConfig::parent(),
                };
                let has_at = r.u()? == 1;
                let (ax, ay) = (r.u()?, r.u()?);
                if has_at {
                    cfg = cfg.at(align_idx(ax as usize), align_idx(ay as usize));
                }
                let has_self = r.u()? == 1;
                let (sx, sy) = (r.u()?, r.u()?);
                if has_self {
                    cfg = cfg.self_at(align_idx(sx as usize), align_idx(sy as usize));
                }
                cfg = cfg.offset(r.f()? as f32, r.f()? as f32);
                if r.u()? == 1 {
                    cfg = cfg.fit();
                }
                out.spec = std::mem::take(&mut out.spec).float(cfg);
            }
            P_KEY_FOCUS => out.key_focus = true,
            P_KEY => out.key = Some(r.req_str()?.to_string()),
            P_TITLE => out.title = Some(r.req_str()?.to_string()),
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
                    Kind::Msg => Parsed::Msg(payload(r.req_str()?)?),
                };
                schema::apply(def, parsed, &mut out)?;
            }
        }
    }
    Ok(out)
}

fn decode_op(op: u32, r: &mut Reader<'_>, core: &mut Core) -> Result<()> {
    match op {
        OP_OPEN => {
            let p = read_props(r)?;
            let node_key = match &p.key {
                Some(label) => core.open_keyed(label, p.spec),
                None => core.open(p.spec),
            };
            if p.key_focus {
                core.set_key_focus(Some(node_key));
            }
            decode_until_close(r, core)?;
            core.close();
            Ok(())
        }
        OP_TEXT => {
            let content = r.req_str()?;
            let p = read_props(r)?;
            core.text_node(content, p.style);
            Ok(())
        }
        OP_RICH_TEXT => {
            let p = read_props(r)?;
            let nspans = r.u()? as usize;
            let mut spans = Vec::with_capacity(nspans);
            for _ in 0..nspans {
                let text = r.req_str()?;
                let flags = r.u()?;
                let color = r.f()?;
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
                spans.push(s);
            }
            core.rich_text_node(&spans, p.style);
            Ok(())
        }
        OP_BUTTON => {
            // Mirrors the JSON path's button styling exactly.
            let label = r.req_str()?;
            let key = r.str_ref()?;
            let msg = match r.str_ref()? {
                Some(s) => payload(s)?,
                None => kui_core::Value::Null,
            };
            let label_key = key.unwrap_or(label);
            let node_key = core.child_key(label_key);
            let bg = if core.is_pressed(node_key) {
                Color::rgb8(0x2f, 0x54, 0xc4)
            } else if core.is_hovered(node_key) {
                Color::rgb8(0x47, 0x6c, 0xe0)
            } else {
                Color::rgb8(0x3b, 0x5b, 0xd4)
            };
            core.open_keyed(
                label_key,
                NodeSpec::row()
                    .pad_xy(14.0, 8.0)
                    .bg(bg)
                    .radius(6.0)
                    .center()
                    .on_click(msg),
            );
            core.text_node(label, TextStyle::new(15.0).color(Color::WHITE));
            core.close();
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
            core.text_edit(label, initial, &opts, p.spec);
            Ok(())
        }
        OP_IMAGE => {
            let (hi, lo) = (r.f()? as u64, r.f()? as u64);
            let p = read_props(r)?;
            core.image_node(ImageId::from_ffi((hi << 32) | lo), p.spec);
            Ok(())
        }
        OP_TITLEBAR => {
            let title = r.str_ref()?.map(str::to_string);
            let has_children = r.u()? == 1;
            let mut ui = kui_core::Ui::wrap(core);
            if has_children {
                // titlebar_with opens and closes its own container, so the
                // children loop must only consume up to the CLOSE op.
                let mut result = Ok(());
                widgets::titlebar_with(&mut ui, |ui| {
                    result = decode_until_close(r, ui.core());
                });
                result
            } else {
                widgets::titlebar(&mut ui, title.as_deref().unwrap_or(""));
                Ok(())
            }
        }
        OP_WINDOW_BUTTONS => {
            widgets::window_buttons(&mut kui_core::Ui::wrap(core));
            Ok(())
        }
        OP_LATENCY_GRAPH => {
            widgets::latency_graph(&mut kui_core::Ui::wrap(core));
            Ok(())
        }
        OP_LATENCY_HUD => {
            let (x, y) = (r.u()?, r.u()?);
            widgets::latency_hud_at(
                &mut kui_core::Ui::wrap(core),
                align_idx(x as usize),
                align_idx(y as usize),
            );
            Ok(())
        }
        other => Err(err(format!("unknown opcode {other}"))),
    }
}

/// Runs ops until the matching CLOSE, which it consumes but does not act on —
/// the caller owns closing (or not, for widgets that close themselves).
fn decode_until_close(r: &mut Reader<'_>, core: &mut Core) -> Result<()> {
    loop {
        match r.u()? {
            OP_CLOSE => return Ok(()),
            op => decode_op(op, r, core)?,
        }
    }
}

/// Lowers a whole encoded frame (the binary analogue of `lower_root`).
pub fn lower_binary(core: &mut Core, stream: &[f64], strings: &[u8]) -> Result<()> {
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
    let p = read_props(&mut r)?;
    if let Some(t) = &p.title {
        core.set_window_title(t);
    }
    core.configure_root(p.spec);
    if p.key_focus {
        core.set_key_focus(Some(core.root_key()));
    }
    loop {
        match r.u()? {
            OP_END => return Ok(()),
            op => decode_op(op, &mut r, core)?,
        }
    }
}
