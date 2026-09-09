//! Opinionated helpers composed purely from primitives — the pattern custom
//! widgets should follow (composition over traits, state by key), which is
//! what keeps them reachable from scripting frontends.

use crate::access::Role;
use crate::color::Color;
use crate::edit::EditOptions;
use crate::geom::{Edges, Vec2};
use crate::key::Key;
use crate::menu::{MenuItem, MenuRole};
use crate::spec::{Align, FloatConfig, NodeSpec, Sizing, TextStyle};
use crate::stats::{FrameSample, STATS_CAPACITY};
use crate::ui::Ui;
use crate::value::Value;
use crate::window::WindowButton;

/// Floating latency HUD: `latency_graph` in a translucent panel pinned to a
/// viewport corner, above all content and out of layout flow. Call anywhere
/// in the view; pick the corner with `latency_hud_at`.
pub fn latency_hud(ui: &mut Ui<'_>) {
    latency_hud_at(ui, Align::End, Align::End);
}

pub fn latency_hud_at(ui: &mut Ui<'_>, x: Align, y: Align) {
    // Under custom chrome the top of the viewport is the app's titlebar;
    // keep the HUD below it.
    let top_inset = if ui.env().window.custom_chrome {
        TITLEBAR_H
    } else {
        0.0
    };
    let dx = match x {
        Align::Start => 12.0,
        Align::Center => 0.0,
        Align::End => -12.0,
    };
    let dy = match y {
        Align::Start => 12.0 + top_inset,
        Align::Center => 0.0,
        Align::End => -12.0,
    };
    ui.with(
        NodeSpec::column()
            .float(
                crate::spec::FloatConfig::viewport()
                    .at(x, y)
                    .self_at(x, y)
                    .offset(dx, dy),
            )
            .pad(10.0)
            .bg(Color::rgba8(0x10, 0x12, 0x1a, 0xb4))
            .radius(8.0)
            .border(1.0, Color::rgba8(0x3a, 0x3e, 0x4e, 0x80)),
        latency_graph,
    );
}

/// Frame-latency graph: the last ~120 frames as stacked per-phase bars
/// (input / view / layout / render, bottom to top) against the display's
/// frame budget (`env.refresh_hz`, 120 Hz fallback) — a bar that blows the
/// budget turns red. Feed `core.stats` (and `core.env`) from your frame
/// driver (the built-in runner does this automatically).
pub fn latency_graph(ui: &mut Ui<'_>) {
    const GRAPH_H: f32 = 34.0;
    const INPUT: Color = Color {
        r: 0.45,
        g: 0.85,
        b: 0.55,
        a: 1.0,
    };
    const VIEW: Color = Color {
        r: 0.28,
        g: 0.42,
        b: 0.88,
        a: 1.0,
    };
    const LAYOUT: Color = Color {
        r: 0.60,
        g: 0.42,
        b: 0.88,
        a: 1.0,
    };
    const RENDER: Color = Color {
        r: 0.94,
        g: 0.72,
        b: 0.35,
        a: 1.0,
    };
    const WAIT: Color = Color {
        r: 0.42,
        g: 0.45,
        b: 0.52,
        a: 0.7,
    };
    const OVER: Color = Color {
        r: 0.91,
        g: 0.36,
        b: 0.36,
        a: 1.0,
    };

    let budget_ms = ui.env().frame_budget_ms(); // full graph height
    let stats = &ui.core().stats;
    let samples: Vec<FrameSample> = stats.iter().collect();
    let (avg_work, max_work) = (stats.avg_work(), stats.max_work());
    let avg_wait = if samples.is_empty() {
        0.0
    } else {
        samples.iter().map(|s| s.wait_ms).sum::<f32>() / samples.len() as f32
    };

    // A development overlay, not app content: kept out of the access tree
    // so a screen reader does not read frame timings between the controls.
    ui.with(
        NodeSpec::column()
            .gap(3.0)
            .cross_align(Align::End)
            .role(crate::access::Role::None),
        |ui| {
            let mut label = format!("work {avg_work:.2}ms avg · {max_work:.2}ms max");
            if avg_wait > 0.05 {
                label.push_str(&format!(" · +{avg_wait:.2}ms vsync"));
            }
            ui.with(NodeSpec::row().gap(6.0).cross_align(Align::Center), |ui| {
                ui.text(
                    &label,
                    TextStyle::new(10.0).color(Color::rgb8(0x8a, 0x8f, 0xa3)),
                );
                // "?" badge: hover for the color legend. Also the dynamic-float
                // showcase — in the default bottom-right HUD the tooltip has no
                // room below or to the right, so it flips above and slides left.
                let badge = ui.child_key("kui:latency-legend");
                let badge_bg = if ui.is_hovered(badge) {
                    Color::rgba8(0x8a, 0x8f, 0xa3, 0x50)
                } else {
                    Color::rgba8(0x8a, 0x8f, 0xa3, 0x28)
                };
                ui.with_keyed(
                    "kui:latency-legend",
                    NodeSpec::column()
                        .width(Sizing::Fixed(13.0))
                        .height(Sizing::Fixed(13.0))
                        .center()
                        .bg(badge_bg)
                        .radius(6.5)
                        .hoverable(),
                    |ui| {
                        ui.text(
                            "?",
                            TextStyle::new(9.0).color(Color::rgb8(0xc9, 0xcc, 0xd6)),
                        );
                        if ui.is_hovered(badge) {
                            tooltip_with(ui, |ui| {
                                ui.with(NodeSpec::column().gap(5.0), |ui| {
                                    for (color, name) in [
                                        (INPUT, "input — events & edits"),
                                        (VIEW, "view — rebuilding the tree"),
                                        (LAYOUT, "layout — sizing & positions"),
                                        (RENDER, "render — encode + submit"),
                                        (WAIT, "vsync wait (not work)"),
                                        (OVER, "cap: work over frame budget"),
                                    ] {
                                        ui.with(
                                            NodeSpec::row().gap(7.0).cross_align(Align::Center),
                                            |ui| {
                                                ui.with(
                                                    NodeSpec::column()
                                                        .width(Sizing::Fixed(9.0))
                                                        .height(Sizing::Fixed(9.0))
                                                        .bg(color)
                                                        .radius(2.0),
                                                    |_| {},
                                                );
                                                ui.text(
                                                    name,
                                                    TextStyle::new(11.0)
                                                        .color(Color::rgb8(0xc9, 0xcc, 0xd6)),
                                                );
                                            },
                                        );
                                    }
                                });
                            });
                        }
                    },
                );
            });
            ui.with(
                NodeSpec::row()
                    .width(Sizing::Fixed(STATS_CAPACITY as f32 * 2.0))
                    .height(Sizing::Fixed(GRAPH_H))
                    .gap(1.0)
                    .main_align(Align::End)
                    .cross_align(Align::End)
                    .bg(Color::rgba8(0x0c, 0x0e, 0x14, 0x99))
                    .radius(3.0)
                    .clip(),
                |ui| {
                    let px_per_ms = GRAPH_H / budget_ms;
                    for s in &samples {
                        // Phases keep their colors even over budget — a spike
                        // you can't attribute is a spike you can't fix. Work
                        // (not vsync pacing) over budget gets a red cap.
                        let over = s.work() > budget_ms;
                        ui.with(
                            NodeSpec::column()
                                .width(Sizing::Fixed(1.0))
                                .main_align(Align::End)
                                .max_height(GRAPH_H),
                            |ui| {
                                if over {
                                    ui.with(
                                        NodeSpec::column()
                                            .width(Sizing::Fixed(1.0))
                                            .height(Sizing::Fixed(3.0))
                                            .bg(OVER),
                                        |_| {},
                                    );
                                }
                                // Column children run top->bottom; push in
                                // reverse so input sits at the bottom.
                                for (ms, color) in [
                                    (s.wait_ms, WAIT),
                                    (s.render_ms, RENDER),
                                    (s.layout_ms, LAYOUT),
                                    (s.view_ms, VIEW),
                                    (s.input_ms, INPUT),
                                ] {
                                    if ms <= 0.0 {
                                        continue;
                                    }
                                    let h = (ms * px_per_ms).max(1.0);
                                    ui.with(
                                        NodeSpec::column()
                                            .width(Sizing::Fixed(1.0))
                                            .height(Sizing::Fixed(h))
                                            .bg(color),
                                        |_| {},
                                    );
                                }
                            },
                        );
                    }
                },
            );
        },
    );
}

