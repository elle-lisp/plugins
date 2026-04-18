//! Elle Arrow plugin — Apache Arrow columnar data via the `arrow` and `parquet` crates.

use std::io::Cursor;
use std::sync::Arc;

use arrow::array::{
    Array, ArrayRef, BooleanArray, Float64Array, Int64Array, NullArray, RecordBatch, StringArray,
};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::ipc::reader::StreamReader;
use arrow::ipc::writer::StreamWriter;
use arrow::util::pretty::pretty_format_batches;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::arrow::ArrowWriter;

use elle_plugin::{ElleResult, ElleValue, EllePrimDef, SIG_ERROR};

// ---------------------------------------------------------------------------
// Type wrappers
// ---------------------------------------------------------------------------

/// Wrapped RecordBatch stored as an external value.
struct BatchWrap(RecordBatch);

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn get_batch<'a>(val: ElleValue, name: &str) -> Result<&'a BatchWrap, ElleResult> {
    let a = api();
    a.get_external::<BatchWrap>(val, "arrow/batch").ok_or_else(|| {
        a.err(
            "type-error",
            &format!("{}: expected arrow/batch, got {}", name, a.type_name(val)),
        )
    })
}

fn extract_string(val: ElleValue, name: &str) -> Result<String, ElleResult> {
    let a = api();
    a.get_string(val)
        .map(|s| s.to_owned())
        .ok_or_else(|| {
            a.err(
                "type-error",
                &format!("{}: expected string, got {}", name, a.type_name(val)),
            )
        })
}

/// Convert an Elle array of values into an Arrow ArrayRef by inferring types.
fn elle_values_to_arrow(
    values: &[ElleValue],
    field_name: &str,
) -> Result<ArrayRef, String> {
    let a = api();
    if values.is_empty() {
        return Ok(Arc::new(NullArray::new(0)));
    }

    // Infer type from first non-nil value
    let first_non_nil = values.iter().find(|v| !a.check_nil(**v));
    match first_non_nil {
        None => Ok(Arc::new(NullArray::new(values.len()))),
        Some(v) if a.get_int(*v).is_some() => {
            let arr: Int64Array = values.iter().map(|v| a.get_int(*v)).collect();
            Ok(Arc::new(arr))
        }
        Some(v) if a.get_float(*v).is_some() => {
            let arr: Float64Array = values.iter().map(|v| a.get_float(*v)).collect();
            Ok(Arc::new(arr))
        }
        Some(v) if a.get_bool(*v).is_some() => {
            let arr: BooleanArray = values.iter().map(|v| a.get_bool(*v)).collect();
            Ok(Arc::new(arr))
        }
        Some(v) if a.get_string(*v).is_some() => {
            let strings: Vec<Option<String>> = values
                .iter()
                .map(|v| a.get_string(*v).map(|s| s.to_owned()))
                .collect();
            let arr: StringArray = strings.iter().map(|s| s.as_deref()).collect();
            Ok(Arc::new(arr))
        }
        _ => Err(format!(
            "cannot convert column '{}' to Arrow: unsupported element type",
            field_name,
        )),
    }
}

