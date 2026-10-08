//! Core-owned text stack. Shaping and line layout run through cosmic-text and
//! are cached across frames keyed by (content, style, scale) — the layout pass
//! measures through this cache, so shaping survives resizes and static text
//! costs a hash lookup per frame. Rasterization feeds the shared glyph atlas;
//! renderers only ever see positioned atlas quads.

use cosmic_text::{
    Attrs, Buffer, CacheKeyFlags, Ellipsize, EllipsizeHeightLimit, FontSystem, Metrics, Shaping,
    Style as FontStyle, SwashContent, Weight, Wrap,
};
use rustc_hash::FxHashMap;

use crate::atlas::GlyphAtlas;
use crate::color::Color;
use crate::display::{Clip, ClipId, Quad, QuadKind};
use crate::geom::{Rect, Size, Vec2};
use crate::key::Key;
use crate::resources::Resources;
use crate::retain::Kept;
use crate::spec::{FontFamily, TextStyle, TextWrap, UnderlineStyle};
use crate::tree::TextId;
use crate::value::Value;

/// The glyph rasterizer: swash's scaler, as cosmic-text's `SwashCache`
/// drives it, for plain alpha masks, LCD subpixel masks and color bitmaps
/// alike — our own so the subpixel format is ours to pick, and so a
/// variable face is drawn at the `wght` coordinate its axis gives a CSS
/// weight and a glyph marked for synthetic bold is drawn bold, neither of
/// which cosmic-text's cache does.
pub(crate) struct Raster {
    ctx: swash::scale::ScaleContext,
    /// Each face's `wght` axis, read once: `None` for a face without one.
    axes: FxHashMap<cosmic_text::fontdb::ID, Option<crate::weights::WghtAxis>>,
    /// Rasterize outline glyphs as per-channel subpixel coverage. Set by
    /// the driver from what its renderer can blend; see
    /// `Core::set_subpixel_text`.
    pub(crate) subpixel: bool,
}

impl Raster {
    fn new() -> Self {
        Self {
            ctx: swash::scale::ScaleContext::new(),
            axes: FxHashMap::default(),
            subpixel: false,
        }
    }

    /// cosmic-text's `swash_image` in `format`: with `Format::Subpixel`,
    /// three rasterizations shifted by a third of a pixel land in r, g and
    /// b. Color sources are tried first so emoji still come out as bitmaps.
    ///
    /// A variable face is drawn where its `wght` axis puts the glyph's
    /// weight ([`WghtAxis`](crate::weights::WghtAxis)) — cosmic-text takes
    /// the CSS number itself as the coordinate, which draws Berkeley Mono
    /// Variable's regular at its Bold — and a glyph marked
    /// [`SYNTHETIC_BOLD`](crate::weights::SYNTHETIC_BOLD) at the axis's
    /// bold, or, when the face has no heavier instance or no axis, with
    /// its outline emboldened.
    fn image(
        &mut self,
        fs: &mut FontSystem,
        key: cosmic_text::CacheKey,
        format: swash::zeno::Format,
    ) -> Option<cosmic_text::SwashImage> {
        use swash::scale::{Render, Source, StrikeWith};
        use swash::zeno::{Angle, Transform, Vector};
        let font = fs.get_font(key.font_id, key.font_weight)?;
        let swash_font = font.as_swash();
        let axis = self
            .axes
            .entry(key.font_id)
            .or_insert_with(|| {
                let weight = fs.db().face(key.font_id).map_or(400, |face| face.weight.0);
                crate::weights::WghtAxis::read(swash_font, weight)
            })
            .as_ref();
        let size = f32::from_bits(key.font_size_bits);
        let face_weight = f32::from(key.font_weight.0);
        let bold = key.flags.contains(crate::weights::SYNTHETIC_BOLD);
        let asked = if bold {
            f32::from(Weight::BOLD.0)
        } else {
            face_weight
        };
        let at = axis.map(|axis| axis.coordinate(asked));
        // Bold that the axis cannot draw heavier than the face's own
        // weight, or a face with no axis: the outline grown instead.
        let embolden = bold
            && match (axis, at) {
                (Some(axis), Some(at)) => at <= axis.coordinate(face_weight),
                _ => true,
            };
        let mut scaler = self
            .ctx
            .builder(swash_font)
            .size(size)
            .hint(!key.flags.contains(CacheKeyFlags::DISABLE_HINTING));
        if let Some(at) = at {
            scaler = scaler.normalized_coords(
                swash_font
                    .variations()
                    .normalized_coords([(swash::Tag::from_be_bytes(*b"wght"), at)]),
            );
        }
        let mut scaler = scaler.build();
        let offset = if key.flags.contains(CacheKeyFlags::PIXEL_FONT) {
            Vector::new(key.x_bin.as_float().round(), key.y_bin.as_float().round())
        } else {
            Vector::new(key.x_bin.as_float(), key.y_bin.as_float())
        };
        Render::new(&[
            Source::ColorOutline(0),
            Source::ColorBitmap(StrikeWith::BestFit),
            Source::Outline,
        ])
        .format(format)
        .offset(offset)
        .embolden(if embolden {
            crate::weights::embolden_strength(size)
        } else {
            0.0
        })
        .transform(
            key.flags
                .contains(CacheKeyFlags::FAKE_ITALIC)
                .then(|| Transform::skew(Angle::from_degrees(14.0), Angle::from_degrees(0.0))),
        )
        .render(&mut scaler, key.glyph_id)
    }
}

/// Rasterizes one glyph into the atlas (shared by static text and editors).
pub(crate) fn raster_glyph(
    key: cosmic_text::CacheKey,
    fs: &mut FontSystem,
    raster: &mut Raster,
    atlas: &mut crate::atlas::GlyphAtlas,
) -> Option<crate::atlas::GlyphSlot> {
    atlas.get_or_insert(key, || {
        let format = if raster.subpixel {
            swash::zeno::Format::Subpixel
        } else {
            swash::zeno::Format::Alpha
        };
        let image = raster.image(fs, key, format)?;
        if image.placement.width == 0 || image.placement.height == 0 {
            return None;
        }
        let (data, is_color, subpixel) = match image.content {
            SwashContent::Mask => {
                let mut rgba = Vec::with_capacity(image.data.len() * 4);
                for &a in image.data.iter() {
                    rgba.extend_from_slice(&[255, 255, 255, a]);
                }
                (rgba, false, false)
            }
            SwashContent::Color => (image.data.to_vec(), true, false),
            SwashContent::SubpixelMask => {
                // rgb carry the per-channel coverages; alpha (which zeno
                // leaves untouched) becomes the union, the value a backend
                // without per-channel blending falls back to.
                let mut rgba = image.data.to_vec();
                for px in rgba.as_chunks_mut::<4>().0 {
                    px[3] = px[0].max(px[1]).max(px[2]);
                }
                (rgba, false, true)
            }
        };
        Some(crate::atlas::RasterGlyph {
            w: image.placement.width,
            h: image.placement.height,
            left: image.placement.left,
            top: image.placement.top,
            color: is_color,
            subpixel,
            data,
        })
    })
}

/// The quad kind a rasterized glyph draws as.
pub(crate) fn glyph_kind(slot: &crate::atlas::GlyphSlot) -> QuadKind {
    if slot.color_glyph {
        QuadKind::GlyphColor
    } else if slot.subpixel {
        QuadKind::GlyphSubpixel
    } else {
        QuadKind::GlyphMask
    }
}

/// The default byte budget for the shaped-text cache. Sized
/// so a screenful of code never meets it — two panes of 55 highlighted
/// lines are ~900 short entries, a few megabytes — and a pane streaming
/// new text meets it within seconds, which is when the clock alone let the
/// cache reach gigabytes. `Core::set_text_cache_budget` changes it.
pub const DEFAULT_TEXT_CACHE_BYTES: usize = 64 << 20;

/// What one cache entry costs, estimated: a fixed floor (the `Buffer`, its
/// line, the attrs list, the entry itself) plus a per-glyph rate that
/// covers cosmic-text's `ShapeGlyph` and `LayoutGlyph`, our
/// `GlyphTemplate`, and the entry's share of the shape-run cache. Measured
/// on 2026-09-07 with a counting allocator around a `Core` drawing 50 new
/// lines a frame: 7.6 KB per 10-glyph line, 22 KB per 40, 92 KB per 200
/// (cosmic-text's own `Buffer` is 284 B/glyph of that and the shape-run
/// cache 118 B/glyph). The estimate lands within ~10% of those, on the
/// high side, so the budget errs toward evicting.
const ENTRY_BASE_BYTES: usize = 4096;
const ENTRY_GLYPH_BYTES: usize = 480;

/// A non-wrapping text at least this long is shaped in chunks: a minified
/// bundle, a log line with a blob in it, a base64 field.
/// Shorter text takes the path it always took, so nothing below the line
/// moves. Bytes, not characters, for the same reason a step line carries
/// integers: every binding can count them.
pub const LONG_LINE_BYTES: usize = 4096;
/// How long a chunk is, at most: cut at the last whitespace in the second
/// half of the window, else at a grapheme boundary. cosmic-text shapes
/// per whitespace-delimited word and its shape-run cache keys on words, so
/// a cut at whitespace loses nothing the whole line kept.
const CHUNK_BYTES: usize = 1024;
/// Mixed into a long line's key so it never collides with the entry a
/// short text of the same content would get.
const LONG_SALT: u64 = 0x5f5f_6c6f_6e67_5f5f;
/// Mixed into the key of a long line's copies, one for each further node
/// of a frame drawing the same line ([`TextSystem::claim_long`]).
const COPY_SALT: u64 = 0x5f5f_636f_7079_5f5f;

/// When the cache is over budget it is evicted down to this fraction of it,
/// not to the line, so a stream that adds a little every frame walks the
/// cache once per quarter-budget of new text rather than every frame.
const EVICT_TO_NUMERATOR: usize = 3;
const EVICT_TO_DENOMINATOR: usize = 4;

pub(crate) struct CachedText {
    buffer: Buffer,
    /// The text itself (spans concatenated, for rich text): what the
    /// access tree names a control by.
    content: String,
    /// Wrap width (physical px) the buffer is currently laid out at.
    wrap: Option<f32>,
    /// Unwrapped measurement, physical px.
    intrinsic: Size,
    /// Lines past this many are dropped (0 = unlimited): the buffer-wide
    /// budget; cosmic-text's ellipsize limit is per paragraph.
    max_lines: usize,
    /// The content may run past the node's width (no-wrap, ellipsis):
    /// report the box width and clip glyphs to it.
    clamp_w: bool,
    last_used: u64,
    /// What this entry costs the budget (see `ENTRY_BASE_BYTES`).
    bytes: usize,
    /// Positioned glyph quads relative to the text origin, so steady-state
    /// emission is a memcpy-style walk instead of per-glyph atlas lookups.
    glyphs: Vec<GlyphTemplate>,
    /// The decoration rects that go with them: a span's
    /// background under its glyphs, its underline and strikethrough over
    /// them, one rect per run of the span per line, so they wrap with it.
    deco: Vec<DecoTemplate>,
    /// What each span asked for, by the index its glyphs carry as
    /// metadata; one entry for plain text.
    span_deco: Vec<SpanDeco>,
    /// (wrap, atlas stamp) the template cache was built for.
    glyphs_built_for: Option<(Option<u32>, u64)>,
    /// The frame a node last wrapped this run for, and the wrap it asked:
    /// a second node of that frame asking another width wraps a copy of
    /// its own ([`TextSystem::own_wrap`]), since the queries read the
    /// buffer as it was left.
    claimed: u64,
    claimed_wrap: Option<f32>,
    /// A break may fall between any two glyphs (`TextWrap::Glyph`), not
    /// only where a line may break: what the min-content reads.
    breaks_anywhere: bool,
    /// The widest stretch no break falls inside, physical px — CSS's
    /// min-content — measured the first time a shrink asks and kept for
    /// the entry's life, since no width changes it.
    min_content: Option<f32>,
}

struct GlyphTemplate {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    uv: [u32; 4],
    kind: QuadKind,
    /// Per-span color override (rich text); falls back to the node color.
    color: Option<Color>,
    /// The byte the glyph starts at in the entry's content: what puts it
    /// on a row when a wrapped long line's chunk is drawn.
    byte: u32,
}

/// One decoration rect relative to the text origin, physical px.
struct DecoTemplate {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    /// The rect's colour; `None` is the text's own (an underline in the
    /// text colour), which a background never is.
    color: Option<Color>,
    /// Painted before the glyphs (a background) rather than after (a line).
    under: bool,
    /// The shape, for an underline: the rect is where a solid line goes,
    /// and a wave or dots are built around it at emission.
    style: UnderlineStyle,
    /// A background's radius, logical px; above zero the frame joins it
    /// with the ones it meets ([`JoinBg`]).
    radius: f32,
}

/// A rounded span background emitted this frame: the quad
/// it is, square for now, which the frame's last pass turns into its part
/// of one shape once every text has been painted and each can be told the
/// ones it meets (`crate::join`). The radius is logical px. `outer` is the
/// clip the text was given, and `own` its own box's physical x-range when
/// it clips to it, as a no-wrap text does: a piece is no wider than that
/// box shows, but a fillet past the end of a short line lies outside the
/// box, beside it, and is clipped only as the text's parent is.
#[derive(Clone, Copy, Debug)]
pub(crate) struct JoinBg {
    pub quad: u32,
    pub radius: f32,
    pub outer: ClipId,
    pub own: Option<(f32, f32)>,
}

/// The decorations one span (or a plain text's whole content) asked for.
#[derive(Clone, Copy, Default)]
struct SpanDeco {
    underline: bool,
    underline_color: Option<Color>,
    underline_style: UnderlineStyle,
    strikethrough: bool,
    bg: Option<Color>,
    /// The background's radius, logical px: above zero it is joined with
    /// the backgrounds it meets.
    bg_radius: f32,
}

impl SpanDeco {
    fn of_style(style: &TextStyle) -> Self {
        Self {
            underline: style.underline,
            underline_color: style.underline_color,
            underline_style: style.underline_style,
            strikethrough: style.strikethrough,
            bg: None,
            bg_radius: 0.0,
        }
    }

    fn any(&self) -> bool {
        self.underline || self.strikethrough || self.bg.is_some()
    }
}

/// One styled run inside a rich-text paragraph. Spans are shaped and wrapped
/// together as a single flow; plain data, so every frontend can build them.
#[derive(Clone, Copy, Debug)]
pub struct Span<'a> {
    pub text: &'a str,
    pub color: Option<Color>,
    pub bold: bool,
    pub italic: bool,
    /// A line under the span, where the face puts its underline.
    pub underline: bool,
    /// The underline's own colour; `None` is the span's.
    pub underline_color: Option<Color>,
    /// The underline's shape.
    pub underline_style: UnderlineStyle,
    /// A line through the span, where the face puts its strikeout.
    pub strikethrough: bool,
    /// A background behind the span's glyphs, one rect per line it spans,
    /// so it follows the span across a wrap the way a box cannot.
    pub bg: Option<Color>,
    /// The background's corner radius, logical px. Above
    /// zero, every background of the same colour and radius that meets
    /// another edge to edge on the line above or below — in this text or
    /// another — is one shape with it: its corners convex where a line
    /// reaches past its neighbour, concave where it falls short, round
    /// where nothing meets it. What a selection over lines looks like.
    pub bg_radius: f32,
    /// The span's own face, in place of the paragraph's: inline `code` in
    /// `FontFamily::Mono` inside a sans line. `None` is the paragraph's.
    /// The span keeps the paragraph's weights for its family (bold is the
    /// family's bold), and its glyphs, carets and selection are measured
    /// in the face they are drawn in, so byte positions stay exact across
    /// the change.
    pub family: Option<FontFamily>,
    /// The span's own size in logical px, in place of the paragraph's: a
    /// heading's larger first word, a smaller footnote mark. Its line
    /// height scales with it at the paragraph's ratio, and a line is as
    /// tall as its tallest span. `None` is the paragraph's. A paragraph
    /// with a sized span is shaped whole, never chunked as a long line.
    pub size: Option<f32>,
}

impl<'a> Span<'a> {
    pub fn new(text: &'a str) -> Self {
        Self {
            text,
            color: None,
            bold: false,
            italic: false,
            underline: false,
            underline_color: None,
            underline_style: UnderlineStyle::Solid,
            strikethrough: false,
            bg: None,
            bg_radius: 0.0,
            family: None,
            size: None,
        }
    }

    /// The span in its own face (see the `family` field).
    pub fn family(mut self, family: FontFamily) -> Self {
        self.family = Some(family);
        self
    }

    /// The span in the monospace face: inline code.
    ///
    /// ```
    /// # use kui_core::{FontFamily, text::Span};
    /// let code = Span::new("cargo test").mono();
    /// assert_eq!(code.family, Some(FontFamily::Mono));
    /// ```
    pub fn mono(self) -> Self {
        self.family(FontFamily::Mono)
    }

    /// The span at its own size in logical px (see the `size` field); a
    /// size that is not a positive number is the paragraph's.
    pub fn size(mut self, px: f32) -> Self {
        self.size = (px.is_finite() && px > 0.0).then_some(px);
        self
    }

    pub fn color(mut self, c: Color) -> Self {
        self.color = Some(c);
        self
    }

    pub fn bold(mut self) -> Self {
        self.bold = true;
        self
    }

    pub fn italic(mut self) -> Self {
        self.italic = true;
        self
    }

    pub fn underline(mut self) -> Self {
        self.underline = true;
        self
    }

    /// An underline in its own colour; turns it on.
    pub fn underline_color(mut self, c: Color) -> Self {
        self.underline = true;
        self.underline_color = Some(c);
        self
    }

    /// An underline of this shape; turns it on.
    pub fn underline_style(mut self, s: UnderlineStyle) -> Self {
        self.underline = true;
        self.underline_style = s;
        self
    }

    pub fn strikethrough(mut self) -> Self {
        self.strikethrough = true;
        self
    }

    pub fn bg(mut self, c: Color) -> Self {
        self.bg = Some(c);
        self
    }

    /// Rounds the background, joined with the ones it meets (see
    /// [`Span::bg_radius`](Self#structfield.bg_radius)).
    pub fn bg_radius(mut self, r: f32) -> Self {
        self.bg_radius = r.max(0.0);
        self
    }

    /// The span without its text: what a long line keeps per span so a
    /// chunk can be rebuilt from the content and the ranges.
    fn attrs_only(&self) -> SpanAttrs {
        SpanAttrs {
            color: self.color,
            bold: self.bold,
            italic: self.italic,
            underline: self.underline,
            underline_color: self.underline_color,
            underline_style: self.underline_style,
            strikethrough: self.strikethrough,
            bg: self.bg,
            bg_radius: self.bg_radius,
            family: self.family,
            size: self.size,
        }
    }

    fn attrs<'r>(
        &self,
        family: cosmic_text::Family<'r>,
        weights: crate::weights::Weights,
    ) -> Attrs<'r> {
        // Pin the family (the paragraph base's) so weight/style variants
        // stay in one typeface instead of falling back to whatever face
        // matches first — at weights it has faces for (backlog F100).
        let mut attrs = weights.apply(Attrs::new().family(family), self.bold);
        if self.italic {
            attrs = attrs.style(FontStyle::Italic);
        }
        if let Some(c) = self.color {
            attrs = attrs.color(cosmic_text::Color::rgba(
                (c.r * 255.0) as u8,
                (c.g * 255.0) as u8,
                (c.b * 255.0) as u8,
                (c.a * 255.0) as u8,
            ));
        }
        attrs
    }
}

/// A [`Span`]'s attributes without its text, owned: what a long rich
/// line keeps per span, with the range in [`OwnedSpan`], so a chunk can
/// be handed the spans that intersect it, sliced.
#[derive(Clone, Copy)]
struct SpanAttrs {
    color: Option<Color>,
    bold: bool,
    italic: bool,
    underline: bool,
    underline_color: Option<Color>,
    underline_style: UnderlineStyle,
    strikethrough: bool,
    bg: Option<Color>,
    bg_radius: f32,
    family: Option<FontFamily>,
    size: Option<f32>,
}

impl SpanAttrs {
    fn span<'a>(&self, text: &'a str) -> Span<'a> {
        Span {
            text,
            color: self.color,
            bold: self.bold,
            italic: self.italic,
            underline: self.underline,
            underline_color: self.underline_color,
            underline_style: self.underline_style,
            strikethrough: self.strikethrough,
            bg: self.bg,
            bg_radius: self.bg_radius,
            family: self.family,
            size: self.size,
        }
    }
}

/// One span of a long rich line: its byte range in the content and its
/// attributes. The ranges are contiguous and cover the content.
#[derive(Clone)]
struct OwnedSpan {
    start: usize,
    end: usize,
    attrs: SpanAttrs,
}

/// The spans of `spans` that intersect `start..end` of `content`, sliced
/// to it and with the empty dropped: what a chunk of a long rich line is
/// shaped from. A span cut by the chunk boundary becomes two, one a
/// side, with the same attributes — its background rects meet at the cut.
fn chunk_spans<'a>(
    content: &'a str,
    spans: &[OwnedSpan],
    start: usize,
    end: usize,
) -> Vec<Span<'a>> {
    let first = spans.partition_point(|s| s.end <= start);
    spans[first..]
        .iter()
        .take_while(|s| s.start < end)
        .filter_map(|s| {
            let (a, b) = (s.start.max(start), s.end.min(end));
            (a < b).then(|| s.attrs.span(&content[a..b]))
        })
        .collect()
}

struct FrameText {
    cache_key: u64,
    color: Color,
}

/// One entry of the text cache: a shaped run, or a long line whose chunks
/// are runs of their own in the same map. Whether a text is
/// long is decided once, at [`TextSystem::add`], and lives here as the
/// variant; every operation after it asks the entry rather than carrying
/// a flag beside the key. A long line joins a selection scope's
/// concatenation like any run: being long is how it was shaped, not
/// something a reader dragging across it should feel.
pub(crate) enum Entry {
    Run(CachedText),
    Long(LongLine),
}

impl Entry {
    pub(crate) fn content(&self) -> &str {
        match self {
            Entry::Run(e) => &e.content,
            Entry::Long(l) => &l.content,
        }
    }

    fn last_used(&self) -> u64 {
        match self {
            Entry::Run(e) => e.last_used,
            Entry::Long(l) => l.last_used,
        }
    }

    fn touch(&mut self, frame_no: u64) {
        match self {
            Entry::Run(e) => e.last_used = frame_no,
            Entry::Long(l) => l.last_used = frame_no,
        }
    }

    /// What the entry costs the budget.
    fn bytes(&self) -> usize {
        match self {
            Entry::Run(e) => e.bytes,
            Entry::Long(l) => l.bytes,
        }
    }

