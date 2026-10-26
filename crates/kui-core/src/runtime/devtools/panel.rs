//! The panel's frame: the dock (or the window) with its header, its tab
//! strip and the tab in view, and the small controls every tab shares.

use super::icons::{self, Icon};
use super::{DEVTOOLS_KEY, Dock, Place, Shown, State, Tab, action, perf, stream, tree};
use crate::color::Color;
use crate::env::Appearance;
use crate::menu::MenuItem;
use crate::runtime::inspect::NodeInfo;
use crate::spec::TextStyle;
use crate::spec::{Align, FloatConfig, Min, NodeSpec, Sizing};
use crate::theme::Theme;
use crate::tree::OriginId;
use crate::ui::Ui;
use crate::widgets;

/// The whole of what the panel declares in this window.
pub(super) fn build(ui: &mut Ui<'_>, st: &mut State, nodes: &[NodeInfo], place: Place) {
    let t = ink(ui.theme());
    if !matches!(st.shown(), Shown::Builtin(Tab::Tree | Tab::Perf)) {
        st.hovered_row = None;
    }
    match place {
        Place::Window => {
            ui.window_title(&format!("kui devtools — {}", st.facts.title));
            let spec = NodeSpec::column().fill();
            panel(ui, st, nodes, &t, place, spec);
        }
        Place::Main(dock) if dock.docked() => {
            // The pane's extent is what the handle left, within its floor
            // and what the window has: the same number the host's
            // viewport was cut by (`Core::devtools_area`).
            let vp = st.facts.viewport;
            let spec = if dock.is_side() {
                NodeSpec::row()
                    .width(st.pane_w(vp.w))
                    .min_width(super::SIDE_MIN_W)
                    .grow_height()
            } else {
                NodeSpec::column()
                    .grow_width()
                    .height(st.pane_h(vp.h))
                    .min_height(super::BOTTOM_MIN_H)
            }
            // A Tab ring of its own that the app's never enters, and a
            // named group to assistive technology (ADR 0022).
            .focus_region()
            .label("Devtools");
            panel(ui, st, nodes, &t, place, spec);
        }
        Place::Main(_) => {
            // Nothing docked: a zero-size holder for the outlines and the
            // picker to float from, so everything the panel declares is
            // still under one key.
            ui.with_keyed(DEVTOOLS_KEY, NodeSpec::row().size(0.0, 0.0), |ui| {
                tree::overlays(ui, st, nodes, &t, place)
            });
        }
    }
}

/// The theme the panel paints with: the app's, with the accent held to
/// what can be *read* on the panel's surface — and everything that comes
/// off the accent recomputed from that.
///
/// The stock widgets use the accent as a fill with `on_accent` on top,
/// which is readable whatever the accent is. The panel uses it as ink:
/// icon strokes, the lit placement, a selected row's kind, the picker's
/// outline, a focused field's border, the legend's keys. An OS accent
/// that sits near the base — Windows' "automatic" accent off a dark
/// wallpaper is a navy on a near-black — is a fine fill and an invisible
/// stroke (the pomodoro's report, 2026-09-12). [`Theme::ink_for`] is
/// that promise — the ring's, held to 3:1 on `surface` and started from
/// the accent itself, so one that already reads is painted verbatim —
/// and this is the theme rebuilt around its answer. The facts row still
/// prints the accent in force; the panel's own window is drawn from this
/// too.
pub(super) fn ink(t: Theme) -> Theme {
    let ink = t.ink_for(t.accent);
    if ink == t.accent {
        t
    } else {
        t.with_accent(ink)
    }
}

