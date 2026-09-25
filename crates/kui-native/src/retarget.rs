//! Where a pointer in one window is in another — the arithmetic behind
//! ADR 0009 (`docs/adr/0009-press-drag-release-into-a-popup.md`,
//! decision 7), and the only part of it that runs without two OS windows.
//!
//! The OS gives a captured drag to the window of the mouse-down, so a
//! press on a combobox field and a release over its popup arrive at the
//! *owner*, in the owner's coordinates. The driver holds both windows'
//! screen positions, so the owner's point is the popup's point by way of
//! the screen. The common frame is the one the platform positions its
//! windows in: **physical** pixels on Windows and X11, where two windows
//! on two monitors can have two scales and logical coordinates do not
//! share an origin — and **points** on macOS, which has no physical
//! screen frame at all: winit's `inner_position` there is points times
//! *that window's* scale, so two windows on displays of different scale
//! would have origins in two frames (backlog AR33). [`Surface`] carries
//! whichever it is as `scale`, the common-frame units per logical px —
//! 1 on macOS — so the arithmetic is one.
//!
//! `Shell` calls it from three places: `retarget_move` while a press is
//! armed, `classify_release` when that press ends, and `Pane::surface`,
//! which is where a window becomes a [`Surface`]. Those are not headlessly
//! testable — they want two OS windows and a captured drag — and this is,
//! which is the whole reason the arithmetic is a module of its own.

use kui_core::{Rect, Size, Vec2};

/// One window as retargeting sees it: where its client area sits in the
/// common screen frame, how many of that frame's units a logical pixel
/// is, and its logical size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Surface {
    /// Top-left of the client area in the common frame: physical screen
    /// pixels (`Window::inner_position`) on Windows and X11, points on
    /// macOS (see the module doc).
    pub origin: (f64, f64),
    /// Common-frame units per logical pixel: `Window::scale_factor` where
    /// the frame is physical, 1 where it is points.
    pub scale: f64,
    /// Client area in logical pixels.
    pub size: Size,
}

impl Surface {
    /// A logical point in this window as a physical screen point.
    pub fn to_screen(self, p: Vec2) -> (f64, f64) {
        (
            self.origin.0 + p.x as f64 * self.scale,
            self.origin.1 + p.y as f64 * self.scale,
        )
    }

    /// A physical screen point in this window's logical coordinates —
    /// possibly outside it; see [`Surface::contains`].
    pub fn to_local(self, s: (f64, f64)) -> Vec2 {
        Vec2::new(
            ((s.0 - self.origin.0) / self.scale) as f32,
            ((s.1 - self.origin.1) / self.scale) as f32,
        )
    }

    /// Whether a logical point of this window lies inside its client
    /// area. Half-open like `Rect::contains`, so a point on the far edge
    /// belongs to whatever is beyond it.
    pub fn contains(self, p: Vec2) -> bool {
        Rect::from_pos_size(Vec2::ZERO, self.size).contains(p)
    }
}

/// The pointer at logical `p` in `from`, as `to` would report it — when
/// it lands inside `to`. `None` is "not over that window", which is what
/// tells the driver to send a `CursorLeft` rather than a `CursorMoved`.
pub(crate) fn retarget(from: &Surface, p: Vec2, to: &Surface) -> Option<Vec2> {
    let q = to.to_local(from.to_screen(p));
    to.contains(q).then_some(q)
}

/// Where a release of an armed press landed (ADR 0009 decision 4).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Landing {
    /// Over armed popup `index`, at this logical point in it: synthesise
    /// the press-and-release there.
    Popup { index: usize, at: Vec2 },
    /// On the anchor the popup was opened against: the menu stays, and
    /// the gesture degrades into click-to-choose.
    Anchor,
    /// Anywhere else: the popup is dismissed as `outside`.
    Outside,
}

