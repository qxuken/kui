-- Ledger: paper and ink, 2px region rules, a hard offset shadow on the one
-- action that matters. Kit's default skin (the style canvas's H′ board).

local skin = require("skin")

local ledger: skin.Skin = {
  name = "ledger",
  light = {
    bg = 0xf6f3eaff, side = 0xefeadcff, panel = 0xfffdf8ff, raised = 0xfffdf8ff,
    sunken = 0xf0ead9ff, hover = 0x14141310, press = 0x1414131f,
    rule = 0xd9d2c0ff, ink = 0x141413ff, fg = 0x141413ff, muted = 0x4a4740ff,
    faint = 0x6b6659ff, sel = 0xffd400ff, sel_fg = 0x141413ff, accent = 0xffd400ff,
    on_accent = 0x141413ff, focus = 0x2f6fedff,
    ok = 0x1d6b45ff, warn = 0x9a4a00ff, danger = 0xb3261eff, scrim = 0x14141366,
    -- Pale tints, so a bar's label reads in `fg` on any of them.
    groups = { 0xd6e2f5ff, 0xcfe8daff, 0xf6d5d0ff, 0xe3d8f2ff, 0xf3e1c2ff, 0xcdeaedff },
    series = { 0x141413ff, 0x2f6fedff, 0xd1491fff, 0x1d8a55ff, 0x8a4fd1ff, 0xb38600ff },
  },
  dark = {
    bg = 0x131312ff, side = 0x0e0e0dff, panel = 0x1a1a18ff, raised = 0x22221fff,
    sunken = 0x0e0e0dff, hover = 0xf1eee612, press = 0xf1eee622,
    rule = 0x3a3a36ff, ink = 0xf1eee6ff, fg = 0xf1eee6ff, muted = 0xb3afa4ff,
    faint = 0x8c887dff, sel = 0xffd400ff, sel_fg = 0x131312ff, accent = 0xffd400ff,
    on_accent = 0x131312ff, focus = 0x7fa8ffff,
    ok = 0x7fe0a8ff, warn = 0xffc46bff, danger = 0xff8a80ff, scrim = 0x000000a0,
    groups = { 0x22344fff, 0x1d3a2cff, 0x4a2420ff, 0x342a4aff, 0x45351aff, 0x1b3c40ff },
    series = { 0xf1eee6ff, 0x7fa8ffff, 0xff9466ff, 0x7fe0a8ff, 0xc9a2ffff, 0xffd400ff },
  },
  radius = 0,
  rule_w = 2,
  hair = 1,
  shadow = 3,
  shadow_blur = 0,
  label_bold = true,
  label_upper = true,
  ctl_border = true,
  field_box = false,
  -- Plex when it is installed or loaded; the stock faces otherwise, so a
  -- machine without Plex draws Kit with no `unknown-family` warning.
  font_ui = { "Space Grotesk", "IBM Plex Sans", "sans" },
  font_data = { "IBM Plex Mono", "mono" },
  size = 12,
  size_label = 11,
  size_title = 13,
}

return ledger