fn panel(
    ui: &mut Ui<'_>,
    st: &mut State,
    nodes: &[NodeInfo],
    t: &Theme,
    place: Place,
    spec: NodeSpec,
) {
    let dock = ui.open_keyed(
        DEVTOOLS_KEY,
        spec.bg(t.surface).border(1.0, t.border).clip(),
    );
    // The inspect chord (Ctrl+Shift+I, or what the app respelled it to):
    // into the dock, or back out to where the app had the keyboard. Resolved when this frame finishes, so the frame that
    // brought the dock back enters it.
    if std::mem::take(&mut st.toggle_region) && matches!(place, Place::Main(_)) {
        ui.focus_region(if ui.region() == Some(dock) {
            None
        } else {
            Some(dock)
        });
    }
    // Docked, the pane's inner edge is a handle: a drag on it resizes the
    // pane (the core reads the pointer in `devtools_consume`). It comes
    // before the content on the edge that faces the app — the left edge
    // of a right dock, the top of a bottom one — and after it on a left
    // dock.
    let docked = match place {
        Place::Main(d) if d.docked() => Some(d),
        _ => None,
    };
    if matches!(docked, Some(Dock::Right | Dock::Bottom)) {
        handle(ui, t, docked.unwrap());
    }
    ui.open(NodeSpec::column().fill().clip());
    // The strip across the top — the tabs, then the placements — and a
    // hairline under it the whole width of the pane, as a browser's
    // devtools draw theirs.
    fixed(ui, |ui| header(ui, st, t));
    ui.leaf(NodeSpec::row().grow_width().height(1.0).bg(t.border));
    ui.open(NodeSpec::column().fill().pad(10.0).gap(8.0).clip());
    match st.shown() {
        Shown::Custom(i) => tab_body(ui, st, i, place),
        Shown::Builtin(Tab::Facts) => {
            // The frame timing is the Perf tab's.
            ui.with(NodeSpec::column().fill().gap(10.0).scroll_y(), |ui| {
                facts(ui, st, t);
                fixed(ui, |ui| tokens(ui, st, t));
                fixed(ui, |ui| legend(ui, st, t));
            });
        }
        Shown::Builtin(Tab::Events) => stream::events_tab(ui, st, t),
        Shown::Builtin(Tab::Perf) => perf::perf_tab(ui, st, t, place),
        Shown::Builtin(Tab::Tree) => tree::tree_tab(ui, st, nodes, t, place),
    }
    ui.close();
    if matches!(place, Place::Main(_)) {
        tree::overlays(ui, st, nodes, t, place);
    }
    ui.close();
    if docked == Some(Dock::Left) {
        handle(ui, t, Dock::Left);
    }
    ui.close();
}

/// A declared tab's body: an empty node that
/// takes the tab area, keyed by the tab's name so the content — a layer
/// built elsewhere, by the host or by an extension's fill — can anchor
/// to it by key. The host form's content lives in the main window's
/// tree, so in the panel's own window the body says so instead.
fn tab_body(ui: &mut Ui<'_>, st: &State, i: usize, place: Place) {
    let decl = &st.tabs[i];
    let key = super::tab_body_key(&decl.name);
    ui.open_with_key(
        key,
        &format!("{DEVTOOLS_KEY}/tab/{}", decl.name),
        NodeSpec::column().fill().clip().label(decl.label.as_str()),
    );
    if place == Place::Window && decl.slot.is_none() {
        let t = ink(ui.theme());
        ui.text(
            "docked only — this tab is drawn from the app's own tree",
            TextStyle::new(12.0).color(t.muted),
        );
    }
    ui.close();
}

/// The resize handle on a docked pane's inner edge: a strip that shows
/// on hover, dragged in the axis the pane grows in.
fn handle(ui: &mut Ui<'_>, t: &Theme, dock: Dock) {
    let spec = if dock.is_side() {
        NodeSpec::row()
            .width(6.0)
            .grow_height()
            .cursor(crate::cursor::CursorShape::EwResize)
    } else {
        NodeSpec::row()
            .grow_width()
            .height(6.0)
            .cursor(crate::cursor::CursorShape::NsResize)
    };
    ui.leaf_keyed(
        "kui-devtools/resize",
        spec.hover_bg(t.accent_soft)
            .pressed_bg(t.accent)
            .on_drag(action("resize"))
            .role(crate::access::Role::None)
            .label("resize the panel"),
    );
}

