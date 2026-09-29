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
//! wrapping — a cell is a cell. And a cell is one *scalar*: `Cell::ch` is
//! a `char`, C's `KuiCell.ch` a `uint32_t`, Node's stream a codepoint
//! packed with its flags. A precomposed character (`é` as U+00E9) is one
//! cell; a base with combining marks, a ZWJ emoji sequence, a flag or a
//! conjunct is not representable — the app precomposes what NFC can and
//! drops what it cannot (backlog C37). A wide character is marked `WIDE`
//! and the cell after it is a spacer the app leaves blank. When a view
//! needs a cluster, the shape is named so it does not grow the cell: a
//! side table of `(cell index, &str)` on `CellGrid` for the few cells
//! whose content is more than a scalar, keyed into the same `other`
//! glyph map by the cluster's string, so a 200 × 50 pane stays 160 KB a
//! frame.
//!
//! Box drawing, block elements and the Powerline separators are not
//! shaped at all: a font's are its line box's height and the cell is
//! `line_height` tall, so every `│` through the font was a dash with a
//! gap under it (backlog F66), and a rounded cap through a fallback font
//! was a squiggle beside its row (F112). `boxdraw` rasterizes them from the cell
//! box into a mask of exactly the cell's size, keyed in the atlas on the
//! character and that size, and they come through `shape_cell` like any
//! glyph so `lookup`'s table caches them the same way.
//!
//! Bound four ways: the `cells` element row, C's `kui_cells` over a
//! `KuiCell` array, Node's `Uint32Array` stream and Lua's `lines` +
//! `runs`, the `terminal` access row, and the `cell` field on click and
//! drag payloads — the entry's steps 1–4, built after the measurement
//! this began as.

use cosmic_text::{Attrs, Buffer, FontSystem, Metrics, Shaping, Style as FontStyle};
use rustc_hash::FxHashMap;

use crate::atlas::GlyphAtlas;
use crate::color::Color;
use crate::display::{Clip, ClipId, Quad, QuadKind};
use crate::geom::{Rect, Size, Vec2};
use crate::key::Key;
use crate::resources::Resources;
use crate::spec::TextStyle;
use crate::text::{Raster, glyph_kind, raster_glyph};

mod boxdraw;

/// Bits in [`Cell::flags`].
pub mod flags {
    pub const BOLD: u8 = 1;
    pub const ITALIC: u8 = 2;
    pub const UNDERLINE: u8 = 4;
    pub const STRIKETHROUGH: u8 = 8;
    /// The glyph is two cells wide; the app leaves the next cell blank.
    pub const WIDE: u8 = 16;
    /// The underline is a wave (SGR 4:3, a terminal's undercurl; backlog
    /// K4). Implies `UNDERLINE`.
    pub const WAVY: u8 = 32;
    /// The underline is dotted (SGR 4:4). Implies `UNDERLINE`.
    pub const DOTTED: u8 = 64;
    /// The bits that make a line under or through a cell, and its shape:
    /// what a run of cells has to agree on to share one.
    pub const LINES: u8 = UNDERLINE | STRIKETHROUGH | WAVY | DOTTED;
}

/// One cell: a character, its colours as `0xRRGGBBAA` (a background of 0
/// is none, an underline colour of 0 the foreground's), and attribute
/// bits. Sixteen bytes, so a 200×50 pane is a 160 KB slice a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Cell {
    pub ch: char,
    pub fg: u32,
    pub bg: u32,
    pub flags: u8,
    /// The underline's own colour (SGR 58; backlog K4), or 0 for `fg`.
    pub ul: u32,
}

impl Cell {
    pub const fn new(ch: char, fg: u32, bg: u32) -> Self {
        Self {
            ch,
            fg,
            bg,
            flags: 0,
            ul: 0,
        }
    }

