//! Odin: a package (`odin/<addon>/<addon>.odin`) of props structs and place
//! procedures: `kit.button(ui, {label = "Run", variant = .Primary})`. The
//! props are built as C values directly, so an unset optional (a nil
//! `Maybe`, an empty string) is left out rather than sent as null.

use std::fmt::Write;

use crate::{Component, File, Schema, Type, pascal, wrap};

/// How far the package sits from `packages/odin/kui`: generated into
/// `addons/<addon>/bindings/odin/<addon>/`.
const KUI: &str = "../../../../../packages/odin/kui";

fn ty(schema: &Schema, t: &Type) -> String {
    match t {
        Type::String => "string".into(),
        Type::Number => "f64".into(),
        Type::Integer => "i64".into(),
        Type::Boolean => "bool".into(),
        Type::Color => "u32".into(),
        Type::Length => "f32".into(),
        Type::Channel => "^c.Channel".into(),
        Type::Any => "any".into(),
        Type::Named(n) => n.clone(),
        Type::List(t) => format!("[]{}", ty(schema, t)),
        Type::Optional(t) => match t.as_ref() {
            // A pointer, a string, an any and a slice have a nil of their own.
            Type::String | Type::Channel | Type::Any | Type::List(_) => ty(schema, t),
            inner => format!("Maybe({})", ty(schema, inner)),
        },
    }
}

/// `name`, or `name_` where it is one of Odin's keywords (`switch`, `in`).
fn ident(name: &str) -> String {
    const KEYWORDS: &[&str] = &[
        "asm", "auto_cast", "bit_field", "bit_set", "break", "case", "cast", "context", "continue",
        "defer", "distinct", "do", "dynamic", "else", "enum", "fallthrough", "for", "foreign",
        "if", "import", "in", "map", "matrix", "not_in", "or_break", "or_continue", "or_else",
        "or_return", "package", "proc", "return", "struct", "switch", "transmute", "typeid",
        "union", "using", "when", "where",
    ];
    if KEYWORDS.contains(&name) { format!("{name}_") } else { name.to_string() }
}

fn odin_doc(doc: &str, indent: &str) -> String {
    wrap(doc, &format!("{indent}// "), 80)
}

/// An expression for a C value of `v` of type `t`.
fn value(schema: &Schema, t: &Type, v: &str) -> String {
    let a = &schema.addon;
    match t {
        Type::String => format!("c.value_str({v})"),
        Type::Number => format!("c.value_float({v})"),
        Type::Integer => format!("c.value_int({v})"),
        Type::Boolean => format!("c.value_bool({v})"),
        Type::Color => format!("c.value_int(i64({v}))"),
        Type::Length => format!("c.value_float(f64({v}))"),
        Type::Channel => format!("c.channel_handle({v})"),
        Type::Any => format!("(^c.Value)(kui.encode({v}))"),
        Type::Named(n) => {
            if schema.enum_named(n).is_some() {
                format!("c.value_str({}_NAMES[{v}])", screaming(n))
            } else {
                format!("{}_value({v})", crate::emit::c::snake(n))
            }
        }
        Type::List(inner) => format!(
            "{a}_list({v}, proc(x: {}) -> ^c.Value {{ return {} }})",
            ty(schema, inner),
            value(schema, inner, "x")
        ),
        Type::Optional(t) => value(schema, t, v),
    }
}

fn screaming(name: &str) -> String {
    crate::emit::c::snake(name).to_uppercase()
}

/// A statement putting field `name` of `p` into `m`, skipping it unset.
fn put(schema: &Schema, s: &mut String, name: &str, t: &Type) {
    let (inner, req) = t.required();
    let v = format!("p.{}", ident(name));
    if req && !matches!(inner, Type::Channel | Type::Any) {
        let _ = writeln!(s, "\tc.value_map_set(m, \"{name}\", {})", value(schema, inner, &v));
        return;
    }
    match inner {
        Type::String => {
            let _ = writeln!(s, "\tif {v} != \"\" do c.value_map_set(m, \"{name}\", {})", value(schema, inner, &v));
        }
        Type::Channel | Type::Any | Type::List(_) => {
            let _ = writeln!(s, "\tif {v} != nil do c.value_map_set(m, \"{name}\", {})", value(schema, inner, &v));
        }
        _ => {
            let _ = writeln!(
                s,
                "\tif x, ok := {v}.?; ok do c.value_map_set(m, \"{name}\", {})",
                value(schema, inner, "x")
            );
        }
    }
}

pub fn files(schema: &Schema) -> Vec<File> {
    let a = &schema.addon;
    vec![File {
        path: format!("odin/{a}/{a}.odin"),
        text: package(schema),
    }]
}

