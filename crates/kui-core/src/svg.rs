//! The `svg` element's core (ADR 0063): an SVG parsed once by `usvg`,
//! drawn as kui paths when every shape is one `path` can draw — crisp at
//! any size, recoloured by the element's `color` — and rasterized by
//! `resvg` into an image when it is not (a gradient, a mask, a filter, an
//! embedded picture, a butt cap).
//!
//! `currentColor` is the element's `color`: the document is parsed with a
//! style sheet that sets `color` on the root to a colour no design uses,
//! and a paint of that colour is the element's (with its opacity) — so an
//! icon set drawn `stroke="currentColor"` takes the theme's ink, a token,
//! or whatever the view says.
//!
//! ```rust
//! use kui_core::svg::{Svg, SvgFit, SvgMode};
//!
//! let icon = Svg::parse(br#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor"
//!     stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
//!   <path d="M5 12h14"/><circle cx="12" cy="12" r="10"/></svg>"#).unwrap();
//! assert_eq!(icon.intrinsic(), (24.0, 24.0));
//! assert!(icon.drawable_as_paths(), "{:?}", icon.why_image());
//! let shapes = icon.shapes(16.0, 16.0, SvgFit::Contain, kui_core::Color::WHITE);
//! assert_eq!(shapes.len(), 2);
//! let _ = SvgMode::Auto;
//! ```

use crate::color::Color;
use crate::path::Path;

/// How the element draws.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SvgMode {
    /// Paths when every shape is drawable as one, an image otherwise.
    #[default]
    Auto,
    /// Always paths: what cannot be drawn as one (a gradient's fill, a
    /// clip) is approximated or left out, with a warning once.
    Paths,
    /// Always an image, rasterized at the drawn size and scale.
    Image,
}

impl SvgMode {
    /// Every mode, in the order the `mode` row names them.
    pub const ALL: [SvgMode; 3] = [SvgMode::Auto, SvgMode::Paths, SvgMode::Image];
    pub const NAMES: [&'static str; 3] = ["auto", "paths", "image"];
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "auto" => Some(Self::Auto),
            "paths" => Some(Self::Paths),
            "image" => Some(Self::Image),
            _ => None,
        }
    }
    pub fn name(self) -> &'static str {
        Self::NAMES[self as usize]
    }
}

/// How the picture's viewBox sits in the element's box.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SvgFit {
    /// Whole, centred, the aspect kept.
    #[default]
    Contain,
    /// Filling the box, centred, the aspect kept, the overflow clipped.
    Cover,
    /// Stretched to the box.
    Fill,
}

impl SvgFit {
    /// Every fit, in the order the `fit` row names them.
    pub const ALL: [SvgFit; 3] = [SvgFit::Contain, SvgFit::Cover, SvgFit::Fill];
    pub const NAMES: [&'static str; 3] = ["contain", "cover", "fill"];
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "contain" => Some(Self::Contain),
            "cover" => Some(Self::Cover),
            "fill" => Some(Self::Fill),
            _ => None,
        }
    }
    pub fn name(self) -> &'static str {
        Self::NAMES[self as usize]
    }
}

/// Why an SVG would not parse.
#[derive(Clone, Debug, PartialEq)]
pub struct SvgError(pub String);

impl std::fmt::Display for SvgError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for SvgError {}

/// How far from the origin, in device px, a rasterized picture may reach:
/// tiny-skia's anti-aliased scan converter works in 16.16 fixed point and
/// panics on a stroke past it (found by the `svg` fuzz target).
pub const RASTER_REACH: f32 = 30_000.0;

/// One shape at a size: the kui path (with its stroke) and its fill.
#[derive(Clone, Debug)]
pub struct SvgShape {
    pub path: Path,
    pub fill: Option<Color>,
}

/// The parser and the rasterizer, behind the default `svg` feature.
#[cfg(feature = "svg")]
mod real {
    use std::sync::Arc;

