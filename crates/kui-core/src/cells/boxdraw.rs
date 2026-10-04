//! The glyphs a `cells` node draws from the cell box instead of the font:
//! box drawing (U+2500–U+257F), block elements (U+2580–U+259F) and the
//! Powerline separators (U+E0B0–U+E0BF: the arrows, and the Powerline
//! Extra half circles and wedges).
//!
//! A font's box-drawing glyphs span *its* line box — Iosevka's are 1.25 em
//! tall — and a cell is `line_height` tall, which no font can know, so
//! every `│` a TUI draws through the font is a dash with a gap under it.
//! Every terminal that draws these from the cell (Alacritty, kitty,
//! WezTerm, foot, Ghostty) does what this module does: rasterize the
//! character into an alpha mask of exactly the cell's size, strokes on
//! whole pixels, so adjacent cells' strokes meet with no seam.
//!
//! The geometry is a function of the cell size alone — the light stroke
//! is `max(1, round(cell_w / 8))`, the heavy one three times that, and
//! every stroke sits on the column or row `(cell - stroke) / 2` — so
//! every character in a row lands on the same pixel column and every one
//! in a column on the same pixel row. Bold does not thicken a light line
//! (the set has heavy variants), italic is ignored, and the mask is a
//! plain one the atlas already blends.

/// Whether `ch` is drawn here rather than shaped.
pub(crate) fn draws(ch: char) -> bool {
    matches!(ch as u32, 0x2500..=0x259F | 0xE0B0..=0xE0BF)
}

/// The coverage mask for `ch` in a `w × h` cell, `w * h` bytes, row-major.
pub(crate) fn raster(ch: char, w: u32, h: u32) -> Vec<u8> {
    let mut m = Mask::new(w, h);
    let cp = ch as u32;
    match cp {
        0x2500..=0x254B | 0x2574..=0x257F => lines(&mut m, cp),
        0x254C..=0x254F => dashes(&mut m, cp),
        0x2550..=0x256C => doubles(&mut m, cp),
        0x256D..=0x2570 => arc(&mut m, cp),
        0x2571..=0x2573 => diagonal(&mut m, cp),
        0x2580..=0x259F => block(&mut m, cp),
        0xE0B0..=0xE0B3 => powerline(&mut m, cp),
        0xE0B4..=0xE0B7 => half_circle(&mut m, cp),
        0xE0B8..=0xE0BF => wedge(&mut m, cp),
        _ => {}
    }
    m.a
}

struct Mask {
    w: u32,
    h: u32,
    a: Vec<u8>,
}

/// The strokes of one cell size: light and heavy widths, and where a
/// stroke of each sits so the centreline is the same in every cell.
struct Pen {
    light: u32,
    heavy: u32,
}

impl Mask {
    fn new(w: u32, h: u32) -> Self {
        Self {
            w,
            h,
            a: vec![0; (w * h) as usize],
        }
    }

    fn pen(&self) -> Pen {
        let light = ((self.w as f32 / 8.0).round() as u32).max(1);
        Pen {
            light,
            heavy: light * 3,
        }
    }

    /// The first column of a vertical stroke `t` wide, centred.
    fn col(&self, t: u32) -> u32 {
        self.w.saturating_sub(t) / 2
    }

    /// The first row of a horizontal stroke `t` wide, centred.
    fn row(&self, t: u32) -> u32 {
        self.h.saturating_sub(t) / 2
    }

    /// A whole-pixel rectangle `[x0, x1) × [y0, y1)` at full coverage.
    fn rect(&mut self, x0: u32, y0: u32, x1: u32, y1: u32) {
        self.rect_alpha(x0, y0, x1, y1, 255);
    }

    fn rect_alpha(&mut self, x0: u32, y0: u32, x1: u32, y1: u32, alpha: u8) {
        let (x1, y1) = (x1.min(self.w), y1.min(self.h));
        for y in y0..y1 {
            for x in x0..x1 {
                let i = (y * self.w + x) as usize;
                self.a[i] = self.a[i].max(alpha);
            }
        }
    }

