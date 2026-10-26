-- Watchtower's model and messages: what main.tl keeps and view.tl reads.

local record state
  record Model
    skin: string
    env: string
    range: string
    live: boolean
    query: string
    filter: string
    sort: string
    descending: boolean
    selected: string
    events: string
    split: number
    -- What the timer last published, a second apart: channels, so a
    -- component a tick did not change is kept by comparing a handle.
    chart: kui.Channel
    log: kui.Channel
    latencies: kui.Channel
    errors: kui.Channel
    -- Each service's last minute of requests a second, by name.
    trends: {string:kui.Channel}
    -- The services the table shows, as the filter and the sort leave
    -- them; published after every message, not every frame.
    services: kui.Channel
    updated: string
    -- The service a restart waits on the dialog for.
    confirm: string
    -- The service the scale popover is open on, and the count it shows.
    scaling: string
    scale_to: integer
    toast: string
    toast_tone: string
    tick: integer
  end

  record Msg
    kind: string
    name: string
    change: {string:any}
    toggle: {string:any}
    select: {string:integer}
    sort: {string:string}
    resize: {string:number}
  end
end

return state
