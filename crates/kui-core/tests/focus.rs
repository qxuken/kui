//! Keyboard focus as data (`docs/adr/0002-keyboard-focus-as-data.md`): one
//! focus for every node, Tab walking every control in tree order, Enter /
//! Space pressing the focused one, arrows nudging a slider, a ring the
//! core draws for keyboard focus (or the node's `focus_bg`), `disabled`
//! nodes inert and skipped, and assistive technology landing on the same
//! focus a Tab press moves — and, on top of it,
//! `docs/adr/0003-modal-surfaces.md`: a modal surface containing focus,
//! making everything outside it inert, and asking to be dismissed.

use kui_core::{
    AccessAction, AccessRequest, Align, Color, Core, EditKey, EditOptions, FloatConfig, InputEvent,
    Key, Mods, NodeSpec, Role, Size, Sizing, TextStyle, UiEvent, Value, Vec2, WindowButton,
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
    out.extend(core.handle_input(InputEvent::mouse_down(1)));
    out.extend(core.handle_input(InputEvent::mouse_up()));
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

/// `Ui::focus_next` / `focus_prev` defer to `finish`, because a view is
/// still declaring the tree the Tab ring is made of. So a view that has
/// never rendered can still step onto a control it is declaring — where
/// `Core::focus_next`, which walks the last finished tree, has nothing.
#[test]
fn a_view_steps_focus_onto_the_frame_it_is_declaring() {
    let build = |core: &mut Core, step: Option<bool>| {
        let mut ui = core.frame(Size::new(200.0, 200.0), 1.0);
        match step {
            Some(true) => ui.focus_next(),
            Some(false) => ui.focus_prev(),
            None => {}
        }
        for name in ["a", "b", "c"] {
            ui.with_keyed(
                name,
                NodeSpec::row()
                    .focusable()
                    .width(Sizing::Fixed(100.0))
                    .height(Sizing::Fixed(H)),
                |_| {},
            );
        }
        ui.finish();
    };
    let root = Key::ROOT;
    let (a, c) = (root.str("a"), root.str("c"));

    // The very first frame: nothing has been laid out, so an immediate
    // step has no ring at all and the deferred one still lands.
    let mut immediate = Core::new();
    {
        let mut ui = immediate.frame(Size::new(200.0, 200.0), 1.0);
        ui.core().focus_next(true);
        for name in ["a", "b", "c"] {
            ui.with_keyed(
                name,
                NodeSpec::row()
                    .focusable()
                    .width(Sizing::Fixed(100.0))
                    .height(Sizing::Fixed(H)),
                |_| {},
            );
        }
        ui.finish();
    }
    assert_eq!(immediate.focus(), None, "no ring to walk mid-build");

    let mut core = Core::new();
    build(&mut core, Some(true));
    assert_eq!(core.focus(), Some(a), "the deferred step found the ring");

    // And it beats a `set_focus` made in the same frame — the step is the
    // later word, applied once the frame it belongs to is whole.
    {
        let mut ui = core.frame(Size::new(200.0, 200.0), 1.0);
        ui.focus(a);
        ui.focus_prev();
        for name in ["a", "b", "c"] {
            ui.with_keyed(
                name,
                NodeSpec::row()
                    .focusable()
                    .width(Sizing::Fixed(100.0))
                    .height(Sizing::Fixed(H)),
                |_| {},
            );
        }
        ui.finish();
    }
    assert_eq!(core.focus(), Some(c), "stepped back from a, wrapping");

    // Only the last request of a frame is kept, and it is not held for a
    // later frame once applied.
    build(&mut core, None);
    assert_eq!(core.focus(), Some(c), "no step, no move");
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

// -- Modal surfaces (docs/adr/0003-modal-surfaces.md) -----------------------

/// A window with chrome, a scrolling list of buttons behind, an editor,
/// and — when `dialog` says so — a floated modal holding a button and an
/// editor of its own. The viewport is 200x300; the background sits at the
/// top left, the modal at the bottom right.
struct Modal {
    open: Key,
    behind: Key,
    list: Key,
    note: Key,
    dialog: Key,
    ok: Key,
    reason: Key,
}

fn modal_frame(core: &mut Core, dialog: bool) -> Modal {
    let mut ui = core.frame(Size::new(200.0, 300.0), 1.0);
    ui.window_title("Modal");
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
    let open = ui.with_keyed(
        "open",
        row().on_click(Value::str("open")).label("Open"),
        |_| {},
    );
    // A scroll container two rows tall holding four: it overflows, so it
    // has a scrollbar and somewhere for the wheel to go.
    let list = ui.with_keyed(
        "list",
        NodeSpec::column()
            .width(Sizing::Fixed(100.0))
            .height(Sizing::Fixed(2.0 * H))
            .scroll_y(),
        |ui| {
            for n in 0..4 {
                ui.with_keyed(
                    ["a", "b", "c", "d"][n],
                    row().on_click(Value::str("row")).label("Row"),
                    |_| {},
                );
            }
        },
    );
    let behind = Key::ROOT.str("list").str("a");
    let note = ui.text_edit("note", "hi", &EditOptions::default(), row().label("Note"));
    let mut ok = Key::ROOT;
    let mut reason = Key::ROOT;
    let mut dlg = Key::ROOT;
    if dialog {
        dlg = ui.with_keyed(
            "dialog",
            NodeSpec::column()
                .width(Sizing::Fixed(100.0))
                .height(Sizing::Fixed(3.0 * H))
                .float(
                    FloatConfig::viewport()
                        .at(Align::End, Align::End)
                        .self_at(Align::End, Align::End),
                )
                .modal(Value::str("dlg"))
                .label("Settings"),
            |ui| {
                ok = ui.with_keyed("ok", row().on_click(Value::str("ok")).label("OK"), |_| {});
                reason = ui.text_edit("reason", "", &EditOptions::default(), row().label("Reason"));
            },
        );
    }
    ui.finish();
    Modal {
        open,
        behind,
        list,
        note,
        dialog: dlg,
        ok,
        reason,
    }
}

/// The centre of the modal's OK button (bottom right, the modal's first
/// row), a point on the modal's own background under its two rows, and the
/// centre of the button behind it (top left).
const IN_MODAL: (f32, f32) = (150.0, 300.0 - 3.0 * H + H / 2.0);
const MODAL_BG: (f32, f32) = (150.0, 300.0 - H / 2.0);
const BEHIND: (f32, f32) = (50.0, H + H / 2.0);

#[test]
fn everything_outside_the_modal_is_inert() {
    let mut core = Core::new();
    let m = modal_frame(&mut core, true);
    assert_eq!(core.modal(), Some(m.dialog));

    // A press on the button behind emits its click payload no longer —
    // only the modal's dismiss — and does not move focus.
    let out = click_at(&mut core, BEHIND.0, BEHIND.1);
    assert_eq!(payloads(&out), ["dismiss:"], "no `open`, one dismiss");
    assert_eq!(
        out[0].payload.get("reason").and_then(Value::as_str),
        Some("outside")
    );
    assert_eq!(out[0].key, m.dialog);
    assert_eq!(core.focus(), Some(m.ok), "focus stays in the modal");

    // Hover, Enter / Space and a reader's click all resolve against the
    // same hit list, so all three are inert too.
    core.handle_input(InputEvent::CursorMoved(Vec2::new(BEHIND.0, BEHIND.1)));
    assert!(!core.is_hovered(m.open));
    core.set_focus(Some(m.open));
    assert!(key(&mut core, EditKey::Enter).is_empty());
    assert!(core.handle_input(InputEvent::Text(" ".into())).is_empty());
    assert!(
        core.handle_input(InputEvent::Access(AccessRequest::new(
            m.behind,
            AccessAction::Click
        )))
        .is_empty()
    );

    // The wheel over the list behind does nothing; the modal's own
    // controls are live.
    let before = core.scroll.offset(m.list);
    core.handle_input(InputEvent::Scroll(Vec2::new(0.0, -30.0)));
    assert_eq!(core.scroll.offset(m.list), before, "the list stays put");
    assert_eq!(
        payloads(&click_at(&mut core, IN_MODAL.0, IN_MODAL.1)),
        ["ok"]
    );
    assert_eq!(core.focus(), Some(m.ok));

    // Without the modal, every one of those works again.
    let m = modal_frame(&mut core, false);
    assert_eq!(core.modal(), None);
    assert_eq!(payloads(&click_at(&mut core, BEHIND.0, BEHIND.1)), ["open"]);
    assert_eq!(core.focus(), Some(m.open));
    core.handle_input(InputEvent::CursorMoved(Vec2::new(50.0, 3.0 * H)));
    core.handle_input(InputEvent::Scroll(Vec2::new(0.0, -30.0)));
    assert!(core.scroll.offset(m.list).y > 0.0);
}

#[test]
fn window_chrome_stays_live_behind_a_modal() {
    let mut core = Core::new();
    let m = modal_frame(&mut core, true);
    // The close button is chrome: a press on it issues its window command
    // rather than a dismiss.
    let out = click_at(&mut core, H / 2.0, H / 2.0);
    assert!(payloads(&out).is_empty(), "chrome emits no app event");
    assert_eq!(core.take_window_commands().len(), 1, "the window closes");
    assert_eq!(core.focus(), Some(m.ok), "and focus stays in the modal");
}

#[test]
fn escape_asks_the_modal_to_go_away_and_does_nothing_else() {
    let mut core = Core::new();
    let m = modal_frame(&mut core, true);

    let out = key(&mut core, EditKey::Escape);
    assert_eq!(payloads(&out), ["dismiss:"]);
    assert_eq!(out[0].key, m.dialog);
    assert_eq!(
        out[0].payload.get("reason").and_then(Value::as_str),
        Some("escape")
    );
    assert_eq!(
        out[0].payload.get("tag").and_then(Value::as_str),
        Some("dlg"),
        "the modal's tag rides along"
    );
    assert_eq!(core.focus(), Some(m.ok), "Escape does not let go");

    // Not even from inside an editor in the dialog: the modal owns it.
    core.set_focus(Some(m.reason));
    let out = key(&mut core, EditKey::Escape);
    assert_eq!(payloads(&out), ["dismiss:"]);
    assert_eq!(core.focus(), Some(m.reason));
    assert_eq!(core.edit.focused(), Some(m.reason));

    // The core closed nothing — the app decides. Once it stops declaring
    // the dialog, Escape means what ADR 0002 gave it again.
    let m = modal_frame(&mut core, false);
    core.set_focus(Some(m.note));
    assert!(payloads(&key(&mut core, EditKey::Escape)).is_empty());
    assert_eq!(core.focus(), None);
}

#[test]
fn a_press_on_the_modal_itself_is_not_outside_it() {
    let mut core = Core::new();
    let m = modal_frame(&mut core, true);
    // The strip under the modal's two rows is its own background: a press
    // there is inside, so nothing is dismissed and nothing is emitted.
    let out = click_at(&mut core, MODAL_BG.0, MODAL_BG.1);
    assert!(payloads(&out).is_empty(), "{:?}", payloads(&out));
    assert_eq!(core.modal(), Some(m.dialog));
}

#[test]
fn a_modal_without_a_tag_still_dismisses() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(200.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let menu = ui.with_keyed(
        "menu",
        NodeSpec::column()
            .width(Sizing::Fixed(100.0))
            .height(Sizing::Fixed(H))
            .float(FloatConfig::viewport())
            .modal(Value::Null)
            .label("Menu"),
        |_| {},
    );
    ui.finish();
    let out = key(&mut core, EditKey::Escape);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].key, menu);
    assert_eq!(
        out[0].payload.get("kind").and_then(Value::as_str),
        Some("dismiss")
    );
    assert_eq!(out[0].payload.get("tag"), None, "a null tag carries none");
}

