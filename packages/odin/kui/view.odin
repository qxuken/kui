package kui

import c "c"

// The elements, written by hand as the schema says they are
// (schema::ELEMENTS: "hand-lowered per binding"): a container that closes
// itself, a widget that takes its state as an argument. The elements whose
// C door already has the right shape - image, line, polygon, path, cells,
// fragment, select, audio, tooltip, titlebar, menu_bar - are generated
// doors (generated.odin).
//
//     view :: proc(s: ^State, ui: ^kui.Ui) {
//         t := kui.theme(ui)
//         kui.root(ui, {width = kui.GROW, height = kui.GROW, bg = t.bg})
//         if kui.column(ui, {gap = 12, pad = kui.pad(16)}) {
//             kui.text(ui, "Hello", {size = 24})
//             kui.button(ui, "+1", Inc{})
//         }
//     }
//
// A container opens with its proc and closes at the end of the block it
// was called in - the `if` above - through @(deferred_in), so an open
// without its close cannot be written. Always `if kui.box(...) { }`:
//
//   - never a bare `kui.box(ui, spec)`: the node would stay open to the end
//     of the enclosing block and swallow every sibling after it. The
//     openers are @(require_results), so that does not compile; a box with
//     no children is `kui.rect(ui, spec)`.
//   - never `if !kui.column(ui, spec) do return`: the `if` is the block, so
//     the column closes at once and its children land in the parent. That
//     one compiles; nothing to return from, the openers are always true.
//
// (backlog F230, from a game embedding kui)

// The root of the frame: the viewport, laid out by spec.
root :: proc(ui: ^Ui, spec: Spec = {}) {
	cs: c.Spec
	scratch: Scratch
	defer scratch_free(&scratch)
	spec_to_c(spec, &cs, &scratch)
	c.root(ui, &cs)
}

// The root of the frame, and the messages the events about this window
// arrive as (ADR 0048): a resize, its keyboard focus, the OS's settings,
// the fonts, the held modifiers and, on the main window, the documents the
// OS opens. A message left nil is an event the window does not hear, and
// the group is declared each frame like the rest of the root: a frame that
// calls plain root stops hearing them.
//
//     Resized :: struct {using _: kui.With_Resize}
//     kui.root_with(ui, kui.Spec{width = kui.GROW, height = kui.GROW}, {on_resize = Resized{}})
//
// A nil spec leaves the root as the frame began it and declares the group
// alone. The spec is a Maybe, so a literal one names its type: a bare
// `{width = ...}` does not compile, and a bare `{}` is the nil.
root_with :: proc(ui: ^Ui, spec: Maybe(Spec), events: Window_Events) {
	scratch: Scratch
	defer scratch_free(&scratch)
	cs: c.Spec
	cs_ptr: ^c.Spec
	if s, ok := spec.?; ok {
		spec_to_c(s, &cs, &scratch)
		cs_ptr = &cs
	}
	ce := c.WindowEvents {
		on_resize                 = borrow(&scratch, events.on_resize),
		on_focus                  = borrow(&scratch, events.on_focus),
		on_system_settings_change = borrow(&scratch, events.on_system_settings_change),
		on_fonts                  = borrow(&scratch, events.on_fonts),
		on_modifiers              = borrow(&scratch, events.on_modifiers),
		on_open                   = borrow(&scratch, events.on_open),
	}
	c.root_with(ui, cs_ptr, &ce)
}

// The messages the events about a window arrive as (root_with), kui.h's
// KuiWindowEvents: any message each, with the event's data in the field
// named for it, which the With_X beside each decodes; nil for an event the
// window does not hear.
Window_Events :: struct {
	on_resize:                 any, // resize = {width, height, scale}: With_Resize
	on_focus:                  any, // window = {phase = "focused" | "blurred"}: With_Window
	on_system_settings_change: any, // system = {appearance, accent, ...}: With_System
	on_fonts:                  any, // no data: the message as declared
	on_modifiers:              any, // modifiers = {shift, ctrl, alt, super}: With_Modifiers
	on_open:                   any, // open = {paths}, the main window's only: With_Open
}

