//! A component instance replayed (ADR 0060, ADR 0067): what an
//! extension's fill did last frame, pushed again without asking it.
//!
//! Every placement of a component keeps what its fill did — every node it
//! pushed, as the door it came through saw it, before hover, accent or a
//! transition's easing touched the spec; the labels and data indices
//! beside them; the components it placed inside itself, with their props;
//! and every fact of the frame it read while it ran (which node was
//! hovered, where a scroller stood, the theme and the metrics, a role or a
//! label read by name). The next placement at the same key with the same
//! props pushes the kept nodes again through the same doors (`Replayed`)
//! when nothing it read has moved, and runs the extension otherwise,
//! saying why (`FillAnswer`). An instance not placed for a frame forgets
//! what it kept.
//!
//! What a replay re-issues is the fill's *nodes*: a nested component is
//! placed again and filled or replayed by its own lights. What a replay
//! does not re-issue is anything the fill declared of the *frame* — a
//! title, a window, a frame asked for, a devtools tab — so a fill that
//! declares one is kept as not replayable, as is one that pushed a node
//! through a door the journal does not know (a `cells` grid), or whose
//! view failed. The check is by count: every node the fill pushed is
//! either in the journal, a nested fill's, or the core's own (a hover
//! hint), or the kept fill is refused whole.
//!
//! Nothing here keeps a frame: the tree is built from scratch as always,
//! laid out and emitted as always; only the extension's `view` — and a
//! binding's work between its tables and the tree — is skipped.

use std::cell::RefCell;

use super::*;
use crate::edit::EditOptions;
use crate::fragment::FragmentRef;
use crate::line::Stroke;
use crate::path::{FillRule, PathOp, Turn};
use crate::resources::{ImageId, ImageOpts};
use crate::schema::EnvFacts;
use crate::scroll::ScrollGeometry;
use crate::slot::Instance;
use crate::text::Span;

/// How a component placement was filled: replayed from what was kept, or
/// run fresh, and why. Every answer but `Replayed` left the instance
/// filled and kept, so the next frame may replay it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FillAnswer {
    /// Last frame's nodes, pushed again; the extension was not asked.
    Replayed = 0,
    /// Nothing was kept at the key: its first frame, a frame after one
    /// that did not place it, a `fresh` placement, or a `root`.
    NotKept = 1,
    /// The props differ from the kept fill's.
    Params = 2,
    /// A fact of the frame the fill read has moved since: a hover, a
    /// focus, a scroll offset, the theme or the metrics, the clock.
    Reads = 3,
    /// The kept fill cannot be replayed: it declared something of the
    /// frame beyond its nodes, pushed through a door the journal does
    /// not know, or its view failed.
    NotReplayable = 4,
    /// The component instance heard one of its own events since it was
    /// filled, so its extension's state may have moved with it: a wheel
    /// that zoomed a waterfall, a drag that moved a split. (Code 5 was
    /// the slot's `Moved`, gone with name-keyed slots, ADR 0067.)
    Heard = 6,
}

impl FillAnswer {
    /// Whether the extension was spared.
    pub fn replayed(self) -> bool {
        self == Self::Replayed
    }

