//! Gradients: what a box's `gradient` row paints
//! (`docs/adr/0042-a-gradient-is-an-image-the-core-paints.md`).
//!
//! A gradient is a shape — a direction, or a centre — and two or more
//! stops, defined on the box's **unit square** and stretched to the box.
//! The core rasterizes it once into a small RGBA image in the atlas, keyed
//! by what it is and not by where it is drawn, and the box paints it with
//! one `Image` quad: a strip for a gradient along an axis, a square for
//! any other. So a renderer that draws an image draws a gradient, and a
//! box that resizes, or a thousand boxes that share one, rasterize
//! nothing.
//!
//! ```rust
//! use kui_core::{Color, Gradient, Side};
//!
//! let g = Gradient::to(Side::Bottom, [Color::hex(0x1e2030ff), Color::hex(0x14161eff)]);
//! assert_eq!(g.raster_size(), (1, 256));
//! ```

use crate::color::Color;
use crate::geom::Vec2;
use crate::value::Value;

/// One stop: a colour, and where along the gradient it sits, 0 to 1. A
/// stop with no position is spaced evenly between its neighbours that
/// have one, the first defaulting to 0 and the last to 1, as CSS spaces
/// them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stop {
    pub color: Color,
    pub at: Option<f32>,
}

impl From<Color> for Stop {
    fn from(color: Color) -> Self {
        Stop { color, at: None }
    }
}

impl From<(Color, f32)> for Stop {
    fn from((color, at): (Color, f32)) -> Self {
        Stop {
            color,
            at: Some(at),
        }
    }
}

/// Where a linear gradient runs to: a side or a corner of the box.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Right,
    BottomRight,
    Bottom,
    BottomLeft,
    Left,
    TopLeft,
    Top,
    TopRight,
}

impl Side {
    /// Every side with its spelling, clockwise from east — an eighth of a
    /// turn apart, which is what [`Self::turns`] reads off.
    pub const ALL: [(Side, &'static str); 8] = [
        (Side::Right, "right"),
        (Side::BottomRight, "bottom right"),
        (Side::Bottom, "bottom"),
        (Side::BottomLeft, "bottom left"),
        (Side::Left, "left"),
        (Side::TopLeft, "top left"),
        (Side::Top, "top"),
        (Side::TopRight, "top right"),
    ];

    /// The side as an angle: turns clockwise from east.
    pub fn turns(self) -> f32 {
        self as u8 as f32 / 8.0
    }

    /// `"bottom"`, `"top left"` — or the corner's words the other way
    /// round, as CSS takes them.
    pub fn parse(s: &str) -> Option<Side> {
        let mut words: Vec<&str> = s.split_whitespace().collect();
        words.sort_unstable();
        Side::ALL
            .iter()
            .find(|(_, name)| {
                let mut want: Vec<&str> = name.split(' ').collect();
                want.sort_unstable();
                want == words
            })
            .map(|&(side, _)| side)
    }
}

/// What a gradient runs along.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Shape {
    /// Along a direction — a unit vector, clockwise from east with y
    /// down — measured in the unit square: 0 at the corner it leaves, 1
    /// at the corner it reaches.
    Linear { dx: f32, dy: f32 },
    /// Out from a centre (fractions of the box) to the farthest corner:
    /// an ellipse once the square is stretched to the box.
    Radial { at: Vec2 },
}

/// A box's gradient: see the module's notes. Built by [`Gradient::to`],
/// [`Gradient::angle`] or [`Gradient::radial`], or read from plain data by
/// [`parse_with`].
#[derive(Clone, Debug, PartialEq)]
pub struct Gradient {
    shape: Shape,
    /// The stops with every position resolved, in order.
    stops: Stops,
    /// The shape and the stops as one number: the atlas slot's key.
    key: u64,
}

/// A gradient's stops: up to four in place — nearly every gradient a
/// view declares, and declares again every frame, so they cost it no
/// allocation — and a list past that.
#[derive(Clone, Debug)]
enum Stops {
    Few([(Color, f32); 4], u8),
    Many(Vec<(Color, f32)>),
}

impl Stops {
    fn push(&mut self, stop: (Color, f32)) {
        match self {
            Stops::Few(held, n) if (*n as usize) < held.len() => {
                held[*n as usize] = stop;
                *n += 1;
            }
            Stops::Few(held, _) => {
                let mut list = Vec::with_capacity(16);
                list.extend_from_slice(held);
                list.push(stop);
                *self = Stops::Many(list);
            }
            Stops::Many(list) => list.push(stop),
        }
    }
}

