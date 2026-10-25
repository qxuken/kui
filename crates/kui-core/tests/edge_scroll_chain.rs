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

/// A selectable page holding a code block that scrolls only sideways:
/// the keys of the page and of the block.
fn selectable_page(core: &mut Core) -> (Key, Key) {
    let mut ui = core.frame(WIN, 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.with_keyed(
        "page",
        NodeSpec::column()
            .grow_width()
            .height(WIN.h)
            .scroll_y()
            .selectable(),
        |ui| {
            ui.with_keyed(
                "code",
                NodeSpec::column().grow_width().height(60.0).scroll_x(),
                |ui| {
                    ui.text(&"code ".repeat(80), TextStyle::new(14.0).nowrap());
                },
            );
            for i in 0..80 {
                ui.text(&format!("paragraph {i}"), TextStyle::new(14.0));
            }
        },
    );
    ui.finish();
    (core.key_of("page").unwrap(), core.key_of("code").unwrap())
}

/// A drag-select pressed in the code block is followed from the block,
/// not from the selection's scope: held at the right edge it scrolls
/// the block sideways, held at the bottom the page.
#[test]
fn a_drag_select_in_a_sideways_block_scrolls_it_and_hands_down_to_the_page() {
    let mut core = Core::new();
    let mut t = 1.0;
    core.set_time(t);
    selectable_page(&mut core);
    let (page_key, code) = selectable_page(&mut core);
    let mut step = |core: &mut Core, n: usize| {
        for _ in 0..n {
            t += FRAME;
            core.set_time(t);
            selectable_page(core);
        }
    };
    // The one-line text overflows the block: it has somewhere to scroll.
    let g = core.scroll_geometry(code).unwrap();
    assert!(
        g.content.w > g.rect.w + 100.0,
        "a nowrap line overflows: {g:?}"
    );
    press(&mut core, Vec2::new(20.0, 10.0));
    core.handle_input(InputEvent::CursorMoved(Vec2::new(WIN.w - 0.5, 10.0)));
    step(&mut core, 20);
    assert!(core.scroll_offset(code).x > 0.0, "the block, sideways");
    assert_eq!(core.scroll_offset(page_key).y, 0.0);
    // Held on, to the line's end: the whole line is in the block's range.
    step(&mut core, 120);
    let g = core.scroll_geometry(code).unwrap();
    assert_eq!(core.scroll_offset(code).x, g.max_offset.x, "to the end");
    assert!(g.content.w > 1500.0, "the whole line: {g:?}");
    core.handle_input(InputEvent::CursorMoved(Vec2::new(200.0, WIN.h - 0.5)));
    step(&mut core, 20);
    assert!(core.scroll_offset(page_key).y > 0.0, "down is the page's");
    // On past the block scrolling out of view, to the page's end.
    step(&mut core, 200);
    let end = core.scroll_geometry(page_key).unwrap().max_offset.y;
    assert_eq!(
        core.scroll_offset(page_key).y,
        end,
        "all the way down, the block long gone"
    );
    release(&mut core);
}

/// The other way: a drag-select pressed in the page and brought into the
/// code block, held at the block's right edge, scrolls the block — the
/// walk starts at the scroller under the pointer inside the press's —
/// so part of a long line can be selected.
#[test]
fn a_drag_select_brought_into_a_sideways_block_scrolls_it() {
    let mut core = Core::new();
    let mut t = 1.0;
    core.set_time(t);
    selectable_page(&mut core);
    let (page_key, code) = selectable_page(&mut core);
    press(&mut core, Vec2::new(20.0, 120.0));
    core.handle_input(InputEvent::CursorMoved(Vec2::new(200.0, 30.0)));
    core.handle_input(InputEvent::CursorMoved(Vec2::new(WIN.w - 0.5, 30.0)));
    for _ in 0..30 {
        t += FRAME;
        core.set_time(t);
        selectable_page(&mut core);
    }
    assert!(
        core.scroll_offset(code).x > 100.0,
        "{:?}",
        core.scroll_offset(code)
    );
    assert_eq!(core.scroll_offset(page_key).y, 0.0);
    let sel = core.selection().expect("a selection");
    assert_ne!(
        sel.anchor.node, sel.focus.node,
        "from a paragraph into the block"
    );
    release(&mut core);
}

/// A page with a box that scrolls both ways in its middle, 30 paragraphs
/// down: the keys of the page and of the box.
fn page_with_grid(core: &mut Core) -> (Key, Key) {
    let mut ui = core.frame(WIN, 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.with_keyed(
        "page",
        NodeSpec::column()
            .grow_width()
            .height(WIN.h)
            .scroll_y()
            .selectable(),
        |ui| {
            for i in 0..80 {
                ui.text(&format!("paragraph {i}"), TextStyle::new(14.0));
                if i == 30 {
                    ui.with_keyed(
                        "grid",
                        NodeSpec::column()
                            .grow_width()
                            .height(160.0)
                            .scroll_x()
                            .scroll_y(),
                        |ui| {
                            for r in 0..30 {
                                ui.text(
                                    &format!("row {r} ").repeat(30),
                                    TextStyle::new(12.0).nowrap(),
                                );
                            }
                        },
                    );
                }
            }
        },
    );
    ui.finish();
    (core.key_of("page").unwrap(), core.key_of("grid").unwrap())
}

fn hold(core: &mut Core, t: &mut f64, n: usize) {
    for _ in 0..n {
        *t += FRAME;
        core.set_time(*t);
        page_with_grid(core);
    }
}

/// A box the page carries under a pointer held still at the bottom edge
/// does not take the drag: the hand never brought the drag into it.
#[test]
fn a_box_scrolled_under_a_still_pointer_leaves_the_drag_to_the_page() {
    let mut core = Core::new();
    let mut t = 1.0;
    core.set_time(t);
    page_with_grid(&mut core);
    let (page_key, grid) = page_with_grid(&mut core);
    press(&mut core, Vec2::new(20.0, 100.0));
    core.handle_input(InputEvent::CursorMoved(Vec2::new(20.0, 300.0)));
    core.handle_input(InputEvent::CursorMoved(Vec2::new(20.0, WIN.h - 1.0)));
    hold(&mut core, &mut t, 120);
    let end = core.scroll_geometry(page_key).unwrap().max_offset.y;
    assert_eq!(core.scroll_offset(page_key).y, end, "the page to its end");
    assert_eq!(core.scroll_offset(grid), Vec2::ZERO, "the box untouched");
    release(&mut core);
}

/// Brought into the two-way box and held at its bottom-right corner, a
/// drag scrolls it on both axes.
#[test]
fn a_two_way_box_held_at_its_corner_scrolls_both_ways() {
    let mut core = Core::new();
    let mut t = 1.0;
    core.set_time(t);
    page_with_grid(&mut core);
    let (page_key, grid) = page_with_grid(&mut core);
    // The box into view first, its top at the window's middle.
    let r = core.scroll_geometry(grid).unwrap().rect;
    core.set_scroll(page_key, Vec2::new(0.0, r.y - WIN.h / 2.0));
    hold(&mut core, &mut t, 1);
    let r = core.scroll_geometry(grid).unwrap().rect;
    press(&mut core, Vec2::new(20.0, r.y - 30.0));
    core.handle_input(InputEvent::CursorMoved(Vec2::new(60.0, r.y + 40.0)));
    core.handle_input(InputEvent::CursorMoved(Vec2::new(
        r.x + r.w - 2.0,
        r.y + r.h - 2.0,
    )));
    let page_at = core.scroll_offset(page_key).y;
    hold(&mut core, &mut t, 30);
    let o = core.scroll_offset(grid);
    assert!(o.x > 50.0 && o.y > 50.0, "both ways: {o:?}");
    assert_eq!(core.scroll_offset(page_key).y, page_at, "the page stays");
    release(&mut core);
}
