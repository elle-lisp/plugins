//! Connection options and rustls config construction.
//!
//! Elle passes options as a struct. This module turns that struct into a
//! `ClientConfig` or a `ServerConfig`, and reports what went wrong in terms
//! elle understands.

use crate::api;
use elle_plugin::{ElleCtx, ElleResult, ElleValue};
use rustls::server::WebPkiClientVerifier;
use rustls::{ClientConfig, RootCertStore, ServerConfig};
use rustls_native_certs::load_native_certs;
use rustls_pki_types::pem::PemObject;
use rustls_pki_types::{CertificateDer, PrivateKeyDer};
use std::fmt::Display;
use std::io::Cursor;
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// A configuration failure carrying the elle error kind to raise.
///
/// Reading a file and parsing what it contains fail for different reasons —
/// a missing path is `:io-error`, a malformed certificate is `:tls-error` —
/// and callers several frames up would otherwise have to guess which.
pub struct ConfigError {
    kind: &'static str,
    message: String,
}

impl ConfigError {
    pub fn io(message: impl Display) -> Self {
        ConfigError { kind: "io-error", message: message.to_string() }
    }

    pub fn tls(message: impl Display) -> Self {
        ConfigError { kind: "tls-error", message: message.to_string() }
    }

    pub fn wrong_type(message: impl Display) -> Self {
        ConfigError { kind: "type-error", message: message.to_string() }
    }

    pub fn value(message: impl Display) -> Self {
        ConfigError { kind: "value-error", message: message.to_string() }
    }

    /// Render as a primitive result, prefixed with the primitive's name so
    /// the message reads the same way as every other error this plugin raises.
    pub fn into_result(self, ctx: *mut ElleCtx, prim: &str) -> ElleResult {
        api().err(ctx, self.kind, &format!("{}: {}", prim, self.message))
    }
}

type Result<T> = std::result::Result<T, ConfigError>;

// ---------------------------------------------------------------------------
// PEM files
// ---------------------------------------------------------------------------

/// Read a PEM certificate chain. `what` names the option in error messages.
fn read_certs(path: &str, what: &str) -> Result<Vec<CertificateDer<'static>>> {
    let data = std::fs::read(path)
        .map_err(|e| ConfigError::io(format!("reading {} '{}': {}", what, path, e)))?;
    let mut reader = Cursor::new(&data);
    let certs: Vec<CertificateDer<'static>> = CertificateDer::pem_reader_iter(&mut reader)
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| ConfigError::tls(format!("{} parse error: {}", what, e)))?;
    if certs.is_empty() {
        return Err(ConfigError::tls(format!(
            "no certificates found in {} '{}'",
            what, path
        )));
    }
    Ok(certs)
}

/// Read a PEM private key. `what` names the option in error messages.
pub fn read_key(path: &str, what: &str) -> Result<PrivateKeyDer<'static>> {
    let data = std::fs::read(path)
        .map_err(|e| ConfigError::io(format!("reading {} '{}': {}", what, path, e)))?;
    let mut reader = Cursor::new(&data);
    PrivateKeyDer::from_pem_reader(&mut reader)
        .map_err(|e| ConfigError::tls(format!("{} parse error in '{}': {}", what, path, e)))
}

/// Read a PEM bundle into a trust store.
fn read_roots(path: &str, what: &str) -> Result<RootCertStore> {
    let mut store = RootCertStore::empty();
    for cert in read_certs(path, what)? {
        store
            .add(cert)
            .map_err(|e| ConfigError::tls(format!("{} cert error: {}", what, e)))?;
    }
    Ok(store)
}

/// The platform trust store, falling back to the compiled-in web roots when
/// the platform has none to offer.
fn native_roots() -> Result<RootCertStore> {
    let mut store = RootCertStore::empty();
    let loaded = load_native_certs().certs;
    if loaded.is_empty() {
        store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    } else {
        for cert in loaded {
            store
                .add(cert)
                .map_err(|e| ConfigError::tls(format!("native cert error: {}", e)))?;
        }
    }
    Ok(store)
}

// ---------------------------------------------------------------------------
// Option structs
// ---------------------------------------------------------------------------

/// The protocol list this plugin has always offered. Kept as the client
/// default so callers that never heard of ALPN keep negotiating HTTP/1.1.
const DEFAULT_ALPN: &str = "http/1.1";

/// Read the optional argument at `idx`, or nil when the caller omitted it.
///
/// # Safety
/// `args` must point to a valid array of at least `nargs` elements.
unsafe fn optional(args: *const ElleValue, nargs: usize, idx: usize) -> ElleValue {
    let a = api();
    if nargs > idx {
        unsafe { a.arg(args, nargs, idx) }
    } else {
        a.nil()
    }
}

/// Read a string-valued option, or None when absent.
fn opt_string(opts: ElleValue, key: &str) -> Option<String> {
    api().get_string(api().get_struct_field(opts, key)).map(|s| s.to_string())
}

