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
use crate::display::{Quad, QuadKind};
use crate::geom::{Rect, Size, Vec2};
use crate::layout::TextMeasure;
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
    /// Positioned glyph quads relative to the text origin, so steady-state
    /// emission is a memcpy-style walk instead of per-glyph atlas lookups.
    glyphs: Vec<GlyphTemplate>,
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
}

/// One styled run inside a rich-text paragraph. Spans are shaped and wrapped
/// together as a single flow; plain data, so every frontend can build them.
#[derive(Clone, Copy, Debug)]
pub struct Span<'a> {
    pub text: &'a str,
    pub color: Option<Color>,
    pub bold: bool,
    pub italic: bool,
}

impl<'a> Span<'a> {
    pub fn new(text: &'a str) -> Self {
        Self {
            text,
            color: None,
            bold: false,
            italic: false,
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

pub struct TextSystem {
    font_system: FontSystem,
    raster: Raster,
    cache: FxHashMap<u64, CachedText>,
    frame: Vec<FrameText>,
    /// The previous frame's list, kept the same way and on the same
    /// condition as `Core`'s previous tree: a departing subtree's text
    /// nodes carry that frame's `TextId`s, and this is what they index
    /// (see [`Self::prev_frame_text`]).
    prev_frame: Vec<FrameText>,
    scale: f32,
    frame_no: u64,
}

impl TextSystem {
    pub fn new() -> Self {
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
        Self {
            font_system,
            raster: Raster::new(),
            cache: FxHashMap::default(),
            frame: Vec::new(),
            prev_frame: Vec::new(),
            scale: 1.0,
            frame_no: 0,
        }
    }

    /// Font system + rasterizer, split-borrowed for glyph raster.
    pub(crate) fn raster_parts(&mut self) -> (&mut FontSystem, &mut Raster) {
        (&mut self.font_system, &mut self.raster)
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

    pub(crate) fn font_system(&self) -> &FontSystem {
        &self.font_system
    }

    pub(crate) fn font_system_mut(&mut self) -> &mut FontSystem {
        &mut self.font_system
    }

    /// Starts a frame. `keep_prev` retains the list just finished so the
    /// next frame can still read its texts — `Core` sets it exactly when it
    /// keeps the previous tree, and the two are read together.
    pub(crate) fn begin_frame(&mut self, scale: f32, keep_prev: bool) {
        // Scale change invalidates every physical-px measurement.
        if (scale - self.scale).abs() > f32::EPSILON {
            self.cache.clear();
        }
        self.scale = scale;
        if keep_prev {
            std::mem::swap(&mut self.frame, &mut self.prev_frame);
        } else {
            self.prev_frame.clear();
        }
        self.frame.clear();
        self.frame_no += 1;
        if self.frame_no.is_multiple_of(240) {
            let cutoff = self.frame_no.saturating_sub(EVICT_AFTER_FRAMES);
            self.cache.retain(|_, e| e.last_used >= cutoff);
            // The shape-run cache makes single-line reshapes ~free while
            // editing; trim it so long sessions don't grow unboundedly.
            self.font_system.shape_run_cache.trim(2);
        }
    }

    fn style_key(content: &str, style: &TextStyle, scale: f32) -> u64 {
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
        h
    }

    /// Shapes (or reuses) the buffer for `content` in `style`; returns its
    /// cache key. Shared by text nodes and measurement, so measuring a
    /// string and then drawing it shapes once.
    fn intern(&mut self, content: &str, style: &TextStyle, res: &Resources) -> u64 {
        let key = Self::style_key(content, style, self.scale);
        let frame_no = self.frame_no;
        let scale = self.scale;
        let fs = &mut self.font_system;
        let entry = self.cache.entry(key).or_insert_with(|| {
            let mut buffer = new_buffer(fs, style, scale);
            buffer.set_text(
                content,
                &Attrs::new().family(res.family_of(style.family)),
                Shaping::Advanced,
                None,
            );
            CachedText::new(buffer, content.to_string(), style, fs, frame_no)
        });
        entry.last_used = frame_no;
        key
    }

    /// The content of one of this frame's texts (spans concatenated).
    pub(crate) fn content(&self, id: TextId) -> &str {
        let key = self.frame[id.0 as usize].cache_key;
        self.cache.get(&key).map_or("", |e| e.content.as_str())
    }

    /// Reads one of this frame's laid-out buffers (the access tree walks
    /// its runs).
    pub(crate) fn with_buffer<T>(&self, id: TextId, f: impl FnOnce(&Buffer) -> T) -> Option<T> {
        let key = self.frame.get(id.0 as usize)?.cache_key;
        self.cache.get(&key).map(|e| f(&e.buffer))
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
        let entry = self.cache.get_mut(&cache_key)?;
        entry.last_used = frame_no;
        self.frame.push(FrameText { cache_key, color });
        Some(TextId((self.frame.len() - 1) as u32))
    }

    /// Registers a text for this frame, shaping (or reusing) its buffer.
    pub fn add(&mut self, content: &str, style: &TextStyle, res: &Resources) -> TextId {
        let key = self.intern(content, style, res);
        self.frame.push(FrameText {
            cache_key: key,
            color: style.color,
        });
        TextId((self.frame.len() - 1) as u32)
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
        max_w: Option<f32>,
    ) -> TextMetrics {
        let key = self.intern(content, style, res);
        self.measure_key(key, max_w)
    }

    /// `measure` for a rich-text paragraph.
    pub fn measure_rich(
        &mut self,
        spans: &[Span<'_>],
        base: &TextStyle,
        res: &Resources,
        max_w: Option<f32>,
    ) -> TextMetrics {
        let key = self.intern_rich(spans, base, res);
        self.measure_key(key, max_w)
    }

    fn measure_key(&mut self, key: u64, max_w: Option<f32>) -> TextMetrics {
        let scale = self.scale;
        let fs = &mut self.font_system;
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
    pub fn add_rich(&mut self, spans: &[Span<'_>], base: &TextStyle, res: &Resources) -> TextId {
        let key = self.intern_rich(spans, base, res);
        self.frame.push(FrameText {
            cache_key: key,
            color: base.color,
        });
        TextId((self.frame.len() - 1) as u32)
    }

    fn intern_rich(&mut self, spans: &[Span<'_>], base: &TextStyle, res: &Resources) -> u64 {
        let mut key = Self::style_key("", base, self.scale) ^ 0x9e37_79b9_7f4a_7c15;
        for s in spans {
            let mut mix = |bytes: &[u8]| {
                for &b in bytes {
                    key ^= b as u64;
                    key = key.wrapping_mul(0x0000_0100_0000_01b3);
                }
            };
            mix(s.text.as_bytes());
            mix(&[s.bold as u8, s.italic as u8, s.color.is_some() as u8]);
            if let Some(c) = s.color {
                mix(&c.r.to_bits().to_le_bytes());
                mix(&c.g.to_bits().to_le_bytes());
                mix(&c.b.to_bits().to_le_bytes());
                mix(&c.a.to_bits().to_le_bytes());
            }
        }
        let frame_no = self.frame_no;
        let scale = self.scale;
        let fs = &mut self.font_system;
        let entry = self.cache.entry(key).or_insert_with(|| {
            let mut buffer = new_buffer(fs, base, scale);
            let family = res.family_of(base.family);
            buffer.set_rich_text(
                spans.iter().map(|s| (s.text, s.attrs(family))),
                &Attrs::new().family(family),
                Shaping::Advanced,
                None,
            );
            let content = spans.iter().map(|s| s.text).collect::<String>();
            CachedText::new(buffer, content, base, fs, frame_no)
        });
        entry.last_used = frame_no;
        key
    }

    fn entry_mut(&mut self, id: TextId) -> &mut CachedText {
        let key = self.frame[id.0 as usize].cache_key;
        self.cache
            .get_mut(&key)
            .expect("frame text missing from cache")
    }

    fn ensure_wrap(&mut self, id: TextId, max_w_logical: f32) {
        let scale = self.scale;
        let key = self.frame[id.0 as usize].cache_key;
        let fs = &mut self.font_system;
        let entry = self
            .cache
            .get_mut(&key)
            .expect("frame text missing from cache");
        let target = wrap_target(entry, Some(max_w_logical), scale);
        wrap_entry(entry, fs, target);
    }

    /// Emits positioned glyph quads for a laid-out text node.
    /// `origin` and `node` are logical; output quads are physical px.
    pub(crate) fn emit(
        &mut self,
        id: TextId,
        origin: Vec2,
        node: Size,
        clip: Rect,
        atlas: &mut GlyphAtlas,
        out: &mut Vec<Quad>,
    ) {
        self.ensure_wrap(id, node.w);
        let color = self.frame[id.0 as usize].color;
        let key = self.frame[id.0 as usize].cache_key;
        let scale = self.scale;
        let ox = (origin.x * scale).round();
        let oy = (origin.y * scale).round();
        let fs = &mut self.font_system;
        let raster = &mut self.raster;
        let entry = self
            .cache
            .get_mut(&key)
            .expect("frame text missing from cache");

        // Overflowing modes own their box: a line that runs past the node's
        // width is clipped there rather than painted over siblings.
        let clip = if entry.clamp_w {
            let own = Rect::new(ox, oy, (node.w * scale).ceil(), (node.h * scale).ceil());
            clip.intersect(&own)
        } else {
            clip
        };

        // Steady state: same wrap, same atlas — reuse positioned templates.
        let built_for = (entry.wrap.map(f32::to_bits), atlas.epoch);
        if entry.glyphs_built_for != Some(built_for) {
            entry.glyphs.clear();
            let lines = line_cap(entry.max_lines);
            for run in entry.buffer.layout_runs().take(lines) {
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
                    });
                }
            }
            // Rasterizing may have reset the atlas mid-build; rebuild next
            // frame if so by stamping the epoch we actually ended on.
            entry.glyphs_built_for = Some((entry.wrap.map(f32::to_bits), atlas.epoch));
        }

        // Glyph templates are in layout order; skip everything above the clip
        // and stop at the first glyph past it (rows below never come back).
        // Horizontally clipped glyphs (an unwrapped line) are dropped too.
        out.extend(
            entry
                .glyphs
                .iter()
                .filter(|g| {
                    oy + g.y + g.h >= clip.y
                        && ox + g.x < clip.x + clip.w
                        && ox + g.x + g.w > clip.x
                })
                .take_while(|g| oy + g.y <= clip.y + clip.h)
                .map(|g| Quad {
                    rect: Rect::new(ox + g.x, oy + g.y, g.w, g.h),
                    color: g.color.unwrap_or(color),
                    border_color: Color::TRANSPARENT,
                    radius: [0.0; 4],
                    border_w: 0.0,
                    blur: 0.0,
                    kind: g.kind,
                    uv: g.uv,
                    clip,
                }),
        );
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
        fs: &mut FontSystem,
        frame_no: u64,
    ) -> Self {
        buffer.shape_until_scroll(fs, false);
        let max_lines = line_budget(style);
        let (intrinsic, _) = measure_buffer(&buffer, max_lines);
        Self {
            buffer,
            content,
            wrap: None,
            intrinsic,
            max_lines,
            clamp_w: style.wrap == TextWrap::None || style.ellipsis,
            last_used: frame_no,
            glyphs: Vec::new(),
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

impl TextMeasure for TextSystem {
    fn intrinsic(&mut self, id: TextId) -> Size {
        let scale = self.scale;
        let e = self.entry_mut(id);
        Size::new(e.intrinsic.w / scale, e.intrinsic.h / scale)
    }

    fn wrapped(&mut self, id: TextId, max_w: f32) -> Size {
        self.ensure_wrap(id, max_w);
        let scale = self.scale;
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
