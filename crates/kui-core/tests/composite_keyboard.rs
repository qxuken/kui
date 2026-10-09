//! Composite keyboard patterns: one Tab stop, arrows inside it
//! (`docs/adr/0007-composite-keyboard-patterns.md`).
//!
//! Every claim of the ADR that a headless core can show: the ring
//! collapsing and where it enters, both arrow pairs, line motion in a
//! wrapped container, Home / End, wrap against clamp per role, type-ahead
//! with a clock and without one, Space extending a search against Space
//! pressing, activation on motion for `radio` and `tab` and its absence
//! for `menuItem` and `listItem`, orientation, and a composite inside a
//! modal.

use kui_core::testing::tab;
use kui_core::{
    Align, Core, EditKey, FloatConfig, InputEvent, Key, Mods, NodeSpec, Role, Size, TextStyle, Ui,
    Value,
};

const VIEW: Size = Size { w: 400.0, h: 300.0 };

/// One composite container of `role` holding `items`, each focusable and
/// named by its own text, with a plain button after it so the ring has
/// somewhere else to go. Returns the item keys and the button's.
fn build(
    ui: &mut Ui<'_>,
    container: Role,
    item: Role,
    names: &[&str],
    selected: usize,
) -> (Vec<Key>, Key) {
    ui.configure_root(NodeSpec::column().fill().gap(4.0));
    let mut keys = Vec::new();
    ui.with_keyed("set", NodeSpec::row().role(container).gap(4.0), |ui| {
        for (i, name) in names.iter().enumerate() {
            keys.push(
                ui.text_in_keyed(
                    name,
                    NodeSpec::row()
                        .role(item)
                        .focusable()
                        .selected(i == selected)
                        .on_click(Value::str(*name))
                        .pad(4.0),
                    name,
                    TextStyle::new(12.0),
                ),
            );
        }
    });
    let after = ui.leaf_keyed("after", NodeSpec::row().on_click("after").label("After"));
    (keys, after)
}

fn frame(
    core: &mut Core,
    container: Role,
    item: Role,
    names: &[&str],
    selected: usize,
) -> (Vec<Key>, Key) {
    let mut ui = core.frame(VIEW, 1.0);
    let keys = build(&mut ui, container, item, names, selected);
    ui.finish();
    keys
}

fn press(core: &mut Core, ek: EditKey) -> Vec<String> {
    let out = core.handle_input(InputEvent::Key(ek, Mods::default()));
    out.iter()
        .filter_map(|e| e.payload.as_str().map(str::to_string))
        .collect()
}

fn typed(core: &mut Core, s: &str) -> Vec<String> {
    let out = core.handle_input(InputEvent::Text(s.to_string()));
    out.iter()
        .filter_map(|e| e.payload.as_str().map(str::to_string))
        .collect()
}

/// The whole point: three tabs are one Tab stop, not three, and the stop
/// is the selected one — so Tab, Tab leaves the composite entirely.
#[test]
fn a_composite_is_one_tab_stop() {
    let mut core = Core::new();
    let (tabs, after) = frame(
        &mut core,
        Role::TabList,
        Role::Tab,
        &["General", "Network", "About"],
        1,
    );
    tab(&mut core, false);
    // Entry is the selected item, not the first.
    assert_eq!(core.focus(), Some(tabs[1]));
    tab(&mut core, false);
    assert_eq!(core.focus(), Some(after));
    // And back: the stop is where the user was, so Shift-Tab returns to
    // the item focus left from rather than to the selected one.
    core.handle_input(InputEvent::Key(EditKey::Tab, Mods::NONE.with_shift()));
    assert_eq!(core.focus(), Some(tabs[1]));
}

/// A `group` of focusable boxes is not a composite: four stops, as today.
#[test]
fn only_the_four_pairs_collapse() {
    let mut core = Core::new();
    let (items, after) = frame(&mut core, Role::Group, Role::Button, &["a", "b", "c"], 0);
    for (n, want) in items.iter().chain([&after]).enumerate() {
        tab(&mut core, false);
        assert_eq!(core.focus(), Some(*want), "stop {n}");
    }
}

