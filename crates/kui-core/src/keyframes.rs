//! Keyframes: CSS `@keyframes` for a node's animatable slots. A node with
//! `NodeSpec::keyframes` cycles those slots through the stops over its
//! transition's duration, in the transition's `repeat` direction, held
//! back by its `delay_ms` — CSS's `animation-*` family on a kui node.
//!
//! The rules are CSS's (and the Web Animations API's) where they had one:
//! a stop's `at` is optional and missing ones spread evenly between their
//! neighbours (the last defaults to 1, the first to 0 — a lone stop is the
//! far end and animates from the node's own value); a slot a stop
//! leaves out is simply not part of that stop; and a slot whose stops don't
//! reach 0 or 1 gets the node's own declared value there, so a single
//! `{ at: 0.5, bg }` stop is a pulse to that color and back.
//!
//! Stops arrive as plain data from the bindings (`parse`), and the runtime
//! flattens them per slot into the tracks [`crate::anim::AnimStore::sample`]
//! walks.

use crate::color::Color;
use crate::schema::{color_hex_str, color_num, sizing_str};
use crate::spec::Sizing;
use crate::value::Value;

/// One stop. Every field is optional: `at` resolves by position, and a
/// slot a stop doesn't name is left to its neighbours.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Keyframe {
    /// Position in the cycle, 0..=1; None spreads evenly.
    pub at: Option<f32>,
    /// Only the amount animates, in the form the node's own `width`
    /// declares (a `fit` width never moves).
    pub width: Option<Sizing>,
    pub height: Option<Sizing>,
    pub bg: Option<Color>,
    /// All four corners.
    pub radius: Option<f32>,
}

impl Keyframe {
    pub fn at(mut self, at: f32) -> Self {
        self.at = Some(at);
        self
    }

    pub fn width(mut self, width: Sizing) -> Self {
        self.width = Some(width);
        self
    }

    pub fn height(mut self, height: Sizing) -> Self {
        self.height = Some(height);
        self
    }

    pub fn bg(mut self, bg: Color) -> Self {
        self.bg = Some(bg);
        self
    }

    pub fn radius(mut self, radius: f32) -> Self {
        self.radius = Some(radius);
        self
    }
}

/// Stops from plain data: a list of maps with any of `at`, `width`,
/// `height`, `bg`, `radius`, in the forms the props themselves take
/// (sizings as a number, `"grow"`, `"50%"`, `{grow}` / `{percent}`;
/// colors as `0xRRGGBBAA` or `"#hex"`). Every binding funnels its
/// keyframes through here, so the shape is the same in JSX, Lua and C.
pub fn parse(v: &Value) -> Result<Vec<Keyframe>, String> {
    let Value::List(stops) = v else {
        return Err("keyframes must be a list of stops".into());
    };
    let mut frames = Vec::with_capacity(stops.len());
    let mut last_at = 0.0f32;
    for (i, stop) in stops.iter().enumerate() {
        let Value::Map(fields) = stop else {
            return Err(format!("keyframe {i} must be an object"));
        };
        let mut kf = Keyframe::default();
        for (k, v) in fields {
            let bad = |what: &str| format!("keyframe {i}: {what}");
            match k.as_str() {
                "at" => {
                    let at = v
                        .as_float()
                        .ok_or_else(|| bad("at must be a number 0..1"))?
                        as f32;
                    if !(0.0..=1.0).contains(&at) {
                        return Err(bad("at must be within 0..1"));
                    }
                    if at < last_at {
                        return Err(bad("at must not decrease"));
                    }
                    last_at = at;
                    kf.at = Some(at);
                }
                "width" => kf.width = Some(sizing_value(v).map_err(|e| bad(&e))?),
                "height" => kf.height = Some(sizing_value(v).map_err(|e| bad(&e))?),
                "bg" => kf.bg = Some(color_value(v).map_err(|e| bad(&e))?),
                "radius" => {
                    kf.radius =
                        Some(v.as_float().ok_or_else(|| bad("radius must be a number"))? as f32)
                }
                other => return Err(bad(&format!("unknown field {other:?}"))),
            }
        }
        frames.push(kf);
    }
    Ok(frames)
}

pub(crate) fn sizing_value(v: &Value) -> Result<Sizing, String> {
    match v {
        Value::Int(_) | Value::Float(_) => Ok(Sizing::Fixed(v.as_float().unwrap_or(0.0) as f32)),
        Value::Str(s) => sizing_str(s),
        Value::Map(_) => {
            if let Some(g) = v.get("grow").and_then(Value::as_float) {
                Ok(Sizing::Grow(g as f32))
            } else if let Some(p) = v.get("percent").and_then(Value::as_float) {
                Ok(Sizing::Percent(p as f32))
            } else if let Some(p) = v.get("pct").and_then(Value::as_float) {
                // The Lua spelling.
                Ok(Sizing::Percent(p as f32 / 100.0))
            } else {
                Err("sizing object needs grow or percent".into())
            }
        }
        _ => Err("bad sizing (fit | grow | number | \"N%\")".into()),
    }
}

pub(crate) fn color_value(v: &Value) -> Result<Color, String> {
    match v {
        Value::Int(n) => Ok(color_num(*n as u32)),
        Value::Float(n) => Ok(color_num(*n as u32)),
        Value::Str(s) => color_hex_str(s),
        _ => Err("color must be a 0xRRGGBBAA number or \"#hex\" string".into()),
    }
}