/// Classifies a release at logical `p` in `owner`. `anchor` is in the
/// owner's logical coordinates (the rect its `onLayout` reported, as the
/// popup's config carried it). `popups` are the armed popups in opening
/// order; where two overlap the **last** one wins, since it opened on
/// top. A popup wins over the anchor, so a menu placed over its own field
/// is still chosen from.
pub(crate) fn landing(owner: &Surface, p: Vec2, anchor: Rect, popups: &[Surface]) -> Landing {
    let hit = popups.iter().enumerate().rev().find_map(|(index, popup)| {
        retarget(owner, p, popup).map(|at| Landing::Popup { index, at })
    });
    if let Some(hit) = hit {
        hit
    } else if anchor.contains(p) {
        Landing::Anchor
    } else {
        Landing::Outside
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The popup example's geometry on a 1x display: the owner's client
    /// area at screen (100, 100), the field 200 x 30 at (16, 40) inside
    /// it, and the popup placed below the field the way
    /// `Shell::popup_position` places it — at the anchor's x, under its
    /// bottom edge.
    const ANCHOR: Rect = Rect {
        x: 16.0,
        y: 40.0,
        w: 200.0,
        h: 30.0,
    };
    const OWNER: Surface = Surface {
        origin: (100.0, 100.0),
        scale: 1.0,
        size: Size { w: 360.0, h: 150.0 },
    };
    const POPUP: Surface = Surface {
        origin: (116.0, 170.0),
        scale: 1.0,
        size: Size { w: 200.0, h: 300.0 },
    };

    fn close(a: Vec2, b: Vec2) -> bool {
        (a.x - b.x).abs() < 1e-3 && (a.y - b.y).abs() < 1e-3
    }

    #[test]
    fn a_point_below_the_field_is_a_point_in_the_popup() {
        // 30 px below the field's bottom edge (y = 70) and 34 px in from
        // its left edge: the popup's (34, 30).
        let q = retarget(&OWNER, Vec2::new(50.0, 100.0), &POPUP).unwrap();
        assert!(close(q, Vec2::new(34.0, 30.0)), "{q:?}");
    }

    #[test]
    fn a_point_above_the_popup_is_not_in_it() {
        assert_eq!(retarget(&OWNER, Vec2::new(50.0, 20.0), &POPUP), None);
        // Its far edges are exclusive, like every rect in the core.
        assert_eq!(retarget(&OWNER, Vec2::new(216.0, 100.0), &POPUP), None);
        assert!(retarget(&OWNER, Vec2::new(215.9, 100.0), &POPUP).is_some());
    }

    #[test]
    fn the_measured_walk_off_the_top_lands_on_the_anchor() {
        // Backlog W2's measurement: pressed in the popup and dragged up
        // out of it, the popup's core saw 93,-1 then 95,-5 then 97,-10.
        // Mapped back through the screen, those are points on the field
        // the popup hangs under — which is what the pointer was over.
        for p in [
            Vec2::new(93.0, -1.0),
            Vec2::new(95.0, -5.0),
            Vec2::new(97.0, -10.0),
        ] {
            assert_eq!(
                retarget(&POPUP, p, &OWNER).map(|q| ANCHOR.contains(q)),
                Some(true),
                "{p:?}"
            );
        }
    }

    #[test]
    fn mixed_dpi_maps_through_physical_pixels() {
        // Owner on a 2x display at physical (0, 0); popup on a 1x display
        // to its right at physical (400, 600). Logical (250, 350) in the
        // owner is physical (500, 700), which is the popup's (100, 100).
        let owner = Surface {
            origin: (0.0, 0.0),
            scale: 2.0,
            size: Size::new(400.0, 400.0),
        };
        let popup = Surface {
            origin: (400.0, 600.0),
            scale: 1.0,
            size: Size::new(200.0, 300.0),
        };
        let q = retarget(&owner, Vec2::new(250.0, 350.0), &popup).unwrap();
        assert!(close(q, Vec2::new(100.0, 100.0)), "{q:?}");
        // And back: the popup's (100, 100) is the owner's (250, 350).
        let r = retarget(&popup, q, &owner).unwrap();
        assert!(close(r, Vec2::new(250.0, 350.0)), "{r:?}");
    }

    /// The macOS shape of the same desktop (backlog AR33): the platform
    /// positions windows in points, so both surfaces carry `scale` 1 and
    /// origins in points, and a Retina owner beside a 1x popup retargets
    /// by points alone — the display scales never enter the arithmetic,
    /// which is what makes `inner_position`'s per-window scaling harmless.
    #[test]
    fn on_macos_the_common_frame_is_points() {
        let owner = Surface {
            origin: (0.0, 0.0),
            scale: 1.0,
            size: Size::new(400.0, 400.0),
        };
        let popup = Surface {
            origin: (200.0, 300.0),
            scale: 1.0,
            size: Size::new(200.0, 300.0),
        };
        let q = retarget(&owner, Vec2::new(250.0, 350.0), &popup).unwrap();
        assert!(close(q, Vec2::new(50.0, 50.0)), "{q:?}");
        let r = retarget(&popup, q, &owner).unwrap();
        assert!(close(r, Vec2::new(250.0, 350.0)), "{r:?}");
    }

    #[test]
    fn a_release_lands_in_one_of_three_places() {
        let popups = [POPUP];
        assert_eq!(
            landing(&OWNER, Vec2::new(50.0, 100.0), ANCHOR, &popups),
            Landing::Popup {
                index: 0,
                at: Vec2::new(34.0, 30.0)
            }
        );
        assert_eq!(
            landing(&OWNER, Vec2::new(100.0, 55.0), ANCHOR, &popups),
            Landing::Anchor
        );
        assert_eq!(
            landing(&OWNER, Vec2::new(300.0, 20.0), ANCHOR, &popups),
            Landing::Outside
        );
        // No armed popup at all: only the anchor is not outside.
        assert_eq!(
            landing(&OWNER, Vec2::new(50.0, 100.0), ANCHOR, &[]),
            Landing::Outside
        );
    }

    #[test]
    fn the_last_opened_popup_wins_where_two_overlap() {
        // A submenu opened over the menu's right half: a release there
        // belongs to the submenu, one over the menu's left half to the
        // menu.
        let sub = Surface {
            origin: (216.0, 200.0),
            scale: 1.0,
            size: Size::new(200.0, 100.0),
        };
        let popups = [POPUP, sub];
        assert!(matches!(
            landing(&OWNER, Vec2::new(150.0, 150.0), ANCHOR, &popups),
            Landing::Popup { index: 1, .. }
        ));
        assert!(matches!(
            landing(&OWNER, Vec2::new(50.0, 150.0), ANCHOR, &popups),
            Landing::Popup { index: 0, .. }
        ));
    }

    #[test]
    fn a_popup_placed_over_its_field_still_wins_over_the_anchor() {
        // A popup opened under the pointer, macOS-style, covers the field
        // it belongs to: a release on the field is a release on the row
        // drawn over it, not "on the anchor".
        let over = Surface {
            origin: (116.0, 130.0),
            scale: 1.0,
            size: Size::new(200.0, 300.0),
        };
        assert!(matches!(
            landing(&OWNER, Vec2::new(100.0, 55.0), ANCHOR, &[over]),
            Landing::Popup { index: 0, .. }
        ));
    }
}
