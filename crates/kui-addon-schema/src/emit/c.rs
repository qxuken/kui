//! C: a header (`c/<addon>.h`) of props structs and `static inline` place
//! functions over `kui_component`:
//!
//! ```c
//! KitButton b = {.label = KUI_STR("Run"), .variant = KIT_VARIANT_PRIMARY};
//! kit_button(ui, &b);
//! ```
//!
//! An optional number, integer, boolean or length carries a `_set` flag; an
//! empty string, a zero enum, a zero colour and a NULL pointer are unset.
//! An event's message (`on_*`) is consumed, as `kui_open_with` consumes a
//! payload. Define `<ADDON>_NAMESPACE` before including it to place the
//! components under another namespace.

use std::fmt::Write;

use crate::{Component, File, Schema, Type, pascal, wrap};

fn up(s: &str) -> String {
    s.to_uppercase()
}

fn c_doc(doc: &str, indent: &str) -> String {
    if doc.is_empty() {
        return String::new();
    }
    let body = wrap(doc, &format!("{indent} * "), 80);
    format!("{indent}/*\n{body}{indent} */\n")
}

/// The C fields a schema field becomes (its name, and its `_set` or
/// `_count` beside it when it needs one).
fn fields(schema: &Schema, name: &str, t: &Type) -> Vec<String> {
    let (inner, req) = t.required();
    let mut out = Vec::new();
    let a = &schema.addon;
    match inner {
        Type::String => out.push(format!("KuiStr {name};")),
        Type::Number => out.push(format!("double {name};")),
        Type::Integer => out.push(format!("int64_t {name};")),
        Type::Boolean => out.push(format!("bool {name};")),
        Type::Color => out.push(format!("uint32_t {name}; /* 0xRRGGBBAA, 0 = unset */")),
        Type::Length => out.push(format!("float {name};")),
        Type::Channel => out.push(format!("const KuiChannel *{name};")),
        Type::Any => out.push(format!("KuiValue *{name}; /* consumed */")),
        Type::Named(n) => {
            if schema.enum_named(n).is_some() {
                out.push(format!("{}{n} {name};", pascal(a)));
            } else {
                out.push(format!("const {}{n} *{name};", pascal(a)));
            }
        }
        Type::List(t) => {
            let elem = match t.as_ref() {
                Type::String => "KuiStr".to_string(),
                Type::Number => "double".to_string(),
                Type::Integer => "int64_t".to_string(),
                Type::Boolean => "bool".to_string(),
                Type::Named(n) if schema.enum_named(n).is_some() => format!("{}{n}", pascal(a)),
                Type::Named(n) => format!("{}{n}", pascal(a)),
                _ => "KuiValue *".to_string(),
            };
            out.push(format!("const {elem} *{name};"));
            out.push(format!("size_t {name}_count;"));
        }
        Type::Optional(_) => unreachable!(),
    }
    if !req && matches!(inner, Type::Number | Type::Integer | Type::Boolean | Type::Length) {
        out.push(format!("bool {name}_set;"));
    }
    out
}

