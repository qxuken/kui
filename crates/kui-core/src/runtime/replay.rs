//! A slot replayed by its host (ADR 0045): the host says an extension's
//! fill would come out as it did last frame, and the core pushes last
//! frame's nodes again without asking the extension.
//!
//! `Ui::slot_kept` fills a slot as `slot_with` does and *keeps* what the
//! fill did — every node it pushed, as the door it came through saw it,
//! before hover, accent or a transition's easing touched the spec; the
//! labels and data indices beside them; the slots it declared inside
//! itself, with their params; and every fact of the frame it read while
//! it ran (which node was hovered, where a scroller stood, the theme and
//! the metrics, a role read by name).
//! `Ui::slot_replay` is the host's claim that nothing *it* feeds the
//! extension has changed. The core checks everything it can see itself
//! — the slot's params, the facts the fill read, where the slot sits —
//! and either pushes the kept nodes again through the same doors
//! (`Replayed`) or runs the extension as `slot_kept` would and says
//! why (`SlotFill`). A slot not declared for a frame forgets what it
//! kept.
//!
//! What a replay re-issues is the fill's *nodes*: a nested slot is
//! declared again and whoever fills it runs fresh, so an engine's field
//! inside a script's pane blinks its caret while the pane around it is
//! not rebuilt. What a replay does not re-issue is anything the fill
//! declared of the *frame* — a title, a window, a frame asked for, a
//! devtools tab — so a fill that declares one is kept as not replayable
//! (it is a fill whose next frame is its own business), as is one that
//! pushed a node through a door the journal does not know (a `cells`
//! grid), or whose view failed. The check is by count: every node the
//! fill pushed is either in the journal, a nested fill's, or the core's
//! own (a hover hint), or the kept fill is refused whole.
//!
//! Nothing here keeps a frame: the tree is built from scratch as always,
//! laid out and emitted as always; only the extension's `view` — and a
//! binding's work between its tables and the tree — is skipped, and only
//! when the host asked and the core found nothing it read moved.

use std::cell::RefCell;

use super::*;
use crate::edit::EditOptions;
use crate::fragment::FragmentRef;
use crate::line::Stroke;
use crate::path::{FillRule, PathOp, Turn};
use crate::resources::{ImageId, ImageOpts};
use crate::schema::EnvFacts;
use crate::scroll::ScrollGeometry;
use crate::slot::Slot;
use crate::text::Span;

/// How `Ui::slot_replay` filled a slot: replayed from what was kept, or
/// run fresh, and why. Every answer but `Replayed` left the slot filled
/// and kept as `slot_kept` would, so the next frame may replay it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotFill {
    /// Last frame's nodes, pushed again; the extension was not asked.
    Replayed,
    /// Nothing was kept for the name: its first frame, a frame after
    /// one that did not declare it, or one that filled it with
    /// `slot_with`.
    NotKept,
    /// The params differ from the kept fill's.
    Params,
    /// A fact of the frame the fill read has moved since: a hover, a
    /// focus, a scroll offset, the theme or the metrics, the clock.
    Reads,
    /// The kept fill cannot be replayed: it declared something of the
    /// frame beyond its nodes, pushed through a door the journal does
    /// not know, or its view failed.
    NotReplayable,
    /// The slot was declared under another parent than the kept fill's,
    /// so its keys would not be the kept ones.
    Moved,
}

impl SlotFill {
    /// Whether the extension was spared.
    pub fn replayed(self) -> bool {
        self == Self::Replayed
    }

    /// A short name, the one the bindings hand out: `replayed`,
    /// `not-kept`, `params`, `reads`, `not-replayable`, `moved`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Replayed => "replayed",
            Self::NotKept => "not-kept",
            Self::Params => "params",
            Self::Reads => "reads",
            Self::NotReplayable => "not-replayable",
            Self::Moved => "moved",
        }
    }

    /// A code for the C side: 0 replayed, then the others in order.
    pub fn code(self) -> i32 {
        self as i32
    }

    /// The inverse of [`Self::code`].
    pub fn from_code(code: i32) -> Option<Self> {
        Some(match code {
            0 => Self::Replayed,
            1 => Self::NotKept,
            2 => Self::Params,
            3 => Self::Reads,
            4 => Self::NotReplayable,
            5 => Self::Moved,
            _ => return None,
        })
    }
}