impl std::ops::Deref for Stops {
    type Target = [(Color, f32)];
    fn deref(&self) -> &Self::Target {
        match self {
            Stops::Few(held, n) => &held[..*n as usize],
            Stops::Many(list) => list,
        }
    }
}

impl std::ops::DerefMut for Stops {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            Stops::Few(held, n) => &mut held[..*n as usize],
            Stops::Many(list) => list,
        }
    }
}

impl PartialEq for Stops {
    fn eq(&self, other: &Self) -> bool {
        **self == **other
    }
}

/// Texels along a strip, for a gradient along an axis.
pub const STRIP: u32 = 256;
/// Texels on a side of the square every other gradient is drawn into.
pub const SQUARE: u32 = 128;

impl Gradient {
    /// A linear gradient towards a side or a corner of the box. To a
    /// corner the half-way line runs through the other two corners at any
    /// aspect, which is CSS's `to bottom right`.
    pub fn to<S: Into<Stop>>(side: Side, stops: impl IntoIterator<Item = S>) -> Self {
        Self::angle(side.turns(), stops)
    }

    /// A linear gradient along `turns`, clockwise from east (0.25 is
    /// downwards), measured in the box's unit square — so an eighth of a
    /// turn runs corner to corner whatever the box's aspect, which CSS's
    /// `45deg`, measured in pixels, does not.
    pub fn angle<S: Into<Stop>>(turns: f32, stops: impl IntoIterator<Item = S>) -> Self {
        let (dx, dy) = direction(turns);
        Self::new(Shape::Linear { dx, dy }, stops)
    }

    /// A radial gradient from the middle of the box out to its corners.
    pub fn radial<S: Into<Stop>>(stops: impl IntoIterator<Item = S>) -> Self {
        Self::radial_at(Vec2::new(0.5, 0.5), stops)
    }

    /// A radial gradient from `at` — fractions of the box, `(0.5, 0)` the
    /// middle of its top edge — out to the corner farthest from it.
    pub fn radial_at<S: Into<Stop>>(at: Vec2, stops: impl IntoIterator<Item = S>) -> Self {
        Self::new(Shape::Radial { at }, stops)
    }

    fn new<S: Into<Stop>>(shape: Shape, stops: impl IntoIterator<Item = S>) -> Self {
        // One list, built once: a stop with no position waits as a NaN
        // for `resolve` to place it.
        let mut list = Stops::Few([(Color::TRANSPARENT, 0.0); 4], 0);
        for s in stops {
            let s: Stop = s.into();
            let at = s.at.filter(|a| a.is_finite()).map(|a| a.clamp(0.0, 1.0));
            list.push((s.color, at.unwrap_or(f32::NAN)));
        }
        let mut stops = list;
        resolve(&mut stops);
        let mut g = Gradient {
            shape,
            stops,
            key: 0,
        };
        g.key = g.hash();
        g
    }

    /// Whether there is anything to paint: two stops or more, and a shape
    /// made of numbers.
    pub fn is_drawable(&self) -> bool {
        let shape = match self.shape {
            Shape::Linear { dx, dy } => dx.is_finite() && dy.is_finite(),
            Shape::Radial { at } => at.x.is_finite() && at.y.is_finite(),
        };
        shape && self.stops.len() >= 2
    }

    /// The key its raster is held under: equal for equal gradients,
    /// whatever box draws them.
    pub fn key(&self) -> u64 {
        self.key
    }

    /// The stops as resolved: a colour and its position, in order.
    pub fn stops(&self) -> &[(Color, f32)] {
        &self.stops
    }

    fn hash(&self) -> u64 {
        const PRIME: u64 = 0x0000_0100_0000_01b3;
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut mix = |v: u32| {
            h ^= u64::from(v);
            h = h.wrapping_mul(PRIME);
        };
        match self.shape {
            // The direction and not the angle, so a whole turn more is
            // the same key.
            Shape::Linear { dx, dy } => {
                mix(1);
                mix(dx.to_bits());
                mix(dy.to_bits());
            }
            Shape::Radial { at } => {
                mix(2);
                mix(at.x.to_bits());
                mix(at.y.to_bits());
            }
        }
        for (c, at) in self.stops.iter() {
            for lane in c.lanes() {
                mix(lane.to_bits());
            }
            mix(at.to_bits());
        }
        h
    }

