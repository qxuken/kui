//! A slider's arithmetic: what value a pointer position, an arrow, a Page
//! key or Home / End means on a node whose role is `Slider` and that
//! declared `on_change`.
//!
//! The core proposes a value and never applies it: the `change` event
//! carries the number, and nothing moves until the view declares it as
//! `value_now`. Nothing here is retained past a drag. The stock control is
//! [`widgets::slider`](crate::widgets::slider); this module is what it and
//! a custom slider share.
//!
//! Numbers are worked in `f64` from the declared `f32`s read back through
//! their shortest decimal spelling, and a result is rounded to the
//! decimals the range and step are written in, so a step of `0.1` from
//! `0` is `0.3` on the wire and not `0.30000001192092896`.

use crate::geom::{Rect, Vec2};
use crate::input::UiEvent;
use crate::key::Key;
use crate::spec::AccessSpec;
use crate::tree::OriginId;
use crate::value::Value;

/// A slider's range and step, as the view declared them. The ARIA
/// defaults stand in for an unset end: 0 and 100.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SliderRange {
    pub min: f64,
    pub max: f64,
    pub step: f64,
    /// Decimals the results are rounded to: the most the declared numbers
    /// are written with.
    decimals: usize,
}

/// One keyboard (or assistive-technology) move of a slider.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SliderMove {
    /// One step up (`1`) or down (`-1`): an arrow, Increment / Decrement.
    Step(i32),
    /// Ten steps: PageUp / PageDown.
    Page(i32),
    /// The range's start: Home.
    Home,
    /// The range's end: End.
    End,
}

/// An `f32` as the decimal it was written as: `0.1f32` is `0.1`, not
/// `0.10000000149011612`.
pub(crate) fn exact(v: f32) -> f64 {
    format!("{v}").parse().unwrap_or(v as f64)
}

/// How many decimals `v`'s shortest spelling has.
fn decimals_of(v: f64) -> usize {
    let s = format!("{v}");
    s.split_once('.').map_or(0, |(_, frac)| frac.len())
}

impl SliderRange {
    /// The range a node declares, or `None` when it cannot be one: an end
    /// that is not finite, or a max not past the min.
    pub fn of(ax: &AccessSpec) -> Option<Self> {
        let min = exact(ax.value_min.unwrap_or(0.0));
        let max = exact(ax.value_max.unwrap_or(100.0));
        if !(min.is_finite() && max.is_finite()) || max <= min {
            return None;
        }
        let step = match ax.value_step {
            Some(s) => exact(s),
            None => (max - min) / 100.0,
        };
        // A hundredth of a range like 0..1 is 0.01, which has two
        // decimals; one of 0..7 is 0.07. The step's own spelling says
        // what the grid is.
        let decimals = decimals_of(min)
            .max(decimals_of(max))
            .max(decimals_of(step))
            .min(9);
        Some(SliderRange {
            min,
            max,
            step,
            decimals,
        })
    }

    /// `v` on the step grid from `min`, inside the range. The top of the
    /// range is always reachable even when the step does not divide it.
    pub fn snap(&self, v: f64) -> f64 {
        let v = v.clamp(self.min, self.max);
        let k = ((v - self.min) / self.step).round();
        let snapped = (self.min + k * self.step).min(self.max);
        let snapped = if self.max - v < (v - snapped).abs() {
            self.max
        } else {
            snapped
        };
        self.round(snapped)
    }

    fn round(&self, v: f64) -> f64 {
        format!("{:.*}", self.decimals, v).parse().unwrap_or(v)
    }

    /// The value a fraction of the track means, snapped.
    pub fn at_fraction(&self, f: f64) -> f64 {
        self.snap(self.min + f.clamp(0.0, 1.0) * (self.max - self.min))
    }

    /// Where one move from `now` lands, snapped and clamped. `now` outside
    /// the range moves from the end it is past.
    pub fn moved(&self, now: Option<f32>, mv: SliderMove) -> f64 {
        let now = now.map_or(self.min, exact).clamp(self.min, self.max);
        match mv {
            SliderMove::Step(n) => self.snap(now + n as f64 * self.step),
            SliderMove::Page(n) => self.snap(now + 10.0 * n as f64 * self.step),
            SliderMove::Home => self.min,
            SliderMove::End => self.max,
        }
    }
}

/// A slider's track as the pointer reads it: the node's content box along
/// its main axis, and the range it maps onto. Registered on the node's
/// hit region when it declared `on_change` (`HitRegion::slider`).
#[derive(Clone, Debug, PartialEq)]
pub struct SliderTrack {
    /// Where the track starts and how long it is, logical px along its
    /// axis: the content box, so a stock slider's padding — half its thumb
    /// — keeps the thumb's centre under the pointer at both ends.
    pub start: f32,
    pub len: f32,
    /// A column slider runs bottom to top, its maximum at the top.
    pub vertical: bool,
    pub range: SliderRange,
    pub tag: Value,
}

