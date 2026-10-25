// Odin as the host of an extension: the twin of examples/c/features/slots/host.c.
// It loads a plugin under a namespace of its own choosing, places that
// plugin's `panel` component in the middle of its own view (docs/adr/0067)
// with a title and the message it wants on a toggle, and counts what comes
// back. The plugin may be the Odin panel beside this file or the C one -
// the same contract, and the host cannot tell which it loaded. It also
// declares a contribution slot, "todos.toolbar": it names no plugin there,
// and whatever the loaded plugins contribute to it - the panel's chip - is
// placed in its column, a press on it arriving as the host's on_press with
// Event.from naming who raised it.
//
//   nu scripts/odin.nu test                      builds the Odin panel and runs this --headless
//   ./target/odin/host                           a window, with target/odin/panel.so
//   ./target/odin/host --headless target/debug/panel.so   the C panel (cbuild builds it)
//
// --headless is the contract without a display: a frame, then a click on a
// row the plugin drew, and checks that the click reached the plugin and not
// the host, and that the plugin's on_toggle reached the host as its own
// message, about the panel's instance. A second frame with nothing moved
// replays the panel without asking the plugin.
package host

import "core:fmt"
import "core:os"
import "core:strings"
import kui "../../../../packages/odin/kui"

// The namespace the panel is loaded under: ours to choose, and what fronts
// its component's name. The plugin calls its component "panel".
NS :: "todos"
PANEL :: NS + "/panel"

// Where `nu scripts/odin.nu` leaves the Odin panel.
DEFAULT_PLUGIN :: "target/odin/panel.dll" when ODIN_OS == .Windows else "target/odin/panel.so"

Host :: struct {
	clicks:      int, // our own button
	toggles:     int, // the panel's on_toggle, heard as our own message
	toggled:     int, // which row it said
	toggle_from: u64, // the instance the event is about: Event.key
	panel_fill:  i32, // how the last frame filled the panel: a kui.FILL_* code
	toolbar:     int, // how many contributions the toolbar slot placed
	chipped:     int, // the slot's on_press, from whichever contributor
	chip_from:   string, // who raised the last one: Event.from, cloned
}

Count :: struct {}
// What the panel says with its on_toggle, under the message's `toggle`.
Toggle_Data :: struct {
	index: int,
	done:  bool,
}
// The message we pass for on_toggle: {kind = "toggled"}. The plugin's
// fields come back in `toggle`; a field named `_` is never written, so the
// declaration carries none of them.
Toggled :: struct {
	using _: struct {
		toggle: Toggle_Data,
	},
}
// The message every contribution to the toolbar raises: {kind = "chipped"}.
Chipped :: struct {}
Msg :: union {
	Count,
	Toggled,
	Chipped,
}

// What we place the panel with: a key (so the instance is keyed by name,
// not position), its title, and the message we want back on a toggle.
Panel_Props :: struct {
	key:       string,
	title:     string,
	on_toggle: Toggled,
}

view :: proc(h: ^Host, ui: ^kui.Ui) {
	kui.root(ui, {dir = .Row, width = kui.GROW, height = kui.GROW, pad = kui.pad(16), gap = 16})

	// Our half. Its colours are theme roles, so the host and the plugin it
	// loads agree on what a surface is without either being told.
	t := kui.theme(ui)
	if kui.column(ui, {width = kui.GROW, height = kui.GROW, pad = kui.pad(24), gap = 12, bg = t.surface, radius = 10}) {
		kui.text(ui, "the host, in Odin", {size = 18})
		kui.text(ui, fmt.tprintf("clicks %d, toggles heard %d", h.clicks, h.toggles), {size = 13, color = t.muted})
		kui.button(ui, "count", Count{})
		// A contribution slot (ADR 0067): a place, not a plugin. Whatever
		// the loaded extensions contribute to it lands here, each with
		// these props; the event's `from` says which raised it.
		h.toolbar = kui.slot(ui, "todos.toolbar", struct {
			on_press: Chipped,
		}{})
	}

	// The plugin's half, in place: a component among our own children,
	// drawn then and there by whoever we loaded under `todos` - and
	// replayed while nothing it read moved (ADR 0067).
	h.panel_fill = kui.component(ui, PANEL, Panel_Props{key = PANEL, title = "todos, from Odin", on_toggle = Toggled{}})
}

on_event :: proc(h: ^Host, ev: kui.Event) {
	// Our own nodes come back with origin 0, and so do the messages we
	// passed a component, raised by the plugin with emit: `event` says
	// "component". The plugin's own clicks carry its origin and never
	// reach us.
	if ev.origin != 0 do return
	switch m in kui.message(ev, Msg) {
	case Count:
		h.clicks += 1
	case Toggled:
		if ev.event != "component" do return
		h.toggles += 1
		h.toggled = m.toggle.index
		// And which instance: one plugin may be placed many times.
		h.toggle_from = ev.key
	case Chipped:
		if ev.event != "component" do return
		h.chipped += 1
		// Borrowed like the payload, until the next poll: kept by copy.
		delete(h.chip_from)
		h.chip_from = strings.clone(ev.from)
	}
}

