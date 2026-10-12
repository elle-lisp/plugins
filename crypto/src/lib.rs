//! Elle crypto plugin — SHA-2 hashes, HMAC, HKDF, Ed25519 signatures, X25519 key agreement and XChaCha20-Poly1305 encryption.
//!
//! Uses the stable elle-plugin ABI. Can be compiled independently from elle.

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::XChaCha20Poly1305;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha224, Sha256, Sha384, Sha512, Sha512_224, Sha512_256};
use x25519_dalek::{PublicKey as X25519Public, StaticSecret};

use elle_plugin::{ElleCtx, ElleResult, ElleValue, EllePrimDef, SIG_ERROR, SIG_OK};

elle_plugin::define_plugin!("crypto/", &PRIMITIVES);

// ── Helpers ───────────────────────────────────────────────────────────

/// Coerce a string or bytes argument to raw bytes.
///
/// Takes the per-call `ctx` so that the error value it may construct is
/// allocated into the calling primitive's region (ABI v3).
fn extract_byte_data(
    ctx: *mut ElleCtx,
    val: ElleValue,
    name: &str,
    pos: &str,
) -> Result<Vec<u8>, ElleResult> {
    let a = api();
    if let Some(s) = a.get_string(val) {
        Ok(s.as_bytes().to_vec())
    } else if let Some(b) = a.get_bytes(val) {
        Ok(b.to_vec())
    } else {
        Err(a.err(ctx, "type-error", &format!(
            "{}: {} must be string or bytes", name, pos,
        )))
    }
}

// ── Primitive generators ──────────────────────────────────────────────

macro_rules! hash_primitive {
    ($fn_name:ident, $hasher:ty, $prim_name:expr) => {
        extern "C" fn $fn_name(
            ctx: *mut ElleCtx,
            args: *const ElleValue,
            nargs: usize,
        ) -> ElleResult {
            let a = api();
            if nargs != 1 {
                return a.err(ctx, "arity-error", &format!(
                    "{}: expected 1 argument, got {}", $prim_name, nargs,
                ));
            }
            let val = unsafe { a.arg(args, nargs, 0) };
            let data = match extract_byte_data(ctx, val, $prim_name, "argument") {
                Ok(d) => d,
                Err(e) => return e,
            };
            let hash = <$hasher>::digest(&data);
            a.ok(a.bytes(ctx, &hash))
        }
    };
}

macro_rules! hmac_primitive {
    ($fn_name:ident, $hasher:ty, $prim_name:expr) => {
        extern "C" fn $fn_name(
            ctx: *mut ElleCtx,
            args: *const ElleValue,
            nargs: usize,
        ) -> ElleResult {
            let a = api();
            if nargs != 2 {
                return a.err(ctx, "arity-error", &format!(
                    "{}: expected 2 arguments, got {}", $prim_name, nargs,
                ));
            }
            let key_val = unsafe { a.arg(args, nargs, 0) };
            let msg_val = unsafe { a.arg(args, nargs, 1) };
            let key = match extract_byte_data(ctx, key_val, $prim_name, "key") {
                Ok(d) => d,
                Err(e) => return e,
            };
            let message = match extract_byte_data(ctx, msg_val, $prim_name, "message") {
                Ok(d) => d,
                Err(e) => return e,
            };
            let mut mac =
                <Hmac<$hasher>>::new_from_slice(&key).expect("HMAC accepts any key length");
            mac.update(&message);
            let result = mac.finalize().into_bytes();
            a.ok(a.bytes(ctx, &result))
        }
    };
}

// ── Primitives ────────────────────────────────────────────────────────

hash_primitive!(prim_sha224, Sha224, "crypto/sha224");
hash_primitive!(prim_sha256, Sha256, "crypto/sha256");
hash_primitive!(prim_sha384, Sha384, "crypto/sha384");
hash_primitive!(prim_sha512, Sha512, "crypto/sha512");
hash_primitive!(prim_sha512_224, Sha512_224, "crypto/sha512-224");
hash_primitive!(prim_sha512_256, Sha512_256, "crypto/sha512-256");

