//! Long-lived, host-registered resources. Slotmap keys give typed handles
//! with generational use-after-free protection, and convert to/from `u64`
//! (`KeyData::as_ffi`) so they cross the scripting boundary as plain integers
//! with the generation check intact on the way back.
//!
//! **A handle is unique to the process, not to its session.** Every
//! `Session` has its own registry, and two registries that each minted
//! their own keys would mint the same ones — slot 0, generation 1 — so an
//! `ImageId` from one session, looked up in another, would draw *that*
//! session's first image, silently. To make that a detectable miss rather
//! than an alias, the keys come from one process-wide slotmap per kind
//! ([`Mint`]), which also records the session that owns each; a session's
//! registry is a secondary map over those keys, so it can only ever hold
//! entries it registered itself. A handle that misses here is then one of
//! two things, and the mint tells them apart: no longer live anywhere
//! (removed — the documented behaviour: the image draws nothing, the font
//! shapes as sans, the sound is silent), or live in *another* session
//! (foreign — the same behaviour, plus a `foreign-resource` warning, kept
//! on the registry until a `Core::take_warnings` drains it). The mint is
//! touched on registration, removal and the miss path only; a live lookup
//! never locks it.

use std::cell::RefCell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard};

use slotmap::{Key as _, SlotMap, SparseSecondaryMap, new_key_type};

use crate::spec::FontFamily;

new_key_type! {
    pub struct ImageId;
    pub struct PainterId;
    /// A registered font (`Core::add_font_data` / `add_system_font`), used
    /// through `TextStyle::font`.
    pub struct FontId;
    /// A registered sound (`Core::add_sound`): encoded file bytes the
    /// driver's audio backend decodes. Played through `Core::play`, an
    /// `audio` node, or `NodeSpec::click_sound` / `hover_sound`.
    pub struct SoundId;
}

impl SoundId {
    /// The handle as a plain integer for C/Lua/JS (generation check intact).
    pub fn to_ffi(self) -> u64 {
        self.data().as_ffi()
    }

    pub fn from_ffi(raw: u64) -> Self {
        Self::from(slotmap::KeyData::from_ffi(raw))
    }
}

impl FontId {
    /// The handle as a plain integer for C/Lua/JS (generation check intact).
    pub fn to_ffi(self) -> u64 {
        self.data().as_ffi()
    }

    pub fn from_ffi(raw: u64) -> Self {
        Self::from(slotmap::KeyData::from_ffi(raw))
    }
}

impl ImageId {
    /// The handle as a plain integer for C/Lua (generation check intact).
    pub fn to_ffi(self) -> u64 {
        self.data().as_ffi()
    }

    pub fn from_ffi(raw: u64) -> Self {
        Self::from(slotmap::KeyData::from_ffi(raw))
    }
}

/// Which `Session` a registry — and so every handle it minted — belongs
/// to. Process-wide unique, from a counter; never serialized, so the
/// number means nothing across runs and is only ever compared or printed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SessionId(pub u64);

impl SessionId {
    /// The next unused id. Starts at 1 so a zeroed struct is never a
    /// session.
    pub(crate) fn next() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}

/// The kind of resource a handle names, for the warning that reports a
/// foreign one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceKind {
    Image,
    Font,
    Sound,
}

impl ResourceKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Font => "font",
            Self::Sound => "sound",
        }
    }

    /// What a handle of this kind does when it does not resolve.
    fn fallback(self) -> &'static str {
        match self {
            Self::Image => "draws nothing",
            Self::Font => "shapes as sans-serif",
            Self::Sound => "plays nothing",
        }
    }
}

/// A handle from another session that this registry was asked to resolve:
/// what `diag::foreign_resource` turns into a warning.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Foreign {
    pub kind: ResourceKind,
    /// The handle as `to_ffi` shows it — what a host would have logged.
    pub raw: u64,
    /// The session the handle is live in.
    pub owner: SessionId,
    /// The session that was asked.
    pub here: SessionId,
}