#[test]
fn the_access_tree_reports_the_modal_and_derives_a_dialog() {
    let mut core = Core::new();
    let m = modal_frame(&mut core, true);
    let tree = core.access_tree().clone();
    let dlg = tree.get(m.dialog).unwrap();
    assert_eq!(dlg.role, Role::Dialog, "a modal box is a dialog");
    assert_eq!(dlg.name.as_deref(), Some("Settings"));
    assert!(dlg.modal, "aria-modal");
    assert_eq!(tree.focus, Some(m.ok), "the reader's cursor is inside it");
    // The background is still in the tree — the platform acts on the
    // modal flag — and nothing else claims to be modal.
    assert!(tree.get(m.open).is_some());
    assert_eq!(tree.nodes.iter().filter(|n| n.modal).count(), 1);
    let hash = tree.hash;

    // Without the modal, the flag is gone and the tree says so.
    modal_frame(&mut core, false);
    let tree = core.access_tree();
    assert!(!tree.nodes.iter().any(|n| n.modal));
    assert_ne!(tree.hash, hash);
}

/// A dialog is named by its `label` alone (it is not one of ARIA's
/// name-from-content roles), so an unlabelled one is announced as an
/// unnamed dialog — the `control-without-name` defect, on the node that
/// just took focus.
#[test]
fn a_modal_without_a_name_warns() {
    let dialog = |label: Option<&'static str>| {
        let mut core = Core::new();
        let mut ui = core.frame(Size::new(200.0, 300.0), 1.0);
        ui.configure_root(NodeSpec::column().fill());
        let mut spec = NodeSpec::column()
            .width(Sizing::Fixed(100.0))
            .height(Sizing::Fixed(2.0 * H))
            .float(FloatConfig::viewport())
            .modal(Value::str("dlg"));
        if let Some(label) = label {
            spec = spec.label(label);
        }
        let key = ui.with_keyed("dialog", spec, |ui| {
            // Text inside is not a name for a dialog, so it does not
            // silence the warning the way it would for a button.
            ui.text("Delete everything?", TextStyle::default());
        });
        ui.finish();
        (core.take_warnings(), key)
    };

    let (ws, key) = dialog(None);
    let named: Vec<_> = ws
        .iter()
        .filter(|w| w.code == kui_core::diag::MODAL_WITHOUT_NAME)
        .collect();
    assert_eq!(named.len(), 1);
    assert_eq!(named[0].key, key);
    assert!(named[0].message.contains("`label`"), "{}", named[0].message);

    let (ws, _) = dialog(Some("Confirm"));
    assert!(
        !ws.iter()
            .any(|w| w.code == kui_core::diag::MODAL_WITHOUT_NAME),
        "a labelled dialog says nothing: {ws:?}"
    );
}

