// The host loop, without a window (backlog F238): what a host that brings
// its own window runs every turn - kui.h's "The host loop": build, drain,
// draw, then input and drain again - with nothing from the runner. It
// links against either library the release ships: the one with the runner
// and the one without (-define:KUI_CORE=true in the pack), which
// pack-ffi.nu's verify links it against.
//
//   nu scripts/odin.nu run host_loop      exit 0 on a clean loop
//   nu scripts/odin.nu test               runs it with --headless
package host_loop

import "core:fmt"
import "core:os"
import kui "../../../packages/odin/kui"

Go :: struct {}

fails := 0

check :: proc(ok: bool, what: string) {
	if !ok {
		fmt.eprintln("FAIL:", what)
		fails += 1
	}
}

State :: struct {}

// A panel holding a button, on a transparent root.
view :: proc(_: ^State, ui: ^kui.Ui) {
	if kui.box(ui, {width = kui.px(160), height = kui.px(80), pad = kui.pad(8), bg = 0x202020cc, radius = 6}) {
		if kui.box(ui, {width = kui.px(60), height = kui.px(24), bg = 0x3a6ff0ff, radius = 4, label = "go", on_click = Go{}}) {
			kui.text(ui, "go", {size = 13, color = 0xffffffff})
		}
	}
}

// Drains the queue; how many clicks said Go.
drain :: proc(ui: ^kui.Ui) -> (clicks: int) {
	for ev in kui.poll_event(ui) {
		if ev.event == "click" {
			if _, ok := kui.message(ev, Go); ok do clicks += 1
		}
	}
	return
}

main :: proc() {
	ui := kui.new_ui()
	if ui == nil {
		fmt.eprintln("FAIL: libkui_ffi's ABI is not the one this binding was generated against")
		os.exit(1)
	}
	defer kui.free_ui(ui)
	s: State

	// Build, drain, draw.
	kui.frame(ui, &s, view, 320, 200, 2)
	drain(ui)
	dd, ok := kui.draw_data(ui)
	check(ok && len(dd.quads) > 0, "the frame drew")
	check(dd.bounds == {0, 0, 320, 160}, "the bounds are the panel's, in physical px")

	// Input: the pointer over the button is the UI's, then a click.
	kui.input_cursor(ui, 38, 20)
	check(kui.pointer_wants(ui) && kui.hit(ui, 38, 20) != 0, "the button is under the pointer")
	kui.input_mouse(ui, true, 1)
	kui.input_mouse(ui, false, 1)
	check(drain(ui) == 1, "the click arrived as its message")
	kui.input_cursor(ui, 300, 180)
	check(!kui.pointer_wants(ui), "past the panel is the world's")

	if fails > 0 {
		fmt.eprintfln("host_loop: %d check(s) failed", fails)
		os.exit(1)
	}
	fmt.printfln("host loop OK: %d quads, bounds %dx%d", len(dd.quads), dd.bounds[2], dd.bounds[3])
}
