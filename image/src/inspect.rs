//! Image introspection, pixel access, and mutability conversion.

use image::{DynamicImage, GenericImage, GenericImageView};
use elle_plugin::{ElleResult, ElleValue};
use crate::{api, color_type_keyword, extract_color, get_image, get_image_mut, get_image_ref, parse_color_type, require_int, wrap_image, wrap_image_mut};

pub(crate) extern "C" fn prim_width(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let r = match get_image_ref(unsafe { a.arg(args, nargs, 0) }, "image/width") { Ok(r) => r, Err(e) => return e };
    a.ok(a.int(r.with(|img| img.width() as i64)))
}

pub(crate) extern "C" fn prim_height(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let r = match get_image_ref(unsafe { a.arg(args, nargs, 0) }, "image/height") { Ok(r) => r, Err(e) => return e };
    a.ok(a.int(r.with(|img| img.height() as i64)))
}

pub(crate) extern "C" fn prim_dimensions(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let r = match get_image_ref(unsafe { a.arg(args, nargs, 0) }, "image/dimensions") { Ok(r) => r, Err(e) => return e };
    let (w, h) = r.with(|img| img.dimensions());
    a.ok(a.array(&[a.int(w as i64), a.int(h as i64)]))
}

pub(crate) extern "C" fn prim_color_type(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let r = match get_image_ref(unsafe { a.arg(args, nargs, 0) }, "image/color-type") { Ok(r) => r, Err(e) => return e };
    a.ok(a.keyword(r.with(color_type_keyword)))
}

pub(crate) extern "C" fn prim_pixels(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let img = match get_image(unsafe { a.arg(args, nargs, 0) }, "image/pixels") { Ok(i) => i, Err(e) => return e };
    a.ok(a.bytes(img.as_bytes()))
}

pub(crate) extern "C" fn prim_from_pixels(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let w = match require_int(unsafe { a.arg(args, nargs, 0) }, "image/from-pixels", "width") { Ok(v) => v as u32, Err(e) => return e };
    let h = match require_int(unsafe { a.arg(args, nargs, 1) }, "image/from-pixels", "height") { Ok(v) => v as u32, Err(e) => return e };
    let v2 = unsafe { a.arg(args, nargs, 2) };
    let fmt_kw = match a.get_keyword_name(v2) { Some(s) => s, None => return a.err("type-error", &format!("image/from-pixels: format must be keyword, got {}", a.type_name(v2))) };
    let v3 = unsafe { a.arg(args, nargs, 3) };
    let data = match a.get_bytes(v3) { Some(b) => b.to_vec(), None => return a.err("type-error", &format!("image/from-pixels: data must be bytes, got {}", a.type_name(v3))) };
    let result = match fmt_kw {
        "rgba8" => image::RgbaImage::from_raw(w, h, data).map(DynamicImage::ImageRgba8),
        "rgb8" => image::RgbImage::from_raw(w, h, data).map(DynamicImage::ImageRgb8),
        "luma8" => image::GrayImage::from_raw(w, h, data).map(DynamicImage::ImageLuma8),
        _ => return a.err("value-error", &format!("image/from-pixels: unsupported format :{}", fmt_kw)),
    };
    match result {
        Some(img) => a.ok(wrap_image(img)),
        None => a.err("value-error", &format!("image/from-pixels: data length does not match {}x{} :{}", w, h, fmt_kw)),
    }
}

pub(crate) extern "C" fn prim_get_pixel(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let r = match get_image_ref(unsafe { a.arg(args, nargs, 0) }, "image/get-pixel") { Ok(r) => r, Err(e) => return e };
    let x = match require_int(unsafe { a.arg(args, nargs, 1) }, "image/get-pixel", "x") { Ok(v) => v as u32, Err(e) => return e };
    let y = match require_int(unsafe { a.arg(args, nargs, 2) }, "image/get-pixel", "y") { Ok(v) => v as u32, Err(e) => return e };
    r.with(|img| {
        if x >= img.width() || y >= img.height() {
            a.err("range-error", &format!("image/get-pixel: ({}, {}) out of bounds for {}x{} image", x, y, img.width(), img.height()))
        } else {
            let px = img.to_rgba8().get_pixel(x, y).0;
            a.ok(a.array(&[a.int(px[0] as i64), a.int(px[1] as i64), a.int(px[2] as i64), a.int(px[3] as i64)]))
        }
    })
}

pub(crate) extern "C" fn prim_put_pixel(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let x = match require_int(unsafe { a.arg(args, nargs, 1) }, "image/put-pixel", "x") { Ok(v) => v as u32, Err(e) => return e };
    let y = match require_int(unsafe { a.arg(args, nargs, 2) }, "image/put-pixel", "y") { Ok(v) => v as u32, Err(e) => return e };
    let color = match extract_color(unsafe { a.arg(args, nargs, 3) }, "image/put-pixel") { Ok(c) => c, Err(e) => return e };
    let v0 = unsafe { a.arg(args, nargs, 0) };
    if let Some(m) = a.get_external::<crate::ImageMut>(v0, "@image") {
        let mut img = m.0.borrow_mut();
        if x >= img.width() || y >= img.height() {
            return a.err("range-error", &format!("image/put-pixel: ({}, {}) out of bounds for {}x{} image", x, y, img.width(), img.height()));
        }
        img.as_mut_rgba8().map(|buf| buf.put_pixel(x, y, color))
            .or_else(|| img.as_mut_rgb8().map(|buf| buf.put_pixel(x, y, image::Rgb([color.0[0], color.0[1], color.0[2]]))))
            .or_else(|| img.as_mut_luma8().map(|buf| buf.put_pixel(x, y, image::Luma([color.0[0]]))));
        return a.ok(a.nil());
    }
    let mut img = match get_image(v0, "image/put-pixel") { Ok(i) => i, Err(e) => return e };
    if x >= img.width() || y >= img.height() {
        return a.err("range-error", &format!("image/put-pixel: ({}, {}) out of bounds for {}x{} image", x, y, img.width(), img.height()));
    }
    img.put_pixel(x, y, color);
    a.ok(wrap_image(img))
}

pub(crate) extern "C" fn prim_thaw(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let img = match get_image(unsafe { a.arg(args, nargs, 0) }, "image/thaw") { Ok(i) => i, Err(e) => return e };
    a.ok(wrap_image_mut(img))
}

pub(crate) extern "C" fn prim_freeze(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let m = match get_image_mut(unsafe { a.arg(args, nargs, 0) }, "image/freeze") { Ok(m) => m, Err(e) => return e };
    a.ok(wrap_image(m.0.borrow().clone()))
}

pub(crate) extern "C" fn prim_new(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let w = match require_int(unsafe { a.arg(args, nargs, 0) }, "image/new", "width") { Ok(v) => v as u32, Err(e) => return e };
    let h = match require_int(unsafe { a.arg(args, nargs, 1) }, "image/new", "height") { Ok(v) => v as u32, Err(e) => return e };
    let v2 = unsafe { a.arg(args, nargs, 2) };
    let fmt_kw = match a.get_keyword_name(v2) { Some(s) => s, None => return a.err("type-error", &format!("image/new: format must be keyword, got {}", a.type_name(v2))) };
    match parse_color_type(fmt_kw) {
        Some(ctor) => a.ok(wrap_image_mut(ctor(w, h))),
        None => a.err("value-error", &format!("image/new: unsupported format :{}", fmt_kw)),
    }
}