// A context with the plugin loaded, or nil with the reason printed: both
// ways of running start here.
load :: proc(plugin: string) -> ^kui.Ui {
	ui := kui.new_ui()
	if ui == nil do return nil
	if !kui.ctx_add_extension(ui, NS, plugin) {
		why, _ := kui.ctx_extension_error(ui)
		fmt.eprintln("kui:", why)
		fmt.eprintln("build it first: nu scripts/odin.nu test (the Odin panel) or cargo run -p kui-devtools --bin cbuild (the C one)")
		kui.free_ui(ui)
		return nil
	}
	return ui
}

// The centre of the first row the plugin drew that is not done yet: by
// origin and name, as a screen reader would find it.
find_row :: proc(ui: ^kui.Ui, origin: u32) -> (x, y: f32, ok: bool) {
	for n in kui.access_tree(ui) {
		if n.origin == origin && strings.has_prefix(n.name, "[ ") do return n.x + n.w / 2, n.y + n.h / 2, true
	}
	return
}

// The centre of the first node of `origin` named exactly `name`.
find_named :: proc(ui: ^kui.Ui, origin: u32, name: string) -> (x, y: f32, ok: bool) {
	for n in kui.access_tree(ui) {
		if n.origin == origin && n.name == name do return n.x + n.w / 2, n.y + n.h / 2, true
	}
	return
}

headless :: proc(plugin: string) -> bool {
	h: Host
	ui := load(plugin)
	if ui == nil do return false
	defer kui.free_ui(ui)
	fmt.printfln("loaded %d extension(s)", kui.ctx_extension_count(ui))

	kui.frame(ui, &h, view, 900, 600)
	if h.panel_fill != kui.FILL_NOT_KEPT {
		fmt.eprintfln("FAIL: the first frame did not fill the panel (%d)", h.panel_fill)
		return false
	}
	// A second frame with nothing moved: the kept fill is replayed.
	kui.frame(ui, &h, view, 900, 600)
	if h.panel_fill != kui.FILL_REPLAYED {
		fmt.eprintfln("FAIL: the second frame did not replay the panel (%d)", h.panel_fill)
		return false
	}
	x, y, found := find_row(ui, 1)
	if !found {
		fmt.eprintln("FAIL: the panel drew no rows - was it placed?")
		return false
	}
	// Click it, then build again so the press resolves into an event.
	kui.click(ui, x, y)
	kui.frame(ui, &h, view, 900, 600)
	for ev in kui.poll_event(ui) do on_event(&h, ev)

	ok := true
	if h.clicks != 0 {
		fmt.eprintfln("FAIL: the plugin's click reached the host (%d)", h.clicks)
		ok = false
	}
	if h.toggles != 1 {
		fmt.eprintfln("FAIL: expected the panel's on_toggle once, heard %d", h.toggles)
		ok = false
	}
	// Placed with `key`, the instance is found by that name like a keyed node.
	if ok && h.toggle_from != kui.key_of(ui, PANEL) {
		fmt.eprintfln("FAIL: the on_toggle names instance %x, not %s's", h.toggle_from, PANEL)
		ok = false
	}
	// Who drew the panel: the first extension, origin 1, named back.
	ns, ns_ok := kui.ctx_extension_namespace(ui, 1)
	if ok && !ns_ok {
		fmt.eprintln("FAIL: origin 1 names no extension")
		ok = false
	}
	// The panel kept its own state: the row it ticked reads done now.
	kui.frame(ui, &h, view, 900, 600)
	ticked := false
	for n in kui.access_tree(ui) do if n.origin == 1 && strings.has_prefix(n.name, "[x]") do ticked = true
	if ok && !ticked {
		fmt.eprintln("FAIL: the panel did not tick the row it was clicked on")
		ok = false
	}
	// The toolbar slot: one contribution, the panel's chip, found by the
	// name it took from the slot it was placed in - and its press, from
	// the namespace we loaded the panel under.
	if ok && h.toolbar != 1 {
		fmt.eprintfln("FAIL: the toolbar slot placed %d contributions, not 1", h.toolbar)
		ok = false
	}
	if cx, cy, chip_found := find_named(ui, 1, "chip in todos.toolbar"); ok && chip_found {
		kui.click(ui, cx, cy)
		kui.frame(ui, &h, view, 900, 600)
		for ev in kui.poll_event(ui) do on_event(&h, ev)
	} else if ok {
		fmt.eprintln("FAIL: the panel's chip is not in the toolbar slot")
		ok = false
	}
	if ok && (h.chipped != 1 || h.chip_from != NS) {
		fmt.eprintfln("FAIL: expected one on_press from %q, heard %d, last from %q", NS, h.chipped, h.chip_from)
		ok = false
	}
	delete(h.chip_from)
	if ok {
		fmt.printfln("ok: `%s/panel` placed and replayed, the click reached the plugin and not the host, its on_toggle (row %d) and the toolbar chip's press (from `%s`) arrived as our own messages", ns, h.toggled, NS)
	}
	return ok
}

main :: proc() {
	no_window := false
	plugin := DEFAULT_PLUGIN
	for arg in os.args[1:] {
		if arg == "--headless" do no_window = true
		else do plugin = arg
	}
	if no_window do os.exit(0 if headless(plugin) else 1)

	ui := load(plugin)
	if ui == nil do os.exit(1)
	h: Host
	// What is registered on the context - the plugin - reaches the window.
	ok := kui.run("kui - an Odin host and a panel", &h, view, on_event, {width = 720, height = 480, min_w = 360}, ui = ui)
	kui.free_ui(ui)
	os.exit(0 if ok else 1)
}
