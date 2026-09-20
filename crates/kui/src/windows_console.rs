//! The terminal a windowed app was launched from, given back to it.
//!
//! A Windows binary is built for one of two subsystems. A *console* one
//! gets a console the moment it starts — the black window that pops up
//! beside the app's own when it is opened from the Explorer, before a
//! line of the app has run — and a *windows* one gets none, which is the
//! only way not to have that window: it is made by the loader, so
//! nothing the app does at runtime can prevent it, only close it after it
//! has been seen. An app says which with one line in its binary crate,
//!
//! ```ignore
//! #![cfg_attr(windows, windows_subsystem = "windows")]
//! ```
//!
//! and pays for it in the terminal: a windows-subsystem process started
//! from a shell has no standard handles at all, so its `println!`, its
//! panics, the runner's `kui:` lines and `kui_wgpu::report_faults` all go
//! nowhere. This module gives those back. [`attach_parent`] asks for the
//! console of the process that launched this one (`AttachConsole`), once,
//! when the process has no output handle — a windows-subsystem app under
//! a shell, which is the case; from the Explorer there is no parent
//! console, the call fails, and nothing is shown. A process that already
//! has handles — a console build, or one whose launcher piped its output
//! (cargo, the smoke round) — is left with what it was given.
//!
//! What it does not change: the shell does not wait for a windowed
//! process, so the prompt is back before the app's first line, which
//! lands after it; and Ctrl+C in that terminal reaches the app as it
//! would a console one.

use std::sync::Once;
use std::sync::atomic::{AtomicBool, Ordering};

/// Attaches the process to its parent's console when it has no standard
/// output of its own; see the module doc. Once per process. Returns
/// whether it attached: `false` for a process that had handles already
/// (the common case for a console build) and for one with no parent
/// console to attach to.
pub(super) fn attach_parent() -> bool {
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::System::Console::{
        ATTACH_PARENT_PROCESS, AttachConsole, GetStdHandle, STD_ERROR_HANDLE, STD_OUTPUT_HANDLE,
    };
    static ONCE: Once = Once::new();
    static ATTACHED: AtomicBool = AtomicBool::new(false);
    ONCE.call_once(|| {
        let has = |id| {
            let h = unsafe { GetStdHandle(id) };
            h != 0 && h != INVALID_HANDLE_VALUE
        };
        // Either handle set means a parent chose where this process
        // writes — a pipe, a file, its own console handed down — and
        // that choice stands.
        if has(STD_OUTPUT_HANDLE) || has(STD_ERROR_HANDLE) {
            return;
        }
        // Sets the standard handles to the parent's console on success;
        // Rust's stdout and stderr ask for the handle at every write, so
        // the next `println!` lands there. On failure (no parent console)
        // the handles stay null and every write is the no-op it was.
        let ok = unsafe { AttachConsole(ATTACH_PARENT_PROCESS) } != 0;
        ATTACHED.store(ok, Ordering::Relaxed);
    });
    ATTACHED.load(Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A test binary is a console build with its handles set — by the
    /// harness's capture or the terminal — so the attach declines, and
    /// its output still goes where the harness put it.
    #[test]
    fn a_process_with_handles_keeps_them() {
        use windows_sys::Win32::System::Console::{GetStdHandle, STD_ERROR_HANDLE};
        let before = unsafe { GetStdHandle(STD_ERROR_HANDLE) };
        assert!(!attach_parent());
        assert!(!attach_parent(), "once per process, and the answer is kept");
        assert_eq!(unsafe { GetStdHandle(STD_ERROR_HANDLE) }, before);
        eprintln!("still here");
    }
}