/// One thing a kept fill did to the tree, as the door saw it.
pub(crate) enum Op {
    /// A box opened; a `Close` ends it.
    Open {
        key: Key,
        spec: Box<NodeSpec>,
    },
    Close,
    Text {
        key: Key,
        content: Box<str>,
        style: TextStyle,
    },
    Rich {
        key: Key,
        /// Each span's text beside the span with its text left empty: a
        /// `Span` is `Copy` but for the borrow, and this keeps every
        /// other field it has or gains.
        spans: Vec<(Box<str>, Span<'static>)>,
        base: TextStyle,
    },
    Image {
        key: Key,
        id: ImageId,
        opts: ImageOpts,
        spec: Box<NodeSpec>,
    },
    /// A fragment opened; a `Close` ends it.
    Fragment {
        key: Key,
        frag: FragmentRef,
        params: Vec<f32>,
        spec: Box<NodeSpec>,
    },
    Line {
        key: Key,
        points: Vec<Vec2>,
        stroke: Stroke,
        spec: Box<NodeSpec>,
    },
    Polygon {
        key: Key,
        points: Vec<Vec2>,
        spec: Box<NodeSpec>,
    },
    Path {
        key: Key,
        ops: Vec<PathOp>,
        rule: FillRule,
        stroke: Option<Stroke>,
        turn: Option<Turn>,
        spec: Box<NodeSpec>,
    },
    Edit {
        label: Box<str>,
        initial: Box<str>,
        opts: Box<EditOptions>,
        spec: Box<NodeSpec>,
    },
    /// The node under `key` is named `label` for `key_of`.
    Label {
        key: Key,
        label: Box<str>,
    },
    /// The node pushed last is at data index `i`.
    Indexed(u64),
    /// The node pushed last holds this many virtual rows (a prop list's).
    RowCount(u64),
    /// The node open now holds this many virtual rows (`Core::row_count`).
    RowCountOpen(u64),
    Hint {
        key: Key,
        text: Box<str>,
    },
    KeyFocus(Key),
    /// A slot declared inside the fill: declared again on replay, and
    /// filled fresh by whoever fills it.
    Slot {
        name: Box<str>,
        params: Value,
    },
}

/// A fact of the frame a kept fill read, with what it read: the replay
/// holds only while every one reads the same.
#[derive(Debug)]
pub(crate) enum Read {
    Hover(Key, bool),
    Pressed(Key, bool),
    Drop(Key, bool),
    GroupHover(u64, bool),
    GroupPressed(u64, bool),
    Focus(Option<Key>),
    Focused(Key, bool),
    FocusVisible(bool),
    Caret(bool),
    /// The frame clock: never the same twice.
    Clock,
    Mods(crate::input::KeyMods),
    Cursor(Option<Vec2>),
    Scroll(Key, Vec2),
    ScrollGeom(Key, Option<ScrollGeometry>),
    Layout(Key, Option<Rect>),
    TextHit(Key, Vec2, Option<crate::text::TextHit>),
    EditText(Key, Option<String>),
    /// The env reading handed out whole, less the clock and the caret
    /// phase it carries (see [`env_same`]).
    Env(EnvFacts),
    /// Text was measured: the same while the fonts are.
    Measure(u64),
    /// The palette, read whole (`Core::theme`, and so `Ui::theme`, C's
    /// `kui_theme`, the bindings' theme tables) or a role by name through
    /// a token lookup: a colour drawn from it is baked into the spec the
    /// journal keeps, so a palette that differs runs the fill again. Noted
    /// once a fill, compared by value — a palette set again to what it
    /// was is no change. Boxed, so the other reads stay small.
    Theme(Box<crate::theme::Theme>),
    /// The sizes, read as the palette is (`Core::metrics`, `kui_metrics`,
    /// a length role by name).
    Metrics(Box<crate::metrics::Metrics>),
    /// Whether anyone chose the accent (`Core::has_accent`).
    Accent(bool),
    /// Something the core does not compare (the selection's text): a
    /// fill that read it is run every frame.
    Opaque,
}

/// [`Core::note_read_once`]'s bits: the palette, the sizes, the accent's
/// being chosen.
pub(crate) const ONCE_THEME: u8 = 1;
pub(crate) const ONCE_METRICS: u8 = 2;
pub(crate) const ONCE_ACCENT: u8 = 4;

/// The fill being kept, while it runs.
pub(crate) struct Recording {
    name: String,
    slot_key: Key,
    origin: OriginId,
    params: Value,
    ops: Vec<Op>,
    /// Through `&self` doors (`is_hovered`), so a cell.
    reads: RefCell<Vec<Read>>,
    /// The reads noted once a fill, as [`ONCE_THEME`] bits: the palette
    /// and the sizes are read by every stock widget, and one note of each
    /// is all a comparison needs.
    once: std::cell::Cell<u8>,
    taint: Option<&'static str>,
    /// The tree's length when the fill began.
    first: u32,
    /// Node ops journaled.
    nodes: u32,
    /// Nodes pushed while paused: a nested fill's, a hover hint's.
    foreign: u32,
    paused: u32,
    pause_from: u32,
}

/// What a slot kept, between frames.
pub(crate) struct Kept {
    slot_key: Key,
    origin: OriginId,
    params: Value,
    ops: Vec<Op>,
    reads: Vec<Read>,
    taint: Option<&'static str>,
    /// The frame it was kept or replayed in; one a frame behind at
    /// `finish_frame` is forgotten.
    frame: u64,
}

impl Kept {
    /// How many nodes a replay pushes, for a reader.
    pub(crate) fn nodes(&self) -> usize {
        self.ops
            .iter()
            .filter(|op| {
                matches!(
                    op,
                    Op::Open { .. }
                        | Op::Text { .. }
                        | Op::Rich { .. }
                        | Op::Image { .. }
                        | Op::Fragment { .. }
                        | Op::Line { .. }
                        | Op::Polygon { .. }
                        | Op::Path { .. }
                        | Op::Edit { .. }
                )
            })
            .count()
    }
}

/// Whether two env readings agree on everything but the clock and the
/// caret phase. A binding that hands the reading out whole (Lua's `env`,
/// Node's `ctx.env()`) cannot say which fields the script used, and
/// comparing those two would make every fill stale every frame; a script
/// that draws from them is one its host must not replay, and the
/// explicit doors (`Ui::now`, `Ui::caret_visible`) still note the read.
fn env_same(a: &EnvFacts, b: &EnvFacts) -> bool {
    a.env == b.env
        && a.viewport == b.viewport
        && a.scale == b.scale
        && a.focus == b.focus
        && a.focus_visible == b.focus_visible
        && a.region == b.region
}

impl Core {
    /// Whether a fill is being kept right now — what every door asks
    /// before it spends anything on the journal. One load.
    #[inline]
    pub(crate) fn keeping(&self) -> bool {
        self.recording.as_ref().is_some_and(|r| r.paused == 0)
    }

