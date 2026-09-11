//! The animatable slots an entrance or a keyframe stop may name — width,
//! height, bg, radius, opacity — as one value. [`crate::enter::Enter`]
//! (where a node starts on first sight) and [`crate::keyframes::Keyframe`]
//! (a stop in a cycle) are this plus one field each; before this module the
//! five slots were two structs with the same builders and the same parse
//! arms, and every consumer packed them into a tween's four lanes by hand.
//! A sixth animatable slot is one field here, one arm in `parse_field`,
//! one case in `lanes` — and nothing in the two structs that carry it.

use crate::anim::Slot;
use crate::color::Color;
use crate::spec::Sizing;
use crate::value::Value;

/// The slots, each optional: one left out is not part of the entrance or
/// the stop, and keeps whatever the node itself declares.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Slots {
    /// Only the amount animates, in the form the node's own `width`
    /// declares (a `fit` width never moves).
    pub width: Option<Sizing>,
    pub height: Option<Sizing>,
    pub bg: Option<Color>,
    /// All four corners.
    pub radius: Option<f32>,
    /// Group opacity, 0..=1.
    pub opacity: Option<f32>,
}

impl Slots {
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

    pub fn opacity(mut self, opacity: f32) -> Self {
        self.opacity = Some(opacity.clamp(0.0, 1.0));
        self
    }

    /// Reads one field of a stop or an entrance from plain data, in the
    /// form the prop itself takes (sizings as a number, `"grow"`, `"50%"`,
    /// `{grow}` / `{percent}`; colours as `0xRRGGBBAA` or `"#hex"`).
    /// `Ok(false)` when `name` is none of the five, so the caller can read
    /// its own fields after. Every binding funnels through here, so the
    /// shape is the same in JSX, Lua and C.
    pub(crate) fn parse_field(&mut self, name: &str, v: &Value) -> Result<bool, String> {
        let num = |what: &str| {
            v.as_float()
                .map(|n| n as f32)
                .ok_or_else(|| format!("{what} must be a number"))
        };
        match name {
            "width" => self.width = Some(sizing_value(v)?),
            "height" => self.height = Some(sizing_value(v)?),
            "bg" => self.bg = Some(color_value(v)?),
            "radius" => self.radius = Some(num("radius")?),
            "opacity" => self.opacity = Some(num("opacity")?.clamp(0.0, 1.0)),
            _ => return Ok(false),
        }
        Ok(true)
    }

    /// The slot's value as the four lanes a tween carries, or `None` when
    /// the slot is not named — or is a sizing with no amount to animate.
    /// The slots a transition tweens that no entrance or stop can name
    /// (border, position, shadow) answer `None` too. Inlined: it is asked
    /// once per slot per transitioning node per frame.
    #[inline]
    pub(crate) fn lanes(&self, slot: Slot) -> Option<[f32; 4]> {
        match slot {
            Slot::Width => self.width.and_then(Sizing::amount).map(one),
            Slot::Height => self.height.and_then(Sizing::amount).map(one),
            Slot::Bg => self.bg.map(Color::lanes),
            Slot::Radius => self.radius.map(|r| [r; 4]),
            Slot::Opacity => self.opacity.map(one),
            Slot::Border | Slot::Pos | Slot::Shadow | Slot::ShadowColor => None,
        }
    }
}

/// A scalar in a tween's four lanes: the value, then nothing.
#[inline]
pub(crate) fn one(v: f32) -> [f32; 4] {
    [v, 0.0, 0.0, 0.0]
}

/// The five delegating builders on a type with a `slots: Slots` field, so
/// `Enter::from(..).bg(..)` and `Keyframe::default().at(..).bg(..)` read
/// the same and are written once.
macro_rules! slot_builders {
    ($ty:ty) => {
        impl $ty {
            pub fn width(mut self, width: crate::spec::Sizing) -> Self {
                self.slots = self.slots.width(width);
                self
            }

            pub fn height(mut self, height: crate::spec::Sizing) -> Self {
                self.slots = self.slots.height(height);
                self
            }

            pub fn bg(mut self, bg: crate::color::Color) -> Self {
                self.slots = self.slots.bg(bg);
                self
            }

            pub fn radius(mut self, radius: f32) -> Self {
                self.slots = self.slots.radius(radius);
                self
            }

            pub fn opacity(mut self, opacity: f32) -> Self {
                self.slots = self.slots.opacity(opacity);
                self
            }
        }

        impl std::ops::Deref for $ty {
            type Target = crate::slots::Slots;
            fn deref(&self) -> &crate::slots::Slots {
                &self.slots
            }
        }

        impl std::ops::DerefMut for $ty {
            fn deref_mut(&mut self) -> &mut crate::slots::Slots {
                &mut self.slots
            }
        }
    };
}
pub(crate) use slot_builders;

/// A sizing from plain data, in the forms the prop takes.
pub(crate) fn sizing_value(v: &Value) -> Result<Sizing, String> {
    match v {
        Value::Int(_) | Value::Float(_) => Ok(Sizing::Fixed(v.as_float().unwrap_or(0.0) as f32)),
        Value::Str(s) => crate::schema::sizing_str(s),
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

/// A colour from plain data: `0xRRGGBBAA` or `"#hex"`.
pub(crate) fn color_value(v: &Value) -> Result<Color, String> {
    match v {
        Value::Int(n) => Ok(crate::schema::color_num(*n as u32)),
        Value::Float(n) => Ok(crate::schema::color_num(*n as u32)),
        Value::Str(s) => crate::schema::color_hex_str(s),
        _ => Err("color must be a 0xRRGGBBAA number or \"#hex\" string".into()),
    }
}
