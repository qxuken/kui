//! Paths: what a `path` node draws (`docs/adr/0040-a-path-is-a-mask-in-the-atlas.md`).
//!
//! A path is a list of [`PathOp`]s — SVG's `d` with every coordinate
//! absolute and `H`, `V`, `S`, `T` and the relative forms expanded by
//! [`Path::parse`], the one parser every binding goes through — filled by
//! a rule and, optionally, stroked. The core flattens it once per frame
//! for the hit outline, rasterizes it through zeno (swash's rasterizer,
//! already in the tree for glyphs) into an alpha mask at the node's
//! physical scale, keeps the mask in the glyph atlas keyed on a hash of
//! the ops, and draws it as a `GlyphMask` quad tinted by the fill or the
//! stroke colour. The ops live in a per-frame list beside the tree, as a
//! line's points do, and a node refers to its run by [`PathId`].

use crate::geom::{Rect, Vec2};
use crate::retain::Kept;

/// Index into the frame's path list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PathId(pub u32);

/// One drawing command, every coordinate absolute, in the space the
/// path was declared in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PathOp {
    /// Begins a subpath at the point.
    MoveTo(Vec2),
    /// A straight line to the point.
    LineTo(Vec2),
    /// A quadratic curve through one control point to the end point.
    QuadTo(Vec2, Vec2),
    /// A cubic curve through two control points to the end point.
    CubicTo(Vec2, Vec2, Vec2),
    /// An elliptical arc as SVG spells it: the radii, the ellipse's
    /// rotation in degrees, the large-arc and sweep flags, the end point.
    ArcTo {
        rx: f32,
        ry: f32,
        rotation: f32,
        large: bool,
        sweep: bool,
        to: Vec2,
    },
    /// Closes the subpath back to its start.
    Close,
}

/// The op codes of the flat wire form: a code, then its operands.
pub const OP_MOVE: f32 = 0.0;
pub const OP_LINE: f32 = 1.0;
pub const OP_QUAD: f32 = 2.0;
pub const OP_CUBIC: f32 = 3.0;
pub const OP_ARC: f32 = 4.0;
pub const OP_CLOSE: f32 = 5.0;

impl PathOp {
    /// How many floats follow the code of op `code` on the wire; `None`
    /// for a code that is not one.
    pub fn operands(code: f32) -> Option<usize> {
        // A whole number or nothing: `as` would read 0.9, -0.5 and a NaN
        // as 0, a move.
        if code.fract() != 0.0 {
            return None;
        }
        match code as i32 {
            0 | 1 => Some(2),
            2 => Some(4),
            3 => Some(6),
            4 => Some(7),
            5 => Some(0),
            _ => None,
        }
    }

    /// Whether every number of the op is one: no NaN, no infinity.
    pub fn is_finite(&self) -> bool {
        let ok = |p: Vec2| p.x.is_finite() && p.y.is_finite();
        match *self {
            PathOp::MoveTo(p) | PathOp::LineTo(p) => ok(p),
            PathOp::QuadTo(c, p) => ok(c) && ok(p),
            PathOp::CubicTo(a, b, p) => ok(a) && ok(b) && ok(p),
            PathOp::ArcTo {
                rx,
                ry,
                rotation,
                to,
                ..
            } => rx.is_finite() && ry.is_finite() && rotation.is_finite() && ok(to),
            PathOp::Close => true,
        }
    }

    /// Appends the op's wire form: the code and its operands.
    pub fn write(&self, out: &mut Vec<f32>) {
        match *self {
            PathOp::MoveTo(p) => out.extend_from_slice(&[OP_MOVE, p.x, p.y]),
            PathOp::LineTo(p) => out.extend_from_slice(&[OP_LINE, p.x, p.y]),
            PathOp::QuadTo(c, p) => out.extend_from_slice(&[OP_QUAD, c.x, c.y, p.x, p.y]),
            PathOp::CubicTo(a, b, p) => {
                out.extend_from_slice(&[OP_CUBIC, a.x, a.y, b.x, b.y, p.x, p.y])
            }
            PathOp::ArcTo {
                rx,
                ry,
                rotation,
                large,
                sweep,
                to,
            } => out.extend_from_slice(&[
                OP_ARC,
                rx,
                ry,
                rotation,
                f32::from(large),
                f32::from(sweep),
                to.x,
                to.y,
            ]),
            PathOp::Close => out.push(OP_CLOSE),
        }
    }

    /// The op in a mask's physical px: every point at `scale` from
    /// `off`, the radii at `scale`.
    fn placed(&self, scale: f32, off: Vec2) -> PathOp {
        let s = |p: Vec2| Vec2::new(p.x * scale + off.x, p.y * scale + off.y);
        match *self {
            PathOp::MoveTo(p) => PathOp::MoveTo(s(p)),
            PathOp::LineTo(p) => PathOp::LineTo(s(p)),
            PathOp::QuadTo(c, p) => PathOp::QuadTo(s(c), s(p)),
            PathOp::CubicTo(a, b, p) => PathOp::CubicTo(s(a), s(b), s(p)),
            PathOp::ArcTo {
                rx,
                ry,
                rotation,
                large,
                sweep,
                to,
            } => PathOp::ArcTo {
                rx: rx * scale,
                ry: ry * scale,
                rotation,
                large,
                sweep,
                to: s(to),
            },
            PathOp::Close => PathOp::Close,
        }
    }

    /// The op with every point moved by `d`.
    fn shifted(&self, d: Vec2) -> PathOp {
        let s = |p: Vec2| Vec2::new(p.x + d.x, p.y + d.y);
        match *self {
            PathOp::MoveTo(p) => PathOp::MoveTo(s(p)),
            PathOp::LineTo(p) => PathOp::LineTo(s(p)),
            PathOp::QuadTo(c, p) => PathOp::QuadTo(s(c), s(p)),
            PathOp::CubicTo(a, b, p) => PathOp::CubicTo(s(a), s(b), s(p)),
            PathOp::ArcTo {
                rx,
                ry,
                rotation,
                large,
                sweep,
                to,
            } => PathOp::ArcTo {
                rx,
                ry,
                rotation,
                large,
                sweep,
                to: s(to),
            },
            PathOp::Close => PathOp::Close,
        }
    }
}

/// How a path's inside is decided: SVG's default, or the polygon's rule.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FillRule {
    /// A point is inside when the outline winds around it a net nonzero
    /// number of times: a self-intersecting outline fills its overlaps.
    #[default]
    NonZero,
    /// A point is inside when an odd number of edges cross a ray from it:
    /// overlaps are unfilled, which is how a ring is a ring.
    EvenOdd,
}

impl FillRule {
    /// The names the bindings spell: `nonzero`, `evenodd`.
    pub const ALL: &[&str] = &["nonzero", "evenodd"];

    pub fn from_index(i: usize) -> Self {
        match i {
            1 => FillRule::EvenOdd,
            _ => FillRule::NonZero,
        }
    }

    pub fn index(self) -> usize {
        match self {
            FillRule::NonZero => 0,
            FillRule::EvenOdd => 1,
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "nonzero" => Some(FillRule::NonZero),
            "evenodd" => Some(FillRule::EvenOdd),
            _ => None,
        }
    }
}

/// A path's turn (`docs/adr/0041-a-mask-turns-about-its-centre.md`): how
/// far it is turned, in turns — clockwise with y down, as
/// [`Path::sector`] counts them — and the point it turns about, in the
/// path's own coordinates; `None` is the centre of the outline's box. A
/// path with a turn is boxed by the square the turn sweeps, so its mask
/// is one mask at every angle and the quad that draws it carries the
/// angle.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Turn {
    pub turns: f32,
    pub pivot: Option<Vec2>,
}

impl Turn {
    /// The angle in radians, clockwise with y down.
    pub fn radians(self) -> f32 {
        self.turns * std::f32::consts::TAU
    }
}

/// `p` turned by `angle` radians (clockwise, y down) about `c`.
pub fn turned(p: Vec2, c: Vec2, angle: f32) -> Vec2 {
    let (sin, cos) = angle.sin_cos();
    let (x, y) = (p.x - c.x, p.y - c.y);
    Vec2::new(c.x + x * cos - y * sin, c.y + x * sin + y * cos)
}

/// Where a `d` string went wrong: the byte offset and what was expected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PathError {
    pub at: usize,
    pub what: &'static str,
}

impl std::fmt::Display for PathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} at byte {}", self.what, self.at)
    }
}

impl std::error::Error for PathError {}

/// A path as an app builds one: the ops, the fill rule, and an optional
/// stroke. The fill colour is the node's `bg`; the stroke's width and
/// colour are the [`crate::line::Stroke`]'s (`curve` is ignored).
#[derive(Clone, Debug, PartialEq)]
pub struct Path {
    ops: Vec<PathOp>,
    rule: FillRule,
    stroke: Option<crate::line::Stroke>,
    turn: Option<Turn>,
}

impl Default for Path {
    fn default() -> Self {
        Self::new()
    }
}

impl Path {
    pub fn new() -> Self {
        Self {
            ops: Vec::new(),
            rule: FillRule::NonZero,
            stroke: None,
            turn: None,
        }
    }

    /// A path over ops already built.
    pub fn from_ops(ops: Vec<PathOp>) -> Self {
        Self {
            ops,
            rule: FillRule::NonZero,
            stroke: None,
            turn: None,
        }
    }

    /// A path from its flat wire form: a code, then its operands, per op.
    pub fn from_floats(f: &[f32]) -> Result<Self, PathError> {
        let mut ops = Vec::new();
        let mut i = 0;
        while i < f.len() {
            let code = f[i];
            let Some(n) = PathOp::operands(code) else {
                return Err(PathError {
                    at: i,
                    what: "an op code 0..=5",
                });
            };
            let Some(a) = f.get(i + 1..i + 1 + n) else {
                return Err(PathError {
                    at: i,
                    what: "the op's operands",
                });
            };
            ops.push(match code as i32 {
                0 => PathOp::MoveTo(Vec2::new(a[0], a[1])),
                1 => PathOp::LineTo(Vec2::new(a[0], a[1])),
                2 => PathOp::QuadTo(Vec2::new(a[0], a[1]), Vec2::new(a[2], a[3])),
                3 => PathOp::CubicTo(
                    Vec2::new(a[0], a[1]),
                    Vec2::new(a[2], a[3]),
                    Vec2::new(a[4], a[5]),
                ),
                4 => PathOp::ArcTo {
                    rx: a[0],
                    ry: a[1],
                    rotation: a[2],
                    large: a[3] != 0.0,
                    sweep: a[4] != 0.0,
                    to: Vec2::new(a[5], a[6]),
                },
                _ => PathOp::Close,
            });
            i += 1 + n;
        }
        Ok(Self::from_ops(ops))
    }

