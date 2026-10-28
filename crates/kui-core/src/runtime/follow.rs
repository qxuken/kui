//! A held drag following its scroller.
//!
//! Two things a caret drag or a drag-select needs beyond the press, the
//! motion and the release, both of them about the frame moving under a
//! pointer that did not: when the layout under the held pointer changes
//! — a wheel notch, a scroller nudged, a virtual list re-sliced — the
//! live end is placed again where the pointer is; and when
//! the pointer is held in the edge zone of the scroller or past it, the
//! core is what changes the layout, at the edge scroll's feel
//! (`EdgeScrollFeel`: the depth, how long it is held, a cap). Both run at
//! the start of every frame against the frame that finished, so the
//! frame being built paints the result.
//!
//! The scroller is found once, at the press: the nearest ancestor-or-self
//! of the pressed node that scrolls, or that declared `on_scroll` — the
//! second kind is nudged as an event, since the core holds nothing of
//! what it scrolls (a `cells` grid is one screenful of the app's history;
//! decision 4). From a container the edge step hands on, per axis: one
//! that cannot move the way its zone scrolls — it does not scroll that
//! way, it is at its end, the pointer is clear of its zones — gives that
//! axis to the container around it, as the wheel chains, unless it says
//! `overscroll: contain` (ADR 0029, amended 2026-10-25). `Ui::edge_scroll`
//! hands on the same way.

use super::*;
use crate::edge_scroll::EdgeRamp;

/// The frame a core with no clock takes, in seconds — a sixtieth, so a
/// headless drive steps by a knowable amount. A clockless core does not
/// snap here: there is no end state to snap to.
pub const CLOCKLESS_FRAME: f64 = 1.0 / 60.0;
/// The longest frame the edge scroll is applied over: a window that sat
/// hidden for a second must not scroll a second's worth when it comes
/// back.
pub(crate) const MAX_FRAME: f64 = 0.1;

/// What a held drag scrolls, and what its live end was last placed
/// against.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Scroller {
    /// A scroll container: the nudge is an offset the layout clamps, and
    /// the gate is the offset the last layout placed the content at.
    Container { key: Key, seen: Vec2 },
    /// An `on_scroll` node: the nudge is a `scroll` event the app answers
    /// by re-declaring. For a `cells` grid the gate is the `origin_line`
    /// it declared; any other handler moves what it likes, so the gate
    /// is every frame while the drag is held.
    Handler { key: Key, seen: Option<u64> },
}

impl Scroller {
    fn key(self) -> Key {
        match self {
            Scroller::Container { key, .. } | Scroller::Handler { key, .. } => key,
        }
    }
}

/// A held drag and what it follows: the pointer's last position, kept
/// across a `CursorLeft` (a press dragged out of the window gets one on
/// some platforms, and the drag is still held); the scroller; whether
/// the last frame stepped it, which is what asks for the next frame
/// (`Core::animating`); and the edge scroll's ramp.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DragFollow {
    pub point: Vec2,
    pub scroller: Option<Scroller>,
    pub stepped: bool,
    /// The container the last step moved — the scroller or one it
    /// handed an axis to: who holds the next frame in a trace.
    pub stepped_key: Option<Key>,
    /// The innermost scroller inside the scroller that was under the
    /// pointer when the hand last moved it: a box the drag was brought
    /// into, which its edge step starts from — and not one the content
    /// carried under a pointer held still.
    pub entered: Option<Key>,
    /// A handler scroller's ramp; a container's are kept per container
    /// by the hand-off (`Core::step_edge_chain`).
    pub ramp: EdgeRamp,
}

/// What an axis of a held drag scrolls this frame: the container and the
/// rect its zones are measured against.
pub(crate) type EdgePick = Option<(Key, Rect)>;

/// How close to its limit a container has to be to count as there: a
/// held drag steps the last sub-pixel too, so a page ends where its end
/// is, where a wheel notch counts half a pixel as none.
const AT_LIMIT: f32 = 0.001;

impl Core {
    /// Arms the follow for a drag a press just started on `node` at
    /// `point`: finds the scroller and records where it is now.
    pub(crate) fn arm_follow(&mut self, node: Key, point: Vec2) {
        let scroller = self.scroller_of(node);
        self.drag_follow = Some(DragFollow {
            point,
            scroller,
            stepped: false,
            stepped_key: None,
            entered: None,
            ramp: EdgeRamp::default(),
        });
    }

