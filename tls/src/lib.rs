//! Elle TLS plugin — TLS state machine primitives via rustls.
//!
//! This plugin exposes rustls's UnbufferedClientConnection /
//! UnbufferedServerConnection as pure state machine primitives.
//! All socket I/O is performed in Elle code using port/read and
//! port/write on native TCP ports. No I/O happens in this plugin.

use elle_plugin::{ElleCtx, ElleResult, ElleValue, EllePrimDef, SIG_OK, SIG_ERROR};
use rustls::client::UnbufferedClientConnection;
use rustls::server::UnbufferedServerConnection;
use rustls::unbuffered::{
    ConnectionState, EncodeTlsData, EncryptError, ReadTraffic, UnbufferedStatus, WriteTraffic,
};
use std::cell::{Cell, RefCell};
use std::sync::Arc;

mod config;
mod verify;

use config::{ClientOpts, ServerOpts};

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

impl TlsState {
    /// Wrap a fresh connection. All three buffers start empty and the
    /// handshake starts incomplete, whichever side this is.
    fn new(conn: TlsConnection) -> Self {
        TlsState {
            conn: RefCell::new(conn),
            incoming: RefCell::new(Vec::new()),
            outgoing: RefCell::new(Vec::new()),
            plaintext: RefCell::new(Vec::new()),
            handshake_complete: Cell::new(false),
            close_notify_pending: Cell::new(false),
        }
    }

    /// The protocol agreed via ALPN, or None before the handshake settles
    /// and when no protocol was agreed.
    fn alpn_protocol(&self) -> Option<Vec<u8>> {
        match &*self.conn.borrow() {
            TlsConnection::Client(c) => c.alpn_protocol().map(|p| p.to_vec()),
            TlsConnection::Server(s) => s.alpn_protocol().map(|p| p.to_vec()),
        }
    }
}

pub struct TlsServerConfig {
    config: Arc<rustls::ServerConfig>,
}

// ---------------------------------------------------------------------------
// Plugin entry point
// ---------------------------------------------------------------------------

elle_plugin::define_plugin!("tls/", &PRIMITIVES);

// rustls needs a process-global crypto provider before any connection or
// config is built. define_plugin! generates elle_plugin_init and offers no
// hook for custom init code, so install the provider on first use instead.
// Every call after the first returns Err, which is what we want to ignore.
fn ensure_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

fn get_tls_state<'a>(ctx: *mut ElleCtx, args: *const ElleValue, nargs: usize, idx: usize, name: &str) -> Result<&'a TlsState, ElleResult> {
    let a = api();
    let v = unsafe { a.arg(args, nargs, idx) };
    a.get_external::<TlsState>(v, "tls-state").ok_or_else(|| {
        a.err(ctx, "type-error", &format!("{}: expected tls-state, got {}", name, a.type_name(v)))
    })
}

fn tls_err(ctx: *mut ElleCtx, name: &str, msg: impl std::fmt::Display) -> ElleResult {
    api().err(ctx, "tls-error", &format!("{}: {}", name, msg))
}

/// Drop the bytes rustls consumed from the head of the incoming buffer.
fn discard_consumed(incoming: &mut Vec<u8>, discard: usize) {
    if discard > 0 {
        incoming.drain(..discard);
    }
}

/// A handshake record never exceeds 16 KiB plus its header and tag.
const RECORD_BUFFER: usize = 16_640;

/// Append one handshake record to the outgoing buffer.
fn encode_record<D>(encode: &mut EncodeTlsData<'_, D>, outgoing: &mut Vec<u8>) -> Result<(), String> {
    let start = outgoing.len();
    outgoing.resize(start + RECORD_BUFFER, 0u8);
    match encode.encode(&mut outgoing[start..]) {
        Ok(written) => {
            outgoing.truncate(start + written);
            Ok(())
        }
        Err(e) => {
            outgoing.truncate(start);
            Err(format!("encode error: {}", e))
        }
    }
}

