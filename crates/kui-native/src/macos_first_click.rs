//! The first press on a pumped window's edge, on macOS 26 and earlier
//! (backlog RG168, C54).
//!
//! A press on the resize edge of a window whose app is not active goes to
//! AppKit's delayed first-click path: `-[NSWindow _handleMouseDownEvent:
//! isDelayedEvent:]` waits for the activation to come back, 78–230 ms, and
//! only then starts the resize. A native run loop is there the whole time.
//! A pumped one — the Node driver's, which runs AppKit a slice at a time
//! and sleeps in libuv between — saw the activation arrive after a quick
//! click's release, and the resize it then started waited for a release
//! that had come and gone: the window stuck to the pointer until the next
//! click, and a press held and let go outside the window hung the process
//! inside AppKit for good, ⌘Q included. Measured on macOS 26.6.2; macOS 27
//! activates in time and resizes on the first press, as Finder does.
//!
//! So under a pump, on macOS 26 and earlier, AppKit never sees such a
//! press: the monitor below drops it, and the drags and the release of the
//! same gesture with it, activates the app, brings the window forward, and
//! resizes the window itself from the dropped drags — the sides the press
//! landed on follow the pointer, inside the window's min and max sizes and
//! under the menu bar — so the gesture resizes as it would in a native
//! app, and no resize of AppKit's is left waiting for a release (C54).
//! RG168 only activated, and the second press resized. A press anywhere
//! else, a press while the app is active, and every press on macOS 27 go
//! on as before.
//!
//! Tried on macOS 26.6.2 and not enough: posting a release back into the
//! queue after activation (the resize ignored it), and handing the dropped
//! press back to AppKit once the app was active with the button still down,
//! 2–6 ms on (the resize stuck and hung as before).
//!
//! `KUI_EDGE_RESIZE` takes a comma list, read once: `force` installs the
//! monitor on macOS 27 too, so the resize can be tried where the bug is
//! not; `off` only activates, as RG168 did; `log` writes each gesture's
//! press, drag count and release to stderr.

use std::cell::RefCell;
use std::ptr::NonNull;
use std::sync::Once;
use std::time::Instant;

use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{
    NSApplication, NSCursor, NSCursorFrameResizeDirections, NSCursorFrameResizePosition, NSEvent,
    NSEventMask, NSEventType, NSScreen, NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{NSPoint, NSRect, NSSize};

/// How far inside the window's frame a press still counts as its edge, in
/// points: AppKit's resize band reaches a few points in along an edge and
/// the corner's 19 × 19 zone about ten, and the presses that stuck were all
/// within eleven.
pub(crate) const EDGE: f64 = 12.0;

/// How far along an edge from a corner a press on that edge still takes the
/// corner, in points: AppKit's corner zone is 19 on a side.
pub(crate) const CORNER: f64 = 19.0;

/// Whether a press at `x`, `y` (window coordinates, origin bottom left) on a
/// window `w` × `h` is on its edge: within [`EDGE`] of any side, or outside.
pub(crate) fn on_edge(x: f64, y: f64, w: f64, h: f64) -> bool {
    x < EDGE || y < EDGE || x > w - EDGE || y > h - EDGE
}

/// The sides of a window a press on its edge moves.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Sides {
    pub left: bool,
    pub right: bool,
    pub bottom: bool,
    pub top: bool,
}

/// The sides a press at `x`, `y` on a window `w` × `h` grabs: the side it is
/// within [`EDGE`] of, and the side across the corner too when it is within
/// [`CORNER`] of that one. A press on no edge grabs nothing.
pub(crate) fn sides(x: f64, y: f64, w: f64, h: f64) -> Sides {
    grab(x, y, w, h, EDGE, CORNER)
}

