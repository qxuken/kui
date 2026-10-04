//! [`Session`]: what a set of windows shares.
//!
//! kui is one [`Core`](crate::Core) per window, and everything per frame
//! (the tree, the display list, focus, hit regions) stays with its
//! window. What must not be duplicated lives in a `Session`: the font
//! database every window shapes against, the resource registry behind
//! every `FontId` / `ImageId` / `SoundId`, the audio store (the process
//! has one device), and the declared window set. Any number of cores are
//! constructed against one session with
//! [`Core::new_in`](crate::Core::new_in), so a font, image or sound
//! registered in one window draws and plays in every window.
//! [`Core::new`](crate::Core::new) makes a private session, which is why a
//! single-window app never has to know one exists.
//!
//! ```rust
//! use kui_core::{Core, Session};
//!
//! let session = Session::new();
//! let main = Core::new_in(&session);
//! let popup = Core::new_in(&session);
//!
//! // Registered through one window, visible to both.
//! let image = main.resources.add_image(2, 2, vec![255; 16]);
//! assert_eq!(popup.resources.image_size(image), Some((2, 2)));
//! assert!(main.session().is(&session));
//! ```
//!
//! The shaped-text cache and the glyph atlas stay per window even though
//! they look like caches: a cache entry is only valid for the atlas page
//! it was packed against, and [`Core::output`](crate::Core::output) lends
//! that page to the window's renderer.
//!
//! A handle belongs to its session. Handles are unique to the process, so
//! a `FontId` / `ImageId` / `SoundId` handed to a core of another session
//! is a miss, never an alias: it behaves as a removed handle does, and the
//! next [`Core::take_warnings`](crate::Core::take_warnings) reports it as
//! `foreign-resource`. Two `Core::new()`s are two sessions.
//!
//! The session is `Rc<RefCell<_>>`: a `Core` borrows it inside a method
//! and drops the borrow before returning, and the windows of a
//! multi-window app are driven one frame at a time on one thread, so the
//! check never contends.

use std::cell::{RefCell, RefMut};
use std::rc::Rc;
use std::sync::Arc;

use cosmic_text::FontSystem;

use crate::audio::AudioStore;
use crate::resources::{FontId, FragmentId, ImageId, Resources, SessionId, SoundId};
use crate::tree::OriginId;
use crate::window::{WindowConfig, WindowId};

/// The session's contents. Reached through [`Session::state`], one borrow
/// at a time.
pub(crate) struct SessionState {
    /// Which session this is, for the handles the registry mints.
    pub(crate) id: SessionId,
    /// The font database every window in the session shapes against — a
    /// face loaded by one window resolves for all of them.
    pub(crate) fonts: FontSystem,
    /// The scan of the system's fonts `fonts` holds the faces of, so a
    /// newer scan can be applied as what came and went since
    /// (`Core::reload_system_fonts`).
    pub(crate) system: std::sync::Arc<crate::text::SystemFonts>,
    /// Bumped by every rescan of the system's fonts that changed this
    /// session's (`apply_system_fonts`) — not by a font the app loads —
    /// so each window can report it as one `fonts` event.
    pub(crate) system_fonts_rev: u64,
    pub(crate) resources: Resources,
    pub(crate) audio: AudioStore,
    /// Bumped by every font registration and removal, so a `Core` can tell
    /// whether its name mirror is behind without walking the slotmap.
    pub(crate) fonts_rev: u64,
    /// Bumped when the weights a registered family is asked at change
    /// under it — a face of it loaded or removed — so each `Core`
    /// drops the text it shaped at the old ones.
    pub(crate) weights_rev: u64,
    /// Whether the database's file-backed faces have been mapped once and
    /// shared (`share_faces`): done on the first family an
    /// app names, so an app that only ever shapes the stock families never
    /// pays for it.
    pub(crate) faces_shared: bool,
    /// Bumped by every image removal, so a `Core` can tell whether its
    /// atlas still holds a slot for an image the registry no longer has.
    /// The atlas is per window, so every core re-checks its own.
    pub(crate) images_rev: u64,
    /// Handles removed and not yet handed to a display list: what
    /// a backend frees on the device. Textures and fragment pipelines
    /// live on the device every window of the session shares, so the
    /// list is the session's and whichever core begins a frame next
    /// carries it — a removal through a window that closes before its
    /// next frame is not lost with the window.
    pub(crate) dropped: Dropped,
    /// The declared window set and the windows it has opened.
    pub(crate) windows: WindowRegistry,
    /// The devtools panel's state: one
    /// panel for the session, whichever window draws it.
    pub(crate) devtools: crate::runtime::devtools::State,
}

