//! Slots: the places a host declares in its own view for an extension to
//! fill, with parameters in and replies out.
//!
//! A slot is a position, not a node. `Ui::slot` (or `slot_with`) is a call
//! the host makes anywhere among its children, and whatever fills it draws
//! then and there, as children of the node the host is inside. The filling
//! is done by whatever implements [`Fill`], which the frame was begun with
//! (`Core::frame_with`); the windowed runner hands in its [`Extensions`].
//! An app that loads no extensions never meets this module.
//!
//! Slot names are namespaced, and the host decides the namespace. An
//! extension names the slots it fills in its own vocabulary (`"panel"`,
//! `"status"`, no `/` in them). The host gives each extension a namespace
//! when it loads it ([`Extensions::push_as`]; [`Extensions::push`] uses the
//! extension's own name) and declares slots by their full name:
//! `ui.slot("fs/panel")` is the `"panel"` of the extension the host calls
//! `fs`. The same plugin loaded twice is two namespaces with two sets of
//! slots.
//!
//! ```rust
//! use kui_core::slot::{full_name, split_name, Extensions, ROOT_SLOT};
//!
//! assert_eq!(full_name("fs", "panel"), "fs/panel");
//! assert_eq!(split_name("left/fs/panel"), ("left/fs", "panel"));
//! assert_eq!(split_name("root"), ("", ROOT_SLOT));
//!
//! // The runner's list; `push_as(namespace, Box<dyn Extension>)` loads one.
//! let exts = Extensions::new();
//! assert!(exts.is_empty());
//! ```
//!
//! The reserved slot name `"root"` ([`ROOT_SLOT`]) is what an extension
//! listing no slots fills: `ns/root`, once after the host's view. A host
//! that declares `ui.slot("ns/root")` itself moves that fill to the
//! position it chose.
//!
//! An extension may host extensions of its own, by the same mechanism one
//! level down: [`Fill::add`] loads one while a frame is being built, under
//! a namespace of its own in the same list; a guest's `ui.slot(..)`
//! declares a slot like anyone's, except that it cannot fill itself (a
//! cycle is the `recursive-slot` warning and an empty position); and
//! replies go to whoever declared the slot ([`Extensions::route`]), which
//! for every extension the host declared is the host.
//!
//! A **component** (ADR 0060) is a slot made cheap and repeatable: an
//! extension lists the components it provides ([`Extension::components`]),
//! and a host places one with `Ui::component` as many times as it likes,
//! each instance keyed by where it was placed. An instance can hold the
//! host's own children — the fill marks where with `Ui::children(group)`,
//! and the host's [`Children`] builds them under the host's origin — and
//! its events reach the host as the messages the host passed in its
//! props: the extension answers an event with [`emit`], and
//! [`Extensions::route`] delivers the host's message for that prop.

use crate::input::UiEvent;
use crate::key::Key;
use crate::runtime::Extension;
use crate::tree::OriginId;
use crate::ui::Ui;
use crate::value::Value;

/// Which slot an extension is filling, handed to `Extension::view`.
#[derive(Clone, Copy, Debug)]
pub struct Slot<'a> {
    /// The name in the extension's own vocabulary — what it listed in
    /// `slots`, or `"root"` for the fill after the host's view.
    pub name: &'a str,
    /// The namespace the host gave this extension: what makes `name` a
    /// full slot name (`namespace/name`), and what tells one instance of
    /// a plugin loaded twice from the other.
    pub namespace: &'a str,
    /// What the host passed with `slot_with`; `Value::Null` for `slot`.
    /// Declared every frame and never retained — read it, do not keep it.
    pub params: &'a Value,
    /// The slot's own key, `enclosing.str("namespace/name")`: what
    /// `key_of` answers for the full name, and the namespace the
    /// extension's nodes are keyed under.
    pub key: Key,
}

/// The reserved slot name an extension listing none fills.
pub const ROOT_SLOT: &str = "root";

/// What separates a namespace from a slot name in a full name. A
/// namespace may contain it (the host chooses namespaces, and `"left/fs"`
/// is a fine one); a slot name an extension lists may not, so a full name
/// splits at its last one.
pub const NAMESPACE_SEPARATOR: char = '/';