    fn run(&self) -> Option<&CachedText> {
        match self {
            Entry::Run(e) => Some(e),
            Entry::Long(_) => None,
        }
    }

    fn run_mut(&mut self) -> Option<&mut CachedText> {
        match self {
            Entry::Run(e) => Some(e),
            Entry::Long(_) => None,
        }
    }

    fn long(&self) -> Option<&LongLine> {
        match self {
            Entry::Long(l) => Some(l),
            Entry::Run(_) => None,
        }
    }

    fn long_mut(&mut self) -> Option<&mut LongLine> {
        match self {
            Entry::Long(l) => Some(l),
            Entry::Run(_) => None,
        }
    }
}

/// One chunk of a long line: its byte range in the content, the cache key
/// its shaped entry has (an ordinary `CachedText`, budgeted like any), and
/// its width once shaped.
#[derive(Clone)]
struct Chunk {
    start: usize,
    end: usize,
    key: u64,
    /// The run's own width: its tabs' stops measured from its start.
    width: Option<f32>,
    /// For a shaped chunk that starts off the line's tab stops and ends in
    /// its one tab (`chunk_ranges`): where the tab starts in the run, and
    /// the stops' interval, physical px. The run's width put that tab on
    /// a stop of the chunk's own; the line puts it on one of its own, from
    /// where the chunk starts (`LongLine::reprefix`, backlog RG76).
    tab: Option<(f32, f32)>,
    /// While the line is wrapped and this chunk is shaped: where each of
    /// its rows starts. Empty otherwise (one row, or an estimate).
    rows: Vec<RowStart>,
}

/// Where a row of a wrapped long line's chunk starts, relative to the
/// chunk: the byte, and the x of that glyph in the chunk's unwrapped run,
/// which the row's glyphs are shifted back by when drawn.
#[derive(Clone, Copy, Debug)]
struct RowStart {
    byte: u32,
    x: f32,
}

/// A non-wrapping text past [`LONG_LINE_BYTES`], shaped in chunks on demand.
/// The line itself holds no buffer: its chunks are cache
/// entries interned when emission, a hit-test or a caret query lands in
/// them, and `prefix` is where each chunk starts — an estimate from the
/// first chunk's mean advance until the chunk shapes, exact after. So a
/// 100k-character line costs the screenful it shows, a keystroke into it
/// costs the chunk it lands in, and the width the scrollbar sees can move
/// a little as chunks fill in, exact under monospace.
///
/// Its rows are its own, so a line is laid out for one node a frame: a
/// second node drawing the same content takes a copy
/// ([`TextSystem::claim_long`]), which shares the chunks' shaped runs.
#[derive(Clone)]
pub(crate) struct LongLine {
    content: String,
    /// The spans, for a rich line: each chunk is shaped as a
    /// rich run of the spans that intersect it, sliced. Empty for a plain
    /// line, whose chunks are plain runs.
    spans: Vec<OwnedSpan>,
    /// The chunks' style: the text's with `wrap` set to `None`, since a
    /// chunk is always shaped as one run and the line breaks it itself.
    style: TextStyle,
    /// The text's own line breaking. `None` is one row; `Word` and
    /// `Glyph` break the chunks into rows once the line does not fit its
    /// box (C19's step 5): a one-direction prefix computation over glyph
    /// positions, each chunk's first row starting where the previous
    /// chunk's last row ended, so it costs positions and not shaping.
    wrap: TextWrap,
    /// The physical width the rows are broken to, while they are; `None`
    /// is one row — the line fits, or its style never wraps.
    wrap_w: Option<f32>,
    /// While wrapped: where each chunk begins, the row and the x on it —
    /// one more than there are chunks, the last being where the text
    /// ends. The wrapped counterpart of `prefix`, estimated the same way
    /// for a chunk that never showed.
    starts: Vec<(u32, f32)>,
    chunks: Vec<Chunk>,
    /// Physical px from the line's origin to each chunk's start, one more
    /// than there are chunks: the last is the line's width.
    prefix: Vec<f32>,
    /// Physical line height, from the first chunk's buffer.
    line_h: f32,
    /// Physical px per byte, from the first chunk: the estimate an
    /// unshaped chunk's width is.
    avg: f32,
    last_used: u64,
    /// The frame a node last took this line to draw, `u64::MAX` before
    /// one has.
    claimed: u64,
    /// The rows layout gave the node this frame, and the rows the last
    /// frame asked for was owed for (`u32::MAX` before any): emission
    /// breaks the rows again once the chunks it shapes are known, and
    /// rows that differ from layout's owe a frame laid out on them —
    /// once per count, so a cache too small to keep the chunks cannot
    /// ask forever.
    laid_rows: u32,
    asked_rows: u32,
    bytes: usize,
}

impl LongLine {
    /// The chunk `x` (physical, from the line's origin) falls in.
    fn chunk_at(&self, x: f32) -> usize {
        match self.prefix[1..].partition_point(|&p| p <= x) {
            i if i >= self.chunks.len() => self.chunks.len().saturating_sub(1),
            i => i,
        }
    }

    /// The chunk byte `byte` falls in (the last for the end).
    fn chunk_of_byte(&self, byte: usize) -> usize {
        match self.chunks.partition_point(|c| c.end <= byte) {
            i if i >= self.chunks.len() => self.chunks.len().saturating_sub(1),
            i => i,
        }
    }

    fn width(&self) -> f32 {
        self.prefix.last().copied().unwrap_or(0.0)
    }

    /// Rows while wrapped; one otherwise.
    fn rows(&self) -> u32 {
        match self.starts.last() {
            Some(&(r, _)) if self.wrap_w.is_some() => r + 1,
            _ => 1,
        }
    }

    /// The chunks whose rows touch `ra..=rb`, while wrapped: chunk `i`
    /// spans `starts[i].0..=starts[i + 1].0`.
    fn chunks_on_rows(&self, ra: u32, rb: u32) -> Option<(usize, usize)> {
        let n = self.chunks.len();
        if n == 0 || self.starts.len() != n + 1 {
            return None;
        }
        let first = self.starts[1..].partition_point(|s| s.0 < ra);
        let last = self.starts[..n].partition_point(|s| s.0 <= rb);
        (first < last).then_some((first, last - 1))
    }

    /// Recomputes `prefix` from the chunk widths, estimating the unshaped.
    /// A chunk ending in a tab it started off the stops for is as wide as
    /// takes that tab to the line's next stop (backlog RG76), so the chunk
    /// after it starts on one; the stop is exact once every chunk since
    /// the line's previous tab is shaped, and otherwise as near as the
    /// estimate `prefix` already is.
    fn reprefix(&mut self) {
        let mut at = 0.0f32;
        self.prefix.clear();
        self.prefix.push(0.0);
        for c in &self.chunks {
            at = match (c.width, c.tab) {
                (Some(_), Some((tab_x, every))) => next_tab_stop(at + tab_x, every),
                (Some(w), None) => at + w,
                (None, _) => at + (c.end - c.start) as f32 * self.avg,
            };
            self.prefix.push(at);
        }
    }

    /// How far the line moves chunk `i`'s end from where its run ends:
    /// nonzero only for a chunk whose tab the line places (`reprefix`).
    fn tab_shift(&self, i: usize) -> f32 {
        let c = &self.chunks[i];
        match (c.width, c.tab) {
            (Some(w), Some(_)) => self.prefix[i + 1] - self.prefix[i] - w,
            _ => 0.0,
        }
    }

    /// `tab_shift` for a caret `local` bytes into chunk `i`: only the
    /// chunk's end is past a tab the line places, which is its last
    /// character, so only the end caret moves — and with it the line's end
    /// caret, which then agrees with the line's width (backlog RG122).
    fn end_shift(&self, i: usize, local: usize) -> f32 {
        let c = &self.chunks[i];
        match local >= c.end - c.start {
            true => self.tab_shift(i),
            false => 0.0,
        }
    }

    /// The byte a point at `x` along chunk `i`'s run answers when it is on
    /// a tab the line places: the run's tab is as wide as the chunk's own
    /// stops make it and the drawn one as wide as the line's (`reprefix`),
    /// so the run's hit test splits it at the wrong place. The tab before
    /// the drawn tab's middle, the byte after it from there (backlog
    /// RG122). `None` off such a tab.
    fn placed_tab_hit(&self, i: usize, x: f32) -> Option<usize> {
        let c = &self.chunks[i];
        let (tab_x, _) = c.tab?;
        c.width?;
        if x < tab_x {
            return None;
        }
        let drawn_end = self.prefix[i + 1] - self.prefix[i];
        Some(match x < (tab_x + drawn_end) / 2.0 {
            true => c.end - 1,
            false => c.end,
        })
    }
}

/// The tab stop after `x` (physical px from the line's start), stops
/// `every` px apart: cosmic-text's rule, which moves a tab already on a
/// stop to the next one. `x` is a sum of chunk widths, so a stop within a
/// sixteenth of a pixel ahead counts as reached — the drift that sum has
/// over a stretch without a tab, which a whole line's own sum has too.
fn next_tab_stop(x: f32, every: f32) -> f32 {
    if every <= 0.0 {
        return x;
    }
    (((x + 1.0 / 16.0) / every).floor() + 1.0) * every
}

/// Where a long line is cut: after the last tab in the second half of each
/// window, else after the last whitespace there, else at the last grapheme
/// boundary inside it — except that a chunk starting anywhere but after a
/// tab ends after its first tab, wherever in the window that falls. A line
/// under [`LONG_LINE_BYTES`] — long only by its `break-spaces` — is one
/// chunk, shaped whole as the run it would otherwise be.
///
/// Tab stops are why: cosmic-text measures them from where
/// the shaped text starts, so a chunk shaped alone put a tab after its
/// start at a stop measured from there and not from the line's. A chunk
/// cut just after a tab ends on a stop, so the next one starts on one and
/// its stops are the line's (RG75). A chunk that starts off the stops —
/// after a stretch of more than half a window without a tab — holds at
/// most one tab, its last character, whose advance the line places from
/// where the chunk starts (`Chunk::tab`, backlog RG76): nothing in the
/// chunk follows that tab, so its run needs no reshaping for the line's x.
fn chunk_ranges(content: &str) -> Vec<(usize, usize)> {
    use unicode_segmentation::UnicodeSegmentation;
    if content.len() < LONG_LINE_BYTES {
        return vec![(0, content.len())];
    }
    let bytes = content.as_bytes();
    let mut out = Vec::with_capacity(content.len() / CHUNK_BYTES + 1);
    let mut start = 0usize;
    while start < content.len() {
        // The window's end floored to a char boundary: a multibyte
        // character straddling `start + CHUNK_BYTES` is the window's, not
        // the next one's, and slicing inside it was a panic (backlog C44).
        let mut window_end = (start + CHUNK_BYTES).min(content.len());
        while !content.is_char_boundary(window_end) {
            window_end -= 1;
        }
        let window = &content[start..window_end];
        // Where the window's second half starts, on a char boundary.
        let mut from = (CHUNK_BYTES / 2).min(window.len());
        while !window.is_char_boundary(from) {
            from += 1;
        }
        // Off the stops (after anything but a tab), a tab in the first
        // half ends the chunk: `find` is a memchr, so a tab-free line pays
        // a search per window and not a second walk of it (RG75's cost).
        let off_stops = start > 0 && bytes[start - 1] != b'\t';
        let early_tab = match off_stops {
            true => window[..from].find('\t'),
            false => None,
        };
        let end = if let Some(t) = early_tab {
            start + t + 1
        } else if window_end == content.len() && !(off_stops && window[from..].contains('\t')) {
            window_end
        } else {
            // One pass over the second half, noting its first and last
            // tab and its last whitespace: a line re-cut on every edit
            // (C19) must not walk each window twice.
            let (mut first_tab, mut last_tab, mut space) = (None, None, None);
            for (i, c) in window[from..].char_indices() {
                if c.is_whitespace() {
                    let end = from + i + c.len_utf8();
                    space = Some(end);
                    if c == '\t' {
                        first_tab = first_tab.or(Some(end));
                        last_tab = Some(end);
                    }
                }
            }
            let tab = if off_stops { first_tab } else { last_tab };
            match tab.or(space) {
                Some(i) => start + i,
                None => {
                    // The last grapheme boundary at or before the window's end.
                    let mut cut = 0usize;
                    for (i, _) in window.grapheme_indices(true) {
                        if i == 0 {
                            continue;
                        }
                        cut = i;
                    }
                    if cut == 0 { window_end } else { start + cut }
                }
            }
        };
        out.push((start, end));
        start = end;
    }
    out
}

/// How many enclosing keys a place remembers: a text run answers to its
/// own key and to any of this many ancestors, which is a `line` row, a
/// selection wrapper around a run, and two to spare. A text further than
/// this below its `line` raises `text-beyond-line`.
pub(crate) const PLACE_ANCESTORS: usize = 4;

/// The keys above a text node, nearest first, as many as a place
/// remembers, and where among them a `role="none"` ancestor sits — what
/// `Core::text_ancestors` gathers for [`TextSystem::place`].
pub(crate) struct Ancestry {
    pub(crate) keys: [Key; PLACE_ANCESTORS],
    pub(crate) depth: usize,
    pub(crate) none_at: Option<usize>,
}

/// Where a text node was drawn: what `Core::text_hit` and
/// `Core::caret_rect` answer from. Recorded at emission, so
/// a node the frame culled — scrolled out of its clip — has no place and
/// answers nothing, which is also true of a point nobody can click.
pub(crate) struct TextPlace {
    pub(crate) key: Key,
    /// The keys above it, nearest first, as many as `depth` says.
    ancestors: [Key; PLACE_ANCESTORS],
    depth: u8,
    /// The nearest ancestor (an index into `ancestors`) declaring
    /// `role="none"`, or `u8::MAX` for none within reach: a query by a key
    /// above it does not reach this run, the way the access tree skips a
    /// gutter's text when it reads a `line`; a query by the
    /// gutter itself still does.
    none_at: u8,
    cache_key: u64,
    /// The node's origin, logical viewport px.
    origin: Vec2,
    /// The innermost `selectable` node above this run, when there is one.
    /// What `scope_runs` gathers by, and the reason a place
    /// is recorded for a run that was never drawn — see `drawn`.
    scope: Option<Key>,
    /// Whether the run was painted. False for a run inside a selection
    /// scope that the frame culled: emission skips a node clipped
    /// entirely away, but a selection reaching past the viewport needs
    /// that node's content and its place in the order, so a scoped run
    /// records where it *would* have been. Hit tests and the public
    /// `text_hit` / `caret_rect` queries ignore these, because a point
    /// nobody can click still answers nothing.
    drawn: bool,
}

impl TextPlace {
    fn answers_to(&self, key: Key) -> bool {
        if self.key == key {
            return true;
        }
        // Found among the ancestors, and not through a `role="none"`
        // subtree below the key: a gutter's text is not the line's, and
        // is still the gutter's own.
        self.ancestors[..self.depth as usize]
            .iter()
            .position(|k| *k == key)
            .is_some_and(|d| d <= self.none_at as usize)
    }
}

/// One run inside a selection scope: where it was placed, its text, and
/// the byte offset its content starts at in the scope's concatenation.
/// The bases count content only — a separator between two runs is a
/// decision the *copy* makes (see `TextSystem::scope_slice`), so an
/// offset means the same thing whoever asks for it.
pub(crate) struct ScopeRun<'a> {
    pub place: &'a TextPlace,
    /// The run's entry, whichever way it was shaped.
    pub text: &'a Entry,
    pub base: usize,
}

impl ScopeRun<'_> {
    /// The half-open byte range this run occupies in the concatenation.
    /// The run's `[start, end)` in the scope's concatenation.
    pub(crate) fn span(&self) -> (usize, usize) {
        (self.base, self.base + self.text.content().len())
    }
}

/// Where a point landed in the text a keyed node drew: a byte offset and
/// the visual row it is on. `byte` is a caret position: between two
/// characters, past the last one at the end, and cosmic-text's rule for
/// which side of a glyph the point fell on. For a node holding several
/// text runs the offset runs across them in tree order, the way the
/// access tree reads a `line`. `line` is the **visual row** within the
/// node, 0-based, counted across every run the key covers by where the
/// rows sit: a `line` row of three inline runs is one row, a run that
/// wrapped is as many as it wrapped to, and two runs stacked are two —
/// not the wrapped line within one run's buffer, and not the ordinal `role="line"` node a pointer
/// event's `line` names (that one counts rows of the editor, this one
/// rows of the text asked about).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextHit {
    pub byte: usize,
    pub line: u32,
}

impl TextHit {
    /// `{byte, line}`.
    pub fn to_value(self) -> Value {
        Value::map([
            ("byte", Value::Int(self.byte as i64)),
            ("line", Value::Int(self.line as i64)),
        ])
    }
}

/// What a piece of text measures, in logical px at the current scale —
/// the same numbers layout uses for a text node with that content and
/// style, so a view can size a column to its widest label or pick a tier
/// that fits without hand-tuned magic numbers.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TextMetrics {
    pub width: f32,
    pub height: f32,
    /// Lines after wrapping (capped by `max_lines`).
    pub lines: u32,
}

impl TextMetrics {
    /// `{width, height, lines}`.
    pub fn to_value(self) -> Value {
        Value::map([
            ("width", Value::float(self.width)),
            ("height", Value::float(self.height)),
            ("lines", Value::Int(self.lines as i64)),
        ])
    }
}

/// The shaping and rasterization state of one window. The font database
/// it shapes against is not in here — that is the session's
/// ([`crate::session::Session`]), passed in as `fs` — because a font
/// registered in one window has to shape in every window of the session.
/// What is here is coupled to this window's glyph atlas: the shaped-buffer
/// cache stamps its positioned glyphs with the atlas stamp they were
/// packed against, and the frame lists are what `TextId` indexes.
pub struct TextSystem {
    raster: Raster,
    /// The rounded span backgrounds this frame emitted, for the pass that
    /// joins them once every text is painted.
    joins: Vec<JoinBg>,
    /// Every shaped run and every long line, by key — a long line's key
    /// is salted (`LONG_SALT`) and its chunks are runs beside it.
    entries: FxHashMap<u64, Entry>,
    /// The sum of the entries' `bytes`, kept exact against inserts and
    /// removals so a frame inside its budget costs one comparison.
    bytes: usize,
    budget: usize,
    /// This frame's text nodes, and the previous frame's kept the same way
    /// and on the same condition as `Core`'s previous tree: a departing
    /// subtree's text nodes carry that frame's `TextId`s, and this is what
    /// they index (see [`Self::prev_frame_text`]).
    frame: Kept<FrameText>,
    /// Where this frame's text nodes were drawn, and where the last
    /// frame's were — always kept: a query during a build answers from the
    /// frame that finished, which is the layout a click was made against.
    places: Kept<TextPlace>,
    scale: f32,
    frame_no: u64,
    /// Emission broke a long line into rows other than the ones layout
    /// used: the frame laid out on them is owed. Taken by [`Self::take_owed`].
    owed: bool,
}

/// A style's features in cosmic-text's terms. Empty stays empty, which is
/// the shaper's own defaults.
pub(crate) fn cosmic_features(f: &crate::spec::FontFeatures) -> cosmic_text::FontFeatures {
    let mut out = cosmic_text::FontFeatures::new();
    for (tag, value) in f.iter() {
        out.set(cosmic_text::FeatureTag::new(tag), value);
    }
    out
}

/// The faces the three generic families resolve to, per platform, in
/// order of preference. cosmic-text's own defaults are `Open Sans`,
/// `DejaVu Serif` and `Noto Sans Mono`, none of which a stock Windows or
/// macOS machine has; sans survives that through the platform fallback
/// list, but a missing monospace family falls to the *lowest-id*
/// monospaced face in the database — style is not in that ranking's key,
/// so on a machine whose first monospaced face is an italic instance every
/// `Mono` glyph is italic. The first installed name in each
/// list wins; when none is, cosmic-text's own name stays so its fallback
/// still runs.
#[cfg(target_os = "macos")]
const DEFAULT_FAMILIES: [&[&str]; 3] = [
    &["Helvetica Neue", "Helvetica"],
    &["Times New Roman", "Times"],
    &["SF Mono", "Menlo", "Monaco"],
];
#[cfg(target_os = "windows")]
const DEFAULT_FAMILIES: [&[&str]; 3] = [
    &["Segoe UI", "Arial"],
    &["Times New Roman"],
    &["Cascadia Mono", "Consolas", "Courier New"],
];
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const DEFAULT_FAMILIES: [&[&str]; 3] = [
    &["DejaVu Sans", "Noto Sans", "Liberation Sans", "Ubuntu"],
    &["DejaVu Serif", "Noto Serif", "Liberation Serif"],
    &[
        "DejaVu Sans Mono",
        "Noto Sans Mono",
        "Liberation Mono",
        "Ubuntu Mono",
    ],
];

/// The first family on `list` that some installed face is a member of,
/// matched the way `fontdb::Database::query` matches a name.
fn first_installed<'a>(db: &cosmic_text::fontdb::Database, list: &[&'a str]) -> Option<&'a str> {
    list.iter().copied().find(|name| {
        db.faces()
            .any(|face| face.families.iter().any(|(f, _)| f == name))
    })
}

/// The session's font database, set up the way kui shapes against it: the
/// generic sans, serif and monospace families pinned to an installed face
/// (see [`DEFAULT_FAMILIES`]), so weight and style matching starts from a
/// face with real variants rather than whatever the fallback pops.
/// Faces whose glyphs cannot be measured are taken out first.
///
/// The system's faces are scanned and checked once a process
/// ([`system_fonts`]); every session after the first starts from a copy of
/// that database. The scan opens every font file twice (fontdb's, then
/// [`keep_measurable`]'s) — on a Mac 1312 faces in ~850 files, 20–70 ms and
/// most of it `open`, paid again by every `Core::new` — where a copy is a
/// list of names. A font installed while the process runs is seen when
/// something asks for a new scan (`Core::reload_system_fonts`).
pub(crate) fn new_font_system() -> (FontSystem, std::sync::Arc<SystemFonts>) {
    let system = system_fonts();
    let fonts = font_system_over(system.locale.clone(), system.db.clone(), &[]);
    (fonts, system)
}

/// A font system over `db` whose fallback asks `first`'s families before
/// the platform's ([`AppFallback`]). cosmic-text reads its fallback lists
/// when the system is built and keeps them, so a new list is a new system
/// over the same database: the ids stay.
pub(crate) fn font_system_over(
    locale: String,
    db: cosmic_text::fontdb::Database,
    first: &[String],
) -> FontSystem {
    FontSystem::new_with_locale_and_db_and_fallback(locale, db, AppFallback::new(first))
}

