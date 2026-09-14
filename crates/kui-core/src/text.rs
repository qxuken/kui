//! Core-owned text stack. Shaping and line layout run through cosmic-text and
//! are cached across frames keyed by (content, style, scale) — the layout pass
//! measures through this cache, so shaping survives resizes and static text
//! costs a hash lookup per frame. Rasterization feeds the shared glyph atlas;
//! renderers only ever see positioned atlas quads.

use cosmic_text::{
    Attrs, Buffer, CacheKeyFlags, Ellipsize, EllipsizeHeightLimit, FontSystem, Metrics, Shaping,
    Style as FontStyle, SwashCache, SwashContent, Weight, Wrap,
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

/// The glyph rasterizer: cosmic-text's swash cache for plain alpha masks and
/// color bitmaps, plus our own scaler for LCD subpixel masks (cosmic-text's
/// cache is fixed to `Format::Alpha`).
pub(crate) struct Raster {
    swash: SwashCache,
    ctx: swash::scale::ScaleContext,
    /// Rasterize outline glyphs as per-channel subpixel coverage. Set by
    /// the driver from what its renderer can blend; see
    /// `Core::set_subpixel_text`.
    pub(crate) subpixel: bool,
}

impl Raster {
    fn new() -> Self {
        Self {
            swash: SwashCache::new(),
            ctx: swash::scale::ScaleContext::new(),
            subpixel: false,
        }
    }

    /// cosmic-text's `swash_image`, with `Format::Subpixel`: three
    /// rasterizations shifted by a third of a pixel land in r, g and b.
    /// Color sources are tried first so emoji still come out as bitmaps.
    fn subpixel_image(
        &mut self,
        fs: &mut FontSystem,
        key: cosmic_text::CacheKey,
    ) -> Option<cosmic_text::SwashImage> {
        use swash::scale::{Render, Source, StrikeWith};
        use swash::zeno::{Angle, Format, Transform, Vector};
        let font = fs.get_font(key.font_id, key.font_weight)?;
        let swash_font = font.as_swash();
        let variable_weight = swash_font
            .variations()
            .find_by_tag(swash::Tag::from_be_bytes(*b"wght"));
        let mut scaler = self
            .ctx
            .builder(swash_font)
            .size(f32::from_bits(key.font_size_bits))
            .hint(!key.flags.contains(CacheKeyFlags::DISABLE_HINTING));
        if let Some(v) = variable_weight {
            scaler = scaler.normalized_coords(swash_font.variations().normalized_coords([(
                swash::Tag::from_be_bytes(*b"wght"),
                f32::from(key.font_weight.0).clamp(v.min_value(), v.max_value()),
            )]));
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
        .format(Format::Subpixel)
        .offset(offset)
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
        let image = if raster.subpixel {
            raster.subpixel_image(fs, key)?
        } else {
            raster.swash.get_image_uncached(fs, key)?
        };
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

/// The default byte budget for the shaped-text cache (backlog C16). Sized
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

/// A non-wrapping text at least this long is shaped in chunks (backlog
/// C19): a minified bundle, a log line with a blob in it, a base64 field.
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
    /// The decoration rects that go with them (backlog C22): a span's
    /// background under its glyphs, its underline and strikethrough over
    /// them, one rect per run of the span per line, so they wrap with it.
    deco: Vec<DecoTemplate>,
    /// What each span asked for, by the index its glyphs carry as
    /// metadata; one entry for plain text.
    span_deco: Vec<SpanDeco>,
    /// (wrap, atlas epoch) the template cache was built for.
    glyphs_built_for: Option<(Option<u32>, u64)>,
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
    /// and a wave or dots are built around it at emission (backlog K4).
    style: UnderlineStyle,
}

/// The decorations one span (or a plain text's whole content) asked for.
#[derive(Clone, Copy, Default)]
struct SpanDeco {
    underline: bool,
    underline_color: Option<Color>,
    underline_style: UnderlineStyle,
    strikethrough: bool,
    bg: Option<Color>,
}

impl SpanDeco {
    fn of_style(style: &TextStyle) -> Self {
        Self {
            underline: style.underline,
            underline_color: style.underline_color,
            underline_style: style.underline_style,
            strikethrough: style.strikethrough,
            bg: None,
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
    /// A line under the span, where the face puts its underline (backlog
    /// C22).
    pub underline: bool,
    /// The underline's own colour; `None` is the span's (backlog K4).
    pub underline_color: Option<Color>,
    /// The underline's shape (backlog K4).
    pub underline_style: UnderlineStyle,
    /// A line through the span, where the face puts its strikeout.
    pub strikethrough: bool,
    /// A background behind the span's glyphs, one rect per line it spans,
    /// so it follows the span across a wrap the way a box cannot.
    pub bg: Option<Color>,
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
        }
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

    /// An underline in its own colour (backlog K4); turns it on.
    pub fn underline_color(mut self, c: Color) -> Self {
        self.underline = true;
        self.underline_color = Some(c);
        self
    }

    /// An underline of this shape (backlog K4); turns it on.
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

    fn attrs(&self, family: cosmic_text::Family<'a>) -> Attrs<'a> {
        // Pin the family (the paragraph base's) so weight/style variants
        // stay in one typeface instead of falling back to whatever face
        // matches first.
        let mut attrs = Attrs::new().family(family);
        if self.bold {
            attrs = attrs.weight(Weight::BOLD);
        }
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

struct FrameText {
    cache_key: u64,
    color: Color,
}

/// One entry of the text cache: a shaped run, or a long line whose chunks
/// are runs of their own in the same map (backlog C19). Whether a text is
/// long is decided once, at [`TextSystem::add`], and lives here as the
/// variant; every operation after it asks the entry rather than carrying
/// a flag beside the key (AR6). A long line joins a selection scope's
/// concatenation like any run: being long is how it was shaped, not
/// something a reader dragging across it should feel (ADR 0017).
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
struct Chunk {
    start: usize,
    end: usize,
    key: u64,
    width: Option<f32>,
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

/// A non-wrapping text past [`LONG_LINE_BYTES`], shaped in chunks on demand
/// (backlog C19). The line itself holds no buffer: its chunks are cache
/// entries interned when emission, a hit-test or a caret query lands in
/// them, and `prefix` is where each chunk starts — an estimate from the
/// first chunk's mean advance until the chunk shapes, exact after. So a
/// 100k-character line costs the screenful it shows, a keystroke into it
/// costs the chunk it lands in, and the width the scrollbar sees can move
/// a little as chunks fill in, exact under monospace.
pub(crate) struct LongLine {
    content: String,
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
    fn reprefix(&mut self) {
        let mut at = 0.0f32;
        self.prefix.clear();
        self.prefix.push(0.0);
        for c in &self.chunks {
            at += c
                .width
                .unwrap_or_else(|| (c.end - c.start) as f32 * self.avg);
            self.prefix.push(at);
        }
    }
}

/// Where a long line is cut: after the last whitespace in the second half
/// of each window, else at the last grapheme boundary inside it.
fn chunk_ranges(content: &str) -> Vec<(usize, usize)> {
    use unicode_segmentation::UnicodeSegmentation;
    let mut out = Vec::with_capacity(content.len() / CHUNK_BYTES + 1);
    let mut start = 0usize;
    while start < content.len() {
        let window_end = (start + CHUNK_BYTES).min(content.len());
        let end = if window_end == content.len() {
            window_end
        } else {
            let window = &content[start..window_end];
            let half = CHUNK_BYTES / 2;
            let after_space = window
                .char_indices()
                .filter(|(i, c)| *i >= half && c.is_whitespace())
                .map(|(i, c)| i + c.len_utf8())
                .next_back();
            match after_space {
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
/// this below its `line` raises `text-beyond-line` (backlog AR30).
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
/// `Core::caret_rect` answer from (backlog C18). Recorded at emission, so
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
    /// gutter's text when it reads a `line` (backlog AR30); a query by the
    /// gutter itself still does.
    none_at: u8,
    cache_key: u64,
    /// The node's origin, logical viewport px.
    origin: Vec2,
    /// The innermost `selectable` node above this run, when there is one
    /// (ADR 0017). What `scope_runs` gathers by, and the reason a place
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
/// not the wrapped line within one run's buffer, which is what it was
/// until backlog AR30, and not the ordinal `role="line"` node a pointer
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
/// cache stamps its positioned glyphs with the atlas epoch they were
/// packed against, and the frame lists are what `TextId` indexes.
pub struct TextSystem {
    raster: Raster,
    /// Every shaped run and every long line, by key — a long line's key
    /// is salted (`LONG_SALT`) and its chunks are runs beside it (backlog
    /// C19).
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
/// `Mono` glyph is italic (backlog C32). The first installed name in each
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
pub(crate) fn new_font_system() -> FontSystem {
    let mut font_system = FontSystem::new();
    let db = font_system.db_mut();
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
    font_system
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
            entries: FxHashMap::default(),
            bytes: 0,
            budget: DEFAULT_TEXT_CACHE_BYTES,
            frame: Kept::default(),
            places: Kept::default(),
            scale: 1.0,
            frame_no: 0,
        }
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

    /// How many long lines are held (backlog C19).
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
        // FNV over content + shaping-relevant style bits (color excluded).
        let mut h = crate::key::FNV_OFFSET;
        let mut mix = |bytes: &[u8]| h = crate::key::fnv(h, bytes);
        mix(content.as_bytes());
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
        let frame_no = self.frame_no;
        let scale = self.scale;
        if !self.entries.contains_key(&key) {
            let mut buffer = new_buffer(fs, style, scale);
            buffer.set_text(
                content,
                &Attrs::new()
                    .family(res.family_of(style.family))
                    .font_features(cosmic_features(&style.features)),
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

    /// Reads one of this frame's laid-out buffers (the access tree walks
    /// its runs). A long line has none: its value is readable through
    /// [`Self::content`] and its chunks are not walked, the way a tall
    /// document's off-screen lines are not.
    pub(crate) fn with_buffer<T>(&self, id: TextId, f: impl FnOnce(&Buffer) -> T) -> Option<T> {
        let ft = self.frame.get(id.0 as usize)?;
        self.run(ft.cache_key).map(|e| f(&e.buffer))
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
        self.entries.get_mut(&cache_key)?.touch(frame_no);
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
        let key = if is_long(content, style) {
            self.intern_long(content, style, res, fs)
        } else {
            self.intern(content, style, res, fs)
        };
        self.frame.push(FrameText {
            cache_key: key,
            color: style.color_or_default(),
        });
        TextId((self.frame.len() - 1) as u32)
    }

    /// Registers (or touches) the long line `content` is, shaping its
    /// first chunk for the line height and the advance the rest are
    /// estimated from; see [`LongLine`].
    fn intern_long(
        &mut self,
        content: &str,
        style: &TextStyle,
        res: &Resources,
        fs: &mut FontSystem,
    ) -> u64 {
        let key = Self::style_key(content, style, self.scale) ^ LONG_SALT;
        let frame_no = self.frame_no;
        if let Some(line) = self.long_mut(key) {
            line.last_used = frame_no;
            return key;
        }
        let wrap = style.wrap;
        let style = &TextStyle {
            wrap: TextWrap::None,
            ..*style
        };
        let chunks: Vec<Chunk> = chunk_ranges(content)
            .into_iter()
            .map(|(start, end)| Chunk {
                start,
                end,
                key: Self::style_key(&content[start..end], style, self.scale),
                width: None,
                rows: Vec::new(),
            })
            .collect();
        // The first chunk is shaped now: the line's height and the mean
        // advance the estimates need come from it.
        let (w0, line_h) = match chunks.first() {
            Some(c) => {
                let k = self.intern(&content[c.start..c.end], style, res, fs);
                let e = self.run(k).expect("just interned");
                (e.intrinsic.w, e.buffer.metrics().line_height)
            }
            None => (0.0, style.line_height * self.scale),
        };
        let avg = chunks
            .first()
            .map_or(0.0, |c| w0 / (c.end - c.start).max(1) as f32);
        let mut line = LongLine {
            content: content.to_string(),
            style: *style,
            wrap,
            wrap_w: None,
            starts: Vec::new(),
            chunks,
            prefix: Vec::new(),
            line_h,
            avg,
            last_used: frame_no,
            bytes: 0,
        };
        if let Some(c) = line.chunks.first_mut() {
            c.width = Some(w0);
        }
        line.reprefix();
        line.bytes = ENTRY_BASE_BYTES + line.content.len() + line.chunks.len() * 48;
        self.insert(key, Entry::Long(line));
        key
    }

    /// Makes sure chunk `i` of the long line `key` is shaped, and moves
    /// the prefix sums if its width was an estimate. Returns whether it
    /// shaped now — what tells a wrapped line its rows need breaking.
    fn ensure_chunk(&mut self, key: u64, i: usize, res: &Resources, fs: &mut FontSystem) -> bool {
        let (text, style, chunk_key, known) = {
            let line = self.long(key).expect("a long line");
            let c = &line.chunks[i];
            (
                line.content[c.start..c.end].to_string(),
                line.style,
                c.key,
                c.width,
            )
        };
        let frame_no = self.frame_no;
        if known.is_some()
            && let Some(e) = self.run_mut(chunk_key)
        {
            e.last_used = frame_no;
            return false;
        }
        let k = self.intern(&text, &style, res, fs);
        let w = self.run(k).expect("just interned").intrinsic.w;
        let line = self.long_mut(key).expect("just read");
        line.chunks[i].key = k;
        if line.chunks[i].width != Some(w) {
            line.chunks[i].width = Some(w);
            line.reprefix();
        }
        true
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
            .map(|c| {
                let shaped = c.width.and_then(|_| self.run(c.key));
                let rows = match shaped {
                    Some(e) => {
                        let text = &line.content[c.start..c.end];
                        let (rows, end) = break_rows(&e.buffer, text, line.wrap, w, x);
                        row += rows.len() as u32 - 1;
                        x = end;
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
        if is_long(content, style) {
            let key = self.intern_long(content, style, res, fs);
            let scale = self.scale;
            // Without a width nothing is broken, and the layout the frame
            // holds is left as it is.
            let (size, lines) = match max_w {
                Some(m) => self.long_size(key, Some(m * scale)),
                None => {
                    let line = self.long(key).expect("just interned");
                    (Size::new(line.width(), line.line_h), 1)
                }
            };
            return TextMetrics {
                width: size.w / scale,
                height: size.h / scale,
                lines,
            };
        }
        let key = self.intern(content, style, res, fs);
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
        let key = self.intern_rich(spans, base, res, fs);
        self.measure_key(key, fs, max_w)
    }

    fn measure_key(&mut self, key: u64, fs: &mut FontSystem, max_w: Option<f32>) -> TextMetrics {
        let scale = self.scale;
        let entry = self.run_mut(key).expect("just interned");
        let target = wrap_target(entry, max_w, scale);
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
    /// flow, so wrapping crosses style boundaries correctly.
    pub fn add_rich(
        &mut self,
        spans: &[Span<'_>],
        base: &TextStyle,
        res: &Resources,
        fs: &mut FontSystem,
    ) -> TextId {
        let key = self.intern_rich(spans, base, res, fs);
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
        let mut key = Self::style_key("", base, self.scale) ^ 0x9e37_79b9_7f4a_7c15;
        for s in spans {
            let mut mix = |bytes: &[u8]| key = crate::key::fnv(key, bytes);
            mix(s.text.as_bytes());
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
            for c in [s.color, s.bg, s.underline_color].into_iter().flatten() {
                mix(&c.r.to_bits().to_le_bytes());
                mix(&c.g.to_bits().to_le_bytes());
                mix(&c.b.to_bits().to_le_bytes());
                mix(&c.a.to_bits().to_le_bytes());
            }
        }
        let frame_no = self.frame_no;
        let scale = self.scale;
        if !self.entries.contains_key(&key) {
            let mut buffer = new_buffer(fs, base, scale);
            let family = res.family_of(base.family);
            let features = cosmic_features(&base.features);
            // Each span's index rides its glyphs as metadata, which is how
            // the decoration rects find their span after layout.
            buffer.set_rich_text(
                spans.iter().enumerate().map(|(i, s)| {
                    (
                        s.text,
                        s.attrs(family).font_features(features.clone()).metadata(i),
                    )
                }),
                &Attrs::new().family(family).font_features(features.clone()),
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

    fn ensure_wrap(&mut self, id: TextId, max_w_logical: f32, fs: &mut FontSystem) {
        let scale = self.scale;
        let entry = self.entry_mut(id);
        let target = wrap_target(entry, Some(max_w_logical), scale);
        wrap_entry(entry, fs, target);
    }

    /// Emits positioned glyph quads for a laid-out text node.
    /// `origin` and `node` are logical; output quads are physical px.
    #[allow(clippy::too_many_arguments)]
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
        // A long line owns its box the way a no-wrap line does, and draws
        // the chunks inside the clip plus one either side, shaping them
        // now if this is the first time they show (backlog C19).
        if let Some(line) = self.long(key) {
            let own = Rect::new(ox, oy, (node.w * scale).ceil(), (node.h * scale).ceil());
            let clip = clip.intersect(own, crate::display::SQUARE);
            if clip.rect.w <= 0.0 || clip.rect.h <= 0.0 {
                return;
            }
            let clip_id = crate::display::intern_clip(clips, clip);
            if let Some(((from, to), tint)) = sel {
                let rects = self.long_highlight(line, from, to);
                push_highlight(&rects, ox, oy, tint, clip_id, out);
            }
            if let Some(w) = line.wrap_w {
                self.emit_long_rows(key, w, ox, oy, color, clip, clip_id, res, fs, atlas, out);
                return;
            }
            let (first, last) = {
                if line.chunks.is_empty() {
                    return;
                }
                let a = line.chunk_at(clip.rect.x - ox).saturating_sub(1);
                let b =
                    (line.chunk_at(clip.rect.x + clip.rect.w - ox) + 1).min(line.chunks.len() - 1);
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
                    color,
                    clip,
                    clip_id,
                    raster,
                    fs,
                    atlas,
                    out,
                );
            }
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
            push_highlight(&rects, ox, oy, tint, clip_id, out);
        }
        emit_entry(entry, ox, oy, color, clip, clip_id, raster, fs, atlas, out);
    }
}

/// Pushes selection rects as quads at a run's origin — before its glyphs,
/// so the text stays on top of its own highlight.
fn push_highlight(
    rects: &[Rect],
    ox: f32,
    oy: f32,
    tint: Color,
    clip: ClipId,
    out: &mut Vec<Quad>,
) {
    for r in rects {
        out.push(Quad {
            rect: Rect::new(ox + r.x, oy + r.y, r.w, r.h),
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
    /// unwrapped templates (backlog C19, step 5).
    #[allow(clippy::too_many_arguments)]
    fn emit_long_rows(
        &mut self,
        key: u64,
        w: f32,
        ox: f32,
        oy: f32,
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
            // A chunk shaped now moves every row after it.
            self.relayout_long(key, Some(w));
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
) {
    build_templates(entry, raster, fs, atlas);
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
}

/// The rows a chunk's unwrapped run breaks into at width `w` when its
/// first row starts `head_x` in: greedy, at the break opportunities UAX
/// #14 gives (what cosmic-text's `WordOrGlyph` takes) or at every glyph
/// for `Glyph`, a piece wider than a row breaking by glyph, trailing
/// whitespace hanging past the edge as cosmic-text lets it. Returns the
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
    let mut pieces: Vec<(usize, usize)> = Vec::new();
    match wrap {
        TextWrap::Word => {
            let mut at = 0usize;
            for (i, _) in unicode_linebreak::linebreaks(text) {
                if i > at {
                    pieces.push((at, i));
                    at = i;
                }
            }
            if at < text.len() {
                pieces.push((at, text.len()));
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
            .find(|g| !blank(g))
            .map_or(x0, |g| g.x + g.w);
        if ink_end - row_x0 > avail && x0 > row_x0 {
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
                if g.x + g.w - row_x0 > avail && g.x > row_x0 && !blank(g) {
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
        let built_for = (entry.wrap.map(f32::to_bits), atlas.epoch);
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
            // Rasterizing may have reset the atlas mid-build; rebuild next
            // frame if so by stamping the epoch we actually ended on.
            entry.glyphs_built_for = Some((entry.wrap.map(f32::to_bits), atlas.epoch));
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
    color: Color,
    clip: Clip,
    clip_id: ClipId,
    raster: &mut Raster,
    fs: &mut FontSystem,
    atlas: &mut GlyphAtlas,
    out: &mut Vec<Quad>,
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
        // A span's background goes under its glyphs; its lines go over.
        out.extend(
            entry
                .deco
                .iter()
                .filter(|d| d.under && inside(d.x, d.y, d.w, d.h))
                .map(deco_quad),
        );

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

/// Whether `content` in `style` is shaped in chunks: a plain text past
/// `LONG_LINE_BYTES` with no line breaks of its own, whatever its `wrap`
/// — a wrapped one breaks its chunks into rows ([`LongLine::starts`]).
/// `max_lines` and `ellipsis` keep the whole path, since a line budget
/// is a property of the whole.
fn is_long(content: &str, style: &TextStyle) -> bool {
    content.len() >= LONG_LINE_BYTES
        && style.max_lines == 0
        && !style.ellipsis
        && !content.contains(['\n', '\r'])
}

/// The decoration rects for one laid-out line: consecutive glyphs of one
/// span (its index is their metadata) become one background rect, one
/// underline and one strikethrough, as the span asked. Where the lines go
/// is the face's own recommendation — swash's `underline_offset`,
/// `strikeout_offset` and `stroke_size`, scaled to the glyph's size — read
/// from the run's first glyph, so a fallback glyph in the middle of a
/// span does not move the line.
fn build_decorations(
    run: &cosmic_text::LayoutRun<'_>,
    spans: &[SpanDeco],
    fs: &mut FontSystem,
    out: &mut Vec<DecoTemplate>,
) {
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
            if let Some(bg) = deco.bg {
                out.push(DecoTemplate {
                    x: x0,
                    y: run.line_top,
                    w,
                    h: run.line_height,
                    color: Some(bg),
                    under: true,
                    style: UnderlineStyle::Solid,
                });
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
                    });
                }
            }
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
    /// frame that finished (`prev`) or the one being emitted.
    fn runs_of(&self, key: Key, prev: bool) -> Vec<(&TextPlace, &CachedText, usize)> {
        let list = if prev {
            self.places.prev()
        } else {
            &self.places
        };
        let mut base = 0usize;
        let mut out = Vec::new();
        for place in list.iter().filter(|p| p.drawn && p.answers_to(key)) {
            // A long line answers alone, through `long_place`.
            let Some(entry) = self.run(place.cache_key) else {
                continue;
            };
            out.push((place, entry, base));
            base += entry.content.len();
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
    /// it scrolled away (ADR 0017, tier 2).
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
        let (ox, oy) = self.physical_origin(run.place);
        match run.text {
            Entry::Run(e) => self.physical_box(run.place, e),
            Entry::Long(l) => Rect::new(ox, oy, l.width(), l.line_h * l.rows() as f32),
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
    /// own colour where one was declared (ADR 0017, decision 7).
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
                // A long line is shaped in chunks and holds one style
                // throughout, so its selection is plain escaped text.
                Entry::Long(_) => escape_into(&content[lo..hi], &mut out),
                Entry::Run(entry) => html_of_run(entry, lo, hi, &mut out),
            }
            prev_run = Some(run);
        }
        out
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
                        // A long line holds one style, so its baseline is
                        // the row's height less the descender the metrics
                        // put under it; `line_h` is all this path knows.
                        return Some(Vec2::new(
                            (ox + x) / self.scale,
                            (oy + y + l.line_h * 0.8) / self.scale,
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
    /// nothing between two that share one — the join rule ADR 0017 leaves
    /// open, in its first form. The exact length is known before a byte
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

    /// The long line `key` names, if the place that answers to it is one.
    /// A long line is queried alone — the node that holds a 100k-character
    /// line holds nothing else — so it does not join `runs_of`'s
    /// concatenation.
    fn long_place(&self, key: Key, prev: bool) -> Option<(&TextPlace, &LongLine)> {
        let list = if prev {
            self.places.prev()
        } else {
            &self.places
        };
        list.iter()
            .rev()
            .filter(|p| p.drawn && p.answers_to(key))
            .find_map(|p| Some((p, self.long(p.cache_key)?)))
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
                let cursor = e.buffer.hit(local_x, line.line_h / 2.0)?;
                let lo = rs.byte as usize;
                let hi = c
                    .rows
                    .get(r + 1)
                    .map_or(c.end - c.start, |n| n.byte as usize);
                c.start + cursor.index.clamp(lo, hi)
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

    /// The caret for `byte`, in physical px from the long line's own
    /// origin: its x on its row, and that row's top. What `long_caret`
    /// places in the viewport and what a selection highlight measures
    /// between (ADR 0017).
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
                    let cx = caret_x(&run, local.min(c.end - c.start));
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
                    line.prefix[i] + caret_x(&run, local.min(c.end - c.start))
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
            let h = entry.buffer.metrics().line_height;
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
        if let Some((place, line)) = self.long_place(key, prev) {
            return self.long_hit(place, line, point);
        }
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
                let ga = gap(&self.physical_box(a.0, a.1));
                let gb = gap(&self.physical_box(b.0, b.1));
                ga.partial_cmp(&gb).unwrap_or(std::cmp::Ordering::Equal)
            })
            .copied()?;
        let (ox, oy) = self.physical_origin(place);
        // Into the run's box vertically: the nearest run was chosen for a
        // point outside every run, and cosmic-text answers a point above
        // its first row with byte 0 whatever the x — so a press in the
        // padding above a line's text placed the caret at its start
        // (backlog C34). Clamped, it hits the nearest row at that x.
        let bx = self.physical_box(place, entry);
        let py = py.clamp(bx.y, (bx.y + bx.h - 0.01).max(bx.y));
        let cursor = entry.buffer.hit(px - ox, py - oy)?;
        // The visual row within the node (AR30): the row the hit landed
        // on, placed among every row of every run the key covers by its
        // top edge, so runs side by side share a row and runs stacked
        // count in turn.
        let row = visual_line(&entry.buffer, cursor.line, cursor.index).map_or(0, |(l, _)| l);
        let hit_top = oy
            + entry
                .buffer
                .layout_runs()
                .nth(row)
                .map_or(0.0, |r| r.line_top);
        let mut tops: Vec<f32> = runs
            .iter()
            .flat_map(|(p, e, _)| {
                let (_, oy) = self.physical_origin(p);
                e.buffer.layout_runs().map(move |r| oy + r.line_top)
            })
            .collect();
        tops.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        tops.dedup_by(|a, b| (*a - *b).abs() < 0.5);
        let line = tops.iter().filter(|t| **t < hit_top - 0.5).count();
        Some(TextHit {
            byte: base + entry.byte_of(cursor),
            line: line as u32,
        })
    }

    /// The caret rect for `byte` in the text `key` drew; see
    /// `Core::caret_rect`. A byte on the seam between two runs is the
    /// start of the later one, except at the very end.
    pub(crate) fn caret_at(&self, key: Key, byte: usize, prev: bool) -> Option<Rect> {
        if let Some((place, line)) = self.long_place(key, prev) {
            return self.long_caret(place, line, byte);
        }
        let runs = self.runs_of(key, prev);
        let total = runs.last().map(|(_, e, base)| base + e.content.len())?;
        let byte = byte.min(total);
        let (place, entry, base) = runs
            .iter()
            .find(|(_, e, base)| byte < base + e.content.len())
            .or_else(|| runs.last())
            .copied()?;
        let (ox, oy) = self.physical_origin(place);
        let byte = (byte - base).min(entry.content.len());
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
    let Some(here) = content[at..].chars().next().map(&class) else {
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
            let bold = attrs.weight >= cosmic_text::Weight::BOLD;
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

/// The nearest char boundary at or below `i`, so a slice taken from a/// The nearest char boundary at or below `i`, so a slice taken from a
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
/// whose glyphs cover the byte, else the paragraph's last run — an index
/// at the end of the paragraph, or a paragraph with no glyphs.
fn visual_line(buffer: &Buffer, line_i: usize, index: usize) -> Option<(usize, usize)> {
    let mut last_of_line = None;
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
    }
    last_of_line.map(|n| (n, n))
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
        }
    }
}

/// A buffer set up for the style's line breaking: cosmic-text's wrap mode,
/// plus tail ellipsizing at the line budget when asked for.
fn new_buffer(fs: &mut FontSystem, style: &TextStyle, scale: f32) -> Buffer {
    let metrics = Metrics::new(style.size * scale, style.line_height * scale);
    let mut buffer = Buffer::new(fs, metrics);
    buffer.set_wrap(match style.wrap {
        TextWrap::Word => Wrap::WordOrGlyph,
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
    for run in buffer.layout_runs().take(line_cap(max_lines)) {
        w = w.max(run.line_w);
        lines += 1;
    }
    (
        Size::new(w, lines as f32 * buffer.metrics().line_height),
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
            let (size, _) = self.long_size(key, Some(max_w * scale));
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

/// The generic families resolve to a face that is what its name says
/// (backlog C32): upright, and monospaced for `Mono`.
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
        let mut fs = new_font_system();
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
        let mut fs = new_font_system();
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
        let fs = new_font_system();
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