    /// The raster's size in texels: a strip along the axis the gradient
    /// runs on, or the square.
    pub fn raster_size(&self) -> (u32, u32) {
        match self.shape {
            Shape::Linear { dy: 0.0, .. } => (STRIP, 1),
            Shape::Linear { dx: 0.0, .. } => (1, STRIP),
            Shape::Linear { .. } => (SQUARE, SQUARE),
            Shape::Radial { .. } => (SQUARE, SQUARE),
        }
    }

    /// The texels its slot takes: [`Self::raster_size`] and a gutter of
    /// one all round.
    pub fn slot_size(&self) -> (u32, u32) {
        let (w, h) = self.raster_size();
        (w + 2, h + 2)
    }

    /// The raster: [`Self::slot_size`] texels of straight RGBA, row after
    /// row, each the gradient at its texel's centre. The quad draws the
    /// inner [`Self::raster_size`] of them; the gutter is the gradient
    /// carried on a texel past each edge, because the quad is stretched
    /// and sampled linearly, and at the box's edge the sampler reads half
    /// a texel past the rect it was given — a strip one texel high would
    /// otherwise fade into whatever the atlas holds above and below it.
    pub fn rasterize(&self) -> Vec<u8> {
        let (w, h) = self.raster_size();
        let mut out = Vec::with_capacity(((w + 2) * (h + 2) * 4) as usize);
        if w.min(h) == 1 {
            // A strip: each texel its own mix.
            for row in 0..h + 2 {
                let y = (row as f32 - 0.5) / h as f32;
                for col in 0..w + 2 {
                    let x = (col as f32 - 0.5) / w as f32;
                    out.extend_from_slice(&bytes(self.color_at(self.along(x, y))));
                }
            }
            return out;
        }
        // A square: seventeen thousand texels of at most a few hundred
        // different colours. The ramp is mixed once, finely, and each
        // texel reads it — a quarter of an 8-bit level off at the most.
        const RAMP: usize = 1024;
        let ramp: Vec<[u8; 4]> = (0..=RAMP)
            .map(|i| bytes(self.color_at(i as f32 / RAMP as f32)))
            .collect();
        for row in 0..h + 2 {
            let y = (row as f32 - 0.5) / h as f32;
            for col in 0..w + 2 {
                let x = (col as f32 - 0.5) / w as f32;
                let t = self.along(x, y).clamp(0.0, 1.0);
                out.extend_from_slice(&ramp[(t * RAMP as f32 + 0.5) as usize]);
            }
        }
        out
    }

    /// The colour at a point of the box, as fractions of it: `(0, 0)`
    /// its top left, `(1, 1)` its bottom right. What the raster samples,
    /// and what a test of the stretched raster compares it with.
    pub fn color_in(&self, x: f32, y: f32) -> Color {
        self.color_at(self.along(x, y))
    }

    /// How far along the gradient a point of the box is.
    #[inline]
    fn along(&self, x: f32, y: f32) -> f32 {
        match self.shape {
            Shape::Linear { dx, dy } => {
                // The corners the direction leaves and reaches are this
                // far apart along it, in the unit square.
                let len = dx.abs() + dy.abs();
                ((x - 0.5) * dx + (y - 0.5) * dy) / len + 0.5
            }
            Shape::Radial { at } => {
                let far = (at.x.max(1.0 - at.x)).hypot(at.y.max(1.0 - at.y));
                let (rx, ry) = (x - at.x, y - at.y);
                (rx * rx + ry * ry).sqrt() / if far > 0.0 { far } else { 1.0 }
            }
        }
    }

    /// The colour `t` of the way along: the stops either side mixed in
    /// straight sRGB with the alpha premultiplied, as CSS mixes them, so
    /// a fade to transparent does not pass through grey. Before the first
    /// stop it is the first, past the last the last.
    pub fn color_at(&self, t: f32) -> Color {
        let stops = &self.stops;
        let Some(&(first, first_at)) = stops.first() else {
            return Color::TRANSPARENT;
        };
        if t.is_nan() || t <= first_at {
            return first;
        }
        for pair in stops.windows(2) {
            let ((a, a_at), (b, b_at)) = (pair[0], pair[1]);
            if t <= b_at {
                let span = b_at - a_at;
                let u = if span > 0.0 { (t - a_at) / span } else { 1.0 };
                let alpha = a.a + (b.a - a.a) * u;
                if alpha <= 0.0 {
                    // Nothing shows; keep a colour the sampler can mix
                    // towards without darkening the texel beside it.
                    return Color {
                        a: 0.0,
                        ..a.lerp(b, u)
                    };
                }
                let lane = |a_c: f32, b_c: f32| (a_c * a.a + (b_c * b.a - a_c * a.a) * u) / alpha;
                return Color {
                    r: lane(a.r, b.r),
                    g: lane(a.g, b.g),
                    b: lane(a.b, b.b),
                    a: alpha,
                };
            }
        }
        stops[stops.len() - 1].0
    }
}