impl SliderTrack {
    /// The track of a node laid out at `rect` with `padding`, running
    /// along its main axis.
    pub fn new(
        rect: Rect,
        padding: crate::geom::Edges,
        vertical: bool,
        range: SliderRange,
        tag: Value,
    ) -> Self {
        let (start, len) = if vertical {
            (rect.y + padding.t, rect.h - padding.y())
        } else {
            (rect.x + padding.l, rect.w - padding.x())
        };
        SliderTrack {
            start,
            len: len.max(0.0),
            vertical,
            range,
            tag,
        }
    }

    /// The value under `p`, snapped.
    pub fn value_at(&self, p: Vec2) -> f64 {
        if self.len <= 0.0 {
            return self.range.min;
        }
        let along = if self.vertical { p.y } else { p.x };
        let f = ((along - self.start) / self.len) as f64;
        self.range
            .at_fraction(if self.vertical { 1.0 - f } else { f })
    }
}

/// `{kind="change", value, phase, tag}` on the slider.
pub fn change_event(origin: OriginId, key: Key, value: f64, phase: &str, tag: &Value) -> UiEvent {
    let payload = Value::map([
        ("kind", Value::str("change")),
        ("value", Value::Float(value)),
        ("phase", Value::str(phase)),
    ]);
    UiEvent::on(origin, key, payload).tagged(Some(tag))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::Edges;

    fn range(min: f32, max: f32, step: Option<f32>) -> SliderRange {
        SliderRange::of(&AccessSpec {
            value_min: Some(min),
            value_max: Some(max),
            value_step: step,
            ..AccessSpec::EMPTY
        })
        .unwrap()
    }

    #[test]
    fn a_step_lands_on_the_decimal_it_names() {
        let r = range(0.0, 1.0, Some(0.1));
        assert_eq!(r.moved(Some(0.2), SliderMove::Step(1)), 0.3);
        assert_eq!(r.moved(Some(0.3), SliderMove::Step(-1)), 0.2);
        assert_eq!(r.moved(Some(0.95), SliderMove::Step(1)), 1.0);
        assert_eq!(r.moved(Some(0.0), SliderMove::Step(-1)), 0.0);
    }

    #[test]
    fn the_default_step_is_a_hundredth_and_the_ends_are_aria_s() {
        let r = SliderRange::of(&AccessSpec::EMPTY).unwrap();
        assert_eq!((r.min, r.max, r.step), (0.0, 100.0, 1.0));
        assert_eq!(r.moved(Some(41.0), SliderMove::Page(1)), 51.0);
        assert_eq!(r.moved(None, SliderMove::End), 100.0);
        assert_eq!(r.moved(Some(3.0), SliderMove::Home), 0.0);
    }

    #[test]
    fn a_step_that_does_not_divide_the_range_still_reaches_the_top() {
        let r = range(0.0, 10.0, Some(3.0));
        assert_eq!(r.snap(9.9), 10.0);
        assert_eq!(r.snap(8.0), 9.0);
        assert_eq!(r.moved(Some(9.0), SliderMove::Step(1)), 10.0);
    }

    #[test]
    fn a_range_that_is_not_one_is_none() {
        let flat = AccessSpec {
            value_min: Some(5.0),
            value_max: Some(5.0),
            ..AccessSpec::EMPTY
        };
        assert!(SliderRange::of(&flat).is_none());
    }

    #[test]
    fn the_pointer_reads_the_content_box_and_a_column_runs_upward() {
        let r = range(0.0, 100.0, Some(10.0));
        let pad = Edges {
            l: 8.0,
            r: 8.0,
            t: 8.0,
            b: 8.0,
        };
        let t = SliderTrack::new(Rect::new(0.0, 0.0, 116.0, 16.0), pad, false, r, Value::Null);
        assert_eq!(t.value_at(Vec2::new(8.0, 8.0)), 0.0);
        assert_eq!(t.value_at(Vec2::new(58.0, 8.0)), 50.0);
        assert_eq!(t.value_at(Vec2::new(-40.0, 8.0)), 0.0);
        assert_eq!(t.value_at(Vec2::new(500.0, 8.0)), 100.0);
        let v = SliderTrack::new(Rect::new(0.0, 0.0, 16.0, 116.0), pad, true, r, Value::Null);
        assert_eq!(v.value_at(Vec2::new(8.0, 8.0)), 100.0);
        assert_eq!(v.value_at(Vec2::new(8.0, 108.0)), 0.0);
    }
}
