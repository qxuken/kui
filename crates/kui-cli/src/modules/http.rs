//! `kui.http`: a client (`get`, `post`, `request`, each an operation, so a
//! task waits without blocking the frame) and a server (`serve`, lua's half
//! in lua/runtime.lua) whose requests arrive in the state's inbox and run
//! as tasks; a handler's answer goes back through `__respond`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use kui_core::Value;
use mlua::{Lua, Table};

use crate::ops::{self, Inbound};

use super::{fail, shared, try_fn};

/// One running server: what stops it and the requests waiting on a handler.
pub struct Server {
    inner: Arc<tiny_http::Server>,
    pending: Arc<Mutex<HashMap<u64, tiny_http::Request>>>,
}

struct Req {
    method: String,
    url: String,
    headers: Vec<(String, String)>,
    body: Option<Vec<u8>>,
    timeout: Option<f64>,
}

fn request_opts(url_or_opts: mlua::Value, method: &str, extra: Option<Table>) -> mlua::Result<Req> {
    let mut r = Req {
        method: method.to_string(),
        url: String::new(),
        headers: Vec::new(),
        body: None,
        timeout: None,
    };
    fn read(r: &mut Req, t: &Table) -> mlua::Result<()> {
        if let Some(u) = t.get::<Option<String>>("url")? {
            r.url = u;
        }
        if let Some(m) = t.get::<Option<String>>("method")? {
            r.method = m.to_uppercase();
        }
        if let Some(h) = t.get::<Option<Table>>("headers")? {
            for pair in h.pairs::<String, String>() {
                r.headers.push(pair?);
            }
        }
        if let Some(j) = t.get::<Option<mlua::Value>>("json")?
            && !j.is_nil()
        {
            let mut out = String::new();
            super::formats::write_json(&mut out, &kui_lua::lua_to_value(&j)?, false, 0);
            r.body = Some(out.into_bytes());
            r.headers
                .push(("content-type".into(), "application/json".into()));
        }
        if let Some(b) = t.get::<Option<mlua::LuaString>>("body")? {
            r.body = Some(b.as_bytes().to_vec());
        }
        r.timeout = t.get::<Option<f64>>("timeout")?;
        Ok(())
    }
    match url_or_opts {
        mlua::Value::String(s) => r.url = s.to_str()?.to_string(),
        mlua::Value::Table(t) => read(&mut r, &t)?,
        other => {
            return Err(fail(format!(
                "http: expected a URL or an options table, got {}",
                other.type_name()
            )));
        }
    }
    if let Some(t) = extra {
        read(&mut r, &t)?;
    }
    if r.url.is_empty() {
        return Err(fail("http: no url".into()));
    }
    Ok(r)
}

fn is_json(content_type: Option<&str>) -> bool {
    content_type.is_some_and(|c| c.contains("json"))
}

fn fetch(r: Req) -> Result<Value, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs_f64(
            r.timeout.unwrap_or(30_000.0) / 1000.0,
        )))
        .build()
        .into();
    macro_rules! with_headers {
        ($b:expr) => {{
            let mut b = $b;
            for (k, v) in &r.headers {
                b = b.header(k, v);
            }
            b
        }};
    }
    let body = r.body.unwrap_or_default();
    let res = match r.method.as_str() {
        "GET" => with_headers!(agent.get(&r.url)).call(),
        "HEAD" => with_headers!(agent.head(&r.url)).call(),
        "DELETE" => with_headers!(agent.delete(&r.url)).call(),
        "POST" => with_headers!(agent.post(&r.url)).send(&body[..]),
        "PUT" => with_headers!(agent.put(&r.url)).send(&body[..]),
        "PATCH" => with_headers!(agent.patch(&r.url)).send(&body[..]),
        other => return Err(format!("http: no method {other}")),
    };
    let mut res = res.map_err(|e| format!("{} {}: {e}", r.method, r.url))?;
    let status = res.status().as_u16();
    let mut headers = Vec::new();
    for (k, v) in res.headers() {
        headers.push((
            k.as_str().to_string(),
            Value::Str(v.to_str().unwrap_or("").to_string()),
        ));
    }
    let content_type = res
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let text = res
        .body_mut()
        .with_config()
        .limit(256 * 1024 * 1024)
        .read_to_string()
        .unwrap_or_default();
    let mut out = vec![
        ("status", Value::Int(status.into())),
        ("ok", Value::Bool((200..300).contains(&status))),
        ("headers", Value::Map(headers)),
    ];
    if is_json(content_type.as_deref())
        && let Ok(j) = kui_core::json::parse(&text)
    {
        out.push(("json", j));
    }
    out.push(("body", Value::Str(text)));
    Ok(Value::map(out))
}

