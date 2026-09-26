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
    /// A registered WGSL fragment function (`Core::add_fragment`), drawn
    /// by a `fragment` node
    /// (`docs/adr/0015-a-fragment-element-and-the-painter-it-is-not.md`).
    pub struct FragmentId;
}

impl FragmentId {
    /// The handle as a plain integer for C/Lua/JS (generation check intact).
    pub fn to_ffi(self) -> u64 {
        self.data().as_ffi()
    }

    pub fn from_ffi(raw: u64) -> Self {
        Self::from(slotmap::KeyData::from_ffi(raw))
    }
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
    Fragment,
}

impl ResourceKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Font => "font",
            Self::Sound => "sound",
            Self::Fragment => "fragment",
        }
    }

    /// What a handle of this kind does when it does not resolve.
    fn fallback(self) -> &'static str {
        match self {
            Self::Image => "draws nothing",
            Self::Font => "shapes as sans-serif",
            Self::Sound => "plays nothing",
            Self::Fragment => "draws nothing",
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
    fragments: SlotMap<FragmentId, SessionId>,
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
    /// Shared rather than owned so a frame's display list can hand a
    /// backend the pixels of a texture-backed image without copying them
    /// (`DisplayList::texture_pixels`): an update replaces the `Arc`, and
    /// a backend mid-upload keeps the old one alive until it is done.
    pub rgba: std::sync::Arc<Vec<u8>>,
    /// Moves on every [`Resources::update_image`]; a backend re-uploads a
    /// texture-backed image when the revision it uploaded is behind.
    pub rev: u32,
    /// Where the pixels live on the GPU (ADR 0025, decision 2).
    pub backing: ImageBacking,
    /// The buffer the last [`Resources::update_image_with`] replaced, kept
    /// for the next one to write into once no display list holds it
    /// (backlog W20). An update between frames finds `rgba` still shared
    /// with the last frame's `texture_pixels`, so without it every update
    /// was a fresh `w × h × 4` allocation and the previous one freed —
    /// 590 µs of page faults and 170 µs of release at 1080p on Windows,
    /// three times the copy itself. Two buffers per streamed image, and a
    /// stream stops allocating from its fourth update. Kept only for an
    /// image updated before, at the same size, and dropped by
    /// [`Resources::release_spares`] once [`SPARE_FRAMES`] frames pass
    /// with no update: an image updated once, or a stream that stopped,
    /// holds one buffer again.
    pub(crate) spare: Option<std::sync::Arc<Vec<u8>>>,
    /// [`Resources::frames`] when `spare` was last set.
    spare_at: u64,
}

/// How many frames an image's spare buffer outlives its last update
/// (backlog W20). Long enough for a stream slower than the display — a
/// 30 fps video beside a 120 Hz animation updates every fourth frame —
/// and short enough that one that stopped gives its buffer back.
pub const SPARE_FRAMES: u64 = 30;

/// Where a registered image's pixels are kept for drawing
/// (`docs/adr/0025-the-image-is-the-canvas.md`, decision 2). The core
/// decides on the two facts that matter — whether the image fits an atlas
/// page, and whether its pixels were ever replaced — and the app never
/// chooses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageBacking {
    /// Blitted into the glyph atlas at first draw and drawn as
    /// [`crate::display::QuadKind::Image`]: icons, thumbnails, anything
    /// that fits and never changes.
    Atlas,
    /// A texture of its own, drawn as [`crate::display::QuadKind::Texture`]
    /// through an entry in `DisplayList::textures`: an image that does not
    /// fit a `MAX_ATLAS_SIZE` page, or one that has been updated in place.
    /// Once here, an image stays here.
    Texture,
}

/// How an `image` node meets the pixels it shows — the two per-node rows
/// ADR 0025 decision 4 gives it. Carried on the node's content rather than
/// on `NodeSpec`, so a box pays nothing for a row only an image reads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ImageOpts {
    pub sampling: Sampling,
    pub fit: ImageFit,
}

/// The `sampling` row: how a backend reads texels between pixel centres.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sampling {
    /// Bilinear — a photo, a rendered frame (the default).
    #[default]
    Linear,
    /// Nearest texel — pixel art, an emulator, a data grid that must stay
    /// square under zoom.
    Nearest,
}

impl Sampling {
    /// Every mode, in the order the `sampling` row names them.
    pub const ALL: [Sampling; 2] = [Sampling::Linear, Sampling::Nearest];

    pub fn name(self) -> &'static str {
        match self {
            Sampling::Linear => "linear",
            Sampling::Nearest => "nearest",
        }
    }
}

