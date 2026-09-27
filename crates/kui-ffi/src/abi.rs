//! The ABI handshake: the version a host compares, and the size-led
//! [out] structs the library writes no further into than the host
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
//   stricter [out] rule below). Appending a field is a bump, since
//   2026-09-14 (backlog AR50, ADR 0006's amendment). The note here used
//   to say the opposite — "the library reads no further than the host
//   wrote" — and the library never did: `kui_open` copies `*spec`,
//   `window_config_of` reads every field, so a host built against the
//   shorter `KuiSpec` had the appended field read from whatever followed
//   its struct on the stack, a garbage `KuiStr` in `tooltip`'s case.
//   Only a host-written `size` could make an append safe, and [in]
//   structs carry none — a `size` on every `KuiSpec` literal was the tax
//   declined below — so they are held to the one rule with everything
//   else. The array-shaped ones (`KuiSpan`, `KuiMenuItem`) were bumps
//   already, for the stride (ABI 8, ABI 13).
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

/// The ABI this build implements, returned by `kui_abi_version`.
/// `KUI_ABI_VERSION` in `include/kui.h` is the one a host compiled against,
/// and `mod abi_parity` asserts the two agree.
///
/// **Bump it when the layout of any struct in the note above changes** —
/// [in], [out], [out-array] or [lib], in any way, appends included — and
/// when an existing function's signature changes (ABI 12, ABI 16). **Do
/// not bump it** for a new function: a host that does not call one is
/// unaffected, and one that does fails to *link*, which is loud. An [in]
/// append was exempt until 2026-09-14 on a premise the readers never kept
/// (the note above, backlog AR50); `abi_parity::an_in_struct_s_size_is_
/// the_abi_s` pins every [in] layout so the next append fails a test
/// until this number moves.
///
/// **It bumps per change, not per release** (ADR 0006 decision 8), so the
/// entries below are a log of breaks and not a list of published versions:
/// 1 through 5 all came and went between two releases and none of them
/// shipped, the scheme having landed after 0.1.0-alpha.5. A skipped number
/// is normal and costs a host nothing, because the check is equality — no
/// one reasons about the distance between two of these.
///
/// **The same log is mirrored in the `-- ABI version --` block of
/// `include/kui.h`**, which is the copy a C host actually reads — it has
/// the header, not this file. A bump writes an entry in both.
///
/// ABI 4 was the first bump that appended to an [out] struct
/// (`KuiEvent.window`). Hosts that set `size` need no source change for it;
/// the bump is for the ones that skipped `kui_abi_version()` and would
/// otherwise take the short write unaware.
///
/// ABI 5 is multi-window (ADR 0004, step 3): `kui_take_window_commands`'s
/// `uint32_t` array became the `KuiWindowCommand` [out] struct behind
/// `kui_take_window_command`, and `kui_env_set_window` gained the window
/// id. Both are source breaks a host sees at compile time; the bump is
/// for a binary that was not recompiled.
///
/// ABI 6 appends `width`/`height` to `KuiWindowCommand`, for the
/// `KUI_CMD_SET_SIZE` that `kui_set_window_size` queues (ADR 0004 step 5).
/// It is the compatible kind of change — the struct leads with `size`, so
/// a host that reserved through `config` gets the prefix it knows and
/// stops — and no host that never calls `kui_set_window_size` can even
/// receive the new verb. The version bumps anyway, for the host that
/// skipped the check.
///
/// ABI 7 is the popup (ADR 0004 step 4): `KuiWindowConfig` gains the four
/// `anchor_*` floats a popup is placed against, and `KuiWindowCommand`
/// appends `owner`. **This is the first change the size handshake cannot
/// make compatible**, and it is worth being precise about why. Appending
/// to `KuiWindowConfig` is the compatible move for an [in] struct, and
/// `owner` is the compatible move for an [out] one — but `KuiWindowCommand`
/// embeds a `KuiWindowConfig` **by value**, and a field appended inside an
/// embedded struct moves every field after it. So the [out] floor
/// (`ABI_V1_SIZE`, measured through `config`) rises by those 16 bytes, past
/// the whole size of the ABI-6 struct: an ABI-6 host's reservation is
/// *refused* by `out_accepts` rather than short-written, and its drain loop
/// sees an empty queue instead of its windows. Nothing is corrupted, which
/// is the handshake doing its job; `kui_abi_version()` is what turns a
/// silent empty queue into a message. Every host recompiles anyway — the
/// header changed — and none of them edits a line.
///
/// ABI 8 appends `bg` to `KuiSpan` (backlog C22). An [in] struct, which
/// the rule above says not to bump for — except that spans travel as an
/// array (`kui_rich_text`, `kui_measure_rich_text` take `const KuiSpan *,
/// size_t`), so the append moved the stride, which is the [out-array]
/// hazard mirrored: an old binary's element 1 is read at the wrong place
/// whatever element 0 says. The bump makes that a message. Recompile and
/// nothing in a host's source changes; a zeroed `bg` is none.
///
/// ABI 9 appends `fragments`, `fragment_count` and `time` to
/// `KuiDrawData` for ADR 0015's `fragment` element.
///
/// ABI 10 appends `reply_sink` to `KuiEvent`. Another [out] append, and by
/// the rule above one that would not need a bump — a host reserving the
/// older layout keeps polling correctly and never sees the field, which is
/// right, because the field is not for a host. The bump is for the other
/// side: `kui_reply` used to find its sink in a `thread_local`, which is
/// one sink *per copy of this library in the process*, and a plugin does
/// not always share the host's copy — on Windows it cannot, since a DLL
/// may not leave `kui_reply` undefined and resolve it from the executable
/// the way ELF does. Every reply then landed in a list nobody read. The
/// sink now travels on the event as a function pointer into the copy that
/// opened it. A plugin's source does not change; a plugin *binary* built
/// against ABI 9 must not be handed an ABI 10 event, and the version is
/// what says so.
///
/// ABI 12 appends `origin_line` to `kui_cells` (ADR 0017 decision 4): the
/// absolute line a grid's row 0 is, so a terminal's selection keeps its
/// ends across a scroll. This is the case the note above does not cover —
/// not an [out] struct's layout, not an [in] struct's append, not a new
/// function, but an existing function's *signature*. A host that does not
/// recompile passes one argument too few and the library reads whatever is
/// in that register, which is exactly the silent failure the version check
/// turns into a message.
///
/// ABI 11 takes the clip off `KuiQuad` and puts it behind an index into a
/// new `KuiDrawData::clips`. This is the second bump the size handshake
/// cannot absorb (ABI 7 was the first): `KuiQuad` travels as an array, so
/// the [out-array] hazard applies — the struct got 28 bytes shorter and
/// every field after `kind` moved, which an ABI-10 host reading element 1
/// of the new array would find as garbage whatever element 0 said. The
/// reason is cost, not tidiness: the clip was a rect and four radii on a
/// struct written once per quad and then walked again by the fade pass,
/// the backend's upload and the previous frame `depart` keeps, for a
/// value nearly every quad of a frame shares. A host reads
/// `dd.clips[q.clip]` where it used to read `q.clip` and `q.clip_radius`;
/// entry zero clips nothing, so there is no null case.
///
/// ABI 13 appends `checked` to `KuiMenuItem` (the menu bar, ADR 0018): a
/// row that is a setting rather than a command draws a checkmark. An [in]
/// struct, which the rule as it then stood exempted — but this one travels
/// as an *array*, so the append moves the stride and every row after the
/// first is read from the wrong bytes. The same exception `KuiSpan` is,
/// for the same reason (ABI 8). A recompiled host's zeroed tail is
/// `checked = 0`, which is what every row had before.
///
/// ABI 14 appends `textures` and `texture_count` to `KuiDrawData` for
/// ADR 0025's texture-backed images — an [out] append the size handshake
/// covers, so a host reserving the ABI-13 layout keeps working and never
/// sees a `KUI_QUAD_TEXTURE` quad's side entry (it draws that quad as a
/// solid, wrongly and harmlessly, as a pre-segment host draws a segment).
/// The bump is for `KUI_QUAD_TEXTURE` itself: a ninth kind a host's own
/// renderer may want to refuse by version rather than meet by surprise.
///
/// ABI 15 appends `image_source`, `image_texture` and `image_uv` to
/// `KuiFragmentDraw` for the fragment image input (backlog V1, ADR 0025
/// decision 7). An *array* element again, so the append moves the stride
/// — the `KuiSpan` and `KuiMenuItem` exception, for the same reason.
/// Recompile; a host that never reads `fragments` has nothing to change.
///
/// ABI 16 gives `kui_run_with` a `KuiRunConfig` (backlog AR27): a third
/// argument, between the title and the view. The struct is [in] and would
/// not bump on its own; the bump is ABI 12's case again — an existing
/// function's *signature* — since a host that did not recompile passes
/// one argument too few and the library reads its view callback out of
/// the register the config should be in. `kui_run` is unchanged.
///
/// ABI 17 appends the underline's own colour and shape (backlog K4) to
/// `KuiTextStyle` and `KuiSpan` (`underline_color`, `underline_style`)
/// and the underline colour `ul` to `KuiCell` — three [in] appends under
/// the withdrawn rule, two of them array elements whose stride moved.
/// Recompile; a zeroed field is what the struct meant before.
///
/// ABI 18 appends `on_drop` and `drop_bg` to `KuiSpec` for the drop zone
/// (ADR 0031, backlog C40) — the first [in] append under the amended
/// rule, bumping because the library reads the whole struct and a host
/// that did not recompile would have the two read from past its end.
/// Recompile; a zeroed tail is no zone and no colour, which is what every
/// node was. The same version adds `kui_input_drag_files`,
/// `kui_input_drop_files`, `kui_input_drag_cancel`, `kui_is_drop_target`
/// and `kui_drop_target` — five functions, nothing the library writes
/// moved. Still at 18: `kui_set_devtools_tab`, `kui_devtools_current_tab`
/// and `kui_on_teardown` — functions, no struct (backlog RG1 chose the
/// setter over a `KuiRunConfig` append for the last: the config is the
/// window and `kui_run` takes none) — `kui_set_icon` for the same reason
/// (backlog F86) — and `kui_select` (backlog F73, a
/// widget function), `KUI_TABLE` (F75, a value of `KuiSpec.dir`) and
/// `KUI_VALUE_CARET_SOLID` (F68, a bit in `value_set`): nothing a host
/// had laid out moved for any of the three.
///
/// ABI 19 appends `float_clip` to `KuiSpec` (backlog F90): a
/// parent-anchored float that sets it takes its parent's clip instead of
/// escaping it. An [in] append under the amended rule, as ABI 18's was.
/// Recompile; a zeroed field is the float that escapes, which is what
/// every float was. Also new under 19, and no break of its own:
/// `kui_host_rect` (backlog F92), one function writing the
/// `KuiLayoutRect` it already had.
/// The same bump appends `aspect_ratio` after it (backlog C14); a zeroed
/// field is no ratio. And `mixed`, `value_step` (`KUI_VALUE_STEP`) and
/// `on_change` after it for the stock controls (ADR 0034), with the
/// functions `kui_checkbox`, `kui_radio`, `kui_switch`,
/// `kui_radio_group_open` and `kui_slider` and the flag
/// `KUI_ACCESS_MIXED`. And `KuiRunConfig.frame_latency` (backlog C47).
/// And the file dialogs (backlog C51): the new [in] structs `KuiFileFilter`
/// and `KuiFileDialog` and five functions, `kui_request_files` through
/// `kui_input_files`. Still at 19, since nothing a host had laid out
/// moved: `KuiSystemFont`, a new [out-array] struct, with
/// `kui_system_fonts` (backlog F97).
/// `KUI_SPACE_BETWEEN`, `KUI_SPACE_AROUND`,
/// `KUI_SPACE_EVENLY` and `KUI_BASELINE` (backlog C13) are new values of
/// `main_align` / `cross_align`, which moved nothing.
///
/// ABI 20 appends `pixel_snap` to `KuiSpec`: a box that sets it is painted
/// with each edge on a whole pixel, so it meets a text's background or
/// another snapped box without a seam. An [in] append; recompile. A zeroed
/// field is the box drawn where layout put it, which is what every box was.
/// The same bump appends `keep_focus` after it (backlog DX10): a press
/// that leaves keyboard focus where it was; zeroed, a press focuses as it
/// did. Then `on_focus` (backlog DX18): focus entering and leaving the
/// node's subtree, as an event; NULL hears nothing. Then `rules` and
/// `rule_w` (backlog DX21): a table's grid lines; zeroed, none. Then
/// `on_button` and `buttons` (backlog F104): the non-primary buttons as
/// events on the node that claims them, captured from press to release;
/// NULL hears nothing, and a zeroed `buttons` with `on_button` set claims
/// all three kinds. The 64-bit size is 640. Recompile.
/// The same bump appends `bg_radius` to `KuiSpan` (backlog F101): a
/// span's background rounded and joined with the ones it meets. On a
/// 64-bit target it takes what was the struct's tail padding, so the
/// stride did not move there, but a host that did not recompile leaves
/// those bytes to chance (on a 32-bit one the stride moved, as ABI 8's
/// did). Recompile; a zeroed field is the square background every span
/// had.
pub const KUI_ABI_VERSION: u32 = 20;

/// The ABI version this library implements, for a host to compare against
/// the `KUI_ABI_VERSION` of the header it compiled against, before its
/// first other call.
///
/// This is the one mismatch a C host cannot otherwise detect: the header
/// and the library are settled at build time by `mod abi_parity`, but a
/// host loads whatever `libkui_ffi` the system hands it, and the failure
/// that follows (a newer library writing a longer `KuiEvent` into an older
/// host's shorter one) is silent memory corruption, not a crash.
#[unsafe(no_mangle)]
pub extern "C" fn kui_abi_version() -> u32 {
    KUI_ABI_VERSION
}

/// Bytes through the end of field `$f` (of type `$t`) in `$ty`: what a
/// caller must have reserved to hold the fields up to and including it.
///
/// Used to state each [out] struct's ABI-1 layout without writing a number
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
/// and a later appended field costs an un-recompiled host nothing. This is
/// deliberately not the rule for [in] structs: the library only reads
/// those, so a short one is already safe, and a `size` field would be a tax
/// on every `KuiSpec` literal in every builder call.
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
/// Separate from [`write_out`] so a call can refuse *before* it moves any
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
/// Returns false — writing nothing — when [`out_accepts`] refuses. Growth
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
