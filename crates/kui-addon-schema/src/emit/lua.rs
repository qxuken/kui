//! Lua: a host module with LuaLS annotations (`lua/<addon>/ui.lua`), and
//! its Teal declarations (`teal/<addon>/ui.d.tl`). A host script requires
//! it and calls `kit.button { label = "Run", on_press = { kind = "run" } }`,
//! which is `component("kit/button", props)`.

use std::fmt::Write;

use crate::{Component, File, Schema, Type, wrap};

fn luals(schema: &Schema, t: &Type) -> String {
    match t {
        Type::String => "string".into(),
        Type::Number => "number".into(),
        Type::Integer => "integer".into(),
        Type::Boolean => "boolean".into(),
        Type::Color => "integer|string".into(),
        Type::Length => "number|string".into(),
        Type::Channel => "kui.Channel".into(),
        Type::Any => "any".into(),
        Type::Named(n) => format!("{}.{n}", schema.addon),
        Type::List(t) => format!("{}[]", luals(schema, t)),
        Type::Optional(t) => luals(schema, t),
    }
}

fn teal(t: &Type) -> String {
    match t {
        Type::String => "string".into(),
        Type::Number => "number".into(),
        Type::Integer => "integer".into(),
        Type::Boolean => "boolean".into(),
        Type::Color => "integer | string".into(),
        Type::Length => "number | string".into(),
        Type::Channel => "kui.Channel".into(),
        Type::Any => "any".into(),
        Type::Named(n) => n.clone(),
        Type::List(t) => format!("{{{}}}", teal(t)),
        Type::Optional(t) => teal(t),
    }
}

fn has_children(c: &Component) -> bool {
    !c.children.is_empty()
}

fn children_doc(c: &Component) -> String {
    let mut parts = Vec::new();
    for g in &c.children {
        if g.name.is_empty() {
            parts.push(format!("its list part {}", g.doc.trim_end_matches('.')));
        } else {
            parts.push(format!("`{}` {}", g.name, g.doc.trim_end_matches('.')));
        }
    }
    format!("Your children: {}.", parts.join("; "))
}

pub fn files(schema: &Schema) -> Vec<File> {
    let a = &schema.addon;
    vec![
        File {
            path: format!("lua/{a}/ui.lua"),
            text: lua_module(schema),
        },
        File {
            path: format!("teal/{a}/ui.d.tl"),
            text: teal_decls(schema),
        },
    ]
}

fn lua_module(schema: &Schema) -> String {
    let a = &schema.addon;
    let mut s = super::banner("--", schema);
    s.push_str(&wrap(&schema.doc, "-- ", 78));
    let _ = writeln!(
        s,
        "--\n-- local {a} = require(\"{a}.ui\")  -- the components under the namespace `{a}`\n\
         -- local other = {a}.at(\"left\")    -- the same, for an add-on loaded as `left`\n"
    );
    for e in &schema.enums {
        s.push_str(&wrap(&e.doc, "---", 78));
        let values: Vec<String> = e.values.iter().map(|v| format!("\"{v}\"")).collect();
        let _ = writeln!(s, "---@alias {a}.{} {}\n", e.name, values.join("|"));
    }
    for r in &schema.records {
        s.push_str(&wrap(&r.doc, "---", 78));
        let _ = writeln!(s, "---@class {a}.{}", r.name);
        for f in &r.fields {
            let (_, req) = f.ty.required();
            let _ = writeln!(
                s,
                "---@field {}{} {} {}",
                f.name,
                if req { "" } else { "?" },
                luals(schema, &f.ty),
                f.doc
            );
        }
        s.push('\n');
    }
    for c in &schema.components {
        let p = crate::pascal(&c.name);
        let _ = writeln!(
            s,
            "---The props of `{a}/{}`.\n---@class {a}.{p}Props",
            c.name
        );
        for f in &c.props {
            let (_, req) = f.ty.required();
            let _ = writeln!(
                s,
                "---@field {}{} {} {}",
                f.name,
                if req { "" } else { "?" },
                luals(schema, &f.ty),
                f.doc
            );
        }
        for e in &c.events {
            let _ = writeln!(
                s,
                "---@field on_{}? any The message {}: yours, with `{} = {{…}}` in it ({a}.{p}{}). {}",
                e.name,
                "it arrives as",
                e.name,
                crate::pascal(&e.name),
                e.doc
            );
        }
        let _ = writeln!(
            s,
            "---@field key? string Keys the instance; by default it is keyed by where it is placed.\n\
             ---@field fresh? boolean Asks the extension every frame instead of replaying an unchanged instance.\n"
        );
        for e in &c.events {
            let _ = writeln!(
                s,
                "---What `on_{}`'s message carries under `{}`.\n---@class {a}.{p}{}",
                e.name,
                e.name,
                crate::pascal(&e.name)
            );
            for f in &e.fields {
                let _ = writeln!(s, "---@field {} {} {}", f.name, luals(schema, &f.ty), f.doc);
            }
            s.push('\n');
        }
    }
    s.push_str("local function make(namespace)\n  local M = {}\n\n");
    for c in &schema.components {
        let p = crate::pascal(&c.name);
        s.push_str(&wrap(&c.doc, "  ---", 78));
        if has_children(c) {
            s.push_str("  ---\n");
            s.push_str(&wrap(&children_doc(c), "  ---", 78));
        }
        let _ = writeln!(s, "  ---@param props {a}.{p}Props");
        if has_children(c) {
            let _ = writeln!(s, "  ---@param children? table");
        }
        let _ = writeln!(s, "  ---@return kui.Node");
        if has_children(c) {
            let _ = writeln!(
                s,
                "  function M.{}(props, children)\n    return component(namespace .. \"/{}\", props, children)\n  end\n",
                c.name, c.name
            );
        } else {
            let _ = writeln!(
                s,
                "  function M.{}(props)\n    return component(namespace .. \"/{}\", props)\n  end\n",
                c.name, c.name
            );
        }
    }
    let _ = writeln!(
        s,
        "  ---The same components under another namespace: for {a} loaded twice.\n  \
         ---@param other string\n  function M.at(other)\n    return make(other)\n  end\n\n  \
         return M\nend\n\nreturn make(\"{a}\")"
    );
    s
}

