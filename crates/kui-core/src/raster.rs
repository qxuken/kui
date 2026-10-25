//! The rasterizer a path's mask is drawn with (backlog C96).
//!
//! It is zeno's — swash's, the one glyphs are drawn with — line for line
//! where it walks an outline into cells, and different in one place:
//! where it keeps them. zeno keeps each scanline's cells in a list sorted
//! by x and finds a new cell's place by walking that list from its start,
//! so a cell costs as many steps as the cells left of it on its row. An
//! outline drawn left to right lands each cell at its row's end, and a
//! row crossed by k edges costs k² steps: the area under a 1,400-vertex
//! zigzag at scale 2 took 1.6 s, a second of it in the bleed's stroke,
//! whose outline has three times the edges. Here a cell is appended to
//! one flat list; at the end the list is sorted by row (a counting sort,
//! linear) and each row is summed into a dense strip the mask's width,
//! swept once left to right. Linear in the cells and the mask's area,
//! and the mask is the same to the byte: the cells hold the same fixed
//! point numbers, summed in a ring where order does not matter, and the
//! sweep is zeno's.
//!
//! Strokes are zeno's stroker (`zeno::apply`) feeding this rasterizer, so
//! a stroke, a dash and the fill's bleed are the outlines they were.
//!
//! zeno is MIT or Apache-2.0, by Chad Brokaw; the cell walk below
//! (`line_to`, `quad_to`, `curve_to` and the splits) is its
//! `raster.rs`, itself FreeType's "gray" rasterizer.

use std::cell::RefCell;

use swash::zeno::{Command, Fill, PathBuilder, Point, Style};

const PIXEL_BITS: i32 = 8;
const ONE_PIXEL: i32 = 1 << PIXEL_BITS;

/// A cell crossed by the outline: its row and column, and the signed
/// area and cover the edges left in it.
#[derive(Clone, Copy, Default)]
struct Cell {
    y: i32,
    x: i32,
    area: i32,
    cover: i32,
}

/// What a raster keeps between calls, per thread: the cells, the same
/// cells by row, where each row starts, and the strip a row is summed
/// into with a bit for each slot it touched. Kept so that a thousand small paths a frame allocate nothing.
#[derive(Default)]
struct Scratch {
    cells: Vec<Cell>,
    by_row: Vec<Cell>,
    starts: Vec<u32>,
    next: Vec<u32>,
    area: Vec<i32>,
    cover: Vec<i32>,
    touched: Vec<u64>,
}

thread_local! {
    static SCRATCH: RefCell<Scratch> = RefCell::new(Scratch::default());
}

/// More cells than this and the scratch is let go after the raster
/// rather than kept: a mask the size of a texture's limit holds a
/// hundred megabytes of them, which a thread should not keep for the
/// hexagons after it.
const KEEP_CELLS: usize = 1 << 20;

/// How the raster lands in the buffer: written over it, or by max with
/// what is there — the fill's bleed, drawn over the fill.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Blend {
    Over,
    Max,
}

/// Rasterizes `cmds` (in the mask's physical px) with `style` into the
/// `w × h` alpha mask `buf`, row after row, as `zeno::Mask` with an
/// explicit size renders into a zeroed buffer. The caller has checked
/// that zeno's fixed point holds the outline (`path::drawable`).
pub(crate) fn render(
    cmds: &[Command],
    style: Style<'_>,
    w: u32,
    h: u32,
    buf: &mut [u8],
    blend: Blend,
) {
    let fill = match style {
        Style::Fill(f) => f,
        Style::Stroke(_) => Fill::NonZero,
    };
    SCRATCH.with(|s| {
        let mut s = s.borrow_mut();
        let s = &mut *s;
        s.cells.clear();
        let mut r = Raster::new(&mut s.cells, w as i32, h as i32);
        swash::zeno::apply(cmds, style, None, &mut r);
        r.finish();
        s.sweep(fill, w as usize, h as usize, buf, blend);
        if s.cells.capacity() > KEEP_CELLS {
            *s = Scratch::default();
        }
    });
}

