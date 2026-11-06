local test=require('kui.test')
test.it('notebook persistence',function()
  local dir=kui.fs.temp_dir { prefix='kui-sample-' }
  local app=test.start { args={ '--db', dir..'/notes.db' } }
  app:task([[return require('fixtures.scenario')()]])
end)