    /// The innermost scroll container under the pointer that is `scope`
    /// or inside it, by the finished frame: where a drag-select pressed
    /// there is followed from.
    pub(crate) fn scroller_under_in(&self, scope: Key) -> Option<Key> {
        let start = self.tree.index_of(scope)?;
        let end = self.tree.subtree_end(start);
        self.interaction
            .scroll_regions_at()
            .filter(|r| !r.handler)
            .map(|r| r.key)
            .find(|&k| {
                self.tree
                    .index_of(k)
                    .is_some_and(|i| (start..end).contains(&i))
            })
    }

    /// The pointer moved and the caller placed the live end there against
    /// the finished frame: the scroller's reading is taken again, so the
    /// next frame does not place the end a second time for a move the
    /// frame already answered.
    pub(crate) fn follow_point(&mut self, point: Vec2) {
        let Some(mut f) = self.drag_follow else {
            return;
        };
        f.point = point;
        f.scroller = f.scroller.map(|s| self.reading_of(s.key(), s));
        f.entered = f.scroller.and_then(|s| self.scroller_under_in(s.key()));
        self.drag_follow = Some(f);
    }

    /// The drag ended: the live end is placed once more where the pointer
    /// is, against the finished frame — a release right after a nudge
    /// lands where the pointer is, not a frame behind — and the follow
    /// is dropped. The drags themselves are the caller's to clear.
    pub(crate) fn end_follow(&mut self) {
        if let Some(f) = self.drag_follow.take() {
            self.rehit(f.point);
        }
    }

    /// Whether a held drag stepped its scroller this frame — one more
    /// frame is wanted with nothing in the queue.
    pub(crate) fn autoscrolling(&self) -> bool {
        self.drag_follow.is_some_and(|f| f.stepped)
    }

    /// The scroller a held drag stepped this frame, when it did: who
    /// [`Self::autoscrolling`] names in a trace.
    pub(crate) fn autoscroller(&self) -> Option<Key> {
        self.drag_follow
            .filter(|f| f.stepped)
            .and_then(|f| f.stepped_key.or(f.scroller.map(Scroller::key)))
    }

    /// The nearest ancestor-or-self of `node` that a held drag scrolls,
    /// with its current reading. Self first, because a `uniform_list`
    /// is both the selection's scope and its scroller. A handler beats
    /// a container on the same node: the app asked to hear the wheel
    /// (`ScrollRegion::handler`). Read off the finished frame's tree,
    /// which is what a press between two frames sees; a press during a
    /// build has no scroller.
    fn scroller_of(&self, node: Key) -> Option<Scroller> {
        if self.building {
            return None;
        }
        let mut i = self.tree.index_of(node)?;
        loop {
            let key = self.tree.keys[i];
            let spec = &self.tree.specs[i];
            if spec.events().on_scroll().is_some() {
                return Some(self.reading_of(key, Scroller::Handler { key, seen: None }));
            }
            if spec.layout.scroll_x || spec.layout.scroll_y {
                return Some(self.reading_of(
                    key,
                    Scroller::Container {
                        key,
                        seen: Vec2::ZERO,
                    },
                ));
            }
            match self.tree.parent[i] {
                NIL => return None,
                p => i = p as usize,
            }
        }
    }

    /// The scroller `key`, of `kind`'s kind, at what the finished frame
    /// placed it: the one reading `scroller_of` takes, the gate compares
    /// against, and a placement refreshes.
    fn reading_of(&self, key: Key, kind: Scroller) -> Scroller {
        match kind {
            Scroller::Container { .. } => Scroller::Container {
                key,
                seen: self.scroll.laid_offset(key).unwrap_or(Vec2::ZERO),
            },
            Scroller::Handler { .. } => Scroller::Handler {
                key,
                seen: self.grid_line(key),
            },
        }
    }

    /// The `origin_line` the finished frame's grid `key` declared, if the
    /// key is a grid.
    fn grid_line(&self, key: Key) -> Option<u64> {
        self.cells_id_of_ref(key)
            .map(|id| self.cells.origin_line(id, self.building))
    }

    /// The row height of the grid `key`, logical px — what a wheel delta
    /// or an edge step is divided by to become lines.
    fn grid_line_height(&mut self, key: Key) -> Option<f32> {
        let id = self.cells_id_of_ref(key)?;
        let sess = &mut *self.session.state();
        let cell = self
            .cells
            .cell_size(id, self.building, &sess.resources, &mut sess.fonts);
        (cell.h > 0.0).then_some(cell.h)
    }

