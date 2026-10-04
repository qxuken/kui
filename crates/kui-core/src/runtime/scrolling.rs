//! Retained scroll offsets: the outside API (`reveal`, `set_scroll`, the
//! geometry a virtual list reads) and the after-layout nudges that keep a
//! caret or a revealed node in view. The wheel and the scrollbars move the
//! same offsets from `dispatch`.

use super::*;

impl Core {
    /// Sets one axis of a container's scroll offset, keeping the other
    /// where it stands — mid-leg, the drawn place and not the target, so
    /// a thumb dragged on one axis does not jump the other to where an
    /// eased leg was going.
    pub(crate) fn set_scroll_axis(&mut self, key: Key, axis: ScrollAxis, value: f32) {
        let mut off = self.scroll.standing(key);
        match axis {
            ScrollAxis::X => off.x = value,
            ScrollAxis::Y => off.y = value,
        }
        self.scroll.set(key, off);
    }

    // -- Scrolling ------------------------------------------------------
    // Scroll offsets are retained per node key, clamped to the overflow the
    // last layout found. The wheel, the scrollbars, Tab and the caret all
    // move them from inside; these three are the same moves from outside,
    // so "scroll to the selected row", "jump to the top" and "restore the
    // position I saved" need no layout arithmetic in the app.

    /// Scrolls the nearest scrolling ancestor of `key` so the node is
    /// inside it — what Tab does to the control it lands on, asked for by
    /// name. Already-visible nodes stay put.
    ///
    /// The request resolves at the next `finish_frame`, against the frame
    /// that one lays out — the frame being built if this is called from a
    /// view, the one after it if from an event handler (a frame is
    /// requested, so one comes). That is what lets a view reveal a row it
    /// is declaring for the first time. If that frame does not declare
    /// `key`, or nothing above it scrolls, it is a no-op — the request is
    /// spent, not held for the frame that might. Two reveals before one
    /// frame into the *same* container are contradictory, so the last one
    /// wins there; reveals into different containers — a tab strip and
    /// the pane list under it — are not, and each lands.
    ///
    /// Traced, the ask is `"reveal"` at the caller's line.
    #[track_caller]
    pub fn reveal(&mut self, key: Key) {
        self.pending_reveal.push(key);
        self.owe_frame("reveal");
    }

    /// [`Self::reveal`] by the label a node declares, resolved when the
    /// frame finishes — against the frame being built, or the next one
    /// when none is — so a view may name a row it is declaring right now,
    /// or one the frame after declares. A label that frame
    /// does not declare raises `label-without-node` and moves nothing.
    #[track_caller]
    pub fn reveal_label(&mut self, label: &str) {
        self.pending_reveal_labels
            .push((label.to_string(), self.origin));
        self.owe_frame("reveal_label");
    }

    /// [`Self::set_scroll`] by label, resolved like [`Self::reveal_label`]
    /// but before layout, so the frame that resolves it lays out at the
    /// offset.
    #[track_caller]
    pub fn set_scroll_label(&mut self, label: &str, offset: Vec2) {
        self.pending_scroll_labels
            .push((label.to_string(), self.origin, offset));
        if !self.building {
            self.owe_frame("set_scroll_label");
        }
    }

    /// A deferred label, found as the origin that asked would find it
    /// (`find_label` answers per origin), in this frame alone.
    fn find_label_as(&mut self, label: &str, origin: crate::tree::OriginId) -> Option<Key> {
        let at = std::mem::replace(&mut self.origin, origin);
        let key = self.find_label(label, false);
        self.origin = at;
        key
    }

    /// Before layout: the `set_scroll_label` asks, as `set_scroll`s.
    pub(crate) fn resolve_scroll_labels(&mut self) {
        for (label, origin, offset) in std::mem::take(&mut self.pending_scroll_labels) {
            match self.find_label_as(&label, origin) {
                Some(key) => self.scroll.set_smooth(key, offset),
                None => self
                    .diag
                    .raise(crate::diag::label_without_node("set_scroll", &label)),
            }
        }
    }

    /// The retained scroll offset of the container `key`, as the last
    /// layout clamped it (positive = content moved up / left) — the
    /// target: while a container with a `transition` eases to it the
    /// content is drawn short of it, where [`Self::scroll_geometry`]
    /// says. Zero for a node that never scrolled, and for one that is not
    /// a container at all — the store keeps offsets, not membership.
    pub fn scroll_offset(&self, key: Key) -> Vec2 {
        self.scroll.offset(key)
    }

