//! What a window does not own alone.
//!
//! kui is a `Core` per window (see `docs/adr/0004-multi-window.md`), and
//! every per-frame singleton — tree, display list, focus, hit list — stays
//! per-window. What must not be duplicated is here: the **font database**
//! every window shapes against, the **resource registry** behind every
//! `FontId` / `ImageId` / `SoundId`, and the **audio store**, because the
//! process has one audio device and not one per window. A `Session` owns
//! them and any number of `Core`s are constructed against it
//! (`Core::new_in`), so a font, image or sound registered in one window
//! draws and plays in every window of the session.
//!
//! `Core::new()` is sugar for a private session of one, which is why
//! nothing above the core — no test, no binding, no conformance scene —
//! has to know a session exists.
//!
//! **What stays per window, and why.** The shaped-text cache and the glyph
//! atlas do not move here even though they look like caches. They are one
//! unit with a window's texture: `CachedText` stamps its positioned glyphs
//! with the atlas epoch they were packed against, so a cache entry is only
//! valid for the page it was built from, and `Core::output` lends that page
//! to the renderer as `&mut GlyphAtlas`. See the ADR note in
//! `docs/adr/0004-multi-window.md`.
//!
//! **Borrow discipline.** The session is `Rc<RefCell<_>>`: shared
//! ownership with a runtime check. A `Core` takes the borrow inside a
//! method and drops it before returning, so a re-entrant borrow is not
//! reachable from outside; the windows of a multi-window app are driven one
//! frame at a time on one thread, so the check never contends. The one
//! thing a `Core` lends out of the session — the family name behind
//! `Core::font_family` — cannot live behind that borrow, so it is read from
//! a per-window mirror that every registration and every frame refreshes.

use std::cell::{RefCell, RefMut};
use std::rc::Rc;
use std::sync::Arc;

use cosmic_text::FontSystem;

use crate::audio::AudioStore;
use crate::resources::{FontId, ImageId, Resources, SoundId};

/// The session's contents. Reached through [`Session::state`], one borrow
/// at a time; the fields are borrowed disjointly the way `Core`'s own
/// fields used to be.
pub(crate) struct SessionState {
    /// The font database every window in the session shapes against — a
    /// face loaded by one window resolves for all of them.
    pub(crate) fonts: FontSystem,
    pub(crate) resources: Resources,
    pub(crate) audio: AudioStore,
    /// Bumped by every font registration and removal, so a `Core` can tell
    /// whether its name mirror is behind without walking the slotmap.
    pub(crate) fonts_rev: u64,
}

impl SessionState {
    fn new() -> Self {
        Self {
            fonts: crate::text::new_font_system(),
            resources: Resources::default(),
            audio: AudioStore::default(),
            fonts_rev: 0,
        }
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
/// window's driver drains the queue applies every window's sounds.
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

    /// The playback a keyed `audio` node is running, if it is mounted.
    pub fn playback_of(&self, key: crate::key::Key) -> Option<crate::audio::PlaybackId> {
        self.0.state().audio.playback_of(key)
    }

    /// Whether any `audio` node is mounted.
    pub(crate) fn any_mounted(&self) -> bool {
        self.0.state().audio.any_mounted()
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

    /// Drops an image from the registry. The atlas keeps its blit until
    /// the owning `Core` evicts it (`Core::remove_image` does both).
    pub fn remove_image(&self, id: ImageId) -> bool {
        self.0.state().resources.remove_image(id).is_some()
    }

    /// The pixel dimensions behind an image handle, if it is live.
    pub fn image_size(&self, id: ImageId) -> Option<(u32, u32)> {
        let state = self.0.state();
        let e = state.resources.images.get(id)?;
        Some((e.width, e.height))
    }

    /// Registers a sound from its encoded file bytes.
    pub fn add_sound(&self, bytes: Vec<u8>) -> SoundId {
        self.0.state().resources.add_sound(bytes)
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
