//! Plain geometry in logical pixels: [`Vec2`], [`Size`], [`Rect`] and
//! [`Edges`].
//!
//! Every number here is a logical pixel, before the window's scale factor
//! is applied; the display list and the renderer work in physical pixels.
//! The types are `#[repr(C)]` and `Copy`, so they cross the FFI boundary
//! unchanged.

/// A point or offset in logical pixels.
///
/// ```rust
/// use kui_core::Vec2;
/// let p = Vec2::new(10.0, 4.0).plus(Vec2::new(2.0, 1.0));
/// assert_eq!((p.x, p.y), (12.0, 5.0));
/// ```
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const ZERO: Vec2 = Vec2 { x: 0.0, y: 0.0 };

    /// `{x, y}` — an offset as a readback spells it.
    pub fn to_value(self) -> crate::value::Value {
        use crate::value::Value;
        Value::map([("x", Value::float(self.x)), ("y", Value::float(self.y))])
    }

    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Component-wise sum.
    pub fn plus(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x + o.x, self.y + o.y)
    }

    /// Component-wise difference.
    pub fn minus(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x - o.x, self.y - o.y)
    }

    /// This displacement rounded to a whole number of physical pixels.
    ///
    /// Every offset a *subtree* is moved by goes through here — a `slide`,
    /// an `enter`/`exit` offset, a scroll — because glyphs are placed at
    /// whole physical pixels (one raster per glyph, `text::emit`) and their
    /// box is not. A fractional displacement moves the two by different
    /// amounts, so text wobbles ±0.5 px inside its own background for as
    /// long as the motion lasts; a whole one moves them together. Where a
    /// node sits when it is *still* is untouched: this rounds the offset,
    /// not the position, so a card laid out at a fractional x stays there
    /// and its text keeps the gap it had.
    pub(crate) fn snapped(self, scale: f32) -> Self {
        if scale <= 0.0 || !scale.is_finite() {
            return self;
        }
        Self::new(
            snap_px(self.x * scale) / scale,
            snap_px(self.y * scale) / scale,
        )
    }
}

