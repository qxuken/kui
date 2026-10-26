//! The paint audit (ADR 0074): every quad a frame drew, checked against
//! the clip of the nearest box above its node that clips or scrolls.
//!
//! The rule is that what a node and its subtree paint is cut by that box,
//! whatever the node's float, move, keyframe or turn, unless the node
//! declares it leaves — a viewport float, or a float anchored to its
//! parent that says `escape` (a menu, a popover, a tooltip). The clip walk
//! in `emit` is what keeps it; this is what checks it, from the outside:
//! it reads the draw list a renderer reads, each quad cut by the clip it
//! names, and the boxes above its node as layout placed them, and owes
//! nothing to how the walk got there. A test aid, off by default
//! (`Core::set_paint_audit`): on, emission notes which node each run of
//! quads belongs to, and the frame's end raises a `paint-escape` warning
//! for each node that drew outside its box.

use super::*;
use crate::display::{NO_CLIP, Quad, QuadKind};
use crate::geom::Rect;

/// A quad drawn outside the clip its node is held by (ADR 0074): what
/// [`Core::paint_audit`] reports.
#[derive(Clone, Debug, PartialEq)]
pub struct PaintEscape {
    /// The node that drew it.
    pub node: Key,
    /// The nearest box above it that clips or scrolls.
    pub clipper: Key,
    /// Where the quad shows, cut by the clip it names, logical px.
    pub drawn: Rect,
    /// What the boxes above the node leave it, logical px.
    pub allowed: Rect,
    /// The quad's kind.
    pub kind: QuadKind,
    /// The node, by the key labels from it up (`plot / slide3`), or its
    /// content where it has none.
    pub path: String,
}

/// What the audit counted since it was turned on.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PaintAudit {
    /// Frames audited.
    pub frames: u64,
    /// Quads checked: drawn by a node that is held by a clip.
    pub quads: u64,
    /// Every escape found, the first per node.
    pub escapes: Vec<PaintEscape>,
}

/// Physical px a quad may stand past its box before it counts: the
/// rounding of a snapped edge, never a ring or a line.
const SLACK: f32 = 0.5;

impl Core {
    /// Turns the paint audit on or off (ADR 0074): while on, every frame's
    /// quads are checked against the clip of the nearest box above their
    /// node that clips or scrolls, and one that lands outside it raises a
    /// `paint-escape` warning naming the node. A kui-native driver turns
    /// it on for `KUI_PAINT_AUDIT=1`. A test aid: it costs a span per node
    /// and a walk per frame, and is off by default.
    pub fn set_paint_audit(&mut self, on: bool) {
        self.paint_audit = on.then(Box::default);
        self.audit_spans = on.then(Vec::new);
    }

    /// What the audit has counted, `None` while it is off.
    pub fn paint_audit(&self) -> Option<&PaintAudit> {
        self.paint_audit.as_deref()
    }

    /// Notes that the quads from here on are node `node`'s (`NIL`: no
    /// node's — a ghost's, a join's — and not audited).
    #[inline]
    pub(super) fn audit_span(&mut self, node: u32) {
        if let Some(a) = self.audit_spans.as_mut() {
            a.push((self.display.quads.len() as u32, node));
        }
    }

