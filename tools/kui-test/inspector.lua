-- Immutable run history and live controls share the same selection model.
local kit=require('kit.ui')
local I={}
local bridge=kui.test_inspector
local function pretty(v) return kui.json.encode(v,{pretty=true}) end
local function label(s,opts)
  local p={size='$kitSizeLabel',family='$kitUi',color='$kitMuted',max_lines=1,ellipsis=true}
  for k,v in pairs(opts or {}) do p[k]=v end
  return text(tostring(s or ''),p)
end
local function title(s,size) return label(s,{size=size or 16,bold=true,color='$kitFg'}) end
local function button(s,kind,extra,disabled,variant)
  local msg={kind=kind};for k,v in pairs(extra or {}) do msg[k]=v end
  return kit.button{label=s,on_press=msg,disabled=disabled,variant=variant or 'ghost'}
end
local function path_controls(path,line)
  if not path or path=='' then return nil end
  local name=path:match('[^/\\]+$') or path
  return row{width='grow',gap=6,
    column{width='grow',tooltip=path,label((line and name..':'..line..' · ' or '')..path,{max_lines=1})},
    button('Copy path','copy_path',{path=path}),button('Reveal','path_action',{action='reveal',path=path}),button('Open in editor','path_action',{action='editor',path=path,line=line})}
end
local function short(s) return tostring(s or ''):match('[^\n]+') or 'Unknown error' end
local function message(s) local line=short(s);return line:match('%.lua:%d+:%s*(.*)') or line:match('%.tl:%d+:%s*(.*)') or line end
local function basename(s) return tostring(s or ''):match('[^/\\]+$') or '' end
local function attempt(m,fn)
  local ok,out=pcall(fn)
  if not ok then m.error=short(tostring(out):gsub('^runtime error: ','')) else m.error=nil end
  return ok,out
end
local selected_payload
local function busy(m) return m.pending or m.data.busy or (m.data.run or {}).status=='running' end
local function fraction(value,minimum,maximum,default)
  if type(value)~='number' or value~=value or value==math.huge or value==-math.huge then return default end
  return math.max(minimum,math.min(maximum,value))
end
function I.new(saved)
  saved=type(saved)=='table' and saved or {}
  return {data={files={}},query='',tab='state',selected=1,node=0,live=true,mode='after',sidebar='steps',layout={tests=fraction(saved.tests,0.10,0.45),actions=fraction(saved.actions,0.10,0.45),preview=fraction(saved.preview,0.15,0.85,0.62)}}
