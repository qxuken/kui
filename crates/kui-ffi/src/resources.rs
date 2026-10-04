//! Resources: images, fonts and sounds registered through a context, and
//! the playback commands a host with its own loop drains.

use super::*;

/// The byte length of a `w × h` RGBA buffer at `rgba`, or `None` for a
/// null pointer, an empty size, or one whose length overflows (or passes
/// `isize::MAX`, which no slice may) — checked before the slice is made,
/// since a wrapped length is undefined behaviour even unread.
fn rgba_len(rgba: *const u8, w: u32, h: u32) -> Option<usize> {
    if rgba.is_null() || w == 0 || h == 0 {
        return None;
    }
    (w as usize)
        .checked_mul(h as usize)?
        .checked_mul(4)
        .filter(|&n| n <= isize::MAX as usize)
}

/// Registers a w×h RGBA image (pixels copied); returns its handle, 0 on
/// failure. Draw it with `kui_image`; free it with `kui_image_remove`.
#[unsafe(no_mangle)]
pub extern "C" fn kui_image_add(ptr: *mut KuiCtx, w: u32, h: u32, rgba: *const u8) -> u64 {
    guard(0, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return 0;
        };
        let Some(len) = rgba_len(rgba, w, h) else {
            return 0;
        };
        let data = unsafe { std::slice::from_raw_parts(rgba, len) }.to_vec();
        c.core().resources.add_image(w, h, data).to_ffi()
    })
}

/// Replaces an image's pixels in place (copied): the handle is unchanged,
/// so every node showing it draws the new pixels next frame; `w`/`h` may
/// differ from the registration. From the first update on the image is
/// drawn from a texture of its own, as a `KUI_QUAD_TEXTURE` quad. A dead
/// or foreign handle warns `foreign-resource` and changes nothing.
#[unsafe(no_mangle)]
pub extern "C" fn kui_image_update(ptr: *mut KuiCtx, id: u64, w: u32, h: u32, rgba: *const u8) {
    guard((), || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return;
        };
        let Some(len) = rgba_len(rgba, w, h) else {
            return;
        };
        let data = unsafe { std::slice::from_raw_parts(rgba, len) };
        // Into a buffer the core recycles, not a fresh copy a frame.
        c.core()
            .update_image_with(kui_core::ImageId::from_ffi(id), w, h, |px| {
                px.copy_from_slice(data)
            });
    });
}

/// The pixels behind an image handle, for a host that renders the draw
/// list itself and meets a `KUI_QUAD_TEXTURE` quad: `w`, `h` and `rgba`
/// (w×h×4 bytes) are written and true returned when the handle is live
/// here. The bytes are borrowed and valid until the next call of this
/// function or `kui_image_update` on the same handle.
#[unsafe(no_mangle)]
pub extern "C" fn kui_image_pixels(
    ptr: *mut KuiCtx,
    id: u64,
    w: *mut u32,
    h: *mut u32,
    rgba: *mut *const u8,
) -> bool {
    guard(false, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return false;
        };
        if w.is_null() || h.is_null() || rgba.is_null() {
            return false;
        }
        let Some((iw, ih, px)) = c.core().image_pixels(kui_core::ImageId::from_ffi(id)) else {
            return false;
        };
        // Kept on the context so the pointer outlives this call — the
        // `Arc` holds the bytes even if an update replaces the entry's.
        c.image_pixels = Some(px);
        unsafe {
            *w = iw;
            *h = ih;
            *rgba = c.image_pixels.as_ref().unwrap().as_ptr();
        }
        true
    })
}

/// Registers a font from file bytes (TTF/OTF/TTC, copied); returns its
/// handle for `KuiTextStyle.font`, 0 when the data holds no usable face.
#[unsafe(no_mangle)]
pub extern "C" fn kui_font_add(ptr: *mut KuiCtx, data: *const u8, len: usize) -> u64 {
    guard(0, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return 0;
        };
        if data.is_null() || len == 0 {
            return 0;
        }
        let bytes = unsafe { std::slice::from_raw_parts(data, len) }.to_vec();
        c.core().add_font_data(bytes).map_or(0, |id| id.to_ffi())
    })
}

/// Registers an installed font by family name; 0 when none matches.
#[unsafe(no_mangle)]
pub extern "C" fn kui_font_add_system(ptr: *mut KuiCtx, name: KuiStr) -> u64 {
    guard(0, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return 0;
        };
        c.core()
            .add_system_font(&kstr(name))
            .map_or(0, |id| id.to_ffi())
    })
}

