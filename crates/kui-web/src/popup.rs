//! A linked Canvas 2D surface. The browser host supplies its placement and
//! routes the owner's press into it as described in ADR 0009.

use super::{App, Runner, access, canvas};
use kui_core::{Core, CursorShape, InputEvent, KeyPress, Size, WindowId, window::DismissReason};

pub struct CanvasPopup {
    core: Core,
    painter: canvas::Painter,
    access: access::Mirror,
}

impl CanvasPopup {
    pub fn id(&self) -> WindowId {
        self.core.env.window.id
    }

    pub fn pointer_cursor(&self) -> bool {
        self.core.cursor_shape() == CursorShape::Pointer
    }
}

impl<A: App> Runner<A> {
    pub fn open_popup(&self, id: WindowId) -> CanvasPopup {
        let mut core = Core::new_in(self.core.session());
        core.env.window.id = id;
        core.set_platform(self.core.env_facts().platform);
        core.env.system = self.core.env.system;
        core.set_focused(true);
        core.set_native_menus(false);
        core.set_native_menu_bar(false);
        CanvasPopup {
            core,
            painter: canvas::Painter::default(),
            access: access::Mirror::default(),
        }
    }

    pub fn popup_frame(
        &mut self,
        popup: &mut CanvasPopup,
        viewport: Size,
        scale: f32,
        time: f64,
        redraw: bool,
    ) -> bool {
        if redraw {
            popup.painter.invalidate();
        }
        let next = Self::build_frame_for(
            &mut self.app,
            &mut self.extensions,
            &mut popup.core,
            viewport,
            scale,
            time,
        );
        popup.painter.paint(&mut popup.core);
        popup.access.publish(&mut popup.core);
        next
    }

    pub fn popup_input(&mut self, popup: &mut CanvasPopup, event: InputEvent, time: f64) {
        popup.core.set_time(time);
        let events = popup.core.handle_input(event);
        Self::route_to(&mut self.app, &mut self.extensions, &mut popup.core, events);
    }

    /// While the popup borrows the keyboard, the owner still gets
    /// releases for keys pressed before it opened.
    pub fn popup_key(
        &mut self,
        popup: &mut CanvasPopup,
        key: &KeyPress,
        time: f64,
        down: bool,
    ) -> bool {
        if !down && self.core.holds_key(key) {
            return false;
        }
        popup.core.set_time(time);
        let events = if down {
            popup.core.press(key.clone())
        } else {
            popup.core.release(key.clone())
        };
        Self::route_to(&mut self.app, &mut self.extensions, &mut popup.core, events);
        true
    }

    pub fn dismiss_popup(&mut self, popup: &mut CanvasPopup, reason: DismissReason, time: f64) {
        popup.core.set_time(time);
        popup.core.dismiss_window(popup.id(), reason);
        let events = popup.core.take_pending_events();
        Self::route_to(&mut self.app, &mut self.extensions, &mut popup.core, events);
    }
}
