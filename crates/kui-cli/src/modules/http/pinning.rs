//! Decode the request's public-key pin options.

pub(super) use crate::tls::Pins;
use crate::tls::spki_digest;
use base64::{Engine, engine::general_purpose::STANDARD};

impl Pins {
    pub(super) fn from_file(path: &std::path::Path) -> Result<Self, String> {
        use rustls::pki_types::{CertificateDer, pem::PemObject};
        let bytes = std::fs::read(path).map_err(|e| {
            super::super::coded(
                super::super::io_code(&e),
                format!("http pin_file {}: {e}", path.display()),
            )
        })?;
        // PEM uses the first CERTIFICATE block; otherwise try a DER certificate.
        let cert =
            CertificateDer::from_pem_slice(&bytes).unwrap_or_else(|_| CertificateDer::from(bytes));
        let pin = spki_digest(&cert).map_err(|e| {
            super::super::coded(
                "invalid",
                format!(
                    "http pin_file {}: expected a PEM or DER certificate: {e}",
                    path.display()
                ),
            )
        })?;
        Ok(Self(vec![pin]))
    }

    pub(super) fn from_lua(value: mlua::Value) -> mlua::Result<Self> {
        let invalid = || {
            super::super::fail_as(
                "invalid",
                "http: pins must be a nonempty array of sha256/<base64 SPKI digest> strings",
            )
        };
        let mlua::Value::Table(table) = value else {
            return Err(invalid());
        };
        let count = table.clone().pairs::<mlua::Value, mlua::Value>().count();
        if count == 0 {
            return Err(invalid());
        }
        let mut pins = Vec::with_capacity(count);
        for i in 1..=count {
            let value: String = table.raw_get(i).map_err(|_| invalid())?;
            let bytes = value
                .strip_prefix("sha256/")
                .and_then(|s| STANDARD.decode(s).ok())
                .and_then(|b| b.try_into().ok())
                .ok_or_else(invalid)?;
            pins.push(bytes);
        }
        Ok(Self(pins))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tls::agent_with_tls;
    use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};
    use rustls::{ClientConfig, RootCertStore, StreamOwned};
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::Arc;
    use std::thread::JoinHandle;
    use std::time::Duration;

    struct Identity {
        ca: CertificateDer<'static>,
        cert: CertificateDer<'static>,
        key: Vec<u8>,
        pin: [u8; 32],
    }

    impl Identity {
        fn new(host: &str, expired: bool) -> Self {
            use rcgen::{
                BasicConstraints, CertificateParams, IsCa, Issuer, KeyPair, PublicKeyData,
            };
            let mut params = CertificateParams::new(Vec::<String>::new()).unwrap();
            params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
            let ca_key = KeyPair::generate().unwrap();
            let ca = params.self_signed(&ca_key).unwrap().der().clone();
            let issuer = Issuer::new(params, ca_key);
            let key = KeyPair::generate().unwrap();
            let mut params = CertificateParams::new(vec![host.to_string()]).unwrap();
            if expired {
                params.not_after = rcgen::date_time_ymd(2020, 1, 1);
            }
            let cert = params.signed_by(&key, &issuer).unwrap().der().clone();
            let pin = ring::digest::digest(&ring::digest::SHA256, &key.subject_public_key_info())
                .as_ref()
                .try_into()
                .unwrap();
            Self {
                ca,
                cert,
                pin,
                key: key.serialize_der(),
            }
        }

        fn serve(&self, response: String) -> (String, JoinHandle<Vec<u8>>) {
            let tls = rustls::ServerConfig::builder_with_provider(Arc::new(
                rustls::crypto::ring::default_provider(),
            ))
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_no_client_auth()
            .with_single_cert(
                vec![self.cert.clone()],
                PrivatePkcs8KeyDer::from(self.key.clone()).into(),
            )
            .unwrap();
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let url = format!(
                "https://localhost:{}/",
                listener.local_addr().unwrap().port()
            );
            let thread = std::thread::spawn(move || {
                let (socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                socket
                    .set_write_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let conn = rustls::ServerConnection::new(Arc::new(tls)).unwrap();
                let mut stream = StreamOwned::new(conn, socket);
                let mut request = Vec::new();
                let mut buf = [0; 1024];
                while let Ok(n) = stream.read(&mut buf) {
                    if n == 0 {
                        break;
                    }
                    request.extend_from_slice(&buf[..n]);
                    if request.windows(4).any(|w| w == b"\r\n\r\n") {
                        let _ = stream.write_all(response.as_bytes());
                        break;
                    }
                }
                request
            });
            (url, thread)
        }
    }

    const OK: &str = "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok";

