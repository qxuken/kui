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
//! Several codes do not come from the tree walk. A prop name no table claims
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
//!
//! Two codes come from further out still, and both are about a sound.
//! [`TRUNCATED_PLAYBACK`] is about a sound that was still playing when the
//! core stopped it, and whether it was is the one thing the core cannot
//! see: it queues the `stop` as data and a device applies it. So the core
//! remembers which stops could have cut a one-shot off, the driver answers
//! `crate::Core::audio_truncated` for the ones its handle found still
//! running, and the warning lands on the node from there.
//! [`PLAYBACK_REFUSED`] is the one the core cannot see at all: only the
//! driver knows the device said no, so it reports the playback back
//! (`Core::audio_refused`) and the core keys the warning on the node that
//! asked for the sound. A headless core never hears either answer and so
//! never raises either — which is right, since nothing played.

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

impl Warning {
    /// `{code, key, message}`, the key spelled by `h`.
    pub fn to_value(&self, h: crate::value::Handles) -> crate::value::Value {
        use crate::value::Value;
        Value::map([
            ("code", Value::str(self.code)),
            ("key", (h.key)(self.key)),
            ("message", Value::Str(self.message.clone())),
        ])
    }
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
    /// A label resolved by name (`focus("beta")` in Node, `env.set_focus("beta")`
    /// in Lua, `kui_key_of` in C) is declared by more than one node in the
    /// frame, under different parents, so they have distinct keys and the name
    /// picked the first in tree order. Labels are unique among siblings, not
    /// across a tree. An extension asking from inside its fill is answered
    /// from the nodes it opened and no one else's, and the host from its own
    /// first — so this is a clash among the asker's own. Give the node meant
    /// a label nothing else declares, or pass the hex key an event carried.
    /// Two nodes with the *same* key are `duplicate-key`.
    pub const AMBIGUOUS_KEY: &str = "ambiguous-key";
    /// A `focusRegion(name)` (`Core::focus_region`, `env.focus_region`,
    /// `kui_focus_region`) named a node the frame after it did not declare
    /// as a `focusRegion` — no node under the label, or a node without the
    /// row — so nothing was entered and focus stayed where it was. The call
    /// is resolved against the frame it lands on, so an `update` that
    /// toggles a dock on and enters it in one go is fine; this is that
    /// call with the view half missing, with a name the view spells
    /// differently, or naming a node that is not a region
    /// (`docs/adr/0022-focus-regions.md`, decision 4).
    pub const FOCUS_REGION_WITHOUT_NODE: &str = "focus-region-without-node";
    /// A `selectable` node inside another `selectable` node. Selection
    /// scopes do not nest: the innermost one owns every run under it, so
    /// the outer scope selects only the text outside the inner one — and
    /// a drag that crosses the boundary stops there, which reads as a
    /// selection that will not extend. Declare the scope once, on the
    /// container whose text should select as one.
    pub const NESTED_SELECTION_SCOPE: &str = "nested-selection-scope";
    /// An image with no `label`: assistive technology has nothing to say
    /// for it. Decorative images take `role="none"`.
    pub const IMAGE_WITHOUT_LABEL: &str = "image-without-label";
    /// A `slider` whose `valueNow` lies outside its own `valueMin` /
    /// `valueMax`, or whose `valueMin` is above its `valueMax`. The row is
    /// advertised verbatim, so a screen reader reads a value the range
    /// says is impossible; the app that clamps in its own `update` keeps the
    /// range in two places with nothing tying them, and this is the tie.
    /// Declare the range the value is really held to, or clamp where the
    /// view declares it.
    pub const SLIDER_VALUE_OUT_OF_RANGE: &str = "slider-value-out-of-range";
    /// `Core::add_fragment` was given WGSL that does not compile, so no
    /// handle was minted and nothing will draw. The message carries naga's
    /// own error with the line numbers moved into the app's source
    /// (`docs/adr/0015-a-fragment-element-and-the-painter-it-is-not.md`,
    /// decision 1). The source is rejected here rather than at the first
    /// frame that shows it, so a headless test sees it too.
    pub const FRAGMENT_REJECTED: &str = "fragment-rejected";
    /// A `fragment` node declared more than sixteen `params`. The shader
    /// takes four `vec4<f32>` and no more, so the extra numbers were
    /// dropped; pass fewer, or pack what the fragment needs into the
    /// sixteen it has.
    pub const FRAGMENT_PARAMS_TRUNCATED: &str = "fragment-params-truncated";
    /// A `polygon` declared more than eight points: the stock fragment
    /// takes eight vertices in the sixteen params it has, so the rest
    /// were dropped. Two polygons, or the path primitive kui does not have
    /// (`docs/adr/0025-the-image-is-the-canvas.md`, decision 6).
    pub const POLYGON_POINTS_TRUNCATED: &str = "polygon-points-truncated";
    /// The frame's modal surface is not in a float, and content painted after
    /// it is drawn on top of it: everything the user can see over the modal is
    /// inert, which looks like inert-behind is broken. A modal that has to
    /// cover the app is a float (`float="viewport"`); see
    /// `docs/adr/0003-modal-surfaces.md`. Also raised for a modal that *is*
    /// a float when another float from outside its scope stacks over it
    /// (`docs/adr/0023-layers-stack-in-the-order-they-open.md`): a HUD
    /// opened after the dialog is the same inert surface over it.
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
    /// A `radio` with no `radioGroup` above it, or a `tab` with no
    /// `tabList` — the stock `<radio>` included. Outside its container an
    /// item is no composite's (`docs/adr/0007-composite-keyboard-patterns.md`):
    /// each one is a Tab stop of its own, the arrows, Home and End do not
    /// move the choice, and a screen reader announces no "2 of 3". Wrap the
    /// set in the container, labelled with what the choice is. A `menuItem`
    /// or `listItem` on its own is not reported: the menus build their own
    /// container, and a row outside a list is only a looser reading.
    pub const ITEM_OUTSIDE_CONTAINER: &str = "item-outside-container";
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
    /// `wrapChildren` on a container that cannot break lines: a column, a
    /// row whose main axis scrolls, or a row of a table, whose children are
    /// the table's columns. All lay out exactly as if the flag were absent,
    /// which reads as "wrapping is broken"; see `LayoutSpec::wrap` for why
    /// a column cannot have it.
    pub const WRAP_IGNORED: &str = "wrap-ignored";
    /// An alignment declared where it means nothing (backlog C13): a spread
    /// (`spaceBetween` / `spaceAround` / `spaceEvenly`) on `crossAlign`,
    /// `baseline` on `mainAlign` or on a column's `crossAlign`, or either
    /// as a float's attach point. Each lays out as `start` — the two
    /// centring spreads as `center` — which reads as "the value is
    /// broken" when it is the axis that is wrong.
    pub const ALIGN_IGNORED: &str = "align-ignored";
    /// An `aspectRatio` with nothing it can set (backlog C14): both axes are
    /// declared, or the width is `fit` under a `grow` or percent height,
    /// which is resolved only after every width is. The ratio sizes a fit
    /// height from the width, or a fit width from a fixed height.
    pub const ASPECT_IGNORED: &str = "aspect-ignored";
    /// A text node sits more than four levels below the `line` row above
    /// it, which is as far as a text's place remembers its ancestors — so
    /// `textHit` / `caretRect` asked by that row's key cannot find the run,
    /// and a press inside it reports `byte: 0`. Flatten the wrappers between
    /// the row and its text, or ask by a nearer key.
    pub const TEXT_BEYOND_LINE: &str = "text-beyond-line";
    /// One frame removed more nodes declaring `exit` than the exit store
    /// will hold (512, `depart::MAX_NODES`), so none of that frame's removal
    /// animated: every departing node of it vanished at once, as a node with
    /// no `exit` does, rather than some sliding out and the rest blinking
    /// (`docs/adr/0012-the-exit-budget.md`, decision 2). Correct, and
    /// invisible from the outside, which is the whole reason it is a line
    /// here: a list that drops a thousand rows wants `exit` on the list, not
    /// on every row. A removal that fits the budget but finds earlier exits
    /// still in flight evicts those, oldest first, and is not this warning.
    pub const EXIT_BUDGET: &str = "exit-budget";
    /// A prop name nothing claims: not a schema row, not a composite, not one of
    /// the element's own props (see `schema::known_prop`). The binding threw the
    /// declaration away — `hoverBg` in a Lua table, `onclick` in JSX — so unlike
    /// every other code here this one is raised by the frontend that saw it,
    /// through `Core::warn`: by the time a frame is a tree the name is
    /// gone. The message names the likely spelling. Also raised for a key
    /// a menu row map carried that no row reads — `disabled` on a select's
    /// option, where the key is `enabled` — by the binding that read the
    /// row (backlog RG10).
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

