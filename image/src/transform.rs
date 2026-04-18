//! Image transforms and adjustments.

use image::imageops::FilterType;
use image::DynamicImage;
use elle_plugin::{ElleResult, ElleValue};
use crate::{api, get_image, require_float, require_int, wrap_image};

fn parse_filter(val: ElleValue) -> FilterType {
    let a = api();
    a.get_keyword_name(val).map(|s| match s {
        "nearest" => FilterType::Nearest,
        "bilinear" | "triangle" => FilterType::Triangle,
        "catmull-rom" | "cubic" => FilterType::CatmullRom,
        "gaussian" => FilterType::Gaussian,
        "lanczos3" | "lanczos" => FilterType::Lanczos3,
        _ => FilterType::Lanczos3,
    }).unwrap_or(FilterType::Lanczos3)
}

pub(crate) extern "C" fn prim_resize(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let img = match get_image(unsafe { a.arg(args, nargs, 0) }, "image/resize") { Ok(i) => i, Err(e) => return e };
    let w = match require_int(unsafe { a.arg(args, nargs, 1) }, "image/resize", "width") { Ok(v) => v as u32, Err(e) => return e };
    let h = match require_int(unsafe { a.arg(args, nargs, 2) }, "image/resize", "height") { Ok(v) => v as u32, Err(e) => return e };
    let filter = if nargs > 3 { parse_filter(unsafe { a.arg(args, nargs, 3) }) } else { FilterType::Lanczos3 };
    a.ok(wrap_image(img.resize_exact(w, h, filter)))
}

pub(crate) extern "C" fn prim_crop(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let mut img = match get_image(unsafe { a.arg(args, nargs, 0) }, "image/crop") { Ok(i) => i, Err(e) => return e };
    let x = match require_int(unsafe { a.arg(args, nargs, 1) }, "image/crop", "x") { Ok(v) => v as u32, Err(e) => return e };
    let y = match require_int(unsafe { a.arg(args, nargs, 2) }, "image/crop", "y") { Ok(v) => v as u32, Err(e) => return e };
    let w = match require_int(unsafe { a.arg(args, nargs, 3) }, "image/crop", "width") { Ok(v) => v as u32, Err(e) => return e };
    let h = match require_int(unsafe { a.arg(args, nargs, 4) }, "image/crop", "height") { Ok(v) => v as u32, Err(e) => return e };
    a.ok(wrap_image(img.crop(x, y, w, h)))
}

pub(crate) extern "C" fn prim_rotate(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let img = match get_image(unsafe { a.arg(args, nargs, 0) }, "image/rotate") { Ok(i) => i, Err(e) => return e };
    let v1 = unsafe { a.arg(args, nargs, 1) };
    let angle = match a.get_keyword_name(v1) { Some(s) => s, None => return a.err("type-error", &format!("image/rotate: angle must be :r90 :r180 :r270, got {}", a.type_name(v1))) };
    let result = match angle {
        "r90" | "90" => img.rotate90(), "r180" | "180" => img.rotate180(), "r270" | "270" => img.rotate270(),
        _ => return a.err("value-error", &format!("image/rotate: expected :r90 :r180 :r270, got :{}", angle)),
    };
    a.ok(wrap_image(result))
}

pub(crate) extern "C" fn prim_flip(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let img = match get_image(unsafe { a.arg(args, nargs, 0) }, "image/flip") { Ok(i) => i, Err(e) => return e };
    let v1 = unsafe { a.arg(args, nargs, 1) };
    let dir = match a.get_keyword_name(v1) { Some(s) => s, None => return a.err("type-error", &format!("image/flip: direction must be :h or :v, got {}", a.type_name(v1))) };
    let result = match dir {
        "h" | "horizontal" => img.fliph(), "v" | "vertical" => img.flipv(),
        _ => return a.err("value-error", &format!("image/flip: expected :h or :v, got :{}", dir)),
    };
    a.ok(wrap_image(result))
}

pub(crate) extern "C" fn prim_blur(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let img = match get_image(unsafe { a.arg(args, nargs, 0) }, "image/blur") { Ok(i) => i, Err(e) => return e };
    let sigma = match require_float(unsafe { a.arg(args, nargs, 1) }, "image/blur", "sigma") { Ok(v) => v as f32, Err(e) => return e };
    a.ok(wrap_image(img.blur(sigma)))
}

pub(crate) extern "C" fn prim_contrast(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let img = match get_image(unsafe { a.arg(args, nargs, 0) }, "image/contrast") { Ok(i) => i, Err(e) => return e };
    let c = match require_float(unsafe { a.arg(args, nargs, 1) }, "image/contrast", "contrast") { Ok(v) => v as f32, Err(e) => return e };
    a.ok(wrap_image(img.adjust_contrast(c)))
}

pub(crate) extern "C" fn prim_brighten(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let img = match get_image(unsafe { a.arg(args, nargs, 0) }, "image/brighten") { Ok(i) => i, Err(e) => return e };
    let b = match require_int(unsafe { a.arg(args, nargs, 1) }, "image/brighten", "brightness") { Ok(v) => v as i32, Err(e) => return e };
    a.ok(wrap_image(img.brighten(b)))
}

pub(crate) extern "C" fn prim_grayscale(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let img = match get_image(unsafe { a.arg(args, nargs, 0) }, "image/grayscale") { Ok(i) => i, Err(e) => return e };
    a.ok(wrap_image(img.grayscale()))
}

pub(crate) extern "C" fn prim_invert(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let mut img = match get_image(unsafe { a.arg(args, nargs, 0) }, "image/invert") { Ok(i) => i, Err(e) => return e };
    img.invert();
    a.ok(wrap_image(img))
}

pub(crate) extern "C" fn prim_hue_rotate(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let img = match get_image(unsafe { a.arg(args, nargs, 0) }, "image/hue-rotate") { Ok(i) => i, Err(e) => return e };
    let deg = match require_int(unsafe { a.arg(args, nargs, 1) }, "image/hue-rotate", "degrees") { Ok(v) => v as i32, Err(e) => return e };
    a.ok(wrap_image(img.huerotate(deg)))
}

pub(crate) extern "C" fn prim_to_rgba8(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let img = match get_image(unsafe { a.arg(args, nargs, 0) }, "image/to-rgba8") { Ok(i) => i, Err(e) => return e };
    a.ok(wrap_image(DynamicImage::ImageRgba8(img.to_rgba8())))
}

pub(crate) extern "C" fn prim_to_rgb8(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let img = match get_image(unsafe { a.arg(args, nargs, 0) }, "image/to-rgb8") { Ok(i) => i, Err(e) => return e };
    a.ok(wrap_image(DynamicImage::ImageRgb8(img.to_rgb8())))
}

pub(crate) extern "C" fn prim_to_luma8(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let img = match get_image(unsafe { a.arg(args, nargs, 0) }, "image/to-luma8") { Ok(i) => i, Err(e) => return e };
    a.ok(wrap_image(DynamicImage::ImageLuma8(img.to_luma8())))
}