    /// Journals `op` for the fill being kept, if one is and it is not
    /// paused. Cold: the hot doors branch on [`Self::keeping`] first.
    #[cold]
    #[inline(never)]
    pub(crate) fn keep_op(&mut self, op: Op) {
        let Some(r) = self.recording.as_mut() else {
            return;
        };
        if r.paused > 0 {
            return;
        }
        if matches!(
            op,
            Op::Open { .. }
                | Op::Text { .. }
                | Op::Rich { .. }
                | Op::Image { .. }
                | Op::Fragment { .. }
                | Op::Line { .. }
                | Op::Polygon { .. }
                | Op::Path { .. }
                | Op::Edit { .. }
        ) {
            r.nodes += 1;
        }
        r.ops.push(op);
    }

    /// Notes a fact of the frame the fill being kept read.
    #[inline]
    pub(crate) fn note_read(&self, read: impl FnOnce() -> Read) {
        if let Some(r) = self.recording.as_ref()
            && r.paused == 0
        {
            r.reads.borrow_mut().push(read());
        }
    }

    /// [`Self::note_read`] for a fact read whole and often — the palette,
    /// the sizes — noted the first time the fill reads it and not again:
    /// `bit` says which, one of the `ONCE_*` bits.
    #[inline]
    pub(crate) fn note_read_once(&self, bit: u8, read: impl FnOnce() -> Read) {
        if let Some(r) = self.recording.as_ref()
            && r.paused == 0
            && r.once.get() & bit == 0
        {
            r.once.set(r.once.get() | bit);
            r.reads.borrow_mut().push(read());
        }
    }