/// The strip across the top: the tabs on the left, scrolling sideways
/// when there are more than fit, then the menu that holds the placements
/// (as a browser's devtools keep their dock side in theirs) and the
/// close. The window's title and the frame counter are rows of the Facts
/// tab: the strip was where a narrow dock cut the title to "kawoosh — k…"
/// to make room for them. The app-state toggles sit on the Facts rows
/// they change.
fn header(ui: &mut Ui<'_>, st: &mut State, t: &Theme) {
    ui.with(
        NodeSpec::row()
            .grow_width()
            .pad_xy(4.0, 0.0)
            .cross_align(Align::Center)
            .gap(2.0),
        |ui| {
            let overflows = tabs(ui, st, t);
            // The menu's button, and the menu floating under it while
            // open: anchored to the wrapper, which is the button's box.
            ui.with(NodeSpec::row(), |ui| {
                icon_lit(ui, t, "more", Icon::More, None, "dock side and more");
                if st.more_open {
                    more_menu(ui, st, t, overflows);
                }
            });
            icon_lit(
                ui,
                t,
                &format!("dock:{}", Dock::Off.name()),
                Icon::Close,
                None,
                "close · Ctrl+Shift+D",
            );
        },
    );
}

/// The menu under the strip's "⋮": the placements as one row of icons,
/// the current one lit — a radio group, one Tab stop the arrows walk (ADR
/// 0007), the walk `Ctrl+Shift+D` makes a press at a time — the close
/// with its chord, and, when the strip cannot show every tab, every tab
/// to go to. A press outside it, Escape or a choice closes it.
fn more_menu(ui: &mut Ui<'_>, st: &State, t: &Theme, overflows: bool) {
    ui.with_keyed(
        "kui-devtools/more-menu",
        NodeSpec::column()
            .float(
                FloatConfig::parent()
                    .at(Align::End, Align::End)
                    .self_at(Align::End, Align::Start)
                    .offset(0.0, 4.0)
                    .fit()
                    .escaping(),
            )
            .modal(action("more-close"))
            .role(crate::access::Role::Menu)
            .label("Devtools menu")
            .width(Sizing::Fit)
            .min_width(200.0)
            .pad(8.0)
            .gap(6.0)
            .bg(t.raised)
            .border(1.0, t.border)
            .radius(6.0),
        |ui| {
            ui.with(
                NodeSpec::row()
                    .grow_width()
                    .gap(6.0)
                    .cross_align(Align::Center),
                |ui| {
                    ui.text_in(
                        NodeSpec::row().grow_width(),
                        "Dock side",
                        TextStyle::new(11.0).color(t.fg).nowrap(),
                    );
                    ui.with(
                        NodeSpec::row()
                            .gap(2.0)
                            .cross_align(Align::Center)
                            .role(crate::access::Role::RadioGroup)
                            .label("Dock side"),
                        |ui| {
                            for dock in Dock::ALL {
                                let (glyph, what) = match dock {
                                    Dock::Left => (Icon::DockLeft, "dock on the left"),
                                    Dock::Right => (Icon::DockRight, "dock on the right"),
                                    Dock::Bottom => (Icon::DockBottom, "dock at the bottom"),
                                    Dock::Window => (Icon::DockWindow, "undock into a window"),
                                    Dock::Off => continue,
                                };
                                icon_lit(
                                    ui,
                                    t,
                                    &format!("dock:{}", dock.name()),
                                    glyph,
                                    Some(dock == st.dock),
                                    what,
                                );
                            }
                        },
                    );
                },
            );
            menu_row(ui, t, "more-close-panel", "Close devtools", "Ctrl+Shift+D");
            if overflows {
                ui.leaf(NodeSpec::row().grow_width().height(1.0).bg(t.border));
                let shown = st.shown();
                for (id, label, which) in tab_list(st) {
                    let mark = if which == shown { "●" } else { "" };
                    menu_row(ui, t, &format!("more-tab:{id}"), &label, mark);
                }
            }
        },
    );
}

