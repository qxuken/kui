// kui with Odin as the *extension*, not the host: a shared library a host
// loads into its own frame (docs/adr/0067-slots-fold-into-components.md).
// The same panel as examples/c/features/slots/panel.c and the Lua one,
// deliberately - the extension contract is the contract, and the language
// is a detail. It provides two components: "panel", drawn where the host
// places "<namespace>/panel", which keeps its todos, hears its own clicks
// (the host never sees them) and raises the host's `on_toggle` with emit;
// and "badge", a dot the host places as often as it likes with its own
// children inside, whose press raises `onPress`. It also contributes a
// "chip" to the "todos.toolbar" slot: wherever a host declares that slot,
// the chip is placed there with the slot's props, and its press raises the
// slot's `on_press`, which the host hears with `from` naming this plugin.
//
// It links against nothing: built -build-mode:shared with KUI_PLUGIN, every
// kui_* stays undefined and resolves from the host at load. So the one
// target/odin/panel.so loads into every host:
//
//   nu scripts/odin.nu test                     builds it; host.odin drives it
//   ./target/odin/host target/odin/panel.so     an Odin host, a window
//   ./target/debug/host --headless target/odin/panel.so          the C host
//   cargo run -p kui-ffi --example c_panel -- --headless target/odin/panel.so
package panel

import "core:fmt"
import "core:strings"
import kui "../../../../packages/odin/kui"

Todo :: struct {
	text: string,
	done: bool,
}

Panel :: struct {
	todos:        [dynamic]Todo,
	presses:      int, // the badges' dots, pressed so far
	chip_presses: int, // the toolbar chip, pressed so far
}

// The panel's own messages: they come back to on_event because the host
// tagged these nodes with the panel's origin, not its own.
Toggle :: struct {
	index: int,
}
Add :: struct {}
Clear :: struct {}
Badge :: struct {}
Chip :: struct {}
Msg :: union {
	Toggle,
	Add,
	Clear,
	Badge,
	Chip,
}

// -- the contract: eight exports, each one line -------------------------------

COMPONENTS := [?]string{"panel", "badge", "chip"}
// What it adds to other people's slots (ADR 0067): a host names the slot,
// never this plugin, and every contributor to it is placed there.
CONTRIBUTIONS := [?]kui.Ext_Contribution{{slot = "todos.toolbar", component = "chip"}}

@(export)
kui_ext_abi :: proc "c" () -> u32 {return kui.ABI_VERSION}
@(export)
kui_ext_name :: proc "c" () -> cstring {return "odin panel"}
@(export)
kui_ext_components :: proc "c" (count: ^uint) -> [^]string {return kui.ext_components(COMPONENTS[:], count)}
@(export)
kui_ext_contributions :: proc "c" (count: ^uint) -> [^]kui.Ext_Contribution {return kui.ext_contributions(CONTRIBUTIONS[:], count)}
@(export)
kui_ext_init :: proc "c" () -> rawptr {return kui.ext_init(Panel, init)}
@(export)
kui_ext_view :: proc "c" (user: rawptr, ui: ^kui.Ui) {kui.ext_view(user, ui, view)}
@(export)
kui_ext_on_event :: proc "c" (user: rawptr, ev: ^kui.Ext_Event) {kui.ext_on_event(user, ev, on_event)}
@(export)
kui_ext_free :: proc "c" (user: rawptr) {kui.ext_free(Panel, user, teardown)}

// -- the panel --------------------------------------------------------------

init :: proc(p: ^Panel) {
	for text in ([]string{"ship the layout solver", "wire up wgpu", "write this panel"}) {
		append(&p.todos, Todo{text = text})
	}
}

teardown :: proc(p: ^Panel) {
	for t in p.todos do if strings.has_prefix(t.text, "todo #") do delete(t.text)
	delete(p.todos)
}