    /// An anti-aliased shape: `inside` over pixel-centre coordinates,
    /// sampled 4 × 4 per pixel and unioned with what is there.
    fn shape(&mut self, inside: impl Fn(f32, f32) -> bool) {
        const N: u32 = 4;
        for y in 0..self.h {
            for x in 0..self.w {
                let mut hits = 0;
                for sy in 0..N {
                    for sx in 0..N {
                        let px = x as f32 + (sx as f32 + 0.5) / N as f32;
                        let py = y as f32 + (sy as f32 + 0.5) / N as f32;
                        if inside(px, py) {
                            hits += 1;
                        }
                    }
                }
                if hits > 0 {
                    let i = (y * self.w + x) as usize;
                    let a = (hits * 255 / (N * N)) as u8;
                    self.a[i] = self.a[i].max(a);
                }
            }
        }
    }
}

/// An arm's weight: none, light or heavy — and, in the double set, none,
/// single or double under the same three names.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Arm {
    None,
    Light,
    Heavy,
}

impl Arm {
    fn from(c: u8) -> Self {
        match c {
            b'l' | b's' => Arm::Light,
            b'h' | b'd' => Arm::Heavy,
            _ => Arm::None,
        }
    }
}

/// The arms of one character as `up, down, left, right`.
#[derive(Clone, Copy)]
struct Arms {
    up: Arm,
    down: Arm,
    left: Arm,
    right: Arm,
}

impl Arms {
    fn parse(s: &[u8]) -> Self {
        Self {
            up: Arm::from(s[0]),
            down: Arm::from(s[1]),
            left: Arm::from(s[2]),
            right: Arm::from(s[3]),
        }
    }
}

/// U+2500–U+254B, four bytes each, `.` none / `l` light / `h` heavy in
/// the order up, down, left, right. The dashed rows (U+2504–U+250B) carry
/// the axis and weight of their solid line; `dashes` reads the count.
const LINES: &[u8; 76 * 4] = b"\
..ll..hhll..hh..\
..ll..hhll..hh..\
..ll..hhll..hh..\
.l.l.l.h.h.l.h.h\
.ll..lh..hl..hh.\
l..ll..hh..lh..h\
l.l.l.h.h.l.h.h.\
ll.lll.hhl.llh.lhh.lhl.hlh.hhh.h\
lll.llh.hll.lhl.hhl.hlh.lhh.hhh.\
.lll.lhl.llh.lhh.hll.hhl.hlh.hhh\
l.lll.hll.lhl.hhh.llh.hlh.lhh.hh\
llllllhllllhllhhhlll\
lhllhhllhlhlhllhlhhllhlhhlhhlhhhhhhlhhlhhhhh";

/// U+2574–U+257F: the half lines and the two-weight lines.
const STUBS: &[u8; 12 * 4] = b"..l.l......l.l....h.h......h.h....lhlh....hlhl..";

/// U+2550–U+256C, `.` none / `s` single / `d` double.
const DOUBLES: &[u8; 29 * 4] = b"\
..dddd...s.d.d.s.d.d.sd..ds..dd.\
s..dd..sd..ds.d.d.s.d.d.\
ss.ddd.sdd.dssd.dds.ddd.\
.sdd.dss.ddds.ddd.ssd.dd\
ssddddssdddd";

fn arms_of(cp: u32) -> Arms {
    let (table, i): (&[u8], usize) = match cp {
        0x2500..=0x254B => (LINES, (cp - 0x2500) as usize),
        0x2574..=0x257F => (STUBS, (cp - 0x2574) as usize),
        0x2550..=0x256C => (DOUBLES, (cp - 0x2550) as usize),
        _ => (b"....", 0),
    };
    Arms::parse(&table[i * 4..i * 4 + 4])
}

fn width(pen: &Pen, arm: Arm) -> u32 {
    match arm {
        Arm::None => 0,
        Arm::Light => pen.light,
        Arm::Heavy => pen.heavy,
    }
}

