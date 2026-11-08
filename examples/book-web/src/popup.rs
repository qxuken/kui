//! The book hosts one non-activating child overlay over its owner canvas.
use std::cell::RefCell;

use kui_core::{
    AccessAction, AccessRequest, InputEvent, Key, KeyPress, MouseButton, Size, Vec2, WindowCommand,
    WindowId, WindowKind, window::DismissReason,
};
use kui_web::CanvasPopup;
use serde_json::json;

use crate::host::{RUNNER, publish_browser_bytes};

thread_local! {
    static POPUP: RefCell<Option<CanvasPopup>> = const { RefCell::new(None) };
}

#[unsafe(no_mangle)]
pub extern "C" fn demo_popup_changes() -> usize {
    RUNNER.with_borrow_mut(|runner| {
        POPUP.with_borrow_mut(|popup| {
            let mut change = None;
            for command in runner.core_mut().take_window_commands() {
                match command {
                    WindowCommand::Open {
                        id,
                        owner: WindowId::MAIN,
                        config,
                        ..
                    } if config.kind == WindowKind::Popup
                        && !config.activates
                        && popup.is_none() =>
                    {
                        *popup = Some(runner.open_popup(id));
                        let a = config.anchor;
                        change = Some(json!({
                            "anchor": [a.x, a.y, a.w, a.h],
                            "width": config.size.w, "height": config.size.h,
                        }));
                    }
                    WindowCommand::Close(id) if popup.as_ref().is_some_and(|p| p.id() == id) => {
                        *popup = None;
                        change = Some(serde_json::Value::Null);
                    }
                    _ => {}
                }
            }
            change.map_or(0, |state| {
                publish_browser_bytes(state.to_string().into_bytes())
            })
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn demo_popup_frame(w: f32, h: f32, scale: f32, time: f64, redraw: u32) -> u32 {
    RUNNER.with_borrow_mut(|runner| {
        POPUP.with_borrow_mut(|popup| {
            u32::from(popup.as_mut().is_some_and(|popup| {
                runner.popup_frame(popup, Size::new(w, h), scale, time, redraw != 0)
            }))
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn demo_popup_pointer(kind: u32, x: f32, y: f32, time: f64) -> u32 {
    let event = match kind {
        0 => InputEvent::CursorMoved(Vec2::new(x, y)),
        1 => InputEvent::CursorLeft,
        2 => InputEvent::MouseDown {
            button: MouseButton::Primary,
            clicks: 1,
        },
        3 => InputEvent::MouseUp {
            button: MouseButton::Primary,
        },
        _ => return 0,
    };
    RUNNER.with_borrow_mut(|runner| {
        POPUP.with_borrow_mut(|popup| {
            let Some(popup) = popup else { return 0 };
            runner.popup_input(popup, event, time);
            u32::from(popup.pointer_cursor())
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn demo_popup_dismiss(escape: u32, time: f64) {
    let reason = if escape == 0 {
        DismissReason::Outside
    } else {
        DismissReason::Escape
    };
    RUNNER.with_borrow_mut(|runner| {
        POPUP.with_borrow_mut(|popup| {
            if let Some(popup) = popup {
                runner.dismiss_popup(popup, reason, time);
            }
        })
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn demo_popup_activate(key: u64, time: f64) {
    RUNNER.with_borrow_mut(|runner| {
        POPUP.with_borrow_mut(|popup| {
            if let Some(popup) = popup {
                runner.popup_input(
                    popup,
                    InputEvent::Access(AccessRequest::new(Key(key), AccessAction::Click)),
                    time,
                );
            }
        })
    });
}

pub fn key_event(key: &KeyPress, time: f64, down: bool) -> bool {
    RUNNER.with_borrow_mut(|runner| {
        POPUP.with_borrow_mut(|popup| {
            popup
                .as_mut()
                .is_some_and(|popup| runner.popup_key(popup, key, time, down))
        })
    })
}
