//! Keyboard focus as data (`docs/adr/0002-keyboard-focus-as-data.md`): one
//! focus for every node, Tab walking every control in tree order, Enter /
//! Space pressing the focused one, arrows nudging a slider, a ring the
//! core draws for keyboard focus (or the node's `focus_bg`), `disabled`
//! nodes inert and skipped, and assistive technology landing on the same
//! focus a Tab press moves.

use kui_core::{
    AccessAction, AccessRequest, Color, Core, EditKey, EditOptions, InputEvent, Key, Mods,
    NodeSpec, Role, Size, Sizing, TextStyle, UiEvent, Value, Vec2, WindowButton,
};

const H: f32 = 20.0;

/// The keys of the frame's nodes, in tree order.
struct Keys {
    go: Key,
    save: Key,
    mute: Key,
    vol: Key,
    off: Key,
    hidden: Key,
    row: Key,
    name: Key,
    pane: Key,
    styled: Key,
}

const FOCUS_BG: Color = Color {
    r: 1.0,
    g: 0.0,
    b: 1.0,
    a: 1.0,
};

/// Thirteen rows of height `H`, so row `n` is centred at `y = n * H + 10`:
/// a titlebar with a close button (chrome), a heading, a button, an icon
/// button, a switch, a slider, a disabled button, a button inside
/// decoration, a plain box, a `focusable` row, an editor, a key sink and a
/// button with its own `focus_bg`. Repeating the frame repeats the
/// declaration on the sink when `declare_pane` says so.
fn frame(core: &mut Core, declare_pane: bool, autofocus_edit: bool) -> Keys {
    let mut ui = core.frame(Size::new(200.0, 300.0), 1.0);
    ui.window_title("Focus");
    ui.configure_root(NodeSpec::column().fill());
    let row = || {
        NodeSpec::row()
            .width(Sizing::Fixed(100.0))
            .height(Sizing::Fixed(H))
    };
    ui.with_keyed("titlebar", row().window_drag(), |ui| {
        ui.with_keyed(
            "close",
            NodeSpec::row()
                .width(Sizing::Fixed(H))
                .height(Sizing::Fixed(H))
                .window_button(WindowButton::Close),
            |_| {},
        );
    });
    ui.with_keyed("heading", row().role(Role::Heading), |ui| {
        ui.text("Controls", TextStyle::new(12.0))
    });
    let go = ui.with_keyed("go", row().on_click(Value::str("go")), |ui| {
        ui.text("Go", TextStyle::new(12.0))
    });
    let save = ui.with_keyed(
        "save",
        row().on_click(Value::str("save")).label("Save"),
        |_| {},
    );
    let mute = ui.with_keyed(
        "mute",
        row()
            .role(Role::Switch)
            .checked(true)
            .on_click(Value::str("mute"))
            .label("Mute"),
        |_| {},
    );
    let vol = ui.with_keyed(
        "vol",
        row()
            .role(Role::Slider)
            .on_drag(Value::str("vol"))
            .label("Volume")
            .value_now(3.0)
            .value_min(0.0)
            .value_max(10.0),
        |_| {},
    );
    let off = ui.with_keyed(
        "off",
        row()
            .on_click(Value::str("nope"))
            .disabled(true)
            .hover_bg(Color::WHITE)
            .label("Off"),
        |_| {},
    );
    let mut hidden = Key::ROOT;
    ui.with_keyed("deco", row().role(Role::None), |ui| {
        hidden = ui.with_keyed(
            "hidden",
            NodeSpec::row()
                .width(Sizing::Fixed(H))
                .height(Sizing::Fixed(H))
                .on_click(Value::str("hidden")),
            |_| {},
        );
    });
    ui.with_keyed("plain", row(), |_| {});
    let row_key = ui.with_keyed("row", row().focusable().label("Row"), |_| {});
    let name = ui.text_edit(
        "name",
        "hi",
        &EditOptions {
            autofocus: autofocus_edit,
            ..Default::default()
        },
        row().label("Name"),
    );
    let pane = ui.with_keyed("pane", row().on_key(Value::str("pane")), |_| {});
    if declare_pane {
        ui.take_key_focus(pane);
    }
    let styled = ui.with_keyed(
        "styled",
        row()
            .on_click(Value::str("styled"))
            .bg(Color::BLACK)
            .focus_bg(FOCUS_BG),
        |_| {},
    );
    ui.finish();
    Keys {
        go,
        save,
        mute,
        vol,
        off,
        hidden,
        row: row_key,
        name,
        pane,
        styled,
    }
}

fn tab(core: &mut Core, shift: bool) -> Vec<UiEvent> {
    core.handle_input(InputEvent::Key(
        EditKey::Tab,
        Mods {
            shift,
            ..Default::default()
        },
    ))
}