    /// A `setEditText` (`Core::set_edit_text`, `kui_edit_set_text`) named a
    /// key or a label, the text was held for the frame that would declare
    /// it, and the frame after the call declared no editor under that name
    /// — so nothing was ever seeded and the text is dropped. The call is
    /// meant to run from an `update` that also opens the editor, one frame
    /// ahead of the view that declares it; this is the same call with the
    /// view half missing, or with a name the view spells differently. Pass
    /// the label the editor's `key` prop declares — the spelling that
    /// needs nothing to exist yet — or the hex key an event carried.
    /// `keyOf`/`kui_key_of` turns a label into that key, but only for an
    /// editor some frame declared (Lua's verbs take the label itself). An editor that already
    /// exists takes the text where the call is made and never reaches
    /// this.
    pub const EDIT_TEXT_WITHOUT_EDITOR: &str = "edit-text-without-editor";

    /// A one-shot `audio` node went away — or changed its `src` — while
    /// the sound it started was still playing, so the user heard it cut
    /// off. Almost always a duration guessed short: the view keeps the
    /// node declared for a constant it picked, and the asset is longer.
    /// Ask for the sound instead of the guess —
    /// `finish` (`AudioSpec::finish`, `finish` in JSX and Lua) releases the
    /// playback on removal so it plays itself out, and a `tag` reports
    /// `{kind:"sound", phase:"ended"}` when it gets there. A view that
    /// means to cut the sound off says so by stopping what it started
    /// (`Core::stop`), which is not reported, and a `looped` playback
    /// never is: it has no end to be short of.
    ///
    /// Only a driver with a real device raises it, because only a device
    /// knows the sound was still running: the core queues the `stop` and
    /// the driver answers `Core::audio_truncated` for the ones its handle
    /// found still playing. A headless `Ctx` therefore never raises it —
    /// nothing plays — and the assertion point there is the other end of
    /// the same fact: `audioCommands()` holding a `stop` for the node,
    /// where the suite expected none.
    pub const TRUNCATED_PLAYBACK: &str = "truncated-playback";