/// The platform's fallback lists with the app's families ahead of them
/// (`Core::set_fallback_fonts`, backlog F121): a character the text's own
/// family has no glyph for is asked of each of the app's, in order, before
/// the script's faces and the platform's common ones — on macOS the
/// system's proportional face, which is no editor's second choice.
/// With none it is the platform's lists as they are.
pub(crate) struct AppFallback {
    platform: cosmic_text::PlatformFallback,
    first: &'static [&'static str],
    common: &'static [&'static str],
    /// A script's list with `first` ahead of it, made when first asked
    /// for.
    scripts: std::sync::Mutex<
        rustc_hash::FxHashMap<(unicode_script::Script, String), &'static [&'static str]>,
    >,
}

/// `names` for the life of the process, each spelling and each list kept
/// once: cosmic-text's lists are `&'static`, and an app sets a handful.
fn keep(names: Vec<&'static str>) -> &'static [&'static str] {
    static LISTS: std::sync::Mutex<Vec<&'static [&'static str]>> =
        std::sync::Mutex::new(Vec::new());
    let mut lists = LISTS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(&kept) = lists.iter().find(|l| ***l == names[..]) {
        return kept;
    }
    let kept: &'static [&'static str] = Box::leak(names.into_boxed_slice());
    lists.push(kept);
    kept
}

fn keep_name(name: &str) -> &'static str {
    static NAMES: std::sync::Mutex<Vec<&'static str>> = std::sync::Mutex::new(Vec::new());
    let mut names = NAMES.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(&kept) = names.iter().find(|n| **n == name) {
        return kept;
    }
    let kept: &'static str = Box::leak(name.to_string().into_boxed_str());
    names.push(kept);
    kept
}

impl AppFallback {
    pub(crate) fn new(first: &[String]) -> Self {
        use cosmic_text::Fallback;
        let platform = cosmic_text::PlatformFallback;
        let first = keep(first.iter().map(|n| keep_name(n)).collect());
        let common = keep(
            first
                .iter()
                .chain(platform.common_fallback())
                .copied()
                .collect(),
        );
        Self {
            platform,
            first,
            common,
            scripts: Default::default(),
        }
    }
}

impl cosmic_text::Fallback for AppFallback {
    fn common_fallback(&self) -> &[&'static str] {
        self.common
    }

    fn forbidden_fallback(&self) -> &[&'static str] {
        self.platform.forbidden_fallback()
    }

    fn script_fallback(&self, script: unicode_script::Script, locale: &str) -> &[&'static str] {
        let theirs = self.platform.script_fallback(script, locale);
        if self.first.is_empty() {
            return theirs;
        }
        let mut scripts = self.scripts.lock().unwrap_or_else(|e| e.into_inner());
        scripts
            .entry((script, locale.to_string()))
            .or_insert_with(|| keep(self.first.iter().chain(theirs).copied().collect()))
    }
}

/// One scan of the system's fonts, checked and pinned the way kui shapes
/// against them, and which files' faces it found — what a session started
/// from, so a later scan can say what came and went since.
pub(crate) struct SystemFonts {
    locale: String,
    db: cosmic_text::fontdb::Database,
    /// Each face the scan kept, by its file and its index in the file.
    pub(crate) faces: rustc_hash::FxHashSet<(std::path::PathBuf, u32)>,
}

/// The process's scan; `None` until the first session asks.
static SYSTEM: std::sync::Mutex<Option<std::sync::Arc<SystemFonts>>> = std::sync::Mutex::new(None);

/// The process's scan of the system's fonts, made on the first call. Held
/// under the lock while it scans, so sessions made at once on several
/// threads wait for one scan rather than each making its own.
pub(crate) fn system_fonts() -> std::sync::Arc<SystemFonts> {
    let mut held = SYSTEM.lock().unwrap_or_else(|e| e.into_inner());
    held.get_or_insert_with(|| std::sync::Arc::new(scan_system_fonts()))
        .clone()
}

/// Scans the system's fonts again and makes that the process's scan, so
/// sessions made from here on start from it; returns it.
pub(crate) fn rescan_system_fonts() -> std::sync::Arc<SystemFonts> {
    let fresh = std::sync::Arc::new(scan_system_fonts());
    *SYSTEM.lock().unwrap_or_else(|e| e.into_inner()) = Some(fresh.clone());
    fresh
}

fn scan_system_fonts() -> SystemFonts {
    let (locale, db) = FontSystem::new().into_locale_and_db();
    SystemFonts::from_db(locale, db)
}

impl SystemFonts {
    /// A scan from a database already loaded: the unmeasurable faces taken
    /// out and the generic families pinned.
    pub(crate) fn from_db(locale: String, mut db: cosmic_text::fontdb::Database) -> Self {
        let all: Vec<_> = db.faces().map(|face| face.id).collect();
        keep_measurable(&mut db, all);
        pin_default_families(&mut db);
        let faces = db.faces().filter_map(face_file).collect();
        Self { locale, db, faces }
    }

    /// This scan's database, to build a test's own scan from.
    #[cfg(test)]
    pub(crate) fn db(&self) -> &cosmic_text::fontdb::Database {
        &self.db
    }

    #[cfg(test)]
    pub(crate) fn locale(&self) -> &str {
        &self.locale
    }
}

/// The file a face was read from and its index there; `None` for a face
/// loaded from bytes.
pub(crate) fn face_file(face: &cosmic_text::fontdb::FaceInfo) -> Option<(std::path::PathBuf, u32)> {
    use cosmic_text::fontdb::Source;
    match &face.source {
        Source::File(path) | Source::SharedFile(path, _) => Some((path.clone(), face.index)),
        Source::Binary(_) => None,
    }
}

/// Points the generic sans, serif and monospace families at the first
/// installed family of [`DEFAULT_FAMILIES`]' lists; a list with none
/// installed leaves its generic as it was.
pub(crate) fn pin_default_families(db: &mut cosmic_text::fontdb::Database) {
    let [sans, serif, mono] = DEFAULT_FAMILIES;
    if let Some(name) = first_installed(db, sans) {
        db.set_sans_serif_family(name);
    }
    if let Some(name) = first_installed(db, serif) {
        db.set_serif_family(name);
    }
    if let Some(name) = first_installed(db, mono) {
        db.set_monospace_family(name);
    }
}

/// [`new_font_system`] over a database already loaded. The font system is
/// built after the unmeasurable faces are out, so cosmic-text's list of
/// monospaced faces, which `Mono`'s fallback walks, never names one.
#[cfg(test)]
fn font_system_with(locale: String, db: cosmic_text::fontdb::Database) -> FontSystem {
    let system = SystemFonts::from_db(locale, db);
    FontSystem::new_with_locale_and_db(system.locale, system.db)
}

/// Whether the shaper can say how wide a face's glyphs are:
/// a `head` whose units per em are in the range the spec allows
/// (16–16384), an `hhea` with at least one horizontal metric, and an
/// `hmtx` as long as that says. Every advance is design units over units
/// per em, so a face without a readable `head` shapes to infinitely wide
/// glyphs — macOS's GB18030 Bitmap, which carries Apple's `bhed` in its
/// place — and swash, which reads `hmtx` for the raster, subtracts one
/// from a zero count. Read with skrifa, the reader the shaper measures
/// with, from the table directory, two fixed-size headers and `hmtx`'s
/// length: nothing that can panic on the face it is there to catch.
pub(crate) fn measurable(data: &[u8], index: u32) -> bool {
    use cosmic_text::skrifa::raw::{FontRef, TableProvider};
    let Ok(font) = FontRef::from_index(data, index) else {
        return false;
    };
    font.head()
        .is_ok_and(|head| (16..=16384).contains(&head.units_per_em()))
        && font.hhea().is_ok_and(|hhea| hhea.number_of_h_metrics() > 0)
        && font.hmtx().is_ok()
}

/// Files per checking thread, below which [`keep_measurable`] checks on
/// the caller's thread: a font added from bytes, or a folder of a few.
const FILES_PER_CHECKER: usize = 64;

/// Takes the faces of `ids` the shaper cannot measure out of `db` (see
/// [`measurable`]) and returns the rest, so a face that would shape to
/// infinitely wide glyphs is not a family to list, to name or to fall
/// back to. A face whose file cannot be read goes too: the
/// shaper could not load it either.
///
/// Each file is opened once for all its faces, and the files are spread
/// over a few threads: opening one is most of the cost. Measured over the
/// 1312 faces in 850 files of a Mac, the check adds about 4.5 ms to the
/// ~20 ms fontdb takes to scan them warm (release build); on one thread
/// it took 15–22 ms.
pub(crate) fn keep_measurable(
    db: &mut cosmic_text::fontdb::Database,
    ids: Vec<cosmic_text::fontdb::ID>,
) -> Vec<cosmic_text::fontdb::ID> {
    use cosmic_text::fontdb::{ID, Source};
    // The faces of one file together; bytes already in memory each alone.
    let mut by_file = FxHashMap::<&std::path::Path, Vec<ID>>::default();
    let mut groups = Vec::new();
    for &id in &ids {
        match db.face(id).map(|face| &face.source) {
            Some(Source::File(path) | Source::SharedFile(path, _)) => {
                by_file.entry(path).or_default().push(id);
            }
            Some(Source::Binary(_)) => groups.push(vec![id]),
            None => {}
        }
    }
    groups.extend(by_file.into_values());
    let db_ref = &*db;
    let unmeasurable_in = |groups: &[Vec<ID>]| -> Vec<ID> {
        let mut out = Vec::new();
        for group in groups {
            let read = db_ref.with_face_data(group[0], |data, _| {
                group
                    .iter()
                    .filter(|&&id| {
                        db_ref
                            .face(id)
                            .is_none_or(|face| !measurable(data, face.index))
                    })
                    .copied()
                    .collect::<Vec<_>>()
            });
            out.extend(read.unwrap_or_else(|| group.clone()));
        }
        out
    };
    let checkers = std::thread::available_parallelism()
        .map_or(1, |n| n.get())
        .min(groups.len().div_ceil(FILES_PER_CHECKER));
    let unmeasurable = if checkers <= 1 {
        unmeasurable_in(&groups)
    } else {
        let per = groups.len().div_ceil(checkers);
        std::thread::scope(|scope| {
            let parts: Vec<_> = groups
                .chunks(per)
                .map(|part| {
                    let spawned = std::thread::Builder::new()
                        .name("kui-font-check".into())
                        .spawn_scoped(scope, move || unmeasurable_in(part));
                    (part, spawned.ok())
                })
                .collect();
            let mut out = Vec::new();
            for (part, checker) in parts {
                out.extend(match checker {
                    Some(checker) => checker
                        .join()
                        .unwrap_or_else(|panic| std::panic::resume_unwind(panic)),
                    // No thread to be had: check that part here.
                    None => unmeasurable_in(part),
                });
            }
            out
        })
    };
    for &id in &unmeasurable {
        db.remove_face(id);
    }
    ids.into_iter()
        .filter(|id| !unmeasurable.contains(id))
        .collect()
}

/// The family names `Sans`, `Serif` and `Mono` shape with, in that order —
/// what [`new_font_system`] pinned, or cosmic-text's own name where nothing
/// on the list was installed.
pub(crate) fn default_families(fs: &FontSystem) -> [&str; 3] {
    let db = fs.db();
    [
        db.family_name(&cosmic_text::Family::SansSerif),
        db.family_name(&cosmic_text::Family::Serif),
        db.family_name(&cosmic_text::Family::Monospace),
    ]
}

impl TextSystem {
    pub fn new() -> Self {
        Self {
            raster: Raster::new(),
            joins: Vec::new(),
            entries: FxHashMap::default(),
            bytes: 0,
            budget: DEFAULT_TEXT_CACHE_BYTES,
            frame: Kept::default(),
            places: Kept::default(),
            scale: 1.0,
            frame_no: 0,
            owed: false,
        }
    }

    /// Whether this frame's emission owes another frame,
    /// clearing it.
    pub(crate) fn take_owed(&mut self) -> bool {
        std::mem::take(&mut self.owed)
    }

    /// The rounded backgrounds emitted since the last call: what the
    /// frame's join pass shapes (`crate::join`).
    pub(crate) fn take_joins(&mut self) -> Vec<JoinBg> {
        std::mem::take(&mut self.joins)
    }

    /// This window's rasterizer, for glyph raster against its atlas.
    pub(crate) fn raster_mut(&mut self) -> &mut Raster {
        &mut self.raster
    }

    pub(crate) fn subpixel(&self) -> bool {
        self.raster.subpixel
    }

    /// Switches outline rasterization between alpha masks and LCD subpixel
    /// coverage. Returns whether it changed (the caller resets the atlas).
    pub(crate) fn set_subpixel(&mut self, on: bool) -> bool {
        let changed = self.raster.subpixel != on;
        self.raster.subpixel = on;
        changed
    }

    /// The byte budget the cache is evicted to; see
    /// `Core::set_text_cache_budget`.
    pub fn budget(&self) -> usize {
        self.budget
    }

    /// Sets the budget. Takes effect at the next frame's start, where
    /// eviction runs; nothing is evicted here.
    pub fn set_budget(&mut self, bytes: usize) {
        self.budget = bytes;
    }

    /// The estimated bytes the cache holds (see `ENTRY_BASE_BYTES`).
    pub fn bytes(&self) -> usize {
        self.bytes
    }

    /// How many shaped texts the cache holds (a long line's chunks each
    /// count; the line itself does not).
    pub fn len(&self) -> usize {
        self.entries.len() - self.long_lines()
    }

