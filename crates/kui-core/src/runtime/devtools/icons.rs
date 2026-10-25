//! The panel's icons, drawn from the core's own vocabulary rather than
//! pulled from whatever font the platform falls back to: a
//! `line` per stroke, a `polygon` per fill, a
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
use crate::spec::NodeSpec;
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
    /// A crosshair: pick a node from the app.
    Pick,
    /// An eye: keep the selected node highlighted in the app. Its pupil
    /// is the part filled when the toggle is set.
    Eye,
    /// A square with a minus: fold every row.
    FoldAll,
    /// A square with a plus: unfold every row.
    UnfoldAll,
    /// A dot in a ring: start recording. Its dot is the fill.
    Record,
    /// A square: stop recording.
    Stop,
    /// A camera: screenshot the window.
    Camera,
    /// A film camera: record the window. Its body is the fill.
    Video,
    /// A disclosure, closed: a chevron pointing right.
    ChevronRight,
    /// … open: a chevron pointing down.
    ChevronDown,
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
    ui.with(NodeSpec::column().size(SIZE, SIZE), |ui| match icon {
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
        Icon::Pick => {
            ui.polyline(&circle(8.0, 8.0, 4.5), hair, spec());
            ui.line(v(8.0, 1.0), v(8.0, 5.0), stroke, spec());
            ui.line(v(8.0, 11.0), v(8.0, 15.0), stroke, spec());
            ui.line(v(1.0, 8.0), v(5.0, 8.0), stroke, spec());
            ui.line(v(11.0, 8.0), v(15.0, 8.0), stroke, spec());
        }
        Icon::Eye => {
            // The lids as two arcs meeting at the corners, the pupil a
            // disc inside them.
            let lid = |dy: f32| -> Vec<Vec2> {
                (0..=12)
                    .map(|i| {
                        let s = i as f32 / 12.0;
                        let x = 1.5 + 13.0 * s;
                        v(x, 8.0 + dy * 4.0 * s * (1.0 - s))
                    })
                    .collect()
            };
            ui.polyline(&lid(-5.5), stroke, spec());
            ui.polyline(&lid(5.5), stroke, spec());
            // The pupil: a dot (a zero-length segment) in the fill — a
            // polygon holds eight points, a circle more.
            if fill.is_visible() {
                ui.line(v(8.0, 8.0), v(8.0, 8.0), Stroke::new(4.8, fill), spec());
            }
            ui.polyline(&circle(8.0, 8.0, 2.4), hair, spec());
        }
        Icon::FoldAll | Icon::UnfoldAll => {
            // A folder's outline: a row behind, offset up and right, and
            // the square in front with the sign in it.
            ui.polyline(&[v(5.0, 2.5), v(13.5, 2.5), v(13.5, 11.0)], hair, spec());
            frame(ui, stroke, 2.5, 5.0, 11.0, 13.5);
            ui.line(v(4.75, 9.25), v(8.75, 9.25), stroke, spec());
            if icon == Icon::UnfoldAll {
                ui.line(v(6.75, 7.25), v(6.75, 11.25), stroke, spec());
            }
        }
        Icon::Record => {
            ui.polyline(&circle(8.0, 8.0, 6.0), stroke, spec());
            let dot = if fill.is_visible() { fill } else { color };
            ui.line(v(8.0, 8.0), v(8.0, 8.0), Stroke::new(6.0, dot), spec());
        }
        Icon::Stop => {
            ui.polygon(
                &[v(4.0, 4.0), v(12.0, 4.0), v(12.0, 12.0), v(4.0, 12.0)],
                spec().bg(if fill.is_visible() { fill } else { color }),
            );
        }
        Icon::Camera => {
            ui.polyline(
                &[
                    v(1.5, 5.0),
                    v(5.0, 5.0),
                    v(6.5, 3.0),
                    v(9.5, 3.0),
                    v(11.0, 5.0),
                    v(14.5, 5.0),
                    v(14.5, 13.5),
                    v(1.5, 13.5),
                    v(1.5, 5.0),
                ],
                stroke,
                spec(),
            );
            ui.polyline(&circle(8.0, 9.0, 2.6), stroke, spec());
        }
        Icon::Video => {
            ui.polygon(
                &[v(2.0, 4.5), v(10.5, 4.5), v(10.5, 11.5), v(2.0, 11.5)],
                spec().bg(fill),
            );
            frame(ui, stroke, 1.5, 4.5, 10.5, 11.5);
            ui.polyline(
                &[v(10.5, 7.0), v(14.5, 4.5), v(14.5, 11.5), v(10.5, 9.0)],
                stroke,
                spec(),
            );
        }
        Icon::ChevronRight => {
            ui.polyline(&[v(6.5, 4.5), v(10.0, 8.0), v(6.5, 11.5)], stroke, spec());
        }
        Icon::ChevronDown => {
            ui.polyline(&[v(4.5, 6.5), v(8.0, 10.0), v(11.5, 6.5)], stroke, spec());
        }
    });
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
