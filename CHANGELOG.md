# Changelog

Every release lists what it adds and, separately, **what you can delete**:
the workaround, the model field or the arithmetic the release made
unnecessary. The second list is the point of the first — a library whose
upgrades remove code from the apps on it is doing the job. A "what you can
delete" line names the *behaviour* the release removed the need for, not the
workaround it guesses an app wrote — a workaround that accreted two purposes
only sheds the one the release addressed, and only the app's own tests know
which of its lines that was.

From 0.1.0-alpha.9 on, a release section opens with **What breaks.** as a
bullet list — one line per break, naming the symbol — and then the paragraphs
that argue each one. The list is for the reader with a build to fix, who
needs to grep for a name before reading 30 KB of prose; the paragraphs are
for the reader deciding whether to upgrade. Earlier sections keep the shape
they shipped with and are not retrofitted (backlog F31, from the alpha.8
field reports). A fix that changes what an existing input draws is a break
too — for whoever wrote to the old behaviour, documented or not — and is
listed under both (backlog F61, from the alpha.12 field reports: the list
is what the release knows it broke, and a fix it did not think of as one
was the first bare bump to break an app in five releases).

## 0.1.0-alpha.70 (unreleased)

**What breaks.**

- A float anchored to its parent that does not `escape` opens just above
  the floats its layer holds already, where it opened on top of every
  float (backlog F257): one that opens inside a page while a toast, a HUD
  or a dialog is up now paints under it. A menu or a popover that relied
  on opening over those says `escape`.
- A `kui-native` window leaves the screen when its close is accepted —
  every window, before `App::teardown` runs — where it stayed up through
  the teardown (backlog F246). A teardown that draws into a window, or
  shows a dialog over it, finds it hidden.
- `MenuAction` has an eighth variant, `OpenRecording { path }` (backlog
  F253), the same way: a Rust host that matches it whole adds an arm, and
  a C host that does not know `KUI_MENU_ACTION_OPEN_RECORDING` drops it.
- `MenuAction` has a seventh variant, `SetClipboardFiles { paths }`: a
  Rust host that matches the enum whole adds an arm (backlog F247). A C
  host that drains `kui_take_menu_action` sees the new kind,
  `KUI_MENU_ACTION_SET_CLIPBOARD_FILES`, and one that does not know it
  drops it.
- The C ABI is 41 (backlog C149): `KuiClip` takes `stretch[3]` after
  `transform`, 23 words where it was 20; `KuiKeyframe` and `KuiEnter`
  append `scale_x` and `scale_y` (60 and 56 bytes), and `KuiSpec` appends
  them too and grows with its two `KuiEnter`s (912 bytes on 64-bit).
  Recompile; a renderer that strides `KuiDrawData.clips` by its own
  `sizeof` reads the new stride, and one that ignores the new words draws
  a stretched box unstretched. Node's `clips()` is 92 bytes an entry, and
  `decodeClips` reads `stretch` before `inner`.
- `kui_core::Transform` has three more fields, `sx`, `sy` and `shear`,
  and `TransformSpec` one, `stretch`: a struct literal of either names
  them or takes `..Transform::IDENTITY` / `..TransformSpec::NONE`.
- Kit's line chart draws a new y range at once, its lines gliding to it
  in a box and its gridlines and labels sliding there (backlog C149),
  where it drew every frame of the glide at a range part of the way: a
  test that read a label in the middle of a glide reads the new range's.

- A lazy branch chosen by a query (`lazy { query = … }`) takes the room
  its parent offers in the axes the query reads, where it was a column
  its content's size (backlog F255): a branch that should stay that size
  says so inside it.

- `InputEvent` has a variant `Pinch { delta, begins }` and
  `kui_core::event::Scroll` a field `zoom: Option<f32>` (backlog F256): a
  Rust host that matches `InputEvent` whole adds an arm, and one that
  builds a `Scroll` by its fields adds `zoom: None`.

Nothing else breaks: two crates split out of `kui-core` and re-exported
where they were, four channel doors added in every binding, and the
recording's reader, its made-up sessions and its Lua module added beside
the format.

The first step of [ADR 0075](docs/adr/0075-kui-perf-reads-a-recording-in-kit.md),
`kui-perf`, the trace inspector: the recording's reader needs to hand a
script its columns as channels, and could not reach the channel from
under `kui-core`. Splitting it out brought two of the channel's open
entries with it — the inspector's Live view appends to a million spans,
and its tables sort and filter them.

### Added

- **A stretch: `scaleX` / `scaleY`** (backlog C149; the 2026-10-28
  amendment of [ADR 0043](docs/adr/0043-a-node-turns-about-its-pivot.md)).
  A node and its subtree stretched across and down by their own factors,
  along the node's axes before its turn and about its pivot, multiplying
  `scale` — paint-only as `scale` is: layout, the room taken and
  `onLayout` are the unstretched node's, while what it draws, clips and
  hits stretches and the access rect is the bounding box. It tweens with
  `transition`, and an entrance, an exit or a keyframe stop may name it
  (`enter: { scaleY: 2, dy: 40 }`). The clip entry carries a stretch —
  `sx`, `sy` and a `shear`, which only a turn nested inside a stretch
  composes to — so nesting stays exact. What is inside is drawn through
  it: a stroke widens across, a glyph is drawn wide, resampled as a
  turned one is, so keep text outside a box you stretch or settle it
  back to 1. Rust `NodeSpec::scale_x` / `scale_y` and `Enter` /
  `Keyframe` `scale_x` / `scale_y`; JSX `scaleX` / `scaleY` (and in
  `enter`, `exit`, `keyframes`); Lua `scale_x` / `scale_y`; C
  `KuiSpec.scale_x` / `scale_y` (0 is 1), `KuiEnter` / `KuiKeyframe`
  with `KUI_ENTER_SCALE_X` / `_Y` and `KUI_KF_SCALE_X` / `_Y`; Odin the
  generated `Spec`, `Enter` and `Keyframe` fields with `.Scale_X` /
  `.Scale_Y`. The devtools' facts and `Core::nodes` show it beside
  `rotate` and `scale`.

- **A trackpad's pinch** (backlog F256): a macOS trackpad's pinch reaches
  the innermost `onScroll` node under the pointer as its message with
  `scroll: {x, y, dx: 0, dy: 0, lines: null, zoom}` in it — `zoom` the
  magnification since the last event, positive zooming in — and stays
  that node's until the fingers lift, wherever the pointer goes. Its
  `scrollAxes` are not asked; a node that names `scrollMods` is passed
  by; no scroll container moves for it, and one begun over no `onScroll`
  node is nobody's. A canvas or a timeline that zooms on the wheel zooms
  on a pinch by reading `zoom`. Headless, `InputEvent::Pinch`,
  `kui_input_pinch`, Odin `input_pinch`, Node `ctx.pinch(delta,
  begins)`. Windows and Linux report no pinch through winit.
- **kui-perf, the trace inspector** (ADR 0075, backlog F200, F254):
  `kui perf session.kuitrace` opens a recording — from the devtools' Perf
  tab, `KUI_PERF_TRACE` or `kui-trace synth` — in a Teal app the runner
  carries (`tools/kui-perf`), built from Kit over `kui.trace`: the frames
  as an overview, the spans in Kit's waterfall with counters and gauges as
  graph lanes on the same window, the frame or span selected with its
  verdict, phases, input path and the components drawn fresh, tables over
  the window, filters, three bands by the inspector's own width, the keys
  (← → / h l, [ ] slow frames, { } burns, w a s d, f, 0, /), Phosphor in
  yellow, and the file watched: Live brings what it gains, or a refresh
  button counts it. A 72,000-frame recording is on screen 0.82 s after
  the process starts.

  *What you can delete:* a trace converted for Perfetto or speedscope
  only to look at it — they draw no verdicts, burns or causes.

- **Open in kui-perf** (backlog F253): the devtools' Perf tab saves the
  recording and asks the runner to open it, as
  `MenuAction::OpenRecording { path }`; `kui_native::open_recording` starts
  `kui perf` with the runner's kui or the one on `PATH`, the file shown in
  its folder when neither starts. C `kui_open_recording` and
  `KUI_MENU_ACTION_OPEN_RECORDING`, Odin `open_recording`, Node
  `openRecording` and its `{ kind: 'openRecording', path }` action.

- **Kit's line chart takes a host's x window**: `x_min` / `x_max`, for a
  chart that shares its axis with another view — a timeline's lanes — and
  a held `gutter`, its y labels' column, so its plot starts where the view
  over it does.

- **Kit's waterfall selects a range and zooms by any wheel** (backlog
  F258): a drag along its time axis selects a range, drawn over the lanes
  and the axis with its length beside the title, zoomed to by its button
  or cleared by its × or a click on the axis — `on_range`, and
  `range_start` / `range_length` for a host that keeps it; the wheel zooms
  about the pointer however deep the trace is (its lanes scroll in a box
  around them, where they scrolled under the wheel once they overflowed),
  a sideways swipe or ⇧ and the wheel pans, a pinch zooms (`scroll.zoom`);
  `gutter` keeps an empty column left of the axis and the lanes, to line
  them up with charts under them.

- **kui-perf: the keys' card, the dock, the range** (backlog F258): `?`,
  Help ▸ Keys and gestures or the toolbar's ⓘ shows every key and
  gesture on a card, open by itself the first time a recording opens; the
  dock under the timeline is a split whose rule is dragged (neither under
  140 px), kept with the preferences; the range selected on the axis is
  zoomed to by `z` (or `f`) and cleared by Esc; the graph lanes' plots
  start where the waterfall's lanes do. Its headless test keeps its
  preferences in a folder of its own, where it removed the app's.

- **`kui.trace`'s filter hides span names whole**: `hide_names = {
  "devtools" }`.

- **Files on the clipboard** (backlog F247, from Carnet's "Copy file"):
  `Ui::set_clipboard_files(paths)` puts files there as a file manager's
  Copy does, so they paste into Explorer, Finder, a mail or a chat as the
  files. The window writes them as `CF_HDROP` on Windows (pasted as a
  copy), file URLs on the macOS pasteboard, and a `text/uri-list` offer on
  Linux, X11 and Wayland alike, each path made absolute first; a headless
  run keeps the paths as its clipboard's text, a line each. C
  `kui_set_clipboard_files` (a `KUI_MENU_ACTION_SET_CLIPBOARD_FILES` with
  the paths in its `text`, a line each), Odin `set_clipboard_files`, Node
  `setClipboardFiles(paths)` (`{kind: "setClipboardFiles", paths}` from
  `takeMenuActions()`), Lua `env.set_clipboard_files({...})`.

- **`kui-value` and `kui-channel`, crates of their own** (backlog F248):
  `Value`, `json` and the message traits, with `lua` (`kui_value::lua`,
  the conversions to and from an `mlua` value) and `derive`
  (`#[derive(Message)]`, `#[message(crate = "kui_value")]` for a crate
  with no core) features; and the data channel, with a `lua` feature for
  the userdata a script holds. Neither depends on another kui crate, so
  what has no core — the recording's reader next — speaks values and
  makes channels. `kui-core` re-exports both where they were
  (`kui_core::Value`, `kui_core::json`, `kui_core::message`,
  `kui_core::channel`), and kui-lua's `lua_to_value`, `value_to_lua` and
  `mark_array` are the new crates' with a channel crossing as its handle.

- **A recording read as it arrives** (backlog F249): `kui_trace::Stream`
  decodes bytes pushed in as they come, keeping a record cut between
  pushes for the next, and grows its timeline in place
  (`Timeline::extend`, which sorts only the end the new records reach
  into); a `Transport` trait says what a source had — `More`, `Idle`,
  `Ended`, or `Reset` for one that began again — and `FileTransport`
  reads what a file grew by, or the whole again when it was rewritten;
  `Watch` checks a file on a thread of its own and calls back only when
  its size or time moved. `Trace::slice` is the frames-range cut the Perf
  tab's range reads, now the crate's.

- **`kui-trace synth`** (backlog F250): a made-up session by frames,
  spans a frame, seed and Hz, in the shape a recorder writes — gaps
  asleep and woken, input and handlers, the app's spans nested under
  `build`, an extension's fill and its placements, burns, slow frames,
  a parse on a track of its own, gauges, marks and notes — streamed to a
  `.kuitrace`, or `kui_trace::synth::Synth::trace()` in memory. 72,000
  frames, about 937,000 spans, are 27 MB written in 0.3 s.

- **`kui.trace`, a recording for a script** (backlog F251, ADR 0075):
  `kui-trace`'s `lua` feature, which the runner turns on. `kui.trace.open
  (path, { message })` reads a `.kuitrace` into channels — `rec.spans` in
  Kit's waterfall's shape, `rec.frames` a row a finished frame with its
  phases and verdict, `rec.instants` the waterfall's marks — and answers
  `rec:frame(row)`, `rec:span(row)`, `rec:stats(first, last)` and
  `rec:series(name)` as small tables and channels, all read in Rust.
  `rec:filter{ origins, name, min_us, verdicts, hide_unchanged }` shows
  what it keeps under the same handles. The file is watched: as it
  grows `update` is handed `message`, and `rec:poll()` appends what came
  (`"reset"` for a file written again). The 72,000-frame recording opens
  into channels in 0.57 s and filters by name in 53 ms. `try_open` and
  `try_read` as the runner's other calls have them; typed for Teal.
  `kui_channel::Channel::publish_from` is new beside it: the next
  revision of a view's identity holding another channel's rows, shared.

- **A channel's intervals on lanes** (ADR 0075): `Channel::lanes(start,
  len, lane)` sorts each lane's intervals once in Rust and makes its
  levels as they are first asked for; `Lanes::window(lane, v0, v1, min)`
  answers the runs reaching into a window — an interval at least `min`
  long its own, shorter ones a gap under it apart merged into a strip
  that counts them — so a window costs about a run a pixel, whatever the
  rows behind it. A million intervals zoomed fully out are about 1,200
  runs. In every binding: Lua `ch:lanes(...)` and `lanes:window(...)` (a
  list per field), Node `channel.lanes(...)` and `ChannelLanes.window`
  (typed arrays), C `kui_channel_lanes`, `_window`, `_count`, `_extent`,
  `_free` and `KUI_CHANNEL_RUN`, Odin's.

- **`where` in `kui.trace`'s filter**: `rec:filter { where = function(s)
  return s.len > 0.5 and s.name:find("fill") end }`, called from Rust
  with one table a span; 200 ms over 937,000 spans.

- **kui-perf's spike** (backlog F252, `tools/kui-perf/spike`): a bare Lua
  app — Kit's waterfall and table over `kui.trace` — that measures
  itself phase by phase over a long recording, and the script that reads
  its recording back. ADR 0075 holds the numbers and is accepted with
  them.

- **A channel appended to** (backlog C147): `Channel::append(rows, cap)`
  is the next revision with `rows` after this one's — each column named,
  of its kind, integers widening into a number column — the oldest past
  `cap` dropped. A revision's rows are shared with the next, not copied:
  a table is a few runs that merge as they grow, so a log appended to a
  line at a time costs about its new lines and a reader by row looks
  through a dozen runs at most; a reader of a whole column has it put
  together once a revision. In every binding: Lua `ch:append(columns,
  cap)`, Node `channel.append(columns, cap)`, C `kui_channel_append(ch,
  builder, cap)`, Odin `channel_append`.

  *What you can delete:* a log's cap kept low because every line
  republished the lot — the Kit gallery's 500.

- **A channel's filter, sort and take** (backlog C148): `filter(col,
  match, within)` — a value (`Match::Eq`), a string's part with or
  without its letters' case (`Match::Contains`), a number range
  (`Match::Range`) — `sort(col, descending, within)`, ties in order and
  NaN last, both answering row indices, and `take(rows)`, a new channel
  of those rows in order (`take_columns` for publishing them as the next
  revision of a view's own). In every binding: Lua `ch:filter(col, {
  eq | contains, case | min, max }, rows)` / `ch:sort` / `ch:take`, Node
  `channel.filter` / `sort` (a `Uint32Array`) / `take`, C
  `kui_channel_filter_f64` (a range, `min == max` for one value),
  `kui_channel_filter_str` (`KUI_CHANNEL_EQUAL`, `_CONTAINS`,
  `_CONTAINS_ANY_CASE`), `kui_channel_sort` and `kui_channel_take`, and
  Odin's.

  *What you can delete:* a worker's copy of its rows in Lua kept only to
  filter and sort them — the gallery's 21 MB.

### Changed

- **Kit's line chart glides its y range on the core's clock** (backlog
  C149). When the rounded range changed it asked for a frame every frame
  for 240 ms and ran the view in each. The lines are drawn for the new
  range at once, in a box keyed by the glide that enters stretched and
  moved to where the old range had them (`scale_y` and `dy`) and settles
  out of a cubic; the gridlines and their labels are keyed by value and
  slide to their places, fading in and out where the ranges differ. The
  frames of the glide are built from the kept tree (`Core::reframe`):
  two view runs for a change (the frame that sees it and the next, the
  first to keep what building again needs), where it was fifteen at 60
  Hz. Under reduced motion it jumps, as it did.

- **Kit's waterfall costs its width, not its spans** (ADR 0075): it reads
  its bars from the channel's lanes, a window at a time, and the names
  and groups of the bars it draws — where it made a Lua table of every
  row and nineteen levels of every lane on each new revision — and draws
  marks closer than a pixel as one rule whose tooltip counts them, where
  it drew each. Over 936,723 spans: the first frame 4.6 s → 0.1 s past
  the open, a frame showing the whole recording 50 → 0.4 ms, zooming out
  520 → 3.5 ms at its worst.

### Fixed

- **A float that opens inside a box no longer draws over a dialog**
  (backlog F257): Kit's line chart lays out more x tick labels once its
  plot is measured, a frame after a dialog over it opened, and those
  labels — floats held by the axis — went on top of the stack, over the
  dialog's panel, while the rest stayed under it; a live chart's slide,
  under a fresh key each sample, did the same. A float anchored to its
  parent that does not escape is its box's content (ADR 0074) and now
  opens with the layer that holds it (ADR 0023, amended); one that
  escapes, or is placed against the viewport or a node, opens on top as
  before.

- **A window no longer flashes white as it closes** (backlog F246, from
  Carnet). The main window stayed on screen from the close until the
  process ended, through the app's `teardown` and the surfaces being let
  go; on Windows its empty surface showed white for as long as that
  took — long enough to see in an app that saves its documents on the way
  out. Every window is hidden the moment the close is accepted (the close
  button, `WindowCommand::Close` of the main window, an OS Quit, a pumped
  runner's retirement), and a closing pane before it is destroyed.
  Measured on Windows 11 with a 1.5 s teardown: off the screen 7–16 ms
  after the close, where it stayed for the whole 1.6 s.

**What you can delete.**

- A range eased by hand in a view — a frame asked for every frame and
  everything drawn again at the range part of the way — kept only so a
  chart's lines did not jump: draw them for the new range in a box that
  enters at `{ scaleY, dy }` from where the old one had them.
- Platform clipboard code of an app's own (`CF_HDROP`, `NSPasteboard`
  file URLs, `text/uri-list`), kept only to copy a file.
- A hidden-window dance or a quick exit in an app's close path, kept
  only so the window did not flash as it went.

## 0.1.0-alpha.69 (2026-10-28)

**What breaks.**

- A runner call that fails raises its message at the line that called
  it — `main.lua:12: json: 1:3: expected a key in double quotes` — where
  it raised `runtime error: json: …` with a stack traceback after it.
  Code that read the old text — `gsub("^runtime error: ", "")`, the
  first line of `tostring(err)` — now sees the caller's position at the
  front; reach for the call's `try_` form instead, whose message is the
  error's alone.
- **A Kit `on*` prop is typed against the app's registered messages**
  (backlog F208): `onPress`, `onChange`, `onChoose` and every other Kit
  event prop take `HandlerMsg<{ press: ButtonPress }>` and its siblings,
  where they took `unknown`. An app that augments `KuiMsg` registers its
  Kit messages as they arrive — `ButtonPressMsg<{ kind: 'run' }>` — or
  the prop no longer typechecks; an app that does not augment it sees
  `Msg`, any plain data, as at every core prop.
- A `$name` in a Node view that neither the roles nor the surface's
  `setTokens` hold is resolved by the core rather than refused by the
  encoder (backlog F207): an extension's exported token now paints. One
  that nothing declares still keeps the slot's default and raises
  `unknown-token`, but the encoder's `unknownTokens` no longer lists it —
  the core raises it, by its name. The Node frame is v28; the encoder
  and the addon ship together, so only a hand-built stream sees it.
- A context menu, submenu or drawn menu bar's menu the core draws that
  is taller than the window is cut to it and scrolls, where it was
  pinned to the window's top with its last rows past the bottom; its
  panel is a scroll container (`scroll_y`, `overscroll: contain`) at
  every size, so the wheel over a menu that fits scrolls nothing behind
  it (backlog F210).
- In a `kui-native` window on Windows, X11, and macOS where the core
  draws its menus, a context menu that would be moved or cut in the
  window opens in a popup window of its own: the window's core keeps it
  as state and draws none of it, so its rows are not in that window's
  frame or access tree while it is up (`Core::menu_in_popup`). A menu
  opened there is drawn nowhere until the runner has placed it, within
  the same turn (`Core::menu_awaits_host`). Every other host draws every
  menu in the window as before (backlog F210, ADR 0004 amended
  2026-10-27).
- A declared popup window (`WindowKind::Popup`) appears at once on
  Windows, with its first frame, where the OS faded it in over a white
  surface.
- `Menu` gained `max_size` and `in_window` (backlog F210): a Rust struct
  literal names them or is built with `Menu::new`.
- `Span` gained `bg_pad` (backlog F227): a Rust struct literal names it or
  is built with `Span::new`.
- C's ABI is 40: `KuiSpan` gained `bg_pad` (64 bytes on 64-bit, from 56;
  spans travel as an array, so the stride moved). Recompile; zeroed, a
  span's background is flush with its glyphs, as it was (backlog F227).
  The same ABI appends `atlas_dirty_rect` to `KuiDrawData` (136 bytes on
  64-bit, from 120; backlog F228), an [out] struct written no further
  than the size you reserved.
- **Odin's openers are `@(require_results)`** (backlog F230): `box`,
  `column`, `row`, `radio_group`, `devtools_tab_open`, `fragment_open` and
  `fragment_open_with` called bare, as a statement, no longer compile.
  Such a call left its node open to the end of the enclosing block, so
  everything after it was its child: write `if kui.box(ui, spec) { }`, or
  `kui.rect(ui, spec)` for a box with no children.
- An editor's `change` arrives when an IME composition begins over a
  selection, carrying the text without it (backlog FZ17), where the
  first `change` came at the commit. An app that counts changes, or
  takes one as the end of a composition, sees one more.
- A view that returns with nodes still open raises `unclosed-node`, and a
  `layout_of` on a node that declares no `on_layout` raises
  `layout-of-without-handler`, where both said nothing (backlog F230,
  F231). Diagnostics only: what is drawn is unchanged.

The failure is the world's — a missing file, a refused connection, a
file that is not JSON — not a bug in the line that asked, so a traceback
through the runner's own frames said nothing an app could use. An error
the app does not catch still says where it happened.

F207, F208 and F209 below are the field reports `kui-node-template`
filed: two as it moved onto Kit in its alpha.68 bump, and one it had
open since alpha.48.

Kit's props were the one corner of a Node app's view a payload typo
slipped through: `<Button onPress={{ kind: 'typo' }}/>` compiled where
`<button onClick={{ kind: 'typo' }}>` failed. They are typed the way
the core's handler props are (ADR 0047): a registered message that
arrives with the event's field, declared without it. That is a type
break and only that — the wrapper writes what it wrote — and the fix
is a line per message in the app's union, the `…Msg<T>` helper Kit
already generated for each event.

### Added

- **The atlas uploads what changed** (backlog F228, from Replicator
  Bloom's wishlist): a new glyph re-uploaded the whole page, 4 MB at
  1024², in kui-wgpu and in every host drawing `kui_draw_data`.
  `GlyphAtlas::dirty_rect()` is the union of what was written since the
  last upload (the whole page when `epoch` moved) and `mark_uploaded()`
  clears it; kui-wgpu uploads that rect alone, and C has it as
  `KuiDrawData.atlas_dirty_rect`.
- **The host loop, written down** (backlog F229): kui.h's "The host
  loop" section gives a host without `kui_run` the order — input, drain,
  build, drain, draw — what only the second drain hears (`on_resize`,
  `on_hover` under a still pointer, `on_layout`, `on_query`, `on_range`,
  and the answers to what was asked between frames), when to build once
  more, and `kui_frame_unchanged` under `kui_set_frame_trace` as the sign
  to skip a draw.
- **`unclosed-node` and `layout-of-without-handler`** (backlog F230,
  F231): the first names the innermost node a view left open, whose
  missing close put every later node, the core's own menu and devtools
  panel included, inside it; the second says why `layout_of` answered
  nothing — the node declares no `on_layout`. In every binding, with
  diagnostics on. **`kui.rect`** in Odin: a box with no children, opened
  and closed in one call.
- **Beside another Rust library, link the shared one** (backlog F232):
  the C and Odin packs' READMEs, the Odin README and kui-ffi's README say
  so, and why — every Rust archive carries its own standard library, so
  two collide at `rust_eh_personality`.

- **A span where no core is at hand** (backlog F215, from kawoosh):
  `kui_core::perf::span("plugin")` — a guard, or `span_begin` /
  `span_end` — records a span on the recording of the core whose span
  is open on this thread (the build, a handler the runner times, the
  app's own), nested where it ran, under that span's origin and node: a
  profiler inside a script runtime, a terminal's read under the status
  line. Outside every core span, or with nothing recording, it is
  nothing, at the cost of one atomic load. C `kui_thread_span_begin` /
  `kui_thread_span_end` (no context), Odin `thread_span_begin` /
  `thread_span_end`, Node `threadSpanBegin` / `threadSpanEnd`.

  *What you can delete:* each plugin's time summed per frame and
  reported with `perf_measure` on a track of its own — the span sits
  under the fill that ran it.

- **`perf_note(text)`** (backlog F216, from kawoosh): a line on the
  timeline beside the numbers — "editor moved, reads fs.stat" — shown in
  its frame's detail in the Perf tab; 64 a frame and its gap, 512 bytes
  each. In every binding: C `kui_perf_note`, Odin `perf_note`, Node
  `perfNote`, Lua `env.perf_note`.

  *What you can delete:* counters named for a reason (`pane ran: WHY`)
  kept only to carry a sentence.

- **A recording says why each component was drawn fresh** (backlog
  F212, from kawoosh): every placement is a `placement` record — its
  kind, its node, its answer (`replayed`, `not-kept`, `params`, `reads`,
  `not-replayable`, `heard`) and, for `reads`, the fact that moved by
  its kind and node (`hover 00000000000000ab`, `the clock`), never what
  it read. The Perf tab lists a frame's components drawn fresh and why,
  and the recording's components by how often each replayed;
  `kui-trace dump` prints them, the adapters write each one drawn fresh
  as an instant, and `kui_trace::analysis::placements` groups them. A
  reader before this one skips the record.

- **One child process started at a time, whichever thread asks**
  (backlog F223): `kui_native::spawn` — `spawn`, `output`, `status` —
  std's own with the start under one process-wide lock, or under the
  host's spawner when it set one (`set_spawner`, F222, now in the same
  module and still at the crate's root). Everything kui starts goes
  through it: the runner's `kui.process`, from the app's thread and from
  every worker's, the tools `show_in_folder` and `open_external` run,
  the devtools' "open folder". An app's own spawns beside kui's take it
  too. On macOS a pipe is made and marked close-on-exec in two calls,
  and a child started between them keeps the other spawn's ends for its
  life.

  *What you can delete:* a lock of the app's own around its spawns,
  kept only so they did not race each other — `kui_native::spawn::spawn`
  is that lock.

- **A host's way of starting children, for kui's** (backlog F222,
  from kawoosh): `kui_native::set_spawner(spawn)` starts the tools
  `show_in_folder` and `open_external` run — and so the devtools' "open
  folder" — where they called `Command::spawn`. A host that serializes
  its spawns under one lock against macOS's pipes, which are not born
  close-on-exec, hands kui that lock. Rust's: a C, Odin or Node host's
  processes are its own.

  *What you can delete:* an opener and a "show in folder" of the app's
  own, kept only so their children started under its spawn lock.

- **A `try_` form for every runner call that can fail on its input or on
  the world** — a file, text read in, the network, a process, a
  database, a worker's script: `local ok, tasks =
  kui.json.try_decode(text)` returns `true` and what the call returns, or
  `false` and the error's message. The `try_` form is the call; the plain
  one is made from it and raises the message. Wrong input is a failure
  too (`kui.http.try_get("")` is `false, "http: no url"`); only an
  argument of the wrong type raises, as an assertion would. The forms
  wait as their calls do; a decoded `null` is `true, nil`; an HTTP 404 is
  a response, a refused connection `false`; `try_transaction` rolls back
  and answers `false` when its function raises. `try_` is the runner's
  convention from here on: the forms of `kui.json`, `kui.toml`, `kui.yaml` (`decode`, `encode`),
  `kui.fs` (`read`, `write`, `append`, `create`, `list`, `lines`, `walk`,
  `mkdir`, `remove`, `rename`), `kui.process` (`run`, `spawn`, and a
  process's `read_line`, `write`, `wait`), `kui.http` (each request and
  `serve`), `kui.sqlite` (`open`, and a database's `query`, `first`,
  `exec`, `transaction`) and `kui.worker` (and a worker's `dispatch`),
  declared for Teal (`if got is kui.Response`) and lua-language-server.
  The book's task list opens a broken `tasks.json` and says why, where it
  stopped; Kit's relay, preferences and field-notes samples and its
  gallery use the `try_` forms, and the preferences sample no longer
  stops on a settings file it cannot read.
- **A context menu that does not fit its window opens past its edge**
  (backlog F210, [ADR 0004 amended
  2026-10-27](docs/adr/0004-multi-window.md)). Where the runner can place
  a window — Windows, macOS, X11 — a menu the core draws that would be
  pushed in over its point, or cut and scrolled, opens as a borderless,
  non-activating popup at its point on the display, flipped left or up
  where the room runs out, held to the monitor's work area, and cut to it
  and scrolling when taller. A submenu opens as a further popup beside
  its row. The keyboard, Escape, a press outside, type-ahead and the
  accessibility tree are the in-window menu's, and a chosen row posts the
  same `menu` event. A menu that fits stays a float, and Wayland keeps
  every menu in the window. For a Rust host that opens its own windows:
  `Core::set_menu_popups`, `set_menu_in_popup`, `menu_awaits_host` on the
  window's core; `open_menu_popup`, `take_menu_popup_answer`
  (`MenuPopupAnswer`), `menu_popup_size`, `menu_popup_submenu`
  (`MenuPopupSubmenu`), `close_menu_popup_submenu` and
  `menu_popup_by_keyboard` on the popup's; `Session::host_window_id`;
  `widgets::MENU_PANEL_PAD` public. C, Odin and Node hosts get it through
  `kui_run` and `runWindowed`, whose runner is kui-native's.
- **A context menu held to a size, or kept in the window** (backlog
  F210). `Menu::max_size(w, h)` holds the menu and each of its submenus
  to `w` by `h` logical px — a row wider is ellipsized, a menu taller
  scrolls, in the window or in a popup — with `f32::INFINITY` leaving an
  axis to the room. `Menu::in_window()` keeps a menu drawn in the window
  where the runner would put one that does not fit in a popup. C:
  `kui_open_menu_with(ctx, key, x, y, items, count, max_w, max_h, flags)`
  with `KUI_MENU_IN_WINDOW` (a side of 0 is the room's); Odin:
  `open_menu_with`; Node: `openMenu(key, x, y, items, {maxWidth,
  maxHeight, inWindow})`; Lua: `open_menu(key, x, y, items, { max_width =,
  max_height =, in_window = })`. `widgets::context_menu_within` is the
  widget with a limit.
- **Air around a span's background** (backlog F227, from Carnet's
  selection): `bg_pad` widens the two ends of the rect a span's
  background is drawn in, so a selection or an inline `code` no longer
  sits flush on its first and last letters. Only the ends: spans of one
  background merged into one rect are padded at its outer ends, each line
  a wrapped span covers at its own, and nothing moves — not the glyphs,
  the line breaks or where a press lands. Where the rect meets another
  background on its line (a selection beside a highlight) the edge they
  share stays, so neither paints over the other, and two closer than their
  padding share the gap. `Span::bg_pad` in Rust, `bgPad` on a JSX
  `<span>`, `bg_pad` in a Lua span table, `KuiSpan.bg_pad` in C and
  `Span.bg_pad` in Odin; logical px.

### Fixed

- **Two checks in one process no longer trip over each other's
  declarations** (backlog F240): F224's part file was named for the
  process, so two threads of one — two `kui check`s a host ran at once —
  wrote the same part, and one failed with "No such file or directory".
  Each write has a part of its own.
- **Kit's tokens paint a Node view's own boxes** (backlog F207, from the
  kui-node-template field report): `<box bg="$kitPanel">` paints the
  skin's panel and follows a skin switch, where the encoder resolved a
  `$name` against the app's `setTokens` and the roles only, and raised
  `unknown-token` for every `$kit…` name. A name the encoder's map does
  not hold now crosses by name and the core resolves it in its own order
  — the running origin's table, the host's, then every extension's
  exported one (`Tokens::exported`, C74) — as Lua's and Rust's lookups
  always did; a name the map holds still rides as its index. A name
  nothing declares misses in the core, and its `unknown-token` line
  names it.
- **Kit's `on*` props take the app's messages** (backlog F208, from the
  kui-node-template field report): typed as `HandlerMsg<{ <event>:
  <Component><Event> }>` by `kui-addon-schema`, so a message that is not
  registered, or is registered for another event, fails at the prop.
- **`keyNamed`'s example compiles** (backlog F209, from the
  kui-node-template field report): its doc comment showed
  `ctx.click(ctx.keyNamed('Like'))`, and Node's `Ctx` has no `click`
  (TS2339). It reads `ctx.access(ctx.keyNamed('Like')!, 'click')`, or
  `app.access(…)` on a headless app, which settles what follows through
  `update`. No `ctx.click(key)` was added: `access(key, 'click')` is
  already the door that drives a node as a reader does, which is the
  doc's point, and `click` on an app is the pointer's, at coordinates.
- **A menu taller than the window can be used** (backlog F210, from
  Carnet's page menu in a short window). The core's context menu was a
  viewport float with `fit` and no height limit: taller than the window,
  `fit` pinned it to the top and its last rows were past the bottom,
  where neither the pointer nor the keyboard reached them; its submenus
  and the drawn bar's menus were the same. Each panel now scrolls, and
  layout cuts one that does not fit to the window less `MENU_EDGE`
  (8 px) above and below: a context menu or submenu is placed as high as
  that room goes — where Windows puts a menu taller than the screen, and
  where macOS's ends up once scrolled to its end — so it covers the point
  it opened at and the row it hangs off; a bar's menu keeps its top under
  its title and is cut at the room's bottom. A menu that fits is placed
  exactly where it was. Up, Down, Home, End and type-ahead keep the row
  they land on in view, as Tab does; the wheel and a trackpad scroll the
  panel under the pointer, and a submenu's scroll is its own.

- **The Perf tab stops measuring when the panel goes** (backlog F211,
  from kawoosh's alpha.68 port): turned off with `set_devtools(false)`,
  or hidden by its close button, while it showed its Perf tab, the panel
  left the recording's live view on, so `perf_measuring()` stayed true
  for the rest of the session — every span, gauge and count an app gates
  on it recorded with nobody looking, and the frame trace held on. A
  panel off returned from its frame's start before the step that lets
  the live view go; it lets it go first now. The hidden panel was right
  already, and is tested beside it.

- **A capture reads its frame back while another thread captures**
  (backlog F225, from PR #7's CI): the capture device is the process's,
  and a readback's wait polled it once — when another thread's poll had
  collected its callback and not run it yet, the wait returned with the
  frame unread, "the GPU could not read the frame back". It polls until
  its own callback has run now, ten seconds at the most.
- **Two `kui check`s at once agree** (backlog F224, from PR #7's CI):
  every check writes kui's declarations into one folder a version before
  it reads them, truncating each file first, so a check that read
  `kui.d.tl` while another wrote it reported every `kui.` name unknown.
  Each file is written beside itself and renamed over now, and not at all
  when it holds the text already.
- **A `kui.process.run` in a worker no longer hangs on a child another
  thread started** (backlog F223): on macOS a `kui.process.spawn` from
  the app's thread could take the write end of a worker's `run`'s pipe,
  and the `run` waited for as long as that child lived. Every start is
  under one lock now.
- **`show_in_folder` selects on Linux, and `open_external` shows a
  folder on macOS** (backlog F222, from kawoosh): a Linux desktop's file
  manager is asked over D-Bus (`FileManager1.ShowItems`) to select the
  file, `xdg-open` on its folder where none answers; and a folder opened
  on macOS is opened in Finder, so a bundle (`X.app`) is shown as the
  folder it is where `open` ran it.
- **`open_external` and `show_in_folder` take a path with `/` in it on
  Windows** (backlog F226, from Carnet). `explorer` reads
  `C:\Users\me\Notes/attachments/report.pdf` — an app's folder joined to
  a Markdown link's path — as no path at all, and opened Documents. A
  target that is not a URL has every `/` made `\` before the command is
  built; a URL is left as it is.
- **A capture no longer panics while another thread captures, on macOS**
  (backlog F241, from smoke.yml's macOS run): F225's wait polled the
  device until its readback had run, and a poll that waits can find the
  queue emptied by another thread's poll before its own fence read caught
  up, which wgpu-core answers with a panic ("the current submission index
  (1) should be at least the wait submission index (4)"). The wait polls
  without waiting now, and sleeps between.
- **A design-token file with a stray character in a hex colour is refused,
  not a panic** (backlog FZ16, found by the `dtcg` fuzz target):
  `"#1̠413"` sliced a character in two in `kui_core::dtcg`. A hex colour
  is hex digits alone now, and `#+fff`, which parsed, is refused with it.
- **`kui check` on Windows rides out a declaration file held open**
  (backlog F242): the rename F224 writes each file with is refused while
  something holds the file without sharing its deletion, and the check
  failed with "Access is denied"; it is tried again for a moment.
- **`kill` on Windows says nothing of its own** (backlog F243): taskkill's
  "SUCCESS: The process with PID …" lines reached the app's output.
- **A composition begun over a selection and then abandoned no longer
  loses the selection behind the app's back** (backlog FZ17, found by
  the `events` fuzz target; in every release since compositions replaced
  a selection). Composing deletes the selection, a recorded edit, but the
  app heard nothing until the commit — and a blur or an empty preedit
  abandons a composition without one, so the field showed the text
  without the selection while the app's model still held it, until the
  next keystroke's `change` made the loss the app's too. The deletion is
  a `change` now, as it happens.

**What you can delete.**

- A `pcall` around a runner call, and the `tostring(err)`, the
  `"runtime error: "` strip and the first-line cut that made its message
  readable.
- A Kit token copied into the app's `setTokens` by value, or a box's
  colour taken from `tokens()` by hand, kept only so a Node box painted
  in Kit's skin.
- A cast or a wrapper around a Kit component, kept only so its `on*`
  prop was checked against the app's messages.
- A context menu split into submenus, or a shorter menu offered in a
  small window, kept only so its last rows could be reached.
- A menu's point moved in from the window's edge, kept only so the menu
  did not cover the point it was opened at.
- A long menu cut into "More…" submenus, kept only so it stayed a
  reasonable height: `max_size` scrolls it at the height you choose.
