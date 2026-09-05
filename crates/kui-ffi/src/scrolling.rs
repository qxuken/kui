//! Retained scroll offsets from outside: reveal a node, set or read an
//! offset, and the geometry a virtual list slices by.

use super::*;

/// Scrolls whatever contains `key` so the node shows — what Tab does to
/// the control it lands on, asked for by name. The request resolves at the
/// next `kui_frame_finish`, against the frame it lays out (the one being
/// built when called from a view callback, the one after it otherwise — a
/// frame is requested, so one comes), so a row a view is about to declare
/// for the first time reveals fine. A key that frame does not declare, or
/// one with nothing scrollable above it, is a no-op and is not kept for a
/// later frame; the last reveal before a frame wins.
#[unsafe(no_mangle)]
pub extern "C" fn kui_reveal(ptr: *mut KuiCtx, key: u64) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().reveal(Key(key));
        }
    });
}

/// Sets a scroll container's retained offset, the way the wheel would
/// (positive = content moved up / left). Takes effect on the next frame,
/// whose layout clamps it to that frame's overflow: 0,0 is "jump to the
/// top" and a huge value is "jump to the end" without knowing the content
/// height. An offset written for a key that never scrolls is harmless.
#[unsafe(no_mangle)]
pub extern "C" fn kui_set_scroll(ptr: *mut KuiCtx, key: u64, x: f32, y: f32) {
    guard((), || {
        if let Some(c) = unsafe { ctx(ptr) } {
            c.core().set_scroll(Key(key), kui_core::Vec2::new(x, y));
        }
    });
}

/// Reads that offset back, as the last layout clamped it — the number to
/// persist and hand to `kui_set_scroll` later. Writes 0,0 for a node that
/// never scrolled; either out pointer may be NULL.
#[unsafe(no_mangle)]
pub extern "C" fn kui_scroll_offset(ptr: *mut KuiCtx, key: u64, x: *mut f32, y: *mut f32) {
    guard((), || {
        let off =
            unsafe { ctx(ptr) }.map_or(kui_core::Vec2::ZERO, |c| c.core().scroll_offset(Key(key)));
        unsafe {
            if let Some(x) = x.as_mut() {
                *x = off.x;
            }
            if let Some(y) = y.as_mut() {
                *y = off.y;
            }
        }
    });
}

/// Everything the last layout resolved for the container `key`. Returns
/// false — leaving `out` untouched — for a bad context, a NULL `out`, or a
/// key no layout has ever resolved as a scroll container.
///
/// This is what makes a long list affordable: the core builds every child a
/// view declares, so ten thousand rows cost ten thousand rows, but a view
/// that knows `h` and `offset_y` can declare the rows that fit plus two
/// spacers holding the space of the rest, and pay for a screenful. Read
/// during a build it describes the previous frame, so a resize slices one
/// frame late — build a row or two extra at each end.
#[unsafe(no_mangle)]
pub extern "C" fn kui_scroll_geometry(
    ptr: *mut KuiCtx,
    key: u64,
    out: *mut KuiScrollGeometry,
) -> bool {
    guard(false, || {
        let Some(c) = (unsafe { ctx(ptr) }) else {
            return false;
        };
        let Some(g) = c.core().scroll_geometry(Key(key)) else {
            return false;
        };
        write_out(
            out,
            KuiScrollGeometry {
                x: g.rect.x,
                y: g.rect.y,
                w: g.rect.w,
                h: g.rect.h,
                content_w: g.content.w,
                content_h: g.content.h,
                offset_x: g.offset.x,
                offset_y: g.offset.y,
                max_offset_x: g.max_offset.x,
                max_offset_y: g.max_offset.y,
                ..Default::default()
            },
        )
    })
}
