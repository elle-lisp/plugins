//! Elle random plugin — pseudo-random and cryptographically secure random
//! number generation via the `rand` and `rand_chacha` crates.

use elle_plugin::{ElleResult, ElleValue, EllePrimDef, SIG_OK, SIG_ERROR};
use rand::seq::SliceRandom;
use rand::{rngs::StdRng, Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;
use std::f64::consts::PI;
use std::sync::{Mutex, OnceLock};

fn rng() -> &'static Mutex<StdRng> {
    static RNG: OnceLock<Mutex<StdRng>> = OnceLock::new();
    RNG.get_or_init(|| Mutex::new(StdRng::from_os_rng()))
}

fn csprng() -> &'static Mutex<ChaCha20Rng> {
    static CSPRNG: OnceLock<Mutex<ChaCha20Rng>> = OnceLock::new();
    CSPRNG.get_or_init(|| Mutex::new(ChaCha20Rng::from_os_rng()))
}
elle_plugin::define_plugin!("random/", &PRIMITIVES);

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract elements from an array value (immutable or mutable).
fn extract_elements(val: ElleValue) -> Option<Vec<ElleValue>> {
    let a = api();
    if let Some(len) = a.get_array_len(val) {
        let mut elems = Vec::with_capacity(len);
        for i in 0..len {
            elems.push(a.get_array_item(val, i));
        }
        return Some(elems);
    }
    None
}

/// Extract a float from a Value (float or int).
fn extract_float(val: ElleValue) -> Option<f64> {
    let a = api();
    if let Some(f) = a.get_float(val) {
        return Some(f);
    }
    if let Some(i) = a.get_int(val) {
        return Some(i as f64);
    }
    None
}

// ---------------------------------------------------------------------------
// Primitives
// ---------------------------------------------------------------------------

extern "C" fn prim_random_seed(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let arg0 = unsafe { a.arg(args, nargs, 0) };
    let seed = match a.get_int(arg0) {
        Some(n) => n as u64,
        None => {
            return a.err(
                "type-error",
                &format!("random/seed: expected integer, got {}", a.type_name(arg0)),
            );
        }
    };
    *rng().lock().unwrap() = StdRng::seed_from_u64(seed);
    a.ok(a.nil())
}

extern "C" fn prim_random_int(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let mut rng = rng().lock().unwrap();
    let val = match nargs {
        0 => rng.random::<u64>() as i64,
        1 => {
            let arg0 = unsafe { a.arg(args, nargs, 0) };
            let max = match a.get_int(arg0) {
                Some(n) => n,
                None => {
                    return a.err(
                        "type-error",
                        &format!("random/int: expected integer, got {}", a.type_name(arg0)),
                    );
                }
            };
            if max <= 0 {
                return a.err("range-error", "random/int: max must be positive");
            }
            rng.random_range(0..max)
        }
        2 => {
            let arg0 = unsafe { a.arg(args, nargs, 0) };
            let arg1 = unsafe { a.arg(args, nargs, 1) };
            let min = match a.get_int(arg0) {
                Some(n) => n,
                None => {
                    return a.err(
                        "type-error",
                        &format!("random/int: expected integer, got {}", a.type_name(arg0)),
                    );
                }
            };
            let max = match a.get_int(arg1) {
                Some(n) => n,
                None => {
                    return a.err(
                        "type-error",
                        &format!("random/int: expected integer, got {}", a.type_name(arg1)),
                    );
                }
            };
            if min >= max {
                return a.err("range-error", "random/int: min must be less than max");
            }
            rng.random_range(min..max)
        }
        _ => {
            return a.err(
                "arity-error",
                &format!("random/int: expected 0-2 arguments, got {}", nargs),
            );
        }
    };
    a.ok(a.int(val))
}

extern "C" fn prim_random_float(_args: *const ElleValue, _nargs: usize) -> ElleResult {
    let a = api();
    a.ok(a.float(rng().lock().unwrap().random::<f64>()))
}

extern "C" fn prim_random_bool(_args: *const ElleValue, _nargs: usize) -> ElleResult {
    let a = api();
    a.ok(a.boolean(rng().lock().unwrap().random::<bool>()))
}

