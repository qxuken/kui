//! Scroll gestures: latching and chaining (backlog F107,
//! `docs/adr/0038-a-scroll-gesture-latches-its-target.md`).
//!
//! A wheel delta used to go to whatever was under the pointer when it
//! came. A swipe that moved the strip carried a terminal under a still
//! pointer, and the terminal — an `on_scroll` node taking every delta —
//! took the rest of the swipe; a list already at its end swallowed a
//! swipe the scroller around it could have used. The browser answer,
//! taken here: a *gesture* — a swipe and its momentum, a wheel spun
//! without a pause, as the driver delimits them
//! ([`crate::InputEvent::ScrollGesture`]) — picks its target when it
//! starts and keeps it (*latching*); the pick skips a scroller already at
//! its limit the way the gesture goes, for the one around it
//! (*chaining*), unless that scroller says `overscroll: contain`.
//!
//! Per axis: the target for `x` and the one for `y` are picked apart,
//! each the first time the gesture moves on that axis, which is what
//! DX13's hand-off of a list's unused `x` to the strip around it becomes.
//! F104's lock keeps a trackpad swipe to one axis, so a swipe has one
//! target in practice; a wheel's diagonal or a turned swipe has two.

use super::*;
use crate::input::ScrollRegion;
use crate::spec::Overscroll;

/// How close to its limit a scroller has to be to count as there, logical
/// px: a sub-pixel remainder is no room to move.
const AT_LIMIT: f32 = 0.5;

/// The targets the gesture under way has latched, per axis. `None` on an
/// axis the gesture has not moved on yet, or that found nothing to move.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ScrollLatch {
    x: Option<Key>,
    y: Option<Key>,
}

/// What a scroll region says to a gesture starting over it on one axis.
enum Answer {
    /// It is the target.
    Take,
    /// It is not; ask the region under it.
    Pass,
}

impl Core {
    /// Routes one wheel delta: to the targets the gesture under way
    /// latched, or — for the gesture's first event, or an axis it first
    /// moves on — to the ones picked under the pointer now.
    pub(crate) fn route_scroll(&mut self, delta: Vec2, begins: bool, out: &mut Vec<UiEvent>) {
        if begins {
            self.scroll_latch = ScrollLatch::default();
        }
        let tx = (delta.x != 0.0)
            .then(|| self.scroll_target(true, delta.x))
            .flatten();
        let ty = (delta.y != 0.0)
            .then(|| self.scroll_target(false, delta.y))
            .flatten();
        match (tx, ty) {
            // One target for both: one event on a handler, as the whole
            // delta always was.
            (Some(a), Some(b)) if a.key == b.key => self.scroll_into(a, delta, out),
            (a, b) => {
                if let Some(a) = a {
                    self.scroll_into(a, Vec2::new(delta.x, 0.0), out);
                }
                if let Some(b) = b {
                    self.scroll_into(b, Vec2::new(0.0, delta.y), out);
                }
            }
        }
    }

    /// The region the gesture's `x` (or `y`) goes to: the latched one
    /// while it is still declared and outside any modal, else the first
    /// under the pointer, innermost by paint order, that takes a delta
    /// of sign `d` on that axis — latched from here on.
    fn scroll_target(&mut self, x: bool, d: f32) -> Option<ScrollRegion> {
        let latched = if x {
            self.scroll_latch.x
        } else {
            self.scroll_latch.y
        };
        if let Some(key) = latched {
            // By key, not under the pointer: the gesture keeps its target
            // wherever the pointer, or the content under it, has gone.
            if let Some(r) = self
                .interaction
                .scroll_regions
                .iter()
                .rev()
                .find(|r| r.key == key && !r.inert)
            {
                return Some(*r);
            }
        }
        let regions: Vec<ScrollRegion> = self.interaction.scroll_regions_at().copied().collect();
        let picked = regions
            .into_iter()
            .find(|r| matches!(self.answer(r, x, d), Answer::Take));
        let key = picked.map(|r| r.key);
        if x {
            self.scroll_latch.x = key;
        } else {
            self.scroll_latch.y = key;
        }
        picked
    }

    /// Whether a gesture starting over `r` with a delta of sign `d` on
    /// `x` (or `y`) is `r`'s. A handler's on the axes it takes, whether
    /// or not it has anywhere to go — the core cannot ask it; a
    /// container's on the axes it scrolls, while it can still move that
    /// way or when it says `contain`.
    fn answer(&self, r: &ScrollRegion, x: bool, d: f32) -> Answer {
        let spec = &self.tree.specs[r.node as usize];
        if r.handler {
            return if spec.events().scroll_axes.takes(x) {
                Answer::Take
            } else {
                Answer::Pass
            };
        }
        let scrolls = if x {
            spec.layout.scroll_x
        } else {
            spec.layout.scroll_y
        };
        if !scrolls {
            return Answer::Pass;
        }
        if spec.interact().overscroll == Overscroll::Contain {
            return Answer::Take;
        }
        // Positive `d` is the wheel rolling up (or left): toward the
        // start, the offset shrinking. A container no layout has resolved
        // yet has no limit to be at.
        let Some((at, max)) = self.scroll.room(r.key) else {
            return Answer::Take;
        };
        let (at, max) = if x { (at.x, max.x) } else { (at.y, max.y) };
        let room = if d > 0.0 { at } else { max - at };
        if room > AT_LIMIT {
            Answer::Take
        } else {
            Answer::Pass
        }
    }

    /// Moves `r` by `delta`: an event on a handler, the offset on a
    /// container.
    fn scroll_into(&mut self, r: ScrollRegion, delta: Vec2, out: &mut Vec<UiEvent>) {
        if r.handler {
            let p = self.interaction.cursor().unwrap_or(Vec2::ZERO);
            if let Some(ev) = self.scroll_event(r.key, p, delta) {
                out.push(ev);
            }
        } else {
            // Wheel up (positive y) reveals earlier content: offset
            // decreases.
            self.scroll.scroll_by(r.key, Vec2::new(-delta.x, -delta.y));
        }
    }
}
