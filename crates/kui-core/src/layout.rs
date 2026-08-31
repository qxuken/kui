//! Clay-style flex layout over the flat tree, five passes:
//!
//! 1. fit widths      (reverse  = children before parents)
//! 2. grow widths     (forward  = parents before children)
//! 3. fit heights     (reverse; text wraps at its final width here)
//! 4. grow heights    (forward)
//! 5. positions       (forward)
//!
//! Text measurement goes through `TextMeasure` so the solver is testable with
//! a deterministic stub and never depends on system fonts.

use crate::geom::{Rect, Size, Vec2};
use crate::scroll::ScrollStore;
use crate::spec::{Align, Dir, FloatAnchor, Sizing};
use crate::tree::{NIL, NodeContent, Tree};

fn is_float(tree: &Tree, i: u32) -> bool {
    tree.specs[i as usize].layout.float.is_some()
}

fn align_factor(a: Align) -> f32 {
    match a {
        Align::Start => 0.0,
        Align::Center => 0.5,
        Align::End => 1.0,
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
}

pub fn compute(
    tree: &mut Tree,
    text: &mut dyn TextMeasure,
    scroll: &mut ScrollStore,
    viewport: Size,
) {
    if tree.is_empty() {
        return;
    }
    fit_widths(tree, text);
    grow_widths(tree, viewport);
    fit_heights(tree, text);
    grow_heights(tree, viewport);
    positions(tree, scroll, viewport);
}

fn fit_widths(tree: &mut Tree, text: &mut dyn TextMeasure) {
    for i in (0..tree.len()).rev() {
        if let NodeContent::Text(tid) = tree.content[i] {
            tree.size[i].w = text.intrinsic(tid).w;
            continue;
        }
        let spec = tree.specs[i].layout;
        if let NodeContent::Edit(key) = tree.content[i] {
            tree.size[i].w = spec.clamp_w(match spec.width {
                Sizing::Fixed(px) => px,
                Sizing::Grow(_) | Sizing::Percent(_) => 0.0,
                Sizing::Fit => text.edit_intrinsic(key).w + spec.padding.x(),
            });
            continue;
        }
        tree.size[i].w = spec.clamp_w(match spec.width {
            Sizing::Fixed(px) => px,
            // Resolved against the parent later; contributes nothing to fit.
            Sizing::Grow(_) | Sizing::Percent(_) => 0.0,
            Sizing::Fit => {
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
        });
    }
}

fn fit_heights(tree: &mut Tree, text: &mut dyn TextMeasure) {
    for i in (0..tree.len()).rev() {
        if let NodeContent::Text(tid) = tree.content[i] {
            // Width is final by now: wrap to it.
            let wrapped = text.wrapped(tid, tree.size[i].w.max(0.0));
            // The wrapped measurement is authoritative for both axes (a long
            // unbroken word may still exceed the clamp; report it truthfully).
            tree.size[i] = wrapped;
            continue;
        }
        let spec = tree.specs[i].layout;
        if let NodeContent::Edit(key) = tree.content[i] {
            // Always wrap to the final content width so emission and input
            // hit the same line layout, whatever the height sizing is.
            let inner = text.edit_wrapped(key, (tree.size[i].w - spec.padding.x()).max(0.0));
            tree.size[i].h = spec.clamp_h(match spec.height {
                Sizing::Fixed(px) => px,
                Sizing::Grow(_) | Sizing::Percent(_) => 0.0,
                Sizing::Fit => inner.h + spec.padding.y(),
            });
            continue;
        }
        tree.size[i].h = spec.clamp_h(match spec.height {
            Sizing::Fixed(px) => px,
            Sizing::Grow(_) | Sizing::Percent(_) => 0.0,
            Sizing::Fit => {
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

    if is_main {
        // Fixed/Fit keep their size, Percent takes its cut, Grow splits the rest.
        let mut used = 0.0f32;
        let mut grow_total = 0.0f32;
        let mut n = 0u32;
        let mut c = tree.first_child[i as usize];
        while c != NIL {
            if is_float(tree, c) {
                c = tree.next_sibling[c as usize];
                continue;
            }
            match child_sizing(tree, c, axis) {
                Sizing::Grow(f) => grow_total += f.max(0.0),
                Sizing::Percent(p) => {
                    set_axis_clamped(tree, c, axis, content * p);
                    used += get_axis(tree, c, axis);
                }
                _ => used += get_axis(tree, c, axis),
            }
            n += 1;
            c = tree.next_sibling[c as usize];
        }
        if n > 1 {
            used += spec.gap * (n - 1) as f32;
        }
        if grow_total > 0.0 {
            let remain = (content - used).max(0.0);
            let mut c = tree.first_child[i as usize];
            while c != NIL {
                if !is_float(tree, c)
                    && let Sizing::Grow(f) = child_sizing(tree, c, axis)
                {
                    set_axis_clamped(tree, c, axis, remain * f.max(0.0) / grow_total);
                }
                c = tree.next_sibling[c as usize];
            }
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
    }

    // Floating children size Grow/Percent against their anchor.
    let mut c = tree.first_child[i as usize];
    while c != NIL {
        if let Some(cfg) = tree.specs[c as usize].layout.float {
            let anchor_dim = match (cfg.anchor, axis) {
                (FloatAnchor::Parent, AxisSel::Width) => tree.size[i as usize].w,
                (FloatAnchor::Parent, AxisSel::Height) => tree.size[i as usize].h,
                (FloatAnchor::Viewport, AxisSel::Width) => viewport.w,
                (FloatAnchor::Viewport, AxisSel::Height) => viewport.h,
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
    // so fit_heights wraps them.
    if axis == AxisSel::Width {
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

fn positions(tree: &mut Tree, scroll: &mut ScrollStore, viewport: Size) {
    for i in 0..tree.len() {
        if tree.parent[i] == NIL {
            tree.pos[i] = Vec2::ZERO;
        }
        let spec = tree.specs[i].layout;
        let origin = tree.pos[i];
        let size = tree.size[i];

        let (main_content, cross_content, main_pad_start, cross_pad_start) = match spec.dir {
            Dir::Row => (size.w - spec.padding.x(), size.h - spec.padding.y(), spec.padding.l, spec.padding.t),
            Dir::Column => (size.h - spec.padding.y(), size.w - spec.padding.x(), spec.padding.t, spec.padding.l),
        };

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

        // Scroll containers: clamp the retained offset to this frame's
        // overflow and shift children by it.
        let mut offset = Vec2::ZERO;
        if spec.scroll_x || spec.scroll_y {
            let (content_w, content_h) = match spec.dir {
                Dir::Row => (total_main + spec.padding.x(), max_cross + spec.padding.y()),
                Dir::Column => (max_cross + spec.padding.x(), total_main + spec.padding.y()),
            };
            let max = Vec2::new(
                if spec.scroll_x { (content_w - size.w).max(0.0) } else { 0.0 },
                if spec.scroll_y { (content_h - size.h).max(0.0) } else { 0.0 },
            );
            tree.scroll_max[i] = max;
            offset = scroll.clamp(tree.keys[i], max);
        }

        let free = (main_content - total_main).max(0.0);
        let mut cursor = main_pad_start
            + match spec.main_align {
                Align::Start => 0.0,
                Align::Center => free / 2.0,
                Align::End => free,
            }
            - match spec.dir {
                Dir::Row => offset.x,
                Dir::Column => offset.y,
            };

        let mut c = tree.first_child[i];
        while c != NIL {
            if let Some(cfg) = tree.specs[c as usize].layout.float {
                // Anchored placement, out of flow.
                let anchor = match cfg.anchor {
                    FloatAnchor::Parent => Rect::from_pos_size(origin, size),
                    FloatAnchor::Viewport => Rect::new(0.0, 0.0, viewport.w, viewport.h),
                };
                let cs = tree.size[c as usize];
                tree.pos[c as usize] = Vec2::new(
                    anchor.x + align_factor(cfg.anchor_point.0) * anchor.w
                        - align_factor(cfg.self_point.0) * cs.w
                        + cfg.offset.x,
                    anchor.y + align_factor(cfg.anchor_point.1) * anchor.h
                        - align_factor(cfg.self_point.1) * cs.h
                        + cfg.offset.y,
                );
                c = tree.next_sibling[c as usize];
                continue;
            }
            let cs = tree.size[c as usize];
            let (c_main, c_cross) = match spec.dir {
                Dir::Row => (cs.w, cs.h),
                Dir::Column => (cs.h, cs.w),
            };
            let cross_free = (cross_content - c_cross).max(0.0);
            let cross_off = cross_pad_start
                + match spec.cross_align {
                    Align::Start => 0.0,
                    Align::Center => cross_free / 2.0,
                    Align::End => cross_free,
                };
            let cross_scroll = match spec.dir {
                Dir::Row => offset.y,
                Dir::Column => offset.x,
            };
            tree.pos[c as usize] = match spec.dir {
                Dir::Row => Vec2::new(origin.x + cursor, origin.y + cross_off - cross_scroll),
                Dir::Column => Vec2::new(origin.x + cross_off - cross_scroll, origin.y + cursor),
            };
            cursor += c_main + spec.gap;
            c = tree.next_sibling[c as usize];
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
            tree.push(NIL, Key::ROOT, OriginId::HOST, root_spec, NodeContent::Container);
            T { tree }
        }

        fn node(&mut self, parent: u32, spec: NodeSpec) -> u32 {
            let key = Key::ROOT.index(self.tree.len() as u64);
            self.tree.push(parent, key, OriginId::HOST, spec, NodeContent::Container)
        }

        fn text(&mut self, parent: u32, chars: u32) -> u32 {
            let key = Key::ROOT.index(self.tree.len() as u64);
            self.tree.push(parent, key, OriginId::HOST, NodeSpec::default(), NodeContent::Text(TextId(chars)))
        }

        fn run(&mut self, vw: f32, vh: f32) {
            let mut scroll = ScrollStore::default();
            compute(&mut self.tree, &mut StubText, &mut scroll, Size::new(vw, vh));
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
        let b = t.node(0, NodeSpec::column().width(Sizing::Grow(1.0)).height(px(10.0)));
        let c = t.node(0, NodeSpec::column().width(Sizing::Grow(2.0)).height(px(10.0)));
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
        let a = t.node(0, NodeSpec::column().width(Sizing::Percent(0.5)).height(Sizing::Percent(1.0)));
        t.run(1000.0, 1000.0);
        assert_eq!(t.size(a), Size::new(90.0, 80.0)); // (200-20)*0.5, (100-20)*1.0
    }

    #[test]
    fn cross_axis_grow_fills_content() {
        let mut t = T::new(NodeSpec::column().width(px(120.0)).height(px(200.0)).pad(8.0));
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
        let mut t = T::new(NodeSpec::row().width(px(300.0)).height(px(100.0)).pad(10.0).gap(5.0));
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
        let mut t = T::new(NodeSpec::column().width(px(100.0)).height(px(100.0)).main_align(Align::End).gap(10.0));
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
        let mut t = T::new(NodeSpec::column().width(px(100.0)).height(px(500.0)).pad(10.0));
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
        let b = t.node(0, NodeSpec::column().width(Sizing::Grow(1.0)).height(px(10.0)));
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
        let a = t.node(0, NodeSpec::column().width(Sizing::Grow(1.0)).max_width(560.0).height(px(10.0)));
        t.run(1000.0, 1000.0);
        assert_eq!(t.size(a).w, 560.0);
        // And tracks the parent when it's smaller than the cap.
        let mut t = T::new(NodeSpec::row().width(px(400.0)).height(px(100.0)));
        let a = t.node(0, NodeSpec::column().width(Sizing::Grow(1.0)).max_width(560.0).height(px(10.0)));
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
        let a = t.node(0, NodeSpec::row().width(Sizing::Percent(0.9)).max_width(300.0).height(px(10.0)));
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
    fn padding_asymmetric() {
        let mut t = T::new(
            NodeSpec::column()
                .width(px(100.0))
                .height(px(100.0))
                .padding(Edges { l: 1.0, r: 2.0, t: 3.0, b: 4.0 }),
        );
        let a = t.node(0, NodeSpec::row().fill());
        t.run(1000.0, 1000.0);
        assert_eq!(t.pos(a), Vec2::new(1.0, 3.0));
        assert_eq!(t.size(a), Size::new(97.0, 93.0));
    }
}
