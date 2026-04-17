//! Elle TLS plugin — TLS state machine primitives via rustls.
//!
//! This plugin exposes rustls's UnbufferedClientConnection /
//! UnbufferedServerConnection as pure state machine primitives.
//! All socket I/O is performed in Elle code using port/read and
//! port/write on native TCP ports. No I/O happens in this plugin.

use elle_plugin::{ElleResult, ElleValue, EllePrimDef, SIG_OK, SIG_ERROR};
use rustls::client::UnbufferedClientConnection;
use rustls::server::UnbufferedServerConnection;
use rustls::unbuffered::{ConnectionState, UnbufferedStatus};
use rustls::{ClientConfig, RootCertStore, ServerConfig};
use rustls_native_certs::load_native_certs;
use rustls_pki_types::pem::PemObject;
use rustls_pki_types::{CertificateDer, PrivateKeyDer};
use std::cell::{Cell, RefCell};
use std::io::Cursor;
use std::sync::Arc;

// ---------------------------------------------------------------------------
// State structs
// ---------------------------------------------------------------------------

pub enum TlsConnection {
    Client(UnbufferedClientConnection),
    Server(UnbufferedServerConnection),
}

pub struct TlsState {
    conn: RefCell<TlsConnection>,
    incoming: RefCell<Vec<u8>>,
    outgoing: RefCell<Vec<u8>>,
    plaintext: RefCell<Vec<u8>>,
    handshake_complete: Cell<bool>,
    close_notify_pending: Cell<bool>,
}

pub struct TlsServerConfig {
    config: Arc<ServerConfig>,
}

// ---------------------------------------------------------------------------
// Plugin entry point
// ---------------------------------------------------------------------------

elle_plugin::define_plugin!("tls/", &PRIMITIVES);

// We need to install the ring crypto provider at init time.
// The define_plugin! macro generates elle_plugin_init. We need a way to
// run code at init. We'll use a static initializer workaround:
// actually, the define_plugin! init function runs before any prims are called,
// but it doesn't have a hook for custom init code. We'll install the provider
// lazily on first use instead.
fn ensure_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

fn get_tls_state<'a>(args: *const ElleValue, nargs: usize, idx: usize, name: &str) -> Result<&'a TlsState, ElleResult> {
    let a = api();
    let v = a.arg(args, nargs, idx);
    a.get_external::<TlsState>(v, "tls-state").ok_or_else(|| {
        a.err("type-error", &format!("{}: expected tls-state, got {}", name, a.type_name(v)))
    })
}

fn tls_err(name: &str, msg: impl std::fmt::Display) -> ElleResult {
    api().err("tls-error", &format!("{}: {}", name, msg))
}

fn io_err(name: &str, msg: impl std::fmt::Display) -> ElleResult {
    api().err("io-error", &format!("{}: {}", name, msg))
}

fn build_client_config(no_verify: bool, ca_file: Option<&str>) -> Result<Arc<ClientConfig>, String> {
    if no_verify {
        let config = ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(NoVerifier))
            .with_no_client_auth();
        return Ok(Arc::new(config));
    }
    let mut root_store = RootCertStore::empty();
    if let Some(path) = ca_file {
        let data = std::fs::read(path).map_err(|e| format!("ca-file: {}", e))?;
        let mut reader = Cursor::new(&data);
        for cert in CertificateDer::pem_reader_iter(&mut reader) {
            let cert = cert.map_err(|e| format!("ca-file PEM error: {}", e))?;
            root_store.add(cert).map_err(|e| format!("ca-file cert error: {}", e))?;
        }
    } else {
        let native_result = load_native_certs();
        let loaded: Vec<_> = native_result.certs;
        if loaded.is_empty() {
            root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        } else {
            for cert in loaded {
                root_store.add(cert).map_err(|e| format!("native cert error: {}", e))?;
            }
        }
    }
    let config = ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();
    Ok(Arc::new(config))
}

#[derive(Debug)]
struct NoVerifier;