    /// How many long lines are held.
    pub fn long_lines(&self) -> usize {
        self.entries.values().filter(|e| e.long().is_some()).count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Evicts the least recently used entries until the cache is under
    /// three quarters of its budget. Never an entry the frame that just
    /// finished drew: what is on screen stays shaped whatever the budget
    /// says, the way F26's declared state is never evicted. Ordered by
    /// last use and then by key, so the same history evicts the same
    /// entries whatever order the map iterated.
    fn evict_to_budget(&mut self, fs: &mut FontSystem) {
        if self.bytes <= self.budget {
            return;
        }
        let floor = self.budget / EVICT_TO_DENOMINATOR * EVICT_TO_NUMERATOR;
        let drawn_last_frame = self.frame_no.saturating_sub(1);
        // Oldest first, runs and long lines alike: a long line's record is
        // charged to `bytes` like any entry (its shaped chunks are entries
        // in their own right), so the budget reaches it too rather than
        // leaving the records to the 300-frame sweep alone.
        let mut order: Vec<(u64, u64)> = self
            .entries
            .iter()
            .filter(|(_, e)| e.last_used() < drawn_last_frame)
            .map(|(k, e)| (e.last_used(), *k))
            .collect();
        order.sort_unstable();
        for (_, key) in order {
            if self.bytes <= floor {
                break;
            }
            let freed = self.entries.remove(&key).map(|e| e.bytes());
            self.bytes -= freed.unwrap_or(0);
        }
        // The words those entries shaped are still in cosmic-text's
        // shape-run cache, which has no byte budget of its own and ages
        // only when trimmed: drop what has not been used since the last
        // eviction. Measured with the 2 MB budget in `tests/text_budget.rs`,
        // leaving it on its 240-frame clock held nine times the budget in
        // words alone. A line still on screen keeps its shaped entry
        // whatever happens here; only a line edited afterwards shapes its
        // words again, once.
        fs.shape_run_cache.trim(0);
    }

    /// Starts a frame. `keep_prev` retains the list just finished so the
    /// next frame can still read its texts — `Core` sets it exactly when it
    /// keeps the previous tree, and the two are read together.
    pub(crate) fn begin_frame(
        &mut self,
        fs: &mut FontSystem,
        scale: f32,
        keep_prev: bool,
        frame_no: u64,
    ) {
        self.joins.clear();
        // Scale change invalidates every physical-px measurement.
        if (scale - self.scale).abs() > f32::EPSILON {
            self.entries.clear();
            self.bytes = 0;
        }
        self.scale = scale;
        self.frame.begin(keep_prev);
        // Always kept, unlike `frame`: a query while this frame builds
        // answers from the last one, and this is what it answers from.
        self.places.begin(true);
        // The core's counter, not one of this store's own (backlog AR45).
        self.frame_no = frame_no;
        if let Some(cutoff) = crate::retain::sweep_cutoff(self.frame_no) {
            let mut freed = 0usize;
            self.entries.retain(|_, e| {
                let keep = e.last_used() >= cutoff;
                if !keep {
                    freed += e.bytes();
                }
                keep
            });
            self.bytes -= freed;
            // The shape-run cache makes single-line reshapes ~free while
            // editing; trim it so long sessions don't grow unboundedly.
            fs.shape_run_cache.trim(2);
        }
        // The clock above frees what nobody has shown for five seconds;
        // the budget frees what a stream of new text piles up faster than
        // that (backlog C16).
        self.evict_to_budget(fs);
    }

    /// Drops every shaped entry, to shape again on its next draw: the
    /// weights a family is asked at changed under them.
    pub(crate) fn forget_shaped(&mut self) {
        self.entries.clear();
        self.bytes = 0;
    }

    /// Inserts a fresh entry, charging it to the budget.
    fn insert(&mut self, key: u64, entry: Entry) {
        self.bytes += entry.bytes();
        if let Some(old) = self.entries.insert(key, entry) {
            self.bytes -= old.bytes();
        }
    }

    /// The shaped run under `key`, if the entry there is one.
    fn run(&self, key: u64) -> Option<&CachedText> {
        self.entries.get(&key)?.run()
    }

    fn run_mut(&mut self, key: u64) -> Option<&mut CachedText> {
        self.entries.get_mut(&key)?.run_mut()
    }

    /// The long line under `key`, if the entry there is one.
    fn long(&self, key: u64) -> Option<&LongLine> {
        self.entries.get(&key)?.long()
    }

    fn long_mut(&mut self, key: u64) -> Option<&mut LongLine> {
        self.entries.get_mut(&key)?.long_mut()
    }

    pub(crate) fn style_key(content: &str, style: &TextStyle, scale: f32) -> u64 {
        // The content (word-wide past a few bytes, backlog C43) and the
        // shaping-relevant style bits (color excluded).
        let mut h = crate::key::mix_content(crate::key::FNV_OFFSET, content.as_bytes());
        let mut mix = |bytes: &[u8]| h = crate::key::fnv(h, bytes);
        mix(&style.size.to_bits().to_le_bytes());
        mix(&style.line_height.to_bits().to_le_bytes());
        mix(&scale.to_bits().to_le_bytes());
        mix(&[style.wrap as u8, style.ellipsis as u8]);
        mix(&style.max_lines.to_le_bytes());
        let (tag, font) = match style.family {
            FontFamily::Sans => (0u8, 0u64),
            FontFamily::Serif => (1, 0),
            FontFamily::Mono => (2, 0),
            FontFamily::Custom(id) => (3, id.to_ffi()),
        };
        mix(&[tag]);
        mix(&font.to_le_bytes());
        for (t, v) in style.features.iter() {
            mix(t);
            mix(&v.to_le_bytes());
        }
        // Paint only, but a decorated text is a different entry: the
        // decoration rects are built beside the glyph templates — and
        // their shape and colour with them (backlog K4).
        mix(&[
            style.underline as u8,
            style.strikethrough as u8,
            style.underline_style as u8,
            style.underline_color.is_some() as u8,
        ]);
        if let Some(c) = style.underline_color {
            mix(&c.to_hex().to_le_bytes());
        }
        h
    }

    /// Shapes (or reuses) the buffer for `content` in `style`; returns its
    /// cache key. Shared by text nodes and measurement, so measuring a
    /// string and then drawing it shapes once.
    fn intern(
        &mut self,
        content: &str,
        style: &TextStyle,
        res: &Resources,
        fs: &mut FontSystem,
    ) -> u64 {
        let key = Self::style_key(content, style, self.scale);
        self.intern_keyed(key, content, style, res, fs)
    }

    /// [`Self::intern`] under a key already computed.
    fn intern_keyed(
        &mut self,
        key: u64,
        content: &str,
        style: &TextStyle,
        res: &Resources,
        fs: &mut FontSystem,
    ) -> u64 {
        let frame_no = self.frame_no;
        let scale = self.scale;
        if !self.entries.contains_key(&key) {
            let mut buffer = new_buffer(fs, style, scale);
            // At the family's regular, as a span or a cell is: a family
            // with no 400 face asked at 400 falls back to another family
            // (F100).
            buffer.set_text(
                content,
                &res.weights_of(style.family).apply(
                    Attrs::new()
                        .family(res.family_of(style.family))
                        .font_features(cosmic_features(&style.features)),
                    false,
                ),
                Shaping::Advanced,
                None,
            );
            let entry = CachedText::new(
                buffer,
                content.to_string(),
                style,
                vec![SpanDeco::of_style(style)],
                fs,
                frame_no,
            );
            self.insert(key, Entry::Run(entry));
        }
        self.entries
            .get_mut(&key)
            .expect("just inserted")
            .touch(frame_no);
        key
    }

    /// The content of one of this frame's texts (spans concatenated).
    pub(crate) fn content(&self, id: TextId) -> &str {
        let key = self.frame[id.0 as usize].cache_key;
        self.entries.get(&key).map_or("", |e| e.content())
    }

    /// Appends the access runs of one of this frame's texts, as `src`
    /// places them (a custom editor's `line`s are read this way): a run's
    /// laid-out lines, or a long line's rows of the chunks it has shaped —
    /// one that never showed is not walked, the way a tall document's
    /// off-screen lines are not. Every `break-spaces` text is a long line.
    pub(crate) fn access_runs(
        &self,
        id: TextId,
        src: crate::access::RunSource,
        run_no: &mut usize,
        out: &mut Vec<crate::access::AccessRun>,
    ) {
        use crate::access::{RowGlyphs, push_row_runs};
        let Some(ft) = self.frame.get(id.0 as usize) else {
            return;
        };
        let line = match self.entries.get(&ft.cache_key) {
            Some(Entry::Run(e)) => {
                crate::access::runs_of_buffer(&e.buffer, src, run_no, out);
                return;
            }
            Some(Entry::Long(line)) => line,
            None => return,
        };
        let whole = [RowStart { byte: 0, x: 0.0 }];
        for (i, c) in line.chunks.iter().enumerate() {
            let Some(run) = c
                .width
                .and_then(|_| self.run(c.key))
                .and_then(|e| e.buffer.layout_runs().next())
            else {
                continue;
            };
            // Wrapped, the chunk's rows from where its first begins;
            // else one row at its place along the line.
            let (rows, row0, head_x) = match (line.wrap_w, line.starts.get(i)) {
                (Some(_), Some(&(row0, head_x))) if !c.rows.is_empty() => {
                    (&c.rows[..], row0, head_x)
                }
                (Some(_), _) => continue,
                (None, _) => (&whole[..], 0, line.prefix[i]),
            };
            for (r, rs) in rows.iter().enumerate() {
                let hi = rows.get(r + 1).map_or(usize::MAX, |n| n.byte as usize);
                let a = run.glyphs.partition_point(|g| g.start < rs.byte as usize);
                let b = run.glyphs.partition_point(|g| g.start < hi).max(a);
                if a == b {
                    continue;
                }
                let last = i + 1 == line.chunks.len() && r + 1 == rows.len();
                push_row_runs(
                    &RowGlyphs {
                        glyphs: &run.glyphs[a..b],
                        text: run.text,
                        line: 0,
                        top: (row0 as usize + r) as f32 * line.line_h,
                        height: line.line_h,
                        rtl: run.rtl,
                        dx: if r == 0 { head_x } else { 0.0 } - rs.x,
                        base: c.start,
                        newline: last && src.newline_after_last,
                    },
                    &src,
                    run_no,
                    out,
                );
            }
        }
    }

    /// The cache key and colour behind one of the *previous* frame's texts
    /// — what a departing subtree keeps instead of its `TextId`, which
    /// indexes a list rebuilt every frame. The subtree is copied out of the
    /// previous frame's tree, so this is the list its ids belong to (see
    /// [`crate::depart`]).
    pub(crate) fn prev_frame_text(&self, id: TextId) -> (u64, Color) {
        match self.frame.prev().get(id.0 as usize) {
            Some(t) => (t.cache_key, t.color),
            None => (0, Color::TRANSPARENT),
        }
    }

    /// Registers an already-shaped buffer as one of this frame's texts, by
    /// the cache key [`Self::frame_text`] handed out. None once the entry
    /// has been evicted — a ghost older than the cache draws no text
    /// rather than a wrong one. Touching it here keeps it alive for as
    /// long as something still draws it.
    pub(crate) fn readd(&mut self, cache_key: u64, color: Color) -> Option<TextId> {
        let frame_no = self.frame_no;
        let e = self.entries.get_mut(&cache_key)?;
        e.touch(frame_no);
        let cache_key = if e.long().is_some() {
            self.claim_long(cache_key)
        } else {
            cache_key
        };
        self.frame.push(FrameText { cache_key, color });
        Some(TextId((self.frame.len() - 1) as u32))
    }

    /// Registers a text for this frame, shaping (or reusing) its buffer.
    pub fn add(
        &mut self,
        content: &str,
        style: &TextStyle,
        res: &Resources,
        fs: &mut FontSystem,
    ) -> TextId {
        // The one place the long/short decision is made: the entry's
        // variant carries it from here.
        let key = self.intern_any(content, style, res, fs);
        let key = if could_be_long(content.len(), style) {
            self.claim_long(key)
        } else {
            key
        };
        self.frame.push(FrameText {
            cache_key: key,
            color: style.color_or_default(),
        });
        TextId((self.frame.len() - 1) as u32)
    }

    /// The key a node drawing the entry `key` holds it by. A run is
    /// shared, re-wrapped to each node as it is drawn; a long line's rows
    /// are the line's own and every query reads them, so the first node
    /// of a frame takes the line and each further one a copy of its own —
    /// the n-th node the n-th copy, so a frame like the last finds the
    /// copies it made laid out already. The copies share the chunks'
    /// shaped runs. Without it one `break-spaces` file open in two panes
    /// of different widths would draw and answer at the width laid out last.
    #[inline(never)]
    fn claim_long(&mut self, key: u64) -> u64 {
        let frame_no = self.frame_no;
        let mut at = key;
        let mut n = 0u64;
        loop {
            match self.entries.get_mut(&at) {
                Some(Entry::Long(l)) if l.claimed != frame_no => {
                    l.claimed = frame_no;
                    l.last_used = frame_no;
                    return at;
                }
                // A run is never a long line's copy: its key is.
                Some(Entry::Run(_)) if n == 0 => return key,
                Some(_) => {}
                None => {
                    let Some(line) = self.long(key) else {
                        return key;
                    };
                    let mut copy = line.clone();
                    copy.claimed = frame_no;
                    copy.last_used = frame_no;
                    self.insert(at, Entry::Long(copy));
                    return at;
                }
            }
            n += 1;
            at = crate::key::fnv(key ^ COPY_SALT, &n.to_le_bytes());
        }
    }

    /// Interns `content` as the long line it is or the run it is, one
    /// hash either way: a text that could be long by its length and style
    /// is looked up under both keys before its bytes are scanned for a
    /// line break, so the steady state of a megabyte line is its hash and
    /// two lookups.
    fn intern_any(
        &mut self,
        content: &str,
        style: &TextStyle,
        res: &Resources,
        fs: &mut FontSystem,
    ) -> u64 {
        if !could_be_long(content.len(), style) {
            return self.intern(content, style, res, fs);
        }
        let key = Self::style_key(content, style, self.scale);
        let long_key = key ^ LONG_SALT;
        let frame_no = self.frame_no;
        if let Some(e) = self.entries.get_mut(&long_key) {
            e.touch(frame_no);
            return long_key;
        }
        if let Some(e) = self.entries.get_mut(&key) {
            e.touch(frame_no);
            return key;
        }
        if has_line_break(content) || content.len() < LONG_LINE_BYTES && has_rtl(content) {
            self.intern_keyed(key, content, style, res, fs)
        } else {
            self.build_long(long_key, content.to_string(), Vec::new(), style, res, fs)
        }
    }

    /// The rich counterpart of [`Self::intern_any`]: a paragraph past the
    /// threshold with no line break is a long line whose chunks are rich
    /// runs; its content is concatenated only when the line
    /// is built.
    fn intern_rich_any(
        &mut self,
        spans: &[Span<'_>],
        base: &TextStyle,
        res: &Resources,
        fs: &mut FontSystem,
    ) -> u64 {
        let len: usize = spans.iter().map(|s| s.text.len()).sum();
        if !could_be_long(len, base) {
            return self.intern_rich(spans, base, res, fs);
        }
        let key = Self::rich_key(spans, base, self.scale);
        let long_key = key ^ LONG_SALT;
        let frame_no = self.frame_no;
        if let Some(e) = self.entries.get_mut(&long_key) {
            e.touch(frame_no);
            return long_key;
        }
        if let Some(e) = self.entries.get_mut(&key) {
            e.touch(frame_no);
            return key;
        }
        // A sized span makes a line as tall as it is, which the long
        // line's one estimated row height cannot follow: shaped whole.
        if spans
            .iter()
            .any(|s| has_line_break(s.text) || s.size.is_some())
            || len < LONG_LINE_BYTES && spans.iter().any(|s| has_rtl(s.text))
        {
            return self.intern_rich_keyed(key, spans, base, res, fs);
        }
        let mut content = String::with_capacity(len);
        let mut owned = Vec::with_capacity(spans.len());
        for s in spans {
            let start = content.len();
            content.push_str(s.text);
            owned.push(OwnedSpan {
                start,
                end: content.len(),
                attrs: s.attrs_only(),
            });
        }
        self.build_long(long_key, content, owned, base, res, fs)
    }

    /// Builds the long line `content` is under `key` — plain, or rich
    /// with `spans` — shaping its first chunk for the line
    /// height and the advance the rest are estimated from; see
    /// [`LongLine`]. The callers have looked `key` up already.
    fn build_long(
        &mut self,
        key: u64,
        content: String,
        spans: Vec<OwnedSpan>,
        style: &TextStyle,
        res: &Resources,
        fs: &mut FontSystem,
    ) -> u64 {
        let frame_no = self.frame_no;
        let wrap = style.wrap;
        let style = &TextStyle {
            wrap: TextWrap::None,
            ..*style
        };
        let scale = self.scale;
        let chunks: Vec<Chunk> = chunk_ranges(&content)
            .into_iter()
            .map(|(start, end)| Chunk {
                start,
                end,
                key: if spans.is_empty() {
                    Self::style_key(&content[start..end], style, scale)
                } else {
                    Self::rich_key(&chunk_spans(&content, &spans, start, end), style, scale)
                },
                width: None,
                tab: None,
                rows: Vec::new(),
            })
            .collect();
        // The first chunk is shaped now: the line's height and the mean
        // advance the estimates need come from it.
        let (w0, line_h) = match chunks.first() {
            Some(c) => {
                let k = self.shape_chunk(&content, &spans, c.start, c.end, style, res, fs);
                let e = self.run(k).expect("just interned");
                (e.intrinsic.w, e.buffer.metrics().line_height)
            }
            None => (0.0, style.line_height * self.scale),
        };
        let avg = chunks
            .first()
            .map_or(0.0, |c| w0 / (c.end - c.start).max(1) as f32);
        let mut line = LongLine {
            content,
            spans,
            style: *style,
            wrap,
            wrap_w: None,
            starts: Vec::new(),
            chunks,
            prefix: Vec::new(),
            line_h,
            avg,
            last_used: frame_no,
            claimed: u64::MAX,
            laid_rows: 1,
            asked_rows: u32::MAX,
            bytes: 0,
        };
        if let Some(c) = line.chunks.first_mut() {
            c.width = Some(w0);
        }
        line.reprefix();
        line.bytes = ENTRY_BASE_BYTES
            + line.content.len()
            + line.chunks.len() * 48
            + line.spans.len() * std::mem::size_of::<OwnedSpan>();
        // A line long only by its `break-spaces` is one chunk
        // (`chunk_ranges`), shaped now: its first frame is laid out on the
        // rows it has, not on an estimate (RG68).
        self.insert(key, Entry::Long(line));
        key
    }

    /// Shapes (or touches) the run for `start..end` of a long line's
    /// content: a plain run, or a rich one of the spans that intersect
    /// the range. Returns its key.
    #[allow(clippy::too_many_arguments)]
    fn shape_chunk(
        &mut self,
        content: &str,
        spans: &[OwnedSpan],
        start: usize,
        end: usize,
        style: &TextStyle,
        res: &Resources,
        fs: &mut FontSystem,
    ) -> u64 {
        if spans.is_empty() {
            self.intern(&content[start..end], style, res, fs)
        } else {
            let spans = chunk_spans(content, spans, start, end);
            self.intern_rich(&spans, style, res, fs)
        }
    }

    /// Makes sure chunk `i` of the long line `key` is shaped, and moves
    /// the prefix sums if its width was an estimate. Returns whether it
    /// shaped now — what tells a wrapped line its rows need breaking.
    fn ensure_chunk(&mut self, key: u64, i: usize, res: &Resources, fs: &mut FontSystem) -> bool {
        let (chunk_key, known) = {
            let c = &self.long(key).expect("a long line").chunks[i];
            (c.key, c.width)
        };
        let frame_no = self.frame_no;
        if known.is_some()
            && let Some(e) = self.run_mut(chunk_key)
        {
            e.last_used = frame_no;
            return false;
        }
        // Shaping now: the chunk's text and its spans rebased to the
        // slice, copied out so the line stays in the map meanwhile.
        let (text, spans, style, off_stops) = {
            let line = self.long(key).expect("a long line");
            let c = &line.chunks[i];
            let first = line.spans.partition_point(|s| s.end <= c.start);
            let spans: Vec<OwnedSpan> = line.spans[first..]
                .iter()
                .take_while(|s| s.start < c.end)
                .map(|s| OwnedSpan {
                    start: s.start.max(c.start) - c.start,
                    end: s.end.min(c.end) - c.start,
                    attrs: s.attrs,
                })
                .collect();
            let bytes = line.content.as_bytes();
            let off_stops = c.start > 0 && bytes[c.start - 1] != b'\t';
            (
                line.content[c.start..c.end].to_string(),
                spans,
                line.style,
                off_stops,
            )
        };
        let k = self.shape_chunk(&text, &spans, 0, text.len(), &style, res, fs);
        let w = self.run(k).expect("just interned").intrinsic.w;
        let tab = match off_stops && text.ends_with('\t') {
            true => self.chunk_tab(k, &text, &spans, &style, res, fs),
            false => None,
        };
        let line = self.long_mut(key).expect("just read");
        line.chunks[i].key = k;
        line.chunks[i].tab = tab;
        if line.chunks[i].width != Some(w) {
            line.chunks[i].width = Some(w);
            line.reprefix();
        }
        true
    }

    /// For a chunk that starts off the line's tab stops and ends in its one
    /// tab, shaped as `chunk`: where the tab starts in that run, and the
    /// stops' interval — the width of the tab shaped alone in its own
    /// span's style, which starts on a stop (backlog RG76). `None` for a
    /// right-to-left run, whose tab is not at its end.
    #[inline(never)]
    #[allow(clippy::too_many_arguments)]
    fn chunk_tab(
        &mut self,
        chunk: u64,
        text: &str,
        spans: &[OwnedSpan],
        style: &TextStyle,
        res: &Resources,
        fs: &mut FontSystem,
    ) -> Option<(f32, f32)> {
        let at = text.len() - 1;
        let tab_x = {
            let run = self.run(chunk)?.buffer.layout_runs().next()?;
            if run.rtl {
                return None;
            }
            run.glyphs.iter().rev().find(|g| g.start == at)?.x
        };
        let alone = self.shape_chunk(text, spans, at, text.len(), style, res, fs);
        let every = self.run(alone)?.intrinsic.w;
        Some((tab_x, every))
    }

    /// Breaks the long line `key` into rows at `w` (physical px), or lays
    /// it as one row for `None`. Positions, not glyphs: each chunk's rows
    /// start where the previous chunk's last row ended; a shaped chunk
    /// breaks exactly ([`break_rows`]), one that never showed contributes
    /// the estimate its width is, corrected when it shapes — the way
    /// `prefix` is, so the height the scrollbar sees can move a little as
    /// chunks fill in.
    fn relayout_long(&mut self, key: u64, w: Option<f32>) {
        let Some(w) = w else {
            let line = self.long_mut(key).expect("a long line");
            line.wrap_w = None;
            line.starts.clear();
            for c in &mut line.chunks {
                c.rows.clear();
            }
            return;
        };
        let w = w.max(1.0);
        // Read every chunk's rows off its shaped run first — the runs and
        // the line are entries of one map — then write them back.
        let line = self.long(key).expect("a long line");
        let mut starts = Vec::with_capacity(line.chunks.len() + 1);
        starts.push((0, 0.0));
        let (mut row, mut x) = (0u32, 0.0f32);
        let rows: Vec<Vec<RowStart>> = line
            .chunks
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let shaped = c.width.and_then(|_| self.run(c.key));
                let rows = match shaped {
                    Some(e) => {
                        let text = &line.content[c.start..c.end];
                        let (rows, end) = break_rows(&e.buffer, text, line.wrap, w, x);
                        row += rows.len() as u32 - 1;
                        // A tab the line places ends the chunk where the
                        // unwrapped line puts it, as a whole run's tab is
                        // measured before its rows are broken (RG76).
                        x = end + line.tab_shift(i);
                        rows
                    }
                    None => {
                        let est = x + c
                            .width
                            .unwrap_or_else(|| (c.end - c.start) as f32 * line.avg);
                        let added = (est / w).floor();
                        row += added as u32;
                        x = est - added * w;
                        Vec::new()
                    }
                };
                starts.push((row, x));
                rows
            })
            .collect();
        let line = self.long_mut(key).expect("a long line");
        line.wrap_w = Some(w);
        line.starts = starts;
        for (c, rows) in line.chunks.iter_mut().zip(rows) {
            c.rows = rows;
        }
    }

    /// A long line's size at `max_w` (physical px): one row clamped to it,
    /// or, when the style wraps and the line does not fit, its rows broken
    /// to it. Returns the physical size and the row count.
    fn long_size(&mut self, key: u64, max_w: Option<f32>) -> (Size, u32) {
        let (wrap, width, line_h, cur) = {
            let l = self.long(key).expect("a long line");
            (l.wrap, l.width(), l.line_h, l.wrap_w)
        };
        let target = match max_w {
            Some(w) if wrap != TextWrap::None && width > w + 0.5 => Some(w.max(1.0)),
            _ => None,
        };
        let differs = match (cur, target) {
            (None, None) => false,
            (Some(a), Some(b)) => (a - b).abs() > 0.5,
            _ => true,
        };
        if differs {
            self.relayout_long(key, target);
        }
        match target {
            Some(w) => {
                let rows = self.long(key).expect("a long line").rows();
                (Size::new(w, rows as f32 * line_h), rows)
            }
            None => (Size::new(max_w.map_or(width, |m| width.min(m)), line_h), 1),
        }
    }

    /// Measures `content` in `style` without adding a node: its unwrapped
    /// size, or with `max_w` (logical px) its size once wrapped to that
    /// width. The answer is what layout would give a text node with the
    /// same content and style at the current scale, `wrap` / `max_lines` /
    /// `ellipsis` included.
    pub fn measure(
        &mut self,
        content: &str,
        style: &TextStyle,
        res: &Resources,
        fs: &mut FontSystem,
        max_w: Option<f32>,
    ) -> TextMetrics {
        let key = self.intern_any(content, style, res, fs);
        self.measure_any(key, fs, max_w)
    }

    /// `measure` for the entry under `key`, a long line or a run.
    fn measure_any(&mut self, key: u64, fs: &mut FontSystem, max_w: Option<f32>) -> TextMetrics {
        if self.long(key).is_some() {
            let scale = self.scale;
            // Without a width nothing is broken, and the layout the frame
            // holds is left as it is.
            let (size, lines) = match max_w {
                Some(m) => {
                    // Nor with one: the rows are put back for the node
                    // that drew the line, which queries read.
                    let held = self.long(key).expect("checked").wrap_w;
                    let measured = self.long_size(key, Some(m * scale));
                    if wrap_differs(held, self.long(key).expect("checked").wrap_w) {
                        self.relayout_long(key, held);
                    }
                    measured
                }
                None => {
                    let line = self.long(key).expect("checked");
                    (Size::new(line.width(), line.line_h), 1)
                }
            };
            return TextMetrics {
                width: size.w / scale,
                height: size.h / scale,
                lines,
            };
        }
        self.measure_key(key, fs, max_w)
    }

