//! Bytes in and out of text, digests and ciphers: `kui.base64`, `kui.hex`,
//! `kui.hash` and `kui.crypto`.
//!
//! Lua strings are bytes, so a key, a digest or a ciphertext is a plain
//! string and `kui.base64` or `kui.hex` makes text of it. What is quick
//! (a digest, a seal) answers at once; what is slow on purpose (a key
//! stretched from a password) runs off the
//! frame as a waiting call, its bytes crossing back as base64 that
//! runtime.lua decodes, an answer being a `Value` and a `Value` holding text.

use std::num::NonZeroU32;

use base64::Engine;
use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE_NO_PAD};
use kui_core::Value;
use mlua::{Lua, LuaString, Table};
use ring::rand::{SecureRandom, SystemRandom};
use ring::{aead, digest, hmac, pbkdf2};

use super::{fail, fail_as, shared, try_fn};

pub fn install(lua: &Lua, kui: &Table) -> mlua::Result<()> {
    install_text(lua, kui)?;
    install_hash(lua, kui)?;
    install_crypto(lua, kui)
}

/// `kui.base64` and `kui.hex`.
fn install_text(lua: &Lua, kui: &Table) -> mlua::Result<()> {
    let b64 = lua.create_table()?;
    // Standard and padded unless `url` asks for the URL alphabet, which
    // goes unpadded (a JWT's, a URL's) unless `pad` says otherwise.
    b64.set(
        "encode",
        lua.create_function(|_, (data, opts): (LuaString, Option<Table>)| {
            let (url, pad) = match opts {
                Some(o) => {
                    let url = o.get::<Option<bool>>("url")?.unwrap_or(false);
                    (url, o.get::<Option<bool>>("pad")?.unwrap_or(!url))
                }
                None => (false, true),
            };
            let bytes = data.as_bytes();
            Ok(match (url, pad) {
                (false, true) => STANDARD.encode(&*bytes),
                (false, false) => STANDARD_NO_PAD.encode(&*bytes),
                (true, false) => URL_SAFE_NO_PAD.encode(&*bytes),
                (true, true) => base64::engine::general_purpose::URL_SAFE.encode(&*bytes),
            })
        })?,
    )?;
    b64.set(
        "try_decode",
        try_fn(lua, |lua, text: LuaString| {
            let bytes = b64_decode(&text.as_bytes()).map_err(|e| fail_as("parse", e))?;
            lua.create_string(bytes)
        })?,
    )?;
    kui.set("base64", b64)?;

    let hex = lua.create_table()?;
    hex.set(
        "encode",
        lua.create_function(|_, data: LuaString| Ok(to_hex(&data.as_bytes())))?,
    )?;
    hex.set(
        "try_decode",
        try_fn(lua, |lua, text: LuaString| {
            let bytes = from_hex(&text.as_bytes()).map_err(|e| fail_as("parse", e))?;
            lua.create_string(bytes)
        })?,
    )?;
    kui.set("hex", hex)?;
    Ok(())
}

/// `kui.hash`: digests, and HMAC under a key.
fn install_hash(lua: &Lua, kui: &Table) -> mlua::Result<()> {
    let hash = lua.create_table()?;
    for name in ["sha1", "sha256", "sha384", "sha512"] {
        let alg = digest_alg(name).expect("a listed digest");
        hash.set(
            name,
            lua.create_function(move |lua, (data, enc): (LuaString, Option<String>)| {
                let enc = Enc::read(enc.as_deref())?;
                enc.out(lua, digest::digest(alg, &data.as_bytes()).as_ref())
            })?,
        )?;
    }
    hash.set(
        "hmac",
        lua.create_function(
            |lua, (alg, key, data, enc): (String, LuaString, LuaString, Option<String>)| {
                let alg = hmac_alg(&alg).ok_or_else(|| {
                    fail(format!(
                        "kui.hash.hmac: no digest named {alg:?}; sha1, sha256, sha384 or sha512"
                    ))
                })?;
                let enc = Enc::read(enc.as_deref())?;
                let key = hmac::Key::new(alg, &key.as_bytes());
                enc.out(lua, hmac::sign(&key, &data.as_bytes()).as_ref())
            },
        )?,
    )?;
    hash.set(
        "fnv",
        lua.create_function(|_, data: LuaString| {
            let mut h: u64 = 0xcbf29ce484222325;
            for b in data.as_bytes().iter() {
                h = (h ^ *b as u64).wrapping_mul(0x100000001b3);
            }
            Ok(format!("{h:016x}"))
        })?,
    )?;
    kui.set("hash", hash)?;
    Ok(())
}

