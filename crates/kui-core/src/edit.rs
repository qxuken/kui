//! Editable text: the retained state of every `text_edit` node, keyed by
//! node `Key`, built on cosmic-text's editor.
//!
//! A view declares an editor with `Ui::text_edit(label, initial, &opts,
//! spec)`, where `opts` is an [`EditOptions`]. The core keeps the buffer,
//! caret, selection, IME composition and undo history across frames,
//! routes `InputEvent::Text` / `InputEvent::Key` to the focused editor and
//! posts the `change`, `submit`, `cancel` and `boundary` events its spec
//! declares rows for. An app reads the draft back with
//! `Core::edit_text` / `Ui::edit_text` and replaces it with
//! `Core::set_edit_text`, or declares it on every frame as the editor's
//! value with `Ui::text_edit_value` (`EditOptions::value`); the store
//! itself is not something an app touches.
//!
//! ```rust
//! use kui_core::{Core, EditOptions, InputEvent, NodeSpec, Size, TextStyle};
//!
//! let mut core = Core::new();
//! let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
//! let opts = EditOptions {
//!     style: TextStyle::new(14.0),
//!     autofocus: true,
//!     ..Default::default()
//! };
//! let field = ui.text_edit("name", "hello", &opts, NodeSpec::row().grow_width().pad(4.0));
//! ui.finish();
//!
//! // Typing reaches the focused editor; the host reads the draft back.
//! core.handle_input(InputEvent::Text(" world".into()));
//! assert_eq!(core.edit_text(field).as_deref(), Some("hello world"));
//! ```

use std::collections::VecDeque;

use cosmic_text::{
    Action, Attrs, AttrsList, Buffer, Cursor, Edit as _, Editor, FontSystem, Motion, Selection,
    Shaping, Wrap,
};
use rustc_hash::FxHashMap;

use crate::color::Color;
use crate::display::{Clip, ClipId, Quad, QuadKind};
use crate::geom::{Rect, Size, Vec2};
use crate::input::{EditKey, Mods};
use crate::key::Key;
use crate::resources::Resources;
use crate::spec::{TextStyle, TextWrap};
use crate::text::TextSystem;
use crate::tree::OriginId;

/// How a `text_edit` node behaves: its text style, whether it is a
/// single-line field or a multiline document, and how it takes focus and
/// paints its selection. Build one with struct update syntax from
/// `Default`.
#[derive(Clone, Debug, Default)]
pub struct EditOptions {
    pub style: TextStyle,
    /// A document rather than a field: Enter inserts a newline, the text
    /// wraps to the box, and the caret opens at the top.
    pub multiline: bool,
    /// A single-line editor that folds to its width, by `style.wrap`, the
    /// way a document does — instead of taking one line and scrolling it
    /// under the caret. Its keyboard is still a field's:
    /// Enter submits, a newline is never admitted, the caret opens at the
    /// end. In the bindings this is the `wrap` row being declared on the
    /// element; off, a field does not read `style.wrap` at all. A
    /// `multiline` editor wraps either way and ignores this.
    pub wrap: bool,
    /// Takes keyboard focus on the frame this declaration starts — a new
    /// editor, one back after a gap, one whose flag just turned on — and
    /// only while nothing holds focus: never from a focused control, and
    /// never again after a blur.
    pub autofocus: bool,
    /// A single-line editor whose Tab is the app's: Tab and Shift-Tab do
    /// not walk the focus ring from it, and the press goes to the key
    /// sink above it as a chord the editor does not claim does — so a
    /// list or an outline built from fields indents on Tab (backlog F146).
    /// With no sink above, Tab does nothing here. A `multiline` editor
    /// takes it too: its Tab then goes to the sink as well.
    pub keep_tab: bool,
    /// A `multiline` editor whose Tab and Shift-Tab walk the focus ring,
    /// as a browser's textarea does, instead of typing indentation.
    ///
    /// Without it a multiline editor takes Tab — the editors' model, not
    /// the textarea's (backlog C99): Tab types four spaces and Shift-Tab
    /// takes up to four back from the start of the caret's line, and the
    /// keyboard still has three ways out, which is what WCAG's
    /// no-keyboard-trap criterion asks for. Escape then Tab moves focus
    /// (CodeMirror's escape hatch: the Escape arms the next Tab only); the
    /// window's "Tab moves focus" toggle, Ctrl+M or Ctrl+Shift+M on macOS
    /// as Monaco binds it, makes every such editor walk the ring
    /// ([`crate::Core::tab_moves_focus`]); and Tab held with Ctrl, Alt or
    /// ⌘ walks it always. A single-line field ignores this: its Tab walks
    /// the ring either way.
    pub tab_moves_focus: bool,
    /// What an empty editor shows where its text would be, in the theme's
    /// `faint` (backlog F149): never part of the value, gone with the
    /// first character typed or composed, and read by assistive
    /// technology as the field's description when it declares none.
    pub placeholder: Option<String>,
    /// Selection highlight color. `None` is the theme's `selection`,
    /// which is what a field gets unless the caller says otherwise — so a
    /// selection over a label and one over a field are the same tint on
    /// both bases.
    pub accent: Option<Color>,
    /// A password field (backlog F167): the text is drawn as a bullet
    /// (•) per grapheme, and measured, hit and selected as the bullets;
    /// a copy or a cut of it is refused, a paste is taken; no input method
    /// composes into it; it keeps no undo history, which would hold what
    /// was typed; the access tree reads it as a password field whose value
    /// is the bullets; and while it has the keyboard the window asks for
    /// secure keyboard entry and the input method off, as
    /// [`Core::secure_input`](crate::Core::secure_input) and
    /// [`Core::ime_off`](crate::Core::ime_off) read back. The app still
    /// reads the text with `edit_text`. Declared off on a later frame —
    /// a "show password" toggle — the same editor draws the text again,
    /// the caret where it was; on again, it masks it. A secure editor is
    /// a field: `multiline` is ignored, and ⌥← ⌥→ (Ctrl elsewhere) and a
    /// double click take the whole text, there being no words to show.
    /// A frame that does not declare it forgets it, its text zeroed, where
    /// an ordinary editor's draft is kept while it is hidden: a form
    /// closed and opened again starts empty, or at the `value` it declares
    /// (backlog F177). The text the app was handed — each `change`'s, and
    /// `edit_text`'s answer — is the app's copy, and the app's to clear.
    pub secure: bool,
    /// The text `text_edit` is given is the editor's value, not its seed
    /// (ADR 0047, decision 8): on every frame it differs from the
    /// editor's text, the editor is brought to it. Only the span between
    /// what the two share at the start and at the end changes, so a
    /// caret before it stays, one after it moves with the text, and one
    /// inside it goes to the end of what came in; a selection over it
    /// goes. Applied on the frame after the user's own edit, it joins
    /// that edit's undo step, so a filter's correction undoes with the
    /// keystroke it corrected; any other is an undo step of its own.
    /// While an input method composes, the value waits for the commit.
    /// Equal, which is every frame of ordinary typing, it costs a string
    /// compare. A secure editor's value is its secret text.
    pub value: bool,
}

/// What a secure editor draws for each grapheme of its text.
pub const SECURE_BULLET: char = '\u{2022}';
/// [`SECURE_BULLET`]'s length in a secure editor's buffer.
const BULLET_LEN: usize = SECURE_BULLET.len_utf8();

/// `n` bullets, the buffer of a secure editor whose text has `n`
/// graphemes.
fn bullets(n: usize) -> String {
    std::iter::repeat_n(SECURE_BULLET, n).collect()
}

/// How many graphemes `text` has: the bullets a secure editor draws for it.
fn graphemes(text: &str) -> usize {
    use unicode_segmentation::UnicodeSegmentation;
    text.graphemes(true).count()
}

/// The byte where grapheme `n` of `text` starts; its length for `n` past
/// the last.
fn grapheme_start(text: &str, n: usize) -> usize {
    use unicode_segmentation::UnicodeSegmentation;
    text.grapheme_indices(true)
        .nth(n)
        .map_or(text.len(), |(at, _)| at)
}

/// How many graphemes of `text` start before `byte`: the bullet a byte
/// offset into a secure editor's text is drawn at.
fn graphemes_before(text: &str, byte: usize) -> usize {
    use unicode_segmentation::UnicodeSegmentation;
    text.grapheme_indices(true)
        .take_while(|(at, _)| *at < byte)
        .count()
}

/// What an editing key did to an editor: whether the text changed,
/// whether it submitted the field, and the edge it met when it could do
/// neither (backlog F145).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct KeyOutcome {
    pub changed: bool,
    pub submit: bool,
    pub boundary: Option<Edge>,
}

/// Which end of an editor's text a key met: the start (↑ on the first
/// line, ← or Backspace at offset 0) or the end (↓ on the last line, → or
/// Delete at the end).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Edge {
    Start,
    End,
}

impl Edge {
    pub fn name(self) -> &'static str {
        match self {
            Edge::Start => "start",
            Edge::End => "end",
        }
    }
}

