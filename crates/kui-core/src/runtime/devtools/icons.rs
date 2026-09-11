//! The panel's icons, drawn from the core's own vocabulary rather than
//! pulled from whatever font the platform falls back to (backlog D2): a
//! `line` per stroke (ADR 0010), a `polygon` per fill (ADR 0025), a
//! zero-length segment for a dot — all in a 16-px box the button centres,
//! all in the theme's colours, so they read as one set on every platform
//! and follow the appearance like everything else the panel draws.
//!
//! Nothing here is keyed: an icon is redrawn from its button's state every
//! frame, and the button is what carries the identity, the tooltip and
//! the access row.

use crate::color::Color;
use crate::geom::Vec2;
use crate::line::Stroke;
use crate::spec::{NodeSpec, Sizing};
use crate::ui::Ui;

/// The glyphs the panel has. Each is one picture at one size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Icon {
    /// A window with its left pane shaded.
    DockLeft,
    /// … its right pane.
    DockRight,
    /// … its bottom pane.
    DockBottom,
    /// Two windows, the front one shaded: the panel in a window of its own.
    DockWindow,
    /// A cross.
    Close,
    /// A disc split in two: the base follows the app's own setting.
    BaseAuto,
    /// A sun.
    BaseLight,
    /// A crescent.
    BaseDark,
    /// A filled disc, in the accent itself.
    Accent,
    /// Three bars.
    Menus,
    /// A crosshair: pick a node from the app.
    Pick,
}

/// The box an icon is drawn in, logical px.
pub(super) const SIZE: f32 = 16.0;
/// Every stroke's width.
const W: f32 = 1.5;

/// Draws `icon` in a 16×16 box: strokes in `color`, and the part that is
/// "the shaded pane" — the current placement, the set toggle — in `fill`,
/// which a caller passes as the accent when the button is lit and as a
/// faint wash of `color` when it is not.
pub(super) fn draw(ui: &mut Ui<'_>, icon: Icon, color: Color, fill: Color) {
    let stroke = Stroke::new(W, color);
    let hair = Stroke::new(1.0, color);
    let v = Vec2::new;
    let spec = || NodeSpec::column();
    ui.with(
        NodeSpec::column()
            .width(Sizing::Fixed(SIZE))
            .height(Sizing::Fixed(SIZE)),
        |ui| match icon {
            Icon::DockLeft | Icon::DockRight | Icon::DockBottom => {
                // The window frame, then the pane as a fill inside it.
                frame(ui, stroke, 1.5, 1.5, 14.5, 14.5);
                let (x0, y0, x1, y1) = match icon {
                    Icon::DockLeft => (2.5, 2.5, 6.5, 13.5),
                    Icon::DockRight => (9.5, 2.5, 13.5, 13.5),
                    _ => (2.5, 9.5, 13.5, 13.5),
                };
                ui.polygon(
                    &[v(x0, y0), v(x1, y0), v(x1, y1), v(x0, y1)],
                    spec().bg(fill),
                );
            }
            Icon::DockWindow => {
                // The back window, then the front one shaded over it.
                frame(ui, stroke, 1.5, 1.5, 10.5, 10.5);
                ui.polygon(
                    &[v(6.0, 6.0), v(14.5, 6.0), v(14.5, 14.5), v(6.0, 14.5)],
                    spec().bg(fill),
                );
                frame(ui, stroke, 6.0, 6.0, 14.5, 14.5);
            }
            Icon::Close => {
                ui.line(v(4.0, 4.0), v(12.0, 12.0), stroke, spec());
                ui.line(v(12.0, 4.0), v(4.0, 12.0), stroke, spec());
            }
            Icon::BaseAuto => {
                // A ring, its left half filled: eight points down the left
                // arc from the top to the bottom, closed up the middle.
                ui.polyline(&circle(8.0, 8.0, 5.5), stroke, spec());
                let mut half = [Vec2::ZERO; 8];
                for (i, p) in half.iter_mut().enumerate() {
                    let a = std::f32::consts::PI * (0.5 + i as f32 / 7.0);
                    *p = v(8.0 + 5.5 * a.cos(), 8.0 - 5.5 * a.sin());
                }
                ui.polygon(&half, spec().bg(color));
            }
            Icon::BaseLight => {
                // A disc and eight rays.
                ui.line(v(8.0, 8.0), v(8.0, 8.0), Stroke::new(6.0, color), spec());
                for i in 0..8 {
                    let a = std::f32::consts::TAU * i as f32 / 8.0;
                    let (c, s) = (a.cos(), a.sin());
                    ui.line(
                        v(8.0 + 5.0 * c, 8.0 + 5.0 * s),
                        v(8.0 + 7.0 * c, 8.0 + 7.0 * s),
                        hair,
                        spec(),
                    );
                }
            }
            Icon::BaseDark => {
                // A crescent as one eight-point fill: five points down the
                // outer arc, three back up the inner one, the horns where
                // the two meet.
                ui.polygon(
                    &[
                        v(11.0, 2.8),
                        v(5.0, 2.8),
                        v(2.0, 8.0),
                        v(5.0, 13.2),
                        v(11.0, 13.2),
                        v(8.0, 12.3),
                        v(5.5, 8.0),
                        v(8.0, 3.7),
                    ],
                    spec().bg(color),
                );
            }
            Icon::Accent => {
                ui.line(v(8.0, 8.0), v(8.0, 8.0), Stroke::new(9.0, color), spec());
            }
            Icon::Menus => {
                for y in [4.0, 8.0, 12.0] {
                    ui.line(v(2.5, y), v(13.5, y), stroke, spec());
                }
            }
            Icon::Pick => {
                ui.polyline(&circle(8.0, 8.0, 4.5), hair, spec());
                ui.line(v(8.0, 1.0), v(8.0, 5.0), stroke, spec());
                ui.line(v(8.0, 11.0), v(8.0, 15.0), stroke, spec());
                ui.line(v(1.0, 8.0), v(5.0, 8.0), stroke, spec());
                ui.line(v(11.0, 8.0), v(15.0, 8.0), stroke, spec());
            }
        },
    );
}

/// A closed rectangle outline.
fn frame(ui: &mut Ui<'_>, stroke: Stroke, x0: f32, y0: f32, x1: f32, y1: f32) {
    let v = Vec2::new;
    ui.polyline(
        &[v(x0, y0), v(x1, y0), v(x1, y1), v(x0, y1), v(x0, y0)],
        stroke,
        NodeSpec::column(),
    );
}

/// A circle as a closed 16-gon, which at these radii is a circle.
fn circle(cx: f32, cy: f32, r: f32) -> Vec<Vec2> {
    (0..=16)
        .map(|i| {
            let a = std::f32::consts::TAU * i as f32 / 16.0;
            Vec2::new(cx + r * a.cos(), cy + r * a.sin())
        })
        .collect()
}
