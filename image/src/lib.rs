//! Elle image plugin — raster image I/O, transforms, drawing, and analysis
//! via the `image` and `imageproc` crates.

mod analysis;
mod composite;
mod draw;
mod inspect;
mod io;
mod transform;

use std::cell::RefCell;

use image::DynamicImage;

use elle_plugin::{ElleResult, ElleValue, EllePrimDef, SIG_OK, SIG_ERROR};

elle_plugin::define_plugin!("image/", &PRIMITIVES);

// ── Type wrappers ───────────────────────────────────────────────────

pub struct ImageWrap(pub DynamicImage);
pub struct ImageMut(pub RefCell<DynamicImage>);

// ── Helpers ─────────────────────────────────────────────────────────

pub fn get_image(val: ElleValue, name: &str) -> Result<DynamicImage, ElleResult> {
    let a = api();
    if let Some(w) = a.get_external::<ImageWrap>(val, "image") {
        Ok(w.0.clone())
    } else if let Some(m) = a.get_external::<ImageMut>(val, "@image") {
        Ok(m.0.borrow().clone())
    } else {
        Err(a.err("type-error", &format!("{}: expected image or @image, got {}", name, a.type_name(val))))
    }
}

pub fn get_image_ref<'a>(val: ElleValue, name: &str) -> Result<ImageRef<'a>, ElleResult> {
    let a = api();
    if let Some(w) = a.get_external::<ImageWrap>(val, "image") {
        Ok(ImageRef::Immutable(&w.0))
    } else if let Some(m) = a.get_external::<ImageMut>(val, "@image") {
        Ok(ImageRef::Mutable(m))
    } else {
        Err(a.err("type-error", &format!("{}: expected image or @image, got {}", name, a.type_name(val))))
    }
}

pub enum ImageRef<'a> {
    Immutable(&'a DynamicImage),
    Mutable(&'a ImageMut),
}

impl ImageRef<'_> {
    pub fn with<F, R>(&self, f: F) -> R where F: FnOnce(&DynamicImage) -> R {
        match self { ImageRef::Immutable(img) => f(img), ImageRef::Mutable(m) => f(&m.0.borrow()) }
    }
}

pub fn get_image_mut<'a>(val: ElleValue, name: &str) -> Result<&'a ImageMut, ElleResult> {
    let a = api();
    a.get_external::<ImageMut>(val, "@image").ok_or_else(|| {
        a.err("type-error", &format!("{}: expected @image, got {}", name, a.type_name(val)))
    })
}

pub fn wrap_image(img: DynamicImage) -> ElleValue {
    api().external("image", ImageWrap(img))
}

pub fn wrap_image_mut(img: DynamicImage) -> ElleValue {
    api().external("@image", ImageMut(RefCell::new(img)))
}

pub fn require_int(val: ElleValue, name: &str, param: &str) -> Result<i64, ElleResult> {
    let a = api();
    a.get_int(val).ok_or_else(|| a.err("type-error", &format!("{}: {} must be int, got {}", name, param, a.type_name(val))))
}

pub fn require_float(val: ElleValue, name: &str, param: &str) -> Result<f64, ElleResult> {
    let a = api();
    a.get_float(val).or_else(|| a.get_int(val).map(|i| i as f64))
        .ok_or_else(|| a.err("type-error", &format!("{}: {} must be number, got {}", name, param, a.type_name(val))))
}

pub fn require_string(val: ElleValue, name: &str, param: &str) -> Result<String, ElleResult> {
    let a = api();
    a.get_string(val).map(|s| s.to_string())
        .ok_or_else(|| a.err("type-error", &format!("{}: {} must be string, got {}", name, param, a.type_name(val))))
}

