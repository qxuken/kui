//! Loading a C **extension**: a shared library that draws into a frame the
//! host owns, keeps its own state, and gets its own events back.
//!
//! The mirror of the rest of this crate. There, C is the host and kui is the
//! library it links; here a Rust host runs the window and the C library is
//! the guest, sharing one frame with the host's own view - the same deal
//! `kui_lua::LuaExtension` gets, over the same [`Extension`] trait, with a
//! `.so` in place of a script.
//!
//! # The plugin's side
//!
//! Eight symbols, two of them required - `kui_ext_abi` and `kui_ext_view` -
//! and six optional (see `include/kui.h`):
//!
//! ```c
//! uint32_t    kui_ext_abi(void);                        /* required: KUI_ABI_VERSION */
//! const char *kui_ext_name(void);                       /* else: file stem */
//! void       *kui_ext_init(void);                       /* else: NULL      */
//! const KuiStr *kui_ext_components(size_t *count);     /* else: "root"    */
//! const KuiContribution *kui_ext_contributions(size_t *count); /* else: none */
//! void        kui_ext_view(void *user, KuiCtx *ctx);    /* required        */
//! void        kui_ext_on_event(void *user, const KuiEvent *ev);
//! void        kui_ext_free(void *user);
//! ```
//!
//! `kui_ext_abi` is required rather than merely checked when present: the
//! plugin most likely to lack it is one built against a header from before
//! the symbol existed, which is exactly the mismatched plugin (older
//! plugin, newer host) the check exists to refuse. Absence is refused the
//! way a mismatch is, before anything else is looked up.
//!
//! `kui_ext_view` receives a context borrowing the host's frame and calls the
//! ordinary `kui_open`/`kui_text`/`kui_close` builders on it. Everything it
//! opens is tagged with the origin the runner assigned this extension, which
//! is what routes its clicks back to `kui_ext_on_event` and keeps the host
//! from ever seeing them. The context is alive for that one call: nothing
//! inside it may be stored across frames, and the input and draw entry points
//! do not apply to it - the runner drives those.
//!
//! # Where the `kui_*` symbols come from
//!
//! On ELF and Mach-O the plugin links against nothing. It leaves the whole
//! API undefined and resolves it from the host executable at `dlopen` time,
//! exactly as a Lua C module resolves `lua_*` from the interpreter that
//! loaded it. That costs the host one linker flag - `--export-dynamic`,
//! without which its symbols are in the binary but not in the dynamic
//! symbol table the loader reads - and this crate's `build.rs` passes it
//! for the examples here. A host outside this crate passes its own;
//! `cargo run -p kui-devtools --bin cbuild` builds the plugin side.
//!
//! Windows cannot work that way: a DLL may not leave an import unresolved,
//! so the plugin names the module each `kui_*` comes from and takes that
//! name from an import library. Two shapes, and both are fine:
//!
//! * against the **host's** import library, which is what `build.rs` makes
//!   for the examples here by exporting the host's `kui_*`. One copy of the
//!   library in the process, as on ELF, and the plugin loads into that host
//!   and no other.
//! * against **`kui_ffi.dll`**, which is the ordinary Windows plugin shape
//!   (a Python extension imports from `python313.dll`, not from
//!   `python.exe`) and gives a plugin binary that loads into any host
//!   shipping that DLL.
//!
//! The second used to be broken, and the fix is what ABI 10 is: two copies
//! of this library in one process is fine for everything that travels
//! through a pointer the host hands over - a `KuiCtx` is a `KuiCtx` - and
//! was fine for nothing that lived in a `static`. The reply sink was the
//! only such thing, and it now rides on the event as a function pointer
//! into whichever copy opened it (see [`crate::KuiReplySink`]); `kui_emit`
//! answers through it. The loader
//! below is the same either way.

use std::ffi::{CStr, CString, c_char, c_void};
use std::path::Path;

use kui_core::{Contribution, Extension, Instance, Ui, UiEvent, Value};

use crate::convert::kstr;
use crate::{KUI_ABI_VERSION, KuiContribution, KuiCtx, KuiEvent, KuiStr, KuiValue};