/// `Value::Null` with a `'static` address, for a slot with no params.
pub static NULL_PARAMS: Value = Value::Null;

/// The host's children of a component instance, by group: what a fill's
/// `Ui::children(group)` builds, at the place the fill marked, under the
/// host's origin. A closure `|group, ui| …` is one; the unnamed group is
/// `""`.
pub trait Children {
    fn build(&mut self, group: &str, ui: &mut Ui<'_>);
}

impl<F: FnMut(&str, &mut Ui<'_>)> Children for F {
    fn build(&mut self, group: &str, ui: &mut Ui<'_>) {
        self(group, ui)
    }
}

/// The reply an extension answers an event with to raise one of its
/// component's events (ADR 0060): `prop` names the event prop the host
/// passed its message in (`"onSelect"`, or Lua's `"on_select"`), `fields`
/// what rides with it (`Value::Null` for nothing). [`Extensions::route`]
/// delivers the host's message for that prop, on the instance the event
/// came from, as an [`EventKind::Component`](crate::input::EventKind)
/// event; a prop the host left out delivers nothing.
pub fn emit(prop: &str, fields: Value) -> Value {
    Value::map([(EMIT_PROP, Value::str(prop)), (EMIT_FIELDS, fields)])
}

/// The two keys an [`emit`] reply is spelled with: reserved, so a reply
/// of the extension's own cannot be read as one.
pub const EMIT_PROP: &str = "$emit";
pub const EMIT_FIELDS: &str = "$fields";

/// An [`emit`] reply's prop and fields, or `None` for any other reply.
pub fn emit_of(reply: &Value) -> Option<(&str, &Value)> {
    let prop = reply.get_str(EMIT_PROP)?;
    Some((prop, reply.get(EMIT_FIELDS).unwrap_or(&NULL_PARAMS)))
}

/// Whether a component prop is an event prop: `on` and then an upper-case
/// letter (`onSelect`) or an underscore (Lua's `on_select`).
pub fn is_event_prop(name: &str) -> bool {
    let b = name.as_bytes();
    b.len() > 2 && b.starts_with(b"on") && (b[2].is_ascii_uppercase() || b[2] == b'_')
}

/// Whether event prop `prop` (either spelling) is the one whose field is
/// `field` — `onSelect` and `on_select` both match `select` — without
/// building the field.
pub fn prop_is_field(prop: &str, field: &str) -> bool {
    let rest = prop.strip_prefix("on").unwrap_or(prop);
    let rest = rest.strip_prefix('_').unwrap_or(rest);
    let mut a = rest.chars();
    let mut b = field.chars();
    match (a.next(), b.next()) {
        (Some(x), Some(y)) => x.to_lowercase().eq(std::iter::once(y)) && a.eq(b),
        (None, None) => true,
        _ => false,
    }
}

/// The field an event prop's data rides in on the host's message:
/// `onSelect` and `on_select` are both `select`.
pub fn event_field(prop: &str) -> String {
    let rest = prop.strip_prefix("on").unwrap_or(prop);
    let rest = rest.strip_prefix('_').unwrap_or(rest);
    let mut out = String::with_capacity(rest.len());
    let mut chars = rest.chars();
    if let Some(c) = chars.next() {
        out.extend(c.to_lowercase());
    }
    out.extend(chars);
    out
}

/// How many times a reply may be answered by another reply before the
/// rest go to the host instead ([`Extensions::route`]). Nesting is a few
/// levels deep in anything sane; this is the bound that keeps two
/// extensions answering each other from being an infinite loop.
pub const MAX_REPLY_HOPS: usize = 16;

impl Slot<'_> {
    /// The `"root"` slot as a test drives an extension without a runner:
    /// no namespace, no params, keyed under the root. The runner's own
    /// root fill is `Slot { name: "root", namespace: <the host's> }`, keyed
    /// under `"ns/root"`.
    pub fn root() -> Slot<'static> {
        Slot {
            name: ROOT_SLOT,
            namespace: "",
            params: &NULL_PARAMS,
            key: Key::ROOT.str(ROOT_SLOT),
        }
    }

    /// The full name the host declares: `namespace/name`, or just `name`
    /// for a slot with no namespace (`Slot::root`).
    pub fn full_name(&self) -> String {
        full_name(self.namespace, self.name)
    }
}

