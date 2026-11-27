//! Native text-service regression. The visible screen uses the regular widgets,
//! model change messages and host. Only the input driver is test-specific.
use kui_native::{Core, Key, mobile::Waker};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

// UIKit decides which words are misspelled and which corrections it supports.
// Complete sentences also distinguish a later-word offset from a UTF-16 offset.
#[cfg(target_os = "ios")]
const SPELLING_SAMPLE: &str = "🙂 Please check the speling in this sentance. I recieved a mesage from my freind becuase the adress was definately wrong. We should accomodate their seperate requests tommorow and fix the erorrs.";

#[cfg(target_os = "ios")]
#[derive(Default)]
struct SpellingFixture {
    word: String,
    guess: String,
    learn: String,
    revision: u64,
    initial_pending: bool,
}

#[derive(Default)]
pub struct Regression {
    pub fields: [Option<Key>; 2],
    pub changes: [usize; 2],
    step: i32,
    since: Option<Instant>,
    #[cfg(target_os = "ios")]
    awaiting_teardown: bool,
    #[cfg(target_os = "ios")]
    spelling: SpellingFixture,
    pub result: PathBuf,
    #[cfg(target_os = "android")]
    pub activity: Option<kui_native::mobile::AndroidApp>,
}

impl Regression {
    pub fn ime_off(&self) -> bool {
        self.step == 22
    }

