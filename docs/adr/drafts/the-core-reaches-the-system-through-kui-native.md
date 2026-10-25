---
status: proposed
date: 2026-10-25
---

# The core reaches the system only through kui-native's `system` module

> **Proposed (2026-10-25), grilled, not built; backlog C-sys. Its
> number and its backlog id are given when it lands, after Kit.** From
> the user, after F203 put `show_in_folder` in kui-native: "all system calls
> and integrations should go through the native layer". An audit of
> `kui-core` found four groups that do not — fonts, the perf recorder,
> the devtools and the clocks — and a grilling over one conversation
> settled how each moves. **Waits on** Kit
> (`claude/kui-tools-design-system-b8ffa1`) landing on main: that branch
> takes ADRs from 0059 on (it is writing 0067 now) and backlog ids from
> C59 on, and touches the same files in
> kui-ffi, kui-native and the core. [draft ADR I](the-cores-io-asks.md)
> builds on this one.

## Context

The audit of `kui-core` (2026-10-25, at `356dc309`) found:

- **Fonts.** `text::scan_system_fonts` calls `FontSystem::new()`, which
  walks the OS font folders (fontconfig on Linux) and reads the locale
  through `sys-locale`; `keep_measurable` opens every file again on
  `kui-font-check` threads; `DEFAULT_FAMILIES` is three `cfg(target_os)`
  tables. `Core::load_font_file` and `load_fonts_dir` read paths,
  `apply_system_fonts` loads by path on a rescan, `share_faces`
  memory-maps every installed file in an `unsafe` block, and a face not
  yet mapped is read from disk while text is shaped. cosmic-text's default
  features put fontconfig in the core's own manifest.
- **The perf recorder.** `perf_trace_to` creates the file,
  `perf_flush` and the recorder's `Drop` write it; `perf_from_env` reads
  `KUI_PERF_TRACE` (only kui-native calls it).
- **The devtools.** `capture_path` reads `KUI_CAPTURE_DIR`, `HOME` and
  `USERPROFILE`, asks the disk `is_dir()` and reads `temp_dir()` and
  `SystemTime`; "save trace" calls `std::fs::write` while a picture
  and a recording go out as a `CaptureCommand`; `devtools_from_env`
  reads `KUI_DEVTOOLS` (only kui-native calls it); `KUI_SMOKE_FRAMES` is
  read in the core though kui-native already reads it.
- **Clocks.** The recorder's epoch is `Instant::now`, its session is
  stamped with `SystemTime`, and a span's CPU time is
  `libc::clock_gettime(CLOCK_THREAD_CPUTIME_ID)` — the one reason the
  core depends on `libc`.

Everything else was clean: the `unsafe` in `calc.rs` and `edit.rs` is
memory only; audio, windows, captures and menus are commands as data;
sounds are decoded in kui-native; locks and atomics are in-process; the
other `fs` and `env` hits are tests or doc examples.

Three facts shape the answer. The headless C library
(`kui-ffi --no-default-features`, 5.1 MB against 12.3 MB) does not depend
on kui-native, which is the whole GUI runtime, yet its host makes Cores
and wants the system's fonts. Shaping needs a font's data the moment it
asks, so fonts cannot be an ask a runner applies later, as F203's
`MenuAction::ShowInFolder` is. And kui-core's own tests construct a Core
or a Session about 1,380 times; other crates' about 200 more.

## Decision