// A container: a box laid out by spec, holding what the block declares.
// Returns true, for the `if` that scopes it.
@(deferred_in = close, require_results)
box :: proc(ui: ^Ui, spec: Spec = {}) -> bool {
	open(ui, spec)
	return true
}

// A box whose children stack top to bottom (the default direction).
@(deferred_in = close, require_results)
column :: proc(ui: ^Ui, spec: Spec = {}) -> bool {
	s := spec
	s.dir = .Column
	open(ui, s)
	return true
}

// A box whose children sit side by side.
@(deferred_in = close, require_results)
row :: proc(ui: ^Ui, spec: Spec = {}) -> bool {
	s := spec
	s.dir = .Row
	open(ui, s)
	return true
}

// A virtual list of `rows` rows `row_h` px tall, built a screenful at a
// time (backlog F280): list_begin's rows, first ..< last, to build in the
// block, each with list_row; list_end closes it at the end of the block.
// The container is `spec` made a scrolling column with no gap.
//
//     if first, last, ok := kui.list(ui, "log", n, 28, {width = kui.grow(1), height = kui.grow(1)}); ok {
//         for i in first ..< last {
//             if kui.list_row_box(ui, i, {pad = kui.pad(6), on_click = Pick{int(i)}}) {
//                 kui.text(ui, lines[i])
//             }
//         }
//     }
@(deferred_in = list_close, require_results)
list :: proc(ui: ^Ui, label: string, rows: u64, row_h: f32, spec: Spec = {}) -> (first, last: u64, ok: bool) {
	_, first, last = list_begin(ui, label, spec, rows, row_h)
	return first, last, true
}

@(private)
list_close :: proc(ui: ^Ui, _: string, _: u64, _: f32, _: Spec = {}) {
	list_end(ui)
}

// Row `index` of the open list as a container: list_row with the spec's
// own click, closed at the end of the block.
@(deferred_in = close_list_row, require_results)
list_row_box :: proc(ui: ^Ui, index: u64, spec: Spec = {}) -> bool {
	list_row(ui, index, spec, spec.on_click)
	return true
}

@(private)
close_list_row :: proc(ui: ^Ui, _: u64, _: Spec = {}) {
	close(ui)
}

// A box with no children - a swatch, a rule, a decoration - opened and
// closed in one call, where a bare box would stay open. Returns its key.
rect :: proc(ui: ^Ui, spec: Spec = {}) -> (key: u64) {
	key = open(ui, spec)
	close(ui)
	return
}

// The unscoped pair under box, column and row, for a node opened in one
// procedure and closed in another. Every open needs its close before the
// frame finishes. Returns the node's key.
open :: proc(ui: ^Ui, spec: Spec) -> (key: u64) {
	cs: c.Spec
	scratch: Scratch
	defer scratch_free(&scratch)
	spec_to_c(spec, &cs, &scratch)
	// The four message arguments are consumed by the call; the spec's own
	// are borrowed and freed above.
	on_click := to_value(spec.on_click)
	switch {
	case spec.on_drag != nil || spec.on_key != nil || spec.on_hover != nil:
		key = c.open_with(ui, spec.key, &cs, on_click, to_value(spec.on_drag), to_value(spec.on_key), to_value(spec.on_hover))
	case spec.index != nil:
		key = c.open_indexed(ui, spec.index.?, &cs, on_click)
	case spec.key != "":
		key = c.open_keyed(ui, spec.key, &cs, on_click)
	case:
		key = c.open(ui, &cs, on_click)
	}
	if spec.key_focus do set_key_focus(ui, key)
	if rows, ok := spec.row_count.?; ok do row_count(ui, rows)
	return
}

close :: proc(ui: ^Ui, _: Spec = {}) {
	c.close(ui)
}

// -- Text -------------------------------------------------------------------

text :: proc(ui: ^Ui, s: string, style: Text_Style = {}) {
	st := text_style_to_c(style)
	c.text(ui, s, &st)
}

// One paragraph of differently styled runs.
rich_text :: proc(ui: ^Ui, spans: []Span, style: Text_Style = {}) {
	st := text_style_to_c(style)
	c.rich_text(ui, ([^]c.Span)(raw_data(spans)), uint(len(spans)), &st)
}

