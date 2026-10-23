//! A faded subtree drawn as one picture (backlog V11, ADR 0056).
//!
//! With the renderer's switch on (`Core::set_opacity_groups`), a node
//! whose own `opacity` is under 1 opens a group as the walk reaches it:
//! a [`QuadKind::GroupBegin`] marker, then every quad its subtree emits
//! in this walk faded only by what lies between it and the group, then a
//! [`QuadKind::GroupEnd`] carrying the group's own fade. The renderer
//! draws the run into a picture and the picture into the frame, faded
//! once, so a label on its fill and two overlapping pieces fade as one.
//!
//! A group is closed by the first node the walk emits outside it — or by
//! the end of the walk — so whatever paints inside its subtree's turn
//! lands in it: an `over_siblings` child painted after its siblings, a
//! departing subtree painted under a live one. A float under the node is
//! emitted with its own layer, later, outside the group, faded per quad
//! by the whole of its opacity as before.
//!
//! The groups the walk closed are finished after the frame is emitted,
//! inner before outer: each one's bounds from what it drew, and the
//! groups not worth a picture — fewer than two things drawn, a backdrop
//! blur inside, nothing on screen — undone into today's fade per quad.

use super::Core;
use crate::Color;
use crate::display::{DisplayList, Quad, QuadKind};
use crate::geom::Rect;

/// A group the walk is inside.
#[derive(Clone, Copy, Debug)]
pub(crate) struct OpenGroup {
    /// The node that opened it, and the end of its subtree: a node the
    /// walk emits outside `root..end` closes it.
    root: u32,
    end: u32,
    /// The node's opacity, its ancestors' with its own: what a quad's
    /// own is divided by inside the group.
    opacity: f32,
    /// The begin marker's index in the display list.
    begin: u32,
}

/// A group the walk closed: its two markers' indices.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Span {
    begin: u32,
    end: u32,
}

impl Core {
    /// Closes every open group that does not hold node `i`, before
    /// anything is painted for `i`: a departing subtree under it, or `i`.
    #[cold]
    #[inline(never)]
    pub(super) fn groups_settle(&mut self, i: usize) {
        while let Some(g) = self.groups.last() {
            if g.root as usize <= i && i < g.end as usize {
                return;
            }
            self.group_close();
        }
    }

    /// The fade to paint node `i` with, from `opacity`, the whole of it
    /// the walk worked out: what lies between `i` and the group it is
    /// in. A node that fades its subtree opens its own group first, and
    /// paints at 1 inside it.
    #[cold]
    #[inline(never)]
    pub(super) fn groups_enter(&mut self, i: usize, opacity: f32) -> f32 {
        self.groups_settle(i);
        let outer = self.groups.last().map_or(1.0, |g| g.opacity);
        if opacity <= 0.0 {
            return 0.0;
        }
        if self.tree.specs[i].style.opacity < 1.0 {
            let begin = self.display.quads.len() as u32;
            self.display.quads.push(marker(
                QuadKind::GroupBegin,
                (opacity / outer).min(1.0),
            ));
            self.groups.push(OpenGroup {
                root: i as u32,
                end: self.tree.subtree_end(i) as u32,
                opacity,
                begin,
            });
            return 1.0;
        }
        (opacity / outer).min(1.0)
    }

    /// Closes the innermost open group with its end marker.
    fn group_close(&mut self) {
        let g = self.groups.pop().expect("closes an open group");
        let end = self.display.quads.len() as u32;
        let begin = &mut self.display.quads[g.begin as usize];
        begin.uv[0] = end;
        let mut q = marker(QuadKind::GroupEnd, begin.color.a);
        q.uv[0] = g.begin;
        self.display.quads.push(q);
        self.group_spans.push(Span {
            begin: g.begin,
            end,
        });
    }

    /// Closes every group still open: the end of a walk.
    pub(super) fn groups_close_all(&mut self) {
        while !self.groups.is_empty() {
            self.group_close();
        }
    }

    /// A departing subtree painted from quad `from` on, inside the open
    /// group: its fade was the whole of its opacity, and inside the group
    /// it is what lies between it and the group.
    pub(super) fn groups_ghost(&mut self, from: usize) {
        let Some(g) = self.groups.last() else {
            return;
        };
        let k = 1.0 / g.opacity;
        for q in &mut self.display.quads[from..] {
            q.color.a = (q.color.a * k).min(1.0);
            q.border_color.a = (q.border_color.a * k).min(1.0);
        }
    }

