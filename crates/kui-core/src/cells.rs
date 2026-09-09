//! A cell grid: what a terminal draws (backlog C20). One node holds
//! `rows × cols` cells — a character, a foreground, a background, a few
//! attribute bits — and its emission is a table walk: a glyph is looked up
//! by character and style variant in a cache that was filled by shaping
//! that one character once, placed at `col × cell_w`, and never shaped
//! again. So a pane whose every cell is new every frame costs the same as
//! one that never changes, which is the property the text runs a terminal
//! could otherwise be built from do not have (see `benches/stream.rs`).
//!
//! What it deliberately is not: shaped text. No ligatures, no kerning, no
//! wrapping — a cell is a cell. A grapheme cluster (an emoji, a base with
//! its combining marks) is one cell's `text`, shaped once; a wide one is
//! marked `WIDE` and the cell after it is a spacer the app leaves blank.
//!
//! Rust-only for now: the element rows, the bindings' transports, the
//! access row and the `cell` field on click and drag payloads are the
//! entry's steps 1–4 and follow the measurement this prototype exists for.

use cosmic_text::{Attrs, Buffer, FontSystem, Metrics, Shaping, Style as FontStyle, Weight};
use rustc_hash::FxHashMap;

use crate::atlas::GlyphAtlas;
use crate::color::Color;
use crate::display::{Clip, ClipId, Quad, QuadKind};
use crate::geom::{Rect, Size, Vec2};
use crate::resources::Resources;
use crate::spec::TextStyle;
use crate::text::{Raster, glyph_kind, raster_glyph};

/// Bits in [`Cell::flags`].
pub mod flags {
    pub const BOLD: u8 = 1;
    pub const ITALIC: u8 = 2;
    pub const UNDERLINE: u8 = 4;
    pub const STRIKETHROUGH: u8 = 8;
    /// The glyph is two cells wide; the app leaves the next cell blank.
    pub const WIDE: u8 = 16;
}

/// One cell: a character, its colours as `0xRRGGBBAA` (a background of 0
/// is none), and attribute bits. Sixteen bytes, so a 200×50 pane is a
/// 160 KB slice a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Cell {
    pub ch: char,
    pub fg: u32,
    pub bg: u32,
    pub flags: u8,
}

impl Cell {
    pub const fn new(ch: char, fg: u32, bg: u32) -> Self {
        Self {
            ch,
            fg,
            bg,
            flags: 0,
        }
    }

    pub const fn with(mut self, flags: u8) -> Self {
        self.flags |= flags;
        self
    }
}

/// How the grid's cursor is drawn, in the colour given with it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CursorShape {
    /// The whole cell, painted under its glyph.
    Block,
    /// A two-pixel bar at the cell's left edge.
    Bar,
    /// A two-pixel line along the cell's bottom.
    Underline,
}

impl CursorShape {
    /// Wire order: the index every binding carries (`block`, `bar`,
    /// `underline`; C's `KUI_CELL_CURSOR_*` is this plus one).
    pub const NAMES: &[&str] = &["block", "bar", "underline"];

    pub fn from_index(i: usize) -> Option<Self> {
        match i {
            0 => Some(Self::Block),
            1 => Some(Self::Bar),
            2 => Some(Self::Underline),
            _ => None,
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        Self::NAMES
            .iter()
            .position(|n| *n == s)
            .and_then(Self::from_index)
    }
}

/// A grid to draw: the cells in row-major order (`rows × cols` of them;
/// fewer draw as blank), the style the glyphs are shaped in (`size`,
/// `line_height` as the cell height, `family` / `font`), and the cursor.
#[derive(Clone, Copy, Debug)]
pub struct CellGrid<'a> {
    pub rows: usize,
    pub cols: usize,
    pub cells: &'a [Cell],
    pub style: TextStyle,
    /// `(row, col, shape, colour)`.
    pub cursor: Option<(usize, usize, CursorShape, Color)>,
    /// The absolute line number of row 0 — where this screenful sits in
    /// the app's own history (`docs/adr/0017-selection-as-a-scope.md`,
    /// decision 4).
    ///
    /// A grid is one screenful and the scrollback behind it is the app's,
    /// so a row number is not an address: it means a different line after
    /// every scroll. Stamping this makes a selection's ends absolute, and
    /// a terminal that scrolls under a selection keeps it. An app that
    /// never sets it gets 0 and a selection that is correct only while it
    /// does not scroll, which is the honest reading of saying nothing.
    pub origin_line: u64,
}

