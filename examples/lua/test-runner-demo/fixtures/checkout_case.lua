-- A real interaction, asynchronous native mock, state assertions and output.
return function(faulty)
  local test = require('kui.test')
  test.mock_http {
    url = 'https://checkout.example.test/discount?items=2',
    response = { json = { discount_percent = 20 } }, delay = 100,
  }
  local app = test.start {args = faulty and {'--faulty-discount'} or {}}
  test.step('Add two items', function()
    app:press('Add $10 item')
    app:press('Add $10 item')
    test.expect(app:model().subtotal_cents):to_be(2000)
  end)
  test.step('Apply the loyalty discount', function()
    app:press('Apply discount')
    test.wait_for(function() return app:model().status == 'ready' end)
  end)
  test.step('Check the discounted total', function()
    -- Intentional fault mode returns 2000: inspect this assertion's frame,
    -- model.discount_percent, the HTTP reply and Output to see why.
    test.expect(app:model().total_cents):to_be(1600)
  end)
end
