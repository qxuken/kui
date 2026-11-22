//! Native text-service regression. The visible screen uses the regular widgets,
//! model change messages and host. Only the input driver is test-specific.
use kui_native::{Core, Key, mobile::Waker};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

#[derive(Default)]
pub struct Regression {
    pub fields: [Option<Key>; 2],
    pub changes: [usize; 2],
    step: i32,
    since: Option<Instant>,
    pub result: PathBuf,
    #[cfg(target_os = "android")]
    pub activity: Option<kui_native::mobile::AndroidApp>,
}

impl Regression {
    pub fn wake(&self, waker: Waker) {
        let result = self.result.clone();
        std::thread::spawn(move || {
            while !result.exists() {
                waker.wake();
                std::thread::sleep(Duration::from_millis(50));
            }
        });
    }

    pub fn advance(&mut self, core: &mut Core, first: &str, second: &str) {
        let [Some(a), Some(b)] = self.fields else {
            return;
        };
        if self.step < 0 {
            return;
        }
        let caret = core.edit.caret_and_selection(a);
        let marked = core.edit.preedit(b);
        if self.since.get_or_insert_with(Instant::now).elapsed() > Duration::from_secs(12) {
            self.finish(format!("FAIL step {} model {first:?}, {second:?}, caret {caret:?}, marked {marked:?}, focus {:?}, changes {:?}", self.step, core.focus(), self.changes));
            return;
        }
        let progressed = match self.step {
            0 => {
                core.set_focus(Some(a));
                true
            }
            1 => self.native(1),
            2 => caret.is_some_and(|(_, s)| s == Some((1, 5))) && self.native(2),
            3 => first == "aéb" && caret == Some((3, None)) && self.native(3),
            4 if first == "aé!b" && caret == Some((4, None)) => {
                core.set_focus(Some(b));
                true
            }
            5 => self.native(4),
            6 if second == "second" && marked == Some("か") && core.focus() == Some(b) => {
                core.set_focus(Some(a));
                true
            }
            7 => self.native(5),
            8 if first == "aé!Zb"
                && caret == Some((5, None))
                && second == "second"
                && marked.is_none()
                && core.focus() == Some(a)
                && self.changes == [3, 0] =>
            {
                self.finish("PASS replacement aéb at UTF-8 byte 3; final aé!Zb at byte 5; second unchanged; three changes".into());
                return;
            }
            _ => false,
        };
        if progressed {
            self.step += 1;
            self.since = Some(Instant::now());
        }
    }

    fn finish(&mut self, message: String) {
        std::fs::write(&self.result, message).unwrap();
        self.step = -1;
    }

    #[cfg(target_os = "ios")]
    fn native(&self, step: i32) -> bool {
        unsafe extern "C" {
            fn kui_probe_step(step: i32) -> bool;
        }
        unsafe { kui_probe_step(step) }
    }

    #[cfg(target_os = "android")]
    fn native(&self, step: i32) -> bool {
        use jni::{
            jni_sig, jni_str,
            objects::{JClass, JObject, JString, JValue},
            refs::Global,
        };
        let app = self.activity.as_ref().unwrap();
        let vm = unsafe { jni::JavaVM::from_raw(app.vm_as_ptr().cast()) };
        vm.attach_current_thread(|env| -> jni::errors::Result<bool> {
            let raw = app.activity_as_ptr().cast();
            let activity = unsafe { env.as_cast_raw::<Global<JObject>>(&raw)? };
            let loader = env
                .call_method(
                    &activity,
                    jni_str!("getClassLoader"),
                    jni_sig!("()Ljava/lang/ClassLoader;"),
                    &[],
                )?
                .l()?;
            let name = JString::from_str(env, "dev.kui.EditingProbe")?;
            let class = env
                .call_method(
                    &loader,
                    jni_str!("loadClass"),
                    jni_sig!("(Ljava/lang/String;)Ljava/lang/Class;"),
                    &[JValue::Object(name.as_ref())],
                )?
                .l()?;
            let class = env.cast_local::<JClass>(class)?;
            env.call_static_method(
                &class,
                jni_str!("step"),
                jni_sig!("(Landroid/app/Activity;I)Z"),
                &[JValue::Object(activity.as_ref()), JValue::Int(step)],
            )?
            .z()
        })
        .unwrap()
    }
}
