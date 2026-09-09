//! The Lua adapter over the scene corpus (`kui_core::conformance`).
//!
//! Each scene is re-expressed as the plain table tree a script would
//! return, driven through the same protocol as the Rust reference, and
//! diffed report against report. Both sides run in this process, so no
//! dump file is involved and the quad digests cover real geometry: a prop
//! the Lua parser reads under a different name, or a composite it lowers
//! differently, is a failing assertion naming the scene.

use kui_core::conformance::{self, Fixtures, Scene};
use kui_core::{Core, Extension, Slot};
use kui_lua::LuaExtension;

/// The corpus in Lua. One arm per scene, in `SCENES` order; an unknown name
/// panics rather than skipping, so a new scene cannot land without one.
/// A float list as Lua source. `{:?}` on an f32 prints `1.0` for a whole
/// number and the shortest round-tripping form otherwise, which is what
/// Lua's number literal takes, so the bits reaching the core are the ones
/// the Rust scene declared — and the corpus compares them as bits.
fn lua_numbers(v: &[f32]) -> String {
    v.iter()
        .map(|n| format!("{n:?}"))
        .collect::<Vec<_>>()
        .join(", ")
}

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
        "tabs" => {
            let roomy = conformance::TAB_ROOMY
                .iter()
                .map(|(w, h)| {
                    format!(
                        "column {{ width = {{ grow = 1 }}, min_width = \"fit\", height = {{ pct = 50 }}, min_height = \"fit\", bg = 0x30344aff,
                  column {{ width = {w}, height = {h}, bg = 0x3b5bd4ff }} }},"
                    )
                })
                .collect::<Vec<_>>()
                .join("\n                ");
            let crowded = conformance::TAB_CROWDED
                .iter()
                .map(|w| {
                    format!(
                        "column {{ width = {{ grow = 1 }}, min_width = \"fit\", height = {{ grow = 1 }}, bg = 0x30344aff,
                  column {{ width = {w}, height = 12, bg = 0x3b5bd4ff }} }},"
                    )
                })
                .collect::<Vec<_>>()
                .join("\n                ");
            format!(
                r#"
            return column {{ pad = 4, gap = 4,
              row {{ key = "roomy", width = 200, height = 20, bg = 0x101018ff,
                {roomy}
              }},
              row {{ key = "crowded", width = 200, height = 20, scroll_x = true, bg = 0x101018ff,
                {crowded}
              }},
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
              row { width = 100, height = 20, role = "button", label = "Save",
                    description = "Nothing to save yet" },
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
              button { label = "go", on_click = { kind = "go" }, description = "Starts the run" },
              button { key = "stop", label = "Stop the run", text = "stop", on_click = { kind = "stop" },
                       disabled = true, tooltip = "Nothing is running" },
              edit { key = "note", initial = "hello", size = 13, width = 160,
                     label = "Note" },
              row { key = "focus", width = 120, height = 12, role = "slider",
                    label = "Focus length", value_now = 25, value_min = 5,
                    value_max = 60, value_text = "25 minutes" },
            }
        "#
        .to_string(),
        "ime" => r#"
            return column { pad = 10, gap = 6,
              column { key = "buffer", width = 200, height = 24, bg = 0x1b1d27ff,
                       on_key = { kind = "ed" }, role = "multilineTextInput", label = "Buffer",
                       row { key = "l0", height = 20, role = "line", caret = 1,
                             text("ab", { size = 13, family = "mono" }) } },
              edit { key = "note", initial = "", size = 13, width = 200, label = "Note" } }
        "#
        .to_string(),
        "cells" => r#"
            return column { pad = 10,
              cells { key = "term", rows = 1, cols = 11, size = 13, family = "mono",
                      line_height = 18, lines = { "hello world" },
                      runs = { { 0, 0, 11, 0xd6d8e0ff, 0, 0 }, { 0, 0, 3, 0, 0x1a1d27ff, 0 } },
                      cursor_at = { 0, 3 }, cursor_shape = "block", cursor_color = 0x6a8bffff,
                      on_click = { kind = "hit" }, label = "term" } }
        "#
        .to_string(),
        "keys" => r#"
            local function sink(name, key_up, child)
              return row { key = name, width = 100, height = 24, bg = 0x1b1d27ff,
                           on_key = 1, key_up = key_up, role = "group", label = name,
                           child }
            end
            local go = row { key = "go", width = 80, height = 16, bg = 0x3b5bd4ff,
                             on_click = { kind = "go" }, label = "Go", key_focus = true }
            return column { pad = 10, gap = 6, sink("press", false), sink("held", true),
                            sink("shell", true, go) }
        "#
        .to_string(),
        // `phase` is the host-seeded global the `exit` scene uses: here it
        // drops the dialog, and the declaration the app makes on its way
        // out moves from `open` to the node it was renaming.
        "modal" => r#"
            -- The dialog and the node it was renaming never coexist, so
            -- one trailing child is both of them: a nil in the middle of a
            -- table constructor would end the child list early.
            local last
            if phase == 0 then
              last = column { key = "dialog", width = 120, height = 100,
                       pad = 8, gap = 6, bg = 0x202030ff,
                       float = { anchor = "viewport", at = { "end", "end" },
                                 self_at = { "end", "end" } },
                       modal = { kind = "dlg" }, label = "Settings",
                row { key = "ok", width = 100, height = 24, bg = 0x3b5bd4ff,
                      on_click = { kind = "ok" }, label = "OK" },
                row { key = "cancel", width = 100, height = 24, bg = 0x3b5bd4ff,
                      on_click = { kind = "cancel" }, label = "Cancel" },
              }
            else
              last = row { key = "note", width = 100, height = 20,
                    bg = 0x30344aff, on_click = { kind = "note" },
                    label = "Note", key_focus = true }
            end
            return column { width = { grow = 1 }, gap = 6,
              titlebar { text("app", { size = 12 }) },
              row { key = "open", width = 100, height = 20, bg = 0x30344aff,
                    on_click = { kind = "open" }, label = "Open",
                    key_focus = phase == 0 },
              last,
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
        // docs/adr/0015-a-fragment-element-and-the-painter-it-is-not.md:
        // four fragments, one of them with a dead handle and one with too
        // many params. The handle is the fixture's, as an integer.
        "fragments" => format!(
            r#"
            return column {{ width = 200, height = 120, gap = 4, bg = 0x14161eff,
              fragment {{ id = {frag}, params = {{{p}}}, width = 80, height = 40 }},
              fragment {{ key = "card", id = {frag}, params = {{{p}}},
                         width = 80, height = 40, pad = 6, radius = 8, opacity = 0.5,
                column {{ width = 20, height = 10, bg = 0x202030ff }},
              }},
              fragment {{ id = 0, params = {{{p}}}, width = 20, height = 10 }},
              fragment {{ id = {frag}, params = {{{pl}}}, width = 30, height = 12 }},
            }}
        "#,
            frag = f.fragment.to_ffi(),
            p = lua_numbers(&kui_core::conformance::FRAGMENT_PARAMS),
            pl = lua_numbers(&kui_core::conformance::FRAGMENT_PARAMS_LONG),
        ),
        // docs/adr/0010-a-segment-primitive.md: three strokes and a box in
        // a 200×120 canvas; the elbow's on_click is the one a line ignores.
        "lines" => r#"
            return column { width = 200, height = 120, bg = 0x14161eff,
              line { from = {10, 10}, to = {90, 70}, width = 2, color = 0x7f9cf5ff },
              line { points = {{100, 20}, {140, 20}, {140, 60}}, width = 3,
                     color = 0xd8863bff, on_click = "elbow" },
              line { key = "curve", curve = true, width = 1.5, color = 0x9ad9a0ff,
                     opacity = 0.5,
                     points = {{20, 100}, {60, 80}, {100, 110}, {180, 90}} },
              column { width = 40, height = 20, bg = 0x202030ff },
            }
        "#
        .to_string(),
        // The two playbacks phase 1 drops are declared in the reference's
        // order, so the ids match: `chime` (2) asks to finish and its
        // removal queues nothing, `blip` (3) is stopped.
        "media" => format!(
            r#"
            return column {{ pad = 6, gap = 4,
              image {{ id = {img}, width = 16, radius = 2 }},
              audio {{ key = "music", src = {snd}, volume = 0.5, loop = true }},
              latency_graph(),
              phase == 0 and audio {{ key = "chime", src = {snd}, finish = true }} or nil,
              phase == 0 and audio {{ key = "blip", src = {snd} }} or nil,
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
            -- More one-node departures than the budget, in a slot that
            -- keeps its size when they go (docs/adr/0012-the-exit-budget.md).
            local function slot_rows()
              local t = {{ key = "slotRows", width = 300, height = 4, bg = 0x101018ff }}
              if phase < 4 then
                for _ = 1, {cells} do
                  t[#t + 1] = column {{ width = 0.5, height = 4, bg = 0x8a8fa3ff,
                                       transition = 400, exit = {{ opacity = 0 }} }}
                end
              end
              return row(t)
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
              slot_rows(),
              phase < 3 and bulk() or nil,
            }}
        "#,
            rows = conformance::EXIT_BULK_ROWS,
            cells = conformance::EXIT_ROWS,
        ),
        // The declaration comes and goes with the phase, on the root table
        // beside `window_title`; twice in phase 0, disagreeing on the size.
        "windows" => r#"
            local declared = nil
            if phase == 0 then
              declared = { { name = "palette", width = 400, height = 300 },
                           { name = "palette", width = 500, height = 500 } }
            elseif phase == 2 then
              declared = { { name = "palette", width = 400, height = 300 } }
            end
            return column { windows = declared, pad = 8, bg = 0x14161eff,
              text((phase == 0 or phase == 2) and "open" or "closed", { size = 12 }),
            }
        "#
        .to_string(),
        // A popup, declared with the kind and the anchor a menu carries.
        "popup" => format!(
            r#"
            local declared = nil
            if phase == 0 then
              declared = {{ {{ name = "menu", kind = "popup",
                               width = {w}, height = {h},
                               anchor = {{ x = {x}, y = {y}, w = {aw}, h = {ah} }} }} }}
            end
            return column {{ windows = declared, pad = 8, bg = 0x14161eff,
              text(phase == 0 and "menu" or "closed", {{ size = 12 }}),
            }}
        "#,
            w = 160,
            h = 320,
            x = conformance::POPUP_ANCHOR.x as i32,
            y = conformance::POPUP_ANCHOR.y as i32,
            aw = conformance::POPUP_ANCHOR.w as i32,
            ah = conformance::POPUP_ANCHOR.h as i32,
        ),
        // The row is a plain snake_case prop; the queue is `env.announce`,
        // which is the only place a Lua script can call it from — `env`
        // exists inside `view` and nowhere else, so the phase guard is the
        // script's own (see the `announcement-repeated` warning).
        "live" => r#"
            if phase == 1 then env.announce("Saved", "assertive") end
            return column { pad = 8, gap = 4, bg = 0x14161eff,
              column { key = "status", live = "polite",
                text(phase == 0 and "0 results" or "3 results", { size = 12 }) },
              column { key = "empty", live = "polite" },
            }
        "#
        .to_string(),
        "drag" => r#"
            return column { key = "handle", width = 80, height = 40, bg = 0x30344aff,
                            on_drag = { kind = "split" } }
        "#
        .to_string(),
        "virtual" => {
            // `index` is a number where `key` is a string: the row's own
            // data index, which is what a virtual list opens its rows at.
            let rows = conformance::VIRTUAL_ROWS
                .iter()
                .map(|i| {
                    format!(
                        "column {{ index = {i}, width = \"grow\", height = 20, \
                         bg = 0x30344aff, role = \"listItem\", label = \"row {i}\" }},"
                    )
                })
                .collect::<Vec<_>>()
                .join("\n                ");
            format!(
                r#"
            return column {{ key = "list", width = 120, height = 60, gap = 0,
                             scroll_y = true, bg = 0x101018ff,
                             role = "list", label = "log",
              column {{ key = "lead", width = "grow", height = 20 }},
                {rows}
              column {{ key = "tail", width = "grow", height = 100 }},
            }}
        "#
            )
        }
        other => panic!("no Lua scene for {other:?} — every corpus scene needs one"),
    };
    // Every scene also records what the script saw in `env.window`, so the
    // test can assert the readback: the facts a scene is driven under are
    // the facts its view read, through the surface a script has.
    format!("function view(env)\n  seen_window = env.window\n{body}\nend\n")
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
            ext.view(&Slot::root(), ui)
                .unwrap_or_else(|e| panic!("{}: {e}", scene.name))
        });
        let actual = conformance::report(scene.name, scene.env, scene.steps, &out);

        assert_eq!(
            actual, expected,
            "scene {:?} lowers differently from Lua than from Rust",
            scene.name
        );

        // The readback. `chrome` and `chrome-inset` are the scenes this is
        // about — a frame under custom chrome reports `custom_chrome = true`
        // the same way in every binding — and the other scenes pin the
        // defaults for free. The controls rect is Lua's flattened pair
        // (`schema::ENV_FIELDS` says so), absent when there is none.
        let seen: mlua::Table = ext.lua().globals().get("seen_window").unwrap();
        let r = scene.env.native_controls;
        let got = (
            seen.get::<u32>("id").unwrap(),
            seen.get::<bool>("custom_chrome").unwrap(),
            seen.get::<bool>("maximized").unwrap(),
            seen.get::<bool>("fullscreen").unwrap(),
            seen.get::<Option<f32>>("controls_w").unwrap(),
            seen.get::<Option<f32>>("controls_h").unwrap(),
        );
        let declared = (
            scene.env.id.0,
            scene.env.custom_chrome,
            scene.env.maximized,
            scene.env.fullscreen,
            r.map(|r| r.x + r.w),
            r.map(|r| r.y + r.h),
        );
        assert_eq!(
            got, declared,
            "scene {:?}: the script read a different env.window than the scene declared",
            scene.name
        );
    }
}