/// A row of the "⋮" menu: its text, and on the right a chord or a mark.
fn menu_row(ui: &mut Ui<'_>, t: &Theme, what: &str, text: &str, aside: &str) {
    ui.with_keyed(
        &format!("kui-devtools/{what}"),
        NodeSpec::row()
            .grow_width()
            .pad_xy(6.0, 4.0)
            .gap(12.0)
            .radius(4.0)
            .hover_bg(t.hover)
            .on_click(action(what))
            .role(crate::access::Role::MenuItem)
            .label(text),
        |ui| {
            ui.text_in(
                NodeSpec::row().grow_width(),
                text,
                TextStyle::new(11.0).color(t.fg).nowrap(),
            );
            if !aside.is_empty() {
                ui.text(aside, TextStyle::new(10.0).color(t.muted).nowrap());
            }
        },
    );
}

/// Every tab, in the strip's order: the panel's own, then every tab the
/// app or an extension declared, in declaration order (ADR 0032,
/// decision 1) — each its action's id, its label and which it shows.
fn tab_list(st: &State) -> Vec<(String, String, Shown)> {
    let own = Tab::ALL.iter().map(|tab| {
        (
            tab.name().to_string(),
            tab.label().to_string(),
            Shown::Builtin(*tab),
        )
    });
    let declared = st.tabs.iter().enumerate().map(|(i, d)| {
        (
            format!("custom:{}", d.name),
            d.label.clone(),
            Shown::Custom(i),
        )
    });
    own.chain(declared).collect()
}

/// The tab strip, growing into the strip's leftover width, on one line:
/// a tab is one unbreakable unit — its label never wraps inside it — and
/// when they do not all fit the strip scrolls sideways, a mouse wheel
/// over it too, with the one on show brought into view as it changes,
/// and the "⋮" menu lists them all. The one on show is underlined in the
/// accent, the rest plain text. True when the strip overflows.
fn tabs(ui: &mut Ui<'_>, st: &mut State, t: &Theme) -> bool {
    let strip = ui.child_key("kui-devtools/tabs");
    let overflows = ui
        .scroll_geometry(strip)
        .is_some_and(|g| g.max_offset.x > 0.5);
    let shown = st.shown();
    // Only a strip that overflows has a tab out of view to bring in:
    // asking otherwise would cost a frame for nothing.
    let reveal = overflows && st.tab_revealed != Some(shown);
    st.tab_revealed = Some(shown);
    ui.with_keyed(
        "kui-devtools/tabs",
        NodeSpec::row()
            .grow_width()
            .min_width(0.0)
            .cross_align(Align::End)
            .scroll_x()
            .scrollbar(crate::spec::ScrollbarMode::Hidden)
            // One Tab stop, the arrows walking the tabs (ADR 0007).
            .role(crate::access::Role::TabList)
            .label("Devtools"),
        |ui| {
            for (id, label, which) in tab_list(st) {
                let on = which == shown;
                let key = ui.with_keyed(
                    &format!("kui-devtools/tab-{id}"),
                    NodeSpec::column()
                        .height(30.0)
                        .hover_bg(t.hover)
                        .on_click(action(format!("tab:{id}")))
                        .role(crate::access::Role::Tab)
                        .selected(on)
                        .label(label.as_str())
                        .apply_tooltip("Ctrl+Shift+N · the next tab"),
                    |ui| {
                        ui.text_in(
                            NodeSpec::row().grow_height().pad_xy(6.0, 0.0).center(),
                            &label,
                            TextStyle::new(11.0)
                                .color(if on { t.fg } else { t.muted })
                                .nowrap(),
                        );
                        ui.leaf(
                            NodeSpec::row()
                                .grow_width()
                                .height(2.0)
                                .radius_top(1.0)
                                .bg(if on { t.accent } else { Color::TRANSPARENT }),
                        );
                    },
                );
                if on && reveal {
                    ui.core().reveal(key);
                }
            }
        },
    );
    overflows
}

