//! Elle hash plugin — universal hashing with MD5, SHA-1, SHA-2, SHA-3,
//! BLAKE2, BLAKE3, CRC32, and xxHash.

use blake2::{Blake2b512, Blake2s256};
use crc32fast::Hasher as Crc32;
use digest::Digest;
use md5::Md5;
use sha1::Sha1;
use sha2::{Sha224, Sha256, Sha384, Sha512, Sha512_224, Sha512_256};
use sha3::{Sha3_224, Sha3_256, Sha3_384, Sha3_512};
use std::cell::RefCell;

use elle_plugin::{ElleResult, ElleValue, EllePrimDef, SIG_OK, SIG_ERROR};

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------
elle_plugin::define_plugin!("hash/", &PRIMITIVES);

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract byte data from a string or bytes value.
fn extract_bytes(val: ElleValue, name: &str, pos: &str) -> Result<Vec<u8>, ElleResult> {
    let a = api();
    if let Some(s) = a.get_string(val) {
        Ok(s.as_bytes().to_vec())
    } else if let Some(b) = a.get_bytes(val) {
        Ok(b.to_vec())
    } else {
        Err(a.err(
            "type-error",
            &format!(
                "{}: {} must be string or bytes, got {}",
                name,
                pos,
                a.type_name(val),
            ),
        ))
    }
}

/// Extract a string from a Value, or return a type error.
fn extract_string(val: ElleValue, name: &str, pos: &str) -> Result<String, ElleResult> {
    let a = api();
    a.get_string(val)
        .map(|s| s.to_string())
        .ok_or_else(|| {
            a.err(
                "type-error",
                &format!(
                    "{}: {} must be a string, got {}",
                    name,
                    pos,
                    a.type_name(val),
                ),
            )
        })
}

/// Check arity and extract byte data for a unary hash primitive.
fn oneshot_args(args: *const ElleValue, nargs: usize, name: &str) -> Result<Vec<u8>, ElleResult> {
    let a = api();
    if nargs != 1 {
        return Err(a.err(
            "arity-error",
            &format!("{}: expected 1 argument, got {}", name, nargs),
        ));
    }
    extract_bytes(a.arg(args, nargs, 0), name, "argument")
}

// ---------------------------------------------------------------------------
// One-shot primitives (Digest-based -> bytes)
// ---------------------------------------------------------------------------

macro_rules! digest_prim {
    ($fn_name:ident, $hasher:ty, $prim_name:expr) => {
        extern "C" fn $fn_name(args: *const ElleValue, nargs: usize) -> ElleResult {
            let a = api();
            match oneshot_args(args, nargs, $prim_name) {
                Ok(data) => a.ok(a.bytes(&<$hasher>::digest(&data).to_vec())),
                Err(e) => e,
            }
        }
    };
}

digest_prim!(prim_md5, Md5, "hash/md5");
digest_prim!(prim_sha1, Sha1, "hash/sha1");
digest_prim!(prim_sha224, Sha224, "hash/sha224");
digest_prim!(prim_sha256, Sha256, "hash/sha256");
digest_prim!(prim_sha384, Sha384, "hash/sha384");
digest_prim!(prim_sha512, Sha512, "hash/sha512");
digest_prim!(prim_sha512_224, Sha512_224, "hash/sha512-224");
digest_prim!(prim_sha512_256, Sha512_256, "hash/sha512-256");
digest_prim!(prim_sha3_224, Sha3_224, "hash/sha3-224");
digest_prim!(prim_sha3_256, Sha3_256, "hash/sha3-256");
digest_prim!(prim_sha3_384, Sha3_384, "hash/sha3-384");
digest_prim!(prim_sha3_512, Sha3_512, "hash/sha3-512");
digest_prim!(prim_blake2b512, Blake2b512, "hash/blake2b-512");
digest_prim!(prim_blake2s256, Blake2s256, "hash/blake2s-256");

// ---------------------------------------------------------------------------
// BLAKE3 one-shot (own API, not RustCrypto Digest)
// ---------------------------------------------------------------------------

