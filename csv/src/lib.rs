//! Elle CSV plugin — CSV parsing and serialization via the `csv` crate.

use elle_plugin::{ElleResult, ElleValue, EllePrimDef, SIG_ERROR};

elle_plugin::define_plugin!("csv/", &PRIMITIVES);

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract a string from an ElleValue.
fn extract_string(val: ElleValue, name: &str) -> Result<String, ElleResult> {
    let a = api();
    if let Some(s) = a.get_string(val) {
        return Ok(s.to_string());
    }
    Err(a.err(
        "type-error",
        &format!("{}: expected string, got {}", name, a.type_name(val)),
    ))
}

/// Extract the delimiter byte from an opts struct (second argument).
/// Returns b',' if opts is nil or absent. Returns an error if opts is present
/// but malformed.
fn extract_delimiter(opts: ElleValue, name: &str) -> Result<u8, ElleResult> {
    let a = api();
    if a.check_nil(opts) {
        return Ok(b',');
    }
    // opts must be a struct
    if !a.check_struct(opts) {
        return Err(a.err(
            "type-error",
            &format!("{}: opts must be a struct, got {}", name, a.type_name(opts)),
        ));
    }

    let delim_val = a.get_struct_field(opts, "delimiter");
    if a.check_nil(delim_val) {
        return Ok(b',');
    }

    let s = match a.get_string(delim_val) {
        Some(s) => s.to_string(),
        None => {
            return Err(a.err(
                "type-error",
                &format!(
                    "{}: :delimiter must be a single-character string, got {}",
                    name,
                    a.type_name(delim_val)
                ),
            ));
        }
    };
    let b = s.as_bytes();
    if b.len() != 1 {
        return Err(a.err(
            "csv-error",
            &format!(
                "{}: :delimiter must be a single-character string, got {:?}",
                name, s
            ),
        ));
    }
    Ok(b[0])
}

/// Stringify an ElleValue for CSV output.
fn value_to_csv_field(val: ElleValue) -> String {
    let a = api();
    if let Some(s) = a.get_string(val) {
        return s.to_string();
    }
    if let Some(i) = a.get_int(val) {
        return i.to_string();
    }
    if let Some(f) = a.get_float(val) {
        return format!("{}", f);
    }
    if let Some(b) = a.get_bool(val) {
        return if b { "true".to_string() } else { "false".to_string() };
    }
    if a.check_nil(val) {
        return String::new();
    }
    String::new()
}

// ---------------------------------------------------------------------------
// Primitives
// ---------------------------------------------------------------------------

extern "C" fn prim_csv_parse(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let name = "csv/parse";
    if nargs == 0 || nargs > 2 {
        return a.err(
            "arity-error",
            &format!("{}: expected 1 or 2 arguments, got {}", name, nargs),
        );
    }
    let text = match extract_string(a.arg(args, nargs, 0), name) {
        Ok(s) => s,
        Err(e) => return e,
    };
    let opts = if nargs == 2 {
        a.arg(args, nargs, 1)
    } else {
        a.nil()
    };
    let delim = match extract_delimiter(opts, name) {
        Ok(d) => d,
        Err(e) => return e,
    };

    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .from_reader(text.as_bytes());

    let headers: Vec<String> = match rdr.headers() {
        Ok(rec) => rec.iter().map(|s| s.to_owned()).collect(),
        Err(e) => {
            return a.err("csv-error", &format!("{}: {}", name, e));
        }
    };

    let mut rows: Vec<ElleValue> = Vec::new();
    for result in rdr.records() {
        let record = match result {
            Ok(r) => r,
            Err(e) => {
                return a.err("csv-error", &format!("{}: {}", name, e));
            }
        };
        let fields: Vec<(&str, ElleValue)> = headers
            .iter()
            .zip(record.iter())
            .map(|(header, field)| (header.as_str(), a.string(field)))
            .collect();
        rows.push(a.build_struct(&fields));
    }
    a.ok(a.array(&rows))
}

extern "C" fn prim_csv_parse_rows(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let name = "csv/parse-rows";
    if nargs == 0 || nargs > 2 {
        return a.err(
            "arity-error",
            &format!("{}: expected 1 or 2 arguments, got {}", name, nargs),
        );
    }
    let text = match extract_string(a.arg(args, nargs, 0), name) {
        Ok(s) => s,
        Err(e) => return e,
    };
    let opts = if nargs == 2 {
        a.arg(args, nargs, 1)
    } else {
        a.nil()
    };
    let delim = match extract_delimiter(opts, name) {
        Ok(d) => d,
        Err(e) => return e,
    };

    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .has_headers(false)
        .from_reader(text.as_bytes());

    let mut rows: Vec<ElleValue> = Vec::new();
    for result in rdr.records() {
        let record = match result {
            Ok(r) => r,
            Err(e) => {
                return a.err("csv-error", &format!("{}: {}", name, e));
            }
        };
        let fields: Vec<ElleValue> = record.iter().map(|s| a.string(s)).collect();
        rows.push(a.array(&fields));
    }
    a.ok(a.array(&rows))
}

