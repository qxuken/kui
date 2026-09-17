//! Clay-style flex layout over the flat tree, five passes:
//!
//! 1. fit widths      (reverse  = children before parents)
//! 2. grow widths     (forward  = parents before children)
//! 3. fit heights     (reverse; text wraps at its final width here)
//! 4. grow heights    (forward)
//! 5. positions       (forward)
//!
//! A wrapping row breaks its children into lines in pass 2 and every later
//! pass reads that grouping (`Tree::line`); the order is why wrapping is
//! rows-only, and [`wraps`] says so at length.
//!
//! A sixth pass, [`anchored`], runs only on a frame with a float anchored
//! to a node by key (`FloatAnchor::Node`): the five passes above cannot
//! size such a float, since its anchor may come later in preorder, so
//! its subtree is laid out again from the anchor's final rect.
//!
//! A table (`LayoutSpec::table`, ADR 0033) is a column whose rows' cells
//! line up: pass 1 reaches the table after its rows and cells and sets
//! every fit cell to its column's widest, so the rows' and the table's
//! fit widths are the aligned ones; pass 2 reaches it before its rows,
//! resolves the columns against the widest row once ([`table_columns`])
//! and writes each column's width into its cells, and a row of a table
//! then leaves its children alone — the widths are final, and the row is
//! never shrunk or grown cell by cell. A bare text cell keeps its column's
//! width through pass 3 (`fit_heights`), where a text elsewhere shrinks
//! to what it shaped.
//!
//! Text measurement goes through `TextMeasure` so the solver is testable with
//! a deterministic stub and never depends on system fonts.

use crate::geom::{Rect, Size, Vec2};
use crate::scroll::ScrollStore;
use crate::spec::{Align, Dir, FloatAnchor, Min, Sizing};
use crate::tree::{NIL, NodeContent, Tree};

/// Gated on the tree-level flag first: on a frame with no floats this is
/// one predicted branch rather than a read through every child's spec.
#[inline]
fn is_float(tree: &Tree, i: u32) -> bool {
    tree.any_float && tree.specs[i as usize].layout.float.is_some()
}

fn align_factor(a: Align) -> f32 {
    match a {
        Align::Start => 0.0,
        Align::Center => 0.5,
        Align::End => 1.0,
    }
}

fn mirror(a: Align) -> Align {
    match a {
        Align::Start => Align::End,
        Align::Center => Align::Center,
        Align::End => Align::Start,
    }
}

/// One axis of float attachment: anchor point minus self point, plus offset.
fn attach(
    anchor_pos: f32,
    anchor_len: f32,
    self_len: f32,
    anchor_pt: Align,
    self_pt: Align,
    off: f32,
) -> f32 {
    anchor_pos + align_factor(anchor_pt) * anchor_len - align_factor(self_pt) * self_len + off
}

/// How far `[pos, pos+len]` sticks out of `[0, limit]`.
fn overflow(pos: f32, len: f32, limit: f32) -> f32 {
    (-pos).max(0.0) + (pos + len - limit).max(0.0)
}

/// Whether `i` breaks its children into wrap lines.
///
/// Rows only, and never on a main axis that scrolls. Breaking needs a
/// definite main size to break against, and the pass order hands a row one
/// — its width is final in pass 2, before its height is measured in pass 3,
/// so a line's cross extent is known by the time anything needs it. A
/// column is the mirror image and does not work: its main size is not final
/// until pass 4, two passes after the cross-axis fit that would have to sum
/// the lines. `scroll_x` says the same thing a different way — an axis that
/// scrolls is unbounded, and an unbounded axis has nothing to break
/// against. `diag::WRAP_IGNORED` reports both.
#[inline]
fn wraps(tree: &Tree, i: u32) -> bool {
    if !tree.any_wrap {
        return false;
    }
    let s = &tree.specs[i as usize].layout;
    s.wrap && s.dir == Dir::Row && !s.scroll_x && !is_table_row(tree, i)
}

/// Whether `i` is a row of a table: an in-flow container child of a
/// node with `LayoutSpec::table`. A leaf straight under the table — a
/// heading text beside the rows — has no cells and is left alone. Gated
/// on the tree-level flag first, as [`is_float`] is.
#[inline]
fn is_table_row(tree: &Tree, i: u32) -> bool {
    if !tree.any_table {
        return false;
    }
    let p = tree.parent[i as usize];
    p != NIL
        && tree.specs[p as usize].layout.table
        && matches!(tree.content[i as usize], NodeContent::Container)
        && !is_float(tree, i)
}

/// Whether `i` is a cell of a table: an in-flow child of a table row.
#[inline]
fn is_table_cell(tree: &Tree, i: u32) -> bool {
    if !tree.any_table {
        return false;
    }
    let p = tree.parent[i as usize];
    p != NIL && is_table_row(tree, p) && !is_float(tree, i)
}

/// One column of a table, as its cells declared it (ADR 0033).
#[derive(Clone, Copy, Debug, Default)]
struct Col {
    /// The widest cell's fitted width: a `Fixed` cell's px, a `Fit`
    /// cell's content, a `Grow` or `Percent` cell's floor.
    fit: f32,
    /// The largest `Grow` factor among the cells, 0 for none: the column
    /// grows with the table when any cell asked to.
    grow: f32,
    /// The largest `Percent` among the cells, 0 for none; read only when
    /// nothing grows.
    pct: f32,
    /// Whether any cell is `Fixed`: a fixed column is never shrunk.
    fixed: bool,
    /// The strictest clamps its cells declared: the largest floor and
    /// the smallest ceiling.
    min: f32,
    max: f32,
    /// The width resolved for it.
    w: f32,
}

impl Col {
    fn clamp(&self, w: f32) -> f32 {
        w.clamp(self.min, self.max.max(self.min))
    }
}

/// The columns of table `i`, read off its cells' current widths and
/// specs: the nth in-flow child of each in-flow row is a cell of column
/// n, and a row with fewer cells fills the first columns. A text cell
/// has no spec sizing and reads as `Fit`.
fn table_columns(tree: &Tree, i: u32) -> Vec<Col> {
    let mut cols: Vec<Col> = Vec::new();
    for row in tree.children(i) {
        if !is_table_row(tree, row) {
            continue;
        }
        let mut j = 0usize;
        for cell in tree.children(row) {
            if is_float(tree, cell) {
                continue;
            }
            if j == cols.len() {
                cols.push(Col {
                    max: f32::INFINITY,
                    ..Col::default()
                });
            }
            let col = &mut cols[j];
            let spec = tree.specs[cell as usize].layout;
            col.fit = col.fit.max(tree.size[cell as usize].w);
            match child_sizing(tree, cell, AxisSel::Width) {
                Sizing::Grow(f) => col.grow = col.grow.max(f.max(0.0)),
                Sizing::Percent(p) => col.pct = col.pct.max(p),
                Sizing::Fixed(_) => col.fixed = true,
                Sizing::Fit => {}
            }
            if !matches!(tree.content[cell as usize], NodeContent::Text(_)) {
                col.min = col.min.max(spec.min_w.resolved());
                col.max = col.max.min(spec.max_w);
            }
            j += 1;
        }
    }
    cols
}

/// The width row `row` needs for `cols` — its cells at the columns'
/// widths, the gaps between them and its padding — which is what a
/// `Fit` row of a table is.
fn table_row_fit(tree: &Tree, row: u32, cols: &[Col]) -> f32 {
    let spec = tree.specs[row as usize].layout;
    let mut w = 0.0f32;
    let mut n = 0u32;
    for cell in tree.children(row) {
        if is_float(tree, cell) {
            continue;
        }
        w += cols.get(n as usize).map_or(0.0, |c| c.w);
        n += 1;
    }
    if n > 1 {
        w += spec.gap * (n - 1) as f32;
    }
    w + spec.padding.x()
}

/// Writes each column's width into its cells and sizes the `Fit` rows to
/// them. A text cell takes the width too — `fit_heights` keeps it.
fn table_apply(tree: &mut Tree, i: u32, cols: &[Col]) {
    let mut row = tree.first_child[i as usize];
    while row != NIL {
        if is_table_row(tree, row) {
            let mut j = 0usize;
            let mut cell = tree.first_child[row as usize];
            while cell != NIL {
                if !is_float(tree, cell) {
                    tree.size[cell as usize].w = cols[j].w;
                    j += 1;
                }
                cell = tree.next_sibling[cell as usize];
            }
            if tree.specs[row as usize].layout.width == Sizing::Fit {
                let fit = table_row_fit(tree, row, cols);
                tree.size[row as usize].w = tree.specs[row as usize].layout.clamp_w(fit);
            }
        }
        row = tree.next_sibling[row as usize];
    }
}

/// Pass 1's table step: every column at its fit — the widest cell — and
/// the rows fitted to that, so the table's own fit width (read next, by
/// the caller) is the aligned one. A growing column sits at its floor
/// here, as a grow child of any row does.
fn table_fit(tree: &mut Tree, i: u32) {
    let mut cols = table_columns(tree, i);
    for col in &mut cols {
        col.w = col.clamp(col.fit);
    }
    table_apply(tree, i, &cols);
}