impl Scratch {
    /// The cells to pixels: sorted by row, each row summed into the strip
    /// and swept left to right as zeno sweeps its sorted list — the cover
    /// carried along, each pixel the coverage of what is carried less its
    /// own cell's area.
    fn sweep(&mut self, fill: Fill, w: usize, h: usize, buf: &mut [u8], blend: Blend) {
        if self.cells.is_empty() || w == 0 {
            return;
        }
        // A counting sort by row: every cell's row is in 0..h, the
        // rasterizer dropped the rest.
        self.starts.clear();
        self.starts.resize(h + 1, 0);
        for c in &self.cells {
            self.starts[c.y as usize + 1] += 1;
        }
        for i in 1..=h {
            self.starts[i] += self.starts[i - 1];
        }
        self.by_row.clear();
        self.by_row.resize(self.cells.len(), Cell::default());
        // The next free place in each row, from its start.
        self.next.clear();
        self.next.extend_from_slice(&self.starts[..h]);
        for c in &self.cells {
            let at = &mut self.next[c.y as usize];
            self.by_row[*at as usize] = *c;
            *at += 1;
        }
        // One slot to the left of the mask, for the cells zeno clamps to
        // x = -1: their cover counts, their pixel is not drawn. A bit per
        // slot says which a row's cells touched, so the sweep visits them
        // in order without sorting them and fills the runs between with
        // one coverage each, skipping those where nothing is carried, as
        // zeno's does: the inside of a stroked outline, the outside of a
        // fill.
        let slots = w + 1;
        self.area.clear();
        self.area.resize(slots, 0);
        self.cover.clear();
        self.cover.resize(slots, 0);
        self.touched.clear();
        self.touched.resize(slots.div_ceil(64), 0);
        for y in 0..h {
            let (a, b) = (self.starts[y] as usize, self.starts[y + 1] as usize);
            if a == b {
                continue;
            }
            for c in &self.by_row[a..b] {
                let i = (c.x + 1) as usize;
                self.area[i] = self.area[i].wrapping_add(c.area);
                self.cover[i] = self.cover[i].wrapping_add(c.cover);
                self.touched[i / 64] |= 1 << (i % 64);
            }
            let row = &mut buf[y * w..(y + 1) * w];
            if b - a > w / 4 {
                // A row this crowded — a zigzag's, every edge crossing it —
                // is swept slot by slot, cheaper than finding its cells.
                self.touched.fill(0);
                let mut acc = 0i32;
                for i in 0..slots {
                    acc = acc.wrapping_add(self.cover[i].wrapping_mul(ONE_PIXEL * 2));
                    let v = coverage(fill, acc.wrapping_sub(self.area[i]));
                    self.area[i] = 0;
                    self.cover[i] = 0;
                    if let Some(x) = i.checked_sub(1) {
                        match blend {
                            Blend::Over => row[x] = v,
                            Blend::Max => row[x] = row[x].max(v),
                        }
                    }
                }
                continue;
            }
            // The next pixel not yet drawn, and the cover carried to it.
            let mut x = 0usize;
            let mut acc = 0i32;
            for word in 0..self.touched.len() {
                let mut bits = std::mem::take(&mut self.touched[word]);
                while bits != 0 {
                    let i = word * 64 + bits.trailing_zeros() as usize;
                    bits &= bits - 1;
                    let (area, cover) = (self.area[i], self.cover[i]);
                    self.area[i] = 0;
                    self.cover[i] = 0;
                    let carried = acc;
                    acc = acc.wrapping_add(cover.wrapping_mul(ONE_PIXEL * 2));
                    // Slot 0 is left of the mask: it carries, it is not drawn.
                    let Some(cx) = i.checked_sub(1) else {
                        continue;
                    };
                    run(&mut row[x..cx], coverage(fill, carried), blend);
                    let v = coverage(fill, acc.wrapping_sub(area));
                    match blend {
                        Blend::Over => row[cx] = v,
                        Blend::Max => row[cx] = row[cx].max(v),
                    }
                    x = cx + 1;
                }
            }
            run(&mut row[x..], coverage(fill, acc), blend);
        }
    }
}