fn key(core: &mut Core, k: EditKey) -> Vec<UiEvent> {
    core.handle_input(InputEvent::Key(k, Mods::default()))
}

fn click_at(core: &mut Core, x: f32, y: f32) -> Vec<UiEvent> {
    let mut out = core.handle_input(InputEvent::CursorMoved(Vec2::new(x, y)));
    out.extend(core.handle_input(InputEvent::MouseDown(1)));
    out.extend(core.handle_input(InputEvent::MouseUp));
    out
}

fn payloads(evs: &[UiEvent]) -> Vec<String> {
    evs.iter()
        .map(|e| match e.payload.as_str() {
            Some(s) => s.to_string(),
            None => format!(
                "{}:{}",
                e.payload.get("kind").and_then(Value::as_str).unwrap_or("?"),
                e.payload
                    .get("action")
                    .and_then(Value::as_str)
                    .unwrap_or("")
            ),
        })
        .collect()
}

/// The default focus ring: a transparent quad with a 2px border, on top.
fn ring_count(core: &mut Core) -> usize {
    core.output()
        .0
        .quads
        .iter()
        .filter(|q| q.color.a == 0.0 && q.border_w == 2.0 && q.border_color.a > 0.0)
        .count()
}

#[test]
fn tab_walks_every_control_in_tree_order_and_wraps() {
    let mut core = Core::new();
    let k = frame(&mut core, false, false);
    assert_eq!(core.focus(), None);
    let ring = [k.go, k.save, k.mute, k.vol, k.row, k.name, k.pane];
    let mut seen = Vec::new();
    for _ in 0..ring.len() {
        tab(&mut core, false);
        seen.push(core.focus().unwrap());
    }
    assert_eq!(
        seen, ring,
        "chrome, the heading, the disabled button, decoration and plain boxes are not stops"
    );
    assert!(core.focus_visible(), "keyboard focus shows");
    assert!(!seen.contains(&k.off) && !seen.contains(&k.hidden));

    // A sink owns Tab; the app hands focus on with focus_next.
    tab(&mut core, false);
    assert_eq!(core.focus(), Some(k.pane), "a key sink keeps Tab");
    core.focus_next(true);
    assert_eq!(core.focus(), Some(k.styled));
    assert!(core.is_focused(k.styled) && core.key_focus() == Some(k.styled));

    // Wrapping, both ways; the editor mirrors into the edit store.
    tab(&mut core, false);
    assert_eq!(core.focus(), Some(k.go));
    tab(&mut core, true);
    assert_eq!(core.focus(), Some(k.styled));
    core.set_focus(Some(k.name));
    assert_eq!(core.edit.focused(), Some(k.name));
    tab(&mut core, true);
    assert_eq!(core.focus(), Some(k.row));
    assert_eq!(core.edit.focused(), None);
}

#[test]
fn enter_and_space_press_the_focused_control() {
    let mut core = Core::new();
    let k = frame(&mut core, false, false);
    core.set_focus(Some(k.go));
    assert_eq!(payloads(&key(&mut core, EditKey::Enter)), ["go"]);
    assert_eq!(
        payloads(&core.handle_input(InputEvent::Text(" ".into()))),
        ["go"]
    );
    core.set_focus(Some(k.mute));
    assert_eq!(
        payloads(&core.handle_input(InputEvent::Text(" ".into()))),
        ["mute"]
    );
    // The focusable row has no payload: reachable, nothing to press.
    core.set_focus(Some(k.row));
    assert!(key(&mut core, EditKey::Enter).is_empty());
    // An editor keeps Enter and Space as text; a sink takes every key as
    // data and is never "pressed".
    core.set_focus(Some(k.name));
    assert!(
        key(&mut core, EditKey::Enter).iter().all(|e| e
            .payload
            .get("kind")
            .and_then(Value::as_str)
            == Some("submit"))
    );
    core.set_focus(Some(k.pane));
    assert!(key(&mut core, EditKey::Enter).is_empty());
    assert!(core.handle_input(InputEvent::Text(" ".into())).is_empty());
    // Escape lets go of a control.
    core.set_focus(Some(k.go));
    key(&mut core, EditKey::Escape);
    assert_eq!(core.focus(), None);
}

#[test]
fn arrows_nudge_a_focused_slider_like_assistive_technology_does() {
    let mut core = Core::new();
    let k = frame(&mut core, false, false);
    core.set_focus(Some(k.vol));
    let up = key(&mut core, EditKey::Right);
    assert_eq!(payloads(&up), ["access:increment"]);
    assert_eq!(up[0].key, k.vol);
    assert_eq!(
        up[0].payload.get("tag").and_then(Value::as_str),
        Some("vol")
    );
    assert_eq!(payloads(&key(&mut core, EditKey::Up)), ["access:increment"]);
    assert_eq!(
        payloads(&key(&mut core, EditKey::Left)),
        ["access:decrement"]
    );
    assert_eq!(
        payloads(&key(&mut core, EditKey::Down)),
        ["access:decrement"]
    );
    // The same request from a reader yields the same event.
    let req = core.handle_input(InputEvent::Access(AccessRequest::new(
        k.vol,
        AccessAction::Increment,
    )));
    assert_eq!(payloads(&req), ["access:increment"]);
    // Arrows on a button are nothing.
    core.set_focus(Some(k.go));
    assert!(key(&mut core, EditKey::Right).is_empty());
}

