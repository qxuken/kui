-- kui-perf's state: the shell's (the file, Live, preferences) and the
-- inspector's (inspector.tl), and the messages both hear.

local record state
  record Sort
    field: string
    desc: boolean
  end

  record Shown
    key: string
    ch: kui.Channel
    order: {integer}
    inv: {integer:integer}
  end

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
    -- The range selected on the waterfall's axis, ms; nil for none.
    r0: number
    r1: number
    -- The tables' tab, and the medium band's one pane; the detail's share
    -- of the dock, and the timeline's of the height over it.
    tab: string
    pane: string
    split: number
    dock: number
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
    -- Each table's order by its header, and its rows taken in it.
    sorts: {string:Sort}
    shown: {string:Shown}
    search_focus: integer
  end

  record Prefs
    recent: {string}
    split: number
    dock: number
    -- The keys' card was shown once.
    helped: boolean
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
    -- The keys' card is open.
    help: boolean
  end

  record Msg
    kind: string
    change: {string:any}
    select: {string:integer}
    zoom: {string:number}
    resize: {string:number}
    range: {string:number}
    sort: {string:any}
    scroll: {string:any}
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