/// A run of pixels between two cells, all at one coverage: nothing to do
/// where nothing is carried, since the mask starts empty and a bleed adds
/// by max.
#[inline(always)]
fn run(px: &mut [u8], v: u8, blend: Blend) {
    if v == 0 || px.is_empty() {
        return;
    }
    match blend {
        Blend::Over => px.fill(v),
        Blend::Max => {
            for p in px {
                *p = (*p).max(v);
            }
        }
    }
}

#[inline(always)]
fn coverage(fill: Fill, mut coverage: i32) -> u8 {
    coverage >>= PIXEL_BITS * 2 + 1 - 8;
    if fill == Fill::EvenOdd {
        coverage &= 511;
        if coverage >= 256 {
            coverage = 511i32.wrapping_sub(coverage);
        }
    } else {
        if coverage < 0 {
            coverage = !coverage;
        }
        if coverage >= 256 {
            coverage = 255;
        }
    }
    coverage as u8
}

#[derive(Clone, Copy, Default)]
struct FixedPoint {
    x: i32,
    y: i32,
}

impl FixedPoint {
    #[inline(always)]
    fn from_point(p: Point) -> Self {
        // zeno casts unchecked; `path::drawable` holds every point well
        // inside an i32 here, where the two casts agree.
        Self {
            x: (p.x * 256.0) as i32,
            y: (p.y * 256.0) as i32,
        }
    }
}

#[inline(always)]
fn trunc(x: i32) -> i32 {
    x >> PIXEL_BITS
}

#[inline(always)]
fn fract(x: i32) -> i32 {
    x & (ONE_PIXEL - 1)
}

/// zeno's `Rasterizer` over a mask from (0, 0) to (`xmax`, `ymax`), its
/// cells pushed to a list instead of a sorted storage.
struct Raster<'a> {
    cells: &'a mut Vec<Cell>,
    xmin: i32,
    xmax: i32,
    ymin: i32,
    ymax: i32,
    start: FixedPoint,
    closed: bool,
    current: Point,
    x: i32,
    y: i32,
    px: i32,
    py: i32,
    cover: i32,
    area: i32,
    invalid: bool,
}

impl<'a> Raster<'a> {
    fn new(cells: &'a mut Vec<Cell>, w: i32, h: i32) -> Self {
        Self {
            cells,
            xmin: 0,
            xmax: w,
            ymin: 0,
            ymax: h,
            start: FixedPoint::default(),
            closed: true,
            current: Point::ZERO,
            x: 0,
            y: 0,
            px: 0,
            py: 0,
            cover: 0,
            area: 0,
            invalid: true,
        }
    }

    /// What zeno does after the outline: an open subpath closed, the cell
    /// it ended in kept.
    fn finish(&mut self) {
        if !self.closed {
            self.line(self.start);
        }
        if !self.invalid {
            self.push();
        }
    }

    #[inline(always)]
    fn push(&mut self) {
        self.cells.push(Cell {
            y: self.y,
            x: self.x,
            area: self.area,
            cover: self.cover,
        });
    }

    #[inline(always)]
    fn set_cell(&mut self, x: i32, y: i32) {
        if !self.invalid && (self.area != 0 || self.cover != 0) {
            self.push();
        }
        self.area = 0;
        self.cover = 0;
        self.x = if x > (self.xmin - 1) {
            x
        } else {
            self.xmin - 1
        };
        self.y = y;
        self.invalid = y >= self.ymax || y < self.ymin || x >= self.xmax;
    }