// A box that takes the room left over along its parent's axis.
spacer :: proc(ui: ^Ui) {
	if box(ui, {width = GROW, height = GROW}) {}
}

// -- Stock widgets ----------------------------------------------------------

// The stock button. Of spec it reads label, description, tooltip and
// disabled; its look is the theme's.
button :: proc(ui: ^Ui, label: string, msg: any = nil, spec: Spec = {}) {
	cs: c.Spec
	scratch: Scratch
	defer scratch_free(&scratch)
	spec_to_c(spec, &cs, &scratch)
	c.button_with(ui, label, &cs, to_value(msg))
}

// The stock toggles. Of spec each reads checked (and on a checkbox
// mixed), label, description, tooltip and disabled; of text, its label's
// line breaking - wrap, max_lines and ellipsis - and nothing else: with
// ellipsis the label is one line, cut where the toggle is squeezed, and
// its whole text floats as the hint while it is cut (backlog F184).
checkbox :: proc(ui: ^Ui, label: string, checked: bool, msg: any = nil, spec: Spec = {}, text: Text_Style = {}) -> u64 {
	s := spec
	s.checked = checked
	return stock(c.checkbox_with, ui, label, s, msg, text)
}

radio :: proc(ui: ^Ui, label: string, checked: bool, msg: any = nil, spec: Spec = {}, text: Text_Style = {}) -> u64 {
	s := spec
	s.checked = checked
	return stock(c.radio_with, ui, label, s, msg, text)
}

// The stock switch (kui_switch; `switch` is Odin's).
toggle :: proc(ui: ^Ui, label: string, on: bool, msg: any = nil, spec: Spec = {}, text: Text_Style = {}) -> u64 {
	s := spec
	s.checked = on
	return stock(c.switch_with, ui, label, s, msg, text)
}

@(private)
stock :: proc(door: proc "c" (_: ^c.Ctx, _: string, _: ^c.Spec, _: ^c.TextStyle, _: ^c.Value) -> u64, ui: ^Ui, label: string, spec: Spec, msg: any, text: Text_Style) -> u64 {
	cs: c.Spec
	scratch: Scratch
	defer scratch_free(&scratch)
	spec_to_c(spec, &cs, &scratch)
	st := text_style_to_c(text)
	return door(ui, label, &cs, &st, to_value(msg))
}

// A group of radios, one Tab stop with the arrows inside it.
@(deferred_in = close_radio_group, require_results)
radio_group :: proc(ui: ^Ui, label: string, spec: Spec = {}) -> bool {
	cs: c.Spec
	scratch: Scratch
	defer scratch_free(&scratch)
	spec_to_c(spec, &cs, &scratch)
	c.radio_group_open(ui, label, &cs)
	return true
}

@(private)
close_radio_group :: proc(ui: ^Ui, _: string, _: Spec = {}) {
	c.close(ui)
}

// A slider at value in [lo, hi]. The core proposes changes as msg with
// change = {value, phase} in it (a message embedding kui.With_Slider_Change);
// the app moves value, and the slider follows on the next frame. Without
// msg the slider cannot move.
slider :: proc(ui: ^Ui, label: string, value, lo, hi: f32, msg: any = nil, spec: Spec = {}) -> u64 {
	s := spec
	s.value_now, s.value_min, s.value_max = value, lo, hi
	if msg != nil do s.on_change = msg
	cs: c.Spec
	scratch: Scratch
	defer scratch_free(&scratch)
	spec_to_c(s, &cs, &scratch)
	return c.slider(ui, label, &cs)
}

// A progress bar at value in [lo, hi], read-only: a reader hears the value
// the view declares, and the new one each frame it moves on. A nil value is
// busy. spec.role = .Meter makes it a level (a battery, a disk) rather than
// a task's progress; spec.value_text says the reading where the number does
// not ("3 of 8 files"). Backlog F267.
progress :: proc(ui: ^Ui, label: string, value: Maybe(f32), lo: f32 = 0, hi: f32 = 100, spec: Spec = {}) -> u64 {
	s := spec
	s.value_now, s.value_min, s.value_max = value, lo, hi
	cs: c.Spec
	scratch: Scratch
	defer scratch_free(&scratch)
	spec_to_c(s, &cs, &scratch)
	return c.progress(ui, label, &cs)
}