/// The status block's sections, by the names of the rows each holds, in
/// the order they print. A row the core writes that no section names
/// lands in a last one, so a new fact is never dropped on the floor.
const SECTIONS: [(&str, &[&str]); 4] = [
    (
        "window",
        &[
            "title",
            "window",
            "viewport",
            "keyboard",
            "focus",
            "region",
            "modifiers",
        ],
    ),
    ("theme", &["theme", "accent", "menus", "fonts"]),
    ("system", &["appearance", "motion", "locale", "assistive"]),
    ("runtime", &["frame", "nodes", "audio"]),
];

/// The status block: the rows the main window's core wrote, in sections,
/// each a table — the name column at its longest name, the value column
/// growing and cut with an ellipsis where the pane is narrower than it.
/// The window's title and the frame counter come first, from the panel's
/// own state. Three of the rows — `theme`, `accent`, `menus` — are what
/// the panel can override, and each carries its select in a third
/// column beside the fact it changes: the fact is what the app has, the
/// select what the panel holds it to (`app` leaves the app's own). The
/// chords (`Ctrl+Shift+T` / `A` / `M`) cycle the same choices.
fn facts(ui: &mut Ui<'_>, st: &State, t: &Theme) {
    let frames = match st.smoke_frames {
        Some(n) => format!("{} / {n}", st.frames),
        None => format!("{}", st.frames),
    };
    let mut rows: Vec<(&str, &str)> = vec![("title", st.facts.title.as_str())];
    rows.push(("frame", frames.as_str()));
    rows.extend(st.facts.rows.iter().map(|(k, v)| (*k, v.as_str())));
    let named = |k: &str| SECTIONS.iter().any(|(_, names)| names.contains(&k));
    let rest: Vec<&str> = rows.iter().map(|(k, _)| *k).filter(|k| !named(k)).collect();
    let sections = SECTIONS
        .iter()
        .map(|(name, names)| (*name, names.to_vec()))
        .chain((!rest.is_empty()).then_some(("other", rest)));
    for (name, names) in sections {
        fixed(ui, |ui| {
            section(ui, t, name);
            ui.with(
                NodeSpec::table()
                    .grow_width()
                    .min_height(Min::FIT)
                    .gap(3.0)
                    .pad_xy(2.0, 4.0),
                |ui| {
                    for k in names {
                        let Some((_, v)) = rows.iter().find(|(name, _)| *name == k) else {
                            continue;
                        };
                        ui.with(
                            NodeSpec::row()
                                .grow_width()
                                .gap(8.0)
                                .cross_align(Align::Center),
                            |ui| {
                                ui.text(k, TextStyle::new(11.0).color(t.muted));
                                ui.text_in(
                                    NodeSpec::row().grow_width(),
                                    v,
                                    TextStyle::new(11.0).color(t.fg).mono().nowrap().ellipsis(),
                                );
                                match k {
                                    "theme" => override_select(ui, st, "base"),
                                    "accent" => override_select(ui, st, "accent"),
                                    "menus" => override_select(ui, st, "menus"),
                                    _ => {}
                                }
                            },
                        );
                    }
                },
            );
        });
    }
}

/// A section's heading: its name small and muted, and a hairline running
/// on to the end of the row — what the Facts tab's blocks and the
/// inspector's groups are each headed by.
pub(super) fn section(ui: &mut Ui<'_>, t: &Theme, name: &str) {
    ui.with(
        NodeSpec::row()
            .grow_width()
            .gap(6.0)
            .cross_align(Align::Center)
            .role(crate::access::Role::None),
        |ui| {
            ui.text(name, TextStyle::new(10.0).color(t.muted).bold().nowrap());
            ui.leaf(NodeSpec::row().grow_width().height(1.0).bg(t.border));
        },
    );
}