/// Light and heavy lines, corners, tees and crosses: each arm is a
/// stroke from its edge to the centre, run through the centre by the
/// widest stroke crossing it so the joint is square.
fn lines(m: &mut Mask, cp: u32) {
    let pen = m.pen();
    let arms = arms_of(cp);
    if (0x2504..=0x250B).contains(&cp) {
        // Triple and quadruple dashes of the axis and weight of the
        // solid line two rows up.
        let n = if cp < 0x2508 { 3 } else { 4 };
        let vertical = cp & 2 != 0;
        let heavy = cp & 1 != 0;
        let t = if heavy { pen.heavy } else { pen.light };
        return dashed(m, n, vertical, t);
    }
    let (wu, wd, wl, wr) = (
        width(&pen, arms.up),
        width(&pen, arms.down),
        width(&pen, arms.left),
        width(&pen, arms.right),
    );
    // The band each axis's arms run through at the centre.
    let wv = wu.max(wd);
    let wh = wl.max(wr);
    if wu > 0 {
        let band = wh.max(wu);
        m.rect(m.col(wu), 0, m.col(wu) + wu, m.row(band) + band);
    }
    if wd > 0 {
        let band = wh.max(wd);
        m.rect(m.col(wd), m.row(band), m.col(wd) + wd, m.h);
    }
    if wl > 0 {
        let band = wv.max(wl);
        m.rect(0, m.row(wl), m.col(band) + band, m.row(wl) + wl);
    }
    if wr > 0 {
        let band = wv.max(wr);
        m.rect(m.col(band), m.row(wr), m.w, m.row(wr) + wr);
    }
}

/// `n` dashes along one axis, the gap between them split at the cell's
/// edges so two cells in a row show the same gap as two dashes in one.
fn dashed(m: &mut Mask, n: u32, vertical: bool, t: u32) {
    let len = if vertical { m.h } else { m.w };
    let gap = ((len as f32 / n as f32 * 0.3).round() as u32).max(1);
    // The edges take the gap between them, so the seam between two cells
    // is one gap wide; the rest is `n` dashes with a gap between each.
    let left = gap / 2;
    let span = len.saturating_sub(gap);
    let dash = (span.saturating_sub((n - 1) * gap)) as f32 / n as f32;
    for i in 0..n {
        let at = left as f32 + i as f32 * (dash + gap as f32);
        let s = at.round() as u32;
        let e = ((at + dash).round() as u32).max(s + 1).min(len);
        if vertical {
            m.rect(m.col(t), s, m.col(t) + t, e);
        } else {
            m.rect(s, m.row(t), e, m.row(t) + t);
        }
    }
}

/// U+254C–U+254F: the double dashes.
fn dashes(m: &mut Mask, cp: u32) {
    let pen = m.pen();
    let vertical = cp & 2 != 0;
    let heavy = cp & 1 != 0;
    dashed(m, 2, vertical, if heavy { pen.heavy } else { pen.light });
}

/// U+2550–U+256C: single and double lines. A double arm is two light
/// rails a light stroke apart; where a rail ends depends on what it
/// meets — the near rail of a double on its own side (an inner corner),
/// the far rail of a double on the other side when nothing continues
/// past the centre (an outer corner), the centre otherwise (a line that
/// runs through). A single arm runs to the centre when its opposite
/// continues it, else to the near rail between two doubles (a tee) or
/// the far rail of one (a corner).
fn doubles(m: &mut Mask, cp: u32) {
    let t = m.pen().light;
    let arms = arms_of(cp);
    let gap = t;
    // The two rails of each axis, first pixel of each.
    let (xa, ya) = (m.col(2 * t + gap), m.row(2 * t + gap));
    let (xb, yb) = (xa + t + gap, ya + t + gap);
    let (xc, yc) = (m.col(t), m.row(t));
    let (w, h) = (m.w, m.h);
    // Where an arm from one edge ends, as the first pixel past it toward
    // the centre — `near`/`far` are the rails of the crossing axis,
    // named from the arm's edge.
    let single_end = |opp: Arm, near_side: Arm, far_side: Arm, near: u32, far: u32, centre: u32| {
        if opp != Arm::None {
            centre
        } else if near_side == Arm::Heavy && far_side == Arm::Heavy {
            near
        } else if near_side == Arm::Heavy || far_side == Arm::Heavy {
            far
        } else {
            centre
        }
    };
    let rail_end = |same: Arm, other: Arm, opp: Arm, near: u32, far: u32, centre: u32| {
        if same == Arm::Heavy {
            near
        } else if other == Arm::Heavy && opp == Arm::None {
            far
        } else {
            centre
        }
    };
    // Up arm: from the top edge down. Near rail of the horizontal axis
    // is `ya`, far is `yb`; an end `e` covers `[0, e + t)`.
    match arms.up {
        Arm::None => {}
        Arm::Light => {
            let e = single_end(arms.down, arms.left, arms.right, ya, yb, yc);
            m.rect(xc, 0, xc + t, e + t);
        }
        Arm::Heavy => {
            // Left rail: its own side is the left arm.
            let e = rail_end(arms.left, arms.right, arms.down, ya, yb, yc);
            m.rect(xa, 0, xa + t, e + t);
            let e = rail_end(arms.right, arms.left, arms.down, ya, yb, yc);
            m.rect(xb, 0, xb + t, e + t);
        }
    }
    // Down arm: from the bottom edge up. Near is `yb`, far `ya`; an end
    // `e` covers `[e, h)`.
    match arms.down {
        Arm::None => {}
        Arm::Light => {
            let e = single_end(arms.up, arms.left, arms.right, yb, ya, yc);
            m.rect(xc, e, xc + t, h);
        }
        Arm::Heavy => {
            let e = rail_end(arms.left, arms.right, arms.up, yb, ya, yc);
            m.rect(xa, e, xa + t, h);
            let e = rail_end(arms.right, arms.left, arms.up, yb, ya, yc);
            m.rect(xb, e, xb + t, h);
        }
    }
    // Left arm: from the left edge. Near `xa`, far `xb`; covers `[0, e + t)`.
    match arms.left {
        Arm::None => {}
        Arm::Light => {
            let e = single_end(arms.right, arms.up, arms.down, xa, xb, xc);
            m.rect(0, yc, e + t, yc + t);
        }
        Arm::Heavy => {
            let e = rail_end(arms.up, arms.down, arms.right, xa, xb, xc);
            m.rect(0, ya, e + t, ya + t);
            let e = rail_end(arms.down, arms.up, arms.right, xa, xb, xc);
            m.rect(0, yb, e + t, yb + t);
        }
    }
    // Right arm: from the right edge. Near `xb`, far `xa`; covers `[e, w)`.
    match arms.right {
        Arm::None => {}
        Arm::Light => {
            let e = single_end(arms.left, arms.up, arms.down, xb, xa, xc);
            m.rect(e, yc, w, yc + t);
        }
        Arm::Heavy => {
            let e = rail_end(arms.up, arms.down, arms.left, xb, xa, xc);
            m.rect(e, ya, w, ya + t);
            let e = rail_end(arms.down, arms.up, arms.left, xb, xa, xc);
            m.rect(e, yb, w, yb + t);
        }
    }
}