impl Foreign {
    /// The one-line explanation: which handle, whose it is, what it did
    /// instead, and the fix.
    pub fn message(&self) -> String {
        let Foreign {
            kind,
            raw,
            owner,
            here,
        } = *self;
        let what = kind.name();
        let did = kind.fallback();
        format!(
            "{what} handle {raw:#x} belongs to session #{} and was used in session #{}: a \
             handle is only valid in the session that registered it, so this one is treated \
             as removed and {did} — register the {what} through a core of this session, or \
             build both cores against one `Session` (`Core::new_in`)",
            owner.0, here.0
        )
    }
}

/// Foreign hits a registry keeps before it stops recording: a `Core`
/// drains them on every `take_warnings`, and the diagnostics' own dedup
/// means one line per handle, so a handful is plenty.
const MAX_FOREIGN: usize = 64;

/// The process's one allocator of handles, per kind, with the session each
/// live handle belongs to. A session's registry never mints a key itself:
/// it takes one from here and stores its entry under it, which is what
/// keeps two sessions from ever holding the same bits for different
/// things.
#[derive(Default)]
struct Mint {
    images: SlotMap<ImageId, SessionId>,
    fonts: SlotMap<FontId, SessionId>,
    sounds: SlotMap<SoundId, SessionId>,
}

static MINT: LazyLock<Mutex<Mint>> = LazyLock::new(Mutex::default);

/// The mint, past a poisoned lock: the maps hold plain ids and session
/// numbers, so a panic mid-insert on another thread leaves nothing to
/// distrust.
fn mint() -> MutexGuard<'static, Mint> {
    MINT.lock().unwrap_or_else(|e| e.into_inner())
}

/// An RGBA image registered by the host (rendering lands in a later pass).
pub struct ImageEntry {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// A registered font: the family name shaping resolves it by, and the
/// faces it loaded into the font database (empty for installed fonts).
pub struct FontEntry {
    pub family: String,
    pub faces: Vec<cosmic_text::fontdb::ID>,
}

/// A registered sound: the encoded file (wav/ogg/mp3/flac, whatever the
/// driver's backend decodes), shared so the backend can hold it without a
/// copy. The core never decodes — headless drivers have no use for PCM.
pub struct SoundEntry {
    pub bytes: Arc<[u8]>,
}

/// One session's registry. The maps are secondary to the process-wide
/// [`Mint`] (see the module doc), so a lookup that misses is a handle this
/// session never registered or has since removed — never somebody else's
/// entry.
pub struct Resources {
    session: SessionId,
    pub(crate) images: SparseSecondaryMap<ImageId, ImageEntry>,
    pub(crate) fonts: SparseSecondaryMap<FontId, FontEntry>,
    pub(crate) sounds: SparseSecondaryMap<SoundId, SoundEntry>,
    /// Handles of other sessions this registry was asked for since the
    /// last drain, each once. A `RefCell` because the resolves that find
    /// them (`family_of` under a shaping closure, `image` under the
    /// emitter's shared borrow) hold the registry by `&`.
    foreign: RefCell<Vec<Foreign>>,
}

impl Resources {
    /// The registry for session `session`; keys come from the process's
    /// mint.
    pub fn new(session: SessionId) -> Self {
        Self {
            session,
            images: SparseSecondaryMap::new(),
            fonts: SparseSecondaryMap::new(),
            sounds: SparseSecondaryMap::new(),
            foreign: RefCell::new(Vec::new()),
        }
    }

    /// The session this registry belongs to.
    pub fn session(&self) -> SessionId {
        self.session
    }

    /// The foreign handles resolved since the last call (see
    /// [`Foreign`]); `Core::take_warnings` turns each into one line.
    pub(crate) fn take_foreign(&self) -> Vec<Foreign> {
        std::mem::take(&mut *self.foreign.borrow_mut())
    }