/// Small floating label hanging below the node it's declared inside. The
/// placement is dynamic (`FloatConfig::fit`): it flips above when the
/// viewport bottom is too close and slides sideways off window edges.
/// Typical use: `if ui.is_hovered(key) { widgets::tooltip(ui, "..."); }`
pub fn tooltip(ui: &mut Ui<'_>, text: &str) {
    tooltip_with(ui, |ui| {
        ui.text(text, TextStyle::new(12.0));
    });
}

/// [`tooltip`] chrome around arbitrary content (legends, shortcut hints, …).
pub fn tooltip_with(ui: &mut Ui<'_>, content: impl FnOnce(&mut Ui<'_>)) {
    ui.with(
        NodeSpec::column()
            .float(crate::spec::FloatConfig::below().fit())
            .pad_xy(10.0, 6.0)
            .bg(Color::rgb8(0x24, 0x27, 0x33))
            .radius(6.0)
            .border(1.0, Color::rgb8(0x3a, 0x3e, 0x4e)),
        content,
    );
}

pub fn label(ui: &mut Ui<'_>, text: &str) {
    ui.text(text, TextStyle::default());
}

/// Single-line text input with chrome (background, focus ring).
/// Read the value with `ui.edit_text(key)`; "changed"/"submit" events arrive
/// in `on_event` with this key. The `label` is the key and the accessible
/// name both (`"search"`, `"name"`), so a screen reader has something to
/// announce; use `ui.text_edit` with `NodeSpec::label` when they differ.
pub fn text_input(ui: &mut Ui<'_>, label: &str, initial: &str) -> Key {
    let key = ui.child_key(label);
    let border = if ui.is_focused(key) {
        Color::rgb8(0x3b, 0x5b, 0xd4)
    } else {
        Color::rgb8(0x2a, 0x2d, 0x3a)
    };
    ui.text_edit(
        label,
        initial,
        &EditOptions {
            multiline: false,
            ..Default::default()
        },
        NodeSpec::column()
            .width(Sizing::Grow(1.0))
            .pad_xy(10.0, 8.0)
            .bg(Color::rgb8(0x0e, 0x10, 0x16))
            .radius(6.0)
            .border(1.0, border)
            .clip()
            .label(label),
    )
}

/// Default titlebar height, logical px. Follows platform conventions (as
/// measured by gpui): 32 on Windows (the native caption height), 34
/// elsewhere.
pub const TITLEBAR_H: f32 = if cfg!(target_os = "windows") {
    32.0
} else {
    34.0
};

/// A cross-platform titlebar: a full-width drag strip with the window title
/// left-aligned next to the window controls. Reads `env.window` and adapts
/// by itself — under macOS custom chrome it insets past the native traffic
/// lights and draws no buttons; under custom chrome elsewhere it appends
/// minimize/maximize/close; under native decorations it is just a drag
/// strip (no duplicate buttons).
///
/// Typical use, as the first child of a full-height root:
/// `widgets::titlebar(ui, "my app")`.
pub fn titlebar(ui: &mut Ui<'_>, title: &str) {
    let focused = ui.env().focused;
    let title = title.to_string();
    titlebar_with(ui, move |ui| {
        let color = if focused {
            Color::rgb8(0xc9, 0xcc, 0xd6)
        } else {
            Color::rgb8(0x6e, 0x72, 0x80)
        };
        ui.with(
            NodeSpec::row()
                .width(Sizing::Grow(1.0))
                .height(Sizing::Grow(1.0))
                .cross_align(Align::Center),
            |ui| ui.text(&title, TextStyle::new(13.0).color(color).ellipsis()),
        );
    });
}

