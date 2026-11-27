//! winit reports WM_MOUSEWHEEL and WM_MOUSEHWHEEL in physical notches.
//! Windows supplies the amount of travel separately, per axis.
use kui_core::{Core, InputEvent, UiEvent, Vec2, WheelFeel};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    SPI_GETWHEELSCROLLCHARS, SPI_GETWHEELSCROLLLINES, SystemParametersInfoW,
};

fn setting(horizontal: bool) -> u32 {
    let mut amount = 3_u32;
    let action = if horizontal {
        SPI_GETWHEELSCROLLCHARS
    } else {
        SPI_GETWHEELSCROLLLINES
    };
    let ok = unsafe { SystemParametersInfoW(action, 0, (&raw mut amount).cast(), 0) };
    if ok == 0 { 3 } else { amount }
}

pub(super) fn dispatch(core: &mut Core, lines: Vec2, begins: bool) -> Vec<UiEvent> {
    // The two axes arrive in separate Windows messages. Only query the active
    // axis, so a Settings change applies on the next notch even while focused.
    let horizontal = lines.x != 0.0;
    let amount = setting(horizontal);
    if amount == 0 {
        return Vec::new();
    }
    let event = InputEvent::Wheel { lines, begins };
    if amount == 3 {
        return core.handle_input(event);
    }
    let page = core
        .interaction
        .scroll_regions_at()
        .find_map(|region| {
            let g = core.scroll_geometry(region.key)?;
            if horizontal {
                (g.max_offset.x > 0.0).then_some(g.rect.w)
            } else {
                (g.max_offset.y > 0.0).then_some(g.rect.h)
            }
        })
        .unwrap_or_else(|| {
            if horizontal {
                core.viewport().w
            } else {
                core.viewport().h
            }
        });
    let feel = core.wheel_feel();
    match distance(amount, feel.line_px, page) {
        Distance::Page(px) => core.handle_input(InputEvent::ScrollGesture {
            delta: Vec2::new(lines.x * px, lines.y * px),
            begins,
        }),
        Distance::Line(px) => {
            // Apply to this native dispatch only. Keep the physical notch
            // count for glide, acceleration and handlers that measure lines.
            core.set_wheel_feel(WheelFeel {
                line_px: px,
                ..feel
            });
            let events = core.handle_input(event);
            core.set_wheel_feel(feel);
            events
        }
    }
}

#[derive(Debug, PartialEq)]
enum Distance {
    Line(f32),
    Page(f32),
}

fn distance(amount: u32, base: f32, page: f32) -> Distance {
    let px = base * (amount as f32 / 3.0);
    if amount == u32::MAX || (page > 0.0 && px >= page) {
        Distance::Page(page)
    } else {
        Distance::Line(px)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn system_line_counts_scale_notch_travel_and_page_uses_the_scroller() {
        assert_eq!(distance(1, 60.0, 250.0), Distance::Line(20.0));
        assert_eq!(distance(3, 60.0, 250.0), Distance::Line(60.0));
        assert_eq!(distance(6, 60.0, 250.0), Distance::Line(120.0));
        for count in [30, u32::MAX] {
            assert_eq!(distance(count, 60.0, 250.0), Distance::Page(250.0));
        }
    }
}
