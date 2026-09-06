//! Exit transitions: a subtree the view stopped declaring, kept as a
//! picture and played out.
//!
//! [`crate::enter::Enter`] says where a node's slots *start* on its first
//! sight. `NodeSpec::exit` is the same declaration read the other way —
//! where they *end* — and it needs something `enter` does not: the node
//! itself, one frame after the view stopped mentioning it. The frame model
//! is immediate, so nothing about a node survives the frame that dropped
//! it; what survives here is a copy.
//!
//! A node that declared both a `transition` and an `exit`, was in the last
//! frame's tree and is not in this one, becomes a **ghost**: its specs,
//! contents and laid-out rects are copied out of the previous frame's tree
//! into this store, stamped with the clock reading it left at. Every later
//! frame replays it:
//!
//! - **frozen, not re-laid-out.** Rects are the ones layout gave it the
//!   last time it existed. A dying node must not fight the live layout for
//!   space, which is also how CSS's exit transitions work — the element is
//!   out of flow the moment it is removed.
//! - **in its place, and unclipped.** It is painted where the node was in
//!   the paint order — the same pass (in flow, or among the floats) and
//!   just under the node that painted after it — so a panel that sat
//!   under a HUD leaves under it, rather than jumping to the top of the
//!   window for its last few frames. There is no z-index; floats stack in
//!   tree order, and a picture of a float keeps the place it stacked in.
//!   Its ancestors may be gone, so there is no clip to inherit and nothing
//!   to sit inside: it draws outside every clip.
//! - **inert.** No hit region, no place in the Tab ring, no access row. It
//!   is a picture of a node, not a node.
//! - **self-easing.** Nothing can retarget a ghost — the view has already
//!   stopped talking about it — so its slots are one lerp over its own
//!   clock rather than a retained tween in [`crate::anim::AnimStore`].
//!   Spring easings sample as ease-out, the same substitution
//!   `AnimStore::sample` makes for a keyframed slot, since there is no leg
//!   to carry momentum across.
//!
//! A ghost is dropped when its transition ends, when the same key comes
//! back (the live node wins immediately, so a toast dismissed and re-shown
//! does not double), when it has not been replayed for
//! [`EVICT_AFTER_FRAMES`] frames, and — the part that makes this safe for a
//! list — when the store is already holding [`MAX_NODES`] nodes. See
//! `docs/adr/0005-the-paint-vocabulary.md` for why the budget is over
//! nodes and what it costs when it bites.

use crate::anim::Easing;
use crate::color::Color;
use crate::enter::Enter;
use crate::geom::{Rect, Vec2};
use crate::key::Key;
use crate::resources::ImageId;
use crate::spec::{NodeSpec, Sizing};
use crate::tree::{NIL, NodeContent, Tree};

/// The most nodes every departing subtree together may retain. The budget
/// is over *nodes* rather than subtrees because a node is what a replayed
/// frame pays for; a view that drops more than this at once gets the first
/// of them animating out and the rest vanishing at once, which is exactly
/// the behaviour of a node with no `exit` at all.
pub const MAX_NODES: usize = 512;

/// Evict a ghost not replayed for this many frames — the same backstop
/// `AnimStore` keeps, for a driver whose clock stops moving while frames
/// keep coming.
const EVICT_AFTER_FRAMES: u64 = 300;

/// A departing node's content. Same leaves a live node has, in the forms
/// that outlive the frame that made them: a `TextId` indexes the frame's
/// text list, which is rebuilt every frame, so a ghost carries the cache
/// key of the shaped buffer behind it instead.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum GhostContent {
    Container,
    Text {
        cache_key: u64,
        color: Color,
    },
    Edit(Key),
    Image(ImageId),
    /// A stroke: `len` points from `first` in the ghost's own point list
    /// (relative to the node's box, like the live run's), drawn `width`
    /// wide in the colour the node's `bg` slot eases to.
    Line {
        first: u32,
        len: u32,
        width: f32,
    },
}

pub(crate) struct GhostNode {
    /// Index within this ghost, `NIL` for its root.
    pub parent: u32,
    pub spec: NodeSpec,
    pub content: GhostContent,
    /// Where layout left it, logical, in viewport coordinates.
    pub rect: Rect,
}

