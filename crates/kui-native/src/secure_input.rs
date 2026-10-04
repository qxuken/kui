//! Secure keyboard entry while a window asks for it.
//!
//! macOS's `EnableSecureEventInput` stops every other process from reading
//! the keyboard — an event tap, a keylogger, and with them a launcher's
//! hotkey, a text expander, an accessibility tool — which is what a
//! terminal turns on at a password prompt. It is process-wide and
//! reference-counted: every enable must be balanced by exactly one
//! disable, or the machine's keyboard stays closed to everything else
//! until the process dies. Apple's rule is to hold it only while it is
//! needed and only while the app is active.
//!
//! So the balance is the runner's, not the app's. A frame declares the ask
//! (`Ui::secure_input`, frame state like `always_on_top`), and at the end
//! of every batch of events the runner works out whether any window whose
//! frame asked has the keyboard — its settled `env.focused`, which a popup
//! it owns holding the keyboard does not take away — and moves one
//! [`SecureInput`] to that answer. It holds at most one count however many
//! windows ask, gives it back when the asking window loses the keyboard,
//! closes or stops asking, and on teardown; and the guard gives it back on
//! drop, so an unwinding runner does not leave the keyboard closed.
//!
//! The other platforms have no such switch, and there the calls are
//! nothing.

/// The two platform calls, behind a trait so the balance can be tested
/// by counting them.
pub(crate) trait SecureInputOs {
    fn enable(&mut self);
    fn disable(&mut self);
}

/// The platform's calls: HIToolbox's on macOS, nothing elsewhere.
#[derive(Default)]
pub(crate) struct Platform;

#[cfg(target_os = "macos")]
#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
    // HIToolbox (in Carbon): `OSStatus EnableSecureEventInput(void)` and
    // its pair. Both only ever answer noErr.
    fn EnableSecureEventInput() -> i32;
    fn DisableSecureEventInput() -> i32;
}

impl SecureInputOs for Platform {
    fn enable(&mut self) {
        // SAFETY: no arguments, no state of ours; the count it takes is
        // the one `SecureInput` gives back.
        #[cfg(target_os = "macos")]
        unsafe {
            EnableSecureEventInput();
        }
    }

    fn disable(&mut self) {
        // SAFETY: only ever called by `SecureInput` for a count it holds.
        #[cfg(target_os = "macos")]
        unsafe {
            DisableSecureEventInput();
        }
    }
}

/// One count of secure input, held or not.
pub(crate) struct SecureInput<O: SecureInputOs = Platform> {
    os: O,
    held: bool,
}

impl Default for SecureInput {
    fn default() -> Self {
        Self::new(Platform)
    }
}

impl<O: SecureInputOs> SecureInput<O> {
    pub(crate) fn new(os: O) -> Self {
        Self { os, held: false }
    }

    /// Moves to `want`: one enable when it turns on, one disable when it
    /// turns off, nothing when it stays.
    pub(crate) fn set(&mut self, want: bool) {
        match (self.held, want) {
            (false, true) => self.os.enable(),
            (true, false) => self.os.disable(),
            _ => return,
        }
        self.held = want;
    }

    /// Whether this holds a count now.
    #[cfg(test)]
    pub(crate) fn held(&self) -> bool {
        self.held
    }
}

impl<O: SecureInputOs> Drop for SecureInput<O> {
    fn drop(&mut self) {
        self.set(false);
    }
}

/// Whether secure input is wanted: some window whose frame asked has the
/// keyboard. `windows` is `(asked, focused)` per window.
pub(crate) fn wanted(windows: impl IntoIterator<Item = (bool, bool)>) -> bool {
    windows.into_iter().any(|(asked, focused)| asked && focused)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    /// Counts the calls, and fails on the one thing the OS cannot forgive:
    /// a disable with no enable to balance.
    #[derive(Clone, Default)]
    struct Counting {
        enables: Rc<Cell<u32>>,
        disables: Rc<Cell<u32>>,
    }

    impl SecureInputOs for Counting {
        fn enable(&mut self) {
            self.enables.set(self.enables.get() + 1);
        }
        fn disable(&mut self) {
            self.disables.set(self.disables.get() + 1);
            assert!(
                self.disables.get() <= self.enables.get(),
                "a disable with nothing to balance"
            );
        }
    }

    /// The runner's step: the windows' `(asked, focused)` in, the guard
    /// moved to the answer.
    fn step(guard: &mut SecureInput<Counting>, windows: &[(bool, bool)]) {
        guard.set(wanted(windows.iter().copied()));
    }

    #[test]
    fn secure_input_follows_the_asking_window_s_focus_and_stays_balanced() {
        let os = Counting::default();
        let (en, dis) = (os.enables.clone(), os.disables.clone());
        let calls = || (en.get(), dis.get());
        let mut guard = SecureInput::new(os);

        // Nothing asked: nothing called, however focus moves.
        step(&mut guard, &[(false, true)]);
        step(&mut guard, &[(false, false)]);
        assert_eq!(calls(), (0, 0));

        // Asked in the background: still off — only while active.
        step(&mut guard, &[(true, false)]);
        assert_eq!(calls(), (0, 0));

        // The window takes the keyboard: one enable, and every frame
        // after that keeps asking calls nothing more.
        step(&mut guard, &[(true, true)]);
        step(&mut guard, &[(true, true)]);
        step(&mut guard, &[(true, true)]);
        assert_eq!(calls(), (1, 0));
        assert!(guard.held());

        // The keyboard goes elsewhere: one disable; back: one enable.
        step(&mut guard, &[(true, false)]);
        assert_eq!(calls(), (1, 1));
        step(&mut guard, &[(true, true)]);
        assert_eq!(calls(), (2, 1));

        // The frame stops asking — the prompt is gone — with the window
        // still focused: off.
        step(&mut guard, &[(false, true)]);
        assert_eq!(calls(), (2, 2));
        assert!(!guard.held());

        // Two windows: the one asking is the one that counts, and focus
        // moving to the other gives the count back.
        step(&mut guard, &[(true, true), (false, false)]);
        assert_eq!(calls(), (3, 2));
        step(&mut guard, &[(true, false), (false, true)]);
        assert_eq!(calls(), (3, 3));
        // Both asking: one count, not two, as focus moves between them.
        step(&mut guard, &[(true, true), (true, false)]);
        step(&mut guard, &[(true, false), (true, true)]);
        assert_eq!(calls(), (4, 3));

        // The asking window closes while it holds the keyboard: it is no
        // longer among the windows, and the count comes back.
        step(&mut guard, &[(false, false)]);
        assert_eq!(calls(), (4, 4));

        // Held at exit: dropping the guard gives it back.
        step(&mut guard, &[(true, true)]);
        assert_eq!(calls(), (5, 4));
        drop(guard);
        assert_eq!(calls(), (5, 5), "never left on at exit");
    }

    #[test]
    fn a_guard_that_never_held_calls_nothing_on_drop() {
        let os = Counting::default();
        let (en, dis) = (os.enables.clone(), os.disables.clone());
        let mut guard = SecureInput::new(os);
        guard.set(false);
        drop(guard);
        assert_eq!((en.get(), dis.get()), (0, 0));
    }
}