    fn move_fixed(&mut self, to: FixedPoint) {
        self.set_cell(trunc(to.x), trunc(to.y));
        self.px = to.x;
        self.py = to.y;
    }

    fn line(&mut self, to: FixedPoint) {
        let to_x = to.x;
        let to_y = to.y;
        let mut ey1 = trunc(self.py);
        let ey2 = trunc(to_y);
        if (ey1 >= self.ymax && ey2 >= self.ymax) || (ey1 < self.ymin && ey2 < self.ymin) {
            self.px = to_x;
            self.py = to_y;
            return;
        }
        let mut ex1 = trunc(self.px);
        let ex2 = trunc(to_x);
        let mut fx1 = fract(self.px);
        let mut fy1 = fract(self.py);
        let dx = to_x - self.px;
        let dy = to_y - self.py;
        if ex1 == ex2 && ey1 == ey2 {
            // empty
        } else if dy == 0 {
            self.set_cell(ex2, ey2);
            self.px = to_x;
            self.py = to_y;
            return;
        } else if dx == 0 {
            if dy > 0 {
                loop {
                    let fy2 = ONE_PIXEL;
                    self.cover += fy2 - fy1;
                    self.area += (fy2 - fy1) * fx1 * 2;
                    fy1 = 0;
                    ey1 += 1;
                    self.set_cell(ex1, ey1);
                    if ey1 == ey2 {
                        break;
                    }
                }
            } else {
                loop {
                    let fy2 = 0;
                    self.cover += fy2 - fy1;
                    self.area += (fy2 - fy1) * fx1 * 2;
                    fy1 = ONE_PIXEL;
                    ey1 -= 1;
                    self.set_cell(ex1, ey1);
                    if ey1 == ey2 {
                        break;
                    }
                }
            }
        } else {
            let mut prod = dx * fy1 - dy * fx1;
            let dx_r = if ex1 != ex2 { (0x00FF_FFFF) / dx } else { 0 };
            let dy_r = if ey1 != ey2 { (0x00FF_FFFF) / dy } else { 0 };
            fn udiv(a: i32, b: i32) -> i32 {
                ((a as u64 * b as u64) >> (4 * 8 - PIXEL_BITS)) as i32
            }
            loop {
                if prod <= 0 && prod - dx * ONE_PIXEL > 0 {
                    let fx2 = 0;
                    let fy2 = udiv(-prod, -dx_r);
                    prod -= dy * ONE_PIXEL;
                    self.cover += fy2 - fy1;
                    self.area += (fy2 - fy1) * (fx1 + fx2);
                    fx1 = ONE_PIXEL;
                    fy1 = fy2;
                    ex1 -= 1;
                } else if prod - dx * ONE_PIXEL <= 0 && prod - dx * ONE_PIXEL + dy * ONE_PIXEL > 0 {
                    prod -= dx * ONE_PIXEL;
                    let fx2 = udiv(-prod, dy_r);
                    let fy2 = ONE_PIXEL;
                    self.cover += fy2 - fy1;
                    self.area += (fy2 - fy1) * (fx1 + fx2);
                    fx1 = fx2;
                    fy1 = 0;
                    ey1 += 1;
                } else if prod - dx * ONE_PIXEL + dy * ONE_PIXEL <= 0 && prod + dy * ONE_PIXEL >= 0
                {
                    prod += dy * ONE_PIXEL;
                    let fx2 = ONE_PIXEL;
                    let fy2 = udiv(prod, dx_r);
                    self.cover += fy2 - fy1;
                    self.area += (fy2 - fy1) * (fx1 + fx2);
                    fx1 = 0;
                    fy1 = fy2;
                    ex1 += 1;
                } else {
                    let fx2 = udiv(prod, -dy_r);
                    let fy2 = 0;
                    prod += dx * ONE_PIXEL;
                    self.cover += fy2 - fy1;
                    self.area += (fy2 - fy1) * (fx1 + fx2);
                    fx1 = fx2;
                    fy1 = ONE_PIXEL;
                    ey1 -= 1;
                }
                self.set_cell(ex1, ey1);
                if ex1 == ex2 && ey1 == ey2 {
                    break;
                }
            }
        }
        let fx2 = fract(to_x);
        let fy2 = fract(to_y);
        self.cover += fy2 - fy1;
        self.area += (fy2 - fy1) * (fx1 + fx2);
        self.px = to_x;
        self.py = to_y;
    }