/// `kui.crypto`: random bytes, a constant-time compare, AEAD sealing, and
/// keys and password hashes stretched from passwords.
fn install_crypto(lua: &Lua, kui: &Table) -> mlua::Result<()> {
    let crypto = lua.create_table()?;
    crypto.set(
        "random",
        lua.create_function(|lua, n: usize| {
            if n > 1 << 20 {
                return Err(fail(format!(
                    "kui.crypto.random: {n} bytes asked, 1 MiB at most"
                )));
            }
            lua.create_string(random(n)?)
        })?,
    )?;
    crypto.set(
        "equal",
        lua.create_function(|_, (a, b): (LuaString, LuaString)| {
            Ok(equal(&a.as_bytes(), &b.as_bytes()))
        })?,
    )?;
    crypto.set(
        "try_encrypt",
        try_fn(
            lua,
            |lua, (key, data, opts): (LuaString, LuaString, Option<Table>)| {
                let (alg, aad) = aead_opts(opts)?;
                let key = aead_key(alg, &key.as_bytes())?;
                let nonce: [u8; aead::NONCE_LEN] = random(aead::NONCE_LEN)?.try_into().unwrap();
                let mut sealed = data.as_bytes().to_vec();
                key.seal_in_place_append_tag(
                    aead::Nonce::assume_unique_for_key(nonce),
                    aead::Aad::from(&aad[..]),
                    &mut sealed,
                )
                .map_err(|_| fail("crypto: cannot encrypt".into()))?;
                let mut out = nonce.to_vec();
                out.extend_from_slice(&sealed);
                lua.create_string(out)
            },
        )?,
    )?;
    crypto.set(
        "try_decrypt",
        try_fn(
            lua,
            |lua, (key, data, opts): (LuaString, LuaString, Option<Table>)| {
                let (alg, aad) = aead_opts(opts)?;
                let key = aead_key(alg, &key.as_bytes())?;
                let data = data.as_bytes();
                if data.len() < aead::NONCE_LEN + alg.tag_len() {
                    return Err(fail_as(
                        "invalid",
                        format!(
                            "crypto: {} bytes is too short to be what kui.crypto.encrypt wrote",
                            data.len()
                        ),
                    ));
                }
                let (nonce, sealed) = data.split_at(aead::NONCE_LEN);
                let nonce = aead::Nonce::try_assume_unique_for_key(nonce).unwrap();
                let mut buf = sealed.to_vec();
                let plain = key
                    .open_in_place(nonce, aead::Aad::from(&aad[..]), &mut buf)
                    .map_err(|_| {
                        fail_as(
                            "integrity",
                            "crypto: cannot decrypt: the wrong key or aad, or the data was altered",
                        )
                    })?;
                lua.create_string(&*plain)
            },
        )?,
    )?;
    crypto.set(
        "__try_pbkdf2",
        try_fn(
            lua,
            |lua, (password, salt, opts): (LuaString, LuaString, Option<Table>)| {
                let mut iterations = 600_000u32;
                let mut length = 32usize;
                let mut alg = pbkdf2::PBKDF2_HMAC_SHA256;
                if let Some(o) = opts {
                    if let Some(n) = o.get::<Option<u32>>("iterations")? {
                        iterations = n;
                    }
                    if let Some(n) = o.get::<Option<usize>>("length")? {
                        length = n;
                    }
                    if let Some(h) = o.get::<Option<String>>("hash")? {
                        alg = match h.as_str() {
                            "sha256" => pbkdf2::PBKDF2_HMAC_SHA256,
                            "sha384" => pbkdf2::PBKDF2_HMAC_SHA384,
                            "sha512" => pbkdf2::PBKDF2_HMAC_SHA512,
                            _ => {
                                return Err(fail_as(
                                    "invalid",
                                    format!(
                                        "crypto: pbkdf2 takes sha256, sha384 or sha512, not {h:?}"
                                    ),
                                ));
                            }
                        };
                    }
                }
                let iterations = NonZeroU32::new(iterations).ok_or_else(|| {
                    fail_as("invalid", "crypto: pbkdf2 needs at least 1 iteration")
                })?;
                key_length(length)?;
                let (password, salt) = (password.as_bytes().to_vec(), salt.as_bytes().to_vec());
                Ok(off_frame(lua, move || {
                    let mut out = vec![0u8; length];
                    pbkdf2::derive(alg, iterations, &salt, &password, &mut out);
                    Ok(STANDARD.encode(out))
                }))
            },
        )?,
    )?;
    crypto.set(
        "__try_argon2",
        try_fn(
            lua,
            |lua, (password, salt, opts): (LuaString, LuaString, Option<Table>)| {
                let (argon, length) = argon2_of(opts.as_ref())?;
                key_length(length)?;
                let (password, salt) = (password.as_bytes().to_vec(), salt.as_bytes().to_vec());
                if salt.len() < argon2::MIN_SALT_LEN {
                    return Err(fail_as(
                        "invalid",
                        format!(
                            "crypto: argon2 wants a salt of {} bytes or more; this one is {}",
                            argon2::MIN_SALT_LEN,
                            salt.len()
                        ),
                    ));
                }
                Ok(off_frame(lua, move || {
                    let mut out = vec![0u8; length];
                    argon
                        .hash_password_into(&password, &salt, &mut out)
                        .map_err(|e| format!("crypto: argon2: {e}"))?;
                    Ok(STANDARD.encode(out))
                }))
            },
        )?,
    )?;
    // A password kept to check later: argon2id, a fresh salt, and the
    // parameters, in the PHC string format (`$argon2id$v=19$m=…`).
    crypto.set(
        "__try_hash_password",
        try_fn(lua, |lua, (password, opts): (LuaString, Option<Table>)| {
            use argon2::password_hash::{PasswordHasher, SaltString};
            let (argon, _) = argon2_of(opts.as_ref())?;
            let salt =
                SaltString::encode_b64(&random(16)?).map_err(|e| fail(format!("crypto: {e}")))?;
            let password = password.as_bytes().to_vec();
            Ok(off_frame(lua, move || {
                argon
                    .hash_password(&password, &salt)
                    .map(|h| h.to_string())
                    .map_err(|e| format!("crypto: argon2: {e}"))
            }))
        })?,
    )?;
    crypto.set(
        "__try_verify_password",
        try_fn(lua, |lua, (password, hash): (LuaString, String)| {
            use argon2::password_hash::{PasswordHash, PasswordVerifier};
            PasswordHash::new(&hash)
                .map_err(|e| fail_as("invalid", format!("crypto: not a password hash ({e})")))?;
            let password = password.as_bytes().to_vec();
            let id = shared(lua).ops.id();
            crate::ops::run(&shared(lua).ops, id, move || {
                let parsed = PasswordHash::new(&hash).map_err(|e| e.to_string())?;
                match argon2::Argon2::default().verify_password(&password, &parsed) {
                    Ok(()) => Ok(Value::Bool(true)),
                    Err(argon2::password_hash::Error::Password) => Ok(Value::Bool(false)),
                    Err(e) => Err(format!("crypto: argon2: {e}")),
                }
            });
            Ok(id)
        })?,
    )?;
    kui.set("crypto", crypto)?;
    Ok(())
}