hmac_primitive!(prim_hmac_sha224, Sha224, "crypto/hmac-sha224");
hmac_primitive!(prim_hmac_sha256, Sha256, "crypto/hmac-sha256");
hmac_primitive!(prim_hmac_sha384, Sha384, "crypto/hmac-sha384");
hmac_primitive!(prim_hmac_sha512, Sha512, "crypto/hmac-sha512");
hmac_primitive!(prim_hmac_sha512_224, Sha512_224, "crypto/hmac-sha512-224");
hmac_primitive!(prim_hmac_sha512_256, Sha512_256, "crypto/hmac-sha512-256");

// ── Ed25519 ───────────────────────────────────────────────────────────

/// Coerce an argument to exactly `n` bytes, as a seed, a public key or a
/// signature requires. A wrong length is a range-error, so a truncated key
/// never silently derives a different identity.
fn fixed_bytes<const N: usize>(
    ctx: *mut ElleCtx,
    val: ElleValue,
    name: &str,
    pos: &str,
) -> Result<[u8; N], ElleResult> {
    let a = api();
    let data = extract_byte_data(ctx, val, name, pos)?;
    <[u8; N]>::try_from(data.as_slice()).map_err(|_| {
        a.err(ctx, "range-error", &format!(
            "{}: {} must be exactly {} bytes, got {}", name, pos, N, data.len(),
        ))
    })
}

extern "C" fn prim_ed25519_public(
    ctx: *mut ElleCtx,
    args: *const ElleValue,
    nargs: usize,
) -> ElleResult {
    let a = api();
    let seed = match fixed_bytes::<32>(ctx, unsafe { a.arg(args, nargs, 0) }, "crypto/ed25519-public", "seed") {
        Ok(s) => s,
        Err(e) => return e,
    };
    let key = SigningKey::from_bytes(&seed);
    a.ok(a.bytes(ctx, &key.verifying_key().to_bytes()))
}

extern "C" fn prim_ed25519_sign(
    ctx: *mut ElleCtx,
    args: *const ElleValue,
    nargs: usize,
) -> ElleResult {
    let a = api();
    let seed = match fixed_bytes::<32>(ctx, unsafe { a.arg(args, nargs, 0) }, "crypto/ed25519-sign", "seed") {
        Ok(s) => s,
        Err(e) => return e,
    };
    let message = match extract_byte_data(ctx, unsafe { a.arg(args, nargs, 1) }, "crypto/ed25519-sign", "message") {
        Ok(m) => m,
        Err(e) => return e,
    };
    let key = SigningKey::from_bytes(&seed);
    a.ok(a.bytes(ctx, &key.sign(&message).to_bytes()))
}

extern "C" fn prim_ed25519_verify(
    ctx: *mut ElleCtx,
    args: *const ElleValue,
    nargs: usize,
) -> ElleResult {
    let a = api();
    let pk = match fixed_bytes::<32>(ctx, unsafe { a.arg(args, nargs, 0) }, "crypto/ed25519-verify", "public key") {
        Ok(k) => k,
        Err(e) => return e,
    };
    let message = match extract_byte_data(ctx, unsafe { a.arg(args, nargs, 1) }, "crypto/ed25519-verify", "message") {
        Ok(m) => m,
        Err(e) => return e,
    };
    let sig = match fixed_bytes::<64>(ctx, unsafe { a.arg(args, nargs, 2) }, "crypto/ed25519-verify", "signature") {
        Ok(s) => s,
        Err(e) => return e,
    };
    // A public key that is not a point on the curve verifies nothing, and a
    // signature that does not check out is the false this answers, not a raise.
    let ok = match VerifyingKey::from_bytes(&pk) {
        Ok(key) => key.verify(&message, &Signature::from_bytes(&sig)).is_ok(),
        Err(_) => false,
    };
    a.ok(a.boolean(ok))
}

// ── HKDF ──────────────────────────────────────────────────────────────

/// The most output HKDF-SHA256 can expand to: 255 blocks of 32 bytes.
const HKDF_SHA256_MAX: i64 = 255 * 32;

