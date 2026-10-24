//! How a held drag scrolls a container at its edge (ADR 0029, amended
//! 2026-10-25): one feel for kui's own drags — a caret drag, a
//! drag-select — and for an app's own, through `Core::edge_scroll`.
//!
//! The speed is set by three things. How deep the pointer is in an edge
//! zone inside the container, along an ease-in curve: slow at the zone's
//! inner edge, for fine control, fast at the container's edge. How long
//! it has been held there: the speed multiplies up a ramp, so a pointer
//! held still gets fast. And a cap. A pointer pinned against the
//! window's edge counts as fully deep, which is what a full-screen or
//! maximised window needs — there the pointer stops at the monitor's
//! edge and can go no deeper; where the window is smaller the pointer
//! goes on past the container's edge, and the distance past still adds
//! speed, within the cap.

use crate::geom::{Rect, Vec2};

/// How close to the window's edge, in logical px, a pointer counts as
/// pinned against it: a pointer at the monitor's edge reads a pixel or
/// so short of the window's last row.
pub const PINNED_PX: f32 = 2.0;

/// The most of a container's extent on an axis each of its two zones
/// takes: in a short container the zones shrink so that its middle
/// still holds a pointer still.
const ZONE_SHARE: f32 = 0.25;

/// How a held drag scrolls a container at its edge (ADR 0029, amended
/// 2026-10-25). The app's input feel rather than a view's, set with
/// `Core::set_edge_scroll_feel`; the native runner copies the main
/// window's to every other, and `KUI_EDGE_SCROLL` in the environment
/// overrides it there for comparing without a rebuild.
///
/// [`EdgeScrollFeel::STANDARD`] is the default. The presets, from slow
/// to fast, at a pointer held still at full depth:
///
/// | preset     | zone  | at once  | after 0.5 s | top speed       |
/// |------------|-------|----------|-------------|-----------------|
/// | `gentle`   | 72 px | 360 px/s | 720 px/s    | 1800 px/s, 2 s  |
/// | `standard` | 60 px | 500 px/s | 1330 px/s   | 3000 px/s, 1.5 s|
/// | `brisk`    | 48 px | 700 px/s | 2800 px/s   | 4900 px/s, 1 s  |
///
/// Half way into the zone each is a quarter of its speed at once (the
/// quadratic curve), so a selection can be walked a line at a time
/// near the zone's inner edge.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EdgeScrollFeel {
    /// How deep the edge zone inside a container is, in logical px: the
    /// speed rises from nothing at its inner edge to `speed_px_s` at the
    /// container's edge. A short container's zones take at most a
    /// quarter of it each.
    pub zone_px: f32,
    /// The speed at full depth before the ramp, in logical px a second:
    /// at the container's edge, at the window's edge when the pointer is
    /// pinned there, or anywhere past either.
    pub speed_px_s: f32,
    /// The curve's exponent: 1 is linear, 2 (the default) quadratic,
    /// slower near the inner edge. Past the container's edge the speed
    /// goes on rising along the curve's tangent at full depth.
    pub curve: f32,
    /// How long the pointer must stay in the zone, in seconds, for the
    /// speed to reach `ramp_max` times itself. 0 is at once.
    pub ramp_secs: f32,
    /// What the ramp multiplies the speed by at its top: 1 is no ramp.
    pub ramp_max: f32,
    /// The most a container scrolls, in logical px a second, after the
    /// depth and the ramp.
    pub cap_px_s: f32,
    /// How far the pointer may move back toward the content before the
    /// ramp starts again, in logical px: a hand easing off slows the
    /// scroll at once.
    pub pull_back_px: f32,
}

