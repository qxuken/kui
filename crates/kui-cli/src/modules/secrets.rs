//! `kui.secrets`: text kept in the OS's own store — the Keychain on macOS,
//! the Credential Manager on Windows, the Secret Service (GNOME Keyring,
//! KWallet) on Linux — filed under the app's id, or the `service` a call
//! names. Every call waits: the store may be a D-Bus hop away, or the OS
//! may be asking the user whether the app may read it.
//!
//! There is no fallback to a file: a store that is missing or refused is
//! the call's error, said as such, and `probe` asks ahead of time, so an
//! app chooses what to do instead (ask each time, keep an age file under a
//! passphrase) rather than finding its token written out in the clear.
//! `KUI_SECRETS=memory` keeps them in the process instead, for a test.

use std::sync::{Arc, OnceLock};

use keyring_core::{CredentialStore, Error};
use kui_core::Value;
use mlua::{Lua, Table};

use super::{fail, shared, try_fn};

pub fn install(lua: &Lua, kui: &Table) -> mlua::Result<()> {
    let t = lua.create_table()?;
    t.set("store", store_name())?;
    t.set(
        "__try_get",
        try_fn(lua, |lua, (name, opts): (String, Option<Table>)| {
            let service = service(&name, opts)?;
            Ok(off_frame(lua, move || {
                match entry(&service, &name)?.get_password() {
                    Ok(v) => Ok(Value::Str(v)),
                    Err(Error::NoEntry) => Ok(Value::Null),
                    Err(e) => Err(message(e)),
                }
            }))
        })?,
    )?;
    t.set(
        "__try_set",
        try_fn(
            lua,
            |lua, (name, value, opts): (String, mlua::LuaString, Option<Table>)| {
                let service = service(&name, opts)?;
                let value = value
                    .to_str()
                    .map_err(|_| {
                        fail(
                            "[invalid] secrets: a secret is text; kui.base64.encode bytes first"
                                .into(),
                        )
                    })?
                    .to_string();
                Ok(off_frame(lua, move || {
                    entry(&service, &name)?
                        .set_password(&value)
                        .map_err(message)?;
                    Ok(Value::Null)
                }))
            },
        )?,
    )?;
    t.set(
        "__try_delete",
        try_fn(lua, |lua, (name, opts): (String, Option<Table>)| {
            let service = service(&name, opts)?;
            Ok(off_frame(lua, move || {
                match entry(&service, &name)?.delete_credential() {
                    Ok(()) => Ok(Value::Bool(true)),
                    Err(Error::NoEntry) => Ok(Value::Bool(false)),
                    Err(e) => Err(message(e)),
                }
            }))
        })?,
    )?;
    // Whether a secret can be kept and read back now: the store is there,
    // and it answers. A secret of its own is written, read and removed.
    t.set(
        "__try_probe",
        try_fn(lua, |lua, opts: Option<Table>| {
            let service = service("probe", opts)?;
            Ok(off_frame(lua, move || {
                let e = entry(&service, "kui.secrets.probe")?;
                e.set_password("probe").map_err(message)?;
                let back = e.get_password().map_err(message);
                let _ = e.delete_credential();
                if back? != "probe" {
                    return Err(
                        "[failed] secrets: the store did not give back what was put in it".into(),
                    );
                }
                Ok(Value::Bool(true))
            }))
        })?,
    )?;
    kui.set("secrets", t)?;
    Ok(())
}

/// The service a secret is filed under: `opts.service`, else the app's id.
fn service(name: &str, opts: Option<Table>) -> mlua::Result<String> {
    if name.is_empty() {
        return Err(fail("[invalid] secrets: a secret needs a name".into()));
    }
    if let Some(s) = opts
        .map(|o| o.get::<Option<String>>("service"))
        .transpose()?
        .flatten()
    {
        return Ok(s);
    }
    Ok(super::app()
        .map(|a| a.id.clone())
        .filter(|id| !id.is_empty())
        .unwrap_or_else(|| "kui".into()))
}

fn off_frame(lua: &Lua, work: impl FnOnce() -> Result<Value, String> + Send + 'static) -> u64 {
    let s = shared(lua);
    let id = s.ops.id();
    crate::ops::run(&s.ops, id, work);
    id
}

fn memory() -> bool {
    std::env::var("KUI_SECRETS").is_ok_and(|v| v == "memory")
}

/// What `kui.secrets.store` says the secrets are kept in.
fn store_name() -> &'static str {
    if memory() {
        "memory"
    } else if cfg!(target_os = "macos") {
        "keychain"
    } else if cfg!(windows) {
        "credential-manager"
    } else if cfg!(unix) {
        "secret-service"
    } else {
        "none"
    }
}

/// The store, opened once for the process (workers share it) on the first
/// call that needs it: on Linux that is a connection to the session bus.
fn store() -> Result<&'static Arc<CredentialStore>, String> {
    static STORE: OnceLock<Result<Arc<CredentialStore>, String>> = OnceLock::new();
    STORE
        .get_or_init(|| {
            let opened: keyring_core::Result<Arc<CredentialStore>> = if memory() {
                keyring_core::mock::Store::new().map(|s| s as Arc<CredentialStore>)
            } else {
                platform()
            };
            opened.map_err(|e| match e {
                Error::NoDefaultStore => {
                    format!(
                        "[unavailable] secrets: no store on {}",
                        std::env::consts::OS
                    )
                }
                e => format!(
                    "[unavailable] secrets: the {} cannot be opened: {e}",
                    store_name()
                ),
            })
        })
        .as_ref()
        .map_err(Clone::clone)
}

#[cfg(target_os = "macos")]
fn platform() -> keyring_core::Result<Arc<CredentialStore>> {
    apple_native_keyring_store::keychain::Store::new().map(|s| s as Arc<CredentialStore>)
}

#[cfg(windows)]
fn platform() -> keyring_core::Result<Arc<CredentialStore>> {
    windows_native_keyring_store::Store::new().map(|s| s as Arc<CredentialStore>)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn platform() -> keyring_core::Result<Arc<CredentialStore>> {
    zbus_secret_service_keyring_store::Store::new().map(|s| s as Arc<CredentialStore>)
}

#[cfg(not(any(unix, windows)))]
fn platform() -> keyring_core::Result<Arc<CredentialStore>> {
    Err(Error::NoDefaultStore)
}

fn entry(service: &str, name: &str) -> Result<keyring_core::Entry, String> {
    store()?.build(service, name, None).map_err(message)
}

/// What went wrong, said for the app's user: a refusal is not a failure.
/// Its `[code]` in front is split off by runtime.lua into the `try_`
/// form's third value (`denied`, `unavailable`, `failed`, `invalid`,
/// `encoding`, `too_long`), what an app matches on.
fn message(e: Error) -> String {
    let store = store_name();
    match e {
        Error::NoStorageAccess(e) => format!(
            "[denied] secrets: the {store} refused access (a prompt denied or dismissed, or a locked collection): {e}"
        ),
        Error::PlatformFailure(e) => format!("[failed] secrets: the {store} failed: {e}"),
        Error::NoDefaultStore => format!(
            "[unavailable] secrets: no store on {}",
            std::env::consts::OS
        ),
        Error::BadEncoding(_) => {
            "[encoding] secrets: the secret kept there is not UTF-8 text".into()
        }
        Error::TooLong(what, n) => {
            format!("[too_long] secrets: the {what} is longer than the {store} takes ({n})")
        }
        Error::Invalid(what, why) => format!("[invalid] secrets: the {what} is not valid: {why}"),
        e => format!("[failed] secrets: {e}"),
    }
}