    /// An underline in its own colour (backlog K4); sets `UNDERLINE`.
    pub const fn underline_color(mut self, ul: u32) -> Self {
        self.flags |= flags::UNDERLINE;
        self.ul = ul;
        self
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
    /// The node that drew it, so a grid stays findable after the frame it
    /// was built in — a view asking about the selection runs while the
    /// next frame's tree is half-built, and the answer is last frame's.
    key: Key,
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
    /// The atlas stamp the slots were looked up against.
    epoch: u64,
}

const VARIANTS: usize = 4;

fn variant(flags: u8) -> usize {
    (flags & (flags::BOLD | flags::ITALIC)) as usize
}

/// The frame's grids and the glyph tables they draw from. The frame
/// before it is kept too, the way the text store keeps its places: a host
/// that reads the selection from inside its own `view` is asking about a
/// frame that has not been built yet.
pub struct CellStore {
    /// This frame's grids and the frame before's — always kept, the way
    /// the text store keeps its places (`retain::Kept`).
    frame: crate::retain::Kept<Entry>,
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
            frame: Default::default(),
            tables: FxHashMap::default(),
            scale: 1.0,
        }
    }

    /// Drops every style's table, to shape again on its next draw: the
    /// weights a family is asked at changed under them (RG59).
    pub(crate) fn forget_shaped(&mut self) {
        self.tables.clear();
    }

    pub(crate) fn begin_frame(&mut self, scale: f32) {
        if (scale - self.scale).abs() > f32::EPSILON {
            self.tables.clear();
        }
        self.scale = scale;
        self.frame.begin(true);
    }

    /// The grid `key` drew, in this frame or the one before it.
    pub(crate) fn find(&self, key: Key, prev: bool) -> Option<CellsId> {
        self.list(prev)
            .iter()
            .position(|e| e.key == key)
            .map(|i| CellsId(i as u32))
    }

    fn list(&self, prev: bool) -> &[Entry] {
        if prev { self.frame.prev() } else { &self.frame }
    }

    fn entry(&self, id: CellsId, prev: bool) -> &Entry {
        &self.list(prev)[id.0 as usize]
    }

    pub(crate) fn add(&mut self, key: Key, grid: &CellGrid<'_>) -> CellsId {
        let n = grid.rows * grid.cols;
        let mut cells = Vec::with_capacity(n);
        cells.extend_from_slice(&grid.cells[..grid.cells.len().min(n)]);
        cells.resize(n, Cell::default());
        self.frame.push(Entry {
            key,
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
    pub(crate) fn cell_size(
        &mut self,
        id: CellsId,
        prev: bool,
        res: &Resources,
        fs: &mut FontSystem,
    ) -> Size {
        let style = self.entry(id, prev).style;
        let key = self.table(&style, res, fs);
        let t = &self.tables[&key];
        Size::new(t.cell_w / self.scale, t.cell_h / self.scale)
    }

    /// The absolute line the grid's row 0 is (`CellGrid::origin_line`).
    pub(crate) fn origin_line(&self, id: CellsId, prev: bool) -> u64 {
        self.entry(id, prev).origin_line
    }

    /// The character in one cell, and whether it is a spacer after a wide
    /// glyph (which a copy skips rather than turning into a space).
    ///
    /// The flag lives on the *glyph*, so the spacer is recognised by the
    /// cell before it — reading `WIDE` off the cell itself said the wide
    /// character was the spacer, and copying a line of CJK gave back a
    /// row of blanks.
    pub(crate) fn cell_char(
        &self,
        id: CellsId,
        row: usize,
        col: usize,
        prev: bool,
    ) -> Option<(char, bool)> {
        let e = self.entry(id, prev);
        if row >= e.rows || col >= e.cols {
            return None;
        }
        let c = e.cells[row * e.cols + col];
        Some((c.ch, self.is_spacer(e, row, col)))
    }

    /// Whether this cell is the blank the app leaves after a wide glyph.
    fn is_spacer(&self, e: &Entry, row: usize, col: usize) -> bool {
        col > 0 && e.cells[row * e.cols + col - 1].flags & flags::WIDE != 0
    }

    /// The word around one cell, as a half-open column range on that row:
    /// the run of like cells it sits in, classed the way a double click
    /// classes text — word characters (alphanumeric or `_`), blanks, and
    /// everything else. `'\0'` and the spacer after a wide glyph are the
    /// glyph's own, so a double click on a wide character takes the pair.
    pub(crate) fn word_at(
        &self,
        id: CellsId,
        row: usize,
        col: usize,
        prev: bool,
    ) -> Option<(usize, usize)> {
        let e = self.entry(id, prev);
        if row >= e.rows || col >= e.cols {
            return None;
        }
        let class = |c: usize| -> u8 {
            // A spacer belongs to the glyph in front of it, so a wide
            // character and its blank are never two different words.
            let c = if self.is_spacer(e, row, c) { c - 1 } else { c };
            let ch = e.cells[row * e.cols + c].ch;
            if ch == '\0' || ch.is_whitespace() {
                1
            } else if ch.is_alphanumeric() || ch == '_' {
                0
            } else {
                2
            }
        };
        let here = class(col);
        let mut from = col;
        while from > 0 && class(from - 1) == here {
            from -= 1;
        }
        let mut to = col + 1;
        while to < e.cols && class(to) == here {
            to += 1;
        }
        Some((from, to))
    }

    pub(crate) fn dims(&self, id: CellsId, prev: bool) -> (usize, usize) {
        let e = self.entry(id, prev);
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
        if table.epoch != atlas.stamp {
            // The page was replaced — reset, whose slots are gone, or
            // resized, whose slots stayed — or the atlas is measuring
            // what the frame uses (RG56): look every one up again.
            table.ascii.iter_mut().for_each(|g| *g = None);
            table.other.clear();
            table.epoch = atlas.stamp;
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
            // Glyphs, and the lines through and under them per run: cells
            // sharing the same line bits and colours share one line.
            let mut line_run: Option<(usize, u8, u32, u32)> = None;
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
                let lines = cell.flags & flags::LINES;
                let same = line_run
                    .is_some_and(|(_, f, fg, ul)| f == lines && fg == cell.fg && ul == cell.ul);
                if !same {
                    if let Some((start, f, fg, ul)) = line_run.take()
                        && f != 0
                    {
                        push_lines(
                            out, &quad, clip_id, ox, cy, cw, ch, stroke, start, c, f, fg, ul,
                        );
                    }
                    line_run = Some((c, lines, cell.fg, cell.ul));
                }
            }
            if let Some((start, f, fg, ul)) = line_run
                && f != 0
            {
                push_lines(
                    out, &quad, clip_id, ox, cy, cw, ch, stroke, start, c1, f, fg, ul,
                );
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn push_lines(
    out: &mut Vec<Quad>,
    quad: &dyn Fn(Rect, Color, QuadKind, [u32; 4]) -> Quad,
    clip_id: ClipId,
    ox: f32,
    cy: f32,
    cw: f32,
    ch: f32,
    stroke: f32,
    start: usize,
    end: usize,
    f: u8,
    fg: u32,
    ul: u32,
) {
    let x = ox + start as f32 * cw;
    let w = (end - start) as f32 * cw;
    if f & (flags::UNDERLINE | flags::WAVY | flags::DOTTED) != 0 {
        // The shape bits imply the line; its colour is its own where the
        // cell says (SGR 58), else the foreground's (backlog K4).
        let style = if f & flags::WAVY != 0 {
            crate::spec::UnderlineStyle::Wavy
        } else if f & flags::DOTTED != 0 {
            crate::spec::UnderlineStyle::Dotted
        } else {
            crate::spec::UnderlineStyle::Solid
        };
        let color = Color::hex(if ul != 0 { ul } else { fg });
        crate::deco::push_line(
            out,
            style,
            x,
            cy + ch - 2.0 * stroke,
            w,
            stroke,
            color,
            clip_id,
        );
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
        let g = shape_cell(
            ch,
            flags,
            style,
            res,
            fs,
            raster,
            atlas,
            scale,
            (table.cell_w, table.cell_h),
        );
        table.ascii[i] = Some(g);
        return g;
    }
    if let Some(g) = table.other.get(&(ch, v as u8)) {
        return *g;
    }
    let g = shape_cell(
        ch,
        flags,
        style,
        res,
        fs,
        raster,
        atlas,
        scale,
        (table.cell_w, table.cell_h),
    );
    table.other.insert((ch, v as u8), g);
    g
}

/// Shapes one cell's character and rasterizes its glyph into the atlas —
/// or, for a character the cell box draws (`boxdraw`), rasterizes the
/// cell-sized mask and skips the font.
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
    cell: (f32, f32),
) -> Option<CellGlyph> {
    if boxdraw::draws(ch) {
        let (w, h) = (cell.0 as u32, cell.1 as u32);
        let slot = atlas.get_or_insert_synth(ch, w, h, || boxdraw::raster(ch, w, h))?;
        return Some(CellGlyph {
            x: 0.0,
            y: 0.0,
            w: w as f32,
            h: h as f32,
            uv: [slot.x, slot.y, slot.w, slot.h],
            kind: QuadKind::GlyphMask,
        });
    }
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
    // Bold at a weight the family has a face for, never another family's
    // (backlog F100).
    let mut attrs = res.weights_of(style.family).apply(
        Attrs::new().family(res.family_of(style.family)),
        flags & flags::BOLD != 0,
    );
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
