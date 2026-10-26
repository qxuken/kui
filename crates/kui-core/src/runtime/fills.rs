//! Slots on the core's side:
//! declaring one (`begin_slot`), and the bounded, namespaced fill an
//! extension draws into (`fill`). The loop over extensions is
//! `crate::slot`'s; what is here is what only the core can do, because
//! it owns the node stack and the keys.

use super::*;
use crate::slot::{Children, Instance};

/// A fill being drawn: the instance it draws, and what its host's
/// children are built against.
#[derive(Clone)]
pub(crate) struct FillFrame {
    key: Key,
    /// Whether the placement passed children, which the fill may ask for.
    instance: bool,
    /// The origin that placed it, and the namespace its nodes were keyed
    /// in: what the host's children are built under.
    placer_origin: OriginId,
    placer_ns: (usize, Key),
    /// The label scope the placer's nodes were named in: the host's
    /// children's.
    placer_label: Key,
    /// The groups whose children were built, hashed: each once.
    built: Vec<u64>,
}

impl FillFrame {
    /// The instance this fill draws.
    pub(crate) fn key(&self) -> Key {
        self.key
    }
}

/// What `Core::begin_children` changed, for `end_children` to put back.
pub(crate) struct ChildrenState {
    origin: OriginId,
    ns: (usize, Key),
    counter: u64,
    first: u32,
    depth: usize,
    group_key: Key,
    hole: Key,
    label_ns: Key,
}

impl Core {
    /// The key the `root` of the extension `full` (`ns/root`) is drawn
    /// under after the host's view — `parent.str(full)`, recorded for
    /// `key_of` — or `None` outside a frame (ADR 0067: an extension
    /// providing nothing else is placed there unless the host placed it).
    /// The contribution slot of the placement about to be filled, taken:
    /// what its instance reads as `slot` (ADR 0067). Taken at the fill's
    /// start, so a component the fill places inside is a plain placement.
    pub(crate) fn take_placing_slot(&mut self) -> Option<Box<str>> {
        self.placing_slot.take()
    }

    pub(crate) fn set_placing_slot(&mut self, slot: Option<&str>) {
        self.placing_slot = slot.map(Into::into);
    }

    pub(crate) fn root_fill_key(&mut self, full: &str) -> Option<Key> {
        if self.tree.is_empty() {
            return None;
        }
        let key = self.parent_key().str(full);
        self.key_labels.push(key, full, self.origin, self.label_ns);
        Some(key)
    }

