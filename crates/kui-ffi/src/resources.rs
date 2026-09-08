//! Resources: images, fonts and sounds registered through a context, and
//! the playback commands a host with its own loop drains.

use super::*;

/// Registers a w×h RGBA image (pixels copied); returns its handle, 0 on
/// failure. Draw it with `kui_image`; free it with `kui_image_remove`.
#[unsafe(no_mangle)]
pub extern "C" fn kui_image_add(ptr: *mut KuiCtx, w: u32, h: u32, rgba: *const u8) -> u64 {
    guard(0, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return 0;
        };
        if rgba.is_null() || w == 0 || h == 0 {
            return 0;
        }
        let data = unsafe { std::slice::from_raw_parts(rgba, (w * h * 4) as usize) }.to_vec();
        c.core().resources.add_image(w, h, data).to_ffi()
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

#[unsafe(no_mangle)]
pub extern "C" fn kui_stop(ptr: *mut KuiCtx, playback: u64, fade_ms: f32) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().stop(kui_core::PlaybackId(playback), fade_ms);
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn kui_set_volume(ptr: *mut KuiCtx, playback: u64, volume: f32, tween_ms: f32) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core()
                .set_volume(kui_core::PlaybackId(playback), volume, tween_ms);
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn kui_pause(ptr: *mut KuiCtx, playback: u64, fade_ms: f32) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().pause(kui_core::PlaybackId(playback), fade_ms);
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn kui_resume(ptr: *mut KuiCtx, playback: u64, fade_ms: f32) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().resume(kui_core::PlaybackId(playback), fade_ms);
        }
    });
}

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
                    o.kind = 1;
                    o.playback = playback.0;
                    o.sound = sound.to_ffi();
                    o.volume = volume;
                    o.ms = fade_in_ms;
                    o.looped = looped as u32;
                }
                A::Stop { playback, fade_ms } => {
                    o.kind = 2;
                    o.playback = playback.0;
                    o.ms = fade_ms;
                }
                A::SetVolume {
                    playback,
                    volume,
                    tween_ms,
                } => {
                    o.kind = 3;
                    o.playback = playback.0;
                    o.volume = volume;
                    o.ms = tween_ms;
                }
                A::Pause { playback, fade_ms } => {
                    o.kind = 4;
                    o.playback = playback.0;
                    o.ms = fade_ms;
                }
                A::Resume { playback, fade_ms } => {
                    o.kind = 5;
                    o.playback = playback.0;
                    o.ms = fade_ms;
                }
                A::MasterVolume { volume, tween_ms } => {
                    o.kind = 6;
                    o.volume = volume;
                    o.ms = tween_ms;
                }
                A::Unload { sound } => {
                    o.kind = 7;
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
            c.events.extend(pending);
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn kui_image_remove(ptr: *mut KuiCtx, id: u64) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().remove_image(kui_core::ImageId::from_ffi(id));
        }
    });
}