/// Append the ciphertext for `data` to the outgoing buffer.
///
/// rustls splits the plaintext across 16 KiB records and each record pays its
/// own header and AEAD tag, so the ciphertext grows with the RECORD COUNT. A
/// flat slack would cover only the first handful of records and cap the
/// payload, so ask rustls for the size it wants and retry at that size.
fn encrypt_records<D>(wt: &mut WriteTraffic<'_, D>, data: &[u8], outgoing: &mut Vec<u8>) -> Result<(), String> {
    let start = outgoing.len();
    outgoing.resize(start + data.len() + 256, 0u8);
    match wt.encrypt(data, &mut outgoing[start..]) {
        Ok(written) => {
            outgoing.truncate(start + written);
            Ok(())
        }
        Err(EncryptError::InsufficientSize(need)) => {
            outgoing.resize(start + need.required_size, 0u8);
            match wt.encrypt(data, &mut outgoing[start..]) {
                Ok(written) => {
                    outgoing.truncate(start + written);
                    Ok(())
                }
                // A second InsufficientSize would mean the size rustls asked
                // for is still short; report, no loop.
                Err(e) => {
                    outgoing.truncate(start);
                    Err(format!("encrypt error: {}", e))
                }
            }
        }
        Err(e) => {
            outgoing.truncate(start);
            Err(format!("encrypt error: {}", e))
        }
    }
}

