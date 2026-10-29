//! `kui.json`, `kui.toml`, `kui.yaml`: `decode(text)` into plain tables and
//! `encode(value, opts?)` back, through `kui_core::Value` so every format
//! reads the same tables. An object's keys come out sorted, so an encoded
//! file diffs well; `{ pretty = true }` indents.

use kui_core::Value;
use mlua::{Lua, Table};

use super::{fail_as, try_fn};

pub fn install(lua: &Lua, kui: &Table) -> mlua::Result<()> {
    let json = lua.create_table()?;
    json.set(
        "try_decode",
        try_fn(lua, |lua, text: mlua::LuaString| {
            let v = kui_core::json::parse(&text.to_str()?)
                .map_err(|e| fail_as("parse", format!("json: {e}")))?;
            kui_lua::value_to_lua(lua, &v)
        })?,
    )?;
    json.set(
        "try_encode",
        try_fn(lua, |_, (v, opts): (mlua::Value, Option<Table>)| {
            let pretty = opts
                .map(|o| o.get::<Option<bool>>("pretty"))
                .transpose()?
                .flatten()
                == Some(true);
            let mut out = String::new();
            write_json(&mut out, &kui_lua::lua_to_value(&v)?, pretty, 0);
            Ok(out)
        })?,
    )?;
    // `kui.json.array(t)`: `t` (or a new table) marked a list, so that
    // empty it encodes as `[]`; a decoded empty list comes back marked.
    json.set(
        "array",
        lua.create_function(|lua, t: Option<Table>| {
            let t = match t {
                Some(t) => t,
                None => lua.create_table()?,
            };
            kui_lua::mark_array(lua, &t)?;
            Ok(t)
        })?,
    )?;
    kui.set("json", json)?;

    let toml = lua.create_table()?;
    toml.set(
        "try_decode",
        try_fn(lua, |lua, text: mlua::LuaString| {
            let v: toml::Table = toml::from_str(&text.to_str()?)
                .map_err(|e| fail_as("parse", format!("toml: {e}")))?;
            kui_lua::value_to_lua(lua, &from_toml(toml::Value::Table(v)))
        })?,
    )?;
    toml.set(
        "try_encode",
        try_fn(lua, |_, v: mlua::Value| {
            let t = to_toml(&kui_lua::lua_to_value(&v)?)?;
            toml::to_string_pretty(&t).map_err(|e| fail_as("invalid", format!("toml: {e}")))
        })?,
    )?;
    kui.set("toml", toml)?;

    let yaml = lua.create_table()?;
    yaml.set(
        "try_decode",
        try_fn(lua, |lua, text: mlua::LuaString| {
            let docs = yaml_rust2::YamlLoader::load_from_str(&text.to_str()?)
                .map_err(|e| fail_as("parse", format!("yaml: {e}")))?;
            let first = docs.into_iter().next().unwrap_or(yaml_rust2::Yaml::Null);
            kui_lua::value_to_lua(lua, &from_yaml(first))
        })?,
    )?;
    yaml.set(
        "try_encode",
        try_fn(lua, |_, v: mlua::Value| {
            let y = to_yaml(&kui_lua::lua_to_value(&v)?);
            let mut out = String::new();
            yaml_rust2::YamlEmitter::new(&mut out)
                .dump(&y)
                .map_err(|e| fail_as("invalid", format!("yaml: {e}")))?;
            // The emitter opens with a document marker; a file reads cleaner without it.
            Ok(out.strip_prefix("---\n").unwrap_or(&out).to_string() + "\n")
        })?,
    )?;
    kui.set("yaml", yaml)?;
    Ok(())
}

fn sorted(entries: &[(String, Value)]) -> Vec<&(String, Value)> {
    let mut v: Vec<_> = entries.iter().collect();
    v.sort_by(|a, b| a.0.cmp(&b.0));
    v
}

