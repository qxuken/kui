-- Overlays: floats over the window. A dialog, a popover and a menu are
-- modal surfaces (ADR 0003): Tab stays inside, the rest of the window is
-- inert, and Escape or a press outside arrives as `dismiss` — the host
-- closes it by no longer placing it. A toast is a float in the corner.

local ctx = require("ctx")
local icons = require("icons")

local type Ctx = ctx.Ctx
local type P = {string:any}
local ref, act = ctx.ref, ctx.act

local M = {}

-- Under what opened it, its left edge on the opener's: a menu reads down
-- from where the eye already is (the `below` preset centres, as a tooltip
-- does). `fit` flips and clamps it onto the screen.
local MENU_FLOAT: {any:any} = { anchor = "below", at = { "start", "end" }, self = { "start", "start" }, fit = true }

local function str(v: any): string
  return v is string and v or nil
end

-- A floating surface: the raised colour, the skin's edge and shadow.
local function surface(c: Ctx, t: kui.Props): kui.Props
  t.bg = ref(c, "raised")
  t.radius = "$kitRadius"
  t.border = { w = c.s.skin.ctl_border and "$kitRuleW" or "$kitHair", color = ref(c, "ink") }
  t.clip = true
  ctx.shadow(c, t, true)
  return t
end

function M.dialog(c: Ctx, p: P): kui.Node
  local env = c.env
  local title = str(p.title) or ""
  local box = surface(c, {
    key = "dialog",
    role = "dialog",
    label = title,
    modal = act(c, "dialog.dismiss"),
    width = (p.width or 420) as number,
    max_height = math.max(120, (env.viewport_h or 600) - 48),
    enter = { opacity = 0, dy = 8 },
    transition = 140,
    row {
      width = "grow", height = 40, pad = { x = 16 }, cross_align = "center",
      text(title, { size = "$kitSizeTitle", bold = true, family = "$kitUi", color = ref(c, "fg") }),
    },
    column { key = "body", width = "grow", height = "grow", scroll_y = true, pad = { x = 16, y = 4 }, gap = 8, children("") },
    row {
      width = "grow", pad = 12, gap = 8, cross_align = "center",
      ctx.grow(),
      children("actions"),
    },
  })
  return column {
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
  }
end

function M.popover(c: Ctx, p: P): kui.Node
  return column(surface(c, {
    key = "popover",
    role = "dialog",
    float = MENU_FLOAT,
    modal = act(c, "popover.dismiss"),
    width = p.width as number,
    pad = 10,
    gap = 8,
    enter = { opacity = 0, dy = -4 },
    transition = 100,
    children(""),
  }))
end

function M.menu(c: Ctx, p: P): kui.Node
  local items = (p.items or {}) as {P}
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
    local row_t: kui.Props = {
      key = "i" .. i,
      role = "menuItem",
      label = it.label as string,
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
      ctx.text(c, it.label as string, { color = ref(c, danger and "danger" or "fg") }),
      ctx.grow(),
    }
    if not off then focused = true end
    if str(it.shortcut) then
      row_t[#row_t + 1] = ctx.data(c, it.shortcut as string, { size = "$kitSizeLabel", color = ref(c, "faint") })
    end
    t[#t + 1] = row(row_t)
  end
  return column(t)
end

local TONE_ICON: {string:string} = { ok = "check", warn = "alert", danger = "alert", accent = "info", neutral = "info" }

function M.toast(c: Ctx, p: P): kui.Node
  local tone = str(p.tone) or "neutral"
  local message = str(p.message) or ""
  local role = (tone == "ok" or tone == "warn" or tone == "danger") and tone or (tone == "accent" and "accent" or "muted")
  return row(surface(c, {
    key = "toast",
    float = { anchor = "viewport", at = { "end", "end" }, dx = -16, dy = -16 },
    label = message,
    min_width = 240,
    max_width = 420,
    pad = { x = 12, y = 10 },
    gap = 10,
    cross_align = "center",
    enter = { opacity = 0, dy = 12 },
    transition = 160,
    icons.node(c, TONE_ICON[tone] or "info", 14, role),
    -- Live on the message alone, so the close button stays a control.
    row { width = "grow", live = "polite", ctx.text(c, message, { wrap = "word" }) },
    row {
      role = "button", label = "Dismiss", width = 20, height = 20, main_align = "center",
      cross_align = "center", radius = "$kitRadius", hover_bg = ref(c, "hover"), cursor = "pointer",
      on_click = act(c, "toast.dismiss"),
      icons.node(c, "x", 12, "muted"),
    },
  }))
end

function M.event(m: P, kit: string): any
  if kit == "dialog.dismiss" or kit == "popover.dismiss" or kit == "menu.dismiss" or kit == "toast.dismiss" then
    return emit("on_dismiss", {})
  elseif kit == "menu.choose" then
    return emit("on_choose", { value = m.value })
  end
  return nil
end

return M