    /// Places the held drag's live end at `p` against the finished frame,
    /// exactly as a `CursorMoved` there would: the caret for a caret
    /// drag, the selection's focus for a drag-select, in whichever
    /// geometry its scope has.
    pub(crate) fn rehit(&mut self, p: Vec2) {
        // Inside the click slop a press is a click: its caret, or its
        // selection's two ends together, stay where the press put them,
        // whatever slid under the pointer (backlog F170, F173).
        if !self.left_slop(p) {
            return;
        }
        if let Some((key, _)) = self.edit.dragging {
            self.edit_drag_to(key, p);
        }
        if let Some(drag) = self.select_dragging {
            self.extend_select_drag(drag, p);
        }
    }

    /// Whether a drag held at `p` has left the click slop of its press,
    /// and so places its live end: once it has, it has for the rest of
    /// the press — a drag that wanders back is still a drag. A press that
    /// never leaves it is a click, and the release, the frames while it
    /// is held and the pointer's shake under it place nothing. The slop
    /// is the pointer's, against the window: a pane a strip scrolls into
    /// view under the held button moves its text, not the hand.
    fn left_slop(&mut self, p: Vec2) -> bool {
        let Some(at) = self.press_slop else {
            return true;
        };
        if (p.x - at.x).abs() + (p.y - at.y).abs() <= crate::input::DRAG_SLOP {
            return false;
        }
        self.press_slop = None;
        true
    }

    /// Moves the caret drag in editor `key` to the viewport point `p`,
    /// against the origin the editor was drawn at this frame — not the
    /// press's, which a nudge has moved.
    pub(crate) fn edit_drag_to(&mut self, key: Key, p: Vec2) {
        let origin = self
            .interaction
            .edit_origin_of(key)
            .or_else(|| self.edit.dragging.map(|(_, o)| o));
        if let Some(origin) = origin {
            let q = self.unturned(key, p);
            let local = Vec2::new(q.x - origin.x, q.y - origin.y);
            self.edit_with_fonts(|edit, fs| edit.drag(key, local, fs));
        }
    }

    /// At the start of a frame, against the one that finished: re-places
    /// the live end if the scroller moved since it was last placed, then
    /// steps the scroller if the pointer is held in its edge zone or past
    /// it. Nothing while no drag is held, and no step while the press is
    /// still inside its click slop: a click near an edge scrolls nothing.
    pub(crate) fn follow_drag(&mut self) {
        let now = self.anim.time();
        let dt = match (now, self.last_frame_time) {
            (Some(n), Some(l)) => (n - l).clamp(0.0, MAX_FRAME),
            (Some(_), None) | (None, _) => CLOCKLESS_FRAME,
        } as f32;
        self.last_frame_time = now;
        let Some(mut f) = self.drag_follow else {
            return;
        };
        // A step the last frame took may have moved a container the
        // scroller handed an axis to, which its own reading misses.
        let was_stepped = f.stepped;
        f.stepped = false;
        f.stepped_key = None;
        if let Some(scroller) = f.scroller {
            // 1. The frame under the pointer moved: place the end again.
            let key = scroller.key();
            let now = self.reading_of(key, scroller);
            let moved = was_stepped
                || match scroller {
                    Scroller::Container { .. } => now != scroller,
                    Scroller::Handler { seen, .. } => seen.is_none() || now != scroller,
                };
            if moved {
                self.rehit(f.point);
                f.scroller = Some(now);
            }
            // 2. The pointer is in the edge zone: step the scroller.
            let still = self.press_slop.is_some();
            if let Some(i) = self.tree.index_of(key).filter(|_| !still) {
                let rect = Rect::from_pos_size(self.tree.pos[i], self.tree.size[i]);
                let window = Rect::new(0.0, 0.0, self.viewport.w, self.viewport.h);
                let feel = self.edge_feel;
                f.stepped = match scroller {
                    Scroller::Container { .. } => {
                        // Per axis, the scroller or the container it hands
                        // that axis to. A step the clamp would undo is no
                        // step: a scroller at its end under a held pointer
                        // is idle, not asking for frames until the release.
                        // From the innermost scroller under the pointer
                        // inside this one — a code block the drag has come
                        // into from the text around it, held at its edge,
                        // while it is still the one the hand brought it
                        // into rather than one that scrolled under a still
                        // pointer — or this one: the press's scroller is
                        // where the walk ends up, not the only place it
                        // starts.
                        let inner = self
                            .scroller_under_in(key)
                            .filter(|&k| k != key && f.entered == Some(k))
                            .and_then(|k| self.tree.index_of(k).map(|j| (k, j)));
                        let (from, rect) = match inner {
                            Some((k, j)) => {
                                (k, Rect::from_pos_size(self.tree.pos[j], self.tree.size[j]))
                            }
                            None => (key, rect),
                        };
                        let picks = self.edge_chain(from, rect, f.point, window, |r| r);
                        let (moved, by) = self.step_edge_chain(picks, f.point, window, dt);
                        f.stepped_key = by;
                        moved != Vec2::ZERO
                    }
                    Scroller::Handler { .. } => {
                        // A grid scrolls lines and has no columns to
                        // scroll to; any other handler hears both axes.
                        let grid = self.grid_line(key).is_some();
                        let delta = f.ramp.step(&feel, rect, window, f.point, (!grid, true), dt);
                        if delta != Vec2::ZERO {
                            // The event's delta is the wheel's: positive
                            // `dy` is earlier content, so a step toward
                            // later content — the pointer below — is
                            // negative.
                            let wheel = Vec2::new(-delta.x, -delta.y);
                            if let Some(ev) = self.scroll_event(key, f.point, wheel) {
                                self.pending.push(ev);
                            }
                            true
                        } else {
                            false
                        }
                    }
                };
            }
        }
        self.drag_follow = Some(f);
    }