/// The family names `kui_font_add_system` can take — every face the
/// context knows, installed or loaded, sorted and deduplicated — written
/// into `out` up to `cap` and the total returned, so a short array can be
/// resized and the call repeated. Strings are borrowed until the next
/// call on this context. What Node's `systemFontFamilies()` answers.
#[unsafe(no_mangle)]
pub extern "C" fn kui_font_families(ptr: *mut KuiCtx, out: *mut KuiStr, cap: usize) -> usize {
    guard(0, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return 0;
        };
        c.font_families = c.core().system_font_families();
        let n = c.font_families.len();
        if out.is_null() {
            return n;
        }
        for (i, name) in c.font_families.iter().take(cap).enumerate() {
            unsafe {
                out.add(i).write(KuiStr {
                    ptr: name.as_ptr(),
                    len: name.len(),
                })
            };
        }
        n
    })
}

/// Every family [`kui_font_families`] names, one per family and in its
/// order, with what its faces say they are (monospaced, the weights, an
/// italic), written into `out` up to `cap` and the total
/// returned, as `kui_font_families` does. Read from what the font database
/// recorded when it scanned each face: nothing is loaded or shaped. The
/// names and weight arrays are borrowed until the next call on this
/// context. What Node's `systemFonts()` answers.
#[unsafe(no_mangle)]
pub extern "C" fn kui_system_fonts(ptr: *mut KuiCtx, out: *mut KuiSystemFont, cap: usize) -> usize {
    guard(0, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return 0;
        };
        c.system_fonts = c.core().system_fonts();
        let n = c.system_fonts.len();
        if out.is_null() {
            return n;
        }
        for (i, font) in c.system_fonts.iter().take(cap).enumerate() {
            unsafe {
                out.add(i).write(KuiSystemFont {
                    family: KuiStr {
                        ptr: font.family.as_ptr(),
                        len: font.family.len(),
                    },
                    weights: font.weights.as_ptr(),
                    weight_count: font.weights.len() as u32,
                    monospaced: u32::from(font.monospaced),
                    italic: u32::from(font.italic),
                })
            };
        }
        n
    })
}

/// Registers a WGSL fragment function; 0 when it does not compile, with a
/// `fragment-rejected` warning carrying the message. Idempotent by source.
#[unsafe(no_mangle)]
pub extern "C" fn kui_fragment_add(ptr: *mut KuiCtx, wgsl: KuiStr) -> u64 {
    guard(0, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return 0;
        };
        c.core()
            .add_fragment(&kstr(wgsl))
            .map_or(0, |id| id.to_ffi())
    })
}

/// Forgets a registered fragment; nodes still naming it draw nothing.
#[unsafe(no_mangle)]
pub extern "C" fn kui_fragment_remove(ptr: *mut KuiCtx, id: u64) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().remove_fragment(kui_core::FragmentId::from_ffi(id));
        }
    })
}

/// The whole WGSL module behind a handle — the app's source between the
/// core's prelude and epilogue — for a host that compiles it itself. The
/// string is borrowed and valid until the next call; false when the handle
/// is not live in this session.
#[unsafe(no_mangle)]
pub extern "C" fn kui_fragment_source(ptr: *mut KuiCtx, id: u64, out: *mut KuiStr) -> bool {
    guard(false, || {
        let (Some(c), false) = (unsafe { ctx(ptr) }, out.is_null()) else {
            return false;
        };
        let Some(src) = c
            .core()
            .fragment_module_source(kui_core::FragmentId::from_ffi(id))
        else {
            return false;
        };
        // Kept on the context so the pointer outlives this call, the way
        // every other borrowed string this header hands out is.
        c.fragment_source = src;
        unsafe {
            *out = KuiStr {
                ptr: c.fragment_source.as_ptr(),
                len: c.fragment_source.len(),
            }
        };
        true
    })
}

/// Registers a font file by path (memory-mapped); 0 when it cannot be read
/// or holds no usable face.
#[unsafe(no_mangle)]
pub extern "C" fn kui_font_load_file(ptr: *mut KuiCtx, path: KuiStr) -> u64 {
    guard(0, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return 0;
        };
        let path = kstr(path).into_owned();
        c.core().load_font_file(path).map_or(0, |id| id.to_ffi())
    })
}

/// Loads every font file under a folder (recursively) so its families can
/// be picked by name with `kui_font_add_system`; returns the face count.
#[unsafe(no_mangle)]
pub extern "C" fn kui_font_load_dir(ptr: *mut KuiCtx, dir: KuiStr) -> usize {
    guard(0, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return 0;
        };
        let dir = kstr(dir).into_owned();
        c.core().load_fonts_dir(dir)
    })
}

/// Scans the system's fonts again, so a font installed while the app runs
/// is found (`Core::reload_system_fonts`); returns how many faces came and
/// went, 0 when nothing did.
#[unsafe(no_mangle)]
pub extern "C" fn kui_font_reload_system(ptr: *mut KuiCtx) -> usize {
    guard(0, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return 0;
        };
        c.core().reload_system_fonts()
    })
}

