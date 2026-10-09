//! A slot replayed by its host (ADR 0045, backlog F142): `Ui::slot_kept`
//! keeps what an extension's fill built, and `Ui::slot_replay` pushes it
//! again without asking the extension — when the params agree, the slot
//! is where it was, and nothing the fill read of the frame has moved.
//! Everything else here is the core refusing, and saying why.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use kui_core::{
    Color, Core, Extension, Extensions, InputEvent, Key, NodeSpec, OriginId, Size, Slot, SlotFill,
    TextStyle, Ui, UiEvent, Value, Vec2,
};

/// A stand-in extension that counts its views and draws what the test
/// asks: `rows` text rows in a keyed column, each a hover-tracking box,
/// and — on request — reads a fact of the frame, declares a nested
/// slot, asks for a frame, or draws a `cells` grid.
#[derive(Default)]
struct Ext {
    views: Rc<Cell<usize>>,
    events: Rc<RefCell<Vec<Value>>>,
    rows: usize,
    /// Read `is_hovered` of the first row while drawing.
    read_hover: bool,
    /// Read the frame clock while drawing.
    read_clock: bool,
    /// Declare `inner/panel` between the rows.
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
    fn slots(&self) -> &[String] {
        &[]
    }
    fn view(&mut self, slot: &Slot<'_>, ui: &mut Ui<'_>) -> Result<(), String> {
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
            if self.read_clock {
                let now = ui.now();
                ui.text(&format!("{now:.1}"), TextStyle::new(12.0));
            }
            if self.ask_frame {
                ui.request_frame();
            }
            for i in 0..self.rows {
                let label = format!("row{i}");
                ui.text_in_keyed(&label, row(i), &format!("row {i}"), TextStyle::new(12.0));
                if self.nest && i == 0 {
                    ui.slot("inner/panel");
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

/// The inner extension a nested slot asks for: one text, counted.
#[derive(Default)]
struct Inner {
    views: Rc<Cell<usize>>,
}

impl Extension for Inner {
    fn name(&self) -> &str {
        "inner"
    }
    fn slots(&self) -> &[String] {
        std::slice::from_ref(&PANEL)
    }
    fn view(&mut self, _slot: &Slot<'_>, ui: &mut Ui<'_>) -> Result<(), String> {
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

static PANEL: String = String::new();

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
    // `Inner` lists its slot by a static the test cannot name in `slots`,
    // so it is loaded under the wildcard instead.
    exts.push_as(
        "inner",
        Box::new(InnerAny {
            inner: Inner {
                views: inner_views.clone(),
            },
            slots: vec![kui_core::ANY_SLOT.to_owned()],
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

/// `Inner` with a slots list of its own.
struct InnerAny {
    inner: Inner,
    slots: Vec<String>,
}

impl Extension for InnerAny {
    fn name(&self) -> &str {
        self.inner.name()
    }
    fn slots(&self) -> &[String] {
        &self.slots
    }
    fn view(&mut self, slot: &Slot<'_>, ui: &mut Ui<'_>) -> Result<(), String> {
        self.inner.view(slot, ui)
    }
    fn on_event(&mut self, ev: &UiEvent) -> Vec<Value> {
        self.inner.on_event(ev)
    }
}

fn params(title: &str) -> Value {
    Value::map([("title", Value::Str(title.into()))])
}

impl Rig {
    /// One frame: a root column, the slot — kept when `fresh`, replayed
    /// otherwise — then a host cell; `inside` puts the slot in a keyed
    /// box of its own instead of at the root. Answers what `slot_replay`
    /// said (`None` for a `slot_kept` frame).
    fn frame_with(&mut self, fresh: bool, title: &str, inside: bool) -> Option<SlotFill> {
        let p = params(title);
        let mut ui = self
            .core
            .frame_with(Size::new(400.0, 300.0), 1.0, &mut self.exts);
        ui.configure_root(NodeSpec::column().fill());
        let place = |ui: &mut Ui<'_>| {
            if fresh {
                assert!(ui.slot_kept("outer/root", &p));
                None
            } else {
                ui.slot_replay("outer/root", &p)
            }
        };
        let fill = if inside {
            let mut fill = None;
            ui.with_keyed("other", NodeSpec::column(), |ui| fill = place(ui));
            fill
        } else {
            place(&mut ui)
        };
        ui.leaf(NodeSpec::row().size(40.0, 10.0));
        ui.finish();
        fill
    }

    fn frame(&mut self, fresh: bool) -> Option<SlotFill> {
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
fn a_replayed_slot_spares_the_extension_and_builds_the_same_tree() {
    let mut r = rig(Ext {
        rows: 3,
        ..Default::default()
    });
    r.frame(true);
    assert_eq!(r.views.get(), 1);
    // A second frame, fresh, for the tree to compare against.
    r.frame(true);
    let fresh = r.keys();
    let fresh_texts = r.texts();
    assert_eq!(r.views.get(), 2);

    assert_eq!(r.frame(false), Some(SlotFill::Replayed));
    assert_eq!(r.views.get(), 2, "the extension was not asked");
    assert_eq!(r.keys(), fresh, "the same nodes under the same keys");
    assert_eq!(r.texts(), fresh_texts, "and the same text");
    assert_eq!(r.core.slot_fill("outer/root"), Some(SlotFill::Replayed));
    // The panel, its title, and a box with a text per row.
    assert_eq!(r.core.slot_kept_nodes("outer/root"), Some(8));

    // Again and again.
    for _ in 0..3 {
        assert_eq!(r.frame(false), Some(SlotFill::Replayed));
    }
    assert_eq!(r.views.get(), 2);
}

#[test]
fn a_replayed_node_still_takes_input_under_the_extensions_origin() {
    let mut r = rig(Ext {
        rows: 2,
        ..Default::default()
    });
    r.frame(true);
    assert_eq!(r.frame(false), Some(SlotFill::Replayed));
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
fn a_fresh_fill_is_asked_for_and_says_why() {
    let mut r = rig(Ext {
        rows: 1,
        ..Default::default()
    });
    // Nothing kept yet: the first replay fills, and keeps.
    assert_eq!(r.frame(false), Some(SlotFill::NotKept));
    assert_eq!(r.views.get(), 1);
    assert_eq!(r.frame(false), Some(SlotFill::Replayed));

    // Other params: filled fresh, and the new params are what is kept.
    assert_eq!(
        r.frame_with(false, "another", false),
        Some(SlotFill::Params)
    );
    assert_eq!(r.views.get(), 2);
    assert!(r.texts().iter().any(|t| t == "another"));
    assert_eq!(
        r.frame_with(false, "another", false),
        Some(SlotFill::Replayed)
    );

    // Declared under another parent: the keys would differ, so fresh.
    assert_eq!(r.frame_with(false, "another", true), Some(SlotFill::Moved));
    assert_eq!(r.views.get(), 3);
    assert_eq!(
        r.frame_with(false, "another", true),
        Some(SlotFill::Replayed)
    );

    // A frame that fills it plainly, or not at all, forgets the kept one.
    {
        let mut ui = r.core.frame_with(Size::new(400.0, 300.0), 1.0, &mut r.exts);
        ui.slot_with("outer/root", &params("another"));
        ui.finish();
    }
    assert_eq!(
        r.frame_with(false, "another", true),
        Some(SlotFill::NotKept)
    );
    {
        let mut ui = r.core.frame_with(Size::new(400.0, 300.0), 1.0, &mut r.exts);
        ui.leaf(NodeSpec::row().size(40.0, 10.0));
        ui.finish();
    }
    assert_eq!(
        r.frame_with(false, "another", true),
        Some(SlotFill::NotKept)
    );
}

#[test]
fn a_fact_the_fill_read_that_moved_runs_it_again() {
    let mut r = rig(Ext {
        rows: 2,
        read_hover: true,
        ..Default::default()
    });
    r.frame(true);
    assert_eq!(r.frame(false), Some(SlotFill::Replayed));
    assert!(r.texts().iter().any(|t| t == "not hovered"));

    // The pointer onto the row the fill asked about: what it read moved.
    let rect = r.rect_of("row0");
    r.core.handle_input(InputEvent::CursorMoved(Vec2::new(
        rect.x + 1.0,
        rect.y + 1.0,
    )));
    assert_eq!(r.frame(false), Some(SlotFill::Reads));
    assert!(
        r.core
            .slot_fill_why("outer/root")
            .is_some_and(|w| w.starts_with("Hover(")),
        "{:?}",
        r.core.slot_fill_why("outer/root")
    );
    assert_eq!(r.views.get(), 2);
    assert!(r.texts().iter().any(|t| t == "hovered"));
    // Still there: replayed, hovered.
    assert_eq!(r.frame(false), Some(SlotFill::Replayed));
    // Away again.
    r.core
        .handle_input(InputEvent::CursorMoved(Vec2::new(390.0, 290.0)));
    assert_eq!(r.frame(false), Some(SlotFill::Reads));
    assert!(r.texts().iter().any(|t| t == "not hovered"));
}

#[test]
fn the_clock_is_never_the_same_twice() {
    let mut r = rig(Ext {
        rows: 1,
        read_clock: true,
        ..Default::default()
    });
    r.frame(true);
    assert_eq!(r.frame(false), Some(SlotFill::Reads));
    assert_eq!(r.frame(false), Some(SlotFill::Reads));
    assert_eq!(r.views.get(), 3);
}

#[test]
fn a_nested_slot_is_declared_again_and_filled_fresh() {
    let mut r = rig(Ext {
        rows: 2,
        nest: true,
        ..Default::default()
    });
    r.frame(true);
    assert_eq!((r.views.get(), r.inner_views.get()), (1, 1));
    let fresh = r.keys();
    assert_eq!(r.frame(false), Some(SlotFill::Replayed));
    assert_eq!(
        (r.views.get(), r.inner_views.get()),
        (1, 2),
        "the outer was spared, the inner ran"
    );
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
        r.frame(true);
        assert_eq!(r.frame(false), Some(SlotFill::NotReplayable));
        assert_eq!(r.frame(false), Some(SlotFill::NotReplayable));
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
    r.frame(true);
    assert_eq!(r.frame(false), Some(SlotFill::Replayed));
    let rect = r.rect_of("row1");
    r.core.handle_input(InputEvent::CursorMoved(Vec2::new(
        rect.x + 1.0,
        rect.y + 1.0,
    )));
    assert_eq!(r.frame(false), Some(SlotFill::Replayed));
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

#[test]
fn a_slot_kept_inside_a_kept_fill_is_a_plain_slot() {
    // The nested extension's own `slot_replay` has nothing to replay from
    // inside a fill being kept: it fills, answers `NotKept`, and the
    // outer's journal is whole.
    struct Nester {
        inner_fill: Rc<Cell<Option<SlotFill>>>,
    }
    impl Extension for Nester {
        fn name(&self) -> &str {
            "nester"
        }
        fn slots(&self) -> &[String] {
            &[]
        }
        fn view(&mut self, _slot: &Slot<'_>, ui: &mut Ui<'_>) -> Result<(), String> {
            ui.with_keyed("box", NodeSpec::column(), |ui| {
                self.inner_fill
                    .set(ui.slot_replay("inner/panel", &Value::Null));
            });
            Ok(())
        }
        fn on_event(&mut self, _ev: &UiEvent) -> Vec<Value> {
            Vec::new()
        }
    }
    let inner_fill = Rc::new(Cell::new(None));
    let inner_views = Rc::new(Cell::new(0));
    let mut exts = Extensions::new();
    exts.push_as(
        "nester",
        Box::new(Nester {
            inner_fill: inner_fill.clone(),
        }),
    )
    .unwrap();
    exts.push_as(
        "inner",
        Box::new(InnerAny {
            inner: Inner {
                views: inner_views.clone(),
            },
            slots: vec![kui_core::ANY_SLOT.to_owned()],
        }),
    )
    .unwrap();
    let mut core = Core::new();
    for i in 0..3 {
        let mut ui = core.frame_with(Size::new(200.0, 200.0), 1.0, &mut exts);
        let fill = ui.slot_replay("nester/root", &Value::Null);
        ui.finish();
        assert_eq!(
            fill,
            Some(if i == 0 {
                SlotFill::NotKept
            } else {
                SlotFill::Replayed
            })
        );
    }
    assert_eq!(inner_fill.get(), Some(SlotFill::NotKept));
    assert_eq!(inner_views.get(), 3, "the inner ran every frame");
}
