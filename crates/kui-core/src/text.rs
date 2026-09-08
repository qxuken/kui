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
use crate::display::{Clip, Quad, QuadKind};
use crate::geom::{Rect, Size, Vec2};
use crate::key::Key;
use crate::resources::Resources;
use crate::spec::{FontFamily, TextStyle, TextWrap};
use crate::tree::TextId;

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

/// Evict cache entries unused for this many frames.
const EVICT_AFTER_FRAMES: u64 = 300;

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

struct CachedText {
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
}

/// The decorations one span (or a plain text's whole content) asked for.
#[derive(Clone, Copy, Default)]
struct SpanDeco {
    underline: bool,
    strikethrough: bool,
    bg: Option<Color>,
}

impl SpanDeco {
    fn of_style(style: &TextStyle) -> Self {
        Self {
            underline: style.underline,
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
    /// The key names a [`LongLine`] rather than a `CachedText`.
    long: bool,
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
struct LongLine {
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
/// selection wrapper around a run, and two to spare.
const PLACE_ANCESTORS: usize = 4;

/// Where a text node was drawn: what `Core::text_hit` and
/// `Core::caret_rect` answer from (backlog C18). Recorded at emission, so
/// a node the frame culled — scrolled out of its clip — has no place and
/// answers nothing, which is also true of a point nobody can click.
struct TextPlace {
    key: Key,
    /// The keys above it, nearest first, as many as `depth` says.
    ancestors: [Key; PLACE_ANCESTORS],
    depth: u8,
    cache_key: u64,
    /// `cache_key` names a [`LongLine`].
    long: bool,
    /// The node's origin, logical viewport px.
    origin: Vec2,
}

impl TextPlace {
    fn answers_to(&self, key: Key) -> bool {
        self.key == key || self.ancestors[..self.depth as usize].contains(&key)
    }
}

/// Where a point landed in the text a keyed node drew: a byte offset and
/// the visual line it is on — the wrapped line, 0-based, not the paragraph.
/// `byte` is a caret position: between two characters, past the last one
/// at the end, and cosmic-text's rule for which side of a glyph the point
/// fell on. For a node holding several text runs the offset runs across
/// them in tree order, the way the access tree reads a `line`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextHit {
    pub byte: usize,
    pub line: u32,
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

/// The shaping and rasterization state of one window. The font database
/// it shapes against is not in here — that is the session's
/// ([`crate::session::Session`]), passed in as `fs` — because a font
/// registered in one window has to shape in every window of the session.
/// What is here is coupled to this window's glyph atlas: the shaped-buffer
/// cache stamps its positioned glyphs with the atlas epoch they were
/// packed against, and the frame lists are what `TextId` indexes.
pub struct TextSystem {
    raster: Raster,
    cache: FxHashMap<u64, CachedText>,
    /// Non-wrapping texts past `LONG_LINE_BYTES`, by their salted key;
    /// their chunks are in `cache` (backlog C19).
    long: FxHashMap<u64, LongLine>,
    /// The sum of the entries' `bytes`, kept exact against inserts and
    /// removals so a frame inside its budget costs one comparison.
    bytes: usize,
    budget: usize,
    frame: Vec<FrameText>,
    /// The previous frame's list, kept the same way and on the same
    /// condition as `Core`'s previous tree: a departing subtree's text
    /// nodes carry that frame's `TextId`s, and this is what they index
    /// (see [`Self::prev_frame_text`]).
    prev_frame: Vec<FrameText>,
    /// Where this frame's text nodes were drawn, and where the last
    /// frame's were: a query during a build answers from the frame that
    /// finished, which is the layout a click was made against.
    places: Vec<TextPlace>,
    prev_places: Vec<TextPlace>,
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

/// The session's font database, set up the way kui shapes against it.
pub(crate) fn new_font_system() -> FontSystem {
    let mut font_system = FontSystem::new();
    // Map the generic sans-serif family to a face with real bold/italic
    // variants; otherwise weight/style matching can wander into whatever
    // font happens to advertise the variant (monospace included).
    let sans = if cfg!(target_os = "macos") {
        Some("Helvetica Neue")
    } else if cfg!(target_os = "windows") {
        Some("Segoe UI")
    } else {
        None // fontconfig platforms usually map sans-serif sensibly
    };
    if let Some(name) = sans {
        font_system.db_mut().set_sans_serif_family(name);
    }
    font_system
}

impl TextSystem {
    pub fn new() -> Self {
        Self {
            raster: Raster::new(),
            cache: FxHashMap::default(),
            long: FxHashMap::default(),
            bytes: 0,
            budget: DEFAULT_TEXT_CACHE_BYTES,
            frame: Vec::new(),
            prev_frame: Vec::new(),
            places: Vec::new(),
            prev_places: Vec::new(),
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
        self.cache.len()
    }

    /// How many long lines are held (backlog C19).
    pub fn long_lines(&self) -> usize {
        self.long.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
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
        let mut order: Vec<(u64, u64)> = self
            .cache
            .iter()
            .filter(|(_, e)| e.last_used < drawn_last_frame)
            .map(|(k, e)| (e.last_used, *k))
            .collect();
        order.sort_unstable();
        for (_, key) in order {
            if self.bytes <= floor {
                break;
            }
            if let Some(e) = self.cache.remove(&key) {
                self.bytes -= e.bytes;
            }
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
    pub(crate) fn begin_frame(&mut self, fs: &mut FontSystem, scale: f32, keep_prev: bool) {
        // Scale change invalidates every physical-px measurement.
        if (scale - self.scale).abs() > f32::EPSILON {
            self.cache.clear();
            self.long.clear();
            self.bytes = 0;
        }
        self.scale = scale;
        if keep_prev {
            std::mem::swap(&mut self.frame, &mut self.prev_frame);
        } else {
            self.prev_frame.clear();
        }
        self.frame.clear();
        // Always kept, unlike `prev_frame`: a query while this frame builds
        // answers from the last one, and this is what it answers from.
        std::mem::swap(&mut self.places, &mut self.prev_places);
        self.places.clear();
        self.frame_no += 1;
        if self.frame_no.is_multiple_of(240) {
            let cutoff = self.frame_no.saturating_sub(EVICT_AFTER_FRAMES);
            let mut freed = 0usize;
            self.cache.retain(|_, e| {
                let keep = e.last_used >= cutoff;
                if !keep {
                    freed += e.bytes;
                }
                keep
            });
            self.long.retain(|_, l| {
                let keep = l.last_used >= cutoff;
                if !keep {
                    freed += l.bytes;
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

    /// Inserts a freshly shaped entry, charging it to the budget.
    fn insert(&mut self, key: u64, entry: CachedText) {
        self.bytes += entry.bytes;
        if let Some(old) = self.cache.insert(key, entry) {
            self.bytes -= old.bytes;
        }
    }

    pub(crate) fn style_key(content: &str, style: &TextStyle, scale: f32) -> u64 {
        // FNV over content + shaping-relevant style bits (color excluded).
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        let mut mix = |bytes: &[u8]| {
            for &b in bytes {
                h ^= b as u64;
                h = h.wrapping_mul(0x0000_0100_0000_01b3);
            }
        };
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
        // decoration rects are built beside the glyph templates.
        mix(&[style.underline as u8, style.strikethrough as u8]);
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
        if !self.cache.contains_key(&key) {
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
            self.insert(key, entry);
        }
        self.cache.get_mut(&key).expect("just inserted").last_used = frame_no;
        key
    }

    /// The content of one of this frame's texts (spans concatenated).
    pub(crate) fn content(&self, id: TextId) -> &str {
        let FrameText {
            cache_key, long, ..
        } = self.frame[id.0 as usize];
        if long {
            return self.long.get(&cache_key).map_or("", |l| l.content.as_str());
        }
        self.cache
            .get(&cache_key)
            .map_or("", |e| e.content.as_str())
    }

    /// Reads one of this frame's laid-out buffers (the access tree walks
    /// its runs). A long line has none: its value is readable through
    /// [`Self::content`] and its chunks are not walked, the way a tall
    /// document's off-screen lines are not.
    pub(crate) fn with_buffer<T>(&self, id: TextId, f: impl FnOnce(&Buffer) -> T) -> Option<T> {
        let ft = self.frame.get(id.0 as usize)?;
        if ft.long {
            return None;
        }
        self.cache.get(&ft.cache_key).map(|e| f(&e.buffer))
    }

    /// The cache key and colour behind one of the *previous* frame's texts
    /// — what a departing subtree keeps instead of its `TextId`, which
    /// indexes a list rebuilt every frame. The subtree is copied out of the
    /// previous frame's tree, so this is the list its ids belong to (see
    /// [`crate::depart`]).
    pub(crate) fn prev_frame_text(&self, id: TextId) -> (u64, Color) {
        match self.prev_frame.get(id.0 as usize) {
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
        let long = if let Some(entry) = self.cache.get_mut(&cache_key) {
            entry.last_used = frame_no;
            false
        } else {
            self.long.get_mut(&cache_key)?.last_used = frame_no;
            true
        };
        self.frame.push(FrameText {
            cache_key,
            color,
            long,
        });
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
        let long = is_long(content, style);
        let key = if long {
            self.intern_long(content, style, res, fs)
        } else {
            self.intern(content, style, res, fs)
        };
        self.frame.push(FrameText {
            cache_key: key,
            color: style.color,
            long,
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
        if let Some(line) = self.long.get_mut(&key) {
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
                let e = &self.cache[&k];
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
        self.bytes += line.bytes;
        self.long.insert(key, line);
        key
    }

    /// Makes sure chunk `i` of the long line `key` is shaped, and moves
    /// the prefix sums if its width was an estimate. Returns whether it
    /// shaped now — what tells a wrapped line its rows need breaking.
    fn ensure_chunk(&mut self, key: u64, i: usize, res: &Resources, fs: &mut FontSystem) -> bool {
        let (text, style, chunk_key, known) = {
            let line = &self.long[&key];
            let c = &line.chunks[i];
            (
                line.content[c.start..c.end].to_string(),
                line.style,
                c.key,
                c.width,
            )
        };
        if known.is_some() && self.cache.contains_key(&chunk_key) {
            self.cache.get_mut(&chunk_key).expect("checked").last_used = self.frame_no;
            return false;
        }
        let k = self.intern(&text, &style, res, fs);
        let w = self.cache[&k].intrinsic.w;
        let line = self.long.get_mut(&key).expect("just read");
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
        let line = self.long.get_mut(&key).expect("a long line");
        line.wrap_w = w;
        line.starts.clear();
        let Some(w) = w else {
            for c in &mut line.chunks {
                c.rows.clear();
            }
            return;
        };
        let w = w.max(1.0);
        line.starts.push((0, 0.0));
        let (mut row, mut x) = (0u32, 0.0f32);
        for c in &mut line.chunks {
            let shaped = c.width.and_then(|_| self.cache.get(&c.key));
            match shaped {
                Some(e) => {
                    let text = &line.content[c.start..c.end];
                    let (rows, end) = break_rows(&e.buffer, text, line.wrap, w, x);
                    row += rows.len() as u32 - 1;
                    x = end;
                    c.rows = rows;
                }
                None => {
                    c.rows.clear();
                    let est = x + c
                        .width
                        .unwrap_or_else(|| (c.end - c.start) as f32 * line.avg);
                    let added = (est / w).floor();
                    row += added as u32;
                    x = est - added * w;
                }
            }
            line.starts.push((row, x));
        }
    }

    /// A long line's size at `max_w` (physical px): one row clamped to it,
    /// or, when the style wraps and the line does not fit, its rows broken
    /// to it. Returns the physical size and the row count.
    fn long_size(&mut self, key: u64, max_w: Option<f32>) -> (Size, u32) {
        let (wrap, width, line_h, cur) = {
            let l = &self.long[&key];
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
                let rows = self.long[&key].rows();
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
                    let line = &self.long[&key];
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
        let entry = self.cache.get_mut(&key).expect("just interned");
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
            color: base.color,
            long: false,
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
            let mut mix = |bytes: &[u8]| {
                for &b in bytes {
                    key ^= b as u64;
                    key = key.wrapping_mul(0x0000_0100_0000_01b3);
                }
            };
            mix(s.text.as_bytes());
            mix(&[
                s.bold as u8,
                s.italic as u8,
                s.color.is_some() as u8,
                s.underline as u8,
                s.strikethrough as u8,
                s.bg.is_some() as u8,
            ]);
            for c in [s.color, s.bg].into_iter().flatten() {
                mix(&c.r.to_bits().to_le_bytes());
                mix(&c.g.to_bits().to_le_bytes());
                mix(&c.b.to_bits().to_le_bytes());
                mix(&c.a.to_bits().to_le_bytes());
            }
        }
        let frame_no = self.frame_no;
        let scale = self.scale;
        if !self.cache.contains_key(&key) {
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
                    strikethrough: s.strikethrough || base.strikethrough,
                    bg: s.bg,
                })
                .collect();
            let entry = CachedText::new(buffer, content, base, decos, fs, frame_no);
            self.insert(key, entry);
        }
        self.cache.get_mut(&key).expect("just inserted").last_used = frame_no;
        key
    }

    fn entry_mut(&mut self, id: TextId) -> &mut CachedText {
        let key = self.frame[id.0 as usize].cache_key;
        self.cache
            .get_mut(&key)
            .expect("frame text missing from cache")
    }

    fn ensure_wrap(&mut self, id: TextId, max_w_logical: f32, fs: &mut FontSystem) {
        if self.frame[id.0 as usize].long {
            return;
        }
        let scale = self.scale;
        let key = self.frame[id.0 as usize].cache_key;
        let entry = self
            .cache
            .get_mut(&key)
            .expect("frame text missing from cache");
        let target = wrap_target(entry, Some(max_w_logical), scale);
        wrap_entry(entry, fs, target);
    }

    /// Emits positioned glyph quads for a laid-out text node.
    /// `origin` and `node` are logical; output quads are physical px.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn emit(
        &mut self,
        id: TextId,
        origin: Vec2,
        node: Size,
        clip: Clip,
        res: &Resources,
        fs: &mut FontSystem,
        atlas: &mut GlyphAtlas,
        out: &mut Vec<Quad>,
    ) {
        let FrameText {
            cache_key: key,
            color,
            long,
        } = self.frame[id.0 as usize];
        let scale = self.scale;
        let ox = crate::geom::snap_px(origin.x * scale);
        let oy = crate::geom::snap_px(origin.y * scale);
        // A long line owns its box the way a no-wrap line does, and draws
        // the chunks inside the clip plus one either side, shaping them
        // now if this is the first time they show (backlog C19).
        if long {
            let own = Rect::new(ox, oy, (node.w * scale).ceil(), (node.h * scale).ceil());
            let clip = clip.intersect(own, crate::display::SQUARE);
            if clip.rect.w <= 0.0 || clip.rect.h <= 0.0 {
                return;
            }
            if let Some(w) = self.long[&key].wrap_w {
                self.emit_long_rows(key, w, ox, oy, color, clip, res, fs, atlas, out);
                return;
            }
            let (first, last) = {
                let line = &self.long[&key];
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
            self.long.get_mut(&key).expect("checked").last_used = self.frame_no;
            for i in first..=last {
                let (chunk_key, x) = {
                    let line = &self.long[&key];
                    (line.chunks[i].key, line.prefix[i])
                };
                let raster = &mut self.raster;
                let entry = self.cache.get_mut(&chunk_key).expect("just ensured");
                emit_entry(entry, ox + x, oy, color, clip, raster, fs, atlas, out);
            }
            return;
        }
        self.ensure_wrap(id, node.w, fs);
        let raster = &mut self.raster;
        let entry = self
            .cache
            .get_mut(&key)
            .expect("frame text missing from cache");

        // Overflowing modes own their box: a line that runs past the node's
        // width is clipped there rather than painted over siblings.
        let clip = if entry.clamp_w {
            let own = Rect::new(ox, oy, (node.w * scale).ceil(), (node.h * scale).ceil());
            clip.intersect(own, crate::display::SQUARE)
        } else {
            clip
        };
        emit_entry(entry, ox, oy, color, clip, raster, fs, atlas, out);
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
        res: &Resources,
        fs: &mut FontSystem,
        atlas: &mut GlyphAtlas,
        out: &mut Vec<Quad>,
    ) {
        let (first, last) = {
            let line = &self.long[&key];
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
        self.long.get_mut(&key).expect("checked").last_used = self.frame_no;
        for i in first..=last {
            let line = &self.long[&key];
            let (row0, head_x) = line.starts[i];
            let chunk = &line.chunks[i];
            if chunk.rows.is_empty() {
                continue;
            }
            let raster = &mut self.raster;
            let entry = self.cache.get_mut(&chunk.key).expect("just ensured");
            emit_entry_rows(
                entry,
                ox,
                oy + row0 as f32 * line.line_h,
                head_x,
                &chunk.rows,
                line.line_h,
                color,
                clip,
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
        if y > clip.rect.y + clip.rect.h {
            break;
        }
        if y + g.h < clip.rect.y || x >= clip.rect.x + clip.rect.w || x + g.w <= clip.rect.x {
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
            uv: g.uv,
            clip: clip.rect,
            clip_radius: clip.radius,
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
            uv: [0; 4],
            clip: clip.rect,
            clip_radius: clip.radius,
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
                    uv: g.uv,
                    clip: clip.rect,
                    clip_radius: clip.radius,
                }),
        );
        out.extend(
            entry
                .deco
                .iter()
                .filter(|d| !d.under && inside(d.x, d.y, d.w, d.h))
                .map(deco_quad),
        );
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
                        color: first
                            .color_opt
                            .map(|c| Color::rgba8(c.r(), c.g(), c.b(), c.a())),
                        under: false,
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
    pub(crate) fn place(&mut self, key: Key, ancestors: &[Key], id: TextId, origin: Vec2) {
        let FrameText {
            cache_key, long, ..
        } = self.frame[id.0 as usize];
        let mut anc = [Key::ROOT; PLACE_ANCESTORS];
        let depth = ancestors.len().min(PLACE_ANCESTORS);
        anc[..depth].copy_from_slice(&ancestors[..depth]);
        self.places.push(TextPlace {
            key,
            ancestors: anc,
            depth: depth as u8,
            cache_key,
            long,
            origin,
        });
    }

    /// The runs `key` names, in tree order, each with its entry and the
    /// byte offset its content starts at in the concatenation — from the
    /// frame that finished (`prev`) or the one being emitted.
    fn runs_of(&self, key: Key, prev: bool) -> Vec<(&TextPlace, &CachedText, usize)> {
        let list = if prev {
            &self.prev_places
        } else {
            &self.places
        };
        let mut base = 0usize;
        let mut out = Vec::new();
        for place in list.iter().filter(|p| p.answers_to(key) && !p.long) {
            let Some(entry) = self.cache.get(&place.cache_key) else {
                continue;
            };
            out.push((place, entry, base));
            base += entry.content.len();
        }
        out
    }

    /// The long line `key` names, if the place that answers to it is one.
    /// A long line is queried alone — the node that holds a 100k-character
    /// line holds nothing else — so it does not join `runs_of`'s
    /// concatenation.
    fn long_place(&self, key: Key, prev: bool) -> Option<(&TextPlace, &LongLine)> {
        let list = if prev {
            &self.prev_places
        } else {
            &self.places
        };
        let place = list.iter().rev().find(|p| p.answers_to(key) && p.long)?;
        let line = self.long.get(&place.cache_key)?;
        Some((place, line))
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
        let byte = match self.cache.get(&c.key) {
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
        let byte = match self.cache.get(&c.key) {
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
        let byte = byte.min(line.content.len());
        if let Some(w) = line.wrap_w
            && !line.chunks.is_empty()
        {
            let i = line.chunk_of_byte(byte);
            let c = &line.chunks[i];
            let local = byte - c.start;
            let (row0, head_x) = line.starts[i];
            let (r, x) = match self.cache.get(&c.key) {
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
            return Some(Rect::new(
                (ox + x) / scale,
                (oy + y) / scale,
                0.0,
                line.line_h / scale,
            ));
        }
        let x = if line.chunks.is_empty() {
            0.0
        } else {
            let i = line.chunk_of_byte(byte);
            let c = &line.chunks[i];
            let local = byte - c.start;
            match self.cache.get(&c.key) {
                Some(e) if c.width.is_some() => {
                    let run = e.buffer.layout_runs().next()?;
                    line.prefix[i] + caret_x(&run, local.min(c.end - c.start))
                }
                _ => line.prefix[i] + local as f32 * line.avg,
            }
        };
        Some(Rect::new(
            (ox + x) / scale,
            oy / scale,
            0.0,
            line.line_h / scale,
        ))
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
        let cursor = entry.buffer.hit(px - ox, py - oy)?;
        let starts = line_starts(&entry.buffer, &entry.content);
        let byte = starts.get(cursor.line).copied().unwrap_or(0) + cursor.index;
        let line = visual_line(&entry.buffer, cursor.line, cursor.index).map_or(0, |(l, _)| l);
        Some(TextHit {
            byte: base + byte.min(entry.content.len()),
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
        let starts = line_starts(&entry.buffer, &entry.content);
        // The paragraph the byte is in: the last one starting at or
        // before it, and its offset inside that paragraph's text.
        let line_i = starts.iter().rposition(|&s| s <= byte).unwrap_or(0);
        let index = (byte - starts[line_i]).min(entry.buffer.lines[line_i].text().len());
        let (_, run_no) = visual_line(&entry.buffer, line_i, index)?;
        let run = entry.buffer.layout_runs().nth(run_no)?;
        let x = caret_x(&run, index);
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
fn line_starts(buffer: &Buffer, content: &str) -> Vec<usize> {
    let mut starts = Vec::with_capacity(buffer.lines.len());
    let mut at = 0usize;
    for line in &buffer.lines {
        starts.push(at.min(content.len()));
        at += line.text().len();
        let rest = &content[at.min(content.len())..];
        for ending in ["\r\n", "\n\r", "\n", "\r"] {
            if rest.starts_with(ending) {
                at += ending.len();
                break;
            }
        }
    }
    starts
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
fn wrap_entry(entry: &mut CachedText, fs: &mut FontSystem, target: Option<f32>) {
    let differs = match (entry.wrap, target) {
        (None, None) => false,
        (Some(a), Some(b)) => (a - b).abs() > 0.5,
        _ => true,
    };
    if differs {
        entry.buffer.set_size(target, None);
        entry.buffer.shape_until_scroll(fs, false);
        entry.wrap = target;
    }
}

impl CachedText {
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
fn measure_buffer(buffer: &Buffer, max_lines: usize) -> (Size, u32) {
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
        let ft = &self.frame[id.0 as usize];
        if !ft.long {
            return None;
        }
        self.long.get(&ft.cache_key)
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
