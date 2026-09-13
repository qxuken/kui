//! A held drag following its scroller
//! (`docs/adr/0029-a-selection-follows-the-pointer-past-the-edge.md`).
//!
//! Two things a caret drag or a drag-select needs beyond the press, the
//! motion and the release, both of them about the frame moving under a
//! pointer that did not: when the layout under the held pointer changes
//! — a wheel notch, a scroller nudged, a virtual list re-sliced — the
//! live end is placed again where the pointer is (decision 1); and when
//! the pointer is held past the scroller's edge, the core is what changes
//! the layout, at a rate from how far past (decision 2). Both run at the
//! start of every frame against the frame that finished, so the frame
//! being built paints the result.
//!
//! The scroller is found once, at the press: the nearest ancestor-or-self
//! of the pressed node that scrolls, or that declared `on_scroll` — the
//! second kind is nudged as an event, since the core holds nothing of
//! what it scrolls (a `cells` grid is one screenful of the app's history;
//! decision 4).

use super::*;

/// Past this many logical px beyond the edge the step stops speeding up.
pub const AUTOSCROLL_REACH: f32 = 100.0;
/// Logical px per second of scrolling, per logical px past the edge: 60
/// px past is 600 px/s, the reach is 1000 px/s.
pub const AUTOSCROLL_RATE: f32 = 10.0;
/// The frame a core with no clock takes, in seconds — a sixtieth, so a
/// headless drive steps by a knowable amount. A clockless core does not
/// snap here: there is no end state to snap to.
pub const CLOCKLESS_FRAME: f64 = 1.0 / 60.0;
/// The longest frame the rate is applied over: a window that sat hidden
/// for a second must not scroll a second's worth when it comes back.
const MAX_FRAME: f64 = 0.1;

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

/// A held drag and what it follows: the pointer's last position, kept
/// across a `CursorLeft` (a press dragged out of the window gets one on
/// some platforms, and the drag is still held); the scroller; and the
/// fraction of a line an edge step has not covered yet, for a grid.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DragFollow {
    pub point: Vec2,
    pub scroller: Option<Scroller>,
    pub carry: f32,
}

impl Core {
    /// Arms the follow for a drag a press just started on `node` at
    /// `point`: finds the scroller and records where it is now.
    pub(crate) fn arm_follow(&mut self, node: Key, point: Vec2) {
        let scroller = self.scroller_of(node);
        self.drag_follow = Some(DragFollow {
            point,
            scroller,
            carry: 0.0,
        });
    }

    /// The pointer moved while the drag is held.
    pub(crate) fn follow_point(&mut self, point: Vec2) {
        if let Some(f) = &mut self.drag_follow {
            f.point = point;
        }
    }

    /// The drag ended: the live end is placed once more where the pointer
    /// is, against the finished frame — a release right after a nudge
    /// lands where the pointer is, not a frame behind — and the follow
    /// is dropped. The drags themselves are the caller's to clear.
    pub(crate) fn end_follow(&mut self) {
        if let Some(f) = self.drag_follow.take() {
            self.rehit(f.point);
        }
        self.autoscrolling = false;
    }