    fn quad(&mut self, control: FixedPoint, to: FixedPoint) {
        let mut arc = [FixedPoint::default(); 16 * 2 + 1];
        arc[0] = to;
        arc[1] = control;
        arc[2] = FixedPoint {
            x: self.px,
            y: self.py,
        };
        if (trunc(arc[0].y) >= self.ymax
            && trunc(arc[1].y) >= self.ymax
            && trunc(arc[2].y) >= self.ymax)
            || (trunc(arc[0].y) < self.ymin
                && trunc(arc[1].y) < self.ymin
                && trunc(arc[2].y) < self.ymin)
        {
            self.px = arc[0].x;
            self.py = arc[0].y;
            return;
        }
        let mut dx = (arc[2].x + arc[0].x - 2 * arc[1].x).abs();
        let dy = (arc[2].y + arc[0].y - 2 * arc[1].y).abs();
        if dx < dy {
            dx = dy;
        }
        let mut draw = 1i32;
        while dx > ONE_PIXEL / 4 {
            dx >>= 2;
            draw <<= 1;
        }
        let mut a = 0;
        loop {
            let mut split = draw.isolate_lowest_one();
            loop {
                split >>= 1;
                if split == 0 {
                    break;
                }
                split_quad(&mut arc[a..]);
                a += 2;
            }
            let p = arc[a];
            self.line(p);
            draw -= 1;
            if draw == 0 {
                break;
            }
            a -= 2;
        }
    }

    fn cubic(&mut self, control1: FixedPoint, control2: FixedPoint, to: FixedPoint) {
        let mut arc = [FixedPoint::default(); 16 * 8 + 1];
        arc[0] = to;
        arc[1] = control2;
        arc[2] = control1;
        arc[3] = FixedPoint {
            x: self.px,
            y: self.py,
        };
        if (trunc(arc[0].y) >= self.ymax
            && trunc(arc[1].y) >= self.ymax
            && trunc(arc[2].y) >= self.ymax
            && trunc(arc[3].y) >= self.ymax)
            || (trunc(arc[0].y) < self.ymin
                && trunc(arc[1].y) < self.ymin
                && trunc(arc[2].y) < self.ymin
                && trunc(arc[3].y) < self.ymin)
        {
            self.px = arc[0].x;
            self.py = arc[0].y;
            return;
        }
        let mut a = 0;
        loop {
            if (2 * arc[a].x - 3 * arc[a + 1].x + arc[a + 3].x).abs() > ONE_PIXEL / 2
                || (2 * arc[a].y - 3 * arc[a + 1].y + arc[a + 3].y).abs() > ONE_PIXEL / 2
                || (arc[a].x - 3 * arc[a + 2].x + 2 * arc[a + 3].x).abs() > ONE_PIXEL / 2
                || (arc[a].y - 3 * arc[a + 2].y + 2 * arc[a + 3].y).abs() > ONE_PIXEL / 2
            {
                if arc.len() - a >= 7 {
                    split_cubic(&mut arc[a..]);
                    a += 3;
                    continue;
                } else {
                    self.line(to);
                    return;
                }
            }
            let p = arc[a];
            self.line(p);
            if a == 0 {
                return;
            }
            a -= 3;
        }
    }
}