extern "C" fn prim_random_bytes(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let arg0 = unsafe { a.arg(args, nargs, 0) };
    let len = match a.get_int(arg0) {
        Some(n) if n >= 0 => n as usize,
        Some(_) => {
            return a.err("range-error", "random/bytes: length must be non-negative");
        }
        None => {
            return a.err(
                "type-error",
                &format!("random/bytes: expected integer, got {}", a.type_name(arg0)),
            );
        }
    };
    let mut buf = vec![0u8; len];
    rng().lock().unwrap().fill(&mut buf[..]);
    a.ok(a.bytes(&buf))
}

extern "C" fn prim_random_shuffle(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let arg0 = unsafe { a.arg(args, nargs, 0) };
    let mut elements = match extract_elements(arg0) {
        Some(elems) => elems,
        None => {
            return a.err(
                "type-error",
                &format!(
                    "random/shuffle: expected array or list, got {}",
                    a.type_name(arg0),
                ),
            );
        }
    };
    elements.shuffle(&mut *rng().lock().unwrap());
    a.ok(a.array(&elements))
}

extern "C" fn prim_random_choice(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let arg0 = unsafe { a.arg(args, nargs, 0) };
    let elements = match extract_elements(arg0) {
        Some(elems) => elems,
        None => {
            return a.err(
                "type-error",
                &format!(
                    "random/choice: expected array or list, got {}",
                    a.type_name(arg0),
                ),
            );
        }
    };
    if elements.is_empty() {
        return a.err(
            "range-error",
            "random/choice: cannot choose from empty collection",
        );
    }
    let idx = rng().lock().unwrap().random_range(0..elements.len());
    a.ok(elements[idx])
}

extern "C" fn prim_random_normal(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let (mean, stddev) = match nargs {
        0 => (0.0f64, 1.0f64),
        1 => {
            let arg0 = unsafe { a.arg(args, nargs, 0) };
            let m = match extract_float(arg0) {
                Some(f) => f,
                None => {
                    return a.err(
                        "type-error",
                        &format!(
                            "random/normal: expected number for mean, got {}",
                            a.type_name(arg0),
                        ),
                    );
                }
            };
            (m, 1.0f64)
        }
        2 => {
            let arg0 = unsafe { a.arg(args, nargs, 0) };
            let arg1 = unsafe { a.arg(args, nargs, 1) };
            let m = match extract_float(arg0) {
                Some(f) => f,
                None => {
                    return a.err(
                        "type-error",
                        &format!(
                            "random/normal: expected number for mean, got {}",
                            a.type_name(arg0),
                        ),
                    );
                }
            };
            let s = match extract_float(arg1) {
                Some(f) => f,
                None => {
                    return a.err(
                        "type-error",
                        &format!(
                            "random/normal: expected number for stddev, got {}",
                            a.type_name(arg1),
                        ),
                    );
                }
            };
            (m, s)
        }
        _ => unreachable!("arity enforced by PRIMITIVES table"),
    };
    if stddev <= 0.0 {
        return a.err("range-error", "random/normal: stddev must be positive");
    }
    // Box-Muller transform
    let sample = {
        let mut r = rng().lock().unwrap();
        loop {
            let u1 = r.random::<f64>();
            let u2 = r.random::<f64>();
            if u1 > 0.0 {
                break mean + stddev * (-2.0 * u1.ln()).sqrt() * (2.0 * PI * u2).cos();
            }
        }
    };
    a.ok(a.float(sample))
}

extern "C" fn prim_random_exponential(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let lambda = match nargs {
        0 => 1.0f64,
        1 => {
            let arg0 = unsafe { a.arg(args, nargs, 0) };
            match extract_float(arg0) {
                Some(f) => f,
                None => {
                    return a.err(
                        "type-error",
                        &format!(
                            "random/exponential: expected number for lambda, got {}",
                            a.type_name(arg0),
                        ),
                    );
                }
            }
        }
        _ => unreachable!("arity enforced by PRIMITIVES table"),
    };
    if lambda <= 0.0 {
        return a.err("range-error", "random/exponential: lambda must be positive");
    }
    let u: f64 = rng().lock().unwrap().random::<f64>();
    let sample = -(1.0 - u).ln() / lambda;
    a.ok(a.float(sample))
}

