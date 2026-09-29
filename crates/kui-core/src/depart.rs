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
//! frame's tree and is not in this one — while its parent still is: a node
//! that went with an ancestor goes at once, as the ancestor's subtree does
//! (backlog DX19) — becomes a **ghost**: its specs,
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
//!   to sit inside: it draws outside every clip *they* held. The clips
//!   inside the picture are its own and stay: a scroll box that departs
//!   still bounds the rows it held, so the overscan a virtual list built
//!   past its edge does not appear the frame the list leaves.
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
//! [`crate::retain::KEEP_FOR`] frames, and — the part that makes this safe for a
//! list — when a later frame's removal needs the room it is taking. The
//! store holds at most [`MAX_NODES`] nodes, and the budget is applied to a
//! frame's removal *whole*: the diff counts what the frame wants to add
//! before it copies anything ([`DepartStore::admit`]), evicts the oldest
//! ghosts until the removal fits, and if the removal alone is over the
//! budget refuses all of it, so a list never gets half its rows sliding
//! out and the rest blinking away. See
//! `docs/adr/0005-the-paint-vocabulary.md` for why the budget is over
//! nodes, and `docs/adr/0012-the-exit-budget.md` for why it is judged per
//! frame and what it costs when it bites.

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
/// frame pays for. A frame whose removal is larger than this gets none of
/// it animated — every departing node vanishes at once, which is exactly
/// the behaviour of a node with no `exit` at all — rather than the first
/// of them sliding out and the rest blinking (ADR 0012, decision 2). The
/// number is a decision with a curve behind it: with the departing frame
/// linear (decision 5), and measured again on 2026-09-27 (release, M3 Pro,
/// a pane of rows of a box and a text), a departing subtree costs about
/// 0.065 µs a node in the frame it leaves and 0.021 µs a node a frame while
/// it plays — 4096 nodes are 267 µs, then 87 µs a frame, where the same
/// pane cost 404 µs a frame alive. So a fade costs less than the frames it
/// follows, and the bound is on memory and on a removal nobody meant to
/// animate, not on time. It was 512 until kawoosh's fonts and themes panes
/// (1,500–1,800 nodes) could not fade out (backlog DX23).
pub const MAX_NODES: usize = 4096;

/// Whether a node can depart at all: it declared an `exit`, and a
/// `transition` with a duration to run it over. The diff counts a frame's
/// removal by this before the store copies any of it, and
/// [`DepartStore::depart`] returns early on the same two conditions, so a
/// subtree counted is a subtree copied.
#[inline]
pub(crate) fn can_depart(spec: &NodeSpec) -> bool {
    spec.anim().exit.is_some()
        && spec
            .transition
            .as_ref()
            .is_some_and(|t| t.duration_ms > 0.0)
}

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
    Image(ImageId, crate::resources::ImageOpts),
    /// A stroke: `len` points from `first` in the ghost's own point list
    /// (relative to the node's box, like the live run's), drawn `width`
    /// wide in the colour the node's `bg` slot eases to.
    Line {
        first: u32,
        len: u32,
        width: f32,
    },
    /// A fragment, by value: the handle and the parameters the node last
    /// declared. The ghost re-declares them every frame it draws, so the
    /// picture is frozen at departure while the box eases.
    Fragment(crate::fragment::Draw),
    /// A polygon, by value like a fragment; the fill is the colour the
    /// node's `bg` slot eases to.
    Polygon(crate::fragment::Draw),
}

pub(crate) struct GhostNode {
    /// Index within this ghost, `NIL` for its root.
    pub parent: u32,
    pub spec: NodeSpec,
    pub content: GhostContent,
    /// Where layout left it, logical, in viewport coordinates.
    pub rect: Rect,
}

/// Where a departing subtree sat in the paint order (ADR 0023): the
/// layer, and the key of the first node painted after it in that layer
/// that the frame which noticed it gone still declares. Its ghost is
/// painted just under that node — and at the end of its layer once the
/// node is gone too, or on top of everything once the layer is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Place {
    /// In the in-flow layer, just under `before`; at its end for `None`.
    InFlow { before: Option<Key> },
    /// Inside the float layer rooted at `layer`, just under `before`; at
    /// that layer's end for `None`.
    InLayer { layer: Key, before: Option<Key> },
    /// A float layer of its own, just under the layer rooted at `before`;
    /// on top of the stack for `None`.
    Layer { before: Option<Key> },
}

