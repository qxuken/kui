//! Resources through a core: fonts, sounds and images live in the
//! session's registry (`resources`), so registering one here registers
//! it for every window; playback is commands the driver drains
//! (`audio`). Nothing here touches a device.

use super::*;

impl Core {
    /// Turns the sounds nodes asked for (`click_sound` / `hover_sound`)
    /// into play commands. Declarative sounds carry no tag, so they never
    /// report `ended`.
    pub(crate) fn flush_sound_requests(&mut self) {
        let window = self.env.window.id;
        let mut sess = self.session.state();
        for sound in self.interaction.take_sound_requests() {
            sess.audio.play(
                OriginId::HOST,
                window,
                Key::ROOT,
                sound,
                crate::audio::PlayOptions::default(),
            );
        }
    }

    // -- Fragments ------------------------------------------------------

    /// Registers a WGSL fragment function for a `fragment` node
    /// (`docs/adr/0015-a-fragment-element-and-the-painter-it-is-not.md`).
    /// The app writes one function:
    ///
    /// ```wgsl
    /// fn fragment(in: FragmentIn, params: array<vec4<f32>, 4>) -> vec4<f32>
    /// ```
    ///
    /// and the core wraps it in the prelude and epilogue that give it the
    /// node's rounded box, the inherited clip, the group opacity and the
    /// blend (see `crate::fragment`). `None` when the source does not
    /// compile, with a `fragment-rejected` warning carrying naga's message
    /// in the app's own line numbers — so a bad shader is a warning at
    /// registration, in a headless test included, and never a blank box in
    /// a window.
    ///
    /// Idempotent by source: the same text gets the same handle without
    /// validating again, so a view may call this every frame. Registering
    /// at startup is still the advice, because the *backend* builds a
    /// pipeline the first time it sees a handle.
    pub fn add_fragment(&mut self, wgsl: &str) -> Option<crate::resources::FragmentId> {
        if let Some(id) = self.session.state().resources.find_fragment(wgsl) {
            return Some(id);
        }
        if let Err(message) = crate::fragment::validate(wgsl) {
            self.warn(crate::diag::Warning {
                code: crate::diag::FRAGMENT_REJECTED,
                key: crate::key::Key::ROOT,
                message: format!("fragment source rejected: {message}"),
            });
            return None;
        }
        Some(self.session.state().resources.add_fragment(wgsl))
    }

    /// Forgets a registered fragment. Nodes still naming it draw nothing,
    /// and the next frame any window draws has the backend drop the
    /// pipelines it built for it.
    pub fn remove_fragment(&mut self, id: crate::resources::FragmentId) {
        self.session.state().remove_fragment(id);
        // The stock polygon's handle, if that is what went: the next
        // `polygon` node registers it again rather than drawing nothing.
        if self.stock_polygon == Some(id) {
            self.stock_polygon = None;
        }
    }

    /// The whole WGSL module behind a fragment handle — the app's source
    /// between the core's prelude and epilogue — which is what a backend
    /// compiles. A host rendering the display list itself asks for this
    /// rather than assembling its own, so what it compiles is what the
    /// core validated.
    pub fn fragment_module_source(&self, id: crate::resources::FragmentId) -> Option<String> {
        let sess = self.session.state();
        let app = sess.resources.fragment(id)?;
        Some(crate::fragment::module_source(app))
    }

    // -- Fonts ----------------------------------------------------------

    /// Registers a font from its file bytes (TTF/OTF/TTC); `None` when the
    /// data holds no usable face — none the font database can read, or
    /// none whose glyphs can be measured (no `head`, `hhea` or `hmtx`,
    /// backlog F98). Shape with it via `TextStyle::font`.
    pub fn add_font_data(&mut self, data: Vec<u8>) -> Option<crate::resources::FontId> {
        use cosmic_text::fontdb::Source;
        let id = {
            let sess = &mut *self.session.state();
            let ids = sess
                .fonts
                .db_mut()
                .load_font_source(Source::Binary(std::sync::Arc::new(data)));
            sess.share_loaded_faces();
            let db = sess.fonts.db_mut();
            let ids = crate::text::keep_measurable(db, ids.to_vec());
            let family = db.face(*ids.first()?)?.families.first()?.0.clone();
            sess.fonts_rev += 1;
            let touched = families_of(db, &ids);
            let id = sess.resources.add_font(family, ids.to_vec());
            if sess.resources.reweigh(sess.fonts.db(), &touched, Some(id)) {
                sess.weights_rev += 1;
            }
            id
        };
        self.sync_font_names();
        Some(id)
    }

