-- Generated from src/neutral.tl by addons/kit/build.lua. Edit the .tl.




local skin = require("skin")

local neutral = {
   name = "neutral",
   light = {
      bg = 0xf7f7f8ff, side = 0xf0f0f2ff, panel = 0xffffffff, raised = 0xffffffff,
      sunken = 0xf0f0f2ff, hover = 0x0f172a0d, press = 0x0f172a1a,
      rule = 0xe4e4e8ff, ink = 0xc9c9d1ff, fg = 0x18181bff, muted = 0x5b5b66ff,
      faint = 0x9b9ba6ff, sel = 0x2f6fed1f, sel_fg = 0x18181bff, accent = 0x2f6fedff,
      on_accent = 0xffffffff, focus = 0x2f6fedff,
      ok = 0x15803dff, warn = 0xa16207ff, danger = 0xc0262dff, scrim = 0x0f172a4d,
      groups = { 0xdbe6fbff, 0xd6f0e0ff, 0xfbe0dcff, 0xe8e0fbff, 0xfbecd2ff, 0xd5eef2ff },
      series = { 0x2f6fedff, 0xe5592bff, 0x15a05aff, 0x8b5cf6ff, 0xd4a106ff, 0x0e9fb5ff },
   },
   dark = {
      bg = 0x111113ff, side = 0x0c0c0eff, panel = 0x18181bff, raised = 0x222226ff,
      sunken = 0x0c0c0eff, hover = 0xffffff0f, press = 0xffffff1c,
      rule = 0x2a2a30ff, ink = 0x3f3f47ff, fg = 0xececf0ff, muted = 0xa1a1abff,
      faint = 0x6b6b75ff, sel = 0x5b8cff33, sel_fg = 0xececf0ff, accent = 0x5b8cffff,
      on_accent = 0x0b0b0dff, focus = 0x5b8cffff,
      ok = 0x4ade80ff, warn = 0xfacc15ff, danger = 0xf87171ff, scrim = 0x00000099,
      groups = { 0x1e2a44ff, 0x173326ff, 0x3f211dff, 0x2a2244ff, 0x3b2f18ff, 0x15343aff },
      series = { 0x5b8cffff, 0xff8a5bff, 0x4ade80ff, 0xb197fcff, 0xfacc15ff, 0x38bdf8ff },
   },
   radius = 6,
   rule_w = 1,
   hair = 1,
   shadow = 4,
   shadow_blur = 16,
   label_bold = false,
   label_upper = false,
   ctl_border = false,
   field_box = true,
   font_ui = { "Inter", "sans" },
   font_data = { "JetBrains Mono", "IBM Plex Mono", "mono" },
   size = 12.5,
   size_label = 12,
   size_title = 13.5,
}

return neutral