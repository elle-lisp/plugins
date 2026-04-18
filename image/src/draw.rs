//! Drawing primitives for mutable @image.

use imageproc::drawing;
use elle_plugin::{ElleResult, ElleValue};
use crate::{api, extract_color, get_image_mut, require_int};

pub(crate) extern "C" fn prim_draw_line(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let m = match get_image_mut(unsafe { a.arg(args, nargs, 0) }, "image/draw-line") { Ok(m) => m, Err(e) => return e };
    let x1 = match require_int(unsafe { a.arg(args, nargs, 1) }, "image/draw-line", "x1") { Ok(v) => v as i32, Err(e) => return e };
    let y1 = match require_int(unsafe { a.arg(args, nargs, 2) }, "image/draw-line", "y1") { Ok(v) => v as i32, Err(e) => return e };
    let x2 = match require_int(unsafe { a.arg(args, nargs, 3) }, "image/draw-line", "x2") { Ok(v) => v as i32, Err(e) => return e };
    let y2 = match require_int(unsafe { a.arg(args, nargs, 4) }, "image/draw-line", "y2") { Ok(v) => v as i32, Err(e) => return e };
    let color = match extract_color(unsafe { a.arg(args, nargs, 5) }, "image/draw-line") { Ok(c) => c, Err(e) => return e };
    let mut img = m.0.borrow_mut();
    let rgba = img.as_mut_rgba8().expect("draw requires rgba8 @image");
    drawing::draw_line_segment_mut(rgba, (x1 as f32, y1 as f32), (x2 as f32, y2 as f32), color);
    a.ok(a.nil())
}

pub(crate) extern "C" fn prim_draw_rect(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let m = match get_image_mut(unsafe { a.arg(args, nargs, 0) }, "image/draw-rect") { Ok(m) => m, Err(e) => return e };
    let x = match require_int(unsafe { a.arg(args, nargs, 1) }, "image/draw-rect", "x") { Ok(v) => v as i32, Err(e) => return e };
    let y = match require_int(unsafe { a.arg(args, nargs, 2) }, "image/draw-rect", "y") { Ok(v) => v as i32, Err(e) => return e };
    let w = match require_int(unsafe { a.arg(args, nargs, 3) }, "image/draw-rect", "width") { Ok(v) => v as u32, Err(e) => return e };
    let h = match require_int(unsafe { a.arg(args, nargs, 4) }, "image/draw-rect", "height") { Ok(v) => v as u32, Err(e) => return e };
    let color = match extract_color(unsafe { a.arg(args, nargs, 5) }, "image/draw-rect") { Ok(c) => c, Err(e) => return e };
    let mut img = m.0.borrow_mut();
    let rgba = img.as_mut_rgba8().expect("draw requires rgba8 @image");
    let rect = imageproc::rect::Rect::at(x, y).of_size(w, h);
    drawing::draw_hollow_rect_mut(rgba, rect, color);
    a.ok(a.nil())
}

pub(crate) extern "C" fn prim_draw_circle(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let m = match get_image_mut(unsafe { a.arg(args, nargs, 0) }, "image/draw-circle") { Ok(m) => m, Err(e) => return e };
    let cx = match require_int(unsafe { a.arg(args, nargs, 1) }, "image/draw-circle", "cx") { Ok(v) => v as i32, Err(e) => return e };
    let cy = match require_int(unsafe { a.arg(args, nargs, 2) }, "image/draw-circle", "cy") { Ok(v) => v as i32, Err(e) => return e };
    let r = match require_int(unsafe { a.arg(args, nargs, 3) }, "image/draw-circle", "radius") { Ok(v) => v as i32, Err(e) => return e };
    let color = match extract_color(unsafe { a.arg(args, nargs, 4) }, "image/draw-circle") { Ok(c) => c, Err(e) => return e };
    let mut img = m.0.borrow_mut();
    let rgba = img.as_mut_rgba8().expect("draw requires rgba8 @image");
    drawing::draw_hollow_circle_mut(rgba, (cx, cy), r, color);
    a.ok(a.nil())
}

pub(crate) extern "C" fn prim_fill_rect(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let m = match get_image_mut(unsafe { a.arg(args, nargs, 0) }, "image/fill-rect") { Ok(m) => m, Err(e) => return e };
    let x = match require_int(unsafe { a.arg(args, nargs, 1) }, "image/fill-rect", "x") { Ok(v) => v as i32, Err(e) => return e };
    let y = match require_int(unsafe { a.arg(args, nargs, 2) }, "image/fill-rect", "y") { Ok(v) => v as i32, Err(e) => return e };
    let w = match require_int(unsafe { a.arg(args, nargs, 3) }, "image/fill-rect", "width") { Ok(v) => v as u32, Err(e) => return e };
    let h = match require_int(unsafe { a.arg(args, nargs, 4) }, "image/fill-rect", "height") { Ok(v) => v as u32, Err(e) => return e };
    let color = match extract_color(unsafe { a.arg(args, nargs, 5) }, "image/fill-rect") { Ok(c) => c, Err(e) => return e };
    let mut img = m.0.borrow_mut();
    let rgba = img.as_mut_rgba8().expect("draw requires rgba8 @image");
    let rect = imageproc::rect::Rect::at(x, y).of_size(w, h);
    drawing::draw_filled_rect_mut(rgba, rect, color);
    a.ok(a.nil())
}

pub(crate) extern "C" fn prim_fill_circle(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let m = match get_image_mut(unsafe { a.arg(args, nargs, 0) }, "image/fill-circle") { Ok(m) => m, Err(e) => return e };
    let cx = match require_int(unsafe { a.arg(args, nargs, 1) }, "image/fill-circle", "cx") { Ok(v) => v as i32, Err(e) => return e };
    let cy = match require_int(unsafe { a.arg(args, nargs, 2) }, "image/fill-circle", "cy") { Ok(v) => v as i32, Err(e) => return e };
    let r = match require_int(unsafe { a.arg(args, nargs, 3) }, "image/fill-circle", "radius") { Ok(v) => v as i32, Err(e) => return e };
    let color = match extract_color(unsafe { a.arg(args, nargs, 4) }, "image/fill-circle") { Ok(c) => c, Err(e) => return e };
    let mut img = m.0.borrow_mut();
    let rgba = img.as_mut_rgba8().expect("draw requires rgba8 @image");
    drawing::draw_filled_circle_mut(rgba, (cx, cy), r, color);
    a.ok(a.nil())
}
