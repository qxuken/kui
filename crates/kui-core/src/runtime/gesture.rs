//! Scroll gestures: latching and chaining.
//!
//! A wheel delta routed to whatever is under the pointer when it comes
//! misbehaves twice: a swipe that moves a strip carries a terminal under
//! the still pointer, which then takes the rest of the swipe; and a list
//! already at its end swallows a swipe the scroller around it could have
//! used. The browser answer, taken here: a *gesture* — a swipe and its momentum, a wheel spun
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
use crate::tree::NIL;

/// How close to its limit a scroller has to be to count as there, logical
/// px: a sub-pixel remainder is no room to move.
const AT_LIMIT: f32 = 0.5;

/// The targets the gesture under way has latched, per axis. `None` on an
/// axis the gesture has not moved on yet, or that found nothing to move.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ScrollLatch {
    x: Option<Key>,
    y: Option<Key>,
    /// The `y` target is a scroller that moves only sideways, which a
    /// mouse wheel's turn moves along `x` (ADR 0050, amended 2026-10-25).
    y_sideways: bool,
    /// The modifiers held when the gesture began (`KeyMods::bits`): what
    /// a handler's `scroll_mods` is asked against for the whole of it, so
    /// a glide stays what the swipe was whatever is let go or pressed
    /// meanwhile (backlog F122).
    held: u32,
}

/// What a scroll region says to a gesture starting over it on one axis.
enum Answer {
    /// It is the target.
    Take,
    /// It is not; ask the region under it.
    Pass,
}

impl ScrollRegion {
    /// Whether this is a handler that names one of the modifiers `held`
    /// and takes the axis: the gesture is its before any other region is
    /// asked (`scroll_mods`, backlog F122).
    fn hears(&self, x: bool, held: u32) -> bool {
        self.mods & held != 0 && if x { self.takes_x } else { self.takes_y }
    }

    /// The region as a gesture begun with `held` meets it on `x` (or
    /// `y`). A handler that names modifiers is, to a gesture it does not
    /// hear, as if it declared no `on_scroll`: the container it may also
    /// be, which scrolls as any other does, and otherwise nothing.
    fn asked(&self, x: bool, held: u32) -> ScrollRegion {
        if self.mods == 0 || self.hears(x, held) {
            return *self;
        }
        ScrollRegion {
            handler: false,
            takes_x: self.scrolls_x,
            takes_y: self.scrolls_y,
            mods: 0,
            ..*self
        }
    }
}

impl Core {
    /// Routes one wheel delta: to the targets the gesture under way
    /// latched, or — for the gesture's first event, or an axis it first
    /// moves on — to the ones picked under the pointer now. `wheel` is a
    /// notch's, `Some(whole)` (ADR 0050); `None` for pixels.
    pub(crate) fn route_scroll(
        &mut self,
        delta: Vec2,
        begins: bool,
        wheel: Option<bool>,
        out: &mut Vec<UiEvent>,
    ) {
        if begins {
            self.scroll_latch = ScrollLatch {
                held: self.interaction.modifiers().bits(),
                ..ScrollLatch::default()
            };
        }
        // A mouse wheel's turn with Shift held scrolls sideways, as a
        // browser's does: macOS turns it before it gets here, Windows and
        // Linux leave it to the app. A handler that names Shift hears the
        // turn as it came (`scroll_mods`, backlog F122).
        let held = self.scroll_latch.held;
        let mut delta = delta;
        if wheel.is_some()
            && delta.x == 0.0
            && held & crate::input::KeyMods::SHIFT != 0
            && !self.named_hears(false, held)
        {
            delta = Vec2::new(delta.y, 0.0);
        }
        // A plain turn may move a scroller that only goes sideways; a
        // trackpad's pixels have an `x` of their own.
        let sideways = wheel.is_some() && delta.x == 0.0;
        let tx = (delta.x != 0.0)
            .then(|| self.scroll_target(true, delta.x, false))
            .flatten();
        let ty = (delta.y != 0.0)
            .then(|| self.scroll_target(false, delta.y, sideways))
            .flatten();
        let turned = ty.is_some() && self.scroll_latch.y_sideways;
        match (tx, ty) {
            // One target for both: one event on a handler, as the whole
            // delta always was.
            (Some(a), Some(b)) if !turned && a.key == b.key && a.handler == b.handler => {
                self.scroll_into(a, delta, wheel, out)
            }
            (a, b) => {
                if let Some(a) = a {
                    self.scroll_into(a, Vec2::new(delta.x, 0.0), wheel, out);
                }
                if let Some(b) = b {
                    let d = if turned {
                        Vec2::new(delta.y, 0.0)
                    } else {
                        Vec2::new(0.0, delta.y)
                    };
                    self.scroll_into(b, d, wheel, out);
                }
            }
        }
    }