    /// Everything the last layout resolved for the container `key`: its own
    /// box, its content size, and the clamped offset — `None` for a key no
    /// layout has ever resolved as a scroll container. While an eased
    /// leg runs the offset is where the content is drawn rather than
    /// the target, sampled at the clock the coming frame reads, so a view
    /// slices the rows that frame shows.
    ///
    /// This is what makes a long list affordable. The core culls glyphs by
    /// viewport but builds every child a view declares, so ten thousand rows
    /// cost ten thousand rows; with the offset and the container's height a
    /// view can declare only the rows that can be seen and two spacers, and
    /// pay for a screenful. `widgets::uniform_list` is that, done.
    ///
    /// Read during a build, it describes the frame before — the tree it came
    /// from is already cleared. That is one frame of lag on the size, so the
    /// frame after a resize slices to the old height; a row or two of
    /// overscan covers it, which is what the widget does. The rect is the
    /// same one an `on_layout` on that node would post, without the round
    /// trip through the app's model, and without firing every time an
    /// enclosing container scrolls the whole list past.
    pub fn scroll_geometry(&self, key: Key) -> Option<crate::scroll::ScrollGeometry> {
        let shift = self.dt_shift();
        self.scroll.geometry(key).map(|mut g| {
            g.rect.x -= shift.x;
            g.rect.y -= shift.y;
            g
        })
    }

    /// The rect the last frame laid `key` out at, in logical viewport px —
    /// for a node that declared `on_layout`, whose rect the core keeps for
    /// the event's edge trigger anyway. The query
    /// shape of the `layout` event: the same numbers, read during the next
    /// build with no event, no tag and no model field. `None` for a key
    /// that did not declare `on_layout` last frame; read during a build it
    /// describes the previous frame, like [`Self::scroll_geometry`].
    pub fn layout_of(&self, key: Key) -> Option<Rect> {
        let shift = self.dt_shift();
        // "Last frame" is the one before this build while a build is on,
        // and the one just finished between two — `frame_no` has already
        // moved on in the first case and not in the second.
        let last = self.frame_no - u64::from(self.building);
        self.layouts
            .get(&key)
            .filter(|(_, seen)| *seen == last)
            .map(|(r, _)| Rect::new(r.x - shift.x, r.y - shift.y, r.w, r.h))
    }

    /// Sets the container `key`'s retained offset, the way the wheel would.
    /// Takes effect at the next layout, which clamps it to that frame's
    /// overflow: `Vec2::ZERO` is "jump to the top", and a large value is
    /// "jump to the end" without knowing the content height. Writing an
    /// offset for a key that never scrolls is harmless; it just never
    /// reads back. Between two frames the write asks for the frame that
    /// lands it; from inside a view it asks for nothing, because the
    /// frame being built is that frame — the positions pass reads the
    /// store after the view has run — and a view writing every frame
    /// would otherwise be a window that never idles (the devtools' events
    /// list and `widgets::list` both write this way).
    #[track_caller]
    pub fn set_scroll(&mut self, key: Key, offset: Vec2) {
        // Programmatic, so a container with a `transition` eases into it
        // (F80); the wheel and the thumb go through `scroll_by` and
        // `set_scroll_axis`, which do not.
        self.scroll.set_smooth(key, offset);
        if !self.building {
            self.owe_frame("set_scroll");
        }
    }

    /// Moves `key`'s scroll state by the content that moved under it:
    /// `drawn` for where the content is drawn (and an eased leg's start),
    /// `target` for the retained offset. No ease is asked or ended, and no
    /// frame is asked for: it is a correction to the frame about to be
    /// laid out — the rows above the window were measured and came out
    /// another height — so it belongs to that frame, whoever calls it. A
    /// variable-height list's anchor: `widgets::list` from its view,
    /// the Node and Lua ports from theirs, just before the tree they
    /// return is laid out.
    pub fn shift_scroll(&mut self, key: Key, drawn: Vec2, target: Vec2) {
        self.scroll.shift(key, drawn, target);
    }

    /// After layout: if the focused edit's caret moved this frame, nudge the
    /// nearest scrollable ancestor so the caret stays visible, then re-run
    /// the positions pass with the adjusted offset (positions is the only
    /// pass scroll offsets feed into, so nothing else needs recomputing).
    pub(crate) fn scroll_caret_into_view(&mut self) {
        let Some(key) = self.edit.caret_moved else {
            return;
        };
        // Not under a held pointer drag: the caret is where the pointer
        // is, past the edge or not, and what moves the scroller then is
        // the drag's own rate (ADR 0029, decision 2) — a reveal there
        // would jump it by the whole distance past, every frame. The move
        // stays noted, so the release reveals where the caret landed;
        // a field still scrolls its own text below, at once.
        let held = self.edit.dragging.is_some_and(|(k, _)| k == key);
        if !held {
            self.edit.caret_moved = None;
        }
        if self.edit.focused() != Some(key) {
            return;
        }
        let Some(i) =
            (0..self.tree.len()).find(|&i| self.tree.content[i] == NodeContent::Edit(key))
        else {
            return;
        };
        let pad = self.tree.specs[i].layout.padding;
        // A single-line field scrolls its own text, and the box it does
        // that in is final now that layout has run — so settle its offset
        // before asking where the caret is. Without this the ancestor
        // scrolls to reveal a caret the field is about to bring into view
        // itself (F41); emission recomputes the same number.
        let inner_w = (self.tree.size[i].w - pad.x()).max(0.0) * self.scale;
        self.edit_with_fonts(|edit, fs| edit.line_offset(key, inner_w, fs));
        if held {
            return;
        }
        let Some((_, caret)) = self.stock_caret_viewport_rect(key) else {
            return;
        };
        self.scroll_rect_into_view(i, caret, true);
    }