impl Place {
    /// The key a per-node ask names, for the membership mask: the node a
    /// ghost is painted under, or for one at a layer's end the layer.
    fn mask_key(self) -> Option<Key> {
        match self {
            Place::InFlow { before } | Place::Layer { before } => before,
            Place::InLayer { layer, before } => Some(before.unwrap_or(layer)),
        }
    }

    /// The node (or, for a whole layer, the layer) the ghost is painted
    /// just under; `None` at the end of wherever it is.
    fn before(self) -> Option<Key> {
        match self {
            Place::InFlow { before } | Place::Layer { before } | Place::InLayer { before, .. } => {
                before
            }
        }
    }

    /// The same place, under something else.
    fn with_before(self, before: Option<Key>) -> Self {
        match self {
            Place::InFlow { .. } => Place::InFlow { before },
            Place::InLayer { layer, .. } => Place::InLayer { layer, before },
            Place::Layer { .. } => Place::Layer { before },
        }
    }
}

/// One ask a pass makes of the replay: the slot the emission has reached.
/// The three `Under*` asks are per node and gated by [`Replay::may_precede`];
/// the three `*End`/`Top` asks are per layer and also sweep up every
/// ghost of that layer whose `before` node is gone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum At {
    /// Just under in-flow node `key`.
    UnderInFlow(Key),
    /// The end of the in-flow layer.
    InFlowEnd,
    /// Just under the float layer rooted at `key`.
    UnderLayer(Key),
    /// Just under node `key` inside a float layer.
    UnderInLayer(Key),
    /// The end of the float layer rooted at `key`.
    LayerEnd(Key),
    /// Above every layer: whatever is still unpainted.
    Top,
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
            bg: e.bg.map(|to| root.style.bg.lerp(to, p)),
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
    /// The core's frame counter as of `begin_frame`.
    frame_no: u64,
    /// Whether a ghost replayed this frame is still mid-flight.
    active: bool,
    /// A membership mask over the ghosts' `before` keys, so a departure
    /// whose key no ghost sits under — every row of a mass removal — skips
    /// the walk that would hand its place on. Never cleared: a stale bit
    /// costs one walk, and there are at most a few hundred ghosts to walk.
    before_mask: u64,
    /// The ghosts' own keys, as an exact set kept in step with `ghosts`,
    /// so a departure of a key no ghost holds skips [`Self::retire`] —
    /// which is a pass over the whole store, and a mass removal would
    /// otherwise pay one per row. Exact rather than a 64-bit mask like
    /// `before_mask`: a mass removal is hundreds of distinct keys, which
    /// saturates 64 bits and makes a mask answer "maybe" every time. See
    /// `docs/adr/0012-the-exit-budget.md`, decision 5.
    held: rustc_hash::FxHashSet<Key>,
}

impl DepartStore {
    /// Starts a frame under the core's counter (backlog AR45).
    pub(crate) fn begin_frame(&mut self, frame_no: u64) {
        self.frame_no = frame_no;
        self.active = false;
        // A ghost not replayed for a while goes — the same backstop the
        // anim store keeps, for a driver whose clock stops moving while
        // frames keep coming (`retain::sweep_cutoff`).
        if !self.ghosts.is_empty()
            && let Some(cutoff) = crate::retain::sweep_cutoff(self.frame_no)
        {
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
    pub fn keys(&self) -> impl Iterator<Item = Key> + '_ {
        self.ghosts.iter().map(|g| g.key)
    }

    /// Each departing root's key and the spec it left with — what a
    /// trace names a departure by, since its node is no longer in any
    /// tree (backlog F111).
    pub(crate) fn roots(&self) -> impl Iterator<Item = (Key, &NodeSpec)> + '_ {
        self.ghosts
            .iter()
            .filter_map(|g| g.nodes.first().map(|n| (g.key, &n.spec)))
    }

