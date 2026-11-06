-- The runner's test module (ADR 0072). `require("kui.test")`.
local type Predicate = function(): boolean
local record test
  record Expectation
    never: Expectation
    to_be: function(self: Expectation, expected: any): Expectation
    to_equal: function(self: Expectation, expected: any): Expectation
    to_be_near: function(self: Expectation, expected: number, tolerance?: number): Expectation
    to_contain: function(self: Expectation, expected: any): Expectation
    to_match: function(self: Expectation, pattern: string): Expectation
    to_throw: function(self: Expectation, text?: string): Expectation
    to_have_been_called: function(self: Expectation, times?: integer): Expectation
    to_have_been_called_with: function(self: Expectation, ...: any): Expectation
  end
  record Size
    w: number
    h: number
  end
  record StartOpts
    args: {string}
    size: Size
  end
  record App
    frames: function(self: App, count?: integer)
    advance: function(self: App, milliseconds: number)
    press: function(self: App, target: string | integer)
    click: function(self: App, target: any, y?: number)
    type: function(self: App, label_or_text: string, text?: string)
    key: function(self: App, name: string, mods?: {string:boolean})
    resize: function(self: App, width: number, height: number)
    dispatch: function(self: App, message: any)
    nodes: function(self: App): {any}
    model: function(self: App): any
    text: function(self: App, key_label: string): {string}
    warnings: function(self: App): {string}
    eval: function(self: App, source: string): any
    task: function(self: App, source: string, timeout_ms?: number): any
    expect_tree: function(self: App, name: string)
    expect_quads: function(self: App, name: string)
    expect_shot: function(self: App, name: string, opts?: {string:any})
    expect_table: function(self: App, name: string, value: any)
  end
  describe: function(name: string, fn: function())
  it: function(name: string, fn: function())
  before_each: function(fn: function())
  after_each: function(fn: function())
  expect: function(value: any): Expectation
  start: function(opts?: StartOpts): App
  frames: function(count?: integer)
  advance: function(milliseconds: number)
  press: function(target: string | integer)
  click: function(target: any, y?: number)
  type: function(label_or_text: string, text?: string)
  key: function(name: string, mods?: {string:boolean})
  resize: function(width: number, height: number)
  dispatch: function(message: any)
  nodes: function(): {any}
  model: function(): any
  text: function(key_label: string): {string}
  warnings: function(): {string}
  eval: function(source: string): any
  task: function(source: string, timeout_ms?: number): any
  step: function(name: string, fn: function(): any): any
  pause: function()
  wait_for: function(condition: Predicate, timeout_ms?: number)
  expect_tree: function(name: string)
  expect_quads: function(name: string)
  expect_shot: function(name: string, opts?: {string:any})
  expect_table: function(name: string, value: any)
  mock: function(service: string, rule: {string:any})
  mock_http: function(rule: {string:any})
  mock_process: function(rule: {string:any})
  mock_fs: function(rule: {string:any})
  mock_sqlite: function(rule: {string:any})
  mock_secrets: function(rule: {string:any})
  calls: function(service?: string): {any}
  spy: function(target: any, key?: string): any
  stub: function(target: any, key: string, replacement: any): any
  replay: function(adapter: {string:any})
end
return test