impl rustls::client::danger::ServerCertVerifier for NoVerifier {
    fn verify_server_cert(&self, _end_entity: &rustls::pki_types::CertificateDer<'_>, _intermediates: &[rustls::pki_types::CertificateDer<'_>], _server_name: &rustls::pki_types::ServerName<'_>, _ocsp: &[u8], _now: rustls::pki_types::UnixTime) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }
    fn verify_tls12_signature(&self, _message: &[u8], _cert: &rustls::pki_types::CertificateDer<'_>, _dss: &rustls::DigitallySignedStruct) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }
    fn verify_tls13_signature(&self, _message: &[u8], _cert: &rustls::pki_types::CertificateDer<'_>, _dss: &rustls::DigitallySignedStruct) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }
    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        rustls::crypto::ring::default_provider().signature_verification_algorithms.supported_schemes()
    }
}

// ---------------------------------------------------------------------------
// Drive loop helper
// ---------------------------------------------------------------------------

macro_rules! handle_conn_state {
    ($conn_state:expr, $outgoing:expr, $plaintext:expr, $handshake_done:expr, $state:expr) => {{
        match $conn_state {
            ConnectionState::EncodeTlsData(mut encode) => {
                let start = $outgoing.len();
                $outgoing.resize(start + 16_640, 0u8);
                let written = match encode.encode(&mut $outgoing[start..]) {
                    Ok(w) => w,
                    Err(e) => return Err(tls_err("tls/process", format!("encode error: {}", e))),
                };
                $outgoing.truncate(start + written);
                None
            }
            ConnectionState::TransmitTlsData(transmit) => { transmit.done(); None }
            ConnectionState::BlockedHandshake => Some("handshaking"),
            ConnectionState::ReadTraffic(mut read_traffic) => {
                while let Some(record) = read_traffic.next_record() {
                    match record {
                        Ok(app_data) => $plaintext.extend_from_slice(app_data.payload),
                        Err(e) => return Err(tls_err("tls/process", format!("read_traffic error: {}", e))),
                    }
                }
                Some("has-data")
            }
            ConnectionState::WriteTraffic(mut wt) => {
                $handshake_done.set(true);
                if $state.close_notify_pending.get() {
                    $state.close_notify_pending.set(false);
                    let start = $outgoing.len();
                    $outgoing.resize(start + 64, 0u8);
                    match wt.queue_close_notify(&mut $outgoing[start..]) {
                        Ok(written) => $outgoing.truncate(start + written),
                        Err(_) => $outgoing.truncate(start),
                    }
                }
                Some("ready")
            }
            ConnectionState::PeerClosed => Some("peer-closed"),
            ConnectionState::Closed => Some("closed"),
            _ => Some("handshaking"),
        }
    }};
}

fn drive_state_machine(state: &TlsState, new_data: &[u8]) -> Result<&'static str, ElleResult> {
    state.incoming.borrow_mut().extend_from_slice(new_data);
    let mut conn = state.conn.borrow_mut();
    let mut incoming = state.incoming.borrow_mut();
    let mut outgoing = state.outgoing.borrow_mut();
    let mut plaintext = state.plaintext.borrow_mut();

    loop {
        macro_rules! one_round {
            ($raw_conn:expr) => {{
                let UnbufferedStatus { discard, state: cs } = $raw_conn.process_tls_records(&mut incoming);
                let status = match cs {
                    Err(e) => { if discard > 0 { incoming.drain(..discard); } return Err(tls_err("tls/process", e)); }
                    Ok(conn_state) => {
                        let r = handle_conn_state!(conn_state, outgoing, plaintext, state.handshake_complete, state);
                        if discard > 0 { incoming.drain(..discard); }
                        r
                    }
                };
                status
            }};
        }
        let status = match &mut *conn {
            TlsConnection::Client(c) => one_round!(c),
            TlsConnection::Server(s) => one_round!(s),
        };
        if let Some(kw) = status { return Ok(kw); }
    }
}

// ---------------------------------------------------------------------------
// Primitive implementations
// ---------------------------------------------------------------------------

