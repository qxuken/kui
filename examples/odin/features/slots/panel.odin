// kui with Odin as the *extension*, not the host: a shared library a host
// loads into its own frame (docs/adr/0014). The same panel as
// examples/c/features/slots/panel.c and the Lua one, deliberately - the
// extension contract is the contract, and the language is a detail. It
// draws into the slot the host declares as "<namespace>/panel", keeps its
// todos, hears its own clicks (the host never sees them), and answers a
// toggle with the reply the host asked for in the slot's params.
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
	todos:       [dynamic]Todo,
	// The `kind` of the reply the host asked for on a toggle, copied out of
	// the slot's params each frame: they are borrowed for the view, and the
	// reply is made from on_event. Empty: the host wants no reply.
	toggle_kind: string,
}

// The panel's own messages: they come back to on_event because the host
// tagged these nodes with the panel's origin, not its own.
Toggle :: struct {
	index: int,
}
Add :: struct {}
Clear :: struct {}
Msg :: union {
	Toggle,
	Add,
	Clear,
}

// The reply: the host's template's kind, with what changed. A `kind` field
// stands in for the type's own name.
Toggled :: struct {
	kind:  string,
	index: int,
	done:  bool,
}

// -- the contract: seven exports, each one line -------------------------------

SLOTS := [?]string{"panel"}

@(export)
kui_ext_abi :: proc "c" () -> u32 {return kui.ABI_VERSION}
@(export)
kui_ext_name :: proc "c" () -> cstring {return "odin panel"}
@(export)
kui_ext_slots :: proc "c" (count: ^uint) -> [^]string {return kui.ext_slots(SLOTS[:], count)}
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
	delete(p.toggle_kind)
}

view :: proc(p: ^Panel, ui: ^kui.Ui) {
	// What the host passed with the slot, borrowed for this call: a title,
	// and the template of the reply it wants on a toggle. Read by field, as
	// any host's map is; a host's own kind for it means nothing here.
	params := kui.slot_params(ui)
	title, has_title := kui.as_string(kui.get(params, "title"))
	if !has_title do title = "odin panel"
	delete(p.toggle_kind)
	p.toggle_kind = strings.clone(kui.kind(kui.get(params, "on_toggle")))

	// Opened where the host declared the slot. Its colours are the *host's*
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

on_event :: proc(p: ^Panel, ev: kui.Event) {
	switch m in kui.message(ev, Msg) {
	case Toggle:
		if m.index < 0 || m.index >= len(p.todos) do return
		p.todos[m.index].done = !p.todos[m.index].done
		// The host asked to hear about this: its template, filled in.
		if p.toggle_kind != "" {
			kui.reply(ev, Toggled{p.toggle_kind, m.index, p.todos[m.index].done})
		}
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
	// {kind = "changed"} from the filter field lands here too and needs
	// nothing: view reads the field back every frame.
}