/// Which emission pass painted a node: in-flow content first, then every
/// floating subtree on top of it (see `Core::emit_frame`). A ghost is
/// painted in the pass its root was, at the place it had.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Pass {
    InFlow,
    Float,
}

/// Where a departing subtree sat in the paint order: the pass, and the key
/// of the first node painted after it in that pass that the frame which
/// noticed it gone still declares. Its ghost is painted just under that
/// node — and at the end of its pass once the node is gone too.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Place {
    pub pass: Pass,
    pub before: Option<Key>,
}

/// One departing subtree, with what it needs to play itself out.
pub(crate) struct Ghost {
    pub key: Key,
    pub nodes: Vec<GhostNode>,
    /// The points of every `line` in the subtree, copied out of the frame
    /// that had them (a `LineId` indexes a list that is rebuilt every
    /// frame). Empty, and unallocated, for a subtree with no lines.
    pub points: Vec<Vec2>,
    /// Where it is painted, relative to the live frame.
    pub place: Place,
    /// Clock reading (driver seconds) of the frame that noticed it gone.
    left_at: f64,
    /// How long the exit runs, in seconds (the root's `transition`).
    duration: f64,
    easing: Easing,
    /// Where the root's slots are headed.
    exit: Enter,
    /// The group opacity the departing root inherited from ancestors that
    /// may no longer exist.
    base_opacity: f32,
    last_used: u64,
}

/// A ghost's root as it should be painted this frame: the eased slots, and
/// the offset to move every rect in the subtree by.
pub(crate) struct Playback {
    pub offset: Vec2,
    pub bg: Option<Color>,
    pub radius: Option<[f32; 4]>,
    pub size: Option<(Option<f32>, Option<f32>)>,
    /// Multiplied into the root's own opacity.
    pub opacity: f32,
    pub base_opacity: f32,
}

impl Ghost {
    /// Where the exit has got to at `now`: `None` once it is over.
    fn playback(&self, now: f64) -> Option<Playback> {
        let raw = ((now - self.left_at) / self.duration) as f32;
        if raw >= 1.0 || raw.is_nan() {
            return None;
        }
        let p = self.easing.apply(raw.max(0.0));
        let root = &self.nodes[0].spec;
        let lerp = |from: f32, to: f32| from + (to - from) * p;
        let e = &self.exit;
        Some(Playback {
            offset: Vec2::new(e.dx * p, e.dy * p),
            bg: e.bg.map(|to| {
                let from = root.style.bg;
                Color {
                    r: lerp(from.r, to.r),
                    g: lerp(from.g, to.g),
                    b: lerp(from.b, to.b),
                    a: lerp(from.a, to.a),
                }
            }),
            radius: e
                .radius
                .map(|to| root.style.radius.map(|from| lerp(from, to))),
            size: (e.width.is_some() || e.height.is_some()).then(|| {
                let rect = self.nodes[0].rect;
                (
                    e.width.and_then(Sizing::amount).map(|to| lerp(rect.w, to)),
                    e.height.and_then(Sizing::amount).map(|to| lerp(rect.h, to)),
                )
            }),
            opacity: match e.opacity {
                Some(to) => lerp(root.style.opacity, to),
                None => root.style.opacity,
            },
            base_opacity: self.base_opacity,
        })
    }
}

/// The departing subtrees, bounded (see [`MAX_NODES`]).
#[derive(Default)]
pub struct DepartStore {
    ghosts: Vec<Ghost>,
    /// Nodes across every ghost, kept in step with `ghosts` so the budget
    /// is a comparison rather than a walk.
    nodes: usize,
    frame_no: u64,
    /// Whether a ghost replayed this frame is still mid-flight.
    active: bool,
    /// A departure the budget refused since the last drain — the view is
    /// asking for more exits at once than the store will hold.
    pub(crate) refused: Option<Key>,
    /// A membership mask over the ghosts' `before` keys, so a departure
    /// whose key no ghost sits under — every row of a mass removal — skips
    /// the walk that would hand its place on. Never cleared: a stale bit
    /// costs one walk, and there are at most a few hundred ghosts to walk.
    before_mask: u64,
}