// A one-line text field, keyed by label; read it with edit_text. It
// reports on its key with no message of the app's: an edit as {change =
// {text}} (kui.With_Edit_Change), Enter and Escape as Ev.event "submit" and
// "cancel" with an empty payload.
text_input :: proc(ui: ^Ui, label: string, initial := "") -> u64 {
	return c.text_input(ui, label, initial)
}

// text_input in messages of the app's own (ADR 0047): an edit arrives as
// on_change with change = {text} in it (embed kui.With_Edit_Change), Enter
// as on_submit and Escape as on_cancel, as declared; one left nil is the
// field alone. With value, text is the field's value - brought back on
// every frame it differs from the field's text - rather than its seed.
text_input_with :: proc(ui: ^Ui, label: string, text := "", value := false, on_change: any = nil, on_submit: any = nil, on_cancel: any = nil) -> u64 {
	flags: Edit_Flags = {.Value} if value else {}
	return c.text_input_with(ui, label, text, transmute(u32)flags, to_value(on_change), to_value(on_submit), to_value(on_cancel))
}

// The editor: one line or many (flags), styled, laid out by spec. The
// placeholder is what it shows, faint, while it is empty. With .Value in
// flags, initial is the editor's value, not its seed: declare spec.on_change
// with it, or the field reverts every keystroke. The editor reports in the
// messages spec declares - on_change (change = {text}), on_submit,
// on_cancel, on_boundary - and sends nothing without them.
text_edit :: proc(ui: ^Ui, label: string, initial := "", flags: Edit_Flags = {}, style: Text_Style = {}, spec: Spec = {}, placeholder := "") -> u64 {
	st := text_style_to_c(style)
	cs: c.Spec
	scratch: Scratch
	defer scratch_free(&scratch)
	spec_to_c(spec, &cs, &scratch)
	if placeholder != "" {
		return c.text_edit_placeholder(ui, label, initial, &st, transmute(u32)flags, placeholder, &cs)
	}
	return c.text_edit(ui, label, initial, &st, transmute(u32)flags, &cs)
}

// -- Bodies: a procedure that builds content in a stock frame -----------------

// A titlebar hosting custom content (tabs, a search field): body builds it.
titlebar_with :: proc(ui: ^Ui, data: ^$T, body: proc(data: ^T, ui: ^Ui)) {
	Body :: struct {
		data: ^T,
		body: proc(data: ^T, ui: ^Ui),
		ctx:  Saved_Context,
	}
	call :: proc "c" (user: rawptr, ui: ^c.Ctx) {
		b := (^Body)(user)
		context = b.ctx
		b.body(b.data, ui)
	}
	b := Body{data, body, context}
	c.titlebar_with(ui, call, &b)
}

// A hint that always draws, around content body builds.
tooltip_with :: proc(ui: ^Ui, data: ^$T, body: proc(data: ^T, ui: ^Ui)) {
	Body :: struct {
		data: ^T,
		body: proc(data: ^T, ui: ^Ui),
		ctx:  Saved_Context,
	}
	call :: proc "c" (user: rawptr, ui: ^c.Ctx) {
		b := (^Body)(user)
		context = b.ctx
		b.body(b.data, ui)
	}
	b := Body{data, body, context}
	c.tooltip_with(ui, call, &b)
}

// -- The devtools panel -------------------------------------------------------

// A tab of the app's own in the devtools panel, whose content is built only
// while the tab is on show: `if` it, build inside, and the block's end closes
// the node - when there is one, since a tab not on show opens nothing.
@(deferred_in_out = close_devtools_tab, require_results)
devtools_tab_open :: proc(ui: ^Ui, name, label: string) -> bool {
	return c.devtools_tab_open(ui, name, label)
}

@(private)
close_devtools_tab :: proc(ui: ^Ui, _, _: string, opened: bool) {
	if opened do c.close(ui)
}