/// Forgets a registered font; text still naming it falls back to the
/// family.
#[unsafe(no_mangle)]
pub extern "C" fn kui_font_remove(ptr: *mut KuiCtx, id: u64) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().remove_font(kui_core::FontId::from_ffi(id));
        }
    });
}

// -- Audio -------------------------------------------------------------------
// Sounds are resources, playback is commands the driver drains; kui_run
// plays them itself, a host with its own loop drains kui_take_audio_commands.

/// Registers a sound from its encoded file bytes (wav/ogg/mp3/flac, copied);
/// returns its handle for `KuiSpec.click_sound` / `kui_audio` / `kui_play`,
/// 0 when empty.
#[unsafe(no_mangle)]
pub extern "C" fn kui_sound_add(ptr: *mut KuiCtx, data: *const u8, len: usize) -> u64 {
    guard(0, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return 0;
        };
        if data.is_null() || len == 0 {
            return 0;
        }
        let bytes = unsafe { std::slice::from_raw_parts(data, len) }.to_vec();
        c.core().add_sound(bytes).to_ffi()
    })
}

/// Forgets a registered sound and stops its playbacks.
#[unsafe(no_mangle)]
pub extern "C" fn kui_sound_remove(ptr: *mut KuiCtx, id: u64) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().remove_sound(kui_core::SoundId::from_ffi(id));
        }
    });
}

/// Starts a playback; returns its id for kui_stop / kui_set_volume /
/// kui_pause / kui_resume. `opts` may be NULL (defaults). A non-NULL `tag`
/// (consumed) asks for a `{kind="sound", phase="ended", playback, tag}`
/// event when the playback finishes on its own.
#[unsafe(no_mangle)]
pub extern "C" fn kui_play(
    ptr: *mut KuiCtx,
    sound: u64,
    opts: *const KuiPlay,
    tag: *mut KuiValue,
) -> u64 {
    guard(0, || {
        let tag = take_msg(tag);
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return 0;
        };
        let mut po = kui_core::PlayOptions::default();
        if let Some(o) = unsafe { opts.as_ref() } {
            po.volume = o.volume;
            po.looped = o.looped != 0;
            po.fade_in_ms = o.fade_in_ms;
        }
        po.tag = tag;
        c.core().play(kui_core::SoundId::from_ffi(sound), po).0
    })
}

/// Stops a playback from [`kui_play`], fading out over `fade_ms` (0 for
/// at once).
#[unsafe(no_mangle)]
pub extern "C" fn kui_stop(ptr: *mut KuiCtx, playback: u64, fade_ms: f32) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().stop(kui_core::PlaybackId(playback), fade_ms);
        }
    });
}

/// Sets a playback's volume (linear amplitude, 0..1), easing to it over
/// `tween_ms`.
#[unsafe(no_mangle)]
pub extern "C" fn kui_set_volume(ptr: *mut KuiCtx, playback: u64, volume: f32, tween_ms: f32) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core()
                .set_volume(kui_core::PlaybackId(playback), volume, tween_ms);
        }
    });
}

/// Pauses a playback, fading out over `fade_ms`.
#[unsafe(no_mangle)]
pub extern "C" fn kui_pause(ptr: *mut KuiCtx, playback: u64, fade_ms: f32) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().pause(kui_core::PlaybackId(playback), fade_ms);
        }
    });
}

/// Resumes a paused playback, fading in over `fade_ms`.
#[unsafe(no_mangle)]
pub extern "C" fn kui_resume(ptr: *mut KuiCtx, playback: u64, fade_ms: f32) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().resume(kui_core::PlaybackId(playback), fade_ms);
        }
    });
}

/// Sets the master volume every playback is scaled by (0..1), easing to
/// it over `tween_ms`.
#[unsafe(no_mangle)]
pub extern "C" fn kui_set_master_volume(ptr: *mut KuiCtx, volume: f32, tween_ms: f32) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().set_master_volume(volume, tween_ms);
        }
    });
}

