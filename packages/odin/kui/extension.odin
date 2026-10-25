package kui

import "base:runtime"
import c "c"

// An Odin plugin: a shared library a host loads into its own frame, which
// provides components the host places (docs/adr/0060, 0067), keeps its own
// state and hears its own events (kui.h's "Extension ABI"). The host may be
// Rust, C, Node, Lua or Odin; the plugin does not know which.
//
// The contract is up to eight C symbols the plugin exports, two required.
// Odin cannot export from a generic, so a plugin writes them itself, each a
// line calling the procedure here that does the work:
//
//     State :: struct { ... }
//     COMPONENTS := [?]string{"panel", "save"}
//     CONTRIBUTIONS := [?]kui.Ext_Contribution{{slot = "app.toolbar", component = "save"}}
//
//     @(export) kui_ext_abi        :: proc "c" () -> u32 { return kui.ABI_VERSION }
//     @(export) kui_ext_name       :: proc "c" () -> cstring { return "my panel" }
//     @(export) kui_ext_components :: proc "c" (count: ^uint) -> [^]string { return kui.ext_components(COMPONENTS[:], count) }
//     @(export) kui_ext_contributions :: proc "c" (count: ^uint) -> [^]kui.Ext_Contribution { return kui.ext_contributions(CONTRIBUTIONS[:], count) }
//     @(export) kui_ext_init       :: proc "c" () -> rawptr { return kui.ext_init(State, init) }
//     @(export) kui_ext_view       :: proc "c" (user: rawptr, ui: ^kui.Ui) { kui.ext_view(user, ui, view) }
//     @(export) kui_ext_on_event   :: proc "c" (user: rawptr, ev: ^kui.Ext_Event) { kui.ext_on_event(user, ev, on_event) }
//     @(export) kui_ext_free       :: proc "c" (user: rawptr) { kui.ext_free(State, user) }
//
// A plugin without kui_ext_components provides only "root", its view, drawn
// once after the host's view unless the host places "<namespace>/root". One
// without kui_ext_contributions adds nothing to anyone's slots.
//
// view and on_event have run's shapes: proc(state: ^State, ui: ^kui.Ui) and
// proc(state: ^State, ev: kui.Event). Inside view, instance_name,
// instance_namespace and instance_params say which component this instance
// is and what the host placed it with, and instance_slot which slot it was
// contributed to (none for a placement by name); children(ui, group) puts the host's
// children. Inside on_event, emit(ev, prop, fields) raises one of the
// instance's events: whoever placed it hears the message it passed in that
// prop - the one way a plugin speaks.
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

// kui_ext_components: the components the plugin provides, from an array
// that outlives the plugin (a package-level one). Empty provides only
// "root", drawn once after the host's view.
ext_components :: proc "contextless" (components: []string, count: ^uint) -> [^]string {
	count^ = uint(len(components))
	return raw_data(components)
}

// One thing the plugin adds to someone else's slot (ADR 0067): one of its
// own components, placed wherever a host declares `slot` (slot(ui, name,
// props)), with the slot's props. `order` places it among the slot's other
// contributions, lower first, ties in load order.
Ext_Contribution :: c.Contribution

// kui_ext_contributions: what the plugin contributes, from an array that
// outlives the plugin (a package-level one). Each component must be one the
// plugin provides, or the host refuses the load. Empty contributes nothing.
ext_contributions :: proc "contextless" (contributions: []Ext_Contribution, count: ^uint) -> [^]Ext_Contribution {
	count^ = uint(len(contributions))
	return raw_data(contributions)
}

// kui_ext_init: the plugin's state, new and set up by init.
ext_init :: proc "contextless" ($T: typeid, init: proc(state: ^T) = nil) -> rawptr {
	context = ext_context()
	state := new(T)
	if init != nil do init(state)
	return state
}

// kui_ext_view: one instance's fill, built by view into the host's frame. The
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