fn quote(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// `v` as JSON onto `out`.
pub fn write_json(out: &mut String, v: &Value, pretty: bool, depth: usize) {
    let nl = |out: &mut String, d: usize| {
        if pretty {
            out.push('\n');
            out.push_str(&"  ".repeat(d));
        }
    };
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Int(i) => out.push_str(&i.to_string()),
        Value::Float(f) if f.is_finite() => {
            if f.fract() == 0.0 && f.abs() < 1e15 {
                out.push_str(&format!("{f:.1}"));
            } else {
                out.push_str(&f.to_string());
            }
        }
        Value::Float(_) => out.push_str("null"),
        Value::Str(s) => quote(out, s),
        Value::List(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                nl(out, depth + 1);
                write_json(out, item, pretty, depth + 1);
            }
            if !items.is_empty() {
                nl(out, depth);
            }
            out.push(']');
        }
        Value::Map(entries) => {
            out.push('{');
            for (i, (k, item)) in sorted(entries).into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                nl(out, depth + 1);
                quote(out, k);
                out.push_str(if pretty { ": " } else { ":" });
                write_json(out, item, pretty, depth + 1);
            }
            if !entries.is_empty() {
                nl(out, depth);
            }
            out.push('}');
        }
    }
}

fn from_toml(v: toml::Value) -> Value {
    match v {
        toml::Value::String(s) => Value::Str(s),
        toml::Value::Integer(i) => Value::Int(i),
        toml::Value::Float(f) => Value::Float(f),
        toml::Value::Boolean(b) => Value::Bool(b),
        toml::Value::Datetime(d) => Value::Str(d.to_string()),
        toml::Value::Array(a) => Value::List(a.into_iter().map(from_toml).collect()),
        toml::Value::Table(t) => {
            Value::Map(t.into_iter().map(|(k, v)| (k, from_toml(v))).collect())
        }
    }
}

fn to_toml(v: &Value) -> mlua::Result<toml::Value> {
    Ok(match v {
        Value::Null => return Err(fail_as("invalid", "toml has no null; leave the key out")),
        Value::Bool(b) => toml::Value::Boolean(*b),
        Value::Int(i) => toml::Value::Integer(*i),
        Value::Float(f) => toml::Value::Float(*f),
        Value::Str(s) => toml::Value::String(s.clone()),
        Value::List(items) => {
            toml::Value::Array(items.iter().map(to_toml).collect::<mlua::Result<_>>()?)
        }
        Value::Map(entries) => {
            let mut t = toml::Table::new();
            for (k, v) in sorted(entries) {
                if !matches!(v, Value::Null) {
                    t.insert(k.clone(), to_toml(v)?);
                }
            }
            toml::Value::Table(t)
        }
    })
}

fn from_yaml(y: yaml_rust2::Yaml) -> Value {
    use yaml_rust2::Yaml;
    match y {
        Yaml::Real(s) => s.parse().map(Value::Float).unwrap_or(Value::Str(s)),
        Yaml::Integer(i) => Value::Int(i),
        Yaml::String(s) => Value::Str(s),
        Yaml::Boolean(b) => Value::Bool(b),
        Yaml::Array(a) => Value::List(a.into_iter().map(from_yaml).collect()),
        Yaml::Hash(h) => Value::Map(
            h.into_iter()
                .map(|(k, v)| {
                    let key = match k {
                        Yaml::String(s) => s,
                        Yaml::Integer(i) => i.to_string(),
                        Yaml::Real(s) => s,
                        Yaml::Boolean(b) => b.to_string(),
                        other => format!("{other:?}"),
                    };
                    (key, from_yaml(v))
                })
                .collect(),
        ),
        Yaml::Alias(_) | Yaml::Null | Yaml::BadValue => Value::Null,
    }
}

fn to_yaml(v: &Value) -> yaml_rust2::Yaml {
    use yaml_rust2::Yaml;
    match v {
        Value::Null => Yaml::Null,
        Value::Bool(b) => Yaml::Boolean(*b),
        Value::Int(i) => Yaml::Integer(*i),
        Value::Float(f) => Yaml::Real(f.to_string()),
        Value::Str(s) => Yaml::String(s.clone()),
        Value::List(items) => Yaml::Array(items.iter().map(to_yaml).collect()),
        Value::Map(entries) => {
            let mut h = yaml_rust2::yaml::Hash::new();
            for (k, v) in sorted(entries) {
                h.insert(Yaml::String(k.clone()), to_yaml(v));
            }
            Yaml::Hash(h)
        }
    }
}