/// Decision 2's line between the two things kui spells `list`: a picker's
/// rows are focusable themselves, so the list collapses; a navigation
/// list's rows are not, so its links keep their own stops.
#[test]
fn a_list_is_a_composite_only_when_its_rows_are_focusable() {
    let mut core = Core::new();
    let mut ui = core.frame(VIEW, 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let mut links = Vec::new();
    ui.with_keyed("nav", NodeSpec::column().role(Role::List), |ui| {
        for name in ["one", "two"] {
            ui.with_keyed(name, NodeSpec::row().role(Role::ListItem), |ui| {
                links.push(ui.leaf_keyed("link", NodeSpec::row().role(Role::Link).label(name)));
            });
        }
    });
    ui.finish();
    for (n, want) in links.iter().enumerate() {
        tab(&mut core, false);
        assert_eq!(core.focus(), Some(*want), "link {n}");
    }
}

/// Both arrow pairs move, whatever the container's `dir` says: the
/// perpendicular pair costs nothing, and refusing it would turn a
/// mis-derived axis into a keyboard dead end.
#[test]
fn both_arrow_pairs_move_inside_a_composite() {
    let mut core = Core::new();
    let (t, _) = frame(&mut core, Role::TabList, Role::Tab, &["a", "b", "c"], 0);
    tab(&mut core, false);
    assert_eq!(core.focus(), Some(t[0]));
    press(&mut core, EditKey::Right);
    assert_eq!(core.focus(), Some(t[1]));
    press(&mut core, EditKey::Down);
    assert_eq!(core.focus(), Some(t[2]));
    press(&mut core, EditKey::Left);
    assert_eq!(core.focus(), Some(t[1]));
    press(&mut core, EditKey::Up);
    assert_eq!(core.focus(), Some(t[0]));
}

/// Home and End go to the first and last item, from anywhere.
#[test]
fn home_and_end_reach_the_ends() {
    let mut core = Core::new();
    let (t, _) = frame(&mut core, Role::TabList, Role::Tab, &["a", "b", "c"], 1);
    tab(&mut core, false);
    press(&mut core, EditKey::End);
    assert_eq!(core.focus(), Some(t[2]));
    press(&mut core, EditKey::Home);
    assert_eq!(core.focus(), Some(t[0]));
}

/// A choice among *n* wraps; a list clamps, because a list can be
/// windowed and its last item is not its last row.
#[test]
fn a_choice_wraps_and_a_sequence_clamps() {
    let mut core = Core::new();
    let (t, _) = frame(&mut core, Role::TabList, Role::Tab, &["a", "b", "c"], 0);
    tab(&mut core, false);
    press(&mut core, EditKey::Left);
    assert_eq!(core.focus(), Some(t[2]), "a tab list wraps");
    press(&mut core, EditKey::Right);
    assert_eq!(core.focus(), Some(t[0]));

    let mut core = Core::new();
    let (r, _) = frame(&mut core, Role::List, Role::ListItem, &["a", "b", "c"], 0);
    tab(&mut core, false);
    press(&mut core, EditKey::Up);
    assert_eq!(core.focus(), Some(r[0]), "a list clamps");
    press(&mut core, EditKey::End);
    press(&mut core, EditKey::Down);
    assert_eq!(core.focus(), Some(r[2]), "and at the other end");
}

/// `radio` and `tab` activate on motion — the item's own click payload,
/// so an app that handles clicks handles arrows with no new code — and
/// `menuItem` and `listItem` do not.
#[test]
fn motion_activates_only_where_selection_follows_focus() {
    for (container, item, activates) in [
        (Role::TabList, Role::Tab, true),
        (Role::RadioGroup, Role::Radio, true),
        (Role::Menu, Role::MenuItem, false),
        (Role::List, Role::ListItem, false),
    ] {
        let mut core = Core::new();
        let (items, _) = frame(&mut core, container, item, &["a", "b", "c"], 0);
        tab(&mut core, false);
        let events = press(&mut core, EditKey::Right);
        assert_eq!(core.focus(), Some(items[1]), "{item:?} moved");
        assert_eq!(
            events,
            if activates {
                vec!["b".to_string()]
            } else {
                vec![]
            },
            "{item:?} activation on motion"
        );
        // Enter activates any of them, unchanged.
        assert_eq!(press(&mut core, EditKey::Enter), vec!["b".to_string()]);
    }
}

/// The core moves focus and never writes `selected`: the frame is data the
/// app re-declares, so a view that wants selection to follow focus writes
/// it from focus itself.
#[test]
fn the_core_never_writes_selected() {
    let mut core = Core::new();
    let (t, _) = frame(&mut core, Role::TabList, Role::Tab, &["a", "b", "c"], 0);
    tab(&mut core, false);
    press(&mut core, EditKey::Right);
    // The view still says `a`, and the access tree still says `a`.
    frame(&mut core, Role::TabList, Role::Tab, &["a", "b", "c"], 0);
    let tree = core.access_tree();
    let selected: Vec<Option<bool>> = tree
        .nodes
        .iter()
        .filter(|n| n.role == Role::Tab)
        .map(|n| n.selected)
        .collect();
    assert_eq!(selected, vec![Some(true), Some(false), Some(false)]);
    assert_eq!(core.focus(), Some(t[1]));
}

/// The container's `dir` reaches the access node as an orientation, and
/// "3 of 7" now covers a radio group too.
#[test]
fn orientation_and_set_positions_reach_the_access_node() {
    let mut core = Core::new();
    let mut ui = core.frame(VIEW, 1.0);
    build(&mut ui, Role::RadioGroup, Role::Radio, &["a", "b"], 0);
    ui.finish();
    let tree = core.access_tree();
    let group = tree
        .nodes
        .iter()
        .find(|n| n.role == Role::RadioGroup)
        .expect("a radio group");
    assert_eq!(group.orientation, Some(kui_core::Orientation::Horizontal));
    assert_eq!(group.set_size, Some(2));
    let radios: Vec<Option<usize>> = tree
        .nodes
        .iter()
        .filter(|n| n.role == Role::Radio)
        .map(|n| n.pos_in_set)
        .collect();
    assert_eq!(radios, vec![Some(0), Some(1)]);
}

/// A column of items reports a vertical orientation, from the same `dir`.
#[test]
fn a_column_composite_reports_vertical() {
    let mut core = Core::new();
    let mut ui = core.frame(VIEW, 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.with_keyed("set", NodeSpec::column().role(Role::Menu), |ui| {
        for name in ["a", "b"] {
            ui.text_in_keyed(
                name,
                NodeSpec::row().role(Role::MenuItem),
                name,
                TextStyle::new(12.0),
            );
        }
    });
    ui.finish();
    let tree = core.access_tree();
    let menu = tree.nodes.iter().find(|n| n.role == Role::Menu).unwrap();
    assert_eq!(menu.orientation, Some(kui_core::Orientation::Vertical));
}

/// Type-ahead with a clock: consecutive characters extend a buffer, and
/// the buffer ages out a second later.
#[test]
fn type_ahead_extends_and_ages() {
    let mut core = Core::new();
    core.set_time(0.0);
    let rows = &["Inbox", "Drafts", "Sent"];
    let (r, _) = frame(&mut core, Role::List, Role::ListItem, rows, 0);
    tab(&mut core, false);
    assert_eq!(core.focus(), Some(r[0]));
    typed(&mut core, "d");
    assert_eq!(core.focus(), Some(r[1]));
    // "dr" still names Drafts: the search wraps round to the focused item.
    frame(&mut core, Role::List, Role::ListItem, rows, 0);
    typed(&mut core, "r");
    assert_eq!(core.focus(), Some(r[1]));
    // Past a second, the next character starts over — so "s" is Sent and
    // not a continuation of "dr", which names nothing.
    core.set_time(2.0);
    frame(&mut core, Role::List, Role::ListItem, rows, 0);
    typed(&mut core, "s");
    assert_eq!(core.focus(), Some(r[2]));
}

/// A window parks between frames, so after a quiet second the next
/// keystroke can come before any frame has aged the buffer. The driver
/// stamps the clock for the input (backlog F139) and the keystroke ages
/// the buffer itself: "s" is Sent and not "ds", and a Space after the
/// pause presses the item rather than extending a search gone stale.
#[test]
fn type_ahead_ages_at_the_keystroke_with_no_frame_between() {
    let mut core = Core::new();
    core.set_time(0.0);
    let rows = &["Inbox", "Drafts", "Sent"];
    let (r, _) = frame(&mut core, Role::List, Role::ListItem, rows, 0);
    tab(&mut core, false);
    typed(&mut core, "d");
    frame(&mut core, Role::List, Role::ListItem, rows, 0);
    assert_eq!(core.focus(), Some(r[1]));
    core.set_time(2.0);
    typed(&mut core, "s");
    assert_eq!(core.focus(), Some(r[2]));
    frame(&mut core, Role::List, Role::ListItem, rows, 0);
    core.set_time(4.0);
    assert_eq!(typed(&mut core, " "), vec!["Sent".to_string()]);
}

/// With no clock every keystroke starts a fresh search: the useful half of
/// type-ahead, and a behaviour a headless driver can rely on.
#[test]
fn type_ahead_without_a_clock_starts_fresh() {
    let mut core = Core::new();
    let (r, _) = frame(
        &mut core,
        Role::List,
        Role::ListItem,
        &["Inbox", "Drafts", "Sent"],
        0,
    );
    tab(&mut core, false);
    typed(&mut core, "d");
    assert_eq!(core.focus(), Some(r[1]));
    // Not "ds" (which names nothing) — "s".
    typed(&mut core, "s");
    assert_eq!(core.focus(), Some(r[2]));
}

/// Space presses the item with no search under way, and extends one that
/// is: the only interaction between type-ahead and ADR 0002's
/// Space-activates.
#[test]
fn space_presses_or_extends() {
    let mut core = Core::new();
    core.set_time(0.0);
    let rows = &["Inbox", "New mail", "Sent"];
    let (r, _) = frame(&mut core, Role::List, Role::ListItem, rows, 0);
    tab(&mut core, false);
    assert_eq!(typed(&mut core, " "), vec!["Inbox".to_string()]);
    // A search under way: Space is part of the name, not a press.
    typed(&mut core, "n");
    assert_eq!(core.focus(), Some(r[1]));
    frame(&mut core, Role::List, Role::ListItem, rows, 0);
    assert!(
        typed(&mut core, " ").is_empty(),
        "Space extended the search"
    );
    frame(&mut core, Role::List, Role::ListItem, rows, 0);
    typed(&mut core, "m");
    assert_eq!(core.focus(), Some(r[1]), "\"new m\" still names it");
}

/// A wrapped container's cross-axis pair moves by a line, keeping the
/// position within it; the main-axis pair still moves by one item.
#[test]
fn a_wrapped_container_moves_by_line() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(120.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let mut keys = Vec::new();
    // 50px items in a 120px row: two per line, so a, b / c, d / e.
    ui.with_keyed(
        "set",
        NodeSpec::row()
            .role(Role::List)
            .width(kui_core::Sizing::Grow(1.0))
            .wrap(),
        |ui| {
            for name in ["a", "b", "c", "d", "e"] {
                keys.push(
                    ui.leaf_keyed(
                        name,
                        NodeSpec::row()
                            .role(Role::ListItem)
                            .focusable()
                            .width(kui_core::Sizing::Fixed(50.0))
                            .height(kui_core::Sizing::Fixed(10.0))
                            .label(name),
                    ),
                );
            }
        },
    );
    ui.finish();
    tab(&mut core, false);
    assert_eq!(core.focus(), Some(keys[0]));
    press(&mut core, EditKey::Down);
    assert_eq!(core.focus(), Some(keys[2]), "a line down from a");
    press(&mut core, EditKey::Right);
    assert_eq!(core.focus(), Some(keys[3]), "one item along");
    press(&mut core, EditKey::Down);
    // The last line holds one item, so the position clamps into it.
    assert_eq!(core.focus(), Some(keys[4]));
    press(&mut core, EditKey::Up);
    assert_eq!(core.focus(), Some(keys[2]));
}

/// Modal scoping is untouched: the ring is computed inside the scope and a
/// composite inside it collapses the same way.
#[test]
fn a_composite_inside_a_modal_collapses_too() {
    let mut core = Core::new();
    let mut ui = core.frame(VIEW, 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.leaf_keyed("behind", NodeSpec::row().on_click("behind").label("Behind"));
    let mut tabs = Vec::new();
    let mut ok = Key::ROOT;
    ui.with_keyed(
        "dialog",
        NodeSpec::column()
            .float(FloatConfig::viewport().at(Align::Center, Align::Center))
            .modal("d")
            .label("Pick"),
        |ui| {
            ui.with_keyed("tabs", NodeSpec::row().role(Role::TabList), |ui| {
                for name in ["a", "b"] {
                    tabs.push(ui.leaf_keyed(
                        name,
                        NodeSpec::row().role(Role::Tab).focusable().label(name),
                    ));
                }
            });
            ok = ui.leaf_keyed("ok", NodeSpec::row().on_click("ok").label("OK"));
        },
    );
    ui.finish();
    // Focus entered the dialog on its ring's first stop: the tab list's
    // one stop, not its first tab as one of two.
    assert_eq!(core.focus(), Some(tabs[0]));
    tab(&mut core, false);
    assert_eq!(core.focus(), Some(ok));
    tab(&mut core, false);
    assert_eq!(core.focus(), Some(tabs[0]), "the ring is two stops");
    press(&mut core, EditKey::Right);
    assert_eq!(core.focus(), Some(tabs[1]));
}

/// Decision 5's line, drawn at the item and not at the container: a
/// focusable node inside an item is unreachable and the core says so,
/// while one inside the container but outside every item — a "+" at the
/// end of a tab bar — is reachable and reported by nothing.
#[test]
fn focusable_inside_an_item_is_reported_and_beside_one_is_not() {
    let mut core = Core::new();
    let mut ui = core.frame(VIEW, 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let mut buried = Key::ROOT;
    let mut plus = Key::ROOT;
    ui.with_keyed("rows", NodeSpec::column().role(Role::List), |ui| {
        ui.with_keyed(
            "row",
            NodeSpec::row()
                .role(Role::ListItem)
                .focusable()
                .label("Row"),
            |ui| {
                buried = ui.leaf_keyed("delete", NodeSpec::row().on_click("del").label("Delete"));
            },
        );
        plus = ui.leaf_keyed("add", NodeSpec::row().on_click("add").label("Add"));
    });
    ui.finish();
    let warnings = core.take_warnings();
    let codes: Vec<&str> = warnings.iter().map(|w| w.code).collect();
    assert_eq!(codes, [kui_core::diag::FOCUSABLE_INSIDE_ITEM]);
    assert_eq!(warnings[0].key, buried);
    // And the "+" keeps its own Tab stop, beside the composite's one.
    tab(&mut core, false);
    tab(&mut core, false);
    assert_eq!(core.focus(), Some(plus));
}

/// Backlog F96: a `radio` with no `radioGroup` above it and a `tab` with
/// no `tabList` are each reported once, the stock radio with them; the
/// same items inside their containers, however deep, are not, and a lone
/// `menuItem` or `listItem` is left alone.
#[test]
fn a_radio_or_tab_outside_its_container_is_reported() {
    use kui_core::diag::ITEM_OUTSIDE_CONTAINER;
    let mut core = Core::new();
    let mut lone = Vec::new();
    for _ in 0..3 {
        lone.clear();
        let mut ui = core.frame(VIEW, 1.0);
        ui.configure_root(NodeSpec::column().fill());
        for (name, role) in [
            ("radio", Role::Radio),
            ("tab", Role::Tab),
            ("menu item", Role::MenuItem),
            ("list item", Role::ListItem),
        ] {
            let key = ui.leaf_keyed(
                name,
                NodeSpec::row()
                    .role(role)
                    .on_click(Value::str(name))
                    .label(name),
            );
            if matches!(role, Role::Radio | Role::Tab) {
                lone.push(key);
            }
        }
        lone.push(kui_core::widgets::radio(&mut ui, "Stock", false, "stock"));
        // Inside their containers, with a plain box between.
        ui.with_keyed(
            "group",
            NodeSpec::row().role(Role::RadioGroup).label("Group"),
            |ui| {
                ui.with(NodeSpec::column(), |ui| {
                    kui_core::widgets::radio(ui, "In", true, "in");
                });
            },
        );
        ui.with_keyed(
            "tabs",
            NodeSpec::row().role(Role::TabList).label("Tabs"),
            |ui| {
                ui.with(NodeSpec::row(), |ui| {
                    ui.leaf_keyed(
                        "t",
                        NodeSpec::row().role(Role::Tab).on_click("t").label("T"),
                    );
                });
            },
        );
        // The other pair's container does not count.
        ui.with_keyed(
            "wrong",
            NodeSpec::row().role(Role::TabList).label("Wrong"),
            |ui| {
                lone.push(kui_core::widgets::radio(ui, "Astray", false, "astray"));
            },
        );
        ui.finish();
    }
    let warnings = core.take_warnings();
    let codes: Vec<&str> = warnings.iter().map(|w| w.code).collect();
    assert_eq!(codes, [ITEM_OUTSIDE_CONTAINER; 4], "{warnings:#?}");
    let keys: Vec<Key> = warnings.iter().map(|w| w.key).collect();
    assert_eq!(keys, lone, "once per node over three frames");
    assert!(warnings[0].message.contains("`radioGroup`"));
    assert!(warnings[1].message.contains("`tabList`"));
}
