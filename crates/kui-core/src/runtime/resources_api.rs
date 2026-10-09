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
            sess.repin_mono();
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
            sess.repin_mono();
            id
        };
        self.sync_font_names();
        Some(id)
    }

    /// Loads every font file under `dir` (recursively) into the font
    /// database, so their families become available to `add_system_font`
    /// by name; returns how many faces were added. A face whose glyphs
    /// cannot be measured is left out and not counted. A
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
        sess.repin_mono();
        added.len()
    }

    /// Scans the system's fonts again and brings this session's font
    /// database up to it: a face installed since the last scan joins it,
    /// one uninstalled leaves it. Returns how many faces came and went, 0
    /// when nothing did.
    ///
    /// The system's fonts are scanned once a process, and every session
    /// starts from that scan, so a font the user installs while the app
    /// runs is not seen until something asks. The winit runner
    /// (`kui-native`) asks itself when macOS or Windows says the installed
    /// fonts changed; a host with its own windowing, or on Linux, where
    /// fontconfig says nothing, calls this when it has reason to think the
    /// set changed (a "fonts" pane opening, the window taking focus back).
    /// It opens every font file on the system — tens of milliseconds on a
    /// Mac's 1300 faces — so it is not a per-frame call. Sessions made after
    /// it start from the new scan; another
    /// session that already exists keeps what it has until it calls this
    /// too.
    ///
    /// Faces still installed keep their handles and their place in every
    /// cache; fonts the app loaded itself (`add_font_data`,
    /// `load_font_file`, `load_fonts_dir`) are not touched. Every window
    /// of the session shapes its text again on its next frame, since
    /// fallback can land on a new face anywhere; every one of them owes
    /// that frame (`animating`), and each window's next frame reports a `fonts` event to
    /// the host, for an app that keeps the font list in its model. A face
    /// whose file was replaced in place, under the same
    /// path, is not read again.
    pub fn reload_system_fonts(&mut self) -> usize {
        let fresh = crate::text::rescan_system_fonts();
        let changed = self.session.state().apply_system_fonts(fresh);
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
            sess.fonts = crate::text::font_system_over(locale, db, &sess.resources.fallback);
            if sess.faces_shared {
                share_faces(sess.fonts.db_mut());
            }
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
    /// the rest. Sharing before the load, as DX24 first did,
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
        fresh: std::sync::Arc<crate::text::SystemFonts>,
    ) -> usize {
        use cosmic_text::fontdb::{ID, Source};
        let old = std::mem::replace(&mut self.system, fresh.clone());
        let gone: rustc_hash::FxHashSet<_> = old.faces.difference(&fresh.faces).collect();
        let came: Vec<_> = fresh.faces.difference(&old.faces).collect();
        if gone.is_empty() && came.is_empty() {
            return 0;
        }
        let db = self.fonts.db_mut();
        let leaving: Vec<ID> = db
            .faces()
            .filter(|face| crate::text::face_file(face).is_some_and(|key| gone.contains(&key)))
            .map(|face| face.id)
            .collect();
        let mut touched = families_of(db, &leaving);
        for &id in &leaving {
            db.remove_face(id);
        }
        // What came, a file at a time: fontdb loads every face of a file,
        // and only the ones the scan kept (measurable, and not already
        // here through a sibling that stayed) are wanted.
        let mut by_file = rustc_hash::FxHashMap::<&std::path::Path, Vec<u32>>::default();
        for (path, index) in &came {
            by_file.entry(path.as_path()).or_default().push(*index);
        }
        let mut arrived = Vec::new();
        for (path, indices) in by_file {
            for id in db.load_font_source(Source::File(path.to_path_buf())) {
                let wanted = db
                    .face(id)
                    .is_some_and(|face| indices.contains(&face.index));
                if wanted {
                    arrived.push(id);
                } else {
                    db.remove_face(id);
                }
            }
        }
        touched.extend(families_of(db, &arrived));
        crate::text::pin_default_families(db);
        // cosmic-text works out its monospaced faces, and which scripts
        // each covers, when the font system is built; rebuilt over the
        // same database, the ids stay and the lists are new.
        let fonts = std::mem::replace(
            &mut self.fonts,
            cosmic_text::FontSystem::new_with_locale_and_db(String::new(), Default::default()),
        );
        let (locale, db) = fonts.into_locale_and_db();
        self.fonts = crate::text::font_system_over(locale, db, &self.resources.fallback);
        if self.faces_shared {
            share_faces(self.fonts.db_mut());
        }
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

/// Maps every file-backed face in the database once and shares the
/// mapping, as cosmic-text does for each face it loads.
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
/// so a loaded file's own faces are shared too; an app on the stock
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
        use crate::text::{SystemFonts, face_file};
        let mut core = Core::new();
        let held = core.session.state().system.clone();
        let dir = std::env::temp_dir().join(format!("kui-rescan-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let installed = dir.join("rescan.ttf");
        std::fs::write(
            &installed,
            crate::testing::font_face("Kui Rescan Face", 400, false, false),
        )
        .unwrap();

        // A file's face is uninstalled whole: one index of a file can be
        // several faces (Ubuntu's variable `Ubuntu[wdth,wght].ttf` is two
        // at index 0), and a scan keys on the file, so removing one id
        // would leave the file installed. The face that stays is another
        // file's.
        let mut db = held.db().clone();
        let uninstalled = db.faces().find_map(face_file);
        let leaving: Vec<_> = db
            .faces()
            .filter(|f| uninstalled.is_some() && face_file(f) == uninstalled)
            .map(|f| f.id)
            .collect();
        let stays = db
            .faces()
            .filter_map(|f| Some((f.id, face_file(f)?)))
            .find(|(_, key)| Some(key) != uninstalled.as_ref());
        for &id in &leaving {
            db.remove_face(id);
        }
        db.load_font_file(&installed).unwrap();
        let fresh = std::sync::Arc::new(SystemFonts::from_db(held.locale().into(), db));

        let framed = |core: &mut Core| {
            core.frame(crate::Size::new(100.0, 100.0), 1.0).finish();
            let evs = core.take_pending_events();
            evs.iter()
                .filter_map(|e| e.kind().map(str::to_owned))
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
        assert_eq!(framed(&mut core), ["fonts"], "one event, on the root");
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
        let _ = std::fs::remove_dir_all(&dir);
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
                .any(|e| e.kind() == Some("fonts"))
        );
        let after = Core::new();
        assert!(std::sync::Arc::ptr_eq(
            &after.session.state().system,
            &core.session.state().system
        ));
    }

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
