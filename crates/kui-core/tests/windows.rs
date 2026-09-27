//! The declared window set across cores (`docs/adr/0004-multi-window.md`,
//! decisions 4-6): what the conformance corpus cannot pin, because a scene
//! runs in a session of one core. Two cores in one session are what the
//! union, the lowest-id rule and the cascade are *for*.

use kui_core::{
    Core, DismissReason, Rect, Session, Size, UiEvent, Value, WindowCommand, WindowConfig, WindowId,
};

fn frame(core: &mut Core, declare: &[(&str, WindowConfig)]) -> (Vec<WindowCommand>, Vec<UiEvent>) {
    let mut ui = core.frame(Size::new(320.0, 240.0), 1.0);
    for (name, cfg) in declare {
        ui.window(name, *cfg);
    }
    ui.finish();
    (core.take_window_commands(), core.take_pending_events())
}

fn opened(cmds: &[WindowCommand]) -> Vec<(u32, WindowConfig)> {
    cmds.iter()
        .filter_map(|c| match *c {
            WindowCommand::Open { id, config, .. } => Some((id.0, config)),
            _ => None,
        })
        .collect()
}

/// `(id, owner)` for every `Open`: who the window is and whose frame
/// declared it, which is what a popup is anchored against and owned by.
fn owners(cmds: &[WindowCommand]) -> Vec<(u32, u32)> {
    cmds.iter()
        .filter_map(|c| match *c {
            WindowCommand::Open { id, owner, .. } => Some((id.0, owner.0)),
            _ => None,
        })
        .collect()
}

fn row(ev: &UiEvent) -> (Option<String>, Option<String>) {
    let get = |k: &str| {
        ev.payload
            .get(k)
            .and_then(Value::as_str)
            .map(str::to_string)
    };
    (get("kind"), get("reason").or_else(|| get("phase")))
}

fn closed(cmds: &[WindowCommand]) -> Vec<u32> {
    cmds.iter()
        .filter_map(|c| match *c {
            WindowCommand::Close(id) => Some(id.0),
            _ => None,
        })
        .collect()
}

/// A second core, standing in for the window `id` an `Open` asked for: the
/// driver constructs it against the same session and hands it the id.
fn pane(session: &Session, id: u32) -> Core {
    let mut core = Core::new_in(session);
    core.env.window.id = WindowId(id);
    core
}

#[test]
fn the_set_is_the_union_of_every_window_and_a_window_takes_its_children_with_it() {
    let session = Session::new();
    let mut main = pane(&session, 0);
    let cfg = WindowConfig::sized(400.0, 300.0);

    // Main declares a palette; it opens as window 1.
    let (cmds, events) = frame(&mut main, &[("palette", cfg)]);
    assert_eq!(opened(&cmds), vec![(1, cfg)]);
    assert_eq!(
        events[0]
            .payload
            .get("phase")
            .and_then(kui_core::Value::as_str),
        Some("opened")
    );
    assert_eq!(main.windows().len(), 2);

    // The palette's own frame declares a submenu: the union has it, so it
    // opens too — a popup declaring its own submenu, in ADR 0004's words.
    let mut palette = pane(&session, 1);
    assert_eq!(&*palette.window_name(), "palette");
    let (cmds, _) = frame(&mut palette, &[("submenu", WindowConfig::default())]);
    assert_eq!(opened(&cmds), vec![(2, WindowConfig::default())]);
    let names: Vec<String> = main.windows().iter().map(|(_, n)| n.to_string()).collect();
    assert_eq!(names, ["main", "palette", "submenu"]);

    // Main keeps declaring the palette: nothing happens, in either core.
    assert!(frame(&mut main, &[("palette", cfg)]).0.is_empty());
    assert!(
        frame(&mut palette, &[("submenu", WindowConfig::default())])
            .0
            .is_empty()
    );

    // Main stops: the palette closes, and so does the submenu only the
    // palette declared — in the same diff, from the same core, parent
    // first (its slot leaves the union, and the next pass finds the child).
    let (cmds, events) = frame(&mut main, &[]);
    assert_eq!(closed(&cmds), vec![1, 2]);
    assert_eq!(events.len(), 2, "one `closed` event each: {events:?}");
    assert_eq!(main.windows().len(), 1);
}

