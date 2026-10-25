//! A component instance replayed (ADR 0060, ADR 0067; first built for
//! slots as ADR 0045, backlog F142): every placement keeps what the
//! extension's fill built, and the next placement at the same key pushes it
//! again without asking the extension — when the props agree and nothing
//! the fill read of the frame has moved. Everything else here is the core
//! refusing, and saying which.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use kui_core::{
    Color, Core, Extension, Extensions, InputEvent, Key, NodeSpec, OriginId, Size, Instance, FillAnswer,
    TextStyle, Ui, UiEvent, Value, Vec2,
};

/// A stand-in extension that counts its views and draws what the test
/// asks: `rows` text rows in a keyed column, each a hover-tracking box,
/// and — on request — reads a fact of the frame, places a nested
/// component, asks for a frame, or draws a `cells` grid.
#[derive(Default)]
struct Ext {
    views: Rc<Cell<usize>>,
    events: Rc<RefCell<Vec<Value>>>,
    rows: usize,
    /// Read `is_hovered` of the first row while drawing.
    read_hover: bool,
    /// Read the frame clock while drawing.
    read_clock: bool,
    /// Ask for `row0` by label before declaring it, as a view asks for
    /// its own scroller's geometry at its top.
    read_label: bool,
    /// Draw the theme's accent, read through `Ui::theme`.
    read_theme: bool,
    /// Draw the metrics' radius, read through `Ui::metrics`.
    read_metrics: bool,
    /// Draw the `accent` role, read by name through `Ui::token_color`.
    read_token: bool,
    /// Place `inner/panel` between the rows.
    nest: bool,
    /// Ask for a frame while drawing.
    ask_frame: bool,
    /// Draw a `cells` grid, which the journal does not know.
    cells: bool,
    /// Fail the view.
    fail: bool,
}

fn row(i: usize) -> NodeSpec {
    NodeSpec::row()
        .size(80.0, 20.0)
        .hover_bg(Color::hex(0x3355ffff))
        .on_click(Value::map([("row", Value::Int(i as i64))]))
}

impl Extension for Ext {
    fn name(&self) -> &str {
        "outer"
    }
    fn components(&self) -> &[String] {
        std::slice::from_ref(&PANE)
    }
    fn view(&mut self, slot: &Instance<'_>, ui: &mut Ui<'_>) -> Result<(), String> {
        self.views.set(self.views.get() + 1);
        if self.fail {
            return Err("no view today".into());
        }
        let title = match slot.params.get("title") {
            Some(Value::Str(s)) => s.to_string(),
            _ => "untitled".into(),
        };
        ui.with_keyed("panel", NodeSpec::column().gap(2.0), |ui| {
            ui.text(&title, TextStyle::new(12.0));
            if self.read_hover {
                let k = ui.child_key("row0");
                let hovered = ui.is_hovered(k);
                ui.text(
                    if hovered { "hovered" } else { "not hovered" },
                    TextStyle::new(12.0),
                );
            }
            if self.read_label {
                let found = ui.key_of("row0").is_some();
                ui.text(if found { "found row0" } else { "no row0 yet" }, TextStyle::new(12.0));
            }
            if self.read_clock {
                let now = ui.now();
                ui.text(&format!("{now:.1}"), TextStyle::new(12.0));
            }
            if self.read_theme {
                let accent = ui.theme().accent.to_hex();
                ui.text(&format!("{accent:08x}"), TextStyle::new(12.0));
            }
            if self.read_metrics {
                let radius = ui.metrics().radius;
                ui.text(&format!("radius {radius}"), TextStyle::new(12.0));
            }
            if self.read_token {
                let accent = ui.token_color("accent").map(|c| c.to_hex());
                ui.text(
                    &format!("{:08x}", accent.unwrap_or(0)),
                    TextStyle::new(12.0),
                );
            }
            if self.ask_frame {
                ui.request_frame();
            }
            for i in 0..self.rows {
                let label = format!("row{i}");
                ui.text_in_keyed(&label, row(i), &format!("row {i}"), TextStyle::new(12.0));
                if self.nest && i == 0 {
                    ui.component("inner/panel", &Value::Null);
                }
            }
            if self.cells {
                let grid = kui_core::CellGrid {
                    cols: 2,
                    rows: 1,
                    cells: &[],
                    style: TextStyle::new(12.0).mono(),
                    cursor: None,
                    origin_line: 0,
                };
                ui.cells(&grid, NodeSpec::column());
            }
        });
        Ok(())
    }
    fn on_event(&mut self, ev: &UiEvent) -> Vec<Value> {
        self.events.borrow_mut().push(ev.payload.clone());
        Vec::new()
    }
}