    /// `measure` for a rich-text paragraph.
    pub fn measure_rich(
        &mut self,
        spans: &[Span<'_>],
        base: &TextStyle,
        res: &Resources,
        fs: &mut FontSystem,
        max_w: Option<f32>,
    ) -> TextMetrics {
        let key = self.intern_rich_any(spans, base, res, fs);
        self.measure_any(key, fs, max_w)
    }

    fn measure_key(&mut self, key: u64, fs: &mut FontSystem, max_w: Option<f32>) -> TextMetrics {
        let scale = self.scale;
        let entry = self.run_mut(key).expect("just interned");
        let target = wrap_target(entry, max_w, scale);
        // A run a node has drawn keeps the rows it was drawn at: its
        // `text_hit` and `caret_rect` read them until the next frame, which
        // would only wrap them back. A measure at another width lays out a
        // copy at that width instead, kept for the next measure there —
        // between frames as well as within one, where RG72's claim is
        // (backlog RG76).
        let key = if entry.claimed != u64::MAX && wrap_differs(entry.wrap, target) {
            self.wrap_copy(key, target, fs)
        } else {
            key
        };
        let entry = self.run_mut(key).expect("just interned");
        wrap_entry(entry, fs, target);
        let (mut m, lines) = measure_buffer(&entry.buffer, entry.max_lines);
        if entry.clamp_w
            && let Some(t) = target
        {
            m.w = m.w.min(t);
        }
        TextMetrics {
            width: m.w / scale,
            height: m.h / scale,
            lines,
        }
    }

    /// Registers a rich-text paragraph for this frame. Spans shape as one
    /// flow, so wrapping crosses style boundaries correctly; past the
    /// long-line threshold the flow is chunked like a plain line's.
    pub fn add_rich(
        &mut self,
        spans: &[Span<'_>],
        base: &TextStyle,
        res: &Resources,
        fs: &mut FontSystem,
    ) -> TextId {
        let key = self.intern_rich_any(spans, base, res, fs);
        let len = spans.iter().map(|s| s.text.len()).sum();
        let key = if could_be_long(len, base) {
            self.claim_long(key)
        } else {
            key
        };
        self.frame.push(FrameText {
            cache_key: key,
            color: base.color_or_default(),
        });
        TextId((self.frame.len() - 1) as u32)
    }

    fn intern_rich(
        &mut self,
        spans: &[Span<'_>],
        base: &TextStyle,
        res: &Resources,
        fs: &mut FontSystem,
    ) -> u64 {
        let key = Self::rich_key(spans, base, self.scale);
        self.intern_rich_keyed(key, spans, base, res, fs)
    }

    /// The cache key of a rich paragraph: the base style's, then every
    /// span's text and attributes in order. Never a plain text's key for
    /// the same content.
    fn rich_key(spans: &[Span<'_>], base: &TextStyle, scale: f32) -> u64 {
        let mut key = Self::style_key("", base, scale) ^ 0x9e37_79b9_7f4a_7c15;
        for s in spans {
            key = crate::key::mix_content(key, s.text.as_bytes());
            let mut mix = |bytes: &[u8]| key = crate::key::fnv(key, bytes);
            mix(&[
                s.bold as u8,
                s.italic as u8,
                s.color.is_some() as u8,
                s.underline as u8,
                s.strikethrough as u8,
                s.bg.is_some() as u8,
                s.underline_style as u8,
                s.underline_color.is_some() as u8,
            ]);
            mix(&s.bg_radius.to_bits().to_le_bytes());
            // The span's own face and size: a different shaping, so a
            // different entry.
            let (tag, font) = match s.family {
                None => (0u8, 0u64),
                Some(FontFamily::Sans) => (1, 0),
                Some(FontFamily::Serif) => (2, 0),
                Some(FontFamily::Mono) => (3, 0),
                Some(FontFamily::Custom(id)) => (4, id.to_ffi()),
            };
            mix(&[tag]);
            mix(&font.to_le_bytes());
            mix(&s.size.map_or(0, f32::to_bits).to_le_bytes());
            for c in [s.color, s.bg, s.underline_color].into_iter().flatten() {
                mix(&c.r.to_bits().to_le_bytes());
                mix(&c.g.to_bits().to_le_bytes());
                mix(&c.b.to_bits().to_le_bytes());
                mix(&c.a.to_bits().to_le_bytes());
            }
        }
        key
    }

    /// [`Self::intern_rich`] under a key already computed.
    fn intern_rich_keyed(
        &mut self,
        key: u64,
        spans: &[Span<'_>],
        base: &TextStyle,
        res: &Resources,
        fs: &mut FontSystem,
    ) -> u64 {
        let frame_no = self.frame_no;
        let scale = self.scale;
        if !self.entries.contains_key(&key) {
            let mut buffer = new_buffer(fs, base, scale);
            let family = res.family_of(base.family);
            let weights = res.weights_of(base.family);
            let features = cosmic_features(&base.features);
            // Once one span has a size of its own every span carries its
            // metrics: cosmic-text makes a line as tall as the tallest
            // glyph that names its own, so a line holding only a smaller
            // span would otherwise come out shorter than the paragraph's.
            let sized = spans.iter().any(|s| s.size.is_some());
            let metrics_of = |size: Option<f32>| {
                let size = size.unwrap_or(base.size);
                let ratio = if base.size > 0.0 {
                    base.line_height / base.size
                } else {
                    1.0
                };
                shaper_metrics(size * scale, size * ratio * scale)
            };
            // Each span's index rides its glyphs as metadata, which is how
            // the decoration rects find their span after layout. A span
            // with a face of its own is shaped in it, at its weights.
            buffer.set_rich_text(
                spans.iter().enumerate().map(|(i, s)| {
                    let (family, weights) = match s.family {
                        Some(f) => (res.family_of(f), res.weights_of(f)),
                        None => (family, weights),
                    };
                    let mut attrs = s
                        .attrs(family, weights)
                        .font_features(features.clone())
                        .metadata(i);
                    if sized {
                        attrs = attrs.metrics(metrics_of(s.size));
                    }
                    (s.text, attrs)
                }),
                &{
                    let attrs = weights.apply(
                        Attrs::new().family(family).font_features(features.clone()),
                        false,
                    );
                    if sized {
                        attrs.metrics(metrics_of(None))
                    } else {
                        attrs
                    }
                },
                Shaping::Advanced,
                None,
            );
            let content = spans.iter().map(|s| s.text).collect::<String>();
            let decos = spans
                .iter()
                .map(|s| SpanDeco {
                    underline: s.underline || base.underline,
                    // The span's own where it says, else the paragraph's.
                    underline_color: s.underline_color.or(base.underline_color),
                    underline_style: if s.underline {
                        s.underline_style
                    } else {
                        base.underline_style
                    },
                    strikethrough: s.strikethrough || base.strikethrough,
                    bg: s.bg,
                    bg_radius: s.bg_radius,
                })
                .collect();
            let entry = CachedText::new(buffer, content, base, decos, fs, frame_no);
            self.insert(key, Entry::Run(entry));
        }
        self.entries
            .get_mut(&key)
            .expect("just inserted")
            .touch(frame_no);
        key
    }

    /// The shaped run behind one of this frame's texts; callers have
    /// already taken the long line's path when it is one.
    fn entry_mut(&mut self, id: TextId) -> &mut CachedText {
        let key = self.frame[id.0 as usize].cache_key;
        self.run_mut(key).expect("frame text missing from cache")
    }

    /// Wraps text `id`'s run to `max_w_logical`: layout at the node's
    /// width, and emission again, since the run is shared by every node
    /// drawing the same text. The first node of a frame claims it; one
    /// asking another width the same frame moves to a copy of its own.
    fn ensure_wrap(&mut self, id: TextId, max_w_logical: f32, fs: &mut FontSystem) {
        let (scale, frame_no) = (self.scale, self.frame_no);
        let entry = self.entry_mut(id);
        let target = wrap_target(entry, Some(max_w_logical), scale);
        if entry.claimed == frame_no && wrap_differs(entry.claimed_wrap, target) {
            self.own_wrap(id, target, fs);
            return;
        }
        entry.claimed = frame_no;
        entry.claimed_wrap = target;
        wrap_entry(entry, fs, target);
    }

    /// `ensure_wrap` for a node drawing a run another node of this frame
    /// wrapped to another width — the same label in two panes: the node
    /// takes the copy of the run at its width, made the first time and
    /// found by width after, so each answers `text_hit` and `caret_rect`
    /// at its own rows and neither re-wraps the other's (a
    /// long line's twin is [`Self::claim_long`]). Off the path every other
    /// text takes: a copy costs a buffer clone once and a lookup a frame.
    #[inline(never)]
    fn own_wrap(&mut self, id: TextId, target: Option<f32>, fs: &mut FontSystem) {
        let frame_no = self.frame_no;
        let key = self.frame[id.0 as usize].cache_key;
        let copy = self.wrap_copy(key, target, fs);
        self.frame[id.0 as usize].cache_key = copy;
        let entry = self.run_mut(copy).expect("just made");
        entry.claimed = frame_no;
        entry.claimed_wrap = target;
    }

    /// The copy of run `key` laid out at `target`, made the first time a
    /// width asks for it and found by width after: what a second node
    /// drawing the run at another width ([`Self::own_wrap`]) and a
    /// `measure_text` at a width no node drew it at ([`Self::measure_key`])
    /// lay out instead of the run a node drew.
    #[inline(never)]
    fn wrap_copy(&mut self, key: u64, target: Option<f32>, fs: &mut FontSystem) -> u64 {
        let frame_no = self.frame_no;
        // By the half pixel `wrap_differs` tells widths apart by.
        let slot = target.map_or(u64::MAX, |t| (t * 2.0).round() as u64);
        let copy = crate::key::fnv(key ^ COPY_SALT, &slot.to_le_bytes());
        if self.run(copy).is_none() {
            let fork = self.run(key).expect("a run").fork(frame_no);
            self.insert(copy, Entry::Run(fork));
        }
        let entry = self.run_mut(copy).expect("just made");
        entry.last_used = frame_no;
        wrap_entry(entry, fs, target);
        copy
    }

    /// Emits positioned glyph quads for a laid-out text node.
    /// `origin` and `node` are logical; output quads are physical px. The
    /// rounded backgrounds it notes are clipped as its parent is, whatever
    /// its own box does (`JoinBg::outer`).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn emit(
        &mut self,
        id: TextId,
        origin: Vec2,
        node: Size,
        clip: Clip,
        clip_id: ClipId,
        clips: &mut Vec<Clip>,
        res: &Resources,
        fs: &mut FontSystem,
        atlas: &mut GlyphAtlas,
        out: &mut Vec<Quad>,
        sel: Option<((usize, usize), Color)>,
    ) {
        let from = self.joins.len();
        self.emit_text(
            id, origin, node, clip, clip_id, clips, res, fs, atlas, out, sel,
        );
        if self.joins.len() == from {
            return;
        }
        // The box a long line or an overflowing text clips to, as
        // `emit_text` works it out.
        let key = self.frame[id.0 as usize].cache_key;
        let clamps = match self.entries.get(&key) {
            Some(Entry::Long(l)) => l.wrap == TextWrap::None,
            Some(Entry::Run(e)) => e.clamp_w,
            None => false,
        };
        let own = clamps.then(|| {
            let ox = crate::geom::snap_px(origin.x * self.scale);
            (ox, ox + (node.w * self.scale).ceil())
        });
        for j in &mut self.joins[from..] {
            j.outer = clip_id;
            j.own = own;
        }
    }

    /// `emit_text` for a long line, kept off the run's path: it draws the
    /// chunks inside the clip plus one either side, shaping them now if
    /// this is the first time they show. Layout broke its
    /// rows at the node's final width (`wrapped`), and the line is this
    /// node's alone (`claim_long`).
    #[allow(clippy::too_many_arguments)]
    #[inline(never)]
    fn emit_long(
        &mut self,
        key: u64,
        node: Size,
        ox: f32,
        oy: f32,
        nudge: Vec2,
        color: Color,
        clip: Clip,
        clip_id: ClipId,
        clips: &mut Vec<Clip>,
        res: &Resources,
        fs: &mut FontSystem,
        atlas: &mut GlyphAtlas,
        out: &mut Vec<Quad>,
        sel: Option<((usize, usize), Color)>,
    ) {
        let scale = self.scale;
        let line = self.long(key).expect("a long line");
        // A line that does not wrap owns its box, as a run that does not
        // does; one that wraps draws under its parent's clip, as a
        // wrapped run does, so a glyph overhanging a tight row is not cut
        // (the alpha.22 regression pass: every `break-spaces` text is a
        // long line).
        let (clip, clip_id) = if line.wrap == TextWrap::None {
            let own = Rect::new(ox, oy, (node.w * scale).ceil(), (node.h * scale).ceil());
            let clip = clip.intersect(own, crate::display::SQUARE);
            if clip.rect.w <= 0.0 || clip.rect.h <= 0.0 {
                return;
            }
            (clip, crate::display::intern_clip(clips, clip))
        } else {
            (clip, clip_id)
        };
        if let Some(((from, to), tint)) = sel {
            let rects = self.long_highlight(line, from, to);
            push_highlight(&rects, ox, oy, nudge, tint, clip_id, out);
        }
        if let Some(w) = line.wrap_w {
            self.emit_long_rows(
                key, w, ox, oy, nudge, color, clip, clip_id, res, fs, atlas, out,
            );
            return;
        }
        let (first, last) = {
            if line.chunks.is_empty() {
                return;
            }
            let a = line.chunk_at(clip.rect.x - ox).saturating_sub(1);
            let b = (line.chunk_at(clip.rect.x + clip.rect.w - ox) + 1).min(line.chunks.len() - 1);
            (a, b)
        };
        for i in first..=last {
            self.ensure_chunk(key, i, res, fs);
        }
        let frame_no = self.frame_no;
        self.long_mut(key).expect("checked").last_used = frame_no;
        for i in first..=last {
            let (chunk_key, x) = {
                let line = self.long(key).expect("checked");
                (line.chunks[i].key, line.prefix[i])
            };
            let raster = &mut self.raster;
            let entry = self
                .entries
                .get_mut(&chunk_key)
                .and_then(Entry::run_mut)
                .expect("just ensured");
            emit_entry(
                entry,
                ox + x,
                oy,
                nudge,
                color,
                clip,
                clip_id,
                raster,
                fs,
                atlas,
                out,
                &mut self.joins,
            );
        }
    }

    /// [`Self::emit`]'s body.
    #[allow(clippy::too_many_arguments)]
    fn emit_text(
        &mut self,
        id: TextId,
        origin: Vec2,
        node: Size,
        clip: Clip,
        clip_id: ClipId,
        clips: &mut Vec<Clip>,
        res: &Resources,
        fs: &mut FontSystem,
        atlas: &mut GlyphAtlas,
        out: &mut Vec<Quad>,
        // `sel`: the bytes of this run the window's selection covers and
        // the colour to paint them under (ADR 0017). Resolved by the
        // caller, the only place that knows this run's ordinal in its
        // scope.
        sel: Option<((usize, usize), Color)>,
    ) {
        let FrameText {
            cache_key: key,
            color,
        } = self.frame[id.0 as usize];
        let scale = self.scale;
        let ox = crate::geom::snap_px(origin.x * scale);
        let oy = crate::geom::snap_px(origin.y * scale);
        // Where layout put the text, less where it is drawn: what a
        // background's edges are snapped from (`on_pixels`).
        let nudge = Vec2::new(origin.x * scale - ox, origin.y * scale - oy);
        // A long line is drawn by chunks, off the run's path (backlog
        // C19).
        if self.long(key).is_some() {
            self.emit_long(
                key, node, ox, oy, nudge, color, clip, clip_id, clips, res, fs, atlas, out, sel,
            );
            return;
        }
        self.ensure_wrap(id, node.w, fs);
        let raster = &mut self.raster;
        let entry = self
            .entries
            .get_mut(&key)
            .and_then(Entry::run_mut)
            .expect("frame text missing from cache");

        // Overflowing modes own their box: a line that runs past the node's
        // width is clipped there rather than painted over siblings.
        let (clip, clip_id) = if entry.clamp_w {
            let own = Rect::new(ox, oy, (node.w * scale).ceil(), (node.h * scale).ceil());
            let clip = clip.intersect(own, crate::display::SQUARE);
            (clip, crate::display::intern_clip(clips, clip))
        } else {
            (clip, clip_id)
        };
        if let Some(((from, to), tint)) = sel {
            let rects = Self::run_highlight(entry, from, to);
            push_highlight(&rects, ox, oy, nudge, tint, clip_id, out);
        }
        emit_entry(
            entry,
            ox,
            oy,
            nudge,
            color,
            clip,
            clip_id,
            raster,
            fs,
            atlas,
            out,
            &mut self.joins,
        );
    }
}

/// A text's background rect at physical (`x`, `y`) with each edge
/// snapped to a whole pixel, so it meets another text's — the next run's,
/// the next row's — on a pixel line and not inside one, where each drew
/// part of the pixel and the two left a seam. A box is drawn where layout
/// put it, so a gap between two boxes is there at any scale, unless it
/// asks for this too (`pixelSnap`), from its layout rect.
///
/// The edges are snapped from where layout put them, `nudge` (layout's
/// origin less the drawn, snapped one) past `x` and `y`, so the next
/// text's edge, snapped from its own layout origin, lands on the same
/// pixel. From the drawn origin a row's rect ended a pixel into the next
/// row wherever their pitch was not whole pixels, and the join drew
/// twice.
fn on_pixels(x: f32, y: f32, w: f32, h: f32, nudge: Vec2) -> Rect {
    Rect::new(x + nudge.x, y + nudge.y, w, h).on_pixels()
}

/// Pushes selection rects as quads at a run's origin — before its glyphs,
/// so the text stays on top of its own highlight.
fn push_highlight(
    rects: &[Rect],
    ox: f32,
    oy: f32,
    nudge: Vec2,
    tint: Color,
    clip: ClipId,
    out: &mut Vec<Quad>,
) {
    for r in rects {
        out.push(Quad {
            rect: on_pixels(ox + r.x, oy + r.y, r.w, r.h, nudge),
            color: tint,
            border_color: Color::TRANSPARENT,
            radius: [0.0; 4],
            border_w: 0.0,
            blur: 0.0,
            kind: QuadKind::Solid,
            uv: [0; 4],
            clip,
        });
    }
}

impl TextSystem {
    /// `emit` for a wrapped long line: the chunks whose rows touch the
    /// clip, plus one either side, each drawn row by row from its
    /// unwrapped templates.
    #[allow(clippy::too_many_arguments)]
    fn emit_long_rows(
        &mut self,
        key: u64,
        w: f32,
        ox: f32,
        oy: f32,
        nudge: Vec2,
        color: Color,
        clip: Clip,
        clip_id: ClipId,
        res: &Resources,
        fs: &mut FontSystem,
        atlas: &mut GlyphAtlas,
        out: &mut Vec<Quad>,
    ) {
        let (first, last) = {
            let line = self.long(key).expect("a long line");
            let ra = ((clip.rect.y - oy) / line.line_h).floor().max(0.0) as u32;
            let rb = ((clip.rect.y + clip.rect.h - oy) / line.line_h)
                .floor()
                .max(0.0) as u32;
            let Some((a, b)) = line.chunks_on_rows(ra, rb) else {
                return;
            };
            (a.saturating_sub(1), (b + 1).min(line.chunks.len() - 1))
        };
        let mut fresh = false;
        for i in first..=last {
            fresh |= self.ensure_chunk(key, i, res, fs);
        }
        if fresh {
            // A chunk shaped now moves every row after it — and the box
            // layout gave the line, which was the estimate's.
            self.relayout_long(key, Some(w));
            let line = self.long_mut(key).expect("checked");
            let rows = line.rows();
            if rows != line.laid_rows && rows != line.asked_rows {
                line.asked_rows = rows;
                self.owed = true;
            }
        }
        let frame_no = self.frame_no;
        self.long_mut(key).expect("checked").last_used = frame_no;
        for i in first..=last {
            let chunk_key = self.long(key).expect("checked").chunks[i].key;
            // The line and the chunk's run are two entries of one map, so
            // the pair is borrowed together; a chunk's key is never the
            // line's own (`LONG_SALT`).
            let [Some(Entry::Long(line)), Some(Entry::Run(entry))] =
                self.entries.get_disjoint_mut([&key, &chunk_key])
            else {
                continue;
            };
            let (row0, head_x) = line.starts[i];
            let chunk = &line.chunks[i];
            if chunk.rows.is_empty() {
                continue;
            }
            let raster = &mut self.raster;
            emit_entry_rows(
                entry,
                ox,
                oy + row0 as f32 * line.line_h,
                nudge,
                head_x,
                &chunk.rows,
                line.line_h,
                color,
                clip,
                clip_id,
                raster,
                fs,
                atlas,
                out,
                &mut self.joins,
            );
        }
    }
}

/// `emit_entry` for one chunk of a wrapped long line: the templates are
/// the chunk's unwrapped run, and each row's glyphs are shifted back by
/// where the row starts in it and down by the row. `oy` is the chunk's
/// first row; `head_x` where that row begins.
#[allow(clippy::too_many_arguments)]
fn emit_entry_rows(
    entry: &mut CachedText,
    ox: f32,
    oy: f32,
    nudge: Vec2,
    head_x: f32,
    rows: &[RowStart],
    line_h: f32,
    color: Color,
    clip: Clip,
    clip_id: ClipId,
    raster: &mut Raster,
    fs: &mut FontSystem,
    atlas: &mut GlyphAtlas,
    out: &mut Vec<Quad>,
    joins: &mut Vec<JoinBg>,
) {
    build_templates(entry, raster, fs, atlas);
    // A decoration rect spans glyphs of the unwrapped run; the row a
    // glyph is on is decided by its byte, and a rect by its x — row `r`
    // covers `rows[r].x..rows[r + 1].x` of the run — so a rect a row
    // break falls inside is cut there, one piece a row, each shifted as
    // the row's glyphs are (backlog C42: a rich chunk's spans).
    let mut row_pieces = |d: &DecoTemplate, out: &mut Vec<Quad>| {
        let (x0, x1) = (d.x, d.x + d.w);
        for r in 0..rows.len() {
            let ra = rows[r].x;
            let rb = rows.get(r + 1).map_or(f32::INFINITY, |n| n.x);
            let (a, b) = (x0.max(ra), x1.min(rb));
            if b <= a {
                continue;
            }
            let dx = if r == 0 { head_x } else { 0.0 } - ra;
            let x = ox + a + dx;
            let y = oy + d.y + r as f32 * line_h;
            if y >= clip.rect.y + clip.rect.h {
                break;
            }
            if y + d.h <= clip.rect.y
                || x >= clip.rect.x + clip.rect.w
                || x + (b - a) <= clip.rect.x
            {
                continue;
            }
            let quad = Quad {
                rect: if d.under {
                    on_pixels(x, y, b - a, d.h, nudge)
                } else {
                    Rect::new(x, y, b - a, d.h)
                },
                color: d.color.unwrap_or(color),
                border_color: Color::TRANSPARENT,
                radius: [0.0; 4],
                border_w: 0.0,
                blur: 0.0,
                kind: QuadKind::Solid,
                clip: clip_id,
                uv: [0; 4],
            };
            // A background or a solid line is its rect; a wave or dots
            // are pieces built around it, as in `emit_entry`. A rounded
            // background is noted for the join pass (backlog F101).
            if d.under && d.radius > 0.0 {
                joins.push(JoinBg {
                    quad: out.len() as u32,
                    radius: d.radius,
                    outer: clip_id,
                    own: None,
                });
            }
            if d.under || d.style == UnderlineStyle::Solid {
                out.push(quad);
            } else {
                crate::deco::push_line(
                    out,
                    d.style,
                    x,
                    y,
                    b - a,
                    d.h,
                    d.color.unwrap_or(color),
                    clip_id,
                );
            }
        }
    };
    for d in entry.deco.iter().filter(|d| d.under) {
        row_pieces(d, out);
    }
    let mut r = 0usize;
    for g in &entry.glyphs {
        while r + 1 < rows.len() && g.byte >= rows[r + 1].byte {
            r += 1;
        }
        let dx = if r == 0 { head_x } else { 0.0 } - rows[r].x;
        let x = ox + g.x + dx;
        let y = oy + g.y + r as f32 * line_h;
        // Rows only go down: past the clip's bottom nothing comes back.
        // Both edges are inclusive, as the x test's are: a glyph whose ink
        // ends exactly at the clip's top or starts exactly at its bottom
        // has no pixel inside it, and which glyph that is depends on the
        // face the machine resolved `Mono` to (the alpha.12 Windows round
        // saw two, after C32 moved the face).
        if y >= clip.rect.y + clip.rect.h {
            break;
        }
        if y + g.h <= clip.rect.y || x >= clip.rect.x + clip.rect.w || x + g.w <= clip.rect.x {
            continue;
        }
        out.push(Quad {
            rect: Rect::new(x, y, g.w, g.h),
            color: g.color.unwrap_or(color),
            border_color: Color::TRANSPARENT,
            radius: [0.0; 4],
            border_w: 0.0,
            blur: 0.0,
            kind: g.kind,
            clip: clip_id,
            uv: g.uv,
        });
    }
    for d in entry.deco.iter().filter(|d| !d.under) {
        row_pieces(d, out);
    }
}

/// The rows a chunk's unwrapped run breaks into at width `w` when its
/// first row starts `head_x` in: greedy, at the break opportunities UAX
/// #14 gives (what cosmic-text's `WordOrGlyph` takes) or at every glyph
/// for `Glyph`, a piece wider than a row breaking by glyph, trailing
/// whitespace hanging past the edge as cosmic-text lets it — except under
/// `BreakSpaces`, where each whitespace character is a piece of its own
/// and takes its room like ink, so one that does not fit starts the next
/// row and none hangs (CSS's `break-spaces`, a break before the run too).
/// Returns the
/// row starts — the first is the chunk's own — and the x its last row
/// ends at. Positions are read left to right: a bidi run breaks by its
/// glyph order.
fn break_rows(
    buffer: &Buffer,
    text: &str,
    wrap: TextWrap,
    w: f32,
    head_x: f32,
) -> (Vec<RowStart>, f32) {
    let mut rows = vec![RowStart { byte: 0, x: 0.0 }];
    let Some(run) = buffer.layout_runs().next() else {
        return (rows, head_x);
    };
    let glyphs = run.glyphs;
    let blank = |g: &cosmic_text::LayoutGlyph| {
        text.get(g.start..g.end)
            .is_some_and(|s| s.chars().all(char::is_whitespace))
    };
    let spaces = wrap == TextWrap::BreakSpaces;
    let mut pieces: Vec<(usize, usize)> = Vec::new();
    match wrap {
        TextWrap::Word | TextWrap::BreakSpaces => {
            let mut at = 0usize;
            let mut piece = |from: usize, to: usize| {
                if !spaces {
                    pieces.push((from, to));
                    return;
                }
                // The trailing whitespace a break follows, a piece a
                // character: the break is before each of them as well.
                let ink = text[from..to].trim_end_matches(char::is_whitespace).len();
                if ink > 0 {
                    pieces.push((from, from + ink));
                }
                for (i, c) in text[from + ink..to].char_indices() {
                    let at = from + ink + i;
                    pieces.push((at, at + c.len_utf8()));
                }
            };
            for (i, _) in unicode_linebreak::linebreaks(text) {
                if i > at {
                    piece(at, i);
                    at = i;
                }
            }
            if at < text.len() {
                piece(at, text.len());
            }
        }
        TextWrap::Glyph | TextWrap::None => {
            pieces.extend(glyphs.iter().map(|g| (g.start, g.end)));
        }
    }
    let mut row_x0 = 0.0f32;
    let mut avail = w - head_x;
    let mut gi = 0usize;
    for (s, e) in pieces {
        let g0 = gi;
        while gi < glyphs.len() && glyphs[gi].start < e {
            gi += 1;
        }
        if g0 == gi {
            continue;
        }
        let piece = &glyphs[g0..gi];
        let x0 = piece[0].x;
        let ink_end = piece
            .iter()
            .rev()
            .find(|g| spaces || !blank(g))
            .map_or(x0, |g| g.x + g.w);
        // A piece at the row's start has nowhere to go — except the
        // chunk's first, whose row is the one the previous chunk left off
        // on `head_x` in: that row holds something, so the piece can open
        // the next (the alpha.22 regression pass; the chunk's first row
        // is then empty, which the rest reads by byte).
        let opens = x0 > row_x0 || (rows.len() == 1 && head_x > 0.0);
        if ink_end - row_x0 > avail && opens {
            rows.push(RowStart {
                byte: s as u32,
                x: x0,
            });
            row_x0 = x0;
            avail = w;
        }
        if ink_end - row_x0 > avail {
            // Wider than a row on its own: by glyph.
            for g in piece {
                if g.x + g.w - row_x0 > avail && g.x > row_x0 && (spaces || !blank(g)) {
                    rows.push(RowStart {
                        byte: g.start as u32,
                        x: g.x,
                    });
                    row_x0 = g.x;
                    avail = w;
                }
            }
        }
    }
    let end = if rows.len() == 1 {
        head_x + run.line_w
    } else {
        run.line_w - row_x0
    };
    (rows, end)
}

/// Builds the entry's positioned templates if the wrap or the atlas moved
/// since they were: the steady state reuses them.
fn build_templates(
    entry: &mut CachedText,
    raster: &mut Raster,
    fs: &mut FontSystem,
    atlas: &mut GlyphAtlas,
) {
    {
        // Steady state: same wrap, same atlas — reuse positioned templates.
        let built_for = (entry.wrap.map(f32::to_bits), atlas.stamp);
        if entry.glyphs_built_for != Some(built_for) {
            entry.glyphs.clear();
            entry.deco.clear();
            let lines = line_cap(entry.max_lines);
            let decorated = entry.span_deco.iter().any(SpanDeco::any);
            for run in entry.buffer.layout_runs().take(lines) {
                if decorated {
                    build_decorations(&run, &entry.span_deco, fs, &mut entry.deco);
                }
                for glyph in run.glyphs.iter() {
                    let physical = glyph.physical((0.0, 0.0), 1.0);
                    let Some(slot) = raster_glyph(physical.cache_key, fs, raster, atlas) else {
                        continue;
                    };
                    entry.glyphs.push(GlyphTemplate {
                        x: physical.x as f32 + slot.left as f32,
                        y: run.line_y.round() + physical.y as f32 - slot.top as f32,
                        w: slot.w as f32,
                        h: slot.h as f32,
                        uv: [slot.x, slot.y, slot.w, slot.h],
                        kind: glyph_kind(&slot),
                        color: glyph
                            .color_opt
                            .map(|c| Color::rgba8(c.r(), c.g(), c.b(), c.a())),
                        byte: glyph.start as u32,
                    });
                }
            }
            // Rasterizing may have extended the page mid-build, which
            // keeps every slot where it was but moves the stamp: keep the
            // one we ended on.
            entry.glyphs_built_for = Some((entry.wrap.map(f32::to_bits), atlas.stamp));
        }
    }
}

/// Emits one cache entry's glyphs and decorations at the physical origin
/// (`ox`, `oy`): the steady-state template walk, shared by a text node and
/// by each chunk of a long line.
#[allow(clippy::too_many_arguments)]
fn emit_entry(
    entry: &mut CachedText,
    ox: f32,
    oy: f32,
    nudge: Vec2,
    color: Color,
    clip: Clip,
    clip_id: ClipId,
    raster: &mut Raster,
    fs: &mut FontSystem,
    atlas: &mut GlyphAtlas,
    out: &mut Vec<Quad>,
    joins: &mut Vec<JoinBg>,
) {
    build_templates(entry, raster, fs, atlas);
    {
        let inside = |x: f32, y: f32, w: f32, h: f32| {
            oy + y + h >= clip.rect.y
                && oy + y <= clip.rect.y + clip.rect.h
                && ox + x < clip.rect.x + clip.rect.w
                && ox + x + w > clip.rect.x
        };
        let deco_quad = |d: &DecoTemplate| Quad {
            rect: Rect::new(ox + d.x, oy + d.y, d.w, d.h),
            color: d.color.unwrap_or(color),
            border_color: Color::TRANSPARENT,
            radius: [0.0; 4],
            border_w: 0.0,
            blur: 0.0,
            kind: QuadKind::Solid,
            clip: clip_id,
            uv: [0; 4],
        };
        // A span's background goes under its glyphs, on whole pixels;
        // its lines go over. A rounded one is noted for the join pass,
        // which gives it its corners once every text is painted (backlog
        // F101).
        for d in entry
            .deco
            .iter()
            .filter(|d| d.under && inside(d.x, d.y, d.w, d.h))
        {
            if d.radius > 0.0 {
                joins.push(JoinBg {
                    quad: out.len() as u32,
                    radius: d.radius,
                    outer: clip_id,
                    own: None,
                });
            }
            out.push(Quad {
                rect: on_pixels(ox + d.x, oy + d.y, d.w, d.h, nudge),
                ..deco_quad(d)
            });
        }

        // Glyph templates are in layout order; skip everything above the clip
        // and stop at the first glyph past it (rows below never come back).
        // Horizontally clipped glyphs (an unwrapped line) are dropped too.
        out.extend(
            entry
                .glyphs
                .iter()
                .filter(|g| {
                    oy + g.y + g.h >= clip.rect.y
                        && ox + g.x < clip.rect.x + clip.rect.w
                        && ox + g.x + g.w > clip.rect.x
                })
                .take_while(|g| oy + g.y <= clip.rect.y + clip.rect.h)
                .map(|g| Quad {
                    rect: Rect::new(ox + g.x, oy + g.y, g.w, g.h),
                    color: g.color.unwrap_or(color),
                    border_color: Color::TRANSPARENT,
                    radius: [0.0; 4],
                    border_w: 0.0,
                    blur: 0.0,
                    kind: g.kind,
                    clip: clip_id,
                    uv: g.uv,
                }),
        );
        for d in entry
            .deco
            .iter()
            .filter(|d| !d.under && inside(d.x, d.y, d.w, d.h))
        {
            // A solid line is its rect; a wave or dots are pieces built
            // around it (`crate::deco`, backlog K4).
            match d.style {
                UnderlineStyle::Solid => out.push(deco_quad(d)),
                style => crate::deco::push_line(
                    out,
                    style,
                    ox + d.x,
                    oy + d.y,
                    d.w,
                    d.h,
                    d.color.unwrap_or(color),
                    clip_id,
                ),
            }
        }
    }
}

/// Whether a text of `len` bytes in `style` is shaped in chunks, by what
/// costs nothing to ask: past `LONG_LINE_BYTES`, with no `max_lines` or
/// `ellipsis`, since a line budget is a property of the whole. The
/// other half is [`has_line_break`] — and [`has_rtl`] for a text long
/// only by its `break-spaces` — asked only when the cache has not
/// seen the text — a wrapped one breaks its chunks into rows
/// ([`LongLine::starts`]), whatever its `wrap`. Plain or rich alike.
fn could_be_long(len: usize, style: &TextStyle) -> bool {
    (len >= LONG_LINE_BYTES || len > 0 && style.wrap == TextWrap::BreakSpaces)
        && style.max_lines == 0
        && !style.ellipsis
}

/// Whether the content breaks lines of its own — a byte scan, since the
/// `str` pattern search is per character.
fn has_line_break(content: &str) -> bool {
    content.bytes().any(|b| b == b'\n' || b == b'\r')
}

/// Whether the content holds a right-to-left character — a strong one of
/// the right-to-left scripts' blocks, or a mark or embedding asking for
/// that direction. A text long only by its `break-spaces` and holding one
/// is shaped as the run it would otherwise be, wrapping as `word` does
/// (as one with line breaks of its own does): the long line breaks rows
/// over glyph positions left to right, which a right-to-left run's are
/// not, and a Hebrew paragraph that had to wrap would draw one glyph.
fn has_rtl(content: &str) -> bool {
    !content.is_ascii()
        && content.chars().any(|c| {
            matches!(c as u32,
                0x0590..=0x08FF
                | 0x200F | 0x202B | 0x202E | 0x2067
                | 0xFB1D..=0xFDFF
                | 0xFE70..=0xFEFF
                | 0x10800..=0x10FFF
                | 0x1E800..=0x1EFFF)
        })
}

/// The decoration rects for one laid-out line: consecutive glyphs of one
/// span (its index is their metadata) become one background rect, one
/// underline and one strikethrough, as the span asked. Where the lines go
/// is the face's own recommendation — swash's `underline_offset`,
/// `strikeout_offset` and `stroke_size`, scaled to the glyph's size — read
/// from the run's first glyph, so a fallback glyph in the middle of a
/// span does not move the line. Neighbouring spans of one background are
/// one rect: an editor's selection is its syntax runs, a span each, and
/// a rect each drew a seam between every two of them wherever the join
/// fell inside a pixel.
fn build_decorations(
    run: &cosmic_text::LayoutRun<'_>,
    spans: &[SpanDeco],
    fs: &mut FontSystem,
    out: &mut Vec<DecoTemplate>,
) {
    // The background rect the last group drew, which the next one of the
    // same colour starting where it ends extends.
    let mut last_bg: Option<usize> = None;
    let mut i = 0;
    while i < run.glyphs.len() {
        let span_no = run.glyphs[i].metadata;
        let mut j = i + 1;
        while j < run.glyphs.len() && run.glyphs[j].metadata == span_no {
            j += 1;
        }
        let deco = spans.get(span_no).copied().unwrap_or_default();
        if deco.any() {
            let group = &run.glyphs[i..j];
            let x0 = group.iter().map(|g| g.x).fold(f32::INFINITY, f32::min);
            let x1 = group
                .iter()
                .map(|g| g.x + g.w)
                .fold(f32::NEG_INFINITY, f32::max);
            let w = (x1 - x0).max(0.0);
            match (deco.bg, last_bg.map(|k| &mut out[k])) {
                (Some(bg), Some(prev))
                    if prev.color == Some(bg)
                        && prev.radius == deco.bg_radius
                        && (prev.x + prev.w - x0).abs() < 0.01 =>
                {
                    prev.w = (x1 - prev.x).max(prev.w);
                }
                (Some(bg), _) => {
                    // A span shorter than its line (`Span::size`, a line
                    // a larger span made taller) has a background its own
                    // height, around its glyphs, not the line's.
                    let (y, h) = match group[0].line_height_opt {
                        Some(lh) if lh < run.line_height - 0.01 => {
                            let g = &group[0];
                            let (asc, desc) = fs
                                .get_font(g.font_id, g.font_weight)
                                .map(|font| {
                                    let m = font.as_swash().metrics(&[]).scale(g.font_size);
                                    (m.ascent, m.descent)
                                })
                                .unwrap_or((0.8 * g.font_size, 0.2 * g.font_size));
                            let mid = run.line_y - (asc - desc) / 2.0;
                            (mid - lh / 2.0, lh)
                        }
                        _ => (run.line_top, run.line_height),
                    };
                    last_bg = Some(out.len());
                    out.push(DecoTemplate {
                        x: x0,
                        y,
                        w,
                        h,
                        color: Some(bg),
                        under: true,
                        style: UnderlineStyle::Solid,
                        radius: deco.bg_radius,
                    });
                }
                (None, _) => last_bg = None,
            }
            if deco.underline || deco.strikethrough {
                let first = &group[0];
                let metrics = fs
                    .get_font(first.font_id, first.font_weight)
                    .map(|font| font.as_swash().metrics(&[]).scale(first.font_size));
                // Without the face (it was unloaded between frames), a
                // line under the descent and one through the x-height.
                let (under_off, strike_off, stroke) = match metrics {
                    Some(m) => (m.underline_offset, m.strikeout_offset, m.stroke_size),
                    None => (
                        -0.1 * first.font_size,
                        0.3 * first.font_size,
                        0.06 * first.font_size,
                    ),
                };
                let stroke = stroke.max(1.0).round();
                let baseline = run.line_y.round();
                if deco.underline {
                    out.push(DecoTemplate {
                        x: x0,
                        y: (baseline - under_off).round(),
                        w,
                        h: stroke,
                        color: deco.underline_color.or_else(|| {
                            first
                                .color_opt
                                .map(|c| Color::rgba8(c.r(), c.g(), c.b(), c.a()))
                        }),
                        under: false,
                        style: deco.underline_style,
                        radius: 0.0,
                    });
                }
                if deco.strikethrough {
                    out.push(DecoTemplate {
                        x: x0,
                        y: (baseline - strike_off).round(),
                        w,
                        h: stroke,
                        color: first
                            .color_opt
                            .map(|c| Color::rgba8(c.r(), c.g(), c.b(), c.a())),
                        under: false,
                        style: UnderlineStyle::Solid,
                        radius: 0.0,
                    });
                }
            }
        } else {
            last_bg = None;
        }
        i = j;
    }
}

impl TextSystem {
    /// Records where a live text node was drawn, for the queries below,
    /// with the keys above it so a query by a `line` row or a wrapper
    /// finds the runs inside. Called beside [`Self::emit`] for the frame's
    /// own nodes and not for ghosts, which take no input.
    pub(crate) fn place(
        &mut self,
        key: Key,
        ancestry: &Ancestry,
        id: TextId,
        origin: Vec2,
        scope: Option<Key>,
        drawn: bool,
    ) {
        let cache_key = self.frame[id.0 as usize].cache_key;
        self.places.push(TextPlace {
            key,
            ancestors: ancestry.keys,
            depth: ancestry.depth.min(PLACE_ANCESTORS) as u8,
            none_at: ancestry.none_at.map_or(u8::MAX, |n| n as u8),
            cache_key,
            origin,
            scope,
            drawn,
        });
    }

    /// The runs `key` names, in tree order, each with its entry and the
    /// byte offset its content starts at in the concatenation — from the
    /// frame that finished (`prev`) or the one being emitted. A long line
    /// is a run among them: every `break-spaces` text is one, and a `line`
    /// row of several would otherwise answer with the last one's bytes alone.
    fn runs_of(&self, key: Key, prev: bool) -> Vec<(&TextPlace, &Entry, usize)> {
        let list = if prev {
            self.places.prev()
        } else {
            &self.places
        };
        let mut base = 0usize;
        let mut out = Vec::new();
        for place in list.iter().filter(|p| p.drawn && p.answers_to(key)) {
            let Some(entry) = self.entries.get(&place.cache_key) else {
                continue;
            };
            out.push((place, entry, base));
            base += entry.content().len();
        }
        out
    }

    /// How many bytes of content this frame's text `id` holds — what a
    /// selection range over it is measured against.
    pub(crate) fn content_len(&self, id: TextId) -> usize {
        let key = self.frame[id.0 as usize].cache_key;
        self.entries.get(&key).map_or(0, |e| e.content().len())
    }

    /// Every run inside the selection scope `scope`, in emission order —
    /// which is tree order, which is reading order — each with the byte
    /// offset its content starts at in the scope's concatenation.
    ///
    /// Undrawn runs are included: a scoped run the frame culled recorded
    /// a place precisely so a selection can reach past the viewport, and
    /// leaving it out here would renumber everything after it the moment
    /// it scrolled away.
    pub(crate) fn scope_runs(&self, scope: Key, prev: bool) -> Vec<ScopeRun<'_>> {
        let list = if prev {
            self.places.prev()
        } else {
            &self.places
        };
        let mut base = 0usize;
        let mut out = Vec::new();
        for place in list.iter().filter(|p| p.scope == Some(scope)) {
            let Some(text) = self.entries.get(&place.cache_key) else {
                continue;
            };
            let len = text.content().len();
            out.push(ScopeRun { place, text, base });
            base += len;
        }
        out
    }

    /// Where `point` (logical viewport px) lands in a selection scope, as
    /// the address a selection endpoint is made of: the node whose run it
    /// landed in and the byte offset inside *that node's* text. Nearest
    /// drawn run wins, vertically first, exactly as [`Self::hit_at`]
    /// resolves a point inside one node — an undrawn run is not under any
    /// pointer, so it is not a candidate.
    pub(crate) fn scope_hit(&self, scope: Key, point: Vec2, prev: bool) -> Option<(Key, usize)> {
        let runs = self.scope_runs(scope, prev);
        let px = point.x * self.scale;
        let py = point.y * self.scale;
        let gap = |r: &Rect| {
            let dy = (r.y - py).max(py - (r.y + r.h)).max(0.0);
            let dx = (r.x - px).max(px - (r.x + r.w)).max(0.0);
            (dy, dx)
        };
        let run = runs.iter().filter(|r| r.place.drawn).min_by(|a, b| {
            let ga = gap(&self.scope_box(a));
            let gb = gap(&self.scope_box(b));
            ga.partial_cmp(&gb).unwrap_or(std::cmp::Ordering::Equal)
        })?;
        let byte = self.hit_in_run(run, point)?;
        Some((run.place.key, byte))
    }

    /// A scoped run's laid-out box, physical px in viewport space.
    fn scope_box(&self, run: &ScopeRun<'_>) -> Rect {
        self.entry_box(run.place, run.text)
    }

    /// A run's laid-out box, physical px in viewport space, whichever way
    /// it was shaped. A wrapped long line is as wide as its rows were
    /// broken to — its unwrapped width would claim the text beside it in a
    /// selection scope.
    fn entry_box(&self, place: &TextPlace, entry: &Entry) -> Rect {
        match entry {
            Entry::Run(e) => self.physical_box(place, e),
            Entry::Long(l) => {
                let (ox, oy) = self.physical_origin(place);
                let w = l.wrap_w.unwrap_or_else(|| l.width());
                Rect::new(ox, oy, w, l.line_h * l.rows() as f32)
            }
        }
    }

    /// The tops of a run's visual rows, physical px in viewport space.
    fn row_tops(&self, place: &TextPlace, entry: &Entry) -> Vec<f32> {
        let (_, oy) = self.physical_origin(place);
        match entry {
            Entry::Run(e) => e.buffer.layout_runs().map(|r| oy + r.line_top).collect(),
            Entry::Long(l) => (0..l.rows()).map(|r| oy + r as f32 * l.line_h).collect(),
        }
    }

    /// Where `point` lands inside one scoped run, as a byte offset into
    /// that run's own content.
    fn hit_in_run(&self, run: &ScopeRun<'_>, point: Vec2) -> Option<usize> {
        match run.text {
            Entry::Long(l) => Some(self.long_hit(run.place, l, point)?.byte),
            Entry::Run(e) => {
                let (ox, oy) = self.physical_origin(run.place);
                let cursor = e
                    .buffer
                    .hit(point.x * self.scale - ox, point.y * self.scale - oy)?;
                Some(e.byte_of(cursor))
            }
        }
    }

    /// The word around `byte` in one node's own text, as a byte range in
    /// that node — what a double click (and a force click) selects.
    /// Words are runs of alphanumerics and underscores; anything else is
    /// selected as the run of like characters around it, so a double
    /// click in whitespace takes the whitespace and one in `->` takes
    /// both arrows. `None` when the node holds no text at all.
    pub(crate) fn word_at(
        &self,
        scope: Key,
        node: Key,
        byte: usize,
        prev: bool,
    ) -> Option<(usize, usize)> {
        let run = self
            .scope_runs(scope, prev)
            .into_iter()
            .find(|r| r.place.key == node)?;
        let content = run.text.content();
        Some(word_range(content, byte))
    }

    /// A selection endpoint's address — the node and a byte inside its
    /// text — as an offset in the scope's concatenation, which is what
    /// orders two endpoints against each other. `None` when that node is
    /// not (or no longer) in the scope: an address the frame cannot
    /// resolve resolves to nothing rather than to whatever took its place.
    pub(crate) fn scope_offset(
        &self,
        scope: Key,
        node: Key,
        byte: usize,
        prev: bool,
    ) -> Option<usize> {
        let run = self
            .scope_runs(scope, prev)
            .into_iter()
            .find(|r| r.place.key == node)?;
        Some(run.base + byte.min(run.text.content().len()))
    }

    /// The selection `from..to` as HTML, carrying the styling that is
    /// *the text's* rather than the theme's: bold, italic, and a span's
    /// own colour where one was declared.
    ///
    /// What it deliberately leaves behind is the node's colour. A
    /// paragraph drawn light grey on a dark card is grey because of the
    /// app's theme, not because the words are grey; pasting it into a
    /// white document as grey-on-white is how "copy with formatting"
    /// earns its reputation. A `rich_text` span that declared a colour is
    /// the other case — that colour is authored, the way a highlighted
    /// keyword is — and it travels.
    ///
    /// Runs are joined the way [`Self::scope_slice`] joins them: a `<br>`
    /// where the plain text gets a newline.
    pub(crate) fn scope_html(&self, scope: Key, from: usize, to: usize, prev: bool) -> String {
        let (from, to) = (from.min(to), from.max(to));
        let mut out = String::new();
        let mut prev_run: Option<&ScopeRun<'_>> = None;
        for run in &self.scope_runs(scope, prev) {
            let (start, end) = run.span();
            if end <= from || start >= to {
                continue;
            }
            if let Some(p) = prev_run
                && (p.place.origin.y - run.place.origin.y).abs() > f32::EPSILON
            {
                out.push_str("<br>");
            }
            let content = run.text.content();
            let lo = floor_boundary(content, from.saturating_sub(start));
            let hi = floor_boundary(content, (to - start).min(content.len()));
            match run.text {
                Entry::Long(line) => self.long_html(line, lo, hi, &mut out),
                Entry::Run(entry) => html_of_run(entry, lo, hi, &mut out),
            }
            prev_run = Some(run);
        }
        out
    }

    /// `html_of_run` for `lo..hi` of a long line: each chunk the range
    /// touches through its own shaped run, so a rich line's spans copy as
    /// a short rich text's do — every `break-spaces` text is a long line. A chunk that never
    /// showed, or whose run the cache dropped, is plain escaped text: its
    /// spans are the line's, but shaping it is not a copy's to do.
    fn long_html(&self, line: &LongLine, lo: usize, hi: usize, out: &mut String) {
        for c in &line.chunks {
            let (a, b) = (lo.max(c.start), hi.min(c.end));
            if a >= b {
                continue;
            }
            match c.width.and_then(|_| self.run(c.key)) {
                Some(e) => html_of_run(e, a - c.start, b - c.start, out),
                None => escape_into(&line.content[a..b], out),
            }
        }
    }

    /// Where a platform panel about the selection `from..to` should point:
    /// the **baseline origin of its first line**, logical viewport px.
    ///
    /// Not the box's corner, and not the union's bottom. macOS's
    /// `showDefinitionForAttributedString:atPoint:` takes the point the
    /// string's own baseline starts at and draws the term back over the
    /// text there — so a box's bottom puts the term a line low, and a
    /// multi-run selection's union puts it at the bottom of the last line
    /// while the panel shows the first.
    pub(crate) fn scope_selection_anchor(
        &self,
        scope: Key,
        from: usize,
        to: usize,
        prev: bool,
    ) -> Option<Vec2> {
        let (from, to) = (from.min(to), from.max(to));
        for run in self.scope_runs(scope, prev) {
            let (start, end) = run.span();
            if !run.place.drawn || end <= from || start >= to {
                continue;
            }
            let content = run.text.content();
            let lo = floor_boundary(content, from.saturating_sub(start));
            let hi = floor_boundary(content, (to - start).min(content.len()));
            let (ox, oy) = self.physical_origin(run.place);
            match run.text {
                Entry::Run(e) => {
                    if let Some((x, base)) = Self::run_anchor(e, lo, hi) {
                        return Some(Vec2::new((ox + x) / self.scale, (oy + base) / self.scale));
                    }
                }
                Entry::Long(l) => {
                    if let Some((x, y)) = self.long_caret_local(l, lo) {
                        return Some(Vec2::new(
                            (ox + x) / self.scale,
                            (oy + y + self.long_baseline(l)) / self.scale,
                        ));
                    }
                }
            }
        }
        None
    }

    /// The x and baseline of the first row of `lo..hi` inside one run,
    /// physical px from the run's origin.
    fn run_anchor(entry: &CachedText, lo: usize, hi: usize) -> Option<(f32, f32)> {
        let (a, b) = (entry.cursor_of(lo), entry.cursor_of(hi));
        for run in entry.buffer.layout_runs() {
            if run.line_i < a.line || run.line_i > b.line {
                continue;
            }
            if let Some((x, _)) = run.highlight(a, b).next() {
                return Some((x, run.line_y));
            }
        }
        None
    }

    /// The box the selection `from..to` occupies in `scope`, logical
    /// viewport px: the union of the drawn runs it touches, clipped to
    /// the part of each run that is actually selected in x only where the
    /// run holds both ends. Rough on purpose — it anchors a platform
    /// panel, and a panel wants the block of text, not its outline.
    pub(crate) fn scope_selection_rect(
        &self,
        scope: Key,
        from: usize,
        to: usize,
        prev: bool,
    ) -> Option<Rect> {
        let (from, to) = (from.min(to), from.max(to));
        let mut out: Option<Rect> = None;
        for run in self.scope_runs(scope, prev) {
            let (start, end) = run.span();
            if !run.place.drawn || end <= from || start >= to {
                continue;
            }
            let content = run.text.content();
            let lo = floor_boundary(content, from.saturating_sub(start));
            let hi = floor_boundary(content, (to - start).min(content.len()));
            // The rects the *painter* would draw, not the run's box: a
            // paragraph that wraps is one run four rows tall, and its box
            // is four rows tall with it. Anchoring a panel to that puts it
            // under the whole paragraph instead of under the word, which
            // is what a reader sees as the popover pointing at nothing.
            let (ox, oy) = self.physical_origin(run.place);
            let rects = match run.text {
                Entry::Long(l) => self.long_highlight(l, lo, hi),
                Entry::Run(e) => Self::run_highlight(e, lo, hi),
            };
            for r in rects {
                let r = Rect::new(
                    (ox + r.x) / self.scale,
                    (oy + r.y) / self.scale,
                    r.w / self.scale,
                    r.h / self.scale,
                );
                out = Some(match out {
                    None => r,
                    Some(o) => o.union(&r),
                });
            }
        }
        out
    }

    /// The scope's text between two offsets in its concatenation, with a
    /// newline between two runs that were laid out on different lines and
    /// nothing between two that share one. The exact length is known before a byte
    /// is copied, so this reserves once and grows never.
    pub(crate) fn scope_slice(&self, scope: Key, from: usize, to: usize, prev: bool) -> String {
        let (from, to) = (from.min(to), from.max(to));
        let runs = self.scope_runs(scope, prev);
        let mut out = String::new();
        let mut reserved = false;
        let mut prev_run: Option<&ScopeRun<'_>> = None;
        for run in &runs {
            let (start, end) = run.span();
            if end <= from || start >= to {
                continue;
            }
            if !reserved {
                out.reserve(to - from + runs.len());
                reserved = true;
            }
            if let Some(p) = prev_run
                && (p.place.origin.y - run.place.origin.y).abs() > f32::EPSILON
            {
                out.push('\n');
            }
            let content = run.text.content();
            let lo = from.saturating_sub(start).min(content.len());
            let hi = (to - start).min(content.len());
            out.push_str(&content[floor_boundary(content, lo)..floor_boundary(content, hi)]);
            prev_run = Some(run);
        }
        out
    }

    /// `hit_at` for a long line: the chunk under the point answers through
    /// its shaped entry, an unshaped one (never on screen) by the mean
    /// advance.
    fn long_hit(&self, place: &TextPlace, line: &LongLine, point: Vec2) -> Option<TextHit> {
        let (ox, oy) = self.physical_origin(place);
        let px = point.x * self.scale - ox;
        let py = point.y * self.scale - oy;
        if line.chunks.is_empty() {
            return Some(TextHit { byte: 0, line: 0 });
        }
        if let Some(w) = line.wrap_w {
            return self.long_hit_wrapped(line, w, px, py);
        }
        if px >= line.width() {
            return Some(TextHit {
                byte: line.content.len(),
                line: 0,
            });
        }
        let i = line.chunk_at(px.max(0.0));
        let c = &line.chunks[i];
        let local = px - line.prefix[i];
        let byte = match self.run(c.key) {
            Some(_) if let Some(b) = line.placed_tab_hit(i, local) => b,
            Some(e) if c.width.is_some() => {
                let cursor = e.buffer.hit(local, py.clamp(0.0, line.line_h - 0.01))?;
                c.start + cursor.index.min(c.end - c.start)
            }
            _ => {
                let est = (local / line.avg.max(f32::EPSILON)).round() as usize;
                let mut b = (c.start + est).min(c.end);
                while !line.content.is_char_boundary(b) {
                    b -= 1;
                }
                b
            }
        };
        Some(TextHit { byte, line: 0 })
    }

    /// `long_hit` while the line is wrapped: the row under the point, the
    /// chunk on that row the point is over, and the chunk's unwrapped run
    /// asked at the x the row was shifted from.
    fn long_hit_wrapped(&self, line: &LongLine, w: f32, px: f32, py: f32) -> Option<TextHit> {
        let row = ((py / line.line_h).floor().max(0.0) as u32).min(line.rows() - 1);
        let Some((a, b)) = line.chunks_on_rows(row, row) else {
            return Some(TextHit {
                byte: line.content.len(),
                line: row,
            });
        };
        // The first chunk whose span on this row reaches past the point,
        // else the last one on it.
        let i = (a..=b)
            .find(|&j| {
                let (r1, end_x) = line.starts[j + 1];
                px < if r1 == row { end_x } else { w }
            })
            .unwrap_or(b);
        let c = &line.chunks[i];
        let (row0, head_x) = line.starts[i];
        let r = (row - row0) as usize;
        let byte = match self.run(c.key) {
            Some(e) if r < c.rows.len() => {
                let rs = c.rows[r];
                let local_x = px - if r == 0 { head_x } else { 0.0 } + rs.x;
                let lo = rs.byte as usize;
                let hi = c
                    .rows
                    .get(r + 1)
                    .map_or(c.end - c.start, |n| n.byte as usize);
                // A tab the line places, when it is on this row.
                let tab_here = (lo..hi).contains(&(c.end - c.start - 1));
                match line.placed_tab_hit(i, local_x) {
                    Some(b) if tab_here => b,
                    _ => {
                        let cursor = e.buffer.hit(local_x, line.line_h / 2.0)?;
                        c.start + cursor.index.clamp(lo, hi)
                    }
                }
            }
            _ => {
                // The inverse of `long_caret`'s estimate: rows back into
                // one linear run from the chunk's start.
                let linear = r as f32 * w + px - head_x;
                let est = (linear / line.avg.max(f32::EPSILON)).round() as usize;
                let mut b = (c.start + est).min(c.end);
                while !line.content.is_char_boundary(b) {
                    b -= 1;
                }
                b
            }
        };
        // The byte a row breaks at is the next row's first, and its caret
        // is drawn there: a point past this row's end answers the byte
        // before the row's last character, which is on this row, as a
        // `word` run's hanging space does (the alpha.22 regression pass).
        let next_row = self
            .long_caret_local(line, byte)
            .is_some_and(|(_, y)| y > (row as f32 + 0.5) * line.line_h);
        let byte = if next_row && byte > 0 {
            floor_boundary(&line.content, byte - 1)
        } else {
            byte
        };
        Some(TextHit { byte, line: row })
    }

    /// `caret_at` for a long line, exact in a shaped chunk and by the mean
    /// advance in one that never showed.
    fn long_caret(&self, place: &TextPlace, line: &LongLine, byte: usize) -> Option<Rect> {
        let (ox, oy) = self.physical_origin(place);
        let scale = self.scale;
        let (x, y) = self.long_caret_local(line, byte)?;
        Some(Rect::new(
            (ox + x) / scale,
            (oy + y) / scale,
            0.0,
            line.line_h / scale,
        ))
    }

    /// A long line's first baseline, physical px below its top: its first
    /// chunk's, shaped when the line was built, rounded as its glyphs are
    /// drawn at it — a fifth of the row above the row's foot only while
    /// that chunk is not shaped. On that estimate alone a `break-spaces`
    /// text in a baseline row would sit a pixel off a `word` one.
    fn long_baseline(&self, line: &LongLine) -> f32 {
        line.chunks
            .first()
            .and_then(|c| self.run(c.key))
            .and_then(|e| e.buffer.layout_runs().next())
            .map_or(line.line_h * 0.8, |r| r.line_y.round())
    }

    /// The caret for `byte`, in physical px from the long line's own
    /// origin: its x on its row, and that row's top. What `long_caret`
    /// places in the viewport and what a selection highlight measures
    /// between.
    fn long_caret_local(&self, line: &LongLine, byte: usize) -> Option<(f32, f32)> {
        let byte = byte.min(line.content.len());
        if let Some(w) = line.wrap_w
            && !line.chunks.is_empty()
        {
            let i = line.chunk_of_byte(byte);
            let c = &line.chunks[i];
            let local = byte - c.start;
            let (row0, head_x) = line.starts[i];
            let (r, x) = match self.run(c.key) {
                Some(e) if !c.rows.is_empty() => {
                    let r = c.rows.partition_point(|rs| rs.byte as usize <= local) - 1;
                    let run = e.buffer.layout_runs().next()?;
                    let cx = caret_x(&run, local.min(c.end - c.start)) + line.end_shift(i, local);
                    (r, cx - c.rows[r].x + if r == 0 { head_x } else { 0.0 })
                }
                _ => {
                    let linear = head_x + local as f32 * line.avg;
                    let r = (linear / w).floor();
                    (r as usize, linear - r * w)
                }
            };
            let y = (row0 as usize + r) as f32 * line.line_h;
            return Some((x, y));
        }
        let x = if line.chunks.is_empty() {
            0.0
        } else {
            let i = line.chunk_of_byte(byte);
            let c = &line.chunks[i];
            let local = byte - c.start;
            match self.run(c.key) {
                Some(e) if c.width.is_some() => {
                    let run = e.buffer.layout_runs().next()?;
                    let x = caret_x(&run, local.min(c.end - c.start));
                    line.prefix[i] + x + line.end_shift(i, local)
                }
                _ => line.prefix[i] + local as f32 * line.avg,
            }
        };
        Some((x, 0.0))
    }

    /// Selection rects for the byte range `from..to` of a long line, in
    /// physical px from the line's origin. One rect per row it covers:
    /// the first from the start caret to the row's end, whole rows
    /// between, and the last from the row's start to the end caret — the
    /// shape any selection over wrapped text has.
    fn long_highlight(&self, line: &LongLine, from: usize, to: usize) -> Vec<Rect> {
        let Some((x0, y0)) = self.long_caret_local(line, from) else {
            return Vec::new();
        };
        let Some((x1, y1)) = self.long_caret_local(line, to) else {
            return Vec::new();
        };
        let h = line.line_h;
        if (y1 - y0).abs() < f32::EPSILON {
            return vec![Rect::new(x0, y0, (x1 - x0).max(0.0), h)];
        }
        // Several rows: only a wrapped line has any, so it has a width.
        let w = line.wrap_w.unwrap_or_else(|| line.width());
        let mut out = vec![Rect::new(x0, y0, (w - x0).max(0.0), h)];
        let mut y = y0 + h;
        while y < y1 - h / 2.0 {
            out.push(Rect::new(0.0, y, w, h));
            y += h;
        }
        out.push(Rect::new(0.0, y1, x1.max(0.0), h));
        out
    }

    /// Selection rects for the byte range `from..to` of an ordinary
    /// shaped run, in physical px from the run's origin — cosmic-text's
    /// own `highlight`, which is what the editor paints with
    /// (`crates/kui-core/src/edit.rs`), so a selection over a label and
    /// one over a field are the same shape.
    fn run_highlight(entry: &CachedText, from: usize, to: usize) -> Vec<Rect> {
        let (a, b) = (entry.cursor_of(from), entry.cursor_of(to));
        let mut out = Vec::new();
        for run in entry.buffer.layout_runs() {
            if run.line_i < a.line || run.line_i > b.line {
                continue;
            }
            // The run's own height: a line holding a larger span is taller
            // than the paragraph's metric.
            let h = run.line_height;
            let mut any = false;
            for (x, w) in run.highlight(a, b) {
                any = true;
                out.push(Rect::new(x, run.line_top, w.max(2.0), h));
            }
            // A selected newline on an empty line keeps the highlight
            // continuous, the way the editor's does.
            if !any && run.glyphs.is_empty() && b.line > run.line_i {
                out.push(Rect::new(0.0, run.line_top, 2.0, h));
            }
        }
        out
    }

    /// The physical origin `emit` drew a place at: what a point is
    /// measured from and a rect is measured to.
    fn physical_origin(&self, place: &TextPlace) -> (f32, f32) {
        (
            crate::geom::snap_px(place.origin.x * self.scale),
            crate::geom::snap_px(place.origin.y * self.scale),
        )
    }

    /// A run's laid-out box, physical px in viewport space.
    fn physical_box(&self, place: &TextPlace, entry: &CachedText) -> Rect {
        let (ox, oy) = self.physical_origin(place);
        let (size, _) = measure_buffer(&entry.buffer, entry.max_lines);
        Rect::new(ox, oy, size.w, size.h)
    }

    /// Where `point` (logical viewport px) lands in the text `key` drew;
    /// see `Core::text_hit`. With several runs, the run under the point,
    /// else the nearest one on the point's line, else the nearest line.
    pub(crate) fn hit_at(&self, key: Key, point: Vec2, prev: bool) -> Option<TextHit> {
        let runs = self.runs_of(key, prev);
        if runs.is_empty() {
            return None;
        }
        let px = point.x * self.scale;
        let py = point.y * self.scale;
        // Distance from a run's box: vertical first, so a point on a line
        // of runs picks among that line, then horizontal.
        let gap = |r: &Rect| {
            let dy = (r.y - py).max(py - (r.y + r.h)).max(0.0);
            let dx = (r.x - px).max(px - (r.x + r.w)).max(0.0);
            (dy, dx)
        };
        let (place, entry, base) = runs
            .iter()
            .min_by(|a, b| {
                let ga = gap(&self.entry_box(a.0, a.1));
                let gb = gap(&self.entry_box(b.0, b.1));
                ga.partial_cmp(&gb).unwrap_or(std::cmp::Ordering::Equal)
            })
            .copied()?;
        let (ox, oy) = self.physical_origin(place);
        let (byte, hit_top) = match entry {
            Entry::Long(line) => {
                let hit = self.long_hit(place, line, point)?;
                (hit.byte, oy + hit.line as f32 * line.line_h)
            }
            Entry::Run(entry) => {
                // Into the run's box vertically: the nearest run was
                // chosen for a point outside every run, and cosmic-text
                // answers a point above its first row with byte 0 whatever
                // the x — so a press in the padding above a line's text
                // placed the caret at its start (backlog C34). Clamped, it
                // hits the nearest row at that x.
                let bx = self.physical_box(place, entry);
                let py = py.clamp(bx.y, (bx.y + bx.h - 0.01).max(bx.y));
                let cursor = entry.buffer.hit(px - ox, py - oy)?;
                let row =
                    visual_line(&entry.buffer, cursor.line, cursor.index).map_or(0, |(l, _)| l);
                let top = entry
                    .buffer
                    .layout_runs()
                    .nth(row)
                    .map_or(0.0, |r| r.line_top);
                (entry.byte_of(cursor), oy + top)
            }
        };
        // The visual row within the node (AR30): the row the hit landed
        // on, placed among every row of every run the key covers by its
        // top edge, so runs side by side share a row and runs stacked
        // count in turn.
        let mut tops: Vec<f32> = runs
            .iter()
            .flat_map(|(p, e, _)| self.row_tops(p, e))
            .collect();
        tops.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        tops.dedup_by(|a, b| (*a - *b).abs() < 0.5);
        let line = tops.iter().filter(|t| **t < hit_top - 0.5).count();
        Some(TextHit {
            byte: base + byte,
            line: line as u32,
        })
    }

    /// The caret rect for `byte` in the text `key` drew; see
    /// `Core::caret_rect`. A byte on the seam between two runs is the
    /// start of the later one, except at the very end.
    pub(crate) fn caret_at(&self, key: Key, byte: usize, prev: bool) -> Option<Rect> {
        let runs = self.runs_of(key, prev);
        let total = runs.last().map(|(_, e, base)| base + e.content().len())?;
        let byte = byte.min(total);
        let (place, entry, base) = runs
            .iter()
            .find(|(_, e, base)| byte < base + e.content().len())
            .or_else(|| runs.last())
            .copied()?;
        let byte = (byte - base).min(entry.content().len());
        let entry = match entry {
            Entry::Long(line) => return self.long_caret(place, line, byte),
            Entry::Run(entry) => entry,
        };
        let (ox, oy) = self.physical_origin(place);
        let cursor = entry.cursor_of(byte);
        let (_, run_no) = visual_line(&entry.buffer, cursor.line, cursor.index)?;
        let run = entry.buffer.layout_runs().nth(run_no)?;
        let x = caret_x(&run, cursor.index);
        let scale = self.scale;
        Some(Rect::new(
            (ox + x) / scale,
            (oy + run.line_top) / scale,
            0.0,
            run.line_height / scale,
        ))
    }
}

/// The byte offset in `content` at which each of the buffer's paragraphs
/// starts. cosmic-text splits on every line ending it knows and keeps the
/// ending out of the paragraph's text, so each start is found by matching
/// the paragraph back onto the content and skipping the ending after it.
/// The word around `byte`: the run of like characters it sits in, where
/// "like" is one of three classes — word characters (alphanumeric or
/// `_`), whitespace, and everything else. At the very end of the text the
/// word before it, so a double click past the last character selects the
/// last word rather than nothing.
fn word_range(content: &str, byte: usize) -> (usize, usize) {
    if content.is_empty() {
        return (0, 0);
    }
    #[derive(PartialEq)]
    enum Class {
        Word,
        Space,
        Other,
    }
    let class = |c: char| {
        if c.is_alphanumeric() || c == '_' {
            Class::Word
        } else if c.is_whitespace() {
            Class::Space
        } else {
            Class::Other
        }
    };
    // Past the end (or on the boundary after the last character), the
    // word being pointed at is the one just passed, so step back into it.
    let at = floor_boundary(content, byte.min(content.len()));
    let at = if at >= content.len() {
        content.char_indices().next_back().map_or(0, |(i, _)| i)
    } else {
        at
    };
    let Some(here) = content[at..].chars().next().map(class) else {
        return (at, at);
    };
    let mut start = at;
    for (i, c) in content[..at].char_indices().rev() {
        if class(c) != here {
            break;
        }
        start = i;
    }
    let mut end = at;
    for (i, c) in content[at..].char_indices() {
        if class(c) != here {
            break;
        }
        end = at + i + c.len_utf8();
    }
    (start, end)
}

/// One shaped run's `lo..hi` as HTML, span by span: cosmic-text keeps the
/// attributes the text was shaped with, so the bold in a rich paragraph is
/// still readable off the buffer long after the spans that declared it are
/// gone.
fn html_of_run(entry: &CachedText, lo: usize, hi: usize, out: &mut String) {
    let starts = line_starts(&entry.buffer, &entry.content);
    for (li, line) in entry.buffer.lines.iter().enumerate() {
        let base = starts.get(li).copied().unwrap_or(0);
        let text = line.text();
        if li > 0 {
            // A newline inside one node's text is a line break in the
            // copy, the same way it is on screen.
            if base > lo && base <= hi {
                out.push_str("<br>");
            }
        }
        // `spans_iter` lists only the ranges something *changed* — a
        // plain paragraph has none at all and reads its line's defaults —
        // so this walks the line and asks per character, coalescing runs
        // that answer the same. `get_span` falls back to the defaults
        // itself, which is exactly the case a rich paragraph's gaps are.
        let attrs_list = line.attrs_list();
        let mut spans: Vec<(usize, usize, cosmic_text::AttrsOwned)> = Vec::new();
        for (i, _) in text.char_indices() {
            let a = cosmic_text::AttrsOwned::new(&attrs_list.get_span(i));
            match spans.last_mut() {
                Some((_, end, prev)) if *prev == a && *end == i => {
                    *end = i + text[i..].chars().next().map_or(1, char::len_utf8);
                }
                _ => spans.push((i, i + text[i..].chars().next().map_or(1, char::len_utf8), a)),
            }
        }
        for (start, end, attrs) in &spans {
            let (s, e) = (base + start, base + end);
            let (s, e) = (s.max(lo), e.min(hi));
            if s >= e {
                continue;
            }
            let piece = &entry.content[s..e];
            // Heavier than the line's default, which is the family's
            // regular (F100): a family whose bold face is its SemiBold
            // bolds at 600, and one whose only face is 700 is regular
            // there.
            let bold = attrs.weight > attrs_list.defaults().weight
                || attrs
                    .cache_key_flags
                    .contains(crate::weights::SYNTHETIC_BOLD);
            let italic = attrs.style != cosmic_text::Style::Normal;
            if let Some(c) = attrs.color_opt {
                let _ = std::fmt::Write::write_fmt(
                    out,
                    format_args!(
                        "<span style=\"color:#{:02x}{:02x}{:02x}\">",
                        c.r(),
                        c.g(),
                        c.b()
                    ),
                );
            }
            if bold {
                out.push_str("<b>");
            }
            if italic {
                out.push_str("<i>");
            }
            escape_into(piece, out);
            if italic {
                out.push_str("</i>");
            }
            if bold {
                out.push_str("</b>");
            }
            if attrs.color_opt.is_some() {
                out.push_str("</span>");
            }
        }
    }
}

/// HTML-escapes into `out`. The four that matter in element content and
/// nothing else: a clipboard flavour is not a document, and over-escaping
/// is what turns a copied apostrophe into `&#39;` in someone's email.
fn escape_into(s: &str, out: &mut String) {
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(ch),
        }
    }
}

/// The nearest char boundary at or below `i`, so a slice taken from a/// The
/// nearest char boundary at or below `i`, so a slice taken from a
/// selection never splits a character. A selection's ends come from
/// shaping and are boundaries already; this is for the arithmetic around
/// them (a clamp, a saturating subtraction) that has no such guarantee.
fn floor_boundary(s: &str, i: usize) -> usize {
    let mut i = i.min(s.len());
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

fn line_starts(buffer: &Buffer, content: &str) -> Vec<usize> {
    paragraph_starts(buffer, content).collect()
}

/// Where each of `buffer`'s paragraphs starts in `content`, in order.
/// cosmic-text splits the text at line endings and keeps the pieces, so
/// the ending bytes between two paragraphs are found again in `content`
/// — any of the four spellings — and skipped. No allocation: the two
/// conversions below walk this once and stop.
fn paragraph_starts<'a>(buffer: &'a Buffer, content: &'a str) -> impl Iterator<Item = usize> + 'a {
    let mut at = 0usize;
    buffer.lines.iter().map(move |line| {
        let start = at.min(content.len());
        at += line.text().len();
        let rest = &content[at.min(content.len())..];
        for ending in ["\r\n", "\n\r", "\n", "\r"] {
            if rest.starts_with(ending) {
                at += ending.len();
                break;
            }
        }
        start
    })
}

/// The visual line (0-based over every wrapped line of the buffer) that
/// byte `index` of paragraph `line_i` lays out on, and that run's ordinal
/// in `layout_runs()` (the same number, kept apart for reading): the run
/// whose glyphs cover the byte, else the last run that starts before it —
/// a byte with no glyph inside the paragraph is whitespace a `Word` break
/// swallowed, which ends the row it broke, and the end of the paragraph
/// ends its last — else the paragraph's last run, for a paragraph with no
/// glyphs. The paragraph's last run whatever the byte, as it was, put a
/// caret on a swallowed space at the end of the whole paragraph.
fn visual_line(buffer: &Buffer, line_i: usize, index: usize) -> Option<(usize, usize)> {
    let mut last_of_line = None;
    let mut before = None;
    for (n, run) in buffer.layout_runs().enumerate() {
        if run.line_i != line_i {
            if last_of_line.is_some() {
                break;
            }
            continue;
        }
        last_of_line = Some(n);
        if run.glyphs.iter().any(|g| g.start <= index && index < g.end) {
            return Some((n, n));
        }
        if run
            .glyphs
            .iter()
            .map(|g| g.start)
            .min()
            .is_some_and(|s| s <= index)
        {
            before = Some(n);
        }
    }
    before.or(last_of_line).map(|n| (n, n))
}

/// Where the caret sits for byte `index` inside `run`, physical px from the
/// text origin: the leading edge of the glyph covering it — its right edge
/// in a right-to-left run — and past the last glyph at the run's end.
fn caret_x(run: &cosmic_text::LayoutRun<'_>, index: usize) -> f32 {
    if let Some(g) = run
        .glyphs
        .iter()
        .find(|g| g.start <= index && index < g.end)
    {
        return if g.level.is_rtl() { g.x + g.w } else { g.x };
    }
    match run.glyphs.last() {
        Some(g) if !g.level.is_rtl() => g.x + g.w,
        Some(g) => g.x,
        None => 0.0,
    }
}

impl Default for TextSystem {
    fn default() -> Self {
        Self::new()
    }
}

/// The physical wrap width an entry needs for a logical `max_w`: none when
/// the unwrapped text already fits (or no width was given).
fn wrap_target(entry: &CachedText, max_w_logical: Option<f32>, scale: f32) -> Option<f32> {
    let max_w = max_w_logical?;
    let intrinsic_fits = entry.intrinsic.w <= max_w * scale + 0.5;
    (!intrinsic_fits).then_some(max_w * scale)
}

/// Re-lays the entry's buffer out at `target` (physical px) if it is not
/// already there.
/// Whether a buffer laid out at `current` needs laying out again for
/// `target`: a wrap width that moved by half a physical pixel or less is
/// the same wrap, which is what keeps a box that jitters by float error
/// from reshaping its text every frame. The one rule for the text cache
/// and the editors alike.
pub(crate) fn wrap_differs(current: Option<f32>, target: Option<f32>) -> bool {
    match (current, target) {
        (None, None) => false,
        (Some(a), Some(b)) => (a - b).abs() > 0.5,
        _ => true,
    }
}

fn wrap_entry(entry: &mut CachedText, fs: &mut FontSystem, target: Option<f32>) {
    if wrap_differs(entry.wrap, target) {
        entry.buffer.set_size(target, None);
        entry.buffer.shape_until_scroll(fs, false);
        entry.wrap = target;
    }
}

impl CachedText {
    /// The byte offset in `content` that `cursor` (a paragraph and an
    /// index into it) names, clamped to the content.
    fn byte_of(&self, cursor: cosmic_text::Cursor) -> usize {
        let start = paragraph_starts(&self.buffer, &self.content)
            .nth(cursor.line)
            .unwrap_or(0);
        (start + cursor.index).min(self.content.len())
    }

    /// The cursor at `byte`: the paragraph starting at or before it — the
    /// last one that does — and the offset inside that paragraph's own
    /// text, clamped to it. The inverse of [`Self::byte_of`]; the one
    /// place the byte ↔ paragraph arithmetic lives, where three copies
    /// used to (`run_anchor`, `run_highlight`, `caret_at`).
    fn cursor_of(&self, byte: usize) -> cosmic_text::Cursor {
        let mut li = 0;
        let mut start = 0;
        for (i, s) in paragraph_starts(&self.buffer, &self.content).enumerate() {
            if s <= byte {
                (li, start) = (i, s);
            } else {
                break;
            }
        }
        let len = self.buffer.lines.get(li).map_or(0, |l| l.text().len());
        cosmic_text::Cursor::new(li, (byte - start).min(len))
    }

    fn new(
        mut buffer: Buffer,
        content: String,
        style: &TextStyle,
        span_deco: Vec<SpanDeco>,
        fs: &mut FontSystem,
        frame_no: u64,
    ) -> Self {
        buffer.shape_until_scroll(fs, false);
        let max_lines = line_budget(style);
        let (intrinsic, _) = measure_buffer(&buffer, max_lines);
        let glyphs: usize = buffer.layout_runs().map(|r| r.glyphs.len()).sum();
        Self {
            buffer,
            bytes: ENTRY_BASE_BYTES + content.len() + ENTRY_GLYPH_BYTES * glyphs,
            content,
            wrap: None,
            intrinsic,
            max_lines,
            clamp_w: style.wrap == TextWrap::None || style.ellipsis,
            last_used: frame_no,
            glyphs: Vec::new(),
            deco: Vec::new(),
            span_deco,
            glyphs_built_for: None,
            claimed: u64::MAX,
            claimed_wrap: None,
            breaks_anywhere: style.wrap == TextWrap::Glyph,
            min_content: None,
        }
    }

    /// A copy laid out as this one is, for a second node that draws the
    /// same text at another width ([`TextSystem::own_wrap`]): the shaped
    /// buffer, not the templates, which the copy builds when drawn.
    fn fork(&self, frame_no: u64) -> Self {
        Self {
            buffer: self.buffer.clone(),
            content: self.content.clone(),
            wrap: self.wrap,
            intrinsic: self.intrinsic,
            max_lines: self.max_lines,
            clamp_w: self.clamp_w,
            last_used: frame_no,
            bytes: self.bytes,
            glyphs: Vec::new(),
            deco: Vec::new(),
            span_deco: self.span_deco.clone(),
            glyphs_built_for: None,
            claimed: u64::MAX,
            claimed_wrap: None,
            breaks_anywhere: self.breaks_anywhere,
            min_content: self.min_content,
        }
    }

    /// The widest stretch of this text that no break opportunity falls
    /// inside, physical px — CSS's min-content: the longest word under
    /// `word` and `break-spaces` (a word longer than the box still breaks
    /// between glyphs when drawn, as `overflow-wrap: break-word` does in
    /// CSS, which leaves the min-content alone), the widest glyph under
    /// `glyph`, and the whole line under `none` or an ellipsis, which
    /// never wrap. Trailing whitespace hangs, as in CSS. Read off the
    /// shaped glyphs of whatever width the buffer was last laid out at —
    /// a glyph's advance is the same at every width — with the breaks
    /// `unicode-linebreak` finds, so asking lays nothing out.
    fn min_content(&mut self) -> f32 {
        if let Some(w) = self.min_content {
            return w;
        }
        let w = if self.clamp_w {
            self.intrinsic.w
        } else {
            widest_unbreakable(&self.buffer, self.breaks_anywhere)
        };
        self.min_content = Some(w);
        w
    }
}

/// [`CachedText::min_content`]'s walk: every run of a line in order, a
/// stretch closing at each glyph a break falls before, and a stretch's
/// width up to its last glyph that is not whitespace.
fn widest_unbreakable(buffer: &Buffer, anywhere: bool) -> f32 {
    let mut widest = 0.0f32;
    let mut line = usize::MAX;
    let mut breaks: Vec<usize> = Vec::new();
    let (mut run_w, mut ink) = (0.0f32, 0.0f32);
    for run in buffer.layout_runs() {
        if run.line_i != line {
            widest = widest.max(ink);
            (run_w, ink) = (0.0, 0.0);
            line = run.line_i;
            breaks.clear();
            if !anywhere {
                breaks.extend(unicode_linebreak::linebreaks(run.text).map(|(i, _)| i));
            }
        }
        for g in run.glyphs {
            if anywhere || breaks.binary_search(&g.start).is_ok() {
                widest = widest.max(ink);
                (run_w, ink) = (0.0, 0.0);
            }
            run_w += g.w;
            let blank = run
                .text
                .get(g.start..g.end)
                .is_some_and(|t| t.chars().all(char::is_whitespace));
            if !blank {
                ink = run_w;
            }
        }
    }
    widest.max(ink)
}

/// The shaper's metrics for a font size and line height in physical px,
/// each at least one pixel: cosmic-text asserts a line height is not 0,
/// which aborted a Node or Lua process over `lineHeight = 0` (or a size
/// under 0.4, whose derived height rounds to 0), and a negative one spun
/// its layout without end. A NaN or an infinity is a pixel too. Every
/// buffer kui shapes into is made through here.
pub(crate) fn shaper_metrics(size: f32, line_height: f32) -> Metrics {
    let px = |v: f32| if v.is_finite() { v.max(1.0) } else { 1.0 };
    Metrics::new(px(size), px(line_height))
}

/// A buffer set up for the style's line breaking: cosmic-text's wrap mode,
/// plus tail ellipsizing at the line budget when asked for.
fn new_buffer(fs: &mut FontSystem, style: &TextStyle, scale: f32) -> Buffer {
    let metrics = shaper_metrics(style.size * scale, style.line_height * scale);
    let mut buffer = Buffer::new(fs, metrics);
    buffer.set_wrap(match style.wrap {
        TextWrap::Word | TextWrap::BreakSpaces => Wrap::WordOrGlyph,
        TextWrap::Glyph => Wrap::Glyph,
        TextWrap::None => Wrap::None,
    });
    if style.ellipsis {
        let lines = line_budget(style).max(1);
        buffer.set_ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(lines)));
    }
    buffer.set_size(None, None);
    buffer
}

