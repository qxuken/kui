function view(env)
  local t = env.theme
  return column {
    width = "grow", height = "grow", pad = 24, gap = 16, bg = t.bg,
    text("Make room", { size = 26, color = t.fg }),
    text("Try changing the gap, padding, and widths.", { color = t.muted }),
    row { width = "grow", height = "grow", gap = 12,
      column { width = 140, height = "grow", pad = 16, gap = 8,
        bg = t.surface, radius = 12,
        text("Sidebar", { color = t.fg }),
        text("A fixed width", { color = t.muted }),
      },
      column { width = "grow", height = "grow", pad = 16, gap = 8,
        bg = t.surface, radius = 12,
        text("Content", { color = t.fg }),
        text("Takes the remaining space", { color = t.muted }),
      },
    },
  }
end
