# elle-tls

Agent guide for the `elle-tls` plugin — TLS state machine primitives via rustls.

## Architecture

The plugin exposes rustls's `UnbufferedClientConnection` /
`UnbufferedServerConnection` as opaque `ExternalObject` values. These are
pure state machines — no I/O happens in the plugin. All socket I/O is done
in Elle code using `port/read` and `port/write` on native TCP ports.

This is the same pattern as `lib/dns.lisp` — a multi-step protocol driven
entirely in Elle, with native I/O primitives handling the network.

## Data structures

### `TlsState` (type_name: `"tls-state"`)

```rust
pub struct TlsState {
    conn: RefCell<TlsConnection>,          // Client or Server UnbufferedConnection
    incoming: RefCell<Vec<u8>>,            // ciphertext from network, not yet processed
    outgoing: RefCell<Vec<u8>>,            // ciphertext ready to send to network
    plaintext: RefCell<Vec<u8>>,           // decrypted app data, not yet consumed
    handshake_complete: Cell<bool>,
    close_notify_pending: Cell<bool>,      // set by tls/close-notify, cleared by drive loop
}
```

### `TlsServerConfig` (type_name: `"tls-server-config"`)

```rust
pub struct TlsServerConfig {
    config: Arc<rustls::ServerConfig>,
}
```

## Primitive table

| Primitive | Arity | Signal | Returns | Purpose |
|-----------|-------|--------|---------|---------|
| `tls/client-state` | 1-2 | errors | tls-state | Create client state machine |
| `tls/server-config` | 2-3 | errors | tls-server-config | Build server config from PEM files |
| `tls/server-state` | 1 | errors | tls-state | Create server state machine |
| `tls/process` | 2 | errors | keyword | Feed bytes, return status keyword |
| `tls/write-plaintext` | 2 | errors | `{:status :ok/:error :outgoing bytes}` | Encrypt plaintext after handshake |
| `tls/get-outgoing` | 1 | silent | bytes | Drain outgoing ciphertext buffer |
| `tls/get-plaintext` | 1 | silent | bytes | Drain entire plaintext buffer |
| `tls/read-plaintext` | 2 | silent | bytes | Drain up to N bytes from plaintext buffer |
| `tls/plaintext-indexof` | 2 | silent | int or nil | Scan for byte without draining |
| `tls/handshake-complete?` | 1 | silent | bool | Check handshake status |
| `tls/alpn-protocol` | 1 | errors | string or nil | Protocol agreed via ALPN |
| `tls/close-notify` | 1 | errors | `{:outgoing bytes}` | Encode close_notify alert bytes |

The "silent" primitives above still raise `:type-error` when handed
something that is not a tls-state. The column describes the signals they
raise in normal operation, not argument validation.

## Options for `tls/client-state`

The optional second argument is a struct. Unknown keys are ignored.

| Key | Type | Default | Meaning |
|-----|------|---------|---------|
| `:no-verify` | bool | false | Skip server certificate verification. Development only |
| `:ca-file` | string | — | PEM CA bundle that replaces the system roots |
| `:client-cert` | string | — | PEM client certificate chain, for mutual TLS |
| `:client-key` | string | — | PEM private key matching `:client-cert` |
| `:alpn` | array of strings | `["http/1.1"]` | Protocols to offer, most preferred first |

`:client-cert` and `:client-key` are a pair. Supplying one without the
other is a `:value-error` — a half-configured client would otherwise
fail later, during the handshake, where the cause is much harder to see.

`:alpn` defaults to `["http/1.1"]` rather than to nothing, because that
is what this plugin has always offered. Pass `[]` to send no ALPN
extension at all. Every element must be a non-empty string.

## Options for `tls/server-config`

The optional third argument is a struct. Unknown keys are ignored.

| Key | Type | Default | Meaning |
|-----|------|---------|---------|
| `:alpn` | array of strings | none | Protocols the server accepts, most preferred first |
| `:client-ca` | string | — | PEM CA bundle for verifying client certificates. Enables mutual TLS |

A server with no `:alpn` accepts no ALPN extension, so
`tls/alpn-protocol` returns nil on both sides.

Setting `:client-ca` makes the server request a client certificate and
refuse the handshake when the client does not present one that the CA
bundle verifies.

## ALPN

The client offers `:alpn` and the server picks the first entry of its
own `:alpn` that the client also offered. `tls/alpn-protocol` returns
that choice on both sides once the handshake is complete, and nil
before then or when no protocol was agreed.

When both sides configure ALPN but share no protocol, rustls fails the
handshake with a `no_application_protocol` alert. This surfaces as a
`:tls-error` from `tls/process`, not as a nil protocol.

## Status keywords from `tls/process`