    /// The flat wire form.
    pub fn to_floats(&self) -> Vec<f32> {
        let mut out = Vec::with_capacity(self.ops.len() * 3);
        for op in &self.ops {
            op.write(&mut out);
        }
        out
    }

    pub fn ops(&self) -> &[PathOp] {
        &self.ops
    }

    pub fn into_ops(self) -> Vec<PathOp> {
        self.ops
    }

    pub fn rule(&self) -> FillRule {
        self.rule
    }

    pub fn stroke(&self) -> Option<crate::line::Stroke> {
        self.stroke
    }

    pub fn turn(&self) -> Option<Turn> {
        self.turn
    }

    /// Turns the path by `turns` (clockwise, y down) about its pivot —
    /// the centre of its outline's box unless [`Self::pivot`] names one.
    /// The turn is the quad's and not the mask's: one raster, whatever
    /// the angle.
    pub fn rotated(mut self, turns: f32) -> Self {
        self.turn.get_or_insert_default().turns = turns;
        self
    }

    /// The point the path turns about, in the path's own coordinates. A
    /// spinner's arc names its circle's centre, which is not its own
    /// box's.
    pub fn pivot(mut self, x: f32, y: f32) -> Self {
        self.turn.get_or_insert_default().pivot = Some(Vec2::new(x, y));
        self
    }

    pub fn fill_rule(mut self, rule: FillRule) -> Self {
        self.rule = rule;
        self
    }

    /// Strokes the outline `stroke.width` wide in `stroke.color`, over
    /// the fill. Round joins and caps.
    pub fn stroked(mut self, stroke: crate::line::Stroke) -> Self {
        self.stroke = Some(stroke);
        self
    }

    pub fn move_to(mut self, x: f32, y: f32) -> Self {
        self.ops.push(PathOp::MoveTo(Vec2::new(x, y)));
        self
    }

    pub fn line_to(mut self, x: f32, y: f32) -> Self {
        self.ops.push(PathOp::LineTo(Vec2::new(x, y)));
        self
    }

    pub fn quad_to(mut self, cx: f32, cy: f32, x: f32, y: f32) -> Self {
        self.ops
            .push(PathOp::QuadTo(Vec2::new(cx, cy), Vec2::new(x, y)));
        self
    }

    pub fn cubic_to(mut self, c1x: f32, c1y: f32, c2x: f32, c2y: f32, x: f32, y: f32) -> Self {
        self.ops.push(PathOp::CubicTo(
            Vec2::new(c1x, c1y),
            Vec2::new(c2x, c2y),
            Vec2::new(x, y),
        ));
        self
    }

    /// An elliptical arc to `(x, y)`, as SVG's `A`: radii, the ellipse's
    /// rotation in degrees, whether the larger of the two arcs is taken,
    /// whether it sweeps clockwise (y down).
    #[allow(clippy::too_many_arguments)]
    pub fn arc_to(
        mut self,
        rx: f32,
        ry: f32,
        rotation: f32,
        large: bool,
        sweep: bool,
        x: f32,
        y: f32,
    ) -> Self {
        self.ops.push(PathOp::ArcTo {
            rx,
            ry,
            rotation,
            large,
            sweep,
            to: Vec2::new(x, y),
        });
        self
    }

    pub fn close(mut self) -> Self {
        self.ops.push(PathOp::Close);
        self
    }

    /// A sector of an annulus centred on `(cx, cy)`: from `from` turns
    /// (0 is east, turns run clockwise with y down) sweeping `sweep` turns,
    /// between `inner` and `outer` radii. A pie wedge with `inner` 0, a
    /// donut's segment otherwise, a ring with `sweep` 1.
    pub fn sector(cx: f32, cy: f32, outer: f32, inner: f32, from: f32, sweep: f32) -> Self {
        let tau = std::f32::consts::TAU;
        let sweep = sweep.clamp(-1.0, 1.0);
        let (a0, a1) = (from * tau, (from + sweep) * tau);
        let at = |r: f32, a: f32| (cx + r * a.cos(), cy + r * a.sin());
        let large = sweep.abs() > 0.5;
        let cw = sweep >= 0.0;
        if sweep.abs() >= 1.0 {
            // A full turn is two half arcs, since one arc cannot return to
            // its start.
            let mid = a0 + tau * 0.5;
            let (ox, oy) = at(outer, a0);
            let (mx, my) = at(outer, mid);
            let mut p = Path::new()
                .move_to(ox, oy)
                .arc_to(outer, outer, 0.0, false, cw, mx, my)
                .arc_to(outer, outer, 0.0, false, cw, ox, oy)
                .close();
            if inner > 0.0 {
                let (ix, iy) = at(inner, a0);
                let (jx, jy) = at(inner, mid);
                p = p
                    .move_to(ix, iy)
                    .arc_to(inner, inner, 0.0, false, !cw, jx, jy)
                    .arc_to(inner, inner, 0.0, false, !cw, ix, iy)
                    .close();
            }
            return p;
        }
        let (ox0, oy0) = at(outer, a0);
        let (ox1, oy1) = at(outer, a1);
        let mut p = Path::new()
            .move_to(ox0, oy0)
            .arc_to(outer, outer, 0.0, large, cw, ox1, oy1);
        if inner > 0.0 {
            let (ix1, iy1) = at(inner, a1);
            let (ix0, iy0) = at(inner, a0);
            p = p
                .line_to(ix1, iy1)
                .arc_to(inner, inner, 0.0, large, !cw, ix0, iy0);
        } else {
            p = p.line_to(cx, cy);
        }
        p.close()
    }

    /// Parses SVG path data: `M L H V C S Q T A Z` and their relative
    /// forms, implicit repeats, numbers run together as SVG allows. Every
    /// relative command, `H`, `V`, `S` and `T` is expanded into the six
    /// ops, so what comes out is the same in every binding.
    pub fn parse(d: &str) -> Result<Self, PathError> {
        let b = d.as_bytes();
        let mut i = 0;
        let mut ops = Vec::new();
        let mut cur = Vec2::ZERO;
        let mut start = Vec2::ZERO;
        // The last control point, for `S` / `T` reflection, and which
        // kind of curve left it.
        let mut last_cubic: Option<Vec2> = None;
        let mut last_quad: Option<Vec2> = None;
        let mut cmd: Option<u8> = None;

        fn skip_ws(b: &[u8], i: &mut usize) {
            while *i < b.len() && (b[*i].is_ascii_whitespace() || b[*i] == b',') {
                *i += 1;
            }
        }
        fn number(b: &[u8], i: &mut usize) -> Result<f32, PathError> {
            skip_ws(b, i);
            let at = *i;
            let mut j = at;
            if j < b.len() && (b[j] == b'-' || b[j] == b'+') {
                j += 1;
            }
            let digits = j;
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            if j < b.len() && b[j] == b'.' {
                j += 1;
                while j < b.len() && b[j].is_ascii_digit() {
                    j += 1;
                }
            }
            if j == digits || (j == digits + 1 && b[digits] == b'.') {
                return Err(PathError {
                    at,
                    what: "a number",
                });
            }
            if j < b.len() && (b[j] == b'e' || b[j] == b'E') {
                let mut k = j + 1;
                if k < b.len() && (b[k] == b'-' || b[k] == b'+') {
                    k += 1;
                }
                let e = k;
                while k < b.len() && b[k].is_ascii_digit() {
                    k += 1;
                }
                if k > e {
                    j = k;
                }
            }
            let s = std::str::from_utf8(&b[at..j]).map_err(|_| PathError {
                at,
                what: "a number",
            })?;
            *i = j;
            s.parse::<f32>().map_err(|_| PathError {
                at,
                what: "a number",
            })
        }
        fn flag(b: &[u8], i: &mut usize) -> Result<bool, PathError> {
            skip_ws(b, i);
            match b.get(*i) {
                Some(b'0') => {
                    *i += 1;
                    Ok(false)
                }
                Some(b'1') => {
                    *i += 1;
                    Ok(true)
                }
                _ => Err(PathError {
                    at: *i,
                    what: "an arc flag (0 or 1)",
                }),
            }
        }

        loop {
            skip_ws(b, &mut i);
            if i >= b.len() {
                break;
            }
            let c = b[i];
            if c.is_ascii_alphabetic() {
                cmd = Some(c);
                i += 1;
                if c == b'Z' || c == b'z' {
                    ops.push(PathOp::Close);
                    cur = start;
                    last_cubic = None;
                    last_quad = None;
                    continue;
                }
            } else if cmd.is_none() {
                return Err(PathError {
                    at: i,
                    what: "a command letter",
                });
            }
            let Some(c) = cmd else { break };
            let rel = c.is_ascii_lowercase();
            let base = if rel { cur } else { Vec2::ZERO };
            let point = |b: &[u8], i: &mut usize| -> Result<Vec2, PathError> {
                let x = number(b, i)?;
                let y = number(b, i)?;
                Ok(Vec2::new(base.x + x, base.y + y))
            };
            match c.to_ascii_uppercase() {
                b'M' => {
                    let p = point(b, &mut i)?;
                    ops.push(PathOp::MoveTo(p));
                    cur = p;
                    start = p;
                    // Further pairs are implicit line-tos.
                    cmd = Some(if rel { b'l' } else { b'L' });
                    last_cubic = None;
                    last_quad = None;
                }
                b'L' => {
                    let p = point(b, &mut i)?;
                    ops.push(PathOp::LineTo(p));
                    cur = p;
                    last_cubic = None;
                    last_quad = None;
                }
                b'H' => {
                    let x = number(b, &mut i)?;
                    let p = Vec2::new(base.x + x, cur.y);
                    ops.push(PathOp::LineTo(p));
                    cur = p;
                    last_cubic = None;
                    last_quad = None;
                }
                b'V' => {
                    let y = number(b, &mut i)?;
                    let p = Vec2::new(cur.x, base.y + y);
                    ops.push(PathOp::LineTo(p));
                    cur = p;
                    last_cubic = None;
                    last_quad = None;
                }
                b'C' => {
                    let c1 = point(b, &mut i)?;
                    let c2 = point(b, &mut i)?;
                    let p = point(b, &mut i)?;
                    ops.push(PathOp::CubicTo(c1, c2, p));
                    last_cubic = Some(c2);
                    last_quad = None;
                    cur = p;
                }
                b'S' => {
                    let c2 = point(b, &mut i)?;
                    let p = point(b, &mut i)?;
                    let c1 = match last_cubic {
                        Some(l) => Vec2::new(2.0 * cur.x - l.x, 2.0 * cur.y - l.y),
                        None => cur,
                    };
                    ops.push(PathOp::CubicTo(c1, c2, p));
                    last_cubic = Some(c2);
                    last_quad = None;
                    cur = p;
                }
                b'Q' => {
                    let c1 = point(b, &mut i)?;
                    let p = point(b, &mut i)?;
                    ops.push(PathOp::QuadTo(c1, p));
                    last_quad = Some(c1);
                    last_cubic = None;
                    cur = p;
                }
                b'T' => {
                    let p = point(b, &mut i)?;
                    let c1 = match last_quad {
                        Some(l) => Vec2::new(2.0 * cur.x - l.x, 2.0 * cur.y - l.y),
                        None => cur,
                    };
                    ops.push(PathOp::QuadTo(c1, p));
                    last_quad = Some(c1);
                    last_cubic = None;
                    cur = p;
                }
                b'A' => {
                    let rx = number(b, &mut i)?;
                    let ry = number(b, &mut i)?;
                    let rotation = number(b, &mut i)?;
                    let large = flag(b, &mut i)?;
                    let sweep = flag(b, &mut i)?;
                    let p = point(b, &mut i)?;
                    ops.push(PathOp::ArcTo {
                        rx: rx.abs(),
                        ry: ry.abs(),
                        rotation,
                        large,
                        sweep,
                        to: p,
                    });
                    last_cubic = None;
                    last_quad = None;
                    cur = p;
                }
                _ => {
                    return Err(PathError {
                        at: i - 1,
                        what: "one of M L H V C S Q T A Z",
                    });
                }
            }
        }
        Ok(Self::from_ops(ops))
    }
}