/// Drain every decrypted record into the plaintext buffer.
fn take_records<D>(rt: &mut ReadTraffic<'_, '_, D>, plaintext: &mut Vec<u8>) -> Result<(), String> {
    while let Some(record) = rt.next_record() {
        match record {
            Ok(app_data) => plaintext.extend_from_slice(app_data.payload),
            Err(e) => return Err(format!("read_traffic error: {}", e)),
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Drive loop
// ---------------------------------------------------------------------------

macro_rules! handle_conn_state {
    ($ctx:expr, $conn_state:expr, $outgoing:expr, $plaintext:expr, $handshake_done:expr, $state:expr) => {{
        match $conn_state {
            ConnectionState::EncodeTlsData(mut encode) => {
                if let Err(e) = encode_record(&mut encode, &mut $outgoing) {
                    return Err(tls_err($ctx, "tls/process", e));
                }
                None
            }
            ConnectionState::TransmitTlsData(transmit) => { transmit.done(); None }
            ConnectionState::BlockedHandshake => Some("handshaking"),
            ConnectionState::ReadTraffic(mut read_traffic) => {
                if let Err(e) = take_records(&mut read_traffic, &mut $plaintext) {
                    return Err(tls_err($ctx, "tls/process", e));
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

fn drive_state_machine(ctx: *mut ElleCtx, state: &TlsState, new_data: &[u8]) -> Result<&'static str, ElleResult> {
    state.incoming.borrow_mut().extend_from_slice(new_data);
    let mut conn = state.conn.borrow_mut();
    let mut incoming = state.incoming.borrow_mut();
    let mut outgoing = state.outgoing.borrow_mut();
    let mut plaintext = state.plaintext.borrow_mut();

    let mut last_kw = "ready";

    loop {
        macro_rules! one_round {
            ($raw_conn:expr) => {{
                let UnbufferedStatus { discard, state: cs } = $raw_conn.process_tls_records(&mut incoming);
                let status = match cs {
                    Err(e) => { discard_consumed(&mut incoming, discard); return Err(tls_err(ctx, "tls/process", e)); }
                    Ok(conn_state) => {
                        let r = handle_conn_state!(ctx, conn_state, outgoing, plaintext, state.handshake_complete, state);
                        discard_consumed(&mut incoming, discard);
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
        match status {
            Some("has-data") => {
                last_kw = "has-data";
                // Rustls consumed our incoming bytes but may have buffered
                // multiple records internally. Loop to drain them all.
                continue;
            }
            // After draining all records, rustls returns to WriteTraffic
            // ("ready"). If we previously got data, return "has-data".
            Some("ready") if last_kw == "has-data" => return Ok("has-data"),
            Some(kw) => return Ok(kw),
            None => continue,
        }
    }
}

// ---------------------------------------------------------------------------
// Primitive implementations
// ---------------------------------------------------------------------------

extern "C" fn prim_tls_client_state(ctx: *mut ElleCtx, args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    ensure_provider();
    let name = "tls/client-state";
    let v0 = unsafe { a.arg(args, nargs, 0) };
    let hostname = match a.get_string(v0) {
        Some(s) if !s.is_empty() => s.to_string(),
        Some(_) => return tls_err(ctx, name, "hostname must not be empty"),
        None => return a.err(ctx, "type-error", &format!("{}: expected string for hostname, got {}", name, a.type_name(v0))),
    };

    let opts = match unsafe { ClientOpts::parse(args, nargs, 1) } {
        Ok(o) => o, Err(e) => return e.into_result(ctx, name),
    };
    let config = match opts.build() {
        Ok(c) => c, Err(e) => return e.into_result(ctx, name),
    };
    let server_name = match rustls::pki_types::ServerName::try_from(hostname.as_str()) {
        Ok(n) => n.to_owned(), Err(e) => return tls_err(ctx, name, format!("invalid hostname: {}", e)),
    };
    let conn = match UnbufferedClientConnection::new(config, server_name) {
        Ok(c) => c, Err(e) => return tls_err(ctx, name, e),
    };
    a.ok(a.external(ctx, "tls-state", TlsState::new(TlsConnection::Client(conn))))
}

extern "C" fn prim_tls_process(ctx: *mut ElleCtx, args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let name = "tls/process";
    let state = match get_tls_state(ctx, args, nargs, 0, name) { Ok(s) => s, Err(e) => return e };
    let v1 = unsafe { a.arg(args, nargs, 1) };
    let new_data = match a.get_bytes(v1) {
        Some(b) => b.to_vec(),
        None => return a.err(ctx, "type-error", &format!("{}: expected bytes, got {}", name, a.type_name(v1))),
    };
    match drive_state_machine(ctx, state, &new_data) {
        Ok(kw) => a.ok(a.keyword(kw)),
        Err(e) => e,
    }
}

extern "C" fn prim_tls_get_outgoing(ctx: *mut ElleCtx, args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let state = match get_tls_state(ctx, args, nargs, 0, "tls/get-outgoing") { Ok(s) => s, Err(e) => return e };
    let drained: Vec<u8> = std::mem::take(&mut *state.outgoing.borrow_mut());
    a.ok(a.bytes(ctx, &drained))
}

extern "C" fn prim_tls_get_plaintext(ctx: *mut ElleCtx, args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let state = match get_tls_state(ctx, args, nargs, 0, "tls/get-plaintext") { Ok(s) => s, Err(e) => return e };
    let drained: Vec<u8> = std::mem::take(&mut *state.plaintext.borrow_mut());
    a.ok(a.bytes(ctx, &drained))
}

extern "C" fn prim_tls_read_plaintext(ctx: *mut ElleCtx, args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let name = "tls/read-plaintext";
    let state = match get_tls_state(ctx, args, nargs, 0, name) { Ok(s) => s, Err(e) => return e };
    let v1 = unsafe { a.arg(args, nargs, 1) };
    let n = match a.get_int(v1) {
        Some(i) if i >= 0 => i as usize,
        Some(_) => return a.err(ctx, "value-error", &format!("{}: n must be non-negative", name)),
        None => return a.err(ctx, "type-error", &format!("{}: expected int for n, got {}", name, a.type_name(v1))),
    };
    let mut buf = state.plaintext.borrow_mut();
    let take = n.min(buf.len());
    let drained: Vec<u8> = buf.drain(..take).collect();
    a.ok(a.bytes(ctx, &drained))
}

extern "C" fn prim_tls_plaintext_indexof(ctx: *mut ElleCtx, args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let name = "tls/plaintext-indexof";
    let state = match get_tls_state(ctx, args, nargs, 0, name) { Ok(s) => s, Err(e) => return e };
    let v1 = unsafe { a.arg(args, nargs, 1) };
    let byte_val = match a.get_int(v1) {
        Some(i) if (0..=255).contains(&i) => i as u8,
        Some(_) => return a.err(ctx, "value-error", &format!("{}: byte must be 0-255", name)),
        None => return a.err(ctx, "type-error", &format!("{}: expected int for byte, got {}", name, a.type_name(v1))),
    };
    let buf = state.plaintext.borrow();
    match buf.iter().position(|&b| b == byte_val) {
        Some(idx) => a.ok(a.int(idx as i64)),
        None => a.ok(a.nil()),
    }
}

extern "C" fn prim_tls_handshake_complete(ctx: *mut ElleCtx, args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let state = match get_tls_state(ctx, args, nargs, 0, "tls/handshake-complete?") { Ok(s) => s, Err(e) => return e };
    a.ok(a.boolean(state.handshake_complete.get()))
}

extern "C" fn prim_tls_alpn_protocol(ctx: *mut ElleCtx, args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let name = "tls/alpn-protocol";
    let state = match get_tls_state(ctx, args, nargs, 0, name) { Ok(s) => s, Err(e) => return e };
    match state.alpn_protocol() {
        None => a.ok(a.nil()),
        // RFC 7301 protocol IDs are opaque byte strings, but elle strings are
        // UTF-8. Every registered protocol is ASCII, so anything else means
        // the peer selected something this API cannot name — say so rather
        // than report it as "no protocol agreed".
        Some(p) => match std::str::from_utf8(&p) {
            Ok(s) => a.ok(a.string(ctx, s)),
            Err(_) => tls_err(ctx, name, "peer selected a non-UTF-8 protocol name"),
        },
    }
}

extern "C" fn prim_tls_close_notify(ctx: *mut ElleCtx, args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let name = "tls/close-notify";
    let state = match get_tls_state(ctx, args, nargs, 0, name) { Ok(s) => s, Err(e) => return e };
    state.close_notify_pending.set(true);
    if let Err(e) = drive_state_machine(ctx, state, &[]) { return e; }
    let outgoing: Vec<u8> = std::mem::take(&mut *state.outgoing.borrow_mut());
    a.ok(a.build_struct(ctx, &[("outgoing", a.bytes(ctx, &outgoing))]))
}

extern "C" fn prim_tls_write_plaintext(ctx: *mut ElleCtx, args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let name = "tls/write-plaintext";
    let state = match get_tls_state(ctx, args, nargs, 0, name) { Ok(s) => s, Err(e) => return e };

    if !state.handshake_complete.get() {
        return a.ok(a.build_struct(ctx, &[
            ("status", a.keyword("error")),
            ("message", a.string(ctx, &format!("{}: handshake not complete", name))),
        ]));
    }

    let v1 = unsafe { a.arg(args, nargs, 1) };
    let data: Vec<u8> = if let Some(b) = a.get_bytes(v1) {
        b.to_vec()
    } else if let Some(s) = a.get_string(v1) {
        s.as_bytes().to_vec()
    } else {
        return a.err(ctx, "type-error", &format!("{}: expected bytes or string, got {}", name, a.type_name(v1)));
    };

    let mut conn = state.conn.borrow_mut();
    let mut incoming = state.incoming.borrow_mut();
    let mut outgoing = state.outgoing.borrow_mut();

    // Pump the state machine until it offers WriteTraffic, then encrypt.
    // Both connection kinds run the identical sequence and differ only in
    // the concrete type fed to process_tls_records, which no object-safe
    // rustls trait spans — hence a macro rather than a function.
    macro_rules! write_round {
        ($raw_conn:expr) => {{
            let UnbufferedStatus { discard, state: cs } = $raw_conn.process_tls_records(&mut incoming);
            match cs {
                Err(e) => { discard_consumed(&mut incoming, discard); return tls_err(ctx, name, e); }
                Ok(ConnectionState::WriteTraffic(mut wt)) => {
                    discard_consumed(&mut incoming, discard);
                    match encrypt_records(&mut wt, &data, &mut outgoing) {
                        Ok(()) => true,
                        Err(e) => return tls_err(ctx, name, e),
                    }
                }
                Ok(ConnectionState::EncodeTlsData(mut encode)) => {
                    let encoded = encode_record(&mut encode, &mut outgoing);
                    discard_consumed(&mut incoming, discard);
                    match encoded { Ok(()) => false, Err(e) => return tls_err(ctx, name, e) }
                }
                Ok(ConnectionState::TransmitTlsData(tx)) => {
                    tx.done();
                    discard_consumed(&mut incoming, discard);
                    false
                }
                Ok(ConnectionState::ReadTraffic(mut rt)) => {
                    let mut pt = state.plaintext.borrow_mut();
                    let taken = take_records(&mut rt, &mut pt);
                    drop(pt);
                    discard_consumed(&mut incoming, discard);
                    match taken { Ok(()) => false, Err(e) => return tls_err(ctx, name, e) }
                }
                Ok(other) => {
                    let msg = format!("{:?}", other);
                    drop(other);
                    discard_consumed(&mut incoming, discard);
                    return tls_err(ctx, name, format!("unexpected state for write: {}", msg));
                }
            }
        }};
    }

    loop {
        let encrypted = match &mut *conn {
            TlsConnection::Client(c) => write_round!(c),
            TlsConnection::Server(s) => write_round!(s),
        };
        if encrypted { break; }
    }

    let ciphertext: Vec<u8> = std::mem::take(&mut *outgoing);
    a.ok(a.build_struct(ctx, &[("status", a.keyword("ok")), ("outgoing", a.bytes(ctx, &ciphertext))]))
}

extern "C" fn prim_tls_server_config(ctx: *mut ElleCtx, args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    ensure_provider();
    let name = "tls/server-config";
    let v0 = unsafe { a.arg(args, nargs, 0) };
    let cert_path = match a.get_string(v0) { Some(s) => s.to_string(), None => return a.err(ctx, "type-error", &format!("{}: expected string for cert-path, got {}", name, a.type_name(v0))) };
    let v1 = unsafe { a.arg(args, nargs, 1) };
    let key_path = match a.get_string(v1) { Some(s) => s.to_string(), None => return a.err(ctx, "type-error", &format!("{}: expected string for key-path, got {}", name, a.type_name(v1))) };

    let opts = match unsafe { ServerOpts::parse(args, nargs, 2) } {
        Ok(o) => o, Err(e) => return e.into_result(ctx, name),
    };
    let config = match opts.build(&cert_path, &key_path) {
        Ok(c) => c, Err(e) => return e.into_result(ctx, name),
    };
    a.ok(a.external(ctx, "tls-server-config", TlsServerConfig { config }))
}

extern "C" fn prim_tls_server_state(ctx: *mut ElleCtx, args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    ensure_provider();
    let name = "tls/server-state";
    let v0 = unsafe { a.arg(args, nargs, 0) };
    let server_config = match a.get_external::<TlsServerConfig>(v0, "tls-server-config") {
        Some(c) => c,
        None => return a.err(ctx, "type-error", &format!("{}: expected tls-server-config, got {}", name, a.type_name(v0))),
    };
    let conn = match UnbufferedServerConnection::new(Arc::clone(&server_config.config)) {
        Ok(c) => c, Err(e) => return tls_err(ctx, name, e),
    };
    a.ok(a.external(ctx, "tls-state", TlsState::new(TlsConnection::Server(conn))))
}

// ---------------------------------------------------------------------------
// Primitive registration table
// ---------------------------------------------------------------------------

static PRIMITIVES: &[EllePrimDef] = &[
    EllePrimDef::range("tls/client-state", prim_tls_client_state, SIG_ERROR, 1, 2,
        "Create a TLS client state machine. hostname used for SNI and cert verification.\nopts: {:no-verify bool :ca-file string :client-cert string :client-key string :alpn [string]}", "tls",
        r#"(tls/client-state "example.com" {:alpn ["h2" "http/1.1"]})"#),
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
    EllePrimDef::exact("tls/alpn-protocol", prim_tls_alpn_protocol, SIG_ERROR, 1,
        "The protocol agreed via ALPN, or nil before the handshake completes\nand when no protocol was agreed.", "tls",
        r#"(tls/alpn-protocol state)"#),
    EllePrimDef::exact("tls/write-plaintext", prim_tls_write_plaintext, SIG_ERROR, 2,
        "Encrypt plaintext data. Only valid after handshake complete.\nReturns {:status :ok :outgoing bytes} or {:status :error :message string}.", "tls",
        r#"(tls/write-plaintext state (bytes "hello"))"#),
    EllePrimDef::range("tls/server-config", prim_tls_server_config, SIG_ERROR, 2, 3,
        "Build a TLS server config from PEM cert and key files.\nopts: {:alpn [string] :client-ca string}", "tls",
        r#"(tls/server-config "cert.pem" "key.pem" {:alpn ["h2"]})"#),
    EllePrimDef::exact("tls/server-state", prim_tls_server_state, SIG_ERROR, 1,
        "Create a TLS server state machine from a tls-server-config.", "tls",
        r#"(tls/server-state config)"#),
    EllePrimDef::exact("tls/close-notify", prim_tls_close_notify, SIG_ERROR, 1,
        "Queue a TLS close_notify alert and encode it.\nReturns {:outgoing bytes} to send before closing the TCP port.", "tls",
        r#"(tls/close-notify state)"#),
];
