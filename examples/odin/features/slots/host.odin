// Odin as the host of an extension: the twin of examples/c/features/slots/host.c.
// It loads a plugin under a namespace of its own choosing, declares that
// plugin's slot in the middle of its own view, and counts what comes back.
// The plugin may be the Odin panel beside this file or the C one - the
// same contract, and the host cannot tell which it loaded.
//
//   nu scripts/odin.nu test                      builds the Odin panel and runs this --headless
//   ./target/odin/host                           a window, with target/odin/panel.so
//   ./target/odin/host --headless target/debug/panel.so   the C panel (cbuild builds it)
//
// --headless is the contract without a display: a frame, then a click on a
// row the plugin drew, and checks that the click reached the plugin and not
// the host, and that the plugin's reply reached the host with its origin
// and its slot on it.
package host

import "core:fmt"
import "core:os"
import "core:strings"
import kui "../../../../packages/odin/kui"

// The namespace the panel is loaded under: ours to choose, and what fronts
// its slot's name. The plugin calls its slot "panel".
NS :: "todos"
PANEL_SLOT :: NS + "/panel"

// Where `nu scripts/odin.nu` leaves the Odin panel.
DEFAULT_PLUGIN :: "target/odin/panel.dll" when ODIN_OS == .Windows else "target/odin/panel.so"

Host :: struct {
	clicks:     int, // our own button
	toggles:    int, // replies from the panel
	reply_from: u16,
	reply_slot: u64, // whose slot the reply is about
}

Count :: struct {}
// The reply the panel sends on a toggle, in the shape we ask for below.
Toggled :: struct {
	index: int,
	done:  bool,
}
Msg :: union {
	Count,
	Toggled,
}

// What we pass with the slot: the panel's title, and the template of the
// reply we want back.
Panel_Params :: struct {
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
	}

	// The plugin's half, in place: a position among our own children,
	// filled then and there by whoever we loaded under `todos`.
	kui.slot(ui, PANEL_SLOT, Panel_Params{title = "todos, from Odin", on_toggle = Toggled{}})
}

on_event :: proc(h: ^Host, ev: kui.Event) {
	// Our own nodes come back with origin 0; a reply carries the origin of
	// the extension that made it, which is what tells the two apart.
	switch m in kui.message(ev, Msg) {
	case Count:
		if ev.origin == 0 do h.clicks += 1
	case Toggled:
		h.toggles += 1
		h.reply_from = ev.origin
		// And which slot: one plugin may fill several.
		h.reply_slot = ev.slot
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

headless :: proc(plugin: string) -> bool {
	h: Host
	ui := load(plugin)
	if ui == nil do return false
	defer kui.free_ui(ui)
	fmt.printfln("loaded %d extension(s)", kui.ctx_extension_count(ui))

	kui.frame(ui, &h, view, 900, 600)
	x, y, found := find_row(ui, 1)
	if !found {
		fmt.eprintln("FAIL: the panel drew no rows - did the slot fill?")
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
		fmt.eprintfln("FAIL: expected one reply from the panel, got %d", h.toggles)
		ok = false
	}
	ns, ns_ok := kui.ctx_extension_namespace(ui, h.reply_from)
	if ok && !ns_ok {
		fmt.eprintfln("FAIL: the reply's origin %d names no extension", h.reply_from)
		ok = false
	}
	if ok && h.reply_slot != kui.key_of(ui, PANEL_SLOT) {
		fmt.eprintfln("FAIL: the reply names slot %x, not %s's", h.reply_slot, PANEL_SLOT)
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
	if ok {
		fmt.printfln("ok: the slot filled, the click reached the plugin and not the host, and its reply came back from `%s`", ns)
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