impl PathBuilder for Raster<'_> {
    fn current_point(&self) -> Point {
        self.current
    }

    #[inline(always)]
    fn move_to(&mut self, to: impl Into<Point>) -> &mut Self {
        if !self.closed {
            self.line(self.start);
        }
        let to = to.into();
        let p = FixedPoint::from_point(to);
        self.move_fixed(p);
        self.closed = false;
        self.start = p;
        self.current = to;
        self
    }

    #[inline(always)]
    fn line_to(&mut self, to: impl Into<Point>) -> &mut Self {
        let to = to.into();
        self.current = to;
        self.closed = false;
        self.line(FixedPoint::from_point(to));
        self
    }

    #[inline(always)]
    fn quad_to(&mut self, control: impl Into<Point>, to: impl Into<Point>) -> &mut Self {
        let to = to.into();
        self.current = to;
        self.closed = false;
        self.quad(
            FixedPoint::from_point(control.into()),
            FixedPoint::from_point(to),
        );
        self
    }

    #[inline(always)]
    fn curve_to(
        &mut self,
        control1: impl Into<Point>,
        control2: impl Into<Point>,
        to: impl Into<Point>,
    ) -> &mut Self {
        let to = to.into();
        self.current = to;
        self.closed = false;
        self.cubic(
            FixedPoint::from_point(control1.into()),
            FixedPoint::from_point(control2.into()),
            FixedPoint::from_point(to),
        );
        self
    }

    #[inline(always)]
    fn close(&mut self) -> &mut Self {
        self.line(self.start);
        self.closed = true;
        self
    }
}

fn split_quad(base: &mut [FixedPoint]) {
    let mut a;
    let mut b;
    base[4].x = base[2].x;
    a = base[0].x + base[1].x;
    b = base[1].x + base[2].x;
    base[3].x = b >> 1;
    base[2].x = (a + b) >> 2;
    base[1].x = a >> 1;
    base[4].y = base[2].y;
    a = base[0].y + base[1].y;
    b = base[1].y + base[2].y;
    base[3].y = b >> 1;
    base[2].y = (a + b) >> 2;
    base[1].y = a >> 1;
}

fn split_cubic(base: &mut [FixedPoint]) {
    let mut a;
    let mut b;
    let mut c;
    base[6].x = base[3].x;
    a = base[0].x + base[1].x;
    b = base[1].x + base[2].x;
    c = base[2].x + base[3].x;
    base[5].x = c >> 1;
    c += b;
    base[4].x = c >> 2;
    base[1].x = a >> 1;
    a += b;
    base[2].x = a >> 2;
    base[3].x = (a + c) >> 3;
    base[6].y = base[3].y;
    a = base[0].y + base[1].y;
    b = base[1].y + base[2].y;
    c = base[2].y + base[3].y;
    base[5].y = c >> 1;
    c += b;
    base[4].y = c >> 2;
    base[1].y = a >> 1;
    a += b;
    base[2].y = a >> 2;
    base[3].y = (a + c) >> 3;
}

#[cfg(test)]
mod tests {
    use super::*;
    use swash::zeno::{Cap, Join, Mask, Stroke};

    /// zeno's own mask of `cmds`, for the comparison.
    fn zeno(cmds: &[Command], style: Style<'_>, w: u32, h: u32) -> Vec<u8> {
        let mut buf = vec![0u8; (w * h) as usize];
        Mask::new(cmds)
            .style(style)
            .size(w, h)
            .render_into(&mut buf, None);
        buf
    }

    fn ours(cmds: &[Command], style: Style<'_>, w: u32, h: u32) -> Vec<u8> {
        let mut buf = vec![0u8; (w * h) as usize];
        render(cmds, style, w, h, &mut buf, Blend::Over);
        buf
    }

