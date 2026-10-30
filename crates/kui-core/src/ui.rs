//! [`Ui`]: the frame builder a Rust view declares its tree through.
//!
//! A `Ui` borrows a [`Core`] for the length of one frame. It is a thin,
//! safe facade over the core's flat builder methods (which are also the FFI
//! surface): every call opens, fills or closes a node, or reads a fact the
//! last frame established. It never captures app state, so `view(&state)`
//! and `update(&mut state)` cannot conflict.
//!
//! The shape of a view: containers open with [`Ui::with`] (scoped) or
//! [`Ui::open`]/[`Ui::close`], leaves with [`Ui::leaf`] and [`Ui::text`],
//! and each node's look and behaviour is a [`NodeSpec`]. Nodes are keyed by
//! position unless a `_keyed` or `_indexed` form gives them a stable
//! identity, which anything retained across frames (focus, scrolling, an
//! edit buffer, a transition) needs.
//!
//! ```rust
//! use kui_core::{Color, Core, NodeSpec, Size, Sizing, TextStyle, widgets};
//!
//! let mut core = Core::new();
//! let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
//! let theme = ui.theme();
//! ui.configure_root(NodeSpec::column().fill().pad(12.0).gap(8.0));
//!
//! // A toolbar: a row of buttons, keyed by their labels.
//! ui.with(NodeSpec::row().gap(6.0), |ui| {
//!     widgets::button(ui, "Open", "open");
//!     widgets::button(ui, "Save", "save");
//! });
//!
//! // A panel that grows to fill the rest, with a label in it.
//! let panel = ui.with_keyed(
//!     "panel",
//!     NodeSpec::column().fill().pad(8.0).bg(theme.surface).radius(6.0),
//!     |ui| {
//!         ui.text("Ready.", TextStyle::new(14.0).color(theme.muted));
//!         ui.leaf(NodeSpec::row().size(Sizing::GROW, 1.0).bg(Color::hex(0x80808080)));
//!     },
//! );
//! assert!(!ui.is_hovered(panel)); // nothing has moved the pointer yet
//! ui.finish();
//! ```
//!
//! [`Ui::finish`] is the one way out of a frame: it runs layout and
//! emission and leaves the results in [`Core::output`].

use crate::edit::EditOptions;
use crate::env::Env;
use crate::geom::{Rect, Size, Vec2};
use crate::key::Key;
use crate::line::Stroke;
use crate::runtime::{Core, FillAnswer};
use crate::slot::{Children, Fill, Instance};
use crate::spec::{NodeSpec, TextStyle};
use crate::text::Span;
use crate::tree::OriginId;
use crate::value::Value;
use crate::window::{WindowCommand, WindowConfig, WindowEvents, WindowId, WindowLifecycle};

/// What a memo is called in a recording's placements (backlog F285).
const MEMO: &str = "memo";

pub struct Ui<'a> {
    core: &'a mut Core,
    /// What fills the slots this frame declares (`Core::frame_with`);
    /// `None` for a frame begun without one, and inside a fill — an
    /// extension's `slot` declares and fills nothing.
    filler: Option<&'a mut dyn Fill>,
    /// The host's children of the component instance this `Ui` fills, if
    /// it fills one (ADR 0060): what `children(group)` builds.
    children: Option<&'a mut dyn Children>,
}

impl<'a> Ui<'a> {
    pub(crate) fn new(core: &'a mut Core) -> Self {
        Self {
            core,
            filler: None,
            children: None,
        }
    }

    /// A `Ui` from its parts: the core, the filler answering its slots,
    /// and the host's children of the instance it fills.
    pub(crate) fn parts(
        core: &'a mut Core,
        filler: Option<&'a mut dyn Fill>,
        children: Option<&'a mut dyn Children>,
    ) -> Self {
        Self {
            core,
            filler,
            children,
        }
    }

    /// [`Self::wrap`] with something to fill the slots the frame declares
    /// — for a frontend that drives `Core` directly and keeps an
    /// extension list of its own (the C context, a headless Node one), so
    /// its `slot` and `finish` are this type's rather than a copy of them.
    pub fn with_filler(core: &'a mut Core, filler: &'a mut dyn Fill) -> Self {
        Self {
            core,
            filler: Some(filler),
            children: None,
        }
    }

    /// Escape hatch to the underlying core (e.g. for FFI view callbacks).
    pub fn core(&mut self) -> &mut Core {
        self.core
    }

    /// The inverse escape hatch: wraps a borrowed core mid-frame so foreign
    /// frontends that drive `Core` directly (FFI, Node) can call `widgets::*`.
    pub fn wrap(core: &'a mut Core) -> Self {
        Self {
            core,
            filler: None,
            children: None,
        }
    }

    /// Declares a devtools tab an extension draws: `name` is the tab's
    /// identity, `label` what the strip shows, `component` the full name
    /// of the component (`"ts/panel"`) placed in its body (ADR 0067).
    /// While the tab is the one on show, the panel places the component in
    /// the tab's body with `props` and the panel's facts (`selected`,
    /// `hovered`, `picked`, `region`, `focus`) as its props — event props
    /// among `props` bring the tab's events to the host as its own
    /// messages; otherwise nothing is placed and the extension is not
    /// asked. Made every frame, panel on or off. A name declared twice in a
    /// frame warns `duplicate-tab`.
    pub fn devtools_tab(&mut self, name: &str, label: &str, component: &str, props: &Value) {
        self.core
            .devtools_tab_declare_with(name, label, Some(component), props);
    }

