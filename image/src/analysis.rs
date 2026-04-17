//! Image analysis: histogram, edge detection, threshold, morphology.

use image::DynamicImage;
use imageproc::contrast;
use imageproc::edges;
use imageproc::morphology;
use elle_plugin::{ElleResult, ElleValue};
use crate::{api, get_image, require_int, wrap_image};

pub(crate) extern "C" fn prim_histogram(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let img = match get_image(a.arg(args, nargs, 0), "image/histogram") { Ok(i) => i, Err(e) => return e };
    let rgba = img.to_rgba8();
    let mut r_hist = vec![0i64; 256];
    let mut g_hist = vec![0i64; 256];
    let mut b_hist = vec![0i64; 256];
    let mut a_hist = vec![0i64; 256];
    for px in rgba.pixels() {
        r_hist[px[0] as usize] += 1;
        g_hist[px[1] as usize] += 1;
        b_hist[px[2] as usize] += 1;
        a_hist[px[3] as usize] += 1;
    }
    let to_array = |h: Vec<i64>| { let vals: Vec<ElleValue> = h.into_iter().map(|v| a.int(v)).collect(); a.array(&vals) };
    a.ok(a.build_struct(&[
        ("r", to_array(r_hist)), ("g", to_array(g_hist)),
        ("b", to_array(b_hist)), ("a", to_array(a_hist)),
    ]))
}

pub(crate) extern "C" fn prim_edges(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let img = match get_image(a.arg(args, nargs, 0), "image/edges") { Ok(i) => i, Err(e) => return e };
    let algo = if nargs > 1 {
        a.get_keyword_name(a.arg(args, nargs, 1)).unwrap_or("canny")
    } else { "canny" };
    let gray = img.to_luma8();
    let result = match algo {
        "canny" => edges::canny(&gray, 50.0, 100.0),
        "sobel" => {
            let h = imageproc::gradients::horizontal_sobel(&gray);
            let v = imageproc::gradients::vertical_sobel(&gray);
            let mut out = image::GrayImage::new(gray.width(), gray.height());
            for (x, y, px) in out.enumerate_pixels_mut() {
                let hv = h.get_pixel(x, y)[0] as f64;
                let vv = v.get_pixel(x, y)[0] as f64;
                let mag = (hv * hv + vv * vv).sqrt().min(255.0) as u8;
                *px = image::Luma([mag]);
            }
            out
        }
        _ => return a.err("value-error", &format!("image/edges: unknown algorithm :{}, expected :canny or :sobel", algo)),
    };
    a.ok(wrap_image(DynamicImage::ImageLuma8(result)))
}

pub(crate) extern "C" fn prim_threshold(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let img = match get_image(a.arg(args, nargs, 0), "image/threshold") { Ok(i) => i, Err(e) => return e };
    let t = match require_int(a.arg(args, nargs, 1), "image/threshold", "threshold") { Ok(v) => v.clamp(0, 255) as u8, Err(e) => return e };
    let gray = img.to_luma8();
    let result = contrast::threshold(&gray, t, imageproc::contrast::ThresholdType::Binary);
    a.ok(wrap_image(DynamicImage::ImageLuma8(result)))
}

pub(crate) extern "C" fn prim_erode(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let img = match get_image(a.arg(args, nargs, 0), "image/erode") { Ok(i) => i, Err(e) => return e };
    let radius = match require_int(a.arg(args, nargs, 1), "image/erode", "radius") { Ok(v) => v.max(0) as u8, Err(e) => return e };
    let gray = img.to_luma8();
    let result = morphology::erode(&gray, imageproc::distance_transform::Norm::LInf, radius);
    a.ok(wrap_image(DynamicImage::ImageLuma8(result)))
}

pub(crate) extern "C" fn prim_dilate(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let img = match get_image(a.arg(args, nargs, 0), "image/dilate") { Ok(i) => i, Err(e) => return e };
    let radius = match require_int(a.arg(args, nargs, 1), "image/dilate", "radius") { Ok(v) => v.max(0) as u8, Err(e) => return e };
    let gray = img.to_luma8();
    let result = morphology::dilate(&gray, imageproc::distance_transform::Norm::LInf, radius);
    a.ok(wrap_image(DynamicImage::ImageLuma8(result)))
}
