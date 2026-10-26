//! Props by condition (ADR 0069): `when` entries over the node's own size,
//! the window's, the system's settings and the node's state, resolved by
//! the core; `min_column`; `hidden`; the `query` event.

use kui_core::{
    Appearance, Color, Cond, ContrastPref, Core, Dir, EventKind, InputEvent, Key, MotionPref,
    NodeInfo, NodeSpec, Size, Sizing, SystemEnv, Value, Vec2, diag,
};

fn node<'a>(nodes: &'a [NodeInfo], label: &str) -> &'a NodeInfo {
    nodes
        .iter()
        .find(|n| n.label.as_deref() == Some(label))
        .unwrap_or_else(|| panic!("no node {label}"))
}

/// One frame of `view` at `size`, the snapshot of its nodes, and what it
/// raised.
fn frame(
    core: &mut Core,
    size: Size,
    view: impl FnOnce(&mut kui_core::Ui<'_>),
) -> (Vec<NodeInfo>, Vec<kui_core::UiEvent>) {
    core.set_inspect(true);
    let mut ui = core.frame(size, 1.0);
    view(&mut ui);
    ui.finish();
    (core.nodes(), core.take_pending_events())
}

/// A row of three 100 px boxes that turns into a column under 400 px of
/// its own width, the band holding it there up to 420.
fn responsive(ui: &mut kui_core::Ui<'_>) {
    ui.with_keyed(
        "row",
        NodeSpec::row()
            .grow_width()
            .gap(10.0)
            .when(Cond::width_below(400.0).band(20.0), |p| {
                p.dir(Dir::Column).gap(4.0)
            })
            .on_query(Value::str("q")),
        |ui| {
            for name in ["a", "b", "c"] {
                ui.leaf_keyed(name, NodeSpec::row().size(100.0, 30.0));
            }
        },
    );
}

#[test]
fn a_width_entry_lays_the_children_out_in_the_frame_it_matches() {
    let mut core = Core::new();
    let (nodes, events) = frame(&mut core, Size::new(600.0, 400.0), responsive);
    let row = node(&nodes, "row");
    assert_eq!((row.dir, row.gap), (Dir::Row, 10.0));
    assert_eq!(node(&nodes, "b").rect.x, 110.0);
    // The first sight reports what matched: nothing.
    let q: Vec<_> = events
        .iter()
        .filter(|e| e.event == EventKind::Query)
        .collect();
    assert_eq!(q.len(), 1);
    assert_eq!(
        q[0].payload.get("query").and_then(|q| q.get("matched")),
        Some(&Value::List(vec![]))
    );

    // Narrower than 400: a column in this very frame, no frame late.
    let (nodes, events) = frame(&mut core, Size::new(380.0, 400.0), responsive);
    let row = node(&nodes, "row");
    assert_eq!((row.dir, row.gap), (Dir::Column, 4.0));
    assert_eq!(
        node(&nodes, "b").rect,
        kui_core::Rect::new(0.0, 34.0, 100.0, 30.0)
    );
    let q: Vec<_> = events
        .iter()
        .filter(|e| e.event == EventKind::Query)
        .collect();
    assert_eq!(q.len(), 1);
    assert_eq!(
        q[0].payload.get("query").and_then(|q| q.get("matched")),
        Some(&Value::List(vec![Value::Int(0)]))
    );
    let row = core.key_of("row").unwrap();
    assert_eq!(core.matched_rules(row), 1);

    // Inside the band it holds; past it, it lets go.
    let (nodes, events) = frame(&mut core, Size::new(410.0, 400.0), responsive);
    assert_eq!(node(&nodes, "row").dir, Dir::Column, "the band holds it");
    assert!(
        events.iter().all(|e| e.event != EventKind::Query),
        "nothing moved"
    );
    let (nodes, _) = frame(&mut core, Size::new(420.0, 400.0), responsive);
    assert_eq!(node(&nodes, "row").dir, Dir::Row, "out at the band's edge");
    assert!(core.take_warnings().is_empty());
}

#[test]
fn a_height_entry_takes_one_more_pass_on_the_frame_it_changes() {
    let view = |ui: &mut kui_core::Ui<'_>| {
        ui.with_keyed(
            "panel",
            NodeSpec::column()
                .grow_width()
                .grow_height()
                .pad(20.0)
                .when(Cond::height_below(200.0), |p| p.pad(2.0)),
            |ui| {
                ui.leaf_keyed("child", NodeSpec::row().grow_width().height(10.0));
            },
        );
    };
    let mut core = Core::new();
    let (nodes, _) = frame(&mut core, Size::new(300.0, 300.0), view);
    assert_eq!(node(&nodes, "child").rect.w, 260.0);
    // The height moved under the threshold: the padding, which changes the
    // child's width, is in this frame's layout.
    let (nodes, _) = frame(&mut core, Size::new(300.0, 150.0), view);
    let child = node(&nodes, "child");
    assert_eq!((child.rect.x, child.rect.w), (2.0, 296.0));
}