pub(crate) struct EditState {
    editor: Editor<'static>,
    pub(crate) style: TextStyle,
    pub(crate) accent: Color,
    pub(crate) multiline: bool,
    /// `EditOptions::keep_tab`, as last declared.
    keep_tab: bool,
    /// `EditOptions::tab_moves_focus`, as last declared.
    tab_moves_focus: bool,
    /// A secure editor's text (`EditOptions::secure`), whose buffer holds
    /// a [`SECURE_BULLET`] per grapheme of it. `None` for any other: the
    /// buffer is the text.
    secret: Option<Secret>,
    /// Whether the editor was ever declared `secure`: a password shown
    /// again ("show password") is still a password, and the sweep at the
    /// end of a frame forgets it when undeclared as it does a masked one.
    was_secure: bool,
    /// `EditOptions::placeholder`, shaped in a buffer of its own at the
    /// editor's metrics, beside the `metrics_rev` it was shaped at.
    placeholder: Option<(String, u32, Buffer)>,
    /// Whether the buffer is laid out to its box's width: a document, or a
    /// field with `wrap` declared. What `wrapped`,
    /// `line_offset` and emission's clip decide by — a field that does not
    /// fold takes one line and scrolls it.
    folds: bool,
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
    /// width `wrapped` last set on it.
    natural: Option<(u64, u32, Size)>,
    /// How far a single-line field has scrolled its text left, in physical
    /// px. A field keeps the caret inside its box by moving the text under
    /// it, the way a native field does, rather than by wrapping; an
    /// editor that folds (`folds`) wraps and this stays 0.
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
    /// Whether the user's own edit went into the history since this
    /// editor was last declared: a declared value that differs on the
    /// next declaration is that edit's correction, and joins its undo
    /// step (`EditOptions::value`).
    fresh_edit: bool,
    /// Whether the last declaration carried `autofocus`: with
    /// `last_declared`, what makes the next one an edge or a repeat.
    autofocus: bool,
    /// The frame this key was last declared in. Only the budget reads it
    /// (see [`MAX_UNDECLARED_EDITS`]); a state declared every frame never
    /// looks at it again.
    last_declared: u64,
}

/// How many *undeclared* editors the store keeps before the longest
/// undeclared one is dropped. A declared editor is never evicted, however
/// many there are: retention across absence is what `text_edit` promises,
/// so this is a ceiling, not a prune. An editor holding a short line is a
/// few KB and one holding a long line about 22 KB, so a full budget is a
/// few MB.
pub const MAX_UNDECLARED_EDITS: usize = 256;

/// A secure editor's text: the one copy kui keeps of it, zeroed when it is
/// dropped or replaced, and grown into a fresh allocation with the old one
/// zeroed, so a password does not linger in memory kui has let go of
/// (backlog F177). What it cannot reach is what left it on purpose — the
/// `change` message's text and `edit_text`'s answer are the app's copies —
/// and what the allocator or the input method made of the bytes before
/// kui held them.
pub(crate) struct Secret(String);

