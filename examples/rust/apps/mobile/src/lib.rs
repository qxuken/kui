#![cfg(any(target_os = "ios", target_os = "android"))]

#[cfg(all(feature = "candidate-trial", not(feature = "navigation-trial")))]
mod candidates;

#[cfg(feature = "navigation-trial")]
mod navigation;

#[cfg(not(any(feature = "candidate-trial", feature = "navigation-trial")))]
use kui_native::{App, Event, Message, NodeSpec, TextStyle, Ui, UiEvent, widgets};
use kui_native::{Core, mobile};

#[cfg(all(
    feature = "editing-regression",
    not(any(feature = "candidate-trial", feature = "navigation-trial"))
))]
mod editing_regression;

#[cfg(not(any(feature = "candidate-trial", feature = "navigation-trial")))]
#[derive(Default)]
struct Screen {
    #[cfg(feature = "editing-regression")]
    regression: editing_regression::Regression,
    drafts: Option<std::path::PathBuf>,
    taps: usize,
    note: String,
    bottom_note: String,
}

#[cfg(not(any(
    feature = "candidate-trial",
    feature = "editing-regression",
    feature = "navigation-trial"
)))]
impl Screen {
    fn restore(dir: Option<std::path::PathBuf>) -> Self {
        let mut screen = Self::default();
        if let Some(dir) = dir {
            if let Err(err) = std::fs::create_dir_all(&dir) {
                eprintln!("kui mobile: opening draft directory: {err}");
                return screen;
            }
            let read = |name| match std::fs::read_to_string(dir.join(name)) {
                Ok(text) => text,
                Err(err) => {
                    if err.kind() != std::io::ErrorKind::NotFound {
                        eprintln!("kui mobile: reading draft {name}: {err}");
                    }
                    String::new()
                }
            };
            screen.note = read("note.txt");
            screen.bottom_note = read("bottom.txt");
            screen.drafts = Some(dir);
        }
        screen
    }
}

#[cfg(any(not(feature = "candidate-trial"), feature = "navigation-trial"))]
fn save_draft(dir: Option<&std::path::Path>, name: &str, text: &str) {
    if let Some(dir) = dir {
        let pending = dir.join(format!("{name}.tmp"));
        if let Err(err) =
            std::fs::write(&pending, text).and_then(|()| std::fs::rename(pending, dir.join(name)))
        {
            eprintln!("kui mobile: saving draft {name}: {err}");
        }
    }
}

#[cfg(not(any(feature = "candidate-trial", feature = "navigation-trial")))]
#[derive(Message, Clone, Debug, PartialEq)]
enum Msg {
    Increment,
    Edit,
    EditBottom,
}

#[cfg(not(any(feature = "candidate-trial", feature = "navigation-trial")))]
impl App for Screen {
    fn view(&mut self, ui: &mut Ui<'_>) {
        let theme = ui.theme();
        ui.with(
            NodeSpec::column()
                .fill()
                .padding(ui.safe_area())
                .gap(12.)
                .bg(theme.bg),
            |ui| {
                ui.text(
                    &format!("Button taps: {}", self.taps),
                    TextStyle::new(26.).color(theme.fg),
                );
                ui.with(NodeSpec::column().fill().scroll_y().gap(16.), |ui| {
                    widgets::button(ui, "Increment", Msg::Increment);
                    let _field = widgets::text_input_with(
                        ui,
                        "Note",
                        &self.note,
                        true,
                        &widgets::InputMessages {
                            change: Some(Msg::Edit.into()),
                            ..Default::default()
                        },
                    );
                    #[cfg(feature = "editing-regression")]
                    {
                        self.regression.fields[0] = Some(_field);
                    }
                    #[cfg(not(feature = "editing-regression"))]
                    for row in 1..=48 {
                        ui.text(
                            &format!("List row {row}"),
                            TextStyle::new(20.).color(theme.fg),
                        );
                    }
                    ui.text("Bottom note", TextStyle::new(20.).color(theme.fg));
                    let _field = widgets::text_input_with(
                        ui,
                        "Bottom note",
                        &self.bottom_note,
                        true,
                        &widgets::InputMessages {
                            change: Some(Msg::EditBottom.into()),
                            ..Default::default()
                        },
                    );
                    #[cfg(feature = "editing-regression")]
                    {
                        self.regression.fields[1] = Some(_field);
                    }
                });
            },
        );
    }

    #[cfg(feature = "editing-regression")]
    fn setup(&mut self, waker: kui_native::mobile::Waker) {
        self.note = "a🙂b".into();
        self.bottom_note = "second".into();
        let _ = std::fs::remove_file(&self.regression.result);
        self.regression.wake(waker);
    }

    #[cfg(feature = "editing-regression")]
    fn before_frame(&mut self, core: &mut Core) {
        self.regression.advance(core, &self.note, &self.bottom_note);
    }