/// Index into the frame's grid list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellsId(pub u32);

struct Entry {
    rows: usize,
    cols: usize,
    cells: Vec<Cell>,
    style: TextStyle,
    cursor: Option<(usize, usize, CursorShape, Color)>,
    origin_line: u64,
}

/// A glyph placed in a cell: where its raster goes, from the cell's
/// top-left, physical px.
#[derive(Clone, Copy)]
struct CellGlyph {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    uv: [u32; 4],
    kind: QuadKind,
}

/// The glyphs of one style at one scale: ASCII by direct index in four
/// variants (plain, bold, italic, both), everything else by map.
struct StyleTable {
    cell_w: f32,
    cell_h: f32,
    ascii: Vec<Option<Option<CellGlyph>>>,
    other: FxHashMap<(char, u8), Option<CellGlyph>>,
    /// The atlas epoch the slots were packed against.
    epoch: u64,
}

const VARIANTS: usize = 4;

fn variant(flags: u8) -> usize {
    (flags & (flags::BOLD | flags::ITALIC)) as usize
}

/// The frame's grids and the glyph tables they draw from.
pub struct CellStore {
    frame: Vec<Entry>,
    tables: FxHashMap<u64, StyleTable>,
    scale: f32,
}

impl Default for CellStore {
    fn default() -> Self {
        Self::new()
    }
}

impl CellStore {
    pub fn new() -> Self {
        Self {
            frame: Vec::new(),
            tables: FxHashMap::default(),
            scale: 1.0,
        }
    }

    pub(crate) fn begin_frame(&mut self, scale: f32) {
        if (scale - self.scale).abs() > f32::EPSILON {
            self.tables.clear();
        }
        self.scale = scale;
        self.frame.clear();
    }

    pub(crate) fn add(&mut self, grid: &CellGrid<'_>) -> CellsId {
        let n = grid.rows * grid.cols;
        let mut cells = Vec::with_capacity(n);
        cells.extend_from_slice(&grid.cells[..grid.cells.len().min(n)]);
        cells.resize(n, Cell::default());
        self.frame.push(Entry {
            rows: grid.rows,
            cols: grid.cols,
            cells,
            style: grid.style,
            cursor: grid.cursor,
            origin_line: grid.origin_line,
        });
        CellsId((self.frame.len() - 1) as u32)
    }

    fn table_key(style: &TextStyle, scale: f32) -> u64 {
        crate::text::TextSystem::style_key("", style, scale)
    }

    /// The table for `style`, built if this is the first time: the cell
    /// width is `M`'s advance and the cell height the style's line
    /// height, both physical.
    fn table(&mut self, style: &TextStyle, res: &Resources, fs: &mut FontSystem) -> u64 {
        let key = Self::table_key(style, self.scale);
        if !self.tables.contains_key(&key) {
            let scale = self.scale;
            let cell_w = shape_one(style, "M", 0, res, fs, scale)
                .map_or(style.size * scale * 0.6, |(_, advance, _)| advance)
                .round()
                .max(1.0);
            self.tables.insert(
                key,
                StyleTable {
                    cell_w,
                    cell_h: (style.line_height * scale).round().max(1.0),
                    ascii: vec![None; VARIANTS * 128],
                    other: FxHashMap::default(),
                    epoch: u64::MAX,
                },
            );
        }
        key
    }

    /// One cell's size, logical px.
    pub(crate) fn cell_size(&mut self, id: CellsId, res: &Resources, fs: &mut FontSystem) -> Size {
        let style = self.frame[id.0 as usize].style;
        let key = self.table(&style, res, fs);
        let t = &self.tables[&key];
        Size::new(t.cell_w / self.scale, t.cell_h / self.scale)
    }