impl Secret {
    /// Takes `text`, which is kui's own temporary: copied into room for
    /// growth, and the temporary zeroed.
    fn new(mut text: String) -> Self {
        let mut own = String::with_capacity(text.len().max(32).next_power_of_two());
        own.push_str(&text);
        wipe(&mut text);
        Secret(own)
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    /// `String::replace_range`, growing into a fresh allocation — and the
    /// old one zeroed — rather than letting the allocator move the bytes.
    fn replace_range(&mut self, range: std::ops::Range<usize>, text: &str) {
        let need = self.0.len() - (range.end - range.start) + text.len();
        if need > self.0.capacity() {
            let mut grown = String::with_capacity(need.next_power_of_two());
            grown.push_str(&self.0);
            wipe(&mut std::mem::replace(&mut self.0, grown));
        }
        self.0.replace_range(range, text);
    }

    /// The text, for a secure editor shown again: the editor's own text
    /// from now on.
    fn into_string(mut self) -> String {
        std::mem::take(&mut self.0)
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        wipe(&mut self.0);
    }
}

/// Zeroes all of `s`'s allocation — the spare capacity too, which a
/// shrinking edit left the old bytes in — through volatile writes the
/// optimizer may not drop for a buffer about to be freed.
fn wipe(s: &mut String) {
    let cap = s.capacity();
    // SAFETY: the bytes are made all zero, which is valid UTF-8, and the
    // length is then cleared; every write is inside the allocation.
    unsafe {
        let v = s.as_mut_vec();
        let p = v.as_mut_ptr();
        for i in 0..cap {
            std::ptr::write_volatile(p.add(i), 0);
        }
        v.set_len(0);
    }
    std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
}

const UNDO_CAP: usize = 1000;
/// What Tab types in a multiline editor whose Tab is its own, and the
/// most Shift-Tab takes back from the start of a line.
const TAB_INDENT: &str = "    ";
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
    /// Undone and redone with the op below it, as one step: a declared
    /// value's correction of the user's edit (`EditOptions::value`).
    joins: bool,
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

/// A drag's selection from the caret to itself is no selection: cosmic-
/// text's `Drag` seeds one at the caret before it moves, so a drag that
/// ended where it began — or a Shift-click on the caret — held an empty
/// one, which reports no edge and is what Backspace took (backlog F170).
/// Only a character selection: a word's or a line's is never empty.
fn no_empty_selection(editor: &mut Editor<'static>) {
    if let Selection::Normal(a) = editor.selection()
        && same_pos(a, editor.cursor())
    {
        editor.set_selection(Selection::None);
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
        // A secure editor keeps no history: it would hold what was typed.
        if self.secret.is_some() {
            return;
        }
        self.fresh_edit = true;
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
        self.push_undo(op);
        self.coalesce = kind;
    }

    /// Pushes `op` as the newest undo op, dropping the oldest past
    /// [`UNDO_CAP`]. An op that joined the dropped one is a step of its
    /// own from then on: there is nothing left below it to undo with it.
    fn push_undo(&mut self, op: EditOp) {
        if self.undo.len() >= UNDO_CAP {
            self.undo.pop_front();
            if let Some(first) = self.undo.front_mut() {
                first.joins = false;
            }
        }
        self.undo.push_back(op);
    }

    /// Caret motion, clicks, blur: the next edit starts a new undo unit.
    fn break_coalesce(&mut self) {
        self.coalesce = None;
    }

    /// Whether this editor's Tab types (`EditOptions::tab_moves_focus`):
    /// multiline, and neither walking the ring by declaration nor handing
    /// Tab to the sink above (`keep_tab`).
    fn types_tab(&self) -> bool {
        self.multiline && !self.tab_moves_focus && !self.keep_tab
    }

    /// Shift-Tab's half of indentation: takes up to one indent — four
    /// spaces, or one tab character — from the start of the caret's line,
    /// as one recorded op, the caret keeping its place in the text.
    /// False when the line starts with neither.
    fn outdent_recorded(&mut self) -> bool {
        if self.secret.is_some() {
            return false;
        }
        let cursor = self.editor.cursor();
        let deleted = self.editor.with_buffer(|b| {
            let Some(line) = b.lines.get(cursor.line) else {
                return String::new();
            };
            let t = line.text();
            let lead = if t.starts_with('\t') {
                1
            } else {
                t.bytes()
                    .take(TAB_INDENT.len())
                    .take_while(|&c| c == b' ')
                    .count()
            };
            t[..lead].to_string()
        });
        if deleted.is_empty() {
            return false;
        }
        let at = Cursor::new(cursor.line, 0);
        self.splice(at, &deleted, "");
        let after = Cursor::new(cursor.line, cursor.index.saturating_sub(deleted.len()));
        self.editor.set_cursor(after);
        self.break_coalesce();
        self.record(
            EditOp {
                at,
                deleted,
                inserted: String::new(),
                cursor_before: cursor,
                cursor_after: after,
                joins: false,
            },
            None,
        );
        true
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
        if self.secret.is_some() {
            self.secure_replace(text);
            return;
        }
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
                joins: false,
            },
            kind,
        );
    }

    /// Deletes the selection as one recorded op; false when there is none.
    fn delete_selection_recorded(&mut self) -> bool {
        if self.secret.is_some() {
            return self.selected().is_some() && {
                self.secure_replace("");
                true
            };
        }
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
                joins: false,
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
        if self.secret.is_some() {
            self.secure_replace("");
            return true;
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
                joins: false,
            },
            Some(kind),
        );
        true
    }

    /// The selection's ends in the buffer, when it is not empty.
    fn selected(&self) -> Option<(Cursor, Cursor)> {
        self.editor
            .selection_bounds()
            .filter(|(a, z)| !same_pos(*a, *z))
    }

    /// A secure editor's edit: `text` over the selection, or at the caret,
    /// in its text, and the buffer as many bullets as the text has
    /// graphemes, the caret after what went in. Every bullet is alike, so
    /// the buffer moves by its end alone. Unrecorded: a secure editor
    /// keeps no history.
    fn secure_replace(&mut self, text: &str) {
        let Some(secret) = self.secret.as_mut() else {
            return;
        };
        // A field's: one line, whichever door the text came by.
        let text = admitted(text, false);
        let caret = self.editor.cursor();
        let (a, z) = self.editor.selection_bounds().unwrap_or((caret, caret));
        let (a, z) = (
            grapheme_start(secret.as_str(), a.index / BULLET_LEN),
            grapheme_start(secret.as_str(), z.index / BULLET_LEN),
        );
        secret.replace_range(a..z, &text);
        let at = graphemes_before(secret.as_str(), a + text.len());
        let n = graphemes(secret.as_str());
        self.set_bullets(n);
        self.editor
            .set_cursor(Cursor::new(0, at.min(n) * BULLET_LEN));
    }

    /// Makes a secure editor's buffer `n` bullets, with no selection.
    fn set_bullets(&mut self, n: usize) {
        let had = self
            .editor
            .with_buffer(|b| b.lines.first().map_or(0, |l| l.text().len()))
            / BULLET_LEN;
        self.editor.set_selection(Selection::None);
        if n > had {
            self.editor.set_cursor(Cursor::new(0, had * BULLET_LEN));
            self.editor.insert_string(&bullets(n - had), None);
        } else if n < had {
            self.editor
                .set_selection(Selection::Normal(Cursor::new(0, n * BULLET_LEN)));
            self.editor.set_cursor(Cursor::new(0, had * BULLET_LEN));
            self.editor.delete_selection();
        }
    }

    /// Masks the text (`on`) or shows it again, the caret on the grapheme
    /// it was on and no selection. The history goes either way: masked,
    /// it would hold the text; shown, it holds edits to the bullets.
    fn set_secure(&mut self, on: bool, attrs: &Attrs<'_>) {
        self.abandon_preedit();
        let caret = self.editor.cursor();
        let (shown, at) = if on {
            // The text as a field holds it, and how many of its graphemes
            // are before the caret.
            let (text, before) = self.editor.with_buffer(|b| {
                let text = buffer_text(b);
                let at = abs_offset(b, caret).min(text.len());
                let before = text.get(..at).map_or(0, |t| graphemes(&admitted(t, false)));
                (admitted(&text, false).into_owned(), before)
            });
            let shown = bullets(graphemes(&text));
            self.secret = Some(Secret::new(text));
            self.was_secure = true;
            (shown, before * BULLET_LEN)
        } else {
            let text = self
                .secret
                .take()
                .map(Secret::into_string)
                .unwrap_or_default();
            let at = grapheme_start(&text, caret.index / BULLET_LEN);
            (text, at)
        };
        self.editor
            .with_buffer_mut(|b| b.set_text(&shown, attrs, Shaping::Advanced, None));
        self.editor.set_selection(Selection::None);
        self.editor.set_cursor(Cursor::new(0, at));
        self.undo.clear();
        self.redo.clear();
        self.coalesce = None;
        self.version += 1;
        self.wrap = None;
        self.invalidate_measurements();
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

    /// Undoes one step: the newest op, and with it every op below that it
    /// joins (`EditOp::joins`).
    fn undo_one(&mut self) -> bool {
        let mut any = false;
        while let Some(op) = self.undo.pop_back() {
            self.splice(op.at, &op.inserted, &op.deleted);
            self.editor.set_cursor(op.cursor_before);
            self.editor.set_selection(Selection::None);
            let joins = op.joins;
            self.redo.push_back(op);
            any = true;
            if !joins {
                break;
            }
        }
        self.coalesce = None;
        self.fresh_edit = false;
        any
    }

    /// Redoes one step: the op undone last, and every op above it that
    /// joins it.
    fn redo_one(&mut self) -> bool {
        let Some(op) = self.redo.pop_back() else {
            return false;
        };
        let mut next = Some(op);
        while let Some(op) = next.take() {
            self.splice(op.at, &op.deleted, &op.inserted);
            self.editor.set_cursor(op.cursor_after);
            self.editor.set_selection(Selection::None);
            self.undo.push_back(op);
            if self.redo.back().is_some_and(|o| o.joins) {
                next = self.redo.pop_back();
            }
        }
        self.coalesce = None;
        self.fresh_edit = false;
        true
    }

    /// Brings the editor to a declared `value` (`EditOptions::value`) as
    /// a splice: the span between what the two texts share at the start
    /// and at the end is replaced, and the caret and selection move as
    /// [`moved`] says. `joins` makes the splice part of the undo step
    /// below it, the user's edit it corrects. Returns whether the caret
    /// moved; `false` too when the two were equal, which is the common
    /// case and is answered by comparing the value with the buffer in
    /// place, without building the editor's text.
    fn take_value(&mut self, value: &str, joins: bool, fs: &mut FontSystem) -> bool {
        if self.secret.is_some() {
            return self.take_secret_value(value);
        }
        if self.editor.with_buffer(|b| buffer_is(b, value)) {
            return false;
        }
        // The value as the buffer would hold it: what a field drops, and
        // a line break in whichever spelling as the one `buffer_text`
        // joins lines with. A value that differs only there is equal.
        let admitted = admitted(value, self.multiline);
        let value = as_buffered(&admitted);
        if self.editor.with_buffer(|b| buffer_is(b, &value)) {
            return false;
        }
        let old = self.editor.with_buffer(buffer_text);
        let span = splice_span(&old, &value);
        let (start, old_end, new_end) = span;
        let cursor_before = self.editor.cursor();
        let (caret, bounds) = self.editor.with_buffer(|b| {
            let caret = abs_offset(b, cursor_before);
            let bounds = self
                .editor
                .selection_bounds()
                .map(|(a, z)| (abs_offset(b, a), abs_offset(b, z)))
                .filter(|(a, z)| a != z);
            (caret, bounds)
        });
        let anchor = self.editor.with_buffer(|b| match self.editor.selection() {
            Selection::None => None,
            Selection::Normal(c) | Selection::Line(c) | Selection::Word(c) => {
                Some(abs_offset(b, c))
            }
        });
        let selection = self.editor.selection();
        let at = cursor_in(&old, start);
        self.splice(at, &old[start..old_end], &value[start..new_end]);
        let new_caret = moved(caret, span);
        let cursor_after = cursor_in(&value, new_caret);
        self.editor.set_cursor(cursor_after);
        // A selection over the span goes; one clear of it stays, its
        // anchor moved as the caret is.
        let kept = bounds.is_none_or(|(a, z)| !overlaps(a, z, span));
        self.editor.set_selection(match (selection, anchor) {
            (_, Some(a)) if kept && moved(a, span) != new_caret => {
                let c = cursor_in(&value, moved(a, span));
                match selection {
                    Selection::Line(_) => Selection::Line(c),
                    Selection::Word(_) => Selection::Word(c),
                    _ => Selection::Normal(c),
                }
            }
            _ => Selection::None,
        });
        self.redo.clear();
        self.push_undo(EditOp {
            at,
            deleted: old[start..old_end].to_string(),
            inserted: value[start..new_end].to_string(),
            cursor_before,
            cursor_after,
            joins: joins && !self.undo.is_empty(),
        });
        // The next keystroke is an edit of its own, not more of this.
        self.coalesce = None;
        self.editor.shape_as_needed(fs, false);
        self.version += 1;
        self.invalidate_measurements();
        new_caret != caret
    }

    /// [`Self::take_value`] for a secure editor: the value is its secret
    /// text, spliced there, and the buffer becomes as many bullets as the
    /// new text has graphemes, the caret and the selection on the
    /// graphemes the splice rules move them to. Unrecorded, as every
    /// secure edit is.
    fn take_secret_value(&mut self, value: &str) -> bool {
        let Some(secret) = self.secret.as_ref().map(Secret::as_str) else {
            return false;
        };
        if secret == value {
            return false;
        }
        let value = admitted(value, false);
        if secret == &*value {
            return false;
        }
        let span = splice_span(secret, &value);
        let byte = |c: Cursor| grapheme_start(secret, c.index / BULLET_LEN);
        let caret = byte(self.editor.cursor());
        let bounds = self.selected().map(|(a, z)| (byte(a), byte(z)));
        let anchor = match self.editor.selection() {
            Selection::Normal(c) | Selection::Line(c) | Selection::Word(c) => Some(byte(c)),
            Selection::None => None,
        };
        let new_caret = moved(caret, span);
        let kept = bounds.is_none_or(|(a, z)| !overlaps(a, z, span));
        let anchor = anchor.filter(|_| kept).map(|a| moved(a, span));
        let bullet = |at: usize| Cursor::new(0, graphemes_before(&value, at) * BULLET_LEN);
        let (caret_at, anchor_at) = (bullet(new_caret), anchor.map(bullet));
        let moved_caret = !same_pos(caret_at, self.editor.cursor());
        self.set_bullets(graphemes(&value));
        self.editor.set_cursor(caret_at);
        if let Some(a) = anchor_at.filter(|a| !same_pos(*a, caret_at)) {
            self.editor.set_selection(Selection::Normal(a));
        }
        self.secret = Some(Secret::new(value.into_owned()));
        self.redo.clear();
        self.version += 1;
        self.invalidate_measurements();
        moved_caret
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

/// The retained editors of one window, owned by `Core`. Reach it through
/// `Ui::text_edit`, `Core::edit_text` and `Core::set_edit_text` rather
/// than directly.
pub struct EditStore {
    states: FxHashMap<Key, EditState>,
    /// Text set for a key nothing has declared yet: `set_text` holds it
    /// here and the next `declare` under that key seeds with it instead of
    /// with `initial`. An `update` that opens an editor and sets its text
    /// in the same turn runs a frame ahead of the view that declares it,
    /// so without this the call lands on nothing and the app sees the
    /// editor open with `initial`. What the frame after it
    /// does not claim is dropped by `finish_frame`, with a warning.
    pending: FxHashMap<Key, String>,
    /// The same seed named by label instead of by key, for the app that
    /// has no key to give: the hex key comes from an event the node
    /// fired, and an editor a rename is opening for the first time has
    /// fired none. Held until `text_edit` declares an
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
    /// Whether a secure editor has been declared since the last frame that
    /// had none: what makes [`EditStore::forget_undeclared_secrets`] walk
    /// the store, so an app with no password field pays one test a frame.
    any_secure: bool,
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
            any_secure: false,
        }
    }
}