/// One override as a select: its choices, the one in force, and the
/// action each posts (`base:light`, `accent:2`, `menus:drawn`).
fn override_select(ui: &mut Ui<'_>, st: &State, what: &str) {
    let (items, current): (Vec<MenuItem>, usize) = match what {
        "base" => {
            let own = format!("app ({})", st.facts.app_appearance.name());
            let items = vec![
                MenuItem::new(own).id(action("base:app")),
                MenuItem::new("light").id(action("base:light")),
                MenuItem::new("dark").id(action("base:dark")),
            ];
            let current = match st.base {
                None => 0,
                Some(Appearance::Light) => 1,
                Some(_) => 2,
            };
            (items, current)
        }
        "accent" => {
            let mut items = vec![MenuItem::new("app").id(action("accent:app"))];
            items.extend(
                super::ACCENTS
                    .iter()
                    .enumerate()
                    .map(|(i, (name, _))| MenuItem::new(*name).id(action(format!("accent:{i}")))),
            );
            (items, st.accent.map_or(0, |i| i + 1))
        }
        _ => {
            let items = vec![
                MenuItem::new("platform").id(action("menus:default")),
                MenuItem::new("native").id(action("menus:native")),
                MenuItem::new("drawn").id(action("menus:drawn")),
            ];
            let current = match st.native_menus {
                None => 0,
                Some(true) => 1,
                Some(false) => 2,
            };
            (items, current)
        }
    };
    // The third column of the facts table: the value column grows, so the
    // field sits at the row's right edge whatever the value's length.
    let t = ink(ui.theme());
    widgets::select_with(
        ui,
        &format!("kui-devtools/{what}"),
        &items,
        Some(current),
        NodeSpec::row()
            .pad_xy(6.0, 1.0)
            .gap(4.0)
            .cross_align(Align::Center)
            .bg(t.sunken)
            .hover_bg(t.hover)
            .radius(4.0),
        TextStyle::new(11.0),
    );
}

/// The declared tokens: a swatch and the hex for a
/// colour — both halves when they differ, the one in effect first — the
/// px for a length, grouped under the origin that declared them. A table
/// per origin: the swatches, the name, the value and the
/// recipe are its four columns, the name column at its longest name.
fn tokens(ui: &mut Ui<'_>, st: &State, t: &Theme) {
    if st.facts.tokens.is_empty() {
        return;
    }
    let dark = t.is_dark();
    ui.with(NodeSpec::column().grow_width().gap(2.0), |ui| {
        let mut last_origin = None;
        let mut open = false;
        for tok in &st.facts.tokens {
            if last_origin != Some(tok.origin) {
                if open {
                    ui.close();
                }
                last_origin = Some(tok.origin);
                let who = if tok.origin == OriginId::HOST {
                    "tokens".to_string()
                } else {
                    format!("tokens · origin {}", tok.origin.0)
                };
                section(ui, t, &who);
                ui.open(NodeSpec::table().grow_width().gap(2.0).pad_xy(2.0, 4.0));
                open = true;
            }
            ui.with(
                NodeSpec::row()
                    .grow_width()
                    .gap(6.0)
                    .cross_align(Align::Center),
                |ui| {
                    // The swatch column: two samples for a colour whose
                    // halves differ, one for a colour, none for a length.
                    ui.with(NodeSpec::row().gap(6.0), |ui| {
                        if tok.kind == crate::tokens::TokenKind::Color {
                            let (first, second) = if dark {
                                (tok.dark, tok.light)
                            } else {
                                (tok.light, tok.dark)
                            };
                            swatch(ui, first, t);
                            if tok.light != tok.dark {
                                swatch(ui, second, t);
                            }
                        }
                    });
                    ui.text(&tok.name, TextStyle::new(11.0).color(t.fg).nowrap());
                    let value = match tok.kind {
                        crate::tokens::TokenKind::Color => {
                            if tok.light != tok.dark {
                                format!(
                                    "#{:08x} · #{:08x}",
                                    if dark { tok.dark } else { tok.light }.to_hex(),
                                    if dark { tok.light } else { tok.dark }.to_hex()
                                )
                            } else {
                                format!("#{:08x}", tok.resolved.to_hex())
                            }
                        }
                        crate::tokens::TokenKind::Length => length_value(tok.length),
                        crate::tokens::TokenKind::Family => match &tok.family {
                            Some(f) => f.clone(),
                            None => "sans (none matched)".to_string(),
                        },
                    };
                    ui.text(&value, TextStyle::new(11.0).color(t.muted).mono().nowrap());
                    // A derived token (ADR 0028) prints its recipe after the
                    // value, so the panel says where the colour came from.
                    if let Some(recipe) = &tok.recipe {
                        ui.text(recipe, TextStyle::new(11.0).color(t.faint).nowrap());
                    }
                },
            );
        }
        if open {
            ui.close();
        }
    });
}

