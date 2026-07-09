//! Elle regex plugin — regular expression support via the `regex` crate.

use elle_plugin::{ElleCtx, ElleResult, ElleValue, EllePrimDef, SIG_ERROR};
use regex::Regex;
elle_plugin::define_plugin!("regex/", &PRIMITIVES);

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn require_arity(ctx: *mut ElleCtx, name: &str, nargs: usize, expected: usize) -> Result<(), ElleResult> {
    if nargs != expected {
        let a = api();
        Err(a.err(ctx, 
            "arity-error",
            &format!(
                "{}: expected {} argument{}, got {}",
                name,
                expected,
                if expected == 1 { "" } else { "s" },
                nargs,
            ),
        ))
    } else {
        Ok(())
    }
}

fn require_regex<'a>(ctx: *mut ElleCtx, name: &str, v: ElleValue) -> Result<&'a Regex, ElleResult> {
    let a = api();
    a.get_external::<Regex>(v, "regex").ok_or_else(|| {
        a.err(ctx, 
            "type-error",
            &format!("{}: expected regex, got {}", name, a.type_name(v)),
        )
    })
}

fn require_string(ctx: *mut ElleCtx, name: &str, v: ElleValue) -> Result<String, ElleResult> {
    let a = api();
    a.get_string(v)
        .map(|s| s.to_string())
        .ok_or_else(|| {
            a.err(ctx, 
                "type-error",
                &format!("{}: expected string, got {}", name, a.type_name(v)),
            )
        })
}

fn match_struct(ctx: *mut ElleCtx, m: regex::Match<'_>) -> ElleValue {
    let a = api();
    a.build_struct(ctx, &[
        ("match", a.string(ctx, m.as_str())),
        ("start", a.int(m.start() as i64)),
        ("end", a.int(m.end() as i64)),
    ])
}

fn captures_struct(ctx: *mut ElleCtx, re: &Regex, caps: &regex::Captures<'_>) -> ElleValue {
    let a = api();
    let mut fields: Vec<(&str, ElleValue)> = Vec::new();
    // Numbered captures — we need owned strings for the keys, but build_struct
    // takes &str with 'static-ish lifetime. Use a vec of owned strings and
    // reference them.
    let mut numbered_keys: Vec<String> = Vec::new();
    let mut numbered_vals: Vec<ElleValue> = Vec::new();
    for (i, m) in caps.iter().enumerate() {
        if let Some(m) = m {
            numbered_keys.push(format!("{}", i));
            numbered_vals.push(a.string(ctx, m.as_str()));
        }
    }
    // Named captures
    let mut named_keys: Vec<String> = Vec::new();
    let mut named_vals: Vec<ElleValue> = Vec::new();
    for name in re.capture_names().flatten() {
        if let Some(m) = caps.name(name) {
            named_keys.push(name.to_string());
            named_vals.push(a.string(ctx, m.as_str()));
        }
    }
    // Build field tuples referencing the owned strings
    for (k, v) in numbered_keys.iter().zip(numbered_vals.iter()) {
        fields.push((k.as_str(), *v));
    }
    for (k, v) in named_keys.iter().zip(named_vals.iter()) {
        fields.push((k.as_str(), *v));
    }
    a.build_struct(ctx, &fields)
}

// ---------------------------------------------------------------------------
// Primitives
// ---------------------------------------------------------------------------

extern "C" fn prim_regex_compile(ctx: *mut ElleCtx, args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    if let Err(e) = require_arity(ctx, "regex/compile", nargs, 1) {
        return e;
    }
    let pattern = match require_string(ctx, "regex/compile", unsafe { a.arg(args, nargs, 0) }) {
        Ok(s) => s,
        Err(e) => return e,
    };
    match Regex::new(&pattern) {
        Ok(re) => a.ok(a.external(ctx, "regex", re)),
        Err(e) => a.err(ctx, "regex-error", &format!("regex/compile: {}", e)),
    }
}

