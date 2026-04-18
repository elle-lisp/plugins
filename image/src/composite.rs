//! Image compositing: overlay and blend.

use image::DynamicImage;
use elle_plugin::{ElleResult, ElleValue};
use crate::{api, get_image, require_float, require_int, wrap_image};

pub(crate) extern "C" fn prim_overlay(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let mut base = match get_image(unsafe { a.arg(args, nargs, 0) }, "image/overlay") { Ok(i) => i.to_rgba8(), Err(e) => return e };
    let overlay = match get_image(unsafe { a.arg(args, nargs, 1) }, "image/overlay") { Ok(i) => i.to_rgba8(), Err(e) => return e };
    let x = match require_int(unsafe { a.arg(args, nargs, 2) }, "image/overlay", "x") { Ok(v) => v, Err(e) => return e };
    let y = match require_int(unsafe { a.arg(args, nargs, 3) }, "image/overlay", "y") { Ok(v) => v, Err(e) => return e };
    image::imageops::overlay(&mut base, &overlay, x, y);
    a.ok(wrap_image(DynamicImage::ImageRgba8(base)))
}

pub(crate) extern "C" fn prim_blend(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let img1 = match get_image(unsafe { a.arg(args, nargs, 0) }, "image/blend") { Ok(i) => i.to_rgba8(), Err(e) => return e };
    let img2 = match get_image(unsafe { a.arg(args, nargs, 1) }, "image/blend") { Ok(i) => i.to_rgba8(), Err(e) => return e };
    let alpha = match require_float(unsafe { a.arg(args, nargs, 2) }, "image/blend", "alpha") { Ok(v) => v as f32, Err(e) => return e };
    if img1.dimensions() != img2.dimensions() {
        return a.err("value-error", &format!("image/blend: images must have same dimensions, got {}x{} and {}x{}", img1.width(), img1.height(), img2.width(), img2.height()));
    }
    let alpha = alpha.clamp(0.0, 1.0);
    let inv = 1.0 - alpha;
    let mut out = image::RgbaImage::new(img1.width(), img1.height());
    for (x, y, px1) in img1.enumerate_pixels() {
        let px2 = img2.get_pixel(x, y);
        let blended = image::Rgba([
            (px1[0] as f32 * inv + px2[0] as f32 * alpha) as u8,
            (px1[1] as f32 * inv + px2[1] as f32 * alpha) as u8,
            (px1[2] as f32 * inv + px2[2] as f32 * alpha) as u8,
            (px1[3] as f32 * inv + px2[3] as f32 * alpha) as u8,
        ]);
        out.put_pixel(x, y, blended);
    }
    a.ok(wrap_image(DynamicImage::ImageRgba8(out)))
}