/// Titlebar with custom content (tabs, a search box, …) between the
/// platform inset and the window buttons. The whole strip is a drag
/// handle; interactive children declared inside it sit on top and win
/// hit-testing, so buttons in a titlebar just work.
pub fn titlebar_with(ui: &mut Ui<'_>, content: impl FnOnce(&mut Ui<'_>)) {
    let win = ui.env().window;
    ui.with_keyed(
        "kui:titlebar",
        NodeSpec::row()
            .width(Sizing::Grow(1.0))
            .height(Sizing::Fixed(TITLEBAR_H))
            .cross_align(Align::Center)
            .window_drag(),
        |ui| {
            // Keep clear of controls the OS draws over our content (the
            // reported rect already includes the trailing gap); without
            // them, a plain leading margin.
            let inset = win.native_controls.map_or(12.0, |r| r.x + r.w);
            ui.with(NodeSpec::row().width(Sizing::Fixed(inset)), |_| {});
            content(ui);
            window_buttons(ui);
        },
    );
}

/// The minimize/maximize/close cluster. Renders nothing when the OS already
/// provides controls (native decorations, or macOS traffic lights), so it
/// is always safe to call.
pub fn window_buttons(ui: &mut Ui<'_>) {
    let win = ui.env().window;
    if !win.custom_chrome || win.native_controls.is_some() {
        return;
    }
    ui.with(NodeSpec::row().height(Sizing::Grow(1.0)), |ui| {
        window_button(ui, WindowButton::Minimize, win.maximized);
        window_button(ui, WindowButton::Maximize, win.maximized);
        window_button(ui, WindowButton::Close, win.maximized);
    });
}

fn window_button(ui: &mut Ui<'_>, button: WindowButton, maximized: bool) {
    let label = match button {
        WindowButton::Minimize => "kui:win-min",
        WindowButton::Maximize => "kui:win-max",
        WindowButton::Close => "kui:win-close",
    };
    let key = ui.child_key(label);
    let (hovered, pressed) = (ui.is_hovered(key), ui.is_pressed(key));
    let fg = Color::rgb8(0xc9, 0xcc, 0xd6);
    let (bg, fg) = match button {
        WindowButton::Close if pressed => (Color::rgb8(0xc5, 0x0f, 0x1f), Color::WHITE),
        WindowButton::Close if hovered => (Color::rgb8(0xe8, 0x11, 0x23), Color::WHITE),
        _ if pressed => (Color::rgba(1.0, 1.0, 1.0, 0.06), fg),
        _ if hovered => (Color::rgba(1.0, 1.0, 1.0, 0.10), fg),
        _ => (Color::TRANSPARENT, fg),
    };
    ui.with_keyed(
        label,
        NodeSpec::row()
            .width(Sizing::Fixed(46.0))
            .height(Sizing::Grow(1.0))
            .center()
            .bg(bg)
            .window_button(button),
        |ui| match button {
            WindowButton::Minimize => {
                ui.with(
                    NodeSpec::row()
                        .width(Sizing::Fixed(10.0))
                        .height(Sizing::Fixed(1.0))
                        .bg(fg),
                    |_| {},
                );
            }
            WindowButton::Maximize if maximized => {
                // Restore: two offset outlines.
                ui.with(
                    NodeSpec::column()
                        .width(Sizing::Fixed(10.0))
                        .height(Sizing::Fixed(10.0)),
                    |ui| {
                        for (x, y) in [(Align::End, Align::Start), (Align::Start, Align::End)] {
                            ui.with(
                                NodeSpec::column()
                                    .width(Sizing::Fixed(7.5))
                                    .height(Sizing::Fixed(7.5))
                                    .border(1.0, fg)
                                    .float(
                                        crate::spec::FloatConfig::parent().at(x, y).self_at(x, y),
                                    ),
                                |_| {},
                            );
                        }
                    },
                );
            }
            WindowButton::Maximize => {
                ui.with(
                    NodeSpec::column()
                        .width(Sizing::Fixed(9.0))
                        .height(Sizing::Fixed(9.0))
                        .border(1.0, fg),
                    |_| {},
                );
            }
            WindowButton::Close => {
                // The multiplication sign inks only about 0.42 em, so it
                // needs a far larger em than the 9-10px bar and box beside
                // it to read as the same size. It also rides the math axis,
                // which sits a little under the middle of the line box, so
                // the bottom padding lifts it back onto the button center.
                const EM: f32 = 23.0;
                ui.with(
                    NodeSpec::row().padding(Edges {
                        b: EM * 0.25,
                        ..Edges::default()
                    }),
                    |ui| ui.text("\u{00d7}", TextStyle::new(EM).line_height(EM).color(fg)),
                );
            }
        },
    );
}

/// The standard button's spec: hover and pressed backgrounds are declared
/// on the node and resolved by the core, so every binding's button is this
/// same data. Add the label as a child.
///
/// The three colours are hand-picked rather than derived from the first, so
/// that the stock button paints exactly what it has always painted; the
/// derivation for any *other* base is [`button_palette`].
pub fn button_spec() -> NodeSpec {
    NodeSpec::row()
        .pad_xy(14.0, 8.0)
        .bg(Color::rgb8(0x3b, 0x5b, 0xd4))
        .hover_bg(Color::rgb8(0x47, 0x6c, 0xe0))
        .pressed_bg(Color::rgb8(0x2f, 0x54, 0xc4))
        .radius(6.0)
        .center()
}

