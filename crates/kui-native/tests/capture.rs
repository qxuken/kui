//! `kui_native::capture` (backlog C55): a core's finished frame as RGBA,
//! drawn again on the process's windowless device. A headless core here,
//! the way a test captures; skipped, saying so, where there is no adapter,
//! and failed there where `KUI_REQUIRE_GPU` is set, as CI sets it over
//! Mesa's software Vulkan.

use kui_native::{Color, Core, NodeSpec, Size, capture};

/// `None` when this machine has no GPU adapter at all, which is the one
/// failure a test cannot do anything about.
fn shot(core: &Core) -> Option<kui_native::Capture> {
    match capture(core) {
        Ok(c) => Some(c),
        Err(e) if e.starts_with("no GPU") => {
            assert!(
                std::env::var_os("KUI_REQUIRE_GPU").is_none_or(|v| v.is_empty()),
                "KUI_REQUIRE_GPU is set, but {e}"
            );
            eprintln!("skipped: {e}");
            None
        }
        Err(e) => panic!("{e}"),
    }
}

fn at(c: &kui_native::Capture, x: u32, y: u32) -> [u8; 4] {
    let o = ((y * c.width + x) * 4) as usize;
    c.rgba[o..o + 4].try_into().unwrap()
}

#[test]
fn a_headless_core_captures_its_frame_over_the_theme_s_ground() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(30.0, 20.0), 2.0);
    ui.configure_root(NodeSpec::column().pad(5.0));
    ui.leaf_keyed(
        "box",
        NodeSpec::column()
            .size(10.0, 5.0)
            .bg(Color::hex(0x00ff00ff)),
    );
    ui.finish();
    let Some(c) = shot(&core) else { return };
    assert_eq!((c.width, c.height, c.scale), (60, 40, 2.0));
    assert_eq!(at(&c, 20, 15), [0, 255, 0, 255], "the box, in physical px");
    // The root paints nothing: what shows is the clear, the theme's bg,
    // as an opaque window's is.
    let bg = core.theme().bg;
    let byte = |v: f32| (v * 255.0).round() as u8;
    let want = [byte(bg.r), byte(bg.g), byte(bg.b), 255];
    let got = at(&c, 55, 35);
    for i in 0..4 {
        assert!(got[i].abs_diff(want[i]) <= 1, "{got:?} against {want:?}");
    }
    // A second capture reuses the device and says the same.
    assert_eq!(shot(&core).expect("again").rgba, c.rgba);
}

#[test]
fn a_core_with_no_finished_frame_has_nothing_to_capture() {
    let core = Core::new();
    let e = capture(&core).expect_err("no frame yet");
    assert!(e.contains("no finished frame"), "{e}");
}

// -- What an app asks the driver to keep (ADR 0055) ------------------------

use kui_native::{CapturePhase, Captured, Captures, Event, EventKind, UiEvent, Value};

/// A frame at `now` on the frame clock: a box `x` px in, green.
fn frame_at(core: &mut Core, now: f64, x: f32) {
    core.set_time(now);
    let mut ui = core.frame(Size::new(30.0, 20.0), 2.0);
    ui.configure_root(NodeSpec::row().pad(5.0));
    ui.leaf_keyed("gap", NodeSpec::column().size(x, 1.0));
    ui.leaf_keyed(
        "box",
        NodeSpec::column()
            .size(10.0, 5.0)
            .bg(Color::hex(0x00ff00ff)),
    );
    ui.finish();
}

/// The `capture` events a serve left pending, as (message, result).
fn answers(core: &mut Core) -> Vec<(Value, Captured)> {
    core.take_pending_events()
        .into_iter()
        .filter(|e: &UiEvent| e.event == EventKind::Capture)
        .map(|e| match e.read() {
            Event::Capture(c) => (e.payload.get("tag").cloned().unwrap_or(Value::Null), c),
            other => panic!("{other:?}"),
        })
        .collect()
}

/// `None`, saying so, when the answer is that there is no GPU.
fn saved(a: &(Value, Captured)) -> Option<&Captured> {
    match (&a.1.phase, &a.1.error) {
        (CapturePhase::Failed, Some(e)) if e.starts_with("no GPU") => {
            eprintln!("skipped: {e}");
            None
        }
        (CapturePhase::Saved, None) => Some(&a.1),
        other => panic!("{other:?}"),
    }
}

