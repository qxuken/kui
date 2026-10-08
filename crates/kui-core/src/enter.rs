//! Entrance transitions: where a node's animatable slots start on the
//! first frame it is seen.
//!
//! Without one, a node's first sight snaps into place. `NodeSpec::enter`
//! takes an [`Enter`] naming where the slots start instead (`dx`/`dy` for
//! the position, plus width, height, bg, radius, opacity, rotate and
//! scale), and they
//! ease from there to what the view declares over the node's
//! `transition`. `NodeSpec::exit` takes the same type read the other way:
//! where the slots end after the view stops declaring the node.
//!
//! ```rust
//! use kui_core::{Core, Enter, NodeSpec, Size};
//!
//! let mut core = Core::new();
//! let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
//! // A toast that rises 24 px and fades in over 200 ms when it first appears,
//! // and does the reverse when the view stops declaring it.
//! ui.leaf_keyed(
//!     "toast",
//!     NodeSpec::row()
//!         .size(200.0, 40.0)
//!         .transition(200.0)
//!         .enter(Enter::from(0.0, 24.0).opacity(0.0))
//!         .exit(Enter::from(0.0, 24.0).opacity(0.0)),
//! );
//! ui.finish();
//! ```
//!
//! `dx`/`dy` move the node's position in place, subtree and all; a node
//! with `enter` but no `slide` eases only its entrance, and a later layout
//! move still snaps. Slots a `keyframes` stop names are sampled, not
//! tweened, so `enter` leaves them alone. A node that leaves and comes
//! back enters again.

use crate::slots::{Slots, slot_builders};
use crate::value::Value;

/// Where a node's slots start on first sight. Every field is optional: a
/// slot `enter` doesn't name simply snaps as it always did. Derefs to its
/// [`Slots`], so `enter.bg` reads the slot.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Enter {
    /// Position offset (logical px) the node eases in from — `dx: -300`
    /// slides in from the left.
    pub dx: f32,
    pub dy: f32,
    /// Width, height, bg, radius, opacity, rotate and scale — the slots a
    /// keyframe stop names too.
    pub slots: Slots,
}

slot_builders!(Enter);

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

    /// Whether the entrance moves the node's position.
    pub fn offsets(&self) -> bool {
        self.dx != 0.0 || self.dy != 0.0
    }
}

/// An entrance from plain data: a map with any of `dx`, `dy`, `width`,
/// `height`, `bg`, `radius`, `opacity`, `rotate`, `scale`, in the forms the props themselves take (the
/// same shapes a keyframe stop accepts). Every binding funnels `enter`
/// through here, so the shape is the same in JSX, Lua and C.
pub fn parse(v: &Value) -> Result<Enter, String> {
    parse_with(v, None)
}

/// [`parse`] with a token lookup, as `keyframes::parse_with`: a `$name`
/// in `width`, `height`, `bg` or `radius` resolves, and a miss leaves the
/// slot unnamed and is remembered on the refs.
pub fn parse_with(
    v: &Value,
    mut refs: Option<&mut crate::tokens::NameRefs<'_>>,
) -> Result<Enter, String> {
    let Value::Map(fields) = v else {
        return Err("enter must be an object".into());
    };
    let mut e = Enter::default();
    for (k, v) in fields {
        let bad = |what: &str| format!("enter: {what}");
        if e.slots
            .parse_field(k, v, refs.as_deref_mut())
            .map_err(|e| bad(&e))?
        {
            continue;
        }
        let num = |what: &str| {
            v.as_float()
                .map(|n| n as f32)
                .ok_or_else(|| bad(&format!("{what} must be a number")))
        };
        match k.as_str() {
            "dx" => e.dx = num("dx")?,
            "dy" => e.dy = num("dy")?,
            other => return Err(bad(&format!("unknown field {other:?}"))),
        }
    }
    Ok(e)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Color;
    use crate::spec::Sizing;

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