extern "C" fn prim_tls_client_state(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    ensure_provider();
    let name = "tls/client-state";
    let v0 = a.arg(args, nargs, 0);
    let hostname = match a.get_string(v0) {
        Some(s) if !s.is_empty() => s.to_string(),
        Some(_) => return tls_err(name, "hostname must not be empty"),
        None => return a.err("type-error", &format!("{}: expected string for hostname, got {}", name, a.type_name(v0))),
    };

    let no_verify = if nargs > 1 {
        let opts = a.arg(args, nargs, 1);
        let nv_val = a.get_struct_field(opts, "no-verify");
        a.get_bool(nv_val).unwrap_or(false)
    } else { false };

    let ca_file: Option<String> = if nargs > 1 {
        let opts = a.arg(args, nargs, 1);
        let cf_val = a.get_struct_field(opts, "ca-file");
        a.get_string(cf_val).map(|s| s.to_string())
    } else { None };

    let config = match build_client_config(no_verify, ca_file.as_deref()) {
        Ok(c) => c, Err(e) => return tls_err(name, e),
    };
    let server_name = match rustls::pki_types::ServerName::try_from(hostname.as_str()) {
        Ok(n) => n.to_owned(), Err(e) => return tls_err(name, format!("invalid hostname: {}", e)),
    };
    let conn = match UnbufferedClientConnection::new(config, server_name) {
        Ok(c) => c, Err(e) => return tls_err(name, e),
    };
    let state = TlsState {
        conn: RefCell::new(TlsConnection::Client(conn)),
        incoming: RefCell::new(Vec::new()),
        outgoing: RefCell::new(Vec::new()),
        plaintext: RefCell::new(Vec::new()),
        handshake_complete: Cell::new(false),
        close_notify_pending: Cell::new(false),
    };
    a.ok(a.external("tls-state", state))
}

extern "C" fn prim_tls_process(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let name = "tls/process";
    let state = match get_tls_state(args, nargs, 0, name) { Ok(s) => s, Err(e) => return e };
    let v1 = a.arg(args, nargs, 1);
    let new_data = match a.get_bytes(v1) {
        Some(b) => b.to_vec(),
        None => return a.err("type-error", &format!("{}: expected bytes, got {}", name, a.type_name(v1))),
    };
    match drive_state_machine(state, &new_data) {
        Ok(kw) => a.ok(a.keyword(kw)),
        Err(e) => e,
    }
}

extern "C" fn prim_tls_get_outgoing(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let state = match get_tls_state(args, nargs, 0, "tls/get-outgoing") { Ok(s) => s, Err(e) => return e };
    let drained: Vec<u8> = std::mem::take(&mut *state.outgoing.borrow_mut());
    a.ok(a.bytes(&drained))
}

extern "C" fn prim_tls_get_plaintext(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let state = match get_tls_state(args, nargs, 0, "tls/get-plaintext") { Ok(s) => s, Err(e) => return e };
    let drained: Vec<u8> = std::mem::take(&mut *state.plaintext.borrow_mut());
    a.ok(a.bytes(&drained))
}

extern "C" fn prim_tls_read_plaintext(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let name = "tls/read-plaintext";
    let state = match get_tls_state(args, nargs, 0, name) { Ok(s) => s, Err(e) => return e };
    let v1 = a.arg(args, nargs, 1);
    let n = match a.get_int(v1) {
        Some(i) if i >= 0 => i as usize,
        Some(_) => return a.err("value-error", &format!("{}: n must be non-negative", name)),
        None => return a.err("type-error", &format!("{}: expected int for n, got {}", name, a.type_name(v1))),
    };
    let mut buf = state.plaintext.borrow_mut();
    let take = n.min(buf.len());
    let drained: Vec<u8> = buf.drain(..take).collect();
    a.ok(a.bytes(&drained))
}

extern "C" fn prim_tls_plaintext_indexof(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let name = "tls/plaintext-indexof";
    let state = match get_tls_state(args, nargs, 0, name) { Ok(s) => s, Err(e) => return e };
    let v1 = a.arg(args, nargs, 1);
    let byte_val = match a.get_int(v1) {
        Some(i) if (0..=255).contains(&i) => i as u8,
        Some(_) => return a.err("value-error", &format!("{}: byte must be 0-255", name)),
        None => return a.err("type-error", &format!("{}: expected int for byte, got {}", name, a.type_name(v1))),
    };
    let buf = state.plaintext.borrow();
    match buf.iter().position(|&b| b == byte_val) {
        Some(idx) => a.ok(a.int(idx as i64)),
        None => a.ok(a.nil()),
    }
}

