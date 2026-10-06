//! Why a frame was drawn: two readings a view can take of
//! the frame it is building, for an app that keeps a ledger of its frames
//! and wants to know why one ran with nothing on screen moving.
//!
//! - [`FrameCause`], the set of reasons that reached the window since the
//!   last frame began. The core records the input it was handed
//!   ([`Core::handle_input`]) and whether the last frame left one owed;
//!   the driver adds what only it sees ([`Core::note_frame_cause`]) — a
//!   wake, a resize, the caret's blink, a surface retry, the OS events
//!   that never become input. Always kept: it is a few ORs a frame.
//! - [`OwedBy`], *who* holds a frame the last one left owed: the nodes
//!   whose transition is mid-flight, the cycles running, the departures
//!   playing, the containers easing a scroll, the held drag's scroller,
//!   the `animate` nodes, and where `request_frame` was called from.
//!   [`Core::owed`] with names. Off unless [`Core::set_frame_trace`] asked
//!   for it, and taken at the start of `begin_frame` — the only moment the
//!   last frame's tree, which names the holders, and the stores' state,
//!   which says who they are, are both still there.
//!
//! The same switch keeps a digest of every finished frame's display list,
//! so [`Core::frame_unchanged`] says whether a frame drew exactly what the
//! one before it drew: a frame that changed nothing on screen.

use std::panic::Location;

use rustc_hash::FxHashMap;

use super::*;
use crate::input::InputEvent;

/// Why a frame was drawn: every reason that reached the window between
/// the start of the last frame and the start of this one.
/// Read from a view as [`Core::frame_cause`]. A set, because a frame
/// answers everything that asked since the last one — a keystroke and the
/// caret's blink, a wheel and the transition it started.
///
/// Empty is a frame nobody here asked for: the platform's own redraw (an
/// expose, a live resize the OS paints through), or a driver that notes
/// nothing.
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct FrameCause(u32);

impl FrameCause {
    /// Nothing recorded.
    pub const NONE: FrameCause = FrameCause(0);

    // The input the core was handed (`Core::handle_input`), by kind —
    // every driver's, recorded by the core itself.
    /// The pointer moved over the window.
    pub const POINTER_MOVE: FrameCause = FrameCause(1 << 0);
    /// The pointer left the window.
    pub const POINTER_LEAVE: FrameCause = FrameCause(1 << 1);
    /// A mouse button went down or up, or a force click.
    pub const BUTTON: FrameCause = FrameCause(1 << 2);
    /// A wheel or a trackpad scroll — a swipe's momentum included, which
    /// the platform sends as scrolls after the hand has left.
    pub const WHEEL: FrameCause = FrameCause(1 << 3);
    /// A key went down or up.
    pub const KEY: FrameCause = FrameCause(1 << 4);
    /// The modifier keys changed.
    pub const MODIFIERS: FrameCause = FrameCause(1 << 5);
    /// Text arrived: typed, committed by an input method, or pasted.
    pub const TEXT: FrameCause = FrameCause(1 << 6);
    /// An input method's composition changed.
    pub const PREEDIT: FrameCause = FrameCause(1 << 7);
    /// Assistive technology asked for an action.
    pub const ACCESS: FrameCause = FrameCause(1 << 8);
    /// Files dragged over the window, dropped on it, or taken away.
    pub const FILE_DRAG: FrameCause = FrameCause(1 << 9);
    /// A file dialog answered, or the OS handed the app documents to open
    /// (`InputEvent::Open`, backlog F124).
    pub const FILES: FrameCause = FrameCause(1 << 10);