/// U+256D–U+2570: a quarter arc of the light stroke joining two stubs,
/// radius half the cell's shorter side.
fn arc(m: &mut Mask, cp: u32) {
    let t = m.pen().light;
    let (cx, cy) = (
        m.col(t) as f32 + t as f32 / 2.0,
        m.row(t) as f32 + t as f32 / 2.0,
    );
    let r = m.w.min(m.h) as f32 / 2.0;
    // Which way the two stubs leave: (horizontal sign, vertical sign).
    let (sx, sy) = match cp {
        0x256D => (1.0, 1.0),   // ╭ down and right
        0x256E => (-1.0, 1.0),  // ╮ down and left
        0x256F => (-1.0, -1.0), // ╯ up and left
        _ => (1.0, -1.0),       // ╰ up and right
    };
    // The vertical stub from the edge to where the arc starts, the
    // horizontal one likewise; both on the stroke's own pixels, and each
    // overlapping the arc's end pixel, which the ring covers by half.
    let (col, row) = (m.col(t), m.row(t));
    let ay = cy + sy * r;
    let ax = cx + sx * r;
    if sy > 0.0 {
        m.rect(col, ay.floor() as u32, col + t, m.h);
    } else {
        m.rect(col, 0, col + t, ay.ceil().max(0.0) as u32);
    }
    if sx > 0.0 {
        m.rect(ax.floor() as u32, row, m.w, row + t);
    } else {
        m.rect(0, row, ax.ceil().max(0.0) as u32, row + t);
    }
    // The arc: the quadrant of the ring of radius `r` around `(ax, ay)`
    // that faces the centre.
    let half = t as f32 / 2.0;
    m.shape(|x, y| {
        let (dx, dy) = (x - ax, y - ay);
        let inward = dx * sx <= 0.0 && dy * sy <= 0.0;
        inward && ((dx * dx + dy * dy).sqrt() - r).abs() <= half
    });
}