#[test]
fn a_modal_that_is_not_floated_warns() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(200.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let sheet = ui.with_keyed(
        "sheet",
        NodeSpec::column()
            .width(Sizing::Fixed(100.0))
            .height(Sizing::Fixed(H))
            .modal(Value::Null)
            .label("Sheet"),
        |_| {},
    );
    ui.with_keyed("after", NodeSpec::row().height(Sizing::Fixed(H)), |_| {});
    ui.finish();
    let ws = core.take_warnings();
    let modal: Vec<_> = ws
        .iter()
        .filter(|w| w.code == kui_core::diag::MODAL_BEHIND_CONTENT)
        .collect();
    assert_eq!(modal.len(), 1);
    assert_eq!(modal[0].key, sheet);

    // Floated, it draws over everything and says nothing.
    let mut core = Core::new();
    modal_frame(&mut core, true);
    assert!(
        !core
            .take_warnings()
            .iter()
            .any(|w| w.code == kui_core::diag::MODAL_BEHIND_CONTENT)
    );
}

#[test]
fn a_scrollbar_behind_a_modal_is_drawn_but_not_grabbable() {
    let mut core = Core::new();
    let m = modal_frame(&mut core, true);
    // The list (x 0..100, y 2H..4H) overflows, so it has a vertical bar at
    // its right edge. A press low on the track would jump the offset there.
    let track = (99.0, 4.0 * H - 4.0);
    let out = click_at(&mut core, track.0, track.1);
    assert_eq!(
        core.scroll.offset(m.list).y,
        0.0,
        "the track press did nothing"
    );
    assert_eq!(
        payloads(&out),
        ["dismiss:"],
        "an inert bar is not a hit region: the press is outside the modal"
    );
    assert_eq!(core.focus(), Some(m.ok));

    // Without the modal the same press jumps the list.
    let m = modal_frame(&mut core, false);
    let out = click_at(&mut core, track.0, track.1);
    assert!(
        payloads(&out).is_empty(),
        "a scrollbar press is not a click"
    );
    assert!(
        core.scroll.offset(m.list).y > 0.0,
        "the track press scrolled"
    );
}

