package kui

import "base:intrinsics"
import "base:runtime"
import "core:reflect"
import "core:strings"
import c "c"

// Messages are plain data. Every binding carries one as a map shaped
// {kind, ...fields}, and kui writes and reads that map from ordinary Odin
// types, so an app declares its messages as types and never builds a map:
//
//     Inc  :: struct {}                  // {kind = "inc"}
//     Pick :: struct {id: int}           // {kind = "pick", id = 3}
//     Msg  :: union {Inc, Pick, kui.Context_Menu_Event, kui.Dismiss_Event}
//
//     kui.button(ui, "+1", Inc{})        // any message where a handler goes
//
//     switch m in kui.message(ev, Msg) { // and back, as the union
//     case Inc:              s.count += 1
//     case Pick:             s.picked = m.id
//     case kui.Context_Menu_Event: s.menu = {m.x, m.y}
//     }
//
// The rules are #[derive(Message)]'s, so a Rust, Lua or Node peer reads the
// same thing:
//
//   - a named struct is a map whose `kind` is its type name in snake_case
//     (Tab_New and TabNew are both "tab_new") and whose other keys are its
//     fields, a `kui:"name"` tag renaming one;
//   - an enum given as a message is {kind = "<value>"}; as a field it is the
//     bare string;
//   - a union is its variant (nil is null);
//   - bool, the numbers and string are themselves, slices and arrays are
//     lists, map[string]T a map, a pointer what it points at.
//
// Reading back, a kind matches a type or enum name with case and
// underscores ignored, and a type's _Event suffix too, so the core's own
// "contextmenu" is Context_Menu_Event (events.odin). A field missing from the payload is left
// zero. Strings read from a payload are borrowed: valid until the next
// poll, or for the callback a run hands the event to; clone one to keep it.

// A payload, borrowed from the event it came with. Read it with get / at /
// as_* or decode it into a type; a field of type Value in a message is
// handed the raw value (the `tag` on a core event, for one).
Value :: distinct ^c.Value

// Ask for a behaviour without a tag: modal = kui.NO_TAG is a modal surface
// whose dismiss event carries no `tag`.
No_Tag :: struct {}
NO_TAG :: No_Tag{}

// -- Reading ----------------------------------------------------------------

// The message an event carries, as the union T (nil when the payload is
// none of T's variants), or as a single struct or enum type with ok.
message :: proc {
	message_union,
	message_one,
}

message_union :: proc(ev: Event, $T: typeid) -> (out: T) where intrinsics.type_is_union(T) {
	decode(ev.payload, &out)
	return
}

message_one :: proc(ev: Event, $T: typeid) -> (out: T, ok: bool) where !intrinsics.type_is_union(T) {
	ok = decode(ev.payload, &out)
	return
}

// The payload's `kind`, "" when it has none.
kind :: proc(v: Value) -> string {
	s, _ := as_string(get(v, "kind"))
	return s
}

get :: proc(v: Value, key: string) -> Value {
	return Value(c.value_get((^c.Value)(v), key))
}

at :: proc(v: Value, i: int) -> Value {
	return Value(c.value_at((^c.Value)(v), uint(i)))
}

// A list's length or a map's entry count.
length :: proc(v: Value) -> int {
	return int(c.value_len((^c.Value)(v)))
}

// A map's i-th entry, for walking one whose keys are not known ahead.
entry :: proc(v: Value, i: int) -> (key: string, value: Value) {
	value = Value(c.value_entry((^c.Value)(v), uint(i), &key))
	return
}

is_null :: proc(v: Value) -> bool {
	return v == nil || c.value_is_null((^c.Value)(v))
}

as_string :: proc(v: Value) -> (s: string, ok: bool) {
	if v == nil do return
	ok = c.value_as_str((^c.Value)(v), &s)
	return
}

as_bool :: proc(v: Value) -> (b: bool, ok: bool) {
	if v == nil do return
	ok = c.value_as_bool((^c.Value)(v), &b)
	return
}

// A number either way it was written: an integer reads as a float too.
as_f64 :: proc(v: Value) -> (f: f64, ok: bool) {
	if v == nil do return
	if c.value_as_float((^c.Value)(v), &f) do return f, true
	i: i64
	if c.value_as_int((^c.Value)(v), &i) do return f64(i), true
	return
}

as_i64 :: proc(v: Value) -> (i: i64, ok: bool) {
	if v == nil do return
	if c.value_as_int((^c.Value)(v), &i) do return i, true
	f: f64
	if c.value_as_float((^c.Value)(v), &f) do return i64(f), true
	return
}

// Reads v into dst, a pointer to any type the rules above cover; false when
// v is not shaped like it (a different kind, a string for a number). Slices
// are allocated from context.temp_allocator.
decode :: proc(v: Value, dst: ^$T) -> bool {
	return decode_any(v, any{dst, typeid_of(T)})
}