// -- Components (ADR 0060, ADR 0067) ------------------------------------------

// How a placement was filled (kui.h's KUI_FILL_*): what component and
// component_with answer.
FILL_NONE :: c.FILL_NONE // nothing provides it, or outside a frame
FILL_REPLAYED :: c.FILL_REPLAYED // last frame's nodes, pushed again; the extension not asked
FILL_NOT_KEPT :: c.FILL_NOT_KEPT // nothing kept: its first frame, fresh, a root, or a frame skipped it
FILL_PARAMS :: c.FILL_PARAMS // the props differ from the kept fill's
FILL_READS :: c.FILL_READS // a fact of the frame the fill read has moved
FILL_NOT_REPLAYABLE :: c.FILL_NOT_REPLAYABLE // it declared of the frame, or failed
FILL_HEARD :: c.FILL_HEARD // the instance heard one of its own events since it was filled

// A component an extension provides, placed here and keyed by where it is
// (or by "key" in props): name is namespace/component ("namespace/root" is
// that extension's view, drawn fresh every frame), props what its fill
// reads - "fresh": true asks every frame, and its on* entries are the
// messages its events arrive as. An unchanged instance is replayed without
// asking the extension. Answers how it was filled, one of the FILL_* codes
// below (FILL_NONE, -1, when nothing provides it: unknown-component).
// Rust: Ui::component.
component :: proc(ui: ^Ui, name: string, props: any = nil) -> i32 {
	scratch: Scratch
	defer scratch_free(&scratch)
	return c.component(ui, name, borrow(&scratch, props), nil, nil)
}

// A branch built only when it can be shown (ADR 0069): render runs when the
// parent offers need_w x need_h (0 for an axis it does not name) - laid out
// at exactly that size - or, with query a map of conditions (width / height
// {below, above, band}, window, dark, ...), while they hold over the
// parent's size, the window's or the environment's; fallback (nil for none)
// runs otherwise, and neither before the first layout, when rerun_owed asks
// for the frame again. on_query is the message the decision's changes
// arrive as. Returns 1 for render, 0 for fallback, -1 for neither.
lazy :: proc(
	ui: ^Ui,
	data: ^$T,
	render: proc(data: ^T, ui: ^Ui),
	fallback: proc(data: ^T, ui: ^Ui) = nil,
	need_w: f32 = 0,
	need_h: f32 = 0,
	query: any = nil,
	on_query: any = nil,
) -> i32 {
	Body :: struct {
		data:     ^T,
		render:   proc(data: ^T, ui: ^Ui),
		fallback: proc(data: ^T, ui: ^Ui),
		ctx:      Saved_Context,
	}
	call_render :: proc "c" (user: rawptr, ui: ^c.Ctx) {
		b := (^Body)(user)
		context = b.ctx
		b.render(b.data, ui)
	}
	call_fallback :: proc "c" (user: rawptr, ui: ^c.Ctx) {
		b := (^Body)(user)
		context = b.ctx
		if b.fallback != nil do b.fallback(b.data, ui)
	}
	b := Body{data, render, fallback, context}
	scratch: Scratch
	defer scratch_free(&scratch)
	return c.lazy(
		ui,
		need_w,
		need_h,
		borrow(&scratch, query),
		borrow(&scratch, on_query),
		call_render,
		call_fallback,
		&b,
	)
}

// component with your own children: children builds the group the component
// asks for (an empty group is the unnamed one), under your origin, keyed by
// the instance and the group.
component_with :: proc(ui: ^Ui, name: string, props: any, data: ^$T, children: proc(data: ^T, ui: ^Ui, group: string)) -> i32 {
	Body :: struct {
		data:     ^T,
		children: proc(data: ^T, ui: ^Ui, group: string),
		ctx:      Saved_Context,
	}
	call :: proc "c" (user: rawptr, ui: ^c.Ctx, group: string) {
		b := (^Body)(user)
		context = b.ctx
		b.children(b.data, ui, group)
	}
	b := Body{data, children, context}
	scratch: Scratch
	defer scratch_free(&scratch)
	return c.component(ui, name, borrow(&scratch, props), call, &b)
}
