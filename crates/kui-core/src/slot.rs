//! Components (ADR 0060, ADR 0067): what an extension provides for a host
//! to place, with props in and the host's own messages out.
//!
//! A placement is a position, not a node. `Ui::component` (or
//! `component_with`, with the host's children) is a call the host makes
//! anywhere among its children, and the instance draws then and there, as
//! children of the node the host is inside. The drawing is done by
//! whatever implements [`Fill`], which the frame was begun with
//! (`Core::frame_with`); the windowed runner hands in its [`Extensions`].
//! An app that loads no extensions never meets this module.
//!
//! Names are namespaced, and the host decides the namespace. An extension
//! names what it provides in its own vocabulary (`"panel"`, `"button"`, no
//! `/` in them). The host gives each extension a namespace when it loads
//! it ([`Extensions::push_as`]; [`Extensions::push`] uses the extension's
//! own name) and places by full name: `ui.component("fs/panel", &props)` is
//! the `"panel"` of the extension the host calls `fs`. The same plugin
//! loaded twice is two namespaces.
//!
//! ```rust
//! use kui_core::slot::{full_name, split_name, Extensions, ROOT_COMPONENT};
//!
//! assert_eq!(full_name("fs", "panel"), "fs/panel");
//! assert_eq!(split_name("left/fs/panel"), ("left/fs", "panel"));
//! assert_eq!(split_name("root"), ("", ROOT_COMPONENT));
//!
//! // The runner's list; `push_as(namespace, Box<dyn Extension>)` loads one.
//! let exts = Extensions::new();
//! assert!(exts.is_empty());
//! ```
//!
//! Every extension provides `"root"` ([`ROOT_COMPONENT`]), its `view`: one
//! that provides nothing else is placed as `ns/root` once after the host's
//! view, and a host that places `ns/root` itself moves it there. A `root`
//! is drawn fresh every frame — a view's state can change without an event
//! the core sees — where every other instance is replayed while what it
//! read holds.
//!
//! An extension may host extensions of its own, by the same mechanism one
//! level down: [`Fill::add`] loads one while a frame is being built, under
//! a namespace of its own in the same list; a guest places components like
//! anyone, except that it cannot place its own while it draws without a
//! spare handle ([`Extension::reentry`]; otherwise `recursive-component`).
//!
//! Each instance is keyed by where it was placed. It can hold the host's
//! own children — the fill marks where with `Ui::children(group)`, and the
//! host's [`Children`] builds them under the host's origin — and its events
//! reach whoever placed it as the messages passed in its props: the
//! extension answers an event with [`emit`], and [`Extensions::route`]
//! delivers the placer's message for that prop.

use crate::input::UiEvent;
use crate::key::Key;
use crate::runtime::Extension;
use crate::tree::OriginId;
use crate::ui::Ui;
use crate::value::Value;

/// Which component instance an extension is drawing, handed to
/// `Extension::view` and `Extension::component`.
#[derive(Clone, Copy, Debug)]
pub struct Instance<'a> {
    /// The component in the extension's own vocabulary — one it lists in
    /// `components`, or `"root"`.
    pub name: &'a str,
    /// The namespace the host gave this extension: what makes `name` a
    /// full name (`namespace/name`), and what tells one load of a plugin
    /// loaded twice from the other.
    pub namespace: &'a str,
    /// The props whoever placed it passed. Declared every frame and never
    /// retained — read it, do not keep it.
    pub params: &'a Value,
    /// The instance's own key: where it was placed (or its `key` prop),
    /// and the namespace the extension's nodes are keyed under.
    pub key: Key,
}

/// The component every extension provides: its `view`.
pub const ROOT_COMPONENT: &str = "root";

/// What separates a namespace from a component name in a full name. A
/// namespace may contain it (the host chooses namespaces, and `"left/fs"`
/// is a fine one); a name an extension lists may not, so a full name
/// splits at its last one.
pub const NAMESPACE_SEPARATOR: char = '/';

/// `Value::Null` with a `'static` address, for an instance with no props.
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

/// How many times an emit may be answered by another before the rest go to
/// the host instead ([`Extensions::route`]): a guest that placed a guest
/// hears its events and may emit in turn. Nesting is a few levels deep in
/// anything sane; this is the bound that keeps two extensions answering
/// each other from being an infinite loop.
pub const MAX_EMIT_HOPS: usize = 16;