    fn on_event_with(&mut self, event: UiEvent, core: &mut Core) {
        match event.split::<Msg>() {
            (Some(Msg::Increment), _) => {
                use kui_native::access::Live;

                self.taps += 1;
                // VoiceOver can reread the focused button over queued speech.
                // The count confirms an explicit activation.
                let priority = if cfg!(target_os = "ios") {
                    Live::Assertive
                } else {
                    Live::Polite
                };
                core.announce(&format!("Button taps: {}", self.taps), priority);
            }
            (Some(Msg::Edit), Event::EditChange(change)) => {
                self.note = change.text;
                save_draft(self.drafts.as_deref(), "note.txt", &self.note);
                #[cfg(feature = "editing-regression")]
                {
                    self.regression.changes[0] += 1;
                }
            }
            (Some(Msg::EditBottom), Event::EditChange(change)) => {
                self.bottom_note = change.text;
                save_draft(self.drafts.as_deref(), "bottom.txt", &self.bottom_note);
                #[cfg(feature = "editing-regression")]
                {
                    self.regression.changes[1] += 1;
                }
            }
            _ => {}
        }
    }
}

mod fonts;

fn session() -> kui_native::Session {
    let font = include_bytes!("../../../../../crates/kui-cli/assets/fonts/NotoSans-Regular.ttf");
    fonts::session(font)
}

fn core() -> Core {
    Core::new_in(&session())
}

#[cfg(all(
    target_os = "ios",
    feature = "ipad-scenes",
    not(any(
        feature = "candidate-trial",
        feature = "editing-regression",
        feature = "navigation-trial"
    ))
))]
mod tablet;
#[cfg(all(
    target_os = "ios",
    feature = "ipad-scenes",
    not(any(
        feature = "candidate-trial",
        feature = "editing-regression",
        feature = "navigation-trial"
    ))
))]
pub use tablet::start as start_tablet;

#[cfg(all(target_os = "ios", not(feature = "navigation-trial")))]
pub fn start() {
    #[cfg(not(any(feature = "candidate-trial", feature = "editing-regression")))]
    let screen = {
        let mut dir = std::env::var_os("HOME")
            .map(|home| std::path::PathBuf::from(home).join("Library/Application Support/KUI"));
        if cfg!(debug_assertions)
            && let Ok(id) = std::env::var("KUI_TEST_DRAFTS")
            && id.len() == 36
            && id.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-')
        {
            dir = Some(std::env::temp_dir().join(id));
        }
        Screen::restore(dir)
    };
    #[cfg(all(feature = "editing-regression", not(feature = "candidate-trial")))]
    let screen = {
        let mut s = Screen::default();
        s.regression.result = std::env::temp_dir().join("editing-result");
        s
    };
    #[cfg(feature = "candidate-trial")]
    let screen = candidates::Candidates::default();
    mobile::run(core(), screen).expect("iOS runner");
}

#[cfg(all(target_os = "ios", feature = "navigation-trial"))]
pub fn start() {
    let dir = std::env::var_os("HOME").map(|home| {
        std::path::PathBuf::from(home).join("Library/Application Support/KUI/navigation")
    });
    mobile::run(core(), navigation::Trial::restore(dir)).expect("iOS runner");
}

#[cfg(all(
    target_os = "android",
    not(any(
        feature = "candidate-trial",
        feature = "editing-regression",
        feature = "navigation-trial"
    ))
))]
fn draft_dir(app: &mobile::AndroidApp) -> Option<std::path::PathBuf> {
    if cfg!(debug_assertions) {
        use jni::{
            jni_sig, jni_str,
            objects::{JObject, JString, JValue},
            refs::Global,
        };
        // Instrumented touch runs exercise the ordinary screen with fresh fields.
        let vm = unsafe { jni::JavaVM::from_raw(app.vm_as_ptr().cast()) };
        let ephemeral = vm
            .attach_current_thread(|env| -> jni::errors::Result<bool> {
                let raw = app.activity_as_ptr().cast();
                let activity = unsafe { env.as_cast_raw::<Global<JObject>>(&raw)? };
                let intent = env
                    .call_method(
                        activity,
                        jni_str!("getIntent"),
                        jni_sig!("()Landroid/content/Intent;"),
                        &[],
                    )?
                    .l()?;
                let key = JString::from_str(env, "dev.kui.mobile.spike.ephemeral")?;
                env.call_method(
                    intent,
                    jni_str!("getBooleanExtra"),
                    jni_sig!("(Ljava/lang/String;Z)Z"),
                    &[JValue::Object(key.as_ref()), JValue::Bool(false)],
                )?
                .z()
            })
            .unwrap_or(false);
        if ephemeral {
            return None;
        }
    }
    app.internal_data_path().map(|path| path.join("drafts"))
}

#[cfg(all(target_os = "android", not(feature = "navigation-trial")))]
#[unsafe(no_mangle)]
fn android_main(activity: mobile::AndroidApp) {
    #[cfg(not(any(feature = "candidate-trial", feature = "editing-regression")))]
    let screen = Screen::restore(draft_dir(&activity));
    #[cfg(all(feature = "editing-regression", not(feature = "candidate-trial")))]
    let screen = {
        let mut s = Screen::default();
        s.regression.result = activity
            .internal_data_path()
            .unwrap()
            .join("editing-result");
        s.regression.activity = Some(activity.clone());
        s
    };
    #[cfg(feature = "candidate-trial")]
    let screen = candidates::Candidates::default();
    mobile::run(activity, core(), screen).expect("Android runner");
}

#[cfg(all(target_os = "android", feature = "navigation-trial"))]
#[unsafe(no_mangle)]
fn android_main(activity: mobile::AndroidApp) {
    let dir = activity
        .internal_data_path()
        .map(|path| path.join("navigation"));
    mobile::run(activity, core(), navigation::Trial::restore(dir)).expect("Android runner");
}
