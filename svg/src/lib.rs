//! Elle SVG plugin — SVG rasterization via resvg.
//!
//! Renders SVG strings (or struct trees emitted to XML) to PNG or raw pixels.
//! Construction and emission live in lib/svg.lisp (pure Elle).

use elle_plugin::{ElleResult, ElleValue, EllePrimDef, SIG_ERROR};

elle_plugin::define_plugin!("svg/", &PRIMITIVES);

// -- Helpers ----------------------------------------------------------------

/// Emit an element struct tree to an SVG XML string.
fn emit_element(a: &elle_plugin::Api, val: ElleValue, out: &mut String) {
    if let Some(s) = a.get_string(val) {
        xml_escape(s, out);
        return;
    }
    if !a.check_struct(val) {
        return;
    }
    let tag_val = a.get_struct_field(val, "tag");
    let tag = match a.get_keyword_name(tag_val) {
        Some(name) => name.to_string(),
        None => return,
    };
    out.push('<');
    out.push_str(&tag);
    // Attributes — the stable ABI doesn't expose struct iteration,
    // so we cannot emit arbitrary attributes from opaque structs.
    // For SVG rendering, the SVG string path is the primary interface.
    let children_val = a.get_struct_field(val, "children");
    let children_len = a.get_array_len(children_val);
    match children_len {
        Some(0) | None => out.push_str("/>"),
        Some(len) => {
            out.push('>');
            for i in 0..len {
                emit_element(a, a.get_array_item(children_val, i), out);
            }
            out.push_str("</");
            out.push_str(&tag);
            out.push('>');
        }
    }
}

fn xml_escape(s: &str, out: &mut String) {
    for c in s.chars() {
        match c {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            _ => out.push(c),
        }
    }
}

/// Get SVG XML string from either a struct tree or a raw string.
fn get_svg_string(val: ElleValue, name: &str) -> Result<String, ElleResult> {
    let a = api();
    if let Some(s) = a.get_string(val) {
        Ok(s.to_string())
    } else if a.check_struct(val) {
        let mut out = String::new();
        out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>");
        emit_element(a, val, &mut out);
        Ok(out)
    } else {
        Err(a.err(
            "type-error",
            &format!(
                "{}: expected SVG string or struct tree, got {}",
                name,
                a.type_name(val)
            ),
        ))
    }
}

fn require_string(val: ElleValue, name: &str, param: &str) -> Result<String, ElleResult> {
    let a = api();
    match a.get_string(val) {
        Some(s) => Ok(s.to_string()),
        None => Err(a.err(
            "type-error",
            &format!(
                "{}: {} must be string, got {}",
                name,
                param,
                a.type_name(val)
            ),
        )),
    }
}

// -- Render helpers ---------------------------------------------------------

struct RenderOpts {
    width: Option<u32>,
    height: Option<u32>,
}

fn parse_render_opts(
    a: &elle_plugin::Api,
    args: *const ElleValue,
    nargs: usize,
    idx: usize,
) -> RenderOpts {
    let mut opts = RenderOpts {
        width: None,
        height: None,
    };
    if nargs > idx {
        let opt_val = a.arg(args, nargs, idx);
        if a.check_struct(opt_val) {
            let w_val = a.get_struct_field(opt_val, "width");
            if let Some(w) = a.get_int(w_val) {
                opts.width = Some(w as u32);
            }
            let h_val = a.get_struct_field(opt_val, "height");
            if let Some(h) = a.get_int(h_val) {
                opts.height = Some(h as u32);
            }
        }
    }
    opts
}

fn render_svg_to_pixmap(
    svg_str: &str,
    opts: &RenderOpts,
) -> Result<resvg::tiny_skia::Pixmap, String> {
    let tree = resvg::usvg::Tree::from_str(svg_str, &resvg::usvg::Options::default())
        .map_err(|e| format!("SVG parse error: {}", e))?;
    let size = tree.size();
    let w = opts.width.unwrap_or(size.width() as u32);
    let h = opts.height.unwrap_or(size.height() as u32);
    let mut pixmap = resvg::tiny_skia::Pixmap::new(w, h)
        .ok_or_else(|| format!("failed to create {}x{} pixmap", w, h))?;
    let sx = w as f32 / size.width();
    let sy = h as f32 / size.height();
    let transform = resvg::tiny_skia::Transform::from_scale(sx, sy);
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    Ok(pixmap)
}