    /// Marks the fill being kept as one that cannot be replayed, with
    /// why: it declared something of the frame beyond its nodes, or
    /// pushed through a door the journal does not know.
    pub(crate) fn taint_kept(&mut self, why: &'static str) {
        if let Some(r) = self.recording.as_mut()
            && r.taint.is_none()
        {
            r.taint = Some(why);
        }
    }

    /// Stops journaling until [`Self::resume_keeping`]: the nodes pushed
    /// between are counted as not the fill's own (a nested fill, a hover
    /// hint the core floats), so the count check at the end still
    /// balances.
    pub(crate) fn pause_keeping(&mut self) {
        let len = self.tree.len() as u32;
        if let Some(r) = self.recording.as_mut() {
            if r.paused == 0 {
                r.pause_from = len;
            }
            r.paused += 1;
        }
    }

    pub(crate) fn resume_keeping(&mut self) {
        let len = self.tree.len() as u32;
        if let Some(r) = self.recording.as_mut()
            && r.paused > 0
        {
            r.paused -= 1;
            if r.paused == 0 {
                r.foreign += len.saturating_sub(r.pause_from);
            }
        }
    }

    /// Asks that the next fill of the slot keyed `key` be kept under
    /// `name`: `fill_within` begins the recording when it reaches that
    /// slot. Nothing happens while a fill is already being kept — a kept
    /// fill's nested slots are filled plainly.
    pub(crate) fn keep_next_fill(&mut self, name: &str, key: Key, params: &Value) {
        if self.recording.is_some() {
            return;
        }
        self.keep_next = Some((name.to_owned(), key, params.clone()));
    }

    /// Nothing filled the slot asked for: the ask lapses.
    pub(crate) fn forget_next_fill(&mut self) {
        self.keep_next = None;
    }

    /// `fill_within`'s first act: the recording begins if this is the
    /// fill asked for.
    pub(crate) fn begin_keeping(&mut self, slot: &Slot<'_>, origin: OriginId) {
        let Some((name, key, params)) = self.keep_next.take_if(|(_, k, _)| *k == slot.key) else {
            return;
        };
        self.recording = Some(Recording {
            name,
            slot_key: key,
            origin,
            params,
            ops: Vec::new(),
            reads: RefCell::new(Vec::new()),
            once: std::cell::Cell::new(0),
            taint: None,
            first: self.tree.len() as u32,
            nodes: 0,
            foreign: 0,
            paused: 0,
            pause_from: 0,
        });
    }

    /// `fill_within`'s last act for the fill being kept: the journal is
    /// checked against what the tree gained and kept for the next frame.
    pub(crate) fn end_keeping(&mut self, slot: &Slot<'_>) {
        let Some(r) = self.recording.as_ref() else {
            return;
        };
        if r.slot_key != slot.key {
            return;
        }
        let mut r = self.recording.take().expect("checked");
        let pushed = (self.tree.len() as u32).saturating_sub(r.first);
        if r.paused > 0 {
            // A pause left open is a door that did not balance: not
            // this module's to repair, and not a fill to replay.
            r.taint = Some("a pause left open");
        } else if pushed.saturating_sub(r.foreign) != r.nodes {
            r.taint = Some("a node pushed through a door the journal does not know");
        }
        let kept = Kept {
            slot_key: r.slot_key,
            origin: r.origin,
            params: r.params,
            ops: r.ops,
            reads: r.reads.into_inner(),
            taint: r.taint,
            frame: self.frame_no,
        };
        self.kept.insert(r.name, kept);
    }

    /// The first fact the kept fill read that reads otherwise now, as
    /// words for a ledger; `None` while every one holds.
    fn first_moved(&self, reads: &[Read]) -> Option<String> {
        reads.iter().find(|r| !self.read_holds(r)).map(|r| match r {
            // A palette's every field is no ledger line.
            Read::Theme(_) => "Theme".to_owned(),
            Read::Metrics(_) => "Metrics".to_owned(),
            r => format!("{r:?}"),
        })
    }