#[test]
fn keyboard_focus_draws_a_ring_and_pointer_focus_does_not() {
    let mut core = Core::new();
    let k = frame(&mut core, false, false);
    assert_eq!(ring_count(&mut core), 0);
    tab(&mut core, false);
    assert_eq!(core.focus(), Some(k.go));
    frame(&mut core, false, false);
    assert_eq!(
        ring_count(&mut core),
        1,
        "Tab onto the button draws the ring"
    );
    let ring = core
        .output()
        .0
        .quads
        .iter()
        .find(|q| q.color.a == 0.0 && q.border_w == 2.0)
        .copied()
        .unwrap();
    // Two px outside the 100×20 button on row 2.
    assert_eq!(
        (ring.rect.x, ring.rect.y, ring.rect.w, ring.rect.h),
        (-2.0, 2.0 * H - 2.0, 104.0, H + 4.0)
    );

    // A click on the same button keeps focus there but hides the ring.
    click_at(&mut core, 50.0, 2.0 * H + 10.0);
    assert_eq!(core.focus(), Some(k.go));
    assert!(!core.focus_visible());
    frame(&mut core, false, false);
    assert_eq!(ring_count(&mut core), 0);

    // Editors and sinks draw no ring: the caret and the app show focus.
    core.set_focus(Some(k.name));
    tab(&mut core, false);
    assert_eq!(core.focus(), Some(k.pane));
    frame(&mut core, false, false);
    assert_eq!(ring_count(&mut core), 0);
    core.focus_next(false);
    assert_eq!(core.focus(), Some(k.name));
    frame(&mut core, false, false);
    assert_eq!(ring_count(&mut core), 0);
}

#[test]
fn focus_bg_swaps_the_background_instead_of_a_ring() {
    let mut core = Core::new();
    let k = frame(&mut core, false, false);
    let styled_quad = |core: &mut Core| {
        core.output()
            .0
            .quads
            .iter()
            .find(|q| q.rect.y == 12.0 * H && q.rect.h == H)
            .copied()
            .unwrap()
    };
    assert_eq!(styled_quad(&mut core).color, Color::BLACK);
    tab(&mut core, true);
    assert_eq!(core.focus(), Some(k.styled));
    frame(&mut core, false, false);
    assert_eq!(styled_quad(&mut core).color, FOCUS_BG);
    assert_eq!(ring_count(&mut core), 0, "a focus_bg node gets no ring");
    // Pointer focus is not keyboard-visible: plain bg again.
    click_at(&mut core, 50.0, 12.0 * H + 10.0);
    frame(&mut core, false, false);
    assert_eq!(styled_quad(&mut core).color, Color::BLACK);
}

#[test]
fn disabled_nodes_are_inert_and_say_so() {
    let mut core = Core::new();
    let k = frame(&mut core, false, false);
    // No click from the pointer, Enter, Space or a reader; no focus.
    assert!(click_at(&mut core, 50.0, 6.0 * H + 10.0).is_empty());
    assert_eq!(
        core.focus(),
        None,
        "a press on a disabled node takes no focus"
    );
    core.set_focus(Some(k.off));
    assert!(key(&mut core, EditKey::Enter).is_empty());
    assert!(core.handle_input(InputEvent::Text(" ".into())).is_empty());
    assert!(
        core.handle_input(InputEvent::Access(AccessRequest::new(
            k.off,
            AccessAction::Click
        )))
        .is_empty()
    );
    // Hover styling stays off; the hit region (for a tooltip) stays.
    core.handle_input(InputEvent::CursorMoved(Vec2::new(50.0, 6.0 * H + 10.0)));
    assert!(core.is_hovered(k.off));
    frame(&mut core, false, false);
    let off_quad = core
        .output()
        .0
        .quads
        .iter()
        .find(|q| q.rect.y == 6.0 * H && q.rect.h == H)
        .copied();
    assert!(
        off_quad.is_none(),
        "no hover_bg swap: the node stays unpainted"
    );
    // The access tree reports it, without click or focus actions.
    let tree = core.access_tree();
    let off = tree.get(k.off).unwrap();
    assert!(off.disabled);
    assert!(!off.supports(AccessAction::Click));
    assert!(!off.supports(AccessAction::Focus));
    let go = tree.get(k.go).unwrap();
    assert!(!go.disabled);
    assert!(go.supports(AccessAction::Click) && go.supports(AccessAction::Focus));
}

