// The Odin binding's generator. Two inputs, three outputs:
//
//     clang -x c -Xclang -ast-dump=json -fsyntax-only kui.h > kui.ast.json
//     cargo run -p kui-core --example schema-dump -- schema.json
//     odin run packages/odin/gen -- kui.ast.json kui.h schema.json packages/odin/kui target/odin/layout.c
//
//   kui/c/kui_c.odin    every declaration of kui.h, prefix dropped (raw.odin)
//   layout.c            a C program printing kui/c/layout.odin, the C
//                       compiler's word on every size, offset and constant
//   kui/generated.odin  the typed layer (typed.odin): the enums, Spec and
//                       Text_Style from the prop schema, a struct mirror
//                       per C struct the doors use, and one door per C
//                       function the hand-written files do not call
//
// `nu scripts/odin.nu gen` runs the whole chain.
package gen

import "core:fmt"
import "core:os"

main :: proc() {
	if len(os.args) != 6 {
		fmt.eprintln("usage: gen <kui.ast.json> <kui.h> <schema.json> <packages/odin/kui> <layout.c path>")
		os.exit(2)
	}
	ast_path, header_path, schema_path, kui_dir, layout_path := os.args[1], os.args[2], os.args[3], os.args[4], os.args[5]

	m := load_c(ast_path, header_path)
	emit_raw(&m, fmt.tprintf("%s/c", kui_dir), layout_path)

	s := load_schema(schema_path)
	emit_typed(&m, &s, kui_dir)
}

fail :: proc(format: string, args: ..any) -> ! {
	fmt.eprint("gen: ")
	fmt.eprintfln(format, ..args)
	os.exit(1)
}

write :: proc(path, text: string) {
	if err := os.write_entire_file(path, transmute([]u8)text); err != nil {
		fail("cannot write %s: %v", path, err)
	}
}