impl Instance<'_> {
    /// The `"root"` instance as a test drives an extension without a
    /// runner: no namespace, no props, keyed under the root. The runner's
    /// own is `Instance { name: "root", namespace: <the host's> }`, keyed
    /// under `"ns/root"`.
    pub fn root() -> Instance<'static> {
        Instance {
            name: ROOT_COMPONENT,
            namespace: "",
            params: &NULL_PARAMS,
            key: Key::ROOT.str(ROOT_COMPONENT),
        }
    }

    /// The full name a host places it by: `namespace/name`, or just `name`
    /// with no namespace (`Instance::root`).
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

/// The one entry in `Extension::components` that means "every name placed
/// under my namespace": for an extension that learns what it provides
/// after it loads (views registered at runtime).
pub const ANY_COMPONENT: &str = "*";

/// Splits a full name at its last separator into (namespace, name);
/// a name with none has the empty namespace.
pub fn split_name(full: &str) -> (&str, &str) {
    match full.rfind(NAMESPACE_SEPARATOR) {
        Some(i) => (&full[..i], &full[i + 1..]),
        None => ("", full),
    }
}

/// What draws components: the runner's extension list, or a test's
/// stand-in. A frame begun with `Core::frame_with` carries one;
/// `Ui::component` calls [`Fill::component`] at the position the host
/// placed it, and `Ui::finish` calls [`Fill::finish`] once the host's view
/// is done.
pub trait Fill {
    /// After the host's view: place every `ns/root` the host did not, and
    /// forget the instances this frame did not place.
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
             extension list to draw its components"
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
    /// cannot know about ([`FillAnswer::Heard`](crate::FillAnswer::Heard)).
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
    /// `None` for exactly as long as this extension is drawing: `fill_one`
    /// takes it out so the list is free to draw what the extension itself
    /// places, and puts it back after. One of its own components placed
    /// while it draws therefore takes a spare, or finds nobody, which is
    /// the cycle warning rather than a borrow panic.
    ext: Option<Box<dyn Extension>>,
    /// Second handles onto the extension (`Extension::reentry`), for a
    /// component of its own placed while it draws — inside a host's
    /// children of one of its instances. One deeper than the deepest
    /// nesting so far; each comes back after its fill.
    spares: Vec<Box<dyn Extension>>,
    /// `Extension::name` and `Extension::components` as they answered at
    /// load: read while `ext` is taken, and every binding reads its list
    /// once, at load.
    name: String,
    components: Vec<String>,
    /// The generation its `root` was last placed in by someone: a root
    /// placed this frame is not placed again after the host's view.
    root_placed: u32,
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
    instances: rustc_hash::FxHashMap<Key, PlacedInstance>,
    /// Bumped by every `finish`: an instance not placed since is dropped.
    generation: u32,
    /// Instances an event reached since they were last placed: their
    /// extension may have changed what it would draw, so the next
    /// placement asks it rather than replaying.
    heard: rustc_hash::FxHashSet<Key>,
}

/// What [`Extensions::route`] needs of a placed instance after the frame.
struct PlacedInstance {
    /// The origin that placed it, and the instance the placer's own nodes
    /// are in (`None` for the host's view).
    placer: OriginId,
    slot: Option<Key>,
    /// The event props the host passed, as (field, message), in prop
    /// order: `onSelect` and Lua's `on_select` are both `select`, so an
    /// extension emits one name whichever binding placed it.
    events: Vec<(String, Value)>,
    generation: u32,
}

