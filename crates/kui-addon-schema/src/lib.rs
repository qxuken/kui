//! Typed wrappers for a kui add-on's components in every binding, from one
//! schema (ADR 0066).
//!
//! An add-on that provides components (ADR 0060) describes them once in a
//! Lua file — each component's props with their types and docs, its events
//! with their fields, its child groups — and [`generate`] writes a wrapper
//! module per binding: a Lua host module with LuaLS annotations and its
//! Teal declarations, Node functions that are also JSX components with
//! their `.d.ts`, a Rust crate, a C header and an Odin package. Each wrapper
//! builds the props value and calls the binding's component door; the
//! extension itself is unchanged.
//!
//! ```rust
//! let schema = kui_addon_schema::Schema::from_lua(r#"
//!     return {
//!       addon = "demo",
//!       doc = "A demo add-on.",
//!       components = {
//!         { "badge", doc = "A count in a pill.",
//!           props = { { "count", "integer", doc = "The number shown." } },
//!           events = { { "press", doc = "Clicked." } } },
//!       },
//!     }
//! "#).unwrap();
//! let files = kui_addon_schema::generate(&schema);
//! assert!(files.iter().any(|f| f.path.ends_with("demo.d.ts")));
//! ```

pub mod emit;
mod types;

pub use types::Type;

/// A schema, read and checked.
#[derive(Clone, Debug)]
pub struct Schema {
    /// The add-on's name: its default namespace, and the name every
    /// generated module goes under.
    pub addon: String,
    pub doc: String,
    pub enums: Vec<Enum>,
    pub records: Vec<Record>,
    pub components: Vec<Component>,
}

/// A named set of strings.
#[derive(Clone, Debug)]
pub struct Enum {
    pub name: String,
    pub doc: String,
    pub values: Vec<String>,
}

/// A named shape of fields, used by props and event fields.
#[derive(Clone, Debug)]
pub struct Record {
    pub name: String,
    pub doc: String,
    pub fields: Vec<Field>,
}

/// A prop, a record's field or an event's field: a snake_case name, a type
/// and a sentence.
#[derive(Clone, Debug)]
pub struct Field {
    pub name: String,
    pub ty: Type,
    pub doc: String,
}

#[derive(Clone, Debug)]
pub struct Event {
    /// `press`: the prop is `on_press` (Node's `onPress`), and the
    /// extension's data rides under `press`.
    pub name: String,
    pub doc: String,
    pub fields: Vec<Field>,
}

#[derive(Clone, Debug)]
pub struct Group {
    /// `""` for the unnamed group.
    pub name: String,
    pub doc: String,
}

#[derive(Clone, Debug)]
pub struct Component {
    pub name: String,
    pub doc: String,
    pub props: Vec<Field>,
    pub events: Vec<Event>,
    pub children: Vec<Group>,
}

/// Why a schema could not be read.
#[derive(Clone, Debug, PartialEq)]
pub struct SchemaError(pub String);

impl std::fmt::Display for SchemaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for SchemaError {}

/// One generated file: where it goes, relative to the output folder, and
/// what it holds.
#[derive(Clone, Debug)]
pub struct File {
    pub path: String,
    pub text: String,
}

fn err(msg: impl Into<String>) -> SchemaError {
    SchemaError(msg.into())
}

fn lua_err(e: mlua::Error) -> SchemaError {
    SchemaError(e.to_string())
}

/// An entry of a list written `{ "name", "type", doc = "..." }` or
/// `{ name = ..., type = ..., doc = ... }`.
fn positional(t: &mlua::Table, i: i64, key: &str) -> Result<Option<String>, SchemaError> {
    if let Some(v) = t.get::<Option<String>>(i).map_err(lua_err)? {
        return Ok(Some(v));
    }
    t.get::<Option<String>>(key).map_err(lua_err)
}

fn doc_of(t: &mlua::Table) -> Result<String, SchemaError> {
    Ok(t.get::<Option<String>>("doc")
        .map_err(lua_err)?
        .unwrap_or_default())
}

fn list(t: &mlua::Table, key: &str) -> Result<Vec<mlua::Table>, SchemaError> {
    match t.get::<Option<mlua::Table>>(key).map_err(lua_err)? {
        None => Ok(Vec::new()),
        Some(l) => l
            .sequence_values::<mlua::Table>()
            .collect::<Result<_, _>>()
            .map_err(|e| err(format!("`{key}` must be a list of tables: {e}"))),
    }
}

fn is_snake(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        && !s.starts_with(|c: char| c.is_ascii_digit())
}

