//! Popups and the keyboard's owner: which pane a key goes to while a
//! non-activating popup is up, the focus settling that follows an OS
//! focus change, dismissal, and the press-drag-release retargeting of
//! `docs/adr/0009-press-drag-release-into-a-popup.md`. Split off `lib.rs`
//! as a pure move.

use super::*;

impl DynShell<'_> {
    /// Which pane a key event that arrived at pane `i` is for.
    ///
    /// A non-activating popup never takes OS focus — that is the point of
    /// it, since a combobox that blurred the field it belongs to would be
    /// useless — so the keyboard stays with the owner and the runner hands
    /// it on (ADR 0004 decision 9). The owner's `env.focused` is left true
    /// meanwhile, so the field still draws focused while the arrow keys
    /// walk the list. A popup that *did* ask to activate holds its own
    /// keyboard and needs none of this.
    /// Followed to the deepest one: a sub-popup a popup's frame declared
    /// has the popup as its owner (a submenu opened from a menu), and the
    /// keys the OS delivers to the window that is key go all the way down
    /// the chain of non-activating popups, not one level (AR21).
    pub(super) fn key_target(&self, i: usize) -> usize {
        let mut at = i;
        // Bounded by the pane count: a chain cannot be longer than the
        // panes there are, so an owner loop — which nothing should build,
        // but which this walk must not hang on — ends where it started.
        for _ in 0..self.panes.len() {
            let owner = self.panes[at].id;
            match self
                .panes
                .iter()
                .position(|p| p.kind == WindowKind::Popup && p.owner == owner && !p.activates)
            {
                Some(next) if next != at && next != i => at = next,
                _ => return at,
            }
        }
        at
    }

    /// The panes pane `i` is owned by, nearest first, up to the window
    /// that owns itself: a sub-popup's parent popup, then that popup's
    /// owner. Empty for a window that is nobody's.
    fn ancestors(&self, i: usize) -> Vec<usize> {
        let mut out = Vec::new();
        let mut at = i;
        while let Some(j) = self.pane_of(self.panes[at].owner)
            && j != at
            && !out.contains(&j)
        {
            out.push(j);
            at = j;
        }
        out
    }

    /// Every popup that a press on pane `i`, or pane `i` losing the
    /// keyboard, should ask to go away: all of them except one the press
    /// landed in and the ones that popup hangs off — a press in a submenu
    /// is not outside the menu it opened from (AR21: it was, so the menu
    /// dismissed, the press was consumed, and the row pressed never heard
    /// it). A press in the owner counts — the owner is "outside" the
    /// popup, which is the whole distinction a separate surface makes —
    /// and so does a press in the parent for the submenu hanging off it.
    ///
    /// Each is paired with whether it took OS focus when it opened, because
    /// ADR 0009 decision 5 treats the two kinds differently: the press that
    /// dismisses a **non-activating** popup is consumed, while an
    /// activating one (a tear-off panel) keeps the pass-through it has.
    pub(super) fn popups_outside(&self, i: usize) -> Vec<(WindowId, bool)> {
        let keep = self.ancestors(i);
        self.panes
            .iter()
            .enumerate()
            .filter(|(j, p)| *j != i && !keep.contains(j) && p.kind == WindowKind::Popup)
            .map(|(_, p)| (p.id, p.activates))
            .collect()
    }

    /// Whether pane `i` and pane `j` share one keyboard: one is a
    /// non-activating popup reached from the other by following owners
    /// through non-activating popups only — a menu and its submenu and
    /// the window they hang off all read as focused while any of them is
    /// (AR21: the pairing stopped at the direct owner, so a sub-popup read
    /// `env.focused == false`).
    fn lends_to(&self, i: usize, j: usize) -> bool {
        let chain = |from: usize| -> Vec<usize> {
            let mut out = vec![from];
            let mut at = from;
            while self.panes[at].kind == WindowKind::Popup
                && !self.panes[at].activates
                && let Some(o) = self.pane_of(self.panes[at].owner)
                && o != at
                && !out.contains(&o)
            {
                out.push(o);
                at = o;
            }
            out
        };
        let (a, b) = (chain(i), chain(j));
        (self.panes[i].kind == WindowKind::Popup && !self.panes[i].activates && a.contains(&j))
            || (self.panes[j].kind == WindowKind::Popup
                && !self.panes[j].activates
                && b.contains(&i))
    }

    /// Works out what each window's *view* should believe about keyboard
    /// focus, from what the OS said about all of them.
    ///
    /// The two are not the same answer, and ADR 0004 decision 9 is why: a
    /// non-activating popup must not take the focus ring off the field
    /// that opened it, so **a window that owns one reads as focused while
    /// the popup holds the keyboard**. Without that the owner draws one
    /// unfocused frame every time a popup opens — the platform hands the
    /// key window over and back, and the frame in between is real. It is
    /// derived rather than patched at each event, and derived at the end of
    /// the batch rather than inside it, because focus *moving* is two
    /// events — three around a new window, since winit queues a
    /// `Focused(false)` for every one it creates — and in the middle of any
    /// ordering of them there is a moment when no window claims the
    /// keyboard. Reading that moment is the flicker.
    ///
    /// Only what changed is written, so a window whose answer did not move
    /// is not redrawn and does not let go of a held key.
    pub(super) fn settle_focus(&mut self) {
        // A non-activating popup that has ended up with the keyboard gives
        // it straight back. It should never have taken it — that is what
        // `activates: false` asked for — but a platform can make a window
        // key for reasons of its own, and pressing one is the reason that
        // matters: AppKit makes the popup key on mouse-down, and the
        // *platform's* titlebar greys out under a window that is not key
        // however `env.focused` reads. Asking again here is the only thing
        // that shortens that to a frame, since nothing else notices.
        for i in 0..self.panes.len() {
            let p = &self.panes[i];
            if p.kind != WindowKind::Popup || p.activates {
                continue;
            }
            if !p.os_focused {
                self.panes[i].handed_back = false;
                continue;
            }
            if p.handed_back {
                continue;
            }
            let owner = p.owner;
            let Some(j) = self.pane_of(owner) else {
                continue;
            };
            self.panes[i].handed_back = true;
            self.panes[j].window.focus_window();
        }
        for i in 0..self.panes.len() {
            // The chain reads as focused together, because along it the
            // keyboard is being routed rather than lost: an owner while its
            // popup holds it, the popup while the owner does, and a
            // submenu with both.
            let together = (0..self.panes.len())
                .any(|j| j != i && self.panes[j].os_focused && self.lends_to(i, j));
            let focused = self.panes[i].os_focused || together;
            let pane = &mut self.panes[i];
            if pane.core.env.focused == focused {
                continue;
            }
            // The core lets go of any held keys on the way out: the OS
            // stops sending key events to a window that lost the
            // keyboard, so the release would never arrive. The synthetic
            // `up`s route out with the pending events.
            pane.core.set_focused(focused);
            pane.redraw_for(FrameCause::FOCUS);
        }
    }

    /// Secure keyboard entry, moved to what this batch settled (backlog
    /// F85): on while a window whose last frame asked for it
    /// (`Ui::secure_input`) has the keyboard, as its view reads it —
    /// `env.focused`, which an owner keeps while its popup holds the
    /// keyboard, the keys still being ours — and off otherwise. After
    /// `settle_focus`, for the reason that is derived at the end of the
    /// batch: in the middle of focus moving no window claims the
    /// keyboard, and acting on that moment would drop the count and take
    /// it again. A closed window is gone from `panes`, so its ask goes
    /// with it.
    pub(super) fn apply_secure_input(&mut self) {
        let want = super::secure_input::wanted(
            self.panes
                .iter()
                .map(|p| (p.core.secure_input(), p.core.env.focused)),
        );
        self.secure_input.set(want);
    }

    /// The app has no window with the keyboard any more, so every popup is
    /// asked to go away: a menu left standing over another application is
    /// the one thing every platform agrees is wrong.
    ///
    /// Decided here, at the end of a batch of events, and not in the
    /// `Focused` handler — because focus *moving* is two events and their
    /// order is the platform's business. A popup opening deactivates its
    /// owner on some window managers, and acting on that `Focused(false)`
    /// alone would dismiss the popup on the frame it appeared. By the time
    /// the loop is about to wait, both halves have landed and "no window of
    /// ours holds the keyboard" is a fact rather than a moment.
    pub(super) fn dismiss_popups_if_deactivated(&mut self) {
        // `os_focused` and not `env.focused`: the whole point of the
        // latter is that an owner reads as focused while its popup holds
        // the keyboard, which would make this condition unreachable.
        if self.panes.iter().any(|p| p.os_focused) {
            return;
        }
        let popups: Vec<WindowId> = self
            .panes
            .iter()
            .filter(|p| p.kind == WindowKind::Popup)
            .map(|p| p.id)
            .collect();
        for id in popups {
            self.dismiss(id, DismissReason::Outside);
        }
    }

    /// Reports a dismissal to the popup's own core and routes the event.
    /// **Closes nothing**: the app stops declaring the window on the frame
    /// it decides to, exactly as it answers a `modal` node's dismissal
    /// (ADR 0003 decision 6, one level up). Raised on the popup's own core,
    /// so `UiEvent::window` is the window it is about — the `window` event
    /// cannot do that, because the window it names has just stopped or not
    /// yet started existing.
    /// Returns whether the dismissal reached the app, so a caller that
    /// redraws for it can leave that frame to the host under
    /// [`Launcher::deferred_events`] — an item chosen in a menu closes the
    /// menu *and* does what it says, and both belong in one frame.
    pub(super) fn dismiss(&mut self, id: WindowId, reason: DismissReason) -> bool {
        let Some(i) = self.pane_of(id) else {
            return false;
        };
        self.panes[i].core.dismiss_window(id, reason);
        let events = self.panes[i].core.take_pending_events();
        self.route_events(events)
    }

    /// Feeds a move the pressed pane received to every popup armed into
    /// that press, in that popup's own coordinates (ADR 0009 decision 2).
    ///
    /// The OS gives a captured drag to the window of the mouse-down, so a
    /// press on a combobox field and a drag over its menu arrive here, at
    /// the owner, and the menu is sent nothing at all. This is the driver
    /// doing what `NSMenu`'s tracking loop, Win32's menu message loop and a
    /// GTK pointer grab do — moving the events, since it cannot move the
    /// drag — and the popup's core is never told: hover, `hover_bg`,
    /// `onHover` and the popup's own drag state all follow from an ordinary
    /// `CursorMoved`. Dragging off the list costs one `CursorLeft` to the
    /// popup left behind, so a row stops highlighting the way a native
    /// menu's does, and staying off it costs nothing.
    ///
    /// The owner keeps every move it would have had (decision 3): nothing
    /// is withheld from it here and nothing is added to it.
    pub(super) fn retarget_move(&mut self, event_loop: &ActiveEventLoop, from: WindowId, p: Vec2) {
        if self.primary_down != Some(from) || self.armed.is_empty() {
            return;
        }
        let Some(owner) = self.pane_of(from).and_then(|i| self.panes[i].surface()) else {
            return;
        };
        // By id rather than by index: a popup that answers one of these
        // moves by closing takes its own entry out of `armed` underneath us.
        let armed: Vec<WindowId> = self.armed.iter().map(|a| a.id).collect();
        for id in armed {
            // The pane the press is in is armed only so its own release is
            // classified (decision 4's last paragraph); it already has
            // these moves first-hand.
            if id == from {
                continue;
            }
            let Some(k) = self.armed.iter().position(|a| a.id == id) else {
                continue;
            };
            let Some(j) = self.pane_of(id) else { continue };
            let Some(popup) = self.panes[j].surface() else {
                continue;
            };
            let at = retarget::retarget(&owner, p, &popup);
            let was = std::mem::replace(&mut self.armed[k].inside, at.is_some());
            match at {
                Some(q) => {
                    self.panes[j].cursor = q;
                    self.dispatch(event_loop, j, InputEvent::CursorMoved(q));
                }
                None if was => {
                    self.dispatch(event_loop, j, InputEvent::CursorLeft);
                }
                None => {}
            }
        }
    }

    /// Classifies the primary release that ends an armed press (ADR 0009
    /// decision 4), and disarms it whatever the answer.
    ///
    /// **Over an armed popup**, the driver synthesises the press-and-release
    /// that popup never saw, straight into its core rather than back through
    /// the `MouseInput` arm below — so the press-outside rule never sees it
    /// and cannot dismiss the window it is choosing from. The popup's core
    /// then does everything a real press-and-release there does: the focus
    /// move, the pressed styling, the click sound, and the
    /// `pressed == hovered` check that makes it a `click` at all. Neither
    /// input event carries a point, so the core presses wherever its cursor
    /// is; the retargeted moves have already put it there, and the
    /// `CursorMoved` below is for the release that arrives without one.
    ///
    /// **Inside the anchor**, nothing: the press opened the menu and the
    /// release on the field keeps it, which is how this gesture degrades
    /// into the two-click interaction that was here first.
    ///
    /// **Anywhere else**, every armed popup is asked to go away, because a
    /// native menu closes when the pointer is dragged off it and released.
    /// That is also the measured case in backlog W2 — pressed in the popup
    /// and dragged off its top edge — where the pane the press is in is the
    /// armed popup itself: a release over it is its own release and gets no
    /// synthetic pair, and a release over the field it hangs under is that
    /// popup's own anchor, mapped into the window the press is in, and
    /// keeps the menu.
    pub(super) fn classify_release(
        &mut self,
        event_loop: &ActiveEventLoop,
        from: WindowId,
        p: Vec2,
    ) {
        let armed = std::mem::take(&mut self.armed);
        if armed.is_empty() {
            return;
        }
        let Some(pressed) = self.pane_of(from).and_then(|i| self.panes[i].surface()) else {
            return;
        };
        // A popup whose window will not say where it is cannot be landed
        // on; an empty surface contains nothing, which is that answer.
        let nowhere = retarget::Surface {
            origin: (0.0, 0.0),
            scale: 1.0,
            size: Size::ZERO,
        };
        let surfaces: Vec<retarget::Surface> = armed
            .iter()
            .map(|a| {
                self.pane_of(a.id)
                    .and_then(|j| self.panes[j].surface())
                    .unwrap_or(nowhere)
            })
            .collect();
        // The last armed popup's anchor, in the coordinates of the window
        // the press is in. Usually it is already in them — the popup was
        // opened against a field in that very window — but for a press that
        // began *inside* the popup the field is a rect of the window next
        // door, and it maps here the way a point does, through the screen.
        // That is the measured case in backlog W2: dragged off the popup's
        // top edge, the pointer is over the field, and releasing there
        // keeps the menu.
        let anchor = armed
            .iter()
            .rev()
            .find_map(|a| {
                let j = self.pane_of(a.id)?;
                let (rect, owner) = (self.panes[j].anchor, self.panes[j].owner);
                if owner == from {
                    return Some(rect);
                }
                let space = self.pane_of(owner).and_then(|k| self.panes[k].surface())?;
                let tl = pressed.to_local(space.to_screen(Vec2::new(rect.x, rect.y)));
                let br =
                    pressed.to_local(space.to_screen(Vec2::new(rect.x + rect.w, rect.y + rect.h)));
                Some(Rect::new(tl.x, tl.y, br.x - tl.x, br.y - tl.y))
            })
            .unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0));
        match retarget::landing(&pressed, p, anchor, &surfaces) {
            retarget::Landing::Popup { index, at } => {
                let id = armed[index].id;
                if id == from {
                    return;
                }
                if let Some(j) = self.pane_of(id)
                    && self.panes[j].cursor != at
                {
                    self.panes[j].cursor = at;
                    self.dispatch(event_loop, j, InputEvent::CursorMoved(at));
                }
                for ev in [
                    InputEvent::MouseDown {
                        button: MouseButton::Primary,
                        clicks: 1,
                    },
                    InputEvent::MouseUp {
                        button: MouseButton::Primary,
                    },
                ] {
                    // Re-found each time: the press can close the window the
                    // release is for, and then there is nothing to release.
                    let Some(j) = self.pane_of(id) else { return };
                    self.dispatch(event_loop, j, ev);
                }
            }
            retarget::Landing::Anchor => {}
            retarget::Landing::Outside => {
                for a in &armed {
                    self.dismiss(a.id, DismissReason::Outside);
                }
            }
        }
    }
}