/// Separates two contours in a flattened outline: a point that is not a
/// point. `in_path` reads it as the end of one closed contour and the
/// start of the next.
pub const CONTOUR_BREAK: Vec2 = Vec2 {
    x: f32::NAN,
    y: f32::NAN,
};

/// How far a flattened curve may stray from the true one, logical px.
/// The hit outline's tolerance; the rasterizer flattens on its own.
pub const FLATTEN_TOLERANCE: f32 = 0.2;

/// The most pieces one curve or arc is cut into.
const MAX_PIECES: usize = 128;

/// How finely a flattening cuts: how far a chord may stray from the
/// curve, and the most pieces one curve or arc becomes.
#[derive(Clone, Copy)]
struct Fineness {
    tolerance: f32,
    pieces: usize,
}

/// The hit outline's, in logical px.
const HIT: Fineness = Fineness {
    tolerance: FLATTEN_TOLERANCE,
    pieces: MAX_PIECES,
};

/// A stroke's, in physical px, as the rasterizer is handed it (see
/// [`rasterize_at`]): a twentieth of a pixel, under what antialiasing
/// shows.
const RASTER: Fineness = Fineness {
    tolerance: 0.05,
    pieces: 4096,
};

/// Flattens `ops` into closed contours in `out`, each contour's points
/// followed by [`CONTOUR_BREAK`]. Every subpath is closed for the
/// purpose of a fill, as SVG closes it. A move with nothing after it
/// adds nothing.
pub fn flatten(ops: &[PathOp], out: &mut Vec<Vec2>) {
    flatten_as(ops, out, false, HIT, None);
}

/// Flattens `ops` into the polylines a stroke draws, each followed by
/// [`CONTOUR_BREAK`]: a subpath its `Z` closed ends back on its start, an
/// open one ends where it was left, so a stroke's hit pieces are the
/// pieces it paints and no chord across an open curve is among them.
pub fn flatten_stroke(ops: &[PathOp], out: &mut Vec<Vec2>) {
    flatten_as(ops, out, true, HIT, None);
}

/// With `closes`, a stroke's flattening says for each contour, in order,
/// whether its `Z` closed it — and leaves the start off its end, for the
/// rasterizer to close it as one outline, the way a stroke joins there.
fn flatten_as(
    ops: &[PathOp],
    out: &mut Vec<Vec2>,
    stroke: bool,
    fine: Fineness,
    mut closes: Option<&mut Vec<bool>>,
) {
    let mut cur = Vec2::ZERO;
    let mut open = false;
    let mut contour_start = out.len();
    let mut close = |out: &mut Vec<Vec2>, open: &mut bool, contour_start: usize, closed: bool| {
        if *open {
            // A contour of one point is nothing.
            if out.len() - contour_start >= 2 {
                match closes.as_deref_mut() {
                    Some(closes) => closes.push(closed),
                    None if stroke && closed => out.push(out[contour_start]),
                    None => {}
                }
                out.push(CONTOUR_BREAK);
            } else {
                out.truncate(contour_start);
            }
            *open = false;
        }
    };
    for op in ops {
        match *op {
            PathOp::MoveTo(p) => {
                close(out, &mut open, contour_start, false);
                contour_start = out.len();
                out.push(p);
                cur = p;
                open = true;
            }
            PathOp::Close => {
                close(out, &mut open, contour_start, true);
                // A draw after a close starts where the subpath began.
                if let Some(&s) = out.get(contour_start) {
                    cur = s;
                }
                contour_start = out.len();
            }
            _ => {
                if !open {
                    contour_start = out.len();
                    out.push(cur);
                    open = true;
                }
                match *op {
                    PathOp::LineTo(p) => {
                        out.push(p);
                        cur = p;
                    }
                    PathOp::QuadTo(c, p) => {
                        flatten_quad(cur, c, p, out, fine);
                        cur = p;
                    }
                    PathOp::CubicTo(a, b, p) => {
                        flatten_cubic(cur, a, b, p, out, fine);
                        cur = p;
                    }
                    PathOp::ArcTo {
                        rx,
                        ry,
                        rotation,
                        large,
                        sweep,
                        to,
                    } => {
                        flatten_arc(cur, rx, ry, rotation, large, sweep, to, out, fine);
                        cur = to;
                    }
                    PathOp::MoveTo(_) | PathOp::Close => unreachable!(),
                }
            }
        }
    }
    close(out, &mut open, contour_start, false);
}

fn pieces(dd: f32, k: f32, fine: Fineness) -> usize {
    ((dd * k / fine.tolerance).sqrt().ceil() as usize).clamp(1, fine.pieces)
}

fn flatten_quad(p0: Vec2, c: Vec2, p1: Vec2, out: &mut Vec<Vec2>, fine: Fineness) {
    let dd = Vec2::new(p0.x - 2.0 * c.x + p1.x, p0.y - 2.0 * c.y + p1.y);
    let n = pieces((dd.x * dd.x + dd.y * dd.y).sqrt(), 0.25, fine);
    for i in 1..=n {
        let t = i as f32 / n as f32;
        let u = 1.0 - t;
        out.push(Vec2::new(
            u * u * p0.x + 2.0 * u * t * c.x + t * t * p1.x,
            u * u * p0.y + 2.0 * u * t * c.y + t * t * p1.y,
        ));
    }
}

fn flatten_cubic(p0: Vec2, a: Vec2, b: Vec2, p1: Vec2, out: &mut Vec<Vec2>, fine: Fineness) {
    let d1 = Vec2::new(p0.x - 2.0 * a.x + b.x, p0.y - 2.0 * a.y + b.y);
    let d2 = Vec2::new(a.x - 2.0 * b.x + p1.x, a.y - 2.0 * b.y + p1.y);
    let dd = (d1.x * d1.x + d1.y * d1.y)
        .max(d2.x * d2.x + d2.y * d2.y)
        .sqrt();
    let n = pieces(dd, 0.75, fine);
    for i in 1..=n {
        let t = i as f32 / n as f32;
        let u = 1.0 - t;
        let (uu, tt) = (u * u, t * t);
        out.push(Vec2::new(
            uu * u * p0.x + 3.0 * uu * t * a.x + 3.0 * u * tt * b.x + tt * t * p1.x,
            uu * u * p0.y + 3.0 * uu * t * a.y + 3.0 * u * tt * b.y + tt * t * p1.y,
        ));
    }
}

/// An arc's rotation in degrees within one turn, exactly for any finite
/// number: a rotation past 1e38 was a NaN in every point of zeno's arc
/// (backlog FZ2), and the sine of one is noise even where it is a number.
fn turn_of(degrees: f32) -> f32 {
    degrees.rem_euclid(360.0)
}