    /// Registers a font file (TTF/OTF/TTC) by path, memory-mapped by the
    /// font database; `None` when it cannot be read or holds no usable
    /// face (see [`add_font_data`](Self::add_font_data)). Shape with it
    /// via `TextStyle::font`.
    pub fn load_font_file(
        &mut self,
        path: impl Into<std::path::PathBuf>,
    ) -> Option<crate::resources::FontId> {
        use cosmic_text::fontdb::Source;
        let id = {
            let sess = &mut *self.session.state();
            let ids = sess
                .fonts
                .db_mut()
                .load_font_source(Source::File(path.into()));
            // Its own faces too, not only the ones before it (DX25).
            sess.share_loaded_faces();
            let db = sess.fonts.db_mut();
            let ids = crate::text::keep_measurable(db, ids.to_vec());
            let family = db.face(*ids.first()?)?.families.first()?.0.clone();
            sess.fonts_rev += 1;
            let touched = families_of(db, &ids);
            let id = sess.resources.add_font(family, ids.to_vec());
            if sess.resources.reweigh(sess.fonts.db(), &touched, Some(id)) {
                sess.weights_rev += 1;
            }
            id
        };
        self.sync_font_names();
        Some(id)
    }

    /// Loads every font file under `dir` (recursively) into the font
    /// database, so their families become available to `add_system_font`
    /// by name; returns how many faces were added. A face whose glyphs
    /// cannot be measured is left out and not counted (backlog F98). A
    /// bundled `fonts/` folder next to the app is the usual case.
    pub fn load_fonts_dir(&mut self, dir: impl AsRef<std::path::Path>) -> usize {
        let sess = &mut *self.session.state();
        let db = sess.fonts.db_mut();
        let before: rustc_hash::FxHashSet<_> = db.faces().map(|face| face.id).collect();
        db.load_fonts_dir(dir);
        sess.share_loaded_faces();
        let db = sess.fonts.db_mut();
        let added = db
            .faces()
            .map(|face| face.id)
            .filter(|id| !before.contains(id))
            .collect();
        let added = crate::text::keep_measurable(db, added);
        // A registered family may have gained a bold (backlog F100).
        let touched = families_of(db, &added);
        if sess.resources.reweigh(sess.fonts.db(), &touched, None) {
            sess.weights_rev += 1;
        }
        added.len()
    }

    /// The handle for a font family by name (`"Menlo"`, `"Antonio"`) —
    /// installed on the system or loaded with `load_fonts_dir` /
    /// `load_font_file`; `None` when no face matches (see
    /// `system_font_families`). Idempotent: the same family gets the same
    /// handle, so views can call it every frame.
    pub fn add_system_font(&mut self, name: &str) -> Option<crate::resources::FontId> {
        let id = self.session.register_family(name)?;
        self.sync_font_names();
        Some(id)
    }

    /// Forgets a registered font; faces loaded from bytes leave the font
    /// database. Styles still naming it shape as sans-serif.
    pub fn remove_font(&mut self, id: crate::resources::FontId) {
        {
            let sess = &mut *self.session.state();
            let Some(entry) = sess.resources.remove_font(id) else {
                return;
            };
            sess.fonts_rev += 1;
            let db = sess.fonts.db_mut();
            let touched = families_of(db, &entry.faces);
            for face in entry.faces {
                db.remove_face(face);
            }
            if sess.resources.reweigh(sess.fonts.db(), &touched, None) {
                sess.weights_rev += 1;
            }
        }
        self.sync_font_names();
    }

    /// The registered family name behind a font handle, if it is live.
    /// Read from this window's mirror of the session's fonts (see
    /// `session`'s module doc), which every registration refreshes and so
    /// does every frame — a font another window registered mid-frame shows
    /// up here on the next one.
    pub fn font_family(&self, id: crate::resources::FontId) -> Option<&str> {
        self.font_names.get(&id).map(|s| &**s)
    }