/// Opens the extension at `path` the way every host's door does (ADR
/// 0059): a `.lua` file, or a directory holding `init.lua`, as a
/// [`kui_lua::LuaExtension`]; anything else as a C library
/// ([`CExtension::open`]). What `kui_ctx_add_extension` and Node's
/// `addExtension` call. Registers kui-ffi's C loader with kui-lua on the
/// way ([`register_native_loader`]), so a script loaded here can load a C
/// plugin of its own with `env.add_extension`.
///
/// # Safety
/// A C library's entry points run on the host's frame in the host's
/// process: loading one is trusting it as much as linking it would be. A
/// script is only as trusted as what it may load, which is the same.
pub unsafe fn open_extension(path: impl AsRef<Path>) -> Result<Box<dyn Extension>, String> {
    register_native_loader();
    let path = path.as_ref();
    if kui_lua::is_lua_path(path) {
        return kui_lua::LuaExtension::from_path(path)
            .map(|ext| Box::new(ext) as Box<dyn Extension>)
            .map_err(|e| e.to_string());
    }
    // SAFETY: the caller's.
    unsafe { CExtension::open(path) }.map(|ext| Box::new(ext) as Box<dyn Extension>)
}

/// Hands kui-ffi's C loader to kui-lua, for a script's `env.add_extension`.
/// [`open_extension`] calls it; a Rust host that builds a
/// [`kui_lua::LuaExtension`] itself, and whose script loads a C plugin,
/// calls it once first. The first registration wins, and this is the only
/// one there is, so calling it again is free.
pub fn register_native_loader() {
    // SAFETY: a script naming a C plugin trusts it as its host trusts the
    // script; see `open_extension`.
    kui_lua::set_native_loader(|path| {
        unsafe { CExtension::open(path) }.map(|ext| Box::new(ext) as Box<dyn Extension>)
    });
}

type ViewFn = extern "C" fn(*mut c_void, *mut KuiCtx);
type EventFn = extern "C" fn(*mut c_void, *const KuiEvent);

/// A C shared library loaded as a guest extension of a Rust host.
///
/// The plugin exports `kui_ext_abi` and `kui_ext_view` (and optionally
/// `kui_ext_name`, `kui_ext_init`, `kui_ext_components`,
/// `kui_ext_contributions`, `kui_ext_on_event` and `kui_ext_free`;
/// `include/kui.h` has the prototypes). Each time the
/// host's `Ui` places one of the plugin's components (its `root`, after
/// the host's view, when it provides nothing else), `kui_ext_view` is
/// called with a context borrowing that frame and builds into it with the
/// ordinary `kui_open` / `kui_text` / `kui_close` calls; events from the
/// nodes it built go to `kui_ext_on_event`, never to the host, and what
/// it raises with `kui_emit` reaches whoever placed the instance.
///
/// It implements [`Extension`], so it plugs into `kui_native::app(..)
/// .extension_as(namespace, ext)` or a `kui_core::Extensions` list like
/// any other extension. On ELF and Mach-O the plugin links against nothing
/// and resolves `kui_*` from the host executable at load, which needs the
/// host linked with `--export-dynamic` (this crate's `build.rs` does it
/// for the examples); on Windows the plugin links against the host's
/// import library or against `kui_ffi.dll`.
///
/// ```rust,no_run
/// use kui_ffi::CExtension;
///
/// // SAFETY: the plugin's code runs in this process; loading it is
/// // trusting it as much as linking it would be.
/// let ext = unsafe { CExtension::open("target/debug/panel.so") }?;
/// let launcher = kui_native::app("host").extension_as("todos", ext);
/// # let _ = launcher;
/// # Ok::<(), String>(())
/// ```
pub struct CExtension {
    name: String,
    handle: *mut c_void,
    /// Whatever `kui_ext_init` returned, handed back to every entry point.
    user: *mut c_void,
    view: ViewFn,
    on_event: Option<EventFn>,
    free: Option<extern "C" fn(*mut c_void)>,
    /// What `kui_ext_components` returned at load: the components it
    /// provides (ADR 0060), each drawn by `kui_ext_view` with the
    /// component's name as `kui_instance_name`. Empty means `"root"` alone.
    components: Vec<String>,
    /// What `kui_ext_contributions` returned at load: the components it
    /// adds to contribution slots (ADR 0067), copied out. Empty when it
    /// contributes nothing.
    contributions: Vec<Contribution>,
}

