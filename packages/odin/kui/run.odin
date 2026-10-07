package kui

import "base:runtime"
import c "c"

// The ABI this binding was generated against; run and new_ui refuse a
// library that implements another (kui.h, "ABI version": equality, not >=).
ABI_VERSION :: c.ABI_VERSION

abi_ok :: proc() -> bool {
	return c.abi_version() == ABI_VERSION
}

// The Odin context a C callback restores. Named outside run so the import
// is used where the checker sees it: run's body is only checked once
// something instantiates it.
@(private)
Saved_Context :: runtime.Context

// Opens a window and runs the app until it closes: `view` rebuilds the tree
// from `state` every frame, and `on_event` moves `state` for each event. The
// temp allocator is freed after each call, so a view may fmt.tprint freely.
//
//     state: State
//     kui.run("Counter", &state, view, on_event)
//
// `teardown` runs as the window goes for good (its close button, Quit), the
// place to save what would be lost; on a Mac's Quit the process ends after
// it without run returning. Pass `ui` (from new_ui) to bring the fonts,
// images and settings registered on it into the window.
run :: proc(
	title: string,
	state: ^$S,
	view: proc(state: ^S, ui: ^Ui),
	on_event: proc(state: ^S, ev: Event) = nil,
	config: Run_Config = {},
	teardown: proc(state: ^S) = nil,
	ui: ^Ui = nil,
) -> bool {
	if !abi_ok() do return false
	App :: struct {
		state:    ^S,
		view:     proc(state: ^S, ui: ^Ui),
		on_event: proc(state: ^S, ev: Event),
		teardown: proc(state: ^S),
		ctx:      Saved_Context,
	}
	view_c :: proc "c" (user: rawptr, ui: ^c.Ctx) {
		app := (^App)(user)
		context = app.ctx
		app.view(app.state, ui)
		free_all(context.temp_allocator)
	}
	event_c :: proc "c" (user: rawptr, ev: ^c.Event) {
		app := (^App)(user)
		context = app.ctx
		if app.on_event != nil do app.on_event(app.state, event_from_c(ev^))
		free_all(context.temp_allocator)
	}
	teardown_c :: proc "c" (user: rawptr) {
		app := (^App)(user)
		context = app.ctx
		if app.teardown != nil do app.teardown(app.state)
	}
	app := App{state, view, on_event, teardown, context}
	cfg := transmute(c.RunConfig)config
	c.on_teardown(teardown_c)
	return c.run_with(ui, title, &cfg, view_c, event_c, &app)
}

// -- Without a window -------------------------------------------------------
//
// The same tree, driven by hand: what a self-check or a host with its own
// renderer does. Input goes in through the input_* doors or the helpers
// below; events come out of poll_event, which a for loop drains:
//
//     for ev in kui.poll_event(ui) { on_event(&state, ev) }

// A context of your own (nil when the library's ABI differs); free it with
// free_ui. Hand it to run to open a window with what was registered on it.
new_ui :: proc() -> ^Ui {
	if !abi_ok() do return nil
	return c.ctx_new()
}

free_ui :: proc(ui: ^Ui) {c.ctx_free(ui)}

// One frame: a w x h logical-px viewport at `scale`, built by view.
frame :: proc(ui: ^Ui, state: ^$S, view: proc(state: ^S, ui: ^Ui), w: f32 = 800, h: f32 = 600, scale: f32 = 1) {
	frame_begin(ui, w, h, scale)
	view(state, ui)
	frame_finish(ui)
}

// A press and release at (x, y), logical px.
click :: proc(ui: ^Ui, x, y: f32, button := Mouse.Primary) {
	input_cursor(ui, x, y)
	input_mouse_button(ui, true, button, 1)
	input_mouse_button(ui, false, button, 1)
}

// A whole key, as a keyboard sends it: "escape", "enter", "a".
press :: proc(ui: ^Ui, code: string, kmods: Key_Mods = {}, text := "") {
	input_press(ui, code, "", kmods, text, false)
	input_release(ui, code, "", kmods)
}

// The node assistive technology names `name` in the last frame, where it
// is: how a self-check finds a button without knowing the layout.
find :: proc(ui: ^Ui, name: string) -> (key: u64, r: Rect, ok: bool) {
	for node in access_tree(ui) {
		if node.name == name do return node.key, {node.x, node.y, node.w, node.h}, true
	}
	return
}

Rect :: struct {
	x, y, w, h: f32,
}

center :: proc "contextless" (r: Rect) -> (x, y: f32) {
	return r.x + r.w / 2, r.y + r.h / 2
}
