//! Touch release velocity and kinetic scrolling shared by canvas and native hosts.
use crate::Vec2;
use std::collections::VecDeque;

/// Logical pixels of finger jitter allowed before a press becomes a drag.
pub const TOUCH_SLOP: f32 = 8.0;

// Sampling and release timestamps use monotonic seconds.
const SAMPLE_WINDOW: f64 = 0.080;
// winit can deliver a batch after a slow Android presentation. Keep one
// extra segment when callbacks are farther apart than the sampling window.
const MAX_SAMPLE_GAP: f64 = 0.25;
const MIN_SPEED: f32 = 20.;
const MAX_SPEED: f32 = 3000.;
const DECAY: f32 = 5.;

/// Momentum left by a released pan, in logical scroll units per second.
pub struct Glide {
    velocity: Vec2,
    time: f64,
}

/// Ordered contact positions from a single pan.
#[derive(Default)]
pub struct PanSamples {
    positions: VecDeque<(f64, Vec2)>,
}

impl PanSamples {
    pub fn record(&mut self, time: f64, at: Vec2) {
        if let Some(&(previous_time, previous)) = self.positions.back()
            && time - previous_time > MAX_SAMPLE_GAP
        {
            self.positions.clear();
            let delta = at.minus(previous);
            if delta.x.hypot(delta.y) > TOUCH_SLOP {
                // A distinct move following a gap has its own velocity. Drop
                // the old stroke, preserving just the start of this segment.
                self.positions.push_back((previous_time, previous));
            }
        }
        self.positions.push_back((time, at));
        while self.positions.len() > 3 && time - self.positions[1].0 >= SAMPLE_WINDOW {
            self.positions.pop_front();
        }
    }

    /// Scale is the number of sample coordinates per logical scroll unit.
    pub fn release(&self, time: f64, scale: f32) -> Option<Glide> {
        if !scale.is_finite() || scale <= 0. {
            return None;
        }
        let &(last_time, last) = self.positions.back()?;
        let &(previous_time, _) = self.positions.iter().rev().nth(1)?;
        // Compare a stationary lift with the actual delivery cadence. On a
        // busy native host even consecutive events can be a frame apart.
        let stale_after = (2. * (last_time - previous_time)).clamp(SAMPLE_WINDOW, MAX_SAMPLE_GAP);
        if time - last_time > stale_after {
            return None;
        }
        // At normal cadence use the oldest sample in the release window.
        // At low cadence a small final correction can occupy an entire frame.
        // Keep the preceding sparse segment in that case. A substantial last
        // movement (including a reversal) should supply its own direction.
        let mut baseline = self
            .positions
            .iter()
            .rev()
            .skip(1)
            .filter(|(t, _)| last_time - t >= 0.005);
        let &(mut first_time, mut first) = baseline.next()?;
        let tail = last.minus(first);
        for &(t, at) in baseline {
            if last_time - t <= SAMPLE_WINDOW {
                first_time = t;
                first = at;
            } else {
                if last_time - first_time > SAMPLE_WINDOW && tail.x.hypot(tail.y) <= TOUCH_SLOP {
                    first_time = t;
                    first = at;
                }
                break;
            }
        }
        let dt = (last_time - first_time) as f32;
        let delta = last.minus(first);
        let delta = Vec2::new(delta.x / scale, delta.y / scale);
        let speed = delta.x.hypot(delta.y) / dt;
        if speed < MIN_SPEED {
            return None;
        }
        let factor = (MAX_SPEED / speed).min(1.) / dt;
        Some(Glide {
            velocity: Vec2::new(delta.x * factor, delta.y * factor),
            time,
        })
    }
}

impl Glide {
    /// Integrate one frame. Stop after a long interruption or when the target
    /// declines the scroll. Returns whether to schedule another frame.
    pub fn advance(&mut self, time: f64, send: impl FnOnce(Vec2) -> bool) -> bool {
        let dt = (time - self.time) as f32;
        if dt <= 0. {
            return true;
        }
        // A foreground Android software renderer can present less than four
        // frames a second. Consume a bounded step on a late frame; suspension
        // and contact cancellation reset the host gesture independently.
        if dt >= 0.75 {
            return false;
        }
        let decay = (-DECAY * dt).exp();
        let distance = (1. - (-DECAY * dt.min(MAX_SAMPLE_GAP as f32)).exp()) / DECAY;
        let v = self.velocity;
        self.time = time;
        self.velocity = Vec2::new(v.x * decay, v.y * decay);
        send(Vec2::new(v.x * distance, v.y * distance))
            && self.velocity.x.hypot(self.velocity.y) >= MIN_SPEED
    }
}
