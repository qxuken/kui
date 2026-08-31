//! Env + declared window title: frame-scoped data the driver reconciles.

use kui_core::{Core, Size};

#[test]
fn window_title_is_frame_scoped() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(100.0, 100.0), 1.0);
    ui.window_title("hello");
    ui.finish();
    assert_eq!(core.window_title(), Some("hello"));

    // An undeclared frame clears it — "leave as-is" for the driver.
    let ui = core.frame(Size::new(100.0, 100.0), 1.0);
    ui.finish();
    assert_eq!(core.window_title(), None);
}

#[test]
fn env_defaults_are_headless_safe() {
    let core = Core::new();
    assert_eq!(core.env.refresh_hz, None);
    assert!(core.env.focused);
    assert!((core.env.frame_budget_ms() - 1000.0 / 120.0).abs() < 1e-4);
}
