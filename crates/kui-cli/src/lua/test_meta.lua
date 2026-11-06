---@meta
---Runner tests: `local test = require("kui.test")`.
---@class kui.TestExpectation
---@field never kui.TestExpectation
local expectation = {}
---@param expected any
---@return kui.TestExpectation
function expectation:to_be(expected) end
---@param expected any
---@return kui.TestExpectation
function expectation:to_equal(expected) end
---@return kui.TestExpectation
function expectation:to_be_near(expected, tolerance) end
---@return kui.TestExpectation
function expectation:to_contain(expected) end
---@return kui.TestExpectation
function expectation:to_match(pattern) end
---@return kui.TestExpectation
function expectation:to_throw(text) end
function expectation:to_have_been_called(times) end
function expectation:to_have_been_called_with(...) end
---@class kui.Test
local test = {}
function test.describe(name, fn) end
function test.it(name, fn) end
function test.before_each(fn) end
function test.after_each(fn) end
---@param value any
---@return kui.TestExpectation
function test.expect(value) end
---Starts a fresh app from its manifest after setting args and temporary directories.
---@param opts? {args?: string[], size?: {w: number, h: number}}
---@return kui.Test
function test.start(opts) end
function test.frames(count) end
function test.advance(milliseconds) end
function test.press(target) end
function test.click(target, y) end
function test.type(label_or_text, text) end
function test.key(name, mods) end
function test.resize(width, height) end
function test.dispatch(message) end
---@return table[]
function test.nodes() end
---@return any
function test.model() end
---@return string[]
function test.text(key_label) end
---@return string[]
function test.warnings() end
---Executes source in the app's state; returned values are detached inspection copies.
---@return any
function test.eval(source) end
---Runs app source as a task and drives frames until it completes.
---@return any
function test.task(source, timeout_ms) end
function test.step(name, fn) end
function test.pause() end
function test.wait_for(condition, timeout_ms) end
function test.expect_tree(name) end
function test.expect_quads(name) end
---@param opts? {tolerance?: integer}
function test.expect_shot(name, opts) end
function test.expect_table(name, value) end
function test.mock(service, rule) end
function test.mock_http(rule) end
function test.mock_process(rule) end
function test.mock_fs(rule) end
function test.mock_sqlite(rule) end
function test.mock_secrets(rule) end
---@return table[]
function test.calls(service) end
function test.spy(target, key) end
function test.stub(target, key, replacement) end
---Registers the app's state capture and a side-effect-free renderer module.
---@param adapter {capture: fun():any, renderer:string}
function test.replay(adapter) end
return test
