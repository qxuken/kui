local test = require('kui.test')
test.it('applies a 20% discount to two $10 items', function()
  require('fixtures.checkout_case')(false)
end)