/// A length token as the panel prints it: the number, to at most three
/// decimals with the trailing zeros cut, and no unit. A length token is
/// any number (ADR 0049) — px where the slot is a length, the number
/// itself in an `opacity` — so `fade: 0.4` reads `0.4`, not `0 px`.
fn length_value(v: f32) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0" } else { s }.to_string()
}

/// A 12 px colour sample with a hairline, so a colour near the panel's
/// own surface still reads as a sample.
fn swatch(ui: &mut Ui<'_>, c: Color, t: &Theme) {
    ui.leaf(
        NodeSpec::row()
            .size(12.0, 12.0)
            .radius(2.0)
            .bg(c)
            .border(1.0, t.border),
    );
}

/// The chords and what they do: a two-column table, the keys
/// column at its longest chord.
fn legend(ui: &mut Ui<'_>, st: &State, t: &Theme) {
    if st.legend.is_empty() {
        return;
    }
    section(ui, t, "keys");
    ui.with(
        NodeSpec::table().grow_width().gap(2.0).pad_xy(2.0, 4.0),
        |ui| {
            for (keys, what) in &st.legend {
                ui.with(NodeSpec::row().grow_width().gap(8.0), |ui| {
                    ui.text(keys, TextStyle::new(11.0).color(t.accent));
                    ui.text(what, TextStyle::new(11.0).color(t.muted));
                });
            }
        },
    );
}

/// A section of the panel that keeps its height whatever the window's.
pub(super) fn fixed(ui: &mut Ui<'_>, f: impl FnOnce(&mut Ui<'_>)) {
    ui.with(NodeSpec::column().grow_width().min_height(Min::FIT), f);
}

/// One of the header's placement radios, a choice among several: `Some(on)`
/// paints it as the one chosen
/// — the strokes in the accent and the pane filled with it, against the
/// muted outline and a faint pane of the others. `None` is a plain button
/// drawn the same way, unlit: the close beside them. It and the button shapes
/// below declare the hand, as the stock button does (`crate::cursor`):
/// the dock's tabs and rows do not, since a native tab strip and list are
/// the arrow.
fn icon_lit(ui: &mut Ui<'_>, t: &Theme, what: &str, glyph: Icon, radio: Option<bool>, hint: &str) {
    let label = format!("kui-devtools/{what}");
    let key = ui.child_key(&label);
    let on = radio == Some(true);
    let mut spec = NodeSpec::row();
    if let Some(on) = radio {
        spec = spec.role(crate::access::Role::Radio).checked(on);
    }
    ui.with_keyed(
        &label,
        spec.size(20.0, 20.0)
            .center()
            .radius(4.0)
            .bg(if on { t.accent_soft } else { t.surface })
            .hover_bg(if on { t.accent_soft } else { t.hover })
            .pressed_bg(t.pressed)
            .on_click(action(what))
            .cursor(crate::cursor::CursorShape::Pointer)
            .label(hint.split(" · ").next().unwrap_or(what))
            .apply_tooltip(hint),
        |ui| {
            let color = if on { t.accent } else { t.muted };
            icons::draw(ui, glyph, color, color.faded(if on { 0.9 } else { 0.3 }));
            if ui.is_hovered(key) {
                widgets::hover_hint(ui, hint);
            }
        },
    );
}