    /// A `FontId` / `ImageId` / `SoundId` registered in one `Session` and used
    /// through a core of another. Handles are unique to the process, so it
    /// cannot resolve to somebody else's resource; it behaves as a removed
    /// handle does (draws nothing, shapes as sans-serif, plays nothing), which
    /// from outside looks like the resource never registered. Two
    /// `Core::new()`s are two sessions; windows that share resources are built
    /// with `Core::new_in` against one `Session`.
    pub const FOREIGN_RESOURCE: &str = "foreign-resource";

    /// An extension names a slot no host declared this frame, so it drew
    /// nothing. A slot is a position the host declares in its own view by
    /// full name, `ui.slot("ns/name")` — the namespace the host gave the
    /// extension, then the name the extension lists; one listing none fills
    /// `"ns/root"` after the host's view. Declare the slot, or drop the name
    /// from the extension's list. See
    /// `docs/adr/0014-slots-an-extension-fills-in-place.md`, decision 5.
    pub const UNKNOWN_SLOT: &str = "unknown-slot";
    /// A colour or length prop named a token — `bg = "$peach"` — that
    /// nothing declared and that is no theme or metrics role, or named one
    /// of the other kind (a length in a colour slot). The slot is left at
    /// the row's default, as if the prop had not been written — no `bg`,
    /// a fit width, the theme's foreground for a text's `color` — never an
    /// explicit transparent or zero, which would hide the node a typo was
    /// on; the same in every binding and in every place a `$name` can go,
    /// a keyframe stop and an entrance included (backlog AR14), and what
    /// `ui.token_color` / `token_length` answer `None` for in Rust. Raised
    /// by the binding that lowered the reference, through
    /// `Core::warn_unknown_token`, once per name, since the name is gone
    /// by the time the frame is a tree. Also
    /// raised at the declaration for a derived token whose source — the
    /// `from`, or the colour a `mix` or `readable` names — is no colour
    /// token declared before it and no theme role: that token is dropped,
    /// the message names both, and the rest of the table lands. See
    /// `docs/adr/0027-tokens-beside-the-theme.md`, decision 4, and
    /// `docs/adr/0028-derived-tokens.md`.
    pub const UNKNOWN_TOKEN: &str = "unknown-token";
    /// A declared token took a theme or metrics role's name (`surface`,
    /// `radius`) and was dropped: the roles are the corpus's contract and
    /// `$surface` always means the theme's, so an app cannot shadow one.
    /// Rename the token. See `docs/adr/0027-tokens-beside-the-theme.md`,
    /// decision 6.
    pub const RESERVED_TOKEN: &str = "reserved-token";
    /// A slot name declared twice in one frame. The second declaration was
    /// ignored: a fill is keyed by the slot's full name, so two fills of one
    /// name would share every key. Two places for one extension are two
    /// names. See ADR 0014, decision 5.
    pub const DUPLICATE_SLOT: &str = "duplicate-slot";
    /// An extension returned from `view` with nodes still open. The core
    /// closed them at the depth the fill began, so the host's tree is what
    /// the host declared; outside the guard, the rest of the host's view
    /// would have landed inside the extension's last open node. The
    /// extension has an `open` without its `close`. See ADR 0014,
    /// decision 5.
    pub const UNBALANCED_EXTENSION: &str = "unbalanced-extension";
    /// An extension's `view` returned an error. The message is drawn in
    /// red where the fill would have been, and reported here once per
    /// extension and slot rather than once per frame.
    pub const EXTENSION_VIEW_ERROR: &str = "extension-view-error";
    /// An extension declared one of its *own* slots while it was drawing,
    /// so filling it would have meant calling it inside itself. The slot
    /// is left empty. An extension may host extensions (`Fill::add`), and
    /// may declare their slots — what it cannot do is be its own guest.
    /// See ADR 0014, decision 5.
    pub const RECURSIVE_SLOT: &str = "recursive-slot";