/// Pass 2's table step, at the table node, before its rows: the columns
/// resolved once against the widest row's content — `Percent` columns
/// take their cut, `Fixed` and `Fit` ones sit at their fit, `Grow`
/// columns split what is left in the freeze loop `distribute_run` runs
/// (a clamped column is frozen and the rest re-share) — and, when the
/// fits alone overflow the row and the table does not scroll x, the
/// `Fit` columns compressed toward their floors largest first, as
/// `shrink_axis` compresses a row's children. Then written into every
/// cell, so the rows have nothing left to distribute.
fn table_resolve(tree: &mut Tree, i: u32) {
    let mut cols = table_columns(tree, i);
    if cols.is_empty() {
        return;
    }
    // The widest row's content: what the columns are laid across. Rows
    // are usually `grow`, and then this is the table's content box less
    // the row's own padding and gaps.
    let mut avail = 0.0f32;
    for row in tree.children(i) {
        if !is_table_row(tree, row) {
            continue;
        }
        let spec = tree.specs[row as usize].layout;
        let chrome = spec.padding.x() + spec.gap * (cols.len() as f32 - 1.0);
        avail = avail.max(tree.size[row as usize].w - chrome);
    }
    let avail = avail.max(0.0);
    let mut used = 0.0f32;
    let mut grow_total = 0.0f32;
    for col in &mut cols {
        if col.grow > 0.0 {
            grow_total += col.grow;
            col.w = col.clamp(0.0);
        } else if col.pct > 0.0 {
            col.w = col.clamp(avail * col.pct);
            used += col.w;
        } else {
            col.w = col.clamp(col.fit);
            used += col.w;
        }
    }
    if grow_total > 0.0 {
        let mut frozen = vec![false; cols.len()];
        loop {
            let remain = (avail - used).max(0.0);
            let mut froze = false;
            for (j, col) in cols.iter_mut().enumerate() {
                if col.grow <= 0.0 || frozen[j] {
                    continue;
                }
                let share = remain * col.grow / grow_total;
                col.w = col.clamp(share);
                if (col.w - share).abs() > 0.01 {
                    frozen[j] = true;
                    used += col.w;
                    grow_total -= col.grow;
                    froze = true;
                }
            }
            if !froze || grow_total <= 0.0 {
                break;
            }
        }
    }
    // The shrink: the fit columns pay the overflow, largest first, down
    // to their floors — never a fixed, a percent or a growing one.
    let total: f32 = cols.iter().map(|c| c.w).sum();
    let mut deficit = total - avail;
    if deficit > 0.5 && !tree.specs[i as usize].layout.scroll_x {
        let shrinkable = |c: &Col| !c.fixed && c.grow <= 0.0 && c.pct <= 0.0;
        let mut guard = 0;
        while deficit > 0.5 && guard < 128 {
            guard += 1;
            let mut largest = f32::NEG_INFINITY;
            let mut second = 0.0f32;
            let mut count = 0u32;
            for c in cols.iter().filter(|c| shrinkable(c) && c.w > c.min + 0.01) {
                if c.w > largest + 0.01 {
                    second = if largest.is_finite() {
                        largest.max(second)
                    } else {
                        second
                    };
                    largest = c.w;
                    count = 1;
                } else if c.w > largest - 0.01 {
                    count += 1;
                } else if c.w > second {
                    second = c.w;
                }
            }
            if count == 0 {
                break;
            }
            let target = (largest - deficit / count as f32).max(second).max(0.0);
            let mut shrunk_any = false;
            for c in cols.iter_mut().filter(|c| shrinkable(c)) {
                if c.w > largest - 0.01 {
                    let new = target.max(c.min);
                    if new < c.w {
                        deficit -= c.w - new;
                        c.w = new;
                        shrunk_any = true;
                    }
                }
            }
            if !shrunk_any {
                break;
            }
        }
    }
    table_apply(tree, i, &cols);
}

/// The first in-flow child of `i` (`NIL` when it has none).
fn first_in_flow(tree: &Tree, i: u32) -> u32 {
    let mut c = tree.first_child[i as usize];
    while c != NIL && is_float(tree, c) {
        c = tree.next_sibling[c as usize];
    }
    c
}

/// The first in-flow child after `c`'s wrap line (`NIL` at the end). Line
/// numbers only ever go up in child order, so one line's in-flow children
/// are a contiguous sibling run and `[c, line_end(c))` is the whole line.
fn line_end(tree: &Tree, c: u32) -> u32 {
    let l = tree.line[c as usize];
    let mut n = tree.next_sibling[c as usize];
    while n != NIL && (is_float(tree, n) || tree.line[n as usize] == l) {
        n = tree.next_sibling[n as usize];
    }
    n
}

/// Extents of the line `[c, end)`, in one walk: `(main, cross)`.
///
/// Main is its children plus the gaps between them. Cross is its tallest
/// child that has a cross size of its own — Grow and Percent children are
/// skipped by *sizing*, not by their current number, because they are
/// sized against the extent this returns and reading them back would make
/// a line's height depend on whether pass 4 had run yet. Skipping them
/// measures the same thing in every pass, and matches an unwrapped row,
/// where a grow child contributes nothing to a fit height either.
/// Wrapping is rows-only, so main is width and cross is height.
fn line_extents(tree: &Tree, c: u32, end: u32, gap: f32) -> (f32, f32) {
    let mut main = 0.0f32;
    let mut cross = 0.0f32;
    let mut n = 0u32;
    let mut k = c;
    while k != end && k != NIL {
        if !is_float(tree, k) {
            let size = tree.size[k as usize];
            main += size.w;
            if !matches!(
                child_sizing(tree, k, AxisSel::Height),
                Sizing::Grow(_) | Sizing::Percent(_)
            ) {
                cross = cross.max(size.h);
            }
            n += 1;
        }
        k = tree.next_sibling[k as usize];
    }
    if n > 1 {
        main += gap * (n - 1) as f32;
    }
    (main, cross)
}

/// A wrapping row's lines in one walk: how many, their stacked cross extent
/// with the cross gaps between them, and the widest line's main extent.
fn wrap_measure(tree: &Tree, i: u32) -> (u32, f32, f32) {
    let spec = tree.specs[i as usize].layout;
    let mut lines = 0u32;
    let mut stacked = 0.0f32;
    let mut widest = 0.0f32;
    let mut c = first_in_flow(tree, i);
    while c != NIL {
        let end = line_end(tree, c);
        let (main, cross) = line_extents(tree, c, end, spec.gap);
        stacked += cross;
        widest = widest.max(main);
        lines += 1;
        c = end;
    }
    if lines > 1 {
        stacked += spec.cross_gap * (lines - 1) as f32;
    }
    (lines, stacked, widest)
}

/// Cross space every line gains beyond its content extent: the container's
/// leftover, shared equally — CSS's `align-content: stretch`, and the
/// reason a wrapping row that happens to fit on one line lays out exactly
/// like an unwrapped one. Zero for a Fit height, whose lines already fill
/// it by construction, and zero when the lines overflow.
fn line_stretch(lines: u32, stacked: f32, cross_content: f32) -> f32 {
    if lines == 0 {
        0.0
    } else {
        (cross_content - stacked).max(0.0) / lines as f32
    }
}

/// Greedy main-axis line breaking, in child order: a child that no longer
/// fits the content box starts the next line, and a child too wide to fit
/// on its own gets a line to itself (and is then the shrink pass's
/// problem). Grow children break on whatever pass 1 left them — zero, or
/// their `min_w` — since a grow child has no size of its own until a line
/// is chosen for it; it then fills what is left of the line it landed on.
fn break_lines(tree: &mut Tree, i: u32, content: f32, gap: f32) {
    let mut line = 0u32;
    let mut used = 0.0f32;
    let mut n = 0u32;
    let mut c = tree.first_child[i as usize];
    while c != NIL {
        if !is_float(tree, c) {
            let base = tree.size[c as usize].w;
            let needed = if n > 0 { gap + base } else { base };
            if n > 0 && used + needed > content + 0.01 {
                line += 1;
                used = base;
                n = 1;
            } else {
                used += needed;
                n += 1;
            }
            tree.line[c as usize] = line;
        }
        c = tree.next_sibling[c as usize];
    }
}

pub trait TextMeasure {
    /// Unwrapped preferred size.
    fn intrinsic(&mut self, id: crate::tree::TextId) -> Size;
    /// Size when wrapped to `max_w` logical pixels.
    fn wrapped(&mut self, id: crate::tree::TextId, max_w: f32) -> Size;
    /// Unwrapped content size of an editable text node.
    fn edit_intrinsic(&mut self, _key: crate::key::Key) -> Size {
        Size::ZERO
    }
    /// Content size of an editable text node wrapped to `max_w`.
    fn edit_wrapped(&mut self, _key: crate::key::Key, _max_w: f32) -> Size {
        Size::ZERO
    }
    /// Pixel dimensions of a registered image (ZERO when unknown).
    fn image_size(&mut self, _id: crate::resources::ImageId) -> Size {
        Size::ZERO
    }
    /// The laid-out size of a cell grid (`rows × cols` cells).
    fn cells_size(&mut self, _id: crate::cells::CellsId) -> Size {
        Size::ZERO
    }
}

pub fn compute(
    tree: &mut Tree,
    text: &mut dyn TextMeasure,
    scroll: &mut ScrollStore,
    viewport: Size,
    // Physical pixels per logical one: `positions` snaps a scroll offset to them.
    scale: f32,
) {
    if tree.is_empty() {
        return;
    }
    // A `Min::FIT` floor is written back into the spec as the number it
    // resolved to (`fit_widths`, `fit_heights`), and a node-anchored
    // float is laid out again in the sixth pass: it would read the
    // number the first run left — measured before the float had a
    // width, a paragraph folded into a column of one word — as a
    // declared floor. Remembered here, put back before the re-run.
    let floors: Vec<(usize, bool, bool)> = if tree.any_node_float {
        (0..tree.len())
            .filter_map(|i| {
                let l = &tree.specs[i].layout;
                let (w, h) = (l.min_w.is_fit(), l.min_h.is_fit());
                (w || h).then_some((i, w, h))
            })
            .collect()
    } else {
        Vec::new()
    };
    fit_widths(tree, text, 0..tree.len());
    grow_widths(tree, viewport);
    fit_heights(tree, text, 0..tree.len());
    grow_heights(tree, viewport);
    positions(tree, scroll, viewport, scale, 0..tree.len());
    if tree.any_node_float {
        anchored(tree, text, scroll, viewport, scale, &floors);
    }
}