impl EdgeScrollFeel {
    /// Softer than the default: a deeper zone, slower at the edge and
    /// slower up the ramp.
    pub const GENTLE: Self = Self {
        zone_px: 72.0,
        speed_px_s: 360.0,
        curve: 2.0,
        ramp_secs: 2.0,
        ramp_max: 5.0,
        cap_px_s: 1800.0,
        pull_back_px: 8.0,
    };
    /// The default: a zone of about two lines, 500 px/s at its edge, six
    /// times that a second and a half later — three screens a second.
    pub const STANDARD: Self = Self {
        zone_px: 60.0,
        speed_px_s: 500.0,
        curve: 2.0,
        ramp_secs: 1.5,
        ramp_max: 6.0,
        cap_px_s: 3000.0,
        pull_back_px: 8.0,
    };
    /// Faster: a shallower zone, quicker at the edge and up the ramp.
    pub const BRISK: Self = Self {
        zone_px: 48.0,
        speed_px_s: 700.0,
        curve: 2.0,
        ramp_secs: 1.0,
        ramp_max: 7.0,
        cap_px_s: 4900.0,
        pull_back_px: 8.0,
    };

    /// The presets by name, slow to fast — what [`Self::preset`] reads,
    /// and the names the bindings and `KUI_EDGE_SCROLL` take.
    pub const PRESETS: [(&'static str, Self); 3] = [
        ("gentle", Self::GENTLE),
        ("standard", Self::STANDARD),
        ("brisk", Self::BRISK),
    ];

    /// The preset called `name`, ignoring case; `None` for any other.
    pub fn preset(name: &str) -> Option<Self> {
        let name = name.trim();
        Self::PRESETS
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|&(_, f)| f)
    }

    /// The feel with each number brought into what the step can use: a
    /// zone from 4 to 400 px, a speed from 1 to 20000 px/s, a curve from
    /// 0.5 to 4, a ramp from 0 to 10 s up to 1 to 20 times, a cap from 1
    /// to 50000 px/s and a pull-back from 0 to 200 px. A number that is
    /// not finite takes the standard one.
    pub fn clamped(self) -> Self {
        let pick = |v: f32, lo: f32, hi: f32, or: f32| {
            if v.is_finite() { v.clamp(lo, hi) } else { or }
        };
        let s = Self::STANDARD;
        Self {
            zone_px: pick(self.zone_px, 4.0, 400.0, s.zone_px),
            speed_px_s: pick(self.speed_px_s, 1.0, 20000.0, s.speed_px_s),
            curve: pick(self.curve, 0.5, 4.0, s.curve),
            ramp_secs: pick(self.ramp_secs, 0.0, 10.0, s.ramp_secs),
            ramp_max: pick(self.ramp_max, 1.0, 20.0, s.ramp_max),
            cap_px_s: pick(self.cap_px_s, 1.0, 50000.0, s.cap_px_s),
            pull_back_px: pick(self.pull_back_px, 0.0, 200.0, s.pull_back_px),
        }
    }

    /// The speed at `depth` px into a zone `zone` px deep, before the
    /// ramp: along the curve inside it, along its tangent past it.
    fn speed_at(&self, depth: f32, zone: f32) -> f32 {
        if depth <= 0.0 {
            0.0
        } else if depth <= zone {
            self.speed_px_s * (depth / zone).powf(self.curve)
        } else {
            self.speed_px_s * (1.0 + self.curve * (depth - zone) / zone)
        }
    }

    /// What the ramp multiplies the speed by after `held` seconds.
    fn ramp(&self, held: f32) -> f32 {
        let t = if self.ramp_secs > 0.0 {
            (held / self.ramp_secs).min(1.0)
        } else {
            1.0
        };
        1.0 + (self.ramp_max - 1.0) * t
    }
}

impl Default for EdgeScrollFeel {
    fn default() -> Self {
        Self::STANDARD
    }
}

/// One container's edge scroll under one held drag: how long the
/// pointer has been in its zone, and the deepest it has been since the
/// ramp last started.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct EdgeRamp {
    held: f32,
    deepest: f32,
}