    /// The device refused a play: its voices are all held, or the sound
    /// did not decode. A released playback (`finish`) holds one of the
    /// device's 128 voices until its file ends, so a view that releases
    /// faster than its sounds finish reaches the limit and the 129th play
    /// is refused. The refusal is not left silent because the playback
    /// never starts and so never ends: a `tag`ged node waiting for
    /// `ended` would wait forever. It gets
    /// `{kind:"sound", phase:"refused"}` instead, and this line says why.
    /// Stop what the view no longer needs rather than releasing it, or
    /// release shorter sounds.
    pub const PLAYBACK_REFUSED: &str = "playback-refused";
    /// A devtools tab name declared twice in one frame (ADR 0032,
    /// decision 1): two `devtools_tab` / `devtools_tab_with` calls, a host
    /// form and an extension form of one name, or an extension declaring
    /// from its fill under a name the host took. The first declaration
    /// stands and the second is ignored; give the second tab its own name.
    pub const DUPLICATE_TAB: &str = "duplicate-tab";
    /// A `devtoolsTab` declaration a binding could not read as either
    /// form (ADR 0032, decision 1): a child that is not a function, both a
    /// `slot` and a child, or a `view` that is not a function in Lua. The
    /// tab was not declared. A tab names a slot for an extension to fill,
    /// or carries a function the binding calls only when the tab is shown.
    pub const BAD_DEVTOOLS_TAB: &str = "bad-devtools-tab";
    /// A select's `current` names no option the field can show: an index
    /// past its options, or a separator's. The field is drawn as if none
    /// were in force — an empty description, no row checked — rather than
    /// blank with a check on a divider; the options are drawn as declared.
    /// A `current` the view computes from a list it also filters is how
    /// this happens; the index is into the options as passed, separators
    /// counted (backlog RG10). Raised once per field.
    pub const SELECT_CURRENT_IGNORED: &str = "select-current-ignored";
}

/// The [`DUPLICATE_TAB`] warning for one name. Keyed by the name, the way
/// `duplicate_slot` is: a tab is not a node.
pub fn duplicate_tab(name: &str) -> Warning {
    Warning {
        code: DUPLICATE_TAB,
        key: Key::ROOT.str(DUPLICATE_TAB).str(name),
        message: format!(
            "devtools tab {name:?} was declared twice in one frame; the second was ignored — \
             give it its own name"
        ),
    }
}

/// The [`BAD_DEVTOOLS_TAB`] warning for one declaration, with what was
/// wrong with it.
pub fn bad_devtools_tab(name: &str, why: &str) -> Warning {
    Warning {
        code: BAD_DEVTOOLS_TAB,
        key: Key::ROOT.str(BAD_DEVTOOLS_TAB).str(name),
        message: format!("devtools tab {name:?} was not declared: {why}"),
    }
}

/// The [`TEXT_BEYOND_LINE`] warning for one text node under `line`
/// (backlog AR30). Keyed by the text's node.
pub fn text_beyond_line(text: Key, line: Key, reach: usize) -> Warning {
    Warning {
        code: TEXT_BEYOND_LINE,
        key: text,
        message: format!(
            "the text ({:016x}) is more than {reach} levels below its `line` row ({:016x}), further than a              place remembers, so a hit asked by the row's key answers byte 0; flatten the wrappers              between them, or ask by a nearer key",
            text.0, line.0
        ),
    }
}

/// The [`UNKNOWN_SLOT`] warning for one extension and slot. Keyed by the
/// full name: there is no node, and one line per slot is the useful
/// count however many frames repeat it.
pub fn unknown_slot(extension: &str, namespace: &str, slot: &str) -> Warning {
    let full = crate::slot::full_name(namespace, slot);
    Warning {
        code: UNKNOWN_SLOT,
        key: Key::ROOT.str(UNKNOWN_SLOT).str(&full),
        message: format!(
            "extension `{extension}` (namespace `{namespace}`) fills slot {slot:?}, which no view \
             declared this frame, so it drew nothing; declare `ui.slot({full:?})` where it \
             should go, or drop the name from the extension's `slots`"
        ),
    }
}

/// The [`DUPLICATE_SLOT`] warning for one name. Keyed by the name: a slot
/// is not a node, and the conflict is between two declarations of it.
pub fn duplicate_slot(slot: &str) -> Warning {
    Warning {
        code: DUPLICATE_SLOT,
        key: Key::ROOT.str(DUPLICATE_SLOT).str(slot),
        message: format!(
            "slot {slot:?} was declared twice in one frame; the second was ignored, since a \
             fill is keyed by the slot's name — two places want two names"
        ),
    }
}

/// The [`UNBALANCED_EXTENSION`] warning for one fill. Keyed by the slot,
/// which names the extension through its namespace, so a plugin that
/// never closes costs one line.
pub fn unbalanced_extension(slot: &str, slot_key: Key, open: usize) -> Warning {
    Warning {
        code: UNBALANCED_EXTENSION,
        key: slot_key.str(UNBALANCED_EXTENSION),
        message: format!(
            "the extension filling slot {slot:?} returned from view with {open} node{} still \
             open; they were closed for it — it has an `open` without its `close`",
            if open == 1 { "" } else { "s" }
        ),
    }
}