- A Linux file manager asked over D-Bus by hand to select a file, and
  `open -a Finder` on macOS to show a folder rather than run it.
- A path rebuilt from its components, kept only so `open_external` or
  `show_in_folder` on Windows got one with `\` alone.
- Spaces, or an inline box, around a highlighted run, kept only so its
  background did not sit flush on its letters.
- A whole-page atlas upload on every `atlas_dirty`, or a shadow copy of
  the page diffed to find the changed rows: upload `atlas_dirty_rect`.
- A probe that walked the access tree, or opened and closed boxes one by
  one, to find which node a missing close swallowed: `unclosed-node`
  names it.

### The release itself

- **A release waits for the macOS and Windows rounds.** release.yml calls
  smoke.yml as its `smoke` job, beside `check`: the workspace tests,
  every windowed example and the C round on macOS and Windows, which
  `packs` and `publish` wait for. alpha.67 and alpha.68 went out with
  those rounds run only on pushes to main, where they do not block.
- **The GitHub release becomes the latest by itself, last.** `packs`
  creates it as a pre-release; a new last job, `latest`, makes it a
  release and GitHub's latest once the crates and npm packages are up
  and the assets attached, when its version sorts highest (npm's rule).
  `releases/latest/download/install.sh` names the newest version once
  its run is done.
- A manual run of release.yml stages no npm packages: it publishes
  nothing.

### Native verification

A pre-tag pass on the release machines after two releases that had none:
alpha.67's and alpha.68's macOS and Windows rounds had run only on
GitHub's hosted runners (software adapters, no input).

On Windows 11 (RTX 5080, 150%, en-US + ru-RU) and WSL Ubuntu 24.04:
fmt, clippy with `-D warnings`, `test.nu --node` (3255 tests over 238
suites and Node's on Windows), the C round, Odin's generator, check, tests
and slots (WSL), the Node examples' typecheck, `cargo audit`,
`release-prep.nu`, the headless round, and the windowed round with the
Node windows — all 58 examples on both bases on the RTX, and under X11 in
WSL. The wgpu capture and opacity tests on the RTX under D3D12 and Vulkan
(`KUI_REQUIRE_GPU=NVIDIA`). By hand on Windows: F210's menu popup (a
window of its own, held to the work area, the owner keeping the
foreground, its rows in its own UIA tree and none in the owner's), F226's
paths (`/`, spaces and Cyrillic reach Explorer with the file selected),
`kui pack` to an `.exe` (GUI subsystem, runs from its archive), and a
process group's kill (`cmd` → `ping`, the tree gone, nothing orphaned).
Ten minutes and then an hour of all ten fuzz targets in WSL, and the
corpus reuse check after.

What it found is above: F241 (smoke.yml's macOS leg), FZ16 and FZ17 (the
fuzzers), F242 and F243 (Windows), and two `kui-cli` tests that assumed
an `echo` program. Open, in the backlog: FZ18 (cosmic-text's shape-run
cache keys no direction; not a regression, alpha.68 the same) and F244
(Vulkan instances made on several threads at once on a machine with an
AMD driver beside NVIDIA's). A release dry run of release.yml with the
blocking smoke legs passed on the branch before the merge.

Not verified natively this release: the macOS accessibility audit and
anything on a Mac by hand, real keyboard and pointer input on Windows
(the menu bar's Edit actions, accelerators under ru-RU, Kit's select
keys), the `latest` job, which only a tag's run exercises, and FZ17's
fix under the WSL leg (Windows and the regression replay only; the tag's
`check` runs it on Linux).

## 0.1.0-alpha.68 (2026-10-27)

**What breaks.**

- **A float anchored to its parent is cut by the box it is in** (backlog
  C151, [ADR 0074](docs/adr/0074-paint-stays-in-the-box-it-is-in.md)): a
  `parent`, `below` or `above` float that showed past a clipping or
  scrolling ancestor — a menu, a popover or a tooltip of an app's own —
  is cut at that ancestor's edge, and not hit past it, until it says
  `escape` (Rust `FloatConfig::escaping()`, `escape` in JSX's and Lua's
  float, C `float_escape`, Odin `Float.escape`). A `viewport` float is
  unchanged. `clip` on a float is what it does now: JSX and Lua read the
  key and drop it, C stops reading `float_clip`, Rust's
  `FloatConfig::clipped()` is deprecated and does nothing, and Rust's
  `FloatConfig { clip, .. }` and Odin's `Float{clip = ..}` no longer
  compile — drop the field.
- A node playing its `exit` is cut by the box it left, where it was drawn
  outside every clip: a row sliding out of a scroller goes under the
  scroller's edge (backlog C151, ADR 0074).
- Kit's toast leaves by sliding out past the window's right edge, whole
  and opaque, where it slid 32 px and faded (backlog C154); its toaster
  leaves the same way.
- Kit's line chart's readout box is placed inside the plot's width and
  height, where it could hang past the plot's right edge or foot
  (backlog C153).
- A Kit `text_field`'s `error` shows whole: beside the text while the
  field has room for it, and under the field, wrapping, while it has not
  — where it was cut short at the field's end with the whole message in a
  tooltip. A narrow field in error is taller by the message's lines
  (backlog C156).
- A Kit line chart with a `window` draws its lines and area from the row
  before the window's first (before the slide's, while it slides), cut at
  the plot's left edge, where they started at the first row at or past
  it (backlog C157).

- A key sink's press under a Mac's Option carries what the Option typed
  in `text` — ⌥- `–`, ⌥⇧- `—`, ⌥m `µ` — where it carried null; under an
  Option `optionAsAlt` names it is null as before (backlog F195).
- A held drag-select or caret drag scrolls its scroller from inside it:
  in an edge zone about two lines deep, at a speed that ramps up while
  the pointer is held there, where it scrolled only once the pointer was
  past the edge, at 10 px/s a px past (backlog F196). Gone with it:
  `runtime::follow::AUTOSCROLL_RATE`, `AUTOSCROLL_REACH` and the step
  they made; `CLOCKLESS_FRAME` stays.
- The devtools' tree tab no longer outlines the selected node over the
  app: `kui-devtools/outline-selected` is drawn only while the tab's
  highlight toggle is on (`devtools_act("highlight")` in a test).
- The devtools' tabs are four: `Ctrl+Shift+N` walks facts → events →
  tree → **perf** → the declared tabs, so a test that counted presses to
  reach a declared tab presses once more; the latency graph is the Perf
  tab's, not the Facts tab's. An app's own tab named `perf` keeps its
  name: `set_devtools_tab` reaches a declared tab of the name, exactly,
  before the panel's own.
- The devtools' tree arrows select the row they land on, and Enter puts
  the selection down rather than picking it (backlog F201).
- A mouse wheel's plain notch over a scroller that moves only sideways
  scrolls it, while it has room, where it went to the page around it;
  and a notch with Shift held scrolls sideways off macOS, where it
  scrolled down (backlog F202, ADR 0050).
- A held drag-select or caret drag, and `Ui::edge_scroll`, hand an axis
  their scroller cannot move on to the scroller around it, where they
  stopped; `overscroll: contain` keeps it in (backlog F204, ADR 0029).
- `MenuAction` has a sixth variant, `ShowInFolder { path }`: a Rust host
  that matches the enum whole adds an arm (backlog F203).
- A `nowrap` text in a box that scrolls x is as wide as its line, where
  it was cut to the box: the box's content and `scroll_geometry` grow by
  the overflow, and the box scrolls to it (backlog F205).
- The devtools' placement buttons are in the strip's "⋮" menu
  (`kui-devtools/more`), so `kui-devtools/dock:left` and the rest are in
  the frame only while it is open; the tabs are one line that scrolls,
  where they wrapped onto more (backlog F206).
- A function key pressed in a focused editor goes to the `onKey` sink
  above it, where the editor kept it and did nothing with it.
- Under custom chrome a press on the drawn menu bar's empty room starts
  a window drag, while no menu is open; a press on a title still opens
  its menu.
- kui-lua no longer depends on kui-ffi: a Rust host that builds a
  `LuaExtension` itself and whose script loads a C plugin with
  `env.add_extension` calls `kui_ffi::register_native_loader()` first
  (backlog C59, ADR 0059).
- `EventKind` and `kui_core::Event` gained `Component`, a component's
  event (ADR 0060), `Range`, a node's rows in view (ADR 0065), and
  `Query`, a node's matched `when` entries (ADR 0069): an exhaustive
  `match` on either adds three arms.
- C's ABI is 39: `KuiSpec` gained `on_range` (808 bytes on 64-bit, from
  800; ABI 38), the slot doors went (ABI 39, below), and `KuiSpec`
  gained `focus_ring` (816 bytes; backlog C93), then `min_column`,
  `hidden`, `on_query` and `when` (840 bytes; ADR 0069), and later in
  the release `text_color` (888 bytes; backlog C130) and `float_escape`
  (888 bytes still, into the tail padding; ADR 0074), with `float_clip`
  no longer read. Recompile; zeroed, a node reports no range, colours no
  ring, has no rules, and a parent-anchored float is cut by its parent's
  clip.
- `NodeSpec` gained `hidden`, and `InteractSpec` `when` (ADR 0069) and
  `text_color` (backlog C130): a struct literal names them or ends with
  `..Default::default()`. `rules::field`'s bits and `Patch::set` are
  `u64`, `Patch::apply` takes a `u64` mask.
- `SystemEnv` gained `contrast` (`ContrastPref`, ADR 0069): a struct
  literal names it, and the `system` event's payload carries it.
- `schema::Kind`, `schema::Apply` and `schema::Parsed` gained `Rules`, a
  `when` list: an exhaustive `match` adds an arm.
- A frame whose layout overturned a lazy branch's decision is built
  twice by kui-native's runners, `kui_run` and Node's `createApp` (ADR
  0069): a view with side effects runs twice for that frame.
- A Lua script's `env` is one table shared by every fill of the script,
  its facts written again as they change (backlog C65): a script that
  stored its own fields on `env` finds them there next fill, and one of
  `env`'s functions called outside a view or a component's function is an
  error, as it was once the view's scope ended.
- Every node's key is a different number: `Key::str` and `Key::index` mix
  their tag in as a leading byte (backlog C71). A key printed, stored or
  compared against a literal across versions is a different key; keys
  derived in the same build are as stable as before.
- `slot::Fill::placed` returns a `bool`, whether the instance heard one of
  its own events since it was filled (backlog C68): an implementation
  outside kui returns `false`.
- `FillAnswer` (`SlotFill` until this release) gained `Heard` (C:
  `KUI_FILL_HEARD`, 6): an exhaustive `match` adds an arm.
- A fill being kept that looked a `$token` up is filled again when any
  origin declares its tokens anew, where it was replayed (backlog C69).
- A Lua view draws the children after one left out with `nil` — `{ a,
  cond and b or nil, c }` draws `c` — where it dropped every sibling
  after the hole (backlog C79).
- A kept fill that asked for one of its own nodes by label before
  declaring it runs again the frame after the node first exists, where it
  replayed its first frame until its params changed (backlog C84).
- **Slots fold into components** (backlog C86, [ADR
  0067](docs/adr/0067-slots-fold-into-components.md), whose last table is
  the migration). Gone: `Ui::slot`, `slot_with`, `slot_kept`,
  `slot_replay`, `slot_declared`, `Core::slot_fill`, `slot_fill_why`,
  `slot_kept_nodes`, `Extension::slots`, `EventKind::Reply`,
  `slot::MAX_REPLY_HOPS`; Lua's `slots` global (refused at load), `fill
  {}` and `env.slot_fill`; Node's `<slot>` and `ctx.slotFill`; C's
  `kui_slot`, `kui_slot_kept`, `kui_slot_replay`, `kui_slot_fill`,
  `KUI_SLOT_*`, `kui_reply` and the `kui_ext_slots` export (refused at
  load); Odin's `slot*` and `reply`. Place with `component`, replayed by
  default; speak with `emit`.
- Renamed: `Slot` is `Instance`, `SlotFill` `FillAnswer` (C
  `KUI_FILL_*`, `KUI_FILL_NONE` = -1), `ROOT_SLOT` / `ANY_SLOT`
  `ROOT_COMPONENT` / `ANY_COMPONENT`, `kui_slot_name` / `_namespace` /
  `_params` `kui_instance_*`, and an event's `slot` is `instance` (Rust
  `UiEvent::instance`, C `KuiEvent.instance` in the same place, Node
  `ev.instance`, Lua `at.instance` beside `at.component`, its full name).
- A devtools tab names a component and its props:
  `Ui::devtools_tab(name, label, component, &props)`,
  `kui_devtools_tab(ctx, name, label, component, props)`, `<devtoolsTab
  component props/>`, Lua `devtools_tab { component =, props = }`.
- The warnings `unknown-slot` and `duplicate-slot` are gone, and
  `recursive-slot` is `recursive-component`.
- An extension's `root` is drawn fresh every frame, never replayed.
- A runner app's `load`, `loadfile`, `dofile` and `require` take Lua
  source only; bytecode loads only from a packed app's own files (ADR
  0068). An app that `load`ed a `string.dump` gets nil and the reason.
- `@qxuken/kui` no longer carries every platform's addon: it names
  `@qxuken/kui-<platform>` as optional dependencies and npm installs this
  machine's (ADR 0068). An install with `--omit=optional`, or from a
  lockfile written on another OS (npm/cli#4828), has no addon and says
  how to fix it; `KUI_NODE_LIB` still points at one by hand.
- In a runner app F12 is the devtools' key, and the devtools are on,
  hidden, until it brings them up at the Log tab: an app's own F12
  binding is now the panel's, unless its manifest says `devtools_key =
  false` (F12 the app's, Ctrl+Shift+I the panel's) or names another
  chord (ADR 0068).
- `Instance` gained `slot`, the contribution slot it was placed in: a
  struct literal adds `slot: None`. `<slot>` in Node and `slot(...)` in
  Lua are back with the new meaning, a contribution slot (below).
- Node's wire protocol is 27: a prebuilt addon from before refuses the
  stream.
- `slide` eases a node's place within its parent, not in the window
  (backlog C97): a move an ancestor makes — a resize, the parent's own
  slide — is followed at once, and `slide` on a node that does not move
  within its parent (a wrapper's child, where the wrapper is reordered)
  no longer animates.
- Escape in a multiline editor whose Tab types keeps the keyboard there
  and arms the next Tab to move focus on (backlog C99), where it blurred
  the editor; a second Escape blurs it as before. Shift-Tab there takes an
  indent back from the start of the line, where it typed four spaces.
  A multiline editor declared `tab_moves_focus` (or a field) still blurs
  on the first Escape.
- A primary press on a node that declares no press handler and is not
  focusable — one with only a hover colour, a cursor, a hover group or
  an `onHover` — goes to the nearest ancestor that takes a press (a
  click, a drag, a slider's track, a key sink, window chrome, an editor,
  a focusable node), which clicks, drags, slides or focuses as if pressed
  itself (backlog C91); it went nowhere. Hover is the child's as before,
  and both show pressed while the pointer is on the child. A disabled
  ancestor that declares a handler keeps the press, doing nothing, and
  the walk stops at a modal's edge and at the selection scope the press
  is in. `HitRegion` gained `press_to`: a struct literal adds
  `press_to: None`.
- A focused `onKey` sink draws the core's focus ring while focus is
  keyboard-visible, as a control does, where it drew none (backlog C93);
  the root never does. And a key a sink takes — a press delivered to it,
  a modifier alone excepted — makes focus keyboard-visible, as a
  browser's :focus-visible is after a key: a list clicked and then walked
  with the arrows showed no ring. A surface that shows its own focus
  turns the ring off with `focusRing` transparent (below) or `focusBg`.
- The editing keys' `Mods` gained `ctrl`, Control itself (backlog C94),
  and so did `event::Boundary` and a `boundary` event's data (`{key,
  edge, word, doc, ctrl}`): a struct literal adds `ctrl: false`. Off a Mac
  it is held whenever `doc` is; on one it is the key ⌘ is not, and Ctrl+Tab
  there is `ctrl` and no longer `doc`. Node's `key(name, mods)` takes
  `ctrl`, and C has `KUI_MOD_CTRL` (8).
- `access::Role` gained `Table`, `Row`, `Cell`, `ColumnHeader` and
  `RowHeader` at its tail (`Role::ALL` is 32; C `KUI_ROLE_TABLE`,
  `KUI_ROLE_ROW`, `KUI_ROLE_CELL`, `KUI_ROLE_COLUMN_HEADER`,
  `KUI_ROLE_ROW_HEADER`; ADR 0070): an exhaustive `match` adds five arms.
  Page Up and Page Down on a focused `list` item move the focus by a page
  of rows, where they did nothing and, under a sink, bubbled to it.
- A layout table (`table()`, `dir="table"`) that declares no role reads to
  assistive technology as a `table` of `row`s of `cell`s (a text or image
  in a row a cell named by its text), where its texts were loose children
  of the window; with focusable cells it is a composite, one Tab stop
  (ADR 0070). A clickable row is still a button.
- `AccessNode` gained `row_index`, `column_index`, `row_count`,
  `column_count` and `cells_focus` (Node `rowIndex` and the rest; C
  `KuiAccessNode`'s four fields, appended inside ABI 39, with
  `KUI_ACCESS_HAS_ROW_INDEX` and its neighbours and
  `KUI_ACCESS_CELLS_FOCUS`); a windowed list's items report their data
  index as `pos_in_set` and the list its `rowCount` as `set_size`. And
  `KuiSpec` gained `on_typeahead` and `active_index` after ADR 0069's
  fields (856 bytes on 64-bit), so a C host recompiles.
- `KuiSpec` gained `essential_motion` (backlog C107), `dx` and `dy`
  (C105), `opens_on_press` (C108) and `on_item_key` (C120), 880 bytes on
  64-bit, inside ABI 39: a C host recompiles; zeroed, nothing changes. `KUI_ENTER_KEEP_ROOM`
  (C110), `kui_set_rules_appearance` (C113) and `kui_platform` with its
  `KUI_PLATFORM_*` codes (C116) are new.
- `EventKind` and `kui_core::Event` gained `ItemKey`, a key a composite's
  item left (backlog C120): an exhaustive `match` adds an arm.
- A Delete, Backspace, unclaimed Enter or function key pressed on a
  composite's item goes to the composite's `onItemKey` where it declares
  one, ahead of an `onKey` sink above it, which heard it before (backlog
  C120). A sink nearer than the composite still hears it, and chords
  still go to the sink.
- `schema::EnvFacts` gained `platform` (backlog C116): a struct literal
  names it. In a `kui run` app and in Kit, `env.load_tokens` reads a
  relative path against the app's folder (a packed app's extracted one),
  as `kui.fs` does, where it read against the working directory.
- Under reduced motion a `keyframes` cycle that runs for ever holds still,
  the node showing its own props, and a finite one rests where it ends,
  where both ran (backlog C107): a spinner or an under-way bar that must
  keep moving declares `essentialMotion`. `AnimSpec` gained
  `essential_motion`: a struct literal names it or ends with
  `..Default::default()`.
- `TransformSpec` gained `translate` (`dx` / `dy`, backlog C105), and
  `Enter` gained `keep_room` (C110): a struct literal adds them.
- The rules' state bits are `u16` (backlog C106): `Cond::state_mask`,
  `Cond::state_want`, `rules::Facts`'s state and the `rules::state`
  constants, which gained `GROUP_HOVERED`, `GROUP_PRESSED`,
  `GROUP_FOCUSED` and `GROUP_FOCUS_VISIBLE`; `rules::field` gained `DX`
  and `DY` and its `NAMES` holds 32.
- `DismissReason` gained `Accelerator` (backlog C109): an exhaustive
  `match` adds an arm, and a modal's `dismiss` may carry `reason:
  "accelerator"` — a key under ⌘ (Ctrl off a Mac) pressed while an app's
  `role: "menu"` modal is open, which closes it, or a row of the
  platform's menu bar chosen then. A view that closes on any dismissal
  needs nothing.
- The stock `select` opens its menu on the press, not the release, and a
  release over a row of the menu picks it (backlog C108); a press held on
  the field and released there leaves the menu open as before. A test
  that clicked the field sees the menu open after the press.
  `InteractSpec` gained `opens_on_press`.
- `TokenLookup::colors`, `lengths` and `families` — Lua's `env.tokens`,
  Node's `tokens()` — list every other origin's exported tokens after the
  reader's own and the host's (backlog C112): a listing walked whole sees
  more names.
- Kit's `line_chart` draws its axes otherwise: x ticks as close as their
  measured labels allow (never under 50 px apart) where a tick came every
  90 px with an 80 px label, tick numbers with as many decimals as their
  step, a y gutter as wide as its widest label where it was 44 px (56
  with a unit) and its labels right-aligned, and a new value range glides
  in over 240 ms where it jumped.
- A Kit `settings` brand accent given as a colour derives the palette
  around it: the roles the skin paints in its accent take it (Ledger's
  selection, Neutral's focus ring, a view switch's chosen segment, the
  first chart series and waterfall group), those in its hue take the new
  hue (all of Phosphor's greens), the ground, surfaces, hairline and
  washes lean toward it, and every text role, tone and the focus colour
  is moved in lightness until it reads at 4.5:1 on each surface — unless
  the brand names those roles too; an accent given as a token name
  changes nothing else, as before. An accent notice's and toast's text is
  the new `accent_text` role, Ledger's yellow made readable on paper.
- A Kit table's or list's badge on the selected row keeps its chip, on
  the panel's colour, where it lost it to the selection's fill.
- A focused, closed Kit `select` opens on Up, Down, Alt+Up/Down, F4,
  Space and Enter; its field is a key sink.
- Kit's tones and focus colour are moved in lightness until they read at
  4.5:1 on every surface in every skin, accent or none (Ledger's blue
  focus and info text are a shade darker); a neutral notice is framed in
  the faint ink, not the hairline.
- A Kit line chart draws a hover layer over its plot (its `readout`, on
  by default; `readout = false` leaves it out), and its buckets sit on
  whole multiples of their row count, so its first bucket reaches before
  the window's first row and is cut at the plot's edge.
- Kit's `select` opens on the press, its menu over the field with the
  current option on it, where it opened on the click and hung under the
  field; a test that clicks an option by a press and a release elsewhere
  presses on the option.
- A Kit `table`'s header and rows sit in a sideways scroller (`wide`,
  around a `sheet` as wide as its columns need: a sharing column's
  `min_width`, 80 px unset), so a table narrower than that scrolls where
  it squeezed its columns to nothing; every cell is keyed `c1`, `c2`, …
  where only a table of cells keyed them.
- A Kit `slider`'s fill eases to a new value (a click, a key, the host)
  where it jumped; under a clock a test that sets the value and reads the
  handle at once reads it on its way. A drag is followed directly, as
  before.
- A Kit line chart's hover bands are as wide as the rows they name, by
  the line's x scale, where each was 3 px naming the row at its share of
  the row count; a host's `on_hover` hears the row nearest the pointer.
- A key sink that is a stop only for being a sink — no `focusable`, no
  editor, no control role, no `onClick` — with a Tab stop inside it is no
  Tab stop of its own: Tab walks the controls under it, where it landed
  on the sink first (ADR 0011, decision 11). A sink with nothing
  focusable inside — a terminal — is a stop as before. And `focus_next`
  from a focus off the ring goes on from where it is (the node a press
  landed on, when the press gave a shell around it the keyboard), where
  it went to the first stop.
- A tooltip's hint is keyed under the box that declared it
  (`box.str("tooltip")`, as a leaf's already was), where it took the
  box's next child key.
- A Kit line chart's readout follows the pointer through `pointer_in`
  (C119): its crosshair stands at the pointer's x and names the sample
  under it, where 3 px hover bands named a row each — a band that kept
  its key while a live window slid kept its old row, and the crosshair
  travelled with the data. The bands are drawn only for a host's
  `on_hover`, each keyed by its row. A `focusable` chart is a slider over
  its samples (role `slider`, the readout its value) and no key sink: the
  first arrow steps from the newest sample (Left a sample back, where it
  showed the newest), Page Up and Down move ten, a press on the plot puts
  the keyboard's readout under the pointer, and every other key — Tab,
  the app's chords — goes on as from any control. Its readout box is
  beside the plot, no longer inside it. Only a chart that says
  `focusable = true` is a Tab stop (a bar chart's bars, with `on_select`):
  left out, Tab passes it by and the pointer's readout works the same;
  the gallery's live line card says it, the rest of its charts do not
  (backlog C158).
- Kit's text roles — `fg`, `muted`, `faint` and the tones — read at
  4.5:1 on the hover and press washes too, so they move in lightness
  where they did not (backlog C131): Ledger light's `muted` and `faint`
  are a shade darker, Neutral light's too, Phosphor's a shade further
  from its ground in both halves, and with an accent each moves as far as
  the accent's washes ask. A test or a picture that pinned a role's value
  reads the new one.
- Kit's waterfall group colours (`$kitGroup1`…`$kitGroup6`) are
  colours, not tints: each reads at 3:1 on every surface, in lightness
  tiers taken in turn (on a light ground a mid tone, then two darker; on
  a dark one a mid-dark tone, then two lighter), and its label is the new
  `$kitGroupFg<n>`, where it was `fg`. The selected span keeps the
  selection's fill with a 2 px edge in the text colour; a time mark is
  drawn in `$kitAccentText`. An app that painted with a group token draws
  the new colour, and text on it reads with `$kitGroupFg<n>`.
- Kit's `faint` is dimmer again in the dark halves: asked to read on a
  segment's hover and press over its track too, it had gone most of the
  way to `fg`, and nothing faint is drawn there (`fg` and `muted`, a
  segment's label and count, still read there).
- Kit's design system moved into the Kit app: `addons/kit/design-system/`
  is `addons/kit/app/design/` — the brand book, `tokens.json`, and a
  guideline per component as `components/<Component>.md` — and the static
  HTML previews and `bundle.css` are gone; the app draws each.
- A click whose message is a map carries what it was made with in its
  `click` data: `mods` while a modifier is held, `clicks` from a double
  click's second click on, `key` (`enter` / `space`) when a key made it;
  a `focus` event the keyboard or the pointer made under a modifier
  carries `mods`. A handler that compared such a message whole reads the
  new field; a plain click, and a bare message (`on_click("go")`), arrive
  as before. `onItemKey` hears select all (⌘A, Ctrl+A off a Mac), which
  bubbled to the key sink above.
- A Kit `list` or `table` enters the rows by Tab on the row the keyboard
  was last on, then the selection's, where it entered on the selection;
  and with an `on_activate` handler Enter activates a row rather than
  selecting it (without one it selects, as before). Its `select` event
  in a multiple selection carries `ranges`, `count` and up to 1,000
  `rows` beside `row`.
- A Kit line chart with a `window` scrolls between samples: drawn one
  sample behind, its lines, areas and x ticks slide left over the time
  samples take to come, a keyframe cycle of the core's keyed by the
  sample (`slide<n>`), where it stepped a sample at a time. A test that
  reads its quads between samples reads them slid; under reduced motion
  or with no frame clock it steps as before. Scrolled out of view it
  slides still, and costs nothing: the core runs no frame for a cycle it
  culled (backlog C137).
- A press on a Tab stop inside a key sink focuses the stop, as a browser
  focuses what a click lands on, where it gave the sink the keyboard
  (backlog C135, ADR 0002 decision 9 amended). The sink keeps its chords
  — a key the stop does not claim bubbles to it (ADR 0011) — but what
  the stop acts on stays with it: Space and Enter on a button, the
  arrows on a slider. A surface an app draws inside its sink that must
  leave the keyboard there — a multiplexer's pane — says `keepFocus`.
- A focus left on a node the frame no longer has moves to the nearest
  key sink it had around it, or is dropped when it had none, where it
  stayed on the vanished node (backlog C136). A test that read `focus`
  after a page change reads the sink, or nothing.

An editor that keeps its own text and hears keys through `onKey` could
not type a dash, or anything else a Mac types with Option: the press
arrived as the chord on `-` with no text, and the character went only to
kui's own editor. It now arrives as both, so a sink that inserts a
press's `text` types what TextEdit types. A sink that inserts `text`
whenever there is some, and also binds the chord, now does both on one
press — look the binding up first and insert only when none matched, or
name that Option in `optionAsAlt`. Windows and Linux are unchanged:
their Alt composes nothing.

A drag that selects text now scrolls near the edge, not only past it. In a full-screen or
maximised window the pointer stops at the monitor's edge, a pixel from
the scroller's, so the old rate — set by the distance *past* the edge —
had nothing to work with, and a window-filling list did not scroll at
all. A test that drove a held drag past an edge and counted pixels
counts different ones; set the old rate back for it with a flat feel,
`EdgeScrollFeel { zone_px: 4.0, speed_px_s: 40.0, curve: 1.0, ramp_secs:
0.0, ramp_max: 1.0, cap_px_s: 1000.0, .. }`, which is 10 px/s for each px
into a 4 px zone. The devtools' three
breaks are the panel's: a test that drove it and read a selection's
outline, counted `Ctrl+Shift+N` presses to a declared tab, or walked
the tree's rows by arrow and expected no selection, is the one that
notices; an app's own tree, keys and events are as they were. A
mouse wheel's notch over a row that scrolls only sideways, or turned
with Shift held, moves that row now: a test that sent one and read the
page's offset reads the row's, and a Windows or Linux user scrolls
what was out of reach before.

Every host now loads a `.lua` extension, and the price of that is
kui-lua giving up its kui-ffi dependency, which was there for one call:
opening the C plugin a script names. kui-ffi depends on kui-lua instead
and hands it that loader. A host that loads through `kui_ffi`'s door
(`open_extension`, `kui_ctx_add_extension`, Node) has it registered for
it. A Rust host that builds its `LuaExtension` by hand and lets the
script open a C plugin adds one call before the first frame; without it
the script's `env.add_extension` answers nil and a message naming the
path, as it did for a missing file. A script loading another script
needs nothing.

Kit found four faults in the instance path while it was being built, and
three of them change answers. An instance whose own event moved its
extension's state — a select opening, a waterfall zoomed by the wheel —
was replayed with the frame before; it is now asked again, and says
`Heard`. A kept fill that painted with `$kitPanel` was replayed after
the add-on declared another skin; token lookups are now a read the replay
checks. And a component's 84th host child had the key of the component's
first node, because a label's tag and an index's traded inside FNV's
first round; the tag is now a byte of its own, which moves every key.

`cond and node or nil` is how a Lua view writes a child that is sometimes
there, and in a list's middle it lost every child after it: the walk read
`#t`, and Lua 5.5's length starts from a hint and stops at the first
hole. The prelude's containers (`row`, `column`, `grid`, `titlebar`,
`tooltip`), a component's host children and a text's spans now close the
gaps — at the cost of a length and one `next` per container when there
is none — so a view that drew less than it said now draws what it says.

There was one way to place what an extension draws too many. A slot —
one named place a frame, filled by whatever was loaded under its
namespace — was a component placement with fewer powers and an event path
of its own: an untyped reply where a component speaks the host's own
message, replay by asking (`slot_kept`, `slot_replay`) where a component
is replayed unless it opts out, a key by name, no host children. It
folds in: a host places `component("ns/panel", props)` wherever it
placed `slot_with("ns/panel", params)`, passes the reply it wanted as an
`on_*` prop, and the extension answers with `emit("on_toggle", fields)`
where it returned a table. A plugin with only a `view` is untouched — its
`root` still follows the host's view. The word comes back in the next
step with the one meaning a component cannot have: a place where any
number of extensions append components — three plugins each adding a
button to one toolbar, without the host naming them.

A switch's knob that slides fell out of step with its track whenever the
window resized: the track moved with layout, and the knob eased towards
where the track had gone, by its own transition. `slide` eased every
change of the node's position, whoever made it. It now eases the node's
place within its parent — the node it is laid against, which is the
anchor for a float and the window for a viewport float — so what the
ancestors do is followed on the frame they do it, and a nested slide adds
only its own move to its parent's, where it trailed its parent's ease by
its own. Reordered rows slide as they did. What changes is a `slide` put
on a node whose parent is what moves: on a row's child rather than the
row, it did glide and now rides along — move `slide` to the row. A keyed
node that changes parents glides on from where it was drawn.

The rest of this release breaks nothing else: a fix to text
at a size no screen shows, a declaration file for Teal beside the one
for lua-language-server, and a new runner beside the window's.

### Added

- **What a node paints stays in the box it is in** (backlog C151, [ADR
  0074](docs/adr/0074-paint-stays-in-the-box-it-is-in.md)): whatever its
  float, move, keyframes, turn or sibling order, a node and its subtree
  are cut by the nearest box above it that clips or scrolls, unless it
  declares it leaves — a viewport float, or `escape` on a float anchored
  to its parent. Five reports were this one bug, each patched in its
  component with `clip = true`: Kit's slider handle over the app's bar, a
  checkbox's focus ring, the skeleton's shimmer band, and Kit's live line
  chart drawing over the window's bar and into its y gutter. The core's
  own floats that must leave say `escape`: the hint under a hovered
  control, a submenu, the menu bar's panel, the devtools' "⋮" menu. A
  node leaving with an `exit` keeps the clip it was held by. Node's float
  stanza carries `escape` as bit 4 of its flags word (protocol 27).
- **The paint audit** (backlog C152, ADR 0074): `Core::set_paint_audit`,
  or `KUI_PAINT_AUDIT=1` under kui-native's drivers, checks every frame's
  quads, glyphs and masks — each cut by the clip it names — against the
  rect the boxes above its node leave it, by what the node declares, and
  raises a `paint-escape` warning per node that drew outside;
  `Core::paint_audit` reads the counts, and a headless run prints them at
  its end. The Kit gallery's walk under it
  (`nothing_the_kit_app_draws_leaves_the_box_it_is_in`, at scale 1 and
  1.5) tours every page in every skin scrolled under the bar, the live
  chart sliding, a slider springing, a checkbox's ring swept past the
  edge, skeletons, toasts leaving, a menu, a popover and a dialog; on the
  old rule it found about 2,400 nodes, the chart's sliding layer and
  axis labels among them.
- **`kui run --scale S`** draws a headless run or a `--shot` at a
  display scale: `1.5` as a 150% display does.
