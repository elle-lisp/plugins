//! Elle selkie plugin — Mermaid diagram rendering via the `selkie-rs` crate.

use std::fs;

use elle_plugin::{ElleCtx, ElleResult, ElleValue, EllePrimDef, SIG_ERROR};

elle_plugin::define_plugin!("selkie/", &PRIMITIVES);

// ---------------------------------------------------------------------------
// Primitives
// ---------------------------------------------------------------------------

extern "C" fn prim_selkie_render(ctx: *mut ElleCtx, args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    if nargs != 1 {
        return a.err(ctx, 
            "arity-error",
            &format!("selkie/render: expected 1 argument, got {}", nargs),
        );
    }
    let diagram = match a.get_string(unsafe { a.arg(args, nargs, 0) }) {
        Some(s) => s.to_string(),
        None => {
            return a.err(ctx, 
                "type-error",
                &format!(
                    "selkie/render: expected string, got {}",
                    a.type_name(unsafe { a.arg(args, nargs, 0) })
                ),
            );
        }
    };
    let parsed = match selkie::parse(&diagram) {
        Ok(d) => d,
        Err(e) => {
            return a.err(ctx, "selkie-error", &format!("selkie/render: parse: {}", e));
        }
    };
    match selkie::render(&parsed) {
        Ok(svg) => a.ok(a.string(ctx, &svg)),
        Err(e) => a.err(ctx, "selkie-error", &format!("selkie/render: render: {}", e)),
    }
}

extern "C" fn prim_selkie_render_to_file(ctx: *mut ElleCtx, args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    if nargs != 2 {
        return a.err(ctx, 
            "arity-error",
            &format!(
                "selkie/render-to-file: expected 2 arguments, got {}",
                nargs
            ),
        );
    }
    let diagram = match a.get_string(unsafe { a.arg(args, nargs, 0) }) {
        Some(s) => s.to_string(),
        None => {
            return a.err(ctx, 
                "type-error",
                &format!(
                    "selkie/render-to-file: expected string, got {}",
                    a.type_name(unsafe { a.arg(args, nargs, 0) })
                ),
            );
        }
    };
    let path = match a.get_string(unsafe { a.arg(args, nargs, 1) }) {
        Some(s) => s.to_string(),
        None => {
            return a.err(ctx, 
                "type-error",
                &format!(
                    "selkie/render-to-file: expected string, got {}",
                    a.type_name(unsafe { a.arg(args, nargs, 1) })
                ),
            );
        }
    };
    let parsed = match selkie::parse(&diagram) {
        Ok(d) => d,
        Err(e) => {
            return a.err(ctx, 
                "selkie-error",
                &format!("selkie/render-to-file: parse: {}", e),
            );
        }
    };
    let svg = match selkie::render(&parsed) {
        Ok(svg) => svg,
        Err(e) => {
            return a.err(ctx, 
                "selkie-error",
                &format!("selkie/render-to-file: render: {}", e),
            );
        }
    };
    match fs::write(&path, &*svg) {
        Ok(()) => a.ok(a.string(ctx, &path)),
        Err(e) => a.err(ctx, "io-error", &format!("selkie/render-to-file: {}", e)),
    }
}

extern "C" fn prim_selkie_render_ascii(ctx: *mut ElleCtx, args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    if nargs != 1 {
        return a.err(ctx, 
            "arity-error",
            &format!(
                "selkie/render-ascii: expected 1 argument, got {}",
                nargs
            ),
        );
    }
    let diagram = match a.get_string(unsafe { a.arg(args, nargs, 0) }) {
        Some(s) => s.to_string(),
        None => {
            return a.err(ctx, 
                "type-error",
                &format!(
                    "selkie/render-ascii: expected string, got {}",
                    a.type_name(unsafe { a.arg(args, nargs, 0) })
                ),
            );
        }
    };
    let parsed = match selkie::parse(&diagram) {
        Ok(d) => d,
        Err(e) => {
            return a.err(ctx, 
                "selkie-error",
                &format!("selkie/render-ascii: parse: {}", e),
            );
        }
    };
    match selkie::render_ascii(&parsed) {
        Ok(ascii) => a.ok(a.string(ctx, &ascii)),
        Err(e) => a.err(ctx, 
            "selkie-error",
            &format!("selkie/render-ascii: render: {}", e),
        ),
    }
}

// ---------------------------------------------------------------------------
// Registration table
// ---------------------------------------------------------------------------

static PRIMITIVES: &[EllePrimDef] = &[
    EllePrimDef::exact(
        "selkie/render",
        prim_selkie_render,
        SIG_ERROR,
        1,
        "Render a Mermaid diagram to SVG",
        "selkie",
        r#"(selkie/render "flowchart LR; A-->B-->C")"#,
    ),
    EllePrimDef::exact(
        "selkie/render-to-file",
        prim_selkie_render_to_file,
        SIG_ERROR,
        2,
        "Render a Mermaid diagram to an SVG file",
        "selkie",
        r#"(selkie/render-to-file "flowchart LR; A-->B" "out.svg")"#,
    ),
    EllePrimDef::exact(
        "selkie/render-ascii",
        prim_selkie_render_ascii,
        SIG_ERROR,
        1,
        "Render a Mermaid diagram to ASCII art",
        "selkie",
        r#"(selkie/render-ascii "flowchart LR; A-->B-->C")"#,
    ),
];
