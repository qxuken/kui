//! Shared TLS trust and transport for app requests and runner downloads.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use rustls::pki_types::{CertificateDer, pem::PemObject};
use rustls::{ClientConfig, ClientConnection, RootCertStore, StreamOwned};
use ureq::unversioned::resolver::DefaultResolver;
use ureq::unversioned::transport::{
    Buffers, ConnectProxyConnector, ConnectionDetails, Connector, LazyBuffers, NextTimeout,
    TcpConnector, Transport, TransportAdapter,
};

#[derive(Clone, Debug)]
pub(crate) struct Pins(pub(crate) Vec<[u8; 32]>);

static EXTRA_PATH: OnceLock<Result<Option<PathBuf>, String>> = OnceLock::new();
static EXTRA_ROOTS: OnceLock<Result<Option<RootCertStore>, String>> = OnceLock::new();

/// Capture process configuration before loading an app's .env or starting workers.
pub(crate) fn init() {
    EXTRA_PATH.get_or_init(|| {
        std::env::var_os("KUI_EXTRA_CA_CERTS")
            .filter(|path| !path.is_empty())
            .map(std::path::absolute)
            .transpose()
            .map_err(|e| format!("KUI_EXTRA_CA_CERTS: {e}"))
    });
}

fn default_roots() -> RootCertStore {
    RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    }
}

fn client_config(roots: RootCertStore) -> ClientConfig {
    ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .with_safe_default_protocol_versions()
        .expect("TLS versions")
        .with_root_certificates(roots)
        .with_no_client_auth()
}

fn extend_roots(roots: &mut RootCertStore, path: &Path) -> Result<(), String> {
    let bytes = read_ca_file(path)?;
    extend_roots_bytes(roots, &bytes)
}

pub(crate) fn read_ca_file(path: &Path) -> Result<Vec<u8>, String> {
    use crate::modules::{coded, io_code};
    std::fs::read(path).map_err(|e| coded(io_code(&e), format!("{}: {e}", path.display())))
}

fn extend_roots_bytes(roots: &mut RootCertStore, bytes: &[u8]) -> Result<(), String> {
    use crate::modules::coded;
    let invalid = |e| coded("invalid", format!("CA bundle: {e}"));
    let mut count = 0;
    for cert in CertificateDer::pem_slice_iter(bytes) {
        let cert = cert.map_err(|e| invalid(e.to_string()))?;
        roots.add(cert).map_err(|e| invalid(e.to_string()))?;
        count += 1;
    }
    if count == 0 {
        return Err(invalid(
            "expected at least one PEM CERTIFICATE block".into(),
        ));
    }
    Ok(())
}

pub(crate) fn agent(
    config: ureq::config::Config,
    pins: Option<Pins>,
    ca_file: Option<&Path>,
) -> Result<ureq::Agent, String> {
    let ca = ca_file.map(read_ca_file).transpose()?;
    agent_with_ca(config, pins, ca.as_deref())
}

pub(crate) fn agent_with_ca(
    config: ureq::config::Config,
    pins: Option<Pins>,
    ca: Option<&[u8]>,
) -> Result<ureq::Agent, String> {
    init();
    let extra = EXTRA_ROOTS
        .get_or_init(|| {
            let path = EXTRA_PATH
                .get()
                .expect("TLS initialized")
                .as_ref()
                .map_err(|e| crate::modules::coded("tls", e))?;
            path.as_deref()
                .map(|path| {
                    let mut roots = default_roots();
                    extend_roots(&mut roots, path).map_err(|e| {
                        crate::modules::coded("tls", format!("KUI_EXTRA_CA_CERTS: {e}"))
                    })?;
                    Ok(roots)
                })
                .transpose()
        })
        .clone()?;
    if extra.is_none() && pins.is_none() && ca.is_none() {
        return Ok(config.into());
    }
    let mut roots = extra.unwrap_or_else(default_roots);
    if let Some(bytes) = ca {
        extend_roots_bytes(&mut roots, bytes)?;
    }
    Ok(agent_with_tls(config, pins, client_config(roots)))
}

impl Pins {
    pub(crate) fn verify(
        &self,
        cert: &rustls::pki_types::CertificateDer<'_>,
    ) -> Result<(), ureq::Error> {
        let digest = spki_digest(cert)?;
        if self.0.contains(&digest) {
            Ok(())
        } else {
            Err(ureq::Error::Tls(
                "server public key does not match any certificate pin",
            ))
        }
    }
}

pub(crate) fn spki_digest(
    cert: &rustls::pki_types::CertificateDer<'_>,
) -> Result<[u8; 32], rustls::Error> {
    let parsed = rustls::server::ParsedCertificate::try_from(cert)?;
    let digest = ring::digest::digest(
        &ring::digest::SHA256,
        parsed.subject_public_key_info().as_ref(),
    );
    Ok(digest.as_ref().try_into().expect("SHA-256 digest length"))
}