- **Props by condition** (backlog C100, [ADR
  0069](docs/adr/0069-props-by-condition.md)): a node's `when` list —
  entries of the conditions they test and the props they set while they
  hold — resolved by the core, so a view declares a responsive, themed,
  interactive box once and is not run again for a resize, a theme switch
  or a hover. The conditions: the node's own `width` / `height` (`{
  below, above, band }`, lengths or `$length` tokens, the band holding a
  range past its threshold so a size resting on it does not flicker), the
  window's (`window: { width, height }`), `dark`, `light`,
  `reducedMotion`, `contrast: "more"`, `platform`, `scale`, and the
  node's state — `hovered`, `pressed`, `focused`, `focusVisible`,
  `disabled`, `selected`. Every matching entry applies in order;
  `hoverBg` / `pressedBg` / `focusBg` are entries ahead of the list.
  Window, environment and state entries resolve as the node opens, and
  what they set eases under its `transition` (jumping under reduced
  motion); own-size entries resolve in layout, before the node's
  children are laid across it, with one more pass on the frame a height
  match moves — no frame is drawn with a stale match. An own-size entry
  sets the layout of the node's children only (direction, gaps, padding,
  wrap, alignment, `minColumn`, and `hidden` naming keyed children to
  drop), and the node is a container in that axis: a `fit` there grows
  instead, with `query-on-fit`. What an entry may not set is left out
  with `rule-prop-refused`. JSX `when={[…]}`, Lua `when = { … }`, C
  `KuiSpec.when` (a `KuiValue` list), Odin `Spec.when_`, Rust
  `NodeSpec::when(Cond::width_below(700.0).band(16.0), |p| p.gap(8.0))`.
  Also: `hidden` (a node out of the frame, its keys and state kept),
  `minColumn` (a row wrapped into equal columns as many as fit — CSS's
  `repeat(auto-fill, minmax(n, 1fr))`), `onQuery` (`query: { matched,
  width, height }` when the own-size and window matches change), the
  increased-contrast setting as `env.system.contrast` (read on macOS and
  Windows; C `kui_env_set_contrast`), and the lazy form: a branch built
  only when its parent offers the room it `needs` — laid out at exactly
  that size — or while a `query` holds, with a `fallback` otherwise:
  `ui.lazy(Needs::w(320.0), …).fallback(…)`, a child table `{ needs =,
  render =, fallback = }` or `lazy { }` in Lua, `<Lazy>` in JSX,
  `kui_lazy` in C, `kui.lazy` in Odin. A lazy decision is taken from the
  last layout's room; when a frame's layout disagrees the runners build
  the frame once more before drawing it (`Core::rerun_owed`,
  `kui_rerun_owed`, `rerunOwed()`), a second disagreement waiting and
  warning (`query-unsettled`); a branch inside a component instance is
  replayed with it while its decision holds. The devtools' tree flags
  `rules`, `lazy` and `hidden`, and the inspector lists each entry with
  whether it matched. `frame_1k_rules` (a thousand nodes, three entries
  each, through a resize and a hover) is a guarded bench row.

  *What you can delete:* a view's `is_hovered` / `is_pressed` reads that
  pick colours, a resize handler that stores the window's width to pick a
  layout, a size read from last frame's `onLayout` to decide a row's
  direction, a branch on `env.system` for dark or reduced motion that
  only swapped props, and the grid arithmetic that computed a column
  count.

- **A channel reduces a range for a chart** (backlog C95):
  `Channel::reduce(col, from, to, buckets)` (and `reduce_into`, which
  keeps the buffer) answers, per bucket of near one size, the least,
  greatest, first and last value of a number column and the row each
  came from — a chart's pixel columns of a hundred thousand rows in
  about 64 µs (0.5 ms for a million), where Kit's Lua copied the column
  and bucketed it at 1.5 ms a series. A range shorter than `buckets`
  gets a bucket a row, `to` past the end stops there, an empty range
  gets none, and NaN is skipped, a bucket of only NaN reading NaN. Lua
  `ch:reduce(col, from, to, n)` (a list per field: `min`, `max`,
  `first`, `last`, `min_row`, …), Node `channel.reduce(...)` (a
  `Float64Array` per value field, a `Uint32Array` per row field), C
  `kui_channel_reduce` (`KUI_CHANNEL_BUCKET` doubles a bucket into an
  array the caller holds), Odin `channel_reduce`. `benches/channel.rs`
  measures it; the howto draws the line.

  *What you can delete:* a `ch:column` copied into a script to bucket
  it, and the bucketing loop.

- **A table of cells** (backlog C90, [ADR
  0070](docs/adr/0070-a-table-of-cells-and-composites-past-the-window.md),
  accepted after a grilling round): `role="table"` around `role="row"`s
  of `role="cell"`s — `columnHeader` and `rowHeader` cells among them, a
  layout `table()` implying all of it. With a focusable cell it is one Tab
  stop: Left / Right along a row, Up / Down between rows keeping the
  column, Home / End to the row's ends and with Ctrl (or the primary) to
  the table's, Page Up / Page Down by the rows its scroller shows; it
  clamps, and moving activates nothing. Enter or F2 enters a cell's
  controls, Tab cycles them and Escape comes back to the cell; a cell with
  one control presses it on Enter or Space. Readers get each cell's row
  and column and the table's counts (AccessKit `row_index` /
  `column_index` / `row_count` / `column_count`; `Grid` and `GridCell`
  when cells take focus, `Table` and `Cell` when not), a header linked to
  its column by index. A table that moves whole rows stays a `list`.
- **Composites that walk past their window** (backlog C89, ADR 0070): a
  `list` or `table` whose rows carry their data `index` under a
  `rowCount` (on the list or the scroller between, as `uniformList` and
  Kit's body declare it) steps by index over all its rows — End is the
  data's last row, not the window's — and a step to a row the frame has
  not built scrolls toward it and lands the focus when the next frame
  builds it, as `onRange` pagers already expect. A key pressed again
  before that frame goes on from the row reached for; focus moving any
  other way lets the reach go. Readers hear "row 120 of 4,000" with no
  `label`. Type-ahead searches the built rows first and then asks the
  composite's `onTypeahead` (`typeahead: {text}`), answered with
  `answer_typeahead(key, index)` in every binding (C
  `kui_answer_typeahead`, Node `answerTypeahead`, Lua
  `env.answer_typeahead`, Odin `answer_typeahead`). Tab back in lands on
  the row focus last held there, or on the one `activeIndex` names.
- **A hover group read by its name in every binding** (backlog C92):
  Lua `env.is_group_hovered(name)` and `env.is_group_pressed(name)`,
  Node `isGroupHovered(name)` and `isGroupPressed(name)`, C
  `kui_is_group_hovered(ctx, name)` and `kui_is_group_pressed(ctx,
  name)`, Odin `is_group_hovered` and `is_group_pressed`, beside Rust's
  `Ui::is_group_hovered` / `is_group_pressed` by id, and pinned in
  `schema::DOORS`. Kit read a pressed group another way.
- **The core focus ring in an app's colour** (backlog C93): `focusRing`
  (Lua and Rust `focus_ring`, C `KuiSpec.focus_ring`, Odin
  `Spec.focus_ring`) colours the ring for the node and everything under
  it that declares none, the nearest declaration winning, else the
  theme's `focusRing`; a `$token` resolves as any colour prop does, so an
  add-on sets `focus_ring = "$kitFocus"` once at the root of what it
  draws. A colour with no alpha draws no ring (in C, whose 0 is unset,
  `0xffffff00`), turning it off without touching the background as
  `focusBg` does. The devtools' node list, which draws its own focused
  border, turns it off so.
- **Show a file in its folder, or open it with its app** (backlog F203).
  `kui_native::show_in_folder(path)` shows a file or folder in the
  system's file manager — selected in Finder or Explorer, its folder
  opened elsewhere — and `kui_native::open_external(target)` opens a
  file, a folder or a URL with what the system opens it with: C
  `kui_show_in_folder` and `kui_open_external`, Odin `show_in_folder`
  and `open_external`, Node `showInFolder` and `openExternal`. Each
  starts the platform's own tool with the path as an argument, no shell
  between, returns once it has started, and fails, naming why, for a
  path that does not exist. The core touches no system: the devtools'
  "open folder" queues `MenuAction::ShowInFolder`, which the runner
  applies and a host of its own applies with the same door
  (`KUI_MENU_ACTION_SHOW_IN_FOLDER`, Node `{kind: 'showInFolder'}`).

  *What you can delete:* an app's own `open` / `explorer` / `xdg-open`
  spawn for "Show in folder" or a link.
- **The `kui` runner** (backlog C61, [ADR 0064](docs/adr/0064-the-kui-runner.md)):
  `kui run app.lua` runs a Lua or Teal app with Kit built in and a
  standard library — `json`/`toml`/`yaml`, `.env`, `fs`, `process`,
  timers, an HTTP client and server, SQLite and workers — whose waiting
  calls yield inside a task (`kui.spawn`) and never stop a frame. An app
  is `view(env)` plus `update(msg)`, which hears its events and every
  `kui.dispatch`; `init()` makes the `model` a `kui run --watch` reload
  keeps. `kui check` type-checks Teal against kui's, Kit's and the
  runner's declarations; `kui new` starts an app; `kui types` writes the
  declarations for lua-language-server and Teal; `--headless` and `--shot
  out.png` run with no window. `kui run addons/kit/app` is the Kit app:
  every component live, 100,000 rows built by a worker and handed over as
  a channel, a live chart, a log, and an HTTP server.

- **What a real app needs from the runner** (backlog C80, from Cookie
  Mapping rebuilt on it): the manifest sets the window (`min_size`,
  `chrome`, `titlebar`, `icon`); `kui run app -- args` hands the app
  `kui.args`, with `kui.cwd`, `kui.platform` and `kui.cpus`;
  `kui.process.spawn(cmd, args, { group = true })` leads a process group
  whose `p:kill("term")` reaches everything it started, a spawned process
  ends with the app unless `detach = true`, and `kui.process.kill(pid,
  sig, group)` signals one the app did not start (`0`: is it alive);
  `kui.next_frame(function(env) … end)` gives `update` the clipboard,
  focus and text measures; `on_exit()` runs however the app ends, and
  Ctrl-C, `SIGTERM` and `SIGHUP` end it the way its close button does;
  `kui.fs.create` writes a file only if it is absent. Kit's `statusbar`
  takes a `trailing` group, as its toolbar does.

- **The runner's second round** (backlog C83): `kui.parse_args(spec)` —
  options with types, defaults, short flags, `env`, `choices`, `many`, and
  positionals — writes `--help` from the spec and says a mistake with what
  to type instead ("did you mean --port?"), ending the app before its
  window; an app that ends while it loads opens none. `print` and
  `kui.log(...)` / `kui.log.warn(...)` are kept for a **Log tab in the
  devtools**, with the runner's errors and every worker's lines, coloured
  and wrapped; `kui.devtools()` toggles the panel and `kui.devtools("log")`
  opens it there, as `--devtools` and the manifest's `devtools = true` do.
  On macOS a windowed app is named after itself in the Dock and ⌘Tab, not
  "kui" (`--no-rename` keeps it). `kui.hash` (`sha1`, `sha256`, `sha512`,
  `fnv`), `kui.json.array` (an empty list that stays `[]`),
  `kui.fs.walk` and `kui.fs.read_all` (a tree or a batch of files off the
  frame), and `kui.process.command_line(pid)`.

- **Kit:** the log draws a message's ANSI colours in the skin's and wraps
  long lines (`wrap`; `time = false` drops the clock) (backlog C81); a
  checkbox's children draw its label — a name with search hits marked —
  with `hide_label`, and `grow` makes the whole row its target; `segmented`
  and `search` take a `label` that names them without drawing (backlog
  C78); a spacer among a toolbar's or statusbar's items pushes what follows
  to the end, where it stopped midway, and `tabs` take a `trailing` group
  for what acts on the chosen tab (backlog C82). Ledger draws fewer edges
  (backlog C85): its 2 px ink rules are for regions only — toolbars, tabs,
  dividers, a panel's title and an idle field's underline take a hairline
  in the soft rule colour, the split's rule is a hairline, a segmented
  control is a track with the chosen segment filled, and badges and
  notices carry their tone without an outline.
  A design pass from the same app, in all three skins: `section` takes
  `leading` and `actions` groups beside its heading and an `indent`, so a
  group's select-all box sits in its rows' checkbox column; a disabled
  button is the default variant without its shadow; `segmented` prints
  the chosen segment in the ink where controls are outlined; an idle
  field's underline is `faint` (3:1) and a focused one the region rule;
  `faint` is 4.5:1 on `panel`; and a toolbar wraps its trailing group onto
  a line of its own when both do not fit. `kui run --appearance
  light|dark` pins the appearance, which a headless `--shot` has no OS to
  ask for.
  From the samples: a dialog shows its body (it was laid out at zero
  height); a popover takes a `label`; a field's `focus` no longer replays
  when the field comes back after the counter moved, and `autofocus`
  focuses one as it first appears; a wrapped log follows to its end; a
  notice wraps its actions under the message when narrow; a toast sits
  inside the window's corner. New: `text_area`; `text_field`'s `error`;
  table columns toned by a `tone` column or drawn as `badge` pills, and a
  list row's toned `badge`; `line_chart`'s round x ticks with `x_format`
  (`number`, `ago`, `clock`), `x_unit` and `y_unit`; `bar_chart`'s value
  axis, value labels, wrapping names, `unit` and `horizontal`; `slider`'s
  `value_label`, `radio_group`'s `hide_label`, a toast's `action`.
  From running the samples: every button variant shades under the
  pointer and the press in every skin, and the shades are opaque — the
  skin works out `accent_hover`, `panel_press` and the rest — where a
  primary or danger button had no hover and every press was a translucent
  wash that showed Ledger's hard shadow through the fill, so the shadow
  seemed to jump. A danger button's label and every button's icon take
  the colour that reads on its fill (a new `on_danger` role): Ledger's
  light danger button had ink on red, and Neutral's icons stayed dark
  beside a white label. A log stops following the moment it is scrolled
  up, starts again at its end, and shows a Latest button while it is
  away (it pulled a scroll that had only begun back down). A switch's
  knob is one mark that slides, in 140 ms (its transition was 0.12 ms),
  over a track that fills behind it. A toast goes by itself after
  `duration` (5 s; 0 keeps it), counted again from when the pointer
  leaves it. A `text_area` is its editor from edge to edge, so a press
  anywhere in the well puts the caret there, and a text field's whole
  height is its editor. Ledger's table header takes a hairline, not a
  second 2 px frame inside its panel, and a panel's header leaves 4 px
  around the controls in it.
  A second pass from running them: a switch's knob slides — it moved by
  layout, which `transition` alone does not ease, so it took `slide` —
  and a `badge` with `fade` comes and goes with it (Preferences'
  "changed" popped in a frame while the knob was still crossing). Ledger's
  primary button lifts its face a pixel under the pointer and presses it
  into its hard shadow, whose far edge stays put; the face moves inside a
  still box that takes the pointer, so it cannot flicker its own hover. A
  slider's handle lights while the track is hovered and stays lit while
  held. Toasts are wider, carry their tone as a bar and an icon, slide
  out as they go, and stack in a `toaster` (each `inline`); `duration` is
  documented as the way to show one for a time or until dismissed. A
  determinate `progress` bar eases to its value on a short spring. A list
  or a table hands Tab on (a key sink owns Tab, so it went in and never
  came out) and draws its edge in the focus colour while it holds the
  keyboard; a list takes a `label`. A wrapped log keeps its measured rows
  when a revision brings the same ones (it blinked on every republish),
  and when the oldest rows drop off a reader scrolled away keeps the rows
  they were reading where they are; a new channel counts as a new
  revision. A table takes `border` (a hairline frame, the header ruled
  inside it) and otherwise sets its header on a plane of its own, with no
  rule; a panel's frame is padding, so the rule under its title stops at
  the frame instead of crossing it. A text field's text is centred in its
  box again. `kui run --shot out.gif --record ms` records an animation
  headless. Preferences' ⌘S / Ctrl+S saves (a menu bar it never had), its
  toasts stack with an Undo each; the book's task list clears done tasks
  with a ghost button beside the primary Add.
  A third pass, on the keyboard and on focus: nothing changes size when
  it takes focus — Ledger's field underline keeps the region rule's height
  whatever its thickness (a focused field moved the rows under it) — and
  a text field's `error` sits at the end of its well, cut short with the
  whole message as its tooltip, so a field in a toolbar keeps its height
  and its buttons their line. Focus rings show only when the keyboard put
  focus there: a click on a list row, a table or a segment focuses without
  one. A checkbox and a switch draw their ring 2 px round the mark rather
  than round the whole row. A segmented control and a radio group are
  entered on the chosen item; a bar chart's bars and a waterfall's spans
  are one Tab stop the arrows walk (Enter picks; the bar the keyboard is
  on takes the hover colour and shows its tooltip) when the host picks
  with `on_select`, and with no `on_select` neither they nor a table's or
  a list's rows, nor sort headers with no `on_sort`, take the keyboard. A
  `text_area` lets Tab leave it, `tab_inserts` keeps Tab as indentation.
  A slider's whole track is grabbed (an open hand, a closed one while
  held) and a drag can start on the handle, which was a region of its own
  that took the press. A bar chart's value column is as wide as its widest
  label. A toast with no tone is `info` (the skin's focus blue; `Tone`
  gains `info`), and the last toast in a `toaster` slides out with it
  rather than vanishing. Phosphor's off switch is a dim track and a solid
  knob, not a hollow square in a lit frame. A live line chart's area is
  polygons rather than a path rasterized again on every tick (emit 3–4 ms
  a tick for 300 points; the line chart's own fill went from 1.7 to 0.4
  ms a frame in `cargo bench -p kui-lua --bench kit -- line_chart`, 100,000
  points from 1.75 s to 5 ms), and its labels and hover bands, the bar
  chart's, the waterfall's and the log's Latest button take their
  parent's clip instead of drawing over a scrolled page's top bar. The
  log keeps its end the frame its box changes height (Watchtower's events
  sat a band off it for a frame). Field Notes' New note puts the keyboard
  in the new title and its checklist folds. The Kit app shows all 39
  components, a Keyboard page and a toast stack, and runs as an app of its
  own (`kui.config.lua`: Kit, `dev.kui.kit.gallery`).
  A sixth: a table's or a list's ring and its selection are one cursor —
  a table's row was a control of its own, so a click focused the row and
  its ring stayed while the arrows moved the selection; rows are list
  items now, the click focuses the table, and while the keyboard drives
  it (Tab, or its keys since the last click) the ring is on the selected
  row. Every Kit control draws its ring in the skin's focus colour
  (Phosphor's green, Neutral's blue, Ledger's), its edge at 2 px, with the
  core's ring — the theme's, and the theme is the host's, which a Lua
  add-on cannot set — turned off. `text_field` takes `browse` (`folder`,
  `open` or `save`): a Choose… button after the field that opens the
  system's dialog and fills the field with the path picked; Tidewater's
  sync folder has one. A slider's handle sits on its value and a drag
  lands on the step nearest the pointer (the handle was laid out after
  the fill, half a handle and more past the value's place). An under-way
  progress bar crosses its whole track, however wide.
  A seventh, adopting what the core gained (ADR 0069, ADR 0070, C89–C99):
  `list` and `table` are core composites rather than key sinks — focusable
  rows under the `row_count` their body declares, so the arrows, Page keys
  and End step past the window, Tab enters on the selected row
  (`active_index`), focus landing on a row proposes it as the selection,
  and a search the built rows cannot answer comes to Kit as `on_typeahead`;
  `table` takes `cells = true` for a table of focusable cells with a row
  of column headers. Every Kit root sets `focus_ring = "$kitFocus"`, which
  replaces Kit's own per-control rings and the list's focus edge; the
  checkbox, switch and radio keep their tight mark ring. The slider's
  handle shares a hover group with the track and takes its look from
  `hovered` and `pressed` rules, so a hover no longer re-fills it. A
  browse field's Choose… goes under the field when its row is narrow,
  Tidewater's setting rows stack below 640 px of their own width,
  Watchtower's headline cards line up by `min_column` and its charts stack
  in a narrow pane (its minimum width is 880 now), and the Kit app's demo
  grid is `min_column`. Every node Kit eases carries a `reduced_motion`
  entry, and the under-way bar stands still under it. `text_area` takes
  `tab_moves_focus` in place of `tab_inserts`: Tab types by default, with
  Escape then Tab or Ctrl+M (Ctrl+Shift+M on macOS) as the ways out. The
  line chart draws from `ch:reduce` — a binary search for its window, one
  bucket for its range, per two px the first, least, greatest and last —
  and a value that is no number breaks its line (`line_chart_100k` 5.0 →
  2.6 ms). The Kit app shows a Conditions demo and a table of cells.
  An eighth: the line chart measures its axis labels
  (`env.measure_text`, kept by text) — x ticks come as close as two
  neighbours' half widths and 12 px allow, never under 50 px, an end
  label giving way to its neighbour unless its tick is the plot's edge;
  the y gutter hugs its widest label and eases when that changes; and a
  new value range glides in over 240 ms, jumping under reduced motion
  (`line_chart_live_300` 457 µs and `line_chart_100k` 2.5 ms, as
  before). Two components: `skeleton`, placeholder lines, a block or a
  circle in the skin's well, rounded by `$kitRadius`, Kit's shimmer
  fragment (`addons/kit/shaders/shimmer.wgsl`, which Kit registers
  itself) crossing every bar in step, still under reduced motion; and
  `shader`, a box a fragment paints — a preset (`shimmer`, `noise`, in the
  skin's colours), WGSL handed over as `source`, both registered by Kit,
  or a host's `id` — rounded by the skin, the well with none. `code` takes
  `copy` (a Copy button that says Copied), `line_numbers` and `label`.
  `settings` takes `appearance` (`system`, `light`, `dark`: Kit's colours
  held to one half, and its own `dark` / `light` rules with them), `hair`,
  `rule`, `font_ui` and `font_data`, and an accent given as a colour
  brings the skin's accent-painted roles along.
  The Kit app is a card a file under `app/demos/`, each with a Source
  button that shows its whole file read from the app's folder (and the
  app's modules it requires) with line numbers and a Copy button; a Theme
  panel sets every `settings` prop with every card following and prints
  the `kit.settings` call; a Loading page shows skeletons, a card that
  swaps them for its content, and a shader box; and nothing it says names
  an ADR. *What you can delete:* a brand's `on_accent`, `sel` and `focus`
  worked out by hand to go with its accent, and an app's own placeholder
  bars for what is loading.
  A ninth, adopting C104–C113: no Kit control reads its hover, press or
  focus in Lua to pick a look — Ledger's primary face moves by `dx` /
  `dy`, its shadow lengthened or dropped, from rules on the hover group
  it shares with the still box that takes the pointer, and a checkbox's, a switch's and a radio's ring shows
  on `group_focus_visible`; the under-way bar and the skeleton leave a
  loop to the core under reduced motion; a `select` opens on the press
  with its current option over the field, so a press, drag and release
  picks and a quick click leaves the menu open; ⌘C closes an open select;
  a leaving toast keeps its room until it has gone and the toaster
  glides; a type-ahead answer reaches a row past the window. The Kit app's
  shader box shows real shaders, its Theme panel reads Kit's tokens as
  they resolve, and its own `dark` / `light` rules follow the appearance
  the panel holds. The Kit app and the samples carry a `.luarc.json`
  pointing lua-language-server at the repository's declarations, and are
  clean under it.
  A tenth, from a review of the Kit app: the keyboard opens a select on
  its current option (Up, Down, Alt+Up/Down, F4, Space, Enter — a Mac's
  pop-up and browsers there; Windows and Linux step the value with a bare
  arrow, which a script cannot tell apart); an accent given as a colour
  derives the whole palette with every text at 4.5:1 on what it sits on
  (checked over three skins, both halves and four accents); a selected
  row keeps its chips; and the Theme panel offers only the faces installed
  on the machine, since Kit ships no font files and a family that is not
  installed draws as the system's sans.
  An eleventh, adopting C116 and C117: a `select` follows the platform's
  convention — on macOS Up and Down open it on its current option, on
  Windows and Linux a bare Up or Down steps the value to the previous or
  next enabled option (clamped, `on_change` as a pick) without opening,
  and Alt+Up, Alt+Down and F4 open it; Space and Enter open it
  everywhere. The Kit app's Theme panel lists every family the machine
  has (`env.system_font_families`), sorted, the skin's own faces first
  and macOS's private interface faces left out, with a filter and a
  count, a pick setting the UI or the data face; it names the skin's
  faces that are not installed, and a font file chosen with Load… (or
  typed, relative to the app's folder) is loaded with
  `env.load_font_file`, its family listed and the printed call saying to
  load it. The probe that declared a family token per candidate is gone.
  A twelfth, from the demo notes: Tab and Shift-Tab leave a select (its
  field, a key sink, hands focus on itself); a click on a cell of a table
  of cells selects its row; the slider's handle, a mark's focus ring and
  the skeleton's band stay inside the scroller they sit in (each a float
  that said no `clip`); every notice's and toast's text reads at 4.5:1
  on its own ground and its frame or tone bar at 3:1, in every skin,
  accent or none (the tones and the focus colour are made legible
  always, not only under an accent; Ledger's info notice read at 3.8:1);
  a `checklist` component, one Tab stop whose arrows walk its items, which
  Field Notes' checklist now is; a line chart's buckets sit on whole
  multiples of their row count, so a sliding window no longer buckets the
  line afresh each tick (Watchtower's charts, whose numbers now drift as
  damped walks about their level); and every chart reads out the value
  under the pointer — a line chart's crosshair, dots and readout of each
  series (from the keyboard too, with `focusable`), a sparkline's value
  and dot, a bar's hint — for one hover layer an idle chart pays
  (`line_chart_live_300` and `line_chart_100k` unchanged). A chart
  card's Source in the Kit app shows the data it draws.
  A thirteenth, from the demo notes again: Watchtower keeps its split
  down to 980 px wide and stacks the halves below that, in a page that
  scrolls (its minimum window is 520×420); a line chart's readout lands
  on the sample nearest the pointer by the x scale its line is drawn
  with, where a window the data filled only part of (a live line's first
  seconds) put the crosshair well off it, and its box keeps beside the
  crosshair and away from the samples it reads (the sparkline's bands
  name the sample nearest where its line draws it, too); a line chart's
  area has no seams where its polygons meet, each chunk ending on a whole
  pixel of the window (a single path was benched again: +0.6 ms a frame
  at 300 points, +7 ms at 100,000 rows); a slider springs to a new value,
  its handle riding the fill's end, but not on a resize or a drag; a
  table's columns take a `priority` and hide lowest first as it narrows,
  the header with them, and what still does not fit scrolls sideways (the
  Kit app's Data card hides Size, the id and Time in turn, and puts its
  Selected panel under the table when the card is narrow); a checklist's
  Delete and Backspace remove the focused item (C120's `on_item_key`);
  and the Kit app's pages change with ⌥↑ and ⌥↓ (Alt+Up and Alt+Down off
  a Mac), from a key sink around the app that a text field keeps them
  from. The Tasks tutorial's list is padded so its checkboxes' rings are
  not cut at the scroller's edge.
  A fourteenth, adopting C125–C127 and fixing C129: the checklist leaves
  a removed row's focus to the core (its record of each row's key gone;
  Tab straight onto the last row and Delete at once lands on the row
  before, which the record missed); the slider keeps `snappy` while it
  zeroes the fill's transition for a drag; a line chart's area chunks end
  on whole device pixels by `env.scale`, so its seams are gone at 1.5 as
  at 1 and 2; a hovered or pressed danger menu row's label and chord are
  `on_danger` on its `danger` fill, where they were `danger` and
  vanished, and a plain row's chord is `fg` on its hover; and a loud
  fill's hover and press shades lean toward its label only while the
  label still reads at 4.5:1 on the press, else away from it (Ledger's
  light danger button and row pressed to 3.7:1). Kit's contrast test
  draws every menu row tone, a danger button and a toast's action
  hovered and pressed.
  A fifteenth, from the user: Tab moves in the gallery again (its ⌥↑ /
  ⌥↓ shell is no Tab stop, hands Tab on, and takes the keyboard while
  nothing has it); a live line chart scrolls smoothly between samples,
  its crosshair held at a still pointer while the samples slide under it,
  and a frame between samples runs no view; a focusable line chart is a
  slider over its samples, so ⌥↓ pages from it; a danger menu row
  draws its label and chord once, recoloured by the row (C130); and the
  text roles read on every row Kit washes, hovered or pressed — a list's
  detail, a table's mono column, a log's time and level, a tab's and a
  segment's count, a checklist's done item, a menu's chords, a select's
  options — in every skin, half and accent (C131). Kit's contrast test
  hovers and presses each.

- **How kui reaches a machine** (backlog C88, [ADR
  0068](docs/adr/0068-how-kui-reaches-a-machine.md)). `kui pack` builds a
  runner app for any platform from any machine — the target's prebuilt
  runner with the app's archive added: a Windows `exe` (archive appended,
  subsystem set), a macOS `.app` (archive in `Resources`, `.icns`, signed
  ad hoc on a Mac), a Linux binary in a `.tar.gz`, an npm package of
  sources over `@qxuken/kui-cli`, or the `.kui` archive alone, which `kui
  run app.kui` runs; sources by default, `--bytecode` opt-in; `--installer
  <url>` writes `install.sh` / `install.ps1` for a company's own host.
  A packed app runs from its archive with every argument its own
  (`KUI_FLAGS` for the runner's), read-only to `kui.fs`, logging to its
  data folder. `kui upgrade [version]` and `kui runners fetch <version>
  --to <dir>` read the GitHub release (or a mirror with its layout). The
  manifest gains `id`, `version`, `kui` (the runner pinned, warned on
  outside the compatibility table) and `pack`; scripts read `kui.id`,
  `kui.app_version`, `kui.data_dir`, `kui.cache_dir` and `kui.packed`.
  The samples and the book's task list keep their files in
  `kui.data_dir`. The release carries the runner for each platform with
  `install.sh` / `install.ps1` and one `SHA256SUMS`; npm has it as
  `@qxuken/kui-cli` over `@qxuken/kui-cli-<platform>`, and the Node
  addon as `@qxuken/kui-<platform>` — an install downloads one platform,
  about 25 MB where it was 89. A macOS runner app gets a View menu with Toggle
  Developer Tools until it declares its own bar.

- **Contribution slots** (backlog C86, [ADR
  0067](docs/adr/0067-slots-fold-into-components.md)'s second step): a
  place a view declares by name where any number of extensions append
  their components — `ui.slot("app.toolbar", &props)`, Lua `slot(name,
  props)`, Node `<slot name props/>`, C `kui_slot`, Odin `slot`. An
  extension contributes by naming the slot and one of its components
  (`Extension::contributions`, Lua's `contributes`, C's
  `kui_ext_contributions`); they are placed in load order, then `order`,
  as siblings, each an instance keyed by the slot, its provider and its
  component, each with the slot's props. Its events arrive as the
  messages in those props, and every component event now says who raised
  it: `UiEvent::from`, Node's `ev.from`, Lua's `at.from`, C's
  `KuiEvent.from`. `inst.slot` (C `kui_instance_slot`) says where an
  instance was placed. An empty slot is silent.

- **Two chapters in the book**: [Extensions: components and
  slots](docs/book/src/extensions.md) — a Rust host placing a Lua plugin's
  component and declaring a toolbar slot two plugins add to, step 11
  (`cargo run -p kui-lua --example tutorial_11_extensions`, tested) — and
  [Apps in Lua: Kit and the runner](docs/book/src/runner.md), a task list
  on Kit with its headless test (`examples/lua/tutorial/tasks`, run by the
  runner's suite).

- **Kit samples** ([`addons/kit/samples`](addons/kit/samples)): four
  apps on the runner to look at Kit in something like real use — Relay,
  an HTTP client serving its own mock API; Watchtower, an ops dashboard
  over a simulated fleet, in Teal; Field Notes, notes and checklists in
  sqlite; Preferences, a settings window over TOML — each with a headless
  test the runner's suite runs and pictures in every skin
  (`addons/kit/samples/shots.nu` redraws them).

- **Kit in the npm package** (backlog C77): `import { Button, extension }
  from '@qxuken/kui/kit'`, then `runWindowed(app, { extensions: [extension]
  })` — the generated components, typed, and Kit's Lua shipped beside them;
  `install(ctx)` for a headless `Ctx`. Kit gained what a real app's
  migration asked for: a `notice` (an inline message with its actions),
  toggle buttons (`pressed`), a button's accessible `name` when it says more
  than its label, `tooltip`s, a `success` variant, counts and alerts on
  segments, `focus` requests on fields, `hide_label`, `max_lines` on code,
  a brand `radius`, a panel's `pad`, solid play and stop icons, and regular
  label weight in Neutral.

- `createApp` takes `extensions`, loaded before the first frame as a
  window's are.

- **A warning for controls inside a live region** (backlog C76):
  `control-inside-live`. A live region reads as one message and its subtree
  is presentational, so a button inside one cannot be reached by assistive
  technology; put `live` on the message alone.

- **Exported tokens** (backlog C74): an extension's token table marked
  exported (`Tokens::exported`, Lua's `export = true`) is read by every
  origin after its own table and the host's, so an app paints its own
  nodes in an add-on's palette — Kit exports its `kit…` names, and `bg =
  "$kitPanel"` follows the skin.

- `LuaExtension::from_source_with` / `from_file_with` run a setup on the
  script's interpreter before the script (how a host gives its scripts
  modules of its own), and `kui_lua::install_data` puts `channel {}` and
  `row_heights` in a state that draws nothing (a worker's).

- **Kit** (backlog C67, [ADR 0066](docs/adr/0066-an-addon-describes-its-components-once.md)),
  kui's UI kit for dense tool windows, in `addons/kit`: 36 components
  written once in Teal and placed from every binding — layout (`panel`,
  `toolbar`, `statusbar`, `split`, `tabs`, `section`, `divider`), inputs
  (`button`, `icon_button`, `text_field`, `search`, `select`, `checkbox`,
  `switch`, `radio_group`, `segmented`, `slider`), data over a channel
  (`table`, `list`, `log`) and beside it (`inspector`, `badge`, `stat`,
  `code`, `empty`, `progress`), overlays (`dialog`, `popover`, `menu`,
  `toast`), charts (`sparkline`, `line_chart`, `bar_chart`, `histogram`)
  and a trace `waterfall`. Three skins — Ledger (paper and ink, the
  default), Phosphor (a terminal) and Neutral (quiet chrome) — in light
  and dark, two densities, and brand roles by colour or `$token`, all
  through one `settings` component. Each binding places them through
  typed wrappers generated from Kit's `schema.lua`:
    - Rust: the `kui-kit` crate — `kit::Button::new("Run").variant(kit::Variant::Primary).on_press(Msg::Run).place(ui)`,
      events read with `kit::TableSelect::from_payload`.
    - Node: `import { Button, Table } from '.../kit.js'`, functions that
      are JSX components, with their `.d.ts`.
    - Lua: `require("kit.ui")` with LuaLS annotations, and Teal
      declarations.
    - C: `kit.h` — `KitButton b = {.label = KUI_STR("Run")}; kit_button(ui, &b);`.
    - Odin: the `kit` package — `kit.button(ui, {label = "Run"})`, event
      data through `kit.With_Segmented_Change`.

  A Kit table follows a selection through 100,000 rows in 1.1 ms a frame
  and the waterfall pans 18,000 spans in 1.3 ms; an unchanged instance is
  replayed for about 1 µs.

- **Typed wrappers for any add-on's components** (backlog C67,
  [ADR 0066](docs/adr/0066-an-addon-describes-its-components-once.md)).
  `cargo run -p kui-addon-schema -- gen <schema.lua> <out>` writes the
  wrappers above from an add-on's schema; `--check` fails on a stale one,
  and `nu scripts/gen.nu` runs it for every add-on under `addons/`.

- **A component inside a component of the same extension, through the
  host's children** (backlog C70): `Extension::reentry` gives the list a
  second handle onto a busy extension, so a host's Kit buttons inside a
  Kit toolbar are drawn and heard. A Lua script answers it by itself; a
  Rust or C extension that wants the same implements it.

- **A data channel** (backlog C66,
  [ADR 0065](docs/adr/0065-a-data-channel.md)). Hand a component a
  hundred thousand rows as one small value: a channel is named columns of
  one length — numbers, integers, booleans or strings — the core owns,
  read by row and column wherever its handle goes, never copied whole:
    - Node: `const rows = new Channel({ name: names, ms: Float64Array })`,
      then `props: { rows }`; `get(row, col)`, `column(name)`,
      `publish(columns)` for the next revision, `Channel.resolve(handle)`.
    - Lua: `channel { name = {...}, ms = {...} }`; a handle in props or
      params arrives as the channel — `#rows`, `rows:get(i, "ms")`,
      `rows:row(i)` — with rows 0-based.
    - C: `kui_channel_new`, `kui_channel_add_f64` / `_i64` / `_str`,
      `kui_channel_finish`, then `kui_channel_handle` for props;
      `kui_channel_resolve`, `kui_channel_get_f64` / `_str` to read. Odin
      gets the same as `channel_*`.
    - Rust: `kui_core::channel::Channel`.

  A new `onRange` row (`on_range`) reports the data rows in a node's view
  as `range: { first, last }` whenever they change, so a component can
  page a source it does not hold. A frame that draws forty of 100,000 rows
  from a channel costs 0.39 ms; converting the same rows into Lua tables
  took 103 ms.

  **What you can delete:** the revision-and-rows dance in params (`data =
  { rev, rows }` sent once), and a scroll offset turned into row numbers
  by hand to page data in.

- **Components an extension provides** (backlog C60,
  [ADR 0060](docs/adr/0060-components-an-extension-provides.md)). An
  extension lists components, and a host places them as often as it
  likes, each keyed by where it was placed:
    - Node: `<component name="kit/card" props={{ title, onClose: msg }}
      groups={{ footer: [...] }}>…children</component>`.
    - Lua: `component("kit/card", { title = …, on_close = msg }, { …children,
      footer = { … } })`; an extension provides them in a `components`
      table of `(env, props, inst)` functions, puts the host's children
      with `children(group)`, and raises an event with `emit(prop, fields)`
      from `on_event`.
    - C: `kui_component(ctx, name, props, children, user)` for a host;
      `kui_ext_components`, `kui_children` and `kui_emit` for a plugin.
      Odin gets `kui.component`, `kui.component_with`, `kui.children` and
      `kui.emit`.
    - Rust: `ui.component(name, &props)`, `ui.component_with(name, &props,
      |group, ui| …)`, and `Extension::components` / `component`.

  The host's children are built where the component asks, under the
  host's origin, so their events stay the host's. A component's events
  arrive as the messages the host passed in its `on*` props
  (`event: "component"`), whichever binding placed it. An instance whose
  props did not change is replayed without asking the extension;
  `fresh: true` asks every frame. `unknown-component` and
  `children-misplaced` say what went wrong.

  **What you can delete:** the slot-per-instance naming scheme, and the
  reply templates a host passed in params to get its own message back.

- **An `svg` element** (backlog V12,
  [ADR 0063](docs/adr/0063-an-svg-element.md)). Draw an SVG file — an
  icon, a logo, a legend glyph — in a box, in every binding:
    - Node: `<svg data={markup} color="$accent" label="Search"/>`, or
      `src="icons/search.svg"`.
    - Lua: `svg { src = "icons/search.svg", width = 16 }`, the path taken
      from the script's folder.
    - C: `kui_svg(ctx, label, data, src, KUI_SVG_AUTO, KUI_SVG_CONTAIN,
      color, &spec, on_click)`. Odin gets `kui.svg`.
    - Rust: `ui.svg(SvgSource::Data(ICON), SvgOpts::default(), spec)`.

  When every shape is one a `path` draws — an icon set's strokes and
  fills — it is drawn as paths, crisp at any size and recoloured live:
  `color` is what `currentColor` paints, the theme's ink by default.
  When one is not — a gradient, a mask, a filter, a picture — it is one
  image rasterized at the drawn size; `mode="paths"` or `"image"` forces
  a road. No size is the document's own; one axis keeps the aspect;
  `fit` is `contain`, `cover` or `fill`. A `src` redraws when the file
  changes. Rasters are shared, so a thousand identical icons hold one.
  It is read as an image (`label` it, or `role="none"`), a clickable one
  as a button. `svg-malformed`, `svg-unreadable`, `svg-approximated` and
  `svg-too-large` say what went wrong; a `svg` fuzz target covers the
  parser. The parser and rasterizer (usvg and resvg, about 1.1 MB of a
  release library) sit behind kui-core's default `svg` feature.

  **What you can delete:** the build step that turned icon files into
  `path` strings, and the colour-per-theme copies of each icon.

- **Design tokens from a DTCG file** (backlog T8,
  [ADR 0062](docs/adr/0062-design-tokens-loaded-from-a-dtcg-file.md)).
  Load a company's exported tokens straight into kui: a W3C Design Tokens
  2025.10 file, or a Resolver Module document that composes several.
  Colours get a light and a dark half from the resolver's light/dark
  modifier, which is found by its contexts whatever it is called.
  Dimensions and numbers become lengths, and font families become family
  tokens:
    - Node: `ctx.setTokens(loadTokens('brand.resolver.json', { prefix: 'brand' }).tokens)`.
      The result also lists each name's DTCG path and every token skipped.
    - Lua: `env.set_tokens(env.load_tokens("brand.resolver.json").tokens)`,
      the path taken from the script's folder.
    - C: `kui_tokens_load_dtcg(ctx, path, prefix, inputs)` adds the tokens
      on top of the origin's own, and `kui_tokens_dtcg_error` says why a
      load failed. Odin gets `tokens_load_dtcg`.
    - Rust: `kui_core::dtcg::from_path` and `Core::add_dtcg`.

  What it reads:
    - all fourteen colour spaces, clamped into sRGB with a warning;
    - aliases, and `$type` inherited down groups;
    - the legacy `"#rrggbb"` and `"12px"` spellings real exports carry.

  A shadow, a typography or a font weight is skipped with a `dtcg-skipped`
  warning naming the token. `kui_core::json` is the strict JSON reader
  underneath, and a `dtcg` fuzz target covers both.

  **What you can delete:** the hand-copied token table and the script that
  kept it in step with the design team's export.

- **A theme with a light half and a dark half** (backlog T7,
  [ADR 0061](docs/adr/0061-a-theme-with-a-light-half-and-a-dark-half.md)).
  Hand the core one palette for each appearance, and it picks the half
  the OS asks for at the start of every frame. The stock widgets, the
  focus ring, the scrollbars, the selection and the menus all follow
  light and dark with no listener:
    - Rust: `Core::set_theme_pair(light, dark)`, or
      `ThemeSource::Pair { light, dark }`.
    - Node: `setTheme({ light: { surface, accent, … }, dark: { … } })`.
      Each half is overrides on its own base, and an `accent` recomputes
      that half's shades. Mixing the two forms is refused by name.
    - C: `kui_theme_set_pair(ctx, &light, &dark)`. Start each half from
      the new `kui_theme_derive(appearance, accent, &out)`, the stock
      palette with no context. Odin gets `theme_set_pair` and
      `theme_derive`.

  An appearance nobody reported takes the dark half. The devtools' base
  override shows the app's own half for that base.

  **What you can delete:** the `onSystemSettingsChange` handler that set
  the theme again for the new appearance, and the second palette's
  bookkeeping beside it.

- **A `.lua` extension loads in every host** (backlog C59,
  [ADR 0059](docs/adr/0059-a-lua-extension-loads-in-every-host.md)).
  Every door that loads an extension now takes a `.lua` file, or a folder
  holding `init.lua`, as a Lua extension, and anything else as a C
  library: Node's `addExtension` and a window's `extensions`, C's
  `kui_ctx_add_extension`, Odin's `ctx_add_extension` and a script's own
  `env.add_extension`. One extension written in Lua (Kit, under
  [`addons/kit`](addons/kit), is the first) reaches every binding.
    - `kui_ffi::open_extension(path)` is the one loader they all call.
    - `kui_lua::is_lua_path` and `LuaExtension::from_path` tell a script
      from a library and load either shape.
    - A loaded file's chunk is named `@<path>`, so errors name the file.
      Its folder goes first on `package.path`, so a script made of modules
      `require`s the files beside it.
    - `Box<dyn Extension>` is an `Extension`.

  The Lua VM now ships in `libkui_ffi` and the Node addon. On a release
  aarch64-apple-darwin build that adds about 1.55 MB: 16.88 → 18.45 MB
  with the runner, 7.72 → 9.26 MB without.

  **What you can delete:**
    - a Rust shim whose only job was to host a Lua extension for a Node,
      C or Odin app;
    - a `package.path` line prepended to a script's source so it finds
      its own modules.