    /// The absolute line the grid's row 0 is (`CellGrid::origin_line`).
    pub(crate) fn origin_line(&self, id: CellsId) -> u64 {
        self.frame[id.0 as usize].origin_line
    }

    /// The character in one cell, and whether it is a spacer after a wide
    /// glyph (which a copy skips rather than turning into a space).
    pub(crate) fn cell_char(&self, id: CellsId, row: usize, col: usize) -> Option<(char, bool)> {
        let e = &self.frame[id.0 as usize];
        if row >= e.rows || col >= e.cols {
            return None;
        }
        let c = e.cells[row * e.cols + col];
        Some((c.ch, c.flags & flags::WIDE != 0))
    }

    pub(crate) fn dims(&self, id: CellsId) -> (usize, usize) {
        let e = &self.frame[id.0 as usize];
        (e.rows, e.cols)
    }

    /// The grid as text, rows joined by newlines with trailing blanks
    /// trimmed — what a screen reader reads (backlog C20).
    pub(crate) fn value(&self, id: CellsId) -> String {
        let e = &self.frame[id.0 as usize];
        let mut out = String::with_capacity(e.rows * (e.cols + 1));
        for r in 0..e.rows {
            let row = &e.cells[r * e.cols..(r + 1) * e.cols];
            let end = row
                .iter()
                .rposition(|c| c.ch != ' ' && c.ch != '\0')
                .map_or(0, |i| i + 1);
            for c in &row[..end] {
                out.push(if c.ch == '\0' { ' ' } else { c.ch });
            }
            if r + 1 < e.rows {
                out.push('\n');
            }
        }
        out
    }

    /// The grid's laid-out size, logical px.
    pub(crate) fn size(&mut self, id: CellsId, res: &Resources, fs: &mut FontSystem) -> Size {
        let (rows, cols, style) = {
            let e = &self.frame[id.0 as usize];
            (e.rows, e.cols, e.style)
        };
        let key = self.table(&style, res, fs);
        let t = &self.tables[&key];
        Size::new(
            cols as f32 * t.cell_w / self.scale,
            rows as f32 * t.cell_h / self.scale,
        )
    }

