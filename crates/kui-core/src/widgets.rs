//! Opinionated helpers composed purely from primitives — the pattern custom
//! widgets should follow (composition over traits, state by key), which is
//! what keeps them reachable from scripting frontends.

use crate::access::Role;
use crate::color::Color;
use crate::cursor::CursorShape;
use crate::edit::EditOptions;
use crate::geom::{Edges, Vec2};
use crate::key::Key;
use crate::menu::{MenuBar, MenuItem, MenuRole};
use crate::metrics::Metrics;
use crate::spec::{Align, FloatConfig, NodeSpec, Sizing, TextStyle};
use crate::stats::{FrameSample, STATS_CAPACITY};
use crate::theme::Theme;
use crate::tree::OriginId;
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
        titlebar_height(ui)
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
    // Translucent over whatever the app is painting, so the panel takes
    // the theme's backmost surface and its strong border at the alphas
    // the HUD has always used.
    let t = ui.theme();
    ui.with(
        NodeSpec::column()
            .float(
                crate::spec::FloatConfig::viewport()
                    .at(x, y)
                    .self_at(x, y)
                    .offset(dx, dy),
            )
            .pad(10.0)
            .bg(t.bg.with_alpha(0.71))
            .radius(8.0)
            .border(1.0, t.border_strong.with_alpha(0.5)),
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

    let theme = ui.theme();
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
                ui.text(&label, TextStyle::new(10.0).color(theme.muted));
                // "?" badge: hover for the color legend. Also the dynamic-float
                // showcase — in the default bottom-right HUD the tooltip has no
                // room below or to the right, so it flips above and slides left.
                let badge = ui.child_key("kui:latency-legend");
                let badge_bg =
                    theme
                        .muted
                        .with_alpha(if ui.is_hovered(badge) { 0.31 } else { 0.16 });
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
                        ui.text("?", TextStyle::new(9.0).color(theme.fg));
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
                                                ui.text(name, TextStyle::new(11.0).color(theme.fg));
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
                    .bg(theme.sunken.with_alpha(0.6))
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
    let size = ui.metrics().hint_text;
    tooltip_with(ui, |ui| {
        ui.text(text, TextStyle::new(size));
    });
}

/// [`tooltip`] chrome around arbitrary content (legends, shortcut hints, …).
pub fn tooltip_with(ui: &mut Ui<'_>, content: impl FnOnce(&mut Ui<'_>)) {
    let spec = tooltip_spec(ui);
    ui.with(spec, content);
}

/// The hint the `tooltip` prop floats under a hovered node, and the stock
/// button under a hovered button: [`tooltip`]'s chrome and text, kept out
/// of the access tree (`Role::None`). The prop has already set the same
/// string as the node's description, which is where a reader hears it;
/// as content it would be read twice under a group and, under a control
/// named from its content, become part of the *name* whenever the pointer
/// crossed it (backlog F88). The `tooltip` element keeps its text, since
/// it is drawn with no description behind it.
pub(crate) fn hover_hint(ui: &mut Ui<'_>, text: &str) {
    let size = ui.metrics().hint_text;
    let spec = tooltip_spec(ui).role(crate::access::Role::None);
    ui.with(spec, |ui| {
        ui.text(text, TextStyle::new(size));
    });
}

fn tooltip_spec(ui: &Ui<'_>) -> NodeSpec {
    let t = ui.theme();
    let m = ui.metrics();
    NodeSpec::column()
        .float(crate::spec::FloatConfig::below().fit())
        .pad_xy(m.hint_pad_x, m.hint_pad_y)
        .bg(t.raised)
        .radius(m.radius)
        .border(1.0, t.border_strong)
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
    let t = ui.theme();
    let m = ui.metrics();
    let border = if ui.is_focused(key) {
        t.accent
    } else {
        t.border
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
            .pad_xy(m.field_pad_x, m.field_pad_y)
            .bg(t.sunken)
            .radius(m.radius)
            .border(1.0, border)
            .clip()
            .label(label),
    )
}

/// What a select's trigger posts when it is clicked; the core takes it
/// back and opens the menu (`Core::consume_select_events`).
pub(crate) fn select_tag() -> Value {
    Value::map([("select", Value::Bool(true))])
}