/// One of a toolbar's icon buttons, drawn as the header's are: `None` a
/// plain button (fold every row), `Some(on)` a toggle that reads as a
/// switch to assistive technology and is lit while set (the picker, the
/// highlight of the selected node).
pub(super) fn icon_button(
    ui: &mut Ui<'_>,
    t: &Theme,
    what: &str,
    glyph: Icon,
    toggle: Option<bool>,
    hint: &str,
) {
    let label = format!("kui-devtools/{what}");
    let key = ui.child_key(&label);
    let on = toggle == Some(true);
    let mut spec = NodeSpec::row();
    if let Some(on) = toggle {
        spec = spec.role(crate::access::Role::Switch).checked(on);
    }
    ui.with_keyed(
        &label,
        spec.size(22.0, 22.0)
            .center()
            .radius(4.0)
            .bg(if on {
                t.accent_soft
            } else {
                Color::TRANSPARENT
            })
            .hover_bg(if on { t.accent_soft } else { t.hover })
            .pressed_bg(t.pressed)
            .on_click(action(what))
            .cursor(crate::cursor::CursorShape::Pointer)
            .label(hint.split(" · ").next().unwrap_or(what))
            .apply_tooltip(hint),
        |ui| {
            let color = if on { t.accent } else { t.muted };
            icons::draw(
                ui,
                glyph,
                color,
                if on { t.accent } else { Color::TRANSPARENT },
            );
            if ui.is_hovered(key) {
                widgets::hover_hint(ui, hint);
            }
        },
    );
}

/// A control the height of a toolbar: the panel's own, drawn off the
/// palette, posting the action its chord posts. `on` paints it as a
/// toggle that is set.
pub(super) fn small_button(
    ui: &mut Ui<'_>,
    t: &Theme,
    what: &str,
    text: &str,
    hint: &str,
    on: bool,
) {
    let label = format!("kui-devtools/{what}");
    let key = ui.child_key(&label);
    ui.with_keyed(
        &label,
        NodeSpec::row()
            .height(22.0)
            .pad_xy(8.0, 0.0)
            .radius(4.0)
            .cross_align(Align::Center)
            .bg(if on {
                t.accent_soft
            } else {
                Color::TRANSPARENT
            })
            .hover_bg(if on { t.accent_soft } else { t.hover })
            .pressed_bg(t.pressed)
            .on_click(action(what))
            .cursor(crate::cursor::CursorShape::Pointer)
            .label(text)
            .apply_tooltip(hint),
        |ui| {
            ui.text(
                text,
                TextStyle::new(11.0)
                    .color(if on { t.accent } else { t.muted })
                    .nowrap(),
            );
            if ui.is_hovered(key) {
                widgets::hover_hint(ui, hint);
            }
        },
    );
}

/// A one-line filter field that names itself by its placeholder.
/// Returns the text it holds.
pub(super) fn filter_field(ui: &mut Ui<'_>, t: &Theme, what: &str, name: &str) -> String {
    let label = format!("kui-devtools/{what}");
    let key = ui.child_key(&label);
    let focused = ui.is_focused(key);
    ui.text_edit(
        &label,
        "",
        &crate::edit::EditOptions {
            style: TextStyle::new(11.0).mono(),
            multiline: false,
            placeholder: Some(name.to_string()),
            ..Default::default()
        },
        NodeSpec::column()
            .grow_width()
            .min_width(60.0)
            .pad_xy(8.0, 4.0)
            .bg(t.sunken)
            .radius(4.0)
            .border(1.0, if focused { t.accent } else { t.border })
            .clip()
            .label(name),
    );
    ui.edit_text(key).unwrap_or_default()
}