    /// At the frame's end: the quads checked, run by run.
    #[cold]
    #[inline(never)]
    pub(super) fn audit_frame(&mut self) {
        let Some(spans) = self.audit_spans.take() else {
            return;
        };
        let scale = self.scale;
        let quads = &self.display.quads;
        let mut checked = 0u64;
        let mut found: Vec<(usize, PaintEscape)> = Vec::new();
        // The allowed rect per node, found once a node.
        let mut allowed: FxHashMap<u32, Option<(Rect, u32)>> = FxHashMap::default();
        for (k, &(from, node)) in spans.iter().enumerate() {
            let to = spans.get(k + 1).map_or(quads.len(), |s| s.0 as usize);
            if node == NIL || from as usize >= to {
                continue;
            }
            let Some((room, clipper)) = *allowed
                .entry(node)
                .or_insert_with(|| self.audit_allowed(node as usize))
            else {
                continue;
            };
            let room_px = room.scaled(scale);
            for q in &quads[from as usize..to] {
                if matches!(q.kind, QuadKind::GroupBegin | QuadKind::GroupEnd) {
                    continue;
                }
                let drawn = drawn_px(q, &self.display);
                if drawn.w <= 0.0 || drawn.h <= 0.0 {
                    continue;
                }
                checked += 1;
                let inside = drawn.x >= room_px.x - SLACK
                    && drawn.y >= room_px.y - SLACK
                    && drawn.x + drawn.w <= room_px.x + room_px.w + SLACK
                    && drawn.y + drawn.h <= room_px.y + room_px.h + SLACK;
                if !inside && !found.iter().any(|(n, _)| *n == node as usize) {
                    let i = node as usize;
                    found.push((
                        i,
                        PaintEscape {
                            node: self.tree.keys[i],
                            clipper: self.tree.keys[clipper as usize],
                            drawn: Rect::new(
                                drawn.x / scale,
                                drawn.y / scale,
                                drawn.w / scale,
                                drawn.h / scale,
                            ),
                            allowed: room,
                            kind: q.kind,
                            path: self.audit_path(i),
                        },
                    ));
                }
            }
        }
        // The buffer back for the next frame, emptied.
        let mut spans = spans;
        spans.clear();
        self.audit_spans = Some(spans);
        for (_, e) in &found {
            let msg = format!(
                "{} drew a {:?} at {:.1},{:.1} {:.1}×{:.1} outside the box it is in ({:.1},{:.1} \
                 {:.1}×{:.1}, node {:016x}): what a node paints is cut by the nearest box above it \
                 that clips or scrolls (ADR 0074); a node that means to leave it says so — a \
                 viewport float, or `escape` on a float anchored to its parent",
                e.path,
                e.kind,
                e.drawn.x,
                e.drawn.y,
                e.drawn.w,
                e.drawn.h,
                e.allowed.x,
                e.allowed.y,
                e.allowed.w,
                e.allowed.h,
                e.clipper.0,
            );
            self.diag
                .raise_once(crate::diag::PAINT_ESCAPE, e.node, || msg.clone());
        }
        if let Some(a) = self.paint_audit.as_mut() {
            a.frames += 1;
            a.quads += checked;
            for (_, e) in found {
                if !a.escapes.iter().any(|x| x.node == e.node) {
                    a.escapes.push(e);
                }
            }
        }
    }

    /// The rect the boxes above node `i` leave what it draws, logical px,
    /// and the nearest of them; `None` when nothing above it clips, when
    /// it declares it leaves, or under a turn (whose rects are in a space
    /// of their own; the core's tests check turns). Walked up from the
    /// node through every box that clips or scrolls, stopping at a float
    /// that does not take its parent's clip — a node under a menu is held
    /// by the menu's own scroller, and not by what the menu left.
    fn audit_allowed(&self, i: usize) -> Option<(Rect, u32)> {
        if self.clips.get(i).is_some_and(|c| c.turned()) {
            return None;
        }
        let mut room = NO_CLIP;
        let mut nearest = NIL;
        let mut x = i;
        loop {
            // What a node declares, not what the walk did with it: a
            // float anchored to its parent is held, as a child is.
            if self.tree.specs[x]
                .layout
                .float
                .is_some_and(|f| f.anchor != crate::spec::FloatAnchor::Parent)
            {
                break;
            }
            let p = self.tree.parent[x];
            if p == NIL {
                break;
            }
            let p = p as usize;
            if self.tree.specs[p].layout.clips() {
                let r = Rect::from_pos_size(self.tree.pos[p], self.tree.size[p]);
                room = room.intersect(&r);
                if nearest == NIL {
                    nearest = p as u32;
                }
            }
            x = p;
        }
        (nearest != NIL).then_some((room, nearest))
    }

    /// Node `i` by the key labels on the way up, nearest first, joined
    /// outermost first; its content's kind when none has one.
    fn audit_path(&self, i: usize) -> String {
        let mut names: Vec<&str> = Vec::new();
        let mut x = i as u32;
        while x != NIL && names.len() < 4 {
            if let Some(l) = self.label_of(self.tree.keys[x as usize]) {
                names.push(l);
            }
            x = self.tree.parent[x as usize];
        }
        let kind = match self.tree.content[i] {
            NodeContent::Text(_) => "text",
            NodeContent::Line(_) => "line",
            NodeContent::Polygon(_) => "polygon",
            NodeContent::Path(_) => "path",
            NodeContent::Image(..) => "image",
            NodeContent::Container => "box",
            _ => "leaf",
        };
        names.reverse();
        if names.is_empty() {
            format!("a {kind}")
        } else {
            format!("a {kind} under {}", names.join(" / "))
        }
    }
}

/// Where a quad shows, physical px: its rect cut by its clip's.
fn drawn_px(q: &Quad, display: &crate::display::DisplayList) -> Rect {
    let clip = display.clip_of(q);
    q.rect.intersect(&clip.rect)
}
