-- The stamp plugin of step 11: one button, first in `app.toolbar`, that
-- counts its presses.

local pressed = 0

contributes = { { slot = "app.toolbar", component = "stamp", order = -1 } }

components = {
  stamp = function(env, props, inst)
    return button { label = "Stamp (" .. pressed .. ")", on_click = { kind = "stamp" } }
  end,
}

function on_event(ev)
  if ev.kind == "stamp" then
    pressed = pressed + 1
    return emit("on_run", { action = "stamp" })
  end
end
