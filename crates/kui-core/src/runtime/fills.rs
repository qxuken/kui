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
    /// cannot be declared: outside a frame, inside another fill (an
    /// extension's `slot` declares nothing; whether one may offer slots
    /// is a decision for the day one asks), or a second time in one
    /// frame, which is the `duplicate-slot` warning.
    pub fn begin_slot(&mut self, name: &str) -> Option<Key> {
        if self.tree.is_empty() || self.ns_depth != usize::MAX {
            return None;
        }
        if self.slot_declared(name) {
            self.diag.raise(crate::diag::duplicate_slot(name));
            return None;
        }
        let key = self.parent_key().str(name);
        self.slot_labels.push(key, name);
        self.key_labels.push(key, name);
        Some(key)
    }

    /// Whether the full name `name` was declared this frame so far.
    pub fn slot_declared(&self, name: &str) -> bool {
        self.slot_labels.find(name).next().is_some()
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

        f(&mut Ui::new(self));

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
