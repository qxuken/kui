-- Generated from src/phosphor.tl by addons/kit/build.lua. Edit the .tl.




local skin = require("skin")

local phosphor = {
   name = "phosphor",
   light = {
      bg = 0xeef3ecff, side = 0xe4ebe1ff, panel = 0xf6faf4ff, raised = 0xf6faf4ff,
      sunken = 0xe4ebe1ff, hover = 0x0f5a2a14, press = 0x0f5a2a24,
      rule = 0xbfd0c0ff, ink = 0x0f5a2aff, fg = 0x0d3b1dff, muted = 0x3f6b4cff,
      faint = 0x56755eff, sel = 0x0f5a2aff, sel_fg = 0xeef3ecff, accent = 0x0f7a35ff,
      on_accent = 0xeef3ecff, focus = 0x0f7a35ff,
      ok = 0x0f7a35ff, warn = 0x8a5a00ff, danger = 0xb3261eff, on_danger = 0xf6faf4ff, scrim = 0x0d3b1d55,
      groups = { 0xd3e8d6ff, 0xe3ead0ff, 0xf0dcd2ff, 0xd5e4e8ff, 0xe6dcecff, 0xece6cfff },
      series = { 0x0f7a35ff, 0x1b6fa8ff, 0xb35a00ff, 0x7a3fb3ff, 0xa8331bff, 0x5a7a0fff },
   },
   dark = {
      bg = 0x070b08ff, side = 0x050806ff, panel = 0x0b120dff, raised = 0x0f1a12ff,
      sunken = 0x050806ff, hover = 0x39ff6a14, press = 0x39ff6a26,
      rule = 0x1c3a24ff, ink = 0x39ff6aff, fg = 0x9cffb0ff, muted = 0x5fa870ff,
      faint = 0x4f8a5cff, sel = 0x39ff6aff, sel_fg = 0x070b08ff, accent = 0x39ff6aff,
      on_accent = 0x070b08ff, focus = 0x39ff6aff,
      ok = 0x39ff6aff, warn = 0xffd24aff, danger = 0xff5a4aff, on_danger = 0x070b08ff, scrim = 0x000000b0,
      groups = { 0x123a1cff, 0x2b3a12ff, 0x3a1f12ff, 0x12303aff, 0x2a1a3aff, 0x3a3412ff },
      series = { 0x39ff6aff, 0x4ad8ffff, 0xffb84aff, 0xd28cffff, 0xff6a5aff, 0xd8ff4aff },
   },
   radius = 0,
   rule_w = 1,
   hair = 1,
   shadow = 0,

   shadow_blur = 8,
   label_bold = true,
   label_upper = true,
   ctl_border = true,
   field_box = false,
   font_ui = { "IBM Plex Mono", "JetBrains Mono", "mono" },
   font_data = { "IBM Plex Mono", "JetBrains Mono", "mono" },
   size = 12,
   size_label = 11,
   size_title = 12,
}

return phosphor