extern "C" fn prim_tls_handshake_complete(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let state = match get_tls_state(args, nargs, 0, "tls/handshake-complete?") { Ok(s) => s, Err(e) => return e };
    a.ok(a.boolean(state.handshake_complete.get()))
}

extern "C" fn prim_tls_close_notify(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let name = "tls/close-notify";
    let state = match get_tls_state(args, nargs, 0, name) { Ok(s) => s, Err(e) => return e };
    state.close_notify_pending.set(true);
    if let Err(e) = drive_state_machine(state, &[]) { return e; }
    let outgoing: Vec<u8> = std::mem::take(&mut *state.outgoing.borrow_mut());
    a.ok(a.build_struct(&[("outgoing", a.bytes(&outgoing))]))
}

extern "C" fn prim_tls_write_plaintext(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let name = "tls/write-plaintext";
    let state = match get_tls_state(args, nargs, 0, name) { Ok(s) => s, Err(e) => return e };

    if !state.handshake_complete.get() {
        return a.ok(a.build_struct(&[
            ("status", a.keyword("error")),
            ("message", a.string(&format!("{}: handshake not complete", name))),
        ]));
    }

    let v1 = a.arg(args, nargs, 1);
    let data: Vec<u8> = if let Some(b) = a.get_bytes(v1) {
        b.to_vec()
    } else if let Some(s) = a.get_string(v1) {
        s.as_bytes().to_vec()
    } else {
        return a.err("type-error", &format!("{}: expected bytes or string, got {}", name, a.type_name(v1)));
    };

    let n = data.len();
    let mut conn = state.conn.borrow_mut();
    let mut incoming = state.incoming.borrow_mut();
    let mut outgoing = state.outgoing.borrow_mut();

    loop {
        match &mut *conn {
            TlsConnection::Client(c) => {
                let UnbufferedStatus { discard, state: cs } = c.process_tls_records(&mut incoming);
                match cs {
                    Err(e) => { if discard > 0 { incoming.drain(..discard); } return tls_err(name, e); }
                    Ok(ConnectionState::WriteTraffic(mut wt)) => {
                        if discard > 0 { incoming.drain(..discard); }
                        let start = outgoing.len();
                        outgoing.resize(start + n + 256, 0u8);
                        match wt.encrypt(&data, &mut outgoing[start..]) {
                            Ok(written) => { outgoing.truncate(start + written); }
                            Err(e) => return tls_err(name, format!("encrypt error: {}", e)),
                        }
                        break;
                    }
                    Ok(ConnectionState::EncodeTlsData(mut encode)) => {
                        let start = outgoing.len();
                        outgoing.resize(start + 16_640, 0u8);
                        let w = encode.encode(&mut outgoing[start..]);
                        if discard > 0 { incoming.drain(..discard); }
                        match w { Ok(written) => { outgoing.truncate(start + written); } Err(e) => return tls_err(name, format!("encode error: {}", e)) }
                    }
                    Ok(ConnectionState::TransmitTlsData(tx)) => { tx.done(); if discard > 0 { incoming.drain(..discard); } }
                    Ok(ConnectionState::ReadTraffic(mut rt)) => {
                        let mut pt = state.plaintext.borrow_mut();
                        while let Some(rec) = rt.next_record() {
                            match rec { Ok(app) => pt.extend_from_slice(app.payload), Err(e) => { drop(pt); if discard > 0 { incoming.drain(..discard); } return tls_err(name, format!("read error: {}", e)); } }
                        }
                        drop(pt);
                        if discard > 0 { incoming.drain(..discard); }
                    }
                    Ok(other) => { let msg = format!("{:?}", other); drop(other); if discard > 0 { incoming.drain(..discard); } return tls_err(name, format!("unexpected state for write: {}", msg)); }
                }
            }
            TlsConnection::Server(s) => {
                let UnbufferedStatus { discard, state: cs } = s.process_tls_records(&mut incoming);
                match cs {
                    Err(e) => { if discard > 0 { incoming.drain(..discard); } return tls_err(name, e); }
                    Ok(ConnectionState::WriteTraffic(mut wt)) => {
                        if discard > 0 { incoming.drain(..discard); }
                        let start = outgoing.len();
                        outgoing.resize(start + n + 256, 0u8);
                        match wt.encrypt(&data, &mut outgoing[start..]) {
                            Ok(written) => { outgoing.truncate(start + written); }
                            Err(e) => return tls_err(name, format!("encrypt error: {}", e)),
                        }
                        break;
                    }
                    Ok(ConnectionState::EncodeTlsData(mut encode)) => {
                        let start = outgoing.len();
                        outgoing.resize(start + 16_640, 0u8);
                        let w = encode.encode(&mut outgoing[start..]);
                        if discard > 0 { incoming.drain(..discard); }
                        match w { Ok(written) => { outgoing.truncate(start + written); } Err(e) => return tls_err(name, format!("encode error: {}", e)) }
                    }
                    Ok(ConnectionState::TransmitTlsData(tx)) => { tx.done(); if discard > 0 { incoming.drain(..discard); } }
                    Ok(ConnectionState::ReadTraffic(mut rt)) => {
                        let mut pt = state.plaintext.borrow_mut();
                        while let Some(rec) = rt.next_record() {
                            match rec { Ok(app) => pt.extend_from_slice(app.payload), Err(e) => { drop(pt); if discard > 0 { incoming.drain(..discard); } return tls_err(name, format!("read error: {}", e)); } }
                        }
                        drop(pt);
                        if discard > 0 { incoming.drain(..discard); }
                    }
                    Ok(other) => { let msg = format!("{:?}", other); drop(other); if discard > 0 { incoming.drain(..discard); } return tls_err(name, format!("unexpected state for write: {}", msg)); }
                }
            }
        }
    }

    let encrypted: Vec<u8> = std::mem::take(&mut *outgoing);
    a.ok(a.build_struct(&[("status", a.keyword("ok")), ("outgoing", a.bytes(&encrypted))]))
}

