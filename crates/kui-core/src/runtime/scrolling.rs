//! Retained scroll offsets: the outside API (`reveal`, `set_scroll`, the
//! geometry a virtual list reads) and the after-layout nudges that keep a
//! caret or a revealed node in view. The wheel and the scrollbars move the
//! same offsets from `dispatch`.

use super::*;

impl Core {
    /// Sets one axis of a container's scroll offset, keeping the other.
    pub(crate) fn set_scroll_axis(&mut self, key: Key, axis: ScrollAxis, value: f32) {
        let mut off = self.scroll.offset(key);
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
    /// frame are contradictory, so the last wins.
    pub fn reveal(&mut self, key: Key) {
        self.pending_reveal = Some(key);
        self.request_frame();
    }

    /// The retained scroll offset of the container `key`, as the last
    /// layout clamped it (positive = content moved up / left). Zero for a
    /// node that never scrolled, and for one that is not a container at
    /// all — the store keeps offsets, not membership.
    pub fn scroll_offset(&self, key: Key) -> Vec2 {
        self.scroll.offset(key)
    }

    /// Everything the last layout resolved for the container `key`: its own
    /// box, its content size, and the clamped offset — `None` for a key no
    /// layout has ever resolved as a scroll container.
    ///
    /// This is what makes a long list affordable. The core culls glyphs by
    /// viewport but builds every child a view declares, so ten thousand rows
    /// cost ten thousand rows; with the offset and the container's height a
    /// view can declare only the rows that can be seen and two spacers, and
    /// pay for a screenful. `widgets::virtual_column` is that, done.
    ///
    /// Read during a build, it describes the frame before — the tree it came
    /// from is already cleared. That is one frame of lag on the size, so the
    /// frame after a resize slices to the old height; a row or two of
    /// overscan covers it, which is what the widget does. The rect is the
    /// same one an `on_layout` on that node would post, without the round
    /// trip through the app's model, and without firing every time an
    /// enclosing container scrolls the whole list past.
    pub fn scroll_geometry(&self, key: Key) -> Option<crate::scroll::ScrollGeometry> {
        self.scroll.geometry(key)
    }

    /// Sets the container `key`'s retained offset, the way the wheel would.
    /// Takes effect on the next frame, whose layout clamps it to that
    /// frame's overflow: `Vec2::ZERO` is "jump to the top", and a large
    /// value is "jump to the end" without knowing the content height.
    /// Writing an offset for a key that never scrolls is harmless; it just
    /// never reads back.
    pub fn set_scroll(&mut self, key: Key, offset: Vec2) {
        self.scroll.set(key, offset);
        self.request_frame();
    }

    /// After layout: if the focused edit's caret moved this frame, nudge the
    /// nearest scrollable ancestor so the caret stays visible, then re-run
    /// the positions pass with the adjusted offset (positions is the only
    /// pass scroll offsets feed into, so nothing else needs recomputing).
    pub(crate) fn scroll_caret_into_view(&mut self) {
        let Some(key) = self.edit.caret_moved.take() else {
            return;
        };
        if self.edit.focused() != Some(key) {
            return;
        }
        let Some(i) =
            (0..self.tree.len()).find(|&i| self.tree.content[i] == NodeContent::Edit(key))
        else {
            return;
        };
        let Some(caret_phys) = self.edit_with_fonts(|edit, fs| edit.caret_rect(key, fs)) else {
            return;
        };
        let pad = self.tree.specs[i].layout.padding;
        let caret = Rect::new(
            self.tree.pos[i].x + pad.l + caret_phys.x / self.scale,
            self.tree.pos[i].y + pad.t + caret_phys.y / self.scale,
            caret_phys.w / self.scale,
            caret_phys.h / self.scale,
        );
        self.scroll_rect_into_view(i, caret, true);
    }

    /// After layout: resolves a pending `reveal(key)` against the frame
    /// just laid out, then forgets it either way — a key this frame did
    /// not declare is a no-op, not a request that waits for the frame that
    /// declares it. Like the caret, the positions pass re-runs, so this
    /// frame already draws the node in view.
    pub(crate) fn apply_pending_reveal(&mut self) {
        let Some(key) = self.pending_reveal.take() else {
            return;
        };
        let Some(i) = (0..self.tree.len()).find(|&i| self.tree.keys[i] == key) else {
            return;
        };
        let rect = Rect::from_pos_size(self.tree.pos[i], self.tree.size[i]);
        self.scroll_rect_into_view(i, rect, true);
    }

    /// Nudges the nearest scrolling ancestor of node `i` so `rect`
    /// (viewport coordinates) is inside it. With `relayout`, re-runs the
    /// positions pass so this frame already shows the new offset
    /// (positions is the only pass scroll offsets feed into); without it
    /// the next frame does.
    pub(crate) fn scroll_rect_into_view(&mut self, i: usize, rect: Rect, relayout: bool) {
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
                    self.scroll.scroll_by(self.tree.keys[a as usize], delta);
                    if relayout {
                        layout::positions(
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