/// Convert an Arrow array to a Vec<ElleValue>.
fn arrow_to_elle_values(arr: &dyn Array) -> Vec<ElleValue> {
    let a = api();
    let len = arr.len();
    let mut out = Vec::with_capacity(len);
    for i in 0..len {
        if arr.is_null(i) {
            out.push(a.nil());
        } else {
            match arr.data_type() {
                DataType::Int8 | DataType::Int16 | DataType::Int32 | DataType::Int64 => {
                    let ia = arr.as_any().downcast_ref::<Int64Array>().or(None);
                    if let Some(ia) = ia {
                        out.push(a.int(ia.value(i)));
                    } else {
                        let casted = arrow::compute::cast(arr, &DataType::Int64).ok();
                        if let Some(ref c) = casted {
                            let ia = c.as_any().downcast_ref::<Int64Array>().unwrap();
                            out.push(a.int(ia.value(i)));
                        } else {
                            out.push(a.string(&format!("{:?}", arr)));
                        }
                    }
                }
                DataType::UInt8 | DataType::UInt16 | DataType::UInt32 | DataType::UInt64 => {
                    let casted = arrow::compute::cast(arr, &DataType::Int64).ok();
                    if let Some(ref c) = casted {
                        let ia = c.as_any().downcast_ref::<Int64Array>().unwrap();
                        out.push(a.int(ia.value(i)));
                    } else {
                        out.push(a.string("<arrow-value>"));
                    }
                }
                DataType::Float16 | DataType::Float32 | DataType::Float64 => {
                    let casted = arrow::compute::cast(arr, &DataType::Float64).ok();
                    if let Some(ref c) = casted {
                        let fa = c.as_any().downcast_ref::<Float64Array>().unwrap();
                        out.push(a.float(fa.value(i)));
                    } else {
                        out.push(a.string("<arrow-value>"));
                    }
                }
                DataType::Boolean => {
                    let ba = arr.as_any().downcast_ref::<BooleanArray>().unwrap();
                    out.push(a.boolean(ba.value(i)));
                }
                DataType::Utf8 | DataType::LargeUtf8 => {
                    let sa = arr.as_any().downcast_ref::<StringArray>();
                    if let Some(sa) = sa {
                        out.push(a.string(sa.value(i)));
                    } else {
                        out.push(a.string(""));
                    }
                }
                _ => {
                    // Fallback: stringify
                    let formatted =
                        arrow::util::display::ArrayFormatter::try_new(arr, &Default::default());
                    if let Ok(f) = formatted {
                        out.push(a.string(&f.value(i).to_string()));
                    } else {
                        out.push(a.string("<arrow-value>"));
                    }
                }
            }
        }
    }
    out
}

/// Convert a RecordBatch to an Elle array of structs.
fn batch_to_elle(batch: &RecordBatch) -> ElleValue {
    let a = api();
    let schema = batch.schema();
    let num_rows = batch.num_rows();
    let mut rows: Vec<ElleValue> = Vec::with_capacity(num_rows);

    // Pre-convert all columns
    let columns: Vec<Vec<ElleValue>> = batch
        .columns()
        .iter()
        .map(|col| arrow_to_elle_values(col.as_ref()))
        .collect();

    // Collect field names as owned strings
    let field_names: Vec<String> = schema.fields().iter().map(|f| f.name().clone()).collect();

    for row_idx in 0..num_rows {
        let fields: Vec<(&str, ElleValue)> = field_names
            .iter()
            .zip(columns.iter())
            .map(|(name, col_vals)| (name.as_str(), col_vals[row_idx]))
            .collect();
        rows.push(a.build_struct(&fields));
    }
    a.array(&rows)
}

// ---------------------------------------------------------------------------
// Plugin entry
// ---------------------------------------------------------------------------
elle_plugin::define_plugin!("arrow/", &PRIMITIVES);

// ---------------------------------------------------------------------------
// Primitives
// ---------------------------------------------------------------------------

/// (arrow/batch columns) — create a RecordBatch from column specifications.
/// Accepts an array of [column-name column-data] pairs.
extern "C" fn prim_batch(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let name = "arrow/batch";
    let arg0 = unsafe { a.arg(args, nargs, 0) };

    // The stable ABI does not expose struct key iteration, so we accept
    // an array of [name, data] pairs as the column specification format.
    let arr_len = match a.get_array_len(arg0) {
        Some(l) => l,
        None => {
            return a.err(
                "type-error",
                &format!("{}: expected array of [column-name column-data] pairs", name),
            );
        }
    };

    let mut columns: Vec<(String, Vec<ElleValue>)> = Vec::new();
    for i in 0..arr_len {
        let pair = a.get_array_item(arg0, i);
        let plen = match a.get_array_len(pair) {
            Some(l) => l,
            None => {
                return a.err(
                    "arrow-error",
                    &format!("{}: each column must be a [name data] pair", name),
                );
            }
        };
        if plen < 2 {
            return a.err(
                "arrow-error",
                &format!("{}: each column pair must have [name data]", name),
            );
        }
        let col_name_val = a.get_array_item(pair, 0);
        let col_data_val = a.get_array_item(pair, 1);
        let col_name = match a
            .get_string(col_name_val)
            .or_else(|| a.get_keyword_name(col_name_val))
        {
            Some(s) => s.to_string(),
            None => {
                return a.err(
                    "arrow-error",
                    &format!("{}: column name must be string or keyword", name),
                );
            }
        };
        let data_len = match a.get_array_len(col_data_val) {
            Some(l) => l,
            None => {
                return a.err(
                    "arrow-error",
                    &format!("{}: column data must be an array", name),
                );
            }
        };
        let mut vals = Vec::with_capacity(data_len);
        for j in 0..data_len {
            vals.push(a.get_array_item(col_data_val, j));
        }
        columns.push((col_name, vals));
    }

    if columns.is_empty() {
        return a.err("arrow-error", &format!("{}: no columns provided", name));
    }

    let mut fields = Vec::new();
    let mut arrays: Vec<ArrayRef> = Vec::new();

    for (col_name, values) in &columns {
        match elle_values_to_arrow(values, col_name) {
            Ok(arr) => {
                fields.push(Field::new(col_name, arr.data_type().clone(), true));
                arrays.push(arr);
            }
            Err(e) => {
                return a.err("arrow-error", &format!("{}: {}", name, e));
            }
        }
    }

    let schema = Arc::new(Schema::new(fields));
    match RecordBatch::try_new(schema, arrays) {
        Ok(batch) => a.ok(a.external("arrow/batch", BatchWrap(batch))),
        Err(e) => a.err("arrow-error", &format!("{}: {}", name, e)),
    }
}

