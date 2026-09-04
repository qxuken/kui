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
use crate::display::{Quad, QuadKind};
use crate::geom::{Rect, Size, Vec2};
use crate::input::{EditKey, Mods};
use crate::key::Key;
use crate::resources::Resources;
use crate::spec::TextStyle;
use crate::text::TextSystem;
use crate::tree::OriginId;

#[derive(Clone, Debug)]
pub struct EditOptions {
    pub style: TextStyle,
    pub multiline: bool,
    /// Takes keyboard focus whenever nothing else holds it (on creation,
    /// and again after a blur) — never from a focused control.
    pub autofocus: bool,
    /// Selection highlight color.
    pub accent: Color,
}

impl Default for EditOptions {
    fn default() -> Self {
        Self {
            style: TextStyle::default(),
            multiline: false,
            autofocus: false,
            accent: Color::rgba8(0x3b, 0x5b, 0xd4, 0x66),
        }
    }
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
    /// Cached wrapped measurement: (version, wrap bits, size).
    measured: Option<(u64, u32, Size)>,
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
}

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

pub struct EditStore {
    states: FxHashMap<Key, EditState>,
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
}

impl Default for EditStore {
    fn default() -> Self {
        Self {
            states: FxHashMap::default(),
            focused: None,
            dragging: None,
            caret_moved: None,
            caret_stamp: 0,
            blink_visible: true,
        }
    }
}

fn attrs_for<'a>(style: &TextStyle, res: &'a Resources) -> Attrs<'a> {
    Attrs::new().family(res.family_of(style.family))
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
                s.measured = None;
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

    /// Ensures state exists for `key`, seeding `initial` on first creation.
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
    ) {
        let state = self.states.entry(key).or_insert_with(|| {
            let metrics = Metrics::new(opts.style.size * scale, opts.style.line_height * scale);
            let mut buffer = Buffer::new(fs, metrics);
            buffer.set_size(None, None);
            buffer.set_text(
                initial,
                &attrs_for(&opts.style, res),
                Shaping::Advanced,
                None,
            );
            EditState {
                editor: Editor::new(buffer),
                style: opts.style,
                accent: opts.accent,
                multiline: opts.multiline,
                origin,
                scale,
                wrap: None,
                version: 0,
                measured: None,
                preedit: None,
                undo: VecDeque::new(),
                redo: VecDeque::new(),
                coalesce: None,
            }
        });
        state.origin = origin;
        state.multiline = opts.multiline;
        state.accent = opts.accent;
        // Style/scale changes re-metric the buffer (text and cursor survive).
        if state.scale != scale || state.style != opts.style {
            state.style = opts.style;
            state.scale = scale;
            let metrics = Metrics::new(opts.style.size * scale, opts.style.line_height * scale);
            state.editor.with_buffer_mut(|b| b.set_metrics(metrics));
            state.wrap = None;
        }
        // `autofocus` is the core's decision (it owns the one focus).
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
        if let Some(s) = self.states.get_mut(&key) {
            s.preedit = None;
            let a = attrs_for(&s.style, res);
            s.editor
                .with_buffer_mut(|b| b.set_text(text, &a, Shaping::Advanced, None));
            s.editor.set_selection(Selection::None);
            s.editor.action(fs, Action::Motion(Motion::BufferEnd));
            s.version += 1;
            s.measured = None;
            s.wrap = None;
            // A wholesale replacement invalidates the recorded deltas.
            s.undo.clear();
            s.redo.clear();
            s.coalesce = None;
            self.touch_caret(key);
        }
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
        s.measured = None;
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
            s.measured = None;
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
        s.measured = None;
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
            s.measured = None;
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
                s.measured = None;
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
        s.measured = None;
        self.touch_caret(key);
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
            x as f32,
            y as f32,
            (2.0 * s.scale).max(2.0),
            line_height,
        ))
    }

    // -- Layout measurement (logical units)

    pub(crate) fn intrinsic(&mut self, key: Key, fs: &mut FontSystem) -> Size {
        let Some(s) = self.states.get_mut(&key) else {
            return Size::ZERO;
        };
        s.editor.shape_as_needed(fs, false);
        let (w, h, lh) = s.editor.with_buffer(|b| {
            let mut w = 0.0f32;
            let mut lines = 0u32;
            for run in b.layout_runs() {
                w = w.max(run.line_w);
                lines += 1;
            }
            (
                w,
                lines.max(1) as f32 * b.metrics().line_height,
                b.metrics().line_height,
            )
        });
        let _ = lh;
        // Caret margin so the cursor at line end isn't clipped.
        Size::new((w + 2.0 * s.scale) / s.scale, h / s.scale)
    }

    pub(crate) fn wrapped(&mut self, key: Key, max_w: f32, fs: &mut FontSystem) -> Size {
        let Some(s) = self.states.get_mut(&key) else {
            return Size::ZERO;
        };
        let target = (max_w * s.scale).max(1.0);
        let differs = match s.wrap {
            Some(a) => (a - target).abs() > 0.5,
            None => true,
        };
        if differs {
            s.editor.with_buffer_mut(|b| b.set_size(Some(target), None));
            s.wrap = Some(target);
        }
        let stamp = (s.version, target.to_bits());
        if let Some((v, w, size)) = s.measured
            && (v, w) == stamp
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
        s.measured = Some((stamp.0, stamp.1, size));
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
        clip: Rect,
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
        let color = s.style.color;
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

        s.editor.with_buffer(|b| {
            let line_height = b.metrics().line_height;
            // Runs come in line order: skip everything above the clip and
            // stop at the first run past its bottom — a 100k-line document
            // emits only the visible screenful of quads.
            let runs = b
                .layout_runs()
                .filter(|run| origin.y + run.line_top + line_height >= clip.y)
                .take_while(|run| origin.y + run.line_top <= clip.y + clip.h);
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
                            clip,
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
                            clip,
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
                            clip,
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
                        clip,
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
                    clip,
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