end
local function frames(m)
  local list={}
  for _,f in ipairs(m.data.files or {}) do for _,frame in ipairs(f.frames or {}) do
    if (not m.case or frame.case==m.case) and (not m.case_file or (f.collection or {}).file==m.case_file) then list[#list+1]=frame end
  end end;return list
end
local function selected(m) return frames(m)[m.selected] end
local function active_file(m)
  if selected(m) then return selected(m).file end
  for _,f in ipairs(m.data.files or {}) do if not m.case_file or (f.collection or {}).file==m.case_file then return f.name end end
end
local function running_file(m)
  if not busy(m) then return nil end
  for i=#(m.data.files or {}),1,-1 do local f=m.data.files[i];if not (f.results or {}).complete then return f.name end end
end
local function load_output(m)
  local file=active_file(m)
  if file and bridge.output then local ok,out=attempt(m,function()return bridge.output(file)end);if ok then m.output=out end end
end
local function select(m,index)
  m.payloads=nil
  local list=frames(m);m.selected=math.max(1,math.min(index,#list));m.node=0;m.override=nil;m.snapshot=nil
  local frame=list[m.selected];m.detail=nil;m.before=nil;m.diffs={}
  if frame then
    local ok,detail=attempt(m,function()return bridge.detail(frame.file,frame.path)end)
    if ok then
      m.detail=detail
      for i=m.selected-1,1,-1 do local prior=list[i]
        if prior.file==frame.file and prior.case==frame.case and ((prior.phase=='before' and prior.step_id==frame.step_id) or (frame.step=='case end' and prior.phase=='after')) then
          local good,before=attempt(m,function()return bridge.detail(prior.file,prior.path)end)
          if good then m.before=before
            for _,field in ipairs({'state','core','kit'}) do local fine,diff=pcall(bridge.changes,frame.file,prior.path,frame.path,field);if fine then m.diffs[field]=diff end end
          end;break
        end
      end
    end
  end
  if m.tab=='output' then load_output(m) end
end
local function preferred_frame(m)
  local list=frames(m)
  if not busy(m) then for i=#list,1,-1 do local check=list[i].expectation or list[i].assertion;if check and check.ok==false then return i end end end
  return #list
end
local function refresh(m)
  local old=selected(m);local n=#frames(m);local old_run=m.run_id;local initial=not m.loaded;m.loaded=true
  local ok,data=pcall(bridge.read);if not ok then m.error=short(data);return end
  m.data=data;m.run_id=(data.run or {}).id
  if m.pending and (data.busy or (data.run or {}).status=='running' or m.run_id~=old_run) then m.pending=false end
  if (data.run or {}).error then m.error=short(data.run.error) end
  if initial or old_run~=m.run_id then
    m.snapshot=nil;m.output=nil
    if not m.case and not m.all then
      local chosen,file
      for _,f in ipairs(data.files or {}) do for _,c in ipairs((f.collection or {}).cases or {}) do
        if not chosen then chosen=c;file=(f.collection or {}).file end
        for _,r in ipairs((f.results or {}).cases or {}) do if r.name==c.name and r.ok==false then chosen=c;file=(f.collection or {}).file end end
      end end
      if chosen then m.case=chosen.name;m.case_file=file end
    end
    for _,f in ipairs(data.files or {}) do for _,r in ipairs((f.results or {}).cases or {}) do
      if r.ok==false then local known=false;for _,c in ipairs((f.collection or {}).cases or {}) do if c.name==r.name then known=true end end;if not known then m.tab='errors' end end
    end end
    select(m,m.live and preferred_frame(m) or 1)
  elseif m.live and #frames(m)~=n then select(m,preferred_frame(m))
  elseif old then
    for i,f in ipairs(frames(m)) do if f.file==old.file and f.path==old.path then m.selected=i;if m.tab=='output' then load_output(m) end;return end end
    select(m,m.selected)
  elseif #frames(m)>0 then select(m,1) end
  if m.pending_run and not busy(m) and data.can_run then
    m.pending_run=false;local good,started=attempt(m,function()return bridge.rerun(nil,false,false,false)end);if good and started~=false then m.pending=true end
  end
end
local function diagnostics(m)
  local out={}
  if (m.data.run or {}).error then out[#out+1]={severity='error',message=m.data.run.error,case='Runner'} end
  for _,f in ipairs(m.data.files or {}) do if not m.case_file or (f.collection or {}).file==m.case_file then
    for _,d in ipairs(f.diagnostics or {}) do
      local known=false;for _,c in ipairs((f.collection or {}).cases or {}) do if c.name==d.case then known=true end end
      if not m.case or not d.case or d.case==m.case or not known then out[#out+1]=d end
    end
    if not f.diagnostics then for _,c in ipairs((f.results or {}).cases or {}) do if c.ok==false and (not m.case or c.name==m.case) then out[#out+1]={severity='error',case=c.name,message=c.error,source={file=c.file,line=c.line}} end end end
  end end;return out
end
function I.update(m,msg)
  local k=msg.kind
  if k=='keys' and msg.key and msg.key.phase=='down' and not msg.key.ctrl and not msg.key.super then
    if msg.key.code=='left' then m.live=false;select(m,m.selected-1) elseif msg.key.code=='right' then m.live=false;select(m,m.selected+1) end
  elseif k=='refresh' then refresh(m)
  elseif k=='source_changed' then m.pending_run=true;refresh(m)
  elseif k=='query' then m.query=msg.change.text
  elseif k=='tab' then m.tab=msg.change.value;if m.tab=='output' then load_output(m) end
  elseif k=='resize_tests' or k=='resize_actions' or k=='resize_preview' then
    local field=k:sub(8);m.layout[field]=fraction(msg.resize and msg.resize.fraction,field=='preview' and 0.15 or 0.10,field=='preview' and 0.85 or 0.45,m.layout[field])
  elseif k=='reset_layout' then m.layout={preview=0.62}
  elseif k=='mode' then m.mode=msg.change.value
  elseif k=='sidebar' then m.sidebar=msg.change.value
  elseif k=='event' then m.event=msg.select.row+1
  elseif k=='case' then m.all=false;m.case=msg.name;m.case_file=msg.file;m.live=false;select(m,1);if msg.failed then m.tab='errors' end
  elseif k=='all' then m.all=true;m.case=nil;m.case_file=nil;select(m,1)
  elseif k=='select' then m.live=false;select(m,msg.index)
  elseif k=='previous' then m.live=false;select(m,m.selected-1)
  elseif k=='next_frame' then m.live=false;select(m,m.selected+1)
  elseif k=='latest' then m.live=true;select(m,#frames(m))
  elseif k=='node' then m.node=msg.index
  elseif k=='dismiss' then m.error=nil
  elseif k=='copy_path' then m.clipboard=msg.path;m.notice='Path copied'
  elseif k=='path_action' then attempt(m,function()bridge.path_action(msg.action,msg.path,msg.line)end)
  elseif k=='path_error' then m.error=msg.error
  elseif k=='markers' then m.hide_markers=not m.hide_markers
  elseif k=='copy_payload' then local p=(m.payloads or {})[msg.key];m.clipboard=p and p.source
  elseif k=='copy_payload_range' then m.clipboard=selected_payload((m.payloads or {})[msg.key],msg.selectionrange)
  elseif k=='stack' then m.stacks=not m.stacks
  elseif k=='snapshot' then local ok,v=attempt(m,function()return bridge.snapshot(msg.file,msg.index)end);if ok then m.snapshot=v;m.tab='assertions' end
  elseif k=='pick_pixel' and m.detail and msg.pick then
    m.node=0;for i,n in ipairs(m.detail.tree or {}) do local r=n.rect;if r and msg.pick.x>=r.x and msg.pick.y>=r.y and msg.pick.x<r.x+r.w and msg.pick.y<r.y+r.h then m.node=i end end;m.tab='tree'
  elseif k=='run' or k=='run_all' or k=='headed' or k=='debug' or k=='update' then
    if busy(m) or not m.data.can_run then return end
    local filter=k=='run_all' and nil or m.case or (m.query~='' and m.query or nil)
    local ok,started=attempt(m,function()return bridge.rerun(filter,k=='headed' or k=='debug',k=='debug',k=='update',k~='run_all' and m.case_file or nil)end)
    if ok and started~=false then m.pending=true;m.live=true end
  elseif k=='control' then local f=running_file(m);if f then attempt(m,function()bridge.command(f,msg.command,msg.enabled)end) end
  elseif k=='perf' then local f=selected(m);if f then attempt(m,function()bridge.perf(f.file,f.path)end) end
  elseif k=='drawing' or k=='render_state' then local f=selected(m);if f then local ok,path=attempt(m,function()return bridge[k](f.file,f.path)end);if ok then m.override=path end end end
end
local function counts(m)
  local pass,fail,total=0,0,0
  for _,f in ipairs(m.data.files or {}) do
    total=total+#((f.collection or {}).cases or (f.results or {}).cases or {})
    for _,c in ipairs((f.results or {}).cases or {}) do if c.ok then pass=pass+1 else fail=fail+1 end end
  end;return pass,fail,total
end
local function tests(m,width)
  local body={key='case-list',width='grow',height='grow',scroll_y=true,gap=5}
  for _,f in ipairs(m.data.files or {}) do
    local outcomes={};for _,c in ipairs((f.results or {}).cases or {}) do outcomes[c.name]=c end
    for _,c in ipairs((f.collection or {}).cases or (f.results or {}).cases or {}) do if m.query=='' or c.name:lower():find(m.query:lower(),1,true) then
      local r=outcomes[c.name];local chosen=m.case==c.name and (not m.case_file or m.case_file==(f.collection or {}).file)
      body[#body+1]=column{key=f.name..c.name,width='grow',pad=10,gap=6,radius=5,bg=chosen and '$kitPanel' or nil,on_click={kind='case',name=c.name,file=(f.collection or {}).file,failed=r and not r.ok},label=c.name,
        row{width='grow',gap=8,kit.badge{label=r and (r.ok and 'PASS' or 'FAIL') or (busy(m) and ((f.status or {}).paused and 'PAUSED' or 'RUNNING') or 'NOT RUN'),tone=r and (r.ok and 'ok' or 'danger') or 'neutral'},column{width='grow'},r and label(string.format('%.2f s',r.seconds)) or nil},
        label(c.name,{color='$kitFg',size=13,wrap='word',max_lines=3}),
        label(basename(c.file or (f.collection or {}).file)..(c.line and ':'..c.line or '')),
        r and r.error and label(message(r.error),{color='$kitDanger',wrap='word',max_lines=2}) or nil}
    end end
  end
  return column{key='tests-pane',width=width or 'grow',height='grow',pad=12,gap=12,bg='$kitSide',
    row{width='grow',title('Tests',15),column{width='grow'},button('All tests','all')},
    kit.search{key='search',value=m.query,label='Filter tests',placeholder='Filter tests…',on_change={kind='query'}},
    #body==0 and kit.empty{title='Collecting tests',detail='Cases appear as each file loads.'} or nil,column(body)}
end
local names={start='Launch app',press='Press control',click='Click',type='Type text',key='Press key',dispatch='Dispatch message',frames='Advance frames',advance='Advance time',task='Run app task',eval='Evaluate app code',resize='Resize viewport',expect_tree='Compare tree',expect_quads='Compare drawing',expect_shot='Compare screenshot',expect_table='Compare state',['case end']='Final state'}
local function action_name(frame)
  if not frame then return 'Test history' end
  local name=names[frame.step] or frame.step
  if frame.target and frame.target.label then name=name..' · '..frame.target.label end
  if frame.expectation then name=name..' · '..(frame.expectation.name or '') end
  return name
end
local function outcome(frame)
  local check=frame.expectation or frame.assertion
  return check and (check.ok and 'PASS' or 'FAIL') or nil
end
local function actions(m)
  local list=frames(m);local rows={};local seen={}
  for i,f in ipairs(list) do
    local key=f.file..':'..tostring(f.case)..':'..tostring(f.step_id or f.step)
    local action=seen[key]
    if not action then action={name=f.step,index=i,frame=f,count=0};seen[key]=action;rows[#rows+1]=action end
    action.count=action.count+1
    if f.phase=='after' or (f.phase=='intermediate' and action.frame.phase~='after') then action.index=i;action.frame=f end
    if i==m.selected then action.selected=true end
  end;return rows
end
local function steps(m,width)
  local body={key='action-list',width='grow',height='grow',scroll_y=true,gap=4}
  for i,a in ipairs(actions(m)) do
    body[#body+1]=column{key='action-'..i,width='grow',pad={x=10,y=9},gap=5,radius=5,bg=a.selected and '$kitPanel' or nil,on_click={kind='select',index=a.index},label='Step '..i..': '..action_name(a.frame),
      row{width='grow',gap=8,label(string.format('%02d',i),{color=a.selected and '$kitAccent' or '$kitMuted',family='$kitData'}),column{width='grow',label(action_name(a.frame),{color='$kitFg',size=13,max_lines=3,wrap='word'})},outcome(a.frame) and kit.badge{label=outcome(a.frame),tone=outcome(a.frame)=='FAIL' and 'danger' or 'ok'} or nil},
      row{width='grow',gap=8,label(string.format('%.1f ms',a.frame.time or 0)),column{width='grow'},label(a.count..(a.count==1 and ' frame' or ' frames'))}}
  end
  return column{key='steps-pane',width=width or 'grow',height='grow',bg='$kitSide',pad=12,gap=12,title('Actions',15),label(#actions(m)..' steps · select to inspect'),column(body)}
end
local function image(frame,width,height,path,highlight,annotation)
  if not frame or not (path or frame.shot_path) then return column{width='grow',height=height,main_align='center',cross_align='center',kit.empty{title='No captured frame',detail='Launch an app or select an action.'}} end
  return component('testmedia/image',{key=(frame.path or '')..(path or frame.shot_path),path=path or frame.shot_path,width=width,height=height,logical_width=frame.logical_width,logical_height=frame.logical_height,highlight=highlight,target=annotation and annotation.target,expectation=annotation and annotation.expectation,on_pick={kind='pick_pixel'}})
end
local function filmstrip(m,width,compact)
  local list=frames(m);local indices={};local current=1
  for i,f in ipairs(list) do if f.shot_path then indices[#indices+1]=i;if i<=m.selected then current=#indices end end end
  local visible=math.max(3,math.floor(width/128));local first=math.max(1,math.min(current-math.floor(visible/2),#indices-visible+1))
  local cards={key='film-cards',width='fit',gap=8}
  for n=first,math.min(#indices,first+visible-1) do local i=indices[n];local f=list[i]
    cards[#cards+1]=column{key='film-'..i,width=120,pad=4,gap=4,bg=i==m.selected and '$kitPanel' or '$kitSide',border={color=i==m.selected and '$kitAccent' or '$kitRule',w=1},radius=4,on_click={kind='select',index=i},role='button',label='Screenshot '..i..': '..action_name(f),
      component('testmedia/image',{path=f.shot_path,width=110,height=compact and 44 or 58,on_pick={kind='select',index=i}}),
      label((outcome(f) and outcome(f)..' ' or '')..i..' · '..action_name(f),{size=11,color=i==m.selected and '$kitAccent' or '$kitFg'}),label(string.format('%.1f ms · %s',f.time or 0,f.phase or ''),{size=10})}
  end
  return column{key='filmstrip',width='grow',gap=6,
    row{width='grow',gap=6,title('Screenshots',12),label(#indices..' captured'),column{width='grow'},button('←','previous'),button('→','next_frame'),button('Latest','latest')},
    row{width='grow',scroll_x=true,row(cards)}}
end
local function errors(m)
  local body={width='grow',gap=12}
  for i,d in ipairs(diagnostics(m)) do
    body[#body+1]=column{key='diagnostic-'..i,width='grow',pad=12,gap=6,bg='$kitPanel',radius=5,
      row{width='grow',gap=10,kit.badge{label=d.severity or 'error',tone=d.severity=='warning' and 'warn' or 'danger'},label(d.case or 'Runner',{color='$kitFg'})},
      label(message(d.message),{size=14,color='$kitFg',wrap='word',max_lines=4}),
      d.source and path_controls(d.source.file,d.source.line) or nil,
      m.stacks and kit.code{key='stack-'..i,text=tostring(d.message or ''),max_lines=160} or nil}
  end
  return column{width='grow',gap=10,row{width='grow',title('Errors & warnings',14),column{width='grow'},button(m.stacks and 'Hide stack traces' or 'Show stack traces','stack')},#body==0 and kit.empty{title='No errors',detail='Failures, worker errors, warnings and logged errors appear here.'} or column(body)}
end
-- Large state payloads retain all their text while only visible rows become nodes.
local function payload(m,env,key,value,height)
  m.payloads=m.payloads or {}
  local cached=m.payloads[key]
  if not cached or cached.value~=value then
    local source=pretty(value);local lines={}
    for line in (source..'\n'):gmatch('(.-)\n') do lines[#lines+1]=line end
    cached={value=value,source=source,lines=lines};m.payloads[key]=cached
  end
  if #cached.lines<=60 then return kit.code{key=key,text=cached.source,max_lines=60,copy=true} end
  return column{key=key,width='grow',gap=6,pad=8,bg='$kitSunken',radius=4,
    row{width='grow',label(#cached.lines..' lines'),column{width='grow'},button('Copy all','copy_payload',{key=key})},
    uniform_list(env,{key='payload-'..key,width='grow',height=height,rows=#cached.lines,row_h=18,overscan=3,selectable=true,scroll_x=true,on_selection_range={kind='copy_payload_range',key=key}},function(i)
      return row{width='fit',height=18,text(cached.lines[i+1]~='' and cached.lines[i+1] or ' ',{family='$kitData',color='$kitFg',size=12,line_height=18,wrap='none'})}
    end)}
end
selected_payload=function(cached,range)
  if not cached or not range or not range.from or not range.to then return nil end
  local first,last=range.from,range.to
  if (first.index or 0)>(last.index or 0) or ((first.index or 0)==(last.index or 0) and first.byte>last.byte) then first,last=last,first end
  local out={};local a=math.max(1,(first.index or 0)+1);local b=math.min(#cached.lines,(last.index or #cached.lines-1)+1)
  for i=a,b do local line=cached.lines[i];local start=i==a and math.min(#line,math.max(0,first.byte or 0)) or 0;local finish=i==b and math.min(#line,math.max(0,last.byte or #line)) or #line;out[#out+1]=line:sub(start+1,finish) end
  return table.concat(out,'\n')
end
local function details(m,env,height)
  local detail=m.detail or {tree={},state={},core={}};local content
  if m.tab=='output' then content=column{width='grow',gap=8,title('Runner output',14),path_controls(m.output and m.output.path),m.output and m.output.truncated and label('Showing the last 128 KiB. Use kui test output for the complete log.',{color='$kitWarn'}) or nil,kit.code{key='output-code',text=m.output and m.output.text~='' and m.output.text or 'No output from this test file.',max_lines=1000}}
  elseif m.tab=='errors' then content=errors(m)
  elseif m.tab=='tree' then
    local tree={width='50%',gap=2};for i,n in ipairs(detail.tree or {}) do tree[#tree+1]=row{key='node-'..i,width='grow',pad=5,bg=m.node==i and '$kitPanel' or nil,on_click={kind='node',index=i},label(string.rep('  ',n.depth or 0)..(n.role or n.kind or 'node')..' '..(n.label or n.text or n.key or ''),{color='$kitFg'})} end
    content=row{width='grow',gap=12,column(tree),column{width='grow',kit.code{key='node-code',text=pretty((detail.tree or {})[m.node] or {}),max_lines=160}}}
  elseif m.tab=='effects' then
    local events={};for _,f in ipairs(m.data.files or {}) do if not active_file(m) or f.name==active_file(m) then for _,e in ipairs(f.events or {}) do if e.kind~='frame' and e.kind~='step' and (not m.case or not e.case or e.case==m.case) then events[#events+1]=e end end end end
    local titles,times={},{ };for i,e in ipairs(events) do titles[i]=e.kind..' '..((e.data or {}).service or (e.data or {}).action or '');times[i]=string.format('%.1f ms · step %s',e.time or 0,tostring(e.step or 0)) end
    content=row{width='grow',height='grow',gap=12,column{width='35%',height='grow',kit.list{key='events',label='Messages and I/O',rows=channel{title=titles,detail=times},title='title',detail='detail',selected=(m.event or 1)-1,grow=true,on_select={kind='event'}}},column{width='grow',height='grow',scroll_y=true,kit.code{key='event-code',text=pretty(events[m.event or 1] or {}),max_lines=400}}}
  elseif m.tab=='assertions' then
    local buttons={width='grow',gap=6}
    for _,f in ipairs(m.data.files or {}) do local index=0;for _,e in ipairs(f.events or {}) do if e.kind=='snapshot' then index=index+1;if (not m.case or e.case==m.case) and (not m.case_file or (f.collection or {}).file==m.case_file) then buttons[#buttons+1]=button(((e.data or {}).ok and 'PASS ' or 'FAIL ')..((e.data or {}).name or 'Snapshot'),'snapshot',{file=f.name,index=index}) end end end end
    local compare
    if m.snapshot then local columns={width='grow',gap=10};for _,v in ipairs({'expected','actual','diff'}) do local snap=m.snapshot;columns[#columns+1]=column{width='grow',gap=6,title(v,12),path_controls(snap[v..'_path']),snap.kind=='expect_shot' and snap[v..'_path'] and component('testmedia/image',{path=snap[v..'_path'],width=220,height=140}) or kit.code{key='snap-'..v,text=snap[v..'_text'] or 'No '..v..' recorded',max_lines=120}} end;compare=row(columns) end
    content=column{width='grow',gap=10,row{width='grow',title('Snapshot comparisons',14),column{width='grow'},button('Update snapshots','update',nil,busy(m) or not m.data.can_run)},column(buttons),compare}
  else
    local field=m.tab=='core' and 'core' or m.tab=='kit' and 'kit_view' or 'state_view';local key=m.tab=='core' and 'core' or m.tab=='kit' and 'kit' or 'state'
    content=column{width='grow',gap=12,title('Changes in this action',13),payload(m,env,'changes-'..field,(m.diffs or {})[key] or {},math.min(180,height)),row{width='grow',gap=12,column{width='50%',gap=6,title('Before',12),payload(m,env,'before-'..field,m.before and m.before[field] or {},height)},column{width='grow',gap=6,title('After',12),payload(m,env,'after-'..field,detail[field] or {},height)}}}
  end
  local n=#diagnostics(m)
  return kit.tabs({key='details',value=m.tab,tabs={{value='state',label='App state'},{value='core',label='Core'},{value='kit',label='Kit'},{value='tree',label='Nodes'},{value='effects',label='Messages & I/O'},{value='assertions',label='Snapshots'},{value='output',label='Output'},{value='errors',label='Errors'..(n>0 and ' ('..n..')' or '')}},on_change={kind='tab'}},{column{key='detail-content',width='grow',height='grow',scroll_y=true,pad=12,content}})
end
function I.view(m,env)
  if m.clipboard then env.set_clipboard(m.clipboard);m.clipboard=nil end
  local w,h=env.viewport_w or 1440,env.viewport_h or 980;local wide=w>=1240
  local tests_fraction=m.layout.tests or (wide and 260 or 230)/w
  local remaining=w-w*tests_fraction-6
  local action_fraction=m.layout.actions or 220/remaining
  local main_width=math.max(160,(wide and remaining*(1-action_fraction)-6 or remaining)-32)
  local frame=m.detail;local before=m.before;local recorded=selected(m);local annotation=not m.hide_markers and recorded or nil;local active=running_file(m);local paused,interact=false,false;local pause_reason;local failure_pause=false
  for _,f in ipairs(m.data.files or {}) do if f.name==active then local status=f.status or {};paused=status.paused;interact=status.interact;pause_reason=status.reason;failure_pause=status.failed end end
  local pass,fail,total=counts(m);local run_status=busy(m) and (paused and 'Paused' or 'Running') or ((m.data.run or {}).status or 'Ready');if run_status=='complete' then run_status=fail>0 and 'failed' or 'passed' end
  local no_run=busy(m) or not m.data.can_run
  local has_error=fail>0 or run_status=='error' or run_status=='failed'
  local diagnostic_count=#diagnostics(m)
  local node=frame and (frame.tree or {})[m.node]
  local location=recorded and recorded.source
  if not location then for _,f in ipairs(m.data.files or {}) do for _,c in ipairs((f.collection or {}).cases or {}) do if c.name==m.case and (not m.case_file or m.case_file==(f.collection or {}).file) then location={file=c.file or (f.collection or {}).file,line=c.line} end end end end
  local source=m.case or (selected(m) and selected(m).case) or 'All tests'
  local failure
  for _,f in ipairs(m.data.files or {}) do for _,c in ipairs((f.results or {}).cases or {}) do if c.ok==false and (not m.case or c.name==m.case) and (not m.case_file or (f.collection or {}).file==m.case_file) then failure=c end end end
  local available=math.max(240,h-270-(not wide and 32 or 0)-(location and 32 or 0)-(recorded and recorded.expectation and 30 or 0)-(failure and 44 or 0)-(paused and 60 or 0))
  local stage_h=math.max(40,available*m.layout.preview-(h<820 and 110 or 128)-22)
  local previews
  if m.mode=='compare' then previews=row{width='grow',gap=10,column{width='50%',gap=5,label(before and 'BEFORE' or 'INITIAL STATE'),image(before or frame,(main_width-16)/2,stage_h,nil,node and node.rect,annotation)},column{width='grow',gap=5,label('AFTER'),image(frame,(main_width-16)/2,stage_h,m.override,node and node.rect,annotation)}}
  else previews=image(m.mode=='before' and (before or frame) or frame,main_width,stage_h,m.mode~='before' and m.override or nil,node and node.rect,annotation) end
  local left=wide and tests(m) or column{key='sidebar',width='grow',height='grow',bg='$kitSide',kit.segmented{key='sidebar-mode',value=m.sidebar,options={{value='tests',label='Tests'},{value='steps',label='Actions'}},on_change={kind='sidebar'}},m.sidebar=='tests' and tests(m) or steps(m)}
  local main=column{key='main',width='grow',height='grow',pad=12,gap=10,
    row{width='grow',gap=10,cross_align='center',column{width='grow',gap=4,title(action_name(recorded or frame),17),label(source)},
      busy(m) and row{gap=5,not paused and button('Pause','control',{command='pause'}) or nil,button('Step','control',{command='next'},not paused),button('Frame','control',{command='frame'},not paused),button(interact and 'End interaction' or 'Interact','control',{command='interact',enabled=not interact},not paused),button('Stop','control',{command='cancel'})} or nil},
    paused and row{key='paused-prompt',width='grow',pad=10,gap=12,bg='$kitPanel',column{width='grow',gap=4,title(failure_pause and 'Test failed — paused for inspection' or 'Paused — ready when you are',14),label(pause_reason or 'Resume to run the test, or Step to execute one action.',{wrap='word',max_lines=2})},button(failure_pause and 'Finish test' or 'Resume test','control',{command='resume'},false,'primary')} or nil,
    location and path_controls(location.file,location.line) or nil,
    failure and row{width='grow',pad=10,gap=10,bg='$kitPanel',label(message(failure.error),{color='$kitDanger',size=13,wrap='word',max_lines=2}),column{width='grow'},button('View error','tab',{change={value='errors'}})} or nil,
    row{key='preview-controls',width='grow',scroll_x=true,gap=8,kit.segmented{key='preview-mode',value=m.mode,options={{value='before',label='Before'},{value='after',label='After'},{value='compare',label='Compare'}},on_change={kind='mode'}},label(frame and string.format('%.1f ms%s',frame.time or 0,m.mode=='before' and not before and ' · initial state' or frame.reconstructed and ' · drawing playback' or '') or ''),column{width=12},wide and row{gap=8,button('Replay drawing','drawing',nil,not frame),button('Render state','render_state',nil,not (frame and frame.state and frame.state.adapter)),button('Perf','perf',nil,not (frame and frame.perf)),button(m.hide_markers and 'Show markers' or 'Hide markers','markers'),recorded and recorded.expectation and button('View comparison','snapshot',{file=recorded.file,index=recorded.snapshot_index}) or nil} or nil},
    not wide and row{width='grow',scroll_x=true,gap=8,button('Replay drawing','drawing',nil,not frame),button('Render state','render_state',nil,not (frame and frame.state and frame.state.adapter)),button('Perf','perf',nil,not (frame and frame.perf)),button(m.hide_markers and 'Show markers' or 'Hide markers','markers'),recorded and recorded.expectation and button('View comparison','snapshot',{file=recorded.file,index=recorded.snapshot_index}) or nil} or nil,
    recorded and recorded.expectation and row{width='grow',gap=8,kit.badge{label=outcome(recorded),tone=outcome(recorded)=='FAIL' and 'danger' or 'ok'},label('Visual expectation · '..(recorded.expectation.name or ''))} or nil,
    frame and path_controls(m.override or (m.mode=='before' and before and before.shot_path) or frame.shot_path) or nil,
    kit.split({key='preview-detail-split',axis='column',fraction=m.layout.preview,min=110,on_resize={kind='resize_preview'}},{
      first={column{key='preview-pane',width='grow',height='grow',gap=10,scroll_y=true,column{key='stage',width='grow',bg='$kitPanel',radius=5,pad=4,previews},filmstrip(m,main_width,h<820)}},
      second={column{key='detail-pane',width='grow',height='grow',details(m,env,math.max(110,available*(1-m.layout.preview)-130))}}})}
  local right=wide and kit.split({key='actions-main-split',fraction=action_fraction,min=180,on_resize={kind='resize_actions'}},{first={steps(m)},second={main}}) or main
  return column{key='inspector',width='grow',height='grow',bg='$kitBg',on_key={kind='keys'},key_focus=env.focus==nil or nil,
    kit.settings{skin='phosphor',density='compact',hair=1.5,font_ui={'Berkeley Mono Variable','sans'},font_data={'Berkeley Mono Variable','mono'},roles={accent=0xffd400ff}},
    row{key='header',width='grow',pad={x=16,y=12},gap=14,cross_align='center',title('kui test',20),kit.badge{label=run_status,tone=has_error and 'danger' or busy(m) and 'accent' or 'ok',dot=true},label(pass..' passed',{color='$kitOk'}),label(fail..' failed',{color=fail>0 and '$kitDanger' or '$kitMuted'}),label(total..' tests'),diagnostic_count>0 and kit.badge{label=diagnostic_count..' diagnostics',tone='danger'} or nil,column{width='grow'},button('Run all','run_all',nil,no_run,'primary'),button('Run selected','run',nil,no_run or not m.case),button('Headed','headed',nil,no_run),button('Debug','debug',nil,no_run)},
    m.error and row{width='grow',pad=10,gap=12,bg='$kitPanel',label(m.error,{color='$kitDanger',wrap='word',max_lines=2}),column{width='grow'},button('Dismiss','dismiss')} or nil,
    kit.split({key='tests-workspace-split',fraction=tests_fraction,min=180,on_resize={kind='resize_tests'}},{first={left},second={right}}),
    kit.statusbar({key='status'},{label((m.data.run or {}).root or 'Test artifacts'),trailing={button('Copy path','copy_path',{path=m.data.artifact_path}),button('Reveal','path_action',{path=m.data.artifact_path,action='reveal'}),label(m.notice or 'Drag dividers to resize · ← → frames'),button('Reset panes','reset_layout')}})}
end
return I
