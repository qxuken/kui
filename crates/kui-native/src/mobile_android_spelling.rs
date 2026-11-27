//! Android text services return UTF-16 spans and complete their requests on a Looper.
use super::{AndroidApp, android_editor::with_activity};
use crate::system::spelling::{Dictionary, worker};
use jni::{
    Env, JavaVM,
    errors::Result,
    jni_sig, jni_str,
    objects::{JIntArray, JObject, JObjectArray, JString, JValue},
    refs::Global,
};
use kui_core::{Core, spelling::SpellingChoices};
use std::ops::Range;

pub(super) fn install(core: &mut Core, activity: AndroidApp) {
    core.edit
        .spelling
        .install_default(move || worker(move || AndroidDictionary::open(&activity).ok()));
}

struct AndroidDictionary {
    vm: JavaVM,
    helper: Global<JObject<'static>>,
}

impl AndroidDictionary {
    fn open(activity: &AndroidApp) -> Result<Self> {
        with_activity(activity, |env, activity| {
            let helper = env
                .call_method(
                    activity,
                    jni_str!("createKuiSpelling"),
                    jni_sig!("()Ldev/kui/KuiSpelling;"),
                    &[],
                )?
                .l()?;
            Ok(Self {
                vm: env.get_java_vm()?,
                helper: env.new_global_ref(helper)?,
            })
        })
    }

    fn with_env<T>(&self, f: impl FnOnce(&mut Env, &JObject) -> Result<T>) -> Result<T> {
        self.vm
            .attach_current_thread(|env| f(env, self.helper.as_ref()))
    }
}

impl Dictionary for AndroidDictionary {
    fn check(&self, text: &str) -> Vec<Range<usize>> {
        let offsets = self
            .with_env(|env, helper| {
                let text = JString::from_str(env, text)?;
                let array = env
                    .call_method(
                        helper,
                        jni_str!("check"),
                        jni_sig!("(Ljava/lang/String;)[I"),
                        &[JValue::Object(text.as_ref())],
                    )?
                    .l()?;
                let array = JIntArray::cast_local(env, array)?;
                let mut offsets = vec![0; array.len(env)?];
                array.get_region(env, 0, &mut offsets)?;
                Ok(offsets)
            })
            .unwrap_or_default();
        if offsets.is_empty() {
            return Vec::new();
        }
        let mut boundaries = Vec::with_capacity(text.len() + 1);
        for (byte, ch) in text.char_indices() {
            boundaries.push(byte);
            if ch.len_utf16() == 2 {
                boundaries.push(usize::MAX);
            }
        }
        boundaries.push(text.len());
        offsets
            .chunks_exact(2)
            .filter_map(|pair| {
                let &start = boundaries.get(pair[0] as usize)?;
                let &end = boundaries.get(pair[1] as usize)?;
                (start < end && end <= text.len()).then_some(start..end)
            })
            .collect()
    }

    fn choices(&self, word: &str) -> SpellingChoices {
        self.with_env(|env, helper| {
            let word = JString::from_str(env, word)?;
            let items = env
                .call_method(
                    helper,
                    jni_str!("choices"),
                    jni_sig!("(Ljava/lang/String;)[Ljava/lang/String;"),
                    &[JValue::Object(word.as_ref())],
                )?
                .l()?;
            if items.is_null() {
                return Ok(SpellingChoices::default());
            }
            let items = JObjectArray::<JString>::cast_local(env, items)?;
            let suggestions = (0..items.len(env)?)
                .map(|i| items.get_element(env, i)?.try_to_string(env))
                .collect::<Result<_>>()?;
            Ok(SpellingChoices {
                suggestions,
                can_learn: true,
            })
        })
        .unwrap_or_default()
    }

    fn learn(&mut self, word: &str) -> bool {
        self.with_env(|env, helper| {
            let word = JString::from_str(env, word)?;
            env.call_method(
                helper,
                jni_str!("learn"),
                jni_sig!("(Ljava/lang/String;)Z"),
                &[JValue::Object(word.as_ref())],
            )?
            .z()
        })
        .unwrap_or(false)
    }
}

impl Drop for AndroidDictionary {
    fn drop(&mut self) {
        let _ = self.with_env(|env, helper| {
            env.call_method(helper, jni_str!("close"), jni_sig!("()V"), &[])?;
            Ok(())
        });
    }
}