    /// Whether a handler around the pointer that names a modifier of
    /// `held` takes the gesture on `x` (or `y`): the walk `scroll_target`
    /// begins with.
    fn named_hears(&self, x: bool, held: u32) -> bool {
        let regions = &self.interaction.scroll_regions;
        let mut at = self
            .interaction
            .scroll_regions_at()
            .next()
            .map(|r| r.node)
            .and_then(|node| regions.iter().rposition(|r| r.node == node));
        while let Some(i) = at {
            let r = regions[i];
            if r.inert {
                return false;
            }
            if r.hears(x, held) {
                return true;
            }
            at = (r.parent != NIL).then_some(r.parent as usize);
        }
        false
    }

    /// The region the gesture's `x` (or `y`) goes to: the latched one
    /// while it is still declared and outside any modal, else — from the
    /// topmost region under the pointer out through the ones around it in
    /// the tree — the first that takes a delta of sign `d` on that axis,
    /// latched from here on. The walk goes by the tree, not by what else
    /// is painted under the pointer: a list at its end in a popover
    /// passes the gesture to the scroller the popover was declared in,
    /// not to the page it happens to float over.
    ///
    /// `sideways` (a mouse wheel's plain turn, on `y`): a container on
    /// the way that scrolls only along `x` and still has room that way
    /// takes it, to move along `x` — `y_sideways` says so — and one at
    /// its end hands it on, as a list at its end does.
    fn scroll_target(&mut self, x: bool, d: f32, sideways: bool) -> Option<ScrollRegion> {
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
                return Some(r.asked(x, self.scroll_latch.held));
            }
        }
        if !x {
            self.scroll_latch.y_sideways = false;
        }
        let regions = &self.interaction.scroll_regions;
        let mut at = self
            .interaction
            .scroll_regions_at()
            .next()
            .map(|r| r.node)
            .and_then(|node| regions.iter().rposition(|r| r.node == node));
        let mut picked = None;
        // A handler that names the modifiers held is the more specific
        // ask (`scroll_mods`, backlog F122): the innermost one around the
        // pointer takes the gesture ahead of every region that names
        // none, whatever room those have.
        let held = self.scroll_latch.held;
        let mut named = at.filter(|_| held != 0);
        while let Some(i) = named {
            let r = regions[i];
            if r.inert {
                break;
            }
            if r.hears(x, held) {
                picked = Some(r);
                at = None;
                break;
            }
            named = (r.parent != NIL).then_some(r.parent as usize);
        }
        while let Some(i) = at {
            let r = regions[i].asked(x, held);
            // Out through a modal's edge is out of its scope: the one
            // around is inert, and so is everything past it.
            if r.inert {
                break;
            }
            if matches!(self.answer(&r, x, d), Answer::Take) {
                picked = Some(r);
                break;
            }
            if sideways
                && !r.handler
                && r.takes_x
                && !r.takes_y
                && matches!(self.answer(&r, true, d), Answer::Take)
            {
                picked = Some(r);
                self.scroll_latch.y_sideways = true;
                break;
            }
            at = (r.parent != NIL).then_some(r.parent as usize);
        }
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
    /// or not it has anywhere to go — the core cannot ask it — unless it
    /// is a container on that axis too, whose offset the app sets and
    /// the core can ask; a container's on the axes it scrolls,
    /// while it can still move that way or when it says `contain`. Read
    /// off the region, as the frame that drew it left it, never off the
    /// tree.
    fn answer(&self, r: &ScrollRegion, x: bool, d: f32) -> Answer {
        let takes = if x { r.takes_x } else { r.takes_y };
        if !takes {
            return Answer::Pass;
        }
        let scrolls = if x { r.scrolls_x } else { r.scrolls_y };
        if r.contain || (r.handler && !scrolls) {
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
    /// container — a notch's glides there (ADR 0050).
    fn scroll_into(
        &mut self,
        r: ScrollRegion,
        delta: Vec2,
        wheel: Option<bool>,
        out: &mut Vec<UiEvent>,
    ) {
        if r.handler {
            let p = self.interaction.cursor().unwrap_or(Vec2::ZERO);
            if let Some(mut ev) = self.scroll_event(r.key, p, delta) {
                if r.mods != 0
                    && let Some(scroll) = ev.data_fields_mut()
                {
                    let held = crate::input::KeyMods::from_bits(self.scroll_latch.held);
                    scroll.push(("mods".to_string(), held.to_fields()));
                }
                out.push(ev);
            }
        } else {
            // Wheel up (positive y) reveals earlier content: offset
            // decreases.
            let d = Vec2::new(-delta.x, -delta.y);
            match wheel {
                Some(whole) if self.glides_wheel() => self.scroll.wheel_by(r.key, d, whole),
                _ => self.scroll.scroll_by(r.key, d),
            }
        }
    }
}
