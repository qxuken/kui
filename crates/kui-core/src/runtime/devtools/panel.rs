//! The panel's frame: the dock (or the window) with its header, its tab
//! strip and the tab in view, and the small controls every tab shares.

use super::icons::{self, Icon};
use super::{DEVTOOLS_KEY, Dock, Place, State, Tab, action, stream, tree};
use crate::color::Color;
use crate::env::Appearance;
use crate::runtime::inspect::NodeInfo;
use crate::spec::TextStyle;
use crate::spec::{Align, Min, NodeSpec, Sizing};
use crate::theme::Theme;
use crate::ui::Ui;
use crate::widgets;

/// The whole of what the panel declares in this window.
pub(super) fn build(ui: &mut Ui<'_>, st: &mut State, nodes: &[NodeInfo], place: Place) {
    let t = ui.theme();
    if st.tab != Tab::Tree {
        st.hovered_row = None;
    }
    match place {
        Place::Window => {
            ui.window_title(&format!("kui devtools — {}", st.facts.title));
            let spec = NodeSpec::column()
                .width(Sizing::Grow(1.0))
                .height(Sizing::Grow(1.0));
            panel(ui, st, nodes, &t, place, spec);
        }
        Place::Main(dock) if dock.docked() => {
            // The pane's extent is what the handle left, within its floor
            // and what the window has: the same number the host's
            // viewport was cut by (`Core::devtools_area`).
            let vp = st.facts.viewport;
            let spec = if dock.is_side() {
                NodeSpec::row()
                    .width(Sizing::Fixed(super::pane_w(st.side_w, vp.w)))
                    .min_width(super::SIDE_MIN_W)
                    .height(Sizing::Grow(1.0))
            } else {
                NodeSpec::column()
                    .width(Sizing::Grow(1.0))
                    .height(Sizing::Fixed(super::pane_h(st.bottom_h, vp.h)))
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
            ui.with_keyed(
                DEVTOOLS_KEY,
                NodeSpec::row()
                    .width(Sizing::Fixed(0.0))
                    .height(Sizing::Fixed(0.0)),
                |ui| tree::overlays(ui, st, nodes, &t, place),
            );
        }
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
    // Ctrl+Shift+I: into the dock, or back out to where the app had the
    // keyboard. Resolved when this frame finishes, so the frame that
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
    ui.open(
        NodeSpec::column()
            .width(Sizing::Grow(1.0))
            .height(Sizing::Grow(1.0))
            .pad(10.0)
            .gap(8.0)
            .clip(),
    );
    fixed(ui, |ui| header(ui, st, t));
    fixed(ui, |ui| tabs(ui, st, t));
    match st.tab {
        Tab::Facts => {
            ui.with(
                NodeSpec::column()
                    .width(Sizing::Grow(1.0))
                    .height(Sizing::Grow(1.0))
                    .gap(10.0)
                    .scroll_y(),
                |ui| {
                    // The graph is this window's frame timing, which is
                    // the app's only where the dock shares its window.
                    if matches!(place, Place::Main(_)) {
                        fixed(ui, widgets::latency_graph);
                    }
                    facts(ui, st, t);
                    fixed(ui, |ui| legend(ui, st, t));
                },
            );
        }
        Tab::Events => stream::events_tab(ui, st, t),
        Tab::Tree => tree::tree_tab(ui, st, nodes, t, place),
    }
    if matches!(place, Place::Main(_)) {
        tree::overlays(ui, st, nodes, t, place);
    }
    ui.close();
    if docked == Some(Dock::Left) {
        handle(ui, t, Dock::Left);
    }
    ui.close();
}

/// The resize handle on a docked pane's inner edge: a strip that shows
/// on hover, dragged in the axis the pane grows in.
fn handle(ui: &mut Ui<'_>, t: &Theme, dock: Dock) {
    let spec = if dock.is_side() {
        NodeSpec::row()
            .width(Sizing::Fixed(6.0))
            .height(Sizing::Grow(1.0))
            .cursor(crate::cursor::CursorShape::EwResize)
    } else {
        NodeSpec::row()
            .width(Sizing::Grow(1.0))
            .height(Sizing::Fixed(6.0))
            .cursor(crate::cursor::CursorShape::NsResize)
    };
    ui.with_keyed(
        "kui-devtools/resize",
        spec.hover_bg(t.accent_soft)
            .pressed_bg(t.accent)
            .on_drag(action("resize"))
            .role(crate::access::Role::None)
            .label("resize the panel"),
        |_| {},
    );
}

/// The title, the frame counter, and the placement buttons — one per
/// place the panel can sit, the current one lit. The app-state toggles
/// are at the end of the tab row.
fn header(ui: &mut Ui<'_>, st: &State, t: &Theme) {
    ui.with(
        NodeSpec::row()
            .width(Sizing::Grow(1.0))
            .cross_align(Align::Center)
            .gap(6.0),
        |ui| {
            ui.text(
                &st.facts.title,
                TextStyle::new(13.0).color(t.fg).nowrap().ellipsis(),
            );
            let frames = match st.smoke_frames {
                Some(n) => format!("{} / {n}", st.frames),
                None => format!("{}", st.frames),
            };
            ui.text(&frames, TextStyle::new(11.0).color(t.faint).mono());
            // One button per placement, the current one lit; the same
            // walk `Ctrl+Shift+D` makes, one press at a time.
            ui.with(NodeSpec::row().width(Sizing::Grow(1.0)), |_| {});
            for dock in Dock::ALL {
                let (glyph, what) = match dock {
                    Dock::Left => (Icon::DockLeft, "dock on the left"),
                    Dock::Right => (Icon::DockRight, "dock on the right"),
                    Dock::Bottom => (Icon::DockBottom, "dock at the bottom"),
                    Dock::Window => (Icon::DockWindow, "undock into a window"),
                    Dock::Off => (Icon::Close, "close · Ctrl+Shift+D brings it back"),
                };
                icon_lit(
                    ui,
                    t,
                    &format!("dock:{}", dock.name()),
                    glyph,
                    dock == st.dock,
                    &format!("{what} · Ctrl+Shift+D walks left → right → bottom → window → off"),
                );
            }
        },
    );
}

fn tabs(ui: &mut Ui<'_>, st: &State, t: &Theme) {
    ui.with(
        NodeSpec::row()
            .width(Sizing::Grow(1.0))
            .gap(2.0)
            .cross_align(Align::Center),
        |ui| {
            for tab in Tab::ALL {
                let on = tab == st.tab;
                ui.with_keyed(
                    &format!("kui-devtools/tab-{}", tab.name()),
                    NodeSpec::row()
                        .pad_xy(10.0, 4.0)
                        .radius_top(5.0)
                        .bg(if on { t.sunken } else { t.surface })
                        .hover_bg(if on { t.sunken } else { t.hover })
                        .on_click(action(format!("tab:{}", tab.name())))
                        .role(crate::access::Role::Tab)
                        .selected(on)
                        .label(tab.name())
                        .apply_tooltip("Ctrl+Shift+N · the next tab"),
                    |ui| {
                        ui.text(
                            tab.name(),
                            TextStyle::new(11.0).color(if on { t.fg } else { t.muted }),
                        );
                    },
                );
            }
            // The app-state toggles — the theme base, the accent, native
            // menus — at the end of the tab row.
            ui.with(NodeSpec::row().width(Sizing::Grow(1.0)), |_| {});
            let base = match st.base {
                None => Icon::BaseAuto,
                Some(Appearance::Light) => Icon::BaseLight,
                Some(_) => Icon::BaseDark,
            };
            icon(
                ui,
                t,
                "base",
                base,
                t.fg,
                &format!(
                    "base: {} · Ctrl+Shift+T cycles the app's own → light → dark",
                    st.base_name()
                ),
            );
            icon(
                ui,
                t,
                "accent",
                Icon::Accent,
                t.accent,
                &format!(
                    "accent: {} · Ctrl+Shift+A cycles the app's, kui's, four the OS might report",
                    st.accent_name()
                ),
            );
            let menus = match st.native_menus {
                Some(true) => "native",
                Some(false) => "drawn",
                None => "the platform's default",
            };
            icon(
                ui,
                t,
                "menus",
                Icon::Menus,
                if st.native_menus == Some(false) {
                    t.accent
                } else {
                    t.fg
                },
                &format!("menus: {menus} · Ctrl+Shift+M toggles native and drawn"),
            );
        },
    );
}

/// The status block: the rows the main window's core wrote.
fn facts(ui: &mut Ui<'_>, st: &State, t: &Theme) {
    ui.with(
        NodeSpec::column()
            .width(Sizing::Grow(1.0))
            .min_height(Min::FIT)
            .gap(3.0),
        |ui| {
            for (k, v) in &st.facts.rows {
                ui.with(
                    NodeSpec::row()
                        .width(Sizing::Grow(1.0))
                        .gap(8.0)
                        .cross_align(Align::Center),
                    |ui| {
                        ui.with(NodeSpec::row().width(Sizing::Fixed(70.0)), |ui| {
                            ui.text(k, TextStyle::new(11.0).color(t.muted));
                        });
                        ui.text(v, TextStyle::new(11.0).color(t.fg).mono().nowrap());
                    },
                );
            }
        },
    );
}

fn legend(ui: &mut Ui<'_>, st: &State, t: &Theme) {
    if st.legend.is_empty() {
        return;
    }
    ui.with(NodeSpec::column().width(Sizing::Grow(1.0)).gap(2.0), |ui| {
        for (keys, what) in &st.legend {
            ui.with(NodeSpec::row().width(Sizing::Grow(1.0)).gap(8.0), |ui| {
                ui.with(NodeSpec::row().width(Sizing::Fixed(90.0)), |ui| {
                    ui.text(keys, TextStyle::new(11.0).color(t.accent));
                });
                ui.text(what, TextStyle::new(11.0).color(t.muted));
            });
        }
    });
}

/// A section of the panel that keeps its height whatever the window's.
pub(super) fn fixed(ui: &mut Ui<'_>, f: impl FnOnce(&mut Ui<'_>)) {
    ui.with(
        NodeSpec::column()
            .width(Sizing::Grow(1.0))
            .min_height(Min::FIT),
        f,
    );
}

/// One icon in the header's strip: what it controls is in its tooltip,
/// and its colour says its state.
pub(super) fn icon(ui: &mut Ui<'_>, t: &Theme, what: &str, glyph: Icon, color: Color, hint: &str) {
    let label = format!("kui-devtools/{what}");
    let key = ui.child_key(&label);
    ui.with_keyed(
        &label,
        NodeSpec::row()
            .width(Sizing::Fixed(22.0))
            .height(Sizing::Fixed(22.0))
            .center()
            .radius(4.0)
            .hover_bg(t.hover)
            .pressed_bg(t.pressed)
            .on_click(action(what))
            .label(what)
            .apply_tooltip(hint),
        |ui| {
            icons::draw(ui, glyph, color, color.with_alpha(0.35));
            if ui.is_hovered(key) {
                widgets::tooltip(ui, hint);
            }
        },
    );
}

/// [`icon`] for a choice among several: `on` paints it as the one chosen
/// — the strokes in the accent and the pane filled with it, against the
/// muted outline and a faint pane of the others.
fn icon_lit(ui: &mut Ui<'_>, t: &Theme, what: &str, glyph: Icon, on: bool, hint: &str) {
    let label = format!("kui-devtools/{what}");
    let key = ui.child_key(&label);
    ui.with_keyed(
        &label,
        NodeSpec::row()
            .width(Sizing::Fixed(22.0))
            .height(Sizing::Fixed(22.0))
            .center()
            .radius(4.0)
            .bg(if on { t.accent_soft } else { t.surface })
            .hover_bg(if on { t.accent_soft } else { t.hover })
            .pressed_bg(t.pressed)
            .on_click(action(what))
            .role(crate::access::Role::Radio)
            .checked(on)
            .label(hint.split(" · ").next().unwrap_or(what))
            .apply_tooltip(hint),
        |ui| {
            let color = if on { t.accent } else { t.muted };
            icons::draw(
                ui,
                glyph,
                color,
                color.with_alpha(if on { 0.9 } else { 0.3 }),
            );
            if ui.is_hovered(key) {
                widgets::tooltip(ui, hint);
            }
        },
    );
}

/// A control the size of the row it sits in: the panel's own, drawn off
/// the palette, posting the action its chord posts. `on` paints it as a
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
            .pad_xy(7.0, 1.0)
            .radius(4.0)
            .bg(if on { t.accent_soft } else { t.raised })
            .hover_bg(if on { t.accent_soft } else { t.hover })
            .pressed_bg(t.pressed)
            .border(1.0, if on { t.accent } else { t.border })
            .on_click(action(what))
            .label(text)
            .apply_tooltip(hint),
        |ui| {
            ui.text(
                text,
                TextStyle::new(10.0).color(if on { t.accent } else { t.fg }),
            );
            if ui.is_hovered(key) {
                widgets::tooltip(ui, hint);
            }
        },
    );
}