extern "C" fn prim_regex_match(ctx: *mut ElleCtx, args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    if let Err(e) = require_arity(ctx, "regex/match?", nargs, 2) {
        return e;
    }
    let re = match require_regex(ctx, "regex/match?", unsafe { a.arg(args, nargs, 0) }) {
        Ok(r) => r,
        Err(e) => return e,
    };
    let text = match require_string(ctx, "regex/match?", unsafe { a.arg(args, nargs, 1) }) {
        Ok(s) => s,
        Err(e) => return e,
    };
    a.ok(a.boolean(re.is_match(&text)))
}

extern "C" fn prim_regex_find(ctx: *mut ElleCtx, args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    if let Err(e) = require_arity(ctx, "regex/find", nargs, 2) {
        return e;
    }
    let re = match require_regex(ctx, "regex/find", unsafe { a.arg(args, nargs, 0) }) {
        Ok(r) => r,
        Err(e) => return e,
    };
    let text = match require_string(ctx, "regex/find", unsafe { a.arg(args, nargs, 1) }) {
        Ok(s) => s,
        Err(e) => return e,
    };
    match re.find(&text) {
        Some(m) => a.ok(match_struct(ctx, m)),
        None => a.ok(a.nil()),
    }
}

extern "C" fn prim_regex_find_all(ctx: *mut ElleCtx, args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    if let Err(e) = require_arity(ctx, "regex/find-all", nargs, 2) {
        return e;
    }
    let re = match require_regex(ctx, "regex/find-all", unsafe { a.arg(args, nargs, 0) }) {
        Ok(r) => r,
        Err(e) => return e,
    };
    let text = match require_string(ctx, "regex/find-all", unsafe { a.arg(args, nargs, 1) }) {
        Ok(s) => s,
        Err(e) => return e,
    };
    let matches: Vec<ElleValue> = re.find_iter(&text).map(|m| match_struct(ctx, m)).collect();
    a.ok(a.array(ctx, &matches))
}

extern "C" fn prim_regex_captures(ctx: *mut ElleCtx, args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    if let Err(e) = require_arity(ctx, "regex/captures", nargs, 2) {
        return e;
    }
    let re = match require_regex(ctx, "regex/captures", unsafe { a.arg(args, nargs, 0) }) {
        Ok(r) => r,
        Err(e) => return e,
    };
    let text = match require_string(ctx, "regex/captures", unsafe { a.arg(args, nargs, 1) }) {
        Ok(s) => s,
        Err(e) => return e,
    };
    match re.captures(&text) {
        Some(caps) => a.ok(captures_struct(ctx, re, &caps)),
        None => a.ok(a.nil()),
    }
}

extern "C" fn prim_regex_captures_all(ctx: *mut ElleCtx, args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    if let Err(e) = require_arity(ctx, "regex/captures-all", nargs, 2) {
        return e;
    }
    let re = match require_regex(ctx, "regex/captures-all", unsafe { a.arg(args, nargs, 0) }) {
        Ok(r) => r,
        Err(e) => return e,
    };
    let text = match require_string(ctx, "regex/captures-all", unsafe { a.arg(args, nargs, 1) }) {
        Ok(s) => s,
        Err(e) => return e,
    };
    let results: Vec<ElleValue> = re
        .captures_iter(&text)
        .map(|caps| captures_struct(ctx, re, &caps))
        .collect();
    a.ok(a.array(ctx, &results))
}

extern "C" fn prim_regex_replace(ctx: *mut ElleCtx, args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    if let Err(e) = require_arity(ctx, "regex/replace", nargs, 3) {
        return e;
    }
    let re = match require_regex(ctx, "regex/replace", unsafe { a.arg(args, nargs, 0) }) {
        Ok(r) => r,
        Err(e) => return e,
    };
    let text = match require_string(ctx, "regex/replace", unsafe { a.arg(args, nargs, 1) }) {
        Ok(s) => s,
        Err(e) => return e,
    };
    let replacement = match require_string(ctx, "regex/replace", unsafe { a.arg(args, nargs, 2) }) {
        Ok(s) => s,
        Err(e) => return e,
    };
    let result = re.replace(&text, replacement.as_str());
    a.ok(a.string(ctx, &result))
}