pub fn extract_color(val: ElleValue, name: &str) -> Result<image::Rgba<u8>, ElleResult> {
    let a = api();
    let len = a.get_array_len(val).ok_or_else(|| {
        a.err("type-error", &format!("{}: color must be [r g b a] array, got {}", name, a.type_name(val)))
    })?;
    if len != 4 {
        return Err(a.err("value-error", &format!("{}: color array must have 4 elements, got {}", name, len)));
    }
    let mut rgba = [0u8; 4];
    for i in 0..4 {
        let v = a.get_array_item(val, i);
        let n = a.get_int(v).ok_or_else(|| {
            a.err("type-error", &format!("{}: color component must be int, got {}", name, a.type_name(v)))
        })?;
        rgba[i] = n.clamp(0, 255) as u8;
    }
    Ok(image::Rgba(rgba))
}

pub fn parse_format(val: ElleValue, name: &str) -> Result<image::ImageFormat, ElleResult> {
    let a = api();
    let kw = a.get_keyword_name(val).ok_or_else(|| {
        a.err("type-error", &format!("{}: format must be a keyword, got {}", name, a.type_name(val)))
    })?;
    match kw {
        "png" => Ok(image::ImageFormat::Png),
        "jpeg" | "jpg" => Ok(image::ImageFormat::Jpeg),
        "gif" => Ok(image::ImageFormat::Gif),
        "webp" => Ok(image::ImageFormat::WebP),
        "tiff" | "tif" => Ok(image::ImageFormat::Tiff),
        "bmp" => Ok(image::ImageFormat::Bmp),
        "ico" => Ok(image::ImageFormat::Ico),
        "qoi" => Ok(image::ImageFormat::Qoi),
        _ => Err(a.err("value-error", &format!("{}: unsupported format :{}", name, kw))),
    }
}

pub fn color_type_keyword(img: &DynamicImage) -> &'static str {
    match img {
        DynamicImage::ImageLuma8(_) => "luma8",
        DynamicImage::ImageLumaA8(_) => "lumaa8",
        DynamicImage::ImageRgb8(_) => "rgb8",
        DynamicImage::ImageRgba8(_) => "rgba8",
        DynamicImage::ImageLuma16(_) => "luma16",
        DynamicImage::ImageLumaA16(_) => "lumaa16",
        DynamicImage::ImageRgb16(_) => "rgb16",
        DynamicImage::ImageRgba16(_) => "rgba16",
        DynamicImage::ImageRgb32F(_) => "rgb32f",
        DynamicImage::ImageRgba32F(_) => "rgba32f",
        _ => "unknown",
    }
}

pub fn parse_color_type(kw: &str) -> Option<fn(u32, u32) -> DynamicImage> {
    match kw {
        "rgba8" => Some(DynamicImage::new_rgba8 as fn(u32, u32) -> DynamicImage),
        "rgb8" => Some(DynamicImage::new_rgb8 as fn(u32, u32) -> DynamicImage),
        "luma8" => Some(DynamicImage::new_luma8 as fn(u32, u32) -> DynamicImage),
        "lumaa8" => Some(DynamicImage::new_luma_a8 as fn(u32, u32) -> DynamicImage),
        _ => None,
    }
}

// ── Primitive table ─────────────────────────────────────────────────

