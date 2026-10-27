//! The recording's doors (docs/adr/0058-a-recording-is-a-timeline.md):
//! spans, counters, gauges, marks and measures into the core's timeline,
//! named frame causes, the recording read back, and the runner's waker.
//! Off — nothing listening — every door but the waker's is a no-op on a
//! flag the core keeps, which `kui_perf_measuring` reads.

use super::*;

/// Begins a span named `name` inside the one open now, recording the node
/// the builder has open; end it with [`kui_span_end`]. 0 while nothing is
/// recorded (and on a bad context), which `kui_span_end` ignores.
#[unsafe(no_mangle)]
pub extern "C" fn kui_span_begin(ptr: *mut KuiCtx, name: KuiStr) -> u32 {
    guard(0, || {
        unsafe { ctx(ptr) }.map_or(0, |c| c.core().span_begin(&kstr(name)).0)
    })
}

/// Ends the span `id` names, and any opened inside it and left open. 0,
/// or an id with no begin, is nothing.
#[unsafe(no_mangle)]
pub extern "C" fn kui_span_end(ptr: *mut KuiCtx, id: u32) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().span_end(kui_core::SpanId(id));
        }
    })
}

/// A span timed elsewhere — a parse on another thread, a job — `ms` long,
/// ending now, on the track named `name`.
#[unsafe(no_mangle)]
pub extern "C" fn kui_perf_measure(ptr: *mut KuiCtx, name: KuiStr, ms: f64) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().perf_measure(&kstr(name), ms);
        }
    })
}

/// Adds `n` to this frame's counter `name`.
#[unsafe(no_mangle)]
pub extern "C" fn kui_perf_count(ptr: *mut KuiCtx, name: KuiStr, n: i64) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().perf_count(&kstr(name), n);
        }
    })
}

/// Sets the reading `name` — a footprint, a buffer's bytes — held until
/// set again.
#[unsafe(no_mangle)]
pub extern "C" fn kui_perf_gauge(ptr: *mut KuiCtx, name: KuiStr, value: f64) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().perf_gauge(&kstr(name), value);
        }
    })
}

/// Puts an instant on the timeline: a file opened, a command run.
#[unsafe(no_mangle)]
pub extern "C" fn kui_perf_mark(ptr: *mut KuiCtx, name: KuiStr) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().perf_mark(&kstr(name));
        }
    })
}

/// Puts a line on the timeline — what the numbers beside it cannot say —
/// shown in its frame's detail: at most 64 a frame and its gap, of 512
/// bytes each.
#[unsafe(no_mangle)]
pub extern "C" fn kui_perf_note(ptr: *mut KuiCtx, text: KuiStr) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().perf_note(&kstr(text));
        }
    })
}

/// Begins a span named `name` with no context at hand — inside a script
/// runtime, a library deep in a stack — on the recording of the context
/// whose span is open on this thread (its view's build, `kui_run`'s
/// callback, a span of the host's), nested in the span open now. 0 while
/// no span of a recording context is open on this thread.
#[unsafe(no_mangle)]
pub extern "C" fn kui_thread_span_begin(name: KuiStr) -> u32 {
    guard(0, || kui_core::perf::span_begin(&kstr(name)).0)
}

/// Ends the span [`kui_thread_span_begin`] began, and any opened inside it
/// and left open. 0, or an id with no begin, is nothing.
#[unsafe(no_mangle)]
pub extern "C" fn kui_thread_span_end(id: u32) {
    guard((), || kui_core::perf::span_end(kui_core::SpanId(id)))
}

/// Whether anything is recorded: the devtools' Perf tab recording or on
/// show, `KUI_PERF_TRACE`, [`kui_set_perf_recording`]. A host reads it to
/// skip reading clocks of its own for [`kui_perf_measure`].
#[unsafe(no_mangle)]
pub extern "C" fn kui_perf_measuring(ptr: *mut KuiCtx) -> bool {
    guard(false, || {
        unsafe { ctx(ptr) }.is_some_and(|c| c.core().perf_measuring())
    })
}

/// Asks for a frame after this one, as `animate` does, and names why: the
/// recording's causes read `request: why`.
#[unsafe(no_mangle)]
pub extern "C" fn kui_request_frame_for(ptr: *mut KuiCtx, why: KuiStr) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().request_frame_for(&kstr(why));
        }
    })
}

/// A wake named `name` reached this context: the next frame's causes read
/// `wake: name`. For a host whose own thread or loop woke it; under
/// `kui_run`, [`kui_waker_wake_named`] notes it.
#[unsafe(no_mangle)]
pub extern "C" fn kui_note_wake(ptr: *mut KuiCtx, name: KuiStr) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().note_wake(&kstr(name));
        }
    })
}

/// Starts a recording, dropping what was kept, or stops one and keeps it
/// for [`kui_perf_trace`]: the Perf tab's record button.
#[unsafe(no_mangle)]
pub extern "C" fn kui_set_perf_recording(ptr: *mut KuiCtx, on: bool) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().set_perf_recording(on);
        }
    })
}

/// Whether a recording runs.
#[unsafe(no_mangle)]
pub extern "C" fn kui_perf_recording(ptr: *mut KuiCtx) -> bool {
    guard(false, || {
        unsafe { ctx(ptr) }.is_some_and(|c| c.core().perf_recording())
    })
}