fn bytes(c: Color) -> [u8; 4] {
    [c.r, c.g, c.b, c.a].map(|lane| (lane.clamp(0.0, 1.0) * 255.0).round() as u8)
}

/// The unit vector `turns` clockwise from east. An eighth of a turn is
/// read off a table — the sides and corners, exactly on their axes and
/// diagonals and with no trigonometry, which is every gradient a `to`
/// spells — and anything else is its cosine and sine.
fn direction(turns: f32) -> (f32, f32) {
    const D: f32 = std::f32::consts::FRAC_1_SQRT_2;
    const EIGHTHS: [(f32, f32); 8] = [
        (1.0, 0.0),
        (D, D),
        (0.0, 1.0),
        (-D, D),
        (-1.0, 0.0),
        (-D, -D),
        (0.0, -1.0),
        (D, -D),
    ];
    let eighths = turns.rem_euclid(1.0) * 8.0;
    if eighths == eighths.round() {
        return EIGHTHS[eighths as usize & 7];
    }
    let a = eighths * (std::f32::consts::TAU / 8.0);
    (a.cos(), a.sin())
}

/// Fills in the positions `Gradient::new` left as NaN: a first with none
/// is 0, a last with none 1, a run with none is spaced evenly between its
/// neighbours, and a position behind the one before it is raised to it.
fn resolve(stops: &mut [(Color, f32)]) {
    let n = stops.len();
    if n == 0 {
        return;
    }
    if stops[0].1.is_nan() {
        stops[0].1 = 0.0;
    }
    if stops[n - 1].1.is_nan() {
        stops[n - 1].1 = 1.0;
    }
    let mut floor = 0.0f32;
    for (_, at) in stops.iter_mut().filter(|s| !s.1.is_nan()) {
        floor = floor.max(*at);
        *at = floor;
    }
    let mut i = 0;
    while i + 1 < n {
        // The next placed stop; the unplaced ones between are spaced
        // evenly up to it.
        let j = (i + 1..n).find(|&j| !stops[j].1.is_nan()).unwrap_or(n - 1);
        let (here, there) = (stops[i].1, stops[j].1);
        for (n, stop) in stops[i + 1..j].iter_mut().enumerate() {
            stop.1 = here + (there - here) * ((n + 1) as f32 / (j - i) as f32);
        }
        i = j;
    }
}