/// The inner extension a nested placement asks for: one text, counted.
#[derive(Default)]
struct Inner {
    views: Rc<Cell<usize>>,
}

impl Extension for Inner {
    fn name(&self) -> &str {
        "inner"
    }
    fn components(&self) -> &[String] {
        &[]
    }
    fn view(&mut self, _slot: &Instance<'_>, ui: &mut Ui<'_>) -> Result<(), String> {
        self.views.set(self.views.get() + 1);
        ui.text_in_keyed(
            "inner",
            NodeSpec::row().size(40.0, 10.0),
            "inner",
            TextStyle::new(10.0),
        );
        Ok(())
    }
    fn on_event(&mut self, _ev: &UiEvent) -> Vec<Value> {
        Vec::new()
    }
}

static PANE: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| "pane".to_owned());

struct Rig {
    core: Core,
    exts: Extensions,
    views: Rc<Cell<usize>>,
    inner_views: Rc<Cell<usize>>,
    events: Rc<RefCell<Vec<Value>>>,
}

fn rig(ext: Ext) -> Rig {
    let views = ext.views.clone();
    let events = ext.events.clone();
    let inner_views = Rc::new(Cell::new(0));
    let mut exts = Extensions::new();
    exts.push_as("outer", Box::new(ext)).unwrap();
    // `Inner` takes every name under its namespace.
    exts.push_as(
        "inner",
        Box::new(InnerAny {
            inner: Inner {
                views: inner_views.clone(),
            },
            components: vec![kui_core::ANY_COMPONENT.to_owned()],
        }),
    )
    .unwrap();
    let mut core = Core::new();
    // The node snapshot (`Core::nodes`) is taken at `finish` while asked.
    core.set_inspect(true);
    Rig {
        core,
        exts,
        views,
        inner_views,
        events,
    }
}

/// `Inner` with a components list of its own.
struct InnerAny {
    inner: Inner,
    components: Vec<String>,
}

impl Extension for InnerAny {
    fn name(&self) -> &str {
        self.inner.name()
    }
    fn components(&self) -> &[String] {
        &self.components
    }
    fn view(&mut self, slot: &Instance<'_>, ui: &mut Ui<'_>) -> Result<(), String> {
        self.inner.view(slot, ui)
    }
    fn on_event(&mut self, ev: &UiEvent) -> Vec<Value> {
        self.inner.on_event(ev)
    }
}

fn params(title: &str, fresh: bool) -> Value {
    let mut p = vec![
        ("key".to_owned(), Value::str("pane")),
        ("title".to_owned(), Value::Str(title.into())),
    ];
    if fresh {
        p.push(("fresh".to_owned(), Value::Bool(true)));
    }
    Value::Map(p)
}

impl Rig {
    /// One frame: a root column, the instance — `fresh` (drawn and not
    /// kept) or replayable — then a host cell; `inside` places it in a
    /// keyed box of its own instead of at the root, which is another key.
    /// Answers what the placement said.
    fn frame_with(&mut self, fresh: bool, title: &str, inside: bool) -> Option<FillAnswer> {
        let p = params(title, fresh);
        let mut ui = self
            .core
            .frame_with(Size::new(400.0, 300.0), 1.0, &mut self.exts);
        ui.configure_root(NodeSpec::column().fill());
        let fill = if inside {
            let mut fill = None;
            ui.with_keyed("other", NodeSpec::column(), |ui| fill = ui.component("outer/pane", &p));
            fill
        } else {
            ui.component("outer/pane", &p)
        };
        ui.leaf(NodeSpec::row().size(40.0, 10.0));
        ui.finish();
        fill
    }

