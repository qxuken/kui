//! The byte budget on the shaped-text cache (backlog C16).
//!
//! Every text a frame draws is shaped once and cached; the cache used to
//! empty only on a 300-frame clock, so a view streaming new text — a
//! terminal, a tailed log — held five seconds of everything it showed,
//! which measured 3.6 GB resident. Now it holds a budget: past it the
//! least recently drawn entries go, and what the last frame drew never
//! does. The allocator below counts live bytes so the budget is checked
//! against what the process actually holds, not against the estimate the
//! core charges itself.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use kui_core::{Color, Core, DEFAULT_TEXT_CACHE_BYTES, NodeSpec, QuadKind, Size, TextStyle};

struct Counting;
static LIVE: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        LIVE.fetch_add(l.size(), Ordering::Relaxed);
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        LIVE.fetch_sub(l.size(), Ordering::Relaxed);
        unsafe { System.dealloc(p, l) }
    }
}

#[global_allocator]
static ALLOC: Counting = Counting;

fn live() -> usize {
    LIVE.load(Ordering::Relaxed)
}

/// The allocator counts the whole process, and the harness runs tests in
/// parallel: every test here holds this so a neighbour's frames are not
/// in its delta.
static SERIAL: Mutex<()> = Mutex::new(());

fn serial() -> std::sync::MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|e| e.into_inner())
}

/// A line of `cols` printable ASCII characters unique to (row, salt).
fn line(row: usize, salt: usize, cols: usize) -> String {
    let mut x = (row as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (salt as u64).wrapping_mul(0xD1B5_4A32_D192_ED03)
        | 1;
    let mut s = String::with_capacity(cols);
    while s.len() < cols {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        s.push((b' ' + (x % 95) as u8) as char);
    }
    s
}

/// One frame of `rows` lines, all new for this `salt`; returns the glyph
/// quads it emitted.
fn stream_frame(core: &mut Core, salt: usize, rows: usize, cols: usize) -> usize {
    let mut ui = core.frame(Size::new(1920.0, 1080.0), 2.0);
    ui.configure_root(NodeSpec::column().fill().bg(Color::rgb8(0, 0, 0)));
    for r in 0..rows {
        let t = line(r, salt, cols);
        ui.text_in(
            NodeSpec::row().height(18.0),
            &t,
            TextStyle::new(13.0).mono().line_height(18.0),
        );
    }
    ui.finish();
    let (dl, _) = core.output();
    dl.quads
        .iter()
        .filter(|q| matches!(q.kind, QuadKind::GlyphMask | QuadKind::GlyphColor))
        .count()
}

#[test]
fn the_default_budget_is_what_the_constant_says() {
    let _serial = serial();
    let core = Core::new();
    assert_eq!(core.text_cache_budget(), DEFAULT_TEXT_CACHE_BYTES);
    assert_eq!(core.text_cache_bytes(), 0);
    assert_eq!(core.text_cache_len(), 0);
}

/// Streaming new lines every frame under a small budget: the cache
/// settles at the budget instead of growing with the frames, and the
/// process's live bytes settle with it.
#[test]
fn a_stream_of_new_text_settles_at_the_budget() {
    let _serial = serial();
    let budget = 2 << 20;
    let mut core = Core::new();
    core.set_text_cache_budget(budget);
    // Warm the fonts and the atlas so they are not in the delta.
    stream_frame(&mut core, 0, 50, 40);
    let mut peak_estimate = 0;
    let mut peak_live = 0;
    let base_live = live();
    for salt in 1..=80 {
        stream_frame(&mut core, salt, 50, 40);
        peak_estimate = peak_estimate.max(core.text_cache_bytes());
        peak_live = peak_live.max(live().saturating_sub(base_live));
    }
    // 80 frames × 50 lines × ~22 KB would be ~88 MB with no budget.
    assert!(
        peak_estimate <= budget + 50 * 40 * 1024,
        "the estimate never runs more than one frame's worth past the budget: {peak_estimate}"
    );
    assert!(
        peak_live < 4 * budget,
        "live bytes track the budget rather than the frames: {peak_live} B over the base"
    );
    // Eviction runs at the start of a frame, so a frame that draws nothing
    // new leaves the cache at or under its line.
    stream_frame(&mut core, 80, 50, 40);
    assert!(core.text_cache_bytes() <= budget);
}

/// The estimate the budget is charged against tracks what the allocator
/// sees: within a factor that makes "64 MB" mean tens of megabytes and not
/// hundreds.
#[test]
fn the_estimate_tracks_the_allocator() {
    let _serial = serial();
    let mut core = Core::new();
    stream_frame(&mut core, 0, 50, 200);
    let b0 = live();
    let e0 = core.text_cache_bytes();
    for salt in 1..=4 {
        stream_frame(&mut core, salt, 50, 200);
    }
    let measured = live() - b0;
    let estimated = core.text_cache_bytes() - e0;
    let ratio = estimated as f64 / measured as f64;
    assert!(
        (0.7..=1.6).contains(&ratio),
        "estimate {estimated} B against {measured} B live: ratio {ratio:.2}"
    );
}

/// The promise the budget must not break: what the last frame drew is
/// never evicted, however small the budget — a screenful that does not
/// fit the budget is kept whole and re-shapes nothing.
#[test]
fn what_the_last_frame_drew_is_never_evicted() {
    let _serial = serial();
    let mut core = Core::new();
    core.set_text_cache_budget(1);
    let first = stream_frame(&mut core, 0, 30, 40);
    assert!(first > 0);
    for _ in 0..10 {
        let again = stream_frame(&mut core, 0, 30, 40);
        assert_eq!(again, first, "every line still draws");
        assert_eq!(core.text_cache_len(), 30, "and every line is still cached");
    }
    assert!(core.text_cache_bytes() > 1, "over budget, by design");
}

/// The other direction: entries the last frame did not draw are what goes,
/// oldest first.
#[test]
fn the_least_recently_drawn_goes_first() {
    let _serial = serial();
    let mut core = Core::new();
    core.set_text_cache_budget(1);
    stream_frame(&mut core, 1, 20, 40);
    stream_frame(&mut core, 2, 20, 40);
    // Frame 3 draws set 2 again: set 1 is now the older of the two.
    stream_frame(&mut core, 2, 20, 40);
    // Frame 4 draws set 3; at its start set 1 (last drawn two frames
    // ago) is evictable and set 2 (drawn last frame) is not.
    stream_frame(&mut core, 3, 20, 40);
    assert_eq!(
        core.text_cache_len(),
        40,
        "set 2 kept, set 1 gone, set 3 added"
    );
}

/// The clock still empties an idle cache: a text shown once and never
/// again is gone after the eviction window, budget or no budget.
#[test]
fn an_idle_cache_still_empties_on_the_clock() {
    let _serial = serial();
    let mut core = Core::new();
    stream_frame(&mut core, 0, 10, 40);
    assert_eq!(core.text_cache_len(), 10);
    for _ in 0..600 {
        let mut ui = core.frame(Size::new(100.0, 100.0), 1.0);
        ui.configure_root(NodeSpec::column().fill());
        ui.finish();
    }
    assert_eq!(core.text_cache_len(), 0);
    assert_eq!(core.text_cache_bytes(), 0);
}
