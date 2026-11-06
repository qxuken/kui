-- Opt-in failing test: fixtures/ is excluded from normal discovery.
local test = require('kui.test')
test.it('DEMO FAILURE · checkout ignores the loyalty discount', function()
  require('fixtures.checkout_case')(true)
end)