    fn frame(&mut self, fresh: bool) -> Option<FillAnswer> {
        self.frame_with(fresh, "a title", false)
    }

    /// The keys the frame holds, in tree order, with each node's origin.
    fn keys(&mut self) -> Vec<(Key, OriginId)> {
        self.core.set_inspect(true);
        let out: Vec<_> = self
            .core
            .nodes()
            .iter()
            .map(|n| (n.key, n.origin))
            .collect();
        out
    }

    fn texts(&mut self) -> Vec<String> {
        self.core
            .access_tree()
            .nodes
            .iter()
            .filter_map(|n| n.name.clone())
            .collect()
    }

    fn rect_of(&mut self, label: &str) -> kui_core::Rect {
        let key = self.core.key_of(label).expect(label);
        self.core.set_inspect(true);
        self.core
            .nodes()
            .iter()
            .find(|n| n.key == key)
            .map(|n| n.rect)
            .expect("in the snapshot")
    }
}

#[test]
fn a_replayed_instance_spares_the_extension_and_builds_the_same_tree() {
    let mut r = rig(Ext {
        rows: 3,
        ..Default::default()
    });
    // A fresh frame, drawn and not kept, for the tree to compare against.
    assert_eq!(r.frame(true), Some(FillAnswer::NotKept));
    let fresh = r.keys();
    let fresh_texts = r.texts();
    assert_eq!(r.views.get(), 1);
    // The first replayable placement fills and keeps; the next replays.
    assert_eq!(r.frame(false), Some(FillAnswer::NotKept));
    assert_eq!(r.frame(false), Some(FillAnswer::Replayed));
    assert_eq!(r.views.get(), 2, "the extension was not asked");
    assert_eq!(r.keys(), fresh, "the same nodes under the same keys");
    assert_eq!(r.texts(), fresh_texts, "and the same text");
    // The panel, its title, and a box with a text per row.
    assert_eq!(r.core.instance_kept_nodes(Key::ROOT.str("pane")), Some(8));
    // Again and again.
    for _ in 0..3 {
        assert_eq!(r.frame(false), Some(FillAnswer::Replayed));
    }
    assert_eq!(r.views.get(), 2);
}

#[test]
fn a_replayed_node_still_takes_input_under_the_extensions_origin() {
    let mut r = rig(Ext {
        rows: 2,
        ..Default::default()
    });
    r.frame(false);
    assert_eq!(r.frame(false), Some(FillAnswer::Replayed));
    let rect = r.rect_of("row1");
    let events = kui_core::testing::click(&mut r.core, Vec2::new(rect.x + 2.0, rect.y + 2.0));
    let hit = events
        .iter()
        .find(|e| e.payload.get("row").is_some())
        .expect("the row's click");
    assert_eq!(hit.origin, OriginId(1), "tagged the extension's");
    assert_eq!(hit.payload.get("row"), Some(&Value::Int(1)));
    // And the runner's routing hands it to the extension.
    r.exts.route(events, |_| {});
    assert_eq!(r.events.borrow().len(), 1, "the extension heard it");
}

#[test]
fn a_fresh_fill_says_why() {
    let mut r = rig(Ext {
        rows: 1,
        ..Default::default()
    });
    // Nothing kept yet: the first placement fills, and keeps.
    assert_eq!(r.frame(false), Some(FillAnswer::NotKept));
    assert_eq!(r.views.get(), 1);
    assert_eq!(r.frame(false), Some(FillAnswer::Replayed));

    // Other props: filled fresh, and the new props are what is kept.
    assert_eq!(r.frame_with(false, "another", false), Some(FillAnswer::Params));
    assert_eq!(r.views.get(), 2);
    assert!(r.texts().iter().any(|t| t == "another"));
    assert_eq!(r.frame_with(false, "another", false), Some(FillAnswer::Replayed));

    // Placed under another parent: another key, so nothing kept there.
    assert_eq!(r.frame_with(false, "another", true), Some(FillAnswer::NotKept));
    assert_eq!(r.views.get(), 3);
    assert_eq!(r.frame_with(false, "another", true), Some(FillAnswer::Replayed));

    // A frame that places it fresh, or not at all, forgets the kept one.
    assert_eq!(r.frame_with(true, "another", true), Some(FillAnswer::NotKept));
    assert_eq!(r.frame_with(false, "another", true), Some(FillAnswer::NotKept));
    {
        let mut ui = r.core.frame_with(Size::new(400.0, 300.0), 1.0, &mut r.exts);
        ui.leaf(NodeSpec::row().size(40.0, 10.0));
        ui.finish();
    }
    assert_eq!(r.frame_with(false, "another", true), Some(FillAnswer::NotKept));
}

