//! `kui.http`: a client (`get`, `post`, `request`, each an operation, so a
//! task waits without blocking the frame) and a server (`serve`, lua's half
//! in lua/runtime.lua) whose requests arrive in the state's inbox and run
//! as tasks; a handler's answer goes back through `__respond`.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Condvar, LazyLock, Mutex};
use std::time::Duration;

use kui_core::Value;
use mlua::{Lua, Table};

use crate::ops::{self, Inbound};

use super::{fail, shared, try_fn};

mod pinning;
mod progress;

/// One running server: what stops it and the requests waiting on a handler.
pub struct Server {
    inner: Arc<tiny_http::Server>,
    pending: Arc<Mutex<HashMap<u64, tiny_http::Request>>>,
}

#[derive(Clone, Default)]
struct ClientOptions {
    headers: Vec<(String, String)>,
    timeout: Option<f64>,
    base_url: Option<String>,
    pins: Option<pinning::Pins>,
    pin_file: Option<std::path::PathBuf>,
    ca_file: Option<std::path::PathBuf>,
}

static DEFAULT_CLIENT: LazyLock<Arc<Client>> =
    LazyLock::new(|| Arc::new(Client::new(ClientOptions::default())));

struct Client {
    options: Mutex<ClientOptions>,
    pool: Arc<Pool>,
}

