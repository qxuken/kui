//! The keep-out rect under macOS custom chrome — where the traffic lights
//! are and how tall the OS's own titlebar is — asked of the window rather
//! than assumed.
//!
//! The numbers move with the OS: macOS 26 drew 12 pt buttons in a 28 pt
//! titlebar, macOS 27 draws 14 pt buttons at (9, 9) in a 32 pt one, and a
//! constant measured on one release is what the strip's height was wrong
//! by on the next. AppKit has no public "titlebar height" either, but a
//! window with a full-size content view answers both halves anyway: the
//! `contentLayoutRect` is the part of the frame *under* the titlebar, so
//! the frame's height less its height is the titlebar; and the standard
//! window buttons are views with frames.
//!
//! The one thing set here rather than asked is the titlebar's height
//! (backlog W22): AppKit lays the lights out itself and puts them back
//! wherever it likes on a resize, so the runner does not move them — it
//! asks for AppKit's taller titlebar, the one an empty toolbar makes, and
//! AppKit centres them in it.

use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{NSToolbar, NSView, NSWindow, NSWindowButton, NSWindowToolbarStyle};
use winit::window::Window;

use crate::{Rect, Titlebar};

/// What the runner reported before it measured: gpui's
/// `TRAFFIC_LIGHT_PADDING` under the macOS 26 SDK (78) over the 28 pt
/// titlebar macOS 26 drew. Only the answer when the window has no AppKit
/// view to ask, or its buttons are not there to measure.
const ASSUMED: Rect = Rect {
    x: 0.0,
    y: 0.0,
    w: 78.0,
    h: 28.0,
};

/// The rect `env.window.native_controls` reports for `window`: from the
/// window's origin, as wide as the traffic lights plus the same gap after
/// them that AppKit leaves before them, and as tall as the strip the
/// lights are centred in — the OS's titlebar, but for a `Titlebar::Tall`
/// one, whose lower 14 pt the lights are not centred over (backlog W22).
pub fn native_controls(window: &Window) -> Rect {
    measure(window).unwrap_or(ASSUMED)
}

/// Gives `window` the titlebar `titlebar` names: none of a toolbar for
/// `Standard`, an empty one in the style that makes the strip as tall for
/// the others. Called once, before the window is shown.
pub fn set_titlebar(window: &Window, titlebar: Titlebar) {
    let style = match titlebar {
        Titlebar::Standard => return,
        Titlebar::Medium => NSWindowToolbarStyle::UnifiedCompact,
        Titlebar::Tall => NSWindowToolbarStyle::Unified,
    };
    let Some(ns_window) = ns_window(window) else {
        return;
    };
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    // No items, no identifier to autosave under: the toolbar is there for
    // the height its style gives the titlebar, and draws nothing.
    let toolbar = NSToolbar::new(mtm);
    ns_window.setToolbar(Some(&toolbar));
    ns_window.setToolbarStyle(style);
}

fn ns_window(window: &Window) -> Option<Retained<NSWindow>> {
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let RawWindowHandle::AppKit(h) = window.window_handle().ok()?.as_raw() else {
        return None;
    };
    // SAFETY: a live winit content view, on the event loop's thread.
    let view = unsafe { Retained::retain(h.ns_view.as_ptr().cast::<NSView>()) }?;
    view.window()
}

fn measure(window: &Window) -> Option<Rect> {
    let ns_window = ns_window(window)?;
    let frame = ns_window.frame();
    // Under a full-size content view the layout rect is what the titlebar
    // leaves; without one the two are the same height and there is no
    // titlebar over the content to keep out of.
    let titlebar = frame.size.height - ns_window.contentLayoutRect().size.height;
    if titlebar <= 0.0 {
        return None;
    }
    // Both buttons in window coordinates: the leading gap is the close
    // button's x, and the same gap follows the zoom button.
    let in_window = |b: NSWindowButton| {
        let button = ns_window.standardWindowButton(b)?;
        // SAFETY: the button's superview is AppKit's titlebar container,
        // retained for as long as the window; read on the main thread.
        let superview = unsafe { button.superview() }?;
        Some(superview.convertRect_toView(button.frame(), None))
    };
    let close = in_window(NSWindowButton::CloseButton)?;
    let zoom = in_window(NSWindowButton::ZoomButton)?;
    let leading = close.origin.x;
    let trailing = zoom.origin.x + zoom.size.width;
    if leading < 0.0 || trailing <= leading {
        return None;
    }
    // Window coordinates run up from the bottom edge. The plain titlebar
    // and the compact toolbar's centre the lights (32 with them at 9, 40
    // at 13 on macOS 27); the full toolbar's is 66 with them at 19, laid
    // out for a 52 pt row of items, and the 14 pt under the row is the
    // OS's but draws nothing and takes no click. The strip an app lays
    // out against is the one its content centres on the lights in, so the
    // height is twice the lights' centre, never past the titlebar.
    let centre = frame.size.height - (close.origin.y + close.size.height / 2.0);
    let h = (2.0 * centre).min(titlebar);
    if h <= 0.0 {
        return None;
    }
    Some(Rect::new(0.0, 0.0, (trailing + leading) as f32, h as f32))
}