#[test]
fn a_float_declared_before_a_modal_paints_over_it_too() {
    // Floats paint after every in-flow node, so a float declared before
    // an in-flow modal still draws on top of it.
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(200.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.with_keyed(
        "hud",
        NodeSpec::row()
            .width(Sizing::Fixed(50.0))
            .height(Sizing::Fixed(H))
            .float(FloatConfig::viewport()),
        |_| {},
    );
    let sheet = ui.with_keyed(
        "sheet",
        NodeSpec::column()
            .width(Sizing::Fixed(100.0))
            .height(Sizing::Fixed(H))
            .modal(Value::Null)
            .label("Sheet"),
        |_| {},
    );
    ui.finish();
    let ws = core.take_warnings();
    let modal: Vec<_> = ws
        .iter()
        .filter(|w| w.code == kui_core::diag::MODAL_BEHIND_CONTENT)
        .collect();
    assert_eq!(modal.len(), 1);
    assert_eq!(modal[0].key, sheet);

    // A float inside the modal is the modal's own (a dropdown, a tooltip):
    // nothing paints over the modal, so nothing is said.
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(200.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.with_keyed(
        "sheet",
        NodeSpec::column()
            .width(Sizing::Fixed(100.0))
            .height(Sizing::Fixed(H))
            .modal(Value::Null)
            .label("Sheet"),
        |ui| {
            ui.with_keyed(
                "menu",
                NodeSpec::row()
                    .width(Sizing::Fixed(50.0))
                    .height(Sizing::Fixed(H))
                    .float(FloatConfig::viewport()),
                |_| {},
            );
        },
    );
    ui.finish();
    assert!(
        !core
            .take_warnings()
            .iter()
            .any(|w| w.code == kui_core::diag::MODAL_BEHIND_CONTENT)
    );
}

/// A draggable handle and a button at the top left, and — when `dialog`
/// says so — a floated modal at the bottom right.
fn drag_frame(core: &mut Core, dialog: bool) -> (Key, Key) {
    let mut ui = core.frame(Size::new(200.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let row = || {
        NodeSpec::row()
            .width(Sizing::Fixed(100.0))
            .height(Sizing::Fixed(H))
    };
    let handle = ui.with_keyed("handle", row().on_drag(Value::str("split")), |_| {});
    let button = ui.with_keyed("button", row().on_click(Value::str("go")), |_| {});
    if dialog {
        ui.with_keyed(
            "dialog",
            NodeSpec::column()
                .width(Sizing::Fixed(100.0))
                .height(Sizing::Fixed(H))
                .float(
                    FloatConfig::viewport()
                        .at(Align::End, Align::End)
                        .self_at(Align::End, Align::End),
                )
                .modal(Value::Null)
                .label("Wait"),
            |ui| {
                ui.with_keyed("ok", row().on_click(Value::str("ok")), |_| {});
            },
        );
    }
    ui.finish();
    (handle, button)
}

fn drag_phases(evs: &[UiEvent]) -> Vec<String> {
    evs.iter()
        .filter(|e| e.payload.get("kind").and_then(Value::as_str) == Some("drag"))
        .map(|e| {
            e.payload
                .get("phase")
                .and_then(Value::as_str)
                .unwrap_or("?")
                .to_string()
        })
        .collect()
}

#[test]
fn a_drag_in_flight_when_a_modal_appears_still_ends() {
    // Pointer capture outlives the frame that raised the modal: the app
    // was told the drag started, so it is told the drag ended, and gets
    // the moves in between. Only a new press is inert.
    let mut core = Core::new();
    drag_frame(&mut core, false);
    let mut all = Vec::new();
    all.extend(core.handle_input(InputEvent::CursorMoved(Vec2::new(50.0, H / 2.0))));
    all.extend(core.handle_input(InputEvent::mouse_down(1)));
    all.extend(core.handle_input(InputEvent::CursorMoved(Vec2::new(60.0, H / 2.0))));
    assert_eq!(drag_phases(&all), ["start", "move"]);

    drag_frame(&mut core, true);
    let mut after = Vec::new();
    after.extend(core.handle_input(InputEvent::CursorMoved(Vec2::new(70.0, H / 2.0))));
    after.extend(core.handle_input(InputEvent::mouse_up()));
    assert_eq!(
        drag_phases(&after),
        ["move", "end"],
        "the capture runs to release"
    );
    assert!(
        !after.iter().any(|e| e.payload.as_str().is_some()),
        "and a release after a drag is not a click"
    );

    // The next press on the handle finds no region: no drag starts, the
    // modal is asked to go away instead.
    let out = click_at(&mut core, 50.0, H / 2.0);
    assert!(drag_phases(&out).is_empty());
    assert_eq!(payloads(&out), ["dismiss:"]);
}

#[test]
fn a_press_behind_released_after_a_modal_appears_is_not_a_click() {
    let mut core = Core::new();
    let (_, button) = drag_frame(&mut core, false);
    core.handle_input(InputEvent::CursorMoved(Vec2::new(50.0, H + H / 2.0)));
    core.handle_input(InputEvent::mouse_down(1));
    assert!(core.is_pressed(button));

    // The modal appears between press and release: the button behind is
    // no longer under the pointer as far as the hit list knows.
    drag_frame(&mut core, true);
    assert!(!core.is_hovered(button));
    let out = core.handle_input(InputEvent::mouse_up());
    assert!(payloads(&out).is_empty(), "{:?}", payloads(&out));
}

// -- The way out of a modal (backlog F4) -------------------------------------

/// The mind map's rename editor: a list of nodes, and — when `editing`
/// says so — a floated modal holding the editor. `focus_on` is the frame's
/// `keyFocus` declaration, the only way a data view has of saying where
/// the keyboard belongs.
fn rename_frame(
    core: &mut Core,
    nodes: &[&str],
    editing: bool,
    focus_on: Option<&str>,
) -> Vec<Key> {
    let mut ui = core.frame(Size::new(200.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let row = || {
        NodeSpec::row()
            .width(Sizing::Fixed(100.0))
            .height(Sizing::Fixed(H))
    };
    let keys: Vec<Key> = nodes
        .iter()
        .map(|n| ui.with_keyed(n, row().focusable().label(*n), |_| {}))
        .collect();
    if let Some(name) = focus_on {
        let i = nodes
            .iter()
            .position(|n| n == &name)
            .expect("a declared node");
        ui.take_key_focus(keys[i]);
    }
    if editing {
        ui.with_keyed(
            "rename",
            NodeSpec::column()
                .width(Sizing::Fixed(100.0))
                .height(Sizing::Fixed(2.0 * H))
                .float(
                    FloatConfig::viewport()
                        .at(Align::End, Align::End)
                        .self_at(Align::End, Align::End),
                )
                .modal(Value::str("rename"))
                .label("Rename"),
            |ui| {
                ui.text_edit("field", "", &EditOptions::default(), row().label("Name"));
            },
        );
    }
    ui.finish();
    keys
}

#[test]
fn a_key_focus_edge_beats_the_focus_the_modal_gives_back() {
    // The report's case: a node is created and its rename editor opened in
    // one frame, so what the modal displaced is the node the user was on
    // *before* — and handing that back on the way out puts the next Enter
    // in the wrong place. The app says where focus lands by declaring it,
    // and the declaration wins (ADR 0003, decision 4).
    let mut core = Core::new();
    let k = rename_frame(&mut core, &["a"], false, Some("a"));
    let a = k[0];
    assert_eq!(core.focus(), Some(a));

    // `b` is created and the editor opened together: the modal remembers
    // `a`, and containment puts focus in the editor.
    let k = rename_frame(&mut core, &["a", "b"], true, None);
    let b = k[1];
    assert_eq!(core.focus(), Some(Key::ROOT.str("rename").str("field")));

    // The editor goes away and the app declares the new node focused.
    rename_frame(&mut core, &["a", "b"], false, Some("b"));
    assert_eq!(
        core.focus(),
        Some(b),
        "the edge stands, the restore is dropped"
    );
    assert_ne!(core.focus(), Some(a));
}

#[test]
fn a_modal_closing_over_no_edge_still_gives_the_focus_back() {
    // The other half, and the reason the rule is an *edge*: a view that
    // repeats `keyFocus` every frame — an app that owns its keyboard —
    // declares nothing new on the frame the modal drops, so decision 4 is
    // untouched and the focus the dialog displaced comes back.
    let mut core = Core::new();
    let k = rename_frame(&mut core, &["a", "b"], false, Some("a"));
    let a = k[0];
    core.set_focus(Some(k[1]));

    rename_frame(&mut core, &["a", "b"], true, Some("a"));
    assert_eq!(core.focus(), Some(Key::ROOT.str("rename").str("field")));
    rename_frame(&mut core, &["a", "b"], false, Some("a"));
    assert_eq!(core.focus(), Some(k[1]), "back where the editor found it");

    // And with no declaration at all, which is where it always landed.
    core.set_focus(Some(a));
    rename_frame(&mut core, &["a", "b"], true, None);
    rename_frame(&mut core, &["a", "b"], false, None);
    assert_eq!(core.focus(), Some(a));
}

/// A click focuses without showing (pointer focus, ADR 0002 decision 4) —
/// and then the keyboard acts on that focus. Space presses the button the
/// pointer left focused, and a press whose ring never appears is a press
/// the user cannot attribute to a control: the frame answered, but nothing
/// on screen says which node answered. So the key that acts shows the
/// focus, exactly as the Tab that could have put it there would have.
#[test]
fn a_key_acting_on_pointer_focus_shows_it() {
    let mut core = Core::new();
    let k = frame(&mut core, false, false);

    // Space, after a click on the same button.
    click_at(&mut core, 50.0, 2.0 * H + 10.0);
    assert_eq!(core.focus(), Some(k.go));
    assert!(!core.focus_visible(), "the click alone shows nothing");
    assert_eq!(
        payloads(&core.handle_input(InputEvent::Text(" ".into()))),
        ["go"]
    );
    assert!(core.focus_visible(), "Space pressed it: say which one");
    frame(&mut core, false, false);
    assert_eq!(ring_count(&mut core), 1);

    // Enter, on a switch the pointer focused.
    click_at(&mut core, 50.0, 4.0 * H + 10.0);
    assert_eq!(core.focus(), Some(k.mute));
    assert!(!core.focus_visible());
    assert_eq!(payloads(&key(&mut core, EditKey::Enter)), ["mute"]);
    assert!(core.focus_visible(), "Enter pressed it");

    // An arrow nudging a slider is the keyboard acting too.
    click_at(&mut core, 50.0, 5.0 * H + 10.0);
    assert_eq!(core.focus(), Some(k.vol));
    assert!(!core.focus_visible());
    assert_eq!(
        payloads(&key(&mut core, EditKey::Right)),
        ["access:increment"]
    );
    assert!(core.focus_visible(), "the slider moved: say which one");

    // A key the core does nothing with shows nothing: an arrow on a
    // button is not the keyboard using the focus, it is a key going
    // nowhere.
    click_at(&mut core, 50.0, 2.0 * H + 10.0);
    assert_eq!(core.focus(), Some(k.go));
    assert!(key(&mut core, EditKey::Right).is_empty());
    assert!(!core.focus_visible(), "nothing acted: nothing to show");

    // Escape is the other exception: it acts by letting go, and a ring
    // around nothing is not a ring.
    key(&mut core, EditKey::Escape);
    assert_eq!(core.focus(), None);
    frame(&mut core, false, false);
    assert_eq!(ring_count(&mut core), 0);
}