impl SessionState {
    fn new() -> Self {
        let id = SessionId::next();
        let (fonts, system) = crate::text::new_font_system();
        Self {
            id,
            fonts,
            system,
            system_fonts_rev: 0,
            resources: Resources::new(id),
            audio: AudioStore::default(),
            fonts_rev: 0,
            weights_rev: 0,
            faces_shared: false,
            images_rev: 0,
            dropped: Dropped::default(),
            windows: WindowRegistry::new(),
            devtools: crate::runtime::devtools::State::default(),
        }
    }
}

impl SessionState {
    /// Unregisters an image: the registry forgets it, every core's atlas
    /// hears (through `images_rev`) and, for a texture-backed one, the
    /// next display list any core builds tells the backend to drop the
    /// texture. `None` for a handle the registry did not hold, which is
    /// noted as a miss.
    pub(crate) fn remove_image(&mut self, id: ImageId) -> Option<crate::resources::ImageEntry> {
        let entry = self.resources.remove_image(id)?;
        self.images_rev += 1;
        if entry.backing == crate::resources::ImageBacking::Texture {
            self.dropped.images.push(id);
        }
        Some(entry)
    }

    /// Unregisters a fragment; the next display list any core builds
    /// tells the backend to drop its pipelines.
    pub(crate) fn remove_fragment(&mut self, id: FragmentId) -> bool {
        if self.resources.remove_fragment(id).is_none() {
            return false;
        }
        self.dropped.fragments.push(id);
        true
    }
}

/// Handles the backend has yet to hear were removed; see
/// [`SessionState::dropped`].
#[derive(Default)]
pub(crate) struct Dropped {
    /// Texture-backed images (an atlas-backed one has nothing on the
    /// device to free).
    pub(crate) images: Vec<ImageId>,
    pub(crate) fragments: Vec<FragmentId>,
}

/// One frame's declaration of a window, as `Core::declare_window` recorded
/// it: the name, the config the declaration carried, and who declared it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct WindowDecl {
    pub(crate) name: Rc<str>,
    pub(crate) config: WindowConfig,
    pub(crate) origin: OriginId,
    /// The same frame declared this name again with a different config.
    /// The first declaration is the one kept; this remembers that there
    /// was a disagreement, for the warning the opening edge raises.
    pub(crate) conflict: bool,
}

/// A window the registry has opened: its name, its id, and whether it is
/// still on screen. `live` goes false when the driver reports an OS close
/// while the name is still declared — the window stays closed until the
/// declaration lapses and starts again.
struct WindowEntry {
    name: Rc<str>,
    id: WindowId,
    live: bool,
}

/// What one diff of the declared set decided; the core turns each into a
/// `WindowCommand`, a `{kind:"window"}` event and, sometimes, a warning.
pub(crate) enum WindowChange {
    Opened {
        id: WindowId,
        name: Rc<str>,
        /// The window whose slot won the declaration — a popup's owner,
        /// and the surface its anchor is measured against.
        owner: WindowId,
        origin: OriginId,
        config: WindowConfig,
        /// Two declarations of this name disagreed about the config on the
        /// frame it opened: `duplicate-window-config`.
        conflict: bool,
    },
    Closed {
        id: WindowId,
        name: Rc<str>,
    },
}

/// The declared window set, across every core of the session.
///
/// `slots` holds each core's latest declarations, keyed by the core's
/// window id and kept in id order — so "the lowest declaring `WindowId`
/// wins" is the first slot that names a window, with no rule about which
/// frame ran first. `windows` is what the diff has opened and not yet
/// closed, seeded with the main window, which the launcher opens and
/// nothing here ever closes.
pub(crate) struct WindowRegistry {
    slots: Vec<(WindowId, Vec<WindowDecl>)>,
    windows: Vec<WindowEntry>,
    next_id: u32,
}

/// The main window's name: what `view(model, window)` is called with in
/// Node, and what `Core::window_name` answers for `WindowId::MAIN`.
pub const MAIN_WINDOW_NAME: &str = "main";

impl WindowRegistry {
    fn new() -> Self {
        Self {
            slots: Vec::new(),
            windows: vec![WindowEntry {
                name: Rc::from(MAIN_WINDOW_NAME),
                id: WindowId::MAIN,
                live: true,
            }],
            next_id: 1,
        }
    }

