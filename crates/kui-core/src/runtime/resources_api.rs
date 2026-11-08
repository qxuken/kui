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

    /// Registers a WGSL fragment function for a `fragment` node.
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
    /// none whose glyphs can be measured (no `head`, `hhea` or `hmtx`).
    /// Shape with it via `TextStyle::font`.
    pub fn add_font_data(&mut self, data: Vec<u8>) -> Option<crate::resources::FontId> {
        use cosmic_text::fontdb::Source;
        self.register_font(|db| {
            db.load_font_source(Source::Binary(std::sync::Arc::new(data)))
                .to_vec()
        })
    }

    /// Registers a font by path through the session's host font loader.
    pub fn load_font_file(
        &mut self,
        path: impl Into<std::path::PathBuf>,
    ) -> Option<crate::resources::FontId> {
        let loader = self.session.state().font_loader.clone()?;
        self.add_prepared_font(loader.file(&path.into()))
    }

    /// Registers a host-prepared font. Every face must already carry its bytes.
    pub fn add_prepared_font(
        &mut self,
        faces: Vec<cosmic_text::fontdb::FaceInfo>,
    ) -> Option<crate::resources::FontId> {
        self.register_font(|db| {
            faces
                .into_iter()
                .map(|face| db.push_face_info(face))
                .collect()
        })
    }

    fn register_font(
        &mut self,
        load: impl FnOnce(&mut cosmic_text::fontdb::Database) -> Vec<cosmic_text::fontdb::ID>,
    ) -> Option<crate::resources::FontId> {
        let id = {
            let sess = &mut *self.session.state();
            let db = sess.fonts.db_mut();
            let added = load(db);
            let ids = crate::text::keep_measurable(db, added);
            let family = db.face(*ids.first()?)?.families.first()?.0.clone();
            sess.fonts_rev += 1;
            let touched = families_of(db, &ids);
            let id = sess.resources.add_font(family, ids);
            if sess.resources.reweigh(sess.fonts.db(), &touched, Some(id)) {
                sess.weights_rev += 1;
            }
            sess.repin_mono();
            id
        };
        self.sync_font_names();
        Some(id)
    }

    /// Registers the usable faces under a directory through the host font loader.
    pub fn load_fonts_dir(&mut self, dir: impl AsRef<std::path::Path>) -> usize {
        let Some(loader) = self.session.state().font_loader.clone() else {
            return 0;
        };
        let faces = loader.directory(dir.as_ref());
        let sess = &mut *self.session.state();
        let db = sess.fonts.db_mut();
        let added = faces
            .into_iter()
            .map(|face| db.push_face_info(face))
            .collect();
        let added = crate::text::keep_measurable(db, added);
        let touched = families_of(db, &added);
        if sess.resources.reweigh(sess.fonts.db(), &touched, None) {
            sess.weights_rev += 1;
        }
        sess.repin_mono();
        added.len()
    }

    /// Asks the host font loader for a fresh catalog and applies its changes.
    pub fn reload_system_fonts(&mut self) -> usize {
        let Some(loader) = self.session.state().font_loader.clone() else {
            return 0;
        };
        self.apply_font_catalog(loader.reload())
    }

    /// Applies a file-keyed system catalog. Surviving faces keep their IDs
    /// and app fonts are kept. Returns the number of faces added or removed.
    /// For new bundled or fetched fonts use [`Self::add_font_data`].
    pub fn apply_font_catalog(
        &mut self,
        catalog: std::sync::Arc<crate::text::FontCatalog>,
    ) -> usize {
        let changed = self.session.state().apply_system_fonts(catalog);
        if changed > 0 {
            self.sync_font_names();
            self.request_frame();
        }
        changed
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

    /// The families asked, in order, for a character the text's own
    /// family has no glyph for — before the platform's fallback list,
    /// whose first choice is the system's interface face: proportional on
    /// macOS, so a Cyrillic word in a Latin-only monospaced family was
    /// set in San Francisco. An editor names its icon face, the face it
    /// ships and a monospaced one the machine has; a family that has not
    /// got the character is passed over, and after the last the
    /// platform's list runs as before. For every family and every kind of
    /// text in the session — plain, spans, an editor, a cell grid — and
    /// kept across `reload_system_fonts`. `FontFamily::Mono` asks them
    /// straight after its own face, ahead of the machine's other
    /// monospaced faces, which with no list it walks first (backlog
    /// RG118). A handle that names no font is left out; an empty list is
    /// the platform's alone.
    ///
    /// Not a per-frame call for a list that changes: a new list builds
    /// the font system again over the same faces and every window of the
    /// session shapes its text again: each owes a frame (`animating`), so
    /// a driver draws the windows the call was not made through as well
    /// (backlog RG118). The same list twice is nothing.
    pub fn set_fallback_fonts(&mut self, fonts: &[crate::resources::FontId]) {
        {
            let sess = &mut *self.session.state();
            let names: Vec<String> = fonts
                .iter()
                .filter_map(|&id| sess.resources.font_family(id).map(str::to_string))
                .collect();
            if names == sess.resources.fallback {
                return;
            }
            sess.resources.fallback = names;
            let old = std::mem::replace(
                &mut sess.fonts,
                cosmic_text::FontSystem::new_with_locale_and_db(String::new(), Default::default()),
            );
            let (locale, db) = old.into_locale_and_db();
            sess.fonts = crate::text::font_system_over(
                locale,
                db,
                &sess.resources.fallback,
                sess.system.fallback.clone(),
            );
            sess.name_mono();
            sess.weights_rev += 1;
        }
        self.request_frame();
    }

    /// The families `set_fallback_fonts` named, in order.
    pub fn fallback_fonts(&self) -> Vec<String> {
        self.session.state().resources.fallback.clone()
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
            sess.repin_mono();
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
    /// italic. Read from what the font database recorded
    /// when it scanned each face, so a fonts pane showing the monospaced
    /// ones first costs no file loaded and no glyph shaped. A face whose
    /// glyphs cannot be measured never entered the database,
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
    /// database was built, so the devtools can say which face
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
    /// playback finishing on its own comes back as that message with
    /// `sound: {phase="ended", playback}` in it, on the current
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
        self.taint_kept("it declared audio");
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
    /// never report `ended` — a tagged one gets its message with
    /// `sound: {phase="refused", playback}` in it instead, pending
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
    /// changed since it last looked: text shaped with a bold
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

    /// Replaces an image's pixels in place; see `Resources::update_image`.
    /// If the image had been drawn from the atlas
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
    /// of what a 1080p update cost. Render into `fill`'s
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
        // A path's own texture is this core's, not the registry's: the
        // handle a `Texture` quad names is answered either way.
        if let Some(px) = self.path_textures.pixels(id) {
            return Some(px);
        }
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

    /// What the session removed and no display list has carried yet,
    /// onto this frame's — a backend frees a texture or a pipeline
    /// once, on whichever window draws next, since both are the device's
    /// and the device is shared. And this window's own atlas slots for
    /// images the registry no longer holds, when a removal has moved the
    /// revision since this core last looked: the atlas is per window, so
    /// the removing core's eviction reached only its own.
    /// How many `path` masks this core draws from textures of their own
    /// rather than the atlas — too big for a page, or animating (ADR
    /// 0040, decisions 7 and 8). For a test of which road a path took.
    pub fn path_texture_count(&self) -> usize {
        self.path_textures.len()
    }

    pub(crate) fn sync_dropped(&mut self) {
        // A path's own texture the last frame did not draw goes with the
        // removed images (ADR 0040, decisions 7 and 8).
        self.display
            .dropped_textures
            .extend(self.path_textures.sweep(self.frame_no));
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
    /// the core, can name a family while it parses.
    pub(crate) fn register_family(&self, name: &str) -> Option<crate::resources::FontId> {
        use cosmic_text::fontdb::{Family, Query};
        let sess = &mut *self.state();
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
    /// Names the family `Mono` shapes as while the app names fallbacks
    /// (`Resources::mono`, backlog RG118): the face the generic monospace
    /// family is pinned to, by name, so the app's list is asked straight
    /// after it rather than after every monospaced face. None with no
    /// list, or where no installed face is of the pinned name (cosmic-text's
    /// own default on a machine without it), where the generic family's
    /// walk of the monospaced faces is the better first answer. With the
    /// name, the weights the family's faces cover, so a single-weight face
    /// is not passed over for bold (backlog RG123).
    ///
    /// Asked again wherever the faces can change under it — the list, a
    /// rescan, and a font the app loads or removes, which can be the
    /// pinned family's only face or a new weight of it (RG123) — and true
    /// when the answer moved, for the caller to have every window shape
    /// again.
    pub(crate) fn name_mono(&mut self) -> bool {
        let db = self.fonts.db();
        let name = db.family_name(&cosmic_text::Family::Monospace);
        let installed = db
            .faces()
            .any(|face| face.families.iter().any(|(f, _)| f == name));
        let mono = (!self.resources.fallback.is_empty() && installed)
            .then(|| (name.to_string(), crate::weights::Weights::of(db, name)));
        if mono == self.resources.mono {
            return false;
        }
        self.resources.mono = mono;
        true
    }

    /// [`Self::name_mono`] after a font the app loaded or removed, with
    /// every window shaping again when `Mono` moved.
    fn repin_mono(&mut self) {
        if self.name_mono() {
            self.weights_rev += 1;
        }
    }

    /// `Core::reload_system_fonts`' half on the session: the faces the
    /// scan this session holds had and `fresh` has not leave the database,
    /// those `fresh` has and it had not are loaded, and the rest stay
    /// where they are, handles and all — a new database would hand out
    /// fresh ids, and every window's atlas, raster axes and shaped text
    /// key glyphs by id. Returns how many faces came and went.
    pub(crate) fn apply_system_fonts(
        &mut self,
        fresh: std::sync::Arc<crate::text::FontCatalog>,
    ) -> usize {
        let old = std::mem::replace(&mut self.system, fresh.clone());
        if old.same_faces(&fresh) {
            return 0;
        }
        let db = self.fonts.db_mut();
        let leaving: Vec<_> = db
            .faces()
            .filter(|face| self.system_face_ids.contains(&face.id) && !fresh.contains_face(face))
            .map(|face| face.id)
            .collect();
        let mut touched = families_of(db, &leaving);
        for &id in &leaving {
            db.remove_face(id);
            self.system_face_ids.remove(&id);
        }
        let arrived: Vec<_> = fresh
            .db
            .faces()
            .filter(|face| fresh.contains_face(face) && !old.contains_face(face))
            .map(|face| db.push_face_info(face.clone()))
            .collect();
        self.system_face_ids.extend(arrived.iter().copied());
        touched.extend(families_of(db, &arrived));
        if fresh.last_resort
            && !db.faces().any(|face| {
                face.families
                    .iter()
                    .any(|(name, _)| name == crate::text::LAST_RESORT_FAMILY)
            })
        {
            crate::text::insert_last_resort(db);
        }
        crate::text::pin_default_families(db, fresh.preferences);
        // cosmic-text works out its monospaced faces, and which scripts
        // each covers, when the font system is built; rebuilt over the
        // same database, the ids stay and the lists are new.
        let fonts = std::mem::replace(
            &mut self.fonts,
            cosmic_text::FontSystem::new_with_locale_and_db(String::new(), Default::default()),
        );
        let (locale, db) = fonts.into_locale_and_db();
        self.fonts = crate::text::font_system_over(
            locale,
            db,
            &self.resources.fallback,
            fresh.fallback.clone(),
        );
        // The scan can pin `Mono` to another face.
        self.name_mono();
        self.resources.reweigh(self.fonts.db(), &touched, None);
        // Every window shapes again, whatever reweigh found: a family no
        // one registered can still be what fallback picks.
        self.fonts_rev += 1;
        self.weights_rev += 1;
        self.system_fonts_rev += 1;
        leaving.len() + arrived.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The atlas's kept page lives for the frame it was kept for and no
    /// longer: `finish` drops it, where the next `begin_frame` did — on a
    /// window that goes idle after emptying a 4096 page, never.
    #[test]
    fn the_emptied_atlas_page_is_dropped_when_its_frame_finishes() {
        let frame = |core: &mut Core| {
            let mut ui = core.frame(crate::Size::new(200.0, 100.0), 1.0);
            ui.text("kept for a frame", crate::TextStyle::new(14.0));
            ui.finish();
        };
        let mut core = Core::new();
        frame(&mut core);
        core.atlas.reset_next_frame();
        core.begin_frame(crate::Size::new(200.0, 100.0), 1.0);
        assert!(
            core.atlas.keeps_prev(),
            "the frame that emptied it keeps it"
        );
        core.finish_frame();
        assert!(!core.atlas.keeps_prev());
    }

    /// `reload_system_fonts`' session half, against a scan made up for
    /// the test (installing a font on the machine running it is not the
    /// test's to do): the session's own scan less one installed face, plus
    /// a font file written for it. The new face is found by name, the
    /// gone one is gone, one that stayed keeps its id, every window is
    /// told to shape again and hears one `fonts` event, and the same scan
    /// a second time changes nothing.
    #[test]
    fn a_rescan_brings_in_what_came_drops_what_went_and_keeps_the_rest() {
        use crate::text::{FontCatalog, shared_face};
        use cosmic_text::fontdb::{Database, Source};
        use std::sync::Arc;

        let face = |db: &mut Database, path: &str, family: &str| {
            let bytes = Arc::new(crate::testing::font_face(family, 400, false, false));
            db.load_font_source(Source::SharedFile(path.into(), bytes))[0]
        };
        let mut db = Database::new();
        let leaving = [face(&mut db, "gone", "Kui Gone")];
        let staying = face(&mut db, "stays", "Kui Stays");
        let face_file =
            |face: &cosmic_text::fontdb::FaceInfo| shared_face(face).map(|(key, _)| key);
        let uninstalled = db.face(leaving[0]).and_then(face_file);
        let stays = db
            .face(staying)
            .and_then(face_file)
            .map(|key| (staying, key));
        let catalog = Arc::new(FontCatalog::from_db("en-US".into(), db.clone()));
        let mut core = Core::new_in(&crate::Session::with_font_catalog(catalog));
        db.remove_face(leaving[0]);
        face(&mut db, "installed", "Kui Rescan Face");
        let fresh = Arc::new(FontCatalog::from_db("en-US".into(), db));

        let framed = |core: &mut Core| {
            let mut ui = core.frame(crate::Size::new(100.0, 100.0), 1.0);
            ui.window_events(crate::WindowEvents::default().on_fonts("fonts"));
            ui.finish();
            let evs = core.take_pending_events();
            evs.iter()
                .map(|e| e.event.name().to_owned())
                .collect::<Vec<_>>()
        };
        let mut other = Core::new_in(&core.session);
        assert!(
            framed(&mut core).is_empty(),
            "the first frame establishes the set"
        );
        assert!(framed(&mut other).is_empty());
        let face_of = |core: &Core, key: &(std::path::PathBuf, u32)| {
            let sess = core.session.state();
            sess.fonts
                .db()
                .faces()
                .find(|f| face_file(f).as_ref() == Some(key))
                .map(|f| f.id)
        };
        let weights = core.session.state().weights_rev;
        let changed = core.session.state().apply_system_fonts(fresh.clone());
        assert_eq!(changed, 1 + leaving.len());
        assert!(
            core.session.state().weights_rev > weights,
            "every window shapes again"
        );
        assert!(
            core.add_system_font("Kui Rescan Face").is_some(),
            "the installed face is found"
        );
        if let Some(key) = &uninstalled {
            assert_eq!(face_of(&core, key), None, "the uninstalled face is gone");
        }
        if let Some((id, key)) = &stays {
            assert_eq!(
                face_of(&core, key),
                Some(*id),
                "a face that stayed keeps its id"
            );
        }
        assert_eq!(
            framed(&mut core),
            ["fonts"],
            "one event, as the root declared"
        );
        assert_eq!(
            framed(&mut other),
            ["fonts"],
            "and one in every window of the session"
        );
        assert!(framed(&mut core).is_empty(), "once");
        assert_eq!(core.session.state().apply_system_fonts(fresh), 0);
        assert!(
            framed(&mut core).is_empty(),
            "a scan that found nothing new is no event"
        );
    }

    #[test]
    fn rescan_replaces_changed_bytes_at_the_same_path() {
        use cosmic_text::fontdb::{Database, Source};
        use std::sync::Arc;

        let catalog = |family| {
            let mut db = Database::new();
            let bytes = Arc::new(crate::testing::font_face(family, 400, false, false));
            db.load_font_source(Source::SharedFile("same.ttf".into(), bytes));
            Arc::new(crate::text::FontCatalog::from_db("en-US".into(), db))
        };
        let mut core = Core::new_in(&crate::Session::with_font_catalog(catalog("Old Font")));
        let fresh = catalog("New Font");
        assert_eq!(core.apply_font_catalog(fresh.clone()), 2);
        assert!(core.add_system_font("Old Font").is_none());
        assert!(core.add_system_font("New Font").is_some());
        assert_eq!(core.apply_font_catalog(fresh), 0);
    }

    /// The real rescan: with nothing installed or removed since the
    /// session's scan, nothing changes and no frame is asked for — and a
    /// session made after it starts from it.
    #[test]
    fn a_rescan_of_an_unchanged_system_changes_nothing() {
        let mut core = Core::new();
        core.frame(crate::Size::new(100.0, 100.0), 1.0).finish();
        core.take_pending_events();
        assert_eq!(core.reload_system_fonts(), 0);
        // Nor does a font the app loads itself raise a `fonts` event.
        core.add_font_data(crate::testing::font_face(
            "Kui Rescan App",
            400,
            false,
            false,
        ))
        .expect("the app's own font loads");
        core.frame(crate::Size::new(100.0, 100.0), 1.0).finish();
        assert!(
            !core
                .take_pending_events()
                .iter()
                .any(|e| e.event.name() == "fonts")
        );
        let after = Core::new();
        assert!(std::sync::Arc::ptr_eq(
            &after.session.state().system,
            &core.session.state().system
        ));
    }

    /// A session whose generic monospace family is pinned to `name`, as
    /// the scan pins it to the machine's (`pin_default_families`); a test
    /// cannot install a face, so the face is one the app loads.
    fn pinned_to(name: &str) -> Core {
        let core = Core::new();
        core.session
            .state()
            .fonts
            .db_mut()
            .set_monospace_family(name);
        core
    }

    /// How far apart two `M`s are drawn in `Mono` at 20 px, bold or not:
    /// 10 px in the fixture's face, which no installed face's is.
    fn mono_m_apart(core: &mut Core, bold: bool) -> Option<f32> {
        let span = crate::text::Span {
            bold,
            ..crate::text::Span::new("MM")
        };
        let mut ui = core.frame(crate::Size::new(400.0, 100.0), 1.0);
        ui.rich_text(&[span], crate::TextStyle::new(20.0).mono());
        ui.finish();
        let (dl, _) = core.output();
        let xs: Vec<f32> = dl
            .quads
            .iter()
            .filter(|q| q.kind != crate::QuadKind::Solid)
            .map(|q| q.rect.x)
            .collect();
        xs.get(1).zip(xs.first()).map(|(b, a)| b - a)
    }

    /// A pinned `Mono` of a single weight stays in its family for bold
    /// (backlog RG123). By name cosmic-text takes a face of the family only
    /// at a weight it has, and asked at 700 a regular-only family was
    /// passed over for the proportional lists: the pinned name is asked at
    /// the weights its faces cover, as a registered family is.
    #[test]
    fn a_single_weight_pinned_mono_stays_itself_in_bold() {
        use crate::testing::{font_face, han_only_face};
        let mut core = pinned_to("Kui RG123 Mono");
        core.add_font_data(font_face("Kui RG123 Mono", 400, false, true))
            .expect("the fixture registers");
        // A list that has no `M`, so it cannot stand in for the pinned face.
        let han = core
            .add_font_data(han_only_face("Kui RG123 Han"))
            .expect("a face with 字");
        core.set_fallback_fonts(&[han]);
        assert_eq!(
            core.session
                .state()
                .resources
                .mono
                .as_ref()
                .map(|m| m.0.as_str()),
            Some("Kui RG123 Mono"),
            "Mono is pinned by name"
        );
        assert_eq!(mono_m_apart(&mut core, false), Some(10.0), "regular");
        assert_eq!(mono_m_apart(&mut core, true), Some(10.0), "and bold");
    }

    /// The pin follows the faces the app loads and removes (backlog
    /// RG123): a pinned family whose only face the app removes leaves
    /// `Mono` generic, not a name nothing answers to, and one that arrives
    /// after the list was set pins it. Each move has every window shape
    /// again.
    #[test]
    fn the_mono_pin_follows_the_faces_the_app_loads_and_removes() {
        use crate::testing::{font_face, han_only_face};
        let mut core = pinned_to("Kui RG123 Late Mono");
        let han = core
            .add_font_data(han_only_face("Kui RG123 Late Han"))
            .expect("a face with 字");
        core.set_fallback_fonts(&[han]);
        let pinned = |core: &Core| {
            core.session
                .state()
                .resources
                .mono
                .as_ref()
                .map(|m| m.0.clone())
        };
        assert_eq!(pinned(&core), None, "no face of the name yet");
        let weights = core.session.state().weights_rev;
        let mono = core
            .add_font_data(font_face("Kui RG123 Late Mono", 400, false, true))
            .expect("the fixture registers");
        assert_eq!(
            pinned(&core).as_deref(),
            Some("Kui RG123 Late Mono"),
            "the face loaded after the list pins it"
        );
        assert!(
            core.session.state().weights_rev > weights,
            "and shapes again"
        );
        assert_eq!(mono_m_apart(&mut core, false), Some(10.0));
        let weights = core.session.state().weights_rev;
        core.remove_font(mono);
        assert_eq!(pinned(&core), None, "its only face gone, Mono is generic");
        assert!(
            core.session.state().weights_rev > weights,
            "and shapes again"
        );
        assert_eq!(
            core.session
                .state()
                .resources
                .family_of(crate::FontFamily::Mono),
            cosmic_text::Family::Monospace
        );
    }
}