extern "C" fn prim_blake3(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    match oneshot_args(args, nargs, "hash/blake3") {
        Ok(data) => a.ok(a.bytes(blake3::hash(&data).as_bytes())),
        Err(e) => e,
    }
}

extern "C" fn prim_blake3_keyed(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    if nargs != 2 {
        return a.err(
            "arity-error",
            &format!("hash/blake3-keyed: expected 2 arguments, got {}", nargs),
        );
    }
    let key = match extract_bytes(a.arg(args, nargs, 0), "hash/blake3-keyed", "key") {
        Ok(d) => d,
        Err(e) => return e,
    };
    if key.len() != 32 {
        return a.err(
            "value-error",
            &format!(
                "hash/blake3-keyed: key must be exactly 32 bytes, got {}",
                key.len(),
            ),
        );
    }
    let data = match extract_bytes(a.arg(args, nargs, 1), "hash/blake3-keyed", "data") {
        Ok(d) => d,
        Err(e) => return e,
    };
    let key_arr: [u8; 32] = key.try_into().unwrap();
    a.ok(a.bytes(blake3::keyed_hash(&key_arr, &data).as_bytes()))
}

extern "C" fn prim_blake3_derive(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    if nargs != 2 {
        return a.err(
            "arity-error",
            &format!("hash/blake3-derive: expected 2 arguments, got {}", nargs),
        );
    }
    let context = match extract_string(a.arg(args, nargs, 0), "hash/blake3-derive", "context") {
        Ok(s) => s,
        Err(e) => return e,
    };
    let data = match extract_bytes(a.arg(args, nargs, 1), "hash/blake3-derive", "data") {
        Ok(d) => d,
        Err(e) => return e,
    };
    a.ok(a.bytes(&blake3::derive_key(&context, &data)))
}

// ---------------------------------------------------------------------------
// CRC32 and xxHash one-shot (return integers or bytes)
// ---------------------------------------------------------------------------

extern "C" fn prim_crc32(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    match oneshot_args(args, nargs, "hash/crc32") {
        Ok(data) => {
            let mut h = Crc32::new();
            h.update(&data);
            a.ok(a.int(h.finalize() as i64))
        }
        Err(e) => e,
    }
}

extern "C" fn prim_xxh32(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    match oneshot_args(args, nargs, "hash/xxh32") {
        Ok(data) => a.ok(a.int(xxhash_rust::xxh32::xxh32(&data, 0) as i64)),
        Err(e) => e,
    }
}

extern "C" fn prim_xxh64(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    match oneshot_args(args, nargs, "hash/xxh64") {
        Ok(data) => a.ok(a.int(xxhash_rust::xxh3::xxh3_64(&data) as i64)),
        Err(e) => e,
    }
}

extern "C" fn prim_xxh128(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    match oneshot_args(args, nargs, "hash/xxh128") {
        Ok(data) => a.ok(a.bytes(&xxhash_rust::xxh3::xxh3_128(&data).to_be_bytes())),
        Err(e) => e,
    }
}

// ---------------------------------------------------------------------------
// hex and algorithms
// ---------------------------------------------------------------------------

/// Shared list of algorithm keyword names, used by both make_hasher and prim_algorithms.
const ALGORITHM_NAMES: &[&str] = &[
    "md5",
    "sha1",
    "sha224",
    "sha256",
    "sha384",
    "sha512",
    "sha512-224",
    "sha512-256",
    "sha3-224",
    "sha3-256",
    "sha3-384",
    "sha3-512",
    "blake2b-512",
    "blake2s-256",
    "blake3",
    "crc32",
    "xxh32",
    "xxh64",
    "xxh128",
];

