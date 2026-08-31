//! Opinionated helpers composed purely from primitives — the pattern custom
//! widgets should follow (composition over traits, state by key), which is
//! what keeps them reachable from scripting frontends.

use crate::color::Color;
use crate::edit::EditOptions;
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
    let top_inset = if ui.env().window.custom_chrome { TITLEBAR_H } else { 0.0 };
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

    ui.with(NodeSpec::column().gap(3.0).cross_align(Align::End), |ui| {
        let mut label = format!("work {avg_work:.2}ms avg · {max_work:.2}ms max");
        if avg_wait > 0.05 {
            label.push_str(&format!(" · +{avg_wait:.2}ms vsync"));
        }
        ui.text(
            &label,
            TextStyle::new(10.0).color(Color::rgb8(0x8a, 0x8f, 0xa3)),
        );
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
    });
}

/// Small floating label hanging below the node it's declared inside.
/// Typical use: `if ui.is_hovered(key) { widgets::tooltip(ui, "..."); }`
pub fn tooltip(ui: &mut Ui<'_>, text: &str) {
    tooltip_at(ui, text, crate::spec::FloatConfig::below());
}

/// `tooltip` with an explicit attachment (e.g. edge-aligned so a hint near
/// the window border stays inside it).
pub fn tooltip_at(ui: &mut Ui<'_>, text: &str, float: crate::spec::FloatConfig) {
    ui.with(
        NodeSpec::column()
            .float(float)
            .pad_xy(10.0, 6.0)
            .bg(Color::rgb8(0x24, 0x27, 0x33))
            .radius(6.0)
            .border(1.0, Color::rgb8(0x3a, 0x3e, 0x4e)),
        |ui| {
            ui.text(text, TextStyle::new(12.0));
        },
    );
}

pub fn label(ui: &mut Ui<'_>, text: &str) {
    ui.text(text, TextStyle::default());
}

/// Single-line text input with chrome (background, focus ring).
/// Read the value with `ui.edit_text(key)`; "changed"/"submit" events arrive
/// in `on_event` with this key.
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
            .clip(),
    )
}

/// Default titlebar height, logical px. Follows platform conventions (as
/// measured by gpui): 32 on Windows (the native caption height), 34
/// elsewhere.
pub const TITLEBAR_H: f32 = if cfg!(target_os = "windows") { 32.0 } else { 34.0 };

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
            |ui| ui.text(&title, TextStyle::new(13.0).color(color)),
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
        |ui| {
            // No window manager to hint for us: name the button on hover.
            // The close hint hangs right-aligned so it stays inside the
            // window edge.
            if hovered && !pressed {
                let (hint, float) = match button {
                    WindowButton::Minimize => ("Minimize", crate::spec::FloatConfig::below()),
                    WindowButton::Maximize if maximized => {
                        ("Restore", crate::spec::FloatConfig::below())
                    }
                    WindowButton::Maximize => ("Maximize", crate::spec::FloatConfig::below()),
                    WindowButton::Close => (
                        "Close",
                        crate::spec::FloatConfig::below()
                            .at(Align::End, Align::End)
                            .self_at(Align::End, Align::Start),
                    ),
                };
                tooltip_at(ui, hint, float);
            }
            match button {
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
                                            crate::spec::FloatConfig::parent()
                                                .at(x, y)
                                                .self_at(x, y),
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
                    ui.text("\u{00d7}", TextStyle::new(16.0).line_height(16.0).color(fg));
                }
            }
        },
    );
}

pub fn button(ui: &mut Ui<'_>, text: &str, payload: impl Into<Value>) {
    let key = ui.child_key(text);
    let bg = if ui.is_pressed(key) {
        Color::rgb8(0x2f, 0x54, 0xc4)
    } else if ui.is_hovered(key) {
        Color::rgb8(0x47, 0x6c, 0xe0)
    } else {
        Color::rgb8(0x3b, 0x5b, 0xd4)
    };
    ui.with_keyed(
        text,
        NodeSpec::row()
            .pad_xy(14.0, 8.0)
            .bg(bg)
            .radius(6.0)
            .center()
            .on_click(payload.into()),
        |ui| ui.text(text, TextStyle::new(15.0).color(Color::WHITE)),
    );
}