/// The `fit` row: how the pixels meet the node's box. The box itself —
/// its layout, its hit region, its access rect — is the same in every
/// mode; only what is painted inside it moves.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ImageFit {
    /// The pixels stretch to the box (the default, and what every image
    /// did before the row existed).
    #[default]
    Fill,
    /// The largest rect of the image's aspect that fits the box, centred;
    /// the rest of the box shows what is behind it.
    Contain,
    /// The box is filled and the pixels that do not fit are cropped,
    /// centred.
    Cover,
}

impl ImageFit {
    /// Every mode, in the order the `fit` row names them.
    pub const ALL: [ImageFit; 3] = [ImageFit::Fill, ImageFit::Contain, ImageFit::Cover];

    pub fn name(self) -> &'static str {
        match self {
            ImageFit::Fill => "fill",
            ImageFit::Contain => "contain",
            ImageFit::Cover => "cover",
        }
    }
}

/// A registered font: the family name shaping resolves it by, the faces
/// it loaded into the font database (empty for installed fonts), and the
/// weights the family is asked at for regular and bold (backlog F100).
pub struct FontEntry {
    pub family: String,
    pub faces: Vec<cosmic_text::fontdb::ID>,
    pub(crate) weights: crate::weights::Weights,
}

/// One family of the font database — installed or loaded — as its faces
/// describe it, from what the database read off each face's tables when
/// it was scanned: nothing is loaded or shaped to answer (backlog F97).
/// What [`Core::system_fonts`](crate::Core::system_fonts) lists, one per
/// family.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SystemFont {
    /// The family name [`Core::add_system_font`](crate::Core::add_system_font)
    /// takes.
    pub family: String,
    /// Every face of the family says it is fixed-pitch (the `post` table's
    /// `isFixedPitch`): a monospaced family. Measuring glyph widths instead
    /// loads and shapes every file, and calls a symbol font whose glyphs
    /// happen to share an advance monospaced.
    pub monospaced: bool,
    /// The weights its faces come in, on the CSS scale (400 regular, 700
    /// bold) as each face's `OS/2` table says it — sorted, each once. A
    /// variable font's face reads as its default instance.
    pub weights: Vec<u16>,
    /// It has an italic or an oblique face.
    pub italic: bool,
}

/// A registered sound: the encoded file (wav/ogg/mp3/flac, whatever the
/// driver's backend decodes), shared so the backend can hold it without a
/// copy. The core never decodes — headless drivers have no use for PCM.
pub struct SoundEntry {
    pub bytes: Arc<[u8]>,
}