| Keyword | Meaning | Action |
|---------|---------|--------|
| `:handshaking` | Need more network data | Send outgoing, read more |
| `:ready` | Handshake just completed | Send/receive app data |
| `:has-data` | App data decrypted | Drain via `tls/get-plaintext` |
| `:peer-closed` | Peer sent close_notify | Close connection |
| `:closed` | Connection fully closed | Nothing to do |

## Error table

| Condition | Error kind | Message prefix |
|-----------|-----------|----------------|
| Bad hostname for SNI | `:tls-error` | `"tls/client-state: invalid hostname: ..."` |
| Empty hostname | `:tls-error` | `"tls/client-state: hostname must not be empty"` |
| System CA load failure | `:tls-error` | `"tls/client-state: ..."` |
| `:client-cert` without `:client-key` | `:value-error` | `"tls/client-state: :client-cert requires :client-key"` |
| `:client-key` without `:client-cert` | `:value-error` | `"tls/client-state: :client-key requires :client-cert"` |
| Client cert file not found | `:io-error` | `"tls/client-state: reading client-cert '...'..."` |
| Client cert/key rejected | `:tls-error` | `"tls/client-state: client cert error: ..."` |
| `:alpn` not an array | `:type-error` | `"tls/client-state: :alpn must be an array of strings, got ..."` |
| Empty `:alpn` entry | `:value-error` | `"tls/client-state: :alpn entries must not be empty"` |
| `:client-ca` file not found | `:io-error` | `"tls/server-config: reading client-ca '...'..."` |
| No usable CA in `:client-ca` | `:tls-error` | `"tls/server-config: client-ca ..."` |
| No shared ALPN protocol | `:tls-error` | `"tls/process: peer is incompatible: no application protocol"` |
| rustls protocol error | `:tls-error` | `"tls/process: ..."` |
| Write before handshake | — (returns `{:status :error :message string}`) | `"tls/write-plaintext: handshake not complete"` |
| Cert file not found | `:io-error` | `"tls/server-config: reading cert-path '...'..."` |
| Key file not found | `:io-error` | `"tls/server-config: reading key-path '...'..."` |
| No certs in PEM file | `:tls-error` | `"tls/server-config: no certificates found in '...'"` |
| Cert/key mismatch | `:tls-error` | `"tls/server-config: server config error: ..."` |
| Wrong type for arg | `:type-error` | `"tls/XXX: expected YYY, got ZZZ"` |

## Invariants

1. **No I/O in the plugin.** All network operations happen in Elle code.
   Plugin primitives are pure state machine operations.

2. **Buffer ownership.** `incoming`, `outgoing`, `plaintext` buffers live in
   the `TlsState` Rust struct. Elle code feeds and drains them via primitives.
   Elle never holds a direct reference to these buffers.

3. **Outgoing data invariant.** After every `tls/process` call, the caller
   MUST drain and send any outgoing bytes via `tls/get-outgoing` and
   `port/write`. TLS 1.3 may produce post-handshake messages at any time.
   Failing to send them will stall the connection. `tls/write-plaintext`
   returns outgoing bytes directly in its result struct — no separate drain
   needed for writes.

4. **Handshake-before-write.** `tls/write-plaintext` returns
   `{:status :error :message "tls/write-plaintext: handshake not complete"}`
   if called before `tls/handshake-complete?` returns true.

5. **close_notify must be sent before closing TCP.** Call `tls/close-notify`
   to get the encoded alert bytes, send them via `port/write`, then call
   `port/close` on the TCP port. `lib/tls.lisp`'s `tls/close` does this
   automatically.

6. **Crypto provider is global.** `define_plugin!` generates
   `elle_plugin_init` and offers no hook for custom init code, so
   `ring::default_provider().install_default()` runs lazily instead —
   `ensure_provider()` at the top of every primitive that builds a
   connection or a config. Every call after the first returns `Err`,
   which is ignored.

7. **Server config is immutable after creation.** `TlsServerConfig` wraps
   `Arc<ServerConfig>`. Multiple `tls/server-state` calls clone the Arc cheaply.

## Coupling points

- `Value::external()`, `as_external::<T>()` — ExternalObject creation/access
- `error_val()` — error construction
- `PluginContext::register()` — primitive registration
- `PrimitiveDef`, `NativeFn`, `Arity`, `Signal` — primitive definition
- `SIG_OK`, `SIG_ERROR` — signal returns

## Files

| File | Purpose |
|------|---------|
| `Cargo.toml` | Crate definition (cdylib, dependencies) |
| `src/lib.rs` | State structs, drive loop, primitives, entry point |
| `src/config.rs` | Option parsing and rustls config construction |
| `src/verify.rs` | The `:no-verify` certificate verifier |
| `../tests/tls.lisp` | Integration tests — a client and a server state machine wired to each other in process, plus one ALPN handshake over a socket |
