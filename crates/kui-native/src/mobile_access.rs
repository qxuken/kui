//! Each window publishes KUI's semantics through its platform's AccessKit adapter.
use super::MobileEvent;
use super::Proxy;
use kui_access::accesskit::{ActionHandler, ActionRequest, ActivationHandler, TreeUpdate};
use kui_core::access::{AccessSnapshot, Announcement};

#[derive(Clone)]
struct Handler(Proxy);

impl ActivationHandler for Handler {
    fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
        let _ = self.0.send_event(MobileEvent::AccessActive(true));
        None
    }
}
impl ActionHandler for Handler {
    fn do_action(&mut self, action: ActionRequest) {
        if let Some(request) = kui_access::request_of(action) {
            let _ = self.0.send_event(MobileEvent::Access(request));
        }
    }
}
#[cfg(target_os = "ios")]
impl kui_access::accesskit::DeactivationHandler for Handler {
    fn deactivate_accessibility(&mut self) {
        let _ = self.0.send_event(MobileEvent::AccessActive(false));
    }
}

#[cfg(target_os = "ios")]
pub(super) type Adapter = accesskit_ios::SubclassingAdapter;
#[cfg(target_os = "android")]
pub(super) type Adapter = accesskit_android::InjectingAdapter;

#[cfg(target_os = "ios")]
pub(super) fn attach(
    window: &super::Window,
    proxy: Proxy,
) -> Result<Adapter, Box<dyn std::error::Error>> {
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let RawWindowHandle::UiKit(view) = window.window_handle()?.as_raw() else {
        return Err("missing UIKit accessibility view".into());
    };
    let handler = Handler(proxy);
    // winit owns the UIView on the main thread for the adapter's lifetime.
    Ok(unsafe {
        Adapter::new(
            view.ui_view.as_ptr(),
            handler.clone(),
            handler.clone(),
            handler,
        )
    })
}

#[cfg(target_os = "android")]
pub(super) fn attach(
    app: &super::AndroidApp,
    proxy: Proxy,
) -> Result<Adapter, Box<dyn std::error::Error>> {
    use accesskit_android::jni::{
        JNIEnv, JavaVM, NativeMethod,
        objects::{JObject, JValue},
        sys::jlong,
    };
    struct Request {
        handler: Handler,
        adapter: Option<Adapter>,
    }
    extern "system" fn install(mut env: JNIEnv, _: JObject, request: jlong, host: JObject) {
        // createKuiAccess forwards the borrowed request synchronously, once.
        let request = unsafe { &mut *(request as *mut Request) };
        request.adapter = Some(Adapter::new(
            &mut env,
            &host,
            request.handler.clone(),
            request.handler.clone(),
        ));
    }
    let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast())? };
    let activity = unsafe { JObject::from_raw(app.activity_as_ptr().cast()) };
    let mut env = vm.attach_current_thread()?;
    let mut request = Request {
        handler: Handler(proxy),
        adapter: None,
    };
    env.with_local_frame(8, |env| -> accesskit_android::jni::errors::Result<()> {
        let class = env.get_object_class(&activity)?;
        env.register_native_methods(
            class,
            &[NativeMethod {
                name: "attachKuiAccess".into(),
                sig: "(JLandroid/view/View;)V".into(),
                fn_ptr: install as *mut _,
            }],
        )?;
        env.call_method(
            &activity,
            "createKuiAccess",
            "(J)V",
            &[JValue::Long(&mut request as *mut Request as jlong)],
        )?;
        Ok(())
    })?;
    request
        .adapter
        .ok_or_else(|| "Android accessibility adapter was not installed".into())
}

#[cfg(target_os = "ios")]
pub(super) fn publish(
    feed: &mut kui_access::Feed,
    adapter: &mut Adapter,
    tree: AccessSnapshot<'_>,
    scale: f32,
    announcements: &[Announcement],
) {
    unsafe extern "C" {
        fn kui_ios_announce(bytes: *const u8, length: usize, assertive: bool);
    }

    feed.publish_snapshot(tree, scale, &[], |update| {
        if let Some(events) = adapter.update_if_active(|| update) {
            events.raise();
        }
    });
    // AccessKit can post a layout change when it publishes the updated count.
    // Submit one-off speech after that notification and keep it out of the tree.
    for announcement in announcements {
        let text = announcement.text.as_bytes();
        unsafe {
            kui_ios_announce(
                text.as_ptr(),
                text.len(),
                announcement.live == kui_core::access::Live::Assertive,
            );
        }
    }
}

#[cfg(target_os = "android")]
pub(super) fn publish(
    feed: &mut kui_access::Feed,
    adapter: &mut Adapter,
    tree: AccessSnapshot<'_>,
    scale: f32,
    announcements: &[Announcement],
) {
    feed.publish_snapshot(tree, scale, announcements, |update| {
        adapter.update_if_active(|| update);
    });
}