extern "C" fn prim_hex(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    if nargs != 2 {
        return a.err(
            "arity-error",
            &format!("hash/hex: expected 2 arguments, got {}", nargs),
        );
    }
    let arg0 = a.arg(args, nargs, 0);
    let kw = match a.get_keyword_name(arg0) {
        Some(k) => k.to_string(),
        None => {
            return a.err(
                "type-error",
                &format!(
                    "hash/hex: first argument must be a keyword, got {}",
                    a.type_name(arg0),
                ),
            );
        }
    };
    let data = match extract_bytes(a.arg(args, nargs, 1), "hash/hex", "data") {
        Ok(d) => d,
        Err(e) => return e,
    };
    // Route through HasherState for consistency with streaming API
    match make_hasher(&kw) {
        Ok(mut state) => {
            state.update(&data);
            let digest = state.finalize_reset(a);
            // Check if bytes or int
            if let Some(b) = a.get_bytes(digest) {
                let hex: String = b.iter().map(|byte| format!("{:02x}", byte)).collect();
                a.ok(a.string(&hex))
            } else if let Some(n) = a.get_int(digest) {
                let hex = format!("{:x}", n);
                a.ok(a.string(&hex))
            } else {
                a.ok(a.string(""))
            }
        }
        Err(e) => e,
    }
}

extern "C" fn prim_algorithms(_args: *const ElleValue, _nargs: usize) -> ElleResult {
    let a = api();
    let elems: Vec<ElleValue> = ALGORITHM_NAMES
        .iter()
        .map(|name| a.keyword(name))
        .collect();
    a.ok(a.set(&elems))
}

// ---------------------------------------------------------------------------
// Streaming API — HasherState enum + new / update / finalize
// ---------------------------------------------------------------------------

/// Macro to dispatch a method call across all HasherState variants.
macro_rules! dispatch {
    ($self:expr, $h:ident => $body:expr) => {
        match $self {
            HasherState::Md5($h) => $body,
            HasherState::Sha1($h) => $body,
            HasherState::Sha224($h) => $body,
            HasherState::Sha256($h) => $body,
            HasherState::Sha384($h) => $body,
            HasherState::Sha512($h) => $body,
            HasherState::Sha512_224($h) => $body,
            HasherState::Sha512_256($h) => $body,
            HasherState::Sha3_224($h) => $body,
            HasherState::Sha3_256($h) => $body,
            HasherState::Sha3_384($h) => $body,
            HasherState::Sha3_512($h) => $body,
            HasherState::Blake2b512($h) => $body,
            HasherState::Blake2s256($h) => $body,
            HasherState::Blake3($h) => $body,
            HasherState::Crc32($h) => $body,
            HasherState::Xxh32($h) => $body,
            HasherState::Xxh64($h) => $body,
            HasherState::Xxh3($h) => $body,
        }
    };
}

enum HasherState {
    Md5(Md5),
    Sha1(Sha1),
    Sha224(Sha224),
    Sha256(Sha256),
    Sha384(Sha384),
    Sha512(Sha512),
    Sha512_224(Sha512_224),
    Sha512_256(Sha512_256),
    Sha3_224(Sha3_224),
    Sha3_256(Sha3_256),
    Sha3_384(Sha3_384),
    Sha3_512(Sha3_512),
    Blake2b512(Blake2b512),
    Blake2s256(Blake2s256),
    Blake3(Box<blake3::Hasher>),
    Crc32(Crc32),
    Xxh32(xxhash_rust::xxh32::Xxh32),
    Xxh64(xxhash_rust::xxh64::Xxh64),
    Xxh3(xxhash_rust::xxh3::Xxh3Default),
}

impl HasherState {
    fn update(&mut self, data: &[u8]) {
        dispatch!(self, h => { h.update(data); });
    }