/// A button's three backgrounds from one base colour: the base, a hover a
/// step toward white, a pressed a step toward black. The steps are the
/// distances the stock button's own trio sits at, so an accent-painted
/// button reads as the same control in a different colour.
///
/// Public because "a button in *this* colour" is the same question with a
/// different answer, and the arithmetic should not be re-guessed per app.
pub fn button_palette(base: Color) -> (Color, Color, Color) {
    (
        base,
        base.mix(Color::WHITE, 0.09),
        base.mix(Color::BLACK, 0.10),
    )
}

/// Black or white, whichever a reader can see on `bg`.
///
/// The split is at `Color::luminance` 0.4 rather than at the midpoint:
/// white text needs a darker background than black text needs a light one,
/// and the accents that land near the line (macOS's yellow at 0.72, its
/// orange at 0.44) come out the way the platform paints them. It is the
/// stock button's answer, not a general contrast checker — a palette that
/// cares should say what its label colour is.
pub fn readable_on(bg: Color) -> Color {
    if bg.luminance() > 0.4 {
        Color::BLACK
    } else {
        Color::WHITE
    }
}

pub const BUTTON_TEXT: f32 = 15.0;
/// What a disabled stock button's opacity is multiplied by. The core makes
/// it inert and drops its hover and pressed backgrounds, and nothing else
/// would show a sighted user the state a reader is told.
pub const BUTTON_DISABLED_OPACITY: f32 = 0.5;

/// A push button showing `text`, keyed by it. A label that changes re-keys
/// the node — a new node, so it loses keyboard focus and a screen reader's
/// cursor; declare such a button with [`button_with`] and a key of its own.
pub fn button(ui: &mut Ui<'_>, text: &str, payload: impl Into<Value>) {
    button_with(ui, text, text, button_spec().on_click(payload.into()), None);
}

/// [`button`] with its spec in the caller's hands: `spec` is [`button_spec`]
/// plus what the caller declared on it — the `on_click`, and the rows the
/// stock button admits in every binding (`schema::BUTTON_ROWS_JSX`): a
/// `label` when the text is not the name, a `description`, `disabled`,
/// and the hover tracking and description a `tooltip` sets, whose float
/// is `hint` — drawn under the button while it is hovered, as every
/// binding's `tooltip` prop floats one. Keyed by `key`, so a label that
/// changes need not re-key the node. A disabled button is dimmed
/// ([`BUTTON_DISABLED_OPACITY`]) as well as inert.
///
/// This is what `<button>`, `button { }` and `kui_button_with` lower to,
/// so a binding cannot end up with a button of its own.
pub fn button_with(ui: &mut Ui<'_>, key: &str, text: &str, spec: NodeSpec, hint: Option<&str>) {
    // `accent` asks for the whole palette, not just the background the
    // core would substitute for any node: a button whose hover and pressed
    // shades stayed the stock blue would flash blue under a yellow accent.
    // A host that cannot tell what the accent is leaves the stock trio,
    // which is what makes this safe to declare unconditionally.
    let spec = match ui.env().system.accent.filter(|_| spec.accent) {
        Some(accent) => {
            let (bg, hover, pressed) = button_palette(accent);
            spec.bg(bg).hover_bg(hover).pressed_bg(pressed)
        }
        None => spec,
    };
    let spec = if spec.disabled {
        let o = spec.style.opacity * BUTTON_DISABLED_OPACITY;
        spec.opacity(o)
    } else {
        spec
    };
    // Whatever the background ended up being: white on the stock blue as
    // it has always been, black on an accent light enough to need it.
    let label = readable_on(spec.style.bg);
    let node = ui.child_key(key);
    ui.with_keyed(key, spec, |ui| {
        ui.text(text, TextStyle::new(BUTTON_TEXT).color(label));
        if let Some(hint) = hint
            && ui.is_hovered(node)
        {
            tooltip(ui, hint);
        }
    });
}

// -- Context menus ----------------------------------------------------------
// The menu every app was writing for itself (ADR 0017, decision 5). It is
// exported rather than hidden inside the core's automatic path, and the
// automatic path calls exactly this — so an app that answers its own
// `onContextMenu` to add two items of its own gets the layout, the
// keyboard, the dismissal and the access rows without rewriting them, and
// the corpus tests one menu rather than two.

/// Menu chrome, in one place so a native renderer's absence still looks
/// deliberate rather than improvised.
pub const MENU_WIDTH: f32 = 200.0;
pub const MENU_TEXT: f32 = 13.0;
/// The reserved label the stock menu is keyed under. A menu the core
/// opened is found by key, not by guessing at payloads, so an app is free
/// to post whatever it likes from its own items.
pub const MENU_KEY: &str = "kui.menu";

/// The nodes [`context_menu`] built: the root a `modal` dismissal arrives
/// on, and one key per item in the order they were given, separators
/// included, so an index into the item list is an index into this.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuNodes {
    pub root: Key,
    pub rows: Vec<Key>,
}