    use super::{RASTER_REACH, SvgError, SvgFit, SvgShape};
    use crate::color::Color;
    use crate::geom::Vec2;
    use crate::line::{Dash, Stroke};
    use crate::path::{FillRule, Path, PathOp};

    /// The colour `currentColor` is parsed as, read back as the element's.
    const SENTINEL: (u8, u8, u8) = (0xfe, 0x01, 0xfe);
    const SENTINEL_CSS: &str = "svg { color: #fe01fe; }";

    #[derive(Clone, Copy, Debug, PartialEq)]
    enum Ink {
        Rgb(u8, u8, u8),
        Current,
    }

    #[derive(Clone, Debug)]
    struct Shape {
        /// In the document's user space, every transform applied.
        ops: Vec<PathOp>,
        rule: FillRule,
        fill: Option<(Ink, f32)>,
        /// Ink, opacity, width in user space, dash.
        stroke: Option<(Ink, f32, f32, Option<Dash>)>,
    }

    /// An SVG parsed once: its intrinsic size, the shapes it draws as paths,
    /// and, when it cannot draw as paths, why.
    pub struct Svg {
        source: Arc<[u8]>,
        tree: usvg::Tree,
        size: (f32, f32),
        shapes: Vec<Shape>,
        image_because: Option<String>,
        approximations: Vec<String>,
    }