    /// A lookup missed: the mint says whether the handle is live in some
    /// other session (recorded, once) or in none (removed — silent, the
    /// documented behaviour). Only the miss path pays for the lock.
    fn note_miss(&self, kind: ResourceKind, raw: u64) {
        let owner = {
            let m = mint();
            match kind {
                ResourceKind::Image => m.images.get(ImageId::from_ffi(raw)).copied(),
                ResourceKind::Font => m.fonts.get(FontId::from_ffi(raw)).copied(),
                ResourceKind::Sound => m.sounds.get(SoundId::from_ffi(raw)).copied(),
            }
        };
        let Some(owner) = owner else {
            return;
        };
        if owner == self.session {
            return;
        }
        let mut foreign = self.foreign.borrow_mut();
        if foreign.len() >= MAX_FOREIGN || foreign.iter().any(|f| f.kind == kind && f.raw == raw) {
            return;
        }
        foreign.push(Foreign {
            kind,
            raw,
            owner,
            here: self.session,
        });
    }

    pub(crate) fn add_font(
        &mut self,
        family: String,
        faces: Vec<cosmic_text::fontdb::ID>,
    ) -> FontId {
        let id = mint().fonts.insert(self.session);
        self.fonts.insert(id, FontEntry { family, faces });
        id
    }

    pub(crate) fn remove_font(&mut self, id: FontId) -> Option<FontEntry> {
        let entry = self.fonts.remove(id);
        match entry {
            Some(_) => {
                mint().fonts.remove(id);
            }
            None => self.note_miss(ResourceKind::Font, id.to_ffi()),
        }
        entry
    }

    /// The registered family name, if the handle is live here.
    pub fn font_family(&self, id: FontId) -> Option<&str> {
        let entry = self.fonts.get(id);
        if entry.is_none() {
            self.note_miss(ResourceKind::Font, id.to_ffi());
        }
        entry.map(|f| f.family.as_str())
    }

    /// The cosmic-text family a style's `FontFamily` shapes with; an unknown
    /// or removed custom font falls back to sans-serif.
    pub(crate) fn family_of(&self, f: FontFamily) -> cosmic_text::Family<'_> {
        match f {
            FontFamily::Sans => cosmic_text::Family::SansSerif,
            FontFamily::Serif => cosmic_text::Family::Serif,
            FontFamily::Mono => cosmic_text::Family::Monospace,
            FontFamily::Custom(id) => match self.font_family(id) {
                Some(name) => cosmic_text::Family::Name(name),
                None => cosmic_text::Family::SansSerif,
            },
        }
    }

    pub fn add_image(&mut self, width: u32, height: u32, rgba: Vec<u8>) -> ImageId {
        debug_assert_eq!(rgba.len(), (width * height * 4) as usize);
        let id = mint().images.insert(self.session);
        self.images.insert(
            id,
            ImageEntry {
                width,
                height,
                rgba,
            },
        );
        id
    }

    pub fn remove_image(&mut self, id: ImageId) -> Option<ImageEntry> {
        let entry = self.images.remove(id);
        match entry {
            Some(_) => {
                mint().images.remove(id);
            }
            None => self.note_miss(ResourceKind::Image, id.to_ffi()),
        }
        entry
    }

    /// The pixels behind an image handle, if it is live here.
    pub fn image(&self, id: ImageId) -> Option<&ImageEntry> {
        let entry = self.images.get(id);
        if entry.is_none() {
            self.note_miss(ResourceKind::Image, id.to_ffi());
        }
        entry
    }

    /// Registers a sound from its encoded file bytes.
    pub fn add_sound(&mut self, bytes: Vec<u8>) -> SoundId {
        let id = mint().sounds.insert(self.session);
        self.sounds.insert(
            id,
            SoundEntry {
                bytes: Arc::from(bytes),
            },
        );
        id
    }

    pub fn remove_sound(&mut self, id: SoundId) -> Option<SoundEntry> {
        let entry = self.sounds.remove(id);
        match entry {
            Some(_) => {
                mint().sounds.remove(id);
            }
            None => self.note_miss(ResourceKind::Sound, id.to_ffi()),
        }
        entry
    }

    /// Whether any sound is registered: what tells an audio backend it
    /// will be asked to play something, before it is.
    pub fn has_sounds(&self) -> bool {
        !self.sounds.is_empty()
    }

    /// The encoded bytes behind a sound handle, if it is live here.
    pub fn sound(&self, id: SoundId) -> Option<&Arc<[u8]>> {
        let entry = self.sounds.get(id);
        if entry.is_none() {
            self.note_miss(ResourceKind::Sound, id.to_ffi());
        }
        entry.map(|s| &s.bytes)
    }
}

