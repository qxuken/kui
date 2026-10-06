//! The ABI handshake: the version a host compares, and the size-led
//! `[out]` structs the library writes no further into than the host
//! reserved. `include/kui.h` mirrors the structs in `types`; `abi_parity`
//! pins the two at build time.

// ---------------------------------------------------------------------------
// ABI version, and who is allowed to write which struct
//
// `include/kui.h` is hand-mirrored from the `repr(C)` structs below, and
// `mod abi_parity` settles the two at build time — but only for a host
// compiled against the header it links against. Nothing settles an old
// *binary* against a new library, and the two directions are not equally
// forgiving:
//
// - **[in]** — the host allocates and fills it, the library reads it:
//   `KuiSpec`, `KuiSizing`, `KuiKeyframe`, `KuiEnter`, `KuiTextStyle`,
//   `KuiSpan`, `KuiPlay`, `KuiAudio`, `KuiWindowConfig`, `KuiTheme` (which
//   `kui_theme_set` reads, and `kui_theme` writes — so it is bound by the
//   stricter [out] rule below). Appending a field is a bump. The note
//   here used to say the opposite — "the library reads no further than
//   the host wrote" — and the library never did: `kui_open` copies
//   `*spec`, `window_config_of` reads every field, so a host built
//   against the shorter `KuiSpec` had the appended field read from
//   whatever followed its struct on the stack, a garbage `KuiStr` in
//   `tooltip`'s case. Only a host-written `size` could make an append
//   safe, and [in] structs carry none — a `size` on every `KuiSpec`
//   literal was the tax declined below — so they are held to the one rule
//   with everything else. The array-shaped ones (`KuiSpan`,
//   `KuiMenuItem`) were bumps already, for the stride (ABI 8, ABI 13).
// - **[out]** — the host allocates it, the library writes it: `KuiEvent`,
//   `KuiDrawData`, `KuiTextMetrics`, `KuiScrollGeometry`, `KuiTextHit`,
//   `KuiCaretRect`, `KuiWindowCommand`, `KuiTheme`. Appending a field
//   here is memory corruption at a host that has not recompiled — it
//   reserved the shorter struct and the library writes the longer one — so
//   each of these leads with `size`, which the host sets to its own
//   `sizeof`. [`write_out`] writes no further than that, which turns the
//   append back into a compatible change.
// - **[out-array]** — the host allocates an array, the library fills up to
//   `cap` of its elements: `KuiAccessNode`, `KuiAccessRun`, `KuiWarning`,
//   `KuiAudioCommand`, `KuiAnnouncement`, `KuiSystemFont`. A `size` field
//   cannot save these. The library strides by its own `size_of`, so
//   element 1 lands past the host's element 1 whatever element 0 says,
//   and the damage is done before any in-band handshake could be read.
//   Growing one of these means adding an
//   explicit stride parameter — a source break every host sees — and
//   bumping `KUI_ABI_VERSION`.
// - **[lib]** — the library allocates it and the host reads it: `KuiQuad`,
//   through `KuiDrawData.quads`. The same stride problem, mirrored: the
//   host walks the array with its own `sizeof`. Read-only, so it misreads
//   rather than corrupting, but it misreads every quad after the first.
//   Growing it bumps the version.
//
// `KuiStr` is the exception: it crosses in both directions (`kui_edit_text`
// and `kui_value_as_str` write one) and its layout is frozen at (ptr, len).
// `KuiEvent` is also handed to `kui_run`'s `on_event` as a library-owned
// `*const KuiEvent`; a single struct behind a pointer is safe to append to,
// since the host reads only the prefix it knows, so it is the [out] use
// above that constrains the type.

