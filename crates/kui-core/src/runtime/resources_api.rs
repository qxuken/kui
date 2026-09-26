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
    /// data holds no usable face. Shape with it via `TextStyle::font`.
    pub fn add_font_data(&mut self, data: Vec<u8>) -> Option<crate::resources::FontId> {
        use cosmic_text::fontdb::Source;
        let id = {
            let sess = &mut *self.session.state();
            let db = sess.fonts.db_mut();
            let ids = db.load_font_source(Source::Binary(std::sync::Arc::new(data)));
            let family = db.face(*ids.first()?)?.families.first()?.0.clone();
            sess.fonts_rev += 1;
            sess.resources.add_font(family, ids.to_vec())
        };
        self.sync_font_names();
        Some(id)
    }

    /// Registers a font file (TTF/OTF/TTC) by path, memory-mapped by the
    /// font database; `None` when it cannot be read or holds no usable
    /// face. Shape with it via `TextStyle::font`.
    pub fn load_font_file(
        &mut self,
        path: impl Into<std::path::PathBuf>,
    ) -> Option<crate::resources::FontId> {
        use cosmic_text::fontdb::Source;
        let id = {
            let sess = &mut *self.session.state();
            let db = sess.fonts.db_mut();
            let ids = db.load_font_source(Source::File(path.into()));
            let family = db.face(*ids.first()?)?.families.first()?.0.clone();
            sess.fonts_rev += 1;
            sess.resources.add_font(family, ids.to_vec())
        };
        self.sync_font_names();
        Some(id)
    }

    /// Loads every font file under `dir` (recursively) into the font
    /// database, so their families become available to `add_system_font`
    /// by name; returns how many faces were added. A bundled `fonts/`
    /// folder next to the app is the usual case.
    pub fn load_fonts_dir(&mut self, dir: impl AsRef<std::path::Path>) -> usize {
        let sess = &mut *self.session.state();
        let db = sess.fonts.db_mut();
        let before = db.len();
        db.load_fonts_dir(dir);
        db.len().saturating_sub(before)
    }

    /// The handle for a font family by name (`"Menlo"`, `"Antonio"`) —
    /// installed on the system or loaded with `load_fonts_dir` /
    /// `load_font_file`; `None` when no face matches (see
    /// `system_font_families`). Idempotent: the same family gets the same
    /// handle, so views can call it every frame.
    pub fn add_system_font(&mut self, name: &str) -> Option<crate::resources::FontId> {
        use cosmic_text::fontdb::{Family, Query};
        let id = {
            let sess = &mut *self.session.state();
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
            sess.resources.add_font(family, Vec::new())
        };
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
            for face in entry.faces {
                db.remove_face(face);
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
    /// ones first costs no file loaded and no glyph shaped.
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
    /// allocating after a stream's second frame, which on Windows is most
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