/// Statements that put field `name` of `*p` into map `m`, skipping an unset
/// optional.
fn put(schema: &Schema, s: &mut String, name: &str, t: &Type, indent: &str) {
    let a = &schema.addon;
    let (inner, req) = t.required();
    let key = format!("KUI_STR(\"{name}\")");
    let v = format!("p->{name}");
    let value = |inner: &Type, v: &str| -> String {
        match inner {
            Type::String => format!("kui_value_str({v})"),
            Type::Number => format!("kui_value_float({v})"),
            Type::Integer => format!("kui_value_int({v})"),
            Type::Boolean => format!("kui_value_bool({v})"),
            Type::Color => format!("kui_value_int((int64_t){v})"),
            Type::Length => format!("kui_value_float((double){v})"),
            Type::Channel => format!("kui_channel_handle({v})"),
            Type::Any => v.to_string(),
            Type::Named(n) => {
                if schema.enum_named(n).is_some() {
                    format!("kui_value_str({}_{}_name({v}))", a, crate::emit::c::snake(n))
                } else {
                    format!("{}_{}_value({v})", a, snake(n))
                }
            }
            _ => "kui_value_null()".to_string(),
        }
    };
    match inner {
        Type::List(elem) => {
            let item = match elem.as_ref() {
                Type::Named(n) if schema.record_named(n).is_some() => {
                    format!("{a}_{}_value(&{v}[i])", snake(n))
                }
                other => value(other, &format!("{v}[i]")),
            };
            let _ = writeln!(
                s,
                "{indent}if ({v} != NULL) {{\n{indent}    KuiValue *list = kui_value_list();\n{indent}    \
                 for (size_t i = 0; i < {v}_count; i++) kui_value_list_push(list, {item});\n{indent}    \
                 kui_value_map_set(m, {key}, list);\n{indent}}}"
            );
        }
        _ => {
            let cond = if req && !matches!(inner, Type::Channel | Type::Any) {
                None
            } else {
                Some(match inner {
                    Type::String => format!("{v}.len > 0"),
                    Type::Number | Type::Integer | Type::Boolean | Type::Length => {
                        format!("{v}_set")
                    }
                    Type::Color => format!("{v} != 0"),
                    Type::Named(n) if schema.enum_named(n).is_some() => format!("{v} != 0"),
                    _ => format!("{v} != NULL"),
                })
            };
            let set = format!("kui_value_map_set(m, {key}, {});", value(inner, &v));
            match cond {
                Some(c) => {
                    let _ = writeln!(s, "{indent}if ({c}) {set}");
                }
                None => {
                    let _ = writeln!(s, "{indent}{set}");
                }
            }
        }
    }
}

