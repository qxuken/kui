local kit = require("kit.ui")
local record presentation
  record Size
    width: number
    height: number
  end
  record Canvas
    w: number
    h: number
  end
  record Context
    state: any
    step: integer
    theme: kit.ContentTheme
    layout: kit.ContentLayout
    mode: string
    now: number
    size: Size
    msg: function(any): any
    dispatch: function(any)
    reveal: function(step: integer, children: {kui.Node}, options?: {string:any}): kui.Node
    content: function(props?: kit.ContentProps, children?: {kui.Node}): kui.Node
    every: function(number, any): any
    after: function(number, any): any
    spawn: function(task: function(signal: {string:any}): any): {string:any}
  end
  record Slide
    key: string
    title: string
    notes: string
    state_version: any
    steps: integer
    init: function(): any
    update: function(any, any): any
    activate: function(Context): any
    view: function(Context): kui.Node
    snapshot: function(Context): kui.Node
  end
  record Deck
    title: string
    slides: {Slide}
    preset: kit.ContentTheme
    theme: kit.ContentTheme
    layout: kit.ContentLayout
    settings: kit.SettingsProps
    fonts: {string}
    canvas: Canvas | Size
    presenter_shortcut: {string:any}
  end
  record Options
    presenter: boolean
    toolbar_in_titlebar: boolean
    size: Size
    snapshot: boolean
    snapshot_time: number
    clock: function(): number
    dispatch: function(any)
  end
  record Session
    model: {string:any}
    counts: {string:integer}
    position: function(Session, source?: string): {string:any}
    present_slide: function(Session)
    next: function(Session, source?: string)
    previous: function(Session, source?: string)
    jump: function(Session, string, source?: string)
    pace: function(Session)
    dispose: function(Session)
    update: function(Session, any): {string:any}
    view: function(Session, kui.Env): kui.Node
    render_slide: function(self: Session, index: integer, step: integer, mode?: string, size?: Size, reveal_guide?: boolean): kui.Node
    export_plan: function(self: Session, all_steps?: boolean): {{string:any}}
  end
  new: function(deck: Deck, options?: Options, previous?: Session): Session
  present: function(deck: Deck, options?: Options): Session
  metamethod __call: function(self: presentation, deck: Deck, options?: Options): Session
end
return presentation
