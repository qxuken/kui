//! Keyboard focus and the Tab ring (`docs/adr/0002-keyboard-focus-as-data.md`),
//! and the modal scope that narrows both (`docs/adr/0003-modal-surfaces.md`):
//! the one focus, who is in the ring, what moving it does to the edit store
//! and the held keys, and the focus a modal displaces and gives back.

use super::*;

impl Core {
    /// Moves keyboard focus to the next / previous focusable node in tree
    /// order (from the last laid-out frame; see `access::focusable`),
    /// wrapping around; with no current focus, enters the first (or last,
    /// going backwards). What Tab does. The landing node scrolls into
    /// view, and the focus shows (ring or `focus_bg`), as keyboard focus
    /// should.
    pub fn focus_next(&mut self, forward: bool) {
        let ring = self.focus_ring();
        if ring.is_empty() {
            return;
        }
        let at = self
            .focus
            .and_then(|cur| ring.iter().position(|(_, k)| *k == cur));
        let (i, key) = match at {
            Some(p) if forward => ring[(p + 1) % ring.len()],
            Some(p) => ring[(p + ring.len() - 1) % ring.len()],
            None if forward => ring[0],
            None => ring[ring.len() - 1],
        };
        self.set_focus(Some(key));
        self.focus_visible = true;
        let rect = Rect::from_pos_size(self.tree.pos[i], self.tree.size[i]);
        self.scroll_rect_into_view(i, rect, false);
    }

    /// The Tab ring: (tree index, key) of every focusable node of the
    /// last frame in tree order, `role="none"` subtrees skipped whole —
    /// and, when a modal is in effect, of its subtree only (see
    /// `docs/adr/0003-modal-surfaces.md`).
    ///
    /// A composite contributes **one** stop instead of one per item
    /// (`docs/adr/0007-composite-keyboard-patterns.md`): the walk enters
    /// the container as usual — a focusable node inside it but outside
    /// every item, a "+" at the end of a tab bar, keeps its own stop — and
    /// then takes the whole of each item's subtree in one step, emitting
    /// the stop only at the item [`Self::composite_entry`] picked. So the
    /// stop sits where that item sits in tree order, and Tab out and back
    /// lands on it again.
    fn focus_ring(&self) -> Vec<(usize, Key)> {
        use crate::access::Role;
        let mut out = Vec::new();
        let (mut i, end) = match self.modal {
            Some((start, end, _)) => (start, end),
            None => (0, self.tree.len()),
        };
        // The composites enclosing `i`, innermost last: how far each runs,
        // which role its items carry, and the one item that is its stop.
        let mut open: Vec<(usize, Role, Option<usize>)> = Vec::new();
        let mut items = Vec::new();
        while i < end {
            while open.last().is_some_and(|(e, _, _)| i >= *e) {
                open.pop();
            }
            let role = self.tree.specs[i].access().role;
            if role == Some(Role::None) {
                i = self.tree.subtree_end(i);
                continue;
            }
            if let Some(&(_, item, chosen)) = open.last()
                && role == Some(item)
            {
                if chosen == Some(i) {
                    out.push((i, self.tree.keys[i]));
                }
                i = self.tree.subtree_end(i);
                continue;
            }
            if let Some(item) = role.and_then(crate::composite::item_role) {
                crate::composite::items(&self.tree, i, item, &mut items);
                if crate::composite::is_composite(&self.tree, i, &items) {
                    let chosen = self.composite_entry(&items);
                    open.push((self.tree.subtree_end(i), item, chosen));
                }
            }
            // The root is never a stop. It encloses everything, so a sink
            // on it hears every key nothing below claims (ADR 0011) and a
            // view may focus it outright — but a Tab stop is a thing the
            // user acts on, and the whole window is not one. Without this
            // a root sink drew the ring around the window.
            if i != 0 && crate::access::focusable(&self.tree, i) {
                out.push((i, self.tree.keys[i]));
            }
            i += 1;
        }
        out
    }

    /// Which item of a composite is its Tab stop, in precedence order
    /// (`docs/adr/0007`, decision 4): the item that currently holds focus,
    /// else one declaring `initial_focus`, else the one declaring
    /// `selected`, else the first. **No new retained state** — the roving
    /// tabindex a browser keeps per composite is, in every pattern kui has,
    /// the item the app already marks `selected`, and the same fact that
    /// tells a reader which one is current tells the keyboard where to
    /// enter. Only focusable items are candidates, so a disabled one is
    /// skipped here as it is everywhere else.
    fn composite_entry(&self, items: &[usize]) -> Option<usize> {
        let live = |&&j: &&usize| crate::access::focusable(&self.tree, j);
        let pick = |f: &dyn Fn(usize) -> bool| items.iter().filter(live).copied().find(|&j| f(j));
        pick(&|j| Some(self.tree.keys[j]) == self.focus)
            .or_else(|| pick(&|j| self.tree.specs[j].initial_focus))
            .or_else(|| pick(&|j| self.tree.specs[j].access().selected))
            .or_else(|| items.iter().filter(live).copied().next())
    }