/// A physical coordinate put on the pixel grid — where a run of glyphs or a
/// cell grid is placed, so one raster serves every frame.
///
/// `floor(v + 0.5)` and not `v.round()`, because this has to survive being
/// *moved*: `round` breaks a .5 tie away from zero, so text sitting at
/// exactly x.5 jumps a whole pixel the moment it crosses the origin, which
/// is the wobble [`Vec2::snapped`] removes coming back at one line on the
/// screen. This one obeys `snap_px(v + k) == snap_px(v) + k` for every
/// whole `k`, which is the property that makes a snapped displacement move
/// a box and its text by the same amount.
///
/// The bias is what makes that property survive floating point. At 150%
/// every other whole logical pixel *is* a half physical one, so exact ties
/// are ordinary here, not a corner — and a snapped displacement reaches
/// this through a divide by the scale and a multiply back, which lands a
/// microscopic hair either side of the tie and picks a different pixel each
/// way. A thousandth of a pixel is three orders above that noise and three
/// below anything a placement could show. (Past ~2^16 physical pixels the
/// float spacing overtakes it again; that is well off any screen.)
pub(crate) fn snap_px(v: f32) -> f32 {
    const TIE: f32 = 1.0 / 1024.0;
    (v + 0.5 + TIE).floor()
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Size {
    pub w: f32,
    pub h: f32,
}

impl Size {
    pub const ZERO: Size = Size { w: 0.0, h: 0.0 };

    pub fn new(w: f32, h: f32) -> Self {
        Self { w, h }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    /// `{x, y, w, h}` — a rect as a readback spells it: a caret, a
    /// scroller's box, a window's anchor.
    pub fn to_value(self) -> crate::value::Value {
        use crate::value::Value;
        Value::map([
            ("x", Value::float(self.x)),
            ("y", Value::float(self.y)),
            ("w", Value::float(self.w)),
            ("h", Value::float(self.h)),
        ])
    }

    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    pub fn from_pos_size(pos: Vec2, size: Size) -> Self {
        Self {
            x: pos.x,
            y: pos.y,
            w: size.w,
            h: size.h,
        }
    }

    /// The point halfway across and halfway down.
    pub fn center(&self) -> Vec2 {
        Vec2 {
            x: self.x + self.w / 2.0,
            y: self.y + self.h / 2.0,
        }
    }

    pub fn contains(&self, p: Vec2) -> bool {
        p.x >= self.x && p.x < self.x + self.w && p.y >= self.y && p.y < self.y + self.h
    }

    pub fn scaled(&self, s: f32) -> Rect {
        Rect {
            x: self.x * s,
            y: self.y * s,
            w: self.w * s,
            h: self.h * s,
        }
    }

    /// This physical rect with each edge snapped to a whole pixel
    /// ([`snap_px`]) on its own, so two rects that share an edge land it
    /// on the same pixel line: `pixelSnap` boxes, and a text's
    /// backgrounds (`text::emit`).
    pub(crate) fn on_pixels(&self) -> Rect {
        let (x0, y0) = (snap_px(self.x), snap_px(self.y));
        let (x1, y1) = (snap_px(self.x + self.w), snap_px(self.y + self.h));
        Rect::new(x0, y0, x1 - x0, y1 - y0)
    }

    /// The smallest rect holding both.
    pub fn union(&self, other: &Rect) -> Rect {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        let r = (self.x + self.w).max(other.x + other.w);
        let b = (self.y + self.h).max(other.y + other.h);
        Rect::new(x, y, r - x, b - y)
    }

    pub fn intersect(&self, other: &Rect) -> Rect {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let r = (self.x + self.w).min(other.x + other.w);
        let b = (self.y + self.h).min(other.y + other.h);
        Rect {
            x,
            y,
            w: (r - x).max(0.0),
            h: (b - y).max(0.0),
        }
    }
}

/// A similarity transform — a turn and a uniform scale about the origin,
/// then a move: `p' = R(angle) · scale · p + t`, y down, so a positive
/// angle turns clockwise on screen. What a node's `rotate`, `scale` and
/// `pivot` compose to (ADR 0043), carried by the clip entry its quads
/// name (`display::Clip::transform`) in the same units as the entry's
/// rect.
///
/// `#[repr(C)]`: four floats, which is how `KuiClip` and the Node
/// `clips()` buffer read it.
///
/// ```rust
/// use kui_core::{Rect, Transform, Vec2};
/// // A quarter turn about the centre of a 100 × 50 box at (10, 10).
/// let t = Transform::about(Vec2::new(60.0, 35.0), 0.25, 1.0);
/// let p = t.apply(Vec2::new(10.0, 10.0));
/// assert!((p.x - 85.0).abs() < 1e-4 && (p.y - (-15.0)).abs() < 1e-4);
/// let back = t.unapply(p);
/// assert!((back.x - 10.0).abs() < 1e-4 && (back.y - 10.0).abs() < 1e-4);
/// // Its bounding box is the box turned: 50 wide, 100 tall, same centre.
/// let b = t.bounds(Rect::new(10.0, 10.0, 100.0, 50.0));
/// assert!((b.w - 50.0).abs() < 1e-3 && (b.h - 100.0).abs() < 1e-3);
/// ```
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    /// Radians, clockwise with y down.
    pub angle: f32,
    /// The uniform factor; 1 for none.
    pub scale: f32,
    /// The move after the turn and the scale.
    pub tx: f32,
    pub ty: f32,
}

impl Default for Transform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Transform {
    /// No turn, no scale, no move.
    pub const IDENTITY: Transform = Transform {
        angle: 0.0,
        scale: 1.0,
        tx: 0.0,
        ty: 0.0,
    };

    /// A turn of `turns` (clockwise, y down) and a scale of `scale` about
    /// `pivot`, which stays where it is.
    pub fn about(pivot: Vec2, turns: f32, scale: f32) -> Self {
        let angle = turns * std::f32::consts::TAU;
        let (s, c) = angle.sin_cos();
        // t = pivot − R·s·pivot
        let rx = (pivot.x * c - pivot.y * s) * scale;
        let ry = (pivot.x * s + pivot.y * c) * scale;
        Transform {
            angle,
            scale,
            tx: pivot.x - rx,
            ty: pivot.y - ry,
        }
    }

    pub fn is_identity(&self) -> bool {
        self.angle == 0.0 && self.scale == 1.0 && self.tx == 0.0 && self.ty == 0.0
    }

    /// `p` through this transform.
    pub fn apply(&self, p: Vec2) -> Vec2 {
        let (s, c) = self.angle.sin_cos();
        let x = p.x * self.scale;
        let y = p.y * self.scale;
        Vec2 {
            x: x * c - y * s + self.tx,
            y: x * s + y * c + self.ty,
        }
    }

    /// The point that maps to `p`: the inverse. A scale of zero has no
    /// inverse; the answer is then the pivot-less origin, which nothing
    /// hits, as nothing is drawn.
    pub fn unapply(&self, p: Vec2) -> Vec2 {
        if self.scale == 0.0 {
            return Vec2::new(f32::NAN, f32::NAN);
        }
        let (s, c) = (-self.angle).sin_cos();
        let x = p.x - self.tx;
        let y = p.y - self.ty;
        Vec2 {
            x: (x * c - y * s) / self.scale,
            y: (x * s + y * c) / self.scale,
        }
    }

    /// This transform, then `outer`: the composition a nested turn is
    /// (`inner.then(outer)` maps a point as `outer.apply(inner.apply(p))`).
    pub fn then(&self, outer: &Transform) -> Transform {
        let t = outer.apply(Vec2::new(self.tx, self.ty));
        Transform {
            angle: self.angle + outer.angle,
            scale: self.scale * outer.scale,
            tx: t.x,
            ty: t.y,
        }
    }

    /// Logical to physical pixels: the move scales, the turn and the
    /// factor do not.
    pub fn scaled(&self, s: f32) -> Transform {
        Transform {
            angle: self.angle,
            scale: self.scale,
            tx: self.tx * s,
            ty: self.ty * s,
        }
    }

    /// The smallest axis-aligned rect holding `r` put through this
    /// transform: what an access rect and a cull read.
    pub fn bounds(&self, r: Rect) -> Rect {
        let corners = [
            self.apply(Vec2::new(r.x, r.y)),
            self.apply(Vec2::new(r.x + r.w, r.y)),
            self.apply(Vec2::new(r.x + r.w, r.y + r.h)),
            self.apply(Vec2::new(r.x, r.y + r.h)),
        ];
        let mut x0 = f32::INFINITY;
        let mut y0 = f32::INFINITY;
        let mut x1 = f32::NEG_INFINITY;
        let mut y1 = f32::NEG_INFINITY;
        for c in corners {
            x0 = x0.min(c.x);
            y0 = y0.min(c.y);
            x1 = x1.max(c.x);
            y1 = y1.max(c.y);
        }
        Rect::new(x0, y0, x1 - x0, y1 - y0)
    }

    /// The bounding box of `r` pulled back through this transform: the
    /// rect in this transform's source space that covers everything of
    /// `r` in its target space — how a clip from outside a turn is
    /// approximated inside it (ADR 0043, decision 4).
    pub fn unbounds(&self, r: Rect) -> Rect {
        if self.scale == 0.0 {
            return Rect::new(0.0, 0.0, 0.0, 0.0);
        }
        let inv = Transform {
            angle: -self.angle,
            scale: 1.0 / self.scale,
            tx: 0.0,
            ty: 0.0,
        };
        let o = inv.apply(Vec2::new(-self.tx, -self.ty));
        Transform {
            tx: o.x,
            ty: o.y,
            ..inv
        }
        .bounds(r)
    }

    /// The four lanes a tween carries for the slot: angle in turns, the
    /// scale, and two spare.
    pub(crate) fn lanes(turns: f32, scale: f32) -> [f32; 4] {
        [turns, scale, 0.0, 0.0]
    }
}

/// Per-side lengths: padding, borders.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Edges {
    pub l: f32,
    pub r: f32,
    pub t: f32,
    pub b: f32,
}

