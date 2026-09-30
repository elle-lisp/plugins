# elle-crypto

<!-- audited: 2026-09-30 -->

A crypto plugin for Elle: SHA-2 hashes and HMAC via the Rust `sha2` and `hmac` crates, and Ed25519 signatures via `ed25519-dalek`.

## Building

Built as part of the workspace:

```sh
cargo build --workspace
```

Produces `target/debug/libelle_crypto.so` (or `target/release/libelle_crypto.so`).

## Usage

```lisp
(import-file "path/to/libelle_crypto.so")

(bytes->hex (crypto/sha256 "hello"))
;; => "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"

(bytes->hex (crypto/hmac-sha256 "key" "message"))
;; => "6e9ef29b75fffc5b7abae527d58fdadb2fe42e7219011976917343065f58ed4a"

(def seed (random/csprng-bytes 32))
(def pk (crypto/ed25519-public seed))
(def sig (crypto/ed25519-sign seed "message"))
(crypto/ed25519-verify pk "message" sig)
;; => true
```

## Primitives

### Hash functions

All accept string, bytes, or @bytes input.

| Name | Output size | Description |
|------|-------------|-------------|
| `crypto/sha224` | 28 bytes | SHA-224 hash |
| `crypto/sha256` | 32 bytes | SHA-256 hash |
| `crypto/sha384` | 48 bytes | SHA-384 hash |
| `crypto/sha512` | 64 bytes | SHA-512 hash |
| `crypto/sha512-224` | 28 bytes | SHA-512/224 (SHA-512 truncated to 224 bits) |
| `crypto/sha512-256` | 32 bytes | SHA-512/256 (SHA-512 truncated to 256 bits) |

### HMAC functions

All take (key, message) where each is string, bytes, or blob.

| Name | Output size | Description |
|------|-------------|-------------|
| `crypto/hmac-sha224` | 28 bytes | HMAC-SHA224 |
| `crypto/hmac-sha256` | 32 bytes | HMAC-SHA256 |
| `crypto/hmac-sha384` | 48 bytes | HMAC-SHA384 |
| `crypto/hmac-sha512` | 64 bytes | HMAC-SHA512 |
| `crypto/hmac-sha512-224` | 28 bytes | HMAC-SHA512/224 |
| `crypto/hmac-sha512-256` | 32 bytes | HMAC-SHA512/256 |

### Ed25519 signatures

A seed is the 32-byte private key of RFC 8032. A seed or a public key
must be exactly 32 bytes, and a signature exactly 64 bytes; any other
length is a `:range-error`. A message is a string or bytes of any length.

| Name | Arguments | Output | Description |
|------|-----------|--------|-------------|
| `crypto/ed25519-public` | seed | 32 bytes | the public key of a seed |
| `crypto/ed25519-sign` | seed, message | 64 bytes | the signature of message under the seed |
| `crypto/ed25519-verify` | public-key, message, signature | boolean | true when the signature is valid for message under the public key |

`crypto/ed25519-verify` answers `false` for a signature that does not
verify, and raises only for an argument of the wrong length or type.