/// `namespace/name`, or `name` alone when the namespace is empty.
pub fn full_name(namespace: &str, name: &str) -> String {
    if namespace.is_empty() {
        name.to_owned()
    } else {
        format!("{namespace}{NAMESPACE_SEPARATOR}{name}")
    }
}

/// The one entry in `Extension::slots` that means "every name the host
/// declares under my namespace": for an extension that learns its slots
/// after it loads. `fill` matches any declared name against it and
/// `finish` has nothing to warn about for it.
pub const ANY_SLOT: &str = "*";

/// Splits a full slot name at its last separator into (namespace, name);
/// a name with none has the empty namespace.
pub fn split_name(full: &str) -> (&str, &str) {
    match full.rfind(NAMESPACE_SEPARATOR) {
        Some(i) => (&full[..i], &full[i + 1..]),
        None => ("", full),
    }
}

/// What fills slots: the runner's extension list, or a test's stand-in.
/// A frame begun with `Core::frame_with` carries one; `Ui::slot` calls
/// [`Fill::fill`] at the position the host declared, and `Ui::finish`
/// calls [`Fill::finish`] once the host's view is done.
pub trait Fill {
    /// Fill the slot declared as `full_name` now, at the cursor, with
    /// `key` its key: for the extension it names,
    /// `ui.fill(origin, slot, |ui| ext.view(slot, ui))`, which is where
    /// the namespace and the bound come from.
    fn fill(&mut self, full_name: &str, key: Key, params: &Value, ui: &mut Ui<'_>);
    /// After the host's view: fill every `ns/root` the host did not
    /// declare, and warn about every slot an extension names that
    /// nothing declared this frame.
    fn finish(&mut self, ui: &mut Ui<'_>);
    /// Loads `ext` under `namespace`, mid-frame, and answers with the
    /// origin it got — what a guest that hosts guests of its own calls
    /// (`env.add_extension` in Lua). It joins the same list as everyone
    /// else, so `namespace` has to be free of the host's names too.
    ///
    /// The default refuses: a `Fill` that is not a list of extensions has
    /// nowhere to put one.
    fn add(&mut self, namespace: &str, ext: Box<dyn Extension>) -> Result<OriginId, String> {
        let _ = ext;
        Err(format!(
            "cannot load `{namespace}`: this frame was begun with something other than an \
             extension list to fill its slots"
        ))
    }
    /// Fill the component instance placed as `full_name` at `key` now,
    /// with `props` as the host passed them and `children` building the
    /// host's own children where the fill asks (ADR 0060); answers
    /// whether anything provides the component. The default provides
    /// none.
    fn component(
        &mut self,
        full_name: &str,
        key: Key,
        props: &Value,
        children: &mut dyn Children,
        ui: &mut Ui<'_>,
    ) -> bool {
        let _ = (full_name, key, props, children, ui);
        false
    }
    /// An instance was placed this frame, filled or replayed: what lets
    /// [`Extensions::route`] find the host's message for an event prop
    /// after the frame. `slot` is the slot the placer's own nodes are in
    /// (`None` for the host's view). Answers whether the instance heard
    /// one of its own events since it was last filled, which a replay
    /// cannot know about ([`SlotFill::Heard`](crate::SlotFill::Heard)).
    /// The default keeps nothing and heard nothing.
    fn placed(
        &mut self,
        full_name: &str,
        key: Key,
        props: &Value,
        placer: OriginId,
        slot: Option<Key>,
    ) -> bool {
        let _ = (full_name, key, props, placer, slot);
        false
    }
}

/// One loaded extension: its namespace, itself, and the answers of its
/// that are read while it is busy.
struct Entry {
    namespace: String,
    /// `None` for exactly as long as this extension is filling a slot:
    /// `fill_one` takes it out so the list is free to fill the slots the
    /// extension itself declares, and puts it back after. A slot of its
    /// own that it declares while filling therefore finds nobody, which
    /// is the cycle warning rather than a borrow panic.
    ext: Option<Box<dyn Extension>>,
    /// Second handles onto the extension (`Extension::reentry`), for a
    /// component of its own placed while it draws — inside a host's
    /// children of one of its instances. One deeper than the deepest
    /// nesting so far; each comes back after its fill.
    spares: Vec<Box<dyn Extension>>,
    /// `Extension::name` and `Extension::slots` as they answered at load.
    /// Copied because both are read while `ext` is taken — and because
    /// every binding already reads `slots` once, at load, so there was
    /// never a second answer to miss.
    name: String,
    slots: Vec<String>,
    /// `Extension::components` as it answered at load (ADR 0060).
    components: Vec<String>,
    /// The origin that declared the slot this extension last filled:
    /// `OriginId::HOST` for one in the host's own view, another
    /// extension's when that extension declared it. Where its replies go
    /// (see `route`).
    asked_by: OriginId,
}

/// The runner's extensions, each under the namespace the host gave it.
/// Origins are positions here: the host is `OriginId::HOST` and the
/// extension at index `i` is `OriginId(i + 1)`, which is what an event's
/// origin indexes back into.
#[derive(Default)]
pub struct Extensions {
    list: Vec<Entry>,
    /// Each component instance placed last frame, by its key: where its
    /// events go and the messages its host passed for them (ADR 0060).
    instances: rustc_hash::FxHashMap<Key, Instance>,
    /// Bumped by every `finish`: an instance not placed since is dropped.
    generation: u32,
    /// Instances an event reached since they were last placed: their
    /// extension may have changed what it would draw, so the next
    /// placement asks it rather than replaying.
    heard: rustc_hash::FxHashSet<Key>,
}

/// What [`Extensions::route`] needs of a placed instance after the frame.
struct Instance {
    /// The origin that placed it, and the slot its nodes are in.
    placer: OriginId,
    slot: Option<Key>,
    /// The event props the host passed, as (field, message), in prop
    /// order: `onSelect` and Lua's `on_select` are both `select`, so an
    /// extension emits one name whichever binding placed it.
    events: Vec<(String, Value)>,
    generation: u32,
}

impl Instance {
    /// Whether `props`' event props are exactly these, without building
    /// a list to compare: a frame that passes the same ones allocates
    /// nothing.
    fn same_events(&self, props: &Value) -> bool {
        let Value::Map(entries) = props else {
            return self.events.is_empty();
        };
        let mut mine = self.events.iter();
        for (k, v) in entries
            .iter()
            .filter(|(k, v)| is_event_prop(k) && *v != Value::Null)
        {
            match mine.next() {
                Some((field, mv)) if prop_is_field(k, field) && mv == v => {}
                _ => return false,
            }
        }
        mine.next().is_none()
    }

    fn events_of(props: &Value) -> Vec<(String, Value)> {
        match props {
            Value::Map(entries) => entries
                .iter()
                // A null message is none declared: a binding that spells an
                // unset prop as null leaves the event undeclared.
                .filter(|(k, v)| is_event_prop(k) && *v != Value::Null)
                .map(|(k, v)| (event_field(k), v.clone()))
                .collect(),
            _ => Vec::new(),
        }
    }
}

impl Extensions {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds `ext` under its own name as the namespace — `import fs` binds
    /// `fs`. `push_as` with an empty namespace, which means the same.
    pub fn push(&mut self, ext: Box<dyn Extension>) -> Result<(), String> {
        self.push_as(String::new(), ext)
    }

    /// Adds `ext` under `namespace` — `import fs as left`. An empty
    /// namespace means the extension's own name: the rule is here, once,
    /// so that a host taking the namespace from outside the program (a C
    /// argument, a Node option, a Lua call) passes it through rather than
    /// substituting `name()` first — three of them did, and a fourth did
    /// not. Fails when that name is empty too, when the namespace is
    /// already taken (two extensions cannot share one, since slot names
    /// would collide), or when a slot the extension lists contains the
    /// separator (a slot name is the extension's own word; the host adds
    /// the namespace).
    pub fn push_as(
        &mut self,
        namespace: impl Into<String>,
        ext: Box<dyn Extension>,
    ) -> Result<(), String> {
        self.insert(namespace.into(), ext).map(|_| ())
    }

    /// `push_as` answering with the origin the extension got, which is
    /// what [`Fill::add`] hands back to a guest that loaded one.
    fn insert(&mut self, namespace: String, ext: Box<dyn Extension>) -> Result<OriginId, String> {
        let namespace = if namespace.is_empty() {
            ext.name().to_owned()
        } else {
            namespace
        };
        if namespace.is_empty() {
            return Err(
                "extension needs a namespace: it names itself nothing and none was given"
                    .to_owned(),
            );
        }
        if let Some(taken) = self.list.iter().find(|e| e.namespace == namespace) {
            return Err(format!(
                "namespace `{namespace}` is already `{}`'s; give `{}` another with `push_as`",
                taken.name,
                ext.name()
            ));
        }
        if let Some(bad) = ext
            .slots()
            .iter()
            .chain(ext.components())
            .find(|s| s.is_empty() || s.contains(NAMESPACE_SEPARATOR))
        {
            return Err(format!(
                "extension `{}` lists slot {bad:?}: a slot name is one word without `{}` — the \
                 host adds the namespace",
                ext.name(),
                NAMESPACE_SEPARATOR
            ));
        }
        let origin = OriginId(self.list.len() as u16 + 1);
        let mut ext = ext;
        let spares = if ext.components().is_empty() {
            Vec::new()
        } else {
            ext.reentry().into_iter().collect()
        };
        self.list.push(Entry {
            namespace,
            name: ext.name().to_owned(),
            slots: ext.slots().to_vec(),
            components: ext.components().to_vec(),
            ext: Some(ext),
            spares,
            asked_by: OriginId::HOST,
        });
        Ok(origin)
    }

    pub fn len(&self) -> usize {
        self.list.len()
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    /// The extension an event's origin names, if any. `None` while that
    /// extension is drawing — nothing routes events mid-frame — and for
    /// an origin no extension has.
    pub fn by_origin(&mut self, origin: OriginId) -> Option<&mut (dyn Extension + 'static)> {
        let i = (origin.0 as usize).checked_sub(1)?;
        self.list.get_mut(i)?.ext.as_deref_mut()
    }

    /// The namespace the host gave the extension at `origin`.
    pub fn namespace_of(&self, origin: OriginId) -> Option<&str> {
        let i = (origin.0 as usize).checked_sub(1)?;
        self.list.get(i).map(|e| e.namespace.as_str())
    }

    /// Who declared the slot the extension at `origin` last filled, and
    /// so where its replies go: `OriginId::HOST` unless another extension
    /// declared it.
    pub fn asked_by(&self, origin: OriginId) -> OriginId {
        let Some(i) = (origin.0 as usize).checked_sub(1) else {
            return OriginId::HOST;
        };
        self.list.get(i).map_or(OriginId::HOST, |e| e.asked_by)
    }

    /// Every (namespace, extension), in origin order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &dyn Extension)> {
        self.list
            .iter()
            .filter_map(|e| Some((e.namespace.as_str(), &**e.ext.as_ref()?)))
    }

    /// Delivers `events` to the extensions they came from and hands
    /// `to_host` everything addressed to the host: the host's own events,
    /// and the replies of every extension whose slot the host declared.
    ///
    /// A reply from an extension a *guest* placed goes to that guest
    /// instead, as one more event — its `origin` still the replier's, so
    /// the receiver knows who spoke — and whatever the guest answers
    /// travels the same way, up to [`MAX_REPLY_HOPS`] levels. This is the
    /// loop all four hosts route with; one that grew its own would
    /// disagree with the others about who hears a nested plugin.
    pub fn route(
        &mut self,
        events: impl IntoIterator<Item = UiEvent>,
        mut to_host: impl FnMut(UiEvent),
    ) {
        // Nobody to deliver to: every event is the host's, and the walk
        // below would only queue them up to say so. This is every host
        // that loaded nothing, so it is the common case.
        if self.list.is_empty() {
            events.into_iter().for_each(to_host);
            return;
        }
        let mut queue: std::collections::VecDeque<(OriginId, usize, UiEvent)> =
            events.into_iter().map(|ev| (ev.origin, 0, ev)).collect();
        while let Some((to, depth, ev)) = queue.pop_front() {
            if to == OriginId::HOST {
                to_host(ev);
                continue;
            }
            let Some(ext) = self.by_origin(to) else {
                // An origin nothing answers to: the host asked for the
                // frame that made it, so it still hears about it.
                to_host(ev);
                continue;
            };
            let replies = ext.on_event(&ev);
            if let Some(inst) = ev.slot
                && self.instances.contains_key(&inst)
            {
                self.heard.insert(inst);
            }
            if replies.is_empty() {
                continue;
            }
            let up = self.asked_by(to);
            for payload in replies {
                // A component's event (ADR 0060): the host's own message
                // for the prop, on the instance the event came from, to
                // whoever placed it.
                if let Some((prop, fields)) = emit_of(&payload)
                    && let Some(inst) = ev.slot
                    && let Some(rec) = self.instances.get(&inst)
                {
                    let Some((field, message)) = rec
                        .events
                        .iter()
                        .find(|(field, _)| prop_is_field(prop, field))
                    else {
                        continue;
                    };
                    let mut out =
                        UiEvent::component(rec.placer, inst, message, field, fields.clone());
                    out.window = ev.window;
                    out.slot = rec.slot;
                    if rec.placer == OriginId::HOST || depth + 1 >= MAX_REPLY_HOPS {
                        to_host(out);
                    } else {
                        queue.push_back((rec.placer, depth + 1, out));
                    }
                    continue;
                }
                let mut reply = UiEvent::declared(
                    crate::input::EventKind::Reply,
                    to,
                    ev.key,
                    Some(&payload),
                    Value::Null,
                );
                reply.window = ev.window;
                // About the same node, so from the same slot.
                reply.slot = ev.slot;
                if up == OriginId::HOST || depth + 1 >= MAX_REPLY_HOPS {
                    to_host(reply);
                } else {
                    queue.push_back((up, depth + 1, reply));
                }
            }
        }
    }

    /// Fills `name` of the extension at `i` as its origin, at the cursor.
    ///
    /// The extension comes *out* of the list for the duration, so the
    /// list itself stays free to fill the slots this extension declares
    /// while it draws — and so the one slot it cannot fill is its own,
    /// which would be the cycle.
    fn fill_one(
        &mut self,
        i: usize,
        name: &str,
        key: Key,
        params: &Value,
        children: Option<&mut dyn Children>,
        ui: &mut Ui<'_>,
    ) {
        let ns = self.list[i].namespace.clone();
        let ext_name = self.list[i].name.clone();
        let instance = children.is_some();
        // Busy drawing: a component of its own placed inside one of its
        // instances' host children is drawn by a spare handle.
        let (mut ext, spare) = match self.list[i].ext.take() {
            Some(ext) => (ext, false),
            None => match self.list[i].spares.pop().filter(|_| instance) {
                Some(mut s) => {
                    if self.list[i].spares.is_empty()
                        && let Some(deeper) = s.reentry()
                    {
                        self.list[i].spares.push(deeper);
                    }
                    (s, true)
                }
                None => {
                    let full = full_name(&ns, name);
                    let warning = if instance {
                        crate::diag::recursive_component(&ext_name, &full, key)
                    } else {
                        crate::diag::recursive_slot(&ext_name, &full, key)
                    };
                    ui.core().warn(warning);
                    return;
                }
            },
        };
        // Who put it here, and so where its replies go — the busy
        // handle's own placer once a spare is done.
        let asked_before = self.list[i].asked_by;
        self.list[i].asked_by = ui.origin();
        let slot = Slot {
            name,
            namespace: &ns,
            params,
            key,
        };
        let origin = OriginId(i as u16 + 1);
        let body = |ui: &mut Ui<'_>| {
            let drawn = if instance {
                ext.component(&slot, ui)
            } else {
                ext.view(&slot, ui)
            };
            if let Err(err) = drawn {
                ui.core().taint_kept("its view failed");
                ui.core().warn(crate::diag::extension_view_error(
                    &ext_name,
                    &slot.full_name(),
                    slot.key,
                    &err,
                ));
                ui.text(
                    &format!("[{ext_name}] {err}"),
                    crate::spec::TextStyle::new(13.0)
                        .color(crate::color::Color::rgb8(0xe8, 0x5d, 0x5d)),
                );
            }
        };
        match children {
            Some(children) => ui.fill_instance(origin, &slot, self, children, body),
            None => ui.fill_within(origin, &slot, self, body),
        }
        if spare {
            self.list[i].spares.push(ext);
            self.list[i].asked_by = asked_before;
        } else {
            self.list[i].ext = Some(ext);
        }
    }
}

impl TryFrom<Vec<Box<dyn Extension>>> for Extensions {
    type Error = String;

