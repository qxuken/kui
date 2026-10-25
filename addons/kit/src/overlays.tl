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
  local vh = env.viewport_h or 600
  local box = surface(c, {
    key = "dialog",
    role = "dialog",
    label = title,
    modal = act(c, "dialog.dismiss"),
    width = (p.width or 420) as number,
    max_height = math.max(120, vh - 48),
    enter = { opacity = 0, dy = 8 },
    transition = 140,
    row {
      width = "grow", height = 40, pad = { x = 16 }, cross_align = "center",
      text(title, { size = "$kitSizeTitle", bold = true, family = "$kitUi", color = ref(c, "fg") }),
    },
    -- The body is as tall as what it holds, up to what the window leaves
    -- past the title and the foot, and scrolls past that. A grow height in
    -- a box with no height of its own laid it out at nothing.
    column { key = "body", width = "grow", max_height = math.max(48, vh - 48 - 40 - 52), scroll_y = true,
      pad = { x = 16, y = 4 }, gap = 8, children("") },
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
    -- A modal surface is announced by its name: what it holds.
    label = str(p.label),
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
    ctx.ring(c, row_t, "i" .. i)
    if not off then focused = true end
    if str(it.shortcut) then
      row_t[#row_t + 1] = ctx.data(c, it.shortcut as string, { size = "$kitSizeLabel", color = ref(c, "faint") })
    end
    t[#t + 1] = row(row_t)
  end
  return column(t)
end

local TONE_ICON: {string:string} = { ok = "check", warn = "alert", danger = "alert", accent = "info", info = "info", neutral = "info" }
-- The bar down a toast's edge, by tone: info (the default) in the skin's
-- focus blue, neutral the soft rule. A toast with no tone was grey, its
-- bar the rule colour, and read as something gone wrong with the toast.
local TONE_BAR: {string:string} = { ok = "ok", warn = "warn", danger = "danger", accent = "accent", info = "focus", neutral = "rule" }

local record Shown
  message: string
  since: number
end

-- How long a toast stays when the host says nothing, in ms.
local TOAST_MS = 5000

-- A row with the skin's focus ring (`ctx.ring`), by the key it carries.
local function ringed(c: Ctx, t: kui.Props): kui.Node
  ctx.ring(c, t, t.key as string)
  return row(t)
end

function M.toast(c: Ctx, p: P): kui.Node
  local tone = str(p.tone) or "info"
  local message = str(p.message) or ""
  local role = (tone == "ok" or tone == "warn" or tone == "danger") and tone
    or (tone == "accent" and "accent" or (tone == "info" and "focus" or "muted"))
  -- It goes by itself after `duration` ms (0 keeps it), counted from when
  -- its message first showed and again from when the pointer leaves it:
  -- a frame asked for at the deadline, and in that frame a node whose
  -- first layout raises `dismiss` as its close button would.
  local env = c.env
  local now = env.now or 0
  local st = ctx.state(c, function(): Shown return {} end)
  if st.message ~= message or not st.since then st.message, st.since = message, now end
  local ok, over = pcall(env.is_hovered, "toast")
  if ok and over then st.since = now end
  local ms = (p.duration or TOAST_MS) as number
  local expire: kui.Node = nil
  if ms > 0 then
    local due = st.since + ms / 1000
    if now >= due then
      expire = column { key = "expire", width = 0, height = 0, on_layout = act(c, "toast.expire") }
    else
      env.request_frame_at(due)
    end
  end
  local inline = p.inline == true
  return row(surface(c, {
    key = "toast",
    -- Its own bottom-right corner on the window's, 16 px in: with `self`
    -- left at its start the toast hung off past the window's corner. In a
    -- `toaster` it is one of a column there, not a float of its own, so
    -- several stack instead of lying on top of one another.
    float = not inline and { anchor = "viewport", at = { "end", "end" }, self = { "end", "end" }, dx = -16, dy = -16 } or nil,
    label = message,
    min_width = 320,
    max_width = 480,
    pad = { r = 12 },
    gap = 10,
    cross_align = "center",
    -- In from below, out to the side, fading both ways; the others in a
    -- toaster slide into the room it leaves.
    enter = { opacity = 0, dy = 12 },
    exit = { opacity = 0, dx = 32 },
    slide = inline or nil,
    transition = 180,
    -- The tone, as a bar down its leading edge in the tone's colour, and
    -- the icon in it: an error reads as one before a word of it is read.
    column { key = "tone", width = 4, height = "grow", bg = ref(c, TONE_BAR[tone] or "rule") },
    row { width = "grow", min_width = 0, gap = 10, pad = { y = 10 }, cross_align = "center",
      icons.node(c, TONE_ICON[tone] or "info", 14, role),
    -- Live on the message alone, so the close button stays a control.
    row { width = "grow", live = "polite", ctx.text(c, message, { wrap = "word" }) },
    -- One action on what just happened (Undo), quiet beside the message.
    str(p.action) and ringed(c, {
      key = "action", role = "button", label = str(p.action_name) or str(p.action), height = "$kitCtl", min_width = "fit",
      pad = { x = 8 }, cross_align = "center", radius = "$kitRadius", hover_bg = ref(c, "raised_hover"),
      pressed_bg = ref(c, "raised_press"), cursor = "pointer", on_click = act(c, "toast.action"),
      ctx.caption(c, str(p.action), { color = ref(c, "fg") }),
    }) or nil,
    ringed(c, {
      key = "dismiss", role = "button", label = "Dismiss", width = 20, height = 20, main_align = "center",
      cross_align = "center", radius = "$kitRadius", hover_bg = ref(c, "hover"), cursor = "pointer",
      on_click = act(c, "toast.dismiss"),
      icons.node(c, "x", 12, "muted"),
    }),
    },
    expire,
  }))
end

-- Toasts stacked in the window's corner, the newest at the foot: its
-- children are toasts placed with `inline`.
function M.toaster(c: Ctx, _p: P): kui.Node
  return column {
    key = "toaster",
    float = { anchor = "viewport", at = { "end", "end" }, self = { "end", "end" }, dx = -16, dy = -16 },
    gap = 8,
    cross_align = "end",
    -- A host that places the toaster only while it has toasts takes it
    -- away with the last one, and a node going with its parent skips its
    -- own exit (DX19): the last toast vanished where the others slid
    -- out. The toaster leaves as a toast does, with whatever is in it.
    exit = { opacity = 0, dx = 32 },
    transition = 180,
    children(""),
  }
end

function M.event(m: P, kit: string): any
  if kit == "dialog.dismiss" or kit == "popover.dismiss" or kit == "menu.dismiss" or kit == "toast.dismiss"
    or kit == "toast.expire" then
    return emit("on_dismiss", {})
  elseif kit == "toast.action" then
    return emit("on_action", {})
  elseif kit == "menu.choose" then
    return emit("on_choose", { value = m.value })
  end
  return nil
end

return M