    /// The containers a held drag with the pointer at `p` scrolls this
    /// frame, per axis (`[x, y]`), walking out from the container
    /// `start` through the finished frame's scroll regions — the wheel's,
    /// which a build under way leaves whole — `start`'s zones measured
    /// against `first`, every other container's against `rect_of` its
    /// region's rect: on each axis the first container that scrolls along
    /// it, has the pointer in its edge zone and room the way that zone
    /// scrolls. One at its end, or with the pointer clear of its zones,
    /// hands the axis to the container around it, as the wheel chains;
    /// one that says `overscroll: contain` keeps the axis whether or not
    /// it can move. The walk ends at a handler, whose room the core cannot
    /// read, and at a modal's edge (ADR 0029, amended 2026-10-25).
    pub(crate) fn edge_chain(
        &self,
        start: Key,
        first: Rect,
        p: Vec2,
        window: Rect,
        rect_of: impl Fn(Rect) -> Rect,
    ) -> [EdgePick; 2] {
        let feel = self.edge_feel;
        let regions = &self.interaction.scroll_regions;
        let region = |k: Key| regions.iter().rposition(|r| r.key == k);
        // A scroller out of view has no region this frame — a code block
        // the page it hands on to has scrolled away: the walk starts at
        // the nearest container around it that still has one, by the
        // finished frame's tree, and the drag goes on to the page's end.
        let (start, own) = match region(start) {
            Some(i) => (i, true),
            None => {
                let mut at = self.tree.index_of(start);
                let mut found = None;
                while let Some(i) = at {
                    let p = self.tree.parent[i];
                    if p == NIL {
                        break;
                    }
                    if let Some(j) = region(self.tree.keys[p as usize]) {
                        found = Some(j);
                        break;
                    }
                    at = Some(p as usize);
                }
                match found {
                    Some(j) => (j, false),
                    None => return [None, None],
                }
            }
        };
        let mut picks = [None, None];
        for (axis, pick) in picks.iter_mut().enumerate() {
            let x = axis == 0;
            let mut i = start;
            loop {
                let r = &regions[i];
                if i != start && (r.handler || r.inert) {
                    break;
                }
                if if x { r.scrolls_x } else { r.scrolls_y } {
                    let rect = if i == start && own {
                        first
                    } else {
                        rect_of(r.rect)
                    };
                    let way = crate::edge_scroll::zone_way(&feel, rect, window, p, x);
                    let room = way != 0.0
                        && self.scroll.geometry(r.key).is_some_and(|g| {
                            let (o, m) = if x {
                                (g.offset.x, g.max_offset.x)
                            } else {
                                (g.offset.y, g.max_offset.y)
                            };
                            if way > 0.0 {
                                o < m - AT_LIMIT
                            } else {
                                o > AT_LIMIT
                            }
                        });
                    if room {
                        *pick = Some((r.key, rect));
                        break;
                    }
                    if r.contain {
                        break;
                    }
                }
                if r.parent == NIL {
                    break;
                }
                i = r.parent as usize;
            }
        }
        picks
    }

