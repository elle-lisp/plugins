//! Image I/O primitives: read, write, decode, encode.

use std::io::Cursor;
use elle_plugin::{ElleCtx, ElleResult, ElleValue};
use crate::{api, get_image, parse_format, require_string, wrap_image};

pub(crate) extern "C" fn prim_read(ctx: *mut ElleCtx, args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let path = match require_string(ctx, unsafe { a.arg(args, nargs, 0) }, "image/read", "path") { Ok(s) => s, Err(e) => return e };
    match image::open(&path) {
        Ok(img) => a.ok(wrap_image(ctx, img)),
        Err(e) => a.err(ctx, "image-error", &format!("image/read: {}", e)),
    }
}

pub(crate) extern "C" fn prim_write(ctx: *mut ElleCtx, args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let img = match get_image(ctx, unsafe { a.arg(args, nargs, 0) }, "image/write") { Ok(i) => i, Err(e) => return e };
    let path = match require_string(ctx, unsafe { a.arg(args, nargs, 1) }, "image/write", "path") { Ok(s) => s, Err(e) => return e };
    match img.save(&path) {
        Ok(()) => a.ok(a.nil()),
        Err(e) => a.err(ctx, "image-error", &format!("image/write: {}", e)),
    }
}

pub(crate) extern "C" fn prim_decode(ctx: *mut ElleCtx, args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let v0 = unsafe { a.arg(args, nargs, 0) };
    let data = match a.get_bytes(v0) {
        Some(b) => b.to_vec(),
        None => return a.err(ctx, "type-error", &format!("image/decode: expected bytes, got {}", a.type_name(v0))),
    };
    let fmt = match parse_format(ctx, unsafe { a.arg(args, nargs, 1) }, "image/decode") { Ok(f) => f, Err(e) => return e };
    let reader = Cursor::new(data);
    match image::load(reader, fmt) {
        Ok(img) => a.ok(wrap_image(ctx, img)),
        Err(e) => a.err(ctx, "image-error", &format!("image/decode: {}", e)),
    }
}

pub(crate) extern "C" fn prim_encode(ctx: *mut ElleCtx, args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let img = match get_image(ctx, unsafe { a.arg(args, nargs, 0) }, "image/encode") { Ok(i) => i, Err(e) => return e };
    let fmt = match parse_format(ctx, unsafe { a.arg(args, nargs, 1) }, "image/encode") { Ok(f) => f, Err(e) => return e };
    let mut buf = Cursor::new(Vec::new());
    match img.write_to(&mut buf, fmt) {
        Ok(()) => a.ok(a.bytes(ctx, &buf.into_inner())),
        Err(e) => a.err(ctx, "image-error", &format!("image/encode: {}", e)),
    }
}
