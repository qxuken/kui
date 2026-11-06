local kit = require('kit.ui')
function init() return { count = 0, reply = nil } end
function update(msg)
  if msg.kind == 'add' then model.count=model.count+1
  elseif msg.kind == 'later' then kui.timer.after(500,function() kui.dispatch {kind='add'} end)
  elseif msg.kind == 'fetch' then kui.spawn(function() local response=kui.http.get('https://example.test/count'); kui.dispatch {kind='reply',body=response.body} end)
  elseif msg.kind == 'reply' then model.reply=msg.body end
end
function view(env)
  return column { width='grow', gap=8,
    kit.button { label='Increment',on_press={kind='add'} },
    text('Count: '..model.count),
  }
end