    /// Each under its own name; fails as `push` does — two extensions of
    /// one name need `push_as`.
    fn try_from(exts: Vec<Box<dyn Extension>>) -> Result<Self, String> {
        let mut out = Self::new();
        for ext in exts {
            out.push(ext)?;
        }
        Ok(out)
    }
}

/// The runner's loop, as a `Fill`. A declared name is split at its last
/// `/` into the namespace and the extension's own slot name; the
/// extension under that namespace fills it if it lists the name (or the
/// name is `"root"` and it lists none). A view that errors leaves its
/// message in the tree (red, where the fill would have been) and raises
/// `extension-view-error`, once per extension and slot.
impl Fill for Extensions {
    fn fill(&mut self, full_name: &str, key: Key, params: &Value, ui: &mut Ui<'_>) {
        let (ns, name) = split_name(full_name);
        let Some(i) = self.list.iter().position(|e| e.namespace == ns) else {
            return;
        };
        let slots = &self.list[i].slots;
        // A wildcard takes every declared name, `root` included: under it
        // `root` is one more name the host chose, not the auto-fill.
        let wants = slots.iter().any(|s| s == ANY_SLOT)
            || if name == ROOT_SLOT {
                slots.is_empty() && self.list[i].components.is_empty()
            } else {
                slots.iter().any(|s| s == name)
            };
        if wants {
            self.fill_one(i, name, key, params, None, ui);
        }
    }