extern "C" fn prim_tls_server_config(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    ensure_provider();
    let name = "tls/server-config";
    let v0 = a.arg(args, nargs, 0);
    let cert_path = match a.get_string(v0) { Some(s) => s.to_string(), None => return a.err("type-error", &format!("{}: expected string for cert-path, got {}", name, a.type_name(v0))) };
    let v1 = a.arg(args, nargs, 1);
    let key_path = match a.get_string(v1) { Some(s) => s.to_string(), None => return a.err("type-error", &format!("{}: expected string for key-path, got {}", name, a.type_name(v1))) };

    let cert_data = match std::fs::read(&cert_path) { Ok(d) => d, Err(e) => return io_err(name, format!("reading cert-path '{}': {}", cert_path, e)) };
    let mut cert_reader = Cursor::new(&cert_data);
    let cert_chain: Vec<CertificateDer<'static>> = match CertificateDer::pem_reader_iter(&mut cert_reader).collect::<Result<Vec<_>, _>>() {
        Ok(c) if !c.is_empty() => c,
        Ok(_) => return tls_err(name, format!("no certificates found in '{}'", cert_path)),
        Err(e) => return tls_err(name, format!("cert parse error: {}", e)),
    };
    let key_data = match std::fs::read(&key_path) { Ok(d) => d, Err(e) => return io_err(name, format!("reading key-path '{}': {}", key_path, e)) };
    let mut key_reader = Cursor::new(&key_data);
    let private_key = match PrivateKeyDer::from_pem_reader(&mut key_reader) { Ok(k) => k, Err(e) => return tls_err(name, format!("key parse error in '{}': {}", key_path, e)) };
    let config = match ServerConfig::builder().with_no_client_auth().with_single_cert(cert_chain, private_key) {
        Ok(c) => Arc::new(c), Err(e) => return tls_err(name, format!("server config error: {}", e)),
    };
    a.ok(a.external("tls-server-config", TlsServerConfig { config }))
}