extern "C" fn prim_regex_replace_all(ctx: *mut ElleCtx, args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    if let Err(e) = require_arity(ctx, "regex/replace-all", nargs, 3) {
        return e;
    }
    let re = match require_regex(ctx, "regex/replace-all", unsafe { a.arg(args, nargs, 0) }) {
        Ok(r) => r,
        Err(e) => return e,
    };
    let text = match require_string(ctx, "regex/replace-all", unsafe { a.arg(args, nargs, 1) }) {
        Ok(s) => s,
        Err(e) => return e,
    };
    let replacement = match require_string(ctx, "regex/replace-all", unsafe { a.arg(args, nargs, 2) }) {
        Ok(s) => s,
        Err(e) => return e,
    };
    let result = re.replace_all(&text, replacement.as_str());
    a.ok(a.string(ctx, &result))
}

extern "C" fn prim_regex_split(ctx: *mut ElleCtx, args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    if let Err(e) = require_arity(ctx, "regex/split", nargs, 2) {
        return e;
    }
    let re = match require_regex(ctx, "regex/split", unsafe { a.arg(args, nargs, 0) }) {
        Ok(r) => r,
        Err(e) => return e,
    };
    let text = match require_string(ctx, "regex/split", unsafe { a.arg(args, nargs, 1) }) {
        Ok(s) => s,
        Err(e) => return e,
    };
    let parts: Vec<ElleValue> = re.split(&text).map(|s| a.string(ctx, s)).collect();
    a.ok(a.array(ctx, &parts))
}

// ---------------------------------------------------------------------------
// Registration table
// ---------------------------------------------------------------------------

static PRIMITIVES: &[EllePrimDef] = &[
    EllePrimDef::exact("regex/compile", prim_regex_compile, SIG_ERROR, 1,
        "Compile a regular expression pattern", "regex",
        r#"(regex/compile "\\d+")"#),
    EllePrimDef::exact("regex/match?", prim_regex_match, SIG_ERROR, 2,
        "Test if a regex matches a string", "regex",
        r#"(regex/match? (regex/compile "\\d+") "abc123")"#),
    EllePrimDef::exact("regex/find", prim_regex_find, SIG_ERROR, 2,
        "Find the first match in a string. Returns a struct with :match, :start, :end or nil.", "regex",
        r#"(regex/find (regex/compile "\\d+") "abc123def")"#),
    EllePrimDef::exact("regex/find-all", prim_regex_find_all, SIG_ERROR, 2,
        "Find all matches in a string. Returns a list of match structs.", "regex",
        r#"(regex/find-all (regex/compile "\\d+") "a1b2c3")"#),
    EllePrimDef::exact("regex/captures", prim_regex_captures, SIG_ERROR, 2,
        "Capture groups from first match. Returns a struct with numbered and named groups, or nil.", "regex",
        r#"(regex/captures (regex/compile "(?P<year>\\d{4})-(?P<month>\\d{2})") "2024-01-15")"#),
    EllePrimDef::exact("regex/captures-all", prim_regex_captures_all, SIG_ERROR, 2,
        "Capture groups from all matches. Returns a list of capture structs.", "regex",
        r#"(regex/captures-all (regex/compile "(\\d+)-(\\w+)") "1-a 2-b 3-c")"#),
    EllePrimDef::exact("regex/replace", prim_regex_replace, SIG_ERROR, 3,
        "Replace the first match in text. Supports $1, $name backreferences in replacement.", "regex",
        r#"(regex/replace (regex/compile "\\d+") "a1b2" "N")"#),
    EllePrimDef::exact("regex/replace-all", prim_regex_replace_all, SIG_ERROR, 3,
        "Replace all matches in text. Supports $1, $name backreferences in replacement.", "regex",
        r#"(regex/replace-all (regex/compile "\\d+") "a1b2" "N")"#),
    EllePrimDef::exact("regex/split", prim_regex_split, SIG_ERROR, 2,
        "Split a string by regex pattern. Returns a list of strings.", "regex",
        r#"(regex/split (regex/compile "[,;\\s]+") "a,b; c  d")"#),
];