fn fields(t: &mlua::Table, key: &str, what: &str) -> Result<Vec<Field>, SchemaError> {
    list(t, key)?
        .iter()
        .map(|f| {
            let name = positional(f, 1, "name")?
                .ok_or_else(|| err(format!("{what}: a {key} entry has no name")))?;
            if !is_snake(&name) {
                return Err(err(format!(
                    "{what}: `{name}` must be snake_case (props cross as the add-on's Lua spells them)"
                )));
            }
            let ty = positional(f, 2, "type")?
                .ok_or_else(|| err(format!("{what}.{name}: no type")))?;
            Ok(Field {
                ty: Type::parse(&ty).map_err(|e| err(format!("{what}.{name}: {e}")))?,
                name,
                doc: doc_of(f)?,
            })
        })
        .collect()
}

impl Schema {
    /// Reads a schema from Lua source returning the schema table, and
    /// checks it: names, types defined, nothing named twice.
    pub fn from_lua(source: &str) -> Result<Schema, SchemaError> {
        let lua = mlua::Lua::new();
        let t: mlua::Table = lua
            .load(source)
            .set_name("schema")
            .eval()
            .map_err(|e| err(format!("the schema did not run: {e}")))?;
        let addon: String = t
            .get::<Option<String>>("addon")
            .map_err(lua_err)?
            .ok_or_else(|| err("the schema names no `addon`"))?;
        if !is_snake(&addon) {
            return Err(err(format!("addon `{addon}` must be snake_case")));
        }
        let enums = list(&t, "enums")?
            .iter()
            .map(|e| {
                let name = e
                    .get::<Option<String>>("name")
                    .map_err(lua_err)?
                    .ok_or_else(|| err("an enum has no name"))?;
                let values: Vec<String> = e
                    .sequence_values::<String>()
                    .collect::<Result<_, _>>()
                    .map_err(|e| err(format!("enum `{name}`: {e}")))?;
                if values.is_empty() {
                    return Err(err(format!("enum `{name}` has no values")));
                }
                Ok(Enum {
                    doc: doc_of(e)?,
                    name,
                    values,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let records = list(&t, "records")?
            .iter()
            .map(|r| {
                let name = r
                    .get::<Option<String>>("name")
                    .map_err(lua_err)?
                    .ok_or_else(|| err("a record has no name"))?;
                Ok(Record {
                    fields: fields(r, "fields", &name)?,
                    doc: doc_of(r)?,
                    name,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let components = list(&t, "components")?
            .iter()
            .map(|c| {
                let name =
                    positional(c, 1, "name")?.ok_or_else(|| err("a component has no name"))?;
                if !is_snake(&name) {
                    return Err(err(format!("component `{name}` must be snake_case")));
                }
                let events = list(c, "events")?
                    .iter()
                    .map(|e| {
                        let ev = positional(e, 1, "name")?
                            .ok_or_else(|| err(format!("{name}: an event has no name")))?;
                        if !is_snake(&ev) {
                            return Err(err(format!("{name}: event `{ev}` must be snake_case")));
                        }
                        Ok(Event {
                            fields: fields(e, "fields", &format!("{name}.{ev}"))?,
                            doc: doc_of(e)?,
                            name: ev,
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let children = list(c, "children")?
                    .iter()
                    .map(|g| {
                        Ok(Group {
                            name: positional(g, 1, "group")?.unwrap_or_default(),
                            doc: doc_of(g)?,
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Component {
                    props: fields(c, "props", &name)?,
                    doc: doc_of(c)?,
                    events,
                    children,
                    name,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let schema = Schema {
            doc: t
                .get::<Option<String>>("doc")
                .map_err(lua_err)?
                .unwrap_or_default(),
            addon,
            enums,
            records,
            components,
        };
        schema.check()?;
        Ok(schema)
    }

    /// Reads a schema file.
    pub fn from_path(path: &std::path::Path) -> Result<Schema, SchemaError> {
        let source =
            std::fs::read_to_string(path).map_err(|e| err(format!("{}: {e}", path.display())))?;
        Self::from_lua(&source).map_err(|e| err(format!("{}: {e}", path.display())))
    }

    fn check(&self) -> Result<(), SchemaError> {
        // A type the generated wrappers name for themselves, or one a
        // language has in scope everywhere.
        const RESERVED_TYPES: &[&str] = &[
            "Option", "Result", "String", "Vec", "Box", "Some", "None", "Ok", "Err", "Self",
            "Value", "Node", "Ui", "Channel", "Paint", "Length", "Map", "Set", "Error", "Object",
            "Array", "Function", "Promise", "Number", "Boolean", "Symbol", "Any",
        ];
        let mut types = std::collections::HashSet::new();
        for n in self
            .enums
            .iter()
            .map(|e| &e.name)
            .chain(self.records.iter().map(|r| &r.name))
        {
            if !n.starts_with(|c: char| c.is_ascii_uppercase()) {
                return Err(err(format!("type `{n}` must start with a capital")));
            }
            if RESERVED_TYPES.contains(&n.as_str()) {
                return Err(err(format!(
                    "type `{n}` would shadow a type of a binding's own; name it for what it is"
                )));
            }
            if !types.insert(n.as_str()) {
                return Err(err(format!("type `{n}` is defined twice")));
            }
        }
        let defined = |ty: &Type, at: &str| -> Result<(), SchemaError> {
            for n in ty.names() {
                if !types.contains(n) {
                    return Err(err(format!("{at}: no type `{n}` is defined")));
                }
            }
            Ok(())
        };
        for r in &self.records {
            for f in &r.fields {
                defined(&f.ty, &format!("{}.{}", r.name, f.name))?;
            }
        }
        let mut names = std::collections::HashSet::new();
        for c in &self.components {
            if !names.insert(c.name.as_str()) {
                return Err(err(format!("component `{}` is defined twice", c.name)));
            }
            let mut props = std::collections::HashSet::new();
            for f in &c.props {
                defined(&f.ty, &format!("{}.{}", c.name, f.name))?;
                if !props.insert(f.name.clone()) {
                    return Err(err(format!("{}: prop `{}` twice", c.name, f.name)));
                }
                if f.name == "key" || f.name == "fresh" || f.name.starts_with("on_") {
                    return Err(err(format!(
                        "{}: `{}` is the core's (key, fresh) or an event's (on_*); declare events \
                         under `events`",
                        c.name, f.name
                    )));
                }
            }
            for e in &c.events {
                for f in &e.fields {
                    if !matches!(
                        f.ty.required().0,
                        Type::String | Type::Number | Type::Integer | Type::Boolean | Type::Any
                    ) {
                        return Err(err(format!(
                            "{}.{}.{}: an event's field is a string, number, integer, boolean or \
                             any — what every binding reads off a payload",
                            c.name, e.name, f.name
                        )));
                    }
                }
                if !props.insert(format!("on_{}", e.name)) {
                    return Err(err(format!("{}: event `{}` twice", c.name, e.name)));
                }
            }
        }
        Ok(())
    }

    pub fn enum_named(&self, name: &str) -> Option<&Enum> {
        self.enums.iter().find(|e| e.name == name)
    }

    pub fn record_named(&self, name: &str) -> Option<&Record> {
        self.records.iter().find(|r| r.name == name)
    }
}

/// Every wrapper file for every binding, paths relative to the output
/// folder: `lua/`, `teal/`, `node/`, `rust/`, `c/`, `odin/` — and the
/// component reference every binding's reader shares, `reference.md`
/// (ADR 0071).
pub fn generate(schema: &Schema) -> Vec<File> {
    let mut out = Vec::new();
    out.extend(emit::lua::files(schema));
    out.extend(emit::markdown::files(schema));
    out.extend(emit::node::files(schema));
    out.extend(emit::rust::files(schema));
    out.extend(emit::c::files(schema));
    out.extend(emit::odin::files(schema));
    out
}

/// `snake_case` → `camelCase`.
pub fn camel(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut up = false;
    for c in s.chars() {
        if c == '_' {
            up = true;
        } else if up {
            out.extend(c.to_uppercase());
            up = false;
        } else {
            out.push(c);
        }
    }
    out
}

/// `snake_case` → `PascalCase`.
pub fn pascal(s: &str) -> String {
    let c = camel(s);
    let mut chars = c.chars();
    match chars.next() {
        Some(f) => f.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// A doc sentence as comment lines of `width` columns behind `prefix`.
pub fn wrap(doc: &str, prefix: &str, width: usize) -> String {
    let mut out = String::new();
    let mut line = String::new();
    for word in doc.split_whitespace() {
        if !line.is_empty() && prefix.len() + line.len() + 1 + word.len() > width {
            out.push_str(prefix);
            out.push_str(&line);
            out.push('\n');
            line.clear();
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        out.push_str(prefix);
        out.push_str(&line);
        out.push('\n');
    }
    out
}
