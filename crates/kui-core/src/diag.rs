//! Diagnostics as data. The misconfigurations that fail silently — a grow
//! weight with nothing to split against, a transition on a positional key
//! under a sibling list that changes, two nodes sharing one key — all look
//! like "the feature is broken" from the outside. The core can see them,
//! so it reports them the way it reports everything else: as plain data a
//! driver drains ([`crate::Core::take_warnings`]). The windowed runners
//! print them; a headless test asserts on them, or on their absence.
//!
//! Each distinct (code, node) pair is reported once per core, so a
//! condition that persists across frames costs one line, not a stream.
//! The checks walk the frame's tree (about 10 ns per node, measured), so
//! they run on the first two frames and then every [`CHECK_EVERY`] frames:
//! a misconfiguration persists, so it still surfaces within that many
//! frames, and the steady-state cost rounds to nothing. A bare `Core` runs
//! them (headless tests are development by definition); the drivers decide
//! for shipped apps — the Rust runner and the Node loops turn them on in
//! development builds only, a standalone C context starts with them off —
//! through [`crate::Core::set_diagnostics`].

use rustc_hash::{FxHashMap, FxHashSet};

use crate::access::{self, Role};
use crate::edit::EditStore;
use crate::key::Key;
use crate::spec::{Dir, Sizing};
use crate::text::TextSystem;
use crate::tree::{NIL, Tree};

/// A silent misconfiguration the core noticed while finishing a frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Warning {
    /// A stable identifier for the kind of problem: one of the constants
    /// in this module. Match on it; the message is for people.
    pub code: &'static str,
    /// The node the warning is about — the parent, for
    /// [`TRANSITION_AUTO_KEY`]; the shared key, for [`DUPLICATE_KEY`].
    pub key: Key,
    pub message: String,
}

/// A `Grow(f)` with `f != 1` on the only grow child of its parent (the
/// weight splits space between grow siblings, so alone it changes nothing)
/// or across the parent's main axis (cross-axis grow fills the parent
/// whatever its weight).
pub const GROW_WEIGHT_IGNORED: &str = "grow-weight-ignored";
/// The child count of a node changed while one of its children carries a
/// transition under an auto-assigned key. Auto keys are sibling positions,
/// so the children that shifted became new nodes and snapped instead of
/// easing. Give list items a key.
pub const TRANSITION_AUTO_KEY: &str = "transition-auto-key";
/// Two nodes in one frame share a key: everything retained per key
/// (transitions, scroll offsets, editors, layout events, hover state) is
/// mixed between them. Siblings need distinct labels.
pub const DUPLICATE_KEY: &str = "duplicate-key";
/// An image with no `label`: assistive technology has nothing to say
/// for it. Decorative images take `role="none"`.
pub const IMAGE_WITHOUT_LABEL: &str = "image-without-label";
/// A control (a button, link, tab, checkbox, slider, editor) with no
/// computable name: no `label`, and no text inside it. Icon buttons and
/// editors need a `label`.
pub const CONTROL_WITHOUT_NAME: &str = "control-without-name";

/// Pending warnings are capped so a host that never drains them cannot
/// grow the queue without bound.
const MAX_PENDING: usize = 256;
/// The checks run on the first two frames and every this many after.
const CHECK_EVERY: u64 = 16;

pub(crate) struct Diagnostics {
    pub(crate) enabled: bool,
    pending: Vec<Warning>,
    warned: FxHashSet<(&'static str, Key)>,
    /// Child count at the last check, for parents with an auto-keyed child
    /// that carries a transition.
    child_counts: FxHashMap<Key, u32>,
    scratch: Vec<u64>,
}

impl Default for Diagnostics {
    fn default() -> Self {
        Self {
            enabled: true,
            pending: Vec::new(),
            warned: FxHashSet::default(),
            child_counts: FxHashMap::default(),
            scratch: Vec::new(),
        }
    }
}

impl Diagnostics {
    pub(crate) fn take(&mut self) -> Vec<Warning> {
        std::mem::take(&mut self.pending)
    }

    fn warn(&mut self, code: &'static str, key: Key, message: impl FnOnce() -> String) {
        if self.pending.len() >= MAX_PENDING || !self.warned.insert((code, key)) {
            return;
        }
        self.pending.push(Warning {
            code,
            key,
            message: message(),
        });
    }

    /// Runs every check over the finished frame's tree, on the frames the
    /// cadence picks (see the module docs).
    pub(crate) fn check(
        &mut self,
        tree: &Tree,
        text: &TextSystem,
        edit: &EditStore,
        frame_no: u64,
    ) {
        if !self.enabled || tree.is_empty() {
            return;
        }
        if frame_no > 2 && !frame_no.is_multiple_of(CHECK_EVERY) {
            return;
        }
        self.check_grow_weights(tree);
        self.check_auto_keyed_transitions(tree);
        self.check_duplicate_keys(tree);
        self.check_access(tree, text, edit);
    }