    /// Family names of every installed font the core can see (sorted,
    /// deduplicated) — what `add_system_font` accepts. The names of
    /// [`system_fonts`](Self::system_fonts), which says what each is.
    pub fn system_font_families(&self) -> Vec<String> {
        self.system_fonts().into_iter().map(|f| f.family).collect()
    }

    /// Every family the core can see, installed or loaded, one per family
    /// and sorted by name — the families `system_font_families` names —
    /// with what its faces say they are: monospaced, the weights, an
    /// italic (backlog F97). Read from what the font database recorded
    /// when it scanned each face, so a fonts pane showing the monospaced
    /// ones first costs no file loaded and no glyph shaped. A face whose
    /// glyphs cannot be measured never entered the database (backlog F98),
    /// so a family of only such faces — macOS's GB18030 Bitmap — is not
    /// listed.
    pub fn system_fonts(&self) -> Vec<crate::resources::SystemFont> {
        use crate::resources::SystemFont;
        use cosmic_text::fontdb::Style;
        let sess = self.session.state();
        let mut by_family = std::collections::BTreeMap::<&str, SystemFont>::new();
        for face in sess.fonts.db().faces() {
            let Some((name, _)) = face.families.first() else {
                continue;
            };
            let font = by_family.entry(name).or_insert_with(|| SystemFont {
                family: name.clone(),
                monospaced: true,
                weights: Vec::new(),
                italic: false,
            });
            font.monospaced &= face.monospaced;
            font.italic |= face.style != Style::Normal;
            font.weights.push(face.weight.0);
        }
        by_family
            .into_values()
            .map(|mut font| {
                font.weights.sort_unstable();
                font.weights.dedup();
                font
            })
            .collect()
    }

    /// The installed families `FontFamily::Sans`, `Serif` and `Mono` shape
    /// with, in that order — pinned per platform when the session's font
    /// database was built (backlog C32), so the devtools can say which face
    /// "mono" is on this machine.
    pub(crate) fn default_font_families(&self) -> [String; 3] {
        crate::text::default_families(&self.session.state().fonts).map(str::to_string)
    }

    // -- Audio ----------------------------------------------------------
    // Sounds are resources, playback is commands the driver drains; see
    // `audio`. Nothing here touches a device.

    /// Registers a sound from its encoded file bytes (wav/ogg/mp3/flac —
    /// the driver's backend decodes; the core only keeps the bytes).
    pub fn add_sound(&mut self, bytes: Vec<u8>) -> crate::resources::SoundId {
        self.session.state().resources.add_sound(bytes)
    }

    /// Forgets a sound; the driver drops its decoded copy. Playbacks
    /// already running keep going.
    pub fn remove_sound(&mut self, id: crate::resources::SoundId) {
        let sess = &mut *self.session.state();
        if sess.resources.remove_sound(id).is_some() {
            sess.audio.unload(id);
        }
    }

    /// Starts a playback; the returned id addresses it in `stop` /
    /// `set_volume` / `pause` / `resume`. With a tag in the options, the
    /// playback finishing on its own comes back as
    /// `{kind="sound", phase="ended", playback, tag}` on the current
    /// origin's root — the host's, or the extension's during its view.
    pub fn play(
        &mut self,
        sound: crate::resources::SoundId,
        opts: crate::audio::PlayOptions,
    ) -> crate::audio::PlaybackId {
        let origin = self.origin;
        let window = self.env.window.id;
        let sess = &mut *self.session.state();
        // The driver's backend resolves the handle when it plays; a
        // headless app has no driver, so a foreign handle is noticed here.
        let _ = sess.resources.sound(sound);
        sess.audio.play(origin, window, Key::ROOT, sound, opts)
    }

    /// Stops a playback, fading over `fade_ms` (0 = at once). A stopped
    /// playback never reports `ended`.
    pub fn stop(&mut self, playback: crate::audio::PlaybackId, fade_ms: f32) {
        self.session.state().audio.stop(playback, fade_ms);
    }