/// A choice among a few named options: a field that shows the one in
/// force and, clicked, drops a menu of them all with the current one
/// checked. `options` are the labels, `current` the index in force (or
/// none). Keyed by `label`, which is the accessible name too.
///
/// The menu is the core's own — the same one a right-click opens
/// (`Core::open_menu`): drawn in the frame, or the platform's where the
/// host shows menus itself, dismissed by Escape or a press outside, its
/// rows walked by the arrows. So the app holds no open state; what it
/// hears is the choice, as the `menu` event a menu row posts, on this
/// key: `{kind: "menu", role: "custom", item: <the option>}`. A view
/// that then draws the select with the new `current` is the whole loop.
///
/// [`select_items`] is the same field over [`MenuItem`]s, for an option
/// that posts an `id` of its own rather than its label.
pub fn select(ui: &mut Ui<'_>, label: &str, options: &[&str], current: Option<usize>) -> Key {
    let items: Vec<MenuItem> = options.iter().map(|o| MenuItem::new(*o)).collect();
    select_items(ui, label, &items, current)
}

/// [`select`] over items the caller built: their labels are the rows,
/// their `id`s what a choice posts, and the `current`th is drawn checked
/// whatever the item said. A separator is a separator here too.
pub fn select_items(
    ui: &mut Ui<'_>,
    label: &str,
    items: &[MenuItem],
    current: Option<usize>,
) -> Key {
    let t = ui.theme();
    let m = ui.metrics();
    select_with(
        ui,
        label,
        items,
        current,
        select_spec(&t, &m),
        TextStyle::new(m.chrome_text),
    )
}

/// The stock select field's spec: a sunken field with the stock radius
/// and padding, as [`button_spec`] is the stock button's. What
/// [`select_with`] is handed by [`select_items`]; a caller with a spec
/// of its own starts here and adds to it.
pub fn select_spec(theme: &Theme, m: &Metrics) -> NodeSpec {
    NodeSpec::row()
        .pad_xy(m.field_pad_x, m.field_pad_y)
        .gap(8.0)
        .cross_align(Align::Center)
        .bg(theme.sunken)
        .hover_bg(theme.hover)
        .radius(m.radius)
}

/// [`select_items`] with its spec and text style in the caller's hands —
/// a compact field in a dense panel — the way [`button_with`] takes the
/// button's. The border, the click, the role and the disclosure are
/// added here whatever `spec` said.
///
/// A `current` that names no option — past the end, or a separator — is
/// none, with a `select-current-ignored` warning on the field (backlog
/// RG10): the field is described by nothing and no row is checked, where
/// it used to check the divider.
pub fn select_with(
    ui: &mut Ui<'_>,
    label: &str,
    items: &[MenuItem],
    current: Option<usize>,
    spec: NodeSpec,
    text: TextStyle,
) -> Key {
    let key = ui.child_key(label);
    let current = current.filter(|&i| {
        let separator = items
            .get(i)
            .is_some_and(|it| it.role == MenuRole::Separator);
        let names_one = i < items.len() && !separator;
        if !names_one {
            ui.core().warn(crate::diag::select_current_ignored(
                key,
                label,
                i,
                items.len(),
                separator,
            ));
        }
        names_one
    });
    let t = ui.theme();
    let shown = current
        .and_then(|i| items.get(i))
        .map_or("", |i| i.text())
        .to_string();
    let open = ui.core().menu().is_some_and(|menu| menu.target == key);
    let border = if open || ui.is_focused(key) {
        t.accent
    } else {
        t.border
    };
    let menu: Vec<MenuItem> = items
        .iter()
        .enumerate()
        .map(|(i, item)| item.clone().checked(current == Some(i)))
        .collect();
    ui.core().declare_select(key, menu);
    ui.with_keyed(
        label,
        spec.border(1.0, border)
            .cursor(CursorShape::Pointer)
            .on_click(select_tag())
            // A button named by the field, described by the choice: what
            // a reader says of a pop-up button, in the two slots a button
            // has (`value` is a slider's and an editor's).
            .role(Role::Button)
            .label(label)
            .description(shown.as_str())
            .expanded(open),
        |ui| {
            ui.text(&shown, text.color(t.fg).nowrap());
            // The disclosure: a small triangle, the mark every platform's
            // pop-up field carries.
            ui.text("\u{25BE}", text.color(t.muted));
        },
    )
}