/// A registered fragment: the app's WGSL as it was given, which is what a
/// backend compiles (around the core's prelude and epilogue — see
/// `crate::fragment`) and what a C host reads back to compile itself.
pub struct FragmentEntry {
    /// The app's source, without the prelude or the epilogue.
    pub source: Arc<str>,
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
    pub(crate) fragments: SparseSecondaryMap<FragmentId, FragmentEntry>,
    /// Handles of other sessions this registry was asked for since the
    /// last drain, each once. A `RefCell` because the resolves that find
    /// them (`family_of` under a shaping closure, `image` under the
    /// emitter's shared borrow) hold the registry by `&`.
    foreign: RefCell<Vec<Foreign>>,
    /// Frames begun by any window of the session, for aging spares.
    frames: u64,
    /// The images holding a spare buffer (W20), swept each frame.
    spared: Vec<ImageId>,
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
            fragments: SparseSecondaryMap::new(),
            foreign: RefCell::new(Vec::new()),
            frames: 0,
            spared: Vec::new(),
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
                ResourceKind::Fragment => m.fragments.get(FragmentId::from_ffi(raw)).copied(),
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
        self.fonts.insert(
            id,
            FontEntry {
                family,
                faces,
                weights: crate::weights::Weights::CSS,
            },
        );
        id
    }

    /// Reads again the weights of the registered families `families` holds
    /// from what `db` has of them now (backlog F100): after a family is
    /// registered, and after faces of it come into the database or leave.
    /// Whether the weights of a font other than `fresh` — the one just
    /// registered, which nothing has shaped in yet — changed: text shaped
    /// in it was shaped at the old ones (RG59).
    pub(crate) fn reweigh(
        &mut self,
        db: &cosmic_text::fontdb::Database,
        families: &rustc_hash::FxHashSet<String>,
        fresh: Option<FontId>,
    ) -> bool {
        let mut changed = false;
        for (id, entry) in self.fonts.iter_mut() {
            if families.contains(&entry.family) {
                let weights = crate::weights::Weights::of(db, &entry.family);
                changed |= Some(id) != fresh && weights != entry.weights;
                entry.weights = weights;
            }
        }
        changed
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

    /// The weights a style's `FontFamily` is asked at (backlog F100): a
    /// registered family's own, the CSS ones for a generic family and for
    /// an unknown or removed custom font (which shapes as sans-serif).
    pub(crate) fn weights_of(&self, f: FontFamily) -> crate::weights::Weights {
        match f {
            FontFamily::Custom(id) => self
                .fonts
                .get(id)
                .map_or(crate::weights::Weights::CSS, |entry| entry.weights),
            _ => crate::weights::Weights::CSS,
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
                rgba: std::sync::Arc::new(rgba),
                rev: 0,
                spare: None,
                spare_at: 0,
                // Past a page it has nowhere to go but its own texture;
                // before ADR 0025 it was dropped at emission, silently.
                backing: if width > crate::atlas::MAX_ATLAS_SIZE
                    || height > crate::atlas::MAX_ATLAS_SIZE
                {
                    ImageBacking::Texture
                } else {
                    ImageBacking::Atlas
                },
            },
        );
        id
    }

    /// Replaces an image's pixels in place (ADR 0025, decision 1): the
    /// handle is unchanged, so every node declaring it shows the new
    /// pixels next frame with no view change; the dimensions may change.
    /// From the first update on the image is texture-backed for life.
    /// Returns whether the pixels were taken: false for a foreign or
    /// removed handle (noted as a miss), and for a buffer that is not
    /// `width × height × 4` bytes, which changes nothing rather than
    /// handing a backend a short upload it would refuse with a validation
    /// error — the doors that take bytes from an app check the length
    /// first and say so; this is the guard behind them.
    pub fn update_image(&mut self, id: ImageId, width: u32, height: u32, rgba: Vec<u8>) -> bool {
        if rgba.len() != width as usize * height as usize * 4 {
            return false;
        }
        let Some(entry) = self.images.get_mut(id) else {
            self.note_miss(ResourceKind::Image, id.to_ffi());
            return false;
        };
        entry.width = width;
        entry.height = height;
        entry.rgba = std::sync::Arc::new(rgba);
        entry.spare = None;
        entry.rev = entry.rev.wrapping_add(1);
        entry.backing = ImageBacking::Texture;
        true
    }

    /// [`Self::update_image`] into a buffer the core recycles: `fill` is
    /// handed `width × height × 4` bytes to write the new pixels into, and
    /// is not called for a foreign or removed handle (noted as a miss),
    /// or for a size whose byte count overflows. The bytes it is handed
    /// hold an earlier frame's pixels, not zeros, so `fill` writes every
    /// one. The buffer is the image's own when no display list still
    /// holds it, else the one the previous update replaced, else a new
    /// one — so a stream updated every frame, between frames or inside
    /// them, allocates at most three times and then never again (backlog
    /// W20). The replaced buffer is kept only for an image updated before
    /// at the same size, and let go [`SPARE_FRAMES`] frames after the
    /// last update. What every door that copies an app's bytes goes
    /// through.
    pub fn update_image_with(
        &mut self,
        id: ImageId,
        width: u32,
        height: u32,
        fill: impl FnOnce(&mut [u8]),
    ) -> bool {
        use std::sync::Arc;
        let Some(len) = (width as usize)
            .checked_mul(height as usize)
            .and_then(|n| n.checked_mul(4))
        else {
            return false;
        };
        let Some(entry) = self.images.get_mut(id) else {
            self.note_miss(ResourceKind::Image, id.to_ffi());
            return false;
        };
        if Arc::get_mut(&mut entry.rgba).is_none() {
            // The last frame's display list (or a backend mid-upload)
            // still reads the current buffer: write into the spare if
            // nothing reads that any more, else into a new one. `vec!`
            // rather than a resize, so a fresh buffer's zeros are the
            // allocator's and not a pass over it.
            let had = entry.spare.is_some();
            // Unique by both counts: a `Weak` a host took of pixels it
            // was handed makes `get_mut` refuse the buffer as well.
            let next = match entry.spare.take() {
                Some(spare) if Arc::strong_count(&spare) == 1 && Arc::weak_count(&spare) == 0 => {
                    spare
                }
                _ => Arc::new(vec![0; len]),
            };
            let replaced = std::mem::replace(&mut entry.rgba, next);
            // The replaced buffer is worth keeping for a stream: an image
            // updated before (not the pixels it was added with) whose size
            // holds. A one-off update, or a resize, keeps nothing.
            if entry.rev > 0 && replaced.len() == len {
                entry.spare = Some(replaced);
                entry.spare_at = self.frames;
                if !had {
                    self.spared.push(id);
                }
            }
        }
        // The size, revision and backing before `fill`, so a `fill` that
        // panics leaves stale pixels at the right length rather than a
        // buffer whose length its size does not match.
        entry.width = width;
        entry.height = height;
        entry.rev = entry.rev.wrapping_add(1);
        entry.backing = ImageBacking::Texture;
        let buf = Arc::get_mut(&mut entry.rgba).expect("unshared: checked or replaced above");
        buf.resize(len, 0);
        // A stream that shrank does not keep its old size's allocation.
        if buf.capacity() > len.saturating_mul(2) {
            buf.shrink_to(len);
        }
        fill(buf);
        true
    }

    /// Called as each frame begins: drops the spare buffer of every image
    /// not updated for [`SPARE_FRAMES`] frames (W20).
    pub(crate) fn release_spares(&mut self) {
        self.frames += 1;
        if self.spared.is_empty() {
            return;
        }
        let (images, frames) = (&mut self.images, self.frames);
        self.spared.retain(|&id| match images.get_mut(id) {
            Some(entry) if entry.spare.is_some() => {
                if frames - entry.spare_at > SPARE_FRAMES {
                    entry.spare = None;
                    false
                } else {
                    true
                }
            }
            _ => false,
        });
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

    /// The handle an identical source already has, if any. What makes a
    /// view that calls `add_fragment` every frame cost a comparison
    /// instead of a validation (55-73 us) and a pipeline build.
    pub(crate) fn find_fragment(&self, source: &str) -> Option<FragmentId> {
        self.fragments
            .iter()
            .find(|(_, f)| &*f.source == source)
            .map(|(id, _)| id)
    }

    /// Registers validated WGSL, or hands back the handle an identical
    /// source already has.
    pub(crate) fn add_fragment(&mut self, source: &str) -> FragmentId {
        if let Some(id) = self.find_fragment(source) {
            return id;
        }
        let id = mint().fragments.insert(self.session);
        self.fragments.insert(
            id,
            FragmentEntry {
                source: Arc::from(source),
            },
        );
        id
    }

    pub(crate) fn remove_fragment(&mut self, id: FragmentId) -> Option<FragmentEntry> {
        let entry = self.fragments.remove(id);
        match entry {
            Some(_) => {
                mint().fragments.remove(id);
            }
            None => self.note_miss(ResourceKind::Fragment, id.to_ffi()),
        }
        entry
    }

    /// The WGSL behind a fragment handle, if it is live here. A miss is
    /// what makes the node draw nothing, and records a foreign handle.
    pub fn fragment(&self, id: FragmentId) -> Option<&Arc<str>> {
        let entry = self.fragments.get(id);
        if entry.is_none() {
            self.note_miss(ResourceKind::Fragment, id.to_ffi());
        }
        entry.map(|f| &f.source)
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
        for (id, _) in self.fragments.iter() {
            m.fragments.remove(id);
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

    /// RG59: a reweigh reports a change only to a family registered
    /// before — text may have been shaped in it — and never for the one
    /// just registered, so a list registering a family per row as it
    /// scrolls does not drop every window's shaped text each time.
    #[test]
    fn a_reweigh_reports_only_a_family_text_may_be_shaped_in() {
        use crate::weights::Weights;
        use cosmic_text::fontdb::{Database, Source};
        let mut db = Database::new();
        let load = |db: &mut Database, weight| {
            let bytes = crate::testing::font_face("Kui Fresh", weight, false, true);
            db.load_font_source(Source::Binary(std::sync::Arc::new(bytes)));
        };
        load(&mut db, 400);
        let touched = std::iter::once("Kui Fresh".to_string()).collect();
        let mut r = Resources::new(SessionId::next());
        let id = r.add_font("Kui Fresh".into(), vec![]);
        assert!(!r.reweigh(&db, &touched, Some(id)), "just registered");
        assert_ne!(r.weights_of(FontFamily::Custom(id)), Weights::CSS);
        assert!(!r.reweigh(&db, &touched, None), "nothing moved");
        load(&mut db, 700);
        assert!(r.reweigh(&db, &touched, None), "its Bold came");
        assert_eq!(r.weights_of(FontFamily::Custom(id)), Weights::CSS);
    }

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

    /// Raw 0 is index 0, the slot the mint never fills, so it misses in
    /// every session and is nobody's — the `fragments` scene's dead `src`
    /// and the doors' "no image" both rest on it. Raw 1 is *not* that:
    /// `from_ffi` reads every handle at an odd generation, so it is the
    /// first key a fresh process hands out (C31).
    #[test]
    fn raw_zero_is_dead_whatever_was_minted() {
        let mut a = Resources::new(SessionId::next());
        let b = Resources::new(SessionId::next());
        let _ = a.add_image(1, 1, vec![0; 4]);
        let _ = a.add_fragment("");
        for r in [&a, &b] {
            assert!(r.image(ImageId::from_ffi(0)).is_none());
            assert!(r.fragment(FragmentId::from_ffi(0)).is_none());
            assert!(r.take_foreign().is_empty(), "raw 0 is nobody's");
        }
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