    /// Sets a playback's volume (linear amplitude), tweening over `tween_ms`.
    pub fn set_volume(&mut self, playback: crate::audio::PlaybackId, volume: f32, tween_ms: f32) {
        self.session
            .state()
            .audio
            .set_volume(playback, volume, tween_ms);
    }

    pub fn pause(&mut self, playback: crate::audio::PlaybackId, fade_ms: f32) {
        self.session.state().audio.pause(playback, fade_ms);
    }

    pub fn resume(&mut self, playback: crate::audio::PlaybackId, fade_ms: f32) {
        self.session.state().audio.resume(playback, fade_ms);
    }

    /// Sets the master volume (linear amplitude), tweening over `tween_ms`.
    pub fn set_master_volume(&mut self, volume: f32, tween_ms: f32) {
        self.session.state().audio.master_volume(volume, tween_ms);
    }

    /// An `audio` node: a playback retained by key for as long as the view
    /// keeps declaring it — present means playing (once, or looped),
    /// gone means stopped; `volume` / `paused` changes apply live, a
    /// changed `src` restarts. The key is auto-assigned from the tree
    /// position; see `audio_node_keyed` for a stable label. Draws nothing
    /// and takes no layout space. The mount is this window's: it is
    /// reconciled against this window's frames, and another window's
    /// frame declaring nothing leaves it playing.
    pub fn audio_node(&mut self, spec: crate::audio::AudioSpec) -> Key {
        if self.tree.is_empty() {
            return Key::ROOT;
        }
        let key = self.auto_key();
        self.declare_audio(key, spec);
        key
    }

    /// `audio_node` with a label-derived key (stable across reorders).
    pub fn audio_node_keyed(&mut self, label: &str, spec: crate::audio::AudioSpec) -> Key {
        if self.tree.is_empty() {
            return Key::ROOT;
        }
        let key = self.child_key(label);
        self.declare_audio(key, spec);
        key
    }

    fn declare_audio(&mut self, key: Key, spec: crate::audio::AudioSpec) {
        let origin = self.origin;
        let window = self.env.window.id;
        let sess = &mut *self.session.state();
        let _ = sess.resources.sound(spec.src);
        sess.audio.declare(window, key, origin, spec);
    }

    /// The playback an `audio` node of this window holds, if it is
    /// mounted — after the frame that declared it has finished.
    pub fn playback_of(&self, key: Key) -> Option<crate::audio::PlaybackId> {
        self.audio.playback_of(self.env.window.id, key)
    }

    /// Drains the audio commands queued since the last drain. Frame
    /// drivers apply them to a real device after each input dispatch and
    /// after each frame; headless drivers may simply never call.
    pub fn take_audio_commands(&mut self) -> Vec<crate::audio::AudioCommand> {
        self.audio.take_commands()
    }

    /// The driver reports a playback finished on its own (not stopped).
    /// A tagged playback becomes an `ended` event, pending like a `resize`
    /// (see `take_pending_events`).
    pub fn audio_ended(&mut self, playback: crate::audio::PlaybackId) {
        let ended = self.session.state().audio.ended(playback);
        if let Some(ev) = ended {
            self.pending.push(ev);
        }
    }

    /// The driver reports it stopped a playback that was still running,
    /// `at` seconds into the sound. When that stop was a one-shot `audio`
    /// node going away (or changing its `src`) without
    /// [`finish`](crate::audio::AudioSpec::finish), the node is named in a
    /// [`TRUNCATED_PLAYBACK`](crate::diag::TRUNCATED_PLAYBACK) warning;
    /// anything else — a stop that landed after the sound ended, an
    /// imperative [`Self::stop`], a loop, a released playback — reports
    /// nothing. The driver stays key-blind, as [`Self::audio_ended`] is.
    pub fn audio_truncated(&mut self, playback: crate::audio::PlaybackId, at: f64) {
        let cut = self.session.state().audio.truncated(playback);
        if let Some((key, why)) = cut {
            self.diag
                .raise(crate::diag::truncated_playback(key, why, at));
        }
    }