    /// Every group the frame closed, inner before outer: its bounds, or
    /// undone when a picture would not be worth it.
    pub(super) fn finish_groups(&mut self) {
        for s in &self.group_spans {
            finish(&mut self.display, *s);
        }
        self.group_spans.clear();
    }
}

/// A marker with no rect yet: `finish` gives it the group's.
fn marker(kind: QuadKind, opacity: f32) -> Quad {
    Quad {
        rect: Rect::default(),
        color: Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: opacity,
        },
        border_color: Color::TRANSPARENT,
        radius: [0.0; 4],
        border_w: 0.0,
        blur: 0.0,
        kind,
        // Entry zero clips nothing (`emit_frame` interns it first): what
        // the group drew was clipped as it was drawn.
        clip: 0,
        uv: [0; 4],
    }
}

/// What a quad covers on screen, physical px: its rect through its
/// clip's turn, a mask's own turn about its centre included, cut by the
/// clip.
fn on_screen(dl: &DisplayList, q: &Quad) -> Rect {
    let mut r = q.rect;
    if q.blur != 0.0 && matches!(q.kind, QuadKind::GlyphMask | QuadKind::Image) {
        // A path's mask turns about its centre by `blur` radians (ADR
        // 0041): the circle through its corners holds it at any angle.
        let half = 0.5 * (r.w * r.w + r.h * r.h).sqrt();
        let (cx, cy) = (r.x + r.w * 0.5, r.y + r.h * 0.5);
        r = Rect::new(cx - half, cy - half, half * 2.0, half * 2.0);
    }
    let clip = dl.clip_of(q);
    if !clip.transform.is_identity() {
        r = clip.transform.bounds(r);
    }
    r.intersect(&clip.rect)
}

/// One closed group: kept with its bounds, or undone.
fn finish(dl: &mut DisplayList, s: Span) {
    let (b, e) = (s.begin as usize, s.end as usize);
    let mut drawn = 0usize;
    let mut blur = false;
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    let mut depth = 0u32;
    for q in &dl.quads[b + 1..e] {
        let r = match q.kind {
            QuadKind::GroupBegin => {
                depth += 1;
                continue;
            }
            // A group inside, kept: its picture is one thing drawn.
            QuadKind::GroupEnd => {
                depth -= 1;
                if depth > 0 {
                    continue;
                }
                q.rect
            }
            _ if depth > 0 => continue,
            QuadKind::Backdrop => {
                blur = true;
                continue;
            }
            _ => on_screen(dl, q),
        };
        if r.w <= 0.0 || r.h <= 0.0 {
            continue;
        }
        drawn += 1;
        x0 = x0.min(r.x);
        y0 = y0.min(r.y);
        x1 = x1.max(r.x + r.w);
        y1 = y1.max(r.y + r.h);
    }
    // Whole pixels, so the picture's texels are the frame's.
    let view = dl.viewport;
    let (x0, y0) = (x0.floor().max(0.0), y0.floor().max(0.0));
    let (x1, y1) = (x1.ceil().min(view.w.ceil()), y1.ceil().min(view.h.ceil()));
    // One thing drawn fades the same on its own, and a backdrop blur
    // reads the frame under it, which a picture of its own has not got.
    if drawn < 2 || blur || x1 <= x0 || y1 <= y0 {
        undo(dl, b, e);
        return;
    }
    let bounds = Rect::new(x0, y0, x1 - x0, y1 - y0);
    dl.quads[b].rect = bounds;
    dl.quads[e].rect = bounds;
}

/// A group not worth a picture: its fade into every quad of its run as
/// the walk would have put it there, a kept group inside it by its
/// picture's fade, and its markers made into nothing.
fn undo(dl: &mut DisplayList, b: usize, e: usize) {
    let a = dl.quads[e].color.a;
    let mut depth = 0u32;
    for q in &mut dl.quads[b + 1..e] {
        match q.kind {
            QuadKind::GroupBegin => {
                if depth == 0 {
                    q.color.a *= a;
                }
                depth += 1;
            }
            QuadKind::GroupEnd => {
                depth -= 1;
                if depth == 0 {
                    q.color.a *= a;
                }
            }
            _ if depth == 0 => {
                q.color.a *= a;
                q.border_color.a *= a;
            }
            _ => {}
        }
    }
    for i in [b, e] {
        dl.quads[i] = marker(QuadKind::Solid, 0.0);
    }
}