#[test]
fn environment_and_window_entries_resolve_as_the_node_opens() {
    let view = |ui: &mut kui_core::Ui<'_>| {
        ui.leaf_keyed(
            "card",
            NodeSpec::row()
                .size(50.0, 50.0)
                .bg(Color::hex(0xffffffff))
                .when(Cond::Dark, |p| p.bg(Color::hex(0x000000ff)))
                .when(Cond::MoreContrast, |p| {
                    p.border(2.0, Color::hex(0xff0000ff))
                })
                .when(Cond::window_width_below(300.0), |p| p.width(20.0))
                .when(Cond::ReducedMotion.and(Cond::Dark), |p| p.opacity(0.5)),
        );
    };
    let mut core = Core::new();
    let (nodes, _) = frame(&mut core, Size::new(400.0, 400.0), view);
    let card = node(&nodes, "card");
    assert_eq!(
        (card.bg, card.border_w, card.rect.w),
        (Color::hex(0xffffffff), 0.0, 50.0)
    );
    core.set_system(SystemEnv {
        appearance: Appearance::Dark,
        contrast: ContrastPref::More,
        ..Default::default()
    });
    let (nodes, _) = frame(&mut core, Size::new(250.0, 400.0), view);
    let card = node(&nodes, "card");
    assert_eq!(card.bg, Color::hex(0x000000ff));
    assert_eq!(card.border_w, 2.0);
    assert_eq!(
        card.rect.w, 20.0,
        "a window entry may set the node's own width"
    );
    assert_eq!(card.opacity, 1.0, "both conditions must hold");
    core.env.system.motion = MotionPref::Reduced;
    let (nodes, _) = frame(&mut core, Size::new(250.0, 400.0), view);
    assert_eq!(node(&nodes, "card").opacity, 0.5);
}

#[test]
fn a_state_entry_follows_the_pointer_after_the_shorthands() {
    let view = |ui: &mut kui_core::Ui<'_>| {
        ui.leaf_keyed(
            "button",
            NodeSpec::row()
                .size(100.0, 40.0)
                .bg(Color::hex(0x111111ff))
                .hover_bg(Color::hex(0x222222ff))
                .on_click("go")
                .label("Go")
                .when(Cond::Hovered, |p| p.radius(8.0))
                .when(Cond::Pressed, |p| p.bg(Color::hex(0x333333ff))),
        );
    };
    let mut core = Core::new();
    let (nodes, _) = frame(&mut core, Size::new(300.0, 300.0), view);
    assert_eq!(node(&nodes, "button").radius, [0.0; 4]);
    core.handle_input(InputEvent::CursorMoved(Vec2::new(10.0, 10.0)));
    let (nodes, _) = frame(&mut core, Size::new(300.0, 300.0), view);
    let b = node(&nodes, "button");
    assert_eq!(b.radius, [8.0; 4]);
    assert_eq!(
        b.bg,
        Color::hex(0x222222ff),
        "the shorthand still paints the hover"
    );
    core.handle_input(InputEvent::mouse_down(1));
    let (nodes, _) = frame(&mut core, Size::new(300.0, 300.0), view);
    assert_eq!(
        node(&nodes, "button").bg,
        Color::hex(0x333333ff),
        "an explicit entry wins over the shorthand ahead of it"
    );
    assert!(core.take_warnings().is_empty());
}