/// Reads a gradient from plain data, the form every binding carries:
///
/// - `{ to = "bottom", stops = {…} }` — a side or a corner ([`Side`]);
/// - `{ angle = 0.125, stops = {…} }` — turns clockwise from east;
/// - `{ radial = true, at = {0.5, 0}, stops = {…} }` — `at` optional.
///
/// No `to`, `angle` or `radial` is `to = "bottom"`. A stop is a colour —
/// `0xRRGGBBAA`, `"#hex"`, or with `refs` a `$name` — or a
/// `{colour, position}` pair. A `$name` that misses is remembered on the
/// refs for the binding to raise, and its stop is left out.
pub fn parse_with(
    v: &Value,
    mut refs: Option<&mut crate::tokens::NameRefs<'_>>,
) -> Result<Gradient, String> {
    let Value::Map(fields) = v else {
        return Err("gradient must be an object with stops".into());
    };
    let num = |v: &Value, what: &str| {
        v.as_float()
            .map(|n| n as f32)
            .filter(|n| n.is_finite())
            .ok_or_else(|| format!("gradient: {what} must be a number"))
    };
    let (mut to, mut angle, mut radial, mut at, mut stops) = (None, None, false, None, None);
    for (k, v) in fields {
        match k.as_str() {
            "to" => {
                let side = v.as_str().and_then(Side::parse).ok_or_else(|| {
                    "gradient: `to` is a side or a corner (\"bottom\", \"top right\", …)"
                        .to_string()
                })?;
                to = Some(side);
            }
            "angle" => angle = Some(num(v, "`angle`")?),
            "radial" => radial = v.as_bool().unwrap_or(false),
            "at" => match v.as_list() {
                Some([x, y]) => at = Some(Vec2::new(num(x, "`at`")?, num(y, "`at`")?)),
                _ => return Err("gradient: `at` is an [x, y] pair".into()),
            },
            "stops" => stops = v.as_list(),
            other => return Err(format!("gradient: unknown field `{other}`")),
        }
    }
    let Some(list) = stops else {
        return Err("gradient needs `stops`, a list of colours".into());
    };
    let mut color = |v: &Value, i: usize| -> Result<Option<Color>, String> {
        if let Some(hit) = refs.as_deref_mut().and_then(|r| r.color_ref(v)) {
            return Ok(hit);
        }
        crate::slots::color_value(v)
            .map(Some)
            .map_err(|e| format!("gradient stop {i}: {e}"))
    };
    let mut out = Vec::with_capacity(list.len());
    for (i, stop) in list.iter().enumerate() {
        let (c, at) = match stop {
            Value::List(pair) => match pair.as_slice() {
                [c, at] => (
                    color(c, i)?,
                    Some(num(at, &format!("stop {i}'s position"))?),
                ),
                _ => {
                    return Err(format!(
                        "gradient stop {i}: a colour or a [colour, at] pair"
                    ));
                }
            },
            c => (color(c, i)?, None),
        };
        if let Some(color) = c {
            out.push(Stop { color, at });
        }
    }
    if list.len() < 2 {
        return Err("gradient needs two stops or more".into());
    }
    let linear = |turns: f32| {
        let (dx, dy) = direction(turns);
        Shape::Linear { dx, dy }
    };
    let shape = match (radial, to, angle) {
        (true, None, None) => Shape::Radial {
            at: at.unwrap_or(Vec2::new(0.5, 0.5)),
        },
        (false, Some(side), None) => linear(side.turns()),
        (false, None, Some(turns)) => linear(turns),
        (false, None, None) => linear(Side::Bottom.turns()),
        _ => return Err("gradient: one of `to`, `angle` and `radial`".into()),
    };
    if at.is_some() && !radial {
        return Err("gradient: `at` is a radial gradient's centre".into());
    }
    Ok(Gradient::new(shape, out))
}

