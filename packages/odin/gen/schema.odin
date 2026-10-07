package gen

import "core:encoding/json"
import "core:os"

// `kui_core::schema` as `cargo run -p kui-core --example schema-dump` writes
// it (examples/rust/tools/schema-dump.rs).

Prop :: struct {
	name, snake: string,
	kind:        string, // f32 color flag enum sizing min max msg tag str family resource keyframes enter gradient
	values:      []string,
	target:      string, // spec | style
	c, doc:      string,
}

Custom :: struct {
	name, c, doc: string,
}

Door :: struct {
	rust:    string,
	c_cell:  string, // is | as | no
	c_text:  string,
	doc:     string,
}

Event_Def :: struct {
	kind, payload, doc: string,
}

Named :: struct {
	name, c, doc: string,
}

Schema :: struct {
	props:    [dynamic]Prop,
	custom:   [dynamic]Custom,
	elements: [dynamic]Named,
	events:   [dynamic]Event_Def,
	doors:    [dynamic]Door,
	theme:    [dynamic]Named,
	metrics:  [dynamic]Named,
	env:      [dynamic]Named,
	lists:    map[string][]string,
}

load_schema :: proc(path: string) -> (s: Schema) {
	data, err := os.read_entire_file(path, context.allocator)
	if err != nil do fail("cannot read %s: %v", path, err)
	root, perr := json.parse(data)
	if perr != .None do fail("%s: bad json: %v", path, perr)
	o := root.(json.Object)

	for v in o["props"].(json.Array) {
		p := v.(json.Object)
		append(&s.props, Prop{
			name = str(p, "name"), snake = str(p, "snake"), kind = str(p, "kind"),
			values = strings_of(p["values"]), target = str(p, "target"), c = str(p, "c"), doc = str(p, "doc"),
		})
	}
	for v in o["custom"].(json.Array) {
		p := v.(json.Object)
		append(&s.custom, Custom{str(p, "name"), str(p, "c"), str(p, "doc")})
	}
	for v in o["elements"].(json.Array) {
		p := v.(json.Object)
		append(&s.elements, Named{str(p, "name"), str(p, "c"), str(p, "doc")})
	}
	for v in o["events"].(json.Array) {
		p := v.(json.Object)
		append(&s.events, Event_Def{str(p, "kind"), str(p, "payload"), str(p, "doc")})
	}
	for v in o["doors"].(json.Array) {
		p := v.(json.Object)
		cell := p["c"].(json.Object)
		append(&s.doors, Door{str(p, "rust"), str(cell, "cell"), str(cell, "text"), str(p, "doc")})
	}
	for v in o["theme"].(json.Array) {
		p := v.(json.Object)
		append(&s.theme, Named{name = str(p, "name"), doc = str(p, "doc")})
	}
	for v in o["metrics"].(json.Array) {
		p := v.(json.Object)
		append(&s.metrics, Named{name = str(p, "name"), doc = str(p, "doc")})
	}
	for v in o["env"].(json.Array) {
		p := v.(json.Object)
		append(&s.env, Named{str(p, "name"), str(p, "c"), str(p, "doc")})
	}
	for k, v in o["lists"].(json.Object) do s.lists[k] = strings_of(v)
	return
}

strings_of :: proc(v: json.Value) -> []string {
	a, ok := v.(json.Array)
	if !ok do return nil
	out := make([]string, len(a))
	for x, i in a do out[i] = x.(json.String)
	return out
}

door_for :: proc(s: ^Schema, c_name: string) -> ^Door {
	for &d in s.doors do if d.c_cell == "is" && d.c_text == c_name do return &d
	return nil
}