    /// The frame's modal scope: the tree range of the last node declaring
    /// `modal`, and its key. The last one wins, so a confirm declared
    /// inside (or after) a dialog is the one in effect and the dialog
    /// under it is as inert as the app under the dialog.
    pub(crate) fn modal_scope(&self) -> Option<(usize, usize, Key)> {
        let i = (0..self.tree.len())
            .rev()
            .find(|&i| self.tree.specs[i].events().modal.is_some())?;
        Some((i, self.tree.subtree_end(i), self.tree.keys[i]))
    }

    /// The key of the modal in effect this frame, if any.
    pub fn modal(&self) -> Option<Key> {
        self.modal.map(|(_, _, key)| key)
    }

    /// Whether node `i` takes input: everything does, until a modal is in
    /// effect — then its subtree does, and so does window chrome (the
    /// window's own controls belong to the platform, not to the dialog).
    pub(crate) fn interactive(&self, i: usize) -> bool {
        match self.modal {
            Some((start, end, _)) => {
                (start..end).contains(&i) || self.tree.specs[i].window.is_some()
            }
            None => true,
        }
    }

    /// Whether node `key` is inside the frame's modal scope (nothing is
    /// outside one when there is no modal).
    pub(crate) fn within_modal(&self, key: Key) -> bool {
        let Some((start, end, _)) = self.modal else {
            return true;
        };
        self.tree.keys[start..end].contains(&key)
    }

    /// Focus follows the modal: a modal that appears remembers the focus
    /// it displaces and takes focus into itself; one that stops being
    /// declared gives that focus back. Run once per frame, after the
    /// scope is known.
    pub(crate) fn resolve_modal_focus(&mut self) {
        // This frame's modals in tree order, each carrying the focus it
        // displaced when it first appeared.
        let mut now: Vec<(Key, Option<Key>)> = Vec::new();
        if self.any_modal {
            for i in 0..self.tree.len() {
                if self.tree.specs[i].events().modal.is_none() {
                    continue;
                }
                let key = self.tree.keys[i];
                let saved = self
                    .modal_focus
                    .iter()
                    .find(|(k, _)| *k == key)
                    .map_or(self.focus, |(_, f)| *f);
                now.push((key, saved));
            }
        }
        // The outermost modal that went away hands its focus back, so a
        // dialog and the confirm inside it closing together land where
        // the dialog was opened from.
        let closed = self
            .modal_focus
            .iter()
            .find(|(k, _)| !now.iter().any(|(n, _)| n == k))
            .map(|(_, saved)| saved.filter(|key| self.tree.keys.contains(key)));
        // A `keyFocus` *edge* on this same frame — a node declared focused
        // now and not last frame — is the app saying where focus lands on
        // the way out, and it wins over the restore (ADR 0003, decision
        // 4): a rename editor opened on a node created in the same frame
        // displaced the node the user was on *before*, and the restore
        // is the default for an app that says nothing, not a rule for one
        // that did. A sink redeclaring itself every frame is no edge, so
        // the restore still lands where it always has under one.
        let edge = self
            .declared_focus
            .iter()
            .any(|k| !self.declared_focus_last.contains(k));
        if let Some(saved) = closed
            && !edge
        {
            // Exactly what it displaced, nothing included: leaving focus
            // on the dismissed modal's own button would be a focus on a
            // node that is not there any more.
            self.set_focus(saved);
        }
        self.modal_focus = now;
        // Containment: focus outside the scope enters it, or is dropped
        // when it holds none. Not `focus_visible`: the app showed the
        // modal, nobody pressed a key. The entry is the first node in the
        // ring declaring `initial_focus` — a destructive confirm opening
        // on its Cancel — and the ring's first when none does. Only
        // *entry* reads it: focus already inside the scope never reaches
        // here, so a Tab press stands and a nested confirm closing leaves
        // focus where it handed it back.
        if self.modal.is_some() && !self.focus.is_some_and(|k| self.within_modal(k)) {
            let ring = self.focus_ring();
            let entry = ring
                .iter()
                .find(|(i, _)| self.tree.specs[*i].initial_focus)
                .or_else(|| ring.first())
                .map(|(_, k)| *k);
            self.set_focus(entry);
        }
    }

    /// Where a primary press on node `key` puts the keyboard: the node
    /// itself when it is focusable, and nothing when it is not — except
    /// on window chrome, which leaves focus alone, and inside a key sink,
    /// which keeps the keyboard through any press landing in its subtree.
    ///
    /// A sink is an app that owns its keyboard (`docs/adr/0002`, decision
    /// 3, the same reason Tab stays with it). The panes, buttons and dead
    /// space it draws are that app's surface, and a derived button among
    /// them is focusable enough to be a Tab stop without being entitled to
    /// take the keys away from the app drawing it — otherwise the first
    /// click anywhere in a multiplexer kills every chord, permanently,
    /// because `take_key_focus` is edge-triggered and will not ask twice.
    /// An editor or a nested sink is its own keyboard owner and still
    /// takes focus (the editor through the caret arm above).
    pub(crate) fn press_focus(&self, key: Key, focusable: bool) -> Option<Key> {
        let Some(i) = self.tree.keys.iter().position(|k| *k == key) else {
            return focusable.then_some(key);
        };
        // Window chrome belongs to the platform, not to the app (decision
        // 2, the same reason it is never a Tab stop): dragging a window by
        // its titlebar, or pressing minimize, is not the app being asked
        // to give up its keyboard.
        if self.tree.specs[i].window.is_some() {
            return self.focus;
        }
        if self.tree.specs[i].events().on_key.is_some() {
            return Some(key);
        }
        match self.enclosing_sink(i) {
            Some(j) => Some(self.tree.keys[j]),
            None => focusable.then_some(key),
        }
    }