    // What the driver saw (`Core::note_frame_cause`).
    /// The window's first frame.
    pub const FIRST: FrameCause = FrameCause(1 << 11);
    /// The app woke the loop from another thread (a `Waker`).
    pub const WAKE: FrameCause = FrameCause(1 << 12);
    /// The host asked for a frame: a pumped runner's `request_redraw`,
    /// a new view handed over.
    pub const HOST: FrameCause = FrameCause(1 << 13);
    /// The window changed size.
    pub const RESIZE: FrameCause = FrameCause(1 << 14);
    /// The window moved to a display of another scale.
    pub const SCALE: FrameCause = FrameCause(1 << 15);
    /// The window gained or lost the keyboard.
    pub const FOCUS: FrameCause = FrameCause(1 << 16);
    /// The window was uncovered, or came back from minimized. The runner
    /// leaves out of that frame what it noted while the window could not
    /// draw and no frame took — another window's input, an appearance
    /// change — so a restore does not report reasons from long before;
    /// `resize` is the same edge on Windows, where
    /// restoring is a resize.
    pub const OCCLUSION: FrameCause = FrameCause(1 << 17);
    /// The system's appearance or settings changed: light or dark, the
    /// accent, reduced motion, assistive technology coming or going, a
    /// font installed or removed.
    pub const APPEARANCE: FrameCause = FrameCause(1 << 18);
    /// The caret's blink changed phase.
    pub const CARET: FrameCause = FrameCause(1 << 19);
    /// The last frame's surface refused it (skipped, out of date,
    /// misconfigured) and the frame is tried again.
    pub const RETRY: FrameCause = FrameCause(1 << 20);
    /// A frame held for the display waited too long and is drawn anyway.
    pub const OVERDUE: FrameCause = FrameCause(1 << 21);
    /// The graphics device was reopened.
    pub const DEVICE: FrameCause = FrameCause(1 << 22);
    /// The last frame's own events — a resize, what a new layout put
    /// under a still pointer — were routed after it, and answered.
    pub const AFTER_FRAME: FrameCause = FrameCause(1 << 23);
    /// Something that reached the app in another window, which can change
    /// what this one shows.
    pub const ELSEWHERE: FrameCause = FrameCause(1 << 24);
    /// A menu (the platform's context menu or menu bar) was answered.
    pub const MENU: FrameCause = FrameCause(1 << 25);
    /// A sound finished or was refused, and its event was routed.
    pub const AUDIO: FrameCause = FrameCause(1 << 26);
    /// A smoke run's frame counter asked for the next frame.
    pub const SMOKE: FrameCause = FrameCause(1 << 27);

    // The core's own.
    /// The last frame left one owed ([`Core::owed`]); [`Core::owed_by`]
    /// says who, when traced.
    pub const OWED: FrameCause = FrameCause(1 << 28);

