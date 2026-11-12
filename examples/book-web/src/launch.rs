use crate::{Backdrop, Core, DEMO_MIN_WIDTH, DEMO_SIZE, Dock, SELECTED, Ui, UiEvent};

type LaunchResult = Result<(), Box<dyn std::error::Error>>;

/// The example's view and event callbacks.
pub trait App {
    fn view(&mut self, ui: &mut Ui<'_>);
    fn on_event(&mut self, _event: UiEvent) {}
    fn on_event_with(&mut self, event: UiEvent, _core: &mut Core) {
        self.on_event(event);
    }
    fn on_wake(&mut self) {}
    fn before_frame(&mut self, _core: &mut Core) {}
    fn after_input(&mut self, _core: &mut Core) {}
}

pub struct BookApp(Box<dyn App>);

impl kui_web::App for BookApp {
    fn view(&mut self, ui: &mut Ui<'_>) {
        self.0.view(ui);
    }
    fn before_frame(&mut self, core: &mut Core) {
        self.0.before_frame(core);
    }
    fn on_event_with(&mut self, event: UiEvent, core: &mut Core) {
        self.0.on_event_with(event, core);
    }
}

pub trait Example: App {
    const KEYS: &'static [(&'static str, &'static str)] = &[];
    const FLAGS: &'static [(&'static str, &'static str)] = &[];
    const RECORDING: Option<Recording> = None;
    fn window(&self) -> Window {
        Window::default()
    }
    fn dock(&self) -> Dock {
        Dock::Right
    }
    fn native_menus(&self) -> Option<bool> {
        None
    }
    fn devtools_menu(&self) -> bool {
        true
    }
    fn headless(&mut self, _core: &mut Core) -> Result<(), String> {
        Err("no headless drive".into())
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Recording {
    pub secs: f64,
    pub cues: &'static [(f64, Cue)],
}

#[derive(Clone, Copy, Debug)]
pub enum Cue {
    Click(&'static str),
    Key(&'static str),
}

#[derive(Clone, Copy, Debug, Default)]
pub enum Chrome {
    #[default]
    Native,
    Custom,
    Borderless,
}

#[derive(Clone, Copy, Debug, Default)]
pub enum Titlebar {
    #[default]
    Standard,
    Medium,
    Tall,
}

impl Titlebar {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "standard" => Some(Self::Standard),
            "medium" => Some(Self::Medium),
            "tall" => Some(Self::Tall),
            _ => None,
        }
    }
}

pub struct Window {
    pub size: (f64, f64),
    pub min_size: Option<(f64, f64)>,
    pub max_size: Option<(f64, f64)>,
    pub chrome: Chrome,
    pub backdrop: Backdrop,
    pub titlebar: Titlebar,
    extensions: Option<kui_core::Extensions>,
}

impl Default for Window {
    fn default() -> Self {
        Self {
            size: (960.0, 640.0),
            min_size: None,
            max_size: None,
            chrome: Chrome::Native,
            backdrop: Backdrop::Opaque,
            titlebar: Titlebar::Standard,
            extensions: None,
        }
    }
}

impl Window {
    pub fn size(mut self, w: f64, h: f64) -> Self {
        self.size = (w, h);
        self
    }
    pub fn min_size(mut self, w: f64, h: f64) -> Self {
        self.min_size = Some((w, h));
        self
    }
    pub fn max_size(mut self, w: f64, h: f64) -> Self {
        self.max_size = Some((w, h));
        self
    }
    pub fn chrome(mut self, chrome: Chrome) -> Self {
        self.chrome = chrome;
        self
    }
    pub fn custom_titlebar(self) -> Self {
        self.chrome(Chrome::Custom)
    }
    pub fn backdrop(mut self, backdrop: Backdrop) -> Self {
        self.backdrop = backdrop;
        self
    }
    pub fn titlebar(mut self, titlebar: Titlebar) -> Self {
        self.titlebar = titlebar;
        self
    }
    #[cfg(target_os = "wasi")]
    pub fn extension_as(
        mut self,
        namespace: &str,
        extension: impl kui_core::Extension + 'static,
    ) -> Self {
        self.extensions
            .get_or_insert_with(kui_core::Extensions::new)
            .push_as(namespace, Box::new(extension))
            .expect("valid example extension namespace");
        self
    }
    pub fn run(self, app: impl App + 'static) -> LaunchResult {
        DEMO_SIZE.set(self.size);
        DEMO_MIN_WIDTH.set(self.min_size.map_or(self.size.0, |size| size.0));
        SELECTED.with_borrow_mut(|slot| *slot = Some((BookApp(Box::new(app)), self.extensions)));
        Ok(())
    }
}

pub fn app(_title: &str) -> Window {
    Window::default()
}

pub fn run(_title: &str, app: impl Example + 'static) {
    app.window().run(app).expect("book example startup");
}

#[macro_export]
macro_rules! main {
    ($example:expr) => {
        pub(crate) fn main() -> Result<(), Box<dyn std::error::Error>> {
            let example = $example;
            let window = $crate::Example::window(&example);
            window.run(example)
        }
    };
}
