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
//! - Message payloads (`onClick` etc.) and keyframe lists are JSON strings
//!   in the table.

use kui_core::{
    Color, Core, Edges, EditOptions, FloatConfig, ImageId, NodeSpec, Span, TextStyle, widgets,
};
use serde_json::{Map as JsonMap, Value as Json};

use crate::schema::{
    self, Kind, P_BORDER, P_DIR, P_FLOAT, P_KEY, P_KEY_FOCUS, P_OVERFLOW, P_PAD, P_SIZE, P_TITLE,
    P_TOOLTIP, Parsed, PropsOut, align_idx, color_num, sizing_num,
};
use crate::{Result, err, value_of};

/// Bumped when the wire format changes (v2: enum props became list indices;
/// v3: float configs carry a has-offset flag so bare presets keep their gap).
pub const VERSION: u32 = 3;

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
            ]
            .into_iter()
            .map(|(k, v)| (k.to_string(), Json::from(v)))
            .collect::<JsonMap<_, _>>(),
        ),
    );
    o.insert("prop".into(), schema::protocol_props());
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
                // Bare presets (below/above) keep their built-in gap; a config
                // object always writes an explicit offset, as the JSON path
                // always applies dx/dy.
                let has_offset = r.u()? == 1;
                let (dx, dy) = (r.f()?, r.f()?);
                if has_offset {
                    cfg = cfg.offset(dx as f32, dy as f32);
                }
                if r.u()? == 1 {
                    cfg = cfg.fit();
                }
                out.spec = std::mem::take(&mut out.spec).float(cfg);
            }
            P_KEY_FOCUS => out.key_focus = true,
            P_KEY => out.key = Some(r.req_str()?.to_string()),
            P_TITLE => out.title = Some(r.req_str()?.to_string()),
            P_TOOLTIP => {
                let hint = r.req_str()?.to_string();
                // The hint is the accessible description too.
                out.with_spec(|s| s.hoverable().description(hint.as_str()));
                out.tooltip = Some(hint);
            }
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
            if let Some(hint) = &p.tooltip
                && core.is_hovered(node_key)
            {
                widgets::tooltip(&mut kui_core::Ui::wrap(core), hint);
            }
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
            core.open_keyed(label_key, widgets::button_spec().on_click(msg));
            core.text_node(
                label,
                TextStyle::new(widgets::BUTTON_TEXT).color(Color::WHITE),
            );
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
        // key?, src (hi, lo), flags (1 loop | 2 paused), volume (-1 =
        // absent), tag JSON? — mirrors the JSON path's `<audio>` arm.
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
                tag,
            );
            match key {
                Some(label) => core.audio_node_keyed(label, spec),
                None => core.audio_node(spec),
            };
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

#[cfg(test)]
mod tests {
    use super::*;
    use kui_core::schema::{CUSTOM, PROPS};
    use kui_core::{Align, Size, Sizing, Value};

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
                    s.extend([1.0, 2.0, 3.0, 4.0]);
                    expected.with_spec(|x| {
                        x.padding(Edges {
                            l: 1.0,
                            r: 2.0,
                            t: 3.0,
                            b: 4.0,
                        })
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
                    // viewport, at=(end,end), self=(end,end), offset(-8,-8), fit
                    s.extend([1.0, 1.0, 2.0, 2.0, 1.0, 2.0, 2.0, 1.0, -8.0, -8.0, 1.0]);
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
                "title" => {
                    s.extend([0.0, 3.0]);
                    strings = b"abc";
                    expected.title = Some("abc".into());
                }
                "tooltip" => {
                    s.extend([0.0, 3.0]);
                    strings = b"abc";
                    expected.tooltip = Some("abc".into());
                    expected.with_spec(|x| x.hoverable().description("abc"));
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
            0.0,
            0.0, // no offset
            0.0, // fit
        ];
        let mut expected = PropsOut::new();
        expected.with_spec(|x| x.float(FloatConfig::below()));
        assert_eq!(decode(&s, b""), expected);
    }

    /// A whole frame lowers headlessly, and the version/root guards hold.
    #[test]
    fn lower_binary_smoke_and_guards() {
        let mut core = Core::new();
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
        lower_binary(&mut core, &stream, b"hi").unwrap();
        core.finish_frame();
        assert!(!core.output().0.quads.is_empty(), "root bg draws a quad");

        let stale = [v + 1.0, OP_ROOT as f64, 0.0, OP_END as f64];
        assert!(lower_binary(&mut core, &stale, b"").is_err());
        assert!(lower_binary(&mut core, &[v, OP_END as f64], b"").is_err());
        assert!(lower_binary(&mut core, &[v, OP_ROOT as f64], b"").is_err());
    }
}
