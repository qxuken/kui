// The counter, from Odin: the one example every binding has in the same
// shape (docs/adr/0021, decision 3). `view` rebuilds the tree from the
// state; a click arrives in `on_event` as a message and moves the state; a
// right-click asks for a menu the next frame declares as a modal float.
//
//   nu scripts/odin.nu run counter              opens a window
//   nu scripts/odin.nu run counter --headless   no window: clicks +1 by its
//       name, right-clicks the card for the menu, dismisses it with Escape,
//       and exits non-zero on a wrong answer - the Rosetta drive every
//       counter runs.
package counter

import "core:fmt"
import "core:os"
import kui "../../../packages/odin/kui"

State :: struct {
	count: int,
	// Where the context menu is open; nil while it is not. The core opens
	// nothing: the view declares the menu while this is set.
	menu:  Maybe([2]f32),
}

// The messages, as types: each is {kind = "<snake_case name>"} on the wire,
// which is what the Rust counter's #[derive(Message)] enum sends too.
Inc :: struct {}
Dec :: struct {}
Add10 :: struct {}
Reset :: struct {}
Card :: struct {} // the tag on the card's context menu
Menu :: struct {} // the tag on the menu's dismiss

Msg :: union {
	Inc,
	Dec,
	Add10,
	Reset,
	kui.Context_Menu_Event,
	kui.Dismiss_Event,
}

view :: proc(s: ^State, ui: ^kui.Ui) {
	// Every colour is a role of the palette the core derived from the OS, so
	// the window follows light and dark with no branch of its own.
	t := kui.theme(ui)
	kui.root(ui, {width = kui.GROW, height = kui.GROW, main_align = .Center, cross_align = .Center, bg = t.bg})

	card := kui.Spec {
		gap             = 20,
		cross_align     = .Center,
		pad             = kui.pad(32),
		bg              = t.surface,
		radius          = 12,
		border_w        = 1,
		border_color    = t.border,
		on_context_menu = Card{}, // a right-click asks for the menu
	}
	if kui.column(ui, card) {
		kui.text(ui, "kui from Odin", {size = 14, color = t.muted})
		kui.text(ui, fmt.tprint(s.count), {size = 56})
		if kui.row(ui, {gap = 12}) {
			kui.button(ui, "-1", Dec{})
			kui.button(ui, "+1", Inc{})
		}
		kui.rich_text(
			ui,
			{
				{text = "same IR as Rust and "},
				{text = "Odin", color = t.success, flags = {.Bold}},
				{text = " — just typed", flags = {.Italic}},
			},
			{size = 13, color = t.faint},
		)
	}

	if at, open := s.menu.?; open do context_menu(ui, t, at)
}

// The menu the right-click asks for: a float at the press, modal so the
// Tab ring is its own and Escape or a press outside sends Dismiss.
context_menu :: proc(ui: ^kui.Ui, t: kui.Theme, at: [2]f32) {
	menu := kui.Spec {
		gap          = 4,
		width        = kui.px(120),
		pad          = kui.pad(4),
		bg           = t.raised,
		radius       = 6,
		border_w     = 1,
		border_color = t.border_strong,
		float        = {mode = .Viewport, dx = at.x, dy = at.y, fit = true},
		modal        = Menu{},
		label        = "Actions", // a modal is a dialog, named by its label
	}
	if kui.column(ui, menu) {
		kui.button(ui, "+10", Add10{})
		kui.button(ui, "Reset", Reset{}, {description = "Back to zero"})
	}
}

on_event :: proc(s: ^State, ev: kui.Event) {
	switch m in kui.message(ev, Msg) {
	case Inc:
		s.count += 1
	case Dec:
		s.count -= 1
	case Add10:
		s.count += 10
		s.menu = nil
	case Reset:
		s.count = 0
		s.menu = nil
	case kui.Context_Menu_Event:
		s.menu = [2]f32{m.x, m.y}
	case kui.Dismiss_Event:
		s.menu = nil
	}
}

teardown :: proc(s: ^State) {
	fmt.println("teardown at count", s.count)
}

// -- headless self-test: the whole loop without a window ---------------------