/// `PascalCase` → `snake_case`.
pub(crate) fn snake(name: &str) -> String {
    let mut out = String::new();
    for (i, ch) in name.chars().enumerate() {
        if ch.is_ascii_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

pub fn files(schema: &Schema) -> Vec<File> {
    vec![File {
        path: format!("c/{}.h", schema.addon),
        text: header(schema),
    }]
}

fn header(schema: &Schema) -> String {
    let a = &schema.addon;
    let pa = pascal(a);
    let ua = up(a);
    let mut s = super::banner("/*", schema);
    s = s.trim_end().to_string() + " */\n";
    s.push_str(&c_doc(&schema.doc, ""));
    let _ = writeln!(
        s,
        "#ifndef {ua}_H\n#define {ua}_H\n\n#include \"kui.h\"\n\n\
         /* The namespace the components are placed under: define it before\n * including this header for {a} loaded under another. */\n\
         #ifndef {ua}_NAMESPACE\n#define {ua}_NAMESPACE \"{a}\"\n#endif\n"
    );
    for e in &schema.enums {
        s.push_str(&c_doc(&e.doc, ""));
        let _ = write!(s, "typedef enum {pa}{} {{\n    {ua}_{}_UNSET = 0,\n", e.name, up(&snake(&e.name)));
        for v in &e.values {
            let _ = writeln!(s, "    {ua}_{}_{},", up(&snake(&e.name)), up(v));
        }
        let _ = writeln!(s, "}} {pa}{};\n", e.name);
        let _ = writeln!(
            s,
            "static inline KuiStr {a}_{}_name({pa}{} v) {{\n    switch (v) {{",
            snake(&e.name),
            e.name
        );
        for v in &e.values {
            let _ = writeln!(
                s,
                "    case {ua}_{}_{}: return KUI_STR(\"{v}\");",
                up(&snake(&e.name)),
                up(v)
            );
        }
        s.push_str("    default: return KUI_STR(\"\");\n    }\n}\n\n");
    }
    for r in &schema.records {
        s.push_str(&c_doc(&r.doc, ""));
        let _ = writeln!(s, "typedef struct {pa}{} {{", r.name);
        for f in &r.fields {
            s.push_str(&c_doc(&f.doc, "    "));
            for line in fields(schema, &f.name, &f.ty) {
                let _ = writeln!(s, "    {line}");
            }
        }
        let _ = writeln!(s, "}} {pa}{};\n", r.name);
        let _ = writeln!(
            s,
            "static inline KuiValue *{a}_{}_value(const {pa}{} *p) {{\n    KuiValue *m = kui_value_map();",
            snake(&r.name),
            r.name
        );
        for f in &r.fields {
            put(schema, &mut s, &f.name, &f.ty, "    ");
        }
        s.push_str("    return m;\n}\n\n");
    }
    for c in &schema.components {
        component(&mut s, schema, c);
    }
    let _ = writeln!(s, "#endif /* {ua}_H */");
    s
}

fn component(s: &mut String, schema: &Schema, c: &Component) {
    let a = &schema.addon;
    let pa = pascal(a);
    let ua = up(a);
    let p = pascal(&c.name);
    s.push_str(&c_doc(&c.doc, ""));
    let _ = writeln!(s, "typedef struct {pa}{p} {{");
    for f in &c.props {
        s.push_str(&c_doc(&f.doc, "    "));
        for line in fields(schema, &f.name, &f.ty) {
            let _ = writeln!(s, "    {line}");
        }
    }
    for e in &c.events {
        s.push_str(&c_doc(
            &format!(
                "The message `{}` arrives as, consumed; NULL for none. {}",
                e.name, e.doc
            ),
            "    ",
        ));
        let _ = writeln!(s, "    KuiValue *on_{};", e.name);
    }
    s.push_str(
        "    /* Keys the instance (empty: where it is placed); fresh asks the\n     * extension every frame instead of replaying. */\n    KuiStr key;\n    bool fresh;\n",
    );
    let _ = writeln!(s, "}} {pa}{p};\n");
    let _ = writeln!(
        s,
        "static inline KuiValue *{a}_{}_props(const {pa}{p} *p) {{\n    KuiValue *m = kui_value_map();",
        c.name
    );
    for f in &c.props {
        put(schema, s, &f.name, &f.ty, "    ");
    }
    for e in &c.events {
        let _ = writeln!(
            s,
            "    if (p->on_{0} != NULL) kui_value_map_set(m, KUI_STR(\"on_{0}\"), p->on_{0});",
            e.name
        );
    }
    s.push_str(
        "    if (p->key.len > 0) kui_value_map_set(m, KUI_STR(\"key\"), kui_value_str(p->key));\n    \
         if (p->fresh) kui_value_map_set(m, KUI_STR(\"fresh\"), kui_value_bool(true));\n    return m;\n}\n\n",
    );
    let _ = writeln!(
        s,
        "/* Places {a}/{0}: a KUI_SLOT_* code, KUI_SLOT_UNDECLARED when nothing provides it. */\n\
         static inline int32_t {a}_{0}(KuiCtx *ctx, const {pa}{p} *p) {{\n    \
         KuiValue *props = {a}_{0}_props(p);\n    \
         int32_t fill = kui_component(ctx, KUI_STR({ua}_NAMESPACE \"/{0}\"), props, NULL, NULL);\n    \
         kui_value_free(props);\n    return fill;\n}}\n",
        c.name
    );
    if !c.children.is_empty() {
        let _ = writeln!(
            s,
            "/* {a}_{0} with your children: `children` builds the group it asks for. */\n\
             static inline int32_t {a}_{0}_with(KuiCtx *ctx, const {pa}{p} *p, KuiChildrenFn children,\n    void *user) {{\n    \
             KuiValue *props = {a}_{0}_props(p);\n    \
             int32_t fill = kui_component(ctx, KUI_STR({ua}_NAMESPACE \"/{0}\"), props, children, user);\n    \
             kui_value_free(props);\n    return fill;\n}}\n",
            c.name
        );
    }
}