    fn finish(&mut self, ui: &mut Ui<'_>) {
        // The instances this frame placed are the ones whose events can
        // still arrive; the rest are gone (ADR 0060).
        let generation = self.generation;
        self.instances.retain(|_, r| r.generation == generation);
        if !self.heard.is_empty() {
            let instances = &self.instances;
            self.heard.retain(|k| instances.contains_key(k));
        }
        self.generation = generation.wrapping_add(1);
        // By index rather than over a snapshot: a root fill here may load
        // an extension of its own, which joins the end of the list, and
        // the frame it arrived on is the frame it should draw on.
        let mut i = 0;
        while i < self.list.len() {
            let ns = self.list[i].namespace.clone();
            if self.list[i].slots.is_empty() && !self.list[i].components.is_empty() {
                // An extension of components only: placed where a host
                // puts them, never filled after the view (ADR 0060).
            } else if self.list[i].slots.is_empty() {
                // The fill every extension got before slots existed: after
                // the host's view, in list order.
                let full = full_name(&ns, ROOT_SLOT);
                if !ui.slot_declared(&full)
                    && let Some(key) = ui.core().begin_slot(&full)
                {
                    self.fill_one(i, ROOT_SLOT, key, &NULL_PARAMS, None, ui);
                }
            } else {
                // A wildcard lists nothing to check: whatever the host
                // declared under the namespace was filled above.
                for name in self.list[i].slots.clone() {
                    if name == ANY_SLOT {
                        continue;
                    }
                    let full = full_name(&ns, &name);
                    if ui.slot_declared(&full) {
                        continue;
                    }
                    let ext_name = self.list[i].name.clone();
                    ui.core()
                        .warn(crate::diag::unknown_slot(&ext_name, &ns, &name));
                }
            }
            i += 1;
        }
    }