impl EdgeRamp {
    /// The scroll for a pointer at `p` over a frame `dt` seconds long,
    /// against a container's `rect` in a window `window`, on the axes it
    /// scrolls: logical px, positive toward later content (the sign
    /// `ScrollStore::scroll_by` takes). Out of the zones it is zero and
    /// the ramp starts again; in them the ramp advances by `dt`.
    pub(crate) fn step(
        &mut self,
        feel: &EdgeScrollFeel,
        rect: Rect,
        window: Rect,
        p: Vec2,
        axes: (bool, bool),
        dt: f32,
    ) -> Vec2 {
        let still = (0.0, 1.0, 0.0);
        let x = if axes.0 {
            axis(feel, rect.x, rect.w, p.x, window.x, window.w)
        } else {
            still
        };
        let y = if axes.1 {
            axis(feel, rect.y, rect.h, p.y, window.y, window.h)
        } else {
            still
        };
        let deep = x.0.max(y.0);
        if deep <= 0.0 {
            *self = Self::default();
            return Vec2::ZERO;
        }
        // Eased off by more than the threshold: slower at once, the
        // curve's doing, and the ramp from the start again.
        if deep < self.deepest - feel.pull_back_px {
            *self = Self::default();
        }
        self.deepest = self.deepest.max(deep);
        let ramp = feel.ramp(self.held);
        self.held += dt.max(0.0);
        let v = |(d, zone, way): (f32, f32, f32)| {
            way * (feel.speed_at(d, zone) * ramp).min(feel.cap_px_s) * dt
        };
        Vec2::new(v(x), v(y))
    }
}

