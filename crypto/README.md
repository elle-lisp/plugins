# elle-crypto

<!-- audited: 2026-10-12 -->

A crypto plugin for Elle: SHA-2 hashes, HMAC and HKDF via the RustCrypto crates, Ed25519 signatures and X25519 key agreement via the dalek crates, and XChaCha20-Poly1305 encryption.

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

### HKDF

`crypto/hkdf-sha256` is the HKDF of RFC 5869 over SHA-256. It extracts a
key from `ikm` and `salt`, then expands it under `info` to `length` bytes.
An empty `salt` is the salt of zeros that RFC 5869 defines. Each of `ikm`,
`salt` and `info` is a string or bytes.

| Name | Arguments | Output | Description |
|------|-----------|--------|-------------|
| `crypto/hkdf-sha256` | ikm, salt, info, length | `length` bytes | key material derived from ikm |

A `length` that is not an integer is a `:type-error`. A `length` below 0 or
above 8160 (255 × 32) is a `:range-error`.

### X25519 key agreement

A secret and a public key are each exactly 32 bytes. Any other length is a
`:range-error`. Each side computes `crypto/x25519` from its own secret and
the other side's public key, and both get the same 32 bytes (RFC 7748).

| Name | Arguments | Output | Description |
|------|-----------|--------|-------------|
| `crypto/x25519-public` | secret | 32 bytes | the public key of a secret |
| `crypto/x25519` | secret, public-key | 32 bytes | the shared secret of the two keys |

A public key of small order gives a shared secret of all zeros, which
anyone can compute. `crypto/x25519` raises a `:value-error` for such a key,
rather than answer that secret. Pass the shared secret through
`crypto/hkdf-sha256` before you use it as a key.

### XChaCha20-Poly1305 encryption

XChaCha20-Poly1305 is an AEAD: it encrypts a message, and it authenticates
the message together with associated data that travels in the clear. A key
is exactly 32 bytes and a nonce exactly 24 bytes; any other length is a
`:range-error`. The plaintext, the ciphertext and the associated data are
strings or bytes of any length.

| Name | Arguments | Output | Description |
|------|-----------|--------|-------------|
| `crypto/xchacha20poly1305-seal` | key, nonce, plaintext, aad | bytes | the ciphertext, followed by its 16-byte tag |
| `crypto/xchacha20poly1305-open` | key, nonce, ciphertext, aad | bytes or nil | the plaintext, or nil when the ciphertext does not authenticate |

`crypto/xchacha20poly1305-open` answers `nil` for a wrong key, a wrong
nonce, other associated data, or a ciphertext changed in any byte. It
raises only for an argument of the wrong length or type. Never seal two
messages under one key with one nonce. A nonce of 24 random bytes from
`random/csprng-bytes` makes such a repeat negligible.