/// Runs `work` on a thread of its own, answering the operation id
/// returned with the text it makes.
fn off_frame(lua: &Lua, work: impl FnOnce() -> Result<String, String> + Send + 'static) -> u64 {
    let s = shared(lua);
    let id = s.ops.id();
    crate::ops::run(&s.ops, id, move || work().map(Value::Str));
    id
}

/// How a digest comes back: hex unless the call names another.
#[derive(Clone, Copy)]
enum Enc {
    Hex,
    Base64,
    Base64Url,
    Raw,
}

impl Enc {
    fn read(name: Option<&str>) -> mlua::Result<Enc> {
        Ok(match name {
            None | Some("hex") => Enc::Hex,
            Some("base64") => Enc::Base64,
            Some("base64url") => Enc::Base64Url,
            Some("raw") => Enc::Raw,
            Some(other) => {
                return Err(fail(format!(
                    "kui.hash: no encoding named {other:?}; hex, base64, base64url or raw"
                )));
            }
        })
    }

    fn out(self, lua: &Lua, bytes: &[u8]) -> mlua::Result<LuaString> {
        match self {
            Enc::Hex => lua.create_string(to_hex(bytes)),
            Enc::Base64 => lua.create_string(STANDARD.encode(bytes)),
            Enc::Base64Url => lua.create_string(URL_SAFE_NO_PAD.encode(bytes)),
            Enc::Raw => lua.create_string(bytes),
        }
    }
}

