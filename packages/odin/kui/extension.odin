package kui

import "base:runtime"
import c "c"

// An Odin plugin: a shared library a host loads into its own frame, which
// draws where the host declares a slot, keeps its own state and hears its
// own events (kui.h's "Extension ABI", docs/adr/0014). The host may be
// Rust, C, Node, Lua or Odin; the plugin does not know which.
//
// The contract is seven C symbols the plugin exports, two required. Odin
// cannot export from a generic, so a plugin writes the seven itself, each a
// line calling the procedure here that does the work:
//
//     State :: struct { ... }
//     SLOTS := [?]string{"panel"}
//
//     @(export) kui_ext_abi      :: proc "c" () -> u32 { return kui.ABI_VERSION }
//     @(export) kui_ext_name     :: proc "c" () -> cstring { return "my panel" }
//     @(export) kui_ext_slots    :: proc "c" (count: ^uint) -> [^]string { return kui.ext_slots(SLOTS[:], count) }
//     @(export) kui_ext_init     :: proc "c" () -> rawptr { return kui.ext_init(State, init) }
//     @(export) kui_ext_view     :: proc "c" (user: rawptr, ui: ^kui.Ui) { kui.ext_view(user, ui, view) }
//     @(export) kui_ext_on_event :: proc "c" (user: rawptr, ev: ^kui.Ext_Event) { kui.ext_on_event(user, ev, on_event) }
//     @(export) kui_ext_free     :: proc "c" (user: rawptr) { kui.ext_free(State, user) }
//
// view and on_event have run's shapes: proc(state: ^State, ui: ^kui.Ui) and
// proc(state: ^State, ev: kui.Event). Inside them, slot_name, slot_namespace
// and slot_params say which slot this is and what the host passed with it,
// and reply(ev, msg) answers the host from an event.
//
// Build it with -build-mode:shared -define:KUI_PLUGIN=true: on macOS and
// Linux that links no kui at all, and every kui_* resolves from the host at
// load (`nu scripts/odin.nu` builds examples/odin/features/slots/panel.odin
// this way).

// The context a plugin's call runs in: the default one, a fresh temp
// allocator's worth, since the host's Odin context (if it has one) is not
// ours to borrow.
@(private)
ext_context :: proc "contextless" () -> runtime.Context {
	return runtime.default_context()
}

// The event kui_ext_on_event receives, as C hands it over.
Ext_Event :: c.Event

// kui_ext_slots: the slots the plugin fills, from an array that outlives
// the plugin (a package-level one). Empty fills "root", once after the
// host's view.
ext_slots :: proc "contextless" (slots: []string, count: ^uint) -> [^]string {
	count^ = uint(len(slots))
	return raw_data(slots)
}

// kui_ext_init: the plugin's state, new and set up by init.
ext_init :: proc "contextless" ($T: typeid, init: proc(state: ^T) = nil) -> rawptr {
	context = ext_context()
	state := new(T)
	if init != nil do init(state)
	return state
}

// kui_ext_view: one slot's fill, built by view into the host's frame. The
// context is the host's for this call only; the temp allocator is freed
// after it.
ext_view :: proc "contextless" (user: rawptr, ui: ^Ui, view: proc(state: ^$T, ui: ^Ui)) {
	context = ext_context()
	view((^T)(user), ui)
	free_all(context.temp_allocator)
}

// kui_ext_on_event: one event of the plugin's own nodes, payload borrowed
// for the call.
ext_on_event :: proc "contextless" (user: rawptr, ev: ^Ext_Event, on_event: proc(state: ^$T, ev: Event)) {
	context = ext_context()
	on_event((^T)(user), event_from_c(ev^))
	free_all(context.temp_allocator)
}

// kui_ext_free: the state, at unload, after teardown if there is one.
ext_free :: proc "contextless" ($T: typeid, user: rawptr, teardown: proc(state: ^T) = nil) {
	context = ext_context()
	state := (^T)(user)
	if teardown != nil do teardown(state)
	free(state)
}
