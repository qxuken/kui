//! Slots: the place a host declares in its own view for an extension to
//! fill, with parameters in and replies out
//! (`docs/adr/0014-slots-an-extension-fills-in-place.md`).
//!
//! A slot is a *position*, not a node. `Ui::slot` (or `slot_with`) is a
//! call the host makes anywhere among its children, and whatever fills it
//! draws then and there, as children of the node the host is inside — so
//! the tree stays preorder by construction and nothing is appended to a
//! node that has closed. The filling is done by whatever implements
//! [`Fill`], which the frame was begun with (`Core::frame_with`); the
//! runner hands in its [`Extensions`], and that type's `Fill` is the loop.
//!
//! **Slot names are namespaced, and the host decides the namespace.** An
//! extension names the slots it fills in its own vocabulary — `"panel"`,
//! `"status"` — with no `/` in them. The host gives each extension it
//! loads a namespace, the way an importer picks an alias (`Extensions::
//! push_as`; `push` uses the extension's own name), and declares slots by
//! their full name: `ui.slot("fs/panel")` is the `"panel"` of the extension
//! the host calls `fs`. Exactly one extension can fill a slot, the same
//! plugin loaded twice is two namespaces with two sets of slots and two
//! sets of params, and nothing an extension names can collide with
//! anything another names.
//!
//! The reserved slot name `"root"` is what an extension listing no slots
//! fills — `ns/root`, once after the host's view, which is where every
//! extension drew before slots existed — and a host that declares
//! `ui.slot("ns/root")` itself moves that fill to the position it chose.
//! The core owns the two things the ADR makes it own: the key namespace
//! of a fill (decision 4; see `Core::fill`) and the bound on it (decision
//! 5).
//!
//! ## An extension may host extensions of its own
//!
//! The ADR left one question open — "whether one may offer slots is a
//! decision for the day one asks" — and the day was a Lua view wanting a
//! native panel inside it. The answer is yes, and it is the same
//! mechanism one level down rather than a second one:
//!
//! * [`Fill::add`] loads an extension while a frame is being built, which
//!   is when a guest knows it wants one. It lands in the *same* list under
//!   a namespace of its own, so the frame has one namespace map and one
//!   origin per extension however deep the loading went. `todos/panel`
//!   means one thing to everybody.
//! * A guest's `ui.slot(…)` declares a slot like anyone's, filled from
//!   that one list. What it may *not* do is fill itself: the extension
//!   doing the filling is out of the list while it fills, so a cycle is
//!   the `recursive-slot` warning and an empty position rather than a
//!   hang.
//! * Replies go to whoever declared the slot ([`Extensions::route`]) —
//!   decision 6 read as it is written, since the thing an extension is
//!   answering is the slot it was put in. For every extension a host
//!   declared itself, that is the host, exactly as before.

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

/// Splits a full slot name at its last separator into (namespace, name);
/// a name with none has the empty namespace.
/// The one entry in `Extension::slots` that means "every name the host
/// declares under my namespace": for an extension that learns its slots
/// after it loads. `fill` matches any declared name against it and
/// `finish` has nothing to warn about for it (backlog K1).
pub const ANY_SLOT: &str = "*";

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
    /// `Extension::name` and `Extension::slots` as they answered at load.
    /// Copied because both are read while `ext` is taken — and because
    /// every binding already reads `slots` once, at load, so there was
    /// never a second answer to miss.
    name: String,
    slots: Vec<String>,
    /// The origin that declared the slot this extension last filled:
    /// `OriginId::HOST` for one in the host's own view, another
    /// extension's when that extension declared it. Where its replies go
    /// (ADR 0014 decision 6; see `route`).
    asked_by: OriginId,
}

/// The runner's extensions, each under the namespace the host gave it.
/// Origins are positions here: the host is `OriginId::HOST` and the
/// extension at index `i` is `OriginId(i + 1)`, which is what an event's
/// origin indexes back into.
#[derive(Default)]
pub struct Extensions {
    list: Vec<Entry>,
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
        self.list.push(Entry {
            namespace,
            name: ext.name().to_owned(),
            slots: ext.slots().to_vec(),
            ext: Some(ext),
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
            if replies.is_empty() {
                continue;
            }
            let up = self.asked_by(to);
            for payload in replies {
                let reply = UiEvent {
                    origin: to,
                    window: ev.window,
                    key: ev.key,
                    payload,
                    // About the same node, so from the same slot.
                    slot: ev.slot,
                };
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
    fn fill_one(&mut self, i: usize, name: &str, key: Key, params: &Value, ui: &mut Ui<'_>) {
        let ns = self.list[i].namespace.clone();
        let ext_name = self.list[i].name.clone();
        let Some(mut ext) = self.list[i].ext.take() else {
            let full = full_name(&ns, name);
            ui.core()
                .warn(crate::diag::recursive_slot(&ext_name, &full, key));
            return;
        };
        // Who put it here, and so where its replies go.
        self.list[i].asked_by = ui.origin();
        let slot = Slot {
            name,
            namespace: &ns,
            params,
            key,
        };
        let origin = OriginId(i as u16 + 1);
        ui.fill_within(origin, &slot, self, |ui| {
            if let Err(err) = ext.view(&slot, ui) {
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
        });
        self.list[i].ext = Some(ext);
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
                slots.is_empty()
            } else {
                slots.iter().any(|s| s == name)
            };
        if wants {
            self.fill_one(i, name, key, params, ui);
        }
    }

    fn finish(&mut self, ui: &mut Ui<'_>) {
        // By index rather than over a snapshot: a root fill here may load
        // an extension of its own, which joins the end of the list, and
        // the frame it arrived on is the frame it should draw on.
        let mut i = 0;
        while i < self.list.len() {
            let ns = self.list[i].namespace.clone();
            if self.list[i].slots.is_empty() {
                // The fill every extension got before slots existed: after
                // the host's view, in list order.
                let full = full_name(&ns, ROOT_SLOT);
                if !ui.slot_declared(&full)
                    && let Some(key) = ui.core().begin_slot(&full)
                {
                    self.fill_one(i, ROOT_SLOT, key, &NULL_PARAMS, ui);
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
}