    /// Emits the grid at `origin` (logical) inside `clip` (physical).
    // The column index is the geometry (`col × cell_w`) as much as the
    // subscript, so the range loops stay.
    #[allow(clippy::too_many_arguments, clippy::needless_range_loop)]
    pub(crate) fn emit(
        &mut self,
        id: CellsId,
        origin: Vec2,
        clip: Clip,
        clip_id: ClipId,
        res: &Resources,
        fs: &mut FontSystem,
        raster: &mut Raster,
        atlas: &mut GlyphAtlas,
        out: &mut Vec<Quad>,
        // The window's selection when it is in *this* grid, and the tint
        // to paint it under (ADR 0017, decision 4). Resolved by the
        // caller, which is the only place that knows which grid is
        // selected in.
        sel: Option<(&crate::select::CellSelection, Color)>,
    ) {
        let scale = self.scale;
        let style = self.frame[id.0 as usize].style;
        let key = self.table(&style, res, fs);
        let ox = crate::geom::snap_px(origin.x * scale);
        let oy = crate::geom::snap_px(origin.y * scale);
        let entry = &self.frame[id.0 as usize];
        let table = self.tables.get_mut(&key).expect("just built");
        if table.epoch != atlas.epoch {
            // The atlas was repacked: every slot is stale.
            table.ascii.iter_mut().for_each(|g| *g = None);
            table.other.clear();
            table.epoch = atlas.epoch;
        }
        let (cw, ch) = (table.cell_w, table.cell_h);
        let quad = |rect: Rect, color: Color, kind: QuadKind, uv: [u32; 4]| Quad {
            rect,
            color,
            border_color: Color::TRANSPARENT,
            radius: [0.0; 4],
            border_w: 0.0,
            blur: 0.0,
            kind,
            clip: clip_id,
            uv,
        };
        // Only the rows and columns the clip can show.
        let r0 = (((clip.rect.y - oy) / ch).floor().max(0.0)) as usize;
        let r1 = (((clip.rect.y + clip.rect.h - oy) / ch).ceil().max(0.0) as usize).min(entry.rows);
        let c0 = (((clip.rect.x - ox) / cw).floor().max(0.0)) as usize;
        let c1 = (((clip.rect.x + clip.rect.w - ox) / cw).ceil().max(0.0) as usize).min(entry.cols);
        if r0 >= r1 || c0 >= c1 {
            return;
        }
        let stroke = (scale).round().max(1.0);
        // The selection, under everything the rows draw: one quad per
        // run of selected columns on each visible row, so a linewise
        // selection is one quad a line and a block selection is a
        // rectangle of them.
        if let Some((sel, tint)) = sel {
            for r in r0..r1 {
                let line = entry.origin_line + r as u64;
                let Some((from, to)) = sel.cols_on(line, entry.cols) else {
                    continue;
                };
                let (from, to) = (from.max(c0), to.min(c1));
                if from >= to {
                    continue;
                }
                out.push(quad(
                    Rect::new(
                        ox + from as f32 * cw,
                        oy + r as f32 * ch,
                        (to - from) as f32 * cw,
                        ch,
                    ),
                    tint,
                    QuadKind::Solid,
                    [0; 4],
                ));
            }
        }
        for r in r0..r1 {
            let row = &entry.cells[r * entry.cols..(r + 1) * entry.cols];
            let cy = oy + r as f32 * ch;
            // Backgrounds: one quad per run of one colour.
            let mut run_start = c0;
            let mut run_bg = row[c0].bg;
            for c in c0..=c1 {
                let bg = if c < c1 { row[c].bg } else { !run_bg };
                if bg != run_bg {
                    if run_bg & 0xff != 0 {
                        out.push(quad(
                            Rect::new(
                                ox + run_start as f32 * cw,
                                cy,
                                (c - run_start) as f32 * cw,
                                ch,
                            ),
                            Color::hex(run_bg),
                            QuadKind::Solid,
                            [0; 4],
                        ));
                    }
                    run_start = c;
                    run_bg = bg;
                }
            }
            // The cursor, under the glyph it sits on.
            if let Some((cr, cc, shape, color)) = entry.cursor
                && cr == r
                && cc >= c0
                && cc < c1
            {
                let cx = ox + cc as f32 * cw;
                let rect = match shape {
                    CursorShape::Block => Rect::new(cx, cy, cw, ch),
                    CursorShape::Bar => Rect::new(cx, cy, 2.0 * stroke, ch),
                    CursorShape::Underline => {
                        Rect::new(cx, cy + ch - 2.0 * stroke, cw, 2.0 * stroke)
                    }
                };
                out.push(quad(rect, color, QuadKind::Solid, [0; 4]));
            }
            // Glyphs, and the lines through and under them per run.
            let mut line_run: Option<(usize, u8, u32)> = None;
            for c in c0..c1 {
                let cell = &row[c];
                let cx = ox + c as f32 * cw;
                if cell.ch != ' ' && cell.ch != '\0' {
                    let g = lookup(
                        table, cell.ch, cell.flags, &style, res, fs, raster, atlas, scale,
                    );
                    if let Some(g) = g {
                        out.push(quad(
                            Rect::new(cx + g.x, cy + g.y, g.w, g.h),
                            Color::hex(cell.fg),
                            g.kind,
                            g.uv,
                        ));
                    }
                }
                let lines = cell.flags & (flags::UNDERLINE | flags::STRIKETHROUGH);
                let same = line_run.is_some_and(|(_, f, fg)| f == lines && fg == cell.fg);
                if !same {
                    if let Some((start, f, fg)) = line_run.take()
                        && f != 0
                    {
                        push_lines(out, &quad, ox, cy, cw, ch, stroke, start, c, f, fg);
                    }
                    line_run = Some((c, lines, cell.fg));
                }
            }
            if let Some((start, f, fg)) = line_run
                && f != 0
            {
                push_lines(out, &quad, ox, cy, cw, ch, stroke, start, c1, f, fg);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn push_lines(
    out: &mut Vec<Quad>,
    quad: &dyn Fn(Rect, Color, QuadKind, [u32; 4]) -> Quad,
    ox: f32,
    cy: f32,
    cw: f32,
    ch: f32,
    stroke: f32,
    start: usize,
    end: usize,
    f: u8,
    fg: u32,
) {
    let x = ox + start as f32 * cw;
    let w = (end - start) as f32 * cw;
    if f & flags::UNDERLINE != 0 {
        out.push(quad(
            Rect::new(x, cy + ch - 2.0 * stroke, w, stroke),
            Color::hex(fg),
            QuadKind::Solid,
            [0; 4],
        ));
    }
    if f & flags::STRIKETHROUGH != 0 {
        out.push(quad(
            Rect::new(x, (cy + ch * 0.55).round(), w, stroke),
            Color::hex(fg),
            QuadKind::Solid,
            [0; 4],
        ));
    }
}

/// The glyph for `ch` in `flags`'s variant, from the table or shaped and
/// rasterized now — once per character and variant for the life of the
/// table.
#[allow(clippy::too_many_arguments)]
fn lookup(
    table: &mut StyleTable,
    ch: char,
    flags: u8,
    style: &TextStyle,
    res: &Resources,
    fs: &mut FontSystem,
    raster: &mut Raster,
    atlas: &mut GlyphAtlas,
    scale: f32,
) -> Option<CellGlyph> {
    let v = variant(flags);
    if (ch as u32) < 128 {
        let i = v * 128 + ch as usize;
        if let Some(g) = table.ascii[i] {
            return g;
        }
        let g = shape_cell(ch, flags, style, res, fs, raster, atlas, scale);
        table.ascii[i] = Some(g);
        return g;
    }
    if let Some(g) = table.other.get(&(ch, v as u8)) {
        return *g;
    }
    let g = shape_cell(ch, flags, style, res, fs, raster, atlas, scale);
    table.other.insert((ch, v as u8), g);
    g
}

/// Shapes one cell's character and rasterizes its glyph into the atlas.
#[allow(clippy::too_many_arguments)]
fn shape_cell(
    ch: char,
    flags: u8,
    style: &TextStyle,
    res: &Resources,
    fs: &mut FontSystem,
    raster: &mut Raster,
    atlas: &mut GlyphAtlas,
    scale: f32,
) -> Option<CellGlyph> {
    let mut buf = [0u8; 4];
    let (key, _, (px, py, line_y)) =
        shape_one(style, ch.encode_utf8(&mut buf), flags, res, fs, scale)?;
    let slot = raster_glyph(key, fs, raster, atlas)?;
    Some(CellGlyph {
        x: px as f32 + slot.left as f32,
        y: line_y.round() + py as f32 - slot.top as f32,
        w: slot.w as f32,
        h: slot.h as f32,
        uv: [slot.x, slot.y, slot.w, slot.h],
        kind: glyph_kind(&slot),
    })
}

/// Shapes `text` alone in `style` at `scale`: the first glyph's cache key,
/// its advance, and where it sits (physical x, y and the baseline).
fn shape_one(
    style: &TextStyle,
    text: &str,
    flags: u8,
    res: &Resources,
    fs: &mut FontSystem,
    scale: f32,
) -> Option<(cosmic_text::CacheKey, f32, (i32, i32, f32))> {
    let metrics = Metrics::new(style.size * scale, style.line_height * scale);
    let mut buffer = Buffer::new(fs, metrics);
    buffer.set_size(None, None);
    let mut attrs = Attrs::new().family(res.family_of(style.family));
    if flags & flags::BOLD != 0 {
        attrs = attrs.weight(Weight::BOLD);
    }
    if flags & flags::ITALIC != 0 {
        attrs = attrs.style(FontStyle::Italic);
    }
    buffer.set_text(text, &attrs, Shaping::Advanced, None);
    buffer.shape_until_scroll(fs, false);
    let run = buffer.layout_runs().next()?;
    let glyph = run.glyphs.first()?;
    let physical = glyph.physical((0.0, 0.0), 1.0);
    Some((
        physical.cache_key,
        glyph.w,
        (physical.x, physical.y, run.line_y),
    ))
}