impl CExtension {
    /// Loads the shared library at `path` and runs its `kui_ext_init`.
    ///
    /// Fails, with the reason as the error, if the library will not load,
    /// if it declares no `kui_ext_abi` or one other than this build's
    /// [`KUI_ABI_VERSION`], or if it has no `kui_ext_view`. The plugin's
    /// `kui_ext_free` runs and the library is unloaded when the
    /// `CExtension` is dropped.
    ///
    /// # Safety
    /// The library's entry points are called on the host's frame and its
    /// code runs in the host's process: loading one is trusting it exactly
    /// as much as linking it would be.
    pub unsafe fn open(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();
        let cpath = sys::path_arg(path)
            .ok_or_else(|| format!("{}: path contains a NUL", path.display()))?;

        let handle = unsafe { sys::load(&cpath) };
        if handle.is_null() {
            return Err(format!("{}: {}", path.display(), sys::last_error()));
        }

        // A plugin built against a header this build has outgrown reads the
        // structs it writes at the wrong offsets. Same check `include/kui.h`
        // asks a C host to make, made for it - and a plugin with no
        // `kui_ext_abi` at all is that case, not a lenient one: the header
        // that predates the symbol is a header this build has outgrown.
        let mut ext = Self {
            name: String::new(),
            handle,
            user: std::ptr::null_mut(),
            // Placeholder: replaced below, before anything can call it.
            view: placeholder_view,
            on_event: None,
            free: None,
            components: Vec::new(),
            contributions: Vec::new(),
        };
        let Some(abi) = (unsafe { ext.sym::<extern "C" fn() -> u32>("kui_ext_abi") }) else {
            return Err(format!(
                "{}: plugin declares no ABI; this build is {KUI_ABI_VERSION}",
                path.display()
            ));
        };
        let claimed = abi();
        if claimed != KUI_ABI_VERSION {
            return Err(format!(
                "{}: plugin is ABI {claimed}, this build is {KUI_ABI_VERSION}",
                path.display()
            ));
        }

        let Some(view) = (unsafe { ext.sym::<ViewFn>("kui_ext_view") }) else {
            return Err(format!("{}: no kui_ext_view", path.display()));
        };
        ext.view = view;
        ext.on_event = unsafe { ext.sym::<EventFn>("kui_ext_on_event") };
        ext.free = unsafe { ext.sym::<extern "C" fn(*mut c_void)>("kui_ext_free") };

        // A plugin from before ADR 0067 lists slots: say what to export now
        // rather than load it and draw nothing.
        if unsafe { ext.sym::<extern "C" fn(*mut usize) -> *const KuiStr>("kui_ext_slots") }
            .is_some()
        {
            return Err(format!(
                "{}: kui_ext_slots is gone (ADR 0067); export kui_ext_components, or nothing to \
                 draw kui_ext_view as the root",
                path.display()
            ));
        }

        // The components it provides, read once: the plugin keeps the array
        // alive for its own lifetime, and the names are copied out so
        // nothing here reads it again (ADR 0060).
        if let Some(components) =
            unsafe { ext.sym::<extern "C" fn(*mut usize) -> *const KuiStr>("kui_ext_components") }
        {
            let mut count = 0usize;
            let p = components(&mut count);
            if !p.is_null() {
                let names = unsafe { std::slice::from_raw_parts(p, count) };
                ext.components = names.iter().map(|s| kstr(*s).into_owned()).collect();
            }
        }

        // What it contributes to slots, read once on the same terms: the
        // array is the plugin's, the strings copied out. Whether each names
        // a component it provides is `Extensions::push_as`'s check, made
        // for every kind of extension alike.
        if let Some(contributions) = unsafe {
            ext.sym::<extern "C" fn(*mut usize) -> *const KuiContribution>("kui_ext_contributions")
        } {
            let mut count = 0usize;
            let p = contributions(&mut count);
            if !p.is_null() {
                let rows = unsafe { std::slice::from_raw_parts(p, count) };
                ext.contributions = rows
                    .iter()
                    .map(|c| Contribution::new(kstr(c.slot), kstr(c.component)).order(c.order))
                    .collect();
            }
        }

        ext.name = match unsafe { ext.sym::<extern "C" fn() -> *const c_char>("kui_ext_name") } {
            Some(f) => {
                let p = f();
                if p.is_null() {
                    String::new()
                } else {
                    unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
                }
            }
            None => String::new(),
        };
        if ext.name.is_empty() {
            ext.name = path.file_stem().map_or_else(
                || path.display().to_string(),
                |s| s.to_string_lossy().into_owned(),
            );
        }

        // Last, so a plugin that allocates in init only does so once every
        // other check has passed and `free` is already wired up to undo it.
        if let Some(init) = unsafe { ext.sym::<extern "C" fn() -> *mut c_void>("kui_ext_init") } {
            ext.user = init();
        }
        Ok(ext)
    }