extern "C" fn prim_random_weighted(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let arg0 = unsafe { a.arg(args, nargs, 0) };
    let arg1 = unsafe { a.arg(args, nargs, 1) };
    let items = match extract_elements(arg0) {
        Some(elems) => elems,
        None => {
            return a.err(
                "type-error",
                &format!(
                    "random/weighted: expected array or list for items, got {}",
                    a.type_name(arg0),
                ),
            );
        }
    };
    let weight_vals = match extract_elements(arg1) {
        Some(elems) => elems,
        None => {
            return a.err(
                "type-error",
                &format!(
                    "random/weighted: expected array or list for weights, got {}",
                    a.type_name(arg1),
                ),
            );
        }
    };
    if items.is_empty() {
        return a.err("range-error", "random/weighted: items must not be empty");
    }
    if items.len() != weight_vals.len() {
        return a.err(
            "range-error",
            &format!(
                "random/weighted: items and weights must have equal length, got {} and {}",
                items.len(),
                weight_vals.len(),
            ),
        );
    }
    // Extract and validate weights
    let mut weights = Vec::with_capacity(weight_vals.len());
    for (i, wv) in weight_vals.iter().enumerate() {
        let w = match extract_float(*wv) {
            Some(f) => f,
            None => {
                return a.err(
                    "type-error",
                    &format!(
                        "random/weighted: weight {} must be a number, got {}",
                        i,
                        a.type_name(*wv),
                    ),
                );
            }
        };
        if w <= 0.0 {
            return a.err(
                "range-error",
                &format!("random/weighted: weight {} must be positive, got {}", i, w),
            );
        }
        weights.push(w);
    }
    // Prefix-sum cumulative distribution
    let mut cumsum = Vec::with_capacity(weights.len());
    let mut total = 0.0f64;
    for w in &weights {
        total += w;
        cumsum.push(total);
    }
    let pick = rng().lock().unwrap().random_range(0.0..total);
    let idx = cumsum.partition_point(|&c| c <= pick);
    let idx = idx.min(items.len() - 1);
    a.ok(items[idx])
}

extern "C" fn prim_random_csprng_bytes(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let arg0 = unsafe { a.arg(args, nargs, 0) };
    let len = match a.get_int(arg0) {
        Some(n) if n >= 0 => n as usize,
        Some(_) => {
            return a.err(
                "range-error",
                "random/csprng-bytes: length must be non-negative",
            );
        }
        None => {
            return a.err(
                "type-error",
                &format!(
                    "random/csprng-bytes: expected integer, got {}",
                    a.type_name(arg0),
                ),
            );
        }
    };
    let mut buf = vec![0u8; len];
    csprng().lock().unwrap().fill(&mut buf[..]);
    a.ok(a.bytes(&buf))
}

extern "C" fn prim_random_csprng_seed(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let arg0 = unsafe { a.arg(args, nargs, 0) };
    // Extract bytes from bytes value
    let data: Vec<u8> = if let Some(b) = a.get_bytes(arg0) {
        b.to_vec()
    } else {
        return a.err(
            "type-error",
            &format!(
                "random/csprng-seed: expected bytes, got {}",
                a.type_name(arg0),
            ),
        );
    };
    if data.len() != 32 {
        return a.err(
            "range-error",
            &format!(
                "random/csprng-seed: expected exactly 32 bytes, got {}",
                data.len(),
            ),
        );
    }
    let mut seed = [0u8; 32];
    seed.copy_from_slice(&data);
    *csprng().lock().unwrap() = ChaCha20Rng::from_seed(seed);
    a.ok(a.nil())
}