    /// A short name, the one the bindings hand out: `replayed`,
    /// `not-kept`, `params`, `reads`, `not-replayable`, `heard`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Replayed => "replayed",
            Self::NotKept => "not-kept",
            Self::Params => "params",
            Self::Reads => "reads",
            Self::NotReplayable => "not-replayable",
            Self::Heard => "heard",
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
            6 => Self::Heard,
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
    /// The host's children of a group, asked for here by an instance's
    /// fill (ADR 0060): built again on replay by the host's own builder,
    /// which the frame's placement hands in.
    Children(Box<str>),
    /// A component the fill placed, with no children of its own: placed
    /// again on replay, and filled or replayed by its own lights.
    Component {
        key: Key,
        name: Box<str>,
        props: Value,
    },
    /// A contribution slot the fill placed (ADR 0067): placed again on
    /// replay, its providers asked anew.
    Slot {
        key: Key,
        name: Box<str>,
        props: Value,
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
    TabMovesFocus(bool),
    Caret(bool),
    /// The frame clock: never the same twice.
    Clock,
    Mods(crate::input::KeyMods),
    Cursor(Option<Vec2>),
    Scroll(Key, Vec2),
    ScrollGeom(Key, Option<ScrollGeometry>),
    /// A node asked for by label before anything answered to it (the
    /// fill's own, not declared yet): holds while nothing does.
    LabelMissing {
        label: Box<str>,
        origin: crate::tree::OriginId,
        ns: Key,
        guest: bool,
    },
    Layout(Key, Option<Rect>),
    TextHit(Key, Vec2, Option<crate::text::TextHit>),
    /// Whether the text under a key was cut short (`Core::text_cut`).
    TextCut(Key, bool),
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
    /// once a fill, as the palette's generation, which moves only when the
    /// palette comes out different — a palette set again to what it was
    /// is no change (backlog C64).
    Theme(u64),
    /// The sizes, read as the palette is (`Core::metrics`, `kui_metrics`,
    /// a length role by name).
    Metrics(u64),
    /// A token looked up by name: the tables' revision
    /// (`Core::tokens_rev`), so a fill that painted with `$kitPanel` runs
    /// again when an origin declares its tokens anew — an add-on's skin
    /// switched, a brand loaded.
    Tokens(u64),
    /// Whether anyone chose the accent (`Core::has_accent`).
    Accent(bool),
    /// A lazy branch's decision (ADR 0069): the branch kept is the one
    /// built, so a decision that moved runs the fill again.
    Lazy(Key, Box<crate::rules::LazySpec>, Option<bool>),
    /// Something the core does not compare (the selection's text): a
    /// fill that read it is run every frame.
    Opaque,
}

/// [`Core::note_read_once`]'s bits: the palette, the sizes, the accent's
/// being chosen.
pub(crate) const ONCE_THEME: u8 = 1;
pub(crate) const ONCE_METRICS: u8 = 2;
pub(crate) const ONCE_ACCENT: u8 = 4;
pub(crate) const ONCE_TOKENS: u8 = 8;

/// The fill being kept, while it runs.
pub(crate) struct Recording {
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
        && a.tab_moves_focus == b.tab_moves_focus
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

    /// Asks that the next fill of the instance keyed `key` be kept:
    /// `fill_instance` begins the recording when it reaches that key.
    /// Nothing happens while a fill is already being kept — a kept fill's
    /// nested instances are filled plainly.
    pub(crate) fn keep_next_instance(&mut self, key: Key, props: &Value) {
        if self.recording.is_some() {
            return;
        }
        self.keep_next = Some((key, props.clone()));
    }

    /// Nothing filled the instance asked for: the ask lapses.
    pub(crate) fn forget_next_fill(&mut self) {
        self.keep_next = None;
    }