    impl std::fmt::Debug for Svg {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("Svg")
                .field("size", &self.size)
                .field("shapes", &self.shapes.len())
                .field("image_because", &self.image_because)
                .finish()
        }
    }

    fn options() -> usvg::Options<'static> {
        usvg::Options {
            style_sheet: Some(SENTINEL_CSS.into()),
            resources_dir: None,
            ..usvg::Options::default()
        }
    }

    impl Svg {
        /// Parses an SVG document. Never reads a file or the network: an
        /// `<image>` it names by path is left out (and the picture is drawn
        /// as an image without it), only a `data:` one is kept.
        pub fn parse(data: &[u8]) -> Result<Svg, SvgError> {
            let tree = usvg::Tree::from_data(data, &options())
                .map_err(|e| SvgError(format!("svg: {e}")))?;
            let size = (tree.size().width(), tree.size().height());
            let mut shapes = Vec::new();
            let mut image_because = None;
            let mut approximations = Vec::new();
            walk(
                tree.root(),
                1.0,
                &mut shapes,
                &mut image_because,
                &mut approximations,
            );
            Ok(Svg {
                source: Arc::from(data),
                tree,
                size,
                shapes,
                image_because,
                approximations,
            })
        }

        /// The width and height the document says it is, in its own units:
        /// what the element is when the view gives it no size.
        pub fn intrinsic(&self) -> (f32, f32) {
            self.size
        }

        /// Whether every shape is one kui's `path` draws exactly.
        pub fn drawable_as_paths(&self) -> bool {
            self.image_because.is_none()
        }

        /// Why `auto` draws this as an image, for a warning or a devtools row.
        pub fn why_image(&self) -> Option<&str> {
            self.image_because.as_deref()
        }

        /// What `paths` mode approximates or leaves out, when forced.
        pub fn approximations(&self) -> &[String] {
            &self.approximations
        }

        /// The scale and offset that put the viewBox into a `w` × `h` box.
        pub(super) fn place(&self, w: f32, h: f32, fit: SvgFit) -> (f32, f32, f32, f32) {
            let (iw, ih) = (self.size.0.max(1e-6), self.size.1.max(1e-6));
            let (sx, sy) = match fit {
                SvgFit::Fill => (w / iw, h / ih),
                SvgFit::Contain => {
                    let s = (w / iw).min(h / ih);
                    (s, s)
                }
                SvgFit::Cover => {
                    let s = (w / iw).max(h / ih);
                    (s, s)
                }
            };
            ((w - iw * sx) / 2.0, (h - ih * sy) / 2.0, sx, sy)
        }

        /// The shapes as kui paths in a `w` × `h` box, `currentColor` painted
        /// `color`, strokes as wide as the scale makes them.
        pub fn shapes(&self, w: f32, h: f32, fit: SvgFit, color: Color) -> Vec<SvgShape> {
            let (ox, oy, sx, sy) = self.place(w, h, fit);
            let tf = |p: Vec2| Vec2::new(ox + p.x * sx, oy + p.y * sy);
            let ink = |i: Ink, a: f32| match i {
                Ink::Current => color.with_alpha(color.a * a),
                Ink::Rgb(r, g, b) => Color::rgba8(r, g, b, 255).with_alpha(a),
            };
            let stroke_scale = (sx * sy).abs().sqrt();
            self.shapes
                .iter()
                .map(|s| {
                    let ops = s
                        .ops
                        .iter()
                        .map(|op| match *op {
                            PathOp::MoveTo(p) => PathOp::MoveTo(tf(p)),
                            PathOp::LineTo(p) => PathOp::LineTo(tf(p)),
                            PathOp::QuadTo(c, p) => PathOp::QuadTo(tf(c), tf(p)),
                            PathOp::CubicTo(a, b, p) => PathOp::CubicTo(tf(a), tf(b), tf(p)),
                            other => other,
                        })
                        .collect();
                    let mut path = Path::from_ops(ops).fill_rule(s.rule);
                    if let Some((i, a, width, dash)) = s.stroke {
                        let mut st = Stroke::new(width * stroke_scale, ink(i, a));
                        if let Some(d) = dash {
                            st = st.dashed(Dash {
                                pattern: d.pattern.map(|v| v * stroke_scale),
                                offset: d.offset * stroke_scale,
                            });
                        }
                        path = path.stroked(st);
                    }
                    SvgShape {
                        path,
                        fill: s.fill.map(|(i, a)| ink(i, a)),
                    }
                })
                .collect()
        }

        /// Whether the picture drawn at `w` × `h` logical px times `scale`
        /// reaches past [`RASTER_REACH`] device px from the origin — a shape
        /// spelled with a coordinate like `8e8` — which the rasterizer's
        /// fixed-point scan converter cannot hold. [`Self::raster`] draws such
        /// a picture as nothing rather than handing it over.
        pub fn reaches_past_raster(&self, w: f32, h: f32, scale: f32, fit: SvgFit) -> bool {
            let b = self.tree.root().abs_layer_bounding_box();
            let (ox, oy, sx, sy) = self.place(w, h, fit);
            let xs = [ox + b.left() * sx, ox + b.right() * sx];
            let ys = [oy + b.top() * sy, oy + b.bottom() * sy];
            xs.iter()
                .chain(ys.iter())
                .any(|v| !(v * scale).is_finite() || (v * scale).abs() > RASTER_REACH)
        }

        /// The picture rasterized at `w` × `h` logical px times `scale`, as
        /// straight-alpha RGBA rows, `currentColor` painted `color`: what the
        /// image mode draws. A picture that [reaches past what the rasterizer
        /// holds](Self::reaches_past_raster) comes back transparent.
        pub fn raster(
            &self,
            w: f32,
            h: f32,
            scale: f32,
            fit: SvgFit,
            color: Color,
        ) -> (u32, u32, Vec<u8>) {
            let pw = ((w * scale).round() as u32).clamp(1, 8192);
            let ph = ((h * scale).round() as u32).clamp(1, 8192);
            // `currentColor` resolved to the colour asked for, so the raster
            // needs no recolouring pass.
            let hex = color.to_hex();
            let css = format!("svg {{ color: #{:06x}; }}", hex >> 8);
            let recoloured = usvg::Tree::from_data(
                &self.source,
                &usvg::Options {
                    style_sheet: Some(css),
                    resources_dir: None,
                    ..usvg::Options::default()
                },
            );
            let tree = recoloured.as_ref().unwrap_or(&self.tree);
            if self.reaches_past_raster(w, h, scale, fit) {
                return (pw, ph, vec![0; (pw * ph * 4) as usize]);
            }
            let Some(mut pix) = resvg::tiny_skia::Pixmap::new(pw, ph) else {
                return (1, 1, vec![0; 4]);
            };
            let (ox, oy, sx, sy) = self.place(w, h, fit);
            let t = resvg::tiny_skia::Transform::from_row(
                sx * scale,
                0.0,
                0.0,
                sy * scale,
                ox * scale,
                oy * scale,
            );
            resvg::render(tree, t, &mut pix.as_mut());
            let mut rgba = pix.take();
            for px in rgba.as_chunks_mut::<4>().0 {
                let a = px[3] as u32;
                if a != 0 && a != 255 {
                    for c in &mut px[..3] {
                        *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
                    }
                }
            }
            // The colour's own alpha, on top.
            if color.a < 1.0 {
                for px in rgba.as_chunks_mut::<4>().0 {
                    px[3] = (px[3] as f32 * color.a).round() as u8;
                }
            }
            (pw, ph, rgba)
        }
    }

    fn ink(p: &usvg::Paint) -> Option<Ink> {
        match p {
            usvg::Paint::Color(c) if (c.red, c.green, c.blue) == SENTINEL => Some(Ink::Current),
            usvg::Paint::Color(c) => Some(Ink::Rgb(c.red, c.green, c.blue)),
            _ => None,
        }
    }

    /// Notes why the picture needs the image mode: the first reason is what
    /// `auto` says, every reason is what `paths` approximates.
    fn note(image_because: &mut Option<String>, approx: &mut Vec<String>, why: &str) {
        if image_because.is_none() {
            *image_because = Some(why.to_string());
        }
        if !approx.iter().any(|a| a == why) {
            approx.push(why.to_string());
        }
    }

    fn walk(
        g: &usvg::Group,
        opacity: f32,
        shapes: &mut Vec<Shape>,
        image_because: &mut Option<String>,
        approx: &mut Vec<String>,
    ) {
        if g.clip_path().is_some() {
            note(image_because, approx, "a clip path");
        }
        if g.mask().is_some() {
            note(image_because, approx, "a mask");
        }
        if !g.filters().is_empty() {
            note(image_because, approx, "a filter");
        }
        let opacity = opacity * g.opacity().get();
        if g.opacity().get() < 1.0 && g.children().len() > 1 {
            note(
                image_because,
                approx,
                "a group's opacity over shapes that overlap",
            );
        }
        for child in g.children() {
            match child {
                usvg::Node::Group(cg) => walk(cg, opacity, shapes, image_because, approx),
                usvg::Node::Image(_) => note(image_because, approx, "an embedded image"),
                usvg::Node::Text(_) => note(image_because, approx, "text"),
                usvg::Node::Path(p) => {
                    if !p.is_visible() {
                        continue;
                    }
                    let t = p.abs_transform();
                    let map = |x: f32, y: f32| {
                        let mut pt = usvg::tiny_skia_path::Point::from_xy(x, y);
                        t.map_point(&mut pt);
                        Vec2::new(pt.x, pt.y)
                    };
                    let mut ops = Vec::new();
                    for seg in p.data().segments() {
                        use usvg::tiny_skia_path::PathSegment as S;
                        ops.push(match seg {
                            S::MoveTo(a) => PathOp::MoveTo(map(a.x, a.y)),
                            S::LineTo(a) => PathOp::LineTo(map(a.x, a.y)),
                            S::QuadTo(c, a) => PathOp::QuadTo(map(c.x, c.y), map(a.x, a.y)),
                            S::CubicTo(c1, c2, a) => {
                                PathOp::CubicTo(map(c1.x, c1.y), map(c2.x, c2.y), map(a.x, a.y))
                            }
                            S::Close => PathOp::Close,
                        });
                    }
                    let fill = match p.fill() {
                        None => None,
                        Some(f) => match ink(f.paint()) {
                            Some(i) => Some((i, f.opacity().get() * opacity)),
                            None => {
                                note(image_because, approx, "a gradient or pattern fill");
                                None
                            }
                        },
                    };
                    let rule = match p.fill().map(|f| f.rule()) {
                        Some(usvg::FillRule::EvenOdd) => FillRule::EvenOdd,
                        _ => FillRule::NonZero,
                    };
                    let stroke = match p.stroke() {
                        None => None,
                        Some(s) => {
                            let open = ops
                                .iter()
                                .zip(
                                    ops.iter()
                                        .skip(1)
                                        .chain(std::iter::once(&PathOp::MoveTo(Vec2::ZERO))),
                                )
                                .any(|(a, b)| {
                                    !matches!(a, PathOp::Close)
                                        && matches!(b, PathOp::MoveTo(_))
                                        && !matches!(a, PathOp::MoveTo(_))
                                });
                            if s.linecap() != usvg::LineCap::Round && open {
                                note(image_because, approx, "a butt or square stroke cap");
                            }
                            if !matches!(s.linejoin(), usvg::LineJoin::Round) {
                                note(image_because, approx, "a mitred or bevelled stroke join");
                            }
                            let dash = match s.dasharray() {
                                None => None,
                                Some(d) => match Dash::of(d) {
                                    Some(mut dd) => {
                                        dd.offset = s.dashoffset();
                                        Some(dd)
                                    }
                                    None => {
                                        note(
                                            image_because,
                                            approx,
                                            "a dash pattern longer than four lengths",
                                        );
                                        None
                                    }
                                },
                            };
                            match ink(s.paint()) {
                                Some(i) => {
                                    Some((i, s.opacity().get() * opacity, s.width().get(), dash))
                                }
                                None => {
                                    note(image_because, approx, "a gradient or pattern stroke");
                                    None
                                }
                            }
                        }
                    };
                    if fill.is_some() || stroke.is_some() {
                        shapes.push(Shape {
                            ops,
                            rule,
                            fill,
                            stroke,
                        });
                    }
                }
            }
        }
    }
}

