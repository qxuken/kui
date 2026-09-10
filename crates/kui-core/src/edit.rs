//! Editable text: retained editor state keyed by widget `Key`, built on
//! cosmic-text's `Editor` so cursor motion, selection, and click-to-position
//! all come from the same shaping truth the rest of the text stack uses.
//! Edits arrive as data (`InputEvent::Text` / `InputEvent::Key`) routed to
//! the focused editor; hosts read text back with `Core::edit_text`.

use std::collections::VecDeque;

use cosmic_text::{
    Action, Attrs, Buffer, Cursor, Edit as _, Editor, FontSystem, Metrics, Motion, Selection,
    Shaping,
};
use rustc_hash::FxHashMap;

use crate::color::Color;
use crate::display::{Clip, ClipId, Quad, QuadKind};
use crate::geom::{Rect, Size, Vec2};
use crate::input::{EditKey, Mods};
use crate::key::Key;
use crate::resources::Resources;
use crate::spec::TextStyle;
use crate::text::TextSystem;
use crate::tree::OriginId;

#[derive(Clone, Debug, Default)]
pub struct EditOptions {
    pub style: TextStyle,
    pub multiline: bool,
    /// Takes keyboard focus on the frame this declaration starts — a new
    /// editor, one back after a gap, one whose flag just turned on — and
    /// only while nothing holds focus: never from a focused control, and
    /// never again after a blur (`docs/adr/0022`, decision 9).
    pub autofocus: bool,
    /// Selection highlight color. `None` is the theme's `selection`,
    /// which is what a field gets unless the caller says otherwise — so a
    /// selection over a label and one over a field are the same tint on
    /// both bases (ADR 0019).
    pub accent: Option<Color>,
}

pub(crate) struct EditState {
    editor: Editor<'static>,
    pub(crate) style: TextStyle,
    pub(crate) accent: Color,
    pub(crate) multiline: bool,
    pub(crate) origin: OriginId,
    scale: f32,
    /// Wrap width (physical) the buffer is laid out at; None = unwrapped.
    wrap: Option<f32>,
    /// Bumped on every content change.
    pub version: u64,
    /// Bumped whenever the same text is laid out under new metrics (a
    /// style or a scale change), so neither cache below can answer a
    /// question about this font with a size measured for the last one.
    metrics_rev: u32,
    /// Cached wrapped measurement: (version, metrics, wrap bits, size).
    measured: Option<(u64, u32, u32, Size)>,
    /// Cached unwrapped measurement: (version, metrics, size). What the
    /// fit width reads, and the reason it is cached rather than taken off
    /// the buffer as it stands: the buffer is still carrying whatever
    /// width `wrapped` last set on it (backlog F38).
    natural: Option<(u64, u32, Size)>,
    /// How far a single-line field has scrolled its text left, in physical
    /// px. A field keeps the caret inside its box by moving the text under
    /// it, the way a native field does, rather than by wrapping (backlog
    /// F41); a multiline editor wraps and this stays 0.
    offset_x: f32,
    /// In-progress IME composition: a marked, uncommitted range living
    /// inside the buffer (so the text around it reflows as it grows).
    preedit: Option<Preedit>,
    /// Edit history, oldest first. The widget owns its buffer, so it owns
    /// undo too — hosts with their own text model (on_key sinks) bring
    /// their own history and never touch this.
    undo: VecDeque<EditOp>,
    redo: VecDeque<EditOp>,
    /// What the top undo op can still absorb (typing bursts, delete runs).
    coalesce: Option<Coalesce>,
    /// Whether the last declaration carried `autofocus`: with
    /// `last_declared`, what makes the next one an edge or a repeat.
    autofocus: bool,
    /// The frame this key was last declared in. Only the budget reads it
    /// (see [`MAX_UNDECLARED_EDITS`]); a state declared every frame never
    /// looks at it again.
    last_declared: u64,
}

/// How many *undeclared* editors the store keeps before the longest
/// undeclared one is dropped (backlog F26). A declared editor is never
/// evicted, however many there are: retention across absence is what the
/// `<edit>` row promises, so this is a ceiling, not a prune.
///
/// Why 256: one `EditState` is an `Editor` over a shaped `Buffer`, and a
/// counting allocator measured a fresh one at 3.4 KB empty, 4.9 KB holding
/// `"hello"`, and 22 KB holding a 39-character line (the shaped glyphs are
/// most of it). 256 of the worst of those is ~5.6 MB, and ~1.2 MB at a
/// short field — a bound an app can afford, and one no ordinary view comes
/// near: 256 fields no longer on screen is already an app generating keys.
pub const MAX_UNDECLARED_EDITS: usize = 256;

const UNDO_CAP: usize = 1000;
/// Max bytes one coalesced op absorbs before a new unit starts.
const COALESCE_MAX: usize = 64;

/// One reversible edit: `deleted` was removed at `at` and `inserted` put in
/// its place. Operational, not a snapshot — undo cost tracks the edit size,
/// never the document size.
struct EditOp {
    at: Cursor,
    deleted: String,
    inserted: String,
    /// Caret restore points for undo / redo.
    cursor_before: Cursor,
    cursor_after: Cursor,
}

#[derive(Clone, Copy, PartialEq)]
enum Coalesce {
    /// Plain typing: appends to the top op's `inserted`.
    Insert,
    /// Backspace runs walking left: prepends to `deleted`.
    Backspace,
    /// Forward-delete runs at a fixed spot: appends to `deleted`.
    Delete,
}

/// Where a cursor lands after inserting `s` at `at`.
fn end_cursor(at: Cursor, s: &str) -> Cursor {
    match s.rsplit_once('\n') {
        None => Cursor::new(at.line, at.index + s.len()),
        Some((head, tail)) => Cursor::new(at.line + head.matches('\n').count() + 1, tail.len()),
    }
}

/// Position equality, ignoring affinity (which editor cursors carry but
/// computed ones don't).
fn same_pos(a: Cursor, b: Cursor) -> bool {
    a.line == b.line && a.index == b.index
}

/// IME composition state. The text is *in* the buffer starting at `start`
/// — inserted without touching the undo history, replaced on every update,
/// removed on commit or cancel — so the paragraph wraps and the following
/// text shifts exactly as if it had been typed. The editor's own caret is
/// parked at the IME-reported offset inside it.
struct Preedit {
    start: Cursor,
    text: String,
}

/// Byte offset of `c` in the buffer's full text (lines joined by '\n').
fn abs_offset(b: &Buffer, c: Cursor) -> usize {
    b.lines
        .iter()
        .take(c.line)
        .map(|l| l.text().len() + 1)
        .sum::<usize>()
        + c.index
}

