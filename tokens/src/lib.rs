//! Elle tokens plugin — a text counted, encoded and decoded in the BPE encodings of `tiktoken-rs`.
//!
//! tokens/README.md

use elle_plugin::{ElleCtx, EllePrimDef, ElleResult, ElleValue, SIG_ERROR, SIG_OK};
use tiktoken_rs::{CoreBPE, Rank};

elle_plugin::define_plugin!("tokens/", &PRIMITIVES);

const COUNT: &str = "tokens/count";
const ENCODE: &str = "tokens/encode";
const DECODE: &str = "tokens/decode";

/// A function that builds an encoding on its first call and answers the
/// same encoding on every later one.
type Singleton = fn() -> &'static CoreBPE;

/// Each encoding's keyword, spelled as `tiktoken` spells it, and its singleton.
const ENCODINGS: &[(&str, Singleton)] = &[
    ("o200k_base", tiktoken_rs::o200k_base_singleton),
    ("o200k_harmony", tiktoken_rs::o200k_harmony_singleton),
    ("cl100k_base", tiktoken_rs::cl100k_base_singleton),
    ("p50k_base", tiktoken_rs::p50k_base_singleton),
    ("p50k_edit", tiktoken_rs::p50k_edit_singleton),
    ("r50k_base", tiktoken_rs::r50k_base_singleton),
];

/// An error result of `kind`, its message prefixed with the primitive's name.
fn fail(ctx: *mut ElleCtx, kind: &str, name: &str, msg: &str) -> ElleResult {
    api().err(ctx, kind, &format!("{}: {}", name, msg))
}

/// The arguments of a primitive that takes a value and an optional
/// encoding. The host checks the arity a primitive declares, and this
/// checks it again, because a wrong count here would index past `args`.
fn value_and_encoding(
    ctx: *mut ElleCtx,
    name: &str,
    args: *const ElleValue,
    nargs: usize,
) -> Result<(ElleValue, &'static CoreBPE), ElleResult> {
    let a = api();
    let args = unsafe { a.args(args, nargs) };
    match args {
        [value] => Ok((*value, tiktoken_rs::o200k_base_singleton())),
        [value, encoding] => Ok((*value, encoding_named(ctx, name, *encoding)?)),
        _ => Err(fail(
            ctx,
            "arity-error",
            name,
            &format!("expected 1 or 2 arguments, got {}", args.len()),
        )),
    }
}

/// The encoding a keyword names. A keyword that names none is an error,
/// so a misspelled encoding never counts in the default one.
fn encoding_named(
    ctx: *mut ElleCtx,
    name: &str,
    v: ElleValue,
) -> Result<&'static CoreBPE, ElleResult> {
    let a = api();
    let Some(key) = a.get_keyword_name(ctx, v) else {
        return Err(fail(
            ctx,
            "type-error",
            name,
            &format!("expected an encoding keyword, got {}", a.type_name(v)),
        ));
    };
    match ENCODINGS.iter().find(|(k, _)| *k == key) {
        Some((_, build)) => Ok(build()),
        None => Err(fail(
            ctx,
            "value-error",
            name,
            &format!("no encoding is named :{}", key),
        )),
    }
}

fn text<'a>(ctx: *mut ElleCtx, name: &str, v: ElleValue) -> Result<&'a str, ElleResult> {
    let a = api();
    a.get_string(v).ok_or_else(|| {
        fail(
            ctx,
            "type-error",
            name,
            &format!("expected a string, got {}", a.type_name(v)),
        )
    })
}

/// The ids of an immutable array or a list. An integer outside the range
/// of a rank names no token, so it is a value-error, as an unknown rank is.
fn ids(ctx: *mut ElleCtx, v: ElleValue) -> Result<Vec<Rank>, ElleResult> {
    let a = api();
    let not_ids = || {
        fail(
            ctx,
            "type-error",
            DECODE,
            &format!("expected an array or a list of ids, got {}", a.type_name(v)),
        )
    };
    // nil is no list in Elle, so it is refused before the list conversion.
    if a.check_nil(v) {
        return Err(not_ids());
    }
    let array = match a.get_array_len(v) {
        Some(_) => v,
        None => a.list_to_array(ctx, v).ok_or_else(not_ids)?,
    };
    let len = a.get_array_len(array).ok_or_else(not_ids)?;
    (0..len)
        .map(|i| {
            let item = a.get_array_item(array, i);
            let Some(n) = a.get_int(item) else {
                return Err(fail(
                    ctx,
                    "type-error",
                    DECODE,
                    &format!("expected an integer id, got {}", a.type_name(item)),
                ));
            };
            Rank::try_from(n).map_err(|_| {
                fail(
                    ctx,
                    "value-error",
                    DECODE,
                    &format!("no token has the id {}", n),
                )
            })
        })
        .collect()
}