impl Drop for Resources {
    /// A session's handles leave the mint with it, so the slots come back
    /// and a process that opens and closes many sessions (a test binary)
    /// does not keep every id it ever minted.
    fn drop(&mut self) {
        let mut m = mint();
        for (id, _) in self.images.iter() {
            m.images.remove(id);
        }
        for (id, _) in self.fonts.iter() {
            m.fonts.remove(id);
        }
        for (id, _) in self.sounds.iter() {
            m.sounds.remove(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slotmap::KeyData;

    #[test]
    fn stale_handle_is_rejected_after_removal() {
        let mut r = Resources::new(SessionId::next());
        let id = r.add_image(1, 1, vec![0; 4]);
        r.remove_image(id);
        assert!(r.image(id).is_none());
        // A new insert may reuse the slot but bumps the generation.
        let id2 = r.add_image(1, 1, vec![0; 4]);
        assert_ne!(id, id2);
        assert!(r.image(id).is_none());
        assert!(r.image(id2).is_some());
        // Removed, not foreign: nothing to report.
        assert!(r.take_foreign().is_empty());
    }

    #[test]
    fn handles_round_trip_through_u64() {
        let mut r = Resources::new(SessionId::next());
        let id = r.add_image(1, 1, vec![0; 4]);
        let raw = id.data().as_ffi();
        let back = ImageId::from(KeyData::from_ffi(raw));
        assert_eq!(id, back);
        assert!(r.image(back).is_some());
    }

    #[test]
    fn two_registries_never_mint_the_same_handle() {
        let mut a = Resources::new(SessionId::next());
        let mut b = Resources::new(SessionId::next());
        let ia = a.add_image(1, 1, vec![0; 4]);
        let ib = b.add_image(2, 2, vec![0; 16]);
        assert_ne!(ia, ib, "two first images, two handles");
        assert_ne!(
            a.add_font("A".into(), vec![]),
            b.add_font("B".into(), vec![])
        );
        assert_ne!(a.add_sound(vec![0]), b.add_sound(vec![1]));
    }

    #[test]
    fn a_foreign_handle_misses_and_is_reported_once() {
        let mut a = Resources::new(SessionId::next());
        let b = Resources::new(SessionId::next());
        let id = a.add_image(1, 1, vec![0; 4]);
        assert!(b.image(id).is_none(), "b never draws a's pixels");
        assert!(b.image(id).is_none());
        let hits = b.take_foreign();
        assert_eq!(
            hits,
            [Foreign {
                kind: ResourceKind::Image,
                raw: id.to_ffi(),
                owner: a.session(),
                here: b.session(),
            }]
        );
        assert!(b.take_foreign().is_empty(), "drained");
        // The owner resolving its own handle records nothing.
        assert!(a.image(id).is_some());
        assert!(a.take_foreign().is_empty());
        // Once the owner removes it, the miss is a removal everywhere.
        a.remove_image(id);
        assert!(b.image(id).is_none());
        assert!(b.take_foreign().is_empty());
    }

    #[test]
    fn removing_a_foreign_handle_touches_nothing_and_reports() {
        let mut a = Resources::new(SessionId::next());
        let mut b = Resources::new(SessionId::next());
        let sound = a.add_sound(vec![1, 2, 3]);
        assert!(b.remove_sound(sound).is_none());
        assert!(a.sound(sound).is_some(), "a's sound is still a's");
        assert_eq!(b.take_foreign()[0].kind, ResourceKind::Sound);
    }

    #[test]
    fn a_dropped_registry_gives_its_slots_back() {
        let id = {
            let mut a = Resources::new(SessionId::next());
            a.add_image(1, 1, vec![0; 4])
        };
        assert!(mint().images.get(id).is_none(), "gone with its session");
        let b = Resources::new(SessionId::next());
        assert!(b.image(id).is_none());
        assert!(
            b.take_foreign().is_empty(),
            "a dead session's handle is just removed"
        );
    }
}
