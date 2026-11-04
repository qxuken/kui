-- Reusable reading content: usable in an app, a deck or a document view.
local ctx = require("ctx")
local type Ctx = ctx.Ctx
local type P = {string:any}
local M = {}

local function str(v: any): string return v is string and v or nil end
local function number(v: any): number return v is number and v or nil end

function M.heading(c: Ctx, p: P): kui.Node
  local level = math.max(1, math.min(6, (p.level or 1) as number))
  local size: kui.Length = number(p.size) or (level == 1 and "$kitHeading" or level == 2 and "$kitSizeTitle" or "$kitContentSize")
  return column { width = "grow", role = "heading", label = str(p.text),
    ctx.text(c, str(p.text) or "", { size = size, family = str(p.family) or "$kitTitle",
      color = p.color or ctx.ref(c, "fg"), bold = p.bold ~= false, wrap = "word", line_height = number(p.line_height) }) }
end

function M.paragraph(c: Ctx, p: P): kui.Node
  return column { width = "grow", selectable = p.selectable == true,
    ctx.text(c, str(p.text) or "", { size = number(p.size) or "$kitContentSize",
    family = str(p.family) or "$kitContent", color = p.color or ctx.ref(c, "fg"),
    wrap = "word", line_height = number(p.line_height) }) }
end

function M.content(c: Ctx, p: P): kui.Node
  local centered = p.preset == "title" or p.preset == "section"
  local lead: kui.Props = { width = "grow", gap = "$kitContentGap" }
  if str(p.eyebrow) then lead[#lead + 1] = ctx.muted(c, p.eyebrow as string, { size = "$kitCaptionSize" }) end
  if str(p.title) then lead[#lead + 1] = M.heading(c, { text = p.title }) end
  if str(p.subtitle) then lead[#lead + 1] = M.paragraph(c, { text = p.subtitle, color = ctx.ref(c, "muted") }) end
  local layout = str(p.layout)
  local main: kui.Node
  if layout == "editorial" or layout == "split" then
    lead[#lead + 1] = children("")
    main = row { width = "grow", gap = "$kitContentGap", cross_align = "center",
      when = { { width = { below = 760 }, dir = "column" } },
      column(lead), column { width = "grow", pad = "$kitContentPad",
        bg = layout == "split" and ctx.ref(c, "accent") or ctx.ref(c, "panel"),
        gap = "$kitContentGap", children("aside") } }
  elseif layout == "briefing" then
    main = row { width = "grow", gap = "$kitContentGap",
      when = { { width = { below = 900 }, dir = "column" } },
      column { width = 400, max_width = "100%", gap = "$kitContentGap", column(lead) },
      column { width = "grow", gap = "$kitContentGap", children(""), children("aside") } }
  else
    lead[#lead + 1] = children("")
    lead[#lead + 1] = children("aside")
    main = column(lead)
  end
  return column { width = "grow", height = p.grow == true and "grow" or nil,
    max_width = number(p.max_width), pad = number(p.pad) or "$kitContentPad",
    gap = number(p.gap) or "$kitContentGap", bg = p.bg as kui.Color,
    main_align = centered and "center" or "start", main }
end

function M.columns(_c: Ctx, p: P): kui.Node
  return row { width = "grow", gap = number(p.gap) or "$kitContentGap",
    when = { { width = { below = number(p.collapse_at) or 760 }, dir = "column" } },
    column { width = "grow", gap = "$kitContentGap", children("first") },
    column { width = "grow", gap = "$kitContentGap", children("second") } }
end

function M.comparison(c: Ctx, p: P): kui.Node
  return row { width = "grow", gap = "$kitContentGap",
    when = { { width = { below = number(p.collapse_at) or 760 }, dir = "column" } },
    column { width = "grow", gap = "$kitContentGap", pad = "$kitContentPad", bg = ctx.ref(c, "panel"),
      M.heading(c, { text = p.first_title or "Before", level = 2 }), children("first") },
    column { width = "grow", gap = "$kitContentGap", pad = "$kitContentPad", bg = ctx.ref(c, "panel"),
      M.heading(c, { text = p.second_title or "After", level = 2 }), children("second") } }
end

function M.quote(c: Ctx, p: P): kui.Node
  return row { width = "grow", gap = "$kitContentGap",
    column { width = 3, height = "grow", bg = ctx.ref(c, "accent") },
    column { width = "grow", gap = 12,
      M.paragraph(c, { text = p.text, size = p.size, family = "$kitTitle" }),
      str(p.attribution) and ctx.muted(c, p.attribution as string, { size = "$kitCaptionSize" }) or nil } }
end

function M.figure(c: Ctx, p: P): kui.Node
  return column { width = "grow", gap = 12, children(""),
    str(p.caption) and ctx.muted(c, p.caption as string, { size = "$kitCaptionSize", wrap = "word" }) or nil }
end

function M.visibility(_c: Ctx, p: P): kui.Node
  local shown = p.visible ~= false
  -- Scale zero retains layout but produces no paint or hit regions. Role
  -- none excludes the entire subtree from accessibility and the Tab ring.
  -- No scale tween: input must disappear on the same frame as visibility.
  return column { width = "grow", hidden = not shown and p.collapse == true,
    scale = shown and 1 or 0, role = not shown and "none" or nil, children("") }
end

return M
