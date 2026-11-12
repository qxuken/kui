-- Click the drawn application menu used on Windows and Linux.
local test = require("kui.test")

test.it("File opens the recording picker", function()
  test.start()
  -- The headless host answers the file dialog with a cancellation.
  test.eval([[
    local handle = update
    function update(msg)
      if msg.kind == "picked" then picker_answered = true end
      handle(msg)
    end
  ]])
  test.click("File")
  test.click("Open…")
  test.frames(1)
  test.expect(test.eval("return picker_answered")):to_be(true)
end)

test.it("View toggles Live and reads pending frames when resumed", function()
  local path = assert(os.getenv("KUI_PERF_TEST_GROWING"))
  test.start { args = { path } }
  test.click("Got it")
  test.click("View")
  test.click("Live")
  test.expect(test.model().live):to_be(false)
  test.expect(test.eval([[
    local rest = assert(os.getenv("KUI_PERF_TEST_REST"))
    frames_before = #model.ins.rec.frames
    kui.fs.append(model.path, kui.fs.read(rest))
    update({ kind = "moved" })
    return model.waiting > 0 and #model.ins.rec.frames == frames_before
  ]])):to_be(true)
  test.click("View")
  test.click("Live")
  test.expect(test.model().live):to_be(true)
  test.expect(test.eval("return model.waiting == 0 and #model.ins.rec.frames > frames_before")):to_be(true)
end)

test.it("Help opens the keys card", function()
  test.start()
  test.click("Help")
  test.click("Keys and gestures")
  test.expect(test.model().help):to_be(true)
end)

test.it("View navigates and Open Recent reopens the recording", function()
  local path = assert(os.getenv("KUI_PERF_TEST_RECORDING"))
  test.start { args = { path } }
  test.click("Got it")
  test.key("w")
  test.expect(test.eval("return model.ins.follow")):to_be(false)
  test.click("View")
  test.click("Go to the latest")
  test.expect(test.eval([[
    local ins = model.ins
    return ins.follow and ins.v0 > ins.session.start and math.abs(ins.v1 - ins.session.finish) < 1e-9
  ]])):to_be(true)
  test.click("View")
  test.click("Fit the recording")
  test.expect(test.eval([[
    local ins = model.ins
    return ins.v0 == ins.session.start and ins.v1 == ins.session.finish
  ]])):to_be(true)

  -- A new inspector starts with no frame selected.
  test.key("l")
  test.expect(test.eval("return model.ins.frame")):to_be(0)
  test.click("File")
  test.click("Open Recent")
  test.click(path:match("[^/\\]+$"))
  test.expect(test.eval("return model.ins.frame")):to_be(-1)
end)