@(private)
decode_any :: proc(v: Value, dst: any) -> bool {
	if v == nil do return false
	ti := type_info_of(dst.id)
	if dst.id == Value {
		(^Value)(dst.data)^ = v
		return true
	}
	base := runtime.type_info_base(ti)
	#partial switch info in base.variant {
	case runtime.Type_Info_Boolean:
		b := as_bool(v) or_return
		return set_int(dst, base.size, false, 1 if b else 0)
	case runtime.Type_Info_Integer:
		i := as_i64(v) or_return
		return set_int(dst, base.size, info.signed, i)
	case runtime.Type_Info_Float:
		f := as_f64(v) or_return
		switch base.size {
		case 4: (^f32)(dst.data)^ = f32(f)
		case 8: (^f64)(dst.data)^ = f
		case: return false
		}
		return true
	case runtime.Type_Info_String:
		s := as_string(v) or_return
		if info.is_cstring do return false
		(^string)(dst.data)^ = s
		return true
	case runtime.Type_Info_Enum:
		// The bare string a field holds, or the {kind} a message is.
		s, is_str := as_string(v)
		if !is_str do s = kind(v)
		for name, i in info.names[:len(info.values)] {
			if same_name(name, s) do return set_int(dst, base.size, true, i64(info.values[i]))
		}
		return false
	case runtime.Type_Info_Struct:
		if named, ok := ti.variant.(runtime.Type_Info_Named); ok {
			k := kind(v)
			if k != "" && !same_name(named.name, k) do return false
		}
		if !is_map(v) do return false
		for i in 0 ..< int(info.field_count) {
			name := field_name(info, i)
			if name == "_" do continue
			field := any{rawptr(uintptr(dst.data) + info.offsets[i]), info.types[i].id}
			fv := get(v, name)
			if fv == nil || is_null(fv) do continue
			decode_any(fv, field)
		}
		return true
	case runtime.Type_Info_Union:
		if is_null(v) {
			mem_zero(dst.data, base.size)
			return true
		}
		for variant in info.variants {
			// Among several variants a message must name its kind: a union
			// is never "the first struct that happens to fit".
			if len(info.variants) > 1 && !names_kind(v, variant) do continue
			scratch := make([]byte, variant.size, context.temp_allocator)
			if !decode_any(v, any{raw_data(scratch), variant.id}) do continue
			mem_zero(dst.data, base.size)
			runtime.mem_copy(dst.data, raw_data(scratch), variant.size)
			reflect.set_union_variant_typeid(dst, variant.id)
			return true
		}
		return false
	case runtime.Type_Info_Slice:
		n := length(v)
		elem := info.elem
		buf := make([]byte, n * elem.size, context.temp_allocator)
		for i in 0 ..< n {
			decode_any(at(v, i), any{rawptr(uintptr(raw_data(buf)) + uintptr(i * elem.size)), elem.id})
		}
		(^runtime.Raw_Slice)(dst.data)^ = {raw_data(buf), n}
		return true
	case runtime.Type_Info_Array:
		for i in 0 ..< min(info.count, length(v)) {
			decode_any(at(v, i), any{rawptr(uintptr(dst.data) + uintptr(i * info.elem_size)), info.elem.id})
		}
		return true
	}
	return false
}

@(private)
names_kind :: proc(v: Value, variant: ^runtime.Type_Info) -> bool {
	named, ok := variant.variant.(runtime.Type_Info_Named)
	if !ok do return true
	#partial switch info in runtime.type_info_base(variant).variant {
	case runtime.Type_Info_Struct:
		return same_name(named.name, kind(v))
	case runtime.Type_Info_Enum:
		s, is_str := as_string(v)
		if !is_str do s = kind(v)
		for name in info.names[:len(info.values)] do if same_name(name, s) do return true
		return false
	}
	return true
}

@(private)
is_map :: proc(v: Value) -> bool {
	key: string
	return c.value_entry((^c.Value)(v), 0, &key) != nil
}

@(private)
set_int :: proc(dst: any, size: int, signed: bool, i: i64) -> bool {
	switch size {
	case 1: (^u8)(dst.data)^ = u8(i)
	case 2: (^u16)(dst.data)^ = u16(i)
	case 4: (^u32)(dst.data)^ = u32(i)
	case 8: (^i64)(dst.data)^ = i
	case: return false
	}
	return true
}

@(private)
mem_zero :: proc(p: rawptr, n: int) {
	runtime.mem_zero(p, n)
}

// A field's key: its `kui:"name"` tag, else its name.
@(private)
field_name :: proc(info: runtime.Type_Info_Struct, i: int) -> string {
	if name, ok := reflect.struct_tag_lookup(reflect.Struct_Tag(info.tags[i]), "kui"); ok do return name
	return info.names[i]
}