impl EditState {
    /// Both measurement caches, dropped together: the text under them
    /// changed, so neither the wrapped size nor the natural one still
    /// describes it.
    fn invalidate_measurements(&mut self) {
        self.measured = None;
        self.natural = None;
    }

    /// Pushes a fresh op (clearing redo), merging into the top op when the
    /// declared coalesce kind matches and the edits are adjacent.
    fn record(&mut self, op: EditOp, kind: Option<Coalesce>) {
        self.redo.clear();
        if let (Some(k), Some(last)) = (kind, self.undo.back_mut())
            && self.coalesce == Some(k)
            && last.deleted.len() + last.inserted.len() + op.deleted.len() + op.inserted.len()
                <= COALESCE_MAX
        {
            let merged = match k {
                Coalesce::Insert => {
                    same_pos(op.at, end_cursor(last.at, &last.inserted)) && {
                        last.inserted.push_str(&op.inserted);
                        true
                    }
                }
                Coalesce::Backspace => {
                    same_pos(end_cursor(op.at, &op.deleted), last.at) && {
                        last.at = op.at;
                        last.deleted.insert_str(0, &op.deleted);
                        true
                    }
                }
                Coalesce::Delete => {
                    same_pos(op.at, last.at) && {
                        last.deleted.push_str(&op.deleted);
                        true
                    }
                }
            };
            if merged {
                last.cursor_after = op.cursor_after;
                return;
            }
        }
        if self.undo.len() >= UNDO_CAP {
            self.undo.pop_front();
        }
        self.undo.push_back(op);
        self.coalesce = kind;
    }

    /// Caret motion, clicks, blur: the next edit starts a new undo unit.
    fn break_coalesce(&mut self) {
        self.coalesce = None;
    }

    /// Replaces `[at .. at+remove]` with `insert`, no recording — the raw
    /// mechanism undo/redo replay through.
    fn splice(&mut self, at: Cursor, remove: &str, insert: &str) {
        if remove.is_empty() {
            self.editor.set_selection(Selection::None);
            self.editor.set_cursor(at);
        } else {
            self.editor.set_selection(Selection::Normal(at));
            self.editor.set_cursor(end_cursor(at, remove));
            self.editor.delete_selection();
        }
        if !insert.is_empty() {
            self.editor.insert_string(insert, None);
        }
    }

    /// Replaces the selection (if any) with `text`, as one recorded op.
    fn insert_recorded(&mut self, text: &str) {
        let cursor_before = self.editor.cursor();
        let deleted = self.editor.copy_selection().unwrap_or_default();
        self.editor.delete_selection();
        let at = self.editor.cursor();
        self.editor.insert_string(text, None);
        let kind = (deleted.is_empty() && !text.contains('\n')).then_some(Coalesce::Insert);
        self.record(
            EditOp {
                at,
                deleted,
                inserted: text.to_string(),
                cursor_before,
                cursor_after: self.editor.cursor(),
            },
            kind,
        );
    }

    /// Deletes the selection as one recorded op; false when there is none.
    fn delete_selection_recorded(&mut self) -> bool {
        let cursor_before = self.editor.cursor();
        let Some(deleted) = self.editor.copy_selection() else {
            return false;
        };
        if !self.editor.delete_selection() {
            return false;
        }
        let at = self.editor.cursor();
        self.record(
            EditOp {
                at,
                deleted,
                inserted: String::new(),
                cursor_before,
                cursor_after: at,
            },
            None,
        );
        true
    }

    /// Backspace/Delete (plain or word): selects via `motion`, deletes as a
    /// recorded op. False at the buffer boundary (nothing to delete).
    fn delete_motion_recorded(
        &mut self,
        motion: Motion,
        kind: Coalesce,
        fs: &mut FontSystem,
    ) -> bool {
        let cursor_before = self.editor.cursor();
        self.editor.set_selection(Selection::Normal(cursor_before));
        self.editor.action(fs, Action::Motion(motion));
        let deleted = self.editor.copy_selection().unwrap_or_default();
        if deleted.is_empty() {
            self.editor.set_selection(Selection::None);
            return false;
        }
        self.editor.delete_selection();
        let at = self.editor.cursor();
        self.record(
            EditOp {
                at,
                deleted,
                inserted: String::new(),
                cursor_before,
                cursor_after: at,
            },
            Some(kind),
        );
        true
    }

    /// Removes an in-progress composition from the buffer (cancel, blur,
    /// or anything that must act on committed text only), leaving the
    /// caret where the composition began. Unrecorded, like its insertion.
    fn abandon_preedit(&mut self) -> bool {
        let Some(pre) = self.preedit.take() else {
            return false;
        };
        self.splice(pre.start, &pre.text, "");
        self.editor.set_cursor(pre.start);
        self.editor.set_selection(Selection::None);
        true
    }

    fn undo_one(&mut self) -> bool {
        let Some(op) = self.undo.pop_back() else {
            return false;
        };
        self.splice(op.at, &op.inserted, &op.deleted);
        self.editor.set_cursor(op.cursor_before);
        self.editor.set_selection(Selection::None);
        self.redo.push_back(op);
        self.coalesce = None;
        true
    }

    fn redo_one(&mut self) -> bool {
        let Some(op) = self.redo.pop_back() else {
            return false;
        };
        self.splice(op.at, &op.deleted, &op.inserted);
        self.editor.set_cursor(op.cursor_after);
        self.editor.set_selection(Selection::None);
        self.undo.push_back(op);
        self.coalesce = None;
        true
    }
}

/// A held `set_edit_text` the frame after it did not claim, in whichever
/// spelling the call used — the two name the same mistake and raise the
/// same code, but a warning that says "key" for a call that said "label"
/// sends its reader looking in the wrong place.
pub(crate) enum Unclaimed {
    Key(Key),
    Label(String),
}