#[cfg(feature = "svg")]
pub use real::Svg;

/// Built without the `svg` feature: every document is refused, so the
/// element draws an empty box and says why (`svg-malformed`).
#[cfg(not(feature = "svg"))]
mod stub {
    use super::{SvgError, SvgFit, SvgShape};
    use crate::color::Color;

    /// Never made: [`Svg::parse`] refuses every document in this build.
    pub struct Svg {
        never: std::convert::Infallible,
    }

    impl std::fmt::Debug for Svg {
        fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self.never {}
        }
    }

    impl Svg {
        pub fn parse(_data: &[u8]) -> Result<Svg, SvgError> {
            Err(SvgError(
                "this kui was built without its `svg` feature".into(),
            ))
        }
        pub fn intrinsic(&self) -> (f32, f32) {
            match self.never {}
        }
        pub fn drawable_as_paths(&self) -> bool {
            match self.never {}
        }
        pub fn why_image(&self) -> Option<&str> {
            match self.never {}
        }
        pub fn approximations(&self) -> &[String] {
            match self.never {}
        }
        pub fn shapes(&self, _w: f32, _h: f32, _fit: SvgFit, _color: Color) -> Vec<SvgShape> {
            match self.never {}
        }
        pub fn reaches_past_raster(&self, _w: f32, _h: f32, _scale: f32, _fit: SvgFit) -> bool {
            match self.never {}
        }
        pub fn raster(
            &self,
            _w: f32,
            _h: f32,
            _scale: f32,
            _fit: SvgFit,
            _color: Color,
        ) -> (u32, u32, Vec<u8>) {
            match self.never {}
        }
    }
}