    /// Outlines that reach every branch of the cell walk: steep and
    /// shallow lines both ways, vertical and horizontal ones, curves of
    /// both kinds, a subpath left open, points off the mask on every side,
    /// overlapping contours either way round.
    fn outlines() -> Vec<Vec<Command>> {
        let mut out = Vec::new();
        let mut seed = 0x2545_f491u32;
        let mut rnd = |lo: f32, hi: f32| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            lo + (seed % 10_000) as f32 / 10_000.0 * (hi - lo)
        };
        for n in [3, 5, 12, 40, 200] {
            let mut c: Vec<Command> = Vec::new();
            c.move_to([rnd(-10.0, 90.0), rnd(-10.0, 70.0)]);
            for i in 0..n {
                let p = [rnd(-10.0, 90.0), rnd(-10.0, 70.0)];
                match i % 4 {
                    0 | 1 => {
                        c.line_to(p);
                    }
                    2 => {
                        c.quad_to([rnd(-10.0, 90.0), rnd(-10.0, 70.0)], p);
                    }
                    _ => {
                        c.curve_to(
                            [rnd(-10.0, 90.0), rnd(-10.0, 70.0)],
                            [rnd(-10.0, 90.0), rnd(-10.0, 70.0)],
                            p,
                        );
                    }
                }
            }
            if n % 2 == 1 {
                c.close();
            }
            out.push(c);
        }
        // Axis-aligned, and a ring whose contours run the same way.
        let mut c: Vec<Command> = Vec::new();
        c.move_to([10.0, 10.0])
            .line_to([70.0, 10.0])
            .line_to([70.0, 50.0])
            .line_to([10.0, 50.0])
            .close();
        c.move_to([20.0, 20.0])
            .line_to([60.0, 20.0])
            .line_to([60.0, 40.0])
            .line_to([20.0, 40.0])
            .close();
        out.push(c);
        // A zigzag, the case the storage was changed for.
        let mut c: Vec<Command> = Vec::new();
        c.move_to([0.5, 59.0]);
        for i in 0..160 {
            c.line_to([0.5 + i as f32 * 0.5, if i % 2 == 0 { 2.25 } else { 57.75 }]);
        }
        c.line_to([80.0, 59.0]).close();
        out.push(c);
        out
    }

    #[test]
    fn the_mask_is_zenos_to_the_byte() {
        let (w, h) = (80, 60);
        let mut stroke = Stroke::new(1.0);
        stroke.join(Join::Round).cap(Cap::Round);
        let mut wide = Stroke::new(5.5);
        wide.join(Join::Round).cap(Cap::Round);
        let mut dashed = Stroke::new(2.0);
        dashed
            .join(Join::Round)
            .cap(Cap::Round)
            .dash(&[4.0, 3.0], 1.5);
        let styles: [Style<'_>; 5] = [
            Style::Fill(Fill::NonZero),
            Style::Fill(Fill::EvenOdd),
            Style::Stroke(stroke),
            Style::Stroke(wide),
            Style::Stroke(dashed),
        ];
        for (i, cmds) in outlines().iter().enumerate() {
            for (j, style) in styles.iter().enumerate() {
                assert!(
                    ours(cmds, *style, w, h) == zeno(cmds, *style, w, h),
                    "outline {i}, style {j}"
                );
            }
        }
    }

    #[test]
    fn the_bleed_lands_by_max() {
        let mut c: Vec<Command> = Vec::new();
        c.move_to([2.0, 2.0])
            .line_to([12.0, 2.0])
            .line_to([12.0, 12.0])
            .close();
        let mut buf = vec![0u8; 16 * 16];
        render(
            &c,
            Style::Fill(Fill::NonZero),
            16,
            16,
            &mut buf,
            Blend::Over,
        );
        let fill = buf.clone();
        let mut stroke = Stroke::new(1.0);
        stroke.join(Join::Round).cap(Cap::Round);
        render(&c, Style::Stroke(stroke), 16, 16, &mut buf, Blend::Max);
        let edge = ours(&c, Style::Stroke(stroke), 16, 16);
        for i in 0..buf.len() {
            assert_eq!(buf[i], fill[i].max(edge[i]));
        }
    }
}