    /// Finalize and reset to a fresh hasher of the same algorithm.
    fn finalize_reset(&mut self, a: &elle_plugin::Api) -> ElleValue {
        // The 14 Digest-compatible types all share finalize_reset -> bytes.
        // BLAKE3, CRC32, and xxHash each need custom finalize + reset logic.
        match self {
            Self::Blake3(h) => {
                let r = h.finalize();
                h.reset();
                a.bytes(r.as_bytes())
            }
            Self::Crc32(h) => {
                let r = h.clone().finalize();
                h.reset();
                a.int(r as i64)
            }
            Self::Xxh32(h) => {
                let r = h.digest();
                *h = xxhash_rust::xxh32::Xxh32::new(0);
                a.int(r as i64)
            }
            Self::Xxh64(h) => {
                let r = h.digest();
                *h = xxhash_rust::xxh64::Xxh64::new(0);
                a.int(r as i64)
            }
            Self::Xxh3(h) => {
                let r = h.digest128();
                *h = xxhash_rust::xxh3::Xxh3Default::new();
                a.bytes(&r.to_be_bytes())
            }
            // All Digest types: finalize_reset returns GenericArray -> to_vec
            Self::Md5(h) => a.bytes(&h.finalize_reset().to_vec()),
            Self::Sha1(h) => a.bytes(&h.finalize_reset().to_vec()),
            Self::Sha224(h) => a.bytes(&h.finalize_reset().to_vec()),
            Self::Sha256(h) => a.bytes(&h.finalize_reset().to_vec()),
            Self::Sha384(h) => a.bytes(&h.finalize_reset().to_vec()),
            Self::Sha512(h) => a.bytes(&h.finalize_reset().to_vec()),
            Self::Sha512_224(h) => a.bytes(&h.finalize_reset().to_vec()),
            Self::Sha512_256(h) => a.bytes(&h.finalize_reset().to_vec()),
            Self::Sha3_224(h) => a.bytes(&h.finalize_reset().to_vec()),
            Self::Sha3_256(h) => a.bytes(&h.finalize_reset().to_vec()),
            Self::Sha3_384(h) => a.bytes(&h.finalize_reset().to_vec()),
            Self::Sha3_512(h) => a.bytes(&h.finalize_reset().to_vec()),
            Self::Blake2b512(h) => a.bytes(&h.finalize_reset().to_vec()),
            Self::Blake2s256(h) => a.bytes(&h.finalize_reset().to_vec()),
        }
    }
}

/// Create a hasher from an algorithm keyword string.
fn make_hasher(kw: &str) -> Result<HasherState, ElleResult> {
    match kw {
        "md5" => Ok(HasherState::Md5(Md5::new())),
        "sha1" => Ok(HasherState::Sha1(Sha1::new())),
        "sha224" => Ok(HasherState::Sha224(Sha224::new())),
        "sha256" => Ok(HasherState::Sha256(Sha256::new())),
        "sha384" => Ok(HasherState::Sha384(Sha384::new())),
        "sha512" => Ok(HasherState::Sha512(Sha512::new())),
        "sha512-224" => Ok(HasherState::Sha512_224(Sha512_224::new())),
        "sha512-256" => Ok(HasherState::Sha512_256(Sha512_256::new())),
        "sha3-224" => Ok(HasherState::Sha3_224(Sha3_224::new())),
        "sha3-256" => Ok(HasherState::Sha3_256(Sha3_256::new())),
        "sha3-384" => Ok(HasherState::Sha3_384(Sha3_384::new())),
        "sha3-512" => Ok(HasherState::Sha3_512(Sha3_512::new())),
        "blake2b-512" => Ok(HasherState::Blake2b512(Blake2b512::new())),
        "blake2s-256" => Ok(HasherState::Blake2s256(Blake2s256::new())),
        "blake3" => Ok(HasherState::Blake3(Box::new(blake3::Hasher::new()))),
        "crc32" => Ok(HasherState::Crc32(Crc32::new())),
        "xxh32" => Ok(HasherState::Xxh32(xxhash_rust::xxh32::Xxh32::new(0))),
        "xxh64" => Ok(HasherState::Xxh64(xxhash_rust::xxh64::Xxh64::new(0))),
        "xxh128" => Ok(HasherState::Xxh3(xxhash_rust::xxh3::Xxh3Default::new())),
        _ => {
            let a = api();
            Err(a.err(
                "value-error",
                &format!("hash/new: unknown algorithm :{}", kw),
            ))
        }
    }
}

extern "C" fn prim_hash_new(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    if nargs != 1 {
        return a.err(
            "arity-error",
            &format!("hash/new: expected 1 argument, got {}", nargs),
        );
    }
    let arg0 = a.arg(args, nargs, 0);
    let kw = match a.get_keyword_name(arg0) {
        Some(k) => k.to_string(),
        None => {
            return a.err(
                "type-error",
                &format!("hash/new: expected keyword, got {}", a.type_name(arg0)),
            );
        }
    };
    match make_hasher(&kw) {
        Ok(state) => a.ok(a.external("hash/context", RefCell::new(state))),
        Err(e) => e,
    }
}