/// The sides within `edge` of `x`, `y`, and across the corner within
/// `corner`.
fn grab(x: f64, y: f64, w: f64, h: f64, edge: f64, corner: f64) -> Sides {
    let near = |reach: f64| Sides {
        left: x < reach,
        right: x > w - reach && x >= reach,
        bottom: y < reach,
        top: y > h - reach && y >= reach,
    };
    let (edge, corner) = (near(edge), near(corner));
    let horizontal = edge.left || edge.right;
    let vertical = edge.bottom || edge.top;
    Sides {
        left: edge.left || (vertical && corner.left),
        right: edge.right || (vertical && corner.right),
        bottom: edge.bottom || (horizontal && corner.bottom),
        top: edge.top || (horizontal && corner.top),
    }
}

/// The frame `start` (screen coordinates, origin bottom left) becomes when
/// the pointer has moved `dx`, `dy` since the press that grabbed `sides`:
/// each grabbed side moves with it and the one across stays put, the size
/// kept within `min` and `max`, and the top no higher than `top_limit`.
pub(crate) fn resized(
    start: NSRect,
    sides: Sides,
    dx: f64,
    dy: f64,
    min: NSSize,
    max: NSSize,
    top_limit: f64,
) -> NSRect {
    let clamp = |v: f64, lo: f64, hi: f64| v.min(hi).max(lo);
    let NSRect { origin, size } = start;
    let (mut x, mut y, mut w, mut h) = (origin.x, origin.y, size.width, size.height);
    if sides.right {
        w = clamp(size.width + dx, min.width, max.width);
    } else if sides.left {
        w = clamp(size.width - dx, min.width, max.width);
        x = origin.x + size.width - w;
    }
    if sides.top {
        let hi = max.height.min(top_limit - origin.y);
        h = clamp(size.height + dy, min.height, hi.max(min.height));
    } else if sides.bottom {
        h = clamp(size.height - dy, min.height, max.height);
        y = origin.y + size.height - h;
    }
    NSRect::new(NSPoint::new(x, y), NSSize::new(w, h))
}

/// How far either side of an edge an active window's own resize takes a
/// press, in points, and how far along an edge from a corner it takes the
/// corner: measured on macOS 27, where 2 in or out resized and 4 did not,
/// and 12 from a corner took the corner and 16 the edge alone.
pub(crate) const BAND: f64 = 3.5;
pub(crate) const BAND_CORNER: f64 = 14.0;

/// Where the pointer is over a window once a gesture is over.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Zone {
    /// On the band AppKit's own resize takes, these sides.
    Edge(Sides),
    Content,
    Outside,
}

/// Where `x`, `y` (window coordinates) is over a window `w` × `h`.
pub(crate) fn zone(x: f64, y: f64, w: f64, h: f64) -> Zone {
    if x < -BAND || y < -BAND || x > w + BAND || y > h + BAND {
        return Zone::Outside;
    }
    let s = grab(x, y, w, h, BAND, BAND_CORNER);
    if s != Sides::default() {
        Zone::Edge(s)
    } else {
        Zone::Content
    }
}

/// The cursor AppKit shows over the grabbed sides.
fn position(s: Sides) -> Option<NSCursorFrameResizePosition> {
    use NSCursorFrameResizePosition as P;
    Some(match (s.left, s.right, s.bottom, s.top) {
        (true, _, true, _) => P::BottomLeft,
        (true, _, _, true) => P::TopLeft,
        (_, true, true, _) => P::BottomRight,
        (_, true, _, true) => P::TopRight,
        (true, ..) => P::Left,
        (_, true, ..) => P::Right,
        (_, _, true, _) => P::Bottom,
        (_, _, _, true) => P::Top,
        _ => return None,
    })
}

/// A dropped press's gesture, still down: its drags resize `window` and its
/// release ends it.
struct Gesture {
    window: Retained<NSWindow>,
    sides: Sides,
    frame: NSRect,
    /// The press's [`pointer`].
    mouse: NSPoint,
    min: NSSize,
    max: NSSize,
    top_limit: f64,
    cursor: Option<Retained<NSCursor>>,
    at: Instant,
    drags: u32,
}

