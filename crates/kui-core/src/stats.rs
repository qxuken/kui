//! Frame timing samples for the latency graph. The core only stores data —
//! whoever drives the frame loop (the runner, an FFI host) measures and
//! pushes; `widgets::latency_graph` renders it with ordinary primitives.

/// One frame's cost in milliseconds, split by phase.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FrameSample {
    /// Input handling since the previous frame (event routing + edits).
    pub input_ms: f32,
    /// Host + extension view() calls (tree building).
    pub view_ms: f32,
    /// Layout, text measurement, and display-list emission.
    pub layout_ms: f32,
    /// GPU encode + present (actual work).
    pub render_ms: f32,
    /// Blocked waiting for a swapchain image (vsync backpressure) — real
    /// latency, but pacing rather than work.
    pub wait_ms: f32,
}

impl FrameSample {
    pub fn total(&self) -> f32 {
        self.input_ms + self.view_ms + self.layout_ms + self.render_ms + self.wait_ms
    }

    /// Time the app is responsible for (everything except vsync pacing).
    pub fn work(&self) -> f32 {
        self.input_ms + self.view_ms + self.layout_ms + self.render_ms
    }
}

pub const STATS_CAPACITY: usize = 120;

/// Fixed-size ring of recent frame samples.
#[derive(Default)]
pub struct FrameStats {
    samples: Vec<FrameSample>,
    head: usize,
    /// Input time accumulated since the last push (events between frames).
    pub pending_input_ms: f32,
}

impl FrameStats {
    pub fn push(&mut self, mut sample: FrameSample) {
        sample.input_ms += self.pending_input_ms;
        self.pending_input_ms = 0.0;
        if self.samples.len() < STATS_CAPACITY {
            self.samples.push(sample);
        } else {
            self.samples[self.head] = sample;
        }
        self.head = (self.head + 1) % STATS_CAPACITY;
    }

    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// Samples oldest -> newest.
    pub fn iter(&self) -> impl Iterator<Item = FrameSample> + '_ {
        let (tail, front) = if self.samples.len() < STATS_CAPACITY {
            (&self.samples[..], &[][..])
        } else {
            let (a, b) = self.samples.split_at(self.head);
            (b, a)
        };
        tail.iter().chain(front.iter()).copied()
    }

    pub fn last(&self) -> Option<FrameSample> {
        if self.samples.is_empty() {
            return None;
        }
        let i = (self.head + STATS_CAPACITY - 1) % STATS_CAPACITY;
        self.samples.get(i).or(self.samples.last()).copied()
    }

    pub fn avg_total(&self) -> f32 {
        if self.samples.is_empty() {
            return 0.0;
        }
        self.samples.iter().map(FrameSample::total).sum::<f32>() / self.samples.len() as f32
    }

    pub fn max_total(&self) -> f32 {
        self.samples.iter().map(FrameSample::total).fold(0.0, f32::max)
    }

    pub fn avg_work(&self) -> f32 {
        if self.samples.is_empty() {
            return 0.0;
        }
        self.samples.iter().map(FrameSample::work).sum::<f32>() / self.samples.len() as f32
    }

    pub fn max_work(&self) -> f32 {
        self.samples.iter().map(FrameSample::work).fold(0.0, f32::max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(ms: f32) -> FrameSample {
        FrameSample { view_ms: ms, ..Default::default() }
    }

    #[test]
    fn ring_wraps_and_iterates_in_order() {
        let mut s = FrameStats::default();
        for i in 0..(STATS_CAPACITY + 10) {
            s.push(sample(i as f32));
        }
        assert_eq!(s.len(), STATS_CAPACITY);
        let v: Vec<f32> = s.iter().map(|f| f.view_ms).collect();
        assert_eq!(v.first().copied(), Some(10.0)); // oldest surviving
        assert_eq!(v.last().copied(), Some((STATS_CAPACITY + 9) as f32));
        assert_eq!(s.last().unwrap().view_ms, (STATS_CAPACITY + 9) as f32);
        // Strictly increasing: ring order is preserved.
        assert!(v.windows(2).all(|w| w[1] > w[0]));
    }

    #[test]
    fn pending_input_folds_into_next_sample() {
        let mut s = FrameStats::default();
        s.pending_input_ms = 0.5;
        s.push(FrameSample { view_ms: 1.0, ..Default::default() });
        let last = s.last().unwrap();
        assert_eq!(last.input_ms, 0.5);
        assert_eq!(s.pending_input_ms, 0.0);
        assert!((last.total() - 1.5).abs() < 1e-6);
    }

    #[test]
    fn aggregates() {
        let mut s = FrameStats::default();
        s.push(sample(1.0));
        s.push(sample(3.0));
        assert_eq!(s.avg_total(), 2.0);
        assert_eq!(s.max_total(), 3.0);
    }
}
