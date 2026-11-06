-- ANCHOR: test
local test = require('kui.test')
local path

test.before_each(function()
  path = kui.fs.temp_dir() .. '/tasks.json'
  test.start { args = { '--file', path } }
end)

test.it('an empty task explains what is missing', function()
  test.press('Add')
  test.expect(test.model().error):to_be('Name the task first')
end)

test.it('tasks can be added, completed, saved and reopened', function()
  test.type('New task', '  Water the ferns  ')
  test.press('Add')
  test.type('New task', 'Call the dentist')
  test.press('Add')
  test.expect(#test.model().tasks):to_be(2)
  test.expect(test.model().tasks[1].text):to_be('Water the ferns')

  test.press('Water the ferns')
  test.press('Clear done')
  test.wait_for(function() return not test.model().saving end)
  test.expect(#kui.json.decode(kui.fs.read(path))):to_be(1)

  test.start { args = { '--file', path } }
  test.expect(test.model().tasks[1].text):to_be('Call the dentist')
end)
-- ANCHOR_END: test
