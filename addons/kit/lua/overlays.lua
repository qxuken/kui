-- Generated from src/overlays.tl by addons/kit/build.lua. Edit the .tl.





local ctx = require("ctx")
local icons = require("icons")



local ref, act = ctx.ref, ctx.act

local M = {}




local MENU_FLOAT = { anchor = "below", at = { "start", "end" }, self = { "start", "start" }, fit = true }

local function str(v)
   return type(v) == "string" and v or nil
end


local function surface(c, t)
   t.bg = ref(c, "raised")
   t.radius = "$kitRadius"
   t.border = { w = c.s.skin.ctl_border and "$kitRuleW" or "$kitHair", color = ref(c, "ink") }
   t.clip = true
   ctx.shadow(c, t, true)
   return t
end

function M.dialog(c, p)
   local env = c.env
   local title = str(p.title) or ""
   local box = surface(c, {
      key = "dialog",
      role = "dialog",
      label = title,
      modal = act(c, "dialog.dismiss"),
      width = (p.width or 420),
      max_height = math.max(120, (env.viewport_h or 600) - 48),
      enter = { opacity = 0, dy = 8 },
      transition = 140,
      row({
         width = "grow", height = 40, pad = { x = 16 }, cross_align = "center",
         text(title, { size = "$kitSizeTitle", bold = true, family = "$kitUi", color = ref(c, "fg") }),
      }),
      column({ key = "body", width = "grow", height = "grow", scroll_y = true, pad = { x = 16, y = 4 }, gap = 8, children("") }),
      row({
         width = "grow", pad = 12, gap = 8, cross_align = "center",
         ctx.grow(),
         children("actions"),
      }),
   })
   return column({
      key = "scrim",
      float = { anchor = "viewport", at = { "start", "start" } },
      width = env.viewport_w,
      height = env.viewport_h,
      bg = ref(c, "scrim"),
      main_align = "center",
      cross_align = "center",
      enter = { opacity = 0 },
      transition = 140,
      column(box),
   })
end

function M.popover(c, p)
   return column(surface(c, {
      key = "popover",
      role = "dialog",
      float = MENU_FLOAT,
      modal = act(c, "popover.dismiss"),
      width = p.width,
      pad = 10,
      gap = 8,
      enter = { opacity = 0, dy = -4 },
      transition = 100,
      children(""),
   }))
end

function M.menu(c, p)
   local items = (p.items or {})
   local t = surface(c, {
      key = "menu",
      role = "menu",
      float = MENU_FLOAT,
      modal = act(c, "menu.dismiss"),
      min_width = 180,
      pad = 4,
      gap = 1,
      enter = { opacity = 0, dy = -4 },
      transition = 100,
   })
   local focused = false
   for i, it in ipairs(items) do
      local off = it.disabled == true
      local danger = it.danger == true
      local row_t = {
         key = "i" .. i,
         role = "menuItem",
         label = it.label,
         disabled = off,
         initial_focus = not off and not focused,
         width = "grow",
         height = "$kitRow",
         pad = { x = 8 },
         gap = 12,
         cross_align = "center",
         radius = "$kitRadius",
         hover_bg = danger and ref(c, "danger") or ref(c, "hover"),
         opacity = off and "$disabledOpacity" or nil,
         cursor = "pointer",
         on_click = act(c, "menu.choose", { value = it.value }),
         ctx.text(c, it.label, { color = ref(c, danger and "danger" or "fg") }),
         ctx.grow(),
      }
      if not off then focused = true end
      if str(it.shortcut) then
         row_t[#row_t + 1] = ctx.data(c, it.shortcut, { size = "$kitSizeLabel", color = ref(c, "faint") })
      end
      t[#t + 1] = row(row_t)
   end
   return column(t)
end

local TONE_ICON = { ok = "check", warn = "alert", danger = "alert", accent = "info", neutral = "info" }

function M.toast(c, p)
   local tone = str(p.tone) or "neutral"
   local message = str(p.message) or ""
   local role = (tone == "ok" or tone == "warn" or tone == "danger") and tone or (tone == "accent" and "accent" or "muted")
   return row(surface(c, {
      key = "toast",
      float = { anchor = "viewport", at = { "end", "end" }, dx = -16, dy = -16 },
      live = "polite",
      label = message,
      min_width = 240,
      max_width = 420,
      pad = { x = 12, y = 10 },
      gap = 10,
      cross_align = "center",
      enter = { opacity = 0, dy = 12 },
      transition = 160,
      icons.node(c, TONE_ICON[tone] or "info", 14, role),
      row({ width = "grow", ctx.text(c, message, { wrap = "word" }) }),
      row({
         role = "button", label = "Dismiss", width = 20, height = 20, main_align = "center",
         cross_align = "center", radius = "$kitRadius", hover_bg = ref(c, "hover"), cursor = "pointer",
         on_click = act(c, "toast.dismiss"),
         icons.node(c, "x", 12, "muted"),
      }),
   }))
end

function M.event(m, kit)
   if kit == "dialog.dismiss" or kit == "popover.dismiss" or kit == "menu.dismiss" or kit == "toast.dismiss" then
      return emit("on_dismiss", {})
   elseif kit == "menu.choose" then
      return emit("on_choose", { value = m.value })
   end
   return nil
end

return M