/// Draws a context menu at `at` (logical viewport px) and returns the key
/// of its root. A float anchored to the viewport rather than to a parent,
/// because a context menu belongs at the pointer and not under whatever
/// node happens to enclose it; `fit` is what keeps it in the window, which
/// for a menu near the bottom edge means flipping above the point.
///
/// It declares `modal`, so a press outside it or Escape asks it to go away
/// through the one mechanism that already exists for that
/// (`docs/adr/0003-modal-surfaces.md`) rather than through a dismissal
/// rule of its own; the caller closes it when that dismissal arrives. The
/// rows are `menuItem`s under a `menu`, which is what makes the arrow keys
/// work (`docs/adr/0007-composite-keyboard-patterns.md`) and what a screen
/// reader reads.
///
/// Each chosen row posts the item's `id`, or its label when it declares
/// none. A `Separator` posts nothing and takes no focus.
pub fn context_menu(ui: &mut Ui<'_>, at: Vec2, items: &[MenuItem]) -> MenuNodes {
    let accent = ui.env().system.accent.unwrap_or(MENU_ACCENT);
    let mut rows = Vec::with_capacity(items.len());
    let root = ui.with_keyed(
        MENU_KEY,
        NodeSpec::column()
            .float(
                FloatConfig::viewport()
                    // Top-left of the menu at the top-left of the
                    // viewport, then offset to the point: the placement
                    // every context menu has, with `fit` flipping it up
                    // or clamping it in when the point is near an edge.
                    .at(Align::Start, Align::Start)
                    .self_at(Align::Start, Align::Start)
                    .offset(at.x, at.y)
                    .fit(),
            )
            .modal(Value::str(MENU_KEY))
            .role(Role::Menu)
            .label("Menu")
            .width(Sizing::Fixed(MENU_WIDTH))
            .pad(4.0)
            .gap(1.0)
            .bg(MENU_BG)
            .border(1.0, MENU_BORDER)
            .radius(6.0),
        |ui| {
            let mut first = true;
            for (i, item) in items.iter().enumerate() {
                if item.role == MenuRole::Separator {
                    rows.push(
                        ui.with_indexed(
                            i as u64,
                            NodeSpec::row()
                                .width(Sizing::Grow(1.0))
                                .height(Sizing::Fixed(1.0))
                                .bg(MENU_BORDER)
                                // Not a row anything reads out: a divider is
                                // paint, and a screen reader hearing "separator"
                                // between every pair of items is noise.
                                .role(Role::None),
                            |_| {},
                        ),
                    );
                    continue;
                }
                let payload = item.id.clone().unwrap_or_else(|| Value::str(item.text()));
                let mut spec = NodeSpec::row()
                    .role(Role::MenuItem)
                    .label(item.text())
                    .width(Sizing::Grow(1.0))
                    .pad_xy(8.0, 5.0)
                    .gap(8.0)
                    .radius(4.0)
                    .main_align(Align::Start)
                    .cross_align(Align::Center);
                if item.enabled {
                    spec = spec.on_click(payload).hover_bg(accent).focus_bg(accent);
                    // The first row that can take focus is where the
                    // modal opens: a menu whose keyboard starts nowhere
                    // makes the arrow keys feel like they missed.
                    if first {
                        spec = spec.initial_focus();
                        first = false;
                    }
                } else {
                    spec = spec.disabled(true).opacity(MENU_DISABLED_OPACITY);
                }
                rows.push(ui.with_indexed(i as u64, spec, |ui| {
                    ui.text(item.text(), TextStyle::new(MENU_TEXT).color(MENU_FG));
                    if let Some(accel) = item.accel_text() {
                        // Pushed to the right edge by a grow spacer, so
                        // the label stays where the eye expects it
                        // whatever the accelerator is.
                        ui.with(NodeSpec::row().width(Sizing::Grow(1.0)), |_| {});
                        ui.text(accel, TextStyle::new(MENU_TEXT).color(MENU_ACCEL));
                    }
                }));
            }
        },
    );
    MenuNodes { root, rows }
}

const MENU_BG: Color = Color {
    r: 0.114,
    g: 0.125,
    b: 0.169,
    a: 1.0,
};
const MENU_BORDER: Color = Color {
    r: 0.231,
    g: 0.247,
    b: 0.294,
    a: 1.0,
};
const MENU_FG: Color = Color {
    r: 0.839,
    g: 0.847,
    b: 0.878,
    a: 1.0,
};
const MENU_ACCEL: Color = Color {
    r: 0.541,
    g: 0.561,
    b: 0.639,
    a: 1.0,
};
const MENU_ACCENT: Color = Color {
    r: 0.231,
    g: 0.357,
    b: 0.831,
    a: 1.0,
};
/// A disabled row is dimmed the way a disabled button is.
const MENU_DISABLED_OPACITY: f32 = BUTTON_DISABLED_OPACITY;

// -- Virtual lists ----------------------------------------------------------
// The core culls glyphs by viewport but builds every child a view declares,
// so a ten-thousand-row log costs ten thousand rows of build and layout on
// every frame — most of a 120 Hz budget spent on rows nobody can see. A view
// that knows the container's height and offset can declare a screenful and
// two spacers instead. `Core::scroll_geometry` is that knowledge; this is
// the arithmetic, for the case where every row is the same height.

/// The half-open range of rows a container of `rows` rows, each `row_h`
/// logical px tall, has any reason to build — those crossing the visible
/// band, plus `overscan` on each side — given the geometry of the frame
/// before. Pure arithmetic, exposed for views that build their own
/// container instead of using [`virtual_column`].
///
/// `vh` is the container's height, and `pad_t` the padding above the first
/// row. `None` geometry means no layout has resolved the container yet:
/// the caller decides what the first frame builds.
pub fn visible_rows(
    offset_y: f32,
    vh: f32,
    pad_t: f32,
    row_h: f32,
    rows: usize,
    overscan: usize,
) -> std::ops::Range<usize> {
    if rows == 0 || row_h <= 0.0 {
        return 0..0;
    }
    // Flow coordinates: row i spans [i*row_h, (i+1)*row_h), and layout puts
    // the flow's origin at pad_t - offset_y inside the container's box, so
    // the visible window is [offset_y - pad_t, that + vh).
    let top = offset_y - pad_t;
    let first = (top / row_h).floor().max(0.0) as usize;
    let last = ((top + vh.max(0.0)) / row_h).ceil().max(0.0) as usize;
    let first = first.saturating_sub(overscan).min(rows);
    let last = last.saturating_add(overscan).min(rows);
    first..last.max(first)
}