pub struct EditStore {
    states: FxHashMap<Key, EditState>,
    /// Text set for a key nothing has declared yet: `set_text` holds it
    /// here and the next `declare` under that key seeds with it instead of
    /// with `initial`. An `update` that opens an editor and sets its text
    /// in the same turn runs a frame ahead of the view that declares it,
    /// so without this the call lands on nothing and the app sees the
    /// editor open with `initial` (backlog F25). What the frame after it
    /// does not claim is dropped by `finish_frame`, with a warning.
    pending: FxHashMap<Key, String>,
    /// The same seed named by label instead of by key, for the app that
    /// has no key to give: the hex key comes from an event the node
    /// fired, and an editor a rename is opening for the first time has
    /// fired none (backlog F32). Held until `text_edit` declares an
    /// editor under the label and claims it — which is also where a
    /// *retained* editor is reached, since a key kept off screen (F20,
    /// F26) has a state `declare` would not reseed and a label
    /// `Core::key_of` cannot resolve while it goes undeclared.
    pending_labels: FxHashMap<String, String>,
    pub(crate) focused: Option<Key>,
    /// Edit node being drag-selected (with its content origin, logical).
    pub(crate) dragging: Option<(Key, Vec2)>,
    /// Edit whose caret moved since the last frame; `finish_frame` scrolls
    /// the nearest scrollable ancestor to keep the caret visible, then clears.
    pub(crate) caret_moved: Option<Key>,
    /// Bumped on anything that should restart the caret blink cycle (edits,
    /// motion, clicks, focus changes). Frame drivers watch it to re-arm
    /// their blink timer — the core itself stays clock-free.
    caret_stamp: u64,
    /// Whether the caret is currently drawn; toggled by the frame driver's
    /// blink timer. Headless drivers never touch it, so the caret is solid.
    blink_visible: bool,
    /// The frame being built, stamped onto every state `declare` touches.
    frame_no: u64,
}

impl Default for EditStore {
    fn default() -> Self {
        Self {
            states: FxHashMap::default(),
            pending: FxHashMap::default(),
            pending_labels: FxHashMap::default(),
            focused: None,
            dragging: None,
            caret_moved: None,
            caret_stamp: 0,
            blink_visible: true,
            frame_no: 0,
        }
    }
}

/// What an editor's buffer may hold: a field is one line, so a newline that
/// arrived in a seed or a `set_text` is dropped rather than drawn below a
/// box measured for one line (F41). The same rule `apply_text` applies to
/// typing and pasting, so the two doors agree.
fn admitted(text: &str, multiline: bool) -> std::borrow::Cow<'_, str> {
    if multiline || !text.contains(['\n', '\r']) {
        return std::borrow::Cow::Borrowed(text);
    }
    std::borrow::Cow::Owned(text.chars().filter(|c| *c != '\n' && *c != '\r').collect())
}

fn attrs_for<'a>(style: &TextStyle, res: &'a Resources) -> Attrs<'a> {
    Attrs::new()
        .family(res.family_of(style.family))
        .font_features(crate::text::cosmic_features(&style.features))
}

impl EditStore {
    pub fn focused(&self) -> Option<Key> {
        self.focused
    }

    pub fn set_focus(&mut self, key: Option<Key>) {
        if self.focused != key {
            self.caret_stamp += 1;
            // A blurred editor abandons any in-progress composition.
            if let Some(old) = self.focused
                && let Some(s) = self.states.get_mut(&old)
                && s.abandon_preedit()
            {
                s.version += 1;
                s.invalidate_measurements();
            }
        }
        self.focused = key;
    }

    /// See `caret_stamp` field: compare across frames to restart blink.
    pub fn caret_stamp(&self) -> u64 {
        self.caret_stamp
    }

    /// Blink-phase toggle for frame drivers; `true` draws the caret.
    pub fn set_blink_visible(&mut self, visible: bool) {
        self.blink_visible = visible;
    }

    fn touch_caret(&mut self, key: Key) {
        self.caret_moved = Some(key);
        self.caret_stamp += 1;
    }

    /// How many states are retained — declared and undeclared together.
    /// What a test watches the budget through (backlog F26).
    pub fn len(&self) -> usize {
        self.states.len()
    }

    pub fn is_empty(&self) -> bool {
        self.states.is_empty()
    }

    /// Stamps the frame being built and, if the map has grown past the
    /// budget, drops the longest-undeclared states
    /// ([`MAX_UNDECLARED_EDITS`]). A state the frame that just ended
    /// declared is never evicted, and neither is the focused or the
    /// drag-selected one — the store still points at those.
    ///
    /// The length test is what an ordinary frame pays: a store inside the
    /// budget never walks itself, which is why this can run every frame
    /// rather than every 240th like the anim store's cutoff sweep.
    pub(crate) fn begin_frame(&mut self, frame_no: u64) {
        self.frame_no = frame_no;
        if self.states.len() > MAX_UNDECLARED_EDITS {
            self.evict(frame_no.saturating_sub(1));
        }
    }

    /// Drops the oldest undeclared states down to the budget. `declared_at`
    /// is the frame that just ended: a state stamped with it is declared.
    fn evict(&mut self, declared_at: u64) {
        let focused = self.focused;
        let dragging = self.dragging.map(|(k, _)| k);
        let mut undeclared: Vec<(u64, Key)> = self
            .states
            .iter()
            .filter(|(k, s)| {
                s.last_declared < declared_at && Some(**k) != focused && Some(**k) != dragging
            })
            .map(|(k, s)| (s.last_declared, *k))
            .collect();
        let Some(excess) = undeclared.len().checked_sub(MAX_UNDECLARED_EDITS) else {
            return;
        };
        if excess == 0 {
            return;
        }
        // Oldest first, and by key inside a frame so the same view evicts
        // the same states whatever order the map iterated in.
        undeclared.sort_unstable();
        for (_, key) in &undeclared[..excess] {
            self.states.remove(key);
            if self.caret_moved == Some(*key) {
                self.caret_moved = None;
            }
        }
    }

