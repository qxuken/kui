// Kit from Odin: the components addons/kit provides, placed through the
// package its schema generates (addons/kit/bindings/odin/kit, ADR 0066) —
// props structs, a proc each, and the event's data read back with the
// generated With_ types. The twin of examples/c/features/kit/kit.c.
//
//   nu scripts/odin.nu test             runs this --headless
//   nu scripts/odin.nu run kit          a window
package kit_host

import "core:fmt"
import "core:os"
import kit "../../../../addons/kit/bindings/odin/kit"
import kui "../../../../packages/odin/kui"

KIT_LUA :: "addons/kit/lua/kit.lua"

App :: struct {
	runs:    int,
	segment: string,
}

Run :: struct {}
Segment :: struct {
	using _: kit.With_Segmented_Change,
}
Msg :: union {
	Run,
	Segment,
}

view :: proc(app: ^App, ui: ^kui.Ui) {
	kui.root(ui, {dir = .Column, width = kui.GROW, height = kui.GROW, pad = kui.pad(12), gap = 10})
	kit.settings(ui, {skin = .Phosphor, density = .Compact})
	kit.toolbar_with(ui, {}, app, proc(app: ^App, ui: ^kui.Ui, group: string) {
		switch group {
		case "":
			kit.button(ui, {label = "Run", variant = .Primary, icon = "play", on_press = Run{}})
		case "trailing":
			kit.badge(ui, {label = "live", tone = .Ok})
		}
	})
	kit.segmented(ui, {options = {{value = "a", label = "Alpha"}, {value = "b", label = "Beta"}}, value = app.segment, on_change = Segment{}})
	kit.slider(ui, {value = 0.4, min = 0, max = 1, label = "Zoom"})
}

on_event :: proc(app: ^App, ev: kui.Event) {
	switch m in kui.message(ev, Msg) {
	case Run:
		app.runs += 1
	case Segment:
		app.segment = m.change.value == "b" ? "b" : "a"
	}
}

load :: proc() -> ^kui.Ui {
	ui := kui.new_ui()
	if ui == nil do return nil
	if !kui.ctx_add_extension(ui, kit.NAMESPACE, KIT_LUA) {
		why, _ := kui.ctx_extension_error(ui)
		fmt.eprintln("kui:", why)
		fmt.eprintln("run from the repository root, where", KIT_LUA, "is")
		kui.free_ui(ui)
		return nil
	}
	return ui
}

press :: proc(ui: ^kui.Ui, app: ^App, name: string) -> bool {
	for n in kui.access_tree(ui) {
		if n.name != name do continue
		kui.click(ui, n.x + n.w / 2, n.y + n.h / 2)
		kui.frame(ui, app, view, 900, 500)
		for ev in kui.poll_event(ui) do on_event(app, ev)
		return true
	}
	fmt.eprintln("FAIL: no node named", name)
	return false
}

headless :: proc() -> bool {
	app := App{segment = "a"}
	ui := load()
	if ui == nil do return false
	defer kui.free_ui(ui)
	kui.frame(ui, &app, view, 900, 500)
	kui.frame(ui, &app, view, 900, 500)
	if w := kui.take_warnings(ui); len(w) > 0 {
		fmt.eprintln("FAIL: warning", w[0].code, w[0].message)
		return false
	}
	if !press(ui, &app, "Run") || !press(ui, &app, "Beta") do return false
	if app.runs != 1 {
		fmt.eprintfln("FAIL: Run reached the host %d times", app.runs)
		return false
	}
	if app.segment != "b" {
		fmt.eprintln("FAIL: the segment's change did not carry b")
		return false
	}
	fmt.println("ok: Kit's toolbar, button and segmented control answered through the kit package")
	return true
}

main :: proc() {
	for arg in os.args[1:] do if arg == "--headless" do os.exit(0 if headless() else 1)
	ui := load()
	if ui == nil do os.exit(1)
	app := App{segment = "a"}
	ok := kui.run("Kit from Odin", &app, view, on_event, {width = 900, height = 500}, ui = ui)
	kui.free_ui(ui)
	os.exit(0 if ok else 1)
}
