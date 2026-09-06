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
//!
//! Five codes do not come from the tree walk. A prop name no table claims
//! ([`UNKNOWN_PROP`]) is gone by the time the frame is a tree, so the
//! binding that dropped it raises it through [`crate::Core::warn`], behind
//! the same gate and the same dedup — and a window kind no build has
//! ([`UNKNOWN_WINDOW_KIND`]) reaches the same door from `kui-ffi`, since
//! only C can name one. The two about declared windows
//! ([`DUPLICATE_WINDOW_CONFIG`], [`WINDOW_DECLARED_WHILE_CLOSED`]) come
//! from the core's diff of the declared set, which has no node to hang
//! them on: they are keyed by the window's name, the way `unknown-prop` is
//! keyed by element and prop, and the way the kind is keyed by the window
//! and the number. And a resource handle from another session
//! ([`FOREIGN_RESOURCE`]) is noticed wherever a handle resolves — under a
//! shaping closure, in the emitter, in the driver's audio backend — so the
//! session's registry keeps the hits and `take_warnings` raises them,
//! keyed by kind and handle.

use rustc_hash::{FxHashMap, FxHashSet};

use crate::access::{self, Role};
use crate::edit::EditStore;
use crate::key::Key;
use crate::resources::Foreign;
use crate::schema;
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

/// One warning code and what it means, for the tables the bindings
/// generate from the core (`docs/props.md`, the Node `WarningCode` union).
/// `doc` is the const's own doc comment, so there is one text to edit.
pub struct WarningDef {
    pub code: &'static str,
    pub doc: &'static str,
}