/// The sixth pass: every float anchored to a node by key, laid out again
/// against that node's final rect. A subtree is a contiguous index range
/// in preorder — every descendant's parent index is at or after the
/// root's — so the five passes run over the range alone: the float's own
/// size is resolved against the anchor the way a `Parent` float's is
/// against its parent, and the rest is what the passes always do. An
/// anchor the frame does not have leaves the float at zero size, which
/// paints nothing and takes no input; a caller that built the content
/// without its body has nothing to show it in.
fn anchored(
    tree: &mut Tree,
    text: &mut dyn TextMeasure,
    scroll: &mut ScrollStore,
    viewport: Size,
    scale: f32,
    floors: &[(usize, bool, bool)],
) {
    for (c, end, key) in node_floats(tree) {
        let Some(a) = tree.index_of(key) else {
            tree.size[c] = Size::default();
            continue;
        };
        let anchor = Rect::from_pos_size(tree.pos[a], tree.size[a]);
        // The fit floors declared in this subtree, as declared again.
        for &(i, w, h) in floors.iter().filter(|(i, ..)| (c..end).contains(i)) {
            if w {
                tree.specs[i].layout.min_w = Min::FIT;
            }
            if h {
                tree.specs[i].layout.min_h = Min::FIT;
            }
        }
        let spec = tree.specs[c].layout;
        fit_widths(tree, text, c..end);
        tree.size[c].w = spec.clamp_w(match spec.width {
            Sizing::Grow(_) => anchor.w,
            Sizing::Percent(p) => anchor.w * p,
            _ => tree.size[c].w,
        });
        for i in c..end {
            distribute_axis(tree, i as u32, AxisSel::Width, viewport);
        }
        fit_heights(tree, text, c..end);
        tree.size[c].h = spec.clamp_h(match spec.height {
            Sizing::Grow(_) => anchor.h,
            Sizing::Percent(p) => anchor.h * p,
            _ => tree.size[c].h,
        });
        for i in c..end {
            distribute_axis(tree, i as u32, AxisSel::Height, viewport);
        }
        place_anchored(tree, c, anchor);
        positions(tree, scroll, viewport, scale, c..end);
    }
}

/// `(root, end, anchor key)` of every node-anchored float, in tree
/// order, each subtree the index range `root..end`.
fn node_floats(tree: &Tree) -> Vec<(usize, usize, crate::key::Key)> {
    let mut out = Vec::new();
    let mut c = 0usize;
    while c < tree.len() {
        if let Some(crate::spec::FloatConfig {
            anchor: FloatAnchor::Node(key),
            ..
        }) = tree.specs[c].layout.float
        {
            let mut end = c + 1;
            while end < tree.len() && (tree.parent[end] as usize) >= c {
                end += 1;
            }
            out.push((c, end, key));
            c = end;
        } else {
            c += 1;
        }
    }
    out
}

/// Attaches the sized float `c` to `anchor`, its config's points.
fn place_anchored(tree: &mut Tree, c: usize, anchor: Rect) {
    let Some(cfg) = tree.specs[c].layout.float else {
        return;
    };
    let cs = tree.size[c];
    tree.pos[c] = Vec2::new(
        attach(
            anchor.x,
            anchor.w,
            cs.w,
            cfg.anchor_point.0,
            cfg.self_point.0,
            cfg.offset.x,
        ),
        attach(
            anchor.y,
            anchor.h,
            cs.h,
            cfg.anchor_point.1,
            cfg.self_point.1,
            cfg.offset.y,
        ),
    );
}

/// Pass 5 again, with the sizes kept: what a scroll that moved a node
/// re-runs. The node-anchored floats follow their anchors.
pub(crate) fn reposition(tree: &mut Tree, scroll: &mut ScrollStore, viewport: Size, scale: f32) {
    positions(tree, scroll, viewport, scale, 0..tree.len());
    if tree.any_node_float {
        for (c, end, key) in node_floats(tree) {
            if let Some(a) = tree.index_of(key) {
                let anchor = Rect::from_pos_size(tree.pos[a], tree.size[a]);
                place_anchored(tree, c, anchor);
                positions(tree, scroll, viewport, scale, c..end);
            }
        }
    }
}

/// The fit width of `i`, a non-text node: what its content wants on its
/// own. Read for a `Fit` width, and for a `Min::FIT` floor under any other
/// sizing — a `Grow` tab that must never be narrower than its label.
#[inline(always)]
fn fit_width(tree: &Tree, i: usize, text: &mut dyn TextMeasure) -> f32 {
    let spec = &tree.specs[i].layout;
    match tree.content[i] {
        NodeContent::Edit(key) => text.edit_intrinsic(key).w + spec.padding.x(),
        // Image pixels as logical px (1:1 at scale 1).
        NodeContent::Image(id, _) => text.image_size(id).w,
        NodeContent::Cells(id) => text.cells_size(id).w + spec.padding.x(),
        _ => {
            let mut w = 0.0f32;
            let mut n = 0u32;
            for c in tree.children(i as u32) {
                if is_float(tree, c) {
                    continue;
                }
                let cw = tree.size[c as usize].w;
                if spec.dir == Dir::Row {
                    w += cw;
                } else {
                    w = w.max(cw);
                }
                n += 1;
            }
            if spec.dir == Dir::Row && n > 1 {
                w += spec.gap * (n - 1) as f32;
            }
            w + spec.padding.x()
        }
    }
}

fn fit_widths(tree: &mut Tree, text: &mut dyn TextMeasure, range: std::ops::Range<usize>) {
    for i in range.rev() {
        if let NodeContent::Text(tid) = tree.content[i] {
            tree.size[i].w = text.intrinsic(tid).w;
            continue;
        }
        // A table's rows and cells are fitted by now (children first):
        // align them, so the fit read next is the aligned one.
        if tree.any_table && tree.specs[i].layout.table {
            table_fit(tree, i as u32);
        }
        let width = tree.specs[i].layout.width;
        let min_fit = tree.specs[i].layout.min_w.is_fit();
        // Measured once for both uses: the Fit sizing, and the Fit floor.
        let fit = if min_fit || width == Sizing::Fit {
            fit_width(tree, i, text)
        } else {
            0.0
        };
        // A `Min::FIT` floor resolves here, once, to the number every later
        // clamp on this axis reads — written back into the spec so
        // `set_axis_clamped` in the grow pass and `break_lines` need no
        // second form.
        if min_fit {
            tree.specs[i].layout.min_w = Min::px(fit);
        }
        tree.size[i].w = tree.specs[i].layout.clamp_w(match width {
            Sizing::Fixed(px) => px,
            // Resolved against the parent later; contributes nothing to fit
            // beyond its own floor.
            Sizing::Grow(_) | Sizing::Percent(_) => 0.0,
            Sizing::Fit => fit,
        });
    }
}

/// The fit height of `i`, a non-text node, against its final width:
/// `fit_width`'s mirror, read for a `Fit` height and for a `Min::FIT`
/// floor. An editor's is its wrapped extent, which the caller has already
/// measured (emission and input must share one line layout, whatever the
/// sizing) and passes in as `edit`.
#[inline(always)]
fn fit_height(tree: &Tree, i: usize, text: &mut dyn TextMeasure, edit: Size) -> f32 {
    let spec = &tree.specs[i].layout;
    match tree.content[i] {
        NodeContent::Edit(_) => edit.h + spec.padding.y(),
        NodeContent::Cells(id) => text.cells_size(id).h + spec.padding.y(),
        // Width is final by now: a Fit height preserves the aspect.
        NodeContent::Image(id, _) => {
            let intrinsic = text.image_size(id);
            if intrinsic.w > 0.0 {
                intrinsic.h * tree.size[i].w / intrinsic.w
            } else {
                0.0
            }
        }
        // A wrapping row is as tall as its lines stacked: the lines were
        // chosen in pass 2, against a width that is already final.
        _ if wraps(tree, i as u32) => wrap_measure(tree, i as u32).1 + spec.padding.y(),
        _ => {
            let mut h = 0.0f32;
            let mut n = 0u32;
            for c in tree.children(i as u32) {
                if is_float(tree, c) {
                    continue;
                }
                let ch = tree.size[c as usize].h;
                if spec.dir == Dir::Column {
                    h += ch;
                } else {
                    h = h.max(ch);
                }
                n += 1;
            }
            if spec.dir == Dir::Column && n > 1 {
                h += spec.gap * (n - 1) as f32;
            }
            h + spec.padding.y()
        }
    }
}