    /// Steps the containers `picks` names by the edge scroll's feel over
    /// `dt` seconds — each on the axes it was picked for, with a ramp of
    /// its own that starts again after a frame it was not stepped in —
    /// and returns what moved, per axis, and the first container that
    /// did.
    pub(crate) fn step_edge_chain(
        &mut self,
        picks: [EdgePick; 2],
        p: Vec2,
        window: Rect,
        dt: f32,
    ) -> (Vec2, Option<Key>) {
        let frame = self.frame_no;
        self.edge_ramps.retain(|e| e.3 + 1 >= frame);
        let jobs: Vec<(Key, Rect, (bool, bool))> = match picks {
            [Some((a, r)), Some((b, _))] if a == b => vec![(a, r, (true, true))],
            _ => picks
                .iter()
                .enumerate()
                .filter_map(|(axis, pick)| pick.map(|(k, r)| (k, r, (axis == 0, axis == 1))))
                .collect(),
        };
        let feel = self.edge_feel;
        let now = self.anim.time();
        let mut moved = Vec2::ZERO;
        let mut by = None;
        for (key, rect, axes) in jobs {
            let at = self.edge_ramps.iter().position(|e| e.0 == key);
            let mut ramp = at.map_or_else(EdgeRamp::default, |j| self.edge_ramps[j].1);
            let delta = ramp.step(&feel, rect, window, p, axes, dt);
            let entry = (key, ramp, now, frame);
            match at {
                Some(j) => self.edge_ramps[j] = entry,
                None => self.edge_ramps.push(entry),
            }
            let Some(g) = self.scroll.geometry(key) else {
                continue;
            };
            // What the clamp leaves of the step: at the end that way, none.
            let go =
                |o: f32, d: f32, m: f32| (o + d).clamp(0.0, m.max(0.0)) - o.clamp(0.0, m.max(0.0));
            let d = Vec2::new(
                go(g.offset.x, delta.x, g.max_offset.x),
                go(g.offset.y, delta.y, g.max_offset.y),
            );
            if d != Vec2::ZERO {
                self.scroll.scroll_by(key, d);
                moved = Vec2::new(moved.x + d.x, moved.y + d.y);
                by = by.or(Some(key));
            }
        }
        (moved, by)
    }

    /// The `scroll` event a delta over the `on_scroll` node `key` makes —
    /// the wheel's, or an edge step's — with the whole lines it covers
    /// on a grid. The fraction left over is carried to the next delta on
    /// the same grid, whichever of the two made it (`line_carry`): a
    /// notch's half line and an edge step's add up. `None` when the node
    /// is not in the finished frame or declared no `on_scroll`.
    pub(crate) fn scroll_event(&mut self, key: Key, p: Vec2, delta: Vec2) -> Option<UiEvent> {
        let i = self.tree.index_of(key)?;
        self.tree.specs[i].events().on_scroll()?;
        let lines = match self.grid_line_height(key) {
            Some(h) => {
                let carry = match self.line_carry {
                    Some((k, c)) if k == key => c,
                    _ => 0.0,
                };
                // Later history is the wheel rolling down: negative `dy`.
                let total = carry - delta.y / h;
                let whole = total.trunc();
                self.line_carry = Some((key, total - whole));
                Value::Int(whole as i64)
            }
            None => Value::Null,
        };
        let origin = self.tree.origins[i];
        let tag = self.tree.specs[i].events().on_scroll();
        let data = Value::map([
            ("x", Value::Float(p.x as f64)),
            ("y", Value::Float(p.y as f64)),
            ("dx", Value::Float(delta.x as f64)),
            ("dy", Value::Float(delta.y as f64)),
            ("lines", lines),
        ]);
        Some(UiEvent::declared(EventKind::Scroll, origin, key, tag, data))
    }

    /// The `scroll` a pinch is on `key`'s `on_scroll` (backlog F256): the
    /// pointer, no delta, no lines — a grid's carried fraction stays as
    /// it was — and the magnification as `zoom`.
    pub(crate) fn pinch_event(&self, key: Key, p: Vec2, zoom: f32) -> Option<UiEvent> {
        let i = self.tree.index_of(key)?;
        let tag = self.tree.specs[i].events().on_scroll();
        tag?;
        let data = Value::map([
            ("x", Value::Float(p.x as f64)),
            ("y", Value::Float(p.y as f64)),
            ("dx", Value::Float(0.0)),
            ("dy", Value::Float(0.0)),
            ("lines", Value::Null),
            ("zoom", Value::Float(zoom as f64)),
        ]);
        Some(UiEvent::declared(
            EventKind::Scroll,
            self.tree.origins[i],
            key,
            tag,
            data,
        ))
    }
}