fn digest_alg(name: &str) -> Option<&'static digest::Algorithm> {
    Some(match name {
        "sha1" => &digest::SHA1_FOR_LEGACY_USE_ONLY,
        "sha256" => &digest::SHA256,
        "sha384" => &digest::SHA384,
        "sha512" => &digest::SHA512,
        _ => return None,
    })
}

fn hmac_alg(name: &str) -> Option<hmac::Algorithm> {
    Some(match name {
        "sha1" => hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY,
        "sha256" => hmac::HMAC_SHA256,
        "sha384" => hmac::HMAC_SHA384,
        "sha512" => hmac::HMAC_SHA512,
        _ => return None,
    })
}

fn random(n: usize) -> mlua::Result<Vec<u8>> {
    let mut out = vec![0u8; n];
    SystemRandom::new()
        .fill(&mut out)
        .map_err(|_| fail("crypto: the system's random source failed".into()))?;
    Ok(out)
}

/// Whether `a` and `b` are the same bytes, in a time that depends on their
/// length alone: a MAC checked this way does not say how much of it was
/// right.
fn equal(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let diff = a.iter().zip(b).fold(0u8, |d, (x, y)| d | (x ^ y));
    std::hint::black_box(diff) == 0
}

/// The cipher `opts.alg` names (AES-256-GCM by default) and `opts.aad`,
/// the data authenticated beside it and not hidden.
fn aead_opts(opts: Option<Table>) -> mlua::Result<(&'static aead::Algorithm, Vec<u8>)> {
    let mut alg = &aead::AES_256_GCM;
    let mut aad = Vec::new();
    if let Some(o) = opts {
        if let Some(name) = o.get::<Option<String>>("alg")? {
            alg = match name.as_str() {
                "aes-256-gcm" => &aead::AES_256_GCM,
                "chacha20-poly1305" => &aead::CHACHA20_POLY1305,
                _ => {
                    return Err(fail_as(
                        "invalid",
                        format!(
                            "crypto: no cipher named {name:?}; aes-256-gcm or chacha20-poly1305"
                        ),
                    ));
                }
            };
        }
        if let Some(a) = o.get::<Option<LuaString>>("aad")? {
            aad = a.as_bytes().to_vec();
        }
    }
    Ok((alg, aad))
}

fn aead_key(alg: &'static aead::Algorithm, key: &[u8]) -> mlua::Result<aead::LessSafeKey> {
    if key.len() != alg.key_len() {
        return Err(fail_as(
            "invalid",
            format!(
                "crypto: the key must be {} bytes (kui.crypto.random({}), or one derived from a password); this one is {}",
                alg.key_len(),
                alg.key_len(),
                key.len()
            ),
        ));
    }
    let key =
        aead::UnboundKey::new(alg, key).map_err(|_| fail_as("invalid", "crypto: a bad key"))?;
    Ok(aead::LessSafeKey::new(key))
}