fn fit_heights(tree: &mut Tree, text: &mut dyn TextMeasure, range: std::ops::Range<usize>) {
    for i in range.rev() {
        if let NodeContent::Text(tid) = tree.content[i] {
            // Width is final by now: wrap to it.
            let wrapped = text.wrapped(tid, tree.size[i].w.max(0.0));
            // The wrapped measurement is authoritative for both axes (a long
            // unbroken word may still exceed the clamp; report it truthfully)
            // — except that a text which is a table's cell keeps the column
            // width pass 2 gave it, or the cells after it would close up.
            if is_table_cell(tree, i as u32) {
                tree.size[i].h = wrapped.h;
                tree.size[i].w = tree.size[i].w.max(wrapped.w);
            } else {
                tree.size[i] = wrapped;
            }
            continue;
        }
        // Always wrap an editor to the final content width so emission and
        // input hit the same line layout, whatever the height sizing is.
        let edit = if let NodeContent::Edit(key) = tree.content[i] {
            let inner = (tree.size[i].w - tree.specs[i].layout.padding.x()).max(0.0);
            text.edit_wrapped(key, inner)
        } else {
            Size::default()
        };
        let height = tree.specs[i].layout.height;
        let min_fit = tree.specs[i].layout.min_h.is_fit();
        // Same resolution as `fit_widths`: measured once, the floor written
        // back as a number.
        let fit = if min_fit || height == Sizing::Fit {
            fit_height(tree, i, text, edit)
        } else {
            0.0
        };
        if min_fit {
            tree.specs[i].layout.min_h = Min::px(fit);
        }
        tree.size[i].h = tree.specs[i].layout.clamp_h(match height {
            Sizing::Fixed(px) => px,
            Sizing::Grow(_) | Sizing::Percent(_) => 0.0,
            Sizing::Fit => fit,
        });
    }
}

fn grow_widths(tree: &mut Tree, viewport: Size) {
    for i in 0..tree.len() {
        if tree.parent[i] == NIL {
            let spec = tree.specs[i].layout;
            tree.size[i].w = spec.clamp_w(resolve_root(spec.width, tree.size[i].w, viewport.w));
        }
        distribute_axis(tree, i as u32, AxisSel::Width, viewport);
    }
}

fn grow_heights(tree: &mut Tree, viewport: Size) {
    for i in 0..tree.len() {
        if tree.parent[i] == NIL {
            let spec = tree.specs[i].layout;
            tree.size[i].h = spec.clamp_h(resolve_root(spec.height, tree.size[i].h, viewport.h));
        }
        distribute_axis(tree, i as u32, AxisSel::Height, viewport);
    }
}

fn resolve_root(sizing: Sizing, fitted: f32, viewport: f32) -> f32 {
    match sizing {
        Sizing::Grow(_) => viewport,
        Sizing::Percent(p) => viewport * p,
        Sizing::Fixed(px) => px,
        Sizing::Fit => fitted,
    }
}

#[derive(Clone, Copy, PartialEq)]
enum AxisSel {
    Width,
    Height,
}

/// Resolves Grow/Percent children of `i` along the given axis, assuming `i`'s
/// own size on that axis is final.
fn distribute_axis(tree: &mut Tree, i: u32, axis: AxisSel, viewport: Size) {
    let spec = tree.specs[i as usize].layout;
    let (own, pad) = match axis {
        AxisSel::Width => (tree.size[i as usize].w, spec.padding.x()),
        AxisSel::Height => (tree.size[i as usize].h, spec.padding.y()),
    };
    let content = (own - pad).max(0.0);
    let is_main = (spec.dir == Dir::Row) == (axis == AxisSel::Width);

    if is_main && axis == AxisSel::Width && is_table_row(tree, i) {
        // The cells were sized by the table (`table_resolve`), the same
        // in every row: nothing to grow, cut or shrink here.
    } else if is_main {
        // Percent takes its cut of the content box first: a wrap line
        // breaks on sizes that are already resolved against the container,
        // not against the line it is about to land on.
        let mut c = tree.first_child[i as usize];
        while c != NIL {
            if !is_float(tree, c)
                && let Sizing::Percent(p) = child_sizing(tree, c, axis)
            {
                set_axis_clamped(tree, c, axis, content * p);
            }
            c = tree.next_sibling[c as usize];
        }
        let scrolls = match axis {
            AxisSel::Width => spec.scroll_x,
            AxisSel::Height => spec.scroll_y,
        };
        if wraps(tree, i) {
            break_lines(tree, i, content, spec.gap);
            let mut c = first_in_flow(tree, i);
            while c != NIL {
                let end = line_end(tree, c);
                let line = tree.line[c as usize];
                // Each line is its own main-axis box: Grow splits what is
                // left of *its* line.
                let total = distribute_run(tree, c, end, axis, content, spec.gap);
                // Wrapping and shrinking answer the same overflow, and
                // wrapping answers it first: greedy breaking never puts a
                // second child on a line that is already full, so the only
                // line that can still overflow is one holding a single
                // child too wide for the box. Nothing can be broken off
                // that, which is exactly when shrinking is the remaining
                // answer — applied to that line alone, so a wide chip
                // compresses without dragging its neighbours on other
                // lines down with it.
                let deficit = total - content;
                if deficit > 0.5 {
                    shrink_axis(tree, i, axis, deficit, Some(line));
                }
                c = end;
            }
        } else {
            // Fixed/Fit keep their size, Percent has taken its cut, Grow
            // splits the rest.
            let total = distribute_run(
                tree,
                tree.first_child[i as usize],
                NIL,
                axis,
                content,
                spec.gap,
            );
            let deficit = total - content;
            if deficit > 0.5 && !scrolls {
                shrink_axis(tree, i, axis, deficit, None);
            }
        }
    } else if wraps(tree, i) {
        // Cross axis of a wrapping row: a Grow child fills *its line*, not
        // the container. The lines share the container's leftover equally
        // (see `line_stretch`), so with one line this is the branch below
        // exactly, and a row that happens not to wrap keeps its old layout.
        let (lines, stacked, _) = wrap_measure(tree, i);
        let stretch = line_stretch(lines, stacked, content);
        let mut c = first_in_flow(tree, i);
        while c != NIL {
            let end = line_end(tree, c);
            let extent = line_extents(tree, c, end, spec.gap).1 + stretch;
            let mut k = c;
            while k != end && k != NIL {
                if !is_float(tree, k) {
                    match child_sizing(tree, k, axis) {
                        Sizing::Grow(_) => set_axis_clamped(tree, k, axis, extent),
                        Sizing::Percent(p) => set_axis_clamped(tree, k, axis, extent * p),
                        _ => {}
                    }
                }
                k = tree.next_sibling[k as usize];
            }
            c = end;
        }
    } else {
        // Cross axis: Grow/Percent resolve against the content box directly.
        let mut c = tree.first_child[i as usize];
        while c != NIL {
            if !is_float(tree, c) {
                match child_sizing(tree, c, axis) {
                    Sizing::Grow(_) => set_axis_clamped(tree, c, axis, content),
                    Sizing::Percent(p) => set_axis_clamped(tree, c, axis, content * p),
                    _ => {}
                }
            }
            c = tree.next_sibling[c as usize];
        }
        // A table's rows are wide by now: lay its columns across them.
        if axis == AxisSel::Width && tree.any_table && spec.table {
            table_resolve(tree, i);
        }
    }

    // Floating children size Grow/Percent against their anchor. A frame
    // with no floats skips the walk: it would read every child's spec to
    // find none.
    let mut c = if tree.any_float {
        tree.first_child[i as usize]
    } else {
        NIL
    };
    while c != NIL {
        if let Some(cfg) = tree.specs[c as usize].layout.float {
            let vp = float_viewport(tree, c, viewport);
            let anchor_dim = match (cfg.anchor, axis) {
                (FloatAnchor::Parent, AxisSel::Width) => tree.size[i as usize].w,
                (FloatAnchor::Parent, AxisSel::Height) => tree.size[i as usize].h,
                (FloatAnchor::Viewport, AxisSel::Width) => vp.w,
                (FloatAnchor::Viewport, AxisSel::Height) => vp.h,
                // Sized in the sixth pass, once the anchor is placed.
                (FloatAnchor::Node(_), _) => 0.0,
            };
            match child_sizing(tree, c, axis) {
                Sizing::Grow(_) => set_axis_clamped(tree, c, axis, anchor_dim),
                Sizing::Percent(p) => set_axis_clamped(tree, c, axis, anchor_dim * p),
                _ => {}
            }
        }
        c = tree.next_sibling[c as usize];
    }

    // Text children have no spec sizing; clamp their width to the content box
    // so fit_heights wraps them. Same walk, same gate: no text, no clamp.
    if axis == AxisSel::Width && tree.any_text {
        let mut c = tree.first_child[i as usize];
        while c != NIL {
            if matches!(tree.content[c as usize], NodeContent::Text(_))
                && tree.size[c as usize].w > content
            {
                tree.size[c as usize].w = content;
            }
            c = tree.next_sibling[c as usize];
        }
    }
}

/// Resolves the Grow children of one main-axis run — a whole child list
/// (`end == NIL`) or one wrap line — into `content`, and returns what the
/// run ends up occupying, gaps included.
fn distribute_run(
    tree: &mut Tree,
    start: u32,
    end: u32,
    axis: AxisSel,
    content: f32,
    gap: f32,
) -> f32 {
    let mut used = 0.0f32;
    let mut grow_total = 0.0f32;
    let mut n = 0u32;
    let mut c = start;
    while c != end && c != NIL {
        if !is_float(tree, c) {
            match child_sizing(tree, c, axis) {
                Sizing::Grow(f) => grow_total += f.max(0.0),
                _ => used += get_axis(tree, c, axis),
            }
            n += 1;
        }
        c = tree.next_sibling[c as usize];
    }
    if n > 1 {
        used += gap * (n - 1) as f32;
    }
    let mut total = used;
    if grow_total > 0.0 {
        // Flexbox's freeze loop. A grow child's share is `remain` split
        // by factor; one whose own min or max holds it off that share is
        // frozen at the clamp, its size moved into `used`, and the rest
        // share what is left, until a pass freezes nothing. A max on one
        // child is room for its siblings, not a hole at the end of the
        // run (a devtools inspector capped at 300 left the node list
        // above it short of the panel by the same 300). At most one pass
        // per grow child, since each pass past the first froze one.
        let mut frozen: Vec<u32> = Vec::new();
        loop {
            let remain = (content - used).max(0.0);
            let mut froze = false;
            let mut c = start;
            while c != end && c != NIL {
                if !is_float(tree, c)
                    && !frozen.contains(&c)
                    && let Sizing::Grow(f) = child_sizing(tree, c, axis)
                {
                    let share = remain * f.max(0.0) / grow_total;
                    set_axis_clamped(tree, c, axis, share);
                    let got = get_axis(tree, c, axis);
                    if (got - share).abs() > 0.01 {
                        frozen.push(c);
                        used += got;
                        grow_total -= f.max(0.0);
                        froze = true;
                    }
                }
                c = tree.next_sibling[c as usize];
            }
            if !froze || grow_total <= 0.0 {
                break;
            }
        }
        total = used;
        let mut c = start;
        while c != end && c != NIL {
            if !is_float(tree, c)
                && !frozen.contains(&c)
                && matches!(child_sizing(tree, c, axis), Sizing::Grow(_))
            {
                total += get_axis(tree, c, axis);
            }
            c = tree.next_sibling[c as usize];
        }
    }
    total
}