impl PlacedInstance {
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
    /// already taken (two extensions cannot share one, since full names
    /// would collide), or when a component the extension lists contains
    /// the separator (a name is the extension's own word; the host adds
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
            .components()
            .iter()
            .find(|s| s.is_empty() || s.contains(NAMESPACE_SEPARATOR))
        {
            return Err(format!(
                "extension `{}` lists component {bad:?}: a name is one word without `{}` — the \
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
            components: ext.components().to_vec(),
            ext: Some(ext),
            spares,
            root_placed: self.generation.wrapping_sub(1),
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

    /// Every (namespace, extension), in origin order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &dyn Extension)> {
        self.list
            .iter()
            .filter_map(|e| Some((e.namespace.as_str(), &**e.ext.as_ref()?)))
    }

    /// Delivers `events` to the extensions they came from and hands
    /// `to_host` everything addressed to the host: the host's own events,
    /// and the component events of every instance the host placed.
    ///
    /// An instance a *guest* placed raises its events to that guest
    /// instead, as one more event, and whatever the guest emits in turn
    /// travels the same way, up to [`MAX_EMIT_HOPS`] levels. An answer that
    /// is not an [`emit`] is dropped (ADR 0067). This is the loop every
    /// host routes with; one that grew its own would disagree with the
    /// others about who hears a nested plugin.
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
            if let Some(inst) = ev.instance
                && self.instances.contains_key(&inst)
            {
                self.heard.insert(inst);
            }
            for payload in replies {
                // A component's event (ADR 0060): the host's own message
                // for the prop, on the instance the event came from, to
                // whoever placed it.
                if let Some((prop, fields)) = emit_of(&payload)
                    && let Some(inst) = ev.instance
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
                    out.instance = rec.slot;
                    if rec.placer == OriginId::HOST || depth + 1 >= MAX_EMIT_HOPS {
                        to_host(out);
                    } else {
                        queue.push_back((rec.placer, depth + 1, out));
                    }
                }
            }
        }
    }

    /// Draws `name` of the extension at `i` as its origin, at the cursor.
    ///
    /// The extension comes *out* of the list for the duration, so the
    /// list itself stays free to draw what this extension places while it
    /// draws — and so one of its own components placed meanwhile takes a
    /// spare handle, or is the cycle.
    fn fill_one(
        &mut self,
        i: usize,
        name: &str,
        key: Key,
        params: &Value,
        children: &mut dyn Children,
        ui: &mut Ui<'_>,
    ) {
        let ns = self.list[i].namespace.clone();
        let ext_name = self.list[i].name.clone();
        // Busy drawing: a component of its own placed inside one of its
        // instances' host children is drawn by a spare handle.
        let (mut ext, spare) = match self.list[i].ext.take() {
            Some(ext) => (ext, false),
            None => match self.list[i].spares.pop() {
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
                    ui.core()
                        .warn(crate::diag::recursive_component(&ext_name, &full, key));
                    return;
                }
            },
        };
        let slot = Instance {
            name,
            namespace: &ns,
            params,
            key,
        };
        let origin = OriginId(i as u16 + 1);
        let body = |ui: &mut Ui<'_>| {
            let drawn = if name == ROOT_COMPONENT {
                ext.view(&slot, ui)
            } else {
                ext.component(&slot, ui)
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
        ui.fill_instance(origin, &slot, self, children, body);
        if spare {
            self.list[i].spares.push(ext);
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

/// The runner's loop, as a `Fill`. A placed name is split at its last
/// `/` into the namespace and the extension's own component name; the
/// extension under that namespace draws it if it provides the name (`root`
/// always). A view that errors leaves its message in the tree (red, where
/// the instance would have been) and raises `extension-view-error`.
impl Fill for Extensions {
    fn finish(&mut self, ui: &mut Ui<'_>) {
        // The instances this frame placed are the ones whose events can
        // still arrive; the rest are gone (ADR 0060).
        let generation = self.generation;
        self.instances.retain(|_, r| r.generation == generation);
        if !self.heard.is_empty() {
            let instances = &self.instances;
            self.heard.retain(|k| instances.contains_key(k));
        }
        // By index rather than over a snapshot: a root drawn here may load
        // an extension of its own, which joins the end of the list, and
        // the frame it arrived on is the frame it should draw on.
        let mut i = 0;
        while i < self.list.len() {
            // An extension providing nothing but its `root`, which the host
            // did not place: drawn after the host's view, in list order —
            // the fill every extension got before components existed.
            if self.list[i].components.is_empty() && self.list[i].root_placed != generation {
                let full = full_name(&self.list[i].namespace, ROOT_COMPONENT);
                if let Some(key) = ui.core().root_fill_key(&full) {
                    self.placed(&full, key, &NULL_PARAMS, OriginId::HOST, None);
                    self.fill_one(i, ROOT_COMPONENT, key, &NULL_PARAMS, &mut |_: &str, _: &mut Ui<'_>| {}, ui);
                }
            }
            i += 1;
        }
        // After the roots, which record their instances under this one.
        self.instances.retain(|_, r| r.generation == generation);
        self.generation = generation.wrapping_add(1);
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
        let root = name == ROOT_COMPONENT;
        if !root && !comps.iter().any(|c| c == name || c == ANY_COMPONENT) {
            return false;
        }
        if root {
            self.list[i].root_placed = self.generation;
        }
        self.fill_one(i, name, key, props, children, ui);
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
                    rec.events = PlacedInstance::events_of(props);
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
                        PlacedInstance {
                            placer,
                            slot,
                            events: PlacedInstance::events_of(props),
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