/// An SVG arc in centre form: the centre, the radii as scaled up when the
/// endpoints are too far apart, the start angle and the sweep in radians.
/// `None` for an arc whose endpoints coincide or whose radii are zero,
/// which SVG draws as a line (or nothing).
///
/// Worked in f64: the products of four radii it takes overflow an f32
/// past a radius of 1e19, and a radius of 1e25 made its centre the
/// chord's middle and its ellipse a small one (backlog FZ2).
#[allow(clippy::too_many_arguments)]
fn arc_center(
    from: Vec2,
    rx: f32,
    ry: f32,
    rotation: f32,
    large: bool,
    sweep: bool,
    to: Vec2,
) -> Option<(Vec2, f32, f32, f32, f32, f32)> {
    if (from.x - to.x).abs() < 1e-6 && (from.y - to.y).abs() < 1e-6 {
        return None;
    }
    let (mut rx, mut ry) = (f64::from(rx.abs()), f64::from(ry.abs()));
    if rx < 1e-6 || ry < 1e-6 {
        return None;
    }
    let phi = f64::from(turn_of(rotation)).to_radians();
    let (sin_phi, cos_phi) = phi.sin_cos();
    let (fx, fy, tx, ty) = (
        f64::from(from.x),
        f64::from(from.y),
        f64::from(to.x),
        f64::from(to.y),
    );
    let dx = (fx - tx) * 0.5;
    let dy = (fy - ty) * 0.5;
    let x1 = cos_phi * dx + sin_phi * dy;
    let y1 = -sin_phi * dx + cos_phi * dy;
    let lambda = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry);
    if lambda > 1.0 {
        let s = lambda.sqrt();
        rx *= s;
        ry *= s;
    }
    let num = (rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1).max(0.0);
    let den = rx * rx * y1 * y1 + ry * ry * x1 * x1;
    let mut coef = if den > 0.0 { (num / den).sqrt() } else { 0.0 };
    if large == sweep {
        coef = -coef;
    }
    let cx1 = coef * rx * y1 / ry;
    let cy1 = -coef * ry * x1 / rx;
    let cx = cos_phi * cx1 - sin_phi * cy1 + (fx + tx) * 0.5;
    let cy = sin_phi * cx1 + cos_phi * cy1 + (fy + ty) * 0.5;
    let ux = (x1 - cx1) / rx;
    let uy = (y1 - cy1) / ry;
    let vx = (-x1 - cx1) / rx;
    let vy = (-y1 - cy1) / ry;
    let angle = |ax: f64, ay: f64, bx: f64, by: f64| -> f64 {
        let dot = ax * bx + ay * by;
        let len = ((ax * ax + ay * ay) * (bx * bx + by * by)).sqrt();
        let mut a = (dot / len).clamp(-1.0, 1.0).acos();
        if ax * by - ay * bx < 0.0 {
            a = -a;
        }
        a
    };
    let theta = angle(1.0, 0.0, ux, uy);
    let mut delta = angle(ux, uy, vx, vy);
    let tau = std::f64::consts::TAU;
    if !sweep && delta > 0.0 {
        delta -= tau;
    } else if sweep && delta < 0.0 {
        delta += tau;
    }
    Some((
        Vec2::new(cx as f32, cy as f32),
        rx as f32,
        ry as f32,
        phi as f32,
        theta as f32,
        delta as f32,
    ))
}

#[allow(clippy::too_many_arguments)]
fn flatten_arc(
    from: Vec2,
    rx: f32,
    ry: f32,
    rotation: f32,
    large: bool,
    sweep: bool,
    to: Vec2,
    out: &mut Vec<Vec2>,
    fine: Fineness,
) {
    let Some((c, rx, ry, phi, theta, delta)) = arc_center(from, rx, ry, rotation, large, sweep, to)
    else {
        out.push(to);
        return;
    };
    let r = rx.max(ry);
    // The chord of one piece strays `r (1 - cos(step / 2))` from the arc.
    // For a radius a tolerance is next to nothing against, `1 - tol / r`
    // is 1 in an f32 and the step 0, which made a large arc one piece -
    // a line - where it goes round an ellipse 1e17 px across (backlog
    // FZ2): `acos(1 - x)` is `sqrt(2x)` there.
    let x = fine.tolerance / r;
    let step = if x < 1e-4 {
        2.0 * (2.0 * x).sqrt()
    } else {
        2.0 * (1.0 - x).clamp(-1.0, 1.0).acos()
    };
    let n = if step > 0.0 {
        ((delta.abs() / step).ceil() as usize).clamp(1, fine.pieces)
    } else {
        1
    };
    let (sin_phi, cos_phi) = phi.sin_cos();
    for i in 1..=n {
        let a = theta + delta * i as f32 / n as f32;
        let (sa, ca) = a.sin_cos();
        let x = rx * ca;
        let y = ry * sa;
        out.push(Vec2::new(
            c.x + cos_phi * x - sin_phi * y,
            c.y + sin_phi * x + cos_phi * y,
        ));
    }
    // The last point is the declared end, to the bit.
    if let Some(last) = out.last_mut() {
        *last = to;
    }
}

/// Whether `p` is inside the flattened contours `pts` (as [`flatten`]
/// lays them out) by `rule`. A point on an edge counts as inside on one
/// side and outside on the other, so of two wedges sharing an edge
/// exactly one takes it.
pub fn in_path(p: Vec2, pts: &[Vec2], rule: FillRule) -> bool {
    let mut winding = 0i32;
    let mut crossings = 0u32;
    for contour in pts.split(|v| v.x.is_nan()) {
        let n = contour.len();
        if n < 3 {
            continue;
        }
        let mut j = n - 1;
        for i in 0..n {
            let (a, b) = (contour[i], contour[j]);
            if (a.y > p.y) != (b.y > p.y) {
                let x = a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x);
                if p.x < x {
                    crossings += 1;
                    winding += if b.y > a.y { 1 } else { -1 };
                }
            }
            j = i;
        }
    }
    match rule {
        FillRule::NonZero => winding != 0,
        FillRule::EvenOdd => crossings % 2 == 1,
    }
}

/// The bounding box of a flattened outline, ignoring contour breaks;
/// `None` for no points.
pub fn bounds(pts: &[Vec2]) -> Option<Rect> {
    let mut it = pts.iter().filter(|p| !p.x.is_nan());
    let first = *it.next()?;
    let (mut min, mut max) = (first, first);
    for p in it {
        min.x = min.x.min(p.x);
        min.y = min.y.min(p.y);
        max.x = max.x.max(p.x);
        max.y = max.y.max(p.y);
    }
    Some(Rect::new(min.x, min.y, max.x - min.x, max.y - min.y))
}

/// FNV-1a over the ops' bits: what a mask is keyed on. The same ops are
/// the same hash in every binding, since the floats crossed as floats.
pub fn hash_ops(ops: &[PathOp]) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut h = OFFSET;
    let mut mix = |v: u32| {
        for byte in v.to_le_bytes() {
            h ^= u64::from(byte);
            h = h.wrapping_mul(PRIME);
        }
    };
    let mut scratch = Vec::with_capacity(8);
    for op in ops {
        scratch.clear();
        op.write(&mut scratch);
        for v in &scratch {
            mix(v.to_bits());
        }
    }
    h
}

/// What one mask paints: the fill by its rule, bled half a pixel so two
/// fills sharing an edge meet without the background showing, or the
/// outline stroked `width` physical px wide — whole, or cut into the
/// marks of a dash pattern (backlog V2), its lengths physical too.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MaskPaint {
    Fill(FillRule),
    Stroke(f32),
    Dashed(f32, DashCut),
}

/// A dash pattern as the rasterizer takes it: the centre-line lengths of
/// a mark, a gap, a mark and a gap, and how far into them the stroke
/// starts. [`crate::line::Dash`] is what an app declares.
pub type DashCut = crate::line::Cut;

/// Rasterizes `ops` (in logical px, relative to the node's box) into a
/// `w × h` alpha mask at `scale`, the node's box shifted by `bin`
/// quarter-pixels so a node at a fractional position lands on the pixel
/// grid as it would be drawn. `w * h` bytes, row after row.
///
/// An outline the rasterizer cannot draw is left blank (backlog FZ2): a
/// number that is not finite, an outline more than [`RASTER_SPAN`] px
/// wide and tall together at this scale (stroke included), one farther
/// than [`RASTER_REACH`] from the mask's origin, or a mask wider or taller
/// than [`MAX_MASK_SIDE`]. A path node never asks for one — its mask is
/// its outline's box, and at most `MAX_MASK_SIDE` a side.
pub fn rasterize(
    ops: &[PathOp],
    scale: f32,
    bin: (u8, u8),
    w: u32,
    h: u32,
    paint: MaskPaint,
) -> Vec<u8> {
    let off = Vec2::new(f32::from(bin.0) * 0.25, f32::from(bin.1) * 0.25);
    rasterize_at(ops, scale, off, w, h, paint)
}