    fn drop_where(&mut self, mut pred: impl FnMut(&Ghost) -> bool) {
        let nodes = &mut self.nodes;
        let held = &mut self.held;
        self.ghosts.retain(|g| {
            let drop = pred(g);
            if drop {
                *nodes -= g.nodes.len();
                held.remove(&g.key);
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

    /// The budget, applied to a frame's removal whole (ADR 0012, decisions
    /// 2 and 3): `wanted` is every node the frame's departing subtrees
    /// would add together, counted before any of them is copied. A removal
    /// that fits an empty store is admitted — and if the store is holding
    /// earlier exits it has no room beside, the **oldest** ghosts go first
    /// until it fits, since they are the ones furthest through their own
    /// fade and the removal the user just caused is the one they are
    /// looking at. A removal larger than [`MAX_NODES`] on its own is refused
    /// whole: `false`, no ghost, and the caller raises `exit-budget` for
    /// the frame. There is no partial credit either way, so a view never
    /// gets half its rows animating and the rest blinking, and how a
    /// removal reads cannot depend on what else the app happened to be
    /// doing 100 ms earlier.
    pub(crate) fn admit(&mut self, wanted: usize) -> bool {
        if wanted > MAX_NODES {
            return false;
        }
        let mut evict = 0;
        while self.nodes + wanted > MAX_NODES {
            // Store order is departure order, so the front is the oldest.
            let g = &self.ghosts[evict];
            self.nodes -= g.nodes.len();
            self.held.remove(&g.key);
            evict += 1;
        }
        if evict > 0 {
            self.ghosts.drain(..evict);
        }
        true
    }

    /// Copies `root`'s subtree out of `tree` (which is the *previous*
    /// frame's, the last one that had it) and starts its exit at `now`.
    /// `text` is read for that same frame's list, since the subtree's text
    /// nodes carry its ids.
    /// The budget is not checked here: the caller has counted the frame's
    /// whole removal and had it admitted ([`Self::admit`]) before copying
    /// any of it, so a subtree that reaches this always fits.
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
        fragments: &crate::fragment::FragmentList,
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
        let key = tree.keys[root];
        // A second departure of the same key (the view showed it, dropped
        // it, showed it and dropped it again inside one exit) replaces the
        // first: two pictures of one node are never right. Behind `held`
        // because `retire` walks the whole store: unguarded, a frame that
        // drops a thousand rows pays one walk per row, which is the one
        // quadratic left in a mass removal — 32 ms for 10k subtrees
        // against 1.05 ms guarded, and 44% of the departing frame at
        // today's budget. Kept rather than deleted even though the frame
        // path cannot reach it — `collect_departures` retires a returning
        // key before it can depart again — because that argument runs
        // through another module's early returns, and a set lookup is a
        // cheap thing to be wrong about.
        if self.held.contains(&key) {
            self.retire(key);
        }
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
                    NodeContent::Image(id, opts) => GhostContent::Image(id, opts),
                    // A departing grid is its box: the cells are the
                    // frame's and go with it.
                    NodeContent::Cells(_) => GhostContent::Container,
                    // A departing fragment keeps painting. Its draw is
                    // sixteen floats and a handle, fixed-size and `Copy`,
                    // so the ghost owns a copy outright rather than
                    // indexing a list that has to outlive the frame —
                    // which is what the fixed sixteen buys. The copy comes
                    // from the previous frame's list, because that is the
                    // tree this ghost is being cut out of.
                    NodeContent::Fragment(id) => match fragments.prev_get(id) {
                        Some(draw) => GhostContent::Fragment(draw),
                        None => GhostContent::Container,
                    },
                    NodeContent::Polygon(id) => match fragments.prev_get(id) {
                        Some(draw) => GhostContent::Polygon(draw),
                        None => GhostContent::Container,
                    },
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
        debug_assert!(
            self.nodes + nodes.len() <= MAX_NODES,
            "a departure the frame did not have admitted"
        );
        self.nodes += nodes.len();
        // A ghost that was painted just under this node loses its place
        // with it, and takes the place this one is taking: the two stay in
        // the order they had, since the store keeps departures in order.
        if self.before_mask & (1u64 << (key.0 & 63)) != 0 {
            for g in &mut self.ghosts {
                if g.place.before() == Some(key) {
                    g.place = g.place.with_before(place.before());
                }
            }
        }
        if let Some(before) = place.before() {
            self.before_mask |= 1u64 << (before.0 & 63);
        }
        self.held.insert(key);
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
        let held = &mut self.held;
        let mut plays = Vec::with_capacity(self.ghosts.len());
        let mut mask = 0u64;
        self.ghosts.retain_mut(|g| match g.playback(now) {
            Some(play) => {
                g.last_used = frame_no;
                if let Some(k) = g.place.mask_key() {
                    mask |= 1u64 << (k.0 & 63);
                }
                plays.push(play);
                true
            }
            None => {
                *nodes -= g.nodes.len();
                held.remove(&g.key);
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
        // `held` is what lets `depart` skip the walk, so it has to be the
        // ghosts' keys exactly: a key missing from it is a retire that
        // will not happen, which is two pictures of one node. Checked here
        // because every frame that replays passes through.
        debug_assert_eq!(self.held.len(), self.ghosts.len());
    }

    /// Every still-running ghost handed to `emit` in store order, and the
    /// finished ones dropped — the replay without the passes, for a test
    /// that has no frame to paint.
    #[cfg(test)]
    pub(crate) fn replay(&mut self, now: f64, mut emit: impl FnMut(&Ghost, &Playback)) {
        let mut replay = self.begin_replay(now);
        replay.paint(At::Top, &mut emit);
        self.end_replay(replay);
    }

    /// Drops every ghost. The frame driver has no reason to; a test that
    /// wants a clean slate does.
    pub fn clear(&mut self) {
        self.ghosts.clear();
        self.held.clear();
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

    /// Hands `emit` the ghosts whose place is `at` — and, for a layer's
    /// end, every ghost of that layer not painted yet, which is where a
    /// ghost whose `before` node is gone ends up: at the end of its layer,
    /// still under everything above it. `At::Top` takes what is left,
    /// which is a ghost whose whole layer is gone.
    pub fn paint(&mut self, at: At, mut emit: impl FnMut(&Ghost, &Playback)) {
        for i in 0..self.ghosts.len() {
            if self.painted[i] {
                continue;
            }
            let here = match (at, self.ghosts[i].place) {
                (At::UnderInFlow(k), Place::InFlow { before }) => before == Some(k),
                (At::InFlowEnd, Place::InFlow { .. }) => true,
                (At::UnderLayer(k), Place::Layer { before }) => before == Some(k),
                (At::UnderInLayer(k), Place::InLayer { before, .. }) => before == Some(k),
                (At::LayerEnd(k), Place::InLayer { layer, .. }) => layer == k,
                (At::Top, _) => true,
                _ => false,
            };
            if !here {
                continue;
            }
            self.painted[i] = true;
            emit(&self.ghosts[i], &self.plays[i]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The core's frame counter, stood in for: each test's frames count
    /// from one.
    struct Frames(u64);
    impl Frames {
        fn next(&mut self) -> u64 {
            self.0 += 1;
            self.0
        }
    }
    use crate::tree::OriginId;

    const IN_FLOW: Place = Place::InFlow { before: None };

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
        let mut frame = Frames(0);
        let text = crate::text::TextSystem::new();
        let lines = crate::line::LineStore::default();
        let fragments = crate::fragment::FragmentList::default();
        let tree = tree_with(departing(NodeSpec::column()), 2);
        d.begin_frame(frame.next());
        d.depart(&tree, 1, 0.0, 1.0, IN_FLOW, &text, &lines, &fragments);
        assert_eq!(d.node_count(), 3, "the subtree, not just its root");

        let mut seen = Vec::new();
        d.begin_frame(frame.next());
        d.replay(0.05, |_, p| seen.push(p.offset.x));
        assert!(d.animating());
        assert_eq!(seen.len(), 1);
        assert!(seen[0] > 0.0 && seen[0] < 50.0, "halfway out: {}", seen[0]);

        d.begin_frame(frame.next());
        d.replay(0.2, |_, _| panic!("the exit is over"));
        assert!(!d.animating());
        assert!(d.is_empty());
        assert_eq!(d.node_count(), 0);
    }

    #[test]
    fn a_key_that_comes_back_takes_its_ghost_with_it() {
        let mut d = DepartStore::default();
        let mut frame = Frames(0);
        let text = crate::text::TextSystem::new();
        let lines = crate::line::LineStore::default();
        let fragments = crate::fragment::FragmentList::default();
        let tree = tree_with(departing(NodeSpec::column()), 0);
        d.begin_frame(frame.next());
        d.depart(&tree, 1, 0.0, 1.0, IN_FLOW, &text, &lines, &fragments);
        assert_eq!(d.keys().collect::<Vec<_>>(), vec![Key::ROOT.str("x")]);
        d.retire(Key::ROOT.str("x"));
        assert!(d.is_empty());
        assert_eq!(d.node_count(), 0);
    }

    /// One key departing twice with no return in between: the second
    /// picture replaces the first rather than joining it. The frame path
    /// cannot produce this — `collect_departures` retires a returning key
    /// first — so this is the only cover the `retire` inside `depart` has,
    /// and it is what says the `held` guard (ADR 0012 decision 5) does not
    /// skip a retire it owed.
    #[test]
    fn a_second_departure_of_one_key_replaces_the_first() {
        let mut d = DepartStore::default();
        let mut frame = Frames(0);
        let text = crate::text::TextSystem::new();
        let lines = crate::line::LineStore::default();
        let fragments = crate::fragment::FragmentList::default();
        let tree = tree_with(departing(NodeSpec::column()), 2);
        d.begin_frame(frame.next());
        d.depart(&tree, 1, 0.0, 1.0, IN_FLOW, &text, &lines, &fragments);
        assert_eq!(d.keys().count(), 1);
        assert_eq!(d.node_count(), 3);

        d.depart(&tree, 1, 0.05, 1.0, IN_FLOW, &text, &lines, &fragments);
        assert_eq!(d.keys().count(), 1, "one picture of one node, not two");
        assert_eq!(d.node_count(), 3, "and the budget charged once for it");
    }

    #[test]
    fn a_node_without_both_halves_never_departs() {
        let text = crate::text::TextSystem::new();
        let lines = crate::line::LineStore::default();
        let fragments = crate::fragment::FragmentList::default();
        for spec in [
            NodeSpec::column(),
            NodeSpec::column().transition(100.0),
            // An `exit` with no duration to run over.
            NodeSpec::column()
                .exit(Enter::from(10.0, 0.0))
                .transition(0.0),
        ] {
            assert!(!can_depart(&spec), "and the diff never counts it");
            let mut d = DepartStore::default();
            let mut frame = Frames(0);
            let tree = tree_with(spec, 0);
            d.begin_frame(frame.next());
            d.depart(&tree, 1, 0.0, 1.0, IN_FLOW, &text, &lines, &fragments);
            assert!(d.is_empty());
        }
    }

    /// `count` subtrees of 16 nodes each, keyed `ROOT[i]` from `from`,
    /// departed one after another at `now` — the way `collect_departures`
    /// feeds a frame's admitted removal to the store.
    fn depart_sixteens(d: &mut DepartStore, from: u64, count: u64, now: f64) {
        let text = crate::text::TextSystem::new();
        let lines = crate::line::LineStore::default();
        let fragments = crate::fragment::FragmentList::default();
        let spec = departing(NodeSpec::column());
        for i in from..from + count {
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
                spec.clone(),
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
            d.depart(&t, 1, now, 1.0, IN_FLOW, &text, &lines, &fragments);
        }
    }

    /// ADR 0012 decision 2: a removal is judged whole. One larger than the
    /// budget is refused before anything is copied, and the store is
    /// exactly what it was.
    #[test]
    fn a_removal_over_the_budget_is_refused_whole() {
        let mut d = DepartStore::default();
        let mut frame = Frames(0);
        d.begin_frame(frame.next());
        assert!(d.admit(3 * 16));
        depart_sixteens(&mut d, 0, 3, 0.0);
        d.begin_frame(frame.next());
        assert!(!d.admit(MAX_NODES + 1), "one node past the budget");
        assert_eq!(d.keys().count(), 3, "and the store was not touched");
        assert_eq!(d.node_count(), 48);
        assert!(d.admit(MAX_NODES), "the budget itself fits an empty store");
    }

    /// ADR 0012 decision 3: a removal that fits the budget but not the
    /// room beside earlier exits evicts those, oldest first, and exactly
    /// as many as it needs.
    #[test]
    fn a_new_removal_evicts_the_oldest_ghosts_until_it_fits() {
        let mut d = DepartStore::default();
        let mut frame = Frames(0);
        // Subtrees of 16 in flight, keyed ROOT[0..n], filling the store to
        // 192 short of the budget.
        let n = (MAX_NODES - 192) / 16;
        d.begin_frame(frame.next());
        assert!(d.admit(n * 16));
        depart_sixteens(&mut d, 0, n as u64, 0.0);
        assert_eq!(d.node_count(), MAX_NODES - 192);
        // A frame wants 15 more of 16 = 240, 48 past the budget, so three
        // ghosts have to go, and they are ROOT[0], [1], [2].
        d.begin_frame(frame.next());
        assert!(d.admit(15 * 16));
        assert_eq!(
            d.node_count(),
            MAX_NODES - 192 - 48,
            "three evicted, not four, not two"
        );
        assert_eq!(
            d.keys().next(),
            Some(Key::ROOT.index(3)),
            "the oldest went first"
        );
        depart_sixteens(&mut d, 100_000, 15, 0.1);
        assert_eq!(
            d.node_count(),
            MAX_NODES,
            "full, with the new removal whole"
        );
        assert_eq!(d.keys().count(), n - 3 + 15);
        assert!(d.held.contains(&Key::ROOT.index(100_014)));
        assert!(!d.held.contains(&Key::ROOT.index(2)), "and `held` followed");
    }

    /// The case decisions 2 and 3 exist to leave alone: a removal that fits
    /// beside what is in flight evicts nothing.
    #[test]
    fn a_removal_that_fits_evicts_nothing() {
        let mut d = DepartStore::default();
        let mut frame = Frames(0);
        d.begin_frame(frame.next());
        assert!(d.admit(16));
        depart_sixteens(&mut d, 0, 1, 0.0);
        d.begin_frame(frame.next());
        assert!(d.admit(MAX_NODES - 16));
        assert_eq!(d.keys().count(), 1);
        assert_eq!(d.node_count(), 16);
        assert!(d.admit(0), "and nothing wanted is always admitted");
    }

    #[test]
    fn a_ghost_nobody_replays_is_swept() {
        let mut d = DepartStore::default();
        let mut frame = Frames(0);
        let text = crate::text::TextSystem::new();
        let lines = crate::line::LineStore::default();
        let fragments = crate::fragment::FragmentList::default();
        let tree = tree_with(departing(NodeSpec::column()), 0);
        d.begin_frame(frame.next());
        d.depart(&tree, 1, 0.0, 1.0, IN_FLOW, &text, &lines, &fragments);
        // Frames without a replay: the sweep runs on the 240th.
        for _ in 0..480 {
            d.begin_frame(frame.next());
        }
        assert!(d.is_empty(), "an unreplayed ghost does not live forever");
        assert_eq!(d.node_count(), 0);
    }

    #[test]
    fn springs_play_out_as_ease_out() {
        let mut d = DepartStore::default();
        let mut frame = Frames(0);
        let text = crate::text::TextSystem::new();
        let lines = crate::line::LineStore::default();
        let fragments = crate::fragment::FragmentList::default();
        let spec = NodeSpec::column()
            .transition(100.0)
            .easing(Easing::Spring)
            .exit(Enter::from(100.0, 0.0));
        let tree = tree_with(spec, 0);
        d.begin_frame(frame.next());
        d.depart(&tree, 1, 0.0, 1.0, IN_FLOW, &text, &lines, &fragments);
        let mut x = 0.0;
        d.replay(0.05, |_, p| x = p.offset.x);
        let expect = Easing::EaseOut.apply(0.5) * 100.0;
        assert!((x - expect).abs() < 1e-3, "{x} != {expect}");
    }

    #[test]
    fn the_exit_reads_an_enter_backwards() {
        let mut d = DepartStore::default();
        let mut frame = Frames(0);
        let text = crate::text::TextSystem::new();
        let lines = crate::line::LineStore::default();
        let fragments = crate::fragment::FragmentList::default();
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
        d.begin_frame(frame.next());
        d.depart(&tree, 1, 0.0, 1.0, IN_FLOW, &text, &lines, &fragments);
        d.replay(0.05, |_, p| {
            assert!((p.opacity - 0.5).abs() < 1e-4, "halfway faded");
            assert!((p.bg.unwrap().a - 0.5).abs() < 1e-4, "halfway transparent");
            assert!((p.radius.unwrap()[0] - 5.0).abs() < 1e-4, "halfway square");
            assert!(p.size.is_none(), "an exit that names no size resizes none");
        });
    }
}