    /// Whether one fact the kept fill read still reads the same.
    fn read_holds(&self, r: &Read) -> bool {
        match r {
            Read::Hover(k, v) => self.interaction.is_hovered(*k) == *v,
            Read::Pressed(k, v) => self.interaction.is_pressed(*k) == *v,
            Read::Drop(k, v) => self.interaction.is_drop_target(*k) == *v,
            Read::GroupHover(g, v) => self.interaction.is_group_hovered(*g) == *v,
            Read::GroupPressed(g, v) => self.interaction.is_group_pressed(*g) == *v,
            Read::Focus(k) => self.focus == *k,
            Read::Focused(k, v) => (self.focus == Some(*k)) == *v,
            Read::FocusVisible(v) => self.focus_visible == *v,
            Read::Caret(v) => self.edit.blink_visible() == *v,
            Read::Clock => false,
            Read::Mods(m) => self.interaction.modifiers() == *m,
            Read::Cursor(p) => self.interaction.cursor().map(|c| c.minus(self.dt_shift())) == *p,
            Read::Scroll(k, v) => self.scroll.offset(*k) == *v,
            Read::ScrollGeom(k, g) => self.scroll_geometry(*k) == *g,
            Read::Layout(k, r) => self.layout_of_raw(*k) == *r,
            Read::TextHit(k, p, h) => self.text_hit_raw(*k, *p) == *h,
            Read::EditText(k, s) => self.edit.text(*k) == *s,
            Read::Env(f) => env_same(f, &self.env_facts_raw()),
            Read::Measure(rev) => self.text_rev() == *rev,
            Read::Theme(t) => **t == self.theme,
            Read::Metrics(m) => **m == self.metrics,
            Read::Accent(a) => self.has_accent_raw() == *a,
            Read::Opaque => false,
        }
    }

    /// The fonts' and weights' revision together: what a measurement
    /// depends on beyond its string and style.
    pub(crate) fn text_rev(&self) -> u64 {
        self.fonts_rev
            .wrapping_mul(0x9E37_79B9)
            .wrapping_add(self.weights_rev)
    }

    /// Whether the slot `name`, declared at `key` with `params`, can be
    /// replayed: the kept fill comes out when it can, the reason stays
    /// when it cannot (and the kept fill is dropped, since the fresh
    /// fill that follows replaces it).
    pub(crate) fn take_replayable(
        &mut self,
        name: &str,
        key: Key,
        params: &Value,
    ) -> Result<Kept, (SlotFill, Option<String>)> {
        let Some(kept) = self.kept.remove(name) else {
            return Err((SlotFill::NotKept, None));
        };
        if kept.slot_key != key {
            return Err((SlotFill::Moved, None));
        }
        if let Some(why) = kept.taint {
            return Err((SlotFill::NotReplayable, Some(why.to_owned())));
        }
        if kept.params != *params {
            return Err((SlotFill::Params, None));
        }
        if let Some(moved) = self.first_moved(&kept.reads) {
            return Err((SlotFill::Reads, Some(moved)));
        }
        Ok(kept)
    }

    /// Pushes the kept fill's nodes again as the fill of `slot`, under
    /// the origin that made them, `filler` answering the slots it
    /// declares inside; then keeps it for the frame after.
    pub(crate) fn replay(
        &mut self,
        name: &str,
        slot: &Slot<'_>,
        mut kept: Kept,
        filler: Option<&mut dyn crate::slot::Fill>,
    ) {
        let origin = kept.origin;
        let ops = std::mem::take(&mut kept.ops);
        self.fill_within(slot, origin, filler, |ui| {
            for op in &ops {
                replay_op(ui, op);
            }
        });
        kept.ops = ops;
        kept.frame = self.frame_no;
        self.kept.insert(name.to_owned(), kept);
    }

    /// The last answer `Ui::slot_replay` gave for `name` this frame, or
    /// the frame before while this one is being built — for a host's
    /// own ledger of what its panes cost, and for a test.
    pub fn slot_fill(&self, name: &str) -> Option<SlotFill> {
        self.find_slot_fill(name).map(|(f, _)| f)
    }