fn scratch(name: &str) -> String {
    let dir = std::env::temp_dir().join(format!("kui-captures-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name).to_string_lossy().into_owned()
}

#[test]
fn a_still_asked_for_is_the_next_frame_written_and_answered() {
    let mut core = Core::new();
    let mut captures = Captures::new();
    let path = scratch("still.png");
    core.capture_to(&path, Some(Value::str("shot")));
    // Between frames, the ask is a frame owed.
    assert!(
        core.animating(),
        "a capture asked for between frames asks for one"
    );
    frame_at(&mut core, 0.0, 0.0);
    captures.serve(&mut core);
    let got = answers(&mut core);
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].0, Value::str("shot"), "in the message it named");
    let Some(c) = saved(&got[0]) else { return };
    assert_eq!(
        (c.path.as_str(), c.width, c.height, c.frames),
        (path.as_str(), 60, 40, 1)
    );
    let png = std::fs::read(&path).unwrap();
    let back = kui_native::decode_image(&png).unwrap();
    assert_eq!((back.width, back.height), (60, 40));
    assert_eq!(&back.rgba[(15 * 60 + 12) * 4..][..4], [0, 255, 0, 255]);
    // Served once: the next frame asks nothing.
    frame_at(&mut core, 0.1, 0.0);
    captures.serve(&mut core);
    assert!(answers(&mut core).is_empty());
}

/// "Copy screenshot": the next frame, queued for the clipboard as a menu
/// action the host applies — the runner with arboard — and answered with
/// an empty path.
#[test]
fn a_copy_asked_for_is_the_next_frame_queued_for_the_clipboard() {
    let mut core = Core::new();
    let mut captures = Captures::new();
    core.capture_to_clipboard(Some(Value::str("copy")));
    frame_at(&mut core, 0.0, 0.0);
    captures.serve(&mut core);
    let got = answers(&mut core);
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].0, Value::str("copy"));
    let actions = core.take_menu_actions();
    let Some(c) = saved(&got[0]) else {
        assert!(actions.is_empty(), "nothing copied when the capture failed");
        return;
    };
    assert_eq!(
        (c.path.as_str(), c.width, c.height, c.frames),
        ("", 60, 40, 1)
    );
    let [
        kui_native::MenuAction::SetClipboardImage {
            width,
            height,
            rgba,
        },
    ] = &actions[..]
    else {
        panic!("one picture for the clipboard, not {actions:?}");
    };
    assert_eq!((*width, *height), (60, 40));
    assert_eq!(&rgba[(15 * 60 + 12) * 4..][..4], [0, 255, 0, 255]);
}

#[test]
fn a_recording_shows_each_frame_until_the_next_on_the_frame_clock() {
    let mut core = Core::new();
    let mut captures = Captures::new();
    let path = scratch("rec.apng");
    // Asked for from the view: the frame that view builds is the first.
    core.set_time(1.0);
    let mut ui = core.frame(Size::new(30.0, 20.0), 2.0);
    ui.record_to(path.clone(), Some(Value::str("rec")));
    ui.finish();
    captures.serve(&mut core);
    assert!(answers(&mut core).iter().all(|a| saved(a).is_some()));
    if !captures.recording() {
        return; // no GPU: the failure above was the skip
    }
    frame_at(&mut core, 1.25, 0.0);
    captures.serve(&mut core);
    frame_at(&mut core, 1.5, 4.0);
    captures.serve(&mut core);
    core.stop_recording();
    // The frame drawn at the stop is left out: the last shows until it.
    frame_at(&mut core, 2.0, 8.0);
    captures.serve(&mut core);
    assert!(!captures.recording());
    let got = answers(&mut core);
    assert_eq!(got.len(), 1);
    let c = saved(&got[0]).expect("the device opened for the first frame");
    assert_eq!((c.width, c.height, c.frames, c.seconds), (60, 40, 3, 1.0));
    let anim = kui_native::decode_animation(&std::fs::read(&path).unwrap()).unwrap();
    let delays: Vec<f64> = anim.frames.iter().map(|f| f.delay).collect();
    // The empty first frame, the box, the box moved.
    assert_eq!(delays, [0.25, 0.25, 0.5]);
    assert_eq!(anim.loops, None, "a recording plays for ever");
}

