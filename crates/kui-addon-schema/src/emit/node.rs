//! Node: ESM functions that are also JSX function components
//! (`node/<addon>.js`), and their types (`node/<addon>.d.ts`). A host writes
//! `<Button label="Run" onPress={Msg.run} />` or `Button({ label: 'Run' })`;
//! either is `el('component', { name: 'kit/button', props })`, the props
//! translated to the wire's snake_case (ADR 0066, decision 3).

use std::fmt::Write;

use crate::{File, Schema, Type, camel, pascal, wrap};

fn ts(t: &Type) -> String {
    match t {
        Type::String => "string".into(),
        Type::Number | Type::Integer => "number".into(),
        Type::Boolean => "boolean".into(),
        Type::Color => "ColorProp".into(),
        Type::Length => "number | string".into(),
        Type::Channel => "Channel".into(),
        Type::Any => "unknown".into(),
        Type::Named(n) => n.clone(),
        Type::List(t) => {
            let inner = ts(t);
            if inner.contains(' ') {
                format!("Array<{inner}>")
            } else {
                format!("{inner}[]")
            }
        }
        Type::Optional(t) => ts(t),
    }
}

pub fn files(schema: &Schema) -> Vec<File> {
    let a = &schema.addon;
    vec![
        File {
            path: format!("node/{a}.js"),
            text: js(schema),
        },
        File {
            path: format!("node/{a}.d.ts"),
            text: dts(schema),
        },
    ]
}

/// Whether a record (to any depth) has a field whose wire name differs
/// from its camelCase one, so its values need translating.
fn record_needs_translation(schema: &Schema, name: &str) -> bool {
    schema.record_named(name).is_some_and(|r| {
        r.fields
            .iter()
            .any(|f| camel(&f.name) != f.name || type_needs(schema, &f.ty))
    })
}

fn type_needs(schema: &Schema, t: &Type) -> bool {
    match t {
        Type::Named(n) => record_needs_translation(schema, n),
        Type::List(t) | Type::Optional(t) => type_needs(schema, t),
        _ => false,
    }
}

fn js(schema: &Schema) -> String {
    let a = &schema.addon;
    let mut s = super::banner("//", schema);
    s.push_str(&wrap(&schema.doc, "// ", 78));
    s.push_str(
        "//\n// Each function builds an `el('component')` element, and each is a JSX\n\
         // function component as it stands: `<Button label=\"Run\" />`.\n\n",
    );
    // Records whose field names translate.
    for r in &schema.records {
        if !record_needs_translation(schema, &r.name) {
            continue;
        }
        let _ = writeln!(
            s,
            "function wire{}(v) {{\n  if (v == null) return v;\n  return {{",
            r.name
        );
        for f in &r.fields {
            let conv = match f.ty.required().0 {
                Type::Named(n) if record_needs_translation(schema, n) => {
                    format!("wire{n}(v.{})", camel(&f.name))
                }
                Type::List(inner) => match inner.as_ref() {
                    Type::Named(n) if record_needs_translation(schema, n) => {
                        format!("v.{}?.map(wire{n})", camel(&f.name))
                    }
                    _ => format!("v.{}", camel(&f.name)),
                },
                _ => format!("v.{}", camel(&f.name)),
            };
            let _ = writeln!(s, "    {}: {conv},", f.name);
        }
        s.push_str("  };\n}\n\n");
    }
    s.push_str(
        "function flat(children) {\n  if (children == null) return [];\n  return Array.isArray(children) ? children : [children];\n}\n\n",
    );
    s.push_str("function make(namespace) {\n  return {\n");
    for c in &schema.components {
        let p = pascal(&c.name);
        let _ = writeln!(s, "    {p}(props = {{}}, children) {{");
        s.push_str("      const wire = {};\n");
        for f in &c.props {
            let conv = match f.ty.required().0 {
                Type::Named(n) if record_needs_translation(schema, n) => {
                    format!("wire{n}(props.{})", camel(&f.name))
                }
                Type::List(inner) => match inner.as_ref() {
                    Type::Named(n) if record_needs_translation(schema, n) => {
                        format!("props.{}?.map(wire{n})", camel(&f.name))
                    }
                    _ => format!("props.{}", camel(&f.name)),
                },
                _ => format!("props.{}", camel(&f.name)),
            };
            let _ = writeln!(
                s,
                "      if (props.{} !== undefined) wire.{} = {conv};",
                camel(&f.name),
                f.name
            );
        }
        for e in &c.events {
            let _ = writeln!(
                s,
                "      if (props.on{} !== undefined) wire.on_{} = props.on{};",
                pascal(&e.name),
                e.name,
                pascal(&e.name)
            );
        }
        let named: Vec<_> = c.children.iter().filter(|g| !g.name.is_empty()).collect();
        if named.is_empty() {
            s.push_str("      const groups = undefined;\n");
        } else {
            s.push_str("      const groups = {};\n");
            for g in &named {
                let _ = writeln!(
                    s,
                    "      if (props.{} !== undefined) groups.{} = flat(props.{});",
                    camel(&g.name),
                    g.name,
                    camel(&g.name)
                );
            }
        }
        let _ = writeln!(
            s,
            "      return {{\n        type: 'component',\n        key: props.key != null ? String(props.key) : undefined,\n        \
             props: {{ name: `${{namespace}}/{}`, props: wire, groups, fresh: props.fresh }},\n        \
             children: flat(children ?? props.children),\n      }};\n    }},",
            c.name
        );
    }
    let _ = writeln!(
        s,
        "    /** The same components under another namespace: for {a} loaded twice. */\n    at: make,\n  }};\n}}\n"
    );
    let names: Vec<String> = schema.components.iter().map(|c| pascal(&c.name)).collect();
    let _ = writeln!(
        s,
        "const {a} = make('{a}');\nexport const {{ {} }} = {a};\nexport const at = make;\nexport default {a};",
        names.join(", ")
    );
    s
}