/// Distance from `(px, py)` to the segment `(x0, y0)–(x1, y1)`.
fn seg_dist(px: f32, py: f32, x0: f32, y0: f32, x1: f32, y1: f32) -> f32 {
    let (dx, dy) = (x1 - x0, y1 - y0);
    let len2 = dx * dx + dy * dy;
    let u = if len2 == 0.0 {
        0.0
    } else {
        (((px - x0) * dx + (py - y0) * dy) / len2).clamp(0.0, 1.0)
    };
    let (qx, qy) = (x0 + u * dx, y0 + u * dy);
    ((px - qx).powi(2) + (py - qy).powi(2)).sqrt()
}

/// U+2571–U+2573: one anti-aliased light line corner to corner, or both.
fn diagonal(m: &mut Mask, cp: u32) {
    let half = m.pen().light as f32 / 2.0;
    let (w, h) = (m.w as f32, m.h as f32);
    if cp != 0x2572 {
        m.shape(|x, y| seg_dist(x, y, w, 0.0, 0.0, h) <= half);
    }
    if cp != 0x2571 {
        m.shape(|x, y| seg_dist(x, y, 0.0, 0.0, w, h) <= half);
    }
}

/// U+2580–U+259F: rectangles over fractions of the cell, edges snapped so
/// `▀` over `▄` fills it and `▐` stacked is one bar; the shades `░▒▓` as
/// flat alpha at 25 / 50 / 75 %.
fn block(m: &mut Mask, cp: u32) {
    let (w, h) = (m.w, m.h);
    let eighth_x = |n: u32| (w as f32 * n as f32 / 8.0).round() as u32;
    let eighth_y = |n: u32| (h as f32 * n as f32 / 8.0).round() as u32;
    let (hx, hy) = (eighth_x(4), eighth_y(4));
    match cp {
        0x2580 => m.rect(0, 0, w, hy),
        // Lower one eighth to the full block.
        0x2581..=0x2588 => m.rect(0, eighth_y(8 - (cp - 0x2580)), w, h),
        // Left seven eighths down to one eighth.
        0x2589..=0x258F => m.rect(0, 0, eighth_x(8 - (cp - 0x2588)), h),
        0x2590 => m.rect(hx, 0, w, h),
        0x2591 => m.rect_alpha(0, 0, w, h, 64),
        0x2592 => m.rect_alpha(0, 0, w, h, 128),
        0x2593 => m.rect_alpha(0, 0, w, h, 192),
        0x2594 => m.rect(0, 0, w, eighth_y(1).max(1)),
        0x2595 => m.rect(w - eighth_x(1).max(1), 0, w, h),
        0x2596..=0x259F => {
            // Quadrants, as bits: upper-left, upper-right, lower-left,
            // lower-right.
            const Q: [u8; 10] = [
                0b0010, // ▖ lower left
                0b0001, // ▗ lower right
                0b1000, // ▘ upper left
                0b1011, // ▙ upper left, lower left, lower right
                0b1001, // ▚ upper left, lower right
                0b1110, // ▛ upper left, upper right, lower left
                0b1101, // ▜ upper left, upper right, lower right
                0b0100, // ▝ upper right
                0b0110, // ▞ upper right, lower left
                0b0111, // ▟ upper right, lower left, lower right
            ];
            let q = Q[(cp - 0x2596) as usize];
            if q & 0b1000 != 0 {
                m.rect(0, 0, hx, hy);
            }
            if q & 0b0100 != 0 {
                m.rect(hx, 0, w, hy);
            }
            if q & 0b0010 != 0 {
                m.rect(0, hy, hx, h);
            }
            if q & 0b0001 != 0 {
                m.rect(hx, hy, w, h);
            }
        }
        _ => {}
    }
}

/// U+E0B0–U+E0B3: the Powerline arrows every prompt draws — a filled
/// triangle pointing right or left, and the chevron of each.
fn powerline(m: &mut Mask, cp: u32) {
    let half = m.pen().light as f32 / 2.0;
    let (w, h) = (m.w as f32, m.h as f32);
    let mid = h / 2.0;
    match cp {
        0xE0B0 => m.shape(|x, y| x <= w * (1.0 - (y - mid).abs() / mid)),
        0xE0B1 => m.shape(|x, y| {
            seg_dist(x, y, 0.0, 0.0, w, mid) <= half || seg_dist(x, y, w, mid, 0.0, h) <= half
        }),
        0xE0B2 => m.shape(|x, y| x >= w * ((y - mid).abs() / mid)),
        _ => m.shape(|x, y| {
            seg_dist(x, y, w, 0.0, 0.0, mid) <= half || seg_dist(x, y, 0.0, mid, w, h) <= half
        }),
    }
}