/// [`small_button`] with an icon before its text — the picker's
/// crosshair. The icon is drawn small to sit on the text's line.
pub(super) fn small_button_iconed(
    ui: &mut Ui<'_>,
    t: &Theme,
    what: &str,
    glyph: Icon,
    text: &str,
    hint: &str,
    on: bool,
) {
    let label = format!("kui-devtools/{what}");
    let key = ui.child_key(&label);
    ui.with_keyed(
        &label,
        NodeSpec::row()
            .pad_xy(6.0, 1.0)
            .gap(3.0)
            .radius(4.0)
            .cross_align(Align::Center)
            .bg(if on { t.accent_soft } else { t.raised })
            .hover_bg(if on { t.accent_soft } else { t.hover })
            .pressed_bg(t.pressed)
            .border(1.0, if on { t.accent } else { t.border })
            .on_click(action(what))
            .label(text)
            .apply_tooltip(hint),
        |ui| {
            let color = if on { t.accent } else { t.fg };
            icons::draw(ui, glyph, color, color.with_alpha(0.35));
            ui.text(text, TextStyle::new(10.0).color(color));
            if ui.is_hovered(key) {
                widgets::tooltip(ui, hint);
            }
        },
    );
}

/// A one-line filter field, `name` beside it. Returns the text it holds.
pub(super) fn filter_field(ui: &mut Ui<'_>, t: &Theme, what: &str, name: &str) -> String {
    let label = format!("kui-devtools/{what}");
    let key = ui.child_key(&label);
    let focused = ui.is_focused(key);
    ui.text(name, TextStyle::new(11.0).color(t.muted));
    ui.text_edit(
        &label,
        "",
        &crate::edit::EditOptions {
            style: TextStyle::new(11.0).mono(),
            multiline: false,
            ..Default::default()
        },
        NodeSpec::column()
            .width(Sizing::Grow(1.0))
            .pad_xy(6.0, 1.0)
            .bg(t.sunken)
            .radius(4.0)
            .border(1.0, if focused { t.accent } else { t.border })
            .clip()
            .label(name),
    );
    ui.edit_text(key).unwrap_or_default()
}