impl Edges {
    pub fn all(v: f32) -> Self {
        Self {
            l: v,
            r: v,
            t: v,
            b: v,
        }
    }

    pub fn xy(x: f32, y: f32) -> Self {
        Self {
            l: x,
            r: x,
            t: y,
            b: y,
        }
    }

    /// Total horizontal extent.
    pub fn x(&self) -> f32 {
        self.l + self.r
    }

    /// Total vertical extent.
    pub fn y(&self) -> f32 {
        self.t + self.b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_whole_pixel_shift_moves_a_snapped_coordinate_by_exactly_that() {
        // The property `Vec2::snapped` relies on: box and text move
        // together only if shifting by a whole pixel shifts the snapped
        // coordinate by the same whole pixel — at a tie, across zero, and
        // through the float noise a logical round trip leaves behind.
        for v in [0.0f32, 0.25, 0.5, 10.5, -0.5, -26.5, 31.5, 7.3, -118.5] {
            for k in [-200.0f32, -1.0, 0.0, 1.0, 3.0, 141.0] {
                assert_eq!(
                    snap_px(v + k),
                    snap_px(v) + k,
                    "snap_px({v}) shifted by {k}"
                );
            }
        }
    }

    #[test]
    fn a_snapped_displacement_is_whole_physical_pixels() {
        for scale in [1.0f32, 1.25, 1.5, 2.0, 3.0] {
            for d in [0.0f32, 0.1, -0.4, 12.34, -99.9] {
                let s = Vec2::new(d, -d).snapped(scale);
                for v in [s.x, s.y] {
                    let px = v * scale;
                    assert!((px - px.round()).abs() < 1e-3, "{v} at {scale} is {px} px");
                }
            }
        }
        // A scale that cannot be divided by is left alone rather than
        // turning a position into a NaN.
        assert_eq!(Vec2::new(1.5, 2.5).snapped(0.0), Vec2::new(1.5, 2.5));
    }
}
