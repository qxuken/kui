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
}

/// The runner's extensions, each under the namespace the host gave it.
/// Origins are positions here: the host is `OriginId::HOST` and the
/// extension at index `i` is `OriginId(i + 1)`, which is what an event's
/// origin indexes back into.
#[derive(Default)]
pub struct Extensions {
    list: Vec<(String, Box<dyn Extension>)>,
}

impl Extensions {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds `ext` under its own name as the namespace — `import fs` binds
    /// `fs`. Fails as `push_as` does.
    pub fn push(&mut self, ext: Box<dyn Extension>) -> Result<(), String> {
        let ns = ext.name().to_owned();
        self.push_as(ns, ext)
    }

    /// Adds `ext` under `namespace` — `import fs as left`. Fails when the
    /// namespace is empty or already taken (two extensions cannot share
    /// one, since slot names would collide), or when a slot the extension
    /// lists contains the separator (a slot name is the extension's own
    /// word; the host adds the namespace).
    pub fn push_as(
        &mut self,
        namespace: impl Into<String>,
        ext: Box<dyn Extension>,
    ) -> Result<(), String> {
        let namespace = namespace.into();
        if namespace.is_empty() {
            return Err(format!(
                "extension `{}` needs a namespace; it cannot be empty",
                ext.name()
            ));
        }
        if let Some((_, taken)) = self.list.iter().find(|(ns, _)| *ns == namespace) {
            return Err(format!(
                "namespace `{namespace}` is already `{}`'s; give `{}` another with `push_as`",
                taken.name(),
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
        self.list.push((namespace, ext));
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.list.len()
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    /// The extension an event's origin names, if any.
    pub fn by_origin(&mut self, origin: OriginId) -> Option<&mut (dyn Extension + 'static)> {
        let i = (origin.0 as usize).checked_sub(1)?;
        self.list.get_mut(i).map(|(_, e)| e.as_mut())
    }

    /// The namespace the host gave the extension at `origin`.
    pub fn namespace_of(&self, origin: OriginId) -> Option<&str> {
        let i = (origin.0 as usize).checked_sub(1)?;
        self.list.get(i).map(|(ns, _)| ns.as_str())
    }

    /// Every (namespace, extension), in origin order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &dyn Extension)> {
        self.list.iter().map(|(ns, e)| (ns.as_str(), &**e))
    }

    /// Every (namespace, full slot name) an extension expects the host to
    /// declare — `ns/root` for one listing no slots.
    fn expected(&self) -> Vec<(usize, String, String)> {
        let mut out = Vec::new();
        for (i, (ns, ext)) in self.list.iter().enumerate() {
            if ext.slots().is_empty() {
                out.push((i, ns.clone(), ROOT_SLOT.to_owned()));
            } else {
                for s in ext.slots() {
                    out.push((i, ns.clone(), s.clone()));
                }
            }
        }
        out
    }

    /// Fills `name` of the extension at `i` as its origin, at the cursor.
    fn fill_one(&mut self, i: usize, name: &str, key: Key, params: &Value, ui: &mut Ui<'_>) {
        let (ns, ext) = &mut self.list[i];
        let slot = Slot {
            name,
            namespace: ns,
            params,
            key,
        };
        let origin = OriginId(i as u16 + 1);
        let ext_name = ext.name().to_owned();
        ui.fill(origin, &slot, |ui| {
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
        let Some(i) = self.list.iter().position(|(n, _)| n == ns) else {
            return;
        };
        let slots = self.list[i].1.slots();
        let wants = if name == ROOT_SLOT {
            slots.is_empty()
        } else {
            slots.iter().any(|s| s == name)
        };
        if wants {
            self.fill_one(i, name, key, params, ui);
        }
    }

    fn finish(&mut self, ui: &mut Ui<'_>) {
        for (i, ns, name) in self.expected() {
            let full = full_name(&ns, &name);
            if ui.slot_declared(&full) {
                continue;
            }
            if name == ROOT_SLOT {
                // The fill every extension got before slots existed: after
                // the host's view, in list order.
                if let Some(key) = ui.core().begin_slot(&full) {
                    self.fill_one(i, &name, key, &NULL_PARAMS, ui);
                }
            } else {
                let ext_name = self.list[i].1.name().to_owned();
                ui.core()
                    .warn(crate::diag::unknown_slot(&ext_name, &ns, &name));
            }
        }
    }
}

impl<F: Fill + ?Sized> Fill for &mut F {
    fn fill(&mut self, full_name: &str, key: Key, params: &Value, ui: &mut Ui<'_>) {
        F::fill(&mut **self, full_name, key, params, ui);
    }
    fn finish(&mut self, ui: &mut Ui<'_>) {
        F::finish(&mut **self, ui);
    }
}

impl<F: Fill + ?Sized> Fill for Box<F> {
    fn fill(&mut self, full_name: &str, key: Key, params: &Value, ui: &mut Ui<'_>) {
        F::fill(&mut **self, full_name, key, params, ui);
    }
    fn finish(&mut self, ui: &mut Ui<'_>) {
        F::finish(&mut **self, ui);
    }
}