fn guess_type(path: &std::path::Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "wasm" => "application/wasm",
        "txt" | "md" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn header(k: &str, v: &str) -> Option<tiny_http::Header> {
    tiny_http::Header::from_bytes(k.as_bytes(), v.as_bytes()).ok()
}

/// A static file under `root` for `path`, refusing anything that climbs
/// out of it.
fn static_file(root: &std::path::Path, path: &str) -> Option<std::path::PathBuf> {
    let rel = path.trim_start_matches('/');
    if rel.split('/').any(|seg| seg == "..") {
        return None;
    }
    let mut p = root.join(rel);
    if p.is_dir() {
        p = p.join("index.html");
    }
    p.is_file().then_some(p)
}

fn query(q: &str) -> Value {
    let mut out = Vec::new();
    for pair in q.split('&').filter(|p| !p.is_empty()) {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        out.push((decode(k), Value::Str(decode(v))));
    }
    Value::Map(out)
}

fn decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < b.len() => {
                match u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or("zz"), 16)
                {
                    Ok(v) => {
                        out.push(v);
                        i += 2;
                    }
                    Err(_) => out.push(b'%'),
                }
            }
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn install(lua: &Lua, kui: &Table) -> mlua::Result<()> {
    let http = lua.create_table()?;
    http.set(
        "__try_request",
        try_fn(
            lua,
            |lua, (target, method, extra): (mlua::Value, Option<String>, Option<Table>)| {
                let r = request_opts(target, method.as_deref().unwrap_or("GET"), extra)?;
                let s = shared(lua);
                let id = s.ops.id();
                ops::run(&s.ops, id, move || fetch(r));
                Ok(id)
            },
        )?,
    )?;
    kui.set("http", http)?;

    kui.set(
        "__try_serve",
        try_fn(lua, |lua, opts: Table| {
            let s = shared(lua);
            let port: u16 = opts.get::<Option<u16>>("port")?.unwrap_or(0);
            let host: String = opts
                .get::<Option<String>>("host")?
                .unwrap_or_else(|| "127.0.0.1".into());
            let root = opts.get::<Option<String>>("static")?.map(|d| s.path(&d));
            let inner = tiny_http::Server::http((host.as_str(), port))
                .map_err(|e| fail(format!("http.serve on {host}:{port}: {e}")))?;
            let bound = inner
                .server_addr()
                .to_ip()
                .map(|a| a.port())
                .unwrap_or(port);
            let inner = Arc::new(inner);
            let pending: Arc<Mutex<HashMap<u64, tiny_http::Request>>> = Arc::default();
            let server_id = s.ops.id();
            {
                let (inner, pending, ops) = (inner.clone(), pending.clone(), s.ops.clone());
                std::thread::Builder::new()
                    .name(format!("kui-http-{bound}"))
                    .spawn(move || {
                        for mut req in inner.incoming_requests() {
                            let method = req.method().as_str().to_uppercase();
                            let url = req.url().to_string();
                            let (path, q) = url.split_once('?').unwrap_or((&url, ""));
                            if method == "GET"
                                && let Some(root) = &root
                                && let Some(file) = static_file(root, path)
                                && let Ok(f) = std::fs::File::open(&file)
                            {
                                let mut resp = tiny_http::Response::from_file(f);
                                if let Some(h) = header("Content-Type", guess_type(&file)) {
                                    resp = resp.with_header(h);
                                }
                                let _ = req.respond(resp);
                                continue;
                            }
                            let mut body = Vec::new();
                            let _ = req.as_reader().read_to_end(&mut body);
                            let mut headers = Vec::new();
                            let mut content_type = None;
                            for h in req.headers() {
                                let k = h.field.as_str().as_str().to_ascii_lowercase();
                                if k == "content-type" {
                                    content_type = Some(h.value.as_str().to_string());
                                }
                                headers.push((k, Value::Str(h.value.as_str().to_string())));
                            }
                            let text = String::from_utf8_lossy(&body).into_owned();
                            let mut v = vec![
                                ("method", Value::Str(method)),
                                ("path", Value::Str(decode(path))),
                                ("query", query(q)),
                                ("headers", Value::Map(headers)),
                            ];
                            if is_json(content_type.as_deref())
                                && let Ok(j) = kui_core::json::parse(&text)
                            {
                                v.push(("json", j));
                            }
                            v.push(("body", Value::Str(text)));
                            let id = ops.id();
                            pending.lock().unwrap().insert(id, req);
                            ops.send(Inbound::Request {
                                server: server_id,
                                id,
                                req: Value::map(v),
                            });
                        }
                    })
                    .map_err(|e| fail(format!("http.serve: {e}")))?;
            }
            s.servers
                .lock()
                .unwrap()
                .insert(server_id, Server { inner, pending });
            Ok((server_id, bound))
        })?,
    )?;
    kui.set(
        "__serve_stop",
        lua.create_function(|lua, id: u64| {
            if let Some(s) = shared(lua).servers.lock().unwrap().remove(&id) {
                s.inner.unblock();
            }
            Ok(())
        })?,
    )?;
    kui.set(
        "__respond",
        lua.create_function(|lua, (server, id, res): (u64, u64, Table)| {
            let s = shared(lua);
            let req = s
                .servers
                .lock()
                .unwrap()
                .get(&server)
                .and_then(|srv| srv.pending.lock().unwrap().remove(&id));
            let Some(req) = req else { return Ok(()) };
            let status: u16 = res.get::<Option<u16>>("status")?.unwrap_or(200);
            let mut content_type = "text/plain; charset=utf-8";
            let body: Vec<u8> = if let Some(j) = res
                .get::<Option<mlua::Value>>("json")?
                .filter(|v| !v.is_nil())
            {
                content_type = "application/json";
                let mut out = String::new();
                super::formats::write_json(&mut out, &kui_lua::lua_to_value(&j)?, false, 0);
                out.into_bytes()
            } else if let Some(h) = res.get::<Option<mlua::LuaString>>("html")? {
                content_type = "text/html; charset=utf-8";
                h.as_bytes().to_vec()
            } else if let Some(b) = res.get::<Option<mlua::LuaString>>("body")? {
                b.as_bytes().to_vec()
            } else {
                Vec::new()
            };
            let mut resp = tiny_http::Response::from_data(body).with_status_code(status);
            let mut typed = false;
            if let Some(h) = res.get::<Option<Table>>("headers")? {
                for pair in h.pairs::<String, String>() {
                    let (k, v) = pair?;
                    typed |= k.eq_ignore_ascii_case("content-type");
                    if let Some(h) = header(&k, &v) {
                        resp = resp.with_header(h);
                    }
                }
            }
            if !typed && let Some(h) = header("Content-Type", content_type) {
                resp = resp.with_header(h);
            }
            // Written off the frame: a slow client must not stall it.
            std::thread::spawn(move || {
                let _ = req.respond(resp);
            });
            Ok(())
        })?,
    )?;
    Ok(())
}