#[test]
fn a_state_entry_without_a_click_still_tracks_the_pointer() {
    let view = |ui: &mut kui_core::Ui<'_>| {
        ui.leaf_keyed(
            "badge",
            NodeSpec::row()
                .size(40.0, 40.0)
                .when(Cond::Hovered, |p| p.bg(Color::hex(0xabcdefff))),
        );
    };
    let mut core = Core::new();
    frame(&mut core, Size::new(100.0, 100.0), view);
    core.handle_input(InputEvent::CursorMoved(Vec2::new(5.0, 5.0)));
    let (nodes, _) = frame(&mut core, Size::new(100.0, 100.0), view);
    assert_eq!(node(&nodes, "badge").bg, Color::hex(0xabcdefff));
}

#[test]
fn min_column_wraps_a_row_into_equal_columns() {
    let view = |ui: &mut kui_core::Ui<'_>| {
        ui.with_keyed(
            "grid",
            NodeSpec::row().width(330.0).gap(10.0).min_column(100.0),
            |ui| {
                for i in 0..5 {
                    ui.leaf_keyed(&format!("c{i}"), NodeSpec::row().width(20.0).height(30.0));
                }
            },
        );
    };
    let mut core = Core::new();
    let (nodes, _) = frame(&mut core, Size::new(400.0, 400.0), view);
    // floor((330 + 10) / (100 + 10)) = 3 columns of (330 - 20) / 3.
    let w = node(&nodes, "c0").rect.w;
    assert!((w - 310.0 / 3.0).abs() < 0.01, "{w}");
    assert_eq!(node(&nodes, "c3").rect.y, 30.0, "the fourth starts a line");
    let last = node(&nodes, "c4");
    assert!(
        (last.rect.w - w).abs() < 1e-4,
        "the last line does not stretch"
    );
    assert_eq!(
        node(&nodes, "grid").rect.h,
        60.0,
        "lines meet at `crossGap`, 0 here"
    );
}

#[test]
fn a_size_entry_hides_a_child_and_keeps_its_place_in_the_tree() {
    let view = |ui: &mut kui_core::Ui<'_>| {
        ui.with_keyed(
            "row",
            NodeSpec::row()
                .grow_width()
                .gap(10.0)
                .when(Cond::width_below(300.0), |p| p.hide("extra")),
            |ui| {
                ui.leaf_keyed("main", NodeSpec::row().size(100.0, 20.0));
                ui.leaf_keyed(
                    "extra",
                    NodeSpec::row()
                        .size(100.0, 20.0)
                        .on_click("x")
                        .label("Extra"),
                );
                ui.leaf_keyed("tail", NodeSpec::row().size(50.0, 20.0));
            },
        );
    };
    let mut core = Core::new();
    let (nodes, _) = frame(&mut core, Size::new(400.0, 100.0), view);
    assert_eq!(node(&nodes, "tail").rect.x, 220.0);
    let (nodes, _) = frame(&mut core, Size::new(250.0, 100.0), view);
    assert_eq!(node(&nodes, "tail").rect.x, 110.0, "no room, no gap");
    assert_eq!(node(&nodes, "extra").rect.w, 0.0);
    // Out of the Tab ring and the access tree.
    let extra = core.key_of("extra").unwrap();
    core.focus_next(true);
    assert_ne!(core.focus(), Some(extra));
    assert!(core.access_tree().nodes.iter().all(|n| n.key != extra));
}

#[test]
fn hidden_takes_a_node_out_of_the_frame() {
    let mut core = Core::new();
    let (nodes, _) = frame(&mut core, Size::new(400.0, 100.0), |ui| {
        ui.with(NodeSpec::row().gap(10.0), |ui| {
            ui.leaf_keyed("a", NodeSpec::row().size(100.0, 20.0).hidden(true));
            ui.leaf_keyed("b", NodeSpec::row().size(100.0, 20.0));
        });
    });
    assert_eq!(node(&nodes, "b").rect.x, 0.0);
}