fn package(schema: &Schema) -> String {
    let a = &schema.addon;
    let mut s = super::banner("//", schema);
    s.push_str(&odin_doc(&schema.doc, ""));
    let _ = writeln!(
        s,
        "package {a}\n\nimport \"base:runtime\"\nimport \"core:strings\"\nimport kui \"{KUI}\"\nimport c \"{KUI}/c\"\n\n\
         // The namespace the components are placed under; the `_in` procedures\n\
         // take another, for {a} loaded twice.\nNAMESPACE :: \"{a}\"\n\n\
         @(private)\n{a}_list :: proc(xs: $T/[]$E, f: proc(x: E) -> ^c.Value) -> ^c.Value {{\n\
         \tl := c.value_list()\n\tfor x in xs do c.value_list_push(l, f(x))\n\treturn l\n}}\n\n\
         // What a `_with` procedure's callback runs its children under.\n@(private)\nBody_Context :: runtime.Context\n\n\
         @(private)\nconcat :: proc(a, b: string) -> string {{\n\treturn strings.concatenate({{a, b}}, context.temp_allocator)\n}}\n"
    );
    for e in &schema.enums {
        s.push_str(&odin_doc(&e.doc, ""));
        let names: Vec<String> = e.values.iter().map(|v| pascal(v)).collect();
        let _ = writeln!(s, "{} :: enum {{\n\t{},\n}}\n", e.name, names.join(",\n\t"));
        let _ = writeln!(s, "@(private)\n{}_NAMES := [{}]string {{", screaming(&e.name), e.name);
        for v in &e.values {
            let _ = writeln!(s, "\t.{} = \"{v}\",", pascal(v));
        }
        s.push_str("}\n\n");
    }
    for r in &schema.records {
        s.push_str(&odin_doc(&r.doc, ""));
        let _ = writeln!(s, "{} :: struct {{", r.name);
        for f in &r.fields {
            s.push_str(&odin_doc(&f.doc, "\t"));
            let _ = writeln!(s, "\t{}: {},", ident(&f.name), ty(schema, &f.ty));
        }
        let _ = writeln!(
            s,
            "}}\n\n@(private)\n{}_value :: proc(p: {}) -> ^c.Value {{\n\tm := c.value_map()",
            crate::emit::c::snake(&r.name),
            r.name
        );
        for f in &r.fields {
            put(schema, &mut s, &f.name, &f.ty);
        }
        s.push_str("\treturn m\n}\n\n");
    }
    for c in &schema.components {
        component(&mut s, schema, c);
    }
    s
}

fn component(s: &mut String, schema: &Schema, c: &Component) {
    let p = pascal(&c.name);
    s.push_str(&odin_doc(&c.doc, ""));
    let _ = writeln!(s, "{p} :: struct {{");
    for f in &c.props {
        s.push_str(&odin_doc(&f.doc, "\t"));
        let _ = writeln!(s, "\t{}: {},", ident(&f.name), ty(schema, &f.ty));
    }
    for e in &c.events {
        s.push_str(&odin_doc(&format!("The message `{}` arrives as. {}", e.name, e.doc), "\t"));
        let _ = writeln!(s, "\ton_{}: any,", e.name);
    }
    s.push_str(
        "\t// Keys the instance (empty: where it is placed); fresh asks every frame.\n\tkey:   string,\n\tfresh: bool,\n}\n\n",
    );
    // Each event's data, and the With_ a message embeds to read it (ADR
    // 0047's shape, as events.odin spells the core's own).
    for e in &c.events {
        let ev = format!("{p}_{}", pascal(&e.name));
        s.push_str(&odin_doc(&format!("What `{}` carries: {}", e.name, e.doc), ""));
        let _ = writeln!(s, "{ev} :: struct {{");
        for f in &e.fields {
            s.push_str(&odin_doc(&f.doc, "\t"));
            let t = match f.ty.required().0 {
                Type::Any => "kui.Value".to_string(),
                other => ty(schema, other),
            };
            let _ = writeln!(s, "\t{}: {t},", ident(&f.name));
        }
        let _ = writeln!(
            s,
            "}}\n\n// Embed with `using _:` in the message passed as `on_{0}`.\nWith_{ev} :: struct {{\n\t{0}: {ev},\n}}\n",
            e.name
        );
    }
    let _ = writeln!(s, "@(private)\n{}_props :: proc(p: {p}) -> ^c.Value {{\n\tm := c.value_map()", c.name);
    for f in &c.props {
        put(schema, s, &f.name, &f.ty);
    }
    for e in &c.events {
        let _ = writeln!(
            s,
            "\tif p.on_{0} != nil do c.value_map_set(m, \"on_{0}\", (^c.Value)(kui.encode(p.on_{0})))",
            e.name
        );
    }
    s.push_str(
        "\tif p.key != \"\" do c.value_map_set(m, \"key\", c.value_str(p.key))\n\tif p.fresh do c.value_map_set(m, \"fresh\", c.value_bool(true))\n\treturn m\n}\n\n",
    );
    let _ = writeln!(
        s,
        "{1} :: proc(ui: ^kui.Ui, p: {p}) -> i32 {{\n\treturn {0}_in(ui, NAMESPACE, p)\n}}\n\n\
         {0}_in :: proc(ui: ^kui.Ui, namespace: string, p: {p}) -> i32 {{\n\tprops := {0}_props(p)\n\tdefer c.value_free(props)\n\t\
         name := concat(namespace, \"/{0}\")\n\treturn c.component(ui, name, props, nil, nil)\n}}\n",
        c.name,
        ident(&c.name)
    );
    if !c.children.is_empty() {
        let _ = writeln!(
            s,
            "// {0} with your children: `children` builds the group it asks for.\n\
             {0}_with :: proc(ui: ^kui.Ui, p: {p}, data: ^$T, children: proc(data: ^T, ui: ^kui.Ui, group: string)) -> i32 {{\n\t\
             Body :: struct {{\n\t\tdata:     ^T,\n\t\tchildren: proc(data: ^T, ui: ^kui.Ui, group: string),\n\t\tctx:      Body_Context,\n\t}}\n\t\
             call :: proc \"c\" (user: rawptr, ui: ^c.Ctx, group: string) {{\n\t\tb := (^Body)(user)\n\t\tcontext = b.ctx\n\t\t\
             b.children(b.data, ui, group)\n\t}}\n\tb := Body{{data, children, context}}\n\tprops := {0}_props(p)\n\tdefer c.value_free(props)\n\t\
             return c.component(ui, concat(NAMESPACE, \"/{0}\"), props, call, &b)\n}}\n",
            c.name
        );
    }
}