    /// Images without a label and controls without a computable name, by
    /// the same derivation the access tree uses (see `access::semantic`).
    fn check_access(&mut self, tree: &Tree, text: &TextSystem, edit: &EditStore) {
        let mut skip_until = 0usize;
        for i in 0..tree.len() {
            if i < skip_until {
                continue;
            }
            let Some(sem) = access::semantic(tree, text, edit, None, i) else {
                continue;
            };
            if sem.role == Role::None || sem.presentational {
                skip_until = tree.subtree_end(i);
            }
            if sem.name.is_some() || sem.role == Role::None {
                continue;
            }
            let key = tree.keys[i];
            if sem.role == Role::Image {
                self.warn(IMAGE_WITHOUT_LABEL, key, || {
                    "this image has no label: assistive technology has nothing to say for it \
                     (give it a `label`, or `role=\"none\"` if it is decoration)"
                        .to_string()
                });
            } else if sem.role.is_control() {
                let what = sem.role.name();
                self.warn(CONTROL_WITHOUT_NAME, key, || {
                    format!(
                        "this {what} has no accessible name: no `label`, and no text inside it \
                         — a screen reader announces an unnamed {what} (give it a `label`)"
                    )
                });
            }
        }
    }

    fn check_grow_weights(&mut self, tree: &Tree) {
        for p in 0..tree.len() {
            if tree.first_child[p] == NIL {
                continue;
            }
            let row = tree.specs[p].layout.dir == Dir::Row;
            let mut grow_children = 0u32;
            let mut lone: Option<(u32, f32)> = None;
            for c in tree.children(p as u32) {
                let layout = tree.specs[c as usize].layout;
                if layout.float.is_some() {
                    continue;
                }
                let (main, cross) = if row {
                    (layout.width, layout.height)
                } else {
                    (layout.height, layout.width)
                };
                if let Sizing::Grow(f) = main {
                    grow_children += 1;
                    lone = Some((c, f));
                }
                if let Sizing::Grow(f) = cross
                    && f != 1.0
                {
                    let axis = if row { "height" } else { "width" };
                    let parent_axis = if row { "row" } else { "column" };
                    self.warn(GROW_WEIGHT_IGNORED, tree.keys[c as usize], || {
                        format!(
                            "{axis} grow {f} has no effect: across a {parent_axis}'s main axis a \
                             grow child fills the parent whatever its weight (use maxWidth / \
                             maxHeight to cap it)"
                        )
                    });
                }
            }
            if grow_children == 1
                && let Some((c, f)) = lone
                && f != 1.0
            {
                let axis = if row { "width" } else { "height" };
                self.warn(GROW_WEIGHT_IGNORED, tree.keys[c as usize], || {
                    format!(
                        "{axis} grow {f} has no effect: it is the only grow child of its parent, \
                         and a weight only splits free space between grow siblings — alone it \
                         takes all of it (cap it with max{}, or give a sibling a grow too)",
                        if row { "Width" } else { "Height" }
                    )
                });
            }
        }
    }

    fn check_auto_keyed_transitions(&mut self, tree: &Tree) {
        let mut counts: FxHashMap<Key, u32> = FxHashMap::default();
        for p in 0..tree.len() {
            if tree.first_child[p] == NIL {
                continue;
            }
            let parent_key = tree.keys[p];
            let mut n = 0u32;
            let mut auto_keyed_transition = false;
            for (i, c) in tree.children(p as u32).enumerate() {
                n += 1;
                // An auto key is the parent's key mixed with the sibling
                // index; a labeled key never collides with one.
                if tree.specs[c as usize].transition.is_some()
                    && tree.keys[c as usize] == parent_key.index(i as u64)
                {
                    auto_keyed_transition = true;
                }
            }
            if auto_keyed_transition {
                counts.insert(parent_key, n);
            }
        }
        for (&parent, &n) in &counts {
            if let Some(&prev) = self.child_counts.get(&parent)
                && prev != n
            {
                self.warn(TRANSITION_AUTO_KEY, parent, || {
                    format!(
                        "this node went from {prev} to {n} children while a child without a key \
                         carries a transition: an auto key is the child's position, so the \
                         children that shifted became new nodes and snapped instead of easing \
                         (and inserting before them will again) — give them a key"
                    )
                });
            }
        }
        self.child_counts = counts;
    }

    fn check_duplicate_keys(&mut self, tree: &Tree) {
        let mut keys = std::mem::take(&mut self.scratch);
        keys.clear();
        keys.extend(tree.keys.iter().map(|k| k.0));
        keys.sort_unstable();
        for w in keys.windows(2) {
            if w[0] == w[1] {
                self.warn(DUPLICATE_KEY, Key(w[0]), || {
                    "two nodes share this key in one frame: state retained per key (transitions, \
                     scroll offsets, editors, layout events, hover) is mixed between them — \
                     siblings need distinct labels"
                        .to_string()
                });
            }
        }
        self.scratch = keys;
    }
}