/// The line budget a style implies: `max_lines`, or one line when only
/// `ellipsis` is set (0 = unlimited).
fn line_budget(style: &TextStyle) -> usize {
    if style.max_lines > 0 {
        style.max_lines as usize
    } else if style.ellipsis {
        1
    } else {
        0
    }
}

fn line_cap(max_lines: usize) -> usize {
    if max_lines == 0 {
        usize::MAX
    } else {
        max_lines
    }
}

/// Physical size of the laid-out buffer plus its line count (both capped
/// by the line budget).
/// The widest laid-out line and the line count of `buffer`, up to
/// `max_lines` of them (0 = all), as physical px: what a text node and an
/// editor both measure themselves by.
pub(crate) fn measure_buffer(buffer: &Buffer, max_lines: usize) -> (Size, u32) {
    let mut w = 0.0f32;
    let mut lines = 0u32;
    // Lines as tall as their tallest span (`Span::size`): the sum of the
    // runs' own heights, and the metric times the count when every run is
    // the metric's, exactly as before.
    let lh = buffer.metrics().line_height;
    let (mut sum, mut uniform) = (0.0f32, true);
    for run in buffer.layout_runs().take(line_cap(max_lines)) {
        w = w.max(run.line_w);
        lines += 1;
        sum += run.line_height;
        uniform &= run.line_height == lh;
    }
    (
        Size::new(w, if uniform { lines as f32 * lh } else { sum }),
        lines,
    )
}