#[test]
fn what_cannot_be_written_fails_saying_why() {
    let mut core = Core::new();
    let mut captures = Captures::new();
    core.capture_to(scratch("still.bmp"), Some(Value::str("bmp")));
    core.record_to(scratch("rec.jpg"), Some(Value::str("jpg")));
    // Nothing named: written or not, nothing is heard.
    core.capture_to(scratch("quiet.bmp"), None);
    frame_at(&mut core, 0.0, 0.0);
    captures.serve(&mut core);
    let got = answers(&mut core);
    assert_eq!(got.len(), 2);
    for (tag, c) in &got {
        assert_eq!(c.phase, CapturePhase::Failed);
        let e = c.error.as_deref().unwrap();
        assert!(e.contains("names no"), "{tag:?}: {e}");
    }
    assert!(!captures.recording());
    // A second recording while one runs is refused, naming the first.
    core.record_to(scratch("one.apng"), Some(Value::str("one")));
    core.record_to(scratch("two.gif"), Some(Value::str("two")));
    frame_at(&mut core, 0.1, 0.0);
    captures.serve(&mut core);
    let got = answers(&mut core);
    let two = got
        .iter()
        .find(|a| a.0 == Value::str("two"))
        .expect("refused");
    assert!(
        two.1.error.as_deref().unwrap().contains("one.apng"),
        "{:?}",
        two.1
    );
    // A stop with none running, or a stop of one that never got its
    // first frame, says what there was.
    if captures.recording() {
        core.stop_recording();
        frame_at(&mut core, 0.2, 0.0);
        captures.serve(&mut core);
        let one = answers(&mut core);
        assert_eq!(one.len(), 1);
        assert_eq!(saved(&one[0]).map(|c| c.frames), Some(1));
    }
    core.stop_recording();
    frame_at(&mut core, 0.3, 0.0);
    captures.serve(&mut core);
    assert!(answers(&mut core).is_empty());
}

/// The runner's mode: a recording written on a thread of its own. The
/// serving thread only draws and submits each frame; the answer comes on
/// a later serve, after the thread has asked for a frame, and the file is
/// the one the serving thread would have written.
#[test]
fn a_recording_on_its_own_thread_writes_the_same_file_and_wakes_for_its_answer() {
    let mut core = Core::new();
    let mut captures = Captures::new();
    let wakes = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    captures.write_on_thread({
        let wakes = wakes.clone();
        move || {
            wakes.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
    });
    let path = scratch("thread.apng");
    core.set_time(1.0);
    let mut ui = core.frame(Size::new(30.0, 20.0), 2.0);
    ui.record_to(path.clone(), Some(Value::str("rec")));
    ui.finish();
    captures.serve(&mut core);
    if kui_native::capture(&core).is_err_and(|e| e.starts_with("no GPU")) {
        eprintln!("skipped: no GPU adapter");
        return;
    }
    assert!(captures.recording());
    frame_at(&mut core, 1.25, 0.0);
    captures.serve(&mut core);
    frame_at(&mut core, 1.5, 4.0);
    captures.serve(&mut core);
    core.stop_recording();
    frame_at(&mut core, 2.0, 8.0);
    captures.serve(&mut core);
    assert!(!captures.recording(), "stopped, though still writing");
    // The answer comes once the thread has written the file.
    let mut got = answers(&mut core);
    let start = std::time::Instant::now();
    while got.is_empty() && start.elapsed() < std::time::Duration::from_secs(20) {
        std::thread::sleep(std::time::Duration::from_millis(5));
        captures.serve(&mut core);
        got = answers(&mut core);
    }
    assert_eq!(got.len(), 1);
    assert!(
        wakes.load(std::sync::atomic::Ordering::SeqCst) >= 1,
        "it asked for the frame"
    );
    let c = saved(&got[0]).expect("written");
    assert_eq!((c.width, c.height, c.frames, c.seconds), (60, 40, 3, 1.0));
    let anim = kui_native::decode_animation(&std::fs::read(&path).unwrap()).unwrap();
    let delays: Vec<f64> = anim.frames.iter().map(|f| f.delay).collect();
    assert_eq!(delays, [0.25, 0.25, 0.5]);
}

/// A window that goes mid-recording on a thread still leaves the file,
/// whole, by the time its captures are dropped.
#[test]
fn a_recording_on_a_thread_is_written_when_its_window_goes() {
    let mut core = Core::new();
    let mut captures = Captures::new();
    captures.write_on_thread(|| {});
    let path = scratch("dropped.gif");
    let _ = std::fs::remove_file(&path);
    core.record_to(path.clone(), None);
    frame_at(&mut core, 0.0, 0.0);
    captures.serve(&mut core);
    if !captures.recording() {
        return; // no GPU: the failure was said on stderr
    }
    frame_at(&mut core, 0.1, 4.0);
    captures.serve(&mut core);
    drop(captures);
    let anim = kui_native::decode_animation(&std::fs::read(&path).expect("written")).unwrap();
    assert_eq!(anim.frames.len(), 2);
}
