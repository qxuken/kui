//! The Lua adapter over the scene corpus (`kui_core::conformance`).
//!
//! Each scene is re-expressed as the plain table tree a script would
//! return, driven through the same protocol as the Rust reference, and
//! diffed report against report. Both sides run in this process, so no
//! dump file is involved and the quad digests cover real geometry: a prop
//! the Lua parser reads under a different name, or a composite it lowers
//! differently, is a failing assertion naming the scene.

use kui_core::conformance::{self, Fixtures, Scene};
use kui_core::{Core, Extension};
use kui_lua::LuaExtension;

/// The corpus in Lua. One arm per scene, in `SCENES` order; an unknown name
/// panics rather than skipping, so a new scene cannot land without one.
fn lua_source(scene: &Scene, f: &Fixtures) -> String {
    let body = match scene.name {
        "layout" => r#"
            return column { pad = 8, gap = 6, bg = 0x14161eff,
              row { key = "card", pad = { l = 12, r = 10, t = 6, b = 4 }, gap = 4,
                    bg = 0x202030ff, border = { w = 2, color = 0x2a2d3aff },
                    radius = 5, width = 180, height = 40,
                    text("ab", { size = 12 }),
                    text("cd", { size = 12 }) },
              text({ "a ", { "b", bold = true, color = 0x73d98cff },
                     { " c", italic = true } }, { size = 13 }),
            }
        "#
        .to_string(),
        "overflow" => {
            let items = conformance::ITEM_KEYS
                .iter()
                .map(|k| {
                    format!(
                        "column {{ key = \"{k}\", width = 100, height = 20, bg = 0x30344aff }},"
                    )
                })
                .collect::<Vec<_>>()
                .join("\n                ");
            format!(
                r#"
            return column {{ pad = 4, clip = true,
              column {{ key = "list", width = 120, height = 60, gap = 4,
                       scroll_y = true, bg = 0x101018ff,
                {items}
              }},
            }}
        "#
            )
        }
        "float" => r#"
            return column { pad = 20, gap = 4,
              column { key = "anchor", width = 80, height = 24, bg = 0x333333ff,
                column { float = "below", width = 40, height = 12, bg = 0xff0000ff } },
              column { float = { anchor = "viewport", at = { "end", "end" },
                                 self_at = { "end", "end" }, dx = -4, dy = -4, fit = true },
                       width = 10, height = 10, bg = 0x00ff00ff },
            }
        "#
        .to_string(),
        "tooltip" => r#"
            return column { pad = 10,
              row { key = "tip", width = 100, height = 40, bg = 0x333333ff,
                    role = "group", tooltip = "a hint",
                    text("badge", { size = 12 }) },
            }
        "#
        .to_string(),
        "chrome" => r#"
            return column { window_title = "kui conformance", gap = 6,
              titlebar { text("app", { size = 12 }), window_buttons() },
              column { key = "sink", width = 40, height = 16, bg = 0x22242cff,
                       focusable = true, key_focus = true, label = "Sink" },
            }
        "#
        .to_string(),
        "controls" => r#"
            return column { pad = 10, gap = 6, on_context_menu = { kind = "menu" },
              button { label = "go", on_click = { kind = "go" } },
              edit { key = "note", initial = "hello", size = 13, width = 160,
                     label = "Note" },
            }
        "#
        .to_string(),
        "media" => format!(
            r#"
            return column {{ pad = 6, gap = 4,
              image {{ id = {img}, width = 16, radius = 2 }},
              audio {{ key = "music", src = {snd}, volume = 0.5, loop = true }},
              latency_graph(),
            }}
        "#,
            img = f.image.to_ffi(),
            snd = f.sound.to_ffi(),
        ),
        other => panic!("no Lua scene for {other:?} — every corpus scene needs one"),
    };
    format!("function view(env)\n{body}\nend\n")
}

#[test]
fn every_scene_lowers_identically_from_lua() {
    for scene in conformance::SCENES {
        let expected = conformance::report(scene.name, scene.steps, &conformance::run(scene));

        let mut core = Core::new();
        let f = conformance::fixtures(&mut core);
        let source = lua_source(scene, &f);
        let mut ext = LuaExtension::from_source(scene.name, &source)
            .unwrap_or_else(|e| panic!("{}: {e}", scene.name));
        let out = conformance::drive(&mut core, scene.steps, |ui| {
            ext.view(ui)
                .unwrap_or_else(|e| panic!("{}: {e}", scene.name))
        });
        let actual = conformance::report(scene.name, scene.steps, &out);

        assert_eq!(
            actual, expected,
            "scene {:?} lowers differently from Lua than from Rust",
            scene.name
        );
    }
}
