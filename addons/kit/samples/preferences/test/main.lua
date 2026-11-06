local test=require('kui.test')
test.it('preferences save and validation',function()
  local dir=kui.fs.temp_dir { prefix='kui-sample-' }
  local app=test.start { args={ '--config', dir..'/settings.toml' } }
  app:task([[return require('fixtures.scenario')()]])
end)