/// The ABI this build implements, returned by [`kui_abi_version`].
///
/// `KUI_ABI_VERSION` in `include/kui.h` is the one a host compiled
/// against. A host compares the two for equality before its first other
/// call: the mismatch to catch is a newer library against an older host,
/// which corrupts memory rather than merely missing a feature.
///
/// The number bumps whenever the layout of any struct the header declares
/// changes (`[in]`, `[out]`, `[out[]]` or `[lib]`, appends included) and
/// whenever an existing function's signature changes. It does not bump for
/// a new function: a host that does not call one is unaffected, and one
/// that does fails to link. It bumps per change, not per release, so the
/// log below lists breaks, not published versions; skipped numbers are
/// normal and cost a host nothing. The same log is kept in the
/// `-- ABI version --` block of `include/kui.h`, which is the copy a C
/// host reads.
///
/// - ABI 4: `KuiEvent` gains `window`, the first append to an `[out]`
///   struct; a host that sets `size` needs no source change.
/// - ABI 5: multi-window. `kui_take_window_commands`'s `uint32_t` array
///   became the `KuiWindowCommand` struct behind `kui_take_window_command`,
///   and `kui_env_set_window` leads with the window id.
/// - ABI 6: `KuiWindowCommand` gains `width` and `height` for
///   `KUI_CMD_SET_SIZE`.
/// - ABI 7: popups. `KuiWindowConfig` gains the four `anchor_*` floats and
///   `KuiWindowCommand` appends `owner`. Because the command embeds the
///   config by value, an un-recompiled host's reservation is refused
///   rather than short-written: its drain loop sees an empty queue.
/// - ABI 8: `KuiSpan` gains `bg`; spans travel as an array, so the stride
///   moved.
/// - ABI 9: `KuiDrawData` gains `fragments`, `fragment_count` and `time`.
/// - ABI 10: `KuiEvent` gains `reply_sink`, so a plugin linked against a
///   different copy of this library still reaches its host's `kui_reply`.
/// - ABI 11: the clip left `KuiQuad` for an index into `KuiDrawData.clips`;
///   read `dd.clips[q.clip]` where you read `q.clip`. Entry zero clips
///   nothing.
/// - ABI 12: `kui_cells` gains `origin_line` (a signature change; pass 0
///   to keep what you had).
/// - ABI 13: `KuiMenuItem` gains `checked`; an array element, so the
///   stride moved.
/// - ABI 14: `KuiDrawData` gains `textures` and `texture_count`, and
///   `KUI_QUAD_TEXTURE` is a new quad kind.
/// - ABI 15: `KuiFragmentDraw` gains `image_source`, `image_texture` and
///   `image_uv`. Also new, with no break of their own: `KuiMetrics`,
///   `KuiColorToken`, `KuiLengthToken`, `KuiColorOp`, `KuiDerivedToken`
///   and their functions.
/// - ABI 16: `kui_run_with` takes a `KuiRunConfig` as its third argument
///   (a signature change); `kui_run` is unchanged. From here on an `[in]`
///   append bumps too, since the library reads the whole struct.
/// - ABI 17: `underline_color` and `underline_style` on `KuiTextStyle` and
///   `KuiSpan`, `ul` on `KuiCell`.
/// - ABI 18: `on_drop` and `drop_bg` on `KuiSpec`, with the file-drag
///   input functions, `kui_set_devtools_tab`, `kui_devtools_current_tab`,
///   `kui_on_teardown`, `kui_set_icon`, `kui_select`, `KUI_TABLE` and
///   `KUI_VALUE_CARET_SOLID`.
/// - ABI 19: `float_clip`, `aspect_ratio`, `mixed`, `value_step` and
///   `on_change` on `KuiSpec`; `frame_latency` on `KuiRunConfig`; the
///   stock controls, the file dialogs, `kui_host_rect`, `kui_system_fonts`
///   and the `KUI_SPACE_*` / `KUI_BASELINE` alignments.
/// - ABI 20: `pixel_snap`, `keep_focus`, `on_focus`, `rules`, `rule_w`,
///   `on_button`, `buttons`, `overscroll` and `scroll_axes` on `KuiSpec`
///   (64-bit size 648); `bg_radius` on `KuiSpan`.
/// - ABI 21: `modifier_keys` on `KuiSpec`; the `kmods` word of
///   `kui_input_key_down` and its siblings also carries `KUI_KLOCK_*` and
///   `KUI_KLOC_*` bits.
/// - ABI 22: `min_w_size`, `max_w_size`, `min_h_size` and `max_h_size` on
///   `KuiSpec` (64-bit size 680); `KuiSizing` takes `KUI_CALC`.
/// - ABI 23: `bounce` on `KuiSpec` (64-bit size 688); `KUI_EASE_SMOOTH`
///   and `KUI_EASE_SNAPPY` are new easings.
/// - ABI 24: `kui_polyline`, `kui_path` and `kui_path_d` take a `dash`
///   (signature changes): five floats, or NULL for a solid stroke.
///   `gradient` on `KuiSpec` (64-bit size 696), with `KuiGradient` and
///   `KuiGradientStop`.
/// - ABI 25: `scroll_mods` on `KuiSpec` (64-bit size 704). Still 25 with
///   `kui_input_open`, a new function.
pub const KUI_ABI_VERSION: u32 = 25;

