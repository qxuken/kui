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
//! Seven symbols, two of them required - `kui_ext_abi` and `kui_ext_view` -
//! and five optional (see `include/kui.h`):
//!
//! ```c
//! uint32_t    kui_ext_abi(void);                        /* required: KUI_ABI_VERSION */
//! const char *kui_ext_name(void);                       /* else: file stem */
//! void       *kui_ext_init(void);                       /* else: NULL      */
//! const KuiStr *kui_ext_slots(size_t *count);          /* else: "root"    */
//! void        kui_ext_view(void *user, KuiCtx *ctx);    /* required        */
//! void        kui_ext_on_event(void *user, const KuiEvent *ev);
//! void        kui_ext_free(void *user);
//! ```
//!
//! `kui_ext_abi` is required rather than merely checked when present because
//! the plugin most likely to lack it is one built against a header from
//! before ADR 0006 introduced it - which is precisely the mismatched plugin
//! the check exists to refuse, in the direction (older plugin, newer host)
//! that corrupts memory rather than merely missing a feature. Absence is
//! refused the way a mismatch is, before anything else is looked up.
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
//! The plugin links against nothing. It leaves the whole API undefined and
//! resolves it from the host executable at `dlopen` time, exactly as a Lua C
//! module resolves `lua_*` from the interpreter that loaded it. That costs
//! the host one linker flag - `--export-dynamic`, without which its symbols
//! are in the binary but not in the dynamic symbol table the loader reads -
//! and this crate's `build.rs` passes it for the examples here. A host
//! outside this crate passes its own; `examples/c/build.sh` builds the
//! plugin side.
//!
//! Windows works differently, and the difference is in the linking rather
//! than here: an .exe exports nothing without an import library, so a plugin
//! there links `kui_ffi.dll` and the host has to be built against that same
//! DLL instead of the static library - otherwise each ends up with its own
//! copy of the library and the `KuiCtx` one hands the other means nothing.
//! The loader below is the same either way.

use std::ffi::{CStr, CString, c_char, c_void};
use std::path::Path;

use kui_core::{Extension, Slot, Ui, UiEvent, Value};

use crate::convert::kstr;
use crate::{KUI_ABI_VERSION, KuiCtx, KuiEvent, KuiStr, KuiValue};

type ViewFn = extern "C" fn(*mut c_void, *mut KuiCtx);
type EventFn = extern "C" fn(*mut c_void, *const KuiEvent);

/// A `dlopen`ed C extension, plugged into a Rust host with
/// `kui::run(title, app, vec![Box::new(ext)])`.
pub struct CExtension {
    name: String,
    handle: *mut c_void,
    /// Whatever `kui_ext_init` returned, handed back to every entry point.
    user: *mut c_void,
    view: ViewFn,
    on_event: Option<EventFn>,
    free: Option<extern "C" fn(*mut c_void)>,
    /// What `kui_ext_slots` returned at load, copied out: the slot names
    /// this plugin fills (ADR 0014 decision 2). Empty means `"root"`.
    slots: Vec<String>,
}

impl CExtension {
    /// Loads a plugin and runs its `kui_ext_init`.
    ///
    /// Fails if the library will not load, if it declares no ABI or one this
    /// build does not implement, or if it has no `kui_ext_view`.
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
            slots: Vec::new(),
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

        // The slots it fills, read once: the plugin keeps the array alive
        // for its own lifetime, and the names are copied out so nothing
        // here reads it again.
        if let Some(slots) =
            unsafe { ext.sym::<extern "C" fn(*mut usize) -> *const KuiStr>("kui_ext_slots") }
        {
            let mut count = 0usize;
            let p = slots(&mut count);
            if !p.is_null() {
                let names = unsafe { std::slice::from_raw_parts(p, count) };
                ext.slots = names.iter().map(|s| kstr(*s).into_owned()).collect();
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

    /// # Safety
    /// Runs the library's initializers.
    pub unsafe fn load(path: &CStr) -> *mut c_void {
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

    /// Windows wants UTF-16 and this wants bytes; the conversion lives with
    /// the platform that needs it.
    pub fn path_arg(path: &std::path::Path) -> Option<CString> {
        CString::new(path.as_os_str().as_encoded_bytes()).ok()
    }
}

#[cfg(windows)]
mod sys {
    use super::{CStr, CString, c_char, c_void};

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn LoadLibraryW(path: *const u16) -> *mut c_void;
        fn GetProcAddress(handle: *mut c_void, name: *const c_char) -> *mut c_void;
        fn FreeLibrary(handle: *mut c_void) -> i32;
        fn GetLastError() -> u32;
    }

    /// # Safety
    /// Runs the library's initializers.
    pub unsafe fn load(path: &CStr) -> *mut c_void {
        // `path_arg` packed UTF-16 into the CString's bytes; unpack it.
        let bytes = path.to_bytes();
        let mut wide: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        wide.push(0);
        unsafe { LoadLibraryW(wide.as_ptr()) }
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

    /// UTF-16 code units, little-endian, in a NUL-free byte string - a path
    /// with an interior NUL would not survive, which is what we want.
    pub fn path_arg(path: &std::path::Path) -> Option<CString> {
        use std::os::windows::ffi::OsStrExt;
        let bytes: Vec<u8> = path
            .as_os_str()
            .encode_wide()
            .flat_map(u16::to_le_bytes)
            .collect();
        CString::new(bytes).ok()
    }
}

impl Extension for CExtension {
    fn name(&self) -> &str {
        &self.name
    }

    fn slots(&self) -> &[String] {
        &self.slots
    }

    fn view(&mut self, slot: &Slot<'_>, ui: &mut Ui<'_>) -> Result<(), String> {
        // Borrows the host's frame for this call only - the plugin builds
        // into the same tree the host just built into, under its own origin.
        // Which slot, and with what, rides on the context: `kui_slot_name`
        // and `kui_slot_params` read it back, borrowed for the call like an
        // event's payload (one clone of the params per fill, the C side's
        // cost, which is what `on_event`'s payload already pays).
        let mut ctx = KuiCtx::borrowing(ui.core());
        ctx.slot_name = Some(slot.name.to_owned());
        ctx.slot_namespace = Some(slot.namespace.to_owned());
        ctx.slot_params =
            (!matches!(slot.params, Value::Null)).then(|| KuiValue(slot.params.clone()));
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
        let out = KuiEvent {
            origin: ev.origin.0,
            key: ev.key.0,
            payload: &payload,
            window: ev.window.0,
            ..Default::default()
        };
        // Replies (ADR 0014 decision 6): the plugin calls `kui_reply(ev,
        // value)` during the callback, as often as it likes, and the sink
        // open around the call collects them for the host.
        crate::slots::collect_replies(&out, || cb(self.user, &out))
    }
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
mod tests {
    use super::*;

    /// A shared library that loads on every target this crate builds for
    /// and defines no `kui_ext_*` symbol at all: the platform's own C
    /// runtime. That is the shape of a plugin built against a header from
    /// before ADR 0006 added `kui_ext_abi` - the case the check exists to
    /// refuse - reached without a C compiler in the test. The real mutant,
    /// `examples/c/panel.c` with its `kui_ext_abi` line deleted, is built by
    /// `examples/c/build.sh` and driven through `c_panel --headless` in CI.
    fn a_library_with_no_kui_symbols() -> &'static str {
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