/// An audio node: a playback retained by key while the frame declares it
/// (present = playing, gone = stopped; volume/paused apply live, a changed
/// src restarts). `finish` changes what gone means — the playback is
/// released to play itself out rather than stopped, except for a loop.
/// Empty label = a key from the tree position. `tag` (nullable, consumed)
/// rides the `ended` event, and survives a release. Returns the node key.
#[unsafe(no_mangle)]
pub extern "C" fn kui_audio(
    ptr: *mut KuiCtx,
    label: KuiStr,
    spec: *const KuiAudio,
    tag: *mut KuiValue,
) -> u64 {
    guard(0, || {
        let tag = take_msg(tag);
        let (Some(c), Some(a)) = (unsafe { ctx(ptr) }, unsafe { spec.as_ref() }) else {
            return 0;
        };
        let mut s = kui_core::AudioSpec::new(kui_core::SoundId::from_ffi(a.src))
            .volume(a.volume)
            .paused(a.paused != 0);
        if a.looped != 0 {
            s = s.looped();
        }
        if a.finish != 0 {
            s = s.finish();
        }
        s.tag = tag;
        let core = c.core();
        if label.ptr.is_null() || label.len == 0 {
            core.audio_node(s).0
        } else {
            core.audio_node_keyed(&kstr(label), s).0
        }
    })
}

/// Drains queued audio commands into `out` (up to `cap`; the rest are
/// dropped, so size it generously); returns the count. Only for hosts
/// driving their own audio device — kui_run plays them itself.
#[unsafe(no_mangle)]
pub extern "C" fn kui_take_audio_commands(
    ptr: *mut KuiCtx,
    out: *mut KuiAudioCommand,
    cap: usize,
) -> usize {
    use kui_core::AudioCommand as A;
    guard(0, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return 0;
        };
        if out.is_null() || cap == 0 {
            return 0;
        }
        let cmds = c.core().take_audio_commands();
        let n = cmds.len().min(cap);
        for (i, cmd) in cmds.into_iter().take(n).enumerate() {
            let mut o = KuiAudioCommand::default();
            match cmd {
                A::Play {
                    playback,
                    sound,
                    volume,
                    looped,
                    fade_in_ms,
                } => {
                    o.kind = KUI_AUDIO_PLAY;
                    o.playback = playback.0;
                    o.sound = sound.to_ffi();
                    o.volume = volume;
                    o.ms = fade_in_ms;
                    o.looped = looped as u32;
                }
                A::Stop { playback, fade_ms } => {
                    o.kind = KUI_AUDIO_STOP;
                    o.playback = playback.0;
                    o.ms = fade_ms;
                }
                A::SetVolume {
                    playback,
                    volume,
                    tween_ms,
                } => {
                    o.kind = KUI_AUDIO_SET_VOLUME;
                    o.playback = playback.0;
                    o.volume = volume;
                    o.ms = tween_ms;
                }
                A::Pause { playback, fade_ms } => {
                    o.kind = KUI_AUDIO_PAUSE;
                    o.playback = playback.0;
                    o.ms = fade_ms;
                }
                A::Resume { playback, fade_ms } => {
                    o.kind = KUI_AUDIO_RESUME;
                    o.playback = playback.0;
                    o.ms = fade_ms;
                }
                A::MasterVolume { volume, tween_ms } => {
                    o.kind = KUI_AUDIO_MASTER_VOLUME;
                    o.volume = volume;
                    o.ms = tween_ms;
                }
                A::Unload { sound } => {
                    o.kind = KUI_AUDIO_UNLOAD;
                    o.sound = sound.to_ffi();
                }
            }
            unsafe { out.add(i).write(o) };
        }
        n
    })
}

/// A host driving its own device reports a playback finished on its own;
/// a tagged one becomes a `sound` event for kui_poll_event.
#[unsafe(no_mangle)]
pub extern "C" fn kui_audio_ended(ptr: *mut KuiCtx, playback: u64) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().audio_ended(kui_core::PlaybackId(playback));
            let pending = c.core().take_pending_events();
            c.absorb(pending);
        }
    });
}

/// The same host reports that a stop it drained landed on a playback
/// still running, `at` seconds in: a one-shot `audio` node that went away
/// without `finish` is named in a `truncated-playback` warning
/// ([`kui_take_warnings`]); any other stop reports nothing.
#[unsafe(no_mangle)]
pub extern "C" fn kui_audio_truncated(ptr: *mut KuiCtx, playback: u64, at: f64) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().audio_truncated(kui_core::PlaybackId(playback), at);
        }
    });
}

/// The same host reports that its device refused a play it drained: a
/// tagged playback becomes a `sound` event with phase `refused` for
/// kui_poll_event, and the node that asked is named in a
/// `playback-refused` warning either way.
#[unsafe(no_mangle)]
pub extern "C" fn kui_audio_refused(ptr: *mut KuiCtx, playback: u64) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().audio_refused(kui_core::PlaybackId(playback));
            let pending = c.core().take_pending_events();
            c.absorb(pending);
        }
    });
}

/// Forgets a registered image; nodes still naming it draw nothing.
#[unsafe(no_mangle)]
pub extern "C" fn kui_image_remove(ptr: *mut KuiCtx, id: u64) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().remove_image(kui_core::ImageId::from_ffi(id));
        }
    });
}
