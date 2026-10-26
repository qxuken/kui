//! A departing node's room, kept until its exit ends (backlog C110).
//!
//! A ghost is frozen at its last rects and takes no room in the live
//! layout (`crate::depart`), so a toast that leaves a stack lets the rest
//! close up the frame it goes, under it while it is still fading out. An
//! exit that says `keep_room` keeps the room instead: while its ghost
//! plays, an empty box of the node's laid size stands in its place among
//! its siblings, and when the ghost is done the box goes and the siblings
//! move in — gliding, where they `slide`.
//!
//! The box is pushed while the view builds, so the frame the node departs
//! in is already laid out with it: at the end of every frame the core
//! notes each in-flow node whose exit keeps its room — its parent, the
//! sibling after it and its size — and the next build, opening that
//! sibling under that parent without having seen the node, pushes the box
//! just before it (or at the parent's close, for a last child). A room
//! lasts as long as its ghost; a node that comes back takes its place
//! itself.

use super::*;
use crate::geom::Size;

/// One node whose exit keeps its room: noted at a frame's end while it is
/// in the tree, then kept while its ghost plays.
#[derive(Clone, Debug)]
pub(crate) struct Room {
    /// Its parent's key, where the room is kept.
    parent: Key,
    /// The node's own key — its ghost's.
    node: Key,
    /// The key of the sibling that followed it (that sibling's own node
    /// key, when the sibling is itself a kept room), `None` for a last
    /// child: the room goes just before it.
    before: Option<Key>,
    /// The size layout last gave the node.
    size: Size,
    /// Whether the node has departed: its room is a box now.
    departed: bool,
    /// This build: the node itself was declared, so nothing departed.
    seen: bool,
    /// This build: its box was pushed.
    placed: bool,
}

impl Room {
    /// The key of the box that stands in for `node`.
    fn box_key(node: Key) -> Key {
        node.str("\u{0}kui-room")
    }
}

impl Core {
    /// As a build begins: every room unseen and unplaced.
    pub(crate) fn rooms_begin_frame(&mut self) {
        for r in &mut self.rooms {
            r.seen = false;
            r.placed = false;
        }
    }

    /// Before node `key` is pushed under the node at `parent`: it is seen,
    /// if it is one a room is noted for, and every room kept just before
    /// it is pushed first. One emptiness check while nothing keeps a room.
    #[inline]
    pub(super) fn rooms_before_push(&mut self, parent: u32, key: Key) {
        if !self.rooms.is_empty() {
            self.place_rooms_before(parent, key);
        }
    }

    #[cold]
    #[inline(never)]
    fn place_rooms_before(&mut self, parent: u32, key: Key) {
        let Some(&parent_key) = self.tree.keys.get(parent as usize) else {
            return;
        };
        for r in &mut self.rooms {
            if r.node == key && r.parent == parent_key {
                r.seen = true;
            }
        }
        self.place_before(parent, parent_key, Some(key));
    }

    /// As node `at` closes: the rooms kept after its last child, and any
    /// of its rooms whose sibling never came — at the end, where they are
    /// nearest to right.
    #[inline]
    pub(super) fn rooms_before_close(&mut self, at: u32) {
        if !self.rooms.is_empty() {
            self.place_rooms_at_close(at);
        }
    }

    #[cold]
    #[inline(never)]
    fn place_rooms_at_close(&mut self, at: u32) {
        let Some(&parent_key) = self.tree.keys.get(at as usize) else {
            return;
        };
        self.place_before(at, parent_key, None);
        let left: Vec<Key> = self
            .rooms
            .iter()
            .filter(|r| r.parent == parent_key && !r.seen && !r.placed)
            .map(|r| r.node)
            .collect();
        for node in left {
            self.push_room(at, node);
        }
    }

    /// Pushes, under `parent`, the box of every unseen room kept just
    /// before `before` — the rooms kept before each of those first, so a
    /// run of departures keeps its order.
    fn place_before(&mut self, parent: u32, parent_key: Key, before: Option<Key>) {
        let due: Vec<Key> = self
            .rooms
            .iter()
            .filter(|r| r.parent == parent_key && r.before == before && !r.seen && !r.placed)
            .map(|r| r.node)
            .collect();
        for node in due {
            self.place_before(parent, parent_key, Some(node));
            self.push_room(parent, node);
        }
    }

    /// The box standing in for departed `node` under `parent`: as large as
    /// the node was laid, empty, inert, and the core's own, so a kept fill
    /// it lands in does not count it.
    fn push_room(&mut self, parent: u32, node: Key) {
        let Some(r) = self.rooms.iter_mut().find(|r| r.node == node) else {
            return;
        };
        if r.placed {
            return;
        }
        r.placed = true;
        let size = r.size;
        let spec = NodeSpec::column().size(size.w, size.h);
        self.pause_keeping();
        self.tree.push(
            parent,
            Room::box_key(node),
            self.origin,
            spec,
            NodeContent::Container,
        );
        self.resume_keeping();
    }

    /// At a frame's end, after emission has cut this frame's departures
    /// out as ghosts: the rooms kept on — each departed node whose ghost
    /// still plays, its sibling read again off this frame's tree — and the
    /// rooms noted afresh for every in-flow node whose exit keeps its room.
    pub(crate) fn settle_rooms(&mut self) {
        if self.rooms.is_empty() && !self.tree.any_keep_room {
            return;
        }
        let rooms = std::mem::take(&mut self.rooms);
        let mut kept: Vec<Room> = Vec::new();
        for mut r in rooms {
            // Gone this frame or before, with a box for it in the tree and
            // a ghost still playing; a node that came back is noted below
            // as any live one is.
            if r.seen || !r.placed || !self.depart.holds(r.node) {
                continue;
            }
            r.departed = true;
            if let Some(i) = self.tree.index_of(Room::box_key(r.node)) {
                r.before = self.sibling_after(i);
            }
            kept.push(r);
        }
        if self.tree.any_keep_room {
            for i in 0..self.tree.len() {
                let spec = &self.tree.specs[i];
                if !spec.anim().exit.is_some_and(|e| e.keep_room)
                    || !crate::depart::can_depart(spec)
                    || spec.layout.float.is_some()
                    || self.tree.parent[i] == NIL
                {
                    continue;
                }
                kept.push(Room {
                    parent: self.tree.keys[self.tree.parent[i] as usize],
                    node: self.tree.keys[i],
                    before: self.sibling_after(i),
                    size: self.tree.size[i],
                    departed: false,
                    seen: false,
                    placed: false,
                });
            }
        }
        self.rooms = kept;
    }

    /// The key a room before node `i`'s next sibling names: that sibling's
    /// own, or — for a box standing in for a departed node — that node's.
    fn sibling_after(&self, i: usize) -> Option<Key> {
        let next = self.tree.next_sibling[i];
        if next == NIL {
            return None;
        }
        let key = self.tree.keys[next as usize];
        Some(
            self.rooms
                .iter()
                .find(|r| Room::box_key(r.node) == key)
                .map_or(key, |r| r.node),
        )
    }

    /// Whether a room is kept for departed `node` this frame — for the
    /// tests and the devtools.
    pub fn keeps_room(&self, node: Key) -> bool {
        self.rooms.iter().any(|r| r.node == node && r.departed)
    }
}
