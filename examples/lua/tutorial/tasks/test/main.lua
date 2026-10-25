-- ANCHOR: test
-- The task list without a window, against a file in the temp folder.
--
--   kui run test --headless          (from the app's folder)
--
-- The exit code is the number of failures.

package.path = kui.dir .. "/../?.lua;" .. package.path
local path = os.tmpname() .. "-tasks.json"
kui.args = { "--file", path }
require("main")
model = init()

local failed = 0
local function check(ok, what)
  print((ok and "ok    " or "FAIL  ") .. what)
  if not ok then failed = failed + 1 end
end

local function saved()
  local t = kui.now()
  while model.saving do
    if kui.now() - t > 3000 then return false end
    kui.sleep(10)
  end
  return true
end

kui.spawn(function()
  update({ kind = "add" })
  check(model.error == "Name the task first", "an empty task is refused, with why")

  update({ kind = "draft", change = { text = "  Water the ferns " } })
  update({ kind = "add" })
  update({ kind = "draft", change = { text = "Call the dentist" } })
  update({ kind = "add" })
  check(#model.tasks == 2 and model.tasks[1].text == "Water the ferns", "two tasks, trimmed")
  check(model.draft == "" and model.error == nil, "the field is empty again")

  update({ kind = "toggle", index = 1, toggle = { checked = true } })
  check(done_count() == 1, "one done")
  check(saved(), "saved")
  check(#kui.json.decode(kui.fs.read(path)) == 2, "the file has both")

  update({ kind = "clear" })
  check(saved() and #model.tasks == 1, "clear done leaves one")
  check(#init().tasks == 1, "and a fresh start reads it back")

  print(("%s: %d failure(s)"):format(failed == 0 and "PASS" or "FAIL", failed))
  kui.exit(failed)
end)
-- ANCHOR_END: test