#[test]
fn what_an_entry_may_not_set_is_refused_by_name() {
    let mut core = Core::new();
    frame(&mut core, Size::new(400.0, 100.0), |ui| {
        ui.leaf_keyed(
            "x",
            NodeSpec::row()
                .grow_width()
                .height(10.0)
                .when(Cond::width_below(100.0), |p| p.width(5.0).gap(2.0)),
        );
    });
    let w = core.take_warnings();
    assert!(
        w.iter()
            .any(|w| w.code == diag::RULE_PROP_REFUSED && w.message.contains("`width`")),
        "{w:?}"
    );
}

#[test]
fn a_fit_node_reading_its_own_width_is_refused_its_fit() {
    let mut core = Core::new();
    let (nodes, _) = frame(&mut core, Size::new(400.0, 100.0), |ui| {
        ui.with_keyed(
            "fit",
            NodeSpec::row().when(Cond::width_below(100.0), |p| p.gap(2.0)),
            |ui| {
                ui.leaf(NodeSpec::row().size(10.0, 10.0));
            },
        );
    });
    assert_eq!(node(&nodes, "fit").width, Sizing::Grow(1.0));
    assert!(
        core.take_warnings()
            .iter()
            .any(|w| w.code == diag::QUERY_ON_FIT)
    );
}

#[test]
fn a_node_with_no_rules_carries_one_pointer() {
    let spec = NodeSpec::row();
    assert!(spec.rules_ref().is_none() && spec.interact.is_none());
    assert_eq!(spec.rules_spec().entries().len(), 0);
    let _ = Key::ROOT;
}

/// A row `width` wide holding a 100 px label and a branch that needs 200
/// px, with a fallback.
fn lazy_row(ui: &mut kui_core::Ui<'_>, width: f32) {
    ui.with_keyed("row", NodeSpec::row().width(width).gap(10.0), |ui| {
        ui.leaf_keyed("label", NodeSpec::row().size(100.0, 20.0));
        ui.lazy_with(
            kui_core::LazySpec::needs(kui_core::Needs::w(200.0)).on_query(Value::str("lazy")),
            |ui| {
                ui.leaf_keyed("chart", NodeSpec::row().grow_width().height(50.0));
            },
        )
        .fallback(|ui| {
            ui.leaf_keyed("small", NodeSpec::row().size(30.0, 20.0));
        });
    });
}

/// Builds a frame as a runner does: once, and once more when a lazy
/// branch crossed.
fn run(core: &mut Core, width: f32) -> (Vec<NodeInfo>, usize) {
    let mut builds = 0;
    loop {
        builds += 1;
        frame(core, Size::new(600.0, 400.0), |ui| lazy_row(ui, width));
        if !core.rerun_owed() || builds == 2 {
            break;
        }
    }
    (core.nodes(), builds)
}

#[test]
fn a_lazy_branch_is_built_when_its_parent_offers_the_room() {
    let mut core = Core::new();
    // The first frame lays out, then builds.
    let (nodes, builds) = run(&mut core, 400.0);
    assert_eq!(builds, 2);
    let chart = node(&nodes, "chart");
    assert_eq!(chart.rect.w, 200.0, "laid out at exactly what it needs");
    assert!(nodes.iter().all(|n| n.label.as_deref() != Some("small")));
    // Steady: one build.
    let (_, builds) = run(&mut core, 400.0);
    assert_eq!(builds, 1);
    // 300 wide: 300 - 100 - 10 = 190 < 200, so the fallback, this frame.
    let (nodes, builds) = run(&mut core, 300.0);
    assert_eq!(builds, 2, "the crossing builds the frame again");
    assert_eq!(node(&nodes, "small").rect.w, 30.0);
    assert!(nodes.iter().all(|n| n.label.as_deref() != Some("chart")));
    let (_, builds) = run(&mut core, 300.0);
    assert_eq!(builds, 1);
    assert!(core.take_warnings().is_empty());
}