    /// Each reason and its name, in bit order.
    pub const ALL: [(FrameCause, &'static str); 29] = [
        (Self::POINTER_MOVE, "pointerMove"),
        (Self::POINTER_LEAVE, "pointerLeave"),
        (Self::BUTTON, "button"),
        (Self::WHEEL, "wheel"),
        (Self::KEY, "key"),
        (Self::MODIFIERS, "modifiers"),
        (Self::TEXT, "text"),
        (Self::PREEDIT, "preedit"),
        (Self::ACCESS, "access"),
        (Self::FILE_DRAG, "fileDrag"),
        (Self::FILES, "files"),
        (Self::FIRST, "first"),
        (Self::WAKE, "wake"),
        (Self::HOST, "host"),
        (Self::RESIZE, "resize"),
        (Self::SCALE, "scale"),
        (Self::FOCUS, "focus"),
        (Self::OCCLUSION, "occlusion"),
        (Self::APPEARANCE, "appearance"),
        (Self::CARET, "caret"),
        (Self::RETRY, "retry"),
        (Self::OVERDUE, "overdue"),
        (Self::DEVICE, "device"),
        (Self::AFTER_FRAME, "afterFrame"),
        (Self::ELSEWHERE, "elsewhere"),
        (Self::MENU, "menu"),
        (Self::AUDIO, "audio"),
        (Self::SMOKE, "smoke"),
        (Self::OWED, "owed"),
    ];

    /// The bits, for a binding or a log.
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// The set these bits spell; bits no reason has are dropped.
    pub const fn from_bits(bits: u32) -> FrameCause {
        FrameCause(bits & ((1 << Self::ALL.len()) - 1))
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Whether every reason in `other` is in this set.
    pub const fn contains(self, other: FrameCause) -> bool {
        self.0 & other.0 == other.0
    }

    /// Whether any reason in `other` is in this set.
    pub const fn intersects(self, other: FrameCause) -> bool {
        self.0 & other.0 != 0
    }

    pub const fn union(self, other: FrameCause) -> FrameCause {
        FrameCause(self.0 | other.0)
    }

    /// The names of the reasons in the set, in bit order.
    pub fn names(self) -> impl Iterator<Item = &'static str> {
        Self::ALL
            .into_iter()
            .filter(move |(c, _)| self.contains(*c))
            .map(|(_, name)| name)
    }

    /// The reason one input is: what [`Core::handle_input`] records, and
    /// what a driver that screens input before the core sees it can note
    /// for an event it keeps.
    pub fn of_input(ev: &InputEvent) -> FrameCause {
        match ev {
            InputEvent::CursorMoved(_) => Self::POINTER_MOVE,
            InputEvent::CursorLeft => Self::POINTER_LEAVE,
            InputEvent::MouseDown { .. } | InputEvent::MouseUp { .. } => Self::BUTTON,
            InputEvent::ForceClick(_) => Self::BUTTON,
            InputEvent::Scroll(_) | InputEvent::ScrollGesture { .. } => Self::WHEEL,
            InputEvent::Key(..) | InputEvent::KeyDown(_) | InputEvent::KeyUp(_) => Self::KEY,
            InputEvent::Modifiers(_) => Self::MODIFIERS,
            InputEvent::Text(_) | InputEvent::Commit(_) | InputEvent::Paste { .. } => Self::TEXT,
            InputEvent::Preedit(..) => Self::PREEDIT,
            InputEvent::Access(_) => Self::ACCESS,
            InputEvent::DragFiles { .. }
            | InputEvent::DropFiles { .. }
            | InputEvent::DragCancel => Self::FILE_DRAG,
            InputEvent::Files(_) | InputEvent::Open(_) => Self::FILES,
        }
    }
}

impl std::ops::BitOr for FrameCause {
    type Output = FrameCause;
    fn bitor(self, rhs: FrameCause) -> FrameCause {
        self.union(rhs)
    }
}

impl std::ops::BitOrAssign for FrameCause {
    fn bitor_assign(&mut self, rhs: FrameCause) {
        self.0 |= rhs.0;
    }
}

impl std::fmt::Debug for FrameCause {
    /// `FrameCause(key|caret)`, `FrameCause()` when empty.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FrameCause(")?;
        for (i, name) in self.names().enumerate() {
            if i > 0 {
                write!(f, "|")?;
            }
            write!(f, "{name}")?;
        }
        write!(f, ")")
    }
}

/// One thing that holds an owed frame, named for a person: see
/// [`OwedBy`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrameHolder {
    pub key: Key,
    /// The node's labels from the root, `/`-joined — the ones it and its
    /// ancestors were opened under (`open_keyed`, a `key` prop), which is
    /// the name the devtools' tree shows a node by. A node opened without
    /// one ends in its accessible name in quotes, or `#` and the low
    /// eight hex digits of its key, as the devtools' event stream writes
    /// an unlabelled node. A departure's node is in no tree, so it is its
    /// own label alone, when a frame still knows it.
    pub name: String,
    /// A transition's slots still mid-flight — `width`, `height`, `bg`,
    /// `borderColor`, `radius`, `position`, `opacity`, `shadow`,
    /// `shadowColor` — in that order. Empty for every other holder.
    pub slots: Vec<&'static str>,
}

/// Where a frame was asked for: a [`Core::request_frame`] call, or one the
/// core makes for itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameRequest {
    /// `"request_frame"` for a call to [`Core::request_frame`] or
    /// `Ui::request_frame` — the app's or a binding's, which `at` tells
    /// apart. A door that asks for the frame that lands it is named for
    /// itself, `at` its caller's line: `"reveal"`,
    /// `"reveal_label"`, `"set_scroll"`, `"set_scroll_label"`,
    /// `"focus_region"`, `"focus_region_by_label"`,
    /// `"request_focus_step"` (`Ui::focus_next`, `Ui::focus_prev`),
    /// `"request_files"`. The core's and its widgets' own asks say what
    /// they are for: `"scrollbar fade"`, `"atlas full"`,
    /// `"long line rows"`, `"resliced"`, `"devtools"`,
    /// `"list first frame"`.
    pub why: &'static str,
    /// The source line that asked (`#[track_caller]`): the app's own for
    /// its calls, a binding's for a guest's, kui's for its own asks.
    pub at: &'static Location<'static>,
}