extern "C" fn prim_hash_update(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    if nargs != 2 {
        return a.err(
            "arity-error",
            &format!("hash/update: expected 2 arguments, got {}", nargs),
        );
    }
    let arg0 = a.arg(args, nargs, 0);
    let cell = match a.get_external::<RefCell<HasherState>>(arg0, "hash/context") {
        Some(c) => c,
        None => {
            return a.err(
                "type-error",
                &format!(
                    "hash/update: first argument must be a hash context, got {}",
                    a.type_name(arg0),
                ),
            );
        }
    };
    let data = match extract_bytes(a.arg(args, nargs, 1), "hash/update", "data") {
        Ok(d) => d,
        Err(e) => return e,
    };
    cell.borrow_mut().update(&data);
    a.ok(arg0)
}

extern "C" fn prim_hash_finalize(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    if nargs != 1 {
        return a.err(
            "arity-error",
            &format!("hash/finalize: expected 1 argument, got {}", nargs),
        );
    }
    let arg0 = a.arg(args, nargs, 0);
    let cell = match a.get_external::<RefCell<HasherState>>(arg0, "hash/context") {
        Some(c) => c,
        None => {
            return a.err(
                "type-error",
                &format!(
                    "hash/finalize: expected a hash context, got {}",
                    a.type_name(arg0),
                ),
            );
        }
    };
    a.ok(cell.borrow_mut().finalize_reset(a))
}

// ---------------------------------------------------------------------------
// Registration table
// ---------------------------------------------------------------------------