- **The Lua DSL described for Teal** (backlog F194).
  `kui_lua::teal_decls()` returns a `kui.d.tl` for
  [Teal](https://teal-language.org), the typed Lua dialect: saved where
  `tl` finds modules and named the global environment
  (`global_env_def = "kui"` in `tlconfig.lua`), it lets `tl check` type a
  view's tables — a misspelt prop, a boolean where a colour goes, a word
  no enum has, a misspelt `env` read, an event phase that does not exist —
  and `tl gen` write the plain Lua a `LuaExtension` loads, so nothing of
  Teal runs in kui and a project mixes typed and untyped scripts. It is
  `luals_meta()`'s file in Teal's words, from the same tables: every
  builder and prop, the event classes (an app's message is a record
  extending `kui.Event` and the `kui.With…` field its event fills, cast
  from `ev`), and `env` typed as `kui.Env` — its facts from
  `schema::ENV_FIELDS`, its palette, metrics and tokens, and every call in
  `schema::DOORS`' Lua column — since Teal will not index an untyped
  table. Both type files are committed under `packages/lua` (`kui.d.tl`,
  and `kui.lua` for lua-language-server) for a script's author to copy
  without writing Rust, and a test fails while either is stale.
  `nu scripts/gen.nu` runs every generator in the repository — those two,
  `npm run gen`, `odin.nu gen` and the book's Examples part — or, with
  `--check`, fails without writing if a rerun would change a file;
  `release-prep.nu` runs it. Tests check the file with Teal 0.24.8 itself:
  every value the parser reads for a prop, Teal takes; a typed view
  compiles and its Lua runs. One gap is Teal's: a table against a union of
  shapes (`width = { percent = 50 }`) is told apart at run time only, so a
  shape no member has gets past `tl check` and the parser refuses it as
  before.

  *What you can delete:* a hand-written `.d.tl` for the prelude, and a
  `kui.Env` record a Teal project kept in step with the crate docs.

- **A recording of what happened, in frames and between them** (backlog
  F199, [ADR 0058](docs/adr/0058-a-recording-is-a-timeline.md)). A
  core can record its window as a timeline in kui's own format,
  [`kui-trace`](crates/kui-trace): frames as intervals, with why each ran
  (`frame_cause`, and named causes — `wake: parser`,
  `request: caret blink`), who held it (`owed_by`), whether it drew
  anything new (`frame_unchanged`) and the driver's present; nested spans
  with their CPU time (unix) and the node the builder had open as they
  began; inputs as they arrive (a run of pointer moves as one record),
  events as they leave (kinds and keys, never payloads), frames asked
  for, and the loop's sleeps and wakes. The core times itself — `build`,
  `fill <namespace>` for each extension, `devtools`, `layout`, `emit`,
  texts shaped and glyphs rasterized — so a C, Odin or headless host has
  view and layout times with no `FrameSample` pushed; the runners add
  the loop, a `handler` span round each call into the app, and the
  present (`Core::push_frame_sample`, which also feeds the latency graph).
  An app adds its own: `span_begin` / `span_end` and `Ui::span`,
  `perf_count`, `perf_gauge`, `perf_mark`, `perf_measure` (work timed off
  the main thread), `request_frame_for(why)`, `note_wake(name)` and
  Rust's `Waker::named`; `perf_measuring()` says whether anyone listens.
  The doors are in every binding, and C and Odin get a waker of their own
  (`kui_waker` and its family) for a `kui_run` host whose data arrives on
  a thread. Nothing is recorded until something listens — the devtools'
  Perf tab recording or showing its live view, `KUI_PERF_TRACE`, or a
  `tracing` subscriber under `kui-core`'s new `tracing` feature, which
  enters every span as a `tracing` span for Tracy, Instruments
  (`tracing-oslog`) or Perfetto live: off, every door is one test of a
  flag. `KUI_PERF_TRACE=path` streams a session to `path`
  (`.kuitrace`), or writes Chrome's, Perfetto's, speedscope's or the
  Firefox Profiler's format at exit when the name says so;
  `Core::perf_trace()` hands back the bytes. `kui-trace` reads them
  with no kui: `kui-trace dump`, `stats`, `convert --to
  chrome|perfetto|speedscope|firefox`, and `diff A B --fail-over 10%`,
  which a CI job holds a scripted session's cost with.

  *What you can delete:* a hand-rolled frame log on `frame_cause` and
  `owed_by`, thread-local spans timed round an app's own systems and the
  tab that reads them, a named-wake ledger, and a per-frame log file — kawoosh's Frames and Perf tabs among them.

- **A Perf tab in the devtools** (backlog F198, F199). Live, the
  recent frames by phase — handler, build, devtools, layout, emit,
  render, vsync's wait — against the frame budget, and the spans that
  cost the most. The record button keeps every frame (the newest 1800);
  stopped, the tab reads the recording whole: frames over seconds, the
  work spread, how many went past the budget, drew nothing new or were
  part of a burn (30 frames or more, each within 100 ms, no input),
  what the gaps between frames held (handlers, asleep, busy elsewhere),
  frames counted by cause and by holder, the spans by name and by
  origin, counters and gauges; a timeline whose bars open into their
  frame, a burn banded under its bars. A frame opened says why it lagged
  — *late* (its input waited for the frame), *blocked* (the thread busy
  outside kui), *slow* (the phase and the span with the most self time),
  *waited* (wall time far over CPU time), *wasted*, *burn* or *fine* —
  with the path from its input to the screen leg by leg, its gap, its
  spans as a flame chart (a span with a node lights it in the app and
  selects it in the tree) and its causes, holders and counters. *save
  trace* writes the recording as a `.kuitrace`. While it records the tab
  draws nothing that moves, so a docked panel never makes a frame look
  changed. A camera saves a screenshot of the window and a film camera
  records it as a GIF until pressed again (`capture_to` / `record_to`),
  to `KUI_CAPTURE_DIR`, else the desktop, else the temporary directory,
  the path noted in the events tab; the panel's `capture` answers come
  back to the panel, never the app.

- **An app runs with no window** (backlog C58, [ADR
  0057](docs/adr/0057-an-app-runs-with-no-window.md)):
  `kui_native::app("x").open_headless(app, w, h, scale)` is a
  `HeadlessRunner` — the same `App`, on the real clock, its frames paced
  by its wakes and drawn for nobody, and kept as files when it asks
  (`capture_to`, `record_to`). `App::setup` gets a `Waker` that wakes it,
  so the app's threads work unchanged. A host turns it (`turn`,
  `turn_until`) or hands it the thread (`run`), and sends it input from
  any thread through a `HeadlessInput` (`send`, `resize`, `focus`,
  `set_clipboard`, `close`); everything sent before a turn is handled
  before that turn's frame. It needs no display — no winit runs — and a
  GPU only for a picture. It keeps a clipboard of its own and never
  writes the user's, drops sounds, answers a file dialog with no files,
  and reads only the `env.system` the launcher pinned, so a picture
  taken on one machine is the one another takes. Rust only.

  **What you can delete:** a second main loop beside `Launcher::run` for
  running the app headless — a `Drive` stepped on the wall clock, a
  condvar for the app's threads to ring, `Captures::serve` after each
  frame, a clipboard stand-in — and the `setup` split in two to give it
  a waker; and a hidden window opened only to be driven and
  photographed.