/// Declares the codes as the `pub const`s they have always been *and* the
/// [`WARNINGS`] table from the same tokens: the doc comment on each const
/// is the table's `doc`. A code added outside this block compiles, but
/// `every_code_is_in_the_table_and_vice_versa` fails, so the two cannot
/// drift.
macro_rules! warnings {
    ($( $(#[doc = $doc:literal])+ pub const $name:ident: &str = $code:literal; )*) => {
        $( $(#[doc = $doc])+ pub const $name: &str = $code; )*
        /// Every warning code with its description, in declaration order.
        pub const WARNINGS: &[WarningDef] = &[
            $( WarningDef { code: $code, doc: concat!($($doc, "\n"),+) }, )*
        ];
    };
}

warnings! {
    /// A grow weight other than 1 on the only grow child of its parent (the
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
    /// mixed between them. Siblings need distinct keys.
    pub const DUPLICATE_KEY: &str = "duplicate-key";
    /// An image with no `label`: assistive technology has nothing to say
    /// for it. Decorative images take `role="none"`.
    pub const IMAGE_WITHOUT_LABEL: &str = "image-without-label";
    /// A `line` declares `onClick`, `onDrag`, `onKey`, `onHover`, `hoverable`
    /// or `focusable`. A line takes no pointer input and emits no hit
    /// region — its bounding box is mostly not the stroke, and a
    /// shape-aware hit test is not built — so the declaration does nothing
    /// (`docs/adr/0010-a-segment-primitive.md`, decisions 7 and 8). Put the
    /// interaction on the nodes the line connects.
    pub const LINE_IGNORES_INPUT: &str = "line-ignores-input";
    /// The frame's modal surface is not in a float, and content painted after
    /// it is drawn on top of it: everything the user can see over the modal is
    /// inert, which looks like inert-behind is broken. A modal that has to
    /// cover the app is a float (`float="viewport"`); see
    /// `docs/adr/0003-modal-surfaces.md`.
    pub const MODAL_BEHIND_CONTENT: &str = "modal-behind-content";
    /// A control (a button, link, tab, checkbox, slider, editor) with no
    /// computable name: no `label`, and no text inside it. Icon buttons and
    /// editors need a `label`.
    pub const CONTROL_WITHOUT_NAME: &str = "control-without-name";
    /// A focusable node inside a composite's *item* — a button inside a list
    /// row, a link inside a tab. The item is one roving stop of a composite
    /// (`docs/adr/0007-composite-keyboard-patterns.md`), so the Tab ring
    /// stops at the item and nothing reaches what is inside it: declared, and
    /// impossible, which is what `modal-behind-content` set the precedent
    /// for. A focusable node inside the *container* but outside every item —
    /// a "+" at the end of a tab bar — is reachable and is not reported.
    pub const FOCUSABLE_INSIDE_ITEM: &str = "focusable-inside-item";
    /// A `modal` surface with no `label`. A dialog is not named by the text
    /// inside it (it is not one of ARIA's name-from-content roles), so a
    /// screen reader announces it as an unnamed dialog — the same silent
    /// defect `control-without-name` catches, on the node that just took
    /// the user's focus. Only the derived dialog role is checked: a modal
    /// that says what it is with an explicit `role` says it with a `label`
    /// too, or means something naming works differently for.
    pub const MODAL_WITHOUT_NAME: &str = "modal-without-name";
    /// A node declares `live` but carries no `label` and holds no text, so
    /// nothing it ever does can be announced: every platform derives the
    /// spoken string from a name, and there is none to derive. The same
    /// silent defect `image-without-label` catches, on the node that was
    /// meant to speak (see
    /// `docs/adr/0008-live-regions-and-announcements.md`).
    pub const LIVE_REGION_WITHOUT_NAME: &str = "live-region-without-name";
    /// The same announcement text was queued on two consecutive frames.
    /// That is what an unguarded `announce` in a frame builder looks like
    /// — a view runs every frame, so the message is said every frame —
    /// and it is never what an app means: a message genuinely repeated is
    /// repeated across frames the user did something in between. The
    /// announcement still goes through; this names the builder that is
    /// shouting.
    pub const ANNOUNCEMENT_REPEATED: &str = "announcement-repeated";
    /// `wrapChildren` on a container that cannot break lines: a column, or a
    /// row whose main axis scrolls. Both lay out exactly as if the flag were
    /// absent, which reads as "wrapping is broken"; see `LayoutSpec::wrap` for
    /// why a column cannot have it.
    pub const WRAP_IGNORED: &str = "wrap-ignored";
    /// More nodes are departing at once than the exit store will hold (512,
    /// `depart::MAX_NODES`), so the subtrees past the budget vanished
    /// instead of animating out. Correct — that is what a node with no `exit`
    /// does — and invisible from the outside, which is the whole reason it is a
    /// line here: a list that drops a thousand rows wants `exit` on the list,
    /// not on every row.
    pub const EXIT_BUDGET: &str = "exit-budget";
    /// A prop name nothing claims: not a schema row, not a composite, not one of
    /// the element's own props (see `schema::known_prop`). The binding threw the
    /// declaration away — `hoverBg` in a Lua table, `onclick` in JSX — so unlike
    /// every other code here this one is raised by the frontend that saw it,
    /// through `Core::warn`: by the time a frame is a tree the name is
    /// gone. The message names the likely spelling.
    pub const UNKNOWN_PROP: &str = "unknown-prop";
    /// One name declared with two different window configs on the frame it
    /// opened. The config is read on the opening edge only, and on that edge
    /// the lowest declaring window wins (the first declaration within one
    /// frame), so the pick is deterministic — but two places in the app
    /// disagree about what `"palette"` is, and only one of them is right. See
    /// `docs/adr/0004-multi-window.md`, decision 4.
    pub const DUPLICATE_WINDOW_CONFIG: &str = "duplicate-window-config";
    /// A window the user closed is still declared, so it stays closed: a
    /// declaration reopens a window only when it *starts*, and this one never
    /// stopped. The first version of every multi-window app does this — it
    /// declares the window unconditionally — and from outside it looks like
    /// `windows` being ignored. Handle the `{kind:"window", phase:"closed"}`
    /// event, stop declaring the name, and declare it again to reopen. See
    /// `docs/adr/0004-multi-window.md`, decision 6.
    pub const WINDOW_DECLARED_WHILE_CLOSED: &str = "window-declared-while-closed";
    /// A window declared with a `KUI_WINDOW_KIND_*` this build does not
    /// have — `KUI_WINDOW_KIND_NORMAL` and `KUI_WINDOW_KIND_POPUP` are the
    /// two there are. Only a C host can reach this: JSX and Lua name a kind
    /// by string, so an unknown one is refused where it is written rather
    /// than reported a frame later. The window still opens, as a normal
    /// one, so a host built against a later header degrades to a window
    /// rather than to nothing; this line is what keeps that from being
    /// silent. See `docs/adr/0004-multi-window.md`, decision 9.
    pub const UNKNOWN_WINDOW_KIND: &str = "unknown-window-kind";

    /// A `FontId` / `ImageId` / `SoundId` registered in one `Session` and used
    /// through a core of another. Handles are unique to the process, so it
    /// cannot resolve to somebody else's resource; it behaves as a removed
    /// handle does (draws nothing, shapes as sans-serif, plays nothing), which
    /// from outside looks like the resource never registered. Two
    /// `Core::new()`s are two sessions; windows that share resources are built
    /// with `Core::new_in` against one `Session`.
    pub const FOREIGN_RESOURCE: &str = "foreign-resource";
}

/// The [`FOREIGN_RESOURCE`] warning for one handle. Keyed by kind and
/// handle: there is no node — an image node, a text style and a `play`
/// call can all carry the same handle — and one line per handle is the
/// useful count however many frames repeat it.
pub fn foreign_resource(f: &Foreign) -> Warning {
    Warning {
        code: FOREIGN_RESOURCE,
        key: Key::ROOT
            .str(FOREIGN_RESOURCE)
            .str(f.kind.name())
            .index(f.raw),
        message: f.message(),
    }
}

/// The [`ANNOUNCEMENT_REPEATED`] warning for one text. Keyed by the text:
/// there is no node behind an announcement, and one line per repeated
/// message is the useful count however many frames repeat it.
pub fn announcement_repeated(text: &str) -> Warning {
    Warning {
        code: ANNOUNCEMENT_REPEATED,
        key: Key::ROOT.str(ANNOUNCEMENT_REPEATED).str(text),
        message: format!(
            "the announcement {text:?} was queued on two consecutive frames: `announce` says \
             something once, and a view runs every frame, so a call made from a frame builder \
             needs a guard the app clears (announce from the event handler, or keep a field the \
             handler sets and the view clears)"
        ),
    }
}

/// The [`DUPLICATE_WINDOW_CONFIG`] warning for one window name. Keyed by
/// the name: there is no node, and the conflict is between declarations,
/// however many frames repeat it.
pub fn duplicate_window_config(name: &str) -> Warning {
    Warning {
        code: DUPLICATE_WINDOW_CONFIG,
        key: Key::ROOT.str(DUPLICATE_WINDOW_CONFIG).str(name),
        message: format!(
            "window `{name}` was declared with two different configs on the frame it opened; \
             the lowest declaring window's first declaration won, and a live window's config is \
             never re-read, so the other one never applies — make them agree"
        ),
    }
}

/// The [`WINDOW_DECLARED_WHILE_CLOSED`] warning for one window name. Keyed
/// by the name, and raised on the core whose frame declared it, so an app
/// that keeps asking every frame reads one line.
pub fn window_declared_while_closed(name: &str) -> Warning {
    Warning {
        code: WINDOW_DECLARED_WHILE_CLOSED,
        key: Key::ROOT.str(WINDOW_DECLARED_WHILE_CLOSED).str(name),
        message: format!(
            "window `{name}` is still declared after the user closed it, so it stays closed: a \
             declaration reopens a window only when it starts — handle the \
             `{{kind:\"window\", phase:\"closed\"}}` event, stop declaring `{name}`, and declare \
             it again to reopen"
        ),
    }
}

/// The [`UNKNOWN_WINDOW_KIND`] warning for one declaration. Keyed by the
/// window name and the kind, the way the other two window codes are keyed by
/// a name and not a node: a declaration is not a node, and one line per
/// (window, kind) is the useful count however many frames repeat it.
pub fn unknown_window_kind(name: &str, kind: u32) -> Warning {
    Warning {
        code: UNKNOWN_WINDOW_KIND,
        key: Key::ROOT
            .str(UNKNOWN_WINDOW_KIND)
            .str(name)
            .index(kind as u64),
        message: format!(
            "window `{name}` was declared with kind {kind}, which this build does not have; \
             `KUI_WINDOW_KIND_NORMAL` (0) and `KUI_WINDOW_KIND_POPUP` (1) are the ones there \
             are, so it opened as a normal window"
        ),
    }
}

/// The [`UNKNOWN_PROP`] warning for one dropped name, with the nearest
/// legitimate spelling when there is an obvious one. The key is derived from
/// the element and the name rather than from a node, so a misspelling costs
/// one line however many nodes carry it and however many frames draw them.
pub fn unknown_prop(element: &str, name: &str, spelling: schema::Spelling) -> Warning {
    let hint = match schema::suggest(element, name, spelling) {
        Some(near) => format!(" (did you mean `{near}`?)"),
        None => String::new(),
    };
    Warning {
        code: UNKNOWN_PROP,
        key: Key::ROOT.str(UNKNOWN_PROP).str(element).str(name),
        message: format!(
            "`{name}` is not a prop of {element}: no binding reads it, so this declaration is \
             dropped{hint}"
        ),
    }
}

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

    /// A warning built elsewhere — by a binding, for what it saw before the
    /// tree existed. Same gate and same once-per-(code, key) dedup as the
    /// checks below, so a frontend can raise one per node per frame and the
    /// host still reads one line.
    pub(crate) fn raise(&mut self, w: Warning) {
        if !self.enabled
            || self.pending.len() >= MAX_PENDING
            || !self.warned.insert((w.code, w.key))
        {
            return;
        }
        self.pending.push(w);
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
        self.check_wrap(tree);
        self.check_auto_keyed_transitions(tree);
        self.check_duplicate_keys(tree);
        self.check_modal(tree);
        self.check_composites(tree);
        self.check_access(tree, text, edit);
        self.check_live_regions(tree, text);
    }

    /// A modal painted under content it makes inert. Paint order is
    /// preorder with floating subtrees last, so anything after the modal's
    /// subtree — or any float outside it — draws over it; a modal that is
    /// itself inside a float is already on top of both.
    fn check_modal(&mut self, tree: &Tree) {
        let Some(i) = (0..tree.len())
            .rev()
            .find(|&i| tree.specs[i].events().modal.is_some())
        else {
            return;
        };
        let end = tree.subtree_end(i);
        let floating = |mut j: usize| {
            loop {
                if tree.specs[j].layout.float.is_some() {
                    return true;
                }
                match tree.parent[j] {
                    NIL => return false,
                    p => j = p as usize,
                }
            }
        };
        if floating(i) {
            return;
        }
        let over = (0..tree.len())
            .filter(|j| !(i..end).contains(j))
            .any(|j| j >= end || floating(j));
        if !over {
            return;
        }
        self.warn(MODAL_BEHIND_CONTENT, tree.keys[i], || {
            "this modal is not in a float, and content declared after it paints on top of it: \
             everything drawn over a modal is inert, which reads as a broken dialog (float it \
             with `float=\"viewport\"`)"
                .to_string()
        });
    }

    /// Focusable nodes buried inside a composite's items, which nothing
    /// can reach (see [`FOCUSABLE_INSIDE_ITEM`]). One walk per composite,
    /// on the same cadence as every other check here.
    fn check_composites(&mut self, tree: &Tree) {
        let mut items: Vec<usize> = Vec::new();
        for c in 0..tree.len() {
            let Some(item) = tree.specs[c]
                .access()
                .role
                .and_then(crate::composite::item_role)
            else {
                continue;
            };
            crate::composite::items(tree, c, item, &mut items);
            if !crate::composite::is_composite(tree, c, &items) {
                continue;
            }
            for &i in &items {
                let end = tree.subtree_end(i);
                for j in i + 1..end {
                    if !access::focusable(tree, j) {
                        continue;
                    }
                    let what = item.name();
                    self.warn(FOCUSABLE_INSIDE_ITEM, tree.keys[j], || {
                        format!(
                            "this node is focusable but sits inside a `{what}`, which is one \
                             roving Tab stop of a composite: the ring stops at the item, so \
                             nothing reaches this node (move it outside the item, or drop its \
                             focusable behaviour)"
                        )
                    });
                }
            }
        }
    }

    /// Images without a label, controls without a computable name and
    /// modals without one, by the same derivation the access tree uses
    /// (see `access::semantic`).
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
            } else if sem.role == Role::Dialog && tree.specs[i].events().modal.is_some() {
                self.warn(MODAL_WITHOUT_NAME, key, || {
                    "this modal has no accessible name: a dialog is named by its `label`, never \
                     by the text inside it — a screen reader announces an unnamed dialog to the \
                     user it has just moved focus to (give it a `label`)"
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

    /// Live regions that can never say anything: `live` declared with no
    /// `label` and no text inside. Its own walk rather than a branch in
    /// [`Self::check_access`], because that one skips the subtree of a
    /// presentational role and a live region can sit inside one.
    fn check_live_regions(&mut self, tree: &Tree, text: &TextSystem) {
        for i in 0..tree.len() {
            let spec = &tree.specs[i];
            if spec.access().live == access::Live::Off
                || spec.access().role == Some(Role::None)
                || access::live_region_speaks(tree, text, i)
            {
                continue;
            }
            self.warn(LIVE_REGION_WITHOUT_NAME, tree.keys[i], || {
                "this node is a live region but has no accessible name: no `label`, and no text \
                 inside it — every platform reads a live region by its name, so nothing this \
                 node ever does can be announced (put `live` on the node that holds the message)"
                    .to_string()
            });
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

    /// `wrapChildren` where nothing can break: see [`WRAP_IGNORED`].
    fn check_wrap(&mut self, tree: &Tree) {
        for i in 0..tree.len() {
            let layout = tree.specs[i].layout;
            if !layout.wrap {
                continue;
            }
            let reason = if layout.dir != Dir::Row {
                "a column's main size is not resolved until after the pass that would have to \
                 sum the lines, so only a row wraps (turn the container into a row, or give the \
                 items a fixed size and lay them out yourself)"
            } else if layout.scroll_x {
                "a scrollX row's main axis is unbounded, and an axis with no bound has nothing \
                 to break against (drop scrollX, or drop wrapChildren and let it scroll)"
            } else {
                continue;
            };
            self.warn(WRAP_IGNORED, tree.keys[i], || {
                format!("wrapChildren has no effect here: {reason}")
            });
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The `pub const NAME: &str = "code";` lines of this file, read back
    /// from the source: a code declared outside the `warnings!` block would
    /// compile and be missing from the table, and this is what notices.
    fn codes_in_source() -> Vec<&'static str> {
        include_str!("diag.rs")
            .lines()
            .filter_map(|line| {
                let rest = line.trim_start().strip_prefix("pub const ")?;
                let (_name, rest) = rest.split_once(": &str = \"")?;
                let (code, tail) = rest.split_once('"')?;
                (tail == ";").then_some(code)
            })
            .collect()
    }

    #[test]
    fn every_code_is_in_the_table_and_vice_versa() {
        let in_source = codes_in_source();
        let in_table: Vec<&str> = WARNINGS.iter().map(|w| w.code).collect();
        assert!(
            in_source.len() >= 13,
            "the scan missed the consts: {in_source:?}"
        );
        assert_eq!(in_source, in_table);
        for (i, code) in in_table.iter().enumerate() {
            assert!(!in_table[i + 1..].contains(code), "duplicate code {code}");
            assert!(
                code.bytes().all(|b| b == b'-' || b.is_ascii_lowercase()),
                "{code}: codes are kebab-case"
            );
        }
    }

    #[test]
    fn every_row_has_a_doc() {
        for w in WARNINGS {
            assert!(
                w.doc.split_whitespace().count() > 5,
                "{}: no description",
                w.code
            );
            assert!(
                !w.doc.contains("[`"),
                "{}: rustdoc link syntax leaks into the bindings' docs",
                w.code
            );
        }
    }
}