impl TextSystem {
    pub(crate) fn intrinsic(&mut self, id: TextId) -> Size {
        let scale = self.scale;
        if let Some(line) = self.long_of(id) {
            return Size::new(line.width() / scale, line.line_h / scale);
        }
        let e = self.entry_mut(id);
        Size::new(e.intrinsic.w / scale, e.intrinsic.h / scale)
    }

    /// Text `id`'s min-content width, logical px: see
    /// [`CachedText::min_content`]. A long line is 0 — its rows are broken
    /// to any width, and measuring its words would shape what C19 exists
    /// not to.
    pub(crate) fn min_content(&mut self, id: TextId) -> f32 {
        let scale = self.scale;
        if self.long_of(id).is_some() {
            return 0.0;
        }
        self.entry_mut(id).min_content() / scale
    }

    /// The long line behind one of this frame's texts, if it is one.
    fn long_of(&self, id: TextId) -> Option<&LongLine> {
        self.long(self.frame[id.0 as usize].cache_key)
    }

    pub(crate) fn wrapped(&mut self, id: TextId, max_w: f32, fs: &mut FontSystem) -> Size {
        let scale = self.scale;
        if self.long_of(id).is_some() {
            // The box, not the line, is the node's width: emission clips a
            // single row to it, or the rows are broken to it.
            let key = self.frame[id.0 as usize].cache_key;
            let (size, rows) = self.long_size(key, Some(max_w * scale));
            self.long_mut(key).expect("a long line").laid_rows = rows;
            return Size::new(size.w / scale, size.h / scale);
        }
        self.ensure_wrap(id, max_w, fs);
        let e = self.entry_mut(id);
        let (mut m, _) = measure_buffer(&e.buffer, e.max_lines);
        if e.clamp_w {
            // The line may run past the box; the box, not the line, is
            // the node's width (emission clips to it).
            m.w = m.w.min(max_w * scale);
        }
        Size::new(m.w / scale, m.h / scale)
    }
}