pub(crate) fn agent_with_tls(
    config: ureq::config::Config,
    pins: Option<Pins>,
    tls: ClientConfig,
) -> ureq::Agent {
    let connector = PinnedConnector {
        pins,
        tls: Arc::new(tls),
        proxy: config.proxy().map(|p| p.uri().clone()),
    };
    ureq::Agent::with_parts(
        config,
        ().chain(ConnectProxyConnector::default())
            .chain(TcpConnector::default())
            .chain(connector),
        DefaultResolver::default(),
    )
}

#[derive(Debug)]
struct PinnedConnector {
    pins: Option<Pins>,
    tls: Arc<ClientConfig>,
    proxy: Option<ureq::http::Uri>,
}

impl<In: Transport> Connector<In> for PinnedConnector {
    type Out = Box<dyn Transport>;

    fn connect(
        &self,
        details: &ConnectionDetails,
        chained: Option<In>,
    ) -> Result<Option<Self::Out>, ureq::Error> {
        // A CONNECT proxy has its own TLS identity; the pins belong to the
        // target inside the tunnel. ureq marks that tunnel as non-TLS.
        let is_proxy = self.proxy.as_ref() == Some(details.uri) && details.config.proxy().is_none();
        let transport = chained.ok_or(ureq::Error::ConnectionFailed)?;
        if !details.needs_tls() {
            if !is_proxy && self.pins.is_some() {
                return Err(ureq::Error::Tls("pinning requires an HTTPS connection"));
            }
            return Ok(Some(transport.boxed()));
        }
        if transport.is_tls() {
            return Err(ureq::Error::Tls("expected a fresh TLS connection"));
        }
        let host = details
            .uri
            .host()
            .ok_or(ureq::Error::Tls("missing TLS hostname"))?;
        // http::Uri retains IPv6's brackets; rustls expects the bare IP.
        let host = host.trim_start_matches('[').trim_end_matches(']');
        let name = rustls::pki_types::ServerName::try_from(host.to_owned())
            .map_err(|_| ureq::Error::Tls("invalid TLS hostname"))?;
        let mut conn = ClientConnection::new(self.tls.clone(), name)?;
        let mut sock = TransportAdapter::new(transport.boxed(), details.current_time.clone());
        let mut timeout = details.timeout;
        use ureq::unversioned::transport::time::{Duration, Instant};
        if let (Duration::Exact(limit), Instant::Exact(start), Instant::Exact(now)) =
            (timeout.after, details.now, (details.current_time)())
        {
            timeout.after =
                Duration::Exact(limit.saturating_sub(now.saturating_duration_since(start)));
        }
        sock.set_timeout(timeout);
        conn.complete_io(&mut sock).map_err(|e| {
            if let Some(tls) = e.get_ref().and_then(|e| e.downcast_ref::<rustls::Error>()) {
                ureq::Error::Rustls(tls.clone())
            } else {
                e.into()
            }
        })?;
        let cert = conn
            .peer_certificates()
            .and_then(|c| c.first())
            .ok_or(ureq::Error::Tls("server sent no certificate"))?;
        if !is_proxy && let Some(pins) = &self.pins {
            pins.verify(cert)?;
        }
        Ok(Some(Box::new(PinnedTransport {
            buffers: LazyBuffers::new(
                details.config.input_buffer_size(),
                details.config.output_buffer_size(),
            ),
            stream: StreamOwned::new(conn, sock),
        })))
    }
}

struct PinnedTransport {
    buffers: LazyBuffers,
    stream: StreamOwned<ClientConnection, TransportAdapter>,
}

impl std::fmt::Debug for PinnedTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PinnedTransport").finish_non_exhaustive()
    }
}

impl Transport for PinnedTransport {
    fn buffers(&mut self) -> &mut dyn Buffers {
        &mut self.buffers
    }

    fn transmit_output(&mut self, amount: usize, timeout: NextTimeout) -> Result<(), ureq::Error> {
        self.stream.sock.set_timeout(timeout);
        self.stream.write_all(&self.buffers.output()[..amount])?;
        Ok(())
    }

    fn await_input(&mut self, timeout: NextTimeout) -> Result<bool, ureq::Error> {
        self.stream.sock.set_timeout(timeout);
        let n = self.stream.read(self.buffers.input_append_buf())?;
        self.buffers.input_appended(n);
        Ok(n > 0)
    }

    fn is_open(&mut self) -> bool {
        self.stream.sock.get_mut().is_open()
    }
    fn is_tls(&self) -> bool {
        true
    }
}
