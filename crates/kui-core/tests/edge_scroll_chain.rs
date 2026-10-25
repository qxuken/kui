//! A held drag hands its edge scroll on (ADR 0029, amended 2026-10-25,
//! backlog F204): on an axis the nearest scroller cannot move — it does
//! not scroll that way, it is at its end, the pointer is clear of its
//! zones — the container around it takes the axis by its own zones, as
//! the wheel chains; `overscroll: contain` keeps it. kui's own drags and
//! an app's `Ui::edge_scroll` alike.

use kui_core::testing::{press, release};
use kui_core::{
    Core, EditOptions, InputEvent, Key, NodeSpec, Overscroll, Rect, Size, TextStyle, Vec2,
};

const WIN: Size = Size { w: 400.0, h: 600.0 };
const FRAME: f64 = 1.0 / 60.0;

/// What sits inside the page, below a 100 px header.
#[derive(Clone, Copy)]
enum Inner {
    /// A 200 px log that scrolls down over 30 lines (600 px): 400 px of
    /// its own travel; `contain` says `overscroll: contain`.
    Log { contain: bool },
    /// An 80 px strip that scrolls only sideways, over a line 800 wide.
    Strip,
}

/// A window-filling page that scrolls down over 2000 px more, holding
/// `inner`; the keys of the page and of the inner scroller.
fn page(core: &mut Core, inner: Inner) -> (Key, Key) {
    let mut ui = core.frame(WIN, 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let opts = |multiline| EditOptions {
        multiline,
        style: TextStyle::new(14.0).line_height(20.0),
        ..EditOptions::default()
    };
    ui.with_keyed(
        "page",
        NodeSpec::column().grow_width().height(WIN.h).scroll_y(),
        |ui| {
            ui.leaf(NodeSpec::column().grow_width().height(100.0));
            match inner {
                Inner::Log { contain } => {
                    let mut spec = NodeSpec::column().grow_width().height(200.0).scroll_y();
                    if contain {
                        spec = spec.overscroll(Overscroll::Contain);
                    }
                    ui.with_keyed("inner", spec, |ui| {
                        let text: String = (0..30).map(|i| format!("log {i}\n")).collect();
                        ui.text_edit("log", &text, &opts(true), NodeSpec::column().width(300.0));
                    });
                }
                Inner::Strip => {
                    ui.with_keyed(
                        "inner",
                        NodeSpec::row().grow_width().height(80.0).scroll_x(),
                        |ui| {
                            let text = "wide ".repeat(40);
                            ui.text_edit("line", &text, &opts(false), NodeSpec::row().width(800.0));
                        },
                    );
                }
            }
            ui.leaf(NodeSpec::column().grow_width().height(2000.0));
        },
    );
    ui.finish();
    (core.key_of("page").unwrap(), core.key_of("inner").unwrap())
}

fn setup(inner: Inner) -> (Core, Key, Key, f64) {
    let mut core = Core::new();
    core.set_time(1.0);
    page(&mut core, inner);
    let (p, i) = page(&mut core, inner);
    (core, p, i, 1.0)
}

/// `n` frames a sixtieth apart.
fn frames(core: &mut Core, inner: Inner, t: &mut f64, n: usize) {
    for _ in 0..n {
        *t += FRAME;
        core.set_time(*t);
        page(core, inner);
    }
}

/// A caret drag pressed at `y` in the inner text and held at the
/// window's bottom edge.
fn drag_to_bottom(core: &mut Core, y: f32) {
    press(core, Vec2::new(12.0, y));
    core.handle_input(InputEvent::CursorMoved(Vec2::new(12.0, WIN.h - 0.5)));
}

#[test]
fn a_log_at_its_end_hands_the_drag_to_the_page() {
    let inner = Inner::Log { contain: false };
    let (mut core, page_key, log, mut t) = setup(inner);
    drag_to_bottom(&mut core, 150.0);
    frames(&mut core, inner, &mut t, 4);
    assert!(core.scroll_offset(log).y > 0.0, "the log first");
    assert_eq!(core.scroll_offset(page_key).y, 0.0, "the page waits");
    frames(&mut core, inner, &mut t, 20);
    let travel = core.scroll_geometry(log).unwrap().max_offset.y;
    assert_eq!(core.scroll_offset(log).y, travel, "the log at its end");
    assert!(
        core.scroll_offset(page_key).y > 100.0,
        "then the page: {}",
        core.scroll_offset(page_key).y
    );
    assert!(core.animating(), "still moving, still asking");
    release(&mut core);
}

#[test]
fn contain_keeps_the_drag_in_the_log() {
    let inner = Inner::Log { contain: true };
    let (mut core, page_key, log, mut t) = setup(inner);
    drag_to_bottom(&mut core, 150.0);
    frames(&mut core, inner, &mut t, 130);
    let travel = core.scroll_geometry(log).unwrap().max_offset.y;
    assert_eq!(core.scroll_offset(log).y, travel);
    assert_eq!(core.scroll_offset(page_key).y, 0.0, "contained");
    assert!(!core.animating(), "at its end, idle");
    release(&mut core);
}

#[test]
fn a_strip_that_scrolls_sideways_hands_down_to_the_page() {
    let (mut core, page_key, strip, mut t) = setup(Inner::Strip);
    drag_to_bottom(&mut core, 110.0);
    frames(&mut core, Inner::Strip, &mut t, 30);
    assert!(core.scroll_offset(page_key).y > 0.0, "down is the page's");
    assert_eq!(core.scroll_offset(strip).y, 0.0);
    release(&mut core);
}

#[test]
fn an_apps_own_drag_hands_on_with_one_call() {
    let (mut core, page_key, strip, mut t) = setup(Inner::Strip);
    let rect: Rect = core.scroll_geometry(strip).unwrap().rect;
    let mut moved = Vec2::ZERO;
    for _ in 0..30 {
        t += FRAME;
        core.set_time(t);
        if let Some(m) = core.edge_scroll(strip, Vec2::new(200.0, WIN.h - 0.5), rect) {
            moved = Vec2::new(moved.x + m.x, moved.y + m.y);
        }
        page(&mut core, Inner::Strip);
    }
    assert!(moved.y > 0.0, "it says the page moved: {moved:?}");
    assert_eq!(core.scroll_offset(page_key).y, moved.y);
    assert_eq!(moved.x, 0.0, "the strip had no zone there");
}
