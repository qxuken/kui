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
        // `conformance::build_align`: the three spreads, a baseline row,
        // and a ratio sizing each axis.
        "align" => r#"
            local function sq() return column { width = 10, height = 10, bg = 0x30344aff } end
            local function spread(a)
              return row { width = 120, main_align = a, sq(), sq(), sq() }
            end
            return column { pad = 4, gap = 6, width = 128, bg = 0x101018ff,
              spread("spaceBetween"), spread("spaceAround"), spread("spaceEvenly"),
              row { gap = 4, cross_align = "baseline",
                text("ab", { size = 12 }), text("cd", { size = 20 }), sq() },
              column { width = { grow = 1 }, aspect_ratio = 4, bg = 0x3b5bd4ff },
              column { height = 12, aspect_ratio = 2, bg = 0x73d98cff },
            }
        "#
        .to_string(),
        // `conformance::build_stock_controls` (ADR 0034).
        "stock-controls" => r#"
            return column { pad = 8, gap = 8,
              slider { label = "Volume", width = 216, value_now = 30,
                       value_min = 0, value_max = 100, value_step = 10,
                       on_change = { kind = "vol" } },
              checkbox { label = "Mute", on_click = { kind = "mute" } },
              checkbox { label = "Sync", checked = true, on_click = { kind = "sync" } },
              checkbox { label = "All", mixed = true,
                         on_click = { kind = "all" } },
              radio_group { label = "Theme",
                radio { label = "Light", on_click = { kind = "light" } },
                radio { label = "Dark", checked = true,
                        on_click = { kind = "dark" } },
              },
              switch { label = "Wi-Fi", checked = true, on_click = { kind = "wifi" } },
            }
        "#
        .to_string(),
        // A table (ADR 0033): `grid`, since `table` is Lua's own. The
        // header is a fit row of two bare texts; each body row a grow row
        // of a bare text, a fixed box and a grow box.
        "table" => {
            let rows = conformance::TABLE_ROWS
                .iter()
                .map(|(label, w, h)| {
                    format!(
                        "row {{ width = \"grow\", gap = 6,
                text(\"{label}\", {{ size = 12 }}),
                column {{ width = {w}, height = {h}, bg = 0x30344aff }},
                column {{ width = \"grow\", height = {h}, bg = 0x3b5bd4ff }},
              }},"
                    )
                })
                .collect::<Vec<_>>()
                .join("\n              ");
            format!(
                r#"
            return grid {{ key = "table", width = 200, pad = 4, gap = 2, bg = 0x101018ff,
                           rules = 0x2b3350ff, rule_width = 1,
              row {{ gap = 6, text("name", {{ size = 12 }}), text("w", {{ size = 12 }}) }},
              {rows}
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
        "scroll-handler-room" => r#"
            return column { pad = 4,
              row { key = "strip", width = 200, height = 80, scroll_x = true, bg = 0x101018ff,
                column { key = "code", width = 100, height = 80, scroll_x = true,
                         on_scroll = { kind = "code" }, bg = 0x161820ff,
                  column { width = 300, height = 80, bg = 0x3b5bd4ff },
                },
                column { width = 200, height = 80, bg = 0x2a2d3aff },
              },
            }
        "#
        .to_string(),
        "scroll-gestures" => {
            let items = conformance::ITEM_KEYS
                .iter()
                .map(|k| {
                    format!("column {{ key = \"{k}\", width = 90, height = 20, bg = 0x30344aff }},")
                })
                .collect::<Vec<_>>()
                .join("\n                    ");
            format!(
                r#"
            return column {{ pad = 4,
              column {{ key = "page", width = 200, height = 100, scroll_y = true, bg = 0x101018ff,
                row {{ key = "strip", width = 200, height = 80, scroll_x = true,
                  column {{ key = "list", width = 100, height = 80, gap = 4, scroll_y = true,
                           overscroll = "contain", bg = 0x161820ff,
                    {items}
                  }},
                  column {{ key = "term", width = 100, height = 80, bg = 0x3b5bd4ff,
                           on_scroll = {{ kind = "term" }}, scroll_axes = "y" }},
                  column {{ width = 100, height = 80, bg = 0x2a2d3aff }},
                }},
                column {{ width = 200, height = 60, bg = 0x22252fff }},
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
        // Backlog F90: two nodes on a `clip` canvas panned half past its
        // top, the first declaring `clip`.
        "clip-float" => r#"
            local function node(key, dx, clip, bg, label)
              return column { key = key, float = { anchor = "parent", dx = dx, dy = -20, clip = clip },
                              width = 80, height = 40, bg = bg, on_click = { kind = key }, label = label }
            end
            return column { width = { grow = 1 }, height = { grow = 1 },
              row { key = "toolbar", width = { grow = 1 }, height = 40, bg = 0x3a3f52ff,
                    on_click = { kind = "toolbar" }, label = "Toolbar" },
              column { key = "canvas", width = { grow = 1 }, height = { grow = 1 }, clip = true,
                       bg = 0x101018ff,
                       node("node", 40, true, 0x3b5bd4ff, "Node"),
                       node("free", 160, false, 0x73d98cff, "Free") },
            }
        "#
        .to_string(),
        // Boxes on whole pixels: two snapped, the first with a hard shadow,
        // and one drawn where layout put it.
        "pixel-snap" => r#"
            local function cell(bg, snap, shadow)
              return column { width = 40.5, height = 20.25, bg = bg, pixel_snap = snap,
                              shadow_color = shadow }
            end
            return row {
              cell(0xd9738cff, true, 0x000000ff),
              cell(0x73d98cff, true),
              cell(0x3b5bd4ff, false),
            }
        "#
        .to_string(),
        // Backlog F93: access rects cut to the clip — three nodes on a
        // `clip` canvas past its top, three rows in a short scroller.
        "clip-access" => r#"
            local function button(key, label, w, h, bg)
              return { key = key, width = w, height = h, bg = bg,
                       on_click = { kind = key }, label = label }
            end
            local function node(key, label, dx, dy, clip)
              local t = button(key, label, 60, 40, 0x3b5bd4ff)
              t.float = { anchor = "parent", dx = dx, dy = dy, clip = clip }
              return column(t)
            end
            local function item(key, label)
              return column(button(key, label, 100, 30, 0x73d98cff))
            end
            local toolbar = button("toolbar", "Toolbar", { grow = 1 }, 40, 0x3a3f52ff)
            return column { width = { grow = 1 }, height = { grow = 1 },
              column(toolbar),
              row { width = { grow = 1 }, height = { grow = 1 },
                column { key = "canvas", width = { grow = 1 }, height = { grow = 1 }, clip = true,
                         bg = 0x101018ff,
                         node("cut", "Cut", 20, -20, true),
                         node("past", "Past", 100, -60, true),
                         node("free", "Free", 140, -60, false) },
                column { key = "list", width = 100, height = 50, scroll_y = true, bg = 0x202030ff,
                         item("row0", "Row 0"), item("row1", "Row 1"), item("row2", "Row 2") },
              },
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
            return column { window_title = "kui conformance", always_on_top = true,
                            secure_input = true, option_as_alt = "left", gap = 6,
              titlebar { text("app", { size = 12 }) },
              row { width = { grow = 1 }, keep_focus = true, window_buttons() },
              column { key = "sink", width = 40, height = 16, bg = 0x22242cff,
                       focusable = true, key_focus = true, on_focus = { kind = "sink" }, label = "Sink" },
            }
        "#
        .to_string(),
        "controls" => r#"
            return column { pad = 10, gap = 6, on_context_menu = { kind = "menu" },
                            on_button = { kind = "panel" }, buttons = "middle",
              button { label = "go", on_click = { kind = "go" }, description = "Starts the run" },
              button { key = "stop", label = "Stop the run", text = "stop", on_click = { kind = "stop" },
                       disabled = true, tooltip = "Nothing is running" },
              edit { key = "note", initial = "hello, on two lines in a narrow field",
                     size = 13, width = 160, wrap = "word", label = "Note" },
              row { key = "focus", width = 120, height = 12, role = "slider",
                    label = "Focus length", value_now = 25, value_min = 5,
                    value_max = 60, value_text = "25 minutes" },
            }
        "#
        .to_string(),
        // The paste scene's tree is the ime scene's (backlog F84).
        "ime" | "paste" => r#"
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
        // The same card, forty px tall and scrolling, over six runs: what a
        // press held past its edge scrolls (ADR 0029).
        "selection-scroll" => {
            let lines = conformance::SELECTION_SCROLL_LINES
                .iter()
                .map(|l| format!("text(\"{l}\", {{ size = 13 }}),"))
                .collect::<Vec<_>>()
                .join("\n                ");
            format!(
                r#"
            return column {{ key = "card", width = 200, height = {h}, pad = 8, gap = 4,
                             bg = 0x14161eff, scroll_y = true, selectable = true,
                {lines}
            }}
        "#,
                h = conformance::SELECTION_SCROLL_HEIGHT
            )
        }
        // The `cells` screen three rows tall, `selectable` and hearing the
        // wheel, row 0 at 100 plus the phase — the phase being how the
        // scene's view answers a `scroll` event (ADR 0029).
        "cells-scroll" => {
            let rows = conformance::CELLS_SCROLL_ROWS
                .iter()
                .map(|r| format!("\"{r:<11}\""))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                r#"
            return column {{ pad = 10,
              cells {{ key = "term", rows = 3, cols = 11, size = 13, family = "mono",
                      line_height = 18, lines = {{ {rows} }},
                      runs = {{ {{ 0, 0, 11, 0xd6d8e0ff, 0, 0 }}, {{ 1, 0, 11, 0xd6d8e0ff, 0, 0 }},
                               {{ 2, 0, 11, 0xd6d8e0ff, 0, 0 }} }},
                      origin_line = {origin} + phase, selectable = true,
                      on_scroll = {{ kind = "term" }}, label = "term" }} }}
        "#,
                origin = conformance::CELLS_SCROLL_ORIGIN
            )
        }
        // Two sinks asking for releases, tagged by kind; the second asks
        // for the modifier keys (backlog F108).
        "modifier-keys" => r#"
            local function sink(name, mods)
              return row { key = name, width = 100, height = 24, bg = 0x1b1d27ff,
                           on_key = { kind = name }, key_up = true, modifier_keys = mods,
                           role = "group", label = name }
            end
            return column { pad = 10, gap = 6, sink("plain", false), sink("mods", true) }
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
        // many params — then the image input (backlog V1): the sampling
        // fixture over the icon, over the stream, and over the removed
        // fixture (`Fixtures::dead`: 0 would mean "no image", and any
        // other number is live in some process). The handles are the
        // fixtures', as integers.
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
              row {{ gap = 4,
                fragment {{ id = {sampler}, image = {image}, params = {{{ip}}}, width = 24, height = 24 }},
                fragment {{ id = {sampler}, image = {stream}, params = {{{ip}}}, width = 32, height = 8 }},
                fragment {{ id = {sampler}, image = {dead}, params = {{{ip}}}, width = 24, height = 24 }},
              }},
            }}
        "#,
            frag = f.fragment.to_ffi(),
            sampler = f.sampler.to_ffi(),
            image = f.image.to_ffi(),
            stream = f.stream.to_ffi(),
            dead = f.dead.to_ffi(),
            p = lua_numbers(&kui_core::conformance::FRAGMENT_PARAMS),
            ip = lua_numbers(&kui_core::conformance::FRAGMENT_IMAGE_PARAMS),
            pl = lua_numbers(&kui_core::conformance::FRAGMENT_PARAMS_LONG),
        ),
        // Backlog K4: a wave in red under a span, a green solid line, dots
        // in their own colour, and an undercurl over three cells whose run
        // carries the wave bit and a seventh entry, the underline colour.
        "underlines" => r#"
            local mono = { size = 14, family = "mono", line_height = 20 }
            return column { pad = 10, gap = 4,
              text({ "let ", { "value", underline_color = 0xff0000ff, underline_style = "wavy" } }, mono),
              text("warn", { size = 14, family = "mono", line_height = 20, underline_color = 0x00ff00ff }),
              text("dots", { size = 14, family = "mono", line_height = 20,
                             underline_color = 0x7f9cf5ff, underline_style = "dotted" }),
              cells { key = "term", rows = 1, cols = 3, size = 14, family = "mono",
                      line_height = 20, lines = { "abc" },
                      runs = { { 0, 0, 3, 0xd6d8e0ff, 0, 32, 0xff0000ff } }, label = "term" },
            }
        "#
        .to_string(),
        // Backlog F101: rounded span backgrounds joined across four texts,
        // the fourth in another colour.
        "joined-backgrounds" => r#"
            local mono = { size = 14, family = "mono", line_height = 20 }
            local function sel(s) return { s, bg = 0x3b5bd466, bg_radius = 4 } end
            return column { pad = 10,
              text({ "let ", sel("a = 1;") }, mono),
              text({ sel("let b = 22;") }, mono),
              text({ sel("c"), " + d" }, mono),
              text({ { "find", bg = 0xd9738c66, bg_radius = 4 } }, mono),
            }
        "#
        .to_string(),
        // Backlog F106: `ab  c` breaking its spaces in a box 4 px wide.
        "break-spaces" => r#"
            return column { pad = 10,
              column { width = 4,
                text({ "ab", { "  ", bg = 0x3b5bd4ff }, "c" },
                     { size = 14, family = "mono", line_height = 20, wrap = "break-spaces" }),
              },
            }
        "#
        .to_string(),
        // Backlog F110: two halves and a gap, two clamps and a gap, each
        // pair fitting its row.
        "relative-shrink" => r#"
            local function bar(w)
              return column { width = w, height = 10, bg = 0x3b5bd4ff }
            end
            return column { gap = 4,
              row { width = 200, gap = 20, bar("50%"), bar("50%") },
              row { width = 300, gap = 20,
                bar("clamp(100px, 60%, 400px)"),
                bar({ clamp = { 100, { pct = 60 }, 400 } }),
              },
            }
        "#
        .to_string(),
        // Backlog F114: three rows of a 20 px bar in a 50 px column at 0,
        // 20 and 40; a clip under a row giving to the 30 px left; a
        // wrapping row's two lines kept.
        "column-squeeze" => r#"
            local function bar()
              return column { width = 100, height = 20, bg = 0x3b5bd4ff }
            end
            local function chip()
              return column { width = 60, height = 20, bg = 0x73d98cff }
            end
            return row { gap = 20,
              column { width = 100, height = 50, row { bar() }, row { bar() }, row { bar() } },
              column { width = 100, height = 50, row { bar() }, column { clip = true, bar(), bar() } },
              column { width = 100, height = 50,
                row { width = "grow", wrap_children = true, gap = 10, cross_gap = 5, chip(), chip() },
                row { bar() },
              },
            }
        "#
        .to_string(),
        // Backlog F116: a fit column in a card capped at 100 held to it,
        // its wrapping row of three 40 px chips on two lines; beside it
        // the same column with `min_width = "fit"` at 140, one line.
        "fit-across" => r#"
            local function card(wrapper)
              local chips = { wrap_children = true, gap = 10, cross_gap = 5 }
              for _ = 1, 3 do
                chips[#chips + 1] = column { width = 40, height = 20, bg = 0x73d98cff }
              end
              wrapper.bg = 0x30344aff
              wrapper[1] = row(chips)
              return column { max_width = 100, column(wrapper) }
            end
            return row { gap = 20, card({}), card({ min_width = "fit" }) }
        "#
        .to_string(),
        // Backlog F109: four bars sized by expressions, spelled and as data.
        "size-expressions" => r#"
            local function bar(t)
              t.height = 10
              t.bg = 0x3b5bd4ff
              return column(t)
            end
            return column { width = 400, gap = 4,
              bar { width = "clamp(100px, 50%, 150px)" },
              bar { width = { min = { { pct = 80 }, 300 } } },
              bar { width = 900, max_width = "25%" },
              bar { min_width = { max = { "40%", 50 } } },
            }
        "#
        .to_string(),
        // docs/adr/0010-a-segment-primitive.md: three strokes and a box in
        // a 200×120 canvas; the elbow takes a click, hit by its stroke.
        "lines" => r#"
            return column { width = 200, height = 120, bg = 0x14161eff,
              line { from = {10, 10}, to = {90, 70}, width = 2, color = 0x7f9cf5ff },
              line { points = {{100, 20}, {140, 20}, {140, 60}}, width = 3,
                     color = 0xd8863bff, on_click = { kind = "elbow" }, label = "Elbow" },
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
        // ADR 0025: the icon as `contain` in a box twice its aspect, then the
        // stream fixture — updated in place by `conformance::fixtures`, so
        // texture-backed — plain, `nearest`, and `cover` in a square box.
        "media" => format!(
            r#"
            return column {{ pad = 6, gap = 4,
              image {{ id = {img}, width = 16, radius = 2 }},
              image {{ id = {img}, width = 32, height = 16, fit = "contain", label = "Icon" }},
              image {{ id = {stream}, width = 16, label = "Stream" }},
              image {{ id = {stream}, width = 16, sampling = "nearest", label = "Crisp" }},
              image {{ id = {stream}, width = 12, height = 12, fit = "cover", label = "Cropped" }},
              audio {{ key = "music", src = {snd}, volume = 0.5, loop = true }},
              latency_graph(),
              phase == 0 and audio {{ key = "chime", src = {snd}, finish = true }} or nil,
              phase == 0 and audio {{ key = "blip", src = {snd} }} or nil,
            }}
        "#,
            img = f.image.to_ffi(),
            stream = f.stream.to_ffi(),
            snd = f.sound.to_ffi(),
        ),
        // docs/adr/0025-the-image-is-the-canvas.md, decision 6: five fills
        // in a 200×120 canvas; the triangle takes a click, hit by its outline
        // (ADR 0026), the star is keyed, the ninth point is dropped, the quad
        // fades. Vertices are `conformance::POLYGON_*` to the number.
        "polygon" => r#"
            return column { width = 200, height = 120, bg = 0x14161eff,
              polygon { points = {{10, 10}, {60, 20}, {20, 50}}, bg = 0x7f9cf5ff,
                        on_click = { kind = "tri" }, label = "Triangle" },
              polygon { points = {{80, 10}, {130, 30}, {80, 50}, {95, 30}}, bg = 0xd8863bff },
              polygon { key = "star", bg = 0xf5d67fff,
                        points = {{170, 10}, {176, 24}, {190, 30}, {176, 36}, {170, 50}, {164, 36}, {150, 30}, {164, 24}} },
              polygon { bg = 0x9ad9a0ff,
                        points = {{10, 70}, {30, 65}, {50, 70}, {70, 65}, {90, 70}, {90, 110}, {50, 100}, {10, 110}, {5, 90}} },
              polygon { points = {{110, 70}, {190, 70}, {180, 110}, {120, 110}}, bg = 0xe07a8aff, opacity = 0.5 },
            }
        "#
        .to_string(),
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
        // The stock select (backlog F73): `dropdown`, since `select` is
        // Lua's own; the options are strings or the row tables
        // `env.open_menu` takes, and `current` counts from 1.
        "select" => r#"
            return column { pad = 10, gap = 6,
              dropdown { label = "language",
                         options = { "English", "Deutsch", "Français",
                                     { label = "Latin", id = "la", enabled = false } },
                         current = 2 },
              text("body", { size = 12 }),
            }
        "#
        .to_string(),
        // The application menu bar (ADR 0018): one element that declares
        // the menu and draws it. The item tables are the ones
        // `env.open_menu` takes, one level down.
        "menubar" => r#"
            return column { gap = 6, width = { grow = 1 },
              menu_bar { menu = {
                { label = "File", items = {
                    { label = "New", id = "file.new", accel = "mod+n" },
                    { role = "separator" },
                    { label = "Wrap", id = "file.wrap", checked = true },
                    { label = "Print", id = "file.print", enabled = false },
                } },
                { label = "Edit", items = { { role = "copy" } } },
              } },
              text("body", { size = 12 }),
            }
        "#
        .to_string(),
        // Same tree as `selection`: the menu is not declared by anyone,
        // it is what the core opens over the card on a secondary press —
        // and a Shift-press keeping the anchor is the core's reading of
        // the modifier, nothing the script declares (ADR 0029).
        "selection" | "menu" | "selection-extend" => {
            let lines = conformance::SELECTION_LINES
                .iter()
                .map(|l| format!("text(\"{l}\", {{ size = 13 }}),"))
                .collect::<Vec<_>>()
                .join("\n                ");
            format!(
                r#"
            return column {{ key = "card", width = 200, pad = 8, gap = 4,
                             bg = 0x14161eff, selectable = true,
                {lines}
            }}
        "#
            )
        }
        "drag" => r#"
            return column { key = "handle", width = 80, height = 40, bg = 0x30344aff,
                            on_drag = { kind = "split" } }
        "#
        .to_string(),
        "virtual" => {
            // `index` is a number where `key` is a string: the row's own
            // data index, which is what a virtual list opens its rows at.
            let count = conformance::VIRTUAL_ROW_COUNT;
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
                             scroll_y = true, bg = 0x101018ff, row_count = {count},
                             role = "list", label = "log",
              column {{ key = "lead", width = "grow", height = 20 }},
                {rows}
              column {{ key = "tail", width = "grow", height = 100 }},
            }}
        "#
            )
        }
        // Scroll anchoring (backlog C26 step 3): two scrollers of the same
        // rows, one with `anchor`; phase 1 prepends a taller row to both.
        // The prepended row goes first, so the children are appended to
        // the table one by one rather than written with a `nil` hole.
        "anchor" => {
            let items = conformance::ITEM_KEYS
                .iter()
                .map(|k| format!("\"{k}\""))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                r#"
            local function list(key, anchor)
              local t = {{ key = key, width = 90, height = 60, scroll_y = true,
                          bg = 0x101018ff, anchor = anchor }}
              if phase >= 1 then
                t[#t + 1] = column {{ key = "new", width = 80, height = 30, bg = 0x30344aff }}
              end
              for _, k in ipairs({{ {items} }}) do
                t[#t + 1] = column {{ key = k, width = 80, height = 20, bg = 0x30344aff }}
              end
              return column(t)
            end
            return row {{ pad = 10, gap = 10, list("anchored", true), list("plain", false) }}
        "#
            )
        }
        // The four scrollbar rows: `scrollbar` is the mode name, the three
        // beside it the thumb's width and two colours.
        "scrollbar" => {
            let items = conformance::ITEM_KEYS
                .iter()
                .map(|k| format!("column {{ key = \"{k}\", width = 80, height = 20, bg = 0x30344aff }},"))
                .collect::<Vec<_>>()
                .join("\n                ");
            let list = |key: &str, extra: &str| {
                format!(
                    "column {{ key = \"{key}\", width = 90, height = 60, gap = 0, scroll_y = true,
                       bg = 0x101018ff, {extra}
                {items}
              }},"
                )
            };
            format!(
                r#"
            return row {{ pad = 10, gap = 10,
              {hidden}
              {styled}
              {auto}
            }}
        "#,
                hidden = list("hidden", "scrollbar = \"hidden\","),
                styled = list(
                    "styled",
                    "scrollbar_width = 8, scrollbar_color = 0x3b5bd4ff, scrollbar_active_color = 0xffcc00ff,"
                ),
                auto = list("auto", "scrollbar = \"auto\","),
            )
        }
        // ADR 0023: the toast is later in the tree than the popover, so it
        // is over it in phase 0; the popover closes in phase 1 and reopens
        // in phase 2, which puts it over the toast. The rows overflow so
        // the page has a bar for the popover to cover.
        // `conformance::build_sampler` (backlog AR47): the generic rows no
        // other scene declares, every one by its snake_case schema name —
        // `direction` for `repeat`, which is a Lua keyword.
        "sampler" => format!(
            r#"
            return column {{ pad = 8, gap = 6,
              row {{ key = "card", width = 120, height = 40, max_width = 100, max_height = 30,
                     center = true, bg = 0x1b1d27ff,
                     radius_tl = 8, radius_tr = 2, radius_br = 8, radius_bl = 2,
                     shadow_color = 0x00000080, shadow_x = 3, shadow_y = 2, shadow_blur = 2,
                     hoverable = true, hover_bg = 0x262a3aff, pressed_bg = 0x30364aff,
                     hover_group = "cards", focusable = true, focus_bg = 0x2b3350ff,
                     initial_focus = true, accent = true, cursor = "pointer",
                     selected = true, expanded = "expanded",
                     on_click = {{ kind = "card" }}, on_hover = {{ kind = "hov" }},
                     on_layout = {{ kind = "lay" }}, on_force_click = {{ kind = "force" }},
                     click_sound = {snd}, hover_sound = {snd}, animate = true,
                     transition = 100, easing = "easeInOut", bounce = 0.3, slide = true, delay = 20,
                     direction = "alternate",
                     keyframes = {{ {{ bg = 0x1b1d27ff }}, {{ at = 1, bg = 0x3b5bd4ff, radius = 12 }} }},
                     enter = {{ dx = -12, opacity = 0 }},
                     role = "tab", label = "Card",
                     text("ab", {{ size = 12 }}) }},
              row {{ key = "strip", width = 60, height = 10, bg = 0x3a3f52ff, window = "drag" }},
              row {{ key = "dock", focus_region = true, gap = 4, height = 30,
                     main_align = "center", cross_align = "end",
                     row {{ key = "stop", width = 20, height = 20, bg = 0x2a2d3aff,
                            focusable = true, role = "button", label = "Stop" }} }},
              column {{ width = 60,
                        text("a long line that is cut short", {{ size = 12, max_lines = 1, ellipsis = true,
                              underline = true, strikethrough = true, features = "liga=0" }}) }},
              row {{ key = "line", height = 16, role = "line", caret = 2, selection_anchor = 0,
                     caret_solid = true, text("sel", {{ size = 12 }}) }},
            }}
        "#,
            snd = f.sound.to_ffi(),
        ),
        "layers" => format!(
            r#"
            local rows = {{ key = "page", width = {{ grow = 1 }}, height = {{ grow = 1 }},
                           scroll_y = true, bg = 0x101018ff }}
            for i = 0, {rows} - 1 do
              rows[#rows + 1] = row {{ key = "row" .. i, width = {{ grow = 1 }}, height = 30,
                                       bg = i % 2 == 0 and 0x22242cff or 0x30344aff }}
            end
            local function at(x, y)
              return {{ anchor = "viewport", at = {{ "start", "start" }},
                       self_at = {{ "start", "start" }}, dx = x, dy = y }}
            end
            local t = {{ width = {{ grow = 1 }}, height = {{ grow = 1 }}, column(rows) }}
            if phase ~= 1 then
              t[#t + 1] = column {{ key = "popover", float = at(200, 40), width = 120, height = 80,
                                    bg = 0x3b5bd4ff, on_click = {{ kind = "popover" }}, label = "Popover" }}
            end
            t[#t + 1] = column {{ key = "toast", float = at(140, 60), width = 120, height = 80,
                                  bg = 0x73d98cff, on_click = {{ kind = "toast" }}, label = "Toast" }}
            return column(t)
        "#,
            rows = conformance::LAYERS_ROWS,
        ),
        // ADR 0031: two zones, a button inside the first, and across the
        // phases a hoverable float over the first zone that is no zone
        // (phase 1) and a modal over it (phase 2).
        "drop" => r#"
            local over = { anchor = "viewport", at = { "start", "start" },
                           self_at = { "start", "start" }, dx = 20, dy = 20 }
            local t = { width = { grow = 1 }, height = { grow = 1 },
              column { key = "files", width = 200, height = { grow = 1 }, pad = 10,
                       bg = 0x22242cff, drop_bg = 0x2b3350ff, on_drop = { kind = "files" },
                       row { key = "pick", width = 60, height = 40, bg = 0x3b5bd4ff,
                             on_click = { kind = "pick" }, label = "Pick" } },
              column { key = "other", width = { grow = 1 }, height = { grow = 1 },
                       bg = 0x30344aff, on_drop = { kind = "other" } },
            }
            if phase == 1 then
              t[#t + 1] = column { key = "overlay", float = over, width = 160, height = 160,
                                   hoverable = true }
            end
            if phase == 2 then
              t[#t + 1] = column { key = "confirm", float = over, width = 160, height = 160,
                                   bg = 0x101018ff, modal = { kind = "dismiss" } }
            end
            return row(t)
        "#
        .to_string(),
        // ADR 0027: the table is the `tokens` global, declared once at
        // load; every value below is a `$name` the parser resolves,
        // `$surface` and `$radius` being the roles, `$nothing` the one
        // that warns. The declaration itself sits outside `view`, in the
        // prelude this arm returns beside the body.
        "tokens" => {
            let keys = conformance::TOKEN_KEYS;
            let derived_cells = keys[4..]
                .iter()
                .map(|k| {
                    format!(
                        "              column {{ key = \"{k}\", width = \"$side_w\", height = 30, bg = \"${k}\" }},"
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            format!(
                r##"
            return row {{ pad = {{ l = "$gap", r = 10, t = 10, b = 10 }}, gap = "$gap",
              column {{ key = "{k0}", width = "$side_w", height = 30, bg = "$peach" }},
              column {{ key = "{k1}", width = "$side_w", height = 30, bg = "$ink",
                        border = {{ w = "$gap", color = "$peach" }} }},
              column {{ key = "{k2}", width = "$side_w", height = 30, bg = "$surface", radius = "$radius" }},
              column {{ key = "{k3}", width = "$side_w", height = 30, bg = "$nothing" }},
{derived_cells}
              text({{ "tokens", {{ "x", color = "$ink" }} }}, {{ size = "$big", color = "$peach" }}),
            }}
        "##,
                k0 = keys[0],
                k1 = keys[1],
                k2 = keys[2],
                k3 = keys[3],
            )
        }
        other => panic!("no Lua scene for {other:?} — every corpus scene needs one"),
    };
    let prelude = match scene.name {
        "tokens" => {
            let colors = conformance::TOKEN_COLORS
                .iter()
                .map(|(n, l, d)| format!("{n} = {{ light = 0x{l:08x}, dark = 0x{d:08x} }}"))
                .collect::<Vec<_>>()
                .join(", ");
            let lengths = conformance::TOKEN_LENGTHS
                .iter()
                .map(|(n, v)| format!("{n} = {v:?}"))
                .collect::<Vec<_>>()
                .join(", ");
            // The derived tokens (ADR 0028) as tuples in the array part;
            // `read`'s single op is written bare, the sugar the parser
            // tells from a list by its first element.
            let derived = conformance::TOKEN_DERIVED
                .iter()
                .map(|(n, from, ops)| {
                    let tuple = |(verb, color, t): &(&str, &str, f32)| {
                        if color.is_empty() {
                            format!("{{ \"{verb}\", {t:?} }}")
                        } else {
                            format!("{{ \"{verb}\", \"{color}\", {t:?} }}")
                        }
                    };
                    let ops = if *n == "read" {
                        tuple(&ops[0])
                    } else {
                        format!(
                            "{{ {} }}",
                            ops.iter().map(tuple).collect::<Vec<_>>().join(", ")
                        )
                    };
                    format!("{n} = {{ from = \"{from}\", ops = {ops} }}")
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "tokens = {{ colors = {{ {colors}, {derived} }}, lengths = {{ {lengths} }} }}\n"
            )
        }
        _ => String::new(),
    };
    // Every scene also records what the script saw in `env.window`, so the
    // test can assert the readback: the facts a scene is driven under are
    // the facts its view read, through the surface a script has.
    format!("{prelude}function view(env)\n  seen_window = env.window\n{body}\nend\n")
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
            seen.get::<bool>("always_on_top").unwrap(),
            seen.get::<Option<f32>>("controls_w").unwrap(),
            seen.get::<Option<f32>>("controls_h").unwrap(),
        );
        let declared = (
            scene.env.id.0,
            scene.env.custom_chrome,
            scene.env.maximized,
            scene.env.fullscreen,
            scene.env.always_on_top,
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