    /// # Safety
    /// `T` must be the signature the plugin defines the symbol with.
    unsafe fn sym<T>(&self, name: &str) -> Option<T> {
        debug_assert_eq!(size_of::<T>(), size_of::<*mut c_void>());
        let cname = CString::new(name).ok()?;
        let p = unsafe { sys::symbol(self.handle, &cname) };
        // Transmuting a data pointer to a function pointer is not something
        // Rust sanctions, but it is what dlsym is for.
        (!p.is_null()).then(|| unsafe { std::mem::transmute_copy::<*mut c_void, T>(&p) })
    }
}

extern "C" fn placeholder_view(_user: *mut c_void, _ctx: *mut KuiCtx) {}

/// The three calls that differ per platform. Everything above is the same
/// on both.
#[cfg(unix)]
mod sys {
    use super::{CStr, CString, c_char, c_void};

    unsafe extern "C" {
        fn dlopen(path: *const c_char, flags: i32) -> *mut c_void;
        fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
        fn dlclose(handle: *mut c_void) -> i32;
        fn dlerror() -> *const c_char;
    }

    /// Resolve everything now, so a plugin missing a `kui_*` symbol fails at
    /// load with a name in the message instead of at the first frame that
    /// reaches it.
    const RTLD_NOW: i32 = 2;
    /// Keep the plugin's own symbols out of the global namespace: two plugins
    /// both defining `kui_ext_view` are the normal case, not a collision.
    ///
    /// Not a shared number, and the mistake is quiet: 4 is `RTLD_LOCAL` on
    /// Apple's dyld, where it has to be passed because the default there is
    /// `RTLD_GLOBAL`, and `RTLD_NOLOAD` on glibc, where local is already the
    /// default. Passing Apple's value on Linux asks for a handle only if the
    /// library is *already* loaded, which it is not, so every load fails.
    #[cfg(target_vendor = "apple")]
    const RTLD_LOCAL: i32 = 4;
    #[cfg(not(target_vendor = "apple"))]
    const RTLD_LOCAL: i32 = 0;

    /// What [`path_arg`] produces and [`load`] takes: the platform's own
    /// spelling of a path, since the two do not agree on one.
    pub type PathArg = CString;

    /// # Safety
    /// Runs the library's initializers.
    pub unsafe fn load(path: &PathArg) -> *mut c_void {
        // Clear any stale error first: dlerror() is only meaningful right
        // after a failed dl* call, and a previous one's message lingers.
        unsafe { dlerror() };
        unsafe { dlopen(path.as_ptr(), RTLD_NOW | RTLD_LOCAL) }
    }

    /// # Safety
    /// `handle` must come from [`load`] and still be open.
    pub unsafe fn symbol(handle: *mut c_void, name: &CStr) -> *mut c_void {
        unsafe { dlsym(handle, name.as_ptr()) }
    }

    /// # Safety
    /// Nothing may call into the library afterwards.
    pub unsafe fn unload(handle: *mut c_void) {
        unsafe { dlclose(handle) };
    }

    pub fn last_error() -> String {
        let e = unsafe { dlerror() };
        if e.is_null() {
            "unknown error".into()
        } else {
            unsafe { CStr::from_ptr(e) }.to_string_lossy().into_owned()
        }
    }

    /// The path as `dlopen` wants it. A path with an interior NUL cannot be
    /// spelled and is refused here rather than truncated.
    pub fn path_arg(path: &std::path::Path) -> Option<PathArg> {
        CString::new(path.as_os_str().as_encoded_bytes()).ok()
    }
}