// "Context_Menu" and "contextmenu", "TabNew" and "tab_new": the same name
// once case and underscores are set aside. A type named X_Event is also
// the kind "x", which is how the core's events are named.
@(private)
same_name :: proc(type_name, kind: string) -> bool {
	if len(type_name) > 6 && type_name[len(type_name) - 6:] == "_Event" && same_spelling(type_name[:len(type_name) - 6], kind) {
		return true
	}
	return same_spelling(type_name, kind)
}

@(private)
same_spelling :: proc(a, b: string) -> bool {
	i, j := 0, 0
	for {
		for i < len(a) && a[i] == '_' do i += 1
		for j < len(b) && b[j] == '_' do j += 1
		if i == len(a) || j == len(b) do return i == len(a) && j == len(b)
		if lower(a[i]) != lower(b[j]) do return false
		i += 1
		j += 1
	}
}

@(private)
lower :: proc(ch: u8) -> u8 {
	return ch + 32 if 'A' <= ch && ch <= 'Z' else ch
}

// -- Writing ----------------------------------------------------------------

// A message as the value a payload is, owned by the caller until
// value_free: for reading back what a door would send. The doors take the
// Odin value itself and encode it on the way.
encode :: proc(msg: any) -> Value {
	return Value(to_value(msg))
}

value_free :: proc(v: Value) {
	c.value_free((^c.Value)(v))
}

// A message as a new C value, for a door to hand over (nil for no message).
@(private)
to_value :: proc(msg: any) -> ^c.Value {
	if msg == nil do return nil
	return encode_any(msg, top = true)
}

@(private)
encode_any :: proc(a: any, top: bool) -> ^c.Value {
	if a.data == nil do return c.value_null()
	ti := type_info_of(a.id)
	if a.id == No_Tag do return c.value_null()
	base := runtime.type_info_base(ti)
	#partial switch info in base.variant {
	case runtime.Type_Info_Any:
		// A value held in an `any` (a map[string]any, a []any): what it holds.
		return encode_any((^any)(a.data)^, top)
	case runtime.Type_Info_Boolean:
		b, _ := reflect.as_bool(a)
		return c.value_bool(b)
	case runtime.Type_Info_Integer:
		i, _ := reflect.as_i64(a)
		return c.value_int(i)
	case runtime.Type_Info_Float:
		f, _ := reflect.as_f64(a)
		return c.value_float(f)
	case runtime.Type_Info_String:
		s, _ := reflect.as_string(a)
		return c.value_str(s)
	case runtime.Type_Info_Enum:
		name := snake(reflect.enum_string(a))
		if !top do return c.value_str(name)
		m := c.value_map()
		c.value_map_set(m, "kind", c.value_str(name))
		return m
	case runtime.Type_Info_Struct:
		m := c.value_map()
		if named, ok := ti.variant.(runtime.Type_Info_Named); ok {
			c.value_map_set(m, "kind", c.value_str(snake(named.name)))
		}
		for i in 0 ..< int(info.field_count) {
			name := field_name(info, i)
			if name == "_" || info.types[i].id == Value do continue
			field := any{rawptr(uintptr(a.data) + info.offsets[i]), info.types[i].id}
			c.value_map_set(m, name, encode_any(field, top = false))
		}
		return m
	case runtime.Type_Info_Union:
		variant := reflect.get_union_variant(a)
		if variant == nil do return c.value_null()
		return encode_any(variant, top)
	case runtime.Type_Info_Pointer:
		p := (^rawptr)(a.data)^
		if p == nil do return c.value_null()
		return encode_any(any{p, info.elem.id}, top)
	case runtime.Type_Info_Slice, runtime.Type_Info_Array, runtime.Type_Info_Dynamic_Array:
		list := c.value_list()
		for i in 0 ..< reflect.length(a) {
			c.value_list_push(list, encode_any(reflect.index(a, i), top = false))
		}
		return list
	case runtime.Type_Info_Map:
		m := c.value_map()
		it: int
		for k, v in reflect.iterate_map(a, &it) {
			key, ok := reflect.as_string(k)
			if !ok do continue
			c.value_map_set(m, key, encode_any(v, top = false))
		}
		return m
	}
	return c.value_null()
}

// Tab_New, TabNew and Add10 as tab_new, tab_new and add10: Rust's
// snake_case, which the derive gives a variant.
@(private)
snake :: proc(name: string) -> string {
	b := strings.builder_make(context.temp_allocator)
	for i in 0 ..< len(name) {
		ch := name[i]
		if 'A' <= ch && ch <= 'Z' {
			prev := name[i - 1] if i > 0 else '_'
			if i > 0 && prev != '_' && !('A' <= prev && prev <= 'Z') do strings.write_byte(&b, '_')
			strings.write_byte(&b, ch + 32)
		} else {
			strings.write_byte(&b, ch)
		}
	}
	return strings.to_string(b)
}