fn key_length(n: usize) -> mlua::Result<()> {
    if (4..=1024).contains(&n) {
        Ok(())
    } else {
        Err(fail_as(
            "invalid",
            format!("crypto: a key of {n} bytes; 4 to 1024 are made"),
        ))
    }
}

/// Argon2id with `opts`' costs, OWASP's floor by default (19 MiB, two
/// passes, one lane), and the key length asked (32 bytes).
fn argon2_of(opts: Option<&Table>) -> mlua::Result<(argon2::Argon2<'static>, usize)> {
    let d = argon2::Params::DEFAULT;
    let (mut m, mut t, mut p, mut len) = (d.m_cost(), d.t_cost(), d.p_cost(), 32usize);
    if let Some(o) = opts {
        if let Some(v) = o.get::<Option<u32>>("memory")? {
            m = v;
        }
        if let Some(v) = o.get::<Option<u32>>("iterations")? {
            t = v;
        }
        if let Some(v) = o.get::<Option<u32>>("parallelism")? {
            p = v;
        }
        if let Some(v) = o.get::<Option<usize>>("length")? {
            len = v;
        }
    }
    let params = argon2::Params::new(m, t, p, Some(len))
        .map_err(|e| fail_as("invalid", format!("crypto: argon2: {e}")))?;
    Ok((
        argon2::Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params),
        len,
    ))
}

fn to_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(DIGITS[(b >> 4) as usize] as char);
        s.push(DIGITS[(b & 15) as usize] as char);
    }
    s
}

fn from_hex(text: &[u8]) -> Result<Vec<u8>, String> {
    let digits: Vec<u8> = text
        .iter()
        .copied()
        .filter(|b| !b.is_ascii_whitespace())
        .collect();
    if !digits.len().is_multiple_of(2) {
        return Err(format!("hex: an odd number of digits ({})", digits.len()));
    }
    let nibble = |c: u8| -> Result<u8, String> {
        match c {
            b'0'..=b'9' => Ok(c - b'0'),
            b'a'..=b'f' => Ok(c - b'a' + 10),
            b'A'..=b'F' => Ok(c - b'A' + 10),
            _ => Err(format!("hex: {:?} is not a hex digit", c as char)),
        }
    };
    digits
        .chunks(2)
        .map(|p| Ok(nibble(p[0])? << 4 | nibble(p[1])?))
        .collect()
}

/// Base64 in either alphabet, padded or not, its line breaks and spaces
/// passed over: what a PEM block, a JWT's part or a header holds.
fn b64_decode(text: &[u8]) -> Result<Vec<u8>, String> {
    let clean: Vec<u8> = text
        .iter()
        .copied()
        .filter(|b| !b.is_ascii_whitespace())
        .collect();
    let body = clean
        .strip_suffix(b"==")
        .or_else(|| clean.strip_suffix(b"="))
        .unwrap_or(&clean);
    let url = body.iter().any(|b| *b == b'-' || *b == b'_');
    let engine = if url {
        URL_SAFE_NO_PAD
    } else {
        STANDARD_NO_PAD
    };
    engine.decode(body).map_err(|e| format!("base64: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_reads_either_alphabet_padded_or_not() {
        for text in ["+/8=", "+/8", "-_8", "-_8=", " +/\n8= "] {
            assert_eq!(b64_decode(text.as_bytes()).unwrap(), [0xfb, 0xff], "{text}");
        }
        assert!(b64_decode(b"a").is_err());
    }

    #[test]
    fn hex_round_trips_and_says_what_is_wrong() {
        assert_eq!(to_hex(&[0, 0xab, 0xff]), "00abff");
        assert_eq!(from_hex(b"00ABff").unwrap(), [0, 0xab, 0xff]);
        assert!(from_hex(b"abc").unwrap_err().contains("odd"));
        assert!(from_hex(b"zz").unwrap_err().contains("'z'"));
    }

    #[test]
    fn equal_compares_bytes() {
        assert!(equal(b"abc", b"abc"));
        assert!(!equal(b"abc", b"abd"));
        assert!(!equal(b"abc", b"ab"));
    }
}
