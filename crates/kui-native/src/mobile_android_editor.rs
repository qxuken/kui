//! Bridge the focused KUI editor to Android's input-method service.
use super::Proxy;
use super::{AndroidApp, Document, MobileEvent, NativeText};
use jni::{
    Env, EnvUnowned, NativeMethod,
    errors::Result,
    jni_sig, jni_str,
    objects::{JObject, JString, JValue},
    refs::Global,
};

pub(super) fn with_activity<T>(
    app: &AndroidApp,
    f: impl FnOnce(&mut Env, &JObject) -> Result<T>,
) -> Result<T> {
    // AndroidApp owns both references across this attached JNI frame.
    let vm = unsafe { jni::JavaVM::from_raw(app.vm_as_ptr().cast()) };
    vm.attach_current_thread(|env| {
        let raw = app.activity_as_ptr().cast();
        let activity = unsafe { env.as_cast_raw::<Global<JObject>>(&raw)? };
        f(env, activity.as_ref())
    })
}

pub(super) struct Editor(AndroidApp);

impl Editor {
    pub(super) fn new(app: &AndroidApp, proxy: Proxy) -> Result<Self> {
        with_activity(app, |env, activity| {
            let class = env.get_object_class(activity)?;
            // Signature matches the instance method on KuiActivity. Only this
            // module accesses its private long as a Proxy, until Editor drops.
            unsafe {
                let method = NativeMethod::from_raw_parts(
                    jni_str!("updateText"),
                    jni_str!("(JLjava/lang/String;IIII)V"),
                    update_text as *mut _,
                );
                env.register_native_methods(&class, &[method])?;
                env.set_rust_field(activity, jni_str!("kuiEvents"), proxy)
            }
        })?;
        Ok(Self(app.clone()))
    }

    pub(super) fn set(
        &self,
        session: i64,
        input_type: i32,
        doc: &Document,
        show: bool,
        refresh: bool,
    ) {
        if let Err(e) = with_activity(&self.0, |env, activity| {
            let value = JString::from_str(env, &doc.text)?;
            env.call_method(
                activity,
                jni_str!("setKuiEditor"),
                jni_sig!("(JILjava/lang/String;IIZZ)V"),
                &[
                    JValue::Long(session),
                    JValue::Int(input_type),
                    JValue::Object(value.as_ref()),
                    JValue::Int(doc.selection.0),
                    JValue::Int(doc.selection.1),
                    JValue::Bool(show),
                    JValue::Bool(refresh),
                ],
            )?;
            Ok(())
        }) {
            eprintln!("kui: updating Android editor: {e}");
        }
    }
}

impl Drop for Editor {
    fn drop(&mut self) {
        let _ = with_activity(&self.0, |env, activity| unsafe {
            env.take_rust_field::<_, _, Proxy>(activity, jni_str!("kuiEvents"))
        });
    }
}

extern "system" fn update_text(
    mut raw: EnvUnowned,
    activity: JObject,
    session: i64,
    text: JString,
    start: i32,
    end: i32,
    mark_start: i32,
    mark_end: i32,
) {
    raw.with_env(|env| -> Result<()> {
        let value = text.try_to_string(env)?;
        if let Ok(proxy) =
            unsafe { env.get_rust_field::<_, _, Proxy>(&activity, jni_str!("kuiEvents")) }
        {
            let event = NativeText {
                text: value,
                selection: (start, end),
                marked: (mark_start, mark_end),
            };
            let _ = proxy.send_event(MobileEvent::Ime(session, event));
        }
        Ok(())
    })
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}