    /// The driver refused a play: the device's voices are all held, or
    /// the sound failed to decode. The playback never started, so it can
    /// never report `ended` — a tagged one gets
    /// `{kind="sound", phase="refused", playback, tag}` instead, pending
    /// like an `ended` is, so a view waiting on the sound is unstuck and
    /// can tell the two apart. [`crate::diag::PLAYBACK_REFUSED`] is raised
    /// on the node that asked either way, behind the usual diagnostics
    /// gate, so an untagged refusal is not silent.
    pub fn audio_refused(&mut self, playback: crate::audio::PlaybackId) {
        let (event, warning) = self.session.state().audio.refused(playback);
        if let Some(ev) = event {
            self.pending.push(ev);
        }
        self.warn(warning);
    }

    /// Drops what this window shaped when a registered family's weights
    /// changed since it last looked (RG59): text shaped with a bold
    /// synthesized for a family that has since gained its Bold, or at a
    /// face since removed, would otherwise stay as it was until evicted.
    /// Shaped text and cell tables shape again on their next draw; an
    /// editor keeps its text and takes the new weights. Rare — a face of
    /// a family already registered coming or going — so dropping every
    /// family's text is cheaper than knowing which.
    pub(crate) fn sync_weights(&mut self) {
        let sess = self.session.state();
        if sess.weights_rev == self.weights_rev {
            return;
        }
        self.weights_rev = sess.weights_rev;
        self.text.forget_shaped();
        self.cells.forget_shaped();
        self.edit.reweigh(&sess.resources);
    }

    /// Re-reads the session's font family names into the mirror
    /// `font_family` lends from, when a registration has moved since.
    pub(crate) fn sync_font_names(&mut self) {
        let sess = self.session.state();
        if sess.fonts_rev == self.fonts_rev {
            return;
        }
        self.fonts_rev = sess.fonts_rev;
        self.font_names.clear();
        for (id, entry) in sess.resources.fonts.iter() {
            self.font_names.insert(id, entry.family.as_str().into());
        }
    }

    /// Replaces an image's pixels in place; see `Resources::update_image`
    /// (ADR 0025, decision 1). If the image had been drawn from the atlas
    /// its slot is forgotten — one eviction, once — and from here on it is
    /// texture-backed. A foreign or removed handle warns and changes
    /// nothing, and so does a buffer that is not `width × height × 4`
    /// bytes; the return says whether the pixels were taken.
    pub fn update_image(
        &mut self,
        id: crate::resources::ImageId,
        width: u32,
        height: u32,
        rgba: Vec<u8>,
    ) -> bool {
        let taken = self
            .session
            .state()
            .resources
            .update_image(id, width, height, rgba);
        if taken {
            self.atlas.evict_image(id);
        }
        taken
    }

    /// Replaces an image's pixels by writing them into a buffer the core
    /// recycles; see `Resources::update_image_with`. `fill` gets
    /// `width × height × 4` bytes holding an earlier frame's pixels and
    /// writes every one. Where [`Self::update_image`] takes a buffer the
    /// app allocated — and frees the one it replaces — this one stops
    /// allocating after a stream's third update, which on Windows is most
    /// of what a 1080p update cost (backlog W20). Render into `fill`'s
    /// slice rather than into a buffer of your own to skip the copy too.
    /// `fill` runs while the session's resources are borrowed, so it must
    /// not reach them through another window's `Core`; that panics.
    pub fn update_image_with(
        &mut self,
        id: crate::resources::ImageId,
        width: u32,
        height: u32,
        fill: impl FnOnce(&mut [u8]),
    ) -> bool {
        let taken = self
            .session
            .state()
            .resources
            .update_image_with(id, width, height, fill);
        if taken {
            self.atlas.evict_image(id);
        }
        taken
    }

    /// The pixels behind an image handle — its size and a shared handle on
    /// the bytes — for a host that renders the display list itself and
    /// meets a `QuadKind::Texture` quad. `None` for a dead or foreign
    /// handle, which is also noted as a miss.
    pub fn image_pixels(
        &self,
        id: crate::resources::ImageId,
    ) -> Option<(u32, u32, std::sync::Arc<Vec<u8>>)> {
        let sess = self.session.state();
        sess.resources
            .image(id)
            .map(|e| (e.width, e.height, e.rgba.clone()))
    }