    /// The nearest key sink strictly above node `i`: the walk
    /// [`Self::press_focus`] makes for a press, made for a key as well
    /// (`docs/adr/0011-keys-bubble-to-the-enclosing-sink.md`, decision 1).
    ///
    /// A disabled node is not a sink at all (`docs/adr/0002`, decision 6),
    /// so it neither answers here nor hides a live sink further up. The
    /// walk stops at the modal boundary rather than climbing through it:
    /// the app around a dialog is inert (`docs/adr/0003`), and a shell that
    /// kept hearing shortcuts while its own dialog was up would be running
    /// commands against a surface the user cannot see the state of.
    pub(crate) fn enclosing_sink(&self, i: usize) -> Option<usize> {
        let mut n = self.tree.parent[i];
        while n != crate::tree::NIL {
            let j = n as usize;
            if !self.interactive(j) {
                return None;
            }
            if self.tree.specs[j].events().on_key.is_some() && !self.tree.specs[j].disabled {
                return Some(j);
            }
            n = self.tree.parent[j];
        }
        None
    }

    /// The focused node's index in the last frame, if it is there.
    pub(crate) fn focus_index(&self) -> Option<usize> {
        let key = self.focus?;
        self.tree.keys.iter().position(|k| *k == key)
    }

    /// Whether `key` holds keyboard focus — any node (see `focus`).
    pub fn is_focused(&self, key: Key) -> bool {
        self.focus == Some(key)
    }

    /// The node holding keyboard focus: an editor, an `on_key` sink, or
    /// a control Tab (or assistive technology, or `set_focus`) put it on.
    pub fn focus(&self) -> Option<Key> {
        self.focus
    }

    /// Whether focus got where it is by keyboard or assistive technology
    /// rather than a click — when it shows (the default ring, or the
    /// node's `focus_bg`).
    pub fn focus_visible(&self) -> bool {
        self.focus_visible
    }

    /// Moves keyboard focus (None blurs). The one writer: the edit store
    /// mirrors it for editor keys, and a landing editor scrolls its caret
    /// into view. Any node can be focused this way; only focusable ones
    /// (see `access::focusable`) are reached by Tab.
    pub fn set_focus(&mut self, key: Option<Key>) {
        let edit_key = key.filter(|k| self.edit.contains(*k));
        if self.focus != key {
            // The keys the leaving sink is holding come up first, while it
            // is still the focus that routing resolves against.
            self.release_held_keys();
            if let Some(k) = edit_key {
                self.edit.caret_moved = Some(k);
            }
        }
        self.focus = key;
        self.edit.set_focus(edit_key);
    }

    /// Asks for a Tab step (`forward`) / Shift-Tab step at the end of the
    /// frame being built. `focus_next` moves focus now, against the last
    /// finished tree — which is what a driver handling a key press between
    /// frames wants, and exactly what a *view* cannot use, since its own
    /// tree does not exist yet. A view asks with this instead and the step
    /// lands on the frame it is declaring.
    pub fn request_focus_step(&mut self, forward: bool) {
        self.pending_focus_step = Some(forward);
        self.request_frame();
    }

    /// Declares a node focused this frame (None blurs at once). The
    /// declaration is edge-triggered: the node takes focus on the first
    /// frame it is declared and keeps being declared without effect
    /// afterwards, so a view that repeats it every frame (an app that
    /// owns its keyboard, a `keyFocus` prop) does not clobber the focus a
    /// Tab press or a click moved. Programmatic focus keeps the modality
    /// of the last input (it shows after keyboard use, not after a
    /// click). To move focus at any time, `set_focus`.
    pub fn set_key_focus(&mut self, key: Option<Key>) {
        let Some(k) = key else {
            self.set_focus(None);
            return;
        };
        if !self.declared_focus.contains(&k) {
            self.declared_focus.push(k);
        }
        if !self.declared_focus_last.contains(&k) {
            self.set_focus(Some(k));
        }
    }

    /// The node holding keyboard focus (the same as `focus`; kept from
    /// when only key sinks and editors could).
    pub fn key_focus(&self) -> Option<Key> {
        self.focus
    }

    /// The keys that declared key focus while the current frame was built
    /// (`set_key_focus`). The scene corpus's coverage derivation reads it:
    /// `keyFocus` leaves no mark on the tree, and the focus it takes is
    /// indistinguishable from the focus a click takes.
    #[cfg(feature = "conformance")]
    pub(crate) fn declared_focus(&self) -> &[Key] {
        &self.declared_focus
    }
}
