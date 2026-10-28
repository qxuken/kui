-- kui-perf's state: the shell's (the file, Live, preferences) and the
-- inspector's (inspector.tl), and the messages both hear.

local record state
  -- What the inspector keeps of one recording.
  record Inspector
    rec: kui.Recording
    session: {string:any}
    -- The window the timeline shows, ms on the recording's clock.
    v0: number
    v1: number
    -- The window rides the recording's end while Live brings frames.
    follow: boolean
    -- Rows of `rec.frames` and `rec.spans`; -1 for none.
    frame: integer
    span: integer
    -- The tables' tab, and the medium band's one pane.
    tab: string
    pane: string
    split: number
    -- The graph lanes shown, by series name, in order.
    lanes: {string}
    series: {string:kui.Channel}
    -- The filter (ADR 0075, decision 8).
    origins: {string:boolean}
    name: string
    min_us: number
    frames: string
    devtools: boolean
    -- The tables over the window, made again when it settles.
    stats: {string:any}
    stats_for: string
    stale: boolean
    restat_due: boolean
    tables: {string:kui.Channel}
    search_focus: integer
  end

  record Prefs
    recent: {string}
    split: number
    -- Graph lanes by recorded app's name.
    lanes: {string:{string}}
    name: string
    min_us: number
    frames: string
    devtools: boolean
  end

  record Model
    path: string
    error: string
    ins: Inspector
    live: boolean
    -- Frames that came while Live was off.
    waiting: integer
    prefs: Prefs
  end

  record Msg
    kind: string
    change: {string:any}
    select: {string:integer}
    zoom: {string:number}
    resize: {string:number}
    toggle: {string:any}
    key: {string:any}
    menu: {string:any}
    files: {string:any}
    drop: {string:any}
    drag: {string:number}
    name: string
    value: any
  end
end

return state