#[test]
fn a_lazy_branch_reports_its_decision() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(600.0, 400.0), 1.0);
    lazy_row(&mut ui, 400.0);
    ui.finish();
    let decided = |events: &[kui_core::UiEvent]| -> Vec<Value> {
        events
            .iter()
            .filter(|e| e.event == EventKind::Query)
            .filter_map(|e| e.payload.get("query")?.get("matched").cloned())
            .collect()
    };
    assert_eq!(
        decided(&core.take_pending_events()),
        [Value::List(vec![Value::Int(0)])]
    );
    let mut ui = core.frame(Size::new(600.0, 400.0), 1.0);
    lazy_row(&mut ui, 250.0);
    ui.finish();
    assert_eq!(decided(&core.take_pending_events()), [Value::List(vec![])]);
}

#[test]
fn a_query_branch_follows_its_parents_width_and_refuses_state() {
    let view = |ui: &mut kui_core::Ui<'_>, width: f32| {
        ui.with_keyed("panel", NodeSpec::column().width(width), |ui| {
            ui.lazy_query(Cond::width_below(300.0).and(Cond::Hovered), |ui| {
                ui.leaf_keyed("narrow", NodeSpec::row().size(10.0, 10.0));
            })
            .fallback(|ui| {
                ui.leaf_keyed("wide", NodeSpec::row().size(10.0, 10.0));
            });
        });
    };
    let mut core = Core::new();
    for _ in 0..2 {
        frame(&mut core, Size::new(600.0, 400.0), |ui| view(ui, 200.0));
    }
    let (nodes, _) = frame(&mut core, Size::new(600.0, 400.0), |ui| view(ui, 200.0));
    assert!(nodes.iter().any(|n| n.label.as_deref() == Some("narrow")));
    assert!(
        core.take_warnings()
            .iter()
            .any(|w| w.code == diag::QUERY_STATE)
    );
}

/// A component whose view holds a lazy branch, counting its views.
struct Lazily {
    views: std::rc::Rc<std::cell::Cell<usize>>,
}

static LAZY_PANE: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| "pane".to_owned());

impl kui_core::Extension for Lazily {
    fn name(&self) -> &str {
        "lazily"
    }
    fn components(&self) -> &[String] {
        std::slice::from_ref(&LAZY_PANE)
    }
    fn view(
        &mut self,
        _inst: &kui_core::Instance<'_>,
        ui: &mut kui_core::Ui<'_>,
    ) -> Result<(), String> {
        self.views.set(self.views.get() + 1);
        ui.with_keyed("box", NodeSpec::row().grow_width(), |ui| {
            ui.lazy(kui_core::Needs::w(200.0), |ui| {
                ui.leaf_keyed("big", NodeSpec::row().size(50.0, 10.0));
            })
            .fallback(|ui| {
                ui.leaf_keyed("small", NodeSpec::row().size(5.0, 10.0));
            });
        });
        Ok(())
    }
    fn on_event(&mut self, _ev: &kui_core::UiEvent) -> Vec<Value> {
        Vec::new()
    }
}

#[test]
fn a_lazy_branch_is_replayed_with_its_instance_while_its_decision_holds() {
    let views = std::rc::Rc::new(std::cell::Cell::new(0));
    let mut exts = kui_core::Extensions::new();
    exts.push_as(
        "lazily",
        Box::new(Lazily {
            views: views.clone(),
        }),
    )
    .unwrap();
    let mut core = Core::new();
    let build = |core: &mut Core, exts: &mut kui_core::Extensions, w: f32| loop {
        let mut ui = core.frame_with(Size::new(w, 300.0), 1.0, exts);
        ui.configure_root(NodeSpec::column().fill());
        ui.component("lazily/pane", &Value::Null);
        ui.finish();
        if !core.rerun_owed() {
            break;
        }
    };
    build(&mut core, &mut exts, 400.0);
    let after_first = views.get();
    build(&mut core, &mut exts, 400.0);
    assert_eq!(views.get(), after_first, "replayed: the decision held");
    build(&mut core, &mut exts, 150.0);
    assert!(views.get() > after_first, "the decision moved: run again");
    core.set_inspect(true);
    build(&mut core, &mut exts, 150.0);
    assert!(
        core.nodes()
            .iter()
            .any(|n| n.label.as_deref() == Some("small"))
    );
}