#[cfg(not(feature = "svg"))]
pub use stub::Svg;

#[cfg(all(test, feature = "svg"))]
mod tests {
    use super::*;

    const LUCIDE_HOUSE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M15 21v-8a1 1 0 0 0-1-1h-4a1 1 0 0 0-1 1v8"/><path d="M3 10a2 2 0 0 1 .709-1.528l7-5.999a2 2 0 0 1 2.582 0l7 5.999A2 2 0 0 1 21 10v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/></svg>"#;

    #[test]
    fn a_lucide_icon_draws_as_paths_in_the_element_colour() {
        let s = Svg::parse(LUCIDE_HOUSE).unwrap();
        assert!(s.drawable_as_paths(), "{:?}", s.why_image());
        let red = Color::hex(0xff0000ff);
        let shapes = s.shapes(48.0, 48.0, SvgFit::Contain, red);
        assert_eq!(shapes.len(), 2);
        for sh in &shapes {
            assert_eq!(sh.fill, None, "fill=none");
            let st = sh.path.stroke().expect("stroked");
            assert_eq!(st.color, red, "currentColor is the element's");
            assert!(
                (st.width - 4.0).abs() < 1e-4,
                "2 at 24 is 4 at 48: {}",
                st.width
            );
        }
    }

    #[test]
    fn a_gradient_or_a_butt_cap_draws_as_an_image() {
        let grad = br#"<svg viewBox="0 0 10 10"><defs><linearGradient id="g"><stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/></linearGradient></defs><rect width="10" height="10" fill="url(#g)"/></svg>"#;
        let s = Svg::parse(grad).unwrap();
        assert_eq!(s.why_image(), Some("a gradient or pattern fill"));
        let butt = br#"<svg viewBox="0 0 10 10"><path d="M1 5H9" stroke="black"/></svg>"#;
        assert_eq!(
            Svg::parse(butt).unwrap().why_image(),
            Some("a butt or square stroke cap")
        );
        // A closed shape's cap does not matter; a filled square is paths.
        let square =
            br##"<svg viewBox="0 0 10 10"><rect width="10" height="10" fill="#123456"/></svg>"##;
        assert!(Svg::parse(square).unwrap().drawable_as_paths());
    }