/// The shrink pass: pays off `deficit` (how far in-flow children overflow
/// the parent's main-axis content box) by compressing Fit-sized children
/// toward their min (default 0), largest first — so equal children end up
/// equal, clay-style. Fixed and Percent
/// keep their declared size; Grow never overflows. Text shrinks in width
/// (it rewraps at the new width in fit_heights) but never in height. Scroll
/// axes skip this entirely — overflow is the point of a scroll container.
///
/// `only_line` restricts it to one wrap line. A wrapping container reaches
/// here only for a line it could not break any further (a single child
/// wider than the box), so the compression stays on that line instead of
/// squeezing children that are already comfortable on other ones.
fn shrink_axis(tree: &mut Tree, i: u32, axis: AxisSel, mut deficit: f32, only_line: Option<u32>) {
    let shrinkable = |tree: &Tree, c: u32| -> Option<f32> {
        if is_float(tree, c) || child_sizing(tree, c, axis) != Sizing::Fit {
            return None;
        }
        if only_line.is_some_and(|l| tree.line[c as usize] != l) {
            return None;
        }
        // Squashing text/editors vertically would clip lines, and images
        // would distort; width shrink rewraps (and re-aspects) instead.
        if axis == AxisSel::Height
            && matches!(
                tree.content[c as usize],
                NodeContent::Text(_) | NodeContent::Edit(_) | NodeContent::Image(..)
            )
        {
            return None;
        }
        // Resolved to a number by the fit pass of this axis, which ran.
        let spec = tree.specs[c as usize].layout;
        Some(match axis {
            AxisSel::Width => spec.min_w.resolved(),
            AxisSel::Height => spec.min_h.resolved(),
        })
    };

    // Largest-first, like grow in reverse: pull the biggest children down
    // to the second-biggest, repeat until the deficit is paid or every
    // shrinkable child sits at its min.
    let mut guard = 0;
    while deficit > 0.5 && guard < 128 {
        guard += 1;
        let mut largest = f32::NEG_INFINITY;
        let mut second = 0.0f32;
        let mut count = 0u32;
        let mut c = tree.first_child[i as usize];
        while c != NIL {
            if let Some(min) = shrinkable(tree, c) {
                let s = get_axis(tree, c, axis);
                if s > min + 0.01 {
                    if s > largest + 0.01 {
                        second = if largest.is_finite() {
                            largest.max(second)
                        } else {
                            second
                        };
                        largest = s;
                        count = 1;
                    } else if s > largest - 0.01 {
                        count += 1;
                    } else if s > second {
                        second = s;
                    }
                }
            }
            c = tree.next_sibling[c as usize];
        }
        if count == 0 {
            break;
        }
        let target = (largest - deficit / count as f32).max(second).max(0.0);
        let mut shrunk_any = false;
        let mut c = tree.first_child[i as usize];
        while c != NIL {
            if let Some(min) = shrinkable(tree, c) {
                let s = get_axis(tree, c, axis);
                if s > largest - 0.01 {
                    let new = target.max(min);
                    if new < s {
                        deficit -= s - new;
                        set_axis(tree, c, axis, new);
                        shrunk_any = true;
                    }
                }
            }
            c = tree.next_sibling[c as usize];
        }
        if !shrunk_any {
            break;
        }
    }
}

fn child_sizing(tree: &Tree, c: u32, axis: AxisSel) -> Sizing {
    if matches!(tree.content[c as usize], NodeContent::Text(_)) {
        return Sizing::Fit;
    }
    match axis {
        AxisSel::Width => tree.specs[c as usize].layout.width,
        AxisSel::Height => tree.specs[c as usize].layout.height,
    }
}

fn get_axis(tree: &Tree, c: u32, axis: AxisSel) -> f32 {
    match axis {
        AxisSel::Width => tree.size[c as usize].w,
        AxisSel::Height => tree.size[c as usize].h,
    }
}

fn set_axis(tree: &mut Tree, c: u32, axis: AxisSel, v: f32) {
    match axis {
        AxisSel::Width => tree.size[c as usize].w = v,
        AxisSel::Height => tree.size[c as usize].h = v,
    }
}

/// set_axis clamped by the child's own min/max on that axis.
fn set_axis_clamped(tree: &mut Tree, c: u32, axis: AxisSel, v: f32) {
    let spec = tree.specs[c as usize].layout;
    let v = match axis {
        AxisSel::Width => spec.clamp_w(v),
        AxisSel::Height => spec.clamp_h(v),
    };
    set_axis(tree, c, axis, v);
}

/// The "viewport" a float of `c` means: the window for the devtools' own
/// nodes and for the core's menu (a transient the platform's own would
/// not confine either — and one the panel's select opens *in* the dock,
/// where the host area would push it into the app), and for everyone
/// else the host area — the window less the devtools' dock — when one
/// is set (`Tree::host_area`).
fn float_viewport(tree: &Tree, c: u32, viewport: Size) -> Rect {
    use crate::tree::OriginId;
    let window = Rect::new(0.0, 0.0, viewport.w, viewport.h);
    let origin = tree.origins[c as usize];
    if tree.host_area.w <= 0.0 || origin == OriginId::DEVTOOLS || origin == OriginId::MENU {
        window
    } else {
        tree.host_area
    }
}

/// Places `c`, which is out of flow, against its anchor.
fn place_float(
    tree: &mut Tree,
    cfg: crate::spec::FloatConfig,
    c: u32,
    parent: Rect,
    viewport: Size,
) {
    let vp = float_viewport(tree, c, viewport);
    let anchor = match cfg.anchor {
        FloatAnchor::Parent => parent,
        FloatAnchor::Viewport => vp,
        // Placed in the sixth pass, once the anchor is.
        FloatAnchor::Node(_) => return,
    };
    let cs = tree.size[c as usize];
    let mut x = attach(
        anchor.x,
        anchor.w,
        cs.w,
        cfg.anchor_point.0,
        cfg.self_point.0,
        cfg.offset.x,
    );
    let mut y = attach(
        anchor.y,
        anchor.h,
        cs.h,
        cfg.anchor_point.1,
        cfg.self_point.1,
        cfg.offset.y,
    );
    // Mirroring across the viewport itself would teleport a
    // cursor-anchored float to the opposite side of the window,
    // so viewport floats only clamp.
    if cfg.fit && cfg.anchor == FloatAnchor::Parent {
        // Mirror the attachment across the anchor per axis when
        // the mirrored side is less off-screen (ties keep the
        // declared side), then clamp the rest. Clamp order pins
        // the top/left edge on screen when nothing fits.
        let fx = attach(
            anchor.x,
            anchor.w,
            cs.w,
            mirror(cfg.anchor_point.0),
            mirror(cfg.self_point.0),
            -cfg.offset.x,
        );
        if overflow(x - vp.x, cs.w, vp.w) > overflow(fx - vp.x, cs.w, vp.w) {
            x = fx;
        }
        let fy = attach(
            anchor.y,
            anchor.h,
            cs.h,
            mirror(cfg.anchor_point.1),
            mirror(cfg.self_point.1),
            -cfg.offset.y,
        );
        if overflow(y - vp.y, cs.h, vp.h) > overflow(fy - vp.y, cs.h, vp.h) {
            y = fy;
        }
    }
    if cfg.fit {
        x = x.min(vp.x + vp.w - cs.w).max(vp.x);
        y = y.min(vp.y + vp.h - cs.h).max(vp.y);
    }
    tree.pos[c as usize] = Vec2::new(x, y);
}