    /// Replaces what the core drawing `id` declared, from its latest frame.
    pub(crate) fn set_slot(&mut self, id: WindowId, decls: &[WindowDecl]) {
        match self.slots.binary_search_by_key(&id, |(i, _)| *i) {
            Ok(i) if decls.is_empty() => {
                self.slots.remove(i);
            }
            Ok(i) => {
                self.slots[i].1.clear();
                self.slots[i].1.extend_from_slice(decls);
            }
            Err(_) if decls.is_empty() => {}
            Err(i) => self.slots.insert(i, (id, decls.to_vec())),
        }
    }

    fn remove_slot(&mut self, id: WindowId) {
        if let Ok(i) = self.slots.binary_search_by_key(&id, |(i, _)| *i) {
            self.slots.remove(i);
        }
    }

    /// The name the window `id` was declared under, while it is open.
    pub(crate) fn name_of(&self, id: WindowId) -> Option<Rc<str>> {
        self.windows
            .iter()
            .find(|w| w.id == id)
            .map(|w| w.name.clone())
    }

    /// Every window on screen, main first, in the order they opened.
    pub(crate) fn live(&self) -> Vec<(WindowId, Rc<str>)> {
        self.windows
            .iter()
            .filter(|w| w.live)
            .map(|w| (w.id, w.name.clone()))
            .collect()
    }

    /// Whether any window was closed by the OS and is still declared —
    /// the state `window-declared-while-closed` reports.
    pub(crate) fn any_closed(&self) -> bool {
        self.windows.iter().any(|w| !w.live)
    }

    /// The names among `decls` whose window the OS closed and nothing has
    /// stopped declaring since.
    pub(crate) fn closed_among<'a>(
        &'a self,
        decls: &'a [WindowDecl],
    ) -> impl Iterator<Item = Rc<str>> + 'a {
        decls.iter().filter_map(|d| {
            self.windows
                .iter()
                .find(|w| !w.live && w.name == d.name)
                .map(|w| w.name.clone())
        })
    }

    /// The driver reports that the OS closed window `id`. Its declarations
    /// leave the union with it; its name stays known, closed, until the
    /// declaration lapses. Returns the name if the window was open; `None`
    /// for the main window (which closing ends the app) and for an id the
    /// diff already closed.
    pub(crate) fn os_closed(&mut self, id: WindowId) -> Option<Rc<str>> {
        if id == WindowId::MAIN {
            return None;
        }
        let w = self.windows.iter_mut().find(|w| w.id == id && w.live)?;
        w.live = false;
        let name = w.name.clone();
        self.remove_slot(id);
        Some(name)
    }

    /// Diffs the union of every slot against the open windows.
    ///
    /// A live window nobody declares any more closes, and its own
    /// declarations leave the union with it — so a window that declared a
    /// child closes the child in the same diff, however deep. A name in
    /// the union with no window opens, with the config from the lowest
    /// declaring slot (main is 0, so main wins whenever it declares) and
    /// the first declaration within that slot's frame; any other
    /// declaration that disagrees marks the open as a conflict. A closed
    /// name still in the union stays closed (decision 6's edge); one that
    /// left it is forgotten, so declaring it again opens it anew.
    pub(crate) fn diff(&mut self, out: &mut Vec<WindowChange>) {
        loop {
            let union = self.union();
            let gone: Vec<usize> = (0..self.windows.len())
                .rev()
                .filter(|&i| {
                    let w = &self.windows[i];
                    w.id != WindowId::MAIN && !union.iter().any(|u| u.0 == w.name)
                })
                .collect();
            if gone.is_empty() {
                for (name, config, owner, origin, conflict) in union {
                    if self.windows.iter().any(|w| w.name == name) {
                        continue;
                    }
                    let id = WindowId(self.next_id);
                    self.next_id += 1;
                    self.windows.push(WindowEntry {
                        name: name.clone(),
                        id,
                        live: true,
                    });
                    out.push(WindowChange::Opened {
                        id,
                        name,
                        owner,
                        origin,
                        config,
                        conflict,
                    });
                }
                return;
            }
            for i in gone {
                let w = self.windows.remove(i);
                self.remove_slot(w.id);
                if w.live {
                    out.push(WindowChange::Closed {
                        id: w.id,
                        name: w.name,
                    });
                }
            }
        }
    }

    /// The declared set: one entry per name, from the lowest declaring
    /// slot — whose window id rides along as the owner, since the slot key
    /// *is* the window whose frame declared it — with whether any
    /// declaration of it disagreed.
    fn union(&self) -> Vec<(Rc<str>, WindowConfig, WindowId, OriginId, bool)> {
        let mut union: Vec<(Rc<str>, WindowConfig, WindowId, OriginId, bool)> = Vec::new();
        for (slot, decls) in &self.slots {
            for d in decls {
                match union.iter_mut().find(|u| u.0 == d.name) {
                    Some(u) => u.4 |= u.1 != d.config,
                    None => union.push((d.name.clone(), d.config, *slot, d.origin, d.conflict)),
                }
            }
        }
        union
    }
}