/// (arrow/schema batch) — return the schema of a batch as a struct.
extern "C" fn prim_schema(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let name = "arrow/schema";
    let batch = match get_batch(unsafe { a.arg(args, nargs, 0) }, name) {
        Ok(b) => b,
        Err(e) => return e,
    };
    let schema = batch.0.schema();
    let field_names: Vec<String> = schema.fields().iter().map(|f| f.name().clone()).collect();
    let type_strs: Vec<String> = schema
        .fields()
        .iter()
        .map(|f| format!("{}", f.data_type()))
        .collect();
    let fields: Vec<(&str, ElleValue)> = field_names
        .iter()
        .zip(type_strs.iter())
        .map(|(name, tstr)| (name.as_str(), a.string(tstr)))
        .collect();
    a.ok(a.build_struct(&fields))
}

/// (arrow/num-rows batch) — return number of rows.
extern "C" fn prim_num_rows(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let name = "arrow/num-rows";
    let batch = match get_batch(unsafe { a.arg(args, nargs, 0) }, name) {
        Ok(b) => b,
        Err(e) => return e,
    };
    a.ok(a.int(batch.0.num_rows() as i64))
}

/// (arrow/num-cols batch) — return number of columns.
extern "C" fn prim_num_cols(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let name = "arrow/num-cols";
    let batch = match get_batch(unsafe { a.arg(args, nargs, 0) }, name) {
        Ok(b) => b,
        Err(e) => return e,
    };
    a.ok(a.int(batch.0.num_columns() as i64))
}

/// (arrow/column batch col-name) — extract a column as an Elle array.
extern "C" fn prim_column(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let name = "arrow/column";
    let batch = match get_batch(unsafe { a.arg(args, nargs, 0) }, name) {
        Ok(b) => b,
        Err(e) => return e,
    };
    let col_name = match extract_string(unsafe { a.arg(args, nargs, 1) }, name) {
        Ok(s) => s,
        Err(e) => return e,
    };
    let schema = batch.0.schema();
    match schema.index_of(&col_name) {
        Ok(idx) => {
            let col = batch.0.column(idx);
            let values = arrow_to_elle_values(col.as_ref());
            a.ok(a.array(&values))
        }
        Err(_) => a.err(
            "arrow-error",
            &format!("{}: column '{}' not found", name, col_name),
        ),
    }
}

/// (arrow/to-rows batch) — convert a RecordBatch to an Elle array of structs.
extern "C" fn prim_to_rows(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let name = "arrow/to-rows";
    let batch = match get_batch(unsafe { a.arg(args, nargs, 0) }, name) {
        Ok(b) => b,
        Err(e) => return e,
    };
    a.ok(batch_to_elle(&batch.0))
}

/// (arrow/display batch) — pretty-print a RecordBatch as a table string.
extern "C" fn prim_display(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let name = "arrow/display";
    let batch = match get_batch(unsafe { a.arg(args, nargs, 0) }, name) {
        Ok(b) => b,
        Err(e) => return e,
    };
    match pretty_format_batches(std::slice::from_ref(&batch.0)) {
        Ok(table) => a.ok(a.string(&table.to_string())),
        Err(e) => a.err("arrow-error", &format!("{}: {}", name, e)),
    }
}

