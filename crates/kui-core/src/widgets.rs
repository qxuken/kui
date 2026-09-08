//! Opinionated helpers composed purely from primitives — the pattern custom
//! widgets should follow (composition over traits, state by key), which is
//! what keeps them reachable from scripting frontends.

use crate::color::Color;
use crate::edit::EditOptions;
use crate::geom::Edges;
use crate::key::Key;
use crate::spec::{Align, NodeSpec, Sizing, TextStyle};
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
