//! Slots on the core's side (`docs/adr/0014-slots-an-extension-fills-in-place.md`):
//! declaring one (`begin_slot`), and the bounded, namespaced fill an
//! extension draws into (`fill`). The loop over extensions is
//! `crate::slot`'s; what is here is what only the core can do, because
//! it owns the node stack and the keys.

use super::*;
use crate::slot::Slot;

impl Core {
    /// Declares a slot under its full name at the cursor and returns its
    /// key — `parent.str(name)`, recorded for `key_of` — or `None` when it
    /// cannot be declared: outside a frame, or a second time in one
    /// frame, which is the `duplicate-slot` warning.
    ///
    /// An extension may declare one too, and its key nests under the slot
    /// the extension is itself filling, like any of its nodes. Names stay
    /// frame-wide because namespaces are: there is one extension called
    /// `todos` however deep the thing that loaded it sat, so `todos/panel`
    /// is one slot and declaring it twice is the same warning wherever
    /// the two declarations came from.
    pub fn begin_slot(&mut self, name: &str) -> Option<Key> {
        if self.tree.is_empty() {
            return None;
        }
        if self.slot_labels.find(name).next().is_some() {
            self.diag.raise(crate::diag::duplicate_slot(name));
            return None;
        }
        let key = self.parent_key().str(name);
        self.slot_labels.push(key, name, self.origin);
        self.key_labels.push(key, name, self.origin);
        Some(key)
    }

    /// Whether the full name `name` was declared this frame so far — or
    /// is the slot of a devtools tab declared this frame, which counts
    /// whether or not the panel mounted it, so a plugin whose only slot
    /// is a tab is quiet with the panel off (ADR 0032, decision 5).
    pub fn slot_declared(&self, name: &str) -> bool {
        self.slot_labels.find(name).next().is_some() || self.devtools_tab_slot(name)
    }

    /// Runs `f` as the fill of `slot` under `origin`: every node it opens
    /// is tagged `origin`, keyed as a child of `slot.key` (the namespace
    /// of ADR 0014 decision 4 — an extension's keys depend on the slot's
    /// full name, which the host's namespace makes its own, and on
    /// nothing the host built around them), counted from zero, and
    /// closed for it if it returns with any open (decision 5, with the
    /// `unbalanced-extension` warning). The host's origin, counter and
    /// namespace are what they were when `f` returns, so the host's next
    /// child is keyed as if the fill had not happened.
    pub fn fill(&mut self, slot: &Slot<'_>, origin: OriginId, f: impl FnOnce(&mut Ui<'_>)) {
        self.fill_within(slot, origin, None, f);
    }

    /// `fill`, with `filler` answering the slots the fill itself declares
    /// — what `Extensions::fill_one` hands in, so an extension can host
    /// extensions of its own (see `crate::slot`). `None` is plain `fill`:
    /// nothing answers, so a slot declared inside draws nothing.
    pub fn fill_within(
        &mut self,
        slot: &Slot<'_>,
        origin: OriginId,
        filler: Option<&mut dyn crate::slot::Fill>,
        f: impl FnOnce(&mut Ui<'_>),
    ) {
        if self.tree.is_empty() {
            return;
        }
        let depth = self.stack.len();
        let saved_counter = self.counters.last().copied().unwrap_or(0);
        if let Some(c) = self.counters.last_mut() {
            *c = 0;
        }
        let saved_origin = self.origin;
        let saved_ns = (self.ns_depth, self.ns_key);
        self.origin = origin;
        self.ns_depth = depth;
        self.ns_key = slot.key;
        let first = self.tree.len() as u32;

        match filler {
            Some(filler) => f(&mut Ui::with_filler(self, filler)),
            None => f(&mut Ui::new(self)),
        }
        // After the fills inside it, so the innermost is found first.
        self.tree
            .fills
            .push((slot.key, first, self.tree.len() as u32));

        if self.stack.len() > depth {
            let open = self.stack.len() - depth;
            self.diag.raise(crate::diag::unbalanced_extension(
                &slot.full_name(),
                slot.key,
                open,
            ));
            self.stack.truncate(depth);
            self.counters.truncate(depth);
        }
        if let Some(c) = self.counters.last_mut() {
            *c = saved_counter;
        }
        self.origin = saved_origin;
        (self.ns_depth, self.ns_key) = saved_ns;
    }
}