- **An edge-scroll feel, for kui's drags and an app's** (backlog F196,
  [ADR 0029's
  amendment](docs/adr/0029-a-selection-follows-the-pointer-past-the-edge.md#amended-2026-10-25-an-edge-zone-a-ramp-and-the-windows-edge)):
  a held drag in the edge zone inside a scroller — 60 px by default, a
  quarter of a short one — scrolls it at a speed from how deep the
  pointer is, along a quadratic curve (slow at the zone's inner edge,
  500 px/s at the scroller's), times a ramp while it is held there (to 6
  times over 1.5 s), within a cap (3000 px/s). A pointer pinned within
  2 px of the window's edge is all the way in, so a full-screen window
  scrolls as fast as any; past the scroller's edge in a smaller window
  the speed goes on rising. Easing back by more than 8 px slows it at
  once and starts the ramp again; leaving the zone stops it; a press
  that has not left its click slop never scrolls. `EdgeScrollFeel {
  zone_px, speed_px_s, curve, ramp_secs, ramp_max, cap_px_s,
  pull_back_px }` with presets `gentle`, `standard` (the default) and
  `brisk`: `Core::set_edge_scroll_feel` / `edge_scroll_feel` and
  `EdgeScrollFeel::preset`; C `kui_set_edge_scroll_feel`,
  `kui_set_edge_scroll_preset`, `kui_edge_scroll_feel`; Odin
  `set_edge_scroll_feel`, `set_edge_scroll_preset`, `edge_scroll_feel`;
  Node `setEdgeScrollFeel('brisk' | { zonePx, … })` and
  `edgeScrollFeel()`, and `edgeScrollFeel` to `runWindowed` and
  `createApp`; the launcher's `edge_scroll_feel`, copied to every
  window. `KUI_EDGE_SCROLL=brisk` (or up to seven numbers,
  `KUI_EDGE_SCROLL=72,600`) tries one without a rebuild and overrides
  the app's. An app holding its own drag calls `Ui::edge_scroll(key,
  pointer, rect)` once a frame — C `kui_edge_scroll`, Odin `edge_scroll`,
  Node `edgeScroll`, Lua `env.edge_scroll` — and gets back what moved; it
  asks for the next frame while it moves and keeps the ramp per
  container. The `edge_scroll` example has a slider for each number and
  a long text to drag-select through.

  **What you can delete:** an app's own autoscroll for a drag it holds
  — the band, the rate, the cap, the frame clock's delta, `shift_scroll`
  by hand and the `request_frame` to keep it going — for one
  `edge_scroll` call and placing the drag's end under the pointer after.

- **Kit's M0 spike** ([`addons/kit`](addons/kit), findings in
  [`M0.md`](addons/kit/M0.md); backlog C62–C65). Kit is a UI kit for kui,
  written once in Teal and loaded as a Lua extension. M0 measures it before
  the core changes it depends on are drafted as ADRs. It adds:
    - a toolbar of buttons, a virtual table and a trace waterfall, written
      in Teal against `packages/lua/kui.d.tl` and painted by the Ledger
      skin from `$kit…` tokens with light and dark halves;
    - `addons/kit/build.lua`, which type-checks and compiles them with the
      Teal compiler kui-lua vendors (`--check` fails on stale Lua);
    - a Rust host, `lua_kit_spike`, whose `--headless` check drives a
      button, a row and a bar in both appearances. Each reaches the app as
      its own `derive(Message)` value: the host passes its messages in the
      params, and Kit replies with a copy that has the event's fields
      merged in.
  
  Nothing in core changed. The waterfall's density levels bound its cost
  by the window's width, not the bar count. `cargo bench -p kui-lua
  --bench kit` holds the budgets, all met in Lua:
    - a table of 100,000 rows scrolling at 1.26 ms a frame;
    - 18,000 bars panning at 1.15 ms;
    - each button placement's slope at 0, 20 and 200, against kui's own
      button from Rust: 0.70 µs native, 1.03 µs as a replayed slot, 5.95 µs
      built in Lua, 29.9 µs as a fresh slot.
  
  The four core costs that profiling a replay found are filed for Kit's
  M2 as backlog C62–C65:
    - `begin_slot`'s duplicate check is O(n²) a frame;
    - a replay re-keeps its fill and clones its specs;
    - a replay compares the whole `Theme`;
    - a Lua fill's `env` costs 22 µs.

- **A state rule moves a node** (backlog C105): `dx` / `dy` move a node
  and its subtree off its laid place after layout — drawn, hit and read
  there on whole pixels, its room its place's, `onLayout` the place —
  eased by `transition` with `rotate` and `scale`, and a `when` entry may
  set either: `when = { { hovered = true, dy = -1 }, { pressed = true, dy
  = 1 } }` is a button face that lifts and sinks with no view code.
- **Conditions on a hover group** (backlog C106): `group_hovered`,
  `group_pressed`, `group_focused` and `group_focus_visible` (and the
  camel spellings; `Cond::GroupHovered` and the rest) test the node's
  hover group — a member under the pointer, pressed, holding focus, its
  focus shown — so a checkbox's mark rings while its row holds the
  keyboard, the two sharing a `hoverGroup`.
- **`essentialMotion`** (backlog C107): a node whose keyframe cycle
  carries meaning keeps it running under reduced motion; every other
  cycle holds still there (above).
- **A press that opens a menu hands its press on** (backlog C108):
  `opensOnPress` makes a node's `onClick` arrive on the press, and the
  release — over a `menuItem` of the menu that click put up — clicks the
  item, as NSPopUpButton's press, drag and release picks an option; a
  quick click leaves the menu open for a second click, and a drag
  released elsewhere dismisses it. Every binding (C `opens_on_press`);
  the stock `select` opens this way.
- **An accelerator closes an open menu float** (backlog C109): a key
  under ⌘ (Ctrl off a Mac) pressed while the app's own `role: "menu"`
  modal is open — Kit's select, a popover of rows — dismisses it with
  `reason: "accelerator"` and is then the window's, focus back where the
  float took it from, so ⌘C copies from the field under a dropdown, as
  the core's own menus let a chord through (C98). A row of the platform's
  menu bar does the same. An editor inside the float keeps its chords.
- **An exit that keeps its room** (backlog C110): `keepRoom` on an `exit`
  (`keep_room`; `Enter::keep_room`; C `KUI_ENTER_KEEP_ROOM`) keeps the
  departing node's room in the layout while it plays out, so a toast
  leaving a stack goes before the rest close up behind it, gliding where
  they `slide`.
- **A script registers its own fragment** (backlog C111):
  `env.add_fragment(wgsl)` returns the id for `fragment { id = }`, nil
  with `fragment-rejected` for source that does not compile; idempotent
  by source, so a view, a component's function or a `kui run` app calls
  it every frame. Kit's `shader` component no longer needs a host's
  handle.
- **An origin sets the appearance its conditions read** (backlog C113):
  `set_rules_appearance` (Rust `Core` / `Ui`, Lua `env`, Node
  `setRulesAppearance`, C `kui_set_rules_appearance`, Odin) has the
  caller's `dark` / `light` conditions read `light` or `dark` in place of
  the palette's, until set again; Kit holding its look light or dark has
  its `when` entries agree with its colours.
- **A script can tell which platform it runs on** (backlog C116):
  `env.platform` — `"macos"`, `"windows"` or `"linux"` — in Lua and in
  Node's `env()`, `kui_platform` (a `KUI_PLATFORM_*`) in C, `platform` in
  Odin, `Ui::platform` / `Core::platform` in Rust: the fact a `when`
  entry's `platform` condition reads, so a script following a platform's
  convention and a rule following it agree. `Core::set_platform` lets a
  Rust test say another, which both follow.
- **A script lists the system's fonts and loads a font file** (backlog
  C117): `env.system_font_families()` lists every family the core can
  see, installed or loaded, sorted (C `kui_font_families`, Node
  `systemFontFamilies`, as before); `env.load_font_file(path)` registers a
  TTF, OTF or TTC and answers its family — drawn by that name from then
  on, in `family = ` or a family token's list — or nil and why. A
  relative path is the script's folder's (in a `kui run` app and in Kit,
  the app's folder; a packed app's, its extracted folder), and the same
  file twice is loaded once, so a view may call it every frame.
  `kui_lua::set_script_dir` gives a script loaded from source its folder.
- **Docs that travel with kui** (backlog C118, [ADR
  0071](docs/adr/0071-docs-that-travel-with-kui.md)). The `kui` runner
  carries props.md, howto.md, Kit's component reference, the book's
  chapters as text and the ADRs, for its own version (about 0.7 MB of
  the binary): `kui docs <query>` prints the props.md rows, howto
  questions and Kit components that match — `kui docs hover_bg`,
  `kui docs kit.table`, `kui docs animate a removal` — best first;
  `kui docs --adr 64` prints an ADR; `kui docs` alone opens the book.
  `kui new` and `kui types` take `--docs`, which writes them into
  `.kui/docs`, and `--agents`, which also writes a short block for
  coding agents — the version, where the types and docs are, the
  commands, a few house rules — into `AGENTS.md`, or between
  `<!-- kui:begin` / `<!-- kui:end -->` markers at the end of an
  `AGENTS.md` or `CLAUDE.md` the app has; kui rewrites only what is
  between them. Without the flags a project gets what it got before.
  The choice is kept in the manifest as `tooling = { docs = true,
  agents = true }`. `.kui/version` names the runner that wrote `.kui/`,
  and `kui run` and `kui check` write `.kui/types` — and the docs and
  the block, where the app opted in — again when it differs, saying so
  in one line, so `kui upgrade` no longer leaves a project's types
  behind. Kit's component reference is generated from its schema by
  `kui-addon-schema` (`addons/kit/bindings/reference.md`); the
  `kui-native` crate has props.md and howto.md as rustdoc pages,
  `kui_native::docs::props` and `::howto`, with an index of the ADRs
  (`::adr`); and `@qxuken/kui` adds the book's text (`docs/book/`) and
  Kit's reference (`kit/reference.md`).
- **A composite hears the keys its items leave** (backlog C120):
  `onItemKey` on a `list`, a `table`, a `menu`, a `radioGroup` or a
  `tabList` (Rust `NodeSpec::on_item_key`, Lua `on_item_key`, Node
  `onItemKey`, C `KuiSpec.on_item_key`, Odin `on_item_key`) hears the
  keys its focused item does not claim — Delete and Backspace on a
  checklist's row, Enter on an item with nothing to press, a function
  key — as an `itemkey` event: the press as a sink hears it, with `item`
  the focused item's key, `index` its data `index` (else its place among
  the items) and, in a table, `column`. The arrows, Home / End, the
  pages, Space and type-ahead stay the composite's, Tab and Escape the
  ring's, and a chord still goes to the sink above. The container stays
  one Tab stop and its items keep their arrows, where an `onKey` on
  either took them; a control inside an item reports the item.
- **Focus stays in a composite whose focused item goes** (backlog C125):
  the frame that no longer has a list's, a table's or another
  composite's focused item lands focus on the item now at its place,
  else the one before it, else the first — a table in the column kept —
  where focus was left on a key no node had. A windowed list keys its
  rows by data index, so only a row past the new end moves focus (to the
  last row, reached for when unbuilt), and a row scrolled out of the
  window keeps it. The focus shows as it did. A composite gone or empty
  leaves focus where it was. And `focus_item(key, index)` names an item
  by its data `index` (else its place), landing when the frame ends and
  reaching for a row a windowed list did not build: `Ui::focus_item`,
  `kui_focus_item`, Odin `focus_item`, Node `focusItem`, Lua
  `env.focus_item`.

  *What you can delete:* a checklist's record of each row's key, kept to
  move focus to the row before when the last one went.
- **A box colours the texts under it** (backlog C130, ADR 0069):
  `textColor` (`text_color`) is the colour of every text under the node
  that names none, inherited as CSS's `color` is — the nearest
  declaration wins, else the theme's foreground — and a `when` entry may
  set it, so a row recolours its label on hover or press once, on the
  row, with no view re-run: `textColor: '$danger', when: [{ hovered:
  true, textColor: '$onDanger' }]`. Every binding: the schema row, C's
  `KuiSpec.text_color`, Odin, Node. Kit's danger menu rows draw one label
  and one chord with it, where each was drawn twice.
- **Selecting rows in a Kit list or table, by kind of interaction**
  (from the user's notes). `selection = "single" | "multiple" | "none"`;
  a multiple selection is the host's `selected_ranges` (`{ first, last }`
  each), so ⌘A on a windowed list of 100,000 rows is one range;
  `select_on_focus = false` keeps the arrows off the selection — they
  move the cursor, Space selects, Enter activates, a click selects — so a
  detail panel loads on `on_select` or `on_activate` and not for every
  row walked past. Three events: `on_cursor { row }` (the keyboard's row
  moved), `on_select` (`{ row }`, or `{ row, ranges, count, rows? }`),
  `on_activate { row }` (Enter, a double click). In a multiple selection
  ⌘-click (Ctrl-click) toggles, Shift-click and Shift+arrows extend from
  the anchor Kit keeps, Space toggles and ⌘A selects all; Escape leaves
  the selection, as Finder's and Explorer's lists do. The gallery's Data
  table is multiple and moves no selection with the arrows, its detail
  loading on a selection or an open, with a line saying the cursor, the
  selection and what was opened. The core gives the facts it needed
  (C101, in part): a click's `mods`, `clicks` and `key`, a focus move's
  `mods`, select all to `onItemKey`. Filed: C139, a list's
  `multiselectable` for assistive technology.
- **Kit's design system in the Kit app.** The sidebar's Design system
  section draws Kit describing itself: **Foundations** (the brand book),
  **Colours** (every colour role of the theme the Theme panel sets, live
  from `env.tokens`: swatch, value, usage note, and a text role's
  contrast on each ground its note names), **Type and spacing** (the type
  styles, the spacing and hairline scales to size, radius, shadow) and
  **Icons**; ⌥↑ / ⌥↓ walk them with the rest. Each card's **Guide**
  button, beside Source, draws the guidelines of the components it
  places. They are drawn by a new component, `markdown { text }` — `#` to
  `####` headings, paragraphs, nested `-` / `*` / `1.` lists, quotes,
  rules, fenced code as `code`, pipe tables, inline `code`, bold, italic
  and links as their text — in every binding. `tokens.json`'s values are
  Kit's own, written by `kui run --headless addons/kit/app/design/tokens.lua
  -- --write`; the runner tests check that file against Kit (`--check`),
  a guideline for every component and none for a missing one, and the
  brand book's icon list against `src/icons.tl`.
- **Where the pointer is over a node** (backlog C119): `pointer_in(key)`
  — `{x, y, w, h}`, the pointer's place from the node's top-left
  (through any turn) and the node's size, as of the last frame — for a
  node with a hit region or `onLayout`; nothing while the pointer is out
  of the window. `Ui::pointer_in`, `kui_pointer_in` (a `KuiLayoutRect`),
  Odin `pointer_in`, Node `pointerIn`, Lua `env.pointer_in`; a kept fill
  that read it runs again when it changes. What a chart's crosshair at
  the pointer's x reads every frame the pointer moves.
- **The display scale in Lua** (backlog C127): `env.scale`, device pixels
  per logical px of the window being built, what a script lands an edge
  on a whole physical pixel with; `Ui::scale`, `kui_scale` and Odin's
  `scale` read it for a plugin's view (Node's `env().viewport.scale`
  already did).

### Changed

- **CI is GitHub's alone, and the crates and npm packages publish to
  crates.io and npmjs only.** The full check (fmt, clippy, the workspace
  tests on lavapipe, the C round, the Odin binding, the Node parity tests,
  the scene corpus, the book, the headless round) runs on every push to
  main and every pull request again, as `.github/workflows/ci.yml`, and a
  release tag runs it as its first job, where it had been fmt, clippy and
  the tests. `smoke.yml` runs the tests, the windowed round and the C
  round natively on macOS and Windows on every push to main, non-blocking;
  `audit.yml` checks yanked crates again; `fuzz.yml` is by hand as before.
  The book is on GitHub Pages at the same address. The repository lives on
  GitHub alone; the Forgejo it grew up on is retired, and its cargo and npm
  registries keep every version up to 0.1.0-alpha.67 and take no new ones
  (`scripts/release-local.nu`, which published to them, is gone).

- **The `kui` runner allocates with mimalloc** (backlog C150), the Lua
  heaps too — mlua allocates through Rust's allocator — and gives freed
  pages back to the OS at once (mimalloc's `purge_delay` 0;
  `MIMALLOC_PURGE_DELAY` in the environment still wins). Headless, against
  the system allocator: the Kit gallery after touring every page rests at
  69 MB on Overview, not 84, and at 66 MB on Charts with Live on, not 103
  (the system zone there was 27% empty); Watchtower 44 MB, not 51; five
  minutes of touring the pages round and round 73 MB, not 92. A small app
  pays mimalloc's per-thread pages: the tutorial's Tasks 16 MB, not 13.
  Frame times in the runner are unchanged within the runs' spread; Kit's
  bench runs 12% faster built with it. `vmmap` and `footprint` list
  mimalloc's heap as "IOAccelerator" (the VM tag it uses on macOS), not as
  malloc zones. Only the runner binary has it: the library crates, the Node
  addon and kui-ffi keep the platform's allocator, and
  `--no-default-features` builds the runner without it.

- **A debug build is about a quarter the size on disk** (backlog C72):
  the workspace's dev profile keeps file and line (`debug =
  "line-tables-only"`) and dependencies none. One worktree's `target/`
  had reached 85 GB in a day — on macOS every incremental rebuild leaves
  its objects behind for the debugger. `CARGO_PROFILE_DEV_DEBUG=full`
  brings full debug info back for a session.

- **Extension fills are much cheaper** (backlog C62–C65). Measured on
  M0's bench, a frame of 200 Kit buttons, each per button:
    - a replayed slot whose host keeps its params: 1.03 → 0.85 µs, against
      0.68 µs for kui's stock button from Rust;
    - a component, its props built every frame: 1.08 → 1.03 µs;
    - a fresh Lua fill: 29.9 → 7.1 µs, and a frame with one fill that
      draws nothing 22.5 → 2.7 µs.

  A Lua script's `env` is built once and shared by its fills, its facts
  written when they change; a slot's duplicate check is a hash lookup; and
  a replay compares the palette and the sizes by generation.

  **What you can delete:** a host's cache of which panes to replay by
  hand to keep a frame of many small extension fills under budget.

- **The devtools light the app as a browser's do** (backlog F197). The
  tree tab highlights the node under a hovered row, a breadcrumb or the
  inspector's parent link — and the one under the picker — with its box
  model over the app (the content blue, the padding green, the border
  yellow) and a badge with its kind, name and size. A selection only
  opens the inspector and leaves the app as it is while its values are
  read; the eye beside the picker keeps it outlined. The inspector opens
  on the node's title, size and a box-model diagram, and lists only the
  values the node has. The rows draw a guide down each level, chevrons
  for the disclosures, and flags as chips; a text node's words read as a
  string. In a bottom dock the inspector sits beside the list.
- **The devtools panel's frame** (backlog F197). The tabs and the
  placement buttons share one strip, the open tab underlined, where a
  header above the tabs held the window's title and the frame counter
  and a narrow dock cut the title to "kawoosh — k…": both are rows of
  the Facts tab now, whose status block is in headed sections with long
  values cut by an ellipsis. Filters name themselves by a placeholder;
  the events tab's rows colour what an entry is apart from where it came
  from and what it carried.

  *What you can delete:* nothing — a look, not a door.
- **The devtools tree by keyboard, the whole spec, and a range of a
  recording** (backlog F201). The tree's arrows select as they walk, the
  cursor's node lit in the app while the list has the keyboard, and a
  row clicked hands the keyboard to the list. The inspector reads the
  selected node's whole spec — its interaction, accessibility, motion,
  layout, paint and text settings — and shows the ones it sets until
  **changed only** is switched to **all fields**. The Perf tab's bars
  share the chart's width. Drag across a recording's bars, or click one
  and shift-click another, and the summary and the tables read those
  frames alone; **whole recording** goes back.

  *What you can delete:* nothing — a look, not a door.
- **The devtools' strip keeps to one line** (backlog F206). The
  placements moved into a "⋮" menu beside the close, as a browser's
  devtools keep their dock side: a row of the four, the close with its
  chord, and every tab when the strip cannot show them all; a press
  outside or Escape closes it. The tabs no longer wrap onto more lines
  when an app or an extension adds many: the strip scrolls sideways, a
  mouse wheel over it too, and brings the tab on show into view. The
  panel's tooltips are a few words, where the longest covered the pane.
  A drag-select pressed in a box that scrolls sideways inside its scope
  is followed from that box, so it scrolls it and hands the scope the
  rest. The examples' harness declares the platform's menu bar with a
  **View ▸ Show / Hide Devtools** row, the way back to a panel closed
  with its ×.

  *What you can delete:* nothing — a look, not a door.
- **A held drag hands on** (backlog F204, [ADR 0029's
  amendment](docs/adr/0029-a-selection-follows-the-pointer-past-the-edge.md#amended-2026-10-25-a-held-drag-hands-on)).
  A drag held at an edge scrolled only the scroller its press was in:
  selecting from a code block that scrolls sideways down a page could
  not scroll the page, and a short log stopped at its end with the page
  still. Now, on each axis, a scroller that cannot move — it does not
  scroll that way, it is at its end, the pointer is clear of its zones —
  hands that axis to the scroller around it, measured by that one's
  own edges, as the wheel chains; `overscroll: contain` keeps it in.
  `Ui::edge_scroll(key, …)` hands on the same way from `key`, so a board
  that scrolls sideways inside a page needs one call.

  *What you can delete:* a second `edge_scroll` call for the page around
  the container a drag scrolls.
- **A mouse wheel reaches what scrolls sideways** (backlog F202,
  [ADR 0050's amendment](docs/adr/0050-a-wheel-notch-glides.md#amended-2026-10-25-a-wheel-reaches-what-scrolls-sideways)).
  On Windows and Linux a mouse could not scroll a row that moves only
  sideways — the devtools' Perf chart among them — since a wheel turns
  only down and up. A plain notch over such a row now moves it, down
  toward its end, and at its end the next turn goes on to the page
  around it. Shift turns any notch sideways, as a browser's wheel does;
  macOS did both of these already for Shift, and still does. A handler
  whose `scroll_mods` names Shift hears the turn as it came, and a
  trackpad's swipe goes the way it is swiped.

  *What you can delete:* a Shift+wheel swap an app did in its own
  `on_scroll`, and a horizontal list's handler that turned `dy` into its
  own offset.

### Fixed

- **The `kui` runner spells an app's folder plainly on Windows.**
  `kui.dir` was the verbatim path `canonicalize` answers
  (`\\?\C:\app`), not the `C:\app` that `kui.cwd` and the user see, and
  a verbatim path takes no `/` as a separator: `kui check` built Teal's
  module path as `<dir>/?.lua`, so a project's own modules were "module
  not found". A packed app's folder and the runner's own path were the
  same. The prefix goes wherever the path has a plain spelling (a drive
  or a share), and `kui.cwd` is spelled the same way, so a short name
  (`RUNNER~1`) that `current_dir` keeps no longer makes the two differ.
  Found by the first run of the workspace tests on a Windows runner.

- **Kit's live line chart stays in its plot** (backlog C153). Its
  sliding layer — the lines, their areas, the x ticks — was a float
  anchored to the plot without `clip`, so it escaped every clip: the
  line, its y range gliding 240 ms behind the data, drew above the plot
  over the card's head and the window's bar, and the sample a slide
  reaches back for drew left of the plot into the y gutter, the leftmost
  bucket jumping there as the window slid; the x labels escaped while
  sliding. Under ADR 0074 the plot's clip holds all of it, the hover
  layer, the crosshair and its dots too, and the leftmost samples come in
  from under the plot's left edge. The readout box, beside the plot in
  the tree and so not cut by it, is placed inside the plot's width and
  height.
- **A Kit toast leaves in one piece** (backlog C154). It slid 32 px and
  faded, and what lay under it showed through at its edges as it went: in
  the gallery a card's 2 px border lies exactly under a toast's right
  edge (the page pads its cards 16 px in, the toaster sits 16 px in), and
  read as the toast's own edge left behind until the toast below slid up
  over it. The core moved every quad of it together all along — its
  hard shadow, face, tone bar, text and the dismiss button under the
  pointer, on the frames the view builds and on those built again from
  the kept tree (`tests/toast_exit.rs`); the toast now slides out past
  the window's edge opaque, covering its old place until it has gone
  past it. Ledger's hard shadow and Phosphor's glow go with it.
- **A Kit toast that goes by itself keeps its close button in place**
  (backlog C155). On the frame its time ran out the toast gained the node
  whose first layout raises its dismiss, in its row's flow, where it took
  a 10 px gap: the × moved left on the frame the exit holds, then slid
  with the toast. The node is a float now and takes no room. Kit's test
  and `tests/toast_exit.rs` check the × against the toast's face on every
  frame from rest through the exit, gone by its duration and dismissed by
  a click on it.
- **A Kit field's error reads whole** (backlog C156). A `text_field`'s
  message sat at the end of its well, cut short past 60% of it: the
  gallery's version field read "⚠ Three numbers: 1…" on the value's line.
  It shows whole now: beside the text while the well has room for all of
  it (measured with `env.measure_text`), under the field and wrapping to
  as many lines as it takes while it has not, the row growing to fit.
  The choice is the field's own-size rule (ADR 0069), so a resize moves
  it in layout without a view run; a toolbar with room keeps its height.
  The mark on a wrapped message sits on its first line, in a text area's
  too. Kit's test lays out a long message in a field, a field with a
  Choose… button, a text area and a field in a settings row that stacks
  below 640 px, at 150 to 900 px, in Ledger and Neutral: every glyph of it
  drawn, inside the field's column, what follows under it.
- **A Kit live line chart's area leaves the plot with its line** (backlog
  C157). As samples left at the left, the area went before the line and
  jumped back with each sample. The rows drawn started at the first one
  at or past the window's edge (or the slide's), found by comparing
  floats, and x accumulated by 0.1 — the gallery's feed — puts the row
  meant for the edge a hair before it: the search landed a row late, the
  slide uncovered a sample's width of nothing at the plot's left edge,
  and the line's round cap stood past its area's end there. What is
  drawn starts a row left of everything the plot shows, so the line and
  the area both cross its edges and its clip cuts them together (ADR
  0074). Recorded through kui-wgpu at 1.5, the gallery's line and area
  now start at the plot's edge on every frame, where they started up to
  four device pixels in. Kit's test runs the gallery's feed through
  several samples, frames built by the view and by the core, at 1 and
  1.5: the line and its area, both cut by the plot's clip, cover the
  same span on every frame, the plot's whole width.
- **A Kit waterfall reads on a dark skin.** Its spans were navy and
  forest blocks on near-black (1.3:1), the selected one a bright block
  that drowned them. Every group's bar now reads at 3:1 on every surface
  in every skin, half and accent, neighbouring phases apart in lightness
  as well as hue, every label at 4.5:1 on its bar; the selected span's
  edge reads at 3:1 and a time mark too. Kit's contrast test covers the
  waterfall's marks.
- **A Kit live line chart's readout is opaque again.** Each sample's
  slide opens a float of its own, and floats stack in the order they
  opened (ADR 0053), so the lines slid over a readout box opened before
  them and showed through it, as if it were clear — on a dark skin most of
  all. The plot and the readout paint in tree order (`floats_in_order`),
  the box over every line, and it carries the skin's shadow, as a raised
  float does. Kit's test draws it in every skin and half over a sliding
  window: an opaque fill, no segment over it, its text at 4.5:1.
- **A channel made or published from Lua costs less** (backlog C143):
  each column is built straight from the Lua values, where every cell
  went through a `Value` — a `String` allocated per string cell — first.
  The Kit gallery's log after an hour, 9,000 lines republished every
  400 ms, publishes in 1.62 ms where it took 2.31.
- **A runner timer that dispatches draws one frame a tick, not two**
  (backlog C141). The callback runs in the drain its wake started, and
  its `kui.dispatch` rang the loop again although that drain delivers it:
  a second frame with nothing new in it. A poke while the runner drains
  now rings nothing. The Kit gallery's Overview went from 23 views a
  second to 12, its frame work a fifth less.
- **A covered or minimized window renders nothing** (backlog C142). On
  macOS and X11 the redraws a wake asks for still reach a window the
  platform says is covered, and each was rendered — the glyph atlas
  synced, the frame's instances written — before its surface refused it.
  Such a frame is now built as one the window cannot draw (F169): the
  view runs and what it declares applies, nothing is rendered or
  submitted, and being uncovered draws.
- **A frame only an animation asks for no longer runs the view**
  (backlog C138). Every frame a keyframe cycle or a transition owed ran
  the host's whole view again at the display's rate — the Kit gallery's
  Charts page, whose chart slides between samples, ran the app's `view`
  57 times a second where its data changed ten. While an animation of
  the core's own runs, a build now keeps the spec each node whose look
  the frame decides declared (a `transition`, `keyframes`, a `when` list)
  and every fact of the frame it read; a frame that only that animation
  asks for — no input, no wake, no resize — while every fact reads the
  same and the app says its state held is built from the last tree
  (`Core::reframe`): those nodes go through the frame's pipeline again,
  so the cycles and transitions are sampled at its clock, and it is laid
  out and emitted as any. An app says what its view reads beyond the
  core with `App::view_state`, an extension its root's with
  `Extension::root_state`; both default to `None`, which runs the view
  every frame as before. The `kui` runner says it for every app — its
  revision moves with each wake, event and drain that did something
  (`kui_lua::keep_root`, `root_moved`) — so the Charts page, headless,
  went from 57 views and 10.7% of a core a second to 26 and 8.2%. `cargo
  bench -p kui-lua --bench kit -- line_chart_live_window` has a
  `reframed` row: 14.5 µs against 22.5 for the replayed fill.
- **A keyframe cycle out of view stops drawing the window** (backlog
  C137). A cycle is sampled as its node is built, wherever it lands, so a
  spinner or a pulse scrolled out of its scroller, or clipped away, kept
  the window drawing at the display's rate. A running cycle now owes a
  frame only while the frame drew its node, or something under it,
  inside its clip and the window; it is sampled again, at the clock, on
  the frame that brings it back. A cycle that moves its node (`dx`, `dy`,
  a turn, a scale) is measured over the reach of its stops, and one of a
  width or a height always runs, since it moves what is laid out beside
  it.

  *What you can delete:* a view's own check of a cycling node's rect
  against the viewport, as Kit's live line chart makes before it slides.
- **A click on a control inside an app shell focuses the control**
  (backlog C135). A press anywhere inside a key sink gave the sink the
  keyboard, so a clicked button stayed unfocused, Space and the arrows
  went to the shell, and the shell had to hand Tab on itself. Since keys
  bubble from a focused control to the sink around it (ADR 0011) the rule
  bought nothing: a press on a Tab stop now focuses it, inside a sink or
  not, and the shell still hears every key the control leaves. A press
  on what is no stop — a label, dead space — gives the sink the keyboard
  as before. `examples/rust/apps/splitmux.rs`'s panes say `keepFocus`, so
  a terminal drawn in one would keep Space.
- **A page changed under the focused control leaves the keyboard
  somewhere** (backlog C136). Focus stayed on the vanished node, under no
  sink, and every key went nowhere until a click — the Kit gallery's
  shell took the keyboard back by hand after its page shortcut. Now,
  when nothing recovers it (a composite's item, C125; a modal's restore;
  a region's), it moves to the nearest key sink it had around it that
  the frame still has, and is dropped when there is none, so a
  `keyFocus` shell takes it. A focus on a node never built is left alone,
  and so is one a windowed list holds on a row scrolled out (ADR 0070).
- **The Kit gallery idles when no page in view is live.** Its chart and
  log feeds ticked behind every page, ~12 views a second, and its log
  grew without end — 9,000 lines after an hour, republished whole every
  400 ms, most of what the app did. A feed's timer now runs only while
  the page in view reads it (`demos/feeds.lua`, `pace`): the chart's on
  Charts, the log's on Data, and on Overview, which only counts the
  samples, once a second ten samples at a time; the log keeps its last
  500 lines. Headless release, Overview: 2.70% of a core and 12 views a
  second before, 0.30% and 1.1 after. The worker filters and sorts the
  100,000 rows by reading the channel it built, a column at a time,
  where it kept a Lua copy for good: its Lua heap holds 0.2 MB where it
  held 10.2, and a view takes 120 ms where it took 80 (a channel has no
  filter or sort of its own). The shell's `paged` / `refocus` dance
  after ⌥↑ / ⌥↓ is gone: the core hands the keyboard back to the sink
  (C136). Kit's live line chart no longer checks its plot against the
  viewport before it slides (C137). Watchtower publishes each service's
  trend and the table's rows as channels when they move, where it made a
  new channel every frame: a frame no tick asked for (a hover, a click
  elsewhere) costs 2.6 ms where it cost 4.9.
- **Tab moves focus in the Kit gallery again.** The key sink the gallery
  wrapped itself in for ⌥↑ / ⌥↓ was a Tab stop — the first — and a sink
  holding focus keeps every key, Tab included, so the first Tab put
  focus on the whole app and no Tab after it moved anything; a press
  anywhere in the app gave the sink the keyboard too, so after hovering a
  chart and clicking, Tab was dead as well. A sink around Tab stops is
  no longer one (ADR 0011, decision 11), the gallery's hands Tab on when
  it holds focus, from where the press was, and takes the keyboard
  whenever nothing has it, so ⌥↑ / ⌥↓ work from launch. The runner
  gains `kui.input` — a key, the pointer, a click, typed text sent into a
  headless run — and the gallery's walks press real keys: through the
  Inputs page, over the live chart, after a press on it, and ⌥↓ / ⌥↑.
- An app names its own nodes inside a component (backlog C134): a node
  the app's view declared as a Kit panel's children answered no label —
  `layout_of`, `edit_text`, `set_edit_text`, `set_scroll`, `reveal` and
  the rest by label found nothing — because host children were filed
  under the component's placement, a scope the app's view never asks in.
  They are the placer's now.
- A composite row `focus_item` names, or focus recovers to when its row
  goes (C125), that is disabled — or a table row with no live cell —
  hands focus to the nearest built row that takes it, the row after on a
  tie, as an arrow steps past it, where focus stayed where it was
  (backlog C132).
- A box with a `tooltip` drawn from a kept fill (ADR 0045) and shown by
  keyboard focus drew its hint under its first child's key — Kit's
  button, its icon first, Tabbed to in the gallery: two nodes under one
  key, `duplicate-key`. The hint has a key of its own.
- A zero-length transition jumps to its target whatever its easing
  (backlog C126): with a spring (`snappy`, `spring`, `bouncy`, `smooth`,
  or a bounce on a timed curve) the integration at no response time blew
  up, and Kit's slider fill drew at zero width while dragged. A reach for
  an unbuilt row begun at a frame's end — a type-ahead answer, a
  `focus_item` — is landed by the frame it owes rather than scrolled for
  twice in the frame it began in.

  *What you can delete:* an easing dropped whenever the transition is
  zeroed.
- A `when` entry that sets a shadow's offsets keeps the node's shadow
  colour, blur and spread (backlog C114): an entry patched the shadow
  whole, from a probe whose colour was transparent, so `shadowX` /
  `shadowY` in a rule drew no shadow.
- A quick click on a field that opens on the press leaves its menu open
  even with the current row under the pointer (backlog C115): the release
  picked the item under it before asking whether the pointer had moved.
- A view that reads its scroller's geometry before the scroller has been
  laid out — by label before it declares it, or by key before its first
  layout — gets the next frame built against the layout (a
  `read before layout` or `resliced` request in a recording). Every
  binding's `list` and `uniform_list` and every Kit list read that way on
  the frame its tab was picked, sliced by a screenful of the window, and
  nothing asked for the frame after: Watchtower's events log showed its
  wrapped rows piled on each other for up to a second after each switch
  back to Events, until the sample's one-second timer drew a frame. The
  log now settles two frames after the click. An `on_layout` box read
  with `layout_of` by label before it is declared owes the same frame.

  *What you can delete:* a `request_frame` a view made on the frame its
  list or chart first appeared, to get the frame after it.
- `answer_typeahead` given while a view builds is taken and applied when
  the frame ends, against the tree it built, as a reach is landed
  (backlog C104): Kit, which answers from its draw, was refused, so
  type-ahead to a row past a list's window never landed.
- Lua's `env.tokens` lists a token another origin exports — `$kitAccent`
  — whose value a prop already resolved (backlog C112); its colours are
  `0xRRGGBBAA` integers, as Teal types them, which the docs now say.
- The answer to a file dialog a component instance asked for (`env.
  request_files` from its fill) carries that instance, so what the
  component proposes in reply reaches its placer's message (ADR 0067):
  a Kit field's Choose… heard the path and could tell nobody.
- While one of the core's menus is open — a context menu (`open_menu`,
  a `select`'s) or the drawn menu bar's — a press matching an enabled
  row's accelerator chooses that row, posting its event as a click on it
  would, and closes the menu, as a native menu does (backlog C98): Kit's
  Actions menu open, ⌘C on its Copy link row did nothing and the menu
  stayed. Rows inside submenus count, and a standard row's own
  accelerator (Copy's ⌘C) performs its role; a disabled row, or one
  under a disabled row, takes nothing. `Accel::matches` is the press test
  the devtools' chords already used.
- An `anchor` scroller the last layout left at its end stays at its end
  in the next, whatever changed size — its rows or its own box — until
  something scrolls it away: a log whose panel grew sat a band short of
  its end for one frame, until the view's own pin to the end came round
  (from Watchtower). `anchor`'s documentation says so.
- A tooltip shows while the keyboard holds focus on its node, as it does
  under the pointer (`hint_shows`): a bar reached by Tab showed none.
- A composite is entered on its `checked` item after a `selected` one
  (ADR 0007 amended): a radio group and a segmented control were entered
  on their first item whichever was chosen.
- The keyboard can leave a multiline editor (backlog C99, which replaced
  this round's first answer, `tab_inserts`, before it shipped): the
  editor still takes Tab as indentation, the editors' model rather than
  the browser textarea's, and gives three ways out, as WCAG's
  no-keyboard-trap criterion asks. Escape then Tab moves focus on
  (CodeMirror's; the Escape arms the next Tab only, any other key disarms
  it, and Shift-Tab after it goes back); Ctrl+M — Ctrl+Shift+M on macOS,
  as Monaco binds it — flips the window's "Tab moves focus" toggle, under
  which every such editor walks the ring (`Core::tab_moves_focus` /
  `set_tab_moves_focus`, `Ui::tab_moves_focus`, Lua
  `env.tab_moves_focus`, Node `tabMovesFocus()`, C `kui_tab_moves_focus`,
  Odin `tab_moves_focus`, for a status label); and Tab under Ctrl, Alt or
  the primary walks it always. Shift-Tab takes up to four spaces (or a
  tab) back from the start of the caret's line. An editor declared
  `tab_moves_focus` (JSX `tabMovesFocus`, C `KUI_EDIT_TAB_MOVES_FOCUS`,
  Odin `EDIT_TAB_MOVES_FOCUS`) is left by Tab and Shift-Tab as a field
  is, as a browser's textarea is. `keep_tab` now holds for a multiline
  editor too. While the devtools are open their Ctrl+Shift+M (the menus
  toggle) comes first.
- `key_focus` on an editor takes focus as the editor first shows, in Lua
  and Node: the editor path does not open through `open_from`, where the
  row is honoured, and dropped it (Kit's `autofocus` never took the
  keyboard from the button that opened the field).
- A component instance placed by an extension's root — every instance of
  a `kui run` app, whose view is its extension's root — hears its own
  events (ADR 0067). `Extensions::finish` pruned the instances that heard
  an event before it drew the roots, when their records were still last
  frame's, so the event was forgotten and the instance replayed what it
  had shown: a Kit select in a runner app opened on some clicks and not
  others, and a press outside never closed its menu.
- `kui run --headless` ends with 1 when a task throws, where it printed
  the error and ran on: a headless test whose check threw waited for a
  `kui.exit` the throw had skipped, and hung (from the Kit samples).
- A request from a runner app to its own `kui.http.serve`, and any reply a
  task waits on, is answered as the wake arrives — `App::on_wake`, new in
  kui-native — instead of at the frame the wake asks for: in a window
  each hop waited for the display's refresh, about 29 ms a round trip at
  60 Hz, now about 13 (from the Kit samples).
- A one-line text in a box that scrolls sideways scrolls (backlog F205).
  A `nowrap` text was clamped to its parent's width like any other, so
  its line ran past the box and was cut there with nothing to scroll to
  — a code block, a log line, a wide label in a strip. In a box that
  scrolls x a text is now no narrower than its min-content — the whole
  line of one that does not wrap, the longest word of one that does —
  so the line counts in the box's content, as CSS counts unbreakable
  overflow, and a text that wraps still wraps at the box.
- `runWindowed(app, { extensions })` loads them: `windowOptions` dropped the
  key, so a Node window opened with none, and `WindowOptions` did not
  declare it (backlog C75).
- Kit's notice and toast keep their buttons reachable by assistive
  technology: `live` sits on the message, not on the whole row.

- A Lua host's children inside another script's component are placed as
  the host's: a Kit button the app puts in a Kit toolbar is drawn by Kit,
  under Kit's origin and palette, where it was taken for the toolbar's
  own and drawn in place under the app's, with every `$kit…` token
  missing (backlog C73).
- Kit's line chart and waterfall fill the width they are given; they read
  their width with `layout_of`, which keeps a rect only for a node that
  declares `on_layout`, and now they do.

- A component whose own event changed what it draws — a select opening, a
  section folding, a waterfall zoomed by the wheel — shows it on the next
  frame; it was replayed unchanged while its props held (backlog C68).
- A component that paints with an add-on's `$tokens` repaints when the
  add-on declares new ones — Kit switching skin — instead of being
  replayed in the old (backlog C69).
- A component's host children past the 83rd keep keys of their own; the
  84th shared the component's first node's, with `duplicate-key` (backlog
  C71).

- An editor of your own types ⌥- and every other character a Mac's Option
  composes: a key sink's press carries it as `text` beside the chord its
  `code` and `alt` spell, as kui's own editor inserts it (backlog F195,
  and under **What breaks**).

  *What you can delete:* a sink's own reading of ⌥ chords into the
  characters a Mac types for them.

- A line of text at a size far past any screen — a million pixels, a few
  thousand characters — lays out and draws, where a build with overflow
  checks panicked and a release build wrapped: a glyph laid past where
  an `i32` reaches is not drawn (cosmic-text added one to the position
  it had saturated), and a wrapped long line's row count is held at half
  a `u32` (backlog FZ15, found by the `scenes` fuzz target).

- The menu bar drawn in the window — Windows and Linux — acts on the
  field the user was in. A press on a title took the keyboard, so the
  menu, a modal, remembered the *title* as the focus it displaced and
  handed focus back to it on close: the field lost its caret, Select All
  selected nothing, and Paste — answered by the runner before the next
  frame — landed on the menu's row. The bar now leaves the keyboard where
  it was (`keep_focus`), and Paste hands it back to the field before
  asking for the clipboard, as Cut and Select All do; the right-click
  menu's Paste the same. Found driving an app on Windows.

  *What you can delete:* an app's own Edit-menu handling that refocused
  the field, or read the clipboard itself, because the drawn bar's rows
  did nothing.

- The drawn menu bar is window chrome under custom chrome: its room past
  the titles moves the window, as the titlebar beside it does (and under
  **What breaks**). An open bar no longer warns `modal-behind-content`
  about itself — it is in flow by design, and the app under it inert on
  purpose.

  *What you can delete:* a `window="drag"` box an app laid behind the
  bar, and a filter on the warning.

- F5, F12 and every other function key reach an app's keymap from a
  focused field (and under **What breaks**): the editor took them as
  typing, though they type nothing and compose nothing, so a devtools or
  refresh shortcut worked everywhere in a window but the field the user
  was typing in.

  *What you can delete:* a field-level `onKey` that forwarded function
  keys to the app's keymap.

- A sliding node keeps to its parent while an ancestor moves (backlog
  C97, and under **What breaks**): the stock switch's knob, and Kit's,
  trailed the track through a window resize, and a sliding child trailed
  its sliding parent by its own transition. Only the node's own move
  within its parent eases now
  (`crates/kui-core/tests/slide_relative.rs`).

  *What you can delete:* a `slide` an app turned off while the window
  resized, and a knob drawn by hand at the track's end to dodge it.

- A path whose `d` changes costs what its size says (backlog C96): the
  mask's raster kept each scanline's cells in a list it walked from the
  start for every new one, so a row crossed by k edges cost k², and the
  area under a 1,400-vertex zigzag over a 360 px plot took 1.65 s at
  scale 2, the 1 px bleed's outline two thirds of it. The cells now go
  to one list sorted by row at the end (kui-core's `raster.rs`, zeno's cell
  walk and sweep over storage of kui's own), and the masks are the same
  to the byte: the zigzag rasterizes in 5.5 ms, a 1,400-point area in
  0.6 ms where it took 2.5, a 300-point one in 0.16 ms where it took
  0.35. An outline of 64 commands or fewer still goes to zeno's mask,
  which is quicker for so few edges. `benches/paths.rs` measures it.

  *What you can delete:* an area chart cut into polygon chunks, or
  thinned before it reached a `path`, to dodge the raster.

### Native verification

No pre-tag pass on the release machines this time: the first release
whose check is GitHub's `ci.yml` in full, run on main and again as the
tag's `check` (Linux, lavapipe), with `smoke.yml` on macOS and Windows
beside it, non-blocking.

On the Mac (M3 Pro, macOS 27) before the tag: `release-prep.nu` (every
generated file current), fmt, clippy with `-D warnings` and the
workspace tests — **2926 tests over 234 suites**, no failure. Main's
first `ci` run went red on fmt, which nothing had checked since alpha.66:
the 2026-10-26/27 rounds had left 51 files unformatted, the Kit Rust
wrappers came out of their generator as rustfmt would not write them
(it formats them now) and with lints clippy refuses, and a dozen more
lints had landed in kui-core, kui-lua and kui-cli; all fixed before the
tag. The tag's first `check` then failed on Linux alone, twice over, and
the tag was moved to the fixes before anything was published: the
toast-exit test matched each quad to the nearest one of its shape at
rest, and two of DejaVu Sans's glyphs in "Saved 2." share a box (it
measures each against the face's offset now, and passes with DejaVu on
the Mac too); and kui-cli's process test spawned `sh -c "sleep …"`,
which dash keeps as the sleep's parent where macOS's sh execs, so the
sleep outlived the app it was not a child of (`exec sleep` now).
`ci.yml` runs the tests `--no-fail-fast` since, so one red suite no
longer hides the rest. The steps after the tests had not run since
alpha.66 either, and two more were red: `index.d.ts` named a `Json` type
nobody declares (kui-node's channel aliased `serde_json::Value`, and
napi-rs writes the name it sees; `any` now), and both `devtools_tab`
examples walked to their tab in two Ctrl+Shift+N presses, from before
the Perf tab made it three; and `edge_scroll`'s drive pressed the text's
middle, which under DejaVu is the code block's sideways scrollbar (it
presses in the first paragraph now; a press inside the code block hands
the drag on as it should, checked with both fonts). The moved tag's run
then passed its check and failed linking the Linux runner binary, the
release's first executable built at the glibc 2.28 floor: lld refused
the runner image's libasound.so, which asks for a newer glibc than zig's
stubs; it links with `--allow-shlib-undefined` now (a user's libasound
is their distribution's). A by-hand run of the release workflow on main
then went through every job but `publish` before the tag moved again.
The next run's `publish` stopped before uploading anything: kui-lua's
new dev-dependency on kui-ffi carried a version, which `cargo package`
keeps and a publish of the crates together cannot meet (kui-ffi depends
on kui-lua and goes up after it); it is path-only now, and dropped. Through those rounds the workspace suite ran at each merge, and the
Kit gallery's paint audit (`KUI_PAINT_AUDIT=1`, ADR 0074) over every
page.

Not verified natively this release: the windowed round, the
accessibility audit and the C round on a workstation, anything on
Windows or Linux by hand, fuzzing, and the npm half of the release
workflow, which only the tag's own run exercises.

## 0.1.0-alpha.67 (2026-10-24)

**What breaks.**

- `hoverBg` lights while the pointer is over the node or anything inside
  it, a clickable child included; it lit only while the node itself was
  the innermost thing under the pointer (backlog F190).

A list row with `hoverBg` holding its checkbox, a field and its buttons
went dark whenever the pointer crossed any of them — over most of the
row, where the checkbox region filled it — and lit only in the gaps
between. It now stays lit across them, as CSS `:hover` does and as a
`revealOnHover` anchor already was hovered; a child's own `hoverBg`
lights beside it. A view that wanted the old reading — a card that dims
while a button inside it is hovered — keeps it with the child's
`onHover` in the model. `pressedBg`, `onHover`, `isHovered` and
`hoverGroup` are unchanged: a press is still on the node it began on, and
a group still lights its members together.

### Added

- **A wheel feel to choose** (backlog F193, [ADR 0050's
  amendment](docs/adr/0050-a-wheel-notch-glides.md#amended-2026-10-24-a-feel-the-app-chooses)):
  how far a mouse wheel's notch goes and how quickly its glide lands,
  for the whole app. Presets by name, slow to fast: `gentle`,
  `standard` (the default, unchanged: 40 px a notch, landed by ~283
  ms), `brisk` (a touch faster: 48 px, nine tenths there by 150 ms) and
  `swift` (64 px); or the numbers, `WheelFeel { line_px, response, kick
  }`. `Core::set_wheel_feel` / `Core::wheel_feel` and
  `WheelFeel::preset`; C `kui_set_wheel_feel`, `kui_set_wheel_preset`,
  `kui_wheel_feel`; Odin `set_wheel_feel`, `set_wheel_preset`,
  `wheel_feel`; Node `setWheelFeel('brisk' | { linePx, response, kick
  })` and `wheelFeel()`, and `wheelFeel` to `runWindowed` and
  `createApp`; the launcher's `wheel_feel`. `KUI_WHEEL=brisk` (or
  `KUI_WHEEL=56,0.25,0.6`) tries one in any native window without a
  rebuild, and overrides the feel the app opens with, in every binding;
  the `wheel_feel` example spells the line to copy.

### Changed

- **Five CPU loops run side by side or vectorized** (backlog C57). None
  changes what is drawn or what an app reads.
  - A text's cache key (`key::hash_bulk`, every text of 32 bytes or more,
    every frame) hashes 64-byte blocks over eight chains folded into
    one, where one chain waited on its multiply every eight bytes:
    thirty-four chunked rows of 500k characters, steady, read **0.51
    ms** a frame, from 2.76 (`long_rows_34x500k_steady`), and a 100k
    line scrolled 6.7 µs from 21. The hash is not kept across versions;
    `key::fnv`, and every digest on it, is untouched.
  - A capture's readback (`capture()`, each recorded frame) straightens
    sixteen pixels a step — swizzled and their alphas' least and most
    read in one NEON pass on aarch64, as words elsewhere — and divides
    by the alpha through a table only in a block that holds a
    translucent one: a 2560×1664 frame, ~1.5 → ~0.4 ms opaque and ~5.1
    → ~2.9 ms translucent throughout, on the recording's thread.
  - A long line's cut searches each half window from the end it wants —
    the tab by memchr, the last whitespace and grapheme boundary from
    the back — where it walked every half forward for the last of
    something: `long_line_100k_edit` 1.26 → 1.18 ms.
  - The newline scan a text the cache has not seen pays reads 32 bytes
    a step, ~5× as fast over a long one.
  - A glyph's or a path's coverage lands in the atlas as white filled
    once and its alphas written over it, 2–2.7× as fast as a push a
    pixel.
- **`@qxuken/kui` reaches npmjs live, with provenance.** The GitHub
  release workflow publishes it through npm's trusted publishing (OIDC,
  no stored token) and moves `latest` by the same rule
  `scripts/npm-approve.nu` applies, where it staged the version with a
  stage-only token for a maintainer to approve by hand. The `NPM_TOKEN`
  secret is no longer read; `npm-approve.nu` stays for a version staged
  by hand ([docs/releasing.md](docs/releasing.md)).

### Fixed

- On macOS a Node window scrolls at the display's rate again under a
  `node` built against the macOS 27 SDK — Homebrew's is: ~98 frames a
  second on a 120 Hz screen, where it drew ~24. Built against that SDK,
  AppKit draws the redraw an event asks for inside the pump that saw
  it, a display frame later, and the runner's second turn of that pump
  answered `nextDeadlineMs()` with nothing due, so `runWindowed` read
  every scroll event as an idle window and pumped at `idlePumpMs`. The
  official Node build, built against SDK 15, never saw it (backlog
  RG169).
- On macOS a window run by `kui_native::run` or `Launcher::run` — every
  Rust and C app — scrolls at the display's rate again when the binary
  was built against the macOS 27 SDK, as today's Xcode builds it: ~110
  frames a second on a 120 Hz screen under a trackpad, where it drew
  ~57. Built against that SDK, AppKit hands the app an event in the
  same turn of the run loop as the display link's tick, just ahead of
  it, and the tick found nothing asked for yet; the frame the event
  asked for was held for the next vsync, so only every other vsync drew.
  A frame asked for after a vsync that had nothing to draw now draws at
  once, as that vsync's frame (backlog RG170).
- The keyboard focus ring is whole on a node whose ancestor clips at the
  node's own edge — a full-height row in a card with `clip`, a field
  filling a scroll box: it is drawn just inside the node's edge there,
  where the ring 2 px outside lost the sides the clip cut. A node with
  room keeps the ring outside, and one the clip cuts too keeps it cut
  with it (backlog F191).
- A node with `slide` inside a scroll container moves with the content
  when it scrolls, on the same frame, where it eased after it from where
  it had sat on screen — every wheel notch trailed by the node's own
  transition, each racer of the transition example on its own curve. A
  move in the content still eases, scrolled or not (backlog F192).

**What you can delete.**

- A lowered `idlePumpMs` or `quietMs`, or a pinned official Node build,
  kept only so a Node window did not stutter when scrolled under
  Homebrew's Node.
- `KUI_FRAME_PACING=0`, set only so a Rust or C app built with Xcode 27
  scrolled at the display's rate.
- A margin or an inset wrapper kept only so a focused row's ring was not
  cut off by the card that clips it.
- A `slide` dropped, or keyed to a flag, while its container scrolls,
  kept only so the node did not trail the scroll.
- A `hoverGroup` shared by a row and its clickable children, or an
  `onHover` on each child feeding a hovered flag back into the row's
  `bg`, kept only so a row stayed lit while the pointer was over them.

### Native verification

One pre-tag pass on 2026-10-24 on the Windows machine and its WSL, over
everything since alpha.66, the two legs side by side.

**Windows** (11 Pro, RTX 5080 at 240 Hz, 150%): clippy, the C round,
`nu scripts/test.nu --node` — **2623 tests over 183 Rust suites and
Node's**, again on the versioned tree — the headless round and the
windowed round, 57 examples drawing 120 frames on both bases. C57's
x86_64 timings and its bench guard against `3c8d111c` were taken on this
machine the same day, while it was built.

**Linux** (WSL Ubuntu 24.04, llvmpipe): CI's `check` in its order —
`release-prep.nu` (every generated file current), fmt, clippy, the
workspace tests and Node's (2618 over 182 suites), the conformance
report, the C round, the Odin leg with `gen --check` (on dev-2026-09;
CI pins dev-2026-10), the examples' typecheck and the headless round —
all clean; the windowed round on X11. Fuzzing: five minutes of all seven
targets at once on eight cores, about 11.9 million inputs, no crash,
hang or OOM.

Not verified natively this release: anything on a Mac, and the npm half
of the release workflow, whose trusted publishing only the tag's own run
can exercise.

## 0.1.0-alpha.66 (2026-10-24)

**What breaks.**

- `setDevtools(true)` (`Core::set_devtools(true)`, `kui_set_devtools`)
  on a panel hidden by its close button, or by `setDevtoolsDock('off')`,
  shows it again, where it was; alpha.65 left it hidden, since the
  panel was on already. The `Ctrl+Shift+I` chord, a pick and a declared
  devtools tab bring a hidden panel back where it was too, where they
  put it on the right (backlog F188).
- On macOS 26 and earlier, under the Node driver or any host that pumps
  the runner (`PumpRunner`, `KuiWindow.pump`), a left press on the edge
  of a window whose app is not active is resized by kui, not AppKit:
  the press, its drags and its release never reach AppKit or the app,
  the window comes forward, and its sides follow the pointer in that
  gesture. alpha.65 started a resize that often stuck (below). The
  native runner, a press anywhere else, and macOS 27 are unchanged
  (backlog RG168, C54).
- A node's drop shadow (`shadowColor`, `Shadow`, `shadow_color`) is no
  longer drawn under the node itself, as CSS's `box-shadow` is not. A
  node with a translucent `bg`, or one faded by `opacity` — a card
  fading in or out — shows what lies under it where alpha.65 showed its
  own shadow, so it reads lighter. The shadow outside the node is
  unchanged (backlog V10).
- In a window, a node faded by `opacity` — every disabled stock control
  among them — is drawn with its subtree as one picture and the picture
  faded once, as a browser fades it: a label sits on its own fill and
  fades with it, and overlapping pieces show no seam. alpha.65 faded
  each piece on its own, so the fill showed through the glyphs of its
  own label; on a page darker than a control's fill, a disabled label
  now reads a little dimmer than it did. Text inside a faded subtree is
  grayscale where the window draws subpixel text. A float under a faded
  node (a tooltip, a menu) still fades on its own. A headless driver and
  a host drawing the quads itself see alpha.65's quads until they ask
  (`set_opacity_groups`, below; backlog V11, [ADR
  0056](docs/adr/0056-a-faded-subtree-is-drawn-as-one-picture.md)).
- Rust: `QuadKind` has two variants more, `GroupBegin` and `GroupEnd`,
  which an exhaustive `match` names; `QuadKind::ALL` has twelve.
- C: `KuiMenuAction` appends `width`, `height` and `rgba` for a new
  kind, `KUI_MENU_ACTION_SET_CLIPBOARD_IMAGE`, and grows to 64 bytes on
  64-bit; `KUI_ABI_VERSION` is 37. Recompile; a host that drains menu
  actions puts the picture on its clipboard, or drops a kind it does not
  know as before. Rust: `MenuAction` has a variant more,
  `SetClipboardImage`, which an exhaustive `match` names. Node's
  `takeMenuActions()` can hand out `{kind: 'setClipboardImage', width,
  height}`.
- A wheel's notch glides longer and starts softer: nine tenths of the
  way by 170 ms and landed by about 280 ms, where alpha.65 was nine
  tenths there by 100 ms and landed by 180. A view or a test that reads
  a container's offset a few frames after a notch reads less of it
  ([ADR 0050](docs/adr/0050-a-wheel-notch-glides.md), amended).

Notches a hand turns slowly, 150–250 ms apart, each started at full
speed and had stopped by the next, so the page jerked forward and
halted once a notch: two notches 150 ms apart went from half a pixel a
frame to fifteen. Each notch now gives the spring 60% of that speed,
on a 0.30 s response where it was 0.21 s. The first frame is still each
notch's fastest, and the same pair goes from 1.3 px a frame to 8.8. A
fast spin trails its target by under a notch, as before.

The close button hides the panel and leaves it on, so its chords can
bring it back, and `devtools()` stays true. An app's toggle on
`devtools()` turned off a panel the user had closed, and the press
after turned on a panel still hidden: it never came back. Turning on a
hidden panel now shows it. A toggle reads whether the panel shows as
`devtools() && devtoolsDock() !== 'off'`, or it takes two presses after
a close.

A press on the resize edge of an inactive window goes to AppKit's
delayed first-click path, which waits 78–230 ms for the activation to
come back and only then starts the resize. A pumped runner runs AppKit
a slice at a time and leaves it between, and on macOS 26 a quick
click's release came before the activation did: the resize that
followed waited for a release that had already come, the window
followed the pointer until the next click, and a press held and let go
outside the window hung the process inside AppKit, deaf to ⌘Q. Posting
a release back after activation did not end it, and neither did handing
the press back once the app was active with the button still down;
dropping the gesture and activating the window does, and kui resizes
the window from the dropped drags itself, within its min and max sizes
and under the menu bar, so a press from another app still resizes in
one gesture (backlog C54). `KUI_EDGE_RESIZE=off` only activates, and
the second press resizes. On macOS 26 the resize cursor can stay after
such a gesture ends outside the window, until the pointer enters it.

A node's group `opacity` multiplies into every quad its subtree draws,
and its shadow is one of them, drawn first, under the node. Faded, the
node let its faded shadow through: a white card at 50% over a light
page came out grey, up to 50 levels off the card faded as a whole over
most of its body. The shadow quad now carries the node's own rounded
box (`Quad::shadow_knockout`: its insets in `uv`, its radii in
`border_color` and `border_w`, fields a shadow had no use for) and is
not drawn there, so
the same card is within rounding of the card faded whole, but for a
few levels on its one-pixel antialiased edge. A translucent `bg` was
the same case without the fade. A node that relied on its own shadow
to darken a translucent fill sets a darker fill. A renderer of a host's
own that ignores those fields draws the shadow as before; kui.h's
`KUI_QUAD_SHADOW` says how to read it.

### Added

- **A frame as pixels** (backlog C55, [ADR
  0054](docs/adr/0054-a-frame-is-drawn-again-to-be-read.md)):
  `kui_native::capture(&core)`, `capture()` on Node's `Ctx` and
  `KuiWindow`, `kui_capture` in C (freed with `kui_pixels_free`) and
  `capture` in Odin. The core's last finished frame is drawn again from
  its draw list on a GPU device of the process's own, with no window,
  and read back as straight RGBA in physical px over the theme's `bg`,
  the shape `addImage` takes, with the scale it was drawn at. A
  window's core captures what the window shows, less what the OS draws
  behind a translucent one; a headless core captures the same way,
  which is what a test wants. The first capture opens the device and
  builds the pipelines (about 40–100 ms on an M3 Pro); a 3456×2234
  frame after that takes about 10 ms. Lua has none: reading back is the
  host's.
- **kui-wgpu draws without a window**: `Gpu::windowless()`, or
  `Gpu::from_parts(instance, adapter, device, queue)` over a device the
  host opened itself, `Renderer::windowless(&gpu, format)`, `Renderer::render_to(list,
  atlas, &texture)` for a host that composites kui into a texture of
  its own each frame, `Renderer::capture(list, &atlas)` returning a
  `Capture`, and `Renderer::format()`. A windowless renderer's `render`
  returns `RenderError::Skip`.
- **The runner encodes as well as decodes** (ADR 0054, decision 7), with
  the crates it already links, so a test or a probe keeps what it saw as
  a file with no image library of its own:
  - `kui_native::encode_image(format, width, height, &rgba)` —
    `encodeImage('png' | 'jpeg' | 'webp' | 'gif', w, h, rgba, {quality})`
    in Node, `kui_encode_image(KUI_ENCODE_*, …)` in C (freed with
    `kui_pixels_free`), `encode_image(.Png, …)` in Odin. PNG is lossless
    at the fast setting (about 12 ms for a text-heavy 3456×2234
    capture), JPEG lossy with the alpha dropped, WebP lossless, GIF 256
    colours.
  - `kui_native::AnimationEncoder` — `new AnimationEncoder('png' | 'gif',
    w, h, {loops})` with `add(rgba, seconds)` and `finish()` in Node,
    `kui_animation_encoder_new` / `_add` / `_finish` / `_free` in C,
    `animation_encoder_*` in Odin: an animated PNG or GIF written a frame
    at a time, a recording of captures. It keeps only the rectangle each
    frame changed, folds a frame that changed nothing into the time
    before it, and gives a frame too short for the format (1 ms for PNG,
    20 ms for GIF) to the next. A GIF frame that turns a pixel clear is
    written whole on a cleared canvas, since a GIF's clear pixel shows
    the one under it (backlog FZ12), and a frame held past what one
    delay holds (65.5 s in an APNG, 655 s in a GIF) as frames that
    change nothing, so a recording left alone keeps its time, up to 4.6
    hours a frame in an APNG (backlog FZ13). Thirty retina frames of a card sliding
    over a page: about 12 ms a frame to capture and add, 15 ms to finish
    an 804 KB APNG, 393 ms for a 283 KB GIF.
- **An app keeps its own window as a file** (backlog C56, [ADR
  0055](docs/adr/0055-an-app-asks-for-its-window-as-a-file.md)):
  `capture_to(path, message)`, `record_to(path, message)` and
  `stop_recording()` on `Core` and `Ui` — `captureTo` / `recordTo` /
  `stopRecording` on Node's `Ctx` and `KuiWindow`, `kui_capture_to` /
  `kui_record_to` / `kui_stop_recording` in C, `capture_to` /
  `record_to` / `stop_recording` in Odin. The runner serves the ask
  after the next frame it draws, in every binding, drawing that frame
  again as `capture` does, and answers with a new `capture` event in the
  message the ask named: `capture: {phase: "saved" | "failed", path,
  width, height, frames, seconds, error}`, the path and never the
  pixels. The path's extension is the format: `.png`, `.jpg`, `.webp` or
  `.gif` for a still, `.png` / `.apng` or `.gif` for a recording, whose
  frames each show until the next on the frame clock. A headless driver
  serves its own: `kui_native::Captures::serve(&mut core)`,
  `ctx.serveCaptures()`, `kui_serve_captures(ctx)`, `serve_captures(ui)`;
  `Core::take_capture_commands()` drains the asks as data for a test
  with no GPU. Rust reads the answer as `Event::Capture(Captured)`, Odin
  with `With_Capture`, Node with `WithCapture`. A window writes a
  recording on a thread of its own: the frame is drawn again and its
  readback submitted on the loop's thread (about 0.6 ms a 1720×1200 frame
  at 60 Hz), and waited for, encoded and written on the recording's, so a
  window records at its display's refresh; a headless driver keeps the
  work on its own thread, its answers pending when `serve` returns
  (`Captures::write_on_thread` chooses).
- **A faded subtree drawn as one picture** (backlog V11, [ADR
  0056](docs/adr/0056-a-faded-subtree-is-drawn-as-one-picture.md)):
  `Core::set_opacity_groups(true)` — `setOpacityGroups` in Node,
  `kui_set_opacity_groups` in C, `set_opacity_groups` in Odin — brackets
  the quads of a node whose own `opacity` is under 1 between a
  `GroupBegin` and a `GroupEnd` (`KUI_QUAD_GROUP_BEGIN` = 10,
  `KUI_QUAD_GROUP_END` = 11), unfaded by it, the end carrying its
  bounds and its fade. A group of one thing drawn, or holding a
  backdrop blur, is left as the fade per quad. kui-wgpu draws each group
  into a texture before the frame and the picture into the frame; the
  runner turns it on for every window, the book's pictures for theirs.
  `benches/groups.rs` in kui-wgpu prints what it costs.
- **A picture on the clipboard**: `set_clipboard_image(width, height,
  rgba)` on `Core` and `Ui` — `setClipboardImage` in Node,
  `kui_set_clipboard_image` in C, `set_clipboard_image` in Odin — queues
  straight RGBA (a capture, a decoded image) for the clipboard, which the
  runner writes as the platform's image type: "Copy image". And
  `capture_to_clipboard(message)` — `captureToClipboard`,
  `kui_capture_to_clipboard`, `capture_to_clipboard` — is "Copy
  screenshot": the next frame drawn, served as `capture_to` is, queued
  for the clipboard, and answered with a `capture` event whose `path` is
  empty (ADR 0055). A headless host drains the picture with the other
  menu actions. `examples/rust/features/capture.rs` has the button.
- `kui_wgpu::Renderer::capture_begin` and `PendingCapture::wait`:
  `capture` in two halves, the second for any thread. `GlyphAtlas::rev`
  and `GlyphAtlas::id`, which a renderer that is not the atlas's own
  reads to tell whether its copy is current.
  `examples/rust/features/capture.rs` is a screenshot button and a
  "Record 5 seconds" toggle. Lua has none: a guest's frames are the
  host's.
- `Core::drawn()`: the finished frame's list and atlas for a reader
  that is not the core's renderer, marking nothing read and leaving the
  atlas's `dirty` flag alone.
- **Pictures in the book's Examples part**: every example page under
  `apps/`, `widgets/` and `features/` opens with the example as it
  draws, made by the example itself through `capture` and the encoders
  above. The harness every example runs inside takes `--shot PNG`, the
  settled frame drawn headlessly at scale 2, and `--record FILE`, the
  example's `Example::RECORDING` (clicks at times on a stepped clock,
  25 frames a second) as a GIF or an animated PNG, both on the light
  theme unless `--dark`, with kui's accent and grayscale text, so a
  rerun draws the same bytes. `transition`, `spring`, `enter_exit` and
  `transform` have recordings, and their pages show them as GIFs (an
  APNG of `transition`'s cycles was 11 MB, the GIF 3 MB). `nu
  scripts/book-examples.nu pictures` draws them all again from nothing
  and needs a GPU; the pages and `--check` only read which pictures are
  committed, so CI needs none. `enter_exit` expires its toasts on the
  frame clock (`ui.now`, `ui.request_frame_at`), not `Instant`, so a
  recording sees them go when its clock says.

### Fixed

- An example under `kui-devtools`' harness hears `on_event_with`, with
  its window's core, in a window as it did in a headless drive; the
  windowed harness called only `on_event`.
- A devtools panel closed with its close button comes back when the app
  turns it on, and every way back puts it where it was (above; backlog
  F188).
- On macOS a repainting window holds ~75 MB less GPU memory and spends
  about half as long in `Renderer::render`: a frame's instances,
  globals and fragment parameters are written into mapped buffers the
  GPU reads in place, where `write_buffer` copied them in every frame
  and the copy kept Metal's blit engine, ~70 MB of it, awake for as
  long as frames came (backlog C53). The `waker` example went from 191
  to 115 MB of graphics memory and from 271 to 194 MB of footprint,
  1.12 to 0.56 ms a frame in `render`. `KUI_MAPPED_FRAMES=0` keeps the
  copy. A frame drawing a texture-backed image or a backdrop blur still
  copies those draws' parameters.
- On macOS 26 a Node app's window no longer sticks to the pointer, or
  hangs, after a click on its edge from another app (above; backlog
  RG168).
- A card fading in or out with a drop shadow no longer turns grey with
  its shadow showing through it, and a translucent card no longer shows
  its own shadow (above; backlog V10).

### Changed

- `cargo audit --deny warnings` is clean again over two advisories that
  landed on main's dependencies after alpha.65: tokio 1.53.1 →
  1.53.3 (RUSTSEC-2026-0386, `AsyncFd::new` unsound; in the lockfile
  only through `wasm-bindgen-futures`, which no target kui ships
  compiles), and clipboard-win 5.4.1 → 5.4.3, which arboard reads the
  Windows clipboard through: 5.4.1 and every version before it were
  yanked for reading HTML off the clipboard by a size the clipboard
  gave and nobody checked, which 5.4.3 bounds. Both inside the two
  weeks the lockfile otherwise waits, for the fixes; 5.4.3's diff is
  that bound, a 5 ms retry sleep and doc comments.

- CI's `check` job draws on a GPU: Mesa's software Vulkan (lavapipe,
  Debian's `mesa-vulkan-drivers` and `libvulkan1`) under
  `WGPU_BACKEND=vulkan`, so the capture tests in kui-wgpu, kui-native,
  kui-ffi and Node, the backdrop blur's, and the C and Odin counters'
  last frame run there instead of skipping for want of an adapter. With
  `KUI_REQUIRE_GPU` set (CI sets it to `llvmpipe`) a missing adapter is
  a failure in each of them, and kui-wgpu's tests also fail on an
  adapter whose name does not contain the value (`1` takes any);
  unset, they skip as before, saying so. lavapipe (Mesa 22.3 on
  bookworm) offers dual-source blending, so the per-channel text path
  is the one CI draws, and no pixel assertion needed loosening.

- The repository pins its line endings to LF (`.gitattributes`, `*
  text=auto eol=lf`), so a Windows clone with `core.autocrlf=true` no
  longer checks every text file out as CRLF, where the generators write
  LF and `release-prep.nu` then read every generated file as stale. No
  file in the repository changed; a clone made before keeps its CRLF
  files until they are checked out again. `odin.nu gen` writes
  `layout.odin` as LF on Windows too, and `gen --check` names a file
  that differs by its line ends alone as such.

- Forgejo's `ci` workflow (`check`, and `publish`, the crates to the
  drydock9 registry) and its `smoke` workflow run by hand only (Actions
  → Run workflow), where they ran on every push to main and every tag;
  `publish` runs when started on a `v*` tag with its `publish` box
  ticked, after `check`. The book and the advisories keep their own
  triggers, and GitHub's release pipeline is unchanged. With no check
  on main left to ask, `release-prep.nu` drops its `ci` step (`--skip
  ci` is now an unknown step), and the tag goes out with a plain `git
  push origin main <tag>`.

**What you can delete.**

- A `screencapture` of the window's id, or another tool outside the
  process, kept only to see the pixels a test's frame drew; `capture()`
  hands them back, headless too.
- A shadow dropped for the length of an `enter` or `exit` fade, or an
  opaque `bg` given to a card that wanted to be translucent, kept only
  so the card's own shadow did not show through it.
- A PNG package (`pngjs`, `sharp`, a crate, libpng) kept only to write a
  test's or a probe's pixels to a file; `encodeImage` does it, and an
  `AnimationEncoder` records a sequence of them.
- A recorder of the app's own — a capture per frame while a flag is on,
  the time between frames, an `AnimationEncoder`, the file and its
  errors — kept for a screenshot or a "record" button; `capture_to` and
  `record_to` ask the runner, which does it after the frame is drawn.
- A copy of kui-wgpu's renderer, or a hidden window, kept only to draw a
  frame into a texture your own device composites.

### Native verification

One pre-tag pass on 2026-10-23/24 on the Windows machine and its WSL,
over everything since alpha.65.

**Windows** (11 Pro, RTX 5080 at 240 Hz, 150%): CI's `check` step by
step — fmt, clippy, the conformance report, the C round, the Odin leg
(`check`, `test`, `slots`) on dev-2026-09, the Node addon under
`KUI_CONFORMANCE_REQUIRED=1`, the examples' typecheck, the headless
round — and `nu scripts/test.nu --node` on the tagged tree: **2605
tests over 183 Rust suites and Node's**. The pass found one defect:
Node's slot test faulted in `kui_ffi.dll`, two copies of the library
disagreeing on `KuiCtx`'s layout (fixed before the tag; it never
shipped). The windowed round with Node's: 56 examples on both bases.
kui-wgpu's and kui-native's GPU tests (opacity groups, captures, the
backdrop blur) on the RTX under both DX12 and Vulkan, with
`KUI_REQUIRE_GPU` set so none skipped. The capture example driven
through UI Automation: the saved PNG and the copied picture match a
screen grab of the window, the clipboard holds PNG and DIBv5 with the
right channel order and alpha, and a five-second recording kept 1182
frames at the display's 240 Hz; stopping early by the toggle wrote a
2.0 s one. `release-prep.nu` read every generated file current on this
checkout, the first time on Windows (the LF pin above). The bench guard
against alpha.65 on `frame`: no guarded row more than 10% slower.

**Linux** (WSL Ubuntu 24.04, lavapipe as CI draws, under
`KUI_REQUIRE_GPU=llvmpipe`): `release-prep.nu`, fmt, clippy, the
workspace tests, the book, the C round and conformance, the Odin leg
with `gen --check`, the Node tests and gen, the typecheck, the headless
round, the lockfile check and `cargo audit --deny warnings`, all
clean; the windowed round on X11, 56 examples on both bases. Fuzzing:
30 minutes of all seven targets at once on eight cores, about 72
million inputs, no crash, hang or OOM, besides the fuzz hour in the
backlog that found FZ12 and FZ13.

Not verified natively this release: anything on a Mac. RG168 and C54's
edge resize were confirmed on macOS 26.6.2 while they were built, C53's
memory figures and the clipboard picture on the macOS pasteboard were
measured on the M3 Pro then; the accessibility audit was not run.

## 0.1.0-alpha.65 (2026-10-23)

**What breaks.**

- On macOS a menu row whose `accel` is a bare function key — `"f12"`,
  `"f5"` — binds it: the key chooses the row, where alpha.64 drew
  `F12` beside the row and bound nothing, so the press reached the
  focused node (backlog F187).
- The latency graph's backdrop, the stats overlay's background and
  border, its `?` badge, and the devtools panel's icons and tree
  highlights take their theme colour's own alpha times the fade they
  ask for, where they set the alpha outright: a translucent theme colour
  paints fainter than in alpha.64, an opaque one exactly as before
  (backlog RG165).
- The devtools tree's and events' rows that are not selected or open
  have no background, and the tree's unfocused list no border, where
  each painted the list's `sunken` again (backlog RG165).
- Glyphs are no longer hinted on macOS, at any scale, nor at 1.5× and
  up anywhere: every glyph's bitmap differs from alpha.64's by up to a
  pixel in height, and an app's pixel snapshots of text change. At 1×
  on Windows and Linux text is hinted as before (backlog RG166).

A row's accelerator binds on macOS only with a modifier, since AppKit
fires a bare key equivalent on every press of the key, and a row bound
to `"a"` would eat every `a` typed into a field. A function key types no
text, so the rule kept a developer-tools row on F12, or a reload on F5,
from binding for nothing: the row showed its key and the key did not
choose it. A bare function key binds now, as a key equivalent AppKit
spells with its own character for the key; a letter, Enter or any other
bare key is still drawn and not bound. An app that declared a bare
function key as a hint only and handled the press in an `onKey` sink
gets the row's message instead.

A theme built from an app's design tokens can hold translucent colours —
a `sunken` of white at 10%, an overlay token, reads as a well in a dark
UI. The core tinted a theme colour with `with_alpha`, which replaced
that 10% with the tint's own figure: the latency graph's 60% backdrop
came out white at 60%, a pale slab in a dark panel. And a list painted
`sunken` behind rows painted `sunken` again, a coat a row: opaque, the
second coat was invisible; translucent, it doubled, and the devtools'
tree and events were a flat grey.

Hinting fits each glyph to the pixel grid by the face's own program,
glyph by glyph, and a program that does not treat every glyph alike
snaps one flat top up and the next one down: at 2× in Helvetica Neue,
`п` sat a pixel above `н` in «Запущенные», and `a` beside `c` in
`accordion`, at 11 and 12 px. macOS draws none of its own text hinted,
and at 2× there is no coarse grid to fit; at 1× elsewhere the snap is
what keeps a stem one pixel wide, and the platform's own text is hinted
too. A window that moves between a 1× and a 2× display draws its glyphs
again.

### Added

- `Color::faded(f)`: the colour with its alpha scaled by `f`, for a
  tint of a colour that may be translucent already; `with_alpha` still
  sets it outright (backlog RG165).

- `Core::set_app_min_size`: the app's declared minimum, for a driver
  to hand the core, so a docked devtools pane leaves at least that
  much; the native runner passes its `min_size` (backlog RG167).

### Fixed

- A macOS menu row's bare function key binds (above; backlog F187).
- A translucent theme colour tinted or layered by the core's own chrome
  — the latency graph, the stats overlay, the devtools panel — keeps
  its alpha (above; backlog RG165).
- Letters of one height share a row at 2× and on macOS: hinting put
  some flat tops a pixel above their neighbours (above; backlog RG166).
- Dragging a docked devtools pane wider than the app's minimum window
  size leaves grows the pane no further. The runner sets the window's
  minimum to the app's plus the pane, and the pane stopped only where it
  would leave the app 160 px, so a drag past the app's minimum asked
  for a wider window, the OS grew it, the pane could widen again, and a
  drag to the far edge grew the window off the screen within a second
  (backlog RG167).

## 0.1.0-alpha.64 (2026-10-23)

**What breaks.**

- A leaf's `tooltip` — an image's, a `cells` grid's, an editor's, a
  `line`'s — is not drawn while the leaf is at `opacity: 0`, and a
  hovered leaf inside a departing box takes its hint with the hover,
  where alpha.63 drew the first in full and faded the second with the
  ghost (backlog RG162).
- Under the native runner a mouse wheel's notch glides to its place,
  most of the way in a tenth of a second and landed in under two,
  where it jumped 40 px on the next frame; a view reading a container's offset the frame after a notch
  reads it mid-glide. `Launcher::smooth_wheel(false)`, `smoothWheel:
  false` to `runWindowed` or `kui_set_smooth_wheel(ctx, false)` brings
  the jump back (ADR 0050, backlog F182). A trackpad, and every door
  that feeds pixels (`scroll`, `scrollGesture`, `kui_input_scroll*`,
  `InputEvent::Scroll`), are unchanged.
- A wrapped text's row that a scroll view's bottom edge cuts through
  draws every glyph reaching into the view, where alpha.63 drew it only
  up to its first glyph whose ink starts below the edge (backlog RG163,
  RG164).
- C: `KuiSpec` appends `reveal_on_hover` and, in the tail padding after
  it, `floats_in_order`, and grows to 800 bytes on 64-bit;
  `KUI_ABI_VERSION` is 36. Recompile; zeroed fields draw and stack a
  node as before (ADR 0051, ADR 0053).
- Rust: `MenuBar` has a third public field, `standard_menus`, and
  `InteractSpec` a `reveal_on_hover`; a struct literal of either names
  it or ends in `..Default::default()` (`MenuBar::new` and the builders
  are unchanged).

### Added

- **A row's actions shown while the row is hovered** (ADR 0051, backlog
  F183). `revealOnHover` (`reveal_on_hover` in Lua and Rust,
  `KuiSpec.reveal_on_hover` in C and Odin) draws a node only while its
  nearest ancestor that tracks the pointer — a `hoverable` row, an
  `onClick` card, a group header — has the pointer over it or anything
  inside it, holds a press begun inside it, or holds keyboard focus
  inside it, and while focus of any kind is inside the node itself. A
  file list's rename and delete, shown for the row under the pointer,
  are one row on their box, with no `onHover`, no hovered row in the
  model and no redraw of the view: the core decides it each frame, as
  it decides a `hoverBg`. Hidden is opacity 0, so the row keeps its
  layout and the controls stay in the access tree and the Tab ring;
  Tab onto the row shows them, and a `transition` fades them. With
  nothing above it that tracks the pointer the node is always drawn,
  and the new `reveal-without-anchor` warning says why.
- **A stock toggle's label cut to one line** (backlog F184).
  `<checkbox ellipsis>`, and `<radio>` and `<switch>` alike, keep the
  label on one line, ending in an ellipsis wherever the toggle is
  squeezed; `maxLines` (a clamp, with `ellipsis`) and `wrap` read on the
  label too, as on a `<text>`. The cut label is still the toggle: a
  press on it toggles, its name is the whole text, and while it is cut
  the whole text floats as its hint on hover unless a `tooltip` says
  something else. Rust `widgets::toggle_with_style`, C
  `kui_checkbox_with` / `kui_radio_with` / `kui_switch_with` (a
  `KuiTextStyle` for the label), Odin's `text` parameter on
  `kui.checkbox` / `kui.radio` / `kui.toggle`. `Core::text_cut` (and
  `Ui::text_cut`) says whether the text under a key was cut in the frame
  that finished.
- **An app's menus beside the standard ones** (ADR 0052, backlog F185).
  `<menuBar standardMenus menu={…}>` (Lua `standard_menus = true`, C
  `kui_menu_bar_standard_menus`, Rust `MenuBar::standard_menus(true)`,
  Odin `kui.menu_bar_standard_menus`) keeps the platform's standard
  menus and puts the declared ones beside them: on macOS the
  application menu, the standard Edit menu, the declared menus in order
  and the standard Window menu, with its tiling and full-screen rows. A
  declared menu's `accel` binds as on any row. Where the bar is drawn in
  the window there are no standard menus, and the flag changes nothing.
  Read back with `menuBar().standardMenus` and
  `kui_menu_bar_has_standard_menus`.
- **A canvas's floats stacked in its document's order** (ADR 0053,
  backlog F186). Floats stack in the order they open, so on a drawing
  whose items are floats an item that panned into view, or was added,
  landed over every other item, and moving an item in the tree moved
  nothing. `floatsInOrder` on the canvas (`floats_in_order` in Lua and
  Rust, `KuiSpec.floats_in_order` in C and Odin) paints every float
  under it, at any depth, in tree order, the last declared on top,
  worked out every frame: a newcomer goes to its place, two items
  swapped in the tree swap on the next frame, and the press follows.
  As a group the canvas's floats keep their place among the window's
  other floats, so a menu opened after them stays over all of them. A
  popup declared under the canvas is one of its floats and paints by
  its place; declare it outside. The hover hint of a `tooltip` is the
  core's and goes over the items.
- **A wheel's notch glides** (ADR 0050, backlog F182). A notch was a
  hard 40 px step on every platform — any mouse wheel on Windows and
  Linux, a plain mouse on macOS — while a trackpad's pixel stream
  looked smooth only because the OS smoothed it. A notch is now an
  input of its own, `InputEvent::Wheel { lines, begins }` (C
  `kui_input_wheel`, Odin `input_wheel`, Node `ctx.wheel`, in lines;
  routed and latched as `ScrollGesture` is), which the native runner
  sends for winit's `LineDelta`. On a scroll container a whole notch
  moves a target, clamped to the travel, and the drawn offset chases it
  on a critically damped spring that each notch gives the speed its own
  share eases out at: about a third of the way on the first frame,
  nine tenths in a tenth of a second, landed exactly in under two
  tenths; and a notch landing
  mid-glide adds to the speed there is, so a spun wheel speeds up
  rather than stepping and a reversed one turns. A fraction of a line
  — a precision touchpad on Windows, a high-resolution wheel — is
  applied as it comes, or joins a glide under way. A list whose glide
  is headed for its end hands the next gesture to the scroller around
  it at once. A trackpad's pixels, the scrollbar's thumb, the drag
  autoscroll, an assistive technology's scroll and a `set_scroll` take
  the content from where it is drawn. An `onScroll` handler hears the
  notch's pixels, unsmoothed. On by default; `Core::set_smooth_wheel`
  (`kui_set_smooth_wheel`, `setSmoothWheel`, Odin `set_smooth_wheel`,
  read back by `smooth_wheel`) turns it off, the launcher's
  `smooth_wheel` and Node's `smoothWheel` for every window of the app.
  The system's reduced motion makes a notch jump either way — the
  first animation the core starts that no view declared, so the first
  the core shortens for it.

### Fixed

- alpha.63 floated no hint under a box at `opacity: 0` and none in an
  exit ghost, but a leaf's hint, floated beside the leaf rather than
  inside it, took neither: a hovered leaf at 0, or at the first frame
  of an `enter` from 0, floated its hint in full over nothing, and a
  hovered leaf in a box removed with an `exit` faded its hint with the
  ghost. A leaf's hint follows the box's rule now. A hint the app
  places itself with `widgets::tooltip` is still its content and fades
  with the box it is in, ghost included (backlog RG162).
- A wrapped text's row that a scroll view's bottom edge cut through — in
  a paragraph, a log, a code block, a minified bundle of 4096 bytes or
  more with no line breaks — drew only up to its first glyph whose ink
  starts below that edge: a `.` or a `,` under the `0` before it, so the
  half-shown row ended mid-row and the rest appeared as it scrolled in.
  A text stopped drawing at its first glyph below the clip; it stops at
  its first line below it now, and the row draws every glyph that
  reaches into the view (backlog RG163, RG164).

**What you can delete.**

- An `onHover` per row and a hovered-row field in the model, kept only
  to draw a row's actions while it is hovered: `revealOnHover` on the
  actions' box (ADR 0051).
- A bare toggle beside a clickable ellipsized text of the app's, kept
  only so a long label would not wrap: `ellipsis` on the toggle (backlog
  F184).
- An application menu, an Edit menu of `copy` / `paste` roles and a
  `Window` menu, declared only so the bar kept them beside the app's
  own menu: `standardMenus` on the bar (ADR 0052).
- An `onScroll` handler on a scroll container that caught the wheel
  only to call `set_scroll` with an easing of its own, so a mouse did
  not step.
- Re-keying a canvas of floats whenever the set it draws changed, or
  re-keying an item to bring it forward, kept only so the floats would
  stack in the document's order — and the node state each re-key threw
  away: `floatsInOrder` on the canvas (ADR 0053).

## 0.1.0-alpha.63 (2026-10-22)

**What breaks.**

- A box's `tooltip` stays down while the pointer is over a descendant
  floated against something other than the box — a dialog or a toast
  on the viewport, a popover anchored to another node by key — that
  takes the pointer, where alpha.62 showed it there (backlog RG160).
- A box at `opacity: 0` floats no hint, and a departing box's ghost
  carries none: alpha.62 drew the first in full and faded the second
  with the box.
- An empty family list (`families: { x: [] }` in Node, `{ x = {} }` in
  Lua) is a token that is always sans, with `unknown-family` listing
  `[]`, where Node and Lua refused it with "a family is a name or a list
  of names"; Rust, C and Odin already took one.
- Node: a family token takes its name over from a colour or length of
  the same name, as it does in the core and every other binding, so
  `bg="$code"` with `code` declared as both gets its default and an
  `unknown-token`, where it painted the colour.
- The devtools' facts tab prints a length token as the number it is —
  `7`, `0.4` — where it printed `{:.0} px`.
- A `cells` grid of 0 rows or 0 columns is an empty grid declared under
  its key in Lua, Node and C: Lua and Node refused it and failed the
  frame, C declared no node. A click on it carries no `cell`. Node
  reads a count as Lua does, a negative or NaN one as 0, where it
  refused it.
- macOS: the windows' `icon` (`Launcher::icon`, Node's `icon` in
  `WindowOptions`, `kui_set_icon`, Odin's `set_icon`) is the app's Dock
  and ⌘Tab icon while it runs, where alpha.62 ignored it — unless the
  app is a bundle whose `Info.plist` names an icon (backlog F181).

### Added

- **The app's icon in the macOS Dock without a bundle** (backlog F181).
  A macOS window has no icon of its own, so the launcher's `icon` was
  nothing there, and an app that is no `.app` — a Node CLI run with
  `node`, a `cargo run` — showed its executable's icon in the Dock and
  ⌘Tab, Node's or the generic one. The pixels are now the app's icon
  (`NSApp.applicationIconImage`), set when the first window opens and
  kept until the process ends. A bundle whose `Info.plist` names an icon
  (`CFBundleIconFile` or `CFBundleIconName`, not empty) keeps its own,
  the way a Windows icon resource wins over the pixels: an `.icns` has a
  frame for each size, and it is what the Finder shows. The name under
  the tile and in the menu bar stays the executable's — no process call
  changes it; only a bundle does. The pixels are read as sRGB, as the
  window's are. Pass 512 or 1024 px with the margin a macOS icon leaves
  around its shape, or it is drawn edge to edge in the tile.

### Fixed

- A box's `tooltip` showed while the pointer was over anything declared
  inside the box, a dialog included: F179's test was the tree's
  subtree, so `if open { dialog(..) }` inside a component put the
  component's hint up over the dialog, painted above content declared
  before it. A descendant floated against the viewport or another node
  is placed somewhere else, and the pointer over it, or over anything
  in it, is not over the box; one floated against the box — a badge, a
  menu below a trigger — still is, as a positioned child is under a
  browser's `title`. The float has to take the pointer — a hover style
  or an event — for the pointer to be over it: a toast
  of bare text lets it through to the box, as it does a `hoverBg`
  (backlog RG160).
- A box at `opacity: 0` — laid out but not shown, or at the first frame
  of an `enter` from 0 — floated its hint in full over nothing, since
  F178 takes the hint's opacity from the box's parent. It floats none.
  And a hovered box removed with an `exit` faded its hint with the
  ghost, from the box's own opacity: the hint leaves with the hover
  now, and the ghost fades without it.
- Lua: a `cells` grid of 0 rows or 0 columns — `rows = room_h /
  line_h` in a pane shorter than one line, or `rows = 0.5` — raised
  "cells needs rows and cols" and failed the whole view, though F180
  made a division the rows that fit. It is an empty grid, zero-sized
  unless it declares a size, kept under its key so its focus and
  selection survive the pane growing back; a count left out is still
  refused. Node refused one the same way, and C declared no node at
  all, so the key and its focus went with it; both declare the empty
  grid now, and Node reads a count as Lua does — floored, a negative or
  NaN one as 0, a `null` one left out. A click on an empty grid named cell (0, 0), clamped to a
  grid that has none, which an app indexing its screen by it read past;
  it names no cell.
- Node: a family token declared under the name of a colour or length
  token left the encoder pointing `$name` at the colour's or length's
  index, so `bg="$code"` painted the orphaned colour where Lua, C and
  Rust raised `unknown-token`. The encoder drops the name as the core
  does.
- An empty family list was refused in Node and Lua and taken in Rust,
  C and Odin. It is taken everywhere: a token that is always sans,
  which `unknown-family` reports.
- The devtools printed a length token as whole px, so a `fade: 0.4`
  token an `opacity` takes read "0 px". A length token is any number
  (ADR 0049): the facts tab prints it trimmed to three decimals, with
  no unit.

**What you can delete.**

- An `.app` wrapper, or a call of your own into AppKit, kept only so an
  app run unbundled on macOS showed its own icon in the Dock.
- A `math.max(1, …)` (or `Math.max`) around a grid's row count that was
  there only so a short pane did not fail the frame.
- A `tooltip` moved off a component, or an `if open` hint guard, kept
  only so the component's hint did not show over its own dialog.

### Native verification

One pre-tag pass on 2026-10-22 over the round since alpha.62 — RG161,
F181 — on the Mac (backlog, "From the alpha.63 pre-tag pass").

**macOS** (Apple silicon, macOS 27): CI's `check` step by step — fmt,
clippy, the book, the conformance report, the C round, the Odin leg
(`gen --check`, `check`, `test`, `slots`) on dev-2026-10, CI's pin, the
Node addon under `KUI_CONFORMANCE_REQUIRED=1`, gen, the examples'
typecheck, the headless round — and `nu scripts/test.nu --node` on the
tagged tree: **2509 tests over 172 Rust suites and Node's**. `cargo
audit` clean. The windowed round with Node's: 55 examples on both
bases, 132 windows. The accessibility audit: **106 of 106**. The bench
guard against alpha.62 on `frame`, `stream`, `cells`, `long_line` and
`editing`: the guarded rows −3.7% to +2.5%, and every row of the five
within its noise. Two read-only reviewers over the diff raised three
things fixed before the tag and folded into F181's and RG161's entries
above — an empty icon name in a bundle's `Info.plist`, the Dock bitmap's
colour space, Node's reading of a negative or `null` grid count — and
one changelog line that promised more than the hover model gives. What
they raised that wants a decision is RG162.

**The C and Odin tarballs**: darwin-arm64 packed natively, bundled, and
`pack-ffi.nu verify` over both — the counter linked shared and static
and run headless, C and Odin, on the Mac. Nothing in the round touched
the packing; the Linux and Windows packs are as alpha.62's, which
GitHub's release run built. The Dock icon itself was seen on this Mac
when F181 was built; the pass did not look at it again by hand.

## 0.1.0-alpha.62 (2026-10-22)

**What breaks.**

- The hint a `tooltip` floats under a box is drawn at the opacity of the
  box's parent, not the box's own: a box at `opacity: 0.4` had a hint at
  0.4 (backlog F178).
- A box's `tooltip` shows while the pointer is over anything inside
  the box, where it showed only while the box itself was the hovered
  node: over a child button, a field or a row with a `hoverBg` it was
  hidden, and now it shows (backlog F179).
- A stock checkbox, radio, switch or slider given a `hint` hears it as
  its accessible description, as the stock button does, unless the spec
  declared one; the hint it floats is out of the access tree. It was
  content, read beside the control and, under one named from its
  content, part of its name while the pointer was over it.
- Rust: `AccessSpec` gains `hint`, so a struct literal of it without
  `..` needs the field.
- Rust: `TokenKind` gains `Family` and `TokenRef` gains `Family(u16)`,
  so a `match` over either needs the arm; `tokens::LENGTH_ROLES` is one
  more, the length space now ending with the theme's number roles
  (ADR 0049, backlog T6).
- A length token named `disabledOpacity` or `disabled_opacity` is
  refused with `reserved-token`, as a role's name is: `$disabledOpacity`
  now means the theme's. A family token named `sans`, `serif` or `mono`
  is refused the same way.
- `ctx.tokens()` and Lua's `env.tokens` carry a third table, `families`,
  so a test comparing the whole reading sees it.
- Lua: a sound row (`click_sound`, `hover_sound`, …) given a fraction is
  refused by name, where it was cut (2.5 was sound 2); an `image`'s `id`,
  a `fragment`'s `id` and `image`, an `audio`'s `src` and
  `env.set_window_size` / `env.focus_window`'s window, refused by mlua
  0.12.2 without a name, now name the prop or the call (backlog F180).
- Lua: a count, position or byte below 0 is 0 where it was refused —
  a `cells` run's or `cursor_at`'s row and col, a button's `index`,
  `env.caret_rect` / `env.caret_in_text`'s byte — as `index` on a box
  and `row_heights` always read one; a string there (`rows = "7"`),
  which mlua coerced, is refused, as `index` always refused one
  (backlog F180).
- Lua: a handle below 0, or past what its kind holds (a window id past
  a `u32`), is refused by name, where it wrapped: `env.focus_window(-1)`
  was window 4294967295 and `4294967297` was window 1. `row_heights`
  and `heights:set_len` refuse a count past 4294967295 by name.

### Added

- **`$disabledOpacity`, the theme's number by name** (ADR 0049, backlog
  T6). Every number prop has taken a `$length` token since ADR 0027 —
  `opacity`, `scale` and `transition` as much as a `width` — and the theme's
  `disabledOpacity` is now a role in that space, beside the metrics:
  `opacity={roles.disabledOpacity}` in JSX, `opacity =
  "$disabled_opacity"` in Lua, in an `enter` or a keyframe stop too,
  dims a node of the app's the way a disabled stock control is dimmed,
  with no `ctx.theme()` read and so no `ctx` passed down to the
  component. A host's `setTheme({ disabledOpacity })` is what it reads.
  Rust reads it with `ui.token_length("disabled_opacity")`, C with
  `kui_token_length`.

- **Font family tokens** (ADR 0049): a name for a face, declared as one
  family or a fallback list beside the colours and lengths —
  `setTokens({ families: { code: ['JetBrains Mono', 'Cascadia Code',
  'mono'] } })`, a `families` table in Lua's `tokens`,
  `Tokens::family("code", [...])` in Rust, `kui_tokens_families` in C,
  `tokens_families` in Odin — and referenced as `family="$code"` on a
  text, an editor or a span. The core picks the first name that is a
  stock family or an installed or loaded one when the table is declared,
  and again when the fonts change, so a family loaded later wins the list
  it lost; a list nothing matches is sans, with `unknown-family` naming
  the token. `defineTokens` types the names (a `FamilyToken` brand),
  `ctx.tokens().families` and `env.tokens.families` read each pick back,
  `ui.token_family` answers the `FontFamily`, `kui_token_family` /
  Odin's `token_family` the two `KuiTextStyle` fields, and the devtools'
  facts list each token with its list and its pick.

### Fixed

- A box's `tooltip` faded with the box: the core floats the hint as the
  box's last child, and opacity multiplies down the tree, so a dimmed
  box — a card faded to say it is unavailable, a stock button at its
  `disabledOpacity` — had a hint as faint as itself, where a leaf's
  (an `image`, a `path`, an `edit`), floated beside the leaf, was drawn
  in full. The hint now takes the opacity the box inherited and not the
  box's own, on a box and a leaf alike; a panel fading around the box
  still fades the hint with it. A stock checkbox, radio, switch or
  slider's own `hint`, which a disabled one keeps so it can say why,
  is drawn the same way. A `widgets::tooltip` the view places itself is
  content and fades as before (backlog F178).
- A box's `tooltip` went away while the pointer was over a child that
  tracks the pointer — a button, a field, a row with a `hoverBg` or an
  `onClick` — since that child, not the box, was the hovered node; it
  showed only over the box's bare background. It now shows while the
  pointer is over the box or anything inside it, as a browser's `title`
  does, and with tooltips nested the innermost one under the pointer is
  the one shown — a stock button's, toggle's or slider's own `hint` and
  a `widgets::tooltip` the view floats included. The same in every
  binding (backlog F179).
- Lua: a number where kui reads a whole one raised for a fraction under
  mlua 0.12.2 (0.12.1 cut it), and Lua 5.5's `/` always yields a float,
  so a terminal grid sized `rows = room_h / line_h` failed the frame on
  an odd window size — while `index` on a box had always read any
  number, and the sound rows cut one. The binding now reads every count,
  position, index, byte offset and line as `index` reads one, floored
  and below 0 as 0: `cells`' `rows`, `cols`, `origin_line`, `cursor_at`
  and its runs' `row`, `col`, `len` and `flags`, a button's `index`, a
  dropdown's `current` (still counted from 1), `list`'s `overscan`,
  `row_heights` and its methods, and `env.caret_rect` /
  `env.caret_in_text`'s byte. A handle — an `image`'s `id`, a
  `fragment`'s `id` and `image`, an `audio`'s `src`, the sound rows, a
  window id — is a whole number: `3.0` is handle 3, and a fraction is
  refused naming the prop or the call, since flooring one names a
  different image. A run's colours read as every colour number does. A
  `cells` grid's buffer is cut to the cells the core draws
  (`MAX_CELLS`, with its `cells-too-large` warning), where `rows *
  cols` was allocated first, so `rows = math.huge` warns rather than
  aborts; `cols = math.huge` is cut the same way, an `origin_line` is
  held where a row added to it still fits, `list`'s `overscan` to the
  row count, and `row_heights` refuses a count past 4294967295, so no
  floored `math.huge` overflows the core (backlog F180).
- Node: a token declared under a role's snake_case name
  (`disabled_opacity`, `border_strong`) is refused by the core, as the
  camelCase one is, but the encoder counted only the camelCase spelling
  as a role, so it gave the refused name an index and every token
  declared after it in the same table read the next one's slot. The
  encoder now knows both spellings, and resolves `'$border_strong'` as
  the core does.

**What you can delete.**

- An `opacity` moved off a box with a `tooltip` and onto its children,
  or a hint drawn by hand outside the box, to keep the hint readable.
- A box's `tooltip` repeated on each interactive child, or an `onHover`
  on the children feeding a hint the app floated itself, so the hint
  stayed up over them.
- A `ctx` or `env` threaded through a component only to read
  `theme().disabledOpacity`: `roles.disabledOpacity` is a module-level
  constant.
- A constant holding a family name, and the "is it installed?" check
  that picked between two: a family token's list is that check.
- A `math.floor` (or `//`) around a Lua count, position or byte offset
  that was there only because the binding raised on a fraction: `cells
  { rows = room_h / line_h }` is the rows that fit.

### Native verification

One pre-tag pass on 2026-10-22 over the round since alpha.61 — F178,
F179, T6 with ADR 0049, F180 — on the Mac (backlog, "From the alpha.62
pre-tag pass").

**macOS** (Apple silicon, macOS 27): CI's `check` step by step — fmt,
clippy, the book, the conformance report, the C round, the Odin leg
(`gen --check`, `check`, `test`, `slots`) on dev-2026-10, CI's pin, the
Node addon under `KUI_CONFORMANCE_REQUIRED=1`, gen, the examples'
typecheck, the headless round — and `nu scripts/test.nu --node` on the
tagged tree: **2497 tests over 172 Rust suites and Node's**. `cargo
audit` clean. The
windowed round with Node's: 55 examples on both bases, 132 windows.
The accessibility audit: **106 of 106**. The bench guard against
alpha.61 on `frame`, `stream`, `cells`, `long_line` and `editing`: the
guarded rows −3.2% to +0.9%, and every row of the five within its
noise. Three read-only reviewers over the diff found RG159, fixed
before the tag: a second hint over a stock control inside a tooltip
box, the toggles' and the slider's hint dimmed with them, four
overflows and a wrapping handle F180 opened in kui-lua, a snake_case
role counted as a token by Node's encoder, a family table re-picked
every frame. What they raised that wants a decision is RG160.

**The C and Odin tarballs**: darwin-arm64 packed natively, bundled, and
`pack-ffi.nu verify` over both — the counter linked shared and static
and run headless, C and Odin, on the Mac. Nothing in the round touched
the packing; the Linux and Windows packs are as alpha.61's, which
GitHub's release run built.

## 0.1.0-alpha.61 (2026-10-22)

**What breaks.**

- Node: a door taking a count or an index throws, naming the door and
  the argument, for a number that is not a whole number from 0 to
  4294967295, where napi wrapped it (-1 was 4294967295, 2.5 was 2):
  `RowHeights` (`new`, `setLen`, `set`, `get`, `measured`, `offsetOf`),
  `mouse`'s clicks and button code, `preedit`'s cursor, `windowClosed`,
  `windowDismissed`, `animationAt`'s loops, `addImage` and
  `updateImage`'s size, `activateMenuItem`, `activateMenuPath`,
  `activateMenuBarItem`, `activateMenuBarPath`, `setWindowSize` and
  `focusWindow` (backlog RG139). The types are unchanged.
- `UiEvent::data()`, and so `Event::read` and the event views, is `None`
  for an event the core wrote no data for (a plain click, a submit,
  `fonts`), where an app's own field named like the event read as the
  event's data. A grid's or a line row's click into a message with its
  own `click` field replaces it and raises `message-field-taken`, where
  the pointer's fields were added to the app's map (backlog RG157).
- A `cells` grid past `cells::MAX_CELLS` (4,194,304 cells, a 2048 x 2048
  screen) draws the rows that fit and raises the new `cells-too-large`
  warning, where `rows * cols` overflowed or asked the allocator for
  gigabytes (backlog RG139). `WarningCode` gains the code.
- Node: a non-string where a string prop goes throws `<box> title must
  be a string, got 42` (the element and the prop), where it threw
  TextEncoder's `The "src" argument must be of type string`; a number
  `key` on the plain-call API is its spelling, as `jsx()` already made
  one. `addImage` / `updateImage` of 0x0 say an image is at least 1x1,
  and `play` refuses a `volume` or `fadeIn` that is not finite by name,
  where it failed in serde. `KuiWindow`'s `close` and `useWindow` return
  through `Result` (the types are unchanged) (backlog RG139).
- A `secure` editor a frame does not declare is forgotten on that frame,
  its text zeroed — one shown again with a "show password" toggle too,
  since it was declared `secure`; it was kept for the undeclared-editor budget, as an
  ordinary draft is, so a sign-in form closed and opened again showed
  the password, and kui held it for the life of the app (backlog F177,
  from carnet). A form opened again starts empty, or at the `value` it
  declares; a field hidden for a frame mid-typing loses what was typed
  unless the app declares it as `value`.
- C: ABI 34. `KUI_TURNED`, OR'd into `kui_path`'s and `kui_path_d`'s
  `fill_rule`, says the path turns, so a `rotate` of 0 is declared and
  keeps the swept box (backlog RG121); no struct moved, and a library
  before 34 would read the bit as a fill rule. Recompile. Odin: `c.TURNED`
  in the `Fill_Rule` argument, `Fill_Rule(u32(Fill_Rule.Evenodd) |
  c.TURNED)`.
- `line::Cut` gains `width`, the stroke width it was cut for, which the
  pixel floor reads its seen gaps by.
- The Odin binding's `kui/c` takes the context as `^Ctx` in the ten
  functions it typed `[^]Ctx` (a context followed by a `size_t`): the
  same ABI, the C type it claims to keep (backlog RG145).
- A primary press on a node that does nothing with a press — one that
  only declares `hoverBg`, `hoverable`, `onHover` or a `cursor` — starts
  the drag of the nearest enclosing `onDrag` node, where it started
  nothing (backlog F176).
- The Odin tarball's libraries moved from `lib/<platform>/` to
  `kui/c/lib/<platform>/`, and its `kui_c.odin` links them itself,
  statically unless `-define:KUI_SHARED=true`: an alpha.60 build line
  that passed `-extra-linker-flags:"-L$ODIN_PACK/lib/…"` names a
  directory that is gone.
- Lua: an `on_event` that already declared a second parameter, or takes
  `...`, receives the envelope there (backlog RG157).

A message named its own fields, and the core read one of them as its
own whenever the event had nothing to write there; the flag that says
the core wrote the field is what both readings now ask.

### Added

- **The Odin tarball links itself**: `kui-odin-<version>.tar.gz` carries
  `libkui_ffi` inside the package, under `kui/c/lib/<platform>/`, and
  its `kui_c.odin` names those libraries per platform the way
  `vendor:raylib` does — the static library and the system libraries its
  `link.txt` lists by default, or the shared one with
  `-define:KUI_SHARED=true` — so `import kui "kui:kui"` with a collection
  is all a program needs, no `-extra-linker-flags`. A Windows plugin
  (`-define:KUI_PLUGIN=true`) links the shared one, importing from the
  host's `kui_ffi.dll`. The repository's
  `kui_c.odin` keeps `KUI_LIB`; the bundle writes the pack's block
  between markers the generator emits. `pack-ffi.nu verify` builds the
  Odin counter that way, static with no flags at all.

- **Lua's `on_event(ev, at)`**: the envelope alone as a second argument
  — `at.event`, `at.node_key`, `at.window`, and `at.slot` and `at.from`
  where they apply — which no message can hide. In the first argument a
  message's own `event` or `node_key` field keeps its value, so a
  message with either hid what happened and where (backlog RG157). A
  script that names one argument is unchanged; one that names two, or
  takes `...`, gets the envelope there.

### Fixed

- A press on a child of an `onDrag` node drags it when the child does
  nothing with a press itself: a scrollbar built of boxes, whose thumb
  only lights up under the pointer, dragged from its empty track and
  never by its thumb — the thumb's hover region took the press and
  dropped it. The press now goes to the nearest enclosing `onDrag`
  node, as a secondary press does to the enclosing menu: past nodes
  that do nothing with a press, stopping at the first that does (a
  clickable chip, a key sink, window chrome, a slider) and at the
  modal boundary, passing a disabled one, and not out of the selection
  scope it is in. A child with a claim of its own keeps it. The drag's
  `parent` is the owner's parent (backlog F176, from kawoosh's hex
  pane). **What you can delete:** a drag handler declared again on
  every lit child of a draggable node, or a bar's thumb drawn as a
  sibling float so that the press finds the track.
- A screen reader's "replace selected text" on an editor goes in as
  typing does: a control, a line break in a single-line field and the
  bidi paragraph ends are dropped. It went in as it came, so one request
  carrying `\u{1e}` before right-to-left text panicked the shaper and
  ended the app, release builds included, and a value the app declared
  back from the `change` it reported was not the text the editor held
  (backlog FZ11, found by the `events` fuzz target).
- A dismissal a driver reports for a window the user closed, while the
  app still declares it, posts nothing; it delivered the entry's
  `onDismiss` about a window that was gone (backlog RG157).
- `scripts/pack-ffi.nu`'s Docker legs (backlog RG145): each checkout
  builds in a target volume of its own, so a worktree no longer packs
  another's newer build beside its own `kui.h`; the staged copies are
  handed back to the caller, which a rootful Docker left root's; a
  `--no-runner` build, which fetches no Windows import libraries, packs
  without them; `SHA256SUMS` lists this version's tarballs only; and a
  `--no-runner` pack is `kui-ffi-<version>-<platform>-no-runner`, where it
  overwrote the full pack of its platform. The volume before this one,
  `kui-pack-target`, can go.
- A Node message nested past serde's 128 levels arrives; past
  `MESSAGE_DEPTH`, 1024, the frame's error says how deep it was, where
  every message deeper than 128 failed the frame with "recursion limit
  exceeded" (backlog RG139). Lua and C take any depth.
- A `KuiWindow`'s `onTeardown` that calls the window back: its own doors
  (`pump`, `pumpUntil`, `close`, `useWindow`, `onTeardown`) throw, and a
  shared door reads a blank core, where each was a second `&mut` of the
  window the pump in flight holds (backlog RG139).
- A custom editor's `KuiAccessNode.caret`, `selection_start` and
  `selection_end` are places in `value`, the start of a character an
  app's byte offset falls inside, as `focus` and `anchor` were since
  RG135: a host slicing `value` at them panicked (backlog RG145).
- `docs/BACKLOG.md`'s ten links into the archive that landed on no
  heading land (each dropped an underscore, or a hyphen around `⌫`), and
  the docs test now checks every such link, the uniqueness of a heading
  marked `` `x` ``, and that every id `howto.md` cites exists (backlog
  RG145).
- The Odin generator matches an event's payload fields by their exact
  name, as the decoder reads them, so a camelCase key fails generation
  where it decoded as zero; `doors.odin`'s `image_pixels` promises its
  bytes until the next call, as kui.h does; and `run.odin` says a loop of
  your own frees `context.temp_allocator` once a turn (backlog RG145).
- A C spinner — `kui_path` with a `rotate` passing through exactly 0 and
  no pivot — keeps its box at 0 with `KUI_TURNED`; it had the tight box
  for that frame, where JSX's and Lua's turn of 0 has the swept square
  (backlog RG121).
- A `cells` grid's combining mark from a fallback face sits at its own
  bearing, over the base before it, as the family's own marks do; it was
  centred, half a cell to the right (backlog RG121). A grid that composes
  a base and its marks into one cell is still not built.
- A sub-pixel dash pattern reads solid or dashed the same on both sides
  of the width that closes one of its gaps: the pixel floor counts a
  period's gaps by how much of each is seen, so `0.2, 0.2, 1, 0.5` is
  dashed at 0.399 px as at 0.4, where it was solid (backlog RG121).
- A warm `cells` grid draws 13% faster (200x50: 78 µs to 68 on an M3
  Pro, streaming 85 to 74): since alpha.41 its emitter had been inlined
  into the paint loop, which ran the same work 29% slower than it does as
  a function of its own (backlog RG126).

### Changed

- `benches/editing.rs` is a divan bench, so `KUI_BENCH=editing
  scripts/bench-check.nu` compares it: a whole keystroke at 500, 10k and
  100k lines, the 100k one into an editor that declares `on_change`, and
  the 100k edit alone, the half RG156 tripled (backlog RG157). It printed a
  table of its own, which the guard could not read.
- Dependencies, each release two weeks old or more: `mlua` 0.12.2 (with
  `mlua-sys` 0.13), `glam`, `objc2`, `either`, `arbitrary`,
  `async-recursion`, `lazy_static`, `napi-build` and `toml_datetime` in
  the lockfile; the examples' `@types/node` 26.6.4; Nushell 0.116.1 in
  CI and the release scripts; zig 0.17.0 under cargo-zigbuild for the
  Linux prebuilds and packs, whose libraries still need only glibc 2.28
  (checked on Debian 10). The 29 newer lockfile updates, napi 3.15 among
  them, and cargo-zigbuild 0.24 wait for their two weeks; `x11rb` 0.14
  waits for winit and arboard, which pin 0.13. `cargo audit` and
  `npm audit` are clean.

**What you can delete.**

- A `setEditText(key, '')` on a password field a form stops showing —
  carnet's `forget_passwords` — to wipe kui's copy: kui forgets the field
  and zeroes its text itself. Clearing the app's own copy is still the
  app's.
- The Odin build's `-extra-linker-flags` (the `-L` to the pack's
  `lib/<platform>/` and its `link.txt` line): the pack's `kui_c.odin`
  links the library itself.
- A Lua message's field renamed away from `event` or `node_key` so
  `on_event` could see the envelope's: `at` has it.
- A Node guard that checked a row count or an index was a whole,
  non-negative number before handing it to kui: the door refuses one.

### Native verification

One pre-tag pass on 2026-10-22 over the round since alpha.60 — F176,
F177, RG121, RG126, RG139, RG145, RG157, FZ11, the C and Odin
tarballs, the dependencies — on the Mac (backlog, "From the alpha.61
pre-tag pass").

**macOS** (Apple silicon, macOS 27): CI's `check` step by step — fmt,
clippy, the book, the conformance report, the C round, the Odin leg
(`gen --check`, `check`, `test`, `slots`) on dev-2026-10, CI's pin, the
Node addon under `KUI_CONFORMANCE_REQUIRED=1`, gen, the examples'
typecheck, the headless round — and `nu scripts/test.nu --node` on the
tagged tree: **2471 tests over 170 Rust suites and Node's**. `cargo audit` clean. The windowed round
with Node's: 55 examples on both bases, 132 windows. The accessibility
audit: **106 of 106**. The bench guard against alpha.60: the guarded
rows −1.0% to +2.4%, stream and long_line flat, a warm cell grid
−11.8% (RG126); `benches/editing.rs`, divan now but not in alpha.60,
compared by hand — a 100 000-line edit 1.40 ms against alpha.60's
1.395. Three read-only reviewers over the diff found RG158, fixed
before the tag: F176's walk out of the selection scope a press on
selectable text is in, F177's shown password, two unzeroed copies of a
password, a Windows Odin plugin linking kui statically.

**The C and Odin tarballs**: darwin-arm64 packed natively, bundled with
the link block RG158 changed, and `pack-ffi.nu verify` over both — the
counter linked shared and static and run `--headless`, C and Odin, on
the Mac. The Linux packs and the Docker legs are as alpha.60's and
GitHub's workflow_dispatch run of `ddb16eb5`, which built all four
platforms, Windows's first pack among them; no Windows plugin has been
built from a pack.

## 0.1.0-alpha.60 (2026-10-22)

**What breaks.**

- Every node event's payload: it is the message the handler declared,
  with the event's data in one field named for the event —
  `{...msg, drag: {phase, x, y, dx, dy, parent}}` — where it was the
  core's `{kind: "drag", …, tag}` (ADR 0047, backlog F174). The event's
  kind is on the envelope: `ev.event` in every binding, `UiEvent::event`
  in Rust, `KuiEvent.event` in C.
- `UiEvent::on`, `UiEvent::kind`, `UiEvent::tag` and `UiEvent::tagged`
  are gone: `UiEvent::declared` builds every event, and `ev.event`,
  `ev.data()`, `ev.message()` and the views read one.
- An editor reports only what it declares: `onChange` (`change: {text}`),
  `onSubmit`, `onCancel`, `onBoundary`. The `changed`, `submit` and
  `cancel` kinds are gone; an `edit` that declares none hears nothing.
- The `access` event is gone. A slider without `onChange` does not
  move, from a key or a reader (`slider-without-handler`); a custom
  editor's setValue is its `onChange` message, replaceSelectedText a
  `text` and setTextSelection a `select` on its `onKey` sink.
- A menu's choice reaches only a node that declares `onMenu`
  (`menu: {role, item}`); the stock right-click menu posts nothing; the
  menu bar's arrives as the `menuBar` element's own `onMenu`. The stock
  select and text input declare a `Null` message by default and keep
  reporting.
- Every node event's payload is a map. A message that is not one — a
  bare `on_click("save")` — arrives as `{tag: "save"}`, with or without
  the event's data, and a `Null` one with no data as `{}`, where both
  arrived bare. `ev.message::<String>()` reads a string message back.
  A click's `cell`, `line`, `byte`, `inside` and `clicks` moved under
  `click`.
- Node: `CoreMsg` is gone, and so is the `RootMsg` that briefly took
  its place: `update` takes the app's own messages. The `*Msg` types
  name a message carrying an event (`DragMsg<T> = T & WithDrag`), so
  `EditMsg`, `AccessMsg` and `FontsMsg` are gone and `ChangeMsg` is
  `SliderChangeMsg`. `UiEvent.event` can be `'dispatch'`: a message the
  app or an effect handler dispatched, which had no envelope at all.
- Lua: `ev.event` names the event; `on_event` gets a table always, as
  every payload is a map. A message's own `event` or `window` field wins
  over the envelope's, and on a `window` event `ev.window` is its data,
  `{phase, id?}`.
- C: ABI 33 — `KuiSpec` gains `on_submit`, `on_cancel`, `on_boundary`,
  `on_menu`, `on_selection_range`; `KuiEvent` gains `event`. Plain
  `kui_menu_bar` and `kui_window_declare` post nothing — a bar's choice
  needs `kui_menu_bar_with`, a window's `opened`/`closed` and dismissal
  `kui_window_declare_with` — and `kui_open_menu` on a node with no
  `on_menu` posts nothing. `kui_text_input` reports a submit and a
  cancel as `{}`, told apart by `KuiEvent.event`.
  Odin: the `*_Event` structs lose `tag`; `Changed_Event`,
  `Access_Event`, `Submit_Event`, `Cancel_Event` and `Change_Event` are
  gone. The frame protocol is v24.
- There are no root events (ADR 0048, backlog F175). A window's own
  events — `resize`, its keyboard focus (`window` `focused` /
  `blurred`), `system`, `fonts`, `modifiers` and the OS's `open` —
  arrive only as the messages its root declares in a `windowEvents`
  group, with the data in the field named for the event; a declared
  window's `opened` / `closed` only as its `windows` entry's `onWindow`
  (`window: {phase, id}`), a popup's dismissal as its `onDismiss`
  (`dismiss: {reason, id}`), and a menu bar's choice as its `onMenu`.
  Nothing that declares none hears it. `name` is gone from the `window`
  and `dismiss` data. `NodeSpec::windows` holds a `WindowLifecycle`
  beside each entry's name and config.
- Node: `activateMenuBarItem` / `activateMenuBarPath` answer `false`
  for a bar that declares no `onMenu`, since nothing is posted; the
  row's standard role is still performed.
- `KeyMods::to_value` and `KeyPress::to_value` no longer write `kind`.
- `testing::kinds` (the `conformance` feature) is the envelope's event
  name, where it was the payload's `kind`; `testing::messages` is the old
  reading.
- `schema::Kind::Tag` is `Kind::Handler`.
- Rust: `EventSpec`'s eighteen handler fields are rows of a sorted list,
  read with an accessor of the same name (`spec.events().on_click()`,
  an `Option<&Value>`) and written with `set` / `put` / `take` by
  `spec::Handler`. A node with a click handler allocates 112 bytes for
  the group, where it was 592.

Matching an editor's `changed` meant `ev.key === ctx.keyOf('title')`,
and a list of editors could not be matched at all: key labels are unique
among siblings, so `keyOf('name')` answered the first row and warned. The
model reached an editor only through `initial` and `setEditText`, which
clears undo. Every node event had the same shape problem: `update`
matched the core's `kind`, then the `tag` inside it, the app registered
messages that only ever arrived as tags, and an app's `Msg::Key` was the
core's `{kind: "key"}` (backlog F148). Now a node declares the message
its events arrive as, and the core writes its data into it under the
event's name — one field per event, so each field has one type. A
message that is not a map is kept under `tag` beside the field; a
`Null` one is the field alone; a field the message already had is the
core's (`message-field-taken`). The events with no node to declare a
handler on went the same way (ADR 0048): a window declares the messages
its own events arrive as on its root, in one boxed `windowEvents` group
rather than six rows on every node; its lifecycle on the `windows` entry
that made it exist, where a closed window's message outlives its root;
the menu bar on its element. `kind` is the app's alone, and Node's
`update` no longer takes a union of core messages beside the app's.

### Added

- **An editor's `value`** (`EditOptions::value`, `Ui::text_edit_value`,
  `widgets::text_input_value`; `<edit value>`, Lua `value =`, C
  `KUI_EDIT_VALUE`, Odin `Edit_Flag.Value`): the text declared every
  frame. Where it differs from the editor's, the editor is brought to it
  as a splice — a caret before it stays, one after it moves, one inside
  it goes to its end — and a correction made in the `update` the edit
  reached joins that edit's undo step, so an uppercasing filter undoes
  with the keystroke. It waits for a composition to commit. An equal
  value costs a compare: 1.25 µs a frame for a field, the same as a
  seed. `edit-value-without-handler` and `edit-value-and-initial` say
  when it is half declared.
- **`onSubmit`, `onCancel`, `onBoundary`, `onMenu` and
  `onSelectionRange`**, and `onChange` on an editor.
- **The stock input with messages** (`widgets::text_input_with`,
  `InputMessages`; `<input value onChange onSubmit onCancel>`, Lua
  `input { value=, on_change= }`, C `kui_text_input_with`, Odin
  `text_input_with`).
- **`kui::Event`** with `UiEvent::split::<Msg>()` and `UiEvent::read()`:
  the message and the event, one variant per kind with its data, for one
  `match` — `(Some(Msg::Pan), Event::Drag(d)) => …`. `EventKind` is
  `UiEvent::event`'s type, `UiEvent::is(kind)` asks it, and `EventField`
  names the field each event's data rides under.
- `widgets::select_items_with`, the stock select over menu items with
  its messages.
- **The derive's arriving twin**: `#[message(on = drag)]` on a variant
  makes `#[derive(Message)]` emit `MsgIn` too, whose variant carries the
  event's data (`MsgIn::Pan { drag }`), for `ev.message::<MsgIn>()` in a
  flat `match`. `on = change` is an editor's change and `on =
  slider_change` a slider's; `incoming = "Name"` and `incoming_derive(…)`
  name and derive the twin. A message declared on the wrong handler
  fails to decode.
- **Helper types for a message carrying an event**: TypeScript's
  `WithDrag`, `WithEditChange`, … (interfaces, so `extends` works),
  `DragMsg<T>`, `EditChangeMsg<T>`, `KeySinkMsg<T>` and `HandlerMsg<W>`,
  which types every handler prop — a message without the event's field
  does not fit `onDrag`; Lua's `kui.With…` classes; Odin's `With_*`
  structs, embedded with `using _:`.
- **`labelOf(key)`** in every binding (`kui_label_of`, Odin `label_of`,
  Node `labelOf`, Lua `env.label_of`): the key label a key was opened
  under, the reverse of `keyOf`.
- **`reply`**, the kind an extension's message to whoever placed its
  slot arrives as.
- **A window's `windowEvents`** (ADR 0048): `onResize`, `onFocus`,
  `onSystemSettingsChange`, `onFonts`, `onModifiers` and `onOpen`, the
  messages the window's own events arrive as, declared on its root —
  Rust `ui.window_events(WindowEvents::default().on_resize(Msg::Fit))`,
  JSX `windowEvents={{…}}` on the root box, Lua `window_events = {…}`
  on the root table, C `kui_root_with` with a `KuiWindowEvents`, Odin
  `root_with`. Read from the last finished frame; what arrives before
  the first (a launch's documents) is held for it. On any other node
  the group is ignored and `window-events-off-root` says so. The last
  group declared wins, and whoever declared it hears it: a Lua slot's
  `window_events` reaches the script, not the host.
- **A window's lifecycle messages**: `onWindow` and `onDismiss` on a
  `windows` entry — Rust `ui.window_with(name, config,
  WindowLifecycle::default().on_window(..).on_dismiss(..))`
  (`Core::declare_window_with`), C `kui_window_declare_with`, Odin
  `window_declare_with`. Read on every declaration, where the config is
  read on the opening edge alone, and a closed window's are the last
  ones declared. The event's origin is the declaring frontend's, so an
  extension hears the window it declared.
- **The menu bar's `onMenu`**: `MenuBar::on_menu`, `<menuBar onMenu>`,
  Lua `menu_bar { on_menu = }`, C `kui_menu_bar_with`, Odin
  `menu_bar_with`.
- The derive's twin takes `on = resize`, `window`, `system`, `modifiers`
  and `open`, with `event::Open` and `SystemEnv::to_value` /
  `from_value` beside them.
- The scene corpus's `window-events` scene, with two steps,
  `viewport W H` and `focused 0|1`, in every adapter.
- `GLOSSARY.md`, the words the API is described in.
- **C and Odin tarballs on every GitHub release**: `kui-c-<version>.tar.gz`
  (`include/kui.h`) and `kui-odin-<version>.tar.gz` (the `kui` package),
  each with `libkui_ffi` for linux-x64 and linux-arm64 (glibc 2.28 or
  newer), darwin-arm64 and win32-x64, shared and static, a directory a
  kind (`lib/<platform>/shared`, `lib/<platform>/static` with
  `link.txt`), a README.md with the link lines and `SHA256SUMS`. Each
  platform is built beside its Node prebuild, from the same target dir,
  and, off Windows, the C and Odin counters are linked against both
  kinds and run before it is packed (`scripts/pack-ffi.nu bundle` and
  `verify`; `pack-ffi.nu --zig` is the Linux runners' build).

### Fixed

- `docs/props.md` lists `Launcher::app_id`'s doors. alpha.59 added the
  `schema::DOORS` row without regenerating the page, which is what
  failed the tag's Forgejo `check`, so that release reached crates.io
  and npmjs but not the drydock9 cargo registry; this one carries the
  drydock9 copy again.
- `<cells originLine>` type-checks in JSX. The encoder has sent it
  since protocol v6, but the element's type left it out, so a
  TypeScript app could not declare where its screen sits without a
  cast.
- A click in a `selectable` scope selects nothing when the layout moves
  under the held button — a pane a strip scrolls into view as the press
  focuses it. A drag-select placed its live end on the release, on
  every frame the scroller moved and on every move, wherever the
  pointer was over the text that slid there; it now waits until the
  pointer leaves the click slop of the press, as a caret drag has since
  alpha.57 (backlog F173, from kawoosh's terminal).
- A reader's increment or decrement nudges only a `slider` role; it
  reached any enabled node.
- A reader's `setValue` on an editor is a recorded edit: it undoes in one
  step as typing does, keeps the history before it, and an app's
  correction to it joins that step (it cleared undo, as `setEditText`
  does).
- An editor's caret, as `caret_and_selection` and the access tree report
  it, is a place in its committed text while a composition is under way;
  it counted the composition, so one character composed into an empty
  field put the caret at 1 of "" (backlog FZ9, found by the new `events`
  fuzz target, which drives every handler row with the fuzzer's own
  messages and an editor whose `value` follows its `change`).
- A path with an arc whose radius, scaled up to reach its end, is past
  what an f32 holds draws nothing on a display under a scale of 1, where
  it drew round an ellipse some 2e38 px across: an overflow panic in a
  debug build, undefined behaviour in the rasterizer in a release one.
  The rasterizer now measures the outline it is handed, not only the
  logical one (backlog FZ10, found by the `path` fuzz target).
- A choice from the platform's menu bar (`activate_menu_bar_item` /
  `_path`, the macOS bar) says which window it was made in and which
  slot drew the bar, is listed in the devtools' events tab, and raises
  `message-field-taken`, as a choice from the drawn bar always did.

### Changed

- **`scripts/release-prep.nu <version>`** is a release's first step, on
  any machine: a clean tree, the last Forgejo `check` on main or a tag
  green, every generator CI diffs rerun with nothing coming back
  changed (`cargo fmt`, the book's Examples part, `npm run gen`,
  `odin.nu gen`), then `set-version.nu`. Each step can be left out by
  name (`--skip odin,ci`), and every one runs before it reports, so one
  run lists all that is wrong. `docs/releasing.md` has it.
- CI checks the Odin binding on Odin dev-2026-10, where it pinned
  dev-2026-09; the binding still builds on dev-2026-09.
- GitHub's release workflow uses `actions/checkout@v7`, `cache@v6`,
  `upload-artifact@v7` and `download-artifact@v8`, which run on Node 24:
  the v4s ran on Node 20, which GitHub deprecated and warned about on
  every job.

**What you can delete.**

- `ev.key === ctx.keyOf('title')` and the table of hex keys a list of
  fields needed to tell its rows apart: the message says which field.
- `editText(ev.key)` after an edit: the `change` carries the text.
- `setEditText` to clear or reformat a field the model holds: declare
  the model's text as its `value`.
- The second `switch` on `msg.tag` inside `case 'drag'`, and the
  messages registered only so a JSX prop would type-check.
- F148's tag-first read, and any guard against an app kind that matched
  a core one.
- An app-side `access` handler that turned a slider's nudge into a
  value: the slider's `onChange` does the arithmetic.

### Native verification

Two pre-tag passes on 2026-10-22 over the round since alpha.59 — ADRs
0047 and 0048, F173 — one on the Windows machine and under WSL, one on
the Mac (backlog, "From the alpha.60 pre-tag pass").

**Windows and Linux** (WSL): CI's `check` in order on both, the windowed
round on both, the bench guard against alpha.59 clean, and ten minutes
of every fuzz target at once on eight cores, which found FZ10 (an arc
past an f32 at one scale) in the `path` target; the other five ran
clean. It also found CI's Odin step red on main: a spec literal
dev-2026-09 refuses, which the Mac's newer local Odin had taken.

**macOS** (Apple silicon, macOS 27): CI's `check` step by step —
fmt, clippy, the book, the conformance report, the C round, the Node
addon under `KUI_CONFORMANCE_REQUIRED=1`, gen, the examples' typecheck,
the headless round — and `nu scripts/test.nu --node` on the tagged
tree: **2449 tests over 170 Rust suites and Node's**. The Odin leg
(`gen --check`, `check`, `test`, `slots`) on the dev-2026-10 release CI
now pins. `cargo audit` clean. The windowed round with Node's: 55
examples on both bases, 132 windows. The accessibility audit: **106 of
106**, after the example's own tag reading was fixed (it read 95). The
bench guard against alpha.59: the guarded rows −5.2% to +0.2%, stream,
cells and long_line flat; `benches/editing.rs`, which the guard does not
read, compared by hand — a keystroke into 100 000 lines 1.38–1.40 ms
against alpha.59's 1.37–1.40, after RG156 (4.3 ms before it). Three
read-only reviewers over the diff found the rest of RG156.

**The C and Odin tarballs**, before the release workflow first builds
them: linux-x64 and linux-arm64 packed in Docker through cargo-zigbuild
(and linux-arm64 again through `pack-ffi.nu --zig`, the Linux runners'
command, to the same link line and glibc floor), darwin-arm64 natively,
bundled, and `pack-ffi.nu verify` over both tarballs — the counter linked
shared and static and run `--headless` — on the Mac (C and Odin) and in
clean Debian 12 containers for both Linux arches (C and Odin, Odin
dev-2026-10); the C counter, both kinds, also on Debian 10, whose glibc
is the 2.28 the packs claim. 113 MB a tarball for three platforms;
win32-x64 is built only on the release workflow's Windows runner and
is not linked there, so its pack has run nowhere yet.

## 0.1.0-alpha.59 (2026-10-19)

**What breaks.** Nothing: one launcher option added in every binding
that has a launcher.

kawoosh installs on Linux with a `kawoosh.desktop` whose `Icon=kawoosh`
the launcher showed, while the running window kept the compositor's
generic icon: a Wayland compositor matches a window to its `.desktop`
file by the window's `app_id`, and a kui window had none — winit sets
one only from a name kui never gave it (backlog F172, from kawoosh's
Linux install). X11 matched by `WM_CLASS`, which winit fills with the
executable's name, so it worked there only while the binary was called
what the file said.

### Added

- **`Launcher::app_id`** (backlog F172; C `kui_set_app_id`, Odin's
  `set_app_id`, Node `appId` in `WindowOptions`): the name the Linux
  desktop knows every window of the app by — the Wayland `app_id`, and
  both halves of the X11 `WM_CLASS`. A compositor shows the window
  with the `Icon=` of `<id>.desktop`, a window manager of the file whose
  `StartupWMClass` is the id, and both group the window under it.
  Nothing on Windows or macOS.

**What you can delete.**

- A Linux window shown with the generic icon, or ungrouped from its
  launcher in the dock, because Wayland had no `app_id` to match to the
  installed `.desktop` file; and on X11 a `StartupWMClass` kept equal to
  whatever the executable happened to be named, or a `RESOURCE_NAME`
  exported for it.

### Native verification

The pre-tag pass on 2026-10-19, over the round since alpha.58 — F172's
app id — on the Windows machine and under WSL. It found one thing, of
the release's own making: howto's link to this section, re-anchored to
the dated heading before the heading was dated, failed the docs test
on both; it passes with the date.

**Windows**, fmt and clippy are clean; `nu scripts/test.nu --node`:
**2358 tests over 166 Rust suites and Node's**, the one failure the
anchor above. `odin.nu gen --check` current, `odin.nu check` over every
Odin example.

**Linux** (WSL, Ubuntu 24.04). fmt and clippy are clean; `cargo test
--workspace`: **2126 tests over 164 suites**, the same one failure.
Under WSLg a kawoosh built against this tree sends
`xdg_toplevel.set_app_id("kawoosh")`, and on X11 its window's
`WM_CLASS` is `"kawoosh", "kawoosh"`, from a binary renamed
`notkawoosh` too. No windowed round over the examples, no `cargo
audit`, no fuzz round and no Mac ran this round: F172 is one window
attribute.

## 0.1.0-alpha.58 (2026-10-18)

**What breaks.** Nothing: two calls added in every binding.

kawoosh places its bar caret over a wrapped row where kui laid the
byte out. On the frame a letter is typed the row's text is new to kui,
and `caret_rect` answers from the layout of the text the row drew last
frame: on the row's first visual line kawoosh measured the text before
the byte instead, and on every later one it stood a letter behind for
that frame (backlog F171, from kawoosh's wrapped caret). Nothing in kui
could say where a byte of a text no node has drawn lands once wrapped
at a width. Now it can.

### Added

- **`Ui::caret_in_text` and `Ui::caret_in_rich_text`** (backlog F171;
  C `kui_caret_in_text` / `kui_caret_in_rich_text`, Odin's
  `caret_in_text` / `caret_in_rich_text`, Node `ctx.caretInText`, Lua
  `env.caret_in_text`, the last two taking spans too): where byte `byte`
  of a text lands once laid out at `max_w`, without a node — the rect
  `caret_rect` gives when a text node with that content and style is
  laid out at that width, from the text's own top-left. Shaped and
  wrapped through the text cache, as `measure_text` is, so asking and
  then drawing shapes once, and a run a node drew keeps its rows.

**What you can delete.**

- A caret on the frame its text is new placed by the layout of the text
  the node drew the frame before — a letter behind on a wrapped text's
  later lines — or by the width of the text before the byte measured
  unwrapped, which is right on the first line and past the box on every
  other; and a guess at the fold (`x / width` rows down) where no layout
  existed yet.

### Native verification

The pre-tag pass on 2026-10-18, over the round since alpha.57 — F171's
caret without a node — on the Windows machine and under WSL. It found
one thing to change: Odin's generated layer, written by hand for F171 on
a machine with no Odin compiler, had `Caret_Rect` where the generator
does not put it, so `odin.nu gen --check` failed; regenerated, the same
declarations in the generator's order.

**Windows**, rustc 1.99.1. fmt and clippy are clean; `cargo test
--workspace`: **2129 tests over 165 suites**, 0 failed. The C round
passes, and so do the scenes through C (59); Node's tests under
`KUI_CONFORMANCE_REQUIRED=1`, **230 of 231** with the one skip Windows
has had before; the examples' typecheck and the headless round. The
windowed round: **all 55 examples**, Node's included, drew 120 frames
on each base and exited cleanly.

**Linux** (WSL, Ubuntu 24.04), on the release's code. fmt and clippy
are clean; `cargo test --workspace`: **2124 tests over 164 suites**, 0
failed; `cargo audit --deny warnings` clean over 451 crates. The C
round and the scenes through C; Odin's four steps, after the
regeneration above; Node **231 of 231** with the corpus required; gen
with no diff, the typecheck, the headless round, the book's examples
current. No fuzz round ran — F171 is a query beside the text cache and
no fuzz target calls it — and no Mac ran this round.

## 0.1.0-alpha.57 (2026-10-14)

**What breaks.**

- A window that cannot be seen — minimized, or covered where the
  platform says so — runs its `view` on a `Waker::wake`, a
  `Waker::wake_at` and a time it asked for with `request_frame_at`, and
  draws nothing (backlog F169). A view that took running for being on
  screen runs where it did not: on Windows while minimized, and on macOS
  once the platform stops delivering a hidden window's redraws.
- A click in an editor leaves a caret and no selection, and a press
  that stays inside the click slop (3 px) moves no caret (backlog F170):
  the first Backspace, ←, →, ↑ or ↓ after a click acts, and one at the
  text's edge reports its `boundary`.

On Windows a minimized window is delivered no redraw, and the runner
asked it for none, so an app whose work runs in its view — a sync
engine's or a folder watcher's thread handing over what came, waking
the loop for it — heard nothing until the window was restored, and a
time the view asked for came and went. Carnet's notes from another
device waited on the taskbar. Now a wake or a due time reaching a
window that cannot draw builds its frame at the end of the loop's turn
and does not draw it: the view runs at the size the window last drew
at (a minimized window on Windows is no size at all, and would have
been a `resize`), the events it made reach the app, what it declared —
the title, windows, sounds, the access tree — is applied, and the
frame that brings the window back draws as before. At most four a
second, however often a thread wakes. A hook of its own (an
`App::woken`) was the other way; a wake already means "run `view`" in
every binding, and an app whose work is in its view keeps it there.
On macOS and X11, where the platform says a window is covered
(`Occluded`) and still delivers its redraws for the surface to skip, a
wake asks for one as before and the view runs there at the pace it
did; the unseen frame is the floor under it, built when no frame came
within a quarter second — macOS stops delivering them some while after
a minimize. On Wayland nothing changes: winit delivers the redraw a
wake asks for whether or not the window is seen.

A press in an editor armed a caret drag, and the release placed its
live end once more where the pointer was — as did every frame while
the button was held and every move under it. cosmic-text's drag seeds
a selection at the caret when there is none, so a click left the field
holding a selection from the caret to itself: no edge to report, and
Backspace took the selection, which was nothing. The first key after a
click did nothing and the second worked (backlog F170, from Carnet's
title field). A caret drag now starts once the pointer has left the
click slop of the press, as an `onDrag` node's does, and a drag that
ends on its anchor, or a Shift-click on the caret, holds no selection.
A drag selects as before; a second click takes the word, a third the
line, and a Shift-click extends from the caret.

### Added

- **`Drive::hide`, `show`, `wake`, `due` and `drawn`** (backlog F169):
  the headless driver's window minimized or covered, its frames built
  and not drawn — laid out at the size it last drew at — as the runner
  builds them for a wake (`wake`) or a time the view asked for (`due`,
  when `advance` has reached it); `show` is the frame that brings it
  back, and `drawn` counts the frames that drew.

### Fixed

- **A minimized window's app runs on a wake and at a time it asked
  for** (backlog F169): see *What breaks*.
- **The first key after a click in an editor acts** (backlog F170):
  see *What breaks*.

**What you can delete.**

- A view's work moved out of it, or a timer thread of the app's own, so
  that a minimized window still took in what a thread brought or ran
  what was due.
- A caret set again by hand after a click — the anchor put back on the
  caret once the button is up — so the first Backspace or arrow acts.

### Native verification

The pre-tag pass on 2026-10-14, over the round since alpha.56 — F169's
unseen frames and F170's click — on the Windows machine and under WSL.
It found nothing to change.

**Windows**, rustc 1.99.0. fmt and clippy are clean; `cargo test
--workspace`: **2121 tests over 164 suites**, 0 failed. The C round
passes, and so do the scenes through C; Node's tests under
`KUI_CONFORMANCE_REQUIRED=1`, **229 of 230** with the one skip Windows
has had before; the examples' typecheck and the headless round. The
windowed round: **all 55 examples**, Node's included, drew 120 frames
on each base and exited cleanly.

**Linux** (WSL, Ubuntu 24.04), on the release's code. fmt and clippy
are clean; `cargo test --workspace`: **163 suites**, 0 failed; `cargo
audit --deny warnings` clean over 451 crates. The C round and the
scenes through C; Odin's four steps; Node **230 of 230** with the
corpus required; gen with no diff, the typecheck, the headless round,
the book's examples current. A short fuzz round on eight cores, nine
minutes a target from the kept corpus — `scenes` on four workers, which
spent most of it replaying its 6 518 inputs on the new code, and
`parsers`, `values`, `path` and `decode` on one each (9.9 million, 12.5
million, 181 000 and 179 000 inputs) — found no crash, timeout or
out-of-memory. No Mac ran this round, and nothing minimized a window by
hand: F169 is pinned by the headless driver's `hide`, `wake` and `due`.

## 0.1.0-alpha.56 (2026-10-11)

**What breaks.**

- Of two nodes a frame gives one key, the access tree keeps the first
  and leaves the second out as it does plain structure, its descendants
  under its nearest semantic ancestor (backlog F165).
- A text whose initial letter is a word of its own ("A calm…", "I
  think…") draws its first row beside the letter a space further left:
  the spaces and tabs straight after the initial are collapsed (backlog
  F166).
- `EditOptions` has a field more, `secure`: a struct literal without
  `..` stops compiling.
- `Role` has a variant more, `Role::PasswordInput`, appended (`Role::ALL`
  is 27 long): a `match` on `Role` without a wildcard stops compiling.
  C's `KUI_ROLE_PASSWORD_INPUT` (27) and `KUI_EDIT_SECURE` are new
  constants; no struct moved, and the ABI stays 32.

A key is a node's id to the platform, and the access tree took every
semantic node under its key, so two nodes sharing one were two nodes of
one id, a child listed twice in AccessKit's update; its consumer panics
on that (`TreeUpdate includes duplicate child`). Carnet's Settings →
Sync opened the first row of each of its lists with `with_indexed(0)` in
one card, so its devices and its passkeys each had a first row under
the same key, their names two static texts under one id: with
assistive technology on — VoiceOver, or any app that reads the
accessibility tree — the app closed. The
`duplicate-key` warning names the key as before, on the frames the
checks run; the access tree no longer passes the clash on. The second
node's descendants that have keys of their own are still read; a
presentational one goes with what it holds, as its name is made of it.

A drop cap whose letter is a word — "A calm notes app…", "I think…" —
kept the space after the letter at the start of the first row beside
it, so that row began a space further in than the rows below it, which
start at the indent (backlog F166, from Carnet's review). CSS Inline
Layout collapses the collapsible white space between an initial letter
and what follows it (§ 7.8.1), for the alignment of the lines beside
it, and print does the same: the drop's gap stands for the word space.
The spaces and tabs straight after the initial are shaped at no width
now; their bytes stay where they were, so a caret, a hit, a selection,
a copy and the access tree still read them, at the start of that row
with the caret after them. A no-break space is kept, as CSS keeps it.

A password field was the app's to mask, keeping the text in its model
and drawing bullets in its place, with the caret, the hits and the
selection mapped across the two (backlog F167, from Carnet's review).
An editor declared `secure` is one: see *Added*. Its text is the app's
through `edit_text` as any editor's; what leaves it otherwise — the
drawing, the access tree, the clipboard, the input method, the undo
history — is the bullets or nothing.

### Added

- **A secure editor, a password field** (backlog F167):
  `EditOptions::secure`, `<edit secure>` in JSX, `secure = true` in Lua,
  `KUI_EDIT_SECURE` in C, `.Secure` in Odin's `Edit_Flags`. The text is
  drawn as a bullet (•, `SECURE_BULLET`) per grapheme and measured, hit
  and selected as the bullets; a copy or a cut of it is refused
  (`copy_selection` answers `None`, the Edit menu's Cut and Copy stay
  dim, `EditStore::can_copy` says so) and a paste is taken; no input
  method composes into it and it keeps no undo history; the access tree
  reads it as `Role::PasswordInput` (`passwordInput`, AccessKit's
  `PasswordInput`: macOS's secure text field, UIA's `IsPassword`,
  AT-SPI's password text) whose value and runs are the bullets. While it
  has the focus, `Core::secure_input` and `Core::ime_off` answer true —
  `Ctx.secureInput()` / `imeOff()`, `kui_secure_input_get` /
  `kui_ime_off_get` — so the runner turns on macOS's Secure Keyboard
  Entry (`EnableSecureEventInput`, as `NSSecureTextField` does while it
  is first responder) and the input method off, and gives both back when
  the focus leaves it, with nothing declared on the root. Declared
  without `secure` on a later frame — a "show password" eye — the same
  editor draws its text again, the caret on the same grapheme; declared
  with it again, it masks it and drops the history. A secure editor is a
  field: `multiline` is ignored, and the word keys and a double click
  take the whole text, as there are no words to show.

### Fixed

- **A key two nodes share no longer crashes AccessKit** (backlog F165):
  see *What breaks*.
- **The space after a one-letter initial no longer moves its row in**
  (backlog F166): see *What breaks*.

**What you can delete.**

- Nothing an app wrote for F165: it is a crash, not a workaround. The
  `duplicate-key` warning is still the one to fix.
- A first word of one letter set apart from the drop cap by hand — the
  space after it taken out of the text, or swapped for one the app
  sized — so its row would start where the rows below it do.
- A password field masked by the app: the bullets drawn in the text's
  place, the text kept beside them in the model, and the caret, the
  hits, the selection and the clipboard mapped between the two; and a
  root `secureInput` / `imeOff` declared while that field has the focus.
  A show-password toggle is the `secure` row following the eye.

### Native verification

The pre-tag pass on 2026-10-11, over the round since alpha.55 — F165's
access tree, F166's collapsed space and F167's secure editor — on the
Windows machine and under WSL. It found nothing to change. The fuzz
target's editor scene was given `secure` for it, from bits of its size
byte no input read before, so the kept regressions replay as they did.

**Windows**, rustc 1.99.0. fmt and clippy are clean; `cargo test
--workspace`: **2334 tests over 163 suites**, 0 failed. The C round
passes (6 checks), and so do the **58 scenes** through C; Odin's `gen
--check` (the merge's note left it to this machine; line-ending changes
only), `check`, `test` and `slots` on CI's pinned dev-2026-09; Node's
tests under `KUI_CONFORMANCE_REQUIRED=1`, **229 of 230** with the one
skip Windows has had before; `npm run gen` with line-ending changes
only, the examples' typecheck, the headless round, the book's examples
current. The windowed round: **all 55 examples**, Node's included, drew
120 frames on each base and exited cleanly. The bench guard against the
alpha.55 tag: seven guarded rows clean on the first run, and
`frame_10k_rects_with_access_tree` — the row F165 touches — too noisy
there to judge; on a run of its own it read **−0.1%** (±0.5%).

**Linux** (WSL, Ubuntu 24.04), on the release's code. fmt and clippy
are clean; `cargo test --workspace`: **163 suites**, 0 failed; `cargo
audit --deny warnings` clean over 451 crates. The C round and the **58
scenes** through C; Odin's four steps; Node **230 of 230** with the
corpus required; gen with no diff, the typecheck, the headless round,
the book's examples current. The `scenes` fuzz target with a secure
editor ran seven minutes — the kept corpus's 8 326 inputs replayed with
the new bit read, and some 600 new ones after — with no crash. No Mac
ran this round: macOS's Secure Keyboard Entry under a secure editor is
the runner's, and nothing ran it.

## 0.1.0-alpha.55 (2026-10-11)

**What breaks.**

- C ABI 31: `KuiSpec` appends `uint32_t over_siblings` after
  `iterations`, and its 64-bit size goes from 744 to 752. Recompile
  against the new `kui.h`. A zeroed field paints in tree order, as before.
- `TextStyle` has a field more, `initial_letter`: a struct literal
  without `..` stops compiling.
- `KuiTextStyle` appends `initial_letter` and grows to 80 bytes on
  64-bit — ABI 32, `KUI_ABI_VERSION` 32. Recompile; zeroed, a text has
  no initial.
- A glyph past 1024 physical px is drawn unhinted, which moves its
  outline by under a pixel (backlog FZ8).

Siblings that cross while they `slide` paint in tree order, so a tab moved
left glided under the neighbour making room for it and a tab moved right
glided over it (backlog F163, from kawoosh's tab strip). The new
`overSiblings` row puts one sibling over the rest. Its place in `KuiSpec`
is past the end of the old struct, which is the whole break.

A drop cap — the first letter as tall as a few lines, the lines beside
it moved in past it and the ones below at full width again — needed the
text to lay some of its lines out narrower than others, which a
cosmic-text paragraph cannot (backlog F164, from Carnet, whose "First
letter: Large" could only make the letter large on its own line, which
stood that line apart from the next). `TextStyle::initial_letter(n)` —
`initialLetter` in JSX, `initial_letter` in Lua, `KuiTextStyle.initial_letter`
in C, `Text_Style.initial_letter` in Odin — sets the text's first letter,
with any opening quote or bracket before it, `n` lines tall, as CSS's
`initial-letter`: its top level with the first line's capitals, its foot
on the n-th line's baseline, in its span's face and colour. Inside, the
letter is the buffer's first line of its own and the paragraph is cut
again where the lines beside it end, each part laid out at its width,
with no line ending at either cut, so the text is still one: a caret, a
hit, a selection's rects and backgrounds, a copy and the access tree's
runs read every byte where it is drawn, the letter's caret and its
background as tall as the lines it drops through. A text with
`maxLines` or `ellipsis`, a right-to-left one and an `edit` have none,
and a text that has one is never shaped in a long line's chunks.

A drop cap is shaped at the size its lines ask, which from a large text
is tens of thousands of px, and there a face's hinting program
overflows skrifa's fixed point: a five-line drop from 3000 px text
panicked a debug build on Linux (backlog FZ8, found fuzzing the drop
cap before the tag). Past 1024 px, the size past which alpha.54 began
measuring a glyph's outline before drawing it, a glyph is no longer
hinted. Grid-fitting moves an outline by under a pixel, which at that
size is under a thousandth of the glyph.

### Added

- **`overSiblings`** — `NodeSpec::over_siblings`, `overSiblings` in JSX,
  `over_siblings` in Lua, `KuiSpec.over_siblings` in C (ABI 31),
  `over_siblings` on Odin's `Spec` (backlog F163, ADR 0046): the node
  and its subtree paint after their siblings, over any they overlap,
  and take the press there. Only the sibling order changes. The parent
  still paints under the node, whatever follows the parent still paints
  over it, it keeps the parent's clip, and a float still opens above it.
  A selection still reads its text in tree order. `overSiblings={t.id
  === selected}` on a strip of `slide` tabs keeps the moved tab on top
  whichever way it goes, and needs no state for when it glides.
- **An initial letter, a drop cap** (backlog F164): see *What breaks*.
- The `scenes` fuzz target's editor scene drops its paragraph's first
  letter, through up to five lines.

### Fixed

- **A glyph past 1024 px no longer overflows the font hinter** (backlog
  FZ8): a drop cap from large text panicked a debug build in skrifa.

**What you can delete.**

- A tab floated in its slot (`float={{ anchor: "parent", clip: true }}`)
  only while it glides, along with the state that tracks when the glide
  starts and ends: declare `overSiblings` on it instead.
- A first letter drawn as a text node of its own over a hole in the
  paragraph, with the caret, the hits and the selection mapped across
  the two by hand.

### Native verification

The pre-tag pass on 2026-10-11, over the round since alpha.54 — F163's
`overSiblings` and F164's drop cap — on the Windows machine and under
WSL. The pass found two things to change. The C adapter's `sampler`
scene opened the drop cap's column with the spec `overSiblings` had
just been given, so C painted the scene differently from the other
bindings: the two branches had each been checked alone, and the C
round was the first to see them together. And fuzzing the drop cap
found FZ8, its size handed to the font hinter.

**Windows**, rustc 1.99.0. fmt and clippy are clean; `cargo test
--workspace`: **2324 tests over 162 suites**, 0 failed. The C round
passes (6 checks), and so do the **58 scenes** through C once the
`sampler` scene's column was its own; Node's tests under
`KUI_CONFORMANCE_REQUIRED=1`, **228 of 229** with the one skip Windows
has had before; `npm run gen` with line-ending changes only, the
examples' typecheck, the headless round, the book's examples current.
The windowed round: **all 55 examples**, Node's included, drew 120
frames on each base and exited cleanly. The bench guard against the
alpha.54 tag: **green on the first run**, the guarded rows −2.7% to
+3.8% (`frame_10k_rects_with_access_tree` the +3.8%); the unguarded
`slot_1500_nodes_fresh` read +6.2% there and −7.4% on a run of its
own, this desktop's noise.

**Linux** (WSL, Ubuntu 24.04), on the release's code. fmt and clippy
are clean; `cargo test --workspace`: **160 suites**, 0 failed; `cargo
audit --deny warnings` clean over 451 crates. The C round (5 checks)
and the **58 scenes** through C; `odin.nu gen --check` (the merge's
note left it to this machine), `check`, `test` and `slots`; Node **229
of 229** with the corpus required; gen with no diff, the typecheck, the
headless round, the book's examples current. The `scenes` fuzz target,
its editor scene's paragraph given a drop cap, crashed twice within
sixteen minutes on FZ8; with the fix, fifteen minutes and 82 000 runs
were clean. A timeout it reported takes 0.26 s outside the fuzzer's
instrumentation, on the target before the drop cap as on this one. No
Mac ran this round: nothing in it is platform-specific but the text,
which the headless suites cover on both.

## 0.1.0-alpha.54 (2026-10-10)

**What breaks.**

- `Accel::parse` reads a letter in lower case whichever case it is
  written in: `"⇧⌘S"` is `KeyCode::Char('s')` with Shift, where it was
  `Char('S')`; `Accel::display` uppercases ASCII letters alone; and a
  modifier as the key (`"⌘⌘"`, `"ctrl+shift"`) parses to nothing, as a
  lock key did.
- `path::rasterize` and `path::rasterize_at` leave blank an outline
  that is not finite, lies farther than `path::RASTER_REACH` from the
  mask or spans more than `path::RASTER_SPAN` with its stroke, or a
  mask over `MAX_MASK_SIDE`; and a path's stroke, dash and fill bleed,
  and every arc, are drawn from kui's own flattening rather than zeno's
  curves, which moves a curve's edge pixels by under a tenth of a
  pixel.
- `decode_animation` is an error for a frame over 512 MiB or an
  animation whose frames pass `MAX_ANIMATION_BYTES` (1 GiB), and
  `decode_image` for an image whose RGBA is over 512 MiB, where only its
  decoder's RGB buffer was held to that.
- A glyph past 1024 px whose outline is wider or taller than an atlas
  page (4096 px) is not drawn; it was refused once drawn.
- U+001C–U+001E, NEL and U+2029 are shaped as characters that end no
  bidi paragraph; an editor takes U+2029 as a newline, in its seed,
  `set_edit_text` and typing, and a seed's other four as that
  character.
- The Node addon refuses a binary frame whose string ref or count runs
  past the stream, where a debug addon panicked and any addon could
  abort on the allocation.
- A stock editor's IME composition drops what typing drops — every
  control but a tab and, in a multiline editor, the newline — and the
  IME's caret offsets move with what stays.

kui is fuzzed now (`fuzz/`, `scripts/fuzz.nu`, and a `fuzz` workflow
run by hand): five cargo-fuzz targets over the string parsers, the
parsers that read a prop as data, paths, the conformance scenes and an
editor under every input a driver sends, and the image decoder, and
the Node addon's binary decoder fuzzed in its own tests. The first
round found seven defects, all but one a crash or an allocation a host
could not stop — backlog FZ1–FZ7 — and each is fixed below, pinned by a
test beside its fix and by its minimized input under
`fuzz/regressions/`, which `cargo test --workspace` replays on stable.
What changes for an app is in the list above: a parsed accelerator's
letter is lower case; the path rasterizer refuses numbers its
rasterizer cannot hold, and strokes from a polyline fine enough not to
show; an animation is held to a budget; a glyph larger than any atlas
page is skipped before it is drawn; and the characters that end a bidi
paragraph in the middle of a line are shaped as ones that do not; and an
IME's composition holds what typing would.

### Added

- **Fuzzing** (backlog FZ1–FZ7): `fuzz/` (the `kui-fuzz` crate, never
  published) holds five cargo-fuzz targets — `parsers`, `values`,
  `path`, `scenes`, `decode` — whose bodies are a library, so
  `tests/regressions.rs` replays every seed and every kept crash on
  stable. `nu scripts/fuzz.nu run <target>`, `all` and `keep` run them
  on a workstation (nightly and cargo-fuzz; Linux, WSL or macOS), and
  `.forgejo/workflows/fuzz.yml` runs them on the runner, by hand only,
  carrying each corpus from run to run. `crates/kui-node/src/fuzz_binary.rs`
  fuzzes the binary decoder inside the addon's own tests.
- `kui_native::MAX_ANIMATION_BYTES`, and `kui_core::path::RASTER_SPAN`
  and `RASTER_REACH`: what the decoder and the rasterizer are held to.

### Fixed

- **An accelerator's letter is one chord in either case** (backlog FZ1).
- **The path rasterizer hands zeno only what it can draw** (backlog
  FZ2): a far point, a long line, a near-zero radius, an arc rotated
  past 1e38 degrees or a curve that doubles back under a stroke
  overflowed zeno's fixed point — a panic in a debug build, and for a
  non-finite or far point undefined behaviour in a release one.
- **`decode_animation` holds to a budget** (backlog FZ3): a 15-byte GIF
  asked for 3.6 GB.
- **The Node addon's binary decoder checks its refs and counts**
  (backlog FZ4).
- **Text at a size past any screen draws what fits** (backlog FZ5): at
  10^6 px the glyph rasterizer overflowed.
- **A bidi paragraph end inside a line no longer panics the shaper**
  (backlog FZ6): `abc\u{1e}אב` in a text node did.
- **A carriage return in an IME composition no longer panics the
  editor** (backlog FZ7): the key that ended the composition split the
  buffer off a character boundary.
- `kui-native` builds without its `audio` feature with no unused-import
  warning.

**What you can delete.**

- A filter of U+2029 and the information separators applied to text
  before it reaches a kui text node or editor.
- A size cap applied to text only so a huge size does not crash the
  glyph rasterizer.
- A check of an image's header dimensions before `decode_animation`.

### Native verification

The pre-tag pass on 2026-10-10, over the round since alpha.53 — the
fuzzing and FZ1–FZ7 — on the Windows machine and under WSL. The pass
found one thing to change: the bench guard read the cold wedge raster
62% slower with arcs and strokes flattened at a twentieth of a pixel,
and the fineness went to a tenth (FZ2's outcome records the numbers).

**Windows**, rustc 1.99.0. fmt and clippy are clean; `cargo test
--workspace`: **2081 tests over 159 suites**, 0 failed. The C round
passes (6 checks), and so do the **58 scenes** through C; Node's tests
under `KUI_CONFORMANCE_REQUIRED=1`, **227 of 228** with the one skip
Windows has had before; `npm run gen` with line-ending changes only,
the examples' typecheck, the headless round, the book's examples
current. The windowed round: **all 55 examples**, Node's included,
drew 120 frames on each base and exited cleanly. The bench guard
against the alpha.53 tag: **green over two runs** — each guarded row
read clean in one of them and none regressed, this desktop's noise
leaving one row unreadable in the first and two in the second;
`frame_1k_paths_fresh` +1.6%, `raster_pie_wedge_220px` 7.60 → 10.9 µs
(unguarded), the price of FZ2.

**Linux** (WSL, Ubuntu 24.04). fmt and clippy are clean; `cargo test
--workspace`: **158 suites**, 0 failed; `cargo audit --deny warnings`
clean over 451 crates (the lockfile gained `arbitrary` and
`libfuzzer-sys`). The C round (5 checks) and the **58 scenes** through
C; `odin.nu gen --check`, `check`, `test` and `slots`; Node **228 of
228** with the corpus required; gen with no diff, the typecheck, the
headless round, the book's examples current. The fuzz targets' eighth
round, twenty minutes a target on the commit before the fineness
changed, was clean on all five (26M, 24M, 762k,
77k and 390k runs), and the Node decoder's own fuzz test clean over
500 000 streams. No Mac ran this round: nothing in it is
platform-specific but the decoder, the text and the paths, all of
which the headless suites cover on both.

## 0.1.0-alpha.53 (2026-10-10)

**What breaks.**

- A glyph whose ink lies over the place of a character of another
  colour, shaped with it in one face, is painted in that colour there:
  a ligature across two spans of different colours is drawn in each
  one's over its own cells, where it was drawn wholly in one span's.
- A glyph that is a ligature of characters from more than one span
  shares its advance among them for their backgrounds, underlines and
  strikethroughs, where the first span's took the whole of it.

cosmic-text shapes spans that differ only in colour as one run, so a
coding font's ligature holds across them — `##`, `->`, `**` — and kui
painted each glyph in its first byte's span's colour (backlog F162,
from kawoosh, whose block caret on a `## What` heading in Berkeley Mono
showed the `#` under it in the heading's colour on the caret's
background). A glyph's ink is now painted by the places under it, as a
selection is: each character's colour over its own advance, a ligature
glyph's advance shared among its characters where a caret inside it
goes. Where those colours differ the glyph is cut at whole pixel
columns into slices of one quad, so a slice's edge is the edge of the
span's background. Ink over a place of another face — an italic's lean
into the upright text after it — keeps its own colour. One thing a
reader may see besides a ligature: the edge column of a glyph that
reaches half a pixel into a neighbour of another colour takes that
neighbour's colour, as a terminal's cell does. A text of one colour is
drawn as it was, one quad a glyph.

### Fixed

- **A ligature across two colours is drawn in both** (backlog F162):
  see *What breaks*.

**What you can delete.**

- `liga=0 calt=0` set on a text only so a caret, a selection or a
  syntax colour over one character of a ligature shows in it.

### Native verification

The by-hand round on 2026-10-10, over the round since alpha.52 — F162
from kawoosh's markdown caret — on the Mac: the mechanical round as CI
runs it, the Odin binding with an Odin built locally rather than CI's
pinned one in its image, the bench guard, the windowed round and the
accessibility audit. The round found nothing to fix.

**macOS**, the pre-tag pass. fmt and clippy are clean (rustc 1.99.0);
`nu scripts/test.nu`: **2070 tests over 156 suites**, 0 failed, 4
ignored. The C round passes (5 checks), and so do the **58 scenes**
through Rust, Lua, C, Odin and Node; Node's tests under
`KUI_CONFORMANCE_REQUIRED=1`, **228 of 228**; `npm run gen` with no
diff, the examples' typecheck, the headless round (39 of 39, 6.9 s),
the book built and its examples current, `cargo audit --deny warnings`
clean. `nu scripts/odin.nu gen --check` says the binding is current,
`odin.nu check` vets both packages and every example, `odin.nu test`
runs its four, and `odin.nu slots` its four host and panel pairs. The
bench guard against the alpha.52 tag: **green**, the eight guarded rows
within tolerance (the widest spread 3.8%), `slot_1500_nodes_fresh` and
`slot_1500_nodes_replayed` at 281 and 208 µs. The windowed round:
**all 55 examples**, Node's included, drew 120 frames on each base and
exited cleanly, and the C hosts (`counter`, `host`) and the `c_panel`
and `lua_panel` hosts drew 120 frames each. The AX audit:
**106/106**. No Windows or Linux machine ran this round; F162 is
platform-independent paint, covered by the headless suite.

## 0.1.0-alpha.52 (2026-10-10)

**What breaks.**

- A label looked up from inside a slot fill — `key_of`, `set_scroll`,
  `reveal`, `scroll_offset`, `layout_of` and every door that takes a
  label — is answered from that fill's own nodes alone, where any node
  its extension opened in any of its slots answered, in tree order.
- A field's Backspace and Delete under the primary modifier delete to
  the line's start and end on a Mac (⌘⌫, ⌘⌦) and a word elsewhere
  (Ctrl+Backspace, Ctrl+Delete), where they deleted one character.
- Escape in a focused editor emits `cancel` on it before letting go,
  where it emitted nothing.
- An editor's arrows read the modifiers as the platform's fields do: on
  a Mac ⌥↑ ⌥↓ go to a paragraph's start and end, where they moved a
  line; on Windows and Linux Ctrl+← Ctrl+→ move by words and Ctrl+↑
  Ctrl+↓ by paragraphs, where they went to the line's and the text's
  ends (Ctrl+Home and Ctrl+End still do the latter).

An extension that fills many slots — a Lua host drawing every pane as a
slot of its one origin, a plugin filling `"*"` — had its panes share
their labels: a pane's `set_scroll("list")` found every pane's "list",
took the first in tree order and warned `ambiguous-key` (backlog F157,
from kawoosh, whose themes pane beside a launcher scrolled the
launcher's "body"). Each fill is a view of its own, and as unable to
know what its sibling fills called their nodes as a guest is the
host's, so the lookup made inside a fill reads that fill's nodes alone —
this frame's, then the last frame's. A view asking for its node before
declaring it, on its first frame, gets nothing, where it got a sibling
pane's. `set_scroll_label` and `reveal_label`, resolved at the frame's
end, resolve as the fill that asked, so they find the node it declared
later in the same frame. A lookup from outside any fill, the host's, is
unchanged. An extension that reached into one of its slots from another
by label now finds nothing there; pass the hex key an event carried.

A field read ⌘ and Ctrl only as `doc`, which its motions use and its
deletions ignored: ⌘⌫ in Carnet's folder name deleted one character
where every Mac field deletes back to the line's start (backlog F159).
Backspace and Delete now read it as the platform's fields do — the line
on a Mac, a word on Windows and Linux, where Ctrl is that modifier.

Escape let go of an editor and said nothing, so a blur from Escape and
a blur from a click elsewhere were the same to the app, and a field
that makes what it names when let go of — Carnet's new folder — could
not drop it on Escape (backlog F160). The editor now emits `cancel` and
then lets go; a handler that ignores kinds it does not know is
unaffected.

An editor read Option (Alt) as words and ⌘ (Ctrl) as the line's and the
text's ends on every platform (backlog F161, from Carnet asking that a
field take a Mac's ⌥ and ⌘ moves): a Mac's ⌥↑ and ⌥↓ moved a line as a
plain arrow does, and on Windows and Linux Ctrl+← went to the line's
start where every native field moves a word. ⌥↑ ⌥↓ — Ctrl+↑ Ctrl+↓
elsewhere — now go to the paragraph's start and end, and on to the
one before or after from there; an edge (`boundary`) only where they
could not move. Ctrl+← Ctrl+→ move by words off a Mac; Alt still does.

### Added

- **`cancel` on an editor** (backlog F160): Escape let go of it — see
  *What breaks*.

### Fixed

- **One extension's panes no longer share their labels** (backlog F157):
  see *What breaks*.
- **On a Mac a double click on a button in the titlebar strip no longer
  zooms the window** (backlog F158). Under `Chrome::Custom` AppKit took
  every press on the content under its hidden titlebar as the
  titlebar's, a toolbar's buttons included. Now none is: a
  `window_drag` region's double click is the only one that acts, counted
  by AppKit and doing what System Settings says a titlebar's does —
  zoom, minimize or nothing — where the runner zoomed on its own 350 ms
  clock.

**What you can delete.**

- A pane's name or id folded into every label a multi-slot extension
  looks up, to keep two of its panes' `list` apart.
- A key sink's own ⌘⌫ / Ctrl+Backspace over a field, and the guess that
  a field's blur followed an Escape.

### Native verification

The by-hand round on 2026-10-10, over the round since alpha.51 — F157
from kawoosh's pane labels, and F158–F161 from Carnet's toolbar and
folder rounds — on the Mac: the mechanical round as CI runs it, the
Odin binding with an Odin built locally rather than CI's pinned one in
its image, the bench guard, and this time the windowed round and the
accessibility audit as well. The round found one regression of its own
making — `odin.nu gen` refused F160's `cancel` row with no Odin
`Cancel_Event` for it — fixed before the tag.

**macOS**, the pre-tag pass. fmt and clippy are clean; `nu
scripts/test.nu`: **2067 tests over 155 suites**, 0 failed, 4 ignored.
The C round passes (5 checks), and so do the **58 scenes** through
Rust, Lua, C, Odin and Node; Node's tests under `KUI_CONFORMANCE_REQUIRED=1`,
**228 of 228**; `npm run gen` with no diff, the examples' typecheck,
the headless round (39 of 39, 3.6 s), the book built and its examples
current, `cargo audit --deny warnings` clean. `nu scripts/odin.nu gen
--check` says the binding is current, `odin.nu check` vets both packages
and every example, `odin.nu test` runs its four, and `odin.nu slots`
its four host and panel pairs. The bench guard against the alpha.51
tag: **green**, the eight guarded rows within tolerance (the widest
spread 2.3%), `slot_1500_nodes_fresh` and `slot_1500_nodes_replayed`
at 277 and 207 µs. The windowed round: **all 55 examples**, Node's
included, drew 120 frames on each base and exited cleanly. The AX
audit: **106/106**. No Windows or Linux machine ran this round; F161's
Windows and Linux arms and F158's Windows half are built for the
Windows target and not run there.

## 0.1.0-alpha.51 (2026-10-09)

**What breaks.**

- ↑ and ↓ on an editor's first or last visual line emit `boundary`
  wherever the caret is on it, not only from the line's end.
- A slot fill kept with `slot_kept` that read the theme or the metrics
  is run again by `slot_replay` when either changes (`SlotFill::Reads`),
  where it was replayed in the old colours.
- `Core::slot_fill` — `kui_slot_fill`, Lua's `env.slot_fill`, Node's
  `ctx.slotFill` — answers `None` after a frame that asked `slot_replay`
  nothing of the slot, where it answered the frame before's.

F145's `boundary` reported ↑ and ↓ only when the key left the caret
exactly where it was, but on a field's one line ↓ walks the caret to the
end first: the edge came on the second press. Carnet's ↓ from the middle
of its title stayed in the title (backlog F154). ↑ on the first visual
line and ↓ on the last now report on the first press, wherever along the
line the caret was; the editor still moves its caret as it always did.
←, →, Backspace and Delete are unchanged.

`slot_replay` checks every fact of the frame a kept fill read before it
replays it, and its docs named the theme among them, but no theme read
was noted: a fill that drew from `Ui::theme`, C's `kui_theme`, Lua's
`env.theme`, Node's theme reader, or a `$role` token a binding lowered
into a colour, was replayed in last frame's palette after the theme
changed (backlog F155, from kawoosh's native-view replay, which put its
palette's hash in its own claim to cover it). The metrics had the same
hole. Both are now reads of the kept fill, noted once a fill and
compared by value, so a palette set again to what it was is no change;
a host that replays a pane across a theme change sees `Reads`, and the
pane drawn in the new palette, where it saw `Replayed`.

`slot_fill` read this frame's answers and then the last frame's, so a
view could ask before its slot was declared; but it did so after the
frame too, and a frame that kept its pane with `slot_kept`, filled it
with `slot_with` or skipped it answered the frame before's `Replayed`
(backlog F156, from kawoosh, whose test read the pane's own count
instead). The frame before's answers now stand in only while a frame is
being built; after it, a frame that did not ask has no answer — `None`,
`KUI_SLOT_UNDECLARED` in C, `nil` in Lua, `null` in Node — the answer
`slot_fill` already gave for a slot no frame declared.

### Added

- **An editor's caret by point** — `Core::edit_caret_rect(key)`, the
  caret in logical viewport px, and `Core::edit_caret_to(key, point)`,
  the caret put at the character nearest a point without a click
  (backlog F154): what carries a column between a field and an app's own
  text. Rust only for now.

### Fixed

- **↓ from the middle of a field is its edge at once** (backlog F154):
  see *What breaks*.
- **A replayed pane follows a theme change** (backlog F155): see *What
  breaks*.
- **`slot_fill` after a kept frame is that frame's** (backlog F156): see
  *What breaks*.

**What you can delete.**

- Pressing ↓ twice, or watching `changed` and the caret, to leave a field
  from the middle of its line.
- The palette's hash, or a theme revision of the host's own, in the claim
  a host makes before `slot_replay`.
- Counting a pane's own views to learn whether a frame replayed it, where
  `slot_fill` read a frame stale.

### Native verification

The by-hand round on 2026-10-09, over the round since alpha.50 — F154,
Carnet's title arrows, and F155 and F156 from kawoosh's native-view
replay — on the Mac: the mechanical round as CI runs it, the Odin
binding with an Odin built locally rather than CI's pinned one in its
image, and the bench guard. The windowed round, the accessibility audit
and the other checks that need someone at the screen were not run this
time.

**macOS**, the pre-tag pass. fmt and clippy are clean; `nu
scripts/test.nu`: **2060 tests over 154 suites**, 0 failed, 4 ignored.
The C round passes (5 checks), and so do the **58 scenes** through
Rust, Lua, C, Odin and Node; Node's tests under `KUI_CONFORMANCE_REQUIRED=1`,
**228 of 228**; `npm run gen` with no diff, the examples' typecheck,
the headless round (5.4 s), the book's examples current. `nu
scripts/odin.nu gen --check` says the binding is current, `odin.nu
check` vets both packages and every example, and `odin.nu test` runs
its four. The bench guard against the alpha.50 tag: **green**, the
eight guarded rows within tolerance (the widest spread 2.4%, on a
machine another job was loading), `slot_1500_nodes_fresh` and
`slot_1500_nodes_replayed` unchanged at 294 and 223 µs. Not run: the
windowed round over every example and the four hosts, and the AX audit.
No Windows or Linux machine ran this round.

## 0.1.0-alpha.50 (2026-10-09)

**What breaks.**

- `UiEvent::message` reads a core event's `tag` before its payload.
- A chord a focused editor does not act on reaches the `on_key` sink
  above it, where it went nowhere.
- An arrow, Backspace or Delete at the edge of an editor's text emits a
  `boundary` event, where it emitted nothing.
- An editor that lost the keyboard no longer draws its selection.
- `KUI_ABI_VERSION` is 30: `KuiTextStyle` gains `bold` (29) and
  `KuiMenuItem` `replay` (30). Recompile a C or Odin host.
- `MenuItem` has a `replay` field: a struct literal of it needs one.
- A pointer event inside a key sink that draws `role="line"` rows carries
  `inside` beside `line` and `byte` — a click's own map payload too.
- `MenuRole` has five more variants (`About`, `Hide`, `HideOthers`,
  `ShowAll`, `Quit`): an exhaustive `match` on it needs them.

`UiEvent::message` read the payload first and the `tag` second, and a
core event's payload is `{kind: "key", …}`, `{kind: "open", …}`, `{kind:
"menu", …}` — so an app whose message enum had a `Key` variant got
`Msg::Key` back for every key event any of its sinks heard, the sink's
own tag never consulted (backlog F148, from Noticon, whose root sink
was tagged `Key` and took the page's keys). A core event — any `kind`
the events table lists but `click` — now reads as its tag when the tag
is an `M`, and as the payload otherwise, as before. A click's payload
is still the app's message as-is, whatever its `kind`. An app that
matched its own variant against a tagged core event, meaning to or not,
now gets the tag.

A focused editor kept the whole keyboard: ⌘N, Ctrl+K — any
press — went to the editor and, if it did nothing with it, nowhere, so an
app's shortcuts were dead while its search box had focus. ADR 0011
deferred this until an app wanted it (backlog F144, Noticon's title and
search fields). An editor now claims what it acts on — the editing keys,
whatever it types, Space, the clipboard and undo chords — and anything
pressed without Control or Command, which is typing; any other chord
goes to the nearest `on_key` sink above it, with its release, as a chord
bubbles from a control. A shell sink that binds a chord now hears it
from inside a field, which is the point; one that should not act while
a field has focus has to check `focused` itself.

An editing key that meets the edge of the text — ↑ on the first line, ↓
on the last, ← or Backspace at the start, → or Delete at the end, with
no selection and no Shift — now emits `{kind: "boundary", key, edge,
word, doc}` on the editor (backlog F145). A handler that counted an
editor's events, or matched every event on an editor's key as a change,
sees one more.

An editor that lost the keyboard kept drawing its selection, so a page of
fields showed a highlight in each and only one of them was what ⌘C
copied (backlog F147). It now keeps the range and draws none of it until
focus comes back — and while a context menu or the drawn menu bar opened
over it is up, since the menu's rows took focus to be chosen and the
selection is what Copy will copy. A screenshot test of a blurred field
with a selection changes.

`KuiTextStyle` appends `bold` into its tail padding — the 64-bit size stays
72 — for F150 below, and `KuiMenuItem` puts `replay` into the padding
before `submenu`, its stride still 72, for F151; the layouts moved, so
the ABI did, twice. Zeroed fields are what a text and a row were.

### Added

- **Chords out of a focused editor** (backlog F144, ADR 0011 decision
  10): see *What breaks*.
- **`boundary`, an editing key at the edge of the text** (backlog
  F145): the event a block editor joins blocks and moves between fields
  by. A field has one line, so its ↑ and ↓ always report.
- **`keep_tab` on an editor** — `EditOptions::keep_tab`, `keepTab` in
  JSX, `keep_tab` in Lua, `KUI_EDIT_KEEP_TAB` in C, `.Keep_Tab` in Odin
  (backlog F146): a field's Tab and Shift-Tab go to the sink above it
  instead of walking the focus ring, so a list or an outline built from
  fields indents on Tab. A document keeps Tab either way. A new flag
  bit, no ABI change.
- **`inside` on a line sink's pointer events** (backlog F153): whether the
  point is within the line's own box, false above, below and in the
  margin beside a row — so a margin gesture is told from a press on the
  text without comparing `x` to a rect.
- **The application menu's rows** — `MenuRole::About`, `Hide`,
  `HideOthers`, `ShowAll`, `Quit`; `"about"`, `"hide"`, `"hideOthers"`,
  `"showAll"`, `"quit"` as a row's `role`; `KUI_MENU_ABOUT` …
  `KUI_MENU_QUIT` in C (appended values, no ABI bump) (backlog F152, ADR
  0018 amended): on a menu bar macOS draws, AppKit's own rows — the About
  panel, `hide:`, `hideOtherApplications:`, `unhideAllApplications:`,
  `terminate:` — worded with the app's name, with ⌘H, ⌥⌘H and ⌘Q, posting
  nothing. In a menu kui draws, Hide, Hide Others and Show All are not
  offered, and About and Quit post their `menu` event for the app.
- **A menu row that is its chord** — `MenuItem::replay`, `replay` on a
  plain-data row (JSX, Lua), `KuiMenuItem.replay` and
  `KUI_MENU_ITEM_REPLAY` in C, `replay` on Odin's `Menu_Item` (backlog
  F151, ADR 0018 amended): chosen — by the pointer, or by the key
  equivalent the macOS bar binds — it plays its `accel` where the
  keyboard is, as the standard Edit menu's rows do, and posts no `menu`
  event. A declared Edit menu's Undo is a field's undo while a field has
  focus and the app's otherwise, with no rebuilding of the bar.
- **`bold` on a whole text or an editor** — `TextStyle::bold`, `bold` in
  JSX and Lua, `KuiTextStyle.bold` in C (ABI 29), `bold` on Odin's
  `Text_Style` (backlog F150): the family's bold — its bold face, or its
  regular drawn bold where it has none, as a span's `bold` is — so a
  heading or a title field is bolder, not only bigger. A span inside a
  bold text is bold too.
- **A placeholder on an editor** — `EditOptions::placeholder`,
  `placeholder` in JSX and Lua, `kui_text_edit_placeholder` in C (a new
  function, no ABI bump), `placeholder` on Odin's `kui.text_edit`
  (backlog F149): what an empty editor shows where its text would be, in
  the theme's `faint`. Never part of the value, gone with the first
  character typed or composed, and the field's accessible description
  when it declares none.

### Fixed

- **A message variant named like a core event no longer catches it**
  (backlog F148): see *What breaks*.
- **One selection on screen among several editors** (backlog F147): see
  *What breaks*.

**What you can delete.**

- Renaming a message variant, or a sink's tag, away from a core event's
  kind (`Key`, `Open`, `Menu`, `Changed`) so `message` reads the tag.
- Routing an app's shortcuts through a declared menu bar, or a field's
  own key handling, so they work while a field has focus.
- Comparing a drag's `x` against a line's rect to tell a margin press
  from a press on its text.
- Sending `hide:`, `hideOtherApplications:` or `unhideAllApplications:`
  to `NSApplication` by hand, and quitting from a declared bar by closing
  the window.
- Rebuilding a declared menu bar as focus moves, unbinding rows a field
  needs its key for, and replaying a chosen row's accelerator by hand.
- A one-span `rich_text` or a registered bold face standing in for a bold
  heading, and a bold title that could not be an editor.
- A faint text floated over an empty field and positioned by hand to
  match the editor's insets, standing in for a placeholder.
- Collapsing an editor's selection when it loses focus, to keep a page
  of fields from showing several highlights.
- Comparing an editor's text before and after an arrow or Backspace to
  tell whether it reached the edge, or a separate key sink over a field
  to hear ↑ and ↓.

### Native verification

The by-hand round on 2026-10-09, over Noticon's macOS polish round —
F144–F153, W23 and the Odin binding checked against the generator — on
the Mac: the mechanical round as CI runs it, the Odin binding with an
Odin built locally against LLVM 21 rather than CI's pinned one in its
image, and the bench guard. The windowed round, the accessibility audit
and the other checks that need someone at the screen were not run this
time.

**macOS**, the pre-tag pass. fmt and clippy are clean; `nu
scripts/test.nu`: **2054 tests over 154 suites**, 0 failed, 4 ignored.
The C round passes (5 checks), and so do the **58 scenes** through
Rust, Lua, C, Odin and Node; Node's tests under
`KUI_CONFORMANCE_REQUIRED=1`, **228 of 228**; `npm run gen` with no
diff, the examples' typecheck, the headless round (8.0 s), the book's
examples current. `nu scripts/odin.nu gen --check` says the binding is
current, `odin.nu check` vets both packages and every example, and
`odin.nu test` runs its four. The bench guard against the alpha.49 tag:
**green**, the eight guarded rows within tolerance. Not run: the
windowed round over every example and the four hosts, and the AX audit.
No Windows or Linux machine ran this round.

## 0.1.0-alpha.49 (2026-10-09)

### Changed

- **A Lua view lowers in about two-thirds of the time** (backlog F143).
  kui-lua reads each table in one pass: `Table::for_each` with a key
  type that copies a string key's bytes off the Lua stack instead of
  making a registry reference for it (a `pairs` step made one for every
  key and every string or table value, and dropped it again), the keys
  matched as bytes, the schema rows looked up in a hash map rather than
  scanned, `size` and `radius` read in the same pass and still applied
  before the rows they underlie, a text's `value` and `spans` taken from
  it, and a span's twelve style keys, a `pad` table's seven edges and
  the unknown-prop check — a second walk over every table while the
  core's diagnostics are on — folded into it. Nothing a view writes
  changes, and a table in an unusual shape (a number for a text's
  `value`, a span's `size` as a string) is read through `get` as before.
  Measured over a settings pane's worth of tables (`cargo bench -p
  kui-lua --bench walk`, medians): **2.55 → 1.64 ms** a frame with the
  diagnostics off, as a release build of the windowed runner has them,
  and **3.42 → 1.75 ms** with them on; in kawoosh's settings, themes,
  theme lab and grammars panes the walk went 3.45 → 2.27, 3.51 → 2.13,
  2.12 → 1.42 and 1.46 → 1.06 ms on the frames a pane is rebuilt in.
  The rest of the walk is mlua's own reference for each string and
  table value, which its safe API does not let a binding skip.

**What you can delete.**

- Hand-flattening a Lua view's tree, or building it in fewer, larger
  tables, to keep the lowering off the frame: kawoosh's settings pane
  lowers at about 1.4 µs a table where it took 2.2.

### Native verification

The by-hand round on 2026-10-09, over F143 — kui-lua's walk from a
view's tables to the tree — on the Mac: the mechanical round as CI runs
it, the Odin binding checked with CI's pinned Odin in its Debian image,
the windowed round, the accessibility audit and the bench guard.

**macOS**, the pre-tag pass. fmt and clippy are clean; `nu
scripts/test.nu`: **2034 tests over 152 suites**, 0 failed. The C round
passes (5 checks), and so do the **58 scenes** through Rust, Lua, C and
Node; Node's tests under `KUI_CONFORMANCE_REQUIRED=1`, **225 of 225**;
`npm run gen` with no diff, the examples' typecheck, the headless round
(4.0 s). `nu scripts/odin.nu gen --check` says the binding is current
and `odin.nu check` vets both packages and every example. The windowed
round with Node's: **55 examples on both bases**, clean on a first run,
and `counter`, `host`, `c_panel` and `lua_panel` for 120 frames each.
The accessibility audit: **106 of 106**. The bench guard against the
alpha.48 tag: **green**, the eight guarded rows within tolerance; the
new `walk` rows are in docs/performance.md. No Windows or Linux machine
ran this round.

## 0.1.0-alpha.48 (2026-10-09)

**What breaks.**

- C ABI 28 (under Added, W22): `KuiRunConfig` appends `titlebar`, a
  `KUI_TITLEBAR_*` (64-bit size 48). Recompile; zeroed, the window is
  what it was. A hand-written mirror (ctypes, Zig) appends one `u32`.
- `ui.metrics().titlebar_h` under macOS custom chrome (under Added,
  W22) reads the measured titlebar, 32 on macOS 27, where it read the
  stock 34; the strip itself was already drawn at 32. A view that laid
  something out against the metric beside the strip moves up 2 px.

### Added

- **The traffic lights sit lower in a taller titlebar** (backlog W22).
  `Launcher::titlebar(Titlebar::Medium | Tall)` under `Chrome::Custom`
  makes macOS's own titlebar a compact toolbar's height (40 pt, the
  lights at 12,13 on macOS 27) or a full toolbar's (52 pt about the
  lights, at 19,19), against the plain one's 32 at 9,9 — the look of a
  Mac app with a toolbar, with the app's own strip in it. kui gives the
  window an empty `NSToolbar` in that style and leaves the buttons to
  AppKit, which keeps them in place through resizing, fullscreen and
  focus; nothing runs per frame. `env.window.native_controls` measures
  where they landed, so `widgets::titlebar` grows and insets with them,
  and the toolbar takes no clicks: tabs in the strip press and the empty
  strip drags as before. `titlebar: 'standard' | 'medium' | 'tall'` in
  Node's window options, `KuiRunConfig.titlebar` in C,
  `Run_Config.titlebar` in Odin. Under macOS custom chrome the
  window's `titlebar_h` metric is now the strip AppKit drew, as
  measured — 32, 40 or 52 on macOS 27 — so `$titlebar_h` says the
  strip's height. On Windows and Linux, where the strip
  is the app's, the same ask makes the window's `titlebar_h` metric 40
  or 52 in place of the caption's 32 or 34, so one setting draws one
  strip on all three: `ui.metrics().titlebar_h` and `$titlebar_h` read
  it, the drawn buttons grow to it, and an app's own metrics keep it
  while their `titlebar_h` is the stock number
  (`Core::set_platform_titlebar_h`). The `titlebar` example takes
  `--titlebar tall`.