    /// Ensures state exists for `key`, seeding `initial` on first creation
    /// — or, if a `set_text` for this key arrived before anything declared
    /// it, that text instead (see [`EditStore::pending`]). Returns whether
    /// this declaration is an *autofocus edge*: the key was not declared
    /// with `autofocus` on the frame before this one — a new editor, one
    /// back after a gap, or one whose `autofocus` just turned on — which
    /// is the one frame the flag may act on (`docs/adr/0022`, decision 9).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn declare(
        &mut self,
        key: Key,
        initial: &str,
        opts: &EditOptions,
        origin: OriginId,
        scale: f32,
        fs: &mut FontSystem,
        res: &Resources,
    ) -> bool {
        let frame_no = self.frame_no;
        let edge = self
            .states
            .get(&key)
            .is_none_or(|s| s.last_declared + 1 != frame_no || !s.autofocus);
        // Only a creation consumes the seed: a key already declared has
        // no pending text (a `set_text` with state behind it is applied
        // where it is called), and taking one here would drop it.
        let seed = if self.states.contains_key(&key) {
            None
        } else {
            self.pending.remove(&key)
        };
        let seeded = seed.is_some();
        let initial = seed.as_deref().unwrap_or(initial);
        let state = self.states.entry(key).or_insert_with(|| {
            let metrics = Metrics::new(opts.style.size * scale, opts.style.line_height * scale);
            let mut buffer = Buffer::new(fs, metrics);
            buffer.set_size(None, None);
            buffer.set_text(
                &admitted(initial, opts.multiline),
                &attrs_for(&opts.style, res),
                Shaping::Advanced,
                None,
            );
            let mut editor = Editor::new(buffer);
            // A single-line field opens with the caret after its seeded
            // text — what a native field does with a prefilled value, and
            // what a rename wants, since typing into a name meant to be
            // extended otherwise prepends to it (backlog F20). A multiline
            // editor is a document and opens at its top, as native text
            // views do. Placed, not moved: no `touch_caret`, so nothing
            // scrolls to reveal it before the user has touched it.
            //
            // A held `set_text` is not `initial`: it is that call arriving
            // where it can land, so it leaves the caret where the call
            // does — at the end, document or not.
            if seeded || !opts.multiline {
                let end = editor.with_buffer(|b| {
                    let line = b.lines.len().saturating_sub(1);
                    Cursor::new(line, b.lines.get(line).map_or(0, |l| l.text().len()))
                });
                editor.set_cursor(end);
            }
            EditState {
                editor,
                style: opts.style,
                accent: opts.accent.unwrap_or(crate::select::TINT),
                multiline: opts.multiline,
                origin,
                scale,
                wrap: None,
                version: 0,
                metrics_rev: 0,
                measured: None,
                natural: None,
                offset_x: 0.0,
                preedit: None,
                undo: VecDeque::new(),
                redo: VecDeque::new(),
                coalesce: None,
                autofocus: false,
                last_declared: frame_no,
            }
        });
        state.last_declared = frame_no;
        state.autofocus = opts.autofocus;
        state.origin = origin;
        state.multiline = opts.multiline;
        state.accent = opts.accent.unwrap_or(crate::select::TINT);
        // Style/scale changes re-metric the buffer (text and cursor survive).
        // The text is the same, so `version` does not move — but every
        // measurement of it is now of the wrong font, which is what
        // `metrics_rev` is for: without it a cache keyed on the text alone
        // answers the new frame with the old size.
        if state.scale != scale || state.style != opts.style {
            state.style = opts.style;
            state.scale = scale;
            let metrics = Metrics::new(opts.style.size * scale, opts.style.line_height * scale);
            state.editor.with_buffer_mut(|b| b.set_metrics(metrics));
            state.wrap = None;
            state.metrics_rev = state.metrics_rev.wrapping_add(1);
            state.invalidate_measurements();
        }
        // `autofocus` is the core's decision (it owns the one focus).
        edge
    }

    pub fn contains(&self, key: Key) -> bool {
        self.states.contains_key(&key)
    }

    pub fn origin_of(&self, key: Key) -> Option<OriginId> {
        self.states.get(&key).map(|s| s.origin)
    }

    /// The committed text: an in-progress composition is not part of it.
    pub fn text(&self, key: Key) -> Option<String> {
        let s = self.states.get(&key)?;
        Some(s.editor.with_buffer(|b| {
            let mut text = buffer_text(b);
            if let Some(pre) = &s.preedit {
                let at = abs_offset(b, pre.start);
                if text.is_char_boundary(at) && at + pre.text.len() <= text.len() {
                    text.replace_range(at..at + pre.text.len(), "");
                }
            }
            text
        }))
    }

    pub fn set_text(&mut self, key: Key, text: &str, fs: &mut FontSystem, res: &Resources) {
        let Some(s) = self.states.get_mut(&key) else {
            // Nothing has declared this key yet. The call is not wrong —
            // the `update` that opens an editor runs before the view that
            // declares it — so hold the text for the frame that does
            // (backlog F25) rather than falling through silently.
            self.pending.insert(key, text.to_string());
            return;
        };
        s.preedit = None;
        let a = attrs_for(&s.style, res);
        let text = admitted(text, s.multiline);
        s.editor
            .with_buffer_mut(|b| b.set_text(&text, &a, Shaping::Advanced, None));
        s.editor.set_selection(Selection::None);
        s.editor.action(fs, Action::Motion(Motion::BufferEnd));
        s.version += 1;
        s.invalidate_measurements();
        s.wrap = None;
        // A wholesale replacement invalidates the recorded deltas.
        s.undo.clear();
        s.redo.clear();
        s.coalesce = None;
        self.touch_caret(key);
    }

    /// Holds `text` for the next editor declared under `label`, for a
    /// `set_edit_text` by a name nothing has declared yet (backlog F32).
    /// The key path is [`EditStore::pending`]; this one is claimed by
    /// [`EditStore::claim_label`] from inside the build, where the label
    /// and the key it resolves to are both in hand.
    pub(crate) fn hold_label(&mut self, label: &str, text: &str) {
        self.pending_labels
            .insert(label.to_string(), text.to_string());
    }

    /// A declaring editor takes the seed held for its label, if there is
    /// one. Two shapes, and the second is the one a seed by key cannot
    /// have: a *new* editor takes it as `declare` takes a pending key,
    /// over `initial`; an editor that already has a state — retained
    /// while its key was off screen — takes it as `set_text`, since
    /// `declare` reseeds nothing that exists and the draft would
    /// otherwise come back over the model's text.
    pub(crate) fn claim_label(
        &mut self,
        key: Key,
        label: &str,
        fs: &mut FontSystem,
        res: &Resources,
    ) {
        if self.pending_labels.is_empty() {
            return;
        }
        let Some(text) = self.pending_labels.remove(label) else {
            return;
        };
        if self.states.contains_key(&key) {
            self.set_text(key, &text, fs, res);
        } else {
            self.pending.insert(key, text);
        }
    }

    /// The seeds no frame claimed, dropped: `finish_frame` drains this
    /// after the build and raises [`crate::diag::EDIT_TEXT_WITHOUT_EDITOR`]
    /// for each, so a `set_edit_text` on a key — or a label — the view
    /// never declares is a line rather than nothing at all. Sorted within
    /// each spelling, so the order two unclaimed seeds are reported in
    /// does not depend on a hash seed.
    pub(crate) fn take_unclaimed_seeds(&mut self) -> Vec<Unclaimed> {
        let mut out: Vec<Unclaimed> = Vec::new();
        if !self.pending.is_empty() {
            let mut keys: Vec<Key> = self.pending.drain().map(|(k, _)| k).collect();
            keys.sort_unstable();
            out.extend(keys.into_iter().map(Unclaimed::Key));
        }
        if !self.pending_labels.is_empty() {
            let mut labels: Vec<String> = self.pending_labels.drain().map(|(l, _)| l).collect();
            labels.sort_unstable();
            out.extend(labels.into_iter().map(Unclaimed::Label));
        }
        out
    }

    pub fn version(&self, key: Key) -> u64 {
        self.states.get(&key).map_or(0, |s| s.version)
    }

    /// The caret as a byte offset into the committed text, and the
    /// non-empty selection as a byte range, for the access tree.
    pub fn caret_and_selection(&self, key: Key) -> Option<(usize, Option<(usize, usize)>)> {
        let s = self.states.get(&key)?;
        Some(s.editor.with_buffer(|b| {
            let caret = abs_offset(b, s.editor.cursor());
            let selection = s
                .editor
                .selection_bounds()
                .map(|(a, z)| (abs_offset(b, a), abs_offset(b, z)))
                .filter(|(a, z)| a != z);
            (caret, selection)
        }))
    }

    /// The selection's anchor and the caret as (line, byte index) pairs
    /// (equal without a selection), for the access tree.
    pub fn selection_cursors(&self, key: Key) -> Option<((usize, usize), (usize, usize))> {
        let s = self.states.get(&key)?;
        let caret = s.editor.cursor();
        let anchor = match s.editor.selection() {
            Selection::Normal(c) | Selection::Line(c) | Selection::Word(c) => c,
            Selection::None => caret,
        };
        Some(((anchor.line, anchor.index), (caret.line, caret.index)))
    }

    /// The editor's laid-out lines as access runs (see
    /// [`crate::access::AccessRun`]); `origin` is where the content box
    /// starts, logical px.
    pub(crate) fn runs(
        &self,
        key: Key,
        node: Key,
        origin: Vec2,
        scale: f32,
    ) -> Vec<crate::access::AccessRun> {
        let Some(s) = self.states.get(&key) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let mut n = 0;
        // Where the glyphs are, not where they would be unscrolled: a
        // screen reader's character rects have to land on the pixels
        // emission drew (a scrolled field, F41).
        let origin = Vec2::new(origin.x - s.offset_x, origin.y);
        s.editor.with_buffer(|b| {
            crate::access::runs_of_buffer(
                b,
                crate::access::RunSource {
                    key: node,
                    line: 0,
                    byte_base: 0,
                    origin,
                    scale,
                    newline_after_last: false,
                },
                &mut n,
                &mut out,
            )
        });
        out
    }

    /// Moves the caret to `focus` and the selection's other end to
    /// `anchor`, both (line, byte index) pairs clamped into the text; equal
    /// pairs clear the selection. What a screen reader's "select from here
    /// to there" becomes.
    pub fn set_selection(&mut self, key: Key, anchor: (usize, usize), focus: (usize, usize)) {
        let Some(s) = self.states.get_mut(&key) else {
            return;
        };
        s.abandon_preedit();
        let clamp = |b: &Buffer, (line, index): (usize, usize)| {
            let line = line.min(b.lines.len().saturating_sub(1));
            let text = b.lines[line].text();
            let mut index = index.min(text.len());
            while !text.is_char_boundary(index) {
                index -= 1;
            }
            Cursor::new(line, index)
        };
        let (a, f) = s
            .editor
            .with_buffer(|b| (clamp(b, anchor), clamp(b, focus)));
        s.editor.set_cursor(f);
        s.editor.set_selection(if a == f {
            Selection::None
        } else {
            Selection::Normal(a)
        });
        s.break_coalesce();
        self.touch_caret(key);
    }

    /// Types `text` over the selection (or at the caret), as one undo
    /// step; true when the content changed.
    pub fn replace_selection(&mut self, key: Key, text: &str, fs: &mut FontSystem) -> bool {
        let Some(s) = self.states.get_mut(&key) else {
            return false;
        };
        s.abandon_preedit();
        if text.is_empty() && s.editor.selection_bounds().is_none() {
            return false;
        }
        s.insert_recorded(text);
        s.editor.shape_as_needed(fs, false);
        s.version += 1;
        s.invalidate_measurements();
        self.touch_caret(key);
        true
    }

    pub fn copy_selection(&self, key: Key) -> Option<String> {
        self.states.get(&key)?.editor.copy_selection()
    }

    /// Deletes the selection; returns true if anything was deleted.
    pub fn delete_selection(&mut self, key: Key, fs: &mut FontSystem) -> bool {
        let Some(s) = self.states.get_mut(&key) else {
            return false;
        };
        let _ = fs;
        s.abandon_preedit();
        if s.delete_selection_recorded() {
            s.version += 1;
            s.invalidate_measurements();
            self.touch_caret(key);
            true
        } else {
            false
        }
    }

    // -- Input application (focused editor). Returns true if content changed.

    pub(crate) fn apply_text(&mut self, key: Key, text: &str, fs: &mut FontSystem) -> bool {
        let Some(s) = self.states.get_mut(&key) else {
            return false;
        };
        // A commit ends the composition (winit also clears preedit first);
        // the committed text goes in where the composition began.
        s.abandon_preedit();
        let filtered: String = text
            .chars()
            .filter(|c| !c.is_control() || (*c == '\n' && s.multiline) || *c == '\t')
            .collect();
        if filtered.is_empty() {
            return false;
        }
        s.insert_recorded(&filtered);
        s.editor.shape_as_needed(fs, false);
        s.version += 1;
        s.invalidate_measurements();
        self.touch_caret(key);
        true
    }

    /// Returns (content_changed, submit) — submit is Enter in single-line.
    pub(crate) fn apply_key(
        &mut self,
        key: Key,
        ek: EditKey,
        mods: Mods,
        fs: &mut FontSystem,
    ) -> (bool, bool) {
        let Some(s) = self.states.get_mut(&key) else {
            return (false, false);
        };
        // Keys reaching the editor mid-composition mean the IME let them
        // through; act on committed text only.
        let mut changed = s.abandon_preedit();
        let mut submit = false;
        match ek {
            EditKey::Left
            | EditKey::Right
            | EditKey::Up
            | EditKey::Down
            | EditKey::Home
            | EditKey::End
            | EditKey::PageUp
            | EditKey::PageDown => {
                let motion = match (ek, mods.word, mods.doc) {
                    (EditKey::Left, true, _) => Motion::LeftWord,
                    (EditKey::Right, true, _) => Motion::RightWord,
                    (EditKey::Left, _, true) => Motion::Home,
                    (EditKey::Right, _, true) => Motion::End,
                    (EditKey::Up, _, true) => Motion::BufferStart,
                    (EditKey::Down, _, true) => Motion::BufferEnd,
                    (EditKey::Left, ..) => Motion::Left,
                    (EditKey::Right, ..) => Motion::Right,
                    (EditKey::Up, ..) => Motion::Up,
                    (EditKey::Down, ..) => Motion::Down,
                    (EditKey::Home, _, true) => Motion::BufferStart,
                    (EditKey::End, _, true) => Motion::BufferEnd,
                    (EditKey::Home, ..) => Motion::Home,
                    (EditKey::End, ..) => Motion::End,
                    (EditKey::PageUp, ..) => Motion::PageUp,
                    (EditKey::PageDown, ..) => Motion::PageDown,
                    _ => unreachable!(),
                };
                if mods.shift {
                    if s.editor.selection() == Selection::None {
                        s.editor.set_selection(Selection::Normal(s.editor.cursor()));
                    }
                } else {
                    s.editor.set_selection(Selection::None);
                }
                s.editor.action(fs, Action::Motion(motion));
                s.break_coalesce();
            }
            EditKey::Backspace => {
                changed |= s.delete_selection_recorded()
                    || s.delete_motion_recorded(
                        if mods.word {
                            Motion::LeftWord
                        } else {
                            Motion::Left
                        },
                        Coalesce::Backspace,
                        fs,
                    );
            }
            EditKey::Delete => {
                changed |= s.delete_selection_recorded()
                    || s.delete_motion_recorded(
                        if mods.word {
                            Motion::RightWord
                        } else {
                            Motion::Right
                        },
                        Coalesce::Delete,
                        fs,
                    );
            }
            EditKey::Enter => {
                if s.multiline {
                    s.insert_recorded("\n");
                    changed = true;
                } else {
                    submit = true;
                }
            }
            EditKey::Tab => {
                if s.multiline {
                    s.insert_recorded("    ");
                    changed = true;
                }
            }
            EditKey::Undo => changed |= s.undo_one(),
            EditKey::Redo => changed |= s.redo_one(),
            EditKey::SelectAll => {
                s.editor.action(fs, Action::Motion(Motion::BufferStart));
                s.editor.set_selection(Selection::Normal(s.editor.cursor()));
                s.editor.action(fs, Action::Motion(Motion::BufferEnd));
                s.break_coalesce();
            }
            EditKey::Escape => {
                s.editor.action(fs, Action::Escape);
                s.break_coalesce();
            }
        }
        s.editor.shape_as_needed(fs, false);
        if changed {
            s.version += 1;
            s.invalidate_measurements();
        }
        self.touch_caret(key);
        (changed, submit)
    }

    /// Mouse press inside the edit at content-local logical position.
    /// `clicks` is the driver-counted multi-click: 2 selects the word,
    /// 3 the line (cosmic-text's double/triple click actions).
    pub(crate) fn click(&mut self, key: Key, local: Vec2, clicks: u8, fs: &mut FontSystem) {
        if let Some(s) = self.states.get_mut(&key) {
            if s.abandon_preedit() {
                s.version += 1;
                s.invalidate_measurements();
            }
            let (x, y) = ((local.x * s.scale) as i32, (local.y * s.scale) as i32);
            let action = match clicks {
                0 | 1 => Action::Click { x, y },
                2 => Action::DoubleClick { x, y },
                _ => Action::TripleClick { x, y },
            };
            s.editor.action(fs, action);
            s.editor.shape_as_needed(fs, false);
            s.break_coalesce();
            self.caret_stamp += 1;
        }
    }

    pub(crate) fn drag(&mut self, key: Key, local: Vec2, fs: &mut FontSystem) {
        if let Some(s) = self.states.get_mut(&key) {
            let (x, y) = ((local.x * s.scale) as i32, (local.y * s.scale) as i32);
            s.editor.action(fs, Action::Drag { x, y });
            self.caret_stamp += 1;
        }
    }

    /// Replaces the focused editor's IME composition. The text lives in
    /// the buffer from the moment it appears — following text shifts and
    /// the paragraph rewraps — but never in the undo history; the caret
    /// sits at the IME-reported offset inside it. Empty text cancels
    /// (winit sends that before every commit). Returns true if anything
    /// visible changed.
    pub(crate) fn set_preedit(
        &mut self,
        key: Key,
        text: &str,
        cursor: Option<(usize, usize)>,
        fs: &mut FontSystem,
    ) -> bool {
        let Some(s) = self.states.get_mut(&key) else {
            return false;
        };
        if text.is_empty() {
            if !s.abandon_preedit() {
                return false;
            }
        } else {
            let start = match s.preedit.take() {
                Some(pre) => {
                    s.splice(pre.start, &pre.text, text);
                    pre.start
                }
                None => {
                    // Composing over a selection replaces it, as typing
                    // would — that part is a real, recorded edit.
                    s.delete_selection_recorded();
                    let at = s.editor.cursor();
                    s.editor.insert_string(text, None);
                    at
                }
            };
            // Caret at the IME's offset inside the composition (its end
            // when unreported), clamped to a char boundary.
            let mut at = cursor.map_or(text.len(), |(c, _)| c.min(text.len()));
            while at > 0 && !text.is_char_boundary(at) {
                at -= 1;
            }
            s.editor.set_cursor(end_cursor(start, &text[..at]));
            s.editor.set_selection(Selection::None);
            s.preedit = Some(Preedit {
                start,
                text: text.to_string(),
            });
            s.break_coalesce();
        }
        s.editor.shape_as_needed(fs, false);
        s.version += 1;
        s.invalidate_measurements();
        self.touch_caret(key);
        true
    }

    /// Drops the selection of one editor, leaving the caret where the
    /// selection's live end was. What a selection started elsewhere in
    /// the window calls, so no window ever shows two selections
    /// (`docs/adr/0017-selection-as-a-scope.md`).
    pub(crate) fn collapse_selection(&mut self, key: Key) -> bool {
        let Some(s) = self.states.get_mut(&key) else {
            return false;
        };
        if matches!(s.editor.selection(), Selection::None) {
            return false;
        }
        s.editor.set_selection(Selection::None);
        self.caret_stamp += 1;
        true
    }

    /// The active composition text, if any (for tests and hosts).
    pub fn preedit(&self, key: Key) -> Option<&str> {
        self.states
            .get(&key)?
            .preedit
            .as_ref()
            .map(|p| p.text.as_str())
    }

    pub fn is_multiline(&self, key: Key) -> bool {
        self.states.get(&key).is_some_and(|s| s.multiline)
    }

    /// Caret rect in physical px, relative to the edit's content origin.
    /// None when the caret isn't laid out (e.g. no state for `key`).
    pub(crate) fn caret_rect(&mut self, key: Key, fs: &mut FontSystem) -> Option<Rect> {
        let s = self.states.get_mut(&key)?;
        s.editor.shape_as_needed(fs, false);
        let (x, y) = s.editor.cursor_position()?;
        let line_height = s.editor.with_buffer(|b| b.metrics().line_height);
        Some(Rect::new(
            x as f32 - s.offset_x,
            y as f32,
            (2.0 * s.scale).max(2.0),
            line_height,
        ))
    }

    /// How far a single-line field's text is scrolled left, in physical
    /// px, for a content box `inner_w` wide (physical too). 0 for a
    /// multiline editor, which wraps instead.
    ///
    /// A field does not wrap ([`EditStore::wrapped`]), so a value that
    /// outgrows its box is moved under the caret rather than folded onto a
    /// second line — which is what a native field does, and what an app
    /// otherwise has to fake by declaring the box wider than the text it
    /// is about to hold (backlog F41). Recomputed where the box is known,
    /// so a field that grows or shrinks between frames re-anchors with it.
    pub(crate) fn line_offset(&mut self, key: Key, inner_w: f32, fs: &mut FontSystem) -> f32 {
        let focused = self.focused == Some(key);
        let Some(s) = self.states.get_mut(&key) else {
            return 0.0;
        };
        if s.multiline {
            return 0.0;
        }
        // Unfocused, a field shows its value from the start: what it says
        // is what a reader wants, not where its caret was left.
        if !focused {
            s.offset_x = 0.0;
            return 0.0;
        }
        s.editor.shape_as_needed(fs, false);
        let caret_w = (2.0 * s.scale).max(2.0);
        let text_w = s
            .editor
            .with_buffer(|b| b.layout_runs().map(|r| r.line_w).fold(0.0f32, f32::max));
        if let Some((x, _)) = s.editor.cursor_position() {
            let x = x as f32;
            // Two ends, one rule: keep the caret inside the box, moving
            // the text by the least that does it.
            if x - s.offset_x > inner_w - caret_w {
                s.offset_x = x - inner_w + caret_w;
            }
            if x < s.offset_x {
                s.offset_x = x;
            }
        }
        // Never past the end of the text (a field that shrank, or one
        // whose value was replaced by a shorter one, scrolls back).
        s.offset_x = s.offset_x.clamp(0.0, (text_w + caret_w - inner_w).max(0.0));
        s.offset_x
    }

    // -- Layout measurement (logical units)

    /// What the text wants on its own: the width a `Fit` editor takes, and
    /// the floor under a `Min::FIT` one.
    ///
    /// Measured with the wrap taken *off*, and cached against the text and
    /// its metrics rather than read off the buffer as it stands. The
    /// buffer is still carrying whatever width `wrapped` last set on it,
    /// and a fit width measured under that is a width that feeds back on
    /// itself: the box takes the widest wrapped line, the next frame wraps
    /// to that, and a field declared to hug its text ratchets down to one
    /// character with every keystroke on its own line (backlog F38).
    pub(crate) fn intrinsic(&mut self, key: Key, fs: &mut FontSystem) -> Size {
        let Some(s) = self.states.get_mut(&key) else {
            return Size::ZERO;
        };
        if let Some((v, m, size)) = s.natural
            && (v, m) == (s.version, s.metrics_rev)
        {
            return size;
        }
        // Taken off and left off: `wrapped` runs after this in the same
        // pass, and putting the frame's width back is its job. Off
        // unconditionally rather than when `wrap` says it is on — a style
        // change clears that flag without touching the buffer, and
        // measuring the wrapped buffer is the whole bug.
        s.editor.with_buffer_mut(|b| b.set_size(None, None));
        s.wrap = None;
        s.editor.shape_as_needed(fs, false);
        let (w, h) = s.editor.with_buffer(|b| {
            let mut w = 0.0f32;
            let mut lines = 0u32;
            for run in b.layout_runs() {
                w = w.max(run.line_w);
                lines += 1;
            }
            (w, lines.max(1) as f32 * b.metrics().line_height)
        });
        // Caret margin so the cursor at line end isn't clipped.
        let size = Size::new((w + 2.0 * s.scale) / s.scale, h / s.scale);
        s.natural = Some((s.version, s.metrics_rev, size));
        size
    }

    /// The editor's height at its final content width — and, for a
    /// single-line field, the width it is *not* wrapped to.
    ///
    /// `multiline: false` is a field, not a short document: it lays out on
    /// one line whatever it is given and scrolls that line under the caret
    /// (see [`EditStore::line_offset`]), which is what a native field does
    /// and what the `<edit>` row has always said it is. Wrapping one was
    /// how a name that outgrew its box came to be drawn two lines tall
    /// inside a box measured for one (backlog F41).
    pub(crate) fn wrapped(&mut self, key: Key, max_w: f32, fs: &mut FontSystem) -> Size {
        let Some(s) = self.states.get_mut(&key) else {
            return Size::ZERO;
        };
        if !s.multiline {
            // The buffer stays unwrapped (an editor that was multiline
            // last frame may be carrying a width), and the box is one line
            // tall whatever the text has in it — a `\n` that reached a
            // field through `set_text` does not make it two.
            if s.wrap.is_some() {
                s.editor.with_buffer_mut(|b| b.set_size(None, None));
                s.wrap = None;
                s.invalidate_measurements();
            }
            s.editor.shape_as_needed(fs, false);
            let line = s.editor.with_buffer(|b| b.metrics().line_height);
            return Size::new(max_w, line / s.scale);
        }
        let target = (max_w * s.scale).max(1.0);
        let differs = match s.wrap {
            Some(a) => (a - target).abs() > 0.5,
            None => true,
        };
        if differs {
            s.editor.with_buffer_mut(|b| b.set_size(Some(target), None));
            s.wrap = Some(target);
        }
        let stamp = (s.version, s.metrics_rev, target.to_bits());
        if let Some((v, m, w, size)) = s.measured
            && (v, m, w) == stamp
        {
            return size;
        }
        s.editor.shape_as_needed(fs, false);
        let h = s.editor.with_buffer(|b| {
            let mut lines = 0u32;
            for _ in b.layout_runs() {
                lines += 1;
            }
            lines.max(1) as f32 * b.metrics().line_height
        });
        let size = Size::new(max_w, h / s.scale);
        s.measured = Some((stamp.0, stamp.1, stamp.2, size));
        size
    }

    // -- Emission

    /// Draws selection, glyphs, and caret. `origin` is the content box origin
    /// in physical px; everything emitted is clipped by `clip`.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn emit(
        &mut self,
        key: Key,
        origin: Vec2,
        focused: bool,
        clip: Clip,
        clip_id: ClipId,
        fs: &mut FontSystem,
        text_system: &mut TextSystem,
        atlas: &mut crate::atlas::GlyphAtlas,
        out: &mut Vec<Quad>,
    ) {
        let blink_visible = self.blink_visible;
        let Some(s) = self.states.get_mut(&key) else {
            return;
        };
        let raster = text_system.raster_mut();
        s.editor.shape_as_needed(fs, false);
        let color = s.style.color_or_default();
        let accent = s.accent;
        let scale = s.scale;
        let selection = s.editor.selection_bounds();
        // The composition range, marked like a selection but drawn as a
        // tint plus underline; the caret stays solid while composing so
        // the IME's offset inside the text is never hidden by a blink.
        let composing = focused && s.preedit.is_some();
        let preedit = s
            .preedit
            .as_ref()
            .map(|p| (p.start, end_cursor(p.start, &p.text)));
        let cursor_pos = if focused && (blink_visible || composing) {
            s.editor.cursor_position()
        } else {
            None
        };
        // A single-line field scrolls its text under the caret; the box it
        // scrolls inside is the clip emission was handed (F41).
        let origin = Vec2::new(origin.x - s.offset_x, origin.y);

        s.editor.with_buffer(|b| {
            let line_height = b.metrics().line_height;
            // Runs come in line order: skip everything above the clip and
            // stop at the first run past its bottom — a 100k-line document
            // emits only the visible screenful of quads.
            let runs = b
                .layout_runs()
                .filter(|run| origin.y + run.line_top + line_height >= clip.rect.y)
                .take_while(|run| origin.y + run.line_top <= clip.rect.y + clip.rect.h);
            for run in runs {
                // Selection highlight for this run (mixed BiDi runs can
                // yield several disjoint spans). `highlight` is only valid
                // for runs on lines inside the selection span — outside it
                // marks the whole run selected.
                if let Some((start, end)) = selection
                    && run.line_i >= start.line
                    && run.line_i <= end.line
                {
                    let mut any = false;
                    for (x, w) in run.highlight(start, end) {
                        any = true;
                        out.push(Quad {
                            rect: Rect::new(
                                origin.x + x,
                                origin.y + run.line_top,
                                w.max(2.0),
                                line_height,
                            ),
                            color: accent,
                            border_color: Color::TRANSPARENT,
                            radius: [0.0; 4],
                            border_w: 0.0,
                            blur: 0.0,
                            kind: QuadKind::Solid,
                            uv: [0; 4],
                            clip: clip_id,
                        });
                    }
                    // Empty line inside the selection: a stub for the
                    // selected newline keeps the highlight continuous.
                    if !any && run.glyphs.is_empty() && end.line > run.line_i {
                        out.push(Quad {
                            rect: Rect::new(origin.x, origin.y + run.line_top, 2.0, line_height),
                            color: accent,
                            border_color: Color::TRANSPARENT,
                            radius: [0.0; 4],
                            border_w: 0.0,
                            blur: 0.0,
                            kind: QuadKind::Solid,
                            uv: [0; 4],
                            clip: clip_id,
                        });
                    }
                }
                // Composition backdrop + underline under its glyphs.
                if let Some((ps, pe)) = preedit
                    && run.line_i >= ps.line
                    && run.line_i <= pe.line
                {
                    let underline_h = scale.max(1.0);
                    for (x, w) in run.highlight(ps, pe) {
                        let solid = |rect: Rect, color: Color| Quad {
                            rect,
                            color,
                            border_color: Color::TRANSPARENT,
                            radius: [0.0; 4],
                            border_w: 0.0,
                            blur: 0.0,
                            kind: QuadKind::Solid,
                            uv: [0; 4],
                            clip: clip_id,
                        };
                        out.push(solid(
                            Rect::new(origin.x + x, origin.y + run.line_top, w, line_height),
                            Color { a: 0.3, ..accent },
                        ));
                        out.push(solid(
                            Rect::new(
                                origin.x + x,
                                origin.y + run.line_top + line_height - underline_h,
                                w,
                                underline_h,
                            ),
                            color,
                        ));
                    }
                }
                // Glyphs.
                for glyph in run.glyphs.iter() {
                    let physical = glyph.physical((0.0, 0.0), 1.0);
                    let Some(slot) =
                        crate::text::raster_glyph(physical.cache_key, fs, raster, atlas)
                    else {
                        continue;
                    };
                    let x = origin.x + physical.x as f32 + slot.left as f32;
                    let y = origin.y + run.line_y.round() + physical.y as f32 - slot.top as f32;
                    let glyph_color = glyph
                        .color_opt
                        .map(|c| Color::rgba8(c.r(), c.g(), c.b(), c.a()))
                        .unwrap_or(color);
                    out.push(Quad {
                        rect: Rect::new(x, y, slot.w as f32, slot.h as f32),
                        color: glyph_color,
                        border_color: Color::TRANSPARENT,
                        radius: [0.0; 4],
                        border_w: 0.0,
                        blur: 0.0,
                        kind: crate::text::glyph_kind(&slot),
                        uv: [slot.x, slot.y, slot.w, slot.h],
                        clip: clip_id,
                    });
                }
            }
            // Caret.
            if let Some((cx, cy)) = cursor_pos {
                out.push(Quad {
                    rect: Rect::new(
                        origin.x + cx as f32,
                        origin.y + cy as f32,
                        (2.0 * scale).max(2.0),
                        line_height,
                    ),
                    color,
                    border_color: Color::TRANSPARENT,
                    radius: [0.0; 4],
                    border_w: 0.0,
                    blur: 0.0,
                    kind: QuadKind::Solid,
                    uv: [0; 4],
                    clip: clip_id,
                });
            }
        });
    }
}

fn buffer_text(b: &Buffer) -> String {
    let mut out = String::new();
    for (i, line) in b.lines.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(line.text());
    }
    out
}