    /// Unregisters an image and forgets its atlas slot — every other
    /// window's atlas forgets its own at that window's next frame — or,
    /// for a texture-backed one, has the next display list any window
    /// builds tell the backend to drop the texture.
    pub fn remove_image(&mut self, id: crate::resources::ImageId) {
        self.session.state().remove_image(id);
        self.atlas.evict_image(id);
    }

    /// What the session removed and no display list has carried yet
    /// (AR8), onto this frame's — a backend frees a texture or a pipeline
    /// once, on whichever window draws next, since both are the device's
    /// and the device is shared. And this window's own atlas slots for
    /// images the registry no longer holds, when a removal has moved the
    /// revision since this core last looked: the atlas is per window, so
    /// the removing core's eviction reached only its own.
    pub(crate) fn sync_dropped(&mut self) {
        let mut sess = self.session.state();
        self.display
            .dropped_textures
            .append(&mut sess.dropped.images);
        self.display
            .dropped_fragments
            .append(&mut sess.dropped.fragments);
        if sess.images_rev != self.images_rev {
            self.images_rev = sess.images_rev;
            let live = &sess.resources.images;
            self.atlas.retain_images(|id| live.contains_key(id));
        }
    }
}

/// Every family name the faces `ids` answer to, for
/// [`Resources::reweigh`](crate::resources::Resources::reweigh).
fn families_of(
    db: &cosmic_text::fontdb::Database,
    ids: &[cosmic_text::fontdb::ID],
) -> rustc_hash::FxHashSet<String> {
    ids.iter()
        .filter_map(|&id| db.face(id))
        .flat_map(|face| face.families.iter().map(|(name, _)| name.clone()))
        .collect()
}

impl crate::session::Session {
    /// `Core::add_system_font`'s registration, on the session every window
    /// shares: a query against the font database's scan and an idempotent
    /// registry entry, with no file opened. Here rather than on `Core` so
    /// a binding's prop parser, which holds the token lookup's borrow of
    /// the core, can name a family while it parses (ADR 0037).
    pub(crate) fn register_family(&self, name: &str) -> Option<crate::resources::FontId> {
        use cosmic_text::fontdb::{Family, Query};
        let sess = &mut *self.state();
        sess.share_faces_once();
        let db = sess.fonts.db();
        let query = Query {
            families: &[Family::Name(name)],
            ..Default::default()
        };
        let id = db.query(&query)?;
        // The canonical spelling, so the style matches the way fontdb does.
        let family = db.face(id)?.families.first()?.0.clone();
        if let Some((id, _)) = sess
            .resources
            .fonts
            .iter()
            .find(|(_, f)| f.faces.is_empty() && f.family == family)
        {
            return Some(id);
        }
        sess.fonts_rev += 1;
        let touched = std::iter::once(family.clone()).collect();
        let id = sess.resources.add_font(family, Vec::new());
        if sess.resources.reweigh(sess.fonts.db(), &touched, Some(id)) {
            sess.weights_rev += 1;
        }
        Some(id)
    }
}

impl crate::session::SessionState {
    /// [`share_faces`], the first time an app names a family, unless a
    /// load has shared them already, and never again.
    pub(crate) fn share_faces_once(&mut self) {
        if !self.faces_shared {
            self.share_loaded_faces();
        }
    }

    /// [`share_faces`] after a load, so the faces it added are shared with
    /// the rest (backlog DX25). Sharing before the load, as DX24 first did,
    /// left every face a file brought in reading its file again each time
    /// a text shaped in a new family, weight or style: kawoosh loads 167
    /// files it ships, and each new family cost a frame ~5 ms in opens.
    /// The walk maps only what is unshared, so a load after the first
    /// maps its own faces and nothing else. Not for `register_family`,
    /// which runs every frame a view names a family: `db_mut` empties
    /// cosmic-text's match cache.
    pub(crate) fn share_loaded_faces(&mut self) {
        self.faces_shared = true;
        share_faces(self.fonts.db_mut());
    }
}