/// Every stop's position: declared `at`s kept, the rest spread evenly
/// between the nearest declared neighbours (0 and 1 at the ends).
pub fn offsets(frames: &[Keyframe]) -> Vec<f32> {
    let n = frames.len();
    let mut out: Vec<f32> = frames.iter().map(|f| f.at.unwrap_or(f32::NAN)).collect();
    if n == 0 {
        return out;
    }
    // WAAPI's rule: the last stop defaults to 1 and, given company, the
    // first to 0 — so a lone stop sits at 1 and animates from the base.
    if n > 1 && out[0].is_nan() {
        out[0] = 0.0;
    }
    if out[n - 1].is_nan() {
        out[n - 1] = 1.0;
    }
    let mut i = 0;
    while i < n {
        if !out[i].is_nan() {
            i += 1;
            continue;
        }
        // A run of undeclared stops between two declared ones.
        let start = i - 1;
        let mut end = i;
        while out[end].is_nan() {
            end += 1;
        }
        let (a, b) = (out[start], out[end]);
        let span = (end - start) as f32;
        for (j, slot) in out[start + 1..end].iter_mut().enumerate() {
            *slot = a + (b - a) * (j + 1) as f32 / span;
        }
        i = end;
    }
    out
}

/// The stops that name one slot, flattened into a track with the node's
/// declared value (`base`) filling in the ends CSS would synthesize — a
/// [`crate::anim::Track`]. None when no stop names the slot: it isn't
/// keyframed and tweens as usual.
pub(crate) fn track(
    frames: &[Keyframe],
    offsets: &[f32],
    base: [f32; 4],
    pick: impl Fn(&Keyframe) -> Option<[f32; 4]>,
) -> Option<Vec<(f32, [f32; 4])>> {
    let mut out: Vec<(f32, [f32; 4])> = Vec::with_capacity(frames.len() + 2);
    for (f, &at) in frames.iter().zip(offsets) {
        if let Some(v) = pick(f) {
            out.push((at, v));
        }
    }
    if out.is_empty() {
        return None;
    }
    if out[0].0 > 0.0 {
        out.insert(0, (0.0, base));
    }
    if out[out.len() - 1].0 < 1.0 {
        out.push((1.0, base));
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stop(fields: &[(&'static str, Value)]) -> Value {
        Value::map(fields.iter().cloned())
    }

    fn list(stops: Vec<Value>) -> Value {
        Value::List(stops)
    }

    #[test]
    fn offsets_spread_evenly_between_declared_stops() {
        let f = vec![Keyframe::default(); 3];
        assert_eq!(offsets(&f), vec![0.0, 0.5, 1.0]);
        let f = vec![
            Keyframe::default().at(0.2),
            Keyframe::default(),
            Keyframe::default(),
            Keyframe::default().at(0.8),
            Keyframe::default(),
        ];
        let o = offsets(&f);
        assert!(
            (o[1] - 0.4).abs() < 1e-6 && (o[2] - 0.6).abs() < 1e-6,
            "{o:?}"
        );
        assert_eq!(o[4], 1.0);
        assert_eq!(offsets(&[Keyframe::default().at(0.5)]), vec![0.5]);
        assert_eq!(
            offsets(&[Keyframe::default()]),
            vec![1.0],
            "a lone stop is the far end"
        );
        assert!(offsets(&[]).is_empty());
    }

    #[test]
    fn tracks_fill_the_ends_with_the_base_value() {
        let f = [Keyframe::default().at(0.5).radius(8.0)];
        let o = offsets(&f);
        let t = track(&f, &o, [2.0; 4], |k| k.radius.map(|r| [r; 4])).unwrap();
        assert_eq!(t, vec![(0.0, [2.0; 4]), (0.5, [8.0; 4]), (1.0, [2.0; 4])]);
        assert!(track(&f, &o, [0.0; 4], |k| k.bg.map(|_| [0.0; 4])).is_none());
    }

    #[test]
    fn parses_prop_shaped_values() {
        let f = parse(&list(vec![
            stop(&[("width", Value::map([("grow", Value::Int(0))]))]),
            stop(&[
                ("at", Value::Int(1)),
                ("width", Value::str("grow")),
                ("bg", Value::str("#ff0000")),
                ("radius", Value::Int(3)),
            ]),
        ]))
        .unwrap();
        assert_eq!(f[0], Keyframe::default().width(Sizing::Grow(0.0)));
        assert_eq!(
            f[1],
            Keyframe::default()
                .at(1.0)
                .width(Sizing::Grow(1.0))
                .bg(Color::hex(0xff0000ff))
                .radius(3.0)
        );
        let f = parse(&list(vec![stop(&[
            ("height", Value::str("50%")),
            ("bg", Value::Int(0xffffffff)),
        ])]))
        .unwrap();
        assert_eq!(f[0].height, Some(Sizing::Percent(0.5)));
        assert_eq!(f[0].bg, Some(Color::hex(0xffffffff)));
        // The Lua percent spelling.
        let f = parse(&list(vec![stop(&[(
            "width",
            Value::map([("pct", Value::Int(25))]),
        )])]))
        .unwrap();
        assert_eq!(f[0].width, Some(Sizing::Percent(0.25)));
    }

    #[test]
    fn rejects_what_css_would() {
        let bad = |v: Value| parse(&v).unwrap_err();
        assert!(bad(stop(&[("at", Value::Int(0))])).contains("list"));
        assert!(
            bad(list(vec![
                stop(&[("at", Value::Float(0.5))]),
                stop(&[("at", Value::Float(0.2))])
            ]))
            .contains("decrease")
        );
        assert!(bad(list(vec![stop(&[("at", Value::Int(2))])])).contains("within"));
        assert!(bad(list(vec![stop(&[("colour", Value::str("#fff"))])])).contains("unknown field"));
        assert!(bad(list(vec![stop(&[("width", Value::str("wide"))])])).contains("bad sizing"));
    }
}
