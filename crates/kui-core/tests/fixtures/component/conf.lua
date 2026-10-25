-- The extension the conformance corpus's `component` scene places
-- (docs/adr/0060). Every adapter but Rust's loads this file through its own
-- door; the reference spells the same extension again in Rust
-- (`kui_core::conformance::ConfKit`), and the two must draw the same.
closes = 0

components = {
  card = function(env, props)
    return column { pad = 6, gap = 4, bg = 0x202634ff, radius = 6, width = 200,
      text(props.title or "card", { size = 14, color = 0xe8e8f0ff }),
      children(),
      row { gap = 4,
        row { key = "close", width = 16, height = 16, bg = 0x7f9cf5ff, label = "Close",
              on_click = { tag = "close" } },
        children("footer"),
      },
    }
  end,
}

function on_event(ev)
  if ev.tag == "close" then
    closes = closes + 1
    return emit("onClose", { count = closes })
  end
end