/// [`parse_with`] with no token lookup: a `$name` is an error.
pub fn parse(v: &Value) -> Result<Gradient, String> {
    parse_with(v, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED: Color = Color {
        r: 1.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    const BLUE: Color = Color {
        r: 0.0,
        g: 0.0,
        b: 1.0,
        a: 1.0,
    };

    fn texel(g: &Gradient, x: u32, y: u32) -> [u8; 4] {
        // Past the gutter.
        let (w, _) = g.slot_size();
        let px = g.rasterize();
        let i = (((y + 1) * w + x + 1) * 4) as usize;
        [px[i], px[i + 1], px[i + 2], px[i + 3]]
    }

    #[test]
    fn a_side_is_a_strip_and_runs_the_way_it_says() {
        let right = Gradient::to(Side::Right, [RED, BLUE]);
        assert_eq!(right.raster_size(), (STRIP, 1));
        assert_eq!(texel(&right, 0, 0), [255, 0, 0, 255]);
        assert_eq!(texel(&right, 255, 0), [0, 0, 255, 255]);
        assert_eq!(texel(&right, 128, 0), [127, 0, 128, 255]);
        let left = Gradient::to(Side::Left, [RED, BLUE]);
        assert_eq!(left.raster_size(), (STRIP, 1));
        assert_eq!(texel(&left, 0, 0), [0, 0, 255, 255]);
        let up = Gradient::to(Side::Top, [RED, BLUE]);
        assert_eq!(up.raster_size(), (1, STRIP));
        assert_eq!(texel(&up, 0, 255), [255, 0, 0, 255]);
        // A whole turn on is the same gradient.
        assert_eq!(
            Gradient::angle(1.25, [RED, BLUE]).key(),
            Gradient::to(Side::Bottom, [RED, BLUE]).key()
        );
    }

    /// To a corner: red where it leaves, blue where it arrives, and the
    /// half-way line through the other two corners.
    #[test]
    fn a_corner_runs_corner_to_corner_in_the_unit_square() {
        let g = Gradient::to(Side::BottomRight, [RED, BLUE]);
        assert_eq!(g.raster_size(), (SQUARE, SQUARE));
        let n = SQUARE - 1;
        let [r, _, b, _] = texel(&g, 0, 0);
        assert!(r > 250 && b < 5, "{r} {b}");
        let [r, _, b, _] = texel(&g, n, n);
        assert!(r < 5 && b > 250, "{r} {b}");
        for (x, y) in [(n, 0), (0, n)] {
            let [r, _, b, _] = texel(&g, x, y);
            assert!(r.abs_diff(b) <= 1, "{r} {b}");
        }
    }

    #[test]
    fn a_radial_reaches_its_farthest_corner() {
        let g = Gradient::radial([RED, BLUE]);
        let mid = SQUARE / 2;
        let [r, _, b, _] = texel(&g, mid, mid);
        assert!(r > 250 && b < 5);
        let [r, _, b, _] = texel(&g, 0, 0);
        assert!(r < 5 && b > 250);
        // From the top edge the bottom corners are the far ones.
        let top = Gradient::radial_at(Vec2::new(0.5, 0.0), [RED, BLUE]);
        let [r, _, b, _] = texel(&top, 0, SQUARE - 1);
        assert!(r < 5 && b > 250);
        assert_ne!(top.key(), g.key());
    }

    #[test]
    fn stops_without_a_position_are_spaced_between_those_with() {
        let g = Gradient::to(
            Side::Right,
            [
                Stop::from(RED),
                Stop::from(BLUE),
                Stop::from((RED, 0.5)),
                Stop::from(BLUE),
                Stop::from(RED),
            ],
        );
        let at: Vec<f32> = g.stops().iter().map(|s| s.1).collect();
        assert_eq!(at, [0.0, 0.25, 0.5, 0.75, 1.0]);
        // One behind the stop before it is raised to it: a hard edge.
        let g = Gradient::to(Side::Right, [(RED, 0.6), (BLUE, 0.4)]);
        let at: Vec<f32> = g.stops().iter().map(|s| s.1).collect();
        assert_eq!(at, [0.6, 0.6]);
        assert_eq!(g.color_at(0.5), RED);
        assert_eq!(g.color_at(0.7), BLUE);
    }

    /// Red fading out: half-way it is still red, at half the alpha — not
    /// the dark red a straight mix with transparent black would give.
    #[test]
    fn a_fade_to_transparent_keeps_its_colour() {
        let g = Gradient::to(Side::Right, [RED, Color::TRANSPARENT]);
        let c = g.color_at(0.5);
        assert_eq!((c.r, c.g, c.b, c.a), (1.0, 0.0, 0.0, 0.5));
    }

    #[test]
    fn one_stop_or_a_number_that_is_not_one_draws_nothing() {
        assert!(!Gradient::to(Side::Right, [RED]).is_drawable());
        assert!(!Gradient::angle(f32::NAN, [RED, BLUE]).is_drawable());
        assert!(Gradient::to(Side::Right, [RED, BLUE]).is_drawable());
    }

    #[test]
    fn plain_data_reads_as_the_builders_do() {
        let stops = || Value::list([Value::str("#ff0000"), Value::Int(0x0000ffff)]);
        let g = parse(&Value::map([
            ("to", Value::str("right bottom")),
            ("stops", stops()),
        ]));
        assert_eq!(g, Ok(Gradient::to(Side::BottomRight, [RED, BLUE])));
        let g = parse(&Value::map([("stops", stops())]));
        assert_eq!(g, Ok(Gradient::to(Side::Bottom, [RED, BLUE])));
        let g = parse(&Value::map([
            ("radial", Value::Bool(true)),
            ("at", Value::floats(&[0.5, 0.0])),
            (
                "stops",
                Value::list([
                    Value::str("#ff0000"),
                    Value::list([Value::str("#0000ff"), Value::float(0.8)]),
                ]),
            ),
        ]));
        assert_eq!(
            g,
            Ok(Gradient::radial_at(
                Vec2::new(0.5, 0.0),
                [Stop::from(RED), Stop::from((BLUE, 0.8))]
            ))
        );
        let bad = |v: Value| parse(&v).unwrap_err();
        assert!(bad(Value::map([("to", Value::str("up")), ("stops", stops())])).contains("side"));
        assert!(
            bad(Value::map([("stops", Value::list([Value::str("#fff")]))])).contains("two stops")
        );
        assert!(bad(Value::map([("angle", Value::float(0.1))])).contains("needs `stops`"));
        assert!(
            bad(Value::map([
                ("angle", Value::float(0.1)),
                ("radial", Value::Bool(true)),
                ("stops", stops())
            ]))
            .contains("one of")
        );
    }
}