    /// Declares a devtools tab the host draws itself, and draws it
    /// **only when it is shown**: `f` runs
    /// when the panel is on, docked in this window, and `name` is the
    /// tab on show — otherwise this declares and returns, and the tab
    /// costs nothing. What `f` builds is the host's: its keys, labels
    /// and origin, its events reaching the host untouched — laid out
    /// and painted as a layer over the panel's tab body, clipped to it,
    /// in the dock's focus region. The panel's facts are read through
    /// `devtools_selected` and its siblings.
    pub fn devtools_tab_with(&mut self, name: &str, label: &str, f: impl FnOnce(&mut Ui<'_>)) {
        if !self.core.devtools_tab_declare(name, label, None) {
            return;
        }
        if !self.core.devtools_tab_shown(name) {
            return;
        }
        self.core.devtools_tab_open(name);
        f(self);
        self.core.close();
    }

    /// Whether the host form of tab `name` is shown this frame — what
    /// `devtools_tab_with` asks before running its closure, for a caller
    /// that builds the content some other way.
    pub fn devtools_tab_shown(&self, name: &str) -> bool {
        self.core.devtools_tab_shown(name)
    }

    /// The bare declaration either form makes: `slot` for the extension
    /// form, `None` for a host form whose
    /// content is built some other way or not at all this frame. Whether
    /// the declaration stood — false for a name already declared this
    /// frame (`duplicate-tab`) or outside a frame.
    pub fn devtools_tab_declare(&mut self, name: &str, label: &str, slot: Option<&str>) -> bool {
        self.core.devtools_tab_declare(name, label, slot)
    }

    /// The host form for a binding that decided the laziness on its own
    /// side (a Node encoder or a Lua converter that already read which
    /// tab is on show): declares the tab and builds
    /// `f` as its content **whether or not** the tab is shown here — a
    /// content whose body the panel did not build anchors to nothing
    /// and paints nothing, and a name already declared this frame
    /// (`duplicate-tab`) builds into a node of no size, so the caller's
    /// stream stays in step either way. `devtools_tab_with` is the door
    /// for a caller that can skip the work.
    pub fn devtools_tab_declared(&mut self, name: &str, label: &str, f: impl FnOnce(&mut Ui<'_>)) {
        if self.core.devtools_tab_declare(name, label, None) {
            self.core.devtools_tab_open(name);
        } else {
            self.core.open(
                NodeSpec::column()
                    .width(crate::spec::Sizing::Fixed(0.0))
                    .height(crate::spec::Sizing::Fixed(0.0))
                    .clip(),
            );
        }
        f(self);
        self.core.close();
    }

    /// The panel's selected node (`Core::devtools_selected`).
    pub fn devtools_selected(&self) -> Option<Key> {
        self.core.devtools_selected()
    }

    /// The panel's hovered tree row (`Core::devtools_hovered`).
    pub fn devtools_hovered(&self) -> Option<Key> {
        self.core.devtools_hovered()
    }

    /// The node the picker is over (`Core::devtools_picked`).
    pub fn devtools_picked(&self) -> Option<Key> {
        self.core.devtools_picked()
    }

    /// Whether the panel's picker is up (`Core::devtools_picking`).
    pub fn devtools_picking(&self) -> bool {
        self.core.devtools_picking()
    }

    /// The tab the panel is on, by name (`Core::devtools_current_tab`).
    pub fn devtools_current_tab(&self) -> String {
        self.core.devtools_current_tab()
    }

    /// Loads `ext` under `namespace` into the list filling this frame's
    /// slots, and answers with the origin it got. This is how an
    /// extension hosts an extension of its own: the guest asks mid-frame,
    /// when it knows what it wants, and the plugin lands in the same list
    /// as the host's own — one namespace map, one origin per extension,
    /// however deep the loading went (`crate::slot`).
    ///
    /// Fails when the namespace is taken, or empty with an extension that
    /// names itself nothing, exactly as
    /// `Extensions::push_as` does, and when this frame was begun without
    /// a filler (`Core::frame`) or with one that is not a list.
    pub fn add_extension(
        &mut self,
        namespace: &str,
        ext: Box<dyn crate::runtime::Extension>,
    ) -> Result<OriginId, String> {
        // A fill that loads an extension wants its next frame.
        self.core.taint_kept("it loaded an extension");
        match self.filler.as_deref_mut() {
            Some(filler) => filler.add(namespace, ext),
            None => Err(format!(
                "cannot load `{namespace}`: this frame declares no slots to fill"
            )),
        }
    }

    /// Runs `f` as the fill of `slot` under `origin`: nodes it opens are
    /// tagged with the origin, keyed under the slot's key, and closed
    /// for it if it leaves any open. What a `Fill` implementation calls
    /// per extension; see `Core::fill`.
    pub fn fill(&mut self, origin: OriginId, slot: &Instance<'_>, f: impl FnOnce(&mut Ui<'_>)) {
        self.core.fill(slot, origin, f);
    }

    /// `fill`, with `filler` answering the slots the fill declares — an
    /// extension hosting extensions of its own. `Extensions::fill_one`
    /// passes itself, which is what makes one namespace map do for every
    /// level; see `crate::slot`.
    pub fn fill_within(
        &mut self,
        origin: OriginId,
        slot: &Instance<'_>,
        filler: &mut dyn Fill,
        f: impl FnOnce(&mut Ui<'_>),
    ) {
        self.core.fill_within(slot, origin, Some(filler), f);
    }

    /// `fill_within` for a component instance (ADR 0060): `children`
    /// builds the host's children where the fill calls
    /// [`Self::children`]. What `Extensions` calls for an instance.
    pub fn fill_instance(
        &mut self,
        origin: OriginId,
        slot: &Instance<'_>,
        filler: &mut dyn Fill,
        children: &mut dyn Children,
        f: impl FnOnce(&mut Ui<'_>),
    ) {
        self.core
            .fill_instance(slot, origin, Some(filler), children, f);
    }

    /// Places a component an extension provides (ADR 0060): `name` is
    /// `namespace/component`, `props` what its fill reads — and the
    /// messages its events arrive as, in its event props (`onSelect`). An
    /// instance is keyed by where it is placed, or by a `key` in `props`,
    /// so a component repeats as freely as a node. It is **replayed by
    /// default**: an instance whose props and whose reads of the frame
    /// are what they were last frame is pushed again without asking the
    /// extension (ADR 0045's journal); `fresh: true` in `props` asks every
    /// frame. Answers how it was filled, `None` when nothing provides it
    /// (`unknown-component`) or outside a frame.
    ///
    /// ```rust,ignore
    /// ui.component("kit/button", &Value::map([("label", "Save".into()), ("onClick", Msg::Save.into())]));
    /// ```
    pub fn component(&mut self, name: &str, props: &Value) -> Option<FillAnswer> {
        self.place(name, props, None, None)
    }

    /// A part of the view built once and pushed again while `deps` hold
    /// (backlog F285): `f` builds it, and every frame after, placed at the
    /// same label with `deps` equal and nothing it read of the frame moved
    /// (a hover it asked about, a scroll, the theme), the nodes it pushed
    /// are pushed again without calling `f` — the journal a component
    /// instance replays by (ADR 0060), with no extension behind it. What
    /// the memo's nodes do is the view's own: their events are the
    /// placer's, their labels are found by `key_of` as any other; only
    /// their keys are derived from the memo's, so its siblings' do not
    /// depend on what it holds. `deps` is everything `f` reads that the
    /// core does not see — the model's numbers, the strings it formats —
    /// and a memo whose `deps` leave one out draws it stale. A memo inside
    /// a memo is kept on its own: the outer replays while both would, and
    /// the inner while it would, when the outer is built again.
    ///
    /// ```rust
    /// # use kui_core::{Core, NodeSpec, Size, TextStyle, Value, FillAnswer};
    /// # let mut core = Core::new();
    /// let units = 240;
    /// for frame in 0..2 {
    ///     let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ///     let answer = ui.memo("fleet", &Value::Int(units), |ui| {
    ///         for i in 0..units {
    ///             ui.text(&format!("unit {i}"), TextStyle::new(13.0));
    ///         }
    ///     });
    ///     assert_eq!(answer.replayed(), frame == 1);
    ///     ui.finish();
    /// }
    /// ```
    pub fn memo(&mut self, label: &str, deps: &Value, f: impl FnOnce(&mut Ui<'_>)) -> FillAnswer {
        let key = self.core.child_key(label);
        match self.memo_at(key, deps, |ui| {
            f(ui);
            Ok::<(), std::convert::Infallible>(())
        }) {
            Ok(answer) => answer,
        }
    }

    /// [`Self::memo`] whose body can fail - a script's that raised: what
    /// it pushed before the error stays in this frame, and nothing is
    /// kept, so the next frame runs it again rather than replaying half.
    pub fn try_memo<E>(
        &mut self,
        label: &str,
        deps: &Value,
        f: impl FnOnce(&mut Ui<'_>) -> Result<(), E>,
    ) -> Result<FillAnswer, E> {
        let key = self.core.child_key(label);
        self.memo_at(key, deps, f)
    }

    /// [`Self::try_memo`] keyed by a data index.
    pub fn try_memo_indexed<E>(
        &mut self,
        i: u64,
        deps: &Value,
        f: impl FnOnce(&mut Ui<'_>) -> Result<(), E>,
    ) -> Result<FillAnswer, E> {
        let key = self.core.child_key_indexed(i);
        self.memo_at(key, deps, f)
    }

    /// [`Self::memo`] keyed by a data index, as `open_indexed` keys a
    /// node: a list's rows each a memo keep theirs as the list shifts.
    pub fn memo_indexed(
        &mut self,
        i: u64,
        deps: &Value,
        f: impl FnOnce(&mut Ui<'_>),
    ) -> FillAnswer {
        let key = self.core.child_key_indexed(i);
        match self.memo_at(key, deps, |ui| {
            f(ui);
            Ok::<(), std::convert::Infallible>(())
        }) {
            Ok(answer) => answer,
        }
    }

    fn memo_at<E>(
        &mut self,
        key: Key,
        deps: &Value,
        f: impl FnOnce(&mut Ui<'_>) -> Result<(), E>,
    ) -> Result<FillAnswer, E> {
        if !self.core.memo_building() {
            return Ok(FillAnswer::NotKept);
        }
        // Memos and component instances share a keep store, not an identity
        // namespace: a component with the same label must not replay this body.
        let key = key.str("kui:memo");
        // Inside a component a kept fill placed, which is filled fresh and
        // not kept, nothing inside is kept either.
        let can_keep = self.core.keeping() || !self.core.recording_busy();
        let found = if can_keep {
            self.core.take_replayable_instance(key, deps)
        } else {
            Err((
                FillAnswer::NotKept,
                Some("inside a fill drawn fresh".into()),
            ))
        };
        let (answer, why) = match found {
            Ok(kept) => {
                self.memo_push(key, kept);
                (FillAnswer::Replayed, None)
            }
            Err((answer, why)) => {
                let scope = self.core.memo_enter(key, deps, false);
                let built = f(self);
                self.core.memo_leave(scope, built.is_err());
                built?;
                (answer, why)
            }
        };
        self.core.perf_placement(key, MEMO, answer, why.as_deref());
        Ok(answer)
    }

    /// What the memo at `key` kept, pushed again as it is: the op a kept
    /// fill journaled for a memo inside it, replayed (`Op::Memo`).
    pub(crate) fn memo_replay(&mut self, key: Key) {
        if let Some(kept) = self.core.take_kept_memo(key) {
            self.memo_push(key, kept);
            self.core
                .perf_placement(key, MEMO, FillAnswer::Replayed, None);
        }
    }

    fn memo_push(&mut self, key: Key, mut kept: crate::runtime::replay::Kept) {
        self.core.note_layout_replay();
        let scope = self.core.memo_enter(key, &Value::Null, true);
        let mut ops = kept.take_ops();
        for op in &mut ops {
            crate::runtime::replay::replay_op(self, op);
        }
        kept.put_ops(ops);
        self.core.keep_again(kept);
        self.core.memo_leave(scope, false);
    }

    /// A contribution slot (ADR 0067): every component the loaded
    /// extensions contribute to `name` is placed here, as siblings in the
    /// current container — no wrapper; wrap the slot for a box — in load
    /// order and then each contribution's `order`. Each gets `props`, and
    /// its instance's `slot` says where it is. `name` is any string, by
    /// the convention `owner.place` (`app.toolbar`), never read as a
    /// namespace. Each contribution is an instance keyed by the slot (its
    /// name, or a `key` prop), its provider and its component, so adding
    /// a plugin leaves the others' keys, focus and scroll where they were;
    /// each is kept and replayed on its own. Its events arrive as the
    /// messages `props` declares, with `from` naming the provider. Answers
    /// how many were placed; an empty slot is silent.
    ///
    /// ```rust,ignore
    /// ui.slot("app.toolbar", &Value::map([("onRun", Msg::Run.into())]));
    /// ```
    pub fn slot(&mut self, name: &str, props: &Value) -> usize {
        self.slot_at(None, name, props)
    }

    /// [`Self::slot`] at a key: a replay places a slot again where its
    /// kept fill placed it, running the provider loop anew.
    pub(crate) fn slot_at(&mut self, at: Option<Key>, name: &str, props: &Value) -> usize {
        if self.core.tree.is_empty() {
            return 0;
        }
        let key = match at {
            Some(k) => k,
            None => self
                .prop_key(props)
                .unwrap_or_else(|| self.core.child_key(name)),
        };
        let found = match self.filler.as_deref() {
            Some(f) => f.contributions(name),
            None => Vec::new(),
        };
        // Inside a fill being kept, the slot is one op: a replay runs the
        // provider loop again, so a plugin loaded since is placed.
        let keeping = self.core.keeping();
        if keeping {
            self.core.keep_op(crate::runtime::replay::Op::Slot {
                key,
                name: name.into(),
                props: props.clone(),
            });
            self.core.pause_keeping();
        }
        let props = match props {
            Value::Map(entries) if entries.iter().any(|(k, _)| k == "key") => Value::Map(
                entries
                    .iter()
                    .filter(|(k, _)| k != "key")
                    .cloned()
                    .collect(),
            ),
            other => other.clone(),
        };
        let mut placed = 0;
        for full in &found {
            let (ns, short) = crate::slot::split_name(full);
            self.core.set_placing_slot(Some(name));
            if self
                .place_at(Some(key.str(ns).str(short)), full, &props, None, None)
                .is_some()
            {
                placed += 1;
            }
        }
        if keeping {
            self.core.resume_keeping();
        }
        placed
    }

    /// [`Self::component`] with the host's own children: `children(group,
    /// ui)` builds the group the fill asks for — `""` for the unnamed one
    /// — where the fill asks, under the host's origin, keyed by the
    /// instance and the group and not by how the fill nests them, their
    /// events the host's. A group the fill does not ask for is not built.
    pub fn component_with(
        &mut self,
        name: &str,
        props: &Value,
        mut children: impl FnMut(&str, &mut Ui<'_>),
    ) -> Option<FillAnswer> {
        self.place(name, props, Some(&mut children), None)
    }

    /// [`Self::component_with`] whose fill is `fill` rather than whatever
    /// extension the name's namespace is: how an extension places a
    /// component of its own inside one of its fills — its own namespace's
    /// extension is busy drawing, so the list cannot ask it — keeping the
    /// instance boundary (its key, its replay, its events, which reach the
    /// extension as its own message). The instance's nodes are the current
    /// origin's.
    pub fn component_by(
        &mut self,
        name: &str,
        props: &Value,
        mut children: impl FnMut(&str, &mut Ui<'_>),
        mut fill: impl FnMut(&Instance<'_>, &mut Ui<'_>),
    ) -> Option<FillAnswer> {
        self.place(name, props, Some(&mut children), Some(&mut fill))
    }

    /// Builds the host's children of `group` here (ADR 0060): what a
    /// component's fill calls where the host's content goes — `""` for
    /// the unnamed group, a name for another (`"footer"`). Once per group
    /// per instance; outside an instance's fill it builds nothing and
    /// raises `children-misplaced`.
    pub fn children(&mut self, group: &str) {
        let Some(state) = self.core.begin_children(group) else {
            return;
        };
        if let Some(children) = self.children.as_deref_mut() {
            let mut ui = Ui::parts(
                &mut *self.core,
                self.filler.as_deref_mut().map(shorter_fill),
                None,
            );
            children.build(group, &mut ui);
        }
        self.core.end_children(state);
    }

    fn place(
        &mut self,
        name: &str,
        props: &Value,
        children: Option<&mut dyn Children>,
        by: Option<InstanceFill<'_>>,
    ) -> Option<FillAnswer> {
        self.place_at(None, name, props, children, by)
    }

    /// A component placed again by a replay, at the key it had when the
    /// kept fill placed it: a replay pushes nodes under their own keys, so
    /// the counter a fresh placement would read is not where it was.
    pub(crate) fn component_at(
        &mut self,
        key: Key,
        name: &str,
        props: &Value,
    ) -> Option<FillAnswer> {
        self.place_at(Some(key), name, props, None, None)
    }

    fn place_at(
        &mut self,
        at: Option<Key>,
        name: &str,
        props: &Value,
        children: Option<&mut dyn Children>,
        by: Option<InstanceFill<'_>>,
    ) -> Option<FillAnswer> {
        if self.core.tree.is_empty() {
            return None;
        }
        // A string key also names the instance in the devtools.
        let label = props.get_str("key");
        let key = match at {
            Some(k) => k,
            None => self.prop_key(props).unwrap_or_else(|| self.core.auto_key()),
        };
        // A `root` is drawn fresh every frame: a view's state can change
        // without an event the core sees (ADR 0067).
        let fresh = matches!(props.get("fresh"), Some(Value::Bool(true)))
            || crate::slot::split_name(name).1 == crate::slot::ROOT_COMPONENT;
        // Inside a fill being kept, a placement is one more op of it: a
        // replay places it again — unless the host's children are a
        // closure of the fill's own, which no replay could call.
        let keeping = self.core.keeping();
        if keeping {
            if children.is_some() {
                self.core
                    .taint_kept("it placed a component with children of its own");
            } else {
                self.core.keep_op(crate::runtime::replay::Op::Component {
                    key,
                    name: name.into(),
                    props: props.clone(),
                });
            }
            self.core.pause_keeping();
        }
        let placer = self.core.origin();
        let slot_now = self.core.current_fill_slot();
        // The host's children see the placer's own children, so a fill
        // can forward its host's content into a component it places.
        let mut none = |_: &str, _: &mut Ui<'_>| {};
        let mut placed = Placed {
            inner: children.unwrap_or(&mut none),
            outer: self.children.as_deref_mut().map(shorter_children),
        };
        let mut answer = None;
        // Why it was drawn fresh, for the recording: the fact that moved,
        // what made it unreplayable, or what never lets it be kept.
        let mut why_fresh: Option<std::borrow::Cow<'static, str>> = if keeping {
            Some("inside a fill drawn fresh".into())
        } else if fresh {
            Some("drawn fresh every frame".into())
        } else {
            None
        };
        let (namespace, short) = crate::slot::split_name(name);
        // The contribution slot this placement is for, if any: the fill
        // that draws it takes it again (`Extensions::fill_one`).
        let in_slot = self.core.take_placing_slot();
        let slot = Instance {
            name: short,
            namespace,
            params: props,
            key,
            slot: in_slot.as_deref(),
        };
        self.core.set_placing_slot(in_slot.as_deref());
        if let Some(by) = by {
            // The fill is the caller's: the instance is the current
            // origin's, filled in place.
            let heard = self
                .filler
                .as_deref_mut()
                .is_some_and(|f| f.placed(name, key, props, placer, slot_now));
            let replayed = if !fresh && !keeping {
                match self.core.take_replayable_instance(key, props) {
                    Ok(kept) if !heard => {
                        let filler = self.filler.as_deref_mut().map(shorter_fill);
                        self.core.replay_instance(&slot, kept, filler, &mut placed);
                        Some(FillAnswer::Replayed)
                    }
                    Ok(_) => {
                        self.core.keep_next_instance(key, props);
                        Some(FillAnswer::Heard)
                    }
                    Err((fill, why)) => {
                        self.core.keep_next_instance(key, props);
                        if !heard {
                            why_fresh = why.map(Into::into);
                        }
                        Some(if heard { FillAnswer::Heard } else { fill })
                    }
                }
            } else {
                None
            };
            answer = match replayed {
                Some(FillAnswer::Replayed) => Some(FillAnswer::Replayed),
                other => {
                    let filler = self.filler.as_deref_mut().map(shorter_fill);
                    self.core
                        .fill_instance(&slot, placer, filler, &mut placed, |ui| by(&slot, ui));
                    self.core.forget_next_fill();
                    Some(other.unwrap_or(FillAnswer::NotKept))
                }
            };
        } else if let Some(filler) = self.filler.as_deref_mut() {
            let heard = filler.placed(name, key, props, placer, slot_now);
            if !fresh && !keeping {
                match self.core.take_replayable_instance(key, props) {
                    Ok(kept) if !heard => {
                        self.core
                            .replay_instance(&slot, kept, Some(filler), &mut placed);
                        answer = Some(FillAnswer::Replayed);
                    }
                    kept => {
                        let fill = match kept {
                            Err((fill, why)) if !heard => {
                                why_fresh = why.map(Into::into);
                                fill
                            }
                            _ => FillAnswer::Heard,
                        };
                        self.core.keep_next_instance(key, props);
                        let filled = filler.component(
                            name,
                            key,
                            props,
                            &mut placed,
                            &mut Ui::new(self.core),
                        );
                        self.core.forget_next_fill();
                        answer = filled.then_some(fill);
                    }
                }
            } else if filler.component(name, key, props, &mut placed, &mut Ui::new(self.core)) {
                answer = Some(FillAnswer::NotKept);
            }
        }
        if keeping {
            self.core.resume_keeping();
        }
        self.core.set_placing_slot(None);
        match answer {
            Some(a) => self.core.perf_placement(key, name, a, why_fresh.as_deref()),
            None => self.core.warn(crate::diag::unknown_component(name, key)),
        }
        if let Some(l) = label {
            self.core.note_label(key, l);
        }
        answer
    }

    /// The origin the nodes opened right now are tagged with:
    /// `OriginId::HOST` in the host's own view, the filling extension's
    /// inside a fill. What records who declared a slot, and so where the
    /// replies of whatever fills it go.
    pub fn origin(&self) -> OriginId {
        self.core.origin()
    }

    /// The viewport this frame lays out into: the window, less the
    /// devtools' dock while the panel is docked (`Core::viewport`).
    pub fn viewport(&self) -> Size {
        self.core.viewport()
    }

    /// The env reading's inputs, the frame's facts included
    /// (`Core::env_facts`): what a binding's `env` table is filled from.
    pub fn env_facts(&self) -> crate::schema::EnvFacts {
        self.core.env_facts()
    }

    /// Host facts pushed by the frame driver (refresh rate, focus).
    pub fn env(&self) -> Env {
        self.core.env
    }

    /// This frame's palette: the named colours the stock widgets paint
    /// with, derived from `env.system` unless the app pinned something
    /// else (see [`crate::theme`]).
    ///
    /// By value, because it is [`Copy`] and a view that took a reference
    /// could not then touch `ui`. `let t = ui.theme();` at the top of a
    /// widget is the idiom.
    pub fn theme(&self) -> crate::theme::Theme {
        *self.core.theme()
    }

    /// Whether anyone chose the theme's accent, or it is kui's fallback
    /// blue — see [`Core::has_accent`](crate::runtime::Core::has_accent).
    pub fn has_accent(&self) -> bool {
        self.core.has_accent()
    }

    /// The sizes the stock widgets are built from
    /// ([`Metrics`](crate::metrics::Metrics)), by value like the theme and
    /// for the same reason. What a view reads to make its own controls
    /// agree with the stock ones on a radius and a padding.
    pub fn metrics(&self) -> crate::metrics::Metrics {
        *self.core.metrics()
    }

    /// Declares the tokens this origin references by name (see
    /// [`crate::tokens`] and
    /// [`Core::set_tokens`](crate::runtime::Core::set_tokens)). Inside a
    /// fill the table is the extension's own.
    pub fn set_tokens(&mut self, tokens: crate::tokens::Tokens) {
        self.core.set_tokens(tokens);
    }

    /// What a `$name` resolves to this frame — the running origin's
    /// table over the host's, the roles in front of both — for a view
    /// that reads a token by name rather than holding its id.
    pub fn tokens(&self) -> crate::tokens::TokenLookup<'_> {
        self.core.token_lookup()
    }

    /// A colour token by name, this frame's half. A name that resolves
    /// to nothing, or to a length, raises `unknown-token` and answers
    /// `None`, so the view leaves the slot at its default
    /// (`.bg(ui.token_color("peach").unwrap_or(t.surface))`) rather than
    /// painting a transparent that would hide the node a typo was on.
    pub fn token_color(&mut self, name: &str) -> Option<crate::color::Color> {
        match self.core.token_lookup().color(name) {
            Ok(c) => Some(c),
            Err(e) => {
                self.core.warn_unknown_token(&e);
                None
            }
        }
    }

    /// A length token by name; unknown warns and answers `None`, as above.
    pub fn token_length(&mut self, name: &str) -> Option<f32> {
        match self.core.token_lookup().length(name) {
            Ok(v) => Some(v),
            Err(e) => {
                self.core.warn_unknown_token(&e);
                None
            }
        }
    }

    /// A family token by name: the face its fallback list picked, sans
    /// for a list nothing matched (ADR 0049); unknown warns and answers
    /// `None`, as above.
    pub fn token_family(&mut self, name: &str) -> Option<crate::spec::FontFamily> {
        match self.core.token_lookup().family(name) {
            Ok(f) => Some(f),
            Err(e) => {
                self.core.warn_unknown_token(&e);
                None
            }
        }
    }

    /// Declares this frame's window title (declare every frame you care;
    /// the driver diffs and applies changes).
    pub fn window_title(&mut self, title: &str) {
        self.core.set_window_title(title);
    }

    /// Declares that this frame wants the window kept above every other
    /// app's; see [`crate::Core::set_always_on_top`]. Declare it every
    /// frame you want it — a frame that does not lowers the window again,
    /// which is what makes a pin button a toggle — and read whether the
    /// platform agreed from `env().window.always_on_top`.
    pub fn always_on_top(&mut self, on_top: bool) {
        self.core.set_always_on_top(on_top);
    }

    /// Declares that this frame wants secure keyboard entry while the
    /// window has the keyboard (a password prompt); see
    /// [`crate::Core::set_secure_input`]. Declare it every frame the
    /// prompt is up: a frame that does not turns it off.
    pub fn secure_input(&mut self, on: bool) {
        self.core.set_secure_input(on);
    }

    /// Declares which Option keys act as Alt in this window on macOS, so
    /// Option-u arrives as the chord `A-u` rather than composing an
    /// accent; see [`crate::Core::set_option_as_alt`]. Declare it every
    /// frame: a frame that does not gives the Option keys back to the
    /// layout.
    pub fn option_as_alt(&mut self, option_as_alt: crate::OptionAsAlt) {
        self.core.set_option_as_alt(option_as_alt);
    }

    /// Declares that this window takes the keyboard as keys, with the
    /// platform's input method off — no composition, and on a Mac no dead
    /// keys and no press-and-hold, so a held letter repeats; see
    /// [`crate::Core::set_ime_off`]. A modal editor's normal mode. Declare
    /// it every frame: a frame that does not gives the IME back.
    pub fn ime_off(&mut self, off: bool) {
        self.core.set_ime_off(off);
    }

    /// Declares that a window named `name` exists this frame; see
    /// `Core::declare_window`. It opens on the first frame that declares
    /// it (`config` is read then and never again), stays open while any
    /// window's frame keeps declaring it, and closes when none does.
    /// `view` is then called for it too, with [`Ui::window_name`] saying
    /// which window is being drawn.
    pub fn window(&mut self, name: &str, config: WindowConfig) {
        self.core.declare_window(name, config);
    }

    /// [`Ui::window`], with the messages its opening, closing and a
    /// popup's dismissal arrive as; see `Core::declare_window_with`.
    pub fn window_with(&mut self, name: &str, config: WindowConfig, lifecycle: WindowLifecycle) {
        self.core.declare_window_with(name, config, lifecycle);
    }

    /// Declares the messages this window's own events arrive as — a
    /// resize, its keyboard focus, a system setting, the fonts, the
    /// modifiers, the OS's documents; see `Core::set_window_events` (ADR
    /// 0048). Declare it every frame: a frame that does not hears none.
    pub fn window_events(&mut self, events: WindowEvents) {
        self.core.set_window_events(events);
    }

    /// The name of the window this frame is drawing: `"main"` for the one
    /// the launcher opened, else the name the declaration that opened it
    /// used. `env().window.id` is the same window as a number.
    pub fn window_name(&self) -> std::rc::Rc<str> {
        self.core.window_name()
    }

    /// Sets the origin the nodes opened from here on are tagged with; see
    /// [`Ui::origin`].
    pub fn set_origin(&mut self, origin: OriginId) {
        self.core.set_origin(origin);
    }

    /// The root node's spec for this frame: its direction, padding, gap
    /// and background. Call it first; the default root is a fit column.
    pub fn configure_root(&mut self, spec: NodeSpec) {
        self.core.configure_root(spec);
    }

    /// The key a child opened under `label` would get here, without
    /// opening it: read it before the node exists to ask `is_hovered` or
    /// `is_focused` while building it.
    pub fn child_key(&self, label: &str) -> Key {
        self.core.child_key(label)
    }

    /// The key the `i`th child gets from auto-keying; see `open_indexed`.
    pub fn child_key_indexed(&self, i: u64) -> Key {
        self.core.child_key_indexed(i)
    }

    /// The key a placement's `key` prop names (backlog F279): a string is a
    /// name, as `child_key`; an integer, or a float with no fraction (a
    /// number from Lua or JavaScript), a data index, as `open_indexed` -
    /// so a row of a list keeps its instance, and what its fill kept, while
    /// the list shifts under it. Names and indices never collide. `None`
    /// without one, or for another kind, which keys by position. Until
    /// alpha.73 an integer was read as no key at all.
    fn prop_key(&self, props: &Value) -> Option<Key> {
        match props.get("key")? {
            Value::Str(label) => Some(self.core.child_key(label)),
            Value::Int(i) => Some(self.core.child_key_indexed(*i as u64)),
            Value::Float(f) if f.fract() == 0.0 && f.is_finite() => {
                Some(self.core.child_key_indexed(*f as i64 as u64))
            }
            _ => None,
        }
    }

    /// Whether the pointer is over `key`, as of the last input. Only a
    /// node that tracks hover answers true (one with a click, a drag, a
    /// hover background or `hoverable`).
    pub fn is_hovered(&self, key: Key) -> bool {
        self.core.is_hovered(key)
    }

    /// Whether the primary button is held on `key`.
    pub fn is_pressed(&self, key: Key) -> bool {
        self.core.is_pressed(key)
    }

    /// Whether files dragged in from the OS are over `key`, for
    /// drop-dependent *layout*; a colour swap is `drop_bg`.
    pub fn is_drop_target(&self, key: Key) -> bool {
        self.core.is_drop_target(key)
    }

    /// The zone the dragged files are over, if any.
    pub fn drop_target(&self) -> Option<Key> {
        self.core.drop_target()
    }

    /// Whether any member of a hover group (`NodeSpec::hover_group`) is
    /// hovered; the id comes from `NodeSpec::hover_group_id`.
    pub fn is_group_hovered(&self, group: u64) -> bool {
        self.core.is_group_hovered(group)
    }

    /// Whether any member of a hover group is pressed.
    pub fn is_group_pressed(&self, group: u64) -> bool {
        self.core.is_group_pressed(group)
    }

    /// Physical modifier state (the host also receives it as a
    /// `{kind="modifiers"}` event whenever it changes).
    pub fn modifiers(&self) -> crate::input::KeyMods {
        self.core.modifiers()
    }

    /// The exit `key` leaves by if this frame stops declaring it (backlog
    /// F136); see `Core::set_exit`. The view that removes a card thrown
    /// by a button names the throw in the same frame, with no frame drawn
    /// first to aim it.
    pub fn exit_with(&mut self, key: Key, exit: crate::enter::Enter) {
        self.core.set_exit(key, exit);
    }

    /// Asks for a frame at `at` on the frame clock (backlog F135); see
    /// `Core::request_frame_at`. `ui.request_frame_at(ui.now() + 3.0)` is
    /// a toast's expiry, with no thread and nothing owed in between.
    pub fn request_frame_at(&mut self, at: f64) {
        self.core.request_frame_at(at);
    }

    /// Asks for this frame, once drawn, written to `path` as a picture
    /// (ADR 0055); see `Core::capture_to`.
    #[track_caller]
    pub fn capture_to(&mut self, path: impl Into<String>, message: Option<Value>) {
        self.core.capture_to(path, message);
    }

    /// Asks for every frame drawn from this one on, written to `path` as
    /// an animation when the recording stops (ADR 0055); see
    /// `Core::record_to`.
    #[track_caller]
    pub fn record_to(&mut self, path: impl Into<String>, message: Option<Value>) {
        self.core.record_to(path, message);
    }

    /// Asks for this frame, once drawn, put on the system clipboard as a
    /// picture (ADR 0055); see `Core::capture_to_clipboard`.
    #[track_caller]
    pub fn capture_to_clipboard(&mut self, message: Option<Value>) {
        self.core.capture_to_clipboard(message);
    }

    /// Ends the recording with this frame, its last (ADR 0055); see
    /// `Core::stop_recording`.
    #[track_caller]
    pub fn stop_recording(&mut self) {
        self.core.stop_recording();
    }

    /// The frame clock in the driver's seconds (backlog F134); see
    /// `Core::now`. A view's deadlines read from it — a toast's expiry, a
    /// sequence's beats — so a test's `advance` moves them with the
    /// core's own easing.
    pub fn now(&self) -> f64 {
        self.core.now()
    }

    /// The display scale factor of the window being built — device pixels
    /// per logical px, the scale the frame was begun with (backlog C127).
    /// A view still lays out in logical px; it reads this to land an edge
    /// on a whole physical pixel, where a seam between shapes drawn side by
    /// side vanishes (a whole logical px is one at integer scales only).
    pub fn scale(&self) -> f32 {
        self.core.scale()
    }

    /// The caret's blink phase — `true` draws it; see
    /// `Core::caret_visible`. A custom editor draws its caret node on the
    /// on phase and skips it on the off, keeping the `caret` row on its
    /// `line` either way (that row is what the clock is armed on).
    pub fn caret_visible(&self) -> bool {
        self.core.caret_visible()
    }

    /// Asks for one more frame after this one; see `Core::request_frame`.
    #[track_caller]
    pub fn request_frame(&mut self) {
        self.core.request_frame();
    }

    /// Asks for one more frame on kui's own behalf (a stock widget's, not
    /// the app's), named `why` in a trace.
    #[track_caller]
    pub(crate) fn owe_frame(&mut self, why: &'static str) {
        self.core.owe_frame(why);
    }

    /// Measures text the way layout would, without adding a node; see
    /// `Core::measure_text`. Sizing a column to its widest label, or
    /// choosing a tier that fits, is arithmetic on these numbers instead
    /// of hand-tuned constants. The metrics do not scale linearly:
    /// `measured × zoom` is not `measure(size × zoom)`, because shaping
    /// rounds per size, so anything that zooms measures at the size it
    /// draws.
    pub fn measure_text(
        &mut self,
        content: &str,
        style: &TextStyle,
        max_w: Option<f32>,
    ) -> crate::text::TextMetrics {
        self.core.measure_text(content, style, max_w)
    }

    /// Where a point lands in the text node `key` drew, as a byte offset
    /// and a visual line; see `Core::text_hit`. During a build it answers
    /// from the last frame, which is the layout a click was made against.
    pub fn text_hit(&self, key: Key, point: Vec2) -> Option<crate::text::TextHit> {
        self.core.text_hit(key, point)
    }

    /// Whether the text under `key` was cut short where the last frame
    /// laid it out — ended in its ellipsis, or lost lines past its
    /// `max_lines`; see `Core::text_cut`. What a label that may be cut
    /// reads to float its whole text as a hint.
    pub fn text_cut(&self, key: Key) -> bool {
        self.core.text_cut(key)
    }

    /// The window's selected text — a `selectable` scope's, or the
    /// focused editor's; see `Core::copy_selection`.
    pub fn selection_text(&self) -> Option<String> {
        self.core.copy_selection()
    }

    /// The text selection's two ends as the drag made them — anchor, then
    /// focus — each a virtualised row's data index (or none) and a byte;
    /// see `Core::selection_ends`.
    pub fn selection_ends(&self) -> Option<(crate::select::RangeEnd, crate::select::RangeEnd)> {
        self.core.selection_ends()
    }

    /// The window's selection when it lives in a `cells` grid — its ends
    /// as absolute lines and columns; see `Core::cell_selection`.
    ///
    /// Offered where the text selection offers only [`Self::selection_text`]
    /// because a grid's ends mean something to the app: they are the
    /// session's own line numbers, not byte offsets into runs the app never
    /// laid out.
    pub fn cell_selection(&self) -> Option<crate::select::CellSelection> {
        self.core.cell_selection()
    }

    /// Opens a context menu; see `Core::open_menu`.
    pub fn open_menu(&mut self, menu: crate::menu::Menu) {
        self.core.open_menu(menu);
    }

    /// Closes whatever menu is open; see `Core::close_menu`.
    pub fn close_menu(&mut self) -> bool {
        self.core.close_menu()
    }

    /// Asks for the selection as text; see `Core::request_copy`.
    pub fn request_copy(&mut self) -> crate::select::CopyRequest {
        self.core.request_copy()
    }

    /// Answers a `typeahead` ask with a row's data index; see
    /// `Core::answer_typeahead`.
    pub fn answer_typeahead(&mut self, key: Key, index: u64) -> bool {
        self.core.answer_typeahead(key, index)
    }

    /// Answers a `selectionrange` ask; see `Core::answer_selection_range`.
    pub fn answer_selection_range(&mut self, text: &str) -> bool {
        self.core.answer_selection_range(text)
    }

    /// The selection as HTML; see `Core::selection_html`.
    pub fn selection_html(&self) -> Option<String> {
        self.core.selection_html()
    }

    /// Puts text on the system clipboard; see `Core::set_clipboard`.
    pub fn set_clipboard(&mut self, text: impl Into<String>, html: Option<String>) {
        self.core.set_clipboard(text, html);
    }

    /// Puts a picture on the system clipboard; see
    /// `Core::set_clipboard_image`.
    pub fn set_clipboard_image(
        &mut self,
        width: u32,
        height: u32,
        rgba: impl Into<std::sync::Arc<[u8]>>,
    ) -> bool {
        self.core.set_clipboard_image(width, height, rgba)
    }

    /// Puts files on the system clipboard, as a file manager's Copy does;
    /// see `Core::set_clipboard_files`.
    pub fn set_clipboard_files<I, P>(&mut self, paths: I) -> bool
    where
        I: IntoIterator<Item = P>,
        P: Into<String>,
    {
        self.core.set_clipboard_files(paths)
    }

    /// Puts a secret on the system clipboard marked concealed and
    /// transient, the way a password manager does; see
    /// `Core::set_clipboard_secret`.
    pub fn set_clipboard_secret(&mut self, text: impl Into<String>) {
        self.core.set_clipboard_secret(text);
    }

    /// Asks for the clipboard's text, delivered as a `text` event on the
    /// focused sink or as typing into the focused editor; see
    /// `Core::request_paste`.
    pub fn request_paste(&mut self) {
        self.core.request_paste();
    }

    /// Whether a paste asked for is still unanswered; see
    /// `Core::awaiting_paste`.
    pub fn awaiting_paste(&self) -> bool {
        self.core.awaiting_paste()
    }

    /// Asks the host for a file dialog; the answer is a `files` event to
    /// whoever's view asked. False when one is already outstanding. See
    /// `Core::request_files`.
    #[track_caller]
    pub fn request_files(&mut self, dialog: crate::dialog::FileDialog) -> bool {
        self.core.request_files(dialog)
    }

    /// `Core::awaiting_files`.
    pub fn awaiting_files(&self) -> bool {
        self.core.awaiting_files()
    }

    /// Selects everything in the scope `key` declared; see
    /// `Core::select_all_in`.
    pub fn select_all_in(&mut self, key: Key) -> bool {
        self.core.select_all_in(key)
    }

    /// Drops the window's selection; see `Core::clear_selection`.
    pub fn clear_selection(&mut self) -> bool {
        self.core.clear_selection()
    }

    /// The caret rect for a byte offset in the text node `key` drew; see
    /// `Core::caret_rect`.
    pub fn caret_rect(&self, key: Key, byte: usize) -> Option<Rect> {
        self.core.caret_rect(key, byte)
    }

    /// `measure_text` for a rich-text paragraph.
    pub fn measure_rich_text(
        &mut self,
        spans: &[Span<'_>],
        base: &TextStyle,
        max_w: Option<f32>,
    ) -> crate::text::TextMetrics {
        self.core.measure_rich_text(spans, base, max_w)
    }

    /// Where a byte of a text lands once wrapped at a width, without a
    /// node; see `Core::caret_in_text`. The rect is from the text's own
    /// top-left: add where the text node sits to place a caret over it on
    /// the frame its text is new, when [`Self::caret_rect`] still answers
    /// for the text it drew last frame.
    pub fn caret_in_text(
        &mut self,
        content: &str,
        style: &TextStyle,
        max_w: Option<f32>,
        byte: usize,
    ) -> Option<Rect> {
        self.core.caret_in_text(content, style, max_w, byte)
    }

    /// `caret_in_text` for a rich-text paragraph.
    pub fn caret_in_rich_text(
        &mut self,
        spans: &[Span<'_>],
        base: &TextStyle,
        max_w: Option<f32>,
        byte: usize,
    ) -> Option<Rect> {
        self.core.caret_in_rich_text(spans, base, max_w, byte)
    }

    /// Opens a node under the next auto key; its children follow until
    /// [`Self::close`]. [`Self::with`] is the scoped form, and the one to
    /// prefer: an `open` without its `close` is a `node-left-open`
    /// warning.
    #[inline]
    pub fn open(&mut self, spec: NodeSpec) -> Key {
        self.core.open(spec)
    }

    /// [`Self::open`] under a label key: stable across frames whatever
    /// the siblings before it, and what `key_of(label)` finds.
    #[inline]
    pub fn open_keyed(&mut self, label: &str, spec: NodeSpec) -> Key {
        self.core.open_keyed(label, spec)
    }

    /// Opens a node under a key the caller built rather than a label:
    /// `ui.child_key("gap").index(id)` is a label and an index with no
    /// string formatted and no clash with the sibling-index keys
    /// `open_indexed` gives, and a key kept from an earlier `child_key` —
    /// read for `is_hovered` first — is opened as it is instead of spelled
    /// twice. Build it from this parent's `child_key`: two nodes under one
    /// key in a frame are a `duplicate-key` warning, as two labels are.
    /// It has no label, so `key_of` does not find it.
    #[inline]
    pub fn open_key(&mut self, key: Key, spec: NodeSpec) -> Key {
        self.core.open_key(key, spec)
    }

    /// Scoped [`Self::open_key`].
    pub fn with_key(&mut self, key: Key, spec: NodeSpec, f: impl FnOnce(&mut Ui<'_>)) -> Key {
        self.open_key(key, spec);
        f(self);
        self.close();
        key
    }

    /// [`Self::leaf`] under a key the caller built; see [`Self::open_key`].
    #[inline]
    pub fn leaf_key(&mut self, key: Key, spec: NodeSpec) -> Key {
        self.open_key(key, spec);
        self.close();
        key
    }

    /// `open` under a key the caller chose — the panel's own nodes, whose
    /// keys another node anchors to by name before they exist.
    #[inline]
    pub(crate) fn open_with_key(&mut self, key: Key, label: &str, spec: NodeSpec) {
        self.core.open_with_key_named(key, label, spec)
    }

    /// `open_keyed` by sibling index: the key auto-keying would have given
    /// the `i`th child. A virtualizing list opens each row with its data
    /// index, so a row keeps its identity when the built range slides past
    /// it. See `Core::open_indexed`.
    #[inline]
    pub fn open_indexed(&mut self, i: u64, spec: NodeSpec) -> Key {
        self.core.open_indexed(i, spec)
    }

    /// Closes the node the last `open*` opened.
    #[inline]
    pub fn close(&mut self) {
        self.core.close();
    }

    /// Opens a node, runs `f` for its children and closes it. The usual
    /// way to declare a container:
    ///
    /// ```rust
    /// # use kui_core::{Core, NodeSpec, Size, TextStyle};
    /// # let mut core = Core::new();
    /// # let mut ui = core.frame(Size::new(200.0, 100.0), 1.0);
    /// ui.with(NodeSpec::row().gap(8.0).pad(4.0), |ui| {
    ///     ui.text("Name", TextStyle::new(14.0));
    ///     ui.text("Ada", TextStyle::new(14.0));
    /// });
    /// # ui.finish();
    /// ```
    pub fn with(&mut self, spec: NodeSpec, f: impl FnOnce(&mut Ui<'_>)) -> Key {
        let key = self.open(spec);
        f(self);
        self.close();
        key
    }

    /// A branch built only when it can be shown (ADR 0069, decision 13):
    /// `render` runs when the parent offers it `needs` — the parent's
    /// content box less its other in-flow children and gaps, in each
    /// axis `needs` names — and the branch is laid out at exactly that
    /// size; [`Lazy::fallback`] runs otherwise. The decision is taken
    /// from the last layout's room, and a frame whose layout says
    /// otherwise is built again before it is drawn
    /// ([`Core::rerun_owed`]).
    ///
    /// ```rust
    /// use kui_core::{Core, Needs, NodeSpec, Size, TextStyle};
    ///
    /// let mut core = Core::new();
    /// let view = |ui: &mut kui_core::Ui<'_>| {
    ///     ui.with(NodeSpec::row().grow_width(), |ui| {
    ///         ui.lazy(Needs::w(320.0), |ui| {
    ///             ui.text("the chart", TextStyle::new(14.0));
    ///         })
    ///         .fallback(|ui| {
    ///             ui.text("too narrow", TextStyle::new(14.0));
    ///         });
    ///     });
    /// };
    /// // The first frame lays out before it builds the branch.
    /// let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    /// view(&mut ui);
    /// ui.finish();
    /// assert!(core.rerun_owed());
    /// let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    /// view(&mut ui);
    /// ui.finish();
    /// assert!(!core.rerun_owed());
    /// ```
    pub fn lazy<'u>(
        &'u mut self,
        needs: crate::rules::Needs,
        render: impl FnOnce(&mut Ui<'_>),
    ) -> Lazy<'u, 'a> {
        self.lazy_with(crate::rules::LazySpec::needs(needs), render)
    }

    /// A branch chosen by a query (ADR 0069, decision 14): `render` runs
    /// while `query` holds over the parent's size (`width`, `height`), the
    /// window's or the environment's. A state condition in it is refused
    /// (`query-state`): a hover must not rebuild a subtree.
    pub fn lazy_query<'u>(
        &'u mut self,
        query: crate::rules::Cond,
        render: impl FnOnce(&mut Ui<'_>),
    ) -> Lazy<'u, 'a> {
        self.lazy_with(crate::rules::LazySpec::query(query), render)
    }

    /// [`Self::lazy`] or [`Self::lazy_query`] from a whole ask, with its
    /// `onQuery` message.
    pub fn lazy_with<'u>(
        &'u mut self,
        spec: crate::rules::LazySpec,
        render: impl FnOnce(&mut Ui<'_>),
    ) -> Lazy<'u, 'a> {
        let built = self.core.lazy_open(spec);
        if built == Some(true) {
            render(self);
        }
        Lazy { ui: self, built }
    }

    /// Scoped [`Self::open_keyed`].
    pub fn with_keyed(&mut self, label: &str, spec: NodeSpec, f: impl FnOnce(&mut Ui<'_>)) -> Key {
        let key = self.open_keyed(label, spec);
        f(self);
        self.close();
        key
    }

    /// A node with no children: a spacer, a rule, a swatch, a hit area —
    /// `with(spec, |_| {})` without the empty closure.
    #[inline]
    pub fn leaf(&mut self, spec: NodeSpec) -> Key {
        let key = self.open(spec);
        self.close();
        key
    }

    /// [`Self::leaf`] under a label key.
    #[inline]
    pub fn leaf_keyed(&mut self, label: &str, spec: NodeSpec) -> Key {
        let key = self.open_keyed(label, spec);
        self.close();
        key
    }

    /// [`Self::leaf`] under a data index; see [`Self::open_indexed`].
    #[inline]
    pub fn leaf_indexed(&mut self, i: u64, spec: NodeSpec) -> Key {
        let key = self.open_indexed(i, spec);
        self.close();
        key
    }

    /// Declares how many indexed rows the open node's virtual list has,
    /// built or not; see [`crate::Core::row_count`].
    pub fn row_count(&mut self, n: u64) {
        self.core.row_count(n);
    }

    /// Scoped `open_indexed`: the `i`th child's auto-key, given to a node
    /// that is not in the `i`th slot.
    pub fn with_indexed(&mut self, i: u64, spec: NodeSpec, f: impl FnOnce(&mut Ui<'_>)) -> Key {
        let key = self.open_indexed(i, spec);
        f(self);
        self.close();
        key
    }

    /// A paragraph of plain text in one style, wrapped at the width its
    /// parent gives it. A text has no box of its own (no padding,
    /// background, key or click); [`Self::text_in`] puts it in one.
    ///
    /// ```rust
    /// # use kui_core::{Color, Core, NodeSpec, Size, TextStyle};
    /// # let mut core = Core::new();
    /// # let mut ui = core.frame(Size::new(200.0, 100.0), 1.0);
    /// ui.text("Plain, in the theme's foreground", TextStyle::new(14.0));
    /// ui.text("Bold-ish, mono, red", TextStyle::new(13.0).mono().color(Color::hex(0xd43b3bff)));
    /// let badge = ui.text_in(NodeSpec::row().pad_xy(6.0, 2.0).radius(4.0), "3", TextStyle::new(12.0));
    /// # let _ = badge;
    /// # ui.finish();
    /// ```
    pub fn text(&mut self, content: &str, style: TextStyle) {
        self.core.text_node(content, style);
    }

    /// A box of `spec` holding one text: `with(spec, |ui| ui.text(…))` for
    /// the label, the cell, the badge that needs a width, a background, a
    /// click or a role. Returns the box's key.
    #[inline]
    pub fn text_in(&mut self, spec: NodeSpec, content: &str, style: TextStyle) -> Key {
        let key = self.open(spec);
        self.text(content, style);
        self.close();
        key
    }

    /// [`Self::text_in`] under a label key.
    #[inline]
    pub fn text_in_keyed(
        &mut self,
        label: &str,
        spec: NodeSpec,
        content: &str,
        style: TextStyle,
    ) -> Key {
        let key = self.open_keyed(label, spec);
        self.text(content, style);
        self.close();
        key
    }

    /// [`Self::text_in`] under a data index; see [`Self::open_indexed`].
    #[inline]
    pub fn text_in_indexed(
        &mut self,
        i: u64,
        spec: NodeSpec,
        content: &str,
        style: TextStyle,
    ) -> Key {
        let key = self.open_indexed(i, spec);
        self.text(content, style);
        self.close();
        key
    }

    /// A cell grid (a terminal's screen) as one node; see
    /// [`crate::cells`]. The spec is the node's own.
    pub fn cells(&mut self, grid: &crate::cells::CellGrid<'_>, spec: NodeSpec) {
        self.core.cells(grid, spec);
    }

    /// [`Self::cells`] under a data index; see [`Self::open_indexed`].
    pub fn cells_indexed(&mut self, i: u64, grid: &crate::cells::CellGrid<'_>, spec: NodeSpec) {
        self.core.cells_indexed(i, grid, spec);
    }

    /// [`Self::cells`] under a declared key.
    pub fn cells_keyed(&mut self, label: &str, grid: &crate::cells::CellGrid<'_>, spec: NodeSpec) {
        self.core.cells_keyed(label, grid, spec);
    }

    /// A paragraph of styled spans, shaped and wrapped as one flow.
    pub fn rich_text(&mut self, spans: &[Span<'_>], base: TextStyle) {
        self.core.rich_text_node(spans, base);
    }

    /// A registered image; see `Core::image_node` for sizing semantics.
    pub fn image(&mut self, id: crate::resources::ImageId, spec: NodeSpec) {
        self.core.image_node(id, spec);
    }

    /// [`Self::image`] with its `sampling` and `fit` rows; see
    /// `Core::image_node_with`.
    pub fn image_with(
        &mut self,
        id: crate::resources::ImageId,
        opts: crate::resources::ImageOpts,
        spec: NodeSpec,
    ) {
        self.core.image_node_with(id, opts, spec);
    }

    /// A box a registered WGSL function paints; see `Core::fragment_node`
    /// for what it is, and `Core::add_fragment` for where the handle comes
    /// from. It has no intrinsic size, so give it one.
    ///
    /// `frag` is the handle, or `handle.with_image(img)` for a function
    /// that reads a registered image through `kui_sample(uv)`: a
    /// waveform, a heatmap, an image effect.
    pub fn fragment(
        &mut self,
        frag: impl Into<crate::fragment::FragmentRef>,
        params: &[f32],
        spec: NodeSpec,
    ) -> Key {
        self.core.fragment_node(frag, params, spec)
    }

    /// A fragment holding children, which paint over it: a gradient card
    /// with a title and buttons on top of it.
    pub fn fragment_with(
        &mut self,
        frag: impl Into<crate::fragment::FragmentRef>,
        params: &[f32],
        spec: NodeSpec,
        f: impl FnOnce(&mut Ui<'_>),
    ) -> Key {
        let key = self.core.open_fragment(frag, params, spec);
        f(self);
        self.core.close();
        key
    }

    /// [`Self::fragment`] under a label key, for one that transitions or
    /// exits and needs a stable identity across frames.
    pub fn fragment_keyed(
        &mut self,
        label: &str,
        frag: impl Into<crate::fragment::FragmentRef>,
        params: &[f32],
        spec: NodeSpec,
    ) -> Key {
        self.core.fragment_node_keyed(label, frag, params, spec)
    }

    /// [`Self::fragment_with`] under a label key.
    pub fn fragment_with_keyed(
        &mut self,
        label: &str,
        frag: impl Into<crate::fragment::FragmentRef>,
        params: &[f32],
        spec: NodeSpec,
        f: impl FnOnce(&mut Ui<'_>),
    ) -> Key {
        let key = self.core.open_fragment_keyed(label, frag, params, spec);
        f(self);
        self.core.close();
        key
    }

    /// [`Self::fragment`] under a data index; see [`Self::open_indexed`].
    pub fn fragment_indexed(
        &mut self,
        i: u64,
        frag: impl Into<crate::fragment::FragmentRef>,
        params: &[f32],
        spec: NodeSpec,
    ) -> Key {
        let key = self.core.open_fragment_indexed(i, frag, params, spec);
        self.core.close();
        key
    }

    /// [`Self::fragment_with`] under a data index.
    pub fn fragment_with_indexed(
        &mut self,
        i: u64,
        frag: impl Into<crate::fragment::FragmentRef>,
        params: &[f32],
        spec: NodeSpec,
        f: impl FnOnce(&mut Ui<'_>),
    ) -> Key {
        let key = self.core.open_fragment_indexed(i, frag, params, spec);
        f(self);
        self.core.close();
        key
    }

    /// A round-capped segment from `from` to `to`, in the parent's box
    /// space; see `Core::line_node` for what it is and is not.
    #[inline]
    pub fn line(&mut self, from: Vec2, to: Vec2, stroke: Stroke, spec: NodeSpec) {
        self.core.line_node(&[from, to], stroke, spec);
    }

    /// [`Self::line`] under a label key.
    pub fn line_keyed(
        &mut self,
        label: &str,
        from: Vec2,
        to: Vec2,
        stroke: Stroke,
        spec: NodeSpec,
    ) {
        self.core.line_node_keyed(label, &[from, to], stroke, spec);
    }

    /// [`Self::line`] under a data index; see [`Self::open_indexed`].
    pub fn line_indexed(&mut self, i: u64, from: Vec2, to: Vec2, stroke: Stroke, spec: NodeSpec) {
        self.core.line_node_indexed(i, &[from, to], stroke, spec);
    }

    /// A stroke through `points`: a polyline, or a smooth curve through
    /// them with [`Stroke::curve`]; see `Core::line_node`.
    #[inline]
    pub fn polyline(&mut self, points: &[Vec2], stroke: Stroke, spec: NodeSpec) {
        self.core.line_node(points, stroke, spec);
    }

    /// [`Self::polyline`] under a data index; see [`Self::open_indexed`].
    pub fn polyline_indexed(&mut self, i: u64, points: &[Vec2], stroke: Stroke, spec: NodeSpec) {
        self.core.line_node_indexed(i, points, stroke, spec);
    }

    /// [`Self::polyline`] under a label key.
    pub fn polyline_keyed(&mut self, label: &str, points: &[Vec2], stroke: Stroke, spec: NodeSpec) {
        self.core.line_node_keyed(label, points, stroke, spec);
    }

    /// A filled polygon through `points` in the parent's box space, the
    /// fill in `spec`'s `bg`; see `Core::polygon_node` for what it is and
    /// is not.
    pub fn polygon(&mut self, points: &[Vec2], spec: NodeSpec) {
        self.core.polygon_node(points, spec);
    }

    /// [`Self::polygon`] under a label key.
    pub fn polygon_keyed(&mut self, label: &str, points: &[Vec2], spec: NodeSpec) {
        self.core.polygon_node_keyed(label, points, spec);
    }

    /// [`Self::polygon`] under a data index; see [`Self::open_indexed`].
    pub fn polygon_indexed(&mut self, i: u64, points: &[Vec2], spec: NodeSpec) {
        self.core.polygon_node_indexed(i, points, spec);
    }

    /// A path — any outline — filled with `spec`'s `bg` and stroked by
    /// the path's own stroke; see `Core::path_node` for what it is and
    /// is not.
    pub fn path(&mut self, path: &crate::path::Path, spec: NodeSpec) {
        self.core.path_node(path, spec);
    }

    /// An SVG in a box: paths when it can, an image when it cannot; see
    /// `Core::svg_node`.
    ///
    /// ```rust,ignore
    /// ui.svg(SvgSource::Data(ICON), SvgOpts::default(), NodeSpec::new().size(16.0, 16.0).label("Search"));
    /// ```
    pub fn svg(
        &mut self,
        src: crate::runtime::SvgSource<'_>,
        opts: crate::runtime::SvgOpts,
        spec: NodeSpec,
    ) -> Key {
        self.core.svg_node(src, opts, spec)
    }

    /// [`Self::svg`] under a label key.
    pub fn svg_keyed(
        &mut self,
        label: &str,
        src: crate::runtime::SvgSource<'_>,
        opts: crate::runtime::SvgOpts,
        spec: NodeSpec,
    ) -> Key {
        self.core.svg_node_keyed(label, src, opts, spec)
    }

    /// [`Self::path`] under a label key.
    pub fn path_keyed(&mut self, label: &str, path: &crate::path::Path, spec: NodeSpec) {
        self.core.path_node_keyed(label, path, spec);
    }

    /// [`Self::path`] under a data index; see [`Self::open_indexed`].
    pub fn path_indexed(&mut self, i: u64, path: &crate::path::Path, spec: NodeSpec) {
        self.core.path_node_indexed(i, path, spec);
    }

    /// [`Self::path`] from SVG path data; data that does not parse raises
    /// `path-malformed` and draws nothing. See `Core::path_d_node`.
    pub fn path_d(
        &mut self,
        d: &str,
        rule: crate::path::FillRule,
        stroke: Option<Stroke>,
        turn: Option<crate::path::Turn>,
        spec: NodeSpec,
    ) {
        self.core.path_d_node(d, rule, stroke, turn, spec);
    }

    /// [`Self::path_d`] under a label key.
    pub fn path_d_keyed(
        &mut self,
        label: &str,
        d: &str,
        rule: crate::path::FillRule,
        stroke: Option<Stroke>,
        turn: Option<crate::path::Turn>,
        spec: NodeSpec,
    ) {
        self.core
            .path_d_node_keyed(label, d, rule, stroke, turn, spec);
    }

    /// An `audio` node: a playback retained for as long as the view keeps
    /// declaring it; see `Core::audio_node`.
    pub fn audio(&mut self, spec: crate::audio::AudioSpec) -> Key {
        self.core.audio_node(spec)
    }

    /// `audio` with a label-derived key; see `Core::audio_node_keyed`.
    pub fn audio_keyed(&mut self, label: &str, spec: crate::audio::AudioSpec) -> Key {
        self.core.audio_node_keyed(label, spec)
    }

    /// Starts a playback from a view; see `Core::play`. Views run every
    /// frame, so gate it on state that changes once (or use `audio`).
    pub fn play(
        &mut self,
        sound: crate::resources::SoundId,
        opts: crate::audio::PlayOptions,
    ) -> crate::audio::PlaybackId {
        self.core.play(sound, opts)
    }

    /// An editable text node; state retained by key. See `Core::text_edit`.
    pub fn text_edit(
        &mut self,
        label: &str,
        initial: &str,
        opts: &EditOptions,
        spec: NodeSpec,
    ) -> Key {
        self.core.text_edit(label, initial, opts, spec)
    }

    /// An editable text node whose text is `value`, declared every frame:
    /// [`Self::text_edit`] with [`EditOptions::value`] set, so where the
    /// editor's text differs from `value` it is brought to it as a
    /// splice. An app that stores the text its `on_change` message brings
    /// (`change: {text}`) and declares it back never moves the
    /// caret; one that declares something else (a filter, a reset) wins.
    pub fn text_edit_value(
        &mut self,
        label: &str,
        value: &str,
        opts: &EditOptions,
        spec: NodeSpec,
    ) -> Key {
        let opts = &EditOptions {
            value: true,
            ..opts.clone()
        };
        self.core.text_edit(label, value, opts, spec)
    }

    /// Whether `key` holds keyboard focus — any node: an editor, a key
    /// sink, a button Tab landed on (see `Core::focus`).
    pub fn is_focused(&self, key: Key) -> bool {
        self.core.is_focused(key)
    }

    /// The node holding keyboard focus, if any.
    pub fn focused(&self) -> Option<Key> {
        self.core.focus()
    }

    /// Whether focus got where it is by keyboard or assistive technology
    /// (a Tab press, a reader's request) rather than a click — when a
    /// view that styles its own focus should show it.
    pub fn focus_visible(&self) -> bool {
        self.core.focus_visible()
    }

    /// Whether the window's "Tab moves focus" toggle is on: a multiline
    /// editor whose Tab would type walks the ring instead (see
    /// [`Core::tab_moves_focus`]). What a status label reads.
    pub fn tab_moves_focus(&self) -> bool {
        self.core.tab_moves_focus()
    }

    /// The platform the core says it runs on, what a `platform` condition
    /// reads; see `Core::platform` (backlog C116).
    pub fn platform(&self) -> crate::rules::Platform {
        self.core.platform()
    }

    /// Sets the appearance this origin's `dark` / `light` conditions read;
    /// see `Core::set_rules_appearance` (backlog C113).
    pub fn set_rules_appearance(&mut self, appearance: crate::env::Appearance) {
        self.core.set_rules_appearance(appearance)
    }

    /// The key of the node opened under the key label `label` — in this
    /// frame so far, then in the last finished one. The key label is the
    /// name the view opened the node under (`with_keyed`, a `key` prop),
    /// not the accessible name its `label` row gives a reader, which
    /// `Core::key_named` looks up. For a caller that holds only the label
    /// and cannot spell the path (`child_key` is the same question asked
    /// from the parent); see `Core::key_of`.
    pub fn key_of(&mut self, label: &str) -> Option<Key> {
        self.core.key_of(label)
    }

    /// Moves keyboard focus to `key` now (an editor, an `on_key` sink, a
    /// control, a `focusable` node); see `Core::set_focus`.
    pub fn focus(&mut self, key: Key) {
        self.core.set_focus(Some(key));
    }

    /// Drops keyboard focus.
    pub fn blur(&mut self) {
        self.core.set_focus(None);
    }

    /// Moves focus to the next focusable node in tree order, wrapping —
    /// what Tab does. A key sink that binds Tab itself calls this to hand
    /// the keyboard on.
    ///
    /// Deferred, unlike the rest of this handle: a `Ui` only exists while a
    /// frame is being built, and `begin_frame` cleared the tree the Tab
    /// ring is made of, so stepping now would walk an empty ring. The step
    /// is applied at `finish`, against the frame this call is part of — so
    /// a view that declares three rows and asks to step lands on one of
    /// them, without waiting a frame for them to exist. Outside a frame
    /// (a driver handling a key press) `Core::focus_next` steps at once.
    #[track_caller]
    pub fn focus_next(&mut self) {
        self.core.request_focus_step(true);
    }

    /// Shift-Tab: the previous focusable node. Deferred to `finish` for the
    /// reason [`Ui::focus_next`] gives.
    #[track_caller]
    pub fn focus_prev(&mut self) {
        self.core.request_focus_step(false);
    }

    /// Enters a focus region (the node `key` names, declared
    /// `focus_region`), or the main ring for `None`: focus lands on what
    /// that ring
    /// last held if the node is still there, else its `initial_focus`,
    /// else its first stop, and shows. Deferred to `finish` like
    /// [`Ui::focus_next`], so a view may name the region it is declaring
    /// right now — the dock this frame toggles on. A key the frame does
    /// not declare as a region raises `focus-region-without-node`.
    #[track_caller]
    pub fn focus_region(&mut self, key: Option<Key>) {
        self.core.focus_region(key);
    }

    /// Focuses item `index` of the list, table or other composite `key`
    /// names (backlog C125): the item whose data `index` that is —
    /// reaching for a row a windowed composite did not build, as an arrow
    /// does — else the item at that place, where items carry no index.
    /// Deferred to `finish` like [`Ui::focus_region`], so a view may name
    /// an item it is declaring now. See `Core::focus_item`.
    pub fn focus_item(&mut self, key: Key, index: u64) {
        self.core.focus_item(key, index);
    }

    /// The focus region in effect — the node whose ring Tab walks — or
    /// `None` for the main ring. What a chord that toggles between a dock
    /// and the app reads to know which way it is going.
    pub fn region(&self) -> Option<Key> {
        self.core.region()
    }

    /// An editor's current text, by its key; `None` for a key no editor
    /// holds.
    pub fn edit_text(&self, key: Key) -> Option<String> {
        self.core.edit_text(key)
    }

    /// Replaces an editor's text, caret at the end (`Core::set_edit_text`).
    /// Returns whether it reached an editor now, or was held for the
    /// frame that declares the key.
    pub fn set_edit_text(&mut self, key: Key, text: &str) -> bool {
        self.core.set_edit_text(key, text)
    }

    /// The same by the label the view declares, for a caller with no key
    /// yet: an `update` opening a field the editor has not fired an
    /// event from (`Core::set_edit_text_by_label`).
    pub fn set_edit_text_by_label(&mut self, label: &str, text: &str) -> bool {
        self.core.set_edit_text_by_label(label, text)
    }

    /// Says something once, with no node behind it (`Core::announce`).
    /// Takes effect at once, unlike `focus_next`: the queue is not made of
    /// a finished tree.
    ///
    /// A view runs every frame, so a call made from here needs a guard the
    /// app clears; the core reports the unguarded case as
    /// `announcement-repeated`. A region whose message is on screen is the
    /// `live` prop instead.
    pub fn announce(&mut self, text: &str, live: crate::access::Live) {
        self.core.announce(text, live);
    }

    /// Scrolls whatever contains `key` so the node shows — "scroll to the
    /// selected row", without the container geometry the app cannot see.
    /// Resolved when this frame finishes laying out, so a row the view is
    /// declaring right now reveals fine; a key the frame does not declare,
    /// or one nothing scrollable contains, is a no-op. See `Core::reveal`.
    #[track_caller]
    pub fn reveal(&mut self, key: Key) {
        self.core.reveal(key);
    }

    /// The handle for an installed or loaded font family by name (what
    /// `family = "Name"` resolves to in the declarative bindings), so a
    /// Rust view names a face without reaching for the core. `None` when
    /// no face matches. Idempotent.
    pub fn system_font(&mut self, name: &str) -> Option<crate::resources::FontId> {
        self.core.add_system_font(name)
    }

    /// [`Self::reveal`] by label, resolved when this frame finishes; see
    /// `Core::reveal_label`.
    #[track_caller]
    pub fn reveal_label(&mut self, label: &str) {
        self.core.reveal_label(label);
    }

    /// `set_scroll` by label, resolved before this frame lays out; see
    /// `Core::set_scroll_label`.
    #[track_caller]
    pub fn set_scroll_label(&mut self, label: &str, offset: Vec2) {
        self.core.set_scroll_label(label, offset);
    }

    /// A scroll container's retained offset, clamped as of the last
    /// layout — the number to stash in a model and hand back to
    /// `set_scroll` later. Zero for a node that never scrolled.
    pub fn scroll_offset(&self, key: Key) -> Vec2 {
        self.core.scroll_offset(key)
    }

    /// Sets that offset, the way the wheel would: `Vec2::ZERO` jumps to
    /// the top, a large value to the end (the next layout clamps it).
    #[track_caller]
    pub fn set_scroll(&mut self, key: Key, offset: Vec2) {
        self.core.set_scroll(key, offset);
    }

    /// Moves a container's scroll state by the content that moved under
    /// it (`drawn` for the drawn place and an eased leg's start, `target`
    /// for the offset) with no ease asked or ended: a variable-height
    /// list's height correction. See `Core::shift_scroll`.
    pub fn shift_scroll(&mut self, key: Key, drawn: Vec2, target: Vec2) {
        self.core.shift_scroll(key, drawn, target);
    }

    /// Scrolls the container `key` for a drag the app holds, when the
    /// pointer is in the edge zone inside `rect` or past it; see
    /// [`Core::edge_scroll`]. Returns what it moved this frame.
    #[track_caller]
    pub fn edge_scroll(&mut self, key: Key, pointer: Vec2, rect: Rect) -> Option<Vec2> {
        self.core.edge_scroll(key, pointer, rect)
    }

    /// What the last layout resolved for a scroll container — its box, its
    /// content size and the clamped offset — so a view can build only the
    /// rows that fit and two spacers instead of ten thousand rows. `None`
    /// until a layout has resolved `key` as a container. It describes the
    /// previous frame; see `Core::scroll_geometry`, or
    /// `widgets::uniform_list` for the uniform-row case.
    pub fn scroll_geometry(&self, key: Key) -> Option<crate::scroll::ScrollGeometry> {
        self.core.scroll_geometry(key)
    }

    /// The rect the last frame laid an `on_layout` node out at; see
    /// `Core::layout_of`.
    pub fn layout_of(&self, key: Key) -> Option<Rect> {
        self.core.layout_of(key)
    }

    /// Where the pointer is over a node — `x`, `y` from its top-left and
    /// `w`, `h` its size — as of the last frame; see `Core::pointer_in`
    /// (backlog C119).
    pub fn pointer_in(&self, key: Key) -> Option<Rect> {
        self.core.pointer_in(key)
    }

    /// Declares this node focused: it takes keyboard focus when the
    /// declaration *starts* (the first frame it is made), and a Tab press
    /// afterwards is not clobbered by the view repeating it. An `on_key`
    /// sink then gets presses in `on_event` as its message with `key: {code,
    /// ctrl, alt, shift, super, text, repeat}` in it. To move focus at any
    /// time, `focus(key)`.
    pub fn take_key_focus(&mut self, key: Key) {
        self.core.set_key_focus(Some(key));
    }

    /// The node holding keyboard focus (the same as `focused`).
    pub fn key_focus(&self) -> Option<Key> {
        self.core.key_focus()
    }

    /// Asks the frame driver to apply a window command (close from a
    /// keymap, minimize from a command line).
    pub fn window_command(&mut self, cmd: WindowCommand) {
        self.core.push_window_command(cmd);
    }

    /// Asks the driver to resize a window (logical px) — a request applied
    /// on the driver's next pump and ignored headlessly, not a declaration:
    /// the user owns a window's size once it exists. `ui.env().window.id`
    /// is the window this view is drawing. See `Core::set_window_size`.
    pub fn set_window_size(&mut self, window: WindowId, size: Size) {
        self.core.set_window_size(window, size);
    }

    /// Asks the driver to give a window keyboard focus; queued the same
    /// way. See `Core::focus_window`.
    pub fn focus_window(&mut self, window: WindowId) {
        self.core.focus_window(window);
    }

    /// Runs layout and emission; results land in `Core::output()`. A frame
    /// begun with a filler lets it finish first: the `"root"` fill, unless
    /// the view declared it, and the `unknown-slot` check. An open context
    /// menu is drawn after both, which is what makes it the frame's modal
    /// scope and its topmost float.
    pub fn finish(self) {
        let Ui { core, filler, .. } = self;
        core.perf_build_done();
        core.check_unclosed();
        core.check_layout_misses();
        // Not in the devtools' own window: nothing the host or an
        // extension declares there is built.
        if let Some(filler) = filler {
            if !core.devtools_window() {
                // A declared tab's extension fill first — a layer
                // anchored to a body the panel builds below, so the
                // filler's own `unknown-slot` check sees the slot
                // declared — then the root fills.
                core.devtools_fill_mount(filler);
                filler.finish(&mut Ui::new(core));
            }
            // The panel, after the fills and before the menu: the menu is
            // the last thing declared and so the topmost float.
            core.devtools_finish();
            if core.devtools_window() {
                // The panel's own window: the extension form's fill lands
                // here too, when the pane gave the frame a filler.
                core.devtools_fill_mount(filler);
            }
        } else {
            core.devtools_finish();
        }
        Core::build_menu(&mut Ui::new(core));
        core.finish_frame();
    }
}

/// A lazy branch's node, open until this is dropped (`Ui::lazy`): what
/// [`Lazy::fallback`] builds into when `render` was not chosen.
pub struct Lazy<'u, 'a> {
    ui: &'u mut Ui<'a>,
    built: Option<bool>,
}

impl Lazy<'_, '_> {
    /// Builds `fallback` when the branch was not chosen — and not on the
    /// first frame, before any layout, when neither is.
    pub fn fallback(self, fallback: impl FnOnce(&mut Ui<'_>)) {
        if self.built == Some(false) {
            fallback(self.ui);
        }
    }

    /// Which branch was built: `Some(true)` for `render`, `Some(false)`
    /// for `fallback`, `None` before any layout.
    pub fn built(&self) -> Option<bool> {
        self.built
    }
}

impl Drop for Lazy<'_, '_> {
    fn drop(&mut self) {
        self.ui.core.lazy_close();
    }
}

/// A fill an instance is drawn by in place (`Ui::component_by`).
type InstanceFill<'f> = &'f mut dyn FnMut(&Instance<'_>, &mut Ui<'_>);

/// A placement's children as the fill sees them: the host's builder, run
/// with the placer's own children in reach (ADR 0060).
struct Placed<'p, 'q> {
    inner: &'p mut dyn Children,
    outer: Option<&'q mut dyn Children>,
}

impl Children for Placed<'_, '_> {
    fn build(&mut self, group: &str, ui: &mut Ui<'_>) {
        let mut inner = Ui::parts(
            &mut *ui.core,
            ui.filler.as_deref_mut().map(shorter_fill),
            self.outer.as_deref_mut().map(shorter_children),
        );
        self.inner.build(group, &mut inner);
    }
}

/// A filler borrowed for as long as the `Ui` it goes into: the trait
/// object's lifetime shortened at a coercion site, since a `Ui` has one.
pub(crate) fn shorter_fill<'s>(f: &'s mut (dyn Fill + '_)) -> &'s mut (dyn Fill + 's) {
    f
}

/// [`shorter_fill`] for the host's children.
pub(crate) fn shorter_children<'s>(c: &'s mut (dyn Children + '_)) -> &'s mut (dyn Children + 's) {
    c
}