/// The [`EXTENSION_VIEW_ERROR`] warning for one extension in one slot.
/// Keyed by both, so an error that repeats every frame costs one line.
pub fn extension_view_error(extension: &str, slot: &str, slot_key: Key, err: &str) -> Warning {
    Warning {
        code: EXTENSION_VIEW_ERROR,
        key: slot_key.str(extension).str(EXTENSION_VIEW_ERROR),
        message: format!("extension `{extension}` failed to build slot {slot:?}: {err}"),
    }
}

/// The [`RECURSIVE_SLOT`] warning for one fill. Keyed by the slot, the
/// same way `unbalanced-extension` is: the conflict is a name, and one
/// line per name is the useful count however many frames repeat it.
pub fn recursive_slot(extension: &str, slot: &str, slot_key: Key) -> Warning {
    Warning {
        code: RECURSIVE_SLOT,
        key: slot_key.str(RECURSIVE_SLOT),
        message: format!(
            "extension `{extension}` declared slot {slot:?} while it was drawing, which is its \
             own; it was left empty, since filling it would mean calling `{extension}` inside \
             itself. An extension may declare the slots of extensions it loaded — not its own"
        ),
    }
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

/// The [`EDIT_TEXT_WITHOUT_EDITOR`] warning for one label. Keyed by
/// `Key::ROOT.str(label)` — not a node's key, since no node took the
/// name, but one key per label all the same, so two unclaimed labels in
/// a frame are two lines under the once-per-(code, key) dedup.
pub fn edit_text_without_editor_label(label: &str) -> Warning {
    Warning {
        code: EDIT_TEXT_WITHOUT_EDITOR,
        key: Key::ROOT.str(label),
        message: format!(
            "`set_edit_text` named label {label:?}, and the frame after it declared no editor \
             under that name, so the text was dropped; the label is the one an editor's `key` \
             prop declares, and the call is held for one frame — for the view that draws the \
             editor the same `update` opened — not longer"
        ),
    }
}

/// The [`PLAYBACK_REFUSED`] warning for one playback. Keyed by the node
/// that asked for the sound — the tagged node, the `audio` element, or
/// the origin's root for an imperative `play` — so a view that keeps
/// asking past the device's limit costs one line rather than one per
/// refusal, which is the whole point of the dedup.
pub fn playback_refused(key: crate::key::Key, playback: crate::audio::PlaybackId) -> Warning {
    Warning {
        code: PLAYBACK_REFUSED,
        key,
        message: format!(
            "the audio device refused playback {} — its voices are all held, or the sound did \
             not decode; a released (`finish`) playback holds one of the device's 128 voices \
             until its file ends, so releasing faster than the sounds finish reaches the limit. \
             The playback never started and will never report `ended`; a tagged one is told so \
             with `phase: \"refused\"`",
            playback.0
        ),
    }
}

/// The [`EDIT_TEXT_WITHOUT_EDITOR`] warning for one key. Keyed by the
/// editor's own key, so a view that never declares it reports once, the
/// way every node-shaped code does.
/// [`MODAL_BEHIND_CONTENT`], the float-stack case: raised from emission,
/// where the stack exists, rather than from the tree walk.
pub(crate) fn modal_under_layer(key: Key) -> Warning {
    Warning {
        code: MODAL_BEHIND_CONTENT,
        key,
        message: "a float from outside this modal's scope opened after it and paints on top \
                  of it: everything drawn over a modal is inert, which reads as a broken \
                  dialog (declare it inside the modal, or close it while the modal is up)"
            .to_string(),
    }
}

pub fn edit_text_without_editor(key: Key) -> Warning {
    Warning {
        code: EDIT_TEXT_WITHOUT_EDITOR,
        key,
        message: format!(
            "`set_edit_text` named key {:#x}, and the frame after it declared no editor under \
             that key, so the text was dropped; the call is held for one frame — for the view \
             that draws the editor the same `update` opened — not longer",
            key.0
        ),
    }
}

/// The [`TRUNCATED_PLAYBACK`] warning for one cut-off playback. Keyed by
/// the `audio` node, so a view that truncates the same chime every time it
/// runs costs one line; `at` is where the playback was, in seconds, which
/// is the half of "how much was lost" a device can actually report.
pub fn truncated_playback(key: Key, why: crate::audio::Why, at: f64) -> Warning {
    Warning {
        code: TRUNCATED_PLAYBACK,
        key,
        message: format!(
            "the `audio` node at key {:#x} was {} {:.2}s into its sound, cutting it off; keep \
             the node declared until its `ended` event, or add `finish` so the playback is \
             released to play itself out",
            key.0,
            why.verb(),
            at
        ),
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

/// The [`FOCUS_REGION_WITHOUT_NODE`] warning for a `focus_region` the frame
/// could not resolve. Keyed by the key or the label's hash, so a call
/// repeated every frame costs one line.
pub(crate) fn focus_region_without_node(target: &crate::runtime::RegionTarget) -> Warning {
    use crate::runtime::RegionTarget;
    let (key, named) = match target {
        RegionTarget::Main => (Key::ROOT, "the main ring".to_string()),
        RegionTarget::Key(k) => (*k, format!("key {:016x}", k.0)),
        RegionTarget::Label(label) => (Key::ROOT.str(label), format!("label {label:?}")),
    };
    Warning {
        code: FOCUS_REGION_WITHOUT_NODE,
        key,
        message: format!(
            "`focus_region` named {named}, and the frame after it declared no `focusRegion` node \
             there, so nothing was entered; the name is the label the region's `key` prop \
             declares, on a node that carries the `focusRegion` row"
        ),
    }
}

/// The [`AMBIGUOUS_KEY`] warning for one label `Core::key_of` found `count`
/// nodes under. Keyed by the node the name resolved to, so a label asked
/// for every frame costs one line.
pub fn ambiguous_key(label: &str, first: Key, count: usize) -> Warning {
    Warning {
        code: AMBIGUOUS_KEY,
        key: first,
        message: format!(
            "{count} nodes are keyed {label:?} under different parents; the first in tree order \
             ({:016x}) was used — give the one meant a label nothing else declares, or pass its \
             hex key",
            first.0
        ),
    }
}

/// The [`SELECT_CURRENT_IGNORED`] warning for one field: `current` was
/// `index` over `count` options and `separator` says whether it landed on
/// one rather than past the end. Keyed by the field, so a view that draws
/// it that way every frame costs one line.
pub fn select_current_ignored(
    key: Key,
    label: &str,
    index: usize,
    count: usize,
    separator: bool,
) -> Warning {
    let why = if separator {
        "which is a separator".to_string()
    } else {
        format!(
            "and the field has {count} option{}",
            if count == 1 { "" } else { "s" }
        )
    };
    Warning {
        code: SELECT_CURRENT_IGNORED,
        key,
        message: format!(
            "`current` of select {label:?} names option {index} counted from 0, {why}, so the \
             field shows no choice and no row is checked — pass an index of an option, or none"
        ),
    }
}

/// The [`UNKNOWN_PROP`] warning for a key a menu row map carried that
/// [`crate::MenuItem::from_value`] does not read — `disabled` for
/// `enabled: false` — so the row was built without it (backlog RG10).
/// Keyed by the name, as [`unknown_prop`]'s are: one line per spelling.
pub fn unknown_menu_item_key(name: &str) -> Warning {
    let keys = crate::MenuItem::KEYS
        .iter()
        .map(|k| format!("`{k}`"))
        .collect::<Vec<_>>()
        .join(", ");
    // The one misspelling with a meaning of its own gets the value it was
    // after; the rest the nearest key by letters, as `schema::suggest` does.
    let squash = |s: &str| s.replace('_', "").to_ascii_lowercase();
    let hint = if name == "disabled" {
        " (did you mean `enabled: false`?)".to_string()
    } else {
        match crate::MenuItem::KEYS
            .iter()
            .find(|k| squash(k) == squash(name))
        {
            Some(near) => format!(" (did you mean `{near}`?)"),
            None => String::new(),
        }
    };
    Warning {
        code: UNKNOWN_PROP,
        key: Key::ROOT
            .str(UNKNOWN_PROP)
            .str(crate::MenuItem::NAME)
            .str(name),
        message: format!(
            "`{name}` is not a key of a menu item: a row takes {keys}, so this declaration is \
             dropped{hint}"
        ),
    }
}

/// The [`UNKNOWN_PROP`] warning for one dropped name, with the nearest
/// legitimate spelling when there is an obvious one. The key is derived from
/// the element and the name rather than from a node, so a misspelling costs
/// one line however many nodes carry it and however many frames draw them.
/// `element` [`crate::MenuItem::NAME`] is a menu row's key rather than a
/// node's prop, and takes [`unknown_menu_item_key`]'s wording.
pub fn unknown_prop(element: &str, name: &str, spelling: schema::Spelling) -> Warning {
    if element == crate::MenuItem::NAME {
        return unknown_menu_item_key(name);
    }
    // A real row on an element that reads only some of them is not a
    // misspelling, and the nearest spelling would be the row itself; the
    // warning says which rows the element does read instead.
    let message = match schema::element_rows(element, spelling) {
        Some(rows) if schema::shared_prop(name, spelling) => {
            let takes = if rows.is_empty() {
                "none of them".to_string()
            } else {
                format!(
                    "only {}",
                    rows.iter()
                        .map(|r| format!("`{r}`"))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            };
            format!(
                "`{name}` is a prop, but not one {element} reads: its look is its own, and it \
                 takes {takes}, so this declaration is dropped — a box with `role` set takes \
                 every row"
            )
        }
        _ => {
            let hint = match schema::suggest(element, name, spelling) {
                Some(near) => format!(" (did you mean `{near}`?)"),
                None => String::new(),
            };
            format!(
                "`{name}` is not a prop of {element}: no binding reads it, so this declaration \
                 is dropped{hint}"
            )
        }
    };
    Warning {
        code: UNKNOWN_PROP,
        key: Key::ROOT.str(UNKNOWN_PROP).str(element).str(name),
        message,
    }
}

/// Pending warnings are capped so a host that never drains them cannot
/// grow the queue without bound.
const MAX_PENDING: usize = 256;
/// The checks run on the first two frames and every this many after.
pub(crate) const CHECK_EVERY: u64 = 16;

pub(crate) struct Diagnostics {
    pub(crate) enabled: bool,
    pending: Vec<Warning>,
    /// Everything that ever reached `pending`, kept after the drain: the
    /// warnings of a core's whole life, for a reader that is not the
    /// driver (a dev overlay showing what the runner printed). Bounded
    /// the way `warned` is, since each (code, key) lands here once.
    log: Vec<Warning>,
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
            log: Vec::new(),
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

    /// Every warning raised so far, drained or not, oldest first.
    pub(crate) fn raised(&self) -> &[Warning] {
        &self.log
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
        self.log.push(w.clone());
        self.pending.push(w);
    }

    fn warn(&mut self, code: &'static str, key: Key, message: impl FnOnce() -> String) {
        if self.pending.len() >= MAX_PENDING || !self.warned.insert((code, key)) {
            return;
        }
        let w = Warning {
            code,
            key,
            message: message(),
        };
        self.log.push(w.clone());
        self.pending.push(w);
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
        self.check_align(tree);
        self.check_auto_keyed_transitions(tree);
        self.check_duplicate_keys(tree);
        self.check_modal(tree);
        self.check_selection_scopes(tree);
        self.check_composites(tree);
        self.check_lone_items(tree);
        self.check_access(tree, text, edit);
        self.check_live_regions(tree, text);
    }

    /// A modal painted under content it makes inert. Paint order is
    /// preorder with floating subtrees last, so anything after the modal's
    /// subtree — or any float outside it — draws over it; a modal that is
    /// itself inside a float is already on top of both.
    /// A selection scope inside another one (ADR 0017). Cheap to skip:
    /// the tree says whether any node declared one at all.
    fn check_selection_scopes(&mut self, tree: &Tree) {
        if !tree.any_selectable {
            return;
        }
        // Parents precede children, so one forward pass carries the
        // nearest enclosing scope down without a stack.
        self.scratch.clear();
        self.scratch.resize(tree.len(), 0);
        for i in 0..tree.len() {
            let inside = match tree.parent[i] {
                NIL => 0,
                p => self.scratch[p as usize],
            };
            let here = tree.specs[i].interact().selectable;
            if here && inside == 1 {
                self.warn(NESTED_SELECTION_SCOPE, tree.keys[i], || {
                    "a `selectable` node inside another one: selection scopes do not nest, so the inner one owns the text under it and the outer selects only what is outside it".to_string()
                });
            }
            self.scratch[i] = u64::from(here || inside == 1);
        }
    }

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

    /// A `radio` or `tab` with no container of its pair above it (see
    /// [`ITEM_OUTSIDE_CONTAINER`]). The pairs are `composite::PAIRS`, less
    /// the menu's and the list's. Parents precede children, so one forward
    /// pass carries down a bit per pair for the containers above each node,
    /// as the selection-scope check carries its one.
    fn check_lone_items(&mut self, tree: &Tree) {
        use crate::composite::PAIRS;
        const CHECKED: [Role; 2] = [Role::Radio, Role::Tab];
        self.scratch.clear();
        self.scratch.resize(tree.len(), 0);
        for i in 0..tree.len() {
            let above = match tree.parent[i] {
                NIL => 0,
                p => self.scratch[p as usize],
            };
            let mut here = above;
            if let Some(role) = tree.specs[i].access().role {
                for (bit, (container, item)) in PAIRS.iter().enumerate() {
                    if role == *container {
                        here |= 1 << bit;
                    } else if role == *item && CHECKED.contains(item) && above & (1 << bit) == 0 {
                        let (item, container) = (item.name(), container.name());
                        self.warn(ITEM_OUTSIDE_CONTAINER, tree.keys[i], || {
                            format!(
                                "this `{item}` has no `{container}` above it, so it is a Tab \
                                 stop of its own: the arrows do not move the choice and a \
                                 screen reader announces no position in the set (wrap the \
                                 set in a `{container}` with a `label`)"
                            )
                        });
                    }
                }
            }
            self.scratch[i] = here;
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
            if sem.role == Role::Slider {
                self.check_slider_range(tree, i);
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

    /// A slider's declared value against its declared range (see
    /// [`SLIDER_VALUE_OUT_OF_RANGE`]). Only the rows it declares are
    /// compared: a slider with no `valueMin` has no floor to fall under.
    fn check_slider_range(&mut self, tree: &Tree, i: usize) {
        let ax = tree.specs[i].access();
        let (now, min, max) = (ax.value_now, ax.value_min, ax.value_max);
        let reason = match (now, min, max) {
            (_, Some(lo), Some(hi)) if lo > hi => {
                format!("valueMin {lo} is above valueMax {hi}, so no value is in range")
            }
            (Some(v), Some(lo), _) if v < lo => {
                format!("valueNow {v} is below valueMin {lo}")
            }
            (Some(v), _, Some(hi)) if v > hi => {
                format!("valueNow {v} is above valueMax {hi}")
            }
            _ => return,
        };
        self.warn(SLIDER_VALUE_OUT_OF_RANGE, tree.keys[i], || {
            format!(
                "this slider's value and its declared range disagree: {reason} — the rows are \
                 read to a screen reader exactly as declared, so a value the app clamps \
                 somewhere else is announced unclamped (declare the range the value is really \
                 held to, or clamp where the view declares it)"
            )
        });
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

    /// Alignments and ratios with nothing to act on: see
    /// [`ALIGN_IGNORED`] and [`ASPECT_IGNORED`].
    fn check_align(&mut self, tree: &Tree) {
        use crate::spec::Align;
        let spread = |a: Align| {
            matches!(
                a,
                Align::SpaceBetween | Align::SpaceAround | Align::SpaceEvenly
            )
        };
        let odd = |a: Align| spread(a) || a == Align::Baseline;
        for i in 0..tree.len() {
            let l = &tree.specs[i].layout;
            let reason = if l.main_align == Align::Baseline {
                Some("mainAlign baseline: a baseline lines children up across a row, not along it (use crossAlign)".to_string())
            } else if spread(l.cross_align) {
                Some(format!(
                    "crossAlign {}: a spread deals free space out between children, and there is one child per line across the axis (use mainAlign)",
                    l.cross_align.name()
                ))
            } else if l.cross_align == Align::Baseline && l.dir == Dir::Column {
                Some("crossAlign baseline on a column: a column's cross axis is horizontal, where a baseline is not a line (lay the text out in a row)".to_string())
            } else {
                l.float.and_then(|f| {
                    let pts = [
                        f.anchor_point.0,
                        f.anchor_point.1,
                        f.self_point.0,
                        f.self_point.1,
                    ];
                    pts.into_iter().find(|&a| odd(a)).map(|a| {
                        format!(
                            "a float attaches at start, center or end, not {} (it lays out as {})",
                            a.name(),
                            if matches!(a, Align::SpaceAround | Align::SpaceEvenly) {
                                "center"
                            } else {
                                "start"
                            }
                        )
                    })
                })
            };
            if let Some(reason) = reason {
                self.warn(ALIGN_IGNORED, tree.keys[i], || {
                    format!("{reason}; it has no effect here")
                });
            }
            if l.aspect > 0.0 && !l.aspect_height() && l.aspect_width().is_none() {
                let why = if l.width == Sizing::Fit {
                    "the width is fit, and a grow or percent height is resolved only after every width is (give the height a fixed size, or let the height be the fit axis)"
                } else {
                    "both axes are declared, so there is no fit axis for the ratio to size (leave one of them fit)"
                };
                self.warn(ASPECT_IGNORED, tree.keys[i], || {
                    format!("aspectRatio {} has no effect: {why}", l.aspect)
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
            } else if layout.float.is_none()
                && tree.parent[i] != NIL
                && tree.specs[tree.parent[i] as usize].layout.is_table()
            {
                "a table's row cannot wrap: its children are the table's columns, one each \
                 (put the wrapping row inside a cell)"
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

    /// The drain hands a warning to the driver once; the log keeps it for
    /// anyone else, and a repeat of the same (code, key) is neither.
    #[test]
    fn the_log_outlives_the_drain() {
        let mut d = Diagnostics::default();
        let w = Warning {
            code: "test-code",
            key: Key::ROOT,
            message: "once".into(),
        };
        d.raise(w.clone());
        d.raise(w.clone());
        assert_eq!(d.take(), vec![w.clone()]);
        assert!(d.take().is_empty(), "drained");
        assert_eq!(d.raised(), &[w]);
    }

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

    /// A menu row's dropped key takes the row's wording, whichever door
    /// raises it: `unknown_prop` under `MENU_ITEM` is `unknown_menu_item_key`
    /// (backlog RG10). One hint for the key with a meaning of its own, the
    /// letters' nearest for the rest, none for anything fuzzier.
    #[test]
    fn a_menu_rows_unknown_key_is_named_as_one() {
        let name = crate::MenuItem::NAME;
        let w = unknown_prop(name, "disabled", schema::Spelling::Camel);
        assert_eq!(w, unknown_menu_item_key("disabled"));
        assert_eq!(w.code, UNKNOWN_PROP);
        assert_eq!(w.key, Key::ROOT.str(UNKNOWN_PROP).str(name).str("disabled"));
        assert!(
            w.message.contains("`disabled` is not a key of a menu item")
                && w.message
                    .contains("`label`, `role`, `enabled`, `checked`, `id`, `accel`")
                && w.message.ends_with("(did you mean `enabled: false`?)"),
            "{}",
            w.message
        );
        assert!(
            unknown_menu_item_key("Label")
                .message
                .ends_with("(did you mean `label`?)")
        );
        assert!(unknown_menu_item_key("lable").message.ends_with("dropped"));
        assert!(
            unknown_prop("box", "disabled", schema::Spelling::Camel)
                .message
                .starts_with("`disabled` is"),
            "an element's is the element's"
        );
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