impl DepartStore {
    pub(crate) fn begin_frame(&mut self) {
        self.frame_no += 1;
        self.active = false;
        if !self.ghosts.is_empty() && self.frame_no.is_multiple_of(240) {
            let cutoff = self.frame_no.saturating_sub(EVICT_AFTER_FRAMES);
            self.drop_where(|g| g.last_used < cutoff);
        }
    }

    /// True when a ghost replayed this frame is still mid-flight, i.e. the
    /// driver owes another frame.
    pub fn animating(&self) -> bool {
        self.active
    }

    /// Whether anything is departing at all — the gate every pass that
    /// would otherwise walk an empty store checks first.
    pub fn is_empty(&self) -> bool {
        self.ghosts.is_empty()
    }

    /// How many nodes are retained across every departing subtree.
    pub fn node_count(&self) -> usize {
        self.nodes
    }

    /// The keys of the departing roots — what a caller tests the live tree
    /// against to notice one coming back.
    pub(crate) fn keys(&self) -> impl Iterator<Item = Key> + '_ {
        self.ghosts.iter().map(|g| g.key)
    }

    fn drop_where(&mut self, mut pred: impl FnMut(&Ghost) -> bool) {
        let nodes = &mut self.nodes;
        self.ghosts.retain(|g| {
            let drop = pred(g);
            if drop {
                *nodes -= g.nodes.len();
            }
            !drop
        });
    }

    /// A key the view declared again: the live node wins and its ghost is
    /// discarded, so a dismissed and re-shown toast does not draw twice.
    pub(crate) fn retire(&mut self, key: Key) {
        self.drop_where(|g| g.key == key);
    }

    /// The same, for every ghost whose key is in `live` — the whole of a
    /// frame's returns in one pass.
    pub(crate) fn retire_returned(&mut self, live: &rustc_hash::FxHashSet<Key>) {
        if !live.is_empty() {
            self.drop_where(|g| live.contains(&g.key));
        }
    }

    /// Copies `root`'s subtree out of `tree` (which is the *previous*
    /// frame's, the last one that had it) and starts its exit at `now`.
    /// `text` is read for that same frame's list, since the subtree's text
    /// nodes carry its ids.
    /// Refused, and remembered as refused, when it would put the store
    /// over [`MAX_NODES`].
    // The two lists at the end are the frame's per-node side tables, read
    // for the same kept frame `tree` is; a struct for the pair would name
    // one thing that only exists here.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn depart(
        &mut self,
        tree: &Tree,
        root: usize,
        now: f64,
        base_opacity: f32,
        place: Place,
        text: &crate::text::TextSystem,
        lines: &crate::line::LineStore,
    ) {
        let spec = &tree.specs[root];
        let (Some(t), Some(exit)) = (spec.transition, spec.anim().exit) else {
            return;
        };
        let duration = t.duration_ms.max(0.0) as f64 / 1000.0;
        if duration <= 0.0 {
            return;
        }
        let end = tree.subtree_end(root);
        let len = end - root;
        if self.nodes + len > MAX_NODES {
            // The first refusal of the frame is the one worth naming; the
            // hundred behind it are the same sentence.
            self.refused.get_or_insert(tree.keys[root]);
            return;
        }
        let key = tree.keys[root];
        // A second departure of the same key (the view showed it, dropped
        // it, showed it and dropped it again inside one exit) replaces the
        // first: two pictures of one node are never right.
        self.retire(key);
        let mut points = Vec::new();
        let nodes = (root..end)
            .map(|i| GhostNode {
                parent: if i == root {
                    NIL
                } else {
                    tree.parent[i] - root as u32
                },
                spec: tree.specs[i].clone(),
                content: match tree.content[i] {
                    NodeContent::Container => GhostContent::Container,
                    NodeContent::Text(id) => {
                        let (cache_key, color) = text.prev_frame_text(id);
                        GhostContent::Text { cache_key, color }
                    }
                    NodeContent::Edit(k) => GhostContent::Edit(k),
                    NodeContent::Image(id) => GhostContent::Image(id),
                    NodeContent::Line(id) => {
                        let (run, pts) = lines.prev_run(id);
                        let first = points.len() as u32;
                        points.extend_from_slice(pts);
                        GhostContent::Line {
                            first,
                            len: pts.len() as u32,
                            width: run.width,
                        }
                    }
                },
                rect: Rect::from_pos_size(tree.pos[i], tree.size[i]),
            })
            .collect::<Vec<_>>();
        self.nodes += nodes.len();
        // A ghost that was painted just under this node loses its place
        // with it, and takes the place this one is taking: the two stay in
        // the order they had, since the store keeps departures in order.
        if self.before_mask & (1u64 << (key.0 & 63)) != 0 {
            for g in &mut self.ghosts {
                if g.place.before == Some(key) {
                    g.place.before = place.before;
                }
            }
        }
        if let Some(before) = place.before {
            self.before_mask |= 1u64 << (before.0 & 63);
        }
        self.ghosts.push(Ghost {
            key,
            nodes,
            points,
            place,
            left_at: now,
            duration,
            easing: t.easing,
            exit,
            base_opacity,
            last_used: self.frame_no,
        });
    }

    /// Starts this frame's replay: drops the ghosts whose exit is over,
    /// and hands the rest out with where each has got to at `now`, taken
    /// out of the store so the emitter can borrow the rest of the core
    /// while it paints them between the live nodes. [`Self::end_replay`]
    /// puts them back.
    pub(crate) fn begin_replay(&mut self, now: f64) -> Replay {
        let frame_no = self.frame_no;
        let nodes = &mut self.nodes;
        let mut plays = Vec::with_capacity(self.ghosts.len());
        let mut mask = 0u64;
        self.ghosts.retain_mut(|g| match g.playback(now) {
            Some(play) => {
                g.last_used = frame_no;
                if let Some(k) = g.place.before {
                    mask |= 1u64 << (k.0 & 63);
                }
                plays.push(play);
                true
            }
            None => {
                *nodes -= g.nodes.len();
                false
            }
        });
        self.active = !self.ghosts.is_empty();
        Replay {
            painted: vec![false; self.ghosts.len()],
            ghosts: std::mem::take(&mut self.ghosts),
            plays,
            mask,
        }
    }

    /// The other half of [`Self::begin_replay`]. Nothing departs between
    /// the two — the diff runs before the passes — so nothing can have
    /// been pushed while the ghosts were out.
    pub(crate) fn end_replay(&mut self, replay: Replay) {
        debug_assert!(self.ghosts.is_empty());
        self.ghosts = replay.ghosts;
    }

    /// Every still-running ghost handed to `emit` in store order, and the
    /// finished ones dropped — the replay without the passes, for a test
    /// that has no frame to paint.
    #[cfg(test)]
    pub(crate) fn replay(&mut self, now: f64, mut emit: impl FnMut(&Ghost, &Playback)) {
        let mut replay = self.begin_replay(now);
        for pass in [Pass::InFlow, Pass::Float] {
            replay.paint(pass, None, &mut emit);
        }
        self.end_replay(replay);
    }

    /// Drops every ghost. The frame driver has no reason to; a test that
    /// wants a clean slate does.
    pub fn clear(&mut self) {
        self.ghosts.clear();
        self.nodes = 0;
        self.active = false;
    }
}