#[cfg(windows)]
mod sys {
    use super::{CStr, c_char, c_void};

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn LoadLibraryExW(path: *const u16, file: *mut c_void, flags: u32) -> *mut c_void;
        fn GetProcAddress(handle: *mut c_void, name: *const c_char) -> *mut c_void;
        fn FreeLibrary(handle: *mut c_void) -> i32;
        fn GetLastError() -> u32;
    }

    /// What [`path_arg`] produces and [`load`] takes: `LoadLibraryW`'s own
    /// UTF-16, NUL-terminated.
    ///
    /// It used to be a `CString` with the code units packed into its bytes,
    /// which could never have worked: every ASCII character puts a zero
    /// byte in its pair, so `CString::new` refused every path there has
    /// ever been and no plugin could load on Windows at all. Found by
    /// running the workspace tests on Windows for the first time on
    /// 2026-09-08 — `ext.rs`'s own test says so now on the platform, and
    /// said "path contains a NUL" for `kernel32.dll`.
    pub type PathArg = Vec<u16>;

    /// `LOAD_WITH_ALTERED_SEARCH_PATH`: search the *plugin's* own directory
    /// for the DLLs it imports, before the process directory and PATH.
    ///
    /// Without it a plugin is looked up from `node.exe`'s directory or the
    /// host exe's, which is not where a plugin's siblings live — and on
    /// Windows a plugin does import something, since a DLL may not leave a
    /// symbol undefined. A panel next to the `kui_ffi.dll` it was linked
    /// against failed to load with error 126 until this flag, which is how
    /// it was found. It only applies to an absolute path, which is why
    /// [`CExtension::open`] makes one.
    const LOAD_WITH_ALTERED_SEARCH_PATH: u32 = 0x8;

    /// # Safety
    /// Runs the library's initializers.
    pub unsafe fn load(path: &PathArg) -> *mut c_void {
        unsafe {
            LoadLibraryExW(
                path.as_ptr(),
                std::ptr::null_mut(),
                LOAD_WITH_ALTERED_SEARCH_PATH,
            )
        }
    }

    /// # Safety
    /// `handle` must come from [`load`] and still be open.
    pub unsafe fn symbol(handle: *mut c_void, name: &CStr) -> *mut c_void {
        unsafe { GetProcAddress(handle, name.as_ptr()) }
    }

    /// # Safety
    /// Nothing may call into the library afterwards.
    pub unsafe fn unload(handle: *mut c_void) {
        unsafe { FreeLibrary(handle) };
    }

    pub fn last_error() -> String {
        format!("LoadLibraryW failed (GetLastError {})", unsafe {
            GetLastError()
        })
    }

    /// The path as `LoadLibraryW` wants it: UTF-16 with a terminator, and
    /// absolute, because [`LOAD_WITH_ALTERED_SEARCH_PATH`] is ignored for a
    /// relative one and that flag is what lets a plugin find the DLLs it
    /// was linked beside. A path that does not resolve keeps its spelling —
    /// a bare `kernel32.dll` is a name for the loader to search, not a file
    /// in the working directory, and canonicalizing it would be wrong.
    /// Only here: on unix a bare `libc.so.6` is a name for `dlopen` to
    /// search too, and resolving it against the working directory first
    /// could load a different library than the one meant.
    ///
    /// A path with an interior NUL is refused, since `LoadLibraryW` would
    /// stop there and open something the caller did not name.
    pub fn path_arg(path: &std::path::Path) -> Option<PathArg> {
        use std::os::windows::ffi::OsStrExt;
        let abs = std::fs::canonicalize(path);
        let path = abs.as_deref().unwrap_or(path);
        let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
        if wide.contains(&0) {
            return None;
        }
        wide.push(0);
        Some(wide)
    }
}

impl Extension for CExtension {
    fn name(&self) -> &str {
        &self.name
    }

    fn components(&self) -> &[String] {
        &self.components
    }

    fn contributions(&self) -> &[Contribution] {
        &self.contributions
    }