#[test]
fn the_snapshot_flags_a_node_with_rules_a_lazy_branch_and_a_hidden_one() {
    let mut core = Core::new();
    let view = |ui: &mut kui_core::Ui<'_>| {
        ui.with_keyed(
            "row",
            NodeSpec::row()
                .grow_width()
                .when(Cond::Dark, |p| p.gap(2.0)),
            |ui| {
                ui.leaf_keyed("gone", NodeSpec::row().size(5.0, 5.0).hidden(true));
                ui.lazy(kui_core::Needs::w(10.0), |ui| {
                    ui.leaf(NodeSpec::row().size(1.0, 1.0));
                });
            },
        );
    };
    frame(&mut core, Size::new(200.0, 100.0), view);
    let (nodes, _) = frame(&mut core, Size::new(200.0, 100.0), view);
    assert!(node(&nodes, "row").flags.contains(&"rules"));
    assert!(node(&nodes, "gone").flags.contains(&"hidden"));
    assert!(nodes.iter().any(|n| n.flags.contains(&"lazy")));
}

/// The colours the frame drew its glyphs in, each once, in order.
fn glyph_colors(core: &mut Core) -> Vec<Color> {
    let (dl, _) = core.output();
    let mut out: Vec<Color> = Vec::new();
    for q in dl.quads.iter() {
        if matches!(
            q.kind,
            kui_core::QuadKind::GlyphMask | kui_core::QuadKind::GlyphSubpixel
        ) && !out.contains(&q.color)
        {
            out.push(q.color);
        }
    }
    out
}

/// `textColor` colours every text under a node that names none, the
/// nearest declaration winning, a text's own `color` over it, and the
/// theme's foreground where nothing above declares one (backlog C130).
#[test]
fn text_color_is_inherited_by_the_texts_that_name_none() {
    let (red, blue, own) = (
        Color::hex(0xcc2222ff),
        Color::hex(0x2244ccff),
        Color::hex(0x22aa44ff),
    );
    let mut core = Core::new();
    let fg = core.theme().fg;
    let mut ui = core.frame(Size::new(300.0, 200.0), 1.0);
    ui.with(NodeSpec::column().text_color(red), |ui| {
        ui.text("Red", kui_core::TextStyle::new(14.0));
        ui.with(NodeSpec::row().text_color(blue), |ui| {
            ui.text("Blue", kui_core::TextStyle::new(14.0));
            ui.text("Own", kui_core::TextStyle::new(14.0).color(own));
        });
    });
    ui.text("Plain", kui_core::TextStyle::new(14.0));
    ui.finish();
    assert_eq!(glyph_colors(&mut core), vec![red, blue, own, fg]);
}

/// A `when` entry sets it: a row recolours its label while hovered with
/// no text drawn twice — the danger menu item's label, which Kit drew
/// twice and showed one copy of by the row's hover group, since a text
/// has no box for a rule.
#[test]
fn a_state_entry_recolours_the_texts_under_its_node() {
    let (rest, hot) = (Color::hex(0xcc2222ff), Color::hex(0xffffffff));
    let view = |ui: &mut kui_core::Ui<'_>| {
        ui.with_keyed(
            "row",
            NodeSpec::row()
                .size(200.0, 30.0)
                .on_click("go")
                .label("Delete")
                .text_color(rest)
                .when(Cond::Hovered, |p| p.bg(rest).text_color(hot)),
            |ui| ui.text("Delete", kui_core::TextStyle::new(14.0)),
        );
    };
    let mut core = Core::new();
    frame(&mut core, Size::new(300.0, 100.0), view);
    assert_eq!(glyph_colors(&mut core), vec![rest]);
    core.handle_input(InputEvent::CursorMoved(Vec2::new(20.0, 10.0)));
    frame(&mut core, Size::new(300.0, 100.0), view);
    assert_eq!(glyph_colors(&mut core), vec![hot], "hovered");
    core.handle_input(InputEvent::CursorMoved(Vec2::new(250.0, 80.0)));
    frame(&mut core, Size::new(300.0, 100.0), view);
    assert_eq!(glyph_colors(&mut core), vec![rest], "and back");
    assert!(core.take_warnings().is_empty());
}
