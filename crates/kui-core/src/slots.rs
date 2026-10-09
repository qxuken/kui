//! The animatable slots an entrance or a keyframe stop may name (width,
//! height, bg, radius, opacity, rotate, scale) as one value.
//!
//! You rarely build a [`Slots`] directly: [`crate::enter::Enter`] (where
//! a node starts on first sight) and [`crate::keyframes::Keyframe`] (a
//! stop in a cycle) are this plus one field each, carry the same builders,
//! and deref to it, so `enter.bg` reads the slot.
//!
//! ```rust
//! use kui_core::{Color, Enter};
//!
//! let enter = Enter::from(-40.0, 0.0).bg(Color::WHITE).opacity(0.0);
//! assert_eq!(enter.opacity, Some(0.0));
//! assert_eq!(enter.bg, Some(Color::WHITE));
//! assert!(enter.width.is_none());
//! ```

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
    /// The node's turn, in turns clockwise (ADR 0043).
    pub rotate: Option<f32>,
    /// The node's uniform scale about its pivot.
    pub scale: Option<f32>,
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

    pub fn rotate(mut self, turns: f32) -> Self {
        self.rotate = Some(turns);
        self
    }

    pub fn scale(mut self, scale: f32) -> Self {
        self.scale = Some(scale);
        self
    }

    /// Reads one field of a stop or an entrance from plain data, in the
    /// form the prop itself takes (sizings as a number, `"grow"`, `"50%"`,
    /// `{grow}` / `{percent}`; colours as `0xRRGGBBAA` or `"#hex"`).
    /// `Ok(false)` when `name` is none of the seven, so the caller can read
    /// its own fields after. Every binding funnels through here, so the
    /// shape is the same in JSX, Lua and C. With `refs`, a `$name` in a
    /// colour or length slot resolves through it, and one that misses
    /// leaves the slot unnamed, remembered on the refs.
    pub(crate) fn parse_field(
        &mut self,
        name: &str,
        v: &Value,
        refs: Option<&mut crate::tokens::NameRefs<'_>>,
    ) -> Result<bool, String> {
        let num = |what: &str| {
            v.as_float()
                .map(|n| n as f32)
                .ok_or_else(|| format!("{what} must be a number"))
        };
        if let Some(refs) = refs {
            let hit = match name {
                "width" => refs
                    .length_ref(v)
                    .map(|px| self.width = px.map(Sizing::Fixed)),
                "height" => refs
                    .length_ref(v)
                    .map(|px| self.height = px.map(Sizing::Fixed)),
                "bg" => refs.color_ref(v).map(|c| self.bg = c),
                "radius" => refs.length_ref(v).map(|r| self.radius = r),
                _ => None,
            };
            if hit.is_some() {
                return Ok(true);
            }
        }
        match name {
            // An expression the full table refused leaves the slot
            // unset, as a prop is left undeclared.
            "width" => self.width = kept(sizing_value(v))?,
            "height" => self.height = kept(sizing_value(v))?,
            "bg" => self.bg = Some(color_value(v)?),
            "radius" => self.radius = Some(num("radius")?),
            "opacity" => self.opacity = Some(num("opacity")?.clamp(0.0, 1.0)),
            "rotate" => self.rotate = Some(num("rotate")?),
            "scale" => self.scale = Some(num("scale")?),
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
            // A lane left out is the node's own, which the caller knows
            // and this does not: `transform_lanes`.
            Slot::Transform => None,
            Slot::Border | Slot::Pos | Slot::Shadow | Slot::ShadowColor => None,
        }
    }

    /// The transform slot's lanes — `[rotate, scale, 0, 0]` — when the
    /// stop or the entrance names either, each lane it leaves out taken
    /// from `base`, the node's own value: a stop that names only `rotate`
    /// does not shrink the box to a scale of nothing.
    #[inline]
    pub(crate) fn transform_lanes(&self, base: [f32; 4]) -> Option<[f32; 4]> {
        if self.rotate.is_none() && self.scale.is_none() {
            return None;
        }
        // A value that is not a finite number is the base's, as unnamed.
        let pick = |v: Option<f32>, b: f32| v.filter(|v| v.is_finite()).unwrap_or(b);
        Some([
            pick(self.rotate, base[0]),
            pick(self.scale, base[1]),
            0.0,
            0.0,
        ])
    }
}

