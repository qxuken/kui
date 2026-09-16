//! The panel's frame: the dock (or the window) with its header, its tab
//! strip and the tab in view, and the small controls every tab shares.

use super::icons::{self, Icon};
use super::{DEVTOOLS_KEY, Dock, Place, Shown, State, Tab, action, stream, tree};
use crate::color::Color;
use crate::env::Appearance;
use crate::runtime::inspect::NodeInfo;
use crate::spec::TextStyle;
use crate::spec::{Align, Min, NodeSpec, Sizing};
use crate::theme::Theme;
use crate::tree::OriginId;
use crate::ui::Ui;
use crate::widgets;

/// The whole of what the panel declares in this window.
pub(super) fn build(ui: &mut Ui<'_>, st: &mut State, nodes: &[NodeInfo], place: Place) {
    let t = ink(ui.theme());
    if st.shown() != Shown::Builtin(Tab::Tree) {
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
    match st.shown() {
        Shown::Custom(i) => tab_body(ui, st, i, place),
        Shown::Builtin(Tab::Facts) => {
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
                    fixed(ui, |ui| tokens(ui, st, t));
                    fixed(ui, |ui| legend(ui, st, t));
                },
            );
        }
        Shown::Builtin(Tab::Events) => stream::events_tab(ui, st, t),
        Shown::Builtin(Tab::Tree) => tree::tree_tab(ui, st, nodes, t, place),
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

/// A declared tab's body (ADR 0032, decision 2): an empty node that
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
        NodeSpec::column()
            .width(Sizing::Grow(1.0))
            .height(Sizing::Grow(1.0))
            .clip()
            .label(decl.label.as_str()),
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
            let shown = st.shown();
            // The panel's three, then every tab the app or an extension
            // declared, in declaration order (ADR 0032, decision 1).
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
            for (id, label, which) in own.chain(declared) {
                let on = which == shown;
                ui.with_keyed(
                    &format!("kui-devtools/tab-{id}"),
                    NodeSpec::row()
                        .pad_xy(10.0, 4.0)
                        .radius_top(5.0)
                        .bg(if on { t.sunken } else { t.surface })
                        .hover_bg(if on { t.sunken } else { t.hover })
                        .on_click(action(format!("tab:{id}")))
                        .role(crate::access::Role::Tab)
                        .selected(on)
                        .label(label.as_str())
                        .apply_tooltip("Ctrl+Shift+N · the next tab"),
                    |ui| {
                        ui.text(
                            &label,
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
            // "The app's own" names the base it resolves to, since an
            // app that pinned one is not on the OS's.
            let own = match st.base {
                None => format!("app ({})", st.facts.app_appearance.name()),
                Some(_) => st.base_name().to_string(),
            };
            icon(
                ui,
                t,
                "base",
                base,
                t.fg,
                &format!("base: {own} · Ctrl+Shift+T cycles the app's own → light → dark"),
            );
            // The swatch is the accent in force — the core's, not the
            // panel's readable ink — inside a hairline in the panel's
            // muted, so it reads as a sample whatever colour it is.
            icon_with(
                ui,
                t,
                "accent",
                Icon::Accent,
                t.muted,
                ui.theme().accent,
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

/// The declared tokens (ADR 0027, decision 7): a swatch and the hex for a
/// colour — both halves when they differ, the one in effect first — the
/// px for a length, grouped under the origin that declared them.
fn tokens(ui: &mut Ui<'_>, st: &State, t: &Theme) {
    if st.facts.tokens.is_empty() {
        return;
    }
    let dark = t.is_dark();
    ui.with(NodeSpec::column().width(Sizing::Grow(1.0)).gap(2.0), |ui| {
        let mut last_origin = None;
        for tok in &st.facts.tokens {
            if last_origin != Some(tok.origin) {
                last_origin = Some(tok.origin);
                let who = if tok.origin == OriginId::HOST {
                    "tokens".to_string()
                } else {
                    format!("tokens · origin {}", tok.origin.0)
                };
                ui.text(&who, TextStyle::new(11.0).color(t.muted));
            }
            ui.with(
                NodeSpec::row()
                    .width(Sizing::Grow(1.0))
                    .gap(6.0)
                    .cross_align(Align::Center)
                    .padding(crate::geom::Edges {
                        l: 8.0,
                        r: 0.0,
                        t: 0.0,
                        b: 0.0,
                    }),
                |ui| {
                    if tok.kind == crate::tokens::TokenKind::Color {
                        let (first, second) = if dark {
                            (tok.dark, tok.light)
                        } else {
                            (tok.light, tok.dark)
                        };
                        swatch(ui, first, t);
                        if tok.light != tok.dark {
                            swatch(ui, second, t);
                        } else {
                            ui.with(NodeSpec::row().width(Sizing::Fixed(12.0)), |_| {});
                        }
                    } else {
                        ui.with(NodeSpec::row().width(Sizing::Fixed(30.0)), |_| {});
                    }
                    ui.with(NodeSpec::row().width(Sizing::Fixed(90.0)), |ui| {
                        ui.text(&tok.name, TextStyle::new(11.0).color(t.fg).nowrap());
                    });
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
                        crate::tokens::TokenKind::Length => format!("{:.0} px", tok.length),
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
    });
}

/// A 12 px colour sample with a hairline, so a colour near the panel's
/// own surface still reads as a sample.
fn swatch(ui: &mut Ui<'_>, c: Color, t: &Theme) {
    ui.with(
        NodeSpec::row()
            .width(Sizing::Fixed(12.0))
            .height(Sizing::Fixed(12.0))
            .radius(2.0)
            .bg(c)
            .border(1.0, t.border),
        |_| {},
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
/// and its colour says its state. Its fill is a faint wash of the strokes.
/// It and the three button shapes below declare the hand, as the stock
/// button does (`crate::cursor`): the dock's tabs and rows do not, since
/// a native tab strip and list are the arrow.
pub(super) fn icon(ui: &mut Ui<'_>, t: &Theme, what: &str, glyph: Icon, color: Color, hint: &str) {
    icon_with(ui, t, what, glyph, color, color.with_alpha(0.35), hint);
}

/// [`icon`] with the fill named apart from the strokes: the accent
/// swatch, whose disc is one colour and whose hairline is another.
fn icon_with(
    ui: &mut Ui<'_>,
    t: &Theme,
    what: &str,
    glyph: Icon,
    color: Color,
    fill: Color,
    hint: &str,
) {
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
            .cursor(crate::cursor::CursorShape::Pointer)
            .label(what)
            .apply_tooltip(hint),
        |ui| {
            icons::draw(ui, glyph, color, fill);
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
            .cursor(crate::cursor::CursorShape::Pointer)
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
            .cursor(crate::cursor::CursorShape::Pointer)
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
            .cursor(crate::cursor::CursorShape::Pointer)
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
