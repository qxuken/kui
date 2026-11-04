-- Generated from src/themes.tl by addons/kit/build.lua. Edit the .tl.



local M = { P = P }

local PRESETS = {
   paper = { skin = "neutral", font_title = { "Georgia", "serif" },
roles = { accent = 0x3c6048ff, bg = { light = 0xf2efe7ff, dark = 0x20241fff },
panel = { light = 0xe9e6dcff, dark = 0x2a3028ff }, },
sizes = { heading = 64, display = 96, content = 24, caption = 14, code = 18, chart = 14, gap = 24, pad = 48 }, },
   signal = { skin = "phosphor", font_title = { "sans" },
roles = { accent = 0xa6cb57ff, bg = { light = 0xf0f4eaff, dark = 0x101814ff },
panel = { light = 0xe4ecdaff, dark = 0x19251dff }, },
sizes = { heading = 60, display = 88, content = 22, caption = 13, code = 18, chart = 13, gap = 20, pad = 40 }, },
   stage = { skin = "neutral", font_title = { "sans" },
roles = { accent = 0xe7683dff, bg = { light = 0xfaf4eeff, dark = 0x171616ff },
panel = { light = 0xf1e4daff, dark = 0x252221ff }, },
sizes = { heading = 80, display = 112, content = 28, caption = 14, code = 20, chart = 16, gap = 28, pad = 48 }, },
   minimal = { skin = "neutral", font_title = { "sans" },
roles = { accent = 0x315ccaff, bg = { light = 0xffffffff, dark = 0x151618ff },
panel = { light = 0xffffffff, dark = 0x1d1f22ff }, },
sizes = { heading = 56, display = 80, content = 24, caption = 14, code = 18, chart = 14, gap = 24, pad = 48 }, },
}

function M.settings(p)
   local base = PRESETS[p.theme] or {}
   local out = {}
   for k, v in pairs(base) do out[k] = v end
   for k, v in pairs(p) do out[k] = v end

   for _, name in ipairs({ "roles", "sizes" }) do
      local merged = {}
      local defaults = base[name]
      local overrides = p[name]
      if defaults then for k, v in pairs(defaults) do merged[k] = v end end
      if overrides then for k, v in pairs(overrides) do merged[k] = v end end
      out[name] = merged
   end
   return out
end

return M