impl Client {
    fn new(options: ClientOptions) -> Self {
        Self {
            options: Mutex::new(options),
            pool: Arc::new(Pool::default()),
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
struct PoolKey {
    timeout: Duration,
    pins: Option<Vec<[u8; 32]>>,
    ca_digest: Option<[u8; 32]>,
}

#[derive(Default)]
struct PoolState {
    closed: bool,
    destroyed: bool,
    next: u64,
    running: HashMap<u64, Option<ureq::Agent>>,
    agents: VecDeque<(PoolKey, ureq::Agent)>,
}

#[derive(Default)]
struct Pool {
    state: Mutex<PoolState>,
    drained: Condvar,
}

struct RequestGuard {
    pool: Arc<Pool>,
    id: u64,
}

impl Drop for RequestGuard {
    fn drop(&mut self) {
        self.pool.state.lock().unwrap().running.remove(&self.id);
        self.pool.drained.notify_all();
    }
}

fn closed() -> String {
    super::coded("closed", "http: client is closed")
}

impl Pool {
    fn begin(self: &Arc<Self>) -> Result<RequestGuard, String> {
        let mut state = self.state.lock().unwrap();
        if state.closed {
            return Err(closed());
        }
        state.next += 1;
        let id = state.next;
        state.running.insert(id, None);
        Ok(RequestGuard {
            pool: self.clone(),
            id,
        })
    }

    fn agent(
        &self,
        id: u64,
        key: PoolKey,
        build: impl FnOnce() -> Result<ureq::Agent, String>,
    ) -> Result<ureq::Agent, String> {
        let mut state = self.state.lock().unwrap();
        if state.destroyed {
            return Err(closed());
        }
        let agent = if let Some(index) = state.agents.iter().position(|(k, _)| k == &key) {
            let entry = state.agents.remove(index).unwrap();
            let agent = entry.1.clone();
            state.agents.push_back(entry);
            agent
        } else {
            let agent = build()?;
            state.agents.push_back((key, agent.clone()));
            if state.agents.len() > 16 {
                state.agents.pop_front();
            }
            agent
        };
        *state.running.get_mut(&id).expect("registered HTTP request") = Some(agent.clone());
        Ok(agent)
    }

    fn start_close(&self) {
        self.state.lock().unwrap().closed = true;
    }

    fn close(&self) {
        let mut state = self.state.lock().unwrap();
        while !state.running.is_empty() {
            state = self.drained.wait(state).unwrap();
        }
        let agents = state
            .agents
            .drain(..)
            .map(|(_, agent)| agent)
            .collect::<Vec<_>>();
        drop(state);
        for agent in agents {
            agent.shutdown();
        }
    }

    fn destroy(&self) {
        let mut state = self.state.lock().unwrap();
        state.closed = true;
        state.destroyed = true;
        let agents = state
            .agents
            .iter()
            .map(|(_, a)| a.clone())
            .chain(state.running.values().filter_map(Clone::clone))
            .collect::<Vec<_>>();
        drop(state);
        for agent in agents {
            agent.shutdown();
        }
    }
}

impl Drop for Pool {
    fn drop(&mut self) {
        for (_, agent) in self.state.get_mut().unwrap().agents.drain(..) {
            agent.shutdown();
        }
    }
}

impl ClientOptions {
    fn parse(lua: &Lua, table: Option<Table>, context: &str) -> mlua::Result<Self> {
        let mut options = Self::default();
        if let Some(table) = table {
            for pair in table.clone().pairs::<String, mlua::Value>() {
                let (key, _) = pair?;
                if !matches!(
                    key.as_str(),
                    "headers" | "timeout" | "pins" | "pin_file" | "ca_file" | "base_url"
                ) {
                    return Err(super::fail_as(
                        "invalid",
                        format!("{context}: unknown option {key}"),
                    ));
                }
            }
            options.read(&table)?;
        }
        options.resolve_paths(&shared(lua).dir);
        Ok(options)
    }

    fn to_lua(&self, lua: &Lua) -> mlua::Result<Table> {
        use base64::{Engine, engine::general_purpose::STANDARD};
        let table = lua.create_table()?;
        let headers = lua.create_table()?;
        for (key, value) in &self.headers {
            headers.set(key.as_str(), value.as_str())?;
        }
        table.set("headers", headers)?;
        table.set("timeout", self.timeout)?;
        table.set("base_url", self.base_url.as_deref())?;
        if let Some(pins) = &self.pins {
            table.set(
                "pins",
                lua.create_sequence_from(
                    pins.0
                        .iter()
                        .map(|pin| format!("sha256/{}", STANDARD.encode(pin))),
                )?,
            )?;
        }
        if let Some(path) = &self.pin_file {
            table.set("pin_file", path.to_string_lossy().as_ref())?;
        }
        if let Some(path) = &self.ca_file {
            table.set("ca_file", path.to_string_lossy().as_ref())?;
        }
        Ok(table)
    }

    fn header(&mut self, key: String, value: String) -> mlua::Result<()> {
        use ureq::http::{HeaderName, HeaderValue};
        HeaderName::from_bytes(key.as_bytes())
            .map_err(|e| super::fail_as("invalid", format!("http header: {e}")))?;
        HeaderValue::from_str(&value)
            .map_err(|e| super::fail_as("invalid", format!("http header: {e}")))?;
        self.headers.retain(|(k, _)| !k.eq_ignore_ascii_case(&key));
        self.headers.push((key, value));
        Ok(())
    }

    fn read(&mut self, table: &Table) -> mlua::Result<()> {
        if let Some(base) = table.get::<Option<String>>("base_url")? {
            validate_base_url(&base)?;
            self.base_url = Some(base);
        }
        if let Some(headers) = table.get::<Option<Table>>("headers")? {
            for pair in headers.pairs::<String, String>() {
                let (key, value) = pair?;
                self.header(key, value)?;
            }
        }
        if let Some(ms) = table.get::<Option<f64>>("timeout")? {
            if ms <= 0.0 || Duration::try_from_secs_f64(ms / 1000.0).is_err() {
                return Err(super::fail_as(
                    "invalid",
                    "http: timeout must be finite positive milliseconds",
                ));
            }
            self.timeout = Some(ms);
        }
        let pins = table.get::<Option<mlua::Value>>("pins")?;
        let pin_file = table.get::<Option<String>>("pin_file")?;
        if pins.is_some() && pin_file.is_some() {
            return Err(super::fail_as(
                "invalid",
                "http: use either pins or pin_file",
            ));
        }
        if let Some(pins) = pins {
            self.pins = Some(pinning::Pins::from_lua(pins)?);
            self.pin_file = None;
        }
        if let Some(path) = pin_file {
            if path.is_empty() {
                return Err(super::fail_as(
                    "invalid",
                    "http: pin_file must not be empty",
                ));
            }
            self.pin_file = Some(path.into());
            self.pins = None;
        }
        if let Some(path) = table.get::<Option<String>>("ca_file")? {
            if path.is_empty() {
                return Err(super::fail_as("invalid", "http: ca_file must not be empty"));
            }
            self.ca_file = Some(path.into());
        }
        Ok(())
    }

    fn resolve_paths(&mut self, dir: &std::path::Path) {
        self.pin_file = self.pin_file.take().map(|path| dir.join(path));
        self.ca_file = self.ca_file.take().map(|path| dir.join(path));
    }
}

struct Req {
    method: String,
    url: String,
    body: Option<Vec<u8>>,
    client: ClientOptions,
}

fn request_opts(url_or_opts: mlua::Value, method: &str, extra: Option<Table>) -> mlua::Result<Req> {
    let mut r = Req {
        method: method.to_string(),
        url: String::new(),
        body: None,
        client: ClientOptions::default(),
    };
    fn read(r: &mut Req, t: &Table) -> mlua::Result<()> {
        r.client.read(t)?;
        if let Some(u) = t.get::<Option<String>>("url")? {
            r.url = u;
        }
        if let Some(m) = t.get::<Option<String>>("method")? {
            r.method = m.to_uppercase();
        }
        if let Some(j) = t.get::<Option<mlua::Value>>("json")?
            && !j.is_nil()
        {
            let mut out = String::new();
            super::formats::write_json(&mut out, &kui_lua::lua_to_value(&j)?, false, 0);
            r.body = Some(out.into_bytes());
            r.client
                .header("content-type".into(), "application/json".into())?;
        }
        if let Some(b) = t.get::<Option<mlua::LuaString>>("body")? {
            r.body = Some(b.as_bytes().to_vec());
        }
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
    if let Some(base) = &r.client.base_url {
        r.url = resolve_url(Some(base), &r.url)?;
    }
    if r.url.is_empty() {
        return Err(super::fail_as("invalid", "http: no url"));
    }
    if r.client.pins.is_some() || r.client.pin_file.is_some() {
        let uri = r
            .url
            .parse::<ureq::http::Uri>()
            .map_err(|_| super::fail_as("invalid", "http: pinning requires a valid HTTPS URL"))?;
        if uri.scheme_str() != Some("https") || uri.host().is_none() {
            return Err(super::fail_as(
                "invalid",
                "http: pinning requires an HTTPS URL",
            ));
        }
    }
    Ok(r)
}

fn validate_base_url(base: &str) -> mlua::Result<()> {
    let uri = base
        .parse::<ureq::http::Uri>()
        .map_err(|_| super::fail_as("invalid", "http: base_url must be an absolute HTTP(S) URL"))?;
    if !matches!(uri.scheme_str(), Some("http" | "https"))
        || uri.host().is_none()
        || uri.authority().is_some_and(|a| a.as_str().contains('@'))
        || base.contains(['?', '#'])
    {
        return Err(super::fail_as(
            "invalid",
            "http: base_url requires HTTP(S), a host, and no user info, query or fragment",
        ));
    }
    Ok(())
}

fn resolve_url(base: Option<&str>, url: &str) -> mlua::Result<String> {
    if url.is_empty() && base.is_none() {
        return Err(super::fail_as("invalid", "http: no url"));
    }
    if let Ok(uri) = url.parse::<ureq::http::Uri>()
        && uri.scheme().is_some()
    {
        validate_base_url_for_request(&uri)?;
        return Ok(url.to_owned());
    }
    if url.starts_with("//")
        || url
            .split(['/', '?', '#'])
            .next()
            .is_some_and(|s| s.contains(':'))
    {
        return Err(super::fail_as(
            "invalid",
            "http: expected a relative path or absolute HTTP(S) URL",
        ));
    }
    let base =
        base.ok_or_else(|| super::fail_as("invalid", "http: relative URL requires base_url"))?;
    validate_base_url(base)?;
    let joined = format!(
        "{}/{}",
        base.trim_end_matches('/'),
        url.trim_start_matches('/')
    );
    let uri = joined
        .parse::<ureq::http::Uri>()
        .map_err(|_| super::fail_as("invalid", "http: invalid URL"))?;
    validate_base_url_for_request(&uri)?;
    Ok(joined)
}

fn validate_base_url_for_request(uri: &ureq::http::Uri) -> mlua::Result<()> {
    if !matches!(uri.scheme_str(), Some("http" | "https")) || uri.host().is_none() {
        return Err(super::fail_as(
            "invalid",
            "http: expected an absolute HTTP(S) URL",
        ));
    }
    Ok(())
}

fn is_json(content_type: Option<&str>) -> bool {
    content_type.is_some_and(|c| c.contains("json"))
}

/// A request's failure by its code: `timeout`, `dns` (the host has no
/// address), `refused` (nothing answered: refused, reset, unreachable),
/// `tls`, `invalid` (the URL), `too_long` (a body past the limit), else
/// `failed`. A status is a response, not a failure.
fn http_code(e: &ureq::Error) -> &'static str {
    use ureq::Error as E;
    match e {
        E::Timeout(_) => "timeout",
        E::HostNotFound => "dns",
        E::ConnectionFailed => "refused",
        E::Io(io) => match super::io_code(io) {
            "not_found" => "dns",
            code => code,
        },
        E::Tls(_) | E::Pem(_) | E::Rustls(_) | E::TlsRequired => "tls",
        E::BadUri(_) | E::InvalidProxyUrl | E::RequireHttpsOnly(_) | E::Http(_) => "invalid",
        E::BodyExceedsLimit(_) | E::LargeResponseHeader(..) => "too_long",
        _ => "failed",
    }
}

fn fetch(
    r: Req,
    request: &RequestGuard,
    progress: Option<progress::Target>,
) -> Result<Value, String> {
    let pins = match r.client.pin_file {
        Some(path) => Some(pinning::Pins::from_file(&path)?),
        None => r.client.pins,
    };
    let timeout = Duration::from_secs_f64(r.client.timeout.unwrap_or(30_000.0) / 1000.0);
    let ca = r
        .client
        .ca_file
        .as_deref()
        .map(crate::tls::read_ca_file)
        .transpose()?;
    let key = PoolKey {
        timeout,
        pins: pins.as_ref().map(|pins| pins.0.clone()),
        ca_digest: ca.as_ref().map(|bytes| {
            ring::digest::digest(&ring::digest::SHA256, bytes)
                .as_ref()
                .try_into()
                .unwrap()
        }),
    };
    let agent = request.pool.agent(request.id, key, || {
        let config = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .https_only(pins.is_some())
            .timeout_global(Some(timeout))
            .build();
        crate::tls::agent_with_ca(config, pins, ca.as_deref())
    })?;
    macro_rules! with_headers {
        ($b:expr) => {{
            let mut b = $b;
            for (k, v) in &r.client.headers {
                b = b.header(k, v);
            }
            b
        }};
    }
    let body = r.body.unwrap_or_default();
    macro_rules! send_body {
        ($builder:expr) => {{
            let mut builder = with_headers!($builder);
            if let Some(progress) = &progress {
                if !r.client.headers.iter().any(|(key, _)| {
                    key.eq_ignore_ascii_case("content-length")
                        || key.eq_ignore_ascii_case("transfer-encoding")
                }) {
                    builder = builder.header("content-length", body.len().to_string());
                }
                let total = body.len() as u64;
                let reader = progress.reader(std::io::Cursor::new(body), "upload", Some(total));
                builder.send(ureq::SendBody::from_owned_reader(reader))
            } else {
                builder.send(&body[..])
            }
        }};
    }
    let res = match r.method.as_str() {
        "GET" => with_headers!(agent.get(&r.url)).call(),
        "HEAD" => with_headers!(agent.head(&r.url)).call(),
        "DELETE" => with_headers!(agent.delete(&r.url)).call(),
        "POST" => send_body!(agent.post(&r.url)),
        "PUT" => send_body!(agent.put(&r.url)),
        "PATCH" => send_body!(agent.patch(&r.url)),
        other => return Err(super::coded("invalid", format!("http: no method {other}"))),
    };
    let mut res =
        res.map_err(|e| super::coded(http_code(&e), format!("{} {}: {e}", r.method, r.url)))?;
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
    let total = if r.method == "HEAD" || matches!(status, 204 | 304) {
        Some(0)
    } else {
        res.body().content_length().or_else(|| {
            (!res.headers().contains_key("transfer-encoding")
                && !res.headers().contains_key("content-encoding")
                && res
                    .headers()
                    .get("content-length")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok())
                    == Some(0))
            .then_some(0)
        })
    };
    let mut reader = res
        .body_mut()
        .with_config()
        .limit(256 * 1024 * 1024)
        .reader();
    let mut text = String::new();
    use std::io::Read;
    let read_result = if let Some(progress) = progress {
        let mut reader = progress.reader(&mut reader, "download", total);
        let result = reader.read_to_string(&mut text);
        if result.is_ok() && !request.pool.state.lock().unwrap().destroyed {
            reader.finish();
        }
        result
    } else {
        reader.read_to_string(&mut text)
    };
    read_result.map_err(|e| {
        let error = ureq::Error::from(e);
        super::coded(
            http_code(&error),
            format!("{} {} body: {error}", r.method, r.url),
        )
    })?;
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

fn client_table(lua: &Lua, client: Arc<Client>) -> mlua::Result<Table> {
    let http = lua.create_table()?;
    let defaults = client.clone();
    http.set(
        "__try_defaults",
        try_fn(lua, move |lua, ()| {
            if defaults.pool.state.lock().unwrap().closed {
                return Err(super::fail_as("closed", "http: client is closed"));
            }
            defaults.options.lock().unwrap().clone().to_lua(lua)
        })?,
    )?;
    let config = client.clone();
    http.set(
        "try_configure",
        try_fn(lua, move |lua, table: Option<Table>| {
            let options = ClientOptions::parse(lua, table, "http.configure")?;
            if config.pool.state.lock().unwrap().closed {
                return Err(super::fail_as("closed", "http: client is closed"));
            }
            *config.options.lock().unwrap() = options;
            Ok(())
        })?,
    )?;
    let pool = client.pool.clone();
    http.set(
        "__try_close",
        try_fn(lua, move |lua, ()| {
            pool.start_close();
            let s = shared(lua);
            let id = s.ops.id();
            let pool = pool.clone();
            ops::run(&s.ops, id, move || {
                pool.close();
                Ok(Value::Null)
            });
            Ok(id)
        })?,
    )?;
    let pool = client.pool.clone();
    http.set(
        "__destroy",
        lua.create_function(move |_, ()| {
            pool.destroy();
            Ok(())
        })?,
    )?;
    http.set(
        "__try_resolve",
        try_fn(lua, |_, (url, base): (mlua::Value, Option<mlua::Value>)| {
            let mlua::Value::String(url) = url else {
                return Err(super::fail_as("invalid", "http: expected a URL string"));
            };
            let base = match base {
                None => None,
                Some(mlua::Value::String(s)) => Some(s.to_str()?.to_owned()),
                _ => {
                    return Err(super::fail_as(
                        "invalid",
                        "http: expected a base_url string",
                    ));
                }
            };
            resolve_url(base.as_deref(), &url.to_str()?)
        })?,
    )?;
    http.set(
        "__try_progress",
        try_fn(lua, |_, (callback, event): (mlua::Function, Table)| {
            callback.call::<()>(event)
        })?,
    )?;
    http.set(
        "__try_request",
        try_fn(
            lua,
            move |lua, (target, method, report): (mlua::Value, Option<String>, Option<bool>)| {
                let mut r = request_opts(target, method.as_deref().unwrap_or("GET"), None)?;
                let s = shared(lua);
                r.client.resolve_paths(&s.dir);
                let request = client.pool.begin().map_err(mlua::Error::RuntimeError)?;
                let id = s.ops.id();
                let progress = report.unwrap_or(false).then(|| progress::Target {
                    ops: s.ops.clone(),
                    id,
                });
                ops::run(&s.ops, id, move || {
                    let result = fetch(r, &request, progress);
                    // Shutdown can end a close-delimited body with a valid
                    // EOF, so cancellation must also override a successful
                    // transport result.
                    if request.pool.state.lock().unwrap().destroyed {
                        Err(closed())
                    } else {
                        result
                    }
                });
                Ok(id)
            },
        )?,
    )?;
    Ok(http)
}

pub fn install(lua: &Lua, kui: &Table) -> mlua::Result<()> {
    let http = client_table(lua, DEFAULT_CLIENT.clone())?;
    http.set(
        "__try_client",
        try_fn(lua, |lua, table: Option<Table>| {
            let options = ClientOptions::parse(lua, table, "http.client")?;
            client_table(lua, Arc::new(Client::new(options)))
        })?,
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
            let inner = tiny_http::Server::http((host.as_str(), port)).map_err(|e| {
                let code = e
                    .downcast_ref::<std::io::Error>()
                    .map_or("failed", super::io_code);
                super::fail_as(code, format!("http.serve on {host}:{port}: {e}"))
            })?;
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