#[test]
fn a_fact_the_fill_read_that_moved_runs_it_again() {
    let mut r = rig(Ext {
        rows: 2,
        read_hover: true,
        ..Default::default()
    });
    r.frame(false);
    assert_eq!(r.frame(false), Some(FillAnswer::Replayed));
    assert!(r.texts().iter().any(|t| t == "not hovered"));

    // The pointer onto the row the fill asked about: what it read moved.
    let rect = r.rect_of("row0");
    r.core.handle_input(InputEvent::CursorMoved(Vec2::new(
        rect.x + 1.0,
        rect.y + 1.0,
    )));
    assert_eq!(r.frame(false), Some(FillAnswer::Reads));
    assert_eq!(r.views.get(), 2);
    assert!(r.texts().iter().any(|t| t == "hovered"));
    // Still there: replayed, hovered.
    assert_eq!(r.frame(false), Some(FillAnswer::Replayed));
    // Away again.
    r.core
        .handle_input(InputEvent::CursorMoved(Vec2::new(390.0, 290.0)));
    assert_eq!(r.frame(false), Some(FillAnswer::Reads));
    assert!(r.texts().iter().any(|t| t == "not hovered"));
}

#[test]
fn a_label_asked_for_before_it_answered_runs_the_fill_again_once_it_does() {
    let mut r = rig(Ext {
        rows: 1,
        read_label: true,
        ..Default::default()
    });
    // The first fill asks before its row exists: no answer, and kept so.
    assert_eq!(r.frame(false), Some(FillAnswer::NotKept));
    assert!(r.texts().iter().any(|t| t == "no row0 yet"));
    // The row is in the last frame now: what was read moved, so the
    // fill runs, and finds it (backlog C84).
    assert_eq!(r.frame(false), Some(FillAnswer::Reads));
    assert!(r.texts().iter().any(|t| t == "found row0"));
    // Answered from then on: replayed.
    assert_eq!(r.frame(false), Some(FillAnswer::Replayed));
    assert_eq!(r.views.get(), 2);
}

#[test]
fn the_clock_is_never_the_same_twice() {
    let mut r = rig(Ext {
        rows: 1,
        read_clock: true,
        ..Default::default()
    });
    r.frame(false);
    assert_eq!(r.frame(false), Some(FillAnswer::Reads));
    assert_eq!(r.frame(false), Some(FillAnswer::Reads));
    assert_eq!(r.views.get(), 3);
}

#[test]
fn a_nested_instance_is_placed_again_and_replayed_by_its_own_lights() {
    let mut r = rig(Ext {
        rows: 2,
        nest: true,
        ..Default::default()
    });
    r.frame(false);
    assert_eq!((r.views.get(), r.inner_views.get()), (1, 1));
    let fresh = r.keys();
    assert_eq!(r.frame(false), Some(FillAnswer::Replayed));
    // Filled plainly while the outer was being kept, the inner keeps of its
    // own from the outer's first replay, and replays from the next.
    assert_eq!(
        (r.views.get(), r.inner_views.get()),
        (1, 2),
        "the outer was spared; the inner it placed again ran, and kept"
    );
    assert_eq!(r.frame(false), Some(FillAnswer::Replayed));
    assert_eq!((r.views.get(), r.inner_views.get()), (1, 2), "both spared now");
    assert_eq!(r.keys(), fresh);
    assert!(r.texts().iter().any(|t| t == "inner"));
    // The inner's nodes carry its origin, inside the outer's.
    assert!(r.keys().iter().any(|(_, o)| *o == OriginId(2)));
}