/// What an editor's buffer may hold: a field is one line, so a newline that
/// arrived in a seed or a `set_text` is dropped rather than drawn below a
/// box measured for one line. The same rule `apply_text` applies to
/// typing and pasting, so the two doors agree. The paragraph separator
/// (U+2029) is the break it names, a newline (backlog FZ6: the shaper
/// panicked on one between text running two ways), and the other
/// characters that end a bidi paragraph go through
/// [`one_paragraph`](crate::text::one_paragraph).
fn admitted(text: &str, multiline: bool) -> std::borrow::Cow<'_, str> {
    let text: std::borrow::Cow<'_, str> = if text.contains('\u{2029}') {
        text.replace('\u{2029}', "\n").into()
    } else {
        text.into()
    };
    let text = if multiline || !text.contains(['\n', '\r']) {
        text
    } else {
        text.chars()
            .filter(|c| *c != '\n' && *c != '\r')
            .collect::<String>()
            .into()
    };
    match crate::text::one_paragraph(&text) {
        std::borrow::Cow::Borrowed(_) => text,
        std::borrow::Cow::Owned(s) => s.into(),
    }
}

/// What typing `c` puts in an editor: the character; the newline the
/// paragraph separator (U+2029) names, as in `admitted` (backlog FZ6);
/// or nothing, for a control a field does not take — every one but a tab
/// and, in a multiline editor, the newline. The other bidi paragraph ends
/// are controls, and go with them.
fn typed(c: char, multiline: bool) -> Option<char> {
    let c = if c == '\u{2029}' { '\n' } else { c };
    (!c.is_control() || (c == '\n' && multiline) || c == '\t').then_some(c)
}

/// An IME's composition as the editor holds it: typed text in waiting,
/// so what typing drops goes (backlog FZ7). A `\r` went in as it came,
/// cosmic-text broke the line at it, and `end_cursor` - which counts
/// `\n` alone - put the composition's end past the line it was on: the
/// next key that ended the composition split the buffer off a character
/// boundary, a panic in cosmic-text. The IME's byte offsets move with
/// what stays.
fn composed(
    text: &str,
    cursor: Option<(usize, usize)>,
    multiline: bool,
) -> (String, Option<(usize, usize)>) {
    let mut out = String::with_capacity(text.len());
    // Where each character of `text` starts, there and in `out`.
    let mut starts = Vec::with_capacity(text.len());
    for (at, c) in text.char_indices() {
        starts.push((at, out.len()));
        if let Some(c) = typed(c, multiline) {
            out.push(c);
        }
    }
    let moved = |o: usize| {
        starts
            .iter()
            .find(|(at, _)| *at >= o)
            .map_or(out.len(), |(_, kept)| *kept)
    };
    let cursor = cursor.map(|(a, b)| (moved(a), moved(b)));
    (out, cursor)
}

/// At the family's regular weight, or its bold for a bold style, as
/// text draws it.
fn attrs_for<'a>(style: &TextStyle, res: &'a Resources) -> Attrs<'a> {
    res.weights_of(style.family).apply(
        Attrs::new()
            .family(res.family_of(style.family))
            .font_features(crate::text::cosmic_features(&style.features)),
        style.bold,
    )
}

impl EditStore {
    /// Gives every editor's text the weights its family is asked at now:
    /// a face of a registered family came or went. The text,
    /// caret and history stay; every line shapes again, and every
    /// measurement of it is of the old weights. Whether or not a line's
    /// attributes moved: a new fallback list (`set_fallback_fonts`)
    /// changes the faces a line is shaped in and none of its attributes
    /// (RG114).
    pub(crate) fn reweigh(&mut self, res: &Resources) {
        for s in self.states.values_mut() {
            let a = AttrsList::new(&attrs_for(&s.style, res));
            s.editor.with_buffer_mut(|b| {
                for line in &mut b.lines {
                    line.set_attrs_list(a.clone());
                    line.reset_shaping();
                }
            });
            s.editor.set_redraw(true);
            s.wrap = None;
            s.metrics_rev = s.metrics_rev.wrapping_add(1);
            s.invalidate_measurements();
        }
    }

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

    /// The phase as last set; `true` draws the caret.
    pub fn blink_visible(&self) -> bool {
        self.blink_visible
    }

    fn touch_caret(&mut self, key: Key) {
        self.caret_moved = Some(key);
        self.caret_stamp += 1;
    }

    /// How many states are retained — declared and undeclared together.
    /// What a test watches the budget through.
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

    /// Drops every secure editor the frame being finished did not declare,
    /// its text zeroed with it (backlog F177). An undeclared editor is
    /// otherwise kept for the budget — a draft survives a field hidden for
    /// a while — but a password kept is a password an app that closed its
    /// form never meant to keep: carnet's sign-in form showed it again on
    /// reopening, and nothing it could see said kui still held it.
    ///
    /// The focused key stays the store's: the core keeps focus on a node
    /// that went away (a dock toggled off lands where the user was), and a
    /// field that comes back is focused again, empty, with no state for a
    /// keystroke in between to reach. A drag on it is let go.
    pub(crate) fn forget_undeclared_secrets(&mut self) {
        if !self.any_secure {
            return;
        }
        let frame = self.frame_no;
        let mut gone: Vec<Key> = Vec::new();
        let mut any = false;
        self.states.retain(|k, s| {
            if s.secret.is_none() && !s.was_secure {
                return true;
            }
            if s.last_declared == frame {
                any = true;
                return true;
            }
            gone.push(*k);
            false
        });
        self.any_secure = any;
        for k in gone {
            if self.dragging.is_some_and(|(d, _)| d == k) {
                self.dragging = None;
            }
            if self.caret_moved == Some(k) {
                self.caret_moved = None;
            }
        }
    }

    /// Drops the oldest undeclared states down to the budget. `declared_at`
    /// is the frame that just ended: a state stamped with it is declared.
    fn evict(&mut self, declared_at: u64) {
        let focused = self.focused;
        let dragging = self.dragging.map(|(k, _)| k);
        let caret_moved = &mut self.caret_moved;
        crate::retain::evict_undeclared(
            &mut self.states,
            MAX_UNDECLARED_EDITS,
            declared_at,
            |s| s.last_declared,
            |k| Some(k) == focused || Some(k) == dragging,
            |k| {
                if *caret_moved == Some(k) {
                    *caret_moved = None;
                }
            },
        );
    }

