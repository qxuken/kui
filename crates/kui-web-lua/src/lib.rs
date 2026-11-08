//! The prebuilt browser runner for projects shipped by `kui pack --format web`.
#![cfg(target_os = "wasi")]

use std::{
    cell::{OnceCell, RefCell},
    path::Path,
};

use kui_core::{DevtoolsDock as Dock, Extensions, Session, SvgFileUpdate, Ui};
use kui_lua::LuaExtension;
use kui_web::{App, Runner};
use serde_json::Value;

mod app;
#[path = "../../../examples/web/host.rs"]
mod host;

thread_local! {
    static APP: RefCell<Option<Extensions>> = const { RefCell::new(None) };
    static TASKS: OnceCell<mlua::Function> = const { OnceCell::new() };
}

struct AppModel;

impl App for AppModel {
    fn view(&mut self, _ui: &mut Ui<'_>) {}
}

fn make_runner() -> Runner<AppModel> {
    let mut fonts = Vec::from(host::bundled_fonts());
    fonts.push(include_bytes!("../../kui-cli/assets/fonts/NotoSansMono-Regular.ttf").to_vec());
    let session = Session::with_fonts("en-US", fonts);
    session.set_svg_file_resolver(|path, previous| {
        // A packed asset has one revision for the lifetime of this instance.
        if !Path::new(path).starts_with("/app") {
            return SvgFileUpdate::Changed {
                stamp: None,
                data: Err(format!("{path}: SVG files must be part of the packed app")),
            };
        }
        let stamp = Some((0, 0));
        if previous == stamp {
            SvgFileUpdate::Unchanged
        } else {
            SvgFileUpdate::Changed {
                stamp,
                data: std::fs::read(path).map_err(|e| format!("{path}: {e}")),
            }
        }
    });
    let mut runner = Runner::new(AppModel, &session);
    runner.set_extensions(APP.with(|app| app.take().expect("a packed app is loaded")));
    let core = runner.core_mut();
    core.set_native_menus(false);
    core.set_native_menu_bar(false);
    core.set_devtools_dock_choices(&[Dock::Bottom, Dock::Off]);
    runner
}

#[unsafe(no_mangle)]
pub extern "C" fn select_app() -> usize {
    load().err().map_or(0, |error| {
        host::publish_browser_bytes(error.to_string().into_bytes())
    })
}

fn load() -> Result<(), Box<dyn std::error::Error>> {
    let manifest: Value = serde_json::from_str(&host::browser_text())?;
    if manifest["runner"].as_str() != Some(env!("CARGO_PKG_VERSION")) {
        return Err("the app bundle and its browser runner use different kui versions".into());
    }
    let entry = manifest["entry"]
        .as_str()
        .ok_or("web bundle has no entry")?;
    let name = manifest["name"].as_str().ok_or("web bundle has no name")?;
    let id = manifest["id"].as_str().ok_or("web bundle has no id")?;
    let version = manifest["version"]
        .as_str()
        .ok_or("web bundle has no version")?;
    let app = LuaExtension::from_file_with(Path::new("/app").join(entry), |lua| {
        app::install(lua, name, id, version)
    })?;
    let drain = app::initialize(&app)?;
    let mut extensions = Extensions::new();
    extensions.push_as(app::kit::NAMESPACE, Box::new(app::kit::extension()?))?;
    extensions.push_as("app", Box::new(app))?;
    TASKS.with(|tasks| {
        tasks
            .set(drain)
            .map_err(|_| "packed app tasks already initialized")
    })?;
    APP.with_borrow_mut(|app| *app = Some(extensions));
    Ok(())
}

#[unsafe(no_mangle)]
pub extern "C" fn next_frame_at() -> f64 {
    host::RUNNER.with_borrow(|runner| runner.next_frame_at().unwrap_or(0.0))
}

#[unsafe(no_mangle)]
pub extern "C" fn frame_for_gpu(width: f32, height: f32, scale: f32, time: f64) -> u32 {
    host::RUNNER.with_borrow_mut(|runner| {
        runner.frame_for_gpu(kui_core::Size::new(width, height), scale, time)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn run_tasks() -> u32 {
    TASKS.with(|tasks| {
        tasks.get().map_or(0, |run| match run.call::<bool>(()) {
            Ok(worked) => u32::from(worked),
            Err(error) => {
                eprintln!("{error}");
                0
            }
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn app_set_devtools(on: u32) {
    host::RUNNER.with_borrow_mut(|runner| runner.core_mut().set_devtools(on != 0));
}

#[unsafe(no_mangle)]
pub extern "C" fn app_devtools_height() -> f32 {
    host::RUNNER.with_borrow_mut(|runner| runner.core_mut().devtools_inset().h)
}