/// Default titlebar height, logical px, where the strip is the app's
/// alone. Follows platform conventions (as measured by gpui): 32 on
/// Windows (the native caption height), 34 elsewhere. The stock
/// [`Metrics`] carries the same number as `titlebar_h`, and the titlebar
/// draws from *that*, so an app that set its own metrics lays out against
/// `ui.metrics().titlebar_h` rather than this constant — and where the OS
/// keeps controls of its own over the strip, against [`titlebar_height`].
pub const TITLEBAR_H: f32 = Metrics::comfortable().titlebar_h;

/// The height the titlebar strip draws at — what an app laying out its
/// own strip, or something under it, should read instead of
/// `ui.metrics().titlebar_h`. Where the OS keeps controls of its own over
/// the strip (`env.window.native_controls`: the macOS traffic lights under
/// custom chrome) the strip is the OS's own titlebar, as tall as the
/// keep-out rect says that titlebar is, so the strip's content centres on
/// the buttons the OS centred in it; a strip 34 px tall beside a 32 px
/// titlebar put its content 2 px under the lights, and looked taller than
/// it was (backlog W17). Everywhere else the strip is the app's alone and
/// `Metrics::titlebar_h` is its height. A keep-out with no height (a host
/// that reported a width only) falls back to the metric.
pub fn titlebar_height(ui: &Ui<'_>) -> f32 {
    match ui.env().window.native_controls {
        Some(r) if r.h > 0.0 => r.h,
        _ => ui.metrics().titlebar_h,
    }
}

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
        // A background window's title recedes; the OS does the same.
        let t = ui.theme();
        let size = ui.metrics().chrome_text;
        let color = if focused { t.fg } else { t.faint };
        ui.with(
            NodeSpec::row()
                .width(Sizing::Grow(1.0))
                .height(Sizing::Grow(1.0))
                .cross_align(Align::Center),
            |ui| ui.text(&title, TextStyle::new(size).color(color).ellipsis()),
        );
    });
}

