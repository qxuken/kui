//! Entrance transitions: where a node's animatable slots start from the
//! first frame it is seen. A transition never animates in from nowhere —
//! a node's first sight snaps, so a view that wants a slide-in used to draw
//! the node off screen for a frame and move it on the next. `NodeSpec::enter`
//! states that starting point as data instead: on first sight the slots it
//! names (`dx`/`dy` for the laid-out position, plus width, height, bg and
//! radius in the forms the props themselves take) start there and ease to
//! what the view declares, on the node's `transition`.
//!
//! `dx`/`dy` move the node's position in place, subtree and all, like
//! `slide` does for reordered siblings; a node with `enter` but no `slide`
//! eases only its entrance — a later layout move still snaps. Slots a
//! `keyframes` stop names are sampled, not tweened, so `enter` leaves them
//! alone. A node that leaves and comes back enters again (the core keeps no
//! memory of a node it did not draw last frame, which is what a dismissed
//! and re-shown toast wants).

use crate::color::Color;
use crate::keyframes::{color_value, sizing_value};
use crate::spec::Sizing;
use crate::value::Value;

/// Where a node's slots start on first sight. Every field is optional: a
/// slot `enter` doesn't name simply snaps as it always did.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Enter {
    /// Position offset (logical px) the node eases in from — `dx: -300`
    /// slides in from the left.
    pub dx: f32,
    pub dy: f32,
    /// Only the amount animates, in the form the node's own `width`
    /// declares (a `fit` width never moves).
    pub width: Option<Sizing>,
    pub height: Option<Sizing>,
    pub bg: Option<Color>,
    /// All four corners.
    pub radius: Option<f32>,
    /// Group opacity: `0` is the fade-in a whole panel wants.
    pub opacity: Option<f32>,
}

impl Enter {
    /// Slide in from `dx`/`dy` px away.
    pub fn from(dx: f32, dy: f32) -> Self {
        Self {
            dx,
            dy,
            ..Self::default()
        }
    }

    pub fn offset(mut self, dx: f32, dy: f32) -> Self {
        self.dx = dx;
        self.dy = dy;
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

    pub fn opacity(mut self, opacity: f32) -> Self {
        self.opacity = Some(opacity.clamp(0.0, 1.0));
        self
    }

    /// Whether the entrance moves the node's position.
    pub fn offsets(&self) -> bool {
        self.dx != 0.0 || self.dy != 0.0
    }
}

/// An entrance from plain data: a map with any of `dx`, `dy`, `width`,
/// `height`, `bg`, `radius`, `opacity`, in the forms the props themselves take (the
/// same shapes a keyframe stop accepts). Every binding funnels `enter`
/// through here, so the shape is the same in JSX, Lua and C.
pub fn parse(v: &Value) -> Result<Enter, String> {
    let Value::Map(fields) = v else {
        return Err("enter must be an object".into());
    };
    let mut e = Enter::default();
    for (k, v) in fields {
        let bad = |what: &str| format!("enter: {what}");
        let num = |what: &str| {
            v.as_float()
                .map(|n| n as f32)
                .ok_or_else(|| bad(&format!("{what} must be a number")))
        };
        match k.as_str() {
            "dx" => e.dx = num("dx")?,
            "dy" => e.dy = num("dy")?,
            "width" => e.width = Some(sizing_value(v).map_err(|e| bad(&e))?),
            "height" => e.height = Some(sizing_value(v).map_err(|e| bad(&e))?),
            "bg" => e.bg = Some(color_value(v).map_err(|e| bad(&e))?),
            "radius" => e.radius = Some(num("radius")?),
            "opacity" => e.opacity = Some(num("opacity")?.clamp(0.0, 1.0)),
            other => return Err(bad(&format!("unknown field {other:?}"))),
        }
    }
    Ok(e)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_prop_shaped_values() {
        let e = parse(&Value::map([
            ("dx", Value::Int(-40)),
            ("dy", Value::Float(2.5)),
            ("width", Value::map([("grow", Value::Int(0))])),
            ("height", Value::str("50%")),
            ("bg", Value::str("#ff000000")),
            ("radius", Value::Int(3)),
            ("opacity", Value::Float(0.0)),
        ]))
        .unwrap();
        assert_eq!(
            e,
            Enter::from(-40.0, 2.5)
                .width(Sizing::Grow(0.0))
                .height(Sizing::Percent(0.5))
                .bg(Color::hex(0xff000000))
                .radius(3.0)
                .opacity(0.0)
        );
        assert!(e.offsets());
        assert!(!Enter::default().bg(Color::WHITE).offsets());
    }

    #[test]
    fn rejects_bad_shapes() {
        let bad = |v: Value| parse(&v).unwrap_err();
        assert!(bad(Value::List(vec![])).contains("object"));
        assert!(bad(Value::map([("dx", Value::str("far"))])).contains("dx must be a number"));
        assert!(bad(Value::map([("colour", Value::str("#fff"))])).contains("unknown field"));
        assert!(bad(Value::map([("width", Value::str("wide"))])).contains("bad sizing"));
    }
}
