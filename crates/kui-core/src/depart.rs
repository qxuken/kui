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
//! - **on top, and unclipped.** It is emitted like a float, after every
//!   live quad: its ancestors may be gone, so there is no clip to inherit
//!   and nothing to sit inside.
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
    Text { cache_key: u64, color: Color },
    Edit(Key),
    Image(ImageId),
}

pub(crate) struct GhostNode {
    /// Index within this ghost, `NIL` for its root.
    pub parent: u32,
    pub spec: NodeSpec,
    pub content: GhostContent,
    /// Where layout left it, logical, in viewport coordinates.
    pub rect: Rect,
}

/// One departing subtree, with what it needs to play itself out.
pub(crate) struct Ghost {
    pub key: Key,
    pub nodes: Vec<GhostNode>,
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
    pub(crate) fn depart(
        &mut self,
        tree: &Tree,
        root: usize,
        now: f64,
        base_opacity: f32,
        text: &crate::text::TextSystem,
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
                },
                rect: Rect::from_pos_size(tree.pos[i], tree.size[i]),
            })
            .collect::<Vec<_>>();
        self.nodes += nodes.len();
        self.ghosts.push(Ghost {
            key,
            nodes,
            left_at: now,
            duration,
            easing: t.easing,
            exit,
            base_opacity,
            last_used: self.frame_no,
        });
    }

    /// Hands each still-running ghost to `emit` with where its exit has got
    /// to at `now`, and drops the ones that have finished. Ghosts are taken
    /// out of the store for the call so the emitter can borrow the rest of
    /// the core.
    pub(crate) fn replay(&mut self, now: f64, mut emit: impl FnMut(&Ghost, &Playback)) {
        let frame_no = self.frame_no;
        let mut ghosts = std::mem::take(&mut self.ghosts);
        let mut active = false;
        let nodes = &mut self.nodes;
        ghosts.retain_mut(|g| {
            let Some(play) = g.playback(now) else {
                *nodes -= g.nodes.len();
                return false;
            };
            g.last_used = frame_no;
            active = true;
            emit(g, &play);
            true
        });
        self.active = active;
        // `replay` is the only place that adds nothing, so nothing can have
        // been pushed while it ran.
        self.ghosts = ghosts;
    }

    /// Drops every ghost. The frame driver has no reason to; a test that
    /// wants a clean slate does.
    pub fn clear(&mut self) {
        self.ghosts.clear();
        self.nodes = 0;
        self.active = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::OriginId;

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
        let tree = tree_with(departing(NodeSpec::column()), 2);
        d.begin_frame();
        d.depart(&tree, 1, 0.0, 1.0, &text);
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
        let tree = tree_with(departing(NodeSpec::column()), 0);
        d.begin_frame();
        d.depart(&tree, 1, 0.0, 1.0, &text);
        assert_eq!(d.keys().collect::<Vec<_>>(), vec![Key::ROOT.str("x")]);
        d.retire(Key::ROOT.str("x"));
        assert!(d.is_empty());
        assert_eq!(d.node_count(), 0);
    }

    #[test]
    fn a_node_without_both_halves_never_departs() {
        let text = crate::text::TextSystem::new();
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
            d.depart(&tree, 1, 0.0, 1.0, &text);
            assert!(d.is_empty());
        }
    }

    #[test]
    fn the_budget_refuses_rather_than_grows() {
        let mut d = DepartStore::default();
        let text = crate::text::TextSystem::new();
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
            d.depart(&t, 1, 0.0, 1.0, &text);
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
        let tree = tree_with(departing(NodeSpec::column()), 0);
        d.begin_frame();
        d.depart(&tree, 1, 0.0, 1.0, &text);
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
        let spec = NodeSpec::column()
            .transition(100.0)
            .easing(Easing::Spring)
            .exit(Enter::from(100.0, 0.0));
        let tree = tree_with(spec, 0);
        d.begin_frame();
        d.depart(&tree, 1, 0.0, 1.0, &text);
        let mut x = 0.0;
        d.replay(0.05, |_, p| x = p.offset.x);
        let expect = Easing::EaseOut.apply(0.5) * 100.0;
        assert!((x - expect).abs() < 1e-3, "{x} != {expect}");
    }

    #[test]
    fn the_exit_reads_an_enter_backwards() {
        let mut d = DepartStore::default();
        let text = crate::text::TextSystem::new();
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
        d.depart(&tree, 1, 0.0, 1.0, &text);
        d.replay(0.05, |_, p| {
            assert!((p.opacity - 0.5).abs() < 1e-4, "halfway faded");
            assert!((p.bg.unwrap().a - 0.5).abs() < 1e-4, "halfway transparent");
            assert!((p.radius.unwrap()[0] - 5.0).abs() < 1e-4, "halfway square");
            assert!(p.size.is_none(), "an exit that names no size resizes none");
        });
    }
}
