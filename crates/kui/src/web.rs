//! The browser (backlog F87): what the runner needs from a page that the
//! desktop platforms give it some other way.
//!
//! winit's web backend is the loop — the page's events arrive as
//! `WindowEvent`s on a `<canvas>` it makes — so most of the runner is the
//! same code. What differs is here: the clipboard a page can hold
//! synchronously, and making the renderer, which on a page is a promise
//! the loop cannot block on.

/// The clipboard, as far as a page can hold it synchronously.
///
/// The browser's clipboard is asynchronous and asks permission to be read,
/// and the runner's paste answers in the turn it was asked. So a copy is
/// kept here, in the page, and also written through to
/// `navigator.clipboard` for the other apps on the machine — a write made
/// while a key is down is a user gesture, which is what the browser asks
/// of one. A paste reads the copy kept here: text copied in another app
/// does not reach a kui page yet.
///
/// Named and shaped as the part of `arboard` the runner calls, so the
/// runner's clipboard code is one code on every platform.
pub(crate) mod clipboard {
    #[derive(Default)]
    pub(crate) struct Clipboard {
        text: Option<String>,
    }

    impl Clipboard {
        pub(crate) fn new() -> Result<Self, ()> {
            Ok(Self::default())
        }

        pub(crate) fn set_text(&mut self, text: String) -> Result<(), ()> {
            write_through(&text);
            self.text = Some(text);
            Ok(())
        }

        /// The words, as a page's clipboard write takes them: the HTML
        /// flavour needs a `ClipboardItem`, and every paste into a kui
        /// page reads the words anyway.
        pub(crate) fn set_html(&mut self, _html: String, text: Option<String>) -> Result<(), ()> {
            self.set_text(text.unwrap_or_default())
        }

        pub(crate) fn get_text(&mut self) -> Result<String, ()> {
            self.text.clone().ok_or(())
        }
    }

    fn write_through(text: &str) {
        let Some(window) = web_sys::window() else {
            return;
        };
        let written = window.navigator().clipboard().write_text(text);
        // Refused (no focus, no permission) is no error of the app's: the
        // copy kept above still pastes inside the page. Awaited only so
        // the refusal is handled rather than reported as uncaught.
        wasm_bindgen_futures::spawn_local(async move {
            let _ = wasm_bindgen_futures::JsFuture::from(written).await;
        });
    }
}

use std::cell::RefCell;
use std::sync::Arc;

use winit::event_loop::EventLoopProxy;
use winit::window::Window;

use crate::access_bridge::UserEvent;

/// A window and its renderer, or why the renderer could not be made.
type Made = (Arc<Window>, Result<kui_wgpu::Renderer, String>);

thread_local! {
    /// The main window's renderer, made and not yet taken: the task that
    /// made it cannot reach the shell, so it leaves it here and wakes the
    /// loop, whose `user_event` opens the window with it.
    static MADE: RefCell<Option<Made>> = const { RefCell::new(None) };
}

/// Makes `window`'s renderer in a task, the adapter and device being
/// promises on a page, and wakes the loop through `proxy` when it is made
/// (or cannot be).
pub(crate) fn make_renderer(window: Arc<Window>, proxy: Option<EventLoopProxy<UserEvent>>) {
    wasm_bindgen_futures::spawn_local(async move {
        let px = window.inner_size();
        let made = kui_wgpu::Renderer::new(window.clone(), px.width, px.height)
            .await
            .map_err(|e| e.to_string());
        MADE.with(|m| *m.borrow_mut() = Some((window, made)));
        if let Some(proxy) = proxy {
            let _ = proxy.send_event(UserEvent::Wake);
        }
    });
}

/// The renderer `make_renderer` left, once.
pub(crate) fn take_renderer() -> Option<Made> {
    MADE.with(|m| m.borrow_mut().take())
}

/// Says `what` on the page's console, where a desktop runner writes to
/// stderr — which on a page goes nowhere.
pub(crate) fn say(what: &str) {
    web_sys::console::error_1(&what.into());
}

/// A window on a page is a `<canvas>`, appended to the body, sized by the
/// page's stylesheet rather than by the size a desktop window opens at —
/// a page lays out its own elements — and shown at once: a desktop window
/// is hidden until its first frame, which a canvas has no need of.
pub(crate) fn canvas_attrs(
    attrs: winit::window::WindowAttributes,
) -> winit::window::WindowAttributes {
    use winit::platform::web::WindowAttributesExtWebSys;
    let mut attrs = attrs.with_append(true).with_visible(true);
    attrs.inner_size = None;
    attrs
}