    /// Runs `f` as the fill of `slot` under `origin`: every node it opens
    /// is tagged `origin`, keyed as a child of `slot.key` (an extension's keys depend on the slot's
    /// full name, which the host's namespace makes its own, and on
    /// nothing the host built around them), counted from zero, and
    /// closed for it if it returns with any open (decision 5, with the
    /// `unbalanced-extension` warning). The host's origin, counter and
    /// namespace are what they were when `f` returns, so the host's next
    /// child is keyed as if the fill had not happened.
    pub fn fill(&mut self, slot: &Instance<'_>, origin: OriginId, f: impl FnOnce(&mut Ui<'_>)) {
        self.fill_within(slot, origin, None, f);
    }

    /// `fill`, with `filler` answering the slots the fill itself declares
    /// — what `Extensions::fill_one` hands in, so an extension can host
    /// extensions of its own (see `crate::slot`). `None` is plain `fill`:
    /// nothing answers, so a slot declared inside draws nothing.
    pub fn fill_within(
        &mut self,
        slot: &Instance<'_>,
        origin: OriginId,
        filler: Option<&mut dyn crate::slot::Fill>,
        f: impl FnOnce(&mut Ui<'_>),
    ) {
        self.fill_frame(slot, origin, filler, None, f);
    }

    /// `fill_within` for a component instance (ADR 0060): `children`
    /// builds the host's own children where the fill asks for them, with
    /// `Ui::children`.
    pub fn fill_instance(
        &mut self,
        slot: &Instance<'_>,
        origin: OriginId,
        filler: Option<&mut dyn crate::slot::Fill>,
        children: &mut dyn Children,
        f: impl FnOnce(&mut Ui<'_>),
    ) {
        self.fill_frame(slot, origin, filler, Some(children), f);
    }

    fn fill_frame(
        &mut self,
        slot: &Instance<'_>,
        origin: OriginId,
        filler: Option<&mut dyn crate::slot::Fill>,
        children: Option<&mut dyn Children>,
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
        let saved_label = self.label_ns;
        self.origin = origin;
        self.ns_depth = depth;
        self.ns_key = slot.key;
        self.label_ns = slot.key;
        let first = self.tree.len() as u32;
        self.fill_frames.push(FillFrame {
            key: slot.key,
            instance: children.is_some(),
            placer_origin: saved_origin,
            placer_ns: saved_ns,
            placer_label: saved_label,
            built: Vec::new(),
        });
        // The fill the host asked to keep begins here (ADR 0045).
        self.begin_keeping(slot, origin);
        let span = self.perf_fill_begin(origin, slot.namespace);

        f(&mut Ui::parts(
            self,
            filler.map(crate::ui::shorter_fill),
            children.map(crate::ui::shorter_children),
        ));
        self.fill_frames.pop();
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
        self.end_keeping(slot);
        self.span_end(span);
        if let Some(c) = self.counters.last_mut() {
            *c = saved_counter;
        }
        self.origin = saved_origin;
        (self.ns_depth, self.ns_key) = saved_ns;
        self.label_ns = saved_label;
    }

    /// The slot the nodes opened right now belong to: the fill being
    /// drawn, `None` in the host's own view (or its host children).
    pub(crate) fn current_fill_slot(&self) -> Option<Key> {
        (self.ns_depth != usize::MAX).then_some(self.ns_key)
    }

    /// A component's fill asks for its host's children of `group` here
    /// (ADR 0060): the host's origin, namespace and label scope come back,
    /// the children are keyed under `instance.str(group)` — so their keys
    /// do not depend on how the fill nests them — and their range is not
    /// the instance's, so their events are the host's. `None`, with
    /// `children-misplaced`, outside an instance's fill or for a group
    /// already built.
    pub(crate) fn begin_children(&mut self, group: &str) -> Option<ChildrenState> {
        if self.tree.is_empty() {
            return None;
        }
        let Some(frame) = self.fill_frames.last_mut() else {
            self.warn(crate::diag::children_misplaced(
                Key::ROOT,
                group,
                "in the host's own view",
            ));
            return None;
        };
        if !frame.instance {
            let key = frame.key;
            self.warn(crate::diag::children_misplaced(
                key,
                group,
                "by a fill placed with no children: an extension's root, or a devtools tab's",
            ));
            return None;
        }
        let h = crate::key::hash_bulk(group.as_bytes());
        if frame.built.contains(&h) {
            let key = frame.key;
            self.warn(crate::diag::children_misplaced(key, group, "twice"));
            return None;
        }
        frame.built.push(h);
        let (key, placer_origin, placer_ns, placer_label) = (
            frame.key,
            frame.placer_origin,
            frame.placer_ns,
            frame.placer_label,
        );
        if self.keeping() {
            self.keep_op(replay::Op::Children(group.into()));
        }
        // The host's nodes are not the fill's: a replay builds them again.
        self.pause_keeping();
        let state = ChildrenState {
            origin: self.origin,
            ns: (self.ns_depth, self.ns_key),
            counter: self.counters.last().copied().unwrap_or(0),
            first: self.tree.len() as u32,
            depth: self.stack.len(),
            group_key: key.str(group),
            hole: if placer_ns.0 == usize::MAX {
                crate::tree::NO_SLOT
            } else {
                placer_ns.1
            },
            label_ns: self.label_ns,
        };
        // Inside the children it is the placer's context again: a
        // `children()` there is the placer's own (its host's children,
        // forwarded), and a fill below it is the placer's frame's.
        let below = self.fill_frames.len().checked_sub(2);
        let placer_frame = match below {
            Some(i) => self.fill_frames[i].clone(),
            None => FillFrame {
                key: Key::ROOT,
                instance: false,
                placer_origin: OriginId::HOST,
                placer_ns: (usize::MAX, Key::ROOT),
                placer_label: Key::ROOT,
                built: Vec::new(),
            },
        };
        self.fill_frames.push(placer_frame);
        self.origin = placer_origin;
        self.ns_depth = self.stack.len();
        self.ns_key = state.group_key;
        self.label_ns = placer_label;
        if let Some(c) = self.counters.last_mut() {
            *c = 0;
        }
        Some(state)
    }

    /// The other half of [`Self::begin_children`]: what the host's
    /// children left open is closed, their range recorded as the
    /// placer's, and the fill's own context put back.
    pub(crate) fn end_children(&mut self, state: ChildrenState) {
        self.fill_frames.pop();
        if self.stack.len() > state.depth {
            let open = self.stack.len() - state.depth;
            self.diag.raise(crate::diag::unbalanced_extension(
                "the host's children",
                state.group_key,
                open,
            ));
            self.stack.truncate(state.depth);
            self.counters.truncate(state.depth);
        }
        self.tree
            .fills
            .push((state.hole, state.first, self.tree.len() as u32));
        if let Some(c) = self.counters.last_mut() {
            *c = state.counter;
        }
        self.origin = state.origin;
        (self.ns_depth, self.ns_key) = state.ns;
        self.label_ns = state.label_ns;
        self.resume_keeping();
    }
}
