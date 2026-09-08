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
//   `KuiSpan`, `KuiPlay`, `KuiAudio`, `KuiWindowConfig`. Appending a field is compatible: a
//   host that predates it passes the shorter struct, the library reads no
//   further than the host wrote, and the zeroed tail is the documented
//   default. `KuiSpec` grew `tooltip` exactly this way. The exception is
//   an [in] struct that travels as an *array* — `KuiSpan` — where an
//   append moves the stride and is a bump (ABI 8).
// - **[out]** — the host allocates it, the library writes it: `KuiEvent`,
//   `KuiDrawData`, `KuiTextMetrics`, `KuiScrollGeometry`, `KuiTextHit`,
//   `KuiCaretRect`, `KuiWindowCommand`. Appending a field
//   here is memory corruption at a host that has not recompiled — it
//   reserved the shorter struct and the library writes the longer one — so
//   each of these leads with `size`, which the host sets to its own
//   `sizeof`. [`write_out`] writes no further than that, which turns the
//   append back into a compatible change.
// - **[out-array]** — the host allocates an array, the library fills up to
//   `cap` of its elements: `KuiAccessNode`, `KuiAccessRun`, `KuiWarning`,
//   `KuiAudioCommand`. A `size` field cannot save these. The library
//   strides by its own `size_of`, so element 1 lands past the host's
//   element 1 whatever element 0 says, and the damage is done before any
//   in-band handshake could be read. Growing one of these means adding an
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
/// **Bump it when the layout of anything the library writes or allocates
/// changes** — an [out], [out-array] or [lib] struct in the note above, in
/// any way, appends included. **Do not bump it** for a field appended to an
/// [in] struct, which old hosts survive by construction, nor for a new
/// function: a host that does not call one is unaffected, and one that does
/// fails to *link*, which is loud.
///
/// **It bumps per change, not per release** (ADR 0006 decision 8), so the
/// entries below are a log of breaks and not a list of published versions:
/// 1 through 5 all came and went between two releases and none of them
/// shipped, the scheme having landed after 0.1.0-alpha.5. A skipped number
/// is normal and costs a host nothing, because the check is equality — no
/// one reasons about the distance between two of these.
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
pub const KUI_ABI_VERSION: u32 = 9;

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