/// Who holds the frame the last one left owed: [`Core::owed`], named.
/// Each list is empty when its kind in [`crate::Owed`] is
/// false, and names what made it true when it is.
///
/// Read with [`Core::owed_by`] from inside a view, where it describes the
/// frame before — the reason the frame being built exists — or between
/// frames, where it still does. Empty unless [`Core::set_frame_trace`]
/// turned the record on before that frame began.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OwedBy {
    /// Nodes with a transition mid-flight, each with its slots
    /// (`Owed::transition`).
    pub transitions: Vec<FrameHolder>,
    /// Nodes with a keyframe cycle running (`Owed::cycle`).
    pub cycles: Vec<FrameHolder>,
    /// Subtrees still playing their exit (`Owed::depart`).
    pub departures: Vec<FrameHolder>,
    /// Scroll containers easing a programmatic offset change
    /// (`Owed::scroll`).
    pub scrolls: Vec<FrameHolder>,
    /// The container a held drag is scrolling (`Owed::autoscroll`).
    pub autoscroll: Option<FrameHolder>,
    /// Nodes declaring `animate` (half of `Owed::requested`).
    pub animate: Vec<FrameHolder>,
    /// Where the frame was asked for (the other half of
    /// `Owed::requested`), one per calling line, in the order first
    /// asked, at most [`OwedBy::REQUESTS`] of them.
    pub requests: Vec<FrameRequest>,
}

impl OwedBy {
    /// How many distinct asking lines a frame keeps.
    pub const REQUESTS: usize = 32;

    /// Nothing held.
    pub fn is_empty(&self) -> bool {
        self.transitions.is_empty()
            && self.cycles.is_empty()
            && self.departures.is_empty()
            && self.scrolls.is_empty()
            && self.autoscroll.is_none()
            && self.animate.is_empty()
            && self.requests.is_empty()
    }

    fn clear(&mut self) {
        self.transitions.clear();
        self.cycles.clear();
        self.departures.clear();
        self.scrolls.clear();
        self.autoscroll = None;
        self.animate.clear();
        self.requests.clear();
    }
}

/// The core's side of the trace: what [`Core::frame_cause`] and
/// [`Core::owed_by`] read, and what fills them.
#[derive(Default)]
pub(crate) struct Trace {
    /// `set_frame_trace`: the holders and the digest are kept.
    on: bool,
    /// The frame being built's reasons (the last frame's, between frames).
    cause: FrameCause,
    /// What has reached the window since this frame began: the next
    /// frame's reasons so far.
    since: FrameCause,
    /// Who held the frame being built, as its `begin_frame` found them.
    owed_by: OwedBy,
    /// The lines that asked for a frame since the last one began.
    requests: Vec<FrameRequest>,
    /// The last finished frame's display-list digest, and whether it
    /// matched the one before.
    digest: Option<u64>,
    unchanged: Option<bool>,
    /// The departures' names, taken by the exit diff from the frame that
    /// still had them: a ghost's node is in no tree after that frame.
    gone: FxHashMap<Key, String>,
    /// [`Core::begin_frame_cause`] took the next frame's reasons ahead of
    /// its `begin_frame`, which keeps them.
    begun: bool,
}

impl Trace {
    fn ask(&mut self, why: &'static str, at: &'static Location<'static>) {
        if self.requests.len() < OwedBy::REQUESTS
            && !self.requests.iter().any(|r| r.at == at && r.why == why)
        {
            self.requests.push(FrameRequest { why, at });
        }
    }
}