/// How deep `v` is in the zones of a container spanning `len` from `lo`
/// on one axis, in a window spanning `win_len` from `win_lo`: the depth,
/// the zone's own depth, and the way it scrolls — -1 toward earlier
/// content, +1 toward later, 0 out of both zones. Pinned against the
/// window's edge counts as the whole zone, however far the container
/// runs on past it.
fn axis(
    feel: &EdgeScrollFeel,
    lo: f32,
    len: f32,
    v: f32,
    win_lo: f32,
    win_len: f32,
) -> (f32, f32, f32) {
    let hi = lo + len;
    let zone = feel.zone_px.min(len.max(0.0) * ZONE_SHARE).max(1.0);
    let pinned_lo = v <= win_lo + PINNED_PX;
    let pinned_hi = v >= win_lo + win_len - PINNED_PX;
    if v < lo + zone || pinned_lo {
        let d = lo + zone - v;
        let d = if pinned_lo { d.max(zone) } else { d };
        (d.max(0.0), zone, -1.0)
    } else if v > hi - zone || pinned_hi {
        let d = v - (hi - zone);
        let d = if pinned_hi { d.max(zone) } else { d };
        (d.max(0.0), zone, 1.0)
    } else {
        (0.0, zone, 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const S: EdgeScrollFeel = EdgeScrollFeel::STANDARD;
    const FRAME: f32 = 1.0 / 60.0;

    fn window() -> Rect {
        Rect::new(0.0, 0.0, 800.0, 600.0)
    }

    fn list() -> Rect {
        Rect::new(100.0, 100.0, 400.0, 300.0)
    }

    #[test]
    fn the_middle_steps_nothing_and_the_zone_eases_in() {
        let mut r = EdgeRamp::default();
        let y = |r: &mut EdgeRamp, y: f32| {
            r.step(
                &S,
                list(),
                window(),
                Vec2::new(200.0, y),
                (false, true),
                1.0,
            )
            .y
        };
        assert_eq!(y(&mut r, 250.0), 0.0);
        // The zone's inner edge: nothing; half way: a quarter; the
        // container's edge: all of it.
        assert_eq!(y(&mut EdgeRamp::default(), 340.0), 0.0);
        assert!((y(&mut EdgeRamp::default(), 370.0) - S.speed_px_s / 4.0).abs() < 1e-2);
        assert!((y(&mut EdgeRamp::default(), 400.0) - S.speed_px_s).abs() < 1e-2);
        // Toward earlier content at the top.
        assert!((y(&mut EdgeRamp::default(), 100.0) + S.speed_px_s).abs() < 1e-2);
        // Past the edge the tangent: 30 px past is half the zone more.
        assert!((y(&mut EdgeRamp::default(), 430.0) - S.speed_px_s * 2.0).abs() < 1e-2);
    }

    #[test]
    fn pinned_at_the_windows_edge_is_full_depth() {
        // A container running past the window's bottom: the pointer at
        // the window's last row is nowhere near the container's zone.
        let tall = Rect::new(0.0, 0.0, 800.0, 2000.0);
        let mut r = EdgeRamp::default();
        let s = r.step(
            &S,
            tall,
            window(),
            Vec2::new(10.0, 599.0),
            (false, true),
            1.0,
        );
        assert!((s.y - S.speed_px_s).abs() < 1e-2, "{s:?}");
        let mut r = EdgeRamp::default();
        let s = r.step(
            &S,
            tall,
            window(),
            Vec2::new(10.0, 590.0),
            (false, true),
            1.0,
        );
        assert_eq!(s.y, 0.0, "a few px off the edge is not pinned");
    }

    #[test]
    fn the_ramp_climbs_to_its_top_and_the_cap_holds() {
        let mut r = EdgeRamp::default();
        let p = Vec2::new(200.0, 400.0);
        let mut last = 0.0;
        for _ in 0..=90 {
            let v = r.step(&S, list(), window(), p, (false, true), FRAME).y / FRAME;
            assert!(v >= last, "{v} after {last}");
            last = v;
        }
        assert!((last - S.speed_px_s * S.ramp_max).abs() < 1.0, "{last}");
        // Far past the edge the cap holds.
        let v =
            r.step(
                &S,
                list(),
                window(),
                Vec2::new(200.0, 590.0),
                (false, true),
                FRAME,
            )
            .y / FRAME;
        assert!((v - S.cap_px_s).abs() < 1e-2, "{v}");
    }

    #[test]
    fn pulling_back_restarts_the_ramp_and_leaving_resets_it() {
        let mut r = EdgeRamp::default();
        for _ in 0..60 {
            r.step(
                &S,
                list(),
                window(),
                Vec2::new(200.0, 400.0),
                (false, true),
                FRAME,
            );
        }
        let held = r.held;
        // Back by less than the threshold: the ramp goes on.
        r.step(
            &S,
            list(),
            window(),
            Vec2::new(200.0, 395.0),
            (false, true),
            FRAME,
        );
        assert!(r.held > held);
        // By more: it starts again.
        let v =
            r.step(
                &S,
                list(),
                window(),
                Vec2::new(200.0, 390.0),
                (false, true),
                FRAME,
            )
            .y / FRAME;
        assert!((v - S.speed_at(50.0, 60.0)).abs() < 1e-2, "{v}");
        // Out of the zone: nothing, and reset.
        assert_eq!(
            r.step(
                &S,
                list(),
                window(),
                Vec2::new(200.0, 250.0),
                (false, true),
                FRAME
            ),
            Vec2::ZERO
        );
        assert_eq!(r, EdgeRamp::default());
    }

    #[test]
    fn a_short_container_keeps_a_still_middle() {
        let short = Rect::new(0.0, 100.0, 400.0, 60.0);
        let mut r = EdgeRamp::default();
        assert_eq!(
            r.step(
                &S,
                short,
                window(),
                Vec2::new(10.0, 130.0),
                (false, true),
                1.0
            ),
            Vec2::ZERO
        );
        assert!(
            r.step(
                &S,
                short,
                window(),
                Vec2::new(10.0, 159.0),
                (false, true),
                1.0
            )
            .y > 0.0
        );
    }

    #[test]
    fn clamped_keeps_numbers_in_range() {
        let f = EdgeScrollFeel {
            zone_px: f32::NAN,
            curve: 9.0,
            ramp_max: 0.0,
            ..S
        }
        .clamped();
        assert_eq!((f.zone_px, f.curve, f.ramp_max), (S.zone_px, 4.0, 1.0));
        assert_eq!(
            EdgeScrollFeel::preset(" Brisk "),
            Some(EdgeScrollFeel::BRISK)
        );
        assert_eq!(EdgeScrollFeel::preset("turbo"), None);
    }
}
