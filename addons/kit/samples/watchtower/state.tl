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
    -- What the timer last published, a second apart.
    chart: kui.Channel
    log: kui.Channel
    latencies: kui.Channel
    errors: kui.Channel
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