impl Core {
    /// Turns the trace of why frames run on or off: who
    /// holds each owed frame ([`Self::owed_by`]) and whether each frame
    /// changed what is drawn ([`Self::frame_unchanged`]). Off by default,
    /// where neither costs anything; on, the holders are taken at the
    /// start of every frame the last one owed — a walk of what is owed
    /// and one of the last frame's tree to name it — and the display
    /// list is hashed at the end of every frame. [`Self::frame_cause`]
    /// is kept either way.
    pub fn set_frame_trace(&mut self, on: bool) {
        let t = &mut self.trace;
        t.on = on;
        if !on {
            t.owed_by.clear();
            t.requests.clear();
            t.digest = None;
            t.unchanged = None;
            t.gone.clear();
        }
    }

    /// Whether [`Self::set_frame_trace`] turned the trace on.
    pub fn frame_trace(&self) -> bool {
        self.trace.on
    }

    /// Why the frame being built runs: every reason that reached the
    /// window between the start of the last frame and the start of this
    /// one. Between frames, the last frame's — or, after
    /// [`Self::begin_frame_cause`], the next one's. See [`FrameCause`].
    pub fn frame_cause(&self) -> FrameCause {
        self.trace.cause
    }

    /// Starts the next frame's record now rather than at its
    /// `begin_frame`: its reasons ([`Self::frame_cause`]) and, traced,
    /// who holds it ([`Self::owed_by`]) — for a driver whose view runs
    /// before the frame it is for begins. Node's loop runs
    /// `view` to a tree and only then hands the tree to a frame, so a
    /// view reading either would read the frame before; the loop calls
    /// this first, and the view reads the frame it is building. The
    /// `begin_frame` that follows keeps what this took, and a second
    /// call before it is nothing. What reaches the window in between —
    /// input, a note — is the frame after's, as it is during a build.
    pub fn begin_frame_cause(&mut self) {
        if !self.trace.begun {
            self.take_frame_cause();
            self.trace.begun = true;
        }
    }

    /// Adds to the next frame's reasons — the driver's door, for what it
    /// saw and the core never will: a wake, a resize, a blink, a retry,
    /// an OS event it kept. The input it hands [`Self::handle_input`] is
    /// recorded without this.
    pub fn note_frame_cause(&mut self, cause: FrameCause) {
        self.trace.since |= cause;
    }

    /// Who held the frame the last one left owed, as the frame being
    /// built found them (see [`OwedBy`]) — or, after
    /// [`Self::begin_frame_cause`], as the next one will. Empty when the
    /// trace is off.
    pub fn owed_by(&self) -> &OwedBy {
        &self.trace.owed_by
    }

    /// Whether the last finished frame drew exactly what the one before
    /// it drew — the same quads, clips, fragments and textures at the
    /// same size and scale — so it changed nothing on screen. `None`
    /// while the trace is off and for the first frame after it came on.
    /// A frame that draws a fragment is never unchanged: the shader reads
    /// the clock. What the glyph atlas holds is not compared, only where
    /// the quads sample it.
    pub fn frame_unchanged(&self) -> Option<bool> {
        self.trace.unchanged
    }

    /// Asks for the next frame on the core's own behalf — `why` says
    /// what for, in a trace.
    #[track_caller]
    pub(crate) fn owe_frame(&mut self, why: &'static str) {
        self.frame_requested = true;
        if self.trace.on {
            self.trace.ask(why, Location::caller());
        }
    }

    /// `request_frame`'s half of the trace.
    #[track_caller]
    pub(super) fn trace_request(&mut self) {
        if self.trace.on {
            self.trace.ask("request_frame", Location::caller());
        }
    }

    /// `begin_frame`'s reset of `frame_requested`, mirrored.
    pub(super) fn trace_forget_requests(&mut self) {
        self.trace.requests.clear();
    }

    /// One input's reason, from [`Self::handle_input`].
    pub(super) fn trace_input(&mut self, ev: &InputEvent) {
        self.trace.since |= FrameCause::of_input(ev);
    }