    /// Ensures state exists for `key`, seeding `initial` on first creation
    /// — or, if a `set_text` for this key arrived before anything declared
    /// it, that text instead (see [`EditStore::pending`]). Returns whether
    /// this declaration is an *autofocus edge*: the key was not declared
    /// with `autofocus` on the frame before this one — a new editor, one
    /// back after a gap, or one whose `autofocus` just turned on — which
    /// is the one frame the flag may act on.
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
        // where it is called), and taking one here would drop it. A
        // declared value wins over it, so an editor opened with one
        // starts from the value and the held text goes.
        let seed = if self.states.contains_key(&key) {
            None
        } else {
            self.pending.remove(&key).filter(|_| !opts.value)
        };
        let seeded = seed.is_some();
        let declared = initial;
        let initial = seed.as_deref().unwrap_or(initial);
        // A secure editor is a field (`EditOptions::secure`).
        let multiline = opts.multiline && !opts.secure;
        let state = self.states.entry(key).or_insert_with(|| {
            let metrics = crate::text::shaper_metrics(
                opts.style.size * scale,
                opts.style.line_height * scale,
            );
            let mut buffer = Buffer::new(fs, metrics);
            buffer.set_size(None, None);
            let text = admitted(initial, multiline);
            let secret = opts.secure.then(|| Secret::new(text.to_string()));
            let shown = match &secret {
                Some(s) => bullets(graphemes(s.as_str())).into(),
                None => text,
            };
            buffer.set_text(
                &shown,
                &attrs_for(&opts.style, res),
                Shaping::Advanced,
                None,
            );
            let mut editor = Editor::new(buffer);
            // A single-line field opens with the caret after its seeded
            // text — what a native field does with a prefilled value, and
            // what a rename wants, since typing into a name meant to be
            // extended otherwise prepends to it. A multiline
            // editor is a document and opens at its top, as native text
            // views do. Placed, not moved: no `touch_caret`, so nothing
            // scrolls to reveal it before the user has touched it.
            //
            // A held `set_text` is not `initial`: it is that call arriving
            // where it can land, so it leaves the caret where the call
            // does — at the end, document or not.
            if seeded || !multiline {
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
                multiline,
                keep_tab: opts.keep_tab,
                tab_moves_focus: opts.tab_moves_focus,
                was_secure: secret.is_some(),
                secret,
                placeholder: None,
                folds: false,
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
                fresh_edit: false,
                autofocus: false,
                last_declared: frame_no,
            }
        });
        state.last_declared = frame_no;
        self.any_secure |= opts.secure || state.was_secure;
        state.autofocus = opts.autofocus;
        state.origin = origin;
        state.multiline = multiline;
        state.keep_tab = opts.keep_tab;
        state.tab_moves_focus = opts.tab_moves_focus;
        // Masked or shown again: a "show password" toggle on the same
        // editor, the caret on the same grapheme.
        if opts.secure != state.secret.is_some() {
            state.set_secure(opts.secure, &attrs_for(&opts.style, res));
        }
        state.accent = opts.accent.unwrap_or(crate::select::TINT);
        // A document wraps between words whatever its style says, as it
        // always has; a field folds only when asked, and then by the mode
        // the `wrap` row picked — so a rename field breaks where the label
        // it renames breaks. `wrap="none"` on a field is the
        // field: one line, scrolled.
        let folds = multiline || (opts.wrap && opts.style.wrap != TextWrap::None);
        let mode = match (multiline, opts.style.wrap) {
            (false, TextWrap::Glyph) => Wrap::Glyph,
            _ => Wrap::WordOrGlyph,
        };
        if state.folds != folds || state.editor.with_buffer(|b| b.wrap()) != mode {
            state.folds = folds;
            state.editor.with_buffer_mut(|b| b.set_wrap(mode));
            state.invalidate_measurements();
        }
        // Style/scale changes re-metric the buffer (text and cursor survive).
        // The text is the same, so `version` does not move — but every
        // measurement of it is now of the wrong font, which is what
        // `metrics_rev` is for: without it a cache keyed on the text alone
        // answers the new frame with the old size.
        if state.scale != scale || state.style != opts.style {
            state.style = opts.style;
            state.scale = scale;
            let metrics = crate::text::shaper_metrics(
                opts.style.size * scale,
                opts.style.line_height * scale,
            );
            state.editor.with_buffer_mut(|b| b.set_metrics(metrics));
            state.wrap = None;
            state.metrics_rev = state.metrics_rev.wrapping_add(1);
            state.invalidate_measurements();
        }
        // The placeholder, shaped again only when its text or the
        // editor's metrics moved.
        match opts.placeholder.as_deref().filter(|p| !p.is_empty()) {
            None => state.placeholder = None,
            Some(p) => {
                let rev = state.metrics_rev;
                if state
                    .placeholder
                    .as_ref()
                    .is_none_or(|(t, r, _)| t != p || *r != rev)
                {
                    let metrics = state.editor.with_buffer(|b| b.metrics());
                    let mut b = Buffer::new(fs, metrics);
                    b.set_size(None, None);
                    b.set_text(
                        &crate::text::one_paragraph(p),
                        &attrs_for(&opts.style, res),
                        Shaping::Advanced,
                        None,
                    );
                    b.shape_until_scroll(fs, false);
                    state.placeholder = Some((p.to_string(), rev, b));
                }
            }
        }
        // A declared value, which the editor is brought to where the two
        // differ: held while an input method composes, so the preedit
        // stays and the value lands on the first declaration after the
        // commit. Whether the user edited since the last declaration is
        // asked once, here, whatever the options — but a value held for a
        // composition keeps the question for the declaration that applies
        // it: a commit and a new composition before one frame would
        // otherwise lose the commit's claim on its correction.
        let held = opts.value && state.preedit.is_some();
        let fresh = if held {
            state.fresh_edit
        } else {
            std::mem::take(&mut state.fresh_edit)
        };
        if opts.value && !held && state.take_value(declared, fresh, fs) {
            self.touch_caret(key);
        }
        // `autofocus` is the core's decision (it owns the one focus).
        edge
    }

    /// What the editor shows of its text, which is what the access tree
    /// reads: the text, but for a secure editor, whose bullets are all it
    /// gives away.
    pub(crate) fn shown_text(&self, key: Key) -> Option<String> {
        let s = self.states.get(&key)?;
        if s.secret.is_some() {
            return Some(s.editor.with_buffer(buffer_text));
        }
        self.text(key)
    }

    /// Whether `key` is a secure editor (`EditOptions::secure`).
    pub fn is_secure(&self, key: Key) -> bool {
        self.states.get(&key).is_some_and(|s| s.secret.is_some())
    }

    /// The placeholder `key` was declared with, if any.
    pub fn placeholder(&self, key: Key) -> Option<&str> {
        let s = self.states.get(&key)?;
        s.placeholder.as_ref().map(|(t, ..)| t.as_str())
    }

    pub fn contains(&self, key: Key) -> bool {
        self.states.contains_key(&key)
    }

    pub fn origin_of(&self, key: Key) -> Option<OriginId> {
        self.states.get(&key).map(|s| s.origin)
    }

    /// The committed text: an in-progress composition is not part of it.
    /// A secure editor's is its text, not the bullets it draws.
    pub fn text(&self, key: Key) -> Option<String> {
        let s = self.states.get(&key)?;
        if let Some(secret) = &s.secret {
            return Some(secret.as_str().to_string());
        }
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

    /// Returns whether the text landed in an editor: `false` says it was
    /// held for the frame that declares the key, so nothing on screen
    /// changed yet and the frame that will change it is the app's — a
    /// driver that redraws on every write would re-lower the tree that
    /// declares no editor and drop the seed to the warning.
    pub fn set_text(&mut self, key: Key, text: &str, fs: &mut FontSystem, res: &Resources) -> bool {
        let Some(s) = self.states.get_mut(&key) else {
            // Nothing has declared this key yet. The call is not wrong —
            // the `update` that opens an editor runs before the view that
            // declares it — so hold the text for the frame that does
            // rather than falling through silently.
            self.pending.insert(key, text.to_string());
            return false;
        };
        s.preedit = None;
        let a = attrs_for(&s.style, res);
        let text = admitted(text, s.multiline);
        let text = match &mut s.secret {
            Some(secret) => {
                *secret = Secret::new(text.into_owned());
                bullets(graphemes(secret.as_str())).into()
            }
            None => text,
        };
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
        s.fresh_edit = false;
        self.touch_caret(key);
        true
    }

    /// Holds `text` for the next editor declared under `label`, for a
    /// `set_edit_text` by a name nothing has declared yet.
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
    ///
    /// A composition sits in the buffer while it is under way, and it is
    /// not committed text: a place inside it reads as where it starts, and
    /// one past it as the same place with the composition taken out.
    pub fn caret_and_selection(&self, key: Key) -> Option<(usize, Option<(usize, usize)>)> {
        let s = self.states.get(&key)?;
        Some(s.editor.with_buffer(|b| {
            let composing = s
                .preedit
                .as_ref()
                .map(|p| (abs_offset(b, p.start), p.text.len()));
            let committed = |at: usize| match composing {
                Some((start, len)) if at > start => start.max(at.saturating_sub(len)),
                _ => at,
            };
            let caret = committed(abs_offset(b, s.editor.cursor()));
            let selection = s
                .editor
                .selection_bounds()
                .map(|(a, z)| (committed(abs_offset(b, a)), committed(abs_offset(b, z))))
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
                |_| (0.0, 0.0),
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

    /// Replaces the whole text with `text` as one recorded edit, as if the
    /// user had selected all and typed it: what assistive technology's
    /// `setValue` on an editor does, so a reader's edit undoes in one step
    /// as typing does, and an app's correction in the `update` it reached
    /// joins that step (ADR 0047, decision 8) — where `set_text`, the
    /// app's own replacement, clears the history. False when the text is
    /// already `text`. A secure editor replaces its secret and records
    /// nothing, as for typing.
    pub fn replace_all(&mut self, key: Key, text: &str, fs: &mut FontSystem) -> bool {
        let Some(s) = self.states.get_mut(&key) else {
            return false;
        };
        s.abandon_preedit();
        let text = admitted(text, s.multiline);
        // Compared in place: a copy of a secret would be one kui does not
        // zero.
        let same = match &s.secret {
            Some(secret) => secret.as_str() == &*text,
            None => s.editor.with_buffer(buffer_text) == *text,
        };
        if same {
            return false;
        }
        s.coalesce = None;
        s.editor.set_selection(Selection::Normal(Cursor::new(0, 0)));
        s.editor.action(fs, Action::Motion(Motion::BufferEnd));
        s.insert_recorded(&text);
        s.coalesce = None;
        s.editor.shape_as_needed(fs, false);
        s.version += 1;
        s.invalidate_measurements();
        self.touch_caret(key);
        true
    }

    /// Types `text` over the selection (or at the caret), as one undo
    /// step; true when the content changed. What a reader's
    /// `replaceSelectedText` does, so it goes in as typing goes in: what
    /// [`typed`] drops is dropped. It went in raw (backlog FZ11), so a
    /// reader put in what no keystroke, paste or declared value can - a
    /// line break in a field, and the bidi paragraph ends `one_paragraph`
    /// keeps from the shaper (FZ6) - and a value declared back from the
    /// `change` it reported was then not the text the editor held.
    pub fn replace_selection(&mut self, key: Key, text: &str, fs: &mut FontSystem) -> bool {
        let Some(s) = self.states.get_mut(&key) else {
            return false;
        };
        s.abandon_preedit();
        let text: String = text.chars().filter_map(|c| typed(c, s.multiline)).collect();
        if text.is_empty() && s.editor.selection_bounds().is_none() {
            return false;
        }
        s.insert_recorded(&text);
        s.editor.shape_as_needed(fs, false);
        s.version += 1;
        s.invalidate_measurements();
        self.touch_caret(key);
        true
    }

    /// The selected text, for a copy or a cut. `None` from a secure
    /// editor: its text does not leave it by the clipboard.
    pub fn copy_selection(&self, key: Key) -> Option<String> {
        let s = self.states.get(&key)?;
        if s.secret.is_some() {
            return None;
        }
        s.editor.copy_selection()
    }

    /// Whether a copy or a cut from editor `key` has anything to take: a
    /// selection, in an editor that is not secure. What an Edit menu
    /// lights Cut and Copy by.
    pub fn can_copy(&self, key: Key) -> bool {
        self.has_selection(key) && !self.is_secure(key)
    }

    /// Whether the editor `key` has a non-empty selection — what
    /// `copy_selection` would answer, without building the string. A
    /// menu asking whether Copy applies asks this every time it opens.
    pub fn has_selection(&self, key: Key) -> bool {
        self.states
            .get(&key)
            .and_then(|s| s.editor.selection_bounds())
            .is_some_and(|(a, b)| a != b)
    }

    /// Whether the editor `key` has an edit to undo, and one to redo.
    pub fn history(&self, key: Key) -> (bool, bool) {
        self.states
            .get(&key)
            .map_or((false, false), |s| (!s.undo.is_empty(), !s.redo.is_empty()))
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
        let filtered: String = text.chars().filter_map(|c| typed(c, s.multiline)).collect();
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
    ) -> KeyOutcome {
        let Some(s) = self.states.get_mut(&key) else {
            return KeyOutcome::default();
        };
        // Keys reaching the editor mid-composition mean the IME let them
        // through; act on committed text only.
        let mut changed = s.abandon_preedit();
        let mut submit = false;
        // Where the caret was, to tell a key that moved or deleted nothing
        // — the caret at an edge of the text — from one that did.
        let at = |e: &Editor<'static>| (e.cursor().line, e.cursor().index);
        let before = (at(&s.editor), s.editor.selection());
        // The visual line the caret is on, for ↑ and ↓: on the first or
        // last one the motion moves the caret along it (to an end of the
        // text) but not off it, and that is the edge.
        let row = |e: &Editor<'static>| e.cursor_position().map(|(_, y)| y);
        let row_before = row(&s.editor);
        let composing = changed;
        // ↑ and ↓ by paragraphs: an edge only where they could not move.
        let mut by_paragraph = false;
        match ek {
            EditKey::Left
            | EditKey::Right
            | EditKey::Up
            | EditKey::Down
            | EditKey::Home
            | EditKey::End
            | EditKey::PageUp
            | EditKey::PageDown => {
                // What the modifiers mean is each platform's fields'. On
                // a Mac ⌥ moves by words, and ⌥↑ ⌥↓ by paragraphs; ⌘ to
                // the line's ends, and ⌘↑ ⌘↓ to the text's. Elsewhere Ctrl
                // moves by words, and Ctrl+↑ Ctrl+↓ by paragraphs; Alt
                // stays a word, as it was here before.
                let mac = cfg!(target_os = "macos");
                let word = mods.word || (!mac && mods.doc);
                // A secure editor has no words to show: a word's move is
                // the line's, as a Mac's password field's is.
                let (word, line) = if s.secret.is_some() {
                    (false, (mac && mods.doc) || word)
                } else {
                    (word, mac && mods.doc)
                };
                let motion = match (ek, line, word) {
                    (EditKey::Left, true, _) => Motion::Home,
                    (EditKey::Right, true, _) => Motion::End,
                    (EditKey::Up, true, _) => Motion::BufferStart,
                    (EditKey::Down, true, _) => Motion::BufferEnd,
                    (EditKey::Left, _, true) => Motion::LeftWord,
                    (EditKey::Right, _, true) => Motion::RightWord,
                    (EditKey::Up, _, true) => Motion::ParagraphStart,
                    (EditKey::Down, _, true) => Motion::ParagraphEnd,
                    (EditKey::Left, ..) => Motion::Left,
                    (EditKey::Right, ..) => Motion::Right,
                    (EditKey::Up, ..) => Motion::Up,
                    (EditKey::Down, ..) => Motion::Down,
                    (EditKey::Home, ..) if mods.doc => Motion::BufferStart,
                    (EditKey::End, ..) if mods.doc => Motion::BufferEnd,
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
                // A paragraph's start or end the caret is already at: the
                // one before or after, as a Mac's ⌥↑ ⌥↓ go on.
                by_paragraph = matches!(motion, Motion::ParagraphStart | Motion::ParagraphEnd);
                if by_paragraph {
                    let c = s.editor.cursor();
                    let len = s
                        .editor
                        .with_buffer(|b| b.lines.get(c.line).map_or(0, |l| l.text().len()));
                    let last = s.editor.with_buffer(|b| b.lines.len().saturating_sub(1));
                    if motion == Motion::ParagraphStart && c.index == 0 && c.line > 0 {
                        s.editor.action(fs, Action::Motion(Motion::Previous));
                    } else if motion == Motion::ParagraphEnd && c.index >= len && c.line < last {
                        s.editor.action(fs, Action::Motion(Motion::Next));
                    }
                }
                s.editor.action(fs, Action::Motion(motion));
                s.break_coalesce();
            }
            EditKey::Backspace | EditKey::Delete => {
                // ⌘⌫ on a Mac takes the line back to its start and ⌘⌦ on
                // to its end, as AppKit's fields do; elsewhere the primary
                // modifier is Ctrl, and Ctrl+Backspace takes a word, as
                // Windows' and GTK's fields do.
                let (line, word) = if cfg!(target_os = "macos") {
                    (mods.doc, mods.word)
                } else {
                    (false, mods.word || mods.doc)
                };
                // A secure editor's word is its line (see the arrows).
                let (line, word) = if s.secret.is_some() {
                    (line || word, false)
                } else {
                    (line, word)
                };
                let back = ek == EditKey::Backspace;
                let motion = match (back, line, word) {
                    (true, true, _) => Motion::Home,
                    (true, _, true) => Motion::LeftWord,
                    (true, ..) => Motion::Left,
                    (false, true, _) => Motion::End,
                    (false, _, true) => Motion::RightWord,
                    (false, ..) => Motion::Right,
                };
                let kind = if back {
                    Coalesce::Backspace
                } else {
                    Coalesce::Delete
                };
                changed |=
                    s.delete_selection_recorded() || s.delete_motion_recorded(motion, kind, fs);
            }
            EditKey::Enter => {
                if s.multiline {
                    s.insert_recorded("\n");
                    changed = true;
                } else {
                    submit = true;
                }
            }
            // Only an editor whose Tab is its own types with one (the core
            // walked the ring for the rest before it got here): Tab
            // indents, and Shift-Tab takes an indent back rather than
            // typing one.
            EditKey::Tab => {
                if s.types_tab() {
                    if mods.shift {
                        changed |= s.outdent_recorded();
                    } else {
                        s.insert_recorded(TAB_INDENT);
                        changed = true;
                    }
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
        // A key the editor holds but could not act on (backlog F145): an
        // arrow, Backspace or Delete that left the caret where it was and
        // changed nothing, from a caret with no selection — the caret was
        // at the edge of the text the key moves toward. Not under Shift,
        // which extends a selection and has no edge to report, nor
        // mid-composition, whose key the IME let through.
        let plain = !changed && !composing && !mods.shift && before.1 == Selection::None;
        let still = plain && (at(&s.editor), s.editor.selection()) == before;
        // ↑ and ↓ meet the edge when they leave the caret on its visual
        // line, wherever along it they moved it: a field's ↓ from the
        // middle of its one line is the end of the field, not a caret
        // walked to the end first and an edge on the second press.
        let same_row = plain && row_before.is_some() && row(&s.editor) == row_before;
        let boundary = match ek {
            EditKey::Up if by_paragraph && still => Some(Edge::Start),
            EditKey::Down if by_paragraph && still => Some(Edge::End),
            EditKey::Up | EditKey::Down if by_paragraph => None,
            EditKey::Up if same_row => Some(Edge::Start),
            EditKey::Down if same_row => Some(Edge::End),
            EditKey::Left | EditKey::Backspace if still => Some(Edge::Start),
            EditKey::Right | EditKey::Delete if still => Some(Edge::End),
            _ => None,
        };
        if changed {
            s.version += 1;
            s.invalidate_measurements();
        }
        self.touch_caret(key);
        KeyOutcome {
            changed,
            submit,
            boundary,
        }
    }

    /// Mouse press inside the edit at content-local logical position.
    /// `clicks` is the driver-counted multi-click: 2 selects the word,
    /// 3 the line (cosmic-text's double/triple click actions). With
    /// `extend` — a Shift-press — the caret moves there keeping the
    /// selection's anchor, seeding one at the caret when there is none
    /// (cosmic-text's `Drag` does both), and the
    /// click count says nothing. Either way it is a click: a live
    /// composition is abandoned and the next edit starts an undo unit.
    pub(crate) fn click(
        &mut self,
        key: Key,
        local: Vec2,
        clicks: u8,
        extend: bool,
        fs: &mut FontSystem,
    ) {
        if let Some(s) = self.states.get_mut(&key) {
            if s.abandon_preedit() {
                s.version += 1;
                s.invalidate_measurements();
            }
            let (x, y) = ((local.x * s.scale) as i32, (local.y * s.scale) as i32);
            let action = match (extend, clicks) {
                (true, _) => Action::Drag { x, y },
                (false, 0 | 1) => Action::Click { x, y },
                // A secure editor's word is its text (see the arrows).
                (false, 2) if s.secret.is_none() => Action::DoubleClick { x, y },
                (false, _) => Action::TripleClick { x, y },
            };
            s.editor.action(fs, action);
            if extend {
                no_empty_selection(&mut s.editor);
            }
            s.editor.shape_as_needed(fs, false);
            s.break_coalesce();
            self.caret_stamp += 1;
        }
    }

    /// Mouse motion with the button held: moves the caret to the point
    /// and keeps the selection's anchor (seeding one at the caret when
    /// there is none — cosmic-text's `Drag` does both). The caret moved,
    /// so it is marked like a keyboard motion's: a field scrolls its own
    /// text toward the drag at once, and a scroller above a document
    /// reveals the caret on the release — not under the held pointer,
    /// where the drag's own rate is what moves it
    /// (`scroll_caret_into_view`).
    pub(crate) fn drag(&mut self, key: Key, local: Vec2, fs: &mut FontSystem) {
        if let Some(s) = self.states.get_mut(&key) {
            let (x, y) = ((local.x * s.scale) as i32, (local.y * s.scale) as i32);
            let before = s.editor.cursor();
            s.editor.action(fs, Action::Drag { x, y });
            no_empty_selection(&mut s.editor);
            if s.editor.cursor() != before {
                self.touch_caret(key);
            } else {
                self.caret_stamp += 1;
            }
        }
    }

    /// Replaces the focused editor's IME composition. The text lives in
    /// the buffer from the moment it appears — following text shifts and
    /// the paragraph rewraps — but never in the undo history; the caret
    /// sits at the IME-reported offset inside it. Empty text cancels
    /// (winit sends that before every commit). Returns true when the
    /// committed text changed — a composition begun over a selection
    /// deletes it, a real edit the app hears as a `change` then, so a
    /// composition abandoned after it (a blur, an empty preedit) leaves
    /// the editor and the app agreeing (backlog FZ17).
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
        // No input method composes into a secure editor: the window has
        // it off while one has the keyboard, and a host that sends a
        // composition anyway is not drawn one.
        if s.secret.is_some() {
            return false;
        }
        let (text, cursor) = composed(text, cursor, s.multiline);
        let text = &*text;
        let mut committed = false;
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
                    committed = s.delete_selection_recorded();
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
        committed
    }

    /// Drops the selection of one editor, leaving the caret where the
    /// selection's live end was. What a selection started elsewhere in
    /// the window calls, so no window ever shows two selections.
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

    /// Whether Tab in `key` is the app's rather than the focus ring's: an
    /// editor declared with `keep_tab`.
    pub fn keeps_tab(&self, key: Key) -> bool {
        self.states.get(&key).is_some_and(|s| s.keep_tab)
    }

    /// Whether Tab in `key` types indentation by its declaration: a
    /// multiline editor that declares neither `tab_moves_focus` nor
    /// `keep_tab`. The core still walks the ring from one when the
    /// keyboard asks for a way out — an armed Escape, the window's
    /// toggle, a modifier (`Core::route_input`).
    pub fn inserts_tab(&self, key: Key) -> bool {
        self.states.get(&key).is_some_and(EditState::types_tab)
    }

    /// Whether `key` lays its text out to its box's width: a document, or
    /// a field with `wrap` declared. What emission asks
    /// before narrowing a field's clip — an editor that folds never
    /// scrolls, so it keeps the node's.
    pub(crate) fn folds(&self, key: Key) -> bool {
        self.states.get(&key).is_some_and(|s| s.folds)
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

    /// The caret of `key` in logical px from the start of its text, as laid
    /// out and before a field's scroll: zero wide, one line tall.
    pub(crate) fn caret_local(&mut self, key: Key, fs: &mut FontSystem) -> Option<Rect> {
        let s = self.states.get_mut(&key)?;
        s.editor.shape_as_needed(fs, false);
        let (x, y) = s.editor.cursor_position()?;
        let line_height = s.editor.with_buffer(|b| b.metrics().line_height);
        Some(Rect::new(
            x as f32 / s.scale,
            y as f32 / s.scale,
            0.0,
            line_height / s.scale,
        ))
    }

    /// How far a single-line field's text is scrolled left, in physical
    /// px, for a content box `inner_w` wide (physical too). 0 for an
    /// editor that folds — a document, or a field with `wrap` — which
    /// wraps instead.
    ///
    /// A field does not wrap ([`EditStore::wrapped`]), so a value that
    /// outgrows its box is moved under the caret rather than folded onto a
    /// second line — which is what a native field does, and what an app
    /// otherwise has to fake by declaring the box wider than the text it
    /// is about to hold. Recomputed where the box is known,
    /// so a field that grows or shrinks between frames re-anchors with it.
    pub(crate) fn line_offset(&mut self, key: Key, inner_w: f32, fs: &mut FontSystem) -> f32 {
        let focused = self.focused == Some(key);
        let Some(s) = self.states.get_mut(&key) else {
            return 0.0;
        };
        if s.folds {
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
    /// character with every keystroke on its own line.
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
        // The same measurement a text node takes of itself, except that an
        // empty editor is still one line tall.
        let (w, h) = s.editor.with_buffer(|b| {
            let (size, lines) = crate::text::measure_buffer(b, 0);
            (size.w, lines.max(1) as f32 * b.metrics().line_height)
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
    /// inside a box measured for one. The exception is a
    /// field that asked to fold (`EditOptions::wrap`): it
    /// wraps to its width exactly as a document does, and keeps a field's
    /// keyboard.
    /// The first line's baseline of editor `key` as `wrapped` last laid it
    /// out, logical px below the top of its text: what a
    /// field beside its label lines up by. An empty editor is one line of
    /// its own metrics. `NaN` for a key no editor holds.
    pub(crate) fn baseline(&self, key: Key) -> f32 {
        let Some(s) = self.states.get(&key) else {
            return f32::NAN;
        };
        let b = s.editor.with_buffer(|b| {
            b.layout_runs()
                .next()
                .map_or(b.metrics().line_height * 0.8, |r| r.line_y.round())
        });
        b / s.scale
    }

    pub(crate) fn wrapped(&mut self, key: Key, max_w: f32, fs: &mut FontSystem) -> Size {
        let Some(s) = self.states.get_mut(&key) else {
            return Size::ZERO;
        };
        if !s.folds {
            // The buffer stays unwrapped (an editor that folded last frame
            // may be carrying a width), and the box is one line tall
            // whatever the text has in it — a `\n` that reached a field
            // through `set_text` does not make it two.
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
        if crate::text::wrap_differs(s.wrap, Some(target)) {
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
            let (_, lines) = crate::text::measure_buffer(b, 0);
            lines.max(1) as f32 * b.metrics().line_height
        });
        let size = Size::new(max_w, h / s.scale);
        s.measured = Some((stamp.0, stamp.1, stamp.2, size));
        size
    }

    // -- Emission

    /// Draws selection, glyphs, and caret. `origin` is the content box origin
    /// in physical px; everything emitted is clipped by `clip`. `selected`
    /// is whether the selection is drawn: an editor that lost the keyboard
    /// keeps its range for when focus comes back, and shows none of it
    /// meanwhile (backlog F147) — a page of fields would otherwise show a
    /// highlight in each, none of which ⌘C copies.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn emit(
        &mut self,
        key: Key,
        origin: Vec2,
        focused: bool,
        selected: bool,
        faint: Color,
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
        let selection = s.editor.selection_bounds().filter(|_| selected);
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
        // scrolls inside is the clip emission was handed.
        let origin = Vec2::new(origin.x - s.offset_x, origin.y);

        // What can show, in the glyphs' own space (ADR 0043).
        let vis = clip.visible();
        // An empty editor shows its placeholder where its text would start,
        // under the caret (backlog F149).
        let empty = s.preedit.is_none()
            && s.editor
                .with_buffer(|b| b.lines.len() == 1 && b.lines[0].text().is_empty());
        if empty && let Some((_, _, b)) = &s.placeholder {
            for run in b.layout_runs() {
                for glyph in run.glyphs.iter() {
                    let Some(physical) = crate::text::physical(glyph) else {
                        continue;
                    };
                    let Some(slot) =
                        crate::text::raster_glyph(physical.cache_key, fs, raster, atlas)
                    else {
                        continue;
                    };
                    out.push(Quad {
                        rect: Rect::new(
                            origin.x + physical.x as f32 + slot.left as f32,
                            origin.y + run.line_y.round() + physical.y as f32 - slot.top as f32,
                            slot.w as f32,
                            slot.h as f32,
                        ),
                        color: faint,
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
        }
        s.editor.with_buffer(|b| {
            let line_height = b.metrics().line_height;
            // Runs come in line order: skip everything above the clip and
            // stop at the first run past its bottom — a 100k-line document
            // emits only the visible screenful of quads.
            let runs = b
                .layout_runs()
                .filter(|run| origin.y + run.line_top + line_height >= vis.y)
                .take_while(|run| origin.y + run.line_top <= vis.y + vis.h);
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
                    let Some(physical) = crate::text::physical(glyph) else {
                        continue;
                    };
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

/// Whether the buffer holds exactly `text`, its lines joined by `\n` as
/// [`buffer_text`] joins them, read line by line in place: what a
/// declared value is compared by on every frame, so an equal one costs a
/// compare and no copy.
fn buffer_is(b: &Buffer, text: &str) -> bool {
    let mut rest = text;
    for (i, line) in b.lines.iter().enumerate() {
        if i > 0 {
            match rest.strip_prefix('\n') {
                Some(r) => rest = r,
                None => return false,
            }
        }
        match rest.strip_prefix(line.text()) {
            Some(r) => rest = r,
            None => return false,
        }
    }
    rest.is_empty()
}

/// `text` with every line break the buffer takes as one (`\r\n`, `\n\r`,
/// `\r`, `\n`, read as cosmic-text's line iterator reads them) spelled
/// `\n`, the way [`buffer_text`] gives the text back. Borrowed when there
/// is no `\r`, which is always but for text pasted from elsewhere.
fn as_buffered(text: &str) -> std::borrow::Cow<'_, str> {
    if !text.contains('\r') {
        return std::borrow::Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find(['\r', '\n']) {
        out.push_str(&rest[..i]);
        out.push('\n');
        let after = &rest[i..];
        let len = if after.starts_with("\r\n") || after.starts_with("\n\r") {
            2
        } else {
            1
        };
        rest = &after[len..];
    }
    out.push_str(rest);
    std::borrow::Cow::Owned(out)
}

/// How many leading bytes `a` and `b` share, compared a block at a time
/// before byte by byte: a value's splice into a long document finds its
/// span by this and [`common_suffix`].
fn common_prefix(a: &[u8], b: &[u8]) -> usize {
    const BLOCK: usize = 64;
    let blocks = a
        .as_chunks::<BLOCK>()
        .0
        .iter()
        .zip(b.as_chunks::<BLOCK>().0)
        .take_while(|(x, y)| x == y)
        .count();
    let at = blocks * BLOCK;
    at + a[at..]
        .iter()
        .zip(&b[at..])
        .take_while(|(x, y)| x == y)
        .count()
}

/// How many trailing bytes `a` and `b` share, at most `max`.
fn common_suffix(a: &[u8], b: &[u8], max: usize) -> usize {
    const BLOCK: usize = 64;
    let (a, b) = (&a[a.len() - max..], &b[b.len() - max..]);
    let blocks = a
        .as_rchunks::<BLOCK>()
        .1
        .iter()
        .rev()
        .zip(b.as_rchunks::<BLOCK>().1.iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    let at = blocks * BLOCK;
    at + a[..a.len() - at]
        .iter()
        .rev()
        .zip(b[..b.len() - at].iter().rev())
        .take_while(|(x, y)| x == y)
        .count()
}

/// Whether `at` is a grapheme boundary of `text`.
fn grapheme_boundary(text: &str, at: usize) -> bool {
    use unicode_segmentation::GraphemeCursor;
    text.is_char_boundary(at)
        && GraphemeCursor::new(at, text.len(), true)
            .is_boundary(text, 0)
            .unwrap_or(true)
}

/// The span a declared value differs from an editor's text by, as
/// `(start, old_end, new_end)`: `old[start..old_end]` is what goes and
/// `new[start..new_end]` what comes in. The text before `start` and after
/// each end is what the two share, each end on a grapheme boundary of
/// both texts, so a splice never leaves half an accent or half a flag.
fn splice_span(old: &str, new: &str) -> (usize, usize, usize) {
    let (a, b) = (old.as_bytes(), new.as_bytes());
    let mut start = common_prefix(a, b);
    let mut tail = common_suffix(a, b, a.len().min(b.len()) - start);
    while start > 0 && !(grapheme_boundary(old, start) && grapheme_boundary(new, start)) {
        start -= 1;
    }
    while tail > 0
        && !(grapheme_boundary(old, old.len() - tail) && grapheme_boundary(new, new.len() - tail))
    {
        tail -= 1;
    }
    (start, old.len() - tail, new.len() - tail)
}

/// Where an offset into an editor's text lands after a value's splice
/// (`span`, from [`splice_span`]): one before the span stays, one after
/// it moves by the length the text gained or lost, and one inside it goes
/// to the end of what came in. An offset at the span's end is after it,
/// so a caret where text is inserted ends up past it, as typing leaves it.
fn moved(at: usize, (start, old_end, new_end): (usize, usize, usize)) -> usize {
    if at >= old_end {
        at - old_end + new_end
    } else if at <= start {
        at
    } else {
        new_end
    }
}

/// Whether the selection `a..z` meets a splice's span: it covers some of
/// what goes, or, for a span that only inserts, the place it goes in.
fn overlaps(a: usize, z: usize, (start, old_end, _): (usize, usize, usize)) -> bool {
    if old_end > start {
        a < old_end && z > start
    } else {
        a < start && z > start
    }
}

/// The buffer cursor at byte `at` of `text`, the text of a buffer whose
/// lines are joined by `\n`.
fn cursor_in(text: &str, at: usize) -> Cursor {
    let head = &text[..at];
    match head.rfind('\n') {
        Some(i) => Cursor::new(head.bytes().filter(|b| *b == b'\n').count(), at - i - 1),
        None => Cursor::new(0, at),
    }
}

#[cfg(test)]
mod secret_tests {
    use super::{Secret, wipe};

    /// `wipe` zeroes the whole allocation, the spare capacity a shrinking
    /// edit left the old bytes in as well as the text (backlog F177).
    #[test]
    fn a_wipe_zeroes_the_allocation_and_not_only_the_text() {
        let mut s = String::with_capacity(64);
        s.push_str("correct horse battery staple");
        s.truncate(7);
        let (ptr, cap) = (s.as_ptr(), s.capacity());
        wipe(&mut s);
        assert!(s.is_empty());
        assert_eq!(s.capacity(), cap, "the allocation is the same one");
        // SAFETY: every byte of the allocation was written by `wipe`.
        let bytes = unsafe { std::slice::from_raw_parts(ptr, cap) };
        assert!(bytes.iter().all(|&b| b == 0), "{bytes:?}");
    }

    /// An edit that outgrows the room moves the text to a fresh
    /// allocation, and the text reads as `String::replace_range`'s would.
    #[test]
    fn a_secret_edits_as_a_string_does() {
        let mut secret = Secret::new("hunter".to_string());
        secret.replace_range(6..6, "2");
        assert_eq!(secret.as_str(), "hunter2");
        let long = "x".repeat(100);
        secret.replace_range(0..1, &long);
        assert_eq!(secret.as_str().len(), 106);
        assert!(secret.as_str().ends_with("unter2"));
        assert_eq!(secret.into_string().len(), 106);
    }
}
