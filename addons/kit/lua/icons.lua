-- Generated from src/icons.tl by addons/kit/build.lua. Edit the .tl.






local ctx = require("ctx")

local M = {}


local BODY = {
   search = '<circle cx="11" cy="11" r="7"/><path d="M20 20l-4-4"/>',
   x = '<path d="M6 6l12 12M18 6L6 18"/>',
   check = '<path d="M5 12.5l4.5 4.5L19 7.5"/>',
   minus = '<path d="M5 12h14"/>',
   plus = '<path d="M12 5v14M5 12h14"/>',
   chevron_down = '<path d="M6 9l6 6 6-6"/>',
   chevron_up = '<path d="M6 15l6-6 6 6"/>',
   chevron_right = '<path d="M9 6l6 6-6 6"/>',
   chevron_left = '<path d="M15 6l-6 6 6 6"/>',
   play = '<path d="M7 5l12 7-12 7z"/>',
   pause = '<path d="M8 5v14M16 5v14"/>',
   stop = '<path d="M6 6h12v12H6z"/>',
   refresh = '<path d="M20 11a8 8 0 1 0-2.3 5.7"/><path d="M20 4v7h-7"/>',
   filter = '<path d="M4 5h16l-6 7.5V19l-4-2v-4.5z"/>',
   sliders = '<path d="M4 7h10M18 7h2M4 17h4M12 17h8"/><circle cx="16" cy="7" r="2"/><circle cx="10" cy="17" r="2"/>',
   more = '<path d="M5 12h.01M12 12h.01M19 12h.01"/>',
   menu = '<path d="M4 6h16M4 12h16M4 18h16"/>',
   alert = '<path d="M12 4l9 16H3z"/><path d="M12 10v4M12 17h.01"/>',
   info = '<circle cx="12" cy="12" r="9"/><path d="M12 11v6M12 7.5h.01"/>',
   circle = '<circle cx="12" cy="12" r="8"/>',
   copy = '<path d="M9 9h11v11H9z"/><path d="M5 15H4V4h11v1"/>',
   trash = '<path d="M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3"/>',
   download = '<path d="M12 4v11M7 10l5 5 5-5M5 20h14"/>',
   upload = '<path d="M12 20V9M7 14l5-5 5 5M5 4h14"/>',
   external = '<path d="M14 4h6v6M20 4l-9 9M18 14v6H4V6h6"/>',
   clock = '<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>',
   zoom_in = '<circle cx="11" cy="11" r="7"/><path d="M20 20l-4-4M8 11h6M11 8v6"/>',
   zoom_out = '<circle cx="11" cy="11" r="7"/><path d="M20 20l-4-4M8 11h6"/>',
   fit = '<path d="M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5"/>',
   link = '<path d="M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1"/><path d="M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1"/>',
   terminal = '<path d="M5 7l5 5-5 5M12 18h7"/>',
   database = '<path d="M5 6c0-1.7 3-3 7-3s7 1.3 7 3-3 3-7 3-7-1.3-7-3zM5 6v12c0 1.7 3 3 7 3s7-1.3 7-3V6M5 12c0 1.7 3 3 7 3s7-1.3 7-3"/>',
   folder = '<path d="M3 6h6l2 2h10v11H3z"/>',
   file = '<path d="M6 3h8l4 4v14H6z"/><path d="M14 3v4h4"/>',
   globe = '<circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3c3 3 3 15 0 18M12 3c-3 3-3 15 0 18"/>',
   bolt = '<path d="M13 3L5 14h6l-1 7 8-11h-6z"/>',
   eye = '<path d="M2 12s4-7 10-7 10 7 10 7-4 7-10 7S2 12 2 12z"/><circle cx="12" cy="12" r="3"/>',
   dot = '<circle cx="12" cy="12" r="3"/>',
}

local SVG_HEAD = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">'


local docs = {}

function M.has(name)
   return BODY[name] ~= nil
end


function M.names()
   local out = {}
   for k in pairs(BODY) do out[#out + 1] = k end
   table.sort(out)
   return out
end



function M.node(c, name, size, role)
   local px = size or 14
   local body = BODY[name]
   if not body then
      return column({ width = px, height = px })
   end
   local doc = docs[name]
   if not doc then
      doc = SVG_HEAD .. body .. "</svg>"
      docs[name] = doc
   end

   return svg({ data = doc, width = px, height = px, role = "none", color = ctx.ref(c, role or "fg") })
end

return M