    /// The start of `begin_frame`, before anything there moves: the
    /// reasons gathered since the last frame began become this frame's,
    /// and — traced — the holders of what the last frame owed are named
    /// against its tree, which `begin_frame` is about to clear.
    pub(super) fn trace_begin_frame(&mut self) {
        // Taken already, ahead of a view that ran before this frame did.
        if std::mem::take(&mut self.trace.begun) {
            return;
        }
        self.take_frame_cause();
    }

    /// The record itself: [`Self::trace_begin_frame`]'s, or
    /// [`Self::begin_frame_cause`]'s ahead of it.
    fn take_frame_cause(&mut self) {
        let owed = self.owed();
        let mut cause = std::mem::take(&mut self.trace.since);
        if owed.any() {
            cause |= FrameCause::OWED;
        }
        self.trace.cause = cause;
        if !self.trace.on {
            return;
        }
        let mut by = std::mem::take(&mut self.trace.owed_by);
        by.clear();
        std::mem::swap(&mut by.requests, &mut self.trace.requests);
        self.trace.requests.clear();
        if owed.any() {
            self.name_holders(owed, &mut by);
        } else {
            by.requests.clear();
        }
        self.trace.owed_by = by;
        if !self.trace.gone.is_empty() {
            let held: rustc_hash::FxHashSet<Key> = self.depart.keys().collect();
            self.trace.gone.retain(|k, _| held.contains(k));
        }
    }

    /// The exit diff's roots, as indices of `prev_tree` — the frame that
    /// declared them — named against that frame, traced.
    pub(super) fn trace_departures(&mut self, roots: &[usize]) {
        if !self.trace.on {
            return;
        }
        let names = Names::new(&self.prev_tree, &self.key_labels_last);
        for &i in roots {
            let key = self.prev_tree.keys[i];
            let name = names.holder(key, None).name;
            self.trace.gone.insert(key, name);
        }
    }

    fn name_holders(&self, owed: crate::Owed, by: &mut OwedBy) {
        let names = Names::new(&self.tree, &self.key_labels);
        if owed.transition {
            let mut slots: FxHashMap<Key, u16> = FxHashMap::default();
            self.anim.owing(|key, slot| {
                *slots.entry(key).or_default() |= 1 << slot as u16;
            });
            let mut held: Vec<(usize, FrameHolder)> = slots
                .into_iter()
                .map(|(key, mask)| {
                    let mut h = names.holder(key, None);
                    h.slots = crate::anim::Slot::names(mask);
                    (names.order(key), h)
                })
                .collect();
            held.sort_by_key(|(at, _)| *at);
            by.transitions = held.into_iter().map(|(_, h)| h).collect();
        }
        if owed.cycle && self.anim.time().is_some() {
            for i in 0..self.tree.len() {
                let spec = &self.tree.specs[i];
                if spec.transition.is_some_and(|t| t.duration_ms > 0.0)
                    && !spec.anim().keyframes.is_empty()
                {
                    by.cycles.push(names.holder(self.tree.keys[i], None));
                }
            }
        }
        if owed.depart {
            for (key, spec) in self.depart.roots() {
                let mut h = names.holder(key, Some(spec));
                if let Some(name) = self.trace.gone.get(&key) {
                    h.name.clone_from(name);
                }
                by.departures.push(h);
            }
        }
        if owed.scroll {
            let mut keys: Vec<Key> = self.scroll.easing().collect();
            keys.sort_by_key(|k| names.order(*k));
            by.scrolls = keys.into_iter().map(|k| names.holder(k, None)).collect();
        }
        if owed.autoscroll {
            by.autoscroll = self.autoscroller().map(|k| names.holder(k, None));
        }
        if self.tree.any_animate {
            for i in 0..self.tree.len() {
                if self.tree.specs[i].animate {
                    by.animate.push(names.holder(self.tree.keys[i], None));
                }
            }
        }
        if !owed.requested {
            by.requests.clear();
        }
    }

    /// The end of `finish_frame`, traced: this frame's display list
    /// against the last one's.
    pub(super) fn trace_finish_frame(&mut self) {
        if !self.trace.on {
            return;
        }
        let digest = digest(&self.display);
        self.trace.unchanged = self.trace.digest.map(|d| d == digest);
        self.trace.digest = Some(digest);
    }
}

