local test=require('kui.test')
test.it('dashboard state and layouts',function()
  local app=test.start { args={} }
  app:task([[return require('fixtures.scenario')()]])
end)