    /// After layout: resolves a pending `reveal(key)` against the frame
    /// just laid out, then forgets it either way — a key this frame did
    /// not declare is a no-op, not a request that waits for the frame that
    /// declares it. Like the caret, the positions pass re-runs, so this
    /// frame already draws the node in view.
    pub(crate) fn apply_pending_reveal(&mut self) {
        for (label, origin) in std::mem::take(&mut self.pending_reveal_labels) {
            match self.find_label_as(&label, origin) {
                Some(key) => self.pending_reveal.push(key),
                None => self
                    .diag
                    .raise(crate::diag::label_without_node("reveal", &label)),
            }
        }
        let asks = std::mem::take(&mut self.pending_reveal);
        if asks.is_empty() {
            return;
        }
        // Each ask by the container it would move — its nearest
        // scrolling ancestor, which is the only one a reveal nudges —
        // the last ask per container kept, in the order asked.
        let mut kept: Vec<(u32, usize)> = Vec::new();
        for key in asks {
            let Some(i) = (0..self.tree.len()).find(|&i| self.tree.keys[i] == key) else {
                continue;
            };
            let mut a = self.tree.parent[i];
            while a != NIL {
                let spec = self.tree.specs[a as usize].layout;
                if spec.scroll_x || spec.scroll_y {
                    break;
                }
                a = self.tree.parent[a as usize];
            }
            if a == NIL {
                continue;
            }
            kept.retain(|(c, _)| *c != a);
            kept.push((a, i));
        }
        for (_, i) in kept {
            let rect = Rect::from_pos_size(self.tree.pos[i], self.tree.size[i]);
            self.scroll_rect_into_view_smooth(i, rect, true);
        }
    }

    /// Nudges the nearest scrolling ancestor of node `i` so `rect`
    /// (viewport coordinates) is inside it. With `relayout`, re-runs the
    /// positions pass so this frame already shows the new offset
    /// (positions is the only pass scroll offsets feed into); without it
    /// the next frame does.
    pub(crate) fn scroll_rect_into_view(&mut self, i: usize, rect: Rect, relayout: bool) {
        self.scroll_rect_into_view_from(i, rect, relayout, false);
    }

    /// The same, easing the nudge on a container that declares a
    /// `transition`: what `reveal` asks for, where a caret nudge
    /// and a focus move take the content there at once.
    pub(crate) fn scroll_rect_into_view_smooth(&mut self, i: usize, rect: Rect, relayout: bool) {
        self.scroll_rect_into_view_from(i, rect, relayout, true);
    }

    fn scroll_rect_into_view_from(&mut self, i: usize, rect: Rect, relayout: bool, smooth: bool) {
        // Slack so the target isn't glued to the container edge.
        const MARGIN: f32 = 4.0;
        let mut a = self.tree.parent[i];
        while a != NIL {
            let spec = self.tree.specs[a as usize].layout;
            if spec.scroll_x || spec.scroll_y {
                let view =
                    Rect::from_pos_size(self.tree.pos[a as usize], self.tree.size[a as usize]);
                let mut delta = Vec2::ZERO;
                if spec.scroll_y {
                    if rect.y < view.y + MARGIN {
                        delta.y = rect.y - (view.y + MARGIN);
                    } else if rect.y + rect.h > view.y + view.h - MARGIN {
                        delta.y = rect.y + rect.h - (view.y + view.h - MARGIN);
                    }
                }
                if spec.scroll_x {
                    if rect.x < view.x + MARGIN {
                        delta.x = rect.x - (view.x + MARGIN);
                    } else if rect.x + rect.w > view.x + view.w - MARGIN {
                        delta.x = rect.x + rect.w - (view.x + view.w - MARGIN);
                    }
                }
                if delta.x != 0.0 || delta.y != 0.0 {
                    let key = self.tree.keys[a as usize];
                    if smooth {
                        self.scroll.scroll_by_smooth(key, delta);
                    } else {
                        self.scroll.scroll_by(key, delta);
                    }
                    if relayout {
                        layout::reposition(
                            &mut self.tree,
                            &mut self.scroll,
                            self.viewport,
                            self.scale,
                        );
                    }
                }
                return;
            }
            a = self.tree.parent[a as usize];
        }
    }
}