/// U+E0B4–U+E0B7: the half circles a rounded tab or a rounded row ends
/// on (yazi's hovered row, a starship prompt) — a filled half ellipse on
/// the cell's left edge bulging right, and on the right edge bulging
/// left, the whole cell wide and tall so it meets the run beside it with
/// no seam; and the arc of each. Through the font they were whatever
/// the fallback had, the size of its own line box.
fn half_circle(m: &mut Mask, cp: u32) {
    let t = m.pen().light as f32;
    let (w, h) = (m.w as f32, m.h as f32);
    let mid = h / 2.0;
    // The flat side's x: the left edge for the right-bulging pair.
    let cx = if cp <= 0xE0B5 { 0.0 } else { w };
    let within = |x: f32, y: f32, rx: f32, ry: f32| {
        let (dx, dy) = ((x - cx) / rx, (y - mid) / ry);
        dx * dx + dy * dy <= 1.0
    };
    if cp == 0xE0B4 || cp == 0xE0B6 {
        m.shape(|x, y| within(x, y, w, mid));
    } else {
        m.shape(|x, y| within(x, y, w, mid) && !within(x, y, w - t, mid - t));
    }
}

/// U+E0B8–U+E0BF: the Powerline Extra wedges — a filled right triangle
/// in each corner of the cell, its hypotenuse the cell's diagonal, and
/// that diagonal alone as a line (`╲` for the lower-left and upper-right
/// pairs, `╱` for the others), in the order the Nerd Fonts table has:
/// lower left, lower right, upper left, upper right.
fn wedge(m: &mut Mask, cp: u32) {
    let half = m.pen().light as f32 / 2.0;
    let (w, h) = (m.w as f32, m.h as f32);
    // Each pair's diagonal: `╲` runs (0,0)–(w,h), `╱` runs (0,h)–(w,0).
    let back = matches!(cp, 0xE0B8 | 0xE0B9 | 0xE0BE | 0xE0BF);
    if cp % 2 == 1 {
        if back {
            m.shape(|x, y| seg_dist(x, y, 0.0, 0.0, w, h) <= half);
        } else {
            m.shape(|x, y| seg_dist(x, y, 0.0, h, w, 0.0) <= half);
        }
        return;
    }
    match cp {
        // ◣ under `╲`.
        0xE0B8 => m.shape(|x, y| y * w >= x * h),
        // ◢ under `╱`.
        0xE0BA => m.shape(|x, y| y * w >= (w - x) * h),
        // ◤ over `╱`.
        0xE0BC => m.shape(|x, y| y * w <= (w - x) * h),
        // ◥ over `╲`.
        _ => m.shape(|x, y| y * w <= x * h),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(a: &[u8], w: u32, x: u32, y: u32) -> u8 {
        a[(y * w + x) as usize]
    }

    /// Every table row is four bytes of the alphabet it declares — a
    /// typo in one is a wrong glyph, not a panic.
    #[test]
    fn the_tables_parse() {
        for cp in 0x2500..=0x254B {
            let _ = arms_of(cp);
        }
        for b in LINES.iter().chain(STUBS.iter()) {
            assert!(matches!(b, b'.' | b'l' | b'h'), "{}", *b as char);
        }
        for b in DOUBLES.iter() {
            assert!(matches!(b, b'.' | b's' | b'd'), "{}", *b as char);
        }
        // Spot checks against the code chart.
        let a = arms_of(0x250C); // ┌
        assert!(
            a.up == Arm::None
                && a.down == Arm::Light
                && a.left == Arm::None
                && a.right == Arm::Light
        );
        let a = arms_of(0x254B); // ╋
        assert!(
            [a.up, a.down, a.left, a.right]
                .iter()
                .all(|&x| x == Arm::Heavy)
        );
        let a = arms_of(0x2542); // ╂
        assert!(
            a.up == Arm::Heavy
                && a.down == Arm::Heavy
                && a.left == Arm::Light
                && a.right == Arm::Light
        );
        let a = arms_of(0x257F); // ╿
        assert!(a.up == Arm::Heavy && a.down == Arm::Light && a.left == Arm::None);
        let a = arms_of(0x2552); // ╒
        assert!(a.down == Arm::Light && a.right == Arm::Heavy && a.up == Arm::None);
        let a = arms_of(0x256B); // ╫
        assert!(
            a.up == Arm::Heavy
                && a.down == Arm::Heavy
                && a.left == Arm::Light
                && a.right == Arm::Light
        );
    }

    /// `│` fills its column top to bottom and `─` its row edge to edge,
    /// on the same pixels a corner uses.
    #[test]
    fn lines_run_edge_to_edge_on_shared_pixels() {
        let (w, h) = (7, 20);
        let v = raster('│', w, h);
        let col = (0..w).find(|&x| at(&v, w, x, 0) == 255).expect("a stroke");
        for y in 0..h {
            assert_eq!(at(&v, w, col, y), 255, "row {y}");
        }
        let hz = raster('─', w, h);
        let row = (0..h).find(|&y| at(&hz, w, 0, y) == 255).expect("a stroke");
        for x in 0..w {
            assert_eq!(at(&hz, w, x, row), 255, "col {x}");
        }
        let corner = raster('┌', w, h);
        assert_eq!(
            at(&corner, w, col, h - 1),
            255,
            "the corner's stem is on │'s column"
        );
        assert_eq!(
            at(&corner, w, w - 1, row),
            255,
            "the corner's arm is on ─'s row"
        );
        assert_eq!(at(&corner, w, col, 0), 0, "and nothing above the corner");
        // Heavy is wider and centred on the same column.
        let hv = raster('┃', w, h);
        assert_eq!(at(&hv, w, col, 0), 255);
        assert!((0..w).filter(|&x| at(&hv, w, x, 0) == 255).count() > 1);
    }

    #[test]
    fn blocks_tile() {
        let (w, h) = (7, 20);
        let upper = raster('▀', w, h);
        let lower = raster('▄', w, h);
        for y in 0..h {
            assert_eq!(
                at(&upper, w, 0, y) as u32 + at(&lower, w, 0, y) as u32,
                255,
                "row {y}: ▀ over ▄ is the cell"
            );
        }
        let right = raster('▐', w, h);
        let left = raster('▌', w, h);
        for x in 0..w {
            assert_eq!(at(&right, w, x, 0) as u32 + at(&left, w, x, 0) as u32, 255);
        }
        assert!(raster('█', w, h).iter().all(|&a| a == 255));
        assert!(raster('▒', w, h).iter().all(|&a| a == 128));
        let ul = raster('▘', w, h);
        assert_eq!(at(&ul, w, 0, 0), 255);
        assert_eq!(at(&ul, w, w - 1, h - 1), 0);
    }

    #[test]
    fn doubles_leave_the_gap_and_close_the_corners() {
        let (w, h) = (9, 20);
        let v = raster('║', w, h);
        let cols: Vec<u32> = (0..w).filter(|&x| at(&v, w, x, 0) == 255).collect();
        assert_eq!(cols.len(), 2, "{cols:?}");
        assert_eq!(cols[1] - cols[0], 2, "a light stroke apart: {cols:?}");
        // ╔: the outer corner is closed at the far rail, the inner at the near.
        let c = raster('╔', w, h);
        let hz = raster('═', w, h);
        let rows: Vec<u32> = (0..h).filter(|&y| at(&hz, w, 0, y) == 255).collect();
        assert_eq!(rows.len(), 2);
        assert_eq!(at(&c, w, cols[0], rows[0]), 255, "outer corner");
        assert_eq!(at(&c, w, cols[1], rows[1]), 255, "inner corner");
        assert_eq!(
            at(&c, w, cols[1], rows[0]),
            255,
            "the outer rail runs over the inner"
        );
        assert_eq!(at(&c, w, cols[0], h - 1), 255);
        assert_eq!(at(&c, w, w - 1, rows[1]), 255);
        assert_eq!(at(&c, w, cols[0], 0), 0, "nothing above the outer corner");
        // ╬: a hole in the middle.
        let x = raster('╬', w, h);
        assert_eq!(at(&x, w, cols[0] + 1, rows[0] + 1), 0, "the hole");
        assert_eq!(at(&x, w, cols[0], 0), 255);
        assert_eq!(at(&x, w, 0, rows[1]), 255);
        // ╪: the single runs through.
        let t = raster('╪', w, h);
        let mid = (0..w)
            .find(|&x| at(&raster('│', w, h), w, x, 0) == 255)
            .unwrap();
        for y in 0..h {
            assert_eq!(at(&t, w, mid, y), 255, "row {y}");
        }
    }

    #[test]
    fn arcs_join_their_stubs_and_the_rest_draws_something() {
        let (w, h) = (8, 20);
        let a = raster('╭', w, h);
        let col = (0..w)
            .find(|&x| at(&raster('│', w, h), w, x, 0) == 255)
            .unwrap();
        let row = (0..h)
            .find(|&y| at(&raster('─', w, h), w, 0, y) == 255)
            .unwrap();
        assert_eq!(at(&a, w, col, h - 1), 255, "the stem reaches the bottom");
        assert_eq!(at(&a, w, w - 1, row), 255, "the arm reaches the right edge");
        assert_eq!(at(&a, w, 0, 0), 0);
        for cp in (0x2500..=0x259Fu32).chain(0xE0B0..=0xE0BF) {
            let ch = char::from_u32(cp).unwrap();
            assert!(draws(ch));
            let m = raster(ch, w, h);
            assert!(m.iter().any(|&a| a > 0), "U+{cp:04X} drew nothing");
        }
        assert!(!draws('a') && !draws('é') && !draws('😀'));
    }

    /// F112: a filled half circle is the cell tall at its flat side and
    /// comes to a point at the far edge's middle, so `` beside a
    /// highlighted run and `` after it round it off with no seam; its
    /// arc is empty inside; a wedge fills its corner and leaves the other.
    #[test]
    fn half_circles_and_wedges_meet_the_run_beside_them() {
        let (w, h) = (8, 20);
        let right = raster('\u{E0B4}', w, h);
        let left = raster('\u{E0B6}', w, h);
        for y in 2..h - 2 {
            assert_eq!(at(&right, w, 0, y), 255, "E0B4 flat side, row {y}");
            assert_eq!(at(&left, w, w - 1, y), 255, "E0B6 flat side, row {y}");
        }
        assert_eq!(
            at(&right, w, w - 1, h / 2),
            255,
            "E0B4 reaches the far edge"
        );
        assert_eq!(at(&right, w, w - 1, 0), 0, "and rounds the corners off");
        assert_eq!(at(&left, w, 0, 0), 0);
        for y in 0..h {
            for x in 0..w {
                assert_eq!(
                    at(&right, w, x, y),
                    at(&left, w, w - 1 - x, y),
                    "E0B6 mirrors E0B4 at ({x}, {y})"
                );
            }
        }
        let arc = raster('\u{E0B5}', w, h);
        assert_eq!(at(&arc, w, 0, h / 2), 0, "the arc is hollow");
        assert_eq!(at(&arc, w, w - 1, h / 2), 255, "and is drawn at its apex");
        let ll = raster('\u{E0B8}', w, h);
        assert_eq!(at(&ll, w, 0, h - 1), 255, "◣ fills the lower left");
        assert_eq!(at(&ll, w, w - 1, 0), 0, "and not the upper right");
        let ur = raster('\u{E0BE}', w, h);
        assert_eq!(at(&ur, w, w - 1, 0), 255, "◥ fills the upper right");
        assert_eq!(at(&ur, w, 0, h - 1), 0);
        for y in 0..h {
            for x in 0..w {
                let (a, b) = (at(&ll, w, x, y) as u32, at(&ur, w, x, y) as u32);
                assert!(
                    a + b >= 250 && a + b <= 260,
                    "◣ and ◥ tile at ({x}, {y}): {a} + {b}"
                );
            }
        }
        let slash = raster('\u{E0BB}', w, h);
        assert!(at(&slash, w, 0, h - 1) > 128, "╱ from the lower left");
        assert_eq!(at(&slash, w, 0, 0), 0);
    }

    /// The dash pattern is continuous across cells: the gap at the seam is
    /// the same as the gap inside.
    #[test]
    fn dashes_split_their_gap_at_the_edges() {
        let (w, h) = (16, 20);
        let d = raster('┄', w, h);
        let row = (0..h)
            .find(|&y| d[(y * w) as usize..((y + 1) * w) as usize].contains(&255))
            .unwrap();
        let line: Vec<bool> = (0..w).map(|x| at(&d, w, x, row) == 255).collect();
        assert!(
            !line[0] && !line[w as usize - 1],
            "gaps at both edges: {line:?}"
        );
        assert_eq!(
            line.windows(2).filter(|p| !p[0] && p[1]).count(),
            3,
            "three dashes: {line:?}"
        );
    }
}