/// [`rasterize`] with the box's origin `off` physical px into the mask:
/// what a turning path's mask is drawn with, its pivot at the mask's
/// centre.
pub fn rasterize_at(
    ops: &[PathOp],
    scale: f32,
    off: Vec2,
    w: u32,
    h: u32,
    paint: MaskPaint,
) -> Vec<u8> {
    use swash::zeno::{Cap, Command, Fill, Join, Mask, PathBuilder, Point, Stroke};
    let len = (w as usize) * (h as usize);
    let mut buf = vec![0u8; len];
    if len == 0 || ops.is_empty() || !drawable(ops, scale, off, w, h, paint) {
        return buf;
    }
    // On zeno's own grid, a 256th of a pixel: two points are then one
    // point or a step apart. Its stroker turns a curve's tangent into a
    // normal by its length, and a control point 1e-41 px from the curve's
    // end made that normal - and the outline offset along it - vast
    // (backlog FZ2): an overflow in a debug build, a number past what its
    // conversion takes in a release one. Nothing drawn moves by more than
    // the 512th of a pixel the rasterizer resolves anyway.
    let snap = |v: f32| (v * 256.0).round() / 256.0;
    let place = |p: Vec2| Vec2::new(p.x * scale + off.x, p.y * scale + off.y);
    let at = |p: Vec2| {
        let q = place(p);
        Point::new(snap(q.x), snap(q.y))
    };
    let mut arc = Vec::new();
    let mut cmds: Vec<Command> = Vec::with_capacity(ops.len() + 1);
    let mut cur = Vec2::ZERO;
    let mut start = Vec2::ZERO;
    let mut open = false;
    for op in ops {
        match *op {
            PathOp::MoveTo(p) => {
                cmds.move_to(at(p));
                cur = p;
                start = p;
                open = true;
            }
            PathOp::Close => {
                if open {
                    cmds.close();
                }
                // A draw after a close starts where the subpath began,
                // as `flatten` has it.
                cur = start;
                open = false;
            }
            _ => {
                if !open {
                    cmds.move_to(at(cur));
                    start = cur;
                    open = true;
                }
                match *op {
                    PathOp::LineTo(p) => {
                        cmds.line_to(at(p));
                        cur = p;
                    }
                    PathOp::QuadTo(c, p) => {
                        cmds.quad_to(at(c), at(p));
                        cur = p;
                    }
                    PathOp::CubicTo(a, b, p) => {
                        cmds.curve_to(at(a), at(b), at(p));
                        cur = p;
                    }
                    PathOp::ArcTo {
                        rx,
                        ry,
                        rotation,
                        large,
                        sweep,
                        to,
                    } => {
                        // kui's own arc, cut at [`RASTER`]'s fineness, not
                        // zeno's: its arc is f32 throughout, and it scaled a
                        // radius of 1e-9 up to an ellipse, drew a radius of
                        // 1e25 round one `drawable` had not measured, and
                        // took a rotation past 1e38 to a NaN (backlog FZ2).
                        // This is the arc the hit outline and `drawable`
                        // are cut from, finer.
                        arc.clear();
                        flatten_arc(
                            place(cur),
                            rx * scale,
                            ry * scale,
                            rotation,
                            large,
                            sweep,
                            place(to),
                            &mut arc,
                            RASTER,
                        );
                        for p in &arc {
                            cmds.line_to(Point::new(snap(p.x), snap(p.y)));
                        }
                        cur = to;
                    }
                    PathOp::MoveTo(_) | PathOp::Close => unreachable!(),
                }
            }
        }
    }
    // What a stroke is drawn from: the outline flattened here, in physical
    // px at [`RASTER`]'s fineness, not zeno's curves. Its stroker offsets
    // a curve along its normals, and a curve that doubles back on itself
    // - a quad whose control point lies on the line past its end, `Q50 0
    // 8 0` from the origin - offset to control points past what its fixed
    // point holds (backlog FZ2). A polyline stroked with round joins and
    // caps is the same stroke, and a dash measures the same length along
    // it. The fill keeps the curves: zeno fills them without offsetting.
    let lines = {
        let mut physical: Vec<PathOp> = Vec::with_capacity(ops.len() + 1);
        // A path that draws before it moves starts at its origin, which
        // is `off` in the mask - not the mask's own origin, where
        // flattening starts one: an arc whose ends were 1e-41 apart was
        // flattened from a point a quarter-pixel off and went round an
        // ellipse 1e25 px tall (backlog FZ2).
        if !matches!(ops.first(), Some(PathOp::MoveTo(_))) {
            physical.push(PathOp::MoveTo(off));
        }
        physical.extend(ops.iter().map(|op| op.placed(scale, off)));
        let (mut pts, mut closes) = (Vec::new(), Vec::new());
        flatten_as(&physical, &mut pts, true, RASTER, Some(&mut closes));
        let mut lines: Vec<Command> = Vec::with_capacity(pts.len() + closes.len());
        let mut closes = closes.into_iter();
        let mut kept: Vec<Point> = Vec::new();
        for contour in pts.split(|p| p.x.is_nan()).filter(|c| !c.is_empty()) {
            let closed = closes.next() == Some(true);
            // A piece of no length is no piece: the stroker would join at
            // a direction it has to make up, and a contour that is one
            // point once they are gone - an arc whose ends are 1e-41 apart -
            // it normalizes to a NaN (backlog FZ2). Such a contour draws
            // nothing.
            kept.clear();
            for p in contour {
                let q = Point::new(snap(p.x), snap(p.y));
                if kept.last() != Some(&q) {
                    kept.push(q);
                }
            }
            if closed && kept.len() > 1 && kept.first() == kept.last() {
                kept.pop();
            }
            let Some((&first, rest)) = kept.split_first() else {
                continue;
            };
            if rest.is_empty() {
                continue;
            }
            lines.move_to(first);
            for &q in rest {
                lines.line_to(q);
            }
            // Closed as zeno closes it, one outline joined at the start:
            // two ends meeting there are two antialiased edges, and their
            // coverage adds.
            if closed {
                lines.close();
            }
        }
        lines
    };
    match paint {
        MaskPaint::Fill(rule) => {
            let fill = match rule {
                FillRule::NonZero => Fill::NonZero,
                FillRule::EvenOdd => Fill::EvenOdd,
            };
            Mask::new(&cmds[..])
                .style(fill)
                .size(w, h)
                .render_into(&mut buf, None);
            // A fill that covers nothing - a ring's sector at a sweep of
            // 0, out along a radius and back - has no edge to bleed: the
            // outline alone would paint it as a hairline.
            if buf.iter().all(|&a| a == 0) {
                return buf;
            }
            // The bleed: the outline a pixel wide, in the same mask by
            // max, so two fills sharing an edge overlap by the ramp.
            let mut edge = vec![0u8; len];
            let mut stroke = Stroke::new(1.0);
            stroke.join(Join::Round).cap(Cap::Round);
            Mask::new(&lines[..])
                .style(stroke)
                .size(w, h)
                .render_into(&mut edge, None);
            for (a, e) in buf.iter_mut().zip(edge) {
                *a = (*a).max(e);
            }
        }
        MaskPaint::Stroke(width) => {
            let mut stroke = Stroke::new(width.max(0.0));
            stroke.join(Join::Round).cap(Cap::Round);
            Mask::new(&lines[..])
                .style(stroke)
                .size(w, h)
                .render_into(&mut buf, None);
        }
        MaskPaint::Dashed(width, cut) => {
            let mut stroke = Stroke::new(width.max(0.0));
            stroke.join(Join::Round).cap(Cap::Round);
            // A mark and its gap under a pixel are not a pattern any
            // more, as on a line: the stroke whole.
            if !cut.finer_than(1.0) {
                stroke.dash(&cut.lens, cut.offset);
            }
            Mask::new(&lines[..])
                .style(stroke)
                .size(w, h)
                .render_into(&mut buf, None);
        }
    }
    buf
}

/// The most an outline may span, wide plus tall, in physical px with its
/// stroke, for the rasterizer to draw it (backlog FZ2). zeno walks a line
/// in 24.8 fixed point and sums its run and rise, each times a pixel's
/// 256, in an `i32`: past some 16 416 px together that overflows — a
/// panic in a debug build, a wrong mask in a release one. A mask of
/// [`MAX_MASK_SIDE`] on both sides is exactly this, and every line zeno
/// cuts an outline into lies inside the outline's box.
pub const RASTER_SPAN: f32 = 16384.0;

/// How far from the mask's origin, in physical px, an outline may lie
/// for the rasterizer to draw it (backlog FZ2). zeno converts a
/// coordinate to fixed point with an unchecked cast, undefined behaviour
/// past ±2^23 px, and sums eight of a curve's where it splits one, which
/// overflows past ±2^20; a curve's control points can lie a few times its
/// span outside its box, so the box is held well inside both.
pub const RASTER_REACH: f32 = 262_144.0;

/// Whether zeno can draw `ops` at `scale` from `off` into a `w × h`
/// mask with `paint`: every number finite, and the outline within
/// [`RASTER_SPAN`] and [`RASTER_REACH`]. What the box is taken from is
/// the hit outline's flattening, the same points a path node's mask is
/// sized by.
fn drawable(ops: &[PathOp], scale: f32, off: Vec2, w: u32, h: u32, paint: MaskPaint) -> bool {
    let width = match paint {
        // The fill's bleed is a one-pixel stroke.
        MaskPaint::Fill(_) => 1.0,
        MaskPaint::Stroke(w) => w,
        MaskPaint::Dashed(w, cut) => {
            if !cut.offset.is_finite() || !cut.lens.iter().all(|l| l.is_finite()) {
                return false;
            }
            w
        }
    };
    if !(scale.is_finite() && off.x.is_finite() && off.y.is_finite() && width.is_finite())
        || w > MAX_MASK_SIDE
        || h > MAX_MASK_SIDE
        || !ops.iter().all(PathOp::is_finite)
    {
        return false;
    }
    let mut pts = Vec::new();
    flatten(ops, &mut pts);
    let Some(b) = bounds(&pts) else {
        // A move alone: nothing to draw, and nothing zeno trips on.
        return true;
    };
    let (x0, y0) = (b.x * scale + off.x, b.y * scale + off.y);
    let (x1, y1) = (x0 + b.w * scale, y0 + b.h * scale);
    let reach = [x0, y0, x1, y1].iter().all(|v| v.abs() <= RASTER_REACH);
    reach && (x1 - x0).abs() + (y1 - y0).abs() + 2.0 * width.abs() <= RASTER_SPAN
}

/// The texels a mask may take of the atlas before it goes to a texture
/// of its own: a quarter of the biggest page, since four of them would
/// empty it every frame.
pub const MAX_ATLAS_MASK_TEXELS: u64 = 2048 * 2048;

/// The widest or tallest a mask may be and still be drawn from a texture
/// of its own: a texture every device kui runs on can hold — `kui-wgpu`
/// opens its device with wgpu's default limits, whose
/// `max_texture_dimension_2d` is this number whatever the adapter could
/// do, so the core's limit and the renderer's are one. Past it the node
/// draws nothing, with `path-too-large`. A host that draws the list
/// itself on a device that holds less refuses the texture on its side.
pub const MAX_MASK_SIDE: u32 = 8192;

/// A mask drawn from a texture of its own rather than the atlas (ADR
/// 0040, decisions 7 and 8): the handle the backend caches it under, and
/// the pixels it uploads from.
pub struct PathTexture {
    pub id: crate::resources::ImageId,
    pub w: u32,
    pub h: u32,
    pub rgba: std::sync::Arc<Vec<u8>>,
    last_used: u64,
}

/// The texture-backed masks of one core, by the mask key the atlas would
/// have held them under. One a frame does not draw is dropped at the
/// next frame's start, its handle handed to the display list for the
/// backend to free; an animating path, whose key is new each frame, thus
/// uploads one texture a frame and frees one. The ones a core still
/// holds when it goes go the way a removed image does: to the session,
/// for the next display list any of its windows builds (RG112).
pub struct PathTextures {
    by_key: rustc_hash::FxHashMap<u64, PathTexture>,
    session: crate::session::Session,
}

impl Drop for PathTextures {
    fn drop(&mut self) {
        for t in self.by_key.values() {
            crate::resources::unmint_image(t.id);
        }
        // Held only if the core is dropped from inside a borrow of its
        // session, which nothing does; then the handles are forgotten
        // and the backend keeps the textures, as before.
        if let Some(mut sess) = self.session.try_state() {
            sess.dropped
                .images
                .extend(self.by_key.values().map(|t| t.id));
        }
    }
}

impl PathTextures {
    pub(crate) fn new(session: crate::session::Session) -> Self {
        Self {
            by_key: Default::default(),
            session,
        }
    }

