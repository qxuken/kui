//! Host environment facts pushed into the core by the frame driver — the
//! inbound mirror of events-as-data. The core never touches a window; the
//! driver (runner, FFI host) reports what it knows and views read it.

/// What the host knows about the display/window. Defaults are safe for
/// headless drivers (tests, benches) that never set anything.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Env {
    /// Display refresh rate in Hz; `None` when the host can't tell.
    pub refresh_hz: Option<f32>,
    /// Whether the window has keyboard focus.
    pub focused: bool,
}

impl Default for Env {
    fn default() -> Self {
        Self { refresh_hz: None, focused: true }
    }
}

impl Env {
    /// Budget fallback when the host doesn't report a refresh rate.
    pub const DEFAULT_HZ: f32 = 120.0;

    /// Per-frame time budget in ms: one vsync interval at the display's
    /// refresh rate (120 Hz when unreported).
    pub fn frame_budget_ms(&self) -> f32 {
        let hz = self.refresh_hz.filter(|hz| *hz > 0.0).unwrap_or(Self::DEFAULT_HZ);
        1000.0 / hz
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_follows_reported_rate() {
        let env = Env { refresh_hz: Some(60.0), ..Default::default() };
        assert!((env.frame_budget_ms() - 16.666).abs() < 1e-2);
    }

    #[test]
    fn budget_falls_back_when_unreported_or_bogus() {
        assert!((Env::default().frame_budget_ms() - 8.333).abs() < 1e-2);
        let bogus = Env { refresh_hz: Some(0.0), ..Default::default() };
        assert!((bogus.frame_budget_ms() - 8.333).abs() < 1e-2);
    }
}