/// (arrow/write-ipc batch) — serialize a RecordBatch to IPC bytes.
extern "C" fn prim_write_ipc(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let name = "arrow/write-ipc";
    let batch = match get_batch(unsafe { a.arg(args, nargs, 0) }, name) {
        Ok(b) => b,
        Err(e) => return e,
    };

    let mut buf = Vec::new();
    let schema = batch.0.schema();
    let mut writer = match StreamWriter::try_new(&mut buf, &schema) {
        Ok(w) => w,
        Err(e) => return a.err("arrow-error", &format!("{}: {}", name, e)),
    };
    if let Err(e) = writer.write(&batch.0) {
        return a.err("arrow-error", &format!("{}: {}", name, e));
    }
    if let Err(e) = writer.finish() {
        return a.err("arrow-error", &format!("{}: {}", name, e));
    }
    a.ok(a.bytes(&buf))
}

/// (arrow/read-ipc bytes) — deserialize IPC bytes to a RecordBatch.
extern "C" fn prim_read_ipc(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let name = "arrow/read-ipc";
    let arg0 = unsafe { a.arg(args, nargs, 0) };
    let bytes = match a.get_bytes(arg0) {
        Some(b) => b,
        None => {
            return a.err(
                "type-error",
                &format!("{}: expected bytes, got {}", name, a.type_name(arg0)),
            );
        }
    };

    let cursor = Cursor::new(bytes.to_vec());
    let reader = match StreamReader::try_new(cursor, None) {
        Ok(r) => r,
        Err(e) => return a.err("arrow-error", &format!("{}: {}", name, e)),
    };

    let mut batches = Vec::new();
    for batch_result in reader {
        match batch_result {
            Ok(batch) => batches.push(batch),
            Err(e) => return a.err("arrow-error", &format!("{}: {}", name, e)),
        }
    }

    if batches.len() == 1 {
        a.ok(a.external(
            "arrow/batch",
            BatchWrap(batches.into_iter().next().unwrap()),
        ))
    } else {
        let vals: Vec<ElleValue> = batches
            .into_iter()
            .map(|b| a.external("arrow/batch", BatchWrap(b)))
            .collect();
        a.ok(a.array(&vals))
    }
}

/// (arrow/write-parquet batch) — serialize a RecordBatch to Parquet bytes.
extern "C" fn prim_write_parquet(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let name = "arrow/write-parquet";
    let batch = match get_batch(unsafe { a.arg(args, nargs, 0) }, name) {
        Ok(b) => b,
        Err(e) => return e,
    };

    let mut buf = Vec::new();
    let schema = batch.0.schema();
    let mut writer = match ArrowWriter::try_new(&mut buf, schema, None) {
        Ok(w) => w,
        Err(e) => return a.err("arrow-error", &format!("{}: {}", name, e)),
    };
    if let Err(e) = writer.write(&batch.0) {
        return a.err("arrow-error", &format!("{}: {}", name, e));
    }
    if let Err(e) = writer.close() {
        return a.err("arrow-error", &format!("{}: {}", name, e));
    }
    a.ok(a.bytes(&buf))
}

/// (arrow/read-parquet bytes) — deserialize Parquet bytes to a RecordBatch.
extern "C" fn prim_read_parquet(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let name = "arrow/read-parquet";
    let arg0 = unsafe { a.arg(args, nargs, 0) };
    let raw_bytes = match a.get_bytes(arg0) {
        Some(b) => b,
        None => {
            return a.err(
                "type-error",
                &format!("{}: expected bytes, got {}", name, a.type_name(arg0)),
            );
        }
    };

    let builder =
        match ParquetRecordBatchReaderBuilder::try_new(bytes::Bytes::from(raw_bytes.to_vec())) {
            Ok(b) => b,
            Err(e) => return a.err("arrow-error", &format!("{}: {}", name, e)),
        };
    let reader = match builder.build() {
        Ok(r) => r,
        Err(e) => return a.err("arrow-error", &format!("{}: {}", name, e)),
    };

    let mut batches = Vec::new();
    for batch_result in reader {
        match batch_result {
            Ok(batch) => batches.push(batch),
            Err(e) => return a.err("arrow-error", &format!("{}: {}", name, e)),
        }
    }

    if batches.is_empty() {
        return a.err("arrow-error", &format!("{}: no data in parquet", name));
    }

    // Concatenate all batches
    if batches.len() == 1 {
        a.ok(a.external(
            "arrow/batch",
            BatchWrap(batches.into_iter().next().unwrap()),
        ))
    } else {
        let schema = batches[0].schema();
        match arrow::compute::concat_batches(&schema, &batches) {
            Ok(merged) => a.ok(a.external("arrow/batch", BatchWrap(merged))),
            Err(e) => a.err("arrow-error", &format!("{}: {}", name, e)),
        }
    }
}