/// One frame's ghosts, out of the store for the length of the emission
/// passes (see [`DepartStore::begin_replay`]). The passes ask for the
/// ghosts under each node as they reach it, and for the rest of a pass
/// once they are through it.
#[derive(Default)]
pub(crate) struct Replay {
    ghosts: Vec<Ghost>,
    plays: Vec<Playback>,
    /// Painted this frame already: a ghost is painted once, whichever of
    /// the two asks finds it first.
    painted: Vec<bool>,
    /// A membership mask over the `before` keys, so a pass answers
    /// "nothing under this node" with one AND for almost every node
    /// rather than a walk over the ghosts.
    mask: u64,
}

impl Replay {
    pub fn is_empty(&self) -> bool {
        self.ghosts.is_empty()
    }

    /// Whether any ghost *may* be painted just under `key`: false is
    /// certain, true is worth the walk [`Self::paint`] makes.
    #[inline]
    pub fn may_precede(&self, key: Key) -> bool {
        self.mask & (1u64 << (key.0 & 63)) != 0
    }

    /// Hands `emit` the ghosts of `pass` painted under `before` — or, for
    /// `None`, every ghost of that pass not painted yet, which is where a
    /// ghost whose place is gone ends up: at the end of its pass, still
    /// under everything the later pass paints.
    pub fn paint(
        &mut self,
        pass: Pass,
        before: Option<Key>,
        mut emit: impl FnMut(&Ghost, &Playback),
    ) {
        for i in 0..self.ghosts.len() {
            let g = &self.ghosts[i];
            if self.painted[i] || g.place.pass != pass {
                continue;
            }
            if before.is_some() && g.place.before != before {
                continue;
            }
            self.painted[i] = true;
            emit(g, &self.plays[i]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::OriginId;

    const IN_FLOW: Place = Place {
        pass: Pass::InFlow,
        before: None,
    };

    fn tree_with(spec: NodeSpec, children: usize) -> Tree {
        let mut t = Tree::new();
        let root = t.push(
            NIL,
            Key::ROOT,
            OriginId::HOST,
            NodeSpec::default(),
            NodeContent::Container,
        );
        let node = t.push(
            root,
            Key::ROOT.str("x"),
            OriginId::HOST,
            spec,
            NodeContent::Container,
        );
        for i in 0..children {
            t.push(
                node,
                Key::ROOT.str("x").index(i as u64),
                OriginId::HOST,
                NodeSpec::default(),
                NodeContent::Container,
            );
        }
        t
    }

    fn departing(spec: NodeSpec) -> NodeSpec {
        spec.transition(100.0)
            .exit(Enter::from(50.0, 0.0).opacity(0.0))
    }

    #[test]
    fn a_ghost_plays_out_and_then_goes() {
        let mut d = DepartStore::default();
        let text = crate::text::TextSystem::new();
        let lines = crate::line::LineStore::default();
        let tree = tree_with(departing(NodeSpec::column()), 2);
        d.begin_frame();
        d.depart(&tree, 1, 0.0, 1.0, IN_FLOW, &text, &lines);
        assert_eq!(d.node_count(), 3, "the subtree, not just its root");

        let mut seen = Vec::new();
        d.begin_frame();
        d.replay(0.05, |_, p| seen.push(p.offset.x));
        assert!(d.animating());
        assert_eq!(seen.len(), 1);
        assert!(seen[0] > 0.0 && seen[0] < 50.0, "halfway out: {}", seen[0]);

        d.begin_frame();
        d.replay(0.2, |_, _| panic!("the exit is over"));
        assert!(!d.animating());
        assert!(d.is_empty());
        assert_eq!(d.node_count(), 0);
    }

    #[test]
    fn a_key_that_comes_back_takes_its_ghost_with_it() {
        let mut d = DepartStore::default();
        let text = crate::text::TextSystem::new();
        let lines = crate::line::LineStore::default();
        let tree = tree_with(departing(NodeSpec::column()), 0);
        d.begin_frame();
        d.depart(&tree, 1, 0.0, 1.0, IN_FLOW, &text, &lines);
        assert_eq!(d.keys().collect::<Vec<_>>(), vec![Key::ROOT.str("x")]);
        d.retire(Key::ROOT.str("x"));
        assert!(d.is_empty());
        assert_eq!(d.node_count(), 0);
    }

    #[test]
    fn a_node_without_both_halves_never_departs() {
        let text = crate::text::TextSystem::new();
        let lines = crate::line::LineStore::default();
        for spec in [
            NodeSpec::column(),
            NodeSpec::column().transition(100.0),
            // An `exit` with no duration to run over.
            NodeSpec::column()
                .exit(Enter::from(10.0, 0.0))
                .transition(0.0),
        ] {
            let mut d = DepartStore::default();
            let tree = tree_with(spec, 0);
            d.begin_frame();
            d.depart(&tree, 1, 0.0, 1.0, IN_FLOW, &text, &lines);
            assert!(d.is_empty());
        }
    }

    #[test]
    fn the_budget_refuses_rather_than_grows() {
        let mut d = DepartStore::default();
        let text = crate::text::TextSystem::new();
        let lines = crate::line::LineStore::default();
        // Subtrees of 16 nodes each: 32 fit, the 33rd does not.
        let tree = tree_with(departing(NodeSpec::column()), 15);
        d.begin_frame();
        for i in 0..40u64 {
            let mut t = Tree::new();
            let root = t.push(
                NIL,
                Key::ROOT,
                OriginId::HOST,
                NodeSpec::default(),
                NodeContent::Container,
            );
            let node = t.push(
                root,
                Key::ROOT.index(i),
                OriginId::HOST,
                tree.specs[1].clone(),
                NodeContent::Container,
            );
            for c in 0..15 {
                t.push(
                    node,
                    Key::ROOT.index(i).index(c),
                    OriginId::HOST,
                    NodeSpec::default(),
                    NodeContent::Container,
                );
            }
            d.depart(&t, 1, 0.0, 1.0, IN_FLOW, &text, &lines);
        }
        assert_eq!(d.node_count(), MAX_NODES);
        assert_eq!(d.keys().count(), MAX_NODES / 16);
        assert_eq!(
            d.refused,
            Some(Key::ROOT.index(32)),
            "and it names the first it refused"
        );
    }

    #[test]
    fn a_ghost_nobody_replays_is_swept() {
        let mut d = DepartStore::default();
        let text = crate::text::TextSystem::new();
        let lines = crate::line::LineStore::default();
        let tree = tree_with(departing(NodeSpec::column()), 0);
        d.begin_frame();
        d.depart(&tree, 1, 0.0, 1.0, IN_FLOW, &text, &lines);
        // Frames without a replay: the sweep runs on the 240th.
        for _ in 0..480 {
            d.begin_frame();
        }
        assert!(d.is_empty(), "an unreplayed ghost does not live forever");
        assert_eq!(d.node_count(), 0);
    }

    #[test]
    fn springs_play_out_as_ease_out() {
        let mut d = DepartStore::default();
        let text = crate::text::TextSystem::new();
        let lines = crate::line::LineStore::default();
        let spec = NodeSpec::column()
            .transition(100.0)
            .easing(Easing::Spring)
            .exit(Enter::from(100.0, 0.0));
        let tree = tree_with(spec, 0);
        d.begin_frame();
        d.depart(&tree, 1, 0.0, 1.0, IN_FLOW, &text, &lines);
        let mut x = 0.0;
        d.replay(0.05, |_, p| x = p.offset.x);
        let expect = Easing::EaseOut.apply(0.5) * 100.0;
        assert!((x - expect).abs() < 1e-3, "{x} != {expect}");
    }

    #[test]
    fn the_exit_reads_an_enter_backwards() {
        let mut d = DepartStore::default();
        let text = crate::text::TextSystem::new();
        let lines = crate::line::LineStore::default();
        let spec = NodeSpec::column()
            .bg(Color::hex(0xff0000ff))
            .radius(10.0)
            .transition(100.0)
            .easing(Easing::Linear)
            .exit(
                Enter::default()
                    .bg(Color::hex(0xff000000))
                    .radius(0.0)
                    .opacity(0.0),
            );
        let mut tree = tree_with(spec, 0);
        tree.size[1] = crate::geom::Size::new(40.0, 20.0);
        d.begin_frame();
        d.depart(&tree, 1, 0.0, 1.0, IN_FLOW, &text, &lines);
        d.replay(0.05, |_, p| {
            assert!((p.opacity - 0.5).abs() < 1e-4, "halfway faded");
            assert!((p.bg.unwrap().a - 0.5).abs() < 1e-4, "halfway transparent");
            assert!((p.radius.unwrap()[0] - 5.0).abs() < 1e-4, "halfway square");
            assert!(p.size.is_none(), "an exit that names no size resizes none");
        });
    }
}