/// Maps every file-backed face in the database once and shares the
/// mapping, as cosmic-text does for each face it loads (backlog DX24).
///
/// The first time a text shapes in a family (a weight, a style) it has not
/// shaped in, cosmic-text ranks every face in the database against it, and
/// for each face of another weight it reads that face's `wght` axis — with
/// the face unshared, opening and mapping its file for that one read. On a
/// Mac's 1,311 faces that was 9.7 ms a new family, in release; kawoosh's
/// fonts pane, drawing each of 613 families in itself, warmed them ahead of
/// time to keep it off the frames. Shared, the read is a slice of a mapping
/// already made: 0.42 ms a family, for 30 ms once. Done on the first font
/// an app registers — a family by name, bytes, a file or a folder — where
/// the per-family cost starts to add up, and after every load from then on
/// so a loaded file's own faces are shared too (DX25); an app on the stock
/// three pays what it always did.
///
/// The mapping is what fontdb's `make_shared_face_data` documents as
/// unsafe: a font file another process rewrites while it is mapped can
/// show the change, and may crash the read. cosmic-text already takes that
/// risk for every face it shapes with; this takes it for every installed
/// face, which on a desktop are the system's and the user's fonts.
pub(crate) fn share_faces(db: &mut cosmic_text::fontdb::Database) -> usize {
    use cosmic_text::fontdb::Source;
    let ids: Vec<_> = db
        .faces()
        .filter(|f| matches!(f.source, Source::File(_)))
        .map(|f| f.id)
        .collect();
    let mut shared = 0;
    for id in ids {
        // A face whose file was shared through a sibling face is skipped by
        // fontdb itself: `make_shared_face_data` updates every face of the
        // file at once and answers the existing mapping after that.
        // SAFETY: see the function's doc — the mapping cosmic-text makes
        // for every face it loads, made for the rest.
        if unsafe { db.make_shared_face_data(id) }.is_some() {
            shared += 1;
        }
    }
    shared
}

#[cfg(test)]
mod tests {
    use super::*;

    /// DX24: naming a family shares the database's file-backed faces, once;
    /// before it, an app on the stock families has mapped nothing.
    #[test]
    fn the_first_named_family_shares_the_faces_once() {
        use cosmic_text::fontdb::Source;
        let mut core = Core::new();
        let file_backed = |core: &Core| {
            let sess = core.session.state();
            sess.fonts
                .db()
                .faces()
                .filter(|f| matches!(f.source, Source::File(_)))
                .count()
        };
        let before = file_backed(&core);
        assert!(!core.session.state().faces_shared);
        let name = core.system_fonts().into_iter().map(|f| f.family).next();
        let Some(name) = name else {
            return; // a machine with no installed fonts has nothing to share
        };
        core.add_system_font(&name);
        assert!(core.session.state().faces_shared);
        assert_eq!(
            file_backed(&core),
            0,
            "all {before} file-backed faces shared"
        );
        // A second name does not walk them again.
        assert_eq!(share_faces(core.session.state().fonts.db_mut()), 0);
    }

    /// DX25: a file loaded by path or in a folder is shared with the rest,
    /// its own faces too, whether it is the first font or a later one.
    /// Sharing before the load left them reading their file each time a
    /// text shaped in a new family.
    #[test]
    fn a_loaded_file_is_shared_too() {
        use cosmic_text::fontdb::Source;
        let file_backed = |core: &Core| {
            let sess = core.session.state();
            sess.fonts
                .db()
                .faces()
                .filter(|f| matches!(f.source, Source::File(_)))
                .count()
        };
        let dir = std::env::temp_dir().join(format!("kui-dx25-{}", std::process::id()));
        let (one, two) = (dir.join("one"), dir.join("two"));
        for d in [&one, &two] {
            std::fs::create_dir_all(d).unwrap();
            std::fs::write(d.join("face.ttf"), crate::testing::liga_font()).unwrap();
        }

        let mut core = Core::new();
        core.load_font_file(one.join("face.ttf"))
            .expect("the first font");
        assert_eq!(file_backed(&core), 0, "the first file's faces shared");
        core.load_font_file(two.join("face.ttf"))
            .expect("a later font");
        assert_eq!(file_backed(&core), 0, "a later file's faces shared");

        let mut fresh = Core::new();
        assert!(fresh.load_fonts_dir(&one) >= 1);
        assert!(fresh.session.state().faces_shared);
        assert_eq!(file_backed(&fresh), 0, "a folder's faces shared");
        std::fs::remove_dir_all(&dir).ok();
    }
}