extern "C" fn prim_count(ctx: *mut ElleCtx, args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let (value, bpe) = match value_and_encoding(ctx, COUNT, args, nargs) {
        Ok(v) => v,
        Err(e) => return e,
    };
    match text(ctx, COUNT, value) {
        Ok(s) => a.ok(a.int(bpe.count_ordinary(s) as i64)),
        Err(e) => e,
    }
}

extern "C" fn prim_encode(ctx: *mut ElleCtx, args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let (value, bpe) = match value_and_encoding(ctx, ENCODE, args, nargs) {
        Ok(v) => v,
        Err(e) => return e,
    };
    let s = match text(ctx, ENCODE, value) {
        Ok(s) => s,
        Err(e) => return e,
    };
    let ids: Vec<ElleValue> = bpe
        .encode_ordinary(s)
        .into_iter()
        .map(|id| a.int(i64::from(id)))
        .collect();
    a.ok(a.array(ctx, &ids))
}

extern "C" fn prim_decode(ctx: *mut ElleCtx, args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let (value, bpe) = match value_and_encoding(ctx, DECODE, args, nargs) {
        Ok(v) => v,
        Err(e) => return e,
    };
    let ids = match ids(ctx, value) {
        Ok(ids) => ids,
        Err(e) => return e,
    };
    let bytes = match bpe.decode_bytes(&ids) {
        Ok(b) => b,
        Err(e) => {
            return fail(
                ctx,
                "value-error",
                DECODE,
                &format!("no token has the id {}", e.token),
            )
        }
    };
    match String::from_utf8(bytes) {
        Ok(s) => a.ok(a.string(ctx, &s)),
        Err(_) => fail(
            ctx,
            "value-error",
            DECODE,
            "the ids decode to bytes that are not UTF-8, as when they cut a character in two",
        ),
    }
}

extern "C" fn prim_encodings(
    ctx: *mut ElleCtx,
    _args: *const ElleValue,
    _nargs: usize,
) -> ElleResult {
    let a = api();
    let names: Vec<ElleValue> = ENCODINGS.iter().map(|(k, _)| a.keyword(ctx, k)).collect();
    a.ok(a.set(ctx, &names))
}

static PRIMITIVES: &[EllePrimDef] = &[
    EllePrimDef::range(
        COUNT,
        prim_count,
        SIG_ERROR,
        1,
        2,
        "Count the tokens a string encodes to. The optional keyword names the encoding, :o200k_base by default. The spelling of a special token counts as the text it is.",
        "tokens",
        r#"(tokens/count "hello world")"#,
    ),
    EllePrimDef::range(
        ENCODE,
        prim_encode,
        SIG_ERROR,
        1,
        2,
        "Encode a string to an array of token ids. The optional keyword names the encoding, :o200k_base by default. The spelling of a special token encodes as the text it is.",
        "tokens",
        r#"(tokens/encode "hello world" :cl100k_base)"#,
    ),
    EllePrimDef::range(
        DECODE,
        prim_decode,
        SIG_ERROR,
        1,
        2,
        "Decode an array or a list of token ids to a string. The optional keyword names the encoding, :o200k_base by default. Ids that name no token, or whose bytes are not UTF-8, are a value-error.",
        "tokens",
        "(tokens/decode [24912 2375])",
    ),
    EllePrimDef::exact(
        "tokens/encodings",
        prim_encodings,
        SIG_OK,
        0,
        "Return the set of encoding keywords that count, encode and decode take.",
        "tokens",
        "(tokens/encodings)",
    ),
];
