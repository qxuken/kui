package gen

import "core:encoding/json"
import "core:os"
import "core:slice"
import "core:strconv"
import "core:strings"

// kui.h as clang parsed it. Types are kept as C spells them ("const KuiSpec
// *"), since what a pointer means - an array, an out-param, a value the
// call consumes - is read off the C type and its neighbours.

Field :: struct {
	name, type: string,
}

Record :: struct {
	name:   string, // KuiSpec
	fields: [dynamic]Field,
}

Param :: struct {
	name, type: string,
}

Function :: struct {
	name:   string,
	ret:    string,
	params: [dynamic]Param,
}

Constant :: struct {
	name, value: string,
	// The C enum it came from, or for a #define the run of adjacent
	// #define lines: what makes KUI_SCROLLBAR_* one family.
	group:       int,
	is_define:   bool,
}

Fn_Typedef :: struct {
	name, type: string,
}

C_Model :: struct {
	records:   [dynamic]Record,
	opaque:    [dynamic]string,
	functions: [dynamic]Function,
	constants: [dynamic]Constant,
	fn_types:  [dynamic]Fn_Typedef,
}

// What a plugin defines and the library only calls (kui.h's "Extension
// ABI"): an Odin plugin would export these, not import them.
PLUGIN_SIDE :: []string{"kui_ext_abi", "kui_ext_name", "kui_ext_init", "kui_ext_slots", "kui_ext_view", "kui_ext_on_event", "kui_ext_free"}

load_c :: proc(ast_path, header_path: string) -> (m: C_Model) {
	data, read_err := os.read_entire_file(ast_path, context.allocator)
	if read_err != nil do fail("cannot read %s: %v", ast_path, read_err)
	root, err := json.parse(data)
	if err != .None do fail("%s: bad json: %v", ast_path, err)

	append(&m.constants, ..header_defines(header_path)[:])

	// clang writes `file` only on the first declaration of a run from one
	// file, so the current file is carried forward.
	current_file := ""
	group := 0
	for decl in root.(json.Object)["inner"].(json.Array) {
		d := decl.(json.Object)
		if loc, has_loc := d["loc"].(json.Object); has_loc {
			if f, has_file := loc["file"].(json.String); has_file do current_file = f
		}
		if !strings.has_suffix(current_file, "kui.h") do continue
		name := str(d, "name")
		switch str(d, "kind") {
		case "RecordDecl":
			inner, has := d["inner"].(json.Array)
			if !has {
				if !slice.contains(m.opaque[:], name) do append(&m.opaque, name)
				continue
			}
			r := Record {
				name = name,
			}
			for f in inner {
				fo := f.(json.Object)
				if str(fo, "kind") != "FieldDecl" do continue
				append(&r.fields, Field{str(fo, "name"), qual(fo)})
			}
			append(&m.records, r)
		case "TypedefDecl":
			t := qual(d)
			if strings.contains(t, "(*)") do append(&m.fn_types, Fn_Typedef{name, t})
		case "EnumDecl":
			group += 1
			next := 0
			for c in d["inner"].(json.Array) {
				co := c.(json.Object)
				if str(co, "kind") != "EnumConstantDecl" do continue
				// The value clang computed: the ConstantExpr it wraps an
				// explicit initializer in (under an ImplicitCastExpr when the
				// initializer is unsigned, `1u << 1`), or for a bare constant
				// the previous one plus one.
				value, explicit := constant_expr(co)
				if !explicit do value = int_string(next)
				n, _ := strconv.parse_int(value)
				next = n + 1
				append(&m.constants, Constant{name = str(co, "name"), value = value, group = group})
			}
		case "FunctionDecl":
			if str(d, "storageClass") == "static" do continue // kui_str_eq: inline, Odin has ==
			if slice.contains(PLUGIN_SIDE, name) do continue
			fq := qual(d)
			f := Function {
				name = name,
				ret  = strings.trim_space(fq[:strings.index_byte(fq, '(')]),
			}
			if inner, ok := d["inner"].(json.Array); ok {
				for p in inner {
					po := p.(json.Object)
					if str(po, "kind") != "ParmVarDecl" do continue
					append(&f.params, Param{str(po, "name"), qual(po)})
				}
			}
			append(&m.functions, f)
		}
	}
	return
}

record :: proc(m: ^C_Model, name: string) -> ^Record {
	for &r in m.records do if r.name == name do return &r
	return nil
}

function :: proc(m: ^C_Model, name: string) -> ^Function {
	for &f in m.functions do if f.name == name do return &f
	return nil
}

constant :: proc(m: ^C_Model, name: string) -> ^Constant {
	for &c in m.constants do if c.name == name do return &c
	return nil
}

str :: proc(o: json.Object, key: string) -> string {
	s, _ := o[key].(json.String)
	return s
}

qual :: proc(o: json.Object) -> string {
	return str(o["type"].(json.Object), "qualType")
}

constant_expr :: proc(o: json.Object) -> (string, bool) {
	if str(o, "kind") == "ConstantExpr" do return str(o, "value"), true
	inner, _ := o["inner"].(json.Array)
	for e in inner {
		if v, ok := constant_expr(e.(json.Object)); ok do return v, true
	}
	return "", false
}

int_string :: proc(n: int) -> string {
	buf: [32]u8
	return strings.clone(strconv.write_int(buf[:], i64(n), 10))
}

// `#define KUI_NAME <number>` - the constants the AST does not carry. A
// macro with arguments, or one whose body is not a lone number (the
// *_INIT initializers, KUI_STR), is not a constant and is skipped. Adjacent
// lines are one family, as an enum's members are.
header_defines :: proc(path: string) -> (out: [dynamic]Constant) {
	text, err := os.read_entire_file(path, context.allocator)
	if err != nil do fail("cannot read %s: %v", path, err)
	rest := string(text)
	group := 1_000_000
	adjacent := false
	for line in strings.split_lines_iterator(&rest) {
		fields := strings.fields(line, context.temp_allocator)
		if len(fields) != 3 || fields[0] != "#define" || !strings.has_prefix(fields[1], "KUI_") {
			adjacent = false
			continue
		}
		v := strings.trim_suffix(strings.trim_suffix(strings.trim(fields[2], "()"), "u"), "f")
		if _, ok := strconv.parse_f64(v); !ok {
			adjacent = false
			continue
		}
		if !adjacent do group += 1
		adjacent = true
		append(&out, Constant{name = strings.clone(fields[1]), value = strings.clone(v), group = group, is_define = true})
	}
	return
}