#[test]
fn a_fill_that_declares_of_the_frame_or_pushes_an_unknown_node_is_not_replayed() {
    for ext in [
        Ext {
            rows: 1,
            ask_frame: true,
            ..Default::default()
        },
        Ext {
            rows: 1,
            cells: true,
            ..Default::default()
        },
        Ext {
            rows: 1,
            fail: true,
            ..Default::default()
        },
    ] {
        let mut r = rig(ext);
        r.frame(false);
        assert_eq!(r.frame(false), Some(FillAnswer::NotReplayable));
        assert_eq!(r.frame(false), Some(FillAnswer::NotReplayable));
        assert_eq!(r.views.get(), 3);
    }
}

#[test]
fn a_replay_resolves_hover_for_its_own_frame() {
    // The row's `hover_bg` is declared, not read: the kept spec is the
    // declared one, and the replay paints it hovered or not as this frame
    // has it — no refill, no stale colour.
    let mut r = rig(Ext {
        rows: 2,
        ..Default::default()
    });
    r.frame(false);
    assert_eq!(r.frame(false), Some(FillAnswer::Replayed));
    let rect = r.rect_of("row1");
    r.core.handle_input(InputEvent::CursorMoved(Vec2::new(
        rect.x + 1.0,
        rect.y + 1.0,
    )));
    assert_eq!(r.frame(false), Some(FillAnswer::Replayed));
    let key = r.core.key_of("row1").unwrap();
    r.core.set_inspect(true);
    let bg = r
        .core
        .nodes()
        .iter()
        .find(|n| n.key == key)
        .map(|n| n.bg)
        .expect("the row");
    assert_eq!(bg, Color::hex(0x3355ffff), "painted hovered");
}

/// What a theme change does to a kept fill: one that read the palette
/// (through `Ui::theme` or a role by name) or the metrics is run again and
/// draws the new one; one that read neither is replayed; and a palette set
/// again to what it was is no change (backlog F155).
#[test]
fn a_fill_that_read_the_theme_or_metrics_runs_again_when_either_changes() {
    let pink = kui_core::Theme::light().with_accent(Color::hex(0xff00ffff));
    let theme = |core: &mut Core| core.set_theme(pink);
    let metrics = |core: &mut Core| {
        let mut m = *core.metrics();
        m.radius = 13.0;
        core.set_metrics(m);
    };
    type Change<'a> = &'a dyn Fn(&mut Core);
    let cases: [(Ext, Change<'_>, &str); 3] = [
        (
            Ext {
                rows: 1,
                read_theme: true,
                ..Default::default()
            },
            &theme,
            "ff00ffff",
        ),
        (
            Ext {
                rows: 1,
                read_token: true,
                ..Default::default()
            },
            &theme,
            "ff00ffff",
        ),
        (
            Ext {
                rows: 1,
                read_metrics: true,
                ..Default::default()
            },
            &metrics,
            "radius 13",
        ),
    ];
    for (ext, change, drawn) in cases {
        let mut r = rig(ext);
        r.frame(false);
        assert_eq!(r.frame(false), Some(FillAnswer::Replayed), "{drawn}");
        change(&mut r.core);
        assert_eq!(r.frame(false), Some(FillAnswer::Reads), "{drawn}");
        assert_eq!(r.views.get(), 2);
        assert!(r.texts().iter().any(|t| t == drawn), "{:?}", r.texts());
        assert_eq!(r.frame(false), Some(FillAnswer::Replayed), "{drawn}");
        // Set again, the same: nothing moved.
        change(&mut r.core);
        assert_eq!(r.frame(false), Some(FillAnswer::Replayed), "{drawn}");
        assert_eq!(r.views.get(), 2);
    }

    // A fill that read neither is replayed across both.
    let mut r = rig(Ext {
        rows: 2,
        ..Default::default()
    });
    r.frame(false);
    theme(&mut r.core);
    metrics(&mut r.core);
    assert_eq!(r.frame(false), Some(FillAnswer::Replayed));
    assert_eq!(r.views.get(), 1);
}