static PRIMITIVES: &[EllePrimDef] = &[
    EllePrimDef::exact("hash/md5", prim_md5, SIG_OK, 1,
        "MD5 hash. Returns 16 bytes. Not cryptographically secure.", "hash",
        "(bytes->hex (hash/md5 \"hello\"))"),
    EllePrimDef::exact("hash/sha1", prim_sha1, SIG_OK, 1,
        "SHA-1 hash. Returns 20 bytes. Not cryptographically secure.", "hash",
        "(bytes->hex (hash/sha1 \"hello\"))"),
    EllePrimDef::exact("hash/sha224", prim_sha224, SIG_OK, 1,
        "SHA-224 hash. Returns 28 bytes.", "hash",
        "(bytes->hex (hash/sha224 \"hello\"))"),
    EllePrimDef::exact("hash/sha256", prim_sha256, SIG_OK, 1,
        "SHA-256 hash. Returns 32 bytes.", "hash",
        "(bytes->hex (hash/sha256 \"hello\"))"),
    EllePrimDef::exact("hash/sha384", prim_sha384, SIG_OK, 1,
        "SHA-384 hash. Returns 48 bytes.", "hash",
        "(bytes->hex (hash/sha384 \"hello\"))"),
    EllePrimDef::exact("hash/sha512", prim_sha512, SIG_OK, 1,
        "SHA-512 hash. Returns 64 bytes.", "hash",
        "(bytes->hex (hash/sha512 \"hello\"))"),
    EllePrimDef::exact("hash/sha512-224", prim_sha512_224, SIG_OK, 1,
        "SHA-512/224 hash. Returns 28 bytes.", "hash",
        "(bytes->hex (hash/sha512-224 \"hello\"))"),
    EllePrimDef::exact("hash/sha512-256", prim_sha512_256, SIG_OK, 1,
        "SHA-512/256 hash. Returns 32 bytes.", "hash",
        "(bytes->hex (hash/sha512-256 \"hello\"))"),
    EllePrimDef::exact("hash/sha3-224", prim_sha3_224, SIG_OK, 1,
        "SHA3-224 (Keccak). Returns 28 bytes.", "hash",
        "(bytes->hex (hash/sha3-224 \"hello\"))"),
    EllePrimDef::exact("hash/sha3-256", prim_sha3_256, SIG_OK, 1,
        "SHA3-256 (Keccak). Returns 32 bytes.", "hash",
        "(bytes->hex (hash/sha3-256 \"hello\"))"),
    EllePrimDef::exact("hash/sha3-384", prim_sha3_384, SIG_OK, 1,
        "SHA3-384 (Keccak). Returns 48 bytes.", "hash",
        "(bytes->hex (hash/sha3-384 \"hello\"))"),
    EllePrimDef::exact("hash/sha3-512", prim_sha3_512, SIG_OK, 1,
        "SHA3-512 (Keccak). Returns 64 bytes.", "hash",
        "(bytes->hex (hash/sha3-512 \"hello\"))"),
    EllePrimDef::exact("hash/blake2b-512", prim_blake2b512, SIG_OK, 1,
        "BLAKE2b-512 hash. Returns 64 bytes.", "hash",
        "(bytes->hex (hash/blake2b-512 \"hello\"))"),
    EllePrimDef::exact("hash/blake2s-256", prim_blake2s256, SIG_OK, 1,
        "BLAKE2s-256 hash. Returns 32 bytes.", "hash",
        "(bytes->hex (hash/blake2s-256 \"hello\"))"),
    EllePrimDef::exact("hash/blake3", prim_blake3, SIG_OK, 1,
        "BLAKE3 hash. Returns 32 bytes. Very fast.", "hash",
        "(bytes->hex (hash/blake3 \"hello\"))"),
    EllePrimDef::exact("hash/blake3-keyed", prim_blake3_keyed, SIG_ERROR, 2,
        "BLAKE3 keyed hash (MAC). Key must be exactly 32 bytes. Returns 32 bytes.", "hash",
        "(bytes->hex (hash/blake3-keyed (hash/blake3 \"mykey\") \"hello\"))"),
    EllePrimDef::exact("hash/blake3-derive", prim_blake3_derive, SIG_ERROR, 2,
        "BLAKE3 key derivation. Context string + input keying material. Returns 32 bytes.", "hash",
        "(bytes->hex (hash/blake3-derive \"myapp 2026\" \"secret\"))"),
    EllePrimDef::exact("hash/crc32", prim_crc32, SIG_OK, 1,
        "CRC32 checksum. Returns an integer.", "hash",
        "(hash/crc32 \"hello\")"),
    EllePrimDef::exact("hash/xxh32", prim_xxh32, SIG_OK, 1,
        "xxHash 32-bit. Returns an integer.", "hash",
        "(hash/xxh32 \"hello\")"),
    EllePrimDef::exact("hash/xxh64", prim_xxh64, SIG_OK, 1,
        "xxHash3 64-bit. Returns an integer.", "hash",
        "(hash/xxh64 \"hello\")"),
    EllePrimDef::exact("hash/xxh128", prim_xxh128, SIG_OK, 1,
        "xxHash3 128-bit. Returns 16 bytes.", "hash",
        "(bytes->hex (hash/xxh128 \"hello\"))"),
    EllePrimDef::exact("hash/hex", prim_hex, SIG_ERROR, 2,
        "Hash data and return hex string. (hash/hex :sha256 \"hello\")", "hash",
        "(hash/hex :sha256 \"hello\")"),
    EllePrimDef::exact("hash/algorithms", prim_algorithms, SIG_OK, 0,
        "Return the set of supported algorithm keywords.", "hash",
        "(hash/algorithms)"),
    EllePrimDef::exact("hash/new", prim_hash_new, SIG_ERROR, 1,
        "Create an incremental hasher. Algorithm keyword: :md5, :sha256, :blake3, etc.", "hash",
        "(hash/new :sha256)"),
    EllePrimDef::exact("hash/update", prim_hash_update, SIG_ERROR, 2,
        "Feed data into a hash context. Returns context for stream/fold chaining.", "hash",
        "(hash/update ctx \"hello\")"),
    EllePrimDef::exact("hash/finalize", prim_hash_finalize, SIG_ERROR, 1,
        "Finalize hash context, return digest. Resets context for reuse.", "hash",
        "(bytes->hex (hash/finalize ctx))"),
];