extern "C" fn prim_tls_server_state(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    ensure_provider();
    let name = "tls/server-state";
    let v0 = a.arg(args, nargs, 0);
    let server_config = match a.get_external::<TlsServerConfig>(v0, "tls-server-config") {
        Some(c) => c,
        None => return a.err("type-error", &format!("{}: expected tls-server-config, got {}", name, a.type_name(v0))),
    };
    let conn = match UnbufferedServerConnection::new(Arc::clone(&server_config.config)) {
        Ok(c) => c, Err(e) => return tls_err(name, e),
    };
    let state = TlsState {
        conn: RefCell::new(TlsConnection::Server(conn)),
        incoming: RefCell::new(Vec::new()),
        outgoing: RefCell::new(Vec::new()),
        plaintext: RefCell::new(Vec::new()),
        handshake_complete: Cell::new(false),
        close_notify_pending: Cell::new(false),
    };
    a.ok(a.external("tls-state", state))
}

// ---------------------------------------------------------------------------
// Primitive registration table
// ---------------------------------------------------------------------------

static PRIMITIVES: &[EllePrimDef] = &[
    EllePrimDef::range("tls/client-state", prim_tls_client_state, SIG_ERROR, 1, 2,
        "Create a TLS client state machine. hostname used for SNI and cert verification.\nopts: {:no-verify bool :ca-file string}", "tls",
        r#"(tls/client-state "example.com")"#),
    EllePrimDef::exact("tls/process", prim_tls_process, SIG_ERROR, 2,
        "Feed ciphertext bytes into the TLS state machine.\nReturns status: :handshaking :ready :has-data :peer-closed :closed", "tls",
        r#"(tls/process state (bytes))"#),
    EllePrimDef::exact("tls/get-outgoing", prim_tls_get_outgoing, SIG_OK, 1,
        "Drain the outgoing ciphertext buffer. Returns bytes to send over the network.", "tls",
        r#"(tls/get-outgoing state)"#),
    EllePrimDef::exact("tls/get-plaintext", prim_tls_get_plaintext, SIG_OK, 1,
        "Drain the entire plaintext buffer. Returns all decrypted application data.", "tls",
        r#"(tls/get-plaintext state)"#),
    EllePrimDef::exact("tls/read-plaintext", prim_tls_read_plaintext, SIG_OK, 2,
        "Drain up to n bytes from the plaintext buffer. Remainder stays buffered.", "tls",
        r#"(tls/read-plaintext state 1024)"#),
    EllePrimDef::exact("tls/plaintext-indexof", prim_tls_plaintext_indexof, SIG_OK, 2,
        "Scan plaintext buffer for a byte value (0-255). Returns index or nil. Does not drain.", "tls",
        r#"(tls/plaintext-indexof state 10)"#),
    EllePrimDef::exact("tls/handshake-complete?", prim_tls_handshake_complete, SIG_OK, 1,
        "True if the TLS handshake is complete.", "tls",
        r#"(tls/handshake-complete? state)"#),
    EllePrimDef::exact("tls/write-plaintext", prim_tls_write_plaintext, SIG_ERROR, 2,
        "Encrypt plaintext data. Only valid after handshake complete.\nReturns {:status :ok :outgoing bytes} or {:status :error :message string}.", "tls",
        r#"(tls/write-plaintext state (bytes "hello"))"#),
    EllePrimDef::range("tls/server-config", prim_tls_server_config, SIG_ERROR, 2, 3,
        "Build a TLS server config from PEM cert and key files.", "tls",
        r#"(tls/server-config "cert.pem" "key.pem")"#),
    EllePrimDef::exact("tls/server-state", prim_tls_server_state, SIG_ERROR, 1,
        "Create a TLS server state machine from a tls-server-config.", "tls",
        r#"(tls/server-state config)"#),
    EllePrimDef::exact("tls/close-notify", prim_tls_close_notify, SIG_ERROR, 1,
        "Queue a TLS close_notify alert and encode it.\nReturns {:outgoing bytes} to send before closing the TCP port.", "tls",
        r#"(tls/close-notify state)"#),
];