/// (arrow/slice batch offset length) — take a zero-copy slice of a batch.
extern "C" fn prim_slice(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let name = "arrow/slice";
    let batch = match get_batch(unsafe { a.arg(args, nargs, 0) }, name) {
        Ok(b) => b,
        Err(e) => return e,
    };
    let offset = match a.get_int(unsafe { a.arg(args, nargs, 1) }) {
        Some(o) => o as usize,
        None => return a.err("type-error", &format!("{}: offset must be integer", name)),
    };
    let length = match a.get_int(unsafe { a.arg(args, nargs, 2) }) {
        Some(l) => l as usize,
        None => return a.err("type-error", &format!("{}: length must be integer", name)),
    };

    let sliced = batch.0.slice(offset, length);
    a.ok(a.external("arrow/batch", BatchWrap(sliced)))
}

// ---------------------------------------------------------------------------
// Registration table
// ---------------------------------------------------------------------------

static PRIMITIVES: &[EllePrimDef] = &[
    EllePrimDef::exact("arrow/batch", prim_batch, SIG_ERROR, 1,
        "Create a RecordBatch from an array of [column-name column-data] pairs. Values are typed by inference (int, float, bool, string).",
        "arrow",
        r#"(arrow/batch [["name" ["Alice" "Bob"]] ["age" [30 25]]])"#),
    EllePrimDef::exact("arrow/schema", prim_schema, SIG_ERROR, 1,
        "Return the schema of a batch as a struct mapping column names to type strings.",
        "arrow", r#"(arrow/schema my-batch)"#),
    EllePrimDef::exact("arrow/num-rows", prim_num_rows, SIG_ERROR, 1,
        "Return the number of rows in a batch.", "arrow",
        "(arrow/num-rows my-batch)"),
    EllePrimDef::exact("arrow/num-cols", prim_num_cols, SIG_ERROR, 1,
        "Return the number of columns in a batch.", "arrow",
        "(arrow/num-cols my-batch)"),
    EllePrimDef::exact("arrow/column", prim_column, SIG_ERROR, 2,
        "Extract a column from a batch by name, returned as an Elle array.", "arrow",
        r#"(arrow/column my-batch "name")"#),
    EllePrimDef::exact("arrow/to-rows", prim_to_rows, SIG_ERROR, 1,
        "Convert a RecordBatch to an Elle array of structs (one struct per row).", "arrow",
        "(arrow/to-rows my-batch)"),
    EllePrimDef::exact("arrow/display", prim_display, SIG_ERROR, 1,
        "Pretty-print a RecordBatch as a formatted table string.", "arrow",
        "(arrow/display my-batch)"),
    EllePrimDef::exact("arrow/write-ipc", prim_write_ipc, SIG_ERROR, 1,
        "Serialize a RecordBatch to Arrow IPC stream format (bytes).", "arrow",
        "(arrow/write-ipc my-batch)"),
    EllePrimDef::exact("arrow/read-ipc", prim_read_ipc, SIG_ERROR, 1,
        "Deserialize Arrow IPC bytes into a RecordBatch (or array of batches).", "arrow",
        "(arrow/read-ipc ipc-bytes)"),
    EllePrimDef::exact("arrow/write-parquet", prim_write_parquet, SIG_ERROR, 1,
        "Serialize a RecordBatch to Parquet format (bytes).", "arrow",
        "(arrow/write-parquet my-batch)"),
    EllePrimDef::exact("arrow/read-parquet", prim_read_parquet, SIG_ERROR, 1,
        "Deserialize Parquet bytes into a RecordBatch.", "arrow",
        "(arrow/read-parquet pq-bytes)"),
    EllePrimDef::exact("arrow/slice", prim_slice, SIG_ERROR, 3,
        "Take a zero-copy slice of a batch: (arrow/slice batch offset length).", "arrow",
        "(arrow/slice my-batch 0 10)"),
];