    /// `fill_within`'s first act: the recording begins if this is the
    /// fill asked for.
    pub(crate) fn begin_keeping(&mut self, slot: &Instance<'_>, origin: OriginId) {
        let Some((key, params)) = self.keep_next.take_if(|(k, _)| *k == slot.key) else {
            return;
        };
        self.recording = Some(Recording {
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
    pub(crate) fn end_keeping(&mut self, slot: &Instance<'_>) {
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
        self.kept_instances.insert(kept.slot_key, kept);
    }

    /// The first fact the kept fill read that reads otherwise now, as
    /// words for a ledger; `None` while every one holds.
    fn first_moved(&self, reads: &[Read]) -> Option<String> {
        reads.iter().find(|r| !self.read_holds(r)).map(|r| match r {
            // A palette's every field is no ledger line.
            Read::Theme(_) => "Theme".to_owned(),
            Read::Metrics(_) => "Metrics".to_owned(),
            Read::Tokens(_) => "Tokens".to_owned(),
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
            Read::TabMovesFocus(v) => self.tab_moves_focus == *v,
            Read::Caret(v) => self.edit.blink_visible() == *v,
            Read::Clock => false,
            Read::Mods(m) => self.interaction.modifiers() == *m,
            Read::Cursor(p) => self.interaction.cursor().map(|c| c.minus(self.dt_shift())) == *p,
            Read::Scroll(k, v) => self.scroll.offset(*k) == *v,
            Read::ScrollGeom(k, g) => self.scroll_geometry(*k) == *g,
            Read::LabelMissing {
                label,
                origin,
                ns,
                guest,
            } => !self.label_answers(label, *origin, *ns, *guest),
            Read::Layout(k, r) => self.layout_of_raw(*k) == *r,
            Read::TextHit(k, p, h) => self.text_hit_raw(*k, *p) == *h,
            Read::TextCut(k, v) => self.text.cut(*k, self.building) == *v,
            Read::EditText(k, s) => self.edit.text(*k) == *s,
            Read::Env(f) => env_same(f, &self.env_facts_raw()),
            Read::Measure(rev) => self.text_rev() == *rev,
            Read::Theme(g) => *g == self.theme_gen,
            Read::Metrics(g) => *g == self.metrics_gen,
            Read::Tokens(rev) => *rev == self.tokens_rev,
            Read::Accent(a) => self.has_accent_raw() == *a,
            Read::Lazy(k, spec, built) => self.lazy_decision(*k, spec) == *built,
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

    /// Whether the instance at `key`, placed with `props`, can be
    /// replayed: the kept fill comes out when it can, the reason stays
    /// when it cannot (and the kept fill is dropped, since the fresh fill
    /// that follows replaces it).
    pub(crate) fn take_replayable_instance(
        &mut self,
        key: Key,
        props: &Value,
    ) -> Result<Kept, (FillAnswer, Option<String>)> {
        let Some(kept) = self.kept_instances.remove(&key) else {
            return Err((FillAnswer::NotKept, None));
        };
        if let Some(why) = kept.taint {
            return Err((FillAnswer::NotReplayable, Some(why.to_owned())));
        }
        if kept.params != *props {
            return Err((FillAnswer::Params, None));
        }
        if let Some(moved) = self.first_moved(&kept.reads) {
            return Err((FillAnswer::Reads, Some(moved)));
        }
        Ok(kept)
    }

    /// Pushes the kept fill's nodes again as the fill of `slot`, under
    /// the origin that made them, `filler` answering the components it
    /// places inside and the host's `children` building their groups
    /// again where the kept fill asked for them; then keeps it for the
    /// frame after.
    pub(crate) fn replay_instance(
        &mut self,
        slot: &Instance<'_>,
        mut kept: Kept,
        filler: Option<&mut dyn crate::slot::Fill>,
        children: &mut dyn crate::slot::Children,
    ) {
        let origin = kept.origin;
        let ops = std::mem::take(&mut kept.ops);
        self.fill_instance(slot, origin, filler, children, |ui| {
            for op in &ops {
                replay_op(ui, op);
            }
        });
        kept.ops = ops;
        kept.frame = self.frame_no;
        self.kept_instances.insert(slot.key, kept);
    }

    /// At `begin_frame`: no fill is being kept yet.
    pub(crate) fn replay_begin_frame(&mut self) {
        self.recording = None;
        self.keep_next = None;
    }

    /// At `finish_frame`: an instance not placed this frame forgets what
    /// it kept, as its props were never retained either.
    pub(crate) fn replay_finish_frame(&mut self) {
        let now = self.frame_no;
        self.kept_instances.retain(|_, k| k.frame == now);
    }

    /// How many nodes the instance at `key` kept, if it kept any.
    pub fn instance_kept_nodes(&self, key: Key) -> Option<usize> {
        self.kept_instances.get(&key).map(Kept::nodes)
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
        Op::Children(group) => ui.children(group),
        Op::Component { key, name, props } => {
            ui.component_at(*key, name, props);
        }
        Op::Slot { key, name, props } => {
            ui.slot_at(Some(*key), name, props);
        }
    }
}