#[test]
fn a_re_declared_live_window_keeps_its_config_and_lowest_window_wins_on_the_edge() {
    let session = Session::new();
    let mut main = pane(&session, 0);
    main.set_diagnostics(true);
    let small = WindowConfig::sized(100.0, 100.0);
    let big = WindowConfig::sized(800.0, 600.0);

    // Two windows share the session; the second one exists first.
    let (cmds, _) = frame(&mut main, &[("tools", WindowConfig::default())]);
    assert_eq!(opened(&cmds).len(), 1);
    let mut tools = pane(&session, 1);

    // The tools window declares the palette; that is the opening edge, and
    // its config is what the `Open` carries.
    let (cmds, _) = frame(&mut tools, &[("palette", small)]);
    assert_eq!(opened(&cmds), vec![(2, small)]);
    // Main re-declares it at another size: a live window's config is never
    // re-read, so nothing is resized, and nothing warns — this is not a
    // conflict on an opening edge.
    let (cmds, _) = frame(
        &mut main,
        &[("tools", WindowConfig::default()), ("palette", big)],
    );
    assert!(cmds.is_empty());
    assert!(main.take_warnings().is_empty());

    // The user closes the palette. Both still declare it, so it stays
    // closed, and the core whose frame declares it says so — once.
    tools.window_closed(WindowId(2));
    frame(
        &mut main,
        &[("tools", WindowConfig::default()), ("palette", big)],
    );
    frame(
        &mut main,
        &[("tools", WindowConfig::default()), ("palette", big)],
    );
    let codes: Vec<&str> = main.take_warnings().iter().map(|w| w.code).collect();
    assert_eq!(codes, ["window-declared-while-closed"]);

    // Both lapse; then both declare again in one round of frames — main
    // first this time. The edge opens it with main's config, because main
    // is the lowest declaring window, and the disagreement with the tools
    // window's slot is reported on that edge.
    frame(&mut tools, &[]);
    frame(&mut main, &[("tools", WindowConfig::default())]);
    assert_eq!(main.windows().len(), 2, "the palette lapsed");
    let (cmds, _) = frame(&mut tools, &[("palette", small)]);
    assert_eq!(
        opened(&cmds),
        vec![(3, small)],
        "tools' frame ran first this time"
    );
    // Close it again and let it lapse, then have main's frame run first:
    // the edge is main's, and its config wins.
    tools.window_closed(WindowId(3));
    frame(&mut tools, &[]);
    frame(&mut main, &[("tools", WindowConfig::default())]);
    let (cmds, _) = frame(
        &mut main,
        &[("tools", WindowConfig::default()), ("palette", big)],
    );
    assert_eq!(opened(&cmds), vec![(4, big)]);
    // And the tools window declaring it differently afterwards changes
    // nothing, again.
    let (cmds, _) = frame(&mut tools, &[("palette", small)]);
    assert!(cmds.is_empty());
    assert!(main.take_warnings().is_empty());
    assert!(tools.take_warnings().is_empty());
}

#[test]
fn a_core_named_for_a_window_the_session_never_opened_still_has_a_name() {
    let session = Session::new();
    let core = pane(&session, 9);
    assert_eq!(&*core.window_name(), "window-9");
    assert_eq!(&*Core::new().window_name(), "main");
}

/// ADR 0004 decision 9's popup, in the parts a core decides: the kind and
/// the anchor ride the declaration through to the `Open` untouched, the
/// owner is the window whose frame declared it, and closing that window
/// takes the popup with it without the driver saying anything.
#[test]
fn a_popup_is_owned_by_the_window_that_declared_it_and_closes_with_it() {
    let session = Session::new();
    let mut main = pane(&session, 0);
    let cfg = WindowConfig::sized(400.0, 300.0);
    let menu = WindowConfig::popup(Rect::new(12.0, 40.0, 160.0, 24.0), 160.0, 320.0);

    // Non-activating by construction: the field that opens a dropdown
    // keeps the ring, which is the reason the constructor exists.
    assert!(!menu.activates);
    assert_eq!(menu.kind, kui_core::WindowKind::Popup);

    let (cmds, _) = frame(&mut main, &[("palette", cfg)]);
    assert_eq!(owners(&cmds), vec![(1, 0)]);

    // The palette declares the popup, so the palette owns it — not main,
    // whose frame never named it.
    let mut palette = pane(&session, 1);
    let (cmds, _) = frame(&mut palette, &[("menu", menu)]);
    assert_eq!(opened(&cmds), vec![(2, menu)]);
    assert_eq!(owners(&cmds), vec![(2, 1)]);

    // The user closes the owner: its declarations leave the union with it,
    // so the popup closes in the same diff and nothing had to know that a
    // popup is a child.
    main.window_closed(WindowId(1));
    assert_eq!(closed(&main.take_window_commands()), vec![2]);
    assert_eq!(main.windows().len(), 1);
}

/// A dismissal is an event and nothing else (ADR 0003 decision 6, one
/// level up): the window stays open, still declared, until the app says
/// otherwise — and a dismissal naming a window the session never opened,
/// or one it has already closed, says nothing at all.
#[test]
fn a_dismissed_popup_closes_nothing_until_the_app_stops_declaring_it() {
    let session = Session::new();
    let mut main = pane(&session, 0);
    let menu = WindowConfig::popup(Rect::new(0.0, 0.0, 80.0, 20.0), 120.0, 400.0);
    frame(&mut main, &[("menu", menu)]);

    main.dismiss_window(WindowId(1), DismissReason::Outside);
    main.dismiss_window(WindowId(1), DismissReason::Escape);
    let events = main.take_pending_events();
    assert_eq!(
        events.iter().map(row).collect::<Vec<_>>(),
        vec![
            (Some("dismiss".into()), Some("outside".into())),
            (Some("dismiss".into()), Some("escape".into())),
        ]
    );
    assert_eq!(
        events[0].payload.get_str("name"),
        Some("menu"),
        "the event says which window, since it is on the root of whichever core noticed"
    );
    assert!(
        main.take_window_commands().is_empty(),
        "the core closes nothing in answer to a dismissal"
    );

    // Still declared, still open, and a frame that says so again is the
    // no-op it is for any live window.
    assert!(frame(&mut main, &[("menu", menu)]).0.is_empty());
    assert_eq!(main.windows().len(), 2);

    // The app answers on the frame it chooses.
    assert_eq!(closed(&frame(&mut main, &[]).0), vec![1]);

    // Nothing to dismiss now, and nothing invented: a driver reporting a
    // stale id gets silence rather than an event about a closed window.
    main.dismiss_window(WindowId(1), DismissReason::Outside);
    main.dismiss_window(WindowId(7), DismissReason::Escape);
    assert!(main.take_pending_events().is_empty());
}