/// A scalar in a tween's four lanes: the value, then nothing.
#[inline]
pub(crate) fn one(v: f32) -> [f32; 4] {
    [v, 0.0, 0.0, 0.0]
}

/// The seven delegating builders on a type with a `slots: Slots` field, so
/// `Enter::from(..).bg(..)` and `Keyframe::default().at(..).bg(..)` read
/// the same and are written once.
macro_rules! slot_builders {
    ($ty:ty) => {
        impl $ty {
            /// A [`crate::spec::Sizing`], or a number of px, as on a spec.
            #[inline]
            pub fn width(mut self, width: impl Into<crate::spec::Sizing>) -> Self {
                self.slots = self.slots.width(width.into());
                self
            }

            /// A [`crate::spec::Sizing`], or a number of px, as on a spec.
            #[inline]
            pub fn height(mut self, height: impl Into<crate::spec::Sizing>) -> Self {
                self.slots = self.slots.height(height.into());
                self
            }

            /// `width(Sizing::GROW)`, as on a spec.
            #[inline]
            pub fn grow_width(self) -> Self {
                self.width(crate::spec::Sizing::GROW)
            }

            /// `height(Sizing::GROW)`, as on a spec.
            #[inline]
            pub fn grow_height(self) -> Self {
                self.height(crate::spec::Sizing::GROW)
            }

            pub fn bg(mut self, bg: crate::color::Color) -> Self {
                self.slots = self.slots.bg(bg);
                self
            }

            pub fn radius(mut self, radius: f32) -> Self {
                self.slots = self.slots.radius(radius);
                self
            }

            /// A turn in turns clockwise (ADR 0043), as on a spec.
            pub fn rotate(mut self, turns: f32) -> Self {
                self.slots = self.slots.rotate(turns);
                self
            }

            /// A uniform scale about the node's pivot, as on a spec.
            pub fn scale(mut self, scale: f32) -> Self {
                self.slots = self.slots.scale(scale);
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

/// `Some` of what parsed, `None` for a size expression the full table
/// refused ([`crate::calc::is_full`]), the error otherwise.
fn kept<T>(r: Result<T, String>) -> Result<Option<T>, String> {
    match r {
        Ok(v) => Ok(Some(v)),
        Err(e) if crate::calc::is_full(&e) => Ok(None),
        Err(e) => Err(e),
    }
}

/// A sizing from plain data, in the forms the prop takes.
pub(crate) fn sizing_value(v: &Value) -> Result<Sizing, String> {
    match v {
        Value::Int(_) | Value::Float(_) => Ok(Sizing::Fixed(v.as_float().unwrap_or(0.0) as f32)),
        Value::Str(s) => crate::schema::sizing_str(s),
        Value::Map(_) => {
            if let Some(g) = v.get_float("grow") {
                Ok(Sizing::Grow(g as f32))
            } else if let Some(p) = v.get_float("percent") {
                // JS's spelling, as `"50%"` and a size expression's
                // `{ percent: 50 }` read it.
                Ok(Sizing::Percent(p as f32 / 100.0))
            } else if let Some(p) = v.get_float("pct") {
                // The Lua spelling.
                Ok(Sizing::Percent(p as f32 / 100.0))
            } else {
                // A size expression as data.
                crate::calc::sizing_value(v).map_err(|e| {
                    format!("sizing object needs grow, percent or a size expression: {e}")
                })
            }
        }
        _ => Err("bad sizing (fit | grow | number | \"N%\" | a size expression)".into()),
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