fn teal_decls(schema: &Schema) -> String {
    let a = &schema.addon;
    let mut s = super::banner("--", schema);
    s.push_str(&wrap(&schema.doc, "-- ", 78));
    let _ = writeln!(s, "\nlocal record {a}");
    for e in &schema.enums {
        s.push_str(&wrap(&e.doc, "  -- ", 78));
        let values: Vec<String> = e.values.iter().map(|v| format!("\"{v}\"")).collect();
        let _ = writeln!(s, "  enum {}\n    {}\n  end\n", e.name, values.join(" "));
    }
    for r in &schema.records {
        s.push_str(&wrap(&r.doc, "  -- ", 78));
        let _ = writeln!(s, "  record {}", r.name);
        for f in &r.fields {
            s.push_str(&wrap(&f.doc, "    -- ", 78));
            let _ = writeln!(s, "    {}: {}", f.name, teal(&f.ty));
        }
        s.push_str("  end\n\n");
    }
    for c in &schema.components {
        let p = crate::pascal(&c.name);
        let _ = writeln!(s, "  -- The props of `{a}/{}`.\n  record {p}Props", c.name);
        for f in &c.props {
            s.push_str(&wrap(&f.doc, "    -- ", 78));
            let _ = writeln!(s, "    {}: {}", f.name, teal(&f.ty));
        }
        for e in &c.events {
            s.push_str(&wrap(
                &format!("The message `{}` arrives as. {}", e.name, e.doc),
                "    -- ",
                78,
            ));
            let _ = writeln!(s, "    on_{}: any", e.name);
        }
        s.push_str("    key: string\n    fresh: boolean\n  end\n\n");
        for e in &c.events {
            let _ = writeln!(
                s,
                "  -- What `on_{}`'s message carries under `{}`.\n  record {p}{}",
                e.name,
                e.name,
                crate::pascal(&e.name)
            );
            for f in &e.fields {
                let _ = writeln!(s, "    {}: {}", f.name, teal(&f.ty));
            }
            s.push_str("  end\n\n");
        }
    }
    for c in &schema.components {
        let p = crate::pascal(&c.name);
        s.push_str(&wrap(&c.doc, "  -- ", 78));
        if has_children(c) {
            let _ = writeln!(
                s,
                "  {}: function(props: {p}Props, children?: {{any:any}}): kui.Node\n",
                c.name
            );
        } else {
            let _ = writeln!(s, "  {}: function(props: {p}Props): kui.Node\n", c.name);
        }
    }
    let _ = writeln!(
        s,
        "  -- The same components under another namespace.\n  at: function(namespace: string): {a}\nend\n\nreturn {a}"
    );
    s
}
