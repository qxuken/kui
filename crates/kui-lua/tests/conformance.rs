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
                    radius = 5, opacity = 0.75,
                    shadow_color = 0x00000066, shadow_blur = 8,
                    shadow_y = 3, shadow_spread = 1,
                    width = 180, height = 40,
                    text("ab", { size = 12 }),
                    text("cd", { size = 12 }) },
              column { pad = { x = 9, y = 3, b = 1 }, bg = 0x2a2d3aff },
              text({ "a ", { "b", bold = true, color = 0x73d98cff },
                     { " c", italic = true } }, { size = 13 }),
            }
        "#
        .to_string(),
        "sizing" => r#"
            return column { pad = { l = 14, r = 14, t = 6, b = 6 },
              row { key = "bar", width = 200, height = 40, bg = 0x101018ff,
                column { width = 30, height = 20, bg = 0x30344aff },
                column { width = "25%", height = 20, bg = 0x3b5bd4ff },
                column { width = "fit", height = 20, bg = 0x73d98cff,
                  column { width = 20, height = 10, bg = 0xff0000ff } },
                column { width = { grow = 1 }, height = 20, bg = 0xffcc00ff },
              },
            }
        "#
        .to_string(),
        "wrap" => {
            let boxes = conformance::WRAP_BOXES
                .iter()
                .map(|(w, h)| format!("column {{ width = {w}, height = {h}, bg = 0x30344aff }},"))
                .collect::<Vec<_>>()
                .join("\n              ");
            format!(
                r#"
            return row {{ wrap_children = true, pad = 4, gap = 6, cross_gap = 10,
                         width = 100, bg = 0x101018ff,
              {boxes}
            }}
        "#
            )
        }
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
                       scroll_y = true, radius = 8, bg = 0x101018ff,
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
              column { key = "nudged", width = 60, height = 20, bg = 0x444444ff,
                column { float = { anchor = "below", dx = 6 },
                         width = 30, height = 10, bg = 0x0000ffff } },
              column { float = { anchor = "viewport", at = { "start", "end" },
                                 self = { "end", "start" }, dx = -6, dy = 14, fit = true },
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
        // One source for both: the two scenes differ only in the env they
        // are driven under, which is the thing being pinned.
        "chrome" | "chrome-inset" => r#"
            return column { window_title = "kui conformance", gap = 6,
              titlebar { text("app", { size = 12 }) },
              row { width = { grow = 1 }, window_buttons() },
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
        "modal" => r#"
            return column { width = { grow = 1 }, gap = 6,
              titlebar { text("app", { size = 12 }) },
              row { key = "open", width = 100, height = 20, bg = 0x30344aff,
                    on_click = { kind = "open" }, label = "Open" },
              column { key = "dialog", width = 120, height = 100, pad = 8, gap = 6,
                       bg = 0x202030ff,
                       float = { anchor = "viewport", at = { "end", "end" },
                                 self_at = { "end", "end" } },
                       modal = { kind = "dlg" }, label = "Settings",
                row { key = "ok", width = 100, height = 24, bg = 0x3b5bd4ff,
                      on_click = { kind = "ok" }, label = "OK" },
                row { key = "cancel", width = 100, height = 24, bg = 0x3b5bd4ff,
                      on_click = { kind = "cancel" }, label = "Cancel" },
              },
            }
        "#
        .to_string(),
        "composite" => r#"
            local function tab(name, kind, selected)
              return row { key = name, role = "tab", selected = selected,
                           width = 60, height = 20, bg = 0x30344aff,
                           on_click = { kind = kind },
                text(name, { size = 12 }) }
            end
            local function pick(name, kind)
              -- `focusable` on the row is what makes the list a composite:
              -- a navigation list holds links, a picker holds rows.
              return row { key = name, role = "listItem", focusable = true,
                           width = 80, height = 18, bg = 0x202030ff,
                           on_click = { kind = kind },
                text(name, { size = 12 }) }
            end
            return column { width = { grow = 1 }, gap = 6,
              row { key = "tabs", role = "tabList", gap = 4,
                tab("One", "one", false),
                tab("Two", "two", true),
                tab("Three", "three", false) },
              row { key = "add", width = 40, height = 20, bg = 0x3b5bd4ff,
                    on_click = { kind = "add" }, label = "Add" },
              column { key = "rows", role = "list", gap = 2,
                pick("Alpha", "alpha"),
                pick("Bravo", "bravo") },
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
        // The one scene whose view changes its mind: `phase` is a global
        // the host writes before each frame (see `every_scene_lowers_...`),
        // and the four subtrees it gates are the departures.
        "exit" => format!(
            r#"
            local function bulk()
              local t = {{ key = "bulk", transition = 400, exit = {{ opacity = 0 }} }}
              for _ = 1, {rows} do t[#t + 1] = column {{}} end
              return column(t)
            end
            return column {{ width = {{ grow = 1 }}, height = {{ grow = 1 }},
                            pad = 8, gap = 6, bg = 0x14161eff,
              row {{ key = "a", width = 60, height = 16, bg = 0x22242cff,
                    focusable = true, label = "A" }},
              column {{ key = "slotFade", width = 140, height = 40, bg = 0x101018ff,
                phase == 0 and column {{ key = "fade", width = 100, height = 24,
                  bg = 0x3b5bd4ff, transition = 400,
                  exit = {{ dx = 40, opacity = 0 }},
                  focusable = true, label = "Fade", on_click = {{ kind = "hit" }},
                  text("bye", {{ size = 12 }}) }} or nil }},
              column {{ key = "slotBlink", width = 140, height = 16, bg = 0x101018ff,
                phase == 0 and column {{ key = "blink", width = 100, height = 12,
                  bg = 0x73d98cff, transition = 50, exit = {{ dx = 20 }} }} or nil }},
              column {{ key = "slotFlash", width = 140, height = 16, bg = 0x101018ff,
                phase ~= 1 and column {{ key = "flash", width = 100, height = 12,
                  bg = 0xffcc00ff, transition = 400, exit = {{ dx = -20 }} }} or nil }},
              row {{ key = "b", width = 60, height = 16, bg = 0x22242cff,
                    focusable = true, label = "B" }},
              phase == 0 and bulk() or nil,
            }}
        "#,
            rows = conformance::EXIT_BULK_ROWS,
        ),
        other => panic!("no Lua scene for {other:?} — every corpus scene needs one"),
    };
    format!("function view(env)\n{body}\nend\n")
}

#[test]
fn every_scene_lowers_identically_from_lua() {
    for scene in conformance::SCENES {
        let expected =
            conformance::report(scene.name, scene.env, scene.steps, &conformance::run(scene));

        let mut core = Core::new();
        let f = conformance::fixtures(&mut core);
        let source = lua_source(scene, &f);
        let mut ext = LuaExtension::from_source(scene.name, &source)
            .unwrap_or_else(|e| panic!("{}: {e}", scene.name));
        let out = conformance::drive(&mut core, scene.env, scene.steps, |ui, phase| {
            // A script has no `env` reading for "which phase of a scene is
            // this"; the host seeds one, which is what a real host does
            // with any fact the core does not carry.
            ext.lua()
                .globals()
                .set("phase", phase)
                .unwrap_or_else(|e| panic!("{}: {e}", scene.name));
            ext.view(ui)
                .unwrap_or_else(|e| panic!("{}: {e}", scene.name))
        });
        let actual = conformance::report(scene.name, scene.env, scene.steps, &out);

        assert_eq!(
            actual, expected,
            "scene {:?} lowers differently from Lua than from Rust",
            scene.name
        );
    }
}