fn positions(
    tree: &mut Tree,
    scroll: &mut ScrollStore,
    viewport: Size,
    scale: f32,
    range: std::ops::Range<usize>,
) {
    for i in range {
        if tree.parent[i] == NIL {
            tree.pos[i] = Vec2::ZERO;
        }
        let spec = tree.specs[i].layout;
        let origin = tree.pos[i];
        let size = tree.size[i];
        let wrap = wraps(tree, i as u32);

        let (main_content, cross_content, main_pad_start, cross_pad_start) = match spec.dir {
            Dir::Row => (
                size.w - spec.padding.x(),
                size.h - spec.padding.y(),
                spec.padding.l,
                spec.padding.t,
            ),
            Dir::Column => (
                size.h - spec.padding.y(),
                size.w - spec.padding.x(),
                spec.padding.t,
                spec.padding.l,
            ),
        };

        // The content box the children occupy, in main/cross terms. For a
        // wrapping row that is the widest line by the longest stack of
        // lines; for everything else the one run of children.
        let mut stretch = 0.0f32;
        let (total_main, max_cross) = if wrap {
            let (lines, stacked, widest) = wrap_measure(tree, i as u32);
            stretch = line_stretch(lines, stacked, cross_content);
            (widest, stacked + stretch * lines as f32)
        } else {
            let mut total_main = 0.0f32;
            let mut max_cross = 0.0f32;
            let mut n = 0u32;
            for c in tree.children(i as u32) {
                if is_float(tree, c) {
                    continue;
                }
                let (c_main, c_cross) = match spec.dir {
                    Dir::Row => (tree.size[c as usize].w, tree.size[c as usize].h),
                    Dir::Column => (tree.size[c as usize].h, tree.size[c as usize].w),
                };
                total_main += c_main;
                max_cross = max_cross.max(c_cross);
                n += 1;
            }
            if n > 1 {
                total_main += spec.gap * (n - 1) as f32;
            }
            (total_main, max_cross)
        };

        // Scroll containers: clamp the retained offset to this frame's
        // overflow and shift children by it.
        let mut offset = Vec2::ZERO;
        // Anchoring (backlog C26 step 3): the scroll axis is the main axis,
        // the container is not wrapping, and a previous layout recorded
        // which child was first in view and where its leading edge sat in
        // the content. Where that edge sits *now* is the same walk the
        // placement below makes, minus the offset; the difference is added
        // to the retained offset before it is clamped, so the child stays
        // where it was on screen whatever grew or shrank before it.
        let anchors = spec.anchor
            && !wrap
            && match spec.dir {
                Dir::Row => spec.scroll_x,
                Dir::Column => spec.scroll_y,
            };
        let free_main = (main_content - total_main).max(0.0);
        let content_start = main_pad_start + align_factor(spec.main_align) * free_main;
        if anchors && let Some((anchor, was_at)) = scroll.anchor(tree.keys[i]) {
            let mut at = content_start;
            let mut c = first_in_flow(tree, i as u32);
            while c != NIL {
                if !is_float(tree, c) {
                    if tree.keys[c as usize] == anchor {
                        let delta = at - was_at;
                        if delta != 0.0 {
                            scroll.scroll_by(
                                tree.keys[i],
                                match spec.dir {
                                    Dir::Row => Vec2::new(delta, 0.0),
                                    Dir::Column => Vec2::new(0.0, delta),
                                },
                            );
                        }
                        break;
                    }
                    let c_main = match spec.dir {
                        Dir::Row => tree.size[c as usize].w,
                        Dir::Column => tree.size[c as usize].h,
                    };
                    at += c_main + spec.gap;
                }
                c = tree.next_sibling[c as usize];
            }
        }
        if spec.scroll_x || spec.scroll_y {
            let (content_w, content_h) = match spec.dir {
                Dir::Row => (total_main + spec.padding.x(), max_cross + spec.padding.y()),
                Dir::Column => (max_cross + spec.padding.x(), total_main + spec.padding.y()),
            };
            let max = Vec2::new(
                if spec.scroll_x {
                    (content_w - size.w).max(0.0)
                } else {
                    0.0
                },
                if spec.scroll_y {
                    (content_h - size.h).max(0.0)
                } else {
                    0.0
                },
            );
            tree.scroll_max[i] = max;
            // The one place the container's resolved box and its content
            // size exist together; the store keeps a copy, since the tree
            // holding them is cleared before the next view reads it.
            // The retained offset stays exact — a wheel notch of 0.3 px is
            // not lost, it accumulates — and what the children are *moved*
            // by is whole physical pixels, so a row's text does not wobble
            // inside the row while the list scrolls (`Vec2::snapped`).
            offset = scroll
                .resolve(
                    tree.keys[i],
                    Rect::from_pos_size(origin, size),
                    Size::new(content_w, content_h),
                    max,
                )
                .snapped(scale);
        }
        let (main_scroll, cross_scroll) = match spec.dir {
            Dir::Row => (offset.x, offset.y),
            Dir::Column => (offset.y, offset.x),
        };
        if anchors {
            // The anchor for the next layout: the first in-flow child whose
            // trailing edge is past the offset — the first one in view —
            // and where its leading edge sits in the content.
            let mut next = None;
            let mut at = content_start;
            let mut c = first_in_flow(tree, i as u32);
            while c != NIL {
                if !is_float(tree, c) {
                    let c_main = match spec.dir {
                        Dir::Row => tree.size[c as usize].w,
                        Dir::Column => tree.size[c as usize].h,
                    };
                    if at + c_main > main_scroll {
                        next = Some((tree.keys[c as usize], at));
                        break;
                    }
                    at += c_main + spec.gap;
                }
                c = tree.next_sibling[c as usize];
            }
            scroll.set_anchor(tree.keys[i], next);
        }

        // Out of flow first, so the in-flow walk is one shape whether or
        // not it goes line by line. A frame with no floats skips the walk.
        let mut c = if tree.any_float {
            tree.first_child[i]
        } else {
            NIL
        };
        while c != NIL {
            if let Some(cfg) = tree.specs[c as usize].layout.float {
                place_float(tree, cfg, c, Rect::from_pos_size(origin, size), viewport);
            }
            c = tree.next_sibling[c as usize];
        }

        // One line for an unwrapped container, N for a wrapping row. Main
        // alignment places each line's children in the content box the way
        // it places the single run's, and cross alignment places a child in
        // its own line; with one line the two compose back into the
        // unwrapped placement exactly.
        let mut cross_cursor = cross_pad_start - cross_scroll;
        let mut line_start = first_in_flow(tree, i as u32);
        while line_start != NIL {
            let (end, extent, run_main) = if wrap {
                let end = line_end(tree, line_start);
                let (main, cross) = line_extents(tree, line_start, end, spec.gap);
                (end, cross + stretch, main)
            } else {
                (NIL, cross_content, total_main)
            };
            let free = (main_content - run_main).max(0.0);
            let mut cursor = main_pad_start + align_factor(spec.main_align) * free - main_scroll;
            let mut c = line_start;
            while c != end && c != NIL {
                if is_float(tree, c) {
                    c = tree.next_sibling[c as usize];
                    continue;
                }
                let cs = tree.size[c as usize];
                let (c_main, c_cross) = match spec.dir {
                    Dir::Row => (cs.w, cs.h),
                    Dir::Column => (cs.h, cs.w),
                };
                let cross_off =
                    cross_cursor + align_factor(spec.cross_align) * (extent - c_cross).max(0.0);
                tree.pos[c as usize] = match spec.dir {
                    Dir::Row => Vec2::new(origin.x + cursor, origin.y + cross_off),
                    Dir::Column => Vec2::new(origin.x + cross_off, origin.y + cursor),
                };
                cursor += c_main + spec.gap;
                c = tree.next_sibling[c as usize];
            }
            cross_cursor += extent + spec.cross_gap;
            line_start = end;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::Edges;
    use crate::key::Key;
    use crate::spec::NodeSpec;
    use crate::tree::{NodeContent, OriginId, TextId};

    /// Deterministic measurer: every text is `10px * chars_hint` wide and wraps
    /// into 20px lines. The TextId encodes the char count for test purposes.
    struct StubText;

    impl TextMeasure for StubText {
        fn intrinsic(&mut self, id: TextId) -> Size {
            Size::new(10.0 * id.0 as f32, 20.0)
        }
        fn wrapped(&mut self, id: TextId, max_w: f32) -> Size {
            let full = 10.0 * id.0 as f32;
            if max_w <= 0.0 || full <= max_w {
                return Size::new(full, 20.0);
            }
            let lines = (full / max_w).ceil();
            Size::new(max_w, lines * 20.0)
        }
    }

    struct T {
        tree: Tree,
    }

    impl T {
        fn new(root_spec: NodeSpec) -> Self {
            let mut tree = Tree::new();
            tree.push(
                NIL,
                Key::ROOT,
                OriginId::HOST,
                root_spec,
                NodeContent::Container,
            );
            T { tree }
        }

        fn node(&mut self, parent: u32, spec: NodeSpec) -> u32 {
            let key = Key::ROOT.index(self.tree.len() as u64);
            self.tree
                .push(parent, key, OriginId::HOST, spec, NodeContent::Container)
        }

        fn text(&mut self, parent: u32, chars: u32) -> u32 {
            let key = Key::ROOT.index(self.tree.len() as u64);
            self.tree.push(
                parent,
                key,
                OriginId::HOST,
                NodeSpec::default(),
                NodeContent::Text(TextId(chars)),
            )
        }

        fn run(&mut self, vw: f32, vh: f32) {
            let mut scroll = ScrollStore::default();
            compute(
                &mut self.tree,
                &mut StubText,
                &mut scroll,
                Size::new(vw, vh),
                1.0,
            );
        }

        fn size(&self, i: u32) -> Size {
            self.tree.size[i as usize]
        }

        fn pos(&self, i: u32) -> Vec2 {
            self.tree.pos[i as usize]
        }
    }

    fn px(v: f32) -> Sizing {
        Sizing::Fixed(v)
    }

    /// Two grow children in a 600 column, one capped at 100: the cap is
    /// the other's room, not a hole. A min that holds a child past its
    /// share takes from its siblings the same way, and a run of clamps
    /// resolves in one layout.
    #[test]
    fn a_grow_childs_clamp_is_its_siblings_room() {
        let mut t = T::new(NodeSpec::column().width(px(100.0)).height(px(600.0)));
        let a = t.node(0, NodeSpec::row().height(Sizing::Grow(1.0)));
        let b = t.node(
            0,
            NodeSpec::row().height(Sizing::Grow(1.0)).max_height(100.0),
        );
        t.run(1000.0, 1000.0);
        assert_eq!(t.size(b).h, 100.0);
        assert_eq!(t.size(a).h, 500.0);

        let mut t = T::new(NodeSpec::row().width(px(300.0)).height(px(50.0)));
        let a = t.node(0, NodeSpec::row().width(Sizing::Grow(1.0)));
        let b = t.node(0, NodeSpec::row().width(Sizing::Grow(1.0)).min_width(200.0));
        let c = t.node(0, NodeSpec::row().width(Sizing::Grow(1.0)).max_width(20.0));
        t.run(1000.0, 1000.0);
        assert_eq!(t.size(b).w, 200.0);
        assert_eq!(t.size(c).w, 20.0);
        assert_eq!(t.size(a).w, 80.0);
        assert_eq!(t.pos(c).x, 280.0);
    }

    #[test]
    fn fit_row_sums_children_and_gaps() {
        let mut t = T::new(NodeSpec::row().pad(10.0).gap(5.0));
        let r = 0;
        t.node(r, NodeSpec::column().width(px(30.0)).height(px(40.0)));
        t.node(r, NodeSpec::column().width(px(20.0)).height(px(25.0)));
        t.run(1000.0, 1000.0);
        // 30 + 5 + 20 + 2*10 pad = 75; height = max(40,25) + 20 = 60
        assert_eq!(t.size(r), Size::new(75.0, 60.0));
    }

    #[test]
    fn fit_column_sums_heights() {
        let mut t = T::new(NodeSpec::column().gap(4.0));
        t.node(0, NodeSpec::row().width(px(10.0)).height(px(10.0)));
        t.node(0, NodeSpec::row().width(px(50.0)).height(px(10.0)));
        t.node(0, NodeSpec::row().width(px(30.0)).height(px(10.0)));
        t.run(1000.0, 1000.0);
        assert_eq!(t.size(0), Size::new(50.0, 38.0));
    }

    #[test]
    fn grow_splits_remaining_space_by_factor() {
        let mut t = T::new(NodeSpec::row().width(px(300.0)).height(px(100.0)).gap(10.0));
        let a = t.node(0, NodeSpec::column().width(px(50.0)).height(px(10.0)));
        let b = t.node(
            0,
            NodeSpec::column().width(Sizing::Grow(1.0)).height(px(10.0)),
        );
        let c = t.node(
            0,
            NodeSpec::column().width(Sizing::Grow(2.0)).height(px(10.0)),
        );
        t.run(1000.0, 1000.0);
        // content 300, fixed 50, gaps 20 -> remain 230 split 1:2
        let bw = t.size(b).w;
        let cw = t.size(c).w;
        assert!((bw - 230.0 / 3.0).abs() < 0.01, "b={bw}");
        assert!((cw - 460.0 / 3.0).abs() < 0.01, "c={cw}");
        assert_eq!(t.size(a).w, 50.0);
    }

    #[test]
    fn percent_resolves_against_content_box() {
        let mut t = T::new(NodeSpec::row().width(px(200.0)).height(px(100.0)).pad(10.0));
        let a = t.node(
            0,
            NodeSpec::column()
                .width(Sizing::Percent(0.5))
                .height(Sizing::Percent(1.0)),
        );
        t.run(1000.0, 1000.0);
        assert_eq!(t.size(a), Size::new(90.0, 80.0)); // (200-20)*0.5, (100-20)*1.0
    }

    #[test]
    fn cross_axis_grow_fills_content() {
        let mut t = T::new(
            NodeSpec::column()
                .width(px(120.0))
                .height(px(200.0))
                .pad(8.0),
        );
        let a = t.node(0, NodeSpec::row().width(Sizing::Grow(1.0)).height(px(30.0)));
        t.run(1000.0, 1000.0);
        assert_eq!(t.size(a).w, 104.0);
    }

    #[test]
    fn nested_fit_propagates_up() {
        let mut t = T::new(NodeSpec::column());
        let mid = t.node(0, NodeSpec::row().pad(5.0).gap(2.0));
        t.node(mid, NodeSpec::column().width(px(10.0)).height(px(10.0)));
        t.node(mid, NodeSpec::column().width(px(10.0)).height(px(10.0)));
        t.run(1000.0, 1000.0);
        assert_eq!(t.size(mid), Size::new(32.0, 20.0));
        assert_eq!(t.size(0), Size::new(32.0, 20.0));
    }

    #[test]
    fn root_grow_takes_viewport() {
        let mut t = T::new(NodeSpec::column().fill());
        t.run(800.0, 600.0);
        assert_eq!(t.size(0), Size::new(800.0, 600.0));
    }

    #[test]
    fn positions_row_with_gap_and_padding() {
        let mut t = T::new(
            NodeSpec::row()
                .width(px(300.0))
                .height(px(100.0))
                .pad(10.0)
                .gap(5.0),
        );
        let a = t.node(0, NodeSpec::column().width(px(40.0)).height(px(20.0)));
        let b = t.node(0, NodeSpec::column().width(px(40.0)).height(px(20.0)));
        t.run(1000.0, 1000.0);
        assert_eq!(t.pos(a), Vec2::new(10.0, 10.0));
        assert_eq!(t.pos(b), Vec2::new(55.0, 10.0));
    }

    #[test]
    fn main_center_alignment_offsets_children() {
        let mut t = T::new(NodeSpec::row().width(px(200.0)).height(px(50.0)).center());
        let a = t.node(0, NodeSpec::column().width(px(60.0)).height(px(20.0)));
        t.run(1000.0, 1000.0);
        assert_eq!(t.pos(a), Vec2::new(70.0, 15.0));
    }

    #[test]
    fn main_end_alignment() {
        let mut t = T::new(
            NodeSpec::column()
                .width(px(100.0))
                .height(px(100.0))
                .main_align(Align::End)
                .gap(10.0),
        );
        let a = t.node(0, NodeSpec::row().width(px(10.0)).height(px(20.0)));
        let b = t.node(0, NodeSpec::row().width(px(10.0)).height(px(20.0)));
        t.run(1000.0, 1000.0);
        assert_eq!(t.pos(a).y, 50.0);
        assert_eq!(t.pos(b).y, 80.0);
    }

    #[test]
    fn text_gets_intrinsic_size_when_it_fits() {
        let mut t = T::new(NodeSpec::column().width(px(500.0)).height(px(500.0)));
        let txt = t.text(0, 8); // 80px wide
        t.run(1000.0, 1000.0);
        assert_eq!(t.size(txt), Size::new(80.0, 20.0));
    }

    #[test]
    fn text_wraps_when_clamped_by_parent() {
        let mut t = T::new(
            NodeSpec::column()
                .width(px(100.0))
                .height(px(500.0))
                .pad(10.0),
        );
        let txt = t.text(0, 20); // 200px intrinsic, clamped to 80 -> 3 lines
        t.run(1000.0, 1000.0);
        assert_eq!(t.size(txt).w, 80.0);
        assert_eq!(t.size(txt).h, 60.0);
    }

    #[test]
    fn text_wrapping_grows_fit_parent_height() {
        let mut t = T::new(NodeSpec::column().width(px(100.0)));
        let txt = t.text(0, 30); // 300px intrinsic -> wraps to 100 -> 3 lines
        t.run(1000.0, 1000.0);
        assert_eq!(t.size(txt).h, 60.0);
        assert_eq!(t.size(0).h, 60.0);
    }

    #[test]
    fn grow_with_no_space_left_gets_zero() {
        let mut t = T::new(NodeSpec::row().width(px(100.0)).height(px(50.0)));
        let a = t.node(0, NodeSpec::column().width(px(120.0)).height(px(10.0)));
        let b = t.node(
            0,
            NodeSpec::column().width(Sizing::Grow(1.0)).height(px(10.0)),
        );
        t.run(1000.0, 1000.0);
        assert_eq!(t.size(a).w, 120.0); // no shrinking in v0
        assert_eq!(t.size(b).w, 0.0);
    }

    #[test]
    fn deep_nesting_positions_accumulate() {
        let mut t = T::new(NodeSpec::column().pad(10.0));
        let l1 = t.node(0, NodeSpec::column().pad(10.0));
        let l2 = t.node(l1, NodeSpec::column().pad(10.0));
        let leaf = t.node(l2, NodeSpec::row().width(px(10.0)).height(px(10.0)));
        t.run(1000.0, 1000.0);
        assert_eq!(t.pos(leaf), Vec2::new(30.0, 30.0));
        assert_eq!(t.size(0), Size::new(70.0, 70.0));
    }

    #[test]
    fn grow_respects_max_width() {
        let mut t = T::new(NodeSpec::row().width(px(800.0)).height(px(100.0)));
        let a = t.node(
            0,
            NodeSpec::column()
                .width(Sizing::Grow(1.0))
                .max_width(560.0)
                .height(px(10.0)),
        );
        t.run(1000.0, 1000.0);
        assert_eq!(t.size(a).w, 560.0);
        // And tracks the parent when it's smaller than the cap.
        let mut t = T::new(NodeSpec::row().width(px(400.0)).height(px(100.0)));
        let a = t.node(
            0,
            NodeSpec::column()
                .width(Sizing::Grow(1.0))
                .max_width(560.0)
                .height(px(10.0)),
        );
        t.run(1000.0, 1000.0);
        assert_eq!(t.size(a).w, 400.0);
    }

    #[test]
    fn min_width_forces_fit_up() {
        let mut t = T::new(NodeSpec::column());
        let a = t.node(0, NodeSpec::row().min_width(120.0).height(px(10.0)));
        t.node(a, NodeSpec::column().width(px(30.0)).height(px(10.0)));
        t.run(1000.0, 1000.0);
        assert_eq!(t.size(a).w, 120.0);
        // Min propagates into the fit parent.
        assert_eq!(t.size(0).w, 120.0);
    }

    #[test]
    fn percent_respects_max() {
        let mut t = T::new(NodeSpec::column().width(px(1000.0)).height(px(1000.0)));
        let a = t.node(
            0,
            NodeSpec::row()
                .width(Sizing::Percent(0.9))
                .max_width(300.0)
                .height(px(10.0)),
        );
        t.run(1000.0, 1000.0);
        assert_eq!(t.size(a).w, 300.0);
    }

    #[test]
    fn text_rewraps_when_capped_parent_shrinks() {
        // Same tree shape at two parent widths: the narrow one wraps taller.
        let build = |parent_w: f32| {
            let mut t = T::new(NodeSpec::column().width(px(parent_w)));
            let txt = t.text(0, 40); // 400px intrinsic
            t.run(1000.0, 1000.0);
            t.size(txt)
        };
        let wide = build(500.0);
        let narrow = build(100.0);
        assert_eq!(wide.h, 20.0);
        assert_eq!(narrow.h, 80.0); // 400 / 100 -> 4 lines
    }

    #[test]
    fn shrink_compresses_largest_fit_child_first() {
        let mut t = T::new(NodeSpec::row().width(px(100.0)).height(px(50.0)));
        let a = t.node(0, NodeSpec::column());
        t.node(a, NodeSpec::row().width(px(80.0)).height(px(10.0)));
        let b = t.node(0, NodeSpec::column());
        t.node(b, NodeSpec::row().width(px(40.0)).height(px(10.0)));
        t.run(1000.0, 1000.0);
        // 120 into 100: the 80 child pays the whole 20px deficit.
        assert_eq!(t.size(a).w, 60.0);
        assert_eq!(t.size(b).w, 40.0);
    }

    #[test]
    fn shrink_respects_min_and_spills_to_the_next() {
        let mut t = T::new(NodeSpec::row().width(px(100.0)).height(px(50.0)));
        let a = t.node(0, NodeSpec::column().min_width(70.0));
        t.node(a, NodeSpec::row().width(px(80.0)).height(px(10.0)));
        let b = t.node(0, NodeSpec::column());
        t.node(b, NodeSpec::row().width(px(40.0)).height(px(10.0)));
        t.run(1000.0, 1000.0);
        // a stops at its min; b pays the rest.
        assert_eq!(t.size(a).w, 70.0);
        assert_eq!(t.size(b).w, 30.0);
    }

    #[test]
    fn equal_children_shrink_equally() {
        let mut t = T::new(NodeSpec::row().width(px(100.0)).height(px(50.0)));
        let mut kids = Vec::new();
        for _ in 0..3 {
            let c = t.node(0, NodeSpec::column());
            t.node(c, NodeSpec::row().width(px(60.0)).height(px(10.0)));
            kids.push(c);
        }
        t.run(1000.0, 1000.0);
        for c in kids {
            assert!(
                (t.size(c).w - 100.0 / 3.0).abs() < 0.1,
                "got {}",
                t.size(c).w
            );
        }
    }

    #[test]
    fn shrunk_text_rewraps() {
        let mut t = T::new(NodeSpec::row().width(px(200.0)).height(px(500.0)));
        t.node(0, NodeSpec::column().width(px(80.0)).height(px(10.0)));
        let txt = t.text(0, 20); // 200px intrinsic
        t.run(1000.0, 1000.0);
        // 280 into 200: text pays the deficit, then wraps at 120 -> 2 lines.
        assert_eq!(t.size(txt).w, 120.0);
        assert_eq!(t.size(txt).h, 40.0);
    }

    #[test]
    fn text_never_shrinks_vertically() {
        let mut t = T::new(NodeSpec::column().width(px(200.0)).height(px(30.0)));
        let txt = t.text(0, 30); // wraps to 200 -> 2 lines = 40 > 30 parent
        t.run(1000.0, 1000.0);
        assert_eq!(
            t.size(txt).h,
            40.0,
            "text overflows rather than clipping lines"
        );
    }

    #[test]
    fn scroll_axis_skips_shrink() {
        let mut t = T::new(
            NodeSpec::column()
                .width(px(100.0))
                .height(px(100.0))
                .scroll_y(),
        );
        for _ in 0..2 {
            let c = t.node(0, NodeSpec::column());
            t.node(c, NodeSpec::row().width(px(10.0)).height(px(80.0)));
        }
        t.run(1000.0, 1000.0);
        // 160 of content in a 100 box stays 160: it scrolls instead.
        for c in [1u32, 3u32] {
            assert_eq!(t.size(c).h, 80.0);
        }
    }

    /// An i3-style tab bar: every tab `grow` with a `min_width`, on a row
    /// that scrolls x. With room the tabs split the bar evenly; past it
    /// each sits at its min and the bar scrolls by the overflow — the
    /// clamp in `distribute_run` and the shrink pass a scroll axis skips
    /// are what make one declaration cover both regimes.
    #[test]
    fn grow_tabs_split_evenly_then_scroll_at_their_min() {
        let bar = || NodeSpec::row().width(px(600.0)).height(px(30.0)).scroll_x();
        let tab = || {
            NodeSpec::column()
                .width(Sizing::Grow(1.0))
                .min_width(80.0)
                .height(px(30.0))
        };
        // Three tabs: 200 each, nothing to scroll.
        let mut t = T::new(bar());
        let tabs: Vec<u32> = (0..3).map(|_| t.node(0, tab())).collect();
        t.run(1000.0, 1000.0);
        for (k, &c) in tabs.iter().enumerate() {
            assert_eq!(t.size(c).w, 200.0);
            assert_eq!(t.pos(c).x, 200.0 * k as f32);
        }
        assert_eq!(t.tree.scroll_max[0].x, 0.0);
        // Ten tabs: 800 of min in a 600 bar, every tab at 80, 200 to scroll.
        let mut t = T::new(bar());
        let tabs: Vec<u32> = (0..10).map(|_| t.node(0, tab())).collect();
        t.run(1000.0, 1000.0);
        for (k, &c) in tabs.iter().enumerate() {
            assert_eq!(t.size(c).w, 80.0);
            assert_eq!(t.pos(c).x, 80.0 * k as f32);
        }
        assert_eq!(t.tree.scroll_max[0].x, 200.0);
        // Same bar without scroll_x: the mins still hold (grow is not
        // shrinkable), so the row overflows and clips instead.
        let mut t = T::new(NodeSpec::row().width(px(600.0)).height(px(30.0)));
        let tabs: Vec<u32> = (0..10).map(|_| t.node(0, tab())).collect();
        t.run(1000.0, 1000.0);
        for &c in &tabs {
            assert_eq!(t.size(c).w, 80.0);
        }
        // The floor as the tab's own content: `Min::FIT` under `Grow`.
        // Ten tabs each around an 80 px label split a 600 bar as the
        // numeric min did — the label is the min — and the fit resolves
        // to a number the spec keeps.
        let mut t = T::new(bar());
        let tabs: Vec<u32> = (0..10)
            .map(|_| {
                let c = t.node(0, tab().min_width(Min::FIT));
                t.text(c, 8);
                c
            })
            .collect();
        t.run(1000.0, 1000.0);
        for (k, &c) in tabs.iter().enumerate() {
            assert_eq!(t.size(c).w, 80.0);
            assert_eq!(t.pos(c).x, 80.0 * k as f32);
            assert_eq!(t.tree.specs[c as usize].layout.min_w, Min::px(80.0));
        }
        assert_eq!(t.tree.scroll_max[0].x, 200.0);
        // And with room, the same tabs split it: 3 × 200, the floor idle.
        let mut t = T::new(bar());
        let tabs: Vec<u32> = (0..3)
            .map(|_| {
                let c = t.node(0, tab().min_width(Min::FIT));
                t.text(c, 8);
                c
            })
            .collect();
        t.run(1000.0, 1000.0);
        for &c in &tabs {
            assert_eq!(t.size(c).w, 200.0);
        }
    }

    /// `Min::FIT` on the cross axis and under a percent: a 50%-tall cell
    /// in a 20-tall row floors at its 16-tall child, and a fit floor with
    /// nothing inside is no floor.
    #[test]
    fn min_fit_floors_a_percent_height_at_its_content() {
        let mut t = T::new(NodeSpec::row().width(px(100.0)).height(px(20.0)));
        let a = t.node(
            0,
            NodeSpec::column()
                .width(px(10.0))
                .height(Sizing::Percent(0.5))
                .min_height(Min::FIT),
        );
        t.node(a, NodeSpec::column().width(px(10.0)).height(px(16.0)));
        let b = t.node(
            0,
            NodeSpec::column()
                .width(px(10.0))
                .height(Sizing::Percent(0.5))
                .min_height(Min::FIT),
        );
        t.run(1000.0, 1000.0);
        assert_eq!(t.size(a).h, 16.0);
        assert_eq!(t.size(b).h, 10.0);
    }

    #[test]
    fn padding_asymmetric() {
        let mut t = T::new(
            NodeSpec::column()
                .width(px(100.0))
                .height(px(100.0))
                .padding(Edges {
                    l: 1.0,
                    r: 2.0,
                    t: 3.0,
                    b: 4.0,
                }),
        );
        let a = t.node(0, NodeSpec::row().fill());
        t.run(1000.0, 1000.0);
        assert_eq!(t.pos(a), Vec2::new(1.0, 3.0));
        assert_eq!(t.size(a), Size::new(97.0, 93.0));
    }
}