    /// The pixels of the texture minted as `id`, for a host that draws
    /// the list itself and asks by handle.
    pub(crate) fn pixels(
        &self,
        id: crate::resources::ImageId,
    ) -> Option<(u32, u32, std::sync::Arc<Vec<u8>>)> {
        self.by_key
            .values()
            .find(|t| t.id == id)
            .map(|t| (t.w, t.h, t.rgba.clone()))
    }

    /// The texture for `key`, made from `coverage` (`w * h` alpha bytes)
    /// on a miss.
    pub(crate) fn get_or_make(
        &mut self,
        key: u64,
        w: u32,
        h: u32,
        frame: u64,
        session: crate::resources::SessionId,
        coverage: impl FnOnce() -> Vec<u8>,
    ) -> &PathTexture {
        if let Some(tex) = self.by_key.get(&key) {
            debug_assert!(tex.w == w && tex.h == h, "a mask key names one size");
        }
        let tex = self.by_key.entry(key).or_insert_with(|| {
            let mask = coverage();
            let mut rgba = Vec::with_capacity(mask.len() * 4);
            for &a in &mask {
                rgba.extend_from_slice(&[255, 255, 255, a]);
            }
            PathTexture {
                id: crate::resources::mint_image(session),
                w,
                h,
                rgba: std::sync::Arc::new(rgba),
                last_used: frame,
            }
        });
        tex.last_used = frame;
        tex
    }

    /// Drops every texture no frame since `frame - 1` drew, and returns
    /// their handles for the display list's `dropped_textures`.
    pub(crate) fn sweep(&mut self, frame: u64) -> Vec<crate::resources::ImageId> {
        let mut gone = Vec::new();
        self.by_key.retain(|_, t| {
            if t.last_used + 1 < frame {
                crate::resources::unmint_image(t.id);
                gone.push(t.id);
                false
            } else {
                true
            }
        });
        gone
    }

    /// How many masks are texture-backed right now.
    pub fn len(&self) -> usize {
        self.by_key.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_key.is_empty()
    }
}

/// The mask key the atlas and the textures hold a path's mask under: its
/// ops' hash with the scale, the quarter-pixel bin and the paint mixed in.
/// The bin a turning path's masks are keyed under: none of the sixteen,
/// since its mask is centred on its pivot and not binned.
pub(crate) const TURNED_BIN: (u8, u8) = (0xff, 0xff);

pub(crate) fn mask_key(hash: u64, scale: f32, bin: (u8, u8), paint: MaskPaint) -> u64 {
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut h = hash;
    let mut mix = |v: u64| {
        h ^= v;
        h = h.wrapping_mul(PRIME);
    };
    mix(u64::from(scale.to_bits()));
    mix(u64::from(bin.0) | (u64::from(bin.1) << 8));
    match paint {
        MaskPaint::Fill(rule) => mix(0x1000 | rule.index() as u64),
        MaskPaint::Stroke(w) => mix(0x2000_0000_0000 | u64::from(w.to_bits())),
        MaskPaint::Dashed(w, cut) => {
            mix(0x3000_0000_0000 | u64::from(w.to_bits()));
            mix(cut.hash());
        }
    }
    h
}

/// How many frames apart two changes of one key's ops may be and still
/// be an animation: a shape driven at a quarter of
/// the frame rate, or one whose changes have a frame between them that
/// something else asked for, moves as surely as one that changes every
/// frame, and each of its shapes would be a slot the atlas never reuses.
pub const ANIMATING_WINDOW: u64 = 8;

/// How many frames an animating key's shape stays the same before it is
/// a still shape again, and its mask the atlas's: a second at 120 Hz. A
/// spinner that pauses for a frame or ten stays out; one that stopped
/// stops costing a texture and a draw of its own.
pub const SETTLED_AFTER: u64 = 120;

/// What the core remembers of one `path` key between frames, to tell a
/// new shape from a moving one.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Motion {
    /// The ops' hash the key last declared, and the frame it did.
    pub hash: u64,
    pub seen: u64,
    /// The frame the hash last differed from the one before; 0 for never.
    pub changed: u64,
    /// Two changes within [`ANIMATING_WINDOW`], until the shape has held
    /// for [`SETTLED_AFTER`].
    pub animating: bool,
}

/// A `d` string's ops as the core last parsed them under one key, so a
/// binding that hands the string over every frame pays the parse once
/// per string rather than once per frame.
pub(crate) struct Parsed {
    /// `key::hash_bulk` of the string, and its length.
    pub hash: u64,
    pub len: usize,
    pub seen: u64,
    pub ops: Vec<PathOp>,
}

/// One path's run in the frame's op list. The ops are stored relative to
/// the node's box, so a node that eases or slides carries them along.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Run {
    pub first: u32,
    pub len: u32,
    pub rule: FillRule,
    /// The stroke width in logical px; 0 for no stroke.
    pub stroke_w: f32,
    /// What cuts the stroke into marks; None for a solid one.
    pub dash: Option<crate::line::Cut>,
    /// [`hash_ops`] of the ops as stored.
    pub hash: u64,
    /// The turn in radians when the path declared one: its box is then
    /// the square about its pivot, and the angle is the quad's.
    pub angle: Option<f32>,
    /// The key's ops changed twice within [`ANIMATING_WINDOW`] frames:
    /// the mask goes to a texture of its own until the shape has held
    /// for [`SETTLED_AFTER`].
    pub animating: bool,
}

/// The frame's paths, and the previous frame's while an `exit` needs it.
#[derive(Default)]
pub struct PathStore {
    runs: Kept<Run>,
    ops: Kept<PathOp>,
    scratch: Vec<Vec2>,
}

impl PathStore {
    /// Starts a frame. `keep_prev` retains the list just finished so a
    /// departing path's ghost can copy its ops out of it.
    pub(crate) fn begin_frame(&mut self, keep_prev: bool) {
        self.runs.begin(keep_prev);
        self.ops.begin(keep_prev);
    }

    /// Adds a path (ops in parent-box coordinates): boxes the outline, and
    /// stores the ops relative to the box. Returns the id and the box, or
    /// None for a path that draws nothing.
    pub(crate) fn push(
        &mut self,
        ops: &[PathOp],
        rule: FillRule,
        stroke_w: f32,
        dash: Option<crate::line::Cut>,
        turn: Option<Turn>,
    ) -> Option<(PathId, Rect)> {
        self.scratch.clear();
        flatten(ops, &mut self.scratch);
        let b = bounds(&self.scratch)?;
        // Two logical px past the outline — one for the bleed, one for
        // the edge ramp — and half the stroke's width further for a
        // stroke, so no mask is cut by its own edge.
        let pad = 2.0 + stroke_w * 0.5;
        let rect = match turn {
            None => Rect::new(b.x - pad, b.y - pad, b.w + 2.0 * pad, b.h + 2.0 * pad),
            // The square the turn sweeps: centred on the pivot, out to
            // the farthest point of the outline, so it is the same box —
            // and the same ops, hash and mask — at every angle.
            Some(turn) => {
                let c = turn
                    .pivot
                    .unwrap_or(Vec2::new(b.x + b.w * 0.5, b.y + b.h * 0.5));
                let far = self
                    .scratch
                    .iter()
                    .filter(|p| !p.x.is_nan())
                    .map(|p| (p.x - c.x).hypot(p.y - c.y))
                    .fold(0.0f32, f32::max);
                let half = far + pad;
                Rect::new(c.x - half, c.y - half, 2.0 * half, 2.0 * half)
            }
        };
        let origin = Vec2::new(rect.x, rect.y);
        let first = self.ops.len();
        let shift = Vec2::new(-origin.x, -origin.y);
        self.ops.extend(ops.iter().map(|op| op.shifted(shift)));
        let hash = hash_ops(&self.ops[first..]);
        let id = PathId(self.runs.len() as u32);
        self.runs.push(Run {
            first: first as u32,
            len: (self.ops.len() - first) as u32,
            rule,
            stroke_w,
            dash,
            hash,
            angle: turn.map(Turn::radians),
            animating: false,
        });
        Some((id, rect))
    }

    pub(crate) fn set_animating(&mut self, id: PathId) {
        if let Some(run) = self.runs.get_mut(id.0 as usize) {
            run.animating = true;
        }
    }

    /// This frame's run and its ops, relative to the node.
    pub(crate) fn run(&self, id: PathId) -> (Run, &[PathOp]) {
        let run = self.runs[id.0 as usize];
        (
            run,
            &self.ops[run.first as usize..(run.first + run.len) as usize],
        )
    }

    /// The same, read from the previous frame's list (a departing path's
    /// id indexes that list, not this frame's).
    pub(crate) fn prev_run(&self, id: PathId) -> Option<(Run, &[PathOp])> {
        let run = *self.runs.prev().get(id.0 as usize)?;
        Some((
            run,
            &self.ops.prev()[run.first as usize..(run.first + run.len) as usize],
        ))
    }