    /// The nearest ancestor-or-self of `node` that a held drag scrolls,
    /// with its current reading. Self first, because a `virtual_column`
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
            if spec.events().on_scroll.is_some() {
                return Some(Scroller::Handler {
                    key,
                    seen: self.grid_line(key),
                });
            }
            if spec.layout.scroll_x || spec.layout.scroll_y {
                return Some(Scroller::Container {
                    key,
                    seen: self.scroll.laid_offset(key).unwrap_or(Vec2::ZERO),
                });
            }
            match self.tree.parent[i] {
                NIL => return None,
                p => i = p as usize,
            }
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
        if let Some((key, _)) = self.edit.dragging {
            self.edit_drag_to(key, p);
        }
        if let Some(drag) = self.select_dragging {
            self.extend_select_drag(drag, p);
        }
    }

    /// Moves the caret drag in editor `key` to the viewport point `p`,
    /// against the origin the editor was drawn at this frame — not the
    /// press's, which a nudge has moved (decision 2).
    pub(crate) fn edit_drag_to(&mut self, key: Key, p: Vec2) {
        let origin = self
            .interaction
            .edit_origin_of(key)
            .or_else(|| self.edit.dragging.map(|(_, o)| o));
        if let Some(origin) = origin {
            let local = Vec2::new(p.x - origin.x, p.y - origin.y);
            self.edit_with_fonts(|edit, fs| edit.drag(key, local, fs));
        }
    }

    /// At the start of a frame, against the one that finished: re-places
    /// the live end if the scroller moved since it was last placed, then
    /// steps the scroller if the pointer is held past its edge. Nothing
    /// while no drag is held.
    pub(crate) fn follow_drag(&mut self) {
        let now = self.anim.time();
        let dt = match (now, self.last_frame_time) {
            (Some(n), Some(l)) => (n - l).clamp(0.0, MAX_FRAME),
            (Some(_), None) | (None, _) => CLOCKLESS_FRAME,
        } as f32;
        self.last_frame_time = now;
        let Some(mut f) = self.drag_follow else {
            self.autoscrolling = false;
            return;
        };
        let Some(scroller) = f.scroller else {
            self.autoscrolling = false;
            return;
        };
        // 1. The frame under the pointer moved: place the end again.
        let moved = match scroller {
            Scroller::Container { key, seen } => {
                self.scroll.laid_offset(key).is_some_and(|o| o != seen)
            }
            Scroller::Handler { key, seen } => seen.is_none() || self.grid_line(key) != seen,
        };
        if moved {
            self.rehit(f.point);
            f.scroller = Some(match scroller {
                Scroller::Container { key, .. } => Scroller::Container {
                    key,
                    seen: self.scroll.laid_offset(key).unwrap_or(Vec2::ZERO),
                },
                Scroller::Handler { key, .. } => Scroller::Handler {
                    key,
                    seen: self.grid_line(key),
                },
            });
        }
        // 2. The pointer is past the edge: step the scroller.
        let stepped = match scroller {
            Scroller::Container { key, .. } => {
                let Some(i) = self.tree.index_of(key) else {
                    self.drag_follow = Some(f);
                    self.autoscrolling = false;
                    return;
                };
                let spec = self.tree.specs[i].layout;
                let rect = Rect::from_pos_size(self.tree.pos[i], self.tree.size[i]);
                let delta = edge_step(rect, f.point, spec.scroll_x, spec.scroll_y, dt);
                if delta != Vec2::ZERO {
                    self.scroll.scroll_by(key, delta);
                    true
                } else {
                    false
                }
            }
            Scroller::Handler { key, .. } => {
                let Some(i) = self.tree.index_of(key) else {
                    self.drag_follow = Some(f);
                    self.autoscrolling = false;
                    return;
                };
                let rect = Rect::from_pos_size(self.tree.pos[i], self.tree.size[i]);
                // A grid scrolls lines and has no columns to scroll to;
                // any other handler hears both axes.
                let grid = self.grid_line(key).is_some();
                let delta = edge_step(rect, f.point, !grid, true, dt);
                if delta != Vec2::ZERO {
                    // The event's delta is the wheel's: positive `dy` is
                    // earlier content, so a step toward later content —
                    // the pointer below — is negative.
                    let wheel = Vec2::new(-delta.x, -delta.y);
                    let p = f.point;
                    let mut carry = f.carry;
                    if let Some(ev) = self.scroll_event(key, p, wheel, &mut carry) {
                        self.pending.push(ev);
                    }
                    f.carry = carry;
                    true
                } else {
                    false
                }
            }
        };
        self.autoscrolling = stepped;
        self.drag_follow = Some(f);
    }

    /// The `scroll` event a delta over the `on_scroll` node `key` makes —
    /// the wheel's, or an edge step's — with the whole lines it covers
    /// on a grid, the fraction left in `carry`. `None` when the node is
    /// not in the finished frame or declared no tag.
    pub(crate) fn scroll_event(
        &mut self,
        key: Key,
        p: Vec2,
        delta: Vec2,
        carry: &mut f32,
    ) -> Option<UiEvent> {
        let i = self.tree.index_of(key)?;
        let tag = self.tree.specs[i].events().on_scroll.clone()?;
        let origin = self.tree.origins[i];
        let lines = match self.grid_line_height(key) {
            Some(h) => {
                // Later history is the wheel rolling down: negative `dy`.
                let total = *carry - delta.y / h;
                let whole = total.trunc();
                *carry = total - whole;
                Value::Int(whole as i64)
            }
            None => Value::Null,
        };
        let payload = Value::map([
            ("kind", Value::str("scroll")),
            ("x", Value::Float(p.x as f64)),
            ("y", Value::Float(p.y as f64)),
            ("dx", Value::Float(delta.x as f64)),
            ("dy", Value::Float(delta.y as f64)),
            ("lines", lines),
        ]);
        Some(UiEvent::on(origin, key, payload).tagged(Some(&tag)))
    }
}

/// How far a scroller steps this frame for a pointer at `p` against its
/// `rect`: on each axis it scrolls, the distance past that edge (zero
/// inside), capped at the reach, times the rate, times the frame. A
/// positive step is toward later content, the sign `ScrollStore::scroll_by`
/// takes.
pub(crate) fn edge_step(rect: Rect, p: Vec2, x: bool, y: bool, dt: f32) -> Vec2 {
    let past = |lo: f32, hi: f32, v: f32| -> f32 {
        if v < lo {
            -(lo - v).min(AUTOSCROLL_REACH)
        } else if v > hi {
            (v - hi).min(AUTOSCROLL_REACH)
        } else {
            0.0
        }
    };
    let k = AUTOSCROLL_RATE * dt;
    Vec2::new(
        if x {
            past(rect.x, rect.x + rect.w, p.x) * k
        } else {
            0.0
        },
        if y {
            past(rect.y, rect.y + rect.h, p.y) * k
        } else {
            0.0
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pointer_inside_steps_nothing() {
        let r = Rect::new(10.0, 10.0, 100.0, 50.0);
        assert_eq!(
            edge_step(r, Vec2::new(50.0, 30.0), true, true, 1.0 / 60.0),
            Vec2::ZERO
        );
    }

    #[test]
    fn the_step_is_proportional_capped_and_per_frame() {
        let r = Rect::new(0.0, 0.0, 100.0, 100.0);
        // 60 px below at a clockless frame: 600 px/s, 10 px a frame.
        let s = edge_step(r, Vec2::new(50.0, 160.0), false, true, 1.0 / 60.0);
        assert!((s.y - 10.0).abs() < 1e-4, "{s:?}");
        assert_eq!(s.x, 0.0);
        // 300 px above: capped at the reach, and toward earlier content.
        let s = edge_step(r, Vec2::new(50.0, -300.0), false, true, 1.0 / 60.0);
        assert!((s.y + AUTOSCROLL_REACH / 6.0).abs() < 1e-3, "{s:?}");
        // An axis the scroller does not have steps nothing.
        let s = edge_step(r, Vec2::new(160.0, 50.0), false, true, 1.0 / 60.0);
        assert_eq!(s, Vec2::ZERO);
        let s = edge_step(r, Vec2::new(160.0, 50.0), true, true, 1.0 / 60.0);
        assert!((s.x - 10.0).abs() < 1e-4, "{s:?}");
    }
}