    fn add(&mut self, namespace: &str, ext: Box<dyn Extension>) -> Result<OriginId, String> {
        self.insert(namespace.to_owned(), ext)
    }

    fn component(
        &mut self,
        full_name: &str,
        key: Key,
        props: &Value,
        children: &mut dyn Children,
        ui: &mut Ui<'_>,
    ) -> bool {
        let (ns, name) = split_name(full_name);
        let Some(i) = self.list.iter().position(|e| e.namespace == ns) else {
            return false;
        };
        let comps = &self.list[i].components;
        if !comps.iter().any(|c| c == name || c == ANY_SLOT) {
            return false;
        }
        self.fill_one(i, name, key, props, Some(children), ui);
        true
    }

    fn placed(
        &mut self,
        full_name: &str,
        key: Key,
        props: &Value,
        placer: OriginId,
        slot: Option<Key>,
    ) -> bool {
        let generation = self.generation;
        let heard = !self.heard.is_empty() && self.heard.remove(&key);
        match self.instances.get_mut(&key) {
            Some(rec) => {
                if !rec.same_events(props) {
                    rec.events = Instance::events_of(props);
                }
                rec.placer = placer;
                rec.slot = slot;
                rec.generation = generation;
            }
            None => {
                let (ns, _) = split_name(full_name);
                if self.list.iter().any(|e| e.namespace == ns) {
                    self.instances.insert(
                        key,
                        Instance {
                            placer,
                            slot,
                            events: Instance::events_of(props),
                            generation,
                        },
                    );
                }
            }
        }
        heard
    }
}

impl<F: Fill + ?Sized> Fill for &mut F {
    fn fill(&mut self, full_name: &str, key: Key, params: &Value, ui: &mut Ui<'_>) {
        F::fill(&mut **self, full_name, key, params, ui);
    }
    fn finish(&mut self, ui: &mut Ui<'_>) {
        F::finish(&mut **self, ui);
    }
    fn add(&mut self, namespace: &str, ext: Box<dyn Extension>) -> Result<OriginId, String> {
        F::add(&mut **self, namespace, ext)
    }
    fn component(
        &mut self,
        full_name: &str,
        key: Key,
        props: &Value,
        children: &mut dyn Children,
        ui: &mut Ui<'_>,
    ) -> bool {
        F::component(&mut **self, full_name, key, props, children, ui)
    }
    fn placed(
        &mut self,
        full_name: &str,
        key: Key,
        props: &Value,
        placer: OriginId,
        slot: Option<Key>,
    ) -> bool {
        F::placed(&mut **self, full_name, key, props, placer, slot)
    }
}

impl<F: Fill + ?Sized> Fill for Box<F> {
    fn fill(&mut self, full_name: &str, key: Key, params: &Value, ui: &mut Ui<'_>) {
        F::fill(&mut **self, full_name, key, params, ui);
    }
    fn finish(&mut self, ui: &mut Ui<'_>) {
        F::finish(&mut **self, ui);
    }
    fn add(&mut self, namespace: &str, ext: Box<dyn Extension>) -> Result<OriginId, String> {
        F::add(&mut **self, namespace, ext)
    }
    fn component(
        &mut self,
        full_name: &str,
        key: Key,
        props: &Value,
        children: &mut dyn Children,
        ui: &mut Ui<'_>,
    ) -> bool {
        F::component(&mut **self, full_name, key, props, children, ui)
    }
    fn placed(
        &mut self,
        full_name: &str,
        key: Key,
        props: &Value,
        placer: OriginId,
        slot: Option<Key>,
    ) -> bool {
        F::placed(&mut **self, full_name, key, props, placer, slot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_props_and_their_fields_in_both_spellings() {
        assert!(is_event_prop("onSelect"));
        assert!(is_event_prop("on_select"));
        assert!(!is_event_prop("one"));
        assert!(!is_event_prop("on"));
        assert_eq!(event_field("onSelect"), "select");
        assert_eq!(event_field("on_select"), "select");
        assert_eq!(event_field("onRowClick"), "rowClick");
        assert!(prop_is_field("onSelect", "select"));
        assert!(prop_is_field("on_select", "select"));
        assert!(!prop_is_field("onSelected", "select"));
        assert!(!prop_is_field("onSelect", "selected"));
    }

    #[test]
    fn an_emit_reply_reads_back() {
        let r = emit("onSave", Value::Int(3));
        assert_eq!(emit_of(&r), Some(("onSave", &Value::Int(3))));
        assert_eq!(emit_of(&Value::map([("save", Value::Bool(true))])), None);
    }
}