/// Read an ALPN protocol list.
///
/// An absent key means `default`; an explicit empty array means send no ALPN
/// extension at all, which is not the same thing.
fn opt_alpn(opts: ElleValue, key: &str, default: &[&str]) -> Result<Vec<Vec<u8>>> {
    let a = api();
    let field = a.get_struct_field(opts, key);
    if a.check_nil(field) {
        return Ok(default.iter().map(|p| p.as_bytes().to_vec()).collect());
    }
    let len = a.get_array_len(field).ok_or_else(|| {
        ConfigError::wrong_type(format!(
            ":{} must be an array of strings, got {}",
            key,
            a.type_name(field)
        ))
    })?;
    let mut out = Vec::with_capacity(len);
    for i in 0..len {
        let item = a.get_array_item(field, i);
        let name = a.get_string(item).ok_or_else(|| {
            ConfigError::wrong_type(format!(
                ":{} entries must be strings, got {}",
                key,
                a.type_name(item)
            ))
        })?;
        if name.is_empty() {
            return Err(ConfigError::value(format!(":{} entries must not be empty", key)));
        }
        out.push(name.as_bytes().to_vec());
    }
    Ok(out)
}

/// Options for `tls/client-state`.
pub struct ClientOpts {
    no_verify: bool,
    ca_file: Option<String>,
    client_auth: Option<(String, String)>,
    alpn: Vec<Vec<u8>>,
}

impl ClientOpts {
    /// Parse the optional options struct at argument `idx`.
    ///
    /// # Safety
    /// `args` must point to a valid array of at least `nargs` elements.
    pub unsafe fn parse(args: *const ElleValue, nargs: usize, idx: usize) -> Result<Self> {
        let a = api();
        let opts = unsafe { optional(args, nargs, idx) };
        let cert = opt_string(opts, "client-cert");
        let key = opt_string(opts, "client-key");
        // Half a client identity is never what the caller meant, and the
        // handshake failure it would cause names neither option.
        let client_auth = match (cert, key) {
            (Some(c), Some(k)) => Some((c, k)),
            (Some(_), None) => {
                return Err(ConfigError::value(":client-cert requires :client-key"))
            }
            (None, Some(_)) => {
                return Err(ConfigError::value(":client-key requires :client-cert"))
            }
            (None, None) => None,
        };
        Ok(ClientOpts {
            no_verify: a.get_bool(a.get_struct_field(opts, "no-verify")).unwrap_or(false),
            ca_file: opt_string(opts, "ca-file"),
            client_auth,
            alpn: opt_alpn(opts, "alpn", &[DEFAULT_ALPN])?,
        })
    }

    /// Build the rustls client config these options describe.
    pub fn build(self) -> Result<Arc<ClientConfig>> {
        let builder = if self.no_verify {
            ClientConfig::builder()
                .dangerous()
                .with_custom_certificate_verifier(Arc::new(crate::verify::NoVerifier))
        } else {
            let roots = match &self.ca_file {
                Some(path) => read_roots(path, "ca-file")?,
                None => native_roots()?,
            };
            ClientConfig::builder().with_root_certificates(roots)
        };
        let mut config = match &self.client_auth {
            Some((cert_path, key_path)) => {
                let chain = read_certs(cert_path, "client-cert")?;
                let key = read_key(key_path, "client-key")?;
                builder
                    .with_client_auth_cert(chain, key)
                    .map_err(|e| ConfigError::tls(format!("client cert error: {}", e)))?
            }
            None => builder.with_no_client_auth(),
        };
        config.alpn_protocols = self.alpn;
        Ok(Arc::new(config))
    }
}

/// Options for `tls/server-config`.
pub struct ServerOpts {
    client_ca: Option<String>,
    alpn: Vec<Vec<u8>>,
}

impl ServerOpts {
    /// Parse the optional options struct at argument `idx`.
    ///
    /// # Safety
    /// `args` must point to a valid array of at least `nargs` elements.
    pub unsafe fn parse(args: *const ElleValue, nargs: usize, idx: usize) -> Result<Self> {
        let opts = unsafe { optional(args, nargs, idx) };
        Ok(ServerOpts {
            client_ca: opt_string(opts, "client-ca"),
            // A server that offers nothing accepts whatever the client asks
            // for, which is what every caller before ALPN expected.
            alpn: opt_alpn(opts, "alpn", &[])?,
        })
    }

    /// Build the rustls server config these options describe, serving
    /// `cert_path` and `key_path`.
    pub fn build(self, cert_path: &str, key_path: &str) -> Result<Arc<ServerConfig>> {
        let chain = read_certs(cert_path, "cert-path")?;
        let key = read_key(key_path, "key-path")?;
        let builder = match &self.client_ca {
            Some(path) => {
                let roots = read_roots(path, "client-ca")?;
                let verifier = WebPkiClientVerifier::builder(Arc::new(roots))
                    .build()
                    .map_err(|e| ConfigError::tls(format!("client-ca error: {}", e)))?;
                ServerConfig::builder().with_client_cert_verifier(verifier)
            }
            None => ServerConfig::builder().with_no_client_auth(),
        };
        let mut config = builder
            .with_single_cert(chain, key)
            .map_err(|e| ConfigError::tls(format!("server config error: {}", e)))?;
        config.alpn_protocols = self.alpn;
        Ok(Arc::new(config))
    }
}
