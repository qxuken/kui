//! Acquire the next drawable without blocking AppKit input delivery.
use crate::access_bridge::UserEvent;
use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_quartz_core::{CAMetalDrawable, CAMetalLayer};
use std::sync::{Arc, Mutex, mpsc};
use winit::event_loop::EventLoopProxy;
use winit::window::Window;

pub(crate) struct Drawable(pub(crate) Retained<ProtocolObject<dyn CAMetalDrawable>>);
// SAFETY: Metal drawables and CAMetalLayer support acquisition/presentation on
// different threads. Ownership crosses once; no encoding happens in the worker.
unsafe impl Send for Drawable {}
struct Layer(Retained<CAMetalLayer>);
// SAFETY: the worker only calls the thread-safe nextDrawable API. AppKit view
// and window queries remain on the native event-loop thread.
unsafe impl Send for Layer {}

pub(crate) struct Acquire {
    ready: Arc<Mutex<Slot<Drawable>>>,
    ask: mpsc::SyncSender<()>,
    layer: Retained<CAMetalLayer>,
}
struct Slot<T> {
    // A completed attempt can time out. Keep that outcome for the runner.
    outcome: Option<Option<T>>,
    pending: bool,
    waiting: bool,
}
impl<T> Default for Slot<T> {
    fn default() -> Self {
        Self {
            outcome: None,
            pending: false,
            waiting: false,
        }
    }
}
impl<T> Slot<T> {
    fn request(&mut self) -> bool {
        if self.pending || self.ready() {
            return false;
        }
        self.pending = true;
        true
    }
    fn complete(&mut self, drawable: Option<T>) -> bool {
        self.outcome = Some(drawable);
        self.pending = false;
        self.waiting
    }
    fn ready(&self) -> bool {
        self.outcome.is_some()
    }
    fn take(&mut self) -> Option<Option<T>> {
        self.waiting = false;
        self.outcome.take()
    }
}
impl Acquire {
    pub(crate) fn new(
        renderer: &kui_wgpu::Renderer,
        window: &Arc<Window>,
        proxy: EventLoopProxy<UserEvent>,
    ) -> Option<Self> {
        let layer = renderer.metal_layer()?;
        layer.setAllowsNextDrawableTimeout(true);
        let worker_layer = Layer(layer.clone());
        let (ask, requests) = mpsc::sync_channel(1);
        let ready = Arc::new(Mutex::new(Slot::default()));
        let answer = ready.clone();
        let window = window.id();
        unsafe extern "C" {
            fn qos_class_self() -> u32;
            fn pthread_set_qos_class_self_np(class: u32, priority: i32) -> i32;
        }
        let qos = unsafe { qos_class_self() };
        std::thread::Builder::new()
            .name("kui drawable".into())
            .spawn(move || {
                // The drawable wake-up serves the same UI deadline as its
                // caller, rather than a lower-priority background task.
                unsafe {
                    pthread_set_qos_class_self_np(qos, 0);
                }
                // Capture the wrapper, not its non-Send Objective-C field.
                fn acquire(layer: &Layer) -> Option<Drawable> {
                    objc2::rc::autoreleasepool(|_| layer.0.nextDrawable().map(Drawable))
                }
                while requests.recv().is_ok() {
                    let value = acquire(&worker_layer);
                    let notify = {
                        let mut slot = answer.lock().unwrap();
                        slot.complete(value)
                    };
                    if notify && proxy.send_event(UserEvent::DrawableReady(window)).is_err() {
                        break;
                    }
                }
            })
            .ok()?;
        let this = Self { ready, ask, layer };
        this.request();
        Some(this)
    }
    pub(crate) fn request(&self) {
        let mut slot = self.ready.lock().unwrap();
        if slot.request() {
            // wgpu reconfiguration resets this property. Restore bounded
            // acquisition on the UI thread before each new request.
            self.layer.setAllowsNextDrawableTimeout(true);
            if self.ask.try_send(()).is_err() {
                slot.pending = false;
            }
        }
    }
    pub(crate) fn admit(&self) -> bool {
        let mut slot = self.ready.lock().unwrap();
        if slot.ready() {
            return true;
        }
        slot.waiting = true;
        drop(slot);
        self.request();
        false
    }
    pub(crate) fn ready(&self) -> bool {
        self.ready.lock().unwrap().ready()
    }
    pub(crate) fn take(&self) -> Option<Option<Drawable>> {
        self.ready.lock().unwrap().take()
    }
}

#[cfg(test)]
mod tests {
    use super::Slot;

    #[test]
    fn a_timeout_is_an_admitted_attempt() {
        let mut slot = Slot::<u8>::default();
        assert!(slot.request());
        slot.waiting = true;
        assert!(slot.complete(None), "wake the frame waiting on the worker");
        assert!(slot.ready(), "consume the timeout as a skipped frame");
        assert_eq!(slot.take(), Some(None));
        assert!(!slot.ready());
        assert!(slot.request(), "the runner can schedule a new attempt");
    }

    #[test]
    fn prefetch_is_bounded_and_does_not_wake_an_idle_window() {
        let mut slot = Slot::default();
        assert!(slot.request());
        assert!(!slot.request(), "only one acquisition in flight");
        assert!(
            !slot.complete(Some(1)),
            "idle prefetch never requests a frame"
        );
        assert!(!slot.request(), "one ready drawable is enough");
        assert_eq!(slot.take(), Some(Some(1)));
        assert!(slot.request());
    }

    #[test]
    fn waiting_frame_is_notified_and_timeout_can_retry() {
        let mut slot = Slot::<u8> {
            waiting: true,
            ..Default::default()
        };
        assert!(slot.request());
        assert!(slot.complete(None));
        assert!(!slot.request(), "consume the result before another attempt");
        assert_eq!(slot.take(), Some(None));
        assert!(slot.request());
        assert!(!slot.complete(Some(2)), "no frame waits for prefetch");
        assert_eq!(slot.take(), Some(Some(2)));
        assert!(!slot.waiting);
        assert!(slot.request());
        assert!(
            !slot.complete(Some(3)),
            "a completed frame clears its wake request"
        );
    }
}