/// The font database, registries and audio store a set of windows share.
/// Cloning one clones the handle, not the contents: that is how a second
/// `Core` joins (`Core::new_in(&session)`).
#[derive(Clone)]
pub struct Session(Rc<RefCell<SessionState>>);

impl Session {
    pub fn new() -> Self {
        Self(Rc::new(RefCell::new(SessionState::new())))
    }

    /// Whether two handles name the same session.
    pub fn is(&self, other: &Session) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }

    /// This session's process-wide id — the number a `foreign-resource`
    /// warning names.
    pub fn id(&self) -> SessionId {
        self.0.borrow().id
    }

    pub(crate) fn state(&self) -> RefMut<'_, SessionState> {
        self.0.borrow_mut()
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Session")
    }
}

/// A window's handle to the session's audio store — `Core`'s `audio`
/// field. The process has one audio device, so the playbacks and the
/// command queue are the session's and not this window's: whichever
/// window's driver drains the queue applies every window's sounds. The
/// `audio` nodes' mounts are per window inside it (see the module doc),
/// which is why the readers here take one.
#[derive(Clone)]
pub struct SharedAudio(Session);

impl SharedAudio {
    pub(crate) fn new(session: &Session) -> Self {
        Self(session.clone())
    }

    /// Drains the commands queued since the last drain.
    pub fn take_commands(&self) -> Vec<crate::audio::AudioCommand> {
        self.0.state().audio.take_commands()
    }

    /// The playback a keyed `audio` node of `window` is running, if it is
    /// mounted. `Core::playback_of` asks for the core's own window.
    pub fn playback_of(
        &self,
        window: WindowId,
        key: crate::key::Key,
    ) -> Option<crate::audio::PlaybackId> {
        self.0.state().audio.playback_of(window, key)
    }

    /// Whether any `audio` node of `window` is mounted.
    #[cfg(feature = "conformance")]
    pub(crate) fn any_mounted(&self, window: WindowId) -> bool {
        self.0.state().audio.any_mounted(window)
    }
}

/// A window's handle to the session's resource registry — `Core`'s
/// `resources` field. A font, image or sound registered through one is
/// registered for every window in the session, and draws in all of them.
#[derive(Clone)]
pub struct SharedResources(Session);

impl SharedResources {
    pub(crate) fn new(session: &Session) -> Self {
        Self(session.clone())
    }

    /// Registers an RGBA image (`width * height * 4` bytes).
    pub fn add_image(&self, width: u32, height: u32, rgba: Vec<u8>) -> ImageId {
        self.0.state().resources.add_image(width, height, rgba)
    }

    /// Drops an image from the registry. Every window's atlas forgets
    /// its blit at that window's next frame, and a texture-backed one is
    /// dropped from the device by the next frame any window draws.
    pub fn remove_image(&self, id: ImageId) -> bool {
        self.0.state().remove_image(id).is_some()
    }

    /// The pixel dimensions behind an image handle, if it is live.
    pub fn image_size(&self, id: ImageId) -> Option<(u32, u32)> {
        let state = self.0.state();
        let e = state.resources.image(id)?;
        Some((e.width, e.height))
    }

    /// Registers a sound from its encoded file bytes.
    pub fn add_sound(&self, bytes: Vec<u8>) -> SoundId {
        self.0.state().resources.add_sound(bytes)
    }

    /// Whether any sound is registered (see [`Resources::has_sounds`]).
    pub fn has_sounds(&self) -> bool {
        self.0.state().resources.has_sounds()
    }

    /// The encoded bytes behind a sound handle, if it is live. Cloning the
    /// `Arc` rather than lending it is what lets the registry be shared:
    /// the caller (an audio backend) holds the bytes, not the borrow.
    pub fn sound(&self, id: SoundId) -> Option<Arc<[u8]>> {
        self.0.state().resources.sound(id).cloned()
    }

    /// The registered family name behind a font handle, if it is live.
    pub fn font_family(&self, id: FontId) -> Option<String> {
        self.0.state().resources.font_family(id).map(str::to_owned)
    }
}
