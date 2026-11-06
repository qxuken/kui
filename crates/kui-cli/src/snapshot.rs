//! Detached Lua value graphs. Channel columns are copied in bulk in Rust;
//! a chart's history must not allocate a Lua table for every numeric cell.
use base64::Engine;
use kui_core::channel::lua::LuaChannel;
use kui_core::channel::{Channel, Column, Data};
use mlua::{Lua, Table, Value};
use serde_json::{Value as J, json};
use std::collections::HashMap;

#[derive(Default)]
struct Graph {
    objects: Vec<J>,
    pending: Vec<Value>,
    seen: HashMap<usize, usize>,
    channels: HashMap<(u64, u64), usize>,
}
impl Graph {
    fn value(&mut self, v: Value) -> mlua::Result<J> {
        let scalar = match &v {
            Value::Nil => Some(json!({"kind":"nil"})),
            Value::Boolean(v) => Some(json!({"kind":"boolean","value":v})),
            Value::Integer(v) => Some(json!({"kind":"integer","value":v.to_string()})),
            Value::Number(v) => Some(json!({"kind":"float","value":v.to_string()})),
            Value::String(v) => Some(
                json!({"kind":"string","base64":base64::engine::general_purpose::STANDARD.encode(v.as_bytes())}),
            ),
            _ => None,
        };
        if let Some(v) = scalar {
            return Ok(v);
        }
        if let Value::UserData(ud) = &v
            && let Ok(ch) = ud.borrow::<LuaChannel>()
        {
            let key = (ch.0.id(), ch.0.rev());
            if let Some(id) = self.channels.get(&key) {
                return Ok(json!({"kind":"ref","id":id}));
            }
            self.channels.insert(key, self.objects.len() + 1);
        }
        let pointer = v.to_pointer() as usize;
        if pointer != 0
            && let Some(id) = self.seen.get(&pointer)
        {
            return Ok(json!({"kind":"ref","id":id}));
        }
        if self.objects.len() >= 1_000_000 {
            return Err(mlua::Error::runtime("snapshot exceeds one million objects"));
        }
        let id = self.objects.len() + 1;
        if pointer != 0 {
            self.seen.insert(pointer, id);
        }
        self.objects.push(J::Null);
        self.pending.push(v);
        Ok(json!({"kind":"ref","id":id}))
    }
    fn finish(mut self, root: Value) -> mlua::Result<J> {
        let root = self.value(root)?;
        // Assign identities through named/scalar-key paths before table keys.
        // A named alias may live in a later pending table, not just the root.
        let mut scan = 0;
        while scan < self.pending.len() {
            if let Value::Table(table) = self.pending[scan].clone() {
                let mut named = table
                    .pairs::<Value, Value>()
                    .collect::<mlua::Result<Vec<_>>>()?;
                named.retain(|(key, _)| {
                    matches!(
                        key,
                        Value::String(_) | Value::Integer(_) | Value::Number(_) | Value::Boolean(_)
                    )
                });
                named.sort_by_cached_key(|(key, _)| sort_key(key, 0));
                for (_, value) in named {
                    self.value(value)?;
                }
            }
            scan += 1;
        }
        let mut index = 0;
        while index < self.pending.len() {
            let id = index + 1;
            let v = self.pending[index].clone();
            let object = match v {
                Value::Table(t) => {
                    let mut entries = t
                        .pairs::<Value, Value>()
                        .collect::<mlua::Result<Vec<_>>>()?;
                    entries.sort_by(|(ka, va), (kb, vb)| {
                        sort_key(ka, 0)
                            .cmp(&sort_key(kb, 0))
                            .then_with(|| sort_key(va, 0).cmp(&sort_key(vb, 0)))
                            .then_with(|| {
                                self.seen
                                    .get(&(ka.to_pointer() as usize))
                                    .cmp(&self.seen.get(&(kb.to_pointer() as usize)))
                            })
                            .then_with(|| {
                                self.seen
                                    .get(&(va.to_pointer() as usize))
                                    .cmp(&self.seen.get(&(vb.to_pointer() as usize)))
                            })
                    });
                    let mut encoded = Vec::with_capacity(entries.len());
                    for (key, value) in entries {
                        encoded.push(json!({"key":self.value(key)?,"value":self.value(value)?}));
                    }
                    let mut o = json!({"kind":"table","id":id,"entries":encoded});
                    if let Some(mt) = t.metatable() {
                        let array = mt.raw_get::<bool>("__kui_array").unwrap_or(false);
                        if array {
                            o["array"] = json!(true);
                        }
                        if !array || mt.pairs::<Value, Value>().count() != 1 {
                            o["metatable"] = json!("opaque");
                            o["restorable"] = json!(false);
                        }
                    }
                    o
                }
                Value::UserData(ud) if ud.is::<LuaChannel>() => {
                    let ch = ud.borrow::<LuaChannel>()?;
                    let mut columns = serde_json::Map::new();
                    for name in ch.0.names() {
                        let column = ch.0.column_named(name).unwrap();
                        let values = match column {
                            Column::F64(v) => {
                                json!(v.iter().map(f64::to_string).collect::<Vec<_>>())
                            }
                            Column::I64(v) => json!(v),
                            Column::Bool(v) => json!(v),
                            Column::Str(v) => json!(
                                (0..v.len())
                                    .map(|i| v.get(i).unwrap_or(""))
                                    .collect::<Vec<_>>()
                            ),
                        };
                        columns.insert(name.clone(), json!({"type":column.kind(),"values":values}));
                    }
                    json!({"kind":"channel","id":id,"revision":ch.0.rev(),"names":ch.0.names(),"columns_data":columns,"bytes_base64":ch.0.as_bytes().map(|v|base64::engine::general_purpose::STANDARD.encode(v))})
                }
                other => json!({"kind":other.type_name(),"id":id,"opaque":true,"restorable":false}),
            };
            self.objects[index] = object;
            index += 1;
        }
        Ok(json!({"version":1,"root":root,"objects":self.objects}))
    }
}
fn sort_key(v: &Value, depth: usize) -> String {
    match v {
        Value::Integer(n) => format!("0:{:020}", (*n as i128) + (1i128 << 64)),
        Value::Number(n) => format!(
            "1:{:020}",
            if n.is_sign_negative() {
                !n.to_bits()
            } else {
                n.to_bits() | (1 << 63)
            }
        ),
        Value::String(s) => format!(
            "2:{}",
            s.as_bytes()
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        ),
        Value::Boolean(b) => format!("3:{b}"),
        Value::Table(t) if depth < 4 => {
            let mut rows = t
                .pairs::<Value, Value>()
                .filter_map(Result::ok)
                .map(|(k, v)| format!("{}={}", sort_key(&k, depth + 1), sort_key(&v, depth + 1)))
                .collect::<Vec<_>>();
            rows.sort();
            format!("4:{{{}}}", rows.join(","))
        }
        _ => format!("5:{}", v.type_name()),
    }
}
pub fn encode(value: Value) -> mlua::Result<String> {
    serde_json::to_string(&Graph::default().finish(value)?).map_err(mlua::Error::external)
}
pub fn install(lua: &Lua, kui: &Table) -> mlua::Result<()> {
    kui.set(
        "__snapshot_encode",
        lua.create_function(|_, value: Value| encode(value))?,
    )?;
    kui.set(
        "__restore_channel",
        lua.create_function(|lua, snapshot: Table| {
            let revision = snapshot.get::<u64>("revision")?;
            if let Some(bytes) = snapshot.get::<Option<String>>("bytes_base64")? {
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(bytes)
                    .map_err(mlua::Error::external)?;
                return lua.create_userdata(LuaChannel(
                    Channel::from_snapshot(Data::Bytes(bytes), revision)
                        .map_err(mlua::Error::external)?,
                ));
            }
            let columns: Table = snapshot.get("columns_data")?;
            let names = snapshot.get::<Option<Vec<String>>>("names")?;
            let mut out = Vec::new();
            for pair in columns.pairs::<String, Table>() {
                let (name, column) = pair?;
                let values: Table = column.get("values")?;
                let kind = column.get::<String>("type")?;
                let column = match kind.as_str() {
                    "integer" => Column::I64(
                        values
                            .sequence_values::<i64>()
                            .collect::<mlua::Result<_>>()?,
                    ),
                    "number" => Column::F64(
                        values
                            .sequence_values::<String>()
                            .map(|s| {
                                s.and_then(|s| s.parse::<f64>().map_err(mlua::Error::external))
                            })
                            .collect::<mlua::Result<_>>()?,
                    ),
                    "boolean" => Column::Bool(
                        values
                            .sequence_values::<bool>()
                            .collect::<mlua::Result<_>>()?,
                    ),
                    "string" => Column::strs(
                        values
                            .sequence_values::<String>()
                            .collect::<mlua::Result<Vec<_>>>()?,
                    ),
                    _ => return Err(mlua::Error::runtime("unknown captured channel column type")),
                };
                out.push((name, column));
            }
            if let Some(names) = names {
                out.sort_by_key(|(name, _)| {
                    names.iter().position(|n| n == name).unwrap_or(usize::MAX)
                });
            } else {
                out.sort_by(|a, b| a.0.cmp(&b.0));
            }
            lua.create_userdata(LuaChannel(
                Channel::from_snapshot(
                    Data::Table {
                        names: out.iter().map(|v| v.0.clone()).collect(),
                        cols: out.into_iter().map(|v| v.1).collect(),
                    },
                    revision,
                )
                .map_err(mlua::Error::external)?,
            ))
        })?,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn aliased_table_keys_are_stable_across_insertion_orders() {
        let lua = Lua::new();
        let make = |order: &str| {
            encode(
                lua.load(format!(
                    "local a,b={{}},{{}};return {{a=a,b=b,map={order}}}"
                ))
                .eval()
                .unwrap(),
            )
            .unwrap()
        };
        let expected = make("{[a]=1,[b]=1}");
        for _ in 0..100 {
            assert_eq!(make("{[b]=1,[a]=1}"), expected);
        }
        let nested = |order: &str| {
            encode(
                lua.load(format!(
                    "local a,b={{}},{{}};return {{map={order},z={{a=a,b=b}}}}"
                ))
                .eval()
                .unwrap(),
            )
            .unwrap()
        };
        let expected = nested("{[a]=1,[b]=1}");
        for _ in 0..100 {
            assert_eq!(nested("{[b]=1,[a]=1}"), expected);
        }
    }
    #[test]
    fn saved_graph_restores_aliases_binary_strings_channel_keys_types_order_and_revision() {
        let lua = Lua::new();
        kui_lua::install_data(&lua).unwrap();
        crate::modules::install(
            &lua,
            crate::modules::Shared::new(std::env::temp_dir(), None),
        )
        .unwrap();
        let channel = Channel::table(vec![
            ("z".into(), Column::I64(vec![i64::MAX])),
            ("a".into(), Column::F64(vec![-0.0])),
        ])
        .unwrap();
        let channel = channel
            .publish(vec![
                ("z".into(), Column::I64(vec![i64::MAX])),
                ("a".into(), Column::F64(vec![-0.0])),
            ])
            .unwrap();
        lua.globals()
            .set("original", LuaChannel(channel.clone()))
            .unwrap();
        lua.globals()
            .set(
                "empty",
                LuaChannel(
                    Channel::table(vec![
                        ("f".into(), Column::F64(vec![])),
                        ("i".into(), Column::I64(vec![])),
                    ])
                    .unwrap(),
                ),
            )
            .unwrap();
        lua.globals()
            .set("bytes", LuaChannel(Channel::bytes(vec![0, 255, 128])))
            .unwrap();
        let root=lua.load("local v={c=original,alias=original,empty=empty,bytes=bytes,binary=string.char(0,255)};v.self=v;v[original]='channel key';return v").eval::<Value>().unwrap();
        let text = encode(root).unwrap();
        let graph: Table = lua.load(include_str!("lua/snapshot.lua")).eval().unwrap();
        let restored: Table = graph
            .get::<mlua::Function>("decode")
            .unwrap()
            .call((text, false))
            .unwrap();
        lua.globals().set("restored", restored.clone()).unwrap();
        lua.load("assert(restored.self==restored and restored.c==restored.alias);assert(restored.c.id~=original.id and restored.c.rev==original.rev);assert(restored.c.names[1]=='z' and restored.c.names[2]=='a');assert(math.type(restored.c:get(0,'z'))=='integer');assert(1/restored.c:get(0,'a')==-math.huge);assert(restored.binary==string.char(0,255));local n=0;for k in pairs(restored) do if type(k)=='userdata' then n=n+1;assert(k==restored.c) end end;assert(n==1)").exec().unwrap();
        let bytes: mlua::AnyUserData = restored.get("bytes").unwrap();
        assert_eq!(
            bytes.borrow::<LuaChannel>().unwrap().0.as_bytes(),
            Some(&[0, 255, 128][..])
        );
        let empty: mlua::AnyUserData = restored.get("empty").unwrap();
        let empty = empty.borrow::<LuaChannel>().unwrap();
        assert!(matches!(empty.0.column_named("f"),Some(Column::F64(v)) if v.is_empty()));
        assert!(matches!(empty.0.column_named("i"),Some(Column::I64(v)) if v.is_empty()));
    }
}