/// Titlebar with custom content (tabs, a search box, …) between the
/// platform inset and the window buttons. The whole strip is a drag
/// handle; interactive children declared inside it sit on top and win
/// hit-testing, so buttons in a titlebar just work.
pub fn titlebar_with(ui: &mut Ui<'_>, content: impl FnOnce(&mut Ui<'_>)) {
    let win = ui.env().window;
    let h = titlebar_height(ui);
    ui.with_keyed(
        "kui:titlebar",
        NodeSpec::row()
            .width(Sizing::Grow(1.0))
            .height(Sizing::Fixed(h))
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
    let t = ui.theme();
    let fg = t.fg;
    // Close is the one button that keeps a colour of its own on both
    // bases — it is the platform's signal, not the palette's — but it is
    // the theme's danger rather than a second red.
    let (bg, fg) = match button {
        WindowButton::Close if pressed => (t.danger.mix(Color::BLACK, 0.15), Color::WHITE),
        WindowButton::Close if hovered => (t.danger, Color::WHITE),
        _ if pressed => (t.pressed, fg),
        _ if hovered => (t.hover, fg),
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
/// The three backgrounds are the theme's accent trio — `accent`,
/// `accent_hover`, `accent_pressed` — so the stock button paints from the
/// palette like every other stock widget (backlog AR41): the OS's accent
/// where the host reports one, the app's where it set or pinned one, and
/// kui's blue otherwise, which is byte-for-byte the trio the button
/// always had (`Theme::dark()` carries the same three values). Takes the
/// theme and the metrics rather than reading them, as [`menu_panel_spec`]
/// takes the palette: `widgets::button_spec(&ui.theme(), &ui.metrics())`
/// is the idiom, and the stock numbers are `button_spec(&Theme::dark(),
/// &Metrics::default())`. The derivation for any *other* base colour is
/// [`button_palette`].
pub fn button_spec(theme: &Theme, m: &Metrics) -> NodeSpec {
    NodeSpec::row()
        .pad_xy(m.control_pad_x, m.control_pad_y)
        .bg(theme.accent)
        .hover_bg(theme.accent_hover)
        .pressed_bg(theme.accent_pressed)
        .radius(m.radius)
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

/// The stock button's text size — [`Metrics::default`]'s `control_text`;
/// the widget itself reads `ui.metrics()`.
pub const BUTTON_TEXT: f32 = Metrics::comfortable().control_text;
/// What a disabled stock button's opacity is multiplied by. The core makes
/// it inert and drops its hover and pressed backgrounds, and nothing else
/// would show a sighted user the state a reader is told.
pub const BUTTON_DISABLED_OPACITY: f32 = 0.5;

/// A push button showing `text`, keyed by it. A label that changes re-keys
/// the node — a new node, so it loses keyboard focus and a screen reader's
/// cursor; declare such a button with [`button_with`] and a key of its own.
/// The pointer over it is the hand (`CursorShape::Pointer`): the core
/// implies no shape from an `on_click`, and the stock button is the one
/// place the hand is declared for you.
pub fn button(ui: &mut Ui<'_>, text: &str, payload: impl Into<Value>) {
    let (theme, m) = (ui.theme(), ui.metrics());
    button_with(
        ui,
        text,
        text,
        button_spec(&theme, &m).on_click(payload.into()),
        None,
    );
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
    button_body(ui, Ident::Label(key), text, spec, hint);
}

/// [`button_with`] keyed by a data index rather than a label — a row of a
/// virtual list (`Ui::open_indexed`), so the button keeps its focus, its
/// hover and its tweens as the built range slides and the same text on
/// two rows is two nodes (backlog AR40). What `<button index>` and
/// `button { index = }` lower to.
pub fn button_indexed(ui: &mut Ui<'_>, index: u64, text: &str, spec: NodeSpec, hint: Option<&str>) {
    button_body(ui, Ident::Index(index), text, spec, hint);
}

/// How a button is keyed: by the label its `key` declares, or by the
/// data index its `index` declares.
enum Ident<'a> {
    Label(&'a str),
    Index(u64),
}

fn button_body(ui: &mut Ui<'_>, ident: Ident<'_>, text: &str, spec: NodeSpec, hint: Option<&str>) {
    let theme = ui.theme();
    // `accent` asks for the whole family, not just the background the
    // core would substitute for any node: a button whose hover and pressed
    // shades stayed put would flash under a yellow accent. On a stock
    // spec it changes nothing — `button_spec` paints from the theme's
    // trio already (AR41) — and on a spec whose caller set its own `bg`
    // it is the ask to take the theme's instead. The family is the
    // *theme's* (ADR 0019), and the theme always has one, so there is no
    // gate here: kui's blue is the accent nobody chose.
    let spec = if spec.accent {
        spec.bg(theme.accent)
            .hover_bg(theme.accent_hover)
            .pressed_bg(theme.accent_pressed)
    } else {
        spec
    };
    let spec = if spec.disabled {
        let o = spec.style.opacity * theme.disabled_opacity;
        spec.opacity(o)
    } else {
        spec
    };
    // The hand is declared, never derived from the `on_click`
    // (`crate::cursor`), and the stock button is where it is declared: a
    // caller's own `cursor` stands, and an inert button is the arrow — the
    // click it refuses is not one to point at.
    let spec = if spec.cursor.is_none() && !spec.disabled {
        spec.cursor(CursorShape::Pointer)
    } else {
        spec
    };
    // The hint floats out of the access tree (`hover_hint`), so it is
    // heard only as the description: a caller that passed one without
    // `apply_tooltip` on the spec still has it said. A declared
    // description stands, as it does over the prop.
    let spec = match hint {
        Some(hint) if spec.access().description.is_none() => spec.apply_tooltip(hint),
        _ => spec,
    };
    // Whatever the background ended up being: white on the stock blue as
    // it has always been, black on an accent light enough to need it.
    let label = readable_on(spec.style.bg);
    let size = ui.metrics().control_text;
    let node = match ident {
        Ident::Label(key) => ui.child_key(key),
        Ident::Index(i) => ui.child_key_index(i),
    };
    let body = |ui: &mut Ui<'_>| {
        ui.text(text, TextStyle::new(size).color(label));
        if let Some(hint) = hint
            && ui.is_hovered(node)
        {
            hover_hint(ui, hint);
        }
    };
    match ident {
        Ident::Label(key) => {
            ui.with_keyed(key, spec, body);
        }
        Ident::Index(i) => {
            ui.with_indexed(i, spec, body);
        }
    }
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
pub const MENU_WIDTH: f32 = Metrics::comfortable().menu_width;
pub const MENU_TEXT: f32 = Metrics::comfortable().chrome_text;
/// The reserved label the stock menu is keyed under. A menu the core
/// opened is found by key, not by guessing at payloads, so an app is free
/// to post whatever it likes from its own items.
pub const MENU_KEY: &str = "kui.menu";

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
pub fn context_menu(ui: &mut Ui<'_>, at: Vec2, items: &[MenuItem]) -> Key {
    let t = ui.theme();
    let m = ui.metrics();
    // The menu's nodes are the core's, not the host's: opened under their
    // own origin, so the core takes their events back by it.
    let saved = ui.origin();
    ui.set_origin(OriginId::MENU);
    // The menu floats against the window, not the host area (a menu the
    // platform showed would not stop at a dock's edge either, and the
    // devtools' own select opens one inside the dock): the host's point
    // becomes the window's.
    let at = at.plus(ui.core().dt_shift());
    let root = menu_panel(
        ui,
        MENU_KEY,
        menu_panel_spec(&t, &m)
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
            .label("Menu"),
        items,
    );
    ui.set_origin(saved);
    root
}

/// The panel every menu is: a fixed-width column of rows, in the palette
/// the stock menu paints. What the caller adds is where it goes and what
/// scope it belongs to — a context menu floats at the pointer and declares
/// its own `modal`; the menu bar's drops out of its title and lives inside
/// the bar's (`docs/adr/0018-a-menu-bar-the-app-declares.md`, decision 5).
/// Takes the palette and the metrics rather than reading them, because a
/// caller that has a `Ui` in one hand cannot lend it to this and to
/// `menu_panel` in the same expression — `let t = ui.theme();` first is
/// the idiom (ADR 0019).
pub fn menu_panel_spec(t: &Theme, m: &Metrics) -> NodeSpec {
    NodeSpec::column()
        .role(Role::Menu)
        .width(Sizing::Fixed(m.menu_width))
        .pad(4.0)
        .gap(1.0)
        .bg(t.raised)
        .border(1.0, t.border_strong)
        .radius(m.radius)
}

/// Builds the rows of one menu into `spec`, keyed under `label`, and
/// reports the keys they took. The one place a menu's rows are drawn:
/// both menus kui has are this function with a different container.
pub fn menu_panel(ui: &mut Ui<'_>, label: &str, spec: NodeSpec, items: &[MenuItem]) -> Key {
    let t = ui.theme();
    let m = ui.metrics();
    // A wash rather than a fill, so a row's label stays readable on both
    // bases without the view guessing a frame ahead of the core — see
    // `Theme::accent_soft`.
    let accent = t.accent_soft;
    // A gutter for the checkmarks, and only where a row has one: a menu of
    // plain commands is not indented for a column nothing uses, and one
    // with a setting in it keeps every label on the same left edge whether
    // the setting is on or off.
    let gutter = items.iter().any(|i| i.checked);
    ui.with_keyed(label, spec, |ui| {
        let mut first = true;
        for (i, item) in items.iter().enumerate() {
            if item.role == MenuRole::Separator {
                ui.with_indexed(
                    i as u64,
                    NodeSpec::row()
                        .width(Sizing::Grow(1.0))
                        .height(Sizing::Fixed(1.0))
                        .bg(t.border)
                        // Not a row anything reads out: a divider is
                        // paint, and a screen reader hearing "separator"
                        // between every pair of items is noise.
                        .role(Role::None),
                    |_| {},
                );
                continue;
            }
            // The row posts which item it is; the core takes the event back
            // by origin, performs the item, and what the app hears is the
            // item's own `id` on the node the menu was about.
            let payload = menu_row_tag(i);
            let mut spec = NodeSpec::row()
                .role(Role::MenuItem)
                .label(item.text())
                .width(Sizing::Grow(1.0))
                .pad_xy(m.menu_pad_x, m.menu_pad_y)
                .gap(8.0)
                .radius(m.radius_inner)
                .main_align(Align::Start)
                .cross_align(Align::Center);
            if item.checked {
                // The gutter's checkmark is paint; this is the same fact for
                // a screen reader, which reads a row that carries one as
                // checked rather than as "✓ Wrap".
                spec = spec.checked(true);
            }
            if item.enabled {
                spec = spec.on_click(payload).hover_bg(accent).focus_bg(accent);
                // The first row that can take focus is where the modal opens:
                // a menu whose keyboard starts nowhere makes the arrow keys
                // feel like they missed.
                if first {
                    spec = spec.initial_focus();
                    first = false;
                }
            } else {
                spec = spec.disabled(true).opacity(t.disabled_opacity);
            }
            ui.with_indexed(i as u64, spec, |ui| {
                if gutter {
                    ui.with(NodeSpec::row().width(Sizing::Fixed(MENU_CHECK_W)), |ui| {
                        if item.checked {
                            ui.text("\u{2713}", TextStyle::new(m.chrome_text).color(t.fg));
                        }
                    });
                }
                ui.text(item.text(), TextStyle::new(m.chrome_text).color(t.fg));
                if let Some(accel) = item.accel_text() {
                    // Pushed to the right edge by a grow spacer, so the label
                    // stays where the eye expects it whatever the
                    // accelerator is.
                    ui.with(NodeSpec::row().width(Sizing::Grow(1.0)), |_| {});
                    ui.text(accel, TextStyle::new(m.chrome_text).color(t.muted));
                }
            });
        }
    })
}

/// What a menu row's click carries: its index in the menu's items, for
/// the core to read back (`Core::menu_row_of`). The title of a menu-bar
/// menu carries its index the same way, under `title`.
fn menu_row_tag(i: usize) -> Value {
    Value::map([("row", Value::Int(i as i64))])
}

fn menu_title_tag(i: usize) -> Value {
    Value::map([("title", Value::Int(i as i64))])
}

/// The reserved label the drawn menu bar is keyed under, the way
/// [`MENU_KEY`] is the open menu's.
pub const MENU_BAR_KEY: &str = "kui.menubar";
/// The label its dropped menu is keyed under, beside the open title.
const MENU_BAR_PANEL_KEY: &str = "kui.menubar.menu";
/// The label each title is keyed under, inside its own wrapper.
const MENU_BAR_TITLE_KEY: &str = "kui.menubar.title";

/// The hover group a title and its menu share, so the widget can ask
/// whether the pointer is on the `i`th title without knowing its key.
fn group_name(i: usize) -> String {
    format!("{MENU_BAR_KEY}.{i}")
}
/// The bar's height, logical px — a little under a titlebar's, which is
/// what every platform that draws one in the window does.
pub const MENU_BAR_H: f32 = Metrics::comfortable().menu_bar_h;

/// The application menu: `bar` is what the app's menu *is*, and calling
/// this is where its titles go when they have to be drawn in the window
/// (`docs/adr/0018-a-menu-bar-the-app-declares.md`).
///
/// One call and not two, because the declaration and the placement are one
/// decision. **It draws nothing where the platform owns the bar** — macOS,
/// where the driver hands this same declaration to `NSApp` — so the call
/// still says what the menu is and the strip simply is not there; that is
/// the contract [`window_buttons`] has under native decorations, and it is
/// what makes one view portable. An empty `bar` takes the menu away.
///
/// Declared every frame, and diffed: an unchanged menu costs a comparison
/// and rebuilds nothing.
///
/// Everything below a title is the stock menu: the same rows, roles,
/// accelerators and access tree the context menu draws, through the same
/// [`menu_panel`]. What is the bar's own is the scope — while a menu is
/// open the *bar* is the frame's modal, not the dropdown, so hovering
/// across the titles moves the open menu the way a menu bar does, a press
/// on the open title closes it, and Escape or a press in the app below
/// closes it through ADR 0003's one mechanism.
///
/// Typical use, as the first child of a full-height root, under the
/// titlebar if there is one:
/// `widgets::menu_bar(ui, self.menu());`
pub fn menu_bar(ui: &mut Ui<'_>, bar: MenuBar) {
    // Declaring it is this call's first half, and drawing it the second:
    // where the platform owns the bar there is no second half, and the
    // frame has still said what the app's menu is.
    ui.core().declare_menu_bar(bar);
    if ui.core().native_menu_bar() {
        return;
    }
    let Some(bar) = ui.core().menu_bar().cloned() else {
        return;
    };
    if bar.menus.is_empty() {
        return;
    }
    let t = ui.theme();
    let m = ui.metrics();
    let accent = t.accent_soft;
    let mut open = ui.core().menu_bar_open();
    // The bar's nodes are the core's, opened under their own origin (see
    // `OriginId::MENU_BAR`), so the core takes their events back by it.
    let saved = ui.origin();
    ui.set_origin(OriginId::MENU_BAR);
    let mut spec = NodeSpec::row()
        .width(Sizing::Grow(1.0))
        .height(Sizing::Fixed(m.menu_bar_h))
        .cross_align(Align::Center)
        .pad_xy(4.0, 0.0)
        .gap(2.0)
        .bg(t.bg)
        .role(Role::Menu)
        .label("Menu bar");
    if open.is_some() {
        // The bar and not the dropdown is the modal while a menu is open:
        // the titles have to stay live for the hover to walk them, and the
        // app below has to be as inert as it is under any other menu.
        spec = spec.modal(Value::str(MENU_BAR_KEY));
    }
    let root = ui.with_keyed(MENU_BAR_KEY, spec, |ui| {
        // Hovering another title while a menu is open moves the open menu
        // to it, which is what a menu bar does everywhere. Resolved before
        // anything is built, so the frame that notices the hover is the
        // frame that draws the new menu and not the one after it — and
        // asked by *group* rather than by key, since a title's key is
        // inside a wrapper this loop has not opened yet.
        if open.is_some() {
            for (i, menu) in bar.menus.iter().enumerate() {
                let hovered = ui.is_group_hovered(NodeSpec::hover_group_id(&group_name(i)));
                if open != Some(i) && menu.enabled && !menu.items.is_empty() && hovered {
                    open = Some(i);
                    ui.core().set_menu_bar_open(open);
                }
            }
        }
        for (i, menu) in bar.menus.iter().enumerate() {
            let live = menu.enabled && !menu.items.is_empty();
            let is_open = open == Some(i);
            // A wrapper the menu drops out of, so the panel is a *sibling*
            // of the title and not a child of it: a `menuItem` is a
            // name-from-content role, and a menu nested inside one would be
            // read as part of its name and never reached on its own.
            ui.with_indexed(i as u64, NodeSpec::row(), |ui| {
                let mut spec = NodeSpec::row()
                    .role(Role::MenuItem)
                    .label(menu.label.as_str())
                    // Two px shorter than a row's, so the bar's height and
                    // not the title's padding decides the strip.
                    .pad_xy(m.menu_pad_x, (m.menu_pad_y - 2.0).max(0.0))
                    .radius(m.radius_inner)
                    .cross_align(Align::Center);
                if live {
                    // Which title this is: the core takes the event back
                    // by origin and opens or closes the `i`th menu.
                    spec = spec
                        .on_click(menu_title_tag(i))
                        .hover_group(&group_name(i))
                        .hover_bg(accent)
                        .focus_bg(accent);
                    if is_open {
                        spec = spec.bg(accent);
                    }
                } else {
                    spec = spec.disabled(true).opacity(t.disabled_opacity);
                }
                ui.with_keyed(MENU_BAR_TITLE_KEY, spec, |ui| {
                    ui.text(
                        menu.label.as_str(),
                        TextStyle::new(m.chrome_text).color(t.fg),
                    );
                });
                if is_open {
                    // Out of the title's bottom-left corner, and `fit` to
                    // slide back in at the right-hand end of the bar.
                    menu_panel(
                        ui,
                        MENU_BAR_PANEL_KEY,
                        menu_panel_spec(&t, &m).label(menu.label.as_str()).float(
                            FloatConfig::parent()
                                .at(Align::Start, Align::End)
                                .self_at(Align::Start, Align::Start)
                                .offset(0.0, 2.0)
                                .fit(),
                        ),
                        &menu.items,
                    );
                }
            });
        }
    });
    ui.set_origin(saved);
    ui.core().set_menu_bar_root(root);
}

/// The checkmark gutter's width, logical px.
const MENU_CHECK_W: f32 = 14.0;

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
/// container instead of using [`uniform_list`].
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
pub fn uniform_list(
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
        // The whole list's size, built or not: what Select All inside a
        // `selectable` list spans (ADR 0017, tier 3).
        ui.row_count(rows as u64);
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
// `uniform_list` takes one stride and every row must come out that tall,
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

/// The heights a [`list`] slices by: a measured number per row
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
    /// differently now — and returns true. [`list`] calls this from
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
/// only the visible ones — [`uniform_list`] where no single stride
/// describes the list.
///
/// `measure(ui, i, width)` returns row `i`'s height at that content width,
/// and is called only for rows the frame is about to build that `heights`
/// has no number for; `ui.measure_text(.., Some(width))` is layout's own
/// answer for a text row, wrap and all, and shapes through the same cache
/// the row's draw will hit. What it returns is the height the row *gets*:
/// each row's node is fixed to it, so the arithmetic above and below can
/// never disagree with the layout, the way `uniform_list`'s stride cannot.
/// A row that would rather size itself has to say what that size is here.
///
/// `row(ui, i)` declares row `i` inside that node, exactly as
/// `uniform_list`'s does, and rows are opened with [`Ui::open_indexed`] at
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
pub fn list(
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
    // Where an eased leg (F80) is going, when it is somewhere other than
    // where the content is drawn: a second anchor, so the row under the
    // target stays the target however the measurements below move the
    // rows between the two (RG18).
    let mut target_y = None;
    let (offset_y, vh, cw, first_frame) = match ui.scroll_geometry(key) {
        Some(g) => {
            let t = ui.scroll_offset(key).y.clamp(0.0, g.max_offset.y);
            if (t - g.offset.y).abs() > 0.5 {
                target_y = Some(t);
            }
            (g.offset.y, g.rect.h, g.rect.w - pad.x(), false)
        }
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
    // What the passes below move `top` away from. The correction is for a
    // *measurement* moving the numbers — not for the clamp above, which
    // on a list shorter than its box (offset 0, padding 6) makes
    // `top + pad.t` differ from the offset every frame, and a `set_scroll`
    // every frame is a frame requested every frame: the devtools' events
    // list never idled again once it had one row (found building ADR
    // 0029, ~130 frames/s after the first event).
    let top_before = top;
    let target_anchor = target_y.map(|t| {
        let t = (t - pad.t).max(0.0);
        let row = heights.row_at(t);
        (t, row, t - heights.offset_of(row))
    });

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

    // A write from inside a view lands on the frame being built: the
    // positions pass reads the store after the view has run. So the frame
    // that learned the rows are a different size is drawn already
    // corrected, and the uncorrected one is never seen. A shift, not a
    // `set_scroll`: the correction moves the coordinates under the
    // content, so it is never eased on a container with a `transition`,
    // and mid-glide it moves the leg with it rather than ending the leg
    // where the content stands (RG18).
    let drawn = top - top_before;
    let target = match target_anchor {
        Some((t, row, into)) => heights.offset_of(row) + into - t,
        None => drawn,
    };
    if drawn.abs() > 0.01 || target.abs() > 0.01 {
        ui.shift_scroll(key, Vec2::new(0.0, drawn), Vec2::new(0.0, target));
    }

    let lead = heights.offset_of(range.start);
    let tail = heights.total() - heights.offset_of(range.end);
    ui.with_keyed(label, spec.scroll_y().gap(0.0), |ui| {
        ui.row_count(heights.len() as u64);
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