/// What is kept — the last recording, or the frames the Perf tab shows —
/// as `kui-trace` bytes, `*len` of them: a `.kuitrace` file's. Borrowed
/// until the next call. NULL and 0 on a bad context.
#[unsafe(no_mangle)]
pub extern "C" fn kui_perf_trace(ptr: *mut KuiCtx, len: *mut usize) -> *const u8 {
    guard(std::ptr::null(), || {
        let put = |n: usize| {
            if let Some(l) = unsafe { len.as_mut() } {
                *l = n;
            }
        };
        put(0);
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return std::ptr::null();
        };
        c.perf_trace = c.core().perf_trace();
        put(c.perf_trace.len());
        c.perf_trace.as_ptr()
    })
}

/// Writes out what the file `KUI_PERF_TRACE` names holds so far. False,
/// with the reason on stderr, when the write fails; true with no file.
#[unsafe(no_mangle)]
pub extern "C" fn kui_perf_flush(ptr: *mut KuiCtx) -> bool {
    guard(false, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return false;
        };
        match c.core().perf_flush() {
            Ok(()) => true,
            Err(e) => {
                eprintln!("kui: kui_perf_flush: {e}");
                false
            }
        }
    })
}

// ---------------------------------------------------------------------------
// The runner's waker (ADR 0058, decision 14)

/// The runner's waker, reference-counted and `Send`: what a host's thread
/// holds to get a frame drawn. From [`kui_waker`], freed with
/// [`kui_waker_free`].
#[cfg(feature = "runner")]
pub struct KuiWaker(pub(crate) kui_native::Waker);

/// Never made in a build without the runner, where [`kui_waker`] is NULL.
#[cfg(not(feature = "runner"))]
pub struct KuiWaker(std::convert::Infallible);

/// The runner's waker, for a thread that changed what the view shows (a
/// PTY reader, a file watcher, a socket) to ask for a frame: under
/// `kui_run`, from a view callback's context. NULL on any other context —
/// a standalone one, whose host has its own way to wake its loop and
/// [`kui_note_wake`] to name it. Free with [`kui_waker_free`].
#[unsafe(no_mangle)]
pub extern "C" fn kui_waker(ptr: *mut KuiCtx) -> *mut KuiWaker {
    guard(std::ptr::null_mut(), || {
        #[cfg(feature = "runner")]
        if let Some(c) = unsafe { ctx(ptr) }
            && let Some(w) = unsafe { c.waker.cast::<kui_native::Waker>().as_ref() }
        {
            return Box::into_raw(Box::new(KuiWaker(w.clone())));
        }
        #[cfg(not(feature = "runner"))]
        let _ = ptr;
        std::ptr::null_mut()
    })
}

/// The waker's loop, from a thread's pointer.
fn waker_of<'a>(w: *const KuiWaker) -> Option<&'a KuiWaker> {
    unsafe { w.as_ref() }
}

/// Asks every window for a frame. Any thread, any rate: wakes coalesce
/// into the loop's next turn. Harmless after the run has ended; NULL is
/// nothing.
#[unsafe(no_mangle)]
pub extern "C" fn kui_waker_wake(w: *const KuiWaker) {
    guard((), || {
        #[cfg(feature = "runner")]
        if let Some(w) = waker_of(w) {
            w.0.wake();
        }
        #[cfg(not(feature = "runner"))]
        let _ = waker_of(w);
    })
}

/// Asks every window for a frame `ms` from now, with no thread waiting for
/// it; the earliest asked for wins. Not more than 0 (or NaN) is now;
/// infinity is never.
#[unsafe(no_mangle)]
pub extern "C" fn kui_waker_wake_at(w: *const KuiWaker, ms: f64) {
    guard((), || {
        #[cfg(feature = "runner")]
        if let Some(w) = waker_of(w) {
            let now = std::time::Instant::now();
            if ms.is_nan() || ms <= 0.0 {
                w.0.wake_at(now);
            } else if let Ok(d) = std::time::Duration::try_from_secs_f64(ms / 1000.0)
                && let Some(at) = now.checked_add(d)
            {
                w.0.wake_at(at);
            }
        }
        #[cfg(not(feature = "runner"))]
        let _ = (waker_of(w), ms);
    })
}

/// [`kui_waker_wake`] under a name: the next frame's causes read
/// `wake: name` in a recording, where a plain wake reads `wake`.
#[unsafe(no_mangle)]
pub extern "C" fn kui_waker_wake_named(w: *const KuiWaker, name: KuiStr) {
    guard((), || {
        #[cfg(feature = "runner")]
        if let Some(w) = waker_of(w) {
            w.0.named(&kstr(name)).wake();
        }
        #[cfg(not(feature = "runner"))]
        let _ = (waker_of(w), name);
    })
}

/// Another handle on the same loop, for another thread; free each. NULL
/// for NULL.
#[unsafe(no_mangle)]
pub extern "C" fn kui_waker_clone(w: *const KuiWaker) -> *mut KuiWaker {
    guard(std::ptr::null_mut(), || {
        #[cfg(feature = "runner")]
        if let Some(w) = waker_of(w) {
            return Box::into_raw(Box::new(KuiWaker(w.0.clone())));
        }
        #[cfg(not(feature = "runner"))]
        let _ = waker_of(w);
        std::ptr::null_mut()
    })
}

/// Frees a waker from [`kui_waker`] or [`kui_waker_clone`]; NULL is
/// nothing.
#[unsafe(no_mangle)]
pub extern "C" fn kui_waker_free(w: *mut KuiWaker) {
    guard((), || {
        if !w.is_null() {
            drop(unsafe { Box::from_raw(w) });
        }
    })
}