impl Gesture {
    /// Moves the grabbed sides to where `e` has the pointer.
    fn follow(&self, e: &NSEvent) {
        let now = pointer(e);
        let (dx, dy) = (now.x - self.mouse.x, self.mouse.y - now.y);
        // A posted event can carry any point; a frame AppKit refuses throws.
        if !dx.is_finite() || !dy.is_finite() {
            return;
        }
        let frame = resized(
            self.frame,
            self.sides,
            dx,
            dy,
            self.min,
            self.max,
            self.top_limit,
        );
        if frame != self.window.frame() {
            self.window.setFrame_display(frame, true);
        }
        // A resize resets the view's cursor rects to the app's cursor.
        if let Some(c) = &self.cursor {
            c.set();
        }
    }
}

/// The cursor after a gesture, kui's until the next press. AppKit's own
/// cursor for the window's edge and the app's for its content are kept by
/// tracking areas that change the cursor only as the pointer crosses into
/// them, and the window became active with the pointer already inside: the
/// resize cursor stayed over the content and outside the window, and an
/// arrow set at the release stayed on the edge, through the native resize
/// the next press started (macOS 26.6.2). So every move until then picks
/// it: the resize cursor on the band, the arrow outside, and over the
/// content the view's cursor rects rebuilt, which is the app's cursor.
struct Custody {
    window: Retained<NSWindow>,
    zone: Option<Zone>,
}

impl Custody {
    /// Puts the cursor right for where `e` has the pointer.
    fn follow(&mut self, e: &NSEvent, mtm: MainThreadMarker, knobs: Knobs, cursors: bool) {
        // The window server's space is the primary screen's, flipped.
        let Some(primary) = NSScreen::screens(mtm).firstObject() else {
            return;
        };
        let p = pointer(e);
        let f = self.window.frame();
        let (x, y) = (
            p.x - f.origin.x,
            primary.frame().size.height - p.y - f.origin.y,
        );
        if !x.is_finite() || !y.is_finite() {
            return;
        }
        let zone = zone(x, y, f.size.width, f.size.height);
        if knobs.log && self.zone != Some(zone) {
            eprintln!("kui edge resize: cursor {zone:?}");
        }
        self.zone = Some(zone);
        match zone {
            Zone::Edge(s) => {
                if let Some(p) = position(s).filter(|_| cursors) {
                    NSCursor::frameResizeCursorFromPosition_inDirections(
                        p,
                        NSCursorFrameResizeDirections::All,
                    )
                    .set();
                }
            }
            Zone::Content => {
                if let Some(view) = self.window.contentView() {
                    self.window.invalidateCursorRectsForView(&view);
                }
            }
            Zone::Outside => NSCursor::arrowCursor().set(),
        }
    }
}

/// Where `e` has the pointer, in the window server's space (origin top
/// left): its own CGEvent's location, which moves with the event, where
/// `+[NSEvent mouseLocation]` (the fallback, flipped) lagged posted drags
/// by a few of them.
fn pointer(e: &NSEvent) -> NSPoint {
    match e.CGEvent() {
        Some(cg) => {
            let p = objc2_core_graphics::CGEvent::location(Some(&cg));
            NSPoint::new(p.x, p.y)
        }
        None => {
            let p = NSEvent::mouseLocation();
            NSPoint::new(p.x, -p.y)
        }
    }
}

/// What `KUI_EDGE_RESIZE` asked for.
#[derive(Clone, Copy, Default)]
struct Knobs {
    force: bool,
    off: bool,
    log: bool,
}

fn knobs() -> Knobs {
    let v = std::env::var("KUI_EDGE_RESIZE").unwrap_or_default();
    let has = |w: &str| v.split(',').any(|p| p.trim() == w);
    Knobs {
        force: has("force"),
        off: has("off"),
        log: has("log"),
    }
}

thread_local! {
    /// A dropped press's gesture is still down: its drags and its release
    /// are dropped too, and resize its window unless `off`.
    static GESTURE: RefCell<Option<Gesture>> = const { RefCell::new(None) };
    /// A gesture's window whose cursor kui keeps until the next press.
    static CUSTODY: RefCell<Option<Custody>> = const { RefCell::new(None) };
}