extern "C" fn prim_random_sample(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let arg0 = unsafe { a.arg(args, nargs, 0) };
    let arg1 = unsafe { a.arg(args, nargs, 1) };
    let elements = match extract_elements(arg0) {
        Some(elems) => elems,
        None => {
            return a.err(
                "type-error",
                &format!(
                    "random/sample: expected array or list, got {}",
                    a.type_name(arg0),
                ),
            );
        }
    };
    let n = match a.get_int(arg1) {
        Some(i) => i,
        None => {
            return a.err(
                "type-error",
                &format!(
                    "random/sample: expected integer for n, got {}",
                    a.type_name(arg1),
                ),
            );
        }
    };
    if n < 0 || n as usize > elements.len() {
        return a.err(
            "range-error",
            &format!(
                "random/sample: n must be between 0 and {} (collection length), got {}",
                elements.len(),
                n,
            ),
        );
    }
    let n = n as usize;
    // Partial Fisher-Yates: shuffle first n elements from a clone
    let mut pool = elements;
    {
        let mut r = rng().lock().unwrap();
        for i in 0..n {
            let j = r.random_range(i..pool.len());
            pool.swap(i, j);
        }
    }
    pool.truncate(n);
    a.ok(a.array(&pool))
}

// ---------------------------------------------------------------------------
// Registration table
// ---------------------------------------------------------------------------

static PRIMITIVES: &[EllePrimDef] = &[
    EllePrimDef::exact("random/seed", prim_random_seed, SIG_ERROR, 1,
        "Seed the PRNG for deterministic output", "random",
        "(random/seed 42)"),
    EllePrimDef::range("random/int", prim_random_int, SIG_ERROR, 0, 2,
        "Random integer. No args: full range. One arg: 0..n. Two args: min..max.", "random",
        "(random/int 100)"),
    EllePrimDef::exact("random/float", prim_random_float, SIG_OK, 0,
        "Random float in [0, 1)", "random",
        "(random/float)"),
    EllePrimDef::exact("random/bool", prim_random_bool, SIG_OK, 0,
        "Random boolean", "random",
        "(random/bool)"),
    EllePrimDef::exact("random/bytes", prim_random_bytes, SIG_ERROR, 1,
        "Generate a byte vector of the given length filled with random bytes", "random",
        "(random/bytes 16)"),
    EllePrimDef::exact("random/shuffle", prim_random_shuffle, SIG_ERROR, 1,
        "Return a new @array with elements shuffled randomly", "random",
        "(random/shuffle [1 2 3 4 5])"),
    EllePrimDef::exact("random/choice", prim_random_choice, SIG_ERROR, 1,
        "Return a random element from an array or list", "random",
        "(random/choice [1 2 3 4 5])"),
    EllePrimDef::range("random/normal", prim_random_normal, SIG_ERROR, 0, 2,
        "Sample from a normal distribution. 0 args: mean=0 stddev=1. 1 arg: mean=arg stddev=1. 2 args: mean and stddev.", "random",
        "(random/normal 0.0 1.0)"),
    EllePrimDef::range("random/exponential", prim_random_exponential, SIG_ERROR, 0, 1,
        "Sample from an exponential distribution. 0 args: lambda=1. 1 arg: lambda=arg.", "random",
        "(random/exponential 2.0)"),
    EllePrimDef::exact("random/weighted", prim_random_weighted, SIG_ERROR, 2,
        "Choose a random item from a collection according to corresponding weights", "random",
        "(random/weighted [\"a\" \"b\" \"c\"] [1.0 2.0 3.0])"),
    EllePrimDef::exact("random/csprng-bytes", prim_random_csprng_bytes, SIG_ERROR, 1,
        "Generate cryptographically secure random bytes of the given length", "random",
        "(random/csprng-bytes 32)"),
    EllePrimDef::exact("random/csprng-seed", prim_random_csprng_seed, SIG_ERROR, 1,
        "Seed the CSPRNG with exactly 32 bytes for deterministic output", "random",
        "(random/csprng-seed (bytes 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0))"),
    EllePrimDef::exact("random/sample", prim_random_sample, SIG_ERROR, 2,
        "Return n randomly selected elements from a collection (no replacement)", "random",
        "(random/sample [1 2 3 4 5] 3)"),
];