view :: proc(p: ^Panel, ui: ^kui.Ui) {
	switch name, _ := kui.instance_name(ui); name {
	case "badge":
		badge(ui)
		return
	case "chip":
		chip(p, ui)
		return
	}
	// What the host placed the panel with, borrowed for this call: a title.
	// Read by field, as any host's map is; a host's own kind for it means
	// nothing here. Its on_toggle is the host's business - emit names the
	// prop and the core finds the host's message for it.
	params := kui.instance_params(ui)
	title, has_title := kui.as_string(kui.get(params, "title"))
	if !has_title do title = "odin panel"

	// Opened where the host placed it. Its colours are the *host's*
	// theme (ADR 0019): a plugin is a guest in someone else's frame, and
	// reading the palette is how it looks like it belongs.
	t := kui.theme(ui)
	panel := kui.Spec {
		width        = kui.px(300),
		height       = kui.GROW,
		pad          = kui.pad(16),
		gap          = 10,
		bg           = t.surface,
		radius       = 10,
		border_w     = 1,
		border_color = t.border,
	}
	if kui.column(ui, panel) {
		left := 0
		for todo in p.todos do if !todo.done do left += 1
		kui.rich_text(ui, {{text = title}, {text = fmt.tprintf(" · %d left", left), color = t.muted}}, {size = 12, color = t.muted})

		// Declared before the list, so the filter it holds is readable in
		// time to build one; the core keeps the text under the key, so the
		// filter is a query and not state this file keeps.
		filter, _ := kui.edit_text(ui, kui.text_input(ui, "filter"))

		if kui.column(ui, {height = kui.GROW, overflow = {.Scroll_Y}}) {
			for todo, i in p.todos {
				if !strings.contains(todo.text, filter) do continue
				row := kui.Spec {
					dir         = .Row,
					gap         = 8,
					cross_align = .Center,
					pad         = {t = 4, b = 4},
					tooltip     = "click to reopen" if todo.done else "click to finish",
					on_click    = Toggle{i},
				}
				if kui.box(ui, row) {
					line := fmt.tprintf("%s %s", "[x]" if todo.done else "[ ]", todo.text)
					kui.text(ui, line, {size = 14, color = t.faint if todo.done else t.fg})
				}
			}
		}

		if kui.row(ui, {gap = 8}) {
			kui.button(ui, "add", Add{})
			kui.button(ui, "clear done", Clear{})
		}
	}
}

// One badge: its label from the props, a pressable dot, and the host's own
// children beside them - children puts them here, under the host's origin.
badge :: proc(ui: ^kui.Ui) {
	label, has_label := kui.as_string(kui.get(kui.instance_params(ui), "label"))
	if !has_label do label = "badge"
	t := kui.theme(ui)
	if kui.row(ui, {gap = 6, cross_align = .Center, pad = {l = 6, r = 6, t = 2, b = 2}, bg = t.surface, radius = 8}) {
		if kui.box(ui, {width = kui.px(12), height = kui.px(12), bg = t.accent, radius = 6, label = label, on_click = Badge{}}) {}
		kui.text(ui, label, {size = 12, color = t.fg})
		kui.children(ui, "")
	}
}

// The toolbar chip: a pressable pill named for the slot it was placed in -
// the name a host finds it by - saying how often it was pressed.
chip :: proc(p: ^Panel, ui: ^kui.Ui) {
	slot, in_slot := kui.instance_slot(ui)
	if !in_slot do slot = "no slot"
	t := kui.theme(ui)
	spec := kui.Spec {
		dir         = .Row,
		cross_align = .Center,
		pad         = {l = 8, r = 8, t = 2, b = 2},
		bg          = t.accent,
		radius      = 8,
		label       = fmt.tprintf("chip in %s", slot),
		on_click    = Chip{},
	}
	if kui.box(ui, spec) {
		kui.text(ui, fmt.tprintf("todos chip · %d", p.chip_presses), {size = 12, color = t.on_accent})
	}
}

on_event :: proc(p: ^Panel, ev: kui.Event) {
	switch m in kui.message(ev, Msg) {
	case Toggle:
		if m.index < 0 || m.index >= len(p.todos) do return
		p.todos[m.index].done = !p.todos[m.index].done
		// Raise on_toggle: the host hears the message it passed for it,
		// these fields under `toggle` - or nothing, when it passed none. An
		// anonymous struct, so the fields carry no `kind` of their own.
		kui.emit(ev, "on_toggle", struct {
			index: int,
			done:  bool,
		}{m.index, p.todos[m.index].done})
	case Badge:
		// A badge's dot: raise its onPress, with how many presses so far.
		p.presses += 1
		kui.emit(ev, "onPress", struct {
			presses: int,
		}{p.presses})
	case Chip:
		// The toolbar chip: raise the slot's on_press. Every contributor
		// gets the slot's props, so the host tells them apart by `from`.
		p.chip_presses += 1
		kui.emit(ev, "on_press", struct {
			presses: int,
		}{p.chip_presses})
	case Add:
		append(&p.todos, Todo{text = fmt.aprintf("todo #%d", len(p.todos) + 1)})
	case Clear:
		kept := 0
		for t in p.todos {
			if t.done {
				if strings.has_prefix(t.text, "todo #") do delete(t.text)
				continue
			}
			p.todos[kept] = t
			kept += 1
		}
		resize(&p.todos, kept)
	}
	// The filter field's edits land here too, as {change = {text}} - the
	// stock input names no message, so no kind, and Msg matches none of it -
	// and need nothing: view reads the field back every frame.
}