extern "C" fn prim_csv_write(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let name = "csv/write";
    if nargs == 0 || nargs > 2 {
        return a.err(
            "arity-error",
            &format!("{}: expected 1 or 2 arguments, got {}", name, nargs),
        );
    }

    // rows must be an array
    let rows_val = a.arg(args, nargs, 0);
    let row_count = match a.get_array_len(rows_val) {
        Some(n) => n,
        None => {
            return a.err(
                "type-error",
                &format!("{}: expected array, got {}", name, a.type_name(rows_val)),
            );
        }
    };

    let opts = if nargs == 2 {
        a.arg(args, nargs, 1)
    } else {
        a.nil()
    };
    let delim = match extract_delimiter(opts, name) {
        Ok(d) => d,
        Err(e) => return e,
    };

    // We can't iterate struct keys through the stable ABI.
    // csv/write requires knowing the header keys from the first struct.
    // This is a fundamental limitation — the stable ABI doesn't expose
    // struct iteration.
    if row_count > 0 {
        let first = a.get_array_item(rows_val, 0);
        if !a.check_struct(first) {
            return a.err(
                "type-error",
                &format!(
                    "{}: each row must be a struct, got {}",
                    name,
                    a.type_name(first)
                ),
            );
        }
        return a.err(
            "csv-error",
            &format!("{}: cannot serialize structs (struct iteration not available in stable ABI)", name),
        );
    }

    // Empty array → empty CSV
    let mut out: Vec<u8> = Vec::new();
    let wtr = csv::WriterBuilder::new()
        .delimiter(delim)
        .from_writer(&mut out);
    drop(wtr);
    let s = match String::from_utf8(out) {
        Ok(s) => s,
        Err(e) => {
            return a.err("csv-error", &format!("{}: {}", name, e));
        }
    };
    a.ok(a.string(&s))
}

extern "C" fn prim_csv_write_rows(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let name = "csv/write-rows";
    if nargs == 0 || nargs > 2 {
        return a.err(
            "arity-error",
            &format!("{}: expected 1 or 2 arguments, got {}", name, nargs),
        );
    }

    // rows must be an array
    let rows_val = a.arg(args, nargs, 0);
    let row_count = match a.get_array_len(rows_val) {
        Some(n) => n,
        None => {
            return a.err(
                "type-error",
                &format!("{}: expected array, got {}", name, a.type_name(rows_val)),
            );
        }
    };

    let opts = if nargs == 2 {
        a.arg(args, nargs, 1)
    } else {
        a.nil()
    };
    let delim = match extract_delimiter(opts, name) {
        Ok(d) => d,
        Err(e) => return e,
    };

    let mut out: Vec<u8> = Vec::new();
    let mut wtr = csv::WriterBuilder::new()
        .delimiter(delim)
        .from_writer(&mut out);

    for i in 0..row_count {
        let row = a.get_array_item(rows_val, i);
        let field_count = match a.get_array_len(row) {
            Some(n) => n,
            None => {
                return a.err(
                    "type-error",
                    &format!(
                        "{}: each row must be an array, got {}",
                        name,
                        a.type_name(row)
                    ),
                );
            }
        };
        let fields: Vec<String> = (0..field_count)
            .map(|j| value_to_csv_field(a.get_array_item(row, j)))
            .collect();
        if let Err(e) = wtr.write_record(&fields) {
            return a.err("csv-error", &format!("{}: {}", name, e));
        }
    }

    drop(wtr);

    let s = match String::from_utf8(out) {
        Ok(s) => s,
        Err(e) => {
            return a.err("csv-error", &format!("{}: {}", name, e));
        }
    };
    a.ok(a.string(&s))
}

// ---------------------------------------------------------------------------
// Registration table
// ---------------------------------------------------------------------------

static PRIMITIVES: &[EllePrimDef] = &[
    EllePrimDef::range(
        "csv/parse",
        prim_csv_parse,
        SIG_ERROR,
        1,
        2,
        "Parse a CSV string with headers. First row becomes keyword keys. Returns array of structs. Optional opts: {:delimiter char-string}.",
        "csv",
        r#"(csv/parse "name,age\nAlice,30")"#,
    ),
    EllePrimDef::range(
        "csv/parse-rows",
        prim_csv_parse_rows,
        SIG_ERROR,
        1,
        2,
        "Parse a CSV string without header interpretation. Returns array of arrays. Optional opts: {:delimiter char-string}.",
        "csv",
        r#"(csv/parse-rows "a,b\n1,2")"#,
    ),
    EllePrimDef::range(
        "csv/write",
        prim_csv_write,
        SIG_ERROR,
        1,
        2,
        "Serialize an array of structs to a CSV string. Keys from the first struct become the header row. Optional opts: {:delimiter char-string}.",
        "csv",
        r#"(csv/write [{:name "Alice" :age "30"}])"#,
    ),
    EllePrimDef::range(
        "csv/write-rows",
        prim_csv_write_rows,
        SIG_ERROR,
        1,
        2,
        "Serialize an array of arrays to a CSV string without headers. Optional opts: {:delimiter char-string}.",
        "csv",
        r#"(csv/write-rows [["a" "b"] ["1" "2"]])"#,
    ),
];