    fn view(&mut self, slot: &Instance<'_>, ui: &mut Ui<'_>) -> Result<(), String> {
        // Borrows the host's frame for this call only - the plugin builds
        // into the same tree the host just built into, under its own origin.
        // Which instance, and with what, rides on the context:
        // `kui_instance_name` and `kui_instance_params` read it back,
        // borrowed for the call like an event's payload (one clone of the
        // props per fill, the C side's cost, which is what `on_event`'s
        // payload already pays). `borrowing_in` rather than `borrowing`,
        // for the same reason the runner's host callback uses it: a
        // plugin's `kui_component` reaches the frame's filler through this
        // pointer, so a plugin may place what an extension the host loaded
        // provides (`kui_core::slot`). It cannot load one itself — a
        // plugin's context has no list of its own.
        let mut ctx = instance_ctx(slot, ui);
        (self.view)(self.user, &mut ctx);
        Ok(())
    }

    fn on_event(&mut self, ev: &UiEvent) -> Vec<Value> {
        let Some(cb) = self.on_event else {
            return Vec::new();
        };
        // Borrowed for the duration of the callback, like every other
        // payload C sees.
        let payload = KuiValue(ev.payload.clone());
        let mut out = KuiEvent {
            origin: ev.origin.0,
            key: ev.key.0,
            payload: &payload,
            window: ev.window.0,
            instance: ev.instance.map_or(0, |k| k.0),
            event: KuiStr::from_static(ev.event.name()),
            // Borrowed from `ev` for the callback, as the payload is.
            from: ev
                .from
                .as_deref()
                .map_or(KuiStr::from_static(""), KuiStr::borrowed),
            ..Default::default()
        };
        // Emits: the plugin calls `kui_emit(ev, prop, fields)` during the
        // callback, as often as it likes, and the sink open around the
        // call collects them for the host.
        crate::slots::collect_replies(&mut out, |ev| cb(self.user, ev))
    }
}

/// The context a C extension's `kui_ext_view` is handed for one instance:
/// borrowing `ui`'s frame, and carrying which instance it is — what
/// `kui_instance_name`, `_namespace`, `_params` and `_slot` answer. Valid
/// for the call that made it, like the borrow.
pub(crate) fn instance_ctx(inst: &Instance<'_>, ui: &mut Ui<'_>) -> KuiCtx {
    let mut ctx = KuiCtx::borrowing_in(ui);
    ctx.slot_name = Some(inst.name.to_owned());
    ctx.slot_namespace = Some(inst.namespace.to_owned());
    ctx.slot_params = (!matches!(inst.params, Value::Null)).then(|| KuiValue(inst.params.clone()));
    ctx.instance_slot = inst.slot.map(str::to_owned);
    ctx
}

impl Drop for CExtension {
    fn drop(&mut self) {
        if let Some(free) = self.free {
            free(self.user);
        }
        // After `free`, so the plugin's own code is still mapped when it runs.
        unsafe { sys::unload(self.handle) };
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A shared library that loads on every target this crate builds for
    /// and defines no `kui_ext_*` symbol at all: the platform's own C
    /// runtime. That is the shape of a plugin built against a header from
    /// before `kui_ext_abi` existed - the case the check exists to
    /// refuse - reached without a C compiler in the test. The real mutant,
    /// `examples/c/features/slots/panel.c` with its `kui_ext_abi` line deleted, is built by
    /// `cbuild` and driven through `c_panel --headless` in CI.
    pub(crate) fn a_library_with_no_kui_symbols() -> &'static str {
        if cfg!(target_vendor = "apple") {
            // Not a file on disk since the dyld shared cache, but dlopen by
            // this path resolves it from the cache.
            "/usr/lib/libSystem.B.dylib"
        } else if cfg!(windows) {
            "kernel32.dll"
        } else if cfg!(target_env = "musl") {
            "libc.so"
        } else {
            "libc.so.6"
        }
    }

    #[test]
    fn a_plugin_without_kui_ext_abi_is_refused_before_anything_else() {
        let path = a_library_with_no_kui_symbols();
        // SAFETY: the library is the process's own C runtime, already loaded.
        let err = match unsafe { CExtension::open(path) } {
            Ok(ext) => panic!("{path} loaded as a plugin named {:?}", ext.name()),
            Err(err) => err,
        };
        // The whole message, not a substring: a library with no kui symbols
        // also has no `kui_ext_view`, and the ABI refusal has to be the one
        // that wins - it is the check the header's own text promises.
        assert_eq!(
            err,
            format!("{path}: plugin declares no ABI; this build is {KUI_ABI_VERSION}")
        );
    }
}
