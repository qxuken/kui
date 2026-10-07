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
// without its close cannot be written.

// The root of the frame: the viewport, laid out by spec.
root :: proc(ui: ^Ui, spec: Spec = {}) {
	cs: c.Spec
	scratch: Scratch
	defer scratch_free(&scratch)
	spec_to_c(spec, &cs, &scratch)
	c.root(ui, &cs)
}

// A container: a box laid out by spec, holding what the block declares.
// Returns true, for the `if` that scopes it.
@(deferred_in = close)
box :: proc(ui: ^Ui, spec: Spec = {}) -> bool {
	open(ui, spec)
	return true
}

// A box whose children stack top to bottom (the default direction).
@(deferred_in = close)
column :: proc(ui: ^Ui, spec: Spec = {}) -> bool {
	s := spec
	s.dir = .Column
	open(ui, s)
	return true
}

// A box whose children sit side by side.
@(deferred_in = close)
row :: proc(ui: ^Ui, spec: Spec = {}) -> bool {
	s := spec
	s.dir = .Row
	open(ui, s)
	return true
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

checkbox :: proc(ui: ^Ui, label: string, checked: bool, msg: any = nil, spec: Spec = {}) -> u64 {
	s := spec
	s.checked = checked
	return stock(c.checkbox, ui, label, s, msg)
}

radio :: proc(ui: ^Ui, label: string, checked: bool, msg: any = nil, spec: Spec = {}) -> u64 {
	s := spec
	s.checked = checked
	return stock(c.radio, ui, label, s, msg)
}

// The stock switch (kui_switch; `switch` is Odin's).
toggle :: proc(ui: ^Ui, label: string, on: bool, msg: any = nil, spec: Spec = {}) -> u64 {
	s := spec
	s.checked = on
	return stock(c.switch_, ui, label, s, msg)
}

@(private)
stock :: proc(door: proc "c" (_: ^c.Ctx, _: string, _: ^c.Spec, _: ^c.Value) -> u64, ui: ^Ui, label: string, spec: Spec, msg: any) -> u64 {
	cs: c.Spec
	scratch: Scratch
	defer scratch_free(&scratch)
	spec_to_c(spec, &cs, &scratch)
	return door(ui, label, &cs, to_value(msg))
}

// A group of radios, one Tab stop with the arrows inside it.
@(deferred_in = close_radio_group)
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

// A slider at value in [lo, hi]. The core proposes changes as a
// Change_Event carrying msg as its tag; the app moves value, and the
// slider follows on the next frame.
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

// A one-line text field, keyed by label; read it with edit_text.
text_input :: proc(ui: ^Ui, label: string, initial := "") -> u64 {
	return c.text_input(ui, label, initial)
}

// The editor: one line or many (flags), styled, laid out by spec.
text_edit :: proc(ui: ^Ui, label: string, initial := "", flags: Edit_Flags = {}, style: Text_Style = {}, spec: Spec = {}) -> u64 {
	st := text_style_to_c(style)
	cs: c.Spec
	scratch: Scratch
	defer scratch_free(&scratch)
	spec_to_c(spec, &cs, &scratch)
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
@(deferred_in_out = close_devtools_tab)
devtools_tab_open :: proc(ui: ^Ui, name, label: string) -> bool {
	return c.devtools_tab_open(ui, name, label)
}

@(private)
close_devtools_tab :: proc(ui: ^Ui, _, _: string, opened: bool) {
	if opened do c.close(ui)
}