/// The ABI version this library implements ([`KUI_ABI_VERSION`]), for a
/// host to compare for equality with the `KUI_ABI_VERSION` of the header
/// it compiled against, before its first other call.
///
/// A host loads whatever `libkui_ffi` the system hands it, and a newer
/// library writing a longer struct into an older host's shorter one is
/// silent memory corruption, not a crash; this check is the only way to
/// catch it.
#[unsafe(no_mangle)]
pub extern "C" fn kui_abi_version() -> u32 {
    KUI_ABI_VERSION
}

/// Bytes through the end of field `$f` (of type `$t`) in `$ty`: what a
/// caller must have reserved to hold the fields up to and including it.
///
/// Used to state each `[out]` struct's ABI-1 layout without writing a number
/// down — `usize` and pointer widths differ per target — and without
/// tracking future growth, which is the point: appending a field never
/// moves the last ABI-1 field, so the floor stays put.
macro_rules! abi_through {
    ($ty:ty, $f:ident, $t:ty) => {
        (std::mem::offset_of!($ty, $f) + std::mem::size_of::<$t>()) as u32
    };
}

/// A struct the library writes into memory the **caller** reserved.
///
/// Each leads with `size`, set by the caller to the `sizeof` of its own
/// copy, so the library can write no further than the caller's reservation
/// and a later appended field costs an un-recompiled host nothing. `[in]`
/// structs carry no `size`: a field on every `KuiSpec` literal would be a
/// tax on every builder call, so an `[in]` append is an ABI bump instead.
///
/// # Safety
///
/// The implementor must be `repr(C)` with `size: u32` as its first field,
/// so that reading the first four bytes behind a `*mut Self` reads the
/// caller's reservation and nothing else.
pub(crate) unsafe trait OutParam: Sized {
    /// This struct's layout in ABI 1, where the handshake starts, measured
    /// through its last ABI-1 field. A caller reserving less than this
    /// predates the handshake entirely, so the call refuses rather than
    /// guessing what the bytes mean.
    const ABI_V1_SIZE: u32;

    /// Where the library records how many bytes it filled.
    fn size_mut(&mut self) -> &mut u32;
}

/// Whether `out` is a reservation this library can write into: non-NULL,
/// and at least the ABI-1 layout.
///
/// Separate from `write_out` so a call can refuse *before* it moves any
/// state — `kui_poll_event` must not pop an event it then cannot deliver.
pub(crate) fn out_accepts<T: OutParam>(out: *mut T) -> bool {
    if out.is_null() {
        return false;
    }
    // `size` leads the struct (the trait's safety contract), so this reads
    // the caller's reservation without assuming the rest of it is there.
    // The caller must have set it; an uninitialized `size` is the one thing
    // this cannot catch, which is why the header leads with the
    // KUI_*_INIT initializers rather than describing the field.
    let reserved = unsafe { out.cast::<u32>().read() };
    reserved >= T::ABI_V1_SIZE
}

/// Writes `value` into `out`, clipped to what the caller reserved, and
/// reports how many bytes that was in `out`'s own `size`.
///
/// Returns false — writing nothing — when `out_accepts` refuses. Growth
/// is append-only by the note above, so "the fields that fit" is exactly
/// "the first `n` bytes", and the `size` written back is stable under
/// repetition: a poll loop reusing one struct clamps to the same `n` every
/// time round.
pub(crate) fn write_out<T: OutParam>(out: *mut T, mut value: T) -> bool {
    if !out_accepts(out) {
        return false;
    }
    let reserved = unsafe { out.cast::<u32>().read() } as usize;
    let n = reserved.min(std::mem::size_of::<T>());
    *value.size_mut() = n as u32;
    unsafe {
        std::ptr::copy_nonoverlapping((&raw const value).cast::<u8>(), out.cast::<u8>(), n);
    }
    true
}