/// Installs the monitor once per process, on macOS 26 and earlier; from the
/// pumped runner only, since a native run loop never has the gap.
pub(crate) fn install(mtm: MainThreadMarker) {
    static ONCE: Once = Once::new();
    let knobs = knobs();
    let major = objc2_foundation::NSProcessInfo::processInfo()
        .operatingSystemVersion()
        .majorVersion;
    if major >= 27 && !knobs.force {
        return;
    }
    // `frameResizeCursorFromPosition:inDirections:` is macOS 15's.
    let cursors = major >= 15;
    ONCE.call_once(|| {
        let handler = block2::RcBlock::new(move |ev: NonNull<NSEvent>| -> *mut NSEvent {
            // SAFETY: AppKit hands the monitor a live event.
            let e = unsafe { ev.as_ref() };
            match e.r#type() {
                NSEventType::MouseMoved => {
                    CUSTODY.with(|c| {
                        let mut c = c.borrow_mut();
                        if !NSApplication::sharedApplication(mtm).isActive() {
                            *c = None;
                        } else if let Some(c) = &mut *c {
                            c.follow(e, mtm, knobs, cursors);
                        }
                    });
                    return ev.as_ptr();
                }
                NSEventType::RightMouseDown | NSEventType::OtherMouseDown => {
                    CUSTODY.with(|c| *c.borrow_mut() = None);
                    return ev.as_ptr();
                }
                NSEventType::LeftMouseDown => {
                    CUSTODY.with(|c| *c.borrow_mut() = None);
                    GESTURE.with(|g| *g.borrow_mut() = None);
                }
                NSEventType::LeftMouseDragged => {
                    return GESTURE.with(|g| match &mut *g.borrow_mut() {
                        Some(g) => {
                            g.drags += 1;
                            if !knobs.off {
                                g.follow(e);
                            }
                            std::ptr::null_mut()
                        }
                        None => ev.as_ptr(),
                    });
                }
                NSEventType::LeftMouseUp => {
                    let Some(g) = GESTURE.with(|g| g.borrow_mut().take()) else {
                        return ev.as_ptr();
                    };
                    if !knobs.off {
                        g.follow(e);
                        let mut c = Custody {
                            window: g.window.clone(),
                            zone: None,
                        };
                        c.follow(e, mtm, knobs, cursors);
                        CUSTODY.with(|slot| *slot.borrow_mut() = Some(c));
                    }
                    if knobs.log {
                        let f = g.window.frame();
                        eprintln!(
                            "kui edge resize: released after {} ms, {} drags, {}x{} at {},{}",
                            g.at.elapsed().as_millis(),
                            g.drags,
                            f.size.width,
                            f.size.height,
                            f.origin.x,
                            f.origin.y,
                        );
                    }
                    return std::ptr::null_mut();
                }
                _ => return ev.as_ptr(),
            }
            let app = NSApplication::sharedApplication(mtm);
            if app.isActive() {
                return ev.as_ptr();
            }
            let Some(window) = e.window(mtm) else {
                return ev.as_ptr();
            };
            let style = window.styleMask();
            if !style.contains(NSWindowStyleMask::Resizable)
                || style.contains(NSWindowStyleMask::FullScreen)
            {
                return ev.as_ptr();
            }
            let (p, frame) = (e.locationInWindow(), window.frame());
            let size = frame.size;
            if !on_edge(p.x, p.y, size.width, size.height) {
                return ev.as_ptr();
            }
            #[allow(deprecated)]
            app.activateIgnoringOtherApps(true);
            window.makeKeyAndOrderFront(None);
            let sides = sides(p.x, p.y, size.width, size.height);
            // winit keeps the app's min and max as the content's; the frame's
            // own are AppKit's defaults unless something set them.
            let content = window.contentRectForFrameRect(frame).size;
            let (bw, bh) = (size.width - content.width, size.height - content.height);
            let (cmin, cmax) = (window.contentMinSize(), window.contentMaxSize());
            let (fmin, fmax) = (window.minSize(), window.maxSize());
            let min = NSSize::new(
                fmin.width.max(cmin.width + bw),
                fmin.height.max(cmin.height + bh),
            );
            let max = NSSize::new(
                fmax.width.min(cmax.width + bw).max(min.width),
                fmax.height.min(cmax.height + bh).max(min.height),
            );
            let top_limit = window
                .screen()
                .map_or(f64::INFINITY, |s| {
                    let v = s.visibleFrame();
                    v.origin.y + v.size.height
                })
                .max(frame.origin.y + size.height);
            let cursor = (cursors && !knobs.off)
                .then(|| position(sides))
                .flatten()
                .map(|p| {
                    NSCursor::frameResizeCursorFromPosition_inDirections(
                        p,
                        NSCursorFrameResizeDirections::All,
                    )
                });
            if let Some(c) = &cursor {
                c.set();
            }
            if knobs.log {
                eprintln!(
                    "kui edge resize: press at {},{} on {}x{}, {sides:?}",
                    p.x, p.y, size.width, size.height
                );
            }
            let gesture = Gesture {
                window,
                sides,
                frame,
                mouse: pointer(e),
                min,
                max,
                top_limit,
                cursor,
                at: Instant::now(),
                drags: 0,
            };
            GESTURE.with(|g| *g.borrow_mut() = Some(gesture));
            std::ptr::null_mut()
        });
        // SAFETY: the handler returns the event it was given, or nil to drop
        // it, as the monitor's contract asks; it lives for the process.
        let monitor = unsafe {
            NSEvent::addLocalMonitorForEventsMatchingMask_handler(
                NSEventMask::LeftMouseDown
                    | NSEventMask::LeftMouseDragged
                    | NSEventMask::LeftMouseUp
                    | NSEventMask::MouseMoved
                    | NSEventMask::RightMouseDown
                    | NSEventMask::OtherMouseDown,
                &handler,
            )
        };
        std::mem::forget(monitor);
        std::mem::forget(handler);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_press_within_the_band_is_the_edge() {
        let (w, h) = (1100.0, 780.0);
        // The presses that stuck on the work Mac, all bottom edge or corner.
        for (x, y) in [
            (1096.0, -1.0),
            (1098.0, 3.0),
            (1066.0, 4.0),
            (979.0, 8.0),
            (1101.0, 11.0),
            (1099.0, 7.0),
        ] {
            assert!(on_edge(x, y, w, h), "{x},{y}");
        }
        assert!(on_edge(5.0, 400.0, w, h), "left");
        assert!(on_edge(600.0, 775.0, w, h), "top");
        assert!(on_edge(1095.0, 400.0, w, h), "right");
        // A press in the content is a click, activation and all.
        for (x, y) in [(942.0, 62.0), (866.0, 268.0), (550.0, 390.0), (13.0, 13.0)] {
            assert!(!on_edge(x, y, w, h), "{x},{y}");
        }
    }

    fn s(left: bool, right: bool, bottom: bool, top: bool) -> Sides {
        Sides {
            left,
            right,
            bottom,
            top,
        }
    }

    #[test]
    fn a_press_grabs_its_side_and_the_corner_near_it() {
        let (w, h) = (1100.0, 780.0);
        // The work Mac's presses: corners, and the bottom edge away from one.
        assert_eq!(sides(1096.0, -1.0, w, h), s(false, true, true, false));
        assert_eq!(sides(1101.0, 11.0, w, h), s(false, true, true, false));
        assert_eq!(sides(1066.0, 4.0, w, h), s(false, false, true, false));
        assert_eq!(sides(979.0, 8.0, w, h), s(false, false, true, false));
        // Within the corner's reach along an edge, the corner.
        assert_eq!(sides(1085.0, 4.0, w, h), s(false, true, true, false));
        assert_eq!(sides(4.0, 765.0, w, h), s(true, false, false, true));
        // Mid-edge, the one side; outside the frame still counts.
        assert_eq!(sides(-2.0, 400.0, w, h), s(true, false, false, false));
        assert_eq!(sides(600.0, 776.0, w, h), s(false, false, false, true));
        assert_eq!(sides(550.0, 390.0, w, h), Sides::default());
    }

    fn rect(x: f64, y: f64, w: f64, h: f64) -> NSRect {
        NSRect::new(NSPoint::new(x, y), NSSize::new(w, h))
    }

    const MIN: NSSize = NSSize::new(720.0, 560.0);
    const MAX: NSSize = NSSize::new(f64::MAX, f64::MAX);

    #[test]
    fn the_grabbed_sides_follow_and_the_others_stay() {
        let start = rect(100.0, 200.0, 1100.0, 780.0);
        // Bottom right, dragged out and down: wider, taller, top left fixed.
        let f = resized(
            start,
            s(false, true, true, false),
            50.0,
            -30.0,
            MIN,
            MAX,
            2000.0,
        );
        assert_eq!(f, rect(100.0, 170.0, 1150.0, 810.0));
        // Left, dragged right: narrower, right side fixed.
        let f = resized(
            start,
            s(true, false, false, false),
            40.0,
            99.0,
            MIN,
            MAX,
            2000.0,
        );
        assert_eq!(f, rect(140.0, 200.0, 1060.0, 780.0));
        // Top, dragged up.
        let f = resized(
            start,
            s(false, false, false, true),
            99.0,
            20.0,
            MIN,
            MAX,
            2000.0,
        );
        assert_eq!(f, rect(100.0, 200.0, 1100.0, 800.0));
    }

    #[test]
    fn the_size_stays_within_min_and_max_and_under_the_menu_bar() {
        let start = rect(100.0, 200.0, 1100.0, 780.0);
        // Past the min from the bottom left: the top right corner holds.
        let f = resized(
            start,
            s(true, false, true, false),
            900.0,
            900.0,
            MIN,
            MAX,
            2000.0,
        );
        assert_eq!(f, rect(480.0, 420.0, 720.0, 560.0));
        // Past the max.
        let max = NSSize::new(1200.0, 900.0);
        let f = resized(
            start,
            s(false, true, false, true),
            500.0,
            500.0,
            MIN,
            max,
            2000.0,
        );
        assert_eq!(f, rect(100.0, 200.0, 1200.0, 900.0));
        // The top stops at the visible frame's.
        let f = resized(
            start,
            s(false, false, false, true),
            0.0,
            500.0,
            MIN,
            MAX,
            1000.0,
        );
        assert_eq!(f, rect(100.0, 200.0, 1100.0, 800.0));
    }

    #[test]
    fn after_a_gesture_the_band_is_appkits() {
        let (w, h) = (600.0, 432.0);
        // What an active window resized from on macOS 27, and what it did not.
        for x in [-2.0, 0.0, 2.0] {
            assert_eq!(zone(x, 200.0, w, h), Zone::Edge(s(true, false, false, false)), "{x}");
        }
        assert_eq!(zone(4.0, 200.0, w, h), Zone::Content);
        assert_eq!(zone(-4.0, 200.0, w, h), Zone::Outside);
        assert_eq!(zone(12.0, 2.0, w, h), Zone::Edge(s(true, false, true, false)));
        assert_eq!(zone(16.0, 2.0, w, h), Zone::Edge(s(false, false, true, false)));
        // An edge press grabs well inside the band; the cursor after it does not.
        assert_eq!(sides(8.0, 200.0, w, h), s(true, false, false, false));
        assert_eq!(zone(8.0, 200.0, w, h), Zone::Content);
    }

    #[test]
    fn each_grab_has_its_cursor() {
        use NSCursorFrameResizePosition as P;
        assert_eq!(position(s(false, true, true, false)), Some(P::BottomRight));
        assert_eq!(position(s(true, false, false, true)), Some(P::TopLeft));
        assert_eq!(position(s(false, false, true, false)), Some(P::Bottom));
        assert_eq!(position(Sides::default()), None);
    }
}