extern "C" fn prim_hkdf_sha256(
    ctx: *mut ElleCtx,
    args: *const ElleValue,
    nargs: usize,
) -> ElleResult {
    let a = api();
    let name = "crypto/hkdf-sha256";
    let ikm = match extract_byte_data(ctx, unsafe { a.arg(args, nargs, 0) }, name, "ikm") {
        Ok(d) => d,
        Err(e) => return e,
    };
    let salt = match extract_byte_data(ctx, unsafe { a.arg(args, nargs, 1) }, name, "salt") {
        Ok(d) => d,
        Err(e) => return e,
    };
    let info = match extract_byte_data(ctx, unsafe { a.arg(args, nargs, 2) }, name, "info") {
        Ok(d) => d,
        Err(e) => return e,
    };
    let length = match a.get_int(unsafe { a.arg(args, nargs, 3) }) {
        Some(n) => n,
        None => return a.err(ctx, "type-error", &format!("{}: length must be an integer", name)),
    };
    if !(0..=HKDF_SHA256_MAX).contains(&length) {
        return a.err(ctx, "range-error", &format!(
            "{}: length must be from 0 to {}, got {}", name, HKDF_SHA256_MAX, length,
        ));
    }
    // An empty salt keys HMAC with no bytes, which HMAC pads to the string
    // of zeros that RFC 5869 names as the salt when none is given.
    let mut okm = vec![0u8; length as usize];
    Hkdf::<Sha256>::new(Some(&salt), &ikm)
        .expand(&info, &mut okm)
        .expect("a length within 255 blocks always expands");
    a.ok(a.bytes(ctx, &okm))
}

// ── X25519 ────────────────────────────────────────────────────────────

extern "C" fn prim_x25519_public(
    ctx: *mut ElleCtx,
    args: *const ElleValue,
    nargs: usize,
) -> ElleResult {
    let a = api();
    let secret = match fixed_bytes::<32>(ctx, unsafe { a.arg(args, nargs, 0) }, "crypto/x25519-public", "secret") {
        Ok(s) => s,
        Err(e) => return e,
    };
    a.ok(a.bytes(ctx, X25519Public::from(&StaticSecret::from(secret)).as_bytes()))
}

extern "C" fn prim_x25519(
    ctx: *mut ElleCtx,
    args: *const ElleValue,
    nargs: usize,
) -> ElleResult {
    let a = api();
    let name = "crypto/x25519";
    let secret = match fixed_bytes::<32>(ctx, unsafe { a.arg(args, nargs, 0) }, name, "secret") {
        Ok(s) => s,
        Err(e) => return e,
    };
    let public = match fixed_bytes::<32>(ctx, unsafe { a.arg(args, nargs, 1) }, name, "public key") {
        Ok(p) => p,
        Err(e) => return e,
    };
    let shared = StaticSecret::from(secret).diffie_hellman(&X25519Public::from(public));
    // A public key of small order yields a secret that every party can
    // compute, so it is refused rather than answered.
    if !shared.was_contributory() {
        return a.err(ctx, "value-error", &format!(
            "{}: the public key has small order, so the shared secret is not secret", name,
        ));
    }
    a.ok(a.bytes(ctx, shared.as_bytes()))
}

// ── XChaCha20-Poly1305 ────────────────────────────────────────────────

/// The four arguments of seal and open: a 32-byte key, a 24-byte nonce, the
/// message, and the associated data.
type AeadArgs = ([u8; 32], [u8; 24], Vec<u8>, Vec<u8>);

fn aead_args(
    ctx: *mut ElleCtx,
    args: *const ElleValue,
    nargs: usize,
    name: &str,
    message: &str,
) -> Result<AeadArgs, ElleResult> {
    let a = api();
    let key = fixed_bytes::<32>(ctx, unsafe { a.arg(args, nargs, 0) }, name, "key")?;
    let nonce = fixed_bytes::<24>(ctx, unsafe { a.arg(args, nargs, 1) }, name, "nonce")?;
    let msg = extract_byte_data(ctx, unsafe { a.arg(args, nargs, 2) }, name, message)?;
    let aad = extract_byte_data(ctx, unsafe { a.arg(args, nargs, 3) }, name, "aad")?;
    Ok((key, nonce, msg, aad))
}

