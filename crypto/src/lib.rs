//! Elle crypto plugin — SHA-2 family hashes and HMAC.
//!
//! Uses the stable elle-plugin ABI. Can be compiled independently from elle.

use hmac::{Hmac, Mac};
use sha2::{Digest, Sha224, Sha256, Sha384, Sha512, Sha512_224, Sha512_256};

use elle_plugin::{ElleResult, ElleValue, EllePrimDef, SIG_OK};

elle_plugin::define_plugin!("crypto/", &PRIMITIVES);

// ── Helpers ───────────────────────────────────────────────────────────

fn extract_byte_data(
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
        Err(a.err("type-error", &format!(
            "{}: {} must be string or bytes", name, pos,
        )))
    }
}

// ── Primitive generators ──────────────────────────────────────────────

macro_rules! hash_primitive {
    ($fn_name:ident, $hasher:ty, $prim_name:expr) => {
        extern "C" fn $fn_name(
            args: *const ElleValue,
            nargs: usize,
        ) -> ElleResult {
            let a = api();
            if nargs != 1 {
                return a.err("arity-error", &format!(
                    "{}: expected 1 argument, got {}", $prim_name, nargs,
                ));
            }
            let val = unsafe { a.arg(args, nargs, 0) };
            let data = match extract_byte_data(val, $prim_name, "argument") {
                Ok(d) => d,
                Err(e) => return e,
            };
            let hash = <$hasher>::digest(&data);
            a.ok(a.bytes(&hash))
        }
    };
}

macro_rules! hmac_primitive {
    ($fn_name:ident, $hasher:ty, $prim_name:expr) => {
        extern "C" fn $fn_name(
            args: *const ElleValue,
            nargs: usize,
        ) -> ElleResult {
            let a = api();
            if nargs != 2 {
                return a.err("arity-error", &format!(
                    "{}: expected 2 arguments, got {}", $prim_name, nargs,
                ));
            }
            let key_val = unsafe { a.arg(args, nargs, 0) };
            let msg_val = unsafe { a.arg(args, nargs, 1) };
            let key = match extract_byte_data(key_val, $prim_name, "key") {
                Ok(d) => d,
                Err(e) => return e,
            };
            let message = match extract_byte_data(msg_val, $prim_name, "message") {
                Ok(d) => d,
                Err(e) => return e,
            };
            let mut mac =
                <Hmac<$hasher>>::new_from_slice(&key).expect("HMAC accepts any key length");
            mac.update(&message);
            let result = mac.finalize().into_bytes();
            a.ok(a.bytes(&result))
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

// ── Registration table ────────────────────────────────────────────────

static PRIMITIVES: &[EllePrimDef] = &[
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
