local kit = require('kit.ui')
local faulty = kui.args[1] == '--faulty-discount'
local function money(cents) return string.format('$%.2f', cents / 100) end

function init()
  return { items = 0, subtotal_cents = 0, discount_percent = 0,
    total_cents = 0, status = 'idle', faulty_discount = faulty }
end

function update(msg)
  if msg.kind == 'add_item' then
    model.items = model.items + 1
    model.subtotal_cents = model.items * 1000
    model.total_cents = model.subtotal_cents
    model.status = 'idle'
    print('Added item; subtotal is '..money(model.subtotal_cents))
  elseif msg.kind == 'checkout' then
    model.status = 'loading'
    local items = model.items
    kui.spawn(function()
      local response = kui.http.get('https://checkout.example.test/discount?items='..items)
      kui.dispatch { kind = 'discount', percent = response.json.discount_percent }
    end)
  elseif msg.kind == 'discount' then
    model.discount_percent = msg.percent
    -- The explicit fault flag simulates a regression without breaking normal runs.
    model.total_cents = faulty and model.subtotal_cents
      or math.floor(model.subtotal_cents * (100 - msg.percent) / 100 + 0.5)
    model.status = 'ready'
    kui.log.info('Discount '..msg.percent..'%; checkout total is '..money(model.total_cents))
  end
end

local function summary(key, name, value)
  return row {key=key,width='grow',gap=12,text(name,{size=14,color='$kitMuted'}),column{width='grow'},text(value,{size=16,bold=true,color='$kitFg'})}
end

function view()
  return column { width='grow', height='grow', pad=24, gap=16, bg='$kitBg',
    kit.settings { skin='neutral' },
    text('Checkout', {size=26, bold=true, color='$kitFg'}),
    text('Two $10 items with a 20% loyalty discount.', {size=14, color='$kitMuted'}),
    faulty and kit.notice {tone='warn',message='Fault injection: the checkout receives the discount but ignores it.'} or nil,
    row {gap=12,
      kit.button {label='Add $10 item',on_press={kind='add_item'}},
      kit.button {label='Apply discount',variant='primary',disabled=model.items==0 or model.status=='loading',on_press={kind='checkout'}},
    },
    summary('items','Items',tostring(model.items)),
    summary('subtotal','Subtotal',money(model.subtotal_cents)),
    summary('discount','Discount',tostring(model.discount_percent)..'%'),
    summary('total','Total',money(model.total_cents)),
    text('Status: '..model.status,{size=13,color='$kitMuted'}),
  }
end