extern "C" fn prim_xchacha20poly1305_seal(
    ctx: *mut ElleCtx,
    args: *const ElleValue,
    nargs: usize,
) -> ElleResult {
    let a = api();
    let (key, nonce, plaintext, aad) =
        match aead_args(ctx, args, nargs, "crypto/xchacha20poly1305-seal", "plaintext") {
            Ok(v) => v,
            Err(e) => return e,
        };
    let sealed = XChaCha20Poly1305::new(&key.into())
        .encrypt(&nonce.into(), Payload { msg: &plaintext, aad: &aad })
        .expect("sealing into a Vec cannot run out of room");
    a.ok(a.bytes(ctx, &sealed))
}

extern "C" fn prim_xchacha20poly1305_open(
    ctx: *mut ElleCtx,
    args: *const ElleValue,
    nargs: usize,
) -> ElleResult {
    let a = api();
    let (key, nonce, ciphertext, aad) =
        match aead_args(ctx, args, nargs, "crypto/xchacha20poly1305-open", "ciphertext") {
            Ok(v) => v,
            Err(e) => return e,
        };
    // A ciphertext that does not authenticate is the nil this answers, not
    // a raise, as ed25519-verify answers false.
    match XChaCha20Poly1305::new(&key.into())
        .decrypt(&nonce.into(), Payload { msg: &ciphertext, aad: &aad })
    {
        Ok(plaintext) => a.ok(a.bytes(ctx, &plaintext)),
        Err(_) => a.ok(a.nil()),
    }
}

// ── Registration table ────────────────────────────────────────────────

