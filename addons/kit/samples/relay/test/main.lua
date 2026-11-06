local test=require('kui.test')
test.it('real HTTP requests and history',function()
  local app=test.start { args={ '--port', '0' } }
  app:task([[return require('fixtures.scenario')()]])
end)