#[test]
fn assistive_technology_focuses_what_tab_would() {
    let mut core = Core::new();
    let k = frame(&mut core, false, false);
    // Every control advertises focus; structure does not.
    let tree = core.access_tree().clone();
    for key in [k.go, k.save, k.mute, k.vol, k.row, k.name, k.pane, k.styled] {
        let n = tree.get(key).unwrap();
        assert!(
            n.supports(AccessAction::Focus) && n.supports(AccessAction::Blur),
            "{:?} advertises focus",
            n.role
        );
        assert!(!n.focused);
    }
    assert_eq!(tree.focus, None);
    let close = tree.get(Key::ROOT.str("titlebar").str("close")).unwrap();
    assert!(
        !close.supports(AccessAction::Focus),
        "chrome is not a Tab stop"
    );

    core.handle_input(InputEvent::Access(AccessRequest::new(
        k.mute,
        AccessAction::Focus,
    )));
    assert_eq!(core.focus(), Some(k.mute));
    assert!(core.focus_visible(), "a reader's focus shows like Tab's");
    let tree = core.access_tree();
    assert_eq!(tree.focus, Some(k.mute));
    assert!(tree.get(k.mute).unwrap().focused);

    // Tab continues from there; the tree follows.
    tab(&mut core, false);
    assert_eq!(core.access_tree().focus, Some(k.vol));

    // Blur only when it is the focused node; a request on structure does
    // nothing.
    core.handle_input(InputEvent::Access(AccessRequest::new(
        k.go,
        AccessAction::Blur,
    )));
    assert_eq!(core.focus(), Some(k.vol));
    core.handle_input(InputEvent::Access(AccessRequest::new(
        k.hidden,
        AccessAction::Focus,
    )));
    assert_eq!(core.focus(), Some(k.vol));
    core.handle_input(InputEvent::Access(AccessRequest::new(
        k.vol,
        AccessAction::Blur,
    )));
    assert_eq!(core.focus(), None);
    assert_eq!(core.access_tree().focus, None);
}

#[test]
fn a_repeated_declaration_takes_focus_once() {
    let mut core = Core::new();
    let k = frame(&mut core, true, false);
    assert_eq!(core.focus(), Some(k.pane), "declared: focused");
    assert!(!core.focus_visible(), "programmatic focus shows no ring");
    // The app hands focus on; the next frame declares the sink again
    // and does not take it back.
    core.focus_next(true);
    assert_eq!(core.focus(), Some(k.styled));
    frame(&mut core, true, false);
    assert_eq!(core.focus(), Some(k.styled));
    // A frame that stops declaring, then one that declares again: an
    // edge, so the sink takes focus once more.
    frame(&mut core, false, false);
    assert_eq!(core.focus(), Some(k.styled));
    frame(&mut core, true, false);
    assert_eq!(core.focus(), Some(k.pane));
    // None blurs at once.
    core.set_key_focus(None);
    assert_eq!(core.focus(), None);
}

#[test]
fn autofocus_takes_the_keyboard_only_while_nothing_holds_it() {
    let mut core = Core::new();
    let k = frame(&mut core, false, true);
    assert_eq!(core.focus(), Some(k.name));
    assert_eq!(core.edit.focused(), Some(k.name));
    core.set_focus(Some(k.go));
    frame(&mut core, false, true);
    assert_eq!(core.focus(), Some(k.go), "a focused control keeps focus");
    assert_eq!(core.edit.focused(), None);
    core.set_focus(None);
    frame(&mut core, false, true);
    assert_eq!(core.focus(), Some(k.name), "free again: autofocus takes it");
}

#[test]
fn a_press_moves_focus_to_what_it_hits() {
    let mut core = Core::new();
    let k = frame(&mut core, false, false);
    assert_eq!(payloads(&click_at(&mut core, 50.0, 2.0 * H + 10.0)), ["go"]);
    assert_eq!(core.focus(), Some(k.go));
    // The focusable row takes focus without emitting anything.
    assert!(click_at(&mut core, 50.0, 9.0 * H + 10.0).is_empty());
    assert_eq!(core.focus(), Some(k.row));
    // A plain box drops it.
    click_at(&mut core, 50.0, 8.0 * H + 10.0);
    assert_eq!(core.focus(), None);
    // An editor: focus and caret together.
    click_at(&mut core, 50.0, 10.0 * H + 10.0);
    assert_eq!(core.focus(), Some(k.name));
    assert_eq!(core.edit.focused(), Some(k.name));
    // Tab from the editor goes on to the sink, and the editor lets go.
    tab(&mut core, false);
    assert_eq!(core.focus(), Some(k.pane));
    assert_eq!(core.edit.focused(), None);
}