1. **What the core may not call.** File and folder access; environment
   and home-folder reads; starting a process; finding, loading or mapping
   fonts; the system locale; clocks (`Instant::now`, `SystemTime::now`, a
   thread's CPU time); starting threads; and `cfg(target_os)` branches —
   platform knowledge lives with the platform. Allowed: allocation, locks,
   atomics and `OnceLock`, and what is fixed when the code is compiled
   (`env!`, `std::env::consts::OS`).
2. **An ask for what someone sees, a call for what the core needs now.**
   What the app or the user sees happen — a window opening, a file shown
   in its folder, a frame captured, a file written ([draft ADR
   I](the-cores-io-asks.md)) — is queued as data for the driver to
   apply. What the core needs answered to do its own work — a font's
   data, a clock, the writer its own trace streams into — is a direct
   call to the platform.
3. **One trait, held by the Session.** `kui_core::Platform: Send + Sync`
   answers `fonts` (the scan: locale, a database of mapped faces, the
   generic families' pins), `font_file`, `fonts_in_dir`, `now_us`,
   `thread_cpu_us`, `wall_ms` and `create_file` (a `Box<dyn Write +
   Send>`), plus the capture folder. A Session holds an
   `Arc<dyn Platform>`; every core of it reads `session.platform` at the
   handful of places it touches the system. `Session::with_platform(p)`
   and `Core::with_platform(p)` set it; there is no process-wide global,
   no `install()`, nothing to call before `Core::new()`.
4. **A bare platform by default.** `Session::new()` and `Core::new()` get
   `Bare`: no system fonts (fonts added as bytes still shape), clocks at
   0, `create_file` an error naming the missing layer, and one diagnostic
   the first time a session asks it for fonts — "no system layer: make
   the core with `kui_native::system::platform()` for the system's
   fonts". One impl in one file; no call site checks anything. Under
   `cfg(test)` the default is the test platform of decision 10 instead.
5. **`kui_native::system`, and a `gui` feature.** One folder:
   - `fonts` — the scan and its process-wide cache, mapping, the
     generic families per platform, and (under `gui`) the font-change
     listener from `system_fonts.rs`;
   - `env` — today's `system_env.rs` (accent, reduce motion, locale) and
     the `KUI_*` variables;
   - `shell` — `show_in_folder` / `open_external`;
   - `files`, `clock`, and `mod.rs` implementing the trait.

   kui-native's runner body moves out of `lib.rs` into a `gui` module
   behind a default `gui` feature (winit, wgpu, kira, accesskit, arboard
   and rfd optional under it), so `lib.rs` is the module list,
   `pub mod system` and one `cfg`. kui-ffi depends on kui-native always,
   `default-features = false`, its `runner` feature turning on
   `kui-native/gui`; kui-lua turns `gui` off. CI checks
   `cargo check -p kui-native --no-default-features`.
6. **Whoever makes the Session passes the platform.** kui-native's runner
   where it makes its Session; kui-ffi's core and session constructors in
   both builds; Node's and Lua's few construction sites; the Kit runner.
   A Rust app on kui-native never sees it.
7. **Fonts.** The system module scans, caches the scan per process, and
   maps every file once during it — address space, not resident memory —
   reading the locale from `system::env`. The core keeps the
   measurable-face check and the generic families' pinning, single
   threaded over mapped memory, so the `kui-font-check` threads, the
   lazy `faces_shared` step and `share_faces`' `unsafe` go.
   `load_font_file` and `load_fonts_dir` ask the platform for mapped data;
   `apply_system_fonts` takes what came from the fresh scan's sources
   rather than reopening paths. cosmic-text in the core's manifest drops
   `fontconfig` (its `std` still brings fontdb's `fs`, `memmap2` and
   `sys-locale` into the graph, which is why decision 11 is a lint).
   Every binding's font door keeps its name and its signature. If the
   first `Core::new()` on a Mac gets slower than today's 20–70 ms, or
   resident memory grows, the module maps each file when it is first
   read instead.
8. **The perf recorder.** It streams into the writer `create_file`
   hands it — a writer that may do its own writing on a thread — and
   writes a converted format through it on flush and on drop. Its epoch,
   its spans' CPU time and its session's wall time come from the
   platform; `libc` leaves kui-core. `perf_from_env` moves to kui-native.
9. **The devtools.** kui-native reads `KUI_DEVTOOLS`,
   `KUI_SMOKE_FRAMES` and `KUI_PERF_TRACE` and hands the core the values.
   `capture_path` takes its folder (`KUI_CAPTURE_DIR`, else the desktop,
   else the temporary folder — resolved in `system::env`) and its time
   from the platform; the date stamp stays in the core, pure. "Save
   trace" writes through `create_file` until draft ADR I's `write_file`
   replaces it.
10. **Tests run on a fake.** kui-core's tests get a test platform that
    serves generated faces: `liga_font`'s generator grown into three
    families, sans, serif and mono, over printable ASCII — the same on
    every machine. A test that needs the real fonts (script fallback,
    a named family such as Menlo) makes its core with
    `Core::with_platform(testing::system_fonts())`, a scan under
    `cfg(test)`. Other crates' tests make theirs with
    `kui_core::testing::platform()`, or `kui_native::system::platform()`
    where they need the real ones. The first build step runs the suite
    on the fake and lists what breaks before anything is moved for it.
11. **A lint and a grep keep it.** `crates/kui-core/clippy.toml`
    disallows `std::fs::*`; `env::{var, var_os, vars, temp_dir,
    home_dir, current_dir}`; `process::Command`; `thread::{spawn, scope,
    Builder}`; `Instant::now` and `SystemTime::now`; fontdb's
    `load_system_fonts`, `load_fonts_dir`, `load_font_file` and
    `make_shared_face_data`; and cosmic-text's `FontSystem::new` and
    `new_with_fonts` — each with a `reason` that names this ADR (by its number once it has one) and says
    what to call instead. It is allowed under `cfg(test)`. CI's existing
    `clippy --workspace --all-targets -D warnings` enforces it, and a CI
    step fails on `cfg(target_os` or `Source::File(` in kui-core's
    non-test code, printing `file:line`, the text, and the same pointer.

## Considered options

- **A crate of its own, `kui-system`,** that kui-native re-exports and
  headless kui-ffi depends on. The user wanted the layer inside
  kui-native; the `gui` feature gives the headless build the same small
  dependency set.
- **kui-ffi depending on all of kui-native.** The headless cdylib goes
  from 5.1 MB to about 12.3 MB for a scan.
- **No system layer for headless C.** Its host would hand every font in
  as bytes, and `kui_font_load_file` would fail there.
- **A process-wide platform installed once.** Simpler at first, but a
  per-core override is wanted anyway, a test cannot differ from the next
  without a thread-local, and two ways to set it are one too many.
- **Panicking when nothing is installed.** Every crate whose tests make a
  Core without one would panic, for the same few lines `Bare` takes.
- **Everything as an ask.** Shaping and timing cannot wait a frame.
- **Moving every test onto bundled fonts by hand.** The fake does it
  once, and the opt-in keeps the few that need real fonts honest.

## Consequences

- **A break for Rust code that makes a bare core.** `kui_core::Core::new()`
  has no system fonts; such code makes its core with
  `Core::with_platform(kui_native::system::platform())`. In the
  changelog, the book's headless chapter and `Core::new`'s doc.
- **A diagnostic row** in `props.md` for decision 4's "no system layer".
- **No door, signature or ABI changes.** The font, perf and devtools
  doors keep their names in every binding, and DOORS is untouched.
- **kui-native gains a `gui` feature.** The runner body moves to a
  `gui` module, which git records as a move. kui-ffi always depends on
  kui-native, `default-features = false` for the headless build.
- **To measure before it lands:** the first `Core::new()`'s time and
  resident memory on the Mac, the headless cdylib's size, and the frame
  benches (no per-frame call is added).
- **The F203 rule is restated as decision 2** — asks for what is seen,
  calls for what is needed now — in the memory that recorded it.

## Build order

On main after Kit lands, a commit each:

1. The trait, `Bare`, the test platform, `with_platform`.
2. `kui_native::system` and the `gui` split.
3. Fonts.
4. Perf and clocks.
5. Devtools.
6. Lint and grep.
7. Measurements, `test.nu`, the smoke round, the changelog and the book.
