-- Generated from src/ledger.tl by addons/kit/build.lua. Edit the .tl.



local skin = require("skin")

local ledger = {
   name = "ledger",
   light = {
      bg = 0xf6f3eaff, side = 0xf6f3eaff, panel = 0xfffdf8ff, hover = 0xf0ead9ff,
      rule = 0xd9d2c0ff, ink = 0x141413ff, fg = 0x141413ff, muted = 0x4a4740ff,
      sel = 0xffd400ff, sel_fg = 0x141413ff, accent = 0xffd400ff, on_accent = 0x141413ff,
      ok = 0x1d6b45ff, warn = 0x9a4a00ff,

      groups = { 0xd6e2f5ff, 0xcfe8daff, 0xf6d5d0ff, 0xe3d8f2ff, 0xf3e1c2ff, 0xcdeaedff },
   },
   dark = {
      bg = 0x131312ff, side = 0x131312ff, panel = 0x1a1a18ff, hover = 0x22221fff,
      rule = 0x3a3a36ff, ink = 0xf1eee6ff, fg = 0xf1eee6ff, muted = 0xb3afa4ff,
      sel = 0xffd400ff, sel_fg = 0x131312ff, accent = 0xffd400ff, on_accent = 0x131312ff,
      ok = 0x7fe0a8ff, warn = 0xffc46bff,
      groups = { 0x22344fff, 0x1d3a2cff, 0x4a2420ff, 0x342a4aff, 0x45351aff, 0x1b3c40ff },
   },
   radius = 0,
   rule_w = 2,
   shadow = 3,
   label_upper = true,


   font_ui = { "Space Grotesk", "IBM Plex Sans", "sans" },
   font_data = { "IBM Plex Mono", "mono" },
}

return ledger