    #[cfg(target_os = "ios")]
    pub fn teardown(&mut self) {
        if self.awaiting_teardown {
            self.finish("PASS emoji replacement, caret and field switch; IME-off canceled composition and the editor reopened; native spelling ranges, suggestions and learning; UIKit termination called App::teardown".into());
        }
    }

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
        let timeout = if cfg!(target_os = "ios") && self.step == 24 {
            24
        } else {
            12
        };
        if self.since.get_or_insert_with(Instant::now).elapsed() > Duration::from_secs(timeout) {
            #[cfg(target_os = "android")]
            if (24..=26).contains(&self.step) {
                self.finish(format!(
                    "FAIL spelling step {}: ranges {:?}, dictionary revision {}",
                    self.step,
                    core.edit
                        .spelling
                        .check("🙂 kuizzzword кошка")
                        .map(<[_]>::to_vec),
                    core.edit.spelling.dictionary_revision()
                ));
                return;
            }
            #[cfg(target_os = "ios")]
            if (24..=27).contains(&self.step) {
                let ranges = core.edit.spelling.check(SPELLING_SAMPLE).map(<[_]>::to_vec);
                let choices = core.edit.spelling.choices(&self.spelling.word);
                self.finish(format!("FAIL spelling step {} for {SPELLING_SAMPLE:?}: worker ranges {ranges:?}, direct UIKit {:?}, choices for {:?} {choices:?}, learned {:?}, generation {}, initial pending {}, result revision {}, {}", self.step, native_spelling_ranges(SPELLING_SAMPLE),
                    self.spelling.word, self.spelling.learn, core.edit.spelling.dictionary_revision(), self.spelling.initial_pending, core.edit.spelling.revision(), spelling_diagnostics(SPELLING_SAMPLE)));
                return;
            }
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
                #[cfg(target_os = "ios")]
                {
                    self.step = 19;
                    true
                }
                #[cfg(target_os = "android")]
                {
                    core.set_focus(Some(b));
                    true
                }
            }
            #[cfg(target_os = "android")]
            9 => self.native(6),
            #[cfg(target_os = "android")]
            10 => second == "second" && marked == Some("sec") && self.native(7),
            #[cfg(target_os = "android")]
            11 if second == "second" && marked == Some("日本") && self.changes == [3, 0] => {
                core.set_focus(Some(a));
                true
            }
            #[cfg(target_os = "android")]
            12 if second == "second" && marked.is_none() && self.native(8) => {
                core.set_focus(Some(b));
                true
            }
            #[cfg(target_os = "android")]
            13 => self.native(9),
            #[cfg(target_os = "android")]
            14 => second == "second" && marked == Some("日本") && self.native(10),
            #[cfg(target_os = "android")]
            15 if second == "日本ond"
                && core.edit.caret_and_selection(b) == Some((6, None))
                && marked.is_none()
                && self.changes == [3, 1] =>
            {
                core.set_focus(Some(a));
                self.step = 19;
                true
            }
            20 => self.native(11),
            21 => first == "aé!Zb" && core.edit.preedit(a) == Some("か"),
            22 => first == "aé!Zb" && core.edit.preedit(a).is_none() && self.native(12),
            23 if self.native(13) && first == "aé!Zb" && self.changes[0] == 3 => {
                #[cfg(target_os = "ios")]
                {
                    unsafe extern "C" {
                        fn kui_probe_spelling_scripts() -> bool;
                    }
                    if !unsafe { kui_probe_spelling_scripts() } {
                        self.finish(
                            "FAIL mixed-script spelling with an unavailable dictionary".into(),
                        );
                        return;
                    }
                    self.spelling.initial_pending = core
                        .edit
                        .spelling
                        .check("🙂 This has a speling mistake.")
                        .is_none();
                    true
                }
                #[cfg(target_os = "android")]
                {
                    self.native(14)
                }
            }
            #[cfg(target_os = "ios")]
            24 => {
                let native = native_spelling_ranges(SPELLING_SAMPLE);
                !native.is_empty()
                    && core
                        .edit
                        .spelling
                        .check(SPELLING_SAMPLE)
                        .is_some_and(|ranges| *ranges == native)
                    && self.spelling.select(&native)
            }
            #[cfg(target_os = "ios")]
            25 => core
                .edit
                .spelling
                .choices(&self.spelling.word)
                .is_some_and(|c| c.can_learn && c.suggestions.contains(&self.spelling.guess)),
            #[cfg(target_os = "ios")]
            26 => {
                self.spelling.revision = core.edit.spelling.dictionary_revision();
                core.edit.spelling.learn(&self.spelling.learn);
                true
            }
            #[cfg(target_os = "ios")]
            27 if core.edit.spelling.dictionary_revision() != self.spelling.revision
                && core
                    .edit
                    .spelling
                    .check(SPELLING_SAMPLE)
                    .is_some_and(|ranges| {
                        *ranges == native_spelling_ranges(SPELLING_SAMPLE)
                            && ranges.iter().all(|r| {
                                SPELLING_SAMPLE.get(r.clone()) != Some(self.spelling.learn.as_str())
                            })
                            && ranges.iter().any(|r| {
                                SPELLING_SAMPLE.get(r.clone()) == Some(self.spelling.word.as_str())
                            })
                    }) =>
            {
                unsafe extern "C" {
                    fn kui_probe_unlearn(bytes: *const u8, len: usize) -> bool;
                }
                // Restore only the word confirmed unlearned during fixture selection.
                if !unsafe {
                    kui_probe_unlearn(self.spelling.learn.as_ptr(), self.spelling.learn.len())
                } {
                    self.finish(
                        "FAIL UIKit personal dictionary did not learn the selected word".into(),
                    );
                    return;
                }
                self.awaiting_teardown = true;
                self.native(14)
            }
            #[cfg(target_os = "android")]
            24 => core
                .edit
                .spelling
                .check("🙂 kuizzzword кошка")
                .is_some_and(|ranges| ranges.len() == 1 && ranges[0] == (5..15)),
            #[cfg(target_os = "android")]
            25 if core
                .edit
                .spelling
                .choices("kuizzzword")
                .is_some_and(|actions| actions.can_learn && actions.suggestions == ["quiz"]) =>
            {
                core.edit.spelling.learn("kuizzzword");
                true
            }
            #[cfg(target_os = "android")]
            26 if core.edit.spelling.dictionary_revision() > 0
                && core
                    .edit
                    .spelling
                    .check("🙂 kuizzzword кошка")
                    .is_some_and(|ranges| ranges.is_empty()) =>
            {
                self.finish("PASS emoji replacement, caret and field switch, composition, IME-off and reopen, Android native spelling offsets, suggestions and learning".into());
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

#[cfg(target_os = "ios")]
fn spelling_diagnostics(text: &str) -> String {
    use std::ffi::{CStr, c_char, c_void};
    unsafe extern "C" {
        fn kui_probe_spelling(
            bytes: *const u8,
            len: usize,
            ctx: *mut c_void,
            report: extern "C" fn(*mut c_void, *const c_char),
        );
    }
    extern "C" fn report(ctx: *mut c_void, value: *const c_char) {
        let value = unsafe { CStr::from_ptr(value) }.to_string_lossy();
        unsafe { &mut *ctx.cast::<String>() }.push_str(&value);
    }
    let mut value = String::new();
    unsafe { kui_probe_spelling(text.as_ptr(), text.len(), (&raw mut value).cast(), report) };
    value
}

#[cfg(target_os = "ios")]
fn native_spelling_ranges(text: &str) -> Vec<std::ops::Range<usize>> {
    use std::{ffi::c_void, ops::Range};
    unsafe extern "C" {
        fn kui_probe_expected_ranges(
            text: *const u8,
            len: usize,
            ctx: *mut c_void,
            emit: extern "C" fn(*mut c_void, usize, usize),
        );
    }
    extern "C" fn emit(ctx: *mut c_void, start: usize, end: usize) {
        unsafe { &mut *ctx.cast::<Vec<Range<usize>>>() }.push(start..end);
    }
    let mut ranges = Vec::new();
    unsafe { kui_probe_expected_ranges(text.as_ptr(), text.len(), (&raw mut ranges).cast(), emit) }
    ranges
}

#[cfg(target_os = "ios")]
impl SpellingFixture {
    fn select(&mut self, ranges: &[std::ops::Range<usize>]) -> bool {
        use std::ffi::{CStr, c_char, c_void};
        unsafe extern "C" {
            fn kui_probe_guess(
                bytes: *const u8,
                len: usize,
                ctx: *mut c_void,
                emit: extern "C" fn(*mut c_void, *const c_char),
            ) -> bool;
        }
        extern "C" fn emit(ctx: *mut c_void, text: *const c_char) {
            let value = unsafe { CStr::from_ptr(text) }.to_string_lossy();
            unsafe { &mut *ctx.cast::<String>() }.push_str(&value);
        }
        for range in ranges {
            let Some(word) = SPELLING_SAMPLE.get(range.clone()) else {
                continue;
            };
            if !word.chars().all(char::is_alphabetic) || word == self.word {
                continue;
            }
            let mut guess = String::new();
            let can_learn = unsafe {
                kui_probe_guess(word.as_ptr(), word.len(), (&raw mut guess).cast(), emit)
            };
            if !can_learn {
                continue;
            }
            if self.word.is_empty() && !guess.is_empty() {
                self.word = word.to_owned();
                self.guess = guess;
            } else if !self.word.is_empty() {
                self.learn = word.to_owned();
                return true;
            }
        }
        false
    }
}