static PRIMITIVES: &[EllePrimDef] = &[
    EllePrimDef::exact(
        "crypto/hkdf-sha256", prim_hkdf_sha256, SIG_ERROR, 4,
        "HKDF-SHA256 (RFC 5869). Takes (ikm, salt, info, length). Returns length bytes, at most 8160.",
        "crypto",
        "(bytes->hex (crypto/hkdf-sha256 \"ikm\" \"salt\" \"info\" 32))",
    ),
    EllePrimDef::exact(
        "crypto/x25519-public", prim_x25519_public, SIG_ERROR, 1,
        "The X25519 public key of a 32-byte secret. Returns 32 bytes.",
        "crypto",
        "(bytes->hex (crypto/x25519-public secret))",
    ),
    EllePrimDef::exact(
        "crypto/x25519", prim_x25519, SIG_ERROR, 2,
        "The X25519 shared secret of a 32-byte secret and another side's 32-byte public key. Returns 32 bytes; a public key of small order is a value-error.",
        "crypto",
        "(bytes->hex (crypto/x25519 secret their-public))",
    ),
    EllePrimDef::exact(
        "crypto/xchacha20poly1305-seal", prim_xchacha20poly1305_seal, SIG_ERROR, 4,
        "Encrypt with XChaCha20-Poly1305. Takes (key, nonce, plaintext, aad): a 32-byte key and a 24-byte nonce. Returns the ciphertext followed by its 16-byte tag.",
        "crypto",
        "(crypto/xchacha20poly1305-seal key nonce \"message\" \"\")",
    ),
    EllePrimDef::exact(
        "crypto/xchacha20poly1305-open", prim_xchacha20poly1305_open, SIG_ERROR, 4,
        "Decrypt with XChaCha20-Poly1305. Takes (key, nonce, ciphertext, aad). Returns the plaintext, or nil when the ciphertext does not authenticate.",
        "crypto",
        "(crypto/xchacha20poly1305-open key nonce sealed \"\")",
    ),
    EllePrimDef::exact(
        "crypto/ed25519-public", prim_ed25519_public, SIG_ERROR, 1,
        "The Ed25519 public key of a 32-byte seed. Returns 32 bytes.",
        "crypto",
        "(bytes->hex (crypto/ed25519-public seed))",
    ),
    EllePrimDef::exact(
        "crypto/ed25519-sign", prim_ed25519_sign, SIG_ERROR, 2,
        "Sign a message under a 32-byte Ed25519 seed. Takes (seed, message). Returns 64 bytes.",
        "crypto",
        "(bytes->hex (crypto/ed25519-sign seed \"message\"))",
    ),
    EllePrimDef::exact(
        "crypto/ed25519-verify", prim_ed25519_verify, SIG_ERROR, 3,
        "Verify an Ed25519 signature. Takes (public-key, message, signature). Returns a boolean.",
        "crypto",
        "(crypto/ed25519-verify pk \"message\" sig)",
    ),
    EllePrimDef::exact(
        "crypto/sha224", prim_sha224, SIG_OK, 1,
        "SHA-224 hash. Accepts string or bytes. Returns 28 bytes.",
        "crypto",
        "(bytes->hex (crypto/sha224 \"hello\"))",
    ),
    EllePrimDef::exact(
        "crypto/sha256", prim_sha256, SIG_OK, 1,
        "SHA-256 hash. Accepts string or bytes. Returns 32 bytes.",
        "crypto",
        "(bytes->hex (crypto/sha256 \"hello\"))",
    ),
    EllePrimDef::exact(
        "crypto/sha384", prim_sha384, SIG_OK, 1,
        "SHA-384 hash. Accepts string or bytes. Returns 48 bytes.",
        "crypto",
        "(bytes->hex (crypto/sha384 \"hello\"))",
    ),
    EllePrimDef::exact(
        "crypto/sha512", prim_sha512, SIG_OK, 1,
        "SHA-512 hash. Accepts string or bytes. Returns 64 bytes.",
        "crypto",
        "(bytes->hex (crypto/sha512 \"hello\"))",
    ),
    EllePrimDef::exact(
        "crypto/sha512-224", prim_sha512_224, SIG_OK, 1,
        "SHA-512/224 hash. Accepts string or bytes. Returns 28 bytes.",
        "crypto",
        "(bytes->hex (crypto/sha512-224 \"hello\"))",
    ),
    EllePrimDef::exact(
        "crypto/sha512-256", prim_sha512_256, SIG_OK, 1,
        "SHA-512/256 hash. Accepts string or bytes. Returns 32 bytes.",
        "crypto",
        "(bytes->hex (crypto/sha512-256 \"hello\"))",
    ),
    EllePrimDef::exact(
        "crypto/hmac-sha224", prim_hmac_sha224, SIG_OK, 2,
        "HMAC-SHA224. Takes (key, message). Returns 28 bytes.",
        "crypto",
        "(bytes->hex (crypto/hmac-sha224 \"key\" \"message\"))",
    ),
    EllePrimDef::exact(
        "crypto/hmac-sha256", prim_hmac_sha256, SIG_OK, 2,
        "HMAC-SHA256. Takes (key, message). Returns 32 bytes.",
        "crypto",
        "(bytes->hex (crypto/hmac-sha256 \"key\" \"message\"))",
    ),
    EllePrimDef::exact(
        "crypto/hmac-sha384", prim_hmac_sha384, SIG_OK, 2,
        "HMAC-SHA384. Takes (key, message). Returns 48 bytes.",
        "crypto",
        "(bytes->hex (crypto/hmac-sha384 \"key\" \"message\"))",
    ),
    EllePrimDef::exact(
        "crypto/hmac-sha512", prim_hmac_sha512, SIG_OK, 2,
        "HMAC-SHA512. Takes (key, message). Returns 64 bytes.",
        "crypto",
        "(bytes->hex (crypto/hmac-sha512 \"key\" \"message\"))",
    ),
    EllePrimDef::exact(
        "crypto/hmac-sha512-224", prim_hmac_sha512_224, SIG_OK, 2,
        "HMAC-SHA512/224. Takes (key, message). Returns 28 bytes.",
        "crypto",
        "(bytes->hex (crypto/hmac-sha512-224 \"key\" \"message\"))",
    ),
    EllePrimDef::exact(
        "crypto/hmac-sha512-256", prim_hmac_sha512_256, SIG_OK, 2,
        "HMAC-SHA512/256. Takes (key, message). Returns 32 bytes.",
        "crypto",
        "(bytes->hex (crypto/hmac-sha512-256 \"key\" \"message\"))",
    ),
];