    #[test]
    fn certificate_files_pin_the_same_key_in_pem_and_der() {
        let identity = Identity::new("localhost", false);
        let other = Identity::new("localhost", false);
        let dir = std::env::temp_dir().join(format!("kui-pin-files-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let pem = format!(
            "-----BEGIN CERTIFICATE-----\n{}\n-----END CERTIFICATE-----\n",
            STANDARD.encode(&identity.cert)
        );
        for (name, bytes) in [
            ("server.pem", pem.as_bytes()),
            ("server.der", identity.cert.as_ref()),
        ] {
            let path = dir.join(name);
            std::fs::write(&path, bytes).unwrap();
            let pins = Pins::from_file(&path).unwrap();
            assert_eq!(pins.0, vec![identity.pin]);
            pins.verify(&identity.cert).unwrap();
            assert!(pins.verify(&other.cert).is_err());
            let (url, server) = identity.serve(OK.into());
            assert_eq!(
                client(&[&identity], pins.0, false)
                    .get(&url)
                    .call()
                    .unwrap()
                    .status(),
                200
            );
            assert!(!server.join().unwrap().is_empty());
        }
        let invalid = dir.join("invalid.pem");
        std::fs::write(&invalid, "not a certificate").unwrap();
        assert!(
            Pins::from_file(&invalid)
                .unwrap_err()
                .starts_with("[invalid]")
        );
        assert!(
            Pins::from_file(&dir.join("missing.pem"))
                .unwrap_err()
                .starts_with("[not_found]")
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn file_pins_require_https_and_cannot_be_combined_with_digest_pins() {
        let lua = mlua::Lua::new();
        for expression in [
            "{ url = 'http://example.test', pin_file = 'cert.pem' }",
            "{ url = 'https://example.test', pin_file = '' }",
            "{ url = 'https://example.test', pin_file = 'cert.pem', pins = { 'sha256/AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=' } }",
        ] {
            let opts = lua.load(format!("return {expression}")).eval().unwrap();
            let error = super::super::request_opts(opts, "GET", None).err().unwrap();
            assert!(error.to_string().contains("[invalid]"), "{error}");
        }
    }

    fn client(roots: &[&Identity], pins: Vec<[u8; 32]>, tls12: bool) -> ureq::Agent {
        client_with_proxy(roots, pins, tls12, None)
    }

    fn client_with_proxy(
        roots: &[&Identity],
        pins: Vec<[u8; 32]>,
        tls12: bool,
        proxy: Option<ureq::Proxy>,
    ) -> ureq::Agent {
        let mut store = RootCertStore::empty();
        for identity in roots {
            store.add(identity.ca.clone()).unwrap();
        }
        let versions = if tls12 {
            vec![&rustls::version::TLS12]
        } else {
            vec![&rustls::version::TLS13]
        };
        let tls =
            ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
                .with_protocol_versions(&versions)
                .unwrap()
                .with_root_certificates(store)
                .with_no_client_auth();
        let config = ureq::Agent::config_builder()
            .proxy(proxy)
            .https_only(true)
            .timeout_global(Some(Duration::from_secs(3)))
            .build();
        agent_with_tls(config, Some(Pins(pins)), tls)
    }

    #[test]
    fn matching_backup_pin_sends_the_request_in_tls12_and_tls13() {
        let identity = Identity::new("localhost", false);
        for tls12 in [true, false] {
            let (url, server) = identity.serve(OK.into());
            let mut response = client(&[&identity], vec![[0; 32], identity.pin], tls12)
                .get(&url)
                .header("authorization", "Bearer secret")
                .call()
                .unwrap();
            assert_eq!(response.body_mut().read_to_string().unwrap(), "ok");
            let request = String::from_utf8(server.join().unwrap()).unwrap();
            assert!(request.contains("Bearer secret"));
        }
    }

    #[test]
    fn wrong_pin_sends_no_http_headers_or_body() {
        let identity = Identity::new("localhost", false);
        let (url, server) = identity.serve(OK.into());
        let error = client(&[&identity], vec![[0; 32]], false)
            .post(&url)
            .header("authorization", "Bearer secret")
            .send("private body")
            .unwrap_err();
        assert_eq!(super::super::http_code(&error), "tls");
        assert!(error.to_string().contains("does not match"));
        assert!(server.join().unwrap().is_empty());
    }

    #[test]
    fn matching_pin_preserves_ca_hostname_and_expiry_checks() {
        for (host, expired, trusted) in [
            ("localhost", false, false),
            ("elsewhere.test", false, true),
            ("localhost", true, true),
        ] {
            let identity = Identity::new(host, expired);
            let (url, server) = identity.serve(OK.into());
            let roots = if trusted { vec![&identity] } else { vec![] };
            let error = client(&roots, vec![identity.pin], false)
                .get(&url)
                .call()
                .unwrap_err();
            assert_eq!(super::super::http_code(&error), "tls", "{error}");
            assert!(server.join().unwrap().is_empty());
        }
    }

    #[test]
    fn redirects_check_the_destination_pin_before_sending_http() {
        let first = Identity::new("localhost", false);
        let second = Identity::new("localhost", false);
        let (target, target_server) = second.serve(OK.into());
        let (url, server) = first.serve(format!("HTTP/1.1 302 Found\r\nLocation: {target}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"));
        let error = client(&[&first, &second], vec![first.pin], false)
            .get(&url)
            .call()
            .unwrap_err();
        assert_eq!(super::super::http_code(&error), "tls");
        assert!(!server.join().unwrap().is_empty());
        assert!(target_server.join().unwrap().is_empty());
    }

    #[test]
    fn redirects_accept_a_destination_with_a_listed_pin() {
        let first = Identity::new("localhost", false);
        let second = Identity::new("localhost", false);
        let (target, target_server) = second.serve(OK.into());
        let (url, server) = first.serve(format!("HTTP/1.1 302 Found\r\nLocation: {target}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"));
        let response = client(&[&first, &second], vec![first.pin, second.pin], false)
            .get(&url)
            .call()
            .unwrap();
        assert_eq!(response.status(), 200);
        assert!(!server.join().unwrap().is_empty());
        assert!(!target_server.join().unwrap().is_empty());
    }

    #[test]
    fn connect_proxy_tunnels_still_enforce_the_target_pin() {
        use std::net::{Shutdown, TcpStream};
        let identity = Identity::new("localhost", false);
        for matches in [true, false] {
            let (url, server) = identity.serve(OK.into());
            let port = url.parse::<ureq::http::Uri>().unwrap().port_u16().unwrap();
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let proxy =
                ureq::Proxy::new(&format!("http://{}", listener.local_addr().unwrap())).unwrap();
            let tunnel = std::thread::spawn(move || {
                let (mut incoming, _) = listener.accept().unwrap();
                incoming
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut connect = Vec::new();
                let mut byte = [0];
                while !connect.ends_with(b"\r\n\r\n") {
                    incoming.read_exact(&mut byte).unwrap();
                    connect.push(byte[0]);
                }
                assert!(
                    String::from_utf8(connect)
                        .unwrap()
                        .starts_with(&format!("CONNECT localhost:{port} HTTP/1.1"))
                );
                let mut upstream = TcpStream::connect(("127.0.0.1", port)).unwrap();
                upstream
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                incoming
                    .write_all(b"HTTP/1.1 200 Connection established\r\n\r\n")
                    .unwrap();
                let mut from_client = incoming.try_clone().unwrap();
                let mut to_server = upstream.try_clone().unwrap();
                let upload = std::thread::spawn(move || {
                    let _ = std::io::copy(&mut from_client, &mut to_server);
                    let _ = to_server.shutdown(Shutdown::Write);
                });
                let _ = std::io::copy(&mut upstream, &mut incoming);
                let _ = incoming.shutdown(Shutdown::Both);
                upload.join().unwrap();
            });
            let pin = if matches { identity.pin } else { [0; 32] };
            let result = client_with_proxy(&[&identity], vec![pin], false, Some(proxy))
                .get(&url)
                .call();
            if matches {
                assert_eq!(result.unwrap().status(), 200);
            } else {
                assert_eq!(super::super::http_code(&result.unwrap_err()), "tls");
            }
            assert_eq!(server.join().unwrap().is_empty(), !matches);
            tunnel.join().unwrap();
        }
    }

    #[test]
    fn redirects_cannot_downgrade_to_http() {
        let identity = Identity::new("localhost", false);
        let target = TcpListener::bind("127.0.0.1:0").unwrap();
        target.set_nonblocking(true).unwrap();
        let (url, server) = identity.serve(format!("HTTP/1.1 302 Found\r\nLocation: http://{}/\r\nContent-Length: 0\r\nConnection: close\r\n\r\n", target.local_addr().unwrap()));
        let error = client(&[&identity], vec![identity.pin], false)
            .get(&url)
            .call()
            .unwrap_err();
        assert_eq!(super::super::http_code(&error), "invalid");
        assert!(!server.join().unwrap().is_empty());
        assert_eq!(
            target.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }

    #[test]
    fn lua_options_reject_invalid_or_empty_pins_and_http() {
        let lua = mlua::Lua::new();
        let pin = format!("sha256/{}", STANDARD.encode([0; 32]));
        lua.globals().set("pin", pin).unwrap();
        for expression in [
            "{}",
            "false",
            "pin",
            "{ 'sha256/bad' }",
            "{ [2] = pin }",
            "{ pin, extra = pin }",
            "{ 'sha512/' .. string.rep('A', 44) }",
        ] {
            let value = lua
                .load(format!(
                    "return {{ url = 'https://example.test', pins = {expression} }}"
                ))
                .eval()
                .unwrap();
            let err = super::super::request_opts(value, "GET", None)
                .err()
                .unwrap();
            assert!(err.to_string().contains("[invalid]"), "{expression}: {err}");
        }
        let opts = lua
            .load("return { url = 'http://example.test', pins = {pin} }")
            .eval()
            .unwrap();
        assert!(super::super::request_opts(opts, "GET", None).is_err());
        let opts = lua
            .load("return { url = 'https://example.test', pins = {pin} }")
            .eval()
            .unwrap();
        assert!(
            super::super::request_opts(opts, "GET", None)
                .unwrap()
                .client
                .pins
                .is_some()
        );
    }
}