fn dts(schema: &Schema) -> String {
    let a = &schema.addon;
    let mut s = super::banner("//", schema);
    s.push_str(&wrap(&schema.doc, "// ", 78));
    s.push_str("\nimport type { Channel, HandlerMsg, KuiNode } from '@qxuken/kui';\nimport type { ColorProp } from '@qxuken/kui/jsx-runtime';\n\n");
    for e in &schema.enums {
        let _ = writeln!(s, "/** {} */", e.doc);
        let values: Vec<String> = e.values.iter().map(|v| format!("'{v}'")).collect();
        let _ = writeln!(s, "export type {} = {};\n", e.name, values.join(" | "));
    }
    for r in &schema.records {
        let _ = writeln!(s, "/** {} */\nexport interface {} {{", r.doc, r.name);
        for f in &r.fields {
            let (_, req) = f.ty.required();
            let _ = writeln!(
                s,
                "  /** {} */\n  {}{}: {};",
                f.doc,
                camel(&f.name),
                if req { "" } else { "?" },
                ts(&f.ty)
            );
        }
        s.push_str("}\n\n");
    }
    for c in &schema.components {
        let p = pascal(&c.name);
        let _ = writeln!(
            s,
            "/** The props of `{a}/{}`. */\nexport interface {p}Props {{",
            c.name
        );
        for f in &c.props {
            let (_, req) = f.ty.required();
            let _ = writeln!(
                s,
                "  /** {} */\n  {}{}: {};",
                f.doc,
                camel(&f.name),
                if req { "" } else { "?" },
                ts(&f.ty)
            );
        }
        // Typed as the core's handler props are (`onDrag` is
        // `HandlerMsg<WithDrag>`): a registered message that arrives with
        // the event's field, declared without it — so a typo, or a
        // message registered for another event, fails at the prop
        // (backlog F208, from the kui-node-template field report).
        for e in &c.events {
            let _ = writeln!(
                s,
                "  /** The message {} arrives as: yours, with `{}: {p}{}` in it — register it as `{p}{}Msg<…>`. {} */\n  on{}?: HandlerMsg<{{ {}: {p}{} }}>;",
                e.name,
                e.name,
                pascal(&e.name),
                pascal(&e.name),
                e.doc,
                pascal(&e.name),
                e.name,
                pascal(&e.name)
            );
        }
        for g in &c.children {
            if g.name.is_empty() {
                let _ = writeln!(s, "  /** {} */\n  children?: KuiNode;", g.doc);
            } else {
                let _ = writeln!(s, "  /** {} */\n  {}?: KuiNode;", g.doc, camel(&g.name));
            }
        }
        s.push_str(
            "  /** Keys the instance; by default it is keyed by where it is placed. */\n  key?: string | number;\n  \
             /** Asks the extension every frame instead of replaying an unchanged instance. */\n  fresh?: boolean;\n}\n\n",
        );
        for e in &c.events {
            let _ = writeln!(
                s,
                "/** What `on{}`'s message carries under `{}`. */\nexport interface {p}{} {{",
                pascal(&e.name),
                e.name,
                pascal(&e.name)
            );
            for f in &e.fields {
                let _ = writeln!(s, "  /** {} */\n  {}: {};", f.doc, f.name, ts(&f.ty));
            }
            s.push_str("}\n");
            let _ = writeln!(
                s,
                "/** Your message `T` as `{}` arrives. */\nexport type {p}{}Msg<T = {{}}> = T & {{ {}: {p}{} }};\n",
                e.name,
                pascal(&e.name),
                e.name,
                pascal(&e.name)
            );
        }
    }
    for c in &schema.components {
        let p = pascal(&c.name);
        let _ = writeln!(
            s,
            "/** {} */\nexport declare function {p}(props: {p}Props, children?: KuiNode): KuiNode;\n",
            c.doc
        );
    }
    let names: Vec<String> = schema
        .components
        .iter()
        .map(|c| format!("  {0}: typeof {0};", pascal(&c.name)))
        .collect();
    let _ = writeln!(
        s,
        "export interface {} {{\n{}\n  at(namespace: string): {};\n}}\n/** The same components under another namespace: for {a} loaded twice. */\nexport declare function at(namespace: string): {};\ndeclare const {a}: {};\nexport default {a};",
        pascal(a),
        names.join("\n"),
        pascal(a),
        pascal(a),
        pascal(a)
    );
    s
}