    pub fn len(&self) -> usize {
        self.runs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.runs.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pts(ops: &[PathOp]) -> Vec<Vec2> {
        let mut out = Vec::new();
        flatten(ops, &mut out);
        out
    }

    /// What zeno cannot draw is left blank, not handed to it (backlog
    /// FZ2, from the first fuzz round): a line wider than its fixed point
    /// spans overflowed it, a control point at 1.4e18 overflowed its curve
    /// split, and a NaN or a point past ±2^23 px is undefined behaviour in
    /// its conversion. An outline it can draw still draws, and one the
    /// size of the largest mask a node makes is one it can.
    #[test]
    fn an_outline_past_the_rasterizer_is_left_blank() {
        let blank = |ops: &[PathOp], paint| {
            rasterize(ops, 1.0, (0, 0), 48, 32, paint)
                .iter()
                .all(|&a| a == 0)
        };
        let fill = MaskPaint::Fill(FillRule::NonZero);
        let long = Path::parse("M60 60 L60610 0 Z").unwrap();
        assert!(blank(long.ops(), fill));
        let far = [
            PathOp::MoveTo(Vec2::new(10.0, 0.0)),
            PathOp::CubicTo(
                Vec2::new(50.0, 0.0),
                Vec2::new(1.0, 0.0),
                Vec2::new(20.0, 1.44e18),
            ),
        ];
        assert!(blank(&far, fill));
        let away = Path::parse("M1e7 1e7 l10 0 l0 10 z").unwrap();
        assert!(blank(away.ops(), MaskPaint::Stroke(2.0)));
        let nan = [
            PathOp::MoveTo(Vec2::new(2.0, 2.0)),
            PathOp::LineTo(Vec2::new(f32::NAN, 20.0)),
            PathOp::LineTo(Vec2::new(30.0, 20.0)),
        ];
        assert!(blank(&nan, fill));
        // A stroke as wide as the span is past it too.
        let tri = Path::parse("M2 2 L40 2 L20 30 Z").unwrap();
        assert!(blank(tri.ops(), MaskPaint::Stroke(RASTER_SPAN)));
        assert!(!blank(tri.ops(), fill));
        assert!(!blank(tri.ops(), MaskPaint::Stroke(2.0)));
        // The largest mask a node makes, corner to corner, still draws.
        let side = (MAX_MASK_SIDE - 2) as f32;
        let diag = [
            PathOp::MoveTo(Vec2::new(1.0, 1.0)),
            PathOp::LineTo(Vec2::new(side, side)),
            PathOp::LineTo(Vec2::new(1.0, side)),
            PathOp::Close,
        ];
        assert!(drawable(
            &diag,
            1.0,
            Vec2::ZERO,
            MAX_MASK_SIDE,
            MAX_MASK_SIDE,
            fill
        ));
    }

    /// An arc's rotation is taken within one turn before it is drawn
    /// (backlog FZ2): one of -1.7e38 degrees became an infinite angle in
    /// zeno and a NaN in every point after. It draws what the same
    /// rotation within a turn draws; and a closed contour's stroke is one
    /// outline, so its start is no darker than the rest of its edge.
    #[test]
    fn an_arc_turned_past_any_turn_draws_as_within_one() {
        let arc = |rotation: f32| {
            let ops = [
                PathOp::MoveTo(Vec2::new(4.0, 20.0)),
                PathOp::ArcTo {
                    rx: 20.0,
                    ry: 8.0,
                    rotation,
                    large: true,
                    sweep: true,
                    to: Vec2::new(40.0, 24.0),
                },
                PathOp::Close,
            ];
            let fill = rasterize(
                &ops,
                1.0,
                (0, 0),
                64,
                48,
                MaskPaint::Fill(FillRule::NonZero),
            );
            let stroke = rasterize(&ops, 1.0, (0, 0), 64, 48, MaskPaint::Stroke(1.5));
            (fill, stroke)
        };
        let far = -1.7e38f32;
        assert_eq!(arc(far), arc(far.rem_euclid(360.0)));
        assert!(arc(far).0.iter().any(|&a| a > 0));
        // A large arc round an ellipse 1e17 px across is flattened round
        // it, not to the one piece an f32's `1 - tol / r` made it - so it
        // is too wide to draw, and is left blank rather than handed over.
        let huge = [
            PathOp::MoveTo(Vec2::new(20.0, 80.0)),
            PathOp::ArcTo {
                rx: 3.6e17,
                ry: 5.0,
                rotation: 30.0,
                large: true,
                sweep: false,
                to: Vec2::new(20.0, 20.0),
            },
        ];
        let mut pts = Vec::new();
        flatten(&huge, &mut pts);
        assert!(pts.len() > 64, "{} points", pts.len());
        let m = rasterize(
            &huge,
            1.0,
            (0, 0),
            64,
            96,
            MaskPaint::Fill(FillRule::NonZero),
        );
        assert!(m.iter().all(|&a| a == 0));
        // The closed square's corner at its start is the corner opposite
        // it, to the coverage.
        let square = Path::parse("M10 10 L30 10 L30 30 L10 30 Z").unwrap();
        let m = rasterize(square.ops(), 1.0, (0, 0), 40, 40, MaskPaint::Stroke(2.0));
        // The pixels just inside each corner, and the ones just outside,
        // to a rasterizer's rounding: two ends meeting there added up to
        // half again.
        let near = |a: u8, b: u8| a.abs_diff(b) <= 4;
        assert!(near(m[10 * 40 + 10], m[29 * 40 + 29]));
        assert!(
            near(m[9 * 40 + 9], m[30 * 40 + 30]),
            "{} {}",
            m[9 * 40 + 9],
            m[30 * 40 + 30]
        );
    }

    /// A path that draws before it moves starts at its origin, in the
    /// stroke's flattening as in the fill's (backlog FZ2): the stroke's
    /// started at the mask's own origin, a quarter-pixel off at this bin,
    /// and an arc whose ends were 1e-41 apart went round an ellipse 1e25
    /// px tall from there.
    #[test]
    fn a_path_that_draws_before_it_moves_starts_at_its_origin() {
        let stroke = |d: &str| {
            let p = Path::parse(d).unwrap();
            rasterize(p.ops(), 1.0, (1, 3), 40, 40, MaskPaint::Stroke(2.0))
        };
        assert_eq!(stroke("L20 10 L30 30"), stroke("M0 0 L20 10 L30 30"));
        let arc = [
            PathOp::ArcTo {
                rx: 16.0,
                ry: 1.0e25,
                rotation: 0.0,
                large: true,
                sweep: true,
                to: Vec2::new(5.7e-41, 0.0),
            },
            PathOp::Close,
        ];
        let m = rasterize(&arc, 1.0, (1, 3), 48, 32, MaskPaint::Stroke(1.5));
        assert!(m.iter().all(|&a| a == 0));
    }

    #[test]
    fn parses_absolute_and_relative_commands_alike() {
        let a = Path::parse("M10 10 L20 10 L20 20 Z").unwrap();
        let b = Path::parse("m10,10 l10 0 l0 10 z").unwrap();
        assert_eq!(a.ops(), b.ops());
        assert_eq!(a.ops().len(), 4);
        // Implicit line-tos after a move, H and V, numbers run together.
        let c = Path::parse("M10 10 20 10V20z").unwrap();
        assert_eq!(a.ops(), c.ops());
        let d = Path::parse("M.5.5-1-1").unwrap();
        assert_eq!(
            d.ops(),
            &[
                PathOp::MoveTo(Vec2::new(0.5, 0.5)),
                PathOp::LineTo(Vec2::new(-1.0, -1.0))
            ]
        );
    }

    #[test]
    fn smooth_curves_reflect_the_last_control_point() {
        let p = Path::parse("M0 0 C 10 0 20 10 20 20 S 30 40 40 40").unwrap();
        match p.ops()[2] {
            PathOp::CubicTo(c1, c2, to) => {
                assert_eq!(c1, Vec2::new(20.0, 30.0));
                assert_eq!(c2, Vec2::new(30.0, 40.0));
                assert_eq!(to, Vec2::new(40.0, 40.0));
            }
            op => panic!("{op:?}"),
        }
        let q = Path::parse("M0 0 Q 10 10 20 0 T 40 0").unwrap();
        match q.ops()[2] {
            PathOp::QuadTo(c, to) => {
                assert_eq!(c, Vec2::new(30.0, -10.0));
                assert_eq!(to, Vec2::new(40.0, 0.0));
            }
            op => panic!("{op:?}"),
        }
    }

    #[test]
    fn a_malformed_string_names_its_byte() {
        let e = Path::parse("M10 10 L20").unwrap_err();
        assert_eq!(e.what, "a number");
        assert_eq!(e.at, 10);
        let e = Path::parse("10 10").unwrap_err();
        assert_eq!(e.what, "a command letter");
        let e = Path::parse("M0 0 X").unwrap_err();
        assert_eq!(e.what, "one of M L H V C S Q T A Z");
        let e = Path::parse("M0 0 A 5 5 0 2 0 10 10").unwrap_err();
        assert_eq!(e.what, "an arc flag (0 or 1)");
    }

    #[test]
    fn the_wire_form_round_trips() {
        let p = Path::parse("M1 2 L3 4 Q5 6 7 8 C9 10 11 12 13 14 A15 16 17 1 0 18 19 Z").unwrap();
        let f = p.to_floats();
        assert_eq!(f.len(), 3 + 3 + 5 + 7 + 8 + 1);
        assert_eq!(Path::from_floats(&f).unwrap().ops(), p.ops());
        assert!(Path::from_floats(&[9.0]).is_err());
        assert!(Path::from_floats(&[OP_LINE, 1.0]).is_err());
    }

    #[test]
    fn a_square_flattens_to_its_corners_and_hits_inside() {
        let p = Path::parse("M0 0 H10 V10 H0 Z").unwrap();
        let v = pts(p.ops());
        assert_eq!(v.len(), 5);
        assert!(v[4].x.is_nan());
        assert!(in_path(Vec2::new(5.0, 5.0), &v, FillRule::NonZero));
        assert!(in_path(Vec2::new(5.0, 5.0), &v, FillRule::EvenOdd));
        assert!(!in_path(Vec2::new(15.0, 5.0), &v, FillRule::NonZero));
        assert_eq!(bounds(&v), Some(Rect::new(0.0, 0.0, 10.0, 10.0)));
    }

    #[test]
    fn a_ring_is_a_ring_by_either_rule_when_its_contours_oppose() {
        // Outer clockwise, inner counter-clockwise (y down).
        let p = Path::parse("M0 0 H20 V20 H0 Z M5 5 V15 H15 V5 Z").unwrap();
        let v = pts(p.ops());
        let hole = Vec2::new(10.0, 10.0);
        let band = Vec2::new(2.0, 10.0);
        assert!(!in_path(hole, &v, FillRule::NonZero));
        assert!(!in_path(hole, &v, FillRule::EvenOdd));
        assert!(in_path(band, &v, FillRule::NonZero));
        assert!(in_path(band, &v, FillRule::EvenOdd));
        // Both the same way: nonzero fills the hole, even-odd does not.
        let same = Path::parse("M0 0 H20 V20 H0 Z M5 5 H15 V15 H5 Z").unwrap();
        let v = pts(same.ops());
        assert!(in_path(hole, &v, FillRule::NonZero));
        assert!(!in_path(hole, &v, FillRule::EvenOdd));
    }

    #[test]
    fn an_arc_flattens_onto_its_circle() {
        // A half circle of radius 10 from (0, 0) to (20, 0), bowing down.
        let p = Path::parse("M0 0 A10 10 0 0 1 20 0").unwrap();
        let v = pts(p.ops());
        assert!(v.len() >= 9, "{}", v.len());
        // Sweep 1 is the positive-angle direction, clockwise on screen
        // with y down: from the left end over the top to the right end.
        for q in v.iter().filter(|q| !q.x.is_nan()) {
            let r = ((q.x - 10.0).powi(2) + q.y.powi(2)).sqrt();
            assert!((r - 10.0).abs() < 0.3, "{q:?} is {r} from the centre");
            assert!(q.y <= 0.01, "{q:?} bows the wrong way");
        }
        assert_eq!(v[v.len() - 2], Vec2::new(20.0, 0.0));
        // The sweep flag picks the other half.
        let down = Path::parse("M0 0 A10 10 0 0 0 20 0").unwrap();
        assert!(
            pts(down.ops())
                .iter()
                .filter(|q| !q.x.is_nan())
                .all(|q| q.y >= -0.01)
        );
    }

    #[test]
    fn a_sector_is_a_wedge_and_a_full_turn_is_a_disc() {
        let w = Path::sector(50.0, 50.0, 40.0, 0.0, 0.0, 0.25);
        let v = pts(w.ops());
        assert!(in_path(Vec2::new(70.0, 70.0), &v, FillRule::NonZero));
        assert!(!in_path(Vec2::new(30.0, 30.0), &v, FillRule::NonZero));
        let d = Path::sector(50.0, 50.0, 40.0, 20.0, 0.0, 1.0);
        let v = pts(d.ops());
        assert!(in_path(Vec2::new(50.0, 20.0), &v, FillRule::NonZero));
        assert!(!in_path(Vec2::new(50.0, 50.0), &v, FillRule::NonZero));
    }

    #[test]
    fn a_draw_after_a_close_starts_where_the_subpath_began() {
        // The second triangle is (10,10) (10,40) (30,40): the hit outline
        // and the mask agree on it.
        let p = Path::parse("M10 10 H30 V30 Z L10 40 L30 40 Z").unwrap();
        let v = pts(p.ops());
        let m = rasterize(
            p.ops(),
            1.0,
            (0, 0),
            48,
            48,
            MaskPaint::Fill(FillRule::NonZero),
        );
        for (x, y) in [(12usize, 25usize), (28, 32), (28, 20), (14, 36)] {
            let hit = in_path(
                Vec2::new(x as f32 + 0.5, y as f32 + 0.5),
                &v,
                FillRule::NonZero,
            );
            assert_eq!(m[y * 48 + x] == 255, hit, "({x}, {y})");
        }
        assert_eq!(m[25 * 48 + 12], 255);
        assert_eq!(m[32 * 48 + 28], 0);
    }

    #[test]
    fn a_strokes_pieces_close_only_what_its_z_closed() {
        let open = Path::parse("M0 0 L10 0 L10 10").unwrap();
        let mut v = Vec::new();
        flatten_stroke(open.ops(), &mut v);
        assert_eq!(v.len(), 4);
        assert_eq!(v[2], Vec2::new(10.0, 10.0));
        assert!(v[3].x.is_nan());
        let closed = Path::parse("M0 0 L10 0 L10 10 Z").unwrap();
        v.clear();
        flatten_stroke(closed.ops(), &mut v);
        assert_eq!(v.len(), 5);
        assert_eq!(v[3], Vec2::ZERO);
        assert!(v[4].x.is_nan());
    }

    #[test]
    fn the_hash_follows_the_ops_and_nothing_else() {
        let a = Path::parse("M0 0 L10 0 L10 10 Z").unwrap();
        let b = Path::parse("M0 0 L10 0 L10 10 Z")
            .unwrap()
            .fill_rule(FillRule::EvenOdd);
        let c = Path::parse("M0 0 L10 0 L10 11 Z").unwrap();
        assert_eq!(hash_ops(a.ops()), hash_ops(b.ops()));
        assert_ne!(hash_ops(a.ops()), hash_ops(c.ops()));
    }

    #[test]
    fn a_filled_square_is_opaque_inside_and_bleeds_past_its_edge() {
        let p = Path::parse("M2 2 H12 V12 H2 Z").unwrap();
        let m = rasterize(
            p.ops(),
            1.0,
            (0, 0),
            16,
            16,
            MaskPaint::Fill(FillRule::NonZero),
        );
        let at = |x: usize, y: usize| m[y * 16 + x];
        assert_eq!(at(7, 7), 255);
        assert_eq!(at(0, 0), 0);
        assert_eq!(at(14, 7), 0);
        // The edge at x = 2 lands between pixels 1 and 2: pixel 1 is the
        // bleed's half, pixel 2 solid.
        assert!(at(1, 7) > 100 && at(1, 7) < 200, "{}", at(1, 7));
        assert_eq!(at(2, 7), 255);
        // Two squares sharing the edge x = 12 composite to full coverage
        // along it: the right one's left bleed over the left one's own.
        let q = Path::parse("M12 2 H22 V12 H12 Z").unwrap();
        let n = rasterize(
            q.ops(),
            1.0,
            (0, 0),
            24,
            16,
            MaskPaint::Fill(FillRule::NonZero),
        );
        let (a, b) = (
            f32::from(at(12, 7)) / 255.0,
            f32::from(n[7 * 24 + 12]) / 255.0,
        );
        assert!(a + b * (1.0 - a) > 0.99, "{a} over {b}");
        let (a, b) = (
            f32::from(at(11, 7)) / 255.0,
            f32::from(n[7 * 24 + 11]) / 255.0,
        );
        assert!(a + b * (1.0 - a) > 0.99, "{a} over {b}");
    }

    /// A dashed stroke's mask is the marks a dashed line would draw: the
    /// lengths seen are the ones declared, a short mark is a dot, and the
    /// offset moves them towards the start.
    #[test]
    fn a_dashed_stroke_is_marks_and_gaps() {
        let p = Path::parse("M4 8 H44").unwrap();
        let row = |dash: crate::line::Dash| {
            let paint = MaskPaint::Dashed(2.0, dash.cut(2.0).unwrap());
            let m = rasterize(p.ops(), 1.0, (0, 0), 48, 16, paint);
            // The row of pixels under the centre line, on or off.
            (0..48).map(|x| m[8 * 48 + x] > 127).collect::<Vec<_>>()
        };
        let on = |r: &[bool], x: std::ops::Range<usize>| r[x].iter().all(|&b| b);
        let off = |r: &[bool], x: std::ops::Range<usize>| r[x].iter().all(|&b| !b);
        // 6 on, 4 off from a cap before x = 4: marks cover 3..9, 13..19.
        let r = row(crate::line::Dash::new(6.0, 4.0));
        assert!(on(&r, 3..9) && off(&r, 10..12) && on(&r, 13..19), "{r:?}");
        // Dots 2 across, 8 apart.
        let r = row(crate::line::Dash::new(2.0, 6.0));
        assert!(on(&r, 3..5) && off(&r, 6..10) && on(&r, 11..13), "{r:?}");
        // 5 px in: the first mark's last pixel, then the gap.
        let r = row(crate::line::Dash::new(6.0, 4.0).offset(5.0));
        assert!(off(&r, 6..7) && on(&r, 8..14), "{r:?}");
    }

    #[test]
    fn a_stroke_covers_the_outline_and_not_the_inside() {
        let p = Path::parse("M2 2 H12 V12 H2 Z").unwrap();
        let m = rasterize(p.ops(), 1.0, (0, 0), 16, 16, MaskPaint::Stroke(2.0));
        let at = |x: usize, y: usize| m[y * 16 + x];
        assert_eq!(at(7, 7), 0);
        assert!(at(2, 7) > 200, "{}", at(2, 7));
        assert!(at(1, 7) > 200, "{}", at(1, 7));
    }

    #[test]
    fn the_store_boxes_a_path_and_keeps_the_ops_relative() {
        let mut store = PathStore::default();
        store.begin_frame(false);
        let p = Path::parse("M10 20 H30 V40 Z").unwrap();
        let (id, rect) = store
            .push(p.ops(), FillRule::NonZero, 0.0, None, None)
            .unwrap();
        assert_eq!(rect, Rect::new(8.0, 18.0, 24.0, 24.0));
        let (run, ops) = store.run(id);
        assert_eq!(ops[0], PathOp::MoveTo(Vec2::new(2.0, 2.0)));
        assert_eq!(run.len, 4);
        assert!(
            store
                .push(
                    &[PathOp::MoveTo(Vec2::ZERO)],
                    FillRule::NonZero,
                    0.0,
                    None,
                    None
                )
                .is_none()
        );
        // A stroke widens the box by half its width.
        let (_, rect) = store
            .push(p.ops(), FillRule::NonZero, 4.0, None, None)
            .unwrap();
        assert_eq!(rect, Rect::new(6.0, 16.0, 28.0, 28.0));
        // A turn boxes the path by the square it sweeps about its pivot:
        // the same box, ops and hash at any angle.
        let turn = |t: f32| Turn {
            turns: t,
            pivot: Some(Vec2::new(10.0, 20.0)),
        };
        let (a, ra) = store
            .push(p.ops(), FillRule::NonZero, 0.0, None, Some(turn(0.0)))
            .unwrap();
        let (b, rb) = store
            .push(p.ops(), FillRule::NonZero, 0.0, None, Some(turn(0.3)))
            .unwrap();
        let far = (20.0f32 * 20.0 + 20.0 * 20.0).sqrt() + 2.0;
        assert_eq!(ra, Rect::new(10.0 - far, 20.0 - far, 2.0 * far, 2.0 * far));
        assert_eq!(ra, rb);
        assert_eq!(store.run(a).0.hash, store.run(b).0.hash);
        assert_eq!(store.run(a).1, store.run(b).1);
        assert_eq!(store.run(b).0.angle, Some(0.3 * std::f32::consts::TAU));
        store.begin_frame(true);
        assert!(store.prev_run(id).is_some());
        assert!(store.is_empty());
    }

    /// A fill with no area paints nothing: the bleed is an edge's, and a
    /// progress ring at 0 has none.
    #[test]
    fn a_fill_with_no_area_is_an_empty_mask() {
        for p in [
            Path::sector(16.0, 16.0, 12.0, 8.0, 0.0, 0.0),
            Path::parse("M2 2 L20 2 Z").unwrap(),
        ] {
            let m = rasterize(
                p.ops(),
                1.0,
                (0, 0),
                32,
                32,
                MaskPaint::Fill(FillRule::NonZero),
            );
            assert!(m.iter().all(|&a| a == 0));
        }
    }
}
