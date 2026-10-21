//! The session a target's cores are built in, kept from one input to the
//! next.
//!
//! Every input gets a `Core` of its own, which is what keeps one input's
//! tree, focus, editors, menus, modal, clock and window facts out of the
//! next. What a core shares is its [`Session`], and a new session was
//! where the time went: its font system starts with empty caches, so the
//! first frame that shapes text matched every family again, opening and
//! parsing font files. That was nine tenths of an `events` or `scenes`
//! input (about 40 and 27 inputs a second under ASan). So a target keeps
//! one session per thread and builds every input's core in it with
//! [`Core::new_in`].
//!
//! The session holds more than fonts, so it goes back to the target only
//! when the input left nothing in it that a new session would not hold:
//!
//! - the window registry: its window ids only count up, so once an input
//!   has opened a window (a `WindowCommand::Open` the driver took) the
//!   next input would open its windows under other ids. The session is
//!   dropped. The same applies when a window other than the main one is
//!   still listed;
//! - the audio store: playback ids count up too, so once any audio
//!   command has been queued the session is dropped. Nothing mounts
//!   without queueing a play;
//! - the devtools panel's state, which nothing here turns on: a panel
//!   that is on drops the session;
//! - the resource registry: `setup` registers what the target needs (the
//!   conformance fixtures) once per session, and the stock polygon
//!   fragment a core registers on first use is found by its source the
//!   next time, not added again;
//! - the handles a core frees as it goes (its path textures), which wait
//!   on the session for the next display list: an empty frame on a
//!   throwaway core takes them, as one does after `setup`, so every input
//!   starts with none waiting.
//!
//! What stays is the font system's memo: the faces it loaded, the matches
//! it found for a family, a shaping plan. A cache hit gives the answer a
//! miss would. So an input runs the same on a kept session as on a new
//! one, which `tests/reuse.rs` checks: every kept input, and inputs made
//! from a fixed seed, run on new sessions ([`set_fresh`]) and on kept
//! ones, twice, with the same trace each time.
//!
//! An input that panics never hands its session back, so the input after
//! it gets a new one. A crash libFuzzer saves replays in a new process,
//! where the session is new too.

use std::cell::{Cell, RefCell};

use kui_core::{Core, Session, Size, WindowCommand};

thread_local! {
    static FRESH: Cell<bool> = const { Cell::new(false) };
    static MADE: Cell<u64> = const { Cell::new(0) };
}

/// A new session for every input on this thread (`on`), or kept sessions
/// (the default). For `tests/reuse.rs`, which runs inputs both ways.
pub fn set_fresh(on: bool) {
    FRESH.with(|f| f.set(on));
}

fn fresh() -> bool {
    FRESH.with(Cell::get)
}

/// How many sessions this thread's targets have made: what shows a test
/// that the kept ones were kept.
pub fn sessions_made() -> u64 {
    MADE.with(Cell::get)
}

/// One target's kept session and what its `setup` returned for it.
pub(crate) struct Kept<T: Copy + 'static> {
    slot: RefCell<Option<(Session, T)>>,
}

impl<T: Copy + 'static> Kept<T> {
    pub(crate) const fn new() -> Self {
        Self {
            slot: RefCell::new(None),
        }
    }

    /// A new core for one input, in the kept session. With none kept, a
    /// new session, `setup` run on a core of its own first.
    pub(crate) fn core(&self, setup: fn(&mut Core) -> T) -> (Core, T) {
        let kept = if fresh() {
            None
        } else {
            self.slot.borrow_mut().take()
        };
        let (session, extra) = kept.unwrap_or_else(|| {
            MADE.with(|m| m.set(m.get() + 1));
            let session = Session::new();
            let mut core = Core::new_in(&session);
            let extra = setup(&mut core);
            settle(core);
            (session, extra)
        });
        (Core::new_in(&session), extra)
    }

    /// The input is over: keeps the core's session for the next input,
    /// unless `dirty` (the driver saw a window open or an audio command)
    /// or the session shows it was changed.
    pub(crate) fn done(&self, core: Core, extra: T, dirty: bool) {
        let session = core.session().clone();
        let dirty = dirty || core.devtools() || core.windows().len() != 1;
        drop(core);
        if dirty || fresh() {
            return;
        }
        settle(Core::new_in(&session));
        *self.slot.borrow_mut() = Some((session, extra));
    }
}

/// An empty frame on `core`, read: what the session held for the next
/// display list leaves with it.
fn settle(mut core: Core) {
    core.frame(Size::new(1.0, 1.0), 1.0).finish();
    let _ = core.output();
    let _ = core.take_pending_events();
    let _ = core.take_window_commands();
    let _ = core.take_audio_commands();
    let _ = core.take_warnings();
}

/// Whether a driver taking `commands` off a core has seen a window open.
pub(crate) fn opens(commands: &[WindowCommand]) -> bool {
    commands
        .iter()
        .any(|c| matches!(c, WindowCommand::Open { .. }))
}
