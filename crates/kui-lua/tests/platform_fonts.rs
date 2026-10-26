//! What a script reads of the machine it runs on: `env.platform` (backlog
//! C116), the same fact a `when` entry's `platform` condition reads; and
//! the system's fonts, listed and loaded from a file (backlog C117).

use kui_core::{Color, Core, Extensions, NodeSpec, Platform, Size, Value, testing};
use kui_lua::LuaExtension;

const PLATFORM: &str = r#"
    components = {
      card = function(env, props)
        seen = env.platform
        return row { width = 10, height = 10, bg = 0xffffffff,
                     when = { { platform = "windows", bg = 0xff0000ff } } }
      end,
    }
"#;

fn frame(core: &mut Core, exts: &mut Extensions, name: &str) -> Vec<Color> {
    let mut ui = core.frame_with(Size::new(200.0, 100.0), 1.0, exts);
    ui.configure_root(NodeSpec::column().fill());
    ui.component(name, &Value::Null);
    ui.finish();
    testing::solids(core).iter().map(|q| q.color).collect()
}

#[test]
fn env_platform_and_the_platform_condition_agree() {
    let mut exts = Extensions::new();
    let ext = LuaExtension::from_source("s.lua", PLATFORM).unwrap();
    let lua = ext.lua().clone();
    exts.push_as("s", Box::new(ext)).unwrap();
    let mut core = Core::new();
    assert_eq!(core.platform(), Platform::current(), "the build's to start");
    for p in Platform::ALL {
        core.set_platform(p);
        frame(&mut core, &mut exts, "s/card");
        let colours = frame(&mut core, &mut exts, "s/card");
        let seen: String = lua.globals().get("seen").unwrap();
        assert_eq!(seen, p.name(), "env.platform reads the core's");
        assert_eq!(
            colours.contains(&Color::hex(0xff0000ff)),
            p == Platform::Windows,
            "{p:?}: the rule reads the same fact: {colours:?}"
        );
    }
    let w = core.take_warnings();
    assert!(w.is_empty(), "{w:?}");
}

const FONTS: &str = r#"
    components = {
      fonts = function(env, props)
        families = env.system_font_families()
        loaded, loaded_err = env.load_font_file("fonts/liga.ttf")
        again = env.load_font_file("fonts/liga.ttf")
        missing, missing_err = env.load_font_file("fonts/none.ttf")
        garbage, garbage_err = env.load_font_file("fonts/garbage.ttf")
        after = env.system_font_families()
        return text("Fonts", { size = 20, family = loaded or "sans" })
      end,
    }
"#;

#[test]
fn a_script_lists_the_fonts_and_loads_a_file_from_its_folder() {
    // The face the tests carry, written out under a folder the script is
    // given as its own: a relative path is read against it.
    let dir = std::env::temp_dir().join(format!("kui-lua-c117-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("fonts")).unwrap();
    std::fs::write(dir.join("fonts/liga.ttf"), testing::liga_font()).unwrap();
    std::fs::write(dir.join("fonts/garbage.ttf"), [0u8; 64]).unwrap();

    let mut exts = Extensions::new();
    let ext = LuaExtension::from_source("s.lua", FONTS).unwrap();
    let lua = ext.lua().clone();
    kui_lua::set_script_dir(&lua, &dir).unwrap();
    exts.push_as("s", Box::new(ext)).unwrap();
    let mut core = Core::new();
    frame(&mut core, &mut exts, "s/fonts");
    frame(&mut core, &mut exts, "s/fonts");
    let g = lua.globals();

    let loaded: Option<String> = g.get("loaded").unwrap();
    let err: Option<String> = g.get("loaded_err").unwrap();
    assert_eq!(loaded.as_deref(), Some("Kui Liga"), "the family: {err:?}");
    let again: Option<String> = g.get("again").unwrap();
    assert_eq!(again, loaded, "the same file again is the same family");

    let missing: Option<String> = g.get("missing").unwrap();
    let missing_err: String = g.get("missing_err").unwrap();
    assert!(
        missing.is_none() && missing_err.contains("none.ttf"),
        "{missing_err}"
    );
    let garbage: Option<String> = g.get("garbage").unwrap();
    let garbage_err: String = g.get("garbage_err").unwrap();
    assert!(
        garbage.is_none() && garbage_err.contains("garbage.ttf"),
        "{garbage_err}"
    );

    let after: Vec<String> = g.get("after").unwrap();
    assert!(
        after.iter().any(|f| f == "Kui Liga"),
        "a loaded family is listed"
    );
    let mut sorted = after.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(after, sorted, "sorted, each once");
    assert_eq!(after, core.system_font_families(), "the core's list");
    // Loaded once however often the view asked.
    let faces = core
        .system_fonts()
        .into_iter()
        .find(|f| f.family == "Kui Liga")
        .map(|f| f.weights.len());
    assert_eq!(faces, Some(1));

    let w = core.take_warnings();
    assert!(
        !testing::codes(&w).contains(&"unknown-family"),
        "the loaded family draws by name: {w:?}"
    );
    std::fs::remove_dir_all(&dir).ok();
}