/// The last frame's tree and labels, indexed once for naming holders.
struct Names<'a> {
    tree: &'a Tree,
    at: FxHashMap<Key, u32>,
    labels: FxHashMap<Key, &'a str>,
}

impl<'a> Names<'a> {
    fn new(tree: &'a Tree, labels: &'a crate::key::LabelIndex) -> Self {
        Names {
            tree,
            at: tree
                .keys
                .iter()
                .enumerate()
                .map(|(i, k)| (*k, i as u32))
                .collect(),
            labels: labels.iter().collect(),
        }
    }

    /// Where `key` sits in tree order, for sorting; past the end when it
    /// is in no tree.
    fn order(&self, key: Key) -> usize {
        self.at.get(&key).map_or(usize::MAX, |i| *i as usize)
    }

    fn holder(&self, key: Key, gone: Option<&NodeSpec>) -> FrameHolder {
        let mut parts: Vec<&str> = Vec::new();
        let spec = match self.at.get(&key) {
            Some(&i) => {
                let mut p = self.tree.parent[i as usize];
                while p != NIL {
                    if let Some(l) = self.labels.get(&self.tree.keys[p as usize]) {
                        parts.push(l);
                    }
                    p = self.tree.parent[p as usize];
                }
                parts.reverse();
                Some(&self.tree.specs[i as usize])
            }
            None => gone,
        };
        let own = match self.labels.get(&key) {
            Some(l) => l.to_string(),
            None => match spec.and_then(|s| s.access().label.as_deref()) {
                Some(l) => format!("\"{l}\""),
                None => format!("#{:08x}", key.0 as u32),
            },
        };
        let mut name = String::new();
        for p in parts {
            name.push_str(p);
            name.push('/');
        }
        name.push_str(&own);
        FrameHolder {
            key,
            name,
            slots: Vec::new(),
        }
    }
}

/// A digest of what a display list draws: every quad, clip, fragment and
/// texture draw, at its viewport and scale. Not the dropped handles (a
/// backend's bookkeeping) and not the pixels behind a texture beyond its
/// revision.
fn digest(dl: &crate::display::DisplayList) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = rustc_hash::FxHasher::default();
    let f = |h: &mut rustc_hash::FxHasher, v: f32| h.write_u32(v.to_bits());
    f(&mut h, dl.viewport.w);
    f(&mut h, dl.viewport.h);
    f(&mut h, dl.scale);
    h.write_usize(dl.quads.len());
    for q in &dl.quads {
        for v in [q.rect.x, q.rect.y, q.rect.w, q.rect.h] {
            f(&mut h, v);
        }
        for c in [q.color, q.border_color] {
            for v in [c.r, c.g, c.b, c.a] {
                f(&mut h, v);
            }
        }
        for v in q.radius {
            f(&mut h, v);
        }
        f(&mut h, q.border_w);
        f(&mut h, q.blur);
        h.write_u32(q.kind as u32);
        h.write_u32(q.clip);
        for w in q.uv {
            h.write_u32(w);
        }
    }
    h.write_usize(dl.clips.len());
    for c in &dl.clips {
        for v in [c.rect.x, c.rect.y, c.rect.w, c.rect.h] {
            f(&mut h, v);
        }
        for v in c.radius {
            f(&mut h, v);
        }
    }
    h.write_usize(dl.fragments.len());
    if !dl.fragments.is_empty() {
        // A fragment reads the clock, so its frame is its own.
        f(&mut h, dl.time);
        for fr in &dl.fragments {
            fr.id.hash(&mut h);
            for v in fr.params {
                f(&mut h, v);
            }
            fr.image.hash(&mut h);
        }
    }
    h.write_usize(dl.textures.len());
    for (t, px) in dl.textures.iter().zip(&dl.texture_pixels) {
        t.id.hash(&mut h);
        for w in t.uv {
            h.write_u32(w);
        }
        h.write_u32(px.rev);
    }
    h.finish()
}