// -- Primitives -------------------------------------------------------------

extern "C" fn prim_render(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let svg_str = match get_svg_string(a.arg(args, nargs, 0), "svg/render") {
        Ok(s) => s,
        Err(e) => return e,
    };
    let opts = parse_render_opts(a, args, nargs, 1);
    match render_svg_to_pixmap(&svg_str, &opts) {
        Ok(pixmap) => match pixmap.encode_png() {
            Ok(png_data) => a.ok(a.bytes(&png_data)),
            Err(e) => a.err("svg-error", &format!("svg/render: PNG encode: {}", e)),
        },
        Err(e) => a.err("svg-error", &format!("svg/render: {}", e)),
    }
}

extern "C" fn prim_render_raw(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let svg_str = match get_svg_string(a.arg(args, nargs, 0), "svg/render-raw") {
        Ok(s) => s,
        Err(e) => return e,
    };
    let opts = parse_render_opts(a, args, nargs, 1);
    match render_svg_to_pixmap(&svg_str, &opts) {
        Ok(pixmap) => {
            let w = pixmap.width();
            let h = pixmap.height();
            let data = pixmap.take();
            a.ok(a.build_struct(&[
                ("width", a.int(w as i64)),
                ("height", a.int(h as i64)),
                ("data", a.bytes(&data)),
            ]))
        }
        Err(e) => a.err("svg-error", &format!("svg/render-raw: {}", e)),
    }
}

extern "C" fn prim_render_to_file(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let svg_str = match get_svg_string(a.arg(args, nargs, 0), "svg/render-to-file") {
        Ok(s) => s,
        Err(e) => return e,
    };
    let path = match require_string(a.arg(args, nargs, 1), "svg/render-to-file", "path") {
        Ok(s) => s,
        Err(e) => return e,
    };
    let opts = parse_render_opts(a, args, nargs, 2);
    match render_svg_to_pixmap(&svg_str, &opts) {
        Ok(pixmap) => match pixmap.save_png(&path) {
            Ok(()) => a.ok(a.nil()),
            Err(e) => a.err("svg-error", &format!("svg/render-to-file: {}", e)),
        },
        Err(e) => a.err("svg-error", &format!("svg/render-to-file: {}", e)),
    }
}

extern "C" fn prim_dimensions(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let svg_str = match get_svg_string(a.arg(args, nargs, 0), "svg/dimensions") {
        Ok(s) => s,
        Err(e) => return e,
    };
    match resvg::usvg::Tree::from_str(&svg_str, &resvg::usvg::Options::default()) {
        Ok(tree) => {
            let size = tree.size();
            let elems = [
                a.float(size.width() as f64),
                a.float(size.height() as f64),
            ];
            a.ok(a.array(&elems))
        }
        Err(e) => a.err("svg-error", &format!("svg/dimensions: {}", e)),
    }
}

// -- Registration -----------------------------------------------------------

static PRIMITIVES: &[EllePrimDef] = &[
    EllePrimDef::range(
        "svg/render",
        prim_render,
        SIG_ERROR,
        1,
        2,
        "Render SVG (string or struct tree) to PNG bytes. Optional opts: {:width N :height N}.",
        "svg",
        "(svg/render \"<svg width='100' height='100'><circle cx='50' cy='50' r='40' fill='red'/></svg>\")",
    ),
    EllePrimDef::range(
        "svg/render-raw",
        prim_render_raw,
        SIG_ERROR,
        1,
        2,
        "Render SVG to raw RGBA8 pixels. Returns {:width :height :data bytes}.",
        "svg",
        "(svg/render-raw svg-string)",
    ),
    EllePrimDef::range(
        "svg/render-to-file",
        prim_render_to_file,
        SIG_ERROR,
        2,
        3,
        "Render SVG to a PNG file.",
        "svg",
        "(svg/render-to-file svg-string \"output.png\")",
    ),
    EllePrimDef::exact(
        "svg/dimensions",
        prim_dimensions,
        SIG_ERROR,
        1,
        "Return [width height] of an SVG's intrinsic dimensions.",
        "svg",
        "(svg/dimensions \"<svg width='100' height='200'></svg>\")",
    ),
];