impl TextSystem {
    /// The first line's baseline of text `id` as `wrapped` last laid it
    /// out, logical px below its top. A run's `line_y`, rounded as the glyphs are drawn at
    /// it; a long line's is [`Self::long_baseline`]. An empty text is one
    /// line of its own metrics.
    pub(crate) fn baseline(&mut self, id: TextId) -> f32 {
        let scale = self.scale;
        if let Some(line) = self.long_of(id) {
            return self.long_baseline(line) / scale;
        }
        let e = self.entry_mut(id);
        let b = e
            .buffer
            .layout_runs()
            .next()
            .map_or(e.buffer.metrics().line_height * 0.8, |r| r.line_y.round());
        b / scale
    }
}

/// The generic families resolve to a face that is what its name says:
/// upright, and monospaced for `Mono`.
#[cfg(test)]
mod default_families {
    use super::*;
    use cosmic_text::Family;
    use cosmic_text::fontdb::FaceInfo;

    /// The face `M` shapes with under `family`, on the database as
    /// `new_font_system` set it up.
    fn resolved(fs: &mut FontSystem, family: Family<'_>) -> Option<FaceInfo> {
        let mut buffer = Buffer::new(fs, Metrics::new(14.0, 18.0));
        buffer.set_text("M", &Attrs::new().family(family), Shaping::Advanced, None);
        buffer.shape_until_scroll(fs, false);
        let id = buffer.layout_runs().next()?.glyphs.first()?.font_id;
        fs.db().face(id).cloned()
    }

    #[test]
    fn mono_is_an_upright_monospaced_face() {
        let mut fs = new_font_system().0;
        if !fs.db().faces().any(|f| f.monospaced) {
            eprintln!("skipped: no monospaced face installed");
            return;
        }
        let face = resolved(&mut fs, Family::Monospace).expect("M shapes in Mono");
        let mono = default_families(&fs)[2].to_string();
        assert_eq!(
            face.style,
            cosmic_text::Style::Normal,
            "Mono ({mono}) resolved to {:?}",
            face.post_script_name
        );
        assert!(
            face.monospaced,
            "Mono ({mono}) resolved to {:?}, which is not monospaced",
            face.post_script_name
        );
    }

    #[test]
    fn sans_and_serif_are_upright_and_not_monospaced() {
        let mut fs = new_font_system().0;
        if fs.db().faces().next().is_none() {
            eprintln!("skipped: no face installed");
            return;
        }
        let [sans, serif, _] = default_families(&fs).map(str::to_string);
        for (family, name) in [(Family::SansSerif, sans), (Family::Serif, serif)] {
            let face = resolved(&mut fs, family).expect("M shapes");
            assert_eq!(
                face.style,
                cosmic_text::Style::Normal,
                "{name} resolved to {:?}",
                face.post_script_name
            );
            assert!(
                !face.monospaced,
                "{name} resolved to {:?}, which is monospaced",
                face.post_script_name
            );
        }
    }

    /// Every name pinned is one an installed face answers to, so
    /// `Family::Name(pinned)` and the generic family agree.
    #[test]
    fn pinned_names_are_installed_or_cosmic_texts_own() {
        let fs = new_font_system().0;
        let db = fs.db();
        for (name, list) in default_families(&fs).into_iter().zip(DEFAULT_FAMILIES) {
            let installed = db
                .faces()
                .any(|f| f.families.iter().any(|(fam, _)| fam == name));
            let on_list = list.contains(&name);
            assert!(
                installed == on_list,
                "{name}: installed {installed}, on the platform list {on_list}"
            );
        }
    }
}

/// A face whose glyph advances cannot be measured never reaches the shaper.
/// macOS's GB18030 Bitmap has no `head`, `hhea` or `hmtx`;
/// it says it is fixed-pitch and maps the ideographs, so Han text in
/// `Mono` fell back to it, and its advances came out infinite: every glyph
/// after one was placed at infinity, and a debug build overflowed in
/// `LayoutGlyph::physical`. The fixture faces stand in for it here.
#[cfg(test)]
mod unmeasurable_faces {
    use super::*;
    use crate::testing::{font_face, han_face, unmeasurable_face};
    use cosmic_text::Family;
    use cosmic_text::fontdb::{Database, Source};

    /// A session font system over the fixture faces alone: a monospaced
    /// family without Han, pinned as `Mono`, the face without metrics that
    /// maps 字 and says it is fixed-pitch, and a proportional face that
    /// maps it too.
    fn font_system() -> FontSystem {
        let mut db = Database::new();
        for bytes in [
            font_face("Kui Mono", 400, false, true),
            unmeasurable_face("Kui Bitmap"),
            han_face("Kui Han"),
        ] {
            db.load_font_source(Source::Binary(std::sync::Arc::new(bytes)));
        }
        let mut fs = font_system_with("en-US".into(), db);
        fs.db_mut().set_monospace_family("Kui Mono");
        fs
    }

    fn family(fs: &FontSystem, id: cosmic_text::fontdb::ID) -> String {
        fs.db()
            .face(id)
            .map_or_else(String::new, |f| f.families[0].0.clone())
    }

    #[test]
    fn a_face_without_metrics_is_not_in_the_database() {
        let fs = font_system();
        let families: Vec<String> = fs.db().faces().map(|f| family(&fs, f.id)).collect();
        assert_eq!(families, ["Kui Mono", "Kui Han"]);
    }

    /// "字 a" in `Mono`: 字 from the face that can say how wide it is, the
    /// space and the `a` from the monospaced family after it, every glyph
    /// at a finite place.
    #[test]
    fn han_in_mono_falls_back_to_a_face_that_measures() {
        let mut fs = font_system();
        let mut buffer = Buffer::new(&mut fs, Metrics::new(14.0, 21.0));
        let attrs = Attrs::new().family(Family::Monospace);
        buffer.set_text("字 a", &attrs, Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut fs, false);
        let run = buffer.layout_runs().next().expect("one line");
        let glyphs: Vec<(&str, f32, f32, String)> = run
            .glyphs
            .iter()
            .map(|g| (&run.text[g.start..g.end], g.x, g.w, family(&fs, g.font_id)))
            .collect();
        for &(text, x, w, _) in &glyphs {
            assert!(x.is_finite() && w.is_finite(), "{text:?} at {x}, {w} wide");
        }
        let faces: Vec<(&str, &str)> = glyphs.iter().map(|g| (g.0, g.3.as_str())).collect();
        assert_eq!(
            faces,
            [("字", "Kui Han"), (" ", "Kui Mono"), ("a", "Kui Mono")]
        );
        // The fixture's advance is half an em, 7 px at 14.
        let xs: Vec<f32> = glyphs.iter().map(|g| g.1).collect();
        assert_eq!(xs, [0.0, 7.0, 14.0]);
        for glyph in run.glyphs {
            glyph.physical((0.0, 0.0), 1.0);
        }
    }
}