headless :: proc() -> bool {
	s: State
	ui := kui.new_ui()
	if ui == nil do return fail("libkui_ffi's ABI is not the one this binding was generated against")
	defer kui.free_ui(ui)

	// Frame 1 lays out, so the buttons have places to be clicked at.
	kui.frame(ui, &s, view)
	_, plus, found := kui.find(ui, "+1")
	if !found do return fail("no node named +1 in the access tree")
	// A primary press and release where it is, as a pointer sends them.
	kui.input_cursor(ui, kui.center(plus))
	kui.input_mouse(ui, true, 1)
	kui.input_mouse(ui, false, 1)
	got := drain(ui, &s)
	fmt.printfln("clicked +1: %d event(s), count = %d", got, s.count)
	if got != 1 || s.count != 1 do return fail("expected one click and a count of 1")

	// The secondary button over the card asks for the menu at the press, and
	// moves nothing else.
	x, y := kui.center(plus)
	kui.click(ui, x, y - 60, .Secondary)
	got = drain(ui, &s)
	at, open := s.menu.?
	fmt.printfln("right-clicked: %d event(s), menu at (%.0f, %.0f)", got, at.x, at.y)
	if got != 1 || !open || s.count != 1 do return fail("expected one contextmenu event and no click")

	// Declare it, then let Escape ask for it back.
	kui.frame(ui, &s, view)
	if _, _, ok := kui.find(ui, "Actions"); !ok do return fail("the menu was not declared")
	kui.press(ui, "escape")
	drain(ui, &s)
	if _, still := s.menu.?; still do return fail("escape did not dismiss the menu")
	fmt.println("escape dismissed the menu")

	// And a message with fields survives the round trip through the core.
	round_trip() or_return

	// The icon's refusals (backlog F86), which need no window: a size with
	// no pixels, and a resource id past 16 bits, keeping nothing.
	if kui.set_icon(nil, 32, 32, 0) || kui.set_icon(nil, 0, 0, 70000) || !kui.set_icon(nil, 0, 0, 0) {
		return fail("set_icon took what is not an icon")
	}
	fmt.println("set_icon refused a size with no pixels and a 17-bit resource")
	fmt.println("headless self-test OK")
	return true
}

drain :: proc(ui: ^kui.Ui, s: ^State) -> (n: int) {
	for ev in kui.poll_event(ui) {
		on_event(s, ev)
		n += 1
	}
	return
}

// Pick :: {kind = "pick", id, name, tags}: an app message with fields,
// encoded by one button's click and decoded from the event it comes back as.
Pick :: struct {
	id:   int,
	name: string,
	tags: []string,
	mode: Mode,
}
Mode :: enum {
	Fast,
	Careful,
}

round_trip :: proc() -> bool {
	ui := kui.new_ui()
	defer kui.free_ui(ui)
	sent := Pick{7, "seven", {"a", "b"}, .Careful}
	pick_view :: proc(p: ^Pick, ui: ^kui.Ui) {
		kui.root(ui, {width = kui.GROW, height = kui.GROW})
		kui.button(ui, "pick", p^)
	}
	kui.frame(ui, &sent, pick_view)
	_, r, _ := kui.find(ui, "pick")
	kui.click(ui, kui.center(r))
	for ev in kui.poll_event(ui) {
		got, ok := kui.message(ev, Pick)
		fmt.printfln("round trip: kind %q -> %v", kui.kind(ev.payload), got)
		if !ok || got.id != 7 || got.name != "seven" || len(got.tags) != 2 || got.tags[1] != "b" || got.mode != .Careful {
			return fail("Pick did not come back as it was sent")
		}
		return true
	}
	return fail("the pick button sent nothing")
}

// Every window's icon (backlog F86): a disc in the buttons' blue, drawn
// here rather than read from a file. Windows shows it in the title bar,
// Alt-Tab and the taskbar, X11 in the window manager's; a Mac has no window
// icon (the Dock draws the bundle's).
set_icon :: proc() -> bool {
	N :: 64
	@(static) px: [N * N * 4]u8
	for y in 0 ..< N {
		for x in 0 ..< N {
			dx, dy, r := f32(x) + 0.5 - N / 2, f32(y) + 0.5 - N / 2, f32(N) / 2 - 2
			// r - d, near the edge, is (r² - d²) / 2r: one pixel of antialiasing.
			a := clamp((r * r - dx * dx - dy * dy) / (2 * r), 0, 1)
			i := (y * N + x) * 4
			px[i], px[i + 1], px[i + 2], px[i + 3] = 0x3b, 0x82, 0xf6, u8(a * 255)
		}
	}
	return kui.set_icon(px[:], N, N, 0)
}

fail :: proc(what: string) -> bool {
	fmt.eprintln("FAIL:", what)
	return false
}

main :: proc() {
	if len(os.args) > 1 && os.args[1] == "--headless" {
		os.exit(0 if headless() else 1)
	}
	state: State
	if !set_icon() do os.exit(1)
	ok := kui.run("kui — Odin counter", &state, view, on_event, teardown = teardown)
	os.exit(0 if ok else 1)
}