    #[test]
    fn fit_places_the_viewbox() {
        let s = Svg::parse(
            br#"<svg viewBox="0 0 20 10"><rect width="20" height="10" fill="red"/></svg>"#,
        )
        .unwrap();
        assert_eq!(s.place(40.0, 40.0, SvgFit::Contain), (0.0, 10.0, 2.0, 2.0));
        assert_eq!(s.place(40.0, 40.0, SvgFit::Cover), (-20.0, 0.0, 4.0, 4.0));
        assert_eq!(s.place(40.0, 40.0, SvgFit::Fill), (0.0, 0.0, 2.0, 4.0));
    }

    #[test]
    fn the_raster_is_straight_alpha_at_the_scale_asked() {
        let s = Svg::parse(br#"<svg viewBox="0 0 4 4"><rect width="4" height="4" fill="currentColor" opacity="0.5"/></svg>"#).unwrap();
        let (w, h, px) = s.raster(4.0, 4.0, 2.0, SvgFit::Contain, Color::hex(0x00ff00ff));
        assert_eq!((w, h), (8, 8));
        let mid = &px[(4 * 8 + 4) * 4..(4 * 8 + 4) * 4 + 4];
        assert_eq!(mid[1], 255, "green, unpremultiplied: {mid:?}");
        assert!((mid[3] as i32 - 128).abs() <= 1, "half alpha: {mid:?}");
    }

    #[test]
    fn a_broken_document_says_so() {
        assert!(Svg::parse(b"<svg").unwrap_err().0.starts_with("svg:"));
    }
}