/// A vertically scrolling column of `rows` uniform rows that builds only the
/// visible ones. `row(ui, i)` declares row `i`; it must come out exactly
/// `row_h` logical px tall, since that is the arithmetic placing every row
/// above and below it.
///
/// The container is `spec` forced to a scrolling column with no gap — put
/// the spacing inside `row_h` (a row that pads itself) rather than in a
/// `gap`, so one number describes the stride. Rows are opened with
/// [`Ui::open_indexed`] at their *data* index, so a row keeps its key, and
/// with it its hover, focus, edit buffer and tweens, as the built range
/// slides over it. Above and below sit two empty spacers holding the space
/// of the rows not built, so the content height, the scrollbar and
/// `set_scroll` all behave as if the whole list were there.
///
/// The geometry it slices by is the previous frame's, so the first frame —
/// before any layout has resolved the container — slices by the viewport
/// height instead and asks for one more frame; a resize is one frame late
/// and covered by the two rows of overscan. Returns the container's key,
/// for `set_scroll` (`Vec2::new(0.0, i as f32 * row_h)` scrolls row `i` to
/// the top, which is how you reach a row that is not built — `reveal` of an
/// unbuilt row finds nothing).
pub fn virtual_column(
    ui: &mut Ui<'_>,
    label: &str,
    spec: NodeSpec,
    rows: usize,
    row_h: f32,
    mut row: impl FnMut(&mut Ui<'_>, usize),
) -> Key {
    const OVERSCAN: usize = 2;

    let key = ui.child_key(label);
    let pad_t = spec.layout.padding.t;
    // Both numbers from the same frame: the geometry's offset is clamped to
    // that frame's travel, so a `set_scroll(key, huge)` between frames
    // slices the end of the list instead of a megabyte past it.
    let (offset_y, vh, first_frame) = match ui.scroll_geometry(key) {
        Some(g) => (g.offset.y, g.rect.h, false),
        // Nothing laid out yet: the container cannot be taller than the
        // window in the ordinary case, so a screenful is a safe over-build
        // for one frame.
        None => (ui.scroll_offset(key).y, ui.viewport().h, true),
    };
    let range = visible_rows(offset_y, vh, pad_t, row_h, rows, OVERSCAN);

    ui.with_keyed(label, spec.scroll_y().gap(0.0), |ui| {
        // Keyed, not auto-keyed: an auto key is a sibling index, and the
        // rows already occupy that namespace at their data indices — an
        // auto-keyed spacer next to a built row 0 would be row 0's key.
        let lead = range.start as f32 * row_h;
        if lead > 0.0 {
            ui.with_keyed("lead", spacer_spec(lead), |_| {});
        }
        for i in range.clone() {
            ui.with_indexed(i as u64, row_spec(row_h), |ui| row(ui, i));
        }
        let tail = (rows - range.end) as f32 * row_h;
        if tail > 0.0 {
            ui.with_keyed("tail", spacer_spec(tail), |_| {});
        }
    });

    if first_frame {
        ui.request_frame();
    }
    key
}

/// Each row's own node: the wrapper the callback builds inside, sized to the
/// stride the arithmetic assumes. A clickable row puts `on_click` on a
/// `.fill()` child of it.
fn row_spec(h: f32) -> NodeSpec {
    NodeSpec::column()
        .width(Sizing::Grow(1.0))
        .height(Sizing::Fixed(h))
}

fn spacer_spec(h: f32) -> NodeSpec {
    NodeSpec::column()
        .width(Sizing::Grow(1.0))
        .height(Sizing::Fixed(h))
}

// -- Variable-height virtual lists ------------------------------------------
// `virtual_column` takes one stride and every row must come out that tall,
// which is the log viewer, the data table and the chat history whose rows are
// one line. A row that wraps, a card with an image, a message that is
// sometimes three lines: none of those have a stride, and the three things
// the uniform arithmetic does with `i * row_h` — the lead spacer, the search
// from an offset to the first visible row, and "scroll to row i" — have no
// closed form without one. Prefix sums are the closed form, and
// `RowHeights` is where they live.
//
// Heights come from the caller, measured only for the rows the frame needs:
// `measure_text` gives layout's own number for a text row (wrap, max_lines
// and the shaping cache included), so a row measured and then drawn shapes
// once. Everything not measured yet stands at an estimate, and the estimate
// is the mean of what has been measured — which means it *moves*, and moving
// it changes the height of every row above the window as well as below.
// That is what the anchor is for.

/// The heights a [`virtual_rows`] list slices by: a measured number per row
/// where one is known, an estimate everywhere else, and the prefix sums over
/// both.
///
/// The app owns it and hands the same one back every frame — a widget
/// composed from primitives keeps no state of its own, which is what keeps
/// it reachable from a scripting frontend. Rebuild it (or [`Self::clear`])
/// when the rows themselves change.
#[derive(Clone, Debug)]
pub struct RowHeights {
    /// One per row; `f32::NAN` for a row nothing has measured yet.
    h: Vec<f32>,
    /// The prefix sums, split so that the estimate is applied at the query
    /// rather than baked in: `m[i]` is the measured height in rows `0..i`
    /// and `u[i]` how many of those rows have none. A moving mean then costs
    /// nothing to fold in — which matters, because every measurement moves
    /// it, and a mean baked into the sums would dirty all of them.
    m: Vec<f32>,
    u: Vec<u32>,
    /// How many entries of `m` / `u` are valid, counting from 0. Filled
    /// on demand and only as far as a query asks, so a list scrolled to row
    /// 30 never sums the 9,970 below it; a measurement at row `i` truncates
    /// this to `i + 1`, since nothing at or below `i` changed.
    clean: usize,
    /// What the caller guessed before anything was measured.
    seed: f32,
    /// Running mean of the measured rows — the estimate for the rest.
    sum: f32,
    n: usize,
    /// The content width the cached heights were measured at. A different
    /// one rewraps every row, so it drops them all.
    width: f32,
    /// Where the last search landed. Scrolling is local, so the next one
    /// gallops out from here instead of bisecting the whole list — which is
    /// what keeps the lazy `ensure` above from being filled past what is
    /// being looked at, and what a bisection from 0..len would defeat by
    /// probing the middle every time.
    last: usize,
}

impl RowHeights {
    /// `rows` rows, none measured, each standing at `estimate` logical px
    /// until it is. The estimate only has to be the right order of
    /// magnitude: it decides how wrong the scrollbar is before the list has
    /// been scrolled through, and nothing else.
    pub fn new(rows: usize, estimate: f32) -> Self {
        RowHeights {
            h: vec![f32::NAN; rows],
            m: vec![0.0],
            u: vec![0],
            clean: 1,
            seed: estimate.max(1.0),
            sum: 0.0,
            n: 0,
            width: f32::NAN,
            last: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.h.len()
    }

    pub fn is_empty(&self) -> bool {
        self.h.is_empty()
    }

    /// Grows or shrinks to `rows`, keeping what is still in range — rows
    /// appended to a log keep every height already measured, and cost
    /// nothing until something asks about them. A list whose rows *changed*
    /// rather than grew wants [`Self::clear`].
    pub fn set_len(&mut self, rows: usize) {
        if rows == self.h.len() {
            return;
        }
        for i in rows..self.h.len() {
            self.forget(i);
        }
        self.h.resize(rows, f32::NAN);
        self.clean = self.clean.min(rows + 1);
    }

    /// Forgets every measurement, keeping the length and the seed — the call
    /// for a list whose contents changed under the same indices.
    pub fn clear(&mut self) {
        self.h.fill(f32::NAN);
        self.sum = 0.0;
        self.n = 0;
        self.clean = 1;
    }

    /// Records row `i`'s height. Rows measured this way are what the
    /// estimate for the others is the mean of.
    pub fn set(&mut self, i: usize, h: f32) {
        if i >= self.h.len() || !h.is_finite() || h < 0.0 {
            return;
        }
        self.forget(i);
        self.h[i] = h;
        self.sum += h;
        self.n += 1;
        // Everything up to and including row `i`'s own top is unchanged.
        self.clean = self.clean.min(i + 1);
    }

    fn forget(&mut self, i: usize) {
        let old = self.h[i];
        if !old.is_nan() {
            self.sum -= old;
            self.n -= 1;
            self.h[i] = f32::NAN;
            self.clean = self.clean.min(i + 1);
        }
    }

    /// Row `i`'s height as it was measured, or `None` for one standing at
    /// the estimate.
    pub fn measured(&self, i: usize) -> Option<f32> {
        self.h.get(i).copied().filter(|h| !h.is_nan())
    }

    /// Row `i`'s height: measured, or the estimate.
    pub fn get(&self, i: usize) -> f32 {
        self.measured(i).unwrap_or_else(|| self.estimate())
    }

    /// What an unmeasured row stands at: the mean of the measured ones, or
    /// the caller's seed before there are any.
    pub fn estimate(&self) -> f32 {
        if self.n == 0 {
            self.seed
        } else {
            self.sum / self.n as f32
        }
    }

    /// The width the measurements were taken at, or `NaN` before any.
    pub fn width(&self) -> f32 {
        self.width
    }

    /// Declares the content width the next measurements are for. A width
    /// that differs from the cached one drops every height — the rows wrap
    /// differently now — and returns true. [`virtual_rows`] calls this from
    /// the container's own laid-out box.
    pub fn set_width(&mut self, w: f32) -> bool {
        if !w.is_finite() || w <= 0.0 || (self.width - w).abs() < 0.5 {
            return false;
        }
        let had = self.n > 0;
        self.width = w;
        if had {
            self.clear();
        }
        true
    }

    /// Fills the prefix sums up to `i` if they do not reach it yet.
    fn ensure(&mut self, i: usize) {
        let want = i.min(self.h.len()) + 1;
        if self.clean >= want {
            return;
        }
        self.m.truncate(self.clean);
        self.u.truncate(self.clean);
        self.m.reserve(want - self.clean);
        self.u.reserve(want - self.clean);
        let (mut acc, mut est) = (self.m[self.clean - 1], self.u[self.clean - 1]);
        for &h in &self.h[self.clean - 1..want - 1] {
            if h.is_nan() {
                est += 1;
            } else {
                acc += h;
            }
            self.m.push(acc);
            self.u.push(est);
        }
        self.clean = want;
    }

    /// The top of row `i` in content coordinates — the height of everything
    /// above it. `offset_of(len())` is the whole list's height.
    pub fn offset_of(&mut self, i: usize) -> f32 {
        let i = i.min(self.h.len());
        self.ensure(i);
        self.m[i] + self.u[i] as f32 * self.estimate()
    }

    /// The list's total height, measured and estimated together — what the
    /// two spacers and the scrollbar are made of. Kept as it goes, so the
    /// tail spacer costs nothing however long the list is.
    pub fn total(&self) -> f32 {
        self.sum + (self.h.len() - self.n) as f32 * self.estimate()
    }

    /// The row `y` (content coordinates) lands in: the last row whose top is
    /// at or above it, clamped to the list. The binary search that replaces
    /// `y / row_h`.
    pub fn row_at(&mut self, y: f32) -> usize {
        let rows = self.h.len();
        if rows == 0 || y <= 0.0 {
            self.last = 0;
            return 0;
        }
        // `offset_of` is non-decreasing, so what is wanted is the last row
        // whose top is at or below `y`. Row 0's top is 0, so it always
        // qualifies and the bracket below always closes.
        let mut lo = self.last.min(rows - 1);
        let mut hi;
        if self.offset_of(lo) > y {
            hi = lo;
            let mut step = 1usize;
            while lo > 0 {
                lo = lo.saturating_sub(step);
                if self.offset_of(lo) <= y {
                    break;
                }
                hi = lo;
                step *= 2;
            }
        } else {
            hi = (lo + 1).min(rows);
            let mut step = 1usize;
            while hi < rows && self.offset_of(hi) <= y {
                lo = hi;
                hi = (hi + step).min(rows);
                step *= 2;
            }
        }
        while lo + 1 < hi {
            let mid = lo + (hi - lo) / 2;
            if self.offset_of(mid) <= y {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        self.last = lo;
        lo
    }
}

/// A vertically scrolling column of rows of *different* heights that builds
/// only the visible ones — [`virtual_column`] where no single stride
/// describes the list.
///
/// `measure(ui, i, width)` returns row `i`'s height at that content width,
/// and is called only for rows the frame is about to build that `heights`
/// has no number for; `ui.measure_text(.., Some(width))` is layout's own
/// answer for a text row, wrap and all, and shapes through the same cache
/// the row's draw will hit. What it returns is the height the row *gets*:
/// each row's node is fixed to it, so the arithmetic above and below can
/// never disagree with the layout, the way `virtual_column`'s stride cannot.
/// A row that would rather size itself has to say what that size is here.
///
/// `row(ui, i)` declares row `i` inside that node, exactly as
/// `virtual_column`'s does, and rows are opened with [`Ui::open_indexed`] at
/// their data index, so a row keeps its hover, focus, edit buffer and tweens
/// as the built range slides over it.
///
/// **What it does that the uniform one never has to:** every row not yet
/// measured stands at the mean of the ones that are, so measuring the rows
/// this frame builds changes the height of every row it does not — the ones
/// above the window included. Left alone that slides the content out from
/// under the pointer on the frame it learns anything. So the widget takes
/// the row the window starts in and how far into it, measures, and then puts
/// that pair back: `Core::set_scroll` from inside a view lands on the frame
/// being built (the positions pass reads the store after the view has run),
/// so the corrected frame is the only one ever seen. What does move is the
/// scrollbar, which is the honest thing to move — the list really did just
/// learn it is a different length.
///
/// Returns the container's key, for `set_scroll` — and "scroll to row `i`"
/// is `set_scroll(key, Vec2::new(0.0, heights.offset_of(i)))`, exact for a
/// measured row and converging over a frame or two for one that is not.
pub fn virtual_rows(
    ui: &mut Ui<'_>,
    label: &str,
    spec: NodeSpec,
    heights: &mut RowHeights,
    mut measure: impl FnMut(&mut Ui<'_>, usize, f32) -> f32,
    mut row: impl FnMut(&mut Ui<'_>, usize),
) -> Key {
    const OVERSCAN: usize = 2;
    // Measuring changes the heights the range was sliced from, which can
    // widen it; four passes is far more than a screenful ever needs and
    // bounds the work whatever the measurements do.
    const PASSES: usize = 4;

    let key = ui.child_key(label);
    let pad = spec.layout.padding;
    let (offset_y, vh, cw, first_frame) = match ui.scroll_geometry(key) {
        Some(g) => (g.offset.y, g.rect.h, g.rect.w - pad.x(), false),
        // Nothing laid out yet: a screenful of the viewport is a safe
        // over-build for one frame, and the width is its width.
        None => (
            ui.scroll_offset(key).y,
            ui.viewport().h,
            ui.viewport().w - pad.x(),
            true,
        ),
    };
    // A resize rewraps every row, so the cache is void; the frame after it
    // measures a screenful again.
    heights.set_width(cw);

    // The row the window starts in and how far into it — the pair the
    // correction below puts back where it was.
    let mut top = (offset_y - pad.t).max(0.0);
    let anchor = heights.row_at(top);
    let into = top - heights.offset_of(anchor);

    let mut range = visible_range(heights, top, vh, OVERSCAN);
    for _ in 0..PASSES {
        let mut measured = false;
        for i in range.clone() {
            if heights.measured(i).is_none() {
                let h = measure(ui, i, cw);
                heights.set(i, h);
                measured = true;
            }
        }
        if !measured {
            break;
        }
        // Measuring moved the numbers the slice was taken from — this row's
        // own, the rows above it, and (through the mean) every row nobody
        // has measured at all. Put the anchor row back where it was before
        // re-slicing, so what is under the pointer does not slide out from
        // under it.
        top = heights.offset_of(anchor) + into;
        let next = visible_range(heights, top, vh, OVERSCAN);
        if next == range {
            break;
        }
        range = next;
    }

    // `Core::set_scroll` from inside a view lands on the frame being built:
    // the positions pass reads the store after the view has run. So the
    // frame that learned the rows are a different size is drawn already
    // corrected, and the uncorrected one is never seen.
    let corrected = top + pad.t;
    if (corrected - offset_y).abs() > 0.01 {
        ui.set_scroll(key, Vec2::new(0.0, corrected));
    }

    let lead = heights.offset_of(range.start);
    let tail = heights.total() - heights.offset_of(range.end);
    ui.with_keyed(label, spec.scroll_y().gap(0.0), |ui| {
        if lead > 0.0 {
            ui.with_keyed("lead", spacer_spec(lead), |_| {});
        }
        for i in range.clone() {
            ui.with_indexed(i as u64, row_spec(heights.get(i)), |ui| row(ui, i));
        }
        if tail > 0.0 {
            ui.with_keyed("tail", spacer_spec(tail), |_| {});
        }
    });

    if first_frame {
        ui.request_frame();
    }
    key
}

/// The rows crossing `[top, top + vh)` plus `overscan` on each side, by
/// prefix-sum search. The variable-height [`visible_rows`].
fn visible_range(
    heights: &mut RowHeights,
    top: f32,
    vh: f32,
    overscan: usize,
) -> std::ops::Range<usize> {
    let rows = heights.len();
    if rows == 0 {
        return 0..0;
    }
    let first = heights.row_at(top).saturating_sub(overscan);
    let last = (heights.row_at(top + vh.max(0.0)) + 1 + overscan).min(rows);
    first..last.max(first)
}