    /// Beside [`Self::slot_fill`]: for `Reads`, the fact that moved as
    /// words (`Hover(Key(…), false)`), for `NotReplayable` what the
    /// fill declared or drew; `None` for the other answers.
    pub fn slot_fill_why(&self, name: &str) -> Option<&str> {
        self.find_slot_fill(name).and_then(|(_, why)| why)
    }

    fn find_slot_fill(&self, name: &str) -> Option<(SlotFill, Option<&str>)> {
        // This frame's answers first, newest first, then the last frame's.
        self.slot_fills
            .iter()
            .rev()
            .chain(self.slot_fills_last.iter().rev())
            .find(|(n, ..)| n == name)
            .map(|(_, f, w)| (*f, w.as_deref()))
    }

    pub(crate) fn note_slot_fill(&mut self, name: &str, fill: SlotFill, why: Option<String>) {
        self.slot_fills.push((name.to_owned(), fill, why));
    }

    /// How many nodes the kept fill of `name` holds, if one is kept.
    pub fn slot_kept_nodes(&self, name: &str) -> Option<usize> {
        self.kept.get(name).map(Kept::nodes)
    }

    /// At `begin_frame`: the fills a frame answered are the frame's.
    pub(crate) fn replay_begin_frame(&mut self) {
        // This frame's answers so far are the last frame's, and a view
        // that asks before its slot is declared reads those.
        self.slot_fills_last = std::mem::take(&mut self.slot_fills);
        self.recording = None;
        self.keep_next = None;
    }

    /// At `finish_frame`: a slot not declared this frame forgets what it
    /// kept, as its params were never retained either.
    pub(crate) fn replay_finish_frame(&mut self) {
        let now = self.frame_no;
        self.kept.retain(|_, k| k.frame == now);
    }
}

/// One journaled op, through the door it came in by — the node under the
/// key it had, the spec as it was declared, so hover, accent and easing
/// are resolved for this frame as a fresh fill's would be.
fn replay_op(ui: &mut Ui<'_>, op: &Op) {
    let core = ui.core();
    match op {
        Op::Open { key, spec } => core.open_with_key(*key, (**spec).clone()),
        Op::Close => core.close(),
        Op::Text {
            key,
            content,
            style,
        } => core.text_with_key(*key, content, *style),
        Op::Rich { key, spans, base } => {
            let spans: Vec<Span<'_>> = spans.iter().map(|(text, s)| Span { text, ..*s }).collect();
            core.rich_text_with_key(*key, &spans, *base);
        }
        Op::Image {
            key,
            id,
            opts,
            spec,
        } => core.image_with_key(*key, *id, *opts, (**spec).clone()),
        Op::Fragment {
            key,
            frag,
            params,
            spec,
        } => core.fragment_with_key(*key, *frag, params, (**spec).clone()),
        Op::Line {
            key,
            points,
            stroke,
            spec,
        } => core.line_with_key(*key, points, stroke, (**spec).clone()),
        Op::Polygon { key, points, spec } => {
            core.polygon_with_key(*key, points, (**spec).clone());
        }
        Op::Path {
            key,
            ops,
            rule,
            stroke,
            turn,
            spec,
        } => core.path_with_key(*key, ops, *rule, *stroke, *turn, (**spec).clone()),
        Op::Edit {
            label,
            initial,
            opts,
            spec,
        } => {
            core.text_edit(label, initial, opts, (**spec).clone());
        }
        Op::Label { key, label } => core.note_label(*key, label),
        Op::Indexed(i) => {
            if let Some(at) = core.tree.len().checked_sub(1) {
                core.tree.indexed.push((at as u32, *i));
            }
        }
        Op::RowCount(n) => {
            if let Some(at) = core.tree.len().checked_sub(1) {
                core.tree.row_counts.push((at as u32, *n));
            }
        }
        Op::RowCountOpen(n) => {
            let at = core.current();
            core.tree.row_counts.push((at, *n));
        }
        Op::Hint { key, text } => core.hint(*key, &**text),
        Op::KeyFocus(key) => core.set_key_focus(Some(*key)),
        Op::Slot { name, params } => {
            ui.slot_with(name, params);
        }
    }
}