static PRIMITIVES: &[EllePrimDef] = &[
    // I/O
    EllePrimDef::exact("image/read", io::prim_read, SIG_ERROR, 1, "Read an image from a file path. Returns an immutable image.", "image", "(image/read \"photo.jpg\")"),
    EllePrimDef::exact("image/write", io::prim_write, SIG_ERROR, 2, "Write an image to a file. Format is inferred from extension.", "image", "(image/write img \"out.png\")"),
    EllePrimDef::exact("image/decode", io::prim_decode, SIG_ERROR, 2, "Decode an image from bytes with a specified format keyword (:png :jpeg :gif :webp :tiff :bmp :ico :qoi).", "image", "(image/decode raw-bytes :png)"),
    EllePrimDef::exact("image/encode", io::prim_encode, SIG_ERROR, 2, "Encode an image to bytes in the specified format. Returns bytes.", "image", "(image/encode img :png)"),
    // Introspection
    EllePrimDef::exact("image/width", inspect::prim_width, SIG_OK, 1, "Return the width of an image in pixels.", "image", "(image/width img)"),
    EllePrimDef::exact("image/height", inspect::prim_height, SIG_OK, 1, "Return the height of an image in pixels.", "image", "(image/height img)"),
    EllePrimDef::exact("image/dimensions", inspect::prim_dimensions, SIG_OK, 1, "Return [width height] of an image.", "image", "(image/dimensions img)"),
    EllePrimDef::exact("image/color-type", inspect::prim_color_type, SIG_OK, 1, "Return the color type keyword of an image (:rgba8 :rgb8 :luma8 etc.).", "image", "(image/color-type img)"),
    EllePrimDef::exact("image/pixels", inspect::prim_pixels, SIG_OK, 1, "Return the raw pixel data as bytes.", "image", "(image/pixels img)"),
    EllePrimDef::exact("image/from-pixels", inspect::prim_from_pixels, SIG_ERROR, 4, "Construct an immutable image from raw pixel data. Format: :rgba8 :rgb8 :luma8.", "image", "(image/from-pixels 2 2 :rgba8 pixel-bytes)"),
    EllePrimDef::exact("image/get-pixel", inspect::prim_get_pixel, SIG_ERROR, 3, "Get pixel at (x, y) as [r g b a] array (0-255).", "image", "(image/get-pixel img 0 0)"),
    EllePrimDef::exact("image/put-pixel", inspect::prim_put_pixel, SIG_ERROR, 4, "Set pixel at (x, y) to [r g b a]. On immutable image returns new image; on @image mutates in place.", "image", "(image/put-pixel img 0 0 [255 0 0 255])"),
    // Mutability
    EllePrimDef::exact("image/thaw", inspect::prim_thaw, SIG_ERROR, 1, "Convert an immutable image to a mutable @image (copy).", "image", "(image/thaw img)"),
    EllePrimDef::exact("image/freeze", inspect::prim_freeze, SIG_ERROR, 1, "Convert a mutable @image to an immutable image (snapshot).", "image", "(image/freeze @img)"),
    EllePrimDef::exact("image/new", inspect::prim_new, SIG_ERROR, 3, "Create a blank mutable @image. Format: :rgba8 :rgb8 :luma8 :lumaa8.", "image", "(image/new 100 100 :rgba8)"),
    // Transforms
    EllePrimDef::range("image/resize", transform::prim_resize, SIG_ERROR, 3, 4, "Resize image to width x height. Optional filter: :nearest :bilinear :catmull-rom :lanczos3 (default).", "image", "(image/resize img 200 150)"),
    EllePrimDef::exact("image/crop", transform::prim_crop, SIG_ERROR, 5, "Crop a region from an image. Returns new image.", "image", "(image/crop img 10 10 100 100)"),
    EllePrimDef::exact("image/rotate", transform::prim_rotate, SIG_ERROR, 2, "Rotate image by :r90, :r180, or :r270.", "image", "(image/rotate img :r90)"),
    EllePrimDef::exact("image/flip", transform::prim_flip, SIG_ERROR, 2, "Flip image :h (horizontal) or :v (vertical).", "image", "(image/flip img :h)"),
    // Adjustments
    EllePrimDef::exact("image/blur", transform::prim_blur, SIG_ERROR, 2, "Apply Gaussian blur with given sigma.", "image", "(image/blur img 2.0)"),
    EllePrimDef::exact("image/contrast", transform::prim_contrast, SIG_ERROR, 2, "Adjust contrast. Positive values increase, negative decrease.", "image", "(image/contrast img 20.0)"),
    EllePrimDef::exact("image/brighten", transform::prim_brighten, SIG_ERROR, 2, "Adjust brightness. Positive brightens, negative darkens.", "image", "(image/brighten img 30)"),
    EllePrimDef::exact("image/grayscale", transform::prim_grayscale, SIG_ERROR, 1, "Convert image to grayscale.", "image", "(image/grayscale img)"),
    EllePrimDef::exact("image/invert", transform::prim_invert, SIG_ERROR, 1, "Invert all pixel colors.", "image", "(image/invert img)"),
    EllePrimDef::exact("image/hue-rotate", transform::prim_hue_rotate, SIG_ERROR, 2, "Rotate hue by given degrees.", "image", "(image/hue-rotate img 90)"),
    EllePrimDef::exact("image/to-rgba8", transform::prim_to_rgba8, SIG_ERROR, 1, "Convert image to RGBA8 color type.", "image", "(image/to-rgba8 img)"),
    EllePrimDef::exact("image/to-rgb8", transform::prim_to_rgb8, SIG_ERROR, 1, "Convert image to RGB8 color type.", "image", "(image/to-rgb8 img)"),
    EllePrimDef::exact("image/to-luma8", transform::prim_to_luma8, SIG_ERROR, 1, "Convert image to 8-bit grayscale.", "image", "(image/to-luma8 img)"),
    // Drawing
    EllePrimDef::exact("image/draw-line", draw::prim_draw_line, SIG_ERROR, 6, "Draw a line on @image from (x1,y1) to (x2,y2) with color [r g b a].", "image", "(image/draw-line @img 0 0 100 100 [255 0 0 255])"),
    EllePrimDef::exact("image/draw-rect", draw::prim_draw_rect, SIG_ERROR, 6, "Draw a rectangle outline on @image.", "image", "(image/draw-rect @img 10 10 80 60 [0 255 0 255])"),
    EllePrimDef::exact("image/draw-circle", draw::prim_draw_circle, SIG_ERROR, 5, "Draw a circle outline on @image.", "image", "(image/draw-circle @img 50 50 30 [0 0 255 255])"),
    EllePrimDef::exact("image/fill-rect", draw::prim_fill_rect, SIG_ERROR, 6, "Draw a filled rectangle on @image.", "image", "(image/fill-rect @img 10 10 80 60 [255 255 0 255])"),
    EllePrimDef::exact("image/fill-circle", draw::prim_fill_circle, SIG_ERROR, 5, "Draw a filled circle on @image.", "image", "(image/fill-circle @img 50 50 30 [255 0 255 255])"),
    // Compositing
    EllePrimDef::exact("image/overlay", composite::prim_overlay, SIG_ERROR, 4, "Composite overlay image onto base at position (x, y). Alpha-blended.", "image", "(image/overlay base-img overlay-img 10 20)"),
    EllePrimDef::exact("image/blend", composite::prim_blend, SIG_ERROR, 3, "Alpha-blend two same-size images. alpha=0 is all img1, alpha=1 is all img2.", "image", "(image/blend img1 img2 0.5)"),
    // Analysis
    EllePrimDef::exact("image/histogram", analysis::prim_histogram, SIG_ERROR, 1, "Compute per-channel histograms. Returns {:r :g :b :a} with 256-element arrays.", "image", "(image/histogram img)"),
    EllePrimDef::range("image/edges", analysis::prim_edges, SIG_ERROR, 1, 2, "Detect edges. Optional algorithm: :canny (default) or :sobel. Returns grayscale image.", "image", "(image/edges img :canny)"),
    EllePrimDef::exact("image/threshold", analysis::prim_threshold, SIG_ERROR, 2, "Binary threshold: pixels above t become 255, below become 0. Returns grayscale.", "image", "(image/threshold img 128)"),
    EllePrimDef::exact("image/erode", analysis::prim_erode, SIG_ERROR, 2, "Morphological erosion with given radius. Operates on grayscale.", "image", "(image/erode img 2)"),
    EllePrimDef::exact("image/dilate", analysis::prim_dilate, SIG_ERROR, 2, "Morphological dilation with given radius. Operates on grayscale.", "image", "(image/dilate img 2)